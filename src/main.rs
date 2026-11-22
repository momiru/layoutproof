#![forbid(unsafe_code)]

use momiru_layoutproof::{
    adapters::a02_momiru::{self, Engine},
    cases::{self, BUDGET},
    ir::{Fault, Status},
    lower, proof,
    verify::{self, Mode},
};
use std::{hint::black_box, path::PathBuf, process::ExitCode, time::Instant};

const HELP: &str = "\
a02-layoutproof [--paper A02] [--engine model|general|light|both]
    [--device cpu] [--case CASE_ID] [--seed N]
    [--mode check|finite|manual|no-address|no-alias|symbolic|query|affine|engine-check|bench|evidence]
    [--samples N] [--warmup N] [--timeout-ms N] [--output-root PATH]
    [--help | --version | --list]

bench runs a local, allocation-inclusive lowering diagnostic.
evidence checks admission for a canonical research run.";

struct Args {
    engine: String,
    device: String,
    case: String,
    mode: String,
    seed: u64,
    warmup: u32,
    samples: u32,
    timeout_ms: u32,
    output_root: Option<PathBuf>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            engine: "model".into(),
            device: "cpu".into(),
            case: cases::IDS[0].into(),
            mode: "check".into(),
            seed: 0,
            warmup: 3,
            samples: 10,
            timeout_ms: 1_000,
            output_root: None,
        }
    }
}

fn options(mut tokens: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = Args::default();
    while let Some(key) = tokens.next() {
        let value = tokens
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?;
        let invalid = || format!("invalid value for {key}: {value}");
        match key.as_str() {
            "--paper" if value == "A02" => {}
            "--engine" if ["model", "general", "light", "both"].contains(&value.as_str()) => {
                args.engine = value
            }
            "--device" => args.device = value,
            "--case" => args.case = value,
            "--mode" => args.mode = value,
            "--output-root" => args.output_root = Some(value.into()),
            "--seed" => args.seed = value.parse().map_err(|_| invalid())?,
            "--warmup" => args.warmup = value.parse().map_err(|_| invalid())?,
            "--samples" => args.samples = value.parse().map_err(|_| invalid())?,
            "--timeout-ms" => args.timeout_ms = value.parse().map_err(|_| invalid())?,
            _ => return Err(invalid()),
        }
    }
    if args.samples == 0
        || args.samples > 100_000
        || args.warmup > 100_000
        || args.timeout_ms > 60_000
    {
        return Err("sample, warmup or solver resource limit exceeded".into());
    }
    Ok(args)
}

fn run(args: Args) -> Result<bool, String> {
    if args.mode == "evidence" {
        println!("NOT_RUN CANONICAL_PROTOCOLS_MISSING");
        return Ok(false);
    }
    if args.output_root.is_some() {
        return Err("--output-root requires --mode evidence".into());
    }
    if args.device != "cpu" {
        println!("UNSUPPORTED DEVICE");
        return Ok(false);
    }
    if args.mode == "engine-check" {
        return run_engines(&args);
    }
    if args.engine != "model" {
        println!(
            "UNSUPPORTED ENGINE_LINKAGE_IN_MODE mode={} use=engine-check",
            args.mode
        );
        return Ok(false);
    }
    let case =
        cases::get(&args.case, args.seed).ok_or_else(|| format!("unknown case {}", args.case))?;
    let mode = match args.mode.as_str() {
        "check" | "bench" => Mode::Full,
        "finite" => Mode::FiniteOnly,
        "manual" => Mode::Manual,
        "no-address" => Mode::NoAddress,
        "no-alias" => Mode::NoAliasPrecondition,
        "affine" => {
            let result = match proof::replay_fixed(
                &case.layout,
                &case.storage,
                &case.rewrite,
                BUDGET,
                args.timeout_ms,
            ) {
                Ok(result) => result,
                Err(fault) => {
                    let status = if fault == Fault::WorkLimit {
                        Status::Unknown
                    } else {
                        Status::Rejected
                    };
                    println!("{} {fault:?} case={}", status.code(), args.case);
                    return Ok(false);
                }
            };
            println!(
                "{} {:?} {:?} case={}",
                result.solver.status.code(),
                result.solver.reason,
                result.addresses,
                args.case
            );
            return Ok(result.solver.status == Status::Pass);
        }
        "symbolic" | "query" => {
            let Some(cert) = proof::certify(&case.rewrite) else {
                println!("UNSUPPORTED SYMBOLIC_LAW case={}", args.case);
                return Ok(false);
            };
            if args.mode == "query" {
                let query = proof::replay_query(&cert).ok_or("unrecognized certificate")?;
                print!("{query}");
                return Ok(true);
            }
            let result = proof::replay_on_layout(&cert, &case.layout, &[], BUDGET, args.timeout_ms);
            println!(
                "{} {:?} case={}",
                result.status.code(),
                result.reason,
                args.case
            );
            return Ok(result.status == Status::Pass);
        }
        _ => return Err(format!("unknown mode {}", args.mode)),
    };
    let verdict = verify::check(&case.layout, &case.storage, &case.rewrite, BUDGET, mode);
    println!(
        "{} {:?} case={} seed={}",
        verdict.status.code(),
        verdict.reason,
        args.case,
        args.seed
    );
    if args.mode != "bench" {
        return Ok(if mode == Mode::Full {
            verdict.status == case.expected
        } else {
            verdict.status == Status::Pass
        });
    }
    if !verdict.eligible(mode) {
        println!("NOT_RUN CORRECTNESS_GATE");
        return Ok(false);
    }
    let samples = bench(&case, args.warmup, args.samples)?;
    let after = verify::check(&case.layout, &case.storage, &case.rewrite, BUDGET, mode);
    if !after.eligible(mode) {
        return Err("post-run correctness check failed".into());
    }
    println!(
        "Development timing only. Package {}. {}. {}-{}. Profile assertions: {}.",
        env!("CARGO_PKG_VERSION"),
        env!("A02_RUSTC_VERSION"),
        std::env::consts::ARCH,
        std::env::consts::OS,
        cfg!(debug_assertions),
    );
    println!("CPU allocation-inclusive lowering, single process, plan drop excluded.");
    for (i, elapsed) in samples.iter().enumerate() {
        let phase = if i < args.warmup as usize {
            "warmup"
        } else {
            "sample"
        };
        println!("{phase} {} ns", elapsed.as_nanos());
    }
    Ok(true)
}

fn run_engines(args: &Args) -> Result<bool, String> {
    let case =
        cases::get(&args.case, args.seed).ok_or_else(|| format!("unknown case {}", args.case))?;
    let engines: &[_] = match args.engine.as_str() {
        "general" => &[Engine::General],
        "light" => &[Engine::Light],
        "both" => &[Engine::General, Engine::Light],
        _ => return Err("engine-check requires --engine general, light or both".into()),
    };
    let rewrite = verify::check(
        &case.layout,
        &case.storage,
        &case.rewrite,
        BUDGET,
        Mode::Full,
    );
    let mut passed = true;
    for &engine in engines {
        let [reference, candidate] = [
            ("reference", &case.rewrite.reference),
            ("candidate", &case.rewrite.candidate),
        ].map(|(variant, ops)| {
            let result =
                a02_momiru::check_read_only(engine, &case.layout, &case.storage, ops, BUDGET);
            let (status, reason, origin) = match &result {
                Ok(_) => (Status::Pass, "MatchesInterpreter".into(), "native-observation"),
                Err(error @ a02_momiru::EngineError::OraclePreflight(_)) => {
                    (error.status(), format!("{error:?}"), "oracle-preflight")
                }
                Err(error) => (error.status(), format!("{error:?}"), "engine-adapter"),
            };
            println!(
                "{} {reason} origin={origin} engine={engine:?} variant={variant} case={} seed={} momiru={}",
                status.code(),
                args.case,
                args.seed,
                a02_momiru::REVISION
            );
            result
        });
        // Each variant may match its own oracle even when the rewrite is
        // unsound. Admission requires both the full model verdict and equality
        // of the observations returned by the selected native engine.
        let (status, reason) = if !rewrite.eligible(Mode::Full) {
            (rewrite.status, format!("Model({:?})", rewrite.reason))
        } else {
            match (reference, candidate) {
                (Ok(reference), Ok(candidate)) if reference == candidate => {
                    (Status::Pass, "EquivalentObservations".into())
                }
                (Ok(_), Ok(_)) => (Status::Fail, "ObservationMismatch".into()),
                (Err(error), _) | (_, Err(error)) => {
                    (error.status(), format!("Variant({error:?})"))
                }
            }
        };
        println!(
            "{} NativeRewrite {reason} engine={engine:?} case={} seed={} momiru={}",
            status.code(),
            args.case,
            args.seed,
            a02_momiru::REVISION
        );
        passed &= status == Status::Pass;
    }
    Ok(passed)
}

fn bench(
    case: &cases::Case,
    warmup: u32,
    samples: u32,
) -> Result<Vec<std::time::Duration>, String> {
    (0..warmup + samples)
        .map(|_| {
            let start = Instant::now();
            let plan = lower::compile(
                black_box(&case.layout),
                black_box(&case.rewrite.candidate),
                BUDGET,
            )
            .map_err(|fault| format!("lowering failed during diagnostic: {fault:?}"))?;
            black_box(&plan);
            Ok(start.elapsed())
        })
        .collect()
}

fn main() -> ExitCode {
    let mut tokens = std::env::args().skip(1).peekable();
    match tokens.peek().map(String::as_str) {
        Some("--help") => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Some("--version") => {
            println!("a02-layoutproof {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Some("--list") => {
            println!("{}", cases::IDS.join("\n"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    match options(tokens).and_then(run) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("ERROR {error}");
            ExitCode::from(2)
        }
    }
}
