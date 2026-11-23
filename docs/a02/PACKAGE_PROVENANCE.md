# Source-package provenance and status (2026-11-23)

This is an **independent source package** for the A02 Rust paper repository,
`https://github.com/momiru/layoutproof`. It is **not** a scientific results release.

- Base: last supplied **committed** source snapshot,
  `A02_LayoutProof_Paper_Code_DRAFT_20261123T111548337109Z.zip`,
  from the user's A02 diagnostic archive. Commit `cd73d5c5da39b30597e57711107814d0fe7a80c3`
  on `codex/a02-layoutproof`.
- Pinned Momiru library: `https://github.com/taleblou/momiru`,
  commit `eba302d3c61313444b56303c8816f74a3aaf9679`.
- The snapshot exporter excluded **one local uncommitted path** from Rust.
  Its content was not supplied and is not reconstructed here.
- Changes made for this package: SPDX headers on each Rust code file,
  explicit two-author MIT license text, a versioned Rust toolchain file,
  a `build.rs` to define the previously referenced `A02_RUSTC_VERSION` at compilation,
  and an integration regression checking that the benchmark reports the compiler.
- No native Cargo build, upstream library checkout, Z3 replay, or live GitHub CI
  was run **while producing this package**. Historical reports under `docs/a02/`
  are inherited from the snapshot and do not certify this revised package.

## Before publishing

Compare the entire package against the *current* branch and any outstanding
uncommitted changes in your local checkout. Do **not** overwrite newer Codex
work with this committed snapshot. Run, at minimum:

```sh
cargo fetch --locked
cargo fmt --all -- --check
cargo check --locked --all-targets --all-features
cargo test --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo run --locked --bin a02-layoutproof -- --mode query --case C09-slice-fusion | z3 -in -smt2
```

Existing QTC1, QTO1 and NTE1 research contracts and independent experimental
evidence are still required before publishing any scientific result.

## Header convention

Rust file headers mirror the Momiru SPDX and structured project/section metadata convention. The original upstream files were not rewritten; these paper sources use the same two-author attribution.

## Post-package dependency audit

Added a checked local native-verification script, a release-audit status document,
README verification instructions, and required native validation integration.
For the exact changes and remaining verification limits, use
[`RELEASE_AUDIT.md`](RELEASE_AUDIT.md). Earlier build reports are inherited and
must not be interpreted as a PASS for this packaged revision.
