use crate::InferredType;

// Standalone precise types
#[derive(Clone, PartialEq, Debug)]
pub struct RecordType(pub Vec<(String, InferredType)>);

#[derive(Clone, PartialEq, Debug)]
pub struct OptionalType(pub InferredType);

#[derive(Clone, PartialEq, Debug)]
pub struct OkType(pub Option<InferredType>);

#[derive(Clone, PartialEq, Debug)]
pub struct ErrType(pub Option<InferredType>);

#[derive(Clone, PartialEq, Debug)]
pub struct ListType(pub InferredType);

#[derive(Clone, PartialEq, Debug)]
pub struct TupleType(pub Vec<InferredType>);

#[derive(Clone, PartialEq, Debug)]
pub struct VariantType(pub Vec<(String, Option<InferredType>)>);
#[derive(Clone, PartialEq, Debug)]
pub struct StringType;

#[allow(dead_code)]
#[derive(Clone, PartialEq, Debug)]
pub struct NumberType;

#[allow(dead_code)]
#[derive(Clone, PartialEq, Debug)]
pub struct CharType;

#[allow(dead_code)]
#[derive(Clone, PartialEq, Debug)]
pub struct BoolType;

#[allow(dead_code)]
#[derive(Clone, PartialEq, Debug)]
pub struct FlagsType(pub Vec<String>);

#[allow(dead_code)]
#[derive(Clone, PartialEq, Debug)]
pub struct EnumType(pub Vec<String>);

#[derive(Clone, PartialEq, Debug)]
pub struct RangeType(pub InferredType, pub Option<InferredType>);
