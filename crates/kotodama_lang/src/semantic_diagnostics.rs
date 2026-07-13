//! Structured adaptation for failures emitted by typed semantic analysis.
//!
//! Name, declaration, type, and call diagnostics are produced by the resolved
//! HIR pass with exact `SourceId`/`TextRange` nodes. Typed analysis joins its
//! AST nodes to the same immutable source table and records exact structured
//! ranges, labels, and fix recipes. Whole-program state invariants use their
//! parser-owned state/lifecycle declaration nodes. This adapter deliberately
//! does not scan diagnostic messages or source tokens to guess a spelling.

use crate::{
    diagnostic::{
        Diagnostic, DiagnosticBundle, DiagnosticFix, DiagnosticLabel, SourceSpan,
        phase_for_semantic_failure,
    },
    resolved::ResolvedProgram,
    source::{SourceFile, SourceRange},
};

#[cfg(test)]
use crate::diagnostic::DiagnosticPhase;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticDiagnosticLabel {
    pub(crate) source: SourceRange,
    pub(crate) message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SemanticFix {
    PositionalStruct {
        name: String,
        fields: Vec<String>,
        arguments: Vec<SourceRange>,
    },
    ListGet {
        target: SourceRange,
        index: SourceRange,
    },
    ListTrySet {
        target: SourceRange,
        index: SourceRange,
        value: SourceRange,
    },
    Replace {
        replacement: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticDiagnostic {
    pub(crate) primary: SourceRange,
    pub(crate) labels: Vec<SemanticDiagnosticLabel>,
    pub(crate) fix: Option<SemanticFix>,
}

fn source_span(source: &SourceFile, range: SourceRange) -> Option<SourceSpan> {
    (source.id() == range.source).then(|| SourceSpan::from_range(source, range.range))
}

fn safe_slice(source: &SourceFile, range: SourceRange) -> Option<&str> {
    (source.id() == range.source)
        .then(|| source.slice(range.range))
        .flatten()
        .filter(|text| !text.contains("//") && !text.contains("/*"))
}

fn strict_child(primary: SourceRange, child: SourceRange) -> bool {
    primary.source == child.source
        && primary.range != child.range
        && !child.range.is_empty()
        && primary.range.contains(child.range)
}

fn materialize_fix(
    source: &SourceFile,
    primary: SourceRange,
    fix: SemanticFix,
) -> Option<DiagnosticFix> {
    let replacement = match fix {
        SemanticFix::PositionalStruct {
            name,
            fields,
            arguments,
        } => {
            if fields.len() != arguments.len()
                || safe_slice(source, primary).is_none()
                || arguments
                    .iter()
                    .any(|argument| !strict_child(primary, *argument))
                || arguments
                    .windows(2)
                    .any(|window| window[0].range.end > window[1].range.start)
            {
                return None;
            }
            let fields = fields
                .iter()
                .zip(arguments)
                .map(|(field, argument)| {
                    safe_slice(source, argument).map(|argument| format!("{field}: {argument}"))
                })
                .collect::<Option<Vec<_>>>()?;
            if fields.is_empty() {
                format!("{name} {{}}")
            } else {
                format!("{name} {{ {}, }}", fields.join(", "))
            }
        }
        SemanticFix::ListGet { target, index } => {
            if safe_slice(source, primary).is_none()
                || !strict_child(primary, target)
                || !strict_child(primary, index)
                || target.range.end > index.range.start
            {
                return None;
            }
            let target = safe_slice(source, target)?;
            let index = safe_slice(source, index)?;
            format!("{target}.get({index})")
        }
        SemanticFix::ListTrySet {
            target,
            index,
            value,
        } => {
            // V1 has no executable unchecked-write form to preserve. Rewriting
            // one complete simple assignment to `try_set` gives it the defined
            // migration semantics: attempt the mutation once and safely ignore
            // an out-of-range `false`. The semantic producer never emits this
            // recipe for compound writes, while these range/comment checks make
            // partial or trivia-moving rewrites fail closed.
            if safe_slice(source, primary).is_none()
                || !strict_child(primary, target)
                || !strict_child(primary, index)
                || !strict_child(primary, value)
                || target.range.end > index.range.start
                || index.range.end > value.range.start
            {
                return None;
            }
            let target = safe_slice(source, target)?;
            let index = safe_slice(source, index)?;
            let value = safe_slice(source, value)?;
            format!("{target}.try_set(index: {index}, value: {value});")
        }
        SemanticFix::Replace { replacement } => {
            safe_slice(source, primary)?;
            replacement
        }
    };
    Some(DiagnosticFix {
        span: source_span(source, primary)?,
        replacement,
    })
}

/// Convert failures from typed/effect analysis into canonical diagnostics.
///
/// Structured semantic metadata supplies the primary range, secondary labels,
/// and any fix recipe. A function location is used only as a conservative
/// fallback for failures that predate structured metadata. Known program-wide
/// invariants use resolved declaration nodes; unknown invariants do not invent
/// a zero-width or spelling-based span when no owning node exists.
pub(crate) fn from_semantic_failures(
    failures: crate::semantic::SemanticFailures,
    _source_name: Option<&str>,
    source: Option<&SourceFile>,
    resolved: Option<&ResolvedProgram>,
) -> DiagnosticBundle {
    DiagnosticBundle::new(
        failures
            .failures
            .into_iter()
            .map(|failure| {
                let code = failure.error.code;
                let message = failure.error.message;
                let semantic = failure.diagnostic;
                let primary_span = if code == "K0004" {
                    None
                } else {
                    semantic
                        .as_ref()
                        .and_then(|diagnostic| {
                            source.and_then(|source| source_span(source, diagnostic.primary))
                        })
                        .or_else(|| {
                            failure.location.and_then(|location| {
                                source.zip(resolved).and_then(|(source, resolved)| {
                                    resolved.span_for_location(
                                        source,
                                        location.line,
                                        location.column,
                                    )
                                })
                            })
                        })
                        .or_else(|| {
                            source.zip(resolved).and_then(|(source, resolved)| {
                                let range = match code {
                                    "E_STATE_HAJIMARI_REQUIRED" => {
                                        resolved.first_scalar_state_keyword_source()
                                    }
                                    "E_STATE_HAJIMARI_INCOMPLETE" => {
                                        resolved.hajimari_name_source()
                                    }
                                    _ => None,
                                }?;
                                source_span(source, range)
                            })
                        })
                };
                let mut diagnostic = Diagnostic::error(
                    code,
                    phase_for_semantic_failure(code),
                    message,
                    primary_span,
                );
                if let Some(semantic) = semantic {
                    if let Some(source) = source {
                        diagnostic.labels = semantic
                            .labels
                            .into_iter()
                            .filter_map(|label| {
                                Some(DiagnosticLabel {
                                    span: source_span(source, label.source)?,
                                    message: label.message,
                                })
                            })
                            .collect();
                        diagnostic.fix = semantic
                            .fix
                            .and_then(|fix| materialize_fix(source, semantic.primary, fix));
                    }
                }
                diagnostic
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceId, TextRange};

    fn range(source: SourceId, text: &str, needle: &str) -> SourceRange {
        let start = text.find(needle).expect("fixture substring");
        SourceRange::new(
            source,
            TextRange::new(
                u32::try_from(start).expect("fixture offset fits u32"),
                u32::try_from(start + needle.len()).expect("fixture end fits u32"),
            ),
        )
    }

    #[test]
    fn stable_error_code_is_independent_of_message_wording() {
        for message in [
            "unknown value `missing`",
            "valeur introuvable `missing`",
            "欠落している値 `missing`",
            "E_OTHER: text that resembles a different diagnostic",
        ] {
            let bundle = from_semantic_failures(
                crate::semantic::SemanticError {
                    code: "K2002",
                    message: message.to_owned(),
                }
                .into(),
                None,
                None,
                None,
            );
            assert_eq!(bundle.diagnostics[0].code, "K2002");
            assert_eq!(bundle.diagnostics[0].phase, DiagnosticPhase::Resolve);
            assert_eq!(bundle.diagnostics[0].message, message);
        }
    }

    #[test]
    fn phase_adapter_only_promotes_registry_owned_resolver_failures() {
        for code in [
            "K2002",
            "E_DUPLICATE_DECLARATION",
            "E_RESERVED_DECLARATION",
            "E_DUPLICATE_ERROR_CODE",
            "E_INTERNAL_RESOLUTION",
        ] {
            assert_eq!(phase_for_semantic_failure(code), DiagnosticPhase::Resolve);
        }
        for code in ["K0004", "K2003", "E_BRANCH_TYPE_MISMATCH", "UNKNOWN"] {
            assert_eq!(phase_for_semantic_failure(code), DiagnosticPhase::Semantic);
        }
    }

    #[test]
    fn positional_struct_fix_uses_exact_argument_spellings() {
        let text = "Pair(1.250_0, nested(2))";
        let source_id = SourceId(7);
        let source = SourceFile::new(source_id, "pair.ko", text);
        let primary = range(source_id, text, text);
        let fix = materialize_fix(
            &source,
            primary,
            SemanticFix::PositionalStruct {
                name: "Pair".to_owned(),
                fields: vec!["left".to_owned(), "right".to_owned()],
                arguments: vec![
                    range(source_id, text, "1.250_0"),
                    range(source_id, text, "nested(2)"),
                ],
            },
        )
        .expect("safe positional fix");
        assert_eq!(fix.span.byte_range, Some(primary.range));
        assert_eq!(fix.replacement, "Pair { left: 1.250_0, right: nested(2), }");
    }

    #[test]
    fn semantic_fixes_fail_closed_for_comments_or_wrong_sources() {
        let source_id = SourceId(3);
        let text = "Pair(1, /* retain */ 2)";
        let source = SourceFile::new(source_id, "comments.ko", text);
        let primary = range(source_id, text, text);
        assert!(
            materialize_fix(
                &source,
                primary,
                SemanticFix::PositionalStruct {
                    name: "Pair".to_owned(),
                    fields: vec!["left".to_owned(), "right".to_owned()],
                    arguments: vec![range(source_id, text, "1"), range(source_id, text, "2"),],
                },
            )
            .is_none()
        );

        let wrong_source = SourceRange::new(SourceId(99), primary.range);
        assert!(
            materialize_fix(
                &source,
                wrong_source,
                SemanticFix::ListGet {
                    target: wrong_source,
                    index: wrong_source,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn safe_list_read_fix_preserves_receiver_and_index_spelling() {
        let source_id = SourceId(11);
        let text = "values[(offset + 1)]";
        let source = SourceFile::new(source_id, "list.ko", text);
        let primary = range(source_id, text, text);
        let fix = materialize_fix(
            &source,
            primary,
            SemanticFix::ListGet {
                target: range(source_id, text, "values"),
                index: range(source_id, text, "(offset + 1)"),
            },
        )
        .expect("safe list read fix");
        assert_eq!(fix.replacement, "values.get((offset + 1))");
    }

    #[test]
    fn safe_list_write_fix_uses_the_complete_statement_range() {
        let source_id = SourceId(12);
        let text = "values[offset] = replacement;";
        let source = SourceFile::new(source_id, "list-write.ko", text);
        let primary = range(source_id, text, text);
        let fix = materialize_fix(
            &source,
            primary,
            SemanticFix::ListTrySet {
                target: range(source_id, text, "values"),
                index: range(source_id, text, "offset"),
                value: range(source_id, text, "replacement"),
            },
        )
        .expect("safe List.try_set fix");
        assert_eq!(fix.span.byte_range, Some(primary.range));
        assert_eq!(
            fix.replacement,
            "values.try_set(index: offset, value: replacement);"
        );
    }

    #[test]
    fn exact_type_replacement_does_not_rewrite_surrounding_source() {
        let source_id = SourceId(13);
        let text = "let bytes raw = query;";
        let source = SourceFile::new(source_id, "query.ko", text);
        let primary = range(source_id, text, "bytes");
        let fix = materialize_fix(
            &source,
            primary,
            SemanticFix::Replace {
                replacement: "Option<AccountView>".to_owned(),
            },
        )
        .expect("exact type replacement");
        assert_eq!(fix.span.byte_range, Some(primary.range));
        assert_eq!(fix.replacement, "Option<AccountView>");
    }
}
