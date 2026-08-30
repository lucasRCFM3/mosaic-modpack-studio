mod application;
mod commands;
mod domain;
mod error;
mod infrastructure;
mod providers;
mod state;

use commands::*;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = tauri::async_runtime::block_on(AppState::initialize(app.handle()))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            catalog_search,
            catalog_game_versions,
            recommendations_feed,
            recommendations_preview,
            profiles_list,
            profiles_create,
            profiles_duplicate,
            profiles_update,
            profiles_rescan_preview,
            profiles_rescan_apply,
            profiles_remove,
            profiles_choose_folder,
            profiles_export,
            profiles_export_mod_list,
            mods_organization_preview,
            mods_organization_export,
            presets_list,
            presets_create,
            presets_update,
            presets_remove,
            presets_resolve,
            mods_resolve,
            mods_resolve_many,
            mods_install,
            mods_remove,
            mods_open_project,
            settings_get,
            settings_save,
            app_version,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o Mosaic");
}
