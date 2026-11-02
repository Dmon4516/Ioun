use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

// This module defines the commands related to application settings in Reign.
// The `load_settings` command retrieves the settings from the store, while the `save_settings` command saves the provided settings to the store.
const STORE_FILE: &str = "settings.json";
// The `SETTINGS_KEY` constant is used as the key for storing and retrieving settings in the store.
const SETTINGS_KEY: &str = "settings";

// The `load_settings` command is an asynchronous function that takes an `AppHandle` as a parameter.
// It retrieves the store associated with the application and attempts to get the settings using the
//`SETTINGS_KEY`. If successful, it returns the settings as an `Option<serde_json::Value>`. If there
// is an error while accessing the store, it returns an error message as a `String`.
#[tauri::command]
pub async fn load_settings(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    Ok(store.get(SETTINGS_KEY))
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: serde_json::Value) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(SETTINGS_KEY, settings);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
