use momiru_layoutproof::{
    adapters, cases,
    ir::{Fault, Layout, Op, Permission, Rewrite, Status, StorageId},
    lower, oracle,
    proof::{self, AddressDecision, Law, ReplayReason},
    verify::{self, Mode, Reason},
};

fn layout(shape: &[usize], strides: &[i64], origin: i64, capacity: usize) -> Layout {
    Layout {
        shape: shape.into(),
        strides: strides.into(),
        origin,
        storage: StorageId(1),
        capacity,
        permission: Permission::Exclusive,
    }
}

fn rules(reference: Vec<Op>, candidate: Vec<Op>) -> Rewrite {
    Rewrite {
        reference,
        candidate,
    }
}

fn check(input: &Layout, data: &[u32], rewrite: &Rewrite) -> verify::Decision {
    verify::check(input, data, rewrite, cases::BUDGET, Mode::Full)
}

#[test]
fn development_cases_and_ablation_admission() {
    for id in cases::IDS {
        let case = cases::get(id, 0xA02).unwrap();
        let verdict = check(&case.layout, &case.storage, &case.rewrite);
        assert_eq!(verdict.status, case.expected, "{id}: {verdict:?}");
        assert_eq!(verdict.eligible(Mode::Full), case.expected == Status::Pass);
    }
    for (id, mode, reason) in [
        (
            "C08-materialize-mutant",
            Mode::NoAddress,
            Reason::Address(vec![]),
        ),
        (
            "C08-alias-mutant",
            Mode::NoAliasPrecondition,
            Reason::CandidatePrecondition(Fault::WriteDenied),
        ),
    ] {
        let case = cases::get(id, 0xA02).unwrap();
        assert_eq!(
            check(&case.layout, &case.storage, &case.rewrite).reason,
            reason
        );
        let ablated = verify::check(
            &case.layout,
            &case.storage,
            &case.rewrite,
            cases::BUDGET,
            mode,
        );
        assert_eq!(ablated.status, Status::Pass);
        assert!(!ablated.eligible(mode));
    }
    let case = cases::get("C08-composed", 0).unwrap();
    let finite = verify::check(
        &case.layout,
        &case.storage,
        &case.rewrite,
        cases::BUDGET,
        Mode::FiniteOnly,
    );
    assert_eq!(finite.status, Status::Pass);
    let allowlisted = cases::get("C08-flip-involution", 0).unwrap().rewrite;
    assert_eq!(
        verify::check(
            &case.layout,
            &case.storage,
            &allowlisted,
            cases::BUDGET,
            Mode::Manual
        )
        .status,
        Status::Pass
    );
    assert_eq!(
        verify::check(
            &case.layout,
            &case.storage,
            &case.rewrite,
            cases::BUDGET,
            Mode::Manual
        )
        .status,
        Status::Unsupported
    );
}

#[test]
fn same_rule_across_rank_and_empty_boundaries() {
    for rank in 0..=16 {
        let input = layout(&vec![1; rank], &vec![i64::MIN; rank], 0, 1);
        let rewrite = rules(vec![Op::Expand(rank), Op::Squeeze(rank)], vec![]);
        assert_eq!(check(&input, &[0x8000_0000], &rewrite).status, Status::Pass);
        if rank > 0 {
            let rewrite = rules(vec![Op::Flip(rank - 1), Op::Flip(rank - 1)], vec![]);
            assert_eq!(check(&input, &[0], &rewrite).status, Status::Pass);
        }
    }
    let input = layout(&[0, usize::MAX, usize::MAX], &[i64::MAX, i64::MIN, 0], 1, 1);
    let rewrite = rules(
        vec![
            Op::Flip(0),
            Op::Transpose {
                first: 1,
                second: 2,
            },
            Op::Transpose {
                first: 2,
                second: 1,
            },
            Op::Reshape(vec![0]),
        ],
        vec![Op::Reshape(vec![0])],
    );
    assert_eq!(check(&input, &[9], &rewrite).status, Status::Pass);
    assert_eq!(
        oracle::interpret(&input, &[9], &rewrite.candidate, 100)
            .unwrap()
            .cells
            .len(),
        0
    );
    assert_eq!(lower::address(&input, &[0, 0, 0]), Err(Fault::Bounds));
}

#[test]
fn signed_slices_broadcast_and_exact_payloads() {
    let input = layout(&[1, 3], &[0, -2], 5, 7);
    let data = [0, 0x7fc0_0001, 0, 0x8000_0000, 0, 0x0000_0001, 0];
    let op = Op::Slice {
        axis: 1,
        start: 2,
        count: 2,
        step: -2,
    };
    let rewrite = rules(
        vec![op.clone(), Op::Broadcast(vec![4, 2])],
        vec![Op::Broadcast(vec![4, 3]), op],
    );
    assert_eq!(check(&input, &data, &rewrite).status, Status::Pass);
    let observed = oracle::interpret(&input, &data, &rewrite.reference, 100).unwrap();
    assert_eq!(observed.permission, Permission::ReadOnly);
    assert_eq!(
        observed
            .cells
            .iter()
            .map(|cell| cell.value)
            .collect::<Vec<_>>(),
        [
            0x7fc0_0001,
            1,
            0x7fc0_0001,
            1,
            0x7fc0_0001,
            1,
            0x7fc0_0001,
            1
        ]
    );
    let read_only = rules(vec![Op::Broadcast(vec![1, 3])], vec![]);
    assert_eq!(check(&input, &data, &read_only).reason, Reason::Permission);
    let reverse = rules(vec![Op::Flip(1)], vec![]);
    assert_eq!(
        check(&input, &data, &reverse).reason,
        Reason::Value(vec![0, 0])
    );
    assert_eq!(
        check(&input, &[42; 7], &reverse).reason,
        Reason::Logical(vec![0, 0])
    );
}

#[test]
fn invalid_operations_and_writable_overlap_are_rejected() {
    let input = layout(&[2, 3], &[3, 1], 0, 6);
    for (op, fault) in [
        (
            Op::Transpose {
                first: 3,
                second: 1,
            },
            Fault::AxisOutOfRange,
        ),
        (Op::Permute(vec![0, 0]), Fault::InvalidPermutation),
        (Op::Flip(3), Fault::AxisOutOfRange),
        (Op::Expand(3), Fault::AxisOutOfRange),
        (Op::Squeeze(1), Fault::InvalidSqueeze),
        (Op::Broadcast(vec![1, 3]), Fault::InvalidBroadcast),
        (Op::Reshape(vec![7]), Fault::ShapeMismatch),
        (
            Op::Slice {
                axis: 1,
                start: 1,
                count: 2,
                step: -2,
            },
            Fault::InvalidSlice,
        ),
        (Op::Materialize(StorageId(1)), Fault::StorageReuse),
    ] {
        let verdict = check(&input, &[0; 6], &rules(vec![op.clone()], vec![]));
        assert_eq!(verdict.status, Status::Rejected, "{op:?}: {verdict:?}");
        assert_eq!(verdict.reason, Reason::Input(fault), "{op:?}");
    }
    for input in [layout(&[2, 2], &[1, 1], 0, 3), layout(&[2], &[0], 0, 1)] {
        let verdict = check(&input, &vec![0; input.capacity], &rules(vec![], vec![]));
        assert_eq!(verdict.reason, Reason::Input(Fault::WriteOverlap));
    }
    let gapped_injective = layout(&[2, 3], &[3, 2], 0, 8);
    assert_eq!(
        check(&gapped_injective, &[0; 8], &rules(vec![], vec![])).status,
        Status::Pass
    );
    assert_eq!(
        lower::compile(&gapped_injective, &[], 5).err(),
        Some(Fault::WorkLimit)
    );
}

#[test]
fn overflow_and_resource_limits() {
    let input = layout(&[usize::MAX, 2], &[0, 0], 0, 1);
    assert_eq!(
        check(&input, &[0], &rules(vec![], vec![])).reason,
        Reason::Input(Fault::Overflow)
    );
    let input = layout(&[2], &[-4], 3, 4);
    assert_eq!(
        check(&input, &[0; 4], &rules(vec![], vec![])).reason,
        Reason::Input(Fault::Bounds)
    );
    let input = layout(&[1], &[i64::MIN], 0, 1);
    let rewrite = rules(vec![Op::Broadcast(vec![1000])], vec![]);
    let result = verify::check(&input, &[0], &rewrite, 4, Mode::Full);
    assert_eq!(result.status, Status::Unknown);
    assert_eq!(result.reason, Reason::Input(Fault::WorkLimit));
    let signed_singleton = rules(
        vec![Op::Slice {
            axis: 0,
            start: 0,
            count: 1,
            step: i64::MIN,
        }],
        vec![],
    );
    assert_eq!(check(&input, &[0], &signed_singleton).status, Status::Pass);
}

#[test]
fn reshape_view_and_materialization_have_distinct_requirements() {
    let input = layout(&[2, 3], &[-3, -1], 5, 6);
    let data = [0, 1, 2, 3, 4, 5];
    let reshape = Op::Reshape(vec![6]);
    let plan = lower::compile(&input, std::slice::from_ref(&reshape), 100).unwrap();
    assert_eq!(plan.output().strides, [-1]);
    let input = layout(&[2, 3], &[1, 2], 0, 6);
    assert_eq!(
        lower::compile(&input, std::slice::from_ref(&reshape), 100).err(),
        Some(Fault::RequiresCopy)
    );
    let rewrite = rules(
        vec![Op::Materialize(StorageId(2)), reshape.clone()],
        vec![
            Op::Materialize(StorageId(2)),
            Op::Reshape(vec![3, 2]),
            reshape,
        ],
    );
    assert_eq!(check(&input, &data, &rewrite).status, Status::Pass);
    let observed = oracle::interpret(&input, &data, &rewrite.reference, 100).unwrap();
    assert_eq!(
        observed
            .cells
            .iter()
            .map(|cell| cell.value)
            .collect::<Vec<_>>(),
        [0, 2, 4, 1, 3, 5]
    );
}

#[test]
fn bounded_stride_matrix_replays_minimal_address_witnesses() {
    for first in -3_i64..=3 {
        for second in -3_i64..=3 {
            let mut input = layout(&[2, 3], &[first, second], 9, 19);
            input.permission = Permission::ReadOnly;
            let backing: Vec<_> = (0..19).collect();
            for axis in 0..2 {
                let positive = rules(vec![Op::Flip(axis), Op::Flip(axis)], vec![]);
                assert_eq!(check(&input, &backing, &positive).status, Status::Pass);
                let mutant = rules(vec![Op::Flip(axis)], vec![]);
                assert_eq!(check(&input, &backing, &mutant).status, Status::Fail);
                let left = oracle::interpret(&input, &backing, &mutant.reference, 100).unwrap();
                let right = oracle::interpret(&input, &backing, &[], 100).unwrap();
                let witness = left
                    .cells
                    .iter()
                    .zip(right.cells)
                    .position(|(a, b)| a.address != b.address);
                let expected = witness.map_or(AddressDecision::Equal, |i| {
                    AddressDecision::Witness(vec![i / 3, i % 3])
                });
                assert_eq!(proof::affine_addresses(&input, &mutant, 0), Ok(expected));
            }
        }
    }
}

#[test]
fn symbolic_laws_replay_and_tampering_fails() {
    let fusion = cases::get("C09-slice-fusion", 0).unwrap().rewrite;
    for rewrite in [
        fusion.clone(),
        rules(vec![Op::Flip(1042), Op::Flip(1042)], vec![]),
        rules(
            vec![
                Op::Transpose {
                    first: 3,
                    second: 12,
                },
                Op::Transpose {
                    first: 12,
                    second: 3,
                },
            ],
            vec![],
        ),
        rules(vec![Op::Expand(1042), Op::Squeeze(1042)], vec![]),
    ] {
        let cert = proof::certify(&rewrite).unwrap();
        assert_eq!(proof::replay(&cert, 0).status, Status::Unknown);
        let result = proof::replay(&cert, 2_000);
        if result.reason == ReplayReason::SolverMissing {
            assert_eq!(result.status, Status::NotRun);
            eprintln!("NOT_RUN Z3 missing");
            continue;
        }
        assert_eq!(result.reason, ReplayReason::Unsat, "{result:?}");
        assert_eq!(result.status, Status::Pass);
    }
    let mut cert = proof::certify(&fusion).unwrap();
    cert.law = Law::FlipInvolution;
    assert_eq!(proof::replay(&cert, 2_000).status, Status::Rejected);
    cert.law = Law::SliceFusion;
    cert.rewrite.candidate = vec![Op::Slice {
        axis: 1,
        start: 1,
        count: 2,
        step: -1,
    }];
    assert!(proof::certify(&cert.rewrite).is_none());
    let result = proof::replay(&cert, 2_000);
    assert!(matches!(
        result.reason,
        ReplayReason::Sat | ReplayReason::SolverMissing
    ));
}

#[test]
fn light_snapshot_preserves_bits_and_rank_boundary() {
    let values = [f32::from_bits(0x7fc0_0001), -0.0_f32, f32::INFINITY];
    let (input, storage) = adapters::light_snapshot([2], [-2], 2, StorageId(7), &values).unwrap();
    assert_eq!(input.permission, Permission::ReadOnly);
    assert_eq!(storage, [0x7fc0_0001, 0x8000_0000, 0x7f80_0000]);
    assert_eq!(
        oracle::interpret(&input, &storage, &[], 100).unwrap().cells[0].value,
        0x7f80_0000
    );
    assert!(adapters::light_snapshot([1; 8], [0; 8], 0, StorageId(7), &[1.0]).is_ok());
    assert_eq!(
        adapters::light_snapshot([1; 9], [0; 9], 0, StorageId(7), &[1.0]),
        Err(Fault::RankMismatch)
    );
}

#[test]
fn fixed_solver_replays_composition_and_minimized_sat_witness() {
    let input = layout(&[2, 3], &[3, 1], 0, 6);
    let data = [0, 1, 2, 3, 4, 5];
    let good = cases::get("C08-composed", 0).unwrap();
    let result = proof::replay_fixed(&input, &data, &good.rewrite, cases::BUDGET, 2_000).unwrap();
    assert_eq!(result.addresses, AddressDecision::Equal);
    assert!(matches!(
        result.solver.reason,
        ReplayReason::Unsat | ReplayReason::SolverMissing
    ));
    let bad = rules(vec![Op::Flip(1)], vec![]);
    let result = proof::replay_fixed(&input, &data, &bad, cases::BUDGET, 2_000).unwrap();
    assert_eq!(result.addresses, AddressDecision::Witness(vec![0, 0]));
    assert!(matches!(
        result.solver.reason,
        ReplayReason::Sat | ReplayReason::SolverMissing
    ));

    // Equal origins and a differing stride force a nonzero minimal index.
    let input = layout(&[4], &[1], 0, 4);
    let bad = rules(
        vec![Op::Slice {
            axis: 0,
            start: 0,
            count: 2,
            step: 2,
        }],
        vec![Op::Slice {
            axis: 0,
            start: 0,
            count: 2,
            step: 3,
        }],
    );
    let result = proof::replay_fixed(&input, &data[..4], &bad, cases::BUDGET, 2_000).unwrap();
    assert_eq!(result.addresses, AddressDecision::Witness(vec![1]));
    assert!(matches!(
        result.solver.reason,
        ReplayReason::Sat | ReplayReason::SolverMissing
    ));
}

#[test]
fn reshape_compares_radix_boundaries_without_enumerating_in_lowerer() {
    for first in -3_i64..=3 {
        for second in -3_i64..=3 {
            let mut input = layout(&[2, 3], &[first, second], 9, 19);
            input.permission = Permission::ReadOnly;
            let backing: Vec<_> = (0..19).collect();
            for new in [vec![6], vec![3, 2], vec![2, 1, 3], vec![1, 2, 3, 1]] {
                let op = Op::Reshape(new.clone());
                let expected = oracle::interpret(&input, &backing, std::slice::from_ref(&op), 100);
                let actual = lower::compile(&input, std::slice::from_ref(&op), 0);
                match (expected, actual) {
                    (Ok(observed), Ok(plan)) => {
                        assert_eq!(
                            check(&input, &backing, &rules(vec![op.clone()], vec![op])).status,
                            Status::Pass
                        );
                        assert_eq!(observed.shape, plan.output().shape);
                    }
                    (Err(Fault::RequiresCopy), Err(Fault::RequiresCopy)) => {
                        assert!(new != [2, 1, 3] && new != [1, 2, 3, 1]);
                    }
                    (observed, actual) => panic!("oracle={observed:?}, lower={:?}", actual.err()),
                }
            }
        }
    }
    let input = layout(&[2, 3, 4], &[20, 4, 1], 0, 32);
    let plan = lower::compile(&input, &[Op::Reshape(vec![2, 12])], 0).unwrap();
    assert_eq!(plan.output().strides, [20, 1]);
}

#[test]
fn permute_and_broadcast_mutations_have_independent_witnesses() {
    let input = layout(&[2, 2, 2], &[4, 2, 1], 0, 8);
    let data = [0, 1, 2, 3, 4, 5, 6, 7];
    let good = rules(
        vec![Op::Permute(vec![2, 0, 1]), Op::Permute(vec![1, 2, 0])],
        vec![],
    );
    assert_eq!(check(&input, &data, &good).status, Status::Pass);
    let bad = rules(good.reference, vec![Op::Permute(vec![2, 1, 0])]);
    assert_eq!(
        check(&input, &data, &bad).reason,
        Reason::Value(vec![0, 0, 1])
    );
    let input = layout(&[1, 2], &[0, 1], 0, 2);
    let bad = rules(
        vec![Op::Broadcast(vec![2, 2])],
        vec![
            Op::Broadcast(vec![2, 2]),
            Op::Transpose {
                first: 0,
                second: 1,
            },
        ],
    );
    assert_eq!(
        check(&input, &[1, 2], &bad).reason,
        Reason::Value(vec![0, 1])
    );
    assert_eq!(
        check(&input, &[1], &rules(vec![], vec![])).reason,
        Reason::Input(Fault::StorageExtent)
    );
}
