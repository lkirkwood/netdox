#[cfg(feature = "pageseeder")]
pub mod pageseeder;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};

use crate::config::RemoteConfig;
use crate::data::model::ObjectID;
use crate::data::DataStore;
use crate::error::NetdoxResult;

#[async_trait]
#[enum_dispatch]
/// Interface for interacting with a remote server.
pub trait RemoteInterface {
    /// Tests the connection to the remote.
    async fn test(&self) -> NetdoxResult<()>;

    /// Downloads the config.
    async fn config(&self) -> NetdoxResult<RemoteConfig>;

    /// Gets Object IDs that have a given label applied.
    async fn labeled(&self, label: &str) -> NetdoxResult<Vec<ObjectID>>;

    /// Publishes processed data from redis to the remote.
    async fn publish(&self, con: DataStore, backup: Option<PathBuf>) -> NetdoxResult<()>;
}

#[allow(clippy::large_enum_variant)]
#[enum_dispatch(RemoteInterface)]
#[derive(Serialize, Deserialize, Debug)]
pub enum Remote {
    Dummy(DummyRemote),
    #[cfg(feature = "pageseeder")]
    #[serde(rename = "pageseeder")]
    PageSeeder(pageseeder::PSRemote),
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
            exclusions: HashSet::new(),
            locations: HashMap::new(),
            metadata: HashMap::new(),
        })
    }

    async fn labeled(&self, _: &str) -> NetdoxResult<Vec<ObjectID>> {
        Ok(vec![])
    }

    async fn publish(&self, _: DataStore, _: Option<PathBuf>) -> NetdoxResult<()> {
        Ok(())
    }
}
