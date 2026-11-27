//! MelloGram app entry point, shared by desktop (`main.rs`) and mobile
//! (Android/iOS load this library and call the mobile entry point).

mod commands;
mod extras;
mod state;

use std::sync::Arc;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mellogram=info,warn".into()),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let state = Arc::new(state::AppState::open(app.handle().clone(), dir)?);
            app.manage(Arc::clone(&state));
            // Reconnect in the background if credentials are already saved.
            tauri::async_runtime::spawn(async move {
                if let Err(e) = state.connect_if_configured().await {
                    tracing::warn!("auto-connect failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::get_melloid_profile,
            commands::save_melloid_profile,
            commands::get_settings,
            commands::set_general,
            commands::set_notifications,
            commands::set_credentials,
            commands::login_state,
            commands::request_code,
            commands::submit_code,
            commands::submit_password,
            commands::logout,
            commands::list_chats,
            commands::load_history,
            commands::send_message,
            commands::typing,
            commands::mark_read,
            commands::mark_all_read,
            commands::delete_messages,
            commands::edit_message,
            commands::list_plugins,
            commands::set_plugin_enabled,
            commands::set_plugin_setting,
            commands::reset_plugin_settings,
            commands::set_plugin_favorite,
            commands::list_commands,
            commands::ui_plugins,
            commands::list_themes,
            commands::get_theme_source,
            commands::install_theme,
            commands::install_theme_url,
            commands::remove_theme,
            commands::set_enabled_themes,
            commands::set_theme_variable,
            commands::set_quick_css,
            commands::get_stylesheet,
            commands::export_backup,
            commands::import_backup,
            extras::get_appearance,
            extras::set_appearance,
            extras::list_workspaces,
            extras::create_workspace,
            extras::switch_workspace,
            extras::duplicate_workspace,
            extras::update_workspace,
            extras::delete_workspace,
            extras::export_profile,
            extras::inspect_profile,
            extras::import_profile,
            extras::lock_status,
            extras::set_lock_pin,
            extras::clear_lock_pin,
            extras::verify_lock_pin,
            extras::set_lock_options,
            extras::plugin_storage_get,
            extras::plugin_storage_set,
            extras::get_diagnostics,
            extras::run_tests,
            extras::support_bundle,
            extras::startup_status,
            extras::restart_app,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(state) = window.app_handle().try_state::<Arc<state::AppState>>() {
                    state.shutdown();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MelloGram");
}
