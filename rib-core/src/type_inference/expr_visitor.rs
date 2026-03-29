use crate::{Expr, TypeInternal};
use std::collections::VecDeque;

// Safe post-order (bottom-up) traversal visiting children before parent.
// Visits in natural program order (left-to-right).
// Equivalent to old: ExprVisitor::bottom_up(expr) + pop_front()
pub fn visit_post_order_mut(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    {
        let mut children = VecDeque::new();
        collect_children_mut(expr, &mut children);
        while let Some(child) = children.pop_front() {
            visit_post_order_mut(child, f);
        }
    }
    f(expr);
}

// Safe post-order (bottom-up) traversal visiting children before parent.
// Visits in reversed order (right-to-left, parent last).
// Equivalent to old: ExprVisitor::bottom_up(expr) + pop_back()
pub fn visit_post_order_rev_mut(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    f(expr);
    {
        let mut children = VecDeque::new();
        collect_children_mut(expr, &mut children);
        while let Some(child) = children.pop_back() {
            visit_post_order_rev_mut(child, f);
        }
    }
}

// Safe pre-order (top-down) traversal visiting parent before children.
// Visits in natural program order (left-to-right).
// Equivalent to old: ExprVisitor::top_down(expr) + pop_front()
pub fn visit_pre_order_mut(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    f(expr);
    {
        let mut children = VecDeque::new();
        collect_children_mut(expr, &mut children);
        while let Some(child) = children.pop_front() {
            visit_pre_order_mut(child, f);
        }
    }
}

// Fallible post-order (bottom-up) traversal. Stops on first error.
// Visits in natural program order (left-to-right).
// Equivalent to old: ExprVisitor::bottom_up(expr) + pop_front()
pub fn try_visit_post_order_mut<E>(
    expr: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    {
        let mut children = VecDeque::new();
        collect_children_mut(expr, &mut children);
        while let Some(child) = children.pop_front() {
            try_visit_post_order_mut(child, f)?;
        }
    }
    f(expr)
}

// Fallible post-order (bottom-up) traversal in reversed order.
// Equivalent to old: ExprVisitor::bottom_up(expr) + pop_back()
pub fn try_visit_post_order_rev_mut<E>(
    expr: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    f(expr)?;
    {
        let mut children = VecDeque::new();
        collect_children_mut(expr, &mut children);
        while let Some(child) = children.pop_back() {
            try_visit_post_order_rev_mut(child, f)?;
        }
    }
    Ok(())
}

// Fallible pre-order (top-down) traversal. Stops on first error.
// Equivalent to old: ExprVisitor::top_down(expr) + pop_front()
pub fn try_visit_pre_order_mut<E>(
    expr: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), E>,
) -> Result<(), E> {
    f(expr)?;
    {
        let mut children = VecDeque::new();
        collect_children_mut(expr, &mut children);
        while let Some(child) = children.pop_front() {
            try_visit_pre_order_mut(child, f)?;
        }
    }
    Ok(())
}

// Immutable post-order traversal for read-only inspection.
pub fn visit_post_order<'a>(expr: &'a Expr, f: &mut impl FnMut(&'a Expr)) {
    match expr {
        Expr::Let { expr, .. } => visit_post_order(expr, f),
        Expr::SelectField { expr, .. } => visit_post_order(expr, f),
        Expr::SelectIndex { expr, index, .. } => {
            visit_post_order(expr, f);
            visit_post_order(index, f);
        }
        Expr::Sequence { exprs, .. } => {
            for e in exprs {
                visit_post_order(e, f);
            }
        }
        Expr::Record { exprs, .. } => {
            for (_, e) in exprs {
                visit_post_order(e, f);
            }
        }
        Expr::Tuple { exprs, .. } => {
            for e in exprs {
                visit_post_order(e, f);
            }
        }
        Expr::Concat { exprs, .. } => {
            for e in exprs {
                visit_post_order(e, f);
            }
        }
        Expr::ExprBlock { exprs, .. } => {
            for e in exprs {
                visit_post_order(e, f);
            }
        }
        Expr::Not { expr, .. } => visit_post_order(expr, f),
        Expr::Length { expr, .. } => visit_post_order(expr, f),
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
            visit_post_order(lhs, f);
            visit_post_order(rhs, f);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            visit_post_order(cond, f);
            visit_post_order(lhs, f);
            visit_post_order(rhs, f);
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            visit_post_order(predicate, f);
            for arm in match_arms {
                for lit in arm.arm_pattern.get_expr_literals() {
                    visit_post_order(lit, f);
                }
                visit_post_order(&arm.arm_resolution_expr, f);
            }
        }
        Expr::Range { range, .. } => {
            for e in range.get_exprs() {
                visit_post_order(e, f);
            }
        }
        Expr::Option {
            expr: Some(expr), ..
        } => visit_post_order(expr, f),
        Expr::Result { expr: Ok(expr), .. } => visit_post_order(expr, f),
        Expr::Result {
            expr: Err(expr), ..
        } => visit_post_order(expr, f),
        Expr::Call { args, .. } => {
            for arg in args {
                visit_post_order(arg, f);
            }
        }
        Expr::Unwrap { expr, .. } => visit_post_order(expr, f),
        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            visit_post_order(iterable_expr, f);
            visit_post_order(yield_expr, f);
        }
        Expr::ListReduce {
            iterable_expr,
            init_value_expr,
            yield_expr,
            ..
        } => {
            visit_post_order(iterable_expr, f);
            visit_post_order(init_value_expr, f);
            visit_post_order(yield_expr, f);
        }
        Expr::InvokeMethodLazy { lhs, args, .. } => {
            visit_post_order(lhs, f);
            for arg in args {
                visit_post_order(arg, f);
            }
        }
        Expr::GetTag { expr, .. } => visit_post_order(expr, f),
        Expr::Literal { .. }
        | Expr::Number { .. }
        | Expr::Flags { .. }
        | Expr::Identifier { .. }
        | Expr::Boolean { .. }
        | Expr::Option { expr: None, .. }
        | Expr::Throw { .. }
        | Expr::GenerateWorkerName { .. } => {}
    }
    f(expr);
}

// Collect only the immediate children of an expression into the queue.
// This is safe because siblings are disjoint borrows.
pub fn collect_children_mut<'a>(expr: &'a mut Expr, queue: &mut VecDeque<&'a mut Expr>) {
    match expr {
        Expr::Let { expr, .. } => queue.push_back(&mut *expr),
        Expr::SelectField { expr, .. } => queue.push_back(&mut *expr),
        Expr::SelectIndex { expr, index, .. } => {
            queue.push_back(&mut *expr);
            queue.push_back(&mut *index);
        }
        Expr::Sequence { exprs, .. } => queue.extend(exprs.iter_mut()),
        Expr::Record { exprs, .. } => queue.extend(exprs.iter_mut().map(|(_, expr)| &mut **expr)),
        Expr::Tuple { exprs, .. } => queue.extend(exprs.iter_mut()),
        Expr::Concat { exprs, .. } => queue.extend(exprs.iter_mut()),
        Expr::ExprBlock { exprs, .. } => queue.extend(exprs.iter_mut()),
        Expr::Not { expr, .. } => queue.push_back(&mut *expr),
        Expr::Length { expr, .. } => queue.push_back(&mut *expr),
        Expr::GreaterThan { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::GreaterThanOrEqualTo { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::LessThanOrEqualTo { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::EqualTo { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::Plus { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::Minus { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::Divide { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::Multiply { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::LessThan { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::Cond { cond, lhs, rhs, .. } => {
            queue.push_back(&mut *cond);
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs);
        }
        Expr::PatternMatch {
            predicate,
            match_arms,
            ..
        } => {
            queue.push_back(&mut *predicate);
            for arm in match_arms {
                let arm_literal_expressions = arm.arm_pattern.get_expr_literals_mut();
                queue.extend(arm_literal_expressions.into_iter().map(|x| x.as_mut()));
                queue.push_back(&mut *arm.arm_resolution_expr);
            }
        }

        Expr::Range { range, .. } => {
            for expr in range.get_exprs_mut() {
                queue.push_back(&mut *expr);
            }
        }

        Expr::Option {
            expr: Some(expr), ..
        } => queue.push_back(&mut *expr),
        Expr::Result { expr: Ok(expr), .. } => queue.push_back(&mut *expr),
        Expr::Result {
            expr: Err(expr), ..
        } => queue.push_back(&mut *expr),
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
        Expr::Unwrap { expr, .. } => queue.push_back(&mut *expr),
        Expr::And { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs)
        }

        Expr::Or { lhs, rhs, .. } => {
            queue.push_back(&mut *lhs);
            queue.push_back(&mut *rhs)
        }

        Expr::ListComprehension {
            iterable_expr,
            yield_expr,
            ..
        } => {
            queue.push_back(&mut *iterable_expr);
            queue.push_back(&mut *yield_expr);
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

        Expr::GetTag { expr, .. } => {
            queue.push_back(&mut *expr);
        }

        Expr::Literal { .. } => {}
        Expr::Number { .. } => {}
        Expr::Flags { .. } => {}
        Expr::Identifier { .. } => {}
        Expr::Boolean { .. } => {}
        Expr::Option { expr: None, .. } => {}
        Expr::Throw { .. } => {}
        Expr::GenerateWorkerName { .. } => {}
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
