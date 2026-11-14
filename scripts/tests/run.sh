#!/usr/bin/env bash
# Tests for scripts/scrub.sh and scripts/gpui-boundary.sh. Each case builds a
# throwaway git repository and runs the scripts with it as the working directory.
# Fixture secrets are assembled from fragments at runtime so this file never
# contains a token-shaped string itself.
set -euo pipefail

SCRIPTS="$(cd "$(dirname "$0")/.." && pwd)"
SCRATCH="$(mktemp -d "${TMPDIR:-/private/tmp}/almahrya-script-tests.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

PASS=0
FAIL=0
NO_PATTERNS="$SCRATCH/no-such-patterns"

ok() { PASS=$((PASS + 1)); echo "ok   - $1"; }
bad() { FAIL=$((FAIL + 1)); echo "FAIL - $1"; }

new_repo() {
  REPO="$SCRATCH/repo.$((PASS + FAIL)).$RANDOM"
  mkdir -p "$REPO"
  git -C "$REPO" init -q
  git -C "$REPO" config user.email t@example.invalid
  git -C "$REPO" config user.name t
  echo "hello" >"$REPO/README.md"
}

commit_all() {
  git -C "$REPO" add -A
  git -C "$REPO" commit -q -m fixture
}

# run_in <script> [ENV=value ...]: sets STATUS and OUTPUT.
run_in() {
  local script="$1"
  shift
  STATUS=0
  OUTPUT="$(cd "$REPO" && env "$@" bash "$SCRIPTS/$script" 2>&1)" || STATUS=$?
}

expect() { # name want-status
  if [ "$STATUS" -eq "$2" ]; then ok "$1"; else bad "$1 (status $STATUS, wanted $2): $OUTPUT"; fi
}

token="ghp_$(printf 'a%.0s' $(seq 1 24))"

# --- scrub.sh ---------------------------------------------------------------
new_repo; commit_all
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub passes on a clean repo" 0

new_repo; mkdir -p "$REPO/sub"; echo "x" >"$REPO/sub/a.txt"; commit_all
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
cd "$REPO/sub"; STATUS=0; OUTPUT="$(env ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS bash "$SCRIPTS/scrub.sh" 2>&1)" || STATUS=$?; cd "$SCRATCH"
expect "scrub works from a subdirectory" 0

new_repo; printf 'key = %s\n' "$token" >"$REPO/leak.txt"; commit_all
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub fails on a token-shaped string" 1
case "$OUTPUT" in *leak.txt:1:*) ok "scrub names file and line" ;; *) bad "scrub names file and line: $OUTPUT" ;; esac
case "$OUTPUT" in *"$token"*|*aaaaaaaa*) bad "scrub output must not echo the match" ;; *) ok "scrub output never contains the token" ;; esac

new_repo; printf 'key = %s\n' "$token" >"$REPO/leak.txt"
git -C "$REPO" add leak.txt   # staged only is still tracked
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub covers staged tracked files" 1

new_repo; printf 'key = %s\n' "$token" >"$REPO/leak.txt"   # untracked
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub ignores untracked files" 0

new_repo; printf 'key = %s\n\000bin\n' "$token" >"$REPO/blob.bin"; commit_all
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub skips binary files" 0

pk="-----BEGIN RSA PRIV""ATE KEY-----"
new_repo; echo "$pk" >"$REPO/k.pem"; commit_all
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub fails on a private key header" 1

new_repo; echo "word zzz-placeholder-zzz here" >"$REPO/note.txt"; commit_all
printf '%s\n' '# comment' '' 'zzz-placeholder-zzz' >"$SCRATCH/local-patterns"
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$SCRATCH/local-patterns"
expect "scrub fails on a local pattern" 1
case "$OUTPUT" in *note.txt:1:*) ok "local pattern names file and line" ;; *) bad "local pattern names file and line: $OUTPUT" ;; esac
case "$OUTPUT" in *zzz-placeholder-zzz*) bad "scrub output must not echo the local pattern" ;; *) ok "scrub output never contains the local pattern" ;; esac
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$NO_PATTERNS"
expect "scrub skips a missing local pattern file" 0

new_repo; echo "ordinary" >"$REPO/note.txt"; commit_all
run_in scrub.sh "ALMAHRYA_SCRUB_PATTERNS=$SCRATCH/local-patterns"
expect "scrub passes when the local pattern is absent" 0

# --- gpui-boundary.sh -------------------------------------------------------
gp="gp""ui"
new_repo
mkdir -p "$REPO/crates/ui-kit/src" "$REPO/crates/app/src" "$REPO/spikes/x/src"
printf '[workspace.dependencies]\n%s = "1"\n' "$gp" >"$REPO/Cargo.toml"
printf '[dependencies]\n%s.workspace = true\n' "$gp" >"$REPO/crates/ui-kit/Cargo.toml"
printf 'use %s::App;\n' "$gp" >"$REPO/crates/ui-kit/src/lib.rs"
printf '[dependencies]\n%s = "1"\n' "$gp" >"$REPO/spikes/x/Cargo.toml"
printf 'use %s::App;\n' "$gp" >"$REPO/spikes/x/src/main.rs"
printf '[dependencies]\nserde = "1"\n' >"$REPO/crates/app/Cargo.toml"
printf 'fn main() {}\n' >"$REPO/crates/app/src/main.rs"
commit_all
run_in gpui-boundary.sh
expect "boundary passes on a clean tree" 0

printf '[dependencies]\n%s.workspace = true\n' "$gp" >"$REPO/crates/app/Cargo.toml"
run_in gpui-boundary.sh
expect "boundary fails on a member manifest" 1
case "$OUTPUT" in *crates/app/Cargo.toml:2*) ok "boundary names the manifest line" ;; *) bad "boundary names the manifest line: $OUTPUT" ;; esac
printf '[dependencies]\nserde = "1"\n' >"$REPO/crates/app/Cargo.toml"

printf 'use %s::App;\n' "$gp" >"$REPO/crates/app/src/main.rs"
run_in gpui-boundary.sh
expect "boundary fails on 'use' in a source file" 1
case "$OUTPUT" in *crates/app/src/main.rs:1*) ok "boundary names the source line" ;; *) bad "boundary names the source line: $OUTPUT" ;; esac

printf 'fn f() { let _ = %s::px(1.0); }\n' "$gp" >"$REPO/crates/app/src/main.rs"
run_in gpui-boundary.sh
expect "boundary fails on a path use" 1

printf 'use %s_component::Button;\n' "$gp" >"$REPO/crates/app/src/main.rs"
run_in gpui-boundary.sh
expect "boundary fails on the component crate" 1

echo
echo "$PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
