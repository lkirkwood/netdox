use psml::{
    model::{PropertiesFragment, Property, PropertyValue},
    text::{CharacterStyle, Monospace, Para, ParaContent},
};

use super::{dns_name_document, processed_node_document};
use crate::{
    data::{model::Node, DataConn},
    remote::pageseeder::psml::links::LinkContent,
    tests_common::{PLUGIN, TEST_REDIS_URL_VAR},
};
use std::{collections::HashSet, env};

use quick_xml::se as xml_se;

async fn backend() -> Box<dyn DataConn> {
    Box::new(
        redis::Client::open(env::var(TEST_REDIS_URL_VAR).unwrap())
            .unwrap()
            .get_async_connection()
            .await
            .unwrap(),
    )
}

#[test]
fn test_pfrag_se() {
    assert_eq!(
        "<properties-fragment id=\"frag-id\">\
            <property name=\"name1\" title=\"First\" datatype=\"string\">\
                <value>value</value>\
            </property>\
            </properties-fragment>",
        xml_se::to_string_with_root(
            "properties-fragment",
            &PropertiesFragment::new("frag-id".to_string()).with_properties(vec![
                Property::with_value(
                    "name1".to_string(),
                    "First".to_string(),
                    PropertyValue::Value("value".to_string()),
                ),
            ]),
        )
        .unwrap()
    )
}

#[test]
fn test_para_se() {
    assert_eq!(
        "<para>some text<monospace>some monospace</monospace></para>",
        xml_se::to_string(&Para::new(vec![
            ParaContent::Text("some text".to_string()),
            ParaContent::Monospace(Monospace {
                content: vec![CharacterStyle::Text("some monospace".to_string())]
            })
        ]))
        .unwrap()
    )
}

#[tokio::test]
async fn test_pfrag_links() {
    assert_eq!(
        "<properties-fragment id=\"frag-id\">\
            <property name=\"name1\" title=\"First\" datatype=\"xref\">\
                <xref docid=\"_nd_dns_domain_com\" display=\"document\" frag=\"default\" reverselink=\"true\"/>\
            </property>\
        </properties-fragment>",
        xml_se::to_string_with_root(
            "properties-fragment",
            &PropertiesFragment::new("frag-id".to_string())
                .with_properties(vec![Property::with_value(
                    "name1".to_string(),
                    "First".to_string(),
                    PropertyValue::Value("(!(dns|!|domain.com)!)".to_string()),
                ),])
                .create_links(&mut backend().await)
                .await
                .unwrap(),
        )
        .unwrap()
    )
}

#[tokio::test]
async fn test_dns_doc() {
    dns_name_document(&mut backend().await, "[doc-network]domain.psml")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_node_doc() {
    processed_node_document(
        &mut backend().await,
        &Node {
            name: "Node Document".to_string(),
            alt_names: HashSet::from(["Also a Document".to_string()]),
            dns_names: HashSet::from(["[doc-network]node.psml".to_string()]),
            plugins: HashSet::from([PLUGIN.to_string()]),
            raw_ids: HashSet::from(["[doc-network]node.psml".to_string()]),
            link_id: "node-docid-part".to_string(),
        },
    )
    .await
    .unwrap();
}
