//! Deterministic, non-shipping Offline Cash V1 consensus fixtures.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader,
        consensus_v2::{SumeragiV2GenesisContextParameters, ValidatorPower},
    },
    isi::offline_cash_v1::{OFFLINE_CASH_CHAIN_VERSION_V1, OfflineCashMintFinalityEpochRosterV1},
    peer::PeerId,
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
                network_id,
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
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"Offline Cash V1 Core test genesis"),
    ));
    let mut roster = (1_u8..=4)
        .map(|seed| {
            let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive deterministic consensus test key");
            ValidatorPower {
                validator: PeerId::new(key_pair.public_key().clone()),
                power: 1,
            }
        })
        .collect::<Vec<_>>();
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    SumeragiV2GenesisContextParameters::recommended(
        mint_finality_roster(network_id, 0, &roster),
        None,
    )
}
