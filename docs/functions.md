# Data Creation Functions
The Redis datastore exposes a number of functions for creating data that netdox can display.

## DNS

`netdox_create_dns` — Creates a DNS record.

**keys**: 1 key containing the DNS name to use as the label for the record.

**args**:
+ plugin — Name of the plugin creating the record.
+ rtype — Optional DNS record type. Creating a record of type `CNAME`, `A`, or `PTR` will add a network qualifier to the value if there is none. This can be empty if value is also empty.
+ value - The value of the DNS record. Can be empty if rtype is also empty.

---

`netdox_map_dns` — Maps a DNS name in one network to a DNS name in another network.

**keys**: 1 key containing the DNS name to use as the origin for the mapping.

**args**:
+ plugin — Name of the plugin creating the mapping.
+ reverse — true if you wish to create a reverse mapping from the destination to the origin.
+ values... — A sequence of qualified DNS names that you wish to map the origin to. Should all be in different networks from each other and the origin.

## Nodes

`netdox_create_node` — Creates a Node.

**keys**: 1 or more DNS names the node believes it owns.

**args**: 
+ plugin — Name of the plugin creating the node.
+ name — Name for the node.
+ exclusive — Optional boolean; true if the only data this node should display is that which is attached to a DNS name in **keys**. Default is false.
+ link_id — Optional link ID for the node. If not provided the node created will be a *soft node*.

## Metadata

`netdox_create_dns_metadata` — Creates some metadata attached to a DNS name.

**keys**: 1 key containing a DNS name to attach the metadata to.

**args**:

+ plugin — Name of the plugin creating the metadata.
+ (key, value)... — A sequence of key, value pairs that make up the metadata to create.

---

`netdox_create_node_metadata` — Creates some metadata attached to a Node.

**keys**: 1 or more DNS names making up the node ID (same as for `netdox_create_node`).

**args**:
+ plugin — Name of the plugin creating the metadata.
+ (key, value)... — A sequence of key, value pairs that make up the metadata to create.

## Plugin Data

`netdox_create_dns_plugin_data` — Creates some plugin data attached to a DNS name.

**keys**: 1 key containing a DNS name to attach the plugin data to.

**args**:
+ plugin — Name of the plugin creating the plugin data.
+ dtype — The type of data to create. One of `hash`, `list`, `string`, `table`.
+ pdata_id — An ID for the plugin data, unique with respect to other plugin data on the DNS name.
+ ... — Some more args decided by `dtype`.

**hash args**:
+ title — A title for the hash.
+ (key, value)... — A sequence of key, value pairs that make up the hash to create.

**list args**:
+ list_title — A title for the list.
+ item_title — A title for each item in the list.
+ values... — A sequence of values that make up the list to create.

**string args**:
+ title — A title for the string.
+ content_type — The type of content in the string, used by the remote to control how it should be displayed. One of `html-markup`, `markdown`, `plain`.
+ value — The string to create.

**table args**:
+ title — A title for the table.
+ columns — Number of columns in each row.
+ cells... — The value of the cells in the table.

---
`netdox_create_node_plugin_data` — Creates some plugin data attached to a Node.

**keys**: 1 or more DNS names making up the node ID (same as for `netdox_create_node`).

**args**:
+ plugin — Name of the plugin creating the plugin data.
+ dtype — The type of data to create. One of `hash`, `list`, `string`.
+ pdata_id — An ID for the plugin data, unique with respect to other plugin data on the DNS name.
+ ... — Some more args decided by `dtype`.

**hash args**:
+ title — A title for the hash.
+ (key, value)... — A sequence of key, value pairs that make up the hash to create.

**list args**:
+ list_title — A title for the list.
+ item_title — A title for each item in the list.
+ values... — A sequence of values that make up the list to create.

**string args**:
+ title — A title for the string.
+ content_type — The type of content in the string, used by the remote to control how it should be displayed. One of `html-markup`, `markdown`, `plain`.
+ value — The string to create.

**table args**:
+ title — A title for the table.
+ columns — Number of columns in each row.
+ cells... — The value of the cells in the table.

## Reports

`netdox_create_report` — Creates a report.

**keys**: 1 key containing a unique ID for the report.

**args**:
+ plugin — Name of the plugin creating the report.
+ title — Title for the report.
+ length — Number of items in the report.

---

`netdox_create_report_data` — Creates a piece of data in a report.

**keys**: 1 key containing the ID of the report.

**args**:
+ plugin — Name of the plugin creating the report data.
+ index — Position in the report, starting at 0. Must not exceed the length set when creating the report.
+ dtype — The type of data to create. One of `hash`, `list`, `string`.
+ ... — Some more args decided by `dtype`.

**hash args**:
+ title — A title for the hash.
+ (key, value)... — A sequence of key, value pairs that make up the hash to create.

**list args**:
+ list_title — A title for the list.
+ item_title — A title for each item in the list.
+ values... — A sequence of values that make up the list to create.

**string args**:
+ title — A title for the string.
+ content_type — The type of content in the string, used by the remote to control how it should be displayed. One of `html-markup`, `markdown`, `plain`.
+ value — The string to create.

**table args**:
+ title — A title for the table.
+ columns — Number of columns in each row.
+ cells... — The value of the cells in the table.

