#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
STAGE_DIR="$ROOT_DIR/src-tauri/resources/bin"

mkdir -p "$STAGE_DIR"

# On CI/CD (GitHub Actions), tools are already staged by workflows
# This script is mainly for local development
if [[ -n "${CI:-}" ]]; then
  if [[ -f "$STAGE_DIR/qpdf" ]] || [[ -f "$STAGE_DIR/qpdf.exe" ]]; then
    echo "Tools already staged for CI build"
    exit 0
  fi
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
  echo "Bundled macOS tools: qpdf, gs"
  exit 0
fi

if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
  # On local Windows dev, require env vars; on CI they're already staged
  QPDF_PATH="${PDFTOOL_QPDF_PATH:-}"
  GS_PATH="${PDFTOOL_GS_PATH:-}"
  if [[ -z "${QPDF_PATH}" || ! -f "${QPDF_PATH}" ]]; then
    if [[ -n "${CI:-}" ]]; then
      echo "CI environment: tools should be staged by workflow" >&2
    else
      echo "For local Windows dev, set PDFTOOL_QPDF_PATH=C:\\Program Files\\qpdf\\bin\\qpdf.exe" >&2
    fi
    exit 1
  fi
  if [[ -z "${GS_PATH}" || ! -f "${GS_PATH}" ]]; then
    if [[ -n "${CI:-}" ]]; then
      echo "CI environment: tools should be staged by workflow" >&2
    else
      echo "For local Windows dev, set PDFTOOL_GS_PATH=C:\\Program Files\\gs\\gs...\\bin\\gswin64c.exe" >&2
    fi
    exit 1
  fi
  copy_tool "$QPDF_PATH" "$STAGE_DIR/qpdf.exe"
  copy_tool "$GS_PATH" "$STAGE_DIR/gswin64c.exe"
  echo "Bundled Windows tools: qpdf.exe, gswin64c.exe"
  exit 0
fi

echo "Unsupported OSTYPE: $OSTYPE" >&2
exit 1

