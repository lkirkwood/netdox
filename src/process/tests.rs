use std::collections::HashSet;

use crate::{
    data::{
        model::{DNSRecord, Node, DNS, NETDOX_PLUGIN},
        store::DataConn,
        DataStore,
    },
    process::process,
    tests_common::*,
};

use super::match_dns_to_node;

#[tokio::test]
async fn test_map_nodes_1() {
    let mut con = setup_db_con().await;
    let mock = Node {
        name: "linkable-name".to_string(),
        link_id: "map-nodes-1-id".to_string(),
        alt_names: HashSet::from(["soft-name".to_string()]),
        dns_names: HashSet::from([
            "[default-net]map-nodes.com".to_string(),
            "[default-net]map-nodes.net".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([
            "[default-net]map-nodes.com".to_string(),
            "[default-net]map-nodes.net".to_string(),
        ]),
    };

    // Setup dns records for merging.
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "map-nodes.net", PLUGIN, "cname", "map-nodes.com"],
    )
    .await;

    // Create soft node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &["1", "map-nodes.com", PLUGIN, "soft-name"],
    )
    .await;

    // Create linkable node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "1",
            "map-nodes.net",
            PLUGIN,
            &mock.name,
            "false",
            &mock.link_id,
        ],
    )
    .await;

    process(DataStore::Redis(con.clone())).await.unwrap();

    let node = con.get_node(&mock.link_id).await.unwrap();
    assert_eq!(mock, node);
}

#[tokio::test]
async fn test_superset() {
    let mut con = setup_db_con().await;
    let mock = Node {
        name: "linkable-node".to_string(),
        link_id: "superset_id".to_string(),
        alt_names: HashSet::new(),
        dns_names: HashSet::from([
            "[superset]superset.com".to_string(),
            "[superset]superset.net".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from(["[superset]superset.com".to_string()]),
    };

    call_fn(
        &mut con,
        "netdox_create_dns",
        &[
            "1",
            "[superset]superset.net",
            PLUGIN,
            "cname",
            "[superset]superset.com",
        ],
    )
    .await;

    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "1",
            "[superset]superset.com",
            PLUGIN,
            "linkable-node",
            "false",
            "superset_id",
        ],
    )
    .await;

    process(DataStore::Redis(con.clone())).await.unwrap();

    let node = con.get_node(&mock.link_id).await.unwrap();
    assert_eq!(mock, node);
}

#[test]
fn test_match_dns_to_node_simple() {
    let domain = format!("[{DEFAULT_NETWORK}]match-dns-test-simple.com");
    let ip = format!("[{DEFAULT_NETWORK}]42.0.0.1");

    let mut dns = DNS::new();
    dns.add_record(DNSRecord {
        name: domain.clone(),
        plugin: NETDOX_PLUGIN.to_string(),
        rtype: "A".to_string(),
        value: ip.clone(),
    });

    let mut node = Node {
        name: "match-dns-node-simple".to_string(),
        link_id: "match-dns-node-simple".to_string(),
        dns_names: HashSet::from([ip.clone()]),
        plugins: HashSet::from([NETDOX_PLUGIN.to_string()]),
        alt_names: HashSet::new(),
        raw_ids: HashSet::new(),
    };

    let proc_nodes = vec![(HashSet::from([ip.clone(), domain.clone()]), node.clone())];

    let matches = match_dns_to_node(dns, proc_nodes).unwrap();

    node.dns_names.insert(domain.clone());
    assert_eq!(
        matches.dns_nodes.get(&domain).unwrap().clone().into_inner(),
        node
    );
}

#[test]
fn test_match_dns_to_node_nosteal() {
    let first_ip = format!("[{DEFAULT_NETWORK}]42.0.0.1");
    let second_ip = format!("[{DEFAULT_NETWORK}]42.0.0.2");
    let third_ip = format!("[{DEFAULT_NETWORK}]42.0.0.3");

    let node_oneip = Node {
        name: "match-dns-node-nosteal-oneip".to_string(),
        link_id: "match-dns-node-nosteal-oneip".to_string(),
        dns_names: HashSet::from([first_ip.clone()]),
        plugins: HashSet::from([NETDOX_PLUGIN.to_string()]),
        alt_names: HashSet::new(),
        raw_ids: HashSet::new(),
    };

    let manyips = HashSet::from([first_ip.clone(), second_ip.clone(), third_ip.clone()]);

    let node_manyips = Node {
        name: "match-dns-node-nosteal-manyips".to_string(),
        link_id: "match-dns-node-nosteal-manyips".to_string(),
        dns_names: manyips.clone(),
        plugins: HashSet::from([NETDOX_PLUGIN.to_string()]),
        alt_names: HashSet::new(),
        raw_ids: HashSet::new(),
    };

    let mut dns = DNS::new();
    dns.qnames.extend(manyips.clone());

    let proc_nodes = vec![
        (HashSet::from([first_ip.clone()]), node_oneip.clone()),
        (manyips, node_manyips.clone()),
    ];

    let matches = match_dns_to_node(dns, proc_nodes).unwrap();

    assert_eq!(
        matches
            .dns_nodes
            .get(&first_ip)
            .unwrap()
            .clone()
            .into_inner(),
        node_oneip
    );

    assert_eq!(
        matches
            .dns_nodes
            .get(&second_ip)
            .unwrap()
            .clone()
            .into_inner(),
        node_manyips
    );

    assert_eq!(
        matches
            .dns_nodes
            .get(&third_ip)
            .unwrap()
            .clone()
            .into_inner(),
        node_manyips
    );
}
