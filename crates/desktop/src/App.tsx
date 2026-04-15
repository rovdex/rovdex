import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import Editor from "@monaco-editor/react";
import { Files, Search, GitBranch, Settings, MessageSquare, ChevronRight, ChevronDown, X, User, Zap, TerminalSquare } from "lucide-react";

export default function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [prompt, setPrompt] = useState("");
  const [expanded, setExpanded] = useState(true);

  const mockCode = `// Welcome to Rovdex Desktop
// This editor is powered by Monaco (the same engine as VS Code).

fn main() {
    println!("Hello, World!");
}

// Ask the Rovdex AI on the right for help!
`;

  async function handleSend() {
    if (!prompt.trim()) return;
    try {
      setGreetMsg(await invoke("greet", { name: prompt }));
    } catch {
      setGreetMsg("Tauri core not connected yet.");
    }
  }

  return (
    <div className="flex h-screen w-screen flex-col bg-[#1e1e1e] text-[#cccccc] font-sans overflow-hidden">
      {/* TitleBar - macOS drag area */}
      <div 
        data-tauri-drag-region 
        className="flex h-8 w-full items-center justify-center bg-[#323233] text-xs select-none"
      >
        Rovdex [Desktop] - src/main.rs
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Activity Bar */}
        <div className="w-12 bg-[#333333] flex flex-col items-center py-2 space-y-5 flex-shrink-0">
          <Files className="w-6 h-6 text-white cursor-pointer border-l-2 border-white pl-[2px] -ml-[2px]" strokeWidth={1.5} />
          <Search className="w-[22px] h-[22px] text-[#858585] hover:text-white cursor-pointer" strokeWidth={1.5} />
          <GitBranch className="w-[22px] h-[22px] text-[#858585] hover:text-white cursor-pointer" strokeWidth={1.5} />
          <MessageSquare className="w-[22px] h-[22px] text-[#858585] hover:text-white cursor-pointer" strokeWidth={1.5} />
          <div className="flex-1" />
          <TerminalSquare className="w-[22px] h-[22px] text-[#858585] hover:text-white cursor-pointer mb-2" strokeWidth={1.5} />
          <Settings className="w-[22px] h-[22px] text-[#858585] hover:text-white cursor-pointer mb-2" strokeWidth={1.5} />
        </div>

        {/* Sidebar (Explorer) */}
        <div className="w-60 bg-[#252526] flex flex-col flex-shrink-0">
          <div className="px-5 py-3 text-[11px] font-semibold tracking-wider text-[#cccccc] flex items-center">
            EXPLORER
          </div>
          <div className="flex-1 overflow-y-auto">
            <div 
              className="px-1 py-1 text-sm font-semibold hover:bg-[#2a2d2e] cursor-pointer flex items-center select-none"
              onClick={() => setExpanded(!expanded)}
            >
              {expanded ? <ChevronDown className="w-4 h-4 mr-1" /> : <ChevronRight className="w-4 h-4 mr-1" />}
              ROVDEX
            </div>
            {expanded && (
              <div className="flex flex-col mt-0.5">
                <div className="pl-6 py-1 text-[13px] hover:bg-[#2a2d2e] cursor-pointer text-[#e2c08d]">Cargo.toml</div>
                <div className="pl-6 py-1 text-[13px] hover:bg-[#2a2d2e] cursor-pointer text-[#e2c08d]">README.md</div>
                <div className="pl-8 py-1 text-[13px] bg-[#37373d] border-l border-[#007fd4] -ml-[1px] cursor-pointer text-[#519aba] flex items-center">
                  src/main.rs
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Main Editor Area */}
        <div className="flex-1 flex flex-col min-w-0 bg-[#1e1e1e] border-l border-[#2b2b2b]">
          {/* Tabs */}
          <div className="flex bg-[#252526] h-9">
            <div className="flex items-center px-3 bg-[#1e1e1e] border-t border-[#007fd4] text-[13px] text-[#cccccc] cursor-pointer group">
              <span className="text-[#519aba] mr-2 text-sm">₹</span> src/main.rs 
              <div className="w-5 h-5 ml-3 flex items-center justify-center hover:bg-[#333333] rounded">
                <X className="w-3.5 h-3.5 opacity-0 group-hover:opacity-100 transition-opacity" />
              </div>
            </div>
          </div>
          {/* Monaco Area */}
          <div className="flex-1 relative">
            <Editor
              height="100%"
              defaultLanguage="rust"
              theme="vs-dark"
              defaultValue={mockCode}
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
        </div>

        {/* Right Panel (AI Assistant Chat) */}
        <div className="w-[320px] bg-[#252526] border-l border-[#2b2b2b] flex flex-col flex-shrink-0">
          <div className="px-4 h-9 text-[11px] font-semibold tracking-wider flex items-center justify-between">
            <span className="flex items-center text-[#cccccc]">CHAT</span>
            <div className="flex gap-2">
              <TerminalSquare className="w-4 h-4 text-[#cccccc] cursor-pointer hover:text-white" />
              <X className="w-4 h-4 text-[#cccccc] cursor-pointer hover:text-white" />
            </div>
          </div>
          
          <div className="flex-1 overflow-y-auto p-4 space-y-6">
            <div className="text-sm">
              <div className="flex items-center text-[#cccccc] font-semibold mb-2">
                <div className="w-6 h-6 rounded bg-[#007acc] flex items-center justify-center mr-2">
                  <Zap className="w-3.5 h-3.5 text-white" />
                </div>
                Rovdex AI
              </div>
              <div className="text-[#cccccc] pl-8 leading-snug">
                Welcome to Rovdex Desktop! I'm integrated directly into this VS Code-style editor. 
                What would you like to build today?
              </div>
            </div>
            {greetMsg && (
              <div className="text-sm pt-2">
                <div className="flex items-center text-[#cccccc] font-semibold mb-2">
                  <div className="w-6 h-6 rounded bg-stone-700 flex items-center justify-center mr-2">
                    <User className="w-3.5 h-3.5 text-white" />
                  </div>
                  You
                </div>
                <div className="text-[#cccccc] pl-8 leading-snug mb-4">
                  {prompt}
                </div>
                <div className="flex items-center text-[#cccccc] font-semibold mb-2">
                  <div className="w-6 h-6 rounded bg-[#007acc] flex items-center justify-center mr-2">
                    <Zap className="w-3.5 h-3.5 text-white" />
                  </div>
                  Rovdex AI
                </div>
                <div className="text-[#cccccc] pl-8 leading-snug">
                  {greetMsg}
                </div>
              </div>
            )}
          </div>

          {/* Prompt Input */}
          <div className="p-4 pt-2 border-t border-[#3c3c3c]">
            <div className="relative">
              <textarea
                className="w-full bg-[#3c3c3c] text-[#cccccc] text-[13px] rounded px-3 py-2 pr-8 outline-none resize-none border border-transparent focus:border-[#007fd4] placeholder-[#858585] transition-colors"
                rows={3}
                placeholder="Ask a question..."
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                    handleSend();
                  }
                }}
              />
              <button 
                onClick={handleSend}
                className="absolute bottom-2 right-2 hover:bg-[#505050] text-white p-1 rounded transition-colors"
              >
                <MessageSquare className="w-3.5 h-3.5 text-[#007acc]" />
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Status Bar */}
      <div className="h-[22px] bg-[#007acc] text-white text-[11px] flex items-center px-3 select-none flex-shrink-0">
        <GitBranch className="w-3.5 h-3.5 mr-1" /> main
        <div className="flex-1" />
        <span className="mr-4 hover:bg-white/20 px-2 cursor-pointer h-full flex items-center">Ln 4, Col 5</span>
        <span className="mr-4 hover:bg-white/20 px-2 cursor-pointer h-full flex items-center">UTF-8</span>
        <span className="mr-4 hover:bg-white/20 px-2 cursor-pointer h-full flex items-center">Rust</span>
        <span className="hover:bg-white/20 px-2 cursor-pointer h-full flex items-center"><Zap className="w-3 h-3 mr-1" /> Rovdex AI Ready</span>
      </div>
    </div>
  );
}
