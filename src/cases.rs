//! Development cases. Their IDs and seed have local test meaning only.

use crate::ir::{Layout, Op, Permission, Rewrite, Status, StorageId, ValueBits};

pub const IDS: &[&str] = &[
    "C08-composed",
    "C08-flip-involution",
    "C08-address-witness",
    "C08-alias-mutant",
    "C08-materialize-mutant",
    "C09-slice-fusion",
];
pub const BUDGET: usize = 65_536;

pub struct Case {
    pub layout: Layout,
    pub storage: Vec<ValueBits>,
    pub rewrite: Rewrite,
    pub expected: Status,
}

pub fn get(id: &str, seed: u64) -> Option<Case> {
    use Op::*;
    let mut layout = Layout {
        shape: vec![2, 3],
        strides: vec![3, 1],
        origin: 0,
        storage: StorageId(1),
        capacity: 6,
        permission: Permission::Exclusive,
    };
    let mut expected = Status::Pass;
    let (reference, candidate) = match id {
        "C08-composed" => (
            vec![
                Expand(0),
                Transpose {
                    first: 1,
                    second: 2,
                },
                Transpose {
                    first: 2,
                    second: 1,
                },
                Squeeze(0),
                Reshape(vec![6]),
                Flip(0),
            ],
            vec![
                Reshape(vec![6]),
                Slice {
                    axis: 0,
                    start: 5,
                    count: 6,
                    step: -1,
                },
            ],
        ),
        "C08-flip-involution" => (vec![Flip(1), Flip(1)], vec![]),
        "C08-address-witness" => {
            expected = Status::Fail;
            (vec![Flip(1), Flip(1)], vec![Flip(1)])
        }
        "C08-alias-mutant" => {
            layout.permission = Permission::ReadOnly;
            expected = Status::Fail;
            (vec![Flip(1), Flip(1)], vec![RequireWritable])
        }
        "C08-materialize-mutant" => {
            expected = Status::Fail;
            (vec![Flip(1), Flip(1)], vec![Materialize(StorageId(2))])
        }
        "C09-slice-fusion" => (
            vec![
                Slice {
                    axis: 1,
                    start: 2,
                    count: 3,
                    step: -1,
                },
                Slice {
                    axis: 1,
                    start: 0,
                    count: 2,
                    step: 2,
                },
            ],
            vec![Slice {
                axis: 1,
                start: 2,
                count: 2,
                step: -2,
            }],
        ),
        _ => return None,
    };
    let storage = (0..6)
        .map(|i| {
            // A reversible permutation on these six payloads, tagged as finite f32.
            0x3f80_0000 | ((seed.wrapping_add(i).wrapping_mul(0x9e37_79b9) as u32) & 0x007f_ffff)
        })
        .collect();
    Some(Case {
        layout,
        storage,
        rewrite: Rewrite {
            reference,
            candidate,
        },
        expected,
    })
}
