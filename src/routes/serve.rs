use actix_web::web;
use actix_web::{HttpResponse, Responder};
use mongodb::bson::doc;
use serde::Deserialize;

use crate::constants::CACHE_CONTROL;
use crate::files::File;
use crate::errors::Result;
use crate::stores::Store;

#[derive(Deserialize)]
pub struct Resize {
    pub size: Option<isize>,
    pub width: Option<isize>,
    pub height: Option<isize>,
    pub max_side: Option<isize>,
}

pub async fn handle(
    path: web::Path<(String, String)>,
    resize: web::Query<Resize>,
) -> Result<impl Responder> {
    let (store_id, id) = path.into_inner();
    Store::get(&store_id)?;
    let file = File::find(&id, &store_id).await?;
    let (contents, content_type) =
        file.fetch(Some(resize.0)).await?;
    let content_type = content_type.unwrap_or(file.content_type);
    let disposition = match content_type.as_ref() {
        "image/jpeg" | "image/png" | "image/gif" | "image/webp" | "video/mp4" | "video/webm"
        | "video/webp" | "audio/quicktime" | "audio/mpeg" => "inline",
        _ => "attachment",
    };
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Disposition", disposition))
        .insert_header(("Cache-Control", CACHE_CONTROL))
        .content_type(content_type)
        .body(contents))
}
