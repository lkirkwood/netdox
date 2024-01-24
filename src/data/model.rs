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
    fn implies(self) -> Option<ImpliedDNSRecord> {
        let new_rtype = match self.rtype.as_str() {
            "A" => "PTR".to_string(),
            "PTR" => "A".to_string(),
            "CNAME" => self.rtype,
            _ => return None,
        };

        Some(ImpliedDNSRecord {
            name: self.value,
            value: self.name,
            rtype: new_rtype,
            plugin: self.plugin,
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

pub enum StringType {
    HtmlMarkup,
    Markdown,
    Plain,
}

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
/// The different kinds of changes that can be made to the data layer.
pub enum ChangeType {
    CreateDnsName,
    CreateDnsRecord,
    UpdatedNetworkMapping,
    CreatePluginNode,
    UpdatedMetadata,
    UpdatedData,
    CreateReport,
}

impl TryFrom<&str> for ChangeType {
    type Error = NetdoxError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "create dns name" => Ok(ChangeType::CreateDnsName),
            "create dns record" => Ok(ChangeType::CreateDnsRecord),
            "updated network mapping" => Ok(ChangeType::UpdatedNetworkMapping),
            "create plugin node" => Ok(ChangeType::CreatePluginNode),
            "updated metadata" => Ok(ChangeType::UpdatedMetadata),
            "updated data" => Ok(ChangeType::UpdatedData),
            "create report" => Ok(ChangeType::CreateReport),
            _ => Err(Self::Error::Redis(format!("Unknown change type: {value}"))),
        }
    }
}

impl From<&ChangeType> for String {
    fn from(value: &ChangeType) -> Self {
        match value {
            ChangeType::CreateDnsName => "create dns name".to_string(),
            ChangeType::CreateDnsRecord => "create dns record".to_string(),
            ChangeType::UpdatedNetworkMapping => "updated network mapping".to_string(),
            ChangeType::CreatePluginNode => "create plugin node".to_string(),
            ChangeType::UpdatedMetadata => "updated metadata".to_string(),
            ChangeType::UpdatedData => "updated data".to_string(),
            ChangeType::CreateReport => "create report".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
/// A record of a change made to the data layer.
pub struct Change {
    pub id: String,
    pub change: ChangeType,
    pub value: String,
    pub plugin: String,
}

impl Change {
    /// Returns the ID of the DNS name/Node/Report that this change targets.
    pub fn target_id(&self) -> NetdoxResult<String> {
        use ChangeType as CT;

        match self.change {
            CT::CreateDnsName | CT::CreatePluginNode | CT::CreateReport => {
                Ok(self.value.to_owned())
            }
            CT::UpdatedMetadata => match self.value.split_once(';') {
                Some((_, id)) => Ok(id.to_owned()),
                None => redis_err!(format!(
                    "{} change value invalid: {}",
                    String::from(&self.change),
                    self.value
                )),
            },
            CT::CreateDnsRecord => {
                let mut parts = self.value.splitn(3, ';').skip(1);
                match parts.next() {
                    Some(id) => Ok(id.to_owned()),
                    None => redis_err!(format!(
                        "CreateDnsRecord change value invalid: {}",
                        self.value
                    )),
                }
            }
            CT::UpdatedData => match self.value.split_once(';') {
                Some((PDATA_KEY, remainder)) => {
                    match remainder
                        .split(';')
                        .skip(1)
                        .collect::<Vec<_>>()
                        .split_last()
                    {
                        Some((_, id)) => Ok(id.join(";")),
                        None => {
                            redis_err!(format!("UpdatedData change value invalid: {}", self.value))
                        }
                    }
                }
                Some((REPORTS_KEY, remainder)) => match remainder.rsplit_once(';') {
                    Some((id, _)) => Ok(id.to_string()),
                    None => redis_err!(format!("UpdatedData change value invalid: {}", self.value)),
                },
                _ => redis_err!(format!("UpdatedData change value invalid: {}", self.value)),
            },
            CT::UpdatedNetworkMapping => todo!("Network mapping changelog"),
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
            Some(redis::Value::Data(id_bytes)) => String::from_utf8_lossy(id_bytes),
            _ => {
                return Err(RedisError::from((
                    redis::ErrorKind::TypeError,
                    "Changelog stream sequence first value must be id (data)",
                )))
            }
        };

        let map: HashMap<String, String> = match vals.get(1) {
            Some(bulk) => match HashMap::from_redis_value(bulk) {
                Ok(map) => map,
                Err(err) => {
                    return Err(RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Failed to parse changelog fields as hash map",
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

        if let (Some(change), Some(value), Some(plugin)) =
            (map.get("change"), map.get("value"), map.get("plugin"))
        {
            match ChangeType::try_from(change.as_str()) {
                Ok(change) => Ok(Change {
                    id: id.to_string(),
                    change,
                    value: value.to_string(),
                    plugin: plugin.to_string(),
                }),
                Err(err) => Err(RedisError::from((
                    redis::ErrorKind::ResponseError,
                    "Failed to parse changelog",
                    err.to_string(),
                ))),
            }
        } else {
            Err(RedisError::from((
                redis::ErrorKind::ResponseError,
                "Changelog item did not have required fields.",
            )))
        }
    }
}
