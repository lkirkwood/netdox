pub mod redis;

use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

use crate::{
    data::model::{Change, Data, Node, RawNode, DNS},
    error::NetdoxResult,
};

use super::model::Report;

#[async_trait]
/// Interface for opening connections to a datastore.
/// Useful for giving each thread/future its own connection.
pub trait DataClient: Send {
    async fn get_con(&mut self) -> NetdoxResult<Box<dyn DataConn>>;
}

#[async_trait]
impl<T: DataClient> DataClient for Box<T> {
    async fn get_con(&mut self) -> NetdoxResult<Box<dyn DataConn>> {
        self.get_con().await
    }
}

#[async_trait]
/// A connection to a datastore.
pub trait DataConn: Send {
    // DNS

    /// Gets all DNS data.
    async fn get_dns(&mut self) -> NetdoxResult<DNS>;

    /// Gets all DNS names.
    async fn get_dns_names(&mut self) -> NetdoxResult<HashSet<String>>;

    /// Gets a DNS struct with only data for the given DNS name.
    async fn get_dns_name(&mut self, name: &str) -> NetdoxResult<DNS>;

    /// Gets a DNS struct with only data for the given DNS name from the given source plugin.
    async fn get_plugin_dns_name(&mut self, name: &str, plugin: &str) -> NetdoxResult<DNS>;

    /// Gets the ID of the processed node for a DNS object.
    async fn get_dns_node_id(&mut self, qname: &str) -> NetdoxResult<Option<String>>;

    /// Gets the default network name.
    async fn get_default_net(&mut self) -> NetdoxResult<String>;

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

    /// Gets the IDs of the raw nodes that make up a processed node.
    async fn get_raw_ids(&mut self, proc_id: &str) -> NetdoxResult<HashSet<String>>;

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

    /// Gets the metadata for a node.
    async fn get_node_metadata(&mut self, node: &Node) -> NetdoxResult<HashMap<String, String>>;

    // Changelog

    /// Gets all changes from log after a given change ID.
    async fn get_changes(&mut self, start: Option<&str>) -> NetdoxResult<Vec<Change>>;
}

// Box impl

#[async_trait]
impl<T: DataConn> DataConn for Box<T> {
    // DNS

    async fn get_dns(&mut self) -> NetdoxResult<DNS> {
        self.get_dns().await
    }

    async fn get_dns_names(&mut self) -> NetdoxResult<HashSet<String>> {
        self.get_dns_names().await
    }

    async fn get_dns_name(&mut self, name: &str) -> NetdoxResult<DNS> {
        self.get_dns_name(name).await
    }

    async fn get_plugin_dns_name(&mut self, name: &str, plugin: &str) -> NetdoxResult<DNS> {
        self.get_plugin_dns_name(name, plugin).await
    }

    async fn get_dns_node_id(&mut self, qname: &str) -> NetdoxResult<Option<String>> {
        self.get_dns_node_id(qname).await
    }

    async fn get_default_net(&mut self) -> NetdoxResult<String> {
        self.get_default_net().await
    }

    // Nodes

    async fn get_raw_node(&mut self, key: &str) -> NetdoxResult<RawNode> {
        self.get_raw_node(key).await
    }

    async fn get_raw_nodes(&mut self) -> NetdoxResult<Vec<RawNode>> {
        self.get_raw_nodes().await
    }

    async fn get_node(&mut self, id: &str) -> NetdoxResult<Node> {
        self.get_node(id).await
    }

    async fn get_nodes(&mut self) -> NetdoxResult<Vec<Node>> {
        self.get_nodes().await
    }

    async fn get_node_ids(&mut self) -> NetdoxResult<HashSet<String>> {
        self.get_node_ids().await
    }

    async fn get_node_from_raw(&mut self, raw_id: &str) -> NetdoxResult<Option<String>> {
        self.get_node_from_raw(raw_id).await
    }

    async fn get_raw_ids(&mut self, proc_id: &str) -> NetdoxResult<HashSet<String>> {
        self.get_raw_ids(proc_id).await
    }

    async fn put_node(&mut self, node: &Node) -> NetdoxResult<()> {
        self.put_node(node).await
    }

    // Plugin Data

    async fn get_data(&mut self, key: &str) -> NetdoxResult<Data> {
        self.get_data(key).await
    }

    async fn get_dns_pdata(&mut self, qname: &str) -> NetdoxResult<Vec<Data>> {
        self.get_dns_pdata(qname).await
    }

    async fn get_node_pdata(&mut self, node: &Node) -> NetdoxResult<Vec<Data>> {
        self.get_node_pdata(node).await
    }

    // Reports

    async fn get_report(&mut self, id: &str) -> NetdoxResult<Report> {
        self.get_report(id).await
    }

    // Metadata

    async fn get_dns_metadata(&mut self, qname: &str) -> NetdoxResult<HashMap<String, String>> {
        self.get_dns_metadata(qname).await
    }

    async fn get_node_metadata(&mut self, node: &Node) -> NetdoxResult<HashMap<String, String>> {
        self.get_node_metadata(node).await
    }

    // Changelog

    async fn get_changes(&mut self, start: Option<&str>) -> NetdoxResult<Vec<Change>> {
        self.get_changes(start).await
    }
}
