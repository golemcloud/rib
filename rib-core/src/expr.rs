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

use crate::analysis::AnalysedType;
use crate::call_type::CallType;
use crate::inferred_type::DefaultType;
use crate::parser::block::block;
use crate::parser::type_name::TypeName;
use crate::rib_source_span::SourceSpan;
use crate::rib_type_error::RibTypeErrorInternal;
use crate::{
    from_string, text, type_checker, type_inference, ComponentDependencies, ComponentDependencyKey,
    CustomInstanceSpec, DynamicParsedFunctionName, GlobalVariableTypeSpec, InferredType,
    InstanceIdentifier, VariableId,
};
use crate::{IntoValueAndType, ValueAndType};
use bigdecimal::{BigDecimal, ToPrimitive};
use combine::parser::char::spaces;
use combine::stream::position;
use combine::Parser;
use combine::{eof, EasyParser};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::VecDeque;
use std::fmt::Display;
use std::ops::Deref;

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expr {
    Let {
        variable_id: VariableId,
        type_annotation: Option<TypeName>,
        expr: Box<Expr>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    SelectField {
        expr: Box<Expr>,
        field: String,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    SelectIndex {
        expr: Box<Expr>,
        index: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Sequence {
        exprs: Vec<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Range {
        range: Range,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Record {
        exprs: Vec<(String, Box<Expr>)>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Tuple {
        exprs: Vec<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Literal {
        value: String,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Number {
        number: Number,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Flags {
        flags: Vec<String>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Identifier {
        variable_id: VariableId,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Boolean {
        value: bool,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Concat {
        exprs: Vec<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    ExprBlock {
        exprs: Vec<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Not {
        expr: Box<Expr>,
        inferred_type: InferredType,
        type_annotation: Option<TypeName>,
        source_span: SourceSpan,
    },
    GreaterThan {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    And {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Or {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    GreaterThanOrEqualTo {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    LessThanOrEqualTo {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Plus {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Multiply {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Minus {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Divide {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    EqualTo {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    LessThan {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Cond {
        cond: Box<Expr>,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    PatternMatch {
        predicate: Box<Expr>,
        match_arms: Vec<MatchArm>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Option {
        expr: Option<Box<Expr>>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Result {
        expr: Result<Box<Expr>, Box<Expr>>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    // `instance("my-worker")` is parsed as Expr::Call { "instance", vec!["my-worker"] }.
    // During function call inference phase, the type of this `Expr::Call` will be `Expr::Call { InstanceCreation,.. }
    // with inferred-type as `InstanceType`. This way any variables attached to the instance creation
    // will be having the `InstanceType`.
    Call {
        call_type: CallType,
        args: Vec<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    // Any calls such as `my-worker-variable-expr.function_name()` will be parsed as Expr::Invoke
    // such that `my-worker-variable-expr` (lhs) will be of the type `InferredType::InstanceType`. `lhs` will
    // be `Expr::Call { InstanceCreation }` with type `InferredType::InstanceType`.
    // As part of a separate type inference phase this will be converted back to `Expr::Call` with fully
    // qualified function names (the complex version) which further takes part in all other type inference phases.
    InvokeMethodLazy {
        lhs: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Unwrap {
        expr: Box<Expr>,
        inferred_type: InferredType,
        type_annotation: Option<TypeName>,
        source_span: SourceSpan,
    },
    Throw {
        message: String,
        inferred_type: InferredType,
        type_annotation: Option<TypeName>,
        source_span: SourceSpan,
    },
    GetTag {
        expr: Box<Expr>,
        inferred_type: InferredType,
        type_annotation: Option<TypeName>,
        source_span: SourceSpan,
    },
    ListComprehension {
        iterated_variable: VariableId,
        iterable_expr: Box<Expr>,
        yield_expr: Box<Expr>,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    ListReduce {
        reduce_variable: VariableId,
        iterated_variable: VariableId,
        iterable_expr: Box<Expr>,
        type_annotation: Option<TypeName>,
        yield_expr: Box<Expr>,
        init_value_expr: Box<Expr>,
        inferred_type: InferredType,
        source_span: SourceSpan,
    },
    Length {
        expr: Box<Expr>,
        inferred_type: InferredType,
        type_annotation: Option<TypeName>,
        source_span: SourceSpan,
    },

    GenerateWorkerName {
        inferred_type: InferredType,
        type_annotation: Option<TypeName>,
        source_span: SourceSpan,
        variable_id: Option<VariableId>,
    },
}

impl Expr {
    pub fn as_record(&self) -> Option<Vec<(String, Expr)>> {
        match self {
            Expr::Record { exprs: fields, .. } => Some(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.deref().clone()))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }
    /// Parse a text directly as Rib expression
    /// Example of a Rib expression:
    ///
    /// ```rib
    ///   let shopping-cart-worker = instance("my-worker");
    ///   let result = shopping-cart-worker.add-to-cart({product-name: "apple", quantity: 2});
    ///
    ///   match result {
    ///     ok(id) => "product-id-${id}",
    ///     err(error_msg) => "Error: ${error_msg}"
    ///   }
    /// ```
    ///
    /// Rib supports conditional calls, function calls, pattern-matching,
    /// string interpolation (see error_message above) etc.
    ///
    pub fn from_text(input: &str) -> Result<Expr, String> {
        if input.trim().ends_with(';') {
            return Err("unexpected `;` at the end of rib expression. \nnote: `;` is used to separate expressions, but it should not appear after the last expression (which is the return value)".to_string());
        }

        spaces()
            .with(block().skip(eof()))
            .easy_parse(position::Stream::new(input))
            .map(|t| t.0)
            .map_err(|err| format!("{err}"))
    }

    pub fn lookup(&self, source_span: &SourceSpan) -> Option<Expr> {
        let mut expr = self.clone();
        find_expr(&mut expr, source_span)
    }

    pub fn is_literal(&self) -> bool {
        matches!(self, Expr::Literal { .. })
    }

    pub fn is_block(&self) -> bool {
        matches!(self, Expr::ExprBlock { .. })
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Expr::Number { .. })
    }

    pub fn is_record(&self) -> bool {
        matches!(self, Expr::Record { .. })
    }

    pub fn is_result(&self) -> bool {
        matches!(self, Expr::Result { .. })
    }

    pub fn is_option(&self) -> bool {
        matches!(self, Expr::Option { .. })
    }

    pub fn is_tuple(&self) -> bool {
        matches!(self, Expr::Tuple { .. })
    }

    pub fn is_list(&self) -> bool {
        matches!(self, Expr::Sequence { .. })
    }

    pub fn is_flags(&self) -> bool {
        matches!(self, Expr::Flags { .. })
    }

    pub fn is_identifier(&self) -> bool {
        matches!(self, Expr::Identifier { .. })
    }

    pub fn is_select_field(&self) -> bool {
        matches!(self, Expr::SelectField { .. })
    }

    pub fn is_if_else(&self) -> bool {
        matches!(self, Expr::Cond { .. })
    }

    pub fn is_function_call(&self) -> bool {
        matches!(self, Expr::Call { .. })
    }

    pub fn is_match_expr(&self) -> bool {
        matches!(self, Expr::PatternMatch { .. })
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Expr::Boolean { .. })
    }

    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            Expr::GreaterThan { .. }
                | Expr::GreaterThanOrEqualTo { .. }
                | Expr::LessThanOrEqualTo { .. }
                | Expr::EqualTo { .. }
                | Expr::LessThan { .. }
        )
    }

    pub fn is_concat(&self) -> bool {
        matches!(self, Expr::Concat { .. })
    }

    pub fn is_multiple(&self) -> bool {
        matches!(self, Expr::ExprBlock { .. })
    }

    pub fn inbuilt_variant(&self) -> Option<(String, Option<Expr>)> {
        match self {
            Expr::Option {
                expr: Some(expr), ..
            } => Some(("some".to_string(), Some(expr.deref().clone()))),
            Expr::Option { expr: None, .. } => Some(("some".to_string(), None)),
            Expr::Result { expr: Ok(expr), .. } => {
                Some(("ok".to_string(), Some(expr.deref().clone())))
            }
            Expr::Result {
                expr: Err(expr), ..
            } => Some(("err".to_string(), Some(expr.deref().clone()))),
            _ => None,
        }
    }
    pub fn unwrap(&self) -> Self {
        Expr::Unwrap {
            expr: Box::new(self.clone()),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn length(expr: Expr) -> Self {
        Expr::Length {
            expr: Box::new(expr),
            inferred_type: InferredType::u64(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn boolean(value: bool) -> Self {
        Expr::Boolean {
            value,
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn and(left: Expr, right: Expr) -> Self {
        Expr::And {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn throw(message: impl AsRef<str>) -> Self {
        Expr::Throw {
            message: message.as_ref().to_string(),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn generate_worker_name(variable_id: Option<VariableId>) -> Self {
        Expr::GenerateWorkerName {
            inferred_type: InferredType::string(),
            type_annotation: None,
            source_span: SourceSpan::default(),
            variable_id,
        }
    }

    pub fn plus(left: Expr, right: Expr) -> Self {
        Expr::Plus {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn minus(left: Expr, right: Expr) -> Self {
        Expr::Minus {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn divide(left: Expr, right: Expr) -> Self {
        Expr::Divide {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn multiply(left: Expr, right: Expr) -> Self {
        Expr::Multiply {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn and_combine(conditions: Vec<Expr>) -> Option<Expr> {
        let mut cond: Option<Expr> = None;

        for i in conditions {
            let left = Box::new(cond.clone().unwrap_or(Expr::boolean(true)));
            cond = Some(Expr::And {
                lhs: left,
                rhs: Box::new(i),
                inferred_type: InferredType::bool(),
                source_span: SourceSpan::default(),
                type_annotation: None,
            });
        }

        cond
    }

    pub fn call_worker_function(
        dynamic_parsed_fn_name: DynamicParsedFunctionName,
        module_identifier: Option<InstanceIdentifier>,
        args: Vec<Expr>,
        component_info: Option<ComponentDependencyKey>,
    ) -> Self {
        Expr::Call {
            call_type: CallType::Function {
                function_name: dynamic_parsed_fn_name,
                instance_identifier: module_identifier.map(Box::new),
                component_info,
            },
            args,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn call(call_type: CallType, args: Vec<Expr>) -> Self {
        Expr::Call {
            call_type,
            args,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn invoke_worker_function(lhs: Expr, function_name: String, args: Vec<Expr>) -> Self {
        Expr::InvokeMethodLazy {
            lhs: Box::new(lhs),
            method: function_name,
            args,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn concat(expressions: Vec<Expr>) -> Self {
        Expr::Concat {
            exprs: expressions,
            inferred_type: InferredType::string(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn cond(cond: Expr, lhs: Expr, rhs: Expr) -> Self {
        Expr::Cond {
            cond: Box::new(cond),
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn equal_to(left: Expr, right: Expr) -> Self {
        Expr::EqualTo {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn err(expr: Expr, type_annotation: Option<TypeName>) -> Self {
        let inferred_type = expr.inferred_type();
        Expr::Result {
            expr: Err(Box::new(expr)),
            type_annotation,
            inferred_type: InferredType::result(Some(InferredType::unknown()), Some(inferred_type)),
            source_span: SourceSpan::default(),
        }
    }

    pub fn flags(flags: Vec<String>) -> Self {
        Expr::Flags {
            flags: flags.clone(),
            inferred_type: InferredType::flags(flags),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn greater_than(left: Expr, right: Expr) -> Self {
        Expr::GreaterThan {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn greater_than_or_equal_to(left: Expr, right: Expr) -> Self {
        Expr::GreaterThanOrEqualTo {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    // An identifier by default is global until name-binding phase is run
    pub fn identifier_global(name: impl AsRef<str>, type_annotation: Option<TypeName>) -> Self {
        Expr::Identifier {
            variable_id: VariableId::global(name.as_ref().to_string()),
            type_annotation,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn identifier_local(
        name: impl AsRef<str>,
        id: u32,
        type_annotation: Option<TypeName>,
    ) -> Self {
        Expr::Identifier {
            variable_id: VariableId::local(name.as_ref(), id),
            type_annotation,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn identifier_with_variable_id(
        variable_id: VariableId,
        type_annotation: Option<TypeName>,
    ) -> Self {
        Expr::Identifier {
            variable_id,
            type_annotation,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn less_than(left: Expr, right: Expr) -> Self {
        Expr::LessThan {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn less_than_or_equal_to(left: Expr, right: Expr) -> Self {
        Expr::LessThanOrEqualTo {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn range(from: Expr, to: Expr) -> Self {
        Expr::Range {
            range: Range::Range {
                from: Box::new(from.clone()),
                to: Box::new(to.clone()),
            },
            inferred_type: InferredType::range(from.inferred_type(), Some(to.inferred_type())),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn range_from(from: Expr) -> Self {
        Expr::Range {
            range: Range::RangeFrom {
                from: Box::new(from.clone()),
            },
            inferred_type: InferredType::range(from.inferred_type(), None),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn range_inclusive(from: Expr, to: Expr) -> Self {
        Expr::Range {
            range: Range::RangeInclusive {
                from: Box::new(from.clone()),
                to: Box::new(to.clone()),
            },
            inferred_type: InferredType::range(from.inferred_type(), Some(to.inferred_type())),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn let_binding(
        name: impl AsRef<str>,
        expr: Expr,
        type_annotation: Option<TypeName>,
    ) -> Self {
        Expr::Let {
            variable_id: VariableId::global(name.as_ref().to_string()),
            type_annotation,
            expr: Box::new(expr),
            source_span: SourceSpan::default(),
            inferred_type: InferredType::tuple(vec![]),
        }
    }

    pub fn let_binding_with_variable_id(
        variable_id: VariableId,
        expr: Expr,
        type_annotation: Option<TypeName>,
    ) -> Self {
        Expr::Let {
            variable_id,
            type_annotation,
            expr: Box::new(expr),
            source_span: SourceSpan::default(),
            inferred_type: InferredType::tuple(vec![]),
        }
    }

    pub fn typed_list_reduce(
        reduce_variable: VariableId,
        iterated_variable: VariableId,
        iterable_expr: Expr,
        init_value_expr: Expr,
        yield_expr: Expr,
        inferred_type: InferredType,
    ) -> Self {
        Expr::ListReduce {
            reduce_variable,
            iterated_variable,
            iterable_expr: Box::new(iterable_expr),
            yield_expr: Box::new(yield_expr),
            init_value_expr: Box::new(init_value_expr),
            inferred_type,
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn list_reduce(
        reduce_variable: VariableId,
        iterated_variable: VariableId,
        iterable_expr: Expr,
        init_value_expr: Expr,
        yield_expr: Expr,
    ) -> Self {
        Expr::typed_list_reduce(
            reduce_variable,
            iterated_variable,
            iterable_expr,
            init_value_expr,
            yield_expr,
            InferredType::unknown(),
        )
    }

    pub fn list_comprehension_typed(
        iterated_variable: VariableId,
        iterable_expr: Expr,
        yield_expr: Expr,
        inferred_type: InferredType,
    ) -> Self {
        Expr::ListComprehension {
            iterated_variable,
            iterable_expr: Box::new(iterable_expr),
            yield_expr: Box::new(yield_expr),
            inferred_type,
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn list_comprehension(
        variable_id: VariableId,
        iterable_expr: Expr,
        yield_expr: Expr,
    ) -> Self {
        Expr::list_comprehension_typed(
            variable_id,
            iterable_expr,
            yield_expr,
            InferredType::list(InferredType::unknown()),
        )
    }

    pub fn literal(value: impl AsRef<str>) -> Self {
        let default_type = DefaultType::String;

        Expr::Literal {
            value: value.as_ref().to_string(),
            inferred_type: InferredType::from(&default_type),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn empty_expr() -> Self {
        Expr::literal("")
    }

    pub fn expr_block(expressions: Vec<Expr>) -> Self {
        let inferred_type = expressions
            .last()
            .map_or(InferredType::unknown(), |e| e.inferred_type());

        Expr::ExprBlock {
            exprs: expressions,
            inferred_type,
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn not(expr: Expr) -> Self {
        Expr::Not {
            expr: Box::new(expr),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn ok(expr: Expr, type_annotation: Option<TypeName>) -> Self {
        let inferred_type = expr.inferred_type();

        Expr::Result {
            expr: Ok(Box::new(expr)),
            type_annotation,
            inferred_type: InferredType::result(Some(inferred_type), Some(InferredType::unknown())),
            source_span: SourceSpan::default(),
        }
    }

    pub fn option(expr: Option<Expr>) -> Self {
        let inferred_type = match &expr {
            Some(expr) => expr.inferred_type(),
            None => InferredType::unknown(),
        };

        Expr::Option {
            expr: expr.map(Box::new),
            type_annotation: None,
            inferred_type: InferredType::option(inferred_type),
            source_span: SourceSpan::default(),
        }
    }

    pub fn or(left: Expr, right: Expr) -> Self {
        Expr::Or {
            lhs: Box::new(left),
            rhs: Box::new(right),
            inferred_type: InferredType::bool(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn pattern_match(expr: Expr, match_arms: Vec<MatchArm>) -> Self {
        Expr::PatternMatch {
            predicate: Box::new(expr),
            match_arms,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn record(expressions: Vec<(String, Expr)>) -> Self {
        let inferred_type = InferredType::record(
            expressions
                .iter()
                .map(|(field_name, expr)| (field_name.to_string(), expr.inferred_type()))
                .collect(),
        );

        Expr::Record {
            exprs: expressions
                .into_iter()
                .map(|(field_name, expr)| (field_name, Box::new(expr)))
                .collect(),
            inferred_type,
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn select_field(
        expr: Expr,
        field: impl AsRef<str>,
        type_annotation: Option<TypeName>,
    ) -> Self {
        Expr::SelectField {
            expr: Box::new(expr),
            field: field.as_ref().to_string(),
            type_annotation,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn select_index(expr: Expr, index: Expr) -> Self {
        Expr::SelectIndex {
            expr: Box::new(expr),
            index: Box::new(index),
            type_annotation: None,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }
    }

    pub fn get_tag(expr: Expr) -> Self {
        Expr::GetTag {
            expr: Box::new(expr),
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn tuple(expressions: Vec<Expr>) -> Self {
        let inferred_type = InferredType::tuple(
            expressions
                .iter()
                .map(|expr| expr.inferred_type())
                .collect(),
        );

        Expr::Tuple {
            exprs: expressions,
            inferred_type,
            source_span: SourceSpan::default(),
            type_annotation: None,
        }
    }

    pub fn sequence(expressions: Vec<Expr>, type_annotation: Option<TypeName>) -> Self {
        let inferred_type = InferredType::list(
            expressions
                .first()
                .map_or(InferredType::unknown(), |x| x.inferred_type()),
        );

        Expr::Sequence {
            exprs: expressions,
            type_annotation,
            inferred_type,
            source_span: SourceSpan::default(),
        }
    }

    pub fn inferred_type_mut(&mut self) -> &mut InferredType {
        match self {
            Expr::Let { inferred_type, .. }
            | Expr::SelectField { inferred_type, .. }
            | Expr::SelectIndex { inferred_type, .. }
            | Expr::Sequence { inferred_type, .. }
            | Expr::Record { inferred_type, .. }
            | Expr::Tuple { inferred_type, .. }
            | Expr::Literal { inferred_type, .. }
            | Expr::Number { inferred_type, .. }
            | Expr::Flags { inferred_type, .. }
            | Expr::Identifier { inferred_type, .. }
            | Expr::Boolean { inferred_type, .. }
            | Expr::Concat { inferred_type, .. }
            | Expr::ExprBlock { inferred_type, .. }
            | Expr::Not { inferred_type, .. }
            | Expr::GreaterThan { inferred_type, .. }
            | Expr::GreaterThanOrEqualTo { inferred_type, .. }
            | Expr::LessThanOrEqualTo { inferred_type, .. }
            | Expr::EqualTo { inferred_type, .. }
            | Expr::Plus { inferred_type, .. }
            | Expr::Minus { inferred_type, .. }
            | Expr::Divide { inferred_type, .. }
            | Expr::Multiply { inferred_type, .. }
            | Expr::LessThan { inferred_type, .. }
            | Expr::Cond { inferred_type, .. }
            | Expr::PatternMatch { inferred_type, .. }
            | Expr::Option { inferred_type, .. }
            | Expr::Result { inferred_type, .. }
            | Expr::Unwrap { inferred_type, .. }
            | Expr::Throw { inferred_type, .. }
            | Expr::GetTag { inferred_type, .. }
            | Expr::And { inferred_type, .. }
            | Expr::Or { inferred_type, .. }
            | Expr::ListComprehension { inferred_type, .. }
            | Expr::ListReduce { inferred_type, .. }
            | Expr::Call { inferred_type, .. }
            | Expr::Range { inferred_type, .. }
            | Expr::InvokeMethodLazy { inferred_type, .. }
            | Expr::Length { inferred_type, .. }
            | Expr::GenerateWorkerName { inferred_type, .. } => &mut *inferred_type,
        }
    }

    pub fn inferred_type(&self) -> InferredType {
        match self {
            Expr::Let { inferred_type, .. }
            | Expr::SelectField { inferred_type, .. }
            | Expr::SelectIndex { inferred_type, .. }
            | Expr::Sequence { inferred_type, .. }
            | Expr::Record { inferred_type, .. }
            | Expr::Tuple { inferred_type, .. }
            | Expr::Literal { inferred_type, .. }
            | Expr::Number { inferred_type, .. }
            | Expr::Flags { inferred_type, .. }
            | Expr::Identifier { inferred_type, .. }
            | Expr::Boolean { inferred_type, .. }
            | Expr::Concat { inferred_type, .. }
            | Expr::ExprBlock { inferred_type, .. }
            | Expr::Not { inferred_type, .. }
            | Expr::GreaterThan { inferred_type, .. }
            | Expr::GreaterThanOrEqualTo { inferred_type, .. }
            | Expr::LessThanOrEqualTo { inferred_type, .. }
            | Expr::EqualTo { inferred_type, .. }
            | Expr::Plus { inferred_type, .. }
            | Expr::Minus { inferred_type, .. }
            | Expr::Divide { inferred_type, .. }
            | Expr::Multiply { inferred_type, .. }
            | Expr::LessThan { inferred_type, .. }
            | Expr::Cond { inferred_type, .. }
            | Expr::PatternMatch { inferred_type, .. }
            | Expr::Option { inferred_type, .. }
            | Expr::Result { inferred_type, .. }
            | Expr::Unwrap { inferred_type, .. }
            | Expr::Throw { inferred_type, .. }
            | Expr::GetTag { inferred_type, .. }
            | Expr::And { inferred_type, .. }
            | Expr::Or { inferred_type, .. }
            | Expr::ListComprehension { inferred_type, .. }
            | Expr::ListReduce { inferred_type, .. }
            | Expr::Call { inferred_type, .. }
            | Expr::Range { inferred_type, .. }
            | Expr::InvokeMethodLazy { inferred_type, .. }
            | Expr::Length { inferred_type, .. }
            | Expr::GenerateWorkerName { inferred_type, .. } => inferred_type.clone(),
        }
    }

    pub fn infer_types(
        &mut self,
        component_dependency: &ComponentDependencies,
        global_variable_type_spec: &Vec<GlobalVariableTypeSpec>,
        custom_instance_spec: &[CustomInstanceSpec],
    ) -> Result<(), RibTypeErrorInternal> {
        use crate::type_inference as ti;

        let (mut arena, mut types, root) = {
            let _p = crate::profile::Scope::new("infer_types: lower");
            crate::expr_arena::lower(self)
        };

        {
            let _p = crate::profile::Scope::new(
                "infer_types: initial_arena + variants/enums/bind_instance",
            );
            ti::initial_arena_phase::run_initial_binding_and_instance_phases(
                root,
                &mut arena,
                &mut types,
                component_dependency,
                global_variable_type_spec.as_slice(),
                custom_instance_spec,
            )?;

            ti::variant_inference::infer_variants_lowered(
                root,
                &mut arena,
                &mut types,
                component_dependency,
            );
            ti::enum_inference::infer_enums_lowered(
                root,
                &mut arena,
                &mut types,
                component_dependency,
            );
            ti::instance_type_binding::bind_instance_types_lowered(root, &arena, &mut types);
        }

        {
            let _p =
                crate::profile::Scope::new("infer_types: fixpoint 1 (instance + worker invokes)");
            ti::arena_type_inference_fix_point(
                |root, arena, types| -> Result<(), RibTypeErrorInternal> {
                    ti::instance_type_binding::bind_instance_types_lowered(root, arena, types);
                    ti::worker_function_invocation::infer_worker_function_invokes_lowered(
                        root,
                        arena,
                        types,
                        component_dependency,
                    )
                },
                root,
                &mut arena,
                &mut types,
            )?;
        }

        {
            let _p = crate::profile::Scope::new("infer_types: infer_function_call_types_lowered");
            ti::call_arguments_inference::infer_function_call_types_lowered(
                root,
                &arena,
                &mut types,
                component_dependency,
                custom_instance_spec,
            )
            .map_err(RibTypeErrorInternal::from)?;
        }

        {
            let _p = crate::profile::Scope::new("infer_types: fixpoint 2 (main scan)");
            ti::arena_type_inference_fix_point(
                |root, arena, types| -> Result<(), RibTypeErrorInternal> {
                    ti::identifier_inference::infer_all_identifiers_lowered(root, arena, types);
                    ti::type_push_down::push_types_down_lowered(root, arena, types)?;
                    ti::identifier_inference::infer_all_identifiers_lowered(root, arena, types);
                    ti::type_pull_up::type_pull_up_lowered(
                        root,
                        arena,
                        types,
                        component_dependency,
                    )?;
                    ti::global_input_inference::infer_global_inputs_lowered(root, arena, types);
                    ti::call_arguments_inference::infer_function_call_types_lowered(
                        root,
                        arena,
                        types,
                        component_dependency,
                        custom_instance_spec,
                    )
                    .map_err(RibTypeErrorInternal::from)?;
                    Ok(())
                },
                root,
                &mut arena,
                &mut types,
            )?;
        }

        {
            let _p = crate::profile::Scope::new("infer_types: final arena refinement + sync/bind");
            ti::type_push_down::push_types_down_lowered(root, &arena, &mut types)?;
            ti::type_pull_up::type_pull_up_lowered(
                root,
                &mut arena,
                &mut types,
                component_dependency,
            )?;
            ti::global_input_inference::infer_global_inputs_lowered(root, &arena, &mut types);
            ti::identifier_inference::infer_all_identifiers_lowered(root, &arena, &mut types);
            ti::instance_type_binding::sync_embedded_worker_exprs_from_calls(
                root, &arena, &mut types,
            );
            ti::instance_type_binding::bind_instance_types_lowered(root, &arena, &mut types);
        }

        {
            let _p = crate::profile::Scope::new("infer_types: type_check");
            type_checker::checker::type_check(root, &arena, &mut types, component_dependency)?;
        }

        {
            let _p = crate::profile::Scope::new("infer_types: unify_types");
            type_inference::unify_types_lowered(root, &arena, &mut types)?;
        }

        {
            let _p = crate::profile::Scope::new("infer_types: rebuild_expr");
            *self = crate::expr_arena::rebuild_expr(root, &arena, &types);
        }

        Ok(())
    }

    pub fn infer_types_initial_phase(
        &mut self,
        component_dependency: &ComponentDependencies,
        global_variable_type_spec: &Vec<GlobalVariableTypeSpec>,
        custom_instance_spec: &[CustomInstanceSpec],
    ) -> Result<(), RibTypeErrorInternal> {
        use crate::type_inference as ti;

        let (mut arena, mut types, root) = crate::expr_arena::lower(self);
        ti::initial_arena_phase::run_initial_binding_and_instance_phases(
            root,
            &mut arena,
            &mut types,
            component_dependency,
            global_variable_type_spec.as_slice(),
            custom_instance_spec,
        )?;
        ti::variant_inference::infer_variants_lowered(
            root,
            &mut arena,
            &mut types,
            component_dependency,
        );
        ti::enum_inference::infer_enums_lowered(root, &mut arena, &mut types, component_dependency);
        *self = crate::expr_arena::rebuild_expr(root, &arena, &types);
        Ok(())
    }

    pub fn bind_type_annotations(&mut self) {
        type_inference::bind_type_annotations(self);
    }

    pub fn merge_inferred_type(&self, new_inferred_type: InferredType) -> Expr {
        let mut expr_copied = self.clone();
        expr_copied.add_infer_type_mut(new_inferred_type);
        expr_copied
    }

    pub fn add_infer_type_mut(&mut self, new_inferred_type: InferredType) {
        match self {
            Expr::Identifier { inferred_type, .. }
            | Expr::Let { inferred_type, .. }
            | Expr::SelectField { inferred_type, .. }
            | Expr::SelectIndex { inferred_type, .. }
            | Expr::Sequence { inferred_type, .. }
            | Expr::Record { inferred_type, .. }
            | Expr::Tuple { inferred_type, .. }
            | Expr::Literal { inferred_type, .. }
            | Expr::Number { inferred_type, .. }
            | Expr::Flags { inferred_type, .. }
            | Expr::Boolean { inferred_type, .. }
            | Expr::Concat { inferred_type, .. }
            | Expr::ExprBlock { inferred_type, .. }
            | Expr::Not { inferred_type, .. }
            | Expr::GreaterThan { inferred_type, .. }
            | Expr::GreaterThanOrEqualTo { inferred_type, .. }
            | Expr::LessThanOrEqualTo { inferred_type, .. }
            | Expr::EqualTo { inferred_type, .. }
            | Expr::Plus { inferred_type, .. }
            | Expr::Minus { inferred_type, .. }
            | Expr::Divide { inferred_type, .. }
            | Expr::Multiply { inferred_type, .. }
            | Expr::LessThan { inferred_type, .. }
            | Expr::Cond { inferred_type, .. }
            | Expr::PatternMatch { inferred_type, .. }
            | Expr::Option { inferred_type, .. }
            | Expr::Result { inferred_type, .. }
            | Expr::Unwrap { inferred_type, .. }
            | Expr::Throw { inferred_type, .. }
            | Expr::GetTag { inferred_type, .. }
            | Expr::And { inferred_type, .. }
            | Expr::Or { inferred_type, .. }
            | Expr::ListComprehension { inferred_type, .. }
            | Expr::ListReduce { inferred_type, .. }
            | Expr::InvokeMethodLazy { inferred_type, .. }
            | Expr::Range { inferred_type, .. }
            | Expr::Length { inferred_type, .. }
            | Expr::GenerateWorkerName { inferred_type, .. }
            | Expr::Call { inferred_type, .. } => {
                if !new_inferred_type.is_unknown() {
                    *inferred_type = inferred_type.merge(new_inferred_type);
                }
            }
        }
    }

    pub fn reset_type(&mut self) {
        type_inference::reset_type_info(self);
    }

    pub fn source_span(&self) -> SourceSpan {
        match self {
            Expr::Identifier { source_span, .. }
            | Expr::Let { source_span, .. }
            | Expr::SelectField { source_span, .. }
            | Expr::SelectIndex { source_span, .. }
            | Expr::Sequence { source_span, .. }
            | Expr::Record { source_span, .. }
            | Expr::Tuple { source_span, .. }
            | Expr::Literal { source_span, .. }
            | Expr::Number { source_span, .. }
            | Expr::Flags { source_span, .. }
            | Expr::Boolean { source_span, .. }
            | Expr::Concat { source_span, .. }
            | Expr::ExprBlock { source_span, .. }
            | Expr::Not { source_span, .. }
            | Expr::GreaterThan { source_span, .. }
            | Expr::GreaterThanOrEqualTo { source_span, .. }
            | Expr::LessThanOrEqualTo { source_span, .. }
            | Expr::EqualTo { source_span, .. }
            | Expr::LessThan { source_span, .. }
            | Expr::Plus { source_span, .. }
            | Expr::Minus { source_span, .. }
            | Expr::Divide { source_span, .. }
            | Expr::Multiply { source_span, .. }
            | Expr::Cond { source_span, .. }
            | Expr::PatternMatch { source_span, .. }
            | Expr::Option { source_span, .. }
            | Expr::Result { source_span, .. }
            | Expr::Unwrap { source_span, .. }
            | Expr::Throw { source_span, .. }
            | Expr::And { source_span, .. }
            | Expr::Or { source_span, .. }
            | Expr::GetTag { source_span, .. }
            | Expr::ListComprehension { source_span, .. }
            | Expr::ListReduce { source_span, .. }
            | Expr::InvokeMethodLazy { source_span, .. }
            | Expr::Range { source_span, .. }
            | Expr::Length { source_span, .. }
            | Expr::Call { source_span, .. }
            | Expr::GenerateWorkerName { source_span, .. } => source_span.clone(),
        }
    }

    pub fn type_annotation(&self) -> &Option<TypeName> {
        match self {
            Expr::Identifier {
                type_annotation, ..
            }
            | Expr::Let {
                type_annotation, ..
            }
            | Expr::SelectField {
                type_annotation, ..
            }
            | Expr::SelectIndex {
                type_annotation, ..
            }
            | Expr::Sequence {
                type_annotation, ..
            }
            | Expr::Record {
                type_annotation, ..
            }
            | Expr::Tuple {
                type_annotation, ..
            }
            | Expr::Literal {
                type_annotation, ..
            }
            | Expr::Number {
                type_annotation, ..
            }
            | Expr::Flags {
                type_annotation, ..
            }
            | Expr::Boolean {
                type_annotation, ..
            }
            | Expr::Concat {
                type_annotation, ..
            }
            | Expr::ExprBlock {
                type_annotation, ..
            }
            | Expr::Not {
                type_annotation, ..
            }
            | Expr::GreaterThan {
                type_annotation, ..
            }
            | Expr::GreaterThanOrEqualTo {
                type_annotation, ..
            }
            | Expr::LessThanOrEqualTo {
                type_annotation, ..
            }
            | Expr::EqualTo {
                type_annotation, ..
            }
            | Expr::LessThan {
                type_annotation, ..
            }
            | Expr::Plus {
                type_annotation, ..
            }
            | Expr::Minus {
                type_annotation, ..
            }
            | Expr::Divide {
                type_annotation, ..
            }
            | Expr::Multiply {
                type_annotation, ..
            }
            | Expr::Cond {
                type_annotation, ..
            }
            | Expr::PatternMatch {
                type_annotation, ..
            }
            | Expr::Option {
                type_annotation, ..
            }
            | Expr::Result {
                type_annotation, ..
            }
            | Expr::Unwrap {
                type_annotation, ..
            }
            | Expr::Throw {
                type_annotation, ..
            }
            | Expr::And {
                type_annotation, ..
            }
            | Expr::Or {
                type_annotation, ..
            }
            | Expr::GetTag {
                type_annotation, ..
            }
            | Expr::ListComprehension {
                type_annotation, ..
            }
            | Expr::ListReduce {
                type_annotation, ..
            }
            | Expr::InvokeMethodLazy {
                type_annotation, ..
            }
            | Expr::Range {
                type_annotation, ..
            }
            | Expr::Length {
                type_annotation, ..
            }
            | Expr::GenerateWorkerName {
                type_annotation, ..
            }
            | Expr::Call {
                type_annotation, ..
            } => type_annotation,
        }
    }

    pub fn with_type_annotation_opt(&self, type_annotation: Option<TypeName>) -> Expr {
        if let Some(type_annotation) = type_annotation {
            self.with_type_annotation(type_annotation)
        } else {
            self.clone()
        }
    }

    pub fn with_type_annotation(&self, type_annotation: TypeName) -> Expr {
        let mut expr_copied = self.clone();
        expr_copied.with_type_annotation_mut(type_annotation);
        expr_copied
    }

    pub fn with_type_annotation_mut(&mut self, type_annotation: TypeName) {
        let new_type_annotation = type_annotation;

        match self {
            Expr::Identifier {
                type_annotation, ..
            }
            | Expr::Let {
                type_annotation, ..
            }
            | Expr::SelectField {
                type_annotation, ..
            }
            | Expr::SelectIndex {
                type_annotation, ..
            }
            | Expr::Sequence {
                type_annotation, ..
            }
            | Expr::Record {
                type_annotation, ..
            }
            | Expr::Tuple {
                type_annotation, ..
            }
            | Expr::Literal {
                type_annotation, ..
            }
            | Expr::Number {
                type_annotation, ..
            }
            | Expr::Flags {
                type_annotation, ..
            }
            | Expr::Boolean {
                type_annotation, ..
            }
            | Expr::Concat {
                type_annotation, ..
            }
            | Expr::ExprBlock {
                type_annotation, ..
            }
            | Expr::Not {
                type_annotation, ..
            }
            | Expr::GreaterThan {
                type_annotation, ..
            }
            | Expr::GreaterThanOrEqualTo {
                type_annotation, ..
            }
            | Expr::LessThanOrEqualTo {
                type_annotation, ..
            }
            | Expr::EqualTo {
                type_annotation, ..
            }
            | Expr::LessThan {
                type_annotation, ..
            }
            | Expr::Plus {
                type_annotation, ..
            }
            | Expr::Minus {
                type_annotation, ..
            }
            | Expr::Divide {
                type_annotation, ..
            }
            | Expr::Multiply {
                type_annotation, ..
            }
            | Expr::Cond {
                type_annotation, ..
            }
            | Expr::PatternMatch {
                type_annotation, ..
            }
            | Expr::Option {
                type_annotation, ..
            }
            | Expr::Result {
                type_annotation, ..
            }
            | Expr::Unwrap {
                type_annotation, ..
            }
            | Expr::Throw {
                type_annotation, ..
            }
            | Expr::And {
                type_annotation, ..
            }
            | Expr::Or {
                type_annotation, ..
            }
            | Expr::GetTag {
                type_annotation, ..
            }
            | Expr::Range {
                type_annotation, ..
            }
            | Expr::ListComprehension {
                type_annotation, ..
            }
            | Expr::ListReduce {
                type_annotation, ..
            }
            | Expr::InvokeMethodLazy {
                type_annotation, ..
            }
            | Expr::Length {
                type_annotation, ..
            }
            | Expr::GenerateWorkerName {
                type_annotation, ..
            }
            | Expr::Call {
                type_annotation, ..
            } => {
                *type_annotation = Some(new_type_annotation);
            }
        }
    }

    pub fn with_source_span(&self, new_source_span: SourceSpan) -> Expr {
        let mut expr_copied = self.clone();
        expr_copied.with_source_span_mut(new_source_span);
        expr_copied
    }

    pub fn with_source_span_mut(&mut self, new_source_span: SourceSpan) {
        match self {
            Expr::Identifier { source_span, .. }
            | Expr::Let { source_span, .. }
            | Expr::SelectField { source_span, .. }
            | Expr::SelectIndex { source_span, .. }
            | Expr::Sequence { source_span, .. }
            | Expr::Number { source_span, .. }
            | Expr::Record { source_span, .. }
            | Expr::Tuple { source_span, .. }
            | Expr::Literal { source_span, .. }
            | Expr::Flags { source_span, .. }
            | Expr::Boolean { source_span, .. }
            | Expr::Concat { source_span, .. }
            | Expr::ExprBlock { source_span, .. }
            | Expr::Not { source_span, .. }
            | Expr::GreaterThan { source_span, .. }
            | Expr::GreaterThanOrEqualTo { source_span, .. }
            | Expr::LessThanOrEqualTo { source_span, .. }
            | Expr::EqualTo { source_span, .. }
            | Expr::LessThan { source_span, .. }
            | Expr::Plus { source_span, .. }
            | Expr::Minus { source_span, .. }
            | Expr::Divide { source_span, .. }
            | Expr::Multiply { source_span, .. }
            | Expr::Cond { source_span, .. }
            | Expr::PatternMatch { source_span, .. }
            | Expr::Option { source_span, .. }
            | Expr::Result { source_span, .. }
            | Expr::Unwrap { source_span, .. }
            | Expr::Throw { source_span, .. }
            | Expr::And { source_span, .. }
            | Expr::Or { source_span, .. }
            | Expr::GetTag { source_span, .. }
            | Expr::Range { source_span, .. }
            | Expr::ListComprehension { source_span, .. }
            | Expr::ListReduce { source_span, .. }
            | Expr::InvokeMethodLazy { source_span, .. }
            | Expr::Length { source_span, .. }
            | Expr::GenerateWorkerName { source_span, .. }
            | Expr::Call { source_span, .. } => {
                *source_span = new_source_span;
            }
        }
    }

    pub fn with_inferred_type(&self, new_inferred_type: InferredType) -> Expr {
        let mut expr_copied = self.clone();
        expr_copied.with_inferred_type_mut(new_inferred_type);
        expr_copied
    }

    // `with_inferred_type` overrides the existing inferred_type and returns a new expr
    // This is different to `merge_inferred_type` where it tries to combine the new inferred type with the existing one.
    pub fn with_inferred_type_mut(&mut self, new_inferred_type: InferredType) {
        match self {
            Expr::Identifier { inferred_type, .. }
            | Expr::Let { inferred_type, .. }
            | Expr::SelectField { inferred_type, .. }
            | Expr::SelectIndex { inferred_type, .. }
            | Expr::Sequence { inferred_type, .. }
            | Expr::Record { inferred_type, .. }
            | Expr::Tuple { inferred_type, .. }
            | Expr::Literal { inferred_type, .. }
            | Expr::Number { inferred_type, .. }
            | Expr::Flags { inferred_type, .. }
            | Expr::Boolean { inferred_type, .. }
            | Expr::Concat { inferred_type, .. }
            | Expr::ExprBlock { inferred_type, .. }
            | Expr::Not { inferred_type, .. }
            | Expr::GreaterThan { inferred_type, .. }
            | Expr::GreaterThanOrEqualTo { inferred_type, .. }
            | Expr::LessThanOrEqualTo { inferred_type, .. }
            | Expr::EqualTo { inferred_type, .. }
            | Expr::LessThan { inferred_type, .. }
            | Expr::Plus { inferred_type, .. }
            | Expr::Minus { inferred_type, .. }
            | Expr::Divide { inferred_type, .. }
            | Expr::Multiply { inferred_type, .. }
            | Expr::Cond { inferred_type, .. }
            | Expr::PatternMatch { inferred_type, .. }
            | Expr::Option { inferred_type, .. }
            | Expr::Result { inferred_type, .. }
            | Expr::Unwrap { inferred_type, .. }
            | Expr::Throw { inferred_type, .. }
            | Expr::And { inferred_type, .. }
            | Expr::Or { inferred_type, .. }
            | Expr::GetTag { inferred_type, .. }
            | Expr::ListComprehension { inferred_type, .. }
            | Expr::ListReduce { inferred_type, .. }
            | Expr::InvokeMethodLazy { inferred_type, .. }
            | Expr::Range { inferred_type, .. }
            | Expr::Length { inferred_type, .. }
            | Expr::GenerateWorkerName { inferred_type, .. }
            | Expr::Call { inferred_type, .. } => {
                *inferred_type = new_inferred_type;
            }
        }
    }

    pub fn visit_expr_nodes_lazy<'a>(&'a mut self, queue: &mut VecDeque<&'a mut Expr>) {
        type_inference::collect_children_mut(self, queue);
    }

    pub fn number_inferred(
        big_decimal: BigDecimal,
        type_annotation: Option<TypeName>,
        inferred_type: InferredType,
    ) -> Expr {
        Expr::Number {
            number: Number { value: big_decimal },
            type_annotation,
            inferred_type,
            source_span: SourceSpan::default(),
        }
    }

    pub fn number(big_decimal: BigDecimal) -> Expr {
        let default_type = DefaultType::from(&big_decimal);
        let inferred_type = InferredType::from(&default_type);

        Expr::number_inferred(big_decimal, None, inferred_type)
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Range {
    Range { from: Box<Expr>, to: Box<Expr> },
    RangeInclusive { from: Box<Expr>, to: Box<Expr> },
    RangeFrom { from: Box<Expr> },
}

impl Range {
    pub fn from(&self) -> Option<&Expr> {
        match self {
            Range::Range { from, .. } => Some(from),
            Range::RangeInclusive { from, .. } => Some(from),
            Range::RangeFrom { from } => Some(from),
        }
    }

    pub fn to(&self) -> Option<&Expr> {
        match self {
            Range::Range { to, .. } => Some(to),
            Range::RangeInclusive { to, .. } => Some(to),
            Range::RangeFrom { .. } => None,
        }
    }

    pub fn inclusive(&self) -> bool {
        matches!(self, Range::RangeInclusive { .. })
    }

    pub fn get_exprs_mut(&mut self) -> Vec<&mut Box<Expr>> {
        match self {
            Range::Range { from, to } => vec![from, to],
            Range::RangeInclusive { from, to } => vec![from, to],
            Range::RangeFrom { from } => vec![from],
        }
    }

    pub fn get_exprs(&self) -> Vec<&Expr> {
        match self {
            Range::Range { from, to } => vec![from.as_ref(), to.as_ref()],
            Range::RangeInclusive { from, to } => vec![from.as_ref(), to.as_ref()],
            Range::RangeFrom { from } => vec![from.as_ref()],
        }
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Ord, PartialOrd)]
pub struct Number {
    pub value: BigDecimal,
}

impl Eq for Number {}

impl Number {
    pub fn to_val(&self, analysed_type: &AnalysedType) -> Option<ValueAndType> {
        match analysed_type {
            AnalysedType::F64(_) => self.value.to_f64().map(|v| v.into_value_and_type()),
            AnalysedType::U64(_) => self.value.to_u64().map(|v| v.into_value_and_type()),
            AnalysedType::F32(_) => self.value.to_f32().map(|v| v.into_value_and_type()),
            AnalysedType::U32(_) => self.value.to_u32().map(|v| v.into_value_and_type()),
            AnalysedType::S32(_) => self.value.to_i32().map(|v| v.into_value_and_type()),
            AnalysedType::S64(_) => self.value.to_i64().map(|v| v.into_value_and_type()),
            AnalysedType::U8(_) => self.value.to_u8().map(|v| v.into_value_and_type()),
            AnalysedType::S8(_) => self.value.to_i8().map(|v| v.into_value_and_type()),
            AnalysedType::U16(_) => self.value.to_u16().map(|v| v.into_value_and_type()),
            AnalysedType::S16(_) => self.value.to_i16().map(|v| v.into_value_and_type()),
            _ => None,
        }
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct MatchArm {
    pub arm_pattern: ArmPattern,
    pub arm_resolution_expr: Box<Expr>,
}

impl MatchArm {
    pub fn new(arm_pattern: ArmPattern, arm_resolution: Expr) -> MatchArm {
        MatchArm {
            arm_pattern,
            arm_resolution_expr: Box::new(arm_resolution),
        }
    }
}

#[derive(Debug, Hash, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub enum ArmPattern {
    WildCard,
    As(String, Box<ArmPattern>),
    Constructor(String, Vec<ArmPattern>),
    TupleConstructor(Vec<ArmPattern>),
    RecordConstructor(Vec<(String, ArmPattern)>),
    ListConstructor(Vec<ArmPattern>),
    Literal(Box<Expr>),
}

impl ArmPattern {
    pub fn is_wildcard(&self) -> bool {
        matches!(self, ArmPattern::WildCard)
    }

    pub fn is_literal_identifier(&self) -> bool {
        matches!(self, ArmPattern::Literal(expr) if expr.is_identifier())
    }

    pub fn constructor(name: &str, patterns: Vec<ArmPattern>) -> ArmPattern {
        ArmPattern::Constructor(name.to_string(), patterns)
    }

    pub fn literal(expr: Expr) -> ArmPattern {
        ArmPattern::Literal(Box::new(expr))
    }

    pub fn get_expr_literals_mut(&mut self) -> Vec<&mut Box<Expr>> {
        match self {
            ArmPattern::Literal(expr) => vec![expr],
            ArmPattern::As(_, pattern) => pattern.get_expr_literals_mut(),
            ArmPattern::Constructor(_, patterns) => {
                let mut result = vec![];
                for pattern in patterns {
                    result.extend(pattern.get_expr_literals_mut());
                }
                result
            }
            ArmPattern::TupleConstructor(patterns) => {
                let mut result = vec![];
                for pattern in patterns {
                    result.extend(pattern.get_expr_literals_mut());
                }
                result
            }
            ArmPattern::RecordConstructor(patterns) => {
                let mut result = vec![];
                for (_, pattern) in patterns {
                    result.extend(pattern.get_expr_literals_mut());
                }
                result
            }
            ArmPattern::ListConstructor(patterns) => {
                let mut result = vec![];
                for pattern in patterns {
                    result.extend(pattern.get_expr_literals_mut());
                }
                result
            }
            ArmPattern::WildCard => vec![],
        }
    }

    pub fn get_expr_literals(&self) -> Vec<&Expr> {
        match self {
            ArmPattern::Literal(expr) => vec![expr.as_ref()],
            ArmPattern::As(_, pattern) => pattern.get_expr_literals(),
            ArmPattern::Constructor(_, patterns) => {
                let mut result = vec![];
                for pattern in patterns {
                    result.extend(pattern.get_expr_literals());
                }
                result
            }
            ArmPattern::TupleConstructor(patterns) => {
                let mut result = vec![];
                for pattern in patterns {
                    result.extend(pattern.get_expr_literals());
                }
                result
            }
            ArmPattern::RecordConstructor(patterns) => {
                let mut result = vec![];
                for (_, pattern) in patterns {
                    result.extend(pattern.get_expr_literals());
                }
                result
            }
            ArmPattern::ListConstructor(patterns) => {
                let mut result = vec![];
                for pattern in patterns {
                    result.extend(pattern.get_expr_literals());
                }
                result
            }
            ArmPattern::WildCard => vec![],
        }
    }
    // Helper to construct ok(v). Cannot be used if there is nested constructors such as ok(some(v)))
    pub fn ok(binding_variable: &str) -> ArmPattern {
        ArmPattern::Literal(Box::new(Expr::Result {
            expr: Ok(Box::new(Expr::Identifier {
                variable_id: VariableId::global(binding_variable.to_string()),
                type_annotation: None,
                inferred_type: InferredType::unknown(),
                source_span: SourceSpan::default(),
            })),
            type_annotation: None,
            inferred_type: InferredType::result(
                Some(InferredType::unknown()),
                Some(InferredType::unknown()),
            ),
            source_span: SourceSpan::default(),
        }))
    }

    // Helper to construct err(v). Cannot be used if there is nested constructors such as err(some(v)))
    pub fn err(binding_variable: &str) -> ArmPattern {
        ArmPattern::Literal(Box::new(Expr::Result {
            expr: Err(Box::new(Expr::Identifier {
                variable_id: VariableId::global(binding_variable.to_string()),
                type_annotation: None,
                inferred_type: InferredType::unknown(),
                source_span: SourceSpan::default(),
            })),
            type_annotation: None,
            inferred_type: InferredType::result(
                Some(InferredType::unknown()),
                Some(InferredType::unknown()),
            ),
            source_span: SourceSpan::default(),
        }))
    }

    // Helper to construct some(v). Cannot be used if there is nested constructors such as some(ok(v)))
    pub fn some(binding_variable: &str) -> ArmPattern {
        ArmPattern::Literal(Box::new(Expr::Option {
            expr: Some(Box::new(Expr::Identifier {
                variable_id: VariableId::local_with_no_id(binding_variable),
                type_annotation: None,
                inferred_type: InferredType::unknown(),
                source_span: SourceSpan::default(),
            })),
            type_annotation: None,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }))
    }

    pub fn none() -> ArmPattern {
        ArmPattern::Literal(Box::new(Expr::Option {
            expr: None,
            type_annotation: None,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }))
    }

    pub fn identifier(binding_variable: &str) -> ArmPattern {
        ArmPattern::Literal(Box::new(Expr::Identifier {
            variable_id: VariableId::global(binding_variable.to_string()),
            type_annotation: None,
            inferred_type: InferredType::unknown(),
            source_span: SourceSpan::default(),
        }))
    }
    pub fn custom_constructor(name: &str, args: Vec<ArmPattern>) -> ArmPattern {
        ArmPattern::Constructor(name.to_string(), args)
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", text::to_string(self).unwrap())
    }
}

impl Display for ArmPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", text::to_string_arm_pattern(self).unwrap())
    }
}

impl<'de> Deserialize<'de> for Expr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(expr_string) => match from_string(expr_string.as_str()) {
                Ok(expr) => Ok(expr),
                Err(message) => Err(serde::de::Error::custom(message.to_string())),
            },

            e => Err(serde::de::Error::custom(format!(
                "Failed to deserialize expression {e}"
            ))),
        }
    }
}

impl Serialize for Expr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match text::to_string(self) {
            Ok(value) => Value::serialize(&Value::String(value), serializer),
            Err(error) => Err(serde::ser::Error::custom(error.to_string())),
        }
    }
}

fn find_expr(expr: &mut Expr, source_span: &SourceSpan) -> Option<Expr> {
    let mut expr = expr.clone();
    let mut found = None;

    type_inference::visit_post_order_rev_mut(&mut expr, &mut |current| {
        if found.is_none() {
            let span = current.source_span();
            if source_span.eq(&span) {
                found = Some(current.clone());
            }
        }
    });

    found
}
