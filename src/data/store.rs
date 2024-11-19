pub mod redis_store;

use async_trait::async_trait;
use enum_dispatch::enum_dispatch;
use std::collections::{HashMap, HashSet};

use crate::{
    data::model::{Data, Node, RawNode, DNS},
    error::NetdoxResult,
};

use super::model::{ChangelogEntry, Report};

#[async_trait]
#[enum_dispatch]
/// A connection to a datastore.
pub trait DataConn: Send + Clone {
    async fn auth(&mut self, password: &str, username: &Option<String>) -> NetdoxResult<()>;

    // DNS

    /// Gets all DNS data.
    async fn get_dns(&mut self) -> NetdoxResult<DNS>;

    /// Gets all DNS names.
    async fn get_dns_names(&mut self) -> NetdoxResult<HashSet<String>>;

    /// Gets the ID of the processed node for a DNS object.
    async fn get_dns_node_id(&mut self, qname: &str) -> NetdoxResult<Option<String>>;

    /// Gets the default network name.
    async fn get_default_net(&mut self) -> NetdoxResult<String>;

    /// Qualifies some DNS names if they are not already.
    async fn qualify_dns_names(&mut self, names: &[&str]) -> NetdoxResult<Vec<String>>;

    // Nodes

    /// Gets a raw node from its redis key.
    async fn get_raw_node(&mut self, key: &str) -> NetdoxResult<RawNode>;

    /// Gets raw nodes from unprocessed data layer.
    async fn get_raw_nodes(&mut self) -> NetdoxResult<Vec<RawNode>>;

    /// Gets a process node from the processed data layer.
    async fn get_node(&mut self, id: &str) -> NetdoxResult<Node>;

    /// Gets nodes from the processed data layer.
    async fn get_nodes(&mut self) -> NetdoxResult<Vec<Node>>;

    /// Gets all node IDs from the processed data layer.
    async fn get_node_ids(&mut self) -> NetdoxResult<HashSet<String>>;

    /// Gets the ID of the processed node that a raw node was consumed by.
    async fn get_node_from_raw(&mut self, raw_id: &str) -> NetdoxResult<Option<String>>;

    /// Builds the ID of a raw node from the given qnames.
    async fn get_raw_id_from_qnames(&mut self, qnames: &[&str]) -> NetdoxResult<String>;

    /// Gets the IDs of the raw nodes that make up a processed node.
    async fn get_raw_ids(&mut self, proc_id: &str) -> NetdoxResult<HashSet<String>>;

    /// Puts a processed node into the data store.
    async fn put_node(&mut self, node: &Node) -> NetdoxResult<()>;

    // Plugin Data

    /// Gets the plugin data at a given key.
    async fn get_data(&mut self, key: &str) -> NetdoxResult<Data>;

    /// Gets all plugin data for a DNS object.
    async fn get_dns_pdata(&mut self, qname: &str) -> NetdoxResult<Vec<Data>>;

    /// Gets all plugin data for a node.
    async fn get_node_pdata(&mut self, node: &Node) -> NetdoxResult<Vec<Data>>;

    // Reports

    /// Gets a report.
    async fn get_report(&mut self, id: &str) -> NetdoxResult<Report>;

    // Metadata

    /// Gets the metadata for a DNS object.
    async fn get_dns_metadata(&mut self, qname: &str) -> NetdoxResult<HashMap<String, String>>;

    /// Adds some metadata to a DNS object.
    async fn put_dns_metadata(
        &mut self,
        qname: &str,
        plugin: &str,
        data: HashMap<&str, &str>,
    ) -> NetdoxResult<()>;

    /// Gets the metadata for a node.
    async fn get_node_metadata(&mut self, node: &Node) -> NetdoxResult<HashMap<String, String>>;

    /// Adds some metadata to a node.
    async fn put_node_metadata(
        &mut self,
        node: &Node,
        plugin: &str,
        data: HashMap<&str, &str>,
    ) -> NetdoxResult<()>;

    // Changelog

    /// Gets all changes from log after a given change ID.
    async fn get_changes(&mut self, start: Option<&str>) -> NetdoxResult<Vec<ChangelogEntry>>;

    // Persistence

    /// Writes a save of the datastore to ensure persistence.
    async fn write_save(&mut self) -> NetdoxResult<()>;
}

#[derive(Clone)]
#[enum_dispatch(DataConn)]
pub enum DataStore {
    Redis(redis::aio::MultiplexedConnection),
}
