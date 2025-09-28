use std::collections::HashMap;
use std::env::var;
use std::process::Command;

use amanda_aicore::tools::Tool;
use amanda_shared::async_trait::async_trait;
use amanda_shared::color_eyre::eyre::{self, Context};
use amanda_shared::color_eyre::{Result, eyre::bail};
use amanda_shared::serde_json::{self, Value, json};
use amanda_shared::tokio::sync::Mutex;

// make the LLM have memories!
struct MemoryTool {
    memories: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str {
        "memory_tool"
    }

    fn description(&self) -> &str {
        "A tool to store and retrieve memories. Use 'store' action to save a memory with a key and value. Use 'retrieve' action to get a memory by key.
        cool keys:
         username - the user's name
         nickname - how the user likes to be called
        "
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["store", "retrieve"],
                    "description": "The action to perform: 'store' to save a memory, 'retrieve' to get a memory."
                },
                "key": {
                    "type": "string",
                    "description": "The key for the memory."
                },
                "value": {
                    "type": "string",
                    "description": "The value to store (required for 'store' action)."
                }
            },
            "required": ["action", "key"],
        })
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        let mut mem = self.memories.lock().await;
        let action = args
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre::eyre!("Missing or invalid 'action' field"))?;
        let key = args
            .get("key")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre::eyre!("Missing or invalid 'key' field"))?;

        match action {
            "store" => {
                let value = args.get("value").and_then(Value::as_str).ok_or_else(|| {
                    eyre::eyre!("Missing or invalid 'value' field for 'store' action")
                })?;
                mem.insert(key.to_string(), value.to_string());
                Ok(
                    json!({"status": "success", "message": format!("Memory stored under key '{}'", key)}),
                )
            }
            "retrieve" => {
                if let Some(value) = mem.get(key) {
                    Ok(json!({"status": "success", "key": key, "value": value}))
                } else {
                    Ok(
                        json!({"status": "error", "message": format!("No memory found for key '{}'", key)}),
                    )
                }
            }
            _ => bail!("Invalid action '{}'. Use 'store' or 'retrieve'.", action),
        }
    }
}

struct MusicTool;

#[async_trait]
impl Tool for MusicTool {
    fn name(&self) -> &str {
        "get_current_playing_music"
    }

    fn description(&self) -> &str {
        "Get the current playing music information including title, artist, status, position (microseconds), length (microseconds), and art URL."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _args: Value) -> Result<Value> {
        let output = Command::new("playerctl")
            .arg("metadata")
            .arg("-f")
            .arg(r#"{"title":"{{title}}","artist":"{{artist}}","status":"{{status}}","position":"{{position}}","mpris:length":"{{mpris:length}}","mpris:artUrl":"{{mpris:artUrl}}"}"#)
            .output()?;

        if !output.status.success() {
            return Ok(
                json!({ "error": "No music player is currently running or playerctl failed" }),
            );
        }

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 from playerctl");
        let music_info: Value =
            serde_json::from_str(&stdout).wrap_err("Failed to parse playerctl output as JSON")?;

        Ok(music_info)
    }
}

pub fn register_tools(store: &mut amanda_aicore::tools::ToolStore) {
    store.add_tool(MusicTool);

    let mut memories = HashMap::new();

    // lets add some default memories
    if let Ok(user) = var("USER") {
        memories.insert("username".to_string(), user.clone());
        memories.insert("nickname".to_string(), user);
    }

    if let Ok(desktop) = var("XDG_SESSION_DESKTOP") {
        memories.insert("desktop_environment".to_string(), desktop);
    }

    store.add_tool(MemoryTool {
        memories: Mutex::new(memories),
    });
}
