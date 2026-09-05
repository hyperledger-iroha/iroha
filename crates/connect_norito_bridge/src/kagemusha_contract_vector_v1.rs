//! Canonical native/mobile contract vector for the KAGEMUSHA V1 surface.
//!
//! The vector is emitted by the linked native library so every mobile binding
//! can pin the exact protocol, proof-artifact, hardware-capability, and device
//! operation inventories compiled into that library. Its domain-separated
//! digest is an ABI/tamper pin only. It is not a signature, hardware
//! attestation, settlement receipt, or source of monetary authority.

use iroha_data_model::kagemusha::{
    KAGEMUSHA_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_RECEIVER_BOUND_CREDIT_COMMIT_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1,
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaArtifactRoleV1,
    KagemushaIpm1PayloadKindV1, KagemushaQualifiedHelperCircuitV1, KagemushaQualifiedRelationV1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use crate::{
    CONNECT_NORITO_KAGEMUSHA_IPM1_MESSAGE_KIND_TAGS_V1, KagemushaDeviceLifecycleOperationV1,
};

/// Version of the sole native/mobile KAGEMUSHA contract vector.
pub const KAGEMUSHA_NATIVE_CONTRACT_VECTOR_VERSION_V1: u16 = 1;
/// Domain authenticating the canonical contract-vector body bytes.
pub const KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:native-contract-vector:v1";
/// Maximum accepted canonical vector size at the native boundary.
pub const KAGEMUSHA_NATIVE_CONTRACT_VECTOR_MAX_BYTES_V1: usize = 4 * 1024;

/// Exact ordered lower-sixteen-bit hardware capability inventory.
pub const KAGEMUSHA_NATIVE_HARDWARE_CAPABILITY_BITS_V1: [u16; 16] = [
    KAGEMUSHA_HARDWARE_CAPABILITY_EXACT_NEXT_PREDECESSOR_CONSUMPTION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ONE_USE_SUCCESSOR_AUTHORIZATION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_COUNTER_AND_JOURNAL_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_SEALED_TRANSITION_RECOVERY_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_RECEIVER_BOUND_CREDIT_COMMIT_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_RESISTANT_ACCEPTED_CREDIT_INBOX_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_INBOUND_STAGING_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_AUTHORITATIVE_REPLAY_ROOT_RECOVERY_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_SENDER_OUTBOX_RESERVATION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_AUTHENTICATED_DURABLE_RETRY_OUTBOX_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ATOMIC_VERIFIED_CANDIDATE_COMMIT_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_RECOVERABLE_TERMINAL_COMMIT_CERTIFICATE_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_TRUSTED_TIME_OR_LEASE_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_OFFLINE_HARDWARE_EPOCH_ROTATION_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_ROLLBACK_SAFE_COUNTER_ROLLOVER_V1,
    KAGEMUSHA_HARDWARE_CAPABILITY_NO_SOFTWARE_FALLBACK_V1,
];

/// Pinned SHA-256 digest of the V1 canonical contract-vector body.
///
/// The digest is an ABI/tamper pin only and never grants monetary authority.
pub const KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DIGEST_V1: [u8; 32] = [
    0x13, 0xb5, 0x11, 0x24, 0xf0, 0x32, 0x9f, 0xc4, 0x7b, 0x0a, 0xa3, 0xbf, 0x55, 0x1f, 0x83, 0xf1,
    0x80, 0x69, 0x20, 0xc9, 0x89, 0x8e, 0x7c, 0x07, 0xcd, 0x7f, 0x07, 0x30, 0xeb, 0x57, 0xfb, 0xb9,
];

/// Closed validation failures for a native KAGEMUSHA contract vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaNativeContractVectorErrorV1 {
    /// The archive is empty or exceeds the fixed native boundary.
    Size,
    /// The bytes are not exact canonical Norito for this V1 type.
    CanonicalEncoding,
    /// A count or ordered inventory differs from the compiled V1 contract.
    InventoryMismatch,
    /// The embedded digest does not authenticate the exact canonical body.
    DigestMismatch,
}

/// One stable numeric code and its canonical lowercase contract name.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct KagemushaNativeContractInventoryEntryV1 {
    /// Stable wire, discriminant, capability-bit, or device-operation code.
    pub code: u16,
    /// Canonical lowercase snake-case semantic name for that code.
    pub name: String,
}

impl KagemushaNativeContractInventoryEntryV1 {
    fn new(code: u16, name: &'static str) -> Self {
        Self {
            code,
            name: name.to_owned(),
        }
    }
}

const fn peer_message_name(kind: KagemushaIpm1PayloadKindV1) -> &'static str {
    match kind {
        KagemushaIpm1PayloadKindV1::Request => "payment_request",
        KagemushaIpm1PayloadKindV1::Payment => "payment",
        KagemushaIpm1PayloadKindV1::Acknowledgement => "acknowledgement",
    }
}

const fn artifact_role_name(role: KagemushaArtifactRoleV1) -> &'static str {
    match role {
        KagemushaArtifactRoleV1::ParamsEq => "params_eq",
        KagemushaArtifactRoleV1::ParamsEp => "params_ep",
        KagemushaArtifactRoleV1::InnerStatePkEq => "inner_state_pk_eq",
        KagemushaArtifactRoleV1::InnerStateVkEq => "inner_state_vk_eq",
        KagemushaArtifactRoleV1::InnerStatePkEp => "inner_state_pk_ep",
        KagemushaArtifactRoleV1::InnerStateVkEp => "inner_state_vk_ep",
        KagemushaArtifactRoleV1::StatePkEq => "state_pk_eq",
        KagemushaArtifactRoleV1::StateVkEq => "state_vk_eq",
        KagemushaArtifactRoleV1::StatePkEp => "state_pk_ep",
        KagemushaArtifactRoleV1::StateVkEp => "state_vk_ep",
        KagemushaArtifactRoleV1::MintAuthorizationPkEq => "mint_authorization_pk_eq",
        KagemushaArtifactRoleV1::MintAuthorizationVkEq => "mint_authorization_vk_eq",
        KagemushaArtifactRoleV1::MintAuthorizationPkEp => "mint_authorization_pk_ep",
        KagemushaArtifactRoleV1::MintAuthorizationVkEp => "mint_authorization_vk_ep",
        KagemushaArtifactRoleV1::MintCreditPkEq => "mint_credit_pk_eq",
        KagemushaArtifactRoleV1::MintCreditVkEq => "mint_credit_vk_eq",
        KagemushaArtifactRoleV1::MintCreditPkEp => "mint_credit_pk_ep",
        KagemushaArtifactRoleV1::MintCreditVkEp => "mint_credit_vk_ep",
        KagemushaArtifactRoleV1::PlatformCredentialPkEq => "platform_credential_pk_eq",
        KagemushaArtifactRoleV1::PlatformCredentialVkEq => "platform_credential_vk_eq",
        KagemushaArtifactRoleV1::PlatformCredentialPkEp => "platform_credential_pk_ep",
        KagemushaArtifactRoleV1::PlatformCredentialVkEp => "platform_credential_vk_ep",
        KagemushaArtifactRoleV1::GuardBundlePkEq => "guard_bundle_pk_eq",
        KagemushaArtifactRoleV1::GuardBundleVkEq => "guard_bundle_vk_eq",
        KagemushaArtifactRoleV1::GuardBundlePkEp => "guard_bundle_pk_ep",
        KagemushaArtifactRoleV1::GuardBundleVkEp => "guard_bundle_vk_ep",
        KagemushaArtifactRoleV1::TerminalAuthorizationPkEq => "terminal_authorization_pk_eq",
        KagemushaArtifactRoleV1::TerminalAuthorizationVkEq => "terminal_authorization_vk_eq",
        KagemushaArtifactRoleV1::TerminalAuthorizationPkEp => "terminal_authorization_pk_ep",
        KagemushaArtifactRoleV1::TerminalAuthorizationVkEp => "terminal_authorization_vk_ep",
        KagemushaArtifactRoleV1::CommitWrapperPkEq => "commit_wrapper_pk_eq",
        KagemushaArtifactRoleV1::CommitWrapperVkEq => "commit_wrapper_vk_eq",
        KagemushaArtifactRoleV1::CommitWrapperPkEp => "commit_wrapper_pk_ep",
        KagemushaArtifactRoleV1::CommitWrapperVkEp => "commit_wrapper_vk_ep",
        KagemushaArtifactRoleV1::InnerMintAuthorizationPkEq => "inner_mint_authorization_pk_eq",
        KagemushaArtifactRoleV1::InnerMintAuthorizationVkEq => "inner_mint_authorization_vk_eq",
        KagemushaArtifactRoleV1::InnerMintAuthorizationPkEp => "inner_mint_authorization_pk_ep",
        KagemushaArtifactRoleV1::InnerMintAuthorizationVkEp => "inner_mint_authorization_vk_ep",
        KagemushaArtifactRoleV1::InnerMintCreditPkEq => "inner_mint_credit_pk_eq",
        KagemushaArtifactRoleV1::InnerMintCreditVkEq => "inner_mint_credit_vk_eq",
        KagemushaArtifactRoleV1::InnerMintCreditPkEp => "inner_mint_credit_pk_ep",
        KagemushaArtifactRoleV1::InnerMintCreditVkEp => "inner_mint_credit_vk_ep",
        KagemushaArtifactRoleV1::MintHashShardPkEq => "mint_hash_shard_pk_eq",
        KagemushaArtifactRoleV1::MintHashShardVkEq => "mint_hash_shard_vk_eq",
        KagemushaArtifactRoleV1::MintHashShardPkEp => "mint_hash_shard_pk_ep",
        KagemushaArtifactRoleV1::MintHashShardVkEp => "mint_hash_shard_vk_ep",
        KagemushaArtifactRoleV1::MintHashClaimPkEq => "mint_hash_claim_pk_eq",
        KagemushaArtifactRoleV1::MintHashClaimVkEq => "mint_hash_claim_vk_eq",
        KagemushaArtifactRoleV1::MintHashClaimPkEp => "mint_hash_claim_pk_ep",
        KagemushaArtifactRoleV1::MintHashClaimVkEp => "mint_hash_claim_vk_ep",
    }
}

const fn relation_name(relation: KagemushaQualifiedRelationV1) -> &'static str {
    match relation {
        KagemushaQualifiedRelationV1::Bootstrap => "bootstrap",
        KagemushaQualifiedRelationV1::MintFold => "mint_fold",
        KagemushaQualifiedRelationV1::SendSplit => "send_split",
        KagemushaQualifiedRelationV1::ReceiveFold => "receive_fold",
        KagemushaQualifiedRelationV1::RedeemSplit => "redeem_split",
        KagemushaQualifiedRelationV1::Rotate => "rotate",
        KagemushaQualifiedRelationV1::TerminalAuthorization => "terminal_authorization",
        KagemushaQualifiedRelationV1::CommitWrapper => "commit_wrapper",
    }
}

const fn helper_name(helper: KagemushaQualifiedHelperCircuitV1) -> &'static str {
    match helper {
        KagemushaQualifiedHelperCircuitV1::MintAuthorization => "mint_authorization",
        KagemushaQualifiedHelperCircuitV1::MintCredit => "mint_credit",
        KagemushaQualifiedHelperCircuitV1::PlatformCredential => "platform_credential",
        KagemushaQualifiedHelperCircuitV1::GuardBundle => "guard_bundle",
        KagemushaQualifiedHelperCircuitV1::MintHashShard => "mint_hash_shard",
        KagemushaQualifiedHelperCircuitV1::MintHashClaim => "mint_hash_claim",
    }
}

const HARDWARE_CAPABILITY_NAMES_V1: [&str; 16] = [
    "exact_next_predecessor_consumption",
    "one_use_successor_authorization",
    "rollback_resistant_counter_and_journal",
    "sealed_transition_recovery",
    "receiver_bound_credit_commit",
    "rollback_resistant_accepted_credit_inbox",
    "authenticated_inbound_staging",
    "authoritative_replay_root_recovery",
    "sender_outbox_reservation",
    "authenticated_durable_retry_outbox",
    "atomic_verified_candidate_commit",
    "recoverable_terminal_commit_certificate",
    "trusted_time_or_lease",
    "offline_hardware_epoch_rotation",
    "rollback_safe_counter_rollover",
    "no_software_fallback",
];

const fn device_operation_name(operation: KagemushaDeviceLifecycleOperationV1) -> &'static str {
    match operation {
        KagemushaDeviceLifecycleOperationV1::ReadActiveHardwareCredential => {
            "read_active_hardware_credential"
        }
        KagemushaDeviceLifecycleOperationV1::StageInboundPayment => "stage_inbound_payment",
        KagemushaDeviceLifecycleOperationV1::RecoverStagedInboundPayment => {
            "recover_staged_inbound_payment"
        }
        KagemushaDeviceLifecycleOperationV1::RecoverInboundInboxPage => {
            "recover_inbound_inbox_page"
        }
        KagemushaDeviceLifecycleOperationV1::PrepareExactNextTransition => {
            "prepare_exact_next_transition"
        }
        KagemushaDeviceLifecycleOperationV1::RecoverPreparedTransition => {
            "recover_prepared_transition"
        }
        KagemushaDeviceLifecycleOperationV1::CommitVerifiedCandidateAndSignTerminal => {
            "commit_verified_candidate_and_sign_terminal"
        }
        KagemushaDeviceLifecycleOperationV1::RecoverTerminalOutcome => "recover_terminal_outcome",
        KagemushaDeviceLifecycleOperationV1::InstallTerminalEnvelope => "install_terminal_envelope",
        KagemushaDeviceLifecycleOperationV1::RecoverInstalledEnvelopeOrStateProof => {
            "recover_installed_envelope_or_state_proof"
        }
        KagemushaDeviceLifecycleOperationV1::SignReceiveAcknowledgement => {
            "sign_receive_acknowledgement"
        }
        KagemushaDeviceLifecycleOperationV1::ReleaseOutboxEntry => "release_outbox_entry",
        KagemushaDeviceLifecycleOperationV1::ReadTrustedTimeOrLease => "read_trusted_time_or_lease",
        KagemushaDeviceLifecycleOperationV1::PrepareMintAuthorization => {
            "prepare_mint_authorization"
        }
        KagemushaDeviceLifecycleOperationV1::RecoverMintAuthorization => {
            "recover_mint_authorization"
        }
        KagemushaDeviceLifecycleOperationV1::VerifyAuthorizationAndStageMintCredit => {
            "verify_authorization_and_stage_mint_credit"
        }
        KagemushaDeviceLifecycleOperationV1::FoldReceiveCredit => "fold_receive_credit",
        KagemushaDeviceLifecycleOperationV1::ReadPendingCreditWatermark => {
            "read_pending_credit_watermark"
        }
        KagemushaDeviceLifecycleOperationV1::RotateHardwareEpoch => "rotate_hardware_epoch",
        KagemushaDeviceLifecycleOperationV1::BootstrapAggregateState => "bootstrap_aggregate_state",
        KagemushaDeviceLifecycleOperationV1::RecoverWalletSnapshot => "recover_wallet_snapshot",
        KagemushaDeviceLifecycleOperationV1::CreateSignedPaymentRequest => {
            "create_signed_payment_request"
        }
    }
}

/// Canonical inventory body authenticated by the contract-vector digest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct KagemushaNativeContractVectorBodyV1 {
    /// Sole supported contract-vector version.
    pub version: u16,
    /// Exact number of transported peer messages.
    pub peer_message_count: u16,
    /// Request, payment, and acknowledgement entries in wire order.
    pub peer_messages: Vec<KagemushaNativeContractInventoryEntryV1>,
    /// Exact number of qualified artifact roles.
    pub artifact_role_count: u16,
    /// All qualified artifact-role entries in canonical release order.
    pub artifact_roles: Vec<KagemushaNativeContractInventoryEntryV1>,
    /// Exact number of recursive relations, including the two internal wrappers.
    pub relation_count: u16,
    /// Six monetary relation entries followed by two internal wrapper entries.
    pub relations: Vec<KagemushaNativeContractInventoryEntryV1>,
    /// Exact number of qualified helper circuits.
    pub helper_count: u16,
    /// All helper-circuit entries, including mint-hash shard and mint-hash claim.
    pub helpers: Vec<KagemushaNativeContractInventoryEntryV1>,
    /// Exact number of mandatory non-forking hardware capabilities.
    pub hardware_capability_count: u16,
    /// Required capability mask; every lower V1 bit is mandatory.
    pub required_hardware_capability_mask: u16,
    /// Individual named mandatory capability bits in ascending order.
    pub hardware_capabilities: Vec<KagemushaNativeContractInventoryEntryV1>,
    /// Exact number of secure-device ABI operations.
    pub device_operation_count: u16,
    /// All named secure-device operations in canonical command-frame order.
    pub device_operations: Vec<KagemushaNativeContractInventoryEntryV1>,
}

impl KagemushaNativeContractVectorBodyV1 {
    /// Construct the exact body from authoritative V1 enums and constants.
    #[must_use]
    pub fn canonical() -> Self {
        Self {
            version: KAGEMUSHA_NATIVE_CONTRACT_VECTOR_VERSION_V1,
            peer_message_count: CONNECT_NORITO_KAGEMUSHA_IPM1_MESSAGE_KIND_TAGS_V1.len() as u16,
            peer_messages: [
                KagemushaIpm1PayloadKindV1::Request,
                KagemushaIpm1PayloadKindV1::Payment,
                KagemushaIpm1PayloadKindV1::Acknowledgement,
            ]
            .map(|kind| {
                KagemushaNativeContractInventoryEntryV1::new(
                    u16::from(kind.wire_tag()),
                    peer_message_name(kind),
                )
            })
            .to_vec(),
            artifact_role_count: KagemushaArtifactRoleV1::ALL.len() as u16,
            artifact_roles: KagemushaArtifactRoleV1::ALL
                .map(|role| {
                    KagemushaNativeContractInventoryEntryV1::new(
                        u16::from(role as u8),
                        artifact_role_name(role),
                    )
                })
                .to_vec(),
            relation_count: KagemushaQualifiedRelationV1::ALL.len() as u16,
            relations: KagemushaQualifiedRelationV1::ALL
                .map(|relation| {
                    KagemushaNativeContractInventoryEntryV1::new(
                        u16::from(relation as u8),
                        relation_name(relation),
                    )
                })
                .to_vec(),
            helper_count: KagemushaQualifiedHelperCircuitV1::ALL.len() as u16,
            helpers: KagemushaQualifiedHelperCircuitV1::ALL
                .map(|helper| {
                    KagemushaNativeContractInventoryEntryV1::new(
                        u16::from(helper as u8),
                        helper_name(helper),
                    )
                })
                .to_vec(),
            hardware_capability_count: KAGEMUSHA_NATIVE_HARDWARE_CAPABILITY_BITS_V1.len() as u16,
            required_hardware_capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            hardware_capabilities: KAGEMUSHA_NATIVE_HARDWARE_CAPABILITY_BITS_V1
                .into_iter()
                .zip(HARDWARE_CAPABILITY_NAMES_V1)
                .map(|(bit, name)| KagemushaNativeContractInventoryEntryV1::new(bit, name))
                .collect(),
            device_operation_count: KagemushaDeviceLifecycleOperationV1::ALL.len() as u16,
            device_operations: KagemushaDeviceLifecycleOperationV1::ALL
                .map(|operation| {
                    KagemushaNativeContractInventoryEntryV1::new(
                        u16::from(operation.code()),
                        device_operation_name(operation),
                    )
                })
                .to_vec(),
        }
    }

    fn canonical_digest(&self) -> Result<[u8; 32], KagemushaNativeContractVectorErrorV1> {
        let body = norito::to_bytes(self)
            .map_err(|_| KagemushaNativeContractVectorErrorV1::CanonicalEncoding)?;
        let body_len =
            u64::try_from(body.len()).map_err(|_| KagemushaNativeContractVectorErrorV1::Size)?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DOMAIN_V1);
        hasher.update(body_len.to_le_bytes());
        hasher.update(&body);
        Ok(hasher.finalize().into())
    }
}

/// Canonical native/mobile KAGEMUSHA V1 contract vector.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
pub struct KagemushaNativeContractVectorV1 {
    /// Exact typed inventory body.
    pub body: KagemushaNativeContractVectorBodyV1,
    /// Domain-separated SHA-256 of the canonical body bytes.
    pub contract_digest: [u8; 32],
}

impl KagemushaNativeContractVectorV1 {
    /// Construct the sole compiled V1 vector.
    pub fn canonical() -> Result<Self, KagemushaNativeContractVectorErrorV1> {
        let body = KagemushaNativeContractVectorBodyV1::canonical();
        let contract_digest = body.canonical_digest()?;
        if contract_digest != KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DIGEST_V1 {
            return Err(KagemushaNativeContractVectorErrorV1::DigestMismatch);
        }
        Ok(Self {
            body,
            contract_digest,
        })
    }

    /// Validate exact counts, ordered inventories, and the pinned body digest.
    pub fn validate(&self) -> Result<(), KagemushaNativeContractVectorErrorV1> {
        if self.body != KagemushaNativeContractVectorBodyV1::canonical() {
            return Err(KagemushaNativeContractVectorErrorV1::InventoryMismatch);
        }
        let expected = self.body.canonical_digest()?;
        if self.contract_digest != expected
            || self.contract_digest != KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DIGEST_V1
        {
            return Err(KagemushaNativeContractVectorErrorV1::DigestMismatch);
        }
        Ok(())
    }

    /// Encode this exact vector as canonical Norito bytes.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, KagemushaNativeContractVectorErrorV1> {
        self.validate()?;
        let bytes = norito::to_bytes(self)
            .map_err(|_| KagemushaNativeContractVectorErrorV1::CanonicalEncoding)?;
        if bytes.len() > KAGEMUSHA_NATIVE_CONTRACT_VECTOR_MAX_BYTES_V1 {
            return Err(KagemushaNativeContractVectorErrorV1::Size);
        }
        Ok(bytes)
    }

    /// Decode exact canonical Norito and reject all inventory or digest drift.
    pub fn decode_canonical_exact(
        bytes: &[u8],
    ) -> Result<Self, KagemushaNativeContractVectorErrorV1> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_NATIVE_CONTRACT_VECTOR_MAX_BYTES_V1 {
            return Err(KagemushaNativeContractVectorErrorV1::Size);
        }
        let decoded: Self = norito::decode_from_bytes(bytes)
            .map_err(|_| KagemushaNativeContractVectorErrorV1::CanonicalEncoding)?;
        let canonical = norito::to_bytes(&decoded)
            .map_err(|_| KagemushaNativeContractVectorErrorV1::CanonicalEncoding)?;
        if canonical != bytes {
            return Err(KagemushaNativeContractVectorErrorV1::CanonicalEncoding);
        }
        decoded.validate()?;
        Ok(decoded)
    }
}

/// Return the exact canonical Norito bytes exported to C and JNI consumers.
pub fn kagemusha_native_contract_vector_bytes_v1()
-> Result<Vec<u8>, KagemushaNativeContractVectorErrorV1> {
    KagemushaNativeContractVectorV1::canonical()?.encode_canonical()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(entries: &[KagemushaNativeContractInventoryEntryV1]) -> Vec<u16> {
        entries.iter().map(|entry| entry.code).collect()
    }

    fn names(entries: &[KagemushaNativeContractInventoryEntryV1]) -> Vec<&str> {
        entries.iter().map(|entry| entry.name.as_str()).collect()
    }

    #[test]
    fn canonical_vector_pins_every_authoritative_inventory() {
        let vector = KagemushaNativeContractVectorV1::canonical().expect("canonical vector");
        let body = &vector.body;

        assert_eq!(body.version, 1);
        assert_eq!(body.peer_message_count, 3);
        assert_eq!(codes(&body.peer_messages), [1, 2, 3]);
        assert_eq!(
            names(&body.peer_messages),
            ["payment_request", "payment", "acknowledgement"]
        );
        assert_eq!(body.artifact_role_count, 50);
        assert_eq!(codes(&body.artifact_roles), (0_u16..50).collect::<Vec<_>>());
        assert_eq!(
            names(&body.artifact_roles),
            [
                "params_eq",
                "params_ep",
                "inner_state_pk_eq",
                "inner_state_vk_eq",
                "inner_state_pk_ep",
                "inner_state_vk_ep",
                "state_pk_eq",
                "state_vk_eq",
                "state_pk_ep",
                "state_vk_ep",
                "mint_authorization_pk_eq",
                "mint_authorization_vk_eq",
                "mint_authorization_pk_ep",
                "mint_authorization_vk_ep",
                "mint_credit_pk_eq",
                "mint_credit_vk_eq",
                "mint_credit_pk_ep",
                "mint_credit_vk_ep",
                "platform_credential_pk_eq",
                "platform_credential_vk_eq",
                "platform_credential_pk_ep",
                "platform_credential_vk_ep",
                "guard_bundle_pk_eq",
                "guard_bundle_vk_eq",
                "guard_bundle_pk_ep",
                "guard_bundle_vk_ep",
                "terminal_authorization_pk_eq",
                "terminal_authorization_vk_eq",
                "terminal_authorization_pk_ep",
                "terminal_authorization_vk_ep",
                "commit_wrapper_pk_eq",
                "commit_wrapper_vk_eq",
                "commit_wrapper_pk_ep",
                "commit_wrapper_vk_ep",
                "inner_mint_authorization_pk_eq",
                "inner_mint_authorization_vk_eq",
                "inner_mint_authorization_pk_ep",
                "inner_mint_authorization_vk_ep",
                "inner_mint_credit_pk_eq",
                "inner_mint_credit_vk_eq",
                "inner_mint_credit_pk_ep",
                "inner_mint_credit_vk_ep",
                "mint_hash_shard_pk_eq",
                "mint_hash_shard_vk_eq",
                "mint_hash_shard_pk_ep",
                "mint_hash_shard_vk_ep",
                "mint_hash_claim_pk_eq",
                "mint_hash_claim_vk_eq",
                "mint_hash_claim_pk_ep",
                "mint_hash_claim_vk_ep",
            ]
        );
        assert_eq!(body.relation_count, 8);
        assert_eq!(codes(&body.relations), (0_u16..8).collect::<Vec<_>>());
        assert_eq!(
            names(&body.relations),
            [
                "bootstrap",
                "mint_fold",
                "send_split",
                "receive_fold",
                "redeem_split",
                "rotate",
                "terminal_authorization",
                "commit_wrapper",
            ]
        );
        assert_eq!(body.helper_count, 6);
        assert_eq!(codes(&body.helpers), (0_u16..6).collect::<Vec<_>>());
        assert_eq!(
            names(&body.helpers),
            [
                "mint_authorization",
                "mint_credit",
                "platform_credential",
                "guard_bundle",
                "mint_hash_shard",
                "mint_hash_claim",
            ]
        );
        assert_eq!(body.hardware_capability_count, 16);
        assert_eq!(
            codes(&body.hardware_capabilities),
            (0_u32..16).map(|bit| 1_u16 << bit).collect::<Vec<_>>()
        );
        assert_eq!(
            names(&body.hardware_capabilities),
            [
                "exact_next_predecessor_consumption",
                "one_use_successor_authorization",
                "rollback_resistant_counter_and_journal",
                "sealed_transition_recovery",
                "receiver_bound_credit_commit",
                "rollback_resistant_accepted_credit_inbox",
                "authenticated_inbound_staging",
                "authoritative_replay_root_recovery",
                "sender_outbox_reservation",
                "authenticated_durable_retry_outbox",
                "atomic_verified_candidate_commit",
                "recoverable_terminal_commit_certificate",
                "trusted_time_or_lease",
                "offline_hardware_epoch_rotation",
                "rollback_safe_counter_rollover",
                "no_software_fallback",
            ]
        );
        assert_eq!(
            body.hardware_capabilities
                .iter()
                .fold(0_u16, |mask, entry| mask | entry.code),
            KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1
        );
        assert_eq!(body.required_hardware_capability_mask, 0xffff);
        assert_eq!(body.device_operation_count, 22);
        assert_eq!(
            codes(&body.device_operations),
            [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
            ]
        );
        assert_eq!(
            names(&body.device_operations),
            [
                "read_active_hardware_credential",
                "stage_inbound_payment",
                "recover_staged_inbound_payment",
                "recover_inbound_inbox_page",
                "prepare_exact_next_transition",
                "recover_prepared_transition",
                "commit_verified_candidate_and_sign_terminal",
                "recover_terminal_outcome",
                "install_terminal_envelope",
                "recover_installed_envelope_or_state_proof",
                "sign_receive_acknowledgement",
                "release_outbox_entry",
                "read_trusted_time_or_lease",
                "prepare_mint_authorization",
                "recover_mint_authorization",
                "verify_authorization_and_stage_mint_credit",
                "fold_receive_credit",
                "read_pending_credit_watermark",
                "rotate_hardware_epoch",
                "bootstrap_aggregate_state",
                "recover_wallet_snapshot",
                "create_signed_payment_request",
            ]
        );
    }

    #[test]
    fn canonical_vector_roundtrips_and_rejects_digest_or_inventory_drift() {
        let vector = KagemushaNativeContractVectorV1::canonical().expect("canonical vector");
        assert_eq!(
            vector.contract_digest,
            KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DIGEST_V1
        );
        assert_eq!(
            hex::encode(vector.contract_digest),
            "13b51124f0329fc47b0aa3bf551f83f1806920c9898e7c07cd7f0730eb57fbb9"
        );
        let encoded = vector.encode_canonical().expect("canonical encoding");
        assert!(encoded.len() <= KAGEMUSHA_NATIVE_CONTRACT_VECTOR_MAX_BYTES_V1);
        assert_eq!(
            KagemushaNativeContractVectorV1::decode_canonical_exact(&encoded),
            Ok(vector.clone())
        );

        let mut bad_digest = vector.clone();
        bad_digest.contract_digest[0] ^= 1;
        assert_eq!(
            bad_digest.validate(),
            Err(KagemushaNativeContractVectorErrorV1::DigestMismatch)
        );

        let mut reordered = vector;
        reordered.body.relations.swap(0, 1);
        reordered.contract_digest = reordered
            .body
            .canonical_digest()
            .expect("reordered body remains encodable");
        assert_eq!(
            reordered.validate(),
            Err(KagemushaNativeContractVectorErrorV1::InventoryMismatch)
        );

        let mut noncanonical = encoded;
        noncanonical.push(0);
        assert!(KagemushaNativeContractVectorV1::decode_canonical_exact(&noncanonical).is_err());
    }

    #[test]
    fn c_export_supports_length_probe_and_emits_the_same_canonical_vector() {
        let mut required = usize::MAX;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_contract_vector_v1(
                    core::ptr::null_mut(),
                    0,
                    &mut required,
                )
            },
            crate::ERR_BUFFER_TOO_SMALL
        );
        assert!(required > 0);
        assert!(required <= KAGEMUSHA_NATIVE_CONTRACT_VECTOR_MAX_BYTES_V1);

        let mut output = vec![0_u8; required];
        let mut written = 0;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_contract_vector_v1(
                    output.as_mut_ptr(),
                    output.len(),
                    &mut written,
                )
            },
            0
        );
        assert_eq!(written, output.len());
        assert_eq!(
            KagemushaNativeContractVectorV1::decode_canonical_exact(&output)
                .expect("C export is canonical")
                .contract_digest,
            KAGEMUSHA_NATIVE_CONTRACT_VECTOR_DIGEST_V1
        );
    }

    #[test]
    fn c_header_pins_counts_digest_and_export() {
        let compact: String = include_str!("../include/connect_norito_bridge.h")
            .split_whitespace()
            .collect();
        for (name, count) in [
            ("PEER_MESSAGE", 3),
            ("ARTIFACT_ROLE", 50),
            ("RELATION", 8),
            ("HELPER", 6),
            ("HARDWARE_CAPABILITY", 16),
            ("DEVICE_OPERATION", 22),
        ] {
            assert!(compact.contains(&format!(
                "CONNECT_NORITO_KAGEMUSHA_CONTRACT_VECTOR_{name}_COUNT_V1UINT16_C({count})"
            )));
        }
        assert!(compact.contains(
            "CONNECT_NORITO_KAGEMUSHA_CONTRACT_VECTOR_DIGEST_HEX_V1\\\"13b51124f0329fc47b0aa3bf551f83f1806920c9898e7c07cd7f0730eb57fbb9\""
        ));
        assert!(compact.contains("connect_norito_kagemusha_contract_vector_v1("));
    }
}
