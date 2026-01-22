use std::{env, sync::LazyLock};

use redis::{aio::MultiplexedConnection, Client};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{config::LocalConfig, data::DataConn, remote::DummyRemote};

pub static TIMESTAMP: LazyLock<u64> = LazyLock::new(|| {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
});

/// Calls a custom function with the specifies args, and unwraps the result.
pub async fn call_fn(con: &mut MultiplexedConnection, function: &str, args: &[&str]) {
    let mut cmd = redis::cmd("fcall");
    cmd.arg(function);
    for arg in args {
        cmd.arg(arg);
    }
    if let Err(err) = cmd.query_async::<_, ()>(con).await {
        panic!(
            "Function call '{function}' with failed with args: '{args:?}' and error message '{err}'"
        )
    }
}

/// Name of the environment variable that contains the test redis server URL.
pub const TEST_REDIS_URL_VAR: &str = "NETDOX_TEST_REDIS_URL";

/// Connects to the database, flushes it, and runs setup commands.
pub async fn setup_db() -> Client {
    let url = env::var(TEST_REDIS_URL_VAR).unwrap_or_else(|_| {
        panic!("Environment variable {TEST_REDIS_URL_VAR} must be set to test lua functions.")
    });
    let client = Client::open(url.as_str())
        .unwrap_or_else(|_| panic!("Failed to create client with url {}", &url));

    let mut con = client
        .get_multiplexed_tokio_connection()
        .await
        .unwrap_or_else(|_| panic!("Failed to open connection with url {}", &url));

    let mut cfg = LocalConfig::template(crate::remote::Remote::Dummy(DummyRemote {
        field: "".to_string(),
    }));
    cfg.default_network = DEFAULT_NETWORK.to_string();
    con.setup(&cfg).await.unwrap();

    client
}

pub async fn setup_db_con() -> MultiplexedConnection {
    setup_db()
        .await
        .get_multiplexed_tokio_connection()
        .await
        .expect("Failed to get connection to test redis from client")
}

// CONSTANTS

/// Default network to use for testing.
pub const DEFAULT_NETWORK: &str = "default-net";
/// Plugin to use for testing.
pub const PLUGIN: &str = "test-plugin";
