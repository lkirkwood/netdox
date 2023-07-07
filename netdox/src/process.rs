use std::collections::{HashMap, HashSet};

use redis::{Client, Commands, Connection};

use crate::{
    error::{NetdoxError, NetdoxResult},
    redis_err,
};

const DNS_KEY: &str = "dns";
const NODES_KEY: &str = "nodes";
const DATA_DB: u8 = 0;
const PROC_DB: u8 = 1;

pub fn process(client: &mut Client) -> NetdoxResult<()> {
    let mut data_con = match client.get_connection() {
        Err(err) => return redis_err!(format!("Failed while connecting to redis: {err}")),
        Ok(_c) => _c,
    };
    let mut proc_con = match client.get_connection() {
        Err(err) => return redis_err!(format!("Failed while connecting to redis: {err}")),
        Ok(_c) => _c,
    };

    if let Err(err) = redis::cmd("SELECT")
        .arg(PROC_DB)
        .query::<String>(&mut proc_con)
    {
        return redis_err!(format!("Failed to select db {PROC_DB}: {err}"));
    }
    process_dns(&mut data_con, &mut proc_con)?;

    Ok(())
}

// DNS

struct DNS {
    records: HashMap<String, Vec<DNSRecord>>,
    net_translations: HashMap<String, HashSet<String>>,
}

impl DNS {
    /// Returns set of all records that this record resolves to/through.
    fn get_superset(&self, name: &str) -> HashSet<String> {
        self._get_superset(name, &mut HashSet::new())
        // TODO implement caching for this
    }

    /// Recursive function which implements get_superset.
    fn _get_superset(&self, name: &str, seen: &mut HashSet<String>) -> HashSet<String> {
        let mut superset = HashSet::from([name.to_owned()]);
        if seen.contains(name) {
            return superset;
        } else {
            seen.insert(name.to_owned());
        }

        if let Some(records) = self.records.get(name) {
            for record in records {
                superset.insert(record.value.to_owned());
                superset.extend(self._get_superset(&record.value, seen));
            }
        }

        if let Some(translations) = self.net_translations.get(name) {
            for translation in translations {
                superset.insert(translation.to_owned());
                superset.extend(self._get_superset(translation, seen));
            }
        }

        superset
    }
}

struct DNSRecord {
    name: String,
    value: String,
    rtype: String,
    plugin: String,
}

fn process_dns(data_con: &mut Connection, proc_con: &mut Connection) -> NetdoxResult<()> {
    let dns: HashSet<String> = match data_con.hgetall(DNS_KEY) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get set of dns names using key {DNS_KEY}: {err}"
            ))
        }
        Ok(_k) => _k,
    };

    for name in dns {
        process_dns_name(&name, data_con, proc_con)?;
    }

    Ok(())
}

fn process_dns_name(
    name: &str,
    data_con: &mut Connection,
    _proc_con: &mut Connection,
) -> NetdoxResult<()> {
    let plugins: HashSet<String> = match data_con.hgetall(format!("{DNS_KEY};{name};plugins")) {
        Err(err) => return redis_err!(format!("Failed to get plugins for dns name {name}: {err}")),
        Ok(_p) => _p,
    };

    for _plugin in plugins {}

    Ok(())
}

fn process_plugin_dns_name(
    name: &str,
    plugin: &str,
    data_con: &mut Connection,
    _proc_con: &mut Connection,
) -> NetdoxResult<Vec<DNSRecord>> {
    let mut records = vec![];
    let rtypes: HashSet<String> = match data_con.hgetall(format!("{DNS_KEY};{name};{plugin}")) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get record types from plugin {plugin} for dns name {name}: {err}"
            ))
        }
        Ok(_t) => _t,
    };

    for rtype in rtypes {
        let values: HashSet<String> = match data_con.hgetall(format!("{DNS_KEY};{name};{plugin};{rtype}")) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get {rtype} record values from plugin {plugin} for dns name {name}: {err}"
                ))
            },
            Ok(_v) => _v
        };
        for value in values {
            records.push(DNSRecord {
                name: name.to_owned(),
                value,
                rtype: rtype.to_owned(),
                plugin: plugin.to_owned(),
            })
        }
    }

    Ok(records)
}

// NODES

struct RawNode {
    name: String,
    redis_key: String,
    dns_names: HashSet<String>,
    link_id: Option<String>,
    exclusive: bool,
    plugin: String,
}

struct ResolvedNode {
    name: String,
    alt_names: HashSet<String>,
    redis_key: String,
    alt_keys: HashSet<String>,
    dns_names: HashSet<String>,
    link_id: String,
    plugins: HashSet<String>,
}

fn map_nodes<'a>(dns: &DNS, nodes: Vec<RawNode>) -> HashMap<Vec<String>, Vec<RawNode>> {
    let mut superset_map = HashMap::new();
    for node in nodes {
        let mut superset = node.dns_names.clone();
        if !node.exclusive {
            for name in &node.dns_names {
                superset.extend(dns.get_superset(name));
            }
        }

        let mut supervec = superset.into_iter().collect::<Vec<String>>();
        supervec.sort();
        if !superset_map.contains_key(&supervec) {
            superset_map.insert(supervec.clone(), Vec::new());
        }
        superset_map.get_mut(&supervec).unwrap().push(node);
    }

    superset_map
}

/// Consolidates raw nodes into resolved nodes.
fn merge_nodes(dns: &DNS, nodes: Vec<RawNode>) -> Vec<ResolvedNode> {
    let mut resolved = Vec::new();
    for (superset, nodes) in map_nodes(dns, nodes) {
        let mut linkable = None;
        let mut alt_names = HashSet::new();
        let mut alt_keys = HashSet::new();
        let mut plugins = HashSet::new();
        for node in nodes {
            plugins.insert(node.plugin.clone());
            if node.link_id.is_some() {
                if linkable.is_none() {
                    linkable = Some(node);
                } else {
                    // TODO review this behaviour
                    eprintln!(
                        "Nodes under superset {superset:?} have multiple link ids: {}, {}",
                        linkable.as_ref().unwrap().link_id.as_ref().unwrap(),
                        node.link_id.as_ref().unwrap()
                    );
                    break;
                }
            } else {
                alt_names.insert(node.name.clone());
                alt_keys.insert(node.redis_key.clone());
            }
        }

        if let Some(node) = linkable {
            resolved.push(ResolvedNode {
                name: node.name.clone(),
                alt_names,
                redis_key: node.redis_key.clone(),
                alt_keys,
                dns_names: superset.into_iter().collect(),
                link_id: node.link_id.clone().unwrap(),
                plugins,
            });
        }
    }

    resolved
}

fn process_nodes(data_con: &mut Connection, _proc_con: &mut Connection) -> NetdoxResult<()> {
    let nodes: HashSet<String> = match data_con.hgetall(NODES_KEY) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get set of nodes using key {NODES_KEY}: {err}"
            ))
        }
        Ok(val) => val,
    };

    for node in nodes {
        let redis_key = format!("{NODES_KEY};{node}");
        let plugins: HashSet<String> = match data_con.hgetall(format!("{redis_key};plugins")) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get plugins for node with key {redis_key}: {err}"
                ))
            }
            Ok(val) => val,
        };
        for _plugin in plugins {}
    }

    Ok(())
}
