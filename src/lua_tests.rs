mod changelog;

use crate::data::model::{DNS_KEY, NODES_KEY, PDATA_KEY, REPORTS_KEY};
use crate::tests_common::*;
use redis::AsyncCommands;
use std::collections::{HashMap, HashSet};

#[tokio::test]
async fn test_create_dns_noval() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let name = "dns-noval.com";
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
    let name = "dns-cname.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "CNAME";
    let value = "dns-cname.net";

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
    let name = "dns-a.com";
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
    let origin = "map-dns-norev.com";
    let qorigin = format!("[{}]{}", DEFAULT_NETWORK, origin);
    let reverse = "false";

    let dest1_net = "[org-net]";
    let dest1_name = "map-dns-norev.org";
    let qdest1 = format!("{}{}", dest1_net, dest1_name);
    let dest2_net = "[gov-net]";
    let dest2_name = "map-dns-norev.gov";
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
    let origin = "map-dns-rev.com";
    let qorigin = format!("[{}]{}", DEFAULT_NETWORK, origin);
    let reverse = "true";

    let dest1_net = "[org-net]";
    let dest1_name = "map-dns-rev.org";
    let qdest1 = format!("{}{}", dest1_net, dest1_name);
    let dest2_net = "[gov-net]";
    let dest2_name = "map-dns-rev.gov";
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
    let domain = "new-node.com";
    let ip = "192.168.0.2";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");

    call_fn(&mut con, function, &["2", domain, ip, PLUGIN, name]).await;

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(format!("{NODES_KEY};{qnames}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{NODES_KEY};{qnames};{result_count}"))
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
    let domain = "new-node-no-exc.com";
    let ip = "192.168.0.3";
    let link_id = "no-exc-node-id";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let exclusive = "false";

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, name, exclusive, link_id],
    )
    .await;

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(format!("{NODES_KEY};{qnames}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{NODES_KEY};{qnames};{result_count}"))
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
    let domain = "exc-node.com";
    let ip = "192.168.0.4";
    let link_id = "exc-node-id";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let exclusive = "true";

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, name, exclusive, link_id],
    )
    .await;

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(format!("{NODES_KEY};{qnames}"))
        .await
        .expect("Failed to get int.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{NODES_KEY};{qnames};{result_count}"))
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
    let name = "metadata.com";
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
    let result_plugins: HashSet<String> = con
        .smembers(&format!("meta;{};{};plugins", DNS_KEY, &qname))
        .await
        .expect("Failed smembers.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", DNS_KEY, &qname))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert!(result_plugins.contains(PLUGIN));
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_dns_metadata_new() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let name = "metadata-new.com";
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
    let result_plugins: HashSet<String> = con
        .smembers(&format!("meta;{};{};plugins", DNS_KEY, &qname))
        .await
        .expect("Failed smembers.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", DNS_KEY, &qname))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert!(result_plugins.contains(PLUGIN));
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_node_metadata_linkable() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_metadata";
    let domain = "metadata-link-node.com";
    let ip = "192.168.0.5";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
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
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(&format!("{NODES_KEY};{qnames}"))
        .await
        .expect("Failed to get int.");

    let result_plugins: HashSet<String> = con
        .smembers(&format!("meta;{NODES_KEY};{qnames};plugins"))
        .await
        .expect("Failed smembers.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{NODES_KEY};{qnames}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_node);
    assert!(result_plugins.contains(PLUGIN));
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_node_metadata_soft() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_metadata";
    let domain = "metadata-node-soft.com";
    let ip = "192.168.0.6";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
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
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(&format!("{NODES_KEY};{qnames}"))
        .await
        .expect("Failed to get int.");

    let result_plugins: HashSet<String> = con
        .smembers(&format!("meta;{NODES_KEY};{qnames};plugins"))
        .await
        .expect("Failed smembers.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{NODES_KEY};{qnames}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_node);
    assert!(result_plugins.contains(PLUGIN));
    assert_eq!(result_count, 1);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[tokio::test]
async fn test_create_node_metadata_new() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_metadata";
    let domain = "metadata-node-new.com";
    let ip = "192.168.0.7";
    let qnames = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    )
    .await;

    let result_node: bool = con
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_count: u64 = con
        .get(&format!("{NODES_KEY};{qnames}"))
        .await
        .expect("Failed to get int.");

    let result_plugins: HashSet<String> = con
        .smembers(&format!("meta;{NODES_KEY};{qnames};plugins"))
        .await
        .expect("Failed smembers.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{NODES_KEY};{qnames}"))
        .await
        .expect("Failed hgetall.");

    assert!(result_node);
    assert!(result_plugins.contains(PLUGIN));
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
    let name = "hash-pdata-dns.com";
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
        .hgetall(&format!("{PDATA_KEY};{DNS_KEY};{qname};{pdata_id}"))
        .await
        .expect("Failed hgetall.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("{PDATA_KEY};{DNS_KEY};{qname};{pdata_id};details"))
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
    let name = "hash-pdata-node.com";
    let qnames = format!("[{}]{}", DEFAULT_NETWORK, name);
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
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");
    let result_data: HashMap<String, String> = con
        .hgetall(&format!("{PDATA_KEY};{NODES_KEY};{qnames};{pdata_id}"))
        .await
        .expect("Failed hgetall.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!(
            "{PDATA_KEY};{NODES_KEY};{qnames};{pdata_id};details"
        ))
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
    let name = "list-pdata-dns.com";
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
        .lrange(&format!("{PDATA_KEY};{DNS_KEY};{qname};{pdata_id}"), 0, -1)
        .await
        .expect("Failed lrange.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("{PDATA_KEY};{DNS_KEY};{qname};{pdata_id};details"))
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
    let name = "list-pdata-node.com";
    let qnames = format!("[{}]{}", DEFAULT_NETWORK, name);
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
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");
    let result_data: Vec<String> = con
        .lrange(
            &format!("{PDATA_KEY};{NODES_KEY};{qnames};{pdata_id}"),
            0,
            -1,
        )
        .await
        .expect("Failed lrange.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!(
            "{PDATA_KEY};{NODES_KEY};{qnames};{pdata_id};details"
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
async fn test_create_dns_pdata_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let pdata_id = "some-data-id";
    let title = "Plugin Data Title";
    let columns = "4";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);

    call_fn(
        &mut con,
        function,
        &[
            "1", name, PLUGIN, "table", pdata_id, title, columns, // details
            "blue", "large", "12", "4.2", // first row
            "yellow", "small", "450", "N/A", // second row
        ],
    )
    .await;

    let result_name: bool = con
        .sismember(DNS_KEY, &qname)
        .await
        .expect("Failed sismember.");

    let result_data: Vec<String> = con
        .lrange(&format!("{PDATA_KEY};{DNS_KEY};{qname};{pdata_id}"), 0, -1)
        .await
        .expect("Failed lrange.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!("{PDATA_KEY};{DNS_KEY};{qname};{pdata_id};details"))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert_eq!(
        result_data,
        vec![
            "blue", "large", "12", "4.2", // first row
            "yellow", "small", "450", "N/A", // second row
        ]
    );
    assert_eq!(result_details.get("type").unwrap(), "table");
    assert_eq!(result_details.get("plugin").unwrap(), PLUGIN);
    assert_eq!(result_details.get("title").unwrap(), title);
}

#[tokio::test]
async fn test_create_node_pdata_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node_plugin_data";
    let pdata_id = "some-data-id";
    let title = "Plugin Data Title";
    let columns = "4";
    let name = "netdox.com";
    let qnames = format!("[{}]{}", DEFAULT_NETWORK, name);

    call_fn(
        &mut con,
        function,
        &[
            "1", name, PLUGIN, "table", pdata_id, title, columns, // details
            "blue", "large", "12", "4.2", // first row
            "yellow", "small", "450", "N/A", // second row
        ],
    )
    .await;

    let result_name: bool = con
        .sismember(NODES_KEY, &qnames)
        .await
        .expect("Failed sismember.");

    let result_data: Vec<String> = con
        .lrange(
            &format!("{PDATA_KEY};{NODES_KEY};{qnames};{pdata_id}"),
            0,
            -1,
        )
        .await
        .expect("Failed lrange.");

    let result_details: HashMap<String, String> = con
        .hgetall(&format!(
            "{PDATA_KEY};{NODES_KEY};{qnames};{pdata_id};details"
        ))
        .await
        .expect("Failed hgetall.");

    assert!(result_name);
    assert_eq!(
        result_data,
        vec![
            "blue", "large", "12", "4.2", // first row
            "yellow", "small", "450", "N/A", // second row
        ]
    );
    assert_eq!(result_details.get("type").unwrap(), "table");
    assert_eq!(result_details.get("plugin").unwrap(), PLUGIN);
    assert_eq!(result_details.get("title").unwrap(), title);
}

#[tokio::test]
async fn test_create_report() {
    let mut con = setup_db_con().await;
    let create_report = "netdox_create_report";
    let id = "report_id";
    let title = "Report Title";
    let length = "3";
    call_fn(&mut con, create_report, &["1", id, PLUGIN, title, length]).await;

    let create_data = "netdox_create_report_data";

    let data1 = HashMap::from([
        ("key1".to_string(), "val1".to_string()),
        ("key2".to_string(), "val2".to_string()),
    ]);
    let data1_title = "Map Title :)";

    call_fn(
        &mut con,
        create_data,
        &[
            "1",
            id,
            PLUGIN,
            "0",
            "hash",
            data1_title,
            // map content
            "key1",
            "val1",
            "key2",
            "val2",
        ],
    )
    .await;

    let actual1: HashMap<String, String> =
        con.hgetall(format!("{REPORTS_KEY};{id};0")).await.unwrap();
    assert_eq!(actual1, data1);

    let data2 = vec![
        "item1".to_string(),
        "item2".to_string(),
        "item3".to_string(),
    ];
    let data2_ltitle = "List Title";
    let data2_ititle = "An Item";

    call_fn(
        &mut con,
        create_data,
        &[
            "1",
            id,
            PLUGIN,
            "1",
            "list",
            data2_ltitle,
            data2_ititle,
            "item1",
            "item2",
            "item3",
        ],
    )
    .await;

    let actual2: Vec<String> = con
        .lrange(format!("{REPORTS_KEY};{id};1"), 0, -1)
        .await
        .unwrap();
    assert_eq!(actual2, data2);

    let data3 = "Third Datum!";
    let data3_title = "String Title";
    let data3_ctype = "plain";

    call_fn(
        &mut con,
        create_data,
        &[
            "1",
            id,
            PLUGIN,
            "2",
            "string",
            data3_title,
            data3_ctype,
            data3,
        ],
    )
    .await;

    let actual3: String = con.get(format!("{REPORTS_KEY};{id};2")).await.unwrap();
    assert_eq!(actual3, data3);
}
