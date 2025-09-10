use crate::{
    config::{IgnoreList, LocalConfig},
    data::{
        model::{
            ChangelogEntry, DNSRecord, Data, Node, RawNode, Report, CHANGELOG_KEY, DNS, DNS_KEY,
            METADATA_KEY, NETDOX_PLUGIN, NODES_KEY, PDATA_KEY, PROC_NODES_KEY, PROC_NODE_REVS_KEY,
            REPORTS_KEY,
        },
        store::DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    io_err, redis_err,
};
use async_trait::async_trait;
use itertools::izip;
use redis::{cmd, AsyncCommands, Value};

use std::{
    collections::{HashMap, HashSet},
    fs,
};

const DNS_METADATA_FN: &str = "netdox_create_dns_metadata";
const PROC_NODE_METADATA_FN: &str = "netdox_create_proc_node_metadata";

const LUA_FUNCTIONS: &str = include_str!("../../../functions.lua");

#[async_trait]
impl DataConn for redis::aio::MultiplexedConnection {
    async fn auth(&mut self, password: &str, username: &Option<String>) -> NetdoxResult<()> {
        let mut auth_cmd = redis::cmd("AUTH");
        if let Some(username) = username {
            auth_cmd.arg(username);
        }
        if let Err(err) = auth_cmd.arg(password).query_async::<_, ()>(self).await {
            return redis_err!(format!("Failed to authenticate with redis: {err}"));
        }

        Ok(())
    }

    async fn setup(&mut self, cfg: &LocalConfig) -> NetdoxResult<()> {
        let dns_ignore = match &cfg.dns_ignore {
            IgnoreList::Set(set) => set.clone(),
            IgnoreList::Path(path) => match fs::read_to_string(path) {
                Ok(str_list) => str_list.lines().map(|s| s.to_owned()).collect(),
                Err(err) => {
                    return io_err!(format!("Failed to read DNS ignorelist from {path}: {err}"))
                }
            },
        };

        redis::cmd("FUNCTION")
            .arg("LOAD")
            .arg("REPLACE")
            .arg(LUA_FUNCTIONS)
            .query_async::<_, ()>(self)
            .await?;

        if let Err(err) = cmd("FCALL")
            .arg("netdox_setup")
            .arg(1)
            .arg(&cfg.default_network)
            .arg(dns_ignore)
            .query_async::<_, ()>(self)
            .await
        {
            return redis_err!(format!("Failed to call Lua setup function: {err}"));
        }

        Ok(())
    }

    async fn init(&mut self) -> NetdoxResult<()> {
        if let Err(err) = cmd("FCALL")
            .arg("netdox_init")
            .arg(0)
            .query_async::<_, ()>(self)
            .await
        {
            return redis_err!(format!("Failed to call Lua init function: {err}"));
        }

        Ok(())
    }

    // DNS

    async fn get_dns(&mut self) -> NetdoxResult<DNS> {
        let mut dns = DNS::new();
        for qname in self.get_dns_names().await? {
            for record in self
                .smembers::<_, Vec<String>>(format!("{DNS_KEY};{qname}"))
                .await?
            {
                let mut rsplit = record.splitn(3, ';');
                let plugin = match rsplit.next() {
                    Some(val) => val.to_string(),
                    None => {
                        return redis_err!(format!(
                            "Invalid DNS record (no plugin) on qname {qname}"
                        ))
                    }
                };

                let rtype = match rsplit.next() {
                    Some(val) => val.to_string(),
                    None => {
                        return redis_err!(format!(
                            "Invalid DNS record (no rtype) on qname {qname}"
                        ))
                    }
                };

                let value = match rsplit.next() {
                    Some(val) => val.to_string(),
                    None => {
                        return redis_err!(format!(
                            "Invalid DNS record (no value) on qname {qname}"
                        ))
                    }
                };

                dns.add_record(DNSRecord {
                    name: qname.clone(),
                    value,
                    rtype,
                    plugin,
                });
            }

            dns.qnames.insert(qname);
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

    async fn qualify_dns_names(&mut self, names: &[&str]) -> NetdoxResult<Vec<String>> {
        let mut fcall = cmd("FCALL");
        fcall.arg("netdox_qualify_dns_names").arg(names.len());
        for name in names {
            fcall.arg(name);
        }

        match fcall.query_async(self).await {
            Ok(names) => Ok(names),
            Err(err) => redis_err!(format!("Failed to qualify DNS names: {err}")),
        }
    }

    // Nodes

    // TODO maybe refactor this to use ID instead of key?
    async fn get_raw_node(&mut self, key: &str) -> NetdoxResult<RawNode> {
        let mut components = key.rsplit(';');
        let dns_names = match (
            components.next(), // last component, index
            components,
        ) {
            (Some(_), remainder) => remainder
                .into_iter()
                .rev()
                .skip(1)
                .map(|s| s.to_string())
                .collect::<HashSet<String>>(),
            _ => return redis_err!(format!("Invalid node redis key: {key}")),
        };

        let mut details: HashMap<String, String> = match self.hgetall(key).await {
            Err(err) => return redis_err!(format!("Failed to get node details at {key}: {err}")),
            Ok(val) => val,
        };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!(format!("Node details at key {key} missing plugin field.")),
        };

        let name = details.get("name").cloned();

        let exclusive = match details.get("exclusive") {
            Some(val) => match val.as_str().parse::<bool>() {
                Ok(_val) => _val,
                Err(_) => {
                    return redis_err!(format!(
                        "Unable to parse boolean from exclusive value at {key}: {val}"
                    ))
                }
            },
            None => {
                return redis_err!(format!(
                    "Node details at key {key} missing exclusive field."
                ))
            }
        };

        Ok(RawNode {
            name,
            exclusive,
            link_id: details.remove("link_id"),
            dns_names,
            plugin,
        })
    }

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
                raw.push(self.get_raw_node(&format!("{redis_key};{index}")).await?)
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

        let alt_names: HashSet<String> = match self.smembers(format!("{key};alt_names")).await {
            Ok(names) => names,
            Err(err) => {
                return redis_err!(format!("Failed to get alt names for node '{id}': {err}"))
            }
        };

        let dns_names: HashSet<String> = match self.smembers(format!("{key};dns_names")).await {
            Ok(names) => names,
            Err(err) => {
                return redis_err!(format!("Failed to get dns names for node '{id}': {err}"))
            }
        };

        let plugins: HashSet<String> = match self.smembers(format!("{key};plugins")).await {
            Ok(names) => names,
            Err(err) => return redis_err!(format!("Failed to get plugins for node '{id}': {err}")),
        };

        let raw_ids: HashSet<String> = match self.smembers(format!("{key};raw_ids")).await {
            Ok(ids) => ids,
            Err(err) => {
                return redis_err!(format!("Failed to get raw keys for node '{id}': {err}"))
            }
        };

        Ok(Node {
            name,
            link_id: id.to_string(),
            alt_names,
            dns_names,
            plugins,
            raw_ids,
        })
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

    async fn get_raw_id_from_qnames(&mut self, qnames: &[&str]) -> NetdoxResult<String> {
        let mut qnames = self.qualify_dns_names(qnames).await?;
        qnames.sort();

        Ok(qnames.join(";"))
    }

    async fn put_node(&mut self, node: &Node) -> NetdoxResult<()> {
        let mut sorted_names: Vec<_> = node.dns_names.iter().map(|v| v.to_owned()).collect();
        sorted_names.sort();

        if let Err(err) = self.sadd::<_, _, u8>(PROC_NODES_KEY, &node.link_id).await {
            return redis_err!(format!(
                "Failed while adding link ID of resolved node to set: {err}"
            ));
        }

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
        } else if let Err(err) = self.del::<_, u8>(format!("{key};dns_names")).await {
            return redis_err!(format!(
                "Failed while clearing old dns names for resolved node: {err}"
            ));
        } else if let Err(err) = self
            .sadd::<_, _, u8>(format!("{key};dns_names"), &node.dns_names)
            .await
        {
            return redis_err!(format!(
                "Failed while setting dns names for resolved node: {err}"
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

    // Data

    async fn get_data(&mut self, key: &str) -> NetdoxResult<Data> {
        let id = match key.rsplit_once(';') {
            Some((_, id)) => id.to_string(),
            None => return redis_err!(format!("Failed to get plugin data id from key: {key}")),
        };

        let details: HashMap<String, String> = match self.hgetall(format!("{key};details")).await {
            Ok(map) => map,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get plugin data details for data at key {key}: {}",
                    err.to_string()
                ))
            }
        };

        match details.get("type") {
            Some(s) if s == "hash" => match (
                self.hgetall(key).await,
                self.lrange(format!("{key};order"), 0, -1).await,
            ) {
                (Ok(content), Ok(order)) => Data::from_hash(id, content, order, details),
                (Err(err), Ok(_)) => {
                    return redis_err!(format!(
                        "Failed to get content for hash plugin data at {key}: {}",
                        err.to_string()
                    ))
                }
                (_, Err(err)) => {
                    return redis_err!(format!(
                        "Failed to get order for hash plugin data at {key}: {}",
                        err.to_string()
                    ))
                }
            },
            Some(s) if s == "list" => {
                let names: Vec<String> = match self.lrange(format!("{key};names"), 0, -1).await {
                    Ok(content) => content,
                    Err(err) => {
                        return redis_err!(format!(
                            "Failed to get names for list plugin data at {key}: {}",
                            err.to_string()
                        ))
                    }
                };

                let titles: Vec<String> = match self.lrange(format!("{key};titles"), 0, -1).await {
                    Ok(content) => content,
                    Err(err) => {
                        return redis_err!(format!(
                            "Failed to get titles for list plugin data at {key}: {}",
                            err.to_string()
                        ))
                    }
                };

                let values: Vec<String> = match self.lrange(key, 0, -1).await {
                    Ok(content) => content,
                    Err(err) => {
                        return redis_err!(format!(
                            "Failed to get values for list plugin data at {key}: {}",
                            err.to_string()
                        ))
                    }
                };

                Data::from_list(id, izip!(names, titles, values).collect(), details)
            }
            Some(s) if s == "string" => match self.get(key).await {
                Ok(content) => Data::from_string(id, content, details),
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get content for string plugin data at {key}: {}",
                        err.to_string()
                    ))
                }
            },
            Some(s) if s == "table" => match self.lrange(key, 0, -1).await {
                Ok(content) => Data::from_table(id, content, details),
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get content for table plugin data at {key}: {}",
                        err.to_string()
                    ))
                }
            },
            other => {
                redis_err!(format!(
                    "Plugin data details for data at {key} had invalid type: {other:?}"
                ))
            }
        }
    }

    // Plugin Data

    async fn get_dns_pdata(&mut self, qname: &str) -> NetdoxResult<Vec<Data>> {
        let pdata_ids: HashSet<String> = match self
            .smembers(format!("{PDATA_KEY};{DNS_KEY};{qname}"))
            .await
        {
            Ok(set) => set,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get plugin data for dns obj: {}",
                    err.to_string()
                ))
            }
        };

        let mut dataset = vec![];
        for id in pdata_ids {
            dataset.push(
                self.get_data(&format!("{PDATA_KEY};{DNS_KEY};{qname};{id}"))
                    .await?,
            );
        }

        Ok(dataset)
    }

    async fn get_node_pdata(&mut self, node: &Node) -> NetdoxResult<Vec<Data>> {
        let mut dataset = vec![];
        for raw in &node.raw_ids {
            // TODO more consistent solution for building this key
            let pdata_ids: HashSet<String> = match self
                .smembers(format!("{PDATA_KEY};{NODES_KEY};{raw}"))
                .await
            {
                Ok(set) => set,
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get plugin data for raw node: {}",
                        err.to_string()
                    ))
                }
            };

            for id in pdata_ids {
                dataset.push(
                    self.get_data(&format!("{PDATA_KEY};{NODES_KEY};{raw};{id}"))
                        .await?,
                );
            }
        }

        let pdata_ids: HashSet<String> = match self
            .smembers(format!("{PDATA_KEY};{PROC_NODES_KEY};{}", node.link_id))
            .await
        {
            Ok(set) => set,
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get plugin data for proc node: {}",
                    err.to_string()
                ))
            }
        };

        for id in pdata_ids {
            dataset.push(
                self.get_data(&format!(
                    "{PDATA_KEY};{PROC_NODES_KEY};{};{id}",
                    node.link_id
                ))
                .await?,
            );
        }

        Ok(dataset)
    }

    // Reports

    async fn get_report(&mut self, id: &str) -> NetdoxResult<Report> {
        let details: HashMap<String, String> =
            match self.hgetall(format!("{REPORTS_KEY};{id}")).await {
                Ok(map) => map,
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get report with id {id}: {}",
                        err.to_string()
                    ))
                }
            };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!(format!("Failed to get plugin for report with id: {id}")),
        };

        let title = match details.get("title") {
            Some(title) => title.to_owned(),
            None => return redis_err!(format!("Failed to get title for report with id: {id}")),
        };

        let length = match details.get("length") {
            Some(length) => match length.parse::<usize>() {
                Ok(int) => int,
                Err(_err) => {
                    return redis_err!(format!(
                        "Failed to parse length {length} of report {id} as an int."
                    ))
                }
            },
            None => return redis_err!(format!("Failed to get length for report with id: {id}")),
        };

        let mut content = Vec::with_capacity(length);
        for i in 0..length {
            content.push(self.get_data(&format!("{REPORTS_KEY};{id};{i}")).await?);
        }

        Ok(Report {
            id: id.to_string(),
            title,
            plugin,
            content,
        })
    }

    async fn put_report(&mut self, id: &str, title: &str, length: usize) -> NetdoxResult<()> {
        cmd("FCALL")
            .arg("netdox_create_report")
            .arg(1)
            .arg(id)
            .arg(NETDOX_PLUGIN)
            .arg(title)
            .arg(length)
            .query_async::<_, ()>(self)
            .await?;

        Ok(())
    }

    async fn put_report_data(&mut self, id: &str, idx: usize, data: &Data) -> NetdoxResult<()> {
        let data_args = data.to_args();
        let plugin = data_args.first().unwrap();

        let mut fcall = cmd("FCALL");
        fcall
            .arg("netdox_create_report_data")
            .arg(1)
            .arg(id)
            .arg(plugin)
            .arg(idx);

        for arg in data_args.iter().skip(1) {
            fcall.arg(arg);
        }

        fcall.query_async::<_, ()>(self).await?;

        Ok(())
    }

    // Metadata

    async fn get_dns_metadata(&mut self, qname: &str) -> NetdoxResult<HashMap<String, String>> {
        match self
            .hgetall(format!("{METADATA_KEY};{DNS_KEY};{qname}"))
            .await
        {
            Ok(map) => Ok(map),
            Err(err) => redis_err!(format!(
                "Failed to get metadata for dns obj {qname}: {}",
                err.to_string()
            )),
        }
    }

    async fn put_dns_metadata(
        &mut self,
        qname: &str,
        plugin: &str,
        data: HashMap<&str, &str>,
    ) -> NetdoxResult<()> {
        let result = cmd("FCALL")
            .arg(DNS_METADATA_FN)
            .arg(1)
            .arg(qname)
            .arg(plugin)
            .arg(data.iter().collect::<Vec<_>>())
            .query_async(self)
            .await;

        match result {
            Ok(()) => Ok(()),
            Err(err) => redis_err!(format!("Failed to update dns metadata: {err}")),
        }
    }

    async fn get_proc_node_metadata(
        &mut self,
        node_id: &str,
    ) -> NetdoxResult<HashMap<String, String>> {
        match self
            .hgetall::<_, HashMap<String, String>>(format!(
                "{METADATA_KEY};{PROC_NODES_KEY};{node_id}"
            ))
            .await
        {
            Ok(map) => Ok(map),
            Err(err) => {
                redis_err!(format!(
                    "Failed to get metadata for proc node {}: {}",
                    node_id,
                    err.to_string()
                ))
            }
        }
    }

    async fn get_node_metadata(&mut self, node: &Node) -> NetdoxResult<HashMap<String, String>> {
        let mut meta = HashMap::new();
        for raw_id in &node.raw_ids {
            let raw_meta: HashMap<String, String> = match self
                .hgetall(format!("{METADATA_KEY};{NODES_KEY};{raw_id}"))
                .await
            {
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

        meta.extend(self.get_proc_node_metadata(&node.link_id).await?);

        Ok(meta)
    }

    async fn put_node_metadata(
        &mut self,
        node_id: &str,
        plugin: &str,
        data: HashMap<&str, &str>,
    ) -> NetdoxResult<()> {
        let result = cmd("FCALL")
            .arg(PROC_NODE_METADATA_FN)
            .arg(1)
            .arg(node_id)
            .arg(plugin)
            .arg(data.iter().collect::<Vec<_>>())
            .query_async(self)
            .await;

        match result {
            Ok(()) => Ok(()),
            Err(err) => redis_err!(format!("Failed to update node metadata: {err}")),
        }
    }

    // Changelog

    async fn get_changes(&mut self, start_id: Option<&str>) -> NetdoxResult<Vec<ChangelogEntry>> {
        let start = match start_id {
            Some(id) => format!("({id}"), // to make range exclusive
            None => "-".to_string(),
        };

        match self.xrange(CHANGELOG_KEY, &start, "+").await {
            Ok(changes) => Ok(changes),
            Err(err) => redis_err!(format!(
                "Failed to fetch changes from {} to present: {}",
                start_id.unwrap_or("start"),
                err.to_string()
            )),
        }
    }

    async fn last_change_id(&mut self) -> NetdoxResult<String> {
        match self.xrevrange_count(CHANGELOG_KEY, "+", "-", 1).await {
            Ok(Value::Bulk(changes)) => match changes.into_iter().next() {
                Some(Value::Bulk(change_details)) => match change_details.into_iter().next() {
                    Some(Value::Data(change_id_bytes)) => {
                        match String::from_utf8(change_id_bytes) {
                            Ok(change_id) => Ok(change_id),
                            Err(err) => {
                                redis_err!(format!("Failed to parse last change ID as utf8: {err}"))
                            }
                        }
                    }
                    Some(_) => {
                        redis_err!("Got unexpected response type from last change ID.".to_string())
                    }
                    None => {
                        redis_err!("Got empty object for last change.".to_string())
                    }
                },
                Some(_) => {
                    redis_err!("Got unexpected response type from last change.".to_string())
                }
                None => redis_err!(
                    "Found 0 changes in changelog when trying to get last one.".to_string()
                ),
            },
            Ok(_) => redis_err!("Got unexpected response type from last change query.".to_string()),
            Err(err) => redis_err!(format!(
                "Failed to fetch changes from start to present: {err}"
            )),
        }
    }

    // Persistence

    async fn write_save(&mut self) -> NetdoxResult<()> {
        Ok(redis::cmd("SAVE").query_async::<_, ()>(self).await?)
    }
}
