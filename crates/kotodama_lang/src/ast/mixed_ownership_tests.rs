//! Mixed type-capacity and typed-expression ownership uses one caller-stack traversal.

use super::*;
use crate::{session, source::SourceId};

const MIXED_DEPTH: u32 = 32_768;

fn source(index: u32) -> SourceRange {
    SourceRange::new(SourceId(7), TextRange::new(index * 2, index * 2 + 1))
}

fn typed_statement(ty: Option<TypeExpr>) -> Statement {
    Statement::Let {
        mutable: true,
        pat: Pattern::Name("slot".to_owned()),
        ty,
        value: Expr::Bool(false),
    }
}

fn expression_with_type(ty: TypeExpr, index: u32) -> Expr {
    Expr::Resolved {
        id: HirId(index),
        source: Some(source(index)),
        expression: Box::new(Expr::If {
            condition: Box::new(Expr::Bool(true)),
            then_branch: Block {
                statements: vec![Statement::Source {
                    node: NodeId(index),
                    source: source(index),
                    statement: Box::new(typed_statement(Some(ty))),
                }],
                tail: Some(Box::new(Expr::IntLiteral(BigInt::one()))),
            },
            else_branch: None,
        }),
    }
}

fn mixed_type(depth: u32) -> TypeExpr {
    let mut ty = TypeExpr::Path("leaf".to_owned());
    for index in 0..depth {
        ty = TypeExpr::Source {
            node: NodeId(index),
            source: source(index),
            ty: Box::new(TypeExpr::Resolved {
                id: HirId(index),
                source: Some(source(index)),
                ty: Box::new(TypeExpr::ConstExpression(Box::new(expression_with_type(
                    ty, index,
                )))),
            }),
        };
    }
    ty
}

fn mixed_leaf(mut ty: &TypeExpr) -> &String {
    loop {
        match ty.kind() {
            TypeExpr::Path(name) => return name,
            TypeExpr::ConstExpression(expression) => {
                let Expr::If { then_branch, .. } = expression.kind() else {
                    panic!("mixed capacity expression must retain its branch");
                };
                let Statement::Let {
                    ty: Some(inner), ..
                } = then_branch.statements[0].kind()
                else {
                    panic!("mixed capacity expression must retain its type annotation");
                };
                ty = inner;
            }
            _ => panic!("mixed type has an unexpected leaf kind"),
        }
    }
}

fn change_mixed_leaf(mut ty: &mut TypeExpr) {
    loop {
        match ty {
            TypeExpr::Source { ty: inner, .. } | TypeExpr::Resolved { ty: inner, .. } => {
                ty = inner;
            }
            TypeExpr::Path(name) => {
                name.push_str("-changed");
                return;
            }
            TypeExpr::ConstExpression(expression) => {
                let Expr::Resolved { expression, .. } = expression.as_mut() else {
                    panic!("resolved capacity provenance must remain present");
                };
                let Expr::If { then_branch, .. } = expression.as_mut() else {
                    panic!("mixed capacity expression must retain its branch");
                };
                let Statement::Source { statement, .. } = &mut then_branch.statements[0] else {
                    panic!("statement source provenance must remain present");
                };
                let Statement::Let {
                    ty: Some(inner), ..
                } = statement.as_mut()
                else {
                    panic!("mixed capacity expression must retain its type annotation");
                };
                ty = inner;
            }
            _ => panic!("mixed type has an unexpected leaf kind"),
        }
    }
}

#[test]
fn mixed_type_clone_and_equality_preserve_provenance_without_workers() {
    let original = mixed_type(MIXED_DEPTH);
    session::reset_compiler_worker_spawn_count();
    let mut cloned = original.clone();
    let equal = original == cloned;
    let independent = mixed_leaf(&original).as_ptr() != mixed_leaf(&cloned).as_ptr();
    change_mixed_leaf(&mut cloned);
    let changed = original != cloned;
    let original_retained = mixed_leaf(&original) == "leaf";
    let clone_changed = mixed_leaf(&cloned) == "leaf-changed";
    drop_type_iterative(cloned);
    drop_type_iterative(original);
    assert!(equal, "all mixed nodes and provenance must be retained");
    assert!(independent, "the clone must own its leaf string");
    assert!(changed, "deep type differences must affect equality");
    assert!(original_retained);
    assert!(clone_changed);
    assert_eq!(session::compiler_worker_spawn_count(), 0);
}

#[test]
fn mixed_expression_and_statement_roots_use_the_same_work_list() {
    let expression = expression_with_type(mixed_type(MIXED_DEPTH), MIXED_DEPTH);
    session::reset_compiler_worker_spawn_count();
    let cloned_expression = expression.clone();
    let expressions_equal = expression == cloned_expression;
    drop_expression_iterative(cloned_expression);
    drop_expression_iterative(expression);
    assert!(expressions_equal);

    let statement = typed_statement(Some(mixed_type(MIXED_DEPTH)));
    let cloned_statement = statement.clone();
    let statements_equal = statement == cloned_statement;
    drop_block_iterative(Block {
        statements: vec![statement, cloned_statement],
        tail: None,
    });
    assert!(statements_equal);
    assert_eq!(session::compiler_worker_spawn_count(), 0);
}

#[test]
fn type_variants_and_optional_annotations_keep_exact_equality() {
    let variants = vec![
        TypeExpr::Path("int".to_owned()),
        TypeExpr::Const(3),
        TypeExpr::ConstExpression(Box::new(Expr::IntLiteral(BigInt::one()))),
        TypeExpr::Tuple(vec![]),
        TypeExpr::Tuple(vec![TypeExpr::Const(1), TypeExpr::Const(2)]),
        TypeExpr::Generic {
            base: "List".to_owned(),
            args: vec![TypeExpr::Path("int".to_owned()), TypeExpr::Const(4)],
        },
        TypeExpr::Source {
            node: NodeId(5),
            source: source(5),
            ty: Box::new(TypeExpr::Const(0)),
        },
        TypeExpr::Source {
            node: NodeId(6),
            source: source(5),
            ty: Box::new(TypeExpr::Const(0)),
        },
        TypeExpr::Source {
            node: NodeId(5),
            source: source(6),
            ty: Box::new(TypeExpr::Const(0)),
        },
        TypeExpr::Resolved {
            id: HirId(6),
            source: None,
            ty: Box::new(TypeExpr::Const(0)),
        },
        TypeExpr::Resolved {
            id: HirId(6),
            source: Some(source(6)),
            ty: Box::new(TypeExpr::Const(0)),
        },
    ];
    for (index, ty) in variants.iter().enumerate() {
        let cloned = ty.clone();
        assert!(ty == &cloned);
        for (other_index, other) in variants.iter().enumerate() {
            assert_eq!(ty == other, index == other_index);
        }
        drop_type_iterative(cloned);
    }
    for ty in variants {
        drop_type_iterative(ty);
    }
    let absent = typed_statement(None);
    let absent_clone = absent.clone();
    let present = typed_statement(Some(TypeExpr::Const(0)));
    assert!(absent == absent_clone);
    assert!(absent != present);
    drop_block_iterative(Block {
        statements: vec![absent, absent_clone, present],
        tail: None,
    });
}

#[test]
fn parsed_capacity_expressions_clone_on_the_caller_stack() {
    let mut capacity = "1".to_owned();
    for _ in 0..16 {
        capacity = format!("if true {{ let List<int, {capacity}> inner = []; 1 }} else {{ 1 }}");
    }
    let source =
        format!("seiyaku Mixed {{ hajimari() {{ let List<int, {capacity}> value = []; }} }}");
    let parsed = crate::parser::parse(&source).expect("parse mixed capacity and typed-let owners");
    session::reset_compiler_worker_spawn_count();
    let cloned = parsed.clone();
    let equal = parsed == cloned;
    drop_program_iterative(cloned);
    drop_program_iterative(parsed);
    assert!(
        equal,
        "parsed mixed-capacity AST identity must be preserved"
    );
    assert_eq!(session::compiler_worker_spawn_count(), 0);
}
