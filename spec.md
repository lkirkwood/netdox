# Data Structure Specification

The data structures in Netdox are primarily represented using redis types.
Because of this, every item must provide a redis key "format" which describes how you would build the key that data structure is stored under in the redis server.

# DNS

## Default Network Name
+ Key: `default_network`
+ Type: `string`
+ Notes: Must be created by client prior to running plugins.

## Set of all DNS names
+ Key: `dns`
+ Type: `set`
+ Notes: All values in this set are qualified with a network, like: `[some-net]domain.com`.

## Set of plugins that provided a DNS name
+ Key: `dns;${DNS_NAME};plugins`
+ Type: `set`

## Set of DNS record types for a given DNS name and source plugin
+ Key: `dns;${DNS_NAME};${PLUGIN_NAME}`
+ Type: `set`
+ Notes: Values in this set are uppercase DNS record type names.

## Set of DNS record values for a given DNS name, source plugin, and record type
+ Key: `dns;${DNS_NAME};${PLUGIN_NAME};${RECORD_TYPE}`
+ Type: `set`
+ Notes: For record types `CNAME`, `A`, `PTR`, the values in this set are qualified with a network.

## Set of network mappings for a given DNS name
+ Key: `dns;${DNS_NAME};maps`
+ Type: `set`
+ Notes: All values in this set are qualified with a network.

# Nodes

## Set of all nodes
+ Key: `nodes`
+ Type: `set`
+ Notes: Values in this set are unresolved node IDs — a sorted set of DNS names claimed by the node.

## Set of all plugins that provided a node with a given ID
+ Key: `nodes;${NODE_ID};plugins`
+ Type: `set`

## Details of a node with a given ID from a given plugin
+ Key: `nodes;${NODE_ID};{$PLUGIN_NAME}`
+ Type: `hash`
+ Notes: Keys in this hash are `name` (string), `exclusive` (bool), `link_id` (string).

# Metadata

## Set of all objects that have metadata associated
+ Key: `meta`
+ Type: `set`
+ Notes: Values in this set can be unresolved node IDs or qualified DNS names.

## Metadata for an object
+ Key: `meta;${OBJECT_ID}`
+ Type: `hash`
+ Notes: This hash has any keys. Object ID is the same as defined above.

# Plugin Data


