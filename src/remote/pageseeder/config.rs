use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
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

/// Gets a single value from $property and assigns it to $variable.
/// Otherwise prints warning about properties in $context.
macro_rules! assign_single_prop_value {
    ($variable: ident, $property: expr, $context: expr) => {
        match (&$variable, &$property.values[..], &$property.attr_value) {
            (&None, [PropertyValue::Value(value)], None) => {
                $variable = Some(value.to_string());
            }
            (&None, [], Some(value)) => {
                $variable = Some(value.to_string());
            }
            _ => {
                warn!("Properties in {} must have exactly one value.", $context);
            }
        }
    };
}

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
                    metadata = Some(parse_metadata(section)?);
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

const LOCATIONS_CONTEXT: &str = "remote config subnet/locations assocations";

fn parse_locations(section: Section) -> HashMap<Ipv4Net, String> {
    let mut locations = HashMap::new();
    for fragment in section.content {
        if let SectionContent::PropertiesFragment(pfrag) = fragment {
            let mut subnet = None;
            let mut location = None;
            for prop in pfrag.properties {
                if prop.name == "subnet" {
                    assign_single_prop_value!(subnet, prop, LOCATIONS_CONTEXT);
                } else if prop.name == "location" {
                    assign_single_prop_value!(location, prop, LOCATIONS_CONTEXT);
                }
            }

            if let (Some(subnet), Some(location)) = (subnet, location) {
                if let Ok(ipv4net) = Ipv4Net::from_str(&subnet) {
                    locations.insert(ipv4net, location);
                } else {
                    warn!("Failed to parse subnet {subnet} in remote config locations.")
                }
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

const METADATA_CONTEXT: &str = "remote config label/metadata association";

fn parse_metadata(section: Section) -> NetdoxResult<HashMap<String, HashMap<String, String>>> {
    let mut cfg: HashMap<String, HashMap<String, String>> = HashMap::new();
    for child in section.content {
        if let SectionContent::PropertiesFragment(pfrag) = child {
            let (mut label, mut key, mut val) = (None, None, None);
            for prop in pfrag.properties {
                if prop.name == "label" {
                    assign_single_prop_value!(label, prop, METADATA_CONTEXT);
                } else if prop.name == "meta-key" {
                    assign_single_prop_value!(key, prop, METADATA_CONTEXT);
                } else if prop.name == "meta-value" {
                    if let [PropertyValue::XRef(xref)] = &prop.values[..] {
                        match &xref.docid {
                            Some(docid) => val = Some(format!("(!(external|!|{})!)", docid)),
                            None => {
                                return config_err!(
                                    "Cannot parse metadata value from xref with no docid."
                                        .to_string()
                                )
                            }
                        }
                    } else {
                        assign_single_prop_value!(val, prop, METADATA_CONTEXT);
                    }
                }
            }

            if let (Some(label), Some(key), Some(val)) = (label, key, val) {
                match cfg.entry(label) {
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().insert(key, val);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(HashMap::from([(key, val)]));
                    }
                }
            }
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use ipnet::Ipv4Net;
    use psml::model::{Fragments, PropertiesFragment, Property, PropertyValue, Section, XRef};
    use Fragments as F;
    use PropertiesFragment as PF;
    use Property as P;
    use PropertyValue as PV;

    use crate::remote::pageseeder::config::{
        parse_locations, parse_metadata, LOCATIONS_SECTION_ID,
    };

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

    #[test]
    fn test_parse_metadata() {
        let section = Section::new(LOCATIONS_SECTION_ID.to_string()).with_fragments(vec![
            F::Properties(
                // label1
                PF::new("label1".to_string()).with_properties(vec![
                    P::with_value(
                        "label".to_string(),
                        "Label Name".to_string(),
                        PV::Value("label1".to_string()),
                    ),
                    P::with_value(
                        "meta-key".to_string(),
                        "Metadata Key".to_string(),
                        PV::Value("key1".to_string()),
                    ),
                    P::with_value(
                        "meta-value".to_string(),
                        "Metadata Value".to_string(),
                        PV::Value("value1".to_string()),
                    ),
                ]),
            ),
            F::Properties(
                // label2
                PF::new("label2".to_string()).with_properties(vec![
                    P::with_value(
                        "label".to_string(),
                        "Label Name".to_string(),
                        PV::Value("label2".to_string()),
                    ),
                    P::with_value(
                        "meta-key".to_string(),
                        "Metadata Key".to_string(),
                        PV::Value("key2".to_string()),
                    ),
                    P::with_value(
                        "meta-value".to_string(),
                        "Metadata Value".to_string(),
                        PV::Value("value2".to_string()),
                    ),
                ]),
            ),
            F::Properties(
                // label3
                PF::new("label3".to_string()).with_properties(vec![
                    P::with_value(
                        "label".to_string(),
                        "Label Name".to_string(),
                        PV::Value("label3".to_string()),
                    ),
                    P::with_value(
                        "meta-key".to_string(),
                        "Metadata Key".to_string(),
                        PV::Value("key3".to_string()),
                    ),
                    P::with_value(
                        "meta-value".to_string(),
                        "Metadata Value".to_string(),
                        PV::XRef(Box::new(XRef::docid("meta-value-xref-docid".to_string()))),
                    ),
                ]),
            ),
        ]);

        let metadata: HashMap<String, HashMap<String, String>> = HashMap::from([
            (
                "label1".to_string(),
                HashMap::from([("key1".to_string(), "value1".to_string())]),
            ),
            (
                "label2".to_string(),
                HashMap::from([("key2".to_string(), "value2".to_string())]),
            ),
            (
                "label3".to_string(),
                HashMap::from([(
                    "key3".to_string(),
                    format!("(!(external|!|meta-value-xref-docid)!)"),
                )]),
            ),
        ]);

        assert_eq!(parse_metadata(section).unwrap(), metadata)
    }
}
