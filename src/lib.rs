use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Local;
use serde::{Deserialize, Serialize};

const CONFIG_FILE_NAME: &str = "fence.toml";
const DEFAULT_LOG_PATH: &str = "decisions.log";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FenceMode {
    Solo,
    Team,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TeamSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack_webhook: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FenceConfig {
    pub project_name: String,
    pub mode: FenceMode,
    #[serde(default = "default_log_path")]
    pub log_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_settings: Option<TeamSettings>,
}

fn default_log_path() -> String {
    DEFAULT_LOG_PATH.to_string()
}

impl FenceConfig {
    pub fn new(
        project_name: String,
        mode: FenceMode,
        team_settings: Option<TeamSettings>,
    ) -> Self {
        Self {
            project_name,
            mode,
            log_path: default_log_path(),
            team_settings,
        }
    }
}

/// The "Engine" that handles finding and writing logs.
pub struct FenceManager;

impl FenceManager {
    pub fn get_author() -> String {
        let output = Command::new("git").args(["config", "user.name"]).output();
        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
            _ => "Unknown Developer".to_string(),
        }
    }

    pub fn get_log_path() -> PathBuf {
        if let Ok(config) = load_config(Path::new(CONFIG_FILE_NAME)) {
            return PathBuf::from(config.log_path);
        }

        if Path::new(".git").exists() {
            PathBuf::from(DEFAULT_LOG_PATH)
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
            .create(true)
            .append(true)
            .open(&path)
            .expect("Failed to open log");

        writeln!(file, "[{}] ({}) {}", timestamp, author, message).expect("Write failed");
    }

    pub fn list() -> String {
        let path = Self::get_log_path();
        fs::read_to_string(&path).unwrap_or_else(|_| "No log file found.".to_string())
    }

    pub fn search(keyword: &str) -> Vec<String> {
        let path = Self::get_log_path();
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);
        let term = keyword.to_lowercase();

        reader
            .lines()
            .map_while(Result::ok)
            .filter(|line| line.to_lowercase().contains(&term))
            .collect()
    }
}

pub fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE_NAME)
}

pub fn load_config(path: &Path) -> Result<FenceConfig, io::Error> {
    let content = fs::read_to_string(path)?;
    toml::from_str(&content).map_err(io::Error::other)
}

pub fn write_config(path: &Path, config: &FenceConfig) -> Result<(), io::Error> {
    let serialized = toml::to_string_pretty(config).map_err(io::Error::other)?;
    fs::write(path, format!("{serialized}\n"))
}

pub fn ensure_log_file(path: &Path) -> Result<(), io::Error> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(|_| ())
}

pub fn has_git_directory() -> bool {
    Path::new(".git").exists()
}

pub fn ensure_gitignore_contains(entry: &str) -> Result<(), io::Error> {
    ensure_ignore_entry(Path::new(".gitignore"), entry)
}

pub fn ensure_ignore_entry(path: &Path, entry: &str) -> Result<(), io::Error> {
    let normalized_entry = entry.trim();
    let existing = fs::read_to_string(path).unwrap_or_default();

    if existing.lines().any(|line| line.trim() == normalized_entry) {
        return Ok(());
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file)?;
    }

    writeln!(file, "{normalized_entry}")
}

pub fn default_project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "fence-project".to_string())
}

pub fn sanitize_project_name(name: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_separator = false;

    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_') {
            sanitized.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            sanitized.push('-');
            last_was_separator = true;
        }
    }

    let sanitized = sanitized.trim_matches([' ', '-']).trim().to_string();

    if sanitized.is_empty() {
        "fence-project".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("fence-{name}-{unique}"))
    }

    #[test]
    fn sanitize_project_name_replaces_invalid_characters() {
        assert_eq!(sanitize_project_name("My/Project"), "My-Project");
        assert_eq!(sanitize_project_name("   "), "fence-project");
    }

    #[test]
    fn ensure_ignore_entry_creates_missing_file() {
        let path = temp_path("gitignore");

        ensure_ignore_entry(&path, "decisions.log").expect("should write ignore entry");

        let content = fs::read_to_string(&path).expect("should read created file");
        assert_eq!(content, "decisions.log\n");

        fs::remove_file(path).ok();
    }

    #[test]
    fn ensure_ignore_entry_does_not_duplicate_entries() {
        let path = temp_path("gitignore-dedup");
        fs::write(&path, "target\n").expect("should seed file");

        ensure_ignore_entry(&path, "decisions.log").expect("should append new entry");
        ensure_ignore_entry(&path, "decisions.log").expect("should avoid duplicate");

        let content = fs::read_to_string(&path).expect("should read file");
        assert_eq!(content, "target\ndecisions.log\n");

        fs::remove_file(path).ok();
    }

    #[test]
    fn write_and_load_config_round_trip() {
        let path = temp_path("config");
        let config = FenceConfig::new(
            "Fence".to_string(),
            FenceMode::Team,
            Some(TeamSettings {
                slack_webhook: Some("https://hooks.slack.test".to_string()),
                jira_domain: None,
            }),
        );

        write_config(&path, &config).expect("should write config");
        let loaded = load_config(&path).expect("should load config");

        assert_eq!(loaded, config);

        fs::remove_file(path).ok();
    }
}
