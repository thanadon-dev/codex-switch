use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};
use uuid::Uuid;
use walkdir::WalkDir;

const USAGE_ENDPOINT: &str = "https://chatgpt.com/backend-api/wham/usage";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    id: String,
    name: String,
    color: String,
    project_path: Option<String>,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Settings {
    live_quota_enabled: bool,
    refresh_seconds: u64,
    minimize_to_tray: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileView {
    profile: Profile,
    signed_in: bool,
    email: Option<String>,
    plan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaWindow {
    used_percent: f64,
    window_minutes: Option<i64>,
    resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaSnapshot {
    profile_id: String,
    source: String,
    plan: Option<String>,
    primary: Option<QuotaWindow>,
    secondary: Option<QuotaWindow>,
    fetched_at: i64,
    stale: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProfileInput {
    name: String,
    color: String,
    project_path: Option<String>,
    import_current: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProfileInput {
    id: String,
    name: String,
    color: String,
    project_path: Option<String>,
}

fn app_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| error.to_string())
}

fn profiles_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_root(app)?.join("profiles"))
}

fn profile_home(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(profiles_root(app)?.join(id))
}

fn metadata_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(profile_home(app, id)?.join("profile.json"))
}

fn current_codex_home() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(value));
    }
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| "ไม่พบโฟลเดอร์ผู้ใช้".to_string())?;
    Ok(PathBuf::from(home).join(".codex"))
}

fn default_project_path() -> Result<String, String> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or_else(|| "ไม่พบโฟลเดอร์ผู้ใช้".to_string())?;
    let projects = PathBuf::from(home).join("Projects");
    fs::create_dir_all(&projects).map_err(|error| error.to_string())?;
    Ok(projects.to_string_lossy().into_owned())
}

fn resolve_project_path(project_path: Option<String>) -> Result<Option<String>, String> {
    match project_path.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => Ok(Some(value)),
        _ => Ok(Some(default_project_path()?)),
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn quota_cache_path(app: &AppHandle, id: &str) -> Result<PathBuf, String> {
    Ok(profile_home(app, id)?.join("quota-cache.json"))
}

fn cache_quota(app: &AppHandle, snapshot: &QuotaSnapshot) -> Result<(), String> {
    write_json(&quota_cache_path(app, &snapshot.profile_id)?, snapshot)
}

fn decode_jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn auth_context(path: &Path) -> Result<(String, String, Option<String>, Option<String>), String> {
    let auth = read_json(path)?;
    let tokens = auth
        .get("tokens")
        .and_then(Value::as_object)
        .ok_or_else(|| "บัญชีนี้ไม่ได้ใช้ ChatGPT login".to_string())?;
    let access_token = tokens
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "ไม่พบ access token".to_string())?
        .to_string();
    let claims = decode_jwt_claims(&access_token).unwrap_or(Value::Null);
    let auth_claims = claims
        .get("https://api.openai.com/auth")
        .unwrap_or(&Value::Null);
    let account_id = tokens
        .get("account_id")
        .and_then(Value::as_str)
        .or_else(|| auth_claims.get("chatgpt_account_id").and_then(Value::as_str))
        .or_else(|| {
            auth_claims
                .get("organizations")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find(|item| item.get("is_default").and_then(Value::as_bool) == Some(true))
                        .or_else(|| items.first())
                })
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str)
        })
        .ok_or_else(|| "ไม่พบ ChatGPT account id".to_string())?
        .to_string();
    let email = claims
        .get("email")
        .and_then(Value::as_str)
        .or_else(|| auth_claims.get("email").and_then(Value::as_str))
        .map(str::to_string);
    let plan = auth_claims
        .get("chatgpt_plan_type")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok((access_token, account_id, email, plan))
}

fn read_profile(app: &AppHandle, id: &str) -> Result<Profile, String> {
    let value = read_json(&metadata_path(app, id)?)?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

fn save_profile(app: &AppHandle, profile: &Profile) -> Result<(), String> {
    write_json(&metadata_path(app, &profile.id)?, profile)
}

fn profile_view(app: &AppHandle, profile: Profile) -> ProfileView {
    let auth_path = profile_home(app, &profile.id)
        .unwrap_or_default()
        .join("auth.json");
    let context = auth_context(&auth_path).ok();
    ProfileView {
        signed_in: auth_path.exists(),
        email: context.as_ref().and_then(|item| item.2.clone()),
        plan: context.as_ref().and_then(|item| item.3.clone()),
        profile,
    }
}

#[tauri::command]
fn list_profiles(app: AppHandle) -> Result<Vec<ProfileView>, String> {
    let root = profiles_root(&app)?;
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut profiles = Vec::new();
    for entry in fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        if let Ok(profile) = read_profile(&app, &entry.file_name().to_string_lossy()) {
            profiles.push(profile_view(&app, profile));
        }
    }
    profiles.sort_by(|left, right| left.profile.created_at.cmp(&right.profile.created_at));
    Ok(profiles)
}

#[tauri::command]
fn create_profile(app: AppHandle, input: CreateProfileInput) -> Result<ProfileView, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("กรุณาตั้งชื่อโปรไฟล์".to_string());
    }
    let import_source = if input.import_current {
        let source = current_codex_home()?.join("auth.json");
        if !source.exists() {
            return Err("ไม่พบ auth.json ของ Codex ปัจจุบัน".to_string());
        }
        Some(source)
    } else {
        None
    };
    let profile = Profile {
        id: Uuid::new_v4().simple().to_string(),
        name: name.to_string(),
        color: input.color,
        project_path: resolve_project_path(input.project_path)?,
        created_at: Utc::now().timestamp(),
    };
    let home = profile_home(&app, &profile.id)?;
    fs::create_dir_all(&home).map_err(|error| error.to_string())?;
    save_profile(&app, &profile)?;
    if let Some(source) = import_source {
        fs::copy(source, home.join("auth.json")).map_err(|error| error.to_string())?;
    }
    Ok(profile_view(&app, profile))
}

#[tauri::command]
fn update_profile(app: AppHandle, input: UpdateProfileInput) -> Result<ProfileView, String> {
    let mut profile = read_profile(&app, &input.id)?;
    profile.name = input.name.trim().to_string();
    profile.color = input.color;
    profile.project_path = resolve_project_path(input.project_path)?;
    save_profile(&app, &profile)?;
    Ok(profile_view(&app, profile))
}

#[tauri::command]
fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    let path = profile_home(&app, &id)?;
    if path.exists() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn terminal_command(home: &Path, project: Option<&str>, login: bool) -> String {
    let mut command = format!("set \"CODEX_HOME={}\"", home.display());
    if let Some(path) = project {
        command.push_str(&format!(" && cd /d \"{}\"", path));
    }
    command.push_str(if login { " && codex login" } else { " && codex" });
    command
}

fn spawn_terminal(command: String) -> Result<(), String> {
    let mut process = Command::new("cmd");
    process.args(["/K", &command]);

    #[cfg(windows)]
    process.creation_flags(CREATE_NEW_CONSOLE);

    process.spawn().map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn login_profile(app: AppHandle, id: String) -> Result<(), String> {
    let profile = read_profile(&app, &id)?;
    let home = profile_home(&app, &id)?;
    spawn_terminal(terminal_command(
        &home,
        profile.project_path.as_deref(),
        true,
    ))
}

#[tauri::command]
fn launch_profile(app: AppHandle, id: String) -> Result<(), String> {
    let profile = read_profile(&app, &id)?;
    let home = profile_home(&app, &id)?;
    if !home.join("auth.json").exists() {
        return Err("โปรไฟล์นี้ยังไม่ได้เข้าสู่ระบบ".to_string());
    }
    spawn_terminal(terminal_command(
        &home,
        profile.project_path.as_deref(),
        false,
    ))
}

fn parse_window(value: Option<&Value>, api_shape: bool) -> Option<QuotaWindow> {
    let object = value?.as_object()?;
    let used_percent = object.get("used_percent")?.as_f64()?;
    let window_minutes = if api_shape {
        object
            .get("limit_window_seconds")
            .and_then(Value::as_i64)
            .map(|seconds| (seconds + 59) / 60)
    } else {
        object.get("window_minutes").and_then(Value::as_i64)
    };
    Some(QuotaWindow {
        used_percent,
        window_minutes,
        resets_at: object.get("reset_at").and_then(Value::as_i64),
    })
}

async fn fetch_live(app: &AppHandle, id: &str) -> Result<QuotaSnapshot, String> {
    let auth_path = profile_home(app, id)?.join("auth.json");
    let (token, account_id, _, auth_plan) = auth_context(&auth_path)?;
    let response = reqwest::Client::new()
        .get(USAGE_ENDPOINT)
        .bearer_auth(token)
        .header("ChatGPT-Account-Id", account_id)
        .header("User-Agent", format!("codex-switch/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Usage API ตอบกลับ {}", status.as_u16()));
    }
    let value: Value = response.json().await.map_err(|error| error.to_string())?;
    let limits = value.get("rate_limit");
    Ok(QuotaSnapshot {
        profile_id: id.to_string(),
        source: "live".to_string(),
        plan: value
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(auth_plan),
        primary: parse_window(limits.and_then(|item| item.get("primary_window")), true),
        secondary: parse_window(limits.and_then(|item| item.get("secondary_window")), true),
        fetched_at: Utc::now().timestamp(),
        stale: false,
        error: None,
    })
}

fn latest_rollout(home: &Path) -> Option<PathBuf> {
    WalkDir::new(home.join("sessions"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("rollout-"))
        .max_by_key(|entry| {
            entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
        .map(|entry| entry.path().to_path_buf())
}

fn fetch_local(app: &AppHandle, id: &str) -> Result<QuotaSnapshot, String> {
    let home = profile_home(app, id)?;
    let path = latest_rollout(&home).ok_or_else(|| "ยังไม่พบข้อมูลโควตาใน session".to_string())?;
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    for line in content.lines().rev() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(limits) = value
            .get("payload")
            .and_then(|item| item.get("rate_limits"))
            .filter(|item| !item.is_null())
        else {
            continue;
        };
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        return Ok(QuotaSnapshot {
            profile_id: id.to_string(),
            source: "local".to_string(),
            plan: limits.get("plan_type").and_then(Value::as_str).map(str::to_string),
            primary: parse_window(limits.get("primary"), false),
            secondary: parse_window(limits.get("secondary"), false),
            fetched_at: timestamp,
            stale: Utc::now().timestamp() - timestamp > 900,
            error: None,
        });
    }
    Err("session ล่าสุดไม่มีข้อมูล rate limit".to_string())
}

#[tauri::command]
async fn refresh_quota(app: AppHandle, id: String, mode: String) -> QuotaSnapshot {
    let result = if mode == "live" {
        fetch_live(&app, &id).await.or_else(|live_error| {
            fetch_local(&app, &id).map_err(|local_error| format!("{live_error}; {local_error}"))
        })
    } else {
        fetch_local(&app, &id)
    };
    let snapshot = result.unwrap_or_else(|error| QuotaSnapshot {
        profile_id: id,
        source: mode,
        plan: None,
        primary: None,
        secondary: None,
        fetched_at: Utc::now().timestamp(),
        stale: true,
        error: Some(error),
    });
    let _ = cache_quota(&app, &snapshot);
    snapshot
}

#[tauri::command]
fn get_cached_quotas(app: AppHandle) -> Result<Vec<QuotaSnapshot>, String> {
    let mut snapshots = Vec::new();
    for profile in list_profiles(app.clone())? {
        let path = quota_cache_path(&app, &profile.profile.id)?;
        if !path.exists() {
            continue;
        }
        if let Ok(value) = read_json(&path) {
            if let Ok(snapshot) = serde_json::from_value(value) {
                snapshots.push(snapshot);
            }
        }
    }
    Ok(snapshots)
}

#[tauri::command]
fn get_default_project_path() -> Result<String, String> {
    default_project_path()
}

fn default_settings() -> Settings {
    Settings {
        live_quota_enabled: false,
        refresh_seconds: 60,
        minimize_to_tray: true,
    }
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let path = app_root(&app)?.join("settings.json");
    if !path.exists() {
        return Ok(default_settings());
    }
    serde_json::from_value(read_json(&path)?).map_err(|error| error.to_string())
}

async fn run_background_monitor(app: AppHandle) {
    loop {
        let settings = get_settings(app.clone()).unwrap_or_else(|_| default_settings());
        let window_visible = app
            .get_webview_window("main")
            .and_then(|window| window.is_visible().ok())
            .unwrap_or(false);
        let foreground_delay = settings.refresh_seconds.clamp(30, 900);
        let delay = if window_visible {
            foreground_delay
        } else {
            foreground_delay.saturating_mul(3).clamp(180, 900)
        };
        if settings.live_quota_enabled {
            if let Ok(profiles) = list_profiles(app.clone()) {
                for profile in profiles.into_iter().filter(|item| item.signed_in) {
                    let id = profile.profile.id;
                    let snapshot = match fetch_live(&app, &id).await {
                        Ok(snapshot) => snapshot,
                        Err(live_error) => fetch_local(&app, &id).unwrap_or_else(|local_error| QuotaSnapshot {
                            profile_id: id.clone(),
                            source: "live".to_string(),
                            plan: None,
                            primary: None,
                            secondary: None,
                            fetched_at: Utc::now().timestamp(),
                            stale: true,
                            error: Some(format!("{live_error}; {local_error}")),
                        }),
                    };
                    let _ = cache_quota(&app, &snapshot);
                    let _ = app.emit("quota-updated", &snapshot);
                }
            }
        }
        tokio_sleep(delay).await;
    }
}

async fn tokio_sleep(seconds: u64) {
    tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(std::time::Duration::from_secs(seconds));
    })
    .await
    .ok();
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    write_json(&app_root(&app)?.join("settings.json"), &settings)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "เปิด Codex Switch", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "ออก", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("missing app icon"))
                .menu(&menu)
                .tooltip("Codex Switch")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            tauri::async_runtime::spawn(run_background_monitor(app.handle().clone()));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            create_profile,
            update_profile,
            delete_profile,
            login_profile,
            launch_profile,
            refresh_quota,
            get_cached_quotas,
            get_default_project_path,
            get_settings,
            save_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Switch");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_live_quota_window() {
        let value = serde_json::json!({
            "used_percent": 42,
            "limit_window_seconds": 18000,
            "reset_at": 4_102_444_800_i64
        });
        let window = parse_window(Some(&value), true).expect("window should parse");
        assert_eq!(window.used_percent, 42.0);
        assert_eq!(window.window_minutes, Some(300));
        assert_eq!(window.resets_at, Some(4_102_444_800));
    }

    #[test]
    fn parses_local_quota_window() {
        let value = serde_json::json!({
            "used_percent": 18.5,
            "window_minutes": 10080,
            "resets_at": 4_103_049_600_i64
        });
        let window = parse_window(Some(&value), false).expect("window should parse");
        assert_eq!(window.used_percent, 18.5);
        assert_eq!(window.window_minutes, Some(10080));
    }

    #[test]
    fn rejects_window_without_usage() {
        let value = serde_json::json!({ "window_minutes": 300 });
        assert!(parse_window(Some(&value), false).is_none());
    }

    #[test]
    fn keeps_explicit_project_path() {
        let value = resolve_project_path(Some("D:\\work".to_string())).expect("path should resolve");
        assert_eq!(value.as_deref(), Some("D:\\work"));
    }

    #[test]
    fn uses_projects_folder_when_project_path_is_blank() {
        let value = resolve_project_path(None)
            .expect("default path should resolve")
            .expect("default path should exist");
        assert!(value.ends_with("Projects"));
        assert!(Path::new(&value).is_dir());
    }

    #[test]
    fn builds_terminal_command_with_spaced_paths() {
        let command = terminal_command(
            Path::new(r"C:\Users\markt\AppData\Roaming\Codex Switch\profiles\main"),
            Some(r"C:\Users\markt\My Projects"),
            false,
        );

        assert_eq!(
            command,
            r#"set "CODEX_HOME=C:\Users\markt\AppData\Roaming\Codex Switch\profiles\main" && cd /d "C:\Users\markt\My Projects" && codex"#
        );
    }
}
