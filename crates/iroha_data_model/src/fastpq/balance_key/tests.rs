//! Cross-context and full-controller regressions for the V1 transfer balance key.

use super::*;
use crate::account::{MultisigMember, MultisigPolicy, address::ChainDiscriminantGuard};
use iroha_crypto::{Algorithm, KeyPair};

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .into_parts()
            .0,
    )
}
fn asset(seed: u8) -> AssetDefinitionId {
    let mut uuid = [seed; 16];
    uuid[6] = 0x40 | (seed & 0x0f);
    uuid[8] = 0x80 | (seed & 0x3f);
    AssetDefinitionId::from_uuid_bytes(uuid).expect("canonical UUIDv4")
}

#[test]
fn balance_key_is_independent_of_chain_display_and_ambient_layout() {
    let asset = asset(3);
    let account = account(5);
    let expected = transfer_balance_key(&asset, &account).expect("canonical key");
    let mut displays = std::collections::BTreeSet::new();
    for discriminant in [0, 369, 753, 65_535] {
        let _display = ChainDiscriminantGuard::enter(discriminant);
        displays.insert(account.to_string());
        for flags in [0, 1, 2, 3, 4, 5, 6, 7, 0x1b, 0x3f] {
            let _layout = norito::core::DecodeFlagsGuard::enter(flags);
            let effective_flags = norito::core::get_decode_flags();
            assert_eq!(transfer_balance_key(&asset, &account).unwrap(), expected);
            assert_eq!(norito::core::get_decode_flags(), effective_flags);
        }
    }
    assert_eq!(
        displays.len(),
        4,
        "the test actually changes account display"
    );
    let key: FastpqBalanceKeyV1 = norito::decode_canonical(&expected).unwrap();
    assert_eq!(key.asset_definition, asset);
    assert_eq!(key.account, account);
    assert_eq!(norito::encode_canonical(&key).unwrap(), expected);
}

#[test]
fn balance_key_binds_asset_and_every_multisig_policy_field() {
    let first = account(5);
    let second = account(6);
    let member = |account: &AccountId, weight| {
        MultisigMember::new(account.expect_single_signatory().clone(), weight).unwrap()
    };
    let policy = |threshold, weight, reverse| {
        let mut members = vec![member(&first, weight), member(&second, 2)];
        if reverse {
            members.reverse();
        }
        AccountId::new_multisig(MultisigPolicy::new(threshold, members).unwrap())
    };
    let base = transfer_balance_key(&asset(3), &policy(2, 1, false)).unwrap();
    assert_eq!(
        base,
        transfer_balance_key(&asset(3), &policy(2, 1, true)).unwrap()
    );
    for changed in [
        policy(3, 1, false),
        policy(2, 2, false),
        first.clone(),
        second.clone(),
    ] {
        assert_ne!(base, transfer_balance_key(&asset(3), &changed).unwrap());
    }
    assert_ne!(
        base,
        transfer_balance_key(&asset(4), &policy(2, 1, false)).unwrap()
    );
}

#[test]
fn balance_key_rejects_display_strings_bare_payloads_and_alternate_layouts() {
    use norito::codec::Encode;
    let key = FastpqBalanceKeyV1 {
        asset_definition: asset(3),
        account: account(5),
    };
    let canonical = norito::encode_canonical(&key).unwrap();
    let display = format!("asset/{}/{}", key.asset_definition, key.account).into_bytes();
    let alternate = {
        let _layout = norito::core::DecodeFlagsGuard::enter(0);
        norito::to_bytes(&key).unwrap()
    };
    assert_ne!(alternate, canonical);
    let mut trailing = canonical.clone();
    trailing.push(0);
    for invalid in [display, key.encode(), alternate, trailing] {
        assert!(norito::decode_canonical::<FastpqBalanceKeyV1>(&invalid).is_err());
    }
}
