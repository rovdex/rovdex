use anyhow::Result;
use clap::{Parser, Subcommand};
use rovdex_core::{Context, Engine, Task};

#[derive(Parser, Debug)]
#[command(name = "rovdex", version, about = "Rovdex coding agent")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Chat {
        prompt: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Engine::new();

    match cli.command {
        Commands::Chat { prompt } => {
            let output = engine.run(
                Context {
                    cwd: std::env::current_dir()?.display().to_string(),
                    repository_root: None,
                },
                Task::new("session", prompt),
            );
            println!("{output}");
        }
    }

    Ok(())
}
