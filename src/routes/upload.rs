use std::convert::TryInto;
use std::io::Write;

use actix_multipart::Multipart;
use actix_web::{web, Responder};
use content_inspector::inspect;
use futures::{StreamExt, TryStreamExt};
use serde::Serialize;
use tempfile::NamedTempFile;

use crate::environment::{get_s3_bucket, LOCAL_STORAGE_PATH, USE_S3};
use crate::errors::{Error, Result};
use crate::files::{get_collection, File, FileMetadata};
use crate::stores::{ContentType, Store};
use crate::utilities::determine_video_size;

#[derive(Serialize)]
pub struct UploadResponse {
    id: String,
}

pub async fn handle(path: web::Path<String>, mut payload: Multipart) -> Result<impl Responder> {
    let store_id = path.into_inner();
    let store = Store::get(&store_id)?;
    if let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition();
        let filename = content_type
            .get_filename()
            .ok_or(Error::InvalidData)?
            .to_string();
        let mut file_size: usize = 0;
        let mut buf: Vec<u8> = Vec::new();
        while let Some(chunk) = field.next().await {
            let data = chunk.map_err(|_| Error::InvalidData)?;
            file_size += data.len();
            if file_size > store.max_size {
                return Err(Error::FileTooLarge {
                    max_size: store.max_size,
                });
            }
            buf.append(&mut data.to_vec());
        }
        let content_type = tree_magic_mini::from_u8(&buf);
        let metadata = match content_type {
            "image/jpeg" | "image/png" | "image/gif" | "image/webp" => {
                if let Ok(imagesize::ImageSize { width, height }) = imagesize::blob_size(&buf) {
                    FileMetadata::Image {
                        width: width.try_into().map_err(|_| Error::ProcessingError)?,
                        height: height.try_into().map_err(|_| Error::ProcessingError)?,
                    }
                } else {
                    FileMetadata::File
                }
            }
            "video/mp4" | "video/webm" | "video/quicktime" => {
                let mut tmp = NamedTempFile::new().map_err(|_| Error::ProcessingError)?;
                tmp.write_all(&buf).map_err(|_| Error::ProcessingError)?;
                if let Ok((width, height)) = determine_video_size(tmp.path()).await {
                    FileMetadata::Video { width, height }
                } else {
                    FileMetadata::File
                }
            }
            "audio/mpeg" => FileMetadata::Audio,
            _ => {
                if inspect(&buf).is_text() {
                    FileMetadata::Text
                } else {
                    FileMetadata::File
                }
            }
        };
        if let Some(content_type) = &store.restrict_content_type {
            if !matches!(
                (content_type, &metadata),
                (ContentType::Image, FileMetadata::Image { .. })
                    | (ContentType::Video, FileMetadata::Video { .. })
                    | (ContentType::Audio, FileMetadata::Audio)
            ) {
                return Err(Error::FileTypeNotAllowed);
            }
        }
        let id = ulid::Ulid::new().to_string();
        let file = File {
            id: id.clone(),
            store: store_id.clone(),
            filename,
            metadata,
            content_type: content_type.to_string(),
            size: buf.len() as isize,
            deleted: false,
            flagged: false,
            attached: false,
        };
        get_collection()
            .insert_one(&file, None)
            .await
            .map_err(|_| Error::DatabaseError)?;
        if *USE_S3 {
            let bucket = get_s3_bucket(&store_id)?;
            let response = bucket
                .put_object(format!("/{}", file.id), &buf)
                .await
                .map_err(|_| Error::StorageError)?;
            if response.status_code() != 200 {
                return Err(Error::StorageError);
            }
        } else {
            let path = format!("{}/{}", *LOCAL_STORAGE_PATH, &file.id);
            let mut f = web::block(|| std::fs::File::create(path))
                .await
                .map_err(|_| Error::StorageError)?
                .map_err(|_| Error::StorageError)?;

            web::block(move || f.write_all(&buf))
                .await
                .map_err(|_| Error::StorageError)?
                .map_err(|_| Error::StorageError)?;
        }
        Ok(web::Json(UploadResponse { id }))
    } else {
        Err(Error::MissingData)
    }
}
