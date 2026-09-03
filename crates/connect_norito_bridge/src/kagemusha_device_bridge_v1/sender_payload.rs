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
    KAGEMUSHA_OUTGOING_PUBLIC_INPUTS_DOMAIN_V1, KagemushaLaneIdV1, KagemushaStateContextV1,
    PreparedOutgoingCandidateV1, PreparedOutgoingRecoveryViewV1,
};
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_ASSET_SCALE_MAX_V1,
        KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
        KAGEMUSHA_REDEMPTION_VOUCHER_MAX_BYTES_V1, KagemushaAcknowledgementV1,
        KagemushaLifecycleBindingV1, KagemushaOperationKindV1, KagemushaPaymentRequestV1,
        KagemushaPaymentV1, KagemushaRedemptionVoucherV1, kagemusha_ciphertext_digest_v1,
        kagemusha_liability_pool_id_v1,
    },
};
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};
use sha2::{Digest as _, Sha256};

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

/// Closed failures for shape and replay binding. No variant grants authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SenderErrorV1 {
    /// Empty, oversized or resource-amplifying archive.
    Size,
    /// Wrong schema or noncanonical Norito bytes.
    CanonicalEncoding,
    /// Unsupported operation, bad selector, or mismatched native context.
    Binding,
    /// Public request, envelope or acknowledgement is invalid.
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
    /// Select the exact peer envelope and its matching durable acknowledgement.
    Release {
        /// Digest of the original caller ID, creation context and public inputs.
        inputs_digest: [u8; 32],
        /// Domain-separated digest of the exact terminal envelope.
        envelope_digest: [u8; 32],
        /// Original public inputs fixed before native preparation.
        inputs: SenderPublicInputsV1,
        /// Exact canonical terminal envelope bytes.
        envelope: Vec<u8>,
        /// Exact peer acknowledgement bound to the terminal payment.
        acknowledgement: Vec<u8>,
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
            } => {
                nonzero(&selector.preparation_id)?;
                nonzero(candidate_digest)
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
                acknowledgement,
            } => {
                ensure(self.digest_for(inputs)? == *inputs_digest)?;
                envelope_metadata(inputs, &self.context, envelope)?;
                ensure(terminal_envelope_digest_v1(envelope)? == *envelope_digest)?;
                accepted_ack_digest(inputs, envelope, acknowledgement).map(|_| ())
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
    /// Peer acknowledgement is accepted and immutable replay anchors remain.
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
    /// Exact accepted peer acknowledgement identity retained by Released.
    pub acknowledgement_digest: Option<[u8; 32]>,
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
            self.acknowledgement_digest,
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
            self.acknowledgement_digest.is_some(),
        );
        ensure(match self.phase {
            SenderPhaseV1::Prepared => shape == (false, false, false, false),
            SenderPhaseV1::CandidatePersisted => shape == (true, false, false, false),
            SenderPhaseV1::Committed => shape == (true, true, false, false),
            SenderPhaseV1::Installed => shape == (true, true, true, false),
            SenderPhaseV1::Released => {
                shape == (true, true, true, true)
                    && self.operation_kind == KagemushaOperationKindV1::SendSplit
            }
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
        SenderCommandBodyV1::Commit { .. } => ensure(matches!(
            record.phase,
            SenderPhaseV1::CandidatePersisted
                | SenderPhaseV1::Committed
                | SenderPhaseV1::Installed
                | SenderPhaseV1::Released
        )),
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
            acknowledgement,
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
            let ack = accepted_ack_digest(inputs, envelope, acknowledgement)?;
            if record
                .acknowledgement_digest
                .is_some_and(|expected| expected != ack)
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
        (previous.acknowledgement_digest, next.acknowledgement_digest),
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

struct EnvelopeMetadata {
    outcome_id: [u8; 32],
    candidate_digest: [u8; 32],
    commit_certificate_digest: [u8; 32],
}

fn envelope_metadata(
    inputs: &SenderPublicInputsV1,
    context: &SenderWalletContextV1,
    bytes: &[u8],
) -> Result<EnvelopeMetadata> {
    inputs.validate_shape(context)?;
    let (outcome_id, certificate, candidate_digest, commit_certificate_digest) = match inputs {
        SenderPublicInputsV1::SendSplit { .. } => {
            let request = inputs.send_request()?;
            bound(bytes, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
            let payment = KagemushaPaymentV1::decode_canonical_shape_exact_against(bytes, &request)
                .map_err(|_| SenderErrorV1::PublicShape)?;
            (
                payment.output.credit_id,
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
                voucher.statement.redemption_id,
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
        outcome_id,
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

fn accepted_ack_digest(
    inputs: &SenderPublicInputsV1,
    envelope: &[u8],
    acknowledgement: &[u8],
) -> Result<[u8; 32]> {
    // Redemption uses its chain-settlement path; a peer acknowledgement cannot
    // be repurposed as settlement authority.
    let request = inputs.send_request()?;
    bound(envelope, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
    bound(acknowledgement, KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
    let payment = KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &request)
        .map_err(|_| SenderErrorV1::PublicShape)?;
    KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
        acknowledgement,
        &request,
        &payment,
    )
    .map_err(|_| SenderErrorV1::PublicShape)?;
    Ok(digest_bytes(ACK_DOMAIN, acknowledgement))
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
pub(super) fn canonical_command_body_for_tests(operation: u8) -> Option<Vec<u8>> {
    let _ = operation;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
                candidate_digest: [3; 32]
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
                acknowledgement: vec![1],
            }
            .operation(),
            12
        );
    }

    #[test]
    fn terminal_digest_rejects_empty_bytes() {
        assert_eq!(terminal_envelope_digest_v1(&[]), Err(SenderErrorV1::Size));
    }
}
