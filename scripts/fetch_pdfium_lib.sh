#!/usr/bin/env bash
set -euo pipefail

REPO="bblanchon/pdfium-binaries"
TAG="chromium%2F7690"
TMP_DIR=""
CANDIDATES=()

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${ROOT_DIR}/lib"

usage() {
  cat <<'EOF'
Usage: fetch_pdfium_lib.sh [options]

Download the latest PDFium binary matching the current OS/arch from:
https://github.com/bblanchon/pdfium-binaries/releases/tag/${TAG}

Options:
  -o, --output-dir <dir>   Output directory for the dynamic library (default: ./lib)
  -h, --help               Show this help message
EOF
}

die() {
  printf 'Error: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  if [[ -n "${TMP_DIR}" && -d "${TMP_DIR}" ]]; then
    rm -rf "${TMP_DIR}"
  fi
}

ensure_cmd() {
  local cmd="$1"
  command -v "${cmd}" >/dev/null 2>&1 || die "Missing required command: ${cmd}"
}

detect_os() {
  local os
  os="$(uname -s)"
  case "${os}" in
    Darwin) echo "mac" ;;
    Linux) echo "linux" ;;
    MINGW*|MSYS*|CYGWIN*) echo "win" ;;
    *) die "Unsupported OS: ${os}" ;;
  esac
}

detect_arch() {
  local arch
  arch="$(uname -m)"
  case "${arch}" in
    x86_64|amd64) echo "x64" ;;
    aarch64|arm64) echo "arm64" ;;
    i386|i686) echo "x86" ;;
    armv7l|armv7|arm) echo "arm" ;;
    ppc64le|ppc64) echo "ppc64" ;;
    *) die "Unsupported architecture: ${arch}" ;;
  esac
}

is_musl() {
  if ! command -v ldd >/dev/null 2>&1; then
    return 1
  fi
  if ldd --version 2>&1 | grep -qi "musl"; then
    return 0
  fi
  return 1
}

build_candidates() {
  local os="$1"
  local arch="$2"
  CANDIDATES=()

  case "${os}" in
    mac)
      CANDIDATES+=("pdfium-mac-${arch}.tgz")
      CANDIDATES+=("pdfium-mac-univ.tgz")
      ;;
    linux)
      if is_musl; then
        case "${arch}" in
          x64|x86|arm64)
            CANDIDATES+=("pdfium-linux-musl-${arch}.tgz")
            ;;
        esac
      fi
      CANDIDATES+=("pdfium-linux-${arch}.tgz")
      ;;
    win)
      CANDIDATES+=("pdfium-win-${arch}.tgz")
      ;;
  esac
}

library_name_for_os() {
  local os="$1"
  case "${os}" in
    mac) echo "libpdfium.dylib" ;;
    linux) echo "libpdfium.so" ;;
    win) echo "pdfium.dll" ;;
    *) die "Unsupported OS key: ${os}" ;;
  esac
}

download_asset() {
  local archive="$1"
  shift
  local selected=""

  for asset in "$@"; do
    local url="https://github.com/${REPO}/releases/download/${TAG}/${asset}"
    printf 'Trying asset: %s\n' "${asset}" >&2
    if curl -fsSL --retry 3 --retry-delay 1 -A "kpdf-pdfium-fetch/1.0" "${url}" -o "${archive}"; then
      selected="${asset}"
      break
    fi
  done

  if [[ -z "${selected}" ]]; then
    die "Could not download a matching asset from tag '${TAG}'."
  fi

  printf '%s\n' "${selected}"
}

find_library() {
  local extract_dir="$1"
  local lib_name="$2"

  local preferred_1="${extract_dir}/lib/${lib_name}"
  local preferred_2="${extract_dir}/bin/${lib_name}"
  local preferred_3="${extract_dir}/${lib_name}"

  if [[ -f "${preferred_1}" ]]; then
    printf '%s\n' "${preferred_1}"
    return 0
  fi
  if [[ -f "${preferred_2}" ]]; then
    printf '%s\n' "${preferred_2}"
    return 0
  fi
  if [[ -f "${preferred_3}" ]]; then
    printf '%s\n' "${preferred_3}"
    return 0
  fi

  local found
  found="$(find "${extract_dir}" -type f -name "${lib_name}" | head -n 1 || true)"
  [[ -n "${found}" ]] || die "Library '${lib_name}' not found in downloaded archive."
  printf '%s\n' "${found}"
}

main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -o|--output-dir)
        [[ $# -ge 2 ]] || die "Missing value for $1"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "Unknown argument: $1 (use --help)"
        ;;
    esac
  done

  ensure_cmd curl
  ensure_cmd tar

  local os arch lib_name
  os="$(detect_os)"
  arch="$(detect_arch)"
  lib_name="$(library_name_for_os "${os}")"

  build_candidates "${os}" "${arch}"
  [[ "${#CANDIDATES[@]}" -gt 0 ]] || die "No download candidates for ${os}/${arch}"

  local archive extract_dir selected_asset src_lib target_lib
  TMP_DIR="$(mktemp -d)"
  trap cleanup EXIT

  archive="${TMP_DIR}/pdfium.tgz"
  extract_dir="${TMP_DIR}/extract"
  mkdir -p "${extract_dir}"

  selected_asset="$(download_asset "${archive}" "${CANDIDATES[@]}")"

  tar -xzf "${archive}" -C "${extract_dir}"
  src_lib="$(find_library "${extract_dir}" "${lib_name}")"

  mkdir -p "${OUTPUT_DIR}"
  target_lib="${OUTPUT_DIR}/${lib_name}"
  cp -f "${src_lib}" "${target_lib}"

  printf 'Downloaded: %s\n' "${selected_asset}"
  printf 'Copied: %s\n' "${target_lib}"
}

main "$@"
