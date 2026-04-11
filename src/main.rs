use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Write, BufRead, BufReader}; // Added 'BufRead' and 'BufReader' for efficient searching
use chrono::Local;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("❌ Usage: \n  fence \"message\"\n  fence --list\n  fence --search \"keyword\"");
        return;
    }

    let input = &args[1];

    // --- PHASE 8: LIST MODE ---
    if input == "--list" {
        match fs::read_to_string("decisions.log") {
            Ok(content) => {
                println!("\n📖 --- FULL LOG ---");
                print!("{}", content);
            }
            Err(_) => println!("⚠️ No log file found."),
        }
        return;
    }

    // --- NEW LOGIC FOR PHASE 9: SEARCH MODE ---
    if input == "--search" {
        if args.len() < 3 {
            println!("❌ Please provide a keyword: fence --search \"keyword\"");
            return;
        }
        let keyword = &args[2].to_lowercase(); // Search is easier when case-insensitive

        let file = fs::File::open("decisions.log").expect("Could not open log");
        let reader = BufReader::new(file);

        println!("\n🔍 --- SEARCH RESULTS FOR '{}' ---", keyword);
        for line in reader.lines() {
            let line_str = line.expect("Could not read line");
            if line_str.to_lowercase().contains(keyword) {
                println!("{}", line_str);
            }
        }
        return;
    }
    // ------------------------------------------

    // Existing "Record" logic
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("decisions.log")
        .expect("Failed to open file");

    writeln!(file, "[{}] {}", timestamp, input).expect("Failed to write to file");
    println!("🚀 Fence: Decision recorded!");
}