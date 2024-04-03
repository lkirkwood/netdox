#[cfg(feature = "pageseeder")]
pub mod pageseeder;

use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::RemoteConfig;
use crate::data::model::ObjectID;
use crate::data::DataConn;
use crate::error::NetdoxResult;

#[async_trait]
/// Interface for interacting with a remote server.
pub trait RemoteInterface {
    /// Tests the connection to the remote.
    async fn test(&self) -> NetdoxResult<()>;

    /// Downloads the config.
    async fn config(&self) -> NetdoxResult<RemoteConfig>;

    /// Gets Object IDs that have a given label applied.
    async fn labeled(&self, label: &str) -> NetdoxResult<Vec<ObjectID>>;

    /// Publishes processed data from redis to the remote.
    async fn publish(&self, con: Box<dyn DataConn>) -> NetdoxResult<()>;
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
            exclusions: HashSet::new(),
            locations: HashMap::new(),
            metadata: HashMap::new(),
        })
    }

    async fn labeled(&self, _label: &str) -> NetdoxResult<Vec<ObjectID>> {
        Ok(vec![])
    }

    async fn publish(&self, _: Box<dyn DataConn>) -> NetdoxResult<()> {
        Ok(())
    }
}
