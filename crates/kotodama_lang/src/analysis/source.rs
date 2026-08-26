use super::{AnalysisCategory, AnalysisFinding};
use crate::{
    ast::{Block, Expr, Item, Program, Statement},
    builtins::Builtin,
    semantic::{
        self, ExprKind, TypedBlock, TypedExpr, TypedFunction, TypedProgram, TypedStatement,
    },
};
use std::collections::HashSet;
/// Run static analysis on a parsed Kotodama program using the associated typed
/// information produced by the semantic analyzer.
pub fn run_static_analysis(program: &Program, typed: &TypedProgram) -> Vec<AnalysisFinding> {
    let state_names = collect_state_names(program);
    let mut findings = Vec::new();
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
fn detect_division_by_zero(typed: &TypedProgram, findings: &mut Vec<AnalysisFinding>) {
    for item in &typed.items {
        let semantic::TypedItem::Function(func) = item;
        visit_exprs(func, &mut |func_name, expr| {
            if !semantic::is_numeric_type(&expr.ty) {
                return;
            }
            if let ExprKind::Binary { op, right, .. } = &expr.expr
                && matches!(op, crate::ast::BinaryOp::Div | crate::ast::BinaryOp::Mod)
                && exact_numeric_literal_is_zero(right)
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
fn exact_numeric_literal_is_zero(expr: &TypedExpr) -> bool {
    match &expr.expr {
        ExprKind::IntLiteral(value) => value.is_zero(),
        ExprKind::DecimalLiteral { value, .. } => value.is_zero(),
        ExprKind::NumericCast { expr } => exact_numeric_literal_is_zero(expr),
        ExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            expr,
        } => exact_numeric_literal_is_zero(expr),
        _ => false,
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
    if let Some(tail) = &block.tail {
        state_before =
            visit_expr_for_host_calls(tail, state_names, state_before, func_name, findings);
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
        Statement::Source { statement, .. } | Statement::Resolved { statement, .. } => {
            analyze_statement_reentrancy(statement, state_names, state_before, func_name, findings)
        }
        Statement::Let { value, .. } => {
            visit_expr_for_host_calls(value, state_names, state_before, func_name, findings)
        }
        Statement::Assign { name, value } => {
            let state_after_value =
                visit_expr_for_host_calls(value, state_names, state_before, func_name, findings);
            state_after_value || state_names.contains(name)
        }
        Statement::AssignExpr { target, value, .. } => {
            let state_after_target =
                visit_expr_for_host_calls(target, state_names, state_before, func_name, findings);
            let state_after_value = visit_expr_for_host_calls(
                value,
                state_names,
                state_after_target,
                func_name,
                findings,
            );
            state_after_value || expr_targets_state(target, state_names)
        }
        Statement::Expr(expr) | Statement::Return(Some(expr)) => {
            visit_expr_for_host_calls(expr, state_names, state_before, func_name, findings)
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => state_before,
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let state_after_condition =
                visit_expr_for_host_calls(cond, state_names, state_before, func_name, findings);
            let then_state = analyze_block_reentrancy(
                then_branch,
                state_names,
                state_after_condition,
                func_name,
                findings,
            );
            let else_state = else_branch.as_ref().map_or(state_after_condition, |block| {
                analyze_block_reentrancy(
                    block,
                    state_names,
                    state_after_condition,
                    func_name,
                    findings,
                )
            });
            then_state || else_state
        }
        Statement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            let state_after_value =
                visit_expr_for_host_calls(value, state_names, state_before, func_name, findings);
            let then_state = analyze_block_reentrancy(
                then_branch,
                state_names,
                state_after_value,
                func_name,
                findings,
            );
            let else_state = else_branch.as_ref().map_or(state_after_value, |block| {
                analyze_block_reentrancy(block, state_names, state_after_value, func_name, findings)
            });
            then_state || else_state
        }
        Statement::While { cond, body } => {
            let state_after_condition =
                visit_expr_for_host_calls(cond, state_names, state_before, func_name, findings);
            let body_state = analyze_block_reentrancy(
                body,
                state_names,
                state_after_condition,
                func_name,
                findings,
            );
            // A later iteration observes writes established by the first.
            if body_state && !state_after_condition {
                visit_expr_for_host_calls(cond, state_names, true, func_name, findings);
                analyze_block_reentrancy(body, state_names, true, func_name, findings);
            }
            body_state
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
                current =
                    visit_expr_for_host_calls(cond_expr, state_names, current, func_name, findings);
            }
            let state_before_iteration = current;
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
            // Revisit one iteration after the first establishes a write; the
            // state flag is monotonic, so additional passes add no information.
            if current && !state_before_iteration {
                if let Some(cond_expr) = cond {
                    visit_expr_for_host_calls(cond_expr, state_names, true, func_name, findings);
                }
                analyze_block_reentrancy(body, state_names, true, func_name, findings);
                if let Some(step_stmt) = step {
                    analyze_statement_reentrancy(step_stmt, state_names, true, func_name, findings);
                }
            }
            current
        }
        Statement::ForEachMap { map, body, .. } => {
            let state_after_map =
                visit_expr_for_host_calls(map, state_names, state_before, func_name, findings);
            let body_state =
                analyze_block_reentrancy(body, state_names, state_after_map, func_name, findings);
            // A later map entry observes writes established for an earlier one.
            if body_state && !state_after_map {
                analyze_block_reentrancy(body, state_names, true, func_name, findings);
            }
            body_state
        }
    }
}
fn visit_expr_for_host_calls(
    expr: &Expr,
    state_names: &HashSet<String>,
    state_before: bool,
    func_name: &str,
    findings: &mut Vec<AnalysisFinding>,
) -> bool {
    match expr {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            visit_expr_for_host_calls(expression, state_names, state_before, func_name, findings)
        }
        Expr::Call { name, args, .. } => {
            let state_after_arguments = args.iter().fold(state_before, |state, argument| {
                visit_expr_for_host_calls(argument, state_names, state, func_name, findings)
            });
            if state_after_arguments && is_external_call(name) {
                let message = format!(
                    "function `{func_name}` writes seiyaku state before calling `{name}`; review for reentrancy"
                );
                if !findings.iter().any(|finding| {
                    finding.code == "static-reentrancy-risk" && finding.message == message
                }) {
                    findings.push(AnalysisFinding::warning(
                        AnalysisCategory::StaticSource,
                        "static-reentrancy-risk",
                        message,
                    ));
                }
            }
            state_after_arguments
        }
        Expr::Binary { left, right, .. } => {
            let state_after_left =
                visit_expr_for_host_calls(left, state_names, state_before, func_name, findings);
            visit_expr_for_host_calls(right, state_names, state_after_left, func_name, findings)
        }
        Expr::Unary { expr, .. }
        | Expr::OptionSome(expr)
        | Expr::ResultOk(expr)
        | Expr::ResultErr(expr)
        | Expr::Propagate(expr) => {
            visit_expr_for_host_calls(expr, state_names, state_before, func_name, findings)
        }
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            let state_after_condition =
                visit_expr_for_host_calls(cond, state_names, state_before, func_name, findings);
            let then_state = visit_expr_for_host_calls(
                then_expr,
                state_names,
                state_after_condition,
                func_name,
                findings,
            );
            let else_state = visit_expr_for_host_calls(
                else_expr,
                state_names,
                state_after_condition,
                func_name,
                findings,
            );
            then_state || else_state
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let state_after_condition = visit_expr_for_host_calls(
                condition,
                state_names,
                state_before,
                func_name,
                findings,
            );
            let then_state = analyze_block_reentrancy(
                then_branch,
                state_names,
                state_after_condition,
                func_name,
                findings,
            );
            let else_state = else_branch
                .as_ref()
                .map_or(state_after_condition, |branch| {
                    analyze_block_reentrancy(
                        branch,
                        state_names,
                        state_after_condition,
                        func_name,
                        findings,
                    )
                });
            then_state || else_state
        }
        Expr::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            let state_after_value =
                visit_expr_for_host_calls(value, state_names, state_before, func_name, findings);
            let then_state = analyze_block_reentrancy(
                then_branch,
                state_names,
                state_after_value,
                func_name,
                findings,
            );
            let else_state = else_branch.as_ref().map_or(state_after_value, |branch| {
                analyze_block_reentrancy(
                    branch,
                    state_names,
                    state_after_value,
                    func_name,
                    findings,
                )
            });
            then_state || else_state
        }
        Expr::Match { value, arms } => {
            let state_after_value =
                visit_expr_for_host_calls(value, state_names, state_before, func_name, findings);
            let mut aggregate_state = state_after_value;
            for arm in arms {
                let arm_state = analyze_block_reentrancy(
                    &arm.body,
                    state_names,
                    state_after_value,
                    func_name,
                    findings,
                );
                aggregate_state = aggregate_state || arm_state;
            }
            aggregate_state
        }
        Expr::Member { object, .. } => {
            visit_expr_for_host_calls(object, state_names, state_before, func_name, findings)
        }
        Expr::Index { target, index } => {
            let state_after_target =
                visit_expr_for_host_calls(target, state_names, state_before, func_name, findings);
            visit_expr_for_host_calls(index, state_names, state_after_target, func_name, findings)
        }
        Expr::Tuple(items) | Expr::List(items) => items.iter().fold(state_before, |state, item| {
            visit_expr_for_host_calls(item, state_names, state, func_name, findings)
        }),
        Expr::JsonObject(entries) => entries.iter().fold(state_before, |state, entry| {
            visit_expr_for_host_calls(&entry.value, state_names, state, func_name, findings)
        }),
        Expr::JsonArray(elements) => elements.iter().fold(state_before, |state, element| {
            visit_expr_for_host_calls(element, state_names, state, func_name, findings)
        }),
        Expr::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            let mut state =
                visit_expr_for_host_calls(source, state_names, state_before, func_name, findings);
            if let Some(condition) = condition {
                state =
                    visit_expr_for_host_calls(condition, state_names, state, func_name, findings);
            }
            visit_expr_for_host_calls(expression, state_names, state, func_name, findings)
        }
        Expr::StructLiteral { fields, .. } => fields.iter().fold(state_before, |state, field| {
            visit_expr_for_host_calls(&field.value, state_names, state, func_name, findings)
        }),
        Expr::Bool(_)
        | Expr::IntLiteral(_)
        | Expr::DecimalLiteral(_)
        | Expr::OptionNone
        | Expr::String(_)
        | Expr::Bytes(_)
        | Expr::Ident(_) => state_before,
    }
}
fn expr_targets_state(expr: &Expr, state_names: &HashSet<String>) -> bool {
    match expr {
        Expr::Source { expression, .. } | Expr::Resolved { expression, .. } => {
            expr_targets_state(expression, state_names)
        }
        Expr::Ident(name) => state_names.contains(name),
        Expr::Member { object, .. } | Expr::Index { target: object, .. } => {
            expr_targets_state(object, state_names)
        }
        Expr::Tuple(items) | Expr::List(items) => items
            .iter()
            .any(|item| expr_targets_state(item, state_names)),
        Expr::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            expr_targets_state(expression, state_names)
                || expr_targets_state(source, state_names)
                || condition
                    .as_deref()
                    .is_some_and(|condition| expr_targets_state(condition, state_names))
        }
        Expr::StructLiteral { fields, .. } => fields
            .iter()
            .any(|field| expr_targets_state(&field.value, state_names)),
        Expr::JsonObject(entries) => entries
            .iter()
            .any(|entry| expr_targets_state(&entry.value, state_names)),
        Expr::JsonArray(elements) => elements
            .iter()
            .any(|element| expr_targets_state(element, state_names)),
        Expr::Call { .. }
        | Expr::Binary { .. }
        | Expr::Unary { .. }
        | Expr::Conditional { .. }
        | Expr::If { .. }
        | Expr::IfLet { .. }
        | Expr::Match { .. }
        | Expr::OptionSome(_)
        | Expr::OptionNone
        | Expr::ResultOk(_)
        | Expr::ResultErr(_)
        | Expr::Propagate(_)
        | Expr::Bool(_)
        | Expr::IntLiteral(_)
        | Expr::DecimalLiteral(_)
        | Expr::String(_)
        | Expr::Bytes(_) => false,
    }
}
fn is_external_call(name: &str) -> bool {
    Builtin::from_source_name(name).is_some_and(|builtin| {
        let effects = builtin.effects();
        effects.host_side_effects || effects.emits_instructions
    })
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
        match stmt.kind() {
            Statement::Source { .. } | Statement::Resolved { .. } => {
                unreachable!("kind() strips provenance wrappers")
            }
            Statement::While { cond, body } => {
                if matches!(cond.kind(), Expr::Bool(true)) && !block_contains_escape(body) {
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
            Statement::IfLet {
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
        match stmt.kind() {
            Statement::Source { .. } | Statement::Resolved { .. } => {
                unreachable!("kind() strips provenance wrappers")
            }
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
            Statement::IfLet {
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
    if let Some(tail) = &block.tail {
        visitor(func_name, tail);
        visit_expr_children(tail, func_name, visitor);
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
        TypedStatement::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            visitor(func_name, value);
            visit_expr_children(value, func_name, visitor);
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
        ExprKind::Unary { expr, .. }
        | ExprKind::NumericCast { expr }
        | ExprKind::NumericTryCast { expr }
        | ExprKind::OptionSome { value: expr }
        | ExprKind::ResultOk { value: expr }
        | ExprKind::ResultErr { error: expr }
        | ExprKind::Propagate { value: expr } => {
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
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visitor(func_name, condition);
            visit_expr_children(condition, func_name, visitor);
            visit_block_exprs(then_branch, func_name, visitor);
            visit_block_exprs(else_branch, func_name, visitor);
        }
        ExprKind::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            visitor(func_name, value);
            visit_expr_children(value, func_name, visitor);
            visit_block_exprs(then_branch, func_name, visitor);
            visit_block_exprs(else_branch, func_name, visitor);
        }
        ExprKind::Match { value, arms } => {
            visitor(func_name, value);
            visit_expr_children(value, func_name, visitor);
            for arm in arms {
                visit_block_exprs(&arm.body, func_name, visitor);
            }
        }
        ExprKind::Call { args, .. } | ExprKind::NamedCall { args, .. } => {
            for arg in args {
                visitor(func_name, arg);
                visit_expr_children(arg, func_name, visitor);
            }
        }
        ExprKind::Tuple(items) | ExprKind::List(items) => {
            for item in items {
                visitor(func_name, item);
                visit_expr_children(item, func_name, visitor);
            }
        }
        ExprKind::JsonObject(entries) => {
            for (_, value) in entries {
                visitor(func_name, value);
                visit_expr_children(value, func_name, visitor);
            }
        }
        ExprKind::JsonArray(elements) => {
            for element in elements {
                visitor(func_name, element);
                visit_expr_children(element, func_name, visitor);
            }
        }
        ExprKind::ListComprehension {
            expression,
            source,
            condition,
            ..
        } => {
            visitor(func_name, source);
            visit_expr_children(source, func_name, visitor);
            visitor(func_name, expression);
            visit_expr_children(expression, func_name, visitor);
            if let Some(condition) = condition {
                visitor(func_name, condition);
                visit_expr_children(condition, func_name, visitor);
            }
        }
        ExprKind::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                visitor(func_name, value);
                visit_expr_children(value, func_name, visitor);
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
        ExprKind::IntLiteral(_)
        | ExprKind::DecimalLiteral { .. }
        | ExprKind::OptionNone
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Bytes(_)
        | ExprKind::Ident(_) => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_test_fragment as parse;
    fn analyze_static(source: &str) -> Vec<AnalysisFinding> {
        let program = parse(source).expect("parse");
        let typed = semantic::analyze(&program).expect("type check");
        run_static_analysis(&program, &typed)
    }
    #[test]
    fn values_above_i64_do_not_trigger_a_legacy_overflow_warning() {
        let findings = analyze_static(
            r#"
            fn wide() -> int {
                return 9223372036854775807 + 1;
            }
        "#,
        );
        assert!(
            findings
                .iter()
                .all(|finding| finding.code != "static-overflow-literal"),
            "valid signed-512 arithmetic received a legacy-width warning: {findings:?}"
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
    fn raw_contract_calls_are_rejected_before_static_analysis() {
        let program = parse(
            r#"
            state StateMap<int, int> balances;

            fn withdraw() {
                balances[1] = 0;
                host::call_contract("target", "entrypoint", Json::parse("{}"));
            }
        "#,
        );
        let program = program.expect("raw host call still has ordinary call syntax");
        let error = semantic::analyze(&program)
            .expect_err("raw host and contract-call capabilities are not V1 source APIs");
        assert!(
            error.message.contains("unknown function or builtin")
                || error.message.contains("compiler-internal"),
            "unexpected raw-call diagnostic: {error:?}"
        );
    }
    #[test]
    fn rejects_unbounded_loop_source() {
        let err = parse("fn spin() { while true {} }")
            .expect_err("canonical source must reject unbounded loops");
        assert!(err.contains("`while` is not supported"));
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
    fn ordinary_state_writes_do_not_create_reentrancy_findings() {
        let findings = analyze_static(
            r#"
            state StateMap<int, int> balances;

            fn withdraw() {
                balances[1] = balances.get(1).unwrap_or(0) - 1;
            }
        "#,
        );
        assert!(
            !findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "state-only code must not be labeled reentrant: {findings:?}"
        );
    }
    #[test]
    fn namespaced_pure_calls_do_not_create_reentrancy_findings() {
        let findings = analyze_static("fn hash() { crypto::sha256(b\"payload\"); }");
        assert!(
            !findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "unexpected reentrancy finding: {findings:?}"
        );
    }

    #[test]
    fn user_function_names_do_not_masquerade_as_external_calls() {
        let findings = analyze_static(
            r#"
            seiyaku Demo {
                state int counter;
                hajimari() { counter = 0; }
                fn transfer() {}
                fn run() {
                    counter = 1;
                    transfer();
                }
            }
        "#,
        );
        assert!(
            !findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "a pure user function named like a host operation was misclassified: {findings:?}"
        );
    }

    #[test]
    fn registered_host_effects_create_reentrancy_findings() {
        let findings = analyze_static(
            r#"
            seiyaku Demo {
                state int counter;
                hajimari() { counter = 0; }
                kotoage fn run() authorize("Run") {
                    counter = 1;
                    debug::info("state changed");
                }
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "a registered host effect was omitted from analysis: {findings:?}"
        );
    }

    #[test]
    fn expression_state_writes_are_visible_to_enclosing_host_calls() {
        let findings = analyze_static(
            r#"
            seiyaku Demo {
                state int counter;
                hajimari() { counter = 0; }
                kotoage fn run(bool update) authorize("Run") {
                    debug::info(if update { counter = 1; 1 } else { 0 });
                }
            }
        "#,
        );
        assert!(
            findings.iter().any(|f| f.code == "static-reentrancy-risk"),
            "a state write in an evaluated call argument was not propagated: {findings:?}"
        );
    }

    #[test]
    fn loop_carried_state_writes_are_visible_to_later_iterations() {
        let findings = analyze_static(
            r#"
            seiyaku Demo {
                state int counter;
                hajimari() { counter = 0; }
                kotoage fn run() authorize("Run") {
                    for index in range(2) {
                        debug::info("before write");
                        counter = index;
                    }
                }
            }
        "#,
        );
        let reentrancy_findings = findings
            .iter()
            .filter(|finding| finding.code == "static-reentrancy-risk")
            .count();
        assert_eq!(
            reentrancy_findings, 1,
            "the later iteration must be analyzed without duplicating the finding: {findings:?}"
        );
    }
}
