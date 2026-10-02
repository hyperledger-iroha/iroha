//! Genuine local peer-credit opening, exact stage signatures, and retained acknowledgements.
//!
//! These P-256 signatures identify an explicit diagnostic provider. They do not establish
//! physical hardware custody or qualify a production release.

use super::*;
use crate::kagemusha_v1_state::{
    CreditStageCertificateV1, PaymentStageAuthorizationV1, StagePaymentOutcomeV1,
};
use iroha_data_model::kagemusha::{
    KagemushaAcknowledgementV1, KagemushaInboxReceiptV1,
    kagemusha_acknowledgement_signing_bytes_v1, kagemusha_inbox_receipt_commitment_v1,
};

fn sign_stage_statement(key: &SigningKey, statement: &CreditStageStatementV1) -> Vec<u8> {
    sign_journal(key, CREDIT_STAGE_DOMAIN, statement)
}

pub(super) fn verify_stage_signature(
    key: &KagemushaDevicePublicKeyV1,
    statement: &CreditStageStatementV1,
    bytes: &[u8],
) -> Result<(), String> {
    verify_journal(key, CREDIT_STAGE_DOMAIN, statement, bytes)
}

fn open_payment_credit(
    receiver: &DiagnosticReceiverCreditV1,
    request: &KagemushaPaymentRequestV1,
    output: &KagemushaPaymentOutputV1,
    encrypted_credit: &[u8],
) -> Result<KagemushaCreditOpeningV1, KagemushaStateErrorV1> {
    if request.recipient_encryption_key != receiver.public_key() {
        return Err(KagemushaStateErrorV1::InvalidPeerCredit);
    }
    let envelope =
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            encrypted_credit,
            request.recipient_encryption_key,
        )
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    let aad = KagemushaEncryptedCreditAadV1::for_peer(output, request)
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    let opening = open_kagemusha_credit_v1(
        &envelope,
        &aad,
        request.recipient_encryption_key,
        &receiver.private_key,
    )
    .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    if opening != receiver.opening(output.credit_id, request.amount) {
        return Err(KagemushaStateErrorV1::InvalidPeerCredit);
    }
    Ok(opening)
}

fn stage_receipt(
    statement: &CreditStageStatementV1,
    payment_digest: [u8; 32],
) -> Result<KagemushaInboxReceiptV1, KagemushaStateErrorV1> {
    let receipt_commitment = kagemusha_inbox_receipt_commitment_v1(
        statement.recipient_lane.device_lane_id,
        statement.receiver_hardware_epoch.epoch_id,
        statement.journal_revision_after,
        statement.credit_id.0,
        payment_digest,
    )
    .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
    Ok(KagemushaInboxReceiptV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: statement.credit_id.0,
        receipt_commitment,
    })
}

/// Open and stage the actual payment through Core's exact preview and original journal owner.
///
/// Exact transport retries return the original acknowledgement from Core without signing a
/// replacement. New credits use separate explicit provider-journal and receiver-device keys.
pub(super) fn stage_peer_payment(
    receiver_machine: &mut DiagnosticMachine<'_>,
    receiver: &DiagnosticReceiverCreditV1,
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    staged_at_ms: u64,
    journal_signing_key: &SigningKey,
    receiver_device_signing_key: &SigningKey,
) -> Result<StagePaymentOutcomeV1, KagemushaStateErrorV1> {
    let opening = open_payment_credit(
        receiver,
        request,
        &payment.output,
        &payment.encrypted_credit,
    )?;
    match receiver_machine.stage_payment(
        request.clone(),
        payment.clone(),
        opening.clone(),
        staged_at_ms,
        None,
    ) {
        Err(KagemushaStateErrorV1::MissingStageAuthorization) => {}
        result => return result,
    }
    if device_public_key(receiver_device_signing_key)
        != request.hardware_credential.device_public_key
    {
        return Err(KagemushaStateErrorV1::InvalidAcknowledgement);
    }
    let statement =
        receiver_machine.preview_stage_payment(request, payment, &opening, staged_at_ms)?;
    let request_digest = request
        .canonical_digest()
        .map_err(|_| KagemushaStateErrorV1::InvalidPaymentRequest)?;
    let payment_digest = payment
        .canonical_digest_against(request)
        .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
    let inbox_receipt = stage_receipt(&statement, payment_digest)?;
    let message = kagemusha_acknowledgement_signing_bytes_v1(
        KAGEMUSHA_WIRE_VERSION_V1,
        request_digest,
        payment_digest,
        inbox_receipt,
    )
    .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
    let acknowledgement = KagemushaAcknowledgementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        request_digest,
        payment_digest,
        inbox_receipt,
        signature: device_signature(receiver_device_signing_key, &message),
    };
    acknowledgement
        .validate_shape_against(request, payment)
        .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
    let guard_bundle = sign_stage_statement(journal_signing_key, &statement);
    receiver_machine.stage_payment(
        request.clone(),
        payment.clone(),
        opening,
        staged_at_ms,
        Some(PaymentStageAuthorizationV1 {
            stage_certificate: CreditStageCertificateV1 {
                statement,
                guard_bundle,
            },
            acknowledgement,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_state::{
        DevicePolicyBindingV1, HardwareEpochV1, KAGEMUSHA_STATE_VERSION_V1, KagemushaLaneIdV1,
    };
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{NetworkId, asset::AssetDefinitionId, block::BlockHeader};
    use iroha_model_base::domain::DomainId;

    fn network(tag: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(&[
            tag,
        ])))
    }

    fn asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("fixture domain"),
            name.parse().expect("fixture asset name"),
        )
    }

    // Structural signature fixture only: it is never submitted as authenticated machine state.
    fn statement() -> CreditStageStatementV1 {
        CreditStageStatementV1 {
            version: KAGEMUSHA_STATE_VERSION_V1,
            recipient_lane: KagemushaLaneIdV1 {
                network_id: network(1),
                device_lane_id: [2; 32],
                asset: asset("rose"),
                scale: 0,
            },
            receiver_state_commitment: [3; 32],
            receiver_hardware_epoch: HardwareEpochV1 {
                generation: 1,
                epoch_id: [4; 32],
            },
            receiver_device_policy_binding: DevicePolicyBindingV1 {
                device_key_reference: [5; 32],
                hardware_policy_id: [6; 32],
            },
            receiver_state_nonce_commitment: [7; 32],
            credit_id: CreditIdV1([8; 32]),
            envelope_digest: [9; 32],
            staged_at_ms: 300,
            journal_revision_before: 4,
            journal_revision_after: 5,
        }
    }

    fn signing_key(byte: u8) -> SigningKey {
        SigningKey::from_bytes((&[byte; 32]).into()).expect("fixture P-256 key")
    }

    #[test]
    fn peer_stage_signature_binds_every_statement_field() {
        let key = signing_key(11);
        let public_key = device_public_key(&key);
        let original = statement();
        let signature = sign_stage_statement(&key, &original);
        assert_eq!(signature, sign_stage_statement(&key, &original));
        assert_eq!(
            verify_stage_signature(&public_key, &original, &signature),
            Ok(())
        );
        type Mutation = Box<dyn Fn(&mut CreditStageStatementV1)>;
        let mutations: Vec<(&str, Mutation)> = vec![
            ("version", Box::new(|s| s.version += 1)),
            (
                "network",
                Box::new(|s| s.recipient_lane.network_id = network(10)),
            ),
            (
                "lane",
                Box::new(|s| s.recipient_lane.device_lane_id[0] ^= 1),
            ),
            (
                "asset",
                Box::new(|s| s.recipient_lane.asset = asset("lily")),
            ),
            ("scale", Box::new(|s| s.recipient_lane.scale += 1)),
            ("state", Box::new(|s| s.receiver_state_commitment[0] ^= 1)),
            (
                "epoch generation",
                Box::new(|s| s.receiver_hardware_epoch.generation += 1),
            ),
            (
                "epoch identity",
                Box::new(|s| s.receiver_hardware_epoch.epoch_id[0] ^= 1),
            ),
            (
                "device key",
                Box::new(|s| s.receiver_device_policy_binding.device_key_reference[0] ^= 1),
            ),
            (
                "device policy",
                Box::new(|s| s.receiver_device_policy_binding.hardware_policy_id[0] ^= 1),
            ),
            (
                "private nonce",
                Box::new(|s| s.receiver_state_nonce_commitment[0] ^= 1),
            ),
            ("credit", Box::new(|s| s.credit_id.0[0] ^= 1)),
            ("envelope", Box::new(|s| s.envelope_digest[0] ^= 1)),
            ("time", Box::new(|s| s.staged_at_ms += 1)),
            (
                "consumed revision",
                Box::new(|s| s.journal_revision_before += 1),
            ),
            (
                "produced revision",
                Box::new(|s| s.journal_revision_after += 1),
            ),
        ];
        for (label, mutate) in mutations {
            let mut changed = original.clone();
            mutate(&mut changed);
            assert_ne!(
                changed, original,
                "{label} mutation must change the statement"
            );
            assert!(
                verify_stage_signature(&public_key, &changed, &signature).is_err(),
                "{label}"
            );
        }
    }

    #[test]
    fn peer_stage_signature_rejects_foreign_domain_key_and_signature_bytes() {
        let key = signing_key(11);
        let original = statement();
        let public_key = device_public_key(&key);
        let signature = sign_stage_statement(&key, &original);
        assert!(
            verify_stage_signature(&device_public_key(&signing_key(12)), &original, &signature)
                .is_err()
        );
        let other_domain = sign_journal(&key, STAGE_DOMAIN, &original);
        assert_ne!(signature, other_domain);
        assert!(verify_stage_signature(&public_key, &original, &other_domain).is_err());
        let mut altered = signature.clone();
        altered[0] ^= 1;
        for bytes in [
            Vec::new(),
            vec![0; signature.len()],
            signature[..63].to_vec(),
            [signature.as_slice(), &[0]].concat(),
            altered,
        ] {
            assert_ne!(bytes, signature);
            assert!(verify_stage_signature(&public_key, &original, &bytes).is_err());
        }
    }

    #[test]
    fn peer_stage_receipt_uses_exact_canonical_lane_epoch_revision_credit_and_payment() {
        let original = statement();
        let payment_digest = [13; 32];
        let receipt = stage_receipt(&original, payment_digest).unwrap();
        assert_eq!(receipt.version, KAGEMUSHA_WIRE_VERSION_V1);
        assert_eq!(receipt.credit_id, original.credit_id.0);
        assert_eq!(
            receipt.receipt_commitment,
            kagemusha_inbox_receipt_commitment_v1(
                original.recipient_lane.device_lane_id,
                original.receiver_hardware_epoch.epoch_id,
                original.journal_revision_after,
                original.credit_id.0,
                payment_digest
            )
            .unwrap()
        );
        for (lane, epoch, revision, credit, payment) in [
            (
                [14; 32],
                original.receiver_hardware_epoch.epoch_id,
                original.journal_revision_after,
                original.credit_id.0,
                payment_digest,
            ),
            (
                original.recipient_lane.device_lane_id,
                [14; 32],
                original.journal_revision_after,
                original.credit_id.0,
                payment_digest,
            ),
            (
                original.recipient_lane.device_lane_id,
                original.receiver_hardware_epoch.epoch_id,
                original.journal_revision_after + 1,
                original.credit_id.0,
                payment_digest,
            ),
            (
                original.recipient_lane.device_lane_id,
                original.receiver_hardware_epoch.epoch_id,
                original.journal_revision_after,
                [14; 32],
                payment_digest,
            ),
            (
                original.recipient_lane.device_lane_id,
                original.receiver_hardware_epoch.epoch_id,
                original.journal_revision_after,
                original.credit_id.0,
                [14; 32],
            ),
        ] {
            assert_ne!(
                receipt.receipt_commitment,
                kagemusha_inbox_receipt_commitment_v1(lane, epoch, revision, credit, payment)
                    .unwrap()
            );
        }
        let mut zero_revision = original;
        zero_revision.journal_revision_after = 0;
        assert_eq!(
            stage_receipt(&zero_revision, payment_digest),
            Err(KagemushaStateErrorV1::InvalidAcknowledgement)
        );
    }

    #[test]
    fn peer_stage_opening_requires_exact_receiver_request_output_and_ciphertext() {
        let material = core_bound_mint_recipient_material(
            0,
            digest(b"peer-stage-opening-release", 0),
            digest(b"vk-set", 0),
            digest(b"peer-stage-opening-manifest", 0),
            1_000,
        );
        let preview = bootstrap_preview(&material, provisional_artifacts(&material));
        let (send, _) = send_preparation(&material, &preview.state, 0);
        let receiver = DiagnosticReceiverCreditV1::new();
        let output = KagemushaPaymentOutputV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            request_digest: send.request.canonical_digest().unwrap(),
            amount: send.request.amount,
            sender_before_commitment: digest(b"peer-stage-opening-before", 0),
            sender_after_commitment: digest(b"peer-stage-opening-after", 0),
            transition_nullifier: send.transition_nullifier,
            credit_id: [0; 32],
            ciphertext_commitment: send.ciphertext_commitment,
            commit_evidence: send.commit_evidence,
            committed_at_ms: SEND_TIME,
        }
        .seal_credit_id_against(&send.request)
        .unwrap();
        let encrypted_credit = receiver.seal_for(&output, &send.request);
        assert_eq!(
            open_payment_credit(&receiver, &send.request, &output, &encrypted_credit),
            Ok(receiver.opening(output.credit_id, send.request.amount))
        );

        let mut wrong_key = DiagnosticReceiverCreditV1::new();
        wrong_key.private_key = Zeroizing::new([0x77; 32]);
        assert_eq!(
            open_payment_credit(&wrong_key, &send.request, &output, &encrypted_credit),
            Err(KagemushaStateErrorV1::InvalidPeerCredit)
        );
        let mut wrong_opening = DiagnosticReceiverCreditV1::new();
        wrong_opening.credit_commitment_opening[0] ^= 1;
        assert_eq!(
            open_payment_credit(&wrong_opening, &send.request, &output, &encrypted_credit),
            Err(KagemushaStateErrorV1::InvalidPeerCredit)
        );
        let mut wrong_output = output.clone();
        wrong_output.sender_after_commitment[0] ^= 1;
        assert_eq!(
            open_payment_credit(&receiver, &send.request, &wrong_output, &encrypted_credit),
            Err(KagemushaStateErrorV1::InvalidPeerCredit)
        );
        let mut wrong_request = send.request.clone();
        wrong_request.amount += 1;
        assert_eq!(
            open_payment_credit(&receiver, &wrong_request, &output, &encrypted_credit),
            Err(KagemushaStateErrorV1::InvalidPeerCredit)
        );
        let mut altered = encrypted_credit.clone();
        *altered.last_mut().unwrap() ^= 1;
        for bytes in [
            Vec::new(),
            encrypted_credit[..encrypted_credit.len() - 1].to_vec(),
            [encrypted_credit.as_slice(), &[0]].concat(),
            altered,
        ] {
            assert_ne!(bytes, encrypted_credit);
            assert_eq!(
                open_payment_credit(&receiver, &send.request, &output, &bytes),
                Err(KagemushaStateErrorV1::InvalidPeerCredit)
            );
        }
    }
}
