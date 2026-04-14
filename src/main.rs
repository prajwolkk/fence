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
    has_git_directory, install_pre_commit_hook, sanitize_project_name, FenceConfig, FenceManager,
    FenceMode, NotificationProvider, NotificationsConfig, TeamSettings,
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
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
    Log { message: String },
    List,
    Search { keyword: String },
    Check,
    Export,
    Browse,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init => run_init()?,
        Commands::Log { message } => {
            FenceManager::record(message)?;
            println!("🚀 Decision recorded and DECISIONS.md updated!");
        }
        Commands::List => {
            println!("\n📖 --- DECISION HISTORY ---");
            println!("{}", FenceManager::list());
        }
        Commands::Search { keyword } => {
            let results = FenceManager::search(keyword);
            println!("\n🔍 --- SEARCH RESULTS ---");
            for line in results {
                println!("{line}");
            }
        }
        Commands::Check => {
            let in_sync = fence::check_sync()?;
            if !in_sync {
                println!("Sync Error: DECISIONS.md is out of sync. Run 'fence export' to fix it.");
                process::exit(1);
            }
        }
        Commands::Export => {
            fence::export_markdown()?;
        }
        Commands::Browse => {
            run_browse()?;
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

    let config = FenceConfig::new(project_name, mode, notifications, team_settings);
    let log_path = Path::new(&config.log_path);

    ensure_log_file(log_path)?;
    fence::write_config(&config_path, &config)?;

    if !has_git_directory() {
        println!("Note: Not a git repository. Fence works best with Git.");
    }

    ensure_gitignore_contains(&config.log_path)?;

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
    loop {
        terminal.draw(|frame| draw_browse_ui(frame))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
            }
        }
    }

    Ok(())
}

fn draw_browse_ui(frame: &mut Frame) {
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(layout[0]);

    let list_block = Block::default().borders(Borders::ALL).title("Decisions");
    let detail_block = Block::default().borders(Borders::ALL).title("Details");

    let empty_message = Paragraph::new("No decisions yet. Run `fence log` to create one.")
        .block(list_block.clone())
        .wrap(Wrap { trim: true });
    frame.render_widget(empty_message, body[0]);

    let detail_message = Paragraph::new("Select a decision to view details.")
        .block(detail_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(detail_message, body[1]);

    let help = Paragraph::new("q: quit")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(help, layout[1]);
}
