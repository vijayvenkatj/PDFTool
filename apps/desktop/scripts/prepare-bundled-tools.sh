#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
STAGE_DIR="$ROOT_DIR/src-tauri/resources/bin"

mkdir -p "$STAGE_DIR"

# On GitHub Actions, tools are already staged by PowerShell workflows
if [[ -n "${GITHUB_ACTIONS:-}" ]]; then
  if [[ -f "$STAGE_DIR/qpdf.exe" ]] || [[ -f "$STAGE_DIR/qpdf" ]]; then
    echo "✓ Tools already staged by CI workflow"
    exit 0
  fi
  echo "✗ Expected tools to be staged by workflow, but not found in $STAGE_DIR"
  exit 1
fi

copy_tool() {
  local src="$1"
  local dst="$2"
  if [[ -e "$dst" ]]; then
    chmod u+w "$dst" || true
    rm -f "$dst"
  fi
  if [[ "$OSTYPE" == "darwin"* ]]; then
    cp -X "$src" "$dst" 2>/dev/null || cp "$src" "$dst"
  else
    cp "$src" "$dst"
  fi
  chmod u+rw "$dst" || true
  if command -v xattr >/dev/null 2>&1; then
    xattr -c "$dst" 2>/dev/null || true
    xattr -d com.apple.provenance "$dst" 2>/dev/null || true
  fi
  chmod a+r "$dst" || true
  chmod +x "$dst" || true
}

if [[ "$OSTYPE" == "darwin"* ]]; then
  QPDF_PATH="${PDFTOOL_QPDF_PATH:-$(command -v qpdf || true)}"
  GS_PATH="${PDFTOOL_GS_PATH:-$(command -v gs || true)}"
  if [[ -z "${QPDF_PATH}" || ! -x "${QPDF_PATH}" ]]; then
    echo "qpdf not found. Install with: brew install qpdf" >&2
    exit 1
  fi
  if [[ -z "${GS_PATH}" || ! -x "${GS_PATH}" ]]; then
    echo "ghostscript not found. Install with: brew install ghostscript" >&2
    exit 1
  fi
  copy_tool "$QPDF_PATH" "$STAGE_DIR/qpdf"
  copy_tool "$GS_PATH" "$STAGE_DIR/gs"
  echo "✓ Bundled macOS tools: qpdf, gs"
  exit 0
fi

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
  # Local Windows dev build
  QPDF_PATH="${PDFTOOL_QPDF_PATH:-}"
  GS_PATH="${PDFTOOL_GS_PATH:-}"
  if [[ -z "${QPDF_PATH}" ]]; then
    echo "ERROR: For local Windows build, set PDFTOOL_QPDF_PATH" >&2
    echo "Example: set PDFTOOL_QPDF_PATH=C:\\Program Files\\qpdf\\bin\\qpdf.exe" >&2
    exit 1
  fi
  if [[ -z "${GS_PATH}" ]]; then
    echo "ERROR: For local Windows build, set PDFTOOL_GS_PATH" >&2
    echo "Example: set PDFTOOL_GS_PATH=C:\\Program Files\\gs\\gs9.56.1\\bin\\gswin64c.exe" >&2
    exit 1
  fi
  if [[ ! -f "${QPDF_PATH}" ]]; then
    echo "ERROR: qpdf not found at $QPDF_PATH" >&2
    exit 1
  fi
  if [[ ! -f "${GS_PATH}" ]]; then
    echo "ERROR: gswin64c.exe not found at $GS_PATH" >&2
    exit 1
  fi
  copy_tool "$QPDF_PATH" "$STAGE_DIR/qpdf.exe"
  copy_tool "$GS_PATH" "$STAGE_DIR/gswin64c.exe"
  echo "✓ Bundled Windows tools: qpdf.exe, gswin64c.exe"
  exit 0
fi

echo "Unsupported OSTYPE: $OSTYPE" >&2
exit 1


