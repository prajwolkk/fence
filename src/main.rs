use std::collections::HashMap;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{self, Command as ProcessCommand};

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use dialoguer::{Confirm, Input, Select};
use fence::{
    DecisionRecordOptions, FenceConfig, FenceManager, FenceMode, NotificationProvider,
    NotificationsConfig, TeamSettings, config_path, default_project_name, ensure_decisions_dir,
    ensure_gitignore_contains, git_hooks_path, git_remote_platform, has_git_directory,
    install_pre_commit_hook, remove_ignore_entry, sanitize_project_name,
};
use serde::Serialize;

mod tui;

#[derive(Parser)]
#[command(name = "fence", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    Init {
        #[arg(long)]
        yes: bool,
        #[arg(long, conflicts_with = "solo")]
        team: bool,
        #[arg(long, conflicts_with = "team")]
        solo: bool,
    },
    Log {
        message: String,
        #[arg(short, long)]
        category: Option<String>,
        #[arg(short, long)]
        tags: Option<String>,
        #[arg(long)]
        replaces: Option<String>,
        #[arg(long)]
        review_due: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        rationale: Option<String>,
        #[arg(long)]
        consequences: Option<String>,
        #[arg(long = "link")]
        links: Vec<String>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        reviewer: Option<String>,
    },
    Amend,
    Edit {
        id: String,
    },
    Review {
        id: String,
        #[arg(long)]
        review_due: Option<String>,
    },
    Deprecate {
        id: String,
    },
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
    List {
        #[arg(long)]
        json: bool,
    },
    Search {
        keyword: String,
    },
    Check,
    Export,
    Migrate {
        #[arg(long, default_value = "decisions.log")]
        from: String,
        #[arg(long)]
        dry_run: bool,
    },
    Browse,
    Site,
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
        #[arg(short, long, default_value_t = 7878)]
        port: u16,
        #[arg(long)]
        open: bool,
    },
    Open {
        #[arg(long, default_value = "127.0.0.1")]
        host: IpAddr,
        #[arg(short, long, default_value_t = 7878)]
        port: u16,
    },
    Stats {
        #[arg(long)]
        json: bool,
    },
    Stale {
        #[arg(long)]
        json: bool,
    },
    Doctor,
    Sentinel {
        #[command(subcommand)]
        command: SentinelCommands,
    },
    Completions {
        shell: Shell,
    },
    Badge,
}

#[derive(Subcommand)]
enum SentinelCommands {
    Init,
    Check {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Explain {
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Validate {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { yes, team, solo } => run_init(InitOptions { yes, team, solo })?,
        Commands::Log {
            message,
            category,
            tags,
            replaces,
            review_due,
            title,
            rationale,
            consequences,
            links,
            owner,
            reviewer,
        } => {
            let decision = FenceManager::record_with_details(
                &message,
                DecisionRecordOptions {
                    category: parse_category(category),
                    optional_tags: parse_tags(tags),
                    replaces,
                    review_due,
                    title,
                    rationale,
                    consequences,
                    links,
                    owner,
                    reviewer,
                },
            )?;
            println!(
                "🚀 Decision recorded: {}. DECISIONS.md updated.",
                decision.id
            );
        }
        Commands::Amend => {
            run_amend()?;
        }
        Commands::Edit { id } => {
            run_edit(&id)?;
        }
        Commands::Review { id, review_due } => {
            if let Some(decision) = fence::review_decision(&id, review_due.as_deref())? {
                println!(
                    "Decision {} reviewed. Next review due: {}",
                    decision.id, decision.review_due
                );
            } else {
                println!("Decision not found or ID prefix is ambiguous: {id}");
                process::exit(1);
            }
        }
        Commands::Deprecate { id } => {
            if fence::deprecate_decision(&id)? {
                println!("Decision deprecated.");
            } else {
                println!("Decision not found: {id}");
                process::exit(1);
            }
        }
        Commands::Show { id, json } => {
            if let Some(entry) = fence::find_decision_file(&id)? {
                if json {
                    print_json(&entry.decision)?;
                } else {
                    println!("{}", fence::decision_detail(&entry.decision));
                }
            } else {
                println!("Decision not found or ID prefix is ambiguous: {id}");
                process::exit(1);
            }
        }
        Commands::List { json } => {
            if json {
                print_json(&fence::read_log_entries()?)?;
            } else {
                println!("\n📖 --- DECISION HISTORY ---");
                println!("{}", FenceManager::list());
            }
        }
        Commands::Search { keyword } => {
            let results = FenceManager::search(&keyword);
            println!("\n🔍 --- SEARCH RESULTS ---");
            for line in results {
                println!("{line}");
            }
        }
        Commands::Check => {
            let sync = fence::sync_status()?;
            let (tracking_ok, log_status, md_status) = fence::check_tracking_integrity()?;
            if !sync.in_sync || !tracking_ok {
                if !sync.in_sync {
                    println!(
                        "Sync Error: DECISIONS.md has {} rows, but Fence has {} decisions. Run 'fence export' to fix it.",
                        sync.markdown_count, sync.decision_count
                    );
                }
                if !tracking_ok {
                    println!(
                        "Tracking Error: tracked files are out of sync with the staged versions."
                    );
                }
                println!(
                    "Status: Log={} MD={}",
                    tracking_label(log_status),
                    tracking_label(md_status)
                );
                process::exit(1);
            }
        }
        Commands::Export => {
            fence::export_markdown()?;
        }
        Commands::Migrate { from, dry_run } => {
            let report = fence::migrate_legacy_log(Path::new(&from), dry_run)?;
            let verb = if dry_run { "Would migrate" } else { "Migrated" };
            println!("{verb} {} legacy decisions from {from}.", report.migrated);
            println!(
                "Scanned: {}  Skipped existing: {}  Ignored: {}",
                report.scanned, report.skipped_existing, report.ignored
            );
        }
        Commands::Browse => {
            tui::run_browse()?;
        }
        Commands::Site => {
            let path = fence::generate_site()?;
            println!("Generated site at {}", path.display());
        }
        Commands::Serve { host, port, open } => {
            run_serve(host, port, open)?;
        }
        Commands::Open { host, port } => {
            run_serve(host, port, true)?;
        }
        Commands::Stats { json } => {
            if json {
                print_json(&fence::decision_status_counts()?)?;
            } else {
                let stats = fence::health_stats()?;
                let count = fence::log_entry_count()?;
                println!("Decisions: {count}");
                println!("Healthy: {}", stats.healthy);
                println!("Needs attention: {}", stats.unhealthy);
                println!("Health Ratio: {:.1}%", stats.ratio);
            }
        }
        Commands::Stale { json } => {
            let stale = fence::stale_decisions()?;
            if json {
                print_json(&stale)?;
            } else if stale.is_empty() {
                println!("No stale decisions.");
            } else {
                for decision in stale {
                    println!("{}", fence::decision_summary_line(&decision));
                }
            }
        }
        Commands::Doctor => {
            run_doctor()?;
        }
        Commands::Sentinel { command } => match command {
            SentinelCommands::Init => {
                run_sentinel_init()?;
            }
            SentinelCommands::Check { base, json } => {
                if !has_git_directory() {
                    println!("The Sentinel requires a Git repository. Please run git init first.");
                    process::exit(1);
                }
                let result = fence::sentinel_check(base)?;
                let enforcement = fence::load_runtime_config().enforcement_level;
                if json {
                    print_json(&result)?;
                    if result.missing_decision && enforcement == fence::EnforcementLevel::Blocking {
                        process::exit(1);
                    }
                    return Ok(());
                }
                print_sentinel_report(&result);
                if result.missing_decision {
                    match enforcement {
                        fence::EnforcementLevel::Warning => {
                            println!(
                                "WARNING: Architectural change detected without a decision. Enforcement level is Warning."
                            );
                        }
                        fence::EnforcementLevel::Blocking => {
                            println!(
                                "Run `fence log \"why this change is intentional\"` and commit the generated .fence/decisions file."
                            );
                            process::exit(1);
                        }
                    }
                }
            }
            SentinelCommands::Explain { base, json } => {
                let result = fence::sentinel_explain(base)?;
                if json {
                    print_json(&result)?;
                } else {
                    print_sentinel_report(&result);
                }
            }
            SentinelCommands::Validate { json } => {
                let report = fence::validate_config(&fence::load_runtime_config());
                if json {
                    print_json(&report)?;
                } else {
                    print_config_validation(&report);
                }
                if !report.valid {
                    process::exit(1);
                }
            }
        },
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            clap_complete::generate(shell, &mut command, "fence", &mut std::io::stdout());
        }
        Commands::Badge => {
            let count = fence::log_entry_count()?;
            let snippet = format!(
                "![Fence Decisions](https://img.shields.io/badge/decisions-{}-blue)",
                count
            );
            println!("{snippet}");
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct InitOptions {
    yes: bool,
    team: bool,
    solo: bool,
}

fn run_init(options: InitOptions) -> Result<(), Box<dyn Error>> {
    if options.yes {
        return run_init_noninteractive(options);
    }

    let config_path = config_path();

    if config_path.exists() {
        let overwrite = Confirm::new()
            .with_prompt("Fence is already initialized. Overwrite config?")
            .default(false)
            .interact()?;

        if !overwrite {
            println!("Initialization aborted.");
            return Ok(());
        }
    }

    let default_name = default_project_name();
    let requested_name: String = Input::new()
        .with_prompt("Project Name")
        .default(default_name)
        .interact_text()?;
    let project_name = sanitize_project_name(&requested_name);

    if project_name != requested_name.trim() {
        println!("Using sanitized project name: {project_name}");
    }

    let mode_index = if options.team {
        1
    } else if options.solo {
        0
    } else {
        Select::new()
            .with_prompt("Fence Mode")
            .items(["Solo (Local/Personal)", "Team (Shared/Collaborative)"])
            .default(0)
            .interact()?
    };

    let (mode, notifications, team_settings) = if mode_index == 1 {
        let provider_index = Select::new()
            .with_prompt("Notification Provider")
            .items(["Slack", "Discord", "Generic Webhook", "Custom Command"])
            .default(0)
            .interact()?;

        let notifications = match provider_index {
            0 => prompt_webhook_provider(NotificationProvider::Slack)?,
            1 => prompt_webhook_provider(NotificationProvider::Discord)?,
            2 => prompt_webhook_provider(NotificationProvider::GenericWebhook)?,
            _ => prompt_custom_command_provider()?,
        };

        let team_settings = TeamSettings { jira_domain: None };

        (FenceMode::Team, notifications, Some(team_settings))
    } else {
        (FenceMode::Solo, None, None)
    };

    let mut config = FenceConfig::new(project_name, mode, notifications, team_settings);
    let detected_stack = fence::detect_stack();
    let suggested_paths = fence::default_monitored_paths();
    let suggested_text = if suggested_paths.is_empty() {
        "".to_string()
    } else {
        suggested_paths.join(",")
    };
    let monitored_input: String = Input::new()
        .with_prompt("Monitored paths (comma-separated)")
        .default(suggested_text)
        .interact_text()?;
    config.monitored_paths = parse_tags(Some(monitored_input));
    config.scoring = default_scoring_for_stack(detected_stack.as_deref());
    ensure_decisions_dir()?;

    let git_present = has_git_directory();
    if !git_present {
        println!("Note: Not a git repository. Fence works best with Git.");
        config.standalone_mode = true;
        config.safe_sync = false;
        config.sync_disclaimer =
            Some("Standalone mode: sync integrity is not guaranteed without Git.".to_string());
    }

    if git_present {
        config.standalone_mode = false;
        config.safe_sync = true;

        let track_log = Confirm::new()
            .with_prompt("Track .fence/decisions in Git?")
            .default(true)
            .interact()?;
        let track_md = Confirm::new()
            .with_prompt("Track DECISIONS.md in Git?")
            .default(true)
            .interact()?;

        if track_log {
            remove_ignore_entry(Path::new(".gitignore"), ".fence/decisions")?;
        } else {
            ensure_gitignore_contains(".fence/decisions")?;
        }
        if track_md {
            remove_ignore_entry(Path::new(".gitignore"), "DECISIONS.md")?;
        } else {
            ensure_gitignore_contains("DECISIONS.md")?;
        }
    }

    if git_present {
        if let Some(platform) = git_remote_platform() {
            let stack_label = fence::detect_stack().unwrap_or_else(|| "Unknown".to_string());
            let setup_sentinel = Confirm::new()
                .with_prompt(format!(
                    "I detected a {stack_label} project on {platform}. Enable Sentinel CI/CD automation? (y/N)"
                ))
                .default(false)
                .interact()?;
            config.sentinel_enabled = setup_sentinel;
            if setup_sentinel {
                config.sentinel_platform = Some(platform.clone());
                maybe_write_ci_template(&platform)?;
            }
        }

        let hooks_dir = git_hooks_path();
        if hooks_dir.is_dir() {
            let install_hook = Confirm::new()
                .with_prompt("Install Git pre-commit hook to automate documentation sync?")
                .default(false)
                .interact()?;

            if install_hook {
                install_pre_commit_hook(&hooks_dir)?;
            }
        }
    }

    fence::write_config(&config_path, &config)?;

    println!("🛡️ Fence initialized! Your intent is now trackable.");
    println!("Run fence log 'your message' to start.");

    Ok(())
}

fn run_init_noninteractive(options: InitOptions) -> Result<(), Box<dyn Error>> {
    let project_name = sanitize_project_name(&default_project_name());
    let mode = if options.team {
        FenceMode::Team
    } else {
        FenceMode::Solo
    };
    let team_settings = (mode == FenceMode::Team).then_some(TeamSettings { jira_domain: None });
    let mut config = FenceConfig::new(project_name, mode, None, team_settings);
    config.monitored_paths = fence::default_monitored_paths();
    config.scoring = default_scoring_for_stack(fence::detect_stack().as_deref());
    config.ignored_paths = fence::default_ignored_paths();

    ensure_decisions_dir()?;
    if has_git_directory() {
        config.standalone_mode = false;
        config.safe_sync = true;
        remove_ignore_entry(Path::new(".gitignore"), ".fence/decisions")?;
        remove_ignore_entry(Path::new(".gitignore"), "DECISIONS.md")?;
    } else {
        config.standalone_mode = true;
        config.safe_sync = false;
        config.sync_disclaimer =
            Some("Standalone mode: sync integrity is not guaranteed without Git.".to_string());
    }

    fence::write_config(&config_path(), &config)?;
    fence::export_markdown()?;

    println!("Fence initialized non-interactively.");
    println!("Mode: {:?}", config.mode);
    println!("Monitored paths: {}", display_list(&config.monitored_paths));
    println!("Run `fence log \"your decision\"` to start.");

    Ok(())
}

fn optional_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn prompt_webhook_provider(
    provider: NotificationProvider,
) -> Result<Option<NotificationsConfig>, Box<dyn Error>> {
    let webhook_url: String = Input::new()
        .with_prompt("Webhook URL")
        .allow_empty(true)
        .interact_text()?;

    Ok(Some(NotificationsConfig {
        provider: Some(provider),
        webhook_url: optional_value(webhook_url),
        custom_command: None,
    }))
}

fn prompt_custom_command_provider() -> Result<Option<NotificationsConfig>, Box<dyn Error>> {
    let custom_command: String = Input::new()
        .with_prompt("Custom Command")
        .allow_empty(true)
        .interact_text()?;

    Ok(Some(NotificationsConfig {
        provider: Some(NotificationProvider::CustomCommand),
        webhook_url: None,
        custom_command: optional_value(custom_command),
    }))
}

fn run_amend() -> Result<(), Box<dyn Error>> {
    let mut files = fence::read_decision_files()?;
    if files.is_empty() {
        println!("No decisions to amend.");
        return Ok(());
    }
    let last = files.pop().expect("last decision");
    let mut decision = last.decision;
    let current_author = FenceManager::get_author();
    if decision.author != current_author {
        let proceed = Confirm::new()
            .with_prompt("Author mismatch. Amend anyway?")
            .default(false)
            .interact()?;
        if !proceed {
            println!("Amend aborted.");
            return Ok(());
        }
    }

    let message: String = Input::new()
        .with_prompt("Decision message")
        .default(decision.message.clone())
        .interact_text()?;

    let category_index = category_index(decision.category);
    let category_choice = Select::new()
        .with_prompt("Category")
        .items(category_options())
        .default(category_index)
        .interact()?;

    let tags_default = if decision.optional_tags.is_empty() {
        "".to_string()
    } else {
        decision.optional_tags.join(",")
    };
    let tags_input: String = Input::new()
        .with_prompt("Tags (comma-separated)")
        .default(tags_default)
        .interact_text()?;

    decision.message = message;
    decision.category = category_from_index(category_choice);
    decision.optional_tags = parse_tags(Some(tags_input));

    fence::write_decision_at_path(&last.path, &decision)?;
    fence::export_markdown()?;
    println!("Decision amended.");

    Ok(())
}

fn run_edit(id: &str) -> Result<(), Box<dyn Error>> {
    let Some(entry) = fence::find_decision_file(id)? else {
        println!("Decision not found or ID prefix is ambiguous: {id}");
        process::exit(1);
    };
    let original = entry.decision;

    let title: String = Input::new()
        .with_prompt("Title")
        .allow_empty(true)
        .default(original.title.clone().unwrap_or_default())
        .interact_text()?;
    let message: String = Input::new()
        .with_prompt("Decision message")
        .default(original.message.clone())
        .interact_text()?;
    let rationale: String = Input::new()
        .with_prompt("Rationale")
        .allow_empty(true)
        .default(original.rationale.clone().unwrap_or_default())
        .interact_text()?;
    let consequences: String = Input::new()
        .with_prompt("Consequences")
        .allow_empty(true)
        .default(original.consequences.clone().unwrap_or_default())
        .interact_text()?;
    let category_choice = Select::new()
        .with_prompt("Category")
        .items(category_options())
        .default(category_index(original.category))
        .interact()?;
    let tags_input: String = Input::new()
        .with_prompt("Tags (comma-separated)")
        .allow_empty(true)
        .default(original.optional_tags.join(","))
        .interact_text()?;
    let links_input: String = Input::new()
        .with_prompt("Links (comma-separated)")
        .allow_empty(true)
        .default(original.links.join(","))
        .interact_text()?;
    let owner: String = Input::new()
        .with_prompt("Owner")
        .allow_empty(true)
        .default(original.owner.clone().unwrap_or_default())
        .interact_text()?;
    let reviewer: String = Input::new()
        .with_prompt("Reviewer")
        .allow_empty(true)
        .default(original.reviewer.clone().unwrap_or_default())
        .interact_text()?;
    let review_due: String = Input::new()
        .with_prompt("Review due")
        .allow_empty(true)
        .default(original.review_due.clone())
        .interact_text()?;

    let edited = fence::update_decision(id, |decision| {
        decision.title = optional_value(title);
        decision.message = message;
        decision.rationale = optional_value(rationale);
        decision.consequences = optional_value(consequences);
        decision.category = category_from_index(category_choice);
        decision.optional_tags = parse_tags(Some(tags_input));
        decision.links = parse_tags(Some(links_input));
        decision.owner = optional_value(owner);
        decision.reviewer = optional_value(reviewer);
        decision.review_due = fence::normalize_review_due(Some(&review_due))?;
        Ok(())
    })?;

    if let Some(decision) = edited {
        println!("Decision {} updated.", decision.id);
    }

    Ok(())
}

fn parse_category(value: Option<String>) -> fence::DecisionCategory {
    let normalized = value.unwrap_or_else(|| "gen".to_string()).to_lowercase();

    match normalized.as_str() {
        "arch" | "architecture" => fence::DecisionCategory::Architecture,
        "tech" | "technical" => fence::DecisionCategory::Technical,
        "prod" | "product" => fence::DecisionCategory::Product,
        "sec" | "security" => fence::DecisionCategory::Security,
        "gen" | "general" => fence::DecisionCategory::General,
        _ => fence::DecisionCategory::General,
    }
}

fn parse_tags(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_string())
        .collect()
}

fn default_scoring_for_stack(stack: Option<&str>) -> HashMap<String, u32> {
    let mut scoring = HashMap::new();
    match stack {
        Some("Rust") => {
            scoring.insert("Cargo.toml".to_string(), 10);
            scoring.insert("src/**/*.rs".to_string(), 2);
        }
        Some("Flutter") => {
            scoring.insert("pubspec.yaml".to_string(), 10);
            scoring.insert("lib/**".to_string(), 2);
        }
        Some("Node") => {
            scoring.insert("package.json".to_string(), 10);
            scoring.insert("src/**".to_string(), 2);
        }
        _ => {}
    }
    scoring
}

fn maybe_write_ci_template(platform: &str) -> Result<(), Box<dyn Error>> {
    match platform {
        "GitHub" => {
            let path = Path::new(".github").join("workflows").join("fence.yml");
            if path.exists() {
                let overwrite = Confirm::new()
                    .with_prompt("fence.yml already exists. Overwrite?")
                    .default(false)
                    .interact()?;
                if !overwrite {
                    return Ok(());
                }
            }
            fence::write_github_workflow(&path)?;
        }
        "GitLab" => {
            let path = Path::new(".gitlab-ci.yml");
            if path.exists() {
                let overwrite = Confirm::new()
                    .with_prompt(".gitlab-ci.yml already exists. Overwrite?")
                    .default(false)
                    .interact()?;
                if !overwrite {
                    return Ok(());
                }
            }
            fence::write_gitlab_ci(path)?;
        }
        _ => {}
    }
    Ok(())
}

fn run_sentinel_init() -> Result<(), Box<dyn Error>> {
    if !has_git_directory() {
        println!("The Sentinel requires a Git repository. Please run git init first.");
        process::exit(1);
    }

    let detected_platform = git_remote_platform().unwrap_or_else(|| "GitHub".to_string());
    let platform = if detected_platform == "GitLab" {
        "GitLab".to_string()
    } else {
        "GitHub".to_string()
    };

    let mut config = if config_path().exists() {
        fence::load_runtime_config()
    } else {
        FenceConfig::new(default_project_name(), FenceMode::Solo, None, None)
    };

    if config.monitored_paths.is_empty() {
        config.monitored_paths = fence::default_monitored_paths();
    }
    if config.scoring.is_empty() {
        config.scoring = default_scoring_for_stack(fence::detect_stack().as_deref());
    }
    config.sentinel_enabled = true;
    config.sentinel_platform = Some(platform.clone());

    ensure_decisions_dir()?;
    maybe_write_ci_template(&platform)?;
    fence::write_config(&config_path(), &config)?;

    println!("Sentinel enabled for {platform}.");
    println!("Monitored paths: {}", display_list(&config.monitored_paths));
    println!("Run `fence sentinel check` to test it locally.");

    Ok(())
}

fn run_doctor() -> Result<(), Box<dyn Error>> {
    let mut issues = 0usize;
    let config_exists = config_path().exists();
    print_check(
        "Config",
        config_exists,
        "fence.toml found",
        "run `fence init`",
    );
    if !config_exists {
        issues += 1;
    }

    let git_present = has_git_directory();
    print_check("Git", git_present, ".git directory found", "run `git init`");
    if !git_present {
        issues += 1;
    }

    let decision_count = fence::log_entry_count()?;
    let legacy_exists = Path::new("decisions.log").exists();
    let legacy_needs_migration = legacy_exists && decision_count == 0;
    print_check(
        "Decision Store",
        !legacy_needs_migration,
        &format!("{decision_count} structured decisions"),
        "legacy decisions.log exists; run `fence migrate`",
    );
    if legacy_needs_migration {
        issues += 1;
    }

    let sync = fence::sync_status()?;
    print_check(
        "Markdown Export",
        sync.in_sync,
        "DECISIONS.md is in sync",
        &format!(
            "DECISIONS.md has {} rows; Fence has {} decisions. Run `fence export`",
            sync.markdown_count, sync.decision_count
        ),
    );
    if !sync.in_sync {
        issues += 1;
    }

    let (tracking_ok, log_status, md_status) = fence::check_tracking_integrity()?;
    print_check(
        "Git Tracking",
        tracking_ok,
        &format!(
            ".fence/decisions={} DECISIONS.md={}",
            tracking_label(log_status),
            tracking_label(md_status)
        ),
        "tracked decision files have unstaged changes",
    );
    if !tracking_ok {
        issues += 1;
    }

    let hook_path = git_hooks_path().join("pre-commit");
    print_optional(
        "Pre-commit Hook",
        hook_path.exists(),
        "pre-commit hook installed",
        "optional: run `fence init` and enable the hook",
    );

    if issues > 0 {
        println!("\nFence doctor found {issues} issue(s).");
        process::exit(1);
    }

    println!("\nFence doctor found no launch-blocking issues.");
    Ok(())
}

fn run_serve(host: IpAddr, port: u16, open_browser: bool) -> Result<(), Box<dyn Error>> {
    let bind_host = if host.is_unspecified() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        host
    };
    let listener = TcpListener::bind(SocketAddr::new(bind_host, port))?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}");

    println!("Fence UI running at {url}");
    println!("Press Ctrl+C to stop.");
    if open_browser {
        open_url(&url);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle_http_request(stream) {
                    eprintln!("Request failed: {err}");
                }
            }
            Err(err) => eprintln!("Connection failed: {err}"),
        }
    }

    Ok(())
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let command = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "start", url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let command = ("xdg-open", vec![url]);

    let _ = ProcessCommand::new(command.0).args(command.1).spawn();
}

fn handle_http_request(mut stream: TcpStream) -> Result<(), Box<dyn Error>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    let (status, content_type, body) = match path {
        "/" | "/index.html" => (
            "HTTP/1.1 200 OK",
            "text/html; charset=utf-8",
            fence::render_site_html()?,
        ),
        "/api/decisions" => (
            "HTTP/1.1 200 OK",
            "application/json; charset=utf-8",
            serde_json::to_string(&fence::read_log_entries()?)?,
        ),
        "/api/stats" => (
            "HTTP/1.1 200 OK",
            "application/json; charset=utf-8",
            serde_json::to_string(&fence::decision_status_counts()?)?,
        ),
        "/health" => (
            "HTTP/1.1 200 OK",
            "application/json; charset=utf-8",
            "{\"status\":\"ok\"}".to_string(),
        ),
        _ => (
            "HTTP/1.1 404 Not Found",
            "text/plain; charset=utf-8",
            "Not found".to_string(),
        ),
    };

    let response = format!(
        "{status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

fn print_json<T: Serialize>(value: &T) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_sentinel_report(result: &fence::SentinelCheckResult) {
    if result.bypassed {
        println!("✅ Sentinel bypassed for latest commit.");
        return;
    }

    let relevant = result
        .files
        .iter()
        .filter(|file| !file.ignored && (file.monitored || file.points > 0))
        .collect::<Vec<_>>();

    if relevant.is_empty() {
        println!("✅ No monitored changes detected.");
        return;
    }

    println!("Changed architectural files:");
    for file in relevant {
        if file.points > 0 {
            if file.deletions > 0 {
                println!(
                    "- {} (+{}, -{}, score {})",
                    file.path, file.additions, file.deletions, file.points
                );
            } else {
                println!(
                    "- {} (+{}, score {})",
                    file.path, file.additions, file.points
                );
            }
        } else if file.deletions > 0 {
            println!("- {} (+{}, -{})", file.path, file.additions, file.deletions);
        } else {
            println!("- {} (+{})", file.path, file.additions);
        }
    }
    println!();
    println!("Required score: >{}", result.threshold);
    println!("Current score: {}", result.score);

    if !result.requires_decision {
        println!("Decision: not required");
    } else if result.decision_found {
        println!("Decision: found");
    } else {
        println!("Missing: .fence/decisions change");
    }
}

fn print_config_validation(report: &fence::ConfigValidationReport) {
    if report.valid {
        println!("Config validation: ok");
    } else {
        println!("Config validation: failed");
    }

    for error in &report.errors {
        println!("Error: {error}");
    }
    for warning in &report.warnings {
        println!("Warning: {warning}");
    }
}

fn print_check(label: &str, ok: bool, success: &str, failure: &str) {
    let status = if ok { "ok" } else { "fix" };
    let message = if ok { success } else { failure };
    println!("{label}: {status} - {message}");
}

fn print_optional(label: &str, ok: bool, success: &str, fallback: &str) {
    let status = if ok { "ok" } else { "optional" };
    let message = if ok { success } else { fallback };
    println!("{label}: {status} - {message}");
}

fn tracking_label(status: fence::TrackingStatus) -> &'static str {
    match status {
        fence::TrackingStatus::Tracked => "Tracked",
        fence::TrackingStatus::Local => "Local",
    }
}

fn display_list(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn category_options() -> [&'static str; 5] {
    [
        "Architecture",
        "Technical",
        "Product",
        "Security",
        "General",
    ]
}

fn category_index(category: fence::DecisionCategory) -> usize {
    match category {
        fence::DecisionCategory::Architecture => 0,
        fence::DecisionCategory::Technical => 1,
        fence::DecisionCategory::Product => 2,
        fence::DecisionCategory::Security => 3,
        fence::DecisionCategory::General => 4,
    }
}

fn category_from_index(index: usize) -> fence::DecisionCategory {
    match index {
        0 => fence::DecisionCategory::Architecture,
        1 => fence::DecisionCategory::Technical,
        2 => fence::DecisionCategory::Product,
        3 => fence::DecisionCategory::Security,
        _ => fence::DecisionCategory::General,
    }
}
