//! Private, fail-closed registered-platform P-256 statement candidate for Offline Cash V2.
//!
//! This module binds one durable registration identity to one separately authenticated current
//! helper candidate, then freezes the exact transaction-time P-256 public statement bytes.  The
//! binding is deliberately unverified: it checks canonical key/signature shape, identity
//! equality, and SHA-256 of the exact current platform message, but never verifies ECDSA or a
//! Halo2 proof.  The only role represented here is role 6, the transaction platform signature.
//! Certificate signatures remain entirely inside native registration.
//!
//! Current-helper authentication, freshness, a circuit/source, compiled protocol, artifacts,
//! backend, `GuardBundle` integration, wire integration, and every production authority remain
//! unavailable.  In particular, the historical registration event's `GuardBundle` digest is not
//! projected or compared with the current transaction helper statement.

use core::{convert::Infallible, fmt};

use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize, Zeroizing};

use super::{
    OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2,
    OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2,
    OfflineCashHalo2CircuitRoleV2, OfflineCashHalo2ParityV2,
    attestation_registration::{
        DurableRegistrationIdentityProjectionV2, NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2,
    },
};

const PLATFORM_MESSAGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-message";

/// Exact bytes in the current transaction's framed platform message.
pub(super) const REGISTERED_PLATFORM_P256_PLATFORM_MESSAGE_BYTES_V2: usize = 494;
/// Exact SEC1 bytes at the beginning of each role-6 statement.
pub(super) const REGISTERED_PLATFORM_P256_SEC1_BYTES_V2: usize = 65;
/// Exact SHA-256 prehash bytes following SEC1.
pub(super) const REGISTERED_PLATFORM_P256_PREHASH_BYTES_V2: usize = 32;
/// Exact fixed-width P1363 `r || s` bytes following the prehash.
pub(super) const REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2: usize = 64;
/// Exact public statement bytes: `[SEC1 | prehash | P1363 signature]`.
pub(super) const REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2: usize =
    REGISTERED_PLATFORM_P256_SEC1_BYTES_V2
        + REGISTERED_PLATFORM_P256_PREHASH_BYTES_V2
        + REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2;

pub(super) const REGISTERED_PLATFORM_P256_SEC1_OFFSET_V2: usize = 0;
pub(super) const REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2: usize =
    REGISTERED_PLATFORM_P256_SEC1_OFFSET_V2 + REGISTERED_PLATFORM_P256_SEC1_BYTES_V2;
pub(super) const REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2: usize =
    REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2 + REGISTERED_PLATFORM_P256_PREHASH_BYTES_V2;
pub(super) const REGISTERED_PLATFORM_P256_STATEMENT_END_V2: usize =
    REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2 + REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2;

/// Stable logical bytes in one durable registration identity projection.
pub(super) const REGISTERED_PLATFORM_P256_DURABLE_IDENTITY_LOGICAL_BYTES_V2: usize = 233;
/// Stable logical bytes in the exact current-helper field projection.
pub(super) const REGISTERED_PLATFORM_P256_CURRENT_HELPER_LOGICAL_BYTES_V2: usize = 401;
/// Stable logical bytes in the move-only authenticated registration/current-helper context.
pub(super) const REGISTERED_PLATFORM_P256_AUTHENTICATED_CONTEXT_LOGICAL_BYTES_V2: usize = 634;
/// Stable logical bytes retained by the owner before the parity pair is emitted.
pub(super) const REGISTERED_PLATFORM_P256_PRE_PAIR_OWNER_LOGICAL_BYTES_V2: usize = 795;
/// Stable logical bytes in one typed parity/role/statement frame.
pub(super) const REGISTERED_PLATFORM_P256_TYPED_STATEMENT_LOGICAL_BYTES_V2: usize = 163;
/// Stable logical bytes in the exact Eq-then-Ep typed pair.
pub(super) const REGISTERED_PLATFORM_P256_TYPED_PAIR_LOGICAL_BYTES_V2: usize = 326;
/// Stable logical bytes retained by the context-preserving source pair.
pub(super) const REGISTERED_PLATFORM_P256_SOURCE_PAIR_LOGICAL_BYTES_V2: usize = 960;

/// The durable 2,823-byte registration envelope is transported out of band from a transaction.
pub(super) const REGISTERED_PLATFORM_P256_OUT_OF_BAND_REGISTRATION_BYTES_V2: usize =
    NATIVE_REGISTRATION_OFFLINE_ENVELOPE_RAW_BYTES_V2;
/// Candidate 32-byte durable-receipt reference reservation.
///
/// This is non-authorizing accounting telemetry only.  No encoder, decoder, session field, cap
/// change, or terminal consumer exists.
pub(super) const REGISTERED_PLATFORM_P256_CANDIDATE_REGISTRATION_RECEIPT_REFERENCE_BYTES_V2: usize =
    32;
/// Reviewed component arithmetic if the still-unimplemented binding reservation is governed.
///
/// This is not an active wire maximum.
pub(super) const REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_BYTES_V2: usize = 9_184;
/// Remaining arithmetic room under the unresolved aggregate policy ceiling.
pub(super) const REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_HEADROOM_BYTES_V2: usize = 219;
/// Arithmetic total if the complete registration envelope were incorrectly put in session.
pub(super) const REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_TOTAL_BYTES_V2: usize = 11_975;
/// Amount by which that incorrect in-session envelope would exceed the unresolved ceiling.
pub(super) const REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_EXCESS_BYTES_V2: usize = 2_572;

/// Role 6 has exactly one semantic meaning in this candidate.
pub(super) const REGISTERED_PLATFORM_P256_ROLE_CONTRACT_V2: &[u8] =
    b"role-6=transaction-platform-signature-only/certificate-signatures=native-registration-only/v2";
/// Frozen public byte contract.  No registration receipt or historical helper digest is emitted.
pub(super) const REGISTERED_PLATFORM_P256_STATEMENT_CONTRACT_V2: &[u8] =
    b"65-byte-canonical-uncompressed-sec1 || sha256(exact-494-byte-current-v1-platform-message) || canonical-low-s-64-byte-p1363-r-s/v2";

/// This structural candidate is privately declared.
pub(super) const REGISTERED_PLATFORM_P256_DECLARED_V2: bool = true;
/// No production adapter can authenticate a current helper candidate.
pub(super) const REGISTERED_PLATFORM_P256_CURRENT_HELPER_AUTHENTICATION_AVAILABLE_V2: bool = false;
/// No ordinary-build freshness authority can authorize the candidate.
pub(super) const REGISTERED_PLATFORM_P256_FRESHNESS_AUTHORITY_AVAILABLE_V2: bool = false;
/// No production Halo2 circuit-source capability is available for this statement.
pub(super) const REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_AVAILABLE_V2: bool = false;
/// No compiled role-6 protocol identity exists.
pub(super) const REGISTERED_PLATFORM_P256_COMPILED_PROTOCOL_AVAILABLE_V2: bool = false;
/// No authenticated role-6 artifacts exist.
pub(super) const REGISTERED_PLATFORM_P256_ARTIFACTS_AUTHENTICATED_V2: bool = false;
/// No role-6 verification backend exists.
pub(super) const REGISTERED_PLATFORM_P256_BACKEND_AVAILABLE_V2: bool = false;
/// No recursive `GuardBundle` adapter consumes this statement.
pub(super) const REGISTERED_PLATFORM_P256_GUARD_BUNDLE_ADAPTER_AVAILABLE_V2: bool = false;
/// No wire adapter carries a new field for this candidate.
pub(super) const REGISTERED_PLATFORM_P256_WIRE_ADAPTER_AVAILABLE_V2: bool = false;
/// No activation/readiness authority exists.
pub(super) const REGISTERED_PLATFORM_P256_READINESS_AVAILABLE_V2: bool = false;
/// This candidate is not release eligible.
pub(super) const REGISTERED_PLATFORM_P256_RELEASE_ELIGIBLE_V2: bool = false;
/// No production path exists.
pub(super) const REGISTERED_PLATFORM_P256_PRODUCTION_AVAILABLE_V2: bool = false;

const _: () = assert!(REGISTERED_PLATFORM_P256_PLATFORM_MESSAGE_BYTES_V2 == 494);
const _: () = assert!(REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2 == 161);
const _: () = assert!(REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2 == 65);
const _: () = assert!(REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2 == 97);
const _: () = assert!(REGISTERED_PLATFORM_P256_STATEMENT_END_V2 == 161);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_AUTHENTICATED_CONTEXT_LOGICAL_BYTES_V2
        == REGISTERED_PLATFORM_P256_DURABLE_IDENTITY_LOGICAL_BYTES_V2
            + REGISTERED_PLATFORM_P256_CURRENT_HELPER_LOGICAL_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_PRE_PAIR_OWNER_LOGICAL_BYTES_V2
        == REGISTERED_PLATFORM_P256_AUTHENTICATED_CONTEXT_LOGICAL_BYTES_V2
            + REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_TYPED_STATEMENT_LOGICAL_BYTES_V2
        == 1 + 1 + REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_TYPED_PAIR_LOGICAL_BYTES_V2
        == 2 * REGISTERED_PLATFORM_P256_TYPED_STATEMENT_LOGICAL_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_SOURCE_PAIR_LOGICAL_BYTES_V2
        == REGISTERED_PLATFORM_P256_AUTHENTICATED_CONTEXT_LOGICAL_BYTES_V2
            + REGISTERED_PLATFORM_P256_TYPED_PAIR_LOGICAL_BYTES_V2
);
const _: () = assert!(REGISTERED_PLATFORM_P256_OUT_OF_BAND_REGISTRATION_BYTES_V2 == 2_823);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_BYTES_V2
        == OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2 as usize
            + REGISTERED_PLATFORM_P256_CANDIDATE_REGISTRATION_RECEIPT_REFERENCE_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_BYTES_V2
        + REGISTERED_PLATFORM_P256_REVIEWED_RAW_SESSION_HEADROOM_BYTES_V2
        == OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2 as usize
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_TOTAL_BYTES_V2
        == OFFLINE_CASH_EXACT_COMPONENT_RAW_SESSION_BYTES_V2 as usize
            + REGISTERED_PLATFORM_P256_OUT_OF_BAND_REGISTRATION_BYTES_V2
);
const _: () = assert!(
    REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_EXCESS_BYTES_V2
        == REGISTERED_PLATFORM_P256_REGISTRATION_IN_SESSION_TOTAL_BYTES_V2
            - OFFLINE_CASH_UNRESOLVED_AGGREGATE_RAW_SESSION_POLICY_CEILING_BYTES_V2 as usize
);
const _: () = assert!(REGISTERED_PLATFORM_P256_DECLARED_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_CURRENT_HELPER_AUTHENTICATION_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_FRESHNESS_AUTHORITY_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_CIRCUIT_SOURCE_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_COMPILED_PROTOCOL_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_ARTIFACTS_AUTHENTICATED_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_BACKEND_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_GUARD_BUNDLE_ADAPTER_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_WIRE_ADAPTER_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_READINESS_AVAILABLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_RELEASE_ELIGIBLE_V2);
const _: () = assert!(!REGISTERED_PLATFORM_P256_PRODUCTION_AVAILABLE_V2);

/// Exact current-helper fields needed to reconstruct the transaction platform message.
///
/// There is intentionally no ordinary constructor.  A future adapter must project these fields
/// from one authenticated *current* helper statement in the same transaction context.
pub(super) struct RegisteredPlatformP256CurrentHelperFieldsV2 {
    operation: u8,
    release_id: [u8; 32],
    context_digest: [u8; 32],
    current_head: [u8; 32],
    current_lineage_digest: [u8; 32],
    transition_digest: [u8; 32],
    wallet_binding: [u8; 32],
    hardware_policy_id: [u8; 32],
    guard_device_id: [u8; 32],
    current_guard_binding: [u8; 32],
    next_guard_binding: [u8; 32],
    from_sequence: u64,
    to_sequence: u64,
    platform_key_digest: [u8; 32],
    platform_message_digest: [u8; 32],
}

/// Uninhabited production authority for current-helper authentication.
pub(super) enum RegisteredPlatformP256CurrentHelperAuthorityV2 {}
/// Uninhabited production freshness authority.
pub(super) enum RegisteredPlatformP256FreshnessAuthorityV2 {}
/// Uninhabited circuit-source authority.
pub(super) enum RegisteredPlatformP256CircuitSourceAuthorityV2 {}
/// Uninhabited compiled-protocol authority.
pub(super) enum RegisteredPlatformP256CompiledProtocolAuthorityV2 {}
/// Uninhabited authenticated-artifact authority.
pub(super) enum RegisteredPlatformP256ArtifactAuthorityV2 {}
/// Uninhabited proof-verifier authority.
pub(super) enum RegisteredPlatformP256VerifierAuthorityV2 {}
/// Uninhabited recursive `GuardBundle` authority.
pub(super) enum RegisteredPlatformP256GuardBundleAuthorityV2 {}
/// Uninhabited wire-adapter authority.
pub(super) enum RegisteredPlatformP256WireAdapterAuthorityV2 {}

/// Move-only claim that a separate authority authenticated the current helper statement.
///
/// The type itself does not verify proof bytes or freshness.  Its production constructor remains
/// impossible while the authority above is uninhabited.
pub(super) struct AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
    registration_identity: DurableRegistrationIdentityProjectionV2,
    current: RegisteredPlatformP256CurrentHelperFieldsV2,
}

impl AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
    /// Borrow the typed durable identity without exposing a raw-parts decomposition.
    pub(super) const fn durable_identity(&self) -> &DurableRegistrationIdentityProjectionV2 {
        &self.registration_identity
    }

    /// Borrow the exact typed current-helper projection without transferring ownership.
    pub(super) const fn current_helper_fields(
        &self,
    ) -> &RegisteredPlatformP256CurrentHelperFieldsV2 {
        &self.current
    }
}

/// The only production constructor for an authenticated current-helper candidate.
pub(super) fn authenticate_registered_platform_current_helper_v2(
    _registration_identity: DurableRegistrationIdentityProjectionV2,
    _current: RegisteredPlatformP256CurrentHelperFieldsV2,
    authority: RegisteredPlatformP256CurrentHelperAuthorityV2,
) -> AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
    match authority {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RegisteredPlatformP256StatementErrorV2 {
    MalformedDurableIdentity,
    DurableIdentityMismatch,
    MalformedCurrentHelper,
    PolicyIdentityMismatch,
    DeviceIdentityMismatch,
    InvalidPlatformPublicKey,
    PlatformKeyIdentityMismatch,
    PlatformMessageGeometryMismatch,
    PlatformMessageDigestMismatch,
    AuthenticatedContextStatementMismatch,
    InvalidPlatformSignature,
    HighSPlatformSignature,
    VerificationUnavailable,
}

impl fmt::Display for RegisteredPlatformP256StatementErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MalformedDurableIdentity => {
                "offline-cash V2 durable registration identity is malformed"
            }
            Self::DurableIdentityMismatch => {
                "offline-cash V2 durable registration identity does not match the authenticated current helper"
            }
            Self::MalformedCurrentHelper => {
                "offline-cash V2 authenticated current-helper statement is malformed"
            }
            Self::PolicyIdentityMismatch => {
                "offline-cash V2 current helper does not use the registered policy"
            }
            Self::DeviceIdentityMismatch => {
                "offline-cash V2 current helper does not use the registered device descriptor"
            }
            Self::InvalidPlatformPublicKey => {
                "offline-cash V2 registered platform key is not canonical uncompressed P-256 SEC1"
            }
            Self::PlatformKeyIdentityMismatch => {
                "offline-cash V2 registered platform key identity does not match the current helper"
            }
            Self::PlatformMessageGeometryMismatch => {
                "offline-cash V2 current platform message does not have the exact frozen geometry"
            }
            Self::PlatformMessageDigestMismatch => {
                "offline-cash V2 current platform-message prehash does not match the helper statement"
            }
            Self::AuthenticatedContextStatementMismatch => {
                "offline-cash V2 typed P-256 statements do not match their authenticated current-helper context"
            }
            Self::InvalidPlatformSignature => {
                "offline-cash V2 transaction platform signature is not canonical P1363"
            }
            Self::HighSPlatformSignature => {
                "offline-cash V2 transaction platform signature is not low-S"
            }
            Self::VerificationUnavailable => {
                "offline-cash V2 registered-platform P-256 verification is unavailable"
            }
        })
    }
}

impl std::error::Error for RegisteredPlatformP256StatementErrorV2 {}

/// Move-only structural binding.  It is explicitly not a verified statement owner.
pub(super) struct UnverifiedRegisteredPlatformP256OwnerBindingV2 {
    authenticated_current_helper: AuthenticatedRegisteredPlatformCurrentHelperCandidateV2,
    statement_bytes: Zeroizing<[u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2]>,
}

impl UnverifiedRegisteredPlatformP256OwnerBindingV2 {
    /// Bind exact public values without verifying ECDSA, Halo2, recursion, or freshness.
    pub(super) fn from_authenticated_current_helper(
        durable_identity: DurableRegistrationIdentityProjectionV2,
        candidate: AuthenticatedRegisteredPlatformCurrentHelperCandidateV2,
        signature: [u8; REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2],
    ) -> Result<Self, RegisteredPlatformP256StatementErrorV2> {
        validate_durable_identity(&durable_identity)?;
        if durable_identity != candidate.registration_identity {
            return Err(RegisteredPlatformP256StatementErrorV2::DurableIdentityMismatch);
        }
        let statement_bytes = exact_statement_bytes_for_authenticated_context(
            &candidate.registration_identity,
            &candidate.current,
            &signature,
        )?;

        Ok(Self {
            authenticated_current_helper: candidate,
            statement_bytes: Zeroizing::new(statement_bytes),
        })
    }

    pub(super) const fn durable_receipt_commitment(&self) -> &[u8; 32] {
        self.authenticated_current_helper
            .registration_identity
            .receipt_commitment()
    }

    pub(super) const fn authenticated_current_helper(
        &self,
    ) -> &AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
        &self.authenticated_current_helper
    }

    pub(super) fn statement_bytes(&self) -> &[u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2] {
        &self.statement_bytes
    }

    /// Move the authenticated context and both parity statements as one indivisible source.
    pub(super) fn into_eq_ep_source_pair(
        self,
    ) -> UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
        let statement_bytes = *self.statement_bytes;
        UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
            authenticated_current_helper: self.authenticated_current_helper,
            statements: [
                UnverifiedRegisteredPlatformP256StatementV2 {
                    parity: OfflineCashHalo2ParityV2::Eq,
                    role: OfflineCashHalo2CircuitRoleV2::P256Signature,
                    statement_bytes,
                },
                UnverifiedRegisteredPlatformP256StatementV2 {
                    parity: OfflineCashHalo2ParityV2::Ep,
                    role: OfflineCashHalo2CircuitRoleV2::P256Signature,
                    statement_bytes,
                },
            ],
        }
    }
}

/// Exact-shape parity statement.  This type carries no verification authority.
pub(super) struct UnverifiedRegisteredPlatformP256StatementV2 {
    parity: OfflineCashHalo2ParityV2,
    role: OfflineCashHalo2CircuitRoleV2,
    statement_bytes: [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2],
}

impl UnverifiedRegisteredPlatformP256StatementV2 {
    pub(super) const fn parity(&self) -> OfflineCashHalo2ParityV2 {
        self.parity
    }

    pub(super) const fn role(&self) -> OfflineCashHalo2CircuitRoleV2 {
        self.role
    }

    pub(super) const fn statement_bytes(
        &self,
    ) -> &[u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2] {
        &self.statement_bytes
    }

    #[cfg(test)]
    pub(super) fn set_parity_for_test_v2(&mut self, parity: OfflineCashHalo2ParityV2) {
        self.parity = parity;
    }

    #[cfg(test)]
    pub(super) fn xor_statement_byte_for_test_v2(&mut self, index: usize, mask: u8) {
        self.statement_bytes[index] ^= mask;
    }

    #[cfg(test)]
    pub(super) fn zero_statement_for_test_v2(&mut self) {
        self.statement_bytes.zeroize();
    }
}

/// Move-only Eq-then-Ep source retaining its authenticated registration/current-helper context.
pub(super) struct UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
    authenticated_current_helper: AuthenticatedRegisteredPlatformCurrentHelperCandidateV2,
    statements: [UnverifiedRegisteredPlatformP256StatementV2; 2],
}

impl UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
    /// Borrow the authenticated context without allowing it to be split from the pair.
    pub(super) const fn authenticated_current_helper(
        &self,
    ) -> &AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
        &self.authenticated_current_helper
    }

    /// Borrow the exact typed Eq-then-Ep provenance without transferring ownership.
    pub(super) const fn statements(&self) -> &[UnverifiedRegisteredPlatformP256StatementV2; 2] {
        &self.statements
    }

    #[cfg(test)]
    pub(super) fn swap_statements_for_test_v2(&mut self) {
        self.statements.swap(0, 1);
    }

    #[cfg(test)]
    pub(super) fn xor_ep_statement_byte_for_test_v2(&mut self, index: usize, mask: u8) {
        self.statements[1].xor_statement_byte_for_test_v2(index, mask);
    }

    #[cfg(test)]
    pub(super) fn zero_statements_for_test_v2(&mut self) {
        self.statements[0].zero_statement_for_test_v2();
        self.statements[1].zero_statement_for_test_v2();
    }

    #[cfg(test)]
    pub(super) fn xor_both_statement_bytes_for_test_v2(&mut self, index: usize, mask: u8) {
        self.statements[0].xor_statement_byte_for_test_v2(index, mask);
        self.statements[1].xor_statement_byte_for_test_v2(index, mask);
    }

    #[cfg(test)]
    pub(super) fn set_eq_parity_for_test_v2(&mut self, parity: OfflineCashHalo2ParityV2) {
        self.statements[0].set_parity_for_test_v2(parity);
    }
}

/// Recheck the context-to-key/prehash binding before an opaque circuit source reads the pair.
pub(super) fn validate_registered_platform_p256_source_pair_context_v2(
    source_pair: &UnverifiedRegisteredPlatformP256StatementSourcePairV2,
) -> Result<(), RegisteredPlatformP256StatementErrorV2> {
    let statement_bytes = source_pair.statements[0].statement_bytes();
    let mut signature = [0_u8; REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2];
    signature.copy_from_slice(
        &statement_bytes[REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2
            ..REGISTERED_PLATFORM_P256_STATEMENT_END_V2],
    );
    let expected = exact_statement_bytes_for_authenticated_context(
        &source_pair
            .authenticated_current_helper
            .registration_identity,
        &source_pair.authenticated_current_helper.current,
        &signature,
    )?;
    if source_pair
        .statements
        .iter()
        .any(|statement| statement.statement_bytes() != &expected)
    {
        return Err(RegisteredPlatformP256StatementErrorV2::AuthenticatedContextStatementMismatch);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn registered_platform_p256_source_pair_for_test_v2(
    statement_bytes: [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2],
) -> UnverifiedRegisteredPlatformP256StatementSourcePairV2 {
    let mut sec1 = [0_u8; REGISTERED_PLATFORM_P256_SEC1_BYTES_V2];
    sec1.copy_from_slice(
        &statement_bytes
            [REGISTERED_PLATFORM_P256_SEC1_OFFSET_V2..REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2],
    );
    let platform_key_digest: [u8; 32] = Sha256::digest(sec1).into();
    let identity = DurableRegistrationIdentityProjectionV2::from_test_parts(
        9,
        [0x17; 32],
        [0x21; 32],
        [0x18; 32],
        platform_key_digest,
        sec1,
        [0x22; 32],
    );
    let mut current = RegisteredPlatformP256CurrentHelperFieldsV2 {
        operation: 1,
        release_id: [0x11; 32],
        context_digest: [0x12; 32],
        current_head: [0x13; 32],
        current_lineage_digest: [0x14; 32],
        transition_digest: [0x15; 32],
        wallet_binding: [0x16; 32],
        hardware_policy_id: [0x17; 32],
        guard_device_id: [0x18; 32],
        current_guard_binding: [0x19; 32],
        next_guard_binding: [0x1A; 32],
        from_sequence: 7,
        to_sequence: 8,
        platform_key_digest,
        platform_message_digest: [0xFF; 32],
    };
    let platform_message =
        exact_current_platform_message(&current).expect("test helper has exact message geometry");
    current.platform_message_digest = Sha256::digest(platform_message).into();
    let candidate = AuthenticatedRegisteredPlatformCurrentHelperCandidateV2 {
        registration_identity: identity.clone(),
        current,
    };
    let mut signature = [0_u8; REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2];
    signature.copy_from_slice(
        &statement_bytes[REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2
            ..REGISTERED_PLATFORM_P256_STATEMENT_END_V2],
    );
    let source_pair =
        UnverifiedRegisteredPlatformP256OwnerBindingV2::from_authenticated_current_helper(
            identity, candidate, signature,
        )
        .expect("test statement has one valid authenticated context")
        .into_eq_ep_source_pair();
    assert_eq!(
        source_pair.statements[0].statement_bytes(),
        &statement_bytes
    );
    source_pair
}

/// No verified value can cross this boundary while the backend remains unavailable.
pub(super) fn fail_closed_registered_platform_p256_boundary_v2(
    _binding: UnverifiedRegisteredPlatformP256OwnerBindingV2,
) -> Result<Infallible, RegisteredPlatformP256StatementErrorV2> {
    Err(RegisteredPlatformP256StatementErrorV2::VerificationUnavailable)
}

fn exact_statement_bytes_for_authenticated_context(
    durable_identity: &DurableRegistrationIdentityProjectionV2,
    current: &RegisteredPlatformP256CurrentHelperFieldsV2,
    signature: &[u8; REGISTERED_PLATFORM_P256_SIGNATURE_BYTES_V2],
) -> Result<[u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2], RegisteredPlatformP256StatementErrorV2>
{
    validate_durable_identity(durable_identity)?;
    validate_current_helper(current)?;
    if durable_identity.policy_digest() != &current.hardware_policy_id {
        return Err(RegisteredPlatformP256StatementErrorV2::PolicyIdentityMismatch);
    }
    if durable_identity.device_descriptor_digest() != &current.guard_device_id {
        return Err(RegisteredPlatformP256StatementErrorV2::DeviceIdentityMismatch);
    }

    let platform_key = durable_identity.device_public_key_sec1();
    let verifying_key = P256VerifyingKey::from_sec1_bytes(platform_key)
        .map_err(|_| RegisteredPlatformP256StatementErrorV2::InvalidPlatformPublicKey)?;
    if platform_key[0] != 4 || verifying_key.to_encoded_point(false).as_bytes() != platform_key {
        return Err(RegisteredPlatformP256StatementErrorV2::InvalidPlatformPublicKey);
    }
    let platform_key_digest: [u8; 32] = Sha256::digest(platform_key).into();
    if durable_identity.device_key_id() != &platform_key_digest
        || current.platform_key_digest != platform_key_digest
    {
        return Err(RegisteredPlatformP256StatementErrorV2::PlatformKeyIdentityMismatch);
    }

    let platform_message = exact_current_platform_message(current)?;
    if platform_message.len() != REGISTERED_PLATFORM_P256_PLATFORM_MESSAGE_BYTES_V2 {
        return Err(RegisteredPlatformP256StatementErrorV2::PlatformMessageGeometryMismatch);
    }
    let platform_message_digest: [u8; 32] = Sha256::digest(&platform_message).into();
    if current.platform_message_digest != platform_message_digest {
        return Err(RegisteredPlatformP256StatementErrorV2::PlatformMessageDigestMismatch);
    }

    let parsed_signature = P256Signature::from_slice(signature)
        .map_err(|_| RegisteredPlatformP256StatementErrorV2::InvalidPlatformSignature)?;
    if parsed_signature.normalize_s().is_some() {
        return Err(RegisteredPlatformP256StatementErrorV2::HighSPlatformSignature);
    }

    let mut statement_bytes = [0_u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];
    statement_bytes
        [REGISTERED_PLATFORM_P256_SEC1_OFFSET_V2..REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2]
        .copy_from_slice(platform_key);
    statement_bytes
        [REGISTERED_PLATFORM_P256_PREHASH_OFFSET_V2..REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2]
        .copy_from_slice(&platform_message_digest);
    statement_bytes
        [REGISTERED_PLATFORM_P256_SIGNATURE_OFFSET_V2..REGISTERED_PLATFORM_P256_STATEMENT_END_V2]
        .copy_from_slice(signature);
    Ok(statement_bytes)
}

fn validate_durable_identity(
    identity: &DurableRegistrationIdentityProjectionV2,
) -> Result<(), RegisteredPlatformP256StatementErrorV2> {
    if identity.policy_epoch() == 0
        || [
            identity.policy_digest(),
            identity.registry_root_intent(),
            identity.device_descriptor_digest(),
            identity.device_key_id(),
            identity.receipt_commitment(),
        ]
        .into_iter()
        .any(|digest| *digest == [0; 32])
    {
        return Err(RegisteredPlatformP256StatementErrorV2::MalformedDurableIdentity);
    }
    Ok(())
}

fn validate_current_helper(
    current: &RegisteredPlatformP256CurrentHelperFieldsV2,
) -> Result<(), RegisteredPlatformP256StatementErrorV2> {
    if !matches!(current.operation, 1 | 2)
        || current.from_sequence.checked_add(1) != Some(current.to_sequence)
        || current.current_head == current.transition_digest
        || current.current_guard_binding == current.next_guard_binding
        || [
            current.release_id,
            current.context_digest,
            current.current_head,
            current.current_lineage_digest,
            current.transition_digest,
            current.wallet_binding,
            current.hardware_policy_id,
            current.guard_device_id,
            current.current_guard_binding,
            current.next_guard_binding,
            current.platform_key_digest,
            current.platform_message_digest,
        ]
        .into_iter()
        .any(|digest| digest == [0; 32])
    {
        return Err(RegisteredPlatformP256StatementErrorV2::MalformedCurrentHelper);
    }
    Ok(())
}

fn exact_current_platform_message(
    current: &RegisteredPlatformP256CurrentHelperFieldsV2,
) -> Result<Vec<u8>, RegisteredPlatformP256StatementErrorV2> {
    let operation = [current.operation];
    let from_sequence = current.from_sequence.to_le_bytes();
    let to_sequence = current.to_sequence.to_le_bytes();
    framed_bytes(
        PLATFORM_MESSAGE_DOMAIN,
        &[
            &operation,
            &current.release_id,
            &current.context_digest,
            &current.current_head,
            &current.current_lineage_digest,
            &current.transition_digest,
            &current.wallet_binding,
            &current.hardware_policy_id,
            &current.guard_device_id,
            &current.current_guard_binding,
            &current.next_guard_binding,
            &from_sequence,
            &to_sequence,
        ],
    )
}

fn framed_bytes(
    domain: &[u8],
    fields: &[&[u8]],
) -> Result<Vec<u8>, RegisteredPlatformP256StatementErrorV2> {
    let payload_len = 8_usize
        .checked_add(domain.len())
        .and_then(|length| {
            fields.iter().try_fold(length, |length, field| {
                length.checked_add(8)?.checked_add(field.len())
            })
        })
        .ok_or(RegisteredPlatformP256StatementErrorV2::PlatformMessageGeometryMismatch)?;
    let mut message = Vec::new();
    message
        .try_reserve_exact(payload_len)
        .map_err(|_| RegisteredPlatformP256StatementErrorV2::PlatformMessageGeometryMismatch)?;
    message.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| RegisteredPlatformP256StatementErrorV2::PlatformMessageGeometryMismatch)?
            .to_le_bytes(),
    );
    message.extend_from_slice(domain);
    for field in fields {
        message.extend_from_slice(
            &u64::try_from(field.len())
                .map_err(|_| {
                    RegisteredPlatformP256StatementErrorV2::PlatformMessageGeometryMismatch
                })?
                .to_le_bytes(),
        );
        message.extend_from_slice(field);
    }
    debug_assert_eq!(message.len(), payload_len);
    Ok(message)
}

#[cfg(test)]
#[path = "registered_platform_p256_statement_tests.rs"]
mod tests;
