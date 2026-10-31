//! Bound retained state while reserving every live departure and host end.
//!
//! Each terminal action can append one scalar nullifier, increase a sequence,
//! and set the end timestamp/status. Its maximum JSON growth is below 256 bytes.
//! Active membership carries one such reservation in addition to the host's.
//! Usage, metadata and relay updates must preserve the same reservations.

use super::{Error, KaigiPrivacyMode, KaigiRecord, KaigiStatus, privacy_error};
use crate::state::StateTransaction;
use iroha_data_model::{
    kaigi::KAIGI_RECORD_MAX_JSON_BYTES_V1, parameter::CustomParameterId, prelude::Json,
};

const ACTION_RESERVATION_BYTES: usize = 256;

fn required_bytes(record_bytes: usize, live: usize, active_private: bool) -> Result<usize, Error> {
    let reserved = if active_private {
        live.checked_add(1)
            .and_then(|actions| actions.checked_mul(ACTION_RESERVATION_BYTES))
            .ok_or_else(|| privacy_error("Kaigi terminal-action reservation overflow"))?
    } else {
        0
    };
    record_bytes
        .checked_add(reserved)
        .ok_or_else(|| privacy_error("Kaigi retained-state reservation overflow"))
}

pub(super) fn enforce(
    state: &StateTransaction<'_, '_>,
    record: &KaigiRecord,
    value: &Json,
) -> Result<(), Error> {
    let required = required_bytes(
        value.as_ref().len(),
        record.roster_commitments.len(),
        record.privacy_mode == KaigiPrivacyMode::ZkRosterV1 && record.status == KaigiStatus::Active,
    )?;
    let params = state.world.parameters.get();
    let configured = params
        .custom()
        .get(&CustomParameterId(
            "max_metadata_value_bytes"
                .parse()
                .expect("static parameter name"),
        ))
        .and_then(|parameter| parameter.payload().try_into_any_norito::<u64>().ok())
        .map_or(crate::smartcontracts::limits::DEFAULT_JSON_LIMIT, |value| {
            usize::try_from(value).unwrap_or(usize::MAX)
        });
    if required > KAIGI_RECORD_MAX_JSON_BYTES_V1.min(configured) {
        return Err(privacy_error(
            "Kaigi call record is full: it must keep room for every live leave and the host end",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{
        new_record, sample_commitment, sample_nullifier, with_state_transaction,
    };
    use super::*;
    use iroha_data_model::kaigi::scalar::KaigiAuthorizationScalarV1;

    #[test]
    fn reservations_are_checked_exactly_and_release_only_at_departure_or_end() {
        assert_eq!(required_bytes(100, 0, true).unwrap(), 356);
        assert_eq!(required_bytes(100, 1, true).unwrap(), 612);
        assert_eq!(required_bytes(100, usize::MAX, false).unwrap(), 100);
        assert!(required_bytes(usize::MAX - 255, 0, true).is_err());
        assert!(required_bytes(0, usize::MAX, true).is_err());
    }

    #[test]
    fn largest_terminal_action_json_growth_fits_its_reservation() {
        let (mut record, _, participant) = new_record(KaigiPrivacyMode::ZkRosterV1);
        let mut commitment = sample_commitment();
        commitment.commitment = KaigiAuthorizationScalarV1::default();
        record
            .private_participation
            .commit_join(&participant, 1, commitment.commitment)
            .unwrap();
        record.push_commitment(commitment);
        let before = Json::try_new(record.clone()).unwrap().as_ref().len();
        record
            .private_participation
            .commit_leave(&participant, 1, commitment.commitment)
            .unwrap();
        assert!(record.remove_commitment(&commitment));
        let mut nullifier = sample_nullifier(1);
        // Largest decimal JSON byte representation in every unrestricted limb.
        let mut bytes = [255; 32];
        bytes[31] = 63;
        nullifier.digest = KaigiAuthorizationScalarV1::from_le_bytes(bytes).unwrap();
        record.push_nullifier(nullifier);
        let after_leave = Json::try_new(record.clone()).unwrap().as_ref().len();
        assert!(after_leave <= before + ACTION_RESERVATION_BYTES);
        record.status = KaigiStatus::Ended;
        record.ended_at_ms = Some(u64::MAX);
        record.push_nullifier(nullifier);
        let after_end = Json::try_new(record).unwrap().as_ref().len();
        assert!(after_end <= after_leave + ACTION_RESERVATION_BYTES);
    }

    /// The roster a private call really reaches, once each participant's commitment,
    /// ledger entry, join nullifier and departure reservation count toward the 1-MiB
    /// ceiling. It sits well below the 4,096 count limit and falls with longer account
    /// keys, so `specs/kaigi_privacy_design.md` quotes these figures.
    #[test]
    fn json_ceiling_sets_the_reachable_private_roster() {
        fn reachable(algorithm: iroha_crypto::Algorithm) -> usize {
            let (mut record, _, _) = new_record(KaigiPrivacyMode::ZkRosterV1);
            record.push_nullifier(sample_nullifier(0));
            for joined in 1_u64.. {
                let key_pair = iroha_crypto::KeyPair::try_random_with_algorithm(algorithm).unwrap();
                let account =
                    iroha_data_model::prelude::AccountId::new(key_pair.public_key().clone());
                let mut bytes = [0xFF; 32];
                bytes[..8].copy_from_slice(&joined.to_le_bytes());
                bytes[31] = 63;
                let commitment = KaigiAuthorizationScalarV1::from_le_bytes(bytes).unwrap();
                record
                    .private_participation
                    .commit_join(&account, 1, commitment)
                    .unwrap();
                let mut entry = sample_commitment();
                entry.commitment = commitment;
                record.push_commitment(entry);
                let mut nullifier = sample_nullifier(0);
                nullifier.digest = commitment;
                record.push_nullifier(nullifier);
                // Encoding the whole record is the slow part, so sample the linear growth.
                if joined % 16 == 0 {
                    let bytes = Json::try_new(record.clone()).unwrap().as_ref().len();
                    let live = record.roster_commitments.len();
                    if required_bytes(bytes, live, true).unwrap() > KAIGI_RECORD_MAX_JSON_BYTES_V1 {
                        return usize::try_from(joined).unwrap();
                    }
                }
            }
            unreachable!("the ceiling stops the loop")
        }
        let ed25519 = reachable(iroha_crypto::Algorithm::Ed25519);
        let ml_dsa = reachable(iroha_crypto::Algorithm::MlDsa);
        println!("first roster over the ceiling: Ed25519 {ed25519}, ML-DSA {ml_dsa}");
        assert!((1_344..=1_376).contains(&ed25519), "{ed25519}");
        assert!((192..=224).contains(&ml_dsa), "{ml_dsa}");
    }

    #[test]
    fn stored_value_must_leave_room_under_the_protocol_limit() {
        with_state_transaction(|state| {
            let (record, _, _) = new_record(KaigiPrivacyMode::ZkRosterV1);
            let at_limit = Json::try_new("x".repeat(KAIGI_RECORD_MAX_JSON_BYTES_V1 - 2)).unwrap();
            assert!(enforce(state, &record, &at_limit).is_err());
            let at_reserved_limit = Json::try_new(
                "x".repeat(KAIGI_RECORD_MAX_JSON_BYTES_V1 - ACTION_RESERVATION_BYTES - 2),
            )
            .unwrap();
            assert!(enforce(state, &record, &at_reserved_limit).is_ok());
        });
    }
}
