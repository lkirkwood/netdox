use crate::{
    config::RemoteConfig,
    config_err,
    data::Datastore,
    error::{NetdoxError, NetdoxResult},
    io_err, redis_err, remote_err,
};

use async_trait::async_trait;
use pageseeder::api::{model::EventType, oauth::PSCredentials, PSServer};
use pageseeder::{
    api::model::{Thread, ThreadStatus, ThreadZip},
    psml::model::Document,
};
use quick_xml::de;
use redis::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

use super::{config::parse_config, REMOTE_CONFIG_PATH};

/// Returns the docid of a DNS object's document from its qualified name.
fn dns_qname_to_docid(qname: &str) -> String {
    format!("_nd_dns_{}", qname.replace('[', "").replace(']', "_"))
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PSRemote {
    pub url: String,
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub group: String,
}

impl PSRemote {
    /// Returns a PSServer that can be used to communicate with the remote.
    pub fn server(&self) -> PSServer {
        PSServer::new(
            self.url.clone(),
            PSCredentials::ClientCredentials {
                id: self.client_id.clone(),
                secret: self.client_secret.clone(),
            },
        )
    }

    pub async fn uri_from_path(&self, path: &str) -> NetdoxResult<String> {
        let (folder, file) = if path.contains('/') {
            path.rsplit_once('/').unwrap()
        } else {
            ("", path)
        };

        let group_slug = self.group.replace('-', "/");
        let filter =
            format!("pstype:document,psfilename:{file},psfolder:/ps/{group_slug}/{folder}");

        let server = self.server();
        let search_results = server
            .group_search(&self.group, HashMap::from([("filters", filter.as_str())]))
            .await?;

        let page = match search_results.first() {
            None => {
                return remote_err!(format!("No pages of results for document at path: {path}"))
            }
            Some(page) => page,
        };

        let result = match page.results.first() {
            None => return remote_err!(format!("No results for document at path: {path}")),
            Some(result) => result,
        };

        for field in &result.fields {
            if field.name == "psid" {
                if field.value.is_empty() {
                    return remote_err!(format!(
                        "URI field was empty for document at path: {path}"
                    ));
                } else {
                    return Ok(field.value.clone());
                }
            }
        }

        remote_err!(format!("No document had a URI at path: {path}"))
    }

    /// Waits for a thread to finish.
    pub async fn await_thread(&self, mut thread: Thread) -> NetdoxResult<Thread> {
        let server = self.server();
        loop {
            if !thread.status.running() {
                match thread.status {
                    // TODO check meaning of warning status
                    ThreadStatus::Completed | ThreadStatus::Warning => return Ok(thread),
                    ThreadStatus::Error | ThreadStatus::Failed | ThreadStatus::Cancelled => {
                        let mut err = format!("Thread has status {}", thread.status);
                        if let Some(message) = thread.message {
                            err.push_str(&format!("; message was: {}", message.message));
                        }
                        return remote_err!(err);
                    }
                    _ => unreachable!(),
                }
            }
            thread = server.thread_progress(&thread.id).await?;
        }
    }

    pub async fn download_config(&self, zip: ThreadZip) -> NetdoxResult<RemoteConfig> {
        let zip_resp = self
            .server()
            .checked_get(
                format!("ps/member-resource/{}/{}", self.group, zip.filename),
                None,
                None,
            )
            .await?;

        let mut zip_reader = match zip_resp.bytes().await {
            Ok(bytes) => Cursor::new(bytes),
            Err(err) => {
                return remote_err!(format!(
                    "Failed to get bytes of zip file from remote: {err:?}"
                ))
            }
        };

        let mut zip = match ZipArchive::new(&mut zip_reader) {
            Ok(zip) => zip,
            Err(err) => {
                return io_err!(format!(
                    "Failed to read bytes from remote as zip: {}",
                    err.to_string()
                ))
            }
        };

        // TODO use constant here.
        let mut file = match zip.by_name("config.psml") {
            Ok(file) => file,
            Err(err) => {
                return remote_err!(format!(
                    "Zip from remote server has no file config.psml: {}",
                    err.to_string()
                ))
            }
        };

        let mut string = String::new();
        file.read_to_string(&mut string)?;

        let doc: Document = match de::from_str(&string) {
            Ok(doc) => doc,
            Err(err) => {
                return config_err!(format!(
                    "Failed to parse config file from remote as PSML: {}",
                    err.to_string()
                ))
            }
        };

        parse_config(doc)
    }

    async fn publish_dns(&self, qname: &str) -> NetdoxResult<()> {
        Ok(())
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
        let server = self.server();
        let mut con = match client.get_connection() {
            Ok(con) => con,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get connection to redis: {}",
                    err.to_string()
                ))
            }
        };

        let info_ft = server.get_uris_history(
            &self.group,
            vec![EventType::Modification],
            HashMap::from([("author", self.username.as_str()), ("from", "???")]),
        );

        Ok(())
    }
}
