use actix_web::{
    web::{Json, Query},
    Responder,
};
use serde::Deserialize;

use crate::errors::Result;
use crate::metadata::Metadata;
use crate::utilities::{fetch, get_media_size, Embed, Image, ImageSize, Video};

#[derive(Deserialize)]
pub struct Parameters {
    url: String,
}

pub async fn handle(info: Query<Parameters>) -> Result<impl Responder> {
    let url = info.into_inner().url;
    let (resp, mime) = fetch(&url).await?;
    match (mime.type_(), mime.subtype()) {
        (_, mime::HTML) => {
            let mut metadata = Metadata::from(resp, url.to_string()).await?;
            metadata.resolve_external().await;
            if metadata.is_none() {
                return Ok(Json(Embed::None));
            }
            Ok(Json(Embed::Website(Box::new(metadata))))
        }
        (mime::IMAGE, _) => {
            if let Ok((width, height)) = get_media_size(resp, mime).await {
                Ok(Json(Embed::Image(Image {
                    url: url.to_string(),
                    width,
                    height,
                    size: ImageSize::Large,
                })))
            } else {
                Ok(Json(Embed::None))
            }
        }
        (mime::VIDEO, _) => {
            if let Ok((width, height)) = get_media_size(resp, mime).await {
                Ok(Json(Embed::Video(Video {
                    url: url.to_string(),
                    width,
                    height,
                })))
            } else {
                Ok(Json(Embed::None))
            }
        }
        _ => Ok(Json(Embed::None)),
    }
}
