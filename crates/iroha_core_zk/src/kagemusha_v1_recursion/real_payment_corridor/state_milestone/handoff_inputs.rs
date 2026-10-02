//! Request and one-use input ownership for the genuine alternating-device proof fixture.
//!
//! These deterministic inputs carry no proof, production hardware authority, or balance
//! admission. The caller must separately generate and install every original Core transition.

use super::*;

/// Build one request from the original receiver credential and current sender predecessor.
pub(super) fn preparation(
    sender: &KagemushaStateV1,
    journal_revision_before: u128,
    authorization_counter_before: u128,
    receiver_material: &MintRecipientMaterial,
    receiver_credit: &DiagnosticReceiverCreditV1,
    receiver_key: &SigningKey,
    handoff_index: u64,
    amount: u128,
) -> Result<(SendSplitPreparationV1, DiagnosticSenderOpeningsV1), String> {
    ensure(
        amount > 0 && amount <= sender.balance,
        "insufficient original sender balance",
    )?;
    ensure(
        device_public_key(receiver_key) == receiver_material.hardware_credential.device_public_key,
        "receiver request signing key differs from its original credential",
    )?;
    let issued_at_ms = handoff_index
        .checked_mul(10)
        .and_then(|delta| SEND_TIME.checked_add(delta))
        .ok_or_else(|| "handoff time overflow".to_owned())?;
    let committed_at_ms = issued_at_ms
        .checked_add(1)
        .ok_or("handoff commit time overflow")?;
    let expires_at_ms = issued_at_ms
        .checked_add(1_000)
        .ok_or("handoff expiry overflow")?;
    let openings = DiagnosticSenderOpeningsV1 {
        predecessor: sender.clone(),
        journal_revision_before,
        journal_revision_after: journal_revision_before
            .checked_add(1)
            .ok_or("journal overflow")?,
        authorization_counter_before,
        authorization_counter_after: authorization_counter_before
            .checked_add(1)
            .ok_or("authorization counter overflow")?,
        one_use_hardware_authorization: digest(b"handoff-original-one-use", handoff_index),
        commit_evidence_opening: KagemushaCommitEvidenceOpeningV1 {
            opening: digest(b"handoff-original-trusted-time", handoff_index),
            trusted_commit_time_ms: committed_at_ms,
            lease_id: [0; 32],
            lease_valid_from_ms: 0,
            lease_expires_at_ms: 0,
        },
    };
    let mut request = KagemushaPaymentRequestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: sender.release_id,
        network_id: sender.lane.network_id,
        asset: sender.lane.asset.clone(),
        asset_incarnation: sender.asset_incarnation,
        scale: sender.lane.scale,
        liability_pool_id: sender.liability_pool_id,
        recipient: receiver_material.recipient.clone(),
        amount,
        recipient_encryption_key: receiver_credit.public_key(),
        hardware_credential: receiver_material.hardware_credential,
        request_id: digest(b"handoff-original-request", handoff_index),
        issued_at_ms,
        expires_at_ms,
        signature: device_signature(receiver_key, b"unsealed handoff request"),
    };
    request.signature = device_signature(
        receiver_key,
        &request
            .canonical_signing_bytes()
            .map_err(|error| error.to_string())?,
    );
    request
        .validate_against_profile(&receiver_material.hardware_profile)
        .map_err(|error| error.to_string())?;
    let authorization = openings.prepared_authorization_digest();
    let prepared = SendSplitPreparationV1 {
        ciphertext_commitment: receiver_credit.commitment(&request),
        request,
        // Core's nonpersisting first preview derives the exact output/credit ID. The caller
        // replaces this shape-only carrier with receiver_credit.seal_for before any proof.
        encrypted_credit: receiver_material
            .authorization_relation
            .encrypted_credit
            .clone(),
        transition_nullifier: canonical_predecessor_conflict_nullifier_v1(authorization),
        successor_state_nonce_commitment: digest(
            b"handoff-original-successor-nonce",
            handoff_index,
        ),
        commit_evidence: openings.commit_evidence()?,
        commit_authorization_reference_ms: committed_at_ms,
        outbox_reservation: KagemushaOutboxReservationV1 {
            reservation_id: digest(b"handoff-original-outbox", handoff_index),
            operation_kind: KagemushaOperationKindV1::SendSplit,
            reserved_outbox_bytes: u32::try_from(KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES)
                .map_err(|_| "outbox reservation width overflow".to_owned())?,
            issued_at_ms,
            expires_at_ms,
        },
        prepared_one_use_authorization_digest: authorization,
        sealed_transition_inputs: digest(b"handoff-original-transition-inputs", handoff_index)
            .to_vec(),
        sealed_recovery_seeds: digest(b"handoff-original-recovery-seed", handoff_index).to_vec(),
    };
    openings.validate_preparation(&prepared)?;
    Ok((prepared, openings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn materials() -> [MintRecipientMaterial; 2] {
        [0, 1].map(|index| {
            core_bound_mint_recipient_material(
                index,
                digest(b"handoff-input-release", 0),
                digest(b"vk-set", 0),
                digest(b"handoff-input-manifest", 0),
                1_000,
            )
        })
    }

    #[test]
    fn inputs_retain_distinct_receiver_and_current_original_authorization() {
        let materials = materials();
        let sender = aggregate_state_with_balance(
            materials[0]
                .authorization_relation
                .statement
                .context
                .release_id,
            &materials[0].platform_credential.statement,
            digest(b"shape-only-funded-predecessor", 0),
            1_000,
            70,
        );
        let credit = DiagnosticReceiverCreditV1::for_device(1);
        let (prepared, openings) = preparation(
            &sender,
            75,
            4,
            &materials[1],
            &credit,
            &deterministic_signing_key(1),
            0,
            400,
        )
        .unwrap();
        assert_eq!(
            prepared.request.hardware_credential,
            materials[1].hardware_credential
        );
        assert_ne!(
            prepared.request.hardware_credential.lane_commitment,
            sender.lane.device_lane_id
        );
        assert_eq!(
            prepared.request.recipient_encryption_key,
            credit.public_key()
        );
        assert_eq!(openings.predecessor, sender);
        assert_eq!(
            (
                openings.journal_revision_before,
                openings.journal_revision_after
            ),
            (75, 76)
        );
        assert_eq!(
            (
                openings.authorization_counter_before,
                openings.authorization_counter_after
            ),
            (4, 5)
        );
        openings.validate_preparation(&prepared).unwrap();
        assert!(
            preparation(
                &sender,
                75,
                4,
                &materials[1],
                &credit,
                &deterministic_signing_key(0),
                0,
                400
            )
            .is_err()
        );
        for amount in [0, 1_001] {
            assert!(
                preparation(
                    &sender,
                    75,
                    4,
                    &materials[1],
                    &credit,
                    &deterministic_signing_key(1),
                    0,
                    amount
                )
                .is_err()
            );
        }
        for (revision, counter, index) in [(u128::MAX, 4, 0), (75, u128::MAX, 0), (75, 4, u64::MAX)]
        {
            assert!(
                preparation(
                    &sender,
                    revision,
                    counter,
                    &materials[1],
                    &credit,
                    &deterministic_signing_key(1),
                    index,
                    400
                )
                .is_err()
            );
        }
    }

    #[test]
    fn all_1024_input_identities_and_receiver_ciphertexts_are_request_bound() {
        let materials = materials();
        let sender = aggregate_state_with_balance(
            materials[0]
                .authorization_relation
                .statement
                .context
                .release_id,
            &materials[0].platform_credential.statement,
            digest(b"shape-only-funded-predecessor", 0),
            1_000,
            1,
        );
        let receiver = DiagnosticReceiverCreditV1::for_device(1);
        let other = DiagnosticReceiverCreditV1::for_device(0);
        assert_ne!(receiver.public_key(), other.public_key());
        let mut requests = BTreeSet::new();
        let mut authorizations = BTreeSet::new();
        let mut reservations = BTreeSet::new();
        let mut nonces = BTreeSet::new();
        let key = deterministic_signing_key(1);
        let mut encrypted = BTreeSet::new();
        for index in 0..1_024 {
            let (prepared, _) = preparation(
                &sender,
                1,
                u128::from(index),
                &materials[1],
                &receiver,
                &key,
                index,
                400,
            )
            .unwrap();
            assert!(requests.insert(prepared.request.canonical_digest().unwrap()));
            assert!(authorizations.insert(prepared.prepared_one_use_authorization_digest));
            assert!(reservations.insert(prepared.outbox_reservation.reservation_id));
            assert!(nonces.insert(prepared.successor_state_nonce_commitment));
            // Real AEAD vectors for the first two shape-only input samples only. These output
            // constructors carry no state proof or admitted spend authority.
            if index < 2 {
                let output = KagemushaPaymentOutputV1 {
                    version: KAGEMUSHA_WIRE_VERSION_V1,
                    request_digest: prepared.request.canonical_digest().unwrap(),
                    amount: prepared.request.amount,
                    sender_before_commitment: sender.state_commitment,
                    sender_after_commitment: digest(b"shape-only-successor", index),
                    transition_nullifier: prepared.transition_nullifier,
                    credit_id: [0; 32],
                    ciphertext_commitment: prepared.ciphertext_commitment,
                    commit_evidence: prepared.commit_evidence,
                    committed_at_ms: prepared.commit_authorization_reference_ms,
                }
                .seal_credit_id_against(&prepared.request)
                .unwrap();
                let sealed = receiver.seal_for(&output, &prepared.request);
                assert_eq!(sealed, receiver.seal_for(&output, &prepared.request));
                assert!(encrypted.insert(sealed));
            }
        }
        assert_eq!(requests.len(), 1_024);
        assert_eq!(encrypted.len(), 2);
    }
}
