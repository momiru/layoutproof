//! Bounded interpreter that moves cells by logical indexing, never lowering strides.

use crate::ir::{Address, Fault, Layout, Op, Permission, ValueBits};
use std::collections::HashSet;

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
    pub storage: crate::ir::StorageId,
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
    mut budget: usize,
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
        permission: layout.permission,
        storage: layout.storage,
    };
    let mut seen = HashSet::new();
    visit(&obs.shape, |index| {
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
        let &value = storage.get(offset).ok_or(Fault::Bounds)?;
        if layout.permission == Permission::Exclusive && !seen.insert(offset) {
            return Err(Fault::WriteOverlap);
        }
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
    let mut used_storage = HashSet::from([obs.storage]);
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
