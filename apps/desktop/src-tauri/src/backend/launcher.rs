use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::Manager;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct BackendLauncher {
    port: u16,
}

impl BackendLauncher {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn start(&self, app: &tauri::AppHandle, child: &mut Option<Child>) -> Result<(), String> {
        // Check if already running
        if let Some(existing) = child.as_mut() {
            match existing.try_wait() {
                Ok(Some(_)) | Err(_) => *child = None,
                Ok(None) => return Ok(()),
            }
        }

        // Check health
        if self.health_ok() {
            return Ok(());
        }

        // Kill stale backend on macOS
        #[cfg(target_os = "macos")]
        {
            if self.port_reachable() && !self.health_ok() {
                self.kill_stale_backend_on_port();
                std::thread::sleep(Duration::from_millis(150));
            }
        }

        let backend_path = self.backend_binary_path(app)?;
        if !backend_path.exists() {
            return Err(format!(
                "Backend binary not found at: {}\n\nBuild it with: cd backend/go-service && make build",
                backend_path.display()
            ));
        }

        eprintln!("[launcher] Starting backend: {}", backend_path.display());

        let mut cmd = Command::new(&backend_path);
        cmd.arg("--port")
            .arg(self.port.to_string())
            .stdin(Stdio::null());

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        // Setup logging
        let log_path = std::env::temp_dir().join("pdftool-backend.log");
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log: {e}"))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log handle: {e}"))?;
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(log_file_err));

        // Resolve tool paths
        if let Some(qpdf_path) = self.resolve_qpdf_binary(app) {
            eprintln!("[launcher] qpdf: {}", qpdf_path);
            cmd.arg("--qpdf").arg(&qpdf_path);
        } else {
            eprintln!("[launcher] qpdf NOT FOUND");
        }

        if let Some(gs_path) = self.resolve_ghostscript_binary(app) {
            eprintln!("[launcher] gs: {}", gs_path);
            cmd.arg("--gs").arg(&gs_path);
        } else {
            eprintln!("[launcher] gs NOT FOUND");
        }

        let mut spawned = cmd.spawn().map_err(|e| format!("Failed to spawn: {e}"))?;
        std::thread::sleep(Duration::from_millis(220));

        if let Ok(Some(status)) = spawned.try_wait() {
            let log_content = std::fs::read_to_string(&log_path).unwrap_or_default();
            return Err(format!(
                "Backend exited immediately with status {}\n\nLog:\n{}",
                status, log_content
            ));
        }

        *child = Some(spawned);
        Ok(())
    }

    fn backend_binary_path(&self, app: &tauri::AppHandle) -> Result<PathBuf, String> {
        if let Ok(explicit) = std::env::var("PDFTOOL_BACKEND_PATH") {
            return Ok(PathBuf::from(explicit));
        }

        let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;

        #[cfg(target_os = "windows")]
        {
            let candidate = resource_dir.join("pdfsvc.exe");
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let candidate = resource_dir.join("pdfsvc");
            if candidate.exists() {
                return Ok(candidate);
            }
        }

        Err("Backend binary not found in resources".to_string())
    }

    fn resolve_tool_binary(&self, _app: &tauri::AppHandle, var_name: &str, tool_name: &str) -> Option<String> {
        // Check environment variable first
        if let Ok(path) = std::env::var(var_name) {
            eprintln!("[launcher] {} from env", var_name);
            return Some(path);
        }

        // Search PATH
        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("where")
                .arg(tool_name)
                .creation_flags(CREATE_NO_WINDOW)
                .output()
            {
                if output.status.success() {
                    if let Ok(path) = String::from_utf8(output.stdout) {
                        if let Some(line) = path.lines().next() {
                            return Some(line.trim().to_string());
                        }
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(output) = Command::new("which").arg(tool_name).output() {
                if output.status.success() {
                    if let Ok(path) = String::from_utf8(output.stdout) {
                        return Some(path.trim().to_string());
                    }
                }
            }
        }

        eprintln!("[launcher] {} not found in PATH", var_name);
        None
    }

    fn resolve_qpdf_binary(&self, app: &tauri::AppHandle) -> Option<String> {
        self.resolve_tool_binary(app, "PDFTOOL_QPDF_PATH", "qpdf")
    }

    fn resolve_ghostscript_binary(&self, app: &tauri::AppHandle) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            self.resolve_tool_binary(app, "PDFTOOL_GS_PATH", "gswin64c")
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.resolve_tool_binary(app, "PDFTOOL_GS_PATH", "gs")
        }
    }

    fn health_ok(&self) -> bool {
        match TcpStream::connect(("127.0.0.1", self.port)) {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
                let mut buf = [0; 1024];
                match stream.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        let response = String::from_utf8_lossy(&buf[..n]);
                        response.contains("ok")
                    }
                    _ => false,
                }
            }
            Err(_) => false,
        }
    }

    fn port_reachable(&self) -> bool {
        TcpStream::connect(("127.0.0.1", self.port)).is_ok()
    }

    #[cfg(target_os = "macos")]
    fn kill_stale_backend_on_port(&self) {
        let _ = Command::new("lsof")
            .args(&["-ti", &format!(":{}", self.port)])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout).ok().and_then(|pids| {
                    if let Some(pid) = pids.trim().split('\n').next() {
                        let _ = Command::new("kill").arg(pid).output();
                    }
                    Ok::<(), ()>(())
                })
            });
    }

    pub fn stop(&self, child: &mut Option<Child>) {
        if let Some(mut c) = child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn status(&self, child: &Option<Child>) -> String {
        match child {
            Some(c) => {
                if c.stdout.is_some() {
                    "running".to_string()
                } else {
                    "stopped".to_string()
                }
            }
            None => "stopped".to_string(),
        }
    }
}
