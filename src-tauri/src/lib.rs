mod authentication;
mod benchmark;
mod commands;
mod db;
mod error;
mod hardware;
mod java;
mod logger;
mod minecraft;
mod mods;
mod net;
mod paths;
mod performance;
mod sources;
mod state;
mod system;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri::Manager;
            let st = state::AppState::new().map_err(|e| e.to_string())?;
            app.manage(st);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_hardware,
            commands::get_usage,
            commands::get_paths,
            commands::list_versions,
            commands::fabric_loader_list,
            commands::forge_version_list,
            commands::remove_version,
            commands::list_instances,
            commands::create_instance,
            commands::delete_instance,
            commands::install_instance,
            commands::required_java,
            commands::detect_javas,
            commands::install_java,
            commands::ms_login_start,
            commands::list_accounts,
            commands::add_offline_account,
            commands::set_active_account,
            commands::remove_account,
            commands::launch_game,
            commands::stop_game,
            commands::running_games,
            commands::list_profiles,
            commands::recommend_profile,
            commands::apply_profile,
            commands::save_settings,
            commands::restore_defaults,
            commands::jvm_preview,
            commands::perf_mod_catalog,
            commands::list_mods,
            commands::preview_mod_install,
            commands::preview_profile_mods,
            commands::set_mod_enabled,
            commands::install_mod,
            commands::install_profile_mods,
            commands::remove_mod,
            commands::list_packs,
            commands::import_pack,
            commands::remove_pack,
            commands::run_benchmark,
            commands::list_benchmarks,
            commands::delete_benchmark,
            commands::get_settings,
            commands::set_setting,
            commands::check_update,
            commands::get_sources,
            commands::set_source,
            commands::get_log_tail
        ])
        .run(tauri::generate_context!())
        .expect("SAYNAY Launcher failed to start");
}
