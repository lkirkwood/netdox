use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum NetdoxError {
    /// Error with netdox config.
    Config(String),
    /// Error with a plugin.
    Plugin(String),
    /// Error with the redis database.
    Redis(String),
    /// Error with the processing logic.
    Process(String),
}

#[macro_export]
macro_rules! config_err {
    ($err:expr) => {
        Err(NetdoxError::Config($err))
    };
}

#[macro_export]
macro_rules! plugin_err {
    ($err:expr) => {
        Err(NetdoxError::Plugin($err))
    };
}

#[macro_export]
macro_rules! redis_err {
    ($err:expr) => {
        Err(NetdoxError::Redis($err))
    };
}

#[macro_export]
macro_rules! process_err {
    ($err:expr) => {
        Err(NetdoxError::Process($err))
    };
}

impl Display for NetdoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Error with netdox config: {msg}"),
            Self::Plugin(msg) => write!(f, "Error with a plugin: {msg}"),
            Self::Redis(msg) => write!(f, "Error with the redis database: {msg}"),
            Self::Process(msg) => write!(f, "Error during node processing: {msg}"),
        }
    }
}

impl Error for NetdoxError {}

pub type NetdoxResult<T> = Result<T, NetdoxError>;
