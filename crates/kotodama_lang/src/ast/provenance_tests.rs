//! Provenance queries and consumption preserve ownership without recursive calls.
use super::*;
use crate::source::SourceId;

fn range(id: u32) -> SourceRange {
    SourceRange {
        source: SourceId(id),
        range: TextRange::new(1, 2),
    }
}

#[test]
fn type_provenance_queries_and_unwrap_are_iterative() {
    let exact_source = range(7);
    let mut value = TypeExpr::Source {
        node: NodeId(11),
        source: exact_source,
        ty: Box::new(TypeExpr::Resolved {
            id: HirId(13),
            source: Some(range(8)),
            ty: Box::new(TypeExpr::Const(0)),
        }),
    };
    for _ in 0..32_768 {
        value = TypeExpr::Resolved {
            id: HirId(19),
            source: None,
            ty: Box::new(value),
        };
    }
    assert_eq!(value.source(), Some(exact_source));
    assert_eq!(value.hir_id(), Some(HirId(19)));
    assert_eq!(value.source_node(), Some(NodeId(11)));
    assert!(matches!(value.kind(), TypeExpr::Const(0)));
    // This consumes and releases every wrapper iteratively on the ordinary test stack.
    assert!(matches!(value.into_kind(), TypeExpr::Const(0)));

    let mut value = TypeExpr::Resolved {
        id: HirId(23),
        source: None,
        ty: Box::new(TypeExpr::Const(0)),
    };
    for _ in 0..32_768 {
        value = TypeExpr::Source {
            node: NodeId(29),
            source: range(9),
            ty: Box::new(value),
        };
    }
    assert_eq!(value.source(), Some(range(9)));
    assert_eq!(value.hir_id(), Some(HirId(23)));
    assert_eq!(value.source_node(), Some(NodeId(29)));
    assert!(matches!(value.kind(), TypeExpr::Const(0)));
    let leaf = value.into_kind();
    assert!(matches!(leaf, TypeExpr::Const(0)));
    assert_eq!(leaf.source(), None);
    assert_eq!(leaf.source_node(), None);
    assert_eq!(leaf.hir_id(), None);
}

#[test]
fn statement_provenance_queries_and_unwrap_are_iterative() {
    let exact_source = range(7);
    let mut value = Statement::Source {
        node: NodeId(11),
        source: exact_source,
        statement: Box::new(Statement::Resolved {
            id: HirId(13),
            source: Some(range(8)),
            statement: Box::new(Statement::Break),
        }),
    };
    for _ in 0..32_768 {
        value = Statement::Resolved {
            id: HirId(19),
            source: None,
            statement: Box::new(value),
        };
    }
    assert_eq!(value.source(), Some(exact_source));
    assert_eq!(value.hir_id(), Some(HirId(19)));
    assert_eq!(value.source_node(), Some(NodeId(11)));
    assert!(matches!(value.kind(), Statement::Break));
    // This consumes and releases every wrapper iteratively on the ordinary test stack.
    assert!(matches!(value.into_kind(), Statement::Break));

    let mut value = Statement::Resolved {
        id: HirId(23),
        source: None,
        statement: Box::new(Statement::Break),
    };
    for _ in 0..32_768 {
        value = Statement::Source {
            node: NodeId(29),
            source: range(9),
            statement: Box::new(value),
        };
    }
    assert_eq!(value.source(), Some(range(9)));
    assert_eq!(value.hir_id(), Some(HirId(23)));
    assert_eq!(value.source_node(), Some(NodeId(29)));
    assert!(matches!(value.kind(), Statement::Break));
    let leaf = value.into_kind();
    assert!(matches!(leaf, Statement::Break));
    assert_eq!(leaf.source(), None);
    assert_eq!(leaf.source_node(), None);
    assert_eq!(leaf.hir_id(), None);
}

#[test]
fn expression_provenance_queries_and_unwrap_are_iterative() {
    let exact_source = range(7);
    let mut value = Expr::Source {
        node: NodeId(11),
        source: exact_source,
        expression: Box::new(Expr::Resolved {
            id: HirId(13),
            source: Some(range(8)),
            expression: Box::new(Expr::Bool(true)),
        }),
    };
    for _ in 0..32_768 {
        value = Expr::Resolved {
            id: HirId(19),
            source: None,
            expression: Box::new(value),
        };
    }
    assert_eq!(value.source(), Some(exact_source));
    assert_eq!(value.hir_id(), Some(HirId(19)));
    assert_eq!(value.source_node(), Some(NodeId(11)));
    assert!(matches!(value.kind(), Expr::Bool(true)));
    // This consumes and releases every wrapper iteratively on the ordinary test stack.
    assert!(matches!(value.into_kind(), Expr::Bool(true)));

    let mut value = Expr::Resolved {
        id: HirId(23),
        source: None,
        expression: Box::new(Expr::Bool(true)),
    };
    for _ in 0..32_768 {
        value = Expr::Source {
            node: NodeId(29),
            source: range(9),
            expression: Box::new(value),
        };
    }
    assert_eq!(value.source(), Some(range(9)));
    assert_eq!(value.hir_id(), Some(HirId(23)));
    assert_eq!(value.source_node(), Some(NodeId(29)));
    assert!(matches!(value.kind(), Expr::Bool(true)));
    let leaf = value.into_kind();
    assert!(matches!(leaf, Expr::Bool(true)));
    assert_eq!(leaf.source(), None);
    assert_eq!(leaf.source_node(), None);
    assert_eq!(leaf.hir_id(), None);
}
