// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum WitExport {
    Function(WitFunction),
    Interface(WitInterface),
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitFunction {
    pub name: String,
    pub parameters: Vec<WitFunctionParameter>,
    pub result: Option<WitFunctionResult>,
}

impl WitFunction {
    pub fn is_constructor(&self) -> bool {
        self.name.starts_with("[constructor]")
            && self.result.is_some()
            && matches!(
                &self.result.as_ref().unwrap().typ,
                WitType::Handle(TypeHandle {
                    mode: AnalysedResourceMode::Owned,
                    ..
                })
            )
    }

    pub fn is_method(&self) -> bool {
        self.name.starts_with("[method]")
            && !self.parameters.is_empty()
            && matches!(
                &self.parameters[0].typ,
                WitType::Handle(TypeHandle {
                    mode: AnalysedResourceMode::Borrowed,
                    ..
                })
            )
    }

    pub fn is_static_method(&self) -> bool {
        self.name.starts_with("[static]")
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitInterface {
    pub name: String,
    pub functions: Vec<WitFunction>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeResult {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub ok: Option<Box<WitType>>,
    pub err: Option<Box<WitType>>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct NameTypePair {
    pub name: String,
    pub typ: WitType,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct NameOptionTypePair {
    pub name: String,
    pub typ: Option<WitType>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeVariant {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub cases: Vec<NameOptionTypePair>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeOption {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub inner: Box<WitType>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeEnum {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub cases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeFlags {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeRecord {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub fields: Vec<NameTypePair>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeTuple {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub items: Vec<WitType>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeList {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub inner: Box<WitType>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeStr;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeChr;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeF64;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeF32;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeU64;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeS64;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeU32;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeS32;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeU16;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeS16;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeU8;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeS8;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeBool;

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeHandle {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub resource_id: AnalysedResourceId,
    pub mode: AnalysedResourceMode,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum WitType {
    Variant(TypeVariant),
    Result(TypeResult),
    Option(TypeOption),
    Enum(TypeEnum),
    Flags(TypeFlags),
    Record(TypeRecord),
    Tuple(TypeTuple),
    List(TypeList),
    Str(TypeStr),
    Chr(TypeChr),
    F64(TypeF64),
    F32(TypeF32),
    U64(TypeU64),
    S64(TypeS64),
    U32(TypeU32),
    S32(TypeS32),
    U16(TypeU16),
    S16(TypeS16),
    U8(TypeU8),
    S8(TypeS8),
    Bool(TypeBool),
    Handle(TypeHandle),
}

impl WitType {
    pub fn name(&self) -> Option<&str> {
        match self {
            WitType::Variant(typ) => typ.name.as_deref(),
            WitType::Result(typ) => typ.name.as_deref(),
            WitType::Option(typ) => typ.name.as_deref(),
            WitType::Enum(typ) => typ.name.as_deref(),
            WitType::Flags(typ) => typ.name.as_deref(),
            WitType::Record(typ) => typ.name.as_deref(),
            WitType::Tuple(typ) => typ.name.as_deref(),
            WitType::List(typ) => typ.name.as_deref(),
            WitType::Handle(typ) => typ.name.as_deref(),
            _ => None,
        }
    }

    pub fn with_optional_name(self, name: Option<String>) -> Self {
        match self {
            WitType::Variant(mut typ) => {
                typ.name = name;
                WitType::Variant(typ)
            }
            WitType::Result(mut typ) => {
                typ.name = name;
                WitType::Result(typ)
            }
            WitType::Option(mut typ) => {
                typ.name = name;
                WitType::Option(typ)
            }
            WitType::Enum(mut typ) => {
                typ.name = name;
                WitType::Enum(typ)
            }
            WitType::Flags(mut typ) => {
                typ.name = name;
                WitType::Flags(typ)
            }
            WitType::Record(mut typ) => {
                typ.name = name;
                WitType::Record(typ)
            }
            WitType::Tuple(mut typ) => {
                typ.name = name;
                WitType::Tuple(typ)
            }
            WitType::List(mut typ) => {
                typ.name = name;
                WitType::List(typ)
            }
            WitType::Handle(mut typ) => {
                typ.name = name;
                WitType::Handle(typ)
            }
            _ => self,
        }
    }

    pub fn named(self, name: impl AsRef<str>) -> Self {
        self.with_optional_name(Some(name.as_ref().to_string()))
    }

    pub fn owner(&self) -> Option<&str> {
        match self {
            WitType::Variant(typ) => typ.owner.as_deref(),
            WitType::Result(typ) => typ.owner.as_deref(),
            WitType::Option(typ) => typ.owner.as_deref(),
            WitType::Enum(typ) => typ.owner.as_deref(),
            WitType::Flags(typ) => typ.owner.as_deref(),
            WitType::Record(typ) => typ.owner.as_deref(),
            WitType::Tuple(typ) => typ.owner.as_deref(),
            WitType::List(typ) => typ.owner.as_deref(),
            WitType::Handle(typ) => typ.owner.as_deref(),
            _ => None,
        }
    }

    pub fn with_optional_owner(self, owner: Option<String>) -> Self {
        match self {
            WitType::Variant(mut typ) => {
                typ.owner = owner;
                WitType::Variant(typ)
            }
            WitType::Result(mut typ) => {
                typ.owner = owner;
                WitType::Result(typ)
            }
            WitType::Option(mut typ) => {
                typ.owner = owner;
                WitType::Option(typ)
            }
            WitType::Enum(mut typ) => {
                typ.owner = owner;
                WitType::Enum(typ)
            }
            WitType::Flags(mut typ) => {
                typ.owner = owner;
                WitType::Flags(typ)
            }
            WitType::Record(mut typ) => {
                typ.owner = owner;
                WitType::Record(typ)
            }
            WitType::Tuple(mut typ) => {
                typ.owner = owner;
                WitType::Tuple(typ)
            }
            WitType::List(mut typ) => {
                typ.owner = owner;
                WitType::List(typ)
            }
            WitType::Handle(mut typ) => {
                typ.owner = owner;
                WitType::Handle(typ)
            }
            _ => self,
        }
    }

    pub fn owned(self, owner: impl AsRef<str>) -> Self {
        self.with_optional_owner(Some(owner.as_ref().to_string()))
    }

    pub fn contains_handle(&self) -> bool {
        match self {
            WitType::Handle(_) => true,
            WitType::Variant(typ) => typ
                .cases
                .iter()
                .any(|case| case.typ.as_ref().is_some_and(|t| t.contains_handle())),
            WitType::Result(typ) => {
                typ.ok.as_ref().is_some_and(|t| t.contains_handle())
                    || typ.err.as_ref().is_some_and(|t| t.contains_handle())
            }
            WitType::Option(typ) => typ.inner.contains_handle(),
            WitType::Record(typ) => typ.fields.iter().any(|f| f.typ.contains_handle()),
            WitType::Tuple(typ) => typ.items.iter().any(|t| t.contains_handle()),
            WitType::List(typ) => typ.inner.contains_handle(),
            _ => false,
        }
    }
}

impl Display for WitType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WitType::Variant(_) => write!(f, "Variant"),
            WitType::Result(_) => write!(f, "Result"),
            WitType::Option(_) => write!(f, "Option"),
            WitType::Enum(_) => write!(f, "Enum"),
            WitType::Flags(_) => write!(f, "Flags"),
            WitType::Record(_) => write!(f, "Record"),
            WitType::Tuple(_) => write!(f, "Tuple"),
            WitType::List(_) => write!(f, "List"),
            WitType::Str(_) => write!(f, "Str"),
            WitType::Chr(_) => write!(f, "Chr"),
            WitType::F64(_) => write!(f, "F64"),
            WitType::F32(_) => write!(f, "F32"),
            WitType::U64(_) => write!(f, "U64"),
            WitType::S64(_) => write!(f, "S64"),
            WitType::U32(_) => write!(f, "U32"),
            WitType::S32(_) => write!(f, "S32"),
            WitType::U16(_) => write!(f, "U16"),
            WitType::S16(_) => write!(f, "S16"),
            WitType::U8(_) => write!(f, "U8"),
            WitType::S8(_) => write!(f, "S8"),
            WitType::Bool(_) => write!(f, "Bool"),
            WitType::Handle(_) => write!(f, "Handle"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub enum AnalysedResourceMode {
    Owned,
    Borrowed,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnalysedResourceId(pub u64);

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitFunctionParameter {
    pub name: String,
    pub typ: WitType,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitFunctionResult {
    pub typ: WitType,
}

/// Helper constructors for building `WitType` values in tests and metadata.
pub mod wit_type {
    use super::*;

    pub fn field(name: &str, typ: WitType) -> NameTypePair {
        NameTypePair {
            name: name.to_string(),
            typ,
        }
    }

    pub fn case(name: &str, typ: WitType) -> NameOptionTypePair {
        NameOptionTypePair {
            name: name.to_string(),
            typ: Some(typ),
        }
    }

    pub fn opt_case(name: &str, typ: Option<WitType>) -> NameOptionTypePair {
        NameOptionTypePair {
            name: name.to_string(),
            typ,
        }
    }

    pub fn unit_case(name: &str) -> NameOptionTypePair {
        NameOptionTypePair {
            name: name.to_string(),
            typ: None,
        }
    }

    pub fn bool() -> WitType {
        WitType::Bool(TypeBool)
    }

    pub fn s8() -> WitType {
        WitType::S8(TypeS8)
    }

    pub fn s16() -> WitType {
        WitType::S16(TypeS16)
    }

    pub fn s32() -> WitType {
        WitType::S32(TypeS32)
    }

    pub fn s64() -> WitType {
        WitType::S64(TypeS64)
    }

    pub fn u8() -> WitType {
        WitType::U8(TypeU8)
    }

    pub fn u16() -> WitType {
        WitType::U16(TypeU16)
    }

    pub fn u32() -> WitType {
        WitType::U32(TypeU32)
    }

    pub fn u64() -> WitType {
        WitType::U64(TypeU64)
    }

    pub fn f32() -> WitType {
        WitType::F32(TypeF32)
    }

    pub fn f64() -> WitType {
        WitType::F64(TypeF64)
    }

    pub fn chr() -> WitType {
        WitType::Chr(TypeChr)
    }

    pub fn str() -> WitType {
        WitType::Str(TypeStr)
    }

    pub fn list(inner: WitType) -> WitType {
        WitType::List(TypeList {
            name: None,
            owner: None,
            inner: Box::new(inner),
        })
    }

    pub fn option(inner: WitType) -> WitType {
        WitType::Option(TypeOption {
            name: None,
            owner: None,
            inner: Box::new(inner),
        })
    }

    pub fn flags(names: &[&str]) -> WitType {
        WitType::Flags(TypeFlags {
            name: None,
            owner: None,
            names: names.iter().map(|n| n.to_string()).collect(),
        })
    }

    pub fn r#enum(cases: &[&str]) -> WitType {
        WitType::Enum(TypeEnum {
            name: None,
            owner: None,
            cases: cases.iter().map(|n| n.to_string()).collect(),
        })
    }

    pub fn tuple(items: Vec<WitType>) -> WitType {
        WitType::Tuple(TypeTuple {
            name: None,
            owner: None,
            items,
        })
    }

    pub fn result(ok: WitType, err: WitType) -> WitType {
        WitType::Result(TypeResult {
            name: None,
            owner: None,
            ok: Some(Box::new(ok)),
            err: Some(Box::new(err)),
        })
    }

    pub fn result_ok(ok: WitType) -> WitType {
        WitType::Result(TypeResult {
            name: None,
            owner: None,
            ok: Some(Box::new(ok)),
            err: None,
        })
    }

    pub fn result_err(err: WitType) -> WitType {
        WitType::Result(TypeResult {
            name: None,
            owner: None,
            ok: None,
            err: Some(Box::new(err)),
        })
    }

    pub fn unit_result() -> WitType {
        WitType::Result(TypeResult {
            name: None,
            owner: None,
            ok: None,
            err: None,
        })
    }

    pub fn record(fields: Vec<NameTypePair>) -> WitType {
        WitType::Record(TypeRecord {
            name: None,
            owner: None,
            fields,
        })
    }

    pub fn variant(cases: Vec<NameOptionTypePair>) -> WitType {
        WitType::Variant(TypeVariant {
            name: None,
            owner: None,
            cases,
        })
    }

    pub fn handle(resource_id: AnalysedResourceId, mode: AnalysedResourceMode) -> WitType {
        WitType::Handle(TypeHandle {
            name: None,
            owner: None,
            resource_id,
            mode,
        })
    }
}

// Re-export helper constructors at module root for ergonomic imports like
// `use crate::wit_type::{record, option, str, ...}`.
pub use wit_type::*;

