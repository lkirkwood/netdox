use crate::{
    config::RemoteConfig,
    config_err,
    data::{model::ObjectID, DataConn, DataStore},
    error::{NetdoxError, NetdoxResult},
    io_err,
    remote::pageseeder::{
        config::parse_config,
        psml::{DNS_OBJECT_TYPE, NODE_OBJECT_TYPE, OBJECT_ID_PROPNAME, REPORT_OBJECT_TYPE},
        publish::PSPublisher,
    },
    remote_err,
};

use async_trait::async_trait;
use lazy_static::lazy_static;
use pageseeder_api::{
    error::PSError,
    model::{Thread, ThreadStatus, ThreadZip},
    oauth::{PSCredentials, PSToken},
    PSServer,
};
use psml::{
    model::{Document, FragmentContent, Fragments},
    text::ParaContent,
};
use quick_xml::de;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};
use tokio::sync::Mutex;
use zip::ZipArchive;

use super::{
    config::{REMOTE_CONFIG_DOCID, REMOTE_CONFIG_FNAME},
    psml::OBJECT_TYPE_PROPNAME,
};

pub const CHANGELOG_DOCID: &str = "_nd_changelog";
pub const CHANGELOG_FRAGMENT: &str = "last-change";

lazy_static! {
    static ref DOCID_INVALID_CHARS: Regex = Regex::new("[^a-zA-Z0-9_-]").unwrap();
}

/// Returns the docid of a DNS object's document from its qualified name.
pub fn dns_qname_to_docid(qname: &str) -> String {
    format!(
        "_nd_{DNS_OBJECT_TYPE}_{}",
        DOCID_INVALID_CHARS.replace_all(qname, "_")
    )
}

/// Returns the docid of a Node's document from its link id.
pub fn node_id_to_docid(link_id: &str) -> String {
    format!(
        "_nd_{NODE_OBJECT_TYPE}_{}",
        DOCID_INVALID_CHARS.replace_all(link_id, "_")
    )
}

pub fn report_id_to_docid(id: &str) -> String {
    format!(
        "_nd_{REPORT_OBJECT_TYPE}_{}",
        DOCID_INVALID_CHARS.replace_all(id, "_")
    )
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PSRemote {
    pub url: String,
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub group: String,
    #[serde(skip)]
    pub pstoken: Mutex<Option<PSToken>>,
}

impl PSRemote {
    /// Returns a PSServer instance with a shared token.
    pub async fn server(&self) -> NetdoxResult<PSServer> {
        let creds = PSCredentials::ClientCredentials {
            id: self.client_id.clone(),
            secret: self.client_secret.clone(),
        };

        let mut token = self.pstoken.lock().await;
        match token.is_some() {
            true => Ok(PSServer::preauth(
                self.url.clone(),
                creds,
                token.as_ref().unwrap().clone(),
            )),
            false => {
                let server = PSServer::new(self.url.clone(), creds);
                if let Err(err) = server.update_token().await {
                    return remote_err!(format!("Failed to get PS auth token: {err}"));
                }

                let _ = token.insert(
                    server
                        .token
                        .lock()
                        .as_ref()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .to_owned(),
                );

                Ok(server)
            }
        }
    }

    pub async fn _uri_from_path(&self, path: &str) -> NetdoxResult<String> {
        let (folder, file) = match path.rsplit_once('/') {
            None => ("", path),
            Some(tuple) => tuple,
        };

        let group_slug = self.group.replace('-', "/");
        let filter =
            format!("pstype:document,psfilename:{file},psfolder:/ps/{group_slug}/{folder}");

        let server = self.server().await?;
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
        let server = self.server().await?;
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
            .await?
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
            .await?
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
                    return Ok(None);
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

lazy_static! {
    static ref OBJECT_ID_INDEX_PROPERTY: String = format!("psproperty-{OBJECT_ID_PROPNAME}");
    static ref OBJECT_TYPE_INDEX_PROPERTY: String = format!("psproperty-{OBJECT_TYPE_PROPNAME}");
}

#[async_trait]
impl crate::remote::RemoteInterface for PSRemote {
    async fn test(&self) -> NetdoxResult<()> {
        match self.server().await?.get_group(&self.group).await {
            Ok(_) => Ok(()),
            Err(err) => remote_err!(err.to_string()),
        }
    }

    async fn config(&self) -> NetdoxResult<RemoteConfig> {
        let thread = self
            .await_thread(
                self.server()
                    .await?
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

    async fn labeled(&self, label: &str) -> NetdoxResult<Vec<ObjectID>> {
        let filter = format!("pslabel:{label}");
        let results = self
            .server()
            .await?
            .group_search(&self.group, HashMap::from([("filters", filter.as_ref())]))
            .await?;

        let mut labeled = vec![];
        for page in results {
            for result in page.results {
                let mut obj_id = None;
                let mut obj_type = None;
                for field in result.fields {
                    if field.name == *OBJECT_ID_INDEX_PROPERTY {
                        obj_id = Some(field.value);
                    } else if field.name == *OBJECT_TYPE_INDEX_PROPERTY {
                        obj_type = Some(field.value);
                    }
                }

                if let (Some(obj_id), Some(obj_type)) = (obj_id, obj_type) {
                    labeled.push(match obj_type.as_str() {
                        DNS_OBJECT_TYPE => ObjectID::DNS(obj_id),
                        NODE_OBJECT_TYPE => ObjectID::Node(obj_id),
                        REPORT_OBJECT_TYPE => ObjectID::Report(obj_id),
                        _ => {
                            return remote_err!(format!(
                                "Invalid object type in document on remote: {obj_type}"
                            ))
                        }
                    })
                }
            }
        }

        Ok(labeled)
    }

    async fn publish(&self, mut con: DataStore) -> NetdoxResult<()> {
        let changes = con
            .get_changes(self.get_last_change().await?.as_deref())
            .await?;
        self.apply_changes(con, changes).await?;

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
            pstoken: Mutex::new(None),
        }
    }

    #[test]
    fn test_config() {
        let string = fs::read_to_string("test/config.psml").unwrap();
        let config = de::from_str(&string).unwrap();
        parse_config(config).unwrap();
    }

    #[ignore]
    #[tokio::test]
    async fn test_config_remote() {
        remote().config().await.unwrap();
    }

    #[ignore]
    #[tokio::test]
    async fn test_changelog() {
        remote().get_last_change().await.unwrap();
    }
}
