use crate::data::model::{DNS_KEY, DNS_PDATA_KEY, NODES_KEY, NODE_PDATA_KEY};
use crate::tests_common::*;
use redis::AsyncCommands;
use std::collections::HashMap;

// TESTS

#[tokio::test]
async fn test_create_dns_noval() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);

    // Unqualified
    call_fn(&mut con, function, &["1", name, PLUGIN]).await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");

    assert!(result_name);
    assert!(result_plugin);

    // Qualified
    call_fn(&mut con, function, &["1", &qname, PLUGIN]).await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");

    assert!(result_name);
    assert!(result_plugin);
}

#[tokio::test]
async fn test_create_dns_cname() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "CNAME";
    let value = "netdox.org";

    // Unqualified
    call_fn(&mut con, function, &["1", name, PLUGIN, rtype, value]).await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_value: bool = con
        .sismember(
            format!("{};{};{};{}", DNS_KEY, &qname, PLUGIN, &rtype),
            format!("[{DEFAULT_NETWORK}]{value}"),
        )
        .await
        .expect("Failed sismember.");

    assert!(result_name);
    assert!(result_plugin);
    assert!(result_value);

    // Qualified
    call_fn(&mut con, function, &["1", &qname, PLUGIN, rtype, value]).await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");

    assert!(result_name);
    assert!(result_plugin);
    assert!(result_value);
}

#[tokio::test]
async fn test_create_dns_a() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "A";
    let value = "192.168.0.1";

    // Unqualified
    call_fn(&mut con, function, &["1", name, PLUGIN, rtype, value]).await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_value: bool = con
        .sismember(
            format!("{};{};{};{}", DNS_KEY, &qname, PLUGIN, &rtype),
            format!("[{DEFAULT_NETWORK}]{value}"),
        )
        .await
        .expect("Failed sismember.");

    assert!(result_name);
    assert!(result_plugin);
    assert!(result_value);

    // Qualified
    call_fn(&mut con, function, &["1", &qname, PLUGIN, rtype, value]).await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");

    assert!(result_name);
    assert!(result_plugin);
    assert!(result_value);
}

#[tokio::test]
async fn test_map_dns_norev() {
    let mut con = setup_db_con().await;
    let function = "netdox_map_dns";
    let origin = "netdox.com";
    let qorigin = format!("[{}]{}", DEFAULT_NETWORK, origin);
    let reverse = "false";

    let dest1_net = "[org-net]";
    let dest1_name = "netdox.org";
    let qdest1 = format!("{}{}", dest1_net, dest1_name);
    let dest2_net = "[gov-net]";
    let dest2_name = "netdox.gov";
    let qdest2 = format!("{}{}", dest2_net, dest2_name);

    call_fn(
        &mut con,
        function,
        &["1", &qorigin, PLUGIN, reverse, &qdest1, &qdest2],
    )
    .await;

    let result_origin_dns: bool = con
        .sismember(DNS_KEY, &qorigin)
        .await
        .expect("Failed sismember.");
    let result_dest1_dns: bool = con
        .sismember(DNS_KEY, &qdest1)
        .await
        .expect("Failed sismember.");
    let result_dest2_dns: bool = con
        .sismember(DNS_KEY, &qdest2)
        .await
        .expect("Failed sismember.");

    let result_origin_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qorigin), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_dest1_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest1), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_dest2_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest2), PLUGIN)
        .await
        .expect("Failed sismember.");

    let result_map: HashMap<String, String> = con
        .hgetall(&format!("{};{};maps", DNS_KEY, &qorigin))
        .await
        .expect("Failed hgetall.");

    assert!(result_origin_dns);
    assert!(result_dest1_dns);
    assert!(result_dest2_dns);

    assert!(result_origin_plugins);
    assert!(result_dest1_plugins);
    assert!(result_dest2_plugins);

    assert_eq!(result_map.get(dest1_net), Some(&dest1_name.to_string()));
    assert_eq!(result_map.get(dest2_net), Some(&dest2_name.to_string()));
}

#[tokio::test]
async fn test_map_dns_rev() {
    let mut con = setup_db_con().await;
    let function = "netdox_map_dns";
    let origin = "netdox.com";
    let qorigin = format!("[{}]{}", DEFAULT_NETWORK, origin);
    let reverse = "true";

    let dest1_net = "[org-net]";
    let dest1_name = "netdox.org";
    let qdest1 = format!("{}{}", dest1_net, dest1_name);
    let dest2_net = "[gov-net]";
    let dest2_name = "netdox.gov";
    let qdest2 = format!("{}{}", dest2_net, dest2_name);

    call_fn(
        &mut con,
        function,
        &["1", &qorigin, PLUGIN, reverse, &qdest1, &qdest2],
    )
    .await;

    let result_origin_dns: bool = con
        .sismember(DNS_KEY, &qorigin)
        .await
        .expect("Failed sismember.");
    let result_dest1_dns: bool = con
        .sismember(DNS_KEY, &qdest1)
        .await
        .expect("Failed sismember.");
    let result_dest2_dns: bool = con
        .sismember(DNS_KEY, &qdest2)
        .await
        .expect("Failed sismember.");

    let result_origin_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qorigin), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_dest1_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest1), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_dest2_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest2), PLUGIN)
        .await
        .expect("Failed sismember.");

    let result_fmap: HashMap<String, String> = con
        .hgetall(&format!("{};{};maps", DNS_KEY, &qorigin))
        .await
        .expect("Failed hgetall.");
    let result_rdest1: Option<String> = con
        .hget(
            &format!("{};{};maps", DNS_KEY, &qdest1),
            &format!("[{}]", DEFAULT_NETWORK),
        )
        .await
        .expect("Failed hget.");
    let result_rdest2: Option<String> = con
        .hget(
            &format!("{};{};maps", DNS_KEY, &qdest2),
            &format!("[{}]", DEFAULT_NETWORK),
        )
        .await
        .expect("Failed hget.");

    assert!(result_origin_dns);
    assert!(result_dest1_dns);
    assert!(result_dest2_dns);

    assert!(result_origin_plugins);
    assert!(result_dest1_plugins);
    assert!(result_dest2_plugins);

    assert_eq!(result_fmap.get(dest1_net), Some(&dest1_name.to_string()));
    assert_eq!(result_fmap.get(dest2_net), Some(&dest2_name.to_string()));

    assert_eq!(result_rdest1, Some(origin.to_string()));
    assert_eq!(result_rdest2, Some(origin.to_string()));
}

#[tokio::test]
async fn test_create_node_soft() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let node_id = format!("{qnames};{PLUGIN}");

    call_fn(&mut con, function, &["2", domain, ip, PLUGIN, name]).await;

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(format!("{NODES_KEY};{node_id}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{NODES_KEY};{node_id};{result_count}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_all_nodes);
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get("name"), Some(&name.to_string()));
    assert_eq!(result_details.get("link_id"), None);
    assert_eq!(result_details.get("exclusive"), Some(&"false".to_string()));
}

#[tokio::test]
async fn test_create_node_no_exc() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let link_id = "node-link-id";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let node_id = format!("{qnames};{PLUGIN}");
    let exclusive = "false";

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, name, exclusive, link_id],
    )
    .await;

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(format!("{NODES_KEY};{node_id}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{NODES_KEY};{node_id};{result_count}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_all_nodes);
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get("name"), Some(&name.to_string()));
    assert_eq!(result_details.get("link_id"), Some(&link_id.to_string()));
    assert_eq!(
        result_details.get("exclusive"),
        Some(&exclusive.to_string())
    );
}

#[tokio::test]
async fn test_create_node_exc() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let link_id = "node-link-id";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let node_id = format!("{qnames};{PLUGIN}");
    let exclusive = "true";

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, name, exclusive, link_id],
    )
    .await;

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(format!("{NODES_KEY};{node_id}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{NODES_KEY};{node_id};{result_count}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_all_nodes);
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get("name"), Some(&name.to_string()));
    assert_eq!(result_details.get("link_id"), Some(&link_id.to_string()));
    assert_eq!(
        result_details.get("exclusive"),
        Some(&exclusive.to_string())
    );
}

#[tokio::test]
async fn test_create_dns_metadata() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(&mut con, "netdox_create_dns", &["1", name, PLUGIN]).await;
    call_fn(
        &mut con,
        function,
        &["1", name, PLUGIN, key1, val1, key2, val2],
    )
    .await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", DNS_KEY, &qname))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert!(result_plugin);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_dns_metadata_new() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &["1", name, PLUGIN, key1, val1, key2, val2],
    )
    .await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .await
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", DNS_KEY, &qname))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert!(result_plugin);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_node_metadata_linkable() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_metadata";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let node_id = format!("{qnames};{PLUGIN}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        "netdox_create_node",
        &["2", domain, ip, PLUGIN, "node-name", "false", "link-id"],
    )
    .await;
    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    )
    .await;

    let result_node: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(&format!("{NODES_KEY};{node_id}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{NODES_KEY};{node_id}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_node);
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_node_metadata_soft() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_metadata";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let node_id = format!("{qnames};{PLUGIN}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        "netdox_create_node",
        &["2", domain, ip, PLUGIN, "node-name"],
    )
    .await;
    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    )
    .await;

    let result_node: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(&format!("{NODES_KEY};{node_id}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{NODES_KEY};{node_id}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_node);
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_node_metadata_new() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_metadata";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let node_id = format!("{qnames};{PLUGIN}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    )
    .await;

    let result_node: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(&format!("{NODES_KEY};{node_id}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{NODES_KEY};{node_id}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_node);
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_dns_pdata_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let pdata_id = "some-data-id";
    let title = "Plugin Data Title";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &[
            "1", name, PLUGIN, "hash", pdata_id, title, key1, val1, key2, val2,
        ],
    )
    .await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_data: HashMap<String, String> = con
        .hgetall(&format!("{DNS_PDATA_KEY};{qname};{pdata_id}"))
        .await
        .expect("Failed hgetall.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("{DNS_PDATA_KEY};{qname};{pdata_id};details"))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert_eq!(result_data.get(key1), Some(&val1.to_string()));
    assert_eq!(result_data.get(key2), Some(&val2.to_string()));
    assert_eq!(result_details.get("type").unwrap(), "hash");
    assert_eq!(result_details.get("plugin").unwrap(), PLUGIN);
    assert_eq!(result_details.get("title").unwrap(), title);
}

#[tokio::test]
async fn test_create_node_pdata_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_plugin_data";
    let pdata_id = "some-data-id";
    let title = "Plugin Data Title";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let node_id = format!("{qname};{PLUGIN}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &[
            "1", name, PLUGIN, "hash", pdata_id, title, key1, val1, key2, val2,
        ],
    )
    .await;

    let result_name: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");
    let result_data: HashMap<String, String> = con
        .hgetall(&format!("{NODE_PDATA_KEY};{node_id};{pdata_id}"))
        .await
        .expect("Failed hgetall.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("{NODE_PDATA_KEY};{node_id};{pdata_id};details"))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert_eq!(result_data.get(key1), Some(&val1.to_string()));
    assert_eq!(result_data.get(key2), Some(&val2.to_string()));
    assert_eq!(result_details.get("type").unwrap(), "hash");
    assert_eq!(result_details.get("plugin").unwrap(), PLUGIN);
    assert_eq!(result_details.get("title").unwrap(), title);
}

#[tokio::test]
async fn test_create_dns_pdata_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let pdata_id = "some-data-id";
    let title = "Plugin Data Title";
    let item_title = "An Item";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let (val1, val2) = ("first-val", "second-val");

    call_fn(
        &mut con,
        function,
        &[
            "1", name, PLUGIN, "list", pdata_id, title, item_title, val1, val2,
        ],
    )
    .await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");
    let result_data: Vec<String> = con
        .lrange(&format!("{};{};{}", DNS_PDATA_KEY, &qname, pdata_id), 0, -1)
        .await
        .expect("Failed lrange.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!(
            "{};{};{};details",
            DNS_PDATA_KEY, &qname, pdata_id
        ))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert!(result_data.contains(&val1.to_string()));
    assert!(result_data.contains(&val2.to_string()));
    assert_eq!(result_details.get("type").unwrap(), "list");
    assert_eq!(result_details.get("plugin").unwrap(), PLUGIN);
    assert_eq!(result_details.get("list_title").unwrap(), title);
    assert_eq!(result_details.get("item_title").unwrap(), item_title);
}

#[tokio::test]
async fn test_create_node_pdata_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_plugin_data";
    let pdata_id = "some-data-id";
    let title = "Plugin Data Title";
    let item_title = "An Item";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let node_id = format!("{qname};{PLUGIN}");
    let (val1, val2) = ("first-val", "second-val");

    call_fn(
        &mut con,
        function,
        &[
            "1", name, PLUGIN, "list", pdata_id, title, item_title, val1, val2,
        ],
    )
    .await;

    let result_name: bool = con
        .sismember(NODES_KEY, &node_id)
        .await
        .expect("Failed sismember.");
    let result_data: Vec<String> = con
        .lrange(&format!("{NODE_PDATA_KEY};{node_id};{pdata_id}"), 0, -1)
        .await
        .expect("Failed lrange.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("{NODE_PDATA_KEY};{node_id};{pdata_id};details"))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert!(result_data.contains(&val1.to_string()));
    assert!(result_data.contains(&val2.to_string()));
    assert_eq!(result_details.get("type").unwrap(), "list");
    assert_eq!(result_details.get("plugin").unwrap(), PLUGIN);
    assert_eq!(result_details.get("list_title").unwrap(), title);
    assert_eq!(result_details.get("item_title").unwrap(), item_title);
}
