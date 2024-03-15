#[cfg(test)]
mod tests;

use std::collections::{hash_map::Entry, HashMap, HashSet};

use paris::warn;

use crate::{
    data::model::*,
    data::DataConn,
    error::{NetdoxError, NetdoxResult},
    process_err,
};

const NETDOX_PLUGIN: &str = "netdox";

pub async fn process(mut con: Box<dyn DataConn>) -> NetdoxResult<()> {
    let dns = con.get_dns().await?;
    let raw_nodes = con.get_raw_nodes().await?;
    for node in resolve_nodes(&dns, raw_nodes)? {
        con.put_node(&node).await?;

        // TODO figure out a stable alg for this
        for dns_name in node.dns_names {
            con.put_dns_metadata(
                &dns_name,
                NETDOX_PLUGIN,
                HashMap::from([(
                    "node",
                    format!("(!(procnode|!|{})!)", node.link_id).as_ref(),
                )]),
            )
            .await?;
        }
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
fn _resolve_nodes(nodes: &[&RawNode], mut dns_names: HashSet<String>) -> NetdoxResult<Vec<Node>> {
    let mut linkable: Vec<Node> = vec![];
    let mut alt_names = HashSet::new();
    let mut plugins = HashSet::new();
    let mut raw_ids = HashSet::new();
    for node in nodes {
        if let Some(link_id) = &node.link_id {
            if !linkable.is_empty() {
                for procnode in &mut linkable {
                    if procnode.dns_names.intersection(&node.dns_names).count() > 0 {
                        if procnode.link_id == *link_id {
                            if let Some(name) = &node.name {
                                procnode.alt_names.insert(name.clone());
                            }
                            procnode.plugins.insert(node.plugin.clone());
                            procnode.dns_names.extend(node.dns_names.clone());
                            procnode.raw_ids.insert(node.id());
                            continue;
                        } else {
                            return process_err!(format!(
                                "Cannot separate ambiguous node set: {nodes:?}"
                            ));
                        };
                    }
                }
            }

            if let Some(name) = &node.name {
                linkable.push(Node {
                    name: name.to_owned(),
                    link_id: link_id.to_owned(),
                    plugins: HashSet::from([node.plugin.clone()]),
                    raw_ids: HashSet::from([node.id()]),
                    dns_names: node.dns_names.clone(),
                    alt_names: HashSet::new(),
                });
            } else {
                return process_err!(format!(
                    "Linkable node with id {} has no name.",
                    node.link_id.as_ref().unwrap()
                ));
            }
        } else {
            if let Some(name) = &node.name {
                alt_names.insert(name.to_owned());
            }
            plugins.insert(node.plugin.clone());
            dns_names.extend(node.dns_names.clone());
            raw_ids.insert(node.id());
        }
    }

    for node in &mut linkable {
        node.dns_names.extend(dns_names.clone());
        node.alt_names.extend(alt_names.clone());
        node.plugins.extend(plugins.clone());
        node.raw_ids.extend(raw_ids.clone());
    }

    Ok(linkable)
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
        let procnodes = _resolve_nodes(&matching_nodes, HashSet::new())?;

        if procnodes.is_empty() {
            warn!("Failed to create processed node from set of nodes: {matching_nodes:?}");
        } else {
            resolved.extend(procnodes)
        }
    }

    // Resolve match groups for all unmatched nodes.
    for (superset, nodes) in map_nodes(dns, unmatched)? {
        let procnodes = _resolve_nodes(&nodes, superset.names.clone())?;

        if procnodes.is_empty() {
            warn!("Failed to create processed node from set of nodes: {nodes:?}");
        } else {
            resolved.extend(procnodes)
        }
    }

    Ok(resolved)
}
