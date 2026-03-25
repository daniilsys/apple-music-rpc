use std::process::Command;

use crate::types::{NowPlaying, PlayerState};

pub fn read_apple_music_raw() -> Option<String> {
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

pub fn parse_now_playing(raw: &str) -> Option<NowPlaying> {
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
