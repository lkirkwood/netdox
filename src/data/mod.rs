pub mod model;
#[cfg(test)]
mod tests;

use async_trait::async_trait;
use redis::AsyncCommands;
use std::collections::{HashMap, HashSet};

use model::{Absorb, DNSRecord, DNS, DNS_KEY, DNS_PDATA_KEY};

use crate::{
    error::{NetdoxError, NetdoxResult},
    redis_err,
};

use self::model::{
    Change, Node, PluginData, RawNode, CHANGELOG_KEY, DNS_NODE_KEY, NODES_KEY, NODE_PDATA_KEY,
    PROC_NODES_KEY, PROC_NODE_REVS_KEY,
};

#[async_trait]
/// Interface for backend datastore.
pub trait Datastore: Send {
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

    // Nodes

    /// Gets raw nodes from unprocessed data layer.
    async fn get_raw_nodes(&mut self) -> NetdoxResult<Vec<RawNode>>;

    /// Gets nodes from the processed data layer.
    async fn get_nodes(&mut self) -> NetdoxResult<Vec<Node>>;

    /// Gets all node IDs from the processed data layer.
    async fn get_node_ids(&mut self) -> NetdoxResult<HashSet<String>>;

    /// Gets the ID of the processed node that a raw node was consumed by.
    async fn get_node_from_raw(&mut self, raw_id: &str) -> NetdoxResult<Option<String>>;

    /// Gets the IDs of the raw nodes that make up a processed node.
    async fn get_raw_ids(&mut self, proc_id: &str) -> NetdoxResult<HashSet<String>>;

    // Plugin Data

    /// Gets all plugin data for a DNS object.
    async fn get_dns_pdata(&mut self, qname: &str) -> NetdoxResult<Vec<PluginData>>;

    /// Gets all plugin data for a node.
    async fn get_node_pdata(&mut self, node: &Node) -> NetdoxResult<Vec<PluginData>>;

    // Metadata

    /// Gets the metadata for a DNS object.
    async fn get_dns_metadata(&mut self, qname: &str) -> NetdoxResult<HashMap<String, String>>;

    /// Gets the metadata for a node.
    async fn get_node_metadata(&mut self, node: &Node) -> NetdoxResult<HashMap<String, String>>;

    // Changelog

    /// Gets all changes from log after a given change ID.
    async fn get_changes(&mut self, start: &str) -> NetdoxResult<Vec<Change>>;
}

#[async_trait]
impl Datastore for redis::aio::Connection {
    // DNS

    async fn get_dns(&mut self) -> NetdoxResult<DNS> {
        let mut dns = DNS::new();
        for name in self.get_dns_names().await? {
            dns.absorb(self.get_dns_name(&name).await?)?;
        }

        Ok(dns)
    }

    async fn get_dns_names(&mut self) -> NetdoxResult<HashSet<String>> {
        match self.smembers(DNS_KEY).await {
            Err(err) => {
                redis_err!(format!(
                    "Failed to get set of dns names using key {DNS_KEY}: {err}"
                ))
            }
            Ok(dns) => Ok(dns),
        }
    }

    async fn get_dns_name(&mut self, name: &str) -> NetdoxResult<DNS> {
        let plugins: HashSet<String> =
            match self.smembers(format!("{DNS_KEY};{name};plugins")).await {
                Err(err) => {
                    return redis_err!(format!("Failed to get plugins for dns name {name}: {err}"))
                }
                Ok(_p) => _p,
            };

        let mut dns = DNS::new();
        for plugin in plugins {
            dns.absorb(self.get_plugin_dns_name(name, &plugin).await?)?;
        }

        let translations: HashSet<String> =
            match self.smembers(format!("{DNS_KEY};{name};maps")).await {
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get network translations for dns name {name}: {err}"
                    ))
                }
                Ok(_t) => _t,
            };

        for tran in translations {
            dns.add_net_translation(name, tran);
        }

        Ok(dns)
    }

    async fn get_plugin_dns_name(&mut self, name: &str, plugin: &str) -> NetdoxResult<DNS> {
        let mut dns = DNS::new();
        let rtypes: HashSet<String> =
            match self.smembers(format!("{DNS_KEY};{name};{plugin}")).await {
                Err(err) => {
                    return redis_err!(format!(
                    "Failed to get record types from plugin {plugin} for dns name {name}: {err}"
                ))
                }
                Ok(_t) => _t,
            };

        for rtype in rtypes {
            let values: HashSet<String> = match self.smembers(format!("{DNS_KEY};{name};{plugin};{rtype}")).await {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get {rtype} record values from plugin {plugin} for dns name {name}: {err}"
                ))
            },
            Ok(_v) => _v
        };
            for value in values {
                dns.add_record(DNSRecord {
                    name: name.to_owned(),
                    value,
                    rtype: rtype.to_owned(),
                    plugin: plugin.to_owned(),
                })
            }
        }

        Ok(dns)
    }

    async fn get_dns_node_id(&mut self, qname: &str) -> NetdoxResult<Option<String>> {
        match self.hget(DNS_NODE_KEY, qname).await {
            Ok(id) => Ok(id),
            Err(err) => redis_err!(format!(
                "Failed to get node id for dns obj {qname}: {}",
                err.to_string()
            )),
        }
    }

    // Nodes

    async fn get_raw_nodes(&mut self) -> NetdoxResult<Vec<RawNode>> {
        let nodes: HashSet<String> = match self.smembers(NODES_KEY).await {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get set of nodes using key {NODES_KEY}: {err}"
                ))
            }
            Ok(val) => val,
        };

        let mut raw = vec![];
        for node in nodes {
            let redis_key = format!("{NODES_KEY};{node}");
            let count: u64 = match self.get(&redis_key).await {
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get number of nodes with key {redis_key}: {err}"
                    ))
                }
                Ok(val) => val,
            };

            for index in 1..=count {
                raw.push(RawNode::read(self, &format!("{redis_key};{index}")).await?)
            }
        }

        Ok(raw)
    }

    async fn get_nodes(&mut self) -> NetdoxResult<Vec<Node>> {
        let mut nodes = vec![];
        for id in self.get_node_ids().await? {
            nodes.push(Node::read(self, &format!("{NODES_KEY};{id}")).await?);
        }

        Ok(nodes)
    }

    async fn get_node_ids(&mut self) -> NetdoxResult<HashSet<String>> {
        match self.smembers(NODES_KEY).await {
            Ok(set) => Ok(set),
            Err(err) => {
                redis_err!(format!(
                    "Failed to get node IDs from proc db: {}",
                    err.to_string()
                ))
            }
        }
    }

    async fn get_node_from_raw(&mut self, raw_id: &str) -> NetdoxResult<Option<String>> {
        match self.hget(PROC_NODE_REVS_KEY, raw_id).await {
            Ok(id) => Ok(id),
            Err(err) => redis_err!(format!(
                "Failed to get proc node for raw node {raw_id}: {}",
                err.to_string()
            )),
        }
    }

    async fn get_raw_ids(&mut self, proc_id: &str) -> NetdoxResult<HashSet<String>> {
        match self
            .smembers(format!("{PROC_NODES_KEY};{proc_id};raw_ids"))
            .await
        {
            Ok(ids) => Ok(ids),
            Err(err) => redis_err!(format!(
                "Failed to get raw ids for proc node {proc_id}: {}",
                err.to_string()
            )),
        }
    }

    // Plugin Data

    async fn get_dns_pdata(&mut self, qname: &str) -> NetdoxResult<Vec<PluginData>> {
        let mut dataset = vec![];
        let pdata_ids: HashSet<String> =
            match self.smembers(&format!("{DNS_PDATA_KEY};{}", qname)).await {
                Ok(set) => set,
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get plugin data for dns obj: {}",
                        err.to_string()
                    ))
                }
            };
        for id in pdata_ids {
            dataset.push(PluginData::read(self, &id).await?);
        }

        Ok(dataset)
    }
    async fn get_node_pdata(&mut self, node: &Node) -> NetdoxResult<Vec<PluginData>> {
        let mut dataset = vec![];
        for raw in &node.raw_ids {
            // TODO more consistent solution for building this key
            let pdata_ids: HashSet<String> =
                match self.smembers(&format!("{NODE_PDATA_KEY};{}", raw)).await {
                    Ok(set) => set,
                    Err(err) => {
                        return redis_err!(format!(
                            "Failed to get plugin data for raw node: {}",
                            err.to_string()
                        ))
                    }
                };

            for id in pdata_ids {
                dataset.push(PluginData::read(self, &id).await?);
            }
        }

        Ok(dataset)
    }

    // Metadata

    async fn get_dns_metadata(&mut self, qname: &str) -> NetdoxResult<HashMap<String, String>> {
        match self.hgetall(format!("meta;{qname}")).await {
            Ok(map) => Ok(map),
            Err(err) => redis_err!(format!(
                "Failed to get metadata for dns obj {qname}: {}",
                err.to_string()
            )),
        }
    }

    async fn get_node_metadata(&mut self, node: &Node) -> NetdoxResult<HashMap<String, String>> {
        let mut meta = HashMap::new();
        for raw_id in &node.raw_ids {
            let raw_meta: HashMap<String, String> =
                match self.hgetall(format!("meta;{raw_id}")).await {
                    Ok(map) => map,
                    Err(err) => {
                        return redis_err!(format!(
                            "Failed to get metadata for raw node {raw_id}: {}",
                            err.to_string()
                        ))
                    }
                };
            meta.extend(raw_meta);
        }
        Ok(meta)
    }

    // Changelog

    async fn get_changes(&mut self, start: &str) -> NetdoxResult<Vec<Change>> {
        match self.xrange(CHANGELOG_KEY, start, -1).await {
            Ok(changes) => Ok(changes),
            Err(err) => redis_err!(format!(
                "Failed to fetch changes from {start} to present: {}",
                err.to_string()
            )),
        }
    }
}
