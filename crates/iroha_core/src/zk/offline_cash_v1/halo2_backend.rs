//! First-party Offline Cash V1 STATE proof verifier.
//!
//! The backend admits only threshold-authenticated release artifacts, parses
//! transparent k=16 Pasta parameters and processed verifier keys against the
//! exact Eq/Fp and Ep/Fq STATE circuit types, verifies the complete typed
//! 229-word public instance plus its parity-local carried-lineage column, and
//! terminally decides both the ordinary Poseidon proof accumulator and the
//! circuit-bound lineage. Construction drops the
//! artifact source after parsing, so peer verification performs no network or
//! artifact fetch.
//!
//! Generic artifact sources are deliberately not production credit authority.
//! `authorize_verified_credit` remains fail-closed for those sources; only the
//! Core-owned complete 34-file installer can grant it after threshold release
//! authentication and reviewed qualification evidence. A secure-device
//! lifecycle boundary is still required separately before wallet mutation.

use std::{fmt, sync::Arc};

use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::VerifyingKey,
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, OfflineCashArtifactBindingV1,
    OfflineCashArtifactRoleV1, OfflineCashAuthenticatedReleaseV1, OfflineCashIpaLineageV1,
};

use super::{
    OfflineCashAuthenticatedArtifactFileSetV1, OfflineCashAuthenticatedVerifierArtifactsV1,
    OfflineCashHalo2ArtifactErrorV1, OfflineCashHalo2ArtifactManifestV1,
    OfflineCashHalo2ArtifactSourceV1, OfflineCashHalo2ParityV1, OfflineCashPairedProofVerifierV1,
    halo2_primitives::{
        parse_offline_cash_ep_params_v1, parse_offline_cash_eq_params_v1,
        parse_processed_verifier_key_v1,
    },
    helper_recursion::{
        offline_cash_ep_lineage_instance_column_v1, offline_cash_eq_lineage_instance_column_v1,
        terminal_verify_ep_outer_and_carried_v1, terminal_verify_eq_outer_and_carried_v1,
    },
    state_abi::OfflineCashStatePublicInstancesV1,
    state_recursion::{OfflineCashEpStateCircuitV1, OfflineCashEqStateCircuitV1},
};

const PRODUCTION_ACTIVATION_BLOCKER_V1: &str = "offline-cash production activation remains blocked until a governed signed ABI22/Kagemusha V4 release authenticates the complete 34-artifact STATE/GuardBundle/helper/P-256 inventory and supplies reviewed proof-shape qualification, reproducible-build, and secure-device evidence";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OfflineCashCreditAuthorityV1 {
    Blocked,
    CompleteAuthenticatedFileSet,
}

/// Parsed, authenticated first-party STATE verifier.
pub(crate) struct OfflineCashHalo2VerifierBackendV1 {
    manifest: OfflineCashHalo2ArtifactManifestV1,
    eq_params: ParamsIPA<EqAffine>,
    eq_verifying_key: VerifyingKey<EqAffine>,
    ep_params: ParamsIPA<EpAffine>,
    ep_verifying_key: VerifyingKey<EpAffine>,
    credit_authority: OfflineCashCreditAuthorityV1,
}

impl fmt::Debug for OfflineCashHalo2VerifierBackendV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashHalo2VerifierBackendV1")
            .field("manifest", &self.manifest)
            .field("state_verification", &"available")
            .field("credit_authority", &self.credit_authority)
            .finish()
    }
}

impl OfflineCashHalo2VerifierBackendV1 {
    /// Authenticate and parse the required Eq/Ep parameters and STATE verifier keys.
    ///
    /// # Errors
    ///
    /// Returns an error before constructing a backend if the release, artifact
    /// bytes, transparent parameters, or processed keys differ from the exact
    /// compiled profile.
    pub(crate) fn from_artifact_source(
        source: Arc<dyn OfflineCashHalo2ArtifactSourceV1>,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1> {
        let artifacts = OfflineCashAuthenticatedVerifierArtifactsV1::load(source)?;
        let manifest = artifacts.manifest();

        let eq_binding = manifest.artifact(OfflineCashArtifactRoleV1::StateVkEq);
        let (eq_params_bytes, eq_verifying_key_bytes) = artifacts.load_state_verifier_bytes(
            OfflineCashHalo2ParityV1::Eq,
            eq_binding,
            manifest.state_protocol_digest(OfflineCashHalo2ParityV1::Eq),
        )?;
        let eq_params = parse_offline_cash_eq_params_v1(&eq_params_bytes)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidParameterArtifact)?;
        drop(eq_params_bytes);
        let eq_verifying_key = parse_processed_verifier_key_v1::<
            EqAffine,
            OfflineCashEqStateCircuitV1,
        >(&eq_verifying_key_bytes, OFFLINE_CASH_HALO2_K_V1)
        .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidVerifierKeyArtifact)?;
        drop(eq_verifying_key_bytes);

        let ep_binding = manifest.artifact(OfflineCashArtifactRoleV1::StateVkEp);
        let (ep_params_bytes, ep_verifying_key_bytes) = artifacts.load_state_verifier_bytes(
            OfflineCashHalo2ParityV1::Ep,
            ep_binding,
            manifest.state_protocol_digest(OfflineCashHalo2ParityV1::Ep),
        )?;
        let ep_params = parse_offline_cash_ep_params_v1(&ep_params_bytes)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidParameterArtifact)?;
        drop(ep_params_bytes);
        let ep_verifying_key = parse_processed_verifier_key_v1::<
            EpAffine,
            OfflineCashEpStateCircuitV1,
        >(&ep_verifying_key_bytes, OFFLINE_CASH_HALO2_K_V1)
        .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidVerifierKeyArtifact)?;
        drop(ep_verifying_key_bytes);

        // Verification must remain network- and artifact-fetch-free. Retain
        // only the authenticated manifest and parsed cryptographic material;
        // dropping `artifacts` also drops the source boundary before this
        // backend can be used by a peer handoff.
        let manifest = manifest.clone();
        drop(artifacts);

        Ok(Self {
            manifest,
            eq_params,
            eq_verifying_key,
            ep_params,
            ep_verifying_key,
            credit_authority: OfflineCashCreditAuthorityV1::Blocked,
        })
    }

    /// Parse a file set which has already passed the complete 34-role
    /// threshold-authenticated installation boundary.
    ///
    /// This is the only constructor that can grant terminal credit authority.
    /// Generic test/source implementations remain cryptographically useful but
    /// fail closed at `authorize_verified_credit`.
    pub(super) fn from_authenticated_file_set(
        source: OfflineCashAuthenticatedArtifactFileSetV1,
    ) -> Result<(Arc<OfflineCashAuthenticatedReleaseV1>, Self), OfflineCashHalo2ArtifactErrorV1>
    {
        let release = source.authenticated_release_arc();
        let mut backend = Self::from_artifact_source(Arc::new(source))?;
        backend.credit_authority = OfflineCashCreditAuthorityV1::CompleteAuthenticatedFileSet;
        Ok((release, backend))
    }

    /// Exact authenticated artifact manifest retained by this backend.
    pub(crate) const fn artifact_manifest(&self) -> &OfflineCashHalo2ArtifactManifestV1 {
        &self.manifest
    }

    fn authenticate_call(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(), String> {
        let verifier_role = match parity {
            OfflineCashHalo2ParityV1::Eq => OfflineCashArtifactRoleV1::StateVkEq,
            OfflineCashHalo2ParityV1::Ep => OfflineCashArtifactRoleV1::StateVkEp,
        };
        if verifying_key.role != verifier_role
            || verifying_key != self.manifest.artifact(verifier_role)
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest.to_string());
        }
        if protocol_digest != self.manifest.state_protocol_digest(parity) {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch.to_string());
        }
        Ok(())
    }

    fn verify_eq_state(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        public_instances: &OfflineCashStatePublicInstancesV1,
        proof: &[u8],
        carried_lineage: &OfflineCashIpaLineageV1,
    ) -> Result<(), String> {
        if public_instances.parity() != OfflineCashHalo2ParityV1::Eq
            || proof.is_empty()
            || proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        {
            return Err("invalid offline-cash Eq STATE proof shape".to_owned());
        }
        self.authenticate_call(OfflineCashHalo2ParityV1::Eq, verifying_key, protocol_digest)?;
        let state_column = public_instances.field_instances::<Fp>().to_vec();
        let lineage_column = offline_cash_eq_lineage_instance_column_v1(carried_lineage)
            .map_err(|error| format!("invalid Eq carried lineage: {error:?}"))?
            .to_vec();
        let instances = vec![state_column, lineage_column];
        terminal_verify_eq_outer_and_carried_v1(
            &self.eq_params,
            &self.eq_verifying_key,
            &instances,
            proof,
            OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
            carried_lineage,
        )
        .map_err(|error| format!("Eq outer/carried terminal verification failed: {error:?}"))
    }

    fn verify_ep_state(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        public_instances: &OfflineCashStatePublicInstancesV1,
        proof: &[u8],
        carried_lineage: &OfflineCashIpaLineageV1,
    ) -> Result<(), String> {
        if public_instances.parity() != OfflineCashHalo2ParityV1::Ep
            || proof.is_empty()
            || proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
        {
            return Err("invalid offline-cash Ep STATE proof shape".to_owned());
        }
        self.authenticate_call(OfflineCashHalo2ParityV1::Ep, verifying_key, protocol_digest)?;
        let state_column = public_instances.field_instances::<Fq>().to_vec();
        let lineage_column = offline_cash_ep_lineage_instance_column_v1(carried_lineage)
            .map_err(|error| format!("invalid Ep carried lineage: {error:?}"))?
            .to_vec();
        let instances = vec![state_column, lineage_column];
        terminal_verify_ep_outer_and_carried_v1(
            &self.ep_params,
            &self.ep_verifying_key,
            &instances,
            proof,
            OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
            carried_lineage,
        )
        .map_err(|error| format!("Ep outer/carried terminal verification failed: {error:?}"))
    }
}

impl super::paired_verifier_sealed::Sealed for OfflineCashHalo2VerifierBackendV1 {}

impl OfflineCashPairedProofVerifierV1 for OfflineCashHalo2VerifierBackendV1 {
    fn verify_eq_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        public_instances: &OfflineCashStatePublicInstancesV1,
        proof: &[u8],
        carried_lineage: &OfflineCashIpaLineageV1,
    ) -> Result<(), String> {
        self.verify_eq_state(
            verifying_key,
            protocol_digest,
            public_instances,
            proof,
            carried_lineage,
        )
    }

    fn verify_ep_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        public_instances: &OfflineCashStatePublicInstancesV1,
        proof: &[u8],
        carried_lineage: &OfflineCashIpaLineageV1,
    ) -> Result<(), String> {
        self.verify_ep_state(
            verifying_key,
            protocol_digest,
            public_instances,
            proof,
            carried_lineage,
        )
    }

    fn authorize_verified_credit(&self) -> Result<(), String> {
        match self.credit_authority {
            OfflineCashCreditAuthorityV1::Blocked => {
                Err(PRODUCTION_ACTIVATION_BLOCKER_V1.to_owned())
            }
            OfflineCashCreditAuthorityV1::CompleteAuthenticatedFileSet => Ok(()),
        }
    }
}
