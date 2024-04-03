mod changelog;
mod config;
pub mod links;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use psml::{
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
pub use config::remote_config_document;
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

    // Details

    let details = document.get_mut_section("details").unwrap();

    details.add_fragment(F::Properties(
        PropertiesFragment::new("details".to_string()).with_properties(vec![
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
        ]),
    ));

    // Metadata

    details.add_fragment(F::Properties(
        metadata_fragment(backend.get_dns_metadata(name).await?)
            .create_links(backend)
            .await?,
    ));

    // Records

    let records = dns.get_records(name);
    let record_sec = document.get_mut_section("dns-records").unwrap();
    for record in &records {
        record_sec.content.push(SectionContent::PropertiesFragment(
            (*record).to_owned().into(),
        ));
    }

    // Implied records

    let implied_records = document.get_mut_section("implied-records").unwrap();
    for record in dns.get_implied_records(name) {
        if !records.contains(&DNSRecord::from(record.clone())) {
            implied_records
                .content
                .push(SectionContent::PropertiesFragment(record.to_owned().into()))
        }
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

    // Details

    let details = document.get_mut_section("details").unwrap();
    details.add_fragment(F::Properties(
        PropertiesFragment::new("details".to_owned())
            .with_properties(vec![Property::with_value(
                "name".to_owned(),
                "Name".to_owned(),
                node.name.to_owned().into(),
            )])
            .with_properties(
                node.alt_names
                    .iter()
                    .map(|n| {
                        Property::with_value(
                            "alt_name".to_owned(),
                            "Alt Name".to_owned(),
                            n.to_owned().into(),
                        )
                    })
                    .collect(),
            )
            .with_properties(
                node.plugins
                    .iter()
                    .map(|p| {
                        Property::with_value(
                            "plugin".to_owned(),
                            "Plugin".to_owned(),
                            p.to_owned().into(),
                        )
                    })
                    .collect(),
            ),
    ));

    // Metadata

    details.add_fragment(F::Properties(
        metadata_fragment(backend.get_node_metadata(node).await?)
            .create_links(backend)
            .await?,
    ));

    // DNS Names

    let dns_section = document.get_mut_section("dns-names").unwrap();
    dns_section.add_fragment(F::Properties(
        PropertiesFragment::new("dns-names".to_owned()).with_properties(
            node.dns_names
                .iter()
                .map(|qname| {
                    Property::with_value(
                        "dns-name".to_owned(),
                        "DNS Name".to_owned(),
                        PropertyValue::XRef(Box::new(XRef::docid(dns_qname_to_docid(qname)))),
                    )
                })
                .collect(),
        ),
    ));

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

pub const DNS_RECORD_SECTION: &str = "dns-records";
pub const IMPLIED_RECORD_SECTION: &str = "implied-records";
pub const PDATA_SECTION: &str = "plugin-data";
pub const RDATA_SECTION: &str = "content";

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
                id: "details".to_string(),
                content: vec![],
                title: Some("Details".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: DNS_RECORD_SECTION.to_string(),
                content: vec![],
                title: Some("DNS Records".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: IMPLIED_RECORD_SECTION.to_string(),
                content: vec![],
                title: Some("Implied DNS Records".to_string()),
                edit: Some(false),
                lockstructure: Some(true),
                content_title: None,
                fragment_types: None,
                overwrite: None,
            },
            Section {
                id: PDATA_SECTION.to_string(),
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
                id: "details".to_string(),
                content: vec![],
                title: Some("Details".to_string()),
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
                id: PDATA_SECTION.to_string(),
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
                id: RDATA_SECTION.to_string(),
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
                Property::with_value(
                    Property::sanitize_name(&key, "-").to_string(),
                    key.to_string(),
                    PropertyValue::Value(val),
                )
            })
            .collect(),
    )
}

// From impls

impl From<DNSRecord> for PropertiesFragment {
    fn from(value: DNSRecord) -> Self {
        let pattern = Regex::new("[^a-zA-Z0-9_=,&.-]").unwrap();
        let id = pattern
            .replace_all(
                &format!("{}_{}_{}", value.plugin, value.rtype, value.value),
                "_",
            )
            .to_string();

        let pval = match value.rtype.as_ref() {
            "CNAME" | "A" | "PTR" => {
                PropertyValue::XRef(Box::new(XRef::docid(dns_qname_to_docid(&value.value))))
            }
            _ => PropertyValue::Value(value.value),
        };

        PropertiesFragment::new(id).with_properties(vec![
            Property::with_value("value".to_string(), "Record Value".to_string(), pval),
            Property::with_value(
                "rtype".to_string(),
                "Record Type".to_string(),
                PropertyValue::Value(value.rtype),
            ),
            Property::with_value(
                "plugin".to_string(),
                "Source Plugin".to_string(),
                PropertyValue::Value(value.plugin),
            ),
        ])
    }
}

impl From<ImpliedDNSRecord> for PropertiesFragment {
    fn from(value: ImpliedDNSRecord) -> Self {
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
                PropertyValue::XRef(Box::new(XRef::docid(dns_qname_to_docid(&value.value)))),
            ),
            Property::with_value(
                "rtype".to_string(),
                "Implied Record Type".to_string(),
                PropertyValue::Value(value.rtype),
            ),
            Property::with_value(
                "plugin".to_string(),
                "Source Plugin".to_string(),
                PropertyValue::Value(value.plugin),
            ),
        ])
    }
}

impl From<DNSRecords> for PropertiesFragment {
    fn from(value: DNSRecords) -> Self {
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
                                Property::with_value(
                                    Property::sanitize_name(&key, "-").to_string(),
                                    key,
                                    PropertyValue::Value(val),
                                )
                            })
                            .collect(),
                    ),
            ),
            D::List {
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
                            .map(|(name, title, value)| {
                                Property::with_value(
                                    Property::sanitize_name(&name, "-").to_string(),
                                    title,
                                    PropertyValue::Value(value),
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
