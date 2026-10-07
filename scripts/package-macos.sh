#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# Only the built .app enters the ZIP. Preferences, credentials and source .env
# files are not under this bundle and must never be added to a release asset.
scripts/bundle-macos.sh
version="$(cargo pkgid --offline | sed 's/.*[@#]//')"
arch="$(uname -m)"
output="${LINGO_DIST_DIR:-dist}"
asset="Lingo-v${version}-macos-${arch}.zip"
ditto -c -k --sequesterRsrc --keepParent "$output/Lingo.app" "$output/$asset"
(
  cd "$output"
  shasum -a 256 "$asset" > SHA256SUMS.txt
)
echo "Release archive: $output/$asset"
echo "Checksum: $output/SHA256SUMS.txt"
