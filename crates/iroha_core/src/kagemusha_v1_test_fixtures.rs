//! Deterministic, non-shipping Kagemusha V1 consensus fixtures.

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::ValidatorPower,
    isi::kagemusha_v1::{KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1},
};
#[cfg(test)]
use iroha_data_model::{
    block::consensus_v2::SumeragiV2GenesisContextParameters,
    isi::kagemusha_v1::{
        KagemushaMintFinalityAuthorityGenerationTemplateV1,
        KagemushaMintFinalityGenesisParametersV1,
    },
};

/// Build real, canonically encoded paired-Pasta public keys aligned with `roster`.
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
    fixture.validate().expect("valid test mint-finality roster");
    fixture
}

/// Build a real networkless signed-genesis template aligned with `roster`.
#[cfg(test)]
pub(crate) fn mint_finality_template(
    generation: u64,
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityAuthorityGenerationTemplateV1 {
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
    let template = KagemushaMintFinalityAuthorityGenerationTemplateV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        generation,
        validators,
    };
    template
        .validate()
        .expect("valid test mint-finality roster template");
    template
}

/// Build mandatory signed Kagemusha genesis parameters for a closed roster.
#[cfg(test)]
pub(crate) fn mint_finality_genesis_parameters(
    roster: &[ValidatorPower],
) -> KagemushaMintFinalityGenesisParametersV1 {
    KagemushaMintFinalityGenesisParametersV1 {
        authority_generation: mint_finality_template(0, roster),
    }
}

/// Build a standalone authenticated-context fixture with an explicit scheduling interval.
///
/// Non-genesis fixtures pin a synthetic predecessor and installed beacon. Tests which exercise
/// an actual boundary must instead use [`mint_finality_successor_authorization`].
pub(crate) fn mint_finality_authorization(
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    epoch: u64,
    first_height: u64,
    last_height: u64,
) -> iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1 {
    use iroha_data_model::isi::kagemusha_v1::{
        BeaconEpochBindingV1, KagemushaMintFinalityEpochAuthorizationV1,
        KagemushaMintFinalityEpochDecisionV1,
    };
    let genesis = epoch == 0;
    let authorization = KagemushaMintFinalityEpochAuthorizationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id: authority.network_id,
        epoch,
        first_height,
        last_height,
        authority_generation: authority.generation,
        authority_id: authority
            .authority_id()
            .expect("fixture authority identity"),
        beacon: if genesis {
            BeaconEpochBindingV1::Bootstrap
        } else {
            BeaconEpochBindingV1::Installed(
                iroha_data_model::isi::kagemusha_v1::InstalledBeaconEpochBindingV1 {
                    session_id: [0x71; 32],
                    transcript_hash: [0x72; 32],
                },
            )
        },
        previous_authorization_id: if genesis { [0; 32] } else { [0x73; 32] },
        transition_id: if genesis { [0; 32] } else { [0x74; 32] },
        decision: if genesis {
            KagemushaMintFinalityEpochDecisionV1::Genesis
        } else {
            KagemushaMintFinalityEpochDecisionV1::Activate
        },
    };
    authorization
        .validate_against_authority(authority)
        .expect("valid standalone scheduling fixture");
    authorization
}

/// Build complete fixture context fields with explicit key lifetime and scheduling interval.
pub(crate) fn mint_finality_context_fields(
    network_id: NetworkId,
    generation: u64,
    epoch: u64,
    first_height: u64,
    last_height: u64,
    roster: &[ValidatorPower],
) -> (
    iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    KagemushaMintFinalityAuthorityGenerationV1,
) {
    let authority = mint_finality_authority(network_id, generation, roster);
    let authorization = mint_finality_authorization(&authority, epoch, first_height, last_height);
    (authorization, authority)
}

/// Build an exact successor suitable for certified-retention and activation boundary fixtures.
pub(crate) fn mint_finality_successor_authorization(
    previous: &iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    authority: &KagemushaMintFinalityAuthorityGenerationV1,
    last_height: u64,
    decision: iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1,
) -> iroha_data_model::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1 {
    use iroha_data_model::isi::kagemusha_v1::{
        BeaconEpochBindingV1, KagemushaMintFinalityEpochDecisionV1,
    };
    let mut successor = mint_finality_authorization(
        authority,
        previous.epoch.checked_add(1).unwrap(),
        previous.last_height.checked_add(1).unwrap(),
        last_height,
    );
    successor.previous_authorization_id = previous.authorization_id().unwrap();
    successor.decision = decision;
    if decision == KagemushaMintFinalityEpochDecisionV1::Retain {
        successor.transition_id = [0; 32];
    }
    if matches!(
        decision,
        KagemushaMintFinalityEpochDecisionV1::Retain
            | KagemushaMintFinalityEpochDecisionV1::RetainAndCancel
    ) && previous.beacon != BeaconEpochBindingV1::Bootstrap
    {
        successor.beacon = previous.beacon;
    }
    successor
        .validate_successor(previous)
        .expect("exact scheduling successor fixture");
    successor
}

/// Build a closed four-validator signed-genesis parameter fixture.
#[cfg(test)]
pub(crate) fn genesis_context_parameters() -> SumeragiV2GenesisContextParameters {
    SumeragiV2GenesisContextParameters::recommended()
}
