use std::cmp::min;
use std::io::Cursor;
use std::sync::Arc;
use std::path::PathBuf;

use actix_web::web::block;
use async_std::fs::File;
use async_std::io::ReadExt;
use image::imageops::FilterType;
use image::io::Reader;
use image::ImageError;
use webp::Encoder;

use crate::database::FileMetadata;
use crate::environment::LOCAL_STORAGE_PATH;
use crate::{
    environment::{get_s3_bucket, USE_S3},
    routes::serve::Resize,
};

use super::errors::Error;

pub fn determine_video_size(path: &std::path::Path) -> Result<(isize, isize), Error> {
    let data = ffprobe::ffprobe(path).map_err(|_| Error::ProcessingError)?;
    for stream in data.streams {
        if let (Some(w), Some(h)) = (stream.width, stream.height) {
            if let (Ok(w), Ok(h)) = (w.try_into(), h.try_into()) {
                return Ok((w, h));
            }
        }
    }
    Err(Error::ProcessingError)
}

pub fn try_resize(buf: &Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, ImageError> {
    let image = Reader::new(Cursor::new(buf))
        .with_guessed_format()?
        .decode()?
        .resize_exact(width, height, FilterType::Gaussian);
    let encoder = Encoder::from_image(&image).expect("Failed to create webp encoder");
    let bytes = encoder.encode_lossless().to_vec();
    Ok(bytes)
}

pub async fn fetch_file(
    id: &str,
    store: &str,
    metadata: FileMetadata,
    resize: Option<Resize>,
) -> Result<(Vec<u8>, Option<String>), Error> {
    let mut contents = Vec::new();
    if *USE_S3 {
        let bucket = get_s3_bucket(store)?;
        let response = bucket
            .get_object(format!("/{}", id))
            .await
            .map_err(|_| Error::StorageError)?;
        if response.status_code() != 200 {
            return Err(Error::StorageError);
        }
        contents = response.bytes().to_vec();
    } else {
        let path: PathBuf = format!("{}/{}", *LOCAL_STORAGE_PATH, id)
            .parse()
            .map_err(|_| Error::StorageError)?;

        let mut f = File::open(path).await.map_err(|_| Error::StorageError)?;
        f.read_to_end(&mut contents)
            .await
            .map_err(|_| Error::StorageError)?;
    }
    let contents_arc = Arc::new(contents);
    if let Some(parameters) = resize {
        if let FileMetadata::Image { width, height } = metadata {
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
            if let Ok(Ok(bytes)) = block(move || {
                try_resize(
                    &contents_arc_moved,
                    target_width as u32,
                    target_height as u32,
                )
            })
            .await
            {
                return Ok((bytes, Some("image/webp".to_string())));
            }
        }
    }
    Ok((contents_arc.to_vec(), None))
}
