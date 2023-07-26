use std::collections::HashSet;

use crate::{
    process::{
        model::{NODES_KEY, PROC_DB},
        process,
    },
    tests_common::*,
};

use super::model::ResolvedNode;

#[test]
fn test_process_1() {
    let mut client = setup_db();
    let mut con = client.get_connection().unwrap();
    let mock = ResolvedNode {
        name: "linkable-name".to_string(),
        link_id: "!link_id!".to_string(),
        alt_names: HashSet::from(["soft-name".to_string()]),
        dns_names: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[default-net]domain.net".to_string(),
            "[private-net]192.168.0.1".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
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
        .expect(&format!("Failed to select db {PROC_DB}"));

    let node = ResolvedNode::read(&format!("{NODES_KEY};{}", mock.link_id), &mut con).unwrap();
    assert_eq!(mock, node);
}

#[test]
fn test_process_2() {
    let mut client = setup_db();
    let mut con = client.get_connection().unwrap();
    let mock = ResolvedNode {
        name: "linkable-node".to_string(),
        link_id: "!link_id!".to_string(),
        alt_names: HashSet::from(["soft-matches".to_string()]),
        dns_names: HashSet::from([
            "[default-net]domain.com".to_string(),
            "[private-net]0.0.0.0".to_string(),
        ]),
        plugins: HashSet::from([PLUGIN.to_string()]),
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
        .expect(&format!("Failed to select db {PROC_DB}"));

    let node = ResolvedNode::read(&format!("{NODES_KEY};{}", mock.link_id), &mut con).unwrap();
    assert_eq!(mock, node);
}
