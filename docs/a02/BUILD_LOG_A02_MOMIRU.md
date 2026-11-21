# Native build and execution log

The paper crate used rustc 1.99.1 (`58cf8f9c3`), Cargo 1.99.1 (`5f94df478`), and Z3 4.8.12 on `x86_64-unknown-linux-gnu`. The paper worktree started at `8416d9b2808afb581b51097f5a77bf4a62ef8843`, and validation used the local changes described in the patch manifest. This is a development validation record.

| Command | Exit | Observed outcome |
| --- | ---: | --- |
| `cargo generate-lockfile --offline` | 0 | Generated the package-only lockfile. |
| `cargo fmt --all -- --check` | 0 | Formatting gate. |
| `cargo check --offline --locked --all-targets --all-features` | 0 | All paper targets checked. |
| `cargo test --offline --locked --all-targets --all-features` | 0 | 16 passed, 0 failed, 0 ignored. |
| `cargo test --offline --locked --release --all-targets --all-features` | 0 | 16 passed, 0 failed, 0 ignored. |
| `cargo clippy --offline --locked --all-targets --all-features -- -D warnings` | 0 | Final run clean. The first run exited 101 for two avoidable test clones, which were corrected. |
| `cargo run --offline --locked --bin a02-layoutproof -- --paper A02 --case C08-composed --seed 7` | 0 | PASS on the concrete composition. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode manual --case C08-flip-involution` | 0 | PASS on the manually allowed cancellation. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode symbolic --case C09-slice-fusion` | 0 | Z3 returned UNSAT for the negated identity. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode affine --case C08-composed` | 0 | Z3 returned UNSAT for a fixed-shape address mismatch. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode affine --case C08-address-witness` | 1 | FAIL with independently checked SAT witness `[0, 0]`. |
| `cargo run --quiet --offline --locked --bin a02-layoutproof -- --mode query --case C09-slice-fusion \| z3 -in -smt2` with pipefail | 0 | Separate solver process returned `unsat`. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode symbolic --case C09-slice-fusion --timeout-ms 0` | 1 | UNKNOWN due to zero budget. No solver was started. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode evidence --engine both --output-root target/a02-evidence-smoke` | 1 | NOT_RUN. No evidence directory was created. |
| `cargo run --offline --locked --release --bin a02-layoutproof -- --paper A02 --engine model --case C08-composed --seed 7 --mode bench --warmup 1 --samples 2` | 0 | One local diagnostic warmup and two samples. No canonical run. |
| `cargo run --offline --locked --bin a02-layoutproof -- --mode bench --case C08-materialize-mutant` | 1 | The oracle rejected the candidate. No timing samples. |
| `cargo run --offline --locked --bin a02-layoutproof -- --engine general` | 1 | UNSUPPORTED because live engine linkage is unavailable. |
| `cargo package --offline --locked --allow-dirty --list` | 0 | Source whitelist verified. |
| `cargo package --offline --locked --allow-dirty` | 0 | Packaged and compiled the standalone source archive. |
| `git diff --check` | 0 | Tracked diff whitespace check. |

Tests exercised solver replay on the installed Z3. Hyperfine availability was confirmed. Publication measurements remain NOT_RUN. Upstream Cargo commands were NOT_RUN because the separate repository has missing declared sources and is outside this work order's writable scope.
