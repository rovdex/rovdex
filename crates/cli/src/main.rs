use anyhow::Result;
use clap::{Parser, Subcommand};
use rovdex_core::{Context, EchoProvider, Engine, Task};

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
    Tui,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Engine::with_standard_tools(EchoProvider);
    let context = Context::from_current_dir()?;

    match cli.command {
        Commands::Chat { prompt } => {
            let result = engine.run(context, Task::new("session", prompt))?;
            println!("{}", result.final_message);
        }
        Commands::Tui => {
            rovdex_tui::run()?;
        }
    }

    Ok(())
}
