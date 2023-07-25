mod model;

use std::collections::{hash_map::Entry, HashMap, HashSet};

use paris::{error, info};
use redis::{Client, Commands, Connection};

use crate::{
    error::{NetdoxError, NetdoxResult},
    redis_err,
};
use model::*;

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
    for node in resolve_nodes(&dns, raw_nodes)? {
        node.write(&mut proc_con)?;
    }

    Ok(())
}

// DNS

/// Gets the DNS data from redis.
fn fetch_dns(con: &mut Connection) -> NetdoxResult<DNS> {
    let dns_names: HashSet<String> = match con.smembers(DNS_KEY) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get set of dns names using key {DNS_KEY}: {err}"
            ))
        }
        Ok(_k) => _k,
    };

    let mut dns = DNS::new();
    for name in dns_names {
        dns.absorb(fetch_dns_name(&name, con)?)?;
    }

    Ok(dns)
}

/// Fetches a DNS struct with only data for the given DNS name.
fn fetch_dns_name(name: &str, con: &mut Connection) -> NetdoxResult<DNS> {
    let plugins: HashSet<String> = match con.smembers(format!("{DNS_KEY};{name};plugins")) {
        Err(err) => return redis_err!(format!("Failed to get plugins for dns name {name}: {err}")),
        Ok(_p) => _p,
    };

    let mut dns = DNS::new();
    for plugin in plugins {
        dns.absorb(fetch_plugin_dns_name(name, &plugin, con)?)?;
    }

    let translations: HashSet<String> = match con.smembers(format!("{DNS_KEY};{name};maps")) {
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

/// Fetches a DNS struct with only data for the given DNS name from the given source plugin.
fn fetch_plugin_dns_name(name: &str, plugin: &str, con: &mut Connection) -> NetdoxResult<DNS> {
    let mut dns = DNS::new();
    let rtypes: HashSet<String> = match con.smembers(format!("{DNS_KEY};{name};{plugin}")) {
        Err(err) => {
            return redis_err!(format!(
                "Failed to get record types from plugin {plugin} for dns name {name}: {err}"
            ))
        }
        Ok(_t) => _t,
    };

    for rtype in rtypes {
        let values: HashSet<String> = match con.smembers(format!("{DNS_KEY};{name};{plugin};{rtype}")) {
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

// RAW NODES

/// Contructs a raw node from the details stored under the provided key.
fn construct_raw_node(key: &str, con: &mut Connection) -> NetdoxResult<RawNode> {
    let (generic_key, plugin) = match key.rsplit_once(';') {
        None => return redis_err!(format!("Invalid node redis key: {key}")),
        Some(val) => val,
    };
    let mut details: HashMap<String, String> = match con.hgetall(key) {
        Err(err) => return redis_err!(format!("Failed to get node details at {key}: {err}")),
        Ok(val) => val,
    };
    let name = match details.get("name") {
        Some(val) => val,
        None => return redis_err!(format!("Node details at key {key} missing name field.")),
    };
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
        let plugins: HashSet<String> = match con.smembers(&format!("{redis_key};plugins")) {
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

/// Maps some nodes by their network-scoped DNS name supersets.
fn map_nodes<'a>(
    dns: &DNS,
    nodes: Vec<&'a RawNode>,
) -> NetdoxResult<HashMap<NetworkSuperSet, Vec<&'a RawNode>>> {
    let mut node_map = HashMap::new();
    for node in nodes {
        for superset in dns.node_superset(node)?.into_iter() {
            match node_map.entry(superset) {
                Entry::Vacant(entry) => {
                    entry.insert(vec![node]);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(node);
                }
            }
        }
    }

    Ok(node_map)
}

// RESOLVED NODES

/// Consolidates raw nodes into resolved nodes.
fn resolve_nodes(dns: &DNS, nodes: Vec<RawNode>) -> NetdoxResult<Vec<ResolvedNode>> {
    let mut resolved = Vec::new();
    for (superset, nodes) in map_nodes(dns, nodes.iter().collect())? {
        info!("{nodes:?}");
        info!("{superset:?}");
        info!("----------------");
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
                    error!(
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
                dns_names: superset.names,
                link_id: node.link_id.clone().unwrap(),
                plugins,
            });
        }
    }

    Ok(resolved)
}
