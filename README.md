# Momiru LayoutProof (Rust)

Momiru LayoutProof is the native Rust research implementation for A02, *LayoutProof: Rank-Polymorphic Verification of Layout-Sensitive Tensor Transformations*. It checks when a proposed tensor view rewrite preserves logical coordinates, exact FP32 value bits, storage addresses, and write permissions. These obligations differ: a copy can preserve every logical value while changing which allocation subsequent operations observe.

The crate provides a typed layout IR, checked affine lowering, an independent bounded interpreter, and a solver replay path. It supports composed transpose, permutation, signed slice, flip, expansion, squeeze, broadcast, reshape, and explicit materialization. Tests include deliberate faults, empty and scalar domains, negative and zero strides, writable overlap, and ranks through 16 in the standalone model.

## Build and run

Rust 1.99.1 is pinned. Cargo dependencies are limited to the Rust standard library. Z3 on `PATH` is required for solver replay and is optional for finite checks.

```sh
cargo fmt --all -- --check
cargo check --offline --locked --all-targets --all-features
cargo test --offline --locked --all-targets --all-features
cargo clippy --offline --locked --all-targets --all-features -- -D warnings

cargo run --offline --locked --bin a02-layoutproof -- --list
cargo run --offline --locked --bin a02-layoutproof -- --paper A02 --case C08-composed --seed 7
cargo run --offline --locked --bin a02-layoutproof -- --mode symbolic --case C09-slice-fusion
cargo run --offline --locked --bin a02-layoutproof -- --mode affine --case C08-composed
```

The release runner supports allocation-inclusive development timings for cases that pass independent preflight and postflight correctness checks.

```sh
cargo run --offline --locked --release --bin a02-layoutproof -- \
  --paper A02 --engine model --case C08-composed --seed 7 --mode bench --warmup 3 --samples 10
```

For reproducibility, record the exact paper source state, including local changes, alongside the case ID, seed, compiler, Z3 version, host and command. The runner reports compiler and host target metadata with local timings. See [the A02 implementation and reproducibility notes](docs/a02/README.md).

## Scope and evidence

This paper crate runs a CPU model. It is independent of the upstream Momiru General and Light engines. The Light adapter imports caller-supplied read-only FP32 metadata and backing bits, with a rank limit of eight. There is no live engine linkage. The separate [library source pin](docs/a02/LIBRARY_SOURCE_PIN.txt) records the last committed revision and its dirty checkout qualification. The [source audit](docs/a02/AUDIT_A02_MOMIRU.md) records missing General and benchmark sources.

Finite checks apply to the concrete input. Fixed-shape SMT replay establishes address obligations for the checked input. Rank-polymorphic SMT replay covers selected local rules under their preconditions. Arbitrary rewrite programs, mutating kernels, device execution, and production-engine equivalence remain outside the verified scope.

Canonical QTC1, QTO1, NTE1, and native evidence documents or vectors are missing. Evidence mode reports `NOT_RUN CANONICAL_PROTOCOLS_MISSING` and produces no files. The development cases and diagnostic timings are for local validation.

Licensed under [MIT](LICENSE). Momiru maintainers are Morteza Taleblou and Mitra Daneshmand.
