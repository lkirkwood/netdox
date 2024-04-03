use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    net::Ipv4Addr,
};

use ipnet::Ipv4Net;

use crate::{
    data::{
        model::{LOCATIONS_META_KEY, LOCATIONS_PLUGIN},
        DataConn,
    },
    error::NetdoxResult,
};

#[derive(PartialEq, Eq, Debug)]
pub struct RemoteConfig {
    /// A set of DNS names to exclude from all networks.
    pub exclude_dns: HashSet<String>,
    /// Maps network-qualified subnets to locations.
    pub locations: HashMap<Ipv4Net, String>,
    /// A map of plugin config values for each dns/node object.
    pub plugin_cfg: HashMap<String, HashMap<String, String>>,
}

impl RemoteConfig {
    pub async fn set_locations(&self, mut con: Box<dyn DataConn>) -> NetdoxResult<()> {
        let mut matches = HashMap::new();
        for name in con.get_dns_names().await? {
            if let Some((_, uq_name)) = name.rsplit_once("]") {
                if let Ok(ipv4) = uq_name.parse::<Ipv4Addr>() {
                    for subnet in self.locations.keys() {
                        if subnet.contains(&ipv4) {
                            match matches.entry(name.clone()) {
                                Entry::Vacant(entry) => {
                                    entry.insert(subnet.clone());
                                }
                                Entry::Occupied(mut entry) => {
                                    if subnet.prefix_len() < entry.get().prefix_len() {
                                        entry.insert(subnet.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for (ipv4, subnet) in matches {
            con.put_dns_metadata(
                &ipv4,
                LOCATIONS_PLUGIN,
                HashMap::from([(
                    LOCATIONS_META_KEY,
                    self.locations.get(&subnet).unwrap().as_ref(),
                )]),
            )
            .await?;
        }

        Ok(())
    }
}
