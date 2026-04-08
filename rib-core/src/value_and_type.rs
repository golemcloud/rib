use crate::wit_type::WitType;
use crate::value::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct ValueAndType {
    pub value: Value,
    pub typ: WitType,
}

impl ValueAndType {
    pub fn new(value: Value, typ: WitType) -> Self {
        Self { value, typ }
    }
}

impl std::fmt::Display for ValueAndType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match crate::print_value_and_type(self) {
            Ok(s) => write!(f, "{s}"),
            Err(_) => write!(f, "{:?}", self.value),
        }
    }
}

impl From<ValueAndType> for Value {
    fn from(value_and_type: ValueAndType) -> Self {
        value_and_type.value
    }
}

impl From<ValueAndType> for WitType {
    fn from(value_and_type: ValueAndType) -> Self {
        value_and_type.typ
    }
}

pub trait IntoValue {
    fn into_value(self) -> Value;
    fn get_type() -> WitType;
}

pub trait IntoValueAndType {
    fn into_value_and_type(self) -> ValueAndType;
}

impl<T: IntoValue + Sized> IntoValueAndType for T {
    fn into_value_and_type(self) -> ValueAndType {
        ValueAndType::new(self.into_value(), Self::get_type())
    }
}

use crate::wit_type::wit_type;

impl IntoValue for u8 {
    fn into_value(self) -> Value {
        Value::U8(self)
    }
    fn get_type() -> WitType {
        wit_type::u8()
    }
}

impl IntoValue for u16 {
    fn into_value(self) -> Value {
        Value::U16(self)
    }
    fn get_type() -> WitType {
        wit_type::u16()
    }
}

impl IntoValue for u32 {
    fn into_value(self) -> Value {
        Value::U32(self)
    }
    fn get_type() -> WitType {
        wit_type::u32()
    }
}

impl IntoValue for u64 {
    fn into_value(self) -> Value {
        Value::U64(self)
    }
    fn get_type() -> WitType {
        wit_type::u64()
    }
}

impl IntoValue for i8 {
    fn into_value(self) -> Value {
        Value::S8(self)
    }
    fn get_type() -> WitType {
        wit_type::s8()
    }
}

impl IntoValue for i16 {
    fn into_value(self) -> Value {
        Value::S16(self)
    }
    fn get_type() -> WitType {
        wit_type::s16()
    }
}

impl IntoValue for i32 {
    fn into_value(self) -> Value {
        Value::S32(self)
    }
    fn get_type() -> WitType {
        wit_type::s32()
    }
}

impl IntoValue for i64 {
    fn into_value(self) -> Value {
        Value::S64(self)
    }
    fn get_type() -> WitType {
        wit_type::s64()
    }
}

impl IntoValue for f32 {
    fn into_value(self) -> Value {
        Value::F32(self)
    }
    fn get_type() -> WitType {
        wit_type::f32()
    }
}

impl IntoValue for f64 {
    fn into_value(self) -> Value {
        Value::F64(self)
    }
    fn get_type() -> WitType {
        wit_type::f64()
    }
}

impl IntoValue for bool {
    fn into_value(self) -> Value {
        Value::Bool(self)
    }
    fn get_type() -> WitType {
        wit_type::bool()
    }
}

impl IntoValue for char {
    fn into_value(self) -> Value {
        Value::Char(self)
    }
    fn get_type() -> WitType {
        wit_type::chr()
    }
}

impl IntoValue for String {
    fn into_value(self) -> Value {
        Value::String(self)
    }
    fn get_type() -> WitType {
        wit_type::str()
    }
}

impl IntoValue for &str {
    fn into_value(self) -> Value {
        Value::String(self.to_string())
    }
    fn get_type() -> WitType {
        wit_type::str()
    }
}

impl<T: IntoValue> IntoValue for Vec<T> {
    fn into_value(self) -> Value {
        Value::List(self.into_iter().map(|v| v.into_value()).collect())
    }
    fn get_type() -> WitType {
        wit_type::list(T::get_type())
    }
}

impl<T: IntoValue> IntoValue for Option<T> {
    fn into_value(self) -> Value {
        Value::Option(self.map(|v| Box::new(v.into_value())))
    }
    fn get_type() -> WitType {
        wit_type::option(T::get_type())
    }
}

impl<A: IntoValue, B: IntoValue> IntoValue for (A, B) {
    fn into_value(self) -> Value {
        Value::Tuple(vec![self.0.into_value(), self.1.into_value()])
    }
    fn get_type() -> WitType {
        wit_type::tuple(vec![A::get_type(), B::get_type()])
    }
}
