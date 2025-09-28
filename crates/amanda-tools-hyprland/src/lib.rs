use std::{error::Error, process::Command};

use amanda_aicore::tools::{Tool, ToolStore};
use amanda_shared::color_eyre::eyre::ContextCompat;
use amanda_shared::color_eyre::{self, Result};
use amanda_shared::{
    async_trait::async_trait,
    serde_json::{self, Value, json},
};

/// Small helper to run `hyprctl` and capture output.
/// Returns (stdout, stderr, success)
fn run_hyprctl_raw(args: &[&str]) -> Result<(String, String, bool)> {
    let output = Command::new("hyprctl").args(args).output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    Ok((stdout, stderr, output.status.success()))
}

/// Runs `hyprctl <args...> -j` and parses the JSON output.
fn run_hyprctl_json(args: &[&str]) -> Result<Value> {
    let mut all_args = Vec::with_capacity(args.len() + 1);
    all_args.extend_from_slice(args);
    all_args.push("-j");

    let (stdout, stderr, ok) = run_hyprctl_raw(&all_args)?;
    if !ok {
        // return Err(format!("hyprctl {:?} failed: {}", &all_args, stderr).into());

        color_eyre::eyre::bail!("hyprctl {:?} failed: {}", &all_args, stderr);
    }

    let parsed: Value = serde_json::from_str(&stdout)?;
    Ok(parsed)
}

/// Tool: Get hyprctl clients -j (with optional filters)
pub struct HyprctlClientsTool;

#[async_trait]
impl Tool for HyprctlClientsTool {
    fn name(&self) -> &str {
        "hypr_clients"
    }

    fn description(&self) -> &str {
        "List Hyprland clients (hyprctl clients -j). Optional filters: class, title, workspace_id, tag."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "class": { "type": "string", "description": "Filter by client class (substring match)" },
                "title": { "type": "string", "description": "Filter by window title (substring match)" },
                "workspace_id": { "type": "integer", "description": "Filter by workspace numeric ID" },
                "tag": { "type": "string", "description": "Filter by window tag (exact match in tags array)" }
            },
            "required": []
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let class_filter = args
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());
        let title_filter = args
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());
        let workspace_id_filter = args.get("workspace_id").and_then(|v| v.as_i64());
        let tag_filter = args.get("tag").and_then(|v| v.as_str());

        let clients_val = run_hyprctl_json(&["clients"])?;
        let mut items = match clients_val {
            Value::Array(arr) => arr,
            other => {
                return Ok(json!({
                    "error": "unexpected clients JSON format",
                    "raw": other
                }));
            }
        };

        // Apply filters
        if let Some(cf) = class_filter {
            items.retain(|c| {
                c.get("class")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase())
                    .map(|s| s.contains(&cf))
                    .unwrap_or(false)
            });
        }
        if let Some(tf) = title_filter {
            items.retain(|c| {
                c.get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase())
                    .map(|s| s.contains(&tf))
                    .unwrap_or(false)
            });
        }
        if let Some(wf) = workspace_id_filter {
            items.retain(|c| {
                c.get("workspace")
                    .and_then(|w| w.get("id"))
                    .and_then(|v| v.as_i64())
                    .map(|id| id == wf)
                    .unwrap_or(false)
            });
        }
        if let Some(tag) = tag_filter {
            items.retain(|c| {
                if let Some(Value::Array(tags)) = c.get("tags") {
                    tags.iter().any(|t| t.as_str() == Some(tag))
                } else {
                    false
                }
            });
        }

        Ok(json!({ "clients": items }))
    }
}

/// Tool: Get hyprctl activewindow -j
pub struct HyprctlActiveWindowTool;

#[async_trait]
impl Tool for HyprctlActiveWindowTool {
    fn name(&self) -> &str {
        "hypr_active_window"
    }

    fn description(&self) -> &str {
        "Get information about the active window (hyprctl activewindow -j)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        let window = run_hyprctl_json(&["activewindow"])?;
        Ok(json!({ "window": window }))
    }
}

/// Tool: Get hyprctl activeworkspace -j
pub struct HyprctlActiveWorkspaceTool;

#[async_trait]
impl Tool for HyprctlActiveWorkspaceTool {
    fn name(&self) -> &str {
        "hypr_active_workspace"
    }

    fn description(&self) -> &str {
        "Get information about the active workspace (hyprctl activeworkspace -j)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        let ws = run_hyprctl_json(&["activeworkspace"])?;
        Ok(json!({ "workspace": ws }))
    }
}

/// Tool: Get hyprctl monitors -j
pub struct HyprctlMonitorsTool;

#[async_trait]
impl Tool for HyprctlMonitorsTool {
    fn name(&self) -> &str {
        "hypr_monitors"
    }

    fn description(&self) -> &str {
        "List monitors (hyprctl monitors -j)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        let monitors = run_hyprctl_json(&["monitors"])?;
        Ok(json!({ "monitors": monitors }))
    }
}

/// Tool: Get hyprctl workspaces -j
pub struct HyprctlWorkspacesTool;

#[async_trait]
impl Tool for HyprctlWorkspacesTool {
    fn name(&self) -> &str {
        "hypr_workspaces"
    }

    fn description(&self) -> &str {
        "List workspaces (hyprctl workspaces -j)."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        let workspaces = run_hyprctl_json(&["workspaces"])?;
        Ok(json!({ "workspaces": workspaces }))
    }
}

/// Tool: Run an arbitrary hyprctl dispatch (safe wrapper)
/// Example: { "subcommand": "focuswindow", "args": ["kitty"] }
pub struct HyprctlDispatchTool;

#[async_trait]
impl Tool for HyprctlDispatchTool {
    fn name(&self) -> &str {
        "hypr_dispatch"
    }

    fn description(&self) -> &str {
        "Run a Hyprland dispatch command: hyprctl dispatch <subcommand> [args...]. Returns stdout/stderr."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subcommand": { "type": "string" },
                "args": {
                    "type": "array",
                    "items": { "type": "string" }
                }
            },
            "required": ["subcommand"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let sub = args
            .get("subcommand")
            .and_then(|v| v.as_str())
            .wrap_err("Missing 'subcommand'")?;
        let arg_list: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let mut hypr_args: Vec<&str> = vec!["dispatch", sub];
        let mut owned: Vec<String> = Vec::new();
        for a in arg_list {
            owned.push(a);
        }
        let owned_refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();
        hypr_args.extend_from_slice(&owned_refs);

        let (stdout, stderr, ok) = run_hyprctl_raw(&hypr_args)?;
        Ok(json!({
            "ok": ok,
            "command": hypr_args,
            "stdout": stdout.trim(),
            "stderr": if stderr.trim().is_empty() { Value::Null } else { Value::String(stderr.trim().to_string()) }
        }))
    }
}

/// Tool: Focus a window by class/title/address using regex.
/// It builds a regex and calls `hyprctl dispatch focuswindow <regex>`.
pub struct HyprctlFocusWindowTool;

#[async_trait]
impl Tool for HyprctlFocusWindowTool {
    fn name(&self) -> &str {
        "hypr_focus_window"
    }

    fn description(&self) -> &str {
        "Focus a window using Hyprland's window selector (e.g., class:<re>, title:<re>, initialclass:<re>, initialtitle:<re>, tag:<re>, pid:<pid>, address:<addr>, activewindow, floating, tiled), or legacy regex via query/target/exact/case_sensitive."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "window": { "type": "string", "description": "Hyprland window selector, e.g., 'class:^kitty$', 'title:Docs', 'initialclass:...', 'initialtitle:...', 'tag:mytag', 'pid:1234', 'address:0x123', 'activewindow', 'floating', 'tiled'." },
                "query": { "type": "string", "description": "Regex or plain text to match" },
                "target": { "type": "string", "enum": ["class", "title", "address"], "description": "Field to match. Hyprland matches against class, title, or address." },
                "exact": { "type": "boolean", "description": "If true, anchors the regex to full-string match (^...$)" },
                "case_sensitive": { "type": "boolean", "description": "If false, prepends (?i) to make the match case-insensitive" }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        if let Some(window) = args.get("window").and_then(|v| v.as_str()) {
            let pattern = window.to_string();
            let (stdout, stderr, ok) = run_hyprctl_raw(&["dispatch", "focuswindow", &pattern])?;
            return Ok(json!({
                "ok": ok,
                "pattern": pattern,
                "target": "window",
                "stdout": stdout.trim(),
                "stderr": if stderr.trim().is_empty() { Value::Null } else { Value::String(stderr.trim().to_string()) }
            }));
        }
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .wrap_err("Provide either 'window' or 'query'")?;
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("class");
        let exact = args.get("exact").and_then(|v| v.as_bool()).unwrap_or(false);
        let case_sensitive = args
            .get("case_sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Build a regex that tries to limit to the chosen target by prefixing (?<=...)
        // Hyprland's focuswindow uses a single regex matched against multiple fields (address, title, class).
        // We'll try to bias the match by adding a hint like "^(?i).*<query>.*$" and expecting user to choose distinct strings.
        // For exact matches, we anchor the pattern.
        let mut pattern = String::new();
        if !case_sensitive {
            pattern.push_str("(?i)");
        }

        // Basic pattern creation; not escaping special chars intentionally to allow regex usage.
        if exact {
            pattern.push('^');
            pattern.push_str(query);
            pattern.push('$');
        } else {
            pattern.push_str(query);
        }

        // For target "address", users should pass the 0x... address. For "class"/"title", matching is best-effort.
        // Run dispatch
        let (stdout, stderr, ok) = run_hyprctl_raw(&["dispatch", "focuswindow", &pattern])?;

        Ok(json!({
            "ok": ok,
            "pattern": pattern,
            "target": target,
            "stdout": stdout.trim(),
            "stderr": if stderr.trim().is_empty() { Value::Null } else { Value::String(stderr.trim().to_string()) }
        }))
    }
}

/// Tool: Move the active window to a workspace (by id or name), optionally follow it.
pub struct HyprctlMoveWindowToWorkspaceTool;

#[async_trait]
impl Tool for HyprctlMoveWindowToWorkspaceTool {
    fn name(&self) -> &str {
        "hypr_move_window_to_workspace"
    }

    fn description(&self) -> &str {
        "Move the active window to a workspace (by id or name). Optionally follow (switch) to it."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": { "type": "string", "description": "Workspace id or name (e.g. '3' or 'code')" },
                "follow": { "type": "boolean", "description": "If true, also switch to the workspace" },
                "silent": { "type": "boolean", "description": "Use movetoworkspacesilent if true" }
            },
            "required": ["target"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .wrap_err("Missing 'target'")?;
        let follow = args.get("follow").and_then(|v| v.as_bool()).unwrap_or(true);
        let silent = args
            .get("silent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let subcmd = if silent {
            "movetoworkspacesilent"
        } else {
            "movetoworkspace"
        };

        let (mv_out, mv_err, mv_ok) = run_hyprctl_raw(&["dispatch", subcmd, target])?;

        let (ws_out, ws_err, ws_ok) = if follow {
            run_hyprctl_raw(&["dispatch", "workspace", target])?
        } else {
            (String::new(), String::new(), true)
        };

        Ok(json!({
            "move": {
                "ok": mv_ok,
                "stdout": mv_out.trim(),
                "stderr": if mv_err.trim().is_empty() { Value::Null } else { Value::String(mv_err.trim().to_string()) }
            },
            "follow": {
                "executed": follow,
                "ok": ws_ok,
                "stdout": ws_out.trim(),
                "stderr": if ws_err.trim().is_empty() { Value::Null } else { Value::String(ws_err.trim().to_string()) }
            }
        }))
    }
}

/// Tool: Switch to a workspace (by id or name)
pub struct HyprctlSwitchWorkspaceTool;

#[async_trait]
impl Tool for HyprctlSwitchWorkspaceTool {
    fn name(&self) -> &str {
        "hypr_switch_workspace"
    }

    fn description(&self) -> &str {
        "Switch to a workspace by id or name using `hyprctl dispatch workspace <target>`."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": { "type": "string", "description": "Workspace id or name (e.g. '1' or 'web')" }
            },
            "required": ["target"]
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let target = args
            .get("target")
            .and_then(|v| v.as_str())
            .wrap_err("Missing 'target'")?;
        let (stdout, stderr, ok) = run_hyprctl_raw(&["dispatch", "workspace", target])?;
        Ok(json!({
            "ok": ok,
            "stdout": stdout.trim(),
            "stderr": if stderr.trim().is_empty() { Value::Null } else { Value::String(stderr.trim().to_string()) }
        }))
    }
}

/// Tool: Get windows on current workspace (convenience).
/// Internally calls `activeworkspace -j` and `clients -j` and filters.
pub struct HyprctlCurrentWorkspaceClientsTool;

#[async_trait]
impl Tool for HyprctlCurrentWorkspaceClientsTool {
    fn name(&self) -> &str {
        "hypr_current_workspace_clients"
    }

    fn description(&self) -> &str {
        "List clients on the active workspace."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "class": { "type": "string", "description": "Optional substring filter by class" },
                "title": { "type": "string", "description": "Optional substring filter by title" }
            },
            "required": []
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let class_filter = args
            .get("class")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());
        let title_filter = args
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());

        let ws = run_hyprctl_json(&["activeworkspace"])?;
        let ws_id = ws
            .get("id")
            .and_then(|v| v.as_i64())
            .wrap_err("Could not read active workspace id")?;

        let clients_val = run_hyprctl_json(&["clients"])?;
        let mut items = match clients_val {
            Value::Array(arr) => arr,
            other => return Ok(json!({"error": "unexpected clients format", "raw": other})),
        };

        items.retain(|c| {
            c.get("workspace")
                .and_then(|w| w.get("id"))
                .and_then(|v| v.as_i64())
                == Some(ws_id)
        });

        if let Some(cf) = class_filter {
            items.retain(|c| {
                c.get("class")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase())
                    .map(|s| s.contains(&cf))
                    .unwrap_or(false)
            });
        }
        if let Some(tf) = title_filter {
            items.retain(|c| {
                c.get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_lowercase())
                    .map(|s| s.contains(&tf))
                    .unwrap_or(false)
            });
        }

        Ok(json!({
            "workspace_id": ws_id,
            "clients": items
        }))
    }
}

/// Registers all Hyprland tools into your ToolHelper.
pub fn register_hyprland_tools(helper: &mut ToolStore) {
    helper.add_tool(HyprctlClientsTool);
    helper.add_tool(HyprctlActiveWindowTool);
    helper.add_tool(HyprctlActiveWorkspaceTool);
    helper.add_tool(HyprctlMonitorsTool);
    helper.add_tool(HyprctlWorkspacesTool);
    helper.add_tool(HyprctlDispatchTool);
    helper.add_tool(HyprctlFocusWindowTool);
    helper.add_tool(HyprctlMoveWindowToWorkspaceTool);
    helper.add_tool(HyprctlSwitchWorkspaceTool);
    helper.add_tool(HyprctlCurrentWorkspaceClientsTool);
}
