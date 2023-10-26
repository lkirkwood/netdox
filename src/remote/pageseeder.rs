mod config;
mod psml;
mod publish;
mod remote;
#[cfg(test)]
mod tests;

use crate::error::NetdoxError;
use pageseeder::error::PSError;
pub use remote::PSRemote;

impl From<PSError> for NetdoxError {
    fn from(value: PSError) -> Self {
        Self::Remote(value.to_string())
    }
}
