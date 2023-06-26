use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum NetdoxError {
    Config(String),
    Plugin(String),
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

impl Display for NetdoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Error with config: {msg}"),
            Self::Plugin(msg) => write!(f, "Error with plugin: {msg}"),
        }
    }
}

impl Error for NetdoxError {}

pub type NetdoxResult<T> = Result<T, NetdoxError>;
