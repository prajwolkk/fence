use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Command;

use globset::Glob;
use serde::Serialize;

use crate::constants::DECISION_DIR;
use crate::model::FenceConfig;
use crate::repository::load_runtime_config;

fn git_head_message() -> Option<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--pretty=%B"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_base_branch() -> Option<String> {
    let candidates = [
        "refs/remotes/origin/main",
        "refs/remotes/origin/master",
        "refs/heads/main",
        "refs/heads/master",
    ];
    for candidate in candidates {
        if git_ref_exists(candidate) {
            return Some(
                candidate
                    .replace("refs/remotes/origin/", "origin/")
                    .replace("refs/heads/", ""),
            );
        }
    }
    None
}

fn git_ref_exists(reference: &str) -> bool {
    let status = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", reference])
        .status();
    matches!(status, Ok(status) if status.success())
}

fn git_diff_files(base: &str) -> Result<Vec<String>, io::Error> {
    let output = Command::new("git")
        .args(["diff", "--name-only", &format!("{base}..HEAD")])
        .output()?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect())
}

#[derive(Debug, Clone)]
struct GitDiffStat {
    path: String,
    additions: u32,
    deletions: u32,
}

fn git_diff_stats(base: &str) -> Result<Vec<GitDiffStat>, io::Error> {
    let output = Command::new("git")
        .args(["diff", "--numstat", &format!("{base}..HEAD")])
        .output()?;

    if !output.status.success() {
        return Ok(git_diff_files(base)?
            .into_iter()
            .map(|path| GitDiffStat {
                path,
                additions: 0,
                deletions: 0,
            })
            .collect());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let additions = parts.next()?.parse::<u32>().unwrap_or(0);
            let deletions = parts.next()?.parse::<u32>().unwrap_or(0);
            let path = parts.next()?.trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(GitDiffStat {
                    path,
                    additions,
                    deletions,
                })
            }
        })
        .collect())
}

fn git_diff_stats_staged() -> Result<Vec<GitDiffStat>, io::Error> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--numstat"])
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let additions = parts.next()?.parse::<u32>().unwrap_or(0);
            let deletions = parts.next()?.parse::<u32>().unwrap_or(0);
            let path = parts.next()?.trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(GitDiffStat {
                    path,
                    additions,
                    deletions,
                })
            }
        })
        .collect())
}

fn is_monitored_path(path: &str, monitored: &[String]) -> bool {
    monitored.iter().any(|entry| {
        if entry.is_empty() {
            return false;
        }
        matches_pattern(path, entry)
    })
}

fn is_ignored_path(path: &str, ignored: &[String]) -> bool {
    ignored
        .iter()
        .any(|entry| !entry.is_empty() && matches_pattern(path, entry))
}

fn weighted_score_for_file(path: &str, scoring: &HashMap<String, u32>) -> u32 {
    scoring
        .iter()
        .filter_map(|(pattern, points)| matches_pattern(path, pattern).then_some(*points))
        .max()
        .unwrap_or(0)
}

fn matches_pattern(path: &str, pattern: &str) -> bool {
    if has_glob_syntax(pattern) {
        return Glob::new(pattern)
            .map(|glob| glob.compile_matcher().is_match(path))
            .unwrap_or_else(|_| wildcard_match(path, pattern));
    }
    path == pattern || path.starts_with(&format!("{pattern}/"))
}

fn has_glob_syntax(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '*' | '?' | '[' | '{'))
}

fn wildcard_match(text: &str, pattern: &str) -> bool {
    let (mut ti, mut pi) = (0usize, 0usize);
    let (text_bytes, pattern_bytes) = (text.as_bytes(), pattern.as_bytes());
    let (mut star_idx, mut match_idx) = (None, 0usize);

    while ti < text_bytes.len() {
        if pi < pattern_bytes.len()
            && (pattern_bytes[pi] == b'?' || pattern_bytes[pi] == text_bytes[ti])
        {
            ti += 1;
            pi += 1;
        } else if pi < pattern_bytes.len() && pattern_bytes[pi] == b'*' {
            star_idx = Some(pi);
            match_idx = ti;
            pi += 1;
        } else if let Some(star) = star_idx {
            pi = star + 1;
            match_idx += 1;
            ti = match_idx;
        } else {
            return false;
        }
    }

    while pi < pattern_bytes.len() && pattern_bytes[pi] == b'*' {
        pi += 1;
    }

    pi == pattern_bytes.len()
}

#[derive(Debug, Clone, Serialize)]
pub struct SentinelChangedFile {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub points: u32,
    pub monitored: bool,
    pub ignored: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SentinelCheckResult {
    pub bypassed: bool,
    pub base: String,
    pub changed_files: usize,
    pub decision_found: bool,
    pub requires_decision: bool,
    pub threshold: u32,
    pub score: u32,
    pub missing_decision: bool,
    pub files: Vec<SentinelChangedFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigValidationReport {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn sentinel_check(base_branch: Option<String>) -> Result<SentinelCheckResult, io::Error> {
    let config = load_runtime_config();
    let monitored = config.monitored_paths.clone();
    let ignored = config.ignored_paths.clone();
    let scoring = config.scoring.clone();
    let threshold = config.threshold;

    let validation = validate_config(&config);
    if !validation.errors.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            validation.errors.join("; "),
        ));
    }

    let base = base_branch
        .or_else(git_base_branch)
        .unwrap_or_else(|| "HEAD~1".to_string());

    let latest_message = git_head_message().unwrap_or_default();
    let lower = latest_message.to_lowercase();
    if lower.contains("[skip fence]") || lower.contains("nolog") {
        return Ok(SentinelCheckResult {
            bypassed: true,
            base,
            changed_files: 0,
            decision_found: true,
            requires_decision: false,
            threshold,
            score: 0,
            missing_decision: false,
            files: Vec::new(),
        });
    }

    let raw_stats = git_diff_stats(&base)?;
    let files = raw_stats
        .into_iter()
        .map(|stat| {
            let ignored = is_ignored_path(&stat.path, &ignored);
            SentinelChangedFile {
                points: if ignored {
                    0
                } else {
                    weighted_score_for_file(&stat.path, &scoring)
                },
                monitored: !ignored && is_monitored_path(&stat.path, &monitored),
                ignored,
                path: stat.path,
                additions: stat.additions,
                deletions: stat.deletions,
            }
        })
        .collect::<Vec<_>>();
    let score = files.iter().map(|file| file.points).sum::<u32>();
    let monitored_changes = files
        .iter()
        .filter(|file| file.monitored && !file.ignored)
        .count();
    let scored_files = files
        .iter()
        .filter(|file| file.points > 0 && !file.ignored)
        .count();

    let requires_decision = if !scoring.is_empty() && threshold > 0 {
        score > threshold
    } else {
        monitored_changes > 0
    };
    let decision_found = files
        .iter()
        .any(|file| file.path.starts_with(&format!("{DECISION_DIR}/")));

    if !requires_decision {
        return Ok(SentinelCheckResult {
            bypassed: false,
            base,
            changed_files: 0,
            decision_found,
            requires_decision,
            threshold,
            score,
            missing_decision: false,
            files,
        });
    }

    let changed_files = if !scoring.is_empty() {
        scored_files
    } else {
        monitored_changes
    };

    Ok(SentinelCheckResult {
        bypassed: false,
        base,
        changed_files,
        decision_found,
        requires_decision,
        threshold,
        score,
        missing_decision: !decision_found,
        files,
    })
}

pub fn sentinel_explain(base_branch: Option<String>) -> Result<SentinelCheckResult, io::Error> {
    sentinel_check(base_branch)
}

pub fn sentinel_check_staged() -> Result<SentinelCheckResult, io::Error> {
    let config = load_runtime_config();
    let validation = validate_config(&config);
    if !validation.errors.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            validation.errors.join("; "),
        ));
    }

    let monitored = config.monitored_paths.clone();
    let ignored = config.ignored_paths.clone();
    let scoring = config.scoring.clone();
    let threshold = config.threshold;
    let raw_stats = git_diff_stats_staged()?;
    let files = raw_stats
        .into_iter()
        .map(|stat| {
            let ignored = is_ignored_path(&stat.path, &ignored);
            SentinelChangedFile {
                points: if ignored {
                    0
                } else {
                    weighted_score_for_file(&stat.path, &scoring)
                },
                monitored: !ignored && is_monitored_path(&stat.path, &monitored),
                ignored,
                path: stat.path,
                additions: stat.additions,
                deletions: stat.deletions,
            }
        })
        .collect::<Vec<_>>();
    let score = files.iter().map(|file| file.points).sum::<u32>();
    let monitored_changes = files
        .iter()
        .filter(|file| file.monitored && !file.ignored)
        .count();
    let scored_files = files
        .iter()
        .filter(|file| file.points > 0 && !file.ignored)
        .count();
    let requires_decision = if !scoring.is_empty() && threshold > 0 {
        score > threshold
    } else {
        monitored_changes > 0
    };
    let decision_found = files
        .iter()
        .any(|file| file.path.starts_with(&format!("{DECISION_DIR}/")));
    let changed_files = if requires_decision {
        if !scoring.is_empty() {
            scored_files
        } else {
            monitored_changes
        }
    } else {
        0
    };

    Ok(SentinelCheckResult {
        bypassed: false,
        base: "staged".to_string(),
        changed_files,
        decision_found,
        requires_decision,
        threshold,
        score,
        missing_decision: requires_decision && !decision_found,
        files,
    })
}

pub fn validate_config(config: &FenceConfig) -> ConfigValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if config.monitored_paths.is_empty() && config.scoring.is_empty() {
        warnings.push("no monitored_paths or scoring rules configured".to_string());
    }

    for path in &config.monitored_paths {
        if path.trim().is_empty() {
            errors.push("monitored_paths contains an empty entry".to_string());
            continue;
        }
        if has_glob_syntax(path) {
            if let Err(err) = Glob::new(path) {
                errors.push(format!("invalid monitored path glob '{path}': {err}"));
            }
        } else if !Path::new(path).exists() {
            warnings.push(format!("monitored path '{path}' does not exist yet"));
        }
    }

    for path in &config.ignored_paths {
        if path.trim().is_empty() {
            errors.push("ignored_paths contains an empty entry".to_string());
            continue;
        }
        if has_glob_syntax(path)
            && let Err(err) = Glob::new(path)
        {
            errors.push(format!("invalid ignored path glob '{path}': {err}"));
        }
    }

    for (pattern, points) in &config.scoring {
        if pattern.trim().is_empty() {
            errors.push("scoring contains an empty pattern".to_string());
        }
        if *points == 0 {
            warnings.push(format!("scoring pattern '{pattern}' has zero points"));
        }
        if has_glob_syntax(pattern)
            && let Err(err) = Glob::new(pattern)
        {
            errors.push(format!("invalid scoring glob '{pattern}': {err}"));
        }
    }

    ConfigValidationReport {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}
