pub mod model;
mod store;
#[cfg(test)]
mod tests;

pub use store::{Datastore, DatastoreClient};
