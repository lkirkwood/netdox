# Quickstart Plugin Authoring Guide

## Things to know upfront

Netdox the application doesn't do very much. The user provides netdox with a config file, and that file lists a bunch of other applications. Those applications should call functions on a redis database; think of this like a REST API. The redis functions create data inside the database in a known, structured format. 

Netdox will run all the applications listed in the config file. Since these applications should be putting data in redis using the functions (read: API), netdox will then query the database and turn the data into documents for humans to read. That's all netdox does: run a series of other executables and then query redis. The plugins are responsible for everything else.

Calling a function on a redis database is not complicated, but the API is fairly limited. To make this easier to work with, there are two wrapper libraries, one for Python and one for Rust. These libraries make it easier to write plugins in those languages because they provide native functions for interacting with the redis API using sane datastructures.

Finally, read this guide all the way through before you start. It will help to know where you are going and why you are doing each step.

## The guide

### High level overview
1. Start a redis server, and have a PageSeeder server you can use.
2. Write a config file that tells Netdox what redis and PageSeeder server to use, which plugins to call, etc.
3. Write a plugin (some executable file) and add it to the config file. It doesn't have to do anything yet.
4. Load the config file — encrypt it and store it in a safe place.
5. Run a Netdox update, which in turn will run your plugin.

### Step by step
You're going to want to run Netdox, so clone this repository. Set up a redis instance on your local machine as well. This will help you debug. I like to run mine on port 9999, but the default is 6379. Just be aware of that — I'll use 9999 for the remainder of this guide.
Follow the following steps to get your development environment set up:
1. Set the environment variable `NETDOX_SECRET` to anything. This will be used to encrypt the config file, because it might contain sensitive information. When you run the `config load` command, Netdox will read the config file, encrypt it using the value of this variable, and store it somewhere. You shouldn't need to worry about this encrypted version, just remember every time you change the config file you have to tell netdox to load it again and replace the existing encrypted one.

2. Build and run Netdox: `cargo run` in this repository or `make test` to run the tests. If thats working, you can start a config file with `cargo run config template`. It will look something like this:
```toml
# This is a template config file.
# You should populate the fields here and run: netdox config load <this file>

default_network = "name for your default network"
dns_ignore = []

[redis]
host = "my.redis.net" # probably localhost
port = 6379 # change to 9999 if thats where your redis is running
db = 0
username = "redis-username" # optional
password = "redis-password-123!?" # optional

[remote.pageseeder]
url = "pageseeder URL"
client_id = "OAuth2 client ID"
client_secret = "OAuth2 client secret"
username = "username"
group = "group"
upload_dir = "directory to upload into"

[[plugin]]
name = "example plugin name"
"plugin config key" = "plugin config value"

[plugin.stages.read-write]
path = "/path/to/other/binary"

[plugin.stages.write-only]
path = "/path/to/plugin/binary"
```

3. Populate the config file redis details and load it with `cargo run config load <path/to/config.toml>`. If it works, you're good to go. Run `cargo run init` to set up redis. You have just done two things: first, you instructed netdox to encrypt and store the config file. Even if you delete the plain text version, it doesn't matter. Make sure to `config load` again if the config file ever changes though. The second thing you did was set up redis, which involves loading in the special functions mentioned in the first section plus some other things you don't need to worry about just yet.

4. Start writing your plugin. This can be anywhere, but probably in [this repository](https://gitlab.allette.com.au/allette/netdox/netdox-redis-plugins) because that's where all the other ones are. Just make a file, whatever language you like. It doesn't have to do anything, we just want *some* executable that you can run to prove your config is working. Could be a Python script, could be a bash script, could be anything, it doesn't matter.

5. Add your plugin to the config file. When we do this we are informing netdox about the plugin. Next time you call `update` (i.e. `cargo run update` or `netdox update`) netdox will look through every plugin entry in this config file and try to run it. Netdox won't bother paying attention to the output or depend in any way on the behaviour of the plugin, it will just trust that the plugin has called any redis functions that it needed to. If you used the template config file above, replace all the lines *below* `[[plugin]]`. This should look something like:

```toml
[[plugin]]
name = "my-plugin-name"
stages.write-only.path = "path/to/the/executable"
```

Don't let the `stages.write-only.path` thing confuse you. If you're not sure, just keep this as is and fill in the path. If you want to know more, look [here](arch.md).

6. Almost there! We have now told Netdox about our plugin. Next time we run `update` Netdox will try to run the executable at that path. Try `cargo run config load <path/to/config.toml>` one more time to update the encrypted config. Then, run `cargo run update`. This is going to run your plugin. Put a print statement or something in your plugin so you know it's working.

7. Okay, so your plugin is running. Lovely. Now we need to make it do something. Plugins work by connecting to redis and invoking special functions. These functions are named like `netdox_create_dns` or `netdox_create_node`. More detail on these functions and their parameters etc. can be found [here](docs/functions.md). If you're using Python or Rust, it's probably easiest to use the wrapper libraries that call these redis functions for you with a nicer interface. Find them [here (Python)](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-py) and [here (Rust)](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-rs). Either way, you're going to need to familiarise yourself with what kind of data you want to create, and once you do it should be easy enough to figure out which functions you want to call. A description of the different data types is [here](docs/arch.md#key-concepts).
