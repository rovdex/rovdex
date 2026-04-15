use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use rovdex_core::{
    exchange_github_token_for_copilot, discover_github_token, AppPaths, AuthProvider, AuthStore,
    Context, Engine, RouterProvider, SessionStore, Task, WorkspaceConfig, WorkspaceMap,
};

#[derive(Parser, Debug)]
#[command(name = "rovdex", version, about = "Rovdex coding agent")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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
    Map {
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    Provider {
        #[command(subcommand)]
        command: ProviderCommands,
    },
    Paths {
        #[arg(long)]
        json: bool,
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

#[derive(Subcommand, Debug)]
enum SessionCommands {
    List,
    Show {
        id: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum AuthCommands {
    Login {
        #[arg(default_value = "copilot")]
        provider: String,
        #[arg(long)]
        github_token: Option<String>,
        #[arg(long)]
        no_verify: bool,
    },
    Status {
        #[arg(default_value = "copilot")]
        provider: String,
    },
    Logout {
        #[arg(default_value = "copilot")]
        provider: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = WorkspaceConfig::default();
    let engine = Engine::with_standard_tools(RouterProvider::from_config(&config)).with_config(config.clone());
    let context = Context::from_current_dir()?;
    let session_store = SessionStore::for_context(&context);
    let auth_store = AuthStore::for_app(&config.app_name)?;

    let command = cli.command.unwrap_or(Commands::Tui {
        demo: false,
        preview: false,
    });

    match command {
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
            let stored = session_store.save_run(&result)?;
            println!("[session:{}]", stored.id);
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
        Commands::Map { path, json } => {
            let root = path
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| context.repository_root.clone().unwrap_or_else(|| context.cwd.clone()));
            let map = WorkspaceMap::scan(&root)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&map)?);
            } else {
                print!("{}", map.render_markdown());
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
        Commands::Session {
            command: SessionCommands::List,
        } => {
            for session in session_store.list()? {
                println!("{}", session.id);
                println!("  provider: {}/{}", session.provider, session.model);
                println!("  agent: {}", session.agent);
                println!("  iterations: {}", session.iterations);
                println!("  cwd: {}", session.cwd);
                println!("  final: {}", session.final_message_preview);
            }
        }
        Commands::Session {
            command: SessionCommands::Show { id },
        } => {
            let stored = match id {
                Some(id) => session_store.load(&id)?,
                None => session_store
                    .latest()?
                    .ok_or_else(|| anyhow!("no stored sessions found in {}", session_store.root().display()))?,
            };
            println!("{}", serde_json::to_string_pretty(&stored)?);
        }
        Commands::Auth {
            command: AuthCommands::Login {
                provider,
                github_token,
                no_verify,
            },
        } => {
            let provider = AuthProvider::parse(&provider)?;
            let discovery = match github_token {
                Some(token) => rovdex_core::TokenDiscovery {
                    token,
                    source: String::from("flag:--github-token"),
                },
                None => discover_github_token()?,
            };

            if matches!(provider, AuthProvider::GitHubCopilot) && !no_verify {
                let exchange = exchange_github_token_for_copilot(&discovery.token)?;
                println!(
                    "verified: {} bearer token acquired{}",
                    provider.as_str(),
                    exchange
                        .expires_at
                        .map(|expires_at| format!(" (expires_at: {expires_at})"))
                        .unwrap_or_default()
                );
            }

            let record = auth_store.save(provider.clone(), discovery.token, discovery.source)?;
            println!("stored: {}", provider.as_str());
            println!("source: {}", record.source);
            println!("auth_file: {}", auth_store.path().display());
        }
        Commands::Auth {
            command: AuthCommands::Status { provider },
        } => {
            let provider = AuthProvider::parse(&provider)?;
            let status = auth_store.status(provider.clone())?;
            println!("provider: {}", provider.as_str());
            println!("stored: {}", status.stored);
            println!("auth_file: {}", status.auth_file);
            println!("source: {}", status.source.as_deref().unwrap_or("<none>"));
        }
        Commands::Auth {
            command: AuthCommands::Logout { provider },
        } => {
            let provider = AuthProvider::parse(&provider)?;
            let removed = auth_store.delete(provider.clone())?;
            println!("provider: {}", provider.as_str());
            println!("removed: {}", removed);
            println!("auth_file: {}", auth_store.path().display());
        }
        Commands::Config => {
            let config = serde_json::to_string_pretty(engine.config())?;
            println!("{config}");
        }
        Commands::Paths { json } => {
            let paths = AppPaths::discover(&config.app_name)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&paths)?);
            } else {
                println!("app: {}", paths.app_name);
                println!("platform: {}", paths.platform.as_str());
                println!("home: {}", paths.home_dir);
                println!("data: {}", paths.data_dir);
                println!("config: {}", paths.config_dir);
                println!("cache: {}", paths.cache_dir);
                println!("project_sessions: {}", session_store.root().display());
                let global_store = SessionStore::for_app(&config.app_name)?;
                println!("global_sessions: {}", global_store.root().display());
                println!("auth_file: {}", auth_store.path().display());
            }
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
