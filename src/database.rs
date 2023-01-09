use crate::environment::MONGODB_URI;

use mongodb::Client;
use once_cell::sync::OnceCell;

pub static DATABASE: OnceCell<Client> = OnceCell::new();

pub async fn connect() {
    let client = Client::with_uri_str(&*MONGODB_URI)
        .await
        .expect("Failed to connect to MongoDB");
    DATABASE.set(client).expect("Failed to set MongoDB client");
}
