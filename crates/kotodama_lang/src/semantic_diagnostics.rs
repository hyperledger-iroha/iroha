//! Span-aware name and type resolution diagnostics for Kotodama V1.
//!
//! This pass runs on the parsed AST and the significant tokens produced by the
//! same lossless scan.  It deliberately precedes typed-HIR construction: name,
//! declaration, simple assignment, and call-graph failures are independent, so
//! reporting one must not hide the others.  The existing semantic pass remains
//! responsible for full builtin-specific and aggregate typing.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{
    ast::{
        BinaryOp, Block, Expr, Function, Item, Pattern, Program, STATE_MAP_GET_INTRINSIC,
        Statement, TypeExpr, UnaryOp,
    },
    builtins::{Builtin, BuiltinMode},
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticLabel, DiagnosticPhase, SourcePosition, SourceSpan,
    },
    lexer::{Token, TokenKind},
    source::{SourceFile, TextRange},
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum ResolvedType {
    Named(String),
    Tuple(Vec<ResolvedType>),
    Unit,
    Unknown,
}

impl ResolvedType {
    fn display(&self) -> String {
        match self {
            Self::Named(name) => name.clone(),
            Self::Tuple(items) => format!(
                "({})",
                items
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Unit => "()".to_owned(),
            Self::Unknown => "<unknown>".to_owned(),
        }
    }

    fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Clone)]
struct Binding {
    ty: ResolvedType,
    mutable: bool,
    span: SourceSpan,
}

#[derive(Clone)]
struct FunctionSignature {
    params: Vec<ResolvedType>,
    return_type: ResolvedType,
    span: SourceSpan,
}

#[derive(Clone, Copy)]
struct FunctionRegion {
    header_start: usize,
    body_start: usize,
    body_end: usize,
}

#[derive(Clone)]
struct CallEdge {
    caller: String,
    callee: String,
    span: SourceSpan,
}

#[derive(Clone)]
struct ExprResult {
    ty: ResolvedType,
    span: Option<SourceSpan>,
}

struct SpanLocator<'source> {
    source: &'source SourceFile,
    tokens: &'source [Token],
    used: BTreeSet<TextRange>,
}

impl<'source> SpanLocator<'source> {
    fn new(source: &'source SourceFile, tokens: &'source [Token]) -> Self {
        Self {
            source,
            tokens,
            used: BTreeSet::new(),
        }
    }

    fn span(&self, range: TextRange) -> SourceSpan {
        SourceSpan::from_range(self.source, range)
    }

    fn span_at_location(&mut self, line: usize, column: usize) -> Option<(usize, SourceSpan)> {
        let (index, token) = self
            .tokens
            .iter()
            .enumerate()
            .find(|(_, token)| token.line == line && token.column == column)?;
        self.used.insert(token.range);
        Some((index, self.span(token.range)))
    }

    fn function_region(&self, function: &Function) -> Option<FunctionRegion> {
        let name_index = self.tokens.iter().position(|token| {
            token.line == function.location.line && token.column == function.location.column
        })?;
        let body_start = (name_index..self.tokens.len())
            .find(|index| matches!(self.tokens[*index].kind, TokenKind::LBrace))?;
        let mut depth = 0_usize;
        let mut body_end = None;
        for index in body_start..self.tokens.len() {
            match self.tokens[index].kind {
                TokenKind::LBrace => depth = depth.saturating_add(1),
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        body_end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        Some(FunctionRegion {
            header_start: name_index,
            body_start,
            body_end: body_end?,
        })
    }

    fn take_name(&mut self, name: &str, start: usize, end: usize) -> Option<SourceSpan> {
        let parts = name.split("::").collect::<Vec<_>>();
        let bounded_end = end.min(self.tokens.len().saturating_sub(1));
        for index in start..=bounded_end {
            let TokenKind::Ident(first) = &self.tokens[index].kind else {
                continue;
            };
            if first != parts[0] || self.used.contains(&self.tokens[index].range) {
                continue;
            }
            let mut last = index;
            let mut matched = true;
            for part in parts.iter().skip(1) {
                if !matches!(
                    self.tokens.get(last + 1).map(|token| &token.kind),
                    Some(TokenKind::ColonColon)
                ) || !matches!(
                    self.tokens.get(last + 2).map(|token| &token.kind),
                    Some(TokenKind::Ident(candidate)) if candidate == part
                ) {
                    matched = false;
                    break;
                }
                last += 2;
            }
            if !matched || last > bounded_end {
                continue;
            }
            let range = TextRange::new(self.tokens[index].range.start, self.tokens[last].range.end);
            self.used.insert(range);
            for token in &self.tokens[index..=last] {
                self.used.insert(token.range);
            }
            return Some(self.span(range));
        }
        None
    }

    fn take_top_level_name(&mut self, name: &str) -> Option<SourceSpan> {
        let mut depth = 0_usize;
        for (index, token) in self.tokens.iter().enumerate() {
            match token.kind {
                TokenKind::LBrace => {
                    depth = depth.saturating_add(1);
                    continue;
                }
                TokenKind::RBrace => {
                    depth = depth.saturating_sub(1);
                    continue;
                }
                _ => {}
            }
            if depth == 1
                && matches!(&token.kind, TokenKind::Ident(candidate) if candidate == name)
                && !self.used.contains(&token.range)
            {
                self.used.insert(token.range);
                return Some(self.span(token.range));
            }
            let _ = index;
        }
        None
    }

    fn take_number(&mut self, value: i64, start: usize, end: usize) -> Option<SourceSpan> {
        if value < 0 {
            return None;
        }
        let bounded_end = end.min(self.tokens.len().saturating_sub(1));
        self.tokens[start..=bounded_end]
            .iter()
            .find(|token| {
                matches!(token.kind, TokenKind::Number(candidate) if candidate == value as u128)
                    && !self.used.contains(&token.range)
            })
            .map(|token| {
                self.used.insert(token.range);
                self.span(token.range)
            })
    }

    fn take_u128(&mut self, raw: &str, start: usize, end: usize) -> Option<SourceSpan> {
        let value = raw.parse::<u128>().ok()?;
        let bounded_end = end.min(self.tokens.len().saturating_sub(1));
        self.tokens[start..=bounded_end]
            .iter()
            .find(|token| {
                matches!(token.kind, TokenKind::Number(candidate) if candidate == value)
                    && !self.used.contains(&token.range)
            })
            .map(|token| {
                self.used.insert(token.range);
                self.span(token.range)
            })
    }

    fn take_keyword(
        &mut self,
        predicate: impl Fn(&TokenKind) -> bool,
        start: usize,
        end: usize,
    ) -> Option<SourceSpan> {
        let bounded_end = end.min(self.tokens.len().saturating_sub(1));
        self.tokens[start..=bounded_end]
            .iter()
            .find(|token| predicate(&token.kind) && !self.used.contains(&token.range))
            .map(|token| {
                self.used.insert(token.range);
                self.span(token.range)
            })
    }
}

struct Audit<'source> {
    locator: SpanLocator<'source>,
    diagnostics: Vec<Diagnostic>,
    known_types: BTreeSet<String>,
    struct_fields: BTreeMap<String, BTreeMap<String, ResolvedType>>,
    declarations: BTreeMap<String, SourceSpan>,
    states: BTreeMap<String, Binding>,
    constants: BTreeMap<String, Binding>,
    functions: BTreeMap<String, FunctionSignature>,
    error_variants: BTreeSet<String>,
    call_edges: Vec<CallEdge>,
}

impl<'source> Audit<'source> {
    fn new(source: &'source SourceFile, tokens: &'source [Token]) -> Self {
        let known_types = [
            "i64",
            "u128",
            "bool",
            "string",
            "bytes",
            "Amount",
            "Json",
            "AccountId",
            "AssetDefinitionId",
            "AssetId",
            "DomainId",
            "Name",
            "NftId",
            "DataSpaceId",
            "Option",
            "Result",
            "StateMap",
            "Secret",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        Self {
            locator: SpanLocator::new(source, tokens),
            diagnostics: Vec::new(),
            known_types,
            struct_fields: BTreeMap::new(),
            declarations: BTreeMap::new(),
            states: BTreeMap::new(),
            constants: BTreeMap::new(),
            functions: BTreeMap::new(),
            error_variants: BTreeSet::new(),
            call_edges: Vec::new(),
        }
    }

    fn push(&mut self, code: &str, message: impl Into<String>, span: Option<SourceSpan>) {
        self.diagnostics.push(Diagnostic::error(
            code,
            DiagnosticPhase::Semantic,
            message,
            span,
        ));
    }

    fn duplicate(
        &mut self,
        message: impl Into<String>,
        current: SourceSpan,
        previous: &SourceSpan,
    ) {
        let mut diagnostic =
            Diagnostic::error("K2001", DiagnosticPhase::Semantic, message, Some(current));
        diagnostic.labels.push(DiagnosticLabel {
            span: previous.clone(),
            message: "the first declaration is here".to_owned(),
        });
        self.diagnostics.push(diagnostic);
    }

    fn item_name(item: &Item) -> Option<(&str, &'static str, bool)> {
        match item {
            Item::Function(value) => Some((&value.name, "function", true)),
            Item::Struct(value) => Some((&value.name, "type", false)),
            Item::ErrorEnum(value) => Some((&value.name, "error enum", false)),
            Item::Const(value) => Some((&value.name, "constant", false)),
            Item::State(value) => Some((&value.name, "state", false)),
            Item::Trigger(value) => Some((&value.name, "trigger", false)),
        }
    }

    fn declaration_span(&mut self, item: &Item) -> Option<SourceSpan> {
        match item {
            Item::Function(function) => self
                .locator
                .span_at_location(function.location.line, function.location.column)
                .map(|(_, span)| span),
            _ => Self::item_name(item)
                .and_then(|(name, _, _)| self.locator.take_top_level_name(name)),
        }
    }

    fn collect_declarations(&mut self, program: &Program) {
        let unit_span = self
            .locator
            .take_top_level_name(&program.unit.name)
            .unwrap_or_else(|| SourceSpan::from_range(self.locator.source, TextRange::empty(0)));
        if crate::semantic::is_reserved_source_declaration(&program.unit.name, false) {
            self.push(
                "E_RESERVED_DECLARATION",
                format!(
                    "source unit `{}` uses a compiler-reserved name",
                    program.unit.name
                ),
                Some(unit_span.clone()),
            );
        }
        self.declarations
            .insert(program.unit.name.clone(), unit_span);

        for item in &program.items {
            if let Item::Struct(definition) = item {
                self.known_types.insert(definition.name.clone());
            }
        }

        for item in &program.items {
            let Some((name, kind, is_function)) = Self::item_name(item) else {
                continue;
            };
            let span = self.declaration_span(item).unwrap_or_else(|| {
                SourceSpan::from_range(self.locator.source, TextRange::empty(0))
            });
            if crate::semantic::is_reserved_source_declaration(name, is_function) {
                self.push(
                    "E_RESERVED_DECLARATION",
                    format!("{kind} `{name}` uses a compiler-reserved name"),
                    Some(span.clone()),
                );
            }
            if let Some(previous) = self.declarations.get(name).cloned() {
                self.duplicate(
                    format!("declaration `{name}` duplicates an earlier declaration"),
                    span.clone(),
                    &previous,
                );
            } else {
                self.declarations.insert(name.to_owned(), span.clone());
            }

            match item {
                Item::State(state) => {
                    self.states.insert(
                        state.name.clone(),
                        Binding {
                            ty: resolve_type(&state.ty),
                            mutable: true,
                            span,
                        },
                    );
                }
                Item::Const(constant) => {
                    self.constants.insert(
                        constant.name.clone(),
                        Binding {
                            ty: constant
                                .ty
                                .as_ref()
                                .map_or(ResolvedType::Unknown, resolve_type),
                            mutable: false,
                            span,
                        },
                    );
                }
                Item::Function(function) => {
                    self.functions.insert(
                        function.name.clone(),
                        FunctionSignature {
                            params: function
                                .params
                                .iter()
                                .map(|param| {
                                    param
                                        .ty
                                        .as_ref()
                                        .map_or(ResolvedType::Unknown, resolve_type)
                                })
                                .collect(),
                            return_type: function
                                .ret_ty
                                .as_ref()
                                .map_or(ResolvedType::Unit, resolve_type),
                            span,
                        },
                    );
                }
                Item::ErrorEnum(definition) => {
                    for variant in &definition.variants {
                        self.error_variants
                            .insert(format!("{}::{}", definition.name, variant.name));
                    }
                }
                Item::Struct(definition) => {
                    self.struct_fields.insert(
                        definition.name.clone(),
                        definition
                            .fields
                            .iter()
                            .map(|(field, ty)| (field.clone(), resolve_type(ty)))
                            .collect(),
                    );
                }
                Item::Trigger(_) => {}
            }
        }
    }

    fn audit_types(&mut self, program: &Program) {
        let mut dependencies = BTreeMap::<String, BTreeSet<String>>::new();
        for item in &program.items {
            match item {
                Item::Struct(definition) => {
                    let span = self
                        .declarations
                        .get(&definition.name)
                        .cloned()
                        .unwrap_or_else(|| {
                            SourceSpan::from_range(self.locator.source, TextRange::empty(0))
                        });
                    let region = self.region_after_span(&span);
                    for (_, ty) in &definition.fields {
                        self.audit_type_expr(ty, region);
                        collect_type_dependencies(ty, &self.known_types, &mut dependencies)
                            .into_iter()
                            .for_each(|dependency| {
                                dependencies
                                    .entry(definition.name.clone())
                                    .or_default()
                                    .insert(dependency);
                            });
                    }
                }
                Item::Function(function) => {
                    let region = self.locator.function_region(function);
                    let fallback = (0, self.locator.tokens.len().saturating_sub(1));
                    let header_bounds = region
                        .map(|region| (region.header_start, region.body_start))
                        .unwrap_or(fallback);
                    let body_bounds = region
                        .map(|region| (region.body_start, region.body_end))
                        .unwrap_or(fallback);
                    for param in &function.params {
                        if let Some(ty) = &param.ty {
                            self.audit_type_expr(ty, header_bounds);
                        }
                    }
                    if let Some(ty) = &function.ret_ty {
                        self.audit_type_expr(ty, header_bounds);
                    }
                    self.audit_statement_types(&function.body, body_bounds);
                }
                Item::Const(constant) => {
                    if let Some(ty) = &constant.ty {
                        self.audit_type_expr(ty, (0, self.locator.tokens.len().saturating_sub(1)));
                    }
                }
                Item::State(state) => {
                    self.audit_type_expr(
                        &state.ty,
                        (0, self.locator.tokens.len().saturating_sub(1)),
                    );
                }
                Item::ErrorEnum(_) | Item::Trigger(_) => {}
            }
        }
        self.audit_cycles(&dependencies, "recursive value type");
    }

    fn region_after_span(&self, span: &SourceSpan) -> (usize, usize) {
        let start_offset = span.byte_range.map_or(0, |range| range.end);
        let start = self
            .locator
            .tokens
            .iter()
            .position(|token| token.range.start >= start_offset)
            .unwrap_or(0);
        (start, self.locator.tokens.len().saturating_sub(1))
    }

    fn audit_type_expr(&mut self, ty: &TypeExpr, region: (usize, usize)) {
        match ty {
            TypeExpr::Path(name) => {
                if !self.known_types.contains(name) {
                    let span = self.locator.take_name(name, region.0, region.1);
                    self.push("K2002", format!("unknown type `{name}`"), span);
                }
            }
            TypeExpr::Generic { base, args } => {
                if !matches!(base.as_str(), "Option" | "Result" | "StateMap" | "Secret") {
                    let span = self.locator.take_name(base, region.0, region.1);
                    self.push("K2002", format!("unknown generic type `{base}`"), span);
                }
                for arg in args {
                    self.audit_type_expr(arg, region);
                }
            }
            TypeExpr::Tuple(items) => {
                for item in items {
                    self.audit_type_expr(item, region);
                }
            }
        }
    }

    fn audit_statement_types(&mut self, block: &Block, region: (usize, usize)) {
        for statement in &block.statements {
            match statement {
                Statement::Let { ty: Some(ty), .. } => self.audit_type_expr(ty, region),
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.audit_statement_types(then_branch, region);
                    if let Some(branch) = else_branch {
                        self.audit_statement_types(branch, region);
                    }
                }
                Statement::While { body, .. }
                | Statement::For { body, .. }
                | Statement::ForEachMap { body, .. } => {
                    self.audit_statement_types(body, region);
                }
                Statement::Let { ty: None, .. }
                | Statement::Assign { .. }
                | Statement::AssignExpr { .. }
                | Statement::Expr(_)
                | Statement::Return(_)
                | Statement::Break
                | Statement::Continue => {}
            }
        }
    }

    fn audit_cycles(&mut self, graph: &BTreeMap<String, BTreeSet<String>>, description: &str) {
        for component in strongly_connected_components(graph) {
            let recursive = component.len() > 1
                || component
                    .first()
                    .is_some_and(|name| graph.get(name).is_some_and(|edges| edges.contains(name)));
            if !recursive {
                continue;
            }
            let Some(primary_name) = component.first() else {
                continue;
            };
            let primary = self.declarations.get(primary_name).cloned();
            let mut diagnostic = Diagnostic::error(
                "K2006",
                DiagnosticPhase::Semantic,
                format!("{description} cycle: {}", component.join(" -> ")),
                primary,
            );
            for name in component.iter().skip(1) {
                if let Some(span) = self.declarations.get(name) {
                    diagnostic.labels.push(DiagnosticLabel {
                        span: span.clone(),
                        message: format!("`{name}` participates in this cycle"),
                    });
                }
            }
            self.diagnostics.push(diagnostic);
        }
    }

    fn audit_functions(&mut self, program: &Program) {
        let struct_types = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Struct(definition) => Some((
                    definition.name.clone(),
                    definition
                        .fields
                        .iter()
                        .map(|(_, ty)| ty.clone())
                        .collect::<Vec<_>>(),
                )),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        for item in &program.items {
            let Item::Function(function) = item else {
                continue;
            };
            let Some(region) = self.locator.function_region(function) else {
                continue;
            };
            let mut bindings = self.states.clone();
            bindings.extend(self.constants.clone());
            let signature = self.functions.get(&function.name).cloned();
            let argument_layout = function
                .params
                .iter()
                .map(|param| {
                    if param.is_state {
                        Some(1)
                    } else {
                        param.ty.as_ref().and_then(|ty| {
                            argument_word_count(ty, &struct_types, &mut BTreeSet::new())
                        })
                    }
                })
                .collect::<Option<Vec<_>>>()
                .and_then(|counts| {
                    let total = counts
                        .iter()
                        .try_fold(0_usize, |total, count| total.checked_add(*count))?;
                    let mut running = 0_usize;
                    let crossing = counts.iter().position(|count| {
                        running = running.saturating_add(*count);
                        running > crate::regalloc::MAX_ARGUMENT_VALUES
                    });
                    Some((counts, total, crossing))
                });
            for (index, param) in function.params.iter().enumerate() {
                let span = self
                    .locator
                    .take_name(
                        &param.name,
                        region.header_start,
                        region.body_start.saturating_sub(1),
                    )
                    .unwrap_or_else(|| {
                        self.declarations
                            .get(&function.name)
                            .cloned()
                            .unwrap_or_else(|| {
                                SourceSpan::from_range(self.locator.source, TextRange::empty(0))
                            })
                    });
                if let Some((counts, total, Some(crossing))) = &argument_layout
                    && *total > crate::regalloc::MAX_ARGUMENT_VALUES
                    && index == *crossing
                {
                    let mut diagnostic = Diagnostic::error(
                        "K2007",
                        DiagnosticPhase::Semantic,
                        format!(
                            "function `{}` requires {total} flattened argument words, exceeding the Kotodama V1 limit of {}",
                            function.name,
                            crate::regalloc::MAX_ARGUMENT_VALUES,
                        ),
                        Some(span.clone()),
                    );
                    if let Some(signature) = &signature {
                        diagnostic.labels.push(DiagnosticLabel {
                            span: signature.span.clone(),
                            message: "this function uses the fixed r10-r22 argument window"
                                .to_owned(),
                        });
                    }
                    diagnostic.notes.push(format!(
                        "parameter `{}` contributes {} recursively flattened word(s)",
                        param.name, counts[index]
                    ));
                    self.diagnostics.push(diagnostic);
                }
                let previous = bindings.get(&param.name).cloned().or_else(|| {
                    self.declarations
                        .get(&param.name)
                        .cloned()
                        .map(|span| Binding {
                            ty: ResolvedType::Unknown,
                            mutable: false,
                            span,
                        })
                });
                if let Some(previous) = previous {
                    self.duplicate(
                        format!("parameter `{}` shadows an existing name", param.name),
                        span.clone(),
                        &previous.span,
                    );
                } else {
                    bindings.insert(
                        param.name.clone(),
                        Binding {
                            ty: signature
                                .as_ref()
                                .and_then(|signature| signature.params.get(index))
                                .cloned()
                                .unwrap_or(ResolvedType::Unknown),
                            mutable: false,
                            span,
                        },
                    );
                }
            }
            let expected_return = signature.as_ref().map_or(ResolvedType::Unit, |signature| {
                signature.return_type.clone()
            });
            self.audit_block(
                &function.name,
                &function.body,
                region,
                &expected_return,
                &mut bindings,
            );
        }

        let mut graph = self
            .functions
            .keys()
            .cloned()
            .map(|name| (name, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        for edge in &self.call_edges {
            graph
                .entry(edge.caller.clone())
                .or_default()
                .insert(edge.callee.clone());
        }
        for component in strongly_connected_components(&graph) {
            let recursive = component.len() > 1
                || component
                    .first()
                    .is_some_and(|name| graph.get(name).is_some_and(|edges| edges.contains(name)));
            if !recursive {
                continue;
            }
            let component_set = component.iter().cloned().collect::<BTreeSet<_>>();
            let primary_edge = self.call_edges.iter().find(|edge| {
                component_set.contains(&edge.caller) && component_set.contains(&edge.callee)
            });
            let mut diagnostic = Diagnostic::error(
                "K2006",
                DiagnosticPhase::Semantic,
                format!(
                    "recursive function calls are not supported in Kotodama V1: {}",
                    component.join(" -> ")
                ),
                primary_edge.map(|edge| edge.span.clone()),
            );
            for name in &component {
                if let Some(signature) = self.functions.get(name) {
                    diagnostic.labels.push(DiagnosticLabel {
                        span: signature.span.clone(),
                        message: format!("function `{name}` is in this call cycle"),
                    });
                }
            }
            self.diagnostics.push(diagnostic);
        }
    }

    fn audit_block(
        &mut self,
        function: &str,
        block: &Block,
        region: FunctionRegion,
        expected_return: &ResolvedType,
        bindings: &mut BTreeMap<String, Binding>,
    ) {
        for statement in &block.statements {
            match statement {
                Statement::Let {
                    mutable,
                    pat,
                    ty,
                    value,
                } => {
                    let value = self.audit_expr(function, value, region, bindings);
                    let declared = ty.as_ref().map(resolve_type);
                    if let Some(expected) = &declared {
                        self.type_mismatch(expected, &value, "binding initializer");
                    }
                    let binding_type = declared.unwrap_or(value.ty);
                    let names = match pat {
                        Pattern::Name(name) => vec![name.as_str()],
                        Pattern::Tuple(names) => names.iter().map(String::as_str).collect(),
                    };
                    for name in names {
                        if name == "_" {
                            continue;
                        }
                        let span = self
                            .locator
                            .take_name(name, region.body_start, region.body_end)
                            .or_else(|| value.span.clone())
                            .unwrap_or_else(|| {
                                SourceSpan::from_range(self.locator.source, TextRange::empty(0))
                            });
                        let previous = bindings.get(name).cloned().or_else(|| {
                            self.declarations.get(name).cloned().map(|span| Binding {
                                ty: ResolvedType::Unknown,
                                mutable: false,
                                span,
                            })
                        });
                        if let Some(previous) = previous {
                            self.duplicate(
                                format!("binding `{name}` shadows an existing name"),
                                span.clone(),
                                &previous.span,
                            );
                        } else {
                            bindings.insert(
                                name.to_owned(),
                                Binding {
                                    ty: binding_type.clone(),
                                    mutable: *mutable,
                                    span,
                                },
                            );
                        }
                    }
                }
                Statement::Assign { name, value } => {
                    let target_span =
                        self.locator
                            .take_name(name, region.body_start, region.body_end);
                    let value = self.audit_expr(function, value, region, bindings);
                    match bindings.get(name).cloned() {
                        None => self.push(
                            "K2002",
                            format!("unknown assignment target `{name}`"),
                            target_span,
                        ),
                        Some(binding) => {
                            if !binding.mutable {
                                let mut diagnostic = Diagnostic::error(
                                    "E_IMMUTABLE_ASSIGNMENT",
                                    DiagnosticPhase::Semantic,
                                    format!(
                                        "cannot assign to immutable binding `{name}`; declare it with `var`"
                                    ),
                                    target_span,
                                );
                                diagnostic.labels.push(DiagnosticLabel {
                                    span: binding.span,
                                    message: "this binding is immutable".to_owned(),
                                });
                                self.diagnostics.push(diagnostic);
                            }
                            self.type_mismatch(&binding.ty, &value, "assignment");
                        }
                    }
                }
                Statement::AssignExpr { target, value, .. } => {
                    self.audit_expr(function, target, region, bindings);
                    self.audit_expr(function, value, region, bindings);
                }
                Statement::Expr(expr) => {
                    self.audit_expr(function, expr, region, bindings);
                }
                Statement::Return(value) => {
                    let result = value.as_ref().map_or(
                        ExprResult {
                            ty: ResolvedType::Unit,
                            span: self.locator.take_keyword(
                                |kind| matches!(kind, TokenKind::Return),
                                region.body_start,
                                region.body_end,
                            ),
                        },
                        |value| self.audit_expr(function, value, region, bindings),
                    );
                    self.type_mismatch(expected_return, &result, "return value");
                }
                Statement::If {
                    cond,
                    then_branch,
                    else_branch,
                } => {
                    let condition = self.audit_expr(function, cond, region, bindings);
                    self.type_mismatch(
                        &ResolvedType::Named("bool".to_owned()),
                        &condition,
                        "if condition",
                    );
                    self.audit_block(function, then_branch, region, expected_return, bindings);
                    if let Some(branch) = else_branch {
                        self.audit_block(function, branch, region, expected_return, bindings);
                    }
                }
                Statement::While { cond, body } => {
                    let span = self.locator.take_keyword(
                        |kind| matches!(kind, TokenKind::Ident(name) if name == "while"),
                        region.body_start,
                        region.body_end,
                    );
                    self.push(
                        "E_UNBOUNDED_LOOP",
                        "`while` is not part of Kotodama V1; use a compiler-proven bounded `for` loop",
                        span,
                    );
                    self.audit_expr(function, cond, region, bindings);
                    self.audit_block(function, body, region, expected_return, bindings);
                }
                Statement::For {
                    init,
                    cond,
                    step,
                    body,
                    ..
                } => {
                    if !is_v1_bounded_for_shape(init, cond, step) {
                        let span = self.locator.take_keyword(
                            |kind| matches!(kind, TokenKind::For),
                            region.body_start,
                            region.body_end,
                        );
                        self.push(
                            "E_UNBOUNDED_LOOP",
                            "for loop is not in the compiler-proven bounded V1 form",
                            span,
                        );
                    }
                    if let Some(Statement::Let { pat, .. }) = init.as_deref() {
                        let names = match pat {
                            Pattern::Name(name) => vec![name.as_str()],
                            Pattern::Tuple(names) => names.iter().map(String::as_str).collect(),
                        };
                        for name in names {
                            if name != "_" && !bindings.contains_key(name) {
                                let span = self
                                    .locator
                                    .take_name(name, region.body_start, region.body_end)
                                    .unwrap_or_else(|| {
                                        SourceSpan::from_range(
                                            self.locator.source,
                                            TextRange::empty(0),
                                        )
                                    });
                                bindings.insert(
                                    name.to_owned(),
                                    Binding {
                                        ty: ResolvedType::Named("i64".to_owned()),
                                        mutable: true,
                                        span,
                                    },
                                );
                            }
                        }
                    }
                    self.audit_block(function, body, region, expected_return, bindings);
                }
                Statement::ForEachMap {
                    key,
                    value,
                    map,
                    body,
                    ..
                } => {
                    // `take` and `range` are syntax-owned iteration bounds, not
                    // source-callable builtins. Audit their receiver and bound
                    // expressions while leaving the complete shape and literal
                    // proof to the semantic pass.
                    match map {
                        Expr::Call { name, args }
                            if (name == "take" && args.len() == 2)
                                || (name == "range" && args.len() == 3) =>
                        {
                            for argument in args {
                                self.audit_expr(function, argument, region, bindings);
                            }
                        }
                        _ => {
                            self.audit_expr(function, map, region, bindings);
                        }
                    }
                    for name in std::iter::once(key).chain(value.iter()) {
                        if !bindings.contains_key(name) {
                            let span = self
                                .locator
                                .take_name(name, region.body_start, region.body_end)
                                .unwrap_or_else(|| {
                                    SourceSpan::from_range(self.locator.source, TextRange::empty(0))
                                });
                            bindings.insert(
                                name.clone(),
                                Binding {
                                    ty: ResolvedType::Unknown,
                                    mutable: false,
                                    span,
                                },
                            );
                        }
                    }
                    self.audit_block(function, body, region, expected_return, bindings);
                }
                Statement::Break | Statement::Continue => {}
            }
        }
    }

    fn audit_expr(
        &mut self,
        function: &str,
        expr: &Expr,
        region: FunctionRegion,
        bindings: &BTreeMap<String, Binding>,
    ) -> ExprResult {
        match expr {
            Expr::Ident(name) => {
                let span = self
                    .locator
                    .take_name(name, region.body_start, region.body_end);
                if let Some(binding) = bindings.get(name) {
                    ExprResult {
                        ty: binding.ty.clone(),
                        span,
                    }
                } else if self.error_variants.contains(name) {
                    ExprResult {
                        ty: ResolvedType::Unknown,
                        span,
                    }
                } else {
                    self.push("K2002", format!("unknown name `{name}`"), span.clone());
                    ExprResult {
                        ty: ResolvedType::Unknown,
                        span,
                    }
                }
            }
            Expr::Call { name, args } => {
                let span = self
                    .locator
                    .take_name(name, region.body_start, region.body_end);
                let arguments = args
                    .iter()
                    .map(|arg| self.audit_expr(function, arg, region, bindings))
                    .collect::<Vec<_>>();
                if name == STATE_MAP_GET_INTRINSIC {
                    // Full aggregate typing in the semantic pass proves the
                    // receiver/key types and resolves the precise Option<V>.
                    // The parser emits this compiler-owned marker only for the
                    // canonical `map.get(key)` method form.
                    return ExprResult {
                        ty: ResolvedType::Unknown,
                        span,
                    };
                }
                if matches!(
                    name.as_str(),
                    "is_some"
                        | "is_none"
                        | "is_ok"
                        | "is_err"
                        | "unwrap_or"
                        | "unwrap_err_or"
                        | "option::some"
                        | "option::none"
                        | "result::ok"
                        | "result::err"
                        | "u128::from_i64"
                        | "Amount::from_i64"
                        | "Amount::from_u128"
                ) {
                    // These are typed language intrinsics rather than host
                    // capabilities. Method-only spellings are introduced by
                    // the parser, while the full semantic pass validates the
                    // receiver, generic payload, and result type.
                    return ExprResult {
                        ty: ResolvedType::Unknown,
                        span,
                    };
                }
                if let Some(builtin) = Builtin::from_name(name)
                    && builtin.spec().mode == BuiltinMode::CompilerInternal
                {
                    self.push(
                        "E_INTERNAL_BUILTIN",
                        format!(
                            "builtin `{name}` is compiler-internal and is not available in Kotodama V1 source"
                        ),
                        span.clone(),
                    );
                    return ExprResult {
                        ty: ResolvedType::Unknown,
                        span,
                    };
                }
                if let Some(signature) = self.functions.get(name).cloned() {
                    self.call_edges.push(CallEdge {
                        caller: function.to_owned(),
                        callee: name.clone(),
                        span: span.clone().unwrap_or_else(|| signature.span.clone()),
                    });
                    for (argument, expected) in arguments.iter().zip(&signature.params) {
                        self.type_mismatch(expected, argument, "function argument");
                    }
                    return ExprResult {
                        ty: signature.return_type,
                        span,
                    };
                }
                if self.known_types.contains(name) {
                    return ExprResult {
                        ty: ResolvedType::Named(name.clone()),
                        span,
                    };
                }
                if matches!(name.as_str(), "Some" | "None" | "Ok" | "Err") {
                    return ExprResult {
                        ty: ResolvedType::Unknown,
                        span,
                    };
                }
                let builtin = Builtin::from_source_name(name).or_else(|| {
                    Builtin::from_name(name)
                        .filter(|value| value.spec().mode != BuiltinMode::CompilerInternal)
                });
                if let Some(builtin) = builtin {
                    return ExprResult {
                        ty: builtin_return_type(builtin),
                        span,
                    };
                }
                self.push(
                    "K2002",
                    format!("unknown function or builtin `{name}`"),
                    span.clone(),
                );
                ExprResult {
                    ty: ResolvedType::Unknown,
                    span,
                }
            }
            Expr::Binary { op, left, right } => {
                let left = self.audit_expr(function, left, region, bindings);
                let right = self.audit_expr(function, right, region, bindings);
                let ty = match op {
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => ResolvedType::Named("bool".to_owned()),
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod => {
                        if left.ty == right.ty {
                            left.ty.clone()
                        } else {
                            ResolvedType::Unknown
                        }
                    }
                };
                ExprResult {
                    ty,
                    span: left.span.or(right.span),
                }
            }
            Expr::Unary { op, expr } => {
                let value = self.audit_expr(function, expr, region, bindings);
                let ty = match op {
                    UnaryOp::Not => ResolvedType::Named("bool".to_owned()),
                    UnaryOp::Neg => value.ty.clone(),
                };
                ExprResult {
                    ty,
                    span: value.span,
                }
            }
            Expr::Conditional {
                cond,
                then_expr,
                else_expr,
            } => {
                let condition = self.audit_expr(function, cond, region, bindings);
                self.type_mismatch(
                    &ResolvedType::Named("bool".to_owned()),
                    &condition,
                    "conditional condition",
                );
                let then_value = self.audit_expr(function, then_expr, region, bindings);
                let else_value = self.audit_expr(function, else_expr, region, bindings);
                ExprResult {
                    ty: if then_value.ty == else_value.ty {
                        then_value.ty.clone()
                    } else {
                        ResolvedType::Unknown
                    },
                    span: then_value.span.or(else_value.span),
                }
            }
            Expr::Member { object, field } => {
                let object = self.audit_expr(function, object, region, bindings);
                let ty = match &object.ty {
                    ResolvedType::Named(name) => self
                        .struct_fields
                        .get(name)
                        .and_then(|fields| fields.get(field))
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown),
                    ResolvedType::Tuple(items) => field
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| items.get(index))
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown),
                    ResolvedType::Unit | ResolvedType::Unknown => ResolvedType::Unknown,
                };
                ExprResult {
                    ty,
                    span: object.span,
                }
            }
            Expr::Index { target, index } => {
                let target = self.audit_expr(function, target, region, bindings);
                self.audit_expr(function, index, region, bindings);
                ExprResult {
                    ty: ResolvedType::Unknown,
                    span: target.span,
                }
            }
            Expr::Tuple(items) => {
                let values = items
                    .iter()
                    .map(|item| self.audit_expr(function, item, region, bindings))
                    .collect::<Vec<_>>();
                ExprResult {
                    ty: ResolvedType::Tuple(values.iter().map(|value| value.ty.clone()).collect()),
                    span: values.into_iter().find_map(|value| value.span),
                }
            }
            Expr::Bool(_) => ExprResult {
                ty: ResolvedType::Named("bool".to_owned()),
                span: self.locator.take_keyword(
                    |kind| matches!(kind, TokenKind::True | TokenKind::False),
                    region.body_start,
                    region.body_end,
                ),
            },
            Expr::Number(value) => ExprResult {
                ty: ResolvedType::Named("i64".to_owned()),
                span: self
                    .locator
                    .take_number(*value, region.body_start, region.body_end),
            },
            Expr::Decimal(raw) => ExprResult {
                ty: ResolvedType::Named("u128".to_owned()),
                span: self
                    .locator
                    .take_u128(raw, region.body_start, region.body_end),
            },
            Expr::String(_) => ExprResult {
                ty: ResolvedType::Named("string".to_owned()),
                span: None,
            },
            Expr::Bytes(_) => ExprResult {
                ty: ResolvedType::Named("bytes".to_owned()),
                span: None,
            },
        }
    }

    fn type_mismatch(&mut self, expected: &ResolvedType, actual: &ExprResult, context: &str) {
        if expected.is_known() && actual.ty.is_known() && expected != &actual.ty {
            self.push(
                "K2003",
                format!(
                    "{context} has type `{}`, expected `{}`; implicit conversions are not part of Kotodama V1",
                    actual.ty.display(),
                    expected.display()
                ),
                actual.span.clone(),
            );
        }
    }
}

fn resolve_type(ty: &TypeExpr) -> ResolvedType {
    match ty {
        TypeExpr::Path(name) => ResolvedType::Named(name.clone()),
        TypeExpr::Generic { base, args } => ResolvedType::Named(format!(
            "{base}<{}>",
            args.iter()
                .map(|arg| resolve_type(arg).display())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        TypeExpr::Tuple(items) if items.is_empty() => ResolvedType::Unit,
        TypeExpr::Tuple(items) => ResolvedType::Tuple(items.iter().map(resolve_type).collect()),
    }
}

fn argument_word_count(
    ty: &TypeExpr,
    structs: &BTreeMap<String, Vec<TypeExpr>>,
    visiting: &mut BTreeSet<String>,
) -> Option<usize> {
    fn sum<'a>(
        types: impl IntoIterator<Item = &'a TypeExpr>,
        structs: &BTreeMap<String, Vec<TypeExpr>>,
        visiting: &mut BTreeSet<String>,
    ) -> Option<usize> {
        types.into_iter().try_fold(0_usize, |total, ty| {
            total.checked_add(argument_word_count(ty, structs, visiting)?)
        })
    }

    match ty {
        TypeExpr::Path(name) => {
            let Some(fields) = structs.get(name) else {
                return Some(1);
            };
            if !visiting.insert(name.clone()) {
                // The independent type-cycle diagnostic owns this malformed
                // case; do not recurse or emit a misleading size diagnostic.
                return None;
            }
            let count = sum(fields, structs, visiting);
            visiting.remove(name);
            count
        }
        TypeExpr::Generic { base, args } if base == "Option" && args.len() == 1 => {
            1_usize.checked_add(argument_word_count(&args[0], structs, visiting)?)
        }
        TypeExpr::Generic { base, args } if base == "Result" && args.len() == 2 => 1_usize
            .checked_add(argument_word_count(&args[0], structs, visiting)?)?
            .checked_add(argument_word_count(&args[1], structs, visiting)?),
        // Secret and any independently rejected generic forms remain one
        // runtime value. StateMap parameters are rejected elsewhere in V1.
        TypeExpr::Generic { .. } => Some(1),
        TypeExpr::Tuple(items) if items.is_empty() => Some(1),
        TypeExpr::Tuple(items) => sum(items, structs, visiting),
    }
}

fn builtin_return_type(builtin: Builtin) -> ResolvedType {
    let return_type = builtin.spec().signature.return_type;
    match return_type {
        "()" => ResolvedType::Unit,
        "i64" | "u128" | "bool" | "string" | "bytes" | "Amount" | "Json" | "AccountId"
        | "AssetDefinitionId" | "AssetId" | "DomainId" | "Name" | "NftId" | "DataSpaceId" => {
            ResolvedType::Named(return_type.to_owned())
        }
        _ => ResolvedType::Unknown,
    }
}

fn collect_type_dependencies(
    ty: &TypeExpr,
    known_types: &BTreeSet<String>,
    dependencies: &mut BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let _ = dependencies;
    match ty {
        TypeExpr::Path(name)
            if known_types.contains(name)
                && !matches!(
                    name.as_str(),
                    "i64"
                        | "u128"
                        | "bool"
                        | "string"
                        | "bytes"
                        | "Amount"
                        | "Json"
                        | "AccountId"
                        | "AssetDefinitionId"
                        | "AssetId"
                        | "DomainId"
                        | "Name"
                        | "NftId"
                        | "DataSpaceId"
                        | "Option"
                        | "Result"
                        | "StateMap"
                        | "Secret"
                ) =>
        {
            vec![name.clone()]
        }
        TypeExpr::Generic { args, .. } | TypeExpr::Tuple(args) => args
            .iter()
            .flat_map(|arg| collect_type_dependencies(arg, known_types, dependencies))
            .collect(),
        TypeExpr::Path(_) => Vec::new(),
    }
}

fn strongly_connected_components(graph: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        seen: &mut HashSet<String>,
        order: &mut Vec<String>,
    ) {
        if !seen.insert(node.to_owned()) {
            return;
        }
        if let Some(edges) = graph.get(node) {
            for edge in edges {
                if graph.contains_key(edge) {
                    visit(edge, graph, seen, order);
                }
            }
        }
        order.push(node.to_owned());
    }

    let mut order = Vec::new();
    let mut seen = HashSet::new();
    for node in graph.keys() {
        visit(node, graph, &mut seen, &mut order);
    }
    let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();
    for node in graph.keys() {
        reverse.entry(node.clone()).or_default();
    }
    for (node, edges) in graph {
        for edge in edges {
            if graph.contains_key(edge) {
                reverse
                    .entry(edge.clone())
                    .or_default()
                    .insert(node.clone());
            }
        }
    }
    let mut components = Vec::new();
    seen.clear();
    while let Some(node) = order.pop() {
        if seen.contains(&node) {
            continue;
        }
        let mut stack = vec![node];
        let mut component = Vec::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            component.push(current.clone());
            if let Some(edges) = reverse.get(&current) {
                stack.extend(edges.iter().rev().cloned());
            }
        }
        component.sort();
        components.push(component);
    }
    components.sort_by(|left, right| left.first().cmp(&right.first()));
    components
}

fn is_v1_bounded_for_shape(
    init: &Option<Box<Statement>>,
    cond: &Option<Expr>,
    step: &Option<Box<Statement>>,
) -> bool {
    let Some(Statement::Let {
        mutable: true,
        pat: Pattern::Name(binding),
        value: Expr::Number(0),
        ..
    }) = init.as_deref()
    else {
        return false;
    };
    let Some(Expr::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    }) = cond
    else {
        return false;
    };
    if !matches!(left.as_ref(), Expr::Ident(name) if name == binding)
        || !matches!(right.as_ref(), Expr::Number(value) if *value >= 0)
    {
        return false;
    }
    matches!(
        step.as_deref(),
        Some(Statement::Assign {
            name,
            value: Expr::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
        }) if name == binding
            && matches!(left.as_ref(), Expr::Ident(value) if value == binding)
            && matches!(right.as_ref(), Expr::Number(1))
    )
}

/// Run the independent, span-aware V1 resolver checks.
pub(crate) fn audit(
    program: &Program,
    source: &SourceFile,
    tokens: &[Token],
) -> Result<(), DiagnosticBundle> {
    let mut audit = Audit::new(source, tokens);
    audit.collect_declarations(program);
    audit.audit_types(program);
    audit.audit_functions(program);
    if audit.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(DiagnosticBundle::new(audit.diagnostics))
    }
}

pub(crate) fn semantic_error_parts(message: String) -> (String, String) {
    if let Some((candidate, rest)) = message.split_once(':')
        && candidate.starts_with("E_")
        && candidate
            .chars()
            .all(|character| character == '_' || character.is_ascii_uppercase())
    {
        return (candidate.to_owned(), rest.trim_start().to_owned());
    }
    let code = if message.starts_with("duplicate ") || message.contains(" shadows ") {
        "K2001"
    } else if message.contains("undefined")
        || message.contains("unknown ")
        || message.contains("not declared")
    {
        "K2002"
    } else if message.contains("type")
        || message.contains("expects")
        || message.contains("assignable")
        || message.contains("immutable")
    {
        "K2003"
    } else if message.contains("permission")
        || message.contains("authorize")
        || message.starts_with("view function")
    {
        "K2004"
    } else if message.contains("state") {
        "K2005"
    } else if message.contains("recursive") || message.contains("cyclic") {
        "K2006"
    } else {
        "K2099"
    };
    (code.to_owned(), message)
}

/// Convert remaining typed-analysis failures into the same canonical
/// diagnostic representation used by the independent resolver.
pub(crate) fn from_semantic_failures(
    failures: crate::semantic::SemanticFailures,
    source_name: Option<&str>,
    source: Option<&SourceFile>,
    tokens: Option<&[Token]>,
) -> DiagnosticBundle {
    let mut locator = source
        .zip(tokens)
        .map(|(source, tokens)| SpanLocator::new(source, tokens));
    let diagnostics = failures
        .failures
        .into_iter()
        .map(|failure| {
            let (code, message) = semantic_error_parts(failure.error.message);
            let primary_span = if code == "K0004" {
                None
            } else if let Some(location) = failure.location {
                locator
                    .as_mut()
                    .and_then(|locator| {
                        locator
                            .span_at_location(location.line, location.column)
                            .map(|(_, span)| span)
                    })
                    .or_else(|| {
                        Some(SourceSpan {
                            source: source_name.map(ToOwned::to_owned),
                            start: SourcePosition {
                                line: location.line,
                                column: location.column,
                            },
                            end: SourcePosition {
                                line: location.line,
                                column: location.column.saturating_add(1),
                            },
                            byte_range: None,
                        })
                    })
            } else if let Some(locator) = locator.as_mut() {
                let end = locator.tokens.len().saturating_sub(1);
                match code.as_str() {
                    "E_STATE_INIT_REQUIRED" => {
                        locator.take_keyword(|kind| matches!(kind, TokenKind::State), 0, end)
                    }
                    "E_STATE_INIT_INCOMPLETE" => {
                        locator.take_keyword(|kind| matches!(kind, TokenKind::Hajimari), 0, end)
                    }
                    _ => None,
                }
            } else {
                None
            };
            Diagnostic::error(code, DiagnosticPhase::Semantic, message, primary_span)
        })
        .collect();
    DiagnosticBundle::new(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FrontendBudget, SourceId};

    fn diagnostics(source_text: &str) -> DiagnosticBundle {
        let source = SourceFile::new(SourceId(0), "audit.ko", source_text);
        let (program, tokens) = crate::parser::parse_source_spanned(&source, FrontendBudget::v1())
            .expect("source should parse before semantic audit");
        audit(&program, &source, &tokens).expect_err("source should fail semantic audit")
    }

    #[test]
    fn reports_multiple_independent_errors_at_exact_byte_ranges() {
        let source = r#"seiyaku Broken {
  fn first(value: Missing) -> i64 { let amount: Amount = 1; return unknown; }
  fn second(flag: bool) { flag = false; missing_call(); }
}"#;
        let bundle = diagnostics(source);
        for code in ["K2002", "K2003", "E_IMMUTABLE_ASSIGNMENT"] {
            assert!(
                bundle
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == code),
                "missing {code}: {bundle:?}"
            );
        }
        assert!(bundle.diagnostics.len() >= 5, "{bundle:?}");
        for diagnostic in &bundle.diagnostics {
            let Some(span) = &diagnostic.primary_span else {
                continue;
            };
            let range = span
                .byte_range
                .expect("audit diagnostics retain byte ranges");
            assert!(range.end >= range.start);
            assert!(usize::try_from(range.end).expect("u32 fits usize") <= source.len());
        }
    }

    #[test]
    fn reports_global_shadowing_and_unit_identity_collisions_with_labels() {
        let source = r#"seiyaku App {
  fn helper() {}
  fn inspect(helper: i64) { let App = 1; }
  fn App() {}
}"#;
        let bundle = diagnostics(source);
        let collisions = bundle
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "K2001")
            .collect::<Vec<_>>();
        assert!(collisions.len() >= 3, "{bundle:?}");
        assert!(
            collisions
                .iter()
                .all(|diagnostic| !diagnostic.labels.is_empty()),
            "every shadowing diagnostic must point to the original declaration: {bundle:?}"
        );
    }

    #[test]
    fn unicode_before_error_keeps_scalar_column_and_utf8_byte_offset_distinct() {
        let source = "seiyaku U { fn f() { let label: string = \"界\"; let value = nope; } }";
        let bundle = diagnostics(source);
        let error = bundle
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("`nope`"))
            .expect("unknown name diagnostic");
        let span = error.primary_span.as_ref().expect("span");
        let range = span.byte_range.expect("byte range");
        assert_eq!(
            &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
            "nope"
        );
        assert!(usize::try_from(range.start).unwrap() > span.start.column);
    }

    #[test]
    fn unknown_local_type_uses_its_exact_function_body_span() {
        let source = "seiyaku U { fn f() { let value: Missing = 1; } }";
        let bundle = diagnostics(source);
        let error = bundle
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code == "K2002" && diagnostic.message == "unknown type `Missing`"
            })
            .expect("unknown local type diagnostic");
        let span = error.primary_span.as_ref().expect("local type span");
        let range = span.byte_range.expect("local type byte range");
        assert_eq!(
            &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
            "Missing"
        );
    }

    #[test]
    fn duplicate_reserved_cycles_and_recursion_have_labels() {
        let source = r#"seiyaku Bad {
  struct A { b: B; }
  struct B { a: A; }
  const value: i64 = 1;
  state value: i64;
  fn i64() {}
  fn left() { right(); }
  fn right() { left(); }
}"#;
        let bundle = diagnostics(source);
        assert!(
            bundle
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "K2001" && !diagnostic.labels.is_empty() })
        );
        assert!(
            bundle
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "E_RESERVED_DECLARATION" })
        );
        assert!(
            bundle
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "K2006")
                .count()
                >= 2
        );
    }

    #[test]
    fn flattened_function_arguments_have_a_spanned_stable_limit_diagnostic() {
        let source = r#"seiyaku WideCall {
  struct Wide {
    f00: i64, f01: i64, f02: i64, f03: i64, f04: i64, f05: i64, f06: i64,
    f07: i64, f08: i64, f09: i64, f10: i64, f11: i64, f12: i64, f13: i64
  }
  view fn inspect(value: Wide) -> i64 { return value.f00; }
}"#;
        let bundle = diagnostics(source);
        let diagnostic = bundle
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "K2007")
            .expect("flattened argument limit diagnostic");
        assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
        assert!(diagnostic.message.contains("14 flattened argument words"));
        assert_eq!(
            diagnostic.help.as_deref(),
            Some(
                "Reduce or regroup parameters so their recursively flattened representation occupies at most 13 words."
            )
        );
        assert!(!diagnostic.labels.is_empty());
        let range = diagnostic
            .primary_span
            .as_ref()
            .and_then(|span| span.byte_range)
            .expect("parameter byte range");
        assert_eq!(
            &source[usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap()],
            "value"
        );
    }

    #[test]
    fn manual_unbounded_loop_is_rejected() {
        let source_text = "seiyaku Loop { fn f() {} }";
        let source = SourceFile::new(SourceId(0), "loop.ko", source_text);
        let (mut program, tokens) =
            crate::parser::parse_source_spanned(&source, FrontendBudget::v1()).unwrap();
        let Item::Function(function) = &mut program.items[0] else {
            panic!("function item")
        };
        function.body.statements.push(Statement::While {
            cond: Expr::Bool(true),
            body: Block { statements: vec![] },
        });
        let bundle = audit(&program, &source, &tokens).expect_err("while must fail closed");
        assert!(
            bundle
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E_UNBOUNDED_LOOP")
        );
    }
}
