pub mod model;
#[cfg(test)]
mod tests;

use std::collections::{hash_map::Entry, HashMap, HashSet};

use paris::warn;
use redis::{Client, Commands, Connection};

use crate::{
    error::{NetdoxError, NetdoxResult},
    process_err, redis_err,
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
        let count: u64 = match con.get(&redis_key) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get number of nodes with key {redis_key}: {err}"
                ))
            }
            Ok(val) => val,
        };

        for index in 1..=count {
            raw.push(RawNode::from_key(con, &format!("{redis_key};{index}"))?)
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

// TODO refactor these two fns with better names.
fn _resolve_nodes(nodes: Vec<&RawNode>) -> NetdoxResult<Option<ResolvedNode>> {
    let num_nodes = nodes.len();
    let mut linkable = None;
    let mut alt_names = HashSet::new();
    let mut dns_names = HashSet::new();
    let mut plugins = HashSet::new();
    let mut raw_keys = HashSet::new();
    for node in &nodes {
        plugins.insert(node.plugin.clone());
        dns_names.extend(node.dns_names.clone());
        raw_keys.insert(node.redis_key());
        if node.link_id.is_some() {
            if linkable.is_none() {
                linkable = Some(node);
            } else {
                // TODO review this behaviour
                return process_err!(format!(
                    "Nodes in set {nodes:?} have multiple link ids: {}, {}",
                    linkable.as_ref().unwrap().link_id.as_ref().unwrap(),
                    node.link_id.as_ref().unwrap()
                ));
            }
        } else {
            alt_names.insert(node.name.clone());
        }
    }

    if let Some(node) = linkable {
        Ok(Some(ResolvedNode {
            name: node.name.clone(),
            alt_names,
            dns_names,
            link_id: node.link_id.clone().unwrap(),
            plugins,
            raw_keys,
        }))
    } else if num_nodes > 1 {
        process_err!(format!(
            "Found matching soft nodes with no link id: {nodes:?}"
        ))
    } else {
        Ok(None)
    }
}

/// Consolidates raw nodes into resolved nodes.
fn resolve_nodes(dns: &DNS, nodes: Vec<RawNode>) -> NetdoxResult<Vec<ResolvedNode>> {
    let mut resolved = Vec::new();

    // Splits nodes into permissive (not exclusive) and exclusive.
    let (mut exclusive, permissive): (Vec<_>, Vec<_>) = nodes.iter().partition(|n| n.exclusive);
    exclusive.sort_by(|n1, n2| n1.dns_names.len().cmp(&n2.dns_names.len()));

    // Splits nodes into exclusive + permissive matches, and unmatched nodes.
    let (mut exc_matches, mut unmatched): (HashMap<&RawNode, Vec<&RawNode>>, Vec<&RawNode>) =
        (HashMap::new(), vec![]);

    for perm_node in permissive {
        let mut exc_match = false;

        // Check if permissive node matches any exclusive nodes.
        for exc_node in &exclusive {
            if perm_node.dns_names.is_subset(&exc_node.dns_names) {
                match exc_matches.entry(exc_node) {
                    Entry::Vacant(entry) => {
                        entry.insert(vec![perm_node]);
                    }
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().push(perm_node);
                    }
                }
                exc_match = true;
                break;
            }
        }

        if !exc_match {
            unmatched.push(perm_node);
        }
    }

    // Add unmatched exclusive nodes to unmatched node pool.
    for exc_node in exclusive {
        if !exc_matches.contains_key(exc_node) {
            unmatched.push(exc_node);
        }
    }

    // Resolve exclusive node match groups.
    for (exc_node, mut matching_nodes) in exc_matches {
        matching_nodes.push(exc_node);
        match _resolve_nodes(matching_nodes)? {
            Some(node) => resolved.push(node),
            None => warn!(
                "Failed to create resolved node from set of nodes matching exclusive node: {exc_node:?}"
            )
        }
    }

    // Resolve match groups for all unmatched nodes.
    for (superset, nodes) in map_nodes(dns, unmatched)? {
        match _resolve_nodes(nodes)? {
            Some(node) => resolved.push(node),
            None => warn!(
                "Failed to create resolved node from set of nodes under superset: {superset:?}"
            ),
        };
    }

    Ok(resolved)
}
