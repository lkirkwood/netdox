mod config;
mod psml;
mod publish;
mod remote;

use crate::error::NetdoxError;
use pageseeder_api::model::PSError;
pub use remote::PSRemote;

impl From<PSError> for NetdoxError {
    fn from(value: PSError) -> Self {
        Self::Remote(value.to_string())
    }
}
