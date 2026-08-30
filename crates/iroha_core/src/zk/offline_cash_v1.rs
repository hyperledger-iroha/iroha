//! Terminal admission boundary for the clean offline-cash V1 paired proof.

/// Authenticated Halo2 artifact-provider boundary.
mod artifacts;
/// Developer-only generation of an unauthenticated complete artifact candidate.
#[cfg(feature = "dev-tools")]
mod dev_artifact_generator;
/// Dedicated reciprocal GuardBundle circuits over authenticated helper/P256 children.
mod guard_bundle_recursion;
/// Authenticated first-party Halo2 STATE verifier with a fail-closed production authority gate.
mod halo2_backend;
/// Strict Pasta IPA artifact and ordinary Poseidon-proof primitives.
mod halo2_primitives;
/// Exact 184-word / 27-cell field-neutral helper public ABI.
mod helper_abi;
/// Fixed Eq/Fp and Ep/Fq helper public-binding circuit scaffolds.
mod helper_circuit;
/// Strict carried-lineage codec and terminal outer-plus-lineage decisions.
mod helper_recursion;
/// Private move-only GuardUse, PlatformBind, KeyCert, and bundle relation.
mod helper_relation;
/// Private common-k16 dual-lane packed affine P-256 child circuit.
mod p256_packed_affine_v3;
/// Eight-column current-row compiler for the compact final recursive wrappers.
mod packed_base;
/// Exact Offline Cash V1 Halo2 profile and protocol identities.
mod protocol;
/// Public fail-closed wallet-runtime boundary.
mod runtime_facade;
/// Exact 229-word / 33-cell field-neutral STATE public-instance ABI.
mod state_abi;
/// Exact Eq/Fp and Ep/Fq STATE relation circuit scaffolds.
mod state_circuit;
/// Final reciprocal State wrappers over StateLeaf and completed GuardBundle.
mod state_recursion;
/// Private canonical balance/credit head and conservation relation.
mod state_relation;
/// Private fixed-geometry SHA-256 bridge used only by the STATE relation.
mod state_sha;
/// Private move-only balance, pending-request, credit, and hardware-guard state machine.
pub(crate) mod state_transition;

pub use artifacts::{
    OfflineCashArtifactFileSetErrorV1, OfflineCashAuthenticatedArtifactFileSetV1,
    OfflineCashHalo2ArtifactErrorV1,
};
pub(crate) use artifacts::{
    OfflineCashAuthenticatedVerifierArtifactsV1, OfflineCashHalo2ArtifactManifestV1,
    OfflineCashHalo2ArtifactSourceV1,
};
#[cfg(feature = "dev-tools")]
pub use dev_artifact_generator::{
    OfflineCashGeneratedArtifactSetV1, OfflineCashGeneratedArtifactSpoolV1,
    generate_offline_cash_artifacts_v1, offline_cash_artifact_file_name_v1,
    offline_cash_artifact_profile_digest_v1, offline_cash_artifact_protocol_digest_v1,
};
pub(crate) use protocol::OfflineCashHalo2ParityV1;
#[cfg(test)]
pub(crate) use protocol::{
    OfflineCashHalo2CircuitRoleV1, offline_cash_halo2_profile_digest_v1,
    offline_cash_halo2_protocol_identity_v1,
};
pub use runtime_facade::{
    OfflineCashWalletSessionActionV1, OfflineCashWalletSessionErrorV1,
    OfflineCashWalletSessionStateV1, OfflineCashWalletSessionStatusV1, OfflineCashWalletSessionV1,
};

/// Exact move-only statement source accepted by the private packed P-256 V3 bridge.
///
/// This outer trait is the only V3 source surface visible to sibling modules. The
/// nested circuit, its raw constructor, and its diagnostic types remain private
/// to this module.
pub(super) trait P256PackedStatementSourceV3 {
    /// Fill the complete `[SEC1 | SHA-256 prehash | P1363 signature]` frame once.
    fn read_exact_statement(&mut self, destination: &mut [u8; 161]) -> Result<(), &'static str>;
}

struct P256PackedStatementSourceAdapterV3<S>(S);

impl<S: P256PackedStatementSourceV3> p256_packed_affine_v3::P256PackedStatementSourceV3
    for P256PackedStatementSourceAdapterV3<S>
{
    fn read_exact_statement(&mut self, destination: &mut [u8; 161]) -> Result<(), &'static str> {
        self.0.read_exact_statement(destination)
    }
}

/// Opaque Eq/Fp packed P-256 V3 child circuit.
#[must_use]
pub(super) struct P256PackedAffineEqChildCircuitV3(
    p256_packed_affine_v3::P256PackedAffineEcdsaCircuitV3<halo2_proofs::halo2curves::pasta::Fp>,
);

/// Opaque Ep/Fq packed P-256 V3 child circuit.
#[must_use]
pub(super) struct P256PackedAffineEpChildCircuitV3(
    p256_packed_affine_v3::P256PackedAffineEcdsaCircuitV3<halo2_proofs::halo2curves::pasta::Fq>,
);

macro_rules! impl_p256_child_circuit_v3 {
    ($wrapper:ident, $field:ty) => {
        impl halo2_proofs::plonk::Circuit<$field> for $wrapper {
            type Config = p256_packed_affine_v3::P256PackedAffineConfigV3;
            type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
            #[cfg(feature = "circuit-params")]
            type Params = ();

            fn without_witnesses(&self) -> Self {
                Self(Default::default())
            }

            #[cfg(feature = "circuit-params")]
            fn params(&self) -> Self::Params {}

            fn configure(
                meta: &mut halo2_proofs::plonk::ConstraintSystem<$field>,
            ) -> Self::Config {
                <p256_packed_affine_v3::P256PackedAffineEcdsaCircuitV3<$field> as halo2_proofs::plonk::Circuit<$field>>::configure(meta)
            }

            fn synthesize(
                &self,
                config: Self::Config,
                layouter: impl halo2_proofs::circuit::Layouter<$field>,
            ) -> Result<(), halo2_proofs::plonk::Error> {
                self.0.synthesize(config, layouter)
            }
        }
    };
}

impl_p256_child_circuit_v3!(
    P256PackedAffineEqChildCircuitV3,
    halo2_proofs::halo2curves::pasta::Fp
);
impl_p256_child_circuit_v3!(
    P256PackedAffineEpChildCircuitV3,
    halo2_proofs::halo2curves::pasta::Fq
);

/// Construct only the opaque Eq/Fp child through the nested exact-source path.
pub(super) fn p256_packed_affine_eq_child_from_source_v3(
    source: impl P256PackedStatementSourceV3,
) -> Result<P256PackedAffineEqChildCircuitV3, halo2_proofs::plonk::Error> {
    p256_packed_affine_v3::P256PackedAffineEcdsaCircuitV3::from_source(
        P256PackedStatementSourceAdapterV3(source),
    )
    .map(P256PackedAffineEqChildCircuitV3)
}

/// Construct only the opaque Ep/Fq child through the nested exact-source path.
pub(super) fn p256_packed_affine_ep_child_from_source_v3(
    source: impl P256PackedStatementSourceV3,
) -> Result<P256PackedAffineEpChildCircuitV3, halo2_proofs::plonk::Error> {
    p256_packed_affine_v3::P256PackedAffineEcdsaCircuitV3::from_source(
        P256PackedStatementSourceAdapterV3(source),
    )
    .map(P256PackedAffineEpChildCircuitV3)
}

impl P256PackedAffineEqChildCircuitV3 {
    pub(super) fn instances(
        &self,
    ) -> Result<Vec<halo2_proofs::halo2curves::pasta::Fp>, halo2_proofs::plonk::Error> {
        self.0.instances()
    }

    #[cfg(test)]
    pub(super) fn row_report_is_closed_for_test(&self) -> bool {
        self.0.row_report().is_err()
    }
}

impl P256PackedAffineEpChildCircuitV3 {
    pub(super) fn instances(
        &self,
    ) -> Result<Vec<halo2_proofs::halo2curves::pasta::Fq>, halo2_proofs::plonk::Error> {
        self.0.instances()
    }

    #[cfg(test)]
    pub(super) fn row_report_is_closed_for_test(&self) -> bool {
        self.0.row_report().is_err()
    }
}

use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    offline::{
        OfflineCashAcknowledgementV1, OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1,
        OfflineCashAuthenticatedReleaseV1, OfflineCashIpaLineageV1, OfflineCashPaymentRequestV1,
        OfflineCashPaymentV1,
    },
};
use sha2::{Digest as _, Sha256};
use std::sync::Arc;

use self::{
    state_abi::OfflineCashStatePublicInstancesV1, state_transition::OfflineCashStateContextV1,
};

/// Exact verification stage that rejected a paired proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashVerificationStageV1 {
    /// Eq/Fp ordinary proof verification and derived-accumulator decision.
    EqCurrent,
    /// Ep/Fq ordinary proof verification and derived-accumulator decision.
    EpCurrent,
}

/// Failure returned by the offline-cash terminal boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfflineCashVerificationErrorV1 {
    /// Canonical request, response, or acknowledgement validation failed.
    InvalidWire,
    /// Request is not live at the authoritative handoff time.
    RequestNotLive,
    /// The signed request selected a different authenticated release.
    ReleaseMismatch,
    /// Authenticated release did not resolve the required parity verifying-key roles.
    ArtifactMismatch,
    /// A current proof or its derived accumulator failed cryptographic verification.
    Cryptographic {
        /// Exact stage that failed.
        stage: OfflineCashVerificationStageV1,
        /// Backend diagnostic for operator logs; it is not a wire value.
        message: String,
    },
    /// Cryptography succeeded but the compiled profile is not production-authorizing.
    ActivationBlocked {
        /// Exact fail-closed compiled-profile blocker for operator logs.
        message: String,
    },
    /// A receiver acknowledgement does not identify the verified credit.
    AcknowledgementMismatch,
}

impl core::fmt::Display for OfflineCashVerificationErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidWire => formatter.write_str("invalid offline-cash wire value"),
            Self::RequestNotLive => formatter.write_str("offline-cash request is not live"),
            Self::ReleaseMismatch => {
                formatter.write_str("offline-cash payment selected a different release")
            }
            Self::ArtifactMismatch => {
                formatter.write_str("offline-cash release has invalid state verifying-key roles")
            }
            Self::Cryptographic { stage, message } => {
                write!(
                    formatter,
                    "offline-cash {stage:?} verification failed: {message}"
                )
            }
            Self::ActivationBlocked { message } => {
                write!(
                    formatter,
                    "offline-cash production activation blocked: {message}"
                )
            }
            Self::AcknowledgementMismatch => {
                formatter.write_str("offline-cash acknowledgement does not match verified credit")
            }
        }
    }
}

impl std::error::Error for OfflineCashVerificationErrorV1 {}

/// Cryptographic implementation used by the role-safe terminal boundary.
///
/// Each current-proof call verifies an ordinary Poseidon transcript and
/// terminally decides the accumulator derived by that transcript. A backend
/// cannot make a payment authoritative by verifying only one Pasta parity.
pub(crate) trait OfflineCashPairedProofVerifierV1: paired_verifier_sealed::Sealed {
    /// Verify the Eq/Fp current proof against the complete typed STATE ABI.
    fn verify_eq_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        public_instances: &OfflineCashStatePublicInstancesV1,
        proof: &[u8],
        carried_lineage: &OfflineCashIpaLineageV1,
    ) -> Result<(), String>;

    /// Verify the Ep/Fq current proof against the complete typed STATE ABI.
    fn verify_ep_current(
        &self,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
        public_instances: &OfflineCashStatePublicInstancesV1,
        proof: &[u8],
        carried_lineage: &OfflineCashIpaLineageV1,
    ) -> Result<(), String>;

    /// Grant final receipt authority only for a fully reviewed compiled profile.
    fn authorize_verified_credit(&self) -> Result<(), String>;
}

pub(crate) mod paired_verifier_sealed {
    /// Prevents any crate outside first-party Core from minting acceptance.
    pub(crate) trait Sealed {}
}

/// Move-only receipt proving that both current proofs and derived accumulators were decided.
#[derive(Debug)]
#[must_use]
pub struct VerifiedOfflineCashCreditV1 {
    release_id: [u8; 32],
    request_digest: [u8; 32],
    payment_digest: [u8; 32],
    network_id: NetworkId,
    asset: AssetDefinitionId,
    scale: u32,
    amount: u128,
    receiver_before: [u8; 32],
    recipient_key_reference: [u8; 32],
    credit_commitment: [u8; 32],
    transition_digest: [u8; 32],
    encrypted_credit_digest: [u8; 32],
}

impl VerifiedOfflineCashCreditV1 {
    /// Return the authenticated release identifier.
    #[must_use]
    pub const fn release_id(&self) -> [u8; 32] {
        self.release_id
    }

    /// Return the verified receiver-request digest.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    /// Return the verified sender-response digest.
    #[must_use]
    pub const fn payment_digest(&self) -> [u8; 32] {
        self.payment_digest
    }

    /// Return the exact network identity.
    #[must_use]
    pub const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }

    /// Return the transferred asset definition.
    #[must_use]
    pub const fn asset(&self) -> &AssetDefinitionId {
        &self.asset
    }

    /// Return the authoritative asset scale.
    #[must_use]
    pub const fn scale(&self) -> u32 {
        self.scale
    }

    /// Return the exact positive credit amount.
    #[must_use]
    pub const fn amount(&self) -> u128 {
        self.amount
    }

    /// Return the receiver head consumed by `ReceiveFold`.
    #[must_use]
    pub const fn receiver_before(&self) -> [u8; 32] {
        self.receiver_before
    }

    /// Return the exact receiver decryption-key reference named by the request.
    #[must_use]
    pub const fn recipient_key_reference(&self) -> [u8; 32] {
        self.recipient_key_reference
    }

    /// Return the proof-bound credit commitment.
    #[must_use]
    pub const fn credit_commitment(&self) -> [u8; 32] {
        self.credit_commitment
    }

    /// Return the common sender-remainder/receiver-credit transition digest.
    #[must_use]
    pub const fn transition_digest(&self) -> [u8; 32] {
        self.transition_digest
    }

    /// Return the SHA-256 of the receiver-only encrypted opening.
    #[must_use]
    pub const fn encrypted_credit_digest(&self) -> [u8; 32] {
        self.encrypted_credit_digest
    }
}

/// Role-safe verifier bound to one authenticated offline-cash release.
pub(crate) struct OfflineCashTerminalVerifierV1<'a, V> {
    release: &'a OfflineCashAuthenticatedReleaseV1,
    backend: &'a V,
}

impl<'a, V> OfflineCashTerminalVerifierV1<'a, V>
where
    V: OfflineCashPairedProofVerifierV1,
{
    /// Bind a cryptographic backend to an already authenticated release.
    #[must_use]
    pub(crate) const fn new(
        release: &'a OfflineCashAuthenticatedReleaseV1,
        backend: &'a V,
    ) -> Self {
        Self { release, backend }
    }

    /// Verify and decide both current ordinary proofs.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid wire values, stale requests, release or
    /// protocol substitution, or any failed current-proof decision.
    pub(crate) fn verify_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        now_ms: u64,
    ) -> Result<VerifiedOfflineCashCreditV1, OfflineCashVerificationErrorV1> {
        payment
            .validate_against(request)
            .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        let statement = payment
            .reconstruct_statement(request)
            .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        if now_ms < request.issued_at_ms || now_ms >= request.expires_at_ms {
            return Err(OfflineCashVerificationErrorV1::RequestNotLive);
        }
        if request.release_id != self.release.release_id() {
            return Err(OfflineCashVerificationErrorV1::ReleaseMismatch);
        }
        let eq_verifying_key = self.release.artifact(OfflineCashArtifactRoleV1::StateVkEq);
        let ep_verifying_key = self.release.artifact(OfflineCashArtifactRoleV1::StateVkEp);
        if eq_verifying_key.role != OfflineCashArtifactRoleV1::StateVkEq
            || ep_verifying_key.role != OfflineCashArtifactRoleV1::StateVkEp
        {
            return Err(OfflineCashVerificationErrorV1::ArtifactMismatch);
        }

        let context = OfflineCashStateContextV1::new(
            statement.release_id,
            statement.network_id.clone(),
            statement.asset.clone(),
            statement.scale,
        )
        .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        let eq_public_instances = OfflineCashStatePublicInstancesV1::send_split(
            &context,
            &statement,
            OfflineCashHalo2ParityV1::Eq,
            &payment.proof.recursive_pair_binding,
        )
        .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        let ep_public_instances = OfflineCashStatePublicInstancesV1::send_split(
            &context,
            &statement,
            OfflineCashHalo2ParityV1::Ep,
            &payment.proof.recursive_pair_binding,
        )
        .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        self.backend
            .verify_eq_current(
                eq_verifying_key,
                self.release.eq_protocol_digest(),
                &eq_public_instances,
                &payment.proof.eq_proof,
                &payment.proof.eq_carried_lineage,
            )
            .map_err(|message| OfflineCashVerificationErrorV1::Cryptographic {
                stage: OfflineCashVerificationStageV1::EqCurrent,
                message,
            })?;
        self.backend
            .verify_ep_current(
                ep_verifying_key,
                self.release.ep_protocol_digest(),
                &ep_public_instances,
                &payment.proof.ep_proof,
                &payment.proof.ep_carried_lineage,
            )
            .map_err(|message| OfflineCashVerificationErrorV1::Cryptographic {
                stage: OfflineCashVerificationStageV1::EpCurrent,
                message,
            })?;
        self.backend
            .authorize_verified_credit()
            .map_err(|message| OfflineCashVerificationErrorV1::ActivationBlocked { message })?;

        let encrypted_credit_digest = Sha256::digest(&payment.encrypted_credit).into();
        Ok(VerifiedOfflineCashCreditV1 {
            release_id: self.release.release_id(),
            request_digest: request
                .canonical_digest()
                .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?,
            payment_digest: payment
                .canonical_digest_against(request)
                .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?,
            network_id: statement.network_id,
            asset: statement.asset,
            scale: statement.scale,
            amount: statement.amount,
            receiver_before: statement.receiver_before,
            recipient_key_reference: request.recipient_key_reference,
            credit_commitment: statement.credit_commitment,
            transition_digest: statement.transition_digest,
            encrypted_credit_digest,
        })
    }

    /// Validate a receiver acknowledgement against one verified credit.
    ///
    /// # Errors
    ///
    /// Returns an error when the acknowledgement signature or identity binding fails.
    pub(crate) fn verify_acknowledgement(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        acknowledgement: &OfflineCashAcknowledgementV1,
        receipt: &VerifiedOfflineCashCreditV1,
    ) -> Result<(), OfflineCashVerificationErrorV1> {
        acknowledgement
            .validate_against(request, payment)
            .map_err(|_| OfflineCashVerificationErrorV1::InvalidWire)?;
        if receipt.release_id != self.release.release_id()
            || acknowledgement.release_id != receipt.release_id
            || acknowledgement.request_digest != receipt.request_digest
            || acknowledgement.payment_digest != receipt.payment_digest
        {
            return Err(OfflineCashVerificationErrorV1::AcknowledgementMismatch);
        }
        Ok(())
    }
}

/// Installed, terminal Offline Cash V1 verifier for one governed artifact release.
///
/// Construction is possible only from a complete
/// [`OfflineCashAuthenticatedArtifactFileSetV1`]. That source has already
/// authenticated every one of the 34 governed files and Core additionally
/// parses the concrete Eq/Ep STATE parameters and verifier keys before this
/// value exists. Verification retains no artifact source, performs no network
/// or filesystem access during peer handoff, and returns an unforgeable
/// move-only receipt only after both parity proofs and carried lineages are
/// terminally decided.
///
/// This type does not mutate wallet/device state. Callers must still apply the
/// verified receipt through the 14-operation secure-device lifecycle boundary;
/// production builds without a real device backend therefore remain online-only.
pub struct OfflineCashVerifierV1 {
    release: Arc<OfflineCashAuthenticatedReleaseV1>,
    backend: halo2_backend::OfflineCashHalo2VerifierBackendV1,
}

impl core::fmt::Debug for OfflineCashVerifierV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OfflineCashVerifierV1")
            .field("release_id", &self.release.release_id())
            .field("manifest_digest", &self.release.manifest_digest())
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

impl OfflineCashVerifierV1 {
    /// Parse and install the exact governed STATE verifier material.
    ///
    /// # Errors
    ///
    /// Returns an error if authenticated parameters or verifier-key bytes do
    /// not parse as the compiled k=16 Eq/Ep STATE circuits selected by the
    /// threshold-authenticated release.
    pub fn from_authenticated_artifact_file_set(
        source: OfflineCashAuthenticatedArtifactFileSetV1,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1> {
        let (release, backend) =
            halo2_backend::OfflineCashHalo2VerifierBackendV1::from_authenticated_file_set(source)?;
        Ok(Self { release, backend })
    }

    /// Governed release identifier accepted by this verifier.
    #[must_use]
    pub fn release_id(&self) -> [u8; 32] {
        self.release.release_id()
    }

    /// Digest of the complete threshold-authenticated artifact manifest.
    #[must_use]
    pub fn manifest_digest(&self) -> [u8; 32] {
        self.release.manifest_digest()
    }

    /// Verify and terminally authorize one exact sender payment.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid wire/signature/time/release bindings or if
    /// either current proof, recursive carried lineage, or terminal accumulator
    /// decision fails. The method performs no device-state mutation.
    pub fn verify_payment(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        now_ms: u64,
    ) -> Result<VerifiedOfflineCashCreditV1, OfflineCashVerificationErrorV1> {
        OfflineCashTerminalVerifierV1::new(self.release.as_ref(), &self.backend)
            .verify_payment(request, payment, now_ms)
    }

    /// Verify a receiver acknowledgement against one move-only verified credit.
    ///
    /// # Errors
    ///
    /// Returns an error if the acknowledgement signature or any release,
    /// request, or payment identity differs from the verified receipt.
    pub fn verify_acknowledgement(
        &self,
        request: &OfflineCashPaymentRequestV1,
        payment: &OfflineCashPaymentV1,
        acknowledgement: &OfflineCashAcknowledgementV1,
        receipt: &VerifiedOfflineCashCreditV1,
    ) -> Result<(), OfflineCashVerificationErrorV1> {
        OfflineCashTerminalVerifierV1::new(self.release.as_ref(), &self.backend)
            .verify_acknowledgement(request, payment, acknowledgement, receipt)
    }
}

#[cfg(test)]
#[path = "offline_cash_v1/terminal_tests.rs"]
mod terminal_tests;

#[cfg(test)]
#[path = "offline_cash_v1/halo2_tests.rs"]
mod halo2_tests;

#[cfg(test)]
#[path = "offline_cash_v1/halo2_primitives_tests.rs"]
mod halo2_primitives_tests;

#[cfg(test)]
#[path = "offline_cash_v1/state_circuit_tests.rs"]
mod state_circuit_tests;

#[cfg(test)]
#[path = "offline_cash_v1/helper_tests.rs"]
mod helper_tests;
