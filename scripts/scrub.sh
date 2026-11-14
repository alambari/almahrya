#!/usr/bin/env bash
# Fails when a tracked text file matches a secret pattern. The repository is public.
#
# Patterns: the built-in generic ones below, plus one extended regex per line from a
# local, untracked file (default ~/.config/almahrya/scrub-patterns, override with
# ALMAHRYA_SCRUB_PATTERNS). A missing file is skipped silently, as in CI.
#
# Output is "file:line: matches <name>" only, so a leaked value is never echoed into
# logs. The built-in patterns are written as split string fragments so this file does
# not match itself and needs no exclusion.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

NAMES=()
PATTERNS=()
add() { NAMES+=("$1"); PATTERNS+=("$2"); }

add "private key header" '-----BEGIN .* PRIV''ATE KEY-----'
add "github token" 'gh''p_[A-Za-z0-9]{20,}'
add "slack token" 'xo''x[bp]-'
add "postgres url with credentials" 'post''gres://[^ ]+:[^ ]+@'

LOCAL="${ALMAHRYA_SCRUB_PATTERNS:-$HOME/.config/almahrya/scrub-patterns}"
if [ -f "$LOCAL" ]; then
  n=0
  while IFS= read -r line || [ -n "$line" ]; do
    n=$((n + 1))
    case "$line" in '' | '#'*) continue ;; esac
    add "local pattern (line $n)" "$line"
  done <"$LOCAL"
fi

found=0
i=0
while [ "$i" -lt "${#PATTERNS[@]}" ]; do
  status=0
  # git grep searches tracked files only; -I skips binary files.
  hits="$(git grep -I -n -E --no-color -e "${PATTERNS[$i]}" 2>/dev/null)" || status=$?
  if [ "$status" -eq 0 ]; then
    found=1
    printf '%s\n' "$hits" | sed -E "s/^([^:]*):([0-9]+):.*/\1:\2: matches ${NAMES[$i]}/"
  elif [ "$status" -ne 1 ]; then
    echo "scrub: could not run ${NAMES[$i]} (invalid pattern?)" >&2
    found=1
  fi
  i=$((i + 1))
done

if [ "$found" -ne 0 ]; then
  echo "scrub: failed" >&2
  exit 1
fi
echo "scrub: clean"
