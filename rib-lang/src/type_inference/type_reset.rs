use crate::{visit_post_order_rev_mut, Expr, InferredType};

pub fn reset_type_info(expr: &mut Expr) {
    visit_post_order_rev_mut(expr, &mut |expr| {
        expr.with_inferred_type_mut(InferredType::unknown());
    });
}
