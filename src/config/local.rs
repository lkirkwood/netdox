use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Display,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::{
    config_err,
    data::{DataConn, DataStore},
    error::{NetdoxError, NetdoxResult},
    io_err, redis_err,
    remote::Remote,
};
use age::{secrecy::SecretString, Decryptor, Encryptor};
use redis::Client;
use serde::{Deserialize, Serialize};
use toml::Value;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum IgnoreList {
    Set(HashSet<String>),
    Path(String),
}

/// Default Redis port.
fn default_port() -> usize {
    6379
}

/// Default Redis logical database.
fn default_db() -> usize {
    0
}

/// Config for a redis data store.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct RedisConfig {
    /// Hostname of the redis server to use.
    pub host: String,
    /// Port of the redis server to use.
    #[serde(default = "default_port")]
    pub port: usize,
    /// Logical database in the redis instance to use.
    #[serde(default = "default_db")]
    pub db: usize,
    /// Username to use when authenticating with redis - if any.
    pub username: Option<String>,
    /// Password to use when authenticating with redis - if any.
    pub password: Option<String>,
}

impl RedisConfig {
    pub fn url(&self) -> String {
        format!(
            "redis://{host}:{port}/{db}",
            host = self.host,
            port = self.port,
            db = self.db
        )
    }
}

/// Stores info about the remote, plugins, and extensions.
#[derive(Serialize, Deserialize, Debug)]
pub struct LocalConfig {
    /// Config for redis server to use as data store.
    pub redis: RedisConfig,
    /// Default network name.
    pub default_network: String,
    /// DNS names to ignore when added to datastore.
    pub dns_ignore: IgnoreList,
    /// Configuration of the remote server to display on.
    pub remote: Remote,
    /// Plugin configuration.
    #[serde(rename = "plugin", default)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Serialize, Deserialize, Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum PluginStage {
    #[serde(rename = "write-only")]
    WriteOnly,
    #[serde(rename = "read-write")]
    ReadWrite,
    #[serde(rename = "connectors")]
    Connectors,
}

impl Display for PluginStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WriteOnly => write!(f, "write-only"),
            Self::ReadWrite => write!(f, "read-write"),
            Self::Connectors => write!(f, "connectors"),
        }
    }
}

/// Stores configuration for a plugin stage.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PluginStageConfig {
    /// Path to the executable for this stage.
    pub path: String,
    /// Plugin-specific configuration map for this stage.
    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
}

/// Stores configuration for a plugin.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct PluginConfig {
    /// Name of the plugin.
    pub name: String,
    /// Plugin-specific configuration map for all stages.
    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
    /// Stages the plugin will run in.
    pub stages: HashMap<PluginStage, PluginStageConfig>,
}

pub const CFG_PATH_VAR: &str = "NETDOX_CONFIG";
const CFG_SECRET_VAR: &str = "NETDOX_SECRET";

fn secret() -> NetdoxResult<SecretString> {
    match env::var(CFG_SECRET_VAR) {
        Err(err) => {
            config_err!(format!(
                "Failed to read environment variable {CFG_SECRET_VAR}: {err}"
            ))
        }
        Ok(txt) => Ok(SecretString::from(txt)),
    }
}

impl LocalConfig {
    /// Creates a template instance with no config.
    pub fn template(remote: Remote) -> Self {
        LocalConfig {
            redis: RedisConfig {
                host: "my.redis.net".to_string(),
                port: 6379,
                db: 0,
                username: Some("redis-username".to_string()),
                password: Some("redis-password-123!?".to_string()),
            },
            default_network: "name for your default network".to_string(),
            dns_ignore: IgnoreList::Set(HashSet::new()),
            remote,
            plugins: vec![],
        }
    }

    /// Creates a DataClient for the configured redis instance and returns it.
    pub async fn con(&self) -> NetdoxResult<DataStore> {
        match Client::open(self.redis.url().as_str()) {
            Ok(client) => match client.get_multiplexed_tokio_connection().await {
                Ok(con) => match &self.redis.password {
                    None => Ok(DataStore::Redis(con)),
                    Some(pass) => {
                        let mut con = DataStore::Redis(con);
                        con.auth(pass, &self.redis.username).await?;
                        Ok(con)
                    }
                },
                Err(err) => redis_err!(format!("Failed to open redis connection: {err}",)),
            },
            Err(err) => {
                redis_err!(format!("Failed to open redis client: {err}"))
            }
        }
    }

    /// Encrypts this config and writes it to the appropriate location.
    pub fn write(&self) -> NetdoxResult<PathBuf> {
        let path = {
            if let Ok(path) = env::var(CFG_PATH_VAR) {
                path
            } else if let Ok(home) = env::var("XDG_CONFIG_HOME") {
                format!("{home}/.netdox")
            } else if let Ok(home) = env::var("HOME") {
                if Path::new(&format!("{home}/.config")).is_dir() {
                    format!("{home}/.config/.netdox")
                } else {
                    format!("{home}/.netdox")
                }
            } else {
                return io_err!(format!(
                    "Cannot find path to store encrypted config. Please set ${CFG_PATH_VAR}."
                ));
            }
        };

        if let Err(err) = fs::write(&path, self.encrypt()?) {
            config_err!(format!("Failed to write encrypted config to {path}: {err}"))
        } else {
            Ok(PathBuf::from(path))
        }
    }

    pub fn read() -> NetdoxResult<Self> {
        let path = match env::var(CFG_PATH_VAR) {
            Ok(path) => path,
            Err(_) => match env::var("HOME") {
                Ok(home) => format!("{}/.config/.netdox", home),
                Err(_) => {
                    return io_err!(format!(
                        "Cannot find path to store encrypted config: \
                    please set ${CFG_PATH_VAR} or $HOME."
                    ))
                }
            },
        };

        let bytes = match fs::read(&path) {
            Err(err) => return config_err!(format!("Failed to read config file at {path}: {err}")),
            Ok(_b) => _b,
        };

        Self::decrypt(&bytes)
    }

    /// Encrypts this config.
    pub fn encrypt(&self) -> NetdoxResult<Vec<u8>> {
        let enc = Encryptor::with_user_passphrase(secret()?);

        let plain = match toml::to_string(&self) {
            Err(err) => return config_err!(format!("Failed to serialize config: {err}")),
            Ok(txt) => txt,
        };
        let mut cipher = vec![];
        let mut writer = match enc.wrap_output(&mut cipher) {
            Err(err) => return config_err!(format!("Failed while encrypting config: {err}")),
            Ok(_w) => _w,
        };

        if let Err(err) = writer.write_all(plain.as_bytes()) {
            return config_err!(format!("Failed while encrypting config: {err}"));
        } else if let Err(err) = writer.finish() {
            return config_err!(format!("Failed while encrypting config: {err}"));
        }

        Ok(cipher)
    }

    /// Decrypts a config from some cipher bytes.
    pub fn decrypt(cipher: &[u8]) -> NetdoxResult<Self> {
        let dec = match Decryptor::new(cipher) {
            Err(err) => return config_err!(format!("Failed creating decryptor: {err}")),
            Ok(decryptor) => match decryptor {
                Decryptor::Passphrase(pass_decryptor) => pass_decryptor,
                _ => unreachable!(),
            },
        };

        let mut plain = vec![];
        let mut reader = match dec.decrypt(&secret()?, None) {
            Err(err) => return config_err!(format!("Failed creating decrypting reader: {err}")),
            Ok(_r) => _r,
        };
        if let Err(err) = reader.read_to_end(&mut plain) {
            return config_err!(format!("Failed reading decrypted config: {err}"));
        }

        let plain_str = match std::str::from_utf8(&plain) {
            Err(err) => return config_err!(format!("Failed encoding decrypted text: {err}")),
            Ok(txt) => txt,
        };

        match toml::from_str(plain_str) {
            Err(err) => config_err!(format!("Failed to deserialize config: {err}")),
            Ok(cfg) => Ok(cfg),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        env::set_var,
        str::FromStr,
    };

    use age::secrecy::{ExposeSecret, SecretString};
    use toml::Value;

    use crate::{
        config::local::{secret, IgnoreList, PluginStage, PluginStageConfig, RedisConfig},
        remote::{DummyRemote, Remote},
    };

    use super::{LocalConfig, PluginConfig, CFG_SECRET_VAR};

    const FAKE_SECRET: &str = "secret-key!";

    #[test]
    fn test_secret_success() {
        set_var(CFG_SECRET_VAR, FAKE_SECRET);
        let expected = SecretString::from_str(FAKE_SECRET).unwrap();
        let actual = secret().unwrap();
        assert_eq!(*expected.expose_secret(), *actual.expose_secret());
    }

    #[test]
    fn test_cfg_crypt_roundtrip() {
        set_var(CFG_SECRET_VAR, FAKE_SECRET);

        let cfg = LocalConfig {
            redis: RedisConfig {
                host: "my.redis.net".to_string(),
                port: 6379,
                db: 0,
                username: Some("redis-username".to_string()),
                password: Some("redis-password-123!?".to_string()),
            },
            default_network: "default-net".to_string(),
            dns_ignore: IgnoreList::Set(HashSet::new()),
            remote: Remote::Dummy(DummyRemote {
                field: "some-value".to_string(),
            }),
            plugins: vec![PluginConfig {
                name: "test-plugin".to_string(),
                fields: HashMap::from([(
                    "global-key".to_string(),
                    Value::String("global-value".to_string()),
                )]),
                stages: HashMap::from([
                    (
                        PluginStage::WriteOnly,
                        PluginStageConfig {
                            path: "/path/to/write/only/exe".to_string(),
                            fields: HashMap::from([(
                                "write-only-key".to_string(),
                                Value::String("write-only-value".to_string()),
                            )]),
                        },
                    ),
                    (
                        PluginStage::ReadWrite,
                        PluginStageConfig {
                            path: "/path/to/read/write/exe".to_string(),
                            fields: HashMap::from([(
                                "read-write-key".to_string(),
                                Value::String("read-write-value".to_string()),
                            )]),
                        },
                    ),
                ]),
            }],
        };

        let enc = cfg.encrypt().unwrap();
        let dec = LocalConfig::decrypt(&enc).unwrap();

        assert_eq!(cfg.redis, dec.redis);
        assert_eq!(cfg.default_network, dec.default_network);
        assert!(matches!(dec.remote, Remote::Dummy(_)));
        assert_eq!(cfg.plugins, dec.plugins);
    }
}
