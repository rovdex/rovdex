use std::{fs, path::PathBuf, process::Command};

use anyhow::{anyhow, Result};

use crate::{Context, Tool, ToolResult, ToolSpec};

#[derive(Debug, Default)]
pub struct CurrentDirectoryTool;

impl Tool for CurrentDirectoryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "current_directory",
            "Show the working directory and repository root",
        )
    }

    fn call(&self, context: &Context, _input: &str) -> Result<ToolResult> {
        let repository_root = context
            .repository_root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<none>".to_string());

        Ok(ToolResult::new(format!(
            "cwd: {}\nrepository_root: {}",
            context.cwd.display(),
            repository_root
        )))
    }
}

#[derive(Debug, Default)]
pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("list_directory", "List files in a directory")
    }

    fn call(&self, context: &Context, input: &str) -> Result<ToolResult> {
        let path = resolve_path(context, input);
        let mut entries = fs::read_dir(&path)?.collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        let mut output = String::new();
        for entry in entries {
            let file_type = entry.file_type()?;
            let suffix = if file_type.is_dir() { "/" } else { "" };
            output.push_str(&format!(
                "{}{}\n",
                entry.file_name().to_string_lossy(),
                suffix
            ));
        }

        Ok(ToolResult::new(output.trim_end()))
    }
}

#[derive(Debug, Default)]
pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("read_file", "Read a text file from the workspace")
    }

    fn call(&self, context: &Context, input: &str) -> Result<ToolResult> {
        let path = resolve_path(context, input);
        let content = fs::read_to_string(&path)
            .map_err(|error| anyhow!("failed to read {}: {error}", path.display()))?;
        Ok(ToolResult::new(content))
    }
}

#[derive(Debug, Default)]
pub struct GitStatusTool;

impl Tool for GitStatusTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("git_status", "Show git status for the repository")
    }

    fn call(&self, context: &Context, _input: &str) -> Result<ToolResult> {
        let root = context
            .repository_root
            .as_ref()
            .ok_or_else(|| anyhow!("git_status requires a git repository"))?;

        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("status")
            .arg("--short")
            .output()?;

        if !output.status.success() {
            return Err(anyhow!(
                "git status failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(ToolResult::new(if stdout.is_empty() {
            String::from("clean")
        } else {
            stdout
        }))
    }
}

fn resolve_path(context: &Context, input: &str) -> PathBuf {
    let path = PathBuf::from(input.trim());
    if path.as_os_str().is_empty() {
        return context.cwd.clone();
    }

    if path.is_absolute() {
        path
    } else {
        context.cwd.join(path)
    }
}
