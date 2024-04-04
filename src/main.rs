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

use config::{local::IgnoreList, LocalConfig, SubprocessConfig};
use error::{NetdoxError, NetdoxResult};
use paris::{error, info, success, warn, Logger};
use remote::{Remote, RemoteInterface};
use tokio::join;
use update::SubprocessResult;

use std::{
    collections::HashMap,
    fs,
    io::{stdin, stdout, Write},
    path::PathBuf,
    process::exit,
};

use clap::{Parser, Subcommand};
use redis::{cmd as redis_cmd, Client};
use toml::Value;

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
    /// Updates data via plugins and processes it.
    Update {
        /// Resets the configured database before updating.
        #[arg(short, long)]
        reset_db: bool,
    },
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
        Commands::Update { reset_db } => update(reset_db),
        Commands::Publish => publish(),
    }
}

/// Gets the user to choose a remote type and then writes a config template for them to populate.
fn init() {
    match fs::write("config.toml", config_template(choose_remote())) {
        Ok(()) => {
            info!("A template config file has been written to: config.toml");
            info!("Populate the values and run: netdox config load config.toml");
        }
        Err(err) => {
            error!("Failed to initialize: {err}");
            exit(1);
        }
    };
}

/// Local config template with the given remote type, as a string.
fn config_template(remote: Remote) -> String {
    let mut config = LocalConfig::template(remote);

    config.plugins.push(SubprocessConfig {
        fields: HashMap::from([(
            "plugin config key".to_string(),
            Value::String("plugin config value".to_string()),
        )]),
        name: "example plugin name".to_string(),
        path: "/path/to/plugin/binary".to_string(),
    });

    config.extensions.push(SubprocessConfig {
        fields: HashMap::from([(
            "extension config key".to_string(),
            Value::String("extension config value".to_string()),
        )]),
        name: "example extension name".to_string(),
        path: "/path/to/extension/binary".to_string(),
    });

    let mut config_str = String::from("# This is a template config file.\n");
    config_str.push_str(
        "# You should populate the fields here and run: netdox config load <this file>\n\n",
    );
    config_str.push_str(&toml::ser::to_string_pretty(&config).unwrap());

    config_str
}

/// Prompt for user choosing remote type.
/// Currently only pageseeder is implemented.
fn choose_remote() -> Remote {
    let mut remotes = String::new();

    #[cfg(feature = "pageseeder")]
    {
        remotes.push_str("pageseeder, ");
    }

    let mut remote = None;
    while remote.is_none() {
        print!(
            "What kind of remote do you want to use? ({}): ",
            &remotes[..remotes.len() - 2] // slice trims trailing comma + space
        );
        let _ = stdout().flush();
        let mut input = String::new();

        if let Err(err) = stdin().read_line(&mut input) {
            error!("Failed while reading from stdin: {err}");
            exit(1);
        }

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
                    pstoken: Default::default(),
                }));
            }
        }

        if remote.is_none() {
            error!("Unsupported remote: {input}");
        }
    }

    remote.unwrap()
}

#[tokio::main]
async fn update(reset_db: bool) {
    info!("Starting update process.");

    let local_cfg = match LocalConfig::read() {
        Ok(config) => config,
        Err(err) => {
            error!("Failed to update data while retrieving local config: {err}");
            exit(1);
        }
    };

    if reset_db {
        match reset(&local_cfg).await {
            Ok(true) => {
                success!("Database was reset.");
            }
            Ok(false) => {
                success!("Aborting database reset — no data will be destroyed.");
                exit(1);
            }
            Err(err) => {
                error!("Failed to reset database before updating: {err}");
                exit(1);
            }
        }
    }

    let plugin_results = match update::run_plugins(&local_cfg).await {
        Ok(results) => results,
        Err(err) => {
            error!("Failed to run plugins: {err}");
            exit(1);
        }
    };

    read_results(plugin_results);

    info!("Processing data...");
    let (proc_res, remote_res) = join!(process(&local_cfg), local_cfg.remote.config());

    if let Err(err) = proc_res {
        error!("Failed while processing data: {err}");
        exit(1);
    } else {
        success!("Processed data.");
    }

    let mut log = Logger::new();
    log.loading("Applying remote config to data.");
    if let Ok(remote_cfg) = remote_res {
        match local_cfg.con().await {
            Ok(con) => {
                let (locations_res, metadata_res) = join!(
                    remote_cfg.set_locations(con.clone()),
                    remote_cfg.set_metadata(con, &local_cfg.remote)
                );

                let mut failed = false;
                if let Err(err) = locations_res {
                    log.error(format!("Failed while setting locations: {err}"));
                    failed = true;
                }
                if let Err(err) = metadata_res {
                    log.error(format!("Failed while setting metadata overrides: {err}"));
                }

                if failed {
                    exit(1);
                } else {
                    log.success("Applied remote config.");
                }
            }
            Err(err) => {
                log.error(format!("Failed to get connection to redis: {err}"));
                exit(1);
            }
        }
    } else {
        log.warn("Failed to pull config from the remote. If this is the first run, ignore this.");
        log.warn(format!("Error was: {}", remote_res.unwrap_err()));
    }

    let extension_results = match update::run_extensions(&local_cfg).await {
        Ok(results) => results,
        Err(err) => {
            error!("Failed to run extensions: {err}");
            exit(1);
        }
    };

    read_results(extension_results);
}

/// Resets the database after asking for confirmation.
/// Return value is true if reset was confirmed.
async fn reset(cfg: &LocalConfig) -> NetdoxResult<bool> {
    print!(
        "Are you sure you want to reset {}? All data will be lost (y/N): ",
        cfg.redis
    );
    let _ = stdout().flush();
    let mut input = String::new();
    if let Err(err) = stdin().read_line(&mut input) {
        return io_err!(format!("Failed to read input: {}", err.to_string()));
    }

    if (input.trim() != "y") & (input.trim() != "yes") {
        return Ok(false);
    }

    let mut client = match Client::open(cfg.redis.as_str()) {
        Ok(client) => client,
        Err(err) => return redis_err!(format!("Failed to open redis client: {}", err.to_string())),
    };

    if let Err(err) = redis_cmd("FLUSHALL").query::<String>(&mut client) {
        return redis_err!(format!("Failed to flush database: {}", err.to_string()));
    }

    let dns_ignore = match &cfg.dns_ignore {
        IgnoreList::Set(set) => set.clone(),
        IgnoreList::Path(path) => match fs::read_to_string(path) {
            Ok(str_list) => str_list.lines().map(|s| s.to_owned()).collect(),
            Err(err) => {
                return io_err!(format!("Failed to read DNS ignorelist from {path}: {err}"))
            }
        },
    };

    if let Err(err) = redis_cmd("FCALL")
        .arg("netdox_init")
        .arg(1)
        .arg(&cfg.default_network)
        .arg(dns_ignore)
        .query::<()>(&mut client)
    {
        return redis_err!(format!(
            "Failed to initialise database: {}",
            err.to_string()
        ));
    }

    Ok(true)
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
    let con = match config.con().await {
        Ok(con) => con,
        Err(err) => {
            return redis_err!(format!(
                "Failed to create client for redis server at {}: {err}",
                &config.redis
            ))
        }
    };

    process::process(con).await
}

#[tokio::main]
async fn publish() {
    let cfg = match LocalConfig::read() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("Failed to parse config as TOML: {err}");
            exit(1);
        }
    };

    let con = match cfg.con().await {
        Ok(con) => con,
        Err(err) => {
            error!(
                "Failed to create connection to redis server at {}: {err}",
                cfg.redis
            );
            exit(1);
        }
    };

    match cfg.remote.publish(con).await {
        Ok(()) => success!("Publishing complete."),
        Err(err) => error!("Failed to publish: {err}"),
    }
}

// CONFIG

#[tokio::main]
async fn load_cfg(path: PathBuf) {
    let string = match fs::read_to_string(&path) {
        Ok(string) => string,
        Err(err) => {
            error!("Failed to read config at {}: {err}", path.to_string_lossy());
            exit(1)
        }
    };

    let cfg: LocalConfig = match toml::from_str(&string) {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("Failed to parse config as TOML: {err}");
            exit(1);
        }
    };

    if let Err(err) = cfg.remote.test().await {
        error!("New config remote failed test: {err}");
        exit(1);
    };

    let client = match Client::open(cfg.redis.as_str()) {
        Ok(client) => client,
        Err(err) => {
            error!(
                "Failed to create client for redis server at {}: {err}",
                cfg.redis
            );
            exit(1);
        }
    };

    if let Err(err) = client.get_async_connection().await {
        error!("Failed to open connection with redis: {err}");
        exit(1);
    }

    if let Err(err) = cfg.write() {
        error!("Failed to write new config: {err}");
        exit(1);
    }

    info!("Encrypted and stored config from {path:?}");
}

fn dump_cfg(path: PathBuf) {
    let cfg = match LocalConfig::read() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("Failed to read encrypted local config: {err}");
            exit(1);
        }
    };

    let toml = match toml::to_string_pretty(&cfg) {
        Ok(toml) => toml,
        Err(err) => {
            error!("Failed to write config as TOML: {err}");
            exit(1);
        }
    };

    match fs::write(&path, toml) {
        Ok(()) => info!("Wrote config in plain text to {path:?}"),
        Err(err) => {
            error!("Failed to write config to disk: {err}");
            exit(1);
        }
    }
}
