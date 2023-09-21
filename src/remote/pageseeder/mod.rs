mod config;
mod remote;

use crate::config::RemoteConfig;
use crate::error::{NetdoxError, NetdoxResult};
use crate::remote_err;
use async_trait::async_trait;
use pageseeder::error::PSError;
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
        Ok(())
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
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use quick_xml::de;

    use super::{config::parse_config, PSRemote};
    use crate::remote::RemoteInterface;

    fn remote() -> PSRemote {
        PSRemote {
            url: env::var("PS_TEST_URL").expect("Set environment variable PS_TEST_URL"),
            client_id: env::var("PS_TEST_ID").expect("Set environment variable PS_TEST_ID"),
            client_secret: env::var("PS_TEST_SECRET")
                .expect("Set environment variable PS_TEST_SECRET"),
            group: env::var("PS_TEST_GROUP").expect("Set environment variable PS_TEST_GROUP"),
            username: env::var("PS_TEST_USER").expect("Set environment variable PS_TEST_USER"),
        }
    }

    #[test]
    fn test_config() {
        let string = fs::read_to_string("test/config.psml").unwrap();
        let config = de::from_str(&string).unwrap();
        parse_config(config).unwrap();
    }

    #[tokio::test]
    async fn test_config_remote() {
        remote().config().await.unwrap();
    }
}
