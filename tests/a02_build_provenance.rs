// SPDX-FileCopyrightText: 2026 Morteza Taleblou
// SPDX-FileCopyrightText: 2026 Mitra Daneshmand
// SPDX-License-Identifier: MIT
//
// ==============================================================================
// MOMIRU LAYOUTPROOF | A02 RESEARCH SOURCE
// ==============================================================================
//
// Project:      Momiru LayoutProof (article A02)
// Component:    A02 integration and regression tests
// Module:       A02 test: a02_build_provenance
// Source:       tests/a02_build_provenance.rs
// Language:     Rust
// Architecture: Independent paper verifier with pinned native Momiru adapters
//
// ------------------------------------------------------------------------------
// ENGINEERING AND RESEARCH CONTRACT
// ------------------------------------------------------------------------------
// Preserve checked tensor-layout semantics, exact address/value obligations,
// explicit alias and write permissions, and honest unsupported outcomes.
// Keep bounded diagnostics separate from verified scientific evidence.
// Respect the pinned Momiru API and provide regression coverage for edits.
//
// Authors:      Morteza Taleblou; Mitra Daneshmand
// Organization: https://github.com/momiru
// Repository:   https://github.com/momiru/layoutproof
// License:      MIT (see LICENSE)
// ==============================================================================

//! Verify the actual build script supplies the compiler version to the runner.

use std::process::Command;

#[test]
fn diagnostic_output_includes_compiler_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_a02-layoutproof"))
        .args(["--mode", "bench", "--warmup", "0", "--samples", "1"])
        .output()
        .expect("cannot run the paper executable");
    assert!(output.status.success(), "{:?}", output);
    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    assert!(stdout.contains("Development timing only"));
    assert!(stdout.contains("rustc "), "missing compiler provenance: {stdout}");
}
