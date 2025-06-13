# Quickstart Plugin Authoring Guide

You're going to want to run Netdox, so clone this repository. Set up a redis instance on your local machine as well. This will help you debug. I like to run mine on port 9999, but the default is 6379. Just be aware of that — I'll use 9999 for the remainder of this guide.
Follow the following steps to get your development environment set up:
1. Set the environment variable `NETDOX_SECRET` to anything. This will be used to encrypt the config file, because it might contain sensitive information.

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

3. Populate the config file redis details and load it with `cargo run config load <path/to/config.toml>`. If it works, you're good to go. Run `cargo run init` to set up redis.

4. Start your plugin. This can be anywhere, but probably in [this repository](https://gitlab.allette.com.au/allette/netdox/netdox-redis-plugins) because that's where all the other ones are. Just make a file, whatever language you like.

5. Add your plugin to the config file. If you used the template replace everything below `[[plugin]]` This should look something like:

```toml
[[plugin]]
name = "my-plugin-name"
stages.write-only.path = "path/to/the/executable"
```

Don't let the `stages.write-only.path` thing confuse you. If you're not sure, just keep this as is and fill in the path. If you want to know more, look [here](arch.md).

6. Almost there! Try `cargo run config load <path/to/config.toml>` one more time to update the encrypted config. Then, run `cargo run update`. This is going to run your plugin. Put a print statement or something in your plugin so you know it's working.

7. Okay, so your plugin is running. Lovely. Now we need to make it do something. Plugins work by connecting to redis and invoking special functions. These functions are named like `netdox_create_dns` or `netdox_create_node`. More detail on these functions and their parameters etc. can be found [here](docs/functions.md). If you're using Python or Rust, it's probably easiest to use the wrapper libraries that call these redis functions for you with a nicer interface. Find them [here (Python)](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-py) and [here (Rust)](https://gitlab.allette.com.au/allette/netdox/netdox-plugin-rs). Either way, you're going to need to familiarise yourself with what kind of data you want to create, and once you do it should be easy enough to figure out which functions you want to call. A description of the different data types is [here](docs/arch.md#key-concepts).
