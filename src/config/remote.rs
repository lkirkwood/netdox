use std::{
    collections::{HashMap, HashSet},
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
    pub locations: HashMap<Ipv4Net, String>, // TODO proper subnet type
    /// A map of plugin config values for each dns/node object.
    pub plugin_cfg: HashMap<String, HashMap<String, String>>,
}

impl RemoteConfig {
    pub async fn set_locations(&self, con: &mut dyn DataConn) -> NetdoxResult<()> {
        for name in con.get_dns_names().await? {
            if let Ok(ipv4) = name.parse::<Ipv4Addr>() {
                for (subnet, location) in &self.locations {
                    if subnet.contains(&ipv4) {
                        con.put_dns_metadata(
                            &name,
                            LOCATIONS_PLUGIN,
                            HashMap::from([(LOCATIONS_META_KEY, location.as_ref())]),
                        )
                        .await?;
                    }
                }
            }
        }
        Ok(())
    }
}
