#[cfg(test)]
mod tests;

use std::collections::{hash_map::Entry, HashMap, HashSet};

use paris::warn;
use redis::{Client, Commands, Connection};

use crate::{
    data::model::*,
    data::Datastore,
    error::{NetdoxError, NetdoxResult},
    process_err, redis_err,
};

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
    let dns = data_con.get_dns()?;
    let raw_nodes = data_con.get_raw_nodes()?;
    for node in resolve_nodes(&dns, raw_nodes)? {
        node.write(&mut proc_con)?;
    }

    Ok(())
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
fn _resolve_nodes(nodes: Vec<&RawNode>) -> NetdoxResult<Option<Node>> {
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
        Ok(Some(Node {
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
fn resolve_nodes(dns: &DNS, nodes: Vec<RawNode>) -> NetdoxResult<Vec<Node>> {
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
