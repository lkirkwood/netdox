# Plugins
A core principle of Netdox is that you don't have to modify the core in order to get data from a new place. Plugins can be any executable, and are listed in an encrypted config file (more on this later) so that Netdox knows where to find them and what arguments to provide; API keys, passwords, URLs, whatever. These plugins only communicate with a redis server, and write data using special commands so that it enters the database in a format that the core can understand and use. This means that any language which has a redis client should be relatively easy to use for writing a new plugin. There is already a dedicated Netdox plugin library for both [Python](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-py) and [Rust](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-rs) which basically just wraps these redis commands in a nicer interface. All the plugins I have written so far live [here](https://gitlab.allette.com.au/allette/netdox/netdox-redis-plugins).

Plugins are separated into a few stages. Plugins in the same stage run in *parallel*, but the stages are executed sequentially and in order. The stages in order of execution, are:
1. Write-only
2. Read-write
3. Connectors

If *all* the plugins ran in parallel, then in a situation where one plugin wants to read data that another plugin will eventually write, that plugin could never be sure the data would be present or up to date. This is why the stages are necessary. The write-only plugins are the most obvious; they don't need to read any data created by other plugins, they just place data into redis. The main ones here are DNS plugins or things like Kubernetes that just provide data. 
Read-write plugins are those that need to read some data in order to create their own. Usually these plugins will create a report (some non-DNS, non-node document) based off of information they acquire by querying redis. These plugins will run after the processing step (explained a bit more below), so they can ask which nodes have been created and which DNS names are associated with a node etc. Examples of read-write plugins include TrueNAS, Icinga, and ZAP. 
Connectors are plugins that connect two other plugins. Theres only one of these so far, a XenOrchestra-TrueNAS link plugin that matches VMs to their backup destination disks (plus some other small tasks).

## The Config File
The config file is a TOML file which is mounted into the final container. It's encrypted by the process the first time it's used. Broadly, it looks like this:
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

# Redis
Netdox uses redis as a bucket in which plugins may easily dump data. Netdox then consolidates data that pertains to the same node around a stable identifier.
The ID must be stable (not change) for the life of the object so it can be used for document linking and history.

This "link ID" must be provided by one of the plugins — preferably the one which knows the most about it and can therefore choose the best ID, e.g. a hypervisor. Remember, if the ID changes a separate node will be created.

# Nodes

**Heads up: you probably don't need to read this section.**

There are two categories of nodes: raw and processed.
There are multiple types of raw node which are discussed below. These are nodes created by plugins. Processed nodes are exclusively created by netdox and are the result of merging raw nodes — again this is covered below. 

## Soft Nodes

A soft node is a raw node with no link id. Raw nodes that *do* have a link ID are called linkable nodes or just nodes.

Soft nodes are a container — their data is not displayed unless they merge with a linkable node (how would you link to the document?)

## Supersets

Now the problem is that without knowledge of the plugin that created the node, no other plugin can predict the ID. 
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
The consolidation process merges soft nodes with the linkable node that has the smallest matching superset to create processed nodes.

## Exclusive Nodes

The exclusive field on a raw node indicates that the plugin that created it is *sure* that no other DNS names should be added to the superset.

So far we've seen that nodes which appear to occupy the same location on the network are combined. For example, all of the data for the machine which serves the `data.domain.com` and `webapp.domain.com` webpages would be in one node if both domains resolve to the same IP address.

This model works for the most part.  However, it is not rare for one node to act as a proxy for other nodes. In this situation, all DNS names resolve to the proxy node, and further forwarding is done at an application level unbeknown to netdox.

The superset logic above could merge the proxy node with the soft nodes carrying the data of the other nodes that it forwards to.

![Diagram illustrating the need for the exclusive parameter](/docs/exclusive.svg)

On the other hand, what about a plugin that provides information about Kubernetes pods. This plugin knows better than DNS which domain names will resolve to it. The plugin should mark this node exclusive, and its ID will instead be only the DNS names it was created with. That way, even if its superset matches another node, the two can be distinguished.

This node can still merge with other raw nodes if their superset is a *subset* of its own.
