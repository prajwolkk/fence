use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::json;

const CONFIG_FILE_NAME: &str = "fence.toml";
const DEFAULT_LOG_PATH: &str = "decisions.log";
const DEFAULT_DECISIONS_MD_PATH: &str = "DECISIONS.md";
const DECISIONS_MD_HEADER: &str = "# 🛡️ Architectural Decision Records\n\n| Date | Author | Decision | Status |\n| :--- | :--- | :--- | :--- |\n";
const PRE_COMMIT_SNIPPET: &str = "#!/bin/sh\nif ! fence check; then\n  echo \"🛡️ Fence: Commit blocked. Your documentation is out of sync.\"\n  echo \"Run 'fence log' through the CLI or 'fence export' to fix it.\"\n  exit 1\nfi\n";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FenceMode {
    Solo,
    Team,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TeamSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jira_domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NotificationProvider {
    Slack,
    Discord,
    GenericWebhook,
    CustomCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NotificationsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<NotificationProvider>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FenceConfig {
    pub project_name: String,
    pub mode: FenceMode,
    #[serde(default = "default_log_path")]
    pub log_path: String,
    #[serde(default = "default_auto_export")]
    pub auto_export: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<NotificationsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_settings: Option<TeamSettings>,
}

fn default_log_path() -> String {
    DEFAULT_LOG_PATH.to_string()
}

fn default_auto_export() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionEntry {
    pub timestamp: String,
    pub author: String,
    pub message: String,
}

impl FenceConfig {
    pub fn new(
        project_name: String,
        mode: FenceMode,
        notifications: Option<NotificationsConfig>,
        team_settings: Option<TeamSettings>,
    ) -> Self {
        Self {
            project_name,
            mode,
            log_path: default_log_path(),
            auto_export: default_auto_export(),
            notifications,
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
            Ok(out) if out.status.success() => {
                let author = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !author.is_empty() {
                    return author;
                }
                fallback_system_author()
            }
            _ => fallback_system_author(),
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

    pub fn record(message: &str) -> Result<(), io::Error> {
        let entry = DecisionEntry {
            timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            author: Self::get_author(),
            message: message.to_string(),
        };
        let config = load_runtime_config();
        let log_path = PathBuf::from(&config.log_path);

        write_raw_log(&log_path, &entry)?;

        if config.auto_export {
            append_markdown_row(Path::new(DEFAULT_DECISIONS_MD_PATH), &entry)?;
        }

        dispatch_notifications(&config, &entry);

        Ok(())
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

pub fn load_runtime_config() -> FenceConfig {
    load_config(Path::new(CONFIG_FILE_NAME)).unwrap_or_else(|_| FenceConfig {
        project_name: default_project_name(),
        mode: FenceMode::Solo,
        log_path: default_log_path(),
        auto_export: default_auto_export(),
        notifications: None,
        team_settings: None,
    })
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

pub fn write_raw_log(path: &Path, entry: &DecisionEntry) -> Result<(), io::Error> {
    ensure_log_file(path)?;

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "[{}] ({}) {}",
        entry.timestamp, entry.author, entry.message
    )
}

pub fn append_markdown_row(path: &Path, entry: &DecisionEntry) -> Result<(), io::Error> {
    ensure_markdown_header(path)?;

    let escaped_message = escape_markdown_cell(&entry.message);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "| {} | {} | {} | ✅ Decided |",
        entry.timestamp, entry.author, escaped_message
    )
}

pub fn ensure_markdown_header(path: &Path) -> Result<(), io::Error> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }

    if path.exists() {
        return Ok(());
    }

    fs::write(path, DECISIONS_MD_HEADER)
}

pub fn escape_markdown_cell(value: &str) -> String {
    value.replace('|', "\\|")
}

pub fn count_log_entries(path: &Path) -> Result<usize, io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };

    Ok(content.lines().filter(|line| !line.trim().is_empty()).count())
}

pub fn count_markdown_entries(path: &Path) -> Result<usize, io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err),
    };

    let mut count = 0;
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        if trimmed.contains("| Date | Author | Decision | Status |") {
            continue;
        }
        if trimmed.contains("| :--- | :--- | :--- | :--- |") {
            continue;
        }
        if trimmed == "|" {
            continue;
        }
        count += 1;
    }

    Ok(count)
}

pub fn check_sync() -> Result<bool, io::Error> {
    let config = load_runtime_config();
    let log_count = count_log_entries(Path::new(&config.log_path))?;
    let markdown_count = count_markdown_entries(Path::new(DEFAULT_DECISIONS_MD_PATH))?;
    Ok(log_count == markdown_count)
}

pub fn dispatch_notifications(config: &FenceConfig, entry: &DecisionEntry) {
    if let Some(notifications) = &config.notifications {
        if let Some(webhook_url) = notifications.webhook_url.as_deref() {
            send_webhook_notification(webhook_url, entry);
        }

        if let Some(custom_command) = notifications.custom_command.as_deref() {
            run_custom_command(custom_command, entry);
        }
    }
}

pub fn has_git_directory() -> bool {
    Path::new(".git").exists()
}

pub fn git_hooks_path() -> PathBuf {
    Path::new(".git").join("hooks")
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

pub fn install_pre_commit_hook(hooks_dir: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(hooks_dir)?;

    let hook_path = hooks_dir.join("pre-commit");
    fs::write(&hook_path, PRE_COMMIT_SNIPPET)?;
    ensure_hook_is_executable(&hook_path)
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

fn fallback_system_author() -> String {
    for key in ["USER", "USERNAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    let output = Command::new("whoami").output();
    match output {
        Ok(out) if out.status.success() => {
            let author = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if author.is_empty() {
                "Unknown Developer".to_string()
            } else {
                author
            }
        }
        _ => "Unknown Developer".to_string(),
    }
}

fn send_webhook_notification(webhook_url: &str, entry: &DecisionEntry) {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .build();
    let agent: ureq::Agent = config.into();

    let payload = json!({
        "author": entry.author,
        "message": entry.message,
        "timestamp": entry.timestamp,
    });

    let _ = agent.post(webhook_url).send_json(payload);
}

fn run_custom_command(template: &str, entry: &DecisionEntry) {
    let command = template
        .replace("{message}", &shell_escape(&entry.message))
        .replace("{author}", &shell_escape(&entry.author))
        .replace("{timestamp}", &shell_escape(&entry.timestamp));

    #[cfg(unix)]
    let _ = Command::new("sh").arg("-c").arg(&command).status();

    #[cfg(windows)]
    let _ = Command::new("cmd").args(["/C", &command]).status();
}

fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(unix)]
fn ensure_hook_is_executable(path: &Path) -> Result<(), io::Error> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn ensure_hook_is_executable(_path: &Path) -> Result<(), io::Error> {
    Ok(())
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
            Some(NotificationsConfig {
                provider: Some(NotificationProvider::Slack),
                webhook_url: Some("https://hooks.slack.test".to_string()),
                custom_command: None,
            }),
            Some(TeamSettings {
                jira_domain: None,
            }),
        );

        write_config(&path, &config).expect("should write config");
        let loaded = load_config(&path).expect("should load config");

        assert_eq!(loaded, config);

        fs::remove_file(path).ok();
    }

    #[test]
    fn escape_markdown_cell_escapes_pipes() {
        assert_eq!(
            escape_markdown_cell("Use A | B for rollout"),
            "Use A \\| B for rollout"
        );
    }

    #[test]
    fn append_markdown_row_creates_header_and_escaped_row() {
        let path = temp_path("decisions-md");
        let entry = DecisionEntry {
            timestamp: "2026-04-14 12:00:00".to_string(),
            author: "praj".to_string(),
            message: "Ship A | B test".to_string(),
        };

        append_markdown_row(&path, &entry).expect("should append markdown row");

        let content = fs::read_to_string(&path).expect("should read markdown file");
        assert!(content.starts_with(DECISIONS_MD_HEADER));
        assert!(content.contains("| 2026-04-14 12:00:00 | praj | Ship A \\| B test | ✅ Decided |"));

        fs::remove_file(path).ok();
    }

    #[test]
    fn install_pre_commit_hook_creates_idempotent_script() {
        let hooks_dir = temp_path("hooks");

        install_pre_commit_hook(&hooks_dir).expect("should install hook");
        install_pre_commit_hook(&hooks_dir).expect("should avoid duplicate hook block");

        let content =
            fs::read_to_string(hooks_dir.join("pre-commit")).expect("should read hook content");
        assert_eq!(content, PRE_COMMIT_SNIPPET);

        fs::remove_dir_all(hooks_dir).ok();
    }

    #[test]
    fn shell_escape_wraps_and_escapes_single_quotes() {
        assert_eq!(shell_escape("ship it"), "'ship it'");
        assert_eq!(shell_escape("it's live"), "'it'\"'\"'s live'");
    }

    #[test]
    fn count_log_entries_ignores_empty_lines() {
        let path = temp_path("log-count");
        fs::write(&path, "[a]\n\n[b]\n").expect("should write log");

        let count = count_log_entries(&path).expect("should count log entries");
        assert_eq!(count, 2);

        fs::remove_file(path).ok();
    }

    #[test]
    fn count_markdown_entries_skips_header_and_separator() {
        let path = temp_path("md-count");
        fs::write(
            &path,
            "# Header\n\n| Date | Author | Decision | Status |\n| :--- | :--- | :--- | :--- |\n| a | b | c | d |\n| e | f | g | h |\n",
        )
        .expect("should write md");

        let count = count_markdown_entries(&path).expect("should count markdown entries");
        assert_eq!(count, 2);

        fs::remove_file(path).ok();
    }
}
