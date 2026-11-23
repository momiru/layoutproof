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
// Module:       momiru_layoutproof::verify
// Source:       src/verify.rs
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

//! Differential development checks and explicit ablations.

use crate::ir::{Fault, Layout, Op, Rewrite, Status, ValueBits};
use crate::{lower, oracle};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Full,
    NoAddress,
    NoAliasPrecondition,
    /// Model-only control. Keep write assertions and ignore cross-view overlap.
    NoLivePeerCheck,
    FiniteOnly,
    Manual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reason {
    Checked,
    Input(Fault),
    CandidatePrecondition(Fault),
    LoweringDisagrees,
    Shape,
    Permission,
    AllocationState,
    Logical(Vec<usize>),
    Value(Vec<usize>),
    Address(Vec<usize>),
    NotOnManualAllowlist,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision {
    pub status: Status,
    pub reason: Reason,
}

impl Decision {
    /// A development timing gate. Ablations never confer full approval.
    pub fn eligible(&self, mode: Mode) -> bool {
        mode == Mode::Full && self.status == Status::Pass && self.reason == Reason::Checked
    }
}

fn decision(status: Status, reason: Reason) -> Decision {
    Decision { status, reason }
}

fn reference_error(fault: Fault) -> Decision {
    let status = if fault == Fault::WorkLimit {
        Status::Unknown
    } else {
        Status::Rejected
    };
    decision(status, Reason::Input(fault))
}

pub fn check(
    input: &Layout,
    storage: &[ValueBits],
    rule: &Rewrite,
    budget: usize,
    mode: Mode,
) -> Decision {
    check_with_peers(input, storage, rule, &[], budget, mode)
}

pub fn check_with_peers(
    input: &Layout,
    storage: &[ValueBits],
    rule: &Rewrite,
    peers: &[Layout],
    budget: usize,
    mode: Mode,
) -> Decision {
    if input.capacity != storage.len() {
        return reference_error(Fault::StorageExtent);
    }
    if mode == Mode::Manual && !manually_allowed(rule) {
        return decision(Status::Unsupported, Reason::NotOnManualAllowlist);
    }
    let check_peer_overlap = mode != Mode::NoLivePeerCheck;
    let baseline = oracle::interpret_in_context(
        input,
        storage,
        &rule.reference,
        peers,
        budget,
        check_peer_overlap,
    );
    let model = if mode == Mode::FiniteOnly || mode == Mode::Manual {
        None
    } else {
        Some(lower::compile_in_context(
            input,
            &rule.reference,
            peers,
            budget,
            check_peer_overlap,
        ))
    };
    let expected = match reconcile(baseline, model, input, storage, mode != Mode::NoAddress) {
        Ok(value) => value,
        Err(Reason::Input(fault)) => return reference_error(fault),
        Err(reason) => return decision(Status::Error, reason),
    };

    let mut candidate_ops = rule.candidate.clone();
    if mode == Mode::NoAliasPrecondition {
        candidate_ops.retain(|op| *op != crate::ir::Op::RequireWritable);
    }
    let candidate = oracle::interpret_in_context(
        input,
        storage,
        &candidate_ops,
        peers,
        budget,
        check_peer_overlap,
    );
    let model = if mode == Mode::FiniteOnly || mode == Mode::Manual {
        None
    } else {
        Some(lower::compile_in_context(
            input,
            &candidate_ops,
            peers,
            budget,
            check_peer_overlap,
        ))
    };
    let observed = match reconcile(candidate, model, input, storage, mode != Mode::NoAddress) {
        Ok(value) => value,
        Err(Reason::Input(Fault::WorkLimit)) => return reference_error(Fault::WorkLimit),
        Err(Reason::Input(fault)) => {
            return decision(Status::Fail, Reason::CandidatePrecondition(fault))
        }
        Err(reason) => return decision(Status::Fail, reason),
    };
    if expected.shape != observed.shape {
        return decision(Status::Fail, Reason::Shape);
    }
    if expected.permission != observed.permission {
        return decision(Status::Fail, Reason::Permission);
    }
    if mode != Mode::NoAddress && expected.storage != observed.storage {
        return decision(Status::Fail, Reason::Address(Vec::new()));
    }

    // Allocation IDs and extents remain visible to continuation programs.
    if mode != Mode::NoAddress && expected.allocations != observed.allocations {
        return decision(Status::Fail, Reason::AllocationState);
    }

    // Choose the first witness in row-major order. Report value before logical
    // provenance when both differ, so exact bit mismatches are observable.
    for (flat, (left, right)) in expected.cells.iter().zip(&observed.cells).enumerate() {
        let coordinate = || lower::coordinate(&expected.shape, flat);
        // An address mismatch is still separable from equal values. In the
        // no-address ablation, logical provenance remains checked.
        let reason = if left.value != right.value {
            Reason::Value(coordinate())
        } else if left.input_ordinal != right.input_ordinal {
            Reason::Logical(coordinate())
        } else if mode != Mode::NoAddress && left.address != right.address {
            Reason::Address(coordinate())
        } else {
            continue;
        };
        return decision(Status::Fail, reason);
    }
    decision(Status::Pass, Reason::Checked)
}

fn manually_allowed(rule: &Rewrite) -> bool {
    rule.reference == rule.candidate
        || matches!(
            (rule.reference.as_slice(), rule.candidate.as_slice()),
            ([Op::Flip(a), Op::Flip(b)] | [Op::Expand(a), Op::Squeeze(b)], []) if a == b
        )
}

fn reconcile(
    interpreted: Result<oracle::Observation, Fault>,
    compiled: Option<Result<lower::Lowered, Fault>>,
    input: &Layout,
    storage: &[ValueBits],
    compare_address: bool,
) -> Result<oracle::Observation, Reason> {
    let Some(compiled) = compiled else {
        return interpreted.map_err(Reason::Input);
    };
    match (interpreted, compiled) {
        (Err(Fault::WorkLimit), _) | (_, Err(Fault::WorkLimit)) => {
            Err(Reason::Input(Fault::WorkLimit))
        }
        (Err(left), Err(right)) if left == right => Err(Reason::Input(left)),
        (Ok(obs), Ok(plan))
            if obs.shape == plan.output.shape
                && obs.permission == plan.output.permission
                && obs.allocations == plan.allocations
                && (!compare_address || obs.storage == plan.output.storage) =>
        {
            let mut flat = 0;
            oracle::visit(&obs.shape, |index| {
                let cell = &obs.cells[flat];
                flat += 1;
                let address = lower::address(&plan.output, index)?;
                let source = plan.source_ordinal(index)?;
                let read = if address.storage == input.storage {
                    address
                } else {
                    lower::address(input, &lower::coordinate(&input.shape, source))?
                };
                if (compare_address && address != cell.address)
                    || storage.get(read.offset) != Some(&cell.value)
                    || source != cell.input_ordinal
                {
                    return Err(Fault::Bounds);
                }
                Ok(())
            })
            .map_err(|_| Reason::LoweringDisagrees)?;
            Ok(obs)
        }
        _ => Err(Reason::LoweringDisagrees),
    }
}
