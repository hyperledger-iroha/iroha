//! Exact current context authority leaf and production descendant inventory.

use super::{
    RustTokenV1, contains_no_words_v1, context_impl_has_no_production_mint_v1,
    descendant_module_has_no_context_mint_v1, exact_production_child_modules_v1, impl_parts_v1,
    item_is_exact_cfg_test_v1, rust_tokens_v1, source_pin_v1, top_level_item_ranges_v1,
    top_level_word_v1,
};

// Only these two imports connect the current opaque authority leaf to its
// actual consumers. Aliases, wildcard imports and wider reexports are rejected.
pub(super) fn exact_current_context_import_v1(item: &[RustTokenV1<'_>]) -> bool {
    [
        "pub(super) use context_authority_v1::ZkAmsPhase23RnsLinkContextV1;",
        "use super::{RNS_LINK_FAMILY_ORDER_V1, RNS_LINK_RELEASE_COMMITMENTS_V1, \
         RNS_LINK_VERSION_V1, ZK_AMS_PHASE23_RNS_LINK_RELEASE_RNS_LIMB_COUNT_V1, \
         ZkAmsPhase23RnsLinkContextV1, ZkAmsPhase23RnsLinkFamilyV1, \
         ZkAmsPhase23RnsLinkReleaseGeometryV1, \
         derive_zk_ams_phase23_rns_link_release_geometry_v1,};",
    ]
    .iter()
    .any(|source| rust_tokens_v1(source).is_some_and(|expected| item == expected.as_slice()))
}

// The authority moved into one small leaf. Check actual Rust privacy and
// production mint absence as well as its separately reviewed whole-source pin.
pub(super) fn current_context_leaf_is_private_v1(source: &str) -> bool {
    use RustTokenV1::Word;
    const CONTEXT: &str = "ZkAmsPhase23RnsLinkContextV1";
    const AXIS: &str = "ContextAxisDigestV1";
    const AXIS_ITEM: &str = "struct ContextAxisDigestV1([u8; 32]);";
    const CONTEXT_ITEM: &str = r#"
pub(in super::super) struct ZkAmsPhase23RnsLinkContextV1 {
    profile_digest: ContextAxisDigestV1,
    algorithm_manifest_digest: ContextAxisDigestV1,
    network_context_digest: ContextAxisDigestV1,
    statement_context_digest: ContextAxisDigestV1,
    transcript_digest: ContextAxisDigestV1,
    batch_digest: ContextAxisDigestV1,
    roster_digest: ContextAxisDigestV1,
    direct_key_admission_digest: ContextAxisDigestV1,
    canonical_map_set_digest: ContextAxisDigestV1,
}
"#;
    let Some(tokens) = rust_tokens_v1(source) else {
        return false;
    };
    let Some(items) = top_level_item_ranges_v1(&tokens) else {
        return false;
    };
    if !contains_no_words_v1(source, &["unsafe", "Clone", "Copy", "Default"])
        || !source.contains("#![forbid(unsafe_code)]")
        || !exact_production_child_modules_v1(source, &[])
    {
        return false;
    }
    let axis_item = rust_tokens_v1(AXIS_ITEM).expect("fixed axis syntax");
    let context_item = rust_tokens_v1(CONTEXT_ITEM).expect("fixed context syntax");
    let (mut axes, mut contexts, mut production_impls, mut test_impls) = (0, 0, 0, 0);
    for item in items {
        let item = &tokens[item];
        if !item
            .iter()
            .any(|token| matches!(token, Word(name) if *name == CONTEXT || *name == AXIS))
        {
            continue;
        }
        if item == axis_item.as_slice() {
            axes += 1;
        } else if item == context_item.as_slice() {
            contexts += 1;
        } else if let Some(implementation) = top_level_word_v1(item, "impl") {
            let Some((header, _)) = impl_parts_v1(item, implementation) else {
                return false;
            };
            if item[header] != [Word(CONTEXT)] {
                return false;
            }
            if item_is_exact_cfg_test_v1(item) {
                test_impls += 1;
            } else if context_impl_has_no_production_mint_v1(item, implementation, CONTEXT) {
                production_impls += 1;
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
    (axes, contexts, production_impls, test_impls) == (1, 1, 1, 1)
}

pub(super) fn exact_pinned_context_descendant_tree_v1(
    root: &str,
    authority: &str,
    external_source: &str,
    external_spool: &str,
    cross_field: &str,
) -> bool {
    const ROOT_CHILDREN: &[(&str, &str)] = &[
        (
            "context_authority_v1",
            "phase23_rns_link_context_authority_v1.rs",
        ),
        ("external_source", "phase23_rns_link_external_source.rs"),
        ("cross_field_v2", "phase23_rns_link_cross_field_v2.rs"),
    ];
    const EXTERNAL_CHILDREN: &[(&str, &str)] =
        &[("confidential_spool", "phase23_rns_link_external_spool.rs")];
    // These pins bind the reviewed source closure; the independent structural
    // checks below still reject newly introduced production mint/expansion paths.
    let authority_pin = (
        6_454,
        [
            0x48, 0x2f, 0x06, 0xe1, 0x05, 0xfc, 0x20, 0xbc, 0x79, 0xf0, 0x8b, 0x13, 0xde, 0xce,
            0x34, 0xb3, 0x7e, 0x7b, 0xa9, 0xce, 0x02, 0x2c, 0x32, 0x05, 0xf4, 0xc1, 0x2f, 0x2c,
            0x05, 0x69, 0xf7, 0x4a,
        ],
    );
    let descendants = [
        (
            external_source,
            (
                41_016,
                [
                    0xa9, 0x35, 0x9a, 0xe6, 0x5f, 0xb1, 0xca, 0x65, 0x10, 0xb7, 0x59, 0x6c, 0xbd,
                    0x55, 0xcf, 0x70, 0xa9, 0x1b, 0xe6, 0xe1, 0x60, 0x35, 0x81, 0xac, 0x05, 0xff,
                    0x4a, 0xf1, 0x90, 0x56, 0x8b, 0xd8,
                ],
            ),
            EXTERNAL_CHILDREN,
        ),
        (
            external_spool,
            (
                12_189,
                [
                    0x0d, 0x7d, 0x84, 0x4b, 0xfb, 0x85, 0x96, 0x66, 0xc4, 0x81, 0x01, 0x21, 0xc8,
                    0x4e, 0x3b, 0xf6, 0xb8, 0x1e, 0x55, 0xa9, 0x20, 0x23, 0x13, 0x7e, 0x04, 0x3f,
                    0xc5, 0xdf, 0x6d, 0x83, 0x7f, 0x3b,
                ],
            ),
            &[][..],
        ),
        (
            cross_field,
            (
                42_260,
                [
                    0xa8, 0x63, 0x11, 0x07, 0x18, 0x06, 0x4c, 0x76, 0xd2, 0xfb, 0xec, 0xef, 0x2b,
                    0xbe, 0x9b, 0xe8, 0xf0, 0x9a, 0x85, 0xbd, 0xaa, 0x98, 0x45, 0xa4, 0x15, 0xd2,
                    0x7b, 0x7f, 0x09, 0x15, 0x2b, 0x92,
                ],
            ),
            &[][..],
        ),
    ];
    source_pin_v1(authority) == authority_pin
        && current_context_leaf_is_private_v1(authority)
        && descendant_module_has_no_context_mint_v1(root)
        && exact_production_child_modules_v1(root, ROOT_CHILDREN)
        && descendants.iter().all(|(source, pin, children)| {
            source_pin_v1(source) == *pin
                && exact_production_child_modules_v1(source, children)
                && descendant_module_has_no_context_mint_v1(source)
        })
}

#[test]
fn current_context_tree_checks_actual_consumers_and_rejects_mint_mutations() {
    const LOGICAL_NOT: &str = r#"
fn inspect(ready: bool) -> bool {
    if !ready { return !ready; }
    if !(ready) { return false; }
    while !ready { return ready != false; }
    ready != false
}
"#;
    assert!(exact_production_child_modules_v1(LOGICAL_NOT, &[]));
    for expansion in [
        "fn inspect() { actual_macro!(true); }",
        "fn inspect() { if_alias!(true); }",
        "fn inspect() { r#if!(true); }",
        "fn inspect() { path::r#if!(true); }",
        "use std::include as r#if; fn inspect() { r#if!(\"mint.rs\"); }",
        "use std::include as if_alias; fn inspect() { if_alias!(\"mint.rs\"); }",
    ] {
        assert!(!exact_production_child_modules_v1(expansion, &[]));
    }
    let grouped =
        "#![allow(dead_code)] use super::{Left, Right}; const X: () = { assert!(true); };";
    let tokens = rust_tokens_v1(grouped).unwrap();
    let items = top_level_item_ranges_v1(&tokens).unwrap();
    assert_eq!(items.len(), 2);
    assert!(
        &tokens[items[0].clone()]
            == rust_tokens_v1("use super::{Left, Right};")
                .unwrap()
                .as_slice()
    );
    assert!(
        tokens[items[1].clone()] == rust_tokens_v1("const X: () = { assert!(true); };").unwrap()
    );
    for inner in [
        "#![cfg(test)]",
        "#![allow(unsafe_code)]",
        "#![unowned_attribute]",
    ] {
        let source = format!("{inner} fn read() {{}}");
        let tokens = rust_tokens_v1(&source).unwrap();
        assert!(top_level_item_ranges_v1(&tokens).is_none());
    }
    let root = include_str!("../../phase23_rns_link.rs");
    let authority = include_str!("../../phase23_rns_link_context_authority_v1.rs");
    let external = include_str!("../../phase23_rns_link_external_source.rs");
    let spool = include_str!("../../phase23_rns_link_external_spool.rs");
    assert!(current_context_leaf_is_private_v1(authority));
    for source in [root, external, spool] {
        assert!(descendant_module_has_no_context_mint_v1(source));
        for mint in [
            "fn mint() -> ZkAmsPhase23RnsLinkContextV1 { loop {} }",
            "type MintAlias = ZkAmsPhase23RnsLinkContextV1;",
            "impl Clone for ZkAmsPhase23RnsLinkContextV1 { fn clone(&self) -> Self { loop {} } }",
        ] {
            assert!(!descendant_module_has_no_context_mint_v1(&format!(
                "{source}\n{mint}"
            )));
        }
    }
    for mutant in [
        authority.replace("#[cfg(test)]", ""),
        authority.replace("#[cfg(test)]", "#[cfg(any(test, feature = \"mint\"))]"),
        authority.replace(
            "struct ContextAxisDigestV1",
            "#[derive(Clone, Copy)] struct ContextAxisDigestV1",
        ),
        authority.replace(
            "profile_digest: ContextAxisDigestV1,",
            "pub(super) profile_digest: ContextAxisDigestV1,",
        ),
        authority.replace(
            "profile_digest: ContextAxisDigestV1,",
            "profile_digest: [u8; 32],",
        ),
        authority.replace("pub(in super::super) struct", "pub(crate) struct"),
        format!("{authority}\ntype MintAlias = ZkAmsPhase23RnsLinkContextV1;"),
    ] {
        assert!(mutant != authority);
        assert!(!current_context_leaf_is_private_v1(&mutant));
    }
    for mutant in [
        root.replace(
            "pub(super) use context_authority_v1::",
            "pub(crate) use context_authority_v1::",
        ),
        root.replace(
            "use context_authority_v1::ZkAmsPhase23RnsLinkContextV1;",
            "use context_authority_v1::ZkAmsPhase23RnsLinkContextV1 as MintAlias;",
        ),
        external.replace(
            "ZkAmsPhase23RnsLinkContextV1,",
            "ZkAmsPhase23RnsLinkContextV1 as MintAlias,",
        ),
    ] {
        assert!(!descendant_module_has_no_context_mint_v1(&mutant));
    }
    let external_children = [("confidential_spool", "phase23_rns_link_external_spool.rs")];
    assert!(exact_production_child_modules_v1(
        external,
        &external_children
    ));
    assert!(exact_production_child_modules_v1(spool, &[]));
    for mutant in [
        external.replace(
            "mod confidential_spool;",
            "pub(super) mod confidential_spool;",
        ),
        external.replace(
            "mod confidential_spool;",
            "#[cfg(any())] mod confidential_spool;",
        ),
        external.replace("phase23_rns_link_external_spool.rs", "unowned_spool.rs"),
        format!("{external}\n#[path = \"unowned.rs\"] mod unowned;"),
    ] {
        assert!(mutant != external);
        assert!(!exact_production_child_modules_v1(
            &mutant,
            &external_children
        ));
    }
    for expansion in [
        "include!(\"mint.rs\");",
        "#[unowned_attribute] fn altered() {}",
    ] {
        assert!(!exact_production_child_modules_v1(
            &format!("{spool}\n{expansion}"),
            &[]
        ));
    }
}
