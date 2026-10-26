mod artifacts;
mod chatgpt;
mod cli_providers;
mod cli_sessions;
mod computer;
mod deepseek;
mod desktop_control;
mod extensions;
mod file_changes;
mod host_environment;
mod permissions;
mod persistence;
mod plugins;
mod providers;
mod search;
mod streaming;
mod terminal;
mod tools;
mod updates;
mod workspaces;
mod zai;
use rusqlite::Connection;
use std::{collections::HashMap, sync::Mutex};
use tauri::Manager;
use tokio_util::sync::CancellationToken;

pub struct AppState {
    db: Mutex<Connection>,
    client: reqwest::Client,
    requests: Mutex<HashMap<String, CancellationToken>>,
    terminal_gate: tokio::sync::Mutex<()>,
    artifacts: Mutex<artifacts::ArtifactStore>,
    computer_target: Mutex<HashMap<String, computer::Session>>,
    permissions: permissions::PermissionManager,
}
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(desktop_control::shortcut_handler)
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .register_uri_scheme_protocol("artifact", |context, request| {
            artifacts::response(context.app_handle(), request.uri().path())
        })
        .setup(|app| {
            app.manage(desktop_control::DesktopControl::default());
            app.manage(updates::Updates::default());
            let directory = app.path().app_data_dir()?;
            app.manage(chatgpt::ChatGPTProvider::new(directory.join("chatgpt")));
            app.manage(cli_providers::CliProviders::new(directory.join("accounts")));
            app.manage(zai::ZaiAuth::default());
            app.manage(deepseek::DeepSeekAuth::default());
            std::fs::create_dir_all(&directory)?;
            let db = persistence::open(&directory.join("converge.sqlite"))?;
            extensions::init(&db)?;
            app.manage(extensions::Extensions::new(directory.join("extensions")));
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(std::time::Duration::from_secs(8))
                .timeout(std::time::Duration::from_secs(900))
                .build()?;
            app.manage(AppState {
                db: Mutex::new(db),
                client,
                requests: Mutex::new(HashMap::new()),

                terminal_gate: tokio::sync::Mutex::new(()),
                artifacts: Mutex::new(Default::default()),
                computer_target: Default::default(),
                permissions: Default::default(),
            });
            tauri::async_runtime::spawn(extensions::mcp::restore_connections(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chatgpt::chatgpt_action,
            cli_providers::cli_provider_action,
            zai::zai_auth_action,
            deepseek::deepseek_auth_action,
            extensions::extensions_snapshot,
            extensions::extension_context,
            extensions::extension_catalog,
            extensions::skill_action,
            extensions::catalogue::skills_catalogue,
            extensions::catalogue::install_catalogue_skill,
            extensions::mcp::mcp_action,
            chatgpt::resolve_codex_tool,
            workspaces::conversation_workspace_context,
            workspaces::choose_conversation_workspace,
            workspaces::remove_conversation_workspace,
            permissions::execute_agent_tool,
            permissions::resolve_tool_permission,
            permissions::revoke_conversation_permissions,
            artifacts::register_artifact,
            artifacts::release_artifact,
            plugins::plugin_catalogue,
            persistence::load_workspace,
            persistence::save_workspace,
            persistence::search_conversations,
            persistence::save_provider,
            persistence::remove_provider,
            providers::discover_models,
            providers::stream_chat,
            providers::cancel_request,
            updates::app_update_check,
            updates::app_update_install,
            desktop_control::computer_control_stop,
            desktop_control::computer_control_finish,
            providers::generate_image,
            tools::read_attachment,
            tools::paste_clipboard_image,
            tools::import_file,
            tools::export_file,
            tools::open_external,
        ])
        .run(tauri::generate_context!())
        .expect("Impossible de démarrer Converge");
}
