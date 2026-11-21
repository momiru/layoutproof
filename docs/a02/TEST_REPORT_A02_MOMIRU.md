# Native test report

The Rust integration suite checks scalar, empty and singleton domains, ranks 0 through 16 in the model, signed and zero strides, stepped slices, checked overflow, invalid offsets, write aliasing, FP32 payload preservation, permutation, reshape and materialization. A bounded cross-product of signed-stride layouts checks minimal witnesses and radix-changing reshapes against the independent coordinate interpreter.

Mutation tests cover wrong coordinate order, physical storage changes after a value-preserving copy, and unauthorized writes. The last two deliberately pass their targeted ablations. They fail full verification and are ineligible for benchmark sampling. Runner tests exercise admitted diagnostic samples, refused benchmark admission, and evidence refusal without filesystem output.

Z3 tests replay four local law families, a composed fixed-shape address equality, inconsistent certificates and minimal concrete SAT witnesses. Zero solver budget exercises `UNKNOWN`. Proof results remain conditional on the documented preconditions and the trusted solver. The CLI development tests supply on-path checks, and the external SMT-LIB replay command provides a way to inspect the arithmetic obligation independently.

Upstream Momiru General and Light runtime tests, GPU, AOT, cross-language conformance, canonical timing runs and confirmatory holdouts were NOT_RUN. See the build log for exact command outcomes.
