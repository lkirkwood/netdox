# Netdox CLI

This is the CLI for netdox.
To see the available commands, run `netdox -h`.

## Config

The config file is loaded from a user-provided path during initialization. The file is then encrypted and stored at `$NETDOX_CONFIG` if set, defaulting to `$HOME/.config/.netdox`.

The encryption key used is the value of `$NETDOX_SECRET`. This variable must be set in the environment in which netdox runs. It should be a 256-bit string. Longer keys will be truncated.

The config file can be managed using the `netdox config` subcommmand.
