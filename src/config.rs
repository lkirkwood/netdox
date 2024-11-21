pub mod local;
pub mod remote;

pub use local::{IgnoreList, LocalConfig, PluginConfig, PluginStage, PluginStageConfig};
pub use remote::RemoteConfig;
