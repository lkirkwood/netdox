#!lua name=netdox

--- UTIL

local function list_to_map(list)
  local is_key = true
  local last_key = nil
  local map = {}

  for value in list do
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

--- CHANGELOG

local function create_change(change, value, plugin)
  redis.call('XADD', 'changelog', '*', 'change', change, 'value', value, 'plugin', plugin)
end

--- DNS

local DNS_KEY = 'dns'

local function create_dns(name, args)
  local name = name[1]
  local plugin, value, rtype = unpack(args)

  if redis.call('SADD', DNS_KEY, name) ~= 0 then
    create_change('create dns name', name, plugin)
  end

  redis.call('SADD', string.format('%s;%s;plugins', DNS_KEY, name), plugin)

  if value ~= nil and rtype ~= nil then
    local value_set = string.format('%s;%s;%s;%s', DNS_KEY, name, plugin, rtype)
    if redis.call('SADD', value_set, value) ~= 0 then
      create_change(
        'create dns record',
        string.format('%s --(%s)-> %s', name, rtype, value),
        plugin
      )
    end
  end
end

--- NODES

local NODES_KEY = 'nodes'

local function create_node(dns_names, args)
  local node_id = dns_names_to_node_id(dns_names)
  local node_key = string.format('%s;%s', NODES_KEY, node_id)
  local plugin, name, exclusive, link_id = unpack(args)

  -- Record node exists with these dns names.
  if redis.call('SADD', NODES_KEY, node_id) ~= 0 then
    create_change(
      'create node with dns names',
      table.concat(dns_names, ', '),
      plugin
    )
    redis.call('SADD', string.format('%s;plugins', node_key), plugin)
  end

  local node_plugin_details = string.format('%s;%s', node_key, plugin)

  -- Record changes to plugin version of node details
  if name ~= nil then
    local old_name = redis.call('HGET', node_plugin_details, 'name')
    if old_name ~= name then
      create_change(
        'plugin updated node name',
        string.format('(%s) %s ---> %s', node_id, old_name, name),
        plugin
      )
      redis.call('HSET', node_plugin_details, 'name', name)
    end
  end

  if exclusive ~= nil then
    local old_exc = redis.call('HGET', node_plugin_details, 'exclusive')
    if old_exc ~= exclusive then
      create_change(
        'plugin updated node exclusivity',
        string.format('(%s) %s ---> %s', node_key, tostring(old_exc), exclusive),
        plugin
      )
      redis.call('HSET', node_plugin_details, 'exclusive', exclusive)
    end
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

local function create_metadata(id, plugin, key, value)
  local meta_key = string.format('meta;%s', id)
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
  create_dns(names[1], {args[1]})
  create_metadata(names[1], unpack(args))
end

local function create_node_metadata(names, args)
  create_node(names, {args[1]})
  create_metadata(dns_names_to_node_id(names), unpack(args))
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
  local dtype = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local title = table.remove(args, 1)
  local data = args

  create_dns(names[1], {plugin})
  create_plugin_data(names[1], dtype, plugin, title, data)
end

local function create_node_plugin_data(names, args)
  local dtype = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local title = table.remove(args, 1)
  local data = args

  create_node(names, {plugin})
  create_plugin_data(
    dns_names_to_node_id(names),
    dtype, plugin, title, data
  )
end


--- FUNCTION REGISTRATION

redis.register_function('netdox_create_dns', create_dns)
redis.register_function('netdox_create_node', create_node)

redis.register_function('netdox_create_dns_plugin_data', create_dns_plugin_data)
redis.register_function('netdox_create_node_plugin_data', create_node_plugin_data)

redis.register_function('netdox_create_dns_metadata', create_dns_metadata)
redis.register_function('netdox_create_node_metadata', create_node_metadata)
