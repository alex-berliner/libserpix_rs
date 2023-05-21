### libserpix_rs
This is a Rust library acting as the screen reader portion to accompany the lua library [LibSerpix](https://github.com/alex-berliner/LibSerpix), which allows real-time data transmission out of World of Warcraft via encoding data into pixels. Together they allow one to transmit JSON formatted messages out of World of Warcraft in real time.

### Example
This is [src/bin/wow.rs](src/bin/wow.rs). Run with `cargo run --release --bin wow`.
```
use std::time::Instant;
use tokio::sync::mpsc::{channel, error};
use libserpix_rs::*;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = channel(100);
    let mut handles = vec![];
    // Thread #1: scan WoW window
    let h = tokio::spawn(async move {
        let hwnd = win_screenshot::utils::find_window("World of Warcraft").expect("Couldn't find window");
        loop {
            read_wow(hwnd, tx.clone()).await;
        }
    });
    handles.push(h);
    // Thread #2: parse output
    let h = tokio::spawn(async move {
        loop {
            match rx.try_recv() {
                Ok(v) => {
                    let jstring = &v.to_string();
                    println!("{}",jstring);
                },
                Err(e) => {
                    match e {
                        error::TryRecvError::Disconnected => break,
                        _ => {}
                    }
                }
            }
        }
    });
    handles.push(h);
    for handle in handles {
        handle.await.expect("Thread exited");
    }
}
```
