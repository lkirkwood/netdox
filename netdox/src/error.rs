use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum NetdoxError {
    RemoteError(String),
    ConfigError(String),
}

#[macro_export]
macro_rules! remote_err {
    ($err:expr) => {
        Err(NetdoxError::RemoteError($err))
    };
}

#[macro_export]
macro_rules! config_err {
    ($err:expr) => {
        Err(NetdoxError::ConfigError($err))
    };
}

impl Display for NetdoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RemoteError(msg) => write!(f, "Error from remote: {msg}"),
            Self::ConfigError(msg) => write!(f, "Error with config: {msg}"),
        }
    }
}

impl Error for NetdoxError {}

pub type NetdoxResult<T> = Result<T, NetdoxError>;
