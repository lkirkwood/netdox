use std::collections::HashSet;

use crate::tests_common::TEST_REDIS_URL_VAR;

use super::model::Node;
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
        dns_names: HashSet::from(["domain.com".to_string()]),
        plugins: HashSet::from(["some-plugin".to_string()]),
        raw_ids: HashSet::from(["[some-net]domain.com".to_string()]),
    };

    expected.write(&mut con).await.unwrap();
    let actual = Node::read(&mut con, &expected.link_id).await.unwrap();

    assert_eq!(expected, actual);
}

// TODO add plugin data tests
