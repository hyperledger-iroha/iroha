//! Compile-time enforcement for Kotodama's on-chain safety profile.
//!
//! The on-chain profile forbids map key types that do not have a stable,
//! deterministic ordering across heterogeneous hardware. This first pass checks
//! typed programs after semantic analysis and emits deterministic errors when
//! contracts attempt to use unsupported key types (e.g., `string`).

use std::collections::HashSet;

use super::semantic::{
    self, ExprKind, Type, TypedBlock, TypedExpr, TypedFunction, TypedItem, TypedProgram,
    TypedStatement,
};

/// Violation emitted when on-chain policy checks fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyError {
    /// Human-readable description of the violation.
    pub message: String,
}

/// Run the on-chain profile enforcement against a typed Kotodama program.
pub fn enforce_on_chain_profile(program: &TypedProgram) -> Result<(), Vec<PolicyError>> {
    let mut checker = Checker::default();
    checker.check_state_env();
    for item in &program.items {
        checker.visit_item(item);
    }
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}

#[derive(Default)]
struct Checker {
    errors: Vec<PolicyError>,
    /// Avoid emitting duplicate messages for the same origin/type combination.
    seen: HashSet<(String, String)>,
}

impl Checker {
    fn check_state_env(&mut self) {
        for (name, ty) in semantic::state_env_snapshot() {
            let origin = format!("state `{name}`");
            self.visit_type(&ty, &origin);
        }
    }

    fn visit_item(&mut self, item: &TypedItem) {
        match item {
            TypedItem::Function(func) => self.visit_function(func),
        }
    }

    fn visit_function(&mut self, func: &TypedFunction) {
        self.visit_block(&func.body, func.name.as_str());
        if let Some(ret_ty) = &func.ret_ty {
            let origin = format!("function `{}` return type", func.name);
            self.visit_type(ret_ty, &origin);
        }
    }

    fn visit_block(&mut self, block: &TypedBlock, func_name: &str) {
        for stmt in &block.statements {
            self.visit_statement(stmt, func_name);
        }
    }

    fn visit_statement(&mut self, stmt: &TypedStatement, func_name: &str) {
        match stmt {
            TypedStatement::Let { name, value } => {
                let origin = format!("binding `{name}` in `{func_name}`");
                self.visit_expr(value, &origin);
            }
            TypedStatement::Expr(expr) => {
                let origin = format!("expression in `{func_name}`");
                self.visit_expr(expr, &origin);
            }
            TypedStatement::Return(Some(expr)) => {
                let origin = format!("return in `{func_name}`");
                self.visit_expr(expr, &origin);
            }
            TypedStatement::Return(None) | TypedStatement::Break | TypedStatement::Continue => {}
            TypedStatement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_origin = format!("condition in `{func_name}`");
                self.visit_expr(cond, &cond_origin);
                self.visit_block(then_branch, func_name);
                if let Some(b) = else_branch {
                    self.visit_block(b, func_name);
                }
            }
            TypedStatement::While { cond, body } => {
                let origin = format!("while condition in `{func_name}`");
                self.visit_expr(cond, &origin);
                self.visit_block(body, func_name);
            }
            TypedStatement::For {
                line: _,
                init,
                cond,
                step,
                body,
            } => {
                if let Some(init_stmt) = init.as_deref() {
                    self.visit_statement(init_stmt, func_name);
                }
                if let Some(cond_expr) = cond {
                    let origin = format!("for condition in `{func_name}`");
                    self.visit_expr(cond_expr, &origin);
                }
                if let Some(step_stmt) = step.as_deref() {
                    self.visit_statement(step_stmt, func_name);
                }
                self.visit_block(body, func_name);
            }
            TypedStatement::ForEachMap { map, body, .. } => {
                let origin = format!("map iteration in `{func_name}`");
                self.visit_expr(map, &origin);
                self.visit_block(body, func_name);
            }
            TypedStatement::MapSet { map, key, value } => {
                let origin = format!("map assignment in `{func_name}`");
                self.visit_expr(map, &origin);
                self.visit_expr(key, &origin);
                self.visit_expr(value, &origin);
            }
        }
    }

    fn visit_expr(&mut self, expr: &TypedExpr, origin: &str) {
        self.visit_type(&expr.ty, origin);
        match &expr.expr {
            ExprKind::Binary { left, right, .. } => {
                self.visit_expr(left, origin);
                self.visit_expr(right, origin);
            }
            ExprKind::Unary { expr: inner, .. } => self.visit_expr(inner, origin),
            ExprKind::NumericCast { expr } => self.visit_expr(expr, origin),
            ExprKind::Conditional {
                cond,
                then_expr,
                else_expr,
            } => {
                self.visit_expr(cond, origin);
                self.visit_expr(then_expr, origin);
                self.visit_expr(else_expr, origin);
            }
            ExprKind::Call { args, .. } | ExprKind::Tuple(args) => {
                for arg in args {
                    self.visit_expr(arg, origin);
                }
            }
            ExprKind::Member { object, .. } => self.visit_expr(object, origin),
            ExprKind::Index { target, index } => {
                self.visit_expr(target, origin);
                self.visit_expr(index, origin);
            }
            ExprKind::Number(_)
            | ExprKind::Decimal(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Bytes(_)
            | ExprKind::Ident(_) => {}
        }
    }

    fn visit_type(&mut self, ty: &Type, origin: &str) {
        let resolved = semantic::resolve_struct_type(ty);
        match &resolved {
            Type::Map(key, value) => {
                self.check_map_key(origin, key, value);
                self.visit_type(key, origin);
                self.visit_type(value, origin);
            }
            Type::Tuple(elems) => {
                for elem in elems {
                    self.visit_type(elem, origin);
                }
            }
            Type::Struct { fields, .. } => {
                for (_name, field_ty) in fields {
                    self.visit_type(field_ty, origin);
                }
            }
            _ => {}
        }
    }

    fn check_map_key(&mut self, origin: &str, key: &Type, value: &Type) {
        if is_allowed_map_key_type(key) {
            return;
        }
        let key_name = display_type(key);
        let value_name = display_type(value);
        let map_desc = format!("Map<{key_name}, {value_name}>");
        if self.seen.insert((origin.to_string(), map_desc.clone())) {
            let message = format!(
                "on-chain profile forbids map with key type `{key_name}` in {origin}. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob."
            );
            self.errors.push(PolicyError { message });
        }
    }
}

fn is_allowed_map_key_type(ty: &Type) -> bool {
    matches!(
        semantic::resolve_struct_type(ty),
        Type::Int
            | Type::FixedU128
            | Type::Amount
            | Type::Balance
            | Type::AccountId
            | Type::AssetDefinitionId
            | Type::AssetId
            | Type::NftId
            | Type::DomainId
            | Type::Name
            | Type::DataSpaceId
            | Type::AxtDescriptor
            | Type::AssetHandle
            | Type::ProofBlob
    )
}

fn display_type(ty: &Type) -> String {
    match semantic::resolve_struct_type(ty) {
        Type::Int => "int".to_string(),
        Type::FixedU128 => "fixed_u128".to_string(),
        Type::Amount => "Amount".to_string(),
        Type::Balance => "Balance".to_string(),
        Type::Bool => "bool".to_string(),
        Type::String => "string".to_string(),
        Type::Blob => "Blob".to_string(),
        Type::Bytes => "bytes".to_string(),
        Type::AccountId => "AccountId".to_string(),
        Type::AssetDefinitionId => "AssetDefinitionId".to_string(),
        Type::AssetId => "AssetId".to_string(),
        Type::NftId => "NftId".to_string(),
        Type::DomainId => "DomainId".to_string(),
        Type::Name => "Name".to_string(),
        Type::DataSpaceId => "DataSpaceId".to_string(),
        Type::AxtDescriptor => "AxtDescriptor".to_string(),
        Type::AssetHandle => "AssetHandle".to_string(),
        Type::ProofBlob => "ProofBlob".to_string(),
        Type::Json => "Json".to_string(),
        Type::Unit => "()".to_string(),
        Type::Map(k, v) => format!("Map<{}, {}>", display_type(&k), display_type(&v)),
        Type::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(display_type).collect();
            format!("({})", parts.join(", "))
        }
        Type::Struct { name, .. } => format!("struct {name}"),
        Type::Opaque(name) => name,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        semantic::{ExprKind, Type, TypedExpr, TypedStatement},
        *,
    };

    #[test]
    fn map_key_violation_reports_origin() {
        let mut checker = Checker::default();
        let stmt = TypedStatement::Expr(TypedExpr {
            expr: ExprKind::Ident("bad_map".into()),
            ty: Type::Map(Box::new(Type::String), Box::new(Type::Int)),
        });

        checker.visit_statement(&stmt, "foo");

        let errors = checker.errors;
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "on-chain profile forbids map with key type `string` in expression in `foo`. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob."
        );
    }
}
