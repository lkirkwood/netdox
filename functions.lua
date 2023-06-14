#!lua name=netdox

--- CHANGELOG

local function create_change(change, value, plugin)
  redis.call('XADD', 'changelog', '*', 'change', change, 'value', value, 'plugin', plugin)
end

--- DNS

local DNS_KEY = 'dns'

local function create_dns(_, args)
  local name, rtype, value, plugin = unpack(args)
  if name == nil then return 'Must provide dns record name'
  elseif rtype == nil then return 'Must provide dns record type'
  elseif value == nil then return 'Must provide dns record value'
  elseif plugin == nil then return 'Must provide plugin name'
  end

  if redis.call('SADD', DNS_KEY, name) ~= 0 then
    create_change('create dns name', name, plugin)
  end

  redis.call('SADD', string.format('%s;%s;plugins', DNS_KEY, name), plugin)

  local value_set = string.format('%s;%s;%s;%s', DNS_KEY, name, plugin, rtype)
  if redis.call('SADD', value_set, value) ~= 0 then
    create_change(
      'create dns record',
      string.format('%s --(%s)-> %s', name, rtype, value),
      plugin
    )
  end
end

--- NODES

local NODES_KEY = 'nodes'

local function create_node(_, args)
  local name = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local exclusive = table.remove(args, 1)
  local dns_names = args
  table.sort(dns_names)

  local node_id = table.concat(dns_names, ';')
  local node_key = string.format('%s;%s', NODES_KEY, node_id)

  -- Record node exists with these dns names.
  if redis.call('SADD', NODES_KEY, node_id) ~= 0 then
    create_change(
      'create node with names',
      table.concat(dns_names, ', '),
      plugin
    )
  end

  -- Add plugin to list of plugins providing a node with these dns names.
  redis.call('SADD', string.format('%s;plugins', node_key), plugin)

  local node_plugin_details = string.format('%s;%s', node_key, plugin)

  -- Record changes to plugin version of node details
  local old_name = redis.call('HGET', node_plugin_details, 'name')
  if old_name ~= name then
    create_change(
      'plugin updated node name',
      string.format('(%s) %s ---> %s', node_id, old_name, name),
      plugin
    )
  end

  local old_exc = redis.call('HGET', node_plugin_details, 'exclusive')
  if old_exc ~= exclusive then
    create_change(
      'plugin updated node exclusivity',
      string.format('(%s) %s ---> %s', node_key, old_exc, exclusive),
      plugin
    )
  end

  -- Update plugin version of node details
  redis.call('HSET', node_plugin_details,
    'name', name, 'exclusive', exclusive
  )

  return node_key
end

--- METADATA

local function create_metadata(_, args)
  local id, key, value, plugin = unpack(args)
  if id == nil then return 'Must provide metadata object ID'
  elseif key == nil then return 'Must provide metadata key'
  elseif value == nil then return 'Must provide metadata value'
  elseif plugin == nil then return 'Must provide metadata source plugin'
  end

  local meta_key = string.format('meta;%s', id)
  local old_val = redis.call('HGET', meta_key, key)
  if old_val ~= value then
    create_change(
      'updated metadata',
      string.format('(%s) %s: %s ---> %s', id, key, old_val, value),
      plugin
    )
    redis.call('HSET', meta_key, key, value)
  end
end

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

local function create_dns_plugin_data(_, args)
  local name = table.remove(args, 1)
  local dtype = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local title = table.remove(args, 1)
  local data = args
  create_plugin_data(name, dtype, plugin, title, data)
end

local function create_node_plugin_data(_, args)
  local namestr = table.remove(args, 1)
  local dtype = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local title = table.remove(args, 1)
  local data = args

  local names = {}
  for name in string.gmatch(namestr, "[^;]+") do
    table.insert(names, name)
  end
  table.sort(names)
  local node_id = table.concat(names, ';')

  create_plugin_data(node_id, dtype, plugin, title, data)
end


--- FUNCTION REGISTRATION

redis.register_function('netdox_create_dns', create_dns)
redis.register_function('netdox_create_node', create_node)
redis.register_function('netdox_create_dns_plugin_data', create_dns_plugin_data)
redis.register_function('netdox_create_node_plugin_data', create_node_plugin_data)
redis.register_function('netdox_create_metadata', create_metadata)
