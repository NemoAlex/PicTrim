#!/usr/bin/env bash

set -euo pipefail

required_variables=(
  MAC_CSC_LINK
  MAC_CSC_KEY_PASSWORD
  APPLE_ID
  APPLE_APP_SPECIFIC_PASSWORD
  APPLE_TEAM_ID
)
for variable_name in "${required_variables[@]}"; do
  if [[ -z "${!variable_name:-}" ]]; then
    echo "Error: required macOS signing variable is missing: $variable_name" >&2
    exit 1
  fi
done

if [[ ! "$APPLE_TEAM_ID" =~ ^[A-Za-z0-9]{10}$ ]]; then
  echo "Error: APPLE_TEAM_ID must contain exactly 10 letters or digits." >&2
  exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_dir="$root/release"
app_path="$release_dir/PicTrim/PicTrim.app"
version="$(node -p "require('$root/package.json').version")"
case "$(uname -m)" in
  arm64) arch="arm64" ;;
  x86_64) arch="x64" ;;
  *) echo "Error: unsupported macOS architecture: $(uname -m)" >&2; exit 1 ;;
esac

if [[ ! -d "$app_path" ]]; then
  echo "Error: application bundle not found: $app_path" >&2
  echo 'Run "npm run tauri:build" before this script.' >&2
  exit 1
fi

work_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/pictrim-sign.XXXXXX")"
keychain_path="$work_dir/signing.keychain-db"
certificate_path="$work_dir/developer-id.p12"
keychain_password="$(openssl rand -base64 32)"

cleanup() {
  security delete-keychain "$keychain_path" >/dev/null 2>&1 || true
  rm -rf "$work_dir"
}
trap cleanup EXIT

printf '%s' "$MAC_CSC_LINK" | openssl base64 -d -A -out "$certificate_path"
security create-keychain -p "$keychain_password" "$keychain_path"
security unlock-keychain -p "$keychain_password" "$keychain_path"
security set-keychain-settings -lut 21600 "$keychain_path"
security import "$certificate_path" \
  -k "$keychain_path" \
  -P "$MAC_CSC_KEY_PASSWORD" \
  -T /usr/bin/codesign \
  -T /usr/bin/security
security set-key-partition-list \
  -S apple-tool:,apple: \
  -s \
  -k "$keychain_password" \
  "$keychain_path" >/dev/null

identity="$(security find-identity -v -p codesigning "$keychain_path" \
  | sed -nE 's/^[[:space:]]*[0-9]+\) ([A-F0-9]{40}) "Developer ID Application:.*$/\1/p' \
  | head -n 1)"
if [[ -z "$identity" ]]; then
  echo "Error: the .p12 does not contain a valid Developer ID Application identity with its private key." >&2
  security find-identity -v -p codesigning "$keychain_path" >&2 || true
  exit 1
fi

echo "Signing nested macOS libraries..."
frameworks_path="$app_path/Contents/Frameworks"
if [[ -d "$frameworks_path" ]]; then
  while IFS= read -r -d '' candidate; do
    if file "$candidate" | grep -q 'Mach-O'; then
      codesign --force \
        --options runtime \
        --timestamp \
        --sign "$identity" \
        --keychain "$keychain_path" \
        "$candidate"
    fi
  done < <(find "$frameworks_path" -type f -print0)
fi

codesign --force \
  --options runtime \
  --timestamp \
  --sign "$identity" \
  --keychain "$keychain_path" \
  "$app_path"
codesign --verify --deep --strict --verbose=2 "$app_path"

notary_zip="$work_dir/PicTrim-notarization.zip"
ditto -c -k --keepParent "$app_path" "$notary_zip"
xcrun notarytool submit "$notary_zip" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_APP_SPECIFIC_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait
xcrun stapler staple "$app_path"
xcrun stapler validate "$app_path"
spctl --assess --type execute --verbose=4 "$app_path"

echo "Creating portable archive..."
cd "$root"
npm run release:package

echo "Creating signed and notarized DMG..."
dmg_stage="$work_dir/dmg"
mkdir -p "$dmg_stage"
ditto "$app_path" "$dmg_stage/PicTrim.app"
ln -s /Applications "$dmg_stage/Applications"
dmg_path="$release_dir/PicTrim-${version}-macos-${arch}.dmg"
rm -f "$dmg_path"
hdiutil create \
  -volname "PicTrim ${version}" \
  -srcfolder "$dmg_stage" \
  -ov \
  -format UDZO \
  "$dmg_path"
codesign --force \
  --timestamp \
  --sign "$identity" \
  --keychain "$keychain_path" \
  "$dmg_path"
codesign --verify --strict --verbose=2 "$dmg_path"
xcrun notarytool submit "$dmg_path" \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_APP_SPECIFIC_PASSWORD" \
  --team-id "$APPLE_TEAM_ID" \
  --wait
xcrun stapler staple "$dmg_path"
xcrun stapler validate "$dmg_path"
spctl --assess --type open \
  --context context:primary-signature \
  --verbose=4 \
  "$dmg_path"

echo "Signed and notarized macOS artifacts are ready in $release_dir"
