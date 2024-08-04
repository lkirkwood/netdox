use std::{error::Error, fmt::Display, io};

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
    /// Error with remote server.
    Remote(String),
    /// Error during IO.
    IO(String),
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

#[macro_export]
macro_rules! remote_err {
    ($err:expr) => {
        Err(NetdoxError::Remote($err))
    };
}

#[macro_export]
macro_rules! io_err {
    ($err:expr) => {
        Err(NetdoxError::IO($err))
    };
}

impl Display for NetdoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Error with netdox config: {msg}"),
            Self::Plugin(msg) => write!(f, "Error with a plugin: {msg}"),
            Self::Redis(msg) => write!(f, "Error with the redis database: {msg}"),
            Self::Process(msg) => write!(f, "Error during node processing: {msg}"),
            Self::Remote(msg) => write!(f, "Error while communicating with remote: {msg}"),
            Self::IO(msg) => write!(f, "Error during IO: {msg}"),
        }
    }
}

impl Error for NetdoxError {}

pub type NetdoxResult<T> = Result<T, NetdoxError>;

// Coercions

impl From<io::Error> for NetdoxError {
    fn from(value: io::Error) -> Self {
        NetdoxError::IO(value.to_string())
    }
}

impl From<redis::RedisError> for NetdoxError {
    fn from(value: redis::RedisError) -> Self {
        NetdoxError::Redis(value.to_string())
    }
}
