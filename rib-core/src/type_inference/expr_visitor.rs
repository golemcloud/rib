use crate::{Expr, TypeInternal};
use std::collections::VecDeque;

// Post-order (bottom-up): children first, then parent. Left-to-right.
pub fn visit_post_order_mut(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    visit_children_mut(expr, |child| visit_post_order_mut(child, f));
    f(expr);
}

// Pre-order reversed: parent first, then children right-to-left.
// This matches the old ExprVisitor::bottom_up + pop_back() semantics.
pub fn visit_post_order_rev_mut(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    f(expr);
    visit_children_rev_mut(expr, |child| visit_post_order_rev_mut(child, f));
}

// Pre-order (top-down): parent first, then children left-to-right.
pub fn visit_pre_order_mut(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    f(expr);
    visit_children_mut(expr, |child| visit_pre_order_mut(child, f));
}

// Fallible post-order: children first, then parent. Stops on first error.
pub fn try_visit_post_order_mut<E>(
    expr: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    try_visit_children_mut(expr, |child| try_visit_post_order_mut(child, f))?;
    f(expr)
}

// Fallible pre-order reversed: parent first, then children right-to-left.
pub fn try_visit_post_order_rev_mut<E>(
    expr: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    f(expr)?;
    try_visit_children_rev_mut(expr, |child| try_visit_post_order_rev_mut(child, f))
}

// Fallible pre-order: parent first, then children left-to-right.
pub fn try_visit_pre_order_mut<E>(
    expr: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    f(expr)?;
    try_visit_children_mut(expr, |child| try_visit_pre_order_mut(child, f))
}

// Immutable post-order traversal.
pub fn visit_post_order<'a>(expr: &'a Expr, f: &mut impl FnMut(&'a Expr)) {
    visit_children(expr, |child| visit_post_order(child, f));
    f(expr);
}

// Collect immediate children into a queue (used by visit_expr_nodes_lazy).
pub fn collect_children_mut<'a>(expr: &'a mut Expr, queue: &mut VecDeque<&'a mut Expr>) {
    match expr {
        Expr::Let { expr, .. } => queue.push_back(expr),
        Expr::SelectField { expr, .. } => queue.push_back(expr),
        Expr::SelectIndex { expr, index, .. } => {
            queue.push_back(&mut *expr);
            queue.push_back(&mut *index);
        }
        Expr::Sequence { exprs, .. }
        | Expr::Tuple { exprs, .. }
        | Expr::Concat { exprs, .. }
        | Expr::ExprBlock { exprs, .. } => queue.extend(exprs.iter_mut()),
        Expr::Record { exprs, .. } => queue.extend(exprs.iter_mut().map(|(_, expr)| &mut **expr)),
        Expr::Not { expr, .. } | Expr::Length { expr, .. } | Expr::Unwrap { expr, .. } => {
            queue.push_back(expr)
        }
        Expr::GreaterThan { lhs, rhs, .. }
        | Expr::GreaterThanOrEqualTo { lhs, rhs, .. }
        | Expr::LessThanOrEqualTo { lhs, rhs, .. }
        | Expr::EqualTo { lhs, rhs, .. }
        | Expr::Plus { lhs, rhs, .. }
        | Expr::Minus { lhs, rhs, .. }
        | Expr::Divide { lhs, rhs, .. }
        | Expr::Multiply { lhs, rhs, .. }
        | Expr::LessThan { lhs, rhs, .. }
        | Expr::And { lhs, rhs, .. }
        | Expr::Or { lhs, rhs, .. } => {
            queue.push_back(lhs);
            queue.push_back(rhs);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            queue.push_back(cond);
            queue.push_back(lhs);
            queue.push_back(rhs);
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            queue.push_back(&mut *predicate);
            for arm in match_arms {
                for lit in arm.arm_pattern.get_expr_literals_mut() {
                    queue.push_back(lit.as_mut());
                }
                queue.push_back(&mut arm.arm_resolution_expr);
            }
        }
        Expr::Range { range, .. } => {
            for e in range.get_exprs_mut() {
                queue.push_back(&mut *e);
            }
        }
        Expr::Option {
            expr: Some(expr), ..
        } => queue.push_back(expr),
        Expr::Result { expr: Ok(expr), .. } => queue.push_back(expr),
        Expr::Result {
            expr: Err(expr), ..
        } => queue.push_back(expr),
        Expr::Call {
            call_type,
            args,
            inferred_type,
            ..
        } => {
            let (exprs, worker) = internal::get_expressions_in_call_type_mut(call_type);
            if let Some(exprs) = exprs {
                queue.extend(exprs.iter_mut())
            }
            if let Some(worker) = worker {
                queue.push_back(worker);
            }
            if let TypeInternal::Instance { instance_type } = inferred_type.inner.as_mut() {
                if let Some(worker_expr) = instance_type.worker_mut() {
                    queue.push_back(worker_expr);
                }
            }
            queue.extend(args.iter_mut())
        }
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            queue.push_back(iterable_expr);
            queue.push_back(yield_expr);
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            queue.push_back(iterable_expr);
            queue.push_back(init_value_expr);
            queue.push_back(yield_expr);
        }
        Expr::InvokeMethodLazy {
            lhs,
            args,
            inferred_type,
            ..
        } => {
            if let TypeInternal::Instance { instance_type } = inferred_type.inner.as_mut() {
                if let Some(worker_expr) = instance_type.worker_mut() {
                    queue.push_back(worker_expr);
                }
            }
            queue.push_back(lhs);
            queue.extend(args.iter_mut());
        }
        Expr::GetTag { expr, .. } => queue.push_back(expr),
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

// --- Core child iteration ---

fn visit_children_mut(expr: &mut Expr, mut each: impl FnMut(&mut Expr)) {
    match expr {
        Expr::Let { expr, .. } => each(expr),
        Expr::SelectField { expr, .. } => each(expr),
        Expr::SelectIndex { expr, index, .. } => {
            each(expr);
            each(index);
        }
        Expr::Sequence { exprs, .. }
        | Expr::Tuple { exprs, .. }
        | Expr::Concat { exprs, .. }
        | Expr::ExprBlock { exprs, .. } => {
            for e in exprs {
                each(e);
            }
        }
        Expr::Record { exprs, .. } => {
            for (_, e) in exprs {
                each(e);
            }
        }
        Expr::Not { expr, .. } | Expr::Length { expr, .. } | Expr::Unwrap { expr, .. } => {
            each(expr)
        }
        Expr::GreaterThan { lhs, rhs, .. }
        | Expr::GreaterThanOrEqualTo { lhs, rhs, .. }
        | Expr::LessThanOrEqualTo { lhs, rhs, .. }
        | Expr::EqualTo { lhs, rhs, .. }
        | Expr::Plus { lhs, rhs, .. }
        | Expr::Minus { lhs, rhs, .. }
        | Expr::Divide { lhs, rhs, .. }
        | Expr::Multiply { lhs, rhs, .. }
        | Expr::LessThan { lhs, rhs, .. }
        | Expr::And { lhs, rhs, .. }
        | Expr::Or { lhs, rhs, .. } => {
            each(lhs);
            each(rhs);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            each(cond);
            each(lhs);
            each(rhs);
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            each(predicate);
            for arm in match_arms {
                for lit in arm.arm_pattern.get_expr_literals_mut() {
                    each(lit.as_mut());
                }
                each(&mut arm.arm_resolution_expr);
            }
        }
        Expr::Range { range, .. } => {
            for e in range.get_exprs_mut() {
                each(&mut *e);
            }
        }
        Expr::Option {
            expr: Some(expr), ..
        } => each(expr),
        Expr::Result { expr: Ok(expr), .. } => each(expr),
        Expr::Result {
            expr: Err(expr), ..
        } => each(expr),
        Expr::Call {
            call_type,
            args,
            inferred_type,
            ..
        } => {
            let (exprs, worker) = internal::get_expressions_in_call_type_mut(call_type);
            if let Some(exprs) = exprs {
                for e in exprs {
                    each(e);
                }
            }
            if let Some(worker) = worker {
                each(worker);
            }
            if let TypeInternal::Instance { instance_type } = inferred_type.inner.as_mut() {
                if let Some(worker_expr) = instance_type.worker_mut() {
                    each(worker_expr);
                }
            }
            for arg in args {
                each(arg);
            }
        }
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            each(iterable_expr);
            each(yield_expr);
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            each(iterable_expr);
            each(init_value_expr);
            each(yield_expr);
        }
        Expr::InvokeMethodLazy {
            lhs,
            args,
            inferred_type,
            ..
        } => {
            if let TypeInternal::Instance { instance_type } = inferred_type.inner.as_mut() {
                if let Some(worker_expr) = instance_type.worker_mut() {
                    each(worker_expr);
                }
            }
            each(lhs);
            for arg in args {
                each(arg);
            }
        }
        Expr::GetTag { expr, .. } => each(expr),
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

fn visit_children_rev_mut(expr: &mut Expr, mut each: impl FnMut(&mut Expr)) {
    match expr {
        Expr::Let { expr, .. } => each(expr),
        Expr::SelectField { expr, .. } => each(expr),
        Expr::SelectIndex { expr, index, .. } => {
            each(index);
            each(expr);
        }
        Expr::Sequence { exprs, .. }
        | Expr::Tuple { exprs, .. }
        | Expr::Concat { exprs, .. }
        | Expr::ExprBlock { exprs, .. } => {
            for e in exprs.iter_mut().rev() {
                each(e);
            }
        }
        Expr::Record { exprs, .. } => {
            for (_, e) in exprs.iter_mut().rev() {
                each(e);
            }
        }
        Expr::Not { expr, .. } | Expr::Length { expr, .. } | Expr::Unwrap { expr, .. } => {
            each(expr)
        }
        Expr::GreaterThan { lhs, rhs, .. }
        | Expr::GreaterThanOrEqualTo { lhs, rhs, .. }
        | Expr::LessThanOrEqualTo { lhs, rhs, .. }
        | Expr::EqualTo { lhs, rhs, .. }
        | Expr::Plus { lhs, rhs, .. }
        | Expr::Minus { lhs, rhs, .. }
        | Expr::Divide { lhs, rhs, .. }
        | Expr::Multiply { lhs, rhs, .. }
        | Expr::LessThan { lhs, rhs, .. }
        | Expr::And { lhs, rhs, .. }
        | Expr::Or { lhs, rhs, .. } => {
            each(rhs);
            each(lhs);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            each(rhs);
            each(lhs);
            each(cond);
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            for arm in match_arms.iter_mut().rev() {
                each(&mut arm.arm_resolution_expr);
                for lit in arm.arm_pattern.get_expr_literals_mut().into_iter().rev() {
                    each(lit.as_mut());
                }
            }
            each(predicate);
        }
        Expr::Range { range, .. } => {
            let mut exprs = range.get_exprs_mut();
            exprs.reverse();
            for e in exprs {
                each(&mut *e);
            }
        }
        Expr::Option {
            expr: Some(expr), ..
        } => each(expr),
        Expr::Result { expr: Ok(expr), .. } => each(expr),
        Expr::Result {
            expr: Err(expr), ..
        } => each(expr),
        Expr::Call {
            call_type,
            args,
            inferred_type,
            ..
        } => {
            for arg in args.iter_mut().rev() {
                each(arg);
            }
            if let TypeInternal::Instance { instance_type } = inferred_type.inner.as_mut() {
                if let Some(worker_expr) = instance_type.worker_mut() {
                    each(worker_expr);
                }
            }
            let (exprs, worker) = internal::get_expressions_in_call_type_mut(call_type);
            if let Some(worker) = worker {
                each(worker);
            }
            if let Some(exprs) = exprs {
                for e in exprs.iter_mut().rev() {
                    each(e);
                }
            }
        }
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            each(yield_expr);
            each(iterable_expr);
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            each(yield_expr);
            each(init_value_expr);
            each(iterable_expr);
        }
        Expr::InvokeMethodLazy {
            lhs,
            args,
            inferred_type,
            ..
        } => {
            for arg in args.iter_mut().rev() {
                each(arg);
            }
            each(lhs);
            if let TypeInternal::Instance { instance_type } = inferred_type.inner.as_mut() {
                if let Some(worker_expr) = instance_type.worker_mut() {
                    each(worker_expr);
                }
            }
        }
        Expr::GetTag { expr, .. } => each(expr),
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

fn try_visit_children_mut<E>(
    expr: &mut Expr,
    mut each: impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    // We need to propagate errors properly. Use a Cell to smuggle the
    // error out of the infallible closure interface.
    let mut err: Option<E> = None;
    visit_children_mut(expr, |child| {
        if err.is_none() {
            if let Err(e) = each(child) {
                err = Some(e);
            }
        }
    });
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn try_visit_children_rev_mut<E>(
    expr: &mut Expr,
    mut each: impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    let mut err: Option<E> = None;
    visit_children_rev_mut(expr, |child| {
        if err.is_none() {
            if let Err(e) = each(child) {
                err = Some(e);
            }
        }
    });
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn visit_children<'a>(expr: &'a Expr, mut each: impl FnMut(&'a Expr)) {
    match expr {
        Expr::Let { expr, .. } => each(expr),
        Expr::SelectField { expr, .. } => each(expr),
        Expr::SelectIndex { expr, index, .. } => {
            each(expr);
            each(index);
        }
        Expr::Sequence { exprs, .. }
        | Expr::Tuple { exprs, .. }
        | Expr::Concat { exprs, .. }
        | Expr::ExprBlock { exprs, .. } => {
            for e in exprs {
                each(e);
            }
        }
        Expr::Record { exprs, .. } => {
            for (_, e) in exprs {
                each(e);
            }
        }
        Expr::Not { expr, .. } | Expr::Length { expr, .. } | Expr::Unwrap { expr, .. } => {
            each(expr)
        }
        Expr::GreaterThan { lhs, rhs, .. }
        | Expr::GreaterThanOrEqualTo { lhs, rhs, .. }
        | Expr::LessThanOrEqualTo { lhs, rhs, .. }
        | Expr::EqualTo { lhs, rhs, .. }
        | Expr::Plus { lhs, rhs, .. }
        | Expr::Minus { lhs, rhs, .. }
        | Expr::Divide { lhs, rhs, .. }
        | Expr::Multiply { lhs, rhs, .. }
        | Expr::LessThan { lhs, rhs, .. }
        | Expr::And { lhs, rhs, .. }
        | Expr::Or { lhs, rhs, .. } => {
            each(lhs);
            each(rhs);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            each(cond);
            each(lhs);
            each(rhs);
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            each(predicate);
            for arm in match_arms {
                for lit in arm.arm_pattern.get_expr_literals() {
                    each(lit);
                }
                each(&arm.arm_resolution_expr);
            }
        }
        Expr::Range { range, .. } => {
            for e in range.get_exprs() {
                each(e);
            }
        }
        Expr::Option {
            expr: Some(expr), ..
        } => each(expr),
        Expr::Result { expr: Ok(expr), .. } => each(expr),
        Expr::Result {
            expr: Err(expr), ..
        } => each(expr),
        Expr::Call { args, .. } => {
            for arg in args {
                each(arg);
            }
        }
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            each(iterable_expr);
            each(yield_expr);
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            each(iterable_expr);
            each(init_value_expr);
            each(yield_expr);
        }
        Expr::InvokeMethodLazy { lhs, args, .. } => {
            each(lhs);
            for arg in args {
                each(arg);
            }
        }
        Expr::GetTag { expr, .. } => each(expr),
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

mod internal {
    use crate::call_type::{CallType, InstanceCreationType};
    use crate::Expr;

    pub(crate) fn get_expressions_in_call_type_mut(
        call_type: &mut CallType,
    ) -> (Option<&mut [Expr]>, Option<&mut Box<Expr>>) {
        match call_type {
            CallType::Function {
                instance_identifier: module,
                ..
            } => (None, module.as_mut().and_then(|m| m.worker_name_mut())),

            CallType::InstanceCreation(instance_creation) => match instance_creation {
                InstanceCreationType::WitWorker { worker_name, .. } => (None, worker_name.as_mut()),

                InstanceCreationType::WitResource { module, .. } => {
                    (None, module.as_mut().and_then(|m| m.worker_name_mut()))
                }
            },

            CallType::VariantConstructor(_) => (None, None),
            CallType::EnumConstructor(_) => (None, None),
        }
    }
}
