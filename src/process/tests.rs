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
