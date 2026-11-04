mod authentication;
mod benchmark;
mod commands;
mod db;
mod error;
mod hardware;
mod java;
mod minecraft;
mod mods;
mod paths;
mod performance;
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
            commands::list_instances,
            commands::create_instance,
            commands::delete_instance,
            commands::install_instance,
            commands::required_java,
            commands::detect_javas,
            commands::install_java,
            commands::ms_login_start,
            commands::ms_account,
            commands::ms_logout,
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
            commands::check_update
        ])
        .run(tauri::generate_context!())
        .expect("SAYNAY Launcher failed to start");
}
