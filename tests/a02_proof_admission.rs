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
// Module:       A02 test: a02_proof_admission
// Source:       tests/a02_proof_admission.rs
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

use momiru_layoutproof::{
    ir::{Layout, Op, Permission, Rewrite, Status, StorageId},
    proof::{self, AddressDecision, Certificate, Law, ReplayReason},
};

fn rewrite(reference: Vec<Op>, candidate: Vec<Op>) -> Rewrite {
    Rewrite {
        reference,
        candidate,
    }
}

fn input() -> Layout {
    Layout {
        shape: vec![2],
        strides: vec![1],
        origin: 0,
        storage: StorageId(1),
        capacity: 2,
        permission: Permission::Exclusive,
    }
}

#[test]
fn certificates_reject_an_axis_with_no_representable_reference_rank() {
    let axis = usize::MAX;
    let slice = Op::Slice {
        axis,
        start: 0,
        count: 1,
        step: 1,
    };
    let transpose = Op::Transpose {
        first: 0,
        second: axis,
    };
    for (law, rule) in [
        (
            Law::FlipInvolution,
            rewrite(vec![Op::Flip(axis), Op::Flip(axis)], vec![]),
        ),
        (
            Law::TransposeInvolution,
            rewrite(vec![transpose.clone(), transpose], vec![]),
        ),
        (
            Law::ExpandThenSqueeze,
            rewrite(vec![Op::Expand(axis), Op::Squeeze(axis)], vec![]),
        ),
        (
            Law::SliceFusion,
            rewrite(vec![slice.clone(), slice.clone()], vec![slice]),
        ),
    ] {
        let supplied = Certificate {
            law,
            rewrite: rule.clone(),
        };
        assert_eq!(proof::replay_query(&supplied), None, "{law:?}");
        assert!(proof::certify(&rule).is_none(), "{law:?}");
        assert_eq!(
            proof::replay(&supplied, 0).reason,
            ReplayReason::UnrecognizedCertificate
        );
    }
}

#[test]
fn independently_observed_shape_or_storage_difference_refutes_fixed_equivalence() {
    for (candidate, witness) in [
        (Op::Expand(0), AddressDecision::ShapeMismatch),
        (
            Op::Materialize(StorageId(2)),
            AddressDecision::StorageMismatch,
        ),
    ] {
        let rule = rewrite(vec![], vec![candidate]);
        let result = proof::replay_fixed(&input(), &[3, 5], &rule, 100, 0).unwrap();
        assert_eq!(result.addresses, witness);
        assert_eq!(result.solver.status, Status::Fail);
    }
}

#[test]
fn fixed_replay_honors_live_peers_before_calling_the_solver() {
    use momiru_layoutproof::ir::Fault;

    let mut selected = input();
    selected.shape = vec![1];
    let mut peer = Layout {
        permission: Permission::ReadOnly,
        ..selected.clone()
    };
    let identity = rewrite(vec![], vec![]);
    let check = |peer: &Layout| {
        proof::replay_fixed_with_peers(
            &selected,
            &[3, 5],
            &identity,
            std::slice::from_ref(peer),
            100,
            0,
        )
    };
    assert_eq!(check(&peer), Err(Fault::LivePeerOverlap));
    peer.origin = 1;
    let disjoint = check(&peer).unwrap();
    assert_eq!(disjoint.addresses, AddressDecision::Equal);
    assert_eq!(disjoint.solver.status, Status::Unknown);
    assert_eq!(disjoint.solver.reason, ReplayReason::Budget);

    // The interpreter checks the actual backing extent even with no peers.
    assert_eq!(
        proof::replay_fixed(&selected, &[3], &identity, 100, 0),
        Err(Fault::StorageExtent)
    );
}
