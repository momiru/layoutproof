# Momiru LayoutProof (Rust)

Momiru LayoutProof is the native Rust research implementation for A02, *LayoutProof: Rank-Polymorphic Verification of Layout-Sensitive Tensor Transformations*. It checks when a proposed tensor view rewrite preserves logical coordinates, exact FP32 value bits, storage addresses, and write permissions. These obligations differ: a copy can preserve every logical value while changing which allocation subsequent operations observe.

The crate provides a typed layout IR, checked affine lowering, an independent bounded interpreter, and a solver replay path. It supports composed transpose, permutation, signed slice, flip, expansion, squeeze, broadcast, reshape, and explicit materialization. Tests include deliberate faults, empty and scalar domains, negative and zero strides, writable overlap, and ranks through 16 in the standalone model.

## Build and run

Rust 1.99.1 and Momiru commit `eba302d3c61313444b56303c8816f74a3aaf9679` are pinned. Fetch the locked Git dependency before running offline Cargo commands. Z3 on `PATH` is required for solver replay and is optional for finite checks.

```sh
cargo fetch --locked
cargo fmt --all -- --check
cargo check --offline --locked --all-targets --all-features
cargo test --offline --locked --all-targets --all-features
cargo clippy --offline --locked --all-targets --all-features -- -D warnings

cargo run --offline --locked --bin a02-layoutproof -- --list
cargo run --offline --locked --bin a02-layoutproof -- --paper A02 --case C08-composed --seed 7
cargo run --offline --locked --bin a02-layoutproof -- --mode symbolic --case C09-slice-fusion
cargo run --offline --locked --bin a02-layoutproof -- --mode affine --case C08-composed
cargo run --offline --locked --bin a02-layoutproof -- --mode engine-check --engine both --case C08-composed
```

The release runner supports allocation-inclusive development timings for cases that pass independent preflight and per-sample correctness checks. Use `--engine model` to time model lowering separately.

```sh
cargo run --offline --locked --release --bin a02-layoutproof -- \
  --paper A02 --engine both --case C08-composed --seed 7 --mode bench --warmup 3 --samples 10
```

For reproducibility, record the exact paper source state, including local changes, alongside the case ID, seed, compiler, Z3 version, host and command. The runner reports compiler and host target metadata with local timings. See [the A02 implementation and reproducibility notes](docs/a02/README.md).

## Scope and evidence

The CPU model and live-engine checks are separate development modes. The native adapter calls Momiru General's runtime-rank FP32 views and Light's const-rank FP32 views through their public APIs. It compares engine shapes, backing addresses and exact payloads to the independent interpreter. Engine-check admits a rewrite only after both native variants agree and the model verifies logical lineage and permissions. Mutable-view regressions test alias rejection and actual write post-state. Terminal materialization tests gather through the native view APIs, check the result of each engine's owned constructor against the interpreter, and verify nonempty source and destination storage are disjoint. Light supports ranks zero through eight. General tests include ranks nine and sixteen. Native reshapes are limited to the engines' forward-contiguous view contracts. [Library source pin](docs/a02/LIBRARY_SOURCE_PIN.txt) · [Source audit](docs/a02/AUDIT_A02_MOMIRU.md)

Finite checks apply to the concrete input. Fixed-shape SMT replay checks address obligations for the supplied input. The symbolic certificates encode directed definedness for selected axis-local rules, and replay on a concrete layout checks both programs before invoking Z3. Writable lease checks cover supplied live peer regions. Device execution, arbitrary-program proof, and confirmatory engine comparisons remain outside the verified scope.

Canonical QTC1, QTO1, NTE1, and native evidence documents or vectors are missing. Evidence mode reports `NOT_RUN CANONICAL_PROTOCOLS_MISSING` and produces no files. The development cases and diagnostic timings are for local validation.

Licensed under [MIT](LICENSE). Momiru maintainers are Morteza Taleblou and Mitra Daneshmand.
