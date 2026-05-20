#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="$ROOT_DIR/src-tauri/resources/bin"

mkdir -p "$BIN_DIR"

copy_tool() {
  local src="$1"
  local dst="$2"
  cp "$src" "$dst"
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
  copy_tool "$QPDF_PATH" "$BIN_DIR/qpdf"
  copy_tool "$GS_PATH" "$BIN_DIR/gs"
  echo "Bundled macOS tools: qpdf, gs"
  exit 0
fi

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
  QPDF_PATH="${PDFTOOL_QPDF_PATH:-}"
  GS_PATH="${PDFTOOL_GS_PATH:-}"
  if [[ -z "${QPDF_PATH}" || ! -f "${QPDF_PATH}" ]]; then
    echo "Set PDFTOOL_QPDF_PATH to qpdf.exe location" >&2
    exit 1
  fi
  if [[ -z "${GS_PATH}" || ! -f "${GS_PATH}" ]]; then
    echo "Set PDFTOOL_GS_PATH to gswin64c.exe location" >&2
    exit 1
  fi
  copy_tool "$QPDF_PATH" "$BIN_DIR/qpdf.exe"
  copy_tool "$GS_PATH" "$BIN_DIR/gswin64c.exe"
  echo "Bundled Windows tools: qpdf.exe, gswin64c.exe"
  exit 0
fi

echo "Unsupported OSTYPE: $OSTYPE" >&2
exit 1
