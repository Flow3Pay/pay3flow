#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

patterns='-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----|AKIA[0-9A-Z]{16}|sk_live_[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{30,}|[0-9]{3}-[0-9]{2}-[0-9]{4}'
set +e
matches="$(git grep -nEI -e "$patterns" -- ':!PLAN2.md' ':!docs/**' ':!scripts/secret-audit.sh' 2>&1)"
status=$?
set -e
if [[ $status -eq 0 ]]; then
  printf '%s\n' "$matches"
  echo "possible production secret found in tracked files" >&2
  exit 1
fi
if [[ $status -gt 1 ]]; then
  printf '%s\n' "$matches" >&2
  exit "$status"
fi

echo "secret audit passed (high-signal tracked-file patterns)"
