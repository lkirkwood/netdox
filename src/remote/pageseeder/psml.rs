use std::{collections::HashMap, ops::Index};

use pageseeder::psml::{
    model::{
        BlockXRef, Document, DocumentInfo, Fragment, FragmentContent, Fragments,
        PropertiesFragment, Property, PropertyValue, Section, SectionContent, URIDescriptor, XRef,
    },
    text::{Heading, Para},
};
use regex::Regex;

use crate::{
    data::{
        model::{DNSRecord, Node, PluginData, StringType},
        DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    redis_err,
    remote::pageseeder::remote::node_id_to_docid,
};

use super::remote::dns_qname_to_docid;

pub const METADATA_FRAGMENT: &str = "meta";

/// Generates a document representing the DNS name.
pub async fn dns_name_document(
    backend: &mut Box<dyn DataConn>,
    name: &str,
) -> NetdoxResult<Document> {
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
    document.doc_info = Some(DocumentInfo {
        uri: Some(URIDescriptor {
            docid: Some(dns_qname_to_docid(name)),
            ..Default::default()
        }),
        ..Default::default()
    });

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
            Property::with_value(
                "name".to_string(),
                "DNS Name".to_string(),
                PV::Value(raw_name.to_string()),
            ),
            Property::with_value(
                "network".to_string(),
                "Logical Network".to_string(),
                PV::Value(network.to_string()),
            ),
            Property::with_value("node".to_string(), "Node".to_string(), node_xref),
        ]),
    ));

    // Metadata

    header.add_fragment(F::Properties(metadata_fragment(
        backend.get_dns_metadata(name).await?,
    )));

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
                Property::with_value(
                    "implied-record".to_string(),
                    "Implied DNS Record".to_string(),
                    PropertyValue::XRef(XRef::docid(dns_qname_to_docid(qn))),
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

pub async fn processed_node_document(
    _backend: &mut Box<dyn DataConn>,
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

// Template documents

/// Returns an empty document for a DNS name with all sections included.
fn dns_template() -> Document {
    Document {
        sections: vec![
            Section {
                id: "title".to_string(),
                content: vec![],
                edit: Some(false),
                lockstructure: Some(true),
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
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "records".to_string(),
                content: vec![],
                title: Some("DNS Records".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "implied-records".to_string(),
                content: vec![],
                title: Some("Implied DNS Records".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "plugin-data".to_string(),
                content: vec![],
                title: Some("Plugin Data".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
        ],
        lockstructure: Some(true),
        ..Default::default()
    }
}

/// Returns an empty document for a node with all sections included.
fn node_template() -> Document {
    Document {
        sections: vec![
            Section {
                id: "title".to_string(),
                content: vec![],
                edit: Some(false),
                lockstructure: Some(true),
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
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "dns-names".to_string(),
                content: vec![],
                title: Some("DNS Names".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: "plugin-data".to_string(),
                content: vec![],
                title: Some("Plugin Data".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
        ],
        lockstructure: Some(true),
        ..Default::default()
    }
}

// Text with links

/// Returns a property value that contains string with any encoded links expanded.
/// If there is an invalid link it is treated as no link at all.
fn property_val_with_links(value: String) -> PropertyValue {
    let pattern = Regex::new(r"^\(!\((dns|node|report)\|!\|([\w0-9\[\]_.-]+)\)!\)$").unwrap();

    match pattern.captures(&value) {
        Some(captures) => match captures.index(0) {
            "dns" => PropertyValue::XRef(XRef::docid(dns_qname_to_docid(captures.index(1)))),
            "node" => PropertyValue::XRef(XRef::docid(node_id_to_docid(captures.index(1)))),
            "report" => {
                todo!("Link to reports from property")
            }
            _ => unreachable!(),
        },
        None => value.into(),
    }
}

fn para_with_links(content: String) -> Vec<FragmentContent> {
    use FragmentContent as FC;

    let pattern = Regex::new(r"\(!\((dns|node|report)\|!\|([\w0-9\[\]_.-]+)\)!\)").unwrap();
    let mut last_index = 0;
    let mut frag_content = vec![];
    for cap in pattern.captures_iter(&content) {
        let fullmatch = cap.get(0).unwrap();
        frag_content.push(FC::Para(Para::new(vec![content
            [last_index..fullmatch.start()]
            .to_string()])));
        last_index = fullmatch.end();

        let cap_groups: [&str; 2] = cap.extract().1;
        frag_content.push(match cap_groups[0] {
            "dns" => FC::BlockXRef(BlockXRef::docid(dns_qname_to_docid(cap_groups[1]))),
            "node" => FC::BlockXRef(BlockXRef::docid(node_id_to_docid(cap_groups[1]))),
            "report" => todo!("Link to report from para"),
            _ => unreachable!(),
        })
    }

    frag_content.push(FC::Para(Para::new(vec![content[last_index..].to_string()])));
    frag_content
}

// Fragment generators

pub fn metadata_fragment(metadata: HashMap<String, String>) -> PropertiesFragment {
    PropertiesFragment::new(METADATA_FRAGMENT.to_string()).with_properties(
        metadata
            .into_iter()
            .map(|(key, val)| {
                Property::with_value(key.clone(), key.clone(), property_val_with_links(val))
            })
            .collect(),
    )
}

// From impls

impl From<&DNSRecord> for PropertiesFragment {
    fn from(value: &DNSRecord) -> Self {
        let pattern = Regex::new("[^a-zA-Z0-9_=,&.-]").unwrap();
        let id = pattern
            .replace_all(
                &format!("{}_{}_{}", value.plugin, value.rtype, value.value),
                "_",
            )
            .to_string();

        PropertiesFragment::new(id).with_properties(vec![
            Property::with_value(
                "value".to_string(),
                "Record Value".to_string(),
                PropertyValue::XRef(XRef::docid(dns_qname_to_docid(&value.value))),
            ),
            Property::with_value(
                "rtype".to_string(),
                "Record Type".to_string(),
                PropertyValue::Value(value.rtype.clone()),
            ),
            Property::with_value(
                "plugin".to_string(),
                "Source Plugin".to_string(),
                PropertyValue::Value(value.plugin.clone()),
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
                StringType::Plain => Fragments::Fragment(
                    Fragment::new(id)
                        .with_content(vec![
                            FragmentContent::Heading(Heading {
                                level: Some(2),
                                content: vec![title],
                            }),
                            FragmentContent::Heading(Heading {
                                level: Some(3),
                                content: vec![format!("Source Plugin: {plugin}")],
                            }),
                        ])
                        .with_content(para_with_links(content)),
                ),
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
                        Property::with_value(
                            "pdata-title".to_string(),
                            "Plugin Data Title".to_string(),
                            title.into(),
                        ),
                        Property::with_value(
                            "plugin".to_string(),
                            "Source Plugin".to_string(),
                            plugin.into(),
                        ),
                    ])
                    .with_properties(
                        content
                            .into_iter()
                            .map(|(key, val)| {
                                Property::with_value(key.clone(), key, property_val_with_links(val))
                            })
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
                        Property::with_value(
                            "pdata-title".to_string(),
                            "Plugin Data Title".to_string(),
                            list_title.into(),
                        ),
                        Property::with_value(
                            "plugin".to_string(),
                            "Source Plugin".to_string(),
                            plugin.into(),
                        ),
                    ])
                    .with_properties(
                        content
                            .into_iter()
                            .map(|item| {
                                Property::with_value(
                                    item_title.clone(),
                                    item_title.clone(),
                                    item.into(),
                                )
                            })
                            .collect(),
                    ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{dns_name_document, processed_node_document};
    use crate::{
        data::{model::Node, DataConn},
        tests_common::{PLUGIN, TEST_REDIS_URL_VAR},
    };
    use std::{collections::HashSet, env};

    async fn backend() -> Box<dyn DataConn> {
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
}
