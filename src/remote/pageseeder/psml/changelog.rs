use pageseeder::psml::{
    model::{Document, DocumentInfo, Fragment, FragmentContent, Fragments, Section, URIDescriptor},
    text::{CharacterStyle, Heading, Para, ParaContent},
};

use crate::remote::pageseeder::remote::CHANGELOG_DOCID;

const TITLE_SECTION_ID: &str = "title";
const TITLE_FRAGMENT_ID: &str = "title";
const MAIN_HEADING: &str = "Netdox Changelog";
const SUB_HEADING: &str = "DO NOT EDIT THIS FILE";
const WARNING: &str = "This document should be modified by netdox ONLY. \
    Modifying this file will likely lead to the loss of data.";

const CONTENT_SECTION_ID: &str = "content";
const CONTENT_FRAGMENT_ID: &str = "last-change";

pub fn changelog_document() -> Document {
    use CharacterStyle as CS;
    use FragmentContent as FC;
    use ParaContent as PC;

    Document {
        lockstructure: Some(true),
        edit: Some(false),
        doc_info: Some(DocumentInfo {
            uri: Some(URIDescriptor {
                docid: Some(CHANGELOG_DOCID.to_string()),
                title: Some(MAIN_HEADING.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        sections: vec![
            Section::new(TITLE_SECTION_ID.to_string()).with_fragments(vec![Fragments::Fragment(
                Fragment::new(TITLE_FRAGMENT_ID.to_string()).with_content(vec![
                    FC::Heading(Heading {
                        level: Some(1),
                        content: vec![CS::Text(MAIN_HEADING.to_string())],
                    }),
                    FC::Heading(Heading {
                        level: Some(2),
                        content: vec![CS::Text(SUB_HEADING.to_string())],
                    }),
                    FC::Para(Para {
                        content: vec![PC::Text(WARNING.to_string())],
                        ..Default::default()
                    }),
                ]),
            )]),
            Section::new(CONTENT_SECTION_ID.to_string()).with_fragments(vec![Fragments::Fragment(
                Fragment::new(CONTENT_FRAGMENT_ID.to_string())
                    .with_content(vec![FC::Para(Para::default())]),
            )]),
        ],
        ..Default::default()
    }
}
