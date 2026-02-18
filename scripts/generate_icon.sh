#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
TOOL_MANIFEST="${ROOT_DIR}/scripts/icon_generator/Cargo.toml"

DEFAULT_INPUT="${ROOT_DIR}/assets/app.png"
DEFAULT_ICO_OUTPUT="${ROOT_DIR}/assets/app.ico"
DEFAULT_ICNS_OUTPUT="${ROOT_DIR}/assets/app.icns"

usage() {
  cat <<'EOF'
Usage: generate_icon.sh [input_png] [output.ico|output.icns] [options]

Generate icon files from a PNG via Rust crates.

Defaults:
  input_png   ./assets/app.png

If output is omitted, both files are generated:
  ./assets/app.ico
  ./assets/app.icns

No-arg mode generates both defaults directly:
  ./assets/app.png -> ./assets/app.ico + ./assets/app.icns

Extra options are passed to the Rust tool, for example:
  --ico ./assets/app.ico --icns ./assets/app.icns
  --ico-sizes 16,32,48,256 --icns-sizes 16,32,128,256,512
EOF
}

die() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

[[ -f "${TOOL_MANIFEST}" ]] || die "Rust icon tool manifest not found: ${TOOL_MANIFEST}"
command -v cargo >/dev/null 2>&1 || die "cargo is required but not found in PATH."

if [[ $# -eq 0 ]]; then
  set -- "${DEFAULT_INPUT}" \
    --ico "${DEFAULT_ICO_OUTPUT}" \
    --icns "${DEFAULT_ICNS_OUTPUT}"
fi

INPUT_PATH="$1"
[[ -f "${INPUT_PATH}" ]] || die "Input PNG not found: ${INPUT_PATH}"

CARGO_TARGET_DIR="${ROOT_DIR}/target/icon_generator" \
  cargo run --quiet --manifest-path "${TOOL_MANIFEST}" -- "$@"
