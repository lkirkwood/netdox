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
use paris::{error, warn};

use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use redis::Client;

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
    Init {
        /// Path to config file to initialize from.
        config_path: PathBuf,
    },

    /// Dumps the config to stdout.
    Config {
        #[command(subcommand)]
        cmd: ConfigCommand,
    },
    /// Updates the data in redis.
    Update,
    /// Processes data layer
    Process,
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
        Commands::Init { config_path } => {
            init(&config_path);
        }
        Commands::Config { cmd } => match cmd {
            ConfigCommand::Load { config_path } => load_cfg(config_path),
            ConfigCommand::Dump { config_path } => dump_cfg(config_path),
        },
        Commands::Update => update(),
        Commands::Process => process(),
        Commands::Publish => publish(),
    }
}

#[tokio::main]
// TODO possibly remove this function
async fn init(config_path: &PathBuf) {
    let config_str = fs::read_to_string(config_path).expect("Failed to read configuration file.");
    let config: LocalConfig =
        toml::from_str(&config_str).expect("Failed to parse configuration file.");

    Client::open(config.redis.as_str())
        .unwrap_or_else(|_| {
            panic!(
                "Failed to create client for redis server at: {}",
                &config.redis
            )
        })
        .get_connection()
        .unwrap_or_else(|_| {
            panic!(
                "Failed to open connection for redis server at: {}",
                &config.redis
            )
        });

    config.remote.test().await.unwrap();
    config.write().unwrap();
    println!(
        "Successfully encrypted and stored the config. \
              You should delete the plain text file at {config_path:?} now."
    )
}

#[tokio::main]
async fn update() {
    for result in update::update(&LocalConfig::read().unwrap()).await.unwrap() {
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

#[tokio::main]
async fn process() {
    let config = LocalConfig::read().unwrap();
    let mut client = Client::open(config.redis.as_str()).unwrap_or_else(|_| {
        panic!(
            "Failed to create client for redis server at: {}",
            &config.redis
        )
    });

    process::process(&mut client).await.unwrap();
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
