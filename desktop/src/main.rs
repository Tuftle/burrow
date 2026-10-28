// burrow desktop: the same web app in a small native window. The vault lives in a file on disk,
// never in the web view's own storage, so the portable exe can leave no trace behind.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;

/// Portable when the exe's name has "portable" in it (Burrow-Portable.exe), or `portable.txt` or a `burrow-data`
/// folder sits next to it. Otherwise `%APPDATA%\Burrow`.
fn data_dir() -> PathBuf {
    if let Some(exe) = std::env::current_exe().ok() {
        let named = exe.file_name().map(|n| n.to_string_lossy().to_lowercase().contains("portable")).unwrap_or(false);
        if let Some(dir) = exe.parent() {
            if named || dir.join("portable.txt").exists() || dir.join("burrow-data").exists() {
                return dir.join("burrow-data");
            }
        }
    }
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Burrow")
}

#[tauri::command]
fn load_vault() -> Result<Option<String>, String> {
    let file = data_dir().join("vault.json");
    match fs::read_to_string(&file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("could not read the vault file: {e}")),
    }
}

/// Writes next to the old file first, keeps one backup, then swaps, so a crash never leaves half a vault.
#[tauri::command]
fn save_vault(json: String) -> Result<(), String> {
    let dir = data_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("vault.json");
    let tmp = dir.join("vault.json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    if file.exists() {
        let _ = fs::copy(&file, dir.join("vault.json.bak"));
    }
    fs::rename(&tmp, &file).map_err(|e| e.to_string())
}

#[tauri::command]
fn data_location() -> String {
    data_dir().display().to_string()
}

fn main() {
    // the web view keeps its own cache in the same folder, so portable means portable
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", data_dir().join("webview"));
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_vault, save_vault, data_location])
        .run(tauri::generate_context!())
        .expect("burrow could not start");
}
