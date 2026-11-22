use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_a02-layoutproof"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn benchmark_rejects_mutant_without_emitting_timings() {
    let output = run(&["--mode", "bench", "--case", "C08-materialize-mutant"]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("FAIL Address"));
    assert!(stdout.contains("NOT_RUN CORRECTNESS_GATE"));
    assert!(!stdout.contains(" ns"));
}

#[test]
fn evidence_refuses_admission_without_creating_a_directory() {
    let path =
        std::path::Path::new("target").join(format!("a02-test-evidence-{}", std::process::id()));
    assert!(!path.exists());
    let output = run(&[
        "--mode",
        "evidence",
        "--engine",
        "both",
        "--output-root",
        path.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "NOT_RUN CANONICAL_PROTOCOLS_MISSING"
    );
    assert!(!path.exists());
}

#[test]
fn engine_selection_is_honest_and_defaults_to_cpu_model() {
    let output = run(&["--engine", "light"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("UNSUPPORTED ENGINE_LINKAGE"));
    let output = run(&["--paper", "A02", "--seed", "7"]);
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("PASS Checked"));
}

#[test]
fn benchmark_positive_path_runs_requested_warmup_and_samples() {
    let result = run(&["--mode", "bench", "--warmup", "1", "--samples", "2"]);
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("Development timing only"));
    assert!(stdout.contains("CPU allocation-inclusive lowering"));
    assert_eq!(
        stdout.lines().filter(|l| l.starts_with("warmup ")).count(),
        1
    );
    assert_eq!(
        stdout.lines().filter(|l| l.starts_with("sample ")).count(),
        2
    );
}
