use serde::{Deserialize, Serialize};

pub const CLIENT_ID: &str = "1470151628547031280";

#[derive(Serialize)]
pub struct Handshake<'a> {
    pub v: u8,
    pub client_id: &'a str,
}

#[derive(Serialize)]
pub struct SetActivityCommand<'a> {
    pub cmd: &'a str,
    pub nonce: String,
    pub args: ActivityArgs<'a>,
}

#[derive(Serialize)]
pub struct ActivityArgs<'a> {
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<Activity<'a>>,
}

#[derive(Serialize)]
pub struct Activity<'a> {
    pub name: &'a str,
    pub r#type: u8,
    pub details: &'a str,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    pub assets: Assets,
}

#[derive(Serialize)]
pub struct Timestamps {
    pub start: i64,
    pub end: i64,
}

#[derive(Serialize)]
pub struct Assets {
    pub large_image: String,
    pub large_text: String,
}

#[derive(Deserialize)]
pub struct DeezerResponse {
    pub data: Option<Vec<DeezerTrack>>,
}

#[derive(Deserialize)]
pub struct DeezerTrack {
    pub album: Option<DeezerAlbum>,
}

#[derive(Deserialize)]
pub struct DeezerAlbum {
    pub cover_xl: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug)]
pub struct NowPlaying {
    pub track: String,
    pub artist: String,
    pub album: String,
    pub state: PlayerState,
    pub position_secs: f32,
    pub duration_secs: f32,
}

impl NowPlaying {
    pub fn key(&self) -> (&str, &str, &str) {
        (&self.track, &self.artist, &self.album)
    }

    pub fn state_string(&self) -> String {
        if self.album.is_empty() {
            self.artist.clone()
        } else {
            format!("{} • {}", self.artist, self.album)
        }
    }
}
