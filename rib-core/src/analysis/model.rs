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
                AnalysedType::Handle(TypeHandle {
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
                AnalysedType::Handle(TypeHandle {
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
    pub ok: Option<Box<AnalysedType>>,
    pub err: Option<Box<AnalysedType>>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct NameTypePair {
    pub name: String,
    pub typ: AnalysedType,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct NameOptionTypePair {
    pub name: String,
    pub typ: Option<AnalysedType>,
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
    pub inner: Box<AnalysedType>,
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
    pub items: Vec<AnalysedType>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct TypeList {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub inner: Box<AnalysedType>,
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
pub enum AnalysedType {
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

impl AnalysedType {
    pub fn name(&self) -> Option<&str> {
        match self {
            AnalysedType::Variant(typ) => typ.name.as_deref(),
            AnalysedType::Result(typ) => typ.name.as_deref(),
            AnalysedType::Option(typ) => typ.name.as_deref(),
            AnalysedType::Enum(typ) => typ.name.as_deref(),
            AnalysedType::Flags(typ) => typ.name.as_deref(),
            AnalysedType::Record(typ) => typ.name.as_deref(),
            AnalysedType::Tuple(typ) => typ.name.as_deref(),
            AnalysedType::List(typ) => typ.name.as_deref(),
            AnalysedType::Handle(typ) => typ.name.as_deref(),
            _ => None,
        }
    }

    pub fn with_optional_name(self, name: Option<String>) -> Self {
        match self {
            AnalysedType::Variant(mut typ) => {
                typ.name = name;
                AnalysedType::Variant(typ)
            }
            AnalysedType::Result(mut typ) => {
                typ.name = name;
                AnalysedType::Result(typ)
            }
            AnalysedType::Option(mut typ) => {
                typ.name = name;
                AnalysedType::Option(typ)
            }
            AnalysedType::Enum(mut typ) => {
                typ.name = name;
                AnalysedType::Enum(typ)
            }
            AnalysedType::Flags(mut typ) => {
                typ.name = name;
                AnalysedType::Flags(typ)
            }
            AnalysedType::Record(mut typ) => {
                typ.name = name;
                AnalysedType::Record(typ)
            }
            AnalysedType::Tuple(mut typ) => {
                typ.name = name;
                AnalysedType::Tuple(typ)
            }
            AnalysedType::List(mut typ) => {
                typ.name = name;
                AnalysedType::List(typ)
            }
            AnalysedType::Handle(mut typ) => {
                typ.name = name;
                AnalysedType::Handle(typ)
            }
            _ => self,
        }
    }

    pub fn named(self, name: impl AsRef<str>) -> Self {
        self.with_optional_name(Some(name.as_ref().to_string()))
    }

    pub fn owner(&self) -> Option<&str> {
        match self {
            AnalysedType::Variant(typ) => typ.owner.as_deref(),
            AnalysedType::Result(typ) => typ.owner.as_deref(),
            AnalysedType::Option(typ) => typ.owner.as_deref(),
            AnalysedType::Enum(typ) => typ.owner.as_deref(),
            AnalysedType::Flags(typ) => typ.owner.as_deref(),
            AnalysedType::Record(typ) => typ.owner.as_deref(),
            AnalysedType::Tuple(typ) => typ.owner.as_deref(),
            AnalysedType::List(typ) => typ.owner.as_deref(),
            AnalysedType::Handle(typ) => typ.owner.as_deref(),
            _ => None,
        }
    }

    pub fn with_optional_owner(self, owner: Option<String>) -> Self {
        match self {
            AnalysedType::Variant(mut typ) => {
                typ.owner = owner;
                AnalysedType::Variant(typ)
            }
            AnalysedType::Result(mut typ) => {
                typ.owner = owner;
                AnalysedType::Result(typ)
            }
            AnalysedType::Option(mut typ) => {
                typ.owner = owner;
                AnalysedType::Option(typ)
            }
            AnalysedType::Enum(mut typ) => {
                typ.owner = owner;
                AnalysedType::Enum(typ)
            }
            AnalysedType::Flags(mut typ) => {
                typ.owner = owner;
                AnalysedType::Flags(typ)
            }
            AnalysedType::Record(mut typ) => {
                typ.owner = owner;
                AnalysedType::Record(typ)
            }
            AnalysedType::Tuple(mut typ) => {
                typ.owner = owner;
                AnalysedType::Tuple(typ)
            }
            AnalysedType::List(mut typ) => {
                typ.owner = owner;
                AnalysedType::List(typ)
            }
            AnalysedType::Handle(mut typ) => {
                typ.owner = owner;
                AnalysedType::Handle(typ)
            }
            _ => self,
        }
    }

    pub fn owned(self, owner: impl AsRef<str>) -> Self {
        self.with_optional_owner(Some(owner.as_ref().to_string()))
    }

    pub fn contains_handle(&self) -> bool {
        match self {
            AnalysedType::Handle(_) => true,
            AnalysedType::Variant(typ) => typ
                .cases
                .iter()
                .any(|case| case.typ.as_ref().is_some_and(|t| t.contains_handle())),
            AnalysedType::Result(typ) => {
                typ.ok.as_ref().is_some_and(|t| t.contains_handle())
                    || typ.err.as_ref().is_some_and(|t| t.contains_handle())
            }
            AnalysedType::Option(typ) => typ.inner.contains_handle(),
            AnalysedType::Record(typ) => typ.fields.iter().any(|f| f.typ.contains_handle()),
            AnalysedType::Tuple(typ) => typ.items.iter().any(|t| t.contains_handle()),
            AnalysedType::List(typ) => typ.inner.contains_handle(),
            _ => false,
        }
    }
}

impl Display for AnalysedType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysedType::Variant(_) => write!(f, "Variant"),
            AnalysedType::Result(_) => write!(f, "Result"),
            AnalysedType::Option(_) => write!(f, "Option"),
            AnalysedType::Enum(_) => write!(f, "Enum"),
            AnalysedType::Flags(_) => write!(f, "Flags"),
            AnalysedType::Record(_) => write!(f, "Record"),
            AnalysedType::Tuple(_) => write!(f, "Tuple"),
            AnalysedType::List(_) => write!(f, "List"),
            AnalysedType::Str(_) => write!(f, "Str"),
            AnalysedType::Chr(_) => write!(f, "Chr"),
            AnalysedType::F64(_) => write!(f, "F64"),
            AnalysedType::F32(_) => write!(f, "F32"),
            AnalysedType::U64(_) => write!(f, "U64"),
            AnalysedType::S64(_) => write!(f, "S64"),
            AnalysedType::U32(_) => write!(f, "U32"),
            AnalysedType::S32(_) => write!(f, "S32"),
            AnalysedType::U16(_) => write!(f, "U16"),
            AnalysedType::S16(_) => write!(f, "S16"),
            AnalysedType::U8(_) => write!(f, "U8"),
            AnalysedType::S8(_) => write!(f, "S8"),
            AnalysedType::Bool(_) => write!(f, "Bool"),
            AnalysedType::Handle(_) => write!(f, "Handle"),
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
    pub typ: AnalysedType,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitFunctionResult {
    pub typ: AnalysedType,
}

pub mod analysed_type {
    use super::*;

    pub fn field(name: &str, typ: AnalysedType) -> NameTypePair {
        NameTypePair {
            name: name.to_string(),
            typ,
        }
    }

    pub fn case(name: &str, typ: AnalysedType) -> NameOptionTypePair {
        NameOptionTypePair {
            name: name.to_string(),
            typ: Some(typ),
        }
    }

    pub fn opt_case(name: &str, typ: Option<AnalysedType>) -> NameOptionTypePair {
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

    pub fn bool() -> AnalysedType {
        AnalysedType::Bool(TypeBool)
    }

    pub fn s8() -> AnalysedType {
        AnalysedType::S8(TypeS8)
    }

    pub fn s16() -> AnalysedType {
        AnalysedType::S16(TypeS16)
    }

    pub fn s32() -> AnalysedType {
        AnalysedType::S32(TypeS32)
    }

    pub fn s64() -> AnalysedType {
        AnalysedType::S64(TypeS64)
    }

    pub fn u8() -> AnalysedType {
        AnalysedType::U8(TypeU8)
    }

    pub fn u16() -> AnalysedType {
        AnalysedType::U16(TypeU16)
    }

    pub fn u32() -> AnalysedType {
        AnalysedType::U32(TypeU32)
    }

    pub fn u64() -> AnalysedType {
        AnalysedType::U64(TypeU64)
    }

    pub fn f32() -> AnalysedType {
        AnalysedType::F32(TypeF32)
    }

    pub fn f64() -> AnalysedType {
        AnalysedType::F64(TypeF64)
    }

    pub fn chr() -> AnalysedType {
        AnalysedType::Chr(TypeChr)
    }

    pub fn str() -> AnalysedType {
        AnalysedType::Str(TypeStr)
    }

    pub fn list(inner: AnalysedType) -> AnalysedType {
        AnalysedType::List(TypeList {
            name: None,
            owner: None,
            inner: Box::new(inner),
        })
    }

    pub fn option(inner: AnalysedType) -> AnalysedType {
        AnalysedType::Option(TypeOption {
            name: None,
            owner: None,
            inner: Box::new(inner),
        })
    }

    pub fn flags(names: &[&str]) -> AnalysedType {
        AnalysedType::Flags(TypeFlags {
            name: None,
            owner: None,
            names: names.iter().map(|n| n.to_string()).collect(),
        })
    }

    pub fn r#enum(cases: &[&str]) -> AnalysedType {
        AnalysedType::Enum(TypeEnum {
            name: None,
            owner: None,
            cases: cases.iter().map(|n| n.to_string()).collect(),
        })
    }

    pub fn tuple(items: Vec<AnalysedType>) -> AnalysedType {
        AnalysedType::Tuple(TypeTuple {
            name: None,
            owner: None,
            items,
        })
    }

    pub fn result(ok: AnalysedType, err: AnalysedType) -> AnalysedType {
        AnalysedType::Result(TypeResult {
            name: None,
            owner: None,
            ok: Some(Box::new(ok)),
            err: Some(Box::new(err)),
        })
    }

    pub fn result_ok(ok: AnalysedType) -> AnalysedType {
        AnalysedType::Result(TypeResult {
            name: None,
            owner: None,
            ok: Some(Box::new(ok)),
            err: None,
        })
    }

    pub fn result_err(err: AnalysedType) -> AnalysedType {
        AnalysedType::Result(TypeResult {
            name: None,
            owner: None,
            ok: None,
            err: Some(Box::new(err)),
        })
    }

    pub fn unit_result() -> AnalysedType {
        AnalysedType::Result(TypeResult {
            name: None,
            owner: None,
            ok: None,
            err: None,
        })
    }

    pub fn record(fields: Vec<NameTypePair>) -> AnalysedType {
        AnalysedType::Record(TypeRecord {
            name: None,
            owner: None,
            fields,
        })
    }

    pub fn variant(cases: Vec<NameOptionTypePair>) -> AnalysedType {
        AnalysedType::Variant(TypeVariant {
            name: None,
            owner: None,
            cases,
        })
    }

    pub fn handle(resource_id: AnalysedResourceId, mode: AnalysedResourceMode) -> AnalysedType {
        AnalysedType::Handle(TypeHandle {
            name: None,
            owner: None,
            resource_id,
            mode,
        })
    }
}
