// SPDX-FileCopyrightText: 2026 Morteza Taleblou
// SPDX-FileCopyrightText: 2026 Mitra Daneshmand
// SPDX-License-Identifier: MIT
//
// ==============================================================================
// MOMIRU LAYOUTPROOF | A02 RESEARCH SOURCE
// ==============================================================================
//
// Project:      Momiru LayoutProof (article A02)
// Component:    Native paper model, verifier and executable
// Module:       momiru_layoutproof::ir
// Source:       src/ir.rs
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

//! Paper-only IR. All offsets and strides are measured in elements.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StorageId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permission {
    ReadOnly,
    /// Exclusive use of this view's reachable region for the modeled lifetime.
    Exclusive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Layout {
    pub shape: Vec<usize>,
    pub strides: Vec<i64>,
    pub origin: i64,
    pub storage: StorageId,
    pub capacity: usize,
    pub permission: Permission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Address {
    pub storage: StorageId,
    pub offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Op {
    Transpose {
        first: usize,
        second: usize,
    },
    /// Output axis j draws from input axis axes[j].
    Permute(Vec<usize>),
    /// Exactly count coordinates, starting at start with the signed step.
    Slice {
        axis: usize,
        start: usize,
        count: usize,
        step: i64,
    },
    Flip(usize),
    Expand(usize),
    Squeeze(usize),
    /// Trailing alignment. Always yields a read-only lease.
    Broadcast(Vec<usize>),
    /// Preserve row-major traversal by an affine signed view.
    Reshape(Vec<usize>),
    /// Copy logical values into fresh dense storage; grants its own exclusive lease.
    Materialize(StorageId),
    /// Assert exclusive write access at this point in the program.
    RequireWritable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rewrite {
    pub reference: Vec<Op>,
    pub candidate: Vec<Op>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fault {
    RankMismatch,
    AxisOutOfRange,
    InvalidPermutation,
    InvalidSlice,
    InvalidSqueeze,
    InvalidBroadcast,
    ShapeMismatch,
    Overflow,
    Bounds,
    StorageExtent,
    StorageReuse,
    RequiresCopy,
    WriteDenied,
    WriteOverlap,
    LivePeerOverlap,
    WorkLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Pass,
    Rejected,
    Fail,
    Unknown,
    Unsupported,
    NotRun,
    Error,
}

impl Status {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Rejected => "REJECTED",
            Self::Fail => "FAIL",
            Self::Unknown => "UNKNOWN",
            Self::Unsupported => "UNSUPPORTED",
            Self::NotRun => "NOT_RUN",
            Self::Error => "ERROR",
        }
    }
}

/// Exact IEEE binary32 payload. A layout rewrite performs no floating arithmetic.
pub type ValueBits = u32;

pub fn cardinality(shape: &[usize]) -> Result<usize, Fault> {
    if shape.contains(&0) {
        return Ok(0);
    }
    shape.iter().try_fold(1_usize, |size, &n| {
        size.checked_mul(n).ok_or(Fault::Overflow)
    })
}
