use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct RemoteConfigParams(BTreeMap<String, toml::Value>); // btreemap keeps order of keys

impl RemoteConfigParams {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        self.0.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, key: String, value: toml::Value) {
        self.0.insert(key, value);
    }
}

impl From<BTreeMap<String, toml::Value>> for RemoteConfigParams {
    fn from(map: BTreeMap<String, toml::Value>) -> Self {
        Self(map)
    }
}
