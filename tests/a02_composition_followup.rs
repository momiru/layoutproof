use momiru_layoutproof::{
    adapters::a02_momiru::{check_read_only, Engine},
    ir::{Fault, Layout, Op, Permission, Rewrite, Status, StorageId},
    proof,
    verify::{self, Mode},
};

fn layout() -> Layout {
    Layout {
        shape: vec![2],
        strides: vec![1],
        origin: 0,
        capacity: 2,
        storage: StorageId(1),
        permission: Permission::ReadOnly,
    }
}

#[test]
fn full_equivalence_preserves_allocation_reservations_and_extents() {
    let copy = Op::Materialize(StorageId(3));
    let reused = Op::Materialize(StorageId(2));
    let slice = Op::Slice {
        axis: 0,
        start: 0,
        count: 1,
        step: 1,
    };
    let reservations = Rewrite {
        reference: vec![copy.clone()],
        candidate: vec![reused.clone(), copy.clone()],
    };
    let mut suffix = reservations.clone();
    suffix.reference.push(reused.clone());
    suffix.candidate.push(reused);
    let check = |rule| verify::check(&layout(), &[1, 2], rule, 100, Mode::Full);
    assert_eq!(
        check(&suffix).reason,
        verify::Reason::CandidatePrecondition(Fault::StorageReuse)
    );
    assert_eq!(check(&reservations).reason, verify::Reason::AllocationState);
    let extents = Rewrite {
        reference: vec![slice.clone(), copy.clone()],
        candidate: vec![copy, slice],
    };
    assert_eq!(check(&extents).reason, verify::Reason::AllocationState);
}

#[test]
fn symbolic_discovery_and_replay_respect_the_layout_vector_rank_bound() {
    for axis in [proof::MAX_LAYOUT_RANK, usize::MAX - 1] {
        let rule = Rewrite {
            reference: vec![Op::Flip(axis), Op::Flip(axis)],
            candidate: vec![],
        };
        assert!(proof::certify(&rule).is_none());
        let cert = proof::Certificate {
            law: proof::Law::FlipInvolution,
            rewrite: rule,
        };
        assert!(proof::replay_query(&cert).is_none());
    }
    let axis = proof::MAX_LAYOUT_RANK - 1;
    let last = Rewrite {
        reference: vec![Op::Flip(axis), Op::Flip(axis)],
        candidate: vec![],
    };
    let query = proof::replay_query(&proof::certify(&last).unwrap()).unwrap();
    assert!(query.contains(&format!("(<= r {})", proof::MAX_LAYOUT_RANK)));
}

#[test]
fn light_uses_native_end_relative_coordinates_for_a_wide_empty_view() {
    let data = Layout {
        shape: vec![usize::MAX, 0],
        strides: vec![0, 0],
        capacity: 0,
        ..layout()
    };
    for step in [-1, 1] {
        let operation = Op::Slice {
            axis: 0,
            start: usize::MAX - 1,
            count: 1,
            step,
        };
        let general = check_read_only(
            Engine::General,
            &data,
            &[],
            std::slice::from_ref(&operation),
            100,
        )
        .unwrap();
        let light = check_read_only(Engine::Light, &data, &[], &[operation], 100).unwrap();
        assert_eq!(general, light);
        assert_eq!(light.shape, [1, 0]);
    }
}

#[test]
fn overlap_ablation_ignores_cross_view_conflicts_only() {
    let mut input = layout();
    input.permission = Permission::Exclusive;
    input.strides = vec![2];
    input.capacity = 4;
    let mut peer = Layout {
        origin: 2,
        strides: vec![-2],
        permission: Permission::ReadOnly,
        ..input.clone()
    };
    let mut rule = Rewrite {
        reference: vec![],
        candidate: vec![Op::RequireWritable],
    };
    let check = |input: &Layout, peer: &Layout, rule: &Rewrite, mode| {
        verify::check_with_peers(
            input,
            &[1, 2, 3, 4],
            rule,
            std::slice::from_ref(peer),
            100,
            mode,
        )
    };
    for mode in [Mode::Full, Mode::NoAliasPrecondition] {
        let verdict = check(&input, &peer, &rule, mode);
        assert_eq!(
            verdict.reason,
            verify::Reason::Input(Fault::LivePeerOverlap)
        );
    }
    let control = Mode::NoLivePeerCheck;
    let verdict = check(&input, &peer, &rule, control);
    assert_eq!(verdict.status, Status::Pass);
    assert!(!verdict.eligible(control));

    input.permission = Permission::ReadOnly;
    assert_eq!(
        check(&input, &peer, &rule, control).reason,
        verify::Reason::CandidatePrecondition(Fault::WriteDenied)
    );
    rule.candidate = vec![Op::Materialize(peer.storage)];
    assert_eq!(
        check(&input, &peer, &rule, control).reason,
        verify::Reason::CandidatePrecondition(Fault::StorageReuse)
    );
    peer.permission = Permission::Exclusive;
    peer.strides = vec![0];
    assert_eq!(
        check(&input, &peer, &rule, control).reason,
        verify::Reason::Input(Fault::WriteOverlap)
    );
}
