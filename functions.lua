#!lua name=netdox

local function create_change(change, value, plugin)
  redis.call('XADD', 'changelog', '*', 'change', change, 'value', value, 'plugin', plugin)
end

local DNS_KEY = 'dns'

local function create_dns(_, args)
  local name, rtype, value, plugin = unpack(args)
  if name == nil then return 'Must provide dns record name'
  elseif rtype == nil then return 'Must provide dns record type'
  elseif value == nil then return 'Must provide dns record value'
  elseif plugin == nil then return 'Must provide plugin name'
  end

  if redis.call('SISMEMBER', DNS_KEY, name) == 0 then
    create_change('create dns name', name, plugin)
    redis.call('SADD', DNS_KEY, name)
  end

  redis.call('SADD', string.format('%s;%s;plugins', DNS_KEY, name), plugin)

  local value_set = string.format('%s;%s;%s;%s', DNS_KEY, name, plugin, rtype)
  if redis.call('SISMEMBER', value_set, value) == 0 then
    create_change(
      'create dns record',
      string.format('%s --(%s)-> %s', name, rtype, value),
      plugin
    )
    redis.call('SADD', value_set, value)
  end
end

local NODE_KEY = 'nodes'

local function create_node(_, args)
  local name = table.remove(args, 1)
  local plugin = table.remove(args, 1)
  local exclusive = table.remove(args, 1)
  local dns_names = args
  table.sort(dns_names)

  local node = string.format('%s;%s', NODE_KEY, table.concat(dns_names, ';'))

  -- Record node exists with these dns names.
  if redis.call('SISMEMBER', NODE_KEY, node) == 0 then
    create_change(
      'create node with names',
      table.concat(dns_names, ', '),
      plugin
    )
    redis.call('SADD', NODE_KEY, node)
  end

  -- Add plugin to list of plugins providing a node with these dns names.
  redis.call('SADD', string.format('%s;plugins', node), plugin)

  local node_plugin_details = string.format('%s;%s', node, plugin)

  -- Record changes to plugin version of node details
  local old_name = redis.call('HGET', node_plugin_details, 'name')
  if old_name ~= name then
    create_change(
      'plugin updated node name',
      string.format('(%s) %s ---> %s', node, old_name, name),
      plugin
    )
  end

  local old_exc = redis.call('HGET', node_plugin_details, 'exclusive')
  if old_exc ~= exclusive then
    create_change(
      'plugin updated node exclusivity',
      string.format('(%s) %s ---> %s', node, old_exc, exclusive),
      plugin
    )
  end

  -- Update plugin version of node details
  redis.call('HSET', node_plugin_details,
    'name', name, 'exclusive', exclusive
  )

  return node
end

redis.register_function('netdox_create_change', create_change)
redis.register_function('netdox_create_dns', create_dns)
redis.register_function('netdox_create_node', create_node)
