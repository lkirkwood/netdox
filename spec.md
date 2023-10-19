# Data Structure Specification

The data structures in Netdox are primarily represented using redis types.
Because of this, every item must provide a redis key "format" which describes how you would build the key that data structure is stored under in the redis server.

# Changes

## Changelog
+ Key: `changelog`
+ Type: `stream`
+ DB: 0
+ Notes: This lists all changes made to the data layer. Possible changes are documented below.

### Changelog Change Types
+ create dns name
+ add plugin to dns name
+ add record type to dns name
+ create dns record
+ updated network mapping
+ create plugin node
+ updated metadata
+ updated plugin data list
+ updated plugin data map
+ updated plugin data string

## Last Modified Time
+ Key: `last-modified`
+ Type: `hash`
+ DB: 0
+ Notes: Keys in this hash are any key in the data layer. The value is a ISO8601 UTC datetime - the last date/time that key was modified.

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
+ Notes: Values in this set are unresolved node IDs — a sorted set of DNS names claimed by the node, postfixed with the plugin name.

## Plugins providing a node with the given ID 
+ Key: `nodes;${NODE_ID}`
+ Type: `set`
+ DB: 0
+ Notes: All values are names of plugins that provided a node with this ID.

## Details of a node with a given ID from a given plugin
+ Key: `nodes;${NODE_ID};${PLUGIN_NAME}`
+ Type: `hash`
+ DB: 0
+ Notes: Keys in this hash are `name` (string), `plugin` (string), `exclusive` (bool), `link_id` (string). `${PLUGIN_NAME}` is a value from the set defined above.

## Set of all processed nodes
+ Key: `nodes`
+ Type: `set`
+ DB: 1
+ Notes: Values in this set are processed node IDs — also known as link IDs.

## Name of a processed node with a given ID
+ Key: `nodes;${NODE_ID}`
+ Type: `string`
+ DB: 1

## Alternative names for a processed node
+ Key: `nodes;${NODE_ID};alt_names`
+ Type: `set`
+ DB: 1

## DNS names for a processed node
+ Key: `nodes;${NODE_ID};dns_names`
+ Type: `set`
+ DB: 1

## Plugins for a processed node
+ Key: `nodes;${NODE_ID};plugins`
+ Type: `set`
+ DB: 1

## Keys of raw nodes used to build a processed node
+ Key: `nodes;${NODE_ID};raw_keys`
+ Type: `set`
+ DB: 1
+ Notes: All keys in this set exist only in DB 0.

## Key of node that each DNS name resolves to
+ Key: `dns_nodes`
+ Type: `hash`
+ DB: 1
+ Notes: Keys are DNS qnames. Values are processed node keys.

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

# Reports

Plugins may generate "reports", which are external documents built using the plugin datatypes and stored in redis. They may be linked with the regular DNS and Node documents or stand alone.
All reports must have their ID declared in the `reports` key in DB 0. The report then lives at `reports;${REPORT_ID}` and has the same format as plugin data.

# Plugin Data

Plugin data is implemented using the following data types. Each data type has an additional associated `${PDATA_KEY};details` key which stores a hash describing the data and how it should be rendered. The *Details* subheading contains the keys and a description of the expected values for this hash.


## Hash
Some key-value data about the object.

### Details
+ type: Always `hash`.
+ plugin: Plugin that provided this data.
+ title: A title to display for this plugin data hash.

## List
A list of values with a specific meaning.

### Details
+ type: Always `list`.
+ plugin: Plugin that provided this data.
+ list_title: A title for this whole list.
+ item_title: A title to display next to each item.

## String
A simple string.

### Details
+ type: Always `string`.
+ plugin: Plugin that provided this data.
+ title: A title for this string.
+ content_type: One of `html-markup`, `markdown`, or `plain`.

## Links

Links in plugin data look like `(!(${LINK_TYPE}|!|${LINK_ID})!)`, where `${LINK_TYPE}` is one of `report`, `dns`, `node` and `${LINK_ID}` is the ID of the target object. All text of this form in any plugin data or report will be converted to a link by the output driver. Invalid links will not be handled differently by netdox.

## Plugin Data for a DNS object
+ Key: `pdata;dns;${OBJECT_ID}`
+ Type: `set`
+ DB: 0
+ Notes: Set of IDs for plugin data added to this object.

## Plugin Data for a Node object
+ Key: `pdata;node;${OBJECT_ID}`
+ Type: `hash`
+ DB: 0
+ Notes: Same as for DNS, but in this case Object ID will be a raw node ID.
