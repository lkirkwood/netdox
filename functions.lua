#!lua name=netdox

-- TODO make changelog change types constants

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

--- Returns true if the lists are equal.
local function cmp_lists(t1, t2)
    for i, value in ipairs(t1) do
        if t2[i] ~= value then
            return false
        end
    end
    return #t2 == #t1
end

local function dns_names_to_node_id(names)
    table.sort(names)
    return table.concat(names, ";")
end

local DEFAULT_NETWORK_KEY = "default_network"
local NETWORK_PATTERN = "%[[%w_-]+%]"

local function is_qualified(name)
    local startindex, _ = string.find(name, NETWORK_PATTERN)
    return startindex == 1
end

local function qualify_dns_name(name)
    if is_qualified(name) then
        return string.lower(name)
    else
        return string.format("[%s]%s", redis.call("GET", DEFAULT_NETWORK_KEY), string.lower(name))
    end
end

local function qualify_dns_names(names)
    for i, name in ipairs(names) do
        names[i] = qualify_dns_name(name)
    end
    return names
end

local ADDRESS_RTYPES = { ["CNAME"] = true, ["A"] = true, ["PTR"] = true }

--- CHANGELOG

local CHANGELOG_KEY = "changelog"

local function create_change(change, value, plugin)
    redis.call("XADD", CHANGELOG_KEY, "*", "change", change, "value", value, "plugin", plugin)
end

--- DNS

local DNS_KEY = "dns"
local DNS_IGNORE_KEY = "dns_ignore"

local function create_dns(names, args)
    local qname = qualify_dns_name(names[1])

    if redis.call("SISMEMBER", DNS_IGNORE_KEY, qname) == 1 then
        return
    end

    local plugin, rtype, value = unpack(args)

    if rtype ~= nil then
        rtype = string.upper(rtype)
    end

    if redis.call("SADD", DNS_KEY, qname) ~= 0 then
        create_change("create dns name", qname, plugin)
    end

    local name_plugins = string.format("%s;%s;plugins", DNS_KEY, qname)
    redis.call("SADD", name_plugins, plugin)

    if value ~= nil and rtype ~= nil then
        -- Record record type for name and plugin.
        local name_plugin_rtypes = string.format("%s;%s;%s", DNS_KEY, qname, plugin)
        redis.call("SADD", name_plugin_rtypes, rtype)

        -- Qualify value if it is an address.
        if ADDRESS_RTYPES[rtype] then
            value = qualify_dns_name(value)
            create_dns({ value }, { plugin })
        end

        -- Add value to set.
        local value_set = string.format("%s;%s;%s;%s", DNS_KEY, qname, plugin, rtype)
        if redis.call("SADD", value_set, value) ~= 0 then
            create_change("create dns record", string.format("%s;%s", value_set, value), plugin)
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
    create_dns({ origin }, { plugin })

    for _, dest in pairs(args) do
        local _net_start, _net_end = string.find(dest, NETWORK_PATTERN)
        if _net_start ~= 1 then
            return "Destination DNS name must be qualified with a network."
        end
        local dest_net = string.sub(dest, _net_start, _net_end)
        local dest_name = string.sub(dest, _net_end + 1)
        create_dns({ dest }, { plugin })

        local maps_key = string.format("%s;%s;maps", DNS_KEY, origin)
        local old = redis.call("HGET", maps_key, dest_net)
        if old ~= dest then
            create_change("updated network mapping", maps_key, plugin)
            redis.call("HSET", maps_key, dest_net, dest_name)
        end

        if reverse == "true" then
            map_dns({ dest }, { plugin, "false", origin })
        end
    end
end

--- NODES

local NODES_KEY = "nodes"

local function create_node(dns_names, args)
    local dns_qnames = qualify_dns_names(dns_names)

    local plugin, name, exclusive, link_id = unpack(args)
    exclusive = exclusive or "false"

    for _, qname in ipairs(dns_qnames) do
        create_dns({ qname }, { plugin })
    end

    local node_id = dns_names_to_node_id(dns_qnames)
    redis.call("SADD", NODES_KEY, node_id)

    local node_key = string.format("%s;%s", NODES_KEY, node_id)
    local node_count = tonumber(redis.call("GET", node_key))
    if node_count == nil then
        node_count = 0
    end

    for index = 1, node_count do
        local details = list_to_map(redis.call("HGETALL", string.format("%s;%s", node_key, index)))
        if
            details["plugin"] == plugin
            and details["name"] == name
            and details["exclusive"] == exclusive
            and details["link_id"] == link_id
        then
            return
        end
    end

    local index = redis.call("INCR", node_key)
    local node_details = string.format("%s;%s", node_key, index)
    redis.call("HSET", node_details, "plugin", plugin)
    if name ~= nil then
        redis.call("HSET", node_details, "name", name)
    end
    redis.call("HSET", node_details, "exclusive", exclusive)
    if link_id ~= nil then
        redis.call("HSET", node_details, "link_id", link_id)
    end

    create_change("create plugin node", node_id, plugin)

    return node_details
end

--- METADATA

local METADATA_KEY = "meta"

local function create_metadata(id, plugin, args)
    local changed = false

    if redis.call("SADD", METADATA_KEY, id) then
        changed = true
    end

    local meta_key = string.format("meta;%s", id)
    local meta_plugins = string.format("%s;plugins", meta_key)

    if redis.call("SISMEMBER", meta_plugins, plugin) == 0 then
        changed = true
        redis.call("SADD", meta_plugins, plugin)
    end

    local old_vals = list_to_map(redis.call("HGETALL", meta_key))

    for key, value in pairs(list_to_map(args)) do
        if old_vals[key] ~= value then
            changed = true
            redis.call("HSET", meta_key, key, value)
        end
    end

    if changed then
        create_change("updated metadata", meta_key, plugin)
    end
end

local function create_dns_metadata(names, args)
    local qname = qualify_dns_name(names[1])
    local plugin = table.remove(args, 1)

    create_dns({ qname }, { plugin })
    create_metadata(string.format("%s;%s", DNS_KEY, qname), plugin, args)
end

local function create_node_metadata(names, args)
    local qnames = qualify_dns_names(names)
    local plugin = table.remove(args, 1)

    local node_id = dns_names_to_node_id(qnames)
    if redis.call("SISMEMBER", NODES_KEY, node_id) == 0 then
        create_node(qnames, { plugin })
    end

    create_metadata(string.format("%s;%s", NODES_KEY, node_id), plugin, args)
end

-- DATA

local function create_data_str(data_key, plugin, title, content_type, content)
    local created = false
    local changed = false
    local details_key = string.format("%s;details", data_key)
    local dtype = redis.call("TYPE", details_key)["ok"]

    if dtype == "none" then
        created = true
    elseif dtype ~= "hash" then
        redis.call("DEL", data_key)
        changed = true
    end

    local details_key = string.format("%s;details", data_key)
    local old_details = list_to_map(redis.call("HGETALL", details_key))
    local new_details = {
        type = "string",
        plugin = plugin,
        title = title,
        content_type = content_type,
    }

    if
        not (
            old_details["type"] == new_details["type"]
            and old_details["plugin"] == new_details["plugin"]
            and old_details["title"] == new_details["title"]
            and old_details["content_type"] == new_details["content_type"]
        )
    then
        redis.call("HSET", details_key, unpack(map_to_list(new_details)))
        changed = true
    end

    if redis.call("GET", data_key) ~= content then
        redis.call("SET", data_key, content)
        changed = true
    end

    if created == true then
        create_change("created data", data_key, plugin)
    elseif changed == true and created == false then
        create_change("updated data", data_key, plugin)
    end
end

local function create_data_hash(data_key, plugin, title, content)
    local created = false
    local changed = false
    local details_key = string.format("%s;details", data_key)
    local dtype = redis.call("TYPE", details_key)["ok"]

    if dtype == "none" then
        created = true
    elseif dtype ~= "hash" then
        redis.call("DEL", data_key)
        changed = true
    end

    local details_key = string.format("%s;details", data_key)
    local old_details = list_to_map(redis.call("HGETALL", details_key))
    local new_details = {
        type = "hash",
        plugin = plugin,
        title = title,
    }

    if
        not (
            old_details["type"] == new_details["type"]
            and old_details["plugin"] == new_details["plugin"]
            and old_details["title"] == new_details["title"]
        )
    then
        redis.call("HSET", details_key, unpack(map_to_list(new_details)))
        changed = true
    end

    local index = 1
    local order = {}
    local old_vals = list_to_map(redis.call("HGETALL", data_key))

    local data_changed = false
    for key, val in pairs(content) do
        order[index] = key
        index = index + 1

        if old_vals[key] ~= val then
            data_changed = true
        end
    end

    local order_key = string.format("%s;order", data_key)
    if not cmp_lists(order, redis.call("LRANGE", order_key, 0, -1)) then
        data_changed = true
    end

    -- TODO add size diffing

    if data_changed == true then
        redis.call("DEL", data_key)
        redis.call("HSET", data_key, unpack(map_to_list(content)))

        redis.call("DEL", order_key)
        redis.call("RPUSH", order_key, unpack(order))

        changed = true
    end

    if created == true then
        create_change("created data", data_key, plugin)
    elseif changed == true and created == false then
        create_change("updated data", data_key, plugin)
    end
end

local function create_data_list(data_key, plugin, title, content)
    local names_key = string.format("%s;names", data_key)
    local titles_key = string.format("%s;titles", data_key)

    local created = false
    local changed = false

    if #content == 0 then
        return
    end

    for _, key in ipairs({ names_key, titles_key, data_key }) do
        local dtype = redis.call("TYPE", key)["ok"]
        if dtype == "none" then
            created = true
        elseif dtype ~= "list" then
            redis.call("DEL", key)
            changed = true
        end
    end

    local old_details = list_to_map(redis.call("HGETALL", details_key))
    local new_details = {
        type = "list",
        plugin = plugin,
        title = title,
    }

    if
        not (
            old_details["type"] == new_details["type"]
            and old_details["plugin"] == new_details["plugin"]
            and old_details["title"] == new_details["title"]
        )
    then
        redis.call("HSET", details_key, unpack(map_to_list(new_details)))
        changed = true
    end

    local proplist = {
        [1] = {},
        [2] = {},
        [3] = {},
    }
    for i, item in ipairs(content) do
        local target = proplist[((i - 1) % 3) + 1]
        target[#target + 1] = item
    end

    local names = redis.call("LRANGE", names_key, 0, -1)
    local titles = redis.call("LRANGE", titles_key, 0, -1)
    local values = redis.call("LRANGE", data_key, 0, -1)
    if not (cmp_lists(proplist[1], names) and cmp_lists(proplist[2], titles) and cmp_lists(proplist[3], values)) then
        redis.call("DEL", names_key, titles_key, data_key)
        redis.call("RPUSH", names_key, unpack(proplist[1]))
        redis.call("RPUSH", titles_key, unpack(proplist[2]))
        redis.call("RPUSH", data_key, unpack(proplist[3]))
        changed = true
    end

    if created == true then
        create_change("created data", data_key, plugin)
    elseif changed == true and created == false then
        create_change("updated data", data_key, plugin)
    end
end

local function create_data_table(data_key, plugin, title, columns, content)
    local created = false
    local changed = false
    local details_key = string.format("%s;details", data_key)
    local dtype = redis.call("TYPE", details_key)["ok"]

    if dtype == "none" then
        created = true
    elseif dtype ~= "hash" then
        redis.call("DEL", data_key)
        changed = true
    end

    local details_key = string.format("%s;details", data_key)
    local old_details = list_to_map(redis.call("HGETALL", details_key))
    local new_details = {
        type = "table",
        plugin = plugin,
        title = title,
        columns = columns,
    }

    if
        not (
            old_details["type"] == new_details["type"]
            and old_details["plugin"] == new_details["plugin"]
            and old_details["title"] == new_details["title"]
            and old_details["columns"] == new_details["columns"]
        )
    then
        redis.call("HSET", details_key, unpack(map_to_list(new_details)))
        changed = true
    end

    if not cmp_lists(content, redis.call("LRANGE", data_key, 0, -1)) then
        redis.call("DEL", data_key)
        redis.call("RPUSH", data_key, unpack(content))
        changed = true
    end

    if created == true then
        create_change("created data", data_key, plugin)
    elseif changed == true and created == false then
        create_change("updated data", data_key, plugin)
    end
end

local function create_data(data_key, plugin, dtype, args)
    if dtype == "list" then
        local title = table.remove(args, 1)
        create_data_list(data_key, plugin, title, args)
    elseif dtype == "hash" then
        local title = table.remove(args, 1)
        create_data_hash(data_key, plugin, title, list_to_map(args))
    elseif dtype == "string" then
        local title = table.remove(args, 1)
        local content_type = table.remove(args, 1)
        local content = table.remove(args, 1)
        create_data_str(data_key, plugin, title, content_type, content)
    elseif dtype == "table" then
        local title = table.remove(args, 1)
        local columns = table.remove(args, 1)
        create_data_table(data_key, plugin, title, columns, args)
    end
end

--- PLUGIN DATA

local PLUGIN_DATA_KEY = "pdata"

local function create_plugin_data(obj_key, args)
    local plugin = table.remove(args, 1)
    local dtype = table.remove(args, 1)
    local pdata_id = table.remove(args, 1)

    local pdata_key = string.format("%s;%s", PLUGIN_DATA_KEY, obj_key)
    redis.call("SADD", pdata_key, pdata_id)

    local data_key = string.format("%s;%s", pdata_key, pdata_id)
    create_data(data_key, plugin, dtype, args)
end

local function create_dns_plugin_data(names, args)
    local qname = qualify_dns_name(names[1])
    local plugin = args[1]

    create_dns({ qname }, { plugin })
    return create_plugin_data(string.format("%s;%s", DNS_KEY, qname), args)
end

local function create_node_plugin_data(names, args)
    local qnames = qualify_dns_names(names)
    local plugin = args[1]

    local node_id = dns_names_to_node_id(qnames)
    if redis.call("SISMEMBER", NODES_KEY, node_id) == 0 then
        create_node(qnames, { plugin })
    end

    return create_plugin_data(string.format("%s;%s", NODES_KEY, node_id), args)
end

--- REPORTS

local REPORTS_KEY = "reports"

local function create_report(_id, args)
    local id = _id[1]

    local changed = false
    if redis.call("SADD", REPORTS_KEY, id) ~= 0 then
        changed = true
    end

    local data_key = string.format("%s;%s", REPORTS_KEY, id)
    local plugin = table.remove(args, 1)
    local title = table.remove(args, 1)
    local length = table.remove(args, 1)

    local old_details = list_to_map(redis.call("HGETALL", data_key))
    local new_details = {
        plugin = plugin,
        title = title,
        length = length,
    }

    if
        not (
            old_details["plugin"] == new_details["plugin"]
            and old_details["title"] == new_details["title"]
            and old_details["length"] == new_details["length"]
        )
    then
        redis.call("HSET", data_key, unpack(map_to_list(new_details)))
        changed = true
    end

    if changed == true then
        create_change("create report", id, plugin)
    end
end

local function create_report_data(_id, args)
    local id = _id[1]
    local plugin = table.remove(args, 1)
    local index = table.remove(args, 1)
    local dtype = table.remove(args, 1)
    local data_key = string.format("%s;%s;%s", REPORTS_KEY, id, index)
    create_data(data_key, plugin, dtype, args)
end

--- INITIALISATION

local function init(keys, args)
    local default_network = keys[1]
    redis.call("DEL", DEFAULT_NETWORK_KEY)
    redis.call("SET", DEFAULT_NETWORK_KEY, default_network)

    redis.call("DEL", DNS_IGNORE_KEY)
    if #args ~= 0 then
        redis.call("SADD", DNS_IGNORE_KEY, unpack(args))
    end

    redis.call("DEL", CHANGELOG_KEY)
    create_change("init", default_network, "netdox")
end

--- FUNCTION REGISTRATION

redis.register_function("netdox_create_dns", create_dns)
redis.register_function("netdox_map_dns", map_dns)

redis.register_function("netdox_create_node", create_node)

redis.register_function("netdox_create_dns_metadata", create_dns_metadata)
redis.register_function("netdox_create_node_metadata", create_node_metadata)

redis.register_function("netdox_create_dns_plugin_data", create_dns_plugin_data)
redis.register_function("netdox_create_node_plugin_data", create_node_plugin_data)

redis.register_function("netdox_create_report", create_report)
redis.register_function("netdox_create_report_data", create_report_data)

redis.register_function("netdox_init", init)

-- TODO add input sanitization
