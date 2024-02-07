use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    io::{Cursor, Write},
};

use crate::{
    data::{
        model::{Change, DNSRecords, DataKind, DNS_KEY, NODES_KEY, PDATA_KEY, REPORTS_KEY},
        DataClient, DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    io_err, process_err, redis_err, remote_err,
};

use super::{
    psml::{
        changelog_document, dns_name_document, links::LinkContent, metadata_fragment,
        processed_node_document, report_document, DNS_RECORD_SECTION, IMPLIED_RECORD_SECTION,
        METADATA_FRAGMENT, PDATA_SECTION, RDATA_SECTION,
    },
    remote::{
        dns_qname_to_docid, node_id_to_docid, report_id_to_docid, CHANGELOG_DOCID,
        CHANGELOG_FRAGMENT,
    },
    PSRemote,
};
use async_trait::async_trait;
use futures::future::{join_all, BoxFuture};
use paris::{success, Logger};
use psml::{
    model::{Document, Fragment, FragmentContent, Fragments, PropertiesFragment},
    text::{Para, ParaContent},
};
use quick_xml::se as xml_se;
use zip::ZipWriter;

const UPLOAD_DIR: &str = "netdox";

#[async_trait]
pub trait PSPublisher {
    /// Adds a DNS record to relevant document given the changelog change value.
    async fn add_dns_record(&self, record: DNSRecords) -> NetdoxResult<()>;

    /// Updates the fragment with the metadata change from the change value.
    async fn update_metadata(
        &self,
        mut backend: Box<dyn DataConn>,
        value: &str,
    ) -> NetdoxResult<()>;

    /// Creates the fragment with the data.
    async fn create_data(
        &self,
        mut backend: Box<dyn DataConn>,
        obj_id: &str,
        data_id: &str,
        kind: &DataKind,
    ) -> NetdoxResult<()>;

    /// Updates the fragment with the data.
    async fn update_data(
        &self,
        mut backend: Box<dyn DataConn>,
        obj_id: &str,
        data_id: &str,
        kind: &DataKind,
    ) -> NetdoxResult<()>;

    /// Uploads a set of PSML documents to the server.
    async fn upload_docs(&self, docs: Vec<Document>) -> NetdoxResult<()>;

    /// Prepares a set of futures that will apply the given changes.
    async fn prep_changes<'a>(
        &'a self,
        client: &mut dyn DataClient,
        changes: &'a [Change],
    ) -> NetdoxResult<Vec<BoxFuture<NetdoxResult<()>>>>;

    /// Applies the given changes to the PageSeeder documents on the remote.
    /// Will attempt to update in place where possible.
    async fn apply_changes(
        &self,
        client: &mut dyn DataClient,
        changes: Vec<Change>,
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
                self.server()
                    .await?
                    .add_uri_fragment(
                        &self.username,
                        &self.group,
                        &docid,
                        &content,
                        HashMap::from([("section", section), ("fragment", &fragment.id)]),
                    )
                    .await?;
            }
            Err(err) => {
                return io_err!(format!(
                    "Failed to serialise DNS record to PSML: {}",
                    err.to_string()
                ))
            }
        }

        Ok(())
    }

    /// Returns the ID of the object owning the metadata.
    async fn update_metadata(
        &self,
        mut backend: Box<dyn DataConn>,
        obj_id: &str,
    ) -> NetdoxResult<()> {
        let mut id_parts = obj_id.split(';');
        let (metadata, docid) = match id_parts.next() {
            Some(NODES_KEY) => {
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
        mut backend: Box<dyn DataConn>,
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
                self.server()
                    .await?
                    .add_uri_fragment(
                        &self.username,
                        &self.group,
                        &docid,
                        &content,
                        HashMap::from([("section", section), ("fragment", id)]),
                    )
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

    async fn update_data(
        &self,
        mut backend: Box<dyn DataConn>,
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

    async fn prep_changes<'a>(
        &'a self,
        client: &mut dyn DataClient,
        changes: &'a [Change],
    ) -> NetdoxResult<Vec<BoxFuture<NetdoxResult<()>>>> {
        use Change as CT;

        let mut con = client.get_con().await?;
        let mut log = Logger::new();
        let num_changes = changes.len();

        let mut uploads = vec![];
        let mut upload_ids = HashSet::new();
        let mut update_map: HashMap<String, Vec<BoxFuture<NetdoxResult<()>>>> = HashMap::new();
        for (num, change) in changes.iter().enumerate() {
            log.loading(format!("Prepared {num} of {num_changes} changes..."));

            match change {
                CT::Init { .. } => {
                    uploads.push(changelog_document());
                    // TODO upload remote config here aswell?
                }

                CT::CreateDnsName { qname, .. } => {
                    uploads.push(dns_name_document(&mut con, qname).await?);
                    upload_ids.insert(format!("{DNS_KEY};{qname}"));
                }

                CT::CreateDnsRecord { record, .. } => {
                    let future = self.add_dns_record(DNSRecords::Actual(record.clone()));

                    match update_map.entry(format!("{DNS_KEY};{}", record.name)) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }

                    if let Some(implied) = record.implies() {
                        let future = self.add_dns_record(DNSRecords::Implied(implied.clone()));

                        match update_map.entry(format!("{DNS_KEY};{}", implied.name)) {
                            Entry::Vacant(entry) => {
                                entry.insert(vec![future]);
                            }
                            Entry::Occupied(mut entry) => {
                                entry.get_mut().push(future);
                            }
                        }
                    }
                }

                CT::CreatePluginNode { node_id, .. } => match con.get_node_from_raw(node_id).await?
                {
                    Some(pnode_id) => {
                        let node = con.get_node(&pnode_id).await?;
                        uploads.push(processed_node_document(&mut con, &node).await?);
                        upload_ids
                            .extend(node.raw_ids.iter().map(|id| format!("{NODES_KEY};{id}")));
                    }
                    None => {
                        log.same().error("\r").error(format!(
                            "No processed node for created raw node: {}",
                            node_id
                        ));
                    }
                },

                CT::UpdatedMetadata { obj_id, .. } => {
                    let future = self.update_metadata(client.get_con().await?, obj_id);

                    match update_map.entry(obj_id.to_string()) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }
                }

                CT::CreatedData {
                    obj_id,
                    data_id,
                    kind,
                    ..
                } => {
                    let future = self.create_data(client.get_con().await?, obj_id, data_id, kind);

                    match update_map.entry(obj_id.to_string()) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }
                }

                CT::UpdatedData {
                    obj_id,
                    data_id,
                    kind,
                    ..
                } => {
                    let future = self.update_data(client.get_con().await?, obj_id, data_id, kind);

                    match update_map.entry(obj_id.to_string()) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }
                }

                CT::CreateReport { report_id, .. } => {
                    uploads.push(report_document(&mut con, report_id).await?);
                    upload_ids.insert(format!("{REPORTS_KEY};{report_id}"));
                }

                CT::UpdatedNetworkMapping { .. } => todo!("Update network mappings"),
            }
        }
        log.success(format!("Prepared all {num_changes} changes."));

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
        client: &mut dyn DataClient,
        changes: Vec<Change>,
    ) -> NetdoxResult<()> {
        let mut errs = vec![];
        for res in join_all(self.prep_changes(client, &changes).await?).await {
            if let Err(err) = res {
                errs.push(err);
            }
        }

        if !errs.is_empty() {
            return remote_err!(format!(
                "Some changes could not be published: {}",
                errs.into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            ));
        }

        if let Some(change) = changes.last() {
            let frag = last_change_fragment(change.id().to_string());
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

        success!("All changes published.");

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
