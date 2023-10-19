use std::collections::HashSet;

use crate::tests_common::{PLUGIN, TEST_REDIS_URL_VAR};

use super::{model::Node, Datastore};
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

    expected.write(&mut con).await.unwrap();
    let actual = Node::read(&mut con, &expected.link_id).await.unwrap();

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

    expected.write(&mut con).await.unwrap();

    assert_eq!(con.get_dns_node_id(&qname).await.unwrap(), link_id)
}
