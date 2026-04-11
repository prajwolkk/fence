use std::env; // To read terminal arguments
use std::fs::OpenOptions; // To open files
use std::io::Write; // To write text

fn main() {
    // Collect what you typed: ["target/debug/fence", "Used", "Redis", "..."]
    let args: Vec<String> = env::args().collect();

    // Check if you actually wrote a message
    if args.len() < 2 {
        println!("❌ Usage: fence \"your reason here\"");
        return;
    }

    // Grab the actual message
    let message = &args[1];

    // Open (or create) the log file in "Append" mode
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("decisions.log")
        .expect("Failed to open the file");

    // Write the message + a newline
    writeln!(file, "{}", message).expect("Failed to write to the file");

    println!("🚀 Fence: Decision recorded!");
}