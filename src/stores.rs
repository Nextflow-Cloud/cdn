use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use crate::environment::STORES;
use crate::errors::{Error, Result};

#[derive(Debug, Deserialize, Serialize)]
pub enum ContentType {
    Image,
    Video,
    Audio,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Store {
    pub max_size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restrict_content_type: Option<ContentType>,
}

impl Store {
    pub fn get(id: &String) -> Result<&'static Store> {
        let stores = get_stores();
        if let Some(store) = stores.get(id) {
            Ok(store)
        } else {
            Err(Error::UnknownStore)
        }
    }
}

static STORE_MAP: OnceCell<HashMap<String, Store>> = OnceCell::new();

pub fn get_stores() -> &'static HashMap<String, Store> {
    STORE_MAP.get().expect("Failed to get global stores")
}

pub fn load_stores() -> std::io::Result<()> {
    let mut file = File::open(&*STORES)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let stores: HashMap<String, Store> = toml::from_str(&contents).expect("Failed to parse stores");
    STORE_MAP.set(stores).expect("Failed to set global stores");
    Ok(())
}
