#[cfg(feature = "pageseeder")]
mod pageseeder;

use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::error::NetdoxResult;

/// Interface for interacting with a remote server.
pub trait RemoteInterface {
    /// Tests the connection to the remote.
    fn test(&self) -> NetdoxResult<()>;
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
}
