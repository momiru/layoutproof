# A02 Rust validation log

Validated with Rust and Cargo 1.99.1 and Z3 4.8.12 on x86-64 Linux. The initial paper HEAD was `78fe9de`. The independent Momiru source was clean at `eba302d3c61313444b56303c8816f74a3aaf9679`. All build and Cargo-cache output used the paper's ignored `target/a02-v7` directory.

## Paper

Commands below used `CARGO_HOME="$PWD/target/a02-v7/cargo-home"` and, for the final validation sequence, `CARGO_TARGET_DIR="$PWD/target/a02-v7/paper"`. Cargo generated the lockfile using a scoped Git URL rewrite from the manifest's canonical remote to the exact local Git object store. The lockfile retains the canonical URL and exact revision.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo generate-lockfile` with the scoped URL rewrite | 0 | Locked Momiru by its full Git revision. |
| `cargo test --offline --locked --target-dir target/a02-v7/paper --test a02_redteam_regressions` before semantic fixes | 101 | Definedness witnesses reproduced and bounds/alias precedence produced `ERROR` instead of `REJECTED`. |
| `cargo test --offline --locked --target-dir target/a02-v7/paper --test a02_momiru_link --test a02_redteam_regressions` | 0 | Five live-engine and six red-team regressions passed after fixes. |
| `cargo fmt --all -- --check` | 0 | Formatting gate. |
| `cargo check --offline --locked --all-targets --all-features` | 0 | All current paper targets including forwarded AOT compilation. |
| `cargo test --offline --locked --all-targets --all-features --quiet` | 0 | 27 passed. |
| `cargo test --offline --locked --release --all-targets --all-features --quiet` | 0 | 27 passed. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 0 | Paper gate passed. Upstream strict Clippy was run separately below. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode engine-check --engine both --case C08-composed --seed 7` | 0 | Both program variants matched the interpreter on each engine. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode affine --case C08-alias-mutant` | 1 | Expected `FAIL CandidatePrecondition(WriteDenied)`. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode query --case C09-slice-fusion \| z3 -in -smt2` with Bash `pipefail` | 0 | Separate Z3 process returned `unsat`. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode evidence --engine both --output-root target/a02-v7/evidence-probe` | 1 | `NOT_RUN CANONICAL_PROTOCOLS_MISSING`. |
| `test ! -e target/a02-v7/evidence-probe` | 0 | Evidence directory absent. |
| `git diff --check` | 0 | Tracked diff whitespace check. |

Workflow YAML comparison, `bash -n`, ShellCheck and the existing source-policy body succeeded. The only workflow differences are the two locked dependency-fetch steps. Hosted CI for the changed paper source remains unverified in this session.

## Independent Momiru checkout

Each native command used `--manifest-path /home/morteza/research-a02/libraries/momiru/Cargo.toml --target-dir target/a02-v7/upstream`. The checkout remained clean.

| Command with the above path flags | Exit | Result |
| --- | ---: | --- |
| `cargo test --offline --locked --test a02_borrowed_signed_views --test a02_general_injectivity --test a02_general_stepped_views --test a02_light_layout --test a02_stride_spacing` | 0 | 21 passed. |
| `cargo check --offline --locked --all-targets --all-features` | 0 | Both modules and all registered targets compiled. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 101 | `clippy::too_many_arguments` on eight-argument `cpu_sgemm_primary_seed` in `src/momiru/tensor_light/aot.rs:1315`. |
| `cargo test --offline --locked --features aot` | 0 | 85 unit tests, 21 integration tests and five doc tests passed. |
| `cargo test --offline --locked --release --lib --all-features` | 0 | 85 unit tests passed. |

The GitHub check-runs query for the library pin exited 1 with a connection error. Canonical evidence, engine timing and independent publication review remain `NOT_RUN`.

## Follow-up at paper HEAD f33bda2

The pinned dependency was unchanged. This run rebuilt into fresh ignored directories under `target/a02-paper-r2`. Paper commands used `CARGO_HOME="$PWD/target/a02-v7/cargo-home"` and `CARGO_TARGET_DIR="$PWD/target/a02-paper-r2/paper"`. Rust and Cargo 1.99.1 and Z3 4.8.12 were used.

| Paper command | Exit | Observation |
| --- | ---: | --- |
| `cargo test --offline --locked --test a02_proof_admission` | 101 → 0 | Two failures reproduced before correction. Three tests passed afterward. |
| `cargo fmt --all -- --check` | 0 | Formatting passed. |
| `cargo check --offline --locked --all-targets --all-features` | 0 | Check passed. |
| `cargo test --offline --locked --all-targets --all-features --quiet` | 0 | 36 passed. |
| `cargo test --offline --locked --release --all-targets --all-features --quiet` | 0 | 36 passed. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 0 | Paper Clippy passed. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode engine-check --engine both --case C08-composed` | 0 | Both native engines admitted the composition. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode engine-check --engine both --case C08-address-witness` | 1 | Both engines reported a failing rewrite, although each variant matched its own oracle. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode affine --case C08-materialize-mutant` | 1 | FAIL with a storage mismatch. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode query --case C09-slice-fusion \| z3 -in -smt2` with Bash `pipefail` | 0 | `unsat` for the encoded directed obligation. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode symbolic --case C09-slice-fusion --timeout-ms 0` | 1 | `UNKNOWN Budget`. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode evidence --engine both` | 1 | `NOT_RUN CANONICAL_PROTOCOLS_MISSING`. |
| `git diff --check` | 0 | Passed. |

For Momiru, append `--manifest-path /home/morteza/research-a02/libraries/momiru/Cargo.toml --target-dir target/a02-paper-r2/upstream` to the Cargo commands below.

| Momiru command | Exit | Observation |
| --- | ---: | --- |
| `cargo check --offline --locked --all-targets --all-features` | 0 | All 30 registered targets discovered and checked. |
| `cargo test --offline --locked --features aot --quiet` | 0 | 111 tests passed across unit, integration and doc suites. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 101 | Reproduced the upstream eight-argument AOT lint at `src/momiru/tensor_light/aot.rs:1315`. |

The GitHub check-runs request for the Momiru pin exited 1 because the API was unreachable. Hosted CI and canonical evidence were unverified. Native timing comparisons were NOT_RUN.

## Full live-context validation at paper HEAD 8f9e75f

Paper commands used `CARGO_HOME="$PWD/target/a02-v7/cargo-home"` and `CARGO_TARGET_DIR="$PWD/target/a02-paper-final/paper"`. The dependency remained pinned at `eba302d3c61313444b56303c8816f74a3aaf9679`.

| Command | Exit | Observed result |
| --- | ---: | --- |
| `cargo test --offline --locked --test a02_live_context` | 101 → 0 | Reproduced admission of conflicting peers before the fix. All three tests passed afterward. |
| `cargo fmt --all -- --check` | 0 | Passed. |
| `cargo check --offline --locked --all-targets --all-features` | 0 | Passed. |
| `cargo test --offline --locked --all-targets --all-features --quiet` | 0 | 43 tests passed. |
| `cargo test --offline --locked --release --all-targets --all-features --quiet` | 0 | 43 tests passed. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 0 | Passed for the paper crate. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode engine-check --engine both --case C08-composed` | 0 | Both native engines admitted the rewrite. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode engine-check --engine both --case C08-address-witness` | 1 | Both native engines reported a failing rewrite. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode query --case C09-slice-fusion \| z3 -in -smt2` with Bash `pipefail` | 0 | `unsat` for the emitted arithmetic obligation. |
| `z3 -in -smt2` for the independently encoded overlap of offsets `2+a` and `3-b` on `0 <= a,b < 2` | 0 | `sat`, with `a=1` and `b=0` both reaching offset 3. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode symbolic --case C09-slice-fusion --timeout-ms 0` | 1 | `UNKNOWN Budget`. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode evidence --engine both` | 1 | `NOT_RUN CANONICAL_PROTOCOLS_MISSING`. |
| `git diff --check` | 0 | Passed. |

The upstream commands used `--manifest-path /home/morteza/research-a02/libraries/momiru/Cargo.toml --target-dir target/a02-paper-final/upstream`. The upstream checkout remained clean.

| Upstream command with the above path flags | Exit | Observed result |
| --- | ---: | --- |
| `cargo check --offline --locked --all-targets --all-features` | 0 | Passed. |
| `cargo test --offline --locked --features aot --quiet` | 0 | 111 tests passed across unit, integration, and doc suites. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 101 | Reproduced `clippy::too_many_arguments` on `cpu_sgemm_primary_seed`. |

The GitHub API query for the pinned revision exited 1 with a connection error. Hosted CI remains unverified. Canonical performance runs remain `NOT_RUN`.
