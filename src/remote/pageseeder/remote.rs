use crate::{
    config::RemoteConfig,
    config_err,
    data::DataConn,
    error::{NetdoxError, NetdoxResult},
    io_err, redis_err,
    remote::pageseeder::{config::parse_config, publish::PSPublisher},
    remote_err,
};

use async_trait::async_trait;
use pageseeder::{
    api::model::{Thread, ThreadStatus, ThreadZip},
    error::PSError,
    psml::{model::Document, text::ParaContent},
};
use pageseeder::{
    api::{oauth::PSCredentials, PSServer},
    psml::model::{FragmentContent, Fragments},
};
use quick_xml::de;
use redis::Client;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

use super::config::{REMOTE_CONFIG_DOCID, REMOTE_CONFIG_FNAME};

pub const CHANGELOG_DOCID: &str = "_nd_changelog";
pub const CHANGELOG_FRAGMENT: &str = "last-change";

// TODO add lazy static for pattern here

/// Returns the docid of a DNS object's document from its qualified name.
pub fn dns_qname_to_docid(qname: &str) -> String {
    let pattern = Regex::new("[^a-zA-Z0-9_-]").unwrap();
    format!("_nd_dns_{}", pattern.replace_all(qname, "_"))
}

/// Returns the docid of a Node's document from its link id.
pub fn node_id_to_docid(link_id: &str) -> String {
    let pattern = Regex::new("[^a-zA-Z0-9_-]").unwrap();
    format!("_nd_node_{}", pattern.replace_all(link_id, "_"))
}

pub fn report_id_to_docid(id: &str) -> String {
    let pattern = Regex::new("[^a-zA-Z0-9_-]").unwrap();
    format!("_nd_report_{}", pattern.replace_all(&id, "_"))
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
    /// TODO MUST CHANGE THIS will generate new token for every thread - should impl deser manually
    pub fn server(&self) -> PSServer {
        PSServer::new(
            self.url.clone(),
            PSCredentials::ClientCredentials {
                id: self.client_id.clone(),
                secret: self.client_secret.clone(),
            },
        )
    }

    pub async fn _uri_from_path(&self, path: &str) -> NetdoxResult<String> {
        let (folder, file) = match path.rsplit_once('/') {
            None => ("", path),
            Some(tuple) => tuple,
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
        let mut file = match zip.by_name(REMOTE_CONFIG_FNAME) {
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

    /// Gets the ID of the latest change to be published to PageSeeder (if any).
    pub async fn get_last_change(&self) -> NetdoxResult<Option<String>> {
        let ps_log = match self
            .server()
            .get_uri_fragment(
                &self.username,
                &self.group,
                CHANGELOG_DOCID,
                CHANGELOG_FRAGMENT,
                HashMap::new(),
            )
            .await
        {
            Ok(log) => log,
            Err(PSError::ApiError(api_err)) => {
                if api_err.message == "Unable to find matching uri." {
                    todo!("Create changelog document")
                } else {
                    Err(PSError::ApiError(api_err))?
                }
            }
            Err(other) => Err(other)?,
        };

        let para = match ps_log.fragment {
            Some(Fragments::Fragment(frag)) => {
                match frag.content.first() {
                    Some(FragmentContent::Para(para)) => para.clone(),
                    _ => return remote_err!(
                    "Changelog last-change fragment on PageSeeder has incorrect content (expected single para)".to_string()
                )
                }
            }
            _ => {
                return remote_err!(
                    "Changelog on PageSeeder has incorrect content (expected fragment)".to_string()
                )
            }
        };

        match para.content.into_iter().next() {
            Some(ParaContent::Text(text)) => Ok(Some(text)),
            _ => Ok(None),
        }
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
                    .uri_export(&self.username, REMOTE_CONFIG_DOCID, vec![])
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
        let mut con = match client.get_async_connection().await {
            Ok(con) => con,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get connection to redis: {}",
                    err.to_string()
                ))
            }
        };

        let changes = con
            .get_changes(self.get_last_change().await?.as_deref())
            .await?;
        self.apply_changes(client, changes).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    use quick_xml::de;

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

    #[tokio::test]
    async fn test_changelog() {
        remote().get_last_change().await.unwrap();
    }
}
