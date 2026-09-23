//! Android one-use-key ratchet proof-head candidate and fail-closed fold gate.
//!
//! A governed credential binds the stable app/device enrollment key. Android
//! KeyMint's hardware-enforced one-use signing key is different at every hop.
//! Its public key must be committed by the preceding proof head, and the next
//! prepared key must be committed by the successor. These records check the
//! exact host-visible link; they do not substitute for a paired recursive
//! proof of key attestation, non-forking use, and head continuity.

use iroha_data_model::kagemusha::{
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1,
    KagemushaHardwareTransitionSelectionExpectedV1, KagemushaHardwareTransitionSelectionV1,
    KagemushaOperationKindV1, kagemusha_device_key_reference_v1,
};
use norito::codec::{Decode, Encode};

use super::{
    DigestV1, KagemushaStateErrorV1, KagemushaStateV1, KagemushaTransitionKindV1,
    TransitionProofStatementV1,
};

/// Enforce that one-use-key mode cannot disappear or reuse a prepared key.
pub(super) fn validate_next_key_transition(
    predecessor: &KagemushaStateV1,
    successor: &KagemushaStateV1,
) -> Result<(), KagemushaStateErrorV1> {
    let before = predecessor.next_one_use_key_reference;
    let after = successor.next_one_use_key_reference;
    if (before == [0; 32] && after == [0; 32])
        || (before != [0; 32] && after != [0; 32] && before != after)
    {
        Ok(())
    } else {
        Err(KagemushaStateErrorV1::StateInvariant)
    }
}

/// One proof head's commitment to the next authorized one-use KeyMint key.
///
/// `committed_next_key_reference` is distinct from the stable governed
/// credential's `device_key_reference`. A host record of this type has no
/// authority until both Pasta state commitments and their recursive link bind
/// the same fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::KagemushaOneUseKeyRatchetHeadV1")]
pub struct KagemushaOneUseKeyRatchetHeadV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Governance-signed stable credential identity.
    pub credential_id: DigestV1,
    /// Private aggregate state commitment represented by this head.
    pub state_commitment: DigestV1,
    /// Exact irreversible use index inherited by the next transition.
    pub secure_index: u128,
    /// Domain-separated reference to the next prepared one-use key.
    pub committed_next_key_reference: DigestV1,
}

/// One candidate transition consuming a predecessor-committed one-use key.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::KagemushaOneUseKeyRatchetLinkV1")]
pub struct KagemushaOneUseKeyRatchetLinkV1 {
    /// Exact predecessor head whose committed key signs this transition.
    pub predecessor: KagemushaOneUseKeyRatchetHeadV1,
    /// Exact successor head committing the key for the following transition.
    pub successor: KagemushaOneUseKeyRatchetHeadV1,
    /// Attested one-use key consumed by the selected transition.
    pub consumed_public_key: KagemushaDevicePublicKeyV1,
    /// Prepared and attested key committed for the successor's next use.
    pub prepared_successor_public_key: KagemushaDevicePublicKeyV1,
    /// Core's exact canonical monetary selection.
    pub selection: KagemushaHardwareTransitionSelectionV1,
    /// Low-S P-256 signature by `consumed_public_key` over canonical `S`.
    pub signature: KagemushaDeviceSignatureV1,
}

impl KagemushaOneUseKeyRatchetLinkV1 {
    /// Check the exact Core selection, one-use signature, and adjacent key heads.
    ///
    /// The `expected` comparisons must be reconstructed from authenticated
    /// release, credential, candidate, and terminal state. This method only
    /// checks the visible link; it does not validate a KeyMint certificate or
    /// recursively prove that the two heads are part of the monetary state.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed heads, skipped indices, substituted
    /// state/keys/selection, or an invalid signature.
    pub fn validate_signature_and_structure(
        &self,
        statement: &TransitionProofStatementV1,
        expected: KagemushaHardwareTransitionSelectionExpectedV1,
        predecessor_state: &KagemushaStateV1,
        successor_state: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let selected = &self.selection;
        let mismatch = || KagemushaStateErrorV1::HardwareCertificateMismatch;
        predecessor_state.validate()?;
        successor_state.validate()?;
        validate_next_key_transition(predecessor_state, successor_state)?;
        let operation = match statement.kind {
            KagemushaTransitionKindV1::MintFold => KagemushaOperationKindV1::MintFold,
            KagemushaTransitionKindV1::SendSplit => KagemushaOperationKindV1::SendSplit,
            KagemushaTransitionKindV1::ReceiveFold => KagemushaOperationKindV1::ReceiveFold,
            KagemushaTransitionKindV1::RedeemSplit => KagemushaOperationKindV1::RedeemSplit,
            KagemushaTransitionKindV1::Rotate => KagemushaOperationKindV1::Rotate,
        };
        selected.validate_shape().map_err(|_| mismatch())?;
        self.consumed_public_key
            .validate()
            .map_err(|_| mismatch())?;
        self.prepared_successor_public_key
            .validate()
            .map_err(|_| mismatch())?;
        if self.predecessor.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.successor.version != KAGEMUSHA_WIRE_VERSION_V1
            || statement.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.predecessor.credential_id == [0; 32]
            || self.predecessor.credential_id != self.successor.credential_id
            || self.predecessor.credential_id != selected.credential_id
            || self.predecessor.state_commitment != statement.predecessor_commitment
            || self.successor.state_commitment != statement.successor_commitment
            || self.predecessor.state_commitment != predecessor_state.state_commitment
            || self.successor.state_commitment != successor_state.state_commitment
            || self.predecessor.committed_next_key_reference
                != predecessor_state.next_one_use_key_reference
            || self.successor.committed_next_key_reference
                != successor_state.next_one_use_key_reference
            || self.predecessor.state_commitment == self.successor.state_commitment
            || self.predecessor.committed_next_key_reference
                != kagemusha_device_key_reference_v1(&self.consumed_public_key)
            || self.successor.committed_next_key_reference
                != kagemusha_device_key_reference_v1(&self.prepared_successor_public_key)
            || self.predecessor.committed_next_key_reference
                == self.successor.committed_next_key_reference
            || self.predecessor.secure_index.checked_add(1) != Some(self.successor.secure_index)
            || selected.secure_index_before != self.predecessor.secure_index
            || selected.secure_index_after != self.successor.secure_index
            || selected.secure_index_before != expected.secure_index_before
            || selected.release_id != statement.release_id
            || selected.release_id != expected.release_id
            || selected.hardware_policy_digest != expected.hardware_policy_digest
            || selected.app_policy_digest != expected.app_policy_digest
            || selected.operation_kind != operation
            || selected.operation_kind != expected.operation_kind
            || selected.transition_statement_digest != statement.digest()?
            || selected.transition_statement_digest != expected.transition_statement_digest
            || selected.candidate_envelope_digest != expected.candidate_envelope_digest
            || selected.terminal_body_commitment != expected.terminal_body_commitment
            || selected.network_id != statement.lane.network_id
            || selected.lane_commitment != statement.lane.device_lane_id
            || selected.hardware_profile_id != statement.hardware_profile_id
            || selected.policy_epoch != statement.policy_epoch
            || selected.hardware_epoch_id != statement.predecessor_epoch.epoch_id
            || u128::from(selected.hardware_epoch_generation)
                != statement.predecessor_epoch.generation
        {
            return Err(mismatch());
        }
        self.signature
            .verify(
                &self.consumed_public_key,
                &selected.canonical_signing_bytes().map_err(|_| mismatch())?,
            )
            .map_err(|_| mismatch())
    }

    /// Fail closed until the one-use key ratchet is bound by both Pasta folds.
    ///
    /// TODO: Replace this rejection only after a paired recursive relation
    /// constrains predecessor commitment to the consumed attested key, the
    /// exact signed Core selection, and the successor's prepared key, while
    /// also proving KeyMint app/hardware/one-use attestation and no-fork use.
    ///
    /// # Errors
    ///
    /// Always rejects monetary authorization in the current first-release
    /// candidate, even when the host-visible signature and link are valid.
    pub fn require_paired_monetary_fold(
        &self,
        statement: &TransitionProofStatementV1,
        expected: KagemushaHardwareTransitionSelectionExpectedV1,
        predecessor_state: &KagemushaStateV1,
        successor_state: &KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.validate_signature_and_structure(
            statement,
            expected,
            predecessor_state,
            successor_state,
        )?;
        Err(KagemushaStateErrorV1::ProofRejected(
            "one-use KeyMint key ratchet is not bound by both Pasta state folds".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_recursion::{
        RejectAllKagemushaRecursiveVerifierV1, tests::artifacts,
    };
    use crate::zk::kagemusha_v1_state::{
        DevicePolicyBindingV1, HardwareEpochV1, KagemushaLaneIdV1, KagemushaStateContextV1,
        KagemushaStateMachineV1, KagemushaStateProofReleaseV1,
        RejectAllKagemushaGuardBundleVerifierV1,
        tests::{snapshot_device_public_key, snapshot_device_signature},
    };
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        kagemusha::{
            KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaEnabledProfileV1,
            KagemushaEvidenceFileV1, KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1,
            kagemusha_suite_commitment_v1,
        },
        nexus::AxtAssetIncarnationV1,
    };
    use iroha_model_base::domain::DomainId;
    use p256::ecdsa::SigningKey;

    fn keyed_state(
        statement: &TransitionProofStatementV1,
        next_key_reference: DigestV1,
        balance: u128,
        sequence: u128,
        nonce: DigestV1,
    ) -> KagemushaStateV1 {
        let context = KagemushaStateContextV1 {
            protocol_version: 1,
            suite_id: statement.predecessor_suite_id,
            vk_digest: statement.predecessor_vk_digest,
            release_id: statement.release_id,
            asset_incarnation: statement.asset_incarnation,
            hardware_profile_id: statement.hardware_profile_id,
            policy_epoch: statement.policy_epoch,
        };
        let state = KagemushaStateV1::build_with_next_one_use_key_reference(
            context,
            statement.liability_pool_id,
            statement.lane.clone(),
            balance,
            sequence,
            statement.predecessor_epoch,
            statement.predecessor_device_policy_binding,
            next_key_reference,
            nonce,
            iroha_data_model::kagemusha::KagemushaPastaStateCommitmentV1::ZERO,
        )
        .expect("test aggregate state");
        state.validate().expect("keyed state commitment");
        state
    }

    fn fixture() -> (
        KagemushaOneUseKeyRatchetLinkV1,
        TransitionProofStatementV1,
        KagemushaHardwareTransitionSelectionExpectedV1,
        KagemushaStateV1,
        KagemushaStateV1,
    ) {
        let current_key = SigningKey::from_bytes((&[7; 32]).into()).expect("current one-use key");
        let next_key = SigningKey::from_bytes((&[8; 32]).into()).expect("next one-use key");
        let current_public_key = snapshot_device_public_key(&current_key);
        let next_public_key = snapshot_device_public_key(&next_key);
        let header = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"ratchet-genesis"));
        let network_id = NetworkId::from_genesis_hash(header);
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("ratchet", "universal").expect("domain"),
            "cash".parse().expect("asset name"),
        );
        let epoch = HardwareEpochV1 {
            generation: 1,
            epoch_id: [0x19; 32],
        };
        let binding = DevicePolicyBindingV1 {
            device_key_reference: [0x20; 32],
            hardware_policy_id: [0x21; 32],
        };
        let mut statement = TransitionProofStatementV1 {
            version: 1,
            protocol_version: 1,
            predecessor_suite_id: [0x22; 32],
            predecessor_vk_digest: [0x23; 32],
            successor_suite_id: [0x22; 32],
            successor_vk_digest: [0x23; 32],
            kind: KagemushaTransitionKindV1::MintFold,
            amount: 1,
            mint_finality_semantic_digest: [0x24; 32],
            mint_finality_proof_binding_digest: [0x25; 32],
            peer_credit_id: [0; 32],
            recipient_encryption_key_binding: [0; 32],
            lifecycle_binding_digest: [0; 32],
            prepared_transition_binding_digest: [0; 32],
            receive_credit_binding_digest: [0; 32],
            predecessor_release_id: [0x26; 32],
            release_id: [0x26; 32],
            asset_incarnation: AxtAssetIncarnationV1::derive(
                &network_id,
                &asset,
                &header,
                &Hash::new(b"ratchet-incarnation"),
                1,
            ),
            liability_pool_id: [0x27; 32],
            hardware_profile_id: [0x28; 32],
            policy_epoch: 1,
            lane: KagemushaLaneIdV1 {
                network_id,
                device_lane_id: [0x29; 32],
                asset,
                scale: 2,
            },
            predecessor_commitment: [0x30; 32],
            successor_commitment: [0x31; 32],
            predecessor_sequence: 3,
            successor_sequence: 4,
            predecessor_epoch: epoch,
            successor_epoch: epoch,
            predecessor_device_policy_binding: binding,
            successor_device_policy_binding: binding,
            predecessor_state_nonce_commitment: [0x32; 32],
            successor_state_nonce_commitment: [0x33; 32],
            journal_revision_before: 3,
            journal_revision_after: 4,
            effect_digest: [0x34; 32],
        };
        statement.liability_pool_id =
            super::super::derive_liability_pool_id(&statement.lane, statement.asset_incarnation)
                .expect("canonical liability pool");
        let predecessor_state = keyed_state(
            &statement,
            kagemusha_device_key_reference_v1(&current_public_key),
            0,
            3,
            statement.predecessor_state_nonce_commitment,
        );
        let successor_state = keyed_state(
            &statement,
            kagemusha_device_key_reference_v1(&next_public_key),
            1,
            4,
            statement.successor_state_nonce_commitment,
        );
        statement.predecessor_commitment = predecessor_state.state_commitment;
        statement.successor_commitment = successor_state.state_commitment;
        let statement_digest = statement.digest().expect("canonical Core statement");
        let selection = KagemushaHardwareTransitionSelectionV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            release_id: statement.release_id,
            hardware_policy_digest: [0x35; 32],
            app_policy_digest: [0x36; 32],
            credential_id: [0x37; 32],
            network_id,
            lane_commitment: statement.lane.device_lane_id,
            hardware_profile_id: statement.hardware_profile_id,
            policy_epoch: statement.policy_epoch,
            hardware_epoch_id: epoch.epoch_id,
            hardware_epoch_generation: 1,
            operation_kind: KagemushaOperationKindV1::MintFold,
            transition_statement_digest: statement_digest,
            candidate_envelope_digest: [0; 32],
            terminal_body_commitment: [0; 32],
            secure_index_before: 3,
            secure_index_after: 4,
        };
        let expected = KagemushaHardwareTransitionSelectionExpectedV1 {
            release_id: selection.release_id,
            hardware_policy_digest: selection.hardware_policy_digest,
            app_policy_digest: selection.app_policy_digest,
            operation_kind: selection.operation_kind,
            transition_statement_digest: statement_digest,
            candidate_envelope_digest: selection.candidate_envelope_digest,
            terminal_body_commitment: selection.terminal_body_commitment,
            secure_index_before: 3,
        };
        let signature = snapshot_device_signature(
            &current_key,
            &selection.canonical_signing_bytes().expect("selection S"),
        );
        let link = KagemushaOneUseKeyRatchetLinkV1 {
            predecessor: KagemushaOneUseKeyRatchetHeadV1 {
                version: 1,
                credential_id: selection.credential_id,
                state_commitment: statement.predecessor_commitment,
                secure_index: 3,
                committed_next_key_reference: kagemusha_device_key_reference_v1(
                    &current_public_key,
                ),
            },
            successor: KagemushaOneUseKeyRatchetHeadV1 {
                version: 1,
                credential_id: selection.credential_id,
                state_commitment: statement.successor_commitment,
                secure_index: 4,
                committed_next_key_reference: kagemusha_device_key_reference_v1(&next_public_key),
            },
            consumed_public_key: current_public_key,
            prepared_successor_public_key: next_public_key,
            selection,
            signature,
        };
        (
            link,
            statement,
            expected,
            predecessor_state,
            successor_state,
        )
    }

    #[test]
    fn exact_one_use_link_checks_signature_and_fails_closed_without_paired_fold() {
        let (link, statement, expected, predecessor_state, successor_state) = fixture();
        link.validate_signature_and_structure(
            &statement,
            expected,
            &predecessor_state,
            &successor_state,
        )
        .expect("host-visible key ratchet link is coherent");
        assert!(matches!(
            link.require_paired_monetary_fold(
                &statement,
                expected,
                &predecessor_state,
                &successor_state
            ),
            Err(KagemushaStateErrorV1::ProofRejected(_))
        ));
    }

    #[test]
    fn substituted_predecessor_or_successor_key_and_index_are_rejected() {
        let (link, statement, expected, predecessor_state, successor_state) = fixture();
        let mut changed = link.clone();
        changed.predecessor.committed_next_key_reference = [0x44; 32];
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
        let mut changed = link.clone();
        changed.successor.committed_next_key_reference = [0x45; 32];
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
        let mut changed = link.clone();
        changed.successor.secure_index += 1;
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
        let mut changed = link.clone();
        changed.predecessor.state_commitment = [0x46; 32];
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
        let substituted_predecessor = keyed_state(
            &statement,
            [0x49; 32],
            0,
            3,
            statement.predecessor_state_nonce_commitment,
        );
        assert!(
            link.validate_signature_and_structure(
                &statement,
                expected,
                &substituted_predecessor,
                &successor_state,
            )
            .is_err()
        );
    }

    #[test]
    fn changed_selection_or_signature_is_rejected() {
        let (link, statement, expected, predecessor_state, successor_state) = fixture();
        let mut changed = link.clone();
        changed.selection.lane_commitment = [0x47; 32];
        let current_key = SigningKey::from_bytes((&[7; 32]).into()).expect("current one-use key");
        changed.signature = snapshot_device_signature(
            &current_key,
            &changed
                .selection
                .canonical_signing_bytes()
                .expect("changed S"),
        );
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
        let mut changed = link.clone();
        changed.selection.app_policy_digest = [0x48; 32];
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
        let mut changed = link;
        changed.consumed_public_key = changed.prepared_successor_public_key;
        changed.predecessor.committed_next_key_reference =
            kagemusha_device_key_reference_v1(&changed.consumed_public_key);
        assert!(
            changed
                .validate_signature_and_structure(
                    &statement,
                    expected,
                    &predecessor_state,
                    &successor_state
                )
                .is_err()
        );
    }

    #[test]
    fn next_key_is_in_both_state_commitments_and_must_change_exactly_once() {
        let (_, statement, _, _, _) = fixture();
        let context = KagemushaStateContextV1 {
            protocol_version: 1,
            suite_id: statement.predecessor_suite_id,
            vk_digest: statement.predecessor_vk_digest,
            release_id: statement.release_id,
            asset_incarnation: statement.asset_incarnation,
            hardware_profile_id: statement.hardware_profile_id,
            policy_epoch: statement.policy_epoch,
        };
        let liability_pool_id =
            super::super::derive_liability_pool_id(&statement.lane, statement.asset_incarnation)
                .expect("canonical liability pool");
        let counter_head = KagemushaStateV1::build(
            context,
            liability_pool_id,
            statement.lane.clone(),
            0,
            0,
            statement.predecessor_epoch,
            statement.predecessor_device_policy_binding,
            [0x61; 32],
            iroha_data_model::kagemusha::KagemushaPastaStateCommitmentV1::ZERO,
        )
        .expect("test aggregate state");
        let first = keyed_state(&statement, [0x62; 32], 0, 0, [0x61; 32]);
        let second = keyed_state(&statement, [0x63; 32], 0, 0, [0x61; 32]);
        let mut stale = first.clone();
        stale.next_one_use_key_reference = [0x63; 32];
        assert_eq!(
            stale.validate(),
            Err(KagemushaStateErrorV1::StateCommitmentMismatch)
        );
        assert_ne!(
            first.state_commitment_components.eq,
            second.state_commitment_components.eq
        );
        assert_ne!(
            first.state_commitment_components.ep,
            second.state_commitment_components.ep
        );
        validate_next_key_transition(&first, &second).expect("fresh successor key");
        assert_eq!(
            validate_next_key_transition(&first, &first),
            Err(KagemushaStateErrorV1::StateInvariant)
        );
        assert_eq!(
            validate_next_key_transition(&first, &counter_head),
            Err(KagemushaStateErrorV1::StateInvariant)
        );
        assert_eq!(
            validate_next_key_transition(&counter_head, &second),
            Err(KagemushaStateErrorV1::StateInvariant)
        );
    }

    #[test]
    fn bootstrap_preview_commits_initial_prepared_key_in_both_parities() {
        let (_, statement, _, _, _) = fixture();
        let governance_key = SigningKey::from_bytes((&[9; 32]).into()).expect("governance key");
        let hardware_profile = KagemushaHardwareProfileV1 {
            version: 1,
            protocol_version: 1,
            hardware_profile_id: [0; 32],
            provider_id: [0x71; 32],
            platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
            product_class_digest: [0x72; 32],
            firmware_policy_digest: [0x73; 32],
            enrollment_attestation_verifier_digest: [0x74; 32],
            attestation_trust_roots_digest: [0x75; 32],
            allowed_suite_commitment: kagemusha_suite_commitment_v1(statement.predecessor_suite_id),
            policy_epoch: 1,
            governance_credential_public_key: snapshot_device_public_key(&governance_key),
            capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [0x76; 32],
            valid_from_ms: 1,
            expires_at_ms: 1_000,
            app_attestation_authority_policy_digest: [0x77; 32],
        }
        .seal_hardware_profile_id()
        .expect("synthetic governed profile");
        let enabled = KagemushaEnabledProfileV1 {
            hardware_profile,
            hardware_profile_id: hardware_profile.hardware_profile_id,
            suite_id: statement.predecessor_suite_id,
            vk_digest: statement.predecessor_vk_digest,
            qualification_digest: [0x78; 32],
            policy_epoch: 1,
            qualification_report: KagemushaEvidenceFileV1 {
                sha256: hardware_profile.qualification_report_digest,
                byte_len: 1,
            },
        };
        let artifacts = artifacts();
        let proof_release =
            KagemushaStateProofReleaseV1::from_test_artifacts(artifacts, vec![enabled])
                .expect("synthetic proof release");
        let context = KagemushaStateContextV1 {
            protocol_version: 1,
            suite_id: statement.predecessor_suite_id,
            vk_digest: statement.predecessor_vk_digest,
            release_id: artifacts.release_id,
            asset_incarnation: statement.asset_incarnation,
            hardware_profile_id: hardware_profile.hardware_profile_id,
            policy_epoch: 1,
        };
        type Machine = KagemushaStateMachineV1<
            RejectAllKagemushaRecursiveVerifierV1,
            RejectAllKagemushaGuardBundleVerifierV1,
            super::super::KagemushaMemoryAuthenticatedHistoryStoreV1,
        >;
        let initial = [0x79; 32];
        let ratchet = Machine::preview_bootstrap(
            proof_release.clone(),
            context,
            statement.lane.clone(),
            statement.predecessor_epoch,
            statement.predecessor_device_policy_binding,
            initial,
            [0x7a; 32],
            10,
        )
        .expect("candidate ratchet bootstrap preview");
        let counter = Machine::preview_bootstrap(
            proof_release,
            context,
            statement.lane,
            statement.predecessor_epoch,
            statement.predecessor_device_policy_binding,
            [0; 32],
            [0x7a; 32],
            10,
        )
        .expect("counter bootstrap preview");
        assert_eq!(ratchet.state.next_one_use_key_reference, initial);
        assert_eq!(ratchet.statement.next_one_use_key_reference, initial);
        assert_ne!(
            ratchet.state.state_commitment_components.eq,
            counter.state.state_commitment_components.eq
        );
        assert_ne!(
            ratchet.state.state_commitment_components.ep,
            counter.state.state_commitment_components.ep
        );
    }
}
