use serde::Serialize;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMode {
    pub id: &'static str,
    pub name: &'static str,
    pub protocols: Vec<&'static str>,
    pub authentication: Vec<&'static str>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub modes: Vec<PluginMode>,
}
/// Built-ins are explicit, versioned descriptors; no downloaded code executes in
/// the privileged Rust process. Add a mode by extending the manifest and adapter.
pub fn registry() -> Vec<PluginManifest> {
    let mut plugins = vec![
        PluginManifest {
            schema_version: 1,
            id: "chatgpt",
            name: "ChatGPT",
            version: "1.0.0",
            modes: vec![PluginMode {
                id: "codex",
                name: "Codex",
                protocols: vec!["codex"],
                authentication: vec!["oauth"],
            }],
        },
        PluginManifest {
            schema_version: 1,
            id: "local",
            name: "Local AI",
            version: "1.0.0",
            modes: vec![
                PluginMode {
                    id: "chat",
                    name: "Conversation",
                    protocols: vec!["ollama", "openai"],
                    authentication: vec!["none", "api-key", "headers"],
                },
                PluginMode {
                    id: "images",
                    name: "Images",
                    protocols: vec!["openai", "comfyui"],
                    authentication: vec!["none", "api-key", "headers"],
                },
            ],
        },
        PluginManifest {
            schema_version: 1,
            id: "custom",
            name: "Custom API",
            version: "1.0.0",
            modes: vec![
                PluginMode {
                    id: "chat",
                    name: "Conversation",
                    protocols: vec!["openai", "anthropic", "ollama"],
                    authentication: vec!["none", "api-key", "headers"],
                },
                PluginMode {
                    id: "images",
                    name: "Images",
                    protocols: vec!["openai", "comfyui"],
                    authentication: vec!["none", "api-key", "headers"],
                },
            ],
        },
    ];
    for (id, name) in [
        ("antigravity", "Antigravity"),
        ("claude-code", "Claude Code"),
        ("grok", "Grok"),
        ("minimax", "MiniMax"),
        ("kimi", "Kimi"),
    ] {
        plugins.push(PluginManifest {
            schema_version: 1,
            id,
            name,
            version: "1.0.0",
            modes: vec![PluginMode {
                id: "account",
                name: "Compte",
                protocols: vec!["cli"],
                authentication: vec!["oauth"],
            }],
        });
    }
    for (id, name) in [("zai", "Z.ai · GLM"), ("opencode", "OpenCode Go / Zen")] {
        plugins.push(PluginManifest {
            schema_version: 1,
            id,
            name,
            version: "1.0.0",
            modes: vec![PluginMode {
                id: "api",
                name: "API",
                protocols: vec!["openai", "anthropic", "responses"],
                authentication: if id == "zai" {
                    vec!["oauth", "api-key"]
                } else {
                    vec!["api-key"]
                },
            }],
        });
    }
    plugins
}
pub fn accepts(plugin: &str, protocol: &str) -> bool {
    registry()
        .iter()
        .any(|p| p.id == plugin && p.modes.iter().any(|m| m.protocols.contains(&protocol)))
}
#[tauri::command]
pub fn plugin_catalogue() -> Vec<PluginManifest> {
    registry()
}
