use momiru_layoutproof::{
    adapters::a02_momiru::{self, Engine},
    cases::BUDGET,
    ir::{Layout, Op, Permission, StorageId},
};
use std::process::{Command, Output};

fn benchmark(engine: &str, case: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_a02-layoutproof"))
        .args([
            "--paper",
            "A02",
            "--mode",
            "bench",
            "--engine",
            engine,
            "--case",
            case,
            "--seed",
            "7",
            "--warmup",
            "1",
            "--samples",
            "2",
        ])
        .output()
        .unwrap()
}

#[test]
fn native_diagnostics_validate_and_report_samples_for_each_selected_engine() {
    for (selection, engines) in [
        ("general", &["General"][..]),
        ("light", &["Light"][..]),
        ("both", &["General", "Light"][..]),
    ] {
        let output = benchmark(selection, "C08-composed");
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Development timing only"));
        let mut blocks = text.split("engine=").skip(1).filter(|block| {
            block
                .lines()
                .next()
                .unwrap()
                .contains(" variant=candidate case=")
                && block.lines().any(|line| line.starts_with("sample "))
        });
        for engine in engines {
            assert!(text.contains(&format!(
                "PASS NativeRewrite EquivalentObservations engine={engine}"
            )));
            let block = blocks.next().unwrap();
            let provenance = block.lines().next().unwrap();
            assert!(provenance.starts_with(&format!("{engine} variant=candidate")));
            assert!(provenance.ends_with(a02_momiru::REVISION));
            assert_eq!(
                block
                    .lines()
                    .filter(|line| line.starts_with("warmup "))
                    .count(),
                1
            );
            assert_eq!(
                block
                    .lines()
                    .filter(|line| line.starts_with("sample "))
                    .count(),
                2
            );
        }
        assert!(blocks.next().is_none());
    }
}

#[test]
fn native_variant_agreement_cannot_admit_a_mutated_rewrite_for_timing() {
    let output = benchmark("both", "C08-address-witness");
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.matches("PASS MatchesInterpreter").count(), 4);
    assert_eq!(text.matches("FAIL NativeRewrite").count(), 2);
    assert!(text.contains("NOT_RUN CORRECTNESS_GATE"));
    assert!(!text.contains("Development timing only"));
    assert!(!text.lines().any(|line| line.starts_with("sample ")));
}

#[test]
fn native_count_to_endpoint_translation_matches_the_interpreter_on_signed_views() {
    let data = [0, 0x8000_0000, 0x7fc0_0001, 0x7fc0_0002, 0xff80_0000, 1, 42];
    for (stride, origin) in [(1, 0), (-1, 6), (0, 3)] {
        let layout = Layout {
            shape: vec![data.len()],
            strides: vec![stride],
            origin,
            capacity: data.len(),
            storage: StorageId(1),
            permission: Permission::ReadOnly,
        };
        for start in 0..=data.len() {
            for step in [-8, -3, -2, -1, 1, 2, 3, 8] {
                for count in 0..=data.len() {
                    if count != 0 {
                        let last = start as i64 + (count as i64 - 1) * step;
                        if start == data.len() || !(0..data.len() as i64).contains(&last) {
                            continue;
                        }
                    }
                    let ops = [
                        Op::Slice {
                            axis: 0,
                            start,
                            count,
                            step,
                        },
                        Op::Flip(0),
                    ];
                    let check = |engine| {
                        a02_momiru::check_read_only(engine, &layout, &data, &ops, BUDGET)
                            .unwrap_or_else(|error| {
                                panic!("{engine:?} {layout:?} {ops:?}: {error:?}")
                            })
                    };
                    assert_eq!(check(Engine::General), check(Engine::Light));
                }
            }
        }
    }
}
