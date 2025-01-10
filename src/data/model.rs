use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
};

use indexmap::IndexMap;
use redis::{FromRedisValue, RedisError};

use crate::{
    error::{NetdoxError, NetdoxResult},
    redis_err,
};

pub const NETDOX_PLUGIN: &str = "netdox";

pub const DEFAULT_NETWORK_KEY: &str = "default_network";
pub const CHANGELOG_KEY: &str = "changelog";
pub const DNS_KEY: &str = "dns";
pub const NODES_KEY: &str = "nodes";
pub const DNS_NODES_KEY: &str = "dns_nodes";
pub const PROC_NODES_KEY: &str = "proc_nodes";
pub const PROC_NODE_REVS_KEY: &str = "proc_node_revs";
pub const REPORTS_KEY: &str = "reports";
pub const PDATA_KEY: &str = "pdata";
pub const METADATA_KEY: &str = "meta";

pub const LOCATIONS_PLUGIN: &str = "locations";
pub const LOCATIONS_META_KEY: &str = "location";

#[allow(clippy::upper_case_acronyms)]
/// An ID for each object that creates a document.
pub enum ObjectID {
    Report(String),
    DNS(String),
    Node(String),
}

// DNS

#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
/// A set of DNS records and network translations.
pub struct DNS {
    /// All the unique names that appear throughout the data store.
    pub qnames: HashSet<String>,
    /// Maps a DNS name to a list of DNS records with a matching name field.
    pub records: HashMap<String, HashSet<DNSRecord>>,
    /// Map a DNS name to a set of DNS names in other networks.
    pub net_translations: HashMap<String, HashSet<String>>,
    /// Map a DNS name to a set of other DNS names that point to it.
    pub implied_records: HashMap<String, HashSet<ImpliedDNSRecord>>,
}

impl DNS {
    pub fn new() -> Self {
        DNS {
            qnames: HashSet::new(),
            records: HashMap::new(),
            net_translations: HashMap::new(),
            implied_records: HashMap::new(),
        }
    }

    /// Returns set of all names that this DNS name resolves to/through.
    pub fn dns_superset(&self, name: &str) -> NetdoxResult<HashSet<String>> {
        self._dns_superset(name, &mut HashSet::new())
        // TODO implement caching for this
    }

    /// Recursive function which implements dns_superset.
    fn _dns_superset(
        &self,
        name: &str,
        seen: &mut HashSet<String>,
    ) -> NetdoxResult<HashSet<String>> {
        let mut superset = HashSet::from([name.to_owned()]);
        if seen.contains(name) {
            return Ok(superset);
        }
        seen.insert(name.to_owned());

        for record in self.get_records(name) {
            match record.rtype.as_str() {
                "A" | "CNAME" | "PTR" | "NAT" => {
                    superset.extend(self._dns_superset(&record.value, seen)?);
                }
                _ => {}
            }
        }

        for record in self.get_implied_records(name) {
            superset.extend(self._dns_superset(&record.value, seen)?);
        }

        for translation in self.get_translations(name) {
            superset.extend(self._dns_superset(translation, seen)?);
        }

        Ok(superset)
    }

    /// Returns the DNS superset for a node.
    pub fn node_superset(&self, node: &RawNode) -> NetdoxResult<HashSet<String>> {
        let mut superset = HashSet::new();
        if node.exclusive {
            superset.extend(node.dns_names.clone());
        } else {
            for name in &node.dns_names {
                superset.extend(self.dns_superset(name)?);
            }
        }
        Ok(superset)
    }

    /// Walks through forward DNS records (not implied ones) and returns
    /// the terminating names.
    pub fn forward_march<'a>(&'a self, name: &'a str) -> Vec<&'a str> {
        let mut seen = HashSet::new();
        self._forward_march(name, &mut seen)
    }

    fn _forward_march<'a>(&'a self, name: &'a str, seen: &mut HashSet<&'a str>) -> Vec<&'a str> {
        if seen.contains(name) {
            return vec![];
        }
        seen.insert(name);

        let records = self.get_records(name);
        if records.is_empty() {
            return vec![name];
        }

        if records
            .iter()
            .all(|record| seen.contains(record.value.as_str()))
        {
            return vec![name];
        }

        records
            .iter()
            .flat_map(|record| self._forward_march(&record.value, seen))
            .collect()
    }

    // GETTERS

    pub fn get_records(&self, name: &str) -> HashSet<&DNSRecord> {
        match self.records.get(name) {
            Some(set) => set.iter().collect(),
            None => HashSet::new(),
        }
    }

    pub fn get_translations(&self, name: &str) -> HashSet<&String> {
        match self.net_translations.get(name) {
            Some(set) => set.iter().collect(),
            None => HashSet::new(),
        }
    }

    pub fn get_implied_records(&self, name: &str) -> HashSet<&ImpliedDNSRecord> {
        match self.implied_records.get(name) {
            Some(set) => set.iter().collect(),
            None => HashSet::new(),
        }
    }

    // SETTERS

    pub fn add_record(&mut self, record: DNSRecord) {
        self.qnames.insert(record.name.clone());
        if let Some(implied) = record.clone().implies() {
            self.qnames.insert(record.value.clone());
            match self.implied_records.entry(record.value.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(HashSet::from([implied]));
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().insert(implied);
                }
            }
        }

        match self.records.entry(record.name.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(HashSet::from([record]));
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().insert(record);
            }
        }
    }
}

/// TODO make fields a reference to DNS data
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct DNSRecord {
    pub name: String,
    pub value: String,
    pub rtype: String,
    pub plugin: String,
}

pub const ADDRESS_RTYPES: [&str; 3] = ["A", "PTR", "CNAME"];

impl DNSRecord {
    pub fn implies(&self) -> Option<ImpliedDNSRecord> {
        let new_rtype = match self.rtype.as_str() {
            "A" => "PTR".to_string(),
            "PTR" => "A".to_string(),
            "CNAME" => self.rtype.to_owned(),
            _ => return None,
        };

        Some(ImpliedDNSRecord {
            name: self.value.to_owned(),
            value: self.name.to_owned(),
            rtype: new_rtype,
            plugin: self.plugin.to_owned(),
        })
    }
}

impl From<ImpliedDNSRecord> for DNSRecord {
    fn from(value: ImpliedDNSRecord) -> Self {
        DNSRecord {
            name: value.name,
            value: value.value,
            rtype: value.rtype,
            plugin: value.plugin,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
/// Distinguishes implied DNS records from actual ones.
pub struct ImpliedDNSRecord {
    pub name: String,
    pub value: String,
    pub rtype: String,
    pub plugin: String,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum DNSRecords {
    Actual(DNSRecord),
    Implied(ImpliedDNSRecord),
}

impl DNSRecords {
    pub fn name(&self) -> &str {
        match self {
            Self::Actual(record) => &record.name,
            Self::Implied(record) => &record.name,
        }
    }
    pub fn value(&self) -> &str {
        match self {
            Self::Actual(record) => &record.value,
            Self::Implied(record) => &record.value,
        }
    }
    pub fn plugin(&self) -> &str {
        match self {
            Self::Actual(record) => &record.plugin,
            Self::Implied(record) => &record.plugin,
        }
    }
    pub fn rtype(&self) -> &str {
        match self {
            Self::Actual(record) => &record.rtype,
            Self::Implied(record) => &record.rtype,
        }
    }
}

// Nodes

#[derive(Debug, PartialEq, Eq)]
/// An unprocessed node.
pub struct RawNode {
    pub name: Option<String>,
    pub dns_names: HashSet<String>,
    pub link_id: Option<String>,
    pub exclusive: bool,
    pub plugin: String,
}

impl Hash for RawNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);

        let mut names = self.dns_names.iter().collect::<Vec<&String>>();
        names.sort();
        names.hash(state);

        self.link_id.hash(state);
        self.exclusive.hash(state);
        self.plugin.hash(state);
    }
}

impl RawNode {
    pub fn id(&self) -> String {
        let mut id = String::new();

        let mut names = self.dns_names.iter().collect::<Vec<_>>();
        names.sort();

        let mut first = true;
        for name in names {
            if first {
                first = false;
            } else {
                id.push(';');
            }
            id.push_str(name);
        }

        id
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// A processed, linkable node.
pub struct Node {
    pub name: String,
    pub link_id: String,
    pub alt_names: HashSet<String>,
    pub dns_names: HashSet<String>,
    pub plugins: HashSet<String>,
    pub raw_ids: HashSet<String>,
}

// Other data

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StringType {
    HtmlMarkup,
    Markdown,
    Plain,
}

impl From<StringType> for &'static str {
    fn from(value: StringType) -> Self {
        match value {
            StringType::Plain => "plain",
            StringType::Markdown => "markdown",
            StringType::HtmlMarkup => "html-markup",
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// The kinds of data.
pub enum DataKind {
    /// Data attached to a report.
    Report,
    /// Plugin data attached to a DNS name or Node.
    Plugin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Data {
    Hash {
        id: String,
        title: String,
        plugin: String,
        content: IndexMap<String, String>,
    },
    List {
        id: String,
        title: String,
        plugin: String,
        content: Vec<(String, String, String)>,
    },
    String {
        id: String,
        title: String,
        content_type: StringType,
        plugin: String,
        content: String,
    },
    Table {
        id: String,
        title: String,
        columns: usize,
        plugin: String,
        content: Vec<String>,
    },
}

impl Data {
    pub fn id(&self) -> &str {
        match self {
            Self::Hash { id, .. } => id,
            Self::List { id, .. } => id,
            Self::String { id, .. } => id,
            Self::Table { id, .. } => id,
        }
    }

    pub fn from_hash(
        id: String,
        mut content: HashMap<String, String>,
        order: Vec<String>,
        details: HashMap<String, String>,
    ) -> NetdoxResult<Data> {
        let title = match details.get("title") {
            Some(title) => title.to_owned(),
            None => return redis_err!("Hash plugin data missing detail 'title'.".to_string()),
        };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!("Hash plugin data missing detail 'plugin'.".to_string()),
        };

        if !order.iter().all(|k| content.contains_key(k)) {
            return redis_err!(
                "Hash data does not contain all keys listed in ordering list.".to_string()
            );
        }

        Ok(Data::Hash {
            id,
            title,
            plugin,
            content: IndexMap::from_iter(
                order
                    .into_iter()
                    .map(|k| (k.clone(), content.remove(&k).unwrap())),
            ),
        })
    }

    pub fn from_list(
        id: String,
        content: Vec<(String, String, String)>,
        details: HashMap<String, String>,
    ) -> NetdoxResult<Data> {
        let title = match details.get("title") {
            Some(title) => title.to_owned(),
            None => return redis_err!("List plugin data missing detail 'title'.".to_string()),
        };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!("List plugin data missing detail 'plugin'.".to_string()),
        };

        Ok(Data::List {
            id,
            title,
            plugin,
            content,
        })
    }

    pub fn from_string(
        id: String,
        content: String,
        details: HashMap<String, String>,
    ) -> NetdoxResult<Data> {
        let title = match details.get("title") {
            Some(title) => title.to_owned(),
            None => return redis_err!("String plugin data missing detail 'title'.".to_string()),
        };

        let content_type = match details.get("content_type") {
            Some(ctype) if ctype == "html-markup" => StringType::HtmlMarkup,
            Some(ctype) if ctype == "markdown" => StringType::Markdown,
            Some(ctype) if ctype == "plain" => StringType::Plain,
            Some(other) => {
                return redis_err!(format!(
                    "String plugin data has invalid content type: {other}"
                ))
            }
            None => {
                return redis_err!("String plugin data missing detail 'content_type'.".to_string())
            }
        };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!("String plugin data missing detail 'plugin'.".to_string()),
        };

        Ok(Data::String {
            id,
            title,
            content_type,
            plugin,
            content,
        })
    }

    pub fn from_table(
        id: String,
        content: Vec<String>,
        details: HashMap<String, String>,
    ) -> NetdoxResult<Self> {
        let title = match details.get("title") {
            Some(title) => title.to_owned(),
            None => return redis_err!("Table data missing detail 'title'.".to_string()),
        };

        let columns = match details.get("columns") {
            Some(columns) => match columns.parse::<usize>() {
                Ok(int) => int,
                Err(_err) => {
                    return redis_err!(format!("Failed to parse table columns as int: {columns}"))
                }
            },
            None => return redis_err!("Table data missing detail 'columns'.".to_string()),
        };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!("Table data missing detail 'plugin'.".to_string()),
        };

        Ok(Data::Table {
            id,
            title,
            columns,
            plugin,
            content,
        })
    }
}

pub struct Report {
    pub id: String,
    pub title: String,
    pub plugin: String,
    pub content: Vec<Data>,
}

pub struct ChangelogEntry {
    pub id: String,
    pub change: Change,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
/// A change recorded in the changelog.
pub enum Change {
    Init,
    CreateDnsName {
        plugin: String,
        qname: String,
    },
    CreateDnsRecord {
        plugin: String,
        record: DNSRecord,
    },
    CreatePluginNode {
        plugin: String,
        node_id: String,
    },
    CreateReport {
        plugin: String,
        report_id: String,
    },
    CreatedData {
        plugin: String,
        obj_id: String,
        data_id: String,
        kind: DataKind,
    },
    UpdatedData {
        plugin: String,
        obj_id: String,
        data_id: String,
        kind: DataKind,
    },
    UpdatedMetadata {
        plugin: String,
        obj_id: String,
    },
    UpdatedNetworkMapping {
        plugin: String,
        source: String,
        dest: String,
    },
}

impl From<&Change> for String {
    fn from(value: &Change) -> Self {
        match value {
            Change::Init { .. } => "init".to_string(),
            Change::CreateDnsName { .. } => "create dns name".to_string(),
            Change::CreateDnsRecord { .. } => "create dns record".to_string(),
            Change::UpdatedNetworkMapping { .. } => "updated network mapping".to_string(),
            Change::CreatePluginNode { .. } => "create plugin node".to_string(),
            Change::CreatedData { .. } => "created data".to_string(),
            Change::UpdatedMetadata { .. } => "updated metadata".to_string(),
            Change::UpdatedData { .. } => "updated data".to_string(),
            Change::CreateReport { .. } => "create report".to_string(),
        }
    }
}

impl FromRedisValue for ChangelogEntry {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let vals = match v {
            redis::Value::Bulk(vals) => vals,
            _ => {
                return Err(RedisError::from((
                    redis::ErrorKind::TypeError,
                    "Changelog stream values must be bulk types",
                )));
            }
        };

        let id = match vals.first() {
            Some(redis::Value::Data(id_bytes)) => String::from_utf8_lossy(id_bytes).to_string(),
            _ => {
                return Err(RedisError::from((
                    redis::ErrorKind::TypeError,
                    "Changelog stream sequence first value must be id (data)",
                )))
            }
        };

        let mut map: HashMap<String, String> = match vals.get(1) {
            Some(bulk) => match HashMap::from_redis_value(bulk) {
                Ok(map) => map,
                Err(err) => {
                    return Err(RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Failed to parse fields of change as hash map",
                        err.to_string(),
                    )))
                }
            },
            _ => {
                return Err(RedisError::from((
                    redis::ErrorKind::TypeError,
                    "Changelog stream sequence second value must be fields (bulk)",
                )))
            }
        };

        let (change, value, plugin) = match (
            map.remove("change"),
            map.remove("value"),
            map.remove("plugin"),
        ) {
            (Some(c), Some(v), Some(p)) => (c, v, p),
            _ => {
                return Err(RedisError::from((
                    redis::ErrorKind::ResponseError,
                    "Changelog item did not have required fields.",
                )))
            }
        };

        let mut val_parts = value.split(';');
        match change.as_str() {
            "init" => Ok(ChangelogEntry {
                id,
                change: Change::Init,
            }),

            "create dns name" => match val_parts.next() {
                Some(qname) => Ok(ChangelogEntry {
                    id,
                    change: Change::CreateDnsName {
                        plugin,
                        qname: qname.to_string(),
                    },
                }),
                None => Err(RedisError::from((
                    redis::ErrorKind::ResponseError,
                    "Invalid change value for CreateDnsName",
                    value,
                ))),
            },

            "create dns record" => match val_parts.nth(1) {
                Some(start) => match (val_parts.nth(1), val_parts.next()) {
                    (Some(rtype), Some(dest)) => Ok(ChangelogEntry {
                        id,
                        change: Change::CreateDnsRecord {
                            plugin: plugin.clone(),
                            record: DNSRecord {
                                name: start.to_string(),
                                value: dest.to_string(),
                                rtype: rtype.to_string(),
                                plugin,
                            },
                        },
                    }),
                    _ => Err(RedisError::from((
                        redis::ErrorKind::ResponseError,
                        "Invalid change value for CreateDnsRecord",
                        value,
                    ))),
                },
                None => Err(RedisError::from((
                    redis::ErrorKind::ResponseError,
                    "Invalid change value for CreateDnsRecord",
                    value,
                ))),
            },

            "create plugin node" => Ok(ChangelogEntry {
                id,
                change: Change::CreatePluginNode {
                    plugin,
                    node_id: value,
                },
            }),

            "updated metadata" => Ok(ChangelogEntry {
                id,
                change: Change::UpdatedMetadata {
                    plugin,
                    obj_id: val_parts.skip(1).collect::<Vec<_>>().join(";"),
                },
            }),

            "created data" => {
                let data_id = match val_parts.clone().last() {
                    Some(id) => id.to_string(),
                    None => {
                        return Err(RedisError::from((
                            redis::ErrorKind::ResponseError,
                            "Invalid change value for CreatedData",
                            value,
                        )))
                    }
                };

                let (obj_id, kind) = match val_parts.next() {
                    Some(PDATA_KEY) => (
                        val_parts
                            .take_while(|i| *i != data_id)
                            .collect::<Vec<_>>()
                            .join(";"),
                        DataKind::Plugin,
                    ),
                    Some(REPORTS_KEY) => (
                        format!(
                            "{REPORTS_KEY};{}",
                            val_parts
                                .take_while(|i| *i != data_id)
                                .collect::<Vec<_>>()
                                .join(";")
                        ),
                        DataKind::Report,
                    ),
                    _ => {
                        return Err(RedisError::from((
                            redis::ErrorKind::ResponseError,
                            "Invalid change value for CreatedData",
                            value,
                        )))
                    }
                };

                Ok(ChangelogEntry {
                    id,
                    change: Change::CreatedData {
                        plugin,
                        obj_id,
                        data_id,
                        kind,
                    },
                })
            }

            "updated data" => {
                let data_id = match val_parts.clone().last() {
                    Some(id) => id.to_string(),
                    None => {
                        return Err(RedisError::from((
                            redis::ErrorKind::ResponseError,
                            "Invalid change value for UpdatedData",
                            value,
                        )))
                    }
                };

                let (obj_id, kind) = match val_parts.next() {
                    Some(PDATA_KEY) => (
                        val_parts
                            .take_while(|i| *i != data_id)
                            .collect::<Vec<_>>()
                            .join(";"),
                        DataKind::Plugin,
                    ),
                    Some(REPORTS_KEY) => (
                        format!(
                            "{REPORTS_KEY};{}",
                            val_parts
                                .take_while(|i| *i != data_id)
                                .collect::<Vec<_>>()
                                .join(";")
                        ),
                        DataKind::Report,
                    ),
                    _ => {
                        return Err(RedisError::from((
                            redis::ErrorKind::ResponseError,
                            "Invalid change value for UpdatedData",
                            value,
                        )))
                    }
                };

                Ok(ChangelogEntry {
                    id,
                    change: Change::UpdatedData {
                        plugin,
                        obj_id,
                        data_id,
                        kind,
                    },
                })
            }

            "create report" => Ok(ChangelogEntry {
                id,
                change: Change::CreateReport {
                    plugin,
                    report_id: value,
                },
            }),

            "updated network mapping" => todo!("network mapping change parsing"),

            other => Err(RedisError::from((
                redis::ErrorKind::ResponseError,
                "Unrecognised change in log",
                other.to_string(),
            ))),
        }
    }
}
