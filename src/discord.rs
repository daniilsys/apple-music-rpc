use std::env;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::Command;

use crate::artwork;
use crate::types::*;
use crate::unix_now_secs;

fn candidate_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(p) = env::var("DISCORD_IPC_PATH") {
        dirs.push(PathBuf::from(p));
    }
    if let Ok(p) = env::var("TMPDIR") {
        dirs.push(PathBuf::from(p));
    }

    // Discover TMPDIR via confstr when env var is missing (e.g. LaunchAgent)
    let output = Command::new("getconf").arg("DARWIN_USER_TEMP_DIR").output();
    if let Ok(out) = output {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            dirs.push(PathBuf::from(path));
        }
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
                    println!("Connected to Discord IPC at: {}", path.display());
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

pub fn read_frame(stream: &mut UnixStream) -> io::Result<(u32, String)> {
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

pub fn set_activity_now_playing(stream: &mut UnixStream, np: &NowPlaying) -> io::Result<()> {
    let timestamps = if np.state == PlayerState::Playing {
        let start = unix_now_secs() - np.position_secs.floor() as i64;
        let end = start + np.duration_secs.floor() as i64;
        Some(Timestamps { start, end })
    } else {
        None
    };

    let artwork_url =
        artwork::fetch_artwork_url(&np.artist, &np.track).unwrap_or_else(|| "am_icon_001".to_string());

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
                    large_image: artwork_url,
                    large_text: np.album.clone(),
                },
            }),
        },
    };

    let payload = serde_json::to_string(&command).unwrap();
    send_frame(stream, 1, &payload)
}

pub fn clear_activity(stream: &mut UnixStream) -> io::Result<()> {
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

pub fn connect_and_handshake() -> io::Result<UnixStream> {
    let mut stream = try_connect_discord_ipc()?;
    send_handshake(&mut stream, CLIENT_ID)?;
    let (_op, _resp) = read_frame(&mut stream)?;
    Ok(stream)
}
