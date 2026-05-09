use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
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

use crate::serve;
use crate::tui;

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
        id: Option<String>,
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        message: Option<String>,
        #[arg(short, long)]
        category: Option<String>,
        #[arg(short, long)]
        tags: Option<String>,
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
    Review {
        id: String,
        #[arg(long)]
        review_due: Option<String>,
    },
    Deprecate {
        id: Option<String>,
        #[arg(long)]
        search: Option<String>,
    },
    Approve {
        id: Option<String>,
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        json: bool,
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
    Pick {
        keyword: String,
        #[arg(long)]
        json: bool,
    },
    Ask {
        query: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        json: bool,
    },
    AgentCheck {
        #[arg(long, conflicts_with = "staged")]
        base: Option<String>,
        #[arg(long)]
        staged: bool,
        #[arg(long, conflicts_with = "markdown")]
        json: bool,
        #[arg(long)]
        markdown: bool,
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
    Owners {
        #[arg(long)]
        json: bool,
    },
    ReviewDue {
        #[arg(long)]
        json: bool,
    },
    Team {
        #[command(subcommand)]
        command: TeamCommands,
    },
    Doctor,
    Sentinel {
        #[command(subcommand)]
        command: SentinelCommands,
    },
    Completions {
        shell: Shell,
    },
    Demo {
        #[arg(long, default_value = "fence-demo")]
        path: PathBuf,
        #[arg(long)]
        force: bool,
    },
    Badge,
}

#[derive(Subcommand)]
enum TeamCommands {
    Status {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum SentinelCommands {
    Init {
        #[arg(long)]
        yes: bool,
        #[arg(long, conflicts_with = "gitlab")]
        github: bool,
        #[arg(long, conflicts_with = "github")]
        gitlab: bool,
    },
    Check {
        #[arg(long)]
        base: Option<String>,
        #[arg(long, conflicts_with = "markdown")]
        json: bool,
        #[arg(long)]
        markdown: bool,
    },
    Explain {
        #[arg(long)]
        base: Option<String>,
        #[arg(long, conflicts_with = "markdown")]
        json: bool,
        #[arg(long)]
        markdown: bool,
    },
    Validate {
        #[arg(long)]
        json: bool,
    },
}

pub fn run() -> Result<(), Box<dyn Error>> {
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
        Commands::Edit {
            id,
            search,
            message,
            category,
            tags,
            review_due,
            title,
            rationale,
            consequences,
            links,
            owner,
            reviewer,
        } => {
            run_edit(EditOptions {
                id,
                search,
                message,
                category,
                tags,
                review_due,
                title,
                rationale,
                consequences,
                links,
                owner,
                reviewer,
            })?;
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
        Commands::Deprecate { id, search } => {
            let id = resolve_decision_id(id, search)?;
            if fence::deprecate_decision(&id)? {
                println!("Decision {id} deprecated.");
            } else {
                println!("Decision not found: {id}");
                process::exit(1);
            }
        }
        Commands::Approve { id, search, json } => {
            let id = resolve_decision_id(id, search)?;
            match fence::approve_decision(&id) {
                Ok(Some(decision)) => {
                    if json {
                        print_json(&decision)?;
                    } else {
                        let approver = decision.approved_by.as_deref().unwrap_or("unknown");
                        println!("Decision {} approved by {}.", decision.id, approver);
                    }
                }
                Ok(None) => {
                    println!("Decision not found or ID prefix is ambiguous: {id}");
                    process::exit(1);
                }
                Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {
                    println!("Decision {id} cannot be approved: {err}");
                    process::exit(1);
                }
                Err(err) => return Err(Box::new(err)),
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
        Commands::Pick { keyword, json } => {
            let results = matching_decisions(&keyword)?;
            if json {
                print_json(&results)?;
            } else if results.is_empty() {
                println!("No decisions matched: {keyword}");
            } else {
                println!("Matching decisions:");
                print_decision_candidates(&results);
            }
        }
        Commands::Ask { query, limit, json } => {
            let results = ask_decisions(&query, limit)?;
            if json {
                print_json(&results)?;
            } else {
                print_ask_results(&query, &results);
            }
        }
        Commands::AgentCheck {
            base,
            staged,
            json,
            markdown,
        } => {
            run_agent_check(base, staged, json, markdown)?;
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
            serve::run_serve(host, port, open)?;
        }
        Commands::Open { host, port } => {
            serve::run_serve(host, port, true)?;
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
        Commands::Owners { json } => {
            let summary = owner_summaries()?;
            if json {
                print_json(&summary)?;
            } else {
                print_owner_summaries(&summary);
            }
        }
        Commands::ReviewDue { json } => {
            let due = review_due_entries()?;
            if json {
                print_json(&due)?;
            } else if due.is_empty() {
                println!("No overdue reviews.");
            } else {
                println!("Overdue reviews:");
                for decision in due {
                    println!("{}", fence::decision_summary_line(&decision));
                }
            }
        }
        Commands::Team { command } => match command {
            TeamCommands::Status { json } => {
                let status = team_status()?;
                if json {
                    print_json(&status)?;
                } else {
                    print_team_status(&status);
                }
            }
        },
        Commands::Doctor => {
            run_doctor()?;
        }
        Commands::Sentinel { command } => match command {
            SentinelCommands::Init {
                yes,
                github,
                gitlab,
            } => {
                run_sentinel_init(SentinelInitOptions {
                    yes,
                    github,
                    gitlab,
                })?;
            }
            SentinelCommands::Check {
                base,
                json,
                markdown,
            } => {
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
                if markdown {
                    println!("{}", sentinel_markdown_report(&result));
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
            SentinelCommands::Explain {
                base,
                json,
                markdown,
            } => {
                let result = fence::sentinel_explain(base)?;
                if json {
                    print_json(&result)?;
                } else if markdown {
                    println!("{}", sentinel_markdown_report(&result));
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
            print_completions(shell)?;
        }
        Commands::Demo { path, force } => {
            run_demo(&path, force)?;
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
                maybe_write_ci_template(&platform, false)?;
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

#[derive(Debug)]
struct EditOptions {
    id: Option<String>,
    search: Option<String>,
    message: Option<String>,
    category: Option<String>,
    tags: Option<String>,
    review_due: Option<String>,
    title: Option<String>,
    rationale: Option<String>,
    consequences: Option<String>,
    links: Vec<String>,
    owner: Option<String>,
    reviewer: Option<String>,
}

impl EditOptions {
    fn is_noninteractive(&self) -> bool {
        self.message.is_some()
            || self.category.is_some()
            || self.tags.is_some()
            || self.review_due.is_some()
            || self.title.is_some()
            || self.rationale.is_some()
            || self.consequences.is_some()
            || !self.links.is_empty()
            || self.owner.is_some()
            || self.reviewer.is_some()
    }
}

fn run_edit(options: EditOptions) -> Result<(), Box<dyn Error>> {
    let id = resolve_decision_id(options.id.clone(), options.search.clone())?;
    if options.is_noninteractive() {
        return run_edit_noninteractive(id, options);
    }

    run_edit_interactive(&id)
}

fn run_edit_noninteractive(id: String, options: EditOptions) -> Result<(), Box<dyn Error>> {
    let edited = fence::update_decision(&id, |decision| {
        if let Some(title) = options.title {
            decision.title = optional_value(title);
        }
        if let Some(message) = options.message {
            decision.message = message;
        }
        if let Some(rationale) = options.rationale {
            decision.rationale = optional_value(rationale);
        }
        if let Some(consequences) = options.consequences {
            decision.consequences = optional_value(consequences);
        }
        if let Some(category) = options.category {
            decision.category = parse_category(Some(category));
        }
        if let Some(tags) = options.tags {
            decision.optional_tags = parse_tags(Some(tags));
        }
        if !options.links.is_empty() {
            decision.links = options.links;
        }
        if let Some(owner) = options.owner {
            decision.owner = optional_value(owner);
        }
        if let Some(reviewer) = options.reviewer {
            decision.reviewer = optional_value(reviewer);
        }
        if let Some(review_due) = options.review_due {
            decision.review_due = fence::normalize_review_due(Some(&review_due))?;
        }
        Ok(())
    })?;

    if let Some(decision) = edited {
        println!("Decision {} updated.", decision.id);
    } else {
        println!("Decision not found or ID prefix is ambiguous: {id}");
        process::exit(1);
    }

    Ok(())
}

fn run_edit_interactive(id: &str) -> Result<(), Box<dyn Error>> {
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

#[derive(Debug, Serialize)]
struct AskDecisionResult {
    id: String,
    score: usize,
    title: Option<String>,
    message: String,
    category: String,
    status: String,
    author: String,
    owner: Option<String>,
    reviewer: Option<String>,
    review_due: String,
    tags: Vec<String>,
    rationale: Option<String>,
    consequences: Option<String>,
    links: Vec<String>,
}

fn ask_decisions(query: &str, limit: usize) -> Result<Vec<AskDecisionResult>, Box<dyn Error>> {
    let tokens = query
        .split(|ch: char| !ch.is_alphanumeric() && ch != '@' && ch != '_' && ch != '-')
        .map(|token| token.to_lowercase())
        .filter(|token| token.len() > 1)
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = fence::read_log_entries()?
        .into_iter()
        .filter_map(|decision| {
            let haystack = decision_search_text(&decision);
            let score = tokens
                .iter()
                .map(|token| haystack.matches(token).count())
                .sum::<usize>();
            let category = fence::decision_category_label(decision.category).to_string();
            let status = fence::decision_status_label(&decision).to_string();
            (score > 0).then_some(AskDecisionResult {
                id: decision.id,
                score,
                title: decision.title,
                message: decision.message,
                category,
                status,
                author: decision.author,
                owner: decision.owner,
                reviewer: decision.reviewer,
                review_due: decision.review_due,
                tags: decision.optional_tags,
                rationale: decision.rationale,
                consequences: decision.consequences,
                links: decision.links,
            })
        })
        .collect::<Vec<_>>();

    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.review_due.cmp(&left.review_due))
    });
    results.truncate(limit.max(1));
    Ok(results)
}

fn decision_search_text(decision: &fence::Decision) -> String {
    format!(
        "{} {} {} {} {} {} {} {} {} {}",
        decision.id,
        decision.title.as_deref().unwrap_or_default(),
        decision.message,
        decision.rationale.as_deref().unwrap_or_default(),
        decision.consequences.as_deref().unwrap_or_default(),
        decision.author,
        decision.owner.as_deref().unwrap_or_default(),
        decision.reviewer.as_deref().unwrap_or_default(),
        decision.optional_tags.join(" "),
        decision.links.join(" ")
    )
    .to_lowercase()
}

fn matching_decisions(keyword: &str) -> Result<Vec<fence::Decision>, Box<dyn Error>> {
    let term = keyword.trim().to_lowercase();
    if term.is_empty() {
        return Ok(Vec::new());
    }

    let mut matches = fence::read_log_entries()?
        .into_iter()
        .filter(|decision| decision_search_text(decision).contains(&term))
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
    Ok(matches)
}

fn resolve_decision_id(
    id: Option<String>,
    search: Option<String>,
) -> Result<String, Box<dyn Error>> {
    match (id, search) {
        (Some(_), Some(_)) => {
            println!("Use either a decision ID or --search, not both.");
            process::exit(1);
        }
        (Some(id), None) => Ok(id),
        (None, Some(query)) => {
            let matches = matching_decisions(&query)?;
            match matches.as_slice() {
                [] => {
                    println!("No decisions matched: {query}");
                    process::exit(1);
                }
                [decision] => Ok(decision.id.clone()),
                _ => {
                    println!("Search matched multiple decisions. Pick one ID:");
                    print_decision_candidates(&matches);
                    process::exit(1);
                }
            }
        }
        (None, None) => {
            println!("Provide a decision ID or use --search <keyword>.");
            process::exit(1);
        }
    }
}

fn print_decision_candidates(decisions: &[fence::Decision]) {
    for decision in decisions {
        println!("{}", fence::decision_summary_line(decision));
    }
}

#[derive(Debug, Serialize)]
struct OwnerSummary {
    owner: String,
    total: usize,
    needs_review: usize,
    missing_reviewer: usize,
    decisions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ReviewerSummary {
    reviewer: String,
    total: usize,
    needs_review: usize,
    decisions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TeamStatusSummary {
    total: usize,
    healthy: usize,
    needs_attention: usize,
    unowned: usize,
    missing_reviewer: usize,
    overdue_reviews: usize,
    owners: Vec<OwnerSummary>,
    reviewers: Vec<ReviewerSummary>,
}

fn owner_summaries() -> Result<Vec<OwnerSummary>, Box<dyn Error>> {
    let mut grouped: HashMap<String, Vec<fence::Decision>> = HashMap::new();
    for decision in fence::read_log_entries()? {
        let owner = decision
            .owner
            .clone()
            .filter(|owner| !owner.trim().is_empty())
            .unwrap_or_else(|| "(unowned)".to_string());
        grouped.entry(owner).or_default().push(decision);
    }

    let mut summaries = grouped
        .into_iter()
        .map(|(owner, decisions)| {
            let needs_review = decisions
                .iter()
                .filter(|decision| fence::is_stale(decision))
                .count();
            let missing_reviewer = decisions
                .iter()
                .filter(|decision| decision.reviewer.as_deref().unwrap_or("").trim().is_empty())
                .count();
            let decision_ids = decisions
                .iter()
                .map(|decision| decision.id.clone())
                .collect::<Vec<_>>();
            OwnerSummary {
                owner,
                total: decisions.len(),
                needs_review,
                missing_reviewer,
                decisions: decision_ids,
            }
        })
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| {
        left.owner
            .cmp(&right.owner)
            .then_with(|| right.needs_review.cmp(&left.needs_review))
    });
    Ok(summaries)
}

fn reviewer_summaries() -> Result<Vec<ReviewerSummary>, Box<dyn Error>> {
    let mut grouped: HashMap<String, Vec<fence::Decision>> = HashMap::new();
    for decision in fence::read_log_entries()? {
        let reviewer = decision
            .reviewer
            .clone()
            .filter(|reviewer| !reviewer.trim().is_empty())
            .unwrap_or_else(|| "(missing reviewer)".to_string());
        grouped.entry(reviewer).or_default().push(decision);
    }

    let mut summaries = grouped
        .into_iter()
        .map(|(reviewer, decisions)| {
            let needs_review = decisions
                .iter()
                .filter(|decision| fence::is_stale(decision))
                .count();
            let decision_ids = decisions
                .iter()
                .map(|decision| decision.id.clone())
                .collect::<Vec<_>>();
            ReviewerSummary {
                reviewer,
                total: decisions.len(),
                needs_review,
                decisions: decision_ids,
            }
        })
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| {
        left.reviewer
            .cmp(&right.reviewer)
            .then_with(|| right.needs_review.cmp(&left.needs_review))
    });
    Ok(summaries)
}

fn review_due_entries() -> Result<Vec<fence::Decision>, Box<dyn Error>> {
    let mut due = fence::read_log_entries()?
        .into_iter()
        .filter(fence::is_stale)
        .collect::<Vec<_>>();
    due.sort_by(|left, right| {
        left.owner
            .as_deref()
            .unwrap_or("")
            .cmp(right.owner.as_deref().unwrap_or(""))
            .then_with(|| left.review_due.cmp(&right.review_due))
    });
    Ok(due)
}

fn team_status() -> Result<TeamStatusSummary, Box<dyn Error>> {
    let entries = fence::read_log_entries()?;
    let counts = fence::decision_status_counts()?;
    let unowned = entries
        .iter()
        .filter(|decision| decision.owner.as_deref().unwrap_or("").trim().is_empty())
        .count();
    let missing_reviewer = entries
        .iter()
        .filter(|decision| decision.reviewer.as_deref().unwrap_or("").trim().is_empty())
        .count();
    let overdue_reviews = entries
        .iter()
        .filter(|decision| fence::is_stale(decision))
        .count();

    Ok(TeamStatusSummary {
        total: counts.total,
        healthy: counts.healthy,
        needs_attention: counts.needs_attention,
        unowned,
        missing_reviewer,
        overdue_reviews,
        owners: owner_summaries()?,
        reviewers: reviewer_summaries()?,
    })
}

fn print_owner_summaries(summaries: &[OwnerSummary]) {
    if summaries.is_empty() {
        println!("No decisions yet.");
        return;
    }

    println!("Decision owners:");
    for summary in summaries {
        println!(
            "{}  decisions={}  overdue={}  missing_reviewer={}  ids={}",
            summary.owner,
            summary.total,
            summary.needs_review,
            summary.missing_reviewer,
            display_list(&summary.decisions)
        );
    }
}

fn print_team_status(status: &TeamStatusSummary) {
    println!("Team decision status");
    println!("Total decisions: {}", status.total);
    println!("Healthy: {}", status.healthy);
    println!("Needs attention: {}", status.needs_attention);
    println!("Unowned: {}", status.unowned);
    println!("Missing reviewer: {}", status.missing_reviewer);
    println!("Overdue reviews: {}", status.overdue_reviews);
    println!();
    print_owner_summaries(&status.owners);
    println!();
    print_reviewer_summaries(&status.reviewers);
}

fn print_reviewer_summaries(summaries: &[ReviewerSummary]) {
    if summaries.is_empty() {
        return;
    }

    println!("Decision reviewers:");
    for summary in summaries {
        println!(
            "{}  decisions={}  overdue={}  ids={}",
            summary.reviewer,
            summary.total,
            summary.needs_review,
            display_list(&summary.decisions)
        );
    }
}

fn print_ask_results(query: &str, results: &[AskDecisionResult]) {
    if results.is_empty() {
        println!("No matching decisions for: {query}");
        println!("Try `fence search <keyword>` or record intent with `fence log`.");
        return;
    }

    println!("Architectural memory results for: {query}");
    for result in results {
        let title = result.title.as_deref().unwrap_or(&result.message);
        println!();
        println!(
            "{}  [{}] {}  score {}",
            result.id, result.category, result.status, result.score
        );
        println!("{title}");
        println!("Author: {}", result.author);
        if let Some(owner) = &result.owner {
            println!("Owner: {owner}");
        }
        if let Some(rationale) = &result.rationale {
            println!("Rationale: {rationale}");
        }
    }
}

fn run_agent_check(
    base: Option<String>,
    staged: bool,
    json: bool,
    markdown: bool,
) -> Result<(), Box<dyn Error>> {
    if !has_git_directory() {
        println!("Agent check requires a Git repository. Please run git init first.");
        process::exit(1);
    }

    let result = if staged {
        fence::sentinel_check_staged()?
    } else {
        fence::sentinel_check(base)?
    };

    if json {
        print_json(&result)?;
    } else if markdown {
        println!("{}", sentinel_markdown_report(&result));
    } else {
        print_sentinel_report(&result);
        if result.missing_decision {
            println!();
            println!(
                "Agent preflight blocked: inspect existing decisions with `fence ask <topic>` or record intent with `fence log`."
            );
        } else {
            println!();
            println!("Agent preflight passed.");
        }
    }

    if result.missing_decision {
        process::exit(1);
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

fn maybe_write_ci_template(platform: &str, overwrite_existing: bool) -> Result<(), Box<dyn Error>> {
    match platform {
        "GitHub" => {
            let path = Path::new(".github").join("workflows").join("fence.yml");
            if path.exists() && !overwrite_existing {
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
            if path.exists() && !overwrite_existing {
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

#[derive(Debug, Clone, Copy)]
struct SentinelInitOptions {
    yes: bool,
    github: bool,
    gitlab: bool,
}

fn run_sentinel_init(options: SentinelInitOptions) -> Result<(), Box<dyn Error>> {
    if !has_git_directory() {
        println!("The Sentinel requires a Git repository. Please run git init first.");
        process::exit(1);
    }

    let detected_platform = if options.github {
        "GitHub".to_string()
    } else if options.gitlab {
        "GitLab".to_string()
    } else {
        git_remote_platform().unwrap_or_else(|| "GitHub".to_string())
    };
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
    maybe_write_ci_template(&platform, options.yes)?;
    fence::write_config(&config_path(), &config)?;

    println!("Sentinel enabled for {platform}.");
    println!("Monitored paths: {}", display_list(&config.monitored_paths));
    println!("Run `fence sentinel check` to test it locally.");

    Ok(())
}

fn run_demo(path: &Path, force: bool) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        if !force {
            println!(
                "{} already exists. Re-run with `--force` to replace it.",
                path.display()
            );
            process::exit(1);
        }
        fs::remove_dir_all(path)?;
    }

    fs::create_dir_all(path.join("src"))?;
    fs::create_dir_all(path.join(".fence/decisions"))?;
    fs::write(path.join(".gitignore"), "target/\n")?;
    fs::write(
        path.join("README.md"),
        "# Fence Demo\n\nThis repo is intentionally prepared to show Sentinel blocking an architectural change without a decision.\n\nRun:\n\n```sh\nfence sentinel check --base HEAD~1\nfence log \"Adopt Tokio runtime for async background jobs\" --title \"Tokio runtime\" --rationale \"Background workers need a maintained async runtime\" --consequences \"Runtime upgrades become part of platform maintenance\" --review-due 2026-12-31 --owner @platform --reviewer @security\n```\n",
    )?;
    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"fence-demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
    )?;
    fs::write(
        path.join("src/lib.rs"),
        "pub fn runtime_name() -> &'static str {\n    \"std\"\n}\n",
    )?;
    fs::write(
        path.join("fence.toml"),
        "project_name = \"fence-demo\"\nmode = \"Team\"\nlog_path = \".fence/decisions\"\nauto_export = true\nmonitored_paths = [\"Cargo.toml\", \"src\"]\nignored_paths = [\"target/**\", \".git/**\"]\nstandalone_mode = false\nsafe_sync = true\nsentinel_enabled = true\nsentinel_platform = \"GitHub\"\nenforcement_level = \"Blocking\"\nthreshold = 10\n\n[scoring]\n\"Cargo.toml\" = 10\n\"src/**/*.rs\" = 2\n",
    )?;
    fs::write(
        path.join("DECISIONS.md"),
        "# Architectural Decision Records\n\n| Date | Author | Decision | Status |\n| :--- | :--- | :--- | :--- |\n",
    )?;

    run_demo_git(path, &["init"])?;
    run_demo_git(path, &["config", "user.name", "Fence Demo"])?;
    run_demo_git(path, &["config", "user.email", "demo@fence.local"])?;
    run_demo_git(path, &["add", "."])?;
    run_demo_git(path, &["commit", "-m", "Initial demo service"])?;

    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"fence-demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\ntokio = { version = \"1\", features = [\"rt-multi-thread\"] }\n",
    )?;
    fs::write(
        path.join("src/lib.rs"),
        "pub fn runtime_name() -> &'static str {\n    \"tokio\"\n}\n\npub fn worker_threads() -> usize {\n    4\n}\n",
    )?;
    run_demo_git(path, &["add", "Cargo.toml", "src/lib.rs"])?;
    run_demo_git(path, &["commit", "-m", "Change runtime dependency"])?;

    println!("Fence demo repo created at {}", path.display());
    println!();
    println!("Try the viral flow:");
    println!("  cd {}", path.display());
    println!("  fence sentinel check --base HEAD~1");
    println!("  fence sentinel check --base HEAD~1 --markdown");
    println!(
        "  fence log \"Adopt Tokio runtime for async background jobs\" --title \"Tokio runtime\" --rationale \"Background workers need a maintained async runtime\" --consequences \"Runtime upgrades become part of platform maintenance\" --review-due 2026-12-31 --owner @platform --reviewer @security"
    );
    println!("  git add .fence/decisions DECISIONS.md");
    println!("  git commit -m \"Record runtime decision\"");
    println!("  fence sentinel check --base HEAD~2");
    println!("  fence serve --open");

    Ok(())
}

fn run_demo_git(path: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(path)
        .output()?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("git {} failed: {stderr}", args.join(" ")).into())
}

fn run_doctor() -> Result<(), Box<dyn Error>> {
    let mut issues = 0usize;
    let config_exists = config_path().exists();
    let config = fence::load_runtime_config();
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
    let git_ok = git_present || config.standalone_mode;
    print_check(
        "Git",
        git_ok,
        if git_present {
            ".git directory found"
        } else {
            "standalone mode enabled"
        },
        "run `git init` or re-run `fence init --yes`",
    );
    if !git_ok {
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

fn print_json<T: Serialize>(value: &T) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_completions(shell: Shell) -> Result<(), Box<dyn Error>> {
    let mut command = Cli::command();
    let mut output = Vec::new();
    clap_complete::generate(shell, &mut command, "fence", &mut output);

    match io::stdout().write_all(&output) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(Box::new(err)),
    }
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

fn sentinel_markdown_report(result: &fence::SentinelCheckResult) -> String {
    if result.bypassed {
        return "### Fence Sentinel\n\nStatus: bypassed for latest commit.".to_string();
    }

    let relevant = result
        .files
        .iter()
        .filter(|file| !file.ignored && (file.monitored || file.points > 0))
        .collect::<Vec<_>>();

    if relevant.is_empty() {
        return "### Fence Sentinel\n\nStatus: no monitored changes detected.".to_string();
    }

    let mut report = String::from("### Fence Sentinel\n\n");
    report.push_str("| File | Changes | Score |\n");
    report.push_str("| :--- | ---: | ---: |\n");
    for file in relevant {
        let changes = if file.deletions > 0 {
            format!("+{}, -{}", file.additions, file.deletions)
        } else {
            format!("+{}", file.additions)
        };
        report.push_str(&format!(
            "| `{}` | {} | {} |\n",
            file.path, changes, file.points
        ));
    }

    report.push_str(&format!(
        "\nRequired score: `>{}`  \nCurrent score: `{}`\n\n",
        result.threshold, result.score
    ));

    if !result.requires_decision {
        report.push_str("Decision: not required.");
    } else if result.decision_found {
        report.push_str("Decision: found.");
    } else {
        report.push_str(
            "Missing: `.fence/decisions` change.\n\nRun `fence log \"why this change is intentional\"` and commit the generated decision file.",
        );
    }

    report
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
