#!/usr/bin/env bash
# Packs a release build of the app for one platform into dist/, as the
# release workflow (.github/workflows/release.yml) publishes it:
#
#     cargo build --release --locked -p wish-app --target <target>
#     tools/package-release.sh <target> <version>
#
# - Linux: genshin-companion-<version>-linux-<arch>.tar.gz, the binary with
#   the README, the licence and THIRD-PARTY-LICENSES.txt (the crates, icons
#   and font it is built with; tools/third-party-licenses.sh);
# - Windows: genshin-companion-<version>-windows-<arch>.zip, the same with the .exe;
# - macOS: genshin-companion-<version>-macos-<apple-silicon|intel>.zip, a
#   "Genshin Companion.app" bundle, signed ad hoc (not by a developer
#   account, so macOS asks before its first launch).
#
# Prints the archive's path.
set -euo pipefail

if [ $# -ne 2 ]; then
    echo "usage: $0 <target triple> <version>" >&2
    exit 2
fi
target=$1
version=$2
root=$(cd "$(dirname "$0")/.." && pwd)
build="$root/target/$target/release"
dist="$root/dist"
mkdir -p "$dist"

arch=${target%%-*}
case "$target" in
    *-linux-*) platform=linux ;;
    *-windows-*) platform=windows ;;
    aarch64-apple-darwin) platform=macos arch=apple-silicon ;;
    x86_64-apple-darwin) platform=macos arch=intel ;;
    *)
        echo "$0: no packaging for $target" >&2
        exit 1
        ;;
esac
name="genshin-companion-$version-$platform-$arch"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
# Beside the binary in every archive.
docs=("$root/README.md" "$root/LICENSE" "$root/crates/wish-app/licenses/THIRD-PARTY-LICENSES.txt")

case $platform in
    linux)
        mkdir "$stage/$name"
        cp "$build/genshin-companion" "${docs[@]}" "$stage/$name/"
        tar -C "$stage" -czf "$dist/$name.tar.gz" "$name"
        echo "$dist/$name.tar.gz"
        ;;
    windows)
        mkdir "$stage/$name"
        cp "$build/genshin-companion.exe" "${docs[@]}" "$stage/$name/"
        (cd "$stage" && 7z a -tzip "$dist/$name.zip" "$name" >/dev/null)
        echo "$dist/$name.zip"
        ;;
    macos)
        app="$stage/Genshin Companion.app"
        mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
        cp "$build/genshin-companion" "$app/Contents/MacOS/"
        cp "${docs[@]}" "$app/Contents/Resources/"
        sed "s/@VERSION@/$version/g" "$root/packaging/macos/Info.plist" >"$app/Contents/Info.plist"
        # Ad hoc: what Apple Silicon requires to run anything at all.
        codesign --force --deep --sign - "$app"
        # ditto keeps the bundle's attributes, which zip would drop.
        ditto -c -k --keepParent "$app" "$dist/$name.zip"
        echo "$dist/$name.zip"
        ;;
esac
