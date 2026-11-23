# A02 Rust source-package release audit — 2026-11-23

**Status: SOURCE PACKAGE / NOT A SCIENTIFIC RESULTS RELEASE.**

## Upstream dependency and live-engine path

- Paper source: `momiru/layoutproof`, based on committed topic head `cd73d5c5da39b30597e57711107814d0fe7a80c3`.
- Native dependency: `taleblou/momiru@eba302d3c61313444b56303c8816f74a3aaf9679` in the real Cargo Git dependency AND lockfile.
- `adapters/a02_momiru.rs` invokes actual `momiru::tensor` and `momiru::tensor_light` APIs; the paper oracle and checker are intentionally separate independent implementations.
- `tests/a02_library_contract.rs` guards the manifest/lock/adapter SHA and links types from both upstream engine modules.
- `scripts/verify_native.sh` requires Cargo, Rust and Z3, runs native tests, a positive General+Light check, a negative rewrite and independent SMT query replay. The new CI check exercises both engines.
- Full local validation now requires a local Momiru checkout at the exact pin with unmodified modern sources; Cargo itself independently resolves the same pinned Git dependency.

## Verified for this downloadable archive

- Archive structure, file-content hashes, SPDX headers, local TOML/YAML syntax, Rust native dependency URI/pins, CI and shell-script syntax: checked offline.
- *Not executed here*: Rust/Cargo compilation, Z3, the upstream engine runtime and live GitHub Actions. This environment has no Rust toolchain, external Git network or Z3.
- Historical `BUILD_LOG_A02_MOMIRU.md` and `TEST_REPORT_A02_MOMIRU.md` describe different prior commits; they are **not** a pass for these new files.

## Must remain blocked before publication / `main` merge

1. Supplied latest committed snapshot omitted **one modified Rust path**. Recover and reconcile that path with the current `~/research-a02/papers/rust-paper` checkout; do not overwrite local work with this archive.
2. Run `bash scripts/verify_native.sh` on the user's Linux workstation; check required GitHub CI jobs against the final SHA.
3. Obtain the authoritative QTC1/QTO1/NTE1 contracts, portable cases and vectors from the project owner. Never invent them or publish dummy evidence.
4. Independently validate proof/counterexample soundness, performance comparators and reproducible raw data. Neither a green Cargo test nor a local PASS establishes paper-quality proof.
5. Review and approve the draft PR; merge only after all scientific and code gates are verified.

## Independent offline re-audit (2026-11-23)

- Scope: exact source files included in this archive, not the user's uncommitted Git worktree.
- Fixes/changes: Confirmed LightView rank registry has unique variants; made local Momiru checkout preflight mandatory; CI source-policy scans full checkout; added Bash syntax check; normalized build.rs formatting; made the missing-protocol evidence gate part of local native validation.
- Offline checks: ZIP archive integrity; manifest/config parsing; shell syntax; native adapter source and version pin consistency; repo-specific path/CI invariants; internal documentation references.
- Remaining release blockers: Authoritative QTC1/QTO1/NTE1 contracts, real native compilation on target machine, live GitHub CI and scientifically accepted measurements are not available here.
- In this environment neither Rust/Cargo nor Pixi/Mojo was available, so successful compilation or executed proof/speedup is **not asserted**.
- These changes are an independently reviewed **source candidate**, not a scientifically certified release.
