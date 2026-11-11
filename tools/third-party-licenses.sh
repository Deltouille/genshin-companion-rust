#!/usr/bin/env bash
# Writes crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt: the licence of every
# crate in the app (cargo-about, see about.toml), then those of what the app
# embeds that cargo-about doesn't list: the Lucide icons (inside gpui-kit-assets,
# which says Apache-2.0 only) and the Google Sans Flex font.
#
# Run it after changing Cargo.lock, and commit the file: the app shows it on its
# Credits page and every release archive holds it. The CI fails when it is stale.
#
#   nix shell nixpkgs#cargo-about -c tools/third-party-licenses.sh
#
# It fails when a crate's licence is not one about.toml accepts.
set -euo pipefail
cd "$(dirname "$0")/.."

out=crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt
crates=$(mktemp)
trap 'rm -f "$crates"' EXIT

cargo-about generate --locked --offline about.hbs -o "$crates"

assets=$(cargo metadata --format-version 1 --locked --offline \
  | jq -r '.packages[] | select(.name == "gpui-kit-assets") | .manifest_path')
assets=$(dirname "$assets")

section() {
  printf '\n%s\n%s\n\n' "--------------------------------------------------------------------------------" "$1"
  cat "$2"
}

{
  cat "$crates"
  section "Lucide icons, through gpui-kit-assets (ISC; some icons from Feather, MIT)" \
    "$assets/LICENSE-LUCIDE"
  section "Google Sans Flex font (SIL Open Font License 1.1)" \
    crates/wish-ui/fonts/GoogleSansFlex-OFL.txt
} > "$out"
echo "$out: $(wc -c < "$out") bytes"
