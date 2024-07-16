#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

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
    for node in resolve_nodes(&dns, raw_nodes)? {
        con.put_node(&node).await?;

        // TODO stabilize this https://gitlab.allette.com.au/allette/netdox/netdox-redis/-/issues/47
        for dns_name in node.dns_names {
            con.put_dns_metadata(
                &dns_name,
                NETDOX_PLUGIN,
                HashMap::from([
                    (
                        "node",
                        format!("(!(procnode|!|{})!)", node.link_id).as_ref(),
                    ),
                    ("_node", node.link_id.as_ref()),
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
                name: node
                    .name
                    .clone()
                    .expect("Linkable node without name.")
                    .into(),
                alt_names: HashSet::new(),
                dns_names: node.dns_names.clone(),
                link_id: node.link_id.clone().unwrap(),
                plugins: HashSet::from([node.plugin.clone()]),
                raw_ids: HashSet::from([node.id()]),
            },
        );
    }

    // Match the locator against linkable nodes by DNS name set
    let mut unmatched_locators = consume_locators(
        &mut resolved,
        &locators.iter().collect::<Vec<_>>(),
        |loc: &RawNode, node: &Node| -> NetdoxResult<bool> {
            return Ok(loc.dns_names.is_subset(&node.dns_names));
        },
    )?;

    // If the locator was not consumed, try again using its superset
    unmatched_locators = consume_locators(
        &mut resolved,
        &unmatched_locators,
        |loc: &RawNode, node: &Node| -> NetdoxResult<bool> {
            return Ok(dns.node_superset(loc)?.is_subset(&node.dns_names));
        },
    )?;

    if !unmatched_locators.is_empty() {
        warn!("Failed to match all locators to a node.");
    }

    Ok(resolved.into_values().collect_vec())
}
