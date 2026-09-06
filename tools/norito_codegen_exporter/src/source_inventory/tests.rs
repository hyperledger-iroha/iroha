//! Adversarial source fixtures for syntax-only declaration inventory.

use super::*;

fn inspect(source: &str) -> FileInventory {
    let report = inspect_source("fixtures/source.rs", source).expect("inspect source");
    assert!(report.parse_error.is_none(), "{:?}", report.parse_error);
    report
}

#[test]
fn exact_utf8_spans_preserve_docs_crlf_and_anchor_bytes() {
    let source = "//! café\r\n#[derive(norito::Encode)]\r\npub(crate) struct Café { value: u32 }";
    let report = inspect(source);
    let item = &report.declarations[0];
    let anchor = item.mapping_anchor.as_ref().expect("struct anchor");
    assert_eq!(anchor.anchor, "pub(crate) struct Café { value: u32 }");
    assert_eq!(
        &source.as_bytes()[anchor.start_byte..anchor.end_byte],
        anchor.anchor.as_bytes()
    );
    assert_eq!(
        item.span.source,
        "#[derive(norito::Encode)]\r\npub(crate) struct Café { value: u32 }"
    );
    assert_eq!(item.derives[0].span.source, "norito::Encode");
    assert_eq!(item.identifier, "Café");
    assert_eq!(report.bytes, source.len());
}

#[test]
fn alias_imports_are_lexical_evidence_without_canonical_name_inference() {
    let source = "use norito::{codec::Encode as Write, codec as codec_alias};\n#[derive(Write, codec_alias::Decode)]\nstruct Wire;";
    let report = inspect(source);
    let derives = &report.declarations[0].derives;
    assert_eq!(derives[0].candidate_families, ["serialize"]);
    assert_eq!(derives[1].candidate_families, ["deserialize"]);
    assert_eq!(
        derives[0].lexical_import_candidates[0].written_path,
        "norito::codec::Encode"
    );
    assert_eq!(derives[0].resolution, "lexical_import_evidence_only");
    assert!(report.declarations[0].declared_literals.is_empty());
}

#[test]
fn conditional_aliases_and_cfg_attr_derives_remain_unresolved() {
    let report = inspect(
        "#[cfg(feature = \"a\")] use norito::Encode as Wire;\n#[cfg(not(feature = \"a\"))] use outside::Other as Wire;\n#[cfg_attr(feature = \"a\", derive(Wire, norito::Decode))]\nstruct Value;",
    );
    let site = &report.declarations[0].derives[0];
    assert_eq!(site.lexical_import_candidates.len(), 2);
    assert_eq!(site.resolution, "ambiguous_lexical_imports");
    assert_eq!(site.conditions, ["feature = \"a\""]);
    assert!(
        site.lexical_import_candidates
            .iter()
            .all(|binding| !binding.conditions.is_empty())
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "path_resolution")
    );
}

#[test]
fn inline_modules_do_not_inherit_parent_import_aliases() {
    let report =
        inspect("use norito::Encode as Write; mod child { #[derive(Write)] struct Value; }");
    let value = &report.declarations[0];
    assert!(value.derives[0].lexical_import_candidates.is_empty());
    assert_eq!(value.derives[0].resolution, "requires_semantic_resolution");
    assert_eq!(value.scopes[1].identifier.as_deref(), Some("child"));
}

#[test]
fn function_local_items_and_nested_generic_slots_are_preserved() {
    let source = "fn make() { use norito::NoritoSchema as Identity; #[derive(Identity)] struct Local<'a, T, const N: usize>(&'a [T; N]) where T: Clone; }";
    let report = inspect(source);
    let item = &report.declarations[0];
    assert_eq!(
        item.generics
            .iter()
            .map(|slot| slot.kind.as_str())
            .collect::<Vec<_>>(),
        ["lifetime", "type", "const"]
    );
    assert_eq!(item.generics[0].identifier, "'a");
    assert_eq!(item.generics[2].span.source, "const N: usize");
    assert_eq!(item.where_clause.as_ref().unwrap().source, "where T: Clone");
    assert!(item.scopes.iter().any(|scope| scope.kind == "function"));
    assert!(item.scopes.iter().any(|scope| scope.kind == "block"));
    assert_eq!(item.derives[0].candidate_families, ["identity"]);
}

#[test]
fn direct_and_generic_manual_impls_retain_exact_self_and_trait_spans() {
    let report = inspect(
        "use norito::NoritoSerialize as Serialize;\nimpl<T> Serialize for Wrapper<T> {}\nimpl Wrapper<u8> {}\n",
    );
    assert_eq!(report.implementations.len(), 2);
    let item = &report.implementations[0];
    assert_eq!(item.self_type.source, "Wrapper<T>");
    assert_eq!(
        item.trait_site.as_ref().unwrap().candidate_families,
        ["serialize"]
    );
    assert_eq!(item.generics[0].identifier, "T");
    assert!(report.implementations[1].trait_site.is_none());
}

#[test]
fn macros_and_includes_are_references_without_invented_declarations() {
    let source = "macro_rules! make { () => { struct Hidden; }; }\nmake!();\ninclude!(\"fragment.rs\");\ninclude!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n";
    let report = inspect(source);
    assert!(report.declarations.is_empty());
    assert_eq!(report.references.len(), 4);
    assert_eq!(report.references[0].kind, "macro_definition");
    assert_eq!(
        report.references[2].literal_path.as_deref(),
        Some("fragment.rs")
    );
    assert!(report.references[3].literal_path.is_none());
    assert_eq!(
        report
            .unresolved
            .iter()
            .filter(|item| item.kind == "include")
            .count(),
        2
    );
}

#[test]
fn module_attributes_cfg_and_external_module_paths_remain_visible() {
    let report = inspect(
        "#[cfg(feature = \"json\")] #[model] mod model { struct Value; }\n#[path = \"somewhere.rs\"] mod external;\n",
    );
    assert_eq!(report.declarations.len(), 1);
    assert_eq!(
        report.declarations[0].conditions,
        ["#[cfg(feature = \"json\")]"]
    );
    assert!(
        report.declarations[0].scopes[1]
            .attributes
            .iter()
            .any(|attr| attr.source == "#[model]")
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "attribute")
    );
    assert_eq!(
        report.references[0].literal_path.as_deref(),
        Some("somewhere.rs")
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "external_module")
    );
}

#[test]
fn only_existing_source_literals_are_reported_as_identity_declarations() {
    let report = inspect(
        "#[derive(norito::NoritoSchema)] #[norito_schema(name = \"captured::Value\", frame = \"alloc::string::String\")] #[norito(schema_name = \"current::Value\")] struct Value;",
    );
    let values = &report.declarations[0].declared_literals;
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].nominal.as_deref(), Some("captured::Value"));
    assert_eq!(values[0].frame.as_deref(), Some("alloc::string::String"));
    assert_eq!(values[1].kind, "active_schema_name");
    assert_eq!(values[1].nominal.as_deref(), Some("current::Value"));
}

#[test]
fn field_variant_and_generic_attributes_are_retained_as_exact_source_sites() {
    let source = "enum Value<#[cfg(feature = \"a\")] T> { #[cfg(feature = \"b\")] A { #[norito(skip)] field: T } }";
    let report = inspect(source);
    assert!(
        report
            .attributes
            .iter()
            .any(|attr| attr.span.source == "#[cfg(feature = \"a\")]")
    );
    assert!(
        report
            .attributes
            .iter()
            .any(|attr| attr.span.source == "#[cfg(feature = \"b\")]")
    );
    assert!(
        report
            .attributes
            .iter()
            .any(|attr| attr.span.source == "#[norito(skip)]")
    );
    for attribute in &report.attributes {
        assert_eq!(
            &source[attribute.span.start_byte..attribute.span.end_byte],
            attribute.span.source
        );
    }
}

#[test]
fn invalid_file_is_hashed_and_marked_unparsed_without_partial_declarations() {
    let source = "struct Good; struct Broken {";
    let report = inspect_source("bad.rs", source).unwrap();
    assert!(report.parse_error.is_some());
    assert!(report.declarations.is_empty());
    assert_eq!(report.sha256, hex::encode(Sha256::digest(source)));
}

#[test]
fn physical_inventory_is_sorted_deduplicated_and_source_bound() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/z.rs"), "struct Z;").unwrap();
    fs::write(directory.path().join("src/a.rs"), "struct A;").unwrap();
    let selections = [PathBuf::from("src"), PathBuf::from("src/a.rs")];
    let report = inventory(directory.path(), &selections).unwrap();
    assert_eq!(
        report
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["src/a.rs", "src/z.rs"]
    );
    let previous = report.source_set_sha256;
    fs::write(directory.path().join("src/z.rs"), "struct Changed;").unwrap();
    assert_ne!(
        inventory(directory.path(), &selections)
            .unwrap()
            .source_set_sha256,
        previous
    );
}

#[test]
fn absolute_and_external_crate_aliases_preserve_spelling_and_cfg() {
    let report = inspect(
        "#[cfg(feature = \"wire\")] use ::norito::Encode as E; extern crate norito as codec; #[derive(E, codec::Decode)] struct Value;",
    );
    assert_eq!(report.imports[0].written_path, "::norito::Encode");
    assert_eq!(report.imports[0].conditions, ["#[cfg(feature = \"wire\")]"]);
    let derives = &report.declarations[0].derives;
    assert_eq!(derives[0].candidate_families, ["serialize"]);
    assert_eq!(derives[1].candidate_families, ["deserialize"]);
    assert_eq!(
        derives[1].lexical_import_candidates[0].written_path,
        "norito"
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "external_crate")
    );
}

#[test]
fn methods_preserve_conditional_local_types_and_associated_type_declarations() {
    let report = inspect(
        "trait Service { type Output<T>; #[cfg(feature = \"a\")] fn f() { struct Local; } } impl<T> Service for T where T: Clone { type Output<U> = U; #[cfg(feature = \"b\")] fn f() { struct Other; } }",
    );
    assert_eq!(
        report
            .declarations
            .iter()
            .filter(|item| item.kind == "associated_type")
            .count(),
        2
    );
    let local = report
        .declarations
        .iter()
        .find(|item| item.identifier == "Local")
        .unwrap();
    let other = report
        .declarations
        .iter()
        .find(|item| item.identifier == "Other")
        .unwrap();
    assert_eq!(local.conditions, ["#[cfg(feature = \"a\")]"]);
    assert_eq!(other.conditions, ["#[cfg(feature = \"b\")]"]);
    assert!(
        local
            .scopes
            .iter()
            .any(|scope| scope.identifier.as_deref() == Some("f"))
    );
    assert_eq!(
        report.implementations[0]
            .where_clause
            .as_ref()
            .unwrap()
            .source,
        "where T: Clone"
    );
}

#[test]
fn unknown_function_and_impl_attributes_are_explicitly_unexpanded() {
    let report = inspect(
        "#[expand] fn owner() { struct Local; } #[cfg_attr(feature = \"wire\", make_impl)] impl Value {} use ambiguous::*;",
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "attribute" && item.span.source == "expand")
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "conditional_attribute")
    );
    assert!(
        report
            .unresolved
            .iter()
            .any(|item| item.kind == "wildcard_import")
    );
    assert!(report.declarations[0].scopes.iter().any(|scope| {
        scope
            .attributes
            .iter()
            .any(|attr| attr.source == "#[expand]")
    }));
}

#[test]
fn directory_exclusions_are_reported_and_can_be_explicitly_selected() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("src/target")).unwrap();
    fs::write(directory.path().join("src/a.rs"), "struct A;").unwrap();
    fs::write(directory.path().join("src/target/b.rs"), "struct B;").unwrap();
    let report = inventory(directory.path(), &["src".into()]).unwrap();
    assert_eq!(report.selections, ["src"]);
    assert_eq!(report.excluded_directories, ["src/target"]);
    assert_eq!(report.files.len(), 1);
    let explicit = inventory(directory.path(), &["src/target".into()]).unwrap();
    assert!(explicit.excluded_directories.is_empty());
    assert_eq!(explicit.files[0].path, "src/target/b.rs");
}

#[cfg(unix)]
#[test]
fn source_selection_rejects_symlink_ancestors_and_nonregular_rust_files() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir(directory.path().join("actual")).unwrap();
    fs::write(directory.path().join("actual/a.rs"), "struct A;").unwrap();
    symlink("actual", directory.path().join("alias")).unwrap();
    assert!(inventory(directory.path(), &["alias/a.rs".into()]).is_err());
    let _socket = UnixListener::bind(directory.path().join("actual/socket.rs")).unwrap();
    assert!(inventory(directory.path(), &["actual".into()]).is_err());
}

#[test]
fn source_selection_rejects_outside_paths_non_utf8_and_oversized_inputs() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("bad.rs"), [0xff_u8]).unwrap();
    assert!(inventory(directory.path(), &["../outside.rs".into()]).is_err());
    assert!(inventory(directory.path(), &["bad.rs".into()]).is_err());
    let file = fs::File::create(directory.path().join("large.rs")).unwrap();
    file.set_len(MAX_SOURCE_BYTES + 1).unwrap();
    assert!(inventory(directory.path(), &["large.rs".into()]).is_err());
}
