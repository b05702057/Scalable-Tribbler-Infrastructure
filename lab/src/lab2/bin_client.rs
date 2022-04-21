use super::bin_user_client::BinUserClient;
use crate::lab1::lab::new_client;
use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use tribbler::{
    self,
    colon::escape,
    err::TribResult,
    storage::{BinStorage, Storage}, // to implement the RPCs
};

// declare a new struct and add fileds to it (addr)
pub struct BinStorageClient {
    pub backs: Vec<String>, // store the storage clients
}

// We escape the name because BinStorage will be tested separately, and invalid usernames that include ":" may be sent.
// Valid usernames like Zack would not be affected by the escape function.
#[async_trait]
impl BinStorage for BinStorageClient {
    async fn bin(&self, name: &str) -> TribResult<Box<dyn Storage>> {
        // get the hash value
        let mut hasher = DefaultHasher::new();
        hasher.write(name.as_bytes());
        let hash_value = hasher.finish() as usize;

        // make the hash value in the range
        let backend_num = self.backs.len();
        let backend_id = hash_value % backend_num;
        let addr = &self.backs[backend_id];
        let storage = new_client(addr).await?;

        // wrap the storage client as a bin storage client
        let user_storage = BinUserClient {
            name: escape(name),
            bin_storage: storage,
        };
        return Ok(Box::new(user_storage));
    }
}
