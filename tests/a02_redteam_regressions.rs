use momiru_layoutproof::{
    ir::{Fault, Layout, Op, Permission, Rewrite, Status, StorageId},
    proof, verify,
};

fn layout(shape: &[usize], strides: &[i64], origin: i64, capacity: usize) -> Layout {
    Layout {
        shape: shape.into(),
        strides: strides.into(),
        origin,
        capacity,
        storage: StorageId(1),
        permission: Permission::ReadOnly,
    }
}

fn rule(reference: Vec<Op>, candidate: Vec<Op>) -> Rewrite {
    Rewrite {
        reference,
        candidate,
    }
}

#[test]
fn whole_layout_bounds_precede_alias_rejection() {
    let mut input = layout(&[2, 2], &[10, 0], 0, 1);
    input.permission = Permission::Exclusive;
    let verdict = verify::check(&input, &[0], &rule(vec![], vec![]), 100, verify::Mode::Full);
    assert_eq!(verdict.status, Status::Rejected);
    assert_eq!(verdict.reason, verify::Reason::Input(Fault::Bounds));
}

#[test]
fn historical_definedness_witnesses() {
    let take = |count| Op::Slice {
        axis: 0,
        start: 0,
        count,
        step: 1,
    };
    let input = layout(&[2], &[1], 0, 2);
    for (rewrite, fault) in [
        (
            rule(vec![take(3), take(1)], vec![take(1)]),
            Fault::InvalidSlice,
        ),
        (
            rule(vec![Op::Flip(1), Op::Flip(1)], vec![]),
            Fault::AxisOutOfRange,
        ),
    ] {
        let reference = momiru_layoutproof::lower::compile(&input, &rewrite.reference, 100);
        assert_eq!(reference.err(), Some(fault));
        assert!(momiru_layoutproof::lower::compile(&input, &rewrite.candidate, 100).is_ok());
        assert!(proof::certify(&rewrite).is_some());
    }
}

#[test]
fn scoped_solver_replay_requires_reference_definedness() {
    let take = |count| Op::Slice {
        axis: 0,
        start: 0,
        count,
        step: 1,
    };
    let too_short = layout(&[2], &[1], 0, 2);
    let cases = [
        (
            rule(vec![take(3), take(1)], vec![take(1)]),
            Fault::InvalidSlice,
            layout(&[3], &[1], 0, 3),
        ),
        (
            rule(vec![Op::Flip(1), Op::Flip(1)], vec![]),
            Fault::AxisOutOfRange,
            layout(&[2, 0], &[0, 0], 0, 0),
        ),
    ];
    for (rewrite, fault, inside_guard) in cases {
        let cert = proof::certify(&rewrite).unwrap();
        let result = proof::replay_on_layout(&cert, &too_short, &[], 100, 2_000);
        assert_eq!(result.status, Status::Rejected);
        assert_eq!(
            result.reason,
            proof::ReplayReason::ReferencePrecondition(fault)
        );
        let result = proof::replay_on_layout(&cert, &inside_guard, &[], 100, 2_000);
        assert!(matches!(
            result.reason,
            proof::ReplayReason::Unsat | proof::ReplayReason::SolverMissing
        ));
        let query = proof::replay_query(&cert).unwrap();
        assert!(query.contains("(=> reference-defined (and candidate-defined"));
    }
}

#[test]
fn peer_leases_check_exact_aliases_and_reserve_storage_identity() {
    let data = [1, 2, 3, 4];
    let mut input = layout(&[2], &[2], 0, 4);
    input.permission = Permission::Exclusive;
    let mut peer = layout(&[2], &[-2], 2, 4);
    let writes = rule(vec![], vec![Op::RequireWritable]);
    let check = |input: &Layout, peer: &Layout, rewrite: &Rewrite, budget| {
        verify::check_with_peers(
            input,
            &data,
            rewrite,
            std::slice::from_ref(peer),
            budget,
            verify::Mode::Full,
        )
    };
    assert_eq!(
        check(&input, &peer, &writes, 100).reason,
        verify::Reason::Input(Fault::LivePeerOverlap)
    );
    assert_eq!(check(&input, &peer, &writes, 2).status, Status::Unknown);
    peer.origin = 3;
    assert_eq!(check(&input, &peer, &writes, 100).status, Status::Pass);
    peer.storage = StorageId(7);
    let copy = rule(vec![], vec![Op::Materialize(peer.storage)]);
    assert_eq!(
        check(&input, &peer, &copy, 100).reason,
        verify::Reason::CandidatePrecondition(Fault::StorageReuse)
    );
    let mut exclusive_peer = layout(&[2], &[-2], 2, 4);
    exclusive_peer.permission = Permission::Exclusive;
    input.permission = Permission::ReadOnly;
    assert_eq!(
        check(&input, &exclusive_peer, &writes, 100).reason,
        verify::Reason::Input(Fault::LivePeerOverlap)
    );
}

#[test]
fn physical_alias_equivalence_cannot_authorize_writes_or_change_lineage() {
    let input = layout(&[2], &[0], 0, 1);
    let flipped = rule(vec![], vec![Op::Flip(0)]);
    assert_eq!(
        proof::affine_addresses(&input, &flipped, 100),
        Ok(proof::AddressDecision::Equal)
    );
    assert_eq!(
        verify::check(&input, &[7], &flipped, 100, verify::Mode::Full).reason,
        verify::Reason::Logical(vec![0])
    );
    let denied = rule(vec![], vec![Op::RequireWritable]);
    let full = verify::check(&input, &[7], &denied, 100, verify::Mode::Full);
    let replay = proof::replay_fixed(&input, &[7], &denied, 100, 2_000).unwrap();
    assert_eq!(
        full.reason,
        verify::Reason::CandidatePrecondition(Fault::WriteDenied)
    );
    assert_eq!(full.status, Status::Fail);
    assert_eq!(replay.solver.status, Status::Fail);
    assert_eq!(
        replay.solver.reason,
        proof::ReplayReason::CandidatePrecondition(Fault::WriteDenied)
    );
}

#[test]
fn z3_reproduces_rank_and_slice_definedness_counterexamples() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let take = |count| Op::Slice {
        axis: 0,
        start: 0,
        count,
        step: 1,
    };
    for (rewrite, witness) in [
        (
            rule(vec![take(3), take(1)], vec![take(1)]),
            "(and (= r 1) (= n 2))",
        ),
        (
            rule(vec![Op::Flip(1), Op::Flip(1)], vec![]),
            "(and (= r 1) (= n 2))",
        ),
    ] {
        let cert = proof::certify(&rewrite).unwrap();
        let emitted = proof::replay_query(&cert).unwrap();
        let (definitions, _) = emitted.split_once("(assert (not (and").unwrap();
        let query = format!("{definitions}(assert {witness})\n(assert (and candidate-defined (not reference-defined)))\n(check-sat)\n");
        let child = Command::new("z3")
            .args(["-in", "-smt2", "-T:3"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("NOT_RUN Z3 missing");
                return;
            }
            Err(error) => panic!("{error}"),
        };
        child
            .stdin
            .take()
            .unwrap()
            .write_all(query.as_bytes())
            .unwrap();
        let observed = child.wait_with_output().unwrap();
        assert!(observed.status.success(), "{observed:?}");
        assert_eq!(String::from_utf8(observed.stdout).unwrap().trim(), "sat");
    }
}
