//! Nexus asset-selector configuration parsing tests.

use super::*;
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
fn nexus_staking_parse_accepts_asset_alias_selector() {
    let cfg = NexusStaking {
        stake_asset_id: "xor#universal".to_owned(),
        ..NexusStaking::default()
    };
    let mut emitter = Emitter::new();
    let parsed = cfg
        .parse(&mut emitter)
        .expect("staking config should parse");
    assert_eq!(parsed.stake_asset_id, "xor#universal");
    assert!(emitter.into_result().is_ok());
}
#[test]
fn nexus_fees_parse_rejects_invalid_asset_selector() {
    let cfg = NexusFees {
        fee_asset_id: "invalid selector".to_owned(),
        ..NexusFees::default()
    };
    let mut emitter = Emitter::new();
    assert!(cfg.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}
#[test]
fn nexus_fees_parse_accepts_xor_alias_selector() {
    let cfg = NexusFees {
        fee_asset_id: "xor#universal".to_owned(),
        ..NexusFees::default()
    };
    let mut emitter = Emitter::new();
    let parsed = cfg.parse(&mut emitter).expect("fees config should parse");
    assert_eq!(parsed.fee_asset_id, "xor#universal");
    assert!(emitter.into_result().is_ok());
}
#[test]
fn nexus_fees_parse_accepts_canonical_xor_asset_definition_id() {
    let canonical_xor = defaults::nexus::fees::fee_asset_id();
    let cfg = NexusFees {
        fee_asset_id: canonical_xor.clone(),
        ..NexusFees::default()
    };
    let mut emitter = Emitter::new();
    let parsed = cfg.parse(&mut emitter).expect("fees config should parse");
    assert_eq!(parsed.fee_asset_id, canonical_xor);
    assert!(emitter.into_result().is_ok());
}
#[test]
fn nexus_fees_parse_rejects_noncanonical_xor_selectors() {
    for selector in [
        " xor#universal",
        "xor#universal ",
        "XOR#universal",
        "xor#Universal",
        "xor#universal.universal",
    ] {
        let cfg = NexusFees {
            fee_asset_id: selector.to_owned(),
            ..NexusFees::default()
        };
        let mut emitter = Emitter::new();
        assert!(
            cfg.parse(&mut emitter).is_none(),
            "noncanonical selector `{selector}` must be rejected"
        );
        assert!(emitter.into_result().is_err());
    }
}
#[test]
fn nexus_fees_parse_uses_typed_sponsor_vault_custody_account() {
    let cfg = NexusFees::default();
    let mut emitter = Emitter::new();
    let parsed = cfg.parse(&mut emitter).expect("fees config should parse");
    assert_eq!(
        parsed.sponsor_vault_custody_account_id,
        defaults::nexus::fees::sponsor_vault_custody_account_id()
    );
    assert!(emitter.into_result().is_ok());
}
#[test]
fn nexus_fees_parse_rejects_invalid_sponsor_vault_custody_account() {
    let result = std::panic::catch_unwind(|| {
        let cfg = NexusFees {
            sponsor_vault_custody_account_id: "not-an-account".to_owned(),
            ..NexusFees::default()
        };
        let mut emitter = Emitter::new();
        let _ = cfg.parse(&mut emitter);
    });
    assert!(result.is_err());
}
#[test]
fn nexus_fees_parse_rejects_non_xor_asset_selector() {
    let cfg = NexusFees {
        fee_asset_id: "pkr#paynet".to_owned(),
        ..NexusFees::default()
    };
    let mut emitter = Emitter::new();
    assert!(cfg.parse(&mut emitter).is_none());
    assert!(emitter.into_result().is_err());
}

#[test]
fn nexus_fees_parse_canonicalizes_fee_exempt_authorities_as_a_typed_set() {
    let authority = |seed| {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive deterministic fee-exempt authority")
                .public_key()
                .clone(),
        )
    };
    let first = authority(0x41);
    let second = authority(0x42);
    let config = NexusFees {
        successful_claim_fee_exempt_authorities: vec![
            second.canonical_i105().expect("canonical second authority"),
            first.canonical_i105().expect("canonical first authority"),
            first
                .canonical_i105()
                .expect("canonical duplicate authority"),
        ],
        ..NexusFees::default()
    };

    let mut emitter = Emitter::new();
    let parsed = config.parse(&mut emitter).expect("valid authority set");
    assert!(emitter.into_result().is_ok());
    assert_eq!(
        parsed.successful_claim_fee_exempt_authorities,
        BTreeSet::from([first, second])
    );
}

#[test]
fn nexus_fees_parse_rejects_noncanonical_fee_exempt_authorities() {
    let authority = AccountId::new(
        KeyPair::try_from_seed(vec![0x43; 32], Algorithm::Ed25519)
            .expect("derive deterministic fee-exempt authority")
            .public_key()
            .clone(),
    );
    let canonical = authority.canonical_i105().expect("canonical authority");
    for literal in [
        String::new(),
        "merchant@paynet".to_owned(),
        format!(" {canonical}"),
    ] {
        let config = NexusFees {
            successful_claim_fee_exempt_authorities: vec![literal],
            ..NexusFees::default()
        };
        let mut emitter = Emitter::new();
        assert!(config.parse(&mut emitter).is_none());
        assert!(emitter.into_result().is_err());
    }
}

#[test]
fn nexus_fees_use_nominal_non_negative_quantities() {
    let cfg = NexusFees::default();
    for value in [
        &cfg.base_fee,
        &cfg.per_byte_fee,
        &cfg.per_instruction_fee,
        &cfg.per_gas_unit_fee,
    ] {
        value
            .as_numeric()
            .validate_decimal()
            .expect("fee quantity is canonical");
    }
    assert!(
        Quantity::try_from_numeric(iroha_primitives::numeric::Numeric::new(-1_i32, 0)).is_err()
    );
}
