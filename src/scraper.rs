use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TwitchChannel {
    pub id: String,
    pub name: String,
    pub color: String,
    pub avatar: String,
    pub banner: String,
}

pub async fn get_twitch_channel(channel_id: String) -> Result<TwitchChannel> {
    const CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
    const ROOT_URL: &str = "https://gql.twitch.tv/gql";
    let client = reqwest::Client::builder()
        .build()
        .expect("Failed to build reqwest client");
    let resp = client
        .post(ROOT_URL)
        .header("Client-ID", CLIENT_ID)
        .header("Content-Type", "text/plain;charset=UTF-8")
        .body(format!(
            "[{{\"operationName\":\"ChannelShell\",\"variables\":{{\"login\":\"{}\"}},\"extensions\":{{\"persistedQuery\":{{\"version\":1,\"sha256Hash\":\"580ab410bcd0c1ad194224957ae2241e5d252b2c5173d8e0cce9d32d5bb14efe\"}}}}}}]",
            channel_id
        ))
        .send()
        .await
        .map_err(|_| Error::InternalRequestFailed)?;
    let body = resp.text().await.map_err(|_| Error::InternalRequestFailed);
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|_| Error::InternalRequestFailed)?;
    let data = &json[0]["data"]["userOrError"];
    let banner = data["bannerImageURL"].as_str().ok_or(Error::InternalRequestFailed)?.to_string();
    let id = data["id"].as_str().ok_or(Error::InternalRequestFailed)?.to_string();
    let name = data["displayName"].as_str().ok_or(Error::InternalRequestFailed)?.to_string();
    let color = data["primaryColorHex"].as_str().ok_or(Error::InternalRequestFailed)?.to_string();
    let avatar = data["profileImageURL"].as_str().ok_or(Error::InternalRequestFailed)?.to_string();
    Ok(TwitchChannel {
        id,
        name,
        color,
        avatar,
        banner,
    })
}
