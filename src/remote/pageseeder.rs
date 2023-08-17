use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct PSRemote {
    url: String,
    client_id: String,
    client_secret: String,
    username: String,
    group: String,
}

impl crate::remote::RemoteInterface for PSRemote {
    fn test(&self) -> crate::error::NetdoxResult<()> {
        Ok(())
    }
}
