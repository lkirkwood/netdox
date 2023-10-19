use pageseeder::psml::{
    model::{
        Document, Fragment, FragmentContent, Fragments, PropertiesFragment, Property,
        PropertyValue, Section, SectionContent, XRef,
    },
    text::Heading,
};

use crate::{
    data::{
        model::{DNSRecord, Node, DNS_NODE_KEY},
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

    let (network, raw_name) = match name.rsplit_once("]") {
        Some(tuple) => match tuple.0.strip_prefix("[") {
            Some(net) => (net, tuple.1),
            None => return redis_err!(format!("Failed to parse network from qname: {name}")),
        },
        None => return redis_err!(format!("Failed to parse network from qname: {name}")),
    };
    let dns = backend.get_dns().await?;

    let mut document = dns_template();

    let title = document.get_mut_section("title").unwrap();
    title.add_fragment(F::Fragment(
        Fragment::new("title".to_string()).with_content(vec![FC::Heading(Heading {
            level: Some(1),
            content: vec![name.to_string()],
        })]),
    ));

    let header = document.get_mut_section("header").unwrap();

    let node_docid = node_id_to_docid(&backend.get_dns_node_id(name).await?);

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
            Property::new(
                "node".to_string(),
                "Node".to_string(),
                vec![PV::XRef(XRef::docid(node_docid))],
            ),
        ]),
    ));

    header.add_fragment(F::Properties(
        PropertiesFragment::new("meta".to_string()).with_properties(
            backend
                .get_dns_metadata(name)
                .await?
                .into_iter()
                .map(|(key, val)| {
                    Property::new(key.clone(), key.clone(), vec![PropertyValue::Value(val)])
                })
                .collect(),
        ),
    ));

    let records = document.get_mut_section("records").unwrap();
    for record in dns.get_records(name) {
        records
            .content
            .push(SectionContent::PropertiesFragment(record.into()));
    }
    // TODO implement implied records

    Ok(document)
}

impl Into<PropertiesFragment> for &DNSRecord {
    fn into(self) -> PropertiesFragment {
        let id = format!("{}_{}_{}", self.plugin, self.rtype, self.value);
        PropertiesFragment::new(id).with_properties(vec![
            Property::new(
                "value".to_string(),
                "Record Value".to_string(),
                vec![PropertyValue::XRef(XRef::docid(dns_qname_to_docid(
                    &self.value,
                )))],
            ),
            Property::new(
                "rtype".to_string(),
                "Record Type".to_string(),
                vec![PropertyValue::Value(self.rtype.clone())],
            ),
            Property::new(
                "plugin".to_string(),
                "Source Plugin".to_string(),
                vec![PropertyValue::Value(self.plugin.clone())],
            ),
        ])
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
        ..Default::default()
    }
}

fn processed_node_document(backend: &mut dyn Datastore, node: &Node) -> NetdoxResult<Document> {
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
        ..Default::default()
    }
}
