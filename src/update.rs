use std::collections::HashMap;

use tokio::{process::Command, task::JoinSet};

use paris::{info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    config::{LocalConfig, PluginStage},
    data::{
        model::{Data, StringType, NETDOX_PLUGIN},
        DataConn,
    },
    error::{NetdoxError, NetdoxResult},
    plugin_err,
};

#[derive(Serialize, Deserialize, Debug)]
/// Contains information about a completed plugin or extension process.
pub struct PluginResult {
    pub stage: PluginStage,
    pub name: String,
    pub code: Option<i32>,
    pub stderr: String,
}

/// Runs one stage for all allowed plugins.
pub async fn run_plugin_stage(
    config: &LocalConfig,
    stage: PluginStage,
    plugin_list: &Option<Vec<String>>,
    exclude: bool,
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

        if let Some(names) = &plugin_list {
            if !(exclude ^ names.contains(&plugin.name)) {
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
    } else {
        info!("No plugins to run for {stage} stage.")
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
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
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

/// Creates a report from the plugin results in the list.
pub async fn plugin_error_report(
    con: &mut impl DataConn,
    mut results: Vec<PluginResult>,
) -> NetdoxResult<()> {
    let id = "plugin-errors";

    if results.iter().all(|result| result.code == Some(0)) {
        con.put_report(id, "Plugin Errors", 1).await?;
        let data = Data::String {
            id: "plugin-errors-none".to_string(),
            title: "No Plugin Errors!".to_string(),
            content_type: StringType::Plain,
            plugin: NETDOX_PLUGIN.to_string(),
            content: "No plugins encountered an error during the last update.".to_string(),
        };
        con.put_report_data(id, 0, &data).await?;
        return Ok(());
    }

    results.retain(|result| result.code != Some(0));

    con.put_report(id, "Plugin Errors", results.len()).await?;
    for (idx, error) in results.into_iter().enumerate() {
        let data = Data::String {
            id: format!("{}-{}-error", error.name, error.stage),
            title: format!("{} Error during stage: {}", error.name, error.stage),
            content_type: StringType::Plain,
            plugin: NETDOX_PLUGIN.to_string(),
            content: error.stderr,
        };
        con.put_report_data(id, idx, &data).await?;
    }

    Ok(())
}
