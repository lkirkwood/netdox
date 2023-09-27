# Config

The config file is loaded from a user-provided path during initialization. The file is then encrypted and stored at `$NETDOX_CONFIG` if set, defaulting to `$HOME/.config/.netdox`.

The encryption key used is the value of `$NETDOX_SECRET`. This variable must be set in the environment in which netdox runs.

The config file can be managed using the `netdox config` subcommmand.

# Testing

Running many of the tests requires a redis server. The url for this server should be available under the environment variable `NETDOX_TEST_REDIS_URL`. **WARNING**: All data in the server used for testing will be lost! 

# Architecture

Plugins run, and create or modify the data in the redis server using custom commands in `functions.lua`. 
Once the plugins finish running, nodes are processed to a separate database in the same redis and the output connector is run.
Output connectors render the changes on the remote in any way they see fit.
![Netdox/Redis Architecture](netdox-redis-arch.drawio.svg)

# Implementation Overview

## Network Address Translation

Previously, netdox has ignored the concept of separate networks. All addresses were considered local to "*the*" network.
In the new version, this will change. All DNS names must be qualified by a network ID, in order to allow netdox to model separate networks. This includes virtual networks like those constructed by Kubernetes.
Those DNS names that are provided by plugins without a network qualifier will be qualified with the default network. This is configured before running netdox the first time.

## IDs

The new version of netdox will use a plugin provided string as the linkable ID for a node.
Plugins other than the one that manages the node cannot predict this "link ID", so providing data to a node must use other methods for matching.
When adding data to a node, plugins must provide two additional pieces of information:
+ The DNS names that the plugin believes resolve to the desired node 
+ Whether those names are "exclusive"

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

## Redis Commands
All commands create a change log message if they make a change, and all commands take a plugin name argument.

**COMING SOON**
        
## Other Features
+ ### Notes
	+ Handle in pageseeder??
	+ Could also just be plugin
	+ Alternatively could add core support for pulling specified data from display remote.
+ ### Organizations
	+ Implemented using metadata and a plugin.
 + ### Locations
	 + Implemented using metadata and a plugin.
 + ### PageSeeder
	 + Output driver
	 + Use python from netdox for now, in future move to lib generated from psml xsd?
 + ### Links
	 + Custom data type within redis
	 + OR special prefix/mangling to indicate link
 + ### Sentencing
	 + If write is not a new change, append to confirmation log?
		 + At the end of plugin run, every data point not confirmed or changed gets marked as stale.

## PageSeeder Atomic Updates
+ Create fragment edit with [PUT URI FRAGMENT](https://dev.pageseeder.com/api/services/uri-fragment-PUT.html)
+ Move fragment to correct section with [MOVE URI FRAGMENT](https://dev.pageseeder.com/api/services/move-uri-fragment-POST.html)
