use std::{
    collections::HashMap,
    env, fs,
    io::{Read, Write},
    path::PathBuf,
};

use crate::{
    config_err,
    error::{NetdoxError, NetdoxResult},
    remote::Remote,
};
use age::{secrecy::SecretString, Decryptor, Encryptor};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// URL of the redis server to use.
    pub redis: String,
    /// Default network name.
    pub default_network: String,
    /// Configuration of the remote server to display on.
    pub remote: Remote,
    /// Plugin configuration.
    #[serde(rename = "plugin")]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PluginConfig {
    /// Name of the plugin.
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

impl Config {
    /// Encrypts this config and writes it to the appropriate location.
    pub fn write(&self) -> NetdoxResult<PathBuf> {
        let path = match env::var(CFG_PATH_VAR) {
            Ok(path) => path,
            Err(_) => match env::var("HOME") {
                Ok(home) => format!("{}/.config/.netdox", home),
                Err(_) => panic!(
                    "Cannot find path to store encrypted config: \
                    please set ${} or $HOME.",
                    CFG_PATH_VAR
                ),
            },
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
                Err(_) => panic!(
                    "Cannot find path to store encrypted config: \
                    please set ${} or $HOME.",
                    CFG_PATH_VAR
                ),
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

        let plain = match toml::to_string(self) {
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
