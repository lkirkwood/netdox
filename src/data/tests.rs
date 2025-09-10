use std::collections::HashSet;

use crate::{data::model::StringType, tests_common::*};

use super::{
    model::{Data, Node},
    store::DataConn,
};

// SUPERSET

#[tokio::test]
async fn test_dns_superset() {
    let mut con = setup_db_con().await;
    let name = "[dns-superset]domain.net";

    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "[dns-superset]domain.com", PLUGIN, "cname", name],
    )
    .await;

    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", "[dns-superset]domain.org", PLUGIN, "cname", name],
    )
    .await;

    call_fn(
        &mut con,
        "netdox_create_dns",
        &["1", name, PLUGIN, "a", "[dns-superset]192.168.0.1"],
    )
    .await;

    assert_eq!(
        HashSet::from([
            name.to_string(),
            "[dns-superset]domain.com".to_string(),
            "[dns-superset]domain.org".to_string(),
            "[dns-superset]192.168.0.1".to_string()
        ]),
        con.get_dns().await.unwrap().dns_superset(name).unwrap()
    )
}

// NODES

#[tokio::test]
async fn test_node_roundtrip() {
    let mut con = setup_db_con().await;
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

// PLUGIN DATA

#[tokio::test]
async fn test_plugin_data_str() {
    let mut con = setup_db_con().await;
    let qname = format!("[{DEFAULT_NETWORK}]dns-pdata-str.com");
    let pdata_id = "pdata_id";
    let pdata_title = "Title!";
    let str_type = StringType::Plain;
    let str_content = "some string content :O";

    call_fn(&mut con, "netdox_create_dns", &["1", &qname, PLUGIN]).await;

    call_fn(
        &mut con,
        "netdox_create_dns_plugin_data",
        &[
            "1",
            &qname,
            PLUGIN,
            "string",
            pdata_id,
            pdata_title,
            str_type.clone().into(),
            str_content,
        ],
    )
    .await;

    assert_eq!(
        con.get_dns_pdata(&qname).await.unwrap(),
        vec![Data::String {
            id: pdata_id.to_owned(),
            title: pdata_title.to_owned(),
            content_type: str_type,
            content: str_content.to_owned(),
            plugin: PLUGIN.to_owned()
        }]
    );
}

#[tokio::test]
async fn test_plugin_data_list() {
    let mut con = setup_db_con().await;
    let qname = format!("[{DEFAULT_NETWORK}]dns-pdata-list.com");
    let pdata_id = "pdata_id";
    let pdata_title = "Title!";

    call_fn(&mut con, "netdox_create_dns", &["1", &qname, PLUGIN]).await;

    call_fn(
        &mut con,
        "netdox_create_dns_plugin_data",
        &[
            "1",
            &qname,
            PLUGIN,
            "list",
            pdata_id,
            pdata_title,
            "name1",
            "title1",
            "value1",
            "name2",
            "title2",
            "value2",
        ],
    )
    .await;

    assert_eq!(
        con.get_dns_pdata(&qname).await.unwrap(),
        vec![Data::List {
            id: pdata_id.to_owned(),
            title: pdata_title.to_owned(),
            content: vec![
                (
                    "name1".to_string(),
                    "title1".to_string(),
                    "value1".to_string(),
                ),
                (
                    "name2".to_string(),
                    "title2".to_string(),
                    "value2".to_string(),
                )
            ],
            plugin: PLUGIN.to_owned()
        }]
    );
}
