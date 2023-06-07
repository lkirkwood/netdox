# Architecture
Redis server starts from a dump.
Plugins then run, and create or modify the data types below using custom commands. If a change occurs during one of these commands a message is pushed to the change log.
Once the plugins finish running the display hooks are triggered.
Display hooks render the changes on the remote in any way they see fit.
![Netdox/Redis Architecture](netdox-redis-arch.drawio.svg)

# Implementation Overview

## Redis Commands
All commands create a change log message if they make a change, and all commands take a plugin name argument.

+ ### Create DNS record
	+ Takes the following arguments:
		+ Record name
		+ Record type
		+ Record value
+ ### Create node
	+ Takes the following arguments:
		+ Set of DNS names
		+ DNS names exclusive? boolean
+ ### Create plugin data
	+ Takes the following arguments:
		+ Identity
		+ Some plugin data

## Redis Keys
+ ### changelog  -  Stream
	+ Contains all changes made to the dataset.
	+ #### Entries
		+ ##### create dns name
			+ `${dns_name}`
		+ ##### create dns record
			+ `${record_name} --(${record_type})-> ${record_value}`
		+ ##### create node with names
			+ `${dns_name_1}, ${dns_name_2}, ...`
		+ ##### plugin updated node name
			+ `(${node_id}) ${old_name} ---> ${new_name}`
		+ ##### plugin updated node exclusivity
			+ `(${node_id}) ${old_exc} ---> ${new_exc}`
   
+ ### dns  -  Set
	+ Contains all dns names.
+ ### dns;${dns_name};plugins  -  Set
	+ Contains all plugins that reference **${dns_name}**.
+ ### dns;\${dns_name};\${plugin_name}  -  Set
	+ Contains all types of records provided by **${plugin_name}** that reference **\${dns_name}**.
+ ### dns;\${dns_name};\${plugin_name};\${record_type}  -  Set
	+ Contains the dns record **values** for all dns records with name **\${dns_name}**, of type **\${record_type}** provided by **\${plugin_name}**. 

+ ### nodes  -  Set
	+ Contains all node ids.
+ ### nodes;\${node_id};plugins  -  Set
	+ Contains all plugins that provde a node at **\${node_id}**.
+ ### nodes;\${node_id};\${plugin_name}  -  Hash
	+ Contains details of a node as provided by the plugin **\${plugin_name}**.
	+ #### Entries
		+ ##### name
			+ Plugin name
		+ ##### exclusive
			+ Whether to merge this node with an equivalent set of dns names.

## Other Features
+ ### Notes
	+ Handle in pageseeder??
	+ Could also just be plugin
	+ Alternatively could add core support for pulling specified data from display remote.
+ ### Organizations
	+ Map org names to set of identities
 + ### Locations
	 + Map locations to subnets
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
