//! Owner, quote-bound, canonical dataspace and stale-policy namespace tests.
use super::*;
use iroha_data_model::sns::fixtures::{default_policy, steward_account};
use iroha_model_base::metadata::Metadata;

fn policy() -> SuffixPolicyV1 {
    let mut value = default_policy();
    value.suffix_id = DOMAIN_NAME_SUFFIX_ID;
    value.suffix = "domain".into();
    value.pricing[0].label_regex = "^[a-z]+\\.universal$".into();
    value
}

#[test]
fn exact_one_year_quote_binds_owner_policy_asset_rent_and_network_deadline() {
    let policy = policy();
    let owner = steward_account();
    let quote = quote_domain(
        &policy,
        DomainId::parse_fully_qualified("developer.universal").unwrap(),
        DataSpaceId::UNIVERSAL,
        owner.clone(),
        10_000,
        Duration::from_secs(600),
    )
    .unwrap();
    assert_eq!(quote.domain, "developer.universal");
    assert_eq!(quote.rent, "0.5".parse().unwrap());
    assert_eq!(quote.valid_until_ms, 310_000);
    let instruction = &quote.request.intents[0];
    assert_eq!(instruction.quote_guard.max_amount, quote.rent);
    assert_eq!(
        instruction.quote_guard.expected_policy_version,
        policy.policy_version
    );
    assert_eq!(
        instruction.quote_guard.expected_payment_asset,
        quote.payment_asset
    );
    assert_eq!(instruction.quote_guard.valid_until_ms, quote.valid_until_ms);
    assert_eq!(instruction.acquisition.term_years, 1);
    assert!(
        matches!(&instruction.intent, AliasIntentV1::Domain(value) if value.owner == owner && value.domain.dataspace_id == DataSpaceId::UNIVERSAL)
    );
    let wire = norito::json::to_vec(&quote.request).unwrap();
    assert_eq!(
        norito::json::from_slice::<AliasSetupPlanRequestV1>(&wire).unwrap(),
        quote.request
    );
}

#[test]
fn namespace_rejects_missing_time_wrong_policy_and_unavailable_one_year_term() {
    let mut policy = policy();
    let domain = DomainId::parse_fully_qualified("developer.universal").unwrap();
    let owner = steward_account();
    for (now, ttl) in [
        (0, Duration::from_secs(60)),
        (u64::MAX, Duration::from_secs(60)),
        (1, Duration::ZERO),
    ] {
        assert!(
            quote_domain(
                &policy,
                domain.clone(),
                DataSpaceId::UNIVERSAL,
                owner.clone(),
                now,
                ttl
            )
            .is_err()
        );
    }
    policy.min_term_years = 2;
    assert!(
        quote_domain(
            &policy,
            domain.clone(),
            DataSpaceId::UNIVERSAL,
            owner.clone(),
            1,
            Duration::from_secs(60)
        )
        .is_err()
    );
    policy.min_term_years = 1;
    policy.suffix_id = DATASPACE_ALIAS_SUFFIX_ID;
    assert!(
        quote_domain(
            &policy,
            domain,
            DataSpaceId::UNIVERSAL,
            owner,
            1,
            Duration::from_secs(60)
        )
        .is_err()
    );
}

#[test]
fn dynamic_dataspace_requires_exact_active_record_and_canonical_numeric_mapping() {
    let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, "builders").unwrap();
    let mut record = NameRecordV1::new(
        selector.clone(),
        steward_account(),
        Vec::new(),
        0,
        1,
        100,
        200,
        300,
        Metadata::default(),
    );
    assert_eq!(
        active_dataspace(&record, "builders", 50).unwrap(),
        DataSpaceId::from_hash(&selector.name_hash())
    );
    assert!(active_dataspace(&record, "different", 50).is_err());
    assert!(active_dataspace(&record, "builders", 100).is_err());
    record.name_hash[0] ^= 1;
    assert!(active_dataspace(&record, "builders", 50).is_err());
    record.name_hash = selector.name_hash();
    record.metadata.insert(
        "sns.dataspace_id".parse().unwrap(),
        iroha_primitives::json::Json::new(55_u64),
    );
    assert_eq!(
        active_dataspace(&record, "builders", 50).unwrap(),
        DataSpaceId::new(55)
    );
    record.metadata.insert(
        "sns.dataspace_id".parse().unwrap(),
        iroha_primitives::json::Json::new("55"),
    );
    assert!(active_dataspace(&record, "builders", 50).is_err());
    assert_eq!(catalog_dataspace(&[], "builders").unwrap(), None);
}

#[test]
fn static_catalog_rejects_conflicts_and_preserves_nondefault_ids() {
    let entry = |name: &str, id| NexusDataspaceCatalogStatus {
        alias: name.to_owned(),
        dataspace_id: id,
        ..NexusDataspaceCatalogStatus::default()
    };
    assert_eq!(
        catalog_dataspace(&[entry("builders", 7)], "builders").unwrap(),
        Some(DataSpaceId::new(7))
    );
    assert!(catalog_dataspace(&[entry("builders", 7), entry("builders", 8)], "builders").is_err());
    assert!(catalog_dataspace(&[entry("builders", 7), entry("other", 7)], "builders").is_err());
    assert_eq!(
        catalog_dataspace(&[entry("builders", 7), entry("builders", 7)], "builders").unwrap(),
        Some(DataSpaceId::new(7))
    );
}
