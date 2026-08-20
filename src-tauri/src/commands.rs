use crate::{domain::*, error::IntoMessage, state::AppState};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn catalog_search(
    state: State<'_, AppState>,
    filters: SearchFilters,
) -> Result<CatalogSearchResult, String> {
    state.catalog.search(filters).await.message()
}

#[tauri::command]
pub async fn catalog_game_versions(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.catalog.game_versions().await.message()
}

#[tauri::command]
pub async fn profiles_list(state: State<'_, AppState>) -> Result<Vec<ModpackProfile>, String> {
    Ok(state.profiles.list().await)
}

#[tauri::command]
pub async fn profiles_create(
    state: State<'_, AppState>,
    input: CreateProfileInput,
) -> Result<ModpackProfile, String> {
    state.profiles.create(input).await.message()
}

#[tauri::command]
pub async fn profiles_update(
    state: State<'_, AppState>,
    profile_id: String,
    input: UpdateProfileInput,
) -> Result<ModpackProfile, String> {
    validate_uuid(&profile_id)?;
    state.profiles.update(&profile_id, input).await.message()
}

#[tauri::command]
pub async fn profiles_remove(state: State<'_, AppState>, profile_id: String) -> Result<(), String> {
    validate_uuid(&profile_id)?;
    state.profiles.remove(&profile_id).await.message()
}

#[tauri::command]
pub async fn profiles_choose_folder() -> Result<Option<String>, String> {
    Ok(rfd::AsyncFileDialog::new()
        .set_title("Escolha a pasta da instância")
        .pick_folder()
        .await
        .map(|folder| folder.path().to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn profiles_export(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Option<String>, String> {
    validate_uuid(&profile_id)?;
    let profile = state.profiles.get(&profile_id).await.message()?;
    let filename = format!("{}.mosaic.json", safe_export_name(&profile.name));
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Exportar lockfile Mosaic")
        .set_file_name(&filename)
        .add_filter("Mosaic lockfile", &["json"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    state
        .profiles
        .export_lockfile(&profile_id, file.path())
        .await
        .message()?;
    Ok(Some(file.path().to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn profiles_export_mod_list(
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<Option<String>, String> {
    validate_uuid(&profile_id)?;
    let profile = state.profiles.get(&profile_id).await.message()?;
    let filename = format!("{}-mods.txt", safe_export_name(&profile.name));
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title("Gerar TXT com a lista de mods")
        .set_file_name(&filename)
        .add_filter("Lista de mods", &["txt"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    state
        .profiles
        .export_mod_list(&profile_id, file.path())
        .await
        .message()?;
    Ok(Some(file.path().to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn presets_list(state: State<'_, AppState>) -> Result<Vec<ModPreset>, String> {
    Ok(state.presets.list().await)
}

#[tauri::command]
pub async fn presets_create(
    state: State<'_, AppState>,
    input: SavePresetInput,
) -> Result<ModPreset, String> {
    state.presets.create(input).await.message()
}

#[tauri::command]
pub async fn presets_update(
    state: State<'_, AppState>,
    preset_id: String,
    input: SavePresetInput,
) -> Result<ModPreset, String> {
    validate_uuid(&preset_id)?;
    state.presets.update(&preset_id, input).await.message()
}

#[tauri::command]
pub async fn presets_remove(state: State<'_, AppState>, preset_id: String) -> Result<(), String> {
    validate_uuid(&preset_id)?;
    state.presets.remove(&preset_id).await.message()
}

#[tauri::command]
pub async fn presets_resolve(
    state: State<'_, AppState>,
    profile_id: String,
    preset_id: String,
    selected_optional: Option<Vec<ProjectRef>>,
) -> Result<ResolutionPlan, String> {
    validate_uuid(&profile_id)?;
    validate_uuid(&preset_id)?;
    let profile = state.profiles.get(&profile_id).await.message()?;
    let preset = state.presets.get(&preset_id).await.message()?;
    let roots = preset
        .entries
        .into_iter()
        .map(|entry| entry.project)
        .collect();
    state
        .resolver
        .resolve_many(&profile, roots, selected_optional.unwrap_or_default())
        .await
        .message()
}

#[tauri::command]
pub async fn mods_resolve(
    state: State<'_, AppState>,
    profile_id: String,
    project: ProjectRef,
    selected_optional: Option<Vec<ProjectRef>>,
) -> Result<ResolutionPlan, String> {
    validate_uuid(&profile_id)?;
    validate_project(&project)?;
    let profile = state.profiles.get(&profile_id).await.message()?;
    state
        .resolver
        .resolve(&profile, project, selected_optional.unwrap_or_default())
        .await
        .message()
}

#[tauri::command]
pub async fn mods_install(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    plan_id: String,
) -> Result<InstallResult, String> {
    validate_uuid(&profile_id)?;
    validate_uuid(&plan_id)?;
    let plan = state.resolver.get_plan(&plan_id).await.message()?;
    let concurrency = state.store.snapshot().await.settings.download_concurrency;
    state
        .downloads
        .install(app, &profile_id, plan, concurrency)
        .await
        .message()
}

#[tauri::command]
pub async fn mods_remove(
    state: State<'_, AppState>,
    profile_id: String,
    project: ProjectRef,
) -> Result<RemoveModResult, String> {
    validate_uuid(&profile_id)?;
    validate_project(&project)?;
    state.removal.remove(&profile_id, &project).await.message()
}

#[tauri::command]
pub async fn mods_open_project(
    state: State<'_, AppState>,
    project: ProjectRef,
) -> Result<(), String> {
    validate_project(&project)?;
    let url = state
        .providers
        .get(project.provider)
        .project_url(&project.project_id)
        .await
        .message()?;
    if !url.starts_with("https://") {
        return Err("O provedor retornou uma URL insegura.".into());
    }
    open::that_detached(url).map_err(|error| format!("Não foi possível abrir o navegador: {error}"))
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.store.snapshot().await.settings;
    Ok(AppSettings {
        curse_forge_configured: state.secrets.get_curseforge_key().message()?.is_some(),
        include_optional_dependencies: settings.include_optional_dependencies,
        download_concurrency: settings.download_concurrency,
        telemetry: false,
    })
}

#[tauri::command]
pub async fn settings_save(
    state: State<'_, AppState>,
    input: SaveSettingsInput,
) -> Result<AppSettings, String> {
    if input.clear_curse_forge_api_key.unwrap_or(false) {
        state.secrets.clear_curseforge_key().message()?;
    } else if let Some(key) = input
        .curse_forge_api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        if key.len() > 512 {
            return Err("A chave da CurseForge é longa demais.".into());
        }
        state.secrets.set_curseforge_key(key).message()?;
    }
    let concurrency = input.download_concurrency.clamp(1, 6);
    state
        .store
        .update(|database| {
            database.settings.include_optional_dependencies = input.include_optional_dependencies;
            database.settings.download_concurrency = concurrency;
        })
        .await
        .message()?;
    settings_get(state).await
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

fn validate_uuid(value: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| "Identificador inválido.".into())
}
fn validate_project(project: &ProjectRef) -> Result<(), String> {
    if project.project_id.is_empty() || project.project_id.len() > 128 {
        Err("Projeto inválido.".into())
    } else {
        Ok(())
    }
}
fn safe_export_name(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    if result.is_empty() {
        "modpack".into()
    } else {
        result
    }
}
