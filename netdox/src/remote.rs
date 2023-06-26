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
    #[serde(rename = "pageseeder")]
    PageSeeder(PSRemote),
}

impl Deref for Remote {
    type Target = dyn RemoteInterface;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::PageSeeder(psremote) => psremote,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PSRemote {
    url: String,
    client_id: String,
    client_secret: String,
    username: String,
    group: String,
}

impl RemoteInterface for PSRemote {
    fn test(&self) -> NetdoxResult<()> {
        return Ok(());
    }
}
