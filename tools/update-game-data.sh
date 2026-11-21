#!/usr/bin/env bash
# Updates the game data the app embeds (crates/wish-core/data/): brings the two
# source clones up to date, making them the first time, then runs both
# extraction scripts. Run it when a new banner, character or weapon comes out,
# then review what changed and run the tests before committing.
#
# Usage: tools/update-game-data.sh
set -euo pipefail

cd "$(dirname "$0")/.."

# Brings the clone in folder $1 up to date, or makes it from $2 with only the
# folders $3…: a shallow, sparse clone, which downloads nothing else.
update() {
  local dir=$1 url=$2
  shift 2
  if [ -d "$dir/.git" ]; then
    echo "Updating $dir"
    git -C "$dir" pull --ff-only --quiet
    # The folders added to the list since the clone (a new language). A full
    # clone has them all already.
    if [ "$(git -C "$dir" config --bool core.sparseCheckout || true)" = true ]; then
      git -C "$dir" sparse-checkout add "$@"
    fi
  else
    echo "Cloning $url into $dir"
    git clone --quiet --depth 1 --filter=blob:none --sparse "$url" "$dir"
    git -C "$dir" sparse-checkout set "$@"
  fi
}

# paimon-moe: the banners and the items a wish can give. Its src/ only: the
# site's images (static/) are a gigabyte nothing here reads.
update reference https://github.com/MadeBaruna/paimon-moe src

# genshin-db: characters, weapons, materials and their texts, in English and
# in the languages of LANGUAGES in tools/extract-genshin-db.mjs.
paths=(src/data/image src/data/stats src/data/curve src/data/version)
for language in English French; do
  for category in characters talents weapons materials constellations; do
    paths+=("src/data/$language/$category")
  done
done
update genshin-db https://github.com/theBowja/genshin-db "${paths[@]}"

node tools/extract-paimon-data.mjs
node tools/extract-genshin-db.mjs

echo
if git diff --quiet -- crates/wish-core/data; then
  echo "The game data is already up to date."
else
  git --no-pager diff --stat -- crates/wish-core/data
  echo
  echo "Review the changes (git diff crates/wish-core/data), run cargo test --workspace, then commit them."
fi
