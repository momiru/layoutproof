# Momiru source audit

The paper checkout initially tracked README.md and LICENSE and had an existing untracked library pin. There was no baseline Cargo target or existing paper test to run. The pinned upstream commit and dirty checkout are different snapshots. The pin remains unchanged.

| Location in independent checkout | Observed capability |
| --- | --- |
| `Cargo.toml` and `rust-toolchain.toml` | Rust 1.99.1, edition 2021, no Cargo dependencies, optional `aot`. Current manifest points to `src/momiru/lib.rs` and the pin points to `src/lib.rs`. Both declare 23 benchmarks with absent source in the available modern tree. |
| `src/momiru/lib.rs` | Declares independent `tensor` and `tensor_light` modules. The runtime-rank General module is absent from both available source inventories. |
| `src/momiru/tensor_light/tensor/layout.rs` | `Layout<R>::new(dims, strides, origin, capacity)`, `checked_offset([usize; R])`, and internal overlap classification. |
| `src/momiru/tensor_light/tensor/signed_view.rs` | Dirty checkout exposes `SignedTensorView::new(dims, strides, origin, storage)` and `get([usize; R]) -> Result<f32, TensorError>`. Read-only signed view operators include reshape, permutation, flip and broadcast. These signatures are source observations. |
| `src/momiru/tensor_light/tensor/signed_view_mut.rs` | Separate dirty mutable view source. No borrowing or interoperation guarantee is inferred for the pinned commit. |
| `tests/a02_light_layout.rs`, `tests/a02_borrowed_signed_views.rs`, `tests/a02_stride_spacing.rs` | Present in dirty upstream. No test copied or changed. |
| `.github/workflows/` | Upstream CI, benchmark, release and security workflows exist. The paper has no inherited CI target. |

Baseline library Cargo execution was NOT_RUN in this work order. Target and module discovery already failed at source inspection and the library is outside writable scope. General, Light, device and AOT runtime execution are unverified for this paper. The native paper crate owns the IR, semantics, interpreter and proof experiments. Adapter descriptors are explicit and preserve engine boundaries without linking the dirty upstream checkout.
