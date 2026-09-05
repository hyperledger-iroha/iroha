//! Native-only private inputs retained from authenticated mint staging into `MintFold`.
//!
//! A caller-provided credit and host-computed digest cannot establish recipient ownership. This
//! projection is constructed only after the state machine selects an exact current pending-map
//! entry and rechecks its authenticated snapshot, replay, retained-credential and Guard evidence.
//! It retains the original credential across ordinary epoch rotation and is deliberately not
//! serializable.

use super::*;
use iroha_data_model::kagemusha::{KagemushaHardwareCredentialV1, KagemushaMintAuthorizationV1};

/// Exact private mint material forwarded from authenticated staging to the recursive witness.
///
/// The custom debug representation omits the plaintext opening, authorization and credit. This
/// value must not cross an SDK, peer, log or unauthenticated persistence boundary.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct KagemushaMintFoldPrivateInputsV1 {
    authorization: KagemushaMintAuthorizationV1,
    recipient_credential: KagemushaHardwareCredentialV1,
    credit_opening: KagemushaCreditOpeningV1,
    credit: KagemushaMintCreditV1,
    stage_certificate: MintStageCertificateV1,
}

/// Borrowed recursive opening of one exact authenticated staged-mint projection.
///
/// There is deliberately no detached constructor. It is created only inside the opaque
/// capability derived from the checked pending entry. Staging evidence remains native journal
/// provenance; it is not presented as circuit authority.
#[derive(Clone, Copy)]
pub(crate) struct KagemushaMintFoldOpeningWitnessV1<'a> {
    authorization: &'a KagemushaMintAuthorizationV1,
    recipient_credential: &'a KagemushaHardwareCredentialV1,
    credit_opening: &'a KagemushaCreditOpeningV1,
    credit: &'a KagemushaMintCreditV1,
}

impl std::fmt::Debug for KagemushaMintFoldOpeningWitnessV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFoldOpeningWitnessV1")
            .field(
                "credit_id",
                &CreditIdV1(self.credit.statement.lifecycle.credit_id),
            )
            .finish_non_exhaustive()
    }
}

/// Opaque authority to disclose the recursive opening of one checked `MintFold` preview.
///
/// This capability is borrowed from the state-owned private inputs, cannot be constructed by
/// callers, and deliberately has no serialization or default representation. Its debug output
/// identifies the public credit only and omits all opening material.
#[derive(Clone, Copy)]
pub struct KagemushaMintFoldOpeningCapabilityV1<'a> {
    opening: KagemushaMintFoldOpeningWitnessV1<'a>,
}

impl std::fmt::Debug for KagemushaMintFoldOpeningCapabilityV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFoldOpeningCapabilityV1")
            .field(
                "credit_id",
                &CreditIdV1(self.opening.credit.statement.lifecycle.credit_id),
            )
            .finish_non_exhaustive()
    }
}

impl<'a> KagemushaMintFoldOpeningCapabilityV1<'a> {
    /// Reveal the private recursive witness only inside `iroha_core`.
    pub(crate) fn opening(self) -> KagemushaMintFoldOpeningWitnessV1<'a> {
        self.opening
    }
}

impl<'a> KagemushaMintFoldOpeningWitnessV1<'a> {
    /// Exact paired recipient authorization selected by authenticated staging.
    pub(crate) fn authorization(self) -> &'a KagemushaMintAuthorizationV1 {
        self.authorization
    }

    /// Original enrolled credential; ordinary rotation does not rewrite provenance.
    pub(crate) fn recipient_credential(self) -> &'a KagemushaHardwareCredentialV1 {
        self.recipient_credential
    }

    /// Private commitment openings recovered by the authenticated recipient.
    pub(crate) fn credit_opening(self) -> &'a KagemushaCreditOpeningV1 {
        self.credit_opening
    }

    /// Exact finalized credit whose canonical envelope is consumed by `MintFold`.
    pub(crate) fn credit(self) -> &'a KagemushaMintCreditV1 {
        self.credit
    }
}

impl std::fmt::Debug for KagemushaMintFoldPrivateInputsV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("KagemushaMintFoldPrivateInputsV1")
            .field(
                "credit_id",
                &CreditIdV1(self.credit.statement.lifecycle.credit_id),
            )
            .field(
                "hardware_epoch",
                &self.stage_certificate.statement.hardware_epoch,
            )
            .field(
                "inbox_revision",
                &self.stage_certificate.statement.inbox_revision_after,
            )
            .finish_non_exhaustive()
    }
}

impl KagemushaMintFoldPrivateInputsV1 {
    /// Clone a record only after the state machine has authenticated that exact pending entry.
    ///
    /// This constructor is state-module-private. Its caller must select the record from the
    /// machine's current pending map and validate the complete snapshot and GuardBundle; detached
    /// decoded `StagedMintCreditV1` values are not accepted by the public state API.
    pub(super) fn from_checked_pending(
        staged: &StagedMintCreditV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let reservation = staged.reservation();
        reservation.validate_inputs()?;
        staged
            .credit()
            .validate_shape_against_authorization(reservation.authorization())
            .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
        if staged.envelope_digest() != mint_envelope_digest_v1(staged.credit())?
            || staged.credit_id().0 != staged.credit().statement.lifecycle.credit_id
            || staged.stage_certificate().statement.credit_id != staged.credit_id()
            || staged.stage_certificate().statement.envelope_digest != staged.envelope_digest()
            || staged.stage_certificate().statement.reservation_digest != reservation.digest()?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(Self {
            authorization: reservation.authorization().clone(),
            recipient_credential: reservation.recipient_credential().clone(),
            credit_opening: *reservation.credit_opening(),
            credit: staged.credit().clone(),
            stage_certificate: staged.stage_certificate().clone(),
        })
    }

    /// Exact authorization, including the proof whose assigned bytes the circuit must hash.
    pub(crate) fn authorization(&self) -> &KagemushaMintAuthorizationV1 {
        &self.authorization
    }

    /// Original enrolled credential; ordinary rotation must not rewrite committed provenance.
    pub(crate) fn recipient_credential(&self) -> &KagemushaHardwareCredentialV1 {
        &self.recipient_credential
    }

    /// Plaintext commitment openings known only after authenticated recipient decryption.
    pub(crate) fn credit_opening(&self) -> &KagemushaCreditOpeningV1 {
        &self.credit_opening
    }

    /// Exact finalized credit whose complete canonical envelope enters the replay leaf.
    pub(crate) fn credit(&self) -> &KagemushaMintCreditV1 {
        &self.credit
    }

    /// Original irreversible staging evidence, retained byte-for-byte across recovery.
    pub(crate) fn stage_certificate(&self) -> &MintStageCertificateV1 {
        &self.stage_certificate
    }

    /// Borrow the sole public capability derived from this checked pending entry.
    pub(super) fn opening_capability(&self) -> KagemushaMintFoldOpeningCapabilityV1<'_> {
        KagemushaMintFoldOpeningCapabilityV1 {
            opening: self.recursive_witness(),
        }
    }

    /// Borrow the recursive witness projection retained inside the opaque capability.
    fn recursive_witness(&self) -> KagemushaMintFoldOpeningWitnessV1<'_> {
        KagemushaMintFoldOpeningWitnessV1 {
            authorization: &self.authorization,
            recipient_credential: &self.recipient_credential,
            credit_opening: &self.credit_opening,
            credit: &self.credit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::DomainId,
        kagemusha::{
            KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1,
            KagemushaLifecycleBindingV1, KagemushaMintAuthorizationContextV1,
            KagemushaMintAuthorizationStatementV1, KagemushaMintCreditStatementV1,
            KagemushaOperationKindV1, KagemushaPairedProofV1, kagemusha_device_key_reference_v1,
        },
        nexus::AxtAssetIncarnationV1,
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    const PUBLIC_CREDIT_ID: DigestV1 = [0x31; 32];
    const SECRET_BYTES: DigestV1 = [0xA5; 32];

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"kagemusha-mint-fold-opening-tests",
        )))
    }

    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }

    fn asset_incarnation(
        network_id: &NetworkId,
        asset: &AssetDefinitionId,
    ) -> AxtAssetIncarnationV1 {
        let registration = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"kagemusha-mint-fold-opening-registration",
        ));
        AxtAssetIncarnationV1::derive(
            network_id,
            asset,
            &registration,
            &Hash::new(b"kagemusha-mint-fold-opening-execution"),
            1,
        )
    }

    fn account(tag: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![tag; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes((&[7; 32]).into()).expect("P-256 signing key")
    }

    fn device_public_key(signing_key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
        .expect("device public key")
    }

    fn device_signature(signing_key: &SigningKey) -> KagemushaDeviceSignatureV1 {
        let signature: Signature = signing_key.sign(b"mint-fold-opening-capability-test");
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("low-S signature")
    }

    fn paired_proof(tag: u8) -> KagemushaPairedProofV1 {
        KagemushaPairedProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: [tag; 32],
            ep_protocol_digest: [tag.wrapping_add(1); 32],
            semantic_digest: [tag.wrapping_add(2); 32],
            guard_eq_credential_audit: [tag.wrapping_add(3); 32],
            guard_ep_credential_audit: [tag.wrapping_add(4); 32],
            eq_deferred_audit: [tag.wrapping_add(5); 32],
            ep_deferred_audit: [tag.wrapping_add(6); 32],
            eq_proof: vec![tag; 8],
            ep_proof: vec![tag.wrapping_add(1); 8],
            eq_history: vec![tag.wrapping_add(2); 8],
            ep_history: vec![tag.wrapping_add(3); 8],
        }
    }

    fn private_inputs_fixture() -> KagemushaMintFoldPrivateInputsV1 {
        let network_id = network();
        let asset = asset();
        let asset_incarnation = asset_incarnation(&network_id, &asset);
        let payer = account(1);
        let recipient = account(2);
        let signing_key = signing_key();
        let device_public_key = device_public_key(&signing_key);
        let authorization_context = KagemushaMintAuthorizationContextV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            operation_id: [0x20; 32],
            release_id: [0x21; 32],
            suite_id: [0x22; 32],
            vk_digest: [0x23; 32],
            artifact_manifest_digest: [0x24; 32],
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 4,
            liability_pool_id: [0x25; 32],
            amount: 19,
            payer,
            recipient: recipient.clone(),
            hardware_credential_id: [0x26; 32],
            hardware_profile_id: [0x27; 32],
            policy_epoch: 3,
            recipient_credential_commitment: [0x28; 32],
            credit_commitment: [0x29; 32],
            recipient_one_time_key: [0x2A; 32],
        };
        let authorization = KagemushaMintAuthorizationV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement: KagemushaMintAuthorizationStatementV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                context: authorization_context,
                issuance_commitment: [0x2B; 32],
                credit_id: PUBLIC_CREDIT_ID,
                ciphertext_digest: [0x2C; 32],
            },
            proof: paired_proof(0xA5),
        };
        let recipient_credential = KagemushaHardwareCredentialV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            credential_id: [0x26; 32],
            network_id,
            hardware_profile_id: [0x27; 32],
            suite_id: [0x22; 32],
            firmware_policy_digest: SECRET_BYTES,
            policy_epoch: 3,
            lane_commitment: [0x2D; 32],
            hardware_epoch_id: [0x2E; 32],
            hardware_epoch_generation: 7,
            device_public_key,
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 10,
            expires_at_ms: 20,
            governance_signature: device_signature(&signing_key),
        };
        let credit_opening = KagemushaCreditOpeningV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            credit_id: PUBLIC_CREDIT_ID,
            amount: 19,
            credit_commitment_opening: SECRET_BYTES,
            recipient_binding_opening: [0xA6; 32],
            recovery_nonce: [0xA7; 32],
        };
        let lifecycle = KagemushaLifecycleBindingV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            network_id,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            suite_id: [0x22; 32],
            vk_digest: [0x23; 32],
            release_id: [0x21; 32],
            asset: asset.clone(),
            asset_incarnation,
            scale: 4,
            liability_pool_id: [0x25; 32],
            hardware_profile_id: [0x27; 32],
            policy_epoch: 3,
            operation_kind: KagemushaOperationKindV1::MintFold,
            request_id: [0; 32],
            receiver_lane_commitment: [0; 32],
            credit_id: PUBLIC_CREDIT_ID,
            ciphertext_digest: [0; 32],
        };
        let credit = KagemushaMintCreditV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement: KagemushaMintCreditStatementV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                lifecycle,
                recipient_credential_commitment: [0x28; 32],
                authorization_context_digest: [0x2F; 32],
                mint_authorization_digest: [0x30; 32],
                amount: 19,
                issuance_commitment: [0x2B; 32],
                recipient,
                credit_commitment: [0x29; 32],
                minted_at_ms: 11,
            },
            proof: paired_proof(0xB5),
            finality_certificate_binding: [0x32; 32],
            finality_authority_head: [0x33; 32],
            finality_genesis_roster_id: [0x34; 32],
            finality_proof_binding_digest: [0x35; 32],
            encrypted_credit: vec![0xA8; 48],
            artifact_manifest_digest: [0x24; 32],
        };
        KagemushaMintFoldPrivateInputsV1 {
            authorization,
            recipient_credential,
            credit_opening,
            credit,
            stage_certificate: MintStageCertificateV1 {
                statement: MintStageStatementV1 {
                    version: KAGEMUSHA_STATE_VERSION_V1,
                    lane: KagemushaLaneIdV1 {
                        network_id,
                        device_lane_id: [0x36; 32],
                        asset,
                        scale: 4,
                    },
                    hardware_epoch: HardwareEpochV1 {
                        generation: 7,
                        epoch_id: [0x2E; 32],
                    },
                    state_commitment: [0x37; 32],
                    inbox_revision_before: 8,
                    inbox_revision_after: 9,
                    reservation_digest: [0x38; 32],
                    credit_id: CreditIdV1(PUBLIC_CREDIT_ID),
                    envelope_digest: [0x39; 32],
                    staged_at_ms: 12,
                    predecessor_journal_commitment: [0x3A; 32],
                    successor_journal_commitment: [0x3B; 32],
                    successor_capacity_commitment: [0x3C; 32],
                },
                guard_bundle: vec![0xA9; 16],
            },
        }
    }

    #[test]
    fn opening_debug_redacts_all_private_material() {
        let inputs = private_inputs_fixture();
        let capability = inputs.opening_capability();
        let witness = capability.opening();
        let public_credit_id = format!("{:?}", CreditIdV1(PUBLIC_CREDIT_ID));
        let secret_bytes = format!("{SECRET_BYTES:?}");

        for rendered in [
            format!("{inputs:?}"),
            format!("{capability:?}"),
            format!("{witness:?}"),
        ] {
            assert!(rendered.contains("credit_id"));
            assert!(rendered.contains(&public_credit_id));
            assert!(!rendered.contains(&secret_bytes));
            assert!(!rendered.contains("credit_commitment_opening"));
            assert!(!rendered.contains("recipient_binding_opening"));
            assert!(!rendered.contains("recovery_nonce"));
            assert!(!rendered.contains("authorization"));
            assert!(!rendered.contains("recipient_credential"));
        }
    }

    #[test]
    fn opening_capability_round_trips_exact_private_input_borrows() {
        let inputs = private_inputs_fixture();
        let witness = inputs.opening_capability().opening();

        assert!(std::ptr::eq(
            witness.authorization(),
            inputs.authorization()
        ));
        assert!(std::ptr::eq(
            witness.recipient_credential(),
            inputs.recipient_credential()
        ));
        assert!(std::ptr::eq(
            witness.credit_opening(),
            inputs.credit_opening()
        ));
        assert!(std::ptr::eq(witness.credit(), inputs.credit()));
    }
}
