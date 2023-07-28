#!lua name=netdox

--- UTIL

local function list_to_map(list)
  local is_key = true
  local last_key = nil
  local map = {}

  for _, value in pairs(list) do
    if is_key then
      is_key = false
      last_key = value
    else
      is_key = true
      map[last_key] = value
    end
  end

  return map
end

local function dns_names_to_node_id(names)
  table.sort(names)
  return table.concat(names, ';')
end

local DEFAULT_NETWORK_KEY = 'default_network'
local NETWORK_PATTERN = '%[[%w_-]+%]'

local function is_qualified(name)
  local startindex, _ = string.find(name, NETWORK_PATTERN)
  return startindex == 1
end

local function qualify_dns_name(name)
  if is_qualified(name) then
    return name
  else
    return string.format('[%s]%s', redis.call('GET', DEFAULT_NETWORK_KEY), name)
  end
end

local function qualify_dns_names(names)
  for i, name in pairs(names) do
    names[i] = qualify_dns_name(name)
  end
  return names
end

local ADDRESS_RTYPES = {["CNAME"] = true, ["A"] = true, ["PTR"] = true}

--- CHANGELOG

local function create_change(change, value, plugin)
  redis.call('XADD', 'changelog', '*', 'change', change, 'value', value, 'plugin', plugin)
end

--- DNS

local DNS_KEY = 'dns'

-- TODO add case folding to record types
local function create_dns(names, args)
  local qname = qualify_dns_name(names[1])
  local plugin, rtype, value = unpack(args)

  if rtype ~= nil then
    rtype = string.upper(rtype)
  end

  if redis.call('SADD', DNS_KEY, qname) ~= 0 then
    create_change('create dns name', qname, plugin)
  end

  redis.call('SADD', string.format('%s;%s;plugins', DNS_KEY, qname), plugin)

  if value ~= nil and rtype ~= nil then
    -- Record record type for name and plugin.
    redis.call('SADD', string.format('%s;%s;%s', DNS_KEY, qname, plugin), rtype)

    -- Qualify value if it is an address.
    if ADDRESS_RTYPES[rtype] then
      value = qualify_dns_name(value)
    end

    -- Add value to set.
    local value_set = string.format('%s;%s;%s;%s', DNS_KEY, qname, plugin, rtype)
    if redis.call('SADD', value_set, value) ~= 0 then
      create_change(
        'create dns record',
        string.format('%s --(%s)-> %s', qname, rtype, value),
        plugin
      )
    end
  end
end

local function map_dns(names, args)
  local origin = names[1]
  local net_start, net_end = string.find(origin, NETWORK_PATTERN)
  if net_start ~= 1 then
    return "Origin DNS name must be qualified with a network."
  end
  local origin_name = string.sub(origin, net_end + 1)
  local origin_net = string.sub(origin, net_start, net_end)

  local plugin, reverse = table.remove(args, 1), table.remove(args, 1)
  create_dns({origin}, {plugin})

  for _, dest in pairs(args) do
    local _net_start, _net_end = string.find(dest, NETWORK_PATTERN)
    if _net_start ~= 1 then
      return "Destination DNS name must be qualified with a network."
    end
    local dest_net = string.sub(dest, _net_start, _net_end)
    local dest_name = string.sub(dest, _net_end + 1)
    create_dns({dest}, {plugin})

    local maps_key = string.format('%s;%s;maps', DNS_KEY, origin)
    local old = redis.call('HGET', maps_key, dest_net)
    if old ~= dest then
      create_change(
        'updated network mapping',
        string.format('%s --%s-> %s', origin, dest_net, dest_name),
        plugin
      )
      redis.call('HSET', maps_key, dest_net, dest_name)
    end

    if reverse == 'true' then
      map_dns({dest}, {plugin, 'false', origin})
    end
  end
end

--- NODES

local NODES_KEY = 'nodes'

local function create_node(dns_names, args)
  local dns_qnames = qualify_dns_names(dns_names)

  local node_id = dns_names_to_node_id(dns_qnames)
  local node_key = string.format('%s;%s', NODES_KEY, node_id)
  local plugin, name, exclusive, link_id = unpack(args)

  -- Record node exists with these dns names.
  if redis.call('SADD', NODES_KEY, node_id) ~= 0 then
    create_change(
      'create node with dns names',
      table.concat(dns_qnames, ', '),
      plugin
    )
  end
  redis.call('SADD', string.format('%s;plugins', node_key), plugin)

  local node_plugin_count = string.format('%s;%s', node_key, plugin)
  local index = redis.call('INCR', node_plugin_count)
  local node_plugin_details = string.format('%s;%s;%s', node_key, plugin, index)

  -- Record changes to plugin version of node details
  if name ~= nil then
    local old_name = redis.call('HGET', node_plugin_details, 'name')
    if old_name ~= name then
      create_change(
        'plugin updated node name',
        string.format('(%s) %s ---> %s', node_id, tostring(old_name), name),
        plugin
      )
      redis.call('HSET', node_plugin_details, 'name', name)
    end
  end

  if exclusive == nil then
    exclusive = "false"
  end
  local old_exc = redis.call('HGET', node_plugin_details, 'exclusive')
  if old_exc == nil or old_exc ~= exclusive then
    create_change(
      'plugin updated node exclusivity',
      string.format('(%s) %s ---> %s', node_key, tostring(old_exc), exclusive),
      plugin
    )
    redis.call('HSET', node_plugin_details, 'exclusive', exclusive)
  end

  if link_id ~= nil then
    local old_link_id = redis.call('HGET', node_plugin_details, 'link_id')
    if old_link_id ~= link_id then
      create_change(
        'plugin updated node link id',
        string.format('(%s) %s ---> %s', node_key, tostring(old_link_id), link_id),
        plugin
      )
      redis.call('HSET', node_plugin_details, 'link_id', link_id)
    end
  end

  return node_key
end

--- METADATA

local METADATA_KEY = 'meta'

local function create_metadata(id, plugin, key, value)
  local meta_key = string.format('meta;%s', id)
  redis.call('SADD', METADATA_KEY, id)
  local old_val = redis.call('HGET', meta_key, key)
  if old_val ~= value then
    create_change(
      'updated metadata',
      string.format('(%s) %s: %s ---> %s', id, key, tostring(old_val), value),
      plugin
    )
    redis.call('HSET', meta_key, key, value)
  end
end

local function create_dns_metadata(names, args)
  local qname = qualify_dns_name(names[1])
  local plugin = table.remove(args, 1)

  create_dns({qname}, {plugin})
  for key, val in pairs(list_to_map(args)) do
    create_metadata(
      string.format("%s;%s", DNS_KEY, qname), plugin, key, val
    )
  end
end

local function create_node_metadata(names, args)
  local qnames = qualify_dns_names(names)
  local plugin = table.remove(args, 1)

  local node_id = dns_names_to_node_id(qnames)

  local node_count_key = string.format("%s;%s;%s", NODES_KEY, node_id, plugin)
  if not redis.call('GET', node_count_key) then
    create_node(qnames, {plugin})
  end

  for key, val in pairs(list_to_map(args)) do
    create_metadata(
      string.format('%s;%s', NODES_KEY, node_id),
      plugin, key, val
    )
  end
end

--- PLUGIN DATA

local PLUGIN_DATA_KEY = 'pdata'

local function create_plugin_data_list(id, plugin, title, list)
  local data_key = string.format('%s;%s;%s', PLUGIN_DATA_KEY, id, title)
  if redis.call('TYPE', data_key) ~= 'list' then
    redis.call('DEL', data_key)
  end

  for index, value in pairs(list) do
    if redis.call('LINDEX', data_key, index) ~= value then
      create_change(
        'updated plugin data list',
        string.format('(%s) index %d: %s', title, index, value),
        plugin
      )
      redis.call('LSET', data_key, index, value)
    end
  end
end

local function create_plugin_data_map(id, plugin, title, map)
  local data_key = string.format('%s;%s;%s', PLUGIN_DATA_KEY, id, title)
  if redis.call('TYPE', data_key) ~= 'hash' then
    redis.call('DEL', data_key)
  end

  for key, value in pairs(map) do
    if redis.call('HGET', data_key, key) ~= value then
      create_change(
        'updated plugin data map',
        string.format('(%s) key %s: %s', title, key, value),
        plugin
      )
      redis.call('HSET', data_key, key, value)
    end
  end
end

local function create_plugin_data_str(id, plugin, title, str)
  local data_key = string.format('%s;%s;%s', PLUGIN_DATA_KEY, id, title)
  if redis.call('GET', data_key) ~= str then
    create_change(
      'updated plugin data string',
      string.format('(%s) %s', title, str),
      plugin
    )
    redis.call('SET', data_key, str)
  end
end

local function create_plugin_data(id, dtype, plugin, title, data)
  if dtype == 'array' then
    create_plugin_data_list(id, plugin, title, data)
  elseif dtype == 'map' then
    create_plugin_data_map(id, plugin, title, list_to_map)
  elseif dtype == 'string' then
    create_plugin_data_str(id, plugin, title, data)
  end
end

local function create_dns_plugin_data(names, args)
  local qname = qualify_dns_name(names[1])
  local dtype = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local title = table.remove(args, 1)
  local data = args

  create_dns({qname}, {plugin})
  create_plugin_data(qname, dtype, plugin, title, data)
end

local function create_node_plugin_data(names, args)
  local qnames = qualify_dns_names(names)
  local dtype = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local title = table.remove(args, 1)
  local data = args

  create_node(qnames, {plugin})
  create_plugin_data(
    dns_names_to_node_id(qnames),
    dtype, plugin, title, data
  )
end


--- FUNCTION REGISTRATION

redis.register_function('netdox_create_dns', create_dns)
redis.register_function('netdox_map_dns', map_dns)

redis.register_function('netdox_create_node', create_node)

redis.register_function('netdox_create_dns_metadata', create_dns_metadata)
redis.register_function('netdox_create_node_metadata', create_node_metadata)

redis.register_function('netdox_create_dns_plugin_data', create_dns_plugin_data)
redis.register_function('netdox_create_node_plugin_data', create_node_plugin_data)

-- TODO add input sanitization
