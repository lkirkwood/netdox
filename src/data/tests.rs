use std::collections::HashSet;

use crate::tests_common::{PLUGIN, TEST_REDIS_URL_VAR};

use super::{model::Node, store::DataConn};
use std::env;

#[tokio::test]
async fn test_node_roundtrip() {
    let mut con = redis::Client::open(env::var(TEST_REDIS_URL_VAR).unwrap())
        .unwrap()
        .get_async_connection()
        .await
        .unwrap();

    let expected = Node {
        link_id: "linkable-id".to_string(),
        name: "Node Name".to_string(),
        alt_names: HashSet::from(["Other Node Name".to_string()]),
        dns_names: HashSet::from(["[some-net]domain.com".to_string()]),
        plugins: HashSet::from(["some-plugin".to_string()]),
        raw_ids: HashSet::from(["[some-net]domain.com".to_string()]),
    };

    con.put_node(&expected).await.unwrap();
    let actual = con.get_node(&expected.link_id).await.unwrap();

    assert_eq!(expected, actual);
}

// TODO add plugin data tests

#[tokio::test]
async fn test_get_dns_node() {
    let mut con = redis::Client::open(env::var(TEST_REDIS_URL_VAR).unwrap())
        .unwrap()
        .get_async_connection()
        .await
        .unwrap();

    let qname = "[some-other-net]domain.net".to_string();
    let link_id = "dns-node-id".to_string();

    let expected = Node {
        link_id: link_id.clone(),
        name: "Node Name".to_string(),
        alt_names: HashSet::from(["Other Node Name".to_string()]),
        dns_names: HashSet::from([qname.clone()]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([qname.clone()]),
    };

    con.put_node(&expected).await.unwrap();

    assert_eq!(con.get_dns_node_id(&qname).await.unwrap(), Some(link_id));
}

#[tokio::test]
async fn test_get_dns_node_none() {
    let mut con = redis::Client::open(env::var(TEST_REDIS_URL_VAR).unwrap())
        .unwrap()
        .get_async_connection()
        .await
        .unwrap();

    let qname = "[somenode-net]domain.com".to_string();
    let other_qname = "[nonode-net]domain.com".to_string();
    let link_id = "dns-nonode-id".to_string();

    let expected = Node {
        link_id: link_id.clone(),
        name: "Node Name".to_string(),
        alt_names: HashSet::from(["Other Node Name".to_string()]),
        dns_names: HashSet::from([qname.clone()]),
        plugins: HashSet::from([PLUGIN.to_string()]),
        raw_ids: HashSet::from([qname.clone()]),
    };

    con.put_node(&expected).await.unwrap();

    assert_eq!(con.get_dns_node_id(&other_qname).await.unwrap(), None);
}
