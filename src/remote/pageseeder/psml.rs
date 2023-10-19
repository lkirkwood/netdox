use std::ops::Index;

use pageseeder::psml::{
    model::{
        Document, Fragment, FragmentContent, Fragments, PropertiesFragment, Property,
        PropertyValue, Section, SectionContent, XRef,
    },
    text::{Heading, Para},
};
use regex::Regex;

use crate::{
    data::{
        model::{DNSRecord, Node, PluginData, StringType},
        Datastore,
    },
    error::{NetdoxError, NetdoxResult},
    redis_err,
    remote::pageseeder::remote::node_id_to_docid,
};

use super::remote::dns_qname_to_docid;

/// Generates a document representing the DNS name.
pub async fn dns_name_document(backend: &mut dyn Datastore, name: &str) -> NetdoxResult<Document> {
    use FragmentContent as FC;
    use Fragments as F;
    use PropertyValue as PV;

    let (network, raw_name) = match name.rsplit_once(']') {
        Some(tuple) => match tuple.0.strip_prefix('[') {
            Some(net) => (net, tuple.1),
            None => return redis_err!(format!("Failed to parse network from qname: {name}")),
        },
        None => return redis_err!(format!("Failed to parse network from qname: {name}")),
    };
    let dns = backend.get_dns().await?;

    let mut document = dns_template();

    // Title

    let title = document.get_mut_section("title").unwrap();
    title.add_fragment(F::Fragment(
        Fragment::new("title".to_string()).with_content(vec![FC::Heading(Heading {
            level: Some(1),
            content: vec![name.to_string()],
        })]),
    ));

    // Header

    let header = document.get_mut_section("header").unwrap();
    let node_xref = match &backend.get_dns_node_id(name).await? {
        Some(id) => PV::XRef(XRef::docid(node_id_to_docid(id))),
        None => PV::Value("—".to_string()),
    };

    header.add_fragment(F::Properties(
        PropertiesFragment::new("header".to_string()).with_properties(vec![
            Property::new(
                "name".to_string(),
                "DNS Name".to_string(),
                vec![PV::Value(raw_name.to_string())],
            ),
            Property::new(
                "network".to_string(),
                "Logical Network".to_string(),
                vec![PV::Value(network.to_string())],
            ),
            Property::new("node".to_string(), "Node".to_string(), vec![node_xref]),
        ]),
    ));

    // Metadata

    header.add_fragment(F::Properties(
        PropertiesFragment::new("meta".to_string()).with_properties(
            backend
                .get_dns_metadata(name)
                .await?
                .into_iter()
                .map(|(key, val)| Property::new(key.clone(), key.clone(), vec![val.into()]))
                .collect(),
        ),
    ));

    // Records

    let records = document.get_mut_section("records").unwrap();
    for record in dns.get_records(name) {
        records
            .content
            .push(SectionContent::PropertiesFragment(record.into()));
    }

    // Implied records

    let impl_records = PropertiesFragment::new("implied-records".to_string()).with_properties(
        dns.get_rev_ptrs(name)
            .into_iter()
            .map(|qn| {
                Property::new(
                    "implied-record".to_string(),
                    "Implied DNS Record".to_string(),
                    vec![PropertyValue::XRef(XRef::docid(dns_qname_to_docid(qn)))],
                )
            })
            .collect(),
    );

    document
        .get_mut_section("implied-records")
        .unwrap()
        .add_fragment(Fragments::Properties(impl_records));

    // Plugin data

    let pdata_section = document.get_mut_section("plugin-data").unwrap();
    for pdata in backend.get_dns_pdata(name).await? {
        pdata_section.add_fragment(pdata.into());
    }

    Ok(document)
}

impl From<&DNSRecord> for PropertiesFragment {
    fn from(value: &DNSRecord) -> Self {
        let id = format!("{}_{}_{}", value.plugin, value.rtype, value.value);
        PropertiesFragment::new(id).with_properties(vec![
            Property::new(
                "value".to_string(),
                "Record Value".to_string(),
                vec![PropertyValue::XRef(XRef::docid(dns_qname_to_docid(
                    &value.value,
                )))],
            ),
            Property::new(
                "rtype".to_string(),
                "Record Type".to_string(),
                vec![PropertyValue::Value(value.rtype.clone())],
            ),
            Property::new(
                "plugin".to_string(),
                "Source Plugin".to_string(),
                vec![PropertyValue::Value(value.plugin.clone())],
            ),
        ])
    }
}

// TODO implement links in pdata
impl From<PluginData> for Fragments {
    fn from(value: PluginData) -> Self {
        match value {
            PluginData::String {
                id,
                title,
                content_type,
                plugin,
                content,
            } => match content_type {
                StringType::Plain => Fragments::Fragment(Fragment::new(id).with_content(vec![
                    FragmentContent::Heading(Heading {
                        level: Some(2),
                        content: vec![title],
                    }),
                    FragmentContent::Heading(Heading {
                        level: Some(3),
                        content: vec![format!("Source Plugin: {plugin}")],
                    }),
                    FragmentContent::Para(Para {
                        content: vec![content],
                        indent: None,
                        numbered: None,
                        prefix: None,
                    }),
                ])),
                StringType::Markdown => todo!("Convert markdown text to psml"),
                StringType::HtmlMarkup => todo!("Convert HtmlMarkup text to psml"),
            },
            PluginData::Hash {
                id,
                title,
                plugin,
                content,
            } => Fragments::Properties(
                PropertiesFragment::new(id)
                    .with_properties(vec![
                        Property::new(
                            "pdata-title".to_string(),
                            "Plugin Data Title".to_string(),
                            vec![title.into()],
                        ),
                        Property::new(
                            "plugin".to_string(),
                            "Source Plugin".to_string(),
                            vec![plugin.into()],
                        ),
                    ])
                    .with_properties(
                        content
                            .into_iter()
                            .map(|(key, val)| Property::new(key.clone(), key, vec![val.into()]))
                            .collect(),
                    ),
            ),
            PluginData::List {
                id,
                list_title,
                item_title,
                plugin,
                content,
            } => Fragments::Properties(
                PropertiesFragment::new(id)
                    .with_properties(vec![
                        Property::new(
                            "pdata-title".to_string(),
                            "Plugin Data Title".to_string(),
                            vec![list_title.into()],
                        ),
                        Property::new(
                            "plugin".to_string(),
                            "Source Plugin".to_string(),
                            vec![plugin.into()],
                        ),
                    ])
                    .with_properties(
                        content
                            .into_iter()
                            .map(|item| {
                                Property::new(
                                    item_title.clone(),
                                    item_title.clone(),
                                    vec![item.into()],
                                )
                            })
                            .collect(),
                    ),
            ),
        }
    }
}

/// Returns an empty document for a DNS name with all sections included.
fn dns_template() -> Document {
    Document {
        sections: vec![
            Section {
                id: "title".to_string(),
                content: vec![],
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                title: None,
                overwrite: None,
            },
            Section {
                id: "header".to_string(),
                content: vec![],
                title: Some("Header".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "records".to_string(),
                content: vec![],
                title: Some("DNS Records".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "implied-records".to_string(),
                content: vec![],
                title: Some("Implied DNS Records".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "plugin-data".to_string(),
                content: vec![],
                title: Some("Plugin Data".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
        ],
        lockstructure: Some(true),
        ..Default::default()
    }
}

pub async fn processed_node_document(
    _backend: &mut dyn Datastore,
    node: &Node,
) -> NetdoxResult<Document> {
    use Fragment as FR;
    use FragmentContent as FC;
    use Fragments as F;

    let mut document = node_template();

    document
        .get_mut_section("title")
        .unwrap()
        .add_fragment(F::Fragment(FR::new("title".to_string()).with_content(
            vec![FC::Heading(Heading {
                level: Some(1),
                content: vec![node.name.to_owned()],
            })],
        )));

    Ok(document)
}

/// Returns an empty document for a node with all sections included.
fn node_template() -> Document {
    Document {
        sections: vec![
            Section {
                id: "title".to_string(),
                content: vec![],
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                title: None,
                overwrite: None,
            },
            Section {
                id: "header".to_string(),
                content: vec![],
                title: Some("Header".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "dns-names".to_string(),
                content: vec![],
                title: Some("DNS Names".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "plugin-data".to_string(),
                content: vec![],
                title: Some("Plugin Data".to_string()),
                edit: Some(false),
                lock: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
        ],
        lockstructure: Some(true),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{dns_name_document, processed_node_document};
    use crate::{
        data::{model::Node, Datastore},
        tests_common::{PLUGIN, TEST_REDIS_URL_VAR},
    };
    use std::{collections::HashSet, env};

    async fn backend() -> Box<dyn Datastore> {
        Box::new(
            redis::Client::open(env::var(TEST_REDIS_URL_VAR).unwrap())
                .unwrap()
                .get_async_connection()
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn test_dns_doc() {
        dns_name_document(&mut *backend().await, "[doc-network]domain.psml")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_node_doc() {
        processed_node_document(
            &mut *backend().await,
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
}

/// Returns a property value that contains string with any encoded links expanded.
/// Only fails if there is an invalid link.
/// If there is no link simply returns a string value.
fn property_val_with_links(value: String) -> NetdoxResult<PropertyValue> {
    let pattern = Regex::new(r"^\(!\((\w+)\|!\|([\w0-9\[\]_.-]+)\)!\)$").unwrap();

    match pattern.captures(&value) {
        Some(captures) => match captures.index(0) {
            "dns" => Ok(PropertyValue::XRef(XRef::docid(dns_qname_to_docid(
                captures.index(1),
            )))),
            "node" => Ok(PropertyValue::XRef(XRef::docid(node_id_to_docid(
                captures.index(1),
            )))),
            "report" => {
                todo!("Linking to reports")
            }
            other => {
                return redis_err!(format!("Invalid link type {other} in plugin data: {value}"))
            }
        },
        None => Ok(value.into()),
    }
}
