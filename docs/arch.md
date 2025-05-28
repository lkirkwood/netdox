# Plugins
A core principle of Netdox is that you don't have to modify the core in order to get data from a new place. Plugins can be any executable, and are listed in an encrypted config file (more on this later) so that Netdox knows where to find them and what arguments to provide; API keys, passwords, URLs, whatever. These plugins only communicate with a redis server, and write data using special commands so that it enters the database in a format that the core can understand and use. This means that any language which has a redis client should be relatively easy to use for writing a new plugin. There is already a dedicated Netdox plugin library for both [Python](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-py) and [Rust](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-rs) which basically just wraps these redis commands in a nicer interface. All the plugins I have written so far live [here](https://gitlab.allette.com.au/allette/netdox/netdox-redis-plugins).

Plugins are separated into a few stages. Plugins in the same stage run in *parallel*, but the stages are executed sequentially and in order. The stages in order of execution, are:
1. Write-only
2. Read-write
3. Connectors

If *all* the plugins ran in parallel, then in a situation where one plugin wants to read data that another plugin will eventually write, that plugin could never be sure the data would be present or up to date. This is why the stages are necessary. The write-only plugins are the most obvious; they don't need to read any data created by other plugins, they just place data into redis. The main ones here are DNS plugins or things like Kubernetes that just provide data. 
Read-write plugins are those that need to read some data in order to create their own. Usually these plugins will create a report (some non-DNS, non-node document) based off of information they acquire by querying redis. These plugins will run after the processing step (explained a bit more below), so they can ask which nodes have been created and which DNS names are associated with a node etc. Examples of read-write plugins include TrueNAS, Icinga, and ZAP. 
Connectors are plugins that connect two other plugins. There's only one of these so far, a XenOrchestra-TrueNAS link plugin that matches VMs to their backup destination disks (plus some other small tasks).

# The Config File
The config file is a TOML file which has to be accessible to the Netdox process when it runs. It should first be loaded, encrypted, and stored in a separate location before it's used because it's where you store all the sensitive plugin configuration like API keys. There's a Netdox command for doing this: `netdox config load`. The file looks like this:
```toml
default_network = "allette"
dns_ignore = []

[redis]
host = "localhost"
port = 9999
db = 0

[remote.pageseeder]
url = "https://ps-netdox-dev.allette.com.au"
client_id = "<client_id>"
client_secret = "<client_secret>"
username = "<ps_username>"
group = "netdox-network"
upload_dir = "documents"

[[plugin]]
name = "made-up-plugin"
stages.write-only.path = "/path/to/plugins/idontexist/plugin.py"
api = "<api key>"
secret = "<secret key>"
```
The default network is just a namespace for DNS names. Plugins can create DNS names like normal, say `example.com`, and internally they will become `[default_network]example.com`. Plugins can also specify this upfron, by creating `[internal]example.com`, which will allow the two to be distinguished. This works for IPs too, and allows Netdox to model internal networks like Kubernetes if necessary. Honestly this has mostly proven unnecesssary, and all of the existing plugins just create normal DNS names and allow the default network to be applied.

Everything else should be mostly self explanatory, except the plugin config. For those unfamiliar, in TOML the `[[key]]` syntax creates a dictionary inside a list called `key`. In JSON, it would look like:
```json
{
    "plugin": [
        {
            "name": "made-up-plugin",
            "stages": {
                "write-only": {
                    "path": "/path/to/plugins/idontexist/plugin.py"
                }
            },
            "api": "<api key>",
            "secret": "<secret key>"
        }
    ]
}
```

The `name` and `stages` keys are required. Everything else gets passed to the plugin as a TOML string when it runs. In fact, you can store more complex data structures here as well:
```toml
[[plugin]]
name = "made-up-plugin"
stages.write-only.path = "/path/to/plugins/idontexist/plugin.py"

[[plugin.api-config]]
api = "<api key>"
secret = "<secret key>"
hosts = ["host1.com", "host2.com", "host3.com"]
```

With equivalent JSON:
```json
{
    "plugin": [
        {
            "name": "made-up-plugin",
            "stages": {
                "write-only": {
                    "path": "/path/to/plugins/idontexist/plugin.py"
                }
            },
            "api-config": {
                "api": "<api key>",
                "secret": "<secret key>",
                "hosts": [
                    "host1.com",
                    "host2.com",
                    "host3.com"
                ]
            }
        }
    ]
}
```

Stage-specific arguments can be provided for plugins with more than one stage — simply add some keys to the stages map like this:
```toml
[[plugin]]
name = "made-up-plugin"
api = "<api key>"
secret = "<secret key>"

[[plugin.stages.write-only]]
path = "/path/to/plugins/idontexist/writeonly.py"
write-only-arg = some-value

[[plugin.stages.read-write]]
path = "/path/to/plugins/idontexist/readwrite.py"
read-write-arg = other-value
```

The redis config will also be passed as a TOML string — the first argument to your plugin will be the redis config, then the plugin config we just went over. Any TOML parser should be able to reconstruct a datastructure from the strings. See any plugin in the repository linked above for an example.

# High-level Process Flow
+ The local config is read, which defines the redis server to use, the plugins to call, which remote server (a PageSeeder instance probably) to use, etc.
+ Plugins run, and put data in the redis server using custom functions provided by netdox. 
  + These custom functions are implemented in Lua and are loaded directly into the redis server. This means that plugins call them similarly to any other redis command, e.g. `set` or `del`.
+ A processing step runs, merging nodes to consolidate data. This matches DNS names to nodes and does a bunch of other things as well.
+ Some more plugins run, reading data (including the final set of nodes) and performing any function: configuring a monitoring tool, writing more data to redis, or anything else.
+ Output connectors publish documents to a remote server for display to the end user.

# Key Concepts
+ DNS names are domain names or IPv4 addresses. Internally these are prefixed with a logical network as indicated above in the config section, but you probably don't need to worry about this.
+ Nodes represent computers, servers, containers, etc. These are the most complicated part of netdox, so if you really need to know how they work they have a whole section below. Again, you probably don't need to worry about them too much. Basically, they have a name, a "Link ID" which is like a globally unique, immutable ID, and they also contain a list of DNS names.
+ Reports are separate documents that aren't anchored to either of the two above concepts. They have a fixed length, so they contain a fixed number of data (next bullet point), they have a fixed ID much like nodes, and they have a title.
+ All of these things above can contain "plugin data". This is data that lives in redis and is created when plugins call those special Lua functions mentioned above (more detail [here](docs/functions.md)). It has one of four data types, and the idea is that plugins create this data, attach it to a DNS name, a node, or a report, and then netdox will automatically publish it for you.
  + These data types are quite primitive, but have so far proven flexible enough for almost anything. They are "string", "list", "hash", and "table". 
  + Strings are self-explanatory, simply some text data.
  + Lists are property lists. Each element has a name, a title, and a value, exactly like PageSeeder properties. The name is a terse, indexable name for the data. The title is a nice, descriptive name that should be displayed to someone viewing the published data. The value is just the content, the actual element of the list. 
  + Hashes are map types, key/value pairs.
  + Tables are... tables. Basically a matrix, or a list of lists (not the lists above, normal lists containing strings only).
  
# Nodes

**Heads up: you probably don't need to read this section.**

There are two categories of nodes: raw and processed. When a plugin runs, and creates a node, this is a raw node. It hasn't been processed yet, and might be merged or altered when the processing step runs in between plugin stages.
There are multiple types of raw node which are discussed below. Processed nodes are exclusively created by netdox and are the result of merging raw nodes — again this is covered below. 

## Soft Nodes

A soft node is a raw node with no link id. Soft nodes are a container — their data is not displayed unless they merge with a linkable node (how would you link to the document?). Hopefully, when the processing step runs, Netdox will look at all the DNS records it knows about, combine that information with all the *raw* nodes that have been created (soft or not), and be able to match soft nodes to some linkable raw nodes. This way, the information will anchored to something with an ID that can be used to link to it.

The idea from a plugin developers perspective is this: say I scan every IP address that has been created in the database and I want to identify any web servers there. I discover there is an NGINX server at 192.168.0.42. I want to attach this information to the node that is running this web server, so that someone can see the operating system etc. alongside this new piece of data about NGINX. What I can do is create a node, and say that node has the DNS name 192.168.0.42 — but I (the plugin) *don't* presume to be the authoritative source of information about this server. I assume that some other plugin will come along and give this server a proper linkable ID. If this happens, then Netdox should be able to figure out that my NGINX server and this linkable node created by a plugin that *is* authoritative occupy the same location on the network, and when it runs the processing step the two will be combined.

## Supersets

As we've seen, the problem is that without knowledge of the plugin that created the node, no other plugin can predict the ID. 
So, if the plugin is creating data for a node but does not "own" the node, it must specify the target using some other parameter that uniquely identifies it. For this we use a "superset" of the DNS names attached to the node (Often this is just the IP of the node)

In netdox a superset is the largest set of DNS names reachable through DNS records (forwards *or* backwards). Take the following list of DNS records:
+ `domain.com -> 192.168.200.25`
+ `domain.net -> 192.168.200.81`
+ `alias.org -> domain.net`

The superset for `domain.com` is:
+ `domain.com`
+ `192.168.200.25`

But for `domain.net` its:
+ `domain.net`
+ `192.168.200.81`
+ `alias.org`

So, the superset for a node created with the DNS names `domain.com` and `domain.net`, is the combined set:
+ `domain.com`
+ `192.168.200.25`
+ `domain.net`
+ `192.168.200.81`
+ `alias.org`

This set is the ID for all raw nodes.
The consolidation process merges soft nodes with the linkable node that has the smallest matching superset to create processed nodes. The algorithm is actually more complicated than simple superset equality, but not much. This is basically how it works.

## Exclusive Nodes

**You really probably don't need to read this bit. Only proceed if you are struggling to get your nodes and DNS names to match up or are just morbidly curious.**

The exclusive field on a raw node indicates that the plugin that created it is *sure* that no other DNS names should be added to the superset.

So far we've seen that nodes which appear to occupy the same location on the network are combined. For example, all of the data for the machine which serves the `data.domain.com` and `webapp.domain.com` webpages would be in one node if both domains resolve to the same IP address.

This model works for the most part.  However, it is not rare for one node to act as a proxy for other nodes. In this situation, all DNS names resolve to the proxy node, and further forwarding is done at an application level unbeknown to netdox.

The superset logic above could merge the proxy node with the soft nodes carrying the data of the other nodes that it forwards to.

![Diagram illustrating the need for the exclusive parameter](/docs/exclusive.svg)

On the other hand, what about a plugin that provides information about Kubernetes pods. This plugin knows better than DNS which domain names will resolve to it. The plugin should mark this node exclusive, and its ID will instead be only the DNS names it was created with. That way, even if its superset matches another node, the two can be distinguished.

This node can still merge with other raw nodes if their superset is a *subset* of its own.
