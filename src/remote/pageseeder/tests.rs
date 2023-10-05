use std::{env, fs};

use quick_xml::de;

use super::{config::parse_config, PSRemote};
use crate::remote::RemoteInterface;

fn remote() -> PSRemote {
    PSRemote {
        url: env::var("PS_TEST_URL").expect("Set environment variable PS_TEST_URL"),
        client_id: env::var("PS_TEST_ID").expect("Set environment variable PS_TEST_ID"),
        client_secret: env::var("PS_TEST_SECRET").expect("Set environment variable PS_TEST_SECRET"),
        group: env::var("PS_TEST_GROUP").expect("Set environment variable PS_TEST_GROUP"),
        username: env::var("PS_TEST_USER").expect("Set environment variable PS_TEST_USER"),
    }
}

#[test]
fn test_config() {
    let string = fs::read_to_string("test/config.psml").unwrap();
    let config = de::from_str(&string).unwrap();
    parse_config(config).unwrap();
}

#[tokio::test]
async fn test_config_remote() {
    remote().config().await.unwrap();
}

#[tokio::test]
async fn test_changelog() {
    remote().get_last_change().await.unwrap();
}
