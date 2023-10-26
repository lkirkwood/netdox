#[cfg(feature = "pageseeder")]
pub mod pageseeder;

use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use async_trait::async_trait;
use redis::Client;
use serde::{Deserialize, Serialize};

use crate::config::RemoteConfig;
use crate::error::NetdoxResult;

#[async_trait]
/// Interface for interacting with a remote server.
pub trait RemoteInterface {
    /// Tests the connection to the remote.
    async fn test(&self) -> NetdoxResult<()>;

    /// Downloads the config.
    async fn config(&self) -> NetdoxResult<RemoteConfig>;

    /// Publishes processed data from redis to the remote.
    async fn publish(&self, client: &mut Client) -> NetdoxResult<()>;
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
pub struct DummyRemote {
    pub field: String,
}

#[async_trait]
impl RemoteInterface for DummyRemote {
    async fn test(&self) -> NetdoxResult<()> {
        Ok(())
    }

    async fn config(&self) -> NetdoxResult<RemoteConfig> {
        Ok(RemoteConfig {
            exclude_dns: HashSet::new(),
            locations: HashMap::new(),
            plugin_cfg: HashMap::new(),
        })
    }

    async fn publish(&self, _: &mut Client) -> NetdoxResult<()> {
        Ok(())
    }
}
