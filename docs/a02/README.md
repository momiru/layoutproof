# A02 implementation

The verifier distinguishes four observations of a rewrite. A result must preserve the output domain, the mapping from output coordinates to original logical coordinates, exact binary32 payloads, and the physical address map and write permission. Logical coordinates and value bits are checked separately, even when repeated values conceal a changed index. IEEE signed zero, infinities, subnormals and NaN payloads are carried as bits. No numerical tolerance or floating-point arithmetic is involved.

## Semantics

- Shape is a vector of unsigned extents. A scalar has one coordinate, the empty tuple. Any zero extent yields an empty domain. Cardinality is checked before coordinate evaluation. Empty layouts require an input origin between zero and capacity inclusive. Lowering canonicalizes empty strides and origin because there is no reachable element.
- A physical address is an abstract storage ID and an element offset. For index `i`, the offset is `origin + sum(i[k] * stride[k])`. The affine lowerer checks signed bounds through the extreme reachable corners and uses wide intermediates for 32- and 64-bit hosts. Singleton strides have no observable effect. There is no unsafe Rust or pointer reinterpretation.
- An exclusive input represents a caller-provided lease on the backing storage. The verifier separately requires the view to be injective. A sorted-span test discharges separated strides, and bounded enumeration checks the remaining cases. Exhausting the budget yields `UNKNOWN`. Broadcast always yields read-only access, even when it preserves shape. Other views keep their permission. `RequireWritable` makes an intermediate write precondition observable. Write checks are modeled assertions about a single input lease.
- Slice specifies an explicit start, count and nonzero signed step. Empty slices allow a start from zero through the axis extent. Coordinates of nonempty slices must all exist. Broadcast aligns trailing axes and can extend a singleton to any extent, including zero.
- Reshape preserves row-major logical order. The lowerer derives candidate strides from the origin and the target basis coordinates. It represents the address minus the origin as a sum of mixed-radix carry terms, each of the form `floor(flat / suffix) * coefficient`. Distinct suffixes below the domain size are independent, since the first differing term can be observed at its suffix. The independent interpreter evaluates the candidate basis at every coordinate. A non-affine reshape returns `RequiresCopy`. Explicit materialization into a fresh ID provides dense, exclusively writable storage. Repeated IDs in a program are rejected. Across the two alternative programs, equal IDs identify corresponding allocations by construction.

## Validation and proof scope

`lower` computes addresses and walks backward through program indexing to recover the input ordinal. `oracle` moves independently enumerated cells forward through each stage and checks actual bit payloads from the provided backing slice. In full mode both paths must agree before comparing programs. For materialized outputs the oracle simulates copied cells with new offsets, while the lowerer derives value provenance from the input. The procedures share the typed IR and its stated semantics.

`proof::affine_addresses` compares concrete affine maps through normalized origins and active-axis stride coefficients. The minimal witness is the zero index if origins differ, or the unit coordinate of the last differing active axis. `replay_fixed` checks the encoding and witness with the bounded interpreter. Z3 separately checks satisfiability and, for a witness, the absence of an earlier index. This is an address-only check. Shape or storage identity mismatches are structural decisions and are reported separately.

`proof::certify` recognizes flip and transpose involutions, expand followed by squeeze, and checked nonempty slice fusion. The replay path rebinds the law to the typed operations and checks a local integer identity with Z3. Rank is arbitrary subject to the axis preconditions. Untouched axes form an unchanged frame. For these laws, domain and input-coordinate preservation imply value and address equality on the same input, and the operations preserve the input lease. Slice fusion requires both reference slices to be defined and the fused step and start to be representable. The query checks local equalities. The code and this argument supply the correspondence between those equalities and the complete layout semantics. The replay trusts Z3 for the arithmetic result.

Replay the SMT-LIB obligation in a separate process:

```sh
set -o pipefail
cargo run --quiet --offline --locked --bin a02-layoutproof -- \
  --mode query --case C09-slice-fusion | z3 -in -smt2
```

`--timeout-ms 0` exercises `UNKNOWN` without starting a solver. Solver absence is `NOT_RUN`. Unsupported symbolic programs remain `UNSUPPORTED` unless a separate applicable check runs. `REJECTED` indicates an invalid input or certificate. `FAIL` indicates a failing candidate or a satisfiable negated obligation. `ERROR` covers infrastructure or conflicting checks.

## Experiments and provenance

Modes `finite`, `symbolic`, and `manual` select the bounded interpreter, supported symbolic laws, and the manual allowlist (literal identity, double flip, and expand/squeeze cancellation). `no-address` disables physical identity comparison. `no-alias` removes candidate write preconditions. The latter two intentionally let their associated negative controls pass the ablated check, and benchmark admission excludes all ablated decisions.

The CLI development benchmarks time candidate lowering on the CPU with a monotonic clock. The measured interval includes allocation and excludes compilation, correctness checks, plan drop and printing. Warmups and subsequent samples are reported in order. A zero-warmup diagnostic starts with the first invocation after preflight in that process. Timing starts inside the running process. Canonical evidence, hardware comparisons and aggregation are gated off at present. See the [test report](TEST_REPORT_A02_MOMIRU.md), [build log](BUILD_LOG_A02_MOMIRU.md), [applicability matrix](APPLICABILITY_MATRIX.tsv), and [handoff](HANDOFF_A02_MOMIRU.md).

The CLI returns exit 0 for an accepted check or benchmark, 1 for an unavailable or refused run, and 2 for invalid command arguments or errors raised by the runner. In default check mode a registered negative control returns exit 0 when its expected failure is observed. The printed decision is still `FAIL`.
