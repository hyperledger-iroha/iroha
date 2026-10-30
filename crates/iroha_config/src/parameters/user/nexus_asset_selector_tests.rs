//! Reject retired node-local staking currency while preserving local validator settings.

use super::*;
use iroha_config_base::toml::TomlSource;
fn checked_nexus_contract_ed25519_key_fixture() -> KeyPair {
    KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .expect("generate checked Nexus contract Ed25519 account key fixture")
}
#[test]
fn nexus_contract_fixture_uses_checked_ed25519_key_generation() {
    let key_pair = checked_nexus_contract_ed25519_key_fixture();
    let algorithm = key_pair
        .public_key()
        .try_algorithm()
        .expect("Nexus contract fixture account key advertises a valid algorithm");
    assert_eq!(algorithm, Algorithm::Ed25519);
}
#[test]
fn nexus_staking_rejects_retired_asset_alias_selector() {
    let table = "stake_asset_id = 'xor#universal'"
        .parse()
        .expect("retired asset selector is valid TOML syntax");
    let error = ConfigReader::new()
        .with_toml_source(TomlSource::inline(table))
        .read_and_complete::<NexusStaking>()
        .expect_err("staking currency must come from the original signed policy");
    let message = format!("{error:?}");
    assert!(message.contains("unknown parameter"), "{message}");
    assert!(message.contains("stake_asset_id"), "{message}");

    let encoded = r#"{"public_validator_mode":"stake_elected","restricted_validator_mode":"admin_managed","max_validators":7,"stake_asset_id":"xor#universal"}"#;
    let error = norito::json::from_str::<NexusStaking>(encoded)
        .expect_err("JSON must reject the same retired asset selector");
    let message = error.to_string();
    assert!(message.contains("unknown field"), "{message}");
    assert!(message.contains("stake_asset_id"), "{message}");
}

#[test]
fn nexus_staking_parse_preserves_retained_modes_and_capacity() {
    let cfg = NexusStaking {
        public_validator_mode: LaneValidatorModeConfig::AdminManaged,
        restricted_validator_mode: LaneValidatorModeConfig::StakeElected,
        max_validators: NonZeroU32::new(7).expect("nonzero validator capacity"),
    };
    let mut emitter = Emitter::new();
    let parsed = cfg
        .parse(&mut emitter)
        .expect("retained staking configuration should parse");
    assert_eq!(
        parsed.public_validator_mode,
        actual::LaneValidatorMode::AdminManaged
    );
    assert_eq!(
        parsed.restricted_validator_mode,
        actual::LaneValidatorMode::StakeElected
    );
    assert_eq!(parsed.max_validators.get(), 7);
    assert!(emitter.into_result().is_ok());
}
