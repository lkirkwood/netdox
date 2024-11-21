use std::collections::HashMap;

use tokio::{process::Command, task::JoinSet};

use paris::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    config::{LocalConfig, PluginStage},
    error::{NetdoxError, NetdoxResult},
    plugin_err,
};

#[derive(Serialize, Deserialize, Debug)]
/// Contains information about a completed plugin or extension process.
pub struct PluginResult {
    pub stage: PluginStage,
    pub name: String,
    pub code: Option<i32>,
}

/// Runs one stage for all allowed plugins.
pub async fn run_plugin_stage(
    config: &LocalConfig,
    stage: PluginStage,
    allow_list: &Option<Vec<String>>,
) -> NetdoxResult<Vec<PluginResult>> {
    let datastore_cfg =
        toml::to_string(&config.redis).expect("Failed to serialise local config to TOML.");

    let mut cmds = HashMap::new();
    for plugin in &config.plugins {
        if cmds.contains_key(&plugin.name) {
            return plugin_err!(format!(
                "Plugin name {} appears multiple times.",
                plugin.name
            ));
        }

        if let Some(names) = &allow_list {
            if !names.contains(&plugin.name) {
                continue;
            }
        }

        if let Some(stage_config) = plugin.stages.get(&stage) {
            let mut cmd = Command::new(&stage_config.path);
            let plugin_cfg = plugin
                .fields
                .iter()
                .chain(&stage_config.fields)
                .collect::<HashMap<_, _>>();

            match toml::to_string(&plugin_cfg) {
                Ok(plugin_cfg_str) => {
                    cmd.arg(&datastore_cfg);
                    cmd.arg(plugin_cfg_str);
                }
                Err(err) => {
                    return plugin_err!(format!(
                        "Failed to serialize additional config fields for {}: {err}",
                        plugin.name
                    ))
                }
            }

            cmds.insert(plugin.name.clone(), cmd);
        }
    }

    if !cmds.is_empty() {
        info!(
            "Starting plugins for {stage} stage: {}",
            cmds.keys()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let mut procs = JoinSet::new();
    for (name, mut cmd) in cmds {
        match cmd.spawn() {
            Ok(proc) => {
                procs.spawn(async move { (name, proc.wait_with_output().await) });
            }
            Err(err) => {
                warn!("Killing all existing plugin processes due to error spawning new one...");
                procs.abort_all();
                return plugin_err!(format!("Failed to spawn process named {name}: {err}"));
            }
        }
    }

    let mut results = vec![];
    while let Some(join_result) = procs.join_next().await {
        match join_result {
            Ok((name, proc_result)) => match proc_result {
                Ok(output) => results.push(PluginResult {
                    stage,
                    name,
                    code: output.status.code(),
                }),
                Err(err) => {
                    return plugin_err!(format!("Error while retrieving plugin output: {err}"))
                }
            },
            Err(err) => {
                return plugin_err!(format!(
                    "Error while waiting for next plugin to complete: {err}"
                ))
            }
        }
    }

    Ok(results)
}
