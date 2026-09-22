//! Deterministic, non-shipping Kagemusha V1 consensus fixtures.

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::ValidatorPower,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, InstalledBeaconEpochBindingV1, KAGEMUSHA_CHAIN_VERSION_V1,
        KagemushaMintFinalityAuthorityGenerationV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1,
    },
};
#[cfg(test)]
use iroha_data_model::{
    block::consensus_v2::SumeragiV2GenesisContextParameters,
    isi::kagemusha_v1::{
        KagemushaMintFinalityAuthorityGenerationTemplateV1,
        KagemushaMintFinalityGenesisParametersV1,
    },
};

/// Build real paired-Pasta public keys for one immutable authority generation.
pub(crate) fn mint_finality_authority(
    network_id: NetworkId,
    generation: u64,
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityAuthorityGenerationV1 {
    let validators = roster
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            let seed_byte = 0xA0_u8
                .wrapping_add(u8::try_from(index).expect("test validator index fits in one byte"));
            crate::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[seed_byte; 32],
                generation,
                validator.validator.clone(),
            )
            .expect("derive deterministic paired-Pasta test validator keys")
        })
        .collect();
    let authority = KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        generation,
        validators,
    };
    authority
        .validate()
        .expect("valid test mint-finality authority");
    authority
}

/// Construct a complete retained-authority chain from genesis to a scheduling epoch.
///
/// Earlier non-genesis epochs each occupy one height. Genesis fills the prefix
/// before them, so every predecessor ID comes from a real contiguous authorization.
/// The target interval is explicit; a later epoch cannot start at height one.
/// This is authorization-body test data, not a manufactured boundary certificate.
pub(crate) fn mint_finality_authorization_and_authority(
    network_id: NetworkId,
    epoch: u64,
    first_height: u64,
    last_height: u64,
    roster: &[ValidatorPower],
) -> (
    KagemushaMintFinalityEpochAuthorizationV1,
    KagemushaMintFinalityAuthorityGenerationV1,
) {
    assert!(
        last_height >= first_height,
        "nonempty fixture authorization interval"
    );
    let authority = mint_finality_authority(network_id, 0, roster);
    if epoch == 0 {
        assert_eq!(
            first_height, 1,
            "genesis authorization starts at height one"
        );
        let authorization =
            KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, last_height)
                .expect("valid fixture genesis authorization");
        return (authorization, authority);
    }
    let genesis_last = first_height
        .checked_sub(epoch)
        .filter(|height| *height > 0)
        .expect("each earlier fixture epoch needs at least one positive height");
    let mut authorization =
        KagemushaMintFinalityEpochAuthorizationV1::genesis(&authority, genesis_last)
            .expect("valid fixture genesis predecessor");
    // A stable, explicit fixture beacon binding is retained through all epochs.
    // It is not accepted as a DKG transcript or boundary certificate by this helper.
    let authority_id = authority
        .authority_id()
        .expect("fixture authority identity");
    let mut beacon_domain = b"iroha-core-mint-finality-fixture-beacon".to_vec();
    beacon_domain.extend_from_slice(network_id.as_bytes());
    beacon_domain.extend_from_slice(&authority_id);
    let session_id = *iroha_crypto::Hash::new(&beacon_domain).as_ref();
    beacon_domain.extend_from_slice(b"installed-transcript");
    let transcript_hash = *iroha_crypto::Hash::new(&beacon_domain).as_ref();
    let beacon = BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
        session_id,
        transcript_hash,
    });
    for next_epoch in 1..=epoch {
        let first = authorization
            .last_height
            .checked_add(1)
            .expect("fixture height fits");
        let next = KagemushaMintFinalityEpochAuthorizationV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            epoch: next_epoch,
            first_height: first,
            last_height: if next_epoch == epoch {
                last_height
            } else {
                first
            },
            authority_generation: authority.generation,
            authority_id,
            beacon,
            previous_authorization_id: authorization
                .authorization_id()
                .expect("fixture predecessor identity"),
            transition_id: [0; 32],
            decision: KagemushaMintFinalityEpochDecisionV1::Retain,
        };
        next.validate_against_authority(&authority)
            .expect("fixture retains original authority");
        next.validate_successor(&authorization)
            .expect("contiguous fixture epoch authorization");
        authorization = next;
    }
    assert_eq!(authorization.first_height, first_height);
    (authorization, authority)
}

/// Build a structurally valid boundary authorization linked to the actual predecessor.
///
/// Tests must separately supply any incumbent certificate or activation readiness required
/// by their consumer. The deterministic transition binding below is fixture body data.
pub(crate) fn mint_finality_successor_authorization(
    previous: &KagemushaMintFinalityEpochAuthorizationV1,
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    last_height: u64,
    decision: KagemushaMintFinalityEpochDecisionV1,
) -> KagemushaMintFinalityEpochAuthorizationV1 {
    let authority_id = authority
        .authority_id()
        .expect("fixture authority identity");
    let previous_authorization_id = previous
        .authorization_id()
        .expect("fixture predecessor identity");
    let mut domain = b"iroha-core-mint-finality-fixture-boundary".to_vec();
    domain.extend_from_slice(&previous_authorization_id);
    domain.extend_from_slice(&authority_id);
    let transition_id = match decision {
        KagemushaMintFinalityEpochDecisionV1::Retain => [0; 32],
        KagemushaMintFinalityEpochDecisionV1::Activate => {
            *iroha_crypto::Hash::new(&domain).as_ref()
        }
        _ => panic!("fixture supports explicit retention or activation"),
    };
    let beacon = if decision == KagemushaMintFinalityEpochDecisionV1::Retain
        && previous.beacon != BeaconEpochBindingV1::Bootstrap
    {
        previous.beacon
    } else {
        domain.extend_from_slice(b"beacon-session");
        let session_id = *iroha_crypto::Hash::new(&domain).as_ref();
        domain.extend_from_slice(b"installed-transcript");
        BeaconEpochBindingV1::Installed(InstalledBeaconEpochBindingV1 {
            session_id,
            transcript_hash: *iroha_crypto::Hash::new(&domain).as_ref(),
        })
    };
    let next = KagemushaMintFinalityEpochAuthorizationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: authority.network_id,
        epoch: previous.epoch.checked_add(1).expect("fixture epoch fits"),
        first_height: previous
            .last_height
            .checked_add(1)
            .expect("fixture height fits"),
        last_height,
        authority_generation: authority.generation,
        authority_id,
        beacon,
        previous_authorization_id,
        transition_id,
        decision,
    };
    next.validate_against_authority(authority)
        .expect("fixture authority matches");
    next.validate_successor(previous)
        .expect("fixture links the exact predecessor");
    next
}

/// Build real network-independent signed-genesis authority parameters.
#[cfg(test)]
pub(crate) fn mint_finality_genesis_parameters(
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityGenesisParametersV1 {
    let validators = roster
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            let seed_byte =
                0xA0_u8.wrapping_add(u8::try_from(index).expect("small fixture roster"));
            crate::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                &[seed_byte; 32],
                0,
                validator.validator.clone(),
            )
            .expect("derive real genesis paired keys")
        })
        .collect();
    let authority_generation = KagemushaMintFinalityAuthorityGenerationTemplateV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        generation: 0,
        validators,
    };
    authority_generation
        .validate()
        .expect("valid fixture genesis authority template");
    KagemushaMintFinalityGenesisParametersV1 {
        authority_generation,
    }
}

/// Build a closed four-validator signed-genesis parameter fixture.
#[cfg(test)]
pub(crate) fn genesis_context_parameters() -> SumeragiV2GenesisContextParameters {
    SumeragiV2GenesisContextParameters::recommended()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::block::BlockHeader;
    use iroha_model_base::peer::PeerId;

    fn fixture() -> (NetworkId, Vec<ValidatorPower>) {
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"mint-finality-fixture-helper-controls"),
        ));
        let mut roster = (1_u8..=4)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS validator");
                ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|a, b| a.validator.cmp(&b.validator));
        (network, roster)
    }

    #[test]
    fn retained_epoch_chain_uses_actual_contiguous_predecessors_and_unchanged_keys() {
        let (network, roster) = fixture();
        let (genesis, authority) =
            mint_finality_authorization_and_authority(network, 0, 1, 10, &roster);
        let (first, first_authority) =
            mint_finality_authorization_and_authority(network, 1, 11, 11, &roster);
        let (second, second_authority) =
            mint_finality_authorization_and_authority(network, 2, 12, 50, &roster);
        assert_eq!(authority, first_authority);
        assert_eq!(authority, second_authority);
        assert_eq!(second.authority_generation, 0);
        assert_eq!(
            first.previous_authorization_id,
            genesis.authorization_id().unwrap()
        );
        assert_eq!(
            second.previous_authorization_id,
            first.authorization_id().unwrap()
        );
        first.validate_successor(&genesis).unwrap();
        second.validate_successor(&first).unwrap();
        assert_eq!(first.beacon, second.beacon);
    }

    #[test]
    fn explicit_boundary_retains_authority_or_activates_exact_next_generation() {
        let (network, roster) = fixture();
        let (genesis, authority) =
            mint_finality_authorization_and_authority(network, 0, 1, 10, &roster);
        let retained = mint_finality_successor_authorization(
            &genesis,
            &authority,
            20,
            KagemushaMintFinalityEpochDecisionV1::Retain,
        );
        let next_authority = mint_finality_authority(network, 1, &roster);
        assert_ne!(authority.validators, next_authority.validators);
        let activated = mint_finality_successor_authorization(
            &retained,
            &next_authority,
            30,
            KagemushaMintFinalityEpochDecisionV1::Activate,
        );
        assert_eq!(
            activated.authority_id,
            next_authority.authority_id().unwrap()
        );
        assert_eq!(activated.authority_generation, 1);
        assert_ne!(activated.transition_id, [0; 32]);
        let mut stale_parent = retained;
        stale_parent.last_height += 1;
        assert!(activated.validate_successor(&stale_parent).is_err());
        let mut false_retention = activated;
        false_retention.decision = KagemushaMintFinalityEpochDecisionV1::Retain;
        false_retention.transition_id = [0; 32];
        assert!(false_retention.validate_successor(&retained).is_err());
    }

    #[test]
    fn signed_genesis_template_binds_the_same_real_authority_keys() {
        let (network, roster) = fixture();
        let template = mint_finality_genesis_parameters(&roster);
        assert_eq!(
            template
                .authority_generation
                .bind_network_id(network)
                .unwrap(),
            mint_finality_authority(network, 0, &roster)
        );
    }

    #[test]
    #[should_panic(expected = "each earlier fixture epoch needs at least one positive height")]
    fn later_epoch_cannot_be_fabricated_at_height_one() {
        let (network, roster) = fixture();
        mint_finality_authorization_and_authority(network, 1, 1, 10, &roster);
    }

    #[test]
    #[should_panic(expected = "genesis authorization starts at height one")]
    fn genesis_cannot_skip_initial_history() {
        let (network, roster) = fixture();
        mint_finality_authorization_and_authority(network, 0, 2, 10, &roster);
    }
}
