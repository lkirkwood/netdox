mod config;
mod remote;
#[cfg(test)]
mod tests;

use crate::config::RemoteConfig;
use crate::data::Datastore;
use crate::error::{NetdoxError, NetdoxResult};
use crate::{redis_err, remote_err};
use async_trait::async_trait;
use pageseeder::error::PSError;
use redis::Client;
pub use remote::PSRemote;

const REMOTE_CONFIG_PATH: &str = "website/config";

impl From<PSError> for NetdoxError {
    fn from(value: PSError) -> Self {
        Self::Remote(value.to_string())
    }
}

#[async_trait]
impl crate::remote::RemoteInterface for PSRemote {
    async fn test(&self) -> NetdoxResult<()> {
        match self.server().get_group(&self.group).await {
            Ok(_) => Ok(()),
            Err(err) => remote_err!(err.to_string()),
        }
    }

    async fn config(&self) -> NetdoxResult<RemoteConfig> {
        let thread = self
            .await_thread(
                self.server()
                    .uri_export(
                        &self.username,
                        &self.uri_from_path(REMOTE_CONFIG_PATH).await?,
                        vec![],
                    )
                    .await?,
            )
            .await?;

        match thread.zip {
            Some(zip) => self.download_config(zip).await,
            None => {
                remote_err!(format!(
                    "Thread with id ({}) has no zip attached.",
                    thread.id
                ))
            }
        }
    }

    async fn publish(&self, client: &mut Client) -> NetdoxResult<()> {
        let mut data_con = match client.get_connection() {
            Ok(con) => con,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get connection to redis: {}",
                    err.to_string()
                ))
            }
        };

        let dns = data_con.get_dns()?;
        todo!("Serialise DNS to PSML");

        Ok(())
    }
}
