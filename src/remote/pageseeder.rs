use std::collections::HashMap;

use crate::config::RemoteConfig;
use crate::error::{NetdoxError, NetdoxResult};
use crate::remote_err;
use async_trait::async_trait;
use pageseeder::api::model::ThreadStatus;
use pageseeder::api::{oauth::PSCredentials, PSServer};
use pageseeder::error::PSError;
use serde::{Deserialize, Serialize};

const REMOTE_CONFIG_PATH: &str = "website/config";

impl From<PSError> for NetdoxError {
    fn from(value: PSError) -> Self {
        Self::Remote(value.to_string())
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

impl PSRemote {
    /// Returns a PSServer that can be used to communicate with the remote.
    fn server(&self) -> PSServer {
        PSServer::new(
            self.url.clone(),
            PSCredentials::ClientCredentials {
                id: self.client_id.clone(),
                secret: self.client_secret.clone(),
            },
        )
    }
}

#[async_trait]
impl crate::remote::RemoteInterface for PSRemote {
    async fn test(&self) -> NetdoxResult<()> {
        Ok(())
    }

    async fn config(&self) -> NetdoxResult<RemoteConfig> {
        // TODO move this logic into file download fn
        let server = self.server();
        let uri = uri_from_path(self, REMOTE_CONFIG_PATH).await?;
        let thread = server.uri_export(&self.username, &uri, vec![]).await?;
        loop {
            let progress = server.thread_progress(&thread.id).await?;
            if !progress.status.running() {
                match progress.status {
                    // TODO check meaning of warning status
                    ThreadStatus::Complete | ThreadStatus::Warning => match progress.zip {
                        None => {
                            return remote_err!(format!(
                                "Completed thread with id ({}) has no zip file",
                                progress.id
                            ))
                        }
                        Some(_zip) => {
                            todo!("Download and unpack zip")
                        }
                    },
                    ThreadStatus::Error | ThreadStatus::Failed | ThreadStatus::Cancelled => {
                        let mut err = format!("Thread has status {}", progress.status);
                        if let Some(message) = progress.message {
                            err.push_str(&format!("; message was: {}", message.message));
                        }
                        return remote_err!(err);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

async fn uri_from_path(remote: &PSRemote, path: &str) -> NetdoxResult<String> {
    let (folder, file) = if path.contains('/') {
        path.rsplit_once('/').unwrap()
    } else {
        ("", path)
    };

    let group_slug = remote.group.replace('-', "/");
    let filter = format!("pstype:document,psfilename:{file},psfolder:/ps/{group_slug}/{folder}");

    let server = remote.server();
    let search_results = server
        .group_search(&remote.group, HashMap::from([("filters", filter.as_str())]))
        .await?;

    let page = match search_results.first() {
        None => return remote_err!(format!("No pages of results for document at path: {path}")),
        Some(page) => page,
    };

    let result = match page.results.first() {
        None => return remote_err!(format!("No results for document at path: {path}")),
        Some(result) => result,
    };

    for field in &result.fields {
        if field.name == "psid" {
            if field.value.is_empty() {
                return remote_err!(format!("URI field was empty for document at path: {path}"));
            } else {
                return Ok(field.value.clone());
            }
        }
    }

    remote_err!(format!("No document had a URI at path: {path}"))
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::PSRemote;
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

    #[tokio::test]
    async fn test_config() {
        remote().config().await.unwrap();
    }
}
