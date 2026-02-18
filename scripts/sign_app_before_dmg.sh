#!/usr/bin/env bash
set -euo pipefail

if [[ "${CARGO_PACKAGER_FORMAT:-}" != "dmg" ]]; then
  exit 0
fi

if [[ "$(uname -s)" != "Darwin" ]]; then
  exit 0
fi

if [[ -z "${MACOS_SIGNING_IDENTITY:-}" ]]; then
  if [[ -n "${CI:-}" ]]; then
    echo "::error::MACOS_SIGNING_IDENTITY is required to sign the app before dmg packaging."
    exit 1
  fi
  echo "[sign_app_before_dmg] MACOS_SIGNING_IDENTITY is empty, skipping app signing."
  exit 0
fi

app_path="target/release/kPDF.app"
if [[ ! -d "${app_path}" ]]; then
  echo "::error::Missing app bundle before dmg packaging: ${app_path}"
  exit 1
fi

while IFS= read -r -d '' dylib; do
  codesign --force --timestamp --sign "${MACOS_SIGNING_IDENTITY}" "${dylib}"
done < <(find "${app_path}" -type f -name "*.dylib" -print0)

codesign --force --deep --options runtime --timestamp --sign "${MACOS_SIGNING_IDENTITY}" "${app_path}"
codesign --verify --deep --strict --verbose=2 "${app_path}"
