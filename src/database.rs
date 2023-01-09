use crate::environment::{
    get_s3_bucket, LOCAL_STORAGE_PATH, MONGODB_DATABASE, MONGODB_URI, USE_S3,
};
use crate::errors::{Error, Result};

use actix_web::web;
use mongodb::bson::doc;
use mongodb::{Client, Collection};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

static DATABASE: OnceCell<Client> = OnceCell::new();

pub async fn connect() {
    let client = Client::with_uri_str(&*MONGODB_URI)
        .await
        .expect("Failed to connect to MongoDB");
    DATABASE.set(client).expect("Failed to set MongoDB client");
}

pub fn get_collection(collection: &str) -> Collection<File> {
    DATABASE
        .get()
        .expect("Failed to get MongoDB client")
        .database(&MONGODB_DATABASE)
        .collection(collection)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileMetadata {
    File,
    Text,
    Image { width: isize, height: isize },
    Video { width: isize, height: isize },
    Audio,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: String,
    pub store: String,
    pub filename: String,
    pub metadata: FileMetadata,
    pub content_type: String,
    pub size: isize,
    pub attached: bool,
    pub deleted: bool,
    pub flagged: bool,
}

impl File {
    pub async fn delete_in_storage(&self) -> Result<()> {
        if *USE_S3 {
            let bucket = get_s3_bucket(&self.store)?;
            let response = bucket
                .delete_object(format!("/{}", &self.id))
                .await
                .map_err(|_| Error::ProcessingError)?;
            if response.status_code() != 200 {
                return Err(Error::ProcessingError);
            }
        } else {
            let path = format!("{}/{}", *LOCAL_STORAGE_PATH, &self.id);
            web::block(|| std::fs::remove_file(path))
                .await
                .map_err(|_| Error::ProcessingError)?
                .map_err(|_| Error::ProcessingError)?;
        }
        Ok(())
    }

    pub async fn delete(self) -> Result<()> {
        get_collection("files")
            .delete_one(doc! { "id": &self.id }, None)
            .await
            .map_err(|_| Error::DatabaseError)?;
        self.delete_in_storage().await?;
        Ok(())
    }

    pub async fn find(id: &str, store_id: &String) -> Result<File> {
        get_collection("files")
            .find_one(
                doc! {
                    "id": id,
                    "store": store_id,
                    "attached": true,
                    "deleted": false,
                },
                None,
            )
            .await
            .map_err(|_| Error::DatabaseError)?
            .ok_or(Error::NotFound)
    }
}
