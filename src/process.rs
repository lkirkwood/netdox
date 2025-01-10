#[cfg(test)]
mod tests;

use std::collections::{hash_map::Entry, HashMap, HashSet};

use itertools::Itertools;
use paris::warn;

use crate::{
    data::{
        model::{Node, RawNode, DNS, NETDOX_PLUGIN},
        store::DataStore,
        DataConn,
    },
    error::NetdoxResult,
};

/// Processes raw nodes and matches DNS names to a node.
///
/// DNS names select a node based on "claims".
/// A claim is produced by a node which has reported that it owns that DNS name.
/// The set of DNS names reported by a node and the superset of those names both
/// create a claim (unless the node is exclusive => no superset).
/// Smaller claims with fewer DNS names are always prioritised over larger ones.
///
/// DNS names are also traced to their "terminal" (see DNS::forward_march).
/// If a DNS name has one or more terminals, the node claims on that terminal
/// are copied to the original DNS name. These claims are given lower priority
/// than regular claims of the same length.
///
/// TODO refactor DNS->node matching into pure function
pub async fn process(mut con: DataStore) -> NetdoxResult<()> {
    let dns = con.get_dns().await?;
    let raw_nodes = con.get_raw_nodes().await?;

    let mut node_map = HashMap::new();
    let proc_nodes = resolve_nodes(&dns, raw_nodes)?;

    let mut dns_node_claims = HashMap::new();
    for (superset, node) in proc_nodes {
        for dns_name in &node.dns_names {
            match dns_node_claims.entry(dns_name.to_string()) {
                Entry::Vacant(entry) => {
                    entry.insert(vec![(node.dns_names.len(), node.link_id.clone())]);
                }
                Entry::Occupied(mut entry) => {
                    entry
                        .get_mut()
                        .push((node.dns_names.len(), node.link_id.clone()));
                }
            }
        }

        for dns_name in &superset {
            match dns_node_claims.entry(dns_name.to_string()) {
                Entry::Vacant(entry) => {
                    entry.insert(vec![(superset.len(), node.link_id.clone())]);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push((superset.len(), node.link_id.clone()));
                }
            }
        }

        node_map.insert(node.link_id.clone(), node);
    }

    // Matches DNS names to the claims on their terminals.
    let mut terminal_node_claims = HashMap::new();
    for dns_name in &dns.qnames {
        for terminal in dns.forward_march(dns_name) {
            if let Entry::Occupied(dns_entry) = dns_node_claims.entry(terminal.to_string()) {
                match terminal_node_claims.entry(dns_name) {
                    Entry::Vacant(terminal_entry) => {
                        terminal_entry.insert(dns_entry.get().clone());
                    }
                    Entry::Occupied(mut terminal_entry) => {
                        terminal_entry
                            .get_mut()
                            .append(&mut dns_entry.get().clone());
                    }
                }
            }
        }
    }

    // Set metadata property on DNS names, and add the DNS name to the node's
    // set of DNS names if not already present.
    for dns_name in &dns.qnames {
        let best_claim_link_id = match (
            terminal_node_claims.get(dns_name),
            dns_node_claims.get(dns_name),
        ) {
            (Some(terminal_claims), Some(regular_claims)) => Some(
                regular_claims
                    .iter()
                    .chain(terminal_claims)
                    .sorted_by(|lhs, rhs| Ord::cmp(&lhs.0, &rhs.0))
                    .next()
                    .unwrap()
                    .1
                    .clone(),
            ),
            (Some(terminal_claims), None) => {
                Some(terminal_claims.iter().sorted().next().unwrap().1.clone())
            }
            (None, Some(regular_claims)) => {
                Some(regular_claims.iter().sorted().next().unwrap().1.clone())
            }
            (None, None) => None,
        };

        if let Some(link_id) = best_claim_link_id {
            con.put_dns_metadata(
                dns_name,
                NETDOX_PLUGIN,
                HashMap::from([
                    ("node", format!("(!(procnode|!|{link_id})!)").as_ref()),
                    ("_node", link_id.as_ref()),
                ]),
            )
            .await?;

            node_map
                .get_mut(&link_id)
                .unwrap()
                .dns_names
                .insert(dns_name.to_string());
        }
    }

    for node in node_map.values() {
        con.put_node(node).await?;
    }

    Ok(())
}

/// Copies the data from each locator into the node that matches based on `cmp`.
/// Returns locators that failed to match any node.
fn consume_locators<'a>(
    nodes: &mut HashMap<String, (HashSet<String>, Node)>,
    locators: &[&'a RawNode],
    cmp: impl Fn(&RawNode, &Node, &HashSet<String>) -> NetdoxResult<bool>,
) -> NetdoxResult<Vec<&'a RawNode>> {
    let mut unmatched = vec![];
    for locator in locators {
        let mut matches = vec![];
        // Build list of all linkable nodes that could consume the locator.
        for (superset, node) in nodes.values() {
            if cmp(locator, node, superset)? {
                matches.push(node.link_id.clone());
            }
        }

        if matches.is_empty() {
            unmatched.push(*locator);
        } else {
            // Let linkable node with smallest matching set of DNS names consume the locator.
            if matches.len() > 1 {
                matches.sort_by(|n1, n2| {
                    nodes
                        .get(n1)
                        .unwrap()
                        .1
                        .dns_names
                        .len()
                        .cmp(&nodes.get(n2).unwrap().1.dns_names.len())
                });
            }

            let consumer = &mut nodes.get_mut(matches.first().unwrap()).unwrap().1;
            consumer.dns_names.extend(locator.dns_names.clone());
            consumer.alt_names.extend(locator.name.clone());
            consumer.plugins.insert(locator.plugin.clone());
            consumer.raw_ids.insert(locator.id());
        }
    }

    Ok(unmatched)
}

/// Processes RawNodes into Nodes.
fn resolve_nodes(dns: &DNS, nodes: Vec<RawNode>) -> NetdoxResult<Vec<(HashSet<String>, Node)>> {
    let (linkable, locators): (Vec<_>, Vec<_>) =
        nodes.into_iter().partition(|n| n.link_id.is_some());

    let mut resolved = HashMap::new();
    for node in linkable {
        resolved.insert(
            node.link_id.clone().unwrap(),
            (
                if node.exclusive {
                    HashSet::new()
                } else {
                    dns.node_superset(&node)?
                },
                Node {
                    name: node.name.clone().expect("Linkable node without name."),
                    alt_names: HashSet::new(),
                    dns_names: node.dns_names.clone(),
                    link_id: node.link_id.clone().unwrap(),
                    plugins: HashSet::from([node.plugin.clone()]),
                    raw_ids: HashSet::from([node.id()]),
                },
            ),
        );
    }

    // Match the locator against linkable nodes by DNS name set
    let mut unmatched_locators = consume_locators(
        &mut resolved,
        &locators.iter().collect_vec(),
        |loc: &RawNode, node: &Node, _: &HashSet<String>| -> NetdoxResult<bool> {
            Ok(loc.dns_names.is_subset(&node.dns_names))
        },
    )?;

    // If the locator was not consumed, try again using its superset
    unmatched_locators = consume_locators(
        &mut resolved,
        &unmatched_locators
            .into_iter()
            .filter(|n| !n.exclusive)
            .collect_vec(),
        |loc: &RawNode, _: &Node, superset: &HashSet<String>| -> NetdoxResult<bool> {
            Ok(dns.node_superset(loc)?.is_subset(superset))
        },
    )?;

    if !unmatched_locators.is_empty() {
        warn!("Failed to match all locators to a node.");
    }

    Ok(resolved.into_values().collect_vec())
}
