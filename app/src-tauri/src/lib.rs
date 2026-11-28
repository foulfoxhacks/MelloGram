//! MelloGram app entry point, shared by desktop (`main.rs`) and mobile
//! (Android/iOS load this library and call the mobile entry point).
// No hand-written unsafe code; Tauri macro expansions are exempt.
#![deny(unsafe_code)]

mod activity;
mod background;
mod secrets;
mod traffic;
mod commands;
mod e2e;
mod media;
mod network;
mod extras;
mod mellorpc;
mod state;
mod wallpapers;

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

    let builder = tauri::Builder::default().plugin(tauri_plugin_notification::init());
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec!["--minimized"]),
    ));
    builder
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let state = Arc::new(state::AppState::open(app.handle().clone(), dir)?);
            app.manage(Arc::clone(&state));
            // Clear any presence left by an interrupted previous session.
            state.publish_presence();
            let rpc = state.store.lock().unwrap().settings.rpc.clone();
            mellorpc::start(Arc::clone(&state), rpc.native_enabled, rpc.discord_compat_enabled);
            background::setup(app.handle(), &state)?;
            // Reconnect in the background if credentials are already saved.
            let connection_state = Arc::clone(&state);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = connection_state.ensure_connected(crate::state::CONNECT_TIMEOUT).await {
                    tracing::warn!("auto-connect failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            wallpapers::get_wallpapers,
            wallpapers::set_wallpapers,
            wallpapers::wallpaper_presets,
            wallpapers::wallpaper_preview_css,
            wallpapers::list_wallpaper_images,
            wallpapers::import_wallpaper_image,
            wallpapers::delete_wallpaper_image,
            commands::app_info,
            e2e::e2e_status,
            e2e::e2e_identity,
            e2e::e2e_reset_identity,
            e2e::e2e_import_peer,
            e2e::e2e_forget_peer,
            e2e::e2e_chat_state,
            e2e::e2e_set_chat,
            e2e::e2e_verify_peer,
            e2e::e2e_clear_history,
            e2e::e2e_group_state,
            e2e::e2e_group_enable,
            e2e::e2e_group_disable,
            e2e::e2e_group_rotate,
            e2e::e2e_group_share,
            e2e::e2e_group_request_keys,
            e2e::e2e_group_sweep,
            commands::membership_log,
            commands::chat_info,
            commands::chat_members,
            media::media_file,
            media::media_lottie,
            media::chat_photo,
            activity::traffic_log,
            activity::traffic_clear,
            activity::wakatime_status,
            activity::wakatime_set_key,
            activity::wakatime_heartbeat,
            activity::melloid_badges,
            commands::check_for_update,
            network::get_network_settings,
            network::set_network_settings,
            media::save_media,
            media::send_location,
            media::get_media_settings,
            media::set_media_settings,
            media::clear_media_cache,
            commands::get_melloid_profile,
            commands::save_melloid_profile,
            commands::get_settings,
            commands::set_general,
            commands::set_notifications,
            commands::set_rpc_settings,
            commands::set_credentials,
            commands::login_state,
            commands::request_code,
            commands::resend_code,
            commands::cancel_code,
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
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if background::intercept_close(window) {
                    api.prevent_close();
                }
            }
            if let tauri::WindowEvent::Focused(focused) = event {
                background::set_mobile_focus(*focused);
                if *focused {
                    background::window_focused_again(window.app_handle());
                }
            }
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(state) = window.app_handle().try_state::<Arc<state::AppState>>() {
                    state.shutdown();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MelloGram");
}
