pub mod model;

use std::collections::HashSet;

use redis::Commands;

use model::{Absorb, DNSRecord, DNS, DNS_KEY};

use crate::{
    error::{NetdoxError, NetdoxResult},
    redis_err,
};

use self::model::{RawNode, NODES_KEY};

pub trait Datastore {
    /// Gets the DNS data from redis.
    fn get_dns(&mut self) -> NetdoxResult<DNS>;

    fn get_dns_names(&mut self) -> NetdoxResult<HashSet<String>>;

    /// Fetches a DNS struct with only data for the given DNS name.
    fn get_dns_name(&mut self, name: &str) -> NetdoxResult<DNS>;

    /// Fetches a DNS struct with only data for the given DNS name from the given source plugin.
    fn get_plugin_dns_name(&mut self, name: &str, plugin: &str) -> NetdoxResult<DNS>;

    /// Fetches raw nodes from a connection.
    fn get_raw_nodes(&mut self) -> NetdoxResult<Vec<RawNode>>;
}

impl Datastore for redis::Connection {
    fn get_dns(&mut self) -> NetdoxResult<DNS> {
        let mut dns = DNS::new();
        for name in self.get_dns_names()? {
            dns.absorb(self.get_dns_name(&name)?)?;
        }

        Ok(dns)
    }

    fn get_dns_names(&mut self) -> NetdoxResult<HashSet<String>> {
        match self.smembers(DNS_KEY) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get set of dns names using key {DNS_KEY}: {err}"
                ))
            }
            Ok(dns) => Ok(dns),
        }
    }

    fn get_dns_name(&mut self, name: &str) -> NetdoxResult<DNS> {
        let plugins: HashSet<String> = match self.smembers(format!("{DNS_KEY};{name};plugins")) {
            Err(err) => {
                return redis_err!(format!("Failed to get plugins for dns name {name}: {err}"))
            }
            Ok(_p) => _p,
        };

        let mut dns = DNS::new();
        for plugin in plugins {
            dns.absorb(self.get_plugin_dns_name(name, &plugin)?)?;
        }

        let translations: HashSet<String> = match self.smembers(format!("{DNS_KEY};{name};maps")) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get network translations for dns name {name}: {err}"
                ))
            }
            Ok(_t) => _t,
        };

        for tran in translations {
            dns.add_net_translation(name, tran);
        }

        Ok(dns)
    }

    fn get_plugin_dns_name(&mut self, name: &str, plugin: &str) -> NetdoxResult<DNS> {
        let mut dns = DNS::new();
        let rtypes: HashSet<String> = match self.smembers(format!("{DNS_KEY};{name};{plugin}")) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get record types from plugin {plugin} for dns name {name}: {err}"
                ))
            }
            Ok(_t) => _t,
        };

        for rtype in rtypes {
            let values: HashSet<String> = match self.smembers(format!("{DNS_KEY};{name};{plugin};{rtype}")) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get {rtype} record values from plugin {plugin} for dns name {name}: {err}"
                ))
            },
            Ok(_v) => _v
        };
            for value in values {
                dns.add_record(DNSRecord {
                    name: name.to_owned(),
                    value,
                    rtype: rtype.to_owned(),
                    plugin: plugin.to_owned(),
                })
            }
        }

        Ok(dns)
    }

    fn get_raw_nodes(&mut self) -> NetdoxResult<Vec<RawNode>> {
        let nodes: HashSet<String> = match self.smembers(NODES_KEY) {
            Err(err) => {
                return redis_err!(format!(
                    "Failed to get set of nodes using key {NODES_KEY}: {err}"
                ))
            }
            Ok(val) => val,
        };

        let mut raw = vec![];
        for node in nodes {
            let redis_key = format!("{NODES_KEY};{node}");
            let count: u64 = match self.get(&redis_key) {
                Err(err) => {
                    return redis_err!(format!(
                        "Failed to get number of nodes with key {redis_key}: {err}"
                    ))
                }
                Ok(val) => val,
            };

            for index in 1..=count {
                raw.push(RawNode::from_key(self, &format!("{redis_key};{index}"))?)
            }
        }

        Ok(raw)
    }
}
