# A02 paper change manifest

| Paths | Change |
| --- | --- |
| `Cargo.toml`, `Cargo.lock` | Exact upstream Momiru Git dependency, AOT feature forwarding, and Git-source distribution configuration. |
| `adapters/mod.rs`, `adapters/a02_momiru.rs`, `src/main.rs` | Live General and Light adapters and distinct engine-check CLI entry point. The prior snapshot adapter and model modes remain available. |
| `src/ir.rs`, `src/lower.rs`, `src/oracle.rs`, `src/verify.rs` | Declared live peer regions, storage identity reservation, and bounds-first overlap validation in the independent interpreter. |
| `src/proof.rs` | Directed SMT definedness and concrete certificate admission, with candidate fault classification aligned to full verification. |
| `tests/a02_momiru_link.rs`, `tests/a02_redteam_regressions.rs` | New live-engine and source-audit regressions. |
| `.github/workflows/ci.yml` | Fetch the pinned Git dependency before existing offline CPU and release gates. |
| `README.md`, `docs/a02/README.md`, `docs/a02/APPLICABILITY_MATRIX.tsv`, `docs/a02/AUDIT_A02_MOMIRU.md`, `docs/a02/BUILD_LOG_A02_MOMIRU.md`, `docs/a02/TEST_REPORT_A02_MOMIRU.md`, `docs/a02/HANDOFF_A02_MOMIRU.md`, `docs/a02/PATCH_MANIFEST_A02_MOMIRU.md` | Scope, reproduction, validation and evidence blockers. |

`docs/a02/LIBRARY_SOURCE_PIN.txt` already had a local orchestrator update to the exact dependency revision when this work began. It was preserved.


## Follow-up proof admission

- `src/proof.rs` rejects certificates with impossible rank guards, reports observed shape and storage mismatches as failures, and adds concrete replay with explicit live peers.
- `tests/a02_proof_admission.rs` covers those corrections without modifying the existing tests.
- `README.md` and `docs/a02/{README,BUILD_LOG_A02_MOMIRU,TEST_REPORT_A02_MOMIRU,PATCH_MANIFEST_A02_MOMIRU}.md` describe the current admission paths and validation.
