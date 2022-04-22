use async_trait::async_trait;
use tribbler::{
    colon::escape,
    err::TribResult,
    storage::{KeyList, KeyString, KeyValue, List, Pattern, Storage},
};
pub struct BinUserClient {
    pub name: String,                  // store the name of the client
    pub bin_storage: Box<dyn Storage>, // store the storage
}

// We escape the name because BinStorage will be tested separately, and invalid keys that include ":" may be sent.
// Valid keys like "followees" and "tribs" would not be affected by the escape function.
#[async_trait]
impl KeyString for BinUserClient {
    async fn get(&self, key: &str) -> TribResult<Option<String>> {
        let prefix_key = (&self.name).to_string() + "::" + &escape(key);
        return self.bin_storage.get(&prefix_key).await;
    }

    async fn set(&self, kv: &KeyValue) -> TribResult<bool> {
        let prefix_key = (&self.name).to_string() + "::" + &escape(&kv.key);
        println!("{}", prefix_key);
        return self
            .bin_storage
            .set(&KeyValue {
                key: prefix_key,
                value: (&kv.value).to_string(),
            })
            .await;
    }

    async fn keys(&self, p: &Pattern) -> TribResult<List> {
        let prefix_prefix = (&self.name).to_string() + "::" + &p.prefix;

        let output_list = self
            .bin_storage
            .keys(&Pattern {
                prefix: prefix_prefix,
                suffix: (&p.suffix).to_string(),
            })
            .await;

        match output_list {
            Ok(output) => {
                let mut output_vec = Vec::<String>::new();
                for key in output.0 {
                    println!("{}", key);
                    let pass_length = self.name.len() + 2;
                    output_vec.push((&key[pass_length..]).to_string());
                }
                return Ok(List(output_vec));
            }
            _ => {
                return output_list;
            }
        }
    }
}

#[async_trait]
impl KeyList for BinUserClient {
    async fn list_get(&self, key: &str) -> TribResult<List> {
        let prefix_key = (&self.name).to_string() + "::" + &escape(key);
        return self.bin_storage.list_get(&prefix_key).await;
    }

    async fn list_append(&self, kv: &KeyValue) -> TribResult<bool> {
        let prefix_key = (&self.name).to_string() + "::" + &escape(&kv.key);
        return self
            .bin_storage
            .list_append(&KeyValue {
                key: prefix_key,
                value: (&kv.value).to_string(),
            })
            .await;
    }

    async fn list_remove(&self, kv: &KeyValue) -> TribResult<u32> {
        let prefix_key = (&self.name).to_string() + "::" + &escape(&kv.key);
        return self
            .bin_storage
            .list_remove(&KeyValue {
                key: prefix_key,
                value: (&kv.value).to_string(),
            })
            .await;
    }

    async fn list_keys(&self, p: &Pattern) -> TribResult<List> {
        let prefix_prefix = (&self.name).to_string() + "::" + &p.prefix;
        let output_list = self
            .bin_storage
            .list_keys(&Pattern {
                prefix: prefix_prefix,
                suffix: (&p.suffix).to_string(),
            })
            .await;

        match output_list {
            Ok(output) => {
                let mut output_vec = Vec::<String>::new();
                for key in output.0 {
                    let pass_length = self.name.len() + 2;
                    output_vec.push((&key[pass_length..]).to_string());
                }
                return Ok(List(output_vec));
            }
            _ => {
                return output_list;
            }
        }
    }
}

// CLOCK LOGIC
// if the parameter is bigger:
//     val = parameter
//     return val
// else:
//     val += 1
//     return val
#[async_trait]
impl Storage for BinUserClient {
    async fn clock(&self, at_least: u64) -> TribResult<u64> {
        return self.bin_storage.clock(at_least).await;
    }
}
