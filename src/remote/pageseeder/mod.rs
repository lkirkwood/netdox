mod config;
mod remote;
#[cfg(test)]
mod tests;

use crate::error::NetdoxError;
use pageseeder::error::PSError;
pub use remote::PSRemote;

const REMOTE_CONFIG_PATH: &str = "website/config";

impl From<PSError> for NetdoxError {
    fn from(value: PSError) -> Self {
        Self::Remote(value.to_string())
    }
}
