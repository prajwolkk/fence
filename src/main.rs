use std::error::Error;
use std::io;
use std::path::Path;
use std::process;
use std::time::Duration;

use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dialoguer::{Confirm, Input, Select};
use fence::{
    config_path, default_project_name, ensure_gitignore_contains, ensure_log_file, git_hooks_path,
    git_remote_platform, has_git_directory, install_pre_commit_hook, remove_ignore_entry,
    sanitize_project_name, FenceConfig, FenceManager, FenceMode, NotificationProvider,
    NotificationsConfig, TeamSettings,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

#[derive(Parser)]
#[command(name = "fence", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Log {
        message: String,
        #[arg(short, long)]
        category: Option<String>,
        #[arg(short, long)]
        tags: Option<String>,
    },
    List,
    Search { keyword: String },
    Check,
    Export,
    Browse,
    Site,
    Sentinel {
        #[command(subcommand)]
        command: SentinelCommands,
    },
    Badge,
}

#[derive(Subcommand)]
enum SentinelCommands {
    Init,
    Check {
        #[arg(long)]
        base: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => run_init()?,
        Commands::Log {
            message,
            category,
            tags,
        } => {
            let category = parse_category(category);
            let tags = parse_tags(tags);
            FenceManager::record_with_metadata(&message, category, tags)?;
            println!("🚀 Decision recorded and DECISIONS.md updated!");
        }
        Commands::List => {
            println!("\n📖 --- DECISION HISTORY ---");
            println!("{}", FenceManager::list());
        }
        Commands::Search { keyword } => {
            let results = FenceManager::search(&keyword);
            println!("\n🔍 --- SEARCH RESULTS ---");
            for line in results {
                println!("{line}");
            }
        }
        Commands::Check => {
            let in_sync = fence::check_sync()?;
            let (tracking_ok, log_status, md_status) = fence::check_tracking_integrity()?;
            if !in_sync || !tracking_ok {
                if !in_sync {
                    println!(
                        "Sync Error: DECISIONS.md is out of sync. Run 'fence export' to fix it."
                    );
                }
                if !tracking_ok {
                    println!("Tracking Error: tracked files are out of sync with the staged versions.");
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
        Commands::Browse => {
            run_browse()?;
        }
        Commands::Site => {
            let path = fence::generate_site()?;
            println!("Generated site at {}", path.display());
        }
        Commands::Sentinel { command } => match command {
            SentinelCommands::Init => {
                if !has_git_directory() {
                    println!(
                        "The Sentinel requires a Git repository. Please run git init first."
                    );
                    process::exit(1);
                }
                println!("Sentinel setup is not yet implemented in this build.");
            }
            SentinelCommands::Check { base } => {
                if !has_git_directory() {
                    println!(
                        "The Sentinel requires a Git repository. Please run git init first."
                    );
                    process::exit(1);
                }
                let result = fence::sentinel_check(base)?;
                if result.bypassed {
                    println!("✅ Sentinel bypassed for latest commit.");
                    return Ok(());
                }
                if result.changed_files == 0 {
                    println!("✅ No monitored changes detected.");
                    return Ok(());
                }
                if result.decision_found {
                    println!(
                        "✅ Decision found for {} modified files.",
                        result.changed_files
                    );
                } else {
                    println!("❌ Architectural change detected without log.");
                    process::exit(1);
                }
            }
        },
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

fn run_init() -> Result<(), Box<dyn Error>> {
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

    let mode_index = Select::new()
        .with_prompt("Fence Mode")
        .items(["Solo (Local/Personal)", "Team (Shared/Collaborative)"])
        .default(0)
        .interact()?;

    let (mode, notifications, team_settings) = if mode_index == 1 {
        let provider_index = Select::new()
            .with_prompt("Notification Provider")
            .items([
                "Slack",
                "Discord",
                "Generic Webhook",
                "Custom Command",
            ])
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
    let suggested_paths = detect_monitored_paths();
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
    let log_path = Path::new(&config.log_path);

    ensure_log_file(log_path)?;

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
            .with_prompt("Track decisions.log in Git?")
            .default(false)
            .interact()?;
        let track_md = Confirm::new()
            .with_prompt("Track DECISIONS.md in Git?")
            .default(true)
            .interact()?;

        if track_log {
            remove_ignore_entry(Path::new(".gitignore"), &config.log_path)?;
        } else {
            ensure_gitignore_contains(&config.log_path)?;
        }
        if track_md {
            remove_ignore_entry(Path::new(".gitignore"), "DECISIONS.md")?;
        } else {
            ensure_gitignore_contains("DECISIONS.md")?;
        }
    }

    if git_present {
        if let Some(platform) = git_remote_platform() {
            let setup_sentinel = Confirm::new()
                .with_prompt(format!(
                    "Enable Sentinel CI automation for {platform}? (Y/n)"
                ))
                .default(true)
                .interact()?;
            config.sentinel_enabled = setup_sentinel;
            if setup_sentinel {
                config.sentinel_platform = Some(platform);
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

fn parse_category(value: Option<String>) -> fence::DecisionCategory {
    let normalized = value
        .unwrap_or_else(|| "gen".to_string())
        .to_lowercase();

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

fn detect_monitored_paths() -> Vec<String> {
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

fn run_browse() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let result = browse_loop(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn browse_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), Box<dyn Error>> {
    let entries = fence::read_log_entries()?;
    let mut list_state = ListState::default();
    if !entries.is_empty() {
        list_state.select(Some(0));
    }
    let mut detail_focus = false;
    let log_status = fence::tracking_status_for_log();
    let md_status = fence::tracking_status_for_markdown();

    loop {
        terminal.draw(|frame| {
            draw_browse_ui(
                frame,
                &entries,
                &mut list_state,
                detail_focus,
                log_status,
                md_status,
            )
        })?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Down | KeyCode::Char('j') => {
                        move_selection(1, &entries, &mut list_state);
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        move_selection(-1, &entries, &mut list_state);
                    }
                    KeyCode::Enter => {
                        detail_focus = !detail_focus;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn draw_browse_ui(
    frame: &mut Frame,
    entries: &[fence::Decision],
    list_state: &mut ListState,
    detail_focus: bool,
    log_status: fence::TrackingStatus,
    md_status: fence::TrackingStatus,
) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(if detail_focus {
            [Constraint::Percentage(25), Constraint::Percentage(75)]
        } else {
            [Constraint::Percentage(40), Constraint::Percentage(60)]
        })
        .split(layout[0]);

    let list_block = Block::default().borders(Borders::ALL).title("Decisions");
    let detail_block = Block::default().borders(Borders::ALL).title("Details");

    if entries.is_empty() {
        let empty_message = Paragraph::new("No decisions yet. Run `fence log` to create one.")
            .block(list_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(empty_message, body[0]);

        let detail_message = Paragraph::new("Select a decision to view details.")
            .block(detail_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(detail_message, body[1]);
    } else {
        let items: Vec<ListItem> = entries
            .iter()
            .map(|entry| ListItem::new(format!("{} {}", entry_date(entry), entry_title(entry))))
            .collect();
        let list = List::new(items)
            .block(list_block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, body[0], list_state);

        let detail_message = Paragraph::new(detail_text(entries, list_state))
            .block(detail_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(detail_message, body[1]);
    }

    let help = Paragraph::new(format!(
        "q: quit  j/k: navigate  enter: toggle detail  [Log: {}] [MD: {}]",
        tracking_label(log_status),
        tracking_label(md_status)
    ))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(help, layout[1]);
}

fn entry_date(entry: &fence::Decision) -> &str {
    entry
        .timestamp
        .split_whitespace()
        .next()
        .unwrap_or(&entry.timestamp)
}

fn entry_title(entry: &fence::Decision) -> String {
    let title = entry.message.lines().next().unwrap_or("").trim();
    let mut clipped = String::new();
    let mut count = 0;
    for ch in title.chars() {
        if count >= 40 {
            clipped.push_str("...");
            break;
        }
        clipped.push(ch);
        count += 1;
    }
    if clipped.is_empty() {
        "<untitled>".to_string()
    } else {
        clipped
    }
}

fn tracking_label(status: fence::TrackingStatus) -> &'static str {
    match status {
        fence::TrackingStatus::Tracked => "Tracked",
        fence::TrackingStatus::Local => "Local",
    }
}

fn move_selection(delta: isize, entries: &[fence::Decision], list_state: &mut ListState) {
    if entries.is_empty() {
        list_state.select(None);
        return;
    }

    let current = list_state.selected().unwrap_or(0) as isize;
    let next = (current + delta).clamp(0, entries.len().saturating_sub(1) as isize);
    list_state.select(Some(next as usize));
}

fn detail_text(entries: &[fence::Decision], list_state: &ListState) -> String {
    let Some(index) = list_state.selected() else {
        return "Select a decision to view details.".to_string();
    };
    let Some(entry) = entries.get(index) else {
        return "Select a decision to view details.".to_string();
    };

    let tags = if entry.optional_tags.is_empty() {
        "Tags: -".to_string()
    } else {
        format!("Tags: {}", entry.optional_tags.join(", "))
    };

    format!(
        "Category: {} {}\nAuthor: {}\nTimestamp: {}\n{}\n\n{}",
        category_icon(entry.category),
        category_label(entry.category),
        entry.author,
        entry.timestamp,
        tags,
        entry.message
    )
}

fn category_icon(category: fence::DecisionCategory) -> &'static str {
    match category {
        fence::DecisionCategory::Architecture => "🏛️",
        fence::DecisionCategory::Technical => "⚙️",
        fence::DecisionCategory::Product => "🎯",
        fence::DecisionCategory::Security => "🛡️",
        fence::DecisionCategory::General => "🏷️",
    }
}

fn category_label(category: fence::DecisionCategory) -> &'static str {
    match category {
        fence::DecisionCategory::Architecture => "Architecture",
        fence::DecisionCategory::Technical => "Technical",
        fence::DecisionCategory::Product => "Product",
        fence::DecisionCategory::Security => "Security",
        fence::DecisionCategory::General => "General",
    }
}
