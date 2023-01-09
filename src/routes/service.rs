use std::collections::HashMap;

use actix_web::{web, Responder};
use serde::Serialize;

use crate::{
    constants::{SERVICE, VERSION},
    stores::{get_stores, Store},
};

#[derive(Serialize)]
pub struct ServiceResponse {
    pub service: &'static str,
    pub stores: &'static HashMap<std::string::String, Store>,
    pub version: &'static str,
}

pub async fn handle() -> impl Responder {
    web::Json(ServiceResponse {
        service: SERVICE,
        stores: get_stores(),
        version: VERSION,
    })
}
