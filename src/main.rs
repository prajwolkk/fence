use std::env;
use std::fs::{self, OpenOptions}; // Added 'fs' to read files
use std::io::Write;
use chrono::Local;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("❌ Usage: \n  fence \"message\" (to record)\n  fence --list (to view)");
        return;
    }

    let input = &args[1];

    // --- NEW LOGIC FOR PHASE 8 ---
    if input == "--list" {
        // Read the file content
        match fs::read_to_string("decisions.log") {
            Ok(content) => {
                println!("\n📖 --- YOUR DECISION LOG ---");
                println!("{}", content);
                println!("----------------------------\n");
            }
            Err(_) => println!("⚠️ No log file found yet. Record something first!"),
        }
        return; // Stop here so we don't try to record "--list" as a decision
    }
    // -----------------------------

    // The existing "Record" logic
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("decisions.log")
        .expect("Failed to open file");

    writeln!(file, "[{}] {}", timestamp, input).expect("Failed to write to file");
    println!("🚀 Fence: Decision recorded!");
}