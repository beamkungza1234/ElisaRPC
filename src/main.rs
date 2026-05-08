use discord_rpc_client::Client;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::thread;
use std::{
    io::Error,
    process::{Command, Stdio},
};


enum PlayerEvent {
    Status(String),
    Metadata(Value),
}

fn main() -> Result<(), Error> {
    let mut drpc = Client::new(1230850847345348669);
    drpc.on_ready(|_ctx| {
        println!("Discord RPC Ready...");
    });
    drpc.start();

    let (tx, rx) = mpsc::channel();
    let tx_status = tx.clone();
    let tx_meta = tx.clone();

    // Watching Start/Stop/Pause status
    thread::spawn(move || {
        let stdout = Command::new("playerctl")
            .args(["-p", "elisa", "-F", "status"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
            .stdout
            .unwrap();
        
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(status) = line {
                let _ = tx_status.send(PlayerEvent::Status(status));
            }
        }
    });

    // Detect thumbnail and name
    thread::spawn(move || {
        let format_str = r#"{"length":"{{duration(mpris:length)}}","trackid":"{{mpris:trackid}}","title":"{{xesam:title}}","artUrl":"{{mpris:artUrl}}"}"#;
        let stdout = Command::new("playerctl")
            .args(["-p", "elisa", "-F", "metadata", "--format", format_str])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
            .stdout
            .unwrap();
        
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(cap) = line {
                if let Ok(metadata) = serde_json::from_str::<Value>(&cap) {
                    // Send Metadata
                    let _ = tx_meta.send(PlayerEvent::Metadata(metadata));
                }
            }
        }
    });

    // Instant RPC Status Update
    let mut current_status = String::from("Playing");
    let mut current_title = String::from("Unknown");
    let mut current_art_url = String::from("");

    // Wait for status data
    for event in rx {
        match event {
            PlayerEvent::Status(s) => {
                current_status = s;
            }
            PlayerEvent::Metadata(m) => {
                current_title = m["title"].as_str().unwrap_or("Unknown").to_string();
                current_art_url = m["artUrl"].as_str().unwrap_or("").to_string();
            }
        }

        println!("Updating -> Status: {}, Title: {}", current_status, current_title);

        // Update Discord RPC Status
        drpc.set_activity(|act| {
            act.state(format!("Listening to {}", current_title))
                .details(&current_status)
                .assets(|ass| {
                    let mut a = ass.small_image("elisalogo").small_text("Elisa Music Player");
                    
                    if current_art_url.starts_with("http") {
                        a = a.large_image(&current_art_url);
                    } else {
                        a = a.large_image("elisalogo"); 
                    }
                    a
                })
        })
        .expect("Could not set activity");
    }

    Ok(())
}