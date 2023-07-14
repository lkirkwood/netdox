use std::collections::{HashMap, HashSet};

use redis::{Client, Commands, Connection};

use crate::{
    error::{NetdoxError, NetdoxResult},
    redis_err,
};

const DNS_KEY: &str = "dns";
const NODES_KEY: &str = "nodes";
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
    let dns = fetch_dns(&mut data_con)?;
    let raw_nodes = fetch_raw_nodes(&mut data_con)?;
    for node in resolve_nodes(&dns, raw_nodes) {
        println!("{node:?}");
        node.write(&mut proc_con)?;
    }

    Ok(())
}

// DNS

#[allow(clippy::upper_case_acronyms)]
struct DNS {
    pub records: HashMap<String, Vec<DNSRecord>>,
    pub net_translations: HashMap<String, HashSet<String>>,
}

impl DNS {
    fn new() -> Self {
        DNS {
            records: HashMap::new(),
            net_translations: HashMap::new(),
        }
    }

    /// Updates this DNS in place with content from another DNS.
    fn add_dns(&mut self, other: DNS) {
        self.records.extend(other.records);
        self.net_translations.extend(other.net_translations);
    }

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

#[derive(PartialEq, Eq, Hash)]
struct DNSRecord {
    name: String,
    value: String,
    rtype: String,
    plugin: String,
}

fn fetch_dns(data_con: &mut Connection) -> NetdoxResult<DNS> {
    let dns_names: HashSet<String> = match data_con.hgetall(DNS_KEY) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get set of dns names using key {DNS_KEY}: {err}"
            ))
        }
        Ok(_k) => _k,
    };

    let mut dns = DNS::new();
    for name in dns_names {
        dns.add_dns(fetch_dns_name(&name, data_con)?);
    }

    Ok(dns)
}

fn fetch_dns_name(name: &str, data_con: &mut Connection) -> NetdoxResult<DNS> {
    let plugins: HashSet<String> = match data_con.hgetall(format!("{DNS_KEY};{name};plugins")) {
        Err(err) => return redis_err!(format!("Failed to get plugins for dns name {name}: {err}")),
        Ok(_p) => _p,
    };

    let mut records = HashMap::new();
    for plugin in plugins {
        records.extend(fetch_plugin_dns_name(name, &plugin, data_con)?.records)
    }

    let translations: HashSet<String> = match data_con.hgetall(format!("{DNS_KEY};{name};maps")) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get network translations for dns name {name}: {err}"
            ))
        }
        Ok(_t) => _t,
    };

    Ok(DNS {
        records,
        net_translations: HashMap::from([(name.to_owned(), translations)]),
    })
}

fn fetch_plugin_dns_name(name: &str, plugin: &str, data_con: &mut Connection) -> NetdoxResult<DNS> {
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

    Ok(DNS {
        records: HashMap::from([(name.to_owned(), records)]),
        net_translations: HashMap::new(),
    })
}

// RAW NODES

/// An unprocessed node.
struct RawNode {
    name: String,
    dns_names: HashSet<String>,
    link_id: Option<String>,
    exclusive: bool,
    plugin: String,
}

/// Contructs a raw node from the details stored under the provided key.
fn construct_raw_node(key: &str, con: &mut Connection) -> NetdoxResult<RawNode> {
    let (generic_key, plugin) = match key.rsplit_once(';') {
        None => return redis_err!(format!("Invalid node redis key: {key}")),
        Some(val) => val,
    };
    let mut details: HashMap<String, String> = match con.hgetall(format!("{key};{plugin}")) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get node details at {key};{plugin}: {err}"
            ))
        }
        Ok(val) => val,
    };
    let name = match details.get("name") {
        Some(val) => val,
        None => {
            return redis_err!(format!(
                "Node details at key {key};{plugin} missing name field."
            ))
        }
    };
    let exclusive = match details.get("exclusive") {
        Some(val) => match val.as_str().parse::<bool>() {
            Ok(_val) => _val,
            Err(_) => {
                return redis_err!(format!(
                    "Unable to parse boolean from exclusive value at {key};{plugin}: {val}"
                ))
            }
        },
        None => {
            return redis_err!(format!(
                "Node details at key {key};{plugin} missing exclusive field."
            ))
        }
    };

    Ok(RawNode {
        name: name.to_owned(),
        exclusive,
        link_id: details.remove("link_id"),
        dns_names: generic_key
            .split(';')
            .map(|v| v.to_owned())
            .skip(1)
            .collect(),
        plugin: plugin.to_owned(),
    })
}

/// Fetches raw nodes from a connection.
fn fetch_raw_nodes(con: &mut Connection) -> NetdoxResult<Vec<RawNode>> {
    let nodes: HashSet<String> = match con.smembers(NODES_KEY) {
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
        let plugins: HashSet<String> = match con.smembers(format!("{redis_key};plugins")) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get plugins for node with key {redis_key}: {err}"
                ))
            }
            Ok(val) => val,
        };

        for plugin in plugins {
            raw.push(construct_raw_node(&format!("{redis_key};{plugin}"), con)?)
        }
    }

    Ok(raw)
}

/// Maps nodes to the superset of their DNS names.
fn map_nodes(dns: &DNS, nodes: Vec<RawNode>) -> HashMap<Vec<String>, Vec<RawNode>> {
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

// RESOLVED NODES

#[derive(Debug)]
/// A processed, linkable node.
struct ResolvedNode {
    name: String,
    link_id: String,
    alt_names: HashSet<String>,
    dns_names: HashSet<String>,
    plugins: HashSet<String>,
}

impl ResolvedNode {
    /// Writes this node to a db.
    fn write(&self, con: &mut Connection) -> NetdoxResult<()> {
        let mut sorted_names: Vec<_> = self.dns_names.iter().map(|v| v.to_owned()).collect();
        sorted_names.sort();

        let key = format!("{NODES_KEY};{}", sorted_names.join(";"));
        if let Err(err) = con.hset_multiple::<_, _, _, String>(
            &key,
            &[("name", &self.name), ("link_id", &self.link_id)],
        ) {
            return redis_err!(format!(
                "Failed while setting name or link_id for resolved node: {err}"
            ));
        }

        if let Err(err) = con.sadd::<_, _, String>(format!("{key};alt_names"), &self.alt_names) {
            return redis_err!(format!(
                "Failed while updating alt names for resolved node: {err}"
            ));
        }

        if let Err(err) = con.sadd::<_, _, String>(format!("{key};dns_names"), &self.dns_names) {
            return redis_err!(format!(
                "Failed while updating dns names for resolved node: {err}"
            ));
        }

        if let Err(err) = con.sadd::<_, _, String>(format!("{key};plugins"), &self.plugins) {
            return redis_err!(format!(
                "Failed while updating plugins for resolved node: {err}"
            ));
        }

        Ok(())
    }
}

/// Consolidates raw nodes into resolved nodes.
fn resolve_nodes(dns: &DNS, nodes: Vec<RawNode>) -> Vec<ResolvedNode> {
    let mut resolved = Vec::new();
    for (superset, nodes) in map_nodes(dns, nodes) {
        let mut linkable = None;
        let mut alt_names = HashSet::new();
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
            }
        }

        if let Some(node) = linkable {
            resolved.push(ResolvedNode {
                name: node.name.clone(),
                alt_names,
                dns_names: superset.into_iter().collect(),
                link_id: node.link_id.clone().unwrap(),
                plugins,
            });
        }
    }

    resolved
}
