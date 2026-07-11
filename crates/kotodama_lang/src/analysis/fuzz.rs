use std::collections::HashMap;

use iroha_primitives::{bigint::BigInt, numeric_abi::IntValueV1};

use super::{AnalysisCategory, AnalysisFinding};
use crate::{
    analysis::SimpleRng,
    ast::{Function as AstFunction, Item, Program, TypeExpr},
    semantic::{
        self, ExprKind, Type, TypedBlock, TypedExpr, TypedFunction, TypedProgram, TypedStatement,
    },
};

/// Outcome of the Kotodama source fuzz pass.
#[derive(Debug, Default)]
pub struct FuzzReport {
    pub findings: Vec<AnalysisFinding>,
    pub cases_executed: usize,
    pub functions_covered: usize,
}

const MAX_PARAMS: usize = 3;
const DEFAULT_MAX_CASES: usize = 64;
const DEFAULT_LOOP_LIMIT: usize = 128;
const DEFAULT_RECURSION_LIMIT: usize = 16;

/// Execute a lightweight interpreter-based fuzzing harness against the typed
/// program. Only integer and boolean parameters are supported currently. Other
/// functions are reported as skipped.
pub fn run_fuzz(
    program: &Program,
    typed: &TypedProgram,
    max_cases_per_function: usize,
) -> FuzzReport {
    let mut report = FuzzReport::default();
    let ast_functions: HashMap<&str, &AstFunction> = program
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(func) => Some((func.name.as_str(), func)),
            _ => None,
        })
        .collect();

    let typed_functions: HashMap<&str, &TypedFunction> = typed
        .items
        .iter()
        .map(|item| match item {
            semantic::TypedItem::Function(func) => (func.name.as_str(), func),
        })
        .collect();

    if typed_functions.is_empty() {
        return report;
    }

    let mut evaluator = Evaluator::new(
        &typed_functions,
        DEFAULT_RECURSION_LIMIT,
        DEFAULT_LOOP_LIMIT,
    );

    for name in typed_functions.keys() {
        let Some(ast_fn) = ast_functions.get(name) else {
            continue;
        };
        let param_specs = match param_specs(ast_fn) {
            Ok(specs) => specs,
            Err(msg) => {
                report.findings.push(AnalysisFinding::info(
                    AnalysisCategory::Fuzz,
                    "fuzz-skip-params",
                    format!("function `{name}` skipped: {msg}"),
                ));
                continue;
            }
        };
        if param_specs.len() > MAX_PARAMS {
            report.findings.push(AnalysisFinding::info(
                AnalysisCategory::Fuzz,
                "fuzz-skip-params",
                format!(
                    "function `{name}` skipped: parameter count {} exceeds fuzz harness limit {}",
                    param_specs.len(),
                    MAX_PARAMS
                ),
            ));
            continue;
        }
        let samples = match build_samples(&param_specs) {
            Some(samples) => samples,
            None => {
                report.findings.push(AnalysisFinding::info(
                    AnalysisCategory::Fuzz,
                    "fuzz-skip-types",
                    format!("function `{name}` skipped: unsupported parameter or return type"),
                ));
                continue;
            }
        };
        let limit = max_cases_per_function.clamp(1, DEFAULT_MAX_CASES);
        let cases = enumerate_cases(&samples, limit);
        if cases.is_empty() {
            continue;
        }

        report.functions_covered += 1;
        let mut rng = SimpleRng::new(hash_name(name));
        let mut executed = 0usize;
        for args in cases {
            match evaluator.run_function(name, &args) {
                Ok(_) => {
                    executed += 1;
                }
                Err(EvalError::UnsupportedFeature(feature)) => {
                    report.findings.push(AnalysisFinding::info(
                        AnalysisCategory::Fuzz,
                        "fuzz-skip-feature",
                        format!(
                            "function `{name}` skipped: feature `{feature}` is outside the fuzz interpreter subset"
                        ),
                    ));
                    executed = 0;
                    break;
                }
                Err(EvalError::UnsupportedCall(call_name)) => {
                    report.findings.push(AnalysisFinding::info(
                        AnalysisCategory::Fuzz,
                        "fuzz-skip-call",
                        format!(
                            "function `{name}` skipped: call to `{call_name}` not supported by harness"
                        ),
                    ));
                    executed = 0;
                    break;
                }
                Err(EvalError::LoopLimitExceeded) => {
                    report.findings.push(AnalysisFinding::warning(
                        AnalysisCategory::Fuzz,
                        "fuzz-loop-limit",
                        format!(
                            "function `{name}` exceeded interpreter loop limit ({DEFAULT_LOOP_LIMIT} iterations)"
                        ),
                    ));
                    break;
                }
                Err(EvalError::RecursionLimitExceeded) => {
                    report.findings.push(AnalysisFinding::warning(
                        AnalysisCategory::Fuzz,
                        "fuzz-recursion-limit",
                        format!(
                            "function `{name}` exceeded interpreter recursion depth ({DEFAULT_RECURSION_LIMIT})"
                        ),
                    ));
                    break;
                }
                Err(EvalError::ArithmeticOverflow(kind)) => {
                    report.findings.push(AnalysisFinding::warning(
                        AnalysisCategory::Fuzz,
                        "fuzz-arithmetic-overflow",
                        format!(
                            "function `{name}` triggered {kind} overflow under interpreter inputs"
                        ),
                    ));
                    break;
                }
                Err(EvalError::Runtime(msg)) => {
                    report.findings.push(AnalysisFinding::warning(
                        AnalysisCategory::Fuzz,
                        "fuzz-runtime-failure",
                        format!("function `{name}` failed under interpreter inputs: {msg}"),
                    ));
                    break;
                }
            }
            // Simple entropy injection: shuffle loop limits occasionally.
            if rng.next_bool() {
                evaluator.set_loop_limit(DEFAULT_LOOP_LIMIT + (rng.next_u32() as usize % 32));
            }
        }
        report.cases_executed += executed;
    }

    report
}

fn hash_name(name: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

fn param_specs(func: &AstFunction) -> Result<Vec<ParamSpec>, String> {
    let mut out = Vec::new();
    for param in &func.params {
        let ty = param_type_from_expr(&param.ty)?;
        out.push(ParamSpec { ty });
    }
    Ok(out)
}

fn param_type_from_expr(expr: &Option<TypeExpr>) -> Result<Type, String> {
    if let Some(t) = expr {
        convert_type_expr(t)
    } else {
        Ok(Type::Int)
    }
}

fn convert_type_expr(expr: &TypeExpr) -> Result<Type, String> {
    Ok(match expr {
        TypeExpr::Source { ty, .. } | TypeExpr::Resolved { ty, .. } => {
            return convert_type_expr(ty);
        }
        TypeExpr::Path(name) => match name.as_str() {
            "int" => Type::Int,
            "quantity" => Type::Quantity,
            "bool" => Type::Bool,
            "()" => Type::Unit,
            other => Type::NamedStruct(other.to_string()),
        },
        TypeExpr::Generic { base, args } => {
            if base == "StateMap" && args.len() == 2 {
                let key_ty = convert_type_expr(&args[0])?;
                let value_ty = convert_type_expr(&args[1])?;
                Type::StateMap(Box::new(key_ty), Box::new(value_ty))
            } else if base == "List" && args.len() == 2 {
                let element = convert_type_expr(&args[0])?;
                let TypeExpr::Const(capacity) = args[1].kind() else {
                    return Err("List capacity must be a compile-time integer".to_owned());
                };
                let capacity = u8::try_from(*capacity)
                    .ok()
                    .filter(|capacity| (1..=64).contains(capacity))
                    .ok_or_else(|| "List capacity must be in 1..=64".to_owned())?;
                Type::List(Box::new(element), capacity)
            } else {
                Type::NamedStruct(base.clone())
            }
        }
        TypeExpr::Tuple(items) => {
            let mut out = Vec::new();
            for item in items {
                out.push(convert_type_expr(item)?);
            }
            Type::Tuple(out)
        }
        TypeExpr::Const(value) => {
            return Err(format!(
                "compile-time integer `{value}` is only valid in List<T, N>"
            ));
        }
    })
}

fn build_samples(param_specs: &[ParamSpec]) -> Option<Vec<Vec<Value>>> {
    let mut samples = Vec::new();
    for spec in param_specs {
        let values = supported_type_samples(&spec.ty)?;
        samples.push(values);
    }
    Some(samples)
}

fn supported_type_samples(ty: &Type) -> Option<Vec<Value>> {
    match ty {
        Type::Int => Some(vec![
            Value::Int(BigInt::from_i128(-2)),
            Value::Int(BigInt::from_i128(-1)),
            Value::Int(BigInt::zero()),
            Value::Int(BigInt::one()),
            Value::Int(BigInt::from_i128(2)),
            Value::Int(
                "-340282366920938463463374607431768211456"
                    .parse()
                    .expect("-2^128 is a V1 int"),
            ),
            Value::Int(
                "340282366920938463463374607431768211456"
                    .parse()
                    .expect("2^128 is a V1 int"),
            ),
        ]),
        Type::Bool => Some(vec![Value::Bool(false), Value::Bool(true)]),
        Type::Unit => Some(vec![Value::Unit]),
        _ => None,
    }
}

fn enumerate_cases(samples: &[Vec<Value>], limit: usize) -> Vec<Vec<Value>> {
    let mut out = Vec::new();
    let mut current = Vec::with_capacity(samples.len());
    enumerate_cases_rec(samples, limit, 0, &mut current, &mut out);
    out
}

fn enumerate_cases_rec(
    samples: &[Vec<Value>],
    limit: usize,
    idx: usize,
    current: &mut Vec<Value>,
    out: &mut Vec<Vec<Value>>,
) {
    if out.len() >= limit {
        return;
    }
    if idx == samples.len() {
        out.push(current.clone());
        return;
    }
    for value in &samples[idx] {
        if out.len() >= limit {
            break;
        }
        current.push(value.clone());
        enumerate_cases_rec(samples, limit, idx + 1, current, out);
        current.pop();
    }
}

struct ParamSpec {
    ty: Type,
}

#[derive(Debug, Clone)]
enum Value {
    Int(BigInt),
    Bool(bool),
    Unit,
    Tuple(Vec<Value>),
}

impl Value {
    fn as_int(&self) -> Option<&BigInt> {
        match self {
            Value::Int(value) => Some(value),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

fn bounded_int(value: BigInt, operation: &'static str) -> Result<Value, EvalError> {
    IntValueV1::try_new(value.clone()).map_err(|_| EvalError::ArithmeticOverflow(operation))?;
    Ok(Value::Int(value))
}

struct Evaluator<'a> {
    functions: HashMap<&'a str, &'a TypedFunction>,
    recursion_limit: usize,
    loop_limit: usize,
}

impl<'a> Evaluator<'a> {
    fn new(
        functions: &HashMap<&'a str, &'a TypedFunction>,
        recursion_limit: usize,
        loop_limit: usize,
    ) -> Self {
        Self {
            functions: functions.clone(),
            recursion_limit,
            loop_limit,
        }
    }

    fn set_loop_limit(&mut self, limit: usize) {
        self.loop_limit = limit.max(1);
    }

    fn run_function(&mut self, name: &str, args: &[Value]) -> Result<Value, EvalError> {
        self.execute_function(name, args, 0)
    }

    fn execute_function(
        &mut self,
        name: &str,
        args: &[Value],
        depth: usize,
    ) -> Result<Value, EvalError> {
        if depth >= self.recursion_limit {
            return Err(EvalError::RecursionLimitExceeded);
        }
        let (params, body) = {
            let function = self
                .functions
                .get(name)
                .ok_or_else(|| EvalError::Runtime(format!("unknown function `{name}`")))?;
            (function.params.clone(), function.body.clone())
        };
        if params.len() != args.len() {
            return Err(EvalError::Runtime(format!(
                "function `{name}` parameter count mismatch: expected {}, got {}",
                params.len(),
                args.len()
            )));
        }
        let mut locals = HashMap::new();
        for (param_name, arg) in params.iter().zip(args.iter()) {
            locals.insert(param_name.clone(), arg.clone());
        }
        match self.exec_block(&body, &mut locals, depth)? {
            FlowControl::Return(value) => Ok(value),
            FlowControl::Next => {
                if let Some(tail) = &body.tail {
                    self.eval_expr(tail, &mut locals, depth)
                } else {
                    Ok(Value::Unit)
                }
            }
            FlowControl::Break | FlowControl::ContinueLoop => Err(EvalError::Runtime(
                "loop control statement escaped function body".into(),
            )),
        }
    }

    fn exec_block(
        &mut self,
        block: &TypedBlock,
        locals: &mut HashMap<String, Value>,
        depth: usize,
    ) -> Result<FlowControl, EvalError> {
        for stmt in &block.statements {
            match self.exec_statement(stmt, locals, depth)? {
                FlowControl::Next => {}
                other => return Ok(other),
            }
        }
        Ok(FlowControl::Next)
    }

    fn exec_statement(
        &mut self,
        stmt: &TypedStatement,
        locals: &mut HashMap<String, Value>,
        depth: usize,
    ) -> Result<FlowControl, EvalError> {
        match stmt {
            TypedStatement::Let { name, value } => {
                let v = self.eval_expr(value, locals, depth)?;
                locals.insert(name.clone(), v);
                Ok(FlowControl::Next)
            }
            TypedStatement::Expr(expr) => {
                let _ = self.eval_expr(expr, locals, depth)?;
                Ok(FlowControl::Next)
            }
            TypedStatement::Return(Some(expr)) => {
                let value = self.eval_expr(expr, locals, depth)?;
                Ok(FlowControl::Return(value))
            }
            TypedStatement::Return(None) => Ok(FlowControl::Return(Value::Unit)),
            TypedStatement::Break => Ok(FlowControl::Break),
            TypedStatement::Continue => Ok(FlowControl::ContinueLoop),
            TypedStatement::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_val = self.eval_expr(cond, locals, depth)?;
                let cond_bool = cond_val
                    .as_bool()
                    .ok_or_else(|| EvalError::Runtime("if condition is not boolean".into()))?;
                if cond_bool {
                    self.exec_block(then_branch, locals, depth)
                } else if let Some(block) = else_branch {
                    self.exec_block(block, locals, depth)
                } else {
                    Ok(FlowControl::Next)
                }
            }
            TypedStatement::IfLet { .. } => Err(EvalError::UnsupportedFeature("if let statement")),
            TypedStatement::While { cond, body } => {
                let mut iterations = 0usize;
                loop {
                    let cond_val = self.eval_expr(cond, locals, depth)?;
                    let cond_bool = cond_val.as_bool().ok_or_else(|| {
                        EvalError::Runtime("while condition is not boolean".into())
                    })?;
                    if !cond_bool {
                        break;
                    }
                    if iterations >= self.loop_limit {
                        return Err(EvalError::LoopLimitExceeded);
                    }
                    iterations += 1;
                    match self.exec_block(body, locals, depth)? {
                        FlowControl::Next => {}
                        FlowControl::ContinueLoop => continue,
                        FlowControl::Break => break,
                        FlowControl::Return(v) => return Ok(FlowControl::Return(v)),
                    }
                }
                Ok(FlowControl::Next)
            }
            TypedStatement::For {
                init,
                cond,
                step,
                body,
                ..
            } => {
                if let Some(init) = init {
                    match self.exec_statement(init, locals, depth)? {
                        FlowControl::Next => {}
                        other => return Ok(other),
                    }
                }
                let mut iterations = 0usize;
                loop {
                    if let Some(cond) = cond {
                        let value = self.eval_expr(cond, locals, depth)?;
                        let condition = value.as_bool().ok_or_else(|| {
                            EvalError::Runtime("for condition is not boolean".into())
                        })?;
                        if !condition {
                            break;
                        }
                    }
                    if iterations >= self.loop_limit {
                        return Err(EvalError::LoopLimitExceeded);
                    }
                    iterations += 1;
                    match self.exec_block(body, locals, depth)? {
                        FlowControl::Next | FlowControl::ContinueLoop => {}
                        FlowControl::Break => break,
                        FlowControl::Return(value) => return Ok(FlowControl::Return(value)),
                    }
                    if let Some(step) = step {
                        match self.exec_statement(step, locals, depth)? {
                            FlowControl::Next => {}
                            FlowControl::Break => break,
                            FlowControl::Return(value) => return Ok(FlowControl::Return(value)),
                            FlowControl::ContinueLoop => {}
                        }
                    }
                }
                Ok(FlowControl::Next)
            }
            TypedStatement::ForEachMap { .. } | TypedStatement::MapSet { .. } => {
                Err(EvalError::UnsupportedFeature("iterators"))
            }
        }
    }

    fn eval_expr(
        &mut self,
        expr: &TypedExpr,
        locals: &mut HashMap<String, Value>,
        depth: usize,
    ) -> Result<Value, EvalError> {
        match &expr.expr {
            ExprKind::IntLiteral(value) => Ok(Value::Int(value.clone())),
            ExprKind::DecimalLiteral { .. } => {
                Err(EvalError::UnsupportedFeature("exact decimal literal"))
            }
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::String(_) | ExprKind::Bytes(_) => {
                Err(EvalError::UnsupportedFeature("string literal"))
            }
            ExprKind::List(_) | ExprKind::ListComprehension { .. } => {
                Err(EvalError::UnsupportedFeature("bounded lists"))
            }
            ExprKind::Ident(name) => locals
                .get(name)
                .cloned()
                .ok_or_else(|| EvalError::Runtime(format!("unknown variable `{name}`"))),
            ExprKind::Unary { op, expr } => {
                let inner = self.eval_expr(expr, locals, depth)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let val = inner.as_int().ok_or_else(|| {
                            EvalError::Runtime("negation expects integer operand".into())
                        })?;
                        let value = val
                            .checked_neg()
                            .map_err(|_| EvalError::ArithmeticOverflow("negation"))?;
                        bounded_int(value, "negation")
                    }
                    crate::ast::UnaryOp::Not => {
                        let val = inner.as_bool().ok_or_else(|| {
                            EvalError::Runtime("logical not expects boolean operand".into())
                        })?;
                        Ok(Value::Bool(!val))
                    }
                }
            }
            ExprKind::NumericCast { expr } => {
                let inner = self.eval_expr(expr, locals, depth)?;
                if inner.as_int().is_some() {
                    Ok(inner)
                } else {
                    Err(EvalError::Runtime(
                        "numeric cast expects integer operand".into(),
                    ))
                }
            }
            ExprKind::NumericTryCast { .. } => Err(EvalError::Runtime(
                "recoverable numeric conversions are not interpreted by the bounded fuzzer".into(),
            )),
            ExprKind::Binary { op, left, right } => {
                let lval = self.eval_expr(left, locals, depth)?;
                let rval = self.eval_expr(right, locals, depth)?;
                self.eval_binary(*op, lval, rval)
            }
            ExprKind::Conditional {
                cond,
                then_expr,
                else_expr,
            } => {
                let cond_val = self.eval_expr(cond, locals, depth)?;
                let cond_bool = cond_val.as_bool().ok_or_else(|| {
                    EvalError::Runtime("ternary condition expects boolean operand".into())
                })?;
                if cond_bool {
                    self.eval_expr(then_expr, locals, depth)
                } else {
                    self.eval_expr(else_expr, locals, depth)
                }
            }
            ExprKind::If { .. }
            | ExprKind::IfLet { .. }
            | ExprKind::Match { .. }
            | ExprKind::OptionSome { .. }
            | ExprKind::OptionNone
            | ExprKind::ResultOk { .. }
            | ExprKind::ResultErr { .. }
            | ExprKind::Propagate { .. } => {
                Err(EvalError::UnsupportedFeature("sum/control-flow expression"))
            }
            ExprKind::Call { name, args } | ExprKind::NamedCall { name, args, .. } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval_expr(arg, locals, depth)?);
                }
                self.eval_call(name, &values, depth)
            }
            ExprKind::Tuple(items) => {
                let mut values = Vec::with_capacity(items.len());
                for item in items {
                    values.push(self.eval_expr(item, locals, depth)?);
                }
                Ok(Value::Tuple(values))
            }
            ExprKind::StructLiteral { fields, .. } => {
                let mut values = Vec::with_capacity(fields.len());
                for (_, value) in fields {
                    values.push(self.eval_expr(value, locals, depth)?);
                }
                Ok(Value::Tuple(values))
            }
            ExprKind::Member { object, field } => {
                let base = self.eval_expr(object, locals, depth)?;
                match base {
                    Value::Tuple(items) => {
                        let idx: usize = field.parse().map_err(|_| {
                            EvalError::Runtime(format!(
                                "tuple field `{field}` is not a valid numeric index"
                            ))
                        })?;
                        items.get(idx).cloned().ok_or_else(|| {
                            EvalError::Runtime(format!(
                                "tuple index {idx} out of bounds (len {})",
                                items.len()
                            ))
                        })
                    }
                    _ => Err(EvalError::UnsupportedFeature("member access")),
                }
            }
            ExprKind::Index { .. } => Err(EvalError::UnsupportedFeature("index expression")),
            ExprKind::JsonObject(_) | ExprKind::JsonArray(_) => {
                Err(EvalError::UnsupportedFeature("native JSON construction"))
            }
        }
    }

    fn eval_binary(
        &self,
        op: crate::ast::BinaryOp,
        left: Value,
        right: Value,
    ) -> Result<Value, EvalError> {
        use crate::ast::BinaryOp;
        match op {
            BinaryOp::Add => {
                let (l, r) = (left.as_int(), right.as_int());
                if let (Some(a), Some(b)) = (l, r) {
                    let value = a
                        .checked_add(b)
                        .map_err(|_| EvalError::ArithmeticOverflow("addition"))?;
                    bounded_int(value, "addition")
                } else {
                    Err(EvalError::Runtime("addition expects integers".into()))
                }
            }
            BinaryOp::Sub => {
                let (l, r) = (left.as_int(), right.as_int());
                if let (Some(a), Some(b)) = (l, r) {
                    let value = a
                        .checked_sub(b)
                        .map_err(|_| EvalError::ArithmeticOverflow("subtraction"))?;
                    bounded_int(value, "subtraction")
                } else {
                    Err(EvalError::Runtime("subtraction expects integers".into()))
                }
            }
            BinaryOp::Mul => {
                let (l, r) = (left.as_int(), right.as_int());
                if let (Some(a), Some(b)) = (l, r) {
                    let value = a
                        .checked_mul(b)
                        .map_err(|_| EvalError::ArithmeticOverflow("multiplication"))?;
                    bounded_int(value, "multiplication")
                } else {
                    Err(EvalError::Runtime("multiplication expects integers".into()))
                }
            }
            BinaryOp::Div => {
                let (l, r) = (left.as_int(), right.as_int());
                if let (Some(a), Some(b)) = (l, r) {
                    if b.is_zero() {
                        return Err(EvalError::Runtime("division by zero".into()));
                    }
                    let (quotient, _) = a
                        .checked_div_rem(b)
                        .map_err(|_| EvalError::ArithmeticOverflow("division"))?;
                    bounded_int(quotient, "division")
                } else {
                    Err(EvalError::Runtime("division expects integers".into()))
                }
            }
            BinaryOp::Mod => {
                let (l, r) = (left.as_int(), right.as_int());
                if let (Some(a), Some(b)) = (l, r) {
                    if b.is_zero() {
                        return Err(EvalError::Runtime("modulo by zero".into()));
                    }
                    let (quotient, remainder) = a
                        .checked_div_rem(b)
                        .map_err(|_| EvalError::ArithmeticOverflow("remainder"))?;
                    IntValueV1::try_new(quotient)
                        .map_err(|_| EvalError::ArithmeticOverflow("remainder"))?;
                    Ok(Value::Int(remainder))
                } else {
                    Err(EvalError::Runtime("modulo expects integers".into()))
                }
            }
            BinaryOp::And => {
                let (l, r) = (left.as_bool(), right.as_bool());
                if let (Some(a), Some(b)) = (l, r) {
                    Ok(Value::Bool(a && b))
                } else {
                    Err(EvalError::Runtime("logical and expects booleans".into()))
                }
            }
            BinaryOp::Or => {
                let (l, r) = (left.as_bool(), right.as_bool());
                if let (Some(a), Some(b)) = (l, r) {
                    Ok(Value::Bool(a || b))
                } else {
                    Err(EvalError::Runtime("logical or expects booleans".into()))
                }
            }
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => {
                let (l, r) = (left.as_int(), right.as_int());
                if let (Some(a), Some(b)) = (l, r) {
                    let result = match op {
                        BinaryOp::Eq => a == b,
                        BinaryOp::Ne => a != b,
                        BinaryOp::Lt => a < b,
                        BinaryOp::Le => a <= b,
                        BinaryOp::Gt => a > b,
                        BinaryOp::Ge => a >= b,
                        _ => unreachable!(),
                    };
                    Ok(Value::Bool(result))
                } else {
                    Err(EvalError::Runtime("comparison expects integers".into()))
                }
            }
        }
    }

    fn eval_call(&mut self, name: &str, args: &[Value], depth: usize) -> Result<Value, EvalError> {
        if self.functions.contains_key(name) {
            self.execute_function(name, args, depth + 1)
        } else {
            Err(EvalError::UnsupportedCall(name.to_string()))
        }
    }
}

#[derive(Debug)]
enum EvalError {
    UnsupportedFeature(&'static str),
    UnsupportedCall(String),
    RecursionLimitExceeded,
    LoopLimitExceeded,
    ArithmeticOverflow(&'static str),
    Runtime(String),
}

#[derive(Debug)]
enum FlowControl {
    Next,
    Return(Value),
    Break,
    ContinueLoop,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_test_fragment as parse;

    fn fuzz(source: &str) -> FuzzReport {
        let program = parse(source).expect("parse");
        let typed = semantic::analyze(&program).expect("typed");
        run_fuzz(&program, &typed, 16)
    }

    #[test]
    fn fuzz_simple_add() {
        let report = fuzz(
            r#"
            fn add(int a, int b) -> int {
                return a + b;
            }
        "#,
        );
        assert_eq!(report.findings.len(), 0);
        assert!(report.cases_executed > 0);
    }

    #[test]
    fn fuzz_interpreter_executes_ints_wider_than_i64() {
        let report = fuzz(
            r#"
            fn wide(int value) -> int {
                return value + 340282366920938463463374607431768211456;
            }
        "#,
        );
        assert!(
            report.findings.is_empty(),
            "signed-512 values must not be skipped or narrowed: {report:?}"
        );
        assert!(report.cases_executed > 0);
    }

    #[test]
    fn fuzz_executes_the_largest_v1_bounded_range() {
        let report = fuzz(
            r#"
            fn spin() {
                for index in range(64) { let value = index; }
            }
        "#,
        );
        assert!(
            !report.findings.iter().any(|f| f.code == "fuzz-loop-limit"),
            "a compiler-proven V1 loop must fit the fuzz evaluator budget: {report:?}"
        );
        assert!(report.cases_executed > 0);
    }

    #[test]
    fn fuzz_skips_unsupported_call() {
        let report = fuzz(
            r#"
            fn call_host() {
                crypto::sha256(b"payload");
            }
        "#,
        );
        assert_eq!(report.cases_executed, 0);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| { matches!(finding.code, "fuzz-skip-call" | "fuzz-skip-feature") }),
            "unsupported host-call fixtures must be skipped deterministically: {report:?}"
        );
    }
}
