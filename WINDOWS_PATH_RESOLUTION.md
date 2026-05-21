# Windows Path Resolution Flow

## Complete Startup Flow

### 1. Tauri App Launches (`apps/desktop/src-tauri/src/backend/launcher.rs`)

When PDFTool.exe starts on Windows:

```
Tauri App Start
  ↓
BackendLauncher::start()
  ↓
Find pdfsvc.exe (backend binary)
  → Check PDFTOOL_BACKEND_PATH env var
  → Check bundled resources at app_resource_dir/pdfsvc.exe
  → Check app_resource_dir/bin/pdfsvc.exe
  → Recursive search up to 6 levels deep
  ✓ Found → Start backend process
  
Backend Process Launched
  ↓
Resolve qpdf.exe
  → Check PDFTOOL_QPDF_PATH env var
  → Check bundled resources (recursively)
  → `where qpdf` search on PATH
  → Pass via --qpdf flag to pdfsvc.exe
  
Resolve gswin64c.exe
  → Check PDFTOOL_GS_PATH env var
  → Check bundled resources (recursively)
  → `where gswin64c` search on PATH
  → Pass via --gs flag to pdfsvc.exe
  
Backend Process Spawned
  ↓
pdfsvc.exe receives args:
  --port 47832
  --qpdf C:\...\qpdf.exe
  --gs C:\...\gswin64c.exe
```

### 2. Backend Startup (`backend/go-service/cmd/pdfsvc/main.go`)

When pdfsvc.exe starts:

```
Parse flags:
  --port, --qpdf, --gs

Normalize each path:
  - Strip whitespace/quotes
  - Remove CR/LF chars
  - Fix leading \ before drive letter:
    \C:\... → C:\...
    /C:\... → C:\...
  
Log normalized paths:
  [backend] qpdf path: C:\...\qpdf.exe
  [backend] ghostscript path: C:\...\gswin64c.exe
  
Start HTTP server on 127.0.0.1:47832
```

### 3. Frontend Health Check (`apps/desktop/src/App.tsx`)

When app loads:

```
Call /v1/health
  ↓
Backend health check runs:
  - Probe qpdf --version
  - Probe gswin64c -version (try -version first, then --version)
  ↓
Return tool status:
  {
    status: "ok",
    qpdf: { ok: true, path: "C:\...\qpdf.exe", version: "..." },
    ghostscript: { ok: true, path: "C:\...\gswin64c.exe", version: "..." }
  }
  
If any tool missing/unusable:
  status: "degraded"
  Show error details to user
```

---

## Key Implementation Details

### Path Normalization (Rust - Tauri Launcher)

```rust
fn sanitize_path_string(&self, raw: &str) -> Option<String> {
    let mut trimmed = raw.trim().trim_matches('"').trim_matches('\'').to_string();
    trimmed.retain(|c| c != '\0' && c != '\r' && c != '\n');
    
    // Fix leading \ or / before drive letter
    // \C:\Users\... → C:\Users\...
    // /C:\Users\... → C:\Users\...
    if trimmed.len() >= 4 && 
       (trimmed[0] == b'\\' || trimmed[0] == b'/') &&
       trimmed[2] == b':' &&
       (trimmed[3] == b'\\' || trimmed[3] == b'/') &&
       (trimmed[1].is_ascii_alphabetic()) {
        trimmed = trimmed[1..].to_string();
    }
    
    if trimmed.is_empty() || trimmed.contains('*') || trimmed.contains('?') {
        return None;
    }
    Some(trimmed)
}
```

### Path Normalization (Go - Backend)

```go
func normalizePathArg(v string) string {
    trimmed := strings.TrimSpace(v)
    trimmed = strings.Trim(trimmed, "\"'")
    trimmed = strings.ReplaceAll(trimmed, "\r", "")
    trimmed = strings.ReplaceAll(trimmed, "\n", "")
    
    // Fix leading \ or / before drive letter
    if len(trimmed) >= 4 &&
        (trimmed[0] == '\\' || trimmed[0] == '/') &&
        trimmed[2] == ':' &&
        (trimmed[3] == '\\' || trimmed[3] == '/') &&
        ((trimmed[1] >= 'a' && trimmed[1] <= 'z') || (trimmed[1] >= 'A' && trimmed[1] <= 'Z')) {
        trimmed = trimmed[1:]
    }
    if trimmed == "" {
        return v
    }
    return trimmed
}
```

### Resource Bundling

**tauri.conf.json:**
```json
"resources": [
  "../../../backend/go-service/bin/*"
]
```

This copies ALL files from `backend/go-service/bin/` into the installed app resources:
- Windows: `C:\Users\<user>\AppData\Local\pdftool\<hash>\resources\...\bin\`
- macOS: `/Applications/PDFTool.app/Contents/Resources/...`

### Bundled vs System Tools Priority

1. **Environment variable** (PDFTOOL_QPDF_PATH, PDFTOOL_GS_PATH) — Highest priority
2. **Bundled resources** — Check app's resource directory recursively (0-6 levels)
3. **System PATH** — Use `where` (Windows) / `which` (macOS/Linux)

---

## Debugging Guide

### Check Launcher Logs

On Windows, launcher logs are printed to `stderr` via `eprintln!`:

```
[launcher] backend binary: C:\Users\...\PDFTool\resources\bin\pdfsvc.exe
[PDFTOOL_QPDF_PATH] searching bundled at: C:\Users\...\PDFTool\resources
[PDFTOOL_QPDF_PATH] found bundled: C:\Users\...\PDFTool\resources\_up_\_up_\_up_\backend\go-service\bin\qpdf.exe
[PDFTOOL_QPDF_PATH] cleaned to: C:\Users\...\PDFTool\resources\_up_\_up_\_up_\backend\go-service\bin\qpdf.exe
[launcher] resolved qpdf: C:\Users\...\PDFTool\resources\_up_\_up_\_up_\backend\go-service\bin\qpdf.exe
```

### Check Backend Logs

Backend logs go to `%TEMP%\pdftool-backend.log`:

```
[backend] qpdf path: C:\Users\...\PDFTool\resources\_up_\_up_\_up_\backend\go-service\bin\qpdf.exe
[backend] ghostscript path: C:\Users\...\PDFTool\resources\_up_\_up_\_up_\backend\go-service\bin\gswin64c.exe
pdf service listening on 127.0.0.1:47832
```

### Common Issues & Solutions

**Issue: "missing or unusable" with illegal characters**
- Check `%TEMP%\pdftool-backend.log` for actual path being used
- Launcher logs (stderr) show full path resolution

**Issue: "degraded" health status**
- Check if qpdf/gs versions are responding (try `qpdf --version` manually on Windows)
- Ghostscript uses `-version`, not `--version` (we try both now)

**Issue: qpdf/gs not found bundled**
- Verify files exist: `backend/go-service/bin/qpdf.exe`, `backend/go-service/bin/gswin64c.exe`
- Rebuild: `npm run tauri:build`
- Windows installer should package them under Resources

---

## Build & Package Instructions

### Rebuild Windows Installer with Fixes

On Windows (or via GitHub Actions):

```
cd apps\desktop
npm install
npm run tauri:build
```

This produces:
- `src-tauri/target/release/bundle/msi/PDFTool_*.msi` (classic installer)
- `src-tauri/target/release/bundle/nsis/PDFTool_*.exe` (recommended)

### Install on Clean Machine

1. Uninstall old PDFTool (Control Panel → Programs)
2. Install new PDFTool.exe
3. Launch PDFTool
4. Logs appear in `%TEMP%\pdftool-backend.log`

---

## Expected Behavior After Fix

✅ App launches with no console popup  
✅ Runtime shows "ok" (not "degraded")  
✅ qpdf/gs paths are clean (no leading `\`)  
✅ Health check passes immediately  
✅ Can upload, compress, merge PDFs without errors  

If you see degraded after install, check:
1. `%TEMP%\pdftool-backend.log` for path/version errors
2. Launcher stderr for resolution trace
3. Run `qpdf --version` / `gswin64c -version` manually to verify tools work
