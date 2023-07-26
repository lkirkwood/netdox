use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    hash::Hash,
};

use redis::{Commands, Connection};

use crate::{
    error::{NetdoxError, NetdoxResult},
    process_err, redis_err,
};

pub const DNS_KEY: &str = "dns";
pub const NODES_KEY: &str = "nodes";
pub const PROC_DB: u8 = 1;

/// For objects that can absorb another of the same type.
pub trait Absorb {
    /// Moves all of the elements in the other object to this one.
    fn absorb(&mut self, other: Self) -> NetdoxResult<()>;
}

// DNS

const ADDRESS_RTYPES: [&str; 3] = ["CNAME", "A", "PTR"];

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
    records: HashMap<String, Vec<DNSRecord>>,
    /// Map a DNS name to a set of DNS names in other networks.
    net_translations: HashMap<String, HashSet<String>>,
    /// Map a DNS name to a set of other DNS names that point to it.
    rev_ptrs: HashMap<String, HashSet<String>>,
}

impl Absorb for DNS {
    fn absorb(&mut self, other: Self) -> NetdoxResult<()> {
        self.records.extend(other.records);
        self.net_translations.extend(other.net_translations);
        self.rev_ptrs.extend(other.rev_ptrs);
        Ok(())
    }
}

impl DNS {
    pub fn new() -> Self {
        DNS {
            records: HashMap::new(),
            net_translations: HashMap::new(),
            rev_ptrs: HashMap::new(),
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
            Some(net) => {
                if !supersets.contains(net) {
                    supersets.insert(NetworkSuperSet::new(net.to_owned()));
                }
                supersets.get_mut(net).unwrap().insert(name.to_owned());
            }
            None => {
                return process_err!(format!(
                    "Cannot get superset for unqualified DNS name {name}."
                ))
            }
        }

        for record in self.get_records(name) {
            supersets.absorb(self._dns_superset(&record.value, seen)?)?;
        }

        for name in self.get_rev_ptrs(name) {
            supersets.absorb(self._dns_superset(name, seen)?)?;
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

    pub fn get_records(&self, name: &str) -> Vec<&DNSRecord> {
        match self.records.get(name) {
            Some(vec) => vec.iter().collect(),
            None => vec![],
        }
    }

    pub fn get_translations(&self, name: &str) -> HashSet<&String> {
        match self.net_translations.get(name) {
            Some(set) => set.iter().collect(),
            None => HashSet::new(),
        }
    }

    pub fn get_rev_ptrs(&self, name: &str) -> HashSet<&String> {
        match self.rev_ptrs.get(name) {
            Some(set) => set.iter().collect(),
            None => HashSet::new(),
        }
    }

    // SETTERS

    pub fn add_record(&mut self, record: DNSRecord) {
        if ADDRESS_RTYPES.contains(&record.rtype.to_uppercase().as_str()) {
            if !self.rev_ptrs.contains_key(&record.value) {
                self.rev_ptrs.insert(record.value.clone(), HashSet::new());
            }
            self.rev_ptrs
                .get_mut(&record.value)
                .unwrap()
                .insert(record.name.clone());
        }

        if !self.records.contains_key(&record.name) {
            self.records.insert(record.name.clone(), vec![]);
        }
        self.records.get_mut(&record.name).unwrap().push(record);
    }

    pub fn add_net_translation(&mut self, origin: &str, dest: String) {
        if !self.net_translations.contains_key(origin) {
            self.net_translations
                .insert(origin.to_owned(), HashSet::new());
        }
        self.net_translations.get_mut(origin).unwrap().insert(dest);
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct DNSRecord {
    pub name: String,
    pub value: String,
    pub rtype: String,
    pub plugin: String,
}

// NODES

#[derive(Debug, PartialEq, Eq)]
/// An unprocessed node.
pub struct RawNode {
    pub name: String,
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
    /// Contructs a raw node from the details stored under the provided key.
    pub fn from_key(con: &mut Connection, key: &str) -> NetdoxResult<Self> {
        let (generic_key, plugin) = match key.rsplit_once(';') {
            None => return redis_err!(format!("Invalid node redis key: {key}")),
            Some(val) => val,
        };
        let mut details: HashMap<String, String> = match con.hgetall(key) {
            Err(err) => return redis_err!(format!("Failed to get node details at {key}: {err}")),
            Ok(val) => val,
        };
        let name = match details.get("name") {
            Some(val) => val,
            None => return redis_err!(format!("Node details at key {key} missing name field.")),
        };
        let exclusive = match details.get("exclusive") {
            Some(val) => match val.as_str().parse::<bool>() {
                Ok(_val) => _val,
                Err(_) => {
                    return redis_err!(format!(
                        "Unable to parse boolean from exclusive value at {key}: {val}"
                    ))
                }
            },
            None => {
                return redis_err!(format!(
                    "Node details at key {key} missing exclusive field."
                ))
            }
        };

        Ok(RawNode {
            name: name.to_owned(),
            exclusive,
            link_id: details.remove("link_id"),
            dns_names: generic_key
                .split(';')
                .map(|v| v.to_owned())
                .skip(1)
                .collect(),
            plugin: plugin.to_owned(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
/// A processed, linkable node.
pub struct ResolvedNode {
    pub name: String,
    pub link_id: String,
    pub alt_names: HashSet<String>,
    pub dns_names: HashSet<String>,
    pub plugins: HashSet<String>,
}

impl Absorb for ResolvedNode {
    fn absorb(&mut self, other: Self) -> NetdoxResult<()> {
        self.alt_names.insert(other.name);
        self.alt_names.extend(other.alt_names);
        self.dns_names.extend(other.dns_names);
        self.plugins.extend(other.plugins);
        Ok(())
    }
}

impl ResolvedNode {
    /// Writes this node to a db.
    pub fn write(&self, con: &mut Connection) -> NetdoxResult<()> {
        let mut sorted_names: Vec<_> = self.dns_names.iter().map(|v| v.to_owned()).collect();
        sorted_names.sort();

        let key = format!("{NODES_KEY};{}", self.link_id);
        if let Err(err) = con.hset_multiple::<_, _, _, String>(
            &key,
            &[("name", &self.name), ("link_id", &self.link_id)],
        ) {
            return redis_err!(format!(
                "Failed while setting name or link_id for resolved node: {err}"
            ));
        }

        if !self.alt_names.is_empty() {
            if let Err(err) = con.sadd::<_, _, u8>(format!("{key};alt_names"), &self.alt_names) {
                return redis_err!(format!(
                    "Failed while updating alt names for resolved node: {err}"
                ));
            }
        }

        if !self.dns_names.is_empty() {
            if let Err(err) = con.sadd::<_, _, u8>(format!("{key};dns_names"), &self.dns_names) {
                return redis_err!(format!(
                    "Failed while updating dns names for resolved node: {err}"
                ));
            }
        }

        if !self.plugins.is_empty() {
            if let Err(err) = con.sadd::<_, _, u8>(format!("{key};plugins"), &self.plugins) {
                return redis_err!(format!(
                    "Failed while updating plugins for resolved node: {err}"
                ));
            }
        }
        // TODO add formal error handling for no dns names or plugins

        Ok(())
    }

    #[cfg(test)]
    /// Reads a node from a key in a db.
    pub fn read(key: &str, con: &mut Connection) -> NetdoxResult<Self> {
        let details: HashMap<String, String> = match con.hgetall(key) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed while reading details for linkable node at {key}: {err}"
                ))
            }
            Ok(val) => val,
        };

        let name = match details.get("name") {
            None => {
                return process_err!(format!(
                    "Linkable node with key {key} missing 'name' field."
                ))
            }
            Some(val) => val.to_owned(),
        };

        let link_id = match details.get("link_id") {
            None => {
                return process_err!(format!(
                    "Linkable node with key {key} missing 'link_id' field."
                ))
            }
            Some(val) => val.to_owned(),
        };

        let alt_names: HashSet<String> = con
            .smembers(format!("{key};alt_names"))
            .expect(&format!("Failed to get alt names for node at '{key}'."));
        let dns_names: HashSet<String> = con
            .smembers(format!("{key};dns_names"))
            .expect(&format!("Failed to get dns names for node at '{key}'."));
        let plugins: HashSet<String> = con
            .smembers(format!("{key};plugins"))
            .expect(&format!("Failed to get plugins for node at '{key}'."));

        Ok(Self {
            name,
            link_id,
            alt_names,
            dns_names,
            plugins,
        })
    }
}
