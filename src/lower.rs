//! Checked affine lowering. The interpreter uses coordinate substitution instead.

use crate::ir::{cardinality, Address, Fault, Layout, Op, Permission};
use std::collections::{BTreeMap, HashSet};

pub struct Lowered {
    pub(crate) output: Layout,
    stages: Vec<Vec<usize>>,
    ops: Vec<Op>,
}

impl Lowered {
    pub fn output(&self) -> &Layout {
        &self.output
    }

    /// Input row-major ordinal for a coordinate in the output domain.
    pub fn source_ordinal(&self, index: &[usize]) -> Result<usize, Fault> {
        ordinal(&self.output.shape, index)?;
        let mut index = index.to_vec();
        for (stage, op) in self.ops.iter().enumerate().rev() {
            let input = &self.stages[stage];
            match op {
                Op::Transpose { first, second } => index.swap(*first, *second),
                Op::Permute(axes) => {
                    let mut previous = vec![0; axes.len()];
                    for (&axis, coordinate) in axes.iter().zip(index) {
                        previous[axis] = coordinate;
                    }
                    index = previous;
                }
                Op::Slice {
                    axis, start, step, ..
                } => {
                    index[*axis] = point(*start, *step, index[*axis])?;
                }
                Op::Flip(axis) => index[*axis] = input[*axis] - 1 - index[*axis],
                Op::Expand(axis) => {
                    index.remove(*axis);
                }
                Op::Squeeze(axis) => index.insert(*axis, 0),
                Op::Broadcast(_) => {
                    let leading = index.len() - input.len();
                    index.drain(..leading);
                    for (coordinate, &extent) in index.iter_mut().zip(input) {
                        if extent == 1 {
                            *coordinate = 0;
                        }
                    }
                }
                Op::Reshape(_) => {
                    index = coordinate(input, ordinal(&self.stages[stage + 1], &index)?)
                }
                Op::RequireWritable | Op::Materialize(_) => {}
            }
        }
        ordinal(&self.stages[0], &index)
    }
}

pub fn compile(layout: &Layout, ops: &[Op], alias_budget: usize) -> Result<Lowered, Fault> {
    let size = validate(layout)?;
    if layout.permission == Permission::Exclusive {
        unique(layout, size, alias_budget)?;
    }
    let mut out = layout.clone();
    canonicalize(&mut out);
    let mut stages = vec![out.shape.clone()];
    let mut used_storage = HashSet::from([out.storage]);
    for op in ops {
        if let Op::Materialize(storage) = op {
            if !used_storage.insert(*storage) {
                return Err(Fault::StorageReuse);
            }
        }
        apply(&mut out, op)?;
        canonicalize(&mut out);
        validate(&out)?;
        stages.push(out.shape.clone());
    }
    Ok(Lowered {
        output: out,
        stages,
        ops: ops.to_vec(),
    })
}

fn canonicalize(layout: &mut Layout) {
    if layout.shape.contains(&0) {
        layout.origin = 0;
        layout.strides.fill(0);
    } else {
        for (stride, &n) in layout.strides.iter_mut().zip(&layout.shape) {
            if n == 1 {
                *stride = 0;
            }
        }
    }
}

fn validate(layout: &Layout) -> Result<usize, Fault> {
    if layout.shape.len() != layout.strides.len() {
        return Err(Fault::RankMismatch);
    }
    let size = cardinality(&layout.shape)?;
    if size == 0 {
        if (0..=layout.capacity as i128).contains(&i128::from(layout.origin)) {
            return Ok(0);
        }
        return Err(Fault::Bounds);
    }
    let mut low = i128::from(layout.origin);
    let mut high = low;
    for (&n, &stride) in layout.shape.iter().zip(&layout.strides) {
        let delta = (n as i128 - 1) * i128::from(stride);
        low = low.checked_add(delta.min(0)).ok_or(Fault::Overflow)?;
        high = high.checked_add(delta.max(0)).ok_or(Fault::Overflow)?;
    }
    // The IR admits signed i64 reachable offsets.
    if low < 0 || high >= layout.capacity as i128 || high > i128::from(i64::MAX) {
        return Err(Fault::Bounds);
    }
    Ok(size)
}

fn unique(layout: &Layout, size: usize, budget: usize) -> Result<(), Fault> {
    if size <= 1 {
        return Ok(());
    }
    let mut axes: Vec<_> = layout
        .shape
        .iter()
        .zip(&layout.strides)
        .filter(|(&n, _)| n > 1)
        .map(|(&n, &s)| (s.unsigned_abs(), n))
        .collect();
    axes.sort_unstable();
    if axes[0].0 == 0 {
        return Err(Fault::WriteOverlap);
    }
    let mut span = 0_u128;
    let separated = axes.into_iter().all(|(stride, extent)| {
        let stride = u128::from(stride);
        if stride <= span {
            return false;
        }
        span += stride * (extent as u128 - 1);
        true
    });
    if separated {
        return Ok(());
    }
    if size > budget {
        return Err(Fault::WorkLimit);
    }
    let mut seen = HashSet::new();
    for flat in 0..size {
        if !seen.insert(address(layout, &coordinate(&layout.shape, flat))?.offset) {
            return Err(Fault::WriteOverlap);
        }
    }
    Ok(())
}

fn axis(layout: &Layout, axis: usize) -> Result<usize, Fault> {
    layout.shape.get(axis).copied().ok_or(Fault::AxisOutOfRange)
}

fn point(start: usize, step: i64, index: usize) -> Result<usize, Fault> {
    let value = i128::from(step)
        .checked_mul(index as i128)
        .and_then(|delta| delta.checked_add(start as i128))
        .ok_or(Fault::Overflow)?;
    usize::try_from(value).map_err(|_| Fault::InvalidSlice)
}

fn slice(
    layout: &mut Layout,
    a: usize,
    start: usize,
    count: usize,
    step: i64,
) -> Result<(), Fault> {
    let n = axis(layout, a)?;
    if step == 0 || start > n || (count != 0 && (start == n || point(start, step, count - 1)? >= n))
    {
        return Err(Fault::InvalidSlice);
    }
    layout.shape[a] = count;
    if !layout.shape.contains(&0) {
        let old = i128::from(layout.strides[a]);
        let origin = old * start as i128 + i128::from(layout.origin);
        layout.origin = i64::try_from(origin).map_err(|_| Fault::Overflow)?;
        if count > 1 {
            layout.strides[a] =
                i64::try_from(old * i128::from(step)).map_err(|_| Fault::Overflow)?;
        }
    }
    Ok(())
}

fn reshape(layout: &mut Layout, shape: &[usize]) -> Result<(), Fault> {
    let size = cardinality(&layout.shape)?;
    if size != cardinality(shape)? {
        return Err(Fault::ShapeMismatch);
    }
    if layout.shape == shape {
        return Ok(());
    }
    let mut strides = vec![0; shape.len()];
    if size > 1 {
        let mut suffix = 1;
        for (&extent, stride) in shape.iter().zip(&mut strides).rev() {
            if extent > 1 {
                let basis = coordinate(&layout.shape, suffix);
                let offset = address(layout, &basis)?.offset as i128;
                *stride = i64::try_from(offset - i128::from(layout.origin))
                    .map_err(|_| Fault::Overflow)?;
                suffix *= extent;
            }
        }
        if carry_terms(&layout.shape, &layout.strides) != carry_terms(shape, &strides) {
            return Err(Fault::RequiresCopy);
        }
    }
    layout.shape = shape.to_vec();
    layout.strides = strides;
    Ok(())
}

/// The offset at flat position t, less the origin, is the sum of
/// floor(t / suffix) times the corresponding mixed-radix carry coefficient.
fn carry_terms(shape: &[usize], strides: &[i64]) -> BTreeMap<usize, i128> {
    let (mut suffix, mut reset) = (1, 0);
    let mut terms = BTreeMap::new();
    for (&extent, &stride) in shape.iter().zip(strides).rev() {
        if extent > 1 {
            let stride = i128::from(stride);
            let coefficient = stride - reset;
            if coefficient != 0 {
                terms.insert(suffix, coefficient);
            }
            reset = extent as i128 * stride;
            suffix *= extent;
        }
    }
    terms
}

fn apply(layout: &mut Layout, op: &Op) -> Result<(), Fault> {
    match op {
        Op::Transpose { first, second } => {
            axis(layout, *first)?;
            axis(layout, *second)?;
            layout.shape.swap(*first, *second);
            layout.strides.swap(*first, *second);
        }
        Op::Permute(axes) => {
            let mut seen = vec![false; layout.shape.len()];
            if axes.len() != seen.len()
                || axes.iter().any(|&a| {
                    seen.get_mut(a)
                        .is_none_or(|flag| std::mem::replace(flag, true))
                })
            {
                return Err(Fault::InvalidPermutation);
            }
            layout.shape = axes.iter().map(|&a| layout.shape[a]).collect();
            layout.strides = axes.iter().map(|&a| layout.strides[a]).collect();
        }
        Op::Slice {
            axis,
            start,
            count,
            step,
        } => slice(layout, *axis, *start, *count, *step)?,
        Op::Flip(a) => {
            let n = axis(layout, *a)?;
            slice(layout, *a, n.saturating_sub(1), n, -1)?;
        }
        Op::Expand(a) => {
            if *a > layout.shape.len() {
                return Err(Fault::AxisOutOfRange);
            }
            layout.shape.insert(*a, 1);
            layout.strides.insert(*a, 0);
        }
        Op::Squeeze(a) => {
            if axis(layout, *a)? != 1 {
                return Err(Fault::InvalidSqueeze);
            }
            layout.shape.remove(*a);
            layout.strides.remove(*a);
        }
        Op::Broadcast(shape) => {
            let leading = shape
                .len()
                .checked_sub(layout.shape.len())
                .ok_or(Fault::InvalidBroadcast)?;
            let mut strides = vec![0; shape.len()];
            for (j, (&old, &new)) in layout.shape.iter().zip(&shape[leading..]).enumerate() {
                if old == new {
                    strides[leading + j] = layout.strides[j];
                } else if old != 1 {
                    return Err(Fault::InvalidBroadcast);
                }
            }
            layout.shape = shape.clone();
            layout.strides = strides;
            layout.permission = Permission::ReadOnly;
        }
        Op::Reshape(shape) => reshape(layout, shape)?,
        Op::Materialize(storage) => {
            layout.storage = *storage;
            layout.capacity = cardinality(&layout.shape)?;
            layout.permission = Permission::Exclusive;
            layout.origin = 0;
            layout.strides.fill(0);
            if layout.capacity != 0 {
                let mut suffix = 1_usize;
                for (&n, stride) in layout.shape.iter().zip(&mut layout.strides).rev() {
                    if n > 1 {
                        *stride = i64::try_from(suffix).map_err(|_| Fault::Overflow)?;
                        suffix *= n;
                    }
                }
            }
        }
        Op::RequireWritable => {
            if layout.permission != Permission::Exclusive {
                return Err(Fault::WriteDenied);
            }
        }
    }
    Ok(())
}

pub fn address(layout: &Layout, index: &[usize]) -> Result<Address, Fault> {
    if index.len() != layout.strides.len() {
        return Err(Fault::RankMismatch);
    }
    ordinal(&layout.shape, index)?;
    let offset = index.iter().zip(&layout.strides).try_fold(
        i128::from(layout.origin),
        |acc, (&i, &s)| {
            acc.checked_add(i as i128 * i128::from(s))
                .ok_or(Fault::Overflow)
        },
    )?;
    let offset = usize::try_from(offset).map_err(|_| Fault::Bounds)?;
    if offset >= layout.capacity || offset as i128 > i128::from(i64::MAX) {
        return Err(Fault::Bounds);
    }
    Ok(Address {
        storage: layout.storage,
        offset,
    })
}

fn ordinal(shape: &[usize], index: &[usize]) -> Result<usize, Fault> {
    if shape.len() != index.len() {
        return Err(Fault::RankMismatch);
    }
    shape
        .iter()
        .zip(index)
        .try_fold(0_usize, |ordinal, (&n, &i)| {
            if i >= n {
                return Err(Fault::Bounds);
            }
            ordinal
                .checked_mul(n)
                .and_then(|v| v.checked_add(i))
                .ok_or(Fault::Overflow)
        })
}

pub(crate) fn coordinate(shape: &[usize], mut ordinal: usize) -> Vec<usize> {
    let mut index = vec![0; shape.len()];
    for (&n, i) in shape.iter().zip(&mut index).rev() {
        *i = ordinal % n;
        ordinal /= n;
    }
    index
}
