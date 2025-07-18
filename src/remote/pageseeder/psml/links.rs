use async_trait::async_trait;
use lazy_static::lazy_static;
use psml::{
    model::{
        BlockXRef, Document, Fragment, FragmentContent, Fragments, PropertiesFragment, Property,
        PropertyDatatype, PropertyValue, SectionContent, Table, XRef,
    },
    text::{CharacterStyle, Para, ParaContent},
};
use regex::{Regex, RegexBuilder};

use crate::{
    data::{DataConn, DataStore},
    error::{NetdoxError, NetdoxResult},
    redis_err,
    remote::pageseeder::remote::{dns_qname_to_docid, node_id_to_docid, report_id_to_docid},
};

const LINK_PATTERN: &str =
    r"^(.*)\(!\((dns|procnode|rawnode|report|external)\|!\|([\w0-9\[\]_.-]+)\)!\)(.*)$";

lazy_static! {
    /// Pattern for matching links.
    /// Capture group 1 is prefix, group 2 is link kind, group 3 is link ID, group 4 is suffix.
    static ref LINK_REGEX: Regex =
        RegexBuilder::new(LINK_PATTERN)
        .dot_matches_new_line(true)
        .swap_greed(true)
        .build().unwrap();
}

struct Link<'a> {
    prefix: &'a str,
    id: String,
    suffix: &'a str,
}

impl<'a> Link<'a> {
    /// Parses a link from some text, if there is one.
    async fn parse_from(backend: &mut DataStore, text: &'a str) -> NetdoxResult<Option<Link<'a>>> {
        match LINK_REGEX.captures(text) {
            Some(captures) => {
                let (prefix, suffix) = (captures.get(1).unwrap(), captures.get(4).unwrap());
                let (kind, id) = (captures.get(2).unwrap(), captures.get(3).unwrap());
                let link_id = match kind.as_str() {
                    "dns" => dns_qname_to_docid(
                        &backend
                            .qualify_dns_names(&[id.as_str()])
                            .await?
                            .pop()
                            .expect("Qualify DNS name returned 0 names."),
                    ),
                    "procnode" => node_id_to_docid(id.as_str()),
                    "rawnode" => {
                        let raw_id = backend
                            .get_raw_id_from_qnames(&id.as_str().split(';').collect::<Vec<_>>())
                            .await?;

                        match backend.get_node_from_raw(&raw_id).await? {
                            Some(id) => node_id_to_docid(&id),
                            None => {
                                return redis_err!(format!(
                                    "Failed to resolve proc node from raw node id: {}",
                                    id.as_str()
                                ))
                            }
                        }
                    }
                    "report" => report_id_to_docid(id.as_str()),
                    "external" => id.as_str().to_string(),
                    _ => unreachable!(),
                };

                Ok(Some(Link {
                    prefix: &text[prefix.start()..prefix.end()],
                    id: link_id,
                    suffix: &text[suffix.start()..suffix.end()],
                }))
            }
            None => Ok(None),
        }
    }
}

#[async_trait]
pub trait LinkContent: Sized {
    /// Searches for links in this object and inserts them
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self>;
}

#[async_trait]
impl LinkContent for Document {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        use SectionContent as SC;

        for section in &mut self.sections {
            for i in 0..section.content.len() {
                let item = &section.content[i];
                match item {
                    SC::Fragment(frag) => {
                        section.content[i] = SC::Fragment(frag.clone().create_links(backend).await?)
                    }
                    SC::PropertiesFragment(pfrag) => {
                        section.content[i] =
                            SC::PropertiesFragment(pfrag.clone().create_links(backend).await?)
                    }
                    _ => {}
                }
            }
        }

        Ok(self)
    }
}

// Fragments

#[async_trait]
impl LinkContent for Fragments {
    async fn create_links(self, backend: &mut DataStore) -> NetdoxResult<Self> {
        match self {
            Self::Fragment(frag) => Ok(Self::Fragment(frag.create_links(backend).await?)),
            Self::Properties(frag) => Ok(Self::Properties(frag.create_links(backend).await?)),
            Self::Xref(_frag) => todo!("Create links in xref fragments"),
            Self::Media(_frag) => todo!("Create links in media fragments"),
        }
    }
}

// Fragment

#[async_trait]
impl LinkContent for Fragment {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        use FragmentContent as FC;
        use ParaContent as PC;

        let mut content = vec![];
        for item in self.content {
            match item {
                FC::BlockXRef(_) => content.push(item),
                FC::Heading(heading) => {
                    content.push(FC::Heading(heading.create_links(backend).await?))
                }
                FC::Para(para) => {
                    content.push(FC::Para(para.create_links(backend).await?));
                }
                FC::Table(table) => content.push(FC::Table(table.create_links(backend).await?)),
                FC::Text(string) => {
                    let mut text = &string[..];
                    loop {
                        if let Some(link) = Link::parse_from(backend, text).await? {
                            content
                                .push(FC::Para(Para::new(vec![PC::Text(link.prefix.to_string())])));
                            content.push(FC::BlockXRef(BlockXRef::docid(link.id)));
                            text = link.suffix;
                        } else {
                            content.push(FC::Para(Para::new(vec![PC::Text(text.to_string())])));
                            break;
                        }
                    }
                }
                _ => todo!("creating links in some fragment content types"),
            }
        }

        self.content = content;

        Ok(self)
    }
}

#[async_trait]
impl LinkContent for Para {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        use ParaContent as PC;

        let mut content = vec![];
        for item in self.content {
            match item {
                PC::Text(string) => {
                    let mut text = &string[..];
                    loop {
                        if let Some(link) = Link::parse_from(backend, text).await? {
                            content.push(PC::Text(link.prefix.to_string()));
                            content.push(PC::XRef(XRef::docid(link.id)));
                            text = link.suffix;
                        } else {
                            content.push(PC::Text(text.to_string()));
                            break;
                        }
                    }
                }
                PC::XRef(_) | PC::Image(_) => content.push(item),
                // Character style
                PC::Bold(bold) => content.push(PC::Bold(bold.create_links(backend).await?)),
                PC::Italic(italic) => content.push(PC::Italic(italic.create_links(backend).await?)),
                PC::Underline(underline) => {
                    content.push(PC::Underline(underline.create_links(backend).await?))
                }
                PC::Subscript(subscript) => {
                    content.push(PC::Subscript(subscript.create_links(backend).await?))
                }
                PC::Superscript(superscript) => {
                    content.push(PC::Superscript(superscript.create_links(backend).await?))
                }
                PC::Monospace(monospace) => {
                    content.push(PC::Monospace(monospace.create_links(backend).await?))
                }
                PC::Link(link) => content.push(PC::Link(link)),
            }
        }

        self.content = content;

        Ok(self)
    }
}

#[async_trait]
impl LinkContent for Table {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        let mut rows = vec![];
        for mut row in self.rows {
            let mut cells = vec![];
            for cell in row.cells {
                cells.push(cell.create_links(backend).await?);
            }
            row.cells = cells;
            rows.push(row);
        }

        self.rows = rows;

        Ok(self)
    }
}

// Text / Character style

macro_rules! impl_char_style_link_content {
    ($name:ty) => {
        #[async_trait]
        impl LinkContent for $name {
            async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
                use CharacterStyle as CS;

                let mut content = vec![];
                for item in self.content {
                    match item {
                        CS::Text(string) => {
                            let mut text = &string[..];
                            loop {
                                if let Some(link) = Link::parse_from(backend, text).await? {
                                    content.push(CS::Text(link.prefix.to_string()));
                                    content.push(CS::XRef(Box::new(XRef::docid(link.id))));
                                    text = link.suffix;
                                } else {
                                    content.push(CS::Text(text.to_string()));
                                    break;
                                }
                            }
                        }
                        CS::XRef(_) => content.push(item),
                        CS::Bold(bold) => content.push(CS::Bold(bold.create_links(backend).await?)),
                        CS::Italic(italic) => {
                            content.push(CS::Italic(italic.create_links(backend).await?))
                        }
                        CS::Underline(underline) => {
                            content.push(CS::Underline(underline.create_links(backend).await?))
                        }
                        CS::Subscript(subscript) => {
                            content.push(CS::Subscript(subscript.create_links(backend).await?))
                        }
                        CS::Superscript(superscript) => {
                            content.push(CS::Superscript(superscript.create_links(backend).await?))
                        }
                        CS::Monospace(monospace) => {
                            content.push(CS::Monospace(monospace.create_links(backend).await?))
                        }
                        CS::Link(link) => content.push(CS::Link(link)),
                    }
                }

                self.content = content;

                Ok(self)
            }
        }
    };
}

impl_char_style_link_content!(psml::text::Bold);
impl_char_style_link_content!(psml::text::Italic);
impl_char_style_link_content!(psml::text::Underline);
impl_char_style_link_content!(psml::text::Subscript);
impl_char_style_link_content!(psml::text::Superscript);
impl_char_style_link_content!(psml::text::Monospace);
impl_char_style_link_content!(psml::text::Heading);
impl_char_style_link_content!(psml::model::TableCell);

// Properties Fragment

#[async_trait]
impl LinkContent for PropertiesFragment {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        let mut props = vec![];
        for prop in self.properties {
            props.push(prop.create_links(backend).await?);
        }

        self.properties = props;

        Ok(self)
    }
}

#[async_trait]
impl LinkContent for Property {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        if let Some(val) = self.attr_value.clone() {
            if let Some(link) = Link::parse_from(backend, &val).await? {
                self.attr_value = None;
                self.values = vec![PropertyValue::XRef(Box::new(XRef::docid(link.id)))];
                self.datatype = Some(PropertyDatatype::XRef);
            }
        } else if self.values.len() == 1 {
            if let Some(PropertyValue::Value(string)) = self.values.first() {
                if let Some(link) = Link::parse_from(backend, string).await? {
                    self.values = vec![PropertyValue::XRef(Box::new(XRef::docid(link.id)))];
                    self.datatype = Some(PropertyDatatype::XRef);
                }
            }
        }

        Ok(self)
    }
}

#[async_trait]
impl LinkContent for PropertyValue {
    async fn create_links(mut self, backend: &mut DataStore) -> NetdoxResult<Self> {
        // TODO implement for markdown + markup
        match self {
            Self::Value(text) => match Link::parse_from(backend, &text).await? {
                Some(link) => Ok(PropertyValue::XRef(Box::new(XRef::docid(link.id)))),
                None => Ok(Self::Value(text)),
            },
            _ => Ok(self),
        }
    }
}
