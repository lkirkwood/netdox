# For Users
## Config

The config file is loaded from a user-provided path during initialization. The file is then encrypted and stored at `$NETDOX_CONFIG` if set, defaulting to `$HOME/.config/.netdox`.

The encryption key used is the value of `$NETDOX_SECRET`. This variable must be set in the environment in which netdox runs.

The config file can be managed using the `netdox config` subcommmand. When in doubt: `netdox -h`

# For Developers
## Testing

Running many of the tests requires a redis server. The url for this server should be available under the environment variable `NETDOX_TEST_REDIS_URL`. **WARNING**: Data in this server may be overwritten or destroyed while running tests!

# Architecture
In netdox, a node is a single machine or server. Each node is attached to one or more DNS names. Each node and each DNS name can have data attached to it in a few simple structures. This data is used to generate documents for display on a remote server.

## Extending Netdox
Netdox is designed to be very extensible. To this end, all creation of data is done by external executables (plugins) that are merely configured in a file — there is no need to modify netdox itself to get data from a new source. Simply package your code in any way you like and tell netdox where to find the executable, along with any parameters it might need.  

Furthermore, the output connectors (the code that reads data and generates documents) are separate from the core. Currently, writing a new output connector involves simply implementing a trait, and then adding your implementation to an enum. This should make it relatively simple for developers to write new output connectors — *but* you are tied to Rust. In the future, I hope to implement this in much the same way plugins work.

## Redis
Netdox uses redis as a bucket in which any application may dump data as long as it conforms to the expected specification. This is most easily done using the custom functions that can be called like normal redis functions — however there is nothing to stop you from creating data structures by hand. But... don't do that.

The functions for creating data are documented [here](docs/functions.md) (useful for those writing plugins) and the specification for the data created by those functions is documented [here](docs/data.md) (useful for those writing output connectors).

## Process Flow
+ The local config is read, which defines the redis server to use, the plugins to call, which remote server to use, etc.
+ Plugins run, and create or modify the data in the redis server using custom functions. In broad strokes this involves creating DNS records and nodes, and then attaching data to these objects.
+ Once the plugins finish running, nodes are consolidated so that all the data for any one node is available in one place.
+ Finally, output connectors render the changes on a remote server in any way they see fit.
  
![Netdox/Redis Architecture](netdox-redis-arch.drawio.svg)

# Implementation Overview

## IDs

Nodes use a plugin-provided string as their linkable ID. 
Plugins other than the one that created the node cannot predict this "link ID", so adding data to a node must use other methods for matching.
When adding data to a node, plugins must provide two additional pieces of information:
+ The DNS names that the plugin believes resolve to the desired node 
+ Whether those names are "exclusive"

These two parameters are used to consolidate data attached to nodes that appear to be the same.

## Network Address Translation

Previously, netdox has ignored the concept of separate networks. All addresses were considered local to "*the*" network.
In the new version, this has changed. All DNS names must be qualified by a network ID, in order to allow netdox to model separate networks. This includes virtual networks like those constructed by Kubernetes.
Those DNS names that are provided by plugins without a network qualifier will be qualified with the default network. This is configured before running netdox the first time.


### Supersets

If a plugin provides some information about a node, but does not manage said node, the plugin simply includes any relevant DNS names it knows about. It does *not* include a link ID.  It may be that this plugin has provided a unique set of DNS names to identify the node — in this case, it has essentially created a *soft node*; it cannot be used on its own as it lacks a link ID, so it must be merged with another node which has one. 

When the data is finished updating, all other DNS names that resolve to/from those claimed by the soft node are added to a "superset" of DNS names. This superset is used for merging information about the same node, provided by different plugins — all nodes which fall under the same superset are merged.

This model works for the most part. Nodes which occupy the same logical location on the network are combined and all of the data for the machine which serves, say, the `data.domain.com` and `webapp.domain.com` webpages is in one place. However, it is not rare for one node to act as a proxy or ingress for other nodes. In this situation, all DNS names resolve to the proxy node, and further forwarding is done at an application level - potentially unbeknownst to netdox.

The superset logic above would then merge the proxy node with all of the other nodes that it forwards to, as both the proxy and the destination node would claim one or more of the DNS names in the superset.

### Exclusive Nodes

To handle this, plugins may provide a boolean value for the *exclusivity* of the node's DNS names. A plugin which is simply providing additional information about a node, like the status of its SSL certificate for example, should set this boolean value to false - the plugin cannot say for certain that **only** the DNS names it knows about resolve to the node.

On the other hand, a plugin that provides information about Kubernetes pods for example, knows that **only** the domains that the Kubernetes configuration specifies will resolve to the pod.
This node can then be merged with *soft nodes* that are identified by a subset of the exclusive DNS names.

In order for this method to succeed, merging must be done according to something similar to  the *longest prefix matching* used by switches. Soft nodes merge with the *linkable node* (node with a link ID) that has the smallest matching DNS superset.
