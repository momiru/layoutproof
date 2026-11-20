LayoutProof for Momiru
Rust-native research on rank-polymorphic verification of tensor layouts, views, and rewrite rules.

Momiru — Rust AI framework · Momijo — independent Mojo framework · Companion Mojo LayoutProof study
Research status — pre-results. This is the dedicated Momiru implementation of the LayoutProof study (internal ID A02). The mathematical method, native checker, executable tests, and hardware measurements must pass the documented acceptance gates before any claim of verification, correctness, or speedup is made. This README specifies the intended scientific and engineering contract; the original source archive and the complete common-protocol package must be audited/imported separately.

Research problem · Method · Evaluation · Reproducibility · Contributing
At a glance
Item	Scope
Parent framework	Momiru — independently developed, Rust-native tensor/AI framework
Repository	momiru/layoutproof
Primary language	Rust for executable source, tests, codecs, and native measurement
Research focus	Tensor layout semantics, signed strides, alias safety, and checkable rewrite evidence
Engines	General runtime-rank tensors and an independent Light FP32 fixed-rank engine
Evidence	Independent checker, mutation controls, native schema-v4 samples, versioned provenance
Availability	Research implementation under development; no validated results published yet


Why Momiru?
Momiru is the Rust-native project aimed at building an extensible AI/deep-learning and numerical-computing framework, with a long-term ambition to provide a Rust-native alternative to PyTorch. LayoutProof is not a replacement for Momiru; it is a paper-specific, isolated research line that develops and evaluates one correctness problem inside its tensor stack.

Rust provides an appropriate environment to express storage ownership, checked shape/offset arithmetic, and carefully bounded unsafe operations. Those language features do not, on their own, prove that any candidate tensor rewrite preserves semantics. This study therefore introduces an explicit reference model, proof obligations, and independent evidence rather than treating type safety as a substitute for algorithmic verification.

The parent project exposes momiru::tensor::Tensor (General) and momiru::tensor_light::Tensor::<R> (Light) as separate implementation families; there is no assumed root-level momiru::Tensor alias. General supports runtime-rank tensor metadata (subject to actual implementation limits); Light is an FP32-focused engine with ranks 0–8. The study evaluates both when semantics overlap, while higher ranks and non-FP32 types are eligible only for the appropriate General implementation. No inter-engine transmute, unchecked ownership conversion, or fictitious GPU capability is permitted.

Related projects: Momiru source foundation, Momijo source foundation, and the independent Mojo companion study. The two LayoutProof implementations share scientific contracts, not an ABI, implementation, compiler, or guaranteed API compatibility.
Design boundaries in Momiru
The supplied article protocol identifies these source-inspection anchors in the Momiru source archive:
- src/tensor/tensor/shape.rs — General shape representation and rank behavior.
- src/tensor/tensor/view.rs — General view semantics and layout-sensitive operations.
- src/tensor_light/tensor/layout.rs — Light-engine layout rules.
These are inspection targets, not an assertion that a finished LayoutProof prover is already present. The baseline audit must also inspect Cargo.toml, existing exports, tests, benches, features, and actual engine contracts. Research functionality should be opt-in; an unchanged reference path must remain callable for regression and ablation comparisons. Any change to a public API requires a documented compatibility review.

Rust-specific safety rules: use checked products and signed-offset operations; verify allocation extent and alignment; encode failures as structured results; minimize and document unsafe; and test ownership/alias invariants at adversarial boundaries. A simulator or CPU state model cannot be reported as actual GPU execution.
Research problem
Tensor rewrites are not correct merely because two expressions return the same values on a handful of shapes. A transformation can preserve logical elements while changing storage addresses, creating aliases, requiring materialization, or permitting unsafe writes. Zero-sized axes, scalar tensors, negative strides, broadcasted zero strides, and symbolic dimensions make these distinctions especially important.

For a signed-strided tensor view, the proposed model treats the address of a logical index i as:
\[
a(\mathbf{i}) = o + \sum_{k=0}^{r-1} i_k s_k
\]
where o is the storage offset, s_k is a signed stride, and the index domain is constrained by the tensor shape. The study separates logical-value equivalence, physical address-map equivalence, and write/alias admissibility. It checks allocation bounds and integer overflow rather than treating out-of-bounds addresses or overflow as valid examples.
Research questions
1. Correctness: Which layout-sensitive tensor rewrites can be verified under explicit shape, storage, and alias preconditions?
2. Comparative value: Does the proposed proof-obligation approach offer a defensible improvement over suitable verification baselines and finite-model testing?
3. Generalizability and cost: What changes with rank, shape, stride family, solver resources, tensor engine, and available hardware?
This work is a research hypothesis, not an already established theorem or performance result.
Proposed LayoutProof method
The intended mechanism is a typed intermediate representation for transformations such as reshape, transpose, permute, slice, flip, expand, squeeze, and broadcast.
1. Normalize the contract: Record shape/rank, dtype, signed strides, storage offset/extent, ownership/alias constraints, operation parameters, and applicable execution capabilities.
2. Preflight: Reject invalid inputs through checked arithmetic, bounds tests, and explicit eligibility checks; never rely on assertions for production validation.
3. Construct obligations: Formalize compatible domains, logical-index/value mappings, physical address mappings, and write permissions separately.
4. Check candidates: Use an appropriately supported symbolic method and/or finite enumeration; record solver assumptions, resource limits, and unresolved constraints.
5. Independently verify evidence: Replay counterexample models or proof/certificate evidence in a distinct checker; test deliberately incorrect rewrite mutants.
6. Measure only admitted variants: Correctness must pass for the exact (case_id, variant) before performance samples are used in an accepted comparison.
An exhaustive search over a bounded set of ranks and shapes is not presented as a universal proof. An inconclusive solver attempt remains UNKNOWN, including when a documented resource limit is reached.
Result-state contract
Status	Meaning
PASS	The applicable oracle or checker independently accepted this concrete result/evidence.
REJECTED	An invalid input or candidate was correctly refused, with a recorded reason.
FAIL	A mismatch, unsound certificate, invalid acceptance, or violated invariant was detected.
UNKNOWN	A supported analysis was inconclusive, including proof-search limits.
UNSUPPORTED	The requested engine, operation, dtype, rank, toolchain, or device capability is unavailable.
NOT_RUN	No attempted run occurred, or a required dependency/hardware was absent.
ERROR	An unexpected infrastructure failure or execution error occurred.


UNKNOWN, UNSUPPORTED, NOT_RUN, and ERROR are never silently converted into PASS. Correctly rejected malformed inputs are not conflated with candidate correctness failures.
Evaluation plan
The experiment matrix is anchored to A02 and includes the following families:
Family	Primary concern
C00	Eligible General/Light FP32 comparisons
C01	Rank boundaries: scalar, rank 1/8, and General-only rank 9/16
C02	Contiguous, transposed, negative-stride, zero-stride, and aliased layouts
C03	Small/large shapes, resource stress, and checked overflow rejection
C04	Ablations with one meaningful mechanism varied at a time
C05	Predeclared held-out cases and uncertainty estimates
C06	Known-broken rewrites and mutation detection
C07	Real toolchain, CPU, and optional verified-device eligibility
C08	LayoutProof's primary compositions: 2–6 rewrite steps and symbolic/concrete shapes
C09	Proof/counterexample replay and resource-limited formal analysis


The study covers rank 0, empty axes, singleton axes, ranks 1/2/4/8, and higher ranks only where the General engine supports them. Cases address overlapping writes, negative and broadcast strides, invalid offsets, and huge shapes that should fail safely. Each family is classified in an applicability matrix before execution; irrelevant experiments are not manufactured just to fill a template.

Candidate baselines include TensorRight, A Verified Compiler for a Functional Tensor Language, independent finite-model checking, unchanged native tensor behavior, and mutation-based validation. Compatibility and licensing must be checked before adopting any comparison. A stronger baseline may invalidate a novelty claim; negative findings remain reportable.

Planned ablations include disabling the physical-address obligation, disabling an alias precondition as a deliberately failing negative control, finite-only checking, symbolic-only checking, and manual/baseline rewrite validation. Intentionally unsound variants may be used to test detection, but must never enter an approved optimization path.
Primary outcomes
- Number and coverage of verified rewrite rules and unsupported rule classes.
- Proof/decision time and solver resource use for eligible cases.
- Counterexample detection rate, reproducibility, and minimized witnesses.
- Failures and limitations under shape polymorphism.
- Correctness-filtered performance and overhead, with uncertainty, when native timed operations genuinely apply.
Planned article-specific artifacts are rewrite_coverage.svg, proof_time_vs_complexity.svg, accepted_vs_rejected_rules.tsv, and proof_certificates/index.tsv. These files are not claimed to exist or contain experimental results until generated and independently validated.
Reproducibility and evidence integrity
The research uses the package-root, schema-v4 contract named COMMON_OUTPUT_CONTRACT.md (from the separate article protocol package). Each actual tensor-native run writes immutable records outside the distributable source tree:
${RESEARCH_OUTPUT_ROOT}/A02/<repository>/<engine>/<run_id>/
  manifest.toml
  cases.tsv
  correctness.tsv
  raw_samples.tsv
  metrics.tsv
  failures.tsv
  ablations.tsv
  summary.md
The reporting stage, which is separate from native execution, uses:
${RESEARCH_OUTPUT_ROOT}/A02/aggregate/<analysis_run_id>/
  analysis_manifest.toml
  input_audit.tsv
  case_eligibility.tsv
  comparisons.tsv
  statistics.tsv
  report_validation.tsv
  figures/
  tables/
  summary.md
  reproducibility.md
  artifact_checksums.sha256
Native output columns and their semantics must come from the canonical contract; this README does not define a competing schema. Raw samples, correctness verdicts, and manifests are immutable, and aggregate reports never overwrite them. Each case has an independently reproducible input digest, a concrete variant-specific oracle outcome, and recorded source/toolchain/hardware provenance.
Portable input, output, and non-tensor protocols
- QTC1 is the versioned portable tensor input corpus. Exact cross-language input equivalence requires independent Rust and Mojo codec conformance, matching bytes/hashes and semantic contracts (including storage alias topology, signed zero and NaN representation where applicable).
- QTO1 is versioned observed numerical output evidence, combined with a genuinely independent numerical oracle, declared tolerances, and post-state/alias checks. Matching digests alone do not establish correctness; tolerant outputs may correctly have different digests.
- NTE1 is the separate evidence route for proof-only queries or installation/provenance checks with no actual tensor operand. Never invent a dummy tensor to fit tensor-run schema v4.
These are protocol requirements, not claims that native codecs, proof solvers, or test vectors have already passed. Their canonical definitions, positive/negative fixtures, and output evidence rules must be imported/pinned from the complete research protocol package before measurement. Actual cross-language comparisons require both independent native conformance reports and separately pinned source revisions; commit hashes from distinct repositories are not expected to be identical.
Measurement admission rules
1. Freeze each repository's baseline commit, compiler and dependency versions, semantic contract, case corpus, and oracle version.
2. Predeclare comparisons, timing boundaries, eligibility rules, and relevant confidence/uncertainty procedures.
3. Run correctness and evidence verification before accepting any timed candidate variant.
4. Do not compare compiler-inclusive durations to kernel-only or allocation-inclusive durations.
5. Retain every valid and failed raw attempt; do not turn missing or failed measurements into zero latency.
6. Report GPU performance only after a supported device executes and synchronizes the measured kernel. Otherwise mark NOT_RUN or UNSUPPORTED with a reason.
7. Use observed, independently validated measurements when editing the manuscript; never insert projected numbers as findings.
Getting started — Momiru (Rust)
Prerequisites: a supported operating system, the Rust toolchain and Cargo, and the actual Momiru baseline source. The parent project currently documents Rust 1.99.1 through rust-toolchain.toml, Cargo edition 2021, and a checked-in Cargo.lock. Pin the exact imported toolchain/dependency state and source commit before interpreting any result; build targets and features are determined by the imported manifests.
# Clone the article repository after it has been created and populated.
git clone https://github.com/momiru/layoutproof.git
cd layoutproof

# Record the actual source/toolchain identity before a run.
git rev-parse HEAD
rustc --version
cargo --version

# Baseline verification once the Momiru sources are imported:
cargo check --locked
cargo check --locked --all-targets
cargo test --locked
cargo test --locked --features aot
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings

# Parent-framework demonstrations (not LayoutProof results):
cargo run --example tensor_basic
cargo run --example tensor_light_basic
The listed commands and example names are documented by the current parent Momiru repository; importing a different source snapshot may change them. If a lockfile is missing, resolve it under the project dependency policy before using --locked. These commands are not claimed as executed in the new article repository. Invoke a real bench target (the parent currently documents tensor_cpu_matmul) only after confirming registration and baseline correctness.
Intended research entry points (not yet certified)
The native implementation is expected to expose an opt-in A02 benchmark/export runner accepting --paper A02, an eligible engine selector (general, light, or both), deterministic --seed, case selection, warmup/repetition controls, and --output-root. Its actual CLI path and executable name must be determined by inspecting the resulting Cargo targets; no placeholder command is presented here as executable.
Planned repository layout after source import
layoutproof/
├── src/                         # Existing Momiru Rust source
│   ├── tensor/                  # General tensor implementation
│   └── tensor_light/            # Independent FP32 Light engine
├── tests/                       # Existing tests + A02 regression tests
├── benches/                     # Existing and A02 Rust benches (if registered)
├── docs/research/A02/           # Protocol, applicability, design, audit and handoff
├── .github/workflows/           # Rust validation and research CI
├── AGENTS.md                    # Short, source-backed coding-agent instructions
├── Cargo.toml                   # Source-of-truth build/features
├── Cargo.lock                   # Baseline-pinned dependency state
├── rust-toolchain.toml           # Baseline-pinned Rust compiler
├── build.rs                     # Inherited build/provenance logic
├── LICENSE                      # Original MIT attribution retained
└── README.md
This is a target map, not a claim that all paths or Cargo targets already exist in the new repository. Prefer extending verified existing modules to creating parallel, unused implementations.
Release readiness — Rust-specific gates
- cargo fmt, cargo test, and cargo clippy must be attempted and their real results recorded under a pinned toolchain (with absent tools reported as NOT_RUN).
- Check General/Light applicability per operation. Light's FP32 rank limit must never be treated as a defect in the General engine or vice versa.
- An independent oracle must reject deliberately broken rewrite variants. Correctness and artifact validation must run before timing comparisons.
- Any Rust unsafe block introduced for research must have explicit safety invariants and boundary tests; an invalid signed offset must never trigger undefined behavior.
- Native GPU benchmarks remain UNSUPPORTED or NOT_RUN until a real tested backend, hardware, and completion synchronization exist.
- The final source distribution contains Rust executable code and allowed metadata/docs—not Python, Mojo, JavaScript application code, generated binaries, local caches, or raw experiment outputs.
Scientific provenance and article
- Study identifier: A02 (an internal project ID, deliberately excluded from the repository slug).
- Working paper title: LayoutProof: Rank-Polymorphic Verification of Layout-Sensitive Tensor Transformations.
- Manuscript status: Pre-results research draft; no formal result, performance advantage, device capability, or acceptance by a journal is asserted.
- Proposed target venue: Proceedings of the ACM on Programming Languages (venue fit to be assessed, not an accepted publication).
- Protocol: RESEARCH_SPEC.md, EXPERIMENT_MATRIX.tsv, ACCEPTANCE_CHECKLIST.md, and the normative common-output / portable-case / output-evidence / non-tensor specifications.
The research program uses this order: architecture and prior-work audit → native implementation and regression testing → independent benchmark/report validation → evidence-backed manuscript revision → independent red-team audit. No step is marked complete solely because its instructions exist.
Relevant prior work (starting points, not an exhaustive review)
- Arora et al. (2025), TensorRight: Automated Verification of Tensor Graph Rewrites.
- Liu, Bernstein, Chlipala, and Ragan-Kelley (2024), A Verified Compiler for a Functional Tensor Language.
- MLIR One-Shot Bufferization (official documentation).
- NumPy ndarray strides (official documentation).
All bibliographic entries, author lists, publication dates, and direct-method comparisons must be independently rechecked against primary sources before submission.
Contributing and engineering standards
Contributions should be narrowly scoped, independently reviewable, and tied to a testable contract. Please open an issue describing the semantic claim or defect, the affected operation/engine, a minimal counterexample, and its expected outcome. Pull requests should include negative and boundary tests, exact build/test commands with outcomes, and a source-to-evidence explanation if a scientific claim changes. Never suppress a failing test or replace measured observations with synthetic successes.

Do not commit secrets, local machine paths, private datasets, generated binaries, build caches, or raw benchmark output to a public release. Preserve all inherited copyright and third-party notices. Vulnerabilities that could expose data or cause memory unsafety should be reported privately to repository maintainers when a security contact is published.
Maintainers, licensing, and citation
Project founders and maintainers: Morteza Taleblou and Mitra Daneshmand. Maintainer status does not determine the final scientific manuscript author order, affiliations, or contribution statements; those will be confirmed by the authors.

The source foundation is MIT-licensed. Preserve the original MIT LICENSE, both copyright holders, and any third-party notices when creating this article repository. No source license may be changed implicitly by copying research code. See the checked-in license after the baseline source import.

When a paper or archived release becomes available, add the verified DOI, tagged source revision, and a correct CITATION.cff. Until then, cite this repository by its exact URL, commit SHA, and access date rather than inventing publication metadata.
LayoutProof is an independent, evidence-driven research implementation associated with its parent framework—not a claim that the entire framework, GPU stack, or formal verification system is already complete.
