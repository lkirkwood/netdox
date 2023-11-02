use std::{
    collections::HashMap,
    io::{Cursor, Write},
};

use crate::{
    data::{
        model::{Change, ChangeType, DNSRecord, DNS_KEY, NODES_KEY},
        DataClient, DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    io_err, process_err, redis_err, remote_err,
};

use super::{
    psml::{dns_name_document, metadata_fragment, processed_node_document, METADATA_FRAGMENT},
    remote::{dns_qname_to_docid, node_id_to_docid},
    PSRemote,
};
use async_trait::async_trait;
use futures::future::join_all;
use pageseeder::psml::model::{Document, Fragments, PropertiesFragment};
use paris::{error, info};
use quick_xml::se as xml_se;
use zip::ZipWriter;

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

    /// Updates the fragment with the plugin data change from the change value.
    async fn update_pdata(&self, mut backend: Box<dyn DataConn>, value: String)
        -> NetdoxResult<()>;

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
        let mut value_iter = value.split(';').into_iter().skip(1);
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
            .unwrap_or(&vec![])
        {
            if record.plugin == plugin {
                let fragment = PropertiesFragment::from(record);
                match xml_se::to_string(&fragment) {
                    Ok(content) => {
                        self.server()
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
        let mut val_iter = value.split(';').into_iter().skip(1);
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
        match xml_se::to_string(&fragment) {
            Ok(content) => {
                self.server()
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

    async fn update_metadata(
        &self,
        mut backend: Box<dyn DataConn>,
        value: String,
    ) -> NetdoxResult<()> {
        let mut val_iter = value.split(';').into_iter().skip(1);
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

    async fn update_pdata(&self, mut backend: Box<dyn DataConn>, key: String) -> NetdoxResult<()> {
        let pdata = backend.get_pdata(&key).await?;

        let mut key_iter = key.split(';').into_iter().skip(1);
        let docid = match key_iter.next() {
            Some(NODES_KEY) => {
                if let Some(id) = backend
                    .get_node_from_raw(&key_iter.collect::<Vec<&str>>().join(";"))
                    .await?
                {
                    node_id_to_docid(&id)
                } else {
                    todo!("Decide what to do here")
                }
            }
            Some(DNS_KEY) => key_iter.collect::<Vec<&str>>().join(";"),
            _ => return redis_err!(format!("Invalid updated plugin data change value: {key}")),
        };

        let fragment = Fragments::from(pdata);
        let id = match &fragment {
            Fragments::Fragment(frag) => &frag.id,
            Fragments::Media(_frag) => todo!("Media fragment in pageseeder-rs"),
            Fragments::Properties(frag) => &frag.id,
            Fragments::Xref(frag) => &frag.id,
        };

        match xml_se::to_string(&fragment) {
            Ok(content) => {
                self.server()
                    .put_uri_fragment(&self.username, &self.group, &docid, id, content, None)
                    .await?;
            }
            Err(err) => {
                return io_err!(format!(
                    "Failed to serialise plugin data to PSML: {}",
                    err.to_string()
                ))
            }
        }

        Ok(())
    }

    async fn upload_docs(&self, docs: Vec<Document>) -> NetdoxResult<()> {
        let mut zip_file = vec![];
        let mut zip = ZipWriter::new(Cursor::new(&mut zip_file));
        for doc in docs {
            let mut filename = String::from(
                doc.doc_info
                    .as_ref()
                    .unwrap()
                    .uri
                    .as_ref()
                    .unwrap()
                    .docid
                    .as_ref()
                    .unwrap(),
            );
            filename.push_str(".psml");

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

        std::fs::write("upload.zip", &zip_file).unwrap(); // TODO remove this debug line

        self.server()
            .upload(
                &self.group,
                "netdox.zip",
                zip_file,
                HashMap::from([("folder", "website")]),
            )
            .await?;
        self.server()
            .unzip_loading_zone(
                &self.username,
                &self.group,
                "website/netdox.zip",
                HashMap::new(),
            )
            .await?;

        Ok(())
    }

    async fn apply_changes(
        &self,
        client: &mut dyn DataClient,
        changes: Vec<Change>,
    ) -> NetdoxResult<()> {
        use ChangeType as CT;
        let mut con = client.get_con().await?;

        let mut uploads = vec![];
        let mut updates = vec![];
        for change in changes {
            match change.change {
                CT::CreateDnsName => {
                    uploads.push(dns_name_document(&mut con, &change.value).await?);
                }
                CT::CreatePluginNode => match con.get_node_from_raw(&change.value).await? {
                    None => {
                        // TODO decide what to do here
                        error!("No processed node for created raw node: {}", change.value);
                    }
                    Some(pnode_id) => {
                        // TODO implement diffing processed node doc
                        let node = con.get_node(&pnode_id).await?;
                        uploads.push(processed_node_document(&mut con, &node).await?);
                    }
                },
                CT::UpdatedMetadata => {
                    updates.push(self.update_metadata(client.get_con().await?, change.value));
                }
                CT::UpdatedPluginData => {
                    updates.push(self.update_pdata(client.get_con().await?, change.value));
                }
                CT::AddPluginToDnsName => {
                    updates.push(self.add_dns_plugin(client.get_con().await?, change.value));
                }
                CT::CreateDnsRecord => {
                    updates.push(self.add_dns_record(client.get_con().await?, change.value));
                }
                CT::UpdatedNetworkMapping => todo!("Update network mappings"),
            }
        }

        info!("Uploading documents to PageSeeder...");
        self.upload_docs(uploads).await?;

        info!("Applying updates to documents on PageSeeder...");
        let mut errs = vec![];
        for res in join_all(updates).await {
            if let Err(err) = res {
                errs.push(err);
            }
        }

        if errs.len() > 0 {
            return remote_err!(format!(
                "Some publishing jobs failed: {}",
                errs.into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            ));
        }

        Ok(())
    }
}
