# Windows Packaging Notes

## Backend binary

Build Go sidecar for Windows:

```bash
cd backend/go-service
GOOS=windows GOARCH=amd64 go build -o bin/pdfsvc.exe ./cmd/pdfsvc
```

## Runtime dependencies

- `qpdf.exe`
- `gswin64c.exe` (Ghostscript CLI)

Set paths via environment in production launch if not bundled in PATH:

- `PDFTOOL_QPDF_PATH`
- `PDFTOOL_GS_PATH`

## Tauri bundle

Place sidecar under app resources:

- `src-tauri` bundle resources already include `../../backend/go-service/bin/*`

## Installer

- Use `npm run tauri:build` in `apps/desktop`
- This will produce NSIS/MSI targets depending on Tauri target config.
