pub mod agent;
pub mod app_paths;
pub mod auth;
pub mod config;
pub mod context;
pub mod engine;
pub mod message;
pub mod provider;
pub mod session;
pub mod session_store;
pub mod task;
pub mod tool;
pub mod tools;
pub mod workspace_map;

pub use agent::{Agent, AgentMode, PermissionAction, PermissionRule, PermissionScope};
pub use app_paths::{AppPaths, DesktopPlatform};
pub use auth::{
    exchange_github_token_for_copilot, discover_github_token, AuthProvider, AuthStatus,
    AuthStore, CopilotExchange, TokenDiscovery,
};
pub use config::{ModelConfig, ProviderConfig, ProviderKind, WorkspaceConfig};
pub use context::Context;
pub use engine::{Engine, RunResult};
pub use message::{Message, Role};
pub use provider::{
    EchoProvider, ModelInfo, Provider, ProviderCatalogEntry, ProviderRequest, ProviderResponse,
    ProviderSelection, RouterProvider, ScriptedProvider, ToolCall,
};
pub use session::{MessagePart, Session, SessionEvent, SessionMessage, SessionRun};
pub use session_store::{SessionStore, SessionSummary, StoredSession};
pub use task::Task;
pub use tool::{Tool, ToolRegistry, ToolResult, ToolSpec};
pub use workspace_map::{DirectorySummary, FileSummary, WorkspaceMap, WorkspaceMapOptions};
