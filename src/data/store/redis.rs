use crate::{
    data::{
        model::{
            Absorb, Change, DNSRecord, Node, PluginData, RawNode, CHANGELOG_KEY, DNS, DNS_KEY,
            DNS_NODE_KEY, DNS_PDATA_KEY, NODES_KEY, NODE_PDATA_KEY, PROC_NODES_KEY,
            PROC_NODE_REVS_KEY,
        },
        store::{DataClient, DataConn},
    },
    error::{NetdoxError, NetdoxResult},
    redis_err,
};
use async_trait::async_trait;
use redis::{AsyncCommands, Client};

use std::collections::{HashMap, HashSet};

#[async_trait]
impl DataClient for Client {
    async fn get_con(&mut self) -> NetdoxResult<Box<dyn DataConn>> {
        match self.get_async_connection().await {
            Ok(con) => Ok(Box::new(con)),
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get connection to redis: {}",
                    err.to_string()
                ))
            }
        }
    }
}

#[async_trait]
impl DataConn for redis::aio::Connection {
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

    async fn get_node(&mut self, id: &str) -> NetdoxResult<Node> {
        let key = format!("{PROC_NODES_KEY};{id}");
        let name: String = match self.get(&key).await {
            Err(err) => {
                return redis_err!(format!(
                    "Error getting name of linkable node with id {id}: {err}"
                ))
            }
            Ok(val) => val,
        };

        let alt_names: HashSet<String> = self
            .smembers(format!("{key};alt_names"))
            .await
            .unwrap_or_else(|_| panic!("Failed to get alt names for node '{id}'."));
        let dns_names: HashSet<String> = self
            .smembers(format!("{key};dns_names"))
            .await
            .unwrap_or_else(|_| panic!("Failed to get dns names for node '{id}'."));
        let plugins: HashSet<String> = self
            .smembers(format!("{key};plugins"))
            .await
            .unwrap_or_else(|_| panic!("Failed to get plugins for node '{id}'."));
        let raw_ids: HashSet<String> = self
            .smembers(format!("{key};raw_ids"))
            .await
            .unwrap_or_else(|_| panic!("Failed to get raw keys for node '{id}'."));

        Ok(Node {
            name,
            link_id: id.to_string(),
            alt_names,
            dns_names,
            plugins,
            raw_ids,
        })
    }

    async fn get_nodes(&mut self) -> NetdoxResult<Vec<Node>> {
        let mut nodes = vec![];
        for id in self.get_node_ids().await? {
            nodes.push(self.get_node(&format!("{NODES_KEY};{id}")).await?);
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

    async fn put_node(&mut self, node: &Node) -> NetdoxResult<()> {
        let mut sorted_names: Vec<_> = node.dns_names.iter().map(|v| v.to_owned()).collect();
        sorted_names.sort();

        let key = format!("{PROC_NODES_KEY};{}", node.link_id);
        if let Err(err) = self.set::<_, _, String>(&key, &node.name).await {
            return redis_err!(format!(
                "Failed while setting name for resolved node: {err}"
            ));
        }

        if !node.alt_names.is_empty() {
            if let Err(err) = self
                .sadd::<_, _, u8>(format!("{key};alt_names"), &node.alt_names)
                .await
            {
                return redis_err!(format!(
                    "Failed while updating alt names for resolved node: {err}"
                ));
            }
        }

        if node.dns_names.is_empty() {
            return redis_err!(format!(
                "Cannot write node {} with no dns names.",
                node.name
            ));
        } else if let Err(err) = self
            .sadd::<_, _, u8>(format!("{key};dns_names"), &node.dns_names)
            .await
        {
            return redis_err!(format!(
                "Failed while updating dns names for resolved node: {err}"
            ));
        }

        for name in &node.dns_names {
            if let Err(err) = self
                .hset::<_, _, _, u8>("dns_nodes", name, &node.link_id)
                .await
            {
                return redis_err!(format!("Failed to set node for dns name: {err}"));
            }
        }

        if node.plugins.is_empty() {
            return redis_err!(format!(
                "Cannot write node {} with no source plugins",
                node.name
            ));
        } else if let Err(err) = self
            .sadd::<_, _, u8>(format!("{key};plugins"), &node.plugins)
            .await
        {
            return redis_err!(format!(
                "Failed while updating plugins for resolved node: {err}"
            ));
        }

        if node.raw_ids.is_empty() {
            return redis_err!(format!(
                "Cannot write node {} with no source raw ids",
                node.name
            ));
        } else if let Err(err) = self
            .sadd::<_, _, u8>(format!("{key};raw_ids"), &node.raw_ids)
            .await
        {
            return redis_err!(format!(
                "Failed while updating raw ids for resolved node: {err}"
            ));
        }

        for raw_id in &node.raw_ids {
            if let Err(err) = self
                .hset::<_, _, _, u8>(PROC_NODE_REVS_KEY.to_string(), raw_id, &node.link_id)
                .await
            {
                return redis_err!(format!(
                    "Failed to set reverse ptr for raw key {raw_id} to {}: {err}",
                    &node.link_id
                ));
            }
        }

        Ok(())
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
