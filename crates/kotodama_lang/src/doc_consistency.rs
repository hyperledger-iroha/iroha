#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::{
        builtins::{Builtin, BuiltinSurface},
        compiler::{Compiler, CompilerOptions},
        lexer::{
            V1_KEYWORD_DOC_TABLE, V1_KEYWORD_EDITOR_PATTERN, V1_KEYWORDS, V1_OPERATOR_DOC_TABLE,
            V1_OPERATOR_EDITOR_PATTERN,
        },
        semantic::{V1_LIST_MEMBER_NAMES, V1_ROUNDING_PATHS, V1_SOURCE_TYPE_NAMES, V1_SUM_PATHS},
        session::{CompileRequest, CompilerSession},
    };

    fn docs_roots() -> [PathBuf; 2] {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        [
            manifest_dir.join("../../specs"),
            manifest_dir.join("../ivm/docs"),
        ]
    }

    fn is_localized_markdown(path: &std::path::Path) -> bool {
        // Translations are informative snapshots. The English V1 grammar and
        // Current branded examples are the release-language input to CI.
        const LOCALES: &[&str] = &[
            "am", "ar", "az", "ba", "dz", "es", "fr", "he", "hy", "ja", "ka", "kk", "mn", "my",
            "pt", "ru", "ur", "uz", "zh-hans", "zh-hant",
        ];
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            return false;
        };
        stem.rsplit_once('.')
            .is_some_and(|(_, suffix)| LOCALES.contains(&suffix))
    }

    fn kotodama_doc_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for root in docs_roots() {
            let entries = fs::read_dir(&root).unwrap_or_else(|err| {
                panic!("read {}: {err}", root.display());
            });
            paths.extend(
                entries
                    .filter_map(|entry| entry.ok().map(|item| item.path()))
                    .filter(|path| {
                        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                            return false;
                        };
                        (name.starts_with("kotodama_grammar")
                            || name.starts_with("kotodama_examples"))
                            && !is_localized_markdown(path)
                    }),
            );
        }
        paths.sort();
        paths
    }

    fn collect_markdown_files(root: &std::path::Path, paths: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(root).unwrap_or_else(|err| {
            panic!("read {}: {err}", root.display());
        });
        for entry in entries {
            let path = entry.expect("documentation directory entry").path();
            if path.is_dir() {
                collect_markdown_files(&path, paths);
            } else if path.extension().is_some_and(|extension| extension == "md")
                && !is_localized_markdown(&path)
            {
                paths.push(path);
            }
        }
    }

    fn kotodama_fences(text: &str) -> Vec<(usize, String)> {
        let mut snippets = Vec::new();
        let mut start_line = None;
        let mut source = String::new();
        for (index, line) in text.lines().enumerate() {
            if start_line.is_some() {
                if line.trim() == "```" {
                    snippets.push((start_line.expect("open fence"), std::mem::take(&mut source)));
                    start_line = None;
                } else {
                    source.push_str(line);
                    source.push('\n');
                }
            } else if line.trim() == "```kotodama" {
                start_line = Some(index + 2);
            }
        }
        assert!(start_line.is_none(), "unterminated `kotodama` code fence");
        snippets
    }

    fn textmate_match<'a>(grammar: &'a norito::json::Value, section: &str) -> &'a str {
        let patterns = grammar
            .pointer(&format!("/repository/{section}/patterns"))
            .and_then(norito::json::Value::as_array)
            .unwrap_or_else(|| panic!("TextMate grammar omitted {section} patterns"));
        assert_eq!(
            patterns.len(),
            1,
            "TextMate {section} table must contain exactly one generated matcher"
        );
        patterns[0]
            .get("match")
            .and_then(norito::json::Value::as_str)
            .unwrap_or_else(|| panic!("TextMate grammar omitted {section} match pattern"))
    }

    fn textmate_attribute_match(grammar: &norito::json::Value) -> &str {
        let patterns = grammar
            .pointer("/repository/attributes/patterns/0/patterns")
            .and_then(norito::json::Value::as_array)
            .expect("TextMate grammar omitted attribute patterns");
        let matches = patterns
            .iter()
            .filter_map(|pattern| pattern.get("match"))
            .filter_map(norito::json::Value::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "TextMate attributes must contain exactly one matcher"
        );
        matches[0]
    }

    fn textmate_named_match<'a>(grammar: &'a norito::json::Value, name: &str) -> &'a str {
        grammar
            .pointer("/repository/numbers/patterns")
            .and_then(norito::json::Value::as_array)
            .expect("TextMate grammar omitted numeric patterns")
            .iter()
            .find(|pattern| pattern.get("name").and_then(norito::json::Value::as_str) == Some(name))
            .and_then(|pattern| pattern.get("match"))
            .and_then(norito::json::Value::as_str)
            .unwrap_or_else(|| panic!("TextMate grammar omitted matcher `{name}`"))
    }

    fn textmate_top_level_includes(grammar: &norito::json::Value) -> Vec<&str> {
        grammar
            .get("patterns")
            .and_then(norito::json::Value::as_array)
            .expect("TextMate grammar omitted top-level patterns")
            .iter()
            .filter_map(|pattern| pattern.get("include"))
            .filter_map(norito::json::Value::as_str)
            .collect()
    }

    fn textmate_definition_matches(grammar: &norito::json::Value) -> Vec<&str> {
        grammar
            .pointer("/repository/definitions/patterns")
            .and_then(norito::json::Value::as_array)
            .expect("TextMate grammar omitted definition patterns")
            .iter()
            .filter_map(|pattern| pattern.get("match"))
            .filter_map(norito::json::Value::as_str)
            .collect()
    }

    fn alternation_pattern(paths: &[&str]) -> String {
        format!(r"\b(?:{})\b", paths.join("|"))
    }

    fn generated_section<'a>(text: &'a str, name: &str) -> &'a str {
        let start_marker = format!("<!-- BEGIN GENERATED: {name} -->\n");
        let end_marker = format!("<!-- END GENERATED: {name} -->");
        let (_, rest) = text
            .split_once(&start_marker)
            .unwrap_or_else(|| panic!("missing generated section `{name}`"));
        let (section, _) = rest
            .split_once(&end_marker)
            .unwrap_or_else(|| panic!("unterminated generated section `{name}`"));
        section
    }

    #[test]
    fn kotodama_docs_do_not_advertise_removed_helper_spellings() {
        for path in kotodama_doc_paths() {
            let text = fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!("read {}: {err}", path.display());
            });
            for needle in [
                "get_or_insert_default(",
                ".get_or_insert_default(",
                ".json_get_",
                ".path_map_key(",
                ".path_map_key_norito(",
                ".has(",
                " json_get_",
                " path_map_key(",
                " path_map_key_norito(",
                "account!(",
                "account_id!(",
                "asset_definition!(",
                "asset_id!(",
                "domain!(",
                "domain_id!(",
                "name!(",
                "json!(",
                "json!{",
                "json![",
                "nft_id!(",
                "blob!(",
                "norito_bytes!(",
            ] {
                assert!(
                    !text.contains(needle),
                    "{} still contains removed helper spelling `{needle}`",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn canonical_syntax_tables_cover_docs_and_textmate_grammar() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let specification =
            fs::read_to_string(manifest_dir.join("../../specs/kotodama_grammar.md"))
                .expect("read normative Kotodama grammar");
        let textmate =
            fs::read_to_string(manifest_dir.join(
                "../../tools/kotodama_linguist/grammar-repo/syntaxes/kotodama.tmLanguage.json",
            ))
            .expect("read TextMate grammar");
        let textmate_value: norito::json::Value =
            norito::json::from_str(&textmate).expect("parse TextMate grammar JSON");

        assert_eq!(
            generated_section(&specification, "kotodama-v1-keywords"),
            V1_KEYWORD_DOC_TABLE,
            "the normative keyword table must be regenerated from grammar/v1.lex"
        );
        assert_eq!(
            generated_section(&specification, "kotodama-v1-operators"),
            V1_OPERATOR_DOC_TABLE,
            "the normative operator table must be regenerated from grammar/v1.lex"
        );
        assert_eq!(
            textmate_match(&textmate_value, "keywords"),
            V1_KEYWORD_EDITOR_PATTERN,
            "TextMate keyword matcher must be generated from grammar/v1.lex"
        );
        assert_eq!(
            textmate_match(&textmate_value, "operators"),
            V1_OPERATOR_EDITOR_PATTERN,
            "TextMate operator matcher must be generated from grammar/v1.lex"
        );
        assert_eq!(
            textmate_match(&textmate_value, "builtins"),
            r"(?:\brequire\b|(?<=::)[A-Za-z_][A-Za-z0-9_]*)(?=\s*\()",
            "TextMate builtin highlighting must remain structural and namespaced"
        );
        assert_eq!(
            textmate_attribute_match(&textmate_value),
            r"\b(?:fixture|test)\b",
            "TextMate attributes must expose only the supported test annotation surface"
        );

        let top_level_includes = textmate_top_level_includes(&textmate_value);
        for section in [
            "#namedFields",
            "#sumVariants",
            "#roundingVariants",
            "#jsonConstruction",
            "#memberCalls",
            "#retiredNumericSuffixes",
        ] {
            assert_eq!(
                top_level_includes
                    .iter()
                    .filter(|include| **include == section)
                    .count(),
                1,
                "TextMate grammar must include V1 contextual section `{section}` exactly once"
            );
        }

        assert_eq!(
            textmate_match(&textmate_value, "namedFields"),
            r"\b[A-Za-z_][A-Za-z0-9_]*\b(?=\s*:)",
            "TextMate named-field highlighting drifted from named calls/struct literals"
        );
        assert_eq!(
            textmate_match(&textmate_value, "sumVariants"),
            alternation_pattern(V1_SUM_PATHS),
            "TextMate sum paths drifted from the active-only Option/Result surface"
        );
        assert_eq!(
            textmate_match(&textmate_value, "roundingVariants"),
            alternation_pattern(V1_ROUNDING_PATHS),
            "TextMate rounding paths drifted from the exact quantity surface"
        );
        assert_eq!(
            textmate_match(&textmate_value, "jsonConstruction"),
            r"\bjson\b(?=\s*[\{\[])",
            "TextMate must treat `json` as contextual object/array construction syntax"
        );

        let mut member_names = V1_LIST_MEMBER_NAMES.to_vec();
        member_names.push("div_round");
        member_names.push("ratio_round");
        for builtin in Builtin::all() {
            if !matches!(
                builtin.surface(),
                BuiltinSurface::MethodOnly | BuiltinSurface::FunctionOrMethod
            ) {
                continue;
            }
            let member_name = builtin.name();
            if !member_names.contains(&member_name) {
                member_names.push(member_name);
            }
        }
        assert_eq!(
            textmate_match(&textmate_value, "memberCalls"),
            format!(r"(?<=\.)(?:{})(?=\s*\()", member_names.join("|")),
            "TextMate member calls drifted from bounded List, quantity, or typed JSON APIs"
        );
        assert_eq!(
            textmate_match(&textmate_value, "retiredNumericSuffixes"),
            r"(?<![A-Za-z0-9_])(?:0[xX][0-9A-Fa-f_]+|0[bB][01_]+|\d(?:[\d_]*\d)?(?:\.\d(?:[\d_]*\d)?)?(?:[eE][+-]?\d(?:[\d_]*\d)?)?)(?:amt|qty)\b",
            "TextMate retired numeric suffix highlighting drifted from amt/qty fix-it policy"
        );

        let mut type_names = V1_SOURCE_TYPE_NAMES.to_vec();
        type_names.push("Rounding");
        assert_eq!(
            textmate_match(&textmate_value, "types"),
            alternation_pattern(&type_names),
            "TextMate types drifted from the canonical V1 source surface"
        );
        assert_eq!(
            textmate_named_match(&textmate_value, "constant.numeric.decimal.kotodama"),
            r"\b(?:\d(?:[\d_]*\d)?\.\d(?:[\d_]*\d)?(?:[eE][+-]?\d(?:[\d_]*\d)?)?|\d(?:[\d_]*\d)?[eE][+-]?\d(?:[\d_]*\d)?)\b",
            "TextMate decimal literal highlighting drifted from unsuffixed V1 syntax"
        );

        let definition_matches = textmate_definition_matches(&textmate_value);
        for retired in ["contract", "entry", "init", "upgrade"] {
            assert!(
                definition_matches
                    .iter()
                    .all(|pattern| !pattern.contains(retired)),
                "TextMate definition matchers still accept retired English declaration spelling `{retired}`"
            );
        }
        for keyword in V1_KEYWORDS {
            assert!(
                !iroha_data_model::smart_contract::entrypoint::is_canonical_kotodama_identifier(
                    keyword,
                ),
                "boundary-schema identifier validation drifted from grammar/v1.lex at `{keyword}`"
            );
        }
        for retired in [
            "assert",
            "assert_eq",
            "contains",
            "get_or_insert_default",
            "transfer_asset",
            "mint_asset",
            "burn_asset",
            "subscription_bill",
            "execute_instruction",
            "execute_query",
            "create_trigger",
            "register_trigger",
            "unregister_trigger",
            "remove_trigger",
            "authority",
            "account_id",
            "asset_definition",
            "asset_id",
            "domain",
            "domain_id",
            "name",
            "json",
            "nft_id",
            "norito_bytes",
            "blob",
            "isqrt",
            "info",
            "zk_vote_verify_ballot",
            "zk_verify_transfer",
            "zk_verify_unshield",
            "sc_execute_unshield",
            "build_submit_ballot_inline",
            "build_unshield_inline",
        ] {
            assert!(
                !textmate_match(&textmate_value, "builtins").contains(retired),
                "TextMate grammar still advertises retired raw or flat builtin `{retired}`"
            );
        }
        for retired in [
            "get_numeric",
            "json_get_int",
            "json_get_numeric",
            "get_or_insert_default",
            "has",
        ] {
            assert!(
                !textmate_match(&textmate_value, "memberCalls").contains(retired),
                "TextMate grammar still advertises retired method `{retired}`"
            );
        }
    }

    #[test]
    fn every_current_kotodama_documentation_fence_compiles() {
        let mut paths = Vec::new();
        for root in docs_roots() {
            collect_markdown_files(&root, &mut paths);
        }
        paths.sort();

        let compiler = Compiler::new();
        let session = CompilerSession::new(CompilerOptions::default());
        let mut failures = Vec::new();
        for path in paths {
            let text = fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!("read {}: {err}", path.display());
            });
            for (line, source) in kotodama_fences(&text) {
                let result = if source.trim_start().starts_with("module ") {
                    session
                        .check(CompileRequest {
                            source: &source,
                            source_name: None,
                        })
                        .map_err(|diagnostics| diagnostics.render_human())
                } else {
                    compiler.compile_source_with_manifest(&source).map(|_| ())
                };
                if let Err(error) = result {
                    failures.push(format!("{}:{line}: {error}", path.display()));
                }
            }
        }

        assert!(
            failures.is_empty(),
            "Kotodama documentation snippets failed to compile:\n{}",
            failures.join("\n")
        );
    }
}
