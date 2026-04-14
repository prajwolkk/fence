use std::error::Error;
use std::path::Path;

use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input, Select};
use fence::{
    config_path, default_project_name, ensure_gitignore_contains, ensure_log_file, has_git_directory,
    sanitize_project_name, FenceConfig, FenceManager, FenceMode, TeamSettings,
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

    let (mode, team_settings) = if mode_index == 1 {
        let slack_webhook: String = Input::new()
            .with_prompt("Slack Webhook URL (optional)")
            .allow_empty(true)
            .interact_text()?;

        let team_settings = TeamSettings {
            slack_webhook: optional_value(slack_webhook),
            jira_domain: None,
        };

        (FenceMode::Team, Some(team_settings))
    } else {
        (FenceMode::Solo, None)
    };

    let config = FenceConfig::new(project_name, mode, team_settings);
    let log_path = Path::new(&config.log_path);

    ensure_log_file(log_path)?;
    fence::write_config(&config_path, &config)?;

    if !has_git_directory() {
        println!("Note: Not a git repository. Fence works best with Git.");
    }

    ensure_gitignore_contains(&config.log_path)?;

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
