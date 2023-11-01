mod config;
mod data;
mod error;
#[cfg(test)]
mod lua_tests;
mod process;
mod remote;
#[cfg(test)]
mod tests_common;
mod update;

use config::LocalConfig;
use error::NetdoxResult;
use paris::{error, warn};
use update::SubprocessResult;

use std::{
    collections::HashMap,
    fs,
    io::{stdin, stdout, Write},
    path::PathBuf,
};

use clap::{Parser, Subcommand};
use redis::Client;

use crate::{config::SubprocessConfig, remote::Remote};

// CLI

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,

    /// Turn on debug logging.
    #[arg(short, long)]
    debug: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialises a new instance of netdox.
    Init,

    /// Commands for manipulating the config.
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },
    /// Updates the data in the datastore using plugins and extensions.
    Update,
    /// Publishes processed data to the remote.
    Publish,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Reads a plain text config file and encrypts then stores it for use.
    #[command(name = "load")]
    Load {
        /// Path to the plain text config file to load.
        config_path: PathBuf,
    },
    /// Reads the current encrypted and stored config file, and writes it out
    /// in plain text to the given path.
    #[command(name = "dump")]
    Dump {
        /// Path to write the plain text config file to.
        config_path: PathBuf,
    },
}

// FUNCTIONALITY
// TODO make top level fns return result

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Init => {
            init();
        }
        Commands::Config { cmd } => match cmd {
            ConfigCommand::Load { config_path } => load_cfg(config_path),
            ConfigCommand::Dump { config_path } => dump_cfg(config_path),
        },
        Commands::Update => update(),
        Commands::Publish => publish(),
    }
}

fn init() {
    let mut remotes = String::new();

    #[cfg(feature = "pageseeder")]
    {
        remotes.push_str("pageseeder, ");
    }

    let mut remote = None;
    while remote.is_none() {
        let input = choose_remote(&remotes[..remotes.len() - 2]);

        #[cfg(feature = "pageseeder")]
        {
            use remote::pageseeder::PSRemote;
            if input.trim() == "pageseeder" {
                remote = Some(Remote::PageSeeder(PSRemote {
                    url: "pageseeder URL".to_string(),
                    username: "username".to_string(),
                    group: "group".to_string(),
                    client_id: "OAuth2 client ID".to_string(),
                    client_secret: "OAuth2 client secret".to_string(),
                }));
            }
        }

        if remote.is_none() {
            println!("Unsupported remote: {input}");
        }
    }

    let mut config = LocalConfig::new(remote.unwrap());

    config.plugins.push(SubprocessConfig {
        fields: HashMap::from([(
            "plugin config key".to_string(),
            "plugin config value".to_string(),
        )]),
        name: "example plugin name".to_string(),
        path: "/path/to/plugin/binary".to_string(),
    });

    config.extensions.push(SubprocessConfig {
        fields: HashMap::from([(
            "extension config key".to_string(),
            "extension config value".to_string(),
        )]),
        name: "example extension name".to_string(),
        path: "/path/to/extension/binary".to_string(),
    });

    let mut config_str = String::from("# This is a template config file.\n");
    config_str.push_str(
        "# You should populate the fields here and run: netdox config load <this file>\n\n",
    );
    config_str.push_str(&toml::ser::to_string_pretty(&config).unwrap());

    fs::write("config.toml", config_str).unwrap();

    println!("A template config file has been written to: config.toml");
    println!("Populate the values and run: netdox config load config.toml");
}

fn choose_remote(remotes: &str) -> String {
    print!("What kind of remote do you want to use? ({remotes}): ",);
    let _ = stdout().flush();
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    input
}

#[tokio::main]
async fn update() {
    let config = LocalConfig::read().unwrap();

    read_results(update::run_plugins(&config).await.unwrap());

    process(&config).await.unwrap();

    read_results(update::run_extensions(&config).await.unwrap());
}

fn read_results(results: Vec<SubprocessResult>) {
    for result in results {
        if let Some(num) = result.code {
            if num != 0 {
                error!(
                    "{} \"{}\" had non-zero exit code {num}.",
                    result.kind, result.name
                );
            }
        } else {
            warn!("{} \"{}\" had unknown exit code.", result.kind, result.name);
        }
    }
}

/// Processes raw nodes into linkable nodes.
async fn process(config: &LocalConfig) -> NetdoxResult<()> {
    let mut client = Client::open(config.redis.as_str()).unwrap_or_else(|_| {
        panic!(
            "Failed to create client for redis server at: {}",
            &config.redis
        )
    });

    process::process(&mut client).await
}

#[tokio::main]
async fn publish() {
    let config = LocalConfig::read().unwrap();
    let mut client = Client::open(config.redis.as_str()).unwrap_or_else(|_| {
        panic!(
            "Failed to create client for redis server at: {}",
            &config.redis
        )
    });
    config.remote.publish(&mut client).await.unwrap();
}

// CONFIG

fn load_cfg(path: PathBuf) {
    let string = fs::read_to_string(&path).unwrap();
    let cfg: LocalConfig = toml::from_str(&string).unwrap();
    cfg.write().unwrap();
    println!("Encrypted and stored config from {path:?}");
}

fn dump_cfg(path: PathBuf) {
    let cfg = LocalConfig::read().unwrap();
    fs::write(&path, toml::to_string_pretty(&cfg).unwrap()).unwrap();
    println!("Wrote config in plain text to {path:?}");
}
