use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::process::Command;

use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::constants::DEFAULT_LOG_PATH;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingStatus {
    Tracked,
    Local,
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
    #[serde(default)]
    pub monitored_paths: Vec<String>,
    #[serde(default = "default_ignored_paths")]
    pub ignored_paths: Vec<String>,
    #[serde(default)]
    pub standalone_mode: bool,
    #[serde(default)]
    pub safe_sync: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_disclaimer: Option<String>,
    #[serde(default)]
    pub sentinel_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentinel_platform: Option<String>,
    #[serde(default)]
    pub enforcement_level: EnforcementLevel,
    #[serde(default)]
    pub scoring: HashMap<String, u32>,
    #[serde(default = "default_threshold")]
    pub threshold: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<NotificationsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team_settings: Option<TeamSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reviewer: Option<String>,
}

pub(crate) fn default_log_path() -> String {
    DEFAULT_LOG_PATH.to_string()
}

pub(crate) fn default_auto_export() -> bool {
    true
}

fn default_category() -> DecisionCategory {
    DecisionCategory::General
}

fn default_status() -> DecisionStatus {
    DecisionStatus::Accepted
}

pub(crate) fn default_review_due() -> String {
    (Local::now() + ChronoDuration::days(365)).to_rfc3339()
}

pub fn normalize_review_due(value: Option<&str>) -> Result<String, io::Error> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(default_review_due());
    };

    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        return Ok(parsed.to_rfc3339());
    }

    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(format!("{date}T00:00:00+00:00"));
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "review due date must be YYYY-MM-DD or RFC3339",
    ))
}

pub(crate) fn default_threshold() -> u32 {
    10
}

pub(crate) fn short_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let hash = hasher.finish();
    let hex = format!("{:016x}", hash);
    hex.chars().take(8).collect()
}

pub(crate) fn get_branch_name() -> String {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if name.is_empty() {
                "unknown".to_string()
            } else {
                name
            }
        }
        _ => "unknown".to_string(),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum EnforcementLevel {
    Warning,
    #[default]
    Blocking,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DecisionCategory {
    Architecture,
    Technical,
    Product,
    Security,
    General,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Proposed,
    Accepted,
    Approved,
    Deprecated,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Decision {
    #[serde(default)]
    pub id: String,
    pub timestamp: String,
    pub author: String,
    #[serde(default)]
    pub branch: String,
    pub message: String,
    #[serde(default = "default_category")]
    pub category: DecisionCategory,
    #[serde(default)]
    pub optional_tags: Vec<String>,
    #[serde(default = "default_status")]
    pub status: DecisionStatus,
    #[serde(default = "default_review_due")]
    pub review_due: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consequences: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DecisionRecordOptions {
    pub category: DecisionCategory,
    pub optional_tags: Vec<String>,
    pub replaces: Option<String>,
    pub review_due: Option<String>,
    pub title: Option<String>,
    pub rationale: Option<String>,
    pub consequences: Option<String>,
    pub links: Vec<String>,
    pub owner: Option<String>,
    pub reviewer: Option<String>,
}

impl Default for DecisionRecordOptions {
    fn default() -> Self {
        Self {
            category: DecisionCategory::General,
            optional_tags: Vec::new(),
            replaces: None,
            review_due: None,
            title: None,
            rationale: None,
            consequences: None,
            links: Vec::new(),
            owner: None,
            reviewer: None,
        }
    }
}

pub fn default_ignored_paths() -> Vec<String> {
    vec![
        "target/**".to_string(),
        ".git/**".to_string(),
        "docs/assets/**".to_string(),
    ]
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
            monitored_paths: Vec::new(),
            ignored_paths: default_ignored_paths(),
            standalone_mode: false,
            safe_sync: false,
            sync_disclaimer: None,
            sentinel_enabled: false,
            sentinel_platform: None,
            enforcement_level: EnforcementLevel::Blocking,
            scoring: HashMap::new(),
            threshold: default_threshold(),
            notifications,
            team_settings,
            default_owner: None,
            default_reviewer: None,
        }
    }
}
