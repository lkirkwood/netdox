# Data Structure Specification

The data structures in Netdox are primarily represented using redis types.
Because of this, every item must provide a redis key "format" which describes how you would build the key that data structure is stored under in the redis server.

# Changes

## Changelog
+ Key: `changelog`
+ Type: `stream`
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
+ Notes: Keys in this hash are any key in the data layer. The value is a ISO8601 UTC datetime - the last date/time that key was modified.

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

## Node ID
The ID of a raw node is defined as:
+ The fully qualified DNS names claimed by that node, separated by ";".
+ The plugin name for the node, appened to the end with another separating ";" before it.

## Set of all nodes
+ Key: `nodes`
+ Type: `set`
+ Notes: Values in this set are raw node IDs, defined above.

## Details of a node with a given ID from a given plugin
+ Key: `nodes;${NODE_ID}`
+ Type: `hash`
+ Notes: Keys in this hash are `name` (string), `exclusive` (bool), `link_id` (string).

## Set of all processed nodes
+ Key: `proc_nodes`
+ Type: `set`
+ Notes: Values in this set are processed node IDs — also known as link IDs.

## Name of a processed node with a given ID
+ Key: `proc_nodes;${LINK_ID}`
+ Type: `string`

## Alternative names for a processed node
+ Key: `proc_nodes;${LINK_ID};alt_names`
+ Type: `set`

## DNS names for a processed node
+ Key: `proc_nodes;${LINK_ID};dns_names`
+ Type: `set`

## Plugins for a processed node
+ Key: `proc_nodes;${LINK_ID};plugins`
+ Type: `set`

## Keys of raw nodes used to build a processed node
+ Key: `proc_nodes;${LINK_ID};raw_keys`
+ Type: `set`

## Key of node that each DNS name resolves to
+ Key: `dns_nodes`
+ Type: `hash`
+ Notes: Keys are DNS qnames. Values are processed node keys.

## Key of processed node that each raw node was absorbed into
+ Key: `proc_node_revs`
+ Type: `hash`
+ Notes: Keys in the hash are raw node IDs (defined above). Values are link IDs of processed nodes.

# Metadata

## Set of all objects that have metadata associated
+ Key: `meta`
+ Type: `set`
+ Notes: Values in this set can be unresolved node IDs or qualified DNS names.

## Metadata for an object
+ Key: `meta;${OBJECT_ID}`
+ Type: `hash`
+ Notes: This hash has any keys. Object ID is the same as defined above.

# Reports

Plugins may generate "reports", which are external documents built using the plugin datatypes and stored in redis. They may be linked with the regular DNS and Node documents or stand alone.
All reports must have their ID declared in the `reports` key. The report then lives at `reports;${REPORT_ID}` and has the same format as plugin data.

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
+ Notes: Set of IDs for plugin data added to this object.

## Plugin Data for a Node object
+ Key: `pdata;node;${OBJECT_ID}`
+ Type: `hash`
+ Notes: Same as for DNS, but in this case Object ID will be a raw node ID.
