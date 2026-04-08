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

//! Arena-based IR for the Rib expression tree.
//!
//! # Design
//!
//! The key insight is that the current `Expr` design embeds `InferredType` directly
//! inside every node, which forces a full tree clone on every fix-point iteration
//! during type inference (just to compare whether types changed).
//!
//! This module introduces three separate concerns:
//!
//! - [`ExprArena`]: owns the *structural* nodes — shape, spans, variable names.
//!   Nodes reference their children by [`ExprId`], not by ownership.
//!   This means no allocation is needed to "snapshot" the tree structure.
//!
//! - [`TypeTable`]: a parallel array `ExprId -> InferredType`.  
//!   Snapshotting types is now a cheap `Vec::clone` of `InferredType` values
//!   instead of a deep tree clone.
//!
//! - [`ArmPatternArena`]: patterns in match arms also contain embedded `Expr`
//!   literals (identifiers, option/result nodes). These are allocated in the
//!   same `ExprArena`; the pattern tree itself is stored separately.
//!
//! # Two-phase lowering
//!
//! The existing parser still produces the old `Expr` tree. The function
//! [`lower`] converts that tree into an `(ExprArena, TypeTable, ExprId)` triple
//! without changing any parser or public API code.
//!
//! # Future phases
//!
//! Once the arena is in place, each type inference pass can be migrated one at
//! a time to operate on `(ExprId, &ExprArena, &mut TypeTable)` instead of
//! `&mut Expr`, eliminating the fix-point clone entirely.

use la_arena::{Arena, Idx};
use std::collections::HashMap;

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

/// Arena-friendly match arm: the resolution expression is an `ExprId`.
#[derive(Debug, Clone)]
pub struct MatchArmNode {
    pub arm_pattern: ArmPatternId,
    pub arm_resolution_expr: ExprId,
}

// ---------------------------------------------------------------------------
// Arm pattern node
// ---------------------------------------------------------------------------

/// An arm pattern with embedded `Expr` literals replaced by `ExprId`.
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

// ---------------------------------------------------------------------------
// TypeTable — the parallel type array
// ---------------------------------------------------------------------------

/// Stores the inferred type for every expression node, keyed by [`ExprId`].
///
/// This is intentionally separate from [`ExprArena`] so that:
/// - Snapshotting for the fix-point loop is a cheap `TypeTable::snapshot()` /
///   `TypeTable::diff()` rather than a full tree clone.
/// - Type mutations during inference passes do not touch structural nodes.
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
    ///
    /// Prefer this over `same_as` in the fix-point loop — it avoids cloning a
    /// second `TypeTable` and instead compares against the already-allocated
    /// `Vec` returned by `snapshot()`.
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

// ---------------------------------------------------------------------------
// Apply-back: TypeTable  →  old Expr tree
// ---------------------------------------------------------------------------

/// Walk the original `Expr` tree in the same traversal order as [`lower`] and
/// write the final `InferredType` from `TypeTable` back into each node.
///
/// This is the bridge that lets the arena-based type inference passes write
/// their results back into the old `Expr` representation so that the compiler
/// and interpreter — which still operate on `Expr` — see the updated types.
///
/// The traversal order must match [`lower_expr`] exactly so that node IDs
/// correspond to the same positions in the tree.
pub fn apply_types_back(expr: &mut Expr, arena: &ExprArena, types: &TypeTable, root: ExprId) {
    apply_types_back_expr(expr, arena, types, root);
}

fn apply_types_back_expr(expr: &mut Expr, arena: &ExprArena, types: &TypeTable, id: ExprId) {
    // Write the type for this node
    if let Some(ty) = types.get_opt(id) {
        expr.with_inferred_type_mut(ty.clone());
    }

    // Recurse into children in the same order as lower_expr
    match expr {
        Expr::Let { expr: rhs, .. } => {
            if let ExprKind::Let { expr: rhs_id, .. } = arena.expr(id).kind {
                apply_types_back_expr(rhs, arena, types, rhs_id);
            }
        }
        Expr::SelectField { expr: inner, .. } => {
            if let ExprKind::SelectField { expr: inner_id, .. } = arena.expr(id).kind {
                apply_types_back_expr(inner, arena, types, inner_id);
            }
        }
        Expr::SelectIndex {
            expr: e, index: i, ..
        } => {
            if let ExprKind::SelectIndex {
                expr: e_id,
                index: i_id,
            } = arena.expr(id).kind
            {
                apply_types_back_expr(e, arena, types, e_id);
                apply_types_back_expr(i, arena, types, i_id);
            }
        }
        Expr::Sequence { exprs, .. }
        | Expr::Tuple { exprs, .. }
        | Expr::Concat { exprs, .. }
        | Expr::ExprBlock { exprs, .. } => {
            let child_ids: Vec<ExprId> = match &arena.expr(id).kind {
                ExprKind::Sequence { exprs }
                | ExprKind::Tuple { exprs }
                | ExprKind::Concat { exprs }
                | ExprKind::ExprBlock { exprs } => exprs.clone(),
                _ => return,
            };
            for (e, cid) in exprs.iter_mut().zip(child_ids) {
                apply_types_back_expr(e, arena, types, cid);
            }
        }
        Expr::Record { exprs, .. } => {
            let child_ids: Vec<ExprId> = match &arena.expr(id).kind {
                ExprKind::Record { fields } => fields.iter().map(|(_, id)| *id).collect(),
                _ => return,
            };
            for ((_, e), cid) in exprs.iter_mut().zip(child_ids) {
                apply_types_back_expr(e, arena, types, cid);
            }
        }
        Expr::Range { range, .. } => match (range, &arena.expr(id).kind) {
            (
                crate::expr::Range::Range { from, to },
                ExprKind::Range {
                    range: RangeKind::Range { from: fid, to: tid },
                },
            ) => {
                apply_types_back_expr(from, arena, types, *fid);
                apply_types_back_expr(to, arena, types, *tid);
            }
            (
                crate::expr::Range::RangeInclusive { from, to },
                ExprKind::Range {
                    range: RangeKind::RangeInclusive { from: fid, to: tid },
                },
            ) => {
                apply_types_back_expr(from, arena, types, *fid);
                apply_types_back_expr(to, arena, types, *tid);
            }
            (
                crate::expr::Range::RangeFrom { from },
                ExprKind::Range {
                    range: RangeKind::RangeFrom { from: fid },
                },
            ) => {
                apply_types_back_expr(from, arena, types, *fid);
            }
            _ => {}
        },
        Expr::Not { expr: inner, .. }
        | Expr::Length { expr: inner, .. }
        | Expr::Unwrap { expr: inner, .. }
        | Expr::GetTag { expr: inner, .. } => {
            let child_id = match arena.expr(id).kind {
                ExprKind::Not { expr }
                | ExprKind::Length { expr }
                | ExprKind::Unwrap { expr }
                | ExprKind::GetTag { expr } => expr,
                _ => return,
            };
            apply_types_back_expr(inner, arena, types, child_id);
        }
        Expr::GreaterThan { lhs, rhs, .. }
        | Expr::GreaterThanOrEqualTo { lhs, rhs, .. }
        | Expr::LessThanOrEqualTo { lhs, rhs, .. }
        | Expr::EqualTo { lhs, rhs, .. }
        | Expr::LessThan { lhs, rhs, .. }
        | Expr::And { lhs, rhs, .. }
        | Expr::Or { lhs, rhs, .. }
        | Expr::Plus { lhs, rhs, .. }
        | Expr::Minus { lhs, rhs, .. }
        | Expr::Multiply { lhs, rhs, .. }
        | Expr::Divide { lhs, rhs, .. } => {
            let (lid, rid) = match arena.expr(id).kind {
                ExprKind::GreaterThan { lhs, rhs }
                | ExprKind::GreaterThanOrEqualTo { lhs, rhs }
                | ExprKind::LessThanOrEqualTo { lhs, rhs }
                | ExprKind::EqualTo { lhs, rhs }
                | ExprKind::LessThan { lhs, rhs }
                | ExprKind::And { lhs, rhs }
                | ExprKind::Or { lhs, rhs }
                | ExprKind::Plus { lhs, rhs }
                | ExprKind::Minus { lhs, rhs }
                | ExprKind::Multiply { lhs, rhs }
                | ExprKind::Divide { lhs, rhs } => (lhs, rhs),
                _ => return,
            };
            apply_types_back_expr(lhs, arena, types, lid);
            apply_types_back_expr(rhs, arena, types, rid);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            if let ExprKind::Cond {
                cond: cid,
                lhs: lid,
                rhs: rid,
            } = arena.expr(id).kind
            {
                apply_types_back_expr(cond, arena, types, cid);
                apply_types_back_expr(lhs, arena, types, lid);
                apply_types_back_expr(rhs, arena, types, rid);
            }
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            if let ExprKind::PatternMatch {
                predicate: pid,
                match_arms: arena_arms,
            } = &arena.expr(id).kind.clone()
            {
                apply_types_back_expr(predicate, arena, types, *pid);
                for (arm, arena_arm) in match_arms.iter_mut().zip(arena_arms.iter()) {
                    apply_types_back_expr(
                        &mut arm.arm_resolution_expr,
                        arena,
                        types,
                        arena_arm.arm_resolution_expr,
                    );
                    apply_types_back_arm_pattern(
                        &mut arm.arm_pattern,
                        arena,
                        types,
                        arena_arm.arm_pattern,
                    );
                }
            }
        }
        Expr::Option {
            expr: Some(inner), ..
        } => {
            if let ExprKind::Option {
                expr: Some(inner_id),
            } = arena.expr(id).kind
            {
                apply_types_back_expr(inner, arena, types, inner_id);
            }
        }
        Expr::Result {
            expr: Ok(inner), ..
        } => {
            if let ExprKind::Result {
                expr: ResultExprKind::Ok(inner_id),
            } = arena.expr(id).kind
            {
                apply_types_back_expr(inner, arena, types, inner_id);
            }
        }
        Expr::Result {
            expr: Err(inner), ..
        } => {
            if let ExprKind::Result {
                expr: ResultExprKind::Err(inner_id),
            } = arena.expr(id).kind
            {
                apply_types_back_expr(inner, arena, types, inner_id);
            }
        }
        Expr::Call { args, .. } => {
            if let ExprKind::Call { args: arg_ids, .. } = &arena.expr(id).kind.clone() {
                let arg_ids = arg_ids.clone();
                for (arg, &arg_id) in args.iter_mut().zip(arg_ids.iter()) {
                    apply_types_back_expr(arg, arena, types, arg_id);
                }
            }
        }
        Expr::InvokeMethodLazy { lhs, args, .. } => {
            if let ExprKind::InvokeMethodLazy {
                lhs: lhs_id,
                args: arg_ids,
                ..
            } = &arena.expr(id).kind.clone()
            {
                let lhs_id = *lhs_id;
                let arg_ids = arg_ids.clone();
                apply_types_back_expr(lhs, arena, types, lhs_id);
                for (arg, &arg_id) in args.iter_mut().zip(arg_ids.iter()) {
                    apply_types_back_expr(arg, arena, types, arg_id);
                }
            }
        }
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            if let ExprKind::ListComprehension {
                iterable_expr: ie_id,
                yield_expr: ye_id,
                ..
            } = arena.expr(id).kind
            {
                apply_types_back_expr(iterable_expr, arena, types, ie_id);
                apply_types_back_expr(yield_expr, arena, types, ye_id);
            }
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            if let ExprKind::ListReduce {
                iterable_expr: ie_id,
                init_value_expr: iv_id,
                yield_expr: ye_id,
                ..
            } = arena.expr(id).kind
            {
                apply_types_back_expr(iterable_expr, arena, types, ie_id);
                apply_types_back_expr(init_value_expr, arena, types, iv_id);
                apply_types_back_expr(yield_expr, arena, types, ye_id);
            }
        }
        // Leaf nodes — type already written above, no children
        Expr::Literal { .. }
        | Expr::Number { .. }
        | Expr::Flags { .. }
        | Expr::Identifier { .. }
        | Expr::Boolean { .. }
        | Expr::Option { expr: None, .. }
        | Expr::Throw { .. }
        | Expr::GenerateWorkerName { .. } => {}
    }
}

fn apply_types_back_arm_pattern(
    pattern: &mut crate::expr::ArmPattern,
    arena: &ExprArena,
    types: &TypeTable,
    pat_id: ArmPatternId,
) {
    match (pattern, arena.pattern(pat_id)) {
        (crate::expr::ArmPattern::Literal(expr), ArmPatternNode::Literal(expr_id)) => {
            let expr_id = *expr_id;
            apply_types_back_expr(expr, arena, types, expr_id);
        }
        (crate::expr::ArmPattern::As(_, inner), ArmPatternNode::As(_, inner_id)) => {
            let inner_id = *inner_id;
            apply_types_back_arm_pattern(inner, arena, types, inner_id);
        }
        (
            crate::expr::ArmPattern::Constructor(_, pats),
            ArmPatternNode::Constructor(_, child_ids),
        )
        | (
            crate::expr::ArmPattern::TupleConstructor(pats),
            ArmPatternNode::TupleConstructor(child_ids),
        )
        | (
            crate::expr::ArmPattern::ListConstructor(pats),
            ArmPatternNode::ListConstructor(child_ids),
        ) => {
            let child_ids = child_ids.clone();
            for (p, cid) in pats.iter_mut().zip(child_ids) {
                apply_types_back_arm_pattern(p, arena, types, cid);
            }
        }
        (
            crate::expr::ArmPattern::RecordConstructor(fields),
            ArmPatternNode::RecordConstructor(arena_fields),
        ) => {
            let arena_fields = arena_fields.clone();
            for ((_, p), (_, cid)) in fields.iter_mut().zip(arena_fields) {
                apply_types_back_arm_pattern(p, arena, types, cid);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Lowering: old Expr tree  →  ExprArena + TypeTable
// ---------------------------------------------------------------------------

/// Lower the old recursive `Expr` tree into an arena representation.
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

/// Lower the `Expr` tree and return a map from each `ExprId` to a raw mutable
/// pointer to the corresponding `InferredType` inside the original `Expr` tree.
///
/// This map is used by [`write_types_back`] to apply the final types from the
/// `TypeTable` back into the original tree **after** arena-based inference,
/// without requiring the `Expr` and arena to have the same shape (which they
/// won't after structural mutations like `Identifier → Call`).
///
/// # Safety
/// The caller must ensure that the original `Expr` tree lives at least as long
/// as the map and that `write_types_back` is called before any drop of `expr`.
pub fn lower_with_type_map(
    expr: &mut Expr,
) -> (
    ExprArena,
    TypeTable,
    ExprId,
    HashMap<ExprId, *mut InferredType>,
) {
    let mut arena = ExprArena::new();
    let mut types = TypeTable::new();
    let mut type_map: HashMap<ExprId, *mut InferredType> = HashMap::new();
    let root = lower_expr_with_map(expr, &mut arena, &mut types, &mut type_map);
    (arena, types, root, type_map)
}

/// Write the final `InferredType` values from `types` back into the original
/// `Expr` tree using the pointer map produced by [`lower_with_type_map`].
///
/// Only nodes whose `ExprId` is in the map are updated (i.e. only nodes that
/// existed in the original tree before any structural mutations).
///
/// # Safety
/// See [`lower_with_type_map`].
pub unsafe fn write_types_back(type_map: &HashMap<ExprId, *mut InferredType>, types: &TypeTable) {
    for (id, ptr) in type_map {
        if let Some(ty) = types.get_opt(*id) {
            unsafe {
                **ptr = ty.clone();
            }
        }
    }
}

fn lower_expr_with_map(
    expr: &mut Expr,
    arena: &mut ExprArena,
    types: &mut TypeTable,
    type_map: &mut HashMap<ExprId, *mut InferredType>,
) -> ExprId {
    // First lower structurally (using the immutable version for all children),
    // then record the pointer for this node.
    let id = lower_expr(expr, arena, types);
    // Record a pointer to this node's inferred_type field
    let ptr: *mut InferredType = expr.inferred_type_mut();
    type_map.insert(id, ptr);
    // Recurse into children to record their pointers too.
    // We use the same traversal order as lower_expr.
    record_children_pointers(expr, id, arena, types, type_map);
    id
}

fn record_children_pointers(
    expr: &mut Expr,
    id: ExprId,
    arena: &ExprArena,
    types: &TypeTable,
    type_map: &mut HashMap<ExprId, *mut InferredType>,
) {
    match expr {
        Expr::Let { expr: rhs, .. } => {
            if let ExprKind::Let { expr: rhs_id, .. } = arena.expr(id).kind {
                let ptr: *mut InferredType = rhs.inferred_type_mut();
                type_map.insert(rhs_id, ptr);
                record_children_pointers(rhs, rhs_id, arena, types, type_map);
            }
        }
        Expr::SelectField { expr: inner, .. } => {
            if let ExprKind::SelectField { expr: inner_id, .. } = arena.expr(id).kind {
                type_map.insert(inner_id, inner.inferred_type_mut());
                record_children_pointers(inner, inner_id, arena, types, type_map);
            }
        }
        Expr::SelectIndex {
            expr: e, index: i, ..
        } => {
            if let ExprKind::SelectIndex {
                expr: e_id,
                index: i_id,
            } = arena.expr(id).kind
            {
                type_map.insert(e_id, e.inferred_type_mut());
                type_map.insert(i_id, i.inferred_type_mut());
                record_children_pointers(e, e_id, arena, types, type_map);
                record_children_pointers(i, i_id, arena, types, type_map);
            }
        }
        Expr::Sequence { exprs, .. }
        | Expr::Tuple { exprs, .. }
        | Expr::Concat { exprs, .. }
        | Expr::ExprBlock { exprs, .. } => {
            let child_ids: Vec<ExprId> = match &arena.expr(id).kind {
                ExprKind::Sequence { exprs }
                | ExprKind::Tuple { exprs }
                | ExprKind::Concat { exprs }
                | ExprKind::ExprBlock { exprs } => exprs.clone(),
                _ => return,
            };
            for (e, cid) in exprs.iter_mut().zip(child_ids) {
                type_map.insert(cid, e.inferred_type_mut());
                record_children_pointers(e, cid, arena, types, type_map);
            }
        }
        Expr::Record { exprs, .. } => {
            let child_ids: Vec<ExprId> = match &arena.expr(id).kind {
                ExprKind::Record { fields } => fields.iter().map(|(_, id)| *id).collect(),
                _ => return,
            };
            for ((_, e), cid) in exprs.iter_mut().zip(child_ids) {
                type_map.insert(cid, e.inferred_type_mut());
                record_children_pointers(e, cid, arena, types, type_map);
            }
        }
        Expr::Range { range, .. } => match (range, &arena.expr(id).kind) {
            (
                crate::expr::Range::Range { from, to },
                ExprKind::Range {
                    range: RangeKind::Range { from: fid, to: tid },
                },
            ) => {
                let (fid, tid) = (*fid, *tid);
                type_map.insert(fid, from.inferred_type_mut());
                type_map.insert(tid, to.inferred_type_mut());
                record_children_pointers(from, fid, arena, types, type_map);
                record_children_pointers(to, tid, arena, types, type_map);
            }
            (
                crate::expr::Range::RangeInclusive { from, to },
                ExprKind::Range {
                    range: RangeKind::RangeInclusive { from: fid, to: tid },
                },
            ) => {
                let (fid, tid) = (*fid, *tid);
                type_map.insert(fid, from.inferred_type_mut());
                type_map.insert(tid, to.inferred_type_mut());
                record_children_pointers(from, fid, arena, types, type_map);
                record_children_pointers(to, tid, arena, types, type_map);
            }
            (
                crate::expr::Range::RangeFrom { from },
                ExprKind::Range {
                    range: RangeKind::RangeFrom { from: fid },
                },
            ) => {
                let fid = *fid;
                type_map.insert(fid, from.inferred_type_mut());
                record_children_pointers(from, fid, arena, types, type_map);
            }
            _ => {}
        },
        Expr::Not { expr: inner, .. }
        | Expr::Length { expr: inner, .. }
        | Expr::Unwrap { expr: inner, .. }
        | Expr::GetTag { expr: inner, .. } => {
            let child_id = match arena.expr(id).kind {
                ExprKind::Not { expr }
                | ExprKind::Length { expr }
                | ExprKind::Unwrap { expr }
                | ExprKind::GetTag { expr } => expr,
                _ => return,
            };
            type_map.insert(child_id, inner.inferred_type_mut());
            record_children_pointers(inner, child_id, arena, types, type_map);
        }
        Expr::GreaterThan { lhs, rhs, .. }
        | Expr::GreaterThanOrEqualTo { lhs, rhs, .. }
        | Expr::LessThanOrEqualTo { lhs, rhs, .. }
        | Expr::EqualTo { lhs, rhs, .. }
        | Expr::LessThan { lhs, rhs, .. }
        | Expr::And { lhs, rhs, .. }
        | Expr::Or { lhs, rhs, .. }
        | Expr::Plus { lhs, rhs, .. }
        | Expr::Minus { lhs, rhs, .. }
        | Expr::Multiply { lhs, rhs, .. }
        | Expr::Divide { lhs, rhs, .. } => {
            let (lid, rid) = match arena.expr(id).kind {
                ExprKind::GreaterThan { lhs, rhs }
                | ExprKind::GreaterThanOrEqualTo { lhs, rhs }
                | ExprKind::LessThanOrEqualTo { lhs, rhs }
                | ExprKind::EqualTo { lhs, rhs }
                | ExprKind::LessThan { lhs, rhs }
                | ExprKind::And { lhs, rhs }
                | ExprKind::Or { lhs, rhs }
                | ExprKind::Plus { lhs, rhs }
                | ExprKind::Minus { lhs, rhs }
                | ExprKind::Multiply { lhs, rhs }
                | ExprKind::Divide { lhs, rhs } => (lhs, rhs),
                _ => return,
            };
            type_map.insert(lid, lhs.inferred_type_mut());
            type_map.insert(rid, rhs.inferred_type_mut());
            record_children_pointers(lhs, lid, arena, types, type_map);
            record_children_pointers(rhs, rid, arena, types, type_map);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            if let ExprKind::Cond {
                cond: cid,
                lhs: lid,
                rhs: rid,
            } = arena.expr(id).kind
            {
                type_map.insert(cid, cond.inferred_type_mut());
                type_map.insert(lid, lhs.inferred_type_mut());
                type_map.insert(rid, rhs.inferred_type_mut());
                record_children_pointers(cond, cid, arena, types, type_map);
                record_children_pointers(lhs, lid, arena, types, type_map);
                record_children_pointers(rhs, rid, arena, types, type_map);
            }
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            if let ExprKind::PatternMatch {
                predicate: pid,
                match_arms: arena_arms,
            } = &arena.expr(id).kind.clone()
            {
                let (pid, arena_arms) = (*pid, arena_arms.clone());
                type_map.insert(pid, predicate.inferred_type_mut());
                record_children_pointers(predicate, pid, arena, types, type_map);
                for (arm, arena_arm) in match_arms.iter_mut().zip(arena_arms.iter()) {
                    type_map.insert(
                        arena_arm.arm_resolution_expr,
                        arm.arm_resolution_expr.inferred_type_mut(),
                    );
                    record_children_pointers(
                        &mut arm.arm_resolution_expr,
                        arena_arm.arm_resolution_expr,
                        arena,
                        types,
                        type_map,
                    );
                    record_arm_pattern_pointers(
                        &mut arm.arm_pattern,
                        arena,
                        types,
                        type_map,
                        arena_arm.arm_pattern,
                    );
                }
            }
        }
        Expr::Option {
            expr: Some(inner), ..
        } => {
            if let ExprKind::Option {
                expr: Some(inner_id),
            } = arena.expr(id).kind
            {
                type_map.insert(inner_id, inner.inferred_type_mut());
                record_children_pointers(inner, inner_id, arena, types, type_map);
            }
        }
        Expr::Result {
            expr: Ok(inner), ..
        } => {
            if let ExprKind::Result {
                expr: ResultExprKind::Ok(inner_id),
            } = arena.expr(id).kind
            {
                type_map.insert(inner_id, inner.inferred_type_mut());
                record_children_pointers(inner, inner_id, arena, types, type_map);
            }
        }
        Expr::Result {
            expr: Err(inner), ..
        } => {
            if let ExprKind::Result {
                expr: ResultExprKind::Err(inner_id),
            } = arena.expr(id).kind
            {
                type_map.insert(inner_id, inner.inferred_type_mut());
                record_children_pointers(inner, inner_id, arena, types, type_map);
            }
        }
        Expr::Call { args, .. } => {
            if let ExprKind::Call { args: arg_ids, .. } = &arena.expr(id).kind.clone() {
                let arg_ids = arg_ids.clone();
                for (arg, &arg_id) in args.iter_mut().zip(arg_ids.iter()) {
                    type_map.insert(arg_id, arg.inferred_type_mut());
                    record_children_pointers(arg, arg_id, arena, types, type_map);
                }
            }
        }
        Expr::InvokeMethodLazy { lhs, args, .. } => {
            if let ExprKind::InvokeMethodLazy {
                lhs: lhs_id,
                args: arg_ids,
                ..
            } = &arena.expr(id).kind.clone()
            {
                let (lhs_id, arg_ids) = (*lhs_id, arg_ids.clone());
                type_map.insert(lhs_id, lhs.inferred_type_mut());
                record_children_pointers(lhs, lhs_id, arena, types, type_map);
                for (arg, &arg_id) in args.iter_mut().zip(arg_ids.iter()) {
                    type_map.insert(arg_id, arg.inferred_type_mut());
                    record_children_pointers(arg, arg_id, arena, types, type_map);
                }
            }
        }
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            if let ExprKind::ListComprehension {
                iterable_expr: ie_id,
                yield_expr: ye_id,
                ..
            } = arena.expr(id).kind
            {
                type_map.insert(ie_id, iterable_expr.inferred_type_mut());
                type_map.insert(ye_id, yield_expr.inferred_type_mut());
                record_children_pointers(iterable_expr, ie_id, arena, types, type_map);
                record_children_pointers(yield_expr, ye_id, arena, types, type_map);
            }
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            if let ExprKind::ListReduce {
                iterable_expr: ie_id,
                init_value_expr: iv_id,
                yield_expr: ye_id,
                ..
            } = arena.expr(id).kind
            {
                type_map.insert(ie_id, iterable_expr.inferred_type_mut());
                type_map.insert(iv_id, init_value_expr.inferred_type_mut());
                type_map.insert(ye_id, yield_expr.inferred_type_mut());
                record_children_pointers(iterable_expr, ie_id, arena, types, type_map);
                record_children_pointers(init_value_expr, iv_id, arena, types, type_map);
                record_children_pointers(yield_expr, ye_id, arena, types, type_map);
            }
        }
        Expr::Literal { .. }
        | Expr::Number { .. }
        | Expr::Flags { .. }
        | Expr::Identifier { .. }
        | Expr::Boolean { .. }
        | Expr::Option { expr: None, .. }
        | Expr::Throw { .. }
        | Expr::GenerateWorkerName { .. } => {}
    }
}

fn record_arm_pattern_pointers(
    pattern: &mut crate::expr::ArmPattern,
    arena: &ExprArena,
    types: &TypeTable,
    type_map: &mut HashMap<ExprId, *mut InferredType>,
    pat_id: ArmPatternId,
) {
    match (pattern, arena.pattern(pat_id)) {
        (crate::expr::ArmPattern::Literal(expr), ArmPatternNode::Literal(expr_id)) => {
            let expr_id = *expr_id;
            type_map.insert(expr_id, expr.inferred_type_mut());
            record_children_pointers(expr, expr_id, arena, types, type_map);
        }
        (crate::expr::ArmPattern::As(_, inner), ArmPatternNode::As(_, inner_id)) => {
            let inner_id = *inner_id;
            record_arm_pattern_pointers(inner, arena, types, type_map, inner_id);
        }
        (
            crate::expr::ArmPattern::Constructor(_, pats),
            ArmPatternNode::Constructor(_, child_ids),
        )
        | (
            crate::expr::ArmPattern::TupleConstructor(pats),
            ArmPatternNode::TupleConstructor(child_ids),
        )
        | (
            crate::expr::ArmPattern::ListConstructor(pats),
            ArmPatternNode::ListConstructor(child_ids),
        ) => {
            let child_ids = child_ids.clone();
            for (p, cid) in pats.iter_mut().zip(child_ids) {
                record_arm_pattern_pointers(p, arena, types, type_map, cid);
            }
        }
        (
            crate::expr::ArmPattern::RecordConstructor(fields),
            ArmPatternNode::RecordConstructor(arena_fields),
        ) => {
            let arena_fields = arena_fields.clone();
            for ((_, p), (_, cid)) in fields.iter_mut().zip(arena_fields) {
                record_arm_pattern_pointers(p, arena, types, type_map, cid);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Rebuild: ExprArena + TypeTable  →  old Expr tree
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
