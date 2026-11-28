#!/usr/bin/env bash
#
# sync-design.sh - copy the app-design assets the browser screens embed.
#
# Source of truth: $APP_DESIGN_DIR (default ~/projects/perso/app-design)
# Destination:     crates/core/web/assets/
# Usage: sync-design.sh [--help]
# Exit codes: 0 success, 1 source missing, 2 invalid usage
set -euo pipefail
IFS=$'\n\t'

SRC="${APP_DESIGN_DIR:-${HOME}/projects/perso/app-design}"
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
DST="${ROOT}/crates/core/web/assets"
FILES=(css/tokens.css css/app.css js/app.js js/icons-data.js js/icons.js logo.svg sprite.svg)

case "${1:-}" in
  "") ;;
  -h | --help)
    sed -n '2,9p' "${BASH_SOURCE[0]}"
    exit 0
    ;;
  *)
    printf 'error: unknown option: %s\n' "$1" >&2
    exit 2
    ;;
esac

if [[ ! -d "${SRC}/assets" ]]; then
  printf 'error: app-design not found at %s (set APP_DESIGN_DIR)\n' "${SRC}" >&2
  exit 1
fi

mkdir -p "${DST}/css" "${DST}/js"
for f in "${FILES[@]}"; do
  cp -- "${SRC}/assets/${f}" "${DST}/${f}"
done
(cd -- "${DST}" && shasum -a 256 -- "${FILES[@]}" > MANIFEST.sha256)
printf 'synced %d files from %s\n' "${#FILES[@]}" "${SRC}" >&2
