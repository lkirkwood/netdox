# Process Flow
+ The local config is read, which defines the redis server to use, the plugins to call, which remote server to use, etc.
+ Plugins run, and create or modify the data in the redis server using custom functions. In broad strokes this involves creating DNS records and nodes, and then attaching data to these objects.
+ Once the plugins finish running, nodes are consolidated so that all the data for any one node is available in one place.
+ Finally, output connectors render the changes on a remote server in any way they see fit.
  
![Netdox/Redis Architecture](netdox-redis-arch.drawio.svg)

# Extending Netdox
Netdox is designed to be very extensible. To this end, all creation of data is done by external executables (plugins) that are merely configured in a file — there is no need to modify netdox itself to get data from a new source. Simply package your code in any way you like and tell netdox where to find the executable, along with any parameters it might need.  

Furthermore, the output connectors (the code that reads data and generates documents) are separate from the core. Currently, writing a new output connector involves simply implementing a trait, and then adding your implementation to an enum. This should make it relatively simple for developers to write new output connectors — *but* you are tied to Rust. In the future, I hope to implement this in much the same way plugins work.

# Redis
Netdox uses redis as a bucket in which any application may dump data as long as it conforms to the expected specification. This is most easily done using the custom functions that can be called like normal redis functions — however there is nothing to stop you from creating data structures by hand. But... don't do that.

The functions for creating data are documented [here](docs/functions.md) (useful for those writing plugins) and the specification for the data created by those functions is documented [here](docs/data.md) (useful for those writing output connectors).

# Linking

One of the problems netdox tries to solve is:
Given data from independent sources with no knowledge of each other, consolidate information that pertains to the same node around a *stable* identifier.
Here stable means it should not change, so that the ID can be used for document linking and history.

This "link ID" must be provided by one of the plugins — preferably the one which knows the most about it and can therefore choose the best ID, e.g. a hypervisor. Remember, if the ID changes a new separate node will be created.

## Soft Nodes

A soft node is a node with no link id. Nodes that *do* have a link ID are called linkable nodes or just nodes.

Soft nodes are just a container — their data is not displayed unless they merge with a linkable node (how would you link to the document?)

## Supersets

Now the problem is that without knowledge of the plugin that created the node, no other plugin can predict the ID. 
So, if the plugin is not setting a link ID it must specify the correct node using a set DNS names that the plugin knows resolve to it. Often this is just the IP of the node.

In netdox a superset is the largest set of DNS names reachable through DNS records. Take the following list of DNS records:
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

So, the superset for a node created with the DNS names `domain.com` and `domain.net`, is the combined list:
+ `domain.com`
+ `192.168.200.25`
+ `domain.net`
+ `192.168.200.81`
+ `alias.org`

This list, sorted and joined with `;` characters, is the ID for a soft node.

## Exclusive Nodes

The exclusive field on a node indicates that the plugin that created it is *sure* that no other DNS names should be added to the superset.

So far we've seen that nodes which appear to occupy the same location on the network are combined. For example, all of the data for the machine which serves the `data.domain.com` and `webapp.domain.com` webpages would be in one node if both domains resolve to the same IP address.

This model works for the most part.  However, it is not rare for one node to act as a proxy for other nodes. In this situation, all DNS names resolve to the proxy node, and further forwarding is done at an application level unbeknown to netdox.

The superset logic above would then merge the proxy node with all of the other nodes that it forwards to, as both the proxy and the destination node would claim one or more of the DNS names in the superset,

[[docs/exclusive.svg|Diagram illustrating the need for the exclusive parameter]]

On the other hand, a plugin that provides information about Kubernetes pods for example, knows that **only** the domains that the Kubernetes configuration specifies will actually resolve to the pod. Therefore even if the node's DNS names are also part of the superset for a proxy server, implying that they are the same machine, the two can be distinguished.

This node *can* still be merged with soft nodes with supersets that are the same or a subset of its own.

In order for this method to succeed, merging must be done according to something similar to  the *longest prefix matching* used by switches. Soft nodes merge with the *linkable node* (node with a link ID) that has the smallest matching DNS superset.
