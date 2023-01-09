use actix_web::{web::Query, HttpResponse, Responder};
use serde::Deserialize;

use crate::errors::{Error, Result};
use crate::utilities::fetch;

#[derive(Deserialize)]
pub struct Parameters {
    url: String,
}

pub async fn handle(info: Query<Parameters>) -> Result<impl Responder> {
    let url = info.into_inner().url;
    let (resp, mime) = fetch(&url).await?;
    if matches!(mime.type_(), mime::IMAGE | mime::VIDEO) {
        let body = resp
            .bytes()
            .await
            .map_err(|_| Error::InternalRequestFailed)?;
        Ok(HttpResponse::Ok().body(body))
    } else {
        Err(Error::CannotProxy)
    }
}
