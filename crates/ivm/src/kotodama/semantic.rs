use std::{cell::RefCell, collections::HashMap};

use indexmap::{IndexMap, IndexSet};

use super::ast::*;

#[derive(Clone, Default)]
struct FunctionSummary {
    direct_sensitive: bool,
    calls: IndexSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    String,
    /// Pointer-ABI raw bytes blob
    Blob,
    /// First-class raw byte sequence (alias to Blob in the pointer ABI).
    Bytes,
    /// Dataspace identifier used for Nexus/AXT flows.
    DataSpaceId,
    /// Atomic cross-transaction descriptor pointer.
    AxtDescriptor,
    /// Capability handle provided by asset dataspace issuers.
    AssetHandle,
    /// Proof material supplied by dataspace verifiers.
    ProofBlob,
    AccountId,
    AssetDefinitionId,
    AssetId,
    NftId,
    DomainId,
    Name,
    Json,
    Unit,
    Map(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),
    /// User-defined product type with named fields.
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
    },
    Opaque(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub expr: ExprKind,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Binary {
        op: BinaryOp,
        left: Box<TypedExpr>,
        right: Box<TypedExpr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    /// Ternary conditional expression: `cond ? then : else`.
    Conditional {
        cond: Box<TypedExpr>,
        then_expr: Box<TypedExpr>,
        else_expr: Box<TypedExpr>,
    },
    Call {
        name: String,
        args: Vec<TypedExpr>,
    },
    Tuple(Vec<TypedExpr>),
    Member {
        object: Box<TypedExpr>,
        field: String,
    },
    Index {
        target: Box<TypedExpr>,
        index: Box<TypedExpr>,
    },
    Number(i64),
    Bool(bool),
    String(String),
    Ident(String),
}

#[derive(Debug, PartialEq)]
pub struct SemanticError {
    pub message: String,
}

#[derive(Debug, PartialEq)]
pub struct TypedProgram {
    pub items: Vec<TypedItem>,
    pub contract_meta: Option<ContractMeta>,
}

thread_local! {
    static STRUCT_ENV: RefCell<HashMap<String, Vec<(String, Type)>>> = RefCell::new(HashMap::new());
    static STATE_ENV: RefCell<IndexMap<String, Type>> = RefCell::new(IndexMap::new());
    static FUNCTION_RETURNS: RefCell<HashMap<String, Type>> = RefCell::new(HashMap::new());
    static FUNCTION_SUMMARY: RefCell<HashMap<String, FunctionSummary>> = RefCell::new(HashMap::new());
}

const SENSITIVE_SYSCALLS: &[&str] = &[
    "set_account_detail",
    "transfer_asset",
    "transfer_v1_batch_begin",
    "transfer_v1_batch_end",
    "mint_asset",
    "burn_asset",
    "register_asset",
    "unregister_asset",
    "nft_mint_asset",
    "nft_transfer_asset",
    "nft_set_metadata",
    "nft_burn_asset",
    "register_domain",
    "unregister_domain",
    "transfer_domain",
    "register_account",
    "unregister_account",
    "register_peer",
    "unregister_peer",
    "create_trigger",
    "remove_trigger",
    "set_trigger_enabled",
    "grant_permission",
    "revoke_permission",
    "create_role",
    "delete_role",
    "grant_role",
    "revoke_role",
];

pub fn analyze(program: &Program) -> Result<TypedProgram, SemanticError> {
    // Collect struct definitions up front and publish to thread-local env for this analysis pass.
    let mut structs: HashMap<String, Vec<(String, Type)>> = HashMap::new();
    let mut state: IndexMap<String, Type> = IndexMap::new();
    let mut fn_returns: HashMap<String, Type> = HashMap::new();
    FUNCTION_SUMMARY.with(|map| map.borrow_mut().clear());
    for item in &program.items {
        match item {
            Item::Struct(def) => {
                let mut fields = Vec::new();
                for (name, ty_expr) in &def.fields {
                    fields.push((name.clone(), convert_type_expr(ty_expr)?));
                }
                structs.insert(def.name.clone(), fields);
            }
            Item::State(st) => {
                let ty = convert_type_expr(&st.ty)?;
                validate_state_type(&ty)?;
                state.insert(st.name.clone(), ty);
            }
            Item::Function(f) => {
                let ret = if let Some(ret_ty) = &f.ret_ty {
                    convert_type_expr(ret_ty)?
                } else {
                    Type::Unit
                };
                fn_returns.insert(f.name.clone(), ret);
            }
        }
    }
    STRUCT_ENV.with(|env| env.replace(structs));
    let resolved_state: IndexMap<String, Type> = state
        .into_iter()
        .map(|(name, ty)| (name, resolve_struct_type(&ty)))
        .collect();
    STATE_ENV.with(|env| env.replace(resolved_state));
    FUNCTION_RETURNS.with(|env| env.replace(fn_returns));

    let mut items = Vec::new();
    for item in &program.items {
        match item {
            Item::Function(f) => items.push(TypedItem::Function(analyze_function(f)?)),
            Item::Struct(_) => {}
            Item::State(_) => {}
        }
    }
    enforce_permission_requirements(&items)?;
    Ok(TypedProgram {
        items,
        contract_meta: program.contract_meta.clone(),
    })
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Int => "int".into(),
        Type::Bool => "bool".into(),
        Type::String => "string".into(),
        Type::Blob => "Blob".into(),
        Type::Bytes => "bytes".into(),
        Type::DataSpaceId => "DataSpaceId".into(),
        Type::AxtDescriptor => "AxtDescriptor".into(),
        Type::AssetHandle => "AssetHandle".into(),
        Type::ProofBlob => "ProofBlob".into(),
        Type::AccountId => "AccountId".into(),
        Type::AssetDefinitionId => "AssetDefinitionId".into(),
        Type::AssetId => "AssetId".into(),
        Type::NftId => "NftId".into(),
        Type::DomainId => "DomainId".into(),
        Type::Name => "Name".into(),
        Type::Json => "Json".into(),
        Type::Unit => "()".into(),
        Type::Map(k, v) => format!("Map<{}, {}>", type_name(k), type_name(v)),
        Type::Tuple(ts) => {
            let parts: Vec<String> = ts.iter().map(type_name).collect();
            format!("({})", parts.join(", "))
        }
        Type::Struct { name, .. } => format!("struct {name}"),
        Type::Opaque(s) => s.clone(),
    }
}

fn is_supported_durable_value_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int | Type::Bool | Type::Json | Type::Blob | Type::Bytes => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
}

fn is_supported_durable_key_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
}

fn is_in_memory_map_word_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int | Type::Bool | Type::String | Type::Blob | Type::Bytes | Type::Json => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
}

fn ensure_in_memory_map_word_types(map_expr: &TypedExpr) -> Result<(), SemanticError> {
    if typed_map_expr_is_state(map_expr) {
        return Ok(());
    }
    match resolve_struct_type(&map_expr.ty) {
        Type::Map(k, v) => {
            if !is_in_memory_map_word_type(&k) {
                return Err(SemanticError {
                    message: format!(
                        "in-memory Map key type `{}` is not supported; use int, bool, string, Blob, bytes, Json, or pointer types",
                        type_name(&k)
                    ),
                });
            }
            if !is_in_memory_map_word_type(&v) {
                return Err(SemanticError {
                    message: format!(
                        "in-memory Map value type `{}` is not supported; use int, bool, string, Blob, bytes, Json, or pointer types",
                        type_name(&v)
                    ),
                });
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_state_type(ty: &Type) -> Result<(), SemanticError> {
    match resolve_struct_type(ty) {
        Type::Map(k, v) => {
            if !is_supported_durable_key_type(&k) {
                return Err(SemanticError {
                    message: format!(
                        "state Map key type `{}` is not supported for durable storage; use int or pointer types",
                        type_name(&k)
                    ),
                });
            }
            if !is_supported_durable_value_type(&v) {
                return Err(SemanticError {
                    message: format!(
                        "state Map value type `{}` is not supported for durable storage; use int, bool, Json, Blob, or pointer types",
                        type_name(&v)
                    ),
                });
            }
            Ok(())
        }
        Type::Struct { fields, .. } => {
            for (_, field_ty) in fields {
                validate_state_type(&field_ty)?;
            }
            Ok(())
        }
        Type::Tuple(items) => {
            for item in items {
                validate_state_type(&item)?;
            }
            Ok(())
        }
        other => {
            if is_supported_durable_value_type(&other) {
                Ok(())
            } else {
                Err(SemanticError {
                    message: format!(
                        "state type `{}` is not supported for durable storage; use int, bool, Json, Blob, or pointer types",
                        type_name(&other)
                    ),
                })
            }
        }
    }
}

pub(crate) fn is_blob_like(ty: &Type) -> bool {
    matches!(resolve_struct_type(ty), Type::Blob | Type::Bytes)
}

fn is_eq_comparable_type(ty: &Type) -> bool {
    match resolve_struct_type(ty) {
        Type::Int | Type::Bool | Type::String | Type::Blob | Type::Bytes | Type::Json => true,
        other if is_pointer_type(&other) => true,
        _ => false,
    }
}

pub fn is_pointer_type(ty: &Type) -> bool {
    matches!(
        resolve_struct_type(ty),
        Type::AccountId
            | Type::AssetDefinitionId
            | Type::AssetId
            | Type::DomainId
            | Type::NftId
            | Type::Name
            | Type::DataSpaceId
            | Type::AxtDescriptor
            | Type::AssetHandle
            | Type::ProofBlob
    )
}

const TRANSFER_BATCH_SIGNATURE: &str =
    "(AccountId, AccountId, AssetDefinitionId, int) tuple entries";

fn is_transfer_batch_entry_tuple(ty: &Type) -> bool {
    match ty {
        Type::Tuple(fields) if fields.len() == 4 => {
            matches!(resolve_struct_type(&fields[0]), Type::AccountId)
                && matches!(resolve_struct_type(&fields[1]), Type::AccountId)
                && matches!(resolve_struct_type(&fields[2]), Type::AssetDefinitionId)
                && matches!(resolve_struct_type(&fields[3]), Type::Int)
        }
        _ => false,
    }
}

fn ensure_transfer_batch_args(args: &[TypedExpr]) -> Result<(), SemanticError> {
    if args.is_empty() {
        return Err(SemanticError {
            message: "transfer_batch expects at least one entry".into(),
        });
    }
    if args
        .iter()
        .all(|expr| is_transfer_batch_entry_tuple(&expr.ty))
    {
        return Ok(());
    }
    Err(SemanticError {
        message: format!("transfer_batch expects {}", TRANSFER_BATCH_SIGNATURE),
    })
}

/// Recursively bind nested struct fields into `name#i#j` variables for convenient lowering.
fn bind_struct_fields_rec(
    out: &mut Vec<TypedStatement>,
    vars: &mut HashMap<String, Type>,
    base_name: &str,
    base_expr: &TypedExpr,
    ty: &Type,
) {
    let resolved_ty = resolve_struct_type(ty);
    if let Type::Struct { fields, .. } = resolved_ty {
        for (i, (_fname, fty)) in fields.iter().enumerate() {
            let member = TypedExpr {
                expr: ExprKind::Member {
                    object: Box::new(base_expr.clone()),
                    field: i.to_string(),
                },
                ty: resolve_struct_type(fty),
            };
            let sname = format!("{base_name}#{i}");
            let field_ty = resolve_struct_type(fty);
            vars.insert(sname.clone(), field_ty.clone());
            out.push(TypedStatement::Let {
                name: sname.clone(),
                value: member.clone(),
            });
            bind_struct_fields_rec(out, vars, &sname, &member, &field_ty);
        }
    }
}

/// Recursively bind tuple elements into `name#i` variables so older lowering
/// helpers can access flattened names. Nested structs continue to use the
/// existing struct binding helper, and nested tuples recurse naturally.
fn bind_tuple_fields_rec(
    out: &mut Vec<TypedStatement>,
    vars: &mut HashMap<String, Type>,
    base_name: &str,
    base_expr: &TypedExpr,
    ty: &Type,
) {
    if let Type::Tuple(elements) = resolve_struct_type(ty) {
        for (idx, elem_ty) in elements.iter().enumerate() {
            let resolved_elem_ty = resolve_struct_type(elem_ty);
            let element_expr = if let ExprKind::Tuple(items) = &base_expr.expr {
                if let Some(item) = items.get(idx) {
                    item.clone()
                } else {
                    TypedExpr {
                        expr: ExprKind::Member {
                            object: Box::new(base_expr.clone()),
                            field: idx.to_string(),
                        },
                        ty: resolved_elem_ty.clone(),
                    }
                }
            } else {
                TypedExpr {
                    expr: ExprKind::Member {
                        object: Box::new(base_expr.clone()),
                        field: idx.to_string(),
                    },
                    ty: resolved_elem_ty.clone(),
                }
            };
            let child_name = format!("{base_name}#{idx}");
            vars.insert(child_name.clone(), resolved_elem_ty.clone());
            out.push(TypedStatement::Let {
                name: child_name.clone(),
                value: element_expr.clone(),
            });
            bind_tuple_fields_rec(out, vars, &child_name, &element_expr, &resolved_elem_ty);
            bind_struct_fields_rec(out, vars, &child_name, &element_expr, &resolved_elem_ty);
        }
    }
}

fn analyze_function(func: &Function) -> Result<TypedFunction, SemanticError> {
    let mut vars = HashMap::new();
    let mut param_names = Vec::new();
    // Seed variable environment with contract-level state declarations so
    // functions can reference `state` names directly.
    STATE_ENV.with(|env| {
        for (name, ty) in env.borrow().iter() {
            vars.insert(name.clone(), ty.clone());
        }
    });
    for param in &func.params {
        ensure_not_state_shadow(&param.name)?;
        let ty = parse_declared_param_type(&param.ty, &param.name)?;
        vars.insert(param.name.clone(), ty);
        param_names.push(param.name.clone());
    }
    let expected_ret = parse_declared_type(&func.ret_ty)?;
    let body = analyze_block(&func.body, &mut vars, expected_ret.as_ref(), 0)?;
    // Enforce declared return coverage and shape
    if let Some(t) = &expected_ret {
        if *t != Type::Unit && !block_returns_all_paths(&func.body) {
            return Err(SemanticError {
                message: "not all paths return a value".into(),
            });
        }
    } else {
        // No declared return type: disallow returning a value to avoid ambiguity
        if block_has_return_value(&func.body) {
            return Err(SemanticError {
                message: "function returns a value but has no declared return type".into(),
            });
        }
    }
    let summary = FunctionSummary {
        direct_sensitive: block_contains_sensitive_syscall(&body),
        calls: collect_called_functions(&body),
    };
    FUNCTION_SUMMARY.with(|map| {
        map.borrow_mut().insert(func.name.clone(), summary);
    });
    Ok(TypedFunction {
        name: func.name.clone(),
        params: param_names,
        body,
        ret_ty: expected_ret,
        modifiers: func.modifiers.clone(),
    })
}

fn analyze_block(
    block: &Block,
    vars: &mut HashMap<String, Type>,
    expected_ret: Option<&Type>,
    loop_depth: usize,
) -> Result<TypedBlock, SemanticError> {
    let _ = loop_depth;
    let mut statements = Vec::new();
    for stmt in &block.statements {
        let mut v = analyze_statement(stmt, vars, expected_ret, loop_depth)?;
        statements.append(&mut v);
    }
    Ok(TypedBlock { statements })
}

fn analyze_statement(
    stmt: &Statement,
    vars: &mut HashMap<String, Type>,
    expected_ret: Option<&Type>,
    loop_depth: usize,
) -> Result<Vec<TypedStatement>, SemanticError> {
    let _ = loop_depth;
    match stmt {
        Statement::Let { pat, ty, value } => {
            let mut expr = analyze_expr(value, vars)?;
            if let Some(tann) = ty {
                let dt = convert_type_expr(tann)?;
                apply_map_new_type_hint(&mut expr, &dt);
                ensure_assignable(&dt, &expr.ty)?;
            }
            if is_state_map_expr(&expr) {
                return Err(SemanticError {
                    message: "E_STATE_MAP_ALIAS: state maps are not first-class; use the state identifier directly.".into(),
                });
            }
            match pat {
                Pattern::Name(name) => {
                    ensure_not_state_shadow(name)?;
                    // Bind the name and, if it's a tuple, also synthesize per-field bindings name#i.
                    let mut out = Vec::new();
                    vars.insert(name.clone(), expr.ty.clone());
                    out.push(TypedStatement::Let {
                        name: name.clone(),
                        value: expr.clone(),
                    });
                    match &expr.ty {
                        Type::Tuple(_) => {
                            bind_tuple_fields_rec(&mut out, vars, name, &expr, &expr.ty);
                        }
                        Type::Struct { fields, .. } => {
                            for (i, (_fname, fty)) in fields.iter().enumerate() {
                                // If the RHS is a struct constructor (tuple of field exprs), bind directly.
                                let val_expr = if let ExprKind::Tuple(ts) = &expr.expr {
                                    ts.get(i).cloned().unwrap_or(TypedExpr {
                                        expr: ExprKind::Number(0),
                                        ty: fty.clone(),
                                    })
                                } else {
                                    TypedExpr {
                                        expr: ExprKind::Member {
                                            object: Box::new(expr.clone()),
                                            field: i.to_string(),
                                        },
                                        ty: fty.clone(),
                                    }
                                };
                                let sname = format!("{name}#{i}");
                                let field_ty = resolve_struct_type(fty);
                                vars.insert(sname.clone(), field_ty.clone());
                                out.push(TypedStatement::Let {
                                    name: sname.clone(),
                                    value: val_expr.clone(),
                                });
                                bind_struct_fields_rec(
                                    &mut out, vars, &sname, &val_expr, &field_ty,
                                );
                            }
                        }
                        _ => {}
                    }
                    Ok(out)
                }
                Pattern::Tuple(names) => {
                    let mut out = Vec::new();
                    for name in names.iter() {
                        ensure_not_state_shadow(name)?;
                    }
                    match &expr.ty {
                        Type::Tuple(ts) => {
                            if names.len() != ts.len() {
                                return Err(SemanticError {
                                    message: format!(
                                        "tuple destructuring expects {} bindings, got {}",
                                        ts.len(),
                                        names.len()
                                    ),
                                });
                            }
                            // Destructure by emitting member-access typed expressions for each field.
                            for (i, name) in names.iter().enumerate() {
                                let ti = ts.get(i).cloned().expect("tuple arity already validated");
                                let member = TypedExpr {
                                    expr: ExprKind::Member {
                                        object: Box::new(expr.clone()),
                                        field: i.to_string(),
                                    },
                                    ty: ti.clone(),
                                };
                                vars.insert(name.clone(), ti.clone());
                                out.push(TypedStatement::Let {
                                    name: name.clone(),
                                    value: member,
                                });
                            }
                        }
                        Type::Struct { fields, .. } => {
                            if names.len() != fields.len() {
                                return Err(SemanticError {
                                    message: format!(
                                        "struct destructuring expects {} bindings, got {}",
                                        fields.len(),
                                        names.len()
                                    ),
                                });
                            }
                            for (i, name) in names.iter().enumerate() {
                                let (_fname, ti) = fields
                                    .get(i)
                                    .cloned()
                                    .expect("struct arity already validated");
                                let val_expr = if let ExprKind::Tuple(ts) = &expr.expr {
                                    ts.get(i).cloned().unwrap_or(TypedExpr {
                                        expr: ExprKind::Number(0),
                                        ty: resolve_struct_type(&ti),
                                    })
                                } else {
                                    TypedExpr {
                                        expr: ExprKind::Member {
                                            object: Box::new(expr.clone()),
                                            field: i.to_string(),
                                        },
                                        ty: resolve_struct_type(&ti),
                                    }
                                };
                                let field_ty = resolve_struct_type(&ti);
                                vars.insert(name.clone(), field_ty.clone());
                                out.push(TypedStatement::Let {
                                    name: name.clone(),
                                    value: val_expr.clone(),
                                });
                                bind_struct_fields_rec(&mut out, vars, name, &val_expr, &field_ty);
                            }
                        }
                        _ => {
                            return Err(SemanticError {
                                message: "tuple destructuring expects a tuple or struct".into(),
                            });
                        }
                    }
                    Ok(out)
                }
            }
        }
        Statement::Assign { name, value } => {
            // Must exist
            let expected = vars.get(name).cloned().ok_or_else(|| SemanticError {
                message: format!("undefined variable {name}"),
            })?;
            if is_state_identifier(name)
                && matches!(resolve_struct_type(&expected), Type::Map(_, _))
            {
                return Err(SemanticError {
                    message:
                        "E_STATE_MAP_ALIAS: state maps cannot be reassigned; use map indexing."
                            .into(),
                });
            }
            let mut expr = analyze_expr(value, vars)?;
            if is_state_map_expr(&expr) {
                return Err(SemanticError {
                    message: "E_STATE_MAP_ALIAS: state maps are not first-class; use the state identifier directly.".into(),
                });
            }
            apply_map_new_type_hint(&mut expr, &expected);
            ensure_assignable(&expected, &expr.ty)?;
            // Rebind SSA name to new value
            vars.insert(name.clone(), expr.ty.clone());
            let mut out = Vec::new();
            out.push(TypedStatement::Let {
                name: name.clone(),
                value: expr.clone(),
            });
            bind_tuple_fields_rec(&mut out, vars, name, &expr, &expr.ty);
            Ok(out)
        }
        Statement::AssignExpr { target, value } => {
            // support map indexing and simple variable rebinding
            match target {
                Expr::Index { target: map, index } => {
                    let map_t = analyze_expr(map, vars)?;
                    let key_t = analyze_expr(index, vars)?;
                    let val_t = analyze_expr(value, vars)?;
                    match map_t.ty.clone() {
                        Type::Map(k, v) => {
                            ensure_assignable(&k, &key_t.ty)?;
                            ensure_assignable(&v, &val_t.ty)?;
                            ensure_in_memory_map_word_types(&map_t)?;
                        }
                        other => {
                            return Err(SemanticError {
                                message: format!(
                                    "map assignment expects Map<K,V> target, got {}",
                                    type_name(&other)
                                ),
                            });
                        }
                    }
                    Ok(vec![TypedStatement::MapSet {
                        map: map_t,
                        key: key_t,
                        value: val_t,
                    }])
                }
                Expr::Ident(name) => {
                    // Simple compound assignment lowering: rebind SSA name
                    let expected = vars.get(name).cloned().ok_or_else(|| SemanticError {
                        message: format!("undefined variable {name}"),
                    })?;
                    if is_state_identifier(name)
                        && matches!(resolve_struct_type(&expected), Type::Map(_, _))
                    {
                        return Err(SemanticError {
                            message:
                                "E_STATE_MAP_ALIAS: state maps cannot be reassigned; use map indexing."
                                    .into(),
                        });
                    }
                    let mut expr = analyze_expr(value, vars)?;
                    if is_state_map_expr(&expr) {
                        return Err(SemanticError {
                            message: "E_STATE_MAP_ALIAS: state maps are not first-class; use the state identifier directly.".into(),
                        });
                    }
                    apply_map_new_type_hint(&mut expr, &expected);
                    ensure_assignable(&expected, &expr.ty)?;
                    vars.insert(name.clone(), expr.ty.clone());
                    let mut out = Vec::new();
                    out.push(TypedStatement::Let {
                        name: name.clone(),
                        value: expr.clone(),
                    });
                    bind_tuple_fields_rec(&mut out, vars, name, &expr, &expr.ty);
                    Ok(out)
                }
                _ => {
                    return Err(SemanticError {
                        message: "assignment target must be a variable or map index".into(),
                    });
                }
            }
        }
        Statement::Expr(e) => Ok(vec![TypedStatement::Expr(analyze_expr(e, vars)?)]),
        Statement::Return(opt) => {
            let mut tv = if let Some(e) = opt {
                Some(analyze_expr(e, vars)?)
            } else {
                None
            };
            if expected_ret.is_none() {
                if tv.is_some() {
                    return Err(SemanticError {
                        message: "returning a value requires a declared return type".into(),
                    });
                }
            } else if let Some(exp) = expected_ret {
                if let Some(expr) = tv.as_mut() {
                    apply_map_new_type_hint(expr, exp);
                }
                match (&tv, exp) {
                    (None, Type::Unit) => {}
                    (None, _) => {
                        return Err(SemanticError {
                            message: "return type mismatch: expected value".into(),
                        });
                    }
                    (Some(_t), Type::Unit) => {
                        return Err(SemanticError {
                            message: "return type mismatch: unexpected value".into(),
                        });
                    }
                    (Some(t), e) => {
                        if let Err(mut err) = ensure_assignable(e, &t.ty) {
                            err.message = format!("return type mismatch: {}", err.message);
                            return Err(err);
                        }
                    }
                }
            }
            Ok(vec![TypedStatement::Return(tv)])
        }
        Statement::Break => {
            if loop_depth == 0 {
                return Err(SemanticError {
                    message: "E_BREAK_OUTSIDE_LOOP: `break` must appear inside a loop".into(),
                });
            }
            Ok(vec![TypedStatement::Break])
        }
        Statement::Continue => {
            if loop_depth == 0 {
                return Err(SemanticError {
                    message: "E_CONTINUE_OUTSIDE_LOOP: `continue` must appear inside a loop".into(),
                });
            }
            Ok(vec![TypedStatement::Continue])
        }
        Statement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_t = analyze_expr(cond, vars)?;
            if cond_t.ty != Type::Bool {
                return Err(SemanticError {
                    message: "if condition must be bool".into(),
                });
            }
            let then_block =
                analyze_block(then_branch, &mut vars.clone(), expected_ret, loop_depth)?;
            let else_block = if let Some(b) = else_branch {
                Some(analyze_block(
                    b,
                    &mut vars.clone(),
                    expected_ret,
                    loop_depth,
                )?)
            } else {
                None
            };
            Ok(vec![TypedStatement::If {
                cond: cond_t,
                then_branch: then_block,
                else_branch: else_block,
            }])
        }
        Statement::While { cond, body } => {
            let cond_t = analyze_expr(cond, vars)?;
            if cond_t.ty != Type::Bool {
                return Err(SemanticError {
                    message: "while condition must be bool".into(),
                });
            }
            let body_t = analyze_block(body, &mut vars.clone(), expected_ret, loop_depth + 1)?;
            Ok(vec![TypedStatement::While {
                cond: cond_t,
                body: body_t,
            }])
        }
        Statement::For {
            line,
            init,
            cond,
            step,
            body,
        } => {
            let mut local = vars.clone();
            let init_t = if let Some(s) = init {
                let mut v = analyze_statement(s, &mut local, expected_ret, loop_depth)?;
                if v.len() != 1 {
                    return Err(SemanticError {
                        message: "E0005: for-loop initializer must be a simple let or expression"
                            .into(),
                    });
                }
                Some(Box::new(v.remove(0)))
            } else {
                None
            };
            let loop_env = local.clone();
            let cond_t = if let Some(c) = cond {
                let mut cond_vars = loop_env.clone();
                let t = analyze_expr(c, &mut cond_vars)?;
                if t.ty != Type::Bool {
                    return Err(SemanticError {
                        message: "for condition must be bool".into(),
                    });
                }
                Some(t)
            } else {
                None
            };
            let step_t = if let Some(s) = step {
                let mut step_vars = loop_env.clone();
                let mut v = analyze_statement(s, &mut step_vars, expected_ret, loop_depth + 1)?;
                if v.len() != 1 {
                    return Err(SemanticError {
                        message: "E0006: for-loop step must be a simple let or expression".into(),
                    });
                }
                Some(Box::new(v.remove(0)))
            } else {
                None
            };
            let body_t = analyze_block(body, &mut loop_env.clone(), expected_ret, loop_depth + 1)?;
            *vars = loop_env;
            Ok(vec![TypedStatement::For {
                line: *line,
                init: init_t,
                cond: cond_t,
                step: step_t,
                body: body_t,
            }])
        }
        Statement::ForEachMap {
            key,
            value,
            map,
            bound: attr_bound,
            body,
        } => {
            if let Some(bound_ref) = attr_bound {
                let bound_value = *bound_ref;
                if bound_value > 1 && !map_expr_is_state(map) {
                    return Err(SemanticError {
                        message: "E_MAP_BOUNDS: in-memory Map iteration supports at most 1 element; reduce the bound or move the map into `state`."
                            .into(),
                    });
                }
                let base_map = analyze_expr(map, &mut vars.clone())?;
                ensure_state_map_iter_supported(&base_map)?;
                ensure_in_memory_map_word_types(&base_map)?;
                let mut local_vars = vars.clone();
                let (k_ty, v_ty) = match &base_map.ty {
                    Type::Map(k, v) => ((**k).clone(), (**v).clone()),
                    _ => (Type::Int, Type::Int),
                };
                ensure_not_state_shadow(key)?;
                local_vars.insert(key.clone(), k_ty);
                if let Some(val_name) = value {
                    ensure_not_state_shadow(val_name)?;
                    local_vars.insert(val_name.clone(), v_ty);
                }
                let body_t = analyze_block(body, &mut local_vars, expected_ret, loop_depth + 1)?;
                if let Expr::Ident(map_name) = &map
                    && block_mutates_map(&body_t, map_name)
                {
                    return Err(SemanticError {
                        message:
                            "E_ITER_MUTATION: structural modifications to the iterated map are forbidden during iteration"
                                .into(),
                    });
                }
                return Ok(vec![TypedStatement::ForEachMap {
                    key: key.clone(),
                    value: value.clone(),
                    map: base_map,
                    body: body_t,
                    start: 0,
                    bound: Some(bound_value),
                    #[cfg(feature = "kotodama_dynamic_bounds")]
                    dyn_count: None,
                    #[cfg(feature = "kotodama_dynamic_bounds")]
                    dyn_start: None,
                }]);
            }
            // Accept bounded forms: `.take(n)` and `.range(start, n)`
            // Desugar to a typed for-each with the base map expression and rely on
            // the current IR lowering which performs a deterministic two-iteration walk.
            // The semantic layer keeps track of the literal bound even though the
            // downstream lowering still clamps to two iterations. Once dynamic bounds
            // are supported end-to-end we can remove that limitation.
            if let Expr::Call { name, args } = map {
                if name == "take" && args.len() == 2 {
                    // Analyze base map expression and infer key/value types
                    let base_map = analyze_expr(&args[0], &mut vars.clone())?;
                    // Extend a local scope with loop variables bound to inferred types
                    ensure_state_map_iter_supported(&base_map)?;
                    ensure_in_memory_map_word_types(&base_map)?;
                    let mut local_vars = vars.clone();
                    let (k_ty, v_ty) = match &base_map.ty {
                        Type::Map(k, v) => ((**k).clone(), (**v).clone()),
                        _ => (Type::Int, Type::Int),
                    };
                    ensure_not_state_shadow(key)?;
                    local_vars.insert(key.clone(), k_ty);
                    if let Some(val_name) = value {
                        ensure_not_state_shadow(val_name)?;
                        local_vars.insert(val_name.clone(), v_ty);
                    }
                    let body_t =
                        analyze_block(body, &mut local_vars, expected_ret, loop_depth + 1)?;
                    let literal_bound = match &args[1] {
                        Expr::Number(n) if *n >= 0 => Some(*n as usize),
                        _ => None,
                    };
                    if literal_bound.is_none() {
                        return Err(SemanticError{ message: "E_UNBOUNDED_ITERATION: `.take(n)` requires a positive integer literal for now".into() });
                    }
                    if literal_bound.unwrap() > 1 && !map_expr_is_state(&args[0]) {
                        return Err(SemanticError {
                            message: "E_MAP_BOUNDS: in-memory Map iteration supports at most 1 element; reduce the bound or move the map into `state`.".into(),
                        });
                    }
                    // E_ITER_MUTATION: forbid structural modifications to the iterated map inside the loop body
                    if let Expr::Ident(map_name) = &args[0]
                        && block_mutates_map(&body_t, map_name)
                    {
                        return Err(SemanticError { message: "E_ITER_MUTATION: structural modifications to the iterated map are forbidden during iteration".into() });
                    }
                    return Ok(vec![TypedStatement::ForEachMap {
                        key: key.clone(),
                        value: value.clone(),
                        map: base_map,
                        body: body_t,
                        start: 0,
                        bound: literal_bound,
                        #[cfg(feature = "kotodama_dynamic_bounds")]
                        dyn_count: None,
                        #[cfg(feature = "kotodama_dynamic_bounds")]
                        dyn_start: None,
                    }]);
                }
                #[cfg(feature = "kotodama_dynamic_bounds")]
                if name == "take" && args.len() == 2 {
                    // Dynamic: accept non-literal n with runtime assert (cap lowering to 2)
                    if !map_expr_is_state(&args[0]) {
                        return Err(SemanticError {
                            message: "E_MAP_BOUNDS: dynamic bounds on in-memory Map iteration are unsupported; move the map into `state`."
                                .into(),
                        });
                    }
                    let base_map = analyze_expr(&args[0], &mut vars.clone())?;
                    ensure_state_map_iter_supported(&base_map)?;
                    ensure_in_memory_map_word_types(&base_map)?;
                    let mut local_vars = vars.clone();
                    let (k_ty, v_ty) = match &base_map.ty {
                        Type::Map(k, v) => ((**k).clone(), (**v).clone()),
                        _ => (Type::Int, Type::Int),
                    };
                    ensure_not_state_shadow(key)?;
                    local_vars.insert(key.clone(), k_ty);
                    if let Some(val_name) = value {
                        ensure_not_state_shadow(val_name)?;
                        local_vars.insert(val_name.clone(), v_ty);
                    }
                    let body_t =
                        analyze_block(body, &mut local_vars, expected_ret, loop_depth + 1)?;
                    // Build a typed assert: n <= 2 && n >= 0
                    let n_t = analyze_expr(&args[1], &mut vars.clone())?;
                    let two = TypedExpr {
                        expr: ExprKind::Number(2),
                        ty: Type::Int,
                    };
                    let zero = TypedExpr {
                        expr: ExprKind::Number(0),
                        ty: Type::Int,
                    };
                    let le = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Le,
                            left: Box::new(n_t.clone()),
                            right: Box::new(two),
                        },
                        ty: Type::Bool,
                    };
                    let ge = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Ge,
                            left: Box::new(n_t.clone()),
                            right: Box::new(zero),
                        },
                        ty: Type::Bool,
                    };
                    let cond = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::And,
                            left: Box::new(le),
                            right: Box::new(ge),
                        },
                        ty: Type::Bool,
                    };
                    let assert_stmt = TypedStatement::Expr(TypedExpr {
                        expr: ExprKind::Call {
                            name: "assert".into(),
                            args: vec![cond],
                        },
                        ty: Type::Unit,
                    });
                    // Emit a single ForEachMap with dynamic count; IR will guard per-iteration
                    return Ok(vec![
                        assert_stmt,
                        TypedStatement::ForEachMap {
                            key: key.clone(),
                            value: value.clone(),
                            map: base_map,
                            body: body_t,
                            start: 0,
                            bound: None,
                            dyn_count: Some(n_t),
                            dyn_start: None,
                        },
                    ]);
                }
                if name == "range" && args.len() == 3 {
                    // range(start, n)
                    let base_map = analyze_expr(&args[0], &mut vars.clone())?;
                    ensure_state_map_iter_supported(&base_map)?;
                    ensure_in_memory_map_word_types(&base_map)?;
                    let mut local_vars = vars.clone();
                    let (k_ty, v_ty) = match &base_map.ty {
                        Type::Map(k, v) => ((**k).clone(), (**v).clone()),
                        _ => (Type::Int, Type::Int),
                    };
                    ensure_not_state_shadow(key)?;
                    local_vars.insert(key.clone(), k_ty);
                    if let Some(val_name) = value {
                        ensure_not_state_shadow(val_name)?;
                        local_vars.insert(val_name.clone(), v_ty);
                    }
                    let body_t =
                        analyze_block(body, &mut local_vars, expected_ret, loop_depth + 1)?;
                    let start = match &args[1] {
                        Expr::Number(n) if *n >= 0 => *n as usize,
                        _ => {
                            return Err(SemanticError{ message: "E_UNBOUNDED_ITERATION: `.range(start, end)` requires non-negative integer literals for now".into() });
                        }
                    };
                    // Interpret second numeric as end; compute n = end - start
                    let end = match &args[2] {
                        Expr::Number(n) if *n >= 0 => *n as usize,
                        _ => {
                            return Err(SemanticError{ message: "E_UNBOUNDED_ITERATION: `.range(start, end)` requires non-negative integer literals for now".into() });
                        }
                    };
                    if end < start {
                        return Err(SemanticError {
                            message:
                                "E_UNBOUNDED_ITERATION: `.range(start, end)` requires end >= start"
                                    .into(),
                        });
                    }
                    if !map_expr_is_state(&args[0]) && (start != 0 || (end - start) > 1) {
                        return Err(SemanticError {
                            message: "E_MAP_BOUNDS: in-memory Map iteration supports at most 1 element starting at index 0; reduce the range or move the map into `state`."
                                .into(),
                        });
                    }
                    let static_bound = Some(end - start);
                    if let Expr::Ident(map_name) = &args[0]
                        && block_mutates_map(&body_t, map_name)
                    {
                        return Err(SemanticError { message: "E_ITER_MUTATION: structural modifications to the iterated map are forbidden during iteration".into() });
                    }
                    return Ok(vec![TypedStatement::ForEachMap {
                        key: key.clone(),
                        value: value.clone(),
                        map: base_map,
                        body: body_t,
                        start,
                        bound: static_bound,
                        #[cfg(feature = "kotodama_dynamic_bounds")]
                        dyn_count: None,
                        #[cfg(feature = "kotodama_dynamic_bounds")]
                        dyn_start: None,
                    }]);
                }
                #[cfg(feature = "kotodama_dynamic_bounds")]
                if name == "range" && args.len() == 3 {
                    // Dynamic: accept non-literal bounds with runtime asserts; lower to cap of 2 from start=0
                    if !map_expr_is_state(&args[0]) {
                        return Err(SemanticError {
                            message: "E_MAP_BOUNDS: dynamic bounds on in-memory Map iteration are unsupported; move the map into `state`."
                                .into(),
                        });
                    }
                    let base_map = analyze_expr(&args[0], &mut vars.clone())?;
                    ensure_state_map_iter_supported(&base_map)?;
                    ensure_in_memory_map_word_types(&base_map)?;
                    let mut local_vars = vars.clone();
                    let (k_ty, v_ty) = match &base_map.ty {
                        Type::Map(k, v) => ((**k).clone(), (**v).clone()),
                        _ => (Type::Int, Type::Int),
                    };
                    local_vars.insert(key.clone(), k_ty);
                    if let Some(val_name) = value {
                        local_vars.insert(val_name.clone(), v_ty);
                    }
                    let body_t =
                        analyze_block(body, &mut local_vars, expected_ret, loop_depth + 1)?;
                    let start_t = analyze_expr(&args[1], &mut vars.clone())?;
                    let end_t = analyze_expr(&args[2], &mut vars.clone())?;
                    // assert(start >= 0 && end >= start && end - start <= 2)
                    let zero = TypedExpr {
                        expr: ExprKind::Number(0),
                        ty: Type::Int,
                    };
                    let ge0 = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Ge,
                            left: Box::new(start_t.clone()),
                            right: Box::new(zero.clone()),
                        },
                        ty: Type::Bool,
                    };
                    let ge_start = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Ge,
                            left: Box::new(end_t.clone()),
                            right: Box::new(start_t.clone()),
                        },
                        ty: Type::Bool,
                    };
                    let diff = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Sub,
                            left: Box::new(end_t.clone()),
                            right: Box::new(start_t.clone()),
                        },
                        ty: Type::Int,
                    };
                    let two = TypedExpr {
                        expr: ExprKind::Number(2),
                        ty: Type::Int,
                    };
                    let le2 = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Le,
                            left: Box::new(diff),
                            right: Box::new(two),
                        },
                        ty: Type::Bool,
                    };
                    let and1 = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::And,
                            left: Box::new(ge0),
                            right: Box::new(ge_start),
                        },
                        ty: Type::Bool,
                    };
                    let cond = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::And,
                            left: Box::new(and1),
                            right: Box::new(le2),
                        },
                        ty: Type::Bool,
                    };
                    let assert_stmt = TypedStatement::Expr(TypedExpr {
                        expr: ExprKind::Call {
                            name: "assert".into(),
                            args: vec![cond],
                        },
                        ty: Type::Unit,
                    });
                    // Use dynamic n = end - start (computed in assert cond path)
                    let diff = TypedExpr {
                        expr: ExprKind::Binary {
                            op: BinaryOp::Sub,
                            left: Box::new(end_t.clone()),
                            right: Box::new(start_t.clone()),
                        },
                        ty: Type::Int,
                    };
                    // For now, dynamic start for loading buckets is not supported (requires dynamic addressing).
                    // Enforce start == 0 at compile time if literal; otherwise proceed with start=0 and guarded counts only.
                    return Ok(vec![
                        assert_stmt,
                        TypedStatement::ForEachMap {
                            key: key.clone(),
                            value: value.clone(),
                            map: base_map,
                            body: body_t,
                            start: 0,
                            bound: None,
                            dyn_count: Some(diff),
                            dyn_start: Some(start_t),
                        },
                    ]);
                }
            }
            Err(SemanticError {
                message: "E_UNBOUNDED_ITERATION: `for (k, v) in map` requires explicit bounds. Use `#[bounded(N)]` or call `.take(N)`/`.range(...)` on the map expression.".into(),
            })
        }
    }
}

fn analyze_expr(expr: &Expr, vars: &mut HashMap<String, Type>) -> Result<TypedExpr, SemanticError> {
    match expr {
        Expr::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            let c = analyze_expr(cond, vars)?;
            if c.ty != Type::Bool {
                return Err(SemanticError {
                    message: "conditional expects a bool condition".into(),
                });
            }
            let t1 = analyze_expr(then_expr, vars)?;
            let t2 = analyze_expr(else_expr, vars)?;
            if t1.ty != t2.ty {
                return Err(SemanticError {
                    message: "conditional branches must have the same type".into(),
                });
            }
            Ok(TypedExpr {
                expr: ExprKind::Conditional {
                    cond: Box::new(c),
                    then_expr: Box::new(t1.clone()),
                    else_expr: Box::new(t2.clone()),
                },
                ty: t1.ty,
            })
        }
        Expr::Tuple(elems) => {
            let mut typed = Vec::new();
            for e in elems {
                typed.push(analyze_expr(e, vars)?);
            }
            let tys = typed.iter().map(|t| t.ty.clone()).collect();
            Ok(TypedExpr {
                expr: ExprKind::Tuple(typed),
                ty: Type::Tuple(tys),
            })
        }
        Expr::Number(n) => Ok(TypedExpr {
            expr: ExprKind::Number(*n),
            ty: Type::Int,
        }),
        Expr::Bool(b) => Ok(TypedExpr {
            expr: ExprKind::Bool(*b),
            ty: Type::Bool,
        }),
        Expr::String(s) => Ok(TypedExpr {
            expr: ExprKind::String(s.clone()),
            ty: Type::String,
        }),
        Expr::Ident(name) => {
            let ty = vars.get(name).cloned().ok_or_else(|| SemanticError {
                message: format!("undefined variable {name}"),
            })?;
            Ok(TypedExpr {
                expr: ExprKind::Ident(name.clone()),
                ty,
            })
        }
        Expr::Unary { op, expr: inner } => {
            let inner_t = analyze_expr(inner, vars)?;
            match op {
                UnaryOp::Neg => {
                    if inner_t.ty != Type::Int {
                        return Err(SemanticError {
                            message: "unary '-' expects int".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Unary {
                            op: *op,
                            expr: Box::new(inner_t.clone()),
                        },
                        ty: Type::Int,
                    })
                }
                UnaryOp::Not => {
                    if inner_t.ty != Type::Bool {
                        return Err(SemanticError {
                            message: "unary '!' expects bool".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Unary {
                            op: *op,
                            expr: Box::new(inner_t.clone()),
                        },
                        ty: Type::Bool,
                    })
                }
            }
        }
        Expr::Member { object, field } => {
            let mut obj = analyze_expr(object, vars)?;
            let resolved_obj_ty = resolve_struct_type(&obj.ty);
            obj.ty = resolved_obj_ty.clone();
            // Tuple numeric indexing
            if let Ok(idx) = field.parse::<usize>() {
                match &resolved_obj_ty {
                    Type::Tuple(ts) => {
                        if let Some(t) = ts.get(idx) {
                            return Ok(TypedExpr {
                                expr: ExprKind::Member {
                                    object: Box::new(obj),
                                    field: field.clone(),
                                },
                                ty: resolve_struct_type(t),
                            });
                        } else {
                            return Err(SemanticError {
                                message: format!(
                                    "tuple index {} out of bounds (len={})",
                                    idx,
                                    ts.len()
                                ),
                            });
                        }
                    }
                    Type::Struct { name, .. } => {
                        return Err(SemanticError {
                            message: format!(
                                "tuple index on non-tuple type struct {name}; unknown field '{field}' on struct {name}"
                            ),
                        });
                    }
                    other => {
                        return Err(SemanticError {
                            message: format!("tuple index on non-tuple type {}", type_name(other)),
                        });
                    }
                }
            }
            // Named access on a tuple is invalid
            if matches!(&resolved_obj_ty, Type::Tuple(_)) {
                return Err(SemanticError {
                    message: format!("unknown field '{field}' on tuple"),
                });
            }
            // Struct named field: map to numeric index for lowering
            if let Type::Struct { name, fields } = &resolved_obj_ty {
                if let Some((idx, (_fname, fty))) = fields
                    .iter()
                    .enumerate()
                    .find(|(_, (fname, _))| fname == field)
                {
                    return Ok(TypedExpr {
                        expr: ExprKind::Member {
                            object: Box::new(obj),
                            field: idx.to_string(),
                        },
                        ty: resolve_struct_type(fty),
                    });
                } else {
                    let avail: Vec<&str> = fields.iter().map(|(f, _)| f.as_str()).collect();
                    return Err(SemanticError {
                        message: format!(
                            "unknown field '{field}' on struct {name} (available: {})",
                            avail.join(", ")
                        ),
                    });
                }
            }
            // Attempt to resolve to a flattened bound variable like `base#i#j` for nested structs.
            let try_flatten = || -> Option<(String, Type)> {
                fn collect_path(e: &TypedExpr, out: &mut Vec<usize>) -> Option<String> {
                    match &e.expr {
                        ExprKind::Member { object, field } => {
                            let base = collect_path(object, out)?;
                            let i = field.parse::<usize>().ok()?;
                            out.push(i);
                            Some(base)
                        }
                        ExprKind::Ident(nm) => Some(nm.clone()),
                        _ => None,
                    }
                }
                let mut path = Vec::new();
                let base = collect_path(
                    &TypedExpr {
                        expr: ExprKind::Member {
                            object: Box::new(obj.clone()),
                            field: field.clone(),
                        },
                        ty: Type::Int,
                    },
                    &mut path,
                )?;
                if path.is_empty() {
                    return None;
                }
                let mut name = base;
                for i in path.into_iter().rev() {
                    name.push('#');
                    name.push_str(&i.to_string());
                }
                // Look up type from vars
                if let Some(ty) = vars.get(&name).cloned() {
                    return Some((name, ty));
                }
                None
            };
            if let Some((n, ty)) = try_flatten() {
                return Ok(TypedExpr {
                    expr: ExprKind::Ident(n),
                    ty,
                });
            }
            Err(SemanticError {
                message: format!("unknown field '{field}' on type {}", type_name(&obj.ty)),
            })
        }
        Expr::Index { target, index } => {
            let tgt = analyze_expr(target, vars)?;
            let idx = analyze_expr(index, vars)?;
            match tgt.ty.clone() {
                Type::Map(k, v) => {
                    ensure_assignable(&k, &idx.ty)?;
                    ensure_in_memory_map_word_types(&tgt)?;
                    Ok(TypedExpr {
                        expr: ExprKind::Index {
                            target: Box::new(tgt),
                            index: Box::new(idx),
                        },
                        ty: *v,
                    })
                }
                _ => Err(SemanticError {
                    message: "indexing not supported on this type".into(),
                }),
            }
        }
        Expr::Binary { op, left, right } => {
            let left_t = analyze_expr(left, vars)?;
            let right_t = analyze_expr(right, vars)?;
            use BinaryOp::*;
            match op {
                Add | Sub | Mul | Div | Mod => {
                    if ensure_assignable(&Type::Int, &left_t.ty).is_err()
                        || ensure_assignable(&Type::Int, &right_t.ty).is_err()
                    {
                        return Err(SemanticError {
                            message: format!("{op:?} expects int operands"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Int,
                    })
                }
                And | Or => {
                    if left_t.ty != Type::Bool || right_t.ty != Type::Bool {
                        return Err(SemanticError {
                            message: format!("{op:?} expects bool operands"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Bool,
                    })
                }
                Eq | Ne => {
                    if left_t.ty != right_t.ty
                        && !(is_blob_like(&left_t.ty) && is_blob_like(&right_t.ty))
                    {
                        return Err(SemanticError {
                            message: "type mismatch in equality".into(),
                        });
                    }
                    if !is_eq_comparable_type(&left_t.ty) {
                        return Err(SemanticError {
                            message: format!(
                                "equality is not supported for type {}",
                                type_name(&left_t.ty)
                            ),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Bool,
                    })
                }
                Lt | Le | Gt | Ge => {
                    if ensure_assignable(&Type::Int, &left_t.ty).is_err()
                        || ensure_assignable(&Type::Int, &right_t.ty).is_err()
                    {
                        return Err(SemanticError {
                            message: format!("{op:?} expects int operands"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Binary {
                            op: *op,
                            left: Box::new(left_t),
                            right: Box::new(right_t),
                        },
                        ty: Type::Bool,
                    })
                }
            }
        }
        Expr::Call { name, args } => {
            let name = normalize_namespaced(name);
            // Special-case contract-style `call <callee>(...)` first, then handle regular builtins.
            if name == "call" {
                if args.is_empty() {
                    return Err(SemanticError {
                        message: "call expects a callee".into(),
                    });
                }
                // The callee must be an identifier like `transfer_asset` or `set_account_detail`.
                let callee = match &args[0] {
                    Expr::Ident(s) => s.clone(),
                    _ => {
                        return Err(SemanticError {
                            message: "call expects identifier callee".into(),
                        });
                    }
                };
                let mut arg_typed = Vec::new();
                for a in &args[1..] {
                    arg_typed.push(analyze_expr(a, vars)?);
                }

                // Re-use normal builtin typing by matching on the resolved callee name.
                return match callee.as_str() {
                    // First slice: translate to existing builtins
                    "set_account_detail" => {
                        if arg_typed.len() != 3
                            || !(arg_typed[0].ty == Type::AccountId
                                && arg_typed[1].ty == Type::Name
                                && arg_typed[2].ty == Type::Json)
                        {
                            return Err(SemanticError {
                                message: "set_account_detail expects (AccountId, Name, Json)"
                                    .into(),
                            });
                        }
                        Ok(TypedExpr {
                            expr: ExprKind::Call {
                                name: callee,
                                args: arg_typed,
                            },
                            ty: Type::Unit,
                        })
                    }
                    "transfer_asset" => {
                        if arg_typed.len() != 4
                            || !(arg_typed[0].ty == Type::AccountId
                                && arg_typed[1].ty == Type::AccountId
                                && arg_typed[2].ty == Type::AssetDefinitionId
                                && arg_typed[3].ty == Type::Int)
                        {
                            return Err(SemanticError {
                                message: "transfer_asset expects (AccountId, AccountId, AssetDefinitionId, int)".into(),
                            });
                        }
                        Ok(TypedExpr {
                            expr: ExprKind::Call {
                                name: callee,
                                args: arg_typed,
                            },
                            ty: Type::Unit,
                        })
                    }
                    "transfer_v1_batch_begin" | "transfer_v1_batch_end" => {
                        if !arg_typed.is_empty() {
                            return Err(SemanticError {
                                message: format!("{callee} expects ()"),
                            });
                        }
                        Ok(TypedExpr {
                            expr: ExprKind::Call {
                                name: callee,
                                args: arg_typed,
                            },
                            ty: Type::Unit,
                        })
                    }
                    "transfer_batch" => {
                        ensure_transfer_batch_args(&arg_typed)?;
                        Ok(TypedExpr {
                            expr: ExprKind::Call {
                                name: callee,
                                args: arg_typed,
                            },
                            ty: Type::Unit,
                        })
                    }
                    // Fallback: user-defined calls use recorded return types when available.
                    other => {
                        if is_user_defined_function(other)
                            && arg_typed.iter().any(is_state_map_expr)
                        {
                            return Err(SemanticError {
                                message:
                                    "E_STATE_MAP_ALIAS: state maps cannot be passed to user-defined functions; use the state identifier directly."
                                        .into(),
                            });
                        }
                        let ret_ty = FUNCTION_RETURNS
                            .with(|env| env.borrow().get(other).cloned())
                            .unwrap_or(Type::Int);
                        Ok(TypedExpr {
                            expr: ExprKind::Call {
                                name: other.to_string(),
                                args: arg_typed,
                            },
                            ty: ret_ty,
                        })
                    }
                };
            }

            // Struct constructor call: `StructName(arg1, arg2, ...)`
            if let Some(fields) = STRUCT_ENV.with(|env| env.borrow().get(&name).cloned()) {
                let mut arg_typed = Vec::new();
                for a in args {
                    arg_typed.push(analyze_expr(a, vars)?);
                }
                if arg_typed.len() != fields.len() {
                    return Err(SemanticError {
                        message: format!(
                            "{} expects {} fields, got {}",
                            name,
                            fields.len(),
                            arg_typed.len()
                        ),
                    });
                }
                for (i, (_fname, fty)) in fields.iter().enumerate() {
                    ensure_assignable(fty, &arg_typed[i].ty)?;
                }
                return Ok(TypedExpr {
                    expr: ExprKind::Tuple(arg_typed),
                    ty: Type::Struct {
                        name: name.clone(),
                        fields,
                    },
                });
            }

            // analyze builtin calls
            let mut arg_typed = Vec::new();
            for a in args {
                arg_typed.push(analyze_expr(a, vars)?);
            }
            match name.as_str() {
                // get_or_default(Map<int,int>, int, int) -> int (utility)
                "get_or_default" => {
                    if arg_typed.len() != 3 {
                        return Err(SemanticError {
                            message: "get_or_default expects (Map<int,int>, int, int)".into(),
                        });
                    }
                    match &arg_typed[0].ty {
                        Type::Map(k, v) if matches!(**k, Type::Int) && matches!(**v, Type::Int) => {
                        }
                        other => {
                            return Err(SemanticError {
                                message: format!(
                                    "get_or_default expects Map<int,int> as first arg, got {}",
                                    type_name(other)
                                ),
                            });
                        }
                    }
                    if arg_typed[1].ty != Type::Int || arg_typed[2].ty != Type::Int {
                        return Err(SemanticError {
                            message: "get_or_default expects (Map<int,int>, int, int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                // has(Map<K,V>, K) -> bool (alias to contains)
                "has" => {
                    if arg_typed.len() != 2 {
                        return Err(SemanticError {
                            message: "has expects (Map<K,V>, K)".into(),
                        });
                    }
                    match &arg_typed[0].ty {
                        Type::Map(k, _v) => {
                            ensure_assignable(&k.clone(), &arg_typed[1].ty)?;
                            ensure_in_memory_map_word_types(&arg_typed[0])?;
                            Ok(TypedExpr {
                                expr: ExprKind::Call {
                                    name: name.clone(),
                                    args: arg_typed,
                                },
                                ty: Type::Bool,
                            })
                        }
                        other => Err(SemanticError {
                            message: format!(
                                "has expects Map<K,V> as first arg, got {}",
                                type_name(other)
                            ),
                        }),
                    }
                }
                // Map contains helper: contains(Map<K,V>, K) -> bool
                "contains" => {
                    if arg_typed.len() != 2 {
                        return Err(SemanticError {
                            message: "contains expects (Map<K,V>, K)".into(),
                        });
                    }
                    match &arg_typed[0].ty {
                        Type::Map(k, _v) => {
                            ensure_assignable(&k.clone(), &arg_typed[1].ty)?;
                            ensure_in_memory_map_word_types(&arg_typed[0])?;
                            Ok(TypedExpr {
                                expr: ExprKind::Call {
                                    name: name.clone(),
                                    args: arg_typed,
                                },
                                ty: Type::Bool,
                            })
                        }
                        other => Err(SemanticError {
                            message: format!(
                                "contains expects Map<K,V> as first arg, got {}",
                                type_name(other)
                            ),
                        }),
                    }
                }
                // get_or_insert_default(Map<int,int>, int) -> int
                // Deterministic lowering: durable path uses STATE_GET/SET with Norito-encoded 0 when missing;
                // ephemeral path compares and inserts 0 on mismatch.
                "get_or_insert_default" => {
                    let original_len = arg_typed.len();
                    if original_len != 2 && original_len != 3 {
                        return Err(SemanticError {
                            message: "get_or_insert_default expects (Map<K,V>, K[, V])".into(),
                        });
                    }
                    let mut call_args = arg_typed;
                    let map_ty = resolve_struct_type(&call_args[0].ty);
                    let (map_key_ty, map_value_ty) = match map_ty {
                        Type::Map(k, v) => (*k, *v),
                        other => {
                            return Err(SemanticError {
                                message: format!(
                                    "get_or_insert_default expects Map<K,V> as first arg, got {}",
                                    type_name(&other)
                                ),
                            });
                        }
                    };
                    let resolved_key_ty = resolve_struct_type(&map_key_ty);
                    let resolved_value_ty = resolve_struct_type(&map_value_ty);
                    ensure_assignable(&resolved_key_ty, &call_args[1].ty)?;
                    ensure_in_memory_map_word_types(&call_args[0])?;

                    if original_len == 2 {
                        match resolve_struct_type(&resolved_value_ty) {
                            Type::Int => {
                                call_args.push(TypedExpr {
                                    expr: ExprKind::Number(0),
                                    ty: Type::Int,
                                });
                            }
                            other => {
                                if is_pointer_type(&other) {
                                    return Err(SemanticError {
                                        message: format!(
                                            "get_or_insert_default requires an explicit default for pointer-valued maps (value type {})",
                                            type_name(&other)
                                        ),
                                    });
                                }
                                return Err(SemanticError {
                                    message: format!(
                                        "get_or_insert_default auto-default is only available for Map<*,int>; provide an explicit default for value type {}",
                                        type_name(&other)
                                    ),
                                });
                            }
                        }
                    } else {
                        ensure_assignable(&resolved_value_ty, &call_args[2].ty)?;
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: call_args,
                        },
                        ty: resolved_value_ty,
                    })
                }
                // keys_take2(Map<int,int>, start:int, which:int{0,1}) -> int
                // values_take2(Map<int,int>, start:int, which:int{0,1}) -> int
                // Fixed-cap guarded iterator helpers returning a single element selected by `which`.
                "keys_take2" | "values_take2" => {
                    if arg_typed.len() != 3 {
                        return Err(SemanticError {
                            message: format!("{name} expects (Map<int,int>, int start, int which)"),
                        });
                    }
                    match &arg_typed[0].ty {
                        Type::Map(k, v) if matches!(**k, Type::Int) && matches!(**v, Type::Int) => {
                        }
                        other => {
                            return Err(SemanticError {
                                message: format!(
                                    "{name} expects Map<int,int> as first arg, got {}",
                                    type_name(other)
                                ),
                            });
                        }
                    }
                    if arg_typed[1].ty != Type::Int || arg_typed[2].ty != Type::Int {
                        return Err(SemanticError {
                            message: format!("{name} expects (Map<int,int>, int, int)"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                // keys_values_take2(Map<int,int>, start:int, which:int{0,1}) -> (int, int)
                "keys_values_take2" => {
                    if arg_typed.len() != 3 {
                        return Err(SemanticError {
                            message: "keys_values_take2 expects (Map<int,int>, int, int)".into(),
                        });
                    }
                    match &arg_typed[0].ty {
                        Type::Map(k, v) if matches!(**k, Type::Int) && matches!(**v, Type::Int) => {
                        }
                        other => {
                            return Err(SemanticError {
                                message: format!(
                                    "keys_values_take2 expects Map<int,int> as first arg, got {}",
                                    type_name(other)
                                ),
                            });
                        }
                    }
                    if arg_typed[1].ty != Type::Int || arg_typed[2].ty != Type::Int {
                        return Err(SemanticError {
                            message: "keys_values_take2 expects (Map<int,int>, int, int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Tuple(vec![Type::Int, Type::Int]),
                    })
                }
                // Durable state host helpers (pointer‑ABI):
                // state_get(Name) -> Blob; state_set(Name, NoritoBytes) -> unit; state_del(Name) -> unit
                "state_get" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                        return Err(SemanticError {
                            message: "state_get expects (Name)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "state_set" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::Name && is_blob_like(&arg_typed[1].ty))
                    {
                        return Err(SemanticError {
                            message: "state_set expects (Name, Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "state_del" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                        return Err(SemanticError {
                            message: "state_del expects (Name)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "path_map_key" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::Name && arg_typed[1].ty == Type::Int)
                    {
                        return Err(SemanticError {
                            message: "path_map_key expects (Name, int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Name,
                    })
                }
                "path_map_key_norito" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::Name && is_blob_like(&arg_typed[1].ty))
                    {
                        return Err(SemanticError {
                            message: "path_map_key_norito expects (Name, Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Name,
                    })
                }
                // ZK verify intrinsics: expect one Blob argument (pointer to INPUT Norito TLV)
                "zk_verify_transfer"
                | "zk_verify_unshield"
                | "zk_verify_batch"
                | "zk_vote_verify_ballot"
                | "zk_vote_verify_tally" => {
                    if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                        return Err(SemanticError {
                            message: format!(
                                "{name} expects (Blob|bytes) where the argument is a pointer to NoritoBytes TLV in INPUT"
                            ),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Vendor bridge helpers: enqueue ISI via SMARTCONTRACT_EXECUTE_INSTRUCTION with NoritoBytes TLV pointer
                "sc_execute_submit_ballot" | "sc_execute_unshield" | "execute_instruction" => {
                    if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                        return Err(SemanticError {
                            message: format!(
                                "{name} expects (Blob|bytes) where the argument is a pointer to NoritoBytes TLV in INPUT"
                            ),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "Map::new" => {
                    if !arg_typed.is_empty() {
                        return Err(SemanticError {
                            message: "Map::new expects no arguments".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Map(Box::new(Type::Int), Box::new(Type::Int)),
                    })
                }
                // Typed constructors for pointer-ABI arguments
                "account_id" | "asset_definition" | "asset_id" | "nft_id" | "name" | "json"
                | "domain" | "domain_id" | "blob" | "norito_bytes" | "dataspace_id"
                | "axt_descriptor" | "asset_handle" | "proof_blob" => {
                    if arg_typed.len() != 1 {
                        return Err(SemanticError {
                            message: format!("{name} expects one argument"),
                        });
                    }
                    let arg_ty = resolve_struct_type(&arg_typed[0].ty);
                    if matches!(arg_ty, Type::String)
                        && !matches!(arg_typed[0].expr, ExprKind::String(_))
                    {
                        return Err(SemanticError {
                            message: format!(
                                "{name} expects a string literal; pass a literal or Blob|bytes"
                            ),
                        });
                    }
                    let (ty, allow_blob, allow_int) = match name.as_str() {
                        "account_id" => (Type::AccountId, true, false),
                        "asset_definition" => (Type::AssetDefinitionId, true, false),
                        "asset_id" => (Type::AssetId, true, false),
                        "nft_id" => (Type::NftId, true, false),
                        "domain" | "domain_id" => (Type::DomainId, true, false),
                        "name" => (Type::Name, true, false),
                        "json" => (Type::Json, true, false),
                        "blob" | "norito_bytes" => (Type::Bytes, true, false),
                        "dataspace_id" => (Type::DataSpaceId, true, false),
                        "axt_descriptor" => (Type::AxtDescriptor, true, false),
                        "asset_handle" => (Type::AssetHandle, true, false),
                        "proof_blob" => (Type::ProofBlob, true, false),
                        _ => unreachable!(),
                    };
                    let ok = matches!(arg_ty, Type::String)
                        || (allow_int && matches!(arg_ty, Type::Int))
                        || arg_ty == ty
                        || (allow_blob && is_blob_like(&arg_ty))
                        || (matches!(arg_ty, Type::Json) && matches!(ty, Type::Json))
                        || (matches!(arg_ty, Type::Name) && matches!(ty, Type::Name));
                    if !ok {
                        let expected = match name.as_str() {
                            "json" => "string, Json, or Blob (NoritoBytes)",
                            "name" => "string, Name, or Blob (NoritoBytes)",
                            "blob" | "norito_bytes" => "string or Blob|bytes",
                            "dataspace_id" => "string, DataSpaceId, or Blob|bytes (NoritoBytes)",
                            _ => "string, matching pointer type, or Blob|bytes (NoritoBytes)",
                        };
                        return Err(SemanticError {
                            message: format!("{name} expects {expected}"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty,
                    })
                }
                // Current authority account id (contextual)
                "authority" => {
                    if !arg_typed.is_empty() {
                        return Err(SemanticError {
                            message: "authority expects no arguments".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: vec![],
                        },
                        ty: Type::AccountId,
                    })
                }
                "assert" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Bool {
                        return Err(SemanticError {
                            message: "assert expects (bool)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "info" => {
                    if arg_typed.len() != 1
                        || !(arg_typed[0].ty == Type::String || arg_typed[0].ty == Type::Int)
                    {
                        return Err(SemanticError {
                            message: "info expects (string|int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "call" => Ok(TypedExpr {
                    expr: ExprKind::Call {
                        name: name.clone(),
                        args: arg_typed,
                    },
                    ty: Type::Unit,
                }),
                "min" | "max" | "mean" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::Int
                        || arg_typed[1].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: format!("{name} expects (int, int)"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "abs" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Int {
                        return Err(SemanticError {
                            message: "abs expects (int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "div_ceil" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::Int
                        || arg_typed[1].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: "div_ceil expects (int, int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "gcd" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::Int
                        || arg_typed[1].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: "gcd expects (int, int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "isqrt" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Int {
                        return Err(SemanticError {
                            message: "isqrt expects (int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "poseidon2" => {
                    if arg_typed.len() != 2 || !arg_typed.iter().all(|t| t.ty == Type::Int) {
                        return Err(SemanticError {
                            message: "poseidon2 expects two int args".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "poseidon6" => {
                    if arg_typed.len() != 6 || !arg_typed.iter().all(|t| t.ty == Type::Int) {
                        return Err(SemanticError {
                            message: "poseidon6 expects six int args".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "sm3_hash" => {
                    if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                        return Err(SemanticError {
                            message: "sm3_hash expects (Blob|bytes) argument pointing to INPUT TLV"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "sm2_verify" => {
                    if arg_typed.len() != 3 && arg_typed.len() != 4 {
                        return Err(SemanticError {
                            message: "sm2_verify expects (Blob, Blob, Blob) or (Blob, Blob, Blob, Blob) where arguments reference INPUT TLVs".into(),
                        });
                    }
                    if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty)) {
                        return Err(SemanticError {
                            message: "sm2_verify expects message, signature, and public key as Blob|bytes pointers".into(),
                        });
                    }
                    if arg_typed.len() == 4 && !is_blob_like(&arg_typed[3].ty) {
                        return Err(SemanticError {
                            message:
                                "sm2_verify optional distid must be provided as Blob|bytes pointer"
                                    .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bool,
                    })
                }
                "sm4_gcm_seal" => {
                    if arg_typed.len() != 4 || arg_typed.iter().any(|t| !is_blob_like(&t.ty)) {
                        return Err(SemanticError {
                            message: "sm4_gcm_seal expects (Blob|bytes, Blob|bytes, Blob|bytes, Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "sm4_gcm_open" => {
                    if arg_typed.len() != 4 || arg_typed.iter().any(|t| !is_blob_like(&t.ty)) {
                        return Err(SemanticError {
                            message: "sm4_gcm_open expects (Blob|bytes, Blob|bytes, Blob|bytes, Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "sm4_ccm_seal" => {
                    if arg_typed.len() != 4 && arg_typed.len() != 5 {
                        return Err(SemanticError {
                            message: "sm4_ccm_seal expects (Blob|bytes, Blob|bytes, Blob|bytes, Blob|bytes[, int])".into(),
                        });
                    }
                    if arg_typed[..4].iter().any(|t| !is_blob_like(&t.ty)) {
                        return Err(SemanticError {
                            message: "sm4_ccm_seal expects key, nonce, aad, plaintext as Blob|bytes pointers"
                                .into(),
                        });
                    }
                    if arg_typed.len() == 5 && arg_typed[4].ty != Type::Int {
                        return Err(SemanticError {
                            message: "sm4_ccm_seal optional tag length must be int".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "sm4_ccm_open" => {
                    if arg_typed.len() != 4 && arg_typed.len() != 5 {
                        return Err(SemanticError {
                            message: "sm4_ccm_open expects (Blob|bytes, Blob|bytes, Blob|bytes, Blob|bytes[, int])".into(),
                        });
                    }
                    if arg_typed[..4].iter().any(|t| !is_blob_like(&t.ty)) {
                        return Err(SemanticError {
                            message: "sm4_ccm_open expects key, nonce, aad, ciphertext as Blob|bytes pointers"
                                .into(),
                        });
                    }
                    if arg_typed.len() == 5 && arg_typed[4].ty != Type::Int {
                        return Err(SemanticError {
                            message: "sm4_ccm_open optional tag length must be int".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                // Encoding helpers for durable state values
                "encode_int" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Int {
                        return Err(SemanticError {
                            message: "encode_int expects (int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "decode_int" => {
                    if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                        return Err(SemanticError {
                            message: "decode_int expects (Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                // JSON helpers
                "encode_json" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                        return Err(SemanticError {
                            message: "encode_json expects (Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "decode_json" => {
                    if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                        return Err(SemanticError {
                            message: "decode_json expects (Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Json,
                    })
                }
                // Schema helpers (placeholder)
                "encode_schema" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::Name && arg_typed[1].ty == Type::Json)
                    {
                        return Err(SemanticError {
                            message: "encode_schema expects (Name, Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "decode_schema" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::Name && is_blob_like(&arg_typed[1].ty))
                    {
                        return Err(SemanticError {
                            message: "decode_schema expects (Name, Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Json,
                    })
                }
                "schema_info" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                        return Err(SemanticError {
                            message: "schema_info expects (Name)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Json,
                    })
                }
                "pointer_to_norito" => {
                    if arg_typed.len() != 1 {
                        return Err(SemanticError {
                            message: "pointer_to_norito expects one argument".into(),
                        });
                    }
                    let ty = resolve_struct_type(&arg_typed[0].ty);
                    if !(is_pointer_type(&ty) || is_blob_like(&ty)) {
                        return Err(SemanticError {
                            message: "pointer_to_norito expects a pointer-ABI type or Blob|bytes argument"
                                    .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "valcom" => {
                    if arg_typed.len() != 2 || !arg_typed.iter().all(|t| t.ty == Type::Int) {
                        return Err(SemanticError {
                            message: "valcom expects two int args".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "pubkgen" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Int {
                        return Err(SemanticError {
                            message: "pubkgen expects one int arg".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Int,
                    })
                }
                "vrf_verify" => {
                    if arg_typed.len() != 4 {
                        return Err(SemanticError {
                            message: "vrf_verify expects (Blob, Blob, Blob, int variant)".into(),
                        });
                    }
                    if arg_typed[..3].iter().any(|t| !is_blob_like(&t.ty))
                        || arg_typed[3].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: "vrf_verify expects (Blob|bytes, Blob|bytes, Blob|bytes, int variant)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "vrf_verify_batch" => {
                    if arg_typed.len() != 1 || !is_blob_like(&arg_typed[0].ty) {
                        return Err(SemanticError {
                            message: "vrf_verify_batch expects (Blob|bytes)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "axt_begin" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::AxtDescriptor {
                        return Err(SemanticError {
                            message: "axt_begin expects (AxtDescriptor)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "axt_touch" => {
                    if arg_typed.is_empty() || arg_typed.len() > 2 {
                        return Err(SemanticError {
                            message: "axt_touch expects (DataSpaceId[, Blob|bytes manifest])"
                                .into(),
                        });
                    }
                    if arg_typed[0].ty != Type::DataSpaceId {
                        return Err(SemanticError {
                            message: "axt_touch expects (DataSpaceId[, Blob|bytes manifest])"
                                .into(),
                        });
                    }
                    if arg_typed.len() == 2 && !is_blob_like(&arg_typed[1].ty) {
                        return Err(SemanticError {
                            message: "axt_touch expects (DataSpaceId[, Blob|bytes manifest])"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "verify_ds_proof" => {
                    if arg_typed.is_empty() || arg_typed.len() > 2 {
                        return Err(SemanticError {
                            message: "verify_ds_proof expects (DataSpaceId[, ProofBlob])".into(),
                        });
                    }
                    if arg_typed[0].ty != Type::DataSpaceId {
                        return Err(SemanticError {
                            message: "verify_ds_proof expects (DataSpaceId[, ProofBlob])".into(),
                        });
                    }
                    if arg_typed.len() == 2 && arg_typed[1].ty != Type::ProofBlob {
                        return Err(SemanticError {
                            message: "verify_ds_proof expects (DataSpaceId[, ProofBlob])".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "use_asset_handle" => {
                    if arg_typed.len() != 2 && arg_typed.len() != 3 {
                        return Err(SemanticError {
                            message:
                                "use_asset_handle expects (AssetHandle, Blob|bytes intent[, ProofBlob])"
                                    .into(),
                        });
                    }
                    if arg_typed[0].ty != Type::AssetHandle || !is_blob_like(&arg_typed[1].ty) {
                        return Err(SemanticError {
                            message:
                                "use_asset_handle expects (AssetHandle, Blob|bytes intent[, ProofBlob])"
                                    .into(),
                        });
                    }
                    if arg_typed.len() == 3 && arg_typed[2].ty != Type::ProofBlob {
                        return Err(SemanticError {
                            message:
                                "use_asset_handle expects (AssetHandle, Blob intent[, ProofBlob])"
                                    .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "axt_commit" => {
                    if !arg_typed.is_empty() {
                        return Err(SemanticError {
                            message: "axt_commit expects no arguments".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Host helper: create one NFT per known account (no args)
                "create_nfts_for_all_users" => {
                    if !arg_typed.is_empty() {
                        return Err(SemanticError {
                            message: "create_nfts_for_all_users expects no arguments".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Host helper: set SmartContract execution depth
                "set_execution_depth" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Int {
                        return Err(SemanticError {
                            message: "set_execution_depth expects one int arg".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Host helper: set account detail using typed args
                "set_account_detail" => {
                    if arg_typed.len() != 3
                        || !(arg_typed[0].ty == Type::AccountId
                            && arg_typed[1].ty == Type::Name
                            && arg_typed[2].ty == Type::Json)
                    {
                        return Err(SemanticError {
                            message: "set_account_detail expects (AccountId, Name, Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Builders: construct Norito InstructionBox in data section and return Blob pointer
                "build_submit_ballot_inline" => {
                    if arg_typed.len() != 6
                        || !(arg_typed[0].ty == Type::String
                            && is_blob_like(&arg_typed[1].ty)
                            && is_blob_like(&arg_typed[2].ty)
                            && arg_typed[3].ty == Type::String
                            && is_blob_like(&arg_typed[4].ty)
                            && is_blob_like(&arg_typed[5].ty))
                    {
                        eprintln!(
                            "[semantic] build_submit_ballot_inline arg types: {:?}",
                            arg_typed.iter().map(|a| a.ty.clone()).collect::<Vec<_>>()
                        );
                        return Err(SemanticError {
                            message: "build_submit_ballot_inline expects (string election_id, Blob|bytes ciphertext, Blob|bytes nullifier32, string backend, Blob|bytes proof, Blob|bytes vk)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "build_unshield_inline" => {
                    if arg_typed.len() != 7
                        || !(arg_typed[0].ty == Type::AssetDefinitionId
                            && arg_typed[1].ty == Type::AccountId
                            && arg_typed[2].ty == Type::Int
                            && is_blob_like(&arg_typed[3].ty)
                            && arg_typed[4].ty == Type::String
                            && is_blob_like(&arg_typed[5].ty)
                            && is_blob_like(&arg_typed[6].ty))
                    {
                        return Err(SemanticError {
                            message: "build_unshield_inline expects (AssetDefinitionId, AccountId, int amount, Blob|bytes inputs32, string backend, Blob|bytes proof, Blob|bytes vk)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Bytes,
                    })
                }
                "mint_asset" => {
                    if arg_typed.len() != 3
                        || !(arg_typed[0].ty == Type::AccountId
                            && arg_typed[1].ty == Type::AssetDefinitionId
                            && arg_typed[2].ty == Type::Int)
                    {
                        return Err(SemanticError {
                            message: "mint_asset expects (AccountId, AssetDefinitionId, int)"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "burn_asset" => {
                    if arg_typed.len() != 3
                        || !(arg_typed[0].ty == Type::AccountId
                            && arg_typed[1].ty == Type::AssetDefinitionId
                            && arg_typed[2].ty == Type::Int)
                    {
                        return Err(SemanticError {
                            message: "burn_asset expects (AccountId, AssetDefinitionId, int)"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "transfer_asset" => {
                    if arg_typed.len() != 4
                        || !(arg_typed[0].ty == Type::AccountId
                            && arg_typed[1].ty == Type::AccountId
                            && arg_typed[2].ty == Type::AssetDefinitionId
                            && arg_typed[3].ty == Type::Int)
                    {
                        return Err(SemanticError {
                            message:
                                "transfer_asset expects (AccountId, AccountId, AssetDefinitionId, int)"
                                    .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "transfer_v1_batch_begin" | "transfer_v1_batch_end" => {
                    if !arg_typed.is_empty() {
                        return Err(SemanticError {
                            message: format!("{name} expects ()"),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "transfer_batch" => {
                    ensure_transfer_batch_args(&arg_typed)?;
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "nft_mint_asset" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::NftId && arg_typed[1].ty == Type::AccountId)
                    {
                        return Err(SemanticError {
                            message: "nft_mint_asset expects (NftId, AccountId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "nft_set_metadata" => {
                    if arg_typed.len() != 2
                        || !(arg_typed[0].ty == Type::NftId && arg_typed[1].ty == Type::Json)
                    {
                        return Err(SemanticError {
                            message: "nft_set_metadata expects (NftId, Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "nft_burn_asset" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::NftId {
                        return Err(SemanticError {
                            message: "nft_burn_asset expects (NftId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "nft_transfer_asset" => {
                    if arg_typed.len() != 3
                        || !(arg_typed[0].ty == Type::AccountId
                            && arg_typed[1].ty == Type::NftId
                            && arg_typed[2].ty == Type::AccountId)
                    {
                        return Err(SemanticError {
                            message: "nft_transfer_asset expects (AccountId, NftId, AccountId)"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "register_asset" => {
                    if arg_typed.len() != 4
                        || arg_typed[0].ty != Type::String
                        || arg_typed[1].ty != Type::String
                        || arg_typed[2].ty != Type::Int
                        || arg_typed[3].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: "register_asset expects string, string, int, int".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "create_new_asset" => {
                    if arg_typed.len() != 5
                        || arg_typed[0].ty != Type::String
                        || arg_typed[1].ty != Type::String
                        || arg_typed[2].ty != Type::Int
                        || arg_typed[3].ty != Type::Int
                        || arg_typed[4].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: "create_new_asset expects string, string, int, int, int"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "assert_eq" => {
                    if arg_typed.len() != 2 || !arg_typed.iter().all(|t| t.ty == Type::Int) {
                        return Err(SemanticError {
                            message: "assert_eq expects two int args".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Peers
                "register_peer" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                        return Err(SemanticError {
                            message: "register_peer expects (Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "unregister_peer" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                        return Err(SemanticError {
                            message: "unregister_peer expects (Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Triggers
                "create_trigger" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Json {
                        return Err(SemanticError {
                            message: "create_trigger expects (Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "remove_trigger" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                        return Err(SemanticError {
                            message: "remove_trigger expects (Name)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "set_trigger_enabled" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::Name
                        || arg_typed[1].ty != Type::Int
                    {
                        return Err(SemanticError {
                            message: "set_trigger_enabled expects (Name, int)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Permissions
                "grant_permission" | "revoke_permission" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::AccountId
                        || !(arg_typed[1].ty == Type::Name || arg_typed[1].ty == Type::Json)
                    {
                        return Err(SemanticError {
                            message: "grant/revoke_permission expects (AccountId, Name|Json)"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                // Roles
                "create_role" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::Name
                        || arg_typed[1].ty != Type::Json
                    {
                        return Err(SemanticError {
                            message: "create_role expects (Name, Json)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "delete_role" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::Name {
                        return Err(SemanticError {
                            message: "delete_role expects (Name)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "grant_role" | "revoke_role" => {
                    if arg_typed.len() != 2
                        || arg_typed[0].ty != Type::AccountId
                        || arg_typed[1].ty != Type::Name
                    {
                        return Err(SemanticError {
                            message: "grant/revoke_role expects (AccountId, Name)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "register_domain" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::DomainId {
                        return Err(SemanticError {
                            message: "register_domain expects (DomainId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "register_account" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::AccountId {
                        return Err(SemanticError {
                            message: "register_account expects (AccountId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "unregister_account" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::AccountId {
                        return Err(SemanticError {
                            message: "unregister_account expects (AccountId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "unregister_asset" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::AssetDefinitionId {
                        return Err(SemanticError {
                            message: "unregister_asset expects (AssetDefinitionId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "unregister_domain" => {
                    if arg_typed.len() != 1 || arg_typed[0].ty != Type::DomainId {
                        return Err(SemanticError {
                            message: "unregister_domain expects (DomainId)".into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                "transfer_domain" => {
                    if arg_typed.len() != 3
                        || arg_typed[0].ty != Type::AccountId
                        || arg_typed[2].ty != Type::AccountId
                    {
                        return Err(SemanticError {
                            message: "transfer_domain expects (AccountId, DomainId, AccountId)"
                                .into(),
                        });
                    }
                    if !matches!(arg_typed[1].ty, Type::DomainId | Type::Name) {
                        return Err(SemanticError {
                            message: "transfer_domain expects (AccountId, DomainId, AccountId)"
                                .into(),
                        });
                    }
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: Type::Unit,
                    })
                }
                _ => {
                    if is_user_defined_function(&name) && arg_typed.iter().any(is_state_map_expr) {
                        return Err(SemanticError {
                            message:
                                "E_STATE_MAP_ALIAS: state maps cannot be passed to user-defined functions; use the state identifier directly."
                                    .into(),
                        });
                    }
                    let ret_ty = FUNCTION_RETURNS
                        .with(|env| env.borrow().get(&name).cloned())
                        .unwrap_or(Type::Int);
                    Ok(TypedExpr {
                        expr: ExprKind::Call {
                            name: name.clone(),
                            args: arg_typed,
                        },
                        ty: ret_ty,
                    })
                }
            }
        }
    }
}

fn parse_declared_type(ty: &Option<TypeExpr>) -> Result<Option<Type>, SemanticError> {
    let Some(t) = ty else { return Ok(None) };
    convert_type_expr(t).map(Some)
}

fn parse_declared_param_type(ty: &Option<TypeExpr>, _name: &str) -> Result<Type, SemanticError> {
    if let Some(t) = ty {
        convert_type_expr(t)
    } else {
        Ok(Type::Int)
    }
}

fn convert_type_expr(ty: &TypeExpr) -> Result<Type, SemanticError> {
    Ok(match ty {
        TypeExpr::Path(s) => match s.as_str() {
            "int" | "i64" | "number" => Type::Int,
            "bool" => Type::Bool,
            "string" | "String" => Type::String,
            "Blob" => Type::Blob,
            "bytes" | "Bytes" => Type::Bytes,
            "DataSpaceId" => Type::DataSpaceId,
            "AxtDescriptor" => Type::AxtDescriptor,
            "AssetHandle" => Type::AssetHandle,
            "ProofBlob" => Type::ProofBlob,
            "unit" | "()" => Type::Unit,
            // Recognize common Iroha types by name
            "AccountId" => Type::AccountId,
            "AssetDefinitionId" => Type::AssetDefinitionId,
            "AssetId" => Type::AssetId,
            "DomainId" => Type::DomainId,
            "Name" => Type::Name,
            "Json" => Type::Json,
            "NftId" => Type::NftId,
            other => Type::Opaque(other.to_string()),
        },
        TypeExpr::Generic { base, args } => {
            if base == "Map" {
                if args.len() != 2 {
                    return Err(SemanticError {
                        message: "Map expects two type parameters".into(),
                    });
                }
                let k = convert_type_expr(&args[0])?;
                let v = convert_type_expr(&args[1])?;
                Type::Map(Box::new(k), Box::new(v))
            } else {
                Type::Opaque(base.clone())
            }
        }
        TypeExpr::Tuple(elems) => {
            let mut out = Vec::new();
            for e in elems {
                out.push(convert_type_expr(e)?);
            }
            Type::Tuple(out)
        }
    })
}

fn apply_map_new_type_hint(expr: &mut TypedExpr, hint: &Type) {
    let hint = resolve_struct_type(hint);
    if !matches!(hint, Type::Map(_, _)) {
        return;
    }
    if let ExprKind::Call { name, .. } = &expr.expr
        && name == "Map::new"
    {
        expr.ty = hint;
    }
}

fn ensure_assignable(expected: &Type, actual: &Type) -> Result<(), SemanticError> {
    let expected = resolve_struct_type(expected);
    let actual = resolve_struct_type(actual);
    if expected == actual {
        return Ok(());
    }
    if is_blob_like(&expected) && is_blob_like(&actual) {
        return Ok(());
    }
    match (&expected, &actual) {
        (Type::Map(ek, ev), Type::Map(ak, av)) => {
            ensure_assignable(ek, ak)?;
            ensure_assignable(ev, av)
        }
        (Type::Tuple(exp_elems), Type::Tuple(act_elems)) => {
            if exp_elems.len() != act_elems.len() {
                return Err(SemanticError {
                    message: format!(
                        "type annotation mismatch: expected tuple of length {}, got {}",
                        exp_elems.len(),
                        act_elems.len()
                    ),
                });
            }
            for (e, a) in exp_elems.iter().zip(act_elems.iter()) {
                ensure_assignable(e, a)?;
            }
            Ok(())
        }
        // Allow using map pointers where raw ints are expected (pointer ABI).
        (Type::Int, Type::Map(_, _)) => Ok(()),
        // Booleans lower to 0/1; permit implicit promotion to int.
        (Type::Int, Type::Bool) => Ok(()),
        // Allow assigning opaque to opaque of any name (best-effort placeholder)
        (Type::Opaque(_), Type::Opaque(_)) => Ok(()),
        // Structs may be referenced by opaque name annotations in struct fields.
        (Type::Opaque(expected_name), Type::Struct { name, .. }) if expected_name == name => Ok(()),
        (Type::Struct { name, .. }, Type::Opaque(actual_name)) if name == actual_name => Ok(()),
        _ => Err(SemanticError {
            message: format!(
                "type annotation mismatch: expected {}, got {}",
                type_name(&expected),
                type_name(&actual)
            ),
        }),
    }
}

pub(crate) fn resolve_struct_type(ty: &Type) -> Type {
    if let Type::Opaque(name) = ty {
        STRUCT_ENV.with(|env| {
            env.borrow()
                .get(name)
                .map(|fields| Type::Struct {
                    name: name.clone(),
                    fields: fields.clone(),
                })
                .unwrap_or_else(|| ty.clone())
        })
    } else {
        ty.clone()
    }
}

fn normalize_namespaced(name: &str) -> String {
    // Map namespaced forms to existing builtin names used by semantics/IR
    if let Some(rest) = name.strip_prefix("host::") {
        return String::from(rest);
    }
    if name == "std::map::new" || name == "std::Map::new" {
        return String::from("Map::new");
    }
    if name == "std::map::contains" {
        return String::from("contains");
    }
    if name == "std::map::has" {
        return String::from("has");
    }
    if name == "std::map::get_or_insert_default" {
        return String::from("get_or_insert_default");
    }
    if name == "std::map::keys_take2" {
        return String::from("keys_take2");
    }
    if name == "std::map::values_take2" {
        return String::from("values_take2");
    }
    if name == "std::map::keys_values_take2" {
        return String::from("keys_values_take2");
    }
    if name == "Map::new" {
        return String::from(name);
    }
    if let Some(rest) = name.strip_prefix("sm::") {
        match rest {
            "hash" | "sm3_hash" => return String::from("sm3_hash"),
            "verify" | "verify_signature" | "verify_with_distid" => {
                return String::from("sm2_verify");
            }
            "seal_gcm" | "gcm_seal" | "sm4_gcm_seal" => {
                return String::from("sm4_gcm_seal");
            }
            "open_gcm" | "gcm_open" | "sm4_gcm_open" => {
                return String::from("sm4_gcm_open");
            }
            "seal_ccm" | "ccm_seal" | "sm4_ccm_seal" => {
                return String::from("sm4_ccm_seal");
            }
            "open_ccm" | "ccm_open" | "sm4_ccm_open" => {
                return String::from("sm4_ccm_open");
            }
            _ => {}
        }
    }
    if let Some(rest) = name.strip_prefix("zk::") {
        // Support zk::verify_transfer and zk::verify_unshield
        if rest == "verify_transfer" {
            return String::from("zk_verify_transfer");
        }
        if rest == "verify_unshield" {
            return String::from("zk_verify_unshield");
        }
        if rest == "verify_batch" {
            return String::from("zk_verify_batch");
        }
        // Support zk::vote::verify_ballot and zk::vote::verify_tally
        if let Some(v) = rest.strip_prefix("vote::") {
            match v {
                "verify_ballot" => return String::from("zk_vote_verify_ballot"),
                "verify_tally" => return String::from("zk_vote_verify_tally"),
                _ => {}
            }
        }
    }
    String::from(name)
}

// Return coverage and shape analysis on the AST (conservative)
fn block_returns_all_paths(block: &super::ast::Block) -> bool {
    let mut always = false;
    for stmt in &block.statements {
        if stmt_returns_all_paths(stmt) {
            always = true;
            break;
        }
    }
    always
}

fn stmt_returns_all_paths(stmt: &super::ast::Statement) -> bool {
    use super::ast::Statement as S;
    match stmt {
        S::Return(_) => true,
        S::If {
            then_branch,
            else_branch,
            ..
        } => else_branch
            .as_ref()
            .map(|else_b| block_returns_all_paths(then_branch) && block_returns_all_paths(else_b))
            .unwrap_or(false),
        S::While { .. } | S::For { .. } => false,
        _ => false,
    }
}

fn block_has_return_value(block: &super::ast::Block) -> bool {
    block.statements.iter().any(stmt_has_return_value)
}

fn stmt_has_return_value(stmt: &super::ast::Statement) -> bool {
    use super::ast::Statement as S;
    match stmt {
        S::Return(Some(_)) => true,
        S::If {
            then_branch,
            else_branch,
            ..
        } => {
            block_has_return_value(then_branch)
                || else_branch
                    .as_ref()
                    .map(block_has_return_value)
                    .unwrap_or(false)
        }
        S::While { body, .. } => block_has_return_value(body),
        S::For { body, .. } => block_has_return_value(body),
        S::ForEachMap { body, .. } => block_has_return_value(body),
        _ => false,
    }
}

// NOTE: `TypedProgram` is defined earlier in this file with contract metadata.

#[derive(Debug, PartialEq)]
pub enum TypedItem {
    Function(TypedFunction),
}

#[derive(Debug, PartialEq)]
pub struct TypedFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: TypedBlock,
    pub ret_ty: Option<Type>,
    pub modifiers: FunctionModifiers,
}

/// Snapshot the state environment (name -> type) collected during the latest
/// analysis pass. Used by IR lowering to allocate ephemeral storage for state
/// variables at function entry. Durable host-backed storage is pending.
pub fn state_env_snapshot() -> IndexMap<String, Type> {
    STATE_ENV.with(|env| env.borrow().clone())
}

#[derive(Debug, PartialEq)]
pub struct TypedBlock {
    pub statements: Vec<TypedStatement>,
}

#[derive(Debug, PartialEq)]
pub enum TypedStatement {
    Let {
        name: String,
        value: TypedExpr,
    },
    Expr(TypedExpr),
    Return(Option<TypedExpr>),
    Break,
    Continue,
    If {
        cond: TypedExpr,
        then_branch: TypedBlock,
        else_branch: Option<TypedBlock>,
    },
    While {
        cond: TypedExpr,
        body: TypedBlock,
    },
    For {
        line: usize,
        init: Option<Box<TypedStatement>>,
        cond: Option<TypedExpr>,
        step: Option<Box<TypedStatement>>,
        body: TypedBlock,
    },
    /// For-each over a map. Lowers to a deterministic, bounded iteration.
    ForEachMap {
        key: String,
        value: Option<String>,
        map: TypedExpr,
        body: TypedBlock,
        /// Start offset (in buckets) for iteration; 0 when not specified.
        start: usize,
        /// Optional upper bound on iterations (e.g., from `.take(n)`).
        bound: Option<usize>,
        /// Dynamic count expression (feature-gated). When present, IR lowering
        /// emits up to K guarded iterations with `i < n` checks.
        #[cfg(feature = "kotodama_dynamic_bounds")]
        dyn_count: Option<TypedExpr>,
        /// Dynamic start expression (feature-gated). When present, IR computes
        /// the address offset based on `start + i`.
        #[cfg(feature = "kotodama_dynamic_bounds")]
        dyn_start: Option<TypedExpr>,
    },
    /// Map set operation: `map[key] = value`.
    MapSet {
        map: TypedExpr,
        key: TypedExpr,
        value: TypedExpr,
    },
}

fn block_mutates_map(block: &TypedBlock, map_name: &str) -> bool {
    fn stmt_mutates(stmt: &TypedStatement, map_name: &str) -> bool {
        match stmt {
            TypedStatement::MapSet { map, .. } => {
                matches!(&map.expr, ExprKind::Ident(n) if n == map_name)
            }
            TypedStatement::If {
                then_branch,
                else_branch,
                ..
            } => {
                block_mutates_map(then_branch, map_name)
                    || else_branch
                        .as_ref()
                        .map(|b| block_mutates_map(b, map_name))
                        .unwrap_or(false)
            }
            TypedStatement::While { body, .. } => block_mutates_map(body, map_name),
            TypedStatement::For { body, .. } => block_mutates_map(body, map_name),
            TypedStatement::ForEachMap { body, .. } => block_mutates_map(body, map_name),
            _ => false,
        }
    }
    block.statements.iter().any(|s| stmt_mutates(s, map_name))
}

fn block_contains_sensitive_syscall(block: &TypedBlock) -> bool {
    block
        .statements
        .iter()
        .any(statement_contains_sensitive_syscall)
}

fn is_state_identifier(name: &str) -> bool {
    STATE_ENV.with(|env| env.borrow().contains_key(name))
}

fn canonical_state_hint(name: &str) -> String {
    let base = name.split('#').next().unwrap_or(name);
    format!("state:{base}")
}

fn mark_state_read(name: &str, reads: &mut IndexSet<String>) {
    if is_state_identifier(name) {
        reads.insert(canonical_state_hint(name));
    }
}

fn mark_state_write(name: &str, writes: &mut IndexSet<String>) {
    if is_state_identifier(name) {
        writes.insert(canonical_state_hint(name));
    }
}

fn collect_state_accesses_block(
    block: &TypedBlock,
    reads: &mut IndexSet<String>,
    writes: &mut IndexSet<String>,
) {
    for stmt in &block.statements {
        collect_state_accesses_statement(stmt, reads, writes);
    }
}

fn collect_state_accesses_statement(
    stmt: &TypedStatement,
    reads: &mut IndexSet<String>,
    writes: &mut IndexSet<String>,
) {
    match stmt {
        TypedStatement::Let { name, value } => {
            collect_state_accesses_expr(value, reads);
            mark_state_write(name, writes);
        }
        TypedStatement::Expr(expr) => collect_state_accesses_expr(expr, reads),
        TypedStatement::Return(Some(expr)) => collect_state_accesses_expr(expr, reads),
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_state_accesses_expr(cond, reads);
            collect_state_accesses_block(then_branch, reads, writes);
            if let Some(b) = else_branch {
                collect_state_accesses_block(b, reads, writes);
            }
        }
        TypedStatement::While { cond, body } => {
            collect_state_accesses_expr(cond, reads);
            collect_state_accesses_block(body, reads, writes);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init.as_deref() {
                collect_state_accesses_statement(init_stmt, reads, writes);
            }
            if let Some(cond_expr) = cond {
                collect_state_accesses_expr(cond_expr, reads);
            }
            if let Some(step_stmt) = step.as_deref() {
                collect_state_accesses_statement(step_stmt, reads, writes);
            }
            collect_state_accesses_block(body, reads, writes);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            collect_state_accesses_expr(map, reads);
            collect_state_accesses_block(body, reads, writes);
        }
        TypedStatement::MapSet { map, key, value } => {
            collect_state_accesses_expr(map, reads);
            collect_state_accesses_expr(key, reads);
            collect_state_accesses_expr(value, reads);
            if let ExprKind::Ident(name) = &map.expr {
                mark_state_write(name, writes);
            }
        }
    }
}

fn collect_state_accesses_expr(expr: &TypedExpr, reads: &mut IndexSet<String>) {
    match &expr.expr {
        ExprKind::Ident(name) => mark_state_read(name, reads),
        ExprKind::Binary { left, right, .. } => {
            collect_state_accesses_expr(left, reads);
            collect_state_accesses_expr(right, reads);
        }
        ExprKind::Unary { expr: inner, .. } => collect_state_accesses_expr(inner, reads),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_state_accesses_expr(cond, reads);
            collect_state_accesses_expr(then_expr, reads);
            collect_state_accesses_expr(else_expr, reads);
        }
        ExprKind::Tuple(items) => {
            for item in items {
                collect_state_accesses_expr(item, reads);
            }
        }
        ExprKind::Member { object, .. } => collect_state_accesses_expr(object, reads),
        ExprKind::Index { target, index } => {
            collect_state_accesses_expr(target, reads);
            collect_state_accesses_expr(index, reads);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                collect_state_accesses_expr(arg, reads);
            }
        }
        ExprKind::Bool(_) | ExprKind::Number(_) | ExprKind::String(_) => {}
    }
}

pub fn function_state_accesses(func: &TypedFunction) -> (IndexSet<String>, IndexSet<String>) {
    let mut reads = IndexSet::new();
    let mut writes = IndexSet::new();
    collect_state_accesses_block(&func.body, &mut reads, &mut writes);
    (reads, writes)
}

fn collect_called_functions(block: &TypedBlock) -> IndexSet<String> {
    let mut calls = IndexSet::new();
    collect_called_functions_into(block, &mut calls);
    calls
}

fn collect_called_functions_into(block: &TypedBlock, calls: &mut IndexSet<String>) {
    for stmt in &block.statements {
        collect_calls_in_statement(stmt, calls);
    }
}

fn collect_calls_in_statement(stmt: &TypedStatement, calls: &mut IndexSet<String>) {
    match stmt {
        TypedStatement::Let { value, .. } => collect_calls_in_expr(value, calls),
        TypedStatement::Expr(expr) => collect_calls_in_expr(expr, calls),
        TypedStatement::Return(Some(expr)) => collect_calls_in_expr(expr, calls),
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_calls_in_expr(cond, calls);
            collect_called_functions_into(then_branch, calls);
            if let Some(b) = else_branch {
                collect_called_functions_into(b, calls);
            }
        }
        TypedStatement::While { cond, body } => {
            collect_calls_in_expr(cond, calls);
            collect_called_functions_into(body, calls);
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            if let Some(init_stmt) = init.as_deref() {
                collect_calls_in_statement(init_stmt, calls);
            }
            if let Some(cond_expr) = cond {
                collect_calls_in_expr(cond_expr, calls);
            }
            if let Some(step_stmt) = step.as_deref() {
                collect_calls_in_statement(step_stmt, calls);
            }
            collect_called_functions_into(body, calls);
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            collect_calls_in_expr(map, calls);
            collect_called_functions_into(body, calls);
        }
        TypedStatement::MapSet { map, key, value } => {
            collect_calls_in_expr(map, calls);
            collect_calls_in_expr(key, calls);
            collect_calls_in_expr(value, calls);
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
    }
}

fn collect_calls_in_expr(expr: &TypedExpr, calls: &mut IndexSet<String>) {
    match &expr.expr {
        ExprKind::Call { name, args } => {
            if is_user_defined_function(name) {
                calls.insert(name.clone());
            }
            for arg in args {
                collect_calls_in_expr(arg, calls);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_calls_in_expr(left, calls);
            collect_calls_in_expr(right, calls);
        }
        ExprKind::Unary { expr: inner, .. } => collect_calls_in_expr(inner, calls),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_calls_in_expr(cond, calls);
            collect_calls_in_expr(then_expr, calls);
            collect_calls_in_expr(else_expr, calls);
        }
        ExprKind::Tuple(items) => {
            for item in items {
                collect_calls_in_expr(item, calls);
            }
        }
        ExprKind::Member { object, .. } => collect_calls_in_expr(object, calls),
        ExprKind::Index { target, index } => {
            collect_calls_in_expr(target, calls);
            collect_calls_in_expr(index, calls);
        }
        ExprKind::Bool(_) | ExprKind::Number(_) | ExprKind::String(_) | ExprKind::Ident(_) => {}
    }
}

fn ensure_state_map_iter_supported(map_expr: &TypedExpr) -> Result<(), SemanticError> {
    if typed_map_expr_is_state(map_expr)
        && matches!(
            resolve_struct_type(&map_expr.ty),
            Type::Map(k, _) if !matches!(resolve_struct_type(&k), Type::Int)
        )
    {
        return Err(SemanticError {
            message: "durable state map iteration supports Map<int, *> keys only".into(),
        });
    }
    Ok(())
}

fn ensure_not_state_shadow(name: &str) -> Result<(), SemanticError> {
    if is_state_identifier(name) {
        return Err(SemanticError {
            message: format!("E_STATE_SHADOWED: `{name}` shadows a state declaration"),
        });
    }
    Ok(())
}

fn is_state_map_expr(expr: &TypedExpr) -> bool {
    matches!(resolve_struct_type(&expr.ty), Type::Map(_, _)) && typed_map_expr_is_state(expr)
}

fn typed_map_expr_is_state(expr: &TypedExpr) -> bool {
    match &expr.expr {
        ExprKind::Ident(name) => is_state_identifier(name),
        ExprKind::Member { object, .. } => typed_map_expr_is_state(object),
        _ => false,
    }
}

fn map_expr_is_state(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) => is_state_identifier(name),
        Expr::Member { object, .. } => map_expr_is_state(object),
        _ => false,
    }
}

fn is_user_defined_function(name: &str) -> bool {
    FUNCTION_RETURNS.with(|env| env.borrow().contains_key(name))
}

fn enforce_permission_requirements(items: &[TypedItem]) -> Result<(), SemanticError> {
    let summaries = FUNCTION_SUMMARY.with(|map| map.borrow().clone());
    let mut requires_permission: HashMap<String, bool> = HashMap::new();
    let mut changed = true;
    while changed {
        changed = false;
        for (name, summary) in summaries.iter() {
            let mut required = summary.direct_sensitive;
            if !required {
                required = summary
                    .calls
                    .iter()
                    .any(|callee| *requires_permission.get(callee).unwrap_or(&false));
            }
            if requires_permission.get(name).copied().unwrap_or(false) != required {
                requires_permission.insert(name.clone(), required);
                changed = true;
            }
        }
    }
    for func in items.iter().map(|item| match item {
        TypedItem::Function(func) => func,
    }) {
        let needs_permission = requires_permission
            .get(&func.name)
            .copied()
            .unwrap_or(false);
        if needs_permission
            && func.modifiers.visibility == FunctionVisibility::Public
            && func.modifiers.permission.is_none()
        {
            return Err(SemanticError {
                message: format!(
                    "public function `{}` calls privileged operations but is missing `permission(...)`",
                    func.name
                ),
            });
        }
    }
    Ok(())
}

fn statement_contains_sensitive_syscall(stmt: &TypedStatement) -> bool {
    match stmt {
        TypedStatement::Expr(expr)
        | TypedStatement::Return(Some(expr))
        | TypedStatement::Let { value: expr, .. }
        | TypedStatement::MapSet { value: expr, .. } => expr_contains_sensitive_syscall(expr),
        TypedStatement::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_contains_sensitive_syscall(cond)
                || block_contains_sensitive_syscall(then_branch)
                || else_branch
                    .as_ref()
                    .map(block_contains_sensitive_syscall)
                    .unwrap_or(false)
        }
        TypedStatement::While { cond, body } => {
            expr_contains_sensitive_syscall(cond) || block_contains_sensitive_syscall(body)
        }
        TypedStatement::For {
            init,
            cond,
            step,
            body,
            ..
        } => {
            init.as_deref()
                .map(statement_contains_sensitive_syscall)
                .unwrap_or(false)
                || cond
                    .as_ref()
                    .map(expr_contains_sensitive_syscall)
                    .unwrap_or(false)
                || step
                    .as_deref()
                    .map(statement_contains_sensitive_syscall)
                    .unwrap_or(false)
                || block_contains_sensitive_syscall(body)
        }
        TypedStatement::ForEachMap { map, body, .. } => {
            expr_contains_sensitive_syscall(map) || block_contains_sensitive_syscall(body)
        }
        TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => false,
    }
}

fn expr_contains_sensitive_syscall(expr: &TypedExpr) -> bool {
    match &expr.expr {
        ExprKind::Call { name, args } => {
            SENSITIVE_SYSCALLS.contains(&name.as_str())
                || args.iter().any(expr_contains_sensitive_syscall)
        }
        ExprKind::Binary { left, right, .. } => {
            expr_contains_sensitive_syscall(left) || expr_contains_sensitive_syscall(right)
        }
        ExprKind::Unary { expr, .. } => expr_contains_sensitive_syscall(expr),
        ExprKind::Conditional {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_contains_sensitive_syscall(cond)
                || expr_contains_sensitive_syscall(then_expr)
                || expr_contains_sensitive_syscall(else_expr)
        }
        ExprKind::Tuple(items) => items.iter().any(expr_contains_sensitive_syscall),
        ExprKind::Member { object, .. } => expr_contains_sensitive_syscall(object),
        ExprKind::Index { target, index } => {
            expr_contains_sensitive_syscall(target) || expr_contains_sensitive_syscall(index)
        }
        ExprKind::Number(_) | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Ident(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kotodama::parser::parse;

    #[test]
    fn return_type_match() {
        let ok1 = analyze(&parse("fn f() -> bool { return true; } ").unwrap());
        assert!(ok1.is_ok());
        let ok2 = analyze(&parse("fn g() -> int { return 1; } ").unwrap());
        assert!(ok2.is_ok());
        let ok3 = analyze(&parse("fn h() -> unit { return; } ").unwrap());
        assert!(ok3.is_ok());
    }

    #[test]
    fn return_type_mismatch() {
        let err = analyze(&parse("fn f() -> bool { return 1; } ").unwrap());
        assert!(err.is_err());
        let err2 = analyze(&parse("fn h() -> unit { return 1; } ").unwrap());
        assert!(err2.is_err());
    }

    #[test]
    fn non_unit_must_return_all_paths() {
        let err = analyze(&parse("fn f() -> int { if true { return 1; } } ").unwrap());
        assert!(err.is_err());
        let ok =
            analyze(&parse("fn g() -> int { if true { return 1; } else { return 2; } } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn return_value_requires_declared_type() {
        let err = analyze(&parse("fn f() { return 1; } ").unwrap());
        assert!(err.is_err());
        let ok = analyze(&parse("fn g() { return; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn param_type_enforcement_primitives() {
        // bool param implicitly promotes to 0/1, so arithmetic is permitted
        analyze(&parse("fn f(bool x) { let y = x + 1; } ").unwrap())
            .expect("bool should coerce to int");
        // string param cannot be used in arithmetic
        let err2 = analyze(&parse("fn g(string s) { let y = s + 1; } ").unwrap());
        assert!(err2.is_err());
        // int default works
        let ok = analyze(&parse("fn h(x, y) -> int { return x + y; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn opaque_param_types() {
        // Opaque type should not be allowed in arithmetic
        let err = analyze(&parse("fn f(AccountId who) { let y = who + 1; } ").unwrap());
        assert!(err.is_err());
        // Equality on same opaque types is allowed
        let ok =
            analyze(&parse("fn g(AccountId a, AccountId b) -> bool { return a == b; } ").unwrap());
        assert!(ok.is_ok());
    }

    #[test]
    fn tuple_bindings_flatten_members() {
        let program = parse("fn f() { let pair = (1, 2); } ").unwrap();
        let typed = analyze(&program).expect("analysis ok");
        let TypedItem::Function(func) = &typed.items[0];
        let names: Vec<String> = func
            .body
            .statements
            .iter()
            .filter_map(|stmt| match stmt {
                TypedStatement::Let { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        let suffixes: Vec<String> = names
            .into_iter()
            .map(|name| name.rsplit("::").next().unwrap().to_string())
            .collect();
        assert_eq!(suffixes, vec!["pair", "pair#0", "pair#1"]);
    }

    #[test]
    fn state_map_iteration_rejects_pointer_keys_in_nested_state() {
        let program = parse(
            "struct Holder { map: Map<Name, int>; } \
             state Holder holder; \
             fn main() { \
                 for (k, v) in holder.map #[bounded(1)] { \
                     let _x = v; \
                 } \
             }",
        )
        .expect("parse nested state map");
        let err = analyze(&program).expect_err("non-int state map keys should error");
        assert_eq!(
            err.message,
            "durable state map iteration supports Map<int, *> keys only"
        );
    }

    #[test]
    fn state_map_alias_is_rejected() {
        let program = parse(
            "state M: Map<int, int>; \
             fn main() { \
                 let m = M; \
             }",
        )
        .expect("parse state map alias");
        let err = analyze(&program).expect_err("aliasing a state map should error");
        assert!(err.message.contains("E_STATE_MAP_ALIAS"));
    }

    #[test]
    fn state_map_reassignment_is_rejected() {
        let program = parse(
            "state M: Map<int, int>; \
             fn main() { \
                 M = Map::new(); \
             }",
        )
        .expect("parse state map reassignment");
        let err = analyze(&program).expect_err("reassigning a state map should error");
        assert!(err.message.contains("E_STATE_MAP_ALIAS"));
    }

    #[test]
    fn state_map_cannot_be_passed_to_user_fn() {
        let program = parse(
            "state M: Map<int, int>; \
             fn f(m: Map<int, int>) { let _x = 0; } \
             fn main() { f(M); }",
        )
        .expect("parse state map arg");
        let err = analyze(&program).expect_err("passing state map to user fn should error");
        assert!(err.message.contains("E_STATE_MAP_ALIAS"));
    }

    #[test]
    fn map_assignment_requires_map_target() {
        let program = parse("fn f() { let x = 1; x[0] = 2; }").expect("parse map assignment");
        let err = analyze(&program).expect_err("non-map assignment should error");
        assert!(err.message.contains("map assignment expects Map<K,V>"));
    }

    #[test]
    fn assignment_allows_bool_to_int() {
        let program =
            parse("fn f() { let x: int = true; x = false; }").expect("parse bool assignment");
        analyze(&program).expect("bool assignment should be allowed for int");
    }

    #[test]
    fn break_requires_loop_context() {
        let program = parse("fn f() { break; }").expect("parse break");
        let err = analyze(&program).expect_err("break outside loop should error");
        assert!(err.message.contains("E_BREAK_OUTSIDE_LOOP"));
    }

    #[test]
    fn continue_requires_loop_context() {
        let program = parse("fn f() { continue; }").expect("parse continue");
        let err = analyze(&program).expect_err("continue outside loop should error");
        assert!(err.message.contains("E_CONTINUE_OUTSIDE_LOOP"));
    }

    #[test]
    fn state_shadowing_is_rejected_in_let() {
        let program =
            parse("state int counter; fn f() { let counter = 1; }").expect("parse shadowing let");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert!(err.message.contains("E_STATE_SHADOWED"));
    }

    #[test]
    fn state_shadowing_is_rejected_in_params() {
        let program =
            parse("state int counter; fn f(counter: int) {}").expect("parse shadowing param");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert!(err.message.contains("E_STATE_SHADOWED"));
    }

    #[test]
    fn state_shadowing_is_rejected_in_map_loop_vars() {
        let program = parse(
            "state int counter; state M: Map<int, int>; \
             fn f() { for (counter, v) in M.take(1) { let _x = v; } }",
        )
        .expect("parse shadowing loop vars");
        let err = analyze(&program).expect_err("state shadowing should error");
        assert!(err.message.contains("E_STATE_SHADOWED"));
    }

    #[test]
    fn for_init_requires_simple_statement() {
        let program = parse(
            "fn f() { \
                for let pair = (1, 2); pair.0 < 3; { \
                    let _x = pair.0; \
                } \
            }",
        )
        .expect("parse for init");
        let err = analyze(&program).expect_err("complex for init should error");
        assert!(err.message.contains("E0005"));
    }

    #[test]
    fn for_step_requires_simple_statement() {
        let program = parse(
            "fn f() { \
                for let i = 0; i < 1; let pair = (1, 2) { \
                    let _x = i; \
                } \
            }",
        )
        .expect("parse for step");
        let err = analyze(&program).expect_err("complex for step should error");
        assert!(err.message.contains("E0006"));
    }

    #[test]
    fn equality_rejects_tuple_types() {
        let program = parse("fn f() { let a = (1, 2); let b = (1, 2); let _x = a == b; }")
            .expect("parse tuple equality");
        let err = analyze(&program).expect_err("tuple equality should error");
        assert!(err.message.contains("equality is not supported"));
    }

    #[test]
    fn pointer_constructor_requires_string_literal() {
        let program = parse("fn f() { let s = \"wonderland\"; let _n = name(s); }")
            .expect("parse pointer constructor");
        let err = analyze(&program).expect_err("non-literal string should error");
        assert!(err.message.contains("string literal"));
    }

    #[test]
    fn for_step_bindings_do_not_leak_into_body() {
        let program = parse(
            "fn f() { \
                for let i = 0; i < 1; let t = 1 { \
                    let _x = t; \
                } \
            }",
        )
        .expect("parse for loop");
        let err = analyze(&program).expect_err("step bindings should be out of scope");
        assert!(err.message.contains("undefined variable"));
    }

    #[test]
    fn for_body_bindings_do_not_escape_loop() {
        let program = parse(
            "fn f() { \
                for let i = 0; i < 1; i = i + 1 { \
                    let x = 1; \
                } \
                let _y = x; \
            }",
        )
        .expect("parse for loop");
        let err = analyze(&program).expect_err("body bindings should not escape");
        assert!(err.message.contains("undefined variable"));
    }

    #[test]
    fn tuple_pattern_requires_tuple_type() {
        let program = parse("fn f() { let (a, b) = 1; }").expect("parse tuple pattern");
        let err = analyze(&program).expect_err("non-tuple destructuring should error");
        assert!(err.message.contains("tuple destructuring expects a tuple"));
    }

    #[test]
    fn tuple_pattern_requires_arity_match() {
        let program = parse("fn f() { let (a, b, c) = (1, 2); }").expect("parse tuple pattern");
        let err = analyze(&program).expect_err("tuple arity mismatch should error");
        assert!(
            err.message
                .contains("tuple destructuring expects 2 bindings")
        );
    }

    #[test]
    fn struct_pattern_requires_arity_match() {
        let program = parse(
            "struct Pair { a: int, b: int } \
             fn f() { let (a) = Pair(1, 2); }",
        )
        .expect("parse struct pattern");
        let err = analyze(&program).expect_err("struct arity mismatch should error");
        assert!(
            err.message
                .contains("struct destructuring expects 2 bindings")
        );
    }

    #[test]
    fn assert_rejects_extra_args() {
        let program = parse("fn f() { assert(true, \"oops\"); }").expect("parse assert");
        let err = analyze(&program).expect_err("assert extra args should error");
        assert!(err.message.contains("assert expects (bool)"));
    }

    #[test]
    fn map_new_respects_type_annotation() {
        let program = parse("fn f() { let m: Map<Name, int> = Map::new(); let _x = m; }")
            .expect("parse Map::new");
        analyze(&program).expect("Map::new should adopt annotated map type");
    }

    #[test]
    fn map_new_respects_return_type() {
        let program = parse("fn f() -> Map<Name, int> { return Map::new(); }")
            .expect("parse Map::new return");
        analyze(&program).expect("Map::new should adopt return map type");
    }

    #[test]
    fn in_memory_map_rejects_tuple_key() {
        let program = parse(
            "fn f() { \
                let m: Map<(int, int), int> = Map::new(); \
                let _x = contains(m, (1, 2)); \
            }",
        )
        .expect("parse tuple map key");
        let err = analyze(&program).expect_err("tuple map key should error");
        assert!(err.message.contains("in-memory Map key type"));
    }

    #[test]
    fn in_memory_map_rejects_tuple_value() {
        let program = parse(
            "fn f() { \
                let m: Map<int, (int, int)> = Map::new(); \
                let _x = m[0]; \
            }",
        )
        .expect("parse tuple map value");
        let err = analyze(&program).expect_err("tuple map value should error");
        assert!(err.message.contains("in-memory Map value type"));
    }

    #[test]
    fn blob_bytes_equality_is_allowed() {
        let program = parse(
            "fn f() { let b: bytes = blob(\"hi\"); let c: Blob = blob(\"hi\"); let _x = b == c; }",
        )
        .expect("parse blob equality");
        analyze(&program).expect("blob/bytes equality should be allowed");
    }

    #[test]
    fn state_map_key_type_is_validated() {
        let program = parse("state M: Map<string, int>; fn f() {}").expect("parse state map");
        let err = analyze(&program).expect_err("state map key should be validated");
        assert!(err.message.contains("state Map key type"));
    }

    #[test]
    fn field_assignment_is_rejected() {
        let program = parse("fn f() { let t = (1, 2); t.0 = 3; }").expect("parse field assignment");
        let err = analyze(&program).expect_err("field assignment should error");
        assert!(err.message.contains("assignment target must be"));
    }

    #[test]
    fn info_accepts_int() {
        let program = parse("fn f() { info(42); }").expect("parse info");
        analyze(&program).expect("info should accept int");
    }

    #[test]
    fn state_scalar_type_is_validated() {
        let program = parse("state string label; fn f() {}").expect("parse state");
        let err = analyze(&program).expect_err("unsupported state type should error");
        assert!(err.message.contains("state type `string`"));
    }

    #[test]
    fn state_struct_field_type_is_validated() {
        let program =
            parse("struct S { label: string } state S s; fn f() {}").expect("parse state struct");
        let err = analyze(&program).expect_err("unsupported state field should error");
        assert!(err.message.contains("state type `string`"));
    }
}
