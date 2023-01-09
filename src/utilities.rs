use std::io::Cursor;
use std::time::Duration;
use std::io::Write;

use image::imageops::FilterType;
use image::io::Reader;
use image::ImageError;
use lazy_static::lazy_static;
use mime::Mime;
use reqwest::{header::CONTENT_TYPE, Client, Response};
use serde::Serialize;
use tempfile::NamedTempFile;
use validator::Validate;
use webp::Encoder;

use crate::scraper::TwitchChannel;
use crate::{
    metadata::Metadata,
};

use super::errors::Error;

#[derive(Debug, Serialize)]
pub enum ImageSize {
    Large,
    Preview,
}

#[derive(Validate, Debug, Serialize)]
pub struct Image {
    #[validate(length(min = 1, max = 512))]
    pub url: String,
    pub width: isize,
    pub height: isize,
    pub size: ImageSize,
}

#[derive(Validate, Debug, Serialize)]
pub struct Video {
    #[validate(length(min = 1, max = 512))]
    pub url: String,
    pub width: isize,
    pub height: isize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Embed {
    Website(Box<Metadata>),
    Image(Image),
    Video(Video),
    None,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum Special {
    None,
    Gif,
    Youtube {
        id: String,
        title: String,
        thumbnail: String,
        author: String,

        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<String>,
    },
    Twitch {
        channel: TwitchChannel,
    },
    Spotify {
        content_type: String,
        id: String,
    },
    Soundcloud,
}

lazy_static! {
    static ref CLIENT: Client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (compatible; NextflowCDN/1.0; +https://github.com/Nextflow-Cloud/cdn)"
        )
        .timeout(Duration::from_secs(2))
        .build()
        .expect("Failed to build reqwest client");
}

pub async fn fetch(url: &str) -> Result<(Response, Mime), Error> {
    let resp = CLIENT
        .get(url)
        .send()
        .await
        .map_err(|_| Error::InternalRequestFailed)?;
    if !resp.status().is_success() {
        return Err(Error::RequestFailed);
    }
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .ok_or(Error::MissingContentType)?
        .to_str()
        .map_err(|_| Error::InternalRequestFailed)?;
    let mime: mime::Mime = content_type
        .parse()
        .map_err(|_| Error::InternalRequestFailed)?;
    Ok((resp, mime))
}

pub async fn determine_video_size(path: &std::path::Path) -> Result<(isize, isize), Error> {
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

pub async fn get_media_size(resp: Response, mime: Mime) -> Result<(isize, isize), Error> {
    let bytes = resp
        .bytes()
        .await
        .map_err(|_| Error::InternalRequestFailed)?;
    match mime.type_() {
        mime::IMAGE => {
            if let Ok(size) = imagesize::blob_size(&bytes) {
                Ok((size.width as isize, size.height as isize))
            } else {
                Err(Error::ProcessingError)
            }
        }
        mime::VIDEO => {
            let mut tmp = NamedTempFile::new().map_err(|_| Error::ProcessingError)?;
            tmp.write_all(&bytes).map_err(|_| Error::ProcessingError)?;
            determine_video_size(tmp.path()).await
        }
        _ => unreachable!(),
    }
}

pub async fn try_resize(buf: &Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, ImageError> {
    let image = Reader::new(Cursor::new(buf))
        .with_guessed_format()?
        .decode()?
        .resize_exact(width, height, FilterType::Gaussian);
    let encoder = Encoder::from_image(&image).expect("Failed to create webp encoder");
    let bytes = encoder.encode_lossless().to_vec();
    Ok(bytes)
}
