use std::{cmp::min, path::PathBuf, sync::Arc};

use futures::AsyncReadExt;
use mongodb::{bson::doc, Collection};
use serde::{Deserialize, Serialize};

use crate::{
    database::DATABASE,
    environment::{get_s3_bucket, LOCAL_STORAGE_PATH, MONGODB_DATABASE, USE_S3},
    errors::{Error, Result},
    routes::serve::Resize,
    utilities::try_resize,
};

pub fn get_collection() -> Collection<File> {
    DATABASE
        .get()
        .expect("Failed to get MongoDB client")
        .database(&MONGODB_DATABASE)
        .collection("files")
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
            async_std::fs::remove_file(path)
                .await
                .map_err(|_| Error::ProcessingError)?
        }
        Ok(())
    }

    pub async fn delete(self) -> Result<()> {
        get_collection()
            .delete_one(doc! { "id": &self.id })
            .await
            .map_err(|_| Error::DatabaseError)?;
        self.delete_in_storage().await?;
        Ok(())
    }

    pub async fn find(id: &str, store_id: &String) -> Result<File> {
        get_collection()
            .find_one(
                doc! {
                    "id": id,
                    "store": store_id,
                    "attached": true,
                    "deleted": false,
                },
            )
            .await
            .map_err(|_| Error::DatabaseError)?
            .ok_or(Error::NotFound)
    }

    pub async fn fetch(&self, resize: Option<Resize>) -> Result<(Vec<u8>, Option<String>)> {
        let mut contents = Vec::new();
        if *USE_S3 {
            let bucket = get_s3_bucket(&self.store)?;
            let response = bucket
                .get_object(format!("/{}", self.id))
                .await
                .map_err(|_| Error::StorageError)?;
            if response.status_code() != 200 {
                return Err(Error::StorageError);
            }
            contents = response.bytes().to_vec();
        } else {
            let path: PathBuf = format!("{}/{}", *LOCAL_STORAGE_PATH, self.id)
                .parse()
                .map_err(|_| Error::StorageError)?;

            let mut f = async_std::fs::File::open(path)
                .await
                .map_err(|_| Error::StorageError)?;
            f.read_to_end(&mut contents)
                .await
                .map_err(|_| Error::StorageError)?;
        }
        let contents_arc = Arc::new(contents);
        if let Some(parameters) = resize {
            if let FileMetadata::Image { width, height } = self.metadata {
                let shortest_length = min(width, height);
                let (target_width, target_height) = match (
                    parameters.size,
                    parameters.max_side,
                    parameters.width,
                    parameters.height,
                ) {
                    (Some(size), _, _, _) => {
                        let smallest_size = min(size, shortest_length);
                        (smallest_size, smallest_size)
                    }
                    (_, Some(size), _, _) => {
                        if shortest_length == width {
                            let h = min(height, size);
                            ((width as f32 * (h as f32 / height as f32)) as isize, h)
                        } else {
                            let w = min(width, size);
                            (w, (height as f32 * (w as f32 / width as f32)) as isize)
                        }
                    }
                    (_, _, Some(w), Some(h)) => (min(width, w), min(height, h)),
                    (_, _, Some(w), _) => {
                        let w = min(width, w);
                        (w, (w as f32 * (height as f32 / width as f32)) as isize)
                    }
                    (_, _, _, Some(h)) => {
                        let h = min(height, h);
                        ((h as f32 * (width as f32 / height as f32)) as isize, h)
                    }
                    _ => return Ok((contents_arc.to_vec(), None)),
                };
                let contents_arc_moved = contents_arc.clone();
                if let Ok(bytes) = try_resize(
                    &contents_arc_moved,
                    target_width as u32,
                    target_height as u32,
                )
                .await
                {
                    return Ok((bytes, Some("image/webp".to_string())));
                }
            }
        }
        Ok((contents_arc.to_vec(), None))
    }
}
