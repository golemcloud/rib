use crate::wit_type::{bool, field, record, str, tuple};
use crate::wit_type::{
    AnalysedResourceId, AnalysedResourceMode, NameOptionTypePair, NameTypePair, TypeBool, TypeChr,
    TypeEnum, TypeF32, TypeF64, TypeFlags, TypeHandle, TypeList, TypeOption, TypeRecord,
    TypeResult, TypeS16, TypeS32, TypeS64, TypeS8, TypeStr, TypeTuple, TypeU16, TypeU32, TypeU64,
    TypeU8, TypeVariant, WitType,
};
use crate::{GetTypeHint, InferredType, InstanceType, TypeInternal};
use serde::{Deserialize, Serialize};

// An absence of wit type is really `Unit`, however, we avoid
// Option<WitType> in favor of `WitTypeWithUnit` for clarity.
// and conversions such as what to print if its `unit` becomes more precise
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WitTypeWithUnit {
    Unit,
    Type(WitType),
}

impl WitTypeWithUnit {
    pub fn unit() -> Self {
        WitTypeWithUnit::Unit
    }

    pub fn analysed_type(typ: WitType) -> Self {
        WitTypeWithUnit::Type(typ)
    }
}

impl TryFrom<WitTypeWithUnit> for WitType {
    type Error = String;

    fn try_from(value: WitTypeWithUnit) -> Result<Self, Self::Error> {
        match value {
            WitTypeWithUnit::Unit => Ok(tuple(vec![])),
            WitTypeWithUnit::Type(typ) => Ok(typ),
        }
    }
}

impl TryFrom<&InferredType> for WitType {
    type Error = String;

    fn try_from(value: &InferredType) -> Result<Self, Self::Error> {
        let with_unit = WitTypeWithUnit::try_from(value)?;
        WitType::try_from(with_unit)
    }
}

impl TryFrom<&InferredType> for WitTypeWithUnit {
    type Error = String;

    fn try_from(inferred_type: &InferredType) -> Result<Self, Self::Error> {
        match inferred_type.internal_type() {
            TypeInternal::Instance { instance_type } => match instance_type.as_ref() {
                InstanceType::Resource {
                    analysed_resource_id,
                    analysed_resource_mode,
                    ..
                } => {
                    let analysed_resource_id = AnalysedResourceId(*analysed_resource_id);

                    let analysed_resource_mode = if *analysed_resource_mode == 0 {
                        AnalysedResourceMode::Owned
                    } else {
                        AnalysedResourceMode::Borrowed
                    };

                    Ok(WitTypeWithUnit::analysed_type(WitType::Handle(
                        TypeHandle {
                            resource_id: analysed_resource_id,
                            mode: analysed_resource_mode,
                            name: None,
                            owner: None,
                        },
                    )))
                }

                _ => Ok(WitTypeWithUnit::analysed_type(str())),
            },
            TypeInternal::Range { from, to } => {
                let from: WitType = WitType::try_from(from)?;
                let to: Option<WitType> = to.as_ref().map(WitType::try_from).transpose()?;
                let analysed_type = match (from, to) {
                    (from_type, Some(to_type)) => record(vec![
                        field("from", from_type),
                        field("to", to_type),
                        field("inclusive", bool()),
                    ]),

                    (from_type, None) => {
                        record(vec![field("from", from_type), field("inclusive", bool())])
                    }
                };
                Ok(WitTypeWithUnit::analysed_type(analysed_type))
            }
            TypeInternal::Bool => Ok(WitTypeWithUnit::analysed_type(WitType::Bool(TypeBool))),
            TypeInternal::S8 => Ok(WitTypeWithUnit::analysed_type(WitType::S8(TypeS8))),
            TypeInternal::U8 => Ok(WitTypeWithUnit::analysed_type(WitType::U8(TypeU8))),
            TypeInternal::S16 => Ok(WitTypeWithUnit::analysed_type(WitType::S16(TypeS16))),
            TypeInternal::U16 => Ok(WitTypeWithUnit::analysed_type(WitType::U16(TypeU16))),
            TypeInternal::S32 => Ok(WitTypeWithUnit::analysed_type(WitType::S32(TypeS32))),
            TypeInternal::U32 => Ok(WitTypeWithUnit::analysed_type(WitType::U32(TypeU32))),
            TypeInternal::S64 => Ok(WitTypeWithUnit::analysed_type(WitType::S64(TypeS64))),
            TypeInternal::U64 => Ok(WitTypeWithUnit::analysed_type(WitType::U64(TypeU64))),
            TypeInternal::F32 => Ok(WitTypeWithUnit::analysed_type(WitType::F32(TypeF32))),
            TypeInternal::F64 => Ok(WitTypeWithUnit::analysed_type(WitType::F64(TypeF64))),
            TypeInternal::Chr => Ok(WitTypeWithUnit::analysed_type(WitType::Chr(TypeChr))),
            TypeInternal::Str => Ok(WitTypeWithUnit::analysed_type(WitType::Str(TypeStr))),
            TypeInternal::List(inferred_type) => {
                Ok(WitTypeWithUnit::analysed_type(WitType::List(TypeList {
                    inner: Box::new(inferred_type.try_into()?),
                    name: None,
                    owner: None,
                })))
            }
            TypeInternal::Tuple(tuple) => {
                Ok(WitTypeWithUnit::analysed_type(WitType::Tuple(TypeTuple {
                    items: tuple
                        .iter()
                        .map(|t| t.try_into())
                        .collect::<Result<Vec<WitType>, String>>()?,
                    name: None,
                    owner: None,
                })))
            }
            TypeInternal::Record(record) => Ok(WitTypeWithUnit::analysed_type(WitType::Record(
                TypeRecord {
                    fields: record
                        .iter()
                        .map(|(name, typ)| {
                            Ok(NameTypePair {
                                name: name.to_string(),
                                typ: typ.try_into()?,
                            })
                        })
                        .collect::<Result<Vec<NameTypePair>, String>>()?,
                    name: None,
                    owner: None,
                },
            ))),
            TypeInternal::Flags(flags) => {
                Ok(WitTypeWithUnit::analysed_type(WitType::Flags(TypeFlags {
                    names: flags.clone(),
                    name: None,
                    owner: None,
                })))
            }
            TypeInternal::Enum(enums) => {
                Ok(WitTypeWithUnit::analysed_type(WitType::Enum(TypeEnum {
                    cases: enums.clone(),
                    name: None,
                    owner: None,
                })))
            }
            TypeInternal::Option(option) => Ok(WitTypeWithUnit::analysed_type(WitType::Option(
                TypeOption {
                    inner: Box::new(option.try_into()?),
                    name: None,
                    owner: None,
                },
            ))),
            TypeInternal::Result { ok, error } => Ok(WitTypeWithUnit::analysed_type(
                // In the case of result, there are instances users give just 1 value with zero function calls, we need to be flexible here
                WitType::Result(TypeResult {
                    ok: ok.as_ref().and_then(|t| t.try_into().ok().map(Box::new)),
                    err: error.as_ref().and_then(|t| t.try_into().ok().map(Box::new)),
                    name: None,
                    owner: None,
                }),
            )),
            TypeInternal::Variant(variant) => Ok(WitTypeWithUnit::analysed_type(WitType::Variant(
                TypeVariant {
                    cases: variant
                        .iter()
                        .map(|(name, typ)| {
                            Ok(NameOptionTypePair {
                                name: name.clone(),
                                typ: typ.as_ref().map(|t| t.try_into()).transpose()?,
                            })
                        })
                        .collect::<Result<Vec<NameOptionTypePair>, String>>()?,
                    name: None,
                    owner: None,
                },
            ))),
            TypeInternal::Resource {
                resource_id,
                resource_mode,
                name: _,
                owner: _,
            } => Ok(WitTypeWithUnit::analysed_type(WitType::Handle(
                TypeHandle {
                    resource_id: AnalysedResourceId(*resource_id),
                    mode: if resource_mode == &0 {
                        AnalysedResourceMode::Owned
                    } else {
                        AnalysedResourceMode::Borrowed
                    },
                    name: None,
                    owner: None,
                },
            ))),

            TypeInternal::AllOf(types) => Err(format!(
                "ambiguous types {}",
                types
                    .iter()
                    .map(|x| x.get_type_hint().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            TypeInternal::Unknown => Err("failed to infer type".to_string()),
            // We don't expect to have a sequence type in the inferred type.as
            // This implies Rib will not support multiple types from worker-function results
            TypeInternal::Sequence(vec) => {
                if vec.is_empty() {
                    Ok(WitTypeWithUnit::unit())
                } else if vec.len() == 1 {
                    let first = &vec[0];
                    Ok(first.try_into()?)
                } else {
                    Err("function with multiple return types not supported".to_string())
                }
            }
        }
    }
}
