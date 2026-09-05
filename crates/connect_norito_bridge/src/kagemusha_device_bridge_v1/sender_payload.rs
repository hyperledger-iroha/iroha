//! Sender operation/recovery contract for KAGEMUSHA V1 operations 5–10 and 12.
//!
//! These are public-input codecs and binding checks, not a hardware service or
//! a durable index. A qualified native session must authenticate the context,
//! atomically retain operation-ID mappings and tombstones, and obtain Core's
//! verified candidate capability before monetary mutation. Equality of host
//! selectors never provides that capability. Stock dispatch remains unavailable.
//! TODO: integrate these public projections with an authenticated, rollback-safe
//! native operation index and hardware service; never substitute a host map.

use iroha_core::zk::kagemusha_v1_state::{
    DevicePolicyBindingV1, DigestV1, DurableOutgoingEnvelopeV1, HardwareEpochV1,
    HardwareTransitionStatementV1, KAGEMUSHA_OUTGOING_PUBLIC_INPUTS_DOMAIN_V1, KagemushaLaneIdV1,
    KagemushaRedemptionTerminalReceiptV1, KagemushaStateContextV1, KagemushaTransitionKindV1,
    PreparedOutgoingCandidateV1, PreparedOutgoingRecoveryViewV1,
    VerifiedKagemushaRedemptionReleaseV1,
};
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_ASSET_SCALE_MAX_V1,
        KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
        KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1, KagemushaAcknowledgementV1,
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaLifecycleBindingV1,
        KagemushaOperationKindV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
        KagemushaRedemptionVoucherV1, kagemusha_ciphertext_digest_v1,
        kagemusha_liability_pool_id_v1,
    },
};
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};
use sha2::{Digest as _, Sha256};

#[cfg(test)]
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

const VERSION: u16 = 1;
/// Maximum canonical command body, including public exchange and final envelope.
pub const SENDER_COMMAND_MAX_BYTES_V1: usize = 16 * 1024;
/// Maximum canonical reply body; a page holds at most four entries.
pub const SENDER_REPLY_MAX_BYTES_V1: usize = 64 * 1024;
/// Transport bound only; repeated pinned pages support an arbitrary backlog.
pub const SENDER_PAGE_COUNT_MAX_V1: u16 = 4;
/// Domain of the one canonical public-input preimage used by every sender call.
pub const SENDER_INPUTS_DOMAIN_V1: &[u8] = KAGEMUSHA_OUTGOING_PUBLIC_INPUTS_DOMAIN_V1;
const ENVELOPE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:terminal-envelope";
const ACK_DOMAIN: &[u8] = b"iroha:kagemusha:device:v1:accepted-acknowledgement";
const HARDWARE_AUTHORIZATION_DOMAIN: &[u8] = b"iroha:kagemusha:device:v1:hardware-authorization";
/// Domain for the qualified Core-to-hardware P-256 verifier-key reference.
pub const HARDWARE_AUTHORIZATION_KEY_REFERENCE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:device:v1:hardware-authorization-key";
const HARDWARE_AUTHORIZATION_MAX_BYTES_V1: usize = 2 * 1024;

/// Closed failures for shape and replay binding. No variant grants authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SenderErrorV1 {
    /// Empty, oversized or resource-amplifying archive.
    Size,
    /// Wrong schema or noncanonical Norito bytes.
    CanonicalEncoding,
    /// Unsupported operation, bad selector, or mismatched native context.
    Binding,
    /// Public request, envelope, or closed terminal receipt is invalid.
    PublicShape,
    /// An operation ID or durable anchor has been rebound.
    Conflict,
    /// An observation regresses or changes a terminal tombstone.
    StateRegression,
    /// A page is not the exact bounded selection at its pinned revision.
    Snapshot,
}
type Result<T> = std::result::Result<T, SenderErrorV1>;

/// Identity selectors authenticated by the native wallet session.
///
/// The receiver credential in a payment request is a different device's
/// credential. It must not be mistaken for this sender credential/epoch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-wallet-context")]
pub struct SenderWalletContextV1 {
    /// Stable network, device-lane, asset and scale identity.
    pub lane: KagemushaLaneIdV1,
    /// Authenticated proof release, asset incarnation and hardware policy scope.
    pub release: KagemushaStateContextV1,
    /// Native-authenticated sender credential identity.
    pub credential_id: DigestV1,
    /// Full-width hardware generation and its exact epoch identity.
    pub hardware_epoch: HardwareEpochV1,
    /// Native key-reference and policy identity bound to this epoch.
    pub device_policy_binding: DevicePolicyBindingV1,
    /// Release/profile-governed verifier key for Core-to-hardware monetary authorizations.
    pub core_authorization_key_reference: DigestV1,
}

impl SenderWalletContextV1 {
    /// Check canonical public shape and bindings without granting native authority.
    pub fn validate_shape(&self) -> Result<()> {
        for value in [
            self.lane.network_id.as_bytes(),
            &self.lane.device_lane_id,
            &self.release.suite_id,
            &self.release.vk_digest,
            &self.release.release_id,
            &self.release.hardware_profile_id,
            &self.credential_id,
            &self.hardware_epoch.epoch_id,
            &self.device_policy_binding.device_key_reference,
            &self.device_policy_binding.hardware_policy_id,
            &self.core_authorization_key_reference,
        ] {
            nonzero(value)?;
        }
        ensure(
            self.release.protocol_version == VERSION
                && self.release.policy_epoch != 0
                && self.hardware_epoch.generation != 0
                && self.lane.scale <= KAGEMUSHA_ASSET_SCALE_MAX_V1,
        )?;
        self.release
            .asset_incarnation
            .validate()
            .map_err(|_| SenderErrorV1::Binding)?;
        self.lane
            .normalized_asset_id()
            .map_err(|_| SenderErrorV1::Binding)?;
        Ok(())
    }

    /// Compare against a context obtained from authenticated native state.
    /// Passing another host-decoded context does not authenticate either value.
    pub fn validate_against_native(&self, native: &Self) -> Result<()> {
        self.validate_shape()?;
        native.validate_shape()?;
        ensure(self == native)
    }

    /// Check an authenticated retained creation context against the current native
    /// wallet. This establishes scope and ordering only; the native session must
    /// independently authenticate the retained record and its historical authority.
    /// Stable lane and asset incarnation survive ordinary credential/suite rotation.
    pub fn validate_retained_against_native(&self, native: &Self) -> Result<()> {
        self.validate_shape()?;
        native.validate_shape()?;
        ensure(
            self.lane == native.lane
                && self.release.asset_incarnation == native.release.asset_incarnation
                && self.hardware_epoch.generation <= native.hardware_epoch.generation
                && (self.hardware_epoch.generation != native.hardware_epoch.generation
                    || self.hardware_epoch.epoch_id == native.hardware_epoch.epoch_id),
        )
    }

    fn pool_id(&self) -> Result<[u8; 32]> {
        kagemusha_liability_pool_id_v1(
            &self.lane.network_id,
            &self.lane.asset,
            self.release.asset_incarnation,
        )
        .map_err(|_| SenderErrorV1::Binding)
    }

    fn validate_lifecycle(&self, lifecycle: &KagemushaLifecycleBindingV1) -> Result<()> {
        lifecycle
            .validate()
            .map_err(|_| SenderErrorV1::PublicShape)?;
        ensure(
            lifecycle.version == VERSION
                && lifecycle.network_id == self.lane.network_id
                && lifecycle.protocol_version == self.release.protocol_version
                && lifecycle.suite_id == self.release.suite_id
                && lifecycle.vk_digest == self.release.vk_digest
                && lifecycle.release_id == self.release.release_id
                && lifecycle.asset == self.lane.asset
                && lifecycle.asset_incarnation == self.release.asset_incarnation
                && lifecycle.scale == self.lane.scale
                && lifecycle.liability_pool_id == self.pool_id()?
                && lifecycle.hardware_profile_id == self.release.hardware_profile_id
                && lifecycle.policy_epoch == self.release.policy_epoch,
        )
    }
}

/// Only public inputs fixed before outgoing preparation. Variant order is wire order.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-public-inputs")]
pub enum SenderPublicInputsV1 {
    /// Peer payment with a fixed signed receiver exchange.
    SendSplit {
        /// Canonical signed receiver payment request.
        request: Vec<u8>,
    },
    /// Chain-facing redemption with a positive amount and beneficiary.
    RedeemSplit {
        /// Positive exact amount in the asset’s indivisible units.
        amount: u128,
        /// Canonical account receiving the chain redemption.
        beneficiary: AccountId,
    },
}

impl SenderPublicInputsV1 {
    /// Return the outgoing Core operation kind fixed by these public inputs.
    pub fn operation_kind(&self) -> KagemushaOperationKindV1 {
        match self {
            Self::SendSplit { .. } => KagemushaOperationKindV1::SendSplit,
            Self::RedeemSplit { .. } => KagemushaOperationKindV1::RedeemSplit,
        }
    }

    /// Check canonical public shape and bindings without granting native authority.
    pub fn validate_shape(&self, context: &SenderWalletContextV1) -> Result<()> {
        context.validate_shape()?;
        match self {
            Self::SendSplit { .. } => {
                let request = self.send_request()?;
                ensure(
                    request.network_id == context.lane.network_id
                        && request.release_id == context.release.release_id
                        && request.asset == context.lane.asset
                        && request.asset_incarnation == context.release.asset_incarnation
                        && request.scale == context.lane.scale
                        && request.liability_pool_id == context.pool_id()?,
                )
            }
            Self::RedeemSplit { amount, .. } => ensure(*amount != 0),
        }
    }

    fn send_request(&self) -> Result<KagemushaPaymentRequestV1> {
        let Self::SendSplit { request } = self else {
            return Err(SenderErrorV1::Binding);
        };
        bound(request, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
        let request = KagemushaPaymentRequestV1::decode_canonical_exact(request)
            .map_err(|_| SenderErrorV1::PublicShape)?;
        Ok(request)
    }
}

/// Exactly one digest preimage, including the caller's independently generated ID.
/// IDs must be persisted before the first native call, never derived from an amount/request.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-public-input-preimage")]
pub struct SenderPublicInputPreimageV1 {
    /// Canonical payload version; currently one.
    pub version: u16,
    /// Independently generated caller ID persisted before the first native call.
    pub operation_id: DigestV1,
    /// Complete immutable creation scope bound by the public-input digest.
    pub context: SenderWalletContextV1,
    /// Original public inputs fixed before native preparation.
    pub inputs: SenderPublicInputsV1,
}

impl SenderPublicInputPreimageV1 {
    /// SHA256(domain || 00 || u64LE(canonical byte count) || canonical Norito bytes).
    pub fn canonical_digest(&self) -> Result<[u8; 32]> {
        ensure(self.version == VERSION)?;
        nonzero(&self.operation_id)?;
        self.inputs.validate_shape(&self.context)?;
        Ok(digest_bytes(
            SENDER_INPUTS_DOMAIN_V1,
            &encode(self, SENDER_COMMAND_MAX_BYTES_V1)?,
        ))
    }
}

/// An existing native preparation; IDs select retained state, never private inputs.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-preparation-selector")]
pub struct SenderPreparationSelectorV1 {
    /// Digest of the original caller ID, creation context and public inputs.
    pub inputs_digest: [u8; 32],
    /// Exact Core preparation identity, selected without exposing sealed state.
    pub preparation_id: [u8; 32],
}

/// A page cursor is meaningful only under the exact pinned native index revision.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-recovery-selector")]
pub enum SenderRecoverySelectorV1 {
    /// Recover one caller-known operation and its exact input binding.
    Lookup {
        /// Digest of the original caller ID, creation context and public inputs.
        inputs_digest: [u8; 32],
    },
    /// Recover a bounded prefix from a stable, revision-pinned native index.
    Page {
        /// Pinned stable-wallet index revision; required after the first page.
        snapshot_revision: Option<u128>,
        /// Exclusive operation-ID cursor at the pinned revision.
        after: Option<[u8; 32]>,
        /// Requested page count, from one through four.
        maximum_entries: u16,
    },
}

/// Closed terminal receipt selector for native outbox release.
///
/// A redemption projection is public binding material only. It cannot authorize
/// release without a matching in-process
/// [`VerifiedKagemushaRedemptionReleaseV1`] constructed by Core from the full
/// finalized operation status and a caller-pinned trust anchor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-terminal-receipt")]
pub enum SenderTerminalReceiptV1 {
    /// Exact durable acknowledgement for the installed peer payment.
    PaymentAcknowledgement(Vec<u8>),
    /// Compact selector for a separately Core-authenticated redemption settlement.
    RedemptionSettlement(KagemushaRedemptionTerminalReceiptV1),
}

/// Closed purpose of a release-pinned Core-to-hardware authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-hardware-authorization-purpose")]
pub enum SenderHardwareAuthorizationPurposeV1 {
    /// Authorize exact-once commitment of one already verified candidate.
    Commit,
    /// Authorize removal of one installed retry envelope after its terminal receipt.
    Release,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-hardware-authorization-preimage")]
struct SenderHardwareAuthorizationPreimageV1 {
    version: u16,
    purpose: SenderHardwareAuthorizationPurposeV1,
    operation_id: [u8; 32],
    inputs_digest: [u8; 32],
    preparation_id: [u8; 32],
    candidate_digest: [u8; 32],
    release_id: [u8; 32],
    hardware_transition_statement: HardwareTransitionStatementV1,
    prepared_one_use_authorization_digest: [u8; 32],
    outbox_reservation_commitment: [u8; 32],
    outcome_id: [u8; 32],
    transition_nullifier: [u8; 32],
    envelope_digest: Option<[u8; 32]>,
    terminal_receipt_digest: Option<[u8; 32]>,
    hardware_one_use_nonce: [u8; 32],
    authorization_public_key: KagemushaDevicePublicKeyV1,
}

/// Hardware-verifiable authorization emitted only after Core admits the exact candidate or receipt.
///
/// Publicly reproducing these fields grants no authority. Qualified hardware must authenticate
/// `authenticator` under the release-pinned Core-to-hardware verifier and consume
/// `authorization_nonce` exactly once before committing or releasing state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-hardware-authorization")]
pub struct SenderHardwareAuthorizationV1 {
    /// Canonical authorization version.
    pub version: u16,
    /// Commit or release authority; no catch-all purpose exists.
    pub purpose: SenderHardwareAuthorizationPurposeV1,
    /// Caller-persisted operation identity.
    pub operation_id: [u8; 32],
    /// Complete immutable sender input digest.
    pub inputs_digest: [u8; 32],
    /// Exact retained preparation identity.
    pub preparation_id: [u8; 32],
    /// Exact Core-verified candidate identity.
    pub candidate_digest: [u8; 32],
    /// Proof-release identity pinned by the hardware session.
    pub release_id: [u8; 32],
    /// Complete exact-next statement recomputed from the hardware's sealed preparation.
    pub hardware_transition_statement: HardwareTransitionStatementV1,
    /// Digest of the hardware-prepared one-use predecessor authorization.
    pub prepared_one_use_authorization_digest: [u8; 32],
    /// Exact one-use durable outbox reservation consumed by this transition.
    pub outbox_reservation_commitment: [u8; 32],
    /// Exact credit or redemption identity produced by the transition.
    pub outcome_id: [u8; 32],
    /// Exact proof-derived transition or terminal nullifier.
    pub transition_nullifier: [u8; 32],
    /// Installed envelope identity, present only for release.
    pub envelope_digest: Option<[u8; 32]>,
    /// Accepted terminal receipt identity, present only for release.
    pub terminal_receipt_digest: Option<[u8; 32]>,
    /// Hardware-issued, operation-scoped challenge retained before Core signs.
    pub hardware_one_use_nonce: [u8; 32],
    /// Sole canonical P-256 Core authorization key pinned by the hardware policy.
    pub authorization_public_key: KagemushaDevicePublicKeyV1,
    /// Domain-separated digest authenticated by the qualified platform channel.
    pub authorization_id: [u8; 32],
    /// Low-S P-256 signature over `authorization_id` by `authorization_public_key`.
    pub authenticator: KagemushaDeviceSignatureV1,
}

impl SenderHardwareAuthorizationV1 {
    /// Decode and validate exact bounded canonical authorization bytes.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let authorization: Self = exact(bytes, HARDWARE_AUTHORIZATION_MAX_BYTES_V1)?;
        authorization.validate_shape()?;
        Ok(authorization)
    }

    fn validate_shape(&self) -> Result<()> {
        ensure(self.version == VERSION)?;
        for digest in [
            self.operation_id,
            self.inputs_digest,
            self.preparation_id,
            self.candidate_digest,
            self.release_id,
            self.prepared_one_use_authorization_digest,
            self.outbox_reservation_commitment,
            self.outcome_id,
            self.transition_nullifier,
            self.hardware_one_use_nonce,
            self.authorization_id,
        ] {
            nonzero(&digest)?;
        }
        self.authorization_public_key
            .validate()
            .map_err(|_| SenderErrorV1::PublicShape)?;
        ensure(match self.purpose {
            SenderHardwareAuthorizationPurposeV1::Commit => {
                self.envelope_digest.is_none() && self.terminal_receipt_digest.is_none()
            }
            SenderHardwareAuthorizationPurposeV1::Release => {
                self.envelope_digest.is_some() && self.terminal_receipt_digest.is_some()
            }
        })?;
        for digest in [self.envelope_digest, self.terminal_receipt_digest]
            .into_iter()
            .flatten()
        {
            nonzero(&digest)?;
        }
        ensure(self.authorization_id == self.expected_authorization_id()?)?;
        self.authenticator
            .verify(&self.authorization_public_key, &self.authorization_id)
            .map_err(|_| SenderErrorV1::Binding)
    }

    fn expected_authorization_id(&self) -> Result<[u8; 32]> {
        let preimage = SenderHardwareAuthorizationPreimageV1 {
            version: self.version,
            purpose: self.purpose,
            operation_id: self.operation_id,
            inputs_digest: self.inputs_digest,
            preparation_id: self.preparation_id,
            candidate_digest: self.candidate_digest,
            release_id: self.release_id,
            hardware_transition_statement: self.hardware_transition_statement.clone(),
            prepared_one_use_authorization_digest: self.prepared_one_use_authorization_digest,
            outbox_reservation_commitment: self.outbox_reservation_commitment,
            outcome_id: self.outcome_id,
            transition_nullifier: self.transition_nullifier,
            envelope_digest: self.envelope_digest,
            terminal_receipt_digest: self.terminal_receipt_digest,
            hardware_one_use_nonce: self.hardware_one_use_nonce,
            authorization_public_key: self.authorization_public_key,
        };
        Ok(digest_bytes(
            HARDWARE_AUTHORIZATION_DOMAIN,
            &encode(&preimage, HARDWARE_AUTHORIZATION_MAX_BYTES_V1)?,
        ))
    }
}

/// Compute the exact Core-authorization key reference carried beside the policy binding.
#[must_use]
pub fn hardware_authorization_key_reference_v1(
    public_key: &KagemushaDevicePublicKeyV1,
) -> [u8; 32] {
    digest_bytes(
        HARDWARE_AUTHORIZATION_KEY_REFERENCE_DOMAIN_V1,
        public_key.as_sec1_bytes(),
    )
}

fn validate_hardware_authorization_statement(
    authorization: &SenderHardwareAuthorizationV1,
    context: &SenderWalletContextV1,
    inputs: Option<&SenderPublicInputsV1>,
) -> Result<()> {
    let statement = &authorization.hardware_transition_statement;
    ensure(
        statement.version == VERSION
            && statement.lane == context.lane
            && statement.predecessor_epoch == context.hardware_epoch
            && statement.successor_epoch == context.hardware_epoch
            && statement.predecessor_device_policy_binding == context.device_policy_binding
            && statement.successor_device_policy_binding == context.device_policy_binding
            && hardware_authorization_key_reference_v1(&authorization.authorization_public_key)
                == context.core_authorization_key_reference
            && statement.predecessor_commitment != [0; 32]
            && statement.successor_commitment != [0; 32]
            && statement.predecessor_state_nonce_commitment != [0; 32]
            && statement.successor_state_nonce_commitment != [0; 32]
            && statement.predecessor_state_nonce_commitment
                != statement.successor_state_nonce_commitment
            && statement.state_transition_digest != [0; 32]
            && statement.normalized_guard_statement_digest != [0; 32]
            && statement.successor_sequence
                == statement
                    .predecessor_sequence
                    .checked_add(1)
                    .ok_or(SenderErrorV1::Binding)?
            && statement.journal_revision_after
                == statement
                    .journal_revision_before
                    .checked_add(1)
                    .ok_or(SenderErrorV1::Binding)?,
    )?;
    if let Some(inputs) = inputs {
        let (kind, amount) = match inputs {
            SenderPublicInputsV1::SendSplit { .. } => (
                KagemushaTransitionKindV1::SendSplit,
                inputs.send_request()?.amount,
            ),
            SenderPublicInputsV1::RedeemSplit { amount, .. } => {
                (KagemushaTransitionKindV1::RedeemSplit, *amount)
            }
        };
        ensure(statement.kind == kind && statement.amount == amount && amount != 0)?;
    } else {
        ensure(matches!(
            statement.kind,
            KagemushaTransitionKindV1::SendSplit | KagemushaTransitionKindV1::RedeemSplit
        ))?;
    }
    Ok(())
}

/// Distinct sender bodies. Variant ordinal is not the ABI operation code.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-command-body")]
pub enum SenderCommandBodyV1 {
    /// Prepare one new exact outgoing transition under current native authority.
    Prepare {
        /// Original public inputs fixed before native preparation.
        inputs: SenderPublicInputsV1,
    },
    /// Observe preparation or later progress using the original input digest.
    RecoverPrepared {
        /// Digest of the original caller ID, creation context and public inputs.
        inputs_digest: [u8; 32],
    },
    /// Select the retained verified candidate without host-supplied proof authority.
    Commit {
        /// Original input digest and retained preparation identity.
        selector: SenderPreparationSelectorV1,
        /// Exact persisted Core candidate digest.
        candidate_digest: [u8; 32],
        /// Canonical Core authorization which qualified hardware authenticates and consumes once.
        hardware_authorization: Vec<u8>,
    },
    /// Observe the exact committed outcome or later retained phase.
    RecoverTerminal {
        /// Digest of the original caller ID, creation context and public inputs.
        inputs_digest: [u8; 32],
    },
    /// Select the exact final envelope after candidate commitment.
    Install {
        /// Original input digest and retained preparation identity.
        selector: SenderPreparationSelectorV1,
        /// Exact persisted Core candidate digest.
        candidate_digest: [u8; 32],
        /// Original public inputs fixed before native preparation.
        inputs: SenderPublicInputsV1,
        /// Exact canonical terminal envelope bytes.
        envelope: Vec<u8>,
    },
    /// Recover installed bytes or a bounded operation-index page.
    RecoverInstalled {
        /// Original input digest and retained preparation identity.
        selector: SenderRecoverySelectorV1,
    },
    /// Select the exact installed envelope and its matching terminal receipt.
    Release {
        /// Digest of the original caller ID, creation context and public inputs.
        inputs_digest: [u8; 32],
        /// Domain-separated digest of the exact terminal envelope.
        envelope_digest: [u8; 32],
        /// Original public inputs fixed before native preparation.
        inputs: SenderPublicInputsV1,
        /// Exact canonical terminal envelope bytes.
        envelope: Vec<u8>,
        /// Closed terminal receipt selected for the outgoing operation kind.
        terminal_receipt: SenderTerminalReceiptV1,
        /// Canonical Core authorization bound to this exact terminal receipt.
        hardware_authorization: Vec<u8>,
    },
}

impl SenderCommandBodyV1 {
    /// Return the closed ABI operation code for this body variant.
    pub fn operation(&self) -> u8 {
        match self {
            Self::Prepare { .. } => 5,
            Self::RecoverPrepared { .. } => 6,
            Self::Commit { .. } => 7,
            Self::RecoverTerminal { .. } => 8,
            Self::Install { .. } => 9,
            Self::RecoverInstalled { .. } => 10,
            Self::Release { .. } => 12,
        }
    }
}

/// Every single-operation request repeats the outer caller-known operation ID.
/// For a page it is a query ID, while each returned record retains its own ID.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-command")]
pub struct SenderCommandV1 {
    /// Canonical payload version; currently one.
    pub version: u16,
    /// Closed outer ABI operation code; must match the body variant.
    pub operation: u8,
    /// Independently generated caller ID persisted before the first native call.
    pub operation_id: [u8; 32],
    /// Immutable creation context for a single operation; current context for a page.
    pub context: SenderWalletContextV1,
    /// Operation-specific canonical public body.
    pub body: SenderCommandBodyV1,
}

impl SenderCommandV1 {
    /// Decode exact bounded canonical bytes and bind the supplied outer selectors.
    pub fn decode_canonical_exact(
        operation: u8,
        request_id: [u8; 32],
        bytes: &[u8],
    ) -> Result<Self> {
        let value: Self = exact(bytes, SENDER_COMMAND_MAX_BYTES_V1)?;
        ensure(value.operation == operation && value.operation_id == request_id)?;
        value.validate_shape()?;
        Ok(value)
    }

    /// Validate and encode a bounded canonical body.
    pub fn encode_canonical(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        encode(self, SENDER_COMMAND_MAX_BYTES_V1)
    }

    fn digest_for(&self, inputs: &SenderPublicInputsV1) -> Result<[u8; 32]> {
        SenderPublicInputPreimageV1 {
            version: VERSION,
            operation_id: self.operation_id,
            context: self.context.clone(),
            inputs: inputs.clone(),
        }
        .canonical_digest()
    }

    /// Return the original single-operation input digest, or None for a page query.
    pub fn expected_inputs_digest(&self) -> Result<Option<[u8; 32]>> {
        Ok(match &self.body {
            SenderCommandBodyV1::Prepare { inputs } => Some(self.digest_for(inputs)?),
            SenderCommandBodyV1::RecoverPrepared { inputs_digest }
            | SenderCommandBodyV1::RecoverTerminal { inputs_digest }
            | SenderCommandBodyV1::Release { inputs_digest, .. }
            | SenderCommandBodyV1::RecoverInstalled {
                selector: SenderRecoverySelectorV1::Lookup { inputs_digest },
            } => Some(*inputs_digest),
            SenderCommandBodyV1::Commit { selector, .. }
            | SenderCommandBodyV1::Install { selector, .. } => Some(selector.inputs_digest),
            SenderCommandBodyV1::RecoverInstalled {
                selector: SenderRecoverySelectorV1::Page { .. },
            } => None,
        })
    }

    /// Check canonical public shape and bindings without granting native authority.
    pub fn validate_shape(&self) -> Result<()> {
        ensure(self.version == VERSION && self.operation == self.body.operation())?;
        nonzero(&self.operation_id)?;
        self.context.validate_shape()?;
        if let Some(digest) = self.expected_inputs_digest()? {
            nonzero(&digest)?;
        }
        match &self.body {
            SenderCommandBodyV1::Prepare { inputs } => inputs.validate_shape(&self.context),
            SenderCommandBodyV1::Commit {
                selector,
                candidate_digest,
                hardware_authorization,
            } => {
                nonzero(&selector.preparation_id)?;
                nonzero(candidate_digest)?;
                let authorization =
                    SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)?;
                validate_hardware_authorization_statement(&authorization, &self.context, None)?;
                ensure(
                    authorization.purpose == SenderHardwareAuthorizationPurposeV1::Commit
                        && authorization.operation_id == self.operation_id
                        && authorization.inputs_digest == selector.inputs_digest
                        && authorization.preparation_id == selector.preparation_id
                        && authorization.candidate_digest == *candidate_digest
                        && authorization.release_id == self.context.release.release_id,
                )
            }
            SenderCommandBodyV1::Install {
                selector,
                candidate_digest,
                inputs,
                envelope,
            } => {
                nonzero(&selector.preparation_id)?;
                nonzero(candidate_digest)?;
                ensure(self.digest_for(inputs)? == selector.inputs_digest)?;
                let metadata = envelope_metadata(inputs, &self.context, envelope)?;
                ensure(metadata.candidate_digest == *candidate_digest)
            }
            SenderCommandBodyV1::Release {
                inputs_digest,
                envelope_digest,
                inputs,
                envelope,
                terminal_receipt,
                hardware_authorization,
            } => {
                ensure(self.digest_for(inputs)? == *inputs_digest)?;
                let metadata = envelope_metadata(inputs, &self.context, envelope)?;
                ensure(terminal_envelope_digest_v1(envelope)? == *envelope_digest)?;
                let receipt_digest = accepted_terminal_receipt_digest(
                    self.operation_id,
                    inputs,
                    envelope,
                    &metadata,
                    terminal_receipt,
                )?;
                let authorization =
                    SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)?;
                validate_hardware_authorization_statement(
                    &authorization,
                    &self.context,
                    Some(inputs),
                )?;
                ensure(
                    authorization.purpose == SenderHardwareAuthorizationPurposeV1::Release
                        && authorization.operation_id == self.operation_id
                        && authorization.inputs_digest == *inputs_digest
                        && authorization.release_id == self.context.release.release_id
                        && authorization.outcome_id == metadata.outcome_id
                        && authorization.transition_nullifier == metadata.terminal_nullifier
                        && authorization.envelope_digest == Some(*envelope_digest)
                        && authorization.terminal_receipt_digest == Some(receipt_digest),
                )
            }
            SenderCommandBodyV1::RecoverInstalled {
                selector:
                    SenderRecoverySelectorV1::Page {
                        snapshot_revision,
                        after,
                        maximum_entries,
                    },
            } => {
                ensure((1..=SENDER_PAGE_COUNT_MAX_V1).contains(maximum_entries))?;
                if let Some(after) = after {
                    nonzero(after)?;
                    ensure(snapshot_revision.is_some())?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// Observed native phase; Missing is represented only by an authenticated absent lookup.
/// Ordinals are stable wire tags, not permission levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-phase")]
pub enum SenderPhaseV1 {
    /// Exact preparation and outbox reservation are durably retained.
    Prepared,
    /// Verified candidate identity is durably retained before commitment.
    CandidatePersisted,
    /// Hardware commitment and its certificate identity are retained.
    Committed,
    /// Canonical terminal envelope is installed and may be recovered.
    Installed,
    /// Terminal receipt is accepted and immutable replay anchors remain.
    Released,
}

/// Public durable-index projection. It contains no sealed inputs or private proof state.
/// Released records retain immutable replay anchors and discard input bytes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-record")]
pub struct SenderRecordV1 {
    /// Independently generated caller ID persisted before the first native call.
    pub operation_id: [u8; 32],
    /// Authenticated immutable creation context, never rewritten during rotation.
    pub context: SenderWalletContextV1,
    /// Digest of the original caller ID, creation context and public inputs.
    pub inputs_digest: [u8; 32],
    /// Peer send or chain redemption; other transition kinds reject.
    pub operation_kind: KagemushaOperationKindV1,
    /// Exact Core preparation identity, selected without exposing sealed state.
    pub preparation_id: [u8; 32],
    /// Exact native reservation retained before predecessor commitment.
    pub outbox_reservation_id: [u8; 32],
    /// Core-derived credit ID for a send, redemption ID for a redemption.
    pub outcome_id: [u8; 32],
    /// Observed durable phase; does not confer mutation authority.
    pub phase: SenderPhaseV1,
    /// Native index revision, distinct from the monetary journal revision.
    pub record_revision: u128,
    /// Original public inputs fixed before native preparation.
    pub inputs: Option<SenderPublicInputsV1>,
    /// Exact persisted Core candidate digest.
    pub candidate_digest: Option<[u8; 32]>,
    /// Exact terminal certificate identity, retained from Committed onward.
    pub commit_certificate_digest: Option<[u8; 32]>,
    /// Domain-separated digest of the exact terminal envelope.
    pub envelope_digest: Option<[u8; 32]>,
    /// Exact accepted terminal receipt identity retained by Released.
    pub terminal_receipt_digest: Option<[u8; 32]>,
}

impl SenderRecordV1 {
    /// Check canonical public shape and bindings without granting native authority.
    pub fn validate_shape(&self, native_context: &SenderWalletContextV1) -> Result<()> {
        self.context
            .validate_retained_against_native(native_context)?;
        for value in [
            &self.operation_id,
            &self.inputs_digest,
            &self.preparation_id,
            &self.outbox_reservation_id,
            &self.outcome_id,
        ] {
            nonzero(value)?;
        }
        for value in [
            self.candidate_digest,
            self.commit_certificate_digest,
            self.envelope_digest,
            self.terminal_receipt_digest,
        ]
        .iter()
        .flatten()
        {
            nonzero(value)?;
        }
        ensure(
            self.record_revision != 0
                && matches!(
                    self.operation_kind,
                    KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
                ),
        )?;
        let terminal = matches!(self.phase, SenderPhaseV1::Released);
        ensure(self.inputs.is_some() != terminal)?;
        if let Some(inputs) = &self.inputs {
            ensure(inputs.operation_kind() == self.operation_kind)?;
            let digest = SenderPublicInputPreimageV1 {
                version: VERSION,
                operation_id: self.operation_id,
                context: self.context.clone(),
                inputs: inputs.clone(),
            }
            .canonical_digest()?;
            ensure(digest == self.inputs_digest)?;
        }
        let shape = (
            self.candidate_digest.is_some(),
            self.commit_certificate_digest.is_some(),
            self.envelope_digest.is_some(),
            self.terminal_receipt_digest.is_some(),
        );
        ensure(match self.phase {
            SenderPhaseV1::Prepared => shape == (false, false, false, false),
            SenderPhaseV1::CandidatePersisted => shape == (true, false, false, false),
            SenderPhaseV1::Committed => shape == (true, true, false, false),
            SenderPhaseV1::Installed => shape == (true, true, true, false),
            SenderPhaseV1::Released => shape == (true, true, true, true),
        })
    }
}

/// Exact final bytes exist only in operation 10's Installed result.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-recovery-item")]
pub struct SenderRecoveryItemV1 {
    /// Authenticated native operation-index projection.
    pub record: SenderRecordV1,
    /// Final bytes only for operation 10 Installed results; otherwise empty.
    pub canonical_envelope: Vec<u8>,
}

/// A tombstone-aware native lookup or exact bounded index page.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-reply-body")]
pub enum SenderReplyBodyV1 {
    /// None means an authenticated tombstone-aware native lookup found no operation.
    /// Empty transport output and transport errors must never be converted to None.
    Lookup(Option<SenderRecoveryItemV1>),
    /// Recover a bounded prefix from a stable, revision-pinned native index.
    Page {
        /// Exact ordered bounded prefix selected from the authenticated native index.
        entries: Vec<SenderRecoveryItemV1>,
        /// Last returned operation ID only when additional entries exist.
        next_cursor: Option<[u8; 32]>,
    },
}

/// Canonical reply authenticated under the current native wallet session.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sender-reply")]
pub struct SenderReplyV1 {
    /// Canonical payload version; currently one.
    pub version: u16,
    /// Closed outer ABI operation code; must match the body variant.
    pub operation: u8,
    /// Exact outer command ID; page queries have an independent ID.
    pub request_id: [u8; 32],
    /// Current authenticated native wallet context, including after rotation.
    pub context: SenderWalletContextV1,
    /// Stable-wallet index revision; does not restart at a hardware rotation.
    pub index_revision: u128,
    /// Operation-specific canonical public body.
    pub body: SenderReplyBodyV1,
}

impl SenderReplyV1 {
    /// Called only after authenticating the complete response and its native context.
    pub fn decode_canonical_exact(
        command: &SenderCommandV1,
        native_context: &SenderWalletContextV1,
        bytes: &[u8],
    ) -> Result<Self> {
        let value: Self = exact(bytes, SENDER_REPLY_MAX_BYTES_V1)?;
        value.validate_against(command, native_context)?;
        Ok(value)
    }

    /// Validate and encode a bounded canonical body.
    pub fn encode_canonical(
        &self,
        command: &SenderCommandV1,
        native_context: &SenderWalletContextV1,
    ) -> Result<Vec<u8>> {
        self.validate_against(command, native_context)?;
        encode(self, SENDER_REPLY_MAX_BYTES_V1)
    }

    /// Bind this reply to its exact command and current authenticated native context.
    pub fn validate_against(
        &self,
        command: &SenderCommandV1,
        native_context: &SenderWalletContextV1,
    ) -> Result<()> {
        command.validate_shape()?;
        if matches!(
            command.body,
            SenderCommandBodyV1::Prepare { .. }
                | SenderCommandBodyV1::RecoverInstalled {
                    selector: SenderRecoverySelectorV1::Page { .. }
                }
        ) {
            // A historical selector can recover existing work, never prepare new work.
            command.context.validate_against_native(native_context)?;
        } else {
            command
                .context
                .validate_retained_against_native(native_context)?;
        }
        self.context.validate_against_native(native_context)?;
        ensure(
            self.version == VERSION
                && self.operation == command.operation
                && self.request_id == command.operation_id,
        )?;
        match (&command.body, &self.body) {
            (
                SenderCommandBodyV1::RecoverInstalled {
                    selector:
                        SenderRecoverySelectorV1::Page {
                            snapshot_revision,
                            after,
                            maximum_entries,
                        },
                },
                SenderReplyBodyV1::Page {
                    entries,
                    next_cursor,
                },
            ) => {
                if snapshot_revision.is_some_and(|revision| revision != self.index_revision)
                    || entries.len() > usize::from(*maximum_entries)
                {
                    return Err(SenderErrorV1::Snapshot);
                }
                let mut previous = *after;
                for entry in entries {
                    self.validate_item(entry)?;
                    if previous.is_some_and(|id| id >= entry.record.operation_id) {
                        return Err(SenderErrorV1::Snapshot);
                    }
                    previous = Some(entry.record.operation_id);
                }
                if let Some(cursor) = next_cursor {
                    if entries.len() != usize::from(*maximum_entries)
                        || entries.last().map(|entry| entry.record.operation_id) != Some(*cursor)
                    {
                        return Err(SenderErrorV1::Snapshot);
                    }
                }
                Ok(())
            }
            (
                SenderCommandBodyV1::RecoverInstalled {
                    selector: SenderRecoverySelectorV1::Page { .. },
                },
                _,
            )
            | (_, SenderReplyBodyV1::Page { .. }) => Err(SenderErrorV1::Binding),
            (_, SenderReplyBodyV1::Lookup(Some(item))) => {
                self.validate_item(item)?;
                validate_existing_operation_v1(command, &item.record)?;
                ensure(item.record.operation_id == command.operation_id)
            }
            (_, SenderReplyBodyV1::Lookup(None)) => ensure(matches!(
                command.body,
                SenderCommandBodyV1::RecoverPrepared { .. }
                    | SenderCommandBodyV1::RecoverTerminal { .. }
                    | SenderCommandBodyV1::RecoverInstalled {
                        selector: SenderRecoverySelectorV1::Lookup { .. }
                    }
            )),
        }
    }

    fn validate_item(&self, item: &SenderRecoveryItemV1) -> Result<()> {
        item.record.validate_shape(&self.context)?;
        if item.record.record_revision > self.index_revision {
            return Err(SenderErrorV1::Snapshot);
        }
        if self.operation == 10 && item.record.phase == SenderPhaseV1::Installed {
            validate_installed_bytes(&item.record, &item.canonical_envelope)
        } else {
            ensure(item.canonical_envelope.is_empty())
        }
    }

    /// Compare the response with the exact native index selection, including its
    /// end marker. A self-consistent host page cannot prove absence of omitted
    /// entries. The qualified index must select the bounded prefix atomically.
    pub fn validate_native_page_selection(
        &self,
        command: &SenderCommandV1,
        native_context: &SenderWalletContextV1,
        native_revision: u128,
        expected_entries: &[SenderRecoveryItemV1],
        has_more: bool,
    ) -> Result<()> {
        self.validate_against(command, native_context)?;
        if self.index_revision != native_revision {
            return Err(SenderErrorV1::Snapshot);
        }
        let SenderReplyBodyV1::Page {
            entries,
            next_cursor,
        } = &self.body
        else {
            return Err(SenderErrorV1::Snapshot);
        };
        let expected_cursor = if has_more {
            Some(
                expected_entries
                    .last()
                    .ok_or(SenderErrorV1::Snapshot)?
                    .record
                    .operation_id,
            )
        } else {
            None
        };
        if entries != expected_entries || *next_cursor != expected_cursor {
            return Err(SenderErrorV1::Snapshot);
        }
        Ok(())
    }

    /// Match an exact native tombstone-aware lookup at the authenticated revision.
    /// In particular, an omitted installed record cannot become a Missing result.
    pub fn validate_native_lookup_selection(
        &self,
        command: &SenderCommandV1,
        native_context: &SenderWalletContextV1,
        native_revision: u128,
        expected: Option<&SenderRecoveryItemV1>,
    ) -> Result<()> {
        self.validate_against(command, native_context)?;
        let SenderReplyBodyV1::Lookup(actual) = &self.body else {
            return Err(SenderErrorV1::Snapshot);
        };
        if self.index_revision != native_revision || actual.as_ref() != expected {
            return Err(SenderErrorV1::Snapshot);
        }
        Ok(())
    }
}

/// Bind a retry or observation to an existing native record, including tombstones.
/// This only checks selectors. It never approves prepare/commit/install/release.
pub fn validate_existing_operation_v1(
    command: &SenderCommandV1,
    record: &SenderRecordV1,
) -> Result<()> {
    command.validate_shape()?;
    record.validate_shape(&command.context)?;
    if command.context != record.context
        || command.operation_id != record.operation_id
        || command.expected_inputs_digest()? != Some(record.inputs_digest)
    {
        return Err(SenderErrorV1::Conflict);
    }
    match &command.body {
        SenderCommandBodyV1::Commit { selector, .. }
        | SenderCommandBodyV1::Install { selector, .. } => {
            if selector.preparation_id != record.preparation_id {
                return Err(SenderErrorV1::Conflict);
            }
        }
        _ => {}
    }
    match &command.body {
        SenderCommandBodyV1::Commit {
            candidate_digest, ..
        }
        | SenderCommandBodyV1::Install {
            candidate_digest, ..
        } => {
            if record.candidate_digest != Some(*candidate_digest) {
                return Err(SenderErrorV1::Conflict);
            }
        }
        _ => {}
    }
    match &command.body {
        SenderCommandBodyV1::Commit {
            hardware_authorization,
            ..
        } => {
            let authorization =
                SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)?;
            validate_hardware_authorization_statement(
                &authorization,
                &record.context,
                record.inputs.as_ref(),
            )?;
            ensure(
                authorization.preparation_id == record.preparation_id
                    && authorization.candidate_digest
                        == record.candidate_digest.ok_or(SenderErrorV1::Conflict)?
                    && authorization.outcome_id == record.outcome_id,
            )?;
            ensure(matches!(
                record.phase,
                SenderPhaseV1::CandidatePersisted
                    | SenderPhaseV1::Committed
                    | SenderPhaseV1::Installed
                    | SenderPhaseV1::Released
            ))
        }
        SenderCommandBodyV1::Install { envelope, .. } => {
            ensure(matches!(
                record.phase,
                SenderPhaseV1::Committed | SenderPhaseV1::Installed | SenderPhaseV1::Released
            ))?;
            let digest = terminal_envelope_digest_v1(envelope)?;
            if record
                .envelope_digest
                .is_some_and(|expected| expected != digest)
            {
                return Err(SenderErrorV1::Conflict);
            }
            let SenderCommandBodyV1::Install { inputs, .. } = &command.body else {
                unreachable!()
            };
            let metadata = envelope_metadata(inputs, &record.context, envelope)?;
            ensure(
                metadata.outcome_id == record.outcome_id
                    && Some(metadata.commit_certificate_digest) == record.commit_certificate_digest,
            )
        }
        SenderCommandBodyV1::Release {
            envelope_digest,
            inputs,
            envelope,
            terminal_receipt,
            hardware_authorization,
            ..
        } => {
            ensure(matches!(
                record.phase,
                SenderPhaseV1::Installed | SenderPhaseV1::Released
            ))?;
            if record.envelope_digest != Some(*envelope_digest) {
                return Err(SenderErrorV1::Conflict);
            }
            let metadata = envelope_metadata(inputs, &record.context, envelope)?;
            if metadata.outcome_id != record.outcome_id
                || record.candidate_digest != Some(metadata.candidate_digest)
                || record.commit_certificate_digest != Some(metadata.commit_certificate_digest)
            {
                return Err(SenderErrorV1::Conflict);
            }
            let terminal_receipt_digest = accepted_terminal_receipt_digest(
                command.operation_id,
                inputs,
                envelope,
                &metadata,
                terminal_receipt,
            )?;
            let authorization =
                SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)?;
            if authorization.preparation_id != record.preparation_id
                || Some(authorization.candidate_digest) != record.candidate_digest
                || authorization.outcome_id != record.outcome_id
                || authorization.transition_nullifier != metadata.terminal_nullifier
                || authorization.terminal_receipt_digest != Some(terminal_receipt_digest)
            {
                return Err(SenderErrorV1::Conflict);
            }
            if record
                .terminal_receipt_digest
                .is_some_and(|expected| expected != terminal_receipt_digest)
            {
                return Err(SenderErrorV1::Conflict);
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Reject rollback of the stable-wallet index across recovery sessions or hardware
/// rotations. Both contexts must already be authenticated by the native service.
pub fn validate_index_progress_v1(
    previous_context: &SenderWalletContextV1,
    previous_revision: u128,
    next_context: &SenderWalletContextV1,
    next_revision: u128,
) -> Result<()> {
    previous_context.validate_retained_against_native(next_context)?;
    if next_revision < previous_revision {
        return Err(SenderErrorV1::StateRegression);
    }
    Ok(())
}

/// A later authenticated lookup cannot forget an observed operation or tombstone.
/// Validate the reply against its command/current context before calling this.
pub fn validate_lookup_progress_v1(previous: &SenderRecordV1, reply: &SenderReplyV1) -> Result<()> {
    let SenderReplyBodyV1::Lookup(Some(item)) = &reply.body else {
        return Err(SenderErrorV1::StateRegression);
    };
    previous.validate_shape(&reply.context)?;
    validate_record_progress_v1(previous, &item.record)
}

/// Validate monotonic observations across lost returns/restarts. Missing cannot
/// follow any retained record; terminal tombstones cannot disappear or restart.
/// This does not authorize any of the transitions it permits observing.
pub fn validate_record_progress_v1(previous: &SenderRecordV1, next: &SenderRecordV1) -> Result<()> {
    previous.validate_shape(&previous.context)?;
    next.validate_shape(&next.context)?;
    if previous.context != next.context
        || previous.operation_id != next.operation_id
        || previous.inputs_digest != next.inputs_digest
        || previous.operation_kind != next.operation_kind
        || previous.preparation_id != next.preparation_id
        || previous.outbox_reservation_id != next.outbox_reservation_id
        || previous.outcome_id != next.outcome_id
    {
        return Err(SenderErrorV1::Conflict);
    }
    if previous == next {
        return Ok(());
    }
    if previous.phase == SenderPhaseV1::Released && next.phase == SenderPhaseV1::Released {
        return if previous.terminal_receipt_digest != next.terminal_receipt_digest {
            Err(SenderErrorV1::Conflict)
        } else {
            Err(SenderErrorV1::StateRegression)
        };
    }
    let progresses = match previous.phase {
        SenderPhaseV1::Prepared => matches!(
            next.phase,
            SenderPhaseV1::CandidatePersisted
                | SenderPhaseV1::Committed
                | SenderPhaseV1::Installed
                | SenderPhaseV1::Released
        ),
        SenderPhaseV1::CandidatePersisted => matches!(
            next.phase,
            SenderPhaseV1::Committed | SenderPhaseV1::Installed | SenderPhaseV1::Released
        ),
        SenderPhaseV1::Committed => matches!(
            next.phase,
            SenderPhaseV1::Installed | SenderPhaseV1::Released
        ),
        SenderPhaseV1::Installed => next.phase == SenderPhaseV1::Released,
        SenderPhaseV1::Released => false,
    };
    if !progresses || next.record_revision <= previous.record_revision {
        return Err(SenderErrorV1::StateRegression);
    }
    for (before, after) in [
        (previous.candidate_digest, next.candidate_digest),
        (
            previous.commit_certificate_digest,
            next.commit_certificate_digest,
        ),
        (previous.envelope_digest, next.envelope_digest),
        (
            previous.terminal_receipt_digest,
            next.terminal_receipt_digest,
        ),
    ] {
        if before.is_some() && before != after {
            return Err(SenderErrorV1::Conflict);
        }
    }
    if previous.inputs.is_some() && next.inputs.is_some() && previous.inputs != next.inputs {
        return Err(SenderErrorV1::Conflict);
    }
    Ok(())
}

/// Verify the public projection of an actual retained Core preparation.
/// Credential authentication and the operation-ID index remain the native
/// session's responsibility; this function never serializes private Core fields.
pub fn validate_core_preparation_binding_v1(
    record: &SenderRecordV1,
    native_context: &SenderWalletContextV1,
    prepared: &PreparedOutgoingCandidateV1,
) -> Result<()> {
    record.validate_shape(native_context)?;
    ensure(
        prepared.version == VERSION
            && prepared.preparation_id == record.preparation_id
            && prepared.outbox_reservation.reservation_id == record.outbox_reservation_id
            && prepared.outbox_reservation.operation_kind == record.operation_kind,
    )?;
    let hardware = prepared.hardware_statement();
    let creation = &record.context;
    ensure(
        hardware.lane == creation.lane
            && hardware.predecessor_epoch == creation.hardware_epoch
            && hardware.successor_epoch == creation.hardware_epoch
            && hardware.predecessor_device_policy_binding == creation.device_policy_binding
            && hardware.successor_device_policy_binding == creation.device_policy_binding,
    )?;
    validate_core_recovery_view_v1(record, native_context, prepared.recovery_view())
}

/// Bind an op7/op12 authorization body to the exact retained Core preparation.
///
/// This verifies the canonical body and all Core-visible bindings. It deliberately
/// does not treat the public body or `authorization_id` as authority: qualified
/// hardware must authenticate `authenticator` under the verifier configuration
/// committed by `device_policy_binding.hardware_policy_id`, recompute the statement
/// from its sealed durable record, and consume `authorization_nonce` atomically.
pub fn validate_core_hardware_authorization_binding_v1(
    command: &SenderCommandV1,
    record: &SenderRecordV1,
    prepared: &PreparedOutgoingCandidateV1,
) -> Result<()> {
    validate_existing_operation_v1(command, record)?;
    validate_core_preparation_binding_v1(record, &command.context, prepared)?;
    let authorization_bytes = match &command.body {
        SenderCommandBodyV1::Commit {
            hardware_authorization,
            ..
        }
        | SenderCommandBodyV1::Release {
            hardware_authorization,
            ..
        } => hardware_authorization,
        _ => return Err(SenderErrorV1::Binding),
    };
    let authorization = SenderHardwareAuthorizationV1::decode_canonical_exact(authorization_bytes)?;
    let (outcome_id, transition_nullifier) = match prepared.recovery_view() {
        PreparedOutgoingRecoveryViewV1::Send { output, .. } => {
            (output.credit_id, output.transition_nullifier)
        }
        PreparedOutgoingRecoveryViewV1::Redemption { statement, .. } => {
            (statement.redemption_id, statement.terminal_nullifier)
        }
    };
    let reservation_commitment = prepared
        .outbox_reservation
        .canonical_commitment()
        .map_err(|_| SenderErrorV1::Binding)?;
    ensure(
        authorization.hardware_transition_statement == prepared.hardware_statement()
            && authorization.prepared_one_use_authorization_digest
                == prepared.prepared_one_use_authorization_digest
            && authorization.outbox_reservation_commitment == reservation_commitment
            && authorization.outcome_id == outcome_id
            && authorization.outcome_id == record.outcome_id
            && authorization.transition_nullifier == transition_nullifier,
    )
}

/// Compare only retained public Core material; a recovery view is not proof authority.
pub fn validate_core_recovery_view_v1(
    record: &SenderRecordV1,
    native_context: &SenderWalletContextV1,
    view: PreparedOutgoingRecoveryViewV1<'_>,
) -> Result<()> {
    record.validate_shape(native_context)?;
    let inputs = record.inputs.as_ref().ok_or(SenderErrorV1::Binding)?;
    match (inputs, view) {
        (
            SenderPublicInputsV1::SendSplit { .. },
            PreparedOutgoingRecoveryViewV1::Send {
                request,
                lifecycle,
                output,
                encrypted_credit,
            },
        ) => {
            let r = inputs.send_request()?;
            record.context.validate_lifecycle(lifecycle)?;
            output
                .validate_shape_against(&r)
                .map_err(|_| SenderErrorV1::PublicShape)?;
            // Bind the retained exact ciphertext rather than trusting a host hash.
            ensure(
                lifecycle.ciphertext_digest == kagemusha_ciphertext_digest_v1(encrypted_credit),
            )?;
            ensure(
                &r == request
                    && lifecycle.operation_kind == record.operation_kind
                    && lifecycle.request_id == r.request_id
                    && lifecycle.receiver_lane_commitment == r.hardware_credential.lane_commitment
                    && lifecycle.credit_id == record.outcome_id
                    && output.credit_id == record.outcome_id,
            )
        }
        (
            SenderPublicInputsV1::RedeemSplit {
                amount,
                beneficiary,
            },
            PreparedOutgoingRecoveryViewV1::Redemption { statement, .. },
        ) => {
            record.context.validate_lifecycle(&statement.lifecycle)?;
            statement
                .validate_shape()
                .map_err(|_| SenderErrorV1::PublicShape)?;
            ensure(
                statement.lifecycle.operation_kind == record.operation_kind
                    && statement.amount == *amount
                    && &statement.beneficiary == beneficiary
                    && statement.redemption_id == record.outcome_id,
            )
        }
        _ => Err(SenderErrorV1::Binding),
    }
}

/// Compare installed bytes and immutable selectors with the actual durable Core
/// value. Core remains responsible for verification and atomic installation.
pub fn validate_core_installed_binding_v1(
    record: &SenderRecordV1,
    native_context: &SenderWalletContextV1,
    durable: &DurableOutgoingEnvelopeV1,
) -> Result<()> {
    ensure(record.phase == SenderPhaseV1::Installed)?;
    validate_core_preparation_binding_v1(
        record,
        native_context,
        &durable.committed.candidate.prepared,
    )?;
    ensure(
        record.candidate_digest == Some(durable.committed.candidate.candidate_envelope_digest)
            && record.commit_certificate_digest
                == Some(durable.committed.commit_certificate_digest)
            && record.envelope_digest == Some(durable.envelope_digest),
    )?;
    validate_installed_bytes(record, durable.retry_bytes())
}

/// Bind a public redemption-release selector to Core's sealed in-process authority.
///
/// This function deliberately requires a borrowed non-serializable capability.
/// Decoding a matching terminal receipt or computing its digest is never enough
/// to authorize operation 12. Core consumes the same capability when it
/// atomically releases the indexed redemption outbox entry.
pub fn validate_core_redemption_release_binding_v1(
    command: &SenderCommandV1,
    record: &SenderRecordV1,
    verified: &VerifiedKagemushaRedemptionReleaseV1,
) -> Result<[u8; 32]> {
    validate_existing_operation_v1(command, record)?;
    let SenderCommandBodyV1::Release {
        terminal_receipt: SenderTerminalReceiptV1::RedemptionSettlement(receipt),
        ..
    } = &command.body
    else {
        return Err(SenderErrorV1::Binding);
    };
    ensure(record.operation_kind == KagemushaOperationKindV1::RedeemSplit)?;
    if receipt != verified.terminal_receipt()
        || command.operation_id != verified.operation_id()
        || record.operation_id != verified.operation_id()
        || record.outcome_id != verified.redemption_id()
        || record.envelope_digest != Some(verified.envelope_digest())
        || receipt
            .canonical_digest()
            .map_err(|_| SenderErrorV1::PublicShape)?
            != verified.terminal_receipt_digest()
    {
        return Err(SenderErrorV1::Conflict);
    }
    Ok(verified.terminal_receipt_digest())
}

struct EnvelopeMetadata {
    network_id: iroha_data_model::NetworkId,
    outcome_id: [u8; 32],
    terminal_nullifier: [u8; 32],
    candidate_digest: [u8; 32],
    commit_certificate_digest: [u8; 32],
}

fn envelope_metadata(
    inputs: &SenderPublicInputsV1,
    context: &SenderWalletContextV1,
    bytes: &[u8],
) -> Result<EnvelopeMetadata> {
    inputs.validate_shape(context)?;
    let (
        network_id,
        outcome_id,
        terminal_nullifier,
        certificate,
        candidate_digest,
        commit_certificate_digest,
    ) = match inputs {
        SenderPublicInputsV1::SendSplit { .. } => {
            let request = inputs.send_request()?;
            bound(bytes, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
            let payment = KagemushaPaymentV1::decode_canonical_shape_exact_against(bytes, &request)
                .map_err(|_| SenderErrorV1::PublicShape)?;
            (
                request.network_id,
                payment.output.credit_id,
                payment.output.transition_nullifier,
                payment.commit_certificate,
                payment.proof.candidate_envelope_digest,
                payment.proof.commit_certificate_digest,
            )
        }
        SenderPublicInputsV1::RedeemSplit {
            amount,
            beneficiary,
        } => {
            bound(bytes, KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1)?;
            let voucher = KagemushaRedemptionVoucherV1::decode_canonical_shape_exact(bytes)
                .map_err(|_| SenderErrorV1::PublicShape)?;
            context.validate_lifecycle(&voucher.statement.lifecycle)?;
            ensure(
                voucher.statement.amount == *amount
                    && &voucher.statement.beneficiary == beneficiary,
            )?;
            (
                voucher.statement.lifecycle.network_id,
                voucher.statement.redemption_id,
                voucher.statement.terminal_nullifier,
                voucher.commit_certificate,
                voucher.proof.candidate_envelope_digest,
                voucher.proof.commit_certificate_digest,
            )
        }
    };
    ensure(
        certificate.hardware_profile_id == context.release.hardware_profile_id
            && certificate.policy_epoch == context.release.policy_epoch
            && certificate.candidate_envelope_digest == candidate_digest,
    )?;
    Ok(EnvelopeMetadata {
        network_id,
        outcome_id,
        terminal_nullifier,
        candidate_digest,
        commit_certificate_digest,
    })
}

fn validate_installed_bytes(record: &SenderRecordV1, bytes: &[u8]) -> Result<()> {
    let inputs = record.inputs.as_ref().ok_or(SenderErrorV1::Binding)?;
    let metadata = envelope_metadata(inputs, &record.context, bytes)?;
    ensure(
        metadata.outcome_id == record.outcome_id
            && record.candidate_digest == Some(metadata.candidate_digest)
            && record.commit_certificate_digest == Some(metadata.commit_certificate_digest)
            && record.envelope_digest == Some(terminal_envelope_digest_v1(bytes)?),
    )
}

/// Same domain and full byte transcript as Core's durable terminal envelope.
pub fn terminal_envelope_digest_v1(bytes: &[u8]) -> Result<[u8; 32]> {
    bound(
        bytes,
        KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1.max(KAGEMUSHA_PAYMENT_MAX_BYTES_V1),
    )?;
    Ok(digest_bytes(ENVELOPE_DOMAIN, bytes))
}

fn accepted_terminal_receipt_digest(
    operation_id: [u8; 32],
    inputs: &SenderPublicInputsV1,
    envelope: &[u8],
    metadata: &EnvelopeMetadata,
    terminal_receipt: &SenderTerminalReceiptV1,
) -> Result<[u8; 32]> {
    match (inputs, terminal_receipt) {
        (
            SenderPublicInputsV1::SendSplit { .. },
            SenderTerminalReceiptV1::PaymentAcknowledgement(acknowledgement),
        ) => {
            let request = inputs.send_request()?;
            bound(envelope, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
            bound(acknowledgement, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
            let payment =
                KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &request)
                    .map_err(|_| SenderErrorV1::PublicShape)?;
            KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
                acknowledgement,
                &request,
                &payment,
            )
            .map_err(|_| SenderErrorV1::PublicShape)?;
            Ok(digest_bytes(ACK_DOMAIN, acknowledgement))
        }
        (
            SenderPublicInputsV1::RedeemSplit { .. },
            SenderTerminalReceiptV1::RedemptionSettlement(receipt),
        ) => {
            let envelope_digest = terminal_envelope_digest_v1(envelope)?;
            if receipt.operation_id != operation_id
                || receipt.network_id != metadata.network_id
                || receipt.redemption_id != metadata.outcome_id
                || receipt.terminal_nullifier != metadata.terminal_nullifier
                || receipt.envelope_digest != envelope_digest
            {
                return Err(SenderErrorV1::Conflict);
            }
            receipt
                .canonical_digest()
                .map_err(|_| SenderErrorV1::PublicShape)
        }
        _ => Err(SenderErrorV1::Binding),
    }
}

fn ensure(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(SenderErrorV1::Binding)
    }
}
fn nonzero(value: &[u8; 32]) -> Result<()> {
    ensure(value != &[0; 32])
}
fn bound(bytes: &[u8], maximum: usize) -> Result<()> {
    if bytes.is_empty() || bytes.len() > maximum {
        Err(SenderErrorV1::Size)
    } else {
        Ok(())
    }
}
fn exact<T>(bytes: &[u8], maximum: usize) -> Result<T>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    bound(bytes, maximum)?;
    norito::decode_canonical_with_limits(
        bytes,
        DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(|_| SenderErrorV1::CanonicalEncoding)
}
fn encode<T: NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(value).map_err(|_| SenderErrorV1::CanonicalEncoding)?;
    bound(&bytes, maximum)?;
    Ok(bytes)
}
fn digest_bytes(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update([0]);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[cfg(test)]
fn hardware_authorization_test_key() -> Option<(SigningKey, KagemushaDevicePublicKeyV1)> {
    let signing_key = SigningKey::from_bytes((&[0x61; 32]).into()).ok()?;
    let public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .ok()?;
    Some((signing_key, public_key))
}

#[cfg(test)]
fn canonical_hardware_authorization_for_tests(
    purpose: SenderHardwareAuthorizationPurposeV1,
    operation_id: [u8; 32],
    inputs_digest: [u8; 32],
    preparation_id: [u8; 32],
    candidate_digest: [u8; 32],
    context: &SenderWalletContextV1,
    inputs: &SenderPublicInputsV1,
    outcome_id: [u8; 32],
    transition_nullifier: [u8; 32],
    envelope_digest: Option<[u8; 32]>,
    terminal_receipt_digest: Option<[u8; 32]>,
) -> Option<Vec<u8>> {
    let (signing_key, authorization_public_key) = hardware_authorization_test_key()?;
    if hardware_authorization_key_reference_v1(&authorization_public_key)
        != context.core_authorization_key_reference
    {
        return None;
    }
    let amount = match inputs {
        SenderPublicInputsV1::SendSplit { .. } => inputs.send_request().ok()?.amount,
        SenderPublicInputsV1::RedeemSplit { amount, .. } => *amount,
    };
    let kind = match inputs {
        SenderPublicInputsV1::SendSplit { .. } => KagemushaTransitionKindV1::SendSplit,
        SenderPublicInputsV1::RedeemSplit { .. } => KagemushaTransitionKindV1::RedeemSplit,
    };
    let initial_signature: P256Signature = signing_key.sign(&[0; 32]);
    let initial_signature = initial_signature.normalize_s().unwrap_or(initial_signature);
    let mut authorization = SenderHardwareAuthorizationV1 {
        version: VERSION,
        purpose,
        operation_id,
        inputs_digest,
        preparation_id,
        candidate_digest,
        release_id: context.release.release_id,
        hardware_transition_statement: HardwareTransitionStatementV1 {
            version: VERSION,
            kind,
            amount,
            lane: context.lane.clone(),
            predecessor_commitment: [0x51; 32],
            successor_commitment: [0x52; 32],
            predecessor_sequence: 11,
            successor_sequence: 12,
            predecessor_epoch: context.hardware_epoch,
            successor_epoch: context.hardware_epoch,
            predecessor_device_policy_binding: context.device_policy_binding,
            successor_device_policy_binding: context.device_policy_binding,
            predecessor_state_nonce_commitment: [0x53; 32],
            successor_state_nonce_commitment: [0x54; 32],
            journal_revision_before: 21,
            journal_revision_after: 22,
            state_transition_digest: [0x55; 32],
            normalized_guard_statement_digest: [0x56; 32],
        },
        prepared_one_use_authorization_digest: [0x57; 32],
        outbox_reservation_commitment: [0x58; 32],
        outcome_id,
        transition_nullifier,
        envelope_digest,
        terminal_receipt_digest,
        hardware_one_use_nonce: [0x59; 32],
        authorization_public_key,
        authorization_id: [0; 32],
        authenticator: KagemushaDeviceSignatureV1::from_raw_bytes(
            initial_signature.to_bytes().as_ref(),
        )
        .ok()?,
    };
    authorization.authorization_id = authorization.expected_authorization_id().ok()?;
    let signature: P256Signature = signing_key.sign(&authorization.authorization_id);
    let signature = signature.normalize_s().unwrap_or(signature);
    authorization.authenticator =
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref()).ok()?;
    encode(&authorization, HARDWARE_AUTHORIZATION_MAX_BYTES_V1).ok()
}

#[cfg(test)]
pub(crate) fn canonical_command_body_for_tests(operation: u8) -> Option<Vec<u8>> {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/offline/kagemusha_v1.json"
    )))
    .ok()?;
    let fixture_bytes = |name: &str| {
        fixture
            .get(name)?
            .get("norito_hex")?
            .as_str()
            .and_then(|value| hex::decode(value).ok())
    };
    let request_bytes = fixture_bytes("payment_request")?;
    let payment_bytes = fixture_bytes("payment")?;
    let acknowledgement_bytes = fixture_bytes("acknowledgement")?;
    let request = KagemushaPaymentRequestV1::decode_canonical_exact(&request_bytes).ok()?;
    let payment =
        KagemushaPaymentV1::decode_canonical_shape_exact_against(&payment_bytes, &request).ok()?;
    KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
        &acknowledgement_bytes,
        &request,
        &payment,
    )
    .ok()?;

    let operation_id = [7; 32];
    let (_, authorization_public_key) = hardware_authorization_test_key()?;
    let context = SenderWalletContextV1 {
        lane: KagemushaLaneIdV1 {
            network_id: request.network_id.clone(),
            device_lane_id: [0x31; 32],
            asset: request.asset.clone(),
            scale: request.scale,
        },
        release: KagemushaStateContextV1 {
            protocol_version: VERSION,
            suite_id: request.hardware_credential.suite_id,
            vk_digest: [0x32; 32],
            release_id: request.release_id,
            asset_incarnation: request.asset_incarnation,
            hardware_profile_id: payment.commit_certificate.hardware_profile_id,
            policy_epoch: payment.commit_certificate.policy_epoch,
        },
        credential_id: [0x33; 32],
        hardware_epoch: HardwareEpochV1 {
            generation: 1,
            epoch_id: [0x34; 32],
        },
        device_policy_binding: DevicePolicyBindingV1 {
            device_key_reference: [0x35; 32],
            hardware_policy_id: [0x36; 32],
        },
        core_authorization_key_reference: hardware_authorization_key_reference_v1(
            &authorization_public_key,
        ),
    };
    let inputs = SenderPublicInputsV1::SendSplit {
        request: request_bytes,
    };
    let inputs_digest = SenderPublicInputPreimageV1 {
        version: VERSION,
        operation_id,
        context: context.clone(),
        inputs: inputs.clone(),
    }
    .canonical_digest()
    .ok()?;
    let preparation_id = [0x37; 32];
    let candidate_digest = payment.proof.candidate_envelope_digest;
    let outcome_id = payment.output.credit_id;
    let transition_nullifier = payment.output.transition_nullifier;
    let envelope_digest = terminal_envelope_digest_v1(&payment_bytes).ok()?;
    let receipt = SenderTerminalReceiptV1::PaymentAcknowledgement(acknowledgement_bytes);
    let metadata = envelope_metadata(&inputs, &context, &payment_bytes).ok()?;
    let receipt_digest = accepted_terminal_receipt_digest(
        operation_id,
        &inputs,
        &payment_bytes,
        &metadata,
        &receipt,
    )
    .ok()?;
    let body = match operation {
        5 => SenderCommandBodyV1::Prepare { inputs },
        6 => SenderCommandBodyV1::RecoverPrepared { inputs_digest },
        7 => SenderCommandBodyV1::Commit {
            selector: SenderPreparationSelectorV1 {
                inputs_digest,
                preparation_id,
            },
            candidate_digest,
            hardware_authorization: canonical_hardware_authorization_for_tests(
                SenderHardwareAuthorizationPurposeV1::Commit,
                operation_id,
                inputs_digest,
                preparation_id,
                candidate_digest,
                &context,
                &inputs,
                outcome_id,
                transition_nullifier,
                None,
                None,
            )?,
        },
        8 => SenderCommandBodyV1::RecoverTerminal { inputs_digest },
        9 => SenderCommandBodyV1::Install {
            selector: SenderPreparationSelectorV1 {
                inputs_digest,
                preparation_id,
            },
            candidate_digest,
            inputs,
            envelope: payment_bytes,
        },
        10 => SenderCommandBodyV1::RecoverInstalled {
            selector: SenderRecoverySelectorV1::Lookup { inputs_digest },
        },
        12 => SenderCommandBodyV1::Release {
            inputs_digest,
            envelope_digest,
            inputs,
            envelope: payment_bytes,
            terminal_receipt: receipt,
            hardware_authorization: canonical_hardware_authorization_for_tests(
                SenderHardwareAuthorizationPurposeV1::Release,
                operation_id,
                inputs_digest,
                preparation_id,
                candidate_digest,
                &context,
                &SenderPublicInputsV1::SendSplit {
                    request: fixture_bytes("payment_request")?,
                },
                outcome_id,
                transition_nullifier,
                Some(envelope_digest),
                Some(receipt_digest),
            )?,
        },
        _ => return None,
    };
    SenderCommandV1 {
        version: VERSION,
        operation,
        operation_id,
        context,
        body,
    }
    .encode_canonical()
    .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_bytes(name: &str) -> Vec<u8> {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/kagemusha_v1.json"
        )))
        .expect("shared KAGEMUSHA fixture must decode");
        hex::decode(
            fixture[name]["norito_hex"]
                .as_str()
                .expect("fixture entry must carry canonical bytes"),
        )
        .expect("fixture bytes must be hexadecimal")
    }

    fn installed_sender_record() -> SenderRecordV1 {
        let request_bytes = fixture_bytes("payment_request");
        let request = KagemushaPaymentRequestV1::decode_canonical_exact(&request_bytes)
            .expect("fixture request must decode");
        let context = SenderWalletContextV1 {
            lane: KagemushaLaneIdV1 {
                network_id: request.network_id.clone(),
                device_lane_id: [0x41; 32],
                asset: request.asset.clone(),
                scale: request.scale,
            },
            release: KagemushaStateContextV1 {
                protocol_version: VERSION,
                suite_id: request.hardware_credential.suite_id,
                vk_digest: [0x42; 32],
                release_id: request.release_id,
                asset_incarnation: request.asset_incarnation,
                hardware_profile_id: request.hardware_credential.hardware_profile_id,
                policy_epoch: request.hardware_credential.policy_epoch,
            },
            credential_id: [0x43; 32],
            hardware_epoch: HardwareEpochV1 {
                generation: u128::from(request.hardware_credential.hardware_epoch_generation),
                epoch_id: request.hardware_credential.hardware_epoch_id,
            },
            device_policy_binding: DevicePolicyBindingV1 {
                device_key_reference: [0x44; 32],
                hardware_policy_id: [0x45; 32],
            },
            core_authorization_key_reference: [0x4d; 32],
        };
        let operation_id = [0x46; 32];
        let inputs = SenderPublicInputsV1::SendSplit {
            request: request_bytes,
        };
        let inputs_digest = SenderPublicInputPreimageV1 {
            version: VERSION,
            operation_id,
            context: context.clone(),
            inputs: inputs.clone(),
        }
        .canonical_digest()
        .expect("fixture input preimage must validate");
        let record = SenderRecordV1 {
            operation_id,
            context,
            inputs_digest,
            operation_kind: KagemushaOperationKindV1::SendSplit,
            preparation_id: [0x47; 32],
            outbox_reservation_id: [0x48; 32],
            outcome_id: [0x49; 32],
            phase: SenderPhaseV1::Installed,
            record_revision: 4,
            inputs: Some(inputs),
            candidate_digest: Some([0x4a; 32]),
            commit_certificate_digest: Some([0x4b; 32]),
            envelope_digest: Some([0x4c; 32]),
            terminal_receipt_digest: None,
        };
        record
            .validate_shape(&record.context)
            .expect("installed fixture record must validate");
        record
    }

    #[test]
    fn sender_operation_inventory_uses_frozen_codes() {
        let request = SenderPublicInputsV1::SendSplit { request: vec![1] };
        let selector = SenderPreparationSelectorV1 {
            inputs_digest: [1; 32],
            preparation_id: [2; 32],
        };
        assert_eq!(
            SenderCommandBodyV1::Prepare {
                inputs: request.clone()
            }
            .operation(),
            5
        );
        assert_eq!(
            SenderCommandBodyV1::RecoverPrepared {
                inputs_digest: [1; 32]
            }
            .operation(),
            6
        );
        assert_eq!(
            SenderCommandBodyV1::Commit {
                selector: selector.clone(),
                candidate_digest: [3; 32],
                hardware_authorization: vec![1],
            }
            .operation(),
            7
        );
        assert_eq!(
            SenderCommandBodyV1::RecoverTerminal {
                inputs_digest: [1; 32]
            }
            .operation(),
            8
        );
        assert_eq!(
            SenderCommandBodyV1::Install {
                selector,
                candidate_digest: [3; 32],
                inputs: request.clone(),
                envelope: vec![1]
            }
            .operation(),
            9
        );
        assert_eq!(
            SenderCommandBodyV1::RecoverInstalled {
                selector: SenderRecoverySelectorV1::Lookup {
                    inputs_digest: [1; 32],
                },
            }
            .operation(),
            10
        );
        assert_eq!(
            SenderCommandBodyV1::Release {
                inputs_digest: [1; 32],
                envelope_digest: [4; 32],
                inputs: request,
                envelope: vec![1],
                terminal_receipt: SenderTerminalReceiptV1::PaymentAcknowledgement(vec![1]),
                hardware_authorization: vec![1],
            }
            .operation(),
            12
        );
    }

    #[test]
    fn terminal_digest_rejects_empty_bytes() {
        assert_eq!(terminal_envelope_digest_v1(&[]), Err(SenderErrorV1::Size));
    }

    #[test]
    fn operation_12_exact_duplicate_terminal_receipt_is_idempotent() {
        let mut released = installed_sender_record();
        released.phase = SenderPhaseV1::Released;
        released.record_revision += 1;
        released.inputs = None;
        released.terminal_receipt_digest = Some([0x4d; 32]);
        released
            .validate_shape(&released.context)
            .expect("released fixture record must validate");

        assert_eq!(validate_record_progress_v1(&released, &released), Ok(()));
    }

    #[test]
    fn operation_12_conflicting_terminal_receipt_is_rejected() {
        let mut released = installed_sender_record();
        released.phase = SenderPhaseV1::Released;
        released.record_revision += 1;
        released.inputs = None;
        released.terminal_receipt_digest = Some([0x4d; 32]);
        let mut conflict = released.clone();
        conflict.terminal_receipt_digest = Some([0x4e; 32]);

        assert_eq!(
            validate_record_progress_v1(&released, &conflict),
            Err(SenderErrorV1::Conflict)
        );
    }
}
