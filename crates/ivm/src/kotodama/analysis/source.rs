use std::collections::HashSet;

use super::{AnalysisCategory, AnalysisFinding};
use crate::kotodama::{
    ast::{Block, Expr, Item, Program, Statement},
    semantic::{
        self, ExprKind, TypedBlock, TypedExpr, TypedFunction, TypedProgram, TypedStatement,
    },
};

/// Run static analysis on a parsed Kotodama program using the associated typed
/// information produced by the semantic analyzer.
pub fn run_static_analysis(program: &Program, typed: &TypedProgram) -> Vec<AnalysisFinding> {
    let state_names = collect_state_names(program);
    let mut findings = Vec::new();
    detect_literal_overflow(typed, &mut findings);
    detect_division_by_zero(typed, &mut findings);
    detect_reentrancy(program, &state_names, &mut findings);
    detect_infinite_loops(program, &mut findings);
    findings
}

fn collect_state_names(program: &Program) -> HashSet<String> {
    program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::State(state) => Some(state.name.clone()),
            _ => None,
        })
        .collect()
}

fn detect_literal_overflow(typed: &TypedProgram, findings: &mut Vec<AnalysisFinding>) {
    for item in &typed.items {
        let semantic::TypedItem::Function(func) = item;
        visit_exprs(func, &mut |func_name, expr| {
            if semantic::resolve_struct_type(&expr.ty) != semantic::Type::Int {
                return;
            }
            if let ExprKind::Binary { op, left, right } = &expr.expr {
                let lhs = literal_i64(left);
                let rhs = literal_i64(right);
                match op {
                    crate::kotodama::ast::BinaryOp::Add => {
                        if let (Some(a), Some(b)) = (lhs, rhs) {
                            let sum = (a as i128) + (b as i128);
                            if sum < i64::MIN as i128 || sum > i64::MAX as i128 {
                                findings.push(AnalysisFinding::warning(
                                    AnalysisCategory::StaticSource,
                                    "static-overflow-literal",
                                    format!(
                                        "function `{func_name}` adds {a} and {b}, which overflows 64-bit range"
                                    ),
                                ));
                            }
                        }
                    }
                    crate::kotodama::ast::BinaryOp::Sub => {
                        if let (Some(a), Some(b)) = (lhs, rhs) {
                            let diff = (a as i128) - (b as i128);
                            if diff < i64::MIN as i128 || diff > i64::MAX as i128 {
                                findings.push(AnalysisFinding::warning(
                                    AnalysisCategory::StaticSource,
                                    "static-overflow-literal",
                                    format!(
                                        "function `{func_name}` subtracts {b} from {a}, which overflows 64-bit range"
                                    ),
                                ));
                            }
                        }
                    }
                    crate::kotodama::ast::BinaryOp::Mul => {
                        if let (Some(a), Some(b)) = (lhs, rhs) {
                            let prod = (a as i128) * (b as i128);
                            if prod < i64::MIN as i128 || prod > i64::MAX as i128 {
                                findings.push(AnalysisFinding::warning(
                                    AnalysisCategory::StaticSource,
                                    "static-overflow-literal",
                                    format!(
                                        "function `{func_name}` multiplies {a} by {b}, which overflows 64-bit range"
                                    ),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

fn detect_division_by_zero(typed: &TypedProgram, findings: &mut Vec<AnalysisFinding>) {
    for item in &typed.items {
        let semantic::TypedItem::Function(func) = item;
        visit_exprs(func, &mut |func_name, expr| {
            if !semantic::is_numeric_type(&expr.ty) {
                return;
            }
            if let ExprKind::Binary { op, right, .. } = &expr.expr
                && matches!(
                    op,
                    crate::kotodama::ast::BinaryOp::Div | crate::kotodama::ast::BinaryOp::Mod
                )
                && literal_i64(right) == Some(0)
            {
                findings.push(AnalysisFinding::warning(
                    AnalysisCategory::StaticSource,
                    "static-div-zero",
                    format!("function `{func_name}` performs {op:?} with a literal zero divisor"),
                ));
            }
        });
    }
}

fn literal_i64(expr: &TypedExpr) -> Option<i64> {
    match &expr.expr {
        ExprKind::Number(n) => Some(*n),
        ExprKind::NumericCast { expr } => literal_i64(expr),
        ExprKind::Unary {
            op: crate::kotodama::ast::UnaryOp::Neg,
            expr,
        } => literal_i64(expr).and_then(|v| v.checked_neg()),
        _ => None,
    }
}

fn detect_reentrancy(
    program: &Program,
    state_names: &HashSet<String>,
    findings: &mut Vec<AnalysisFinding>,
) {
    for item in &program.items {
        if let Item::Function(func) = item {
            analyze_block_reentrancy(&func.body, state_names, false, &func.name, findings);
        }
    }
}

fn analyze_block_reentrancy(
    block: &Block,
    state_names: &HashSet<String>,
    mut state_before: bool,
    func_name: &str,
    findings: &mut Vec<AnalysisFinding>,
) -> bool {
    for stmt in &block.statements {
        state_before =
            analyze_statement_reentrancy(stmt, state_names, state_before, func_name, findings);
    }
    state_before
}

fn analyze_statement_reentrancy(
    stmt: &Statement,
    state_names: &HashSet<String>,
    state_before: bool,
    func_name: &str,
    findings: &mut Vec<AnalysisFinding>,
) -> bool {
    match stmt {
        Statement::Let { value, .. } => {
            visit_expr_for_host_calls(value, state_before, func_name, findings);
            state_before
        }
        Statement::Assign { name, value } => {
            visit_expr_for_host_calls(value, state_before, func_name, findings);
            if state_names.contains(name) {
                true
            } else {
                state_before
            }
        }
        Statement::AssignExpr { target, value, .. } => {
            visit_expr_for_host_calls(target, state_before, func_name, findings);
            visit_expr_for_host_calls(value, state_before, func_name, findings);
            if expr_targets_state(target, state_names) {
                true
            } else {
                state_before
            }
        }
        Statement::Expr(expr) => {
            visit_expr_for_host_calls(expr, state_before, func_name, findings);
            state_before
        }
        Statement::Return(Some(expr)) => {
            visit_expr_for_host_calls(expr, state_before, func_name, findings);
            state_before
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => state_before,
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            visit_expr_for_host_calls(cond, state_before, func_name, findings);
            let then_state = analyze_block_reentrancy(
                then_branch,
                state_names,
                state_before,
                func_name,
                findings,
            );
            let else_state = else_branch.as_ref().map_or(state_before, |block| {
                analyze_block_reentrancy(block, state_names, state_before, func_name, findings)
            });
            state_before || then_state || else_state
        }
        Statement::While { cond, body } => {
            visit_expr_for_host_calls(cond, state_before, func_name, findings);
            let body_state =
                analyze_block_reentrancy(body, state_names, state_before, func_name, findings);
            state_before || body_state
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            let mut current = state_before;
            if let Some(init_stmt) = init {
                current = analyze_statement_reentrancy(
                    init_stmt,
                    state_names,
                    current,
                    func_name,
                    findings,
                );
            }
            if let Some(cond_expr) = cond {
                visit_expr_for_host_calls(cond_expr, current, func_name, findings);
            }
            let body_state =
                analyze_block_reentrancy(body, state_names, current, func_name, findings);
            current = current || body_state;
            if let Some(step_stmt) = step {
                current = analyze_statement_reentrancy(
                    step_stmt,
                    state_names,
                    current,
                    func_name,
                    findings,
                );
            }
            current
        }
        Statement::ForEachMap { map, body, .. } => {
            visit_expr_for_host_calls(map, state_before, func_name, findings);
            let body_state =
                analyze_block_reentrancy(body, state_names, state_before, func_name, findings);
            state_before || body_state
        }
    }
}

fn visit_expr_for_host_calls(
    expr: &Expr,
    state_before: bool,
    func_name: &str,
    findings: &mut Vec<AnalysisFinding>,
) {
    match expr {
        Expr::Call { name, args } => {
            if state_before && is_external_call(name) {
                findings.push(AnalysisFinding::warning(
                    AnalysisCategory::StaticSource,
                    "static-reentrancy-risk",
                    format!(
                        "function `{func_name}` writes contract state before calling `{name}`; review for reentrancy"
                    ),
                ));
            }
            for arg in args {
                visit_expr_for_host_calls(arg, state_before, func_name, findings);
            }
        }
        Expr::Binary { left, right, .. } => {
            visit_expr_for_host_calls(left, state_before, func_name, findings);
            visit_expr_for_host_calls(right, state_before, func_name, findings);
        }
        Expr::Unary { expr, .. } => {
            visit_expr_for_host_calls(expr, state_before, func_name, findings)
        }
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            visit_expr_for_host_calls(cond, state_before, func_name, findings);
            visit_expr_for_host_calls(then_expr, state_before, func_name, findings);
            visit_expr_for_host_calls(else_expr, state_before, func_name, findings);
        }
        Expr::Member { object, .. } => {
            visit_expr_for_host_calls(object, state_before, func_name, findings);
        }
        Expr::Index { target, index } => {
            visit_expr_for_host_calls(target, state_before, func_name, findings);
            visit_expr_for_host_calls(index, state_before, func_name, findings);
        }
        Expr::Tuple(items) => {
            for item in items {
                visit_expr_for_host_calls(item, state_before, func_name, findings);
            }
        }
        Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Decimal(_)
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => {}
    }
}

fn expr_targets_state(expr: &Expr, state_names: &HashSet<String>) -> bool {
    match expr {
        Expr::Ident(name) => state_names.contains(name),
        Expr::Member { object, .. } | Expr::Index { target: object, .. } => {
            expr_targets_state(object, state_names)
        }
        Expr::Tuple(items) => items
            .iter()
            .any(|item| expr_targets_state(item, state_names)),
        Expr::Call { .. }
        | Expr::Binary { .. }
        | Expr::Unary { .. }
        | Expr::Conditional { .. }
        | Expr::Bool(_)
        | Expr::Number(_)
        | Expr::Decimal(_)
        | Expr::String(_)
        | Expr::Bytes(_) => false,
    }
}

fn is_external_call(name: &str) -> bool {
    const HOST_PREFIXES: [&str; 2] = ["host::", "std::host::"];
    if HOST_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
        return true;
    }
    let simple_name = name.split("::").last().unwrap_or(name);
    matches!(
        simple_name,
        "call" | "call_contract" | "call_async" | "invoke" | "invoke_contract" | "transfer"
    )
}

fn detect_infinite_loops(program: &Program, findings: &mut Vec<AnalysisFinding>) {
    for item in &program.items {
        if let Item::Function(function) = item {
            inspect_block_for_loop(&function.body, &function.name, findings);
        }
    }
}

fn inspect_block_for_loop(block: &Block, func_name: &str, findings: &mut Vec<AnalysisFinding>) {
    for stmt in &block.statements {
        match stmt {
            Statement::While { cond, body } => {
                if matches!(cond, Expr::Bool(true)) && !block_contains_escape(body) {
                    findings.push(AnalysisFinding::warning(
                        AnalysisCategory::StaticSource,
                        "static-infinite-loop",
                        format!(
                            "function `{func_name}` contains a `while true` loop without an obvious break or return"
                        ),
                    ));
                }
                inspect_block_for_loop(body, func_name, findings);
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                inspect_block_for_loop(then_branch, func_name, findings);
                if let Some(else_branch) = else_branch {
                    inspect_block_for_loop(else_branch, func_name, findings);
                }
            }
            Statement::For { body, .. } => {
                inspect_block_for_loop(body, func_name, findings);
            }
            Statement::ForEachMap { body, .. } => {
                inspect_block_for_loop(body, func_name, findings);
            }
            Statement::Let { .. }
            | Statement::Assign { .. }
            | Statement::AssignExpr { .. }
            | Statement::Expr(_)
            | Statement::Return(_)
            | Statement::Break
            | Statement::Continue => {}
        }
    }
}

fn block_contains_escape(block: &Block) -> bool {
    for stmt in &block.statements {
        match stmt {
            Statement::Return(_) | Statement::Break => return true,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                if block_contains_escape(then_branch) {
                    return true;
                }
                if let Some(else_branch) = else_branch
                    && block_contains_escape(else_branch)
                {
                    return true;
                }
            }
            Statement::While { body, .. } => {
                if block_contains_escape(body) {
                    return true;
                }
            }
            Statement::For { body, .. } | Statement::ForEachMap { body, .. } => {
                if block_contains_escape(body) {
                    return true;
                }
            }
            Statement::Let { .. }
            | Statement::Assign { .. }
            | Statement::AssignExpr { .. }
            | Statement::Expr(_)
            | Statement::Continue => {}
        }
    }
    false
}

fn visit_exprs<F>(func: &TypedFunction, visitor: &mut F)
where
    F: FnMut(&str, &TypedExpr),
{
    visit_block_exprs(&func.body, &func.name, visitor);
}

fn visit_block_exprs<F>(block: &TypedBlock, func_name: &str, visitor: &mut F)
where
    F: FnMut(&str, &TypedExpr),
{
    for stmt in &block.statements {
        visit_statement_exprs(stmt, func_name, visitor);
    }
}

fn visit_statement_exprs<F>(stmt: &TypedStatement, func_name: &str, visitor: &mut F)
where
    F: FnMut(&str, &TypedExpr),
{
    match stmt {
        TypedStatement::Let { value, .. } => {
            visitor(func_name, value);
            visit_expr_children(value, func_name, visitor);
        }
        TypedStatement::Expr(expr) => {
            visitor(func_name, expr);
            visit_expr_children(expr, func_name, visitor);
        }
        TypedStatement::Return(Some(expr)) => {
            visitor(func_name, expr);
            visit_expr_children(expr, func_name, visitor);
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            visitor(func_name, cond);
            visit_expr_children(cond, func_name, visitor);
            visit_block_exprs(then_branch, func_name, visitor);
            if let Some(block) = else_branch {
                visit_block_exprs(block, func_name, visitor);
            }
        }
        TypedStatement::While { cond, body } => {
            visitor(func_name, cond);
            visit_expr_children(cond, func_name, visitor);
            visit_block_exprs(body, func_name, visitor);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init {
                visit_statement_exprs(init_stmt, func_name, visitor);
            }
            if let Some(cond_expr) = cond {
                visitor(func_name, cond_expr);
                visit_expr_children(cond_expr, func_name, visitor);
            }
            visit_block_exprs(body, func_name, visitor);
            if let Some(step_stmt) = step {
                visit_statement_exprs(step_stmt, func_name, visitor);
            }
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            visitor(func_name, map);
            visit_expr_children(map, func_name, visitor);
            visit_block_exprs(body, func_name, visitor);
        }
        TypedStatement::MapSet { map, key, value } => {
            visitor(func_name, map);
            visit_expr_children(map, func_name, visitor);
            visitor(func_name, key);
            visit_expr_children(key, func_name, visitor);
            visitor(func_name, value);
            visit_expr_children(value, func_name, visitor);
        }
    }
}

fn visit_expr_children<F>(expr: &TypedExpr, func_name: &str, visitor: &mut F)
where
    F: FnMut(&str, &TypedExpr),
{
    match &expr.expr {
        ExprKind::Binary { left, right, .. } => {
            visitor(func_name, left);
            visit_expr_children(left, func_name, visitor);
            visitor(func_name, right);
            visit_expr_children(right, func_name, visitor);
        }
        ExprKind::Unary { expr, .. } => {
            visitor(func_name, expr);
            visit_expr_children(expr, func_name, visitor);
        }
        ExprKind::NumericCast { expr } => {
            visitor(func_name, expr);
            visit_expr_children(expr, func_name, visitor);
        }
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            visitor(func_name, cond);
            visit_expr_children(cond, func_name, visitor);
            visitor(func_name, then_expr);
            visit_expr_children(then_expr, func_name, visitor);
            visitor(func_name, else_expr);
            visit_expr_children(else_expr, func_name, visitor);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                visitor(func_name, arg);
                visit_expr_children(arg, func_name, visitor);
            }
        }
        ExprKind::Tuple(items) => {
            for item in items {
                visitor(func_name, item);
                visit_expr_children(item, func_name, visitor);
            }
        }
        ExprKind::Member { object, .. } => {
            visitor(func_name, object);
            visit_expr_children(object, func_name, visitor);
        }
        ExprKind::Index { target, index } => {
            visitor(func_name, target);
            visit_expr_children(target, func_name, visitor);
            visitor(func_name, index);
            visit_expr_children(index, func_name, visitor);
        }
        ExprKind::Number(_)
        | ExprKind::Decimal(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotodama::parser::parse;

    fn analyze_static(source: &str) -> Vec<AnalysisFinding> {
        let program = parse(source).expect("parse");
        let typed = semantic::analyze(&program).expect("type check");
        run_static_analysis(&program, &typed)
    }

    #[test]
    fn detects_constant_overflow() {
        let findings = analyze_static(
            r#"
            fn overflow() -> int {
                return 9223372036854775807 + 1;
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-overflow-literal"),
            "expected overflow finding, got {findings:?}"
        );
    }

    #[test]
    fn detects_division_by_zero_literal() {
        let findings = analyze_static(
            r#"
            fn div_zero() -> int {
                let x = 10;
                return x / 0;
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-div-zero"),
            "expected div-zero finding, got {findings:?}"
        );
    }

    #[test]
    fn detects_reentrancy_pattern() {
        let findings = analyze_static(
            r#"
            state Map<int, int> balances;

            fn withdraw() {
                balances[1] = 0;
                host::call_contract();
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "expected reentrancy finding, got {findings:?}"
        );
    }

    #[test]
    fn detects_infinite_loop() {
        let findings = analyze_static(
            r#"
            fn spin() {
                while true {
                    let x = 1;
                }
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-infinite-loop"),
            "expected loop finding, got {findings:?}"
        );
    }

    #[test]
    fn unary_neg_on_i64_min_literal_does_not_panic() {
        let findings = analyze_static(
            r#"
            fn neg_min() -> int {
                return -(-9223372036854775808);
            }
        "#,
        );
        assert!(findings.is_empty(), "unexpected findings: {findings:?}");
    }

    #[test]
    fn reentrancy_warning_only_when_write_precedes_call() {
        let findings = analyze_static(
            r#"
            state Map<int, int> balances;

            fn withdraw() {
                balances[1] = balances[1] - 1;
                host::call_contract();
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "expected reentrancy finding, got {findings:?}"
        );
    }

    #[test]
    fn reentrancy_not_reported_when_call_is_before_write() {
        let findings = analyze_static(
            r#"
            state Map<int, int> balances;

            fn withdraw_safe() {
                host::call_contract();
                balances[1] = balances[1] - 1;
            }
        "#,
        );
        assert!(
            !findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "unexpected reentrancy finding: {findings:?}"
        );
    }
}
