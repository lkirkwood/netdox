use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
};

use ipnet::Ipv4Net;
use itertools::{Either, Itertools};
use paris::warn;

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
    ///
    /// This method will:
    /// 1. Loop DNS names
    ///     a. Set location for IPv4s by subnet
    ///     b. Set location for DNS names by forward march
    /// 2. Loop DNS names and set location from the node
    /// 3. Repeated steps 1 and 2 until no new locations are set
    pub async fn set_locations(&self, mut con: DataStore) -> NetdoxResult<()> {
        let dns = con.get_dns().await?;

        // Maps unqualified DNS names to their locations.
        let mut locations = HashMap::new();
        let mut num_located: isize = -1;
        while num_located < 0 || locations.len() as isize > num_located {
            num_located = locations.len() as isize;

            for name in &dns.qnames {
                if locations.contains_key(name) {
                    continue;
                }

                if let Some((_, uq_name)) = name.rsplit_once(']') {
                    // Set IPv4 location by subnet.
                    if let Ok(ipv4) = uq_name.parse::<Ipv4Addr>() {
                        if let Some(subnet) = self.choose_subnet(&ipv4) {
                            let location = self.set_dns_subnet(&mut con, name, subnet).await?;
                            locations.insert(name.to_string(), location.to_string());
                        }
                    // Set domain location by forward march.
                    // The IPv4 terminal with the smallest subnet will be used.
                    // In the event there are no IPv4 terminals, the location of the
                    } else {
                        let terminals = dns.forward_march(name).into_iter();
                        let (term_ips, term_uqnames): (Vec<_>, Vec<_>) = terminals
                            .filter(|term| term.contains(']'))
                            .partition_map(|term| {
                                match term.rsplit_once(']').unwrap().1.parse::<Ipv4Addr>() {
                                    Ok(ipv4) => Either::Left(self.choose_subnet(&ipv4)),
                                    Err(_) => Either::Right(term),
                                }
                            });

                        let subnet = term_ips
                            .into_iter()
                            .flatten()
                            .min_by(|subn_a, subn_b| subn_a.prefix_len().cmp(&subn_b.prefix_len()));

                        if let Some(subnet) = subnet {
                            let location = self.set_dns_subnet(&mut con, name, subnet).await?;
                            locations.insert(name.to_string(), location.to_string());
                            continue;
                        }

                        let domain_locations = term_uqnames
                            .into_iter()
                            .filter_map(|uq_term| locations.get(uq_term))
                            .collect::<HashSet<_>>();

                        match domain_locations.len().cmp(&1) {
                            Ordering::Equal => {
                                let location = domain_locations.iter().next().unwrap();
                                self.set_dns_location(&mut con, name, location).await?;
                                locations.insert(name.to_string(), location.to_string());
                            }
                            Ordering::Greater => {
                                warn!("Multiple locations for {name} from domain terminals.");
                                locations.insert(name.to_string(), "AMBIGUOUS".to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }

            for name in &dns.qnames {
                if locations.contains_key(name) {
                    continue;
                }

                if let Some(node_id) = con.get_dns_metadata(name).await?.get("_node") {
                    let node = &con.get_node(node_id).await?;
                    let node_meta = con.get_node_metadata(node).await?;
                    if let Some(location) = node_meta.get(LOCATIONS_META_KEY) {
                        self.set_dns_location(&mut con, name, location).await?;
                        locations.insert(
                            name.rsplit_once(']').unwrap().1.to_string(),
                            location.to_string(),
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Chooses the most specific location subnet that contains the given IPv4 address.
    fn choose_subnet(&self, ipv4: &Ipv4Addr) -> Option<&Ipv4Net> {
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

    /// Sets the location metadata attribute for the DNS name from the subnet,
    /// and its associated node if there is one.
    /// Returns the location string.
    async fn set_dns_subnet(
        &self,
        con: &mut DataStore,
        name: &str,
        subnet: &Ipv4Net,
    ) -> NetdoxResult<&str> {
        let location = self.locations.get(subnet).unwrap().as_ref();
        self.set_dns_location(con, name, location).await?;
        Ok(location)
    }

    /// Sets the location metadata attribute for the DNS name,
    /// and its associated node if there is one.
    async fn set_dns_location(
        &self,
        con: &mut DataStore,
        name: &str,
        location: &str,
    ) -> NetdoxResult<()> {
        con.put_dns_metadata(
            name,
            LOCATIONS_PLUGIN,
            HashMap::from([(LOCATIONS_META_KEY, location)]),
        )
        .await?;

        if let Some(node_id) = con.get_dns_metadata(name).await?.get("_node") {
            self.set_node_location(con, node_id, location).await?;
        }

        Ok(())
    }

    async fn set_node_location(
        &self,
        con: &mut DataStore,
        node_id: &str,
        location: &str,
    ) -> NetdoxResult<()> {
        let node = con.get_node(node_id).await?;
        con.put_node_metadata(
            &node,
            LOCATIONS_PLUGIN,
            HashMap::from([(LOCATIONS_META_KEY, location)]),
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
