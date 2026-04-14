# Rovdex

Rovdex is an intelligent coding tool built with Rust, designed to improve how developers understand, write, modify, and manage code. It focuses on real-world engineering workflows, emphasizing performance, responsiveness, stability, and extensibility to support the full path from idea to implementation.

## Overview

Modern software development involves much more than writing code. Developers constantly switch between reading unfamiliar codebases, tracing logic, fixing issues, restructuring modules, generating new functionality, running commands, and coordinating changes across projects.

Rovdex is designed around these practical needs. It provides a more natural and efficient development experience, helping developers work more smoothly across local development, project maintenance, code improvement, and engineering productivity.

## Core Features

### 1. Intelligent Code Understanding
Rovdex helps developers quickly understand code structure, module responsibilities, call flows, and business logic by working with project context rather than isolated snippets.

- Workspace map generation for repository-level summaries
- File and symbol extraction for prompt grounding
- Repository-aware system prompt construction

### 2. Code Generation and Modification
It supports generating code from natural language instructions, as well as updating, completing, fixing, and refactoring existing code for both new development and legacy maintenance.

### 3. Project-Level Context Awareness
Rovdex goes beyond single-file assistance by considering the broader project environment, including directory structure, module dependencies, configuration files, interface definitions, and engineering constraints.

### 4. Command and Task Assistance
It can assist with common development tasks such as generating scripts, organizing command workflows, outlining debugging steps, analyzing build issues, and helping investigate runtime problems.

- Tool-enabled execution loop with file, grep, glob, git status, and shell permissions
- OpenAI-compatible tool calling for remote coding models

### 5. High-Performance Architecture
Built with Rust, Rovdex benefits from strong execution efficiency, reliable resource management, and long-running stability, making it well suited for frequent interaction and engineering integration.

### 6. Extensible Design
Rovdex follows a modular design philosophy, making it easier to extend with additional capabilities in the future, such as plugins, workflow orchestration, project rule injection, and team collaboration features.

## Use Cases

Rovdex is useful for a wide range of development scenarios, including:

- Understanding unfamiliar codebases quickly
- Assisting with daily coding and refactoring
- Investigating errors and locating issues
- Generating scripts, utility code, and interface logic
- Supporting configuration, build, and debugging tasks
- Improving team productivity and engineering consistency

## Advantages

### Engineering-Focused
Rovdex is designed around real development workflows rather than isolated question-and-answer interactions. It pays attention to the relationships between code, commands, directory structure, configuration, and tasks.

### Performance and Stability
With Rust as its foundation, Rovdex is built for speed, reliability, controlled resource usage, and stable long-term execution.

### Built for Evolution
Rovdex is designed with future growth in mind, making it suitable as a long-term tool that can continue to expand alongside engineering needs.

## Design Philosophy

Rovdex is not focused only on the act of writing code. Its goal is to support the full development process, from understanding problems and analyzing context to generating solutions and implementing them effectively.

Its design is guided by four principles:

- **Efficient** — reduce repetitive work and shorten development cycles
- **Reliable** — provide a stable and consistent tool experience
- **Clear** — organize capabilities around engineering context instead of scattered features
- **Open** — support future expansion through richer workflows and integrations

## Road Ahead

Rovdex will continue to evolve around practical engineering intelligence, including areas such as:

- Deeper project-level context analysis
- More precise code change control
- Richer command execution and task flow support
- Stronger plugin and extension capabilities
- Better support for collaborative engineering environments

## Current CLI

Rovdex currently exposes a small but usable CLI surface:

```bash
# Inspect the current repository structure
cargo run -p rovdex-cli -- map

# Show the workspace map as JSON
cargo run -p rovdex-cli -- map --json

# List configured model providers
cargo run -p rovdex-cli -- provider list

# Run a local smoke-test chat flow
cargo run -p rovdex-cli -- chat --provider local --model echo "inspect this workspace"

# List saved sessions
cargo run -p rovdex-cli -- session list

# Show the most recent session as JSON
cargo run -p rovdex-cli -- session show

# Show desktop-oriented platform/data paths
cargo run -p rovdex-cli -- paths

# Import GitHub Copilot authentication using the opencode-style discovery flow
cargo run -p rovdex-cli -- auth login copilot

# Check stored authentication status
cargo run -p rovdex-cli -- auth status copilot

# Remove stored authentication
cargo run -p rovdex-cli -- auth logout copilot
```

## Implementation Direction

This version now combines two useful ideas from existing developer tools:

- `claude-code-sourcemap`: repository reconstruction and source-level structural context
- `opencode`: provider-driven CLI/TUI workflow with agents, tools, and sessions

Rovdex adapts those ideas into a Rust workspace with a typed core engine, tool registry, provider routing, session events, TUI shell, and a built-in workspace map that can be fed directly into model context.

## Desktop Direction

Rovdex is being shaped as a desktop-oriented coding tool for macOS and Windows, not only a terminal utility.

- Platform-aware app path discovery is now built into the core
- Project sessions are stored under the repository in `.rovdex/sessions`
- Desktop/global sessions can be routed to OS-native app data folders

Current OS-native path conventions:

- macOS data/config: `~/Library/Application Support/Rovdex`
- macOS cache: `~/Library/Caches/Rovdex`
- Windows data/config: `%APPDATA%\\Rovdex`
- Windows cache: `%LOCALAPPDATA%\\Rovdex\\Cache`

This gives the codebase a clean foundation for adding a real desktop shell later, such as Tauri or another Rust-native application wrapper.

## Authentication

Rovdex now includes an `opencode`-style GitHub Copilot login path.

Behavior:

- First checks `GITHUB_TOKEN`
- Then checks standard GitHub Copilot local files:
  - Linux/macOS: `~/.config/github-copilot/hosts.json`
  - Linux/macOS: `~/.config/github-copilot/apps.json`
  - Windows: `%LOCALAPPDATA%\\github-copilot\\hosts.json`
  - Windows: `%LOCALAPPDATA%\\github-copilot\\apps.json`
- For Copilot login, Rovdex can verify the GitHub token by exchanging it against:
  - `https://api.github.com/copilot_internal/v2/token`

Stored Rovdex auth state is written to the app config directory as `auth.json`, so desktop builds and CLI builds can share the same local credentials.

## Who It Is For

Rovdex is suitable for:

- Backend engineers
- Frontend engineers
- Full-stack developers
- Tooling engineers
- Technical team leads
- Individual developers and teams looking to improve engineering efficiency

## Summary

Rovdex is an intelligent coding tool built for modern software engineering workflows. With performance, engineering awareness, and extensibility at its core, it helps developers understand code faster, handle tasks more efficiently, and build with greater confidence.
