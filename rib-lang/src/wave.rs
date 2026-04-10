use crate::wit_type::{
    TypeEnum, TypeFlags, TypeList, TypeOption, TypeRecord, TypeResult, TypeTuple, TypeVariant,
    WitFunction, WitType,
};
use std::borrow::Cow;
use wasm_wave::wasm::{WasmFunc, WasmType, WasmTypeKind};

impl WasmType for WitType {
    fn kind(&self) -> WasmTypeKind {
        match self {
            WitType::Bool(_) => WasmTypeKind::Bool,
            WitType::S8(_) => WasmTypeKind::S8,
            WitType::U8(_) => WasmTypeKind::U8,
            WitType::S16(_) => WasmTypeKind::S16,
            WitType::U16(_) => WasmTypeKind::U16,
            WitType::S32(_) => WasmTypeKind::S32,
            WitType::U32(_) => WasmTypeKind::U32,
            WitType::S64(_) => WasmTypeKind::S64,
            WitType::U64(_) => WasmTypeKind::U64,
            WitType::F32(_) => WasmTypeKind::F32,
            WitType::F64(_) => WasmTypeKind::F64,
            WitType::Chr(_) => WasmTypeKind::Char,
            WitType::Str(_) => WasmTypeKind::String,
            WitType::List(_) => WasmTypeKind::List,
            WitType::Tuple(_) => WasmTypeKind::Tuple,
            WitType::Record(_) => WasmTypeKind::Record,
            WitType::Flags(_) => WasmTypeKind::Flags,
            WitType::Enum(_) => WasmTypeKind::Enum,
            WitType::Option(_) => WasmTypeKind::Option,
            WitType::Result { .. } => WasmTypeKind::Result,
            WitType::Variant(_) => WasmTypeKind::Variant,
            WitType::Handle(_) => WasmTypeKind::Unsupported,
        }
    }

    fn list_element_type(&self) -> Option<Self> {
        if let WitType::List(TypeList { inner: ty, .. }) = self {
            Some(*ty.clone())
        } else {
            None
        }
    }

    fn record_fields(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Self)> + '_> {
        if let WitType::Record(TypeRecord { fields, .. }) = self {
            Box::new(
                fields
                    .iter()
                    .map(|pair| (Cow::Borrowed(pair.name.as_str()), pair.typ.clone())),
            )
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn tuple_element_types(&self) -> Box<dyn Iterator<Item = Self> + '_> {
        if let WitType::Tuple(TypeTuple { items, .. }) = self {
            Box::new(items.clone().into_iter())
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn variant_cases(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Option<Self>)> + '_> {
        if let WitType::Variant(TypeVariant { cases, .. }) = self {
            Box::new(
                cases
                    .iter()
                    .map(|case| (Cow::Borrowed(case.name.as_str()), case.typ.clone())),
            )
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn enum_cases(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        if let WitType::Enum(TypeEnum { cases, .. }) = self {
            Box::new(cases.iter().map(|name| Cow::Borrowed(name.as_str())))
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn option_some_type(&self) -> Option<Self> {
        if let WitType::Option(TypeOption { inner, .. }) = self {
            Some(*inner.clone())
        } else {
            None
        }
    }

    fn result_types(&self) -> Option<(Option<Self>, Option<Self>)> {
        if let WitType::Result(TypeResult { ok, err, .. }) = self {
            Some((
                ok.as_ref().map(|t| *t.clone()),
                err.as_ref().map(|t| *t.clone()),
            ))
        } else {
            None
        }
    }

    fn flags_names(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        if let WitType::Flags(TypeFlags { names, .. }) = self {
            Box::new(names.iter().map(|name| Cow::Borrowed(name.as_str())))
        } else {
            Box::new(std::iter::empty())
        }
    }
}

impl WasmFunc for WitFunction {
    type Type = WitType;

    fn params(&self) -> Box<dyn Iterator<Item = Self::Type> + '_> {
        Box::new(self.parameters.iter().map(|p| p.typ.clone()))
    }

    fn param_names(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        Box::new(
            self.parameters
                .iter()
                .map(|p| Cow::Borrowed(p.name.as_str())),
        )
    }

    fn results(&self) -> Box<dyn Iterator<Item = Self::Type> + '_> {
        Box::new(self.result.iter().map(|r| r.typ.clone()))
    }
}
