use std::{collections::HashMap, process::Command};

use crate::{
    config::PluginConfig,
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
    let mut cmds = HashMap::new();
    for plugin in plugins {
        if cmds.contains_key(&plugin.name) {
            return plugin_err!(format!(
                "Plugin name {} appears multiple times.",
                plugin.name
            ));
        }

        let mut cmd = Command::new(&plugin.path);

        let field_str = toml::to_string(&plugin.fields);
        if let Err(err) = field_str {
            return plugin_err!(format!(
                "Failed to serialize additional config fields for {}: {err}",
                plugin.name
            ));
        }

        cmd.arg(field_str.unwrap());
        cmds.insert(plugin.name.clone(), cmd);
    }

    let mut procs = HashMap::new();
    for (name, mut cmd) in cmds {
        match cmd.spawn() {
            Ok(proc) => {
                procs.insert(name, proc);
            }
            Err(err) => {
                println!("Killing all existing subprocesses due to error spawning new one...");
                for mut proc in procs {
                    if let Err(err) = proc.1.kill() {
                        println!("Failed to kill process for plugin {}: {err}", proc.0);
                    }
                }
                return plugin_err!(format!("Failed to spawn process for plugin {name}: {err}"));
            }
        }
    }

    let mut results = vec![];
    for (name, proc) in procs {
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
