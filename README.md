# 🎵 Apple Music Discord Rich Presence (macOS)

A **lightweight**, native **Rust** daemon that shows your current Apple Music track
as a **Discord Rich Presence** on macOS.

No Electron. No heavy SDK. Just AppleScript + Discord IPC.

---

## ✨ Features

- Shows **track / artist / album**
- Discord activity type: **Listening**
- Progress bar / remaining time
- Updates only when the track or state changes
- Extremely low memory usage (~1 MB)
- Native macOS (Apple Music app)

---

## 📦 Requirements

- macOS
- Apple Music (Music.app)
- Discord Desktop
- Rust (via `rustup`)

---

## 🚀 Build & Run

```bash
git clone https://github.com/TON_USER/TON_REPO.git
cd TON_REPO
cargo build --release
./target/release/apple-music-discord-rpc
```
