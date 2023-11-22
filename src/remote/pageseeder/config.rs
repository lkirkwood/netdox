use std::collections::{HashMap, HashSet};

use pageseeder::psml::{
    model::{Document, FragmentContent, PropertyValue, Section, SectionContent},
    text::ParaContent,
};
use paris::warn;

use crate::{
    config::RemoteConfig,
    config_err,
    error::{NetdoxError, NetdoxResult},
};

pub const REMOTE_CONFIG_DOCID: &str = "_nd_config";
pub const REMOTE_CONFIG_FNAME: &str = "config.psml";

const LOCATIONS_SECTION_ID: &str = "subnets";
const EXCLUDE_DNS_SECTION_ID: &str = "exclusions";
const PLUGIN_CFG_SECTION_ID: &str = ""; // TODO decide on this

pub fn parse_config(doc: Document) -> NetdoxResult<RemoteConfig> {
    let mut locations = None;
    let mut exclude_dns = None;
    let mut plugin_cfg = None;
    for section in doc.sections {
        match section.id.as_str() {
            LOCATIONS_SECTION_ID => {
                if locations.is_some() {
                    return config_err!(format!(
                        "Remote config document has two locations sections."
                    ));
                } else {
                    locations = Some(parse_locations(section))
                }
            }
            EXCLUDE_DNS_SECTION_ID => {
                if exclude_dns.is_some() {
                    return config_err!(format!(
                        "Remote config document has two dns exclusion sections."
                    ));
                } else {
                    exclude_dns = Some(parse_exclusions(section))
                }
            }
            PLUGIN_CFG_SECTION_ID => {
                if plugin_cfg.is_some() {
                    return config_err!(format!(
                        "Remote config document has two plugin config sections."
                    ));
                } else {
                    plugin_cfg = Some(parse_plugin_cfg(section))
                }
            }
            _ => {}
        }
    }

    Ok(RemoteConfig {
        locations: locations.unwrap_or_default(),
        exclude_dns: exclude_dns.unwrap_or_default(),
        plugin_cfg: plugin_cfg.unwrap_or_default(),
    })
}

fn parse_locations(section: Section) -> HashMap<String, String> {
    let mut locations = HashMap::new();
    for fragment in section.content {
        if let SectionContent::PropertiesFragment(pfrag) = fragment {
            let mut subnet = None;
            let mut location = None;
            for prop in pfrag.properties {
                match prop.name.as_str() {
                    "subnet" => {
                        if let Some(val) = prop.attr_value {
                            subnet = Some(val);
                        } else if prop.values.len() == 1 {
                            if let Some(PropertyValue::Value(string)) = prop.values.first() {
                                subnet = Some(string.to_string());
                            }
                        }
                    }
                    "location" => {
                        if let Some(val) = prop.attr_value {
                            location = Some(val);
                        } else if prop.values.len() == 1 {
                            if let Some(PropertyValue::Value(string)) = prop.values.first() {
                                location = Some(string.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(subnet), Some(location)) = (subnet, location) {
                locations.insert(subnet, location);
            }
        }
    }
    locations
}

fn parse_exclusions(section: Section) -> HashSet<String> {
    let mut exclusions = HashSet::new();
    for fragment in section.content {
        if let SectionContent::Fragment(frag) = fragment {
            if frag.id == "exclude" {
                for elem in frag.content {
                    if let FragmentContent::Para(para) = elem {
                        for item in para.content {
                            match item {
                                ParaContent::Text(text) => {
                                    exclusions.insert(text);
                                }
                                other => {
                                    warn!("Unexpected content in changelog fragment: {:?}", other);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    exclusions
}

fn parse_plugin_cfg(section: Section) -> HashMap<String, HashMap<String, String>> {
    let cfg = HashMap::new();
    for child in section.content {
        if let SectionContent::PropertiesFragment(pfrag) = child {
            for _prop in pfrag.properties {
                todo!("Decide how to parse remote config")
            }
        }
    }
    cfg
}
