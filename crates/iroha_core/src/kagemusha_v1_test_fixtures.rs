//! Deterministic, non-shipping Kagemusha V1 consensus fixtures.

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::ValidatorPower,
    isi::kagemusha_v1::{
        BeaconEpochBindingV1, KAGEMUSHA_CHAIN_VERSION_V1,
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

/// Build actual paired-Pasta keys for one immutable generation, independent of election epoch.
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
    let fixture = KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        generation,
        validators,
    };
    fixture
        .validate()
        .expect("valid test mint-finality authority");
    fixture
}

/// Bind generation zero to the explicit signed-genesis scheduling interval.
pub(crate) fn mint_finality_genesis_for_authority(
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    last_height: u64,
) -> KagemushaMintFinalityEpochAuthorizationV1 {
    KagemushaMintFinalityEpochAuthorizationV1::genesis(authority, last_height)
        .expect("generation-zero genesis authorization")
}

/// Build a complete generation-zero authority and explicit genesis authorization.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub(crate) fn mint_finality_genesis_authorization(
    network_id: NetworkId,
    last_height: u64,
    roster: &[ValidatorPower],
) -> (
    KagemushaMintFinalityEpochAuthorizationV1,
    KagemushaMintFinalityAuthorityGenerationV1,
) {
    let authority = mint_finality_authority(network_id, 0, roster);
    let authorization = mint_finality_genesis_for_authority(&authority, last_height);
    (authorization, authority)
}

/// Build one explicit contiguous successor and check its complete predecessor/authority binding.
#[cfg(test)]
pub(crate) fn mint_finality_successor_authorization(
    previous: &KagemushaMintFinalityEpochAuthorizationV1,
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    last_height: u64,
    beacon: BeaconEpochBindingV1,
    decision: KagemushaMintFinalityEpochDecisionV1,
    transition_id: [u8; 32],
) -> KagemushaMintFinalityEpochAuthorizationV1 {
    let authorization = KagemushaMintFinalityEpochAuthorizationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: authority.network_id,
        epoch: previous
            .epoch
            .checked_add(1)
            .expect("fixture epoch successor"),
        first_height: previous
            .last_height
            .checked_add(1)
            .expect("fixture height successor"),
        last_height,
        authority_generation: authority.generation,
        authority_id: authority
            .authority_id()
            .expect("fixture authority identity"),
        beacon,
        previous_authorization_id: previous
            .authorization_id()
            .expect("fixture previous authorization identity"),
        transition_id,
        decision,
    };
    authorization
        .validate_against_authority(authority)
        .expect("successor authority binding");
    authorization
        .validate_successor(previous)
        .expect("contiguous fixture epoch authorization");
    authorization
}

/// Build a real networkless signed-genesis template aligned with `roster`.
#[cfg(test)]
pub(crate) fn mint_finality_authority_template(
    generation: u64,
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityAuthorityGenerationTemplateV1 {
    let network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"non-shipping mint finality template key derivation"),
    ));
    let authority = mint_finality_authority(network_id, generation, roster);
    let template = KagemushaMintFinalityAuthorityGenerationTemplateV1 {
        version: authority.version,
        generation,
        validators: authority.validators,
    };
    template
        .validate()
        .expect("valid test mint-finality authority template");
    template
}

/// Build mandatory signed Kagemusha genesis parameters for a closed roster.
#[cfg(test)]
pub(crate) fn mint_finality_genesis_parameters(
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityGenesisParametersV1 {
    let parameters = KagemushaMintFinalityGenesisParametersV1 {
        authority_generation: mint_finality_authority_template(0, roster),
    };
    parameters
        .validate()
        .expect("valid generation-zero genesis parameters");
    parameters
}

/// Build a closed four-validator signed-genesis parameter fixture.
#[cfg(test)]
pub(crate) fn genesis_context_parameters() -> SumeragiV2GenesisContextParameters {
    SumeragiV2GenesisContextParameters::recommended()
}

/// Build a scheduling-epoch fixture while retaining generation-zero keys throughout.
///
/// Earlier epochs each span one height; the requested final epoch spans through
/// `last_height`. Each predecessor link is constructed and validated explicitly.
#[cfg(test)]
pub(crate) fn mint_finality_retained_authorization(
    network_id: NetworkId,
    epoch: u64,
    last_height: u64,
    roster: &[ValidatorPower],
) -> (
    KagemushaMintFinalityEpochAuthorizationV1,
    KagemushaMintFinalityAuthorityGenerationV1,
) {
    assert!(epoch < 1_024, "fixture epoch history is bounded");
    let authority = mint_finality_authority(network_id, 0, roster);
    let mut authorization =
        mint_finality_genesis_for_authority(&authority, if epoch == 0 { last_height } else { 1 });
    for next_epoch in 1..=epoch {
        authorization = mint_finality_successor_authorization(
            &authorization,
            &authority,
            if next_epoch == epoch {
                last_height
            } else {
                next_epoch + 1
            },
            fixture_installed_beacon(),
            KagemushaMintFinalityEpochDecisionV1::Retain,
            [0; 32],
        );
    }
    (authorization, authority)
}

/// Build an exact fixed-length scheduling history, retaining generation zero.
#[cfg(test)]
pub(crate) fn mint_finality_scheduled_authorization(
    network_id: NetworkId,
    epoch: u64,
    epoch_length: u64,
    roster: &[ValidatorPower],
) -> (
    KagemushaMintFinalityEpochAuthorizationV1,
    KagemushaMintFinalityAuthorityGenerationV1,
) {
    assert!(epoch < 1_024, "fixture epoch history is bounded");
    assert!(epoch_length != 0, "fixture epochs have at least one height");
    let authority = mint_finality_authority(network_id, 0, roster);
    let mut authorization = mint_finality_genesis_for_authority(&authority, epoch_length);
    for next_epoch in 1..=epoch {
        let last_height = next_epoch
            .checked_add(1)
            .and_then(|count| count.checked_mul(epoch_length))
            .expect("fixture schedule remains representable");
        authorization = mint_finality_successor_authorization(
            &authorization,
            &authority,
            last_height,
            fixture_installed_beacon(),
            KagemushaMintFinalityEpochDecisionV1::Retain,
            [0; 32],
        );
    }
    (authorization, authority)
}

/// Return the explicit installed-beacon binding used by non-shipping authorization histories.
#[cfg(test)]
pub(crate) fn fixture_installed_beacon() -> BeaconEpochBindingV1 {
    BeaconEpochBindingV1::Installed(
        iroha_data_model::isi::kagemusha_v1::InstalledBeaconEpochBindingV1 {
            session_id: [0x71; 32],
            transcript_hash: [0x72; 32],
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_epoch_history_keeps_generation_zero_and_links_each_successor() {
        let network_id =
            NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::new(b"fixture retained authorization history"),
            ));
        // Use the same closed deterministic roster as production-oriented fixtures.
        let mut roster = (1_u8..=4)
            .map(|seed| {
                let key = iroha_crypto::KeyPair::try_from_seed(
                    vec![seed; 32],
                    iroha_crypto::Algorithm::BlsNormal,
                )
                .expect("fixture validator");
                ValidatorPower {
                    validator: iroha_model_base::peer::PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        let (epoch, authority) = mint_finality_retained_authorization(network_id, 3, 40, &roster);
        assert_eq!(epoch.epoch, 3);
        assert_eq!(epoch.first_height, 4);
        assert_eq!(epoch.last_height, 40);
        assert_eq!(authority.generation, 0);
        assert_eq!(epoch.authority_generation, 0);
        assert_eq!(epoch.beacon, fixture_installed_beacon());
        let (previous, _) = mint_finality_retained_authorization(network_id, 2, 3, &roster);
        epoch
            .validate_successor(&previous)
            .expect("exact previous authorization");
        let (scheduled_previous, _) =
            mint_finality_scheduled_authorization(network_id, 2, 10, &roster);
        let (scheduled, scheduled_authority) =
            mint_finality_scheduled_authorization(network_id, 3, 10, &roster);
        assert_eq!(scheduled.first_height, 31);
        assert_eq!(scheduled.last_height, 40);
        assert_eq!(scheduled_authority, authority);
        scheduled
            .validate_successor(&scheduled_previous)
            .expect("exact fixed-length predecessor");
        assert_ne!(
            scheduled.previous_authorization_id,
            epoch.previous_authorization_id
        );
    }
}
