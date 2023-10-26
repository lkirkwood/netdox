use std::collections::HashMap;

use crate::{
    data::{
        model::{Change, ChangeType, DNS_KEY, NODES_KEY},
        DataClient, DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    io_err, redis_err,
};

use super::{
    psml::{dns_name_document, metadata_fragment, processed_node_document, METADATA_FRAGMENT},
    remote::{dns_qname_to_docid, node_id_to_docid},
    PSRemote,
};
use async_trait::async_trait;

#[async_trait]
pub trait PSPublisher {
    /// Updates the fragment with the metadata at key.
    async fn update_metadata(
        &self,
        mut backend: Box<dyn DataConn>,
        key: String,
    ) -> NetdoxResult<()>;

    /// Updates the fragment with the plugin data at key.
    async fn update_pdata(&self, mut backend: Box<dyn DataConn>, key: String) -> NetdoxResult<()>;

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
    async fn update_metadata(
        &self,
        mut backend: Box<dyn DataConn>,
        key: String,
    ) -> NetdoxResult<()> {
        let mut key_iter = key.split(';').into_iter().skip(1);
        let (metadata, docid) = match key_iter.next() {
            Some(NODES_KEY) => {
                let node = backend
                    .get_node(&key_iter.collect::<Vec<&str>>().join(";"))
                    .await?;
                let metadata = backend.get_node_metadata(&node).await?;
                (metadata, node_id_to_docid(&node.link_id))
            }
            Some(DNS_KEY) => {
                let qname = &key_iter.collect::<Vec<&str>>().join(";");
                let metadata = backend.get_dns_metadata(qname).await?;
                (metadata, dns_qname_to_docid(qname))
            }
            _ => return redis_err!(format!("Invalid updated metadata change key: {key}")),
        };

        match quick_xml::se::to_string(&metadata_fragment(metadata)) {
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
            _ => return redis_err!(format!("Invalid updated plugin data change key: {key}")),
        };

        todo!("Update the document on PS");

        Ok(())
    }

    async fn apply_changes(
        &self,
        client: &mut dyn DataClient,
        changes: Vec<Change>,
    ) -> NetdoxResult<()> {
        use ChangeType as CT;
        let mut con = client.get_con().await?;

        let mut uploads = HashMap::new();
        let mut updates = vec![];
        for change in changes {
            match change.change {
                CT::CreateDnsName => {
                    let doc = dns_name_document(&mut con, &change.value).await?;
                    uploads.insert(doc.docid().unwrap().to_string(), doc);
                }
                CT::CreatePluginNode => match con.get_node_from_raw(&change.value).await? {
                    None => {
                        // TODO decide what to do here
                    }
                    Some(pnode_id) => {
                        // TODO implement diffing processed node doc
                        let node = con.get_node(&pnode_id).await?;
                        let doc = processed_node_document(&mut con, &node).await?;
                        uploads.insert(doc.docid().unwrap().to_string(), doc);
                    }
                },
                CT::UpdatedMetadata => {
                    updates.push(self.update_metadata(client.get_con().await?, change.value));
                }
                CT::UpdatedPluginDataList
                | CT::UpdatedPluginDataMap
                | CT::UpdatedPluginDataString => {
                    todo!("Update plugin data")
                }
                CT::AddPluginToDnsName => todo!("Add plugin to dns name"),
                CT::AddRecordTypeToDnsName => todo!("Add record type to dns name"),
                CT::CreateDnsRecord => todo!("Create dns record"),
                CT::UpdatedNetworkMapping => todo!("Update network mappings"),
            }
        }

        Ok(())
    }
}
