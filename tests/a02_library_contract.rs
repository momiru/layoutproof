// SPDX-FileCopyrightText: 2026 Morteza Taleblou
// SPDX-FileCopyrightText: 2026 Mitra Daneshmand
// SPDX-License-Identifier: MIT
//
// ==============================================================================
// MOMIRU LAYOUTPROOF | A02 RESEARCH SOURCE
// ==============================================================================
//
// Project:      Momiru LayoutProof (article A02)
// Component:    Native dependency and exact-commit contract tests
// Module:       A02 test: a02_library_contract
// Source:       tests/a02_library_contract.rs
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

#![forbid(unsafe_code)]

use momiru_layoutproof::adapters::a02_momiru::REVISION;

#[test]
fn manifest_lock_and_adapter_agree_on_exact_upstream_commit() {
    let manifest = include_str!("../Cargo.toml");
    let lock = include_str!("../Cargo.lock");
    let pin_statement = format!("rev = \"{REVISION}\"");
    assert!(
        manifest.contains(&pin_statement),
        "Cargo.toml must pin the same Momiru commit as the native adapter"
    );
    assert!(
        lock.contains(&format!("#{REVISION}")),
        "Cargo.lock must resolve the exact Momiru commit"
    );
    assert!(manifest.contains("https://github.com/taleblou/momiru.git"));
    assert!(lock.contains("git+https://github.com/taleblou/momiru.git"));
}

#[test]
fn both_upstream_engine_error_types_link_at_compile_time() {
    // These types must come from the real Git dependency; a local paper model
    // cannot satisfy these absolute external-crate paths.
    let general = std::any::type_name::<momiru::tensor::TensorError>();
    let light = std::any::type_name::<momiru::tensor_light::TensorError>();
    assert!(general.contains("momiru::tensor::"));
    assert!(light.contains("momiru::tensor_light::"));
}
