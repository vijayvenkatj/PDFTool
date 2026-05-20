#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::net::{SocketAddr, TcpStream};
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Manager, State};

struct BackendState {
  child: Mutex<Option<Child>>,
}

impl BackendState {
  fn new() -> Self {
    Self {
      child: Mutex::new(None),
    }
  }
}

fn backend_binary_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
  if let Ok(explicit) = std::env::var("PDFTOOL_BACKEND_PATH") {
    return Ok(std::path::PathBuf::from(explicit));
  }
  let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
  #[cfg(target_os = "windows")]
  let path = resource_dir.join("bin").join("pdfsvc.exe");
  #[cfg(not(target_os = "windows"))]
  let path = resource_dir.join("bin").join("pdfsvc");
  if path.exists() {
    return Ok(path);
  }
  let mut candidates = Vec::new();
  #[cfg(target_os = "windows")]
  {
    candidates.push(std::path::PathBuf::from("..\\..\\backend\\go-service\\bin\\pdfsvc.exe"));
    candidates.push(std::path::PathBuf::from("..\\..\\..\\backend\\go-service\\bin\\pdfsvc.exe"));
  }
  #[cfg(not(target_os = "windows"))]
  {
    // Stable dev path based on this crate location.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(
      manifest_dir
        .join("../../../backend/go-service/bin/pdfsvc")
        .to_path_buf(),
    );
    candidates.push(std::path::PathBuf::from("../../backend/go-service/bin/pdfsvc"));
    candidates.push(std::path::PathBuf::from("../../../backend/go-service/bin/pdfsvc"));
  }
  for p in candidates {
    if p.exists() {
      return Ok(p);
    }
  }
  Err("backend binary not found; set PDFTOOL_BACKEND_PATH or build backend/go-service/bin/pdfsvc".to_string())
}

fn resolve_tool_env(var_name: &str, candidates: &[&str]) -> Option<String> {
  if let Ok(v) = std::env::var(var_name) {
    if !v.trim().is_empty() {
      return Some(v);
    }
  }
  for candidate in candidates {
    let path = std::path::Path::new(candidate);
    if path.exists() {
      return Some(candidate.to_string());
    }
  }
  None
}

fn resolve_tool_binary(var_name: &str, fallback_name: &str, candidates: &[&str]) -> Option<String> {
  if let Some(path) = resolve_tool_env(var_name, candidates) {
    return Some(path);
  }
  if let Ok(output) = Command::new("which").arg(fallback_name).output() {
    if output.status.success() {
      let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
      if !resolved.is_empty() {
        return Some(resolved);
      }
    }
  }
  None
}

fn backend_port_reachable() -> bool {
  let addr: SocketAddr = match "127.0.0.1:47832".parse() {
    Ok(a) => a,
    Err(_) => return false,
  };
  TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

fn backend_health_ok() -> bool {
  let addr: SocketAddr = match "127.0.0.1:47832".parse() {
    Ok(a) => a,
    Err(_) => return false,
  };
  let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
    Ok(s) => s,
    Err(_) => return false,
  };
  let _ = stream.set_read_timeout(Some(Duration::from_millis(700)));
  let _ = stream.set_write_timeout(Some(Duration::from_millis(700)));
  if stream
    .write_all(b"GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
    .is_err()
  {
    return false;
  }
  let mut buf = [0_u8; 512];
  match stream.read(&mut buf) {
    Ok(n) if n > 0 => String::from_utf8_lossy(&buf[..n]).contains(" 200 "),
    _ => false,
  }
}

#[cfg(target_os = "macos")]
fn kill_stale_backend_on_port() {
  let output = Command::new("lsof")
    .arg("-t")
    .arg("-nP")
    .arg("-iTCP:47832")
    .arg("-sTCP:LISTEN")
    .output();
  let Ok(output) = output else { return };
  if !output.status.success() {
    return;
  }
  let pids = String::from_utf8_lossy(&output.stdout);
  for line in pids.lines() {
    let pid = line.trim();
    if pid.is_empty() {
      continue;
    }
    let _ = Command::new("kill").arg("-9").arg(pid).status();
  }
}

#[tauri::command]
fn start_backend(app: tauri::AppHandle, state: State<BackendState>) -> Result<(), String> {
  let mut guard = state.child.lock().map_err(|_| "backend lock poisoned".to_string())?;
  if let Some(child) = guard.as_mut() {
    match child.try_wait() {
      Ok(Some(_)) => {
        *guard = None;
      }
      Ok(None) => return Ok(()),
      Err(_) => {
        *guard = None;
      }
    }
  }
  if backend_health_ok() {
    return Ok(());
  }
  #[cfg(target_os = "macos")]
  {
    if backend_port_reachable() && !backend_health_ok() {
      kill_stale_backend_on_port();
      std::thread::sleep(Duration::from_millis(150));
    }
  }
  let backend_path = backend_binary_path(&app)?;
  if !backend_path.exists() {
    return Err(format!(
      "backend binary missing at {}. Build it with: cd backend/go-service && make build",
      backend_path.display()
    ));
  }
  let mut cmd = Command::new(backend_path);
  cmd.arg("--port")
    .arg("47832")
    .stdin(Stdio::null());

  let log_path = std::env::temp_dir().join("pdftool-backend.log");
  let log_file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&log_path)
    .map_err(|e| format!("failed to open backend log file {}: {e}", log_path.display()))?;
  let log_file_err = log_file
    .try_clone()
    .map_err(|e| format!("failed to clone backend log file handle: {e}"))?;
  cmd.stdout(Stdio::from(log_file));
  cmd.stderr(Stdio::from(log_file_err));

  #[cfg(target_os = "macos")]
  {
    if let Some(qpdf_path) = resolve_tool_binary(
      "PDFTOOL_QPDF_PATH",
      "qpdf",
      &["/opt/homebrew/bin/qpdf", "/usr/local/bin/qpdf"],
    ) {
      cmd.env("PDFTOOL_QPDF_PATH", qpdf_path.clone());
      cmd.arg("--qpdf").arg(qpdf_path);
    }
    if let Some(gs_path) = resolve_tool_binary(
      "PDFTOOL_GS_PATH",
      "gs",
      &[
        "/opt/homebrew/bin/gs",
        "/usr/local/bin/gs",
      ],
    ) {
      cmd.env("PDFTOOL_GS_PATH", gs_path.clone());
      cmd.arg("--gs").arg(gs_path);
    }
  }

  let child = cmd
    .spawn()
    .map_err(|e| format!("failed to launch backend: {e}"))?;
  let mut child = child;
  std::thread::sleep(Duration::from_millis(220));
  if let Ok(Some(status)) = child.try_wait() {
    return Err(format!(
      "backend exited immediately with status {status}. Check log: {}",
      log_path.display()
    ));
  }
  *guard = Some(child);
  Ok(())
}

#[tauri::command]
fn stop_backend(state: State<BackendState>) -> Result<(), String> {
  let mut guard = state.child.lock().map_err(|_| "backend lock poisoned".to_string())?;
  if let Some(mut child) = guard.take() {
    let _ = child.kill();
    let _ = child.wait();
  }
  Ok(())
}

#[tauri::command]
fn backend_status(state: State<BackendState>) -> Result<String, String> {
  let guard = state.child.lock().map_err(|_| "backend lock poisoned".to_string())?;
  if guard.is_some() || backend_health_ok() {
    Ok("running".to_string())
  } else {
    Ok("stopped".to_string())
  }
}

fn main() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_dialog::init())
    .manage(BackendState::new())
    .setup(|app| {
      let state: State<BackendState> = app.state();
      let _ = start_backend(app.app_handle().clone(), state);
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![start_backend, stop_backend, backend_status])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
