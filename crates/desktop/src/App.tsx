import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Terminal, Copy, MessageSquare, Files, FolderTree } from "lucide-react";

export default function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <div className="flex h-screen w-screen flex-col bg-stone-950 text-stone-300">
      {/* TitleBar - macOS drag area */}
      <div 
        data-tauri-drag-region 
        className="flex h-10 w-full items-center justify-center border-b border-stone-800/60 bg-stone-900/40 text-xs font-semibold text-stone-400 select-none"
      >
        Rovdex AI (Beta)
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <div className="flex w-12 flex-col items-center border-r border-stone-800/60 bg-stone-900/60 py-4 space-y-6 text-stone-500">
          <Files className="w-5 h-5 hover:text-stone-200 cursor-pointer" />
          <FolderTree className="w-5 h-5 hover:text-stone-200 cursor-pointer" />
          <Terminal className="w-5 h-5 hover:text-stone-200 cursor-pointer" />
        </div>

        {/* Main Editor Area */}
        <div className="flex flex-1 flex-col bg-stone-950 px-6 py-6 font-mono text-sm leading-loose">
          <div className="text-stone-500">// Welcome to Rovdex AI</div>
          <div className="mt-4 text-stone-300">
            <p>Start prompting to generate and edit files.</p>
            <div className="mt-8 flex gap-2">
              <input 
                className="bg-stone-900 border border-stone-800 rounded px-3 py-1 outline-none focus:border-stone-600 transition" 
                autoFocus 
                placeholder="Type your name..." 
                value={name} 
                onChange={(e) => setName(e.target.value)} 
              />
              <button 
                className="bg-indigo-600/20 text-indigo-400 border border-indigo-500/30 rounded px-4 py-1 hover:bg-indigo-600/30 transition"
                onClick={greet}
              >
                Greet
              </button>
            </div>
            {greetMsg && (
              <p className="mt-4 text-emerald-400">{greetMsg}</p>
            )}
          </div>
        </div>

        {/* AI Assistant Right Panel */}
        <div className="w-80 border-l border-stone-800/60 bg-stone-900/30 flex flex-col">
          <div className="flex h-10 items-center justify-between border-b border-stone-800/60 px-4 text-xs font-medium text-stone-400">
            <span className="flex items-center gap-2"><MessageSquare className="w-4 h-4"/> Assistant</span>
          </div>
          <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-4 text-sm">
            <div className="bg-stone-800/40 rounded-lg p-3 text-stone-300 border border-stone-700/50">
              Hello! I am Rovdex, your AI pair programmer. How can I help you today?
            </div>
          </div>
          <div className="p-4 border-t border-stone-800/60 bg-stone-900/50">
            <textarea 
              className="w-full bg-stone-950 border border-stone-800 rounded-lg p-3 text-sm placeholder:text-stone-600 outline-none focus:border-stone-600 resize-none h-24"
              placeholder="Ask a question or request a change (Cmd+Enter to send)..."
            />
          </div>
        </div>
      </div>
    </div>
  );
}
