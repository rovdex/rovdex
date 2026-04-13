use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{anyhow, Result};
use glob::glob;
use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;

use crate::{Context, Tool, ToolResult, ToolSpec};

#[derive(Debug, Default)]
pub struct CurrentDirectoryTool;

impl Tool for CurrentDirectoryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new(
            "current_directory",
            "Show the working directory and repository root",
        )
        .with_input_schema(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, _input: &Value) -> Result<ToolResult> {
        let repository_root = context
            .repository_root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<none>".to_string());

        Ok(ToolResult::new(json!({
            "cwd": context.cwd.display().to_string(),
            "repository_root": repository_root,
        })))
    }
}

#[derive(Debug, Default)]
pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("list_directory", "List files in a directory").with_input_schema(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let path = resolve_path(context, input);
        let mut entries = fs::read_dir(&path)?.collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        let mut output = Vec::new();
        for entry in entries {
            let file_type = entry.file_type()?;
            output.push(json!({
                "name": entry.file_name().to_string_lossy().to_string(),
                "kind": if file_type.is_dir() { "directory" } else { "file" },
            }));
        }

        Ok(ToolResult::new(json!({
            "path": path.display().to_string(),
            "entries": output,
        })))
    }
}

#[derive(Debug, Default)]
pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("read_file", "Read a text file from the workspace").with_input_schema(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"],
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let path = resolve_path(context, input);
        let content = fs::read_to_string(&path)
            .map_err(|error| anyhow!("failed to read {}: {error}", path.display()))?;
        Ok(ToolResult::new(json!({
            "path": path.display().to_string(),
            "content": content,
        })))
    }
}

#[derive(Debug, Default)]
pub struct GitStatusTool;

impl Tool for GitStatusTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("git_status", "Show git status for the repository").with_input_schema(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, _input: &Value) -> Result<ToolResult> {
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
        let entries = if stdout.is_empty() {
            Vec::new()
        } else {
            stdout.lines().map(String::from).collect::<Vec<_>>()
        };

        Ok(ToolResult::new(json!({
            "repository_root": root.display().to_string(),
            "clean": entries.is_empty(),
            "entries": entries,
        })))
    }
}

#[derive(Debug, Default)]
pub struct GlobTool;

impl Tool for GlobTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("glob", "Match files using a glob pattern").with_input_schema(json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "base": { "type": "string" }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let base = resolve_named_path(context, input, "base");
        let pattern = required_string(input, "pattern")?;
        let pattern = if Path::new(&pattern).is_absolute() {
            pattern
        } else {
            base.join(pattern).display().to_string()
        };

        let mut matches = Vec::new();
        for entry in glob(&pattern)? {
            let path = entry?;
            matches.push(path.display().to_string());
        }
        matches.sort();

        Ok(ToolResult::new(json!({
            "base": base.display().to_string(),
            "pattern": pattern,
            "matches": matches,
        })))
    }
}

#[derive(Debug, Default)]
pub struct GrepTool;

impl Tool for GrepTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("grep", "Search text in files under a directory").with_input_schema(json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "path": { "type": "string" },
                "is_regex": { "type": "boolean" }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let base = resolve_path(context, input);
        let pattern = required_string(input, "pattern")?;
        let is_regex = input
            .as_object()
            .and_then(|map| map.get("is_regex"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let regex = if is_regex {
            Some(Regex::new(&pattern)?)
        } else {
            None
        };

        let mut matches = Vec::new();
        for entry in WalkDir::new(&base).into_iter().filter_map(std::result::Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let Ok(content) = fs::read_to_string(path) else {
                continue;
            };

            for (index, line) in content.lines().enumerate() {
                let matched = match &regex {
                    Some(regex) => regex.is_match(line),
                    None => line.contains(&pattern),
                };
                if matched {
                    matches.push(json!({
                        "path": path.display().to_string(),
                        "line_number": index + 1,
                        "line": line,
                    }));
                }
            }
        }

        Ok(ToolResult::new(json!({
            "path": base.display().to_string(),
            "pattern": pattern,
            "is_regex": is_regex,
            "matches": matches,
        })))
    }
}

#[derive(Debug, Default)]
pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("write", "Write content to a file in the workspace").with_input_schema(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" },
                "create_parents": { "type": "boolean" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let path = resolve_path(context, input);
        let content = required_string(input, "content")?;
        let create_parents = input
            .as_object()
            .and_then(|map| map.get("create_parents"))
            .and_then(Value::as_bool)
            .unwrap_or(true);

        if create_parents {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(&path, &content)?;

        Ok(ToolResult::new(json!({
            "path": path.display().to_string(),
            "bytes_written": content.len(),
        })))
    }
}

#[derive(Debug, Default)]
pub struct EditFileTool;

impl Tool for EditFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("edit", "Replace text in an existing file").with_input_schema(json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "old_text": { "type": "string" },
                "new_text": { "type": "string" },
                "replace_all": { "type": "boolean" }
            },
            "required": ["path", "old_text", "new_text"],
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let path = resolve_path(context, input);
        let old_text = required_string(input, "old_text")?;
        let new_text = required_string(input, "new_text")?;
        let replace_all = input
            .as_object()
            .and_then(|map| map.get("replace_all"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let content = fs::read_to_string(&path)
            .map_err(|error| anyhow!("failed to read {}: {error}", path.display()))?;
        let occurrences = content.matches(&old_text).count();
        if occurrences == 0 {
            return Err(anyhow!("edit target not found in {}", path.display()));
        }
        if !replace_all && occurrences != 1 {
            return Err(anyhow!(
                "edit target must appear exactly once in {} but found {} matches",
                path.display(),
                occurrences
            ));
        }

        let next = if replace_all {
            content.replace(&old_text, &new_text)
        } else {
            content.replacen(&old_text, &new_text, 1)
        };
        fs::write(&path, next)?;

        Ok(ToolResult::new(json!({
            "path": path.display().to_string(),
            "occurrences": occurrences,
            "replace_all": replace_all,
        })))
    }
}

#[derive(Debug, Default)]
pub struct BashTool;

impl Tool for BashTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec::new("bash", "Run a shell command in the workspace").with_input_schema(json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "cwd": { "type": "string" }
            },
            "required": ["command"],
            "additionalProperties": false
        }))
    }

    fn call(&self, context: &Context, input: &Value) -> Result<ToolResult> {
        let command = required_string(input, "command")?;
        let cwd = resolve_named_path(context, input, "cwd");
        let output = shell_command(&command, &cwd)?.output()?;

        Ok(ToolResult::new(json!({
            "cwd": cwd.display().to_string(),
            "command": command,
            "status": output.status.code(),
            "success": output.status.success(),
            "stdout": String::from_utf8_lossy(&output.stdout).trim().to_string(),
            "stderr": String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })))
    }
}

fn resolve_path(context: &Context, input: &Value) -> PathBuf {
    let raw = match input {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Object(map) => map
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        other => other.as_str().unwrap_or_default().to_string(),
    };

    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() {
        return context.cwd.clone();
    }

    if path.is_absolute() {
        path
    } else {
        context.cwd.join(path)
    }
}

fn resolve_named_path(context: &Context, input: &Value, field: &str) -> PathBuf {
    let raw = input
        .as_object()
        .and_then(|map| map.get(field))
        .and_then(Value::as_str)
        .unwrap_or_default();

    if raw.is_empty() {
        return context.cwd.clone();
    }

    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        context.cwd.join(path)
    }
}

fn required_string(input: &Value, field: &str) -> Result<String> {
    input
        .as_object()
        .and_then(|map| map.get(field))
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| anyhow!("missing required string field: {field}"))
}

fn shell_command(command: &str, cwd: &Path) -> Result<Command> {
    let mut process = if cfg!(windows) {
        let mut command_process = Command::new("cmd");
        command_process.arg("/C").arg(command);
        command_process
    } else {
        let mut command_process = Command::new("sh");
        command_process.arg("-lc").arg(command);
        command_process
    };
    process.current_dir(cwd);
    Ok(process)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rovdex-tools-{unique}"));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn write_and_edit_file_tools_round_trip() {
        let root = temp_dir();
        let context = Context::from_path(&root).expect("context");

        WriteFileTool
            .call(
                &context,
                &json!({
                    "path": "note.txt",
                    "content": "hello world"
                }),
            )
            .expect("write");

        EditFileTool
            .call(
                &context,
                &json!({
                    "path": "note.txt",
                    "old_text": "world",
                    "new_text": "rovdex"
                }),
            )
            .expect("edit");

        let content = fs::read_to_string(root.join("note.txt")).expect("read note");
        assert_eq!(content, "hello rovdex");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn grep_tool_finds_matches() {
        let root = temp_dir();
        fs::write(root.join("app.txt"), "alpha\nbeta\nalpha beta\n").expect("seed");
        let context = Context::from_path(&root).expect("context");

        let result = GrepTool
            .call(
                &context,
                &json!({
                    "path": ".",
                    "pattern": "alpha"
                }),
            )
            .expect("grep");

        assert_eq!(result.output["matches"].as_array().map(Vec::len), Some(2));
        let _ = fs::remove_dir_all(root);
    }
}
