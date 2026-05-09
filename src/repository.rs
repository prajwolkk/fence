use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Local};
use serde::Serialize;
use serde_json::json;

use crate::constants::*;
use crate::model::*;

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
            let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            path.push(".fence_global.log");
            path
        }
    }

    pub fn record(message: &str) -> Result<(), io::Error> {
        Self::record_with_metadata(message, DecisionCategory::General, Vec::new())
    }

    pub fn record_with_metadata(
        message: &str,
        category: DecisionCategory,
        optional_tags: Vec<String>,
    ) -> Result<(), io::Error> {
        Self::record_with_options(message, category, optional_tags, None).map(|_| ())
    }

    pub fn record_with_options(
        message: &str,
        category: DecisionCategory,
        optional_tags: Vec<String>,
        replaces: Option<String>,
    ) -> Result<Decision, io::Error> {
        Self::record_with_details(
            message,
            DecisionRecordOptions {
                category,
                optional_tags,
                replaces,
                ..DecisionRecordOptions::default()
            },
        )
    }

    pub fn record_with_details(
        message: &str,
        options: DecisionRecordOptions,
    ) -> Result<Decision, io::Error> {
        if let Some(ref old_id) = options.replaces
            && find_decision_file(old_id)?.is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Decision not found: {old_id}"),
            ));
        }

        let config = load_runtime_config();
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let author = Self::get_author();
        let branch = get_branch_name();
        let id = short_hash(&format!("{timestamp}{author}{branch}{message}"));
        let review_due = normalize_review_due(options.review_due.as_deref())?;
        let owner = options.owner.or_else(|| config.default_owner.clone());
        let reviewer = options.reviewer.or_else(|| config.default_reviewer.clone());
        let entry = Decision {
            id,
            author,
            timestamp,
            branch,
            message: message.to_string(),
            category: options.category,
            optional_tags: options.optional_tags,
            status: DecisionStatus::Accepted,
            review_due,
            supersedes: options.replaces.clone(),
            superseded_by: None,
            title: options.title,
            rationale: options.rationale,
            consequences: options.consequences,
            links: options.links,
            owner,
            reviewer,
            approved_by: None,
            approved_at: None,
        };
        write_decision_file(&entry)?;
        if let Some(old_id) = options.replaces {
            let _ = supersede_decision(&old_id, &entry.id)?;
        }

        if config.auto_export {
            export_markdown()?;
        }

        dispatch_notifications(&config, &entry);

        Ok(entry)
    }

    pub fn list() -> String {
        match read_decision_entries() {
            Ok(entries) if entries.is_empty() => "No log file found.".to_string(),
            Ok(entries) => entries
                .into_iter()
                .map(|entry| decision_summary_line(&entry))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(_) => "No log file found.".to_string(),
        }
    }

    pub fn search(keyword: &str) -> Vec<String> {
        let term = keyword.to_lowercase();
        read_decision_entries()
            .unwrap_or_default()
            .into_iter()
            .filter(|decision| {
                decision.message.to_lowercase().contains(&term)
                    || decision.author.to_lowercase().contains(&term)
                    || decision.id.to_lowercase().contains(&term)
                    || decision
                        .title
                        .as_deref()
                        .is_some_and(|title| title.to_lowercase().contains(&term))
                    || decision
                        .rationale
                        .as_deref()
                        .is_some_and(|rationale| rationale.to_lowercase().contains(&term))
                    || decision
                        .optional_tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&term))
            })
            .map(|decision| decision_summary_line(&decision))
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
        notifications: None,
        team_settings: None,
        default_owner: None,
        default_reviewer: None,
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
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(|_| ())
}

pub fn decisions_dir() -> PathBuf {
    PathBuf::from(DECISION_DIR)
}

pub fn ensure_decisions_dir() -> Result<(), io::Error> {
    fs::create_dir_all(decisions_dir())
}

pub fn write_decision_file(entry: &Decision) -> Result<(), io::Error> {
    fs::create_dir_all(decisions_dir())?;
    let file_ts = entry.timestamp.replace([':', ' '], "");
    let filename = format!("{file_ts}_{}.json", entry.id);
    let path = decisions_dir().join(filename);
    write_decision_at_path(&path, entry)?;
    Ok(())
}

pub fn write_decision_at_path(path: &Path, entry: &Decision) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(entry).map_err(io::Error::other)?;
    fs::write(path, serialized)
}

pub fn append_markdown_row(path: &Path, entry: &Decision) -> Result<(), io::Error> {
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
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
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

pub fn decision_category_label(category: DecisionCategory) -> &'static str {
    match category {
        DecisionCategory::Architecture => "Architecture",
        DecisionCategory::Technical => "Technical",
        DecisionCategory::Product => "Product",
        DecisionCategory::Security => "Security",
        DecisionCategory::General => "General",
    }
}

pub fn decision_summary_line(decision: &Decision) -> String {
    format!(
        "{}  [{}] {}  {}  ({}) {}",
        decision.id,
        decision_category_label(decision.category),
        decision_status_label(decision),
        decision.timestamp,
        decision.author,
        decision.message
    )
}

pub fn decision_detail(decision: &Decision) -> String {
    let tags = if decision.optional_tags.is_empty() {
        "-".to_string()
    } else {
        decision.optional_tags.join(", ")
    };
    let supersedes = decision.supersedes.as_deref().unwrap_or("-");
    let superseded_by = decision.superseded_by.as_deref().unwrap_or("-");
    let title = decision.title.as_deref().unwrap_or("-");
    let rationale = decision.rationale.as_deref().unwrap_or("-");
    let consequences = decision.consequences.as_deref().unwrap_or("-");
    let links = if decision.links.is_empty() {
        "-".to_string()
    } else {
        decision.links.join(", ")
    };
    let owner = decision.owner.as_deref().unwrap_or("-");
    let reviewer = decision.reviewer.as_deref().unwrap_or("-");
    let approved_by = decision.approved_by.as_deref().unwrap_or("-");
    let approved_at = decision.approved_at.as_deref().unwrap_or("-");

    format!(
        "ID: {}\nTitle: {}\nStatus: {}\nCategory: {}\nAuthor: {}\nOwner: {}\nReviewer: {}\nApproved By: {}\nApproved At: {}\nBranch: {}\nTimestamp: {}\nReview Due: {}\nTags: {}\nLinks: {}\nSupersedes: {}\nSuperseded By: {}\n\n{}\n\nRationale: {}\nConsequences: {}",
        decision.id,
        title,
        decision_status_label(decision),
        decision_category_label(decision.category),
        decision.author,
        owner,
        reviewer,
        approved_by,
        approved_at,
        if decision.branch.is_empty() {
            "-"
        } else {
            &decision.branch
        },
        decision.timestamp,
        decision.review_due,
        tags,
        links,
        supersedes,
        superseded_by,
        decision.message,
        rationale,
        consequences
    )
}

pub fn export_markdown() -> Result<(), io::Error> {
    export_markdown_from_log(Path::new(DEFAULT_DECISIONS_MD_PATH)).map(|_| ())
}

pub fn export_markdown_from_log(markdown_path: &Path) -> Result<PathBuf, io::Error> {
    let mut rewritten = String::from(DECISIONS_MD_HEADER);
    for entry in read_decision_entries()? {
        let escaped_message = escape_markdown_cell(&entry.message);
        rewritten.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            entry.timestamp,
            entry.author,
            escaped_message,
            decision_status_label(&entry)
        ));
    }

    fs::write(markdown_path, rewritten)?;
    Ok(markdown_path.to_path_buf())
}

pub fn parse_log_line(line: &str) -> Option<Decision> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(entry) = serde_json::from_str::<Decision>(trimmed) {
        return Some(entry);
    }

    if !trimmed.starts_with('[') {
        return None;
    }

    let close_bracket = trimmed.find(']')?;
    let timestamp = trimmed.get(1..close_bracket)?.to_string();
    let remainder = trimmed.get(close_bracket + 1..)?.trim();
    let (author, message) = if let Some(rest) = remainder.strip_prefix('(') {
        let close_paren = rest.find(") ")?;
        (
            rest.get(0..close_paren)?.to_string(),
            rest.get(close_paren + 2..)?.to_string(),
        )
    } else {
        (fallback_system_author(), remainder.to_string())
    };

    Some(Decision {
        id: short_hash(&format!("{timestamp}{author}{message}")),
        timestamp,
        author,
        branch: String::new(),
        message,
        category: DecisionCategory::General,
        optional_tags: Vec::new(),
        status: DecisionStatus::Accepted,
        review_due: default_review_due(),
        supersedes: None,

        superseded_by: None,
        title: None,
        rationale: None,
        consequences: None,
        links: Vec::new(),
        owner: None,
        reviewer: None,
        approved_by: None,
        approved_at: None,
    })
}

pub fn tracking_status_for_log() -> TrackingStatus {
    tracking_status_for_path(&decisions_dir())
}

pub fn tracking_status_for_markdown() -> TrackingStatus {
    tracking_status_for_path(Path::new(DEFAULT_DECISIONS_MD_PATH))
}

pub fn tracking_status_for_path(path: &Path) -> TrackingStatus {
    if !has_git_directory() {
        return TrackingStatus::Local;
    }

    if git_is_tracked(path) {
        TrackingStatus::Tracked
    } else {
        TrackingStatus::Local
    }
}

pub fn check_tracking_integrity() -> Result<(bool, TrackingStatus, TrackingStatus), io::Error> {
    let log_path = decisions_dir();
    let md_path = Path::new(DEFAULT_DECISIONS_MD_PATH);
    let log_status = tracking_status_for_path(&log_path);
    let md_status = tracking_status_for_path(md_path);

    let log_ok = match log_status {
        TrackingStatus::Tracked => git_working_matches_index(&log_path)?,
        TrackingStatus::Local => true,
    };
    let md_ok = match md_status {
        TrackingStatus::Tracked => git_working_matches_index(md_path)?,
        TrackingStatus::Local => true,
    };

    Ok((log_ok && md_ok, log_status, md_status))
}

pub fn read_log_entries() -> Result<Vec<Decision>, io::Error> {
    read_decision_entries()
}

pub fn render_site_html() -> Result<String, io::Error> {
    let entries = read_log_entries()?;
    render_site_html_for_entries_with_mode(&entries, true)
}

pub fn render_site_html_for_entries(entries: &[Decision]) -> Result<String, io::Error> {
    render_site_html_for_entries_with_mode(entries, true)
}

pub fn render_site_html_for_entries_with_mode(
    entries: &[Decision],
    writable: bool,
) -> Result<String, io::Error> {
    let data = serde_json::to_string(entries)
        .map_err(io::Error::other)?
        .replace("</", "<\\/");
    Ok(SITE_TEMPLATE.replace("__FENCE_DATA__", &data).replace(
        "__FENCE_WRITABLE__",
        if writable { "true" } else { "false" },
    ))
}

pub fn generate_site() -> Result<PathBuf, io::Error> {
    let entries = read_log_entries()?;
    let html = render_site_html_for_entries_with_mode(&entries, false)?;

    let output_dir = Path::new("fence-site");
    fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join("index.html");
    fs::write(&output_path, html)?;
    Ok(output_path)
}

pub fn write_github_workflow(path: &Path) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, GITHUB_WORKFLOW_TEMPLATE)
}

pub fn write_gitlab_ci(path: &Path) -> Result<(), io::Error> {
    fs::write(path, GITLAB_CI_TEMPLATE)
}

pub fn read_log_entries_from_path(path: &Path) -> Result<Vec<Decision>, io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };

    Ok(content
        .lines()
        .filter_map(parse_log_line)
        .collect::<Vec<_>>())
}

pub struct DecisionFile {
    pub path: PathBuf,
    pub decision: Decision,
}

pub fn read_decision_files() -> Result<Vec<DecisionFile>, io::Error> {
    let mut entries = Vec::new();
    let dir = decisions_dir();
    if !dir.exists() {
        return Ok(entries);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if let Ok(decision) = serde_json::from_str::<Decision>(&content) {
            entries.push(DecisionFile { path, decision });
        }
    }

    entries.sort_by_key(|entry| entry.decision.timestamp.clone());
    Ok(entries)
}

pub fn read_decision_entries() -> Result<Vec<Decision>, io::Error> {
    Ok(read_decision_files()?
        .into_iter()
        .map(|entry| entry.decision)
        .collect())
}

pub fn find_decision_file(id: &str) -> Result<Option<DecisionFile>, io::Error> {
    let trimmed = id.trim();
    let entries = read_decision_files()?;
    if let Some(exact) = entries.iter().find(|entry| entry.decision.id == trimmed) {
        return Ok(Some(DecisionFile {
            path: exact.path.clone(),
            decision: exact.decision.clone(),
        }));
    }

    let mut prefix_matches = entries
        .into_iter()
        .filter(|entry| entry.decision.id.starts_with(trimmed));
    let first = prefix_matches.next();
    if first.is_some() && prefix_matches.next().is_none() {
        Ok(first)
    } else {
        Ok(None)
    }
}

pub fn update_decision<F>(id: &str, updater: F) -> Result<Option<Decision>, io::Error>
where
    F: FnOnce(&mut Decision) -> Result<(), io::Error>,
{
    let Some(mut entry) = find_decision_file(id)? else {
        return Ok(None);
    };

    updater(&mut entry.decision)?;
    write_decision_at_path(&entry.path, &entry.decision)?;
    export_markdown()?;
    Ok(Some(entry.decision))
}

pub fn review_decision(id: &str, review_due: Option<&str>) -> Result<Option<Decision>, io::Error> {
    update_decision(id, |decision| {
        decision.review_due = normalize_review_due(review_due)?;
        Ok(())
    })
}

pub fn approve_decision(id: &str) -> Result<Option<Decision>, io::Error> {
    update_decision(id, |decision| {
        if matches!(
            decision.status,
            DecisionStatus::Deprecated | DecisionStatus::Superseded
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deprecated or superseded decisions cannot be approved",
            ));
        }
        decision.status = DecisionStatus::Approved;
        decision.approved_by = Some(FenceManager::get_author());
        decision.approved_at = Some(Local::now().to_rfc3339());
        Ok(())
    })
}

pub fn stale_decisions() -> Result<Vec<Decision>, io::Error> {
    Ok(read_decision_entries()?
        .into_iter()
        .filter(is_stale)
        .collect())
}

pub fn deprecate_decision(id: &str) -> Result<bool, io::Error> {
    let Some(mut entry) = find_decision_file(id)? else {
        return Ok(false);
    };
    entry.decision.status = DecisionStatus::Deprecated;
    write_decision_at_path(&entry.path, &entry.decision)?;
    export_markdown()?;
    Ok(true)
}

pub fn supersede_decision(old_id: &str, new_id: &str) -> Result<bool, io::Error> {
    let Some(mut entry) = find_decision_file(old_id)? else {
        return Ok(false);
    };
    entry.decision.status = DecisionStatus::Superseded;
    entry.decision.superseded_by = Some(new_id.to_string());
    write_decision_at_path(&entry.path, &entry.decision)?;
    Ok(true)
}

pub fn is_stale(decision: &Decision) -> bool {
    if !matches!(
        decision.status,
        DecisionStatus::Accepted | DecisionStatus::Approved
    ) {
        return false;
    }

    DateTime::parse_from_rfc3339(&decision.review_due)
        .map(|review_due| review_due.with_timezone(&Local) < Local::now())
        .unwrap_or(false)
}

pub fn decision_status_label(decision: &Decision) -> &'static str {
    match decision.status {
        DecisionStatus::Proposed => "Proposed",
        DecisionStatus::Deprecated => "Deprecated",
        DecisionStatus::Superseded => "Superseded",
        DecisionStatus::Accepted | DecisionStatus::Approved if is_stale(decision) => "Stale",
        DecisionStatus::Approved => "Approved",
        DecisionStatus::Accepted => "Accepted",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionHealthStats {
    pub healthy: usize,
    pub unhealthy: usize,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecisionStatusCounts {
    pub total: usize,
    pub healthy: usize,
    pub needs_attention: usize,
    pub accepted: usize,
    pub approved: usize,
    pub proposed: usize,
    pub stale: usize,
    pub deprecated: usize,
    pub superseded: usize,
}

pub fn decision_status_counts() -> Result<DecisionStatusCounts, io::Error> {
    let entries = read_decision_entries()?;
    let mut counts = DecisionStatusCounts {
        total: entries.len(),
        healthy: 0,
        needs_attention: 0,
        accepted: 0,
        approved: 0,
        proposed: 0,
        stale: 0,
        deprecated: 0,
        superseded: 0,
    };

    for decision in entries {
        match decision.status {
            DecisionStatus::Accepted | DecisionStatus::Approved if is_stale(&decision) => {
                counts.stale += 1;
                counts.needs_attention += 1;
            }
            DecisionStatus::Accepted => {
                counts.accepted += 1;
                counts.healthy += 1;
            }
            DecisionStatus::Approved => {
                counts.approved += 1;
                counts.healthy += 1;
            }
            DecisionStatus::Proposed => {
                counts.proposed += 1;
                counts.needs_attention += 1;
            }
            DecisionStatus::Deprecated => {
                counts.deprecated += 1;
                counts.needs_attention += 1;
            }
            DecisionStatus::Superseded => {
                counts.superseded += 1;
            }
        }
    }

    Ok(counts)
}

pub fn health_stats() -> Result<DecisionHealthStats, io::Error> {
    let entries = read_decision_entries()?;
    let mut healthy = 0usize;
    let mut unhealthy = 0usize;

    for decision in entries {
        match decision.status {
            DecisionStatus::Accepted | DecisionStatus::Approved if !is_stale(&decision) => {
                healthy += 1;
            }
            DecisionStatus::Superseded => {}
            _ => unhealthy += 1,
        }
    }

    let total = healthy + unhealthy;
    let ratio = if total == 0 {
        100.0
    } else {
        (healthy as f64 / total as f64) * 100.0
    };

    Ok(DecisionHealthStats {
        healthy,
        unhealthy,
        ratio,
    })
}

pub fn count_log_entries(path: &Path) -> Result<usize, io::Error> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        decisions_dir()
    };
    if !dir.exists() {
        return Ok(0);
    }
    Ok(fs::read_dir(dir)?.filter(|entry| entry.is_ok()).count())
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

pub struct SyncStatus {
    pub in_sync: bool,
    pub decision_count: usize,
    pub markdown_count: usize,
}

pub fn sync_status() -> Result<SyncStatus, io::Error> {
    let config = load_runtime_config();
    let decision_count = count_log_entries(Path::new(&config.log_path))?;
    let markdown_count = count_markdown_entries(Path::new(DEFAULT_DECISIONS_MD_PATH))?;

    Ok(SyncStatus {
        in_sync: decision_count == markdown_count,
        decision_count,
        markdown_count,
    })
}

pub fn check_sync() -> Result<bool, io::Error> {
    sync_status().map(|status| status.in_sync)
}

pub fn log_entry_count() -> Result<usize, io::Error> {
    let config = load_runtime_config();
    count_log_entries(Path::new(&config.log_path))
}

pub fn dispatch_notifications(config: &FenceConfig, entry: &Decision) {
    if let Some(notifications) = &config.notifications {
        if let Some(webhook_url) = notifications.webhook_url.as_deref() {
            send_webhook_notification(webhook_url, entry);
        }

        if let Some(custom_command) = notifications.custom_command.as_deref() {
            run_custom_command(custom_command, entry);
        }
    }
}

pub struct MigrationReport {
    pub scanned: usize,
    pub migrated: usize,
    pub skipped_existing: usize,
    pub ignored: usize,
}

pub fn migrate_legacy_log(path: &Path, dry_run: bool) -> Result<MigrationReport, io::Error> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(MigrationReport {
                scanned: 0,
                migrated: 0,
                skipped_existing: 0,
                ignored: 0,
            });
        }
        Err(err) => return Err(err),
    };

    let mut existing_ids = read_decision_entries()?
        .into_iter()
        .map(|decision| decision.id)
        .collect::<Vec<_>>();

    let mut report = MigrationReport {
        scanned: 0,
        migrated: 0,
        skipped_existing: 0,
        ignored: 0,
    };

    for (index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        report.scanned += 1;

        let decision =
            parse_log_line(trimmed).or_else(|| legacy_plaintext_decision(trimmed, index));
        let Some(decision) = decision else {
            report.ignored += 1;
            continue;
        };

        if existing_ids.iter().any(|id| id == &decision.id) {
            report.skipped_existing += 1;
            continue;
        }

        if !dry_run {
            write_decision_file(&decision)?;
            existing_ids.push(decision.id.clone());
        }
        report.migrated += 1;
    }

    if report.migrated > 0 && !dry_run {
        export_markdown()?;
    }

    Ok(report)
}

fn legacy_plaintext_decision(line: &str, index: usize) -> Option<Decision> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let timestamp = Local::now()
        .checked_add_signed(ChronoDuration::seconds(index as i64))
        .unwrap_or_else(Local::now)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let author = fallback_system_author();
    let id = short_hash(&format!("legacy:{index}:{trimmed}"));

    Some(Decision {
        id,
        timestamp,
        author,
        branch: String::new(),
        message: trimmed.to_string(),
        category: DecisionCategory::General,
        optional_tags: Vec::new(),
        status: DecisionStatus::Accepted,
        review_due: default_review_due(),
        supersedes: None,

        superseded_by: None,
        title: None,
        rationale: None,
        consequences: None,
        links: Vec::new(),
        owner: None,
        reviewer: None,
        approved_by: None,
        approved_at: None,
    })
}

fn git_is_tracked(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let output = Command::new("git")
        .args(["ls-files", "--error-unmatch", "--", &path_str])
        .output();

    matches!(output, Ok(output) if output.status.success())
}

fn git_working_matches_index(path: &Path) -> Result<bool, io::Error> {
    let path_str = path.to_string_lossy();
    let status = Command::new("git")
        .args(["diff", "--quiet", "--", &path_str])
        .status();

    match status {
        Ok(status) => Ok(status.success()),
        Err(err) => Err(err),
    }
}

pub fn has_git_directory() -> bool {
    Path::new(".git").exists()
}

pub fn git_remote_platform() -> Option<String> {
    let output = Command::new("git").args(["remote", "-v"]).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("github.com") {
        return Some("GitHub".to_string());
    }
    if text.contains("gitlab.com") {
        return Some("GitLab".to_string());
    }
    if text.trim().is_empty() {
        None
    } else {
        Some("Remote".to_string())
    }
}

pub fn detect_stack() -> Option<String> {
    if Path::new("Cargo.toml").exists() {
        return Some("Rust".to_string());
    }
    if Path::new("pubspec.yaml").exists() {
        return Some("Flutter".to_string());
    }
    if Path::new("package.json").exists() {
        return Some("Node".to_string());
    }
    None
}

pub fn default_monitored_paths() -> Vec<String> {
    let mut paths = Vec::new();
    if Path::new("Cargo.toml").exists() {
        paths.push("Cargo.toml".to_string());
        paths.push("src".to_string());
    }
    if Path::new("pubspec.yaml").exists() {
        paths.push("pubspec.yaml".to_string());
        paths.push("lib".to_string());
    }
    if Path::new("package.json").exists() {
        paths.push("package.json".to_string());
        paths.push("src".to_string());
    }
    paths.sort();
    paths.dedup();
    paths
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

pub fn remove_ignore_entry(path: &Path, entry: &str) -> Result<(), io::Error> {
    let normalized_entry = entry.trim();
    let existing = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    let original_count = existing.lines().count();
    let lines: Vec<&str> = existing
        .lines()
        .filter(|line| line.trim() != normalized_entry)
        .collect();

    if lines.len() == original_count {
        return Ok(());
    }

    let output = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };

    fs::write(path, output)
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
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
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

fn send_webhook_notification(webhook_url: &str, entry: &Decision) {
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

fn run_custom_command(template: &str, entry: &Decision) {
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
    use std::panic::{UnwindSafe, catch_unwind, resume_unwind};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("fence-{name}-{unique}"))
    }

    fn with_temp_cwd<T, F>(name: &str, test: F) -> T
    where
        F: FnOnce() -> T + UnwindSafe,
    {
        let _guard = TEST_MUTEX.lock().expect("should lock test mutex");
        let original_dir = std::env::current_dir().expect("should read current dir");
        let temp_dir = temp_path(name);
        fs::create_dir_all(&temp_dir).expect("should create temp cwd");
        std::env::set_current_dir(&temp_dir).expect("should switch to temp cwd");

        let result = catch_unwind(test);

        std::env::set_current_dir(original_dir).expect("should restore cwd");
        fs::remove_dir_all(temp_dir).ok();

        match result {
            Ok(value) => value,
            Err(payload) => resume_unwind(payload),
        }
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
            Some(TeamSettings { jira_domain: None }),
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
        let entry = Decision {
            id: "abc12345".to_string(),
            timestamp: "2026-04-14 12:00:00".to_string(),
            author: "praj".to_string(),
            branch: "main".to_string(),
            message: "Ship A | B test".to_string(),
            category: DecisionCategory::General,
            optional_tags: Vec::new(),
            status: DecisionStatus::Accepted,
            review_due: "2027-04-14T12:00:00+00:00".to_string(),
            supersedes: None,

            superseded_by: None,
            title: None,
            rationale: None,
            consequences: None,
            links: Vec::new(),
            owner: None,
            reviewer: None,
            approved_by: None,
            approved_at: None,
        };

        append_markdown_row(&path, &entry).expect("should append markdown row");

        let content = fs::read_to_string(&path).expect("should read markdown file");
        assert!(content.starts_with(DECISIONS_MD_HEADER));
        assert!(
            content.contains("| 2026-04-14 12:00:00 | praj | Ship A \\| B test | ✅ Decided |")
        );

        fs::remove_file(path).ok();
    }

    #[test]
    fn parse_log_line_extracts_entry() {
        let entry =
            parse_log_line("[2026-04-14 12:00:00] (praj) Ship it").expect("should parse log line");
        assert_eq!(entry.timestamp, "2026-04-14 12:00:00");
        assert_eq!(entry.author, "praj");
        assert_eq!(entry.message, "Ship it");
        assert_eq!(entry.category, DecisionCategory::General);
        assert!(entry.optional_tags.is_empty());
    }

    #[test]
    fn parse_log_line_reads_json() {
        let entry = Decision {
            id: "abc12345".to_string(),
            timestamp: "2026-04-14 12:00:00".to_string(),
            author: "praj".to_string(),
            branch: "main".to_string(),
            message: "Ship it".to_string(),
            category: DecisionCategory::Architecture,
            optional_tags: vec!["rust".to_string(), "perf".to_string()],
            status: DecisionStatus::Accepted,
            review_due: "2027-04-14T12:00:00+00:00".to_string(),
            supersedes: None,

            superseded_by: None,
            title: None,
            rationale: None,
            consequences: None,
            links: Vec::new(),
            owner: None,
            reviewer: None,
            approved_by: None,
            approved_at: None,
        };
        let line = serde_json::to_string(&entry).expect("should serialize");
        let parsed = parse_log_line(&line).expect("should parse json");
        assert_eq!(parsed, entry);
    }

    #[test]
    fn parse_log_line_reads_timestamp_without_author() {
        let entry = parse_log_line("[2026-04-14 12:00:00] Ship it").expect("should parse log line");

        assert_eq!(entry.timestamp, "2026-04-14 12:00:00");
        assert_eq!(entry.message, "Ship it");
        assert_eq!(entry.category, DecisionCategory::General);
    }

    #[test]
    fn migrate_legacy_log_converts_old_lines_to_decision_files() {
        with_temp_cwd("migrate-legacy", || {
            let legacy_path = temp_path("legacy-log");
            fs::write(
                &legacy_path,
                "Plain early decision\n[2026-04-14 12:00:00] Timestamp-only decision\n[2026-04-15 12:00:00] (praj) Attributed decision\n",
            )
            .expect("should write legacy log");

            let report =
                migrate_legacy_log(&legacy_path, false).expect("should migrate legacy log");
            let entries = read_decision_entries().expect("should read migrated entries");

            assert_eq!(report.scanned, 3);
            assert_eq!(report.migrated, 3);
            assert_eq!(entries.len(), 3);
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.message == "Plain early decision")
            );
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.message == "Timestamp-only decision")
            );
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.message == "Attributed decision")
            );

            fs::remove_file(legacy_path).ok();
        });
    }

    #[test]
    fn export_markdown_from_log_regenerates_table() {
        with_temp_cwd("export-md-cwd", || {
            let md_path = temp_path("export-md");
            fs::create_dir_all(decisions_dir()).expect("should create decisions dir");

            let first = Decision {
                id: "abc12345".to_string(),
                timestamp: "2026-04-14 12:00:00".to_string(),
                author: "praj".to_string(),
                branch: "main".to_string(),
                message: "Ship it".to_string(),
                category: DecisionCategory::General,
                optional_tags: Vec::new(),
                status: DecisionStatus::Accepted,
                review_due: "2027-04-14T12:00:00+00:00".to_string(),
                supersedes: None,

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            };
            let second = Decision {
                id: "def67890".to_string(),
                timestamp: "2026-04-15 08:00:00".to_string(),
                author: "lex".to_string(),
                branch: "main".to_string(),
                message: "Use A | B".to_string(),
                category: DecisionCategory::General,
                optional_tags: Vec::new(),
                status: DecisionStatus::Accepted,
                review_due: "2027-04-15T08:00:00+00:00".to_string(),
                supersedes: None,

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            };
            write_decision_at_path(
                &decisions_dir().join("20260414120000_abc12345.json"),
                &first,
            )
            .expect("should write decision");
            write_decision_at_path(
                &decisions_dir().join("20260415080000_def67890.json"),
                &second,
            )
            .expect("should write decision");

            export_markdown_from_log(&md_path).expect("should export markdown");

            let content = fs::read_to_string(&md_path).expect("should read markdown");
            assert!(content.starts_with(DECISIONS_MD_HEADER));
            assert!(content.contains("| 2026-04-14 12:00:00 | praj | Ship it | Accepted |"));
            assert!(content.contains("| 2026-04-15 08:00:00 | lex | Use A \\| B | Accepted |"));

            fs::remove_file(md_path).ok();
        });
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
        let temp_dir = temp_path("decision-dir");
        fs::create_dir_all(&temp_dir).expect("should create decisions dir");
        write_decision_at_path(
            &temp_dir.join("20260414120000_a1.json"),
            &Decision {
                id: "a1".to_string(),
                timestamp: "2026-04-14 12:00:00".to_string(),
                author: "praj".to_string(),
                branch: "main".to_string(),
                message: "A".to_string(),
                category: DecisionCategory::General,
                optional_tags: Vec::new(),
                status: DecisionStatus::Accepted,
                review_due: "2027-04-14T12:00:00+00:00".to_string(),
                supersedes: None,

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            },
        )
        .expect("should write decision");
        write_decision_at_path(
            &temp_dir.join("20260415120000_b1.json"),
            &Decision {
                id: "b1".to_string(),
                timestamp: "2026-04-15 12:00:00".to_string(),
                author: "praj".to_string(),
                branch: "main".to_string(),
                message: "B".to_string(),
                category: DecisionCategory::General,
                optional_tags: Vec::new(),
                status: DecisionStatus::Accepted,
                review_due: "2027-04-15T12:00:00+00:00".to_string(),
                supersedes: None,

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            },
        )
        .expect("should write decision");

        let count = count_log_entries(&temp_dir).expect("should count log entries");
        assert_eq!(count, 2);

        fs::remove_dir_all(temp_dir).ok();
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

    #[test]
    fn deprecate_decision_updates_status_and_markdown() {
        with_temp_cwd("deprecate-cwd", || {
            fs::create_dir_all(decisions_dir()).expect("should create decisions dir");
            let path = decisions_dir().join("20260414120000_deadbeef.json");
            let entry = Decision {
                id: "deadbeef".to_string(),
                timestamp: "2026-04-14 12:00:00".to_string(),
                author: "praj".to_string(),
                branch: "main".to_string(),
                message: "Legacy deployment flow".to_string(),
                category: DecisionCategory::Technical,
                optional_tags: vec!["deploy".to_string()],
                status: DecisionStatus::Accepted,
                review_due: "2027-04-14T12:00:00+00:00".to_string(),
                supersedes: None,

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            };
            write_decision_at_path(&path, &entry).expect("should write decision");

            let deprecated = deprecate_decision("deadbeef").expect("should deprecate decision");
            assert!(deprecated);

            let stored = find_decision_file("deadbeef")
                .expect("should read decision")
                .expect("decision should exist")
                .decision;
            assert_eq!(stored.status, DecisionStatus::Deprecated);

            let markdown = fs::read_to_string(DEFAULT_DECISIONS_MD_PATH)
                .expect("should rewrite markdown export");
            assert!(
                markdown.contains(
                    "| 2026-04-14 12:00:00 | praj | Legacy deployment flow | Deprecated |"
                )
            );
        });
    }

    #[test]
    fn supersede_decision_marks_old_entry_and_links_replacement() {
        with_temp_cwd("supersede-cwd", || {
            fs::create_dir_all(decisions_dir()).expect("should create decisions dir");
            let old_path = decisions_dir().join("20260414120000_old12345.json");
            let new_path = decisions_dir().join("20260415120000_new12345.json");
            let old = Decision {
                id: "old12345".to_string(),
                timestamp: "2026-04-14 12:00:00".to_string(),
                author: "praj".to_string(),
                branch: "main".to_string(),
                message: "Use legacy queue".to_string(),
                category: DecisionCategory::Architecture,
                optional_tags: vec!["queue".to_string()],
                status: DecisionStatus::Accepted,
                review_due: "2027-04-14T12:00:00+00:00".to_string(),
                supersedes: None,

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            };
            let replacement = Decision {
                id: "new12345".to_string(),
                timestamp: "2026-04-15 12:00:00".to_string(),
                author: "praj".to_string(),
                branch: "main".to_string(),
                message: "Use durable queue".to_string(),
                category: DecisionCategory::Architecture,
                optional_tags: vec!["queue".to_string(), "durable".to_string()],
                status: DecisionStatus::Accepted,
                review_due: "2027-04-15T12:00:00+00:00".to_string(),
                supersedes: Some("old12345".to_string()),

                superseded_by: None,
                title: None,
                rationale: None,
                consequences: None,
                links: Vec::new(),
                owner: None,
                reviewer: None,
                approved_by: None,
                approved_at: None,
            };
            write_decision_at_path(&old_path, &old).expect("should write old decision");
            write_decision_at_path(&new_path, &replacement).expect("should write replacement");

            let superseded =
                supersede_decision("old12345", "new12345").expect("should supersede decision");
            assert!(superseded);

            let stored = find_decision_file("old12345")
                .expect("should read decision")
                .expect("decision should exist")
                .decision;
            assert_eq!(stored.status, DecisionStatus::Superseded);
            assert_eq!(stored.superseded_by.as_deref(), Some("new12345"));
        });
    }
}
