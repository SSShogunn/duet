mod controller;
mod devices;
mod dsp;
mod engine;
mod settings;
mod sync;

use std::sync::{Arc, Mutex};

use controller::{Controller, Shared, StateView};
use settings::SettingsStore;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const HIDDEN_ARG: &str = "--minimized";

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
    if enabled { launcher.enable() } else { launcher.disable() }.map_err(|e| e.to_string())?;
    Ok(launcher.is_enabled().unwrap_or(false))
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
        .tooltip("Audio Split")
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
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let shared: Shared = Arc::new(Mutex::new(Controller::new(SettingsStore::load(&dir))));
            app.manage(shared.clone());
            build_tray(app.handle())?;
            controller::run_supervisor(app.handle().clone(), shared);
            if std::env::args().any(|a| a == HIDDEN_ARG) {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
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
            set_offset,
            sync_now,
            get_autostart,
            set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
