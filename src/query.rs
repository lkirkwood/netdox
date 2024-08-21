use std::process::exit;

use paris::error;

use crate::{config::LocalConfig, data::DataConn, QueryCommand};

/// Performs the given query command.
#[tokio::main]
pub async fn query(cmd: QueryCommand) {
    match cmd {
        QueryCommand::Counts => counts().await,
    }
}

async fn counts() {
    let cfg = match LocalConfig::read() {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("Failed to get local config in order to print counts: {err}");
            exit(1);
        }
    };

    let mut con = match cfg.con().await {
        Ok(con) => con,
        Err(err) => {
            error!("Failed to get data store connection in order to print counts: {err}");
            exit(1);
        }
    };

    match con.get_node_ids().await {
        Ok(ids) => println!("Number of nodes: {}", ids.len()),
        Err(err) => {
            error!("Failed to get number of nodes for counts: {err}");
            exit(1);
        }
    }

    match con.get_raw_nodes().await {
        Ok(raw_nodes) => println!("Number of raw nodes: {}", raw_nodes.len()),
        Err(err) => {
            error!("Failed to get number of raw nodes for counts: {err}");
            exit(1);
        }
    }

    match con.get_dns_names().await {
        Ok(names) => println!("Number of DNS names: {}", names.len()),
        Err(err) => {
            error!("Failed to get number of DNS names for counts: {err}");
            exit(1);
        }
    }
}
