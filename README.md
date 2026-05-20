# PDFTool

Production-oriented desktop PDF compressor + merger.

## Stack

- Desktop shell: Tauri
- UI: React + TypeScript
- Processing backend: Go sidecar service
- PDF engines: qpdf + Ghostscript

## Project Layout

- `apps/desktop`: Tauri + React app
- `backend/go-service`: Go job manager and subprocess orchestrator
- `docs`: packaging and ops notes

## macOS Dev Setup

1. Install tools:
   - `brew install qpdf ghostscript`
2. Build backend:
   - `cd backend/go-service && make build`
3. Run desktop app:
   - `cd apps/desktop`
   - `npm install`
   - `npm run tauri:dev`

## Notes

- Backend listens on `127.0.0.1:47832`.
- Frontend subscribes to SSE job progress from `/v1/events`.
- Cancellation endpoint: `POST /v1/jobs/{jobId}/cancel`.
- Runtime preflight endpoint: `GET /v1/health` (checks `qpdf` and `ghostscript`).
- File metadata endpoint: `POST /v1/files/inspect` (accurate file sizes from backend FS view).
- Production-like tool path overrides:
  - `PDFTOOL_QPDF_PATH`
  - `PDFTOOL_GS_PATH`
