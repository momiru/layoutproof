// SPDX-FileCopyrightText: 2026 Morteza Taleblou
// SPDX-FileCopyrightText: 2026 Mitra Daneshmand
// SPDX-License-Identifier: MIT
//
// ==============================================================================
// MOMIRU LAYOUTPROOF | A02 RESEARCH SOURCE
// ==============================================================================
//
// Project:      Momiru LayoutProof (article A02)
// Component:    Native Momiru General/Light adapters
// Module:       momiru_layoutproof::adapters::mod
// Source:       adapters/mod.rs
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

//! Paper adapters for model snapshots and the pinned Momiru engines.

use crate::ir::{Fault, Layout, Permission, StorageId, ValueBits};

pub const LIGHT_MAX_RANK: usize = 8;

/// Import a caller-owned, read-only FP32 Light storage snapshot. Fixed-rank
/// metadata has to be supplied by the caller. No upstream engine is invoked.
pub fn light_snapshot<const R: usize>(
    shape: [usize; R],
    strides: [isize; R],
    origin: isize,
    storage: StorageId,
    backing: &[f32],
) -> Result<(Layout, Vec<ValueBits>), Fault> {
    if R > LIGHT_MAX_RANK {
        return Err(Fault::RankMismatch);
    }
    let strides = strides
        .into_iter()
        .map(|s| i64::try_from(s).map_err(|_| Fault::Overflow))
        .collect::<Result<Vec<_>, _>>()?;
    let layout = Layout {
        shape: shape.to_vec(),
        strides,
        origin: i64::try_from(origin).map_err(|_| Fault::Overflow)?,
        storage,
        capacity: backing.len(),
        permission: Permission::ReadOnly,
    };
    crate::lower::compile(&layout, &[], 0)?;
    Ok((layout, backing.iter().map(|v| v.to_bits()).collect()))
}

pub mod a02_momiru;
