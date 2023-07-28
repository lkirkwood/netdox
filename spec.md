# Data Structure Specification

The data structures in Netdox are primarily represented using redis types.
Because of this, every item must provide a redis key "format" which describes how you would build the key that data structure is stored under in the redis server.

# DNS

## Default Network Name
+ Key: `default_network`
+ Type: `string`
+ DB: 0
+ Notes: Must be created by client prior to running plugins.

## Set of all DNS names
+ Key: `dns`
+ Type: `set`
+ DB: 0
+ Notes: All values in this set are qualified with a network, like: `[some-net]domain.com`.

## Set of plugins that provided a DNS name
+ Key: `dns;${DNS_NAME};plugins`
+ Type: `set`
+ DB: 0

## Set of DNS record types for a given DNS name and source plugin
+ Key: `dns;${DNS_NAME};${PLUGIN_NAME}`
+ Type: `set`
+ DB: 0
+ Notes: Values in this set are uppercase DNS record type names.

## Set of DNS record values for a given DNS name, source plugin, and record type
+ Key: `dns;${DNS_NAME};${PLUGIN_NAME};${RECORD_TYPE}`
+ Type: `set`
+ DB: 0
+ Notes: For record types `CNAME`, `A`, `PTR`, the values in this set are qualified with a network.

## Set of network mappings for a given DNS name
+ Key: `dns;${DNS_NAME};maps`
+ Type: `set`
+ DB: 0
+ Notes: All values in this set are qualified with a network.

# Nodes

## Set of all nodes
+ Key: `nodes`
+ Type: `set`
+ DB: 0
+ Notes: Values in this set are unresolved node IDs — a sorted set of DNS names claimed by the node.

## Number of nodes with the a given ID 
+ Key: `nodes;${NODE_ID}`
+ Type: `integer`
+ DB: 0
+ Notes: Used to disambiguate multiple nodes that have the same set of DNS names. Minimum value of 1.

## Details of a node with a given ID
+ Key: `nodes;${NODE_ID};${INDEX}`
+ Type: `hash`
+ DB: 0
+ Notes: Keys in this hash are `name` (string), `plugin` (string), `exclusive` (bool), `link_id` (string). `${INDEX}` is the index in the range from 1 to the number from the key above.

## Set of all processed nodes
+ Key: `nodes`
+ Type: `set`
+ DB: 1
+ Notes: Values in this set are processed node IDs — also known as link IDs.

## Name of a processed node with a given ID
+ Key: `nodes;${NODE_ID}`
+ Type: `string`
+ DB: 1

# Metadata

## Set of all objects that have metadata associated
+ Key: `meta`
+ Type: `set`
+ DB: 0
+ Notes: Values in this set can be unresolved node IDs or qualified DNS names.

## Metadata for an object
+ Key: `meta;${OBJECT_ID}`
+ Type: `hash`
+ DB: 0
+ Notes: This hash has any keys. Object ID is the same as defined above.

# Plugin Data

