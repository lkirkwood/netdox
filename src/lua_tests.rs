use redis::Connection;
use redis::{Client, Commands};
use std::collections::HashMap;
use std::path::PathBuf;
use std::{env, fs};

/// Calls a custom function with the specifies args, and unwraps the result.
fn call_fn(con: &mut Connection, function: &str, args: &[&str]) {
    let mut cmd = redis::cmd("fcall");
    cmd.arg(function);
    for arg in args {
        cmd.arg(arg);
    }
    cmd.query::<()>(con).expect(&format!(
        "Function call '{}' with failed with args {:?}",
        function, args
    ));
}

/// Sets constants required for data entry.
fn set_consts(con: &mut Connection) {
    redis::cmd("SET")
        .arg("default_network")
        .arg(DEFAULT_NETWORK)
        .query::<()>(con)
        .expect("Failed to set default network.");
}

/// Calls FLUSHALL and adds the required constants back.
fn reset_db(con: &mut Connection) {
    redis::cmd("FLUSHALL")
        .query::<()>(con)
        .expect("Failed on FLUSHALL");
    set_consts(con);
}

const TEST_REDIS_URL_VAR: &str = "NETDOX_TEST_REDIS_URL";
const LUA_FUNCTIONS_FILENAME: &str = "functions.lua";

fn setup_con() -> Connection {
    let url = env::var(TEST_REDIS_URL_VAR).expect(&format!(
        "Environment variable {TEST_REDIS_URL_VAR} must be set to test lua functions."
    ));
    let mut con = Client::open(url.as_str())
        .expect(&format!("Failed to create client with url {}", &url))
        .get_connection()
        .expect(&format!("Failed to open connection with url {}", &url));

    reset_db(&mut con);

    let mut lua_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lua_path.push(LUA_FUNCTIONS_FILENAME);

    let fn_content = fs::read_to_string(&lua_path).expect(&format!(
        "Failed to read content of redis functions at {:?}",
        &lua_path
    ));

    redis::cmd("FUNCTION")
        .arg("LOAD")
        .arg("REPLACE")
        .arg(fn_content)
        .query::<()>(&mut con)
        .expect("Failed to load functions into redis.");

    con
}

// CONSTANTS

const DEFAULT_NETWORK: &str = "default-net";
const PLUGIN: &str = "test-plugin";
const DNS_KEY: &str = "dns";
const NODES_KEY: &str = "nodes";

// TESTS

#[test]
fn test_create_dns_noval() {
    let mut con = setup_con();
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);

    // Unqualified
    call_fn(&mut con, &function, &["1", name, PLUGIN]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);

    // Qualified
    call_fn(&mut con, &function, &["1", &qname, PLUGIN]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
}

#[test]
fn test_create_dns_cname() {
    let mut con = setup_con();
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "CNAME";
    let value = "netdox.org";

    // Unqualified
    call_fn(&mut con, &function, &["1", name, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");
    let result_value: bool = con
        .sismember(
            format!("{};{};{};{}", DNS_KEY, &qname, PLUGIN, &rtype),
            format!("[{DEFAULT_NETWORK}]{value}"),
        )
        .expect("Failed sismember.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_value, true);

    // Qualified
    call_fn(&mut con, &function, &["1", &qname, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_value, true);
}

#[test]
fn test_create_dns_a() {
    let mut con = setup_con();
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "A";
    let value = "192.168.0.1";

    // Unqualified
    call_fn(&mut con, &function, &["1", name, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");
    let result_value: bool = con
        .sismember(
            format!("{};{};{};{}", DNS_KEY, &qname, PLUGIN, &rtype),
            format!("[{DEFAULT_NETWORK}]{value}"),
        )
        .expect("Failed sismember.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_value, true);

    // Qualified
    call_fn(&mut con, &function, &["1", &qname, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_value, true);
}

#[test]
fn test_map_dns_norev() {
    let mut con = setup_con();
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
    );

    let result_origin_dns: bool = con.sismember(DNS_KEY, &qorigin).expect("Failed sismember.");
    let result_dest1_dns: bool = con.sismember(DNS_KEY, &qdest1).expect("Failed sismember.");
    let result_dest2_dns: bool = con.sismember(DNS_KEY, &qdest2).expect("Failed sismember.");

    let result_origin_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qorigin), PLUGIN)
        .expect("Failed sismember.");
    let result_dest1_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest1), PLUGIN)
        .expect("Failed sismember.");
    let result_dest2_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest2), PLUGIN)
        .expect("Failed sismember.");

    let result_map: HashMap<String, String> = con
        .hgetall(&format!("{};{};maps", DNS_KEY, &qorigin))
        .expect("Failed hgetall.");

    assert_eq!(result_origin_dns, true);
    assert_eq!(result_dest1_dns, true);
    assert_eq!(result_dest2_dns, true);

    assert_eq!(result_origin_plugins, true);
    assert_eq!(result_dest1_plugins, true);
    assert_eq!(result_dest2_plugins, true);

    assert_eq!(result_map.get(dest1_net), Some(&dest1_name.to_string()));
    assert_eq!(result_map.get(dest2_net), Some(&dest2_name.to_string()));
}

#[test]
fn test_map_dns_rev() {
    let mut con = setup_con();
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
    );

    let result_origin_dns: bool = con.sismember(DNS_KEY, &qorigin).expect("Failed sismember.");
    let result_dest1_dns: bool = con.sismember(DNS_KEY, &qdest1).expect("Failed sismember.");
    let result_dest2_dns: bool = con.sismember(DNS_KEY, &qdest2).expect("Failed sismember.");

    let result_origin_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qorigin), PLUGIN)
        .expect("Failed sismember.");
    let result_dest1_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest1), PLUGIN)
        .expect("Failed sismember.");
    let result_dest2_plugins: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qdest2), PLUGIN)
        .expect("Failed sismember.");

    let result_fmap: HashMap<String, String> = con
        .hgetall(&format!("{};{};maps", DNS_KEY, &qorigin))
        .expect("Failed hgetall.");
    let result_rdest1: Option<String> = con
        .hget(
            &format!("{};{};maps", DNS_KEY, &qdest1),
            &format!("[{}]", DEFAULT_NETWORK),
        )
        .expect("Failed hget.");
    let result_rdest2: Option<String> = con
        .hget(
            &format!("{};{};maps", DNS_KEY, &qdest2),
            &format!("[{}]", DEFAULT_NETWORK),
        )
        .expect("Failed hget.");

    assert_eq!(result_origin_dns, true);
    assert_eq!(result_dest1_dns, true);
    assert_eq!(result_dest2_dns, true);

    assert_eq!(result_origin_plugins, true);
    assert_eq!(result_dest1_plugins, true);
    assert_eq!(result_dest2_plugins, true);

    assert_eq!(result_fmap.get(dest1_net), Some(&dest1_name.to_string()));
    assert_eq!(result_fmap.get(dest2_net), Some(&dest2_name.to_string()));

    assert_eq!(result_rdest1, Some(origin.to_string()));
    assert_eq!(result_rdest2, Some(origin.to_string()));
}

// TODO add test for soft and exclusive??
#[test]
fn test_create_node_soft() {
    let mut con = setup_con();
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let node_id = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");

    call_fn(&mut con, function, &["2", domain, ip, PLUGIN, name]);

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");

    let result_plugins: bool = con
        .sismember(format!("{};{};plugins", NODES_KEY, &node_id), PLUGIN)
        .expect("Failed sismember.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{};{};{}", NODES_KEY, &node_id, PLUGIN))
        .expect("Failed hgetall.");

    assert_eq!(result_all_nodes, true);
    assert_eq!(result_plugins, true);
    assert_eq!(result_details.get("name"), Some(&name.to_string()));
    assert_eq!(result_details.get("link_id"), None);
    assert_eq!(result_details.get("exclusive"), Some(&"false".to_string()));
}

#[test]
fn test_create_node_no_exc() {
    let mut con = setup_con();
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let link_id = "node-link-id";
    let node_id = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let exclusive = "false";

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, name, exclusive, link_id],
    );

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");

    let result_plugins: bool = con
        .sismember(format!("{};{};plugins", NODES_KEY, &node_id), PLUGIN)
        .expect("Failed sismember.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{};{};{}", NODES_KEY, &node_id, PLUGIN))
        .expect("Failed hgetall.");

    assert_eq!(result_all_nodes, true);
    assert_eq!(result_plugins, true);
    assert_eq!(result_details.get("name"), Some(&name.to_string()));
    assert_eq!(result_details.get("link_id"), Some(&link_id.to_string()));
    assert_eq!(
        result_details.get("exclusive"),
        Some(&exclusive.to_string())
    );
}

#[test]
fn test_create_node_exc() {
    let mut con = setup_con();
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let link_id = "node-link-id";
    let node_id = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let exclusive = "true";

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, name, exclusive, link_id],
    );

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");

    let result_plugins: bool = con
        .sismember(format!("{};{};plugins", NODES_KEY, &node_id), PLUGIN)
        .expect("Failed sismember.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{};{};{}", NODES_KEY, &node_id, PLUGIN))
        .expect("Failed hgetall.");

    assert_eq!(result_all_nodes, true);
    assert_eq!(result_plugins, true);
    assert_eq!(result_details.get("name"), Some(&name.to_string()));
    assert_eq!(result_details.get("link_id"), Some(&link_id.to_string()));
    assert_eq!(
        result_details.get("exclusive"),
        Some(&exclusive.to_string())
    );
}

#[test]
fn test_create_dns_metadata() {
    let mut con = setup_con();
    let function = "netdox_create_dns_metadata";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(&mut con, "netdox_create_dns", &["1", name, PLUGIN]);
    call_fn(
        &mut con,
        function,
        &["1", name, PLUGIN, key1, val1, key2, val2],
    );

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", DNS_KEY, &qname))
        .expect("Failed hgetall.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[test]
fn test_create_dns_metadata_new() {
    let mut con = setup_con();
    let function = "netdox_create_dns_metadata";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &["1", name, PLUGIN, key1, val1, key2, val2],
    );

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", DNS_KEY, &qname))
        .expect("Failed hgetall.");

    assert_eq!(result_name, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[test]
fn test_create_node_metadata_linkable() {
    let mut con = setup_con();
    let function = "netdox_create_node_metadata";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let node_id = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        "netdox_create_node",
        &["2", domain, ip, PLUGIN, "node-name", "false", "link-id"],
    );
    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    );

    let result_node: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", NODES_KEY, node_id), PLUGIN)
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", NODES_KEY, node_id))
        .expect("Failed hgetall.");

    assert_eq!(result_node, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[test]
fn test_create_node_metadata_soft() {
    let mut con = setup_con();
    let function = "netdox_create_node_metadata";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let node_id = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        "netdox_create_node",
        &["2", domain, ip, PLUGIN, "node-name"],
    );
    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    );

    let result_node: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", NODES_KEY, node_id), PLUGIN)
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", NODES_KEY, node_id))
        .expect("Failed hgetall.");

    assert_eq!(result_node, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}

#[test]
fn test_create_node_metadata_new() {
    let mut con = setup_con();
    let function = "netdox_create_node_metadata";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let node_id = format!("[{DEFAULT_NETWORK}]{ip};[{DEFAULT_NETWORK}]{domain}");
    let (key1, val1) = ("first-key", "first-val");
    let (key2, val2) = ("second-key", "second-val");

    call_fn(
        &mut con,
        function,
        &["2", domain, ip, PLUGIN, key1, val1, key2, val2],
    );

    let result_node: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(&format!("{};{};plugins", NODES_KEY, node_id), PLUGIN)
        .expect("Failed sismember.");
    let result_details: HashMap<String, String> = con
        .hgetall(&format!("meta;{};{}", NODES_KEY, node_id))
        .expect("Failed hgetall.");

    assert_eq!(result_node, true);
    assert_eq!(result_plugin, true);
    assert_eq!(result_details.get(key1), Some(&val1.to_string()));
    assert_eq!(result_details.get(key2), Some(&val2.to_string()));
}
