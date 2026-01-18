//! Kotodama source linter.
//!
//! Provides lightweight static analysis passes that flag common mistakes in
//! Kotodama programs before compiling them to IVM bytecode. The initial set of
//! checks focuses on surface issues such as unused `state` declarations and
//! obviously unreachable statements that follow a `return`.

use std::collections::{HashMap, HashSet};

use iroha_data_model::{
    isi::{
        BurnBox, ExecuteTrigger, GrantBox, InstructionBox, Log, MintBox, RegisterBox,
        RemoveKeyValueBox, RevokeBox, SetKeyValueBox, TransferBox, UnregisterBox,
    },
    query::{QueryRequest, SingularQueryBox},
};

use super::ast::{Block, Expr, Item, Pattern, Program, Statement};
use crate::kotodama::i18n::{self, Language, Message as I18nMessage, StateShadowContext};
use crate::pointer_abi::{self, PointerType};

/// A lint warning produced by [`lint_program`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintWarning {
    /// Stable identifier for the lint (e.g., `unused-state`).
    pub code: &'static str,
    /// Structured, localizable lint message data.
    pub message: LintMessage,
}

impl LintWarning {
    /// Render the lint message in the requested language.
    pub fn localized_message(&self, lang: Language) -> String {
        self.message.translate(lang)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintMessage {
    UnusedState {
        name: String,
    },
    StateShadowed {
        func: String,
        name: String,
        context: StateShadowContext,
    },
    UnusedParameter {
        func: String,
        name: String,
    },
    UnreachableAfterReturn {
        context: String,
    },
    Custom {
        message: String,
    },
}

impl LintMessage {
    fn translate(&self, lang: Language) -> String {
        match self {
            LintMessage::UnusedState { name } => i18n::translate(
                lang,
                I18nMessage::LintUnusedState {
                    name: name.as_str(),
                },
            ),
            LintMessage::StateShadowed {
                func,
                name,
                context,
            } => i18n::translate(
                lang,
                I18nMessage::LintStateShadowed {
                    func: func.as_str(),
                    name: name.as_str(),
                    context: *context,
                },
            ),
            LintMessage::UnusedParameter { func, name } => i18n::translate(
                lang,
                I18nMessage::LintUnusedParameter {
                    func: func.as_str(),
                    name: name.as_str(),
                },
            ),
            LintMessage::UnreachableAfterReturn { context } => i18n::translate(
                lang,
                I18nMessage::LintUnreachableAfterReturn {
                    context: context.as_str(),
                },
            ),
            LintMessage::Custom { message } => message.clone(),
        }
    }
}

/// Run the Kotodama lint suite against an AST [`Program`].
pub fn lint_program(program: &Program) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    lint_unused_state(program, &mut warnings);
    lint_state_shadowing(program, &mut warnings);
    lint_unused_parameters(program, &mut warnings);
    lint_unreachable_after_return(program, &mut warnings);
    lint_pointer_constructor_usage(program, &mut warnings);
    lint_nonliteral_trigger_specs(program, &mut warnings);
    lint_nonliteral_state_map_keys(program, &mut warnings);
    lint_nonliteral_state_paths(program, &mut warnings);
    lint_opaque_access_hints(program, &mut warnings);
    warnings
}

const OPAQUE_ACCESS_HINT_CALLS: &[&str] = &[
    "register_asset",
    "create_new_asset",
    "transfer_domain",
    "register_peer",
    "unregister_peer",
    "nft_set_metadata",
    "sc_execute_submit_ballot",
    "sc_execute_unshield",
    "subscription_bill",
    "subscription_record_usage",
    "build_submit_ballot_inline",
    "build_unshield_inline",
    "axt_begin",
    "axt_touch",
    "verify_ds_proof",
    "use_asset_handle",
    "axt_commit",
];

const EXECUTE_INSTRUCTION_CALL: &str = "execute_instruction";
const EXECUTE_QUERY_CALL: &str = "execute_query";

fn decode_hex_or_raw_bytes(raw: &str) -> Option<Vec<u8>> {
    if let Some(trimmed) = raw.strip_prefix("0x") {
        if trimmed.len() % 2 == 0 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut out = Vec::with_capacity(trimmed.len() / 2);
            for chunk in trimmed.as_bytes().chunks(2) {
                let byte_str = std::str::from_utf8(chunk).ok()?;
                let byte = u8::from_str_radix(byte_str, 16).ok()?;
                out.push(byte);
            }
            return Some(out);
        }
        return None;
    }
    Some(raw.as_bytes().to_vec())
}

fn decode_norito_bytes_literal(expr: &Expr) -> Option<Vec<u8>> {
    let Expr::Call { name, args } = expr else {
        return None;
    };
    if name != "norito_bytes" || args.len() != 1 {
        return None;
    }
    let raw = match &args[0] {
        Expr::String(value) => decode_hex_or_raw_bytes(value)?,
        Expr::Bytes(value) => value.clone(),
        _ => return None,
    };
    let payload = match pointer_abi::validate_tlv_bytes(&raw) {
        Ok(tlv) => {
            if tlv.type_id != PointerType::NoritoBytes {
                return None;
            }
            tlv.payload.to_vec()
        }
        Err(_) => raw,
    };
    Some(payload)
}

fn decode_instruction_box_literal(args: &[Expr]) -> Option<InstructionBox> {
    let payload = decode_norito_bytes_literal(args.first()?)?;
    norito::decode_from_bytes(&payload).ok()
}

fn decode_query_request_literal(args: &[Expr]) -> Option<QueryRequest> {
    let payload = decode_norito_bytes_literal(args.first()?)?;
    norito::decode_from_bytes(&payload).ok()
}

fn instruction_box_is_hintable(instr: &InstructionBox) -> bool {
    let any = instr.as_any();

    if any.downcast_ref::<Log>().is_some() {
        return true;
    }
    if any.downcast_ref::<TransferBox>().is_some() {
        return true;
    }
    if any.downcast_ref::<MintBox>().is_some() {
        return true;
    }
    if any.downcast_ref::<BurnBox>().is_some() {
        return true;
    }
    if any.downcast_ref::<SetKeyValueBox>().is_some() {
        return true;
    }
    if any.downcast_ref::<RemoveKeyValueBox>().is_some() {
        return true;
    }
    if let Some(rb) = any.downcast_ref::<RegisterBox>() {
        return !matches!(rb, RegisterBox::Peer(_));
    }
    if let Some(ub) = any.downcast_ref::<UnregisterBox>() {
        return !matches!(ub, UnregisterBox::Peer(_));
    }
    if any.downcast_ref::<GrantBox>().is_some() {
        return true;
    }
    if any.downcast_ref::<RevokeBox>().is_some() {
        return true;
    }
    if any.downcast_ref::<ExecuteTrigger>().is_some() {
        return true;
    }

    false
}

fn query_request_is_hintable(request: &QueryRequest) -> bool {
    match request {
        QueryRequest::Singular(query) => matches!(query, SingularQueryBox::FindAssetById(_)),
        QueryRequest::Start(_) | QueryRequest::Continue(_) => false,
    }
}

fn lint_nonliteral_state_map_keys(program: &Program, warnings: &mut Vec<LintWarning>) {
    let mut state_maps = HashSet::new();
    for item in &program.items {
        if let Item::State(state) = item
            && matches!(state.ty, super::ast::TypeExpr::Generic { ref base, .. } if base == "Map")
        {
            state_maps.insert(state.name.clone());
        }
    }
    if state_maps.is_empty() {
        return;
    }
    for item in &program.items {
        if let Item::Function(func) = item {
            lint_block_map_keys(&func.body, &state_maps, warnings);
        }
    }
}

fn lint_nonliteral_state_paths(program: &Program, warnings: &mut Vec<LintWarning>) {
    for item in &program.items {
        if let Item::Function(func) = item {
            lint_state_path_block(&func.body, warnings);
        }
    }
}

fn lint_state_path_block(block: &Block, warnings: &mut Vec<LintWarning>) {
    for stmt in &block.statements {
        lint_state_path_stmt(stmt, warnings);
    }
}

fn lint_state_path_stmt(stmt: &Statement, warnings: &mut Vec<LintWarning>) {
    match stmt {
        Statement::Let { value, .. } => lint_state_path_expr(value, warnings),
        Statement::Assign { value, .. } => lint_state_path_expr(value, warnings),
        Statement::AssignExpr { target, value, .. } => {
            lint_state_path_expr(target, warnings);
            lint_state_path_expr(value, warnings);
        }
        Statement::Expr(expr) => lint_state_path_expr(expr, warnings),
        Statement::Return(Some(expr)) => lint_state_path_expr(expr, warnings),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            lint_state_path_expr(cond, warnings);
            lint_state_path_block(then_branch, warnings);
            if let Some(b) = else_branch {
                lint_state_path_block(b, warnings);
            }
        }
        Statement::While { cond, body } => {
            lint_state_path_expr(cond, warnings);
            lint_state_path_block(body, warnings);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init {
                lint_state_path_stmt(init_stmt, warnings);
            }
            if let Some(cond_expr) = cond {
                lint_state_path_expr(cond_expr, warnings);
            }
            if let Some(step_stmt) = step {
                lint_state_path_stmt(step_stmt, warnings);
            }
            lint_state_path_block(body, warnings);
        }
        Statement::ForEachMap { map, body, .. } => {
            lint_state_path_expr(map, warnings);
            lint_state_path_block(body, warnings);
        }
    }
}

fn lint_state_path_expr(expr: &Expr, warnings: &mut Vec<LintWarning>) {
    match expr {
        Expr::Call { name, args } => {
            if matches!(name.as_str(), "state_get" | "state_set" | "state_del") {
                if let Some(path) = args.first() {
                    if !is_literal_state_path(path) {
                        warnings.push(LintWarning {
                            code: "nonliteral-state-path",
                            message: LintMessage::Custom {
                                message: format!(
                                    "{name} uses a non-literal path; access hints will be skipped"
                                ),
                            },
                        });
                    }
                }
            }
            for arg in args {
                lint_state_path_expr(arg, warnings);
            }
        }
        Expr::Binary { left, right, .. } => {
            lint_state_path_expr(left, warnings);
            lint_state_path_expr(right, warnings);
        }
        Expr::Unary { expr, .. } => lint_state_path_expr(expr, warnings),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            lint_state_path_expr(cond, warnings);
            lint_state_path_expr(then_expr, warnings);
            lint_state_path_expr(else_expr, warnings);
        }
        Expr::Tuple(items) => {
            for item in items {
                lint_state_path_expr(item, warnings);
            }
        }
        Expr::Member { object, .. } => lint_state_path_expr(object, warnings),
        Expr::Index { target, index } => {
            lint_state_path_expr(target, warnings);
            lint_state_path_expr(index, warnings);
        }
        Expr::Number(_) | Expr::Bool(_) | Expr::String(_) | Expr::Bytes(_) | Expr::Ident(_) => {}
    }
}

fn lint_opaque_access_hints(program: &Program, warnings: &mut Vec<LintWarning>) {
    for item in &program.items {
        if let Item::Function(func) = item {
            lint_opaque_access_block(&func.body, warnings);
        }
    }
}

fn lint_opaque_access_block(block: &Block, warnings: &mut Vec<LintWarning>) {
    for stmt in &block.statements {
        lint_opaque_access_stmt(stmt, warnings);
    }
}

fn lint_opaque_access_stmt(stmt: &Statement, warnings: &mut Vec<LintWarning>) {
    match stmt {
        Statement::Let { value, .. } => lint_opaque_access_expr(value, warnings),
        Statement::Assign { value, .. } => lint_opaque_access_expr(value, warnings),
        Statement::AssignExpr { target, value, .. } => {
            lint_opaque_access_expr(target, warnings);
            lint_opaque_access_expr(value, warnings);
        }
        Statement::Expr(expr) => lint_opaque_access_expr(expr, warnings),
        Statement::Return(Some(expr)) => lint_opaque_access_expr(expr, warnings),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            lint_opaque_access_expr(cond, warnings);
            lint_opaque_access_block(then_branch, warnings);
            if let Some(b) = else_branch {
                lint_opaque_access_block(b, warnings);
            }
        }
        Statement::While { cond, body } => {
            lint_opaque_access_expr(cond, warnings);
            lint_opaque_access_block(body, warnings);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init {
                lint_opaque_access_stmt(init_stmt, warnings);
            }
            if let Some(cond_expr) = cond {
                lint_opaque_access_expr(cond_expr, warnings);
            }
            if let Some(step_stmt) = step {
                lint_opaque_access_stmt(step_stmt, warnings);
            }
            lint_opaque_access_block(body, warnings);
        }
        Statement::ForEachMap { map, body, .. } => {
            lint_opaque_access_expr(map, warnings);
            lint_opaque_access_block(body, warnings);
        }
    }
}

fn lint_opaque_access_expr(expr: &Expr, warnings: &mut Vec<LintWarning>) {
    match expr {
        Expr::Call { name, args } => {
            let warn = if name == EXECUTE_INSTRUCTION_CALL {
                !decode_instruction_box_literal(args)
                    .map(|isi| instruction_box_is_hintable(&isi))
                    .unwrap_or(false)
            } else if name == EXECUTE_QUERY_CALL {
                !decode_query_request_literal(args)
                    .map(|query| query_request_is_hintable(&query))
                    .unwrap_or(false)
            } else {
                OPAQUE_ACCESS_HINT_CALLS.contains(&name.as_str())
            };
            if warn {
                warnings.push(LintWarning {
                    code: "opaque-access-hints",
                    message: LintMessage::Custom {
                        message: format!(
                            "call to `{name}` uses opaque host access; access hints will be skipped"
                        ),
                    },
                });
            }
            for arg in args {
                lint_opaque_access_expr(arg, warnings);
            }
        }
        Expr::Binary { left, right, .. } => {
            lint_opaque_access_expr(left, warnings);
            lint_opaque_access_expr(right, warnings);
        }
        Expr::Unary { expr, .. } => lint_opaque_access_expr(expr, warnings),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            lint_opaque_access_expr(cond, warnings);
            lint_opaque_access_expr(then_expr, warnings);
            lint_opaque_access_expr(else_expr, warnings);
        }
        Expr::Tuple(items) => {
            for item in items {
                lint_opaque_access_expr(item, warnings);
            }
        }
        Expr::Member { object, .. } => lint_opaque_access_expr(object, warnings),
        Expr::Index { target, index } => {
            lint_opaque_access_expr(target, warnings);
            lint_opaque_access_expr(index, warnings);
        }
        Expr::Number(_) | Expr::Bool(_) | Expr::String(_) | Expr::Bytes(_) | Expr::Ident(_) => {}
    }
}

fn lint_block_map_keys(
    block: &Block,
    state_maps: &HashSet<String>,
    warnings: &mut Vec<LintWarning>,
) {
    for stmt in &block.statements {
        lint_stmt_map_keys(stmt, state_maps, warnings);
    }
}

fn lint_stmt_map_keys(
    stmt: &Statement,
    state_maps: &HashSet<String>,
    warnings: &mut Vec<LintWarning>,
) {
    match stmt {
        Statement::Let { value, .. } => lint_expr_map_keys(value, state_maps, warnings),
        Statement::Assign { value, .. } => lint_expr_map_keys(value, state_maps, warnings),
        Statement::AssignExpr { target, value, .. } => {
            lint_expr_map_keys(target, state_maps, warnings);
            lint_expr_map_keys(value, state_maps, warnings);
        }
        Statement::Expr(expr) => lint_expr_map_keys(expr, state_maps, warnings),
        Statement::Return(Some(expr)) => lint_expr_map_keys(expr, state_maps, warnings),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            lint_expr_map_keys(cond, state_maps, warnings);
            lint_block_map_keys(then_branch, state_maps, warnings);
            if let Some(else_block) = else_branch {
                lint_block_map_keys(else_block, state_maps, warnings);
            }
        }
        Statement::While { cond, body } => {
            lint_expr_map_keys(cond, state_maps, warnings);
            lint_block_map_keys(body, state_maps, warnings);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init {
                lint_stmt_map_keys(init_stmt, state_maps, warnings);
            }
            if let Some(cond_expr) = cond {
                lint_expr_map_keys(cond_expr, state_maps, warnings);
            }
            if let Some(step_stmt) = step {
                lint_stmt_map_keys(step_stmt, state_maps, warnings);
            }
            lint_block_map_keys(body, state_maps, warnings);
        }
        Statement::ForEachMap { map, body, .. } => {
            lint_expr_map_keys(map, state_maps, warnings);
            lint_block_map_keys(body, state_maps, warnings);
        }
    }
}

fn lint_expr_map_keys(expr: &Expr, state_maps: &HashSet<String>, warnings: &mut Vec<LintWarning>) {
    match expr {
        Expr::Index { target, index } => {
            if let Expr::Ident(name) = &**target
                && state_maps.contains(name)
                && !is_literal_state_key(index)
            {
                warnings.push(LintWarning {
                    code: "nonliteral-state-map-key",
                    message: LintMessage::Custom {
                        message: format!(
                            "state map `{name}` uses a non-literal key; access hints will be skipped"
                        ),
                    },
                });
            }
            lint_expr_map_keys(target, state_maps, warnings);
            lint_expr_map_keys(index, state_maps, warnings);
        }
        Expr::Binary { left, right, .. } => {
            lint_expr_map_keys(left, state_maps, warnings);
            lint_expr_map_keys(right, state_maps, warnings);
        }
        Expr::Unary { expr, .. } => lint_expr_map_keys(expr, state_maps, warnings),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            lint_expr_map_keys(cond, state_maps, warnings);
            lint_expr_map_keys(then_expr, state_maps, warnings);
            lint_expr_map_keys(else_expr, state_maps, warnings);
        }
        Expr::Call { args, .. } | Expr::Tuple(args) => {
            for arg in args {
                lint_expr_map_keys(arg, state_maps, warnings);
            }
        }
        Expr::Member { object, .. } => lint_expr_map_keys(object, state_maps, warnings),
        Expr::Number(_) | Expr::Bool(_) | Expr::String(_) | Expr::Bytes(_) | Expr::Ident(_) => {}
    }
}

fn is_literal_state_key(expr: &Expr) -> bool {
    match expr {
        Expr::Number(_) | Expr::String(_) | Expr::Bytes(_) => true,
        Expr::Call { name, args } => {
            let literal_arg = matches!(args.first(), Some(Expr::String(_)) | Some(Expr::Bytes(_)));
            if !literal_arg || args.len() != 1 {
                return false;
            }
            matches!(
                name.as_str(),
                "account_id"
                    | "account"
                    | "asset_definition"
                    | "asset_id"
                    | "nft_id"
                    | "domain"
                    | "domain_id"
                    | "name"
                    | "json"
                    | "blob"
                    | "norito_bytes"
                    | "dataspace_id"
                    | "axt_descriptor"
                    | "asset_handle"
                    | "proof_blob"
            )
        }
        _ => false,
    }
}

fn is_literal_state_path(expr: &Expr) -> bool {
    match expr {
        Expr::String(_) | Expr::Bytes(_) => true,
        Expr::Call { name, args } => match name.as_str() {
            "name" => {
                args.len() == 1
                    && matches!(args.first(), Some(Expr::String(_)) | Some(Expr::Bytes(_)))
            }
            "path_map_key" => {
                if args.len() != 2 {
                    return false;
                }
                is_literal_state_path(&args[0]) && matches!(args[1], Expr::Number(_))
            }
            "path_map_key_norito" => {
                if args.len() != 2 {
                    return false;
                }
                is_literal_state_path(&args[0]) && is_literal_state_key(&args[1])
            }
            _ => false,
        },
        _ => false,
    }
}

fn lint_unused_state(program: &Program, warnings: &mut Vec<LintWarning>) {
    let state_names: Vec<String> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::State(state) => Some(state.name.clone()),
            _ => None,
        })
        .collect();
    if state_names.is_empty() {
        return;
    }
    let state_lookup: HashSet<String> = state_names.iter().cloned().collect();
    let mut used: HashSet<String> = HashSet::new();
    let mut stmt_stack: Vec<&Statement> = Vec::new();
    for item in &program.items {
        if let Item::Function(func) = item {
            for stmt in func.body.statements.iter().rev() {
                stmt_stack.push(stmt);
            }
        }
    }
    while let Some(stmt) = stmt_stack.pop() {
        match stmt {
            Statement::Let { value, .. } => {
                record_expr_idents(value, &state_lookup, &mut used);
            }
            Statement::Assign { name, value } => {
                if state_lookup.contains(name) {
                    used.insert(name.clone());
                }
                record_expr_idents(value, &state_lookup, &mut used);
            }
            Statement::AssignExpr { target, value, .. } => {
                record_expr_idents(target, &state_lookup, &mut used);
                record_expr_idents(value, &state_lookup, &mut used);
            }
            Statement::Expr(expr) => {
                record_expr_idents(expr, &state_lookup, &mut used);
            }
            Statement::Return(Some(expr)) => {
                record_expr_idents(expr, &state_lookup, &mut used);
            }
            Statement::Return(None) | Statement::Break | Statement::Continue => {}
            Statement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                record_expr_idents(cond, &state_lookup, &mut used);
                for stmt in then_branch.statements.iter().rev() {
                    stmt_stack.push(stmt);
                }
                if let Some(else_block) = else_branch {
                    for stmt in else_block.statements.iter().rev() {
                        stmt_stack.push(stmt);
                    }
                }
            }
            Statement::While { cond, body } => {
                record_expr_idents(cond, &state_lookup, &mut used);
                for stmt in body.statements.iter().rev() {
                    stmt_stack.push(stmt);
                }
            }
            Statement::For {
                line: _,
                init,
                cond,
                step,
                body,
            } => {
                if let Some(init_stmt) = init {
                    stmt_stack.push(&**init_stmt);
                }
                if let Some(cond_expr) = cond {
                    record_expr_idents(cond_expr, &state_lookup, &mut used);
                }
                if let Some(step_stmt) = step {
                    stmt_stack.push(&**step_stmt);
                }
                for stmt in body.statements.iter().rev() {
                    stmt_stack.push(stmt);
                }
            }
            Statement::ForEachMap { map, body, .. } => {
                record_expr_idents(map, &state_lookup, &mut used);
                for stmt in body.statements.iter().rev() {
                    stmt_stack.push(stmt);
                }
            }
        }
    }
    for name in state_names {
        if !used.contains(&name) {
            warnings.push(LintWarning {
                code: "unused-state",
                message: LintMessage::UnusedState { name },
            });
        }
    }
}

fn lint_state_shadowing(program: &Program, warnings: &mut Vec<LintWarning>) {
    let state_names: HashSet<String> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::State(state) => Some(state.name.clone()),
            _ => None,
        })
        .collect();
    if state_names.is_empty() {
        return;
    }

    for item in &program.items {
        if let Item::Function(func) = item {
            for param in &func.params {
                let name = &param.name;
                if state_names.contains(name) && !name.starts_with('_') {
                    warnings.push(LintWarning {
                        code: "state-shadowed",
                        message: LintMessage::StateShadowed {
                            func: func.name.clone(),
                            name: name.clone(),
                            context: StateShadowContext::Parameter,
                        },
                    });
                }
            }
            lint_statement_shadowing_block(&func.body, &state_names, warnings, &func.name);
        }
    }
}

fn lint_statement_shadowing_block(
    block: &Block,
    state_names: &HashSet<String>,
    warnings: &mut Vec<LintWarning>,
    func_name: &str,
) {
    for stmt in &block.statements {
        lint_statement_state_shadowing(stmt, state_names, warnings, func_name);
    }
}

fn lint_statement_state_shadowing(
    stmt: &Statement,
    state_names: &HashSet<String>,
    warnings: &mut Vec<LintWarning>,
    func_name: &str,
) {
    match stmt {
        Statement::Let { pat, .. } => {
            let mut bound_names = Vec::new();
            collect_pattern_names(pat, &mut bound_names);
            for name in bound_names {
                if state_names.contains(name) && !name.starts_with('_') {
                    warnings.push(LintWarning {
                        code: "state-shadowed",
                        message: LintMessage::StateShadowed {
                            func: func_name.to_owned(),
                            name: name.to_owned(),
                            context: StateShadowContext::Binding,
                        },
                    });
                }
            }
        }
        Statement::Assign { .. }
        | Statement::AssignExpr { .. }
        | Statement::Expr(_)
        | Statement::Return(_)
        | Statement::Break
        | Statement::Continue => {}
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            lint_statement_shadowing_block(then_branch, state_names, warnings, func_name);
            if let Some(else_block) = else_branch {
                lint_statement_shadowing_block(else_block, state_names, warnings, func_name);
            }
        }
        Statement::While { body, .. } => {
            lint_statement_shadowing_block(body, state_names, warnings, func_name);
        }
        Statement::For {
            init, step, body, ..
        } => {
            if let Some(init_stmt) = init.as_deref() {
                lint_statement_state_shadowing(init_stmt, state_names, warnings, func_name);
            }
            if let Some(step_stmt) = step.as_deref() {
                lint_statement_state_shadowing(step_stmt, state_names, warnings, func_name);
            }
            lint_statement_shadowing_block(body, state_names, warnings, func_name);
        }
        Statement::ForEachMap {
            key, value, body, ..
        } => {
            if state_names.contains(key) && !key.starts_with('_') {
                warnings.push(LintWarning {
                    code: "state-shadowed",
                    message: LintMessage::StateShadowed {
                        func: func_name.to_owned(),
                        name: key.clone(),
                        context: StateShadowContext::MapBinding,
                    },
                });
            }
            if let Some(value_name) = value
                && state_names.contains(value_name)
                && !value_name.starts_with('_')
            {
                warnings.push(LintWarning {
                    code: "state-shadowed",
                    message: LintMessage::StateShadowed {
                        func: func_name.to_owned(),
                        name: value_name.clone(),
                        context: StateShadowContext::MapBinding,
                    },
                });
            }
            lint_statement_shadowing_block(body, state_names, warnings, func_name);
        }
    }
}

fn lint_unused_parameters(program: &Program, warnings: &mut Vec<LintWarning>) {
    for item in &program.items {
        if let Item::Function(func) = item {
            let param_names: Vec<String> = func
                .params
                .iter()
                .map(|param| param.name.clone())
                .filter(|name| !name.starts_with('_'))
                .collect();
            if param_names.is_empty() {
                continue;
            }
            let lookup: HashSet<String> = param_names.iter().cloned().collect();
            let mut used: HashSet<String> = HashSet::new();
            let mut stmt_stack: Vec<&Statement> = Vec::new();
            for stmt in func.body.statements.iter().rev() {
                stmt_stack.push(stmt);
            }
            while let Some(stmt) = stmt_stack.pop() {
                match stmt {
                    Statement::Let { value, .. } => {
                        record_expr_idents(value, &lookup, &mut used);
                    }
                    Statement::Assign { name, value } => {
                        if lookup.contains(name) {
                            used.insert(name.clone());
                        }
                        record_expr_idents(value, &lookup, &mut used);
                    }
                    Statement::AssignExpr { target, value, .. } => {
                        record_expr_idents(target, &lookup, &mut used);
                        record_expr_idents(value, &lookup, &mut used);
                    }
                    Statement::Expr(expr) => {
                        record_expr_idents(expr, &lookup, &mut used);
                    }
                    Statement::Return(Some(expr)) => {
                        record_expr_idents(expr, &lookup, &mut used);
                    }
                    Statement::Return(None) | Statement::Break | Statement::Continue => {}
                    Statement::If {
                        cond,
                        then_branch,
                        else_branch,
                    } => {
                        record_expr_idents(cond, &lookup, &mut used);
                        for stmt in then_branch.statements.iter().rev() {
                            stmt_stack.push(stmt);
                        }
                        if let Some(else_block) = else_branch {
                            for stmt in else_block.statements.iter().rev() {
                                stmt_stack.push(stmt);
                            }
                        }
                    }
                    Statement::While { cond, body } => {
                        record_expr_idents(cond, &lookup, &mut used);
                        for stmt in body.statements.iter().rev() {
                            stmt_stack.push(stmt);
                        }
                    }
                    Statement::For {
                        init,
                        cond,
                        step,
                        body,
                        ..
                    } => {
                        if let Some(init_stmt) = init {
                            stmt_stack.push(&**init_stmt);
                        }
                        if let Some(cond_expr) = cond {
                            record_expr_idents(cond_expr, &lookup, &mut used);
                        }
                        if let Some(step_stmt) = step {
                            stmt_stack.push(&**step_stmt);
                        }
                        for stmt in body.statements.iter().rev() {
                            stmt_stack.push(stmt);
                        }
                    }
                    Statement::ForEachMap { map, body, .. } => {
                        record_expr_idents(map, &lookup, &mut used);
                        for stmt in body.statements.iter().rev() {
                            stmt_stack.push(stmt);
                        }
                    }
                }
            }
            for name in param_names {
                if !used.contains(&name) {
                    warnings.push(LintWarning {
                        code: "unused-parameter",
                        message: LintMessage::UnusedParameter {
                            func: func.name.clone(),
                            name,
                        },
                    });
                }
            }
        }
    }
}

fn lint_unreachable_after_return(program: &Program, warnings: &mut Vec<LintWarning>) {
    for item in &program.items {
        if let Item::Function(func) = item {
            let mut stack: Vec<(&Block, String)> =
                vec![(&func.body, format!("function `{}`", func.name))];
            while let Some((block, context)) = stack.pop() {
                let mut saw_return = false;
                for stmt in &block.statements {
                    if saw_return {
                        warnings.push(LintWarning {
                            code: "unreachable-return",
                            message: LintMessage::UnreachableAfterReturn {
                                context: context.clone(),
                            },
                        });
                        break;
                    }
                    match stmt {
                        Statement::Return(_) => {
                            saw_return = true;
                        }
                        Statement::If {
                            then_branch,
                            else_branch,
                            ..
                        } => {
                            stack.push((then_branch, format!("{context} then-branch")));
                            if let Some(else_block) = else_branch {
                                stack.push((else_block, format!("{context} else-branch")));
                            }
                        }
                        Statement::While { body, .. } => {
                            stack.push((body, format!("{context} while-body")));
                        }
                        Statement::For { body, .. } => {
                            stack.push((body, format!("{context} for-body")));
                        }
                        Statement::ForEachMap { body, .. } => {
                            stack.push((body, format!("{context} foreach-body")));
                        }
                        Statement::Let { .. }
                        | Statement::Assign { .. }
                        | Statement::AssignExpr { .. }
                        | Statement::Expr(_)
                        | Statement::Break
                        | Statement::Continue => {}
                    }
                }
            }
        }
    }
}

fn collect_pattern_names<'a>(pattern: &'a Pattern, out: &mut Vec<&'a str>) {
    match pattern {
        Pattern::Name(name) => out.push(name.as_str()),
        Pattern::Tuple(names) => {
            for name in names {
                out.push(name.as_str());
            }
        }
    }
}

fn record_expr_idents(expr: &Expr, state_lookup: &HashSet<String>, hits: &mut HashSet<String>) {
    let mut stack = vec![expr];
    while let Some(e) = stack.pop() {
        match e {
            Expr::Ident(name) => {
                if state_lookup.contains(name) {
                    hits.insert(name.clone());
                }
            }
            Expr::Binary { left, right, .. } => {
                stack.push(left);
                stack.push(right);
            }
            Expr::Unary { expr, .. } => {
                stack.push(expr);
            }
            Expr::Conditional {
                cond,
                then_expr,
                else_expr,
            } => {
                stack.push(cond);
                stack.push(then_expr);
                stack.push(else_expr);
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    stack.push(arg);
                }
            }
            Expr::Tuple(values) => {
                for elem in values {
                    stack.push(elem);
                }
            }
            Expr::Member { object, .. } => {
                stack.push(object);
            }
            Expr::Index { target, index } => {
                stack.push(target);
                stack.push(index);
            }
            Expr::Bool(_) | Expr::Number(_) | Expr::String(_) | Expr::Bytes(_) => {}
        }
    }
}

const POINTER_CONSTRUCTORS: &[&str] = &[
    "account_id",
    "asset_definition",
    "asset_id",
    "nft_id",
    "domain",
    "domain_id",
    "name",
    "json",
    "blob",
    "norito_bytes",
];

fn lint_pointer_constructor_usage(program: &Program, warnings: &mut Vec<LintWarning>) {
    let constructors: HashSet<&str> = POINTER_CONSTRUCTORS.iter().copied().collect();
    let mut literal_counts: HashMap<String, usize> = HashMap::new();

    for item in &program.items {
        if let Item::Function(func) = item {
            collect_pointer_literals_from_block(&func.body, &constructors, &mut literal_counts);
            lint_unused_pointer_constructor_block(&func.body, &constructors, &func.name, warnings);
        }
    }

    for (literal, count) in literal_counts {
        if count > 1 {
            warnings.push(LintWarning {
                code: "duplicate-pointer-literal",
                message: LintMessage::Custom {
                    message: format!(
                        "literal `{literal}` appears multiple times in pointer constructors; bind it once (for example, `let id = account!(\"{literal}\");`) and reuse the binding"
                    ),
                },
            });
        }
    }
}

fn lint_nonliteral_trigger_specs(program: &Program, warnings: &mut Vec<LintWarning>) {
    for item in &program.items {
        if let Item::Function(func) = item {
            lint_trigger_specs_in_block(&func.body, &func.name, warnings);
        }
    }
}

fn lint_trigger_specs_in_block(block: &Block, func_name: &str, warnings: &mut Vec<LintWarning>) {
    for stmt in &block.statements {
        lint_trigger_specs_in_stmt(stmt, func_name, warnings);
    }
}

fn lint_trigger_specs_in_stmt(stmt: &Statement, func_name: &str, warnings: &mut Vec<LintWarning>) {
    match stmt {
        Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expr(value)
        | Statement::Return(Some(value)) => {
            lint_trigger_specs_in_expr(value, func_name, warnings);
        }
        Statement::AssignExpr { target, value, .. } => {
            lint_trigger_specs_in_expr(target, func_name, warnings);
            lint_trigger_specs_in_expr(value, func_name, warnings);
        }
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            lint_trigger_specs_in_expr(cond, func_name, warnings);
            lint_trigger_specs_in_block(then_branch, func_name, warnings);
            if let Some(else_block) = else_branch {
                lint_trigger_specs_in_block(else_block, func_name, warnings);
            }
        }
        Statement::While { cond, body } => {
            lint_trigger_specs_in_expr(cond, func_name, warnings);
            lint_trigger_specs_in_block(body, func_name, warnings);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init {
                lint_trigger_specs_in_stmt(init_stmt, func_name, warnings);
            }
            if let Some(cond_expr) = cond {
                lint_trigger_specs_in_expr(cond_expr, func_name, warnings);
            }
            if let Some(step_stmt) = step {
                lint_trigger_specs_in_stmt(step_stmt, func_name, warnings);
            }
            lint_trigger_specs_in_block(body, func_name, warnings);
        }
        Statement::ForEachMap { map, body, .. } => {
            lint_trigger_specs_in_expr(map, func_name, warnings);
            lint_trigger_specs_in_block(body, func_name, warnings);
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn lint_trigger_specs_in_expr(expr: &Expr, func_name: &str, warnings: &mut Vec<LintWarning>) {
    match expr {
        Expr::Call { name, args } => {
            if matches!(name.as_str(), "create_trigger" | "register_trigger") {
                let literal = args.first().map_or(false, is_literal_trigger_spec);
                if !literal {
                    warnings.push(LintWarning {
                        code: "nonliteral-trigger-spec",
                        message: LintMessage::Custom {
                            message: format!(
                                "trigger spec in `{func_name}` is non-literal; access hints may be skipped (use json!(...) or json(\"...\") for literals)"
                            ),
                        },
                    });
                }
            }
            for arg in args {
                lint_trigger_specs_in_expr(arg, func_name, warnings);
            }
        }
        Expr::Binary { left, right, .. } => {
            lint_trigger_specs_in_expr(left, func_name, warnings);
            lint_trigger_specs_in_expr(right, func_name, warnings);
        }
        Expr::Unary { expr, .. } => lint_trigger_specs_in_expr(expr, func_name, warnings),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            lint_trigger_specs_in_expr(cond, func_name, warnings);
            lint_trigger_specs_in_expr(then_expr, func_name, warnings);
            lint_trigger_specs_in_expr(else_expr, func_name, warnings);
        }
        Expr::Tuple(values) => {
            for value in values {
                lint_trigger_specs_in_expr(value, func_name, warnings);
            }
        }
        Expr::Member { object, .. } => lint_trigger_specs_in_expr(object, func_name, warnings),
        Expr::Index { target, index } => {
            lint_trigger_specs_in_expr(target, func_name, warnings);
            lint_trigger_specs_in_expr(index, func_name, warnings);
        }
        Expr::Bool(_) | Expr::Number(_) | Expr::String(_) | Expr::Bytes(_) | Expr::Ident(_) => {}
    }
}

fn is_literal_trigger_spec(expr: &Expr) -> bool {
    match expr {
        Expr::Call { name, args } if name == "json" => {
            matches!(args.first(), Some(Expr::String(_)))
        }
        _ => false,
    }
}

fn collect_pointer_literals_from_stmt(
    stmt: &Statement,
    constructors: &HashSet<&str>,
    counts: &mut HashMap<String, usize>,
) {
    match stmt {
        Statement::Let { value, .. } => {
            collect_pointer_literals_from_expr(value, constructors, counts);
        }
        Statement::Assign { value, .. } => {
            collect_pointer_literals_from_expr(value, constructors, counts);
        }
        Statement::AssignExpr { target, value, .. } => {
            collect_pointer_literals_from_expr(target, constructors, counts);
            collect_pointer_literals_from_expr(value, constructors, counts);
        }
        Statement::Expr(expr) => {
            collect_pointer_literals_from_expr(expr, constructors, counts);
        }
        Statement::Return(Some(expr)) => {
            collect_pointer_literals_from_expr(expr, constructors, counts);
        }
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_pointer_literals_from_expr(cond, constructors, counts);
            collect_pointer_literals_from_block(then_branch, constructors, counts);
            if let Some(else_block) = else_branch {
                collect_pointer_literals_from_block(else_block, constructors, counts);
            }
        }
        Statement::While { cond, body } => {
            collect_pointer_literals_from_expr(cond, constructors, counts);
            collect_pointer_literals_from_block(body, constructors, counts);
        }
        Statement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init {
                collect_pointer_literals_from_stmt(init_stmt, constructors, counts);
            }
            if let Some(cond_expr) = cond {
                collect_pointer_literals_from_expr(cond_expr, constructors, counts);
            }
            if let Some(step_stmt) = step {
                collect_pointer_literals_from_stmt(step_stmt, constructors, counts);
            }
            collect_pointer_literals_from_block(body, constructors, counts);
        }
        Statement::ForEachMap { map, body, .. } => {
            collect_pointer_literals_from_expr(map, constructors, counts);
            collect_pointer_literals_from_block(body, constructors, counts);
        }
    }
}

fn collect_pointer_literals_from_block(
    block: &Block,
    constructors: &HashSet<&str>,
    counts: &mut HashMap<String, usize>,
) {
    for stmt in &block.statements {
        collect_pointer_literals_from_stmt(stmt, constructors, counts);
    }
}

fn collect_pointer_literals_from_expr(
    expr: &Expr,
    constructors: &HashSet<&str>,
    counts: &mut HashMap<String, usize>,
) {
    match expr {
        Expr::Call { name, args } => {
            if constructors.contains(name.as_str())
                && let Some(Expr::String(lit)) = args.first()
            {
                *counts.entry(lit.clone()).or_default() += 1;
            }
            for arg in args {
                collect_pointer_literals_from_expr(arg, constructors, counts);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_pointer_literals_from_expr(left, constructors, counts);
            collect_pointer_literals_from_expr(right, constructors, counts);
        }
        Expr::Unary { expr, .. } => collect_pointer_literals_from_expr(expr, constructors, counts),
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_pointer_literals_from_expr(cond, constructors, counts);
            collect_pointer_literals_from_expr(then_expr, constructors, counts);
            collect_pointer_literals_from_expr(else_expr, constructors, counts);
        }
        Expr::Member { object, .. } => {
            collect_pointer_literals_from_expr(object, constructors, counts);
        }
        Expr::Index { target, index } => {
            collect_pointer_literals_from_expr(target, constructors, counts);
            collect_pointer_literals_from_expr(index, constructors, counts);
        }
        Expr::Tuple(values) => {
            for value in values {
                collect_pointer_literals_from_expr(value, constructors, counts);
            }
        }
        Expr::Bool(_) | Expr::Number(_) | Expr::String(_) | Expr::Bytes(_) | Expr::Ident(_) => {}
    }
}

fn lint_unused_pointer_constructor(
    stmt: &Statement,
    constructors: &HashSet<&str>,
    func_name: &str,
    warnings: &mut Vec<LintWarning>,
) {
    match stmt {
        Statement::Expr(expr) => {
            warn_if_unused_pointer_call(expr, constructors, func_name, warnings)
        }
        Statement::Return(Some(expr)) => {
            warn_if_unused_pointer_call(expr, constructors, func_name, warnings)
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            lint_unused_pointer_constructor_block(then_branch, constructors, func_name, warnings);
            if let Some(else_block) = else_branch {
                lint_unused_pointer_constructor_block(
                    else_block,
                    constructors,
                    func_name,
                    warnings,
                );
            }
        }
        Statement::While { body, .. }
        | Statement::For { body, .. }
        | Statement::ForEachMap { body, .. } => {
            lint_unused_pointer_constructor_block(body, constructors, func_name, warnings);
        }
        Statement::Return(None)
        | Statement::Let { .. }
        | Statement::Assign { .. }
        | Statement::AssignExpr { .. }
        | Statement::Break
        | Statement::Continue => {}
    }
}

fn lint_unused_pointer_constructor_block(
    block: &Block,
    constructors: &HashSet<&str>,
    func_name: &str,
    warnings: &mut Vec<LintWarning>,
) {
    for stmt in &block.statements {
        lint_unused_pointer_constructor(stmt, constructors, func_name, warnings);
    }
}

fn warn_if_unused_pointer_call(
    expr: &Expr,
    constructors: &HashSet<&str>,
    func_name: &str,
    warnings: &mut Vec<LintWarning>,
) {
    if let Expr::Call { name, args } = expr
        && constructors.contains(name.as_str())
        && matches!(args.first(), Some(Expr::String(_)))
    {
        warnings.push(LintWarning {
            code: "unused-pointer-constructor",
            message: LintMessage::Custom {
                message: format!(
                    "result of `{name}` is unused in function `{func_name}`; assign it to a `let` binding or pass it to a syscall"
                ),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotodama::{i18n::Language, parser::parse};

    #[test]
    fn record_expr_idents_collects_only_states() {
        let expr = Expr::Binary {
            op: crate::kotodama::ast::BinaryOp::Add,
            left: Box::new(Expr::Ident("counter".into())),
            right: Box::new(Expr::Ident("temp".into())),
        };
        let state_lookup: HashSet<String> = [String::from("counter"), String::from("balance")]
            .into_iter()
            .collect();
        let mut hits = HashSet::new();
        record_expr_idents(&expr, &state_lookup, &mut hits);
        assert!(hits.contains("counter"));
        assert!(!hits.contains("temp"));
    }

    #[test]
    fn lint_unused_state_flags_state() {
        let program = parse("state int counter; fn main() { let x = 1; }").unwrap();
        let mut warnings = Vec::new();
        lint_unused_state(&program, &mut warnings);
        assert!(warnings.iter().any(|w| w.code == "unused-state"));
    }

    #[test]
    fn lint_unreachable_after_return_flags_code() {
        let program = parse("fn main() { return; let x = 1; }").unwrap();
        let mut warnings = Vec::new();
        lint_unreachable_after_return(&program, &mut warnings);
        assert!(warnings.iter().any(|w| w.code == "unreachable-return"));
    }

    #[test]
    fn lint_program_combines_checks() {
        let program = parse("state int counter; fn main() { return; let x = counter; }").unwrap();
        let warnings = lint_program(&program);
        assert_eq!(warnings.len(), 1, "only unreachable code should remain");
        assert_eq!(warnings[0].code, "unreachable-return");
    }

    #[test]
    fn lint_state_shadowing_flags_parameter() {
        let program = parse("state int balance; fn main(balance: int) {}").unwrap();
        let warnings = lint_program(&program);
        assert!(
            warnings.iter().any(|w| w.code == "state-shadowed"),
            "expected state-shadowed lint when parameter matches state"
        );
    }

    #[test]
    fn lint_unused_parameters_flags_param() {
        let program = parse("fn main(amount: int) { return; }").unwrap();
        let warnings = lint_program(&program);
        assert!(
            warnings.iter().any(|w| w.code == "unused-parameter"),
            "expected unused-parameter lint for unused argument"
        );
    }

    #[test]
    fn lint_unused_parameters_ignores_underscore() {
        let program = parse("fn main(_unused: int) {}").unwrap();
        let warnings = lint_program(&program);
        assert!(
            !warnings.iter().any(|w| w.code == "unused-parameter"),
            "underscore-prefixed arguments should not trigger unused-parameter lint"
        );
    }

    #[test]
    fn lint_warning_localizes_message() {
        let program = parse("state int counter; fn main() {}").unwrap();
        let warnings = lint_program(&program);
        let msg = warnings
            .iter()
            .find(|w| w.code == "unused-state")
            .expect("unused-state lint should be present")
            .localized_message(Language::English);
        assert!(
            msg.contains("counter"),
            "expected localized message to reference the state name: {msg}"
        );
    }

    #[test]
    fn lint_duplicate_pointer_literals_warns() {
        let program = parse(
            "fn main() { let a = account_id(\"alice@wonderland\"); let b = account_id(\"alice@wonderland\"); }",
        )
        .unwrap();
        let warnings = lint_program(&program);
        assert!(
            warnings
                .iter()
                .any(|w| w.code == "duplicate-pointer-literal")
        );
    }

    #[test]
    fn lint_unused_pointer_constructor_warns() {
        let program = parse("fn main() { account_id(\"alice@wonderland\"); }").unwrap();
        let warnings = lint_program(&program);
        assert!(
            warnings
                .iter()
                .any(|w| w.code == "unused-pointer-constructor")
        );
    }

    #[test]
    fn lint_nonliteral_trigger_spec_warns() {
        let program = parse("fn main() { let spec = json(\"{}\"); create_trigger(spec); }")
            .expect("parse trigger");
        let warnings = lint_program(&program);
        assert!(warnings.iter().any(|w| w.code == "nonliteral-trigger-spec"));
    }

    #[test]
    fn lint_literal_trigger_spec_is_silent() {
        let program = parse("fn main() { create_trigger(json(\"{}\")); }").expect("parse trigger");
        let warnings = lint_program(&program);
        assert!(!warnings.iter().any(|w| w.code == "nonliteral-trigger-spec"));
    }

    #[test]
    fn lint_nonliteral_state_map_key_warns() {
        let program = parse("state Foo: Map<int, int>; fn main() { let k = 1; let _x = Foo[k]; }")
            .expect("parse map");
        let warnings = lint_program(&program);
        assert!(
            warnings
                .iter()
                .any(|w| w.code == "nonliteral-state-map-key")
        );
    }

    #[test]
    fn lint_literal_state_map_key_is_silent() {
        let program =
            parse("state Foo: Map<int, int>; fn main() { let _x = Foo[1]; }").expect("parse map");
        let warnings = lint_program(&program);
        assert!(
            !warnings
                .iter()
                .any(|w| w.code == "nonliteral-state-map-key")
        );
    }

    #[test]
    fn lint_nonliteral_state_path_warns() {
        let program =
            parse("fn main() { let p = name(\"foo\"); state_get(p); }").expect("parse state path");
        let warnings = lint_program(&program);
        assert!(warnings.iter().any(|w| w.code == "nonliteral-state-path"));
    }

    #[test]
    fn lint_literal_state_path_is_silent() {
        let program =
            parse("fn main() { let _x = state_get(name(\"foo\")); }").expect("parse state path");
        let warnings = lint_program(&program);
        assert!(!warnings.iter().any(|w| w.code == "nonliteral-state-path"));
    }

    #[test]
    fn lint_opaque_access_hints_warns() {
        let program =
            parse("fn main() { execute_query(json(\"{}\")); }").expect("parse opaque call");
        let warnings = lint_program(&program);
        assert!(warnings.iter().any(|w| w.code == "opaque-access-hints"));
    }

    #[test]
    fn lint_opaque_access_hints_execute_instruction_literal_is_silent() {
        use iroha_data_model::{
            account::AccountId,
            asset::id::{AssetDefinitionId, AssetId},
            isi::{InstructionBox, Mint},
        };

        let account: AccountId =
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
                .parse()
                .unwrap();
        let asset_def: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let asset_id = AssetId::of(asset_def, account);
        let isi = InstructionBox::from(Mint::asset_numeric(1u32, asset_id));
        let bytes = norito::to_bytes(&isi).expect("encode InstructionBox");
        let hex_payload = format!("0x{}", hex::encode(bytes));
        let src = format!(
            "fn main() {{ execute_instruction(norito_bytes(\"{hex_payload}\")); }}"
        );

        let program = parse(&src).expect("parse execute_instruction literal");
        let warnings = lint_program(&program);
        assert!(
            !warnings.iter().any(|w| w.code == "opaque-access-hints"),
            "literal execute_instruction payloads should not warn"
        );
    }

    #[test]
    fn lint_opaque_access_hints_execute_query_literal_is_silent() {
        use iroha_data_model::{
            account::AccountId,
            asset::id::{AssetDefinitionId, AssetId},
            query::asset::FindAssetById,
            query::{QueryRequest, SingularQueryBox},
        };

        let account: AccountId =
            "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
                .parse()
                .unwrap();
        let asset_def: AssetDefinitionId = "rose#wonderland".parse().unwrap();
        let asset_id = AssetId::of(asset_def, account);
        let request = QueryRequest::Singular(SingularQueryBox::FindAssetById(
            FindAssetById::new(asset_id),
        ));
        let bytes = norito::to_bytes(&request).expect("encode QueryRequest");
        let hex_payload = format!("0x{}", hex::encode(bytes));
        let src = format!(
            "fn main() {{ execute_query(norito_bytes(\"{hex_payload}\")); }}"
        );

        let program = parse(&src).expect("parse execute_query literal");
        let warnings = lint_program(&program);
        assert!(
            !warnings.iter().any(|w| w.code == "opaque-access-hints"),
            "literal execute_query payloads should not warn"
        );
    }
}
