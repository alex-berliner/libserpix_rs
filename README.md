### Screen Reader
This is a Rust library and example screen reader application meant to accompany [LibSerpix](https://github.com/alex-berliner/LibSerpix), which allows real-time data transmission out of World of Warcraft via encoding data into pixels.

### Quest Text Sender

### WoW TTS
Finally, included in this repo is a sample python program that captures the output produced by the example application and uses gTTS to perform real-time text-to-speech. You must compile with `cargo build --release --bin wow` then place `target\release\wow.exe` in tts/, then run `python .\parser.py`.
