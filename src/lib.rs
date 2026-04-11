use std::fs::{self, OpenOptions};
use std::io::{Write, BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;
use chrono::Local;

/// The "Engine" that handles finding and writing logs
pub struct FenceManager;

impl FenceManager {
    pub fn get_author() -> String {
        let output = Command::new("git").args(["config", "user.name"]).output();
        match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            }
            _ => "Unknown Developer".to_string(),
        }
    }

    pub fn get_log_path() -> PathBuf {
        if std::path::Path::new(".git").exists() {
            PathBuf::from("decisions.log")
        } else {
            let mut path = dirs::home_dir().expect("Home dir not found");
            path.push(".fence_global.log");
            path
        }
    }

    pub fn record(message: &str) {
        let path = Self::get_log_path();
        let author = Self::get_author();
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        
        let mut file = OpenOptions::new()
            .create(true).append(true).open(&path).expect("Failed to open log");

        writeln!(file, "[{}] ({}) {}", timestamp, author, message).expect("Write failed");
    }

    pub fn list() -> String {
        let path = Self::get_log_path();
        fs::read_to_string(&path).unwrap_or_else(|_| "No log file found.".to_string())
    }

    pub fn search(keyword: &str) -> Vec<String> {
        let path = Self::get_log_path();
        let file = fs::File::open(&path).expect("Could not open log");
        let reader = BufReader::new(file);
        let term = keyword.to_lowercase();

        reader.lines()
            .map(|l| l.unwrap())
            .filter(|l| l.to_lowercase().contains(&term))
            .collect()
    }
}