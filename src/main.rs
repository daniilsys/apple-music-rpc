mod apple_music;
mod artwork;
mod discord;
mod types;

use std::os::unix::net::UnixStream;
use std::thread::sleep;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use types::PlayerState;

pub fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn main() -> std::io::Result<()> {
    let mut stream: Option<UnixStream> = None;
    let mut last_key: Option<(String, String, String)> = None;
    let mut last_state: Option<PlayerState> = None;
    let mut was_stopped = true;

    loop {
        if stream.is_none() {
            match discord::connect_and_handshake() {
                Ok(s) => {
                    stream = Some(s);
                    last_key = None;
                    last_state = None;
                }
                Err(e) => {
                    println!(
                        "Discord non disponible: {}. Nouvelle tentative dans 5s...",
                        e
                    );
                    sleep(Duration::from_secs(5));
                    continue;
                }
            }
        }

        let current =
            apple_music::read_apple_music_raw().and_then(|raw| apple_music::parse_now_playing(&raw));

        let result = match &current {
            None => {
                if !was_stopped {
                    if let Some(ref mut s) = stream {
                        discord::clear_activity(s)
                            .and_then(|_| discord::read_frame(s).map(|_| ()))
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            Some(np) => {
                let key = np.key();
                let state_changed = last_state.as_ref() != Some(&np.state);
                let track_changed = last_key
                    .as_ref()
                    .map(|(t, ar, al)| (t.as_str(), ar.as_str(), al.as_str()))
                    != Some(key);

                if track_changed || state_changed {
                    if let Some(ref mut s) = stream {
                        discord::set_activity_now_playing(s, np)
                            .and_then(|_| discord::read_frame(s).map(|_| ()))
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
        };

        if result.is_err() {
            println!("Connexion Discord perdue. Reconnexion...");
            stream = None;
            continue;
        }

        match current {
            Some(np) => {
                was_stopped = false;
                let key = np.key();
                let state_changed = last_state.as_ref() != Some(&np.state);
                let track_changed = last_key
                    .as_ref()
                    .map(|(t, ar, al)| (t.as_str(), ar.as_str(), al.as_str()))
                    != Some(key);

                if track_changed || state_changed {
                    last_key = Some((np.track, np.artist, np.album));
                    last_state = Some(np.state);
                }
            }
            None => {
                if !was_stopped {
                    was_stopped = true;
                    last_key = None;
                    last_state = None;
                }
            }
        }

        sleep(Duration::from_secs(3));
    }
}
