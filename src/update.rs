use std::{
    collections::HashMap,
    fmt::Display,
    process::{Child, Command},
};

use paris::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    config::{LocalConfig, SubprocessConfig},
    error::{NetdoxError, NetdoxResult},
    plugin_err,
};

#[derive(Serialize, Deserialize, Debug)]
pub enum SubprocessKind {
    Plugin,
    Extension,
}

impl Display for SubprocessKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plugin => write!(f, "Plugin"),
            Self::Extension => write!(f, "Extension"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// Contains information about a completed plugin or extension process.
pub struct SubprocessResult {
    pub kind: SubprocessKind,
    pub name: String,
    pub code: Option<i32>,
}

/// Runs all plugins and returns their result.
pub async fn run_plugins(config: &LocalConfig) -> NetdoxResult<Vec<SubprocessResult>> {
    let mut results = vec![];

    for (name, proc) in run_subprocesses(config, &config.plugins)? {
        let output = match proc.wait_with_output() {
            Err(err) => {
                return plugin_err!(format!(
                    "Failed to retrieve output from plugin {name}: {err}"
                ))
            }
            Ok(out) => out,
        };
        results.push(SubprocessResult {
            kind: SubprocessKind::Plugin,
            name,
            code: output.status.code(),
        })
    }

    Ok(results)
}

pub async fn run_extensions(config: &LocalConfig) -> NetdoxResult<Vec<SubprocessResult>> {
    let mut results = vec![];

    for (name, proc) in run_subprocesses(config, &config.extensions)? {
        let output = match proc.wait_with_output() {
            Err(err) => {
                return plugin_err!(format!(
                    "Failed to retrieve output from extension {name}: {err}"
                ))
            }
            Ok(out) => out,
        };
        results.push(SubprocessResult {
            kind: SubprocessKind::Extension,
            name,
            code: output.status.code(),
        })
    }

    Ok(results)
}

fn run_subprocesses(
    config: &LocalConfig,
    subps: &[SubprocessConfig],
) -> NetdoxResult<HashMap<String, Child>> {
    let mut cmds = HashMap::new();
    for subp in subps {
        if cmds.contains_key(&subp.name) {
            return plugin_err!(format!(
                "Plugin or extension name {} appears multiple times.",
                subp.name
            ));
        }

        let mut cmd = Command::new(&subp.path);

        match toml::to_string(&subp.fields) {
            Ok(field) => {
                cmd.arg(&config.redis.url);
                cmd.arg(field);
            }
            Err(err) => {
                return plugin_err!(format!(
                    "Failed to serialize additional config fields for {}: {err}",
                    subp.name
                ))
            }
        }

        cmds.insert(subp.name.clone(), cmd);
    }

    if !cmds.is_empty() {
        info!(
            "Starting subprocess(es): {}",
            cmds.keys()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let mut procs = HashMap::new();
    for (name, mut cmd) in cmds {
        match cmd.spawn() {
            Ok(proc) => {
                procs.insert(name, proc);
            }
            Err(err) => {
                warn!("Killing all existing subprocesses due to error spawning new one...");
                for mut proc in procs {
                    if let Err(err) = proc.1.kill() {
                        error!("Failed to kill process named {}: {err}", proc.0);
                    }
                }
                return plugin_err!(format!("Failed to spawn process named {name}: {err}"));
            }
        }
    }

    Ok(procs)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::config::LocalConfig;

    use super::{run_extensions, run_plugins};

    #[tokio::test]
    async fn test_plugins() {
        let config: LocalConfig =
            toml::from_str(&fs::read_to_string("test/test.toml").unwrap()).unwrap();

        run_plugins(&config).await.unwrap();
    }

    #[tokio::test]
    async fn test_extensions() {
        let config: LocalConfig =
            toml::from_str(&fs::read_to_string("test/test.toml").unwrap()).unwrap();

        run_extensions(&config).await.unwrap();
    }
}
