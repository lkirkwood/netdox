use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    io::{Cursor, Write},
};

use crate::{
    data::{
        model::{
            Change, ChangelogEntry, DNSRecords, DataKind, DNS_KEY, NODES_KEY, PDATA_KEY,
            PROC_NODES_KEY, REPORTS_KEY,
        },
        store::DataStore,
        DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    io_err, process_err, redis_err, remote_err,
};

use super::{
    psml::{
        changelog_document, dns_name_document, links::LinkContent, metadata_fragment,
        processed_node_document, remote_config_document, report_document, DNS_RECORD_SECTION,
        IMPLIED_RECORD_SECTION, METADATA_FRAGMENT, PDATA_SECTION, RDATA_SECTION,
    },
    remote::{
        dns_qname_to_docid, node_id_to_docid, report_id_to_docid, CHANGELOG_DOCID,
        CHANGELOG_FRAGMENT,
    },
    PSRemote,
};
use async_trait::async_trait;
use futures::future::{join_all, BoxFuture};
use pageseeder_api::error::PSError;
use paris::{warn, Logger};
use psml::{
    model::{Document, Fragment, FragmentContent, Fragments, PropertiesFragment},
    text::{Para, ParaContent},
};
use quick_xml::se as xml_se;
use zip::ZipWriter;

const UPLOAD_DIR: &str = "netdox";

/// Data that can be published by a PSPublisher.
pub enum PublishData<'a> {
    Create {
        target_ids: Vec<String>,
        document: Document,
    },
    Update {
        target_id: String,
        future: BoxFuture<'a, NetdoxResult<()>>,
    },
}

#[async_trait]
pub trait PSPublisher {
    /// Adds a DNS record to relevant document given the changelog change value.
    async fn add_dns_record(&self, record: DNSRecords) -> NetdoxResult<()>;

    /// Updates the fragment with the metadata change from the change value.
    async fn update_metadata(&self, mut backend: DataStore, value: &str) -> NetdoxResult<()>;

    /// Creates the fragment with the data.
    async fn create_data(
        &self,
        mut backend: DataStore,
        obj_id: &str,
        data_id: &str,
        kind: &DataKind,
    ) -> NetdoxResult<()>;

    /// Updates the fragment with the data.
    async fn update_data(
        &self,
        mut backend: DataStore,
        obj_id: &str,
        data_id: &str,
        kind: &DataKind,
    ) -> NetdoxResult<()>;

    /// Uploads a set of PSML documents to the server.
    async fn upload_docs(&self, docs: Vec<Document>) -> NetdoxResult<()>;

    /// Returns publishable data for a change.
    async fn prep_data<'a>(
        &'a self,
        mut con: DataStore,
        change: &'a Change,
    ) -> NetdoxResult<Vec<PublishData>>;

    /// Prepares a set of futures that will apply the given changes.
    async fn prep_changes<'a>(
        &'a self,
        mut con: DataStore,
        changes: HashSet<&'a Change>,
    ) -> NetdoxResult<Vec<BoxFuture<NetdoxResult<()>>>>;

    /// Applies the given changes to the PageSeeder documents on the remote.
    /// Will attempt to update in place where possible.
    async fn apply_changes(
        &self,
        mut con: DataStore,
        changes: Vec<ChangelogEntry>,
    ) -> NetdoxResult<()>;
}

#[async_trait]
impl PSPublisher for PSRemote {
    async fn add_dns_record(&self, record: DNSRecords) -> NetdoxResult<()> {
        let docid = dns_qname_to_docid(record.name());
        let fragment = PropertiesFragment::from(record.clone());
        let section = match record {
            DNSRecords::Actual(_) => DNS_RECORD_SECTION,
            DNSRecords::Implied(_) => IMPLIED_RECORD_SECTION,
        };

        match xml_se::to_string_with_root("properties-fragment", &fragment) {
            Ok(content) => {
                match self
                    .server()
                    .await?
                    .add_uri_fragment(
                        &self.username,
                        &self.group,
                        &docid,
                        &content,
                        HashMap::from([("section", section), ("fragment", &fragment.id)]),
                    )
                    .await
                {
                    Err(PSError::ApiError(err)) => {
                        if err.message == "The fragment already exists." {
                            Ok(())
                        } else {
                            Err(PSError::ApiError(err).into())
                        }
                    }
                    Err(other_err) => Err(other_err.into()),
                    Ok(_) => Ok(()),
                }
            }
            Err(err) => {
                io_err!(format!(
                    "Failed to serialise DNS record to PSML: {}",
                    err.to_string()
                ))
            }
        }
    }

    /// Pushes new metadata to the remote.
    async fn update_metadata(&self, mut backend: DataStore, obj_id: &str) -> NetdoxResult<()> {
        let mut id_parts = obj_id.split(';');
        let (metadata, docid) = match id_parts.next() {
            Some(NODES_KEY) => {
                if let Some(proc_id) = backend
                    .get_node_from_raw(&id_parts.collect::<Vec<&str>>().join(";"))
                    .await?
                {
                    let node = backend.get_node(&proc_id).await?;
                    let metadata = backend.get_node_metadata(&node).await?;
                    (metadata, node_id_to_docid(&node.link_id))
                } else {
                    warn!("Wanted to publish changed metadata for unused raw node: {obj_id}");
                    return Ok(());
                }
            }
            Some(PROC_NODES_KEY) => {
                let node = backend
                    .get_node(&id_parts.collect::<Vec<&str>>().join(";"))
                    .await?;
                let metadata = backend.get_node_metadata(&node).await?;
                (metadata, node_id_to_docid(&node.link_id))
            }
            Some(DNS_KEY) => {
                let qname = &id_parts.collect::<Vec<&str>>().join(";");
                let metadata = backend.get_dns_metadata(qname).await?;
                (metadata, dns_qname_to_docid(qname))
            }
            _ => {
                return redis_err!(format!(
                    "Invalid updated metadata change object id (wrong first segment): {obj_id}"
                ))
            }
        };

        let fragment = metadata_fragment(metadata)
            .create_links(&mut backend)
            .await?;

        match xml_se::to_string_with_root("properties-fragment", &fragment) {
            Ok(content) => {
                self.server()
                    .await?
                    .put_uri_fragment(
                        &self.username,
                        &self.group,
                        &docid,
                        METADATA_FRAGMENT,
                        content,
                        None,
                    )
                    .await?;
            }
            Err(err) => {
                return io_err!(format!(
                    "Failed to serialise metadata to PSML: {}",
                    err.to_string()
                ))
            }
        }

        Ok(())
    }

    async fn create_data(
        &self,
        mut backend: DataStore,
        obj_id: &str,
        data_id: &str,
        kind: &DataKind,
    ) -> NetdoxResult<()> {
        let (data_key, section) = match kind {
            DataKind::Plugin => (format!("{PDATA_KEY};{obj_id};{data_id}"), PDATA_SECTION),
            DataKind::Report => (format!("{obj_id};{data_id}"), RDATA_SECTION),
        };
        let data = backend.get_data(&data_key).await?;

        let mut id_parts = obj_id.split(';');
        let docid = match id_parts.next() {
            Some(DNS_KEY) => dns_qname_to_docid(&id_parts.collect::<Vec<_>>().join(";")),

            Some(NODES_KEY) => {
                let raw_id = id_parts.collect::<Vec<&str>>().join(";");
                if let Some(id) = backend.get_node_from_raw(&raw_id).await? {
                    node_id_to_docid(&id)
                } else {
                    return process_err!(format!(
                        "Data not attached to any processed node was updated. Raw id: {raw_id}"
                    ));
                }
            }

            Some(REPORTS_KEY) => match id_parts.next() {
                Some(id) => report_id_to_docid(id),
                None => return redis_err!(format!("Invalid report data key: {obj_id}")),
            },
            _ => return redis_err!(format!("Invalid created data change value: {obj_id}")),
        };

        let fragment = Fragments::from(data).create_links(&mut backend).await?;
        let id = match &fragment {
            Fragments::Fragment(frag) => &frag.id,
            Fragments::Media(_frag) => todo!("Media fragment in pageseeder-rs"),
            Fragments::Properties(frag) => &frag.id,
            Fragments::Xref(frag) => &frag.id,
        };

        match xml_se::to_string(&fragment) {
            Ok(content) => {
                match self
                    .server()
                    .await?
                    .add_uri_fragment(
                        &self.username,
                        &self.group,
                        &docid,
                        &content,
                        HashMap::from([("section", section), ("fragment", id)]),
                    )
                    .await
                {
                    Err(PSError::ApiError(err)) => {
                        if err.message == "The fragment already exists." {
                            self.update_data(backend, obj_id, data_id, kind).await
                        } else {
                            Err(PSError::ApiError(err).into())
                        }
                    }
                    Err(other_err) => Err(other_err.into()),
                    Ok(_) => Ok(()),
                }
            }
            Err(err) => {
                io_err!(format!(
                    "Failed to serialise data to PSML: {}",
                    err.to_string()
                ))
            }
        }
    }

    async fn update_data(
        &self,
        mut backend: DataStore,
        obj_id: &str,
        data_id: &str,
        kind: &DataKind,
    ) -> NetdoxResult<()> {
        let data_key = match kind {
            DataKind::Plugin => format!("{PDATA_KEY};{obj_id};{data_id}"),
            DataKind::Report => format!("{obj_id};{data_id}"),
        };
        let data = backend.get_data(&data_key).await?;

        let mut id_parts = obj_id.split(';');
        let docid = match id_parts.next() {
            Some(DNS_KEY) => dns_qname_to_docid(&id_parts.collect::<Vec<_>>().join(";")),

            Some(NODES_KEY) => {
                let raw_id = id_parts.collect::<Vec<&str>>().join(";");
                if let Some(id) = backend.get_node_from_raw(&raw_id).await? {
                    node_id_to_docid(&id)
                } else {
                    return process_err!(format!(
                        "Data not attached to any processed node was updated. Raw id: {raw_id}"
                    ));
                }
            }

            Some(REPORTS_KEY) => match id_parts.next() {
                Some(id) => report_id_to_docid(id),
                None => return redis_err!(format!("Invalid report data key: {obj_id}")),
            },
            _ => return redis_err!(format!("Invalid updated data change value: {obj_id}")),
        };

        let fragment = Fragments::from(data).create_links(&mut backend).await?;
        let id = match &fragment {
            Fragments::Fragment(frag) => &frag.id,
            Fragments::Media(_frag) => todo!("Media fragment in pageseeder-rs"),
            Fragments::Properties(frag) => &frag.id,
            Fragments::Xref(frag) => &frag.id,
        };

        match xml_se::to_string(&fragment) {
            Ok(content) => {
                self.server()
                    .await?
                    .put_uri_fragment(&self.username, &self.group, &docid, id, content, None)
                    .await?;
            }
            Err(err) => {
                return io_err!(format!(
                    "Failed to serialise data to PSML: {}",
                    err.to_string()
                ))
            }
        }

        Ok(())
    }

    async fn upload_docs(&self, docs: Vec<Document>) -> NetdoxResult<()> {
        let mut log = Logger::new();
        let num_docs = docs.len();
        log.loading(format!("Zipping {num_docs} documents..."));

        let mut zip_file = vec![];
        let mut zip = ZipWriter::new(Cursor::new(&mut zip_file));
        for doc in docs {
            let filename = match &doc.doc_info {
                None => {
                    return process_err!(format!(
                        "Tried to upload PSML document with no documentinfo."
                    ))
                }
                Some(info) => {
                    match &info.uri {
                        None => {
                            return process_err!(format!(
                                "Tried to upload PSML document with no uri descriptor."
                            ))
                        }
                        Some(uri) => match &uri.docid {
                            None => {
                                return process_err!(format!(
                                    "Tried to upload PSML document with no docid."
                                ))
                            }
                            Some(docid) => {
                                if docid.len() > 100 {
                                    log.warn(format!("Had to skip uploading document with docid too long: {docid}"));
                                    continue;
                                }
                                let mut filename = String::from(docid);
                                filename.push_str(".psml");
                                filename
                            }
                        },
                    }
                }
            };

            if let Err(err) = zip.start_file(filename, Default::default()) {
                return io_err!(format!("Failed to start file in zip to upload: {err}"));
            }

            match quick_xml::se::to_string(&doc) {
                Ok(xml) => {
                    if let Err(err) = zip.write(&xml.into_bytes()) {
                        return io_err!(format!("Failed to write psml document into zip: {err}"));
                    }
                }
                Err(err) => {
                    return process_err!(format!("Failed to serialise psml document: {err}"))
                }
            }
        }

        if let Err(err) = zip.finish() {
            return io_err!(format!(
                "Failed to finished writing zip of psml documents: {err}"
            ));
        }
        drop(zip);

        std::fs::write("uploads.zip", &zip_file).unwrap();

        log.loading(format!("Uploading {num_docs} documents..."));

        self.server()
            .await?
            .upload(
                &self.group,
                "netdox.zip",
                zip_file,
                HashMap::from([("folder", "website")]),
            )
            .await?;

        log.loading(format!("Unzipping {num_docs} documents in loading zone..."));

        let unzip_thread = self
            .server()
            .await?
            .unzip_loading_zone(
                &self.username,
                &self.group,
                "netdox.zip",
                HashMap::from([("deleteoriginal", "true")]),
            )
            .await?
            .thread;

        self.await_thread(unzip_thread).await?;

        log.loading(format!("Loading {num_docs} documents into PageSeeder..."));

        let thread = self
            .server()
            .await?
            .start_loading(
                &self.username,
                &self.group,
                HashMap::from([("overwrite", "true"), ("folder", UPLOAD_DIR)]),
            )
            .await?
            .thread;

        self.await_thread(thread).await?;

        log.success(format!("Uploaded {num_docs} documents to PageSeeder."));

        Ok(())
    }

    async fn prep_data<'a>(
        &'a self,
        mut con: DataStore,
        change: &'a Change,
    ) -> NetdoxResult<Vec<PublishData<'a>>> {
        use Change as CT;
        use PublishData as PC;
        match change {
            CT::Init { .. } => Ok(vec![
                PC::Create {
                    target_ids: vec!["changelog".to_string()],
                    document: changelog_document(),
                },
                PC::Create {
                    target_ids: vec!["config".to_string()],
                    document: remote_config_document(),
                },
            ]),

            CT::CreateDnsName { qname, .. } => Ok(vec![PC::Create {
                target_ids: vec![format!("{DNS_KEY};{qname}")],
                document: dns_name_document(&mut con, qname).await?,
            }]),

            CT::CreateDnsRecord { record, .. } => {
                let mut updates = vec![PC::Update {
                    target_id: format!("{DNS_KEY};{}", record.name),
                    future: self.add_dns_record(DNSRecords::Actual(record.clone())),
                }];

                if let Some(implied) = record.implies() {
                    updates.push(PC::Update {
                        target_id: format!("{DNS_KEY};{}", implied.name),
                        future: self.add_dns_record(DNSRecords::Implied(implied.clone())),
                    });
                }

                Ok(updates)
            }

            CT::CreatePluginNode { node_id, .. } => match con.get_node_from_raw(node_id).await? {
                Some(pnode_id) => {
                    let node = con.get_node(&pnode_id).await?;
                    Ok(vec![PC::Create {
                        target_ids: node
                            .raw_ids
                            .iter()
                            .map(|id| format!("{NODES_KEY};{id}"))
                            .collect(),
                        document: processed_node_document(&mut con, &node).await?,
                    }])
                }
                None => {
                    redis_err!(format!(
                        "No processed node for created raw node: {}",
                        node_id
                    ))
                }
            },

            CT::UpdatedMetadata { obj_id, .. } => Ok(vec![PC::Update {
                target_id: obj_id.to_string(),
                future: self.update_metadata(con, obj_id),
            }]),

            CT::CreatedData {
                obj_id,
                data_id,
                kind,
                ..
            } => Ok(vec![PC::Update {
                target_id: obj_id.to_string(),
                future: self.create_data(con, obj_id, data_id, kind),
            }]),

            CT::UpdatedData {
                obj_id,
                data_id,
                kind,
                ..
            } => Ok(vec![PC::Update {
                target_id: obj_id.to_string(),
                future: self.update_data(con, obj_id, data_id, kind),
            }]),

            CT::CreateReport { report_id, .. } => Ok(vec![PC::Create {
                target_ids: vec![format!("{REPORTS_KEY};{report_id}")],
                document: report_document(&mut con, report_id).await?,
            }]),

            CT::UpdatedNetworkMapping { .. } => todo!("Update network mappings"),
        }
    }

    async fn prep_changes<'a>(
        &'a self,
        con: DataStore,
        changes: HashSet<&'a Change>,
    ) -> NetdoxResult<Vec<BoxFuture<NetdoxResult<()>>>> {
        let mut log = Logger::new();
        let num_changes = changes.len();

        // Fetch from redis

        log.loading(format!("Fetching data to prepare {num_changes} changes..."));
        let mut data_futures = vec![];
        for change in changes {
            data_futures.push(self.prep_data(con.clone(), change));
        }
        let data = join_all(data_futures).await;
        log.success("Fetched data from datastore.");

        // Upload and post changes

        log.loading(format!("Preparing {num_changes} changes..."));
        let mut uploads = vec![];
        let mut upload_ids = HashSet::new();
        let mut update_map: HashMap<String, Vec<BoxFuture<NetdoxResult<()>>>> = HashMap::new();
        for result in data {
            match result {
                Ok(_data) => {
                    for datum in _data {
                        match datum {
                            PublishData::Create {
                                target_ids,
                                document,
                            } => {
                                uploads.push(document);
                                upload_ids.extend(target_ids);
                            }
                            PublishData::Update { target_id, future } => {
                                match update_map.entry(target_id.to_string()) {
                                    Entry::Occupied(mut entry) => entry.get_mut().push(future),
                                    Entry::Vacant(entry) => {
                                        entry.insert(vec![future]);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    log.same()
                        .error("\r")
                        .error(format!("Failed to prepare change: {err}"));
                }
            }
        }
        log.success(format!("Prepared {num_changes} changes."));

        for id in upload_ids {
            // Remove updates to documents that will be uploaded
            update_map.remove(&id);
        }

        let mut updates = update_map.into_values().flatten().collect::<Vec<_>>();
        if !uploads.is_empty() {
            updates.push(self.upload_docs(uploads));
        }

        Ok(updates)
    }

    async fn apply_changes(
        &self,
        con: DataStore,
        changes: Vec<ChangelogEntry>,
    ) -> NetdoxResult<()> {
        let unique_changes = changes
            .iter()
            .map(|entry| &entry.change)
            .collect::<HashSet<_>>();

        let mut errs = vec![];
        for res in join_all(self.prep_changes(con.clone(), unique_changes).await?).await {
            if let Err(err) = res {
                errs.push(err);
            }
        }

        if !errs.is_empty() {
            return remote_err!(format!(
                "Some changes could not be published: \n\n\t{}",
                errs.into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join("\n\n\t")
            ));
        }

        if let Some(change) = changes.last() {
            let frag = last_change_fragment(change.id.clone());
            let xml = match quick_xml::se::to_string(&frag) {
                Ok(string) => string,
                Err(err) => {
                    return io_err!(format!("Failed to serialise changelog to PSML: {err}"))
                }
            };

            self.server()
                .await?
                .put_uri_fragment(
                    &self.username,
                    &self.group,
                    CHANGELOG_DOCID,
                    CHANGELOG_FRAGMENT,
                    xml,
                    None,
                )
                .await?;
        }

        Ok(())
    }
}

fn last_change_fragment(id: String) -> Fragments {
    Fragments::Fragment(
        Fragment::new(CHANGELOG_FRAGMENT.to_string()).with_content(vec![FragmentContent::Para(
            Para::new(vec![ParaContent::Text(id)]),
        )]),
    )
}
