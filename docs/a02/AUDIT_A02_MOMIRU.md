# Momiru source audit

The independent checkout was clean at `eba302d3c61313444b56303c8816f74a3aaf9679`. Cargo discovered 30 targets with all 23 declared benchmark files and both engine modules present. The five upstream A02 test targets ran in a target directory inside the paper workspace. Only the paper checkout was writable.

| Modern source | Observed public interface and integration |
| --- | --- |
| `src/momiru/tensor/mod.rs` | Runtime-rank General exposes `TensorView::new`, `layout().checked_offset`, `get`, `permute`, `flip`, count-based `slice_axis`, `reshape_view` and `broadcast_to`. Native reshape requires forward C-contiguity. The exclusive `TensorViewMut::new` checks proven uniqueness and `get_mut` writes through the borrow. |
| `src/momiru/tensor_light/tensor/signed_view.rs` | Light exports const-rank FP32 `SignedTensorView<R>` for ranks zero through eight. It exposes native signed slicing, permutation, flip, broadcast, expansion, squeeze, reshape and shared references. Reshape requires forward C-contiguity. |
| `src/momiru/tensor_light/tensor/signed_view_mut.rs` | `SignedTensorViewMut<R>::new` checks uniqueness and `set` mutates the storage. |
| `Cargo.toml` | Both engines build from one crate with distinct modules. The upstream `aot` feature is available and paper CI forwards it through Cargo. |

The paper adapter uses both engines' view APIs on actual backing slices and verifies observations against the paper interpreter. The parent library remains a separate Git dependency. The canonical Git URL and exact revision are stored in the paper manifest and lockfile. In this environment Cargo resolved the canonical URL using a per-command rewrite to the exact local Git object store. GitHub check runs for the library were unreachable from this session. The existing library pin's upstream CI annotation is orchestrator-supplied metadata. Direct local all-target/all-feature library Clippy with warnings denied failed on `tensor_light::aot::cpu_sgemm_primary_seed` having eight arguments. No upstream or lint-gate change was made.
