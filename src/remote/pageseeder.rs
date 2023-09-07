use crate::config::RemoteConfig;
use crate::error::{NetdoxError, NetdoxResult};
use async_trait::async_trait;
use pageseeder::api::{oauth::PSCredentials, PSServer};
use pageseeder::error::PSError;
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

impl From<PSError> for NetdoxError {
    fn from(value: PSError) -> Self {
        Self::Remote(value.to_string())
    }
}

#[async_trait]
impl crate::remote::RemoteInterface for PSRemote {
    async fn test(&self) -> NetdoxResult<()> {
        Ok(())
    }

    async fn config(&self) -> NetdoxResult<RemoteConfig> {
        let mut server = PSServer::new(
            self.url.clone(),
            PSCredentials::ClientCredentials {
                id: self.client_id.clone(),
                secret: self.client_secret.clone(),
            },
        );
        let thread = server
            .uri_export(&self.username, REMOTE_CONFIG_PATH)
            .await?;

        println!("{thread:?}");

        todo!("Download files from thread")
    }
}
