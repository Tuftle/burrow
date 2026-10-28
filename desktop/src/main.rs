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

// ---- updates ---------------------------------------------------------------------------------------------------

/// Updates only ever come from Burrow's own GitHub releases.
const RELEASES: &str = "https://github.com/Tuftle/burrow/releases/";

fn url_ok(url: &str) -> bool {
    url.starts_with(RELEASES)
        && url.len() < 300
        && !url.contains("..")
        && url.chars().all(|c| c.is_ascii_alphanumeric() || "-._~:/+".contains(c))
}

fn sha_ok(sha: &str) -> bool {
    sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit())
}

/// "installed" (in %LOCALAPPDATA%\Programs\Burrow, where the installer puts it), "portable", or "other".
fn install_kind() -> &'static str {
    let Ok(exe) = std::env::current_exe() else { return "other" };
    if exe.file_name().map(|n| n.to_string_lossy().to_lowercase().contains("portable")).unwrap_or(false) {
        return "portable";
    }
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        if exe.starts_with(PathBuf::from(base).join("Programs").join("Burrow")) {
            return "installed";
        }
    }
    "other"
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn app_install_kind() -> String {
    install_kind().to_string()
}

/// Downloads the new installer, checks it against the checksum announced with it, then starts it and closes Burrow.
/// The installer waits for Burrow to be gone, replaces the program files, keeps the vault and opens Burrow again.
#[tauri::command]
async fn install_update(app: tauri::AppHandle, url: String, sha256: String) -> Result<(), String> {
    if install_kind() != "installed" {
        return Err("this copy was not installed by the installer, so it cannot update itself".into());
    }
    if !url_ok(&url) || !sha_ok(&sha256) {
        return Err("that update is not from burrow's own releases".into());
    }
    let dir = std::env::temp_dir().join("burrow-update");
    let file = dir.join("Burrow-Setup.exe");
    let (u, f, d) = (url.clone(), file.clone(), dir.clone());
    let got = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        use std::os::windows::process::CommandExt;
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let script = format!(
            "$ProgressPreference='SilentlyContinue'; [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12;              Invoke-WebRequest -UseBasicParsing -Uri '{}' -OutFile '{}'; (Get-FileHash -Algorithm SHA256 -LiteralPath '{}').Hash",
            u, f.display(), f.display()
        );
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|e| format!("could not start the download: {e}"))?;
        if !out.status.success() {
            return Err("the download did not finish".into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_lowercase())
    })
    .await
    .map_err(|e| e.to_string())??;
    if got != sha256.to_lowercase() {
        let _ = fs::remove_dir_all(&dir);
        return Err("the download did not match its checksum, so it was thrown away".into());
    }
    std::process::Command::new(&file).args(["--install", "--update"]).spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_burrows_own_releases_are_trusted() {
        assert!(url_ok("https://github.com/Tuftle/burrow/releases/download/v0.2.0/Burrow-Setup.exe"));
        assert!(url_ok("https://github.com/Tuftle/burrow/releases/latest/download/Burrow-Setup.exe"));
        assert!(!url_ok("http://github.com/Tuftle/burrow/releases/download/v1/x.exe"));
        assert!(!url_ok("https://github.com/Tuftle/wren/releases/download/v1/x.exe"));
        assert!(!url_ok("https://github.com/Tuftle/burrow/releases/../../evil/repo/x.exe"));
        assert!(!url_ok("https://github.com/Tuftle/burrow/releases/x.exe'; calc; '"));
        assert!(!url_ok("https://github.com/Tuftle/burrow/releases/x.exe?a=b&c=d"));
        assert!(!url_ok("https://github.com.evil.net/Tuftle/burrow/releases/x.exe"));
        assert!(!url_ok(""));
    }

    #[test]
    fn checksums_must_be_sha256_hex() {
        assert!(sha_ok("2dbf42822cb96d6e8a17d23177b4251f5cfca78f954d20a43dfebd8045bea5a8"));
        assert!(!sha_ok("2dbf42822cb96d6e8a17d23177b4251f5cfca78f954d20a43dfebd8045bea5a"));
        assert!(!sha_ok("zzzf42822cb96d6e8a17d23177b4251f5cfca78f954d20a43dfebd8045bea5a8"));
        assert!(!sha_ok(""));
    }
}

fn main() {
    // a finished update leaves its installer in the temp folder (it was still running when burrow restarted)
    let _ = fs::remove_dir_all(std::env::temp_dir().join("burrow-update"));
    // the web view keeps its own cache in the same folder, so portable means portable
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", data_dir().join("webview"));
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![load_vault, save_vault, data_location, app_version, app_install_kind, install_update])
        .run(tauri::generate_context!())
        .expect("burrow could not start");
}
