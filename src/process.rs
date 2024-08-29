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

pub async fn process(mut con: DataStore) -> NetdoxResult<()> {
    let dns = con.get_dns().await?;
    let raw_nodes = con.get_raw_nodes().await?;

    let mut dns_node_claims = HashMap::new();
    for node in resolve_nodes(&dns, raw_nodes)? {
        con.put_node(&node).await?;

        // TODO stabilize this https://gitlab.allette.com.au/allette/netdox/netdox-redis/-/issues/47
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
    }

    for (dns_name, mut node_claims) in dns_node_claims {
        node_claims.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some((_, link_id)) = node_claims.first() {
            con.put_dns_metadata(
                &dns_name,
                NETDOX_PLUGIN,
                HashMap::from([
                    ("node", format!("(!(procnode|!|{})!)", link_id).as_ref()),
                    ("_node", link_id.as_ref()),
                ]),
            )
            .await?;
        }
    }

    Ok(())
}

/// Copies the data from each locator into the node that matches based on `cmp`.
/// Returns locators that failed to match any node.
fn consume_locators<'a>(
    nodes: &mut HashMap<String, Node>,
    locators: &[&'a RawNode],
    cmp: impl Fn(&RawNode, &Node) -> NetdoxResult<bool>,
) -> NetdoxResult<Vec<&'a RawNode>> {
    let mut unmatched = vec![];
    for locator in locators {
        let mut matches = vec![];
        // Build list of all linkable nodes that could consume the locator.
        for node in nodes.values() {
            if cmp(locator, node)? {
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
                        .dns_names
                        .len()
                        .cmp(&nodes.get(n2).unwrap().dns_names.len())
                });
            }

            let consumer = nodes.get_mut(matches.first().unwrap()).unwrap();
            consumer.dns_names.extend(locator.dns_names.clone());
            consumer.alt_names.extend(locator.name.clone());
            consumer.plugins.insert(locator.plugin.clone());
            consumer.raw_ids.insert(locator.id());
        }
    }

    Ok(unmatched)
}

/// Processes RawNodes into Nodes.
fn resolve_nodes(dns: &DNS, nodes: Vec<RawNode>) -> NetdoxResult<Vec<Node>> {
    let (linkable, locators): (Vec<_>, Vec<_>) =
        nodes.into_iter().partition(|n| n.link_id.is_some());

    let mut resolved = HashMap::new();
    for node in linkable {
        resolved.insert(
            node.link_id.clone().unwrap(),
            Node {
                name: node.name.clone().expect("Linkable node without name."),
                alt_names: HashSet::new(),
                dns_names: match node.exclusive {
                    true => node.dns_names.clone(),
                    false => dns.node_superset(&node)?,
                },
                link_id: node.link_id.clone().unwrap(),
                plugins: HashSet::from([node.plugin.clone()]),
                raw_ids: HashSet::from([node.id()]),
            },
        );
    }

    // Match the locator against linkable nodes by DNS name set
    let mut unmatched_locators = consume_locators(
        &mut resolved,
        &locators.iter().collect_vec(),
        |loc: &RawNode, node: &Node| -> NetdoxResult<bool> {
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
        |loc: &RawNode, node: &Node| -> NetdoxResult<bool> {
            Ok(dns.node_superset(loc)?.is_subset(&node.dns_names))
        },
    )?;

    if !unmatched_locators.is_empty() {
        warn!("Failed to match all locators to a node.");
    }

    Ok(resolved.into_values().collect_vec())
}
