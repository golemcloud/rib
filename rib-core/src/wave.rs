use crate::analysis::{
    AnalysedFunction, AnalysedType, TypeEnum, TypeFlags, TypeList, TypeOption, TypeRecord,
    TypeResult, TypeTuple, TypeVariant,
};
use std::borrow::Cow;
use wasm_wave::wasm::{WasmFunc, WasmType, WasmTypeKind};

impl WasmType for AnalysedType {
    fn kind(&self) -> WasmTypeKind {
        match self {
            AnalysedType::Bool(_) => WasmTypeKind::Bool,
            AnalysedType::S8(_) => WasmTypeKind::S8,
            AnalysedType::U8(_) => WasmTypeKind::U8,
            AnalysedType::S16(_) => WasmTypeKind::S16,
            AnalysedType::U16(_) => WasmTypeKind::U16,
            AnalysedType::S32(_) => WasmTypeKind::S32,
            AnalysedType::U32(_) => WasmTypeKind::U32,
            AnalysedType::S64(_) => WasmTypeKind::S64,
            AnalysedType::U64(_) => WasmTypeKind::U64,
            AnalysedType::F32(_) => WasmTypeKind::F32,
            AnalysedType::F64(_) => WasmTypeKind::F64,
            AnalysedType::Chr(_) => WasmTypeKind::Char,
            AnalysedType::Str(_) => WasmTypeKind::String,
            AnalysedType::List(_) => WasmTypeKind::List,
            AnalysedType::Tuple(_) => WasmTypeKind::Tuple,
            AnalysedType::Record(_) => WasmTypeKind::Record,
            AnalysedType::Flags(_) => WasmTypeKind::Flags,
            AnalysedType::Enum(_) => WasmTypeKind::Enum,
            AnalysedType::Option(_) => WasmTypeKind::Option,
            AnalysedType::Result { .. } => WasmTypeKind::Result,
            AnalysedType::Variant(_) => WasmTypeKind::Variant,
            AnalysedType::Handle(_) => WasmTypeKind::Unsupported,
        }
    }

    fn list_element_type(&self) -> Option<Self> {
        if let AnalysedType::List(TypeList { inner: ty, .. }) = self {
            Some(*ty.clone())
        } else {
            None
        }
    }

    fn record_fields(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Self)> + '_> {
        if let AnalysedType::Record(TypeRecord { fields, .. }) = self {
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
        if let AnalysedType::Tuple(TypeTuple { items, .. }) = self {
            Box::new(items.clone().into_iter())
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn variant_cases(&self) -> Box<dyn Iterator<Item = (Cow<'_, str>, Option<Self>)> + '_> {
        if let AnalysedType::Variant(TypeVariant { cases, .. }) = self {
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
        if let AnalysedType::Enum(TypeEnum { cases, .. }) = self {
            Box::new(cases.iter().map(|name| Cow::Borrowed(name.as_str())))
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn option_some_type(&self) -> Option<Self> {
        if let AnalysedType::Option(TypeOption { inner, .. }) = self {
            Some(*inner.clone())
        } else {
            None
        }
    }

    fn result_types(&self) -> Option<(Option<Self>, Option<Self>)> {
        if let AnalysedType::Result(TypeResult { ok, err, .. }) = self {
            Some((
                ok.as_ref().map(|t| *t.clone()),
                err.as_ref().map(|t| *t.clone()),
            ))
        } else {
            None
        }
    }

    fn flags_names(&self) -> Box<dyn Iterator<Item = Cow<'_, str>> + '_> {
        if let AnalysedType::Flags(TypeFlags { names, .. }) = self {
            Box::new(names.iter().map(|name| Cow::Borrowed(name.as_str())))
        } else {
            Box::new(std::iter::empty())
        }
    }
}

impl WasmFunc for AnalysedFunction {
    type Type = AnalysedType;

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
