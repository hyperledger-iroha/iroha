//! Genuine release-pinned proving keys and native-owner-scoped proving.
//!
//! The resolver supplies untrusted original files. Fixed signed catalog roles, bounded
//! streaming authentication, canonical structured PK decoding, embedded VK equality and
//! exact compiled protocol identities admit keys. Loading them grants no device/session
//! or monetary authority. Work additionally requires the original Core selection and a
//! separately installed native witness source; public SenderReply bytes cannot supply it.
//! No key generation, experimental release, or fixture constructor is enabled here.

use super::*;
#[path = "native_outgoing_witness.rs"]
mod native_outgoing_witness;
#[path = "production_incoming.rs"]
mod production_incoming;
#[path = "production_ordinary_auxiliaries.rs"]
mod production_ordinary_auxiliaries;
#[path = "production_ordinary_guard.rs"]
mod production_ordinary_guard;
#[path = "production_ordinary_state.rs"]
mod production_ordinary_state;
pub use production_ordinary_auxiliaries::KagemushaRetainedOrdinaryBootstrapAuxiliariesV1;
pub use production_ordinary_state::{
    KagemushaOrdinaryBootstrapAuxiliaryConsumerV1, KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1,
};
#[path = "production_terminal.rs"]
mod production_terminal;
use super::super::{
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaRecursionArtifactsV1,
    KagemushaRecursiveVerifierProfileV1, native_backend,
    terminal_authorization::{
        KagemushaCommitWrapperEpCircuitV1, KagemushaCommitWrapperEqCircuitV1,
    },
    transport_decider::{
        KagemushaTransportDeciderEpCircuitV1, KagemushaTransportDeciderEqCircuitV1,
    },
};
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedBootstrapProvingSelectionV1,
    KagemushaAuthenticatedOutgoingProvingSelectionV1, KagemushaStateErrorV1,
    PersistedOutgoingCandidateV1, PreparedOutgoingCandidateV1, PreparedOutgoingRecoveryViewV1,
};
use iroha_data_model::kagemusha::{KagemushaAuthenticatedReleaseV1, KagemushaReleasePurposeV1};
pub use native_outgoing_witness::{
    KagemushaNativeOutgoingWitnessSourceV1, KagemushaNativeStateWitnessConsumerV1,
    KagemushaNativeTerminalHashWitnessConsumerV1, KagemushaNativeTerminalWitnessConsumerV1,
    register_kagemusha_native_outgoing_witness_source_v1,
};
pub use production_terminal::KagemushaProductionTerminalProofV1;

/// Immutable release and key source for a genuine native outgoing preparation.
///
/// Keys are resolved for the requested proof stage, not all retained at once. The
/// underlying maintained Halo2 prover has no measured mobile RSS guarantee here.
/// A loaded owner is not an observation lease, signature authority or payment.
///
/// ```compile_fail,E0451
/// use iroha_core_zk::kagemusha_v1_recursion::KagemushaProductionProverV1;
/// fn manufacture<R>(resolver: R) -> KagemushaProductionProverV1<R> {
///     KagemushaProductionProverV1 { resolver }
/// }
/// ```
pub struct KagemushaProductionProverV1<R> {
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    artifacts: KagemushaAuthenticatedArtifactSetV1<R>,
    profile: KagemushaRecursiveVerifierProfileV1,
    verifier: KagemushaAuthenticatedRecursiveVerifierV1,
    enabled_hardware_profiles:
        [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Authenticate this owner's exact production release, layout and verifying roles.
    /// Proving-key original streams are admitted only when their proof stage is requested.
    ///
    /// # Errors
    /// Refuses experimental, absent, stale or substituted Core custody, layout, release,
    /// verifier bytes or protocol identities. There is no caller-selected release overload.
    pub fn load(
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let release = selection.authenticated_release().map_err(owner_error)?;
        let owner = Self::from_selected_release(release, profile, resolver)?;
        owner.recheck_selection(selection)?;
        Ok(owner)
    }

    /// Authenticate proving material for the genuine original verified zero-State selection.
    /// The selection alone grants no initialized Core or physical bootstrap authorization.
    pub fn load_bootstrap(
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let owner = Self::from_selected_release(
            selection.authenticated_release().map_err(owner_error)?,
            profile,
            resolver,
        )?;
        owner.recheck_bootstrap_selection(selection)?;
        Ok(owner)
    }

    // The only callers have already selected opaque genuine native originals. This
    // constructor is private: a signed catalog by itself is not a proving admission.
    fn from_selected_release(
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        Self::from_selected_release_with_family(release, profile, resolver, false)
    }

    fn from_selected_ordinary_release(
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        Self::from_selected_release_with_family(release, profile, resolver, true)
    }

    fn from_selected_release_with_family(
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
        ordinary_state_family: bool,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        if release.purpose() != KagemushaReleasePurposeV1::Production {
            return Err(proving_error(
                "production prover refuses experimental release",
            ));
        }
        let empty = crate::kagemusha_v1_state::canonical_empty_durable_effect_digest_v1(
            release.release_id(),
        )
        .map_err(owner_error)?;
        let artifacts = KagemushaAuthenticatedArtifactSetV1::new(&release, empty, resolver)?;
        profile.validate_against_artifacts(&artifacts)?;
        let mut verifier =
            KagemushaAuthenticatedRecursiveVerifierV1::load(&artifacts, profile.clone())?;
        if ordinary_state_family {
            verifier.authorize_ordinary_monetary_release(Arc::clone(&release))
        } else {
            verifier.authorize_monetary_release(Arc::clone(&release))
        }
        .map_err(proving_error)?;
        let enabled_hardware_profiles = release_profile_table(&release)?;
        Ok(Self {
            release,
            artifacts,
            profile,
            verifier,
            enabled_hardware_profiles,
        })
    }

    /// Reauthenticate release custody and the exact original native operation selection.
    /// This check does not unseal a witness or produce monetary authority.
    pub fn recheck_selection(
        &self,
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let release = selection.authenticated_release().map_err(owner_error)?;
        self.require_release_binding(&release)?;
        selection.recheck().map_err(owner_error)
    }

    fn require_release_binding(
        &self,
        release: &KagemushaAuthenticatedReleaseV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        if release.release_id() != self.release.release_id()
            || release.attestation_digest() != self.release.attestation_digest()
            || release.authority_policy_digest() != self.release.authority_policy_digest()
            || release.native_profile_digest() != self.release.native_profile_digest()
            || release.vk_set_digest() != self.release.vk_set_digest()
        {
            return Err(proving_error(
                "prover release differs from original Core selection",
            ));
        }
        Ok(())
    }

    /// Recheck original enrollment scope and same admitted production release.
    /// Native original custody and current time remain separately checked during every proof.
    pub fn recheck_bootstrap_selection(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let release = selection.authenticated_release().map_err(owner_error)?;
        self.require_release_binding(&release)?;
        selection.recheck().map_err(owner_error)
    }

    fn recheck_bootstrap_source(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
        source: &dyn KagemushaNativeOutgoingWitnessSourceV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        self.recheck_bootstrap_selection(selection)?;
        source
            .recheck_bootstrap_originals(selection)
            .map_err(proving_error)?;
        selection
            .recheck_at_trusted_time(
                source
                    .trusted_bootstrap_now_ms(selection)
                    .map_err(proving_error)?,
            )
            .map_err(owner_error)
    }

    /// Prove the real zero-State ordered SHA claim from independently unsealed original material.
    /// The claim grants no Core or hardware authority and is consumed by `prove_bootstrap_state`.
    pub fn prove_bootstrap_state_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        self.recheck_bootstrap_source(selection, source.as_ref())?;
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let mut consumption = WitnessConsumption::default();
        let source_result =
            source.with_borrowed_bootstrap_witness(selection, None, &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_bootstrap_source(selection, source.as_ref())?;
                    validate_bootstrap_witness(
                        selection,
                        &witness.state,
                        &witness.guard_relation,
                        self.artifacts.recursion_artifacts(),
                    )?;
                    let claim =
                        prove_kagemusha_recursive_state_hash_claim_v1(&eq, &ep, witness, seed)?;
                    self.recheck_bootstrap_source(selection, source.as_ref())?;
                    Ok(claim)
                })
            });
        drop(eq);
        drop(ep);
        source_result.map_err(proving_error)?;
        self.recheck_bootstrap_source(selection, source.as_ref())?;
        consumption.finish()
    }

    /// Produce and independently verify the exact initial paired State proof using real PKs.
    /// Hardware Guard evidence, consumed issuer ceremony and INITIAL CAS remain mandatory.
    pub fn prove_bootstrap_state(
        &self,
        selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
        hash_claim: &KagemushaGeneratedMintHashClaimV1,
    ) -> Result<KagemushaGeneratedRecursiveStateProofV1, KagemushaArtifactGenerationErrorV1> {
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        self.recheck_bootstrap_source(selection, source.as_ref())?;
        let (eq, ep) = self.load_state_keys()?;
        let mut consumption = WitnessConsumption::default();
        let source_result = source.with_borrowed_bootstrap_witness(
            selection,
            Some(hash_claim),
            &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_bootstrap_source(selection, source.as_ref())?;
                    validate_bootstrap_witness(
                        selection,
                        &witness.state,
                        &witness.guard_relation,
                        self.artifacts.recursion_artifacts(),
                    )?;
                    let proof = prove_kagemusha_recursive_state_v1(&eq, &ep, witness, seed)?;
                    selection
                        .verify_state_proof(&proof.proof)
                        .map_err(owner_error)?;
                    self.recheck_bootstrap_source(selection, source.as_ref())?;
                    Ok(proof)
                })
            },
        );
        drop(eq);
        drop(ep);
        source_result.map_err(proving_error)?;
        self.recheck_bootstrap_source(selection, source.as_ref())?;
        consumption.finish()
    }

    /// Produce the genuine paired State proof from the installed original native witness.
    /// Every key role and witness claim is admitted before proving; both output parities
    /// are independently checked against this exact original prepared Core candidate.
    /// No hardware commit, balance mutation or journal publication occurs here.
    ///
    /// # Errors
    /// Rejects missing physical source, substituted private state/sealed-input/Guard/release
    /// bindings, zero/multiple callbacks, stale originals or any actual proof rejection.
    pub fn prove_outgoing_state(
        &self,
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
        hash_claim: &KagemushaGeneratedMintHashClaimV1,
    ) -> Result<KagemushaGeneratedRecursiveStateProofV1, KagemushaArtifactGenerationErrorV1> {
        self.recheck_selection(selection)?;
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        source.recheck_originals(selection).map_err(proving_error)?;
        let (eq, ep) = self.load_state_keys()?;
        self.recheck_selection(selection)?;
        let mut consumption = WitnessConsumption::default();
        let source_result = source.with_borrowed_state_witness(
            selection,
            Some(hash_claim),
            &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_selection(selection)?;
                    source.recheck_originals(selection).map_err(proving_error)?;
                    validate_prepared_witness(
                        selection.prepared().map_err(owner_error)?,
                        &witness.state,
                        &witness.guard_relation,
                        self.artifacts.recursion_artifacts(),
                    )?;
                    let proof = prove_kagemusha_recursive_state_v1(&eq, &ep, witness, seed)?;
                    let prepared = selection.prepared().map_err(owner_error)?;
                    let candidate = match prepared.recovery_view() {
                        PreparedOutgoingRecoveryViewV1::Send { .. } => {
                            PersistedOutgoingCandidateV1::verify_and_persist_send(
                                prepared.clone(),
                                proof.proof.clone(),
                                self.artifacts.recursion_artifacts(),
                                &self.verifier,
                            )
                        }
                        PreparedOutgoingRecoveryViewV1::Redemption { .. } => {
                            PersistedOutgoingCandidateV1::verify_and_persist_redemption(
                                prepared.clone(),
                                proof.proof.clone(),
                                self.artifacts.recursion_artifacts(),
                                &self.verifier,
                            )
                        }
                    }
                    .map_err(owner_error)?;
                    drop(candidate);
                    source.recheck_originals(selection).map_err(proving_error)?;
                    self.recheck_selection(selection)?;
                    Ok(proof)
                })
            },
        );
        drop(eq);
        drop(ep);
        source_result.map_err(proving_error)?;
        source.recheck_originals(selection).map_err(proving_error)?;
        self.recheck_selection(selection)?;
        consumption.finish()
    }

    /// Prove the actual ordered SHA queue needed by this original State witness.
    /// The returned claim alone grants no State, hardware or monetary authority.
    ///
    /// # Errors
    /// Applies the same original native custody/claim checks as State proving.
    pub fn prove_outgoing_state_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        self.recheck_selection(selection)?;
        let source = native_outgoing_witness::installed_source().map_err(proving_error)?;
        source.recheck_originals(selection).map_err(proving_error)?;
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let mut consumption = WitnessConsumption::default();
        let source_result =
            source.with_borrowed_state_witness(selection, None, &mut |witness, seed| {
                consumption.consume(|| {
                    self.recheck_selection(selection)?;
                    source.recheck_originals(selection).map_err(proving_error)?;
                    validate_prepared_witness(
                        selection.prepared().map_err(owner_error)?,
                        &witness.state,
                        &witness.guard_relation,
                        self.artifacts.recursion_artifacts(),
                    )?;
                    let claim =
                        prove_kagemusha_recursive_state_hash_claim_v1(&eq, &ep, witness, seed)?;
                    source.recheck_originals(selection).map_err(proving_error)?;
                    self.recheck_selection(selection)?;
                    Ok(claim)
                })
            });
        drop(eq);
        drop(ep);
        source_result.map_err(proving_error)?;
        source.recheck_originals(selection).map_err(proving_error)?;
        self.recheck_selection(selection)?;
        consumption.finish()
    }

    fn load_state_keys(
        &self,
    ) -> Result<
        (
            KagemushaLoadedEqRecursiveStateArtifactsV1,
            KagemushaLoadedEpRecursiveStateArtifactsV1,
        ),
        KagemushaArtifactGenerationErrorV1,
    > {
        // The inner identities are derived by the authenticated verifier from the signed
        // inner VK roles and profile. They are distinct from the public transport protocols.
        let checkpoint = self.verifier.state_checkpoint_material();
        // Use the exact family admitted by the held Native verifier, including the
        // distinct ordinary Guard roles. The generic byte resolver also retains the
        // experimental OEM helper catalog and cannot select this State family.
        let release = checkpoint.artifacts;
        let eq_parameters = self.artifacts.load_eq_params()?;
        let eq_vk_bytes = self.artifacts.resolve(KagemushaArtifactRoleV1::StateVkEq)?;
        let eq_vk = native_backend::read_eq_state_vk(&eq_vk_bytes, self.profile.state_eq.clone())?;
        let eq_pk =
            load_authenticated_proving_key_v1::<EqAffine, KagemushaTransportDeciderEqCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::StatePkEq,
                KagemushaPastaParityV1::Eq,
                self.profile.state_eq.clone(),
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Eq, &eq_pk, &eq_vk_bytes)?;
        let eq_inner_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::InnerStateVkEq)?;
        let eq_inner_vk = native_backend::read_eq_inner_state_vk(
            &eq_inner_vk_bytes,
            self.profile.inner_state_eq.clone(),
        )?;
        let eq_inner_pk =
            load_authenticated_proving_key_v1::<EqAffine, KagemushaRecursiveStateEqCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::InnerStatePkEq,
                KagemushaPastaParityV1::Eq,
                self.profile.inner_state_eq.clone(),
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Eq, &eq_inner_pk, &eq_inner_vk_bytes)?;
        let eq_protocol = compile(
            &eq_parameters,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        let eq_inner_protocol = compile(
            &eq_parameters,
            &eq_inner_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        require_protocol(
            &eq_protocol,
            KagemushaPastaParityV1::Eq,
            release.eq_protocol_digest,
            true,
        )?;
        require_protocol(
            &eq_inner_protocol,
            KagemushaPastaParityV1::Eq,
            checkpoint.binding.inner_eq_protocol_digest,
            false,
        )?;
        let ep_parameters = self.artifacts.load_ep_params()?;
        let ep_vk_bytes = self.artifacts.resolve(KagemushaArtifactRoleV1::StateVkEp)?;
        let ep_vk = native_backend::read_ep_state_vk(&ep_vk_bytes, self.profile.state_ep.clone())?;
        let ep_pk =
            load_authenticated_proving_key_v1::<EpAffine, KagemushaTransportDeciderEpCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::StatePkEp,
                KagemushaPastaParityV1::Ep,
                self.profile.state_ep.clone(),
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Ep, &ep_pk, &ep_vk_bytes)?;
        let ep_inner_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::InnerStateVkEp)?;
        let ep_inner_vk = native_backend::read_ep_inner_state_vk(
            &ep_inner_vk_bytes,
            self.profile.inner_state_ep.clone(),
        )?;
        let ep_inner_pk =
            load_authenticated_proving_key_v1::<EpAffine, KagemushaRecursiveStateEpCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::InnerStatePkEp,
                KagemushaPastaParityV1::Ep,
                self.profile.inner_state_ep.clone(),
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Ep, &ep_inner_pk, &ep_inner_vk_bytes)?;
        let ep_protocol = compile(
            &ep_parameters,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        let ep_inner_protocol = compile(
            &ep_parameters,
            &ep_inner_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        require_protocol(
            &ep_protocol,
            KagemushaPastaParityV1::Ep,
            release.ep_protocol_digest,
            true,
        )?;
        require_protocol(
            &ep_inner_protocol,
            KagemushaPastaParityV1::Ep,
            checkpoint.binding.inner_ep_protocol_digest,
            false,
        )?;
        Ok((
            KagemushaLoadedEqRecursiveStateArtifactsV1 {
                release_id: release.release_id,
                suite_id: self.artifacts.suite_id(),
                vk_digest: self.artifacts.vk_set_digest(),
                parameters: eq_parameters,
                proving_key: eq_pk,
                verifying_key: eq_vk,
                circuit_params: self.profile.state_eq.clone(),
                inner_proving_key: eq_inner_pk,
                inner_verifying_key: eq_inner_vk,
                inner_circuit_params: self.profile.inner_state_eq.clone(),
            },
            KagemushaLoadedEpRecursiveStateArtifactsV1 {
                release_id: release.release_id,
                suite_id: self.artifacts.suite_id(),
                vk_digest: self.artifacts.vk_set_digest(),
                parameters: ep_parameters,
                proving_key: ep_pk,
                verifying_key: ep_vk,
                circuit_params: self.profile.state_ep.clone(),
                inner_proving_key: ep_inner_pk,
                inner_verifying_key: ep_inner_vk,
                inner_circuit_params: self.profile.inner_state_ep.clone(),
            },
        ))
    }

    fn load_terminal_keys(
        &self,
    ) -> Result<
        (
            KagemushaLoadedEqTerminalAuthorizationArtifactsV1,
            KagemushaLoadedEpTerminalAuthorizationArtifactsV1,
        ),
        KagemushaArtifactGenerationErrorV1,
    > {
        let release = self.artifacts.recursion_artifacts();
        let eq_parameters = self.artifacts.load_eq_params()?;
        let eq_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEq)?;
        let eq_vk = native_backend::read_eq_terminal_authorization_vk(
            &eq_vk_bytes,
            self.profile.terminal_authorization_eq.clone(),
        )?;
        let eq_pk = load_authenticated_proving_key_v1::<
            EqAffine,
            KagemushaTerminalAuthorizationEqCircuitV1,
            _,
        >(
            &self.artifacts,
            KagemushaArtifactRoleV1::TerminalAuthorizationPkEq,
            KagemushaPastaParityV1::Eq,
            self.profile.terminal_authorization_eq.clone(),
        )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Eq, &eq_pk, &eq_vk_bytes)?;
        let eq_protocol = compile(
            &eq_parameters,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        require_protocol(
            &eq_protocol,
            KagemushaPastaParityV1::Eq,
            release.terminal_authorization_eq_protocol_digest,
            false,
        )?;
        let ep_parameters = self.artifacts.load_ep_params()?;
        let ep_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::TerminalAuthorizationVkEp)?;
        let ep_vk = native_backend::read_ep_terminal_authorization_vk(
            &ep_vk_bytes,
            self.profile.terminal_authorization_ep.clone(),
        )?;
        let ep_pk = load_authenticated_proving_key_v1::<
            EpAffine,
            KagemushaTerminalAuthorizationEpCircuitV1,
            _,
        >(
            &self.artifacts,
            KagemushaArtifactRoleV1::TerminalAuthorizationPkEp,
            KagemushaPastaParityV1::Ep,
            self.profile.terminal_authorization_ep.clone(),
        )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Ep, &ep_pk, &ep_vk_bytes)?;
        let ep_protocol = compile(
            &ep_parameters,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        require_protocol(
            &ep_protocol,
            KagemushaPastaParityV1::Ep,
            release.terminal_authorization_ep_protocol_digest,
            false,
        )?;
        Ok((
            KagemushaLoadedEqTerminalAuthorizationArtifactsV1 {
                parameters: eq_parameters,
                proving_key: eq_pk,
                verifying_key: eq_vk,
                circuit_params: self.profile.terminal_authorization_eq.clone(),
                protocol_digest: release.terminal_authorization_eq_protocol_digest,
                release_id: release.release_id,
                profile_digest: release.profile_digest,
                artifact_manifest_digest: release.artifact_manifest_digest,
                suite_id: self.artifacts.suite_id(),
                vk_digest: self.artifacts.vk_set_digest(),
                eq_claim_protocol_digest: self.profile.mint_hash_claim_eq_protocol_digest,
                ep_claim_protocol_digest: self.profile.mint_hash_claim_ep_protocol_digest,
                eq_shard_protocol_digest: self.profile.mint_hash_shard_eq_protocol_digest,
                ep_shard_protocol_digest: self.profile.mint_hash_shard_ep_protocol_digest,
                enabled_hardware_profiles: self.enabled_hardware_profiles,
            },
            KagemushaLoadedEpTerminalAuthorizationArtifactsV1 {
                parameters: ep_parameters,
                proving_key: ep_pk,
                verifying_key: ep_vk,
                circuit_params: self.profile.terminal_authorization_ep.clone(),
                protocol_digest: release.terminal_authorization_ep_protocol_digest,
                release_id: release.release_id,
                profile_digest: release.profile_digest,
                artifact_manifest_digest: release.artifact_manifest_digest,
                suite_id: self.artifacts.suite_id(),
                vk_digest: self.artifacts.vk_set_digest(),
                eq_claim_protocol_digest: self.profile.mint_hash_claim_eq_protocol_digest,
                ep_claim_protocol_digest: self.profile.mint_hash_claim_ep_protocol_digest,
                eq_shard_protocol_digest: self.profile.mint_hash_shard_eq_protocol_digest,
                ep_shard_protocol_digest: self.profile.mint_hash_shard_ep_protocol_digest,
                enabled_hardware_profiles: self.enabled_hardware_profiles,
            },
        ))
    }

    fn load_wrapper_keys(
        &self,
    ) -> Result<
        (
            KagemushaLoadedEqCommitWrapperArtifactsV1,
            KagemushaLoadedEpCommitWrapperArtifactsV1,
        ),
        KagemushaArtifactGenerationErrorV1,
    > {
        let release = self.artifacts.recursion_artifacts();
        let eq_parameters = self.artifacts.load_eq_params()?;
        let eq_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::CommitWrapperVkEq)?;
        let eq_vk = native_backend::read_eq_commit_wrapper_vk(
            &eq_vk_bytes,
            self.profile.commit_wrapper_eq.clone(),
        )?;
        let eq_pk =
            load_authenticated_proving_key_v1::<EqAffine, KagemushaCommitWrapperEqCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::CommitWrapperPkEq,
                KagemushaPastaParityV1::Eq,
                self.profile.commit_wrapper_eq.clone(),
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Eq, &eq_pk, &eq_vk_bytes)?;
        let eq_protocol = compile(
            &eq_parameters,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        require_protocol(
            &eq_protocol,
            KagemushaPastaParityV1::Eq,
            release.commit_wrapper_eq_protocol_digest,
            true,
        )?;
        let ep_parameters = self.artifacts.load_ep_params()?;
        let ep_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::CommitWrapperVkEp)?;
        let ep_vk = native_backend::read_ep_commit_wrapper_vk(
            &ep_vk_bytes,
            self.profile.commit_wrapper_ep.clone(),
        )?;
        let ep_pk =
            load_authenticated_proving_key_v1::<EpAffine, KagemushaCommitWrapperEpCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::CommitWrapperPkEp,
                KagemushaPastaParityV1::Ep,
                self.profile.commit_wrapper_ep.clone(),
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Ep, &ep_pk, &ep_vk_bytes)?;
        let ep_protocol = compile(
            &ep_parameters,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]),
        );
        require_protocol(
            &ep_protocol,
            KagemushaPastaParityV1::Ep,
            release.commit_wrapper_ep_protocol_digest,
            true,
        )?;
        Ok((
            KagemushaLoadedEqCommitWrapperArtifactsV1 {
                parameters: eq_parameters,
                proving_key: eq_pk,
                verifying_key: eq_vk,
                circuit_params: self.profile.commit_wrapper_eq.clone(),
                protocol_digest: release.commit_wrapper_eq_protocol_digest,
                terminal_authorization_protocol_digest: release
                    .terminal_authorization_eq_protocol_digest,
                release_id: release.release_id,
                profile_digest: release.profile_digest,
                artifact_manifest_digest: release.artifact_manifest_digest,
                suite_id: self.artifacts.suite_id(),
                vk_digest: self.artifacts.vk_set_digest(),
                enabled_hardware_profiles: self.enabled_hardware_profiles,
            },
            KagemushaLoadedEpCommitWrapperArtifactsV1 {
                parameters: ep_parameters,
                proving_key: ep_pk,
                verifying_key: ep_vk,
                circuit_params: self.profile.commit_wrapper_ep.clone(),
                protocol_digest: release.commit_wrapper_ep_protocol_digest,
                terminal_authorization_protocol_digest: release
                    .terminal_authorization_ep_protocol_digest,
                release_id: release.release_id,
                profile_digest: release.profile_digest,
                artifact_manifest_digest: release.artifact_manifest_digest,
                suite_id: self.artifacts.suite_id(),
                vk_digest: self.artifacts.vk_set_digest(),
                enabled_hardware_profiles: self.enabled_hardware_profiles,
            },
        ))
    }
}

fn release_profile_table(
    release: &KagemushaAuthenticatedReleaseV1,
) -> Result<
    [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    KagemushaArtifactGenerationErrorV1,
> {
    let ids: Vec<_> = release
        .enabled_profiles()
        .iter()
        .map(|profile| profile.hardware_profile_id)
        .collect();
    profile_table(&ids)
}

fn profile_table(
    ids: &[[u8; 32]],
) -> Result<
    [[u8; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    KagemushaArtifactGenerationErrorV1,
> {
    if ids.is_empty()
        || ids.len() > KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1
        || ids.contains(&[0; 32])
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(proving_error(
            "release profile table is not a bounded sorted nonzero set",
        ));
    }
    let mut table = [[0; 32]; KAGEMUSHA_TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
    table[..ids.len()].copy_from_slice(ids);
    Ok(table)
}

fn require_protocol<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaParityV1,
    expected: [u8; 32],
    transported: bool,
) -> Result<(), KagemushaArtifactGenerationErrorV1>
where
    C: snark_verifier::util::arithmetic::CurveAffine,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    let actual = native_parent_protocol_digest_v1(protocol, parity).map_err(proving_error)?;
    require_protocol_digest(expected, actual)?;
    if transported {
        validate_transport_protocol_profile(parity, "released production proof", protocol)?;
    } else {
        ordinary_ipa_proof_profile_v1(protocol).map_err(proving_error)?;
    }
    Ok(())
}

fn require_protocol_digest(
    expected: [u8; 32],
    actual: [u8; 32],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if expected == [0; 32] || actual != expected {
        return Err(proving_error(
            "compiled proving protocol differs from exact signed role",
        ));
    }
    Ok(())
}

fn owner_error(error: KagemushaStateErrorV1) -> KagemushaArtifactGenerationErrorV1 {
    proving_error(format!("original native proving custody rejected: {error}"))
}
fn proving_error(reason: impl Into<String>) -> KagemushaArtifactGenerationErrorV1 {
    KagemushaArtifactGenerationErrorV1::CircuitBuild(reason.into())
}

fn validate_prepared_witness(
    prepared: &PreparedOutgoingCandidateV1,
    state: &KagemushaStateRelationWitnessV1,
    guard: &KagemushaGuardBundleRelationWitnessV1,
    artifacts: super::super::KagemushaRecursionArtifactsV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    state.validate().map_err(proving_error)?;
    guard.validate().map_err(proving_error)?;
    let (before, after) = prepared.private_state_link();
    let statement = &prepared.proof_statement;
    if state.predecessor.as_ref() != Some(before)
        || &state.successor != after
        || state.operation != KagemushaOperationV1::from(statement.kind)
        || state.amount != statement.amount
        || state.journal_revision_before != statement.journal_revision_before
        || state.journal_revision_after != statement.journal_revision_after
        || state.transition_effect_digest != statement.effect_digest
        || state.mint_finality_semantic_digest != statement.mint_finality_semantic_digest
        || state.mint_finality_proof_binding_digest != statement.mint_finality_proof_binding_digest
        || state.peer_credit_id != statement.peer_credit_id
        || state.recipient_encryption_key_binding != statement.recipient_encryption_key_binding
        || state.lifecycle_binding_digest != statement.lifecycle_binding_digest
        || state.prepared_transition_binding_digest != statement.prepared_transition_binding_digest
        || state.prepared_intent != Some(prepared.prepared_intent_commitments())
        || state.receive_credit_binding_digest != statement.receive_credit_binding_digest
        || state.transport_semantic_digest != prepared.semantic_digest().map_err(owner_error)?
        || state.guard_statement_digest != prepared.normalized_guard_statement_digest
        || guard.statement_digest() != prepared.normalized_guard_statement_digest
        || guard.canonical_empty_effect_digest != artifacts.canonical_empty_effect_digest
        || state.eq_protocol_digest != artifacts.eq_protocol_digest
        || state.ep_protocol_digest != artifacts.ep_protocol_digest
        || state.guard_eq_protocol_digest
            != artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|error| proving_error(error.to_string()))?
        || state.guard_ep_protocol_digest
            != artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|error| proving_error(error.to_string()))?
        || state.mint_eq_protocol_digest
            != artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|error| proving_error(error.to_string()))?
        || state.mint_ep_protocol_digest
            != artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|error| proving_error(error.to_string()))?
        || state.mint_authorization_eq_protocol_digest
            != artifacts.mint_authorization_eq_protocol_digest
        || state.mint_authorization_ep_protocol_digest
            != artifacts.mint_authorization_ep_protocol_digest
        || state.commit_wrapper_eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
        || state.commit_wrapper_ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
    {
        return Err(proving_error(
            "unsealed witness differs from original native Prepared claim",
        ));
    }
    Ok(())
}

fn validate_bootstrap_witness(
    selection: &KagemushaAuthenticatedBootstrapProvingSelectionV1<'_>,
    state: &KagemushaStateRelationWitnessV1,
    guard: &KagemushaGuardBundleRelationWitnessV1,
    artifacts: KagemushaRecursionArtifactsV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    selection.recheck().map_err(owner_error)?;
    state.validate().map_err(proving_error)?;
    guard.validate().map_err(proving_error)?;
    let preview = selection.preview().map_err(owner_error)?;
    if state.operation != KagemushaOperationV1::Bootstrap
        || state.predecessor.is_some()
        || state.successor != preview.state
        || state.amount != 0
        || state.journal_revision_before != 0
        || state.journal_revision_after != 0
        || state.transport_semantic_digest != preview.transport_semantic_digest
        || guard.statement != preview.normalized_guard_statement
        || state.guard_statement_digest
            != guard
                .statement
                .canonical_digest()
                .map_err(|error| proving_error(error.to_string()))?
        || state.transition_effect_digest != guard.statement.transition_effect_digest
        || state.eq_protocol_digest != artifacts.eq_protocol_digest
        || state.ep_protocol_digest != artifacts.ep_protocol_digest
        || state.guard_eq_protocol_digest
            != artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|e| proving_error(e.to_string()))?
        || state.guard_ep_protocol_digest
            != artifacts
                .guard_bundle_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|e| proving_error(e.to_string()))?
        || state.mint_eq_protocol_digest
            != artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Eq)
                .map_err(|e| proving_error(e.to_string()))?
        || state.mint_ep_protocol_digest
            != artifacts
                .mint_finality_protocol_digest(KagemushaPastaParityV1::Ep)
                .map_err(|e| proving_error(e.to_string()))?
        || state.mint_authorization_eq_protocol_digest
            != artifacts.mint_authorization_eq_protocol_digest
        || state.mint_authorization_ep_protocol_digest
            != artifacts.mint_authorization_ep_protocol_digest
        || state.commit_wrapper_eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
        || state.commit_wrapper_ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
    {
        return Err(proving_error(
            "zero witness differs from actual verified bootstrap selection",
        ));
    }
    Ok(())
}

struct WitnessConsumption<T> {
    entered: bool,
    repeated: bool,
    result: Option<Result<T, KagemushaArtifactGenerationErrorV1>>,
}
impl<T> Default for WitnessConsumption<T> {
    fn default() -> Self {
        Self {
            entered: false,
            repeated: false,
            result: None,
        }
    }
}
impl<T> WitnessConsumption<T> {
    fn consume(
        &mut self,
        work: impl FnOnce() -> Result<T, KagemushaArtifactGenerationErrorV1>,
    ) -> Result<(), String> {
        if self.entered {
            self.repeated = true;
            return Err("native witness callback repeated".to_owned());
        }
        self.entered = true;
        let result = work();
        let status = result.as_ref().map(|_| ()).map_err(ToString::to_string);
        self.result = Some(result);
        status
    }
    fn finish(self) -> Result<T, KagemushaArtifactGenerationErrorV1> {
        if !self.entered || self.repeated {
            return Err(proving_error("native witness callback missing or repeated"));
        }
        self.result
            .ok_or_else(|| proving_error("native witness callback produced no result"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_profiles_are_exact_sorted_prefix_without_repair() {
        let table = profile_table(&[[1; 32], [2; 32]]).unwrap();
        assert_eq!(&table[..2], &[[1; 32], [2; 32]]);
        assert!(table[2..].iter().all(|id| *id == [0; 32]));
        for ids in [
            vec![],
            vec![[0; 32]],
            vec![[2; 32], [1; 32]],
            vec![[1; 32], [1; 32]],
            vec![[1; 32]; 65],
        ] {
            assert!(profile_table(&ids).is_err());
        }
    }
    #[test]
    fn full_release_profile_table_does_not_truncate() {
        let ids: Vec<_> = (1u8..=64).map(|byte| [byte; 32]).collect();
        assert_eq!(profile_table(&ids).unwrap().as_slice(), ids.as_slice());
        let mut oversized = ids;
        oversized.push([65; 32]);
        assert!(profile_table(&oversized).is_err());
    }
    #[test]
    fn exact_signed_protocol_rejects_substitution_and_zero() {
        require_protocol_digest([1; 32], [1; 32]).unwrap();
        assert!(require_protocol_digest([1; 32], [2; 32]).is_err());
        assert!(require_protocol_digest([0; 32], [0; 32]).is_err());
    }
    #[test]
    fn missing_physical_callback_cannot_return_proof() {
        assert!(WitnessConsumption::<u8>::default().finish().is_err());
    }
    #[test]
    fn swallowed_native_callback_failure_stays_rejected() {
        let mut c = WitnessConsumption::<u8>::default();
        let _ = c.consume(|| Err(proving_error("original custody changed")));
        assert!(c.finish().is_err());
    }
    #[test]
    fn repeated_callback_poisoned_even_after_success() {
        let mut c = WitnessConsumption::default();
        c.consume(|| Ok(7u8)).unwrap();
        let _ = c.consume(|| Ok(8));
        assert!(c.finish().is_err());
    }
    #[test]
    fn successful_native_callback_returns_only_owned_result() {
        let mut c = WitnessConsumption::default();
        c.consume(|| Ok(7u8)).unwrap();
        assert_eq!(c.finish().unwrap(), 7);
    }
}
