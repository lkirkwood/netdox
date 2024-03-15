use psml::{
    model::{Document, DocumentInfo, Fragment, FragmentContent, Fragments, Section, URIDescriptor},
    text::{CharacterStyle, Heading},
};

use crate::remote::pageseeder::config::{
    EXCLUDE_DNS_SECTION_ID, LOCATIONS_SECTION_ID, REMOTE_CONFIG_DOCID,
};

const MAIN_HEADING: &str = "Netdox Config";
const DOCUMENT_TYPE: &str = "netdox-config";

const LOCATIONS_HEADING: &str = "Locations";
const LOCATIONS_DESC: &str =
    "Define associations between IPv4 subnets and a location identifier here.
Objects connected to addresses in the subnets will be assigned the given location.";

const EXCLUDE_DNS_HEADING: &str = "Excluded DNS Names";
const EXCLUDE_DNS_DESC: &str =
    "List DNS names here that you wish to exclude from the dataset - one per line.
No documents or links will be created for these names.";

pub fn remote_config_document() -> Document {
    Document {
        doc_info: Some(DocumentInfo {
            uri: Some(URIDescriptor {
                docid: Some(REMOTE_CONFIG_DOCID.to_string()),
                title: Some(MAIN_HEADING.to_string()),
                doc_type: Some(DOCUMENT_TYPE.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        sections: vec![
            // Title
            Section::new("main-heading".to_string()).with_fragments(vec![Fragments::Fragment(
                Fragment::new("main-heading".to_string()).with_content(vec![
                    FragmentContent::Heading(Heading {
                        level: Some(1),
                        content: vec![CharacterStyle::Text(MAIN_HEADING.to_string())],
                    }),
                ]),
            )]),
            // Locations
            Section::new(LOCATIONS_SECTION_ID.to_string()).with_fragments(vec![
                Fragments::Fragment(Fragment::new("locations-heading".to_string()).with_content(
                    vec![
                        FragmentContent::Heading(Heading {
                            level: Some(1),
                            content: vec![CharacterStyle::Text(LOCATIONS_HEADING.to_string())],
                        }),
                        FragmentContent::Preformat {
                            child: vec![FragmentContent::Text(LOCATIONS_DESC.to_string())],
                        },
                    ],
                )),
            ]),
            // Exclusions
            Section::new(EXCLUDE_DNS_SECTION_ID.to_string()).with_fragments(vec![
                Fragments::Fragment(
                    Fragment::new("exclusions-heading".to_string()).with_content(vec![
                        FragmentContent::Heading(Heading {
                            level: Some(1),
                            content: vec![CharacterStyle::Text(EXCLUDE_DNS_HEADING.to_string())],
                        }),
                        FragmentContent::Preformat {
                            child: vec![FragmentContent::Text(EXCLUDE_DNS_DESC.to_string())],
                        },
                    ]),
                ),
            ]),
        ],
        ..Default::default()
    }
}

// TODO add plugin config
