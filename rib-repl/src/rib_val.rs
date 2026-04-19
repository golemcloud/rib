use std::convert::TryFrom;

use anyhow::{anyhow, bail, Context, Result};
use rib::wit_type::{TypeEnum, WitType};
use rib::{Value, ValueAndType};

/// A **portable, component-model-shaped** value for host calls and external integrations.
///
/// # Role
///
/// `RibVal` exists so **embedders** (Wasmtime CLI, custom hosts, tests) and **REPL plumbing**
/// can pass arguments and results across a small, stable API. Its variants mirror the usual
/// WebAssembly **component model** runtime shape (the same broad case structure as embedders like
/// Wasmtime use for component `Val`), so mapping to or from that host representation is mostly
/// structural recursion—not a parallel type system.
///
/// # Versus [`Value`] / [`ValueAndType`]
///
/// Inside Rib, execution uses [`Value`] together with [`WitType`] as [`ValueAndType`]. That pair
/// is the full interpreter representation. `RibVal` is **not** a replacement for that; it is the
/// **narrow hand-off** when leaving the interpreter to call into a host implementation of an
/// import. Use [`TryFrom`] from `&`[`ValueAndType`] and [`RibVal::try_to_value_and_type`] at those
/// edges when you still need full Rib values; use `RibVal` alone when the other side only speaks in
/// component-shaped values.
///
/// # Conversions
///
/// - Interpreter → `RibVal`: implement [`TryFrom`] for `&`[`ValueAndType`] (use [`RibVal::try_from`]).
/// - `RibVal` → interpreter: [`RibVal::try_to_value_and_type`] (needs the result [`WitType`]; we
///   cannot offer `TryInto<ValueAndType>` here without a second type parameter, and we cannot
///   implement foreign `TryFrom` targets for orphan-rule reasons).
#[derive(Debug, Clone, PartialEq)]
pub enum RibVal {
    Bool(bool),
    S8(i8),
    U8(u8),
    S16(i16),
    U16(u16),
    S32(i32),
    U32(u32),
    S64(i64),
    U64(u64),
    Float32(f32),
    Float64(f64),
    Char(char),
    String(String),
    List(Vec<RibVal>),
    Record(Vec<(String, RibVal)>),
    Tuple(Vec<RibVal>),
    Variant(String, Option<Box<RibVal>>),
    Enum(String),
    Option(Option<Box<RibVal>>),
    Result(Result<Option<Box<RibVal>>, Option<Box<RibVal>>>),
    Flags(Vec<String>),
    /// Resource handle (URI + id); embedders map to/from component resources.
    Handle {
        uri: String,
        resource_id: u64,
    },
}

impl TryFrom<&ValueAndType> for RibVal {
    type Error = anyhow::Error;

    /// Converts a Rib [`ValueAndType`] into a [`RibVal`] (e.g. before [`crate::ComponentFunctionInvoke::invoke`]).
    fn try_from(v: &ValueAndType) -> Result<Self, Self::Error> {
        try_value_to_rib_val(&v.value, &v.typ)
    }
}

fn try_value_to_rib_val(value: &Value, ty: &WitType) -> Result<RibVal> {
    use RibVal as R;
    use WitType as WT;

    Ok(match (ty, value) {
        (WT::Bool(_), Value::Bool(b)) => R::Bool(*b),
        (WT::S8(_), Value::S8(x)) => R::S8(*x),
        (WT::U8(_), Value::U8(x)) => R::U8(*x),
        (WT::S16(_), Value::S16(x)) => R::S16(*x),
        (WT::U16(_), Value::U16(x)) => R::U16(*x),
        (WT::S32(_), Value::S32(x)) => R::S32(*x),
        (WT::U32(_), Value::U32(x)) => R::U32(*x),
        (WT::S64(_), Value::S64(x)) => R::S64(*x),
        (WT::U64(_), Value::U64(x)) => R::U64(*x),
        (WT::F32(_), Value::F32(x)) => R::Float32(*x),
        (WT::F64(_), Value::F64(x)) => R::Float64(*x),
        (WT::Chr(_), Value::Char(c)) => R::Char(*c),
        (WT::Str(_), Value::String(s)) => R::String(s.clone()),
        (WT::List(l), Value::List(items)) => {
            let inner = &*l.inner;
            R::List(
                items
                    .iter()
                    .map(|item| try_value_to_rib_val(item, inner).with_context(|| "list element"))
                    .collect::<Result<_>>()?,
            )
        }
        (WT::Record(rec), Value::Record(vals)) => {
            if rec.fields.len() != vals.len() {
                bail!("record field count mismatch");
            }
            let pairs: Vec<(String, RibVal)> = rec
                .fields
                .iter()
                .zip(vals.iter())
                .map(|(f, v)| {
                    Ok((
                        f.name.clone(),
                        try_value_to_rib_val(v, &f.typ).with_context(|| f.name.clone())?,
                    ))
                })
                .collect::<Result<_>>()?;
            R::Record(pairs)
        }
        (WT::Tuple(tt), Value::Tuple(items)) => {
            if tt.items.len() != items.len() {
                bail!("tuple arity mismatch");
            }
            R::Tuple(
                tt.items
                    .iter()
                    .zip(items.iter())
                    .enumerate()
                    .map(|(i, (t, v))| {
                        try_value_to_rib_val(v, t).with_context(|| format!("tuple field {i}"))
                    })
                    .collect::<Result<_>>()?,
            )
        }
        (
            WT::Variant(var_ty),
            Value::Variant {
                case_idx,
                case_value,
            },
        ) => {
            let case = var_ty
                .cases
                .get(*case_idx as usize)
                .ok_or_else(|| anyhow!("invalid variant case index"))?;
            let name = case.name.clone();
            let payload = match (&case.typ, case_value) {
                (None, None) => None,
                (Some(inner), Some(b)) => Some(Box::new(try_value_to_rib_val(b, inner)?)),
                _ => bail!("variant payload mismatch"),
            };
            R::Variant(name, payload)
        }
        (WT::Enum(TypeEnum { cases, .. }), Value::Enum(idx)) => {
            let s = cases
                .get(*idx as usize)
                .cloned()
                .ok_or_else(|| anyhow!("invalid enum discriminant"))?;
            R::Enum(s)
        }
        (WT::Flags(ft), Value::Flags(bits)) => {
            let names: Vec<String> = ft
                .names
                .iter()
                .enumerate()
                .filter_map(|(i, n)| bits.get(i).copied().unwrap_or(false).then(|| n.clone()))
                .collect();
            R::Flags(names)
        }
        (WT::Option(ot), Value::Option(inner)) => {
            let mapped = match inner {
                None => None,
                Some(b) => Some(Box::new(try_value_to_rib_val(b, &ot.inner)?)),
            };
            R::Option(mapped)
        }
        (WT::Result(rt), Value::Result(inner)) => {
            let mapped = match inner {
                Ok(v) => Ok(match v {
                    None => None,
                    Some(b) => Some(Box::new(try_value_to_rib_val(
                        b,
                        rt.ok
                            .as_deref()
                            .ok_or_else(|| anyhow!("result ok type missing"))?,
                    )?)),
                }),
                Err(v) => Err(match v {
                    None => None,
                    Some(b) => Some(Box::new(try_value_to_rib_val(
                        b,
                        rt.err
                            .as_deref()
                            .ok_or_else(|| anyhow!("result err type missing"))?,
                    )?)),
                }),
            };
            R::Result(mapped)
        }
        (WT::Handle(_), Value::Handle { uri, resource_id }) => R::Handle {
            uri: uri.clone(),
            resource_id: *resource_id,
        },
        _ => bail!(
            "cannot convert Rib value to RibVal for type {:?}: {:?}",
            ty,
            value
        ),
    })
}

impl RibVal {
    /// Converts this value back into Rib’s [`ValueAndType`], using the call’s result [`WitType`]
    /// from the signature (e.g. after a host returns a [`RibVal`]).
    pub fn try_to_value_and_type(&self, ty: &WitType) -> Result<ValueAndType> {
        rib_val_to_value_and_type(self, ty)
    }
}

fn rib_val_to_value_and_type(rv: &RibVal, ty: &WitType) -> Result<ValueAndType> {
    use RibVal as R;
    use WitType as WT;

    let value = match (ty, rv) {
        (WT::Bool(_), R::Bool(b)) => Value::Bool(*b),
        (WT::S8(_), R::S8(x)) => Value::S8(*x),
        (WT::U8(_), R::U8(x)) => Value::U8(*x),
        (WT::S16(_), R::S16(x)) => Value::S16(*x),
        (WT::U16(_), R::U16(x)) => Value::U16(*x),
        (WT::S32(_), R::S32(x)) => Value::S32(*x),
        (WT::U32(_), R::U32(x)) => Value::U32(*x),
        (WT::S64(_), R::S64(x)) => Value::S64(*x),
        (WT::U64(_), R::U64(x)) => Value::U64(*x),
        (WT::F32(_), R::Float32(x)) => Value::F32(*x),
        (WT::F64(_), R::Float64(x)) => Value::F64(*x),
        (WT::Chr(_), R::Char(c)) => Value::Char(*c),
        (WT::Str(_), R::String(s)) => Value::String(s.clone()),
        (WT::List(l), R::List(items)) => {
            let inner = items
                .iter()
                .map(|x| {
                    rib_val_to_value_and_type(x, &l.inner)
                        .map(|v| v.value)
                        .with_context(|| "list element")
                })
                .collect::<Result<_>>()?;
            Value::List(inner)
        }
        (WT::Record(rec_ty), R::Record(pairs)) => {
            if rec_ty.fields.len() != pairs.len() {
                bail!("record field count mismatch");
            }
            let mut out = Vec::with_capacity(pairs.len());
            for (f, (n, rv)) in rec_ty.fields.iter().zip(pairs.iter()) {
                if f.name != *n {
                    bail!(
                        "record field name mismatch: expected `{}`, got `{n}`",
                        f.name
                    );
                }
                out.push(rib_val_to_value_and_type(rv, &f.typ)?.value);
            }
            Value::Record(out)
        }
        (WT::Tuple(tt), R::Tuple(items)) => {
            if tt.items.len() != items.len() {
                bail!("tuple arity mismatch");
            }
            let inner = tt
                .items
                .iter()
                .zip(items.iter())
                .map(|(t, rv)| Ok(rib_val_to_value_and_type(rv, t)?.value))
                .collect::<Result<_>>()?;
            Value::Tuple(inner)
        }
        (WT::Variant(vt), R::Variant(name, payload)) => {
            let (idx, case_ty) = vt
                .cases
                .iter()
                .enumerate()
                .find(|(_, c)| c.name == *name)
                .map(|(i, c)| (i as u32, &c.typ))
                .ok_or_else(|| anyhow!("unknown variant case `{name}`"))?;
            let case_value = match (case_ty, payload) {
                (None, None) => None,
                (Some(inner), Some(p)) => {
                    Some(Box::new(rib_val_to_value_and_type(p, inner)?.value))
                }
                _ => bail!("variant payload mismatch"),
            };
            Value::Variant {
                case_idx: idx,
                case_value,
            }
        }
        (WT::Enum(et), R::Enum(name)) => {
            let idx = et
                .cases
                .iter()
                .position(|c| c == name)
                .ok_or_else(|| anyhow!("unknown enum case `{name}`"))? as u32;
            Value::Enum(idx)
        }
        (WT::Option(ot), R::Option(inner)) => {
            let v = match inner {
                None => None,
                Some(b) => Some(Box::new(rib_val_to_value_and_type(b, &ot.inner)?.value)),
            };
            Value::Option(v)
        }
        (WT::Result(rt), R::Result(inner)) => {
            let v = match inner {
                Ok(x) => Ok(match x {
                    None => None,
                    Some(b) => Some(Box::new(
                        rib_val_to_value_and_type(
                            b,
                            rt.ok.as_deref().ok_or_else(|| anyhow!("ok type"))?,
                        )?
                        .value,
                    )),
                }),
                Err(x) => Err(match x {
                    None => None,
                    Some(b) => Some(Box::new(
                        rib_val_to_value_and_type(
                            b,
                            rt.err.as_deref().ok_or_else(|| anyhow!("err type"))?,
                        )?
                        .value,
                    )),
                }),
            };
            Value::Result(v)
        }
        (WT::Flags(ft), R::Flags(names)) => {
            let mut bits = vec![false; ft.names.len()];
            for n in names {
                let i = ft
                    .names
                    .iter()
                    .position(|x| x == n)
                    .ok_or_else(|| anyhow!("unknown flag `{n}`"))?;
                bits[i] = true;
            }
            Value::Flags(bits)
        }
        (WT::Handle(_), R::Handle { uri, resource_id }) => Value::Handle {
            uri: uri.clone(),
            resource_id: *resource_id,
        },
        _ => bail!(
            "cannot convert RibVal to Rib value for type {:?}: {:?}",
            ty,
            rv
        ),
    };
    Ok(ValueAndType::new(value, ty.clone()))
}

/// Returns the `i`th element [`WitType`] when the return type is a WIT tuple with multiple
/// results (helpers for splitting multi-return handling).
pub fn tuple_element_type(tuple_ty: &WitType, i: usize) -> Result<WitType> {
    match tuple_ty {
        WitType::Tuple(t) => t
            .items
            .get(i)
            .cloned()
            .ok_or_else(|| anyhow!("tuple arity mismatch")),
        _ => bail!("expected tuple return type for multi-value return"),
    }
}
