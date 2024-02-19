# Data Structure Specification

The data structures in Netdox are primarily represented using redis types.
Because of this, every item must provide a redis key "format" which describes how you would build the key that data structure is stored under in the redis server.

# Changes

## Changelog
+ Key: `changelog`
+ Type: `stream`
+ Notes: This lists all changes made to the data in a stream. Each entry has the fields `change`, `value`, and `plugin`.

### Changelog Change Types and Values
The list below maps the `change` field to a description of the `value` field.
+ init: The new default network.
+ create dns name: Qualified DNS name.
+ create dns record: Full redis key of the dns record set with ";${RECORD_VALUE}" appended.
+ create plugin node: ID of the raw node.
+ updated metadata: Full redis key of the updated metadata.
+ created data: Full redis key of the created data.
+ updated data: Full redis key of the updated data.
+ create report: ID of the created report.
+ updated network mapping: unimplemented.

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
The ID of a raw node is defined as the qualified DNS names claimed by that node, sorted alphabetically and separated by ";".

## Set of all nodes
+ Key: `nodes`
+ Type: `set`
+ Notes: Values in this set are raw node IDs, defined above.

## Number of nodes with a given ID
+ Key: `nodes;${NODE_ID}`
+ Type: `int`

## Details of node with given ID and index
+ Key: `nodes;${NODE_ID};${INDEX}`
+ Type: `hash`
+ Notes: Keys in this hash are `plugin` (string), `name` (string), `exclusive` (bool), `link_id` (string). Indices start at 0.

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
+ Key: `proc_nodes;${LINK_ID};raw_ids`
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
+ Notes: This hash has any keys. Object ID is full redis key of object — DNS name or Node.

## Plugins contributing to metadata for an object
+ Key: `meta;${OBJECT_ID};plugins`
+ Type: `set`

# Data

Plugin data is an unordered set of data attached to a DNS name or Node.
Reports are standalone documents containing an ordered list of data.
Both use a common set of data types. These are `hash`, `list`, `string` and `table`.

Any given piece of data at `$DATA_KEY` will have a hash of details at `${DATA_KEY};details` containing the following fields:
+ `plugin` — Name of the plugin that provided this data.
+ `type` — The type of data; one of the data types listed above (`hash` etc.)

Each data type has unique additional attributes that allow you to configure how they should be displayed.

## Hash
The `hash` data type is a mapping of unique keys to values. Order of insertion is preserved.
It has the following additional fields in its details.

+ `title` — A title for the hash.

## List
The `list` data type is a list of 3-tuples; an index name, a display title, and a value.
It has the following additional fields in its details.

+ `title` — A title for the whole list.

Items in a list have an index name, a display title, and a value. Arguments should be passed in that order.

## String
The `string` data type has the following additional fields in its details.

+ `title` — A title for the string.
+ `content_type` — The type of content the string contains. One of `html-markup`, `markdown`, or `plain`.

## Table

The `table` data type has the following additional fields in its details.

+ `title` — A title for the table.
+ `columns` — Number of columns in each row.

## Links

Links in plugin data look like `(!(${LINK_TYPE}|!|${LINK_ID})!)`, where `${LINK_TYPE}` is one of `report`, `dns`, `rawnode`, `procnode` and `${LINK_ID}` is the ID of the target object. All text of this form in any data will be converted to a link by the output driver. Invalid links will not be handled differently by netdox.

### Note on support

Currently in "map" plugin data types, the entire string of the value must be taken up by the link. Otherwise, the text will be rendered as-is.

# Reports

## Set of all report IDs
+ Key: `reports`
+ Type: `set`

## Report details
+ Key: `reports;${REPORT_ID}`
+ Type: `hash`
+ Notes: Keys in this hash are `title`, `plugin`, and `length`.

## Report data 
+ Key: `reports;${REPORT_ID};${INDEX}`
+ Type: depends upon the data type specified in the details (see below)
+ Notes: `$INDEX` is the position of this data in the report. Must be less than `length` defined in report details above (indices start at 0)

## Report data details
+ Key: `reports;${REPORT_ID};${INDEX};details`
+ Type: `hash`
+ Notes: Keys in this hash are `plugin`, `type` + other attributes (see data section above)

# Plugin Data

## DNS name plugin data IDs
+ Key: `pdata;dns;${OBJECT_ID}`
+ Type: `set`
+ Notes: Set of IDs for plugin data added to this object.

## DNS name plugin data content
+ Key: `pdata;dns;${OBJECT_ID};${PDATA_ID}`
+ Type: depends upon the data type specified in the details (see below)

## DNS name plugin data details
+ Key: `pdata;dns;${OBJECT_ID};${PDATA_ID};details`
+ Type: `hash`
+ Notes: Keys in this hash are `plugin`, `type` + other attributes (see data section above)

## Node plugin data IDs
+ Key: `pdata;node;${OBJECT_ID}`
+ Type: `set`
+ Notes: Same as for DNS, but in this case Object ID will be a raw node ID.

## Node name plugin data content
+ Key: `pdata;node;${OBJECT_ID};${PDATA_ID}`
+ Type: depends upon the data type specified in the details (see below)

## Node name plugin data details
+ Key: `pdata;node;${OBJECT_ID};${PDATA_ID};details`
+ Type: `hash`
+ Notes: Keys in this hash are `plugin`, `type` + other attributes (see data section above)
