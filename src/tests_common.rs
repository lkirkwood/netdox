use std::{env, fs, path::PathBuf};

use redis::{Client, Connection};

/// Calls a custom function with the specifies args, and unwraps the result.
pub fn call_fn(con: &mut Connection, function: &str, args: &[&str]) {
    let mut cmd = redis::cmd("fcall");
    cmd.arg(function);
    for arg in args {
        cmd.arg(arg);
    }
    if let Err(err) = cmd.query::<()>(con) {
        panic!(
            "Function call '{}' with failed with args: '{:?}' and error message '{}'",
            function,
            args,
            err.to_string()
        )
    }
}

/// Sets constants required for data entry.
pub fn set_consts(con: &mut Connection) {
    redis::cmd("SET")
        .arg("default_network")
        .arg(DEFAULT_NETWORK)
        .query::<()>(con)
        .expect("Failed to set default network.");
}

/// Calls FLUSHALL and adds the required constants back.
pub fn reset_db(con: &mut Connection) {
    redis::cmd("FLUSHALL")
        .query::<()>(con)
        .expect("Failed on FLUSHALL");
    set_consts(con);
}

/// Name of the environment variable that contains the test redis server URL.
pub const TEST_REDIS_URL_VAR: &str = "NETDOX_TEST_REDIS_URL";
/// File in the root of the project that contains the custom lua functions for redis.
pub const LUA_FUNCTIONS_FILENAME: &str = "functions.lua";

/// Connects to the database, flushes it, and runs setup commands.
pub fn setup_db() -> Client {
    let url = env::var(TEST_REDIS_URL_VAR).unwrap_or_else(|_| {
        panic!("Environment variable {TEST_REDIS_URL_VAR} must be set to test lua functions.")
    });
    let client = Client::open(url.as_str())
        .unwrap_or_else(|_| panic!("Failed to create client with url {}", &url));

    let mut con = client
        .get_connection()
        .unwrap_or_else(|_| panic!("Failed to open connection with url {}", &url));

    reset_db(&mut con);

    let mut lua_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    lua_path.push(LUA_FUNCTIONS_FILENAME);

    let fn_content = fs::read_to_string(&lua_path).unwrap_or_else(|_| {
        panic!(
            "Failed to read content of redis functions at {:?}",
            &lua_path
        )
    });

    redis::cmd("FUNCTION")
        .arg("LOAD")
        .arg("REPLACE")
        .arg(fn_content)
        .query::<()>(&mut con)
        .expect("Failed to load functions into redis.");

    client
}

pub fn setup_db_con() -> Connection {
    setup_db()
        .get_connection()
        .expect("Failed to get connection to test redis from client")
}

// CONSTANTS

/// Default network to use for testing.
pub const DEFAULT_NETWORK: &str = "default-net";
/// Plugin to use for testing.
pub const PLUGIN: &str = "test-plugin";
