# A02 regression results

The paper passed 27 tests in debug and release with all features. Existing tests were preserved. Eleven new tests cover the live dependency and reproduced red-team findings.

| Finding | Reproduction and correction |
| --- | --- |
| P0 slice-fusion definedness | Taking three elements from extent two and then taking one fails as a reference, while the fused single-element slice succeeds. Z3 reproduced the differing domains. The emitted law checks reference-to-candidate definedness, and scoped replay rejects this input before solver execution. |
| P0 flip rank precondition | Two axis-one flips fail on rank one while the identity succeeds. The emitted obligation includes the axis constraint and scoped replay rejects that input. A rank-two empty-axis input exercises the admitted boundary. |
| P0 live aliases and logical lineage | Opposing signed strides over the same declared live region reject an exclusive claim. Interleaved disjoint regions pass exact intersection checking. Reversal of a zero-stride input preserves physical offsets and changes logical lineage. Materialization rejects IDs reserved by live peers. |
| P1 fault precedence and status | Bounds rejection precedes observation of duplicate addresses. In both full and affine verification, a candidate requiring writes on read-only input reports `FAIL CandidatePrecondition(WriteDenied)`. |
| P0/P1 live engine validation | The pinned General and Light implementations matched the independent interpreter on the shared signed-view tests. Tests compare addresses relative to each backing slice, exact FP32 payloads, and actual mutable post-state. Light rank nine is unsupported, and both native engines reject non-forward-contiguous reshape that the abstract model accepts. |

Solver replay establishes the encoded conditional arithmetic obligations and concrete address witnesses. Scientific evidence collection, measured engine comparison, cross-language conformance and end-to-end theorem validation remain outside this test report. Exact commands and the independent upstream Clippy failure are recorded in the [build log](BUILD_LOG_A02_MOMIRU.md).

## Follow-up at paper HEAD f33bda2

Fresh debug and release runs each passed 36 tests. The six admission regressions cover distinction between per-variant agreement and rewrite acceptance, explicit oracle preflight provenance, direct native negative inputs, large unsigned empty slicing, and malformed exclusive peer rejection. The three follow-up proof tests reject impossible rank guards for all four registered law families, refute fixed equivalence on independently observed shape and storage differences, and check declared live peers in fixed replay before starting Z3. [Fresh validation commands](BUILD_LOG_A02_MOMIRU.md)

## Full live-context regression at paper HEAD 8f9e75f

On the previous source, an exclusive signed-stride peer and a shared peer could overlap away from the subject and still pass both model checks. The new regression rejects this configuration whether the conflicting peers share the subject's allocation or occupy a separate allocation. Both implementations accept disjoint interleaved writers and overlapping readers. Capacity disagreement remains a rejection in the overlap ablation, and exhausted verification returns `UNKNOWN`.

Fresh debug and release runs each passed 43 tests. The native engine checks and proof replay ran separately from the live-context regression. [Commands and statuses](BUILD_LOG_A02_MOMIRU.md)

## V11 terminal materialization

The two new regression tests gather a signed, permuted and broadcast view with both pinned native engines, then pass the gathered values to each engine's public owned tensor constructor. They compare actual backing capacity, checked dense offsets, exact FP32 payloads, and disjoint backing ranges with the adapter's independent checks. Coverage includes empty and scalar outputs, General rank nine versus Light's rank limit, and rejection of reuse of the input storage ID. This path covers one terminal materialization. Borrowed rewrite and benchmark modes keep their existing scope.

The paper passed 48 tests in each fresh debug and release run. Historical red-team regressions and the native benchmark admission checks ran with both profiles. [Exact commands and statuses](BUILD_LOG_A02_MOMIRU.md)
