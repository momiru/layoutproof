#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Morteza Taleblou
# SPDX-FileCopyrightText: 2026 Mitra Daneshmand
# SPDX-License-Identifier: MIT
# Source: scripts/verify_native.sh
# Purpose: Run the paper verifier against the exact pinned Momiru dependency.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$root"
pin=eba302d3c61313444b56303c8816f74a3aaf9679
upstream=${MOMIRU_SOURCE_DIR:-"$root/../../libraries/momiru"}

printf '%s\n' 'A02 Rust | PRE-PUBLICATION native validation (not scientific evidence)'
for tool in cargo rustc git z3; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'ERROR: required tool is not installed/on PATH: %s\n' "$tool" >&2
    exit 3
  fi
done
if ! grep -Fq "rev = \"$pin\"" Cargo.toml || \
   ! grep -Fq "#$pin\"" Cargo.lock || \
   ! grep -Fq "pub const REVISION: &str = \"$pin\";" adapters/a02_momiru.rs; then
  printf '%s\n' 'ERROR: Cargo manifest, lock and native adapter do not have the same library pin' >&2
  exit 4
fi
if ! git -C "$upstream" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  printf 'ERROR: required local Momiru checkout is missing or is not a Git repository: %s\n' "$upstream" >&2
  printf '%s\n' 'Set MOMIRU_SOURCE_DIR to a checkout of the pinned library commit.' >&2
  exit 5
fi
observed=$(git -C "$upstream" rev-parse --verify HEAD)
if [[ "$observed" != "$pin" ]]; then
  printf 'ERROR: local Momiru HEAD %s differs from paper pin %s\n' "$observed" "$pin" >&2
  printf '%s\n' 'Update Cargo.toml/Cargo.lock/adapter/docs only after validating a new library revision.' >&2
  exit 5
fi
if [[ -n $(git -C "$upstream" status --porcelain -- src/momiru) ]]; then
  printf '%s\n' 'ERROR: local Momiru src/momiru has modified or untracked files; use a clean pinned revision' >&2
  exit 5
fi
printf '%s\n' 'PASS: exact Git revision contract (manifest / lock / adapter)'

cargo fetch --locked
cargo fmt --all -- --check
cargo check --offline --locked --all-targets --all-features
cargo test --offline --locked --all-targets --all-features
cargo clippy --offline --locked --all-targets --all-features -- -D warnings

positive_output=$(cargo run --quiet --offline --locked --bin a02-layoutproof -- \
  --mode engine-check --engine both --case C08-composed --seed 7)
printf '%s\n' "$positive_output"
for engine in General Light; do
  if ! grep -Fq "PASS NativeRewrite EquivalentObservations engine=$engine" <<<"$positive_output"; then
    printf 'ERROR: %s native positive observation is missing\n' "$engine" >&2
    exit 6
  fi
done
# This negative control MUST fail for the expected reason in BOTH real engines.
set +e
negative_output=$(cargo run --quiet --offline --locked --bin a02-layoutproof -- \
  --mode engine-check --engine both --case C08-address-witness --seed 7 2>&1)
negative_status=$?
set -e
printf '%s\n' "$negative_output"
if [[ "$negative_status" -ne 1 ]]; then
  printf 'ERROR: negative control returned %s (expected 1)\n' "$negative_status" >&2
  exit 6
fi
for engine in General Light; do
  if ! grep -Fq "FAIL NativeRewrite" <<<"$negative_output" || \
     ! grep -Fq "engine=$engine case=C08-address-witness" <<<"$negative_output"; then
    printf 'ERROR: missing failing native negative result for %s\n' "$engine" >&2
    exit 6
  fi
done

# Re-run a generated proof query through an independent Z3 process.
solver_answer=$(cargo run --quiet --offline --locked --bin a02-layoutproof -- \
  --mode query --case C09-slice-fusion | z3 -in -smt2)
if [[ "$solver_answer" != unsat ]]; then
  printf 'ERROR: independent Z3 replay did not return unsat: %s\n' "$solver_answer" >&2
  exit 7
fi
# A pre-publication source candidate must NEVER pretend to have the missing canonical evidence.
if [[ -e "$root/.a02-research-evidence" ]]; then
  printf '%s\n' 'ERROR: evidence test path exists; inspect it before running the gate' >&2
  exit 7
fi
evidence_status=0
evidence_output=$(cargo run --quiet --offline --locked --bin a02-layoutproof -- \
  --mode evidence --output-root "$root/.a02-research-evidence") || evidence_status=$?
if [[ "$evidence_status" -ne 1 || "$evidence_output" != *'NOT_RUN CANONICAL_PROTOCOLS_MISSING'* || -e "$root/.a02-research-evidence" ]]; then
  printf 'ERROR: missing-contract gate did not fail closed (exit=%s): %s\n' "$evidence_status" "$evidence_output" >&2
  exit 7
fi
printf '%s\n' 'PASS: Rust native tests, both Momiru engines, negative control and Z3 replay'
printf '%s\n' 'NOT_CERTIFIED: this is not a QTC1/QTO1/NTE1 evidence or publication-quality benchmark run'
