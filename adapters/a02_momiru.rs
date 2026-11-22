//! Development checks against the independently implemented Momiru engines.

use crate::ir::{cardinality, Fault, Layout, Op, Permission, Status, StorageId, ValueBits};
use crate::{lower, oracle};
use momiru::tensor::{Tensor as GeneralTensor, TensorView, TensorViewMut};
use momiru::tensor_light::{
    Rank, SignedSliceSpec, SignedTensorView, SignedTensorViewMut, Tensor as LightTensor, ValidRank,
};
use std::num::NonZeroIsize;

pub const REVISION: &str = "eba302d3c61313444b56303c8816f74a3aaf9679";

#[derive(Clone, Copy, Debug)]
pub enum Engine {
    General,
    Light,
}

#[derive(Debug)]
pub enum EngineError {
    /// The interpreter rejected or exhausted its budget before native execution.
    OraclePreflight(Fault),
    Model(Fault),
    General(momiru::tensor::TensorError),
    Light(momiru::tensor_light::TensorError),
    UnsupportedRank(usize),
    UnsupportedOperation,
    Unrepresentable,
    MaterializationAliasedInput,
    MaterializationExtent {
        expected: usize,
        observed: usize,
    },
    ShapeMismatch,
    AddressMismatch(Vec<usize>),
    ValueMismatch(Vec<usize>),
}

impl EngineError {
    pub fn status(&self) -> Status {
        match self {
            Self::UnsupportedRank(_)
            | Self::UnsupportedOperation
            | Self::Unrepresentable
            | Self::General(momiru::tensor::TensorError::NonContiguous)
            | Self::Light(momiru::tensor_light::TensorError::NonContiguous) => Status::Unsupported,
            Self::OraclePreflight(Fault::WorkLimit) | Self::Model(Fault::WorkLimit) => {
                Status::Unknown
            }
            Self::OraclePreflight(_)
            | Self::Model(_)
            | Self::General(momiru::tensor::TensorError::OverlappingMutableView)
            | Self::Light(momiru::tensor_light::TensorError::OverlappingMutableView) => {
                Status::Rejected
            }
            _ => Status::Fail,
        }
    }
}

macro_rules! light_rank {
    ($rank:expr, $r:ident => $operation:expr) => {{
        match $rank {
            0 => {
                const $r: usize = 0;
                $operation
            }
            1 => {
                const $r: usize = 1;
                $operation
            }
            2 => {
                const $r: usize = 2;
                $operation
            }
            3 => {
                const $r: usize = 3;
                $operation
            }
            4 => {
                const $r: usize = 4;
                $operation
            }
            5 => {
                const $r: usize = 5;
                $operation
            }
            6 => {
                const $r: usize = 6;
                $operation
            }
            7 => {
                const $r: usize = 7;
                $operation
            }
            8 => {
                const $r: usize = 8;
                $operation
            }
            rank => Err(EngineError::UnsupportedRank(rank)),
        }
    }};
}

#[derive(Debug, Eq, PartialEq)]
pub struct NativeObservation {
    pub shape: Vec<usize>,
    pub offsets: Vec<usize>,
    pub values: Vec<ValueBits>,
}

fn signed_metadata(layout: &Layout, len: usize) -> Result<(Vec<isize>, isize), EngineError> {
    if layout.capacity != len {
        return Err(EngineError::Model(Fault::StorageExtent));
    }
    let strides = layout
        .strides
        .iter()
        .copied()
        .map(isize::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| EngineError::Unrepresentable)?;
    let origin = isize::try_from(layout.origin).map_err(|_| EngineError::Unrepresentable)?;
    Ok((strides, origin))
}

fn fixed<T: Copy, const R: usize>(values: &[T]) -> Result<[T; R], EngineError> {
    values
        .try_into()
        .map_err(|_| EngineError::Model(Fault::RankMismatch))
}

fn observe<'a>(
    shape: &[usize],
    backing: &'a [f32],
    budget: usize,
    mut read: impl FnMut(&[usize]) -> Result<(usize, &'a f32), EngineError>,
) -> Result<NativeObservation, EngineError> {
    let count = cardinality(shape).map_err(EngineError::Model)?;
    if count
        .max(1)
        .checked_mul(shape.len().max(1))
        .is_none_or(|n| n > budget)
    {
        return Err(EngineError::Model(Fault::WorkLimit));
    }
    let (mut offsets, mut values) = (Vec::with_capacity(count), Vec::with_capacity(count));
    for flat in 0..count {
        let index = lower::coordinate(shape, flat);
        let (offset, value) = read(&index)?;
        if backing
            .get(offset)
            .is_none_or(|original| !std::ptr::eq(original, value))
        {
            return Err(EngineError::AddressMismatch(index));
        }
        offsets.push(offset);
        values.push(value.to_bits());
    }
    Ok(NativeObservation {
        shape: shape.to_vec(),
        offsets,
        values,
    })
}

fn general(
    mut view: TensorView<'_, f32>,
    backing: &[f32],
    ops: &[Op],
    budget: usize,
) -> Result<NativeObservation, EngineError> {
    for op in ops {
        let result = match op {
            Op::Flip(axis) => view.flip(*axis),
            Op::Permute(axes) => view.permute(axes),
            Op::Transpose { first, second } => {
                let mut axes: Vec<_> = (0..view.layout().ndim()).collect();
                if *first >= axes.len() || *second >= axes.len() {
                    return Err(EngineError::Model(Fault::AxisOutOfRange));
                }
                axes.swap(*first, *second);
                view.permute(axes)
            }
            Op::Slice {
                axis,
                start,
                count,
                step,
            } => {
                let step = isize::try_from(*step).map_err(|_| EngineError::Unrepresentable)?;
                let step =
                    NonZeroIsize::new(step).ok_or(EngineError::Model(Fault::InvalidSlice))?;
                view.slice_axis(*axis, *start, *count, step)
            }
            Op::Expand(_) | Op::Squeeze(_) => {
                // General's borrowed API has reshape but no signed unsqueeze/squeeze.
                view.reshape_view(reshape_singleton(view.layout().dims(), op)?)
            }
            Op::Reshape(shape) => view.reshape_view(shape),
            Op::Broadcast(shape) => view.broadcast_to(shape),
            Op::Materialize(_) | Op::RequireWritable => {
                return Err(EngineError::UnsupportedOperation)
            }
        };
        view = result.map_err(EngineError::General)?;
    }
    observe(view.layout().dims(), backing, budget, |index| {
        let offset = view
            .layout()
            .checked_offset(index)
            .map_err(EngineError::General)?;
        let value = view.get(index).map_err(EngineError::General)?;
        Ok((offset, value))
    })
}

fn reshape_singleton(shape: &[usize], op: &Op) -> Result<Vec<usize>, EngineError> {
    let mut shape = shape.to_vec();
    match op {
        Op::Expand(axis) if *axis <= shape.len() => shape.insert(*axis, 1),
        Op::Squeeze(axis) if shape.get(*axis) == Some(&1) => {
            shape.remove(*axis);
        }
        Op::Squeeze(axis) if *axis < shape.len() => {
            return Err(EngineError::Model(Fault::InvalidSqueeze))
        }
        _ => return Err(EngineError::Model(Fault::AxisOutOfRange)),
    }
    Ok(shape)
}

/// Translate checked (start, count, step) to the Light engine's half-open
/// convention without clamping away an invalid input.
fn light_slice(
    n: usize,
    start: usize,
    count: usize,
    step: i64,
) -> Result<SignedSliceSpec, EngineError> {
    let invalid = EngineError::Model(Fault::InvalidSlice);
    if step == 0 || start > n {
        return Err(invalid);
    }
    if count == 0 {
        // Equal native endpoints encode the empty domain without converting
        // the original unsigned start, which has no reachable coordinate.
        return Ok(SignedSliceSpec::new(Some(0), Some(0), 1));
    }
    let step = isize::try_from(step).map_err(|_| EngineError::Unrepresentable)?;
    let last = (count as i128 - 1)
        .checked_mul(step as i128)
        .and_then(|delta| delta.checked_add(start as i128))
        .and_then(|last| usize::try_from(last).ok())
        .filter(|&last| last < n && start < n)
        .ok_or(invalid)?;
    let end = if step < 0 {
        last.checked_sub(1)
    } else if last + 1 == n {
        None
    } else {
        Some(last + 1)
    };
    let endpoint = |coordinate: usize| {
        // Light accepts negative endpoints relative to the axis length.
        // Prefer the absolute form when both encodings fit.
        isize::try_from(coordinate)
            .or_else(|_| isize::try_from(coordinate as i128 - n as i128))
            .map_err(|_| EngineError::Unrepresentable)
    };
    Ok(SignedSliceSpec::new(
        Some(endpoint(start)?),
        end.map(endpoint).transpose()?,
        step,
    ))
}

macro_rules! light_view {
    ($(($variant:ident, $rank:literal)),+ $(,)?) => {
        enum LightView<'a> {
            $($variant(SignedTensorView<'a, $rank>)),+
        }

        $(impl<'a> From<SignedTensorView<'a, $rank>> for LightView<'a> {
            fn from(view: SignedTensorView<'a, $rank>) -> Self { Self::$variant(view) }
        })+

        impl<'a> LightView<'a> {
            fn apply(self, op: &Op) -> Result<Self, EngineError> {
                match self {
                    $(Self::$variant(view) => apply_light(view, op)),+
                }
            }

            fn observe(&self, backing: &'a [f32], budget: usize) -> Result<NativeObservation, EngineError> {
                match self {
                    $(Self::$variant(view) => observe_light(view, backing, budget)),+
                }
            }
        }
    };
}

light_view!(
    (R0, 0),
    (R1, 1),
    (R2, 2),
    (R3, 3),
    (R4, 4),
    (R5, 5),
    (R6, 6),
    (R7, 7),
    (R8, 8)
);

fn apply_light<'a, const R: usize>(
    view: SignedTensorView<'a, R>,
    op: &Op,
) -> Result<LightView<'a>, EngineError>
where
    Rank<R>: ValidRank,
    LightView<'a>: From<SignedTensorView<'a, R>>,
{
    let changed_rank = match op {
        Op::Expand(_) | Op::Squeeze(_) => Some(reshape_singleton(view.dims(), op)?.len()),
        Op::Broadcast(shape) | Op::Reshape(shape) => Some(shape.len()),
        _ => None,
    };
    if let Some(rank) = changed_rank {
        return light_rank!(rank, OUT => {
            let result = match op {
                Op::Expand(axis) => view.unsqueeze::<OUT>(*axis),
                Op::Squeeze(axis) => view.squeeze_axis::<OUT>(*axis),
                Op::Broadcast(shape) => view.broadcast_to::<OUT>(fixed(shape)?),
                Op::Reshape(shape) => view.reshape_view::<OUT>(fixed(shape)?),
                _ => unreachable!("rank-directed operators were matched above"),
            };
            result.map(<LightView<'a> as From<SignedTensorView<'a, OUT>>>::from).map_err(EngineError::Light)
        });
    }
    let result = match op {
        Op::Transpose { first, second } => view.transpose(*first, *second),
        Op::Permute(axes) => view.permute(fixed(axes)?),
        Op::Flip(axis) => view.flip(*axis),
        Op::Slice {
            axis,
            start,
            count,
            step,
        } => {
            let &n = view
                .dims()
                .get(*axis)
                .ok_or(EngineError::Model(Fault::AxisOutOfRange))?;
            view.slice_axis(*axis, light_slice(n, *start, *count, *step)?)
        }
        _ => return Err(EngineError::UnsupportedOperation),
    };
    result.map(LightView::from).map_err(EngineError::Light)
}

fn observe_light<'a, const R: usize>(
    view: &SignedTensorView<'a, R>,
    backing: &'a [f32],
    budget: usize,
) -> Result<NativeObservation, EngineError>
where
    Rank<R>: ValidRank,
{
    observe(view.dims(), backing, budget, |index| {
        let index = fixed(index)?;
        let offset = view
            .layout()
            .checked_offset(index)
            .map_err(EngineError::Light)?;
        let value = view.try_get_ref(index).map_err(EngineError::Light)?;
        Ok((offset, value))
    })
}

/// Apply the operations to the selected engine with readonly storage and
/// compare every observed address and exact FP32 payload with the interpreter.
pub fn check_read_only(
    engine: Engine,
    layout: &Layout,
    data: &[ValueBits],
    ops: &[Op],
    budget: usize,
) -> Result<NativeObservation, EngineError> {
    require_borrowed_operations(ops)?;
    let layout = Layout {
        permission: Permission::ReadOnly,
        ..layout.clone()
    };
    let expected =
        oracle::interpret(&layout, data, ops, budget).map_err(EngineError::OraclePreflight)?;
    let backing: Vec<f32> = data.iter().copied().map(f32::from_bits).collect();
    let observed = observe_read_only(engine, &layout, &backing, ops, budget)?;
    compare_observation(&expected, &observed)?;
    Ok(observed)
}

fn compare_observation(
    expected: &oracle::Observation,
    observed: &NativeObservation,
) -> Result<(), EngineError> {
    if expected.shape != observed.shape {
        return Err(EngineError::ShapeMismatch);
    }
    for (flat, cell) in expected.cells.iter().enumerate() {
        if observed.offsets[flat] != cell.address.offset {
            return Err(EngineError::AddressMismatch(lower::coordinate(
                &observed.shape,
                flat,
            )));
        }
        if observed.values[flat] != cell.value {
            return Err(EngineError::ValueMismatch(lower::coordinate(
                &observed.shape,
                flat,
            )));
        }
    }
    Ok(())
}

/// Gather a native shared view into a new owned tensor of the same engine.
/// The adapter supplies the logical-order gather and each engine constructs
/// its own dense storage. Only a single terminal materialization is supported.
pub fn check_materialization(
    engine: Engine,
    layout: &Layout,
    backing: &[f32],
    view_ops: &[Op],
    destination: StorageId,
    budget: usize,
) -> Result<NativeObservation, EngineError> {
    require_borrowed_operations(view_ops)?;
    let layout = Layout {
        permission: Permission::ReadOnly,
        ..layout.clone()
    };
    let mut program = view_ops.to_vec();
    program.push(Op::Materialize(destination));
    let bits: Vec<_> = backing.iter().map(|value| value.to_bits()).collect();
    let expected = oracle::interpret(&layout, &bits, &program, budget)
        .map_err(EngineError::OraclePreflight)?;
    let gathered = observe_read_only(engine, &layout, backing, view_ops, budget)?;
    let values = gathered.values.into_iter().map(f32::from_bits).collect();
    let observed = match engine {
        Engine::General => {
            let owned =
                GeneralTensor::from_vec(&gathered.shape, values).map_err(EngineError::General)?;
            check_owned_backing(backing, owned.as_slice(), expected.cells.len())?;
            general(owned.view(), owned.as_slice(), &[], budget)?
        }
        Engine::Light => light_rank!(gathered.shape.len(), R => {
            let owned = LightTensor::<R>::try_from_vec(fixed(&gathered.shape)?, values)
                .map_err(EngineError::Light)?;
            check_owned_backing(backing, owned.as_slice(), expected.cells.len())?;
            let view = owned.signed_view().map_err(EngineError::Light)?;
            observe_light(&view, owned.as_slice(), budget)
        })?,
    };
    compare_observation(&expected, &observed)?;
    Ok(observed)
}

fn check_owned_backing(
    source: &[f32],
    owned: &[f32],
    expected_extent: usize,
) -> Result<(), EngineError> {
    if owned.len() != expected_extent {
        return Err(EngineError::MaterializationExtent {
            expected: expected_extent,
            observed: owned.len(),
        });
    }
    if !source.is_empty() && !owned.is_empty() {
        let source = source.as_ptr_range();
        let owned = owned.as_ptr_range();
        if source.start.addr() < owned.end.addr() && owned.start.addr() < source.end.addr() {
            return Err(EngineError::MaterializationAliasedInput);
        }
    }
    Ok(())
}

fn require_borrowed_operations(ops: &[Op]) -> Result<(), EngineError> {
    if ops
        .iter()
        .any(|op| matches!(op, Op::RequireWritable | Op::Materialize(_)))
    {
        return Err(EngineError::UnsupportedOperation);
    }
    Ok(())
}

/// Run a shared borrow on the selected engine and collect its physical
/// observations. The timed diagnostic checks these against an independently
/// validated observation outside the measured interval.
pub fn observe_read_only(
    engine: Engine,
    layout: &Layout,
    backing: &[f32],
    ops: &[Op],
    budget: usize,
) -> Result<NativeObservation, EngineError> {
    require_borrowed_operations(ops)?;
    let (strides, origin) = signed_metadata(layout, backing.len())?;
    match engine {
        Engine::General => {
            let view = TensorView::new(&layout.shape, &strides, origin, backing)
                .map_err(EngineError::General)?;
            general(view, backing, ops, budget)
        }
        Engine::Light => light_rank!(layout.shape.len(), R => {
            let view = SignedTensorView::<R>::new(fixed(&layout.shape)?, fixed(&strides)?, origin, backing)
                .map_err(EngineError::Light)?;
            let mut view = LightView::from(view);
            for op in ops {
                view = view.apply(op)?;
            }
            view.observe(backing, budget)
        }),
    }
}

/// Exercise the engine's native exclusive-view admission and indexed write.
pub fn write_once(
    engine: Engine,
    layout: &Layout,
    backing: &mut [f32],
    index: &[usize],
    value: ValueBits,
) -> Result<(), EngineError> {
    if layout.permission != Permission::Exclusive {
        return Err(EngineError::Model(Fault::WriteDenied));
    }
    let (strides, origin) = signed_metadata(layout, backing.len())?;
    match engine {
        Engine::General => {
            let mut view = TensorViewMut::new(&layout.shape, &strides, origin, backing)
                .map_err(EngineError::General)?;
            *view.get_mut(index).map_err(EngineError::General)? = f32::from_bits(value);
            Ok(())
        }
        Engine::Light => light_rank!(layout.shape.len(), R => {
            let mut view = SignedTensorViewMut::<R>::new(fixed(&layout.shape)?, fixed(&strides)?, origin, backing)
                .map_err(EngineError::Light)?;
            view.set(fixed(index)?, f32::from_bits(value)).map_err(EngineError::Light)
        }),
    }
}
