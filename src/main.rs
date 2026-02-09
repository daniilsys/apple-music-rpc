use serde::Serialize;
use std::env;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "1470151628547031280";

#[derive(Serialize)]
struct Handshake<'a> {
    v: u8,
    client_id: &'a str,
}

#[derive(Serialize)]
struct SetActivityCommand<'a> {
    cmd: &'a str,
    nonce: String,
    args: ActivityArgs<'a>,
}

#[derive(Serialize)]
struct ActivityArgs<'a> {
    pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity: Option<Activity<'a>>,
}

#[derive(Serialize)]
struct Activity<'a> {
    name: &'a str,
    r#type: u8,
    details: &'a str,
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamps: Option<Timestamps>,
    assets: Assets<'a>,
}

#[derive(Serialize)]
struct Timestamps {
    start: i64,
    end: i64,
}

#[derive(Serialize)]
struct Assets<'a> {
    large_image: &'a str,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug)]
struct NowPlaying {
    track: String,
    artist: String,
    album: String,
    state: PlayerState,
    position_secs: f32,
    duration_secs: f32,
}

impl NowPlaying {
    fn key(&self) -> (&str, &str, &str) {
        (&self.track, &self.artist, &self.album)
    }

    fn state_string(&self) -> String {
        if self.album.is_empty() {
            self.artist.clone()
        } else {
            format!("{} • {}", self.artist, self.album)
        }
    }
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(p) = env::var("DISCORD_IPC_PATH") {
        dirs.push(PathBuf::from(p));
    }
    if let Ok(p) = env::var("TMPDIR") {
        dirs.push(PathBuf::from(p));
    }
    dirs.push(PathBuf::from("/tmp"));

    dirs
}

fn try_connect_discord_ipc() -> io::Result<UnixStream> {
    let dirs = candidate_dirs();
    for dir in dirs {
        for i in 0..10 {
            let path = dir.join(format!("discord-ipc-{}", i));

            if path.exists() {
                if let Ok(stream) = UnixStream::connect(&path) {
                    println!("✅ Connected to Discord IPC at: {}", path.display());
                    return Ok(stream);
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Could not find Discord IPC socket",
    ))
}

fn send_frame(stream: &mut UnixStream, op: u32, payload_json: &str) -> io::Result<()> {
    let payload = payload_json.as_bytes();
    let len = payload.len() as u32;

    stream.write_all(&op.to_le_bytes())?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(payload)?;
    stream.flush()?;
    Ok(())
}

fn read_frame(stream: &mut UnixStream) -> io::Result<(u32, String)> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;

    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;

    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload)?;

    let json = String::from_utf8_lossy(&payload).to_string();
    Ok((op, json))
}

fn send_handshake(stream: &mut UnixStream, client_id: &str) -> io::Result<()> {
    let handshake = Handshake { v: 1, client_id };
    let payload = serde_json::to_string(&handshake).unwrap();
    send_frame(stream, 0, &payload)
}

fn set_activity_now_playing(stream: &mut UnixStream, np: &NowPlaying) -> io::Result<()> {
    let timestamps = if np.state == PlayerState::Playing {
        let start = unix_now_secs() - np.position_secs.floor() as i64;
        let end = start + np.duration_secs.floor() as i64;
        Some(Timestamps { start, end })
    } else {
        None
    };

    let command = SetActivityCommand {
        cmd: "SET_ACTIVITY",
        nonce: unix_now_secs().to_string(),
        args: ActivityArgs {
            pid: std::process::id(),
            activity: Some(Activity {
                name: "Apple Music",
                r#type: 2,
                details: &np.track,
                state: np.state_string(),
                timestamps,
                assets: Assets {
                    large_image: "am_icon_001",
                },
            }),
        },
    };

    let payload = serde_json::to_string(&command).unwrap();
    send_frame(stream, 1, &payload)
}

fn clear_activity(stream: &mut UnixStream) -> io::Result<()> {
    let command = SetActivityCommand {
        cmd: "SET_ACTIVITY",
        nonce: unix_now_secs().to_string(),
        args: ActivityArgs {
            pid: std::process::id(),
            activity: None,
        },
    };

    let payload = serde_json::to_string(&command).unwrap();
    send_frame(stream, 1, &payload)
}

fn read_apple_music_raw() -> Option<String> {
    let script = r#"
    tell application "Music"
        if not (it is running) then
            return "STOPPED"
        end if

        set ps to player state as text
        if ps is not "playing" and ps is not "paused" then
            return "STOPPED"
        end if

        set t to current track
        return (name of t) & "||" & (artist of t) & "||" & (album of t) & "||" & ps & "||" & (player position) & "||" & (duration of t)
    end tell
    "#;

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();

    if trimmed.is_empty() || trimmed == "STOPPED" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_state(state_str: &str) -> PlayerState {
    match state_str {
        "playing" => PlayerState::Playing,
        "paused" => PlayerState::Paused,
        _ => PlayerState::Stopped,
    }
}

fn parse_f32(s: &str) -> Option<f32> {
    s.replace(',', ".").parse().ok()
}

fn parse_now_playing(raw: &str) -> Option<NowPlaying> {
    let mut parts = raw.split("||").map(str::trim);

    Some(NowPlaying {
        track: parts.next()?.to_string(),
        artist: parts.next()?.to_string(),
        album: parts.next()?.to_string(),
        state: parse_state(parts.next()?),
        position_secs: parse_f32(parts.next()?)?,
        duration_secs: parse_f32(parts.next()?)?,
    })
}

fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn main() -> io::Result<()> {
    let mut stream = try_connect_discord_ipc()?;
    send_handshake(&mut stream, CLIENT_ID)?;
    let (_op, _resp) = read_frame(&mut stream)?;

    let mut last_key: Option<(String, String, String)> = None;
    let mut last_state: Option<PlayerState> = None;
    let mut was_stopped = true;

    loop {
        let current = read_apple_music_raw().and_then(|raw| parse_now_playing(&raw));

        match current {
            None => {
                if !was_stopped {
                    clear_activity(&mut stream)?;
                    let _ = read_frame(&mut stream);
                    was_stopped = true;
                    last_key = None;
                    last_state = None;
                }
            }
            Some(np) => {
                was_stopped = false;

                let key = np.key();
                let state_changed = last_state.as_ref() != Some(&np.state);
                let track_changed = last_key
                    .as_ref()
                    .map(|(t, ar, al)| (t.as_str(), ar.as_str(), al.as_str()))
                    != Some(key);

                if track_changed || state_changed {
                    set_activity_now_playing(&mut stream, &np)?;
                    let _ = read_frame(&mut stream);
                    last_key = Some((np.track, np.artist, np.album));
                    last_state = Some(np.state);
                }
            }
        }

        sleep(Duration::from_secs(3));
    }
}
