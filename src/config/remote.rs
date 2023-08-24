use std::collections::{HashMap, HashSet};

#[derive(PartialEq, Eq, Debug)]
pub struct RemoteConfig {
    /// A set of DNS names to exclude from all networks.
    pub exclude_dns: HashSet<String>,
    /// Maps network-qualified subnets to locations.
    pub locations: HashMap<String, String>, // TODO proper subnet type
    /// A map of plugin config values for each dns/node object.
    pub plugin_cfg: HashMap<String, HashMap<String, String>>,
}
