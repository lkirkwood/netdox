#[cfg(feature = "pageseeder")]
mod pageseeder;

use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::config::RemoteConfig;
use crate::error::NetdoxResult;

/// Interface for interacting with a remote server.
pub trait RemoteInterface {
    /// Tests the connection to the remote.
    fn test(&self) -> NetdoxResult<()>;

    /// Downloads the config.
    fn config(&self) -> NetdoxResult<RemoteConfig>;
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Remote {
    Dummy(DummyRemote),
    #[cfg(feature = "pageseeder")]
    #[serde(rename = "pageseeder")]
    PageSeeder(pageseeder::PSRemote),
}

impl Deref for Remote {
    type Target = dyn RemoteInterface;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Dummy(dummy) => dummy,
            #[cfg(feature = "pageseeder")]
            Self::PageSeeder(psremote) => psremote,
        }
    }
}

// Dummy

#[derive(Serialize, Deserialize, Debug)]
/// Dummy remote server that does nothing.
pub struct DummyRemote;

impl RemoteInterface for DummyRemote {
    fn test(&self) -> NetdoxResult<()> {
        Ok(())
    }

    fn config(&self) -> NetdoxResult<RemoteConfig> {
        Ok(RemoteConfig {
            exclude_dns: HashSet::new(),
            locations: HashMap::new(),
            plugin_cfg: HashMap::new(),
        })
    }
}
