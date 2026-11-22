use momiru_layoutproof::{
    adapters::a02_momiru::{check_read_only, Engine},
    ir::{Fault, Layout, Op, Permission, Rewrite, Status, StorageId},
    lower, oracle,
};
use std::process::Command;

#[test]
fn native_cli_distinguishes_variant_matches_from_rewrite_admission() {
    for (case, expected_status, expected_code) in [
        ("C08-address-witness", "FAIL", 1),
        ("C08-composed", "PASS", 0),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_a02-layoutproof"))
            .args(["--mode", "engine-check", "--engine", "both", "--case", case])
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(expected_code), "{stdout}");
        assert_eq!(
            stdout.matches("PASS MatchesInterpreter").count(),
            4,
            "{stdout}"
        );
        assert_eq!(
            stdout
                .matches(&format!("{expected_status} NativeRewrite"))
                .count(),
            2,
            "{stdout}"
        );
    }
}

#[test]
fn disjoint_peer_still_has_to_prove_its_exclusive_layout_is_injective() {
    let input = Layout {
        shape: vec![1],
        strides: vec![0],
        origin: 1,
        capacity: 2,
        storage: StorageId(1),
        permission: Permission::Exclusive,
    };
    let peer = Layout {
        shape: vec![2],
        origin: 0,
        ..input.clone()
    };
    let ops = [Op::RequireWritable];
    let rewrite = Rewrite {
        reference: vec![],
        candidate: ops.to_vec(),
    };
    assert_eq!(
        lower::compile_with_peers(&input, &ops, std::slice::from_ref(&peer), 100).err(),
        Some(Fault::WriteOverlap)
    );
    assert_eq!(
        oracle::interpret_with_peers(&input, &[1, 2], &ops, std::slice::from_ref(&peer), 100).err(),
        Some(Fault::WriteOverlap)
    );
    let decision = momiru_layoutproof::verify::check_with_peers(
        &input,
        &[1, 2],
        &rewrite,
        &[peer],
        100,
        momiru_layoutproof::verify::Mode::Full,
    );
    assert_eq!(decision.status, Status::Rejected);
}

#[test]
fn light_accepts_a_large_unsigned_start_for_an_empty_slice() {
    let input = Layout {
        shape: vec![usize::MAX, 0],
        strides: vec![0, 0],
        origin: 0,
        capacity: 0,
        storage: StorageId(1),
        permission: Permission::ReadOnly,
    };
    for step in [1, -1] {
        let ops = [Op::Slice {
            axis: 0,
            start: usize::MAX,
            count: 0,
            step,
        }];
        let general = check_read_only(Engine::General, &input, &[], &ops, 100).unwrap();
        let light = check_read_only(Engine::Light, &input, &[], &ops, 100).unwrap();
        assert_eq!(general, light);
        assert_eq!(light.shape, [0, 0]);
        assert!(light.offsets.is_empty() && light.values.is_empty());
    }
}

#[test]
fn invalid_ir_slice_identifies_oracle_preflight() {
    use momiru_layoutproof::adapters::a02_momiru::EngineError;

    let input = Layout {
        shape: vec![2],
        strides: vec![1],
        origin: 0,
        capacity: 2,
        storage: StorageId(1),
        permission: Permission::ReadOnly,
    };
    let invalid = [Op::Slice {
        axis: 0,
        start: 0,
        count: 3,
        step: 1,
    }];
    for engine in [Engine::General, Engine::Light] {
        let error = check_read_only(engine, &input, &[1, 2], &invalid, 100).unwrap_err();
        assert!(matches!(
            error,
            EngineError::OraclePreflight(Fault::InvalidSlice)
        ));
        assert_eq!(error.status(), Status::Rejected);
    }
}

#[test]
fn general_rejects_invalid_slicing_origins_and_shape_overflow_natively() {
    use momiru::tensor::{TensorError, TensorView};
    use std::num::NonZeroIsize;

    let data = [3.0_f32, 7.0];
    let valid = TensorView::new([2], [1], 0, &data).unwrap();
    let one = NonZeroIsize::new(1).unwrap();
    assert_eq!(
        valid.slice_axis(0, 0, 3, one).err(),
        Some(TensorError::IndexOutOfBounds)
    );
    assert_eq!(
        valid.slice_axis(1, 0, 1, one).err(),
        Some(TensorError::AxisOutOfBounds)
    );
    for origin in [-1, 2] {
        assert_eq!(
            TensorView::new([2], [1], origin, &data).err(),
            Some(TensorError::IndexOutOfBounds)
        );
    }
    assert_eq!(
        TensorView::new([usize::MAX, 2], [0, 0], 0, &data).err(),
        Some(TensorError::ShapeOverflow)
    );
}

#[test]
fn light_rejects_invalid_slicing_origins_and_shape_overflow_natively() {
    use momiru::tensor_light::{SignedSliceSpec, SignedTensorView, TensorError};

    let data = [3.0_f32, 7.0];
    let valid = SignedTensorView::new([2], [1], 0, &data).unwrap();
    assert_eq!(
        valid
            .slice_axis(0, SignedSliceSpec::new(Some(0), Some(1), 0))
            .err(),
        Some(TensorError::InvalidSliceStep)
    );
    assert_eq!(
        valid.slice_axis(1, SignedSliceSpec::full()).err(),
        Some(TensorError::AxisOutOfBounds)
    );
    // Light clamps half-open endpoints. The checked-count IR does not.
    let clamped = valid
        .slice_axis(0, SignedSliceSpec::new(Some(0), Some(3), 1))
        .unwrap();
    assert_eq!(clamped.dims(), &[2]);
    assert_eq!(clamped.get([2]), Err(TensorError::IndexOutOfBounds));
    for origin in [-1, 2] {
        assert_eq!(
            SignedTensorView::new([2], [1], origin, &data).err(),
            Some(TensorError::IndexOutOfBounds)
        );
    }
    assert_eq!(
        SignedTensorView::new([usize::MAX, 2], [0, 0], 0, &data).err(),
        Some(TensorError::ShapeOverflow)
    );
}
