// SPDX-FileCopyrightText: 2026 Morteza Taleblou
// SPDX-FileCopyrightText: 2026 Mitra Daneshmand
// SPDX-License-Identifier: MIT
//
// ==============================================================================
// MOMIRU LAYOUTPROOF | A02 RESEARCH SOURCE
// ==============================================================================
//
// Project:      Momiru LayoutProof (article A02)
// Component:    Native paper model, verifier and executable
// Module:       momiru_layoutproof::lower
// Source:       src/lower.rs
// Language:     Rust
// Architecture: Independent paper verifier with pinned native Momiru adapters
//
// ------------------------------------------------------------------------------
// ENGINEERING AND RESEARCH CONTRACT
// ------------------------------------------------------------------------------
// Preserve checked tensor-layout semantics, exact address/value obligations,
// explicit alias and write permissions, and honest unsupported outcomes.
// Keep bounded diagnostics separate from verified scientific evidence.
// Respect the pinned Momiru API and provide regression coverage for edits.
//
// Authors:      Morteza Taleblou; Mitra Daneshmand
// Organization: https://github.com/momiru
// Repository:   https://github.com/momiru/layoutproof
// License:      MIT (see LICENSE)
// ==============================================================================

//! Checked affine lowering. The interpreter uses coordinate substitution instead.

use crate::ir::{cardinality, Address, Fault, Layout, Op, Permission, StorageId};
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct Lowered {
    pub(crate) output: Layout,
    pub(crate) allocations: HashMap<StorageId, usize>,
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
    compile_with_peers(layout, ops, &[], alias_budget)
}

/// Lower in the presence of declared live regions. An exclusive lease must
/// be disjoint from every other live region in the same allocation.
pub fn compile_with_peers(
    layout: &Layout,
    ops: &[Op],
    peers: &[Layout],
    alias_budget: usize,
) -> Result<Lowered, Fault> {
    compile_in_context(layout, ops, peers, alias_budget, true)
}

pub(crate) fn compile_in_context(
    layout: &Layout,
    ops: &[Op],
    peers: &[Layout],
    mut alias_budget: usize,
    check_peer_overlap: bool,
) -> Result<Lowered, Fault> {
    let mut live: HashMap<StorageId, Vec<LiveRegion<'_>>> = HashMap::new();
    for region in std::iter::once(layout).chain(peers) {
        let group = live.entry(region.storage).or_default();
        if group
            .first()
            .is_some_and(|first| first.layout.capacity != region.capacity)
        {
            return Err(Fault::StorageExtent);
        }
        let (size, span) = validate(region)?;
        if region.permission == Permission::Exclusive {
            unique(region, size, &mut alias_budget)?;
        }
        let mut current = LiveRegion {
            layout: region,
            size,
            span,
            offsets: None,
        };
        if check_peer_overlap {
            for previous in group.iter_mut() {
                previous.check_disjoint(&mut current, &mut alias_budget)?;
            }
        }
        group.push(current);
    }
    let mut out = layout.clone();
    canonicalize(&mut out);
    let mut stages = vec![out.shape.clone()];
    let mut reserved: HashSet<_> = live.into_keys().collect();
    let mut allocations = HashMap::new();
    for op in ops {
        if let Op::Materialize(storage) = op {
            if !reserved.insert(*storage) {
                return Err(Fault::StorageReuse);
            }
        }
        apply(&mut out, op)?;
        canonicalize(&mut out);
        validate(&out)?;
        if let Op::Materialize(storage) = op {
            allocations.insert(*storage, out.capacity);
        }
        stages.push(out.shape.clone());
    }
    Ok(Lowered {
        output: out,
        allocations,
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

fn charge_alias(layout: &Layout, size: usize, remaining: &mut usize) -> Result<(), Fault> {
    let cost = size
        .checked_mul(layout.shape.len().max(1))
        .ok_or(Fault::WorkLimit)?;
    *remaining = remaining.checked_sub(cost).ok_or(Fault::WorkLimit)?;
    Ok(())
}

type Span = Option<(usize, usize)>;

struct LiveRegion<'a> {
    layout: &'a Layout,
    size: usize,
    span: Span,
    offsets: Option<HashSet<usize>>,
}

impl LiveRegion<'_> {
    fn check_disjoint(&mut self, other: &mut Self, budget: &mut usize) -> Result<(), Fault> {
        *budget = budget.checked_sub(1).ok_or(Fault::WorkLimit)?;
        if self.layout.permission == Permission::ReadOnly
            && other.layout.permission == Permission::ReadOnly
        {
            return Ok(());
        }
        let (Some((start, end)), Some((other_start, other_end))) = (self.span, other.span) else {
            return Ok(());
        };
        if end < other_start || other_end < start {
            return Ok(());
        }
        let left = self.offsets(budget)?;
        let right = other.offsets(budget)?;
        *budget = budget
            .checked_sub(left.len().min(right.len()))
            .ok_or(Fault::WorkLimit)?;
        if !left.is_disjoint(right) {
            return Err(Fault::LivePeerOverlap);
        }
        Ok(())
    }

    fn offsets(&mut self, budget: &mut usize) -> Result<&HashSet<usize>, Fault> {
        if self.offsets.is_none() {
            charge_alias(self.layout, self.size, budget)?;
            self.offsets = Some(
                (0..self.size)
                    .map(|flat| {
                        address(self.layout, &coordinate(&self.layout.shape, flat))
                            .map(|point| point.offset)
                    })
                    .collect::<Result<_, _>>()?,
            );
        }
        Ok(self.offsets.as_ref().expect("offsets were collected"))
    }
}

fn validate(layout: &Layout) -> Result<(usize, Span), Fault> {
    if layout.shape.len() != layout.strides.len() {
        return Err(Fault::RankMismatch);
    }
    let size = cardinality(&layout.shape)?;
    if size == 0 {
        if (0..=layout.capacity as i128).contains(&i128::from(layout.origin)) {
            return Ok((0, None));
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
    Ok((size, Some((low as usize, high as usize))))
}

fn unique(layout: &Layout, size: usize, budget: &mut usize) -> Result<(), Fault> {
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
    charge_alias(layout, size, budget)?;
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
