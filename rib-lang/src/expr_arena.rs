use la_arena::{Arena, Idx};
use std::collections::HashMap;
use std::fmt;

use crate::call_type::{CallType, InstanceCreationType, InstanceIdentifier};
use crate::expr::{ArmPattern, Expr, MatchArm, Number, Range};
use crate::parser::type_name::TypeName;
use crate::rib_source_span::SourceSpan;
use crate::{InferredType, VariableId};

// ---------------------------------------------------------------------------
// Index types
// ---------------------------------------------------------------------------

/// Stable index into [`ExprArena`] identifying a single expression node.
pub type ExprId = Idx<ExprNode>;

/// Stable index into the arm-pattern arena.
pub type ArmPatternId = Idx<ArmPatternNode>;

// ---------------------------------------------------------------------------
// Structural node — no InferredType, children are IDs
// ---------------------------------------------------------------------------

/// A single expression node stripped of its inferred type.
/// Child expressions are referenced by [`ExprId`] rather than owned.
#[derive(Debug, Clone)]
pub struct ExprNode {
    pub kind: ExprKind,
    pub source_span: SourceSpan,
    pub type_annotation: Option<TypeName>,
}

/// The shape of an expression node.  
/// This mirrors [`Expr`] exactly, replacing every `Box<Expr>` / `Vec<Expr>`
/// with `ExprId` / `Vec<ExprId>` and every `ArmPattern` with `ArmPatternId`.
#[derive(Debug, Clone)]
pub enum ExprKind {
    Let {
        variable_id: VariableId,
        expr: ExprId,
    },
    SelectField {
        expr: ExprId,
        field: String,
    },
    SelectIndex {
        expr: ExprId,
        index: ExprId,
    },
    Sequence {
        exprs: Vec<ExprId>,
    },
    Range {
        range: RangeKind,
    },
    Record {
        fields: Vec<(String, ExprId)>,
    },
    Tuple {
        exprs: Vec<ExprId>,
    },
    Literal {
        value: String,
    },
    Number {
        number: Number,
    },
    Flags {
        flags: Vec<String>,
    },
    Identifier {
        variable_id: VariableId,
    },
    Boolean {
        value: bool,
    },
    Concat {
        exprs: Vec<ExprId>,
    },
    ExprBlock {
        exprs: Vec<ExprId>,
    },
    Not {
        expr: ExprId,
    },
    GreaterThan {
        lhs: ExprId,
        rhs: ExprId,
    },
    GreaterThanOrEqualTo {
        lhs: ExprId,
        rhs: ExprId,
    },
    LessThanOrEqualTo {
        lhs: ExprId,
        rhs: ExprId,
    },
    EqualTo {
        lhs: ExprId,
        rhs: ExprId,
    },
    LessThan {
        lhs: ExprId,
        rhs: ExprId,
    },
    And {
        lhs: ExprId,
        rhs: ExprId,
    },
    Or {
        lhs: ExprId,
        rhs: ExprId,
    },
    Plus {
        lhs: ExprId,
        rhs: ExprId,
    },
    Minus {
        lhs: ExprId,
        rhs: ExprId,
    },
    Multiply {
        lhs: ExprId,
        rhs: ExprId,
    },
    Divide {
        lhs: ExprId,
        rhs: ExprId,
    },
    Cond {
        cond: ExprId,
        lhs: ExprId,
        rhs: ExprId,
    },
    PatternMatch {
        predicate: ExprId,
        match_arms: Vec<MatchArmNode>,
    },
    Option {
        expr: Option<ExprId>,
    },
    Result {
        expr: ResultExprKind,
    },
    Call {
        call_type: CallTypeNode,
        args: Vec<ExprId>,
    },
    InvokeMethodLazy {
        lhs: ExprId,
        method: String,
        args: Vec<ExprId>,
    },
    Unwrap {
        expr: ExprId,
    },
    Throw {
        message: String,
    },
    GetTag {
        expr: ExprId,
    },
    ListComprehension {
        iterated_variable: VariableId,
        iterable_expr: ExprId,
        yield_expr: ExprId,
    },
    ListReduce {
        reduce_variable: VariableId,
        iterated_variable: VariableId,
        iterable_expr: ExprId,
        init_value_expr: ExprId,
        yield_expr: ExprId,
    },
    Length {
        expr: ExprId,
    },
    GenerateWorkerName {
        variable_id: Option<VariableId>,
    },
}

/// Arena-friendly version of [`Range`].
#[derive(Debug, Clone)]
pub enum RangeKind {
    Range { from: ExprId, to: ExprId },
    RangeInclusive { from: ExprId, to: ExprId },
    RangeFrom { from: ExprId },
}

/// Arena-friendly version of `Result<Box<Expr>, Box<Expr>>` in [`Expr::Result`].
#[derive(Debug, Clone)]
pub enum ResultExprKind {
    Ok(ExprId),
    Err(ExprId),
}

/// Arena-friendly version of [`CallType`].
/// [`InstanceIdentifier`] embeds `Option<Box<Expr>>` for the worker name;
/// we replace that with `Option<ExprId>`.
#[derive(Debug, Clone)]
pub enum CallTypeNode {
    Function {
        component_info: Option<crate::ComponentDependencyKey>,
        instance_identifier: Option<InstanceIdentifierNode>,
        function_name: crate::DynamicParsedFunctionName,
    },
    VariantConstructor(String),
    EnumConstructor(String),
    InstanceCreation(InstanceCreationNode),
}

impl CallTypeNode {
    /// Same notion as [`crate::CallType::is_resource_method`], without lowering to [`crate::CallType`].
    pub fn is_resource_method(&self) -> bool {
        match self {
            CallTypeNode::Function { function_name, .. } => function_name
                .to_parsed_function_name()
                .function
                .resource_method_name()
                .is_some(),
            _ => false,
        }
    }
}

impl fmt::Display for CallTypeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallTypeNode::Function { function_name, .. } => write!(f, "{function_name}"),
            CallTypeNode::VariantConstructor(name) => write!(f, "{name}"),
            CallTypeNode::EnumConstructor(name) => write!(f, "{name}"),
            CallTypeNode::InstanceCreation(InstanceCreationNode::WitWorker { .. }) => {
                write!(f, "instance")
            }
            CallTypeNode::InstanceCreation(InstanceCreationNode::WitResource {
                resource_name,
                ..
            }) => write!(f, "{}", resource_name.resource_name),
        }
    }
}

#[derive(Debug, Clone)]
pub enum InstanceIdentifierNode {
    WitWorker {
        variable_id: Option<VariableId>,
        worker_name: Option<ExprId>,
    },
    WitResource {
        variable_id: Option<VariableId>,
        worker_name: Option<ExprId>,
        resource_name: String,
    },
}

#[derive(Debug, Clone)]
pub enum InstanceCreationNode {
    WitWorker {
        component_info: Option<crate::ComponentDependencyKey>,
        worker_name: Option<ExprId>,
    },
    WitResource {
        component_info: Option<crate::ComponentDependencyKey>,
        module: Option<InstanceIdentifierNode>,
        resource_name: crate::FullyQualifiedResourceConstructor,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArmNode {
    pub arm_pattern: ArmPatternId,
    pub arm_resolution_expr: ExprId,
}

#[derive(Debug, Clone)]
pub enum ArmPatternNode {
    WildCard,
    As(String, ArmPatternId),
    Constructor(String, Vec<ArmPatternId>),
    TupleConstructor(Vec<ArmPatternId>),
    RecordConstructor(Vec<(String, ArmPatternId)>),
    ListConstructor(Vec<ArmPatternId>),
    /// A literal pattern (identifier, option node, result node, etc.) is just
    /// an expression embedded in the pattern — we keep the `ExprId` reference.
    Literal(ExprId),
}

#[derive(Debug, Clone)]
pub struct TypeTable {
    types: HashMap<ExprId, InferredType>,
}

impl TypeTable {
    pub fn new() -> Self {
        TypeTable {
            types: HashMap::new(),
        }
    }

    pub fn get(&self, id: ExprId) -> &InferredType {
        self.types
            .get(&id)
            .expect("TypeTable: ExprId not found — was it allocated in this arena?")
    }

    pub fn get_opt(&self, id: ExprId) -> Option<&InferredType> {
        self.types.get(&id)
    }

    pub fn set(&mut self, id: ExprId, ty: InferredType) {
        self.types.insert(id, ty);
    }

    /// Returns a snapshot of all types. The snapshot is a `Vec` ordered by
    /// insertion — used for fix-point convergence checks (O(n) clone of
    /// `InferredType` values, **not** a tree clone).
    pub fn snapshot(&self) -> Vec<(ExprId, InferredType)> {
        self.types.iter().map(|(k, v)| (*k, v.clone())).collect()
    }

    /// Returns `true` if this `TypeTable` has the same types as `other`.
    /// Used to detect convergence in the fix-point loop.
    pub fn same_as(&self, other: &TypeTable) -> bool {
        if self.types.len() != other.types.len() {
            return false;
        }
        self.types
            .iter()
            .all(|(id, ty)| other.types.get(id) == Some(ty))
    }

    /// Returns `true` if the current state matches a previously taken snapshot.
    pub fn same_as_snapshot(&self, snapshot: &[(ExprId, InferredType)]) -> bool {
        if self.types.len() != snapshot.len() {
            return false;
        }
        snapshot
            .iter()
            .all(|(id, ty)| self.types.get(id) == Some(ty))
    }
}

impl Default for TypeTable {
    fn default() -> Self {
        TypeTable::new()
    }
}

// ---------------------------------------------------------------------------
// ExprArena — structural storage
// ---------------------------------------------------------------------------

/// Owns all [`ExprNode`]s and [`ArmPatternNode`]s for a single compiled Rib
/// expression.  Children are referenced by index, never by pointer.
#[derive(Debug, Default)]
pub struct ExprArena {
    pub exprs: Arena<ExprNode>,
    pub patterns: Arena<ArmPatternNode>,
}

impl ExprArena {
    pub fn new() -> Self {
        ExprArena::default()
    }

    /// Allocate a new expression node, returning its stable `ExprId`.
    pub fn alloc_expr(&mut self, node: ExprNode) -> ExprId {
        self.exprs.alloc(node)
    }

    /// Allocate a new arm-pattern node, returning its stable `ArmPatternId`.
    pub fn alloc_pattern(&mut self, node: ArmPatternNode) -> ArmPatternId {
        self.patterns.alloc(node)
    }

    /// Look up an expression node by id.
    pub fn expr(&self, id: ExprId) -> &ExprNode {
        &self.exprs[id]
    }

    /// Look up an expression node mutably by id.
    /// Used by passes that perform structural mutations (e.g. converting an
    /// `Identifier` node into a `Call` node during enum/variant inference).
    pub fn expr_mut(&mut self, id: ExprId) -> &mut ExprNode {
        &mut self.exprs[id]
    }

    /// Look up a pattern node by id.
    pub fn pattern(&self, id: ArmPatternId) -> &ArmPatternNode {
        &self.patterns[id]
    }

    /// Look up a pattern node mutably by id.
    pub fn pattern_mut(&mut self, id: ArmPatternId) -> &mut ArmPatternNode {
        &mut self.patterns[id]
    }
}

/// Lower the recursive `Expr` tree into an arena representation.
///
/// Returns `(arena, type_table, root_id)`.  The caller can discard the old
/// `Expr` once this call returns.
///
/// # Panics
/// Does not panic under normal circumstances.
pub fn lower(expr: &Expr) -> (ExprArena, TypeTable, ExprId) {
    let mut arena = ExprArena::new();
    let mut types = TypeTable::new();
    let root = lower_expr(expr, &mut arena, &mut types);
    (arena, types, root)
}

/// Lower `expr` into an existing arena, allocating fresh [`ExprId`]s and wiring
/// child pointers within `arena` only (unlike naïvely cloning nodes from a
/// separate lowered arena, which would leave stale child ids).
pub fn lower_into(arena: &mut ExprArena, types: &mut TypeTable, expr: &Expr) -> ExprId {
    lower_expr(expr, arena, types)
}

// ---------------------------------------------------------------------------
// Rebuild: ExprArena + TypeTable  →  Expr tree
// ---------------------------------------------------------------------------

/// Reconstruct a full `Expr` tree from the arena, applying the current
/// `TypeTable` values.  This is the inverse of [`lower`] and handles
/// structural mutations (e.g. `InvokeMethodLazy` → `Call`) that occurred
/// during arena-based inference.
pub fn rebuild_expr(id: ExprId, arena: &ExprArena, types: &TypeTable) -> Expr {
    let node = arena.expr(id);
    let inferred = types
        .get_opt(id)
        .cloned()
        .unwrap_or_else(InferredType::unknown);
    let span = node.source_span.clone();
    let annotation = node.type_annotation.clone();

    match &node.kind.clone() {
        ExprKind::Let {
            variable_id,
            expr: rhs_id,
        } => Expr::Let {
            variable_id: variable_id.clone(),
            type_annotation: annotation,
            expr: Box::new(rebuild_expr(*rhs_id, arena, types)),
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::SelectField {
            expr: inner_id,
            field,
        } => Expr::SelectField {
            expr: Box::new(rebuild_expr(*inner_id, arena, types)),
            field: field.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::SelectIndex {
            expr: e_id,
            index: i_id,
        } => Expr::SelectIndex {
            expr: Box::new(rebuild_expr(*e_id, arena, types)),
            index: Box::new(rebuild_expr(*i_id, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Sequence { exprs } => Expr::Sequence {
            exprs: exprs
                .iter()
                .map(|&e| rebuild_expr(e, arena, types))
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Tuple { exprs } => Expr::Tuple {
            exprs: exprs
                .iter()
                .map(|&e| rebuild_expr(e, arena, types))
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Concat { exprs } => Expr::Concat {
            exprs: exprs
                .iter()
                .map(|&e| rebuild_expr(e, arena, types))
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::ExprBlock { exprs } => Expr::ExprBlock {
            exprs: exprs
                .iter()
                .map(|&e| rebuild_expr(e, arena, types))
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Record { fields } => Expr::Record {
            exprs: fields
                .iter()
                .map(|(name, e)| (name.clone(), Box::new(rebuild_expr(*e, arena, types))))
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Range { range } => {
            let range_val = match range {
                RangeKind::Range { from, to } => crate::expr::Range::Range {
                    from: Box::new(rebuild_expr(*from, arena, types)),
                    to: Box::new(rebuild_expr(*to, arena, types)),
                },
                RangeKind::RangeInclusive { from, to } => crate::expr::Range::RangeInclusive {
                    from: Box::new(rebuild_expr(*from, arena, types)),
                    to: Box::new(rebuild_expr(*to, arena, types)),
                },
                RangeKind::RangeFrom { from } => crate::expr::Range::RangeFrom {
                    from: Box::new(rebuild_expr(*from, arena, types)),
                },
            };
            Expr::Range {
                range: range_val,
                type_annotation: annotation,
                inferred_type: inferred,
                source_span: span,
            }
        }
        ExprKind::Literal { value } => Expr::Literal {
            value: value.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Number { number } => Expr::Number {
            number: number.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Flags { flags } => Expr::Flags {
            flags: flags.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Identifier { variable_id } => Expr::Identifier {
            variable_id: variable_id.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Boolean { value } => Expr::Boolean {
            value: *value,
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Not { expr: inner } => Expr::Not {
            expr: Box::new(rebuild_expr(*inner, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Length { expr: inner } => Expr::Length {
            expr: Box::new(rebuild_expr(*inner, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Unwrap { expr: inner } => Expr::Unwrap {
            expr: Box::new(rebuild_expr(*inner, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::GetTag { expr: inner } => Expr::GetTag {
            expr: Box::new(rebuild_expr(*inner, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::GreaterThan { lhs, rhs } => Expr::GreaterThan {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::GreaterThanOrEqualTo { lhs, rhs } => Expr::GreaterThanOrEqualTo {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::LessThanOrEqualTo { lhs, rhs } => Expr::LessThanOrEqualTo {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::EqualTo { lhs, rhs } => Expr::EqualTo {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::LessThan { lhs, rhs } => Expr::LessThan {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::And { lhs, rhs } => Expr::And {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Or { lhs, rhs } => Expr::Or {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Plus { lhs, rhs } => Expr::Plus {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Minus { lhs, rhs } => Expr::Minus {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Multiply { lhs, rhs } => Expr::Multiply {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Divide { lhs, rhs } => Expr::Divide {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Cond { cond, lhs, rhs } => Expr::Cond {
            cond: Box::new(rebuild_expr(*cond, arena, types)),
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            rhs: Box::new(rebuild_expr(*rhs, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::PatternMatch {
            predicate,
            match_arms,
        } => Expr::PatternMatch {
            predicate: Box::new(rebuild_expr(*predicate, arena, types)),
            match_arms: match_arms
                .iter()
                .map(|arm| crate::expr::MatchArm {
                    arm_pattern: rebuild_arm_pattern(arm.arm_pattern, arena, types),
                    arm_resolution_expr: Box::new(rebuild_expr(
                        arm.arm_resolution_expr,
                        arena,
                        types,
                    )),
                })
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Option { expr: None } => Expr::Option {
            expr: None,
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Option { expr: Some(inner) } => Expr::Option {
            expr: Some(Box::new(rebuild_expr(*inner, arena, types))),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Result {
            expr: ResultExprKind::Ok(inner),
        } => Expr::Result {
            expr: Ok(Box::new(rebuild_expr(*inner, arena, types))),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Result {
            expr: ResultExprKind::Err(inner),
        } => Expr::Result {
            expr: Err(Box::new(rebuild_expr(*inner, arena, types))),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Call { call_type, args } => {
            let old_call_type = rebuild_call_type(call_type, arena, types);
            Expr::Call {
                call_type: old_call_type,
                args: args
                    .iter()
                    .map(|&a| rebuild_expr(a, arena, types))
                    .collect(),
                type_annotation: annotation,
                inferred_type: inferred,
                source_span: span,
            }
        }
        ExprKind::InvokeMethodLazy { lhs, method, args } => Expr::InvokeMethodLazy {
            lhs: Box::new(rebuild_expr(*lhs, arena, types)),
            method: method.clone(),
            args: args
                .iter()
                .map(|&a| rebuild_expr(a, arena, types))
                .collect(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::Throw { message } => Expr::Throw {
            message: message.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::ListComprehension {
            iterated_variable,
            iterable_expr,
            yield_expr,
        } => Expr::ListComprehension {
            iterated_variable: iterated_variable.clone(),
            iterable_expr: Box::new(rebuild_expr(*iterable_expr, arena, types)),
            yield_expr: Box::new(rebuild_expr(*yield_expr, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::ListReduce {
            reduce_variable,
            iterated_variable,
            iterable_expr,
            init_value_expr,
            yield_expr,
        } => Expr::ListReduce {
            reduce_variable: reduce_variable.clone(),
            iterated_variable: iterated_variable.clone(),
            iterable_expr: Box::new(rebuild_expr(*iterable_expr, arena, types)),
            init_value_expr: Box::new(rebuild_expr(*init_value_expr, arena, types)),
            yield_expr: Box::new(rebuild_expr(*yield_expr, arena, types)),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
        ExprKind::GenerateWorkerName { variable_id } => Expr::GenerateWorkerName {
            variable_id: variable_id.clone(),
            type_annotation: annotation,
            inferred_type: inferred,
            source_span: span,
        },
    }
}

pub(crate) fn rebuild_arm_pattern(
    pat_id: ArmPatternId,
    arena: &ExprArena,
    types: &TypeTable,
) -> crate::expr::ArmPattern {
    match arena.pattern(pat_id) {
        ArmPatternNode::WildCard => crate::expr::ArmPattern::WildCard,
        ArmPatternNode::As(name, inner) => {
            let inner = *inner;
            crate::expr::ArmPattern::As(
                name.clone(),
                Box::new(rebuild_arm_pattern(inner, arena, types)),
            )
        }
        ArmPatternNode::Literal(expr_id) => {
            crate::expr::ArmPattern::Literal(Box::new(rebuild_expr(*expr_id, arena, types)))
        }
        ArmPatternNode::Constructor(name, children) => {
            let children = children.clone();
            crate::expr::ArmPattern::Constructor(
                name.clone(),
                children
                    .iter()
                    .map(|&c| rebuild_arm_pattern(c, arena, types))
                    .collect(),
            )
        }
        ArmPatternNode::TupleConstructor(children) => {
            let children = children.clone();
            crate::expr::ArmPattern::TupleConstructor(
                children
                    .iter()
                    .map(|&c| rebuild_arm_pattern(c, arena, types))
                    .collect(),
            )
        }
        ArmPatternNode::ListConstructor(children) => {
            let children = children.clone();
            crate::expr::ArmPattern::ListConstructor(
                children
                    .iter()
                    .map(|&c| rebuild_arm_pattern(c, arena, types))
                    .collect(),
            )
        }
        ArmPatternNode::RecordConstructor(fields) => {
            let fields = fields.clone();
            crate::expr::ArmPattern::RecordConstructor(
                fields
                    .iter()
                    .map(|(name, c)| (name.clone(), rebuild_arm_pattern(*c, arena, types)))
                    .collect(),
            )
        }
    }
}

pub(crate) fn rebuild_call_type(
    call_type: &CallTypeNode,
    arena: &ExprArena,
    types: &TypeTable,
) -> crate::call_type::CallType {
    use crate::call_type::{CallType, InstanceCreationType};
    match call_type {
        CallTypeNode::Function {
            component_info,
            instance_identifier,
            function_name,
        } => CallType::Function {
            component_info: component_info.clone(),
            instance_identifier: instance_identifier
                .as_ref()
                .map(|ii| Box::new(rebuild_instance_identifier(ii, arena, types))),
            function_name: function_name.clone(),
        },
        CallTypeNode::VariantConstructor(name) => CallType::VariantConstructor(name.clone()),
        CallTypeNode::EnumConstructor(name) => CallType::EnumConstructor(name.clone()),
        CallTypeNode::InstanceCreation(creation) => {
            let ict = match creation {
                InstanceCreationNode::WitWorker {
                    component_info,
                    worker_name,
                } => InstanceCreationType::WitWorker {
                    component_info: component_info.clone(),
                    worker_name: worker_name
                        .map(|wn_id| Box::new(rebuild_expr(wn_id, arena, types))),
                },
                InstanceCreationNode::WitResource {
                    component_info,
                    module,
                    resource_name,
                } => InstanceCreationType::WitResource {
                    component_info: component_info.clone(),
                    module: module
                        .as_ref()
                        .map(|m| rebuild_instance_identifier(m, arena, types)),
                    resource_name: resource_name.clone(),
                },
            };
            CallType::InstanceCreation(ict)
        }
    }
}

fn rebuild_instance_identifier(
    ii: &InstanceIdentifierNode,
    arena: &ExprArena,
    types: &TypeTable,
) -> crate::call_type::InstanceIdentifier {
    use crate::call_type::InstanceIdentifier;
    match ii {
        InstanceIdentifierNode::WitWorker {
            variable_id,
            worker_name,
        } => InstanceIdentifier::WitWorker {
            variable_id: variable_id.clone(),
            worker_name: worker_name.map(|wn_id| Box::new(rebuild_expr(wn_id, arena, types))),
        },
        InstanceIdentifierNode::WitResource {
            variable_id,
            worker_name,
            resource_name,
        } => InstanceIdentifier::WitResource {
            variable_id: variable_id.clone(),
            worker_name: worker_name.map(|wn_id| Box::new(rebuild_expr(wn_id, arena, types))),
            resource_name: resource_name.clone(),
        },
    }
}

fn lower_expr(expr: &Expr, arena: &mut ExprArena, types: &mut TypeTable) -> ExprId {
    let (kind, span, annotation, inferred) = match expr {
        Expr::Let {
            variable_id,
            type_annotation,
            expr,
            inferred_type,
            source_span,
        } => {
            let child = lower_expr(expr, arena, types);
            (
                ExprKind::Let {
                    variable_id: variable_id.clone(),
                    expr: child,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::SelectField {
            expr,
            field,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let child = lower_expr(expr, arena, types);
            (
                ExprKind::SelectField {
                    expr: child,
                    field: field.clone(),
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::SelectIndex {
            expr,
            index,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let e = lower_expr(expr, arena, types);
            let i = lower_expr(index, arena, types);
            (
                ExprKind::SelectIndex { expr: e, index: i },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Sequence {
            exprs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let ids = exprs.iter().map(|e| lower_expr(e, arena, types)).collect();
            (
                ExprKind::Sequence { exprs: ids },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Range {
            range,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let range_kind = lower_range(range, arena, types);
            (
                ExprKind::Range { range: range_kind },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Record {
            exprs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let fields = exprs
                .iter()
                .map(|(name, e)| (name.clone(), lower_expr(e, arena, types)))
                .collect();
            (
                ExprKind::Record { fields },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Tuple {
            exprs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let ids = exprs.iter().map(|e| lower_expr(e, arena, types)).collect();
            (
                ExprKind::Tuple { exprs: ids },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Literal {
            value,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::Literal {
                value: value.clone(),
            },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),

        Expr::Number {
            number,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::Number {
                number: number.clone(),
            },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),

        Expr::Flags {
            flags,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::Flags {
                flags: flags.clone(),
            },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),

        Expr::Identifier {
            variable_id,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::Identifier {
                variable_id: variable_id.clone(),
            },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),

        Expr::Boolean {
            value,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::Boolean { value: *value },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),

        Expr::Concat {
            exprs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let ids = exprs.iter().map(|e| lower_expr(e, arena, types)).collect();
            (
                ExprKind::Concat { exprs: ids },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::ExprBlock {
            exprs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let ids = exprs.iter().map(|e| lower_expr(e, arena, types)).collect();
            (
                ExprKind::ExprBlock { exprs: ids },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Not {
            expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let child = lower_expr(expr, arena, types);
            (
                ExprKind::Not { expr: child },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::GreaterThan {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::GreaterThan { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::GreaterThanOrEqualTo {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::GreaterThanOrEqualTo { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::LessThanOrEqualTo {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::LessThanOrEqualTo { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::EqualTo {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::EqualTo { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::LessThan {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::LessThan { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::And {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::And { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Or {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::Or { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Plus {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::Plus { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Minus {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::Minus { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Multiply {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::Multiply { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Divide {
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::Divide { lhs: l, rhs: r },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Cond {
            cond,
            lhs,
            rhs,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let c = lower_expr(cond, arena, types);
            let l = lower_expr(lhs, arena, types);
            let r = lower_expr(rhs, arena, types);
            (
                ExprKind::Cond {
                    cond: c,
                    lhs: l,
                    rhs: r,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::PatternMatch {
            predicate,
            match_arms,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let pred = lower_expr(predicate, arena, types);
            let arms = match_arms
                .iter()
                .map(|arm| lower_match_arm(arm, arena, types))
                .collect();
            (
                ExprKind::PatternMatch {
                    predicate: pred,
                    match_arms: arms,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Option {
            expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let child = expr.as_ref().map(|e| lower_expr(e, arena, types));
            (
                ExprKind::Option { expr: child },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Result {
            expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let kind = match expr {
                Ok(e) => ResultExprKind::Ok(lower_expr(e, arena, types)),
                Err(e) => ResultExprKind::Err(lower_expr(e, arena, types)),
            };
            (
                ExprKind::Result { expr: kind },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Call {
            call_type,
            args,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let call_node = lower_call_type(call_type, arena, types);
            let arg_ids = args.iter().map(|a| lower_expr(a, arena, types)).collect();
            (
                ExprKind::Call {
                    call_type: call_node,
                    args: arg_ids,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::InvokeMethodLazy {
            lhs,
            method,
            args,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let lhs_id = lower_expr(lhs, arena, types);
            let arg_ids = args.iter().map(|a| lower_expr(a, arena, types)).collect();
            (
                ExprKind::InvokeMethodLazy {
                    lhs: lhs_id,
                    method: method.clone(),
                    args: arg_ids,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Unwrap {
            expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let child = lower_expr(expr, arena, types);
            (
                ExprKind::Unwrap { expr: child },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Throw {
            message,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::Throw {
                message: message.clone(),
            },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),

        Expr::GetTag {
            expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let child = lower_expr(expr, arena, types);
            (
                ExprKind::GetTag { expr: child },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::ListComprehension {
            iterated_variable,
            iterable_expr,
            yield_expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let iterable = lower_expr(iterable_expr, arena, types);
            let yield_ = lower_expr(yield_expr, arena, types);
            (
                ExprKind::ListComprehension {
                    iterated_variable: iterated_variable.clone(),
                    iterable_expr: iterable,
                    yield_expr: yield_,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::ListReduce {
            reduce_variable,
            iterated_variable,
            iterable_expr,
            init_value_expr,
            yield_expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let iterable = lower_expr(iterable_expr, arena, types);
            let init = lower_expr(init_value_expr, arena, types);
            let yield_ = lower_expr(yield_expr, arena, types);
            (
                ExprKind::ListReduce {
                    reduce_variable: reduce_variable.clone(),
                    iterated_variable: iterated_variable.clone(),
                    iterable_expr: iterable,
                    init_value_expr: init,
                    yield_expr: yield_,
                },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::Length {
            expr,
            type_annotation,
            inferred_type,
            source_span,
        } => {
            let child = lower_expr(expr, arena, types);
            (
                ExprKind::Length { expr: child },
                source_span.clone(),
                type_annotation.clone(),
                inferred_type.clone(),
            )
        }

        Expr::GenerateWorkerName {
            variable_id,
            type_annotation,
            inferred_type,
            source_span,
        } => (
            ExprKind::GenerateWorkerName {
                variable_id: variable_id.clone(),
            },
            source_span.clone(),
            type_annotation.clone(),
            inferred_type.clone(),
        ),
    };

    let id = arena.alloc_expr(ExprNode {
        kind,
        source_span: span,
        type_annotation: annotation,
    });
    types.set(id, inferred);
    id
}

fn lower_range(range: &Range, arena: &mut ExprArena, types: &mut TypeTable) -> RangeKind {
    match range {
        Range::Range { from, to } => RangeKind::Range {
            from: lower_expr(from, arena, types),
            to: lower_expr(to, arena, types),
        },
        Range::RangeInclusive { from, to } => RangeKind::RangeInclusive {
            from: lower_expr(from, arena, types),
            to: lower_expr(to, arena, types),
        },
        Range::RangeFrom { from } => RangeKind::RangeFrom {
            from: lower_expr(from, arena, types),
        },
    }
}

fn lower_match_arm(arm: &MatchArm, arena: &mut ExprArena, types: &mut TypeTable) -> MatchArmNode {
    let pattern_id = lower_arm_pattern(&arm.arm_pattern, arena, types);
    let resolution_id = lower_expr(&arm.arm_resolution_expr, arena, types);
    MatchArmNode {
        arm_pattern: pattern_id,
        arm_resolution_expr: resolution_id,
    }
}

fn lower_arm_pattern(
    pattern: &ArmPattern,
    arena: &mut ExprArena,
    types: &mut TypeTable,
) -> ArmPatternId {
    let node = match pattern {
        ArmPattern::WildCard => ArmPatternNode::WildCard,

        ArmPattern::As(name, inner) => {
            let inner_id = lower_arm_pattern(inner, arena, types);
            ArmPatternNode::As(name.clone(), inner_id)
        }

        ArmPattern::Constructor(name, patterns) => {
            let ids = patterns
                .iter()
                .map(|p| lower_arm_pattern(p, arena, types))
                .collect();
            ArmPatternNode::Constructor(name.clone(), ids)
        }

        ArmPattern::TupleConstructor(patterns) => {
            let ids = patterns
                .iter()
                .map(|p| lower_arm_pattern(p, arena, types))
                .collect();
            ArmPatternNode::TupleConstructor(ids)
        }

        ArmPattern::RecordConstructor(fields) => {
            let pairs = fields
                .iter()
                .map(|(name, p)| (name.clone(), lower_arm_pattern(p, arena, types)))
                .collect();
            ArmPatternNode::RecordConstructor(pairs)
        }

        ArmPattern::ListConstructor(patterns) => {
            let ids = patterns
                .iter()
                .map(|p| lower_arm_pattern(p, arena, types))
                .collect();
            ArmPatternNode::ListConstructor(ids)
        }

        ArmPattern::Literal(expr) => {
            let expr_id = lower_expr(expr, arena, types);
            ArmPatternNode::Literal(expr_id)
        }
    };

    arena.alloc_pattern(node)
}

fn lower_call_type(
    call_type: &CallType,
    arena: &mut ExprArena,
    types: &mut TypeTable,
) -> CallTypeNode {
    match call_type {
        CallType::Function {
            component_info,
            instance_identifier,
            function_name,
        } => CallTypeNode::Function {
            component_info: component_info.clone(),
            instance_identifier: instance_identifier
                .as_ref()
                .map(|ii| lower_instance_identifier(ii, arena, types)),
            function_name: function_name.clone(),
        },

        CallType::VariantConstructor(name) => CallTypeNode::VariantConstructor(name.clone()),
        CallType::EnumConstructor(name) => CallTypeNode::EnumConstructor(name.clone()),

        CallType::InstanceCreation(creation) => {
            CallTypeNode::InstanceCreation(lower_instance_creation(creation, arena, types))
        }
    }
}

fn lower_instance_identifier(
    ii: &InstanceIdentifier,
    arena: &mut ExprArena,
    types: &mut TypeTable,
) -> InstanceIdentifierNode {
    match ii {
        InstanceIdentifier::WitWorker {
            variable_id,
            worker_name,
        } => InstanceIdentifierNode::WitWorker {
            variable_id: variable_id.clone(),
            worker_name: worker_name
                .as_ref()
                .map(|wn| lower_expr(wn.as_ref(), arena, types)),
        },
        InstanceIdentifier::WitResource {
            variable_id,
            worker_name,
            resource_name,
        } => InstanceIdentifierNode::WitResource {
            variable_id: variable_id.clone(),
            worker_name: worker_name
                .as_ref()
                .map(|wn| lower_expr(wn.as_ref(), arena, types)),
            resource_name: resource_name.clone(),
        },
    }
}

fn lower_instance_creation(
    creation: &InstanceCreationType,
    arena: &mut ExprArena,
    types: &mut TypeTable,
) -> InstanceCreationNode {
    match creation {
        InstanceCreationType::WitWorker {
            component_info,
            worker_name,
        } => InstanceCreationNode::WitWorker {
            component_info: component_info.clone(),
            worker_name: worker_name
                .as_ref()
                .map(|wn| lower_expr(wn.as_ref(), arena, types)),
        },
        InstanceCreationType::WitResource {
            component_info,
            module,
            resource_name,
        } => InstanceCreationNode::WitResource {
            component_info: component_info.clone(),
            module: module
                .as_ref()
                .map(|m| lower_instance_identifier(m, arena, types)),
            resource_name: resource_name.clone(),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use test_r::test;

    use super::*;
    use crate::Expr;

    #[test]
    fn test_lower_literal() {
        let expr = Expr::from_text(r#""hello""#).unwrap();
        let (arena, types, root) = lower(&expr);
        let node = arena.expr(root);
        assert!(matches!(node.kind, ExprKind::Literal { .. }));
        // TypeTable must contain an entry for the root
        assert!(types.get_opt(root).is_some());
    }

    #[test]
    fn test_lower_let_binding() {
        let expr = Expr::from_text("let x = 1; x").unwrap();
        let (arena, types, root) = lower(&expr);
        // root should be an ExprBlock
        let node = arena.expr(root);
        assert!(matches!(node.kind, ExprKind::ExprBlock { .. }));
        // All nodes must have a type entry
        for (id, _) in arena.exprs.iter() {
            assert!(types.get_opt(id).is_some(), "missing type for expr node");
        }
    }

    #[test]
    fn test_lower_pattern_match() {
        let src = r#"
            let x = some("hello");
            match x {
              some(v) => v,
              none => "default"
            }
        "#;
        let expr = Expr::from_text(src).unwrap();
        let (arena, types, root) = lower(&expr);
        // Verify every node and every pattern has a type / exists
        for (id, _) in arena.exprs.iter() {
            assert!(types.get_opt(id).is_some());
        }
        // Patterns should all be allocated
        assert!(arena.patterns.iter().count() > 0);
        let _ = root;
    }

    #[test]
    fn test_type_table_snapshot_and_same_as() {
        let expr = Expr::from_text(r#"1 + 2"#).unwrap();
        let (_arena, types, _root) = lower(&expr);
        let snap = types.snapshot();
        // Reconstruct a TypeTable from the snapshot and verify same_as
        let mut reconstructed = TypeTable::new();
        for (id, ty) in snap {
            reconstructed.set(id, ty);
        }
        assert!(types.same_as(&reconstructed));
    }

    #[test]
    fn test_node_count_matches() {
        // A simple expr "1" has exactly 1 node
        let expr = Expr::from_text(r#"1"#).unwrap();
        let (arena, types, _root) = lower(&expr);
        assert_eq!(arena.exprs.iter().count(), types.snapshot().len());
    }
}
