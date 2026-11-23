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
// Module:       momiru_layoutproof::oracle
// Source:       src/oracle.rs
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

//! Bounded interpreter that moves cells by logical indexing, never lowering strides.

use crate::ir::{Address, Fault, Layout, Op, Permission, StorageId, ValueBits};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cell {
    pub input_ordinal: usize,
    pub address: Address,
    pub value: ValueBits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub shape: Vec<usize>,
    pub cells: Vec<Cell>,
    pub storage: StorageId,
    pub(crate) allocations: HashMap<StorageId, usize>,
    pub permission: Permission,
}

/// Independent row-major enumeration. A scalar visits one coordinate.
pub(crate) fn visit(
    shape: &[usize],
    mut f: impl FnMut(&[usize]) -> Result<(), Fault>,
) -> Result<(), Fault> {
    if shape.contains(&0) {
        return Ok(());
    }
    let mut index = vec![0; shape.len()];
    loop {
        f(&index)?;
        let mut advance = false;
        for (i, &n) in index.iter_mut().zip(shape).rev() {
            if *i < n - 1 {
                *i += 1;
                advance = true;
                break;
            }
            *i = 0;
        }
        if !advance {
            return Ok(());
        }
    }
}

fn volume(shape: &[usize]) -> Result<usize, Fault> {
    if shape.contains(&0) {
        return Ok(0);
    }
    let mut total = 1_u128;
    for &extent in shape {
        total = total.checked_mul(extent as u128).ok_or(Fault::Overflow)?;
    }
    usize::try_from(total).map_err(|_| Fault::Overflow)
}

fn charge(shape: &[usize], remaining: &mut usize) -> Result<(), Fault> {
    // Charge metadata even for an empty tensor and count coordinate work.
    let extent = volume(shape)?;
    let rank_cost = shape.len().max(1);
    let work = extent
        .max(1)
        .checked_mul(rank_cost)
        .ok_or(Fault::WorkLimit)?;
    *remaining = remaining.checked_sub(work).ok_or(Fault::WorkLimit)?;
    Ok(())
}

pub fn interpret(
    layout: &Layout,
    storage: &[ValueBits],
    ops: &[Op],
    budget: usize,
) -> Result<Observation, Fault> {
    interpret_with_peers(layout, storage, ops, &[], budget)
}

pub fn interpret_with_peers(
    layout: &Layout,
    storage: &[ValueBits],
    ops: &[Op],
    peers: &[Layout],
    budget: usize,
) -> Result<Observation, Fault> {
    interpret_in_context(layout, storage, ops, peers, budget, true)
}

pub(crate) fn interpret_in_context(
    layout: &Layout,
    storage: &[ValueBits],
    ops: &[Op],
    peers: &[Layout],
    mut budget: usize,
    check_peer_overlap: bool,
) -> Result<Observation, Fault> {
    if layout.capacity != storage.len() {
        return Err(Fault::StorageExtent);
    }
    if layout.shape.len() != layout.strides.len() {
        return Err(Fault::RankMismatch);
    }
    charge(&layout.shape, &mut budget)?;
    if layout.shape.contains(&0)
        && !(0..=layout.capacity as i128).contains(&i128::from(layout.origin))
    {
        return Err(Fault::Bounds);
    }
    let mut obs = Observation {
        shape: layout.shape.clone(),
        cells: Vec::new(),
        allocations: HashMap::new(),
        permission: layout.permission,
        storage: layout.storage,
    };
    visit(&obs.shape, |index| {
        let offset = input_offset(layout, index)?;
        let &value = storage.get(offset).ok_or(Fault::Bounds)?;
        obs.cells.push(Cell {
            input_ordinal: obs.cells.len(),
            address: Address {
                storage: layout.storage,
                offset,
            },
            value,
        });
        Ok(())
    })?;
    let offsets: HashSet<_> = obs.cells.iter().map(|cell| cell.address.offset).collect();
    if layout.permission == Permission::Exclusive && offsets.len() != obs.cells.len() {
        return Err(Fault::WriteOverlap);
    }
    let mut live = HashMap::new();
    let mut subject = LiveOffsets::new(layout.capacity);
    subject.record(offsets, layout.permission, check_peer_overlap);
    live.insert(layout.storage, subject);
    for peer in peers {
        let group = live
            .entry(peer.storage)
            .or_insert_with(|| LiveOffsets::new(peer.capacity));
        if group.capacity != peer.capacity {
            return Err(Fault::StorageExtent);
        }
        if peer.shape.len() != peer.strides.len() {
            return Err(Fault::RankMismatch);
        }
        charge(&peer.shape, &mut budget)?;
        if peer.shape.contains(&0)
            && !(0..=peer.capacity as i128).contains(&i128::from(peer.origin))
        {
            return Err(Fault::Bounds);
        }
        let mut peer_offsets = HashSet::new();
        let mut duplicates = false;
        visit(&peer.shape, |index| {
            duplicates |= !peer_offsets.insert(input_offset(peer, index)?);
            Ok(())
        })?;
        if peer.permission == Permission::Exclusive && duplicates {
            return Err(Fault::WriteOverlap);
        }
        if check_peer_overlap && group.conflicts(&peer_offsets, peer.permission) {
            return Err(Fault::LivePeerOverlap);
        }
        group.record(peer_offsets, peer.permission, check_peer_overlap);
    }
    let mut used_storage: HashSet<_> = live.into_keys().collect();
    for op in ops {
        if let Op::Materialize(storage) = op {
            if !used_storage.insert(*storage) {
                return Err(Fault::StorageReuse);
            }
        }
        apply(&mut obs, op, &mut budget)?;
    }
    Ok(obs)
}

// Build offset unions per allocation, independently of the lowerer's
// pairwise span exclusions and comparisons.
struct LiveOffsets {
    capacity: usize,
    any: HashSet<usize>,
    writers: HashSet<usize>,
}

impl LiveOffsets {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            any: HashSet::new(),
            writers: HashSet::new(),
        }
    }

    fn conflicts(&self, offsets: &HashSet<usize>, permission: Permission) -> bool {
        let occupied = match permission {
            Permission::Exclusive => &self.any,
            Permission::ReadOnly => &self.writers,
        };
        !offsets.is_disjoint(occupied)
    }

    fn record(&mut self, offsets: HashSet<usize>, permission: Permission, enabled: bool) {
        if enabled {
            if permission == Permission::Exclusive {
                self.writers.extend(offsets.iter().copied());
            }
            self.any.extend(offsets);
        }
    }
}

fn input_offset(layout: &Layout, index: &[usize]) -> Result<usize, Fault> {
    let mut offset = i128::from(layout.origin);
    for (axis, &i) in index.iter().enumerate() {
        offset = offset
            .checked_add(i as i128 * i128::from(layout.strides[axis]))
            .ok_or(Fault::Overflow)?;
    }
    let offset = i64::try_from(offset)
        .ok()
        .and_then(|o| usize::try_from(o).ok())
        .ok_or(Fault::Bounds)?;
    if offset >= layout.capacity {
        return Err(Fault::Bounds);
    }
    Ok(offset)
}

fn dim(shape: &[usize], axis: usize) -> Result<usize, Fault> {
    shape.get(axis).copied().ok_or(Fault::AxisOutOfRange)
}

fn from_slice(start: usize, step: i64, position: usize) -> Result<usize, Fault> {
    let delta = (position as u128) * u128::from(step.unsigned_abs());
    let max = usize::MAX as u128;
    if step < 0 {
        return (start as u128)
            .checked_sub(delta)
            .map(|p| p as usize)
            .ok_or(Fault::InvalidSlice);
    }
    let p = start as u128 + delta;
    if p > max {
        return Err(Fault::InvalidSlice);
    }
    Ok(p as usize)
}

fn apply(obs: &mut Observation, op: &Op, remaining: &mut usize) -> Result<(), Fault> {
    let old = &obs.shape;
    let mut shape = old.clone();
    match op {
        Op::Transpose { first, second } => {
            dim(old, *first)?;
            dim(old, *second)?;
            shape.swap(*first, *second);
        }
        Op::Permute(axes) => {
            if axes.len() != old.len()
                || axes.iter().any(|&a| a >= old.len())
                || axes.iter().collect::<HashSet<_>>().len() != axes.len()
            {
                return Err(Fault::InvalidPermutation);
            }
            shape = axes.iter().map(|&a| old[a]).collect();
        }
        Op::Slice {
            axis,
            start,
            count,
            step,
        } => {
            let n = dim(old, *axis)?;
            if *step == 0
                || *start > n
                || (*count > 0 && (*start == n || from_slice(*start, *step, *count - 1)? >= n))
            {
                return Err(Fault::InvalidSlice);
            }
            shape[*axis] = *count;
        }
        Op::Flip(a) => {
            dim(old, *a)?;
        }
        Op::Expand(a) => {
            if *a > old.len() {
                return Err(Fault::AxisOutOfRange);
            }
            shape.insert(*a, 1);
        }
        Op::Squeeze(a) => {
            if dim(old, *a)? != 1 {
                return Err(Fault::InvalidSqueeze);
            }
            shape.remove(*a);
        }
        Op::Broadcast(new) => {
            if new.len() < old.len()
                || old
                    .iter()
                    .rev()
                    .zip(new.iter().rev())
                    .any(|(&from, &to)| from != 1 && from != to)
            {
                return Err(Fault::InvalidBroadcast);
            }
            shape = new.clone();
            obs.permission = Permission::ReadOnly;
        }
        Op::Reshape(new) => {
            if volume(new)? != obs.cells.len() {
                return Err(Fault::ShapeMismatch);
            }
            charge(new, remaining)?;
            if new != old && obs.cells.len() > 1 {
                let offset = |i: usize| obs.cells[i].address.offset as i128;
                let origin = offset(0);
                let mut basis = vec![0; new.len()];
                let mut after = 1;
                for (axis, &n) in new.iter().enumerate().rev() {
                    if n > 1 {
                        basis[axis] = offset(after) - origin;
                        after *= n;
                    }
                }
                let mut flat = 0;
                visit(new, |index| {
                    let predicted =
                        index.iter().zip(&basis).try_fold(origin, |sum, (&i, &s)| {
                            sum.checked_add(i as i128 * s).ok_or(Fault::RequiresCopy)
                        })?;
                    if predicted != offset(flat) {
                        return Err(Fault::RequiresCopy);
                    }
                    flat += 1;
                    Ok(())
                })?;
            }
            obs.shape = new.clone();
            return Ok(());
        }
        Op::Materialize(storage) => {
            charge(old, remaining)?;
            obs.storage = *storage;
            obs.allocations.insert(*storage, obs.cells.len());
            obs.permission = Permission::Exclusive;
            for (offset, cell) in obs.cells.iter_mut().enumerate() {
                cell.address = Address {
                    storage: *storage,
                    offset,
                };
            }
            return Ok(());
        }
        Op::RequireWritable => {
            charge(old, remaining)?;
            return if obs.permission == Permission::Exclusive {
                Ok(())
            } else {
                Err(Fault::WriteDenied)
            };
        }
    }
    charge(&shape, remaining)?;
    let inverse = if let Op::Permute(axes) = op {
        let mut inverse = vec![0; axes.len()];
        for (output, &input) in axes.iter().enumerate() {
            inverse[input] = output;
        }
        inverse
    } else {
        Vec::new()
    };
    let mut cells = Vec::new();
    visit(&shape, |index| {
        let mut source = index.to_vec();
        match op {
            Op::Transpose { first, second } => source.swap(*first, *second),
            Op::Permute(_) => source = inverse.iter().map(|&j| index[j]).collect(),
            Op::Slice {
                axis, start, step, ..
            } => source[*axis] = from_slice(*start, *step, index[*axis])?,
            Op::Flip(a) => source[*a] = old[*a] - (index[*a] + 1),
            Op::Expand(a) => {
                source.remove(*a);
            }
            Op::Squeeze(a) => source.insert(*a, 0),
            Op::Broadcast(_) => {
                source = old
                    .iter()
                    .zip(&index[index.len() - old.len()..])
                    .map(|(&n, &i)| if n == 1 { 0 } else { i })
                    .collect();
            }
            Op::Reshape(_) | Op::Materialize(_) | Op::RequireWritable => {
                unreachable!("handled above")
            }
        }
        let mut multiplier = 1;
        let mut position = 0;
        for (&n, &i) in old.iter().zip(&source).rev() {
            position += multiplier * i;
            multiplier *= n;
        }
        cells.push(obs.cells[position].clone());
        Ok(())
    })?;
    obs.shape = shape;
    obs.cells = cells;
    Ok(())
}
