use std::{collections::HashMap, process::Command};

use crate::{
    config::PluginConfig,
    config_err,
    error::{NetdoxError, NetdoxResult},
    plugin_err,
};

/// Contains information about a completed plugin process.
pub struct PluginResult {
    pub name: String,
    pub code: Option<i32>,
}

/// Runs all plugins and returns their result.
/// TODO add passing plugin config to plugin binary.
pub fn update(plugins: Vec<PluginConfig>) -> NetdoxResult<Vec<PluginResult>> {
    let mut children = HashMap::new();
    let mut config = HashMap::new();
    for plugin in plugins {
        if children.contains_key(&plugin.name) {
            return config_err!(format!(
                "Plugin name {} appears multiple times.",
                plugin.name
            ));
        }
        let proc = match Command::new(&plugin.path).spawn() {
            Err(err) => {
                return plugin_err!(format!(
                    "Failed to spawn subprocess for plugin {}: {err}",
                    plugin.name
                ))
            }
            Ok(_p) => _p,
        };
        children.insert(plugin.name.clone(), proc);
        config.insert(plugin.name.clone(), plugin);
    }

    let mut results = vec![];
    for (name, proc) in children {
        let output = match proc.wait_with_output() {
            Err(err) => {
                return plugin_err!(format!(
                    "Failed to retrieve output from plugin {name}: {err}"
                ))
            }
            Ok(out) => out,
        };
        results.push(PluginResult {
            name,
            code: output.status.code(),
        })
    }

    Ok(results)
}
