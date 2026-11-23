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
// Module:       A02 test: a02_momiru_link
// Source:       tests/a02_momiru_link.rs
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
    adapters::a02_momiru::{self, Engine, EngineError},
    cases::{self, BUDGET},
    ir::{Layout, Op, Permission, StorageId},
    oracle,
};

fn input(shape: &[usize], strides: &[i64], origin: i64, capacity: usize) -> Layout {
    Layout {
        shape: shape.into(),
        strides: strides.into(),
        origin,
        capacity,
        storage: StorageId(11),
        permission: Permission::ReadOnly,
    }
}

#[test]
fn composed_rewrite_matches_independent_oracle_in_both_engines() {
    let case = cases::get("C08-composed", 7).unwrap();
    for engine in [Engine::General, Engine::Light] {
        let observe = |ops| {
            a02_momiru::check_read_only(engine, &case.layout, &case.storage, ops, BUDGET).unwrap()
        };
        assert_eq!(
            observe(&case.rewrite.reference),
            observe(&case.rewrite.candidate)
        );
        let mut mutant = case.rewrite.candidate.clone();
        mutant.pop();
        mutant.push(Op::Slice {
            axis: 0,
            start: 0,
            count: 6,
            step: 1,
        });
        assert_ne!(observe(&case.rewrite.reference), observe(&mutant));
    }
}

#[test]
fn native_signed_slice_reaches_zero_and_broadcasts_exact_fp32_payloads() {
    let input = input(&[2, 3], &[6, -2], 4, 11);
    let mut data: Vec<_> = (0..11).collect();
    data[0] = 0x7fc0_0001;
    data[4] = 0x8000_0000;
    data[6] = 0xff80_0000;
    data[10] = 0x7fc0_0002;
    let ops = [
        Op::Slice {
            axis: 1,
            start: 2,
            count: 2,
            step: -2,
        },
        Op::Broadcast(vec![3, 2, 2]),
        Op::Permute(vec![2, 0, 1]),
        Op::Flip(0),
    ];
    let general =
        a02_momiru::check_read_only(Engine::General, &input, &data, &ops, BUDGET).unwrap();
    let light = a02_momiru::check_read_only(Engine::Light, &input, &data, &ops, BUDGET).unwrap();
    assert_eq!(general, light);
}

#[test]
fn native_rank_limits_and_empty_domains() {
    for rank in [0, 1, 2, 4, 8, 9, 16] {
        let scalar = input(&vec![1; rank], &vec![0; rank], 0, 1);
        let ops = if rank == 0 {
            vec![]
        } else {
            vec![Op::Flip(rank - 1)]
        };
        let general =
            a02_momiru::check_read_only(Engine::General, &scalar, &[0x8000_0000], &ops, BUDGET)
                .unwrap();
        let light =
            a02_momiru::check_read_only(Engine::Light, &scalar, &[0x8000_0000], &ops, BUDGET);
        if rank <= 8 {
            assert_eq!(general, light.unwrap());
        } else {
            assert!(matches!(light, Err(EngineError::UnsupportedRank(r)) if r == rank));
        }
    }
    let empty = input(&[2, 0], &[5, -1], 1, 1);
    for engine in [Engine::General, Engine::Light] {
        let observation =
            a02_momiru::check_read_only(engine, &empty, &[3], &[Op::Flip(1)], BUDGET).unwrap();
        assert!(observation.offsets.is_empty());
    }
}

#[test]
fn native_mutable_admission_and_write_poststate() {
    let mut view = input(&[2, 2], &[4, -2], 2, 7);
    view.permission = Permission::Exclusive;
    let initialized: Vec<u32> = (0..7).collect();
    let model = oracle::interpret(&view, &initialized, &[], BUDGET).unwrap();
    let new_bits = 0x7fc0_0001;
    let mut expected = initialized.clone();
    expected[model.cells[3].address.offset] = new_bits;
    for engine in [Engine::General, Engine::Light] {
        let mut storage: Vec<f32> = initialized.iter().copied().map(f32::from_bits).collect();
        a02_momiru::write_once(engine, &view, &mut storage, &[1, 1], new_bits).unwrap();
        assert_eq!(
            storage.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected
        );
        let overlap = Layout {
            strides: vec![0, -2],
            ..view.clone()
        };
        let rejected = a02_momiru::write_once(engine, &overlap, &mut storage, &[1, 1], 0);
        assert!(matches!(
            rejected,
            Err(EngineError::General(
                momiru::tensor::TensorError::OverlappingMutableView
            )) | Err(EngineError::Light(
                momiru::tensor_light::TensorError::OverlappingMutableView
            ))
        ));
        assert_eq!(
            storage.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn compatible_affine_model_does_not_imply_native_reshape_support() {
    let input = input(&[2, 3], &[-3, -1], 5, 6);
    let data = [0, 1, 2, 3, 4, 5];
    let ops = [Op::Reshape(vec![6])];
    assert!(oracle::interpret(&input, &data, &ops, BUDGET).is_ok());
    for engine in [Engine::General, Engine::Light] {
        let unsupported =
            a02_momiru::check_read_only(engine, &input, &data, &ops, BUDGET).unwrap_err();
        assert_eq!(
            unsupported.status(),
            momiru_layoutproof::ir::Status::Unsupported
        );
    }
}
