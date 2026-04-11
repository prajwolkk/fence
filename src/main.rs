use std::env;
use std::fs::OpenOptions;
use std::io::Write;
// 1. Bring the 'chrono' time library into our code
use chrono::Local; 

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("❌ Usage: fence \"your reason here\"");
        return;
    }

    let message = &args[1];

    // 2. Get the current time and format it: Year-Month-Day Hour:Min:Sec
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("decisions.log")
        .expect("Failed to open the file");

    // 3. Save it like this: [2026-04-11 10:25:00] Your note here
    writeln!(file, "[{}] {}", timestamp, message).expect("Failed to write to the file");

    println!("🚀 Fence: Decision recorded with timestamp!");
}