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
// Module:       momiru_layoutproof::proof
// Source:       src/proof.rs
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

//! Exact fixed-shape affine claims and rank-parametric local rule certificates.

use crate::ir::{cardinality, Fault, Layout, Op, Rewrite, Status};
use crate::lower::{address, compile};
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AddressDecision {
    Equal,
    ShapeMismatch,
    StorageMismatch,
    Witness(Vec<usize>),
    CandidateUndefined(Fault),
    NotCompared,
}

/// Decide equality of validated affine output addresses, including storage
/// identity. This decision does not cover logical provenance or permissions.
pub fn affine_addresses(
    input: &Layout,
    rewrite: &Rewrite,
    budget: usize,
) -> Result<AddressDecision, Fault> {
    let left = compile(input, &rewrite.reference, budget)?;
    let right = match compile(input, &rewrite.candidate, budget) {
        Ok(plan) => plan,
        Err(Fault::WorkLimit) => return Err(Fault::WorkLimit),
        Err(fault) => return Ok(AddressDecision::CandidateUndefined(fault)),
    };
    compare_addresses(left.output(), right.output())
}

fn compare_addresses(left: &Layout, right: &Layout) -> Result<AddressDecision, Fault> {
    if left.shape != right.shape {
        return Ok(AddressDecision::ShapeMismatch);
    }
    if left.storage != right.storage {
        return Ok(AddressDecision::StorageMismatch);
    }
    if cardinality(&left.shape)? == 0 {
        return Ok(AddressDecision::Equal);
    }
    let mut point = vec![0; left.shape.len()];
    if address(left, &point)? != address(right, &point)? {
        return Ok(AddressDecision::Witness(point));
    }
    // The last active axis with a nonzero coefficient difference gives
    // the lexicographically first mismatch once the origins agree.
    if let Some(axis) = (0..left.shape.len())
        .rev()
        .find(|&a| left.shape[a] > 1 && left.strides[a] != right.strides[a])
    {
        point[axis] = 1;
        return Ok(AddressDecision::Witness(point));
    }
    Ok(AddressDecision::Equal)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Law {
    FlipInvolution,
    TransposeInvolution,
    ExpandThenSqueeze,
    SliceFusion,
}

/// A rank-polymorphic claim. Its precondition is a valid input on which the
/// entire reference program is defined. Unmentioned axes are unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate {
    pub law: Law,
    pub rewrite: Rewrite,
}

pub fn certify(rewrite: &Rewrite) -> Option<Certificate> {
    use Op::*;
    let law = match (rewrite.reference.as_slice(), rewrite.candidate.as_slice()) {
        ([Flip(a), Flip(b)], []) if a == b && has_representable_axis(*a) => Law::FlipInvolution,
        (
            [Transpose {
                first: a,
                second: b,
            }, Transpose {
                first: c,
                second: d,
            }],
            [],
        ) if has_representable_axis(*a)
            && has_representable_axis(*b)
            && ((a, b) == (c, d) || (a, b) == (d, c)) =>
        {
            Law::TransposeInvolution
        }
        ([Expand(a), Squeeze(b)], []) if a == b && has_representable_axis(*a) => {
            Law::ExpandThenSqueeze
        }
        ([Slice { .. }, Slice { .. }], [Slice { .. }]) if matches_slice_fusion(rewrite) => {
            Law::SliceFusion
        }
        _ => return None,
    };
    Some(Certificate {
        law,
        rewrite: rewrite.clone(),
    })
}

fn matches_slice_fusion(rule: &Rewrite) -> bool {
    let (
        [Op::Slice {
            axis: a,
            start: x,
            count: count1,
            step: s,
        }, Op::Slice {
            axis: b,
            start: y,
            count: n,
            step: t,
        }],
        [Op::Slice {
            axis: c,
            start: z,
            count: m,
            step: u,
        }],
    ) = (rule.reference.as_slice(), rule.candidate.as_slice())
    else {
        return false;
    };
    has_representable_axis(*a)
        && a == b
        && b == c
        && n == m
        && nonempty_slice_extent(*x, *count1, *s).is_some()
        && nonempty_slice_extent(*y, *n, *t).is_some_and(|need| need <= *count1)
        && i128::from(*s)
            .checked_mul(*y as i128)
            .and_then(|p| p.checked_add(*x as i128))
            == Some(*z as i128)
        && s.checked_mul(*t) == Some(*u)
}

fn nonempty_slice_extent(start: usize, count: usize, step: i64) -> Option<usize> {
    if step == 0 {
        return None;
    }
    let last = (count.checked_sub(1)? as i128)
        .checked_mul(i128::from(step))?
        .checked_add(start as i128)?;
    start.max(usize::try_from(last).ok()?).checked_add(1)
}

/// Upper rank bound from the signed byte limit for the layout's separate
/// shape and stride vectors. Available memory may impose a smaller bound.
pub const MAX_LAYOUT_RANK: usize = (isize::MAX as usize)
    / if size_of::<usize>() > size_of::<i64>() {
        size_of::<usize>()
    } else {
        size_of::<i64>()
    };

fn has_representable_axis(axis: usize) -> bool {
    axis < MAX_LAYOUT_RANK
}

/// Independent binding of a certificate to the typed program, followed by a
/// separately generated integer obligation for the affected coordinates.
pub fn replay_query(cert: &Certificate) -> Option<String> {
    use Op::*;
    let (variables, reference, candidate, obligation, binding) = match (
        cert.law, cert.rewrite.reference.as_slice(), cert.rewrite.candidate.as_slice()
    ) {
        (Law::FlipInvolution, [Flip(a), Flip(b)], []) if a == b && has_representable_axis(*a) => (
            "r n i", format!("(and (< {a} r) (<= 0 n))"), "true".to_owned(),
            "(=> (and (<= 0 i) (< i n)) (let ((j (- n 1 i))) (and (<= 0 j) (< j n) (= (- n 1 j) i))))".to_owned(),
            "true".to_owned(),
        ),
        (Law::TransposeInvolution, [Transpose { first: a, second: b }, Transpose { first: c, second: d }], [])
            if has_representable_axis(*a) && has_representable_axis(*b)
                && ((*c == *a && *d == *b) || (*c == *b && *d == *a)) => (
            "r m n i j", format!("(and (< {a} r) (< {b} r) (<= 0 m) (<= 0 n))"), "true".to_owned(),
            "(=> (and (<= 0 i) (< i m) (<= 0 j) (< j n)) (let ((out0 j) (out1 i)) (and (<= 0 out0) (< out0 n) (<= 0 out1) (< out1 m) (= out1 i) (= out0 j))))".to_owned(),
            "true".to_owned(),
        ),
        (Law::ExpandThenSqueeze, [Expand(a), Squeeze(b)], []) if a == b && has_representable_axis(*a) => (
            "r i j", format!("(and (<= {a} r) (< r {MAX_LAYOUT_RANK}))"), "true".to_owned(),
            "(=> (and (<= 0 i) (<= 0 j) (< j 1)) (and (= j 0) (= (+ i j) i)))".to_owned(),
            "true".to_owned(),
        ),
        (Law::SliceFusion,
            [Slice { axis: a, start: x, count: count1, step: s },
             Slice { axis: b, start: y, count: count2, step: t }],
            [Slice { axis: c, start: z, count: count3, step: u }])
            if has_representable_axis(*a) && a == b && b == c && count2 == count3 && *u != 0
                && nonempty_slice_extent(*x, *count1, *s).is_some()
                && nonempty_slice_extent(*y, *count2, *t).is_some_and(|need| need <= *count1) => {
                let (s, t, u) = (smt_int(*s), smt_int(*t), smt_int(*u));
                let nested = |i: &str| format!("(+ {x} (* {s} (+ {y} (* {t} {i}))))");
                let fused = |i: &str| format!("(+ {z} (* {u} {i}))");
                let first = slice_domain("n", *x, *count1 - 1, &s);
                let second = slice_domain(&count1.to_string(), *y, *count2 - 1, &t);
                let candidate = slice_domain("n", *z, *count3 - 1, &u);
                (
                    "r n i",
                    format!("(and (< {a} r) (<= 0 n) (<= n {}) {first} {second})", usize::MAX),
                    format!("(and (< {c} r) {candidate})"),
                    format!("(=> (and (<= 0 i) (< i {count2})) (and (= {} {}) (<= 0 {}) (< {} n)))",
                        nested("i"), fused("i"), fused("i"), fused("i")),
                    format!("(and (= {} {}) (= {} {}))",
                        nested("0"), fused("0"), nested("1"), fused("1")),
                )
            }
        _ => return None,
    };
    let mut query = String::from("(set-logic QF_LIA)\n");
    for var in variables.split_whitespace() {
        query.push_str(&format!("(declare-const {var} Int)\n"));
    }
    // A directed rewrite is required to be defined whenever the reference is
    // defined. The converse is deliberately outside this claim.
    query.push_str(&format!(
        "(assert (and (<= 0 r) (<= r {MAX_LAYOUT_RANK})))\n(define-fun reference-defined () Bool {reference})\n(define-fun candidate-defined () Bool {candidate})\n(assert (not (and {binding} (=> reference-defined (and candidate-defined {obligation})))))\n(check-sat)\n"
    ));
    Some(query)
}

fn slice_domain(extent: &str, start: usize, last_index: usize, step: &str) -> String {
    format!("(let ((last (+ {start} (* {step} {last_index})))) (and (< {start} {extent}) (<= 0 last) (< last {extent})))")
}

fn smt_int(n: i64) -> String {
    if n < 0 {
        format!("(- {})", n.unsigned_abs())
    } else {
        n.to_string()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayReason {
    Unsat,
    Sat,
    SolverUnknown,
    Budget,
    UnrecognizedCertificate,
    SolverMissing,
    SolverError,
    SolverDisagrees,
    DifferentDomains,
    DifferentStorage,
    ReferencePrecondition(Fault),
    CandidatePrecondition(Fault),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Replay {
    pub status: Status,
    pub reason: ReplayReason,
}

/// Replay the conditional law. Use replay_on_layout for a concrete decision.
pub fn replay(cert: &Certificate, timeout_ms: u32) -> Replay {
    let Some(query) = replay_query(cert) else {
        return Replay {
            status: Status::Rejected,
            reason: ReplayReason::UnrecognizedCertificate,
        };
    };
    run_z3(&query, timeout_ms)
}

/// Check the applicability of a conditional certificate to a concrete input
/// before replay. Callers supply the same live regions for both alternatives.
pub fn replay_on_layout(
    cert: &Certificate,
    input: &Layout,
    peers: &[Layout],
    budget: usize,
    timeout_ms: u32,
) -> Replay {
    use crate::lower::compile_with_peers;
    for (ops, is_reference) in [
        (&cert.rewrite.reference, true),
        (&cert.rewrite.candidate, false),
    ] {
        if let Err(fault) = compile_with_peers(input, ops, peers, budget) {
            return Replay {
                status: if fault == Fault::WorkLimit {
                    Status::Unknown
                } else if is_reference {
                    Status::Rejected
                } else {
                    Status::Fail
                },
                reason: if is_reference {
                    ReplayReason::ReferencePrecondition(fault)
                } else {
                    ReplayReason::CandidatePrecondition(fault)
                },
            };
        }
    }
    replay(cert, timeout_ms)
}

fn run_z3(query: &str, timeout_ms: u32) -> Replay {
    if timeout_ms == 0 {
        return Replay {
            status: Status::Unknown,
            reason: ReplayReason::Budget,
        };
    }
    let seconds = timeout_ms.div_ceil(1000).saturating_add(1);
    let child = Command::new("z3")
        .args([
            "-in",
            "-smt2",
            &format!("-T:{seconds}"),
            &format!("timeout={timeout_ms}"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Replay {
                status: Status::NotRun,
                reason: ReplayReason::SolverMissing,
            };
        }
        Err(_) => {
            return Replay {
                status: Status::Error,
                reason: ReplayReason::SolverError,
            }
        }
    };
    let wrote_query = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(query.as_bytes()).is_ok());
    let output = child.wait_with_output();
    if let Ok(output) = output {
        match String::from_utf8_lossy(&output.stdout).trim() {
            "timeout" | "unknown" => {
                return Replay {
                    status: Status::Unknown,
                    reason: ReplayReason::SolverUnknown,
                }
            }
            "unsat" if wrote_query && output.status.success() && output.stderr.is_empty() => {
                return Replay {
                    status: Status::Pass,
                    reason: ReplayReason::Unsat,
                };
            }
            "sat" if wrote_query && output.status.success() && output.stderr.is_empty() => {
                return Replay {
                    status: Status::Fail,
                    reason: ReplayReason::Sat,
                };
            }
            _ => {}
        }
    }
    Replay {
        status: Status::Error,
        reason: ReplayReason::SolverError,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct FixedReplay {
    pub addresses: AddressDecision,
    pub solver: Replay,
}

/// Replay an address-only certificate for a concrete model. The independent
/// interpreter verifies the affine encodings and the first mismatch before
/// Z3 checks satisfiability and, for a witness, its minimality.
pub fn replay_fixed(
    input: &Layout,
    storage: &[crate::ir::ValueBits],
    rewrite: &Rewrite,
    budget: usize,
    timeout_ms: u32,
) -> Result<FixedReplay, Fault> {
    replay_fixed_with_peers(input, storage, rewrite, &[], budget, timeout_ms)
}

/// Check concrete address equivalence under the supplied live-peer context.
pub fn replay_fixed_with_peers(
    input: &Layout,
    storage: &[crate::ir::ValueBits],
    rewrite: &Rewrite,
    peers: &[Layout],
    budget: usize,
    timeout_ms: u32,
) -> Result<FixedReplay, Fault> {
    use crate::lower::compile_with_peers;
    use crate::oracle::{interpret_with_peers, visit};
    use std::fmt::Write as _;

    if input.capacity != storage.len() {
        return Err(Fault::StorageExtent);
    }
    let evaluate = |ops: &[Op]| {
        (
            interpret_with_peers(input, storage, ops, peers, budget),
            compile_with_peers(input, ops, peers, budget),
        )
    };
    let disagreement = || FixedReplay {
        addresses: AddressDecision::NotCompared,
        solver: Replay {
            status: Status::Error,
            reason: ReplayReason::SolverDisagrees,
        },
    };
    let (observed_left, left) = match evaluate(&rewrite.reference) {
        (Ok(observed), Ok(plan)) => (observed, plan),
        (Err(Fault::WorkLimit), _) | (_, Err(Fault::WorkLimit)) => return Err(Fault::WorkLimit),
        (Err(a), Err(b)) if a == b => return Err(a),
        _ => return Ok(disagreement()),
    };
    let (observed_right, right) = match evaluate(&rewrite.candidate) {
        (Ok(observed), Ok(plan)) => (observed, plan),
        (Err(Fault::WorkLimit), _) | (_, Err(Fault::WorkLimit)) => return Err(Fault::WorkLimit),
        (Err(a), Err(b)) if a == b => {
            return Ok(FixedReplay {
                addresses: AddressDecision::CandidateUndefined(a),
                solver: Replay {
                    status: Status::Fail,
                    reason: ReplayReason::CandidatePrecondition(a),
                },
            })
        }
        _ => return Ok(disagreement()),
    };
    let (left, right) = (left.output(), right.output());
    let addresses = compare_addresses(left, right)?;
    let replay = |status, reason| FixedReplay {
        addresses: addresses.clone(),
        solver: Replay { status, reason },
    };
    if observed_left.shape != left.shape
        || observed_right.shape != right.shape
        || observed_left.storage != left.storage
        || observed_right.storage != right.storage
    {
        return Ok(replay(Status::Error, ReplayReason::SolverDisagrees));
    }
    if observed_left.shape != observed_right.shape {
        return Ok(replay(Status::Fail, ReplayReason::DifferentDomains));
    }
    if observed_left.storage != observed_right.storage {
        return Ok(replay(Status::Fail, ReplayReason::DifferentStorage));
    }
    let mut flat = 0;
    let mut exact = AddressDecision::Equal;
    let mut modeled = true;
    visit(&left.shape, |index| {
        let a = observed_left.cells[flat].address;
        let b = observed_right.cells[flat].address;
        flat += 1;
        modeled &= a == address(left, index)? && b == address(right, index)?;
        if a != b && exact == AddressDecision::Equal {
            exact = AddressDecision::Witness(index.to_vec());
        }
        Ok(())
    })?;
    if !modeled || exact != addresses {
        return Ok(replay(Status::Error, ReplayReason::SolverDisagrees));
    }
    let mut query = String::from("(set-logic QF_LIA)\n");
    for (axis, extent) in left.shape.iter().enumerate() {
        writeln!(
            query,
            "(declare-const x{axis} Int)\n(assert (and (<= 0 x{axis}) (< x{axis} {extent})))"
        )
        .expect("writing to String");
    }
    writeln!(
        query,
        "(assert (not (= {} {})))",
        affine_expression(left),
        affine_expression(right)
    )
    .expect("writing to String");
    let found = run_z3(&format!("{query}(check-sat)\n"), timeout_ms);
    match (&addresses, found.reason) {
        (AddressDecision::Equal, ReplayReason::Unsat) => {}
        (AddressDecision::Witness(witness), ReplayReason::Sat) => {
            // The interpreter already confirmed the witness. Ask Z3 whether
            // any earlier index exists, using a disjunction of strict prefixes.
            let mut prefix = String::new();
            for (axis, index) in witness.iter().enumerate() {
                write!(prefix, "(or (< x{axis} {index}) (and (= x{axis} {index}) ")
                    .expect("writing to String");
            }
            prefix.push_str("false");
            prefix.push_str(&"))".repeat(witness.len()));
            let earlier = run_z3(
                &format!("{query}(assert {prefix})\n(check-sat)\n"),
                timeout_ms,
            );
            if earlier.reason == ReplayReason::Sat {
                return Ok(replay(Status::Error, ReplayReason::SolverDisagrees));
            }
            if earlier.reason != ReplayReason::Unsat {
                return Ok(FixedReplay {
                    addresses,
                    solver: earlier,
                });
            }
        }
        (_, ReplayReason::Sat | ReplayReason::Unsat) => {
            return Ok(replay(Status::Error, ReplayReason::SolverDisagrees));
        }
        _ => {}
    }
    Ok(FixedReplay {
        addresses,
        solver: found,
    })
}

fn affine_expression(layout: &Layout) -> String {
    use std::fmt::Write as _;
    let mut term = format!("(+ {} 0", smt_int(layout.origin));
    for (axis, &stride) in layout.strides.iter().enumerate() {
        if stride != 0 {
            write!(term, " (* {} x{axis})", smt_int(stride)).expect("writing to String");
        }
    }
    term.push(')');
    term
}
