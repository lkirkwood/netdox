#!lua name=netdox

--- UTIL

local function list_to_map(list)
  local last_key = nil
  local map = {}

  for _, value in ipairs(list) do
    if last_key == nil then
      last_key = value
    else
      map[last_key] = value
      last_key = nil
    end
  end

  return map
end

local function map_to_list(map)
  local list = {}
  local index = 0

  for key, value in pairs(map) do
    index = index + 1
    list[index] = key
    index = index + 1
    list[index] = value
  end

  return list
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

local function create_dns(names, args)
  local qname = qualify_dns_name(names[1])
  local plugin, rtype, value = unpack(args)

  if rtype ~= nil then
    rtype = string.upper(rtype)
  end

  if redis.call('SADD', DNS_KEY, qname) ~= 0 then
    create_change('create dns name', DNS_KEY, plugin)
  end

  local name_plugins = string.format('%s;%s;plugins', DNS_KEY, qname)
  if redis.call('SADD', name_plugins, plugin) ~= 0 then
    create_change('add plugin to dns name', name_plugins, plugin)
  end

  if value ~= nil and rtype ~= nil then
    -- Record record type for name and plugin.
    local name_plugin_rtypes = string.format('%s;%s;%s', DNS_KEY, qname, plugin)
    if redis.call('SADD', name_plugin_rtypes, rtype) ~= 0 then
      create_change(
        'add record type to plugin dns name',
        name_plugin_rtypes, plugin
      )
    end

    -- Qualify value if it is an address.
    if ADDRESS_RTYPES[rtype] then
      value = qualify_dns_name(value)
    end

    -- Add value to set.
    local value_set = string.format('%s;%s;%s;%s', DNS_KEY, qname, plugin, rtype)
    if redis.call('SADD', value_set, value) ~= 0 then
      create_change('create dns record', value_set, plugin)
    end
  end
end

-- TODO review this
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
      create_change('updated network mapping', maps_key, plugin)
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
  exclusive = exclusive or "false"

  redis.call('SADD', NODES_KEY, node_id)

  local plugin_node = string.format('%s;%s', node_key, plugin)
  local node_count = redis.call('GET', plugin_node)
  for index=1,node_count do
    local details = redis.call('HGETALL', string.format('%s;%s', plugin_node, index))
    if
      details["name"] == name and
      details["exclusive"] == exclusive and
      details["link_id"] == link_id then
        return
    end
  end

  local index = redis.call('INCR', plugin_node)
  local node_details = string.format('%s;%s', plugin_node, index)
  redis.call('HSET', node_details, 'name', name)
  redis.call('HSET', node_details, 'exclusive', exclusive)
  redis.call('HSET', node_details, 'link_id', link_id)
  create_change('create plugin node', node_details, plugin)

  return node_details
end

--- METADATA

local METADATA_KEY = 'meta'

local function create_metadata(id, plugin, key, value)
  redis.call('SADD', METADATA_KEY, id)
  local meta_key = string.format('meta;%s', id)

  local old_val = redis.call('HGET', meta_key, key)
  if old_val ~= value then
    create_change('updated metadata', meta_key, plugin)
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

  local node_count_key = string.format("%s;%s", NODES_KEY, node_id)
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

local function create_plugin_data_list(obj_key, pdata_id, plugin, args)
  local list_title = table.remove(args, 1)
  local item_title = table.remove(args, 1)

  local data_key = string.format('%s;%s;%s', PLUGIN_DATA_KEY, obj_key, pdata_id)
  if redis.call('TYPE', data_key) ~= 'list' then
    redis.call('DEL', data_key)
  end

  local details_key = string.format('%s;details', data_key)
  redis.call('HSET', details_key, 'type', 'list')
  redis.call('HSET', details_key, 'plugin', plugin)
  redis.call('HSET', details_key, 'list_title', list_title)
  redis.call('HSET', details_key, 'item_title', item_title)

  if redis.call('LRANGE', data_key, 0, -1) ~= args then
    redis.call('DEL', data_key)
    redis.call('LPUSH', data_key, unpack(args))
    create_change('updated plugin data list', data_key, plugin)
  end
end

local function create_plugin_data_hash(obj_key, pdata_id, plugin, args)
  local title = table.remove(args, 1)
  local data = list_to_map(args)

  local data_key = string.format('%s;%s;%s', PLUGIN_DATA_KEY, obj_key, pdata_id)
  if redis.call('TYPE', data_key) ~= 'hash' then
    redis.call('DEL', data_key)
  end

  local details_key = string.format('%s;details', data_key)
  redis.call('HSET', details_key, 'type', 'hash')
  redis.call('HSET', details_key, 'plugin', plugin)
  redis.call('HSET', details_key, 'title', title)

  if redis.call('HGETALL', data_key) ~= args then
    redis.call('DEL', data_key)
    redis.call('HSET', data_key, map_to_list(args))
    create_change('updated plugin data map', data_key, plugin)
  end
end

local function create_plugin_data_str(obj_key, pdata_id, plugin, args)
  local title = table.remove(args, 1)
  local content_type = table.remove(args, 1)
  local content = table.remove(args, 1)

  local data_key = string.format('%s;%s;%s', PLUGIN_DATA_KEY, obj_key, pdata_id)

  local details_key = string.format('%s;details', data_key)
  redis.call('HSET', details_key, 'type', 'string')
  redis.call('HSET', details_key, 'plugin', plugin)
  redis.call('HSET', details_key, 'title', title)
  redis.call('HSET', details_key, 'content_type', content_type)

  if redis.call('GET', data_key) ~= content then
    redis.call('SET', data_key, content)
    create_change('updated plugin data string', data_key, plugin)
  end
end

local function create_plugin_data(obj_key, args)
  local plugin = table.remove(args, 1)
  local dtype = table.remove(args, 1)
  local pdata_id = table.remove(args, 1)

  if dtype == 'list' then
    return create_plugin_data_list(obj_key, pdata_id, plugin, args)
  elseif dtype == 'hash' then
    return create_plugin_data_hash(obj_key, pdata_id, plugin, args)
  elseif dtype == 'string' then
    return create_plugin_data_str(obj_key, pdata_id, plugin, args)
  else
    return string.format("Invalid plugin data type: %s", tostring(dtype))
  end
end

local function create_dns_plugin_data(names, args)
  local qname = qualify_dns_name(names[1])
  local plugin = args[1]

  create_dns({qname}, {plugin})
  return create_plugin_data(
    string.format("%s;%s", DNS_KEY, qname), args
  )
end

local function create_node_plugin_data(names, args)
  local qnames = qualify_dns_names(names)
  local node_id = dns_names_to_node_id(qnames)
  local plugin = args[1]

  create_node(qnames, {plugin})
  return create_plugin_data(
    string.format("%s;%s", NODES_KEY, node_id), args
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
