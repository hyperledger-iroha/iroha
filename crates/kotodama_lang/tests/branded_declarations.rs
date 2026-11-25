//! Kotodama V1 branded declaration syntax invariants.
use kotodama_lang::{
    formatter::format_source,
    lexer::V1_KEYWORD_EDITOR_PATTERN,
    session::{CompileRequest, CompilerSession},
    source::{FrontendBudget, SourceFile, SourceId},
    syntax::parse,
};
fn parse_source(text: &str) -> kotodama_lang::syntax::ParseOutput {
    let source = SourceFile::new(SourceId(0), "branding.ko", text);
    parse(&source, FrontendBudget::v1())
}
#[test]
fn romanized_and_japanese_declaration_sets_are_first_class() {
    for source in [
        r#"seiyaku Branding { permission Set;
            state int value;
            hajimari() { value = 0; }
            kotoage fn set(int next) authorize(Set) { value = next; }
            kaizen() {}
            view fn read() authorize(anyone) -> int { return value; }
            trigger tick -> set { on time pre_commit; }
        }"#,
        r#"誓約 Branding { permission Set;
            state int value;
            始まり() { value = 0; }
            言挙げ fn set(int next) authorize(Set) { value = next; }
            改善() {}
            view fn read() authorize(anyone) -> int { return value; }
            trigger tick -> set { on time pre_commit; }
        }"#,
    ] {
        let output = parse_source(source);
        assert!(output.is_ok(), "{:?}", output.diagnostics.diagnostics);
    }
}
#[test]
fn formatting_preserves_the_selected_japanese_declaration_script() {
    let source = SourceFile::new(
        SourceId(1),
        "japanese-branding.ko",
        "誓約 Branding{ permission Set; 始まり(){}言挙げ fn set() authorize(Set) {}改善(){}}",
    );
    let formatted = format_source(&source, FrontendBudget::v1())
        .expect("Japanese branded declarations must format");
    for spelling in ["誓約", "始まり", "言挙げ", "改善"] {
        assert!(
            formatted.contains(spelling),
            "formatter omitted `{spelling}`: {formatted}"
        );
    }
    for spelling in ["seiyaku", "hajimari", "kotoage", "kaizen"] {
        assert!(
            !formatted.contains(spelling),
            "formatter rewrote Japanese `{spelling}` feature syntax: {formatted}"
        );
    }
}
#[test]
fn editor_keyword_boundaries_cover_unicode_identifier_continuations() {
    for property in [r"\p{L}", r"\p{N}"] {
        assert!(
            V1_KEYWORD_EDITOR_PATTERN.contains(property),
            "editor keyword matcher omitted Unicode boundary `{property}`"
        );
    }
    for alias in ["contract", "entry", "init", "upgrade"] {
        assert!(
            !V1_KEYWORD_EDITOR_PATTERN.contains(alias),
            "editor keyword matcher advertised English feature alias `{alias}`"
        );
    }
}
#[test]
fn english_feature_aliases_are_rejected_by_the_lossless_frontend() {
    for (alias, source) in [
        ("contract", "contract Branding {}"),
        (
            "entry",
            "seiyaku Branding { entry fn set() authorize(anyone) {} }",
        ),
        ("init", "seiyaku Branding { init() {} }"),
        ("upgrade", "seiyaku Branding { upgrade() {} }"),
    ] {
        let output = parse_source(source);
        assert!(
            !output.is_ok(),
            "English feature alias `{alias}` was accepted"
        );
        assert_eq!(
            output
                .tree
                .text(&SourceFile::new(SourceId(0), "branding.ko", source)),
            source,
            "alias rejection must remain lossless"
        );
    }
}
#[test]
fn english_words_remain_available_as_ordinary_identifiers() {
    let source = r#"module OrdinaryWords {
        fn combine(int contract, int entry, int init, int upgrade) -> int {
            return contract + entry + init + upgrade;
        }
    }"#;
    let output = parse_source(source);
    assert!(output.is_ok(), "{:?}", output.diagnostics.diagnostics);
    CompilerSession::c10()
        .check(CompileRequest {
            source,
            source_name: Some("ordinary-words.ko"),
        })
        .expect("English words that are not feature syntax must resolve as ordinary identifiers");
}

#[test]
fn every_mixed_keyword_spelling_emits_identical_bytes_within_each_profile() {
    use kotodama_lang::compiler::CompilerOptions;
    let spellings = [
        ["seiyaku", "誓約"],
        ["kotoage", "言挙げ"],
        ["hajimari", "始まり"],
        ["kaizen", "改善"],
    ];
    for options in [CompilerOptions::default(), CompilerOptions::c10()] {
        let profile = options.target_profile().unwrap();
        let session = CompilerSession::new(options);
        let mut reference = None;
        for mask in 0..16usize {
            let [unit, entry, init, upgrade] =
                std::array::from_fn(|i| spellings[i][(mask >> i) & 1]);
            let source = format!(
                "{unit} Counter {{
                permission Increment;
                state int value;
                {init}() {{ value = 0; }}
                {upgrade}() {{ value = 1; }}
                {entry} fn bump(int step) authorize(Increment) {{ value += step; }}
                view fn read() authorize(anyone) -> int {{ value }}
            }}"
            );
            let artifact = session
                .build(CompileRequest {
                    source: &source,
                    source_name: Some("Counter.ko"),
                })
                .unwrap()
                .artifact;
            assert_eq!(ivm_abi::IvmProfileId::declared(&artifact), Some(profile));
            if let Some(expected) = &reference {
                assert_eq!(&artifact, expected, "{profile:?}, {mask:04b}");
            } else {
                reference = Some(artifact);
            }
        }
    }
}

#[test]
fn formatter_wraps_japanese_arguments_at_display_cell_boundary() {
    let source = r#"誓約 Pretty { 言挙げ fn run() authorize(anyone) { write_pair(first: "書類を確認してから提出してください", second: "口座の残高を確認してください"); } }"#;
    let formatted = |text: &str| {
        format_source(
            &SourceFile::new(SourceId(0), "pretty.ko", text),
            FrontendBudget::v1(),
        )
        .unwrap()
    };
    let output = formatted(source);
    assert!(output.contains("write_pair(\n"), "{output}");
    assert!(output.contains("誓約 Pretty") && output.contains("言挙げ fn run"));
    assert_eq!(formatted(&output), output);
}
