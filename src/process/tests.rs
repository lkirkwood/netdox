use std::collections::HashSet;

use crate::{
    data::{model::Node, store::DataConn, DataStore},
    process::process,
    tests_common::*,
};

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
            "[private-net]192.168.99.1".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([
            "[default-net]map-nodes.com;[private-net]192.168.99.1".to_string(),
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
        &[
            "2",
            "map-nodes.com",
            "[private-net]192.168.99.1",
            PLUGIN,
            "soft-name",
        ],
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
async fn test_map_nodes_2() {
    let mut con = setup_db_con().await;
    let mock = Node {
        name: "linkable-node".to_string(),
        link_id: "map-nodes-2-id".to_string(),
        alt_names: HashSet::from(["soft-matches".to_string()]),
        dns_names: HashSet::from([
            "[default-net]map-nodes-2.com".to_string(),
            "[private-net]192.168.120.55".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([
            "[default-net]map-nodes-2.com".to_string(),
            "[default-net]map-nodes-2.com;[private-net]192.168.120.55".to_string(),
        ]),
    };

    // Create soft nodes
    call_fn(
        &mut con,
        "netdox_create_node",
        &["1", "map-nodes-2.com", PLUGIN, "soft-matches"],
    )
    .await;
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "2",
            "map-nodes-2.net",
            "map-nodes-2.com",
            PLUGIN,
            "soft-nomatch",
        ],
    )
    .await;

    // Link soft nodes (should merge if linkable node not exclusive, as tested above.)
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "map-nodes-2.net", PLUGIN, "cname", "map-nodes-2.com"],
    )
    .await;

    // Create linkable, exclusive node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "2",
            "map-nodes-2.com",
            "[private-net]192.168.120.55",
            PLUGIN,
            "linkable-node",
            "true",
            "map-nodes-2-id",
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
            "[default-net]superset.com".to_string(),
            "[default-net]superset.net".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from(["[default-net]superset.com".to_string()]),
    };

    // Link soft nodes (should merge if linkable node not exclusive, as tested above.)
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "superset.net", PLUGIN, "cname", "superset.com"],
    )
    .await;

    // Create linkable, exclusive node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "1",
            "superset.com",
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
