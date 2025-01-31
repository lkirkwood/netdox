mod config;
mod data;
mod error;
#[cfg(test)]
mod lua_tests;
mod process;
mod query;
mod remote;
#[cfg(test)]
mod tests_common;
mod update;

use config::{IgnoreList, LocalConfig, PluginConfig, PluginStage, PluginStageConfig};
use error::{NetdoxError, NetdoxResult};
use paris::{error, info, success, warn, Logger};
use query::query;
use remote::{Remote, RemoteInterface};
use tokio::join;
use update::PluginResult;

use std::{
    collections::HashMap,
    fs,
    io::{stdin, stdout, Write},
    path::PathBuf,
    process::exit,
};

use clap::{Parser, Subcommand};
use redis::{cmd as redis_cmd, AsyncCommands, Client};
use toml::Value;

use crate::data::{model::DEFAULT_NETWORK_KEY, DataConn, DataStore};

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
        /// Add the specified plugin to a list.
        /// If the list has one or more members, only those plugins will run.
        /// If the exclude flag is present, only plugins not in the list will run.
        #[arg(short, long)]
        plugin: Option<Vec<String>>,
        /// Causes the list of plugins to be treated as an exclusion list.
        #[arg(short = 'x', long)]
        exclude: bool,
    },
    /// Publishes processed data to the remote.
    Publish {
        /// An optional path to write a backup of the published data to.
        #[arg(short, long)]
        backup: Option<PathBuf>,
    },
    /// Commands for querying data store.
    Query {
        #[command(subcommand)]
        cmd: QueryCommand,
    },
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

#[derive(Subcommand, Debug)]
enum QueryCommand {
    /// Prints out the number of each object type in the data store.
    #[command(name = "counts")]
    Counts,
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
        Commands::Update {
            reset_db,
            plugin,
            exclude,
        } => update(reset_db, plugin, exclude),
        Commands::Publish { backup } => publish(backup),
        Commands::Query { cmd } => query(cmd),
    }
    exit(0);
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

    config.plugins.push(PluginConfig {
        fields: HashMap::from([(
            "plugin config key".to_string(),
            Value::String("plugin config value".to_string()),
        )]),
        name: "example plugin name".to_string(),
        stages: HashMap::from([
            (
                PluginStage::WriteOnly,
                PluginStageConfig {
                    path: "/path/to/plugin/binary".to_string(),
                    fields: HashMap::new(),
                },
            ),
            (
                PluginStage::ReadWrite,
                PluginStageConfig {
                    path: "/path/to/other/binary".to_string(),
                    fields: HashMap::new(),
                },
            ),
        ]),
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
                    upload_dir: "directory to upload into".to_string(),
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
async fn update(reset_db: bool, plugins: Option<Vec<String>>, exclude: bool) {
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

    let write_only_results =
        match update::run_plugin_stage(&local_cfg, PluginStage::WriteOnly, &plugins, exclude).await
        {
            Ok(results) => results,
            Err(err) => {
                error!("Failed to run plugins: {err}");
                exit(1);
            }
        };

    read_results(write_only_results);

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

    let read_write_results =
        match update::run_plugin_stage(&local_cfg, PluginStage::ReadWrite, &plugins, exclude).await
        {
            Ok(results) => results,
            Err(err) => {
                error!("Failed to run plugins for read-write stage: {err}");
                exit(1);
            }
        };

    read_results(read_write_results);

    let connectors_results = match update::run_plugin_stage(
        &local_cfg,
        PluginStage::Connectors,
        &plugins,
        exclude,
    )
    .await
    {
        Ok(results) => results,
        Err(err) => {
            error!("Failed to run plugins for connectors stage: {err}");
            exit(1);
        }
    };

    read_results(connectors_results);

    match local_cfg.con().await {
        Ok(mut con) => {
            if let Err(err) = con.write_save().await {
                log.error(err);
                exit(1);
            }
        }
        Err(err) => {
            log.error(format!("Failed to get connection to redis: {err}"));
            exit(1);
        }
    }
}

/// Initialises the redis data store.
async fn init_db<C>(cfg: &LocalConfig, con: &mut C) -> NetdoxResult<()>
where
    C: redis::aio::ConnectionLike,
{
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
        .query_async::<_, ()>(con)
        .await
    {
        return redis_err!(format!("Failed to call Lua init function: {err}"));
    }

    Ok(())
}

/// Resets the database after asking for confirmation.
/// Return value is true if reset was confirmed.
async fn reset(cfg: &LocalConfig) -> NetdoxResult<bool> {
    print!(
        "Are you sure you want to reset {}? All data will be lost (y/N): ",
        cfg.redis.url()
    );
    let _ = stdout().flush();
    let mut input = String::new();
    if let Err(err) = stdin().read_line(&mut input) {
        return io_err!(format!("Failed to read input: {}", err.to_string()));
    }

    if (input.trim() != "y") & (input.trim() != "yes") {
        return Ok(false);
    }

    let mut con = match Client::open(cfg.redis.url().as_str()) {
        Ok(client) => match client.get_multiplexed_tokio_connection().await {
            Ok(con) => con,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to open redis connection: {}",
                    err.to_string()
                ))
            }
        },
        Err(err) => return redis_err!(format!("Failed to open redis client: {}", err.to_string())),
    };

    if let Some(pass) = &cfg.redis.password {
        DataStore::Redis(con.clone())
            .auth(pass, &cfg.redis.username)
            .await?;
    }

    if let Err(err) = redis_cmd("FLUSHALL")
        .query_async::<_, String>(&mut con)
        .await
    {
        return redis_err!(format!("Failed to flush database: {}", err.to_string()));
    }

    init_db(cfg, &mut con).await?;

    Ok(true)
}

/// Reads subprocess results and logs warnings or errors where required.
fn read_results(results: Vec<PluginResult>) {
    let mut any_err = false;
    for result in &results {
        if let Some(num) = result.code {
            if num != 0 {
                any_err = true;
                error!(
                    "Plugin {} had non-zero exit code {num} for {} stage.",
                    result.name, result.stage
                );
            }
        } else {
            warn!(
                "{} had unknown exit code for {} stage.",
                result.name, result.stage
            );
        }
    }

    if !results.is_empty() && !any_err {
        success!("All plugins completed successfully.")
    }
}

/// Processes raw nodes into linkable nodes.
async fn process(config: &LocalConfig) -> NetdoxResult<()> {
    let con = match config.con().await {
        Ok(con) => con,
        Err(err) => {
            return redis_err!(format!(
                "Failed to create client for redis server at {}: {err}",
                &config.redis.url()
            ))
        }
    };

    process::process(con).await
}

#[tokio::main]
async fn publish(backup: Option<PathBuf>) {
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
                cfg.redis.url()
            );
            exit(1);
        }
    };

    match cfg.remote.publish(con, backup).await {
        Ok(()) => success!("Publishing complete."),
        Err(err) => {
            error!("Failed to publish: {err}");
            exit(1);
        }
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

    let mut con = match cfg.con().await {
        Ok(DataStore::Redis(con)) => con,
        Err(err) => {
            error!("{err}");
            exit(1);
        }
    };

    match con.key_type::<_, String>(DEFAULT_NETWORK_KEY).await {
        Err(err) => {
            error!("Failed to check type of default network key: {err}");
            exit(1);
        }
        Ok(string) => match string.as_str() {
            "string" => check_default_net(con, &cfg).await,
            _ => {
                if let Err(err) = con
                    .set::<_, _, ()>(DEFAULT_NETWORK_KEY, &cfg.default_network)
                    .await
                {
                    error!("Failed to set default network: {err}");
                    exit(1);
                }
            }
        },
    }

    if let Err(err) = cfg.write() {
        error!("Failed to write new config: {err}");
        exit(1);
    }

    info!("Encrypted and stored config from {path:?}");
}

/// Checks the default network and updates it (if necessary) after confirming with the user.
async fn check_default_net<C>(mut con: C, cfg: &LocalConfig)
where
    C: redis::aio::ConnectionLike + Send,
{
    match con.get::<_, String>(DEFAULT_NETWORK_KEY).await {
        Err(err) => {
            error!("Failed to get default network: {err}");
            exit(1);
        }
        Ok(default_net) => {
            if default_net != cfg.default_network {
                println!("Existing default network ({default_net}) is different to the one specified in the config ({})", cfg.default_network);
                print!("Would you like to: (U)pdate the value/(R)eset the database/(C)ancel the operation?: ");
                let _ = stdout().flush();
                let mut input = String::new();
                if let Err(err) = stdin().read_line(&mut input) {
                    error!("Failed to read input: {err}");
                    exit(1);
                }

                match input.to_lowercase().chars().next() {
                    Some('u') => {
                        if let Err(err) = con
                            .set::<_, _, ()>(DEFAULT_NETWORK_KEY, &cfg.default_network)
                            .await
                        {
                            error!("Failed to update the default network: {err}");
                            exit(1);
                        }
                    }
                    Some('r') => match reset(cfg).await {
                        Ok(true) => {
                            success!("Database was reset.");
                        }
                        Ok(false) => {
                            success!("Aborting database reset — no data will be destroyed.");
                            warn!("Config will not be loaded.");
                            exit(1);
                        }
                        Err(err) => {
                            error!("Failed to reset database before updating: {err}");
                            warn!("Config will not be loaded.");
                            exit(1);
                        }
                    },
                    Some('c') => exit(0),
                    _ => {
                        error!("Unrecognised choice: {input}");
                        exit(1);
                    }
                }
            }
        }
    }
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
