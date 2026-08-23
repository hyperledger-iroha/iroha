#[test]
fn trigger_decl_builds_typed_metadata() {
    use iroha_data_model::account::AccountId;
    let authority_literal = sample_account_literal();
    let program = parse(&format!(
        r#"
        seiyaku C {{
            kotoage fn run() authorize("RunTrigger") {{}}
            trigger wake -> run {{
                on time pre_commit;
                repeats 2;
                authority "{authority_literal}";
                metadata {{ tag: "alpha"; count: 1; enabled: true; }}
            }}
        }}
        "#,
    ))
    .expect("parse trigger decl");
    let typed = analyze(&program).expect("analyze trigger decl");
    assert_eq!(typed.triggers.len(), 1);
    let trigger = &typed.triggers[0];
    assert_eq!(trigger.id.to_string(), "wake");
    assert!(matches!(trigger.filter, EventFilterBox::Time(_)));
    assert_eq!(trigger.repeats, Repeats::Exactly(2));
    assert_eq!(
        trigger.authority,
        Some(
            AccountId::parse_encoded(authority_literal.as_str())
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .expect("authority literal"),
        )
    );
    assert!(!trigger.metadata.is_empty());
}
analyze_error_code_cases! {
    trigger_metadata_json_parse_uses_json_literal_diagnostics:
    duplicate = include_str!( "semantic/test_sources/trigger_metadata_json_parse_uses_json_literal_diagnostics_1.ko" ) => "E_JSON_DUPLICATE_KEY";
    malformed = include_str!( "semantic/test_sources/trigger_metadata_json_parse_uses_json_literal_diagnostics_2.ko" ) => "E_JSON_LITERAL_INVALID";
}
#[test]
fn trigger_metadata_json_parse_obeys_the_canonical_call_contract() {
    let trigger_source = |value: &str| {
        format!(
            r#"
            seiyaku C {{
                kotoage fn run() authorize("RunTrigger") {{}}
                trigger wake -> run {{
                    on time pre_commit;
                    metadata {{ payload: {value}; }}
                }}
            }}
            "#,
        )
    };
    for value in [r#"Json::parse("{}")"#, r#"Json::parse(value: "{}")"#] {
        let source = trigger_source(value);
        let program = parse(&source).expect("canonical Json::parse metadata should parse");
        analyze(&program).unwrap_or_else(|error| {
            panic!("canonical trigger metadata `{value}` failed: {error:?}")
        });
    }
    for (value, code, message) in [
        (
            r#"Json::parse(raw: "{}")"#,
            "E_UNKNOWN_NAMED_ARGUMENT",
            "call `Json::parse` has no parameter named `raw`",
        ),
        ("Json::parse()", "K2003", "Json::parse expects one argument"),
        (
            r#"Json::parse("{}", "{}")"#,
            "K2003",
            "Json::parse expects one argument",
        ),
        (
            "Json::parse(value: dynamic)",
            "E_JSON_LITERAL_REQUIRED",
            JSON_LITERAL_REQUIRED_MESSAGE,
        ),
        (
            r#"json("{}")"#,
            "E_NON_CANONICAL_BUILTIN",
            "legacy or non-canonical builtin spelling `json` is not supported; use `Json::parse`",
        ),
    ] {
        let error = analyze_error(&trigger_source(value));
        assert_eq!(error.code, code, "{value}: {error:?}");
        assert_eq!(error.message, message, "{value}: {error:?}");
    }
}
#[test]
fn trigger_decl_supports_data_filter() {
    let program = parse(include_str!(
        "semantic/test_sources/trigger_decl_supports_data_filter_1.ko"
    ))
    .expect("parse trigger decl");
    let typed = analyze(&program).expect("analyze trigger decl");
    let trigger = &typed.triggers[0];
    assert!(matches!(
        trigger.filter,
        EventFilterBox::Data(DataEventFilter::Any)
    ));
}
#[test]
fn trigger_decl_supports_structured_asset_data_filter() {
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "rose".parse().expect("name"),
    );
    let asset_definition_literal = asset_definition.to_string();
    let program = parse(&format!(
        r#"
        seiyaku C {{
            kotoage fn run() authorize("RunTrigger") {{}}
            trigger wake -> run {{
                on data asset added {{
                    asset_definition "{asset_definition_literal}";
                }}
            }}
        }}
        "#,
    ))
    .expect("parse trigger decl");
    let typed = analyze(&program).expect("analyze trigger decl");
    let trigger = &typed.triggers[0];
    assert_eq!(
        trigger.filter,
        EventFilterBox::Data(DataEventFilter::Asset(
            AssetEventFilter::new()
                .for_events(AssetEventSet::Added)
                .for_asset_definition(asset_definition),
        ))
    );
}
#[test]
fn trigger_decl_supports_transfer_specific_asset_filter() {
    use iroha_data_model::account::ParsedAccountId;
    let source_literal = sample_account_literal();
    let source = AccountId::parse_encoded(source_literal.as_str())
        .map(ParsedAccountId::into_account_id)
        .expect("source account");
    let destination_literal = {
        let key_pair = iroha_crypto::KeyPair::try_random().expect("destination key");
        AccountId::new(key_pair.public_key().clone()).to_string()
    };
    let destination = AccountId::parse_encoded(destination_literal.as_str())
        .map(ParsedAccountId::into_account_id)
        .expect("destination account");
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "rose".parse().expect("name"),
    );
    let program = parse(&format!(
        r#"
        seiyaku C {{
            kotoage fn run() authorize("RunTrigger") {{}}
            trigger wake -> run {{
                on data asset transferred {{
                    asset_definition "{asset_definition}";
                    source_account "{source_literal}";
                    destination_account "{destination_literal}";
                }}
            }}
        }}
        "#
    ))
    .expect("parse transfer trigger");
    let typed = analyze(&program).expect("analyze transfer trigger");
    assert_eq!(
        typed.triggers[0].filter,
        EventFilterBox::Data(DataEventFilter::Asset(
            AssetEventFilter::new()
                .for_events(AssetEventSet::Transferred)
                .for_asset_definition(asset_definition)
                .for_transfer_source_account(source)
                .for_transfer_destination_account(destination),
        ))
    );
}
#[test]
fn trigger_decl_supports_structured_data_filters_for_core_families() {
    use iroha_data_model::{
        account::{AccountId, ParsedAccountId},
        events::{
            EventFilterBox,
            data::{
                DataEventFilter,
                prelude::{
                    AccountEventFilter, AccountEventSet, AssetDefinitionEventFilter,
                    AssetDefinitionEventSet, AssetEventFilter, AssetEventSet,
                    ConfigurationEventFilter, ConfigurationEventSet, DomainEventFilter,
                    DomainEventSet, ExecutorEventFilter, ExecutorEventSet, NftEventFilter,
                    NftEventSet, PeerEventFilter, PeerEventSet, RoleEventFilter, RoleEventSet,
                    TriggerEventFilter, TriggerEventSet,
                },
            },
        },
        nft::NftId,
        peer::PeerId,
        role::RoleId,
        rwa::RwaId,
        trigger::TriggerId,
    };
    let account_literal = sample_account_literal();
    let account = AccountId::parse_encoded(account_literal.as_str())
        .map(ParsedAccountId::into_account_id)
        .expect("account");
    let peer_literal = "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D";
    let peer: PeerId = peer_literal.parse().expect("peer");
    let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let asset_definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "rose".parse().expect("name"),
    );
    let asset = AssetId::new(asset_definition.clone(), account.clone());
    let asset_literal = asset.canonical_literal();
    let nft: NftId = "n0$wonderland.universal".parse().expect("nft");
    let rwa: RwaId = format!(
        "{}$wonderland.universal",
        iroha_crypto::Hash::prehashed([7; iroha_crypto::Hash::LENGTH])
    )
    .parse()
    .expect("rwa");
    let trigger_id: TriggerId = "wake".parse().expect("trigger");
    let role_id: RoleId = "auditor".parse().expect("role");
    let cases = vec![
        (
            format!(
                r#"
                seiyaku C {{
            kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data peer added {{
                            peer "{peer_literal}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Peer(
                PeerEventFilter::new()
                    .for_events(PeerEventSet::Added)
                    .for_peer(peer),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data domain created {{
                            domain "{domain}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Domain(
                DomainEventFilter::new()
                    .for_events(DomainEventSet::Created)
                    .for_domain(domain.clone()),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data account created {{
                            account "{account_literal}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Account(
                AccountEventFilter::new()
                    .for_events(AccountEventSet::Created)
                    .for_account(account.clone()),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data asset added {{
                            asset "{asset_literal}";
                            asset_definition "{asset_definition}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Asset(
                AssetEventFilter::new()
                    .for_events(AssetEventSet::Added)
                    .for_asset(asset.clone())
                    .for_asset_definition(asset_definition.clone()),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data asset_definition created {{
                            asset_definition "{asset_definition}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::AssetDefinition(
                AssetDefinitionEventFilter::new()
                    .for_events(AssetDefinitionEventSet::Created)
                    .for_asset_definition(asset_definition.clone()),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data nft created {{
                            nft "{nft}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Nft(
                NftEventFilter::new()
                    .for_events(NftEventSet::Created)
                    .for_nft(nft),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data rwa created {{
                            rwa "{rwa}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Rwa(
                RwaEventFilter::new()
                    .for_events(RwaEventSet::Created)
                    .for_rwa(rwa),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data trigger created {{
                            trigger "{trigger_id}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Trigger(
                TriggerEventFilter::new()
                    .for_events(TriggerEventSet::Created)
                    .for_trigger(trigger_id),
            )),
        ),
        (
            format!(
                r#"
                seiyaku C {{
                    kotoage fn run() authorize("RunTrigger") {{}}
                    trigger wake -> run {{
                        on data role created {{
                            role "{role_id}";
                        }}
                    }}
                }}
                "#
            ),
            EventFilterBox::Data(DataEventFilter::Role(
                RoleEventFilter::new()
                    .for_events(RoleEventSet::Created)
                    .for_role(role_id),
            )),
        ),
        (
            include_str!("semantic/test_sources/trigger_decl_supports_structured_data_filters_for_core_families_1.ko")
            .to_string(),
            EventFilterBox::Data(DataEventFilter::Configuration(
                ConfigurationEventFilter::new().for_events(ConfigurationEventSet::Changed),
            )),
        ),
        (
            include_str!("semantic/test_sources/trigger_decl_supports_structured_data_filters_for_core_families_2.ko")
            .to_string(),
            EventFilterBox::Data(DataEventFilter::Executor(
                ExecutorEventFilter::new().for_events(ExecutorEventSet::Upgraded),
            )),
        ),
    ];
    for (src, expected_filter) in cases {
        let program = parse(&src).expect("parse trigger decl");
        let typed = analyze(&program).expect("analyze trigger decl");
        let trigger = &typed.triggers[0];
        assert_eq!(trigger.filter, expected_filter);
    }
}
#[test]
fn trigger_decl_supports_pipeline_filter() {
    use iroha_data_model::events::pipeline::{BlockEventFilter, BlockStatus};
    let program = parse(include_str!(
        "semantic/test_sources/trigger_decl_supports_pipeline_filter_1.ko"
    ))
    .expect("parse trigger decl");
    let typed = analyze(&program).expect("analyze trigger decl");
    let trigger = &typed.triggers[0];
    assert_eq!(
        trigger.filter,
        EventFilterBox::Pipeline(PipelineEventFilterBox::Block(
            BlockEventFilter::new().for_status(BlockStatus::Approved),
        ))
    );
}
#[test]
fn trigger_decl_supports_pipeline_transaction_approved_filter() {
    use iroha_data_model::events::pipeline::{TransactionEventFilter, TransactionStatus};
    let program = parse(include_str!(
        "semantic/test_sources/trigger_decl_supports_pipeline_transaction_approved_filter_1.ko"
    ))
    .expect("parse trigger decl");
    let typed = analyze(&program).expect("analyze trigger decl");
    let trigger = &typed.triggers[0];
    assert_eq!(
        trigger.filter,
        EventFilterBox::Pipeline(PipelineEventFilterBox::Transaction(
            TransactionEventFilter::new().for_status(TransactionStatus::Approved),
        ))
    );
}
#[test]
fn trigger_decl_rejects_invalid_data_matcher_literal() {
    let program = parse(include_str!(
        "semantic/test_sources/trigger_decl_rejects_invalid_data_matcher_literal_1.ko"
    ))
    .expect("parse trigger decl");
    let err = analyze(&program).expect_err("invalid matcher should error");
    assert!(
        err.message
            .contains("invalid `asset_definition` matcher literal")
    );
}
#[test]
fn trigger_decl_rejects_duplicate_data_matchers() {
    let asset_definition_literal = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "rose".parse().expect("name"),
    )
    .to_string();
    let program = parse(&format!(
        r#"
        seiyaku C {{
            kotoage fn run() authorize("RunTrigger") {{}}
            trigger wake -> run {{
                on data asset added {{
                    asset_definition "{asset_definition_literal}";
                    asset_definition "{asset_definition_literal}";
                }}
            }}
        }}
        "#,
    ))
    .expect("parse trigger decl");
    let err = analyze(&program).expect_err("duplicate matcher should error");
    assert!(err.message.contains("duplicate `asset_definition` matcher"));
}
analyze_reject_contains_tests! { trigger_decl_rejects_invalid_authority: include_str!( "semantic/test_sources/trigger_decl_rejects_invalid_authority_1.ko" ) => "parse trigger decl", err = "invalid authority should error", "invalid trigger authority"; }
#[test]
fn trigger_decl_accepts_canonical_domainless_authority() {
    let authority = sample_account_literal();
    let program = parse(&format!(
        r#"
        seiyaku C {{
            kotoage fn run() authorize("RunTrigger") {{}}
            trigger wake -> run {{
                on time pre_commit;
                authority "{authority}";
            }}
        }}
        "#,
    ))
    .expect("parse trigger declaration");
    let typed = analyze(&program).expect("canonical domainless authority must type-check");
    assert_eq!(
        typed.triggers[0]
            .authority
            .as_ref()
            .expect("typed trigger authority")
            .to_string(),
        authority,
    );
}
analyze_reject_contains_tests! { trigger_decl_requires_kotoage_entrypoint: include_str!( "semantic/test_sources/trigger_decl_requires_kotoage_entrypoint_1.ko" ) => "parse trigger decl", err = "non-kotoage target should error", "`kotoage`/`言挙げ` function"; }
#[test]
fn trigger_decl_cannot_target_lifecycle_entrypoints_through_constructed_ast() {
    for lifecycle in ["hajimari", "kaizen"] {
        let mut program = parse(
            include_str!("semantic/test_sources/trigger_decl_cannot_target_lifecycle_entrypoints_through_constructed_ast_1.ko"),
        )
        .expect("parse valid trigger declaration");
        let trigger = program
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Trigger(trigger) => Some(trigger),
                _ => None,
            })
            .expect("trigger declaration");
        trigger.call.entrypoint = lifecycle.to_owned();
        let error =
            analyze(&program).expect_err("lifecycle entrypoints must never be trigger callbacks");
        assert!(
            error
                .message
                .contains("must call a `kotoage`/`言挙げ` function"),
            "unexpected {lifecycle} callback error: {error:?}"
        );
    }
}
#[test]
fn semantic_analysis_defends_against_lifecycle_permission_hints() {
    let mut program = parse("seiyaku Demo { hajimari() {} }").expect("parse hajimari");
    let Item::Function(hajimari) = &mut program.items[0] else {
        panic!("expected hajimari")
    };
    hajimari.modifiers.permission = Some("SourceOwnedPermission".to_owned());
    let error = analyze(&program).expect_err("lifecycle permission must be rejected");
    assert!(
        error
            .message
            .contains("lifecycle authorization is runtime-defined")
    );
}
