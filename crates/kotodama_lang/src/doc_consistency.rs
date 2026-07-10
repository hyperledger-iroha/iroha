#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::{
        compiler::{Compiler, CompilerOptions},
        lexer::{V1_KEYWORDS, V1_OPERATORS},
        session::{CompileRequest, CompilerSession},
    };

    fn docs_roots() -> [PathBuf; 3] {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        [
            manifest_dir.join("../../docs/source"),
            manifest_dir.join("../ivm/docs"),
            manifest_dir.join("../../docs/portal/docs/norito"),
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

    fn keyword_pattern() -> String {
        format!(
            "(?<![A-Za-z0-9_])(?:{})(?![A-Za-z0-9_])",
            V1_KEYWORDS.join("|")
        )
    }

    fn regex_escape_literal(spelling: &str) -> String {
        let mut escaped = String::with_capacity(spelling.len().saturating_mul(2));
        for character in spelling.chars() {
            if matches!(
                character,
                '\\' | '.' | '^' | '$' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}'
            ) {
                escaped.push('\\');
            }
            escaped.push(character);
        }
        escaped
    }

    fn operator_pattern() -> String {
        let mut spellings = V1_OPERATORS.to_vec();
        spellings.sort_unstable_by(|left, right| {
            right.len().cmp(&left.len()).then_with(|| left.cmp(right))
        });
        format!(
            "(?:{})",
            spellings
                .into_iter()
                .map(regex_escape_literal)
                .collect::<Vec<_>>()
                .join("|")
        )
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
            fs::read_to_string(manifest_dir.join("../../docs/source/kotodama_grammar.md"))
                .expect("read normative Kotodama grammar");
        let textmate =
            fs::read_to_string(manifest_dir.join(
                "../../tools/kotodama_linguist/grammar-repo/syntaxes/kotodama.tmLanguage.json",
            ))
            .expect("read TextMate grammar");
        let textmate_value: norito::json::Value =
            norito::json::from_str(&textmate).expect("parse TextMate grammar JSON");

        for keyword in V1_KEYWORDS {
            assert!(
                specification.contains(keyword),
                "normative grammar omitted canonical keyword `{keyword}`"
            );
            assert!(
                textmate.contains(keyword),
                "TextMate grammar omitted canonical keyword `{keyword}`"
            );
        }
        for operator in V1_OPERATORS {
            assert!(
                specification.contains(operator),
                "normative grammar omitted canonical operator `{operator}`"
            );
        }
        assert_eq!(
            textmate_match(&textmate_value, "keywords"),
            keyword_pattern(),
            "TextMate keyword matcher must be generated exactly from V1_KEYWORDS"
        );
        assert_eq!(
            textmate_match(&textmate_value, "operators"),
            operator_pattern(),
            "TextMate operator matcher must be generated exactly from V1_OPERATORS"
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
        for retired in ["contract", "entry", "init", "upgrade"] {
            assert!(
                !textmate.contains(&format!("\\\\b{retired}")),
                "TextMate grammar still accepts retired English declaration spelling `{retired}`"
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
            "zk_verify_unshield",
            "build_submit_ballot_inline",
            "build_unshield_inline",
        ] {
            assert!(
                !textmate_match(&textmate_value, "builtins").contains(retired),
                "TextMate grammar still advertises retired raw or flat builtin `{retired}`"
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
