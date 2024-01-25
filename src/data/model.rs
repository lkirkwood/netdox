use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
};

use redis::{FromRedisValue, RedisError};

use crate::{
    error::{NetdoxError, NetdoxResult},
    process_err, redis_err,
};

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

/// For objects that can absorb another of the same type.
pub trait Absorb {
    /// Moves all of the elements in the other object to this one.
    fn absorb(&mut self, other: Self) -> NetdoxResult<()>;
}

// DNS

/// Returns the network prefix for a qualified DNS name.
fn qname_network(qname: &str) -> Option<&str> {
    if let Some(0) = qname.find('[') {
        if let Some(end) = qname.find(']') {
            return Some(&qname[1..end]);
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
/// A superset of DNS names specific to a network.
pub struct NetworkSuperSet {
    pub network: String,
    pub names: HashSet<String>,
}

impl Hash for NetworkSuperSet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.network.hash(state);
        let mut names = self.names.iter().collect::<Vec<&String>>();
        names.sort();
        names.hash(state);
    }
}

impl Absorb for NetworkSuperSet {
    fn absorb(&mut self, other: Self) -> NetdoxResult<()> {
        if other.network != self.network {
            return process_err!(format!(
                "Cannot extend superset from network {} with one from network {}",
                self.network, other.network
            ));
        }
        self.names.extend(other.names);
        Ok(())
    }
}

impl NetworkSuperSet {
    fn new(network: String) -> Self {
        NetworkSuperSet {
            network,
            names: HashSet::new(),
        }
    }

    fn insert(&mut self, name: String) {
        self.names.insert(name);
    }
}

#[derive(Debug)]
/// Maps network names to a superset of DNS names in that network.
pub struct GlobalSuperSet(HashMap<String, NetworkSuperSet>);

impl Absorb for GlobalSuperSet {
    fn absorb(&mut self, other: Self) -> NetdoxResult<()> {
        for (net, superset) in other.0 {
            match self.entry(net) {
                Entry::Vacant(entry) => {
                    entry.insert(superset);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().absorb(superset)?;
                }
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
impl GlobalSuperSet {
    pub fn new() -> Self {
        GlobalSuperSet(HashMap::new())
    }

    pub fn contains(&self, network: &str) -> bool {
        self.0.contains_key(network)
    }

    pub fn entry(&mut self, key: String) -> Entry<String, NetworkSuperSet> {
        self.0.entry(key)
    }

    pub fn get(&self, network: &str) -> Option<&NetworkSuperSet> {
        self.0.get(network)
    }

    pub fn get_mut(&mut self, network: &str) -> Option<&mut NetworkSuperSet> {
        self.0.get_mut(network)
    }

    /// Inserts a new superset for the network, removing the old one.
    pub fn insert(&mut self, value: NetworkSuperSet) {
        self.0.insert(value.network.clone(), value);
    }

    pub fn add(&mut self, value: NetworkSuperSet) -> NetdoxResult<()> {
        match self.0.entry(value.network.clone()) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().absorb(value)?;
            }
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &NetworkSuperSet> {
        self.0.values()
    }

    pub fn into_iter(self) -> impl Iterator<Item = NetworkSuperSet> {
        self.0.into_values()
    }

    pub fn extend(&mut self, names: HashSet<String>) -> NetdoxResult<()> {
        for name in names {
            if let Some(net) = qname_network(&name) {
                match self.0.entry(net.to_string()) {
                    Entry::Occupied(mut entry) => {
                        entry.get_mut().insert(name);
                    }
                    Entry::Vacant(entry) => {
                        let mut superset = NetworkSuperSet::new(net.to_string());
                        superset.insert(name);
                        entry.insert(superset);
                    }
                }
            } else {
                return process_err!(format!(
                    "Cannot insert unqualified DNS name {name} into superset."
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
/// A set of DNS records and network translations.
pub struct DNS {
    /// Maps a DNS name to a list of DNS records with a matching name field.
    pub records: HashMap<String, HashSet<DNSRecord>>,
    /// Map a DNS name to a set of DNS names in other networks.
    pub net_translations: HashMap<String, HashSet<String>>,
    /// Map a DNS name to a set of other DNS names that point to it.
    pub implied_records: HashMap<String, HashSet<ImpliedDNSRecord>>,
}

impl Absorb for DNS {
    fn absorb(&mut self, other: Self) -> NetdoxResult<()> {
        for (qname, records) in other.records {
            match self.records.entry(qname) {
                Entry::Vacant(entry) => {
                    entry.insert(records);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().extend(records);
                }
            }
        }

        for (qname, records) in other.net_translations {
            match self.net_translations.entry(qname) {
                Entry::Vacant(entry) => {
                    entry.insert(records);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().extend(records);
                }
            }
        }

        for (qname, records) in other.implied_records {
            match self.implied_records.entry(qname) {
                Entry::Vacant(entry) => {
                    entry.insert(records);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().extend(records);
                }
            }
        }

        Ok(())
    }
}

impl DNS {
    pub fn new() -> Self {
        DNS {
            records: HashMap::new(),
            net_translations: HashMap::new(),
            implied_records: HashMap::new(),
        }
    }

    /// Returns set of all names that this DNS name resolves to/through.
    pub fn dns_superset(&self, name: &str) -> NetdoxResult<GlobalSuperSet> {
        self._dns_superset(name, &mut HashSet::new())
        // TODO implement caching for this
    }

    /// Recursive function which implements dns_superset.
    fn _dns_superset(
        &self,
        name: &str,
        seen: &mut HashSet<String>,
    ) -> NetdoxResult<GlobalSuperSet> {
        let mut supersets = GlobalSuperSet::new();
        if seen.contains(name) {
            return Ok(supersets);
        }
        seen.insert(name.to_owned());

        match qname_network(name) {
            Some(net) => match supersets.entry(net.to_owned()) {
                Entry::Vacant(entry) => {
                    let mut superset = NetworkSuperSet::new(net.to_owned());
                    superset.insert(name.to_owned());
                    entry.insert(superset);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().insert(name.to_owned());
                }
            },
            None => {
                return process_err!(format!(
                    "Cannot get superset for unqualified DNS name {name}."
                ))
            }
        }

        for record in self.get_records(name) {
            match record.rtype.as_str() {
                "A" | "CNAME" | "PTR" => {
                    supersets.absorb(self._dns_superset(&record.value, seen)?)?
                }
                _ => {}
            }
        }

        for record in self.get_implied_records(name) {
            supersets.absorb(self._dns_superset(&record.value, seen)?)?;
        }

        for translation in self.get_translations(name) {
            supersets.absorb(self._dns_superset(translation, seen)?)?;
        }

        Ok(supersets)
    }

    /// Returns the DNS superset for a node.
    pub fn node_superset(&self, node: &RawNode) -> NetdoxResult<GlobalSuperSet> {
        let mut superset = GlobalSuperSet::new();
        if node.exclusive {
            superset.extend(node.dns_names.clone())?;
        } else {
            for name in &node.dns_names {
                superset.absorb(self.dns_superset(name)?)?;
            }
        }
        Ok(superset)
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
        if let Some(implied) = record.clone().implies() {
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

    pub fn add_net_translation(&mut self, origin: &str, dest: String) {
        match self.net_translations.entry(origin.to_owned()) {
            Entry::Vacant(entry) => {
                entry.insert(HashSet::from([dest]));
            }
            Entry::Occupied(mut entry) => {
                entry.get_mut().insert(dest);
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct DNSRecord {
    pub name: String,
    pub value: String,
    pub rtype: String,
    pub plugin: String,
}

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

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
/// Distinguishes implied DNS records from actual ones.
pub struct ImpliedDNSRecord {
    pub name: String,
    pub value: String,
    pub rtype: String,
    pub plugin: String,
}

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

#[derive(Debug, PartialEq, Eq)]
/// A processed, linkable node.
pub struct Node {
    pub name: String,
    pub link_id: String,
    pub alt_names: HashSet<String>,
    pub dns_names: HashSet<String>,
    pub plugins: HashSet<String>,
    pub raw_ids: HashSet<String>,
}

impl Absorb for Node {
    fn absorb(&mut self, other: Self) -> NetdoxResult<()> {
        self.alt_names.insert(other.name);
        self.alt_names.extend(other.alt_names);
        self.dns_names.extend(other.dns_names);
        self.plugins.extend(other.plugins);
        self.raw_ids.extend(other.raw_ids);
        Ok(())
    }
}

// Other data

#[derive(Clone, Debug)]
pub enum StringType {
    HtmlMarkup,
    Markdown,
    Plain,
}

#[derive(Clone, Debug)]
/// The kinds of data.
pub enum DataKind {
    /// Data attached to a report.
    Report,
    /// Plugin data attached to a DNS name or Node.
    Plugin,
}

#[derive(Clone, Debug)]
pub enum Data {
    Hash {
        id: String,
        title: String,
        plugin: String,
        content: HashMap<String, String>,
    },
    List {
        id: String,
        list_title: String,
        item_title: String,
        plugin: String,
        content: Vec<String>,
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
    pub fn from_hash(
        id: String,
        content: HashMap<String, String>,
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

        Ok(Data::Hash {
            id,
            title,
            plugin,
            content,
        })
    }

    pub fn from_list(
        id: String,
        content: Vec<String>,
        details: HashMap<String, String>,
    ) -> NetdoxResult<Data> {
        let list_title = match details.get("list_title") {
            Some(title) => title.to_owned(),
            None => return redis_err!("List plugin data missing detail 'list_title'.".to_string()),
        };

        let item_title = match details.get("item_title") {
            Some(title) => title.to_owned(),
            None => return redis_err!("List plugin data missing detail 'item_title'.".to_string()),
        };

        let plugin = match details.get("plugin") {
            Some(plugin) => plugin.to_owned(),
            None => return redis_err!("List plugin data missing detail 'plugin'.".to_string()),
        };

        Ok(Data::List {
            id,
            list_title,
            item_title,
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

#[derive(Debug, Clone)]
/// A change recorded in the changelog.
pub enum Change {
    Init {
        id: String,
    },
    CreateDnsName {
        id: String,
        plugin: String,
        qname: String,
    },
    CreateDnsRecord {
        id: String,
        plugin: String,
        record: DNSRecord,
    },
    CreatePluginNode {
        id: String,
        plugin: String,
        node_id: String,
    },
    CreateReport {
        id: String,
        plugin: String,
        report_id: String,
    },
    UpdatedData {
        id: String,
        plugin: String,
        obj_id: String,
        data_id: String,
        kind: DataKind,
    },
    UpdatedMetadata {
        id: String,
        plugin: String,
        obj_id: String,
    },
    UpdatedNetworkMapping {
        id: String,
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
            Change::UpdatedMetadata { .. } => "updated metadata".to_string(),
            Change::UpdatedData { .. } => "updated data".to_string(),
            Change::CreateReport { .. } => "create report".to_string(),
        }
    }
}

impl Change {
    pub fn id(&self) -> &str {
        match self {
            Self::Init { id } => id,
            Self::CreateDnsName { id, .. } => id,
            Self::CreateDnsRecord { id, .. } => id,
            Self::CreatePluginNode { id, .. } => id,
            Self::CreateReport { id, .. } => id,
            Self::UpdatedData { id, .. } => id,
            Self::UpdatedMetadata { id, .. } => id,
            Self::UpdatedNetworkMapping { id, .. } => id,
        }
    }
}

impl FromRedisValue for Change {
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

        let id = match vals.get(0) {
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
            "init" => Ok(Change::Init { id }),

            "create dns name" => match val_parts.next() {
                Some(qname) => Ok(Change::CreateDnsName {
                    id,
                    plugin,
                    qname: qname.to_string(),
                }),
                None => Err(RedisError::from((
                    redis::ErrorKind::ResponseError,
                    "Invalid change value for CreateDnsName",
                    value,
                ))),
            },

            "create dns record" => match val_parts.nth(1) {
                Some(start) => match (val_parts.nth(1), val_parts.next()) {
                    (Some(rtype), Some(dest)) => Ok(Change::CreateDnsRecord {
                        id,
                        plugin: plugin.clone(),
                        record: DNSRecord {
                            name: start.to_string(),
                            value: dest.to_string(),
                            rtype: rtype.to_string(),
                            plugin,
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

            "create plugin node" => Ok(Change::CreatePluginNode {
                id,
                plugin,
                node_id: value,
            }),

            "updated metadata" => Ok(Change::UpdatedMetadata {
                id,
                plugin,
                obj_id: val_parts.skip(1).collect::<Vec<_>>().join(";"),
            }),

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

                Ok(Change::UpdatedData {
                    id,
                    plugin,
                    obj_id,
                    data_id,
                    kind,
                })
            }

            "create report" => Ok(Change::CreateReport {
                id,
                plugin,
                report_id: value,
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
