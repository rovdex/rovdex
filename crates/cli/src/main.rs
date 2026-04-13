use anyhow::Result;
use clap::{Parser, Subcommand};
use rovdex_core::{Context, Engine, RouterProvider, Task, WorkspaceConfig};

#[derive(Parser, Debug)]
#[command(name = "rovdex", version, about = "Rovdex coding agent")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Chat {
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
        prompt: String,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    Config,
    Tui {
        #[arg(long)]
        demo: bool,
        #[arg(long)]
        preview: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AgentCommands {
    List,
}

#[derive(Subcommand, Debug)]
enum ProviderCommands {
    List,
    Models {
        provider: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = WorkspaceConfig::default();
    let engine = Engine::with_standard_tools(RouterProvider::from_config(&config)).with_config(config.clone());
    let context = Context::from_current_dir()?;

    match cli.command {
        Commands::Chat {
            agent,
            provider,
            model,
            prompt,
        } => {
            let result = engine.run_with_selection(
                context,
                Task::new("session", prompt),
                agent.as_deref(),
                provider.as_deref(),
                model.as_deref(),
            )?;
            println!("{}", result.final_message);
        }
        Commands::Agent {
            command: AgentCommands::List,
        } => {
            for agent in engine.config().agents.values() {
                println!("{} ({:?})", agent.name, agent.mode);
                println!("  {}", agent.description);
                println!("  tools: {}", agent.tools.len());
                println!("  permissions: {}", agent.permissions.len());
            }
        }
        Commands::Provider {
            command: ProviderCommands::List,
        } => {
            for provider in config.providers.values() {
                println!("{}", provider.id);
                println!("  {}", provider.label);
                println!("  kind: {:?}", provider.kind);
                println!("  models: {}", provider.models.len());
                println!("  default: {}", provider.default_model.as_deref().unwrap_or("<none>"));
                println!("  api_base: {}", provider.api_base.as_deref().unwrap_or("<none>"));
                println!("  api_key_env: {}", provider.api_key_env.as_deref().unwrap_or("<none>"));
            }
        }
        Commands::Provider {
            command: ProviderCommands::Models { provider },
        } => {
            let providers = match provider.as_deref() {
                Some(provider_id) => vec![config
                    .provider(provider_id)
                    .ok_or_else(|| anyhow::anyhow!("unknown provider: {provider_id}"))?],
                None => config.providers.values().collect(),
            };

            for provider in providers {
                println!("{}", provider.id);
                for model in provider.models.values() {
                    println!("  {} - {}", model.id, model.label);
                }
            }
        }
        Commands::Config => {
            let config = serde_json::to_string_pretty(engine.config())?;
            println!("{config}");
        }
        Commands::Tui { demo, preview } => {
            if preview {
                print!("{}", rovdex_tui::preview(demo));
            } else {
                rovdex_tui::run(demo)?;
            }
        }
    }

    Ok(())
}
