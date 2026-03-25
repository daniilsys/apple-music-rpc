use std::env;
use std::process::Command;
use std::time::Duration;

use crate::types::{DeezerResponse};

pub fn detect_system_proxy() -> Option<String> {
    let output = Command::new("scutil").arg("--proxy").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);

    let enabled = text.lines().find(|l| l.contains("HTTPEnable"))?;
    if !enabled.contains("1") {
        return None;
    }

    let host = text
        .lines()
        .find(|l| l.contains("HTTPProxy"))?
        .split(':')
        .last()?
        .trim()
        .to_string();
    let port = text
        .lines()
        .find(|l| l.contains("HTTPPort"))?
        .split(':')
        .last()?
        .trim()
        .to_string();

    Some(format!("http://{}:{}", host, port))
}

fn make_agent(proxy_url: Option<&str>) -> ureq::Agent {
    let config = ureq::config::Config::builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .proxy(match proxy_url {
            Some(url) => ureq::Proxy::new(url).ok(),
            None => None,
        });
    ureq::Agent::new_with_config(config.build())
}

fn deezer_search(agent: &ureq::Agent, artist: &str, track: &str) -> Option<String> {
    let queries = [
        format!("artist:\"{}\" track:\"{}\"", artist, track),
        format!("{} {}", artist, track),
    ];
    for query in &queries {
        let url = format!(
            "https://api.deezer.com/search?q={}&limit=1",
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>()
        );
        let Ok(mut resp) = agent.get(&url).call() else { continue };
        let Ok(body) = resp.body_mut().read_json::<DeezerResponse>() else { continue };
        if let Some(cover) = body.data.and_then(|d| d.first()?.album.as_ref()?.cover_xl.clone()) {
            return Some(cover);
        }
    }
    None
}

pub fn fetch_artwork_url(artist: &str, track: &str) -> Option<String> {
    // Try direct connection first
    let direct = make_agent(None);
    if let Some(cover) = deezer_search(&direct, artist, track) {
        return Some(cover);
    }

    // Fallback: try with proxy (env var or system)
    let proxy_url = env::var("HTTPS_PROXY")
        .or_else(|_| env::var("HTTP_PROXY"))
        .ok()
        .or_else(detect_system_proxy);
    if let Some(ref url) = proxy_url {
        let proxied = make_agent(Some(url));
        return deezer_search(&proxied, artist, track);
    }

    None
}
