use std::fs;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tauri::Manager;

pub struct BackendLauncher {
    port: u16,
}

impl BackendLauncher {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn start(&self, app: &tauri::AppHandle, child: &mut Option<Child>) -> Result<(), String> {
        if let Some(existing) = child.as_mut() {
            match existing.try_wait() {
                Ok(Some(_)) | Err(_) => *child = None,
                Ok(None) => return Ok(()),
            }
        }

        if self.health_ok() {
            return Ok(());
        }

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
                "backend binary missing at {}. Build it with: cd backend/go-service && make build",
                backend_path.display()
            ));
        }

        let mut cmd = Command::new(backend_path);
        cmd.arg("--port")
            .arg(self.port.to_string())
            .stdin(Stdio::null());

        let log_path = std::env::temp_dir().join("pdftool-backend.log");
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| {
                format!(
                    "failed to open backend log file {}: {e}",
                    log_path.display()
                )
            })?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("failed to clone backend log file handle: {e}"))?;
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(log_file_err));

        if let Some(qpdf_path) = self.resolve_qpdf_binary(app) {
            cmd.env("PDFTOOL_QPDF_PATH", qpdf_path.clone());
            cmd.arg("--qpdf").arg(qpdf_path);
        }
        if let Some(gs_path) = self.resolve_ghostscript_binary(app) {
            cmd.env("PDFTOOL_GS_PATH", gs_path.clone());
            cmd.arg("--gs").arg(gs_path);
        }

        let mut spawned = cmd
            .spawn()
            .map_err(|e| format!("failed to launch backend: {e}"))?;
        std::thread::sleep(Duration::from_millis(220));
        if let Ok(Some(status)) = spawned.try_wait() {
            return Err(format!(
                "backend exited immediately with status {status}. Check log: {}",
                log_path.display()
            ));
        }
        *child = Some(spawned);
        Ok(())
    }

    pub fn stop(&self, child: &mut Option<Child>) {
        if let Some(mut c) = child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    pub fn status(&self, child: &Option<Child>) -> String {
        if child.is_some() || self.health_ok() {
            "running".to_string()
        } else {
            "stopped".to_string()
        }
    }

    fn backend_binary_path(&self, app: &tauri::AppHandle) -> Result<PathBuf, String> {
        if let Ok(explicit) = std::env::var("PDFTOOL_BACKEND_PATH") {
            return Ok(PathBuf::from(explicit));
        }
        let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        let bundled_names = ["pdfsvc.exe", "pdfsvc"];
        #[cfg(not(target_os = "windows"))]
        let bundled_names = ["pdfsvc"];
        if let Some(path) = self.find_resource_binary(&resource_dir, &bundled_names) {
            return Ok(path);
        }

        let mut candidates = Vec::new();
        #[cfg(target_os = "windows")]
        {
            candidates.push(PathBuf::from(
                "..\\..\\backend\\go-service\\bin\\pdfsvc.exe",
            ));
            candidates.push(PathBuf::from(
                "..\\..\\..\\backend\\go-service\\bin\\pdfsvc.exe",
            ));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            candidates.push(manifest_dir.join("../../../backend/go-service/bin/pdfsvc"));
            candidates.push(PathBuf::from("../../backend/go-service/bin/pdfsvc"));
            candidates.push(PathBuf::from("../../../backend/go-service/bin/pdfsvc"));
        }
        for p in candidates {
            if p.exists() {
                return Ok(p);
            }
        }
        Err("backend binary not found; set PDFTOOL_BACKEND_PATH or build backend/go-service/bin/pdfsvc".to_string())
    }

    fn find_resource_binary(&self, resource_dir: &Path, names: &[&str]) -> Option<PathBuf> {
        for name in names {
            let direct = resource_dir.join(name);
            if direct.exists() {
                return Some(direct);
            }
            let in_bin = resource_dir.join("bin").join(name);
            if in_bin.exists() {
                return Some(in_bin);
            }
        }
        self.find_in_tree(resource_dir, names, 8)
    }

    fn find_in_tree(&self, root: &Path, names: &[&str], max_depth: usize) -> Option<PathBuf> {
        if max_depth == 0 {
            return None;
        }
        let entries = fs::read_dir(root).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if names.iter().any(|candidate| candidate.eq_ignore_ascii_case(file_name)) {
                        return Some(path);
                    }
                }
                continue;
            }
            if path.is_dir() {
                if let Some(found) = self.find_in_tree(&path, names, max_depth - 1) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn resolve_tool_binary(
        &self,
        app: &tauri::AppHandle,
        var_name: &str,
        fallback_name: &str,
        candidates: &[&str],
        bundled_names: &[&str],
    ) -> Option<String> {
        if let Ok(v) = std::env::var(var_name) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
        if let Ok(resource_dir) = app.path().resource_dir() {
            if let Some(p) = self.find_resource_binary(&resource_dir, bundled_names) {
                return Some(p.to_string_lossy().to_string());
            }
        }
        for candidate in candidates {
            if Path::new(candidate).exists() {
                return Some((*candidate).to_string());
            }
        }
        #[cfg(target_os = "windows")]
        if let Ok(output) = Command::new("where").arg(fallback_name).output() {
            if output.status.success() {
                let resolved = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if !resolved.is_empty() {
                    return Some(resolved);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
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

    fn resolve_qpdf_binary(&self, app: &tauri::AppHandle) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            self.resolve_tool_binary(
                app,
                "PDFTOOL_QPDF_PATH",
                "qpdf",
                &["C:\\Program Files\\qpdf\\bin\\qpdf.exe"],
                &["qpdf.exe", "qpdf"],
            )
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.resolve_tool_binary(
                app,
                "PDFTOOL_QPDF_PATH",
                "qpdf",
                &["/opt/homebrew/bin/qpdf", "/usr/local/bin/qpdf"],
                &["qpdf"],
            )
        }
    }

    fn resolve_ghostscript_binary(&self, app: &tauri::AppHandle) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            self.resolve_tool_binary(
                app,
                "PDFTOOL_GS_PATH",
                "gswin64c",
                &[],
                &["gswin64c.exe", "gs.exe", "gswin64c"],
            )
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.resolve_tool_binary(
                app,
                "PDFTOOL_GS_PATH",
                "gs",
                &["/opt/homebrew/bin/gs", "/usr/local/bin/gs"],
                &["gs"],
            )
        }
    }

    fn port_reachable(&self) -> bool {
        let addr: SocketAddr = match format!("127.0.0.1:{}", self.port).parse() {
            Ok(a) => a,
            Err(_) => return false,
        };
        TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
    }

    fn health_ok(&self) -> bool {
        let addr: SocketAddr = match format!("127.0.0.1:{}", self.port).parse() {
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
    fn kill_stale_backend_on_port(&self) {
        let output = Command::new("lsof")
            .arg("-Fpc")
            .arg("-nP")
            .arg(format!("-iTCP:{}", self.port))
            .arg("-sTCP:LISTEN")
            .output();
        let Ok(output) = output else { return };
        if !output.status.success() {
            return;
        }
        let rows = String::from_utf8_lossy(&output.stdout);
        let mut current_pid: Option<String> = None;
        let mut current_cmd: Option<String> = None;
        for line in rows.lines() {
            if let Some(rest) = line.strip_prefix('p') {
                current_pid = Some(rest.trim().to_string());
                continue;
            }
            if let Some(rest) = line.strip_prefix('c') {
                current_cmd = Some(rest.trim().to_string());
            }
            if current_pid.is_some() && current_cmd.is_some() {
                let pid = current_pid.take().unwrap_or_default();
                let cmd = current_cmd.take().unwrap_or_default();
                if !pid.is_empty() && cmd.contains("pdfsvc") {
                    let _ = Command::new("kill").arg("-9").arg(pid).status();
                }
            }
        }
    }
}
