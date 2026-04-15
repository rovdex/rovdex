use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use rovdex_core::{
    exchange_github_token_for_copilot, discover_github_token, AppPaths, AuthProvider, AuthStore,
    Context, Engine, PermissionAction, PermissionScope, RouterProvider, SessionEvent,
    SessionMessage, SessionStore, Task, ToolRegistry, WorkspaceConfig,
};
use rovdex_core::tools::{
    BashTool, CurrentDirectoryTool, EditFileTool, GitStatusTool, GlobTool, GrepTool,
    ListDirectoryTool, ReadFileTool, WorkspaceMapTool, WriteFileTool,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_agent: Option<String>,
    #[serde(default)]
    approval_mode: ApprovalMode,
    #[serde(default)]
    provider_api_keys: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ApprovalMode {
    #[default]
    Manual,
    Auto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopAgent {
    id: String,
    label: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopModel {
    id: String,
    label: String,
    supports_tools: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopProvider {
    id: String,
    label: String,
    kind: String,
    default_model: Option<String>,
    authenticated: bool,
    uses_stored_key: bool,
    models: Vec<DesktopModel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopAuthState {
    provider: String,
    stored: bool,
    source: Option<String>,
    auth_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopSessionSummary {
    id: String,
    provider: String,
    model: String,
    agent: String,
    iterations: usize,
    final_message_preview: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopState {
    app_name: String,
    cwd: String,
    repository_root: Option<String>,
    selected_agent: String,
    approval_mode: ApprovalMode,
    selected_provider: String,
    selected_model: String,
    agents: Vec<DesktopAgent>,
    providers: Vec<DesktopProvider>,
    copilot_auth: DesktopAuthState,
    sessions: Vec<DesktopSessionSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceEntry {
    name: String,
    path: String,
    kind: String,
    children: Option<Vec<WorkspaceEntry>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceTreeResponse {
    root: String,
    entries: Vec<WorkspaceEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceFileResponse {
    path: String,
    content: String,
    language: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveWorkspaceFileRequest {
    path: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceDiffResponse {
    path: String,
    repository_root: Option<String>,
    changed: bool,
    status: Vec<String>,
    diff: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunAgentResponse {
    session_id: String,
    final_message: String,
    iterations: usize,
    messages: Vec<UiMessage>,
    pending_permissions: Vec<PendingPermission>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingPermission {
    tool: String,
    scope: String,
    target: String,
    input: Value,
    preview: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutePendingToolRequest {
    tool: String,
    input: Value,
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecutePendingToolResponse {
    tool: String,
    output: Value,
    rendered: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunAgentRequest {
    prompt: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    approval_mode: Option<ApprovalMode>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderCredentialRequest {
    provider_id: String,
    api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderSelectionRequest {
    provider_id: String,
    model_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionPreferencesRequest {
    agent_id: String,
    approval_mode: ApprovalMode,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CopilotLoginResponse {
    provider: String,
    source: String,
    auth_file: String,
    verified: bool,
    verified_expires_at: Option<i64>,
}

fn settings_path(app_name: &str) -> Result<PathBuf> {
    Ok(AppPaths::discover(app_name)?.config_dir_path().join("desktop-settings.json"))
}

fn load_settings(app_name: &str) -> Result<DesktopSettings> {
    let path = settings_path(app_name)?;
    if !path.exists() {
        return Ok(DesktopSettings::default());
    }

    let content = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

fn save_settings(app_name: &str, settings: &DesktopSettings) -> Result<()> {
    let path = settings_path(app_name)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(settings)?)?;
    Ok(())
}

fn load_config_with_settings() -> Result<(WorkspaceConfig, DesktopSettings)> {
    let mut config = WorkspaceConfig::default();
    let settings = load_settings(&config.app_name)?;
    for (provider_id, api_key) in &settings.provider_api_keys {
        if let Some(provider) = config.providers.get_mut(provider_id) {
            if !api_key.trim().is_empty() {
                provider.api_key = Some(api_key.clone());
            }
        }
    }
    Ok((config, settings))
}

fn context_from(cwd: Option<String>) -> Result<Context> {
    match cwd {
        Some(path) => Context::from_path(PathBuf::from(path)),
        None => Context::from_current_dir(),
    }
}

fn auth_state(app_name: &str) -> Result<DesktopAuthState> {
    let provider = AuthProvider::GitHubCopilot;
    let store = AuthStore::for_app(app_name)?;
    let status = store.status(provider.clone())?;
    Ok(DesktopAuthState {
        provider: provider.as_str().to_string(),
        stored: status.stored,
        source: status.source,
        auth_file: status.auth_file,
    })
}

fn provider_authenticated(provider: &rovdex_core::ProviderConfig) -> bool {
    match provider.kind {
        rovdex_core::ProviderKind::LocalEcho => true,
        rovdex_core::ProviderKind::RemoteOpenAiCompatible => provider
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .is_some()
            || provider
                .api_key_env
                .as_deref()
                .and_then(|name| std::env::var(name).ok())
                .is_some_and(|value| !value.trim().is_empty()),
    }
}

fn build_state(cwd: Option<String>) -> Result<DesktopState> {
    let (config, settings) = load_config_with_settings()?;
    let context = context_from(cwd)?;
    let selected_agent = settings
        .selected_agent
        .as_deref()
        .and_then(|name| config.agent(name))
        .unwrap_or_else(|| config.default_agent());
    let selection = config.resolve_provider_selection(
        selected_agent,
        settings.selected_provider.as_deref(),
        settings.selected_model.as_deref(),
    )?;
    let sessions = SessionStore::for_context(&context)
        .list()?
        .into_iter()
        .take(20)
        .map(|session| DesktopSessionSummary {
            id: session.id,
            provider: session.provider,
            model: session.model,
            agent: session.agent,
            iterations: session.iterations,
            final_message_preview: session.final_message_preview,
        })
        .collect();

    let providers = config
        .providers
        .values()
        .map(|provider| DesktopProvider {
            id: provider.id.clone(),
            label: provider.label.clone(),
            kind: format!("{:?}", provider.kind),
            default_model: provider.default_model.clone(),
            authenticated: provider_authenticated(provider),
            uses_stored_key: settings.provider_api_keys.contains_key(&provider.id),
            models: provider
                .models
                .values()
                .map(|model| DesktopModel {
                    id: model.id.clone(),
                    label: model.label.clone(),
                    supports_tools: model.supports_tools,
                })
                .collect(),
        })
        .collect();
    let agents = config
        .agents
        .values()
        .map(|agent| DesktopAgent {
            id: agent.name.clone(),
            label: agent.name.clone(),
            description: agent.description.clone(),
        })
        .collect();

    Ok(DesktopState {
        app_name: config.app_name.clone(),
        cwd: context.cwd.display().to_string(),
        repository_root: context
            .repository_root
            .as_ref()
            .map(|path| path.display().to_string()),
        selected_agent: selected_agent.name.clone(),
        approval_mode: settings.approval_mode,
        selected_provider: selection.provider_id,
        selected_model: selection.model_id,
        agents,
        providers,
        copilot_auth: auth_state(&config.app_name)?,
        sessions,
    })
}

fn root_path(context: &Context) -> PathBuf {
    context
        .repository_root
        .clone()
        .unwrap_or_else(|| context.cwd.clone())
}

fn scan_dir(path: &Path, depth: usize) -> Result<Vec<WorkspaceEntry>> {
    let mut entries = fs::read_dir(path)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut result = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "node_modules" || name == "target" {
            continue;
        }

        let entry_path = entry.path();
        let file_type = entry.file_type()?;
        let children = if file_type.is_dir() && depth > 0 {
            Some(scan_dir(&entry_path, depth - 1)?)
        } else {
            None
        };

        result.push(WorkspaceEntry {
            name,
            path: entry_path.display().to_string(),
            kind: if file_type.is_dir() {
                "directory".to_string()
            } else {
                "file".to_string()
            },
            children,
        });
    }

    Ok(result)
}

fn detect_language(path: &Path) -> String {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or_default() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "json" => "json",
        "md" => "markdown",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "css" => "css",
        "html" => "html",
        "sh" => "shell",
        _ => "plaintext",
    }
    .to_string()
}

fn repository_root_for_path(path: &Path) -> Option<PathBuf> {
    let base = if path.is_dir() { path } else { path.parent()? };
    Context::from_path(base)
        .ok()
        .and_then(|context| context.repository_root)
}

fn task_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    format!("desktop-{millis}")
}

fn standard_tool_registry() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(BashTool);
    tools.register(CurrentDirectoryTool);
    tools.register(EditFileTool);
    tools.register(ListDirectoryTool);
    tools.register(GlobTool);
    tools.register(GrepTool);
    tools.register(ReadFileTool);
    tools.register(WriteFileTool);
    tools.register(GitStatusTool);
    tools.register(WorkspaceMapTool);
    tools
}

fn render_pending_preview(tool: &str, input: &Value) -> String {
    let object = match input.as_object() {
        Some(value) => value,
        None => return serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string()),
    };

    match tool {
        "write" => {
            let path = object.get("path").and_then(Value::as_str).unwrap_or("<unknown>");
            let content = object.get("content").and_then(Value::as_str).unwrap_or("");
            let snippet = content.chars().take(240).collect::<String>();
            format!("write -> {path}\n\n{snippet}")
        }
        "edit" => {
            let path = object.get("path").and_then(Value::as_str).unwrap_or("<unknown>");
            let old_text = object.get("old_text").and_then(Value::as_str).unwrap_or("");
            let new_text = object.get("new_text").and_then(Value::as_str).unwrap_or("");
            format!(
                "edit -> {path}\n\n--- old ---\n{}\n\n+++ new +++\n{}",
                old_text.chars().take(160).collect::<String>(),
                new_text.chars().take(160).collect::<String>()
            )
        }
        "bash" => {
            let command = object.get("command").and_then(Value::as_str).unwrap_or("");
            format!("bash\n\n{command}")
        }
        _ => serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string()),
    }
}

fn resolve_agent_for_request(
    config: &WorkspaceConfig,
    agent_name: Option<&str>,
    approval_mode: ApprovalMode,
) -> Result<rovdex_core::Agent> {
    let mut agent = agent_name
        .and_then(|name| config.agent(name).cloned())
        .unwrap_or_else(|| config.default_agent().clone());

    for rule in &mut agent.permissions {
        match approval_mode {
            ApprovalMode::Manual => {
                if matches!(rule.scope, PermissionScope::Write | PermissionScope::Bash)
                    && rule.action == PermissionAction::Allow
                {
                    rule.action = PermissionAction::Ask;
                }
            }
            ApprovalMode::Auto => {
                if matches!(rule.scope, PermissionScope::Write | PermissionScope::Bash)
                    && rule.action == PermissionAction::Ask
                {
                    rule.action = PermissionAction::Allow;
                }
            }
        }
    }

    Ok(agent)
}

#[tauri::command]
fn desktop_state(cwd: Option<String>) -> Result<DesktopState, String> {
    build_state(cwd).map_err(|error| error.to_string())
}

#[tauri::command]
fn workspace_tree(cwd: Option<String>) -> Result<WorkspaceTreeResponse, String> {
    let context = context_from(cwd).map_err(|error| error.to_string())?;
    let root = root_path(&context);
    let entries = scan_dir(&root, 2).map_err(|error| error.to_string())?;
    Ok(WorkspaceTreeResponse {
        root: root.display().to_string(),
        entries,
    })
}

#[tauri::command]
fn read_workspace_file(path: String) -> Result<WorkspaceFileResponse, String> {
    let path = PathBuf::from(path);
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(WorkspaceFileResponse {
        path: path.display().to_string(),
        content,
        language: detect_language(&path),
    })
}

#[tauri::command]
fn save_workspace_file(request: SaveWorkspaceFileRequest) -> Result<WorkspaceFileResponse, String> {
    let path = PathBuf::from(&request.path);
    fs::write(&path, request.content.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;

    Ok(WorkspaceFileResponse {
        path: path.display().to_string(),
        content: request.content,
        language: detect_language(&path),
    })
}

#[tauri::command]
fn workspace_file_diff(path: String) -> Result<WorkspaceDiffResponse, String> {
    let path = PathBuf::from(path);
    let repository_root = repository_root_for_path(&path);
    let Some(root) = repository_root.clone() else {
        return Ok(WorkspaceDiffResponse {
            path: path.display().to_string(),
            repository_root: None,
            changed: false,
            status: Vec::new(),
            diff: String::new(),
        });
    };

    let relative_path = path
        .strip_prefix(&root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.clone());

    let status_output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("status")
        .arg("--short")
        .arg("--")
        .arg(&relative_path)
        .output()
        .map_err(|error| format!("failed to run git status: {error}"))?;

    if !status_output.status.success() {
        return Err(String::from_utf8_lossy(&status_output.stderr).trim().to_string());
    }

    let diff_output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("--no-pager")
        .arg("diff")
        .arg("--")
        .arg(&relative_path)
        .output()
        .map_err(|error| format!("failed to run git diff: {error}"))?;

    if !diff_output.status.success() {
        return Err(String::from_utf8_lossy(&diff_output.stderr).trim().to_string());
    }

    let status = String::from_utf8_lossy(&status_output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let diff = String::from_utf8_lossy(&diff_output.stdout).to_string();

    Ok(WorkspaceDiffResponse {
        path: path.display().to_string(),
        repository_root: Some(root.display().to_string()),
        changed: !status.is_empty() || !diff.trim().is_empty(),
        status,
        diff,
    })
}

#[tauri::command]
fn set_provider_api_key(request: ProviderCredentialRequest) -> Result<DesktopState, String> {
    let config = WorkspaceConfig::default();
    let mut settings = load_settings(&config.app_name).map_err(|error| error.to_string())?;
    if request.api_key.trim().is_empty() {
        settings.provider_api_keys.remove(&request.provider_id);
    } else {
        settings
            .provider_api_keys
            .insert(request.provider_id, request.api_key.trim().to_string());
    }
    save_settings(&config.app_name, &settings).map_err(|error| error.to_string())?;
    build_state(None).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_provider_selection(request: ProviderSelectionRequest) -> Result<DesktopState, String> {
    let config = WorkspaceConfig::default();
    let mut settings = load_settings(&config.app_name).map_err(|error| error.to_string())?;
    settings.selected_provider = Some(request.provider_id);
    settings.selected_model = Some(request.model_id);
    save_settings(&config.app_name, &settings).map_err(|error| error.to_string())?;
    build_state(None).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_execution_preferences(request: ExecutionPreferencesRequest) -> Result<DesktopState, String> {
    let config = WorkspaceConfig::default();
    let mut settings = load_settings(&config.app_name).map_err(|error| error.to_string())?;
    settings.selected_agent = Some(request.agent_id);
    settings.approval_mode = request.approval_mode;
    save_settings(&config.app_name, &settings).map_err(|error| error.to_string())?;
    build_state(None).map_err(|error| error.to_string())
}

#[tauri::command]
fn auth_login_copilot(
    github_token: Option<String>,
    no_verify: Option<bool>,
) -> Result<CopilotLoginResponse, String> {
    let config = WorkspaceConfig::default();
    let store = AuthStore::for_app(&config.app_name).map_err(|error| error.to_string())?;
    let provider = AuthProvider::GitHubCopilot;
    let discovery = match github_token {
        Some(token) => rovdex_core::TokenDiscovery {
            token,
            source: "desktop:manual-token".to_string(),
        },
        None => discover_github_token().map_err(|error| error.to_string())?,
    };

    let mut verified = false;
    let mut verified_expires_at = None;
    if !no_verify.unwrap_or(false) {
        let exchange = exchange_github_token_for_copilot(&discovery.token)
            .map_err(|error| error.to_string())?;
        verified = true;
        verified_expires_at = exchange.expires_at;
    }

    let record = store
        .save(provider.clone(), discovery.token, discovery.source)
        .map_err(|error| error.to_string())?;
    Ok(CopilotLoginResponse {
        provider: provider.as_str().to_string(),
        source: record.source,
        auth_file: store.path().display().to_string(),
        verified,
        verified_expires_at,
    })
}

#[tauri::command]
fn auth_logout_copilot() -> Result<DesktopAuthState, String> {
    let config = WorkspaceConfig::default();
    let store = AuthStore::for_app(&config.app_name).map_err(|error| error.to_string())?;
    store
        .delete(AuthProvider::GitHubCopilot)
        .map_err(|error| error.to_string())?;
    auth_state(&config.app_name).map_err(|error| error.to_string())
}

#[tauri::command]
fn run_agent(request: RunAgentRequest) -> Result<RunAgentResponse, String> {
    let (config, settings) = load_config_with_settings().map_err(|error| error.to_string())?;
    let context = context_from(request.cwd).map_err(|error| error.to_string())?;
    let engine =
        Engine::with_standard_tools(RouterProvider::from_config(&config)).with_config(config.clone());
    let approval_mode = request.approval_mode.unwrap_or(settings.approval_mode);
    let agent = resolve_agent_for_request(
        &config,
        request.agent.as_deref().or(settings.selected_agent.as_deref()),
        approval_mode,
    )
    .map_err(|error| error.to_string())?;
    let selection = config
        .resolve_provider_selection(&agent, request.provider.as_deref(), request.model.as_deref())
        .map_err(|error| error.to_string())?;
    let run = engine
        .run_session(
            context.clone(),
            Task::new(task_id(), request.prompt),
            agent,
            selection,
        )
        .map_err(|error| error.to_string())?;

    let stored = SessionStore::for_context(&context)
        .save_run(&run)
        .map_err(|error| error.to_string())?;

    let messages = run
        .session
        .messages
        .iter()
        .filter_map(render_session_message)
        .collect();
    let pending_permissions = run
        .events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::PermissionRequired {
                tool,
                scope,
                target,
                ..
            } => Some(PendingPermission {
                tool: tool.clone(),
                scope: scope.clone(),
                target: target.clone(),
                input: run
                    .session
                    .messages
                    .iter()
                    .find(|message| match &message.parts[..] {
                        [rovdex_core::MessagePart::ToolCall { tool: called_tool, .. }, ..] => called_tool == tool,
                        _ => false,
                    })
                    .and_then(|message| {
                        message.parts.iter().find_map(|part| match part {
                            rovdex_core::MessagePart::ToolCall { input, .. } => Some(input.clone()),
                            _ => None,
                        })
                    })
                    .unwrap_or(Value::Null),
                preview: run
                    .session
                    .messages
                    .iter()
                    .find(|message| match &message.parts[..] {
                        [rovdex_core::MessagePart::ToolCall { tool: called_tool, .. }, ..] => called_tool == tool,
                        _ => false,
                    })
                    .and_then(|message| {
                        message.parts.iter().find_map(|part| match part {
                            rovdex_core::MessagePart::ToolCall { input, .. } => {
                                Some(render_pending_preview(tool, input))
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or_else(|| target.clone()),
            }),
            _ => None,
        })
        .collect();

    Ok(RunAgentResponse {
        session_id: stored.id,
        final_message: run.final_message,
        iterations: run.iterations,
        messages,
        pending_permissions,
    })
}

#[tauri::command]
fn execute_pending_tool(request: ExecutePendingToolRequest) -> Result<ExecutePendingToolResponse, String> {
    let context = context_from(request.cwd).map_err(|error| error.to_string())?;
    let tools = standard_tool_registry();
    let result = tools
        .call(&request.tool, &context, &request.input)
        .map_err(|error| error.to_string())?;

    Ok(ExecutePendingToolResponse {
        tool: request.tool,
        rendered: result.render(),
        output: result.output,
    })
}

fn render_session_message(message: &SessionMessage) -> Option<UiMessage> {
    let content = message
        .parts
        .iter()
        .filter_map(|part| match part {
            rovdex_core::MessagePart::Text { text } => Some(text.clone()),
            rovdex_core::MessagePart::ToolResult { tool, output } => Some(format!(
                "[{tool}] {}",
                serde_json::to_string_pretty(output).unwrap_or_else(|_| output.to_string())
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if content.trim().is_empty() {
        return None;
    }

    Some(UiMessage {
        role: format!("{:?}", message.role).to_lowercase(),
        content,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            desktop_state,
            workspace_tree,
            read_workspace_file,
            save_workspace_file,
            workspace_file_diff,
            set_provider_api_key,
            set_provider_selection,
            set_execution_preferences,
            auth_login_copilot,
            auth_logout_copilot,
            run_agent
            ,execute_pending_tool
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
