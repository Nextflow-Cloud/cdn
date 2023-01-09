use crate::errors::{Error, Result};

use lazy_static::lazy_static;
use s3::{creds::Credentials, Region};
use std::env;

lazy_static! {
    pub static ref STORES: String =
        env::var("STORES").unwrap_or_else(|_| String::from("Stores.toml"));
    pub static ref HOST: String = env::var("HOST").expect("Missing CDN_HOST environment variable");
    pub static ref MONGODB_URI: String =
        env::var("MONGODB_URI").expect("Missing CDN_MONGODB_URI environment variable");
    pub static ref MONGODB_DATABASE: String =
        env::var("MONGODB_DATABASE").unwrap_or_else(|_| "cdn".to_string());
    pub static ref LOCAL_STORAGE_PATH: String =
        env::var("LOCAL_STORAGE_PATH").unwrap_or_else(|_| "./files".to_string());
    pub static ref S3_REGION: Region = Region::Custom {
        region: env::var("S3_REGION").unwrap_or_else(|_| String::new()),
        endpoint: env::var("S3_ENDPOINT").unwrap_or_else(|_| String::new())
    };
    pub static ref S3_CREDENTIALS: Credentials =
        Credentials::default().expect("Failed to get S3 credentials");
    pub static ref USE_S3: bool =
        env::var("CDN_S3_REGION").is_ok() && env::var("CDN_S3_ENDPOINT").is_ok();
}

pub fn get_s3_bucket(bucket: &str) -> Result<s3::Bucket> {
    Ok(
        s3::Bucket::new(bucket, S3_REGION.clone(), S3_CREDENTIALS.clone())
            .map_err(|_| Error::StorageError)?
            .with_path_style(),
    )
}
