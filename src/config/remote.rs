use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    net::Ipv4Addr,
};

use ipnet::Ipv4Net;

use crate::{
    data::{
        model::{ObjectID, LOCATIONS_META_KEY, LOCATIONS_PLUGIN, NETDOX_PLUGIN},
        DataConn,
    },
    error::NetdoxResult,
    remote::{Remote, RemoteInterface},
};

#[derive(PartialEq, Eq, Debug)]
pub struct RemoteConfig {
    /// A set of DNS names to exclude from all networks.
    pub exclusions: HashSet<String>,
    /// Maps unqualified subnets to locations.
    pub locations: HashMap<Ipv4Net, String>,
    /// Maps a document label to a set of metadata key/value overrides.
    pub metadata: HashMap<String, HashMap<String, String>>,
}

impl RemoteConfig {
    pub async fn set_locations(&self, mut con: Box<dyn DataConn>) -> NetdoxResult<()> {
        let mut matches = HashMap::new();
        for name in con.get_dns_names().await? {
            if let Some((_, uq_name)) = name.rsplit_once(']') {
                if let Ok(ipv4) = uq_name.parse::<Ipv4Addr>() {
                    for subnet in self.locations.keys() {
                        if subnet.contains(&ipv4) {
                            match matches.entry(name.clone()) {
                                Entry::Vacant(entry) => {
                                    entry.insert(*subnet);
                                }
                                Entry::Occupied(mut entry) => {
                                    if subnet.prefix_len() < entry.get().prefix_len() {
                                        entry.insert(*subnet);
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

    pub async fn set_metadata(
        &self,
        mut con: Box<dyn DataConn>,
        remote: &Remote,
    ) -> NetdoxResult<()> {
        for (label, meta) in &self.metadata {
            for obj_id in remote.labeled(label).await? {
                match obj_id {
                    ObjectID::DNS(id) => {
                        con.put_dns_metadata(
                            &id,
                            NETDOX_PLUGIN,
                            meta.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect(),
                        )
                        .await?
                    }
                    ObjectID::Node(id) => {
                        let node = con.get_node(&id).await?;
                        con.put_node_metadata(
                            &node,
                            NETDOX_PLUGIN,
                            meta.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect(),
                        )
                        .await?
                    }
                    ObjectID::Report(_id) => {
                        // pass
                    }
                }
            }
        }
        Ok(())
    }
}
