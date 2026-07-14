mod crypto;
mod error;
mod models;
mod vault;

use crate::{
    error::{AppError, AppResult},
    models::{
        AppSettings, BackupPreview, Entry, EntryInput, EntrySummary, EntryType, EntryTypeInput,
        ImportMode, ImportReport, ListOptions, PasswordOptions, VaultState,
    },
    vault::VaultService,
};
use std::{path::PathBuf, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

struct AppState {
    vault: Mutex<VaultService>,
}

fn with_vault<T>(
    state: &State<'_, AppState>,
    action: impl FnOnce(&mut VaultService) -> AppResult<T>,
) -> AppResult<T> {
    let mut vault = state
        .vault
        .lock()
        .map_err(|_| AppError::new("INTERNAL", "密码库状态不可用"))?;
    action(&mut vault)
}

#[tauri::command]
fn get_vault_state(state: State<'_, AppState>) -> AppResult<VaultState> {
    with_vault(&state, VaultService::state)
}

#[tauri::command]
fn initialize_vault(master_password: String, state: State<'_, AppState>) -> AppResult<()> {
    with_vault(&state, |vault| vault.initialize(master_password))
}

#[tauri::command]
fn restore_new_vault(
    path: String,
    backup_password: String,
    new_master_password: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    with_vault(&state, |vault| {
        vault.restore_new_vault(&PathBuf::from(path), backup_password, new_master_password)
    })
}

#[tauri::command]
fn unlock_vault(master_password: String, state: State<'_, AppState>) -> AppResult<()> {
    with_vault(&state, |vault| vault.unlock(master_password))
}

#[tauri::command]
fn lock_vault(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    with_vault(&state, |vault| {
        vault.lock();
        Ok(())
    })?;
    let _ = app.clipboard().clear();
    Ok(())
}

#[tauri::command]
fn list_entries(options: ListOptions, state: State<'_, AppState>) -> AppResult<Vec<EntrySummary>> {
    with_vault(&state, |vault| vault.list_entries(options))
}

#[tauri::command]
fn list_entry_types(state: State<'_, AppState>) -> AppResult<Vec<EntryType>> {
    with_vault(&state, VaultService::list_entry_types)
}

#[tauri::command]
fn create_entry_type(input: EntryTypeInput, state: State<'_, AppState>) -> AppResult<EntryType> {
    with_vault(&state, |vault| vault.create_entry_type(input))
}

#[tauri::command]
fn delete_entry_type(id: String, state: State<'_, AppState>) -> AppResult<()> {
    with_vault(&state, |vault| vault.delete_entry_type(&id))
}

#[tauri::command]
fn get_entry(id: String, state: State<'_, AppState>) -> AppResult<Entry> {
    with_vault(&state, |vault| vault.get_entry(&id))
}

#[tauri::command]
fn create_entry(input: EntryInput, state: State<'_, AppState>) -> AppResult<Entry> {
    with_vault(&state, |vault| vault.create_entry(input))
}

#[tauri::command]
fn update_entry(id: String, input: EntryInput, state: State<'_, AppState>) -> AppResult<Entry> {
    with_vault(&state, |vault| vault.update_entry(&id, input))
}

#[tauri::command]
fn delete_entry(id: String, state: State<'_, AppState>) -> AppResult<()> {
    with_vault(&state, |vault| vault.delete_entry(&id))
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    with_vault(&state, |vault| vault.get_settings())
}

#[tauri::command]
fn update_settings(settings: AppSettings, state: State<'_, AppState>) -> AppResult<AppSettings> {
    with_vault(&state, |vault| vault.update_settings(settings))
}

#[tauri::command]
fn change_master_password(
    current_password: String,
    new_password: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    with_vault(&state, |vault| {
        vault.change_master_password(current_password, new_password)
    })
}

#[tauri::command]
fn generate_password(options: PasswordOptions) -> AppResult<String> {
    VaultService::generate_password(options)
}

#[tauri::command]
fn copy_entry_field(
    app: AppHandle,
    id: String,
    field: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let (value, clear_after) = with_vault(&state, |vault| {
        let value = vault.secret_value(&id, &field)?;
        let settings = vault.get_settings()?;
        Ok((value, settings.clipboard_clear_seconds))
    })?;
    app.clipboard()
        .write_text(value.clone())
        .map_err(|_| AppError::new("CLIPBOARD", "无法写入剪贴板"))?;

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(clear_after as u64)).await;
        if app
            .clipboard()
            .read_text()
            .is_ok_and(|current| current == value)
        {
            let _ = app.clipboard().clear();
        }
    });
    Ok(())
}

#[tauri::command]
fn export_backup(
    path: String,
    backup_password: String,
    state: State<'_, AppState>,
) -> AppResult<BackupPreview> {
    with_vault(&state, |vault| {
        vault.export_backup(&PathBuf::from(path), backup_password)
    })
}

#[tauri::command]
fn inspect_backup(path: String, backup_password: String) -> AppResult<BackupPreview> {
    VaultService::inspect_backup(&PathBuf::from(path), backup_password)
}

#[tauri::command]
fn import_backup(
    path: String,
    backup_password: String,
    mode: ImportMode,
    state: State<'_, AppState>,
) -> AppResult<ImportReport> {
    with_vault(&state, |vault| {
        vault.import_backup(&PathBuf::from(path), backup_password, mode)
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let vault = VaultService::new(data_dir.join("keychains.db"))?;
            app.manage(AppState {
                vault: Mutex::new(vault),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_vault_state,
            initialize_vault,
            restore_new_vault,
            unlock_vault,
            lock_vault,
            list_entries,
            list_entry_types,
            create_entry_type,
            delete_entry_type,
            get_entry,
            create_entry,
            update_entry,
            delete_entry,
            get_settings,
            update_settings,
            change_master_password,
            generate_password,
            copy_entry_field,
            export_backup,
            inspect_backup,
            import_backup
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Keychains");
}
