use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Response;
use scraper::{Html, Selector};
use serde::Serialize;
use std::collections::HashMap;
use validator::Validate;
use youtubei_rs::{query::player, utils::default_client_config};

use crate::{
    errors::{Error, Result},
    scraper::get_twitch_channel,
    utilities::{fetch, get_media_size, Image, ImageSize, Special, Video},
};

lazy_static! {
    static ref RE_YOUTUBE: Regex = Regex::new("^(?:(?:https?:)?//)?(?:(?:www|m)\\.)?(?:(?:youtube\\.com|youtu.be))(?:/(?:[\\w\\-]+\\?v=|embed/|v/)?)([\\w\\-]+)(?:\\S+)?$").expect("Failed to compile regex");
    static ref RE_TWITCH: Regex = Regex::new("^(?:https?://)?(?:www\\.|go\\.)?twitch\\.tv/([a-z0-9_]+)($|\\?)").expect("Failed to compile regex");
    static ref RE_SPOTIFY: Regex = Regex::new("^(?:https?://)?open.spotify.com/(track|user|artist|album|playlist)/([A-z0-9]+)").expect("Failed to compile regex");
    static ref RE_SOUNDCLOUD: Regex = Regex::new("^(?:https?://)?soundcloud.com/([a-zA-Z0-9-]+)/([A-z0-9-]+)").expect("Failed to compile regex");

    static ref RE_GIF: Regex = Regex::new("^(?:https?://)?(www\\.)?(tenor\\.com/view|giphy\\.com/gifs|gfycat\\.com)/[\\w\\d-]+").expect("Failed to compile regex");

    static ref RE_TIMESTAMP: Regex =
        Regex::new("(?:\\?|&)(?:t|start)=([\\w]+)").expect("Failed to compile regex");
}

#[derive(Validate, Debug, Serialize)]
pub struct Metadata {
    #[validate(length(min = 1, max = 256))]
    url: String,
    original_url: String,
    special: Option<Special>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1, max = 100))]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1, max = 2000))]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate]
    image: Option<Image>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate]
    video: Option<Video>,

    #[serde(skip_serializing_if = "Option::is_none")]
    opengraph_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1, max = 100))]
    site_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1, max = 256))]
    icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[validate(length(min = 1, max = 64))]
    color: Option<String>,
}

impl Metadata {
    pub async fn from(resp: Response, url: String) -> Result<Metadata> {
        let body = resp.text().await.map_err(|_| Error::MetaParseFailed)?;
        let fragment = Html::parse_document(&body);
        let meta_selector = Selector::parse("meta").map_err(|_| Error::MetaParseFailed)?;
        let mut meta = HashMap::new();
        for el in fragment.select(&meta_selector) {
            let node = el.value();
            if let (Some(property), Some(content)) = (
                node.attr("property").or_else(|| node.attr("name")),
                node.attr("content"),
            ) {
                meta.insert(property.to_string(), content.to_string());
            }
        }
        let link_selector = Selector::parse("link").map_err(|_| Error::MetaParseFailed)?;
        let mut link = HashMap::new();
        for el in fragment.select(&link_selector) {
            let node = el.value();
            if let (Some(property), Some(content)) = (node.attr("rel"), node.attr("href")) {
                link.insert(property.to_string(), content.to_string());
            }
        }
        let metadata = Metadata {
            title: meta
                .remove("og:title")
                .or_else(|| meta.remove("twitter:title"))
                .or_else(|| meta.remove("title")),
            description: meta
                .remove("og:description")
                .or_else(|| meta.remove("twitter:description"))
                .or_else(|| meta.remove("description")),
            image: meta
                .remove("og:image")
                .or_else(|| meta.remove("og:image:secure_url"))
                .or_else(|| meta.remove("twitter:image"))
                .or_else(|| meta.remove("twitter:image:src"))
                .map(|url| {
                    let mut size = ImageSize::Preview;
                    if let Some(card) = meta.remove("twitter:card") {
                        if &card == "summary_large_image" {
                            size = ImageSize::Large;
                        }
                    }
                    Image {
                        url,
                        width: meta
                            .remove("og:image:width")
                            .unwrap_or_else(|| "0".to_string())
                            .parse()
                            .unwrap_or(0),
                        height: meta
                            .remove("og:image:height")
                            .unwrap_or_else(|| "0".to_string())
                            .parse()
                            .unwrap_or(0),
                        size,
                    }
                }),
            video: meta
                .remove("og:video")
                .or_else(|| meta.remove("og:video:url"))
                .or_else(|| meta.remove("og:video:secure_url"))
                .map(|url| Video {
                    url,
                    width: meta
                        .remove("og:video:width")
                        .unwrap_or_else(|| "0".to_string())
                        .parse()
                        .unwrap_or(0),
                    height: meta
                        .remove("og:video:height")
                        .unwrap_or_else(|| "0".to_string())
                        .parse()
                        .unwrap_or(0),
                }),
            icon_url: link
                .remove("apple-touch-icon")
                .or_else(|| link.remove("icon"))
                .map(|mut v| {
                    if let Some(ch) = v.chars().next() {
                        if ch == '/' {
                            v = format!("{}{}", &url, v);
                        }
                    }
                    v
                }),
            color: meta.remove("theme-color"),
            opengraph_type: meta.remove("og:type"),
            site_name: meta.remove("og:site_name"),
            url: meta.remove("og:url").unwrap_or_else(|| url.clone()),
            original_url: url,
            special: None,
        };
        metadata.validate().map_err(|_| Error::ValidationFailed)?;
        Ok(metadata)
    }

    async fn resolve_image(&mut self) -> Result<()> {
        if let Some(image) = &mut self.image {
            if image.width != 0 && image.height != 0 {
                return Ok(());
            }
            let (resp, mime) = fetch(&image.url).await?;
            let (width, height) = get_media_size(resp, mime).await?;
            image.width = width;
            image.height = height;
        }
        Ok(())
    }

    pub async fn generate_special(&mut self) -> Result<Special> {
        if let Some(captures) = RE_YOUTUBE.captures_iter(&self.url).next() {
            if let Some(video) = &self.video {
                let client = default_client_config();
                let timestamp_captures = RE_TIMESTAMP.captures_iter(&video.url).next();
                let id = captures[1].to_string();
                let video = player(id.clone(), String::new(), &client).await;
                if let Ok(video) = video {
                    if !video.video_details.is_private {
                        let thumbnail = video
                            .video_details
                            .thumbnail
                            .thumbnails
                            .iter()
                            .max_by_key(|t| t.width);
                        return Ok(Special::Youtube {
                            id,
                            timestamp: timestamp_captures.map(|c| c[1].to_string()),
                            title: video.video_details.title,
                            thumbnail: thumbnail.expect("No thumbnail").url.clone(),
                            author: video.video_details.author,
                        });
                    }
                }
            }
        } else if let Some(captures) = RE_TWITCH.captures_iter(&self.original_url).next() {
            let channel_id = captures[1].to_string();
            let channel = get_twitch_channel(channel_id).await;
            if let Ok(channel) = channel {
                return Ok(Special::Twitch { channel });
            }
        } else if let Some(captures) = RE_SPOTIFY.captures_iter(&self.original_url).next() {
            return Ok(Special::Spotify {
                content_type: captures[1].to_string(),
                id: captures[2].to_string(),
            });
        } else if RE_SOUNDCLOUD.is_match(&self.original_url) {
            return Ok(Special::Soundcloud);
        } else if RE_GIF.is_match(&self.original_url) {
            return Ok(Special::Gif);
        }
        Ok(Special::None)
    }

    pub async fn resolve_external(&mut self) {
        if let Ok(special) = self.generate_special().await {
            match &special {
                Special::Youtube { .. } => self.color = Some("#FF424F".to_string()),
                Special::Twitch { .. } => self.color = Some("#7B68EE".to_string()),
                Special::Spotify { .. } => self.color = Some("#1ABC9C".to_string()),
                Special::Soundcloud { .. } => self.color = Some("#FF7F50".to_string()),
                _ => {}
            }
            self.special = Some(special);
        }
        if self.resolve_image().await.is_err() {
            self.image = None;
        }
    }

    pub fn is_none(&self) -> bool {
        self.title.is_none()
            && self.description.is_none()
            && self.image.is_none()
            && self.video.is_none()
    }
}
