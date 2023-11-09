use std::{
    collections::HashMap,
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::{
    config_err,
    error::{NetdoxError, NetdoxResult},
    io_err,
    remote::Remote,
};
use age::{secrecy::SecretString, Decryptor, Encryptor};
use serde::{Deserialize, Serialize};

/// Stores info about the remote, plugins, and extensions.
#[derive(Serialize, Deserialize, Debug)]
pub struct LocalConfig {
    /// URL of the redis server to use.
    pub redis: String,
    /// Redis database to use.
    pub redis_db: u8,
    /// Default network name.
    pub default_network: String,
    /// Configuration of the remote server to display on.
    pub remote: Remote,
    /// Plugin configuration.
    #[serde(rename = "plugin", default)]
    pub plugins: Vec<SubprocessConfig>,
    /// Extension configuration.
    #[serde(rename = "extension", default)]
    pub extensions: Vec<SubprocessConfig>,
}

/// Stores info about a single plugin or extension.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SubprocessConfig {
    /// Name of the plugin/extension.
    pub name: String,
    /// Path to the plugin binary.
    pub path: String,
    /// Plugin-specific configuration map.
    #[serde(flatten)]
    pub fields: HashMap<String, String>,
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
    /// Creates a new instance with no config.
    pub fn new(remote: Remote) -> Self {
        LocalConfig {
            redis: "redis URL".to_string(),
            redis_db: 0,
            default_network: "name for your default network".to_string(),
            remote,
            plugins: vec![],
            extensions: vec![],
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
    use std::{collections::HashMap, env::set_var, str::FromStr};

    use age::secrecy::{ExposeSecret, SecretString};

    use crate::{
        config::local::secret,
        remote::{DummyRemote, Remote},
    };

    use super::{LocalConfig, SubprocessConfig, CFG_SECRET_VAR};

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
            redis: "redis-url".to_string(),
            redis_db: 0,
            default_network: "default-net".to_string(),
            remote: Remote::Dummy(DummyRemote {
                field: "some-value".to_string(),
            }),
            extensions: vec![SubprocessConfig {
                name: "test-extension".to_string(),
                path: "/path/to/ext".to_string(),
                fields: HashMap::from([("key".to_string(), "value".to_string())]),
            }],
            plugins: vec![SubprocessConfig {
                name: "test-plugin".to_string(),
                path: "/path/to/plugin".to_string(),
                fields: HashMap::from([("key".to_string(), "value".to_string())]),
            }],
        };

        let enc = cfg.encrypt().unwrap();
        let dec = LocalConfig::decrypt(&enc).unwrap();

        assert_eq!(cfg.redis, dec.redis);
        assert_eq!(cfg.default_network, dec.default_network);
        assert!(matches!(dec.remote, Remote::Dummy(_)));
        assert_eq!(cfg.extensions, dec.extensions);
        assert_eq!(cfg.plugins, dec.plugins);
    }
}
