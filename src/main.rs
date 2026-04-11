use clap::{Parser, Subcommand};
use fence::FenceManager; // 'fence' is the name of your crate in Cargo.toml

#[derive(Parser)]
#[command(name = "fence", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Log { message: String },
    List,
    Search { keyword: String },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Log { message } => {
            FenceManager::record(message);
            println!("🚀 Decision recorded!");
        }
        Commands::List => {
            println!("\n📖 --- DECISION HISTORY ---");
            println!("{}", FenceManager::list());
        }
        Commands::Search { keyword } => {
            let results = FenceManager::search(keyword);
            println!("\n🔍 --- SEARCH RESULTS ---");
            for line in results { println!("{}", line); }
        }
    }
}