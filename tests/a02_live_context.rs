use momiru_layoutproof::{
    ir::{Fault, Layout, Permission, Rewrite, Status, StorageId},
    lower, oracle, proof,
    verify::{self, Mode, Reason},
};

fn layout(
    storage: u64,
    shape: usize,
    stride: i64,
    origin: i64,
    capacity: usize,
    permission: Permission,
) -> Layout {
    Layout {
        shape: vec![shape],
        strides: vec![stride],
        origin,
        capacity,
        storage: StorageId(storage),
        permission,
    }
}

#[test]
fn conflicting_peers_are_rejected_even_when_disjoint_from_the_subject() {
    let subject = layout(1, 2, 1, 0, 6, Permission::ReadOnly);
    let identity = Rewrite {
        reference: vec![],
        candidate: vec![],
    };
    for id in [1, 2] {
        let peers = [
            layout(id, 2, 1, 2, 6, Permission::Exclusive),
            layout(id, 2, -1, 3, 6, Permission::ReadOnly),
        ];
        let backing = [0; 6];
        assert_eq!(
            lower::compile_with_peers(&subject, &[], &peers, 100).err(),
            Some(Fault::LivePeerOverlap)
        );
        assert_eq!(
            oracle::interpret_with_peers(&subject, &backing, &[], &peers, 100).err(),
            Some(Fault::LivePeerOverlap)
        );
        assert_eq!(
            verify::check_with_peers(&subject, &backing, &identity, &peers, 100, Mode::Full).reason,
            Reason::Input(Fault::LivePeerOverlap)
        );
        assert_eq!(
            proof::replay_fixed_with_peers(&subject, &backing, &identity, &peers, 100, 0),
            Err(Fault::LivePeerOverlap)
        );
        let control = verify::check_with_peers(
            &subject,
            &backing,
            &identity,
            &peers,
            100,
            Mode::NoLivePeerCheck,
        );
        assert_eq!(control.status, Status::Pass);
        assert!(!control.eligible(Mode::NoLivePeerCheck));
    }
}

#[test]
fn external_leases_allow_disjoint_writers_and_shared_readers() {
    let subject = layout(1, 2, 1, 0, 6, Permission::ReadOnly);
    let mut peers = [
        layout(2, 2, 2, 0, 4, Permission::Exclusive),
        layout(2, 2, -2, 3, 4, Permission::Exclusive),
    ];
    let identity = Rewrite {
        reference: vec![],
        candidate: vec![],
    };
    let check = |peers: &[Layout]| {
        verify::check_with_peers(&subject, &[0; 6], &identity, peers, 100, Mode::Full)
    };
    // The address ranges intersect, but the even and odd addresses do not.
    assert!(check(&peers).eligible(Mode::Full));
    peers[0].permission = Permission::ReadOnly;
    peers[1].permission = Permission::ReadOnly;
    peers[1].origin = 2;
    assert!(check(&peers).eligible(Mode::Full));
    assert_eq!(
        verify::check_with_peers(&subject, &[0; 6], &identity, &peers, 5, Mode::Full).status,
        Status::Unknown
    );
}

#[test]
fn capacity_agreement_is_required_even_in_the_overlap_control() {
    let subject = layout(1, 2, 1, 0, 6, Permission::ReadOnly);
    let peers = [
        layout(2, 2, 1, 0, 4, Permission::ReadOnly),
        layout(2, 2, 1, 0, 5, Permission::ReadOnly),
    ];
    let identity = Rewrite {
        reference: vec![],
        candidate: vec![],
    };
    for mode in [Mode::Full, Mode::NoLivePeerCheck] {
        let result = verify::check_with_peers(&subject, &[0; 6], &identity, &peers, 100, mode);
        assert_eq!(result.reason, Reason::Input(Fault::StorageExtent));
        assert_eq!(result.status, Status::Rejected);
    }
}
