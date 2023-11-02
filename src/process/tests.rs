use std::collections::HashSet;

use crate::{
    data::{model::Node, store::DataConn},
    process::process,
    tests_common::*,
};

#[tokio::test]
async fn test_map_nodes_1() {
    let mut client = setup_db().await;
    let mut con = client.get_async_connection().await.unwrap();
    let mock = Node {
        name: "linkable-name".to_string(),
        link_id: "!link_id!".to_string(),
        alt_names: HashSet::from(["soft-name".to_string()]),
        dns_names: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[default-net]domain.net".to_string(),
            "[private-net]192.168.0.1".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([
            "[default-net]domain.com;[private-net]192.168.0.1".to_string(),
            "[default-net]domain.net".to_string(),
        ]),
    };

    // Setup dns records for merging.
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "domain.net", PLUGIN, "cname", "domain.com"],
    )
    .await;

    // Create soft node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "2",
            "domain.com",
            "[private-net]192.168.0.1",
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
            "domain.net",
            PLUGIN,
            &mock.name,
            "false",
            &mock.link_id,
        ],
    )
    .await;

    process(&mut client).await.unwrap();

    let node = con.get_node(&mock.link_id).await.unwrap();
    assert_eq!(mock, node);
}

#[tokio::test]
async fn test_map_nodes_2() {
    let mut client = setup_db().await;
    let mut con = client.get_async_connection().await.unwrap();
    let mock = Node {
        name: "linkable-node".to_string(),
        link_id: "!link_id!".to_string(),
        alt_names: HashSet::from(["soft-matches".to_string()]),
        dns_names: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[private-net]0.0.0.0".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[default-net]domain.com;[private-net]0.0.0.0".to_string(),
        ]),
    };

    // Create soft nodes
    call_fn(
        &mut con,
        "netdox_create_node",
        &["1", "domain.com", PLUGIN, "soft-matches"],
    )
    .await;
    call_fn(
        &mut con,
        "netdox_create_node",
        &["2", "domain.net", "domain.com", PLUGIN, "soft-nomatch"],
    )
    .await;

    // Link soft nodes (should merge if linkable node not exclusive, as tested above.)
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "domain.net", PLUGIN, "cname", "domain.com"],
    )
    .await;

    // Create linkable, exclusive node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "2",
            "domain.com",
            "[private-net]0.0.0.0",
            PLUGIN,
            "linkable-node",
            "true",
            "!link_id!",
        ],
    )
    .await;

    process(&mut client).await.unwrap();

    let node = con.get_node(&mock.link_id).await.unwrap();
    assert_eq!(mock, node);
}

#[tokio::test]
async fn test_superset() {
    let mut client = setup_db().await;
    let mut con = client.get_async_connection().await.unwrap();
    let mock = Node {
        name: "linkable-node".to_string(),
        link_id: "!link_id!".to_string(),
        alt_names: HashSet::new(),
        dns_names: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[default-net]domain.net".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from(["[default-net]domain.com".to_string()]),
    };

    // Link soft nodes (should merge if linkable node not exclusive, as tested above.)
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "domain.net", PLUGIN, "cname", "domain.com"],
    )
    .await;

    // Create linkable, exclusive node.
    call_fn(
        &mut con,
        "netdox_create_node",
        &[
            "1",
            "domain.com",
            PLUGIN,
            "linkable-node",
            "false",
            "!link_id!",
        ],
    )
    .await;

    process(&mut client).await.unwrap();

    let node = con.get_node(&mock.link_id).await.unwrap();
    assert_eq!(mock, node);
}
