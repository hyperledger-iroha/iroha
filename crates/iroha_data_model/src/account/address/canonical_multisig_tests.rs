//! Strict external multisig address validation shared by I105 and AccountId JSON.

use super::*;
use iroha_crypto::{Algorithm, KeyPair};

fn canonical_multisig_bytes() -> Vec<u8> {
    let members = [0x11, 0x22]
        .into_iter()
        .enumerate()
        .map(|(index, seed)| {
            let pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
            MultisigMember::new(pair.public_key().clone(), (index + 1) as u16).unwrap()
        })
        .collect();
    let account = AccountId::new_multisig(MultisigPolicy::new(2, members).unwrap());
    let address = AccountAddress::from_account_id(&account).unwrap();
    address.canonical_bytes().unwrap()
}

#[test]
fn external_multisig_address_rejects_noncanonical_policy_before_accepting_literal() {
    let canonical = canonical_multisig_bytes();
    assert_eq!(canonical.len(), 81);
    let mut invalids = vec![
        [
            canonical[..7].to_vec(),
            canonical[44..].to_vec(),
            canonical[7..44].to_vec(),
        ]
        .concat(),
        [
            canonical[..7].to_vec(),
            canonical[7..44].to_vec(),
            canonical[7..44].to_vec(),
        ]
        .concat(),
    ];
    for (offset, byte) in [(2, 2), (4, 0), (4, 4), (9, 0)] {
        let mut invalid = canonical.clone();
        invalid[offset] = byte;
        invalids.push(invalid);
    }
    for invalid in invalids {
        assert!(AccountAddress::from_canonical_bytes(&invalid).is_err());
        let literal = encode_i105_literal(753, &invalid).unwrap();
        assert!(AccountAddress::from_i105_for_discriminant(&literal, Some(753)).is_err());
        let _chain = ChainDiscriminantGuard::enter(753);
        assert!(AccountId::parse_encoded(&literal).is_err());

        assert!(
            norito::json::from_str::<AccountId>(&norito::json::to_json(&literal).unwrap()).is_err()
        );
    }
}

#[test]
fn external_multisig_address_preserves_complete_canonical_identity() {
    let canonical = canonical_multisig_bytes();
    let literal = encode_i105_literal(753, &canonical).unwrap();
    let _chain = ChainDiscriminantGuard::enter(753);
    let account = AccountId::parse_encoded(&literal).unwrap();
    assert_eq!(account.canonical_i105().unwrap(), literal);
    assert_eq!(
        account
            .to_account_address()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
        canonical
    );
    assert_eq!(account.multisig_policy().unwrap().threshold(), 2);
    assert_eq!(
        account
            .multisig_policy()
            .unwrap()
            .members()
            .iter()
            .map(|member| u32::from(member.weight()))
            .sum::<u32>(),
        3
    );

    assert_eq!(
        norito::json::from_str::<AccountId>(&norito::json::to_json(&account).unwrap()).unwrap(),
        account
    );
}
