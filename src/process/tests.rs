use std::collections::HashSet;

use crate::{
    data::model::{Node, NODES_KEY, PROC_DB},
    process::process,
    tests_common::*,
};

#[test]
fn test_process_1() {
    let mut client = setup_db();
    let mut con = client.get_connection().unwrap();
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
        raw_keys: HashSet::from([
            format!("{NODES_KEY};[default-net]domain.com;[private-net]192.168.0.1"),
            format!("{NODES_KEY};[default-net]domain.net"),
        ]),
    };

    // Setup dns records for merging.
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "domain.net", PLUGIN, "cname", "domain.com"],
    );

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
    );

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
    );

    process(&mut client).unwrap();

    redis::cmd("SELECT")
        .arg(PROC_DB)
        .query::<String>(&mut con)
        .unwrap_or_else(|_| panic!("Failed to select db {PROC_DB}"));

    let node = Node::read(&mut con, &format!("{NODES_KEY};{}", mock.link_id)).unwrap();
    assert_eq!(mock, node);
}

#[test]
fn test_process_2() {
    let mut client = setup_db();
    let mut con = client.get_connection().unwrap();
    let mock = Node {
        name: "linkable-node".to_string(),
        link_id: "!link_id!".to_string(),
        alt_names: HashSet::from(["soft-matches".to_string()]),
        dns_names: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[private-net]0.0.0.0".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_keys: HashSet::from([
            format!("{NODES_KEY};[default-net]domain.com"),
            format!("{NODES_KEY};[default-net]domain.com;[private-net]0.0.0.0"),
        ]),
    };

    // Create soft nodes
    call_fn(
        &mut con,
        "netdox_create_node",
        &["1", "domain.com", PLUGIN, "soft-matches"],
    );
    call_fn(
        &mut con,
        "netdox_create_node",
        &["2", "domain.net", "domain.com", PLUGIN, "soft-nomatch"],
    );

    // Link soft nodes (should merge if linkable node not exclusive, as tested above.)
    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "domain.net", PLUGIN, "cname", "domain.com"],
    );

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
    );

    process(&mut client).unwrap();

    redis::cmd("SELECT")
        .arg(PROC_DB)
        .query::<String>(&mut con)
        .unwrap_or_else(|_| panic!("Failed to select db {PROC_DB}"));

    let node = Node::read(&mut con, &format!("{NODES_KEY};{}", mock.link_id)).unwrap();
    assert_eq!(mock, node);
}
