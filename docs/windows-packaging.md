# Windows Packaging Notes

## Runtime dependencies

The following tools must be bundled as sidecars in `apps/desktop/src-tauri/binaries/`:

- `qpdf-x86_64-pc-windows-msvc.exe` (from qpdf)
- `ghostscript-x86_64-pc-windows-msvc.exe` (renamed from `gswin64c.exe`)

## Tauri bundle

Ensure `tauri.conf.json` has the correct sidecar configuration:

```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": [
    "icons/icon.ico",
    "icons/icon.png"
  ],
  "externalBin": [
    "binaries/qpdf",
    "binaries/ghostscript"
  ]
}
```

## Installer

- Use `npm run tauri:build` in `apps/desktop`
- This will produce NSIS/MSI targets depending on Tauri target config.
- The build process will automatically bundle the sidecars from the `binaries/` directory.
