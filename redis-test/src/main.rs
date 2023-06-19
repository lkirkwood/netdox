use std::{
    collections::HashMap,
    fs,
    io::{stdin, stdout, Write},
    path::PathBuf,
};

use redis::{Client, Commands, Connection};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Args {
    /// URL of the redis instance to write to for testing.
    url: String,
    /// Absolute path to the file containing the functions to test.
    functions: PathBuf,
}

fn main() {
    let args = Args::from_args();
    confirm_url(&args.url);

    let mut con = Client::open(args.url.as_str())
        .expect(&format!("Failed to create client with url {}", &args.url))
        .get_connection()
        .expect(&format!("Failed to open connection with url {}", &args.url));

    let fn_content = fs::read_to_string(&args.functions).expect(&format!(
        "Failed to read content of redis functions at {:?}",
        &args.functions
    ));

    redis::cmd("FUNCTION")
        .arg("LOAD")
        .arg("REPLACE")
        .arg(fn_content)
        .query::<()>(&mut con)
        .expect("Failed to load functions into redis.");
    set_consts(&mut con);

    // Run tests
    let mut results = HashMap::new();
    println!("Running tests...");
    results.insert("create_dns no value", test_create_dns_noval(&mut con));
    results.insert("create_dns cname record", test_create_dns_cname(&mut con));
    results.insert("create_dns a record", test_create_dns_a(&mut con));

    evaluate_results(&&results);
}

// UTILS

/// Asks the user to confirm the url of the redis server is correct.
/// Exits unless confirmation is provided.
fn confirm_url(url: &str) {
    print!(
        "Confirm it is OK to write to redis instance at ({}) y/N: ",
        url
    );
    stdout().flush().unwrap();
    let mut response = String::new();
    stdin().read_line(&mut response).unwrap();
    if response.to_lowercase().trim() != "y" {
        println!("Stopping...");
        std::process::exit(0);
    }
}

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
fn flush(con: &mut Connection) {
    redis::cmd("FLUSHALL")
        .query::<()>(con)
        .expect("Failed on FLUSHALL");
    set_consts(con);
}

type TestResult = Result<(), &'static str>;

/// Evaluates a map of test results.
fn evaluate_results(results: &HashMap<&str, TestResult>) {
    println!(
        "{} out of {} tests completed successfully.",
        results.iter().filter(|t| t.1.is_ok()).count(),
        results.len()
    );

    for (test, result) in results {
        if result.is_err() {
            println!("Test {} failed: {}", test, result.unwrap_err());
        }
    }
}

// CONSTANTS

const DEFAULT_NETWORK: &str = "default-net";
const PLUGIN: &str = "test-plugin";
const DNS_KEY: &str = "dns";

// TESTS

fn test_create_dns_noval(con: &mut Connection) -> TestResult {
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);

    // Unqualified
    call_fn(con, &function, &["1", name, PLUGIN]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    flush(con);
    if !result_name {
        return Err("Set of all DNS names missing new name after create_dns \
            with unqualified name.");
    } else if !result_plugin {
        return Err(
            "Set of plugins for new DNS name missing value after create_dns \
            with unqualified name.",
        );
    }

    // Qualified
    call_fn(con, &function, &["1", &qname, PLUGIN]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    flush(con);
    if !result_name {
        return Err("Set of all DNS names missing new name after create_dns \
            with qualified name.");
    } else if !result_plugin {
        return Err(
            "Set of plugins for new DNS name missing value after create_dns \
            with qualified name.",
        );
    }

    return Ok(());
}

fn test_create_dns_cname(con: &mut Connection) -> TestResult {
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "CNAME";
    let value = "netdox.org";

    // Unqualified
    call_fn(con, &function, &["1", name, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");
    let result_value: bool = con
        .sismember(
            format!("{};{};{};{}", DNS_KEY, &qname, PLUGIN, &rtype),
            value,
        )
        .expect("Failed sismember.");

    flush(con);
    if !result_name {
        return Err("Set of all DNS names missing new name after create_dns \
            with unqualified name.");
    } else if !result_plugin {
        return Err(
            "Set of plugins for new DNS name missing value after create_dns \
            with unqualified name.",
        );
    } else if !result_value {
        return Err(
            "Set of values for CNAME records missing value after create_dns \
            with unqualified name.",
        );
    }

    // Qualified
    call_fn(con, &function, &["1", &qname, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    flush(con);
    if !result_name {
        return Err("Set of all DNS names missing new name after create_dns \
            with qualified name.");
    } else if !result_plugin {
        return Err(
            "Set of plugins for new DNS name missing value after create_dns \
            with qualified name.",
        );
    } else if !result_value {
        return Err(
            "Set of values for CNAME records missing value after create_dns \
            with qualified name.",
        );
    }

    return Ok(());
}

fn test_create_dns_a(con: &mut Connection) -> TestResult {
    let function = "netdox_create_dns";
    let name = "netdox.com";
    let qname = format!("[{}]{}", DEFAULT_NETWORK, name);
    let rtype = "A";
    let value = "192.168.0.1";

    // Unqualified
    call_fn(con, &function, &["1", name, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");
    let result_value: bool = con
        .sismember(
            format!("{};{};{};{}", DNS_KEY, &qname, PLUGIN, &rtype),
            value,
        )
        .expect("Failed sismember.");

    flush(con);
    if !result_name {
        return Err("Set of all DNS names missing new name after create_dns \
            with unqualified name.");
    } else if !result_plugin {
        return Err(
            "Set of plugins for new DNS name missing value after create_dns \
            with unqualified name.",
        );
    } else if !result_value {
        return Err(
            "Set of values for A records missing value after create_dns \
            with unqualified name.",
        );
    }

    // Qualified
    call_fn(con, &function, &["1", &qname, PLUGIN, rtype, value]);

    let result_name: bool = con.sismember(DNS_KEY, &qname).expect("Failed sismember.");
    let result_plugin: bool = con
        .sismember(format!("{};{};plugins", DNS_KEY, &qname), PLUGIN)
        .expect("Failed sismember.");

    flush(con);
    if !result_name {
        return Err("Set of all DNS names missing new name after create_dns \
            with qualified name.");
    } else if !result_plugin {
        return Err(
            "Set of plugins for new DNS name missing value after create_dns \
            with qualified name.",
        );
    } else if !result_value {
        return Err(
            "Set of values for A records missing value after create_dns \
            with qualified name.",
        );
    }

    return Ok(());
}

// TODO add test for soft and exclusive??
fn test_create_node_soft(con: &mut Connection) -> TestResult {
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let node_id = format!("{};{}", ip, domain);

    call_fn(con, function, &["2", domain, ip, PLUGIN, name]);

    let result_all_nodes: bool = con
        .sismember(NODES_KEY, &node_id)
        .expect("Failed sismember.");

    let result_plugins: bool = con
        .sismember(format!("{};{};plugins", NODES_KEY, &node_id), PLUGIN)
        .expect("Failed sismember.");

    let result_details: HashMap<String, String> = con
        .hgetall(format!("{};{};{}", NODES_KEY, &node_id, PLUGIN))
        .expect("Failed hgetall.");

    // flush(con);
    if !result_all_nodes {
        return Err("Set of all nodes missing value after create_node \
                    not exclusive and no link_id.");
    } else if !result_plugins {
        return Err("Set of plugins for node missing value after create_node \
                    not exclusive and no link_id.");
    } else if result_details.get("name") != Some(&name.to_string()) {
        return Err("Value for node name is incorrect after create_node \
                    not exclusive and no link_id.");
    } else if result_details.get("link_id") != None {
        return Err("Value for node link_id is incorrect after create_node \
                    not exclusive and no link_id.");
    }

    let _exclusive = result_details.get("exclusive");
    if _exclusive == None || _exclusive.unwrap() != "false" {
        return Err("Value for node exclusive is incorrect after create_node \
                    not exclusive and no link_id.");
    }

    return Ok(());
}

fn test_create_node_no_exc(con: &mut Connection) -> TestResult {
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let link_id = "node-link-id";
    let node_id = format!("{};{}", ip, domain);
    let exclusive = "false";

    call_fn(
        con,
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

    flush(con);
    if !result_all_nodes {
        return Err("Set of all nodes missing value after create_node \
                    not exclusive.");
    } else if !result_plugins {
        return Err("Set of plugins for node missing value after create_node \
                    not exclusive.");
    } else if result_details.get("name") != Some(&name.to_string()) {
        return Err("Value for node name is incorrect after create_node \
                    not exclusive.");
    }
    let _exclusive = result_details.get("exclusive");
    if _exclusive == None || _exclusive.unwrap() != exclusive {
        return Err("Value for node exclusive is incorrect after create_node \
                    not exclusive.");
    }
    let _link_id = result_details.get("link_id");
    if _link_id == None || _link_id.unwrap() != link_id {
        return Err("Value for node link_id is incorrect after create_node \
                not exclusive.");
    }

    return Ok(());
}

fn test_create_node_exc(con: &mut Connection) -> TestResult {
    let function = "netdox_create_node";

    let name = "new-node";
    let domain = "netdox.com";
    let ip = "192.168.0.1";
    let link_id = "node-link-id";
    let node_id = format!("{};{}", ip, domain);
    let exclusive = "true";

    call_fn(
        con,
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

    flush(con);
    if !result_all_nodes {
        return Err("Set of all nodes missing value after create_node \
                    exclusive.");
    } else if !result_plugins {
        return Err("Set of plugins for node missing value after create_node \
                    exclusive.");
    } else if result_details.get("name") != Some(&name.to_string()) {
        return Err("Value for node name is incorrect after create_node \
                    exclusive.");
    }
    let _exclusive = result_details.get("exclusive");
    if _exclusive == None || _exclusive.unwrap() != "true" {
        return Err("Value for node exclusive is incorrect after create_node \
                    exclusive.");
    }
    let _link_id = result_details.get("link_id");
    if _link_id == None || _link_id.unwrap() != link_id {
        return Err("Value for node link_id is incorrect after create_node \
                exclusive.");
    }

    return Ok(());
}
