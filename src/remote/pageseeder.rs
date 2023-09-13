use crate::config::RemoteConfig;
use crate::error::{NetdoxError, NetdoxResult};
use async_trait::async_trait;
use pageseeder::api::{oauth::PSCredentials, PSServer};
use pageseeder::error::PSError;
use serde::{Deserialize, Serialize};

const REMOTE_CONFIG_PATH: &str = "website/config.psml";

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
        let mut server = self.server();
        let thread = server
            .uri_export(&self.username, REMOTE_CONFIG_PATH, &vec![])
            .await?;

        todo!("Download files from thread")
    }
}

fn uri_from_path(remote: &PSRemote, path: &str) -> NetdoxResult<String> {
    let (folder, file) = if path.contains('/') {
        path.rsplit_once('/').unwrap()
    } else {
        ("", path)
    };

    let group_slug = remote.group.replace('-', "/");
    let search_params = vec![(
        "filters",
        &format!("pstype:folder,psfilename:{file},psfolder:/ps/{group_slug}/{folder}"),
    )];

    // let search_results =
    todo!("Search for file and return URI")
}
