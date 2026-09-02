//! Deterministic, non-shipping Offline Cash V1 consensus fixtures.

use iroha_data_model::{
    NetworkId,
    block::consensus_v2::{SumeragiV2GenesisContextParameters, ValidatorPower},
    isi::offline_cash_v1::{
        OFFLINE_CASH_CHAIN_VERSION_V1, OfflineCashMintFinalityEpochRosterTemplateV1,
        OfflineCashMintFinalityEpochRosterV1, OfflineCashMintFinalityGenesisParametersV1,
    },
};

/// Build real, canonically encoded paired-Pasta public keys aligned with `roster`.
pub(crate) fn mint_finality_roster(
    network_id: NetworkId,
    epoch: u64,
    roster: &[ValidatorPower],
) -> OfflineCashMintFinalityEpochRosterV1 {
    let validators = roster
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            let seed_byte = 0xA0_u8.wrapping_add(
                u8::try_from(index).expect("test validator index fits in one byte"),
            );
            crate::zk::offline_cash_v1_recursion::derive_offline_cash_mint_finality_validator_keys_v1(
                &[seed_byte; 32],
                epoch,
                validator.validator.clone(),
            )
            .expect("derive deterministic paired-Pasta test validator keys")
        })
        .collect();
    let fixture = OfflineCashMintFinalityEpochRosterV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        network_id,
        epoch,
        validators,
    };
    fixture.validate().expect("valid test mint-finality roster");
    fixture
}

/// Build a real networkless signed-genesis template aligned with `roster`.
pub(crate) fn mint_finality_template(
    epoch: u64,
    roster: &[ValidatorPower],
) -> OfflineCashMintFinalityEpochRosterTemplateV1 {
    let validators = roster
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            let seed_byte = 0xA0_u8.wrapping_add(
                u8::try_from(index).expect("test validator index fits in one byte"),
            );
            crate::zk::offline_cash_v1_recursion::derive_offline_cash_mint_finality_validator_keys_v1(
                &[seed_byte; 32],
                epoch,
                validator.validator.clone(),
            )
            .expect("derive deterministic paired-Pasta test validator keys")
        })
        .collect();
    let template = OfflineCashMintFinalityEpochRosterTemplateV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        epoch,
        validators,
    };
    template
        .validate()
        .expect("valid test mint-finality roster template");
    template
}

/// Build mandatory signed Offline Cash genesis parameters for a closed roster.
pub(crate) fn mint_finality_genesis_parameters(
    roster: &[ValidatorPower],
) -> OfflineCashMintFinalityGenesisParametersV1 {
    OfflineCashMintFinalityGenesisParametersV1 {
        epoch_roster: mint_finality_template(0, roster),
        next_epoch_roster: None,
    }
}

/// Build the roster and its self-authenticating canonical identifier.
pub(crate) fn mint_finality_roster_and_id(
    network_id: NetworkId,
    epoch: u64,
    roster: &[ValidatorPower],
) -> ([u8; 32], OfflineCashMintFinalityEpochRosterV1) {
    let roster = mint_finality_roster(network_id, epoch, roster);
    let id = roster
        .finality_epoch_id()
        .expect("derive deterministic mint-finality test roster ID");
    (id, roster)
}

/// Build a closed four-validator signed-genesis parameter fixture.
pub(crate) fn genesis_context_parameters() -> SumeragiV2GenesisContextParameters {
    SumeragiV2GenesisContextParameters::recommended()
}
