pub mod constants;
pub mod database;
pub mod environment;
pub mod errors;
pub mod files;
pub mod metadata;
pub mod routes;
pub mod scraper;
pub mod stores;
pub mod utilities;

use std::{env, time::Duration};

use actix_cors::Cors;
use actix_files::Files;
use actix_web::{middleware::Logger, web, App, HttpServer};
use async_std::stream::StreamExt;
use async_std::{fs::create_dir_all, task};
use log::info;
use mongodb::bson::doc;

use crate::environment::{HOST, LOCAL_STORAGE_PATH, USE_S3};
use crate::files::get_collection;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    if let Ok(v) = env::var("MINIO_ROOT_USER") {
        env::set_var("AWS_ACCESS_KEY_ID", v);
    }
    if let Ok(v) = env::var("MINIO_ROOT_PASSWORD") {
        env::set_var("AWS_SECRET_ACCESS_KEY", v);
    }
    env_logger::init_from_env(env_logger::Env::default().filter_or("RUST_LOG", "info"));

    info!("Nextflow CDN version {}", constants::VERSION);

    stores::load_stores().expect("Failed to load stores");

    info!("Connecting to database...");
    database::connect().await;

    if !*USE_S3 {
        info!("Using local storage, the directory will be created if it does not exist!");
        create_dir_all(LOCAL_STORAGE_PATH.to_string())
            .await
            .expect("Failed to create local storage directory");
    } else {
        info!("Using S3 storage, make sure all configured stores have buckets!");
    }

    info!("Starting background tasks...");
    task::spawn(async {
        loop {
            task::spawn(async {
                let collection = get_collection();
                let mut cursor = collection
                    .find(
                        doc! {
                            "deleted": true,
                            "flagged": false
                        },
                    )
                    .await
                    .expect("Failed to find files to delete");
                while let Some(result) = cursor.next().await {
                    if let Ok(file) = result {
                        file.delete().await.expect("Failed to delete file");
                    }
                    task::sleep(Duration::from_millis(50)).await;
                }
            });
            task::sleep(Duration::from_secs(900)).await;
        }
    });

    info!("Starting server on {}...", *HOST);
    HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allowed_origin_fn(|_, _| true)
                    .allow_any_method()
                    .allow_any_header()
                    .supports_credentials(),
            )
            .wrap(Logger::default())
            .service(Files::new("/assets", "assets"))
            .route("/", web::get().to(routes::service::handle))
            .route("/stores/{store}", web::post().to(routes::upload::handle))
            .route(
                "/stores/{store}/download/{filename:.*}",
                web::get().to(routes::download::handle),
            )
            .route(
                "/stores/{store}/files/{filename:.*}",
                web::get().to(routes::serve::handle),
            )
            .route("/embed", web::get().to(routes::embed::handle))
            .route("/proxy", web::get().to(routes::proxy::handle))
    })
    .bind(&*HOST)?
    .run()
    .await
}
