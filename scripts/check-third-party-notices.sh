#!/usr/bin/env bash
# SPDX-License-Identifier: MIT

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lockfile="$root/Cargo.lock"
notices="$root/THIRD-PARTY-NOTICES.md"
section="Cargo Dependencies"
no_dependencies="No third-party Cargo dependencies are currently used."

fail() {
  printf '%s\n' "$*" >&2
  exit 1
}

section_text="$(
  awk -v heading="## $section" '
    $0 == heading {
      in_section = 1
      next
    }
    in_section && /^## / {
      exit
    }
    in_section {
      print
    }
  ' "$notices"
)"

if [[ -z "${section_text//[[:space:]]/}" ]]; then
  fail "THIRD-PARTY-NOTICES.md is missing a '## $section' section"
fi

expected="$(
  awk '
    /^\[\[package\]\]/ {
      if (name != "" && source != "") {
        print name
      }
      name = ""
      source = ""
      next
    }
    /^name = / {
      name = $0
      sub(/^name = "/, "", name)
      sub(/"$/, "", name)
      next
    }
    /^source = / {
      source = $0
      next
    }
    END {
      if (name != "" && source != "") {
        print name
      }
    }
  ' "$lockfile" | sort -u
)"

listed="$(
  printf '%s\n' "$section_text" |
    sed -n 's/^[[:space:]]*-[[:space:]]*\[\([A-Za-z0-9_.-][A-Za-z0-9_.-]*\)\](.*/\1/p' |
    sort -u
)"

if [[ -z "$expected" ]]; then
  if ! grep -Fqx "$no_dependencies" <<<"$section_text"; then
    fail "THIRD-PARTY-NOTICES.md should state that no third-party Cargo dependencies are currently used"
  fi

  if [[ -n "$listed" ]]; then
    fail "Remove stale Cargo dependency notices:"$'\n'"$listed"
  fi

  printf '%s\n' "third-party notices are current: no Cargo dependencies"
  exit 0
fi

if grep -Fqx "$no_dependencies" <<<"$section_text"; then
  fail "THIRD-PARTY-NOTICES.md still says there are no third-party Cargo dependencies"
fi

missing="$(comm -23 <(printf '%s\n' "$expected") <(printf '%s\n' "$listed"))"
stale="$(comm -13 <(printf '%s\n' "$expected") <(printf '%s\n' "$listed"))"
has_errors=0

if [[ -n "$missing" ]]; then
  printf '%s\n' "Add missing Cargo dependency notices:" >&2
  while IFS= read -r name; do
    [[ -z "$name" ]] && continue
    printf -- '- [%s](https://crates.io/crates/%s) - TODO: license\n' "$name" "$name" >&2
  done <<<"$missing"
  has_errors=1
fi

if [[ -n "$stale" ]]; then
  [[ "$has_errors" -eq 1 ]] && printf '\n' >&2
  printf '%s\n' "Remove stale Cargo dependency notices:" >&2
  printf '%s\n' "$stale" >&2
  has_errors=1
fi

if [[ "$has_errors" -eq 1 ]]; then
  exit 1
fi

count="$(wc -l <<<"$expected" | tr -d ' ')"
printf 'third-party notices are current: %s Cargo dependencies\n' "$count"
