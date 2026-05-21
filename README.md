# PDFTool

Production-oriented desktop PDF compressor + merger.

## Getting Started

Follow the [Installation Guide](installation.md) to set up the required dependencies and install PDFTool on your system.

## Stack

- Desktop shell: Tauri (Rust backend)
- UI: React + TypeScript
- PDF engines: qpdf + Ghostscript (Sidecars)

## Project Layout

- `apps/desktop`: Tauri + React app
- `apps/desktop/src-tauri/src/backend`: Rust logic for PDF pipeline
- `docs`: packaging and ops notes

## macOS Dev Setup

1. Install tools:
   - `brew install qpdf ghostscript`
2. Run desktop app:
   - `cd apps/desktop`
   - `npm install`
   - `npm run tauri:dev`

## Notes

- Frontend invokes Rust commands directly.
- Cancellation: Handled via Rust state management.
- Runtime preflight: `get_health` command checks `qpdf` and `ghostscript`.
- File metadata: `inspect_files` command provides accurate file sizes.
- Production-like tool path overrides (via sidecars):
  - `qpdf`
  - `ghostscript` (gswin64c on Windows)

## Build App Bundle

From `apps/desktop`:

- `npm run tauri:build`

The produced app bundle is self-contained for the build platform.

## GitHub Releases (CI/CD)

The project includes a GitHub Actions workflow that automatically builds and releases the application for Windows and macOS.

To trigger a new release:
1. Update the version in `apps/desktop/package.json` and `apps/desktop/src-tauri/Cargo.toml`.
2. Push a new tag to GitHub:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The workflow will build the installers and create a new GitHub Release automatically.

## Usage Guide

1. **System Dependencies**: Ensure `qpdf` and `ghostscript` are installed and available in your PATH.
   - **macOS**: `brew install qpdf ghostscript`
   - **Windows**: `choco install qpdf ghostscript` (or install manually and add to PATH)
2. **Launch Application**: Open PDFTool.
3. **Add PDFs**: Use the "Choose PDF files" button or drag and drop files into the app.
4. **Arrange**: Drag and drop files in the list to set the merge order.
5. **Configure**:
   - Select a **Compression Preset** (from "None" to "Aggressive").
   - Set the **Output File Path** using the Browse button.
6. **Process**: Click **Start**. The logs will show the progress of merging and compression.
7. **Done**: Your processed PDF will be saved at the specified location.

## Troubleshooting

- **Degraded Status**: If the app shows "degraded" runtime status, it means `qpdf` or `gs` (Ghostscript) was not found. Verify your installation and ensure they are in your system's PATH.
- **Merge/Compression Failed**: Check the logs for specific error messages from the underlying tools.
