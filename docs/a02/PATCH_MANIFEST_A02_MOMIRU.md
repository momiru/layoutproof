# Implementation manifest

| Path | Change and purpose |
| --- | --- |
| `README.md` | Replaces provisional root instructions with the paper crate's real entry points and scope. |
| `.gitignore` | Ignores local native build, cache and evidence output. |
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | Add the Rust 1.99.1 standalone paper package and binary with no external Cargo dependencies. |
| `build.rs` | Captures the Cargo-selected compiler version for development timing output. |
| `src/lib.rs`, `src/ir.rs` | Export the paper-only modules and typed layout/rewrite/status contract. |
| `src/lower.rs`, `src/oracle.rs` | Implement distinct affine and coordinate-interpreter paths. |
| `src/proof.rs` | Implements affine address analysis, minimized witnesses, symbolic certificates and separate Z3 replay. |
| `src/verify.rs`, `src/cases.rs` | Add concrete obligations, ablations, and deterministic development cases. |
| `src/main.rs` | Implements the native diagnostic, solver, benchmark, and canonical evidence-admission entry points. |
| `adapters/mod.rs` | Imports read-only, fixed-rank Light descriptors supplied by a caller. |
| `tests/a02_layoutproof.rs`, `tests/a02_runner.rs` | Add native semantics, solver and runner regression tests. |
| `docs/a02/README.md`, `docs/a02/AUDIT_A02_MOMIRU.md`, `docs/a02/APPLICABILITY_MATRIX.tsv` | Describe semantics, inspected upstream sources and experiment eligibility. |
| `docs/a02/PATCH_MANIFEST_A02_MOMIRU.md`, `docs/a02/BUILD_LOG_A02_MOMIRU.md`, `docs/a02/TEST_REPORT_A02_MOMIRU.md`, `docs/a02/HANDOFF_A02_MOMIRU.md` | Record the handoff, commands and test scope. |

The paper source and tests are new. The package retains the MIT license and the existing library pin. Solver replay uses a separately installed optional Z3 executable. Git publishing belongs to the orchestrator.
