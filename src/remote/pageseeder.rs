use crate::config::RemoteConfig;
use crate::error::NetdoxResult;
use serde::{Deserialize, Serialize};

const REMOTE_CONFIG_PATH: &str = "website/config.psml";

#[derive(Serialize, Deserialize, Debug)]
pub struct PSRemote {
    url: String,
    client_id: String,
    client_secret: String,
    username: String,
    group: String,
}

impl crate::remote::RemoteInterface for PSRemote {
    fn test(&self) -> NetdoxResult<()> {
        Ok(())
    }

    fn config(&self) -> NetdoxResult<RemoteConfig> {
        todo!("Implement pulling config from PS.")
    }
}
