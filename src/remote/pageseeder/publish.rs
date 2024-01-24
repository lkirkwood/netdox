use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    io::{Cursor, Write},
};

use crate::{
    data::{
        model::{Change, ChangeType, DNSRecord, DNS_KEY, NODES_KEY, REPORTS_KEY},
        DataClient, DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    io_err, process_err, redis_err, remote_err,
};

use super::{
    psml::{
        dns_name_document, metadata_fragment, processed_node_document, report_document,
        METADATA_FRAGMENT,
    },
    remote::{dns_qname_to_docid, node_id_to_docid, CHANGELOG_DOCID, CHANGELOG_FRAGMENT},
    PSRemote,
};
use async_trait::async_trait;
use futures::future::{join_all, BoxFuture};
use pageseeder::psml::{
    model::{Document, Fragment, FragmentContent, Fragments, PropertiesFragment},
    text::{Para, ParaContent},
};
use paris::{error, info, success, Logger};
use quick_xml::se as xml_se;
use zip::ZipWriter;

const UPLOAD_DIR: &str = "netdox";

#[async_trait]
pub trait PSPublisher {
    /// Adds all records from the new plugin to the relevant document.
    async fn add_dns_plugin(
        &self,
        mut backend: Box<dyn DataConn>,
        value: String,
    ) -> NetdoxResult<()>;

    /// Adds a DNS record to relevant document given the changelog change value.
    /// Also adds an implied DNS record to the destination document if there is no equivalent record already,
    /// implied or otherwise.
    async fn add_dns_record(
        &self,
        mut backend: Box<dyn DataConn>,
        value: String,
    ) -> NetdoxResult<()>;

    /// Updates the fragment with the metadata change from the change value.
    async fn update_metadata(
        &self,
        mut backend: Box<dyn DataConn>,
        value: String,
    ) -> NetdoxResult<()>;

    /// Updates the fragment with the data change from the change value.
    async fn update_data(&self, mut backend: Box<dyn DataConn>, value: String) -> NetdoxResult<()>;

    /// Uploads a set of PSML documents to the server.
    async fn upload_docs(&self, docs: Vec<Document>) -> NetdoxResult<()>;

    /// Applies a series of changes to the PageSeeder documents on the remote.
    /// Will attempt to update in place where possible.
    async fn apply_changes(
        &self,
        client: &mut dyn DataClient,
        changes: Vec<Change>,
    ) -> NetdoxResult<()>;
}

#[async_trait]
impl PSPublisher for PSRemote {
    async fn add_dns_plugin(
        &self,
        mut backend: Box<dyn DataConn>,
        value: String,
    ) -> NetdoxResult<()> {
        let mut value_iter = value.split(';').skip(1);
        let (qname, plugin) = match value_iter.next() {
            Some(qname) => match value_iter.next() {
                Some(plugin) => (qname, plugin),
                None => {
                    return redis_err!(format!(
                        "Invalid add plugin to dns name change value (missing plugin): {value}"
                    ))
                }
            },
            None => {
                return redis_err!(format!(
                    "Invalid add plugin to dns name change value (missing qname): {value}"
                ))
            }
        };

        let docid = dns_qname_to_docid(qname);

        for record in backend
            .get_dns_name(qname)
            .await?
            .records
            .get(qname)
            .unwrap_or(&HashSet::new())
        {
            if record.plugin == plugin {
                let fragment = PropertiesFragment::from(record);
                match xml_se::to_string_with_root("properties-fragment", &fragment) {
                    Ok(content) => {
                        self.server()
                            .await?
                            .put_uri_fragment(
                                &self.username,
                                &self.group,
                                &docid,
                                &fragment.id,
                                content,
                                None,
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
            }
        }
        Ok(())
    }

    async fn add_dns_record(&self, _backend: Box<dyn DataConn>, value: String) -> NetdoxResult<()> {
        let mut val_iter = value.split(';').skip(1);
        let name = match val_iter.next() {
            Some(name) => name.to_string(),
            None => {
                return redis_err!(format!(
                    "Invalid created dns record change value (missing qname): {value}"
                ))
            }
        };

        let plugin = match val_iter.next() {
            Some(plugin) => plugin.to_string(),
            None => {
                return redis_err!(format!(
                    "Invalid created dns record change value (missing plugin): {value}"
                ))
            }
        };

        let rtype = match val_iter.next() {
            Some(rtype) => rtype.to_string(),
            None => {
                return redis_err!(format!(
                    "Invalid created dns record change value (missing rtype): {value}"
                ))
            }
        };

        let value = match val_iter.next() {
            Some(value) => value.to_string(),
            None => {
                return redis_err!(format!(
                    "Invalid created dns record change value (missing record value): {value}"
                ))
            }
        };

        let docid = dns_qname_to_docid(&name);
        let record = DNSRecord {
            name,
            value,
            rtype,
            plugin,
        };

        let fragment = PropertiesFragment::from(&record);
        match xml_se::to_string_with_root("properties-fragment", &fragment) {
            Ok(content) => {
                self.server()
                    .await?
                    .put_uri_fragment(
                        &self.username,
                        &self.group,
                        &docid,
                        &fragment.id,
                        content,
                        None,
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
        value: String,
    ) -> NetdoxResult<()> {
        let mut val_iter = value.split(';').skip(1);
        let (metadata, docid) = match val_iter.next() {
            Some(NODES_KEY) => {
                let node = backend
                    .get_node(&val_iter.collect::<Vec<&str>>().join(";"))
                    .await?;
                let metadata = backend.get_node_metadata(&node).await?;
                (metadata, node_id_to_docid(&node.link_id))
            }
            Some(DNS_KEY) => {
                let qname = &val_iter.collect::<Vec<&str>>().join(";");
                let metadata = backend.get_dns_metadata(qname).await?;
                (metadata, dns_qname_to_docid(qname))
            }
            _ => {
                return redis_err!(format!(
                    "Invalid updated metadata change value (wrong first segment): {value}"
                ))
            }
        };

        match xml_se::to_string(&metadata_fragment(metadata)) {
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

    async fn update_data(&self, mut backend: Box<dyn DataConn>, key: String) -> NetdoxResult<()> {
        let data = backend.get_data(&key).await?;

        let mut key_iter = key.split(';').skip(1);
        let docid = match key_iter.next() {
            Some(NODES_KEY) => {
                let raw_id = key_iter.collect::<Vec<&str>>().join(";");
                if let Some(id) = backend.get_node_from_raw(&raw_id).await? {
                    node_id_to_docid(&id)
                } else {
                    return process_err!(format!(
                        "Data not attached to any processed node was updated. Raw id: {raw_id}"
                    ));
                }
            }
            Some(DNS_KEY) => key_iter.collect::<Vec<&str>>().join(";"),
            Some(REPORTS_KEY) => match key_iter.next() {
                Some(id) => id.to_string(),
                None => return redis_err!(format!("Invalid report data key: {key}")),
            },
            _ => return redis_err!(format!("Invalid updated data change value: {key}")),
        };

        let fragment = Fragments::from(data);
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

    async fn apply_changes(
        &self,
        client: &mut dyn DataClient,
        changes: Vec<Change>,
    ) -> NetdoxResult<()> {
        use ChangeType as CT;
        let mut log = Logger::new();
        let mut con = client.get_con().await?;

        let num_changes = changes.len();
        let mut uploads = vec![];
        let mut upload_ids = HashSet::new();
        let mut update_map: HashMap<String, Vec<BoxFuture<NetdoxResult<()>>>> = HashMap::new();
        for (num, change) in changes.iter().enumerate() {
            log.loading(format!("Prepared {num} of {num_changes} changes..."));

            let target_id = change.target_id()?;
            if upload_ids.contains(&target_id) {
                continue;
            }

            match change.change {
                CT::CreateDnsName => {
                    uploads.push(dns_name_document(&mut con, &change.value).await?);
                    upload_ids.insert(target_id);
                }
                CT::CreatePluginNode => match con.get_node_from_raw(&change.value).await? {
                    None => {
                        error!("No processed node for created raw node: {}", &change.value);
                    }
                    Some(pnode_id) => {
                        let node = con.get_node(&pnode_id).await?;
                        uploads.push(processed_node_document(&mut con, &node).await?);
                        upload_ids.insert(node.link_id);
                        upload_ids.extend(node.raw_ids);
                    }
                },
                CT::UpdatedMetadata => {
                    let future =
                        self.update_metadata(client.get_con().await?, change.value.clone());

                    match update_map.entry(target_id) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }
                }
                CT::UpdatedData => {
                    let future = self.update_data(client.get_con().await?, change.value.clone());

                    match update_map.entry(target_id) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }
                }
                CT::CreateDnsRecord => {
                    let future = self.add_dns_record(client.get_con().await?, change.value.clone());

                    match update_map.entry(target_id) {
                        Entry::Vacant(entry) => {
                            entry.insert(vec![future]);
                        }
                        Entry::Occupied(mut entry) => {
                            entry.get_mut().push(future);
                        }
                    }
                }
                CT::CreateReport => {
                    uploads.push(report_document(&mut con, &change.value).await?);
                    upload_ids.insert(change.value.clone());
                }
                CT::UpdatedNetworkMapping => todo!("Update network mappings"),
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

        info!("Publishing changes to PageSeeder...");
        let mut errs = vec![];
        for res in join_all(updates).await {
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

        if let Some(change) = changes.into_iter().last() {
            let frag = last_change_fragment(change.id);
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
