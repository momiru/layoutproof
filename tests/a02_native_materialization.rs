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
// Module:       A02 test: a02_native_materialization
// Source:       tests/a02_native_materialization.rs
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
    adapters::a02_momiru::{check_materialization, Engine, EngineError},
    ir::{Fault, Layout, Op, Permission, StorageId},
};

fn input(shape: Vec<usize>, strides: Vec<i64>, origin: i64, capacity: usize) -> Layout {
    Layout {
        shape,
        strides,
        origin,
        capacity,
        storage: StorageId(1),
        permission: Permission::ReadOnly,
    }
}

#[test]
fn both_engines_materialize_signed_broadcast_views_into_fresh_dense_storage() {
    let bits = [
        0x7fc0_0001,
        0x8000_0000,
        1,
        0x7fc0_0002,
        0x7f80_0000,
        0xff80_0000,
        42,
    ];
    let backing = bits.map(f32::from_bits);
    let layout = input(vec![2, 3], vec![4, -1], 2, backing.len());
    let ops = [
        Op::Flip(0),
        Op::Permute(vec![1, 0]),
        Op::Broadcast(vec![2, 3, 2]),
    ];
    // After flipping and swapping axes, each broadcast plane visits
    // backing positions 6, 2, 5, 1, 4, 0 in row-major order.
    let plane = [6, 2, 5, 1, 4, 0].map(|offset| bits[offset]);
    let expected = [plane, plane].concat();
    for engine in [Engine::General, Engine::Light] {
        let observed =
            check_materialization(engine, &layout, &backing, &ops, StorageId(2), 128).unwrap();
        assert_eq!(observed.shape, [2, 3, 2]);
        assert_eq!(observed.offsets, (0..12).collect::<Vec<_>>());
        assert_eq!(observed.values, expected);
        assert_eq!(backing.map(f32::to_bits), bits);
        assert!(matches!(
            check_materialization(engine, &layout, &backing, &ops, layout.storage, 128),
            Err(EngineError::OraclePreflight(Fault::StorageReuse))
        ));
    }
}

#[test]
fn native_materialization_respects_empty_scalar_and_engine_rank_boundaries() {
    for (shape, backing) in [(vec![2, 0], vec![]), (vec![], vec![-0.0_f32])] {
        let layout = input(shape.clone(), vec![0; shape.len()], 0, backing.len());
        let general =
            check_materialization(Engine::General, &layout, &backing, &[], StorageId(2), 64)
                .unwrap();
        let light =
            check_materialization(Engine::Light, &layout, &backing, &[], StorageId(2), 64).unwrap();
        assert_eq!(general, light);
        assert_eq!(general.shape, shape);
        assert_eq!(
            general.values,
            backing
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
    }

    let wide = input(vec![1; 9], vec![0; 9], 0, 1);
    assert_eq!(
        check_materialization(Engine::General, &wide, &[3.0], &[], StorageId(2), 64)
            .unwrap()
            .values,
        [3.0_f32.to_bits()]
    );
    assert!(matches!(
        check_materialization(Engine::Light, &wide, &[3.0], &[], StorageId(2), 64),
        Err(EngineError::UnsupportedRank(9))
    ));
}
