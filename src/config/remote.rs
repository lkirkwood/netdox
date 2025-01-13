use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
};

use ipnet::Ipv4Net;

use crate::{
    data::{
        model::{ObjectID, LOCATIONS_META_KEY, LOCATIONS_PLUGIN, NETDOX_PLUGIN},
        store::DataStore,
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
    /// Sets the location metadata key on all applicable objects in the datastore.
    pub async fn set_locations(&self, mut con: DataStore) -> NetdoxResult<()> {
        let dns = con.get_dns().await?;
        for name in &dns.qnames {
            if let Some((_, uq_name)) = name.rsplit_once(']') {
                if let Ok(ipv4) = uq_name.parse::<Ipv4Addr>() {
                    if let Some(subnet) = self.locate_ipv4(&ipv4) {
                        self.set_dns_location(&mut con, name, subnet).await?;
                    }
                } else {
                    let subnet = dns
                        .forward_march(name)
                        .iter()
                        .filter_map(|term| {
                            if let Some((_, uq_term)) = term.rsplit_once(']') {
                                if let Ok(ipv4) = uq_term.parse::<Ipv4Addr>() {
                                    return self.locate_ipv4(&ipv4);
                                }
                            }
                            None
                        })
                        .min_by(|subn_a, subn_b| subn_a.prefix_len().cmp(&subn_b.prefix_len()));

                    if let Some(subnet) = subnet {
                        self.set_dns_location(&mut con, name, subnet).await?;
                    }
                }
            }
        }

        Ok(())
    }

    fn locate_ipv4(&self, ipv4: &Ipv4Addr) -> Option<&Ipv4Net> {
        let mut best_subnet: Option<&Ipv4Net> = None;
        for subnet in self.locations.keys() {
            if subnet.contains(ipv4) {
                if let Some(_subnet) = best_subnet {
                    if subnet.prefix_len() < _subnet.prefix_len() {
                        best_subnet = Some(subnet);
                    }
                } else {
                    best_subnet = Some(subnet);
                }
            }
        }

        best_subnet
    }

    async fn set_dns_location(
        &self,
        con: &mut DataStore,
        name: &str,
        subnet: &Ipv4Net,
    ) -> NetdoxResult<()> {
        con.put_dns_metadata(
            name,
            LOCATIONS_PLUGIN,
            HashMap::from([(
                LOCATIONS_META_KEY,
                self.locations.get(&subnet).unwrap().as_ref(),
            )]),
        )
        .await?;

        if let Some(node_id) = con.get_dns_metadata(&name).await?.get("_node") {
            self.set_node_location(con, node_id, subnet).await?;
        }

        Ok(())
    }

    async fn set_node_location(
        &self,
        con: &mut DataStore,
        node_id: &str,
        subnet: &Ipv4Net,
    ) -> NetdoxResult<()> {
        let node = con.get_node(node_id).await?;
        con.put_node_metadata(
            &node,
            LOCATIONS_PLUGIN,
            HashMap::from([(
                LOCATIONS_META_KEY,
                self.locations.get(&subnet).unwrap().as_ref(),
            )]),
        )
        .await
    }

    /// Sets label-associated metadata to all applicable objects in the datastore.
    pub async fn set_metadata(&self, mut con: DataStore, remote: &Remote) -> NetdoxResult<()> {
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
