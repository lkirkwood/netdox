use psml::{
    model::{
        Document, DocumentInfo, Fragment, FragmentContent, Fragments, Section, SectionContent,
        URIDescriptor,
    },
    text::{CharacterStyle, Heading},
};

use crate::remote::pageseeder::config::{
    EXCLUSIONS_SECTION_ID, LOCATIONS_SECTION_ID, METADATA_SECTION_ID, REMOTE_CONFIG_DOCID,
};

const MAIN_HEADING: &str = "Netdox Config";
const DOCUMENT_TYPE: &str = "netdox_config";

const LOCATIONS_HEADING: &str = "Locations";
const LOCATIONS_DESC: &str =
    "Define associations between IPv4 subnets and a location identifier here.
Objects connected to addresses in the subnets will be assigned the given location.";

const EXCLUSIONS_HEADING: &str = "Excluded DNS Names";
const EXCLUSIONS_DESC: &str =
    "List DNS names here that you wish to exclude from the dataset - one per line.
No documents or links will be created for these names.";

const METADATA_HEADING: &str = "Label/Metadata Associations";
const METADATA_DESC: &str =
    "Define associations between a document label and a key/value pair here.
Documents with the given labels will have the relevant metadata key overriden with the provided value.";

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
            Section {
                id: LOCATIONS_SECTION_ID.to_string(),
                edit: Some(true),
                lockstructure: Some(false),
                overwrite: None,
                content_title: None,
                title: None,
                fragment_types: Some("subnet-location".to_string()),
                content: vec![SectionContent::Fragment(
                    Fragment::new("locations-heading".to_string()).with_content(vec![
                        FragmentContent::Heading(Heading {
                            level: Some(2),
                            content: vec![CharacterStyle::Text(LOCATIONS_HEADING.to_string())],
                        }),
                        FragmentContent::Preformat {
                            child: vec![FragmentContent::Text(LOCATIONS_DESC.to_string())],
                        },
                    ]),
                )],
            },
            // Exclusions
            Section {
                id: EXCLUSIONS_SECTION_ID.to_string(),
                edit: Some(true),
                lockstructure: Some(true),
                overwrite: None,
                content_title: None,
                fragment_types: None,
                title: None,
                content: vec![
                    SectionContent::Fragment(
                        Fragment::new("exclusions-heading".to_string()).with_content(vec![
                            FragmentContent::Heading(Heading {
                                level: Some(2),
                                content: vec![CharacterStyle::Text(EXCLUSIONS_HEADING.to_string())],
                            }),
                            FragmentContent::Preformat {
                                child: vec![FragmentContent::Text(EXCLUSIONS_DESC.to_string())],
                            },
                        ]),
                    ),
                    SectionContent::Fragment(Fragment::new("exclusions".to_string())),
                ],
            },
            Section {
                id: METADATA_SECTION_ID.to_string(),
                lockstructure: Some(false),
                edit: Some(true),
                overwrite: None,
                content_title: None,
                title: None,
                fragment_types: Some("label-metadata".to_string()),
                content: vec![SectionContent::Fragment(
                    Fragment::new("metadata-heading".to_string()).with_content(vec![
                        FragmentContent::Heading(Heading {
                            level: Some(2),
                            content: vec![CharacterStyle::Text(METADATA_HEADING.to_string())],
                        }),
                        FragmentContent::Preformat {
                            child: vec![FragmentContent::Text(METADATA_DESC.to_string())],
                        },
                    ]),
                )],
            },
        ],
        ..Default::default()
    }
}
