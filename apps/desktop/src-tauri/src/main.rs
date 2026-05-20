#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;

use backend::launcher::BackendLauncher;
use std::process::Child;
use std::sync::Mutex;
use tauri::{Manager, State};

const BACKEND_PORT: u16 = 47832;

struct AppState {
    child: Mutex<Option<Child>>,
    launcher: BackendLauncher,
}

impl AppState {
    fn new() -> Self {
        Self {
            child: Mutex::new(None),
            launcher: BackendLauncher::new(BACKEND_PORT),
        }
    }
}

#[tauri::command]
fn start_backend(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let mut child = state
        .child
        .lock()
        .map_err(|_| "backend lock poisoned".to_string())?;
    state.launcher.start(&app, &mut child)
}

#[tauri::command]
fn stop_backend(state: State<AppState>) -> Result<(), String> {
    let mut child = state
        .child
        .lock()
        .map_err(|_| "backend lock poisoned".to_string())?;
    state.launcher.stop(&mut child);
    Ok(())
}

#[tauri::command]
fn backend_status(state: State<AppState>) -> Result<String, String> {
    let child = state
        .child
        .lock()
        .map_err(|_| "backend lock poisoned".to_string())?;
    Ok(state.launcher.status(&child))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|app| {
            let state: State<AppState> = app.state();
            let _ = start_backend(app.app_handle().clone(), state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_backend,
            stop_backend,
            backend_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
