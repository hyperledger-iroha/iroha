//! Original monetary staking bounds and strict removal of node-local overrides.

use std::{num::NonZeroU32, path::PathBuf};

use iroha_config::{
    parameters::{
        actual::{LaneValidatorMode, Root as ActualConfig},
        defaults,
        user::{NexusStaking as UserStaking, Root as UserConfig},
    },
    profile::{Profile, ProfileId},
};
use iroha_config_base::{read::ConfigReader, toml::TomlSource};
use iroha_data_model::block::consensus::{PublicLaneStakingPolicy, PublicLaneStakingPolicyError};

fn base_reader() -> ConfigReader {
    let base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml");
    ConfigReader::new()
        .read_toml_with_extends(base_path)
        .expect("base config should load")
}

fn parse_actual_config(inline_toml: &str) -> ActualConfig {
    let table: toml::Table = inline_toml.parse().expect("inline TOML");
    base_reader()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<UserConfig>()
        .expect("node configuration")
        .parse()
        .expect("valid node configuration")
}

fn authored_policy() -> PublicLaneStakingPolicy {
    Profile::compiled(ProfileId::IrohaDevV1)
        .expect("complete genesis recipe")
        .genesis_recipe()
        .public_lane_staking
        .clone()
}

#[test]
fn staking_work_bounds_have_conservative_nonzero_defaults() {
    // These values are explicit genesis author choices, not node defaults.
    let staking = authored_policy();
    assert_eq!(staking.max_stake_shares_per_validator.get(), 256);
    assert_eq!(staking.max_pending_unbonds_per_share.get(), 8);
    assert_eq!(
        parse_actual_config("").nexus.staking.max_validators,
        defaults::nexus::staking::MAX_VALIDATORS
    );
}

#[test]
fn staking_work_bounds_roundtrip_only_in_the_original_parameter() {
    let mut staking = authored_policy();
    staking.max_stake_shares_per_validator = NonZeroU32::new(17).unwrap();
    staking.max_pending_unbonds_per_share = NonZeroU32::new(5).unwrap();
    let parameter = staking.clone().into_custom_parameter().unwrap();
    let decoded = PublicLaneStakingPolicy::from_custom_parameter(&parameter).unwrap();
    assert_eq!(decoded.max_stake_shares_per_validator.get(), 17);
    assert_eq!(decoded.max_pending_unbonds_per_share.get(), 5);
    assert_eq!(decoded, staking);
}

#[test]
fn staking_snapshot_must_fit_one_bounded_reward_exposure() {
    let cap = iroha_data_model::validation_fee_rewards::MAX_REWARD_RECIPIENTS;
    let mut accepted = authored_policy();
    accepted.max_stake_shares_per_validator = NonZeroU32::new(u32::try_from(cap).unwrap()).unwrap();
    let parameter = accepted.clone().into_custom_parameter().unwrap();
    assert_eq!(
        PublicLaneStakingPolicy::from_custom_parameter(&parameter)
            .unwrap()
            .max_stake_shares_per_validator
            .get() as usize,
        cap
    );
    accepted.max_stake_shares_per_validator =
        NonZeroU32::new(u32::try_from(cap + 1).unwrap()).unwrap();
    assert!(matches!(
        accepted.into_custom_parameter(),
        Err(PublicLaneStakingPolicyError::TooManyStakeShares)
    ));
}

const RETIRED: [(&str, &str); 8] = [
    ("min_validator_stake", "\"1\""),
    ("max_stake_shares_per_validator", "17"),
    ("max_pending_unbonds_per_share", "5"),
    ("unbonding_delay_ms", "0"),
    ("max_slash_bps", "10000"),
    ("stake_asset_id", "\"6TEAJqbb8oEPmLncoNiMRbLEK6tw\""),
    ("stake_escrow_account_id", "\"escrow@universal\""),
    ("slash_sink_account_id", "\"sink@universal\""),
];

#[test]
fn each_node_monetary_override_needs_the_legacy_genesis_profile_even_in_a_later_layer() {
    // A retired monetary key only describes the genesis terms of a network whose signed genesis
    // predates genesis-authored staking (`crypto.allowed_signing` selects that profile). Outside
    // the profile it is refused, never silently ignored.
    for (field, literal) in RETIRED {
        let allowed: toml::Table = "[nexus.staking]\nmax_validators = 7\n".parse().unwrap();
        let retired: toml::Table = format!("[nexus.staking]\n{field} = {literal}\n")
            .parse()
            .unwrap();
        let error = base_reader()
            .with_toml_source(TomlSource::inline(allowed))
            .with_toml_source(TomlSource::inline(retired))
            .read_and_complete::<UserConfig>()
            .expect("the reader admits the legacy genesis profile's keys")
            .parse()
            .expect_err("retired monetary input must not be silently ignored");
        let message = format!("{error:?}");
        assert!(
            message.contains("legacy genesis profile")
                && message.contains("crypto.allowed_signing"),
            "{field}: {message}"
        );
    }
}

#[test]
fn direct_json_reads_each_retired_monetary_field_into_the_legacy_profile_view() {
    let base = r#"{"public_validator_mode":"stake_elected","restricted_validator_mode":"admin_managed","max_validators":7}"#;
    let retained = norito::json::from_str::<UserStaking>(base).expect("retained JSON fields");
    for (field, literal) in RETIRED {
        let encoded = format!("{},\"{field}\":{literal}}}", &base[..base.len() - 1]);
        let read = norito::json::from_str::<UserStaking>(&encoded)
            .expect("retired JSON monetary field belongs to the legacy profile view");
        assert_ne!(
            format!("{read:?}"),
            format!("{retained:?}"),
            "{field} must be retained for the legacy profile, not dropped"
        );
    }
}

#[test]
fn retained_modes_and_capacity_still_parse_and_validate() {
    let staking = parse_actual_config(
        "[nexus.staking]\npublic_validator_mode = 'admin_managed'\nrestricted_validator_mode = 'stake_elected'\nmax_validators = 7\n",
    ).nexus.staking;
    assert_eq!(
        staking.public_validator_mode,
        LaneValidatorMode::AdminManaged
    );
    assert_eq!(
        staking.restricted_validator_mode,
        LaneValidatorMode::StakeElected
    );
    assert_eq!(staking.max_validators.get(), 7);
    for invalid in ["0", "129", "-1"] {
        let table = format!("[nexus.staking]\nmax_validators = {invalid}\n")
            .parse()
            .unwrap();
        let result = base_reader()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<UserConfig>();
        match result {
            Ok(user) => assert!(user.parse().is_err(), "invalid capacity {invalid}"),
            Err(_) => assert!(invalid == "0" || invalid == "-1"),
        }
    }
}
