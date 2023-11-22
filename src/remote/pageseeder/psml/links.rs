use async_trait::async_trait;
use lazy_static::lazy_static;
use pageseeder::psml::{
    model::{BlockXRef, Document, Fragment, PropertiesFragment, Property, PropertyValue, XRef},
    text::{Heading, Para},
};
use regex::Regex;

use crate::{
    data::DataConn,
    error::{NetdoxError, NetdoxResult},
    redis_err,
    remote::pageseeder::remote::dns_qname_to_docid,
};

use pageseeder::psml::model::{FragmentContent, SectionContent};

lazy_static! {
    /// Pattern for matching links.
    /// Capture group 1 is prefix, group 2 is link kind, group 3 is link ID, group 4 is suffix.
    static ref LINK_PATTERN: Regex =
        Regex::new(r"^(.*)\(!\((dns|procnode|rawnode|report)\|!\|([\w0-9\[\]_.-]+)\)!\)(.*)$").unwrap();
}

struct Link<'a> {
    prefix: &'a str,
    id: String,
    suffix: &'a str,
}

impl<'a> Link<'a> {
    /// Parses a link from some text, if there is one.
    async fn parse_from(
        backend: &mut Box<dyn DataConn>,
        text: &'a str,
    ) -> NetdoxResult<Option<Link<'a>>> {
        match LINK_PATTERN.captures(text) {
            Some(captures) => {
                let (prefix, suffix) = (captures.get(1).unwrap(), captures.get(4).unwrap());
                let (kind, id) = (captures.get(2).unwrap(), captures.get(3).unwrap());
                let link_id = match kind.as_str() {
                    "dns" => dns_qname_to_docid(id.as_str()),
                    "procnode" => id.as_str().to_string(),
                    "rawnode" => match backend.get_node_from_raw(id.as_str()).await? {
                        Some(id) => id,
                        None => {
                            return redis_err!(format!(
                                "Failed to resolve proc node from raw node id: {}",
                                id.as_str()
                            ))
                        }
                    },
                    "report" => {
                        todo!("Link to reports from property")
                    }
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
    async fn create_links(mut self, backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self>;
}

#[async_trait]
impl LinkContent for Document {
    async fn create_links(mut self, backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self> {
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

#[async_trait]
impl LinkContent for Fragment {
    async fn create_links(mut self, backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self> {
        use FragmentContent as FC;
        let mut content = vec![];
        for item in self.content {
            match item {
                FC::BlockXRef(_) => content.push(item),
                FC::Heading(heading) => {
                    content.push(FC::Heading(heading.create_links(backend).await?))
                }
                FC::Para(para) => {
                    for string in para.content {
                        let mut item = string.as_str();
                        loop {
                            if let Some(link) = Link::parse_from(backend, item).await? {
                                content.push(FragmentContent::Para(Para::new(vec![link
                                    .prefix
                                    .to_string()])));
                                content.push(FragmentContent::BlockXRef(BlockXRef::docid(link.id)));
                                item = link.suffix;
                            } else {
                                content
                                    .push(FragmentContent::Para(Para::new(vec![item.to_string()])));
                                break;
                            }
                        }
                    }
                }
                _ => todo!(),
            }
        }

        self.content = content;

        Ok(self)
    }
}

#[async_trait]
impl LinkContent for Heading {
    async fn create_links(mut self, _backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self> {
        //TODO deal with mixing links and text
        Ok(self)
    }
}

#[async_trait]
impl LinkContent for PropertiesFragment {
    async fn create_links(mut self, backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self> {
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
    async fn create_links(mut self, backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self> {
        if let Some(val) = self.attr_value.clone() {
            match Link::parse_from(backend, &val).await? {
                Some(link) => {
                    self.attr_value = None;
                    self.values = vec![PropertyValue::XRef(XRef::docid(link.id))];
                }
                None => {}
            }
        } else {
            let mut values = vec![];
            for val in self.values {
                values.push(val.create_links(backend).await?);
            }
            self.values = values;
        }

        Ok(self)
    }
}

#[async_trait]
impl LinkContent for PropertyValue {
    async fn create_links(mut self, backend: &mut Box<dyn DataConn>) -> NetdoxResult<Self> {
        // TODO implement for markdown + markup
        match self {
            Self::Value(text) => match Link::parse_from(backend, &text).await? {
                Some(link) => Ok(PropertyValue::XRef(XRef::docid(link.id))),
                None => Ok(Self::Value(text)),
            },
            _ => Ok(self),
        }
    }
}
