use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use ipnet::Ipv4Net;
use paris::warn;
use psml::{
    model::{Document, FragmentContent, PropertyValue, Section, SectionContent},
    text::ParaContent,
};

use crate::{
    config::RemoteConfig,
    config_err,
    error::{NetdoxError, NetdoxResult},
};

pub const REMOTE_CONFIG_DOCID: &str = "_nd_config";
pub const REMOTE_CONFIG_FNAME: &str = "_nd_config.psml";

pub const LOCATIONS_SECTION_ID: &str = "locations";
pub const EXCLUSIONS_SECTION_ID: &str = "exclusions";
pub const METADATA_SECTION_ID: &str = "metadata";

pub fn parse_config(doc: Document) -> NetdoxResult<RemoteConfig> {
    let mut locations = None;
    let mut exclusions = None;
    let mut metadata = None;
    for section in doc.sections {
        match section.id.as_str() {
            LOCATIONS_SECTION_ID => {
                if locations.is_some() {
                    return config_err!(format!(
                        "Remote config document has two locations sections."
                    ));
                } else {
                    locations = Some(parse_locations(section));
                }
            }
            EXCLUSIONS_SECTION_ID => {
                if exclusions.is_some() {
                    return config_err!(format!(
                        "Remote config document has two dns exclusion sections."
                    ));
                } else {
                    exclusions = Some(parse_exclusions(section));
                }
            }
            METADATA_SECTION_ID => {
                if metadata.is_some() {
                    return config_err!(format!(
                        "Remote config document has two plugin config sections."
                    ));
                } else {
                    metadata = Some(parse_metadata(section));
                }
            }
            _ => {}
        }
    }

    Ok(RemoteConfig {
        locations: locations.unwrap_or_default(),
        exclusions: exclusions.unwrap_or_default(),
        metadata: metadata.unwrap_or_default(),
    })
}

fn parse_locations(section: Section) -> HashMap<Ipv4Net, String> {
    let mut locations = HashMap::new();
    for fragment in section.content {
        if let SectionContent::PropertiesFragment(pfrag) = fragment {
            let mut subnet = None;
            let mut location = None;
            for prop in pfrag.properties {
                match prop.name.as_str() {
                    "subnet" => {
                        if let Some(val) = prop.attr_value {
                            if let Ok(_subnet) = Ipv4Net::from_str(&val) {
                                subnet = Some(_subnet);
                            }
                        } else if prop.values.len() == 1 {
                            if let Some(PropertyValue::Value(string)) = prop.values.first() {
                                if let Ok(_subnet) = Ipv4Net::from_str(string) {
                                    subnet = Some(_subnet);
                                }
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

fn parse_metadata(section: Section) -> HashMap<String, HashMap<String, String>> {
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

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use ipnet::Ipv4Net;
    use psml::model::{Fragments, PropertiesFragment, Property, PropertyValue, Section};
    use Fragments as F;
    use PropertiesFragment as PF;
    use Property as P;
    use PropertyValue as PV;

    use crate::remote::pageseeder::config::{parse_locations, LOCATIONS_SECTION_ID};

    #[test]
    fn test_parse_locations() {
        let section = Section::new(LOCATIONS_SECTION_ID.to_string()).with_fragments(vec![
            F::Properties(
                // loc1
                PF::new("loc1".to_string()).with_properties(vec![
                    P::with_value(
                        "subnet".to_string(),
                        "Subnet".to_string(),
                        PV::Value("192.168.0.0/24".to_string()),
                    ),
                    P::with_value(
                        "location".to_string(),
                        "Location".to_string(),
                        PV::Value("Loc1".to_string()),
                    ),
                ]),
            ),
            // loc2
            F::Properties(PF::new("loc2".to_string()).with_properties(vec![
                P::with_value(
                    "subnet".to_string(),
                    "Subnet".to_string(),
                    PV::Value("192.168.0.0/28".to_string()),
                ),
                P::with_value(
                    "location".to_string(),
                    "Location".to_string(),
                    PV::Value("Loc2".to_string()),
                ),
            ])),
            // loc3
            F::Properties(PF::new("loc3".to_string()).with_properties(vec![
                P::with_value(
                    "subnet".to_string(),
                    "Subnet".to_string(),
                    PV::Value("192.168.1.0/30".to_string()),
                ),
                P::with_value(
                    "location".to_string(),
                    "Location".to_string(),
                    PV::Value("Loc3".to_string()),
                ),
            ])),
        ]);

        let locations = HashMap::from([
            (
                Ipv4Net::from_str("192.168.0.0/24").unwrap(),
                "Loc1".to_string(),
            ),
            (
                Ipv4Net::from_str("192.168.0.0/28").unwrap(),
                "Loc2".to_string(),
            ),
            (
                Ipv4Net::from_str("192.168.1.0/30").unwrap(),
                "Loc3".to_string(),
            ),
        ]);

        assert_eq!(locations, parse_locations(section));
    }
}
