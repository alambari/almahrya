#!/usr/bin/env bash
# Only crates/ui-kit may depend on GPUI. Spikes are throwaway experiments outside the
# workspace and are exempt, and so is the root manifest, which declares the pinned
# [workspace.dependencies] lines. Prints each offending file:line and exits 1.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

found=0
report() { # <message> <grep output>
  found=1
  printf '%s\n' "$2" | sed -E "s|^([^:]*:[0-9]+):.*|\1: $1|"
}

is_exempt() {
  case "$1" in crates/ui-kit/* | spikes/*) return 0 ;; esac
  return 1
}

# Tracked and untracked-but-not-ignored files, NUL separated.
while IFS= read -r -d '' file; do
  [ -f "$file" ] || continue
  is_exempt "$file" && continue
  case "$file" in
    Cargo.toml) ;;
    */Cargo.toml)
      if hits="$(grep -n 'gpui' "$file" /dev/null)"; then
        report "manifest mentions gpui outside crates/ui-kit" "$hits"
      fi
      ;;
    *.rs)
      if hits="$(grep -nE 'use gpui|gpui::|gpui_component' "$file" /dev/null)"; then
        report "source uses gpui outside crates/ui-kit" "$hits"
      fi
      ;;
  esac
done < <(git ls-files -z --cached --others --exclude-standard)

if [ "$found" -ne 0 ]; then
  echo "gpui-boundary: failed" >&2
  exit 1
fi
echo "gpui-boundary: clean"
