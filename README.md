# Netdox

Framework for generating network documentation. Plugins are used to query various APIs for information about the network, and then an output connector is used to generate documentation describing servers on the network, which domain names they use, IP addresses, etc.

# For Developers

Documentation:
+ [Netdox Architecture](/docs/arch.md)
+ [Running Tests](/docs/testing.md)
+ [Redis Data Spec](/docs/data.md)
+ [Redis Write API](/docs/functions.md)

# For Users

## Key Terms

+ Node — a physical or virtual computer.
+ DNS name — domain name or IPv4 address.
+ Plugin — executable that fetches data and puts it in redis.
+ Output connector — library that generates documents from redis data.
+ Remote — a server used to display documents generated by an output connector.

## Config

There are two sources of configuration for netdox. 
+ A document on the remote allows the end user to tweak small behaviours. 
+ A config file stored on the machine that runs netdox containing the (potentially sensitive) details of redis, the remote, plugins, etc.

The config file is loaded from a user-provided path during initialisation. The file is then encrypted and stored at the path given by the environment variable `$NETDOX_CONFIG` if it's set, defaulting to `$HOME/.config/.netdox`.

You must set `$NETDOX_SECRET` to the value to use as your encryption key. This can be any string.

The config file can be managed using the `netdox config` subcommmand. When in doubt: `netdox config -h`

## Network Address Translation

All DNS names are always be qualified by a network ID. This ID can refer to your LAN, WAN, or virtual networks like those constructed in Kubernetes.
DNS names without a network qualifier will be qualified with the default network when creating objects — e.g. you may create a DNS record like:
    `domain.com -> 192.168.0.1`
but it will become the following:
    `[default-net]domain.com -> [default-net]192.168.0.1`
(provided the DNS record type is one of `CNAME`, `A`, `PTR`).
When placing links in your data you must use this qualified representation.
The default network is configured before running netdox for the first time, and is passed as a parameter to all plugins.


