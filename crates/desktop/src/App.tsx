import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import {
  ChevronDown,
  ChevronRight,
  Files,
  FolderOpen,
  GitBranch,
  KeyRound,
  Save,
  MessageSquare,
  RefreshCw,
  Search,
  Settings,
  TerminalSquare,
  User,
  X,
  Zap,
} from "lucide-react";

type DesktopModel = {
  id: string;
  label: string;
  supportsTools: boolean;
};

type DesktopProvider = {
  id: string;
  label: string;
  kind: string;
  defaultModel?: string;
  authenticated: boolean;
  usesStoredKey: boolean;
  models: DesktopModel[];
};

type DesktopAuthState = {
  provider: string;
  stored: boolean;
  source?: string;
  authFile: string;
};

type DesktopSessionSummary = {
  id: string;
  provider: string;
  model: string;
  agent: string;
  iterations: number;
  finalMessagePreview: string;
};

type DesktopState = {
  appName: string;
  cwd: string;
  repositoryRoot?: string;
  selectedAgent: string;
  approvalMode: ApprovalMode;
  selectedProvider: string;
  selectedModel: string;
  agents: DesktopAgent[];
  providers: DesktopProvider[];
  copilotAuth: DesktopAuthState;
  sessions: DesktopSessionSummary[];
};

type DesktopAgent = {
  id: string;
  label: string;
  description: string;
};

type ApprovalMode = "manual" | "auto";

type WorkspaceEntry = {
  name: string;
  path: string;
  kind: "file" | "directory";
  children?: WorkspaceEntry[];
};

type WorkspaceTreeResponse = {
  root: string;
  entries: WorkspaceEntry[];
};

type WorkspaceFileResponse = {
  path: string;
  content: string;
  language: string;
};

type WorkspaceDiffResponse = {
  path: string;
  repositoryRoot?: string;
  changed: boolean;
  status: string[];
  diff: string;
};

type UiMessage = {
  role: string;
  content: string;
};

type RunAgentResponse = {
  sessionId: string;
  finalMessage: string;
  iterations: number;
  messages: UiMessage[];
  pendingPermissions: PendingPermission[];
};

type PendingPermission = {
  tool: string;
  scope: string;
  target: string;
  input: unknown;
  preview: string;
};

type ExecutePendingToolResponse = {
  tool: string;
  output: unknown;
  rendered: string;
};

function TreeNode(props: {
  entry: WorkspaceEntry;
  depth: number;
  selectedPath?: string;
  onOpenFile: (path: string) => void;
}) {
  const { entry, depth, selectedPath, onOpenFile } = props;
  const [expanded, setExpanded] = useState(depth < 1);
  const paddingLeft = 12 + depth * 14;

  if (entry.kind === "file") {
    return (
      <button
        className={`flex w-full items-center py-1 pr-2 text-left text-[13px] ${
          selectedPath === entry.path ? "bg-[#37373d] text-[#c5e1ff]" : "text-[#cccccc] hover:bg-[#2a2d2e]"
        }`}
        style={{ paddingLeft }}
        onClick={() => onOpenFile(entry.path)}
      >
        <span className="mr-2 text-[#519aba]">•</span>
        <span className="truncate">{entry.name}</span>
      </button>
    );
  }

  return (
    <div>
      <button
        className="flex w-full items-center py-1 pr-2 text-left text-[13px] text-[#cccccc] hover:bg-[#2a2d2e]"
        style={{ paddingLeft }}
        onClick={() => setExpanded((value) => !value)}
      >
        {expanded ? <ChevronDown className="mr-1 h-4 w-4" /> : <ChevronRight className="mr-1 h-4 w-4" />}
        <FolderOpen className="mr-2 h-4 w-4 text-[#dcb67a]" />
        <span className="truncate">{entry.name}</span>
      </button>
      {expanded && entry.children?.map((child) => (
        <TreeNode
          key={child.path}
          entry={child}
          depth={depth + 1}
          selectedPath={selectedPath}
          onOpenFile={onOpenFile}
        />
      ))}
    </div>
  );
}

export default function App() {
  const [desktopState, setDesktopState] = useState<DesktopState | null>(null);
  const [tree, setTree] = useState<WorkspaceTreeResponse | null>(null);
  const [activeFile, setActiveFile] = useState<WorkspaceFileResponse | null>(null);
  const [editorContent, setEditorContent] = useState("");
  const [fileDiff, setFileDiff] = useState<WorkspaceDiffResponse | null>(null);
  const [prompt, setPrompt] = useState("");
  const [messages, setMessages] = useState<UiMessage[]>([]);
  const [pendingPermissions, setPendingPermissions] = useState<PendingPermission[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [manualCopilotToken, setManualCopilotToken] = useState("");

  async function loadDesktopState() {
    const state = await invoke<DesktopState>("desktop_state");
    setDesktopState(state);
    return state;
  }

  async function loadWorkspaceTree() {
    const response = await invoke<WorkspaceTreeResponse>("workspace_tree");
    setTree(response);
    return response;
  }

  async function openFile(path: string) {
    const response = await invoke<WorkspaceFileResponse>("read_workspace_file", { path });
    setActiveFile(response);
    setEditorContent(response.content);
    const diff = await invoke<WorkspaceDiffResponse>("workspace_file_diff", { path });
    setFileDiff(diff);
  }

  async function saveActiveFile() {
    if (!activeFile) return;
    setBusy(true);
    setError(null);
    try {
      const saved = await invoke<WorkspaceFileResponse>("save_workspace_file", {
        request: {
          path: activeFile.path,
          content: editorContent,
        },
      });
      setActiveFile(saved);
      setEditorContent(saved.content);
      const diff = await invoke<WorkspaceDiffResponse>("workspace_file_diff", { path: saved.path });
      setFileDiff(diff);
      await loadDesktopState();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function refreshAll() {
    setError(null);
    const [state, workspaceTree] = await Promise.all([loadDesktopState(), loadWorkspaceTree()]);
    if (!activeFile) {
      const firstFile = findFirstFile(workspaceTree.entries);
      if (firstFile) {
        await openFile(firstFile.path);
      }
    } else {
      await openFile(activeFile.path);
    }
    return state;
  }

  useEffect(() => {
    refreshAll().catch((cause) => {
      setError(String(cause));
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "s" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        saveActiveFile().catch((cause) => setError(String(cause)));
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activeFile, editorContent]);

  async function handleProviderChange(providerId: string) {
    if (!desktopState) return;
    const provider = desktopState.providers.find((item) => item.id === providerId);
    const modelId = provider?.defaultModel ?? provider?.models[0]?.id;
    if (!modelId) return;
    const state = await invoke<DesktopState>("set_provider_selection", {
      request: { providerId, modelId },
    });
    setDesktopState(state);
  }

  async function handleModelChange(modelId: string) {
    if (!desktopState) return;
    const state = await invoke<DesktopState>("set_provider_selection", {
      request: { providerId: desktopState.selectedProvider, modelId },
    });
    setDesktopState(state);
  }

  async function handleExecutionPreferences(agentId: string, approvalMode: ApprovalMode) {
    const state = await invoke<DesktopState>("set_execution_preferences", {
      request: { agentId, approvalMode },
    });
    setDesktopState(state);
  }

  async function saveProviderApiKey() {
    if (!desktopState) return;
    setBusy(true);
    setError(null);
    try {
      const state = await invoke<DesktopState>("set_provider_api_key", {
        request: {
          providerId: desktopState.selectedProvider,
          apiKey: apiKeyInput,
        },
      });
      setDesktopState(state);
      setApiKeyInput("");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function loginCopilot() {
    setBusy(true);
    setError(null);
    try {
      await invoke("auth_login_copilot", {
        githubToken: manualCopilotToken || null,
        noVerify: false,
      });
      setManualCopilotToken("");
      await loadDesktopState();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function logoutCopilot() {
    setBusy(true);
    setError(null);
    try {
      const authState = await invoke<DesktopAuthState>("auth_logout_copilot");
      setDesktopState((current) => (current ? { ...current, copilotAuth: authState } : current));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function handleSend() {
    if (!prompt.trim() || !desktopState) return;
    setBusy(true);
    setError(null);
    try {
      const response = await invoke<RunAgentResponse>("run_agent", {
        request: {
          prompt,
          agent: desktopState.selectedAgent,
          provider: desktopState.selectedProvider,
          model: desktopState.selectedModel,
          cwd: desktopState.cwd,
          approvalMode: desktopState.approvalMode,
        },
      });
      setMessages(response.messages);
      setPendingPermissions(response.pendingPermissions);
      setPrompt("");
      await loadDesktopState();
      if (activeFile) {
        await openFile(activeFile.path);
      }
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  async function approvePendingPermission(permission: PendingPermission) {
    setBusy(true);
    setError(null);
    try {
      const response = await invoke<ExecutePendingToolResponse>("execute_pending_tool", {
        request: {
          tool: permission.tool,
          input: permission.input,
          cwd: desktopState?.cwd,
        },
      });
      setMessages((current) => [
        ...current,
        {
          role: "tool",
          content: `[approved ${response.tool}] ${response.rendered}`,
        },
      ]);
      setPendingPermissions((current) => current.filter((item) => item !== permission));
      if (activeFile) {
        await openFile(activeFile.path);
      }
      await loadDesktopState();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  }

  function rejectPendingPermission(permission: PendingPermission) {
    setPendingPermissions((current) => current.filter((item) => item !== permission));
    setMessages((current) => [
      ...current,
      {
        role: "assistant",
        content: `Rejected pending action: ${permission.tool} -> ${permission.target}`,
      },
    ]);
  }

  const selectedProvider = desktopState?.providers.find(
    (provider) => provider.id === desktopState.selectedProvider,
  );
  const isDirty = !!activeFile && editorContent !== activeFile.content;

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-[#1e1e1e] font-sans text-[#cccccc]">
      <div data-tauri-drag-region className="flex h-8 items-center justify-center bg-[#323233] text-xs select-none">
        {desktopState?.appName ?? "Rovdex"} - {activeFile?.path.split("/").pop() ?? "workspace"}
      </div>

      <div className="flex flex-1 overflow-hidden">
        <div className="flex w-12 flex-shrink-0 flex-col items-center space-y-5 bg-[#333333] py-2">
          <Files className="-ml-[2px] h-6 w-6 cursor-pointer border-l-2 border-white pl-[2px] text-white" strokeWidth={1.5} />
          <Search className="h-[22px] w-[22px] cursor-pointer text-[#858585] hover:text-white" strokeWidth={1.5} />
          <GitBranch className="h-[22px] w-[22px] cursor-pointer text-[#858585] hover:text-white" strokeWidth={1.5} />
          <MessageSquare className="h-[22px] w-[22px] cursor-pointer text-[#858585] hover:text-white" strokeWidth={1.5} />
          <div className="flex-1" />
          <TerminalSquare className="mb-2 h-[22px] w-[22px] cursor-pointer text-[#858585] hover:text-white" strokeWidth={1.5} />
          <Settings className="mb-2 h-[22px] w-[22px] cursor-pointer text-[#858585] hover:text-white" strokeWidth={1.5} />
        </div>

        <div className="flex w-72 flex-shrink-0 flex-col bg-[#252526]">
          <div className="border-b border-[#2b2b2b] px-5 py-3 text-[11px] font-semibold tracking-wider text-[#cccccc]">
            EXPLORER
          </div>
          <div className="border-b border-[#2b2b2b] px-3 py-2 text-[11px] text-[#8f8f8f]">
            {tree?.root ?? desktopState?.repositoryRoot ?? desktopState?.cwd ?? "Loading workspace..."}
          </div>
          <div className="flex-1 overflow-y-auto py-1">
            {tree?.entries.map((entry) => (
              <TreeNode
                key={entry.path}
                entry={entry}
                depth={0}
                selectedPath={activeFile?.path}
                onOpenFile={(path) => {
                  openFile(path).catch((cause) => setError(String(cause)));
                }}
              />
            ))}
          </div>
        </div>

        <div className="flex min-w-0 flex-1 flex-col border-l border-[#2b2b2b] bg-[#1e1e1e]">
          <div className="flex h-9 bg-[#252526]">
            <div className="group flex items-center border-t border-[#007fd4] bg-[#1e1e1e] px-3 text-[13px] text-[#cccccc]">
              <span className="mr-2 text-sm text-[#519aba]">•</span>
              {activeFile?.path.split("/").pop() ?? "welcome.txt"}
              {isDirty && <span className="ml-2 text-[#dcb67a]">●</span>}
              <div className="ml-3 flex h-5 w-5 items-center justify-center rounded hover:bg-[#333333]">
                <X className="h-3.5 w-3.5 opacity-0 transition-opacity group-hover:opacity-100" />
              </div>
            </div>
            <button
              className="ml-auto mr-2 mt-1 flex h-7 items-center gap-1 rounded px-2 text-[12px] text-[#cccccc] hover:bg-[#333333]"
              disabled={!activeFile || !isDirty || busy}
              onClick={() => {
                saveActiveFile().catch((cause) => setError(String(cause)));
              }}
            >
              <Save className="h-3.5 w-3.5" />
              Save
            </button>
          </div>

          <div className="relative flex-1">
            <Editor
              height="100%"
              language={activeFile?.language ?? "plaintext"}
              theme="vs-dark"
              value={editorContent}
              onChange={(value) => setEditorContent(value ?? "")}
              options={{
                minimap: { enabled: false },
                fontSize: 14,
                lineHeight: 24,
                fontFamily: "JetBrains Mono, Menlo, Monaco, Consolas, monospace",
                padding: { top: 12 },
                scrollBeyondLastLine: false,
                smoothScrolling: true,
                cursorBlinking: "smooth",
                cursorSmoothCaretAnimation: "on",
              }}
            />
          </div>
          <div className="max-h-56 overflow-y-auto border-t border-[#2b2b2b] bg-[#181818] px-4 py-3 text-[12px]">
            <div className="mb-2 flex items-center justify-between text-[#8f8f8f]">
              <span>CURRENT FILE DIFF</span>
              <span>
                {fileDiff?.changed ? `${fileDiff.status.length} change markers` : "clean"}
              </span>
            </div>
            {fileDiff?.status.length ? (
              <div className="mb-2 flex flex-wrap gap-2">
                {fileDiff.status.map((entry) => (
                  <span key={entry} className="rounded bg-[#2a2d2e] px-2 py-1 text-[#cccccc]">
                    {entry}
                  </span>
                ))}
              </div>
            ) : null}
            <pre className="whitespace-pre-wrap font-mono leading-5 text-[#bdbdbd]">
              {fileDiff?.diff?.trim() || "No git diff for the active file."}
            </pre>
          </div>
        </div>

        <div className="flex w-[360px] flex-shrink-0 flex-col border-l border-[#2b2b2b] bg-[#252526]">
          <div className="flex h-9 items-center justify-between px-4 text-[11px] font-semibold tracking-wider">
            <span className="text-[#cccccc]">CHAT</span>
            <button className="rounded p-1 hover:bg-[#333333]" onClick={() => refreshAll().catch((cause) => setError(String(cause)))}>
              <RefreshCw className="h-4 w-4 text-[#cccccc]" />
            </button>
          </div>

          <div className="border-y border-[#2b2b2b] p-3 text-[12px]">
            <div className="mb-2 flex items-center gap-2 text-[#8f8f8f]">
              <Zap className="h-3.5 w-3.5 text-[#007acc]" />
              Execution
            </div>
            <div className="grid grid-cols-2 gap-2">
              <select
                className="rounded border border-[#3c3c3c] bg-[#1f1f1f] px-2 py-2 text-[12px] outline-none"
                value={desktopState?.selectedAgent ?? "build"}
                onChange={(event) => {
                  handleExecutionPreferences(
                    event.target.value,
                    desktopState?.approvalMode ?? "manual",
                  ).catch((cause) => setError(String(cause)));
                }}
              >
                {desktopState?.agents.map((agent) => (
                  <option key={agent.id} value={agent.id}>
                    {agent.id}
                  </option>
                ))}
              </select>
              <select
                className="rounded border border-[#3c3c3c] bg-[#1f1f1f] px-2 py-2 text-[12px] outline-none"
                value={desktopState?.approvalMode ?? "manual"}
                onChange={(event) => {
                  handleExecutionPreferences(
                    desktopState?.selectedAgent ?? "build",
                    event.target.value as ApprovalMode,
                  ).catch((cause) => setError(String(cause)));
                }}
              >
                <option value="manual">manual approval</option>
                <option value="auto">auto execute</option>
              </select>
            </div>
            <div className="mt-2 text-[11px] text-[#8f8f8f]">
              {desktopState?.agents.find((agent) => agent.id === desktopState.selectedAgent)?.description}
            </div>

            <div className="mt-3 mb-2 flex items-center gap-2 text-[#8f8f8f]">
              <Zap className="h-3.5 w-3.5 text-[#007acc]" />
              Model provider
            </div>
            <div className="grid grid-cols-2 gap-2">
              <select
                className="rounded border border-[#3c3c3c] bg-[#1f1f1f] px-2 py-2 text-[12px] outline-none"
                value={desktopState?.selectedProvider ?? ""}
                onChange={(event) => {
                  handleProviderChange(event.target.value).catch((cause) => setError(String(cause)));
                }}
              >
                {desktopState?.providers.map((provider) => (
                  <option key={provider.id} value={provider.id}>
                    {provider.id}
                  </option>
                ))}
              </select>
              <select
                className="rounded border border-[#3c3c3c] bg-[#1f1f1f] px-2 py-2 text-[12px] outline-none"
                value={desktopState?.selectedModel ?? ""}
                onChange={(event) => {
                  handleModelChange(event.target.value).catch((cause) => setError(String(cause)));
                }}
              >
                {selectedProvider?.models.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.id}
                  </option>
                ))}
              </select>
            </div>
            <div className="mt-2 text-[11px] text-[#8f8f8f]">
              {selectedProvider?.label}
              {selectedProvider?.authenticated ? " · ready" : " · needs credentials"}
            </div>

            {selectedProvider && selectedProvider.id !== "local" && !selectedProvider.authenticated && (
              <div className="mt-3 rounded border border-[#3c3c3c] bg-[#1f1f1f] p-2">
                <div className="mb-2 flex items-center gap-2 text-[11px] text-[#cccccc]">
                  <KeyRound className="h-3.5 w-3.5 text-[#dcb67a]" />
                  Provider API key
                </div>
                <input
                  className="w-full rounded border border-[#3c3c3c] bg-[#111111] px-2 py-2 text-[12px] outline-none"
                  placeholder="Paste API key"
                  type="password"
                  value={apiKeyInput}
                  onChange={(event) => setApiKeyInput(event.target.value)}
                />
                <button
                  className="mt-2 rounded bg-[#007acc] px-3 py-1.5 text-[12px] text-white hover:bg-[#006bb3]"
                  disabled={busy}
                  onClick={() => {
                    saveProviderApiKey().catch((cause) => setError(String(cause)));
                  }}
                >
                  Save key
                </button>
              </div>
            )}

            <div className="mt-3 rounded border border-[#3c3c3c] bg-[#1f1f1f] p-2">
              <div className="mb-1 text-[11px] text-[#cccccc]">GitHub Copilot auth</div>
              <div className="mb-2 text-[11px] text-[#8f8f8f]">
                {desktopState?.copilotAuth.stored
                  ? `Connected${desktopState.copilotAuth.source ? ` · ${desktopState.copilotAuth.source}` : ""}`
                  : "Not connected"}
              </div>
              <input
                className="w-full rounded border border-[#3c3c3c] bg-[#111111] px-2 py-2 text-[12px] outline-none"
                placeholder="Optional GitHub token"
                type="password"
                value={manualCopilotToken}
                onChange={(event) => setManualCopilotToken(event.target.value)}
              />
              <div className="mt-2 flex gap-2">
                <button
                  className="rounded bg-[#007acc] px-3 py-1.5 text-[12px] text-white hover:bg-[#006bb3]"
                  disabled={busy}
                  onClick={() => {
                    loginCopilot().catch((cause) => setError(String(cause)));
                  }}
                >
                  Login
                </button>
                <button
                  className="rounded border border-[#3c3c3c] px-3 py-1.5 text-[12px] text-[#cccccc] hover:bg-[#2a2d2e]"
                  disabled={busy}
                  onClick={() => {
                    logoutCopilot().catch((cause) => setError(String(cause)));
                  }}
                >
                  Logout
                </button>
              </div>
            </div>

            <div className="mt-3 rounded border border-[#3c3c3c] bg-[#1f1f1f] p-2 text-[11px] text-[#8f8f8f]">
              {desktopState?.approvalMode === "manual"
                ? "Manual approval will stop write and bash actions for review before execution."
                : "Auto execute will allow write and bash actions directly unless the selected agent denies them."}
            </div>
          </div>

          <div className="flex-1 overflow-y-auto p-4 text-sm">
            {messages.length === 0 && (
              <div className="rounded border border-[#3c3c3c] bg-[#1f1f1f] p-3 text-[#cccccc]">
                Start with a real provider/model selection, then ask Rovdex to inspect files, explain code,
                or modify the repository using the tools in `rovdex-core`.
              </div>
            )}

            <div className="space-y-5">
              {messages.map((message, index) => (
                <div key={`${message.role}-${index}`} className="text-sm">
                  <div className="mb-2 flex items-center font-semibold text-[#cccccc]">
                    <div className={`mr-2 flex h-6 w-6 items-center justify-center rounded ${message.role === "user" ? "bg-stone-700" : "bg-[#007acc]"}`}>
                      {message.role === "user" ? <User className="h-3.5 w-3.5 text-white" /> : <Zap className="h-3.5 w-3.5 text-white" />}
                    </div>
                    {message.role === "user" ? "You" : message.role === "assistant" ? "Rovdex AI" : message.role}
                  </div>
                  <pre className="whitespace-pre-wrap pl-8 font-sans leading-6 text-[#cccccc]">{message.content}</pre>
                </div>
              ))}
            </div>

            {pendingPermissions.length > 0 && (
              <div className="mt-6 border-t border-[#2b2b2b] pt-4">
                <div className="mb-2 text-[11px] font-semibold tracking-wider text-[#8f8f8f]">PENDING APPROVAL</div>
                <div className="space-y-2 text-[12px]">
                  {pendingPermissions.map((permission, index) => (
                    <div key={`${permission.tool}-${permission.target}-${index}`} className="rounded border border-[#3c3c3c] bg-[#1f1f1f] p-2">
                      <div className="text-[#cccccc]">{permission.tool}</div>
                      <div className="mt-1 text-[#8f8f8f]">{permission.scope} · {permission.target}</div>
                      <pre className="mt-2 whitespace-pre-wrap rounded bg-[#161616] p-2 font-mono text-[11px] leading-5 text-[#bdbdbd]">
                        {permission.preview}
                      </pre>
                      <div className="mt-2 flex gap-2">
                        <button
                          className="rounded bg-[#007acc] px-3 py-1 text-[12px] text-white hover:bg-[#006bb3]"
                          disabled={busy}
                          onClick={() => {
                            approvePendingPermission(permission).catch((cause) => setError(String(cause)));
                          }}
                        >
                          Approve and run
                        </button>
                        <button
                          className="rounded border border-[#3c3c3c] px-3 py-1 text-[12px] text-[#cccccc] hover:bg-[#2a2d2e]"
                          disabled={busy}
                          onClick={() => rejectPendingPermission(permission)}
                        >
                          Reject
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {desktopState && desktopState.sessions.length > 0 && (
              <div className="mt-6 border-t border-[#2b2b2b] pt-4">
                <div className="mb-2 text-[11px] font-semibold tracking-wider text-[#8f8f8f]">RECENT SESSIONS</div>
                <div className="space-y-2 text-[12px]">
                  {desktopState.sessions.slice(0, 5).map((session) => (
                    <div key={session.id} className="rounded border border-[#3c3c3c] bg-[#1f1f1f] p-2">
                      <div className="text-[#cccccc]">{session.provider}/{session.model}</div>
                      <div className="mt-1 line-clamp-2 text-[#8f8f8f]">{session.finalMessagePreview}</div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          <div className="border-t border-[#3c3c3c] p-4 pt-2">
            <div className="relative">
              <textarea
                className="w-full resize-none rounded border border-transparent bg-[#3c3c3c] px-3 py-2 pr-8 text-[13px] text-[#cccccc] outline-none transition-colors placeholder:text-[#858585] focus:border-[#007fd4]"
                rows={4}
                placeholder="Ask Rovdex to inspect, edit, refactor, or explain this codebase..."
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                    handleSend().catch((cause) => setError(String(cause)));
                  }
                }}
              />
              <button
                className="absolute bottom-2 right-2 rounded p-1 text-white transition-colors hover:bg-[#505050]"
                disabled={busy}
                onClick={() => {
                  handleSend().catch((cause) => setError(String(cause)));
                }}
              >
                <MessageSquare className="h-3.5 w-3.5 text-[#007acc]" />
              </button>
            </div>
            {error && <div className="mt-2 text-[11px] text-[#ff8b8b]">{error}</div>}
          </div>
        </div>
      </div>

      <div className="flex h-[22px] flex-shrink-0 items-center bg-[#007acc] px-3 text-[11px] text-white select-none">
        <GitBranch className="mr-1 h-3.5 w-3.5" /> main
        <div className="flex-1" />
        <span className="mr-4 flex h-full items-center px-2">{activeFile?.language ?? "plaintext"}</span>
        <span className="mr-4 flex h-full items-center px-2">{desktopState?.selectedAgent ?? "build"}</span>
        <span className="mr-4 flex h-full items-center px-2">{desktopState?.selectedProvider ?? "provider"}</span>
        <span className="mr-4 flex h-full items-center px-2">{isDirty ? "Unsaved changes" : "Saved"}</span>
        <span className="flex h-full items-center px-2"><Zap className="mr-1 h-3 w-3" /> Rovdex AI</span>
      </div>
    </div>
  );
}

function findFirstFile(entries: WorkspaceEntry[]): WorkspaceEntry | undefined {
  for (const entry of entries) {
    if (entry.kind === "file") return entry;
    if (entry.children?.length) {
      const nested = findFirstFile(entry.children);
      if (nested) return nested;
    }
  }
  return undefined;
}
