//! Complete original staking recipe admission and canonical commitment checks.

use super::*;
use iroha_data_model::{account::address::ChainDiscriminantGuard, asset::AssetDefinitionId};
use iroha_model_base::domain::DomainId;

const FIELDS: [&str; 8] = [
    "min_validator_stake",
    "max_stake_shares_per_validator",
    "max_pending_unbonds_per_share",
    "unbonding_delay_ms",
    "max_slash_bps",
    "stake_asset_id",
    "stake_escrow_account_id",
    "slash_sink_account_id",
];

fn original_table() -> toml::Table {
    parse_profile_text(ProfileId::SoraNexusV1).unwrap()
}

fn staking(table: &mut toml::Table) -> &mut toml::Table {
    table
        .get_mut("genesis_recipe")
        .unwrap()
        .as_table_mut()
        .unwrap()
        .get_mut("public_lane_staking")
        .unwrap()
        .as_table_mut()
        .unwrap()
}

#[test]
fn staking_recipe_requires_the_complete_original_record() {
    let original = original_table();
    Profile::from_table(ProfileId::SoraNexusV1, original.clone()).unwrap();
    for field in FIELDS {
        let mut missing = original.clone();
        assert!(staking(&mut missing).remove(field).is_some());
        assert!(
            Profile::from_table(ProfileId::SoraNexusV1, missing).is_err(),
            "missing {field} must never acquire a default"
        );
    }
    let mut missing = original.clone();
    assert!(
        missing
            .get_mut("genesis_recipe")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .remove("public_lane_staking")
            .is_some()
    );
    assert!(Profile::from_table(ProfileId::SoraNexusV1, missing).is_err());
    let mut unknown = original;
    staking(&mut unknown).insert("legacy_staking".into(), toml::Value::Boolean(true));
    assert!(Profile::from_table(ProfileId::SoraNexusV1, unknown).is_err());
}

#[test]
fn staking_recipe_rejects_invalid_bounds_and_noncanonical_identities() {
    let original = original_table();
    for (field, invalid) in [
        ("min_validator_stake", toml::Value::String("-1".into())),
        ("max_stake_shares_per_validator", toml::Value::Integer(0)),
        ("max_stake_shares_per_validator", toml::Value::Integer(257)),
        ("max_pending_unbonds_per_share", toml::Value::Integer(0)),
        ("unbonding_delay_ms", toml::Value::Integer(-1)),
        ("max_slash_bps", toml::Value::Integer(10_001)),
        (
            "stake_asset_id",
            toml::Value::String("xor#universal".into()),
        ),
        (
            "stake_escrow_account_id",
            toml::Value::String("escrow@universal".into()),
        ),
        (
            "slash_sink_account_id",
            toml::Value::String("sink@universal".into()),
        ),
    ] {
        let mut changed = original.clone();
        staking(&mut changed).insert(field.into(), invalid);
        assert!(
            Profile::from_table(ProfileId::SoraNexusV1, changed).is_err(),
            "invalid {field}"
        );
    }
}

#[test]
fn staking_recipe_preserves_all_nondefault_fields_and_author_context() {
    let _caller = ChainDiscriminantGuard::enter(753);
    let mut table = original_table();
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::parse_fully_qualified("staking.example").unwrap(),
        "bond".parse().unwrap(),
    );
    let escrow = keyless_role_account(ProfileId::SoraNexusV1, KeylessRole::ALL[0]);
    let sink = keyless_role_account(ProfileId::SoraNexusV1, KeylessRole::ALL[1]);
    let expected = PublicLaneStakingPolicy {
        min_validator_stake: "17.25".parse().unwrap(),
        max_stake_shares_per_validator: std::num::NonZeroU32::new(7).unwrap(),
        max_pending_unbonds_per_share: std::num::NonZeroU32::new(3).unwrap(),
        unbonding_delay_ms: 43_210,
        max_slash_bps: 1_001,
        stake_asset_id: asset.clone(),
        stake_escrow_account_id: escrow.clone(),
        slash_sink_account_id: sink.clone(),
    };
    for (field, value) in [
        ("min_validator_stake", toml::Value::String("17.25".into())),
        ("max_stake_shares_per_validator", toml::Value::Integer(7)),
        ("max_pending_unbonds_per_share", toml::Value::Integer(3)),
        ("unbonding_delay_ms", toml::Value::Integer(43_210)),
        ("max_slash_bps", toml::Value::Integer(1_001)),
        ("stake_asset_id", toml::Value::String(asset.to_string())),
        (
            "stake_escrow_account_id",
            toml::Value::String(escrow.to_i105_for_discriminant(369).unwrap()),
        ),
        (
            "slash_sink_account_id",
            toml::Value::String(sink.to_i105_for_discriminant(369).unwrap()),
        ),
    ] {
        staking(&mut table).insert(field.into(), value);
    }
    let profile = Profile::from_table(ProfileId::SoraNexusV1, table).unwrap();
    assert_eq!(profile.genesis_recipe().public_lane_staking, expected);
    let original = expected.clone().into_custom_parameter().unwrap();
    assert_eq!(
        PublicLaneStakingPolicy::from_custom_parameter(&original).unwrap(),
        expected
    );
    assert_eq!(
        iroha_data_model::account::address::chain_discriminant(),
        753
    );
    assert_ne!(
        expected.stake_escrow_account_id,
        expected.slash_sink_account_id
    );
}

#[test]
fn every_staking_recipe_field_changes_the_original_consensus_commitment() {
    let base = Profile::compiled(ProfileId::SoraNexusV1).unwrap();
    let consensus = base.consensus_digest(4).unwrap();
    let policy = base.policy_digest().unwrap();
    let alternate = keyless_role_account(ProfileId::SoraNexusV1, KeylessRole::ALL[0]);
    for field in 0..8 {
        let mut changed = base.clone();
        let staking = &mut changed.genesis_recipe.public_lane_staking;
        match field {
            0 => staking.min_validator_stake = 17_u32.into(),
            1 => staking.max_stake_shares_per_validator = std::num::NonZeroU32::new(7).unwrap(),
            2 => staking.max_pending_unbonds_per_share = std::num::NonZeroU32::new(3).unwrap(),
            3 => staking.unbonding_delay_ms = 43_210,
            4 => staking.max_slash_bps = 1_001,
            5 => {
                staking.stake_asset_id = AssetDefinitionId::derive_from_components(
                    DomainId::parse_fully_qualified("staking.example").unwrap(),
                    "bond".parse().unwrap(),
                )
            }
            6 => staking.stake_escrow_account_id = alternate.clone(),
            7 => staking.slash_sink_account_id = alternate.clone(),
            _ => unreachable!(),
        }
        assert_ne!(
            changed.genesis_recipe.public_lane_staking,
            base.genesis_recipe.public_lane_staking
        );
        assert_ne!(
            changed.consensus_digest(4).unwrap(),
            consensus,
            "field {field}"
        );
        assert_eq!(changed.policy_digest().unwrap(), policy, "field {field}");
    }
    for id in [
        ProfileId::IrohaDevV1,
        ProfileId::SoraNexusV1,
        ProfileId::SoraNexusV1Qual,
    ] {
        let loaded = Profile::compiled(id).unwrap();
        let local = loaded
            .static_config()
            .get("nexus")
            .and_then(|n| n.get("staking"));
        for field in FIELDS {
            assert!(
                local.and_then(|s| s.get(field)).is_none(),
                "{id}: local {field}"
            );
        }
        let original = loaded
            .genesis_recipe()
            .public_lane_staking
            .clone()
            .into_custom_parameter()
            .unwrap();
        assert_eq!(
            PublicLaneStakingPolicy::from_custom_parameter(&original).unwrap(),
            loaded.genesis_recipe().public_lane_staking
        );
    }
}
