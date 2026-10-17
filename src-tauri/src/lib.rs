mod controller;
mod devices;
mod dsp;
mod engine;
mod errors;
mod settings;
mod sync;

use std::sync::{Arc, Mutex};

use controller::{Controller, Shared, StateView};
use settings::SettingsStore;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, LogicalSize, Manager, State, Theme, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
use tauri_plugin_opener::OpenerExt;

const HIDDEN_ARG: &str = "--minimized";

struct StartHidden(bool);

fn update<F: FnOnce(&mut Controller)>(app: &AppHandle, state: &State<Shared>, change: F) -> StateView {
    let view = {
        let mut c = state.lock().unwrap();
        change(&mut c);
        c.settings.save();
        c.view()
    };
    controller::emit(app, &state.inner().clone());
    view
}

#[tauri::command]
fn get_state(state: State<Shared>) -> StateView {
    state.lock().unwrap().view()
}

#[tauri::command]
fn set_target(app: AppHandle, state: State<Shared>, id: String, enabled: bool) -> StateView {
    update(&app, &state, |c| {
        let targets = &mut c.settings.data.targets;
        targets.retain(|t| t != &id);
        if enabled {
            targets.push(id);
        }
    })
}

#[tauri::command]
fn set_stereo(app: AppHandle, state: State<Shared>, enabled: bool) -> StateView {
    update(&app, &state, |c| {
        c.settings.data.stereo = enabled;
        c.apply_sides();
    })
}

#[tauri::command]
fn swap_sides(app: AppHandle, state: State<Shared>) -> StateView {
    update(&app, &state, |c| {
        c.settings.data.swap = !c.settings.data.swap;
        c.apply_sides();
    })
}

#[tauri::command]
fn set_auto_sync(app: AppHandle, state: State<Shared>, enabled: bool) -> StateView {
    update(&app, &state, |c| c.settings.data.auto_sync = enabled)
}

#[tauri::command]
fn set_sync_sound(app: AppHandle, state: State<Shared>, sound: String) -> StateView {
    update(&app, &state, |c| c.settings.data.sync_sound = if sound == "chime" { sound } else { "silent".into() })
}

#[tauri::command]
fn set_offset(app: AppHandle, state: State<Shared>, id: String, ms: f32) -> StateView {
    update(&app, &state, |c| {
        let ms = ms.clamp(0.0, 500.0);
        c.settings.data.offsets.insert(id.clone(), ms);
        c.set_engine_offset(&id, ms);
    })
}

#[tauri::command]
fn sync_now(app: AppHandle, state: State<Shared>) -> Result<(), String> {
    controller::start_sync(app, state.inner().clone())
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let launcher = app.autolaunch();
    if enabled { launcher.enable() } else { launcher.disable() }
        .map_err(|e| errors::report("start with Windows", e, "Couldn't change the start-with-Windows setting."))?;
    Ok(launcher.is_enabled().unwrap_or(false))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    debug_logging: bool,
    log_dir: String,
    theme: String,
}

fn log_level(debug: bool) -> log::LevelFilter {
    if debug { log::LevelFilter::Debug } else { log::LevelFilter::Info }
}

#[tauri::command]
fn get_app_info(app: AppHandle, state: State<Shared>) -> AppInfo {
    let settings = state.lock().unwrap().settings.data.clone();
    AppInfo {
        version: app.package_info().version.to_string(),
        debug_logging: settings.debug_logging,
        log_dir: app.path().app_log_dir().map(|p| p.display().to_string()).unwrap_or_default(),
        theme: settings.theme,
    }
}

fn apply_theme(app: &AppHandle, theme: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(match theme {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            _ => None,
        });
    }
}

#[tauri::command]
fn set_theme(app: AppHandle, state: State<Shared>, theme: String) -> String {
    let theme = match theme.as_str() {
        "light" | "dark" => theme,
        _ => "system".to_string(),
    };
    let mut c = state.lock().unwrap();
    c.settings.data.theme = theme.clone();
    c.settings.save();
    apply_theme(&app, &theme);
    theme
}

#[tauri::command]
fn set_debug_logging(state: State<Shared>, enabled: bool) -> bool {
    let mut c = state.lock().unwrap();
    c.settings.data.debug_logging = enabled;
    c.settings.save();
    log::set_max_level(log_level(enabled));
    log::info!("debug logging {}", if enabled { "on" } else { "off" });
    enabled
}

#[tauri::command]
fn open_log_folder(app: AppHandle) -> Result<(), String> {
    let failed = |e: String| errors::report("open log folder", e, "Couldn't open the log folder.");
    let dir = app.path().app_log_dir().map_err(|e| failed(e.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|e| failed(e.to_string()))?;
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| failed(e.to_string()))
}

#[tauri::command]
fn open_sound_settings(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("ms-settings:sound", None::<&str>)
        .map_err(|e| errors::report("open sound settings", e, "Couldn't open Windows sound settings."))
}

#[tauri::command]
fn open_vb_cable_page(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("https://vb-audio.com/Cable/", None::<&str>)
        .map_err(|e| errors::report("open VB-Cable page", e, "Couldn't open the browser. Go to vb-audio.com/Cable to download it."))
}

#[tauri::command]
fn reset_offsets(app: AppHandle, state: State<Shared>) -> StateView {
    log::info!("speaker delays reset");
    update(&app, &state, |c| c.reset_offsets())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    log::info!("quit from menu");
    app.exit(0);
}

#[tauri::command]
fn fit_window(window: WebviewWindow, start_hidden: State<StartHidden>, height: f64, first: bool) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let width = window.inner_size().map_err(|e| e.to_string())?.to_logical::<f64>(scale).width;
    let limit = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| m.work_area().size.to_logical::<f64>(m.scale_factor()).height - 60.0)
        .unwrap_or(900.0);
    window
        .set_size(LogicalSize::new(width, height.min(limit).max(300.0)))
        .map_err(|e| e.to_string())?;
    if first {
        let _ = window.center();
        if !start_hidden.0 {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
    Ok(())
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "Sync speakers now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &sync, &quit])?;
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("app icon"))
        .tooltip("Duet")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_window(app),
            "sync" => {
                let shared = app.state::<Shared>().inner().clone();
                let _ = controller::start_sync(app.clone(), shared);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![HIDDEN_ARG])))
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(Target::new(TargetKind::LogDir { file_name: Some("duet".into()) }))
                .target(Target::new(TargetKind::Stdout))
                .level(log::LevelFilter::Debug)
                .level_for("wasapi", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .max_file_size(2_000_000)
                .rotation_strategy(RotationStrategy::KeepOne)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let settings = SettingsStore::load(&dir);
            log::set_max_level(log_level(settings.data.debug_logging));
            log::info!("Duet {} started", app.package_info().version);
            apply_theme(app.handle(), &settings.data.theme);
            let shared: Shared = Arc::new(Mutex::new(Controller::new(settings)));
            app.manage(shared.clone());
            build_tray(app.handle())?;
            controller::run_supervisor(app.handle().clone(), shared);
            app.manage(StartHidden(std::env::args().any(|a| a == HIDDEN_ARG)));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_target,
            set_stereo,
            swap_sides,
            set_auto_sync,
            set_sync_sound,
            set_offset,
            sync_now,
            get_autostart,
            set_autostart,
            get_app_info,
            set_debug_logging,
            set_theme,
            open_log_folder,
            open_sound_settings,
            open_vb_cable_page,
            reset_offsets,
            quit_app,
            fit_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
