mod changelog;
mod links;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use pageseeder::psml::{
    model::{
        Document, DocumentInfo, Fragment, FragmentContent, Fragments, PropertiesFragment, Property,
        PropertyValue, Section, SectionContent, Table, URIDescriptor, XRef,
    },
    text::{CharacterStyle, Heading},
};
use regex::Regex;

use crate::{
    data::{
        model::{DNSRecord, DNSRecords, Data, ImpliedDNSRecord, Node, StringType},
        DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    redis_err,
    remote::pageseeder::remote::{node_id_to_docid, report_id_to_docid},
};
pub use changelog::changelog_document;
use links::LinkContent;

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
            title: Some(raw_name.to_owned()),
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
            content: vec![CharacterStyle::Text(raw_name.to_string())],
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

    let records = document.get_mut_section("dns-records").unwrap();
    for record in dns.get_records(name) {
        records
            .content
            .push(SectionContent::PropertiesFragment(record.into()));
    }

    // Implied records

    let implied_records = document.get_mut_section("implied-records").unwrap();
    for record in dns.get_implied_records(name) {
        implied_records
            .content
            .push(SectionContent::PropertiesFragment(record.into()))
    }

    // Plugin data

    let pdata_section = document.get_mut_section("plugin-data").unwrap();
    for pdata in backend.get_dns_pdata(name).await? {
        pdata_section.add_fragment(pdata.into());
    }

    document.create_links(backend).await
}

pub async fn processed_node_document(
    backend: &mut Box<dyn DataConn>,
    node: &Node,
) -> NetdoxResult<Document> {
    use CharacterStyle as CS;
    use Fragment as FR;
    use FragmentContent as FC;
    use Fragments as F;

    let mut document = node_template();
    document.doc_info = Some(DocumentInfo {
        uri: Some(URIDescriptor {
            title: Some(node.name.to_owned()),
            docid: Some(node_id_to_docid(&node.link_id)),
            ..Default::default()
        }),
        ..Default::default()
    });

    // Title

    document
        .get_mut_section("title")
        .unwrap()
        .add_fragment(F::Fragment(FR::new("title".to_string()).with_content(
            vec![FC::Heading(Heading {
                level: Some(1),
                content: vec![CS::Text(node.name.to_owned())],
            })],
        )));

    // Metadata

    let header = document.get_mut_section("header").unwrap();
    header.add_fragment(F::Properties(metadata_fragment(
        backend.get_node_metadata(node).await?,
    )));

    // Plugin data

    let pdata_section = document.get_mut_section("plugin-data").unwrap();
    for pdata in backend.get_node_pdata(node).await? {
        pdata_section.add_fragment(pdata.into());
    }

    document.create_links(backend).await
}

pub async fn report_document(backend: &mut Box<dyn DataConn>, id: &str) -> NetdoxResult<Document> {
    use CharacterStyle as CS;
    use FragmentContent as FC;

    let mut document = report_template();
    let report = backend.get_report(id).await?;

    document.doc_info = Some(DocumentInfo {
        uri: Some(URIDescriptor {
            title: Some(report.title.clone()),
            docid: Some(report_id_to_docid(&report.id)),
            ..Default::default()
        }),
        ..Default::default()
    });

    document
        .get_mut_section("title")
        .unwrap()
        .add_fragment(Fragments::Fragment(
            Fragment::new("title".to_string()).with_content(vec![FC::Heading(Heading {
                level: Some(1),
                content: vec![CS::Text(report.title)],
            })]),
        ));

    let content = document.get_mut_section("content").unwrap();
    for part in report.content {
        content.add_fragment(Fragments::from(part));
    }

    document.create_links(backend).await
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
                id: "dns-records".to_string(),
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

/// Returns an empty document for a report with all sections included.
fn report_template() -> Document {
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
                id: "content".to_string(),
                content: vec![],
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                title: Some("Content".to_string()),
                overwrite: None,
            },
        ],
        lockstructure: Some(true),
        ..Default::default()
    }
}

// Text with links

// Fragment generators

pub fn metadata_fragment(metadata: HashMap<String, String>) -> PropertiesFragment {
    PropertiesFragment::new(METADATA_FRAGMENT.to_string()).with_properties(
        metadata
            .into_iter()
            .map(|(key, val)| {
                Property::with_value(key.clone(), key.clone(), PropertyValue::Value(val))
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

        let pval = match value.rtype.as_ref() {
            "CNAME" | "A" | "PTR" => {
                PropertyValue::XRef(XRef::docid(dns_qname_to_docid(&value.value)))
            }
            _ => PropertyValue::Value(value.value.to_owned()),
        };

        PropertiesFragment::new(id).with_properties(vec![
            Property::with_value("value".to_string(), "Record Value".to_string(), pval),
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

impl From<&ImpliedDNSRecord> for PropertiesFragment {
    fn from(value: &ImpliedDNSRecord) -> Self {
        let pattern = Regex::new("[^a-zA-Z0-9_=,&.-]").unwrap();
        let id = pattern
            .replace_all(
                &format!("implied_{}_{}_{}", value.plugin, value.rtype, value.value),
                "_",
            )
            .to_string();

        PropertiesFragment::new(id).with_properties(vec![
            Property::with_value(
                "value".to_string(),
                "Implied Record Value".to_string(),
                PropertyValue::XRef(XRef::docid(dns_qname_to_docid(&value.value))),
            ),
            Property::with_value(
                "rtype".to_string(),
                "Implied Record Type".to_string(),
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

impl From<&DNSRecords> for PropertiesFragment {
    fn from(value: &DNSRecords) -> Self {
        match value {
            DNSRecords::Actual(record) => PropertiesFragment::from(record),
            DNSRecords::Implied(record) => PropertiesFragment::from(record),
        }
    }
}

impl From<Data> for Fragments {
    fn from(value: Data) -> Self {
        use CharacterStyle as CS;
        use Data as D;
        use FragmentContent as FC;
        use Fragments as F;
        use StringType as ST;

        match value {
            D::String {
                id,
                title,
                content_type,
                plugin,
                content,
            } => match content_type {
                ST::Plain => F::Fragment(
                    Fragment::new(id)
                        .with_content(vec![
                            FC::Heading(Heading {
                                level: Some(2),
                                content: vec![CS::Text(title)],
                            }),
                            FC::Heading(Heading {
                                level: Some(3),
                                content: vec![CS::Text(format!("Source Plugin: {plugin}"))],
                            }),
                        ])
                        .with_content(vec![FC::Text(content)]),
                ),
                ST::Markdown => todo!("Convert markdown text to psml"),
                ST::HtmlMarkup => todo!("Convert HtmlMarkup text to psml"),
            },
            D::Hash {
                id,
                title,
                plugin,
                content,
            } => F::Properties(
                PropertiesFragment::new(id)
                    .with_properties(vec![
                        Property::with_value(
                            "data-title".to_string(),
                            "Data Title".to_string(),
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
                                Property::with_value(key.clone(), key, PropertyValue::Value(val))
                            })
                            .collect(),
                    ),
            ),
            D::List {
                id,
                list_title,
                item_title,
                plugin,
                content,
            } => F::Properties(
                PropertiesFragment::new(id)
                    .with_properties(vec![
                        Property::with_value(
                            "data-title".to_string(),
                            "Data Title".to_string(),
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
                                    item_title.to_lowercase(),
                                    item_title.clone(),
                                    item.into(),
                                )
                            })
                            .collect(),
                    ),
            ),
            D::Table {
                id,
                title,
                columns,
                plugin,
                content,
            } => {
                let mut cells = vec![];
                let mut row = vec![];
                for (num, cell) in content.iter().enumerate() {
                    if num % columns == 0 {
                        cells.push(row);
                        row = vec![];
                    }
                    row.push(cell.to_owned());
                }
                let mut table = Table::basic(columns, cells, title);
                table.summary = Some(format!("Source: {plugin}"));

                F::Fragment(Fragment::new(id).with_content(vec![FC::Table(table)]))
            }
        }
    }
}
