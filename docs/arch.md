# Process Flow
+ The local config is read, which defines the redis server to use, the plugins to call, which remote server to use, etc.
+ Plugins run, and put data in the redis server using custom functions provided by netdox. 
+ A process runs, merging nodes to consolidate data.
+ Extensions run, reading data (including the final set of nodes) and performing any function: configuring a monitoring tool, writing more data to redis, or anything else.
+ Output connectors publish documents to a remote server for display to the end user.
  
![Netdox/Redis Architecture](netdox-redis-arch.drawio.svg)

# Extending Netdox
Netdox is designed to be very extensible. To this end, all creation of data is done by external executables (plugins) that are merely referred to in a configuration file — there is no need to modify netdox itself to write your own plugin. Just provide the path of an executable and any arguments it requires like API keys. 

The output connectors that read data and generate documents are separate from the core. Writing a new output connector involves implementing a trait, and then adding your implementation to an enum. This should make it relatively straightforward to write a new one but you are tied to Rust and this repository. In the future, I hope to implement this in much the same way plugins work.

# Redis
Netdox uses redis as a bucket in which plugins may easily dump unstructured data. Then netdox consolidates data that pertains to the same node around a stable identifier.
The ID must be stable (not change) for the life of the object so it can be used for document linking and history.

This "link ID" must be provided by one of the plugins — preferably the one which knows the most about it and can therefore choose the best ID, e.g. a hypervisor. Remember, if the ID changes a separate node will be created.

## Soft Nodes

A soft node is a node with no link id. Nodes that do have a link ID are called linkable nodes or just nodes.

Soft nodes are a container — their data is not displayed unless they merge with a linkable node (how would you link to the document?)

## Supersets

Now the problem is that without knowledge of the plugin that created the node, no other plugin can predict the ID. 
So, if the plugin is not setting a link ID it must specify the correct node using some other parameter that uniquely identifies the node. For this we use a "superset" of the DNS names that the plugin knows resolve to that node (Often this is just the IP of the node)

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

This set is the ID for all unprocessed nodes.
The consolidation process merges soft nodes with the linkable node that has the smallest matching superset to create processed nodes.

## Exclusive Nodes

The exclusive field on a node indicates that the plugin that created it is *sure* that no other DNS names should be added to the superset.

So far we've seen that nodes which appear to occupy the same location on the network are combined. For example, all of the data for the machine which serves the `data.domain.com` and `webapp.domain.com` webpages would be in one node if both domains resolve to the same IP address.

This model works for the most part.  However, it is not rare for one node to act as a proxy for other nodes. In this situation, all DNS names resolve to the proxy node, and further forwarding is done at an application level unbeknown to netdox.

The superset logic above could merge the proxy node with the soft nodes carrying the data of the other nodes that it forwards to.

![Diagram illustrating the need for the exclusive parameter](docs/exclusive.svg)

On the other hand, what about a plugin that provides information about Kubernetes pods. This plugin knows better than DNS which domain names will resolve to it. The plugin should mark this node exclusive, and its ID will instead be only the DNS names it was created with. That way, even if its superset matches another node, the two can be distinguished.

This node can still merge with soft nodes with supersets that are in its ID.
