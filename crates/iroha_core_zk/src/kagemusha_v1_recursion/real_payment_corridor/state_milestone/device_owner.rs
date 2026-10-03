//! Per-device genuine proof and Core journal ownership for the diagnostic handoff fixture.
//!
//! The outer context owns the shared proving material. Each device retains its original Core
//! machine, independently bound credential and keys, actual locked journals, and current proof.
//! Signed simulated checkpoint selection is test-provider evidence, not physical durability.

use super::*;
use crate::kagemusha_v1_state::{
    KagemushaCoordinatorOperationStoreV1, KagemushaResponseEvidenceArchiveV1,
};

pub(super) struct DiagnosticDeviceProofContextV1<'a> {
    pub(super) funded: &'a RealFundedPrerequisite,
    pub(super) keys: &'a StateKeys,
    pub(super) artifacts: KagemushaRecursionArtifactsV1,
    pub(super) incoming: &'a IncomingStateProofMaterial,
}

pub(super) struct DiagnosticDeviceV1<'a> {
    pub(super) context: &'a DiagnosticDeviceProofContextV1<'a>,
    pub(super) device_index: u64,
    pub(super) material: &'a MintRecipientMaterial,
    pub(super) credential: &'a CredentialProof,
    pub(super) machine: DiagnosticMachine<'a>,
    pub(super) current: Rc<KagemushaGeneratedRecursiveStateProofV1>,
    pub(super) guard_verifier: DiagnosticGuardVerifier<'a>,
    pub(super) recursive_verifier: DiagnosticVerifier<'a>,
    pub(super) device_key: SigningKey,
    pub(super) journal_key: SigningKey,
    pub(super) _coordinator: KagemushaCoordinatorOperationStoreV1,
    pub(super) _responses: KagemushaResponseEvidenceArchiveV1,
    // Fields drop in declaration order: close both journal owners before removing their directory.
    _bootstrap_storage: tempfile::TempDir,
}

fn device_keys(
    device_index: u64,
    material: &MintRecipientMaterial,
) -> Result<(SigningKey, SigningKey), String> {
    material.platform_credential.validate()?;
    material
        .hardware_credential
        .validate_against_profile(&material.hardware_profile)
        .map_err(|error| error.to_string())?;
    let device_key = deterministic_signing_key(device_index);
    ensure(
        device_public_key(&device_key) == material.hardware_credential.device_public_key,
        "diagnostic device index does not own the original credential key",
    )?;
    let journal_key = SigningKey::from_bytes((&material.provider_policy.provider_secret).into())
        .map_err(|error| error.to_string())?;
    ensure(
        material.platform_credential.provider_authority_secret
            == material.provider_policy.provider_secret,
        "diagnostic journal key does not belong to the proved provider relation",
    )?;
    Ok((device_key, journal_key))
}

impl<'a> DiagnosticDeviceV1<'a> {
    /// Prove and admit the genuine zero-balance bootstrap, retaining all original storage owners.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn bootstrap(
        context: &'a DiagnosticDeviceProofContextV1<'a>,
        device_index: u64,
        material: &'a MintRecipientMaterial,
        credential: &'a CredentialProof,
        guard_keys: &mut Option<GuardKeys>,
        shared_verifier: DiagnosticVerifier<'a>,
        capacity: KagemushaDurableCapacityV1,
        history_bytes: u64,
    ) -> Result<Self, String> {
        capacity.validate().map_err(|error| error.to_string())?;
        let (device_key, journal_key) = device_keys(device_index, material)?;
        ensure(
            credential.relation == material.platform_credential,
            "diagnostic device credential proof belongs to different original material",
        )?;
        ensure(
            std::ptr::eq(shared_verifier.funded, context.funded)
                && std::ptr::eq(shared_verifier.keys, context.keys)
                && shared_verifier.artifacts == context.artifacts,
            "diagnostic shared verifier does not retain the original proof context",
        )?;
        let funded = context.funded;
        let preview = bootstrap_preview(material, context.artifacts);
        ensure(
            preview.state.balance == 0 && preview.state.logical_sequence == 0,
            "Core bootstrap must begin at the original zero balance and sequence",
        )?;
        let bootstrap_guard = Rc::new(prove_guard(
            &funded.eq,
            &funded.ep,
            &funded.credential_keys,
            guard_keys,
            guard_relation(material, preview.normalized_guard_statement),
            credential,
            credential,
        ));
        let keys = guard_keys
            .as_ref()
            .ok_or_else(|| "genuine bootstrap did not retain Guard keys".to_owned())?;
        ensure(
            keys.eq_protocol_digest == context.artifacts.guard_bundle_eq_protocol_digest
                && keys.ep_protocol_digest == context.artifacts.guard_bundle_ep_protocol_digest,
            "diagnostic bootstrap Guard keys differ from the original artifact context",
        )?;
        let relation = bootstrap_relation_for_corridor(
            preview.state.clone(),
            &bootstrap_guard,
            preview.transport_semantic_digest,
            RecursiveStateProtocolBindings::new(
                context.keys.eq_protocol_digest,
                context.keys.ep_protocol_digest,
                keys,
                funded,
                context.incoming,
            ),
        );
        let parent = dummy_parent(
            &context.keys.eq_protocol,
            &context.keys.ep_protocol,
            initial_kagemusha_eq_accumulator_v1(&funded.eq).map_err(|error| error.to_string())?,
            initial_kagemusha_ep_accumulator_v1(&funded.ep).map_err(|error| error.to_string())?,
        );
        let current = Rc::new(prove_recursive_state_step(
            funded,
            context.keys,
            &bootstrap_guard,
            keys,
            &parent,
            context.incoming,
            relation,
            None,
        ));
        shared_verifier.retain_state(Rc::clone(&current));
        let checkpoint =
            recovery_checkpoint::DiagnosticCheckpointRegister::new(material, &preview.state)?;
        let guard_verifier = DiagnosticGuardVerifier {
            funded,
            material,
            eq_protocol: keys.eq_protocol.clone(),
            ep_protocol: keys.ep_protocol.clone(),
            journal_key: device_public_key(&journal_key),
            checkpoint: checkpoint.clone(),
            proofs: Rc::new(RefCell::new(BTreeMap::new())),
        };
        let guard_bundle = guard_verifier.retain(bootstrap_guard);
        let mut enrollment = diagnostic_enrollment_binding(material, &preview.state)?;
        // Bind the simulated provider's actual signing key before admission. A placeholder
        // selector cannot authenticate the subsequent original Core-to-device release command.
        enrollment.core_authorization_key_reference =
            crate::kagemusha_sender_wire::hardware_authorization_key_reference_v1(
                &device_public_key(&journal_key),
            );
        let verified = DiagnosticMachine::stage_bootstrap_for_test(
            diagnostic_release(material, context.artifacts),
            preview.state.context(),
            preview.state.lane.clone(),
            preview.state.hardware_epoch,
            preview.state.device_policy_binding,
            preview.state.next_one_use_key_reference,
            preview.state.state_nonce_commitment,
            BOOTSTRAP_TIME,
            capacity,
            KagemushaMemoryAuthenticatedHistoryStoreV1::new(history_bytes),
            BootstrapAuthorizationV1 {
                proof: current.proof.clone(),
                guard_bundle,
            },
            material.hardware_credential,
            enrollment,
            shared_verifier.clone(),
            guard_verifier.clone(),
        )
        .map_err(|error| error.to_string())?;
        let bootstrap_storage = tempfile::tempdir().map_err(|error| error.to_string())?;
        let directory = bootstrap_storage
            .path()
            .canonicalize()
            .map_err(|error| error.to_string())?;
        let pending = verified
            .initialize_journals(
                &directory.join("journals"),
                capacity.outbox_bytes,
                digest(b"handoff-device-bootstrap-checkpoint", device_index),
            )
            .map_err(|error| error.to_string())?;
        checkpoint.retain_pending(&pending)?;
        let certificate = checkpoint.commit(pending.statement())?;
        let (machine, coordinator, responses) = pending
            .finish(certificate)
            .map_err(|error| error.to_string())?
            .into_parts();
        ensure(
            machine.state() == &preview.state,
            "genuine admitted bootstrap must equal Core's exact original preview",
        )?;
        Ok(Self {
            context,
            device_index,
            material,
            credential,
            machine,
            current,
            guard_verifier,
            recursive_verifier: shared_verifier,
            device_key,
            journal_key,
            _coordinator: coordinator,
            _responses: responses,
            _bootstrap_storage: bootstrap_storage,
        })
    }

    /// Apply the original device-zero finalized credit through actual reservation and MintFold.
    pub(super) fn apply_funded_mint(
        &mut self,
        guard_keys: &mut Option<GuardKeys>,
        successor_nonce: DigestV1,
        committed_at_ms: u64,
    ) -> Result<(), String> {
        let funded = self.context.funded;
        ensure(
            self.device_index == 0
                && self.material.platform_credential == funded.material.platform_credential
                && self.material.hardware_credential == funded.material.hardware_credential,
            "only the original funded device may consume this finalized mint credit",
        )?;
        let reservation = mint_reservation(funded);
        let reservation_statement = self
            .machine
            .preview_mint_reservation(&reservation)
            .map_err(|error| error.to_string())?;
        let reservation_certificate = MintReservationCertificateV1 {
            guard_bundle: sign_journal(
                &self.journal_key,
                RESERVATION_DOMAIN,
                &reservation_statement,
            ),
            statement: reservation_statement,
        };
        self.machine
            .reserve_mint_credit(&reservation, &reservation_certificate)
            .map_err(|error| error.to_string())?;
        let token = self
            .recursive_verifier
            .verify_stage(&reservation, &funded.mint_credit)?;
        let verified = VerifiedMintStageV1::from_genuine_diagnostic_proofs(
            reservation,
            funded.mint_credit.clone(),
            token,
        )
        .map_err(|error| error.to_string())?;
        let finality = verified.mint_finality();
        let stage_statement = self
            .machine
            .preview_stage_mint_credit(&verified, STAGE_TIME)
            .map_err(|error| error.to_string())?;
        let stage_certificate = MintStageCertificateV1 {
            guard_bundle: sign_journal(&self.journal_key, STAGE_DOMAIN, &stage_statement),
            statement: stage_statement,
        };
        self.machine
            .stage_mint_credit(
                &funded.authorization.authorization,
                &funded.mint_credit,
                Some(&verified),
                Some(&stage_certificate),
            )
            .map_err(|error| error.to_string())?;
        ensure(
            self.machine.state().balance == 0,
            "stage alone cannot mint a device balance",
        )?;
        let preview = self
            .machine
            .preview_mint_fold(&funded.mint_credit, successor_nonce, committed_at_ms)
            .map_err(|error| error.to_string())?;
        let expected = preview.transition.successor.clone();
        ensure(
            expected.balance == funded.mint_credit.statement.amount,
            "mint successor must contain exactly the original finalized amount",
        )?;
        let guard = Rc::new(prove_guard(
            &funded.eq,
            &funded.ep,
            &funded.credential_keys,
            guard_keys,
            guard_relation(self.material, preview.transition.normalized_guard_statement),
            self.credential,
            self.credential,
        ));
        let keys = guard_keys
            .as_ref()
            .ok_or_else(|| "genuine mint did not retain Guard keys".to_owned())?;
        ensure(
            keys.eq_protocol_digest == self.context.artifacts.guard_bundle_eq_protocol_digest
                && keys.ep_protocol_digest
                    == self.context.artifacts.guard_bundle_ep_protocol_digest,
            "diagnostic mint Guard keys differ from the original artifact context",
        )?;
        let guard_bundle = self.guard_verifier.retain(Rc::clone(&guard));
        let relation = transition_relation_for_corridor(
            self.machine.state().clone(),
            &preview.transition,
            &guard,
            RecursiveStateProtocolBindings::new(
                self.context.keys.eq_protocol_digest,
                self.context.keys.ep_protocol_digest,
                keys,
                funded,
                self.context.incoming,
            ),
            Some(KagemushaReplayInsertWitnessV1::from(
                &preview.replay_insert_witness,
            )),
            None,
            None,
        );
        let parent = parent_from_generated((*self.current).clone());
        let next = Rc::new(prove_recursive_state_step(
            funded,
            self.context.keys,
            &guard,
            keys,
            &parent,
            self.context.incoming,
            relation,
            Some(preview.mint_fold_opening()),
        ));
        self.recursive_verifier.retain_state(Rc::clone(&next));
        let authorization = TransitionAuthorizationV1::new(
            HardwareTransitionCertificateV1 {
                statement: preview.transition.hardware_statement.clone(),
                guard_bundle,
            },
            next.proof.clone(),
        );
        let root_message = self
            .machine
            .mint_fold_history_root_selection_signing_bytes(&preview)
            .map_err(|error| error.to_string())?;
        let authorization = self
            .machine
            .authorize_mint_fold_history(
                &preview,
                authorization,
                &device_public_key(&self.device_key),
                device_signature(&self.device_key, &root_message),
            )
            .map_err(|error| error.to_string())?;
        self.machine
            .mint_fold_prepared(funded.mint_credit.clone(), preview, finality, authorization)
            .map_err(|error| error.to_string())?;
        // The next proof is installed only after the original Core transition succeeds.
        self.current = next;
        ensure(
            self.machine.state() == &expected,
            "actual successful MintFold must equal its original Core successor",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_keys_preserve_each_original_credential_and_reject_substituted_index() {
        let release = digest(b"device-owner-key-preflight-release", 0);
        let manifest = digest(b"device-owner-key-preflight-manifest", 0);
        let first =
            core_bound_mint_recipient_material(0, release, digest(b"vk-set", 0), manifest, 1_000);
        let second =
            core_bound_mint_recipient_material(1, release, digest(b"vk-set", 0), manifest, 1_000);
        let (first_key, first_provider) = device_keys(0, &first).unwrap();
        let (second_key, second_provider) = device_keys(1, &second).unwrap();
        assert_eq!(
            device_public_key(&first_key),
            first.hardware_credential.device_public_key
        );
        assert_eq!(
            device_public_key(&second_key),
            second.hardware_credential.device_public_key
        );
        assert_ne!(
            device_public_key(&first_key),
            device_public_key(&second_key)
        );
        assert_ne!(
            first.hardware_credential.lane_commitment,
            second.hardware_credential.lane_commitment
        );
        let message = b"diagnostic device owner key isolation";
        let signature = device_signature(&first_key, message);
        assert!(
            signature
                .verify(&first.hardware_credential.device_public_key, message)
                .is_ok()
        );
        assert!(
            signature
                .verify(&second.hardware_credential.device_public_key, message)
                .is_err()
        );
        assert_eq!(
            device_public_key(&first_provider),
            device_public_key(&second_provider),
            "the devices share the same explicit diagnostic provider policy"
        );
        assert!(device_keys(1, &first).is_err());
        assert!(device_keys(0, &second).is_err());
        let mut substituted =
            core_bound_mint_recipient_material(0, release, digest(b"vk-set", 0), manifest, 1_000);
        substituted.provider_policy.provider_secret[0] ^= 1;
        assert!(device_keys(0, &substituted).is_err());
    }
}
