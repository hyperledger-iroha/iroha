//! Canonical payload contracts for KAGEMUSHA V1 control operations 1, 11,
//! and 13--22.
//!
//! These codecs fix public command and successful-reply shapes. They do not
//! implement the qualified hardware service. In particular, canonical profile,
//! credential, proof, receipt or state bytes are never treated as authenticated
//! Core state merely because their public shape validates. Stock dispatch uses
//! the sealed unavailable engine below after performing every shape check.
//! TODO: connect these contracts only to a qualified service that owns the
//! authenticated release catalog, hardware keys, journals and recursive proofs.

use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1,
        KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, KAGEMUSHA_HARDWARE_CREDENTIAL_MAX_BYTES_V1,
        KAGEMUSHA_HARDWARE_PROFILE_MAX_BYTES_V1, KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
        KAGEMUSHA_PAYMENT_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
        KAGEMUSHA_REQUEST_MAX_TTL_MS_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaAcknowledgementV1,
        KagemushaAggregateStateCommitmentV1, KagemushaCommitEvidenceV1, KagemushaDevicePublicKeyV1,
        KagemushaEncryptedCreditEnvelopeV1, KagemushaHardwareCredentialV1,
        KagemushaHardwareProfileV1, KagemushaInboxReceiptV1, KagemushaMintAuthorizationV1,
        KagemushaPaymentRequestV1, KagemushaPaymentV1, kagemusha_ciphertext_digest_v1,
    },
};
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};

#[cfg(test)]
use iroha_crypto::{Algorithm, KeyPair};

const VERSION: u16 = KAGEMUSHA_WIRE_VERSION_V1;
const READ_CREDENTIAL: u8 = 1;
const SIGN_ACKNOWLEDGEMENT: u8 = 11;
const READ_TIME_OR_LEASE: u8 = 13;
const PREPARE_MINT: u8 = 14;
const RECOVER_MINT: u8 = 15;
const FOLD_RECEIVE_CREDIT: u8 = 17;
const READ_PENDING_WATERMARK: u8 = 18;
const ROTATE_HARDWARE_EPOCH: u8 = 19;
const BOOTSTRAP_AGGREGATE_STATE: u8 = 20;
const RECOVER_WALLET_SNAPSHOT: u8 = 21;
const CREATE_SIGNED_PAYMENT_REQUEST: u8 = 22;

const READ_COMMAND_MAX: usize = 256;
const ACKNOWLEDGEMENT_COMMAND_MAX: usize = 12 * 1024;
const MINT_COMMAND_MAX: usize = 2 * 1024;
const FOLD_COMMAND_MAX: usize = 256;
const QUALIFICATION_REPLY_MAX: usize = 2 * 1024;
const ACKNOWLEDGEMENT_REPLY_MAX: usize = 2 * 1024;
const EVIDENCE_REPLY_MAX: usize = 512;
const MINT_REPLY_MAX: usize = 12 * 1024;
const FOLD_REPLY_MAX: usize = 2 * 1024;
const WATERMARK_REPLY_MAX: usize = 256;
const ROTATION_REPLY_MAX: usize = 2 * 1024;
const BOOTSTRAP_COMMAND_MAX: usize = 256;
const PAYMENT_REQUEST_COMMAND_MAX: usize = 2 * 1024;
const WALLET_SNAPSHOT_REPLY_MAX: usize = 2 * 1024;
const PAYMENT_REQUEST_REPLY_MAX: usize = 2 * 1024;

/// Qualified state or custody required after public body validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MissingAuthorityV1 {
    /// The authenticated release/profile membership and active credential are absent.
    AuthenticatedQualification,
    /// Receiver key custody and the durable receipt journal are absent.
    QualifiedReceiverJournal,
    /// Qualified trusted-time or monotonic-lease authority is absent.
    TrustedTimeOrLease,
    /// Authenticated release, proof engine and durable mint journal are absent.
    QualifiedMintJournal,
    /// Authenticated pending-credit state and exact-next fold authority are absent.
    QualifiedAggregateState,
    /// The qualified service has no authority to create the first aggregate state.
    QualifiedBootstrap,
    /// The authenticated aggregate, journal, pending-credit and outbox stores are absent.
    AuthenticatedWalletSnapshot,
    /// The active qualified request-signing key and trusted time source are absent.
    QualifiedRequestSigner,
}

/// Closed failures for the public control contracts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ControlErrorV1 {
    /// The operation is outside this module's closed inventory.
    UnsupportedOperation,
    /// A command or reply is empty, oversized or resource-amplifying.
    Size,
    /// Norito bytes are malformed, non-canonical or use another schema.
    CanonicalEncoding,
    /// A version, operation, selector or returned exchange does not match.
    Binding,
    /// A public KAGEMUSHA object fails its structural or signature checks.
    PublicShape,
    /// A qualified engine prerequisite is absent.
    Unavailable(MissingAuthorityV1),
}
type Result<T> = std::result::Result<T, ControlErrorV1>;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.read-active-hardware-credential-command")]
struct ReadCredentialPayloadV1 {
    version: u16,
    operation: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.sign-receive-acknowledgement-command")]
struct SignAcknowledgementPayloadV1 {
    version: u16,
    operation: u8,
    canonical_request: Vec<u8>,
    canonical_payment: Vec<u8>,
    inbox_receipt: KagemushaInboxReceiptV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.read-trusted-time-or-lease-command")]
struct ReadTimeOrLeasePayloadV1 {
    version: u16,
    operation: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.prepare-mint-authorization-command")]
struct PrepareMintPayloadV1 {
    version: u16,
    operation: u8,
    operation_id: [u8; 32],
    amount: u128,
    payer: AccountId,
    recipient: AccountId,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.recover-mint-authorization-command")]
struct RecoverMintPayloadV1 {
    version: u16,
    operation: u8,
    operation_id: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.fold-receive-credit-command")]
struct FoldReceiveCreditPayloadV1 {
    version: u16,
    operation: u8,
    operation_id: [u8; 32],
    kind: PendingCreditKindV1,
    credit_id: [u8; 32],
}

/// The authenticated inbox holding one pending monetary credit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.pending-credit-kind")]
pub(super) enum PendingCreditKindV1 {
    /// A finalized reserve-backed mint awaiting `MintFold`.
    Mint,
    /// A peer payment awaiting `ReceiveFold`.
    Receive,
}

/// One deterministic pending-credit selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.pending-credit-selector")]
pub(super) struct PendingCreditSelectorV1 {
    kind: PendingCreditKindV1,
    credit_id: [u8; 32],
}

/// Epoch-qualified inclusive inbox boundary retained for one selection pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.pending-credit-watermark")]
pub(super) struct PendingCreditWatermarkV1 {
    hardware_epoch_generation: u128,
    hardware_epoch_id: [u8; 32],
    inbox_revision: u128,
}

/// The amount-aware objective for one pending-credit selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.pending-credit-target")]
pub(super) enum PendingCreditTargetV1 {
    /// Select the next credit regardless of the current aggregate balance.
    DrainAll,
    /// Select only while the authenticated aggregate balance is below this amount.
    RequiredBalance(u128),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.read-pending-credit-watermark-command")]
struct ReadPendingWatermarkPayloadV1 {
    version: u16,
    operation: u8,
    watermark: Option<PendingCreditWatermarkV1>,
    target: PendingCreditTargetV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.rotate-hardware-epoch-command")]
struct RotateHardwareEpochPayloadV1 {
    version: u16,
    operation: u8,
    operation_id: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.bootstrap-aggregate-state-command")]
struct BootstrapAggregateStatePayloadV1 {
    version: u16,
    operation: u8,
    operation_id: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.recover-wallet-snapshot-command")]
struct RecoverWalletSnapshotPayloadV1 {
    version: u16,
    operation: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.create-signed-payment-request-command")]
struct CreateSignedPaymentRequestPayloadV1 {
    version: u16,
    operation: u8,
    request_id: [u8; 32],
    recipient: AccountId,
    amount: u128,
    validity_window_ms: u64,
}

/// Shape-checked public input to one of this module's operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ControlCommandV1 {
    /// Read the active qualification objects; release membership stays native-authenticated.
    ReadCredential,
    /// Sign only the receipt for this exact, already durable public exchange.
    SignAcknowledgement {
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        inbox_receipt: KagemushaInboxReceiptV1,
    },
    /// Read opaque evidence from the qualified deadline source.
    ReadTimeOrLease,
    /// Prepare one proof-bearing authorization under native-owned wallet context.
    PrepareMint {
        operation_id: [u8; 32],
        amount: u128,
        payer: AccountId,
        recipient: AccountId,
    },
    /// Recover the authorization durably retained for this operation ID.
    RecoverMint { operation_id: [u8; 32] },
    /// Fold one staged credit into the aggregate balance.
    FoldReceiveCredit {
        operation_id: [u8; 32],
        selector: PendingCreditSelectorV1,
    },
    /// Read the stable watermark and deterministic next credit for this balance target.
    ReadPendingWatermark {
        watermark: Option<PendingCreditWatermarkV1>,
        target: PendingCreditTargetV1,
    },
    /// Rotate state into the next qualified hardware epoch.
    RotateHardwareEpoch { operation_id: [u8; 32] },
    /// Create the unique sequence-zero aggregate state under native custody.
    BootstrapAggregateState { operation_id: [u8; 32] },
    /// Read one atomic snapshot of all host-visible wallet recovery state.
    RecoverWalletSnapshot,
    /// Construct and sign a request using native trusted time and the active credential.
    CreateSignedPaymentRequest {
        request_id: [u8; 32],
        recipient: AccountId,
        amount: u128,
        validity_window_ms: u64,
    },
}

impl ControlCommandV1 {
    fn operation(&self) -> u8 {
        match self {
            Self::ReadCredential => READ_CREDENTIAL,
            Self::SignAcknowledgement { .. } => SIGN_ACKNOWLEDGEMENT,
            Self::ReadTimeOrLease => READ_TIME_OR_LEASE,
            Self::PrepareMint { .. } => PREPARE_MINT,
            Self::RecoverMint { .. } => RECOVER_MINT,
            Self::FoldReceiveCredit { .. } => FOLD_RECEIVE_CREDIT,
            Self::ReadPendingWatermark { .. } => READ_PENDING_WATERMARK,
            Self::RotateHardwareEpoch { .. } => ROTATE_HARDWARE_EPOCH,
            Self::BootstrapAggregateState { .. } => BOOTSTRAP_AGGREGATE_STATE,
            Self::RecoverWalletSnapshot => RECOVER_WALLET_SNAPSHOT,
            Self::CreateSignedPaymentRequest { .. } => CREATE_SIGNED_PAYMENT_REQUEST,
        }
    }
}

fn bound(bytes: &[u8], maximum: usize) -> Result<()> {
    if bytes.is_empty() || bytes.len() > maximum {
        Err(ControlErrorV1::Size)
    } else {
        Ok(())
    }
}

fn nonzero(value: &[u8; 32]) -> Result<()> {
    if value == &[0; 32] {
        Err(ControlErrorV1::Binding)
    } else {
        Ok(())
    }
}

fn validate_pending_watermark(watermark: PendingCreditWatermarkV1) -> Result<()> {
    if watermark.hardware_epoch_generation == 0 || watermark.hardware_epoch_id == [0; 32] {
        Err(ControlErrorV1::Binding)
    } else {
        Ok(())
    }
}

fn header(version: u16, operation: u8, expected: u8) -> Result<()> {
    if version != VERSION || operation != expected {
        Err(ControlErrorV1::Binding)
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
    .map_err(|_| ControlErrorV1::CanonicalEncoding)
}

fn encode<T: NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(value).map_err(|_| ControlErrorV1::CanonicalEncoding)?;
    bound(&bytes, maximum)?;
    Ok(bytes)
}

fn decode_exchange(
    request: &[u8],
    payment: &[u8],
) -> Result<(KagemushaPaymentRequestV1, KagemushaPaymentV1)> {
    bound(request, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
    bound(payment, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
    let request = KagemushaPaymentRequestV1::decode_canonical_exact(request)
        .map_err(|_| ControlErrorV1::PublicShape)?;
    let payment = KagemushaPaymentV1::decode_canonical_shape_exact_against(payment, &request)
        .map_err(|_| ControlErrorV1::PublicShape)?;
    Ok((request, payment))
}

/// Decode exactly one bounded operation body and bind it to the outer request ID.
pub(super) fn decode_control_command_v1(
    operation: u8,
    request_id: [u8; 32],
    bytes: &[u8],
) -> Result<ControlCommandV1> {
    nonzero(&request_id)?;
    match operation {
        READ_CREDENTIAL => {
            let value: ReadCredentialPayloadV1 = exact(bytes, READ_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            Ok(ControlCommandV1::ReadCredential)
        }
        SIGN_ACKNOWLEDGEMENT => {
            let value: SignAcknowledgementPayloadV1 = exact(bytes, ACKNOWLEDGEMENT_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            let (request, payment) =
                decode_exchange(&value.canonical_request, &value.canonical_payment)?;
            if value.inbox_receipt.version != VERSION
                || value.inbox_receipt.credit_id != payment.output.credit_id
                || value.inbox_receipt.credit_id != request_id
                || value.inbox_receipt.receipt_commitment == [0; 32]
            {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::SignAcknowledgement {
                request,
                payment,
                inbox_receipt: value.inbox_receipt,
            })
        }
        READ_TIME_OR_LEASE => {
            let value: ReadTimeOrLeasePayloadV1 = exact(bytes, READ_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            Ok(ControlCommandV1::ReadTimeOrLease)
        }
        PREPARE_MINT => {
            let value: PrepareMintPayloadV1 = exact(bytes, MINT_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.operation_id)?;
            if value.operation_id != request_id || value.amount == 0 {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::PrepareMint {
                operation_id: value.operation_id,
                amount: value.amount,
                payer: value.payer,
                recipient: value.recipient,
            })
        }
        RECOVER_MINT => {
            let value: RecoverMintPayloadV1 = exact(bytes, READ_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.operation_id)?;
            if value.operation_id != request_id {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::RecoverMint {
                operation_id: value.operation_id,
            })
        }
        FOLD_RECEIVE_CREDIT => {
            let value: FoldReceiveCreditPayloadV1 = exact(bytes, FOLD_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.operation_id)?;
            nonzero(&value.credit_id)?;
            if value.operation_id != request_id {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::FoldReceiveCredit {
                operation_id: value.operation_id,
                selector: PendingCreditSelectorV1 {
                    kind: value.kind,
                    credit_id: value.credit_id,
                },
            })
        }
        READ_PENDING_WATERMARK => {
            let value: ReadPendingWatermarkPayloadV1 = exact(bytes, READ_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            if let Some(watermark) = value.watermark {
                validate_pending_watermark(watermark)?;
            }
            if matches!(value.target, PendingCreditTargetV1::RequiredBalance(0))
                || matches!(value.target, PendingCreditTargetV1::RequiredBalance(_))
                    && value.watermark.is_some()
            {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::ReadPendingWatermark {
                watermark: value.watermark,
                target: value.target,
            })
        }
        ROTATE_HARDWARE_EPOCH => {
            let value: RotateHardwareEpochPayloadV1 = exact(bytes, READ_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.operation_id)?;
            if value.operation_id != request_id {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::RotateHardwareEpoch {
                operation_id: value.operation_id,
            })
        }
        BOOTSTRAP_AGGREGATE_STATE => {
            let value: BootstrapAggregateStatePayloadV1 = exact(bytes, BOOTSTRAP_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.operation_id)?;
            if value.operation_id != request_id {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::BootstrapAggregateState {
                operation_id: value.operation_id,
            })
        }
        RECOVER_WALLET_SNAPSHOT => {
            let value: RecoverWalletSnapshotPayloadV1 = exact(bytes, READ_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            Ok(ControlCommandV1::RecoverWalletSnapshot)
        }
        CREATE_SIGNED_PAYMENT_REQUEST => {
            let value: CreateSignedPaymentRequestPayloadV1 =
                exact(bytes, PAYMENT_REQUEST_COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.request_id)?;
            if value.request_id != request_id
                || value.validity_window_ms == 0
                || value.validity_window_ms > KAGEMUSHA_REQUEST_MAX_TTL_MS_V1
                || value.amount == 0
            {
                return Err(ControlErrorV1::Binding);
            }
            Ok(ControlCommandV1::CreateSignedPaymentRequest {
                request_id: value.request_id,
                recipient: value.recipient,
                amount: value.amount,
                validity_window_ms: value.validity_window_ms,
            })
        }
        _ => Err(ControlErrorV1::UnsupportedOperation),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.active-hardware-credential-reply")]
struct QualificationReplyV1 {
    version: u16,
    operation: u8,
    release_id: [u8; 32],
    hardware_policy_digest: [u8; 32],
    core_authorization_key_reference: [u8; 32],
    profile: KagemushaHardwareProfileV1,
    credential: KagemushaHardwareCredentialV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.receive-acknowledgement-reply")]
struct AcknowledgementReplyV1 {
    version: u16,
    operation: u8,
    canonical_acknowledgement: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.trusted-time-or-lease-reply")]
struct TimeOrLeaseReplyV1 {
    version: u16,
    operation: u8,
    evidence: KagemushaCommitEvidenceV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.mint-construction-bundle-reply")]
struct MintConstructionBundleReplyV1 {
    version: u16,
    operation: u8,
    canonical_authorization: Vec<u8>,
    encrypted_credit: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.fold-receive-credit-reply")]
struct FoldReceiveCreditReplyV1 {
    version: u16,
    operation: u8,
    kind: PendingCreditKindV1,
    credit_id: [u8; 32],
    canonical_aggregate_state: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.pending-credit-watermark-reply")]
struct PendingWatermarkReplyV1 {
    version: u16,
    operation: u8,
    watermark: PendingCreditWatermarkV1,
    next_pending: Option<PendingCreditSelectorV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.rotate-hardware-epoch-reply")]
struct RotationReplyV1 {
    version: u16,
    operation: u8,
    canonical_aggregate_state: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.bootstrap-aggregate-state-reply")]
struct BootstrapAggregateStateReplyV1 {
    version: u16,
    operation: u8,
    canonical_aggregate_state: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.wallet-recovery-snapshot-reply")]
struct WalletRecoverySnapshotReplyV1 {
    version: u16,
    operation: u8,
    canonical_aggregate_state: Option<Vec<u8>>,
    journal_revision: u128,
    pending_credit_count: u128,
    retry_outbox_count: u128,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.signed-payment-request-reply")]
struct SignedPaymentRequestReplyV1 {
    version: u16,
    operation: u8,
    canonical_request: Vec<u8>,
}

/// Validate a canonical success body against the exact decoded command.
///
/// An outer `Missing` status has an empty payload and is how recovery operations
/// distinguish an absent record; it is intentionally not represented here.
pub(super) fn validate_control_reply_v1(command: &ControlCommandV1, bytes: &[u8]) -> Result<()> {
    match command {
        ControlCommandV1::ReadCredential => {
            let reply: QualificationReplyV1 = exact(bytes, QUALIFICATION_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            nonzero(&reply.release_id)?;
            nonzero(&reply.hardware_policy_digest)?;
            nonzero(&reply.core_authorization_key_reference)?;
            reply
                .profile
                .validate()
                .map_err(|_| ControlErrorV1::PublicShape)?;
            reply
                .credential
                .validate_against_profile(&reply.profile)
                .map_err(|_| ControlErrorV1::PublicShape)
        }
        ControlCommandV1::SignAcknowledgement {
            request,
            payment,
            inbox_receipt,
        } => {
            let reply: AcknowledgementReplyV1 = exact(bytes, ACKNOWLEDGEMENT_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            bound(
                &reply.canonical_acknowledgement,
                KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1,
            )?;
            let acknowledgement = KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
                &reply.canonical_acknowledgement,
                request,
                payment,
            )
            .map_err(|_| ControlErrorV1::PublicShape)?;
            if &acknowledgement.inbox_receipt != inbox_receipt {
                return Err(ControlErrorV1::Binding);
            }
            Ok(())
        }
        ControlCommandV1::ReadTimeOrLease => {
            let reply: TimeOrLeaseReplyV1 = exact(bytes, EVIDENCE_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            reply
                .evidence
                .validate()
                .map_err(|_| ControlErrorV1::PublicShape)
        }
        ControlCommandV1::PrepareMint {
            operation_id,
            amount,
            payer,
            recipient,
        } => {
            let reply: MintConstructionBundleReplyV1 = exact(bytes, MINT_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            bound(
                &reply.canonical_authorization,
                KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            )?;
            let authorization = KagemushaMintAuthorizationV1::decode_canonical_shape_exact(
                &reply.canonical_authorization,
            )
            .map_err(|_| ControlErrorV1::PublicShape)?;
            validate_mint_encrypted_credit_v1(&authorization, &reply.encrypted_credit)?;
            let context = &authorization.statement.context;
            if &context.operation_id != operation_id
                || context.amount != *amount
                || &context.payer != payer
                || &context.recipient != recipient
            {
                return Err(ControlErrorV1::Binding);
            }
            Ok(())
        }
        ControlCommandV1::RecoverMint { operation_id } => {
            let reply: MintConstructionBundleReplyV1 = exact(bytes, MINT_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            bound(
                &reply.canonical_authorization,
                KAGEMUSHA_MINT_AUTHORIZATION_MAX_BYTES_V1,
            )?;
            let authorization = KagemushaMintAuthorizationV1::decode_canonical_shape_exact(
                &reply.canonical_authorization,
            )
            .map_err(|_| ControlErrorV1::PublicShape)?;
            validate_mint_encrypted_credit_v1(&authorization, &reply.encrypted_credit)?;
            if &authorization.statement.context.operation_id != operation_id {
                return Err(ControlErrorV1::Binding);
            }
            Ok(())
        }
        ControlCommandV1::FoldReceiveCredit { selector, .. } => {
            let reply: FoldReceiveCreditReplyV1 = exact(bytes, FOLD_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            if reply.kind != selector.kind || reply.credit_id != selector.credit_id {
                return Err(ControlErrorV1::Binding);
            }
            bound(
                &reply.canonical_aggregate_state,
                KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1,
            )?;
            KagemushaAggregateStateCommitmentV1::decode_canonical_exact(
                &reply.canonical_aggregate_state,
            )
            .map_err(|_| ControlErrorV1::PublicShape)?;
            Ok(())
        }
        ControlCommandV1::ReadPendingWatermark { watermark, .. } => {
            let reply: PendingWatermarkReplyV1 = exact(bytes, WATERMARK_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())
                .and_then(|()| validate_pending_watermark(reply.watermark))?;
            if watermark.is_some_and(|expected| expected != reply.watermark) {
                return Err(ControlErrorV1::Binding);
            }
            if let Some(next) = reply.next_pending {
                nonzero(&next.credit_id)?;
            }
            Ok(())
        }
        ControlCommandV1::RotateHardwareEpoch { .. } => {
            let reply: RotationReplyV1 = exact(bytes, ROTATION_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            bound(
                &reply.canonical_aggregate_state,
                KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1,
            )?;
            let state = KagemushaAggregateStateCommitmentV1::decode_canonical_exact(
                &reply.canonical_aggregate_state,
            )
            .map_err(|_| ControlErrorV1::PublicShape)?;
            if state.sequence != 0 {
                return Err(ControlErrorV1::Binding);
            }
            Ok(())
        }
        ControlCommandV1::BootstrapAggregateState { .. } => {
            let reply: BootstrapAggregateStateReplyV1 = exact(bytes, ROTATION_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            bound(
                &reply.canonical_aggregate_state,
                KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1,
            )?;
            let state = KagemushaAggregateStateCommitmentV1::decode_canonical_exact(
                &reply.canonical_aggregate_state,
            )
            .map_err(|_| ControlErrorV1::PublicShape)?;
            if state.sequence != 0 {
                return Err(ControlErrorV1::Binding);
            }
            Ok(())
        }
        ControlCommandV1::RecoverWalletSnapshot => {
            let reply: WalletRecoverySnapshotReplyV1 = exact(bytes, WALLET_SNAPSHOT_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            if let Some(state_bytes) = &reply.canonical_aggregate_state {
                bound(state_bytes, KAGEMUSHA_AGGREGATE_STATE_MAX_BYTES_V1)?;
                KagemushaAggregateStateCommitmentV1::decode_canonical_exact(state_bytes)
                    .map_err(|_| ControlErrorV1::PublicShape)?;
            }
            Ok(())
        }
        ControlCommandV1::CreateSignedPaymentRequest {
            request_id,
            recipient,
            amount,
            validity_window_ms,
        } => {
            let reply: SignedPaymentRequestReplyV1 = exact(bytes, PAYMENT_REQUEST_REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            bound(
                &reply.canonical_request,
                KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1,
            )?;
            let request =
                KagemushaPaymentRequestV1::decode_canonical_exact(&reply.canonical_request)
                    .map_err(|_| ControlErrorV1::PublicShape)?;
            let ttl = request
                .expires_at_ms
                .checked_sub(request.issued_at_ms)
                .ok_or(ControlErrorV1::Binding)?;
            if &request.request_id != request_id
                || &request.recipient != recipient
                || request.amount != *amount
                || ttl != *validity_window_ms
            {
                return Err(ControlErrorV1::Binding);
            }
            Ok(())
        }
    }
}

fn validate_mint_encrypted_credit_v1(
    authorization: &KagemushaMintAuthorizationV1,
    encrypted_credit: &[u8],
) -> Result<()> {
    bound(encrypted_credit, KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1)?;
    KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
        encrypted_credit,
        authorization.statement.context.recipient_one_time_key,
    )
    .map_err(|_| ControlErrorV1::PublicShape)?;
    if authorization.statement.ciphertext_digest != kagemusha_ciphertext_digest_v1(encrypted_credit)
    {
        return Err(ControlErrorV1::Binding);
    }
    Ok(())
}

/// Validate operation 1's self-contained qualification chain and bind it to
/// the two digests previously accepted from the capability frame.
///
/// The returned release identifier still requires membership in Core's
/// authenticated release catalog before this key can authorize monetary use.
pub(super) fn qualification_response_key_v1(
    bytes: &[u8],
    hardware_policy_id: [u8; 32],
    qualification_report_digest: [u8; 32],
) -> Result<([u8; 32], KagemushaDevicePublicKeyV1)> {
    validate_control_reply_v1(&ControlCommandV1::ReadCredential, bytes)?;
    let reply: QualificationReplyV1 = exact(bytes, QUALIFICATION_REPLY_MAX)?;
    if reply.hardware_policy_digest != hardware_policy_id
        || reply.profile.qualification_report_digest != qualification_report_digest
    {
        return Err(ControlErrorV1::Binding);
    }
    Ok((reply.release_id, reply.credential.device_public_key))
}

mod engine_seal {
    pub(super) trait Sealed {}
}

trait ControlEngineV1: engine_seal::Sealed {
    fn execute(&mut self, request_id: [u8; 32], command: &ControlCommandV1) -> Result<Vec<u8>>;
}

struct UnavailableControlEngineV1;
impl engine_seal::Sealed for UnavailableControlEngineV1 {}
impl ControlEngineV1 for UnavailableControlEngineV1 {
    fn execute(&mut self, _: [u8; 32], command: &ControlCommandV1) -> Result<Vec<u8>> {
        let missing = match command {
            ControlCommandV1::ReadCredential => MissingAuthorityV1::AuthenticatedQualification,
            ControlCommandV1::SignAcknowledgement { .. } => {
                MissingAuthorityV1::QualifiedReceiverJournal
            }
            ControlCommandV1::ReadTimeOrLease => MissingAuthorityV1::TrustedTimeOrLease,
            ControlCommandV1::PrepareMint { .. } | ControlCommandV1::RecoverMint { .. } => {
                MissingAuthorityV1::QualifiedMintJournal
            }
            ControlCommandV1::FoldReceiveCredit { .. }
            | ControlCommandV1::ReadPendingWatermark { .. }
            | ControlCommandV1::RotateHardwareEpoch { .. } => {
                MissingAuthorityV1::QualifiedAggregateState
            }
            ControlCommandV1::BootstrapAggregateState { .. } => {
                MissingAuthorityV1::QualifiedBootstrap
            }
            ControlCommandV1::RecoverWalletSnapshot => {
                MissingAuthorityV1::AuthenticatedWalletSnapshot
            }
            ControlCommandV1::CreateSignedPaymentRequest { .. } => {
                MissingAuthorityV1::QualifiedRequestSigner
            }
        };
        Err(ControlErrorV1::Unavailable(missing))
    }
}

fn dispatch<E: ControlEngineV1>(
    engine: &mut E,
    request_id: [u8; 32],
    operation: u8,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    let command = decode_control_command_v1(operation, request_id, bytes)?;
    let response = engine.execute(request_id, &command)?;
    validate_control_reply_v1(&command, &response)?;
    Ok(response)
}

/// Strict stock entry point: decode the exact body, then fail unavailable.
pub(super) fn dispatch_unavailable_control_v1(
    request_id: [u8; 32],
    operation: u8,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    dispatch(
        &mut UnavailableControlEngineV1,
        request_id,
        operation,
        bytes,
    )
}

#[cfg(test)]
fn fixture_bytes(name: &str) -> Option<Vec<u8>> {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/offline/kagemusha_v1.json"
    )))
    .ok()?;
    fixture
        .get(name)?
        .get("norito_hex")?
        .as_str()
        .and_then(|value| hex::decode(value).ok())
}

#[cfg(test)]
fn fixture_account_id() -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
        .expect("deterministic account fixture must be valid");
    AccountId::new(key_pair.public_key().clone())
}

/// Construct canonical bodies for the stock outer-frame tests.
#[cfg(test)]
pub(crate) fn canonical_command_body_for_tests(
    operation: u8,
    request_id: [u8; 32],
) -> Option<Vec<u8>> {
    match operation {
        READ_CREDENTIAL => encode(
            &ReadCredentialPayloadV1 {
                version: VERSION,
                operation,
            },
            READ_COMMAND_MAX,
        )
        .ok(),
        SIGN_ACKNOWLEDGEMENT => {
            let request_bytes = fixture_bytes("payment_request")?;
            let payment = fixture_bytes("payment")?;
            let acknowledgement = fixture_bytes("acknowledgement")?;
            let (typed_request, typed_payment) = decode_exchange(&request_bytes, &payment).ok()?;
            let acknowledgement = KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
                &acknowledgement,
                &typed_request,
                &typed_payment,
            )
            .ok()?;
            encode(
                &SignAcknowledgementPayloadV1 {
                    version: VERSION,
                    operation,
                    canonical_request: request_bytes,
                    canonical_payment: payment,
                    inbox_receipt: acknowledgement.inbox_receipt,
                },
                ACKNOWLEDGEMENT_COMMAND_MAX,
            )
            .ok()
        }
        READ_TIME_OR_LEASE => encode(
            &ReadTimeOrLeasePayloadV1 {
                version: VERSION,
                operation,
            },
            READ_COMMAND_MAX,
        )
        .ok(),
        PREPARE_MINT => encode(
            &PrepareMintPayloadV1 {
                version: VERSION,
                operation,
                operation_id: request_id,
                amount: 1,
                payer: fixture_account_id(),
                recipient: fixture_account_id(),
            },
            MINT_COMMAND_MAX,
        )
        .ok(),
        RECOVER_MINT => encode(
            &RecoverMintPayloadV1 {
                version: VERSION,
                operation,
                operation_id: request_id,
            },
            READ_COMMAND_MAX,
        )
        .ok(),
        FOLD_RECEIVE_CREDIT => encode(
            &FoldReceiveCreditPayloadV1 {
                version: VERSION,
                operation,
                operation_id: request_id,
                kind: PendingCreditKindV1::Receive,
                credit_id: [8; 32],
            },
            FOLD_COMMAND_MAX,
        )
        .ok(),
        READ_PENDING_WATERMARK => encode(
            &ReadPendingWatermarkPayloadV1 {
                version: VERSION,
                operation,
                watermark: None,
                target: PendingCreditTargetV1::DrainAll,
            },
            READ_COMMAND_MAX,
        )
        .ok(),
        ROTATE_HARDWARE_EPOCH => encode(
            &RotateHardwareEpochPayloadV1 {
                version: VERSION,
                operation,
                operation_id: request_id,
            },
            READ_COMMAND_MAX,
        )
        .ok(),
        BOOTSTRAP_AGGREGATE_STATE => encode(
            &BootstrapAggregateStatePayloadV1 {
                version: VERSION,
                operation,
                operation_id: request_id,
            },
            BOOTSTRAP_COMMAND_MAX,
        )
        .ok(),
        RECOVER_WALLET_SNAPSHOT => encode(
            &RecoverWalletSnapshotPayloadV1 {
                version: VERSION,
                operation,
            },
            READ_COMMAND_MAX,
        )
        .ok(),
        CREATE_SIGNED_PAYMENT_REQUEST => encode(
            &CreateSignedPaymentRequestPayloadV1 {
                version: VERSION,
                operation,
                request_id,
                recipient: fixture_account_id(),
                amount: 1,
                validity_window_ms: 1,
            },
            PAYMENT_REQUEST_COMMAND_MAX,
        )
        .ok(),
        _ => None,
    }
}

/// Return the fixture's stable outer ID where it is bound to public contents.
#[cfg(test)]
pub(crate) fn canonical_request_id_for_tests(operation: u8) -> Option<[u8; 32]> {
    match operation {
        SIGN_ACKNOWLEDGEMENT => decode_exchange(
            &fixture_bytes("payment_request")?,
            &fixture_bytes("payment")?,
        )
        .ok()
        .map(|(_, payment)| payment.output.credit_id),
        _ => Some([7; 32]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_credit_fold_binds_credit_and_operation_ids() {
        let payload = FoldReceiveCreditPayloadV1 {
            version: VERSION,
            operation: FOLD_RECEIVE_CREDIT,
            operation_id: [7; 32],
            kind: PendingCreditKindV1::Receive,
            credit_id: [8; 32],
        };
        let bytes = encode(&payload, FOLD_COMMAND_MAX).unwrap();
        assert!(decode_control_command_v1(FOLD_RECEIVE_CREDIT, [7; 32], &bytes).is_ok());

        let mut invalid = payload;
        invalid.credit_id = [0; 32];
        let bytes = encode(&invalid, FOLD_COMMAND_MAX).unwrap();
        assert_eq!(
            decode_control_command_v1(FOLD_RECEIVE_CREDIT, [7; 32], &bytes),
            Err(ControlErrorV1::Binding),
        );
    }

    #[test]
    fn pending_selector_binds_kind_target_and_stable_watermark() {
        let watermark = PendingCreditWatermarkV1 {
            hardware_epoch_generation: 9,
            hardware_epoch_id: [4; 32],
            inbox_revision: 31,
        };
        let command = ControlCommandV1::ReadPendingWatermark {
            watermark: Some(watermark),
            target: PendingCreditTargetV1::RequiredBalance(500),
        };
        let reply = PendingWatermarkReplyV1 {
            version: VERSION,
            operation: READ_PENDING_WATERMARK,
            watermark,
            next_pending: Some(PendingCreditSelectorV1 {
                kind: PendingCreditKindV1::Mint,
                credit_id: [6; 32],
            }),
        };
        let bytes = encode(&reply, WATERMARK_REPLY_MAX).unwrap();
        assert!(validate_control_reply_v1(&command, &bytes).is_ok());

        let mut wrong = reply;
        wrong.watermark.inbox_revision += 1;
        let bytes = encode(&wrong, WATERMARK_REPLY_MAX).unwrap();
        assert_eq!(
            validate_control_reply_v1(&command, &bytes),
            Err(ControlErrorV1::Binding),
        );
    }

    #[test]
    fn control_inventory_rejects_non_control_codes() {
        let bytes = encode(
            &ReadCredentialPayloadV1 {
                version: VERSION,
                operation: READ_CREDENTIAL,
            },
            READ_COMMAND_MAX,
        )
        .unwrap();
        for operation in [0, 2, 3, 4, 12, 23, u8::MAX] {
            assert_eq!(
                decode_control_command_v1(operation, [7; 32], &bytes),
                Err(ControlErrorV1::UnsupportedOperation),
            );
        }
    }
}
