//! Transaction structures and related implementations.
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    iter::IntoIterator,
    num::{NonZeroU32, NonZeroU64},
    str::FromStr,
    string::String,
    sync::LazyLock,
    time::Duration,
    vec::Vec,
};

#[cfg(feature = "fault_injection")]
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use derive_more::{Deref, Display, From, TryInto};
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature, SignatureOf};
use iroha_data_model_derive::model;
use iroha_primitives::{const_vec::ConstVec, json::Json, time::TimeSource};
use iroha_schema::IntoSchema;
use iroha_version::Version;
use norito::{
    codec::{Decode, Encode},
    core::DecodeFromSlice,
};
use thiserror::Error;

pub use self::model::*;
use super::{
    error,
    executable::{Executable, ExecutableBatchItem, IvmBytecode},
    private_kaigi::PrivateKaigiTransaction,
};
use crate::{
    ChainId,
    account::{AccountController, AccountId, MultisigPolicy},
    asset::AssetDefinitionId,
    events::data::prelude::AssetBatchTransferOutcome,
    isi::{
        CustomInstruction, ExecuteTrigger, InstructionBox, OpaqueInstruction,
        privacy::SubmitPrivacyProofV1,
    },
    metadata::Metadata,
    name::Name,
    nexus::FeeSponsorProgramId,
    privacy::{
        PrivacyNullifierV1, PrivacyStatementDigestV1, PrivacyStatementV1,
        PrivacyTransactionIntentDigestV1, PrivacyVegaDeviceAuthenticationDigestV1,
    },
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};
use iroha_primitives::numeric::Quantity;

/// Default signature-bound lifetime assigned by [`TransactionBuilder`].
///
/// Networks govern the admission ceiling through
/// [`crate::parameter::TransactionParameters::max_time_to_live_ms`]. This
/// default matches the default client transaction lifetime.
pub const DEFAULT_TRANSACTION_TIME_TO_LIVE: Duration = Duration::from_secs(100);

fn verify_typed_signature_for_signer<T: Encode>(
    signature: &SignatureOf<T>,
    signer: &PublicKey,
    payload: &T,
) -> Result<(), iroha_crypto::Error> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())?;
        }
        _ => {}
    }
    signature.verify(signer, payload)
}

#[model]
mod model {
    use iroha_primitives::const_vec::ConstVec;

    use super::*;
    use crate::account::AccountId;

    /// Fee system whose charge is bounded by a signed transaction limit.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(tag = "kind", content = "value", rename_all = "snake_case")]
    pub enum FeeChargeKind {
        /// Nexus admission and execution fee.
        Nexus,
        /// Pipeline gas charged for contract or IVM execution.
        PipelineGas,
    }

    /// Signature-bound upper bound for one fee component and asset.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct FeeChargeLimit {
        /// Fee component constrained by this limit.
        pub kind: FeeChargeKind,
        /// Exact canonical asset definition in which the component may be charged.
        pub asset_definition_id: AssetDefinitionId,
        /// Maximum amount the signer authorizes for this component.
        pub max_amount: Quantity,
    }

    /// Signature-bound limits for authority-paid fees.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct AuthorityFeePayment {
        /// Canonically ordered per-component charge limits.
        pub charge_limits: Vec<FeeChargeLimit>,
        /// Maximum executable gas; required for contract and IVM transactions.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub gas_limit: Option<NonZeroU64>,
    }

    /// Signature-bound limits and exact revision for sponsor-program fees.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct SponsorFeePayment {
        /// Exact sponsor program selected before signing.
        pub program_id: FeeSponsorProgramId,
        /// Exact immutable program revision accepted by the signer.
        pub program_revision: u64,
        /// Canonically ordered per-component charge limits.
        pub charge_limits: Vec<FeeChargeLimit>,
        /// Maximum executable gas; required for contract and IVM transactions.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub gas_limit: Option<NonZeroU64>,
    }

    /// Required signature-bound choice of fee funding source and limits.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(
        tag = "payer",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum FeePaymentIntent {
        /// Charge the transaction authority directly.
        Authority(AuthorityFeePayment),
        /// Charge one exact revision of an on-chain sponsor program.
        Sponsor(SponsorFeePayment),
    }

    /// Canonical unsigned transaction draft used by quote, signing, and verification APIs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct TransactionPayload {
        /// Unique id of the blockchain. Used for simple replay attack protection.
        pub chain: ChainId,
        /// Account ID of transaction creator. Signing rejects a key mismatch;
        /// it never rewrites this signature-bound field.
        pub authority: AccountId,
        /// Creation timestamp (unix time in milliseconds).
        pub creation_time_ms: u64,
        /// ISI or IVM smart contract bytecode.
        pub instructions: Executable,
        /// Required signature-bound lifetime. `None` exists only so malformed
        /// wire input can be decoded and rejected deterministically.
        pub time_to_live_ms: Option<NonZeroU64>,
        /// Random value to make different hashes for transactions which occur repeatedly and simultaneously.
        pub nonce: Option<NonZeroU32>,
        /// Explicit fee payer, assets, limits, and executable gas bound.
        pub fee_payment: FeePaymentIntent,
        /// Store for additional information.
        pub metadata: Metadata,
        /// Proof attachments whose exact contents affect transaction execution.
        ///
        /// Attachments are part of the signed intent. Relays cannot add,
        /// remove, or replace them without invalidating every authorization
        /// signature and changing the transaction identifier.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub attachments: Option<crate::proof::ProofAttachmentList>,
    }

    /// Signature of transaction
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct TransactionSignature(pub SignatureOf<TransactionPayload>);

    /// A single signature produced by a multisig member.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct MultisigSignature {
        /// Signer public key.
        pub signer: iroha_crypto::PublicKey,
        /// Signature over the transaction payload produced by `signer`.
        pub signature: SignatureOf<TransactionPayload>,
    }

    impl MultisigSignature {
        /// Construct a new multisig signature entry.
        pub fn new(
            signer: iroha_crypto::PublicKey,
            signature: SignatureOf<TransactionPayload>,
        ) -> Self {
            Self { signer, signature }
        }
    }

    /// Collection of multisig signatures attached to a transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct MultisigSignatures {
        /// Signature entries provided by multisig members.
        pub signatures: Vec<MultisigSignature>,
    }

    impl MultisigSignatures {
        /// Construct a bundle in canonical signer order.
        pub fn new(mut signatures: Vec<MultisigSignature>) -> Self {
            signatures.sort_by(|left, right| left.signer.cmp(&right.signer));
            Self { signatures }
        }

        /// Validate the unique canonical signer ordering required on the wire.
        ///
        /// # Errors
        ///
        /// Returns an error when a signer occurs more than once or entries are
        /// not strictly ordered by public key.
        pub fn validate_canonical(&self) -> Result<(), TransactionSignatureError> {
            if self
                .signatures
                .windows(2)
                .any(|pair| pair[0].signer >= pair[1].signer)
            {
                return Err(TransactionSignatureError::NonCanonicalMultisigSignatures);
            }
            Ok(())
        }
    }

    impl<'a> norito::core::DecodeFromSlice<'a> for TransactionSignature {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let (inner, used) =
                <SignatureOf<TransactionPayload> as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
            Ok((TransactionSignature(inner), used))
        }
    }

    /// Payload signed when committing to a sealed transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SealedTransactionCommitmentPayload {
        /// Unique id of the blockchain.
        pub chain_id: ChainId,
        /// Account authorized to later reveal the transaction.
        pub authority: AccountId,
        /// Commitment to the canonical signed transaction bytes and salt.
        pub commitment: Hash,
        /// First block height where the reveal may execute.
        pub reveal_after_height: u64,
        /// Last block height where the reveal may execute.
        pub reveal_deadline_height: u64,
        /// Optional nonce to let an authority submit multiple indistinguishable commitments.
        pub nonce: Option<NonZeroU64>,
    }

    /// Signed sealed-transaction commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SignedSealedTransactionCommitment {
        /// Signature over [`Self::payload`].
        pub(super) signature: SignatureOf<SealedTransactionCommitmentPayload>,
        /// Commitment payload.
        pub(super) payload: SealedTransactionCommitmentPayload,
    }

    /// Reveal data for a previously committed sealed transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SealedTransactionReveal {
        /// Commitment hash being opened.
        pub commitment: Hash,
        /// Canonical signed transaction hidden by the commitment.
        pub signed_transaction: SignedTransaction,
        /// Salt used when computing the commitment.
        pub salt: [u8; 32],
    }

    /// Transaction containing a signed intent and its authorization proof.
    ///
    /// `Iroha` and its clients use [`Self`] to send transactions over the network.
    /// After a transaction is signed and before it can be processed any further,
    /// the transaction must be accepted by an `Iroha` peer.
    /// The peer verifies the signature and checks the limits.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[display("{}", self.hash())]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SignedTransaction {
        /// Signature of [`Self::payload`].
        pub(super) signature: TransactionSignature,
        /// Payload of the transaction.
        pub(super) payload: TransactionPayload,
        /// Optional bundle of multisig signatures when the authority uses a multisig controller.
        pub(super) multisig_signatures: Option<MultisigSignatures>,
    }

    /// Structure that represents the initial state of a transaction before the transaction receives any signatures.
    #[derive(Debug, Clone)]
    #[must_use]
    pub struct TransactionBuilder {
        /// [`Transaction`] payload.
        pub(super) payload: TransactionPayload,
        /// Optional multisig signature bundle to include upon signing.
        pub(super) multisig_signatures: Option<MultisigSignatures>,
    }

    /// Initial execution step of a transaction, which may invoke data triggers.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Display,
        Decode,
        Encode,
        From,
        TryInto,
        IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum TransactionEntrypoint {
        /// User request that initiates a transaction.
        External(SignedTransaction),
        /// Commitment to a sealed transaction, executed after reveal.
        SealedCommitment(SignedSealedTransactionCommitment),
        /// Reveal of a previously committed sealed transaction.
        SealedReveal(SealedTransactionReveal),
        /// Authority-free private Kaigi request.
        PrivateKaigi(PrivateKaigiTransaction),
        /// Scheduled time trigger that initiates a transaction.
        Time(TimeTriggerEntrypoint),
    }

    /// The outcome of processing a transaction:
    /// either a sequence of data triggers, or a rejection reason.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TransactionResult(
        pub TransactionResultInner,
        /// Durable per-leg receipts emitted by an independently settled native transfer batch.
        pub Vec<AssetBatchTransferOutcome>,
    );

    /// The outcome of processing a transaction:
    /// either a sequence of data triggers, or a rejection reason.
    pub type TransactionResultInner =
        Result<DataTriggerSequence, error::TransactionRejectionReason>;

    /// Single execution step in a transaction, comprising ordered instructions.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Display,
        Decode,
        Encode,
        From,
        Deref,
        IntoSchema,
    )]
    #[display("ExecutionStep")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ExecutionStep(pub ConstVec<InstructionBox>);
}

// Keep explicit slice decoders for hot ingress paths. The generic derived
// decoders regressed on versioned payloads carrying adaptive Norito bodies.
fn decode_signed_transaction_with_cursor(
    bytes: &[u8],
) -> Result<(model::SignedTransaction, usize), norito::core::Error> {
    let _guard = norito::core::PayloadCtxGuard::enter(bytes);
    let mut cursor = std::io::Cursor::new(bytes);
    let decoded = <model::SignedTransaction as norito::codec::Decode>::decode(&mut cursor)?;
    let used =
        usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
    Ok((decoded, used))
}

fn decode_transaction_payload_with_cursor(
    bytes: &[u8],
) -> Result<(model::TransactionPayload, usize), norito::core::Error> {
    let _guard = norito::core::PayloadCtxGuard::enter(bytes);
    let mut cursor = std::io::Cursor::new(bytes);
    let decoded = <model::TransactionPayload as norito::codec::Decode>::decode(&mut cursor)?;
    let used =
        usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
    Ok((decoded, used))
}

fn read_aos_field<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    flags: u8,
) -> Result<&'a [u8], norito::core::Error> {
    let remaining = bytes
        .get(*offset..)
        .ok_or(norito::core::Error::LengthMismatch)?;
    let (field_len, hdr) = norito::core::read_len_from_slice_with_flags(remaining, flags)?;
    let field_start = (*offset)
        .checked_add(hdr)
        .ok_or(norito::core::Error::LengthMismatch)?;
    let field_end = field_start
        .checked_add(field_len)
        .ok_or(norito::core::Error::LengthMismatch)?;
    let field = bytes
        .get(field_start..field_end)
        .ok_or(norito::core::Error::LengthMismatch)?;
    *offset = field_end;
    Ok(field)
}

fn decode_slice_field<T>(field: &[u8], flags: u8) -> Result<T, norito::core::Error>
where
    T: for<'de> norito::core::NoritoDeserialize<'de> + for<'de> norito::core::DecodeFromSlice<'de>,
{
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let (value, used) = norito::core::decode_field_canonical_from_slice::<T>(field)?;
    if used != field.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    Ok(value)
}

fn decode_canonical_field<T>(field: &[u8], flags: u8) -> Result<T, norito::core::Error>
where
    T: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
    let (value, used) = norito::core::decode_field_canonical::<T>(field)?;
    if used != field.len() {
        return Err(norito::core::Error::LengthMismatch);
    }
    Ok(value)
}

fn decode_codec_field<T>(field: &[u8]) -> Result<T, norito::core::Error>
where
    T: norito::codec::Decode,
{
    let mut cursor = std::io::Cursor::new(field);
    <T as norito::codec::Decode>::decode(&mut cursor)
}

impl<'a> norito::core::DecodeFromSlice<'a> for model::TransactionPayload {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return decode_transaction_payload_with_cursor(bytes);
        }

        let mut offset = 0usize;
        let chain =
            decode_canonical_field::<ChainId>(read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let authority =
            decode_slice_field::<AccountId>(read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let creation_time_ms =
            decode_slice_field::<u64>(read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let instructions =
            decode_slice_field::<Executable>(read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let time_to_live_ms = decode_slice_field::<Option<NonZeroU64>>(
            read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let nonce = decode_slice_field::<Option<NonZeroU32>>(
            read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let fee_payment = decode_canonical_field::<FeePaymentIntent>(
            read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let metadata =
            decode_canonical_field::<Metadata>(read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let attachments = decode_slice_field::<Option<crate::proof::ProofAttachmentList>>(
            read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                chain,
                authority,
                creation_time_ms,
                instructions,
                time_to_live_ms,
                nonce,
                fee_payment,
                metadata,
                attachments,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for model::SignedTransaction {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return decode_signed_transaction_with_cursor(bytes);
        }

        let mut offset = 0usize;
        let signature =
            decode_codec_field::<TransactionSignature>(read_aos_field(bytes, &mut offset, flags)?)?;
        let payload = decode_slice_field::<TransactionPayload>(
            read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let multisig_signatures = decode_slice_field::<Option<MultisigSignatures>>(
            read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                signature,
                payload,
                multisig_signatures,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for model::TransactionEntrypoint {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let _guard = norito::core::PayloadCtxGuard::enter(bytes);
        let mut cursor = std::io::Cursor::new(bytes);
        let decoded: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        let used =
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
        Ok((decoded, used))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for model::ExecutionStep {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        let (field_len, field_hdr) = norito::core::read_len_from_slice_with_flags(bytes, flags)?;
        let field_start = field_hdr;
        let field_end = field_start
            .checked_add(field_len)
            .ok_or(norito::core::Error::LengthMismatch)?;
        if field_end != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        let field = bytes
            .get(field_start..field_end)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let (instructions, used) =
            <ConstVec<InstructionBox> as norito::core::DecodeFromSlice>::decode_from_slice(field)?;
        if used != field.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, field_end);
        Ok((Self(instructions), field_end))
    }
}

/// Error returned when verifying a transaction signature.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransactionSignatureError {
    /// Multisig authorities are not supported for transaction signing yet.
    #[error("{MULTISIG_SIGNING_UNSUPPORTED_REASON}")]
    UnsupportedMultisigAuthority,
    /// Signature algorithm is not allowed by configuration.
    #[error("signature algorithm {0} is not permitted by configuration")]
    AlgorithmNotPermitted(Algorithm),
    /// The supplied private key does not control the exact payload authority.
    #[error("transaction authority does not match the supplied signing key")]
    AuthorityKeyMismatch,
    /// Signature verification failed for the provided payload and signatory.
    #[error("{0}")]
    CryptoError(String),
    /// The transaction does not contain any signatures.
    #[error("transaction carries no signatures")]
    NoSignatures,
    /// Multisig signature bundle is missing.
    #[error("missing multisig signatures for multisig authority")]
    MissingMultisigSignatures,
    /// A single-controller transaction carried an unrelated multisig proof.
    #[error("single-controller transaction must not carry multisig signatures")]
    UnexpectedMultisigSignatures,
    /// Transaction contains a signature from a non-member.
    #[error("multisig signature from unknown member")]
    UnknownMultisigSigner,
    /// Multisig signatures are duplicated or not ordered by signer.
    #[error("multisig signatures are not in canonical distinct signer order")]
    NonCanonicalMultisigSignatures,
    /// The signature-bound fee payment intent is malformed or ambiguous.
    #[error("invalid fee payment intent: {0}")]
    InvalidFeePaymentIntent(String),
    /// A signable transaction payload omitted its signature-bound lifetime.
    #[error("transaction time_to_live_ms is required")]
    MissingTimeToLive,
    /// Collected multisig signatures do not satisfy the policy threshold.
    #[error("insufficient multisig weight: collected {collected}, required {required}")]
    InsufficientMultisigWeight {
        #[doc = "Total weight contributed by provided signatures."]
        collected: u32,
        #[doc = "Threshold required by the multisig policy."]
        required: u16,
    },
}

static EXPIRES_AT_HEIGHT_NAME: LazyLock<Name> = LazyLock::new(|| {
    Name::from_str("expires_at_height").expect("expires_at_height is a valid metadata key")
});

/// Stable reason string for rejecting multisig controllers in tx signing paths.
pub const MULTISIG_SIGNING_UNSUPPORTED_REASON: &str =
    "multisig authority requires bundled signatures for verification";

/// Domain separation tag for sealed transaction commitment hashing.
pub const SEALED_TRANSACTION_COMMITMENT_DOMAIN: &[u8] = b"iroha.sealed_tx.v1";

/// Structural error in a signature-bound fee payment intent.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FeePaymentIntentError {
    /// Sponsor revisions start at one; zero cannot identify an active revision.
    #[error("sponsor program revision must be non-zero")]
    ZeroProgramRevision,
    /// Charge limits must be ordered by fee component.
    #[error("charge limits are not in canonical fee-component order")]
    NonCanonicalChargeLimitOrder,
    /// A fee component may select exactly one asset and maximum amount.
    #[error("duplicate charge limit for component {0:?}")]
    DuplicateChargeKind(FeeChargeKind),
    /// A signed maximum must authorize a positive amount.
    #[error("charge limit for component {kind:?} and asset {asset_definition_id} is zero")]
    ZeroChargeLimit {
        /// Component carrying the zero limit.
        kind: FeeChargeKind,
        /// Asset selected by the invalid limit.
        asset_definition_id: AssetDefinitionId,
    },
    /// Retired metadata keys must never coexist with the typed fee surface.
    #[error("legacy transaction metadata key `{0}` is not supported")]
    LegacyMetadataKey(String),
}

impl FeeChargeLimit {
    /// Construct a signature-bound limit for one exact fee component and asset.
    #[must_use]
    pub const fn new(
        kind: FeeChargeKind,
        asset_definition_id: AssetDefinitionId,
        max_amount: Quantity,
    ) -> Self {
        Self {
            kind,
            asset_definition_id,
            max_amount,
        }
    }
}

impl FeePaymentIntent {
    /// Construct an authority-paid intent.
    #[must_use]
    pub const fn authority(
        charge_limits: Vec<FeeChargeLimit>,
        gas_limit: Option<NonZeroU64>,
    ) -> Self {
        Self::Authority(AuthorityFeePayment {
            charge_limits,
            gas_limit,
        })
    }

    /// Construct an exact sponsor-program intent.
    #[must_use]
    pub const fn sponsor(
        program_id: FeeSponsorProgramId,
        program_revision: u64,
        charge_limits: Vec<FeeChargeLimit>,
        gas_limit: Option<NonZeroU64>,
    ) -> Self {
        Self::Sponsor(SponsorFeePayment {
            program_id,
            program_revision,
            charge_limits,
            gas_limit,
        })
    }

    /// Return the canonical per-component charge limits.
    #[must_use]
    pub fn charge_limits(&self) -> &[FeeChargeLimit] {
        match self {
            Self::Authority(payment) => &payment.charge_limits,
            Self::Sponsor(payment) => &payment.charge_limits,
        }
    }

    /// Return the signed executable gas limit, when applicable.
    #[must_use]
    pub const fn gas_limit(&self) -> Option<NonZeroU64> {
        match self {
            Self::Authority(payment) => payment.gas_limit,
            Self::Sponsor(payment) => payment.gas_limit,
        }
    }

    /// Return the exact sponsor program and revision, if sponsorship was selected.
    #[must_use]
    pub const fn sponsor_program(&self) -> Option<(&FeeSponsorProgramId, u64)> {
        match self {
            Self::Authority(_) => None,
            Self::Sponsor(payment) => Some((&payment.program_id, payment.program_revision)),
        }
    }

    /// Return whether two intents select the same payer and executable gas bound.
    ///
    /// Quote-to-sign clients use this before replacing the draft's charge
    /// maxima, preventing a quote from substituting an authority/program,
    /// immutable program revision, or gas authorization.
    #[must_use]
    pub fn has_same_payer_and_gas_bound(&self, other: &Self) -> bool {
        let same_payer = match (self.sponsor_program(), other.sponsor_program()) {
            (None, None) => true,
            (Some(left), Some(right)) => left == right,
            _ => false,
        };
        same_payer && self.gas_limit() == other.gas_limit()
    }

    /// Validate canonical ordering, uniqueness, revision, and positive maxima.
    ///
    /// Empty limits are structurally valid because fee-free networks have no
    /// applicable components. Admission rejects missing limits whenever a fee
    /// component is enabled by the authoritative schedule.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero sponsor revision, a zero charge maximum,
    /// or charge limits that are duplicated or not in canonical order.
    pub fn validate(&self) -> Result<(), FeePaymentIntentError> {
        if matches!(
            self,
            Self::Sponsor(SponsorFeePayment {
                program_revision: 0,
                ..
            })
        ) {
            return Err(FeePaymentIntentError::ZeroProgramRevision);
        }

        let mut previous = None;
        for limit in self.charge_limits() {
            if limit.max_amount.is_zero() {
                return Err(FeePaymentIntentError::ZeroChargeLimit {
                    kind: limit.kind,
                    asset_definition_id: limit.asset_definition_id.clone(),
                });
            }
            if let Some(previous) = previous {
                if limit.kind == previous {
                    return Err(FeePaymentIntentError::DuplicateChargeKind(limit.kind));
                }
                if limit.kind < previous {
                    return Err(FeePaymentIntentError::NonCanonicalChargeLimitOrder);
                }
            }
            previous = Some(limit.kind);
        }
        Ok(())
    }

    fn validate_metadata(metadata: &Metadata) -> Result<(), FeePaymentIntentError> {
        for key in ["fee_sponsor", "gas_limit", "gas_asset_id"] {
            if metadata.get(key).is_some() {
                return Err(FeePaymentIntentError::LegacyMetadataKey(key.to_owned()));
            }
        }
        Ok(())
    }
}

static TX_SEQUENCE_NAME: LazyLock<Name> =
    LazyLock::new(|| Name::from_str("tx_sequence").expect("tx_sequence is a valid metadata key"));

/// Domain separator for the canonical first-release privacy transaction intent.
pub const PRIVACY_TRANSACTION_INTENT_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.transaction-intent-digest.v1";

/// Dynamic or opaque executable path excluded from the V1 privacy intent projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyTransactionIntentUnsupportedPathV1 {
    /// A deployed contract call can enqueue instructions not present in the signed payload.
    ContractCall,
    /// Raw IVM bytecode can enqueue instructions not present in the signed payload.
    Ivm,
    /// A proved IVM overlay is not a direct signed native-instruction list.
    IvmProved,
    /// A mixed batch contains a deployed contract call.
    BatchContractCall,
    /// A custom instruction is interpreted by an executor rather than this closed projection.
    CustomInstruction,
    /// A by-call trigger can execute instructions outside the signed payload.
    ExecuteTrigger,
    /// An opaque instruction has no locally auditable typed payload.
    OpaqueInstruction,
}

/// Failure to derive or validate a canonical V1 privacy transaction intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyTransactionIntentErrorV1 {
    /// The payload has no direct typed privacy proof submission.
    #[error("privacy transaction intent requires exactly one direct SubmitPrivacyProofV1")]
    MissingSubmission,
    /// More than one direct typed privacy proof submission is present.
    #[error("privacy transaction intent contains {count} direct privacy submissions")]
    MultipleSubmissions {
        /// Observed direct submission count.
        count: u64,
    },
    /// The payload uses a dynamic or opaque path excluded from the V1 projection.
    #[error("privacy transaction intent contains an unsupported dynamic or opaque path")]
    UnsupportedPath {
        /// Exact excluded path.
        path: PrivacyTransactionIntentUnsupportedPathV1,
    },
    /// Canonical encoding of the normalized unsigned payload failed.
    #[error("canonical privacy transaction-intent payload encoding failed")]
    PayloadEncodingFailure,
    /// Canonical encoding of the complete statement failed.
    #[error("canonical privacy statement encoding failed during transaction-intent validation")]
    StatementEncodingFailure,
    /// A platform payload length cannot be represented by the fixed u64 frame.
    #[error("privacy transaction-intent payload length overflow")]
    PayloadLengthOverflow,
    /// The stored intent digest is zero.
    #[error("privacy statement transaction-intent digest must not be zero")]
    ZeroIntentDigest,
    /// The stored intent digest is stale or belongs to a different payload.
    #[error("privacy statement transaction-intent digest differs from the canonical payload")]
    IntentDigestMismatch {
        /// Digest recomputed from the canonical normalized payload.
        expected: PrivacyTransactionIntentDigestV1,
        /// Digest carried by the signed statement.
        actual: PrivacyTransactionIntentDigestV1,
    },
    /// The stored statement digest is zero.
    #[error("privacy envelope statement digest must not be zero")]
    ZeroStatementDigest,
    /// The stored statement digest does not commit to the final statement.
    #[error("privacy envelope statement digest differs from the canonical statement")]
    StatementDigestMismatch {
        /// Digest recomputed from the final statement.
        expected: PrivacyStatementDigestV1,
        /// Digest carried by the signed envelope.
        actual: PrivacyStatementDigestV1,
    },
}

#[derive(Default)]
struct PrivacyTransactionIntentScan<'a> {
    first_submission: Option<&'a SubmitPrivacyProofV1>,
    direct_submission_count: u64,
    unsupported_path: Option<PrivacyTransactionIntentUnsupportedPathV1>,
    privacy_in_unsupported_path: bool,
}

impl<'a> PrivacyTransactionIntentScan<'a> {
    fn inspect_direct_instruction(&mut self, instruction: &'a InstructionBox) {
        if let Some(submission) = instruction.as_any().downcast_ref::<SubmitPrivacyProofV1>() {
            self.direct_submission_count = self.direct_submission_count.saturating_add(1);
            self.first_submission.get_or_insert(submission);
            return;
        }
        if let Some(opaque) = instruction.as_any().downcast_ref::<OpaqueInstruction>() {
            self.unsupported_path
                .get_or_insert(PrivacyTransactionIntentUnsupportedPathV1::OpaqueInstruction);
            if opaque.wire_id() == SubmitPrivacyProofV1::WIRE_ID {
                self.privacy_in_unsupported_path = true;
            }
        } else if instruction
            .as_any()
            .downcast_ref::<CustomInstruction>()
            .is_some()
        {
            self.unsupported_path
                .get_or_insert(PrivacyTransactionIntentUnsupportedPathV1::CustomInstruction);
        } else if instruction
            .as_any()
            .downcast_ref::<ExecuteTrigger>()
            .is_some()
        {
            self.unsupported_path
                .get_or_insert(PrivacyTransactionIntentUnsupportedPathV1::ExecuteTrigger);
        }
    }

    fn inspect_proved_overlay_instruction(&mut self, instruction: &InstructionBox) {
        if instruction
            .as_any()
            .downcast_ref::<SubmitPrivacyProofV1>()
            .is_some()
            || instruction
                .as_any()
                .downcast_ref::<OpaqueInstruction>()
                .is_some_and(|opaque| opaque.wire_id() == SubmitPrivacyProofV1::WIRE_ID)
        {
            self.privacy_in_unsupported_path = true;
        }
    }
}

fn scan_privacy_transaction_intent_v1(executable: &Executable) -> PrivacyTransactionIntentScan<'_> {
    let mut scan = PrivacyTransactionIntentScan::default();
    match executable {
        Executable::Instructions(instructions) => {
            for instruction in instructions {
                scan.inspect_direct_instruction(instruction);
            }
        }
        Executable::Batch(items) => {
            for item in items {
                match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        scan.inspect_direct_instruction(instruction);
                    }
                    ExecutableBatchItem::ContractCall(_) => {
                        scan.unsupported_path.get_or_insert(
                            PrivacyTransactionIntentUnsupportedPathV1::BatchContractCall,
                        );
                    }
                }
            }
        }
        Executable::ContractCall(_) => {
            scan.unsupported_path
                .replace(PrivacyTransactionIntentUnsupportedPathV1::ContractCall);
        }
        Executable::Ivm(_) => {
            scan.unsupported_path
                .replace(PrivacyTransactionIntentUnsupportedPathV1::Ivm);
        }
        Executable::IvmProved(proved) => {
            scan.unsupported_path
                .replace(PrivacyTransactionIntentUnsupportedPathV1::IvmProved);
            for instruction in &proved.overlay {
                scan.inspect_proved_overlay_instruction(instruction);
            }
        }
    }
    scan
}

fn validate_exact_direct_privacy_submission_v1<'a>(
    scan: &PrivacyTransactionIntentScan<'a>,
) -> Result<&'a SubmitPrivacyProofV1, PrivacyTransactionIntentErrorV1> {
    if scan.direct_submission_count == 0 {
        if let Some(path) = scan.unsupported_path {
            return Err(PrivacyTransactionIntentErrorV1::UnsupportedPath { path });
        }
        return Err(PrivacyTransactionIntentErrorV1::MissingSubmission);
    }
    if scan.direct_submission_count != 1 {
        return Err(PrivacyTransactionIntentErrorV1::MultipleSubmissions {
            count: scan.direct_submission_count,
        });
    }
    if let Some(path) = scan.unsupported_path {
        return Err(PrivacyTransactionIntentErrorV1::UnsupportedPath { path });
    }
    Ok(scan
        .first_submission
        .expect("a non-zero direct submission count stores its first submission"))
}

fn normalize_privacy_submission_for_intent_v1(submission: &SubmitPrivacyProofV1) -> InstructionBox {
    let mut normalized = submission.clone();
    normalized.envelope.proof.bytes_mut().bytes.clear();
    normalized
        .envelope
        .statement
        .context_mut()
        .transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
    if let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) =
        &mut normalized.envelope.statement
    {
        statement.replay_nullifier = PrivacyNullifierV1::new([0; 32]);
    }
    if let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
        &mut normalized.envelope.statement
    {
        statement.device_authentication_digest =
            PrivacyVegaDeviceAuthenticationDigestV1::new([0; 32]);
    }
    if let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) =
        &mut normalized.envelope.statement
    {
        statement.action_digest = crate::privacy::PrivacyActionDigestV1::new([0; 32]);
    }
    normalized.envelope.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
    normalized.into()
}

fn normalize_privacy_executable_for_intent_v1(
    executable: &Executable,
) -> Result<Executable, PrivacyTransactionIntentErrorV1> {
    let scan = scan_privacy_transaction_intent_v1(executable);
    let _target = validate_exact_direct_privacy_submission_v1(&scan)?;
    match executable {
        Executable::Instructions(instructions) => {
            let normalized = instructions
                .iter()
                .map(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<SubmitPrivacyProofV1>()
                        .map_or_else(
                            || instruction.clone(),
                            normalize_privacy_submission_for_intent_v1,
                        )
                })
                .collect::<Vec<_>>();
            Ok(Executable::Instructions(normalized.into()))
        }
        Executable::Batch(items) => {
            let normalized = items
                .iter()
                .map(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => {
                        ExecutableBatchItem::Instruction(
                            instruction
                                .as_any()
                                .downcast_ref::<SubmitPrivacyProofV1>()
                                .map_or_else(
                                    || instruction.clone(),
                                    normalize_privacy_submission_for_intent_v1,
                                ),
                        )
                    }
                    ExecutableBatchItem::ContractCall(call) => {
                        ExecutableBatchItem::ContractCall(call.clone())
                    }
                })
                .collect::<Vec<_>>();
            Ok(Executable::Batch(normalized.into()))
        }
        Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => {
            unreachable!("exact direct privacy submission validation excludes opaque executables")
        }
    }
}

#[cfg(feature = "fault_injection")]
static FAULT_INJECTION_METADATA_NAME: LazyLock<Name> = LazyLock::new(|| {
    Name::from_str("fault_injection_overlay")
        .expect("fault_injection_overlay is a valid metadata key")
});

impl TransactionPayload {
    /// Validate the complete signature-bound fee intent.
    ///
    /// This combines the typed fee-intent invariants with rejection of retired
    /// metadata keys so every admission path can apply one canonical check.
    ///
    /// # Errors
    ///
    /// Returns an error when the typed intent is non-canonical or legacy fee
    /// metadata is present.
    pub fn validate_fee_payment_intent(&self) -> Result<(), FeePaymentIntentError> {
        self.fee_payment
            .validate()
            .and_then(|()| FeePaymentIntent::validate_metadata(&self.metadata))
    }

    /// Return the canonical privacy transaction-intent projection bytes.
    ///
    /// This is the exact unsigned-payload preimage consumed by
    /// [`Self::privacy_transaction_intent_digest_v1`]. Exposing the projection
    /// removes independent SDK approximations: callers can decode and
    /// byte-identically reproduce the Rust-owned normalization before signing.
    ///
    /// # Errors
    ///
    /// Returns a closed error for zero or multiple submissions, dynamic/opaque
    /// execution paths, or canonical encoding failure.
    pub fn privacy_transaction_intent_projection_bytes_v1(
        &self,
    ) -> Result<Vec<u8>, PrivacyTransactionIntentErrorV1> {
        let mut normalized = self.clone();
        normalized.instructions =
            normalize_privacy_executable_for_intent_v1(&normalized.instructions)?;
        norito::encode_canonical(&normalized)
            .map_err(|_| PrivacyTransactionIntentErrorV1::PayloadEncodingFailure)
    }

    /// Derive the canonical privacy transaction-intent digest from this unsigned payload.
    ///
    /// V1 accepts exactly one direct typed [`SubmitPrivacyProofV1`] in either
    /// [`Executable::Instructions`] or the native-instruction members of
    /// [`Executable::Batch`]. The canonical projection clones the complete
    /// unsigned payload and changes exactly three universally derived values:
    ///
    /// - the typed proof byte vector becomes empty;
    /// - `statement.context.transaction_intent_digest` becomes 32 zero bytes;
    /// - `envelope.statement_digest` becomes 32 zero bytes.
    ///
    /// For ZK-ACE, the replay nullifier also becomes 32 zero bytes because it
    /// is derived from the resulting intent-bound authorization projection.
    /// For Vega, the device-authentication digest also becomes 32 zero bytes
    /// because `H_dev` binds the resulting transaction-intent digest. For the
    /// native IVM private-note protocol, the self-authenticating action digest
    /// likewise becomes 32 zero bytes because its canonical preimage includes
    /// the resulting transaction-intent digest.
    ///
    /// Zeroing these derived fields removes their otherwise unavoidable
    /// self-reference. Every independent payload field, statement field,
    /// instruction tag, and instruction ordinal remains in the Norito preimage.
    ///
    /// # Errors
    ///
    /// Returns a closed error for zero or multiple submissions, dynamic/opaque
    /// execution paths, or canonical encoding failure.
    pub fn privacy_transaction_intent_digest_v1(
        &self,
    ) -> Result<PrivacyTransactionIntentDigestV1, PrivacyTransactionIntentErrorV1> {
        let encoded = self.privacy_transaction_intent_projection_bytes_v1()?;
        let encoded_len = u64::try_from(encoded.len())
            .map_err(|_| PrivacyTransactionIntentErrorV1::PayloadLengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_TRANSACTION_INTENT_DIGEST_DOMAIN_V1);
        hasher.update(&encoded_len.to_le_bytes());
        hasher.update(&encoded);
        Ok(PrivacyTransactionIntentDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }

    /// Validate the final stored intent and statement digests.
    ///
    /// This is the admission-side companion to
    /// [`Self::privacy_transaction_intent_digest_v1`]. It recomputes both
    /// derived values from the exact signed payload and rejects zero, stale, or
    /// foreign values before any executor effect.
    ///
    /// # Errors
    ///
    /// Returns a canonical projection error or an exact derived-field mismatch.
    pub fn validate_privacy_transaction_intent_binding_v1(
        &self,
    ) -> Result<PrivacyTransactionIntentDigestV1, PrivacyTransactionIntentErrorV1> {
        let scan = scan_privacy_transaction_intent_v1(&self.instructions);
        let submission = validate_exact_direct_privacy_submission_v1(&scan)?;
        let expected_intent = self.privacy_transaction_intent_digest_v1()?;
        let actual_intent = submission
            .envelope
            .statement
            .context()
            .transaction_intent_digest;
        if actual_intent.is_zero() {
            return Err(PrivacyTransactionIntentErrorV1::ZeroIntentDigest);
        }
        if actual_intent != expected_intent {
            return Err(PrivacyTransactionIntentErrorV1::IntentDigestMismatch {
                expected: expected_intent,
                actual: actual_intent,
            });
        }

        let expected_statement = submission
            .envelope
            .statement
            .digest()
            .map_err(|_| PrivacyTransactionIntentErrorV1::StatementEncodingFailure)?;
        let actual_statement = submission.envelope.statement_digest;
        if actual_statement.is_zero() {
            return Err(PrivacyTransactionIntentErrorV1::ZeroStatementDigest);
        }
        if actual_statement != expected_statement {
            return Err(PrivacyTransactionIntentErrorV1::StatementDigestMismatch {
                expected: expected_statement,
                actual: actual_statement,
            });
        }
        Ok(expected_intent)
    }

    /// Validate and borrow an optional direct privacy submission for runtime admission.
    ///
    /// Ordinary non-privacy transactions return `Ok(None)`. A typed submission
    /// hidden in a proved overlay or an opaque instruction bearing the privacy
    /// wire id fails closed. When one direct submission exists, every V1
    /// projection and derived-field rule is enforced.
    ///
    /// # Errors
    ///
    /// Returns a closed projection or binding error for any privacy-bearing
    /// payload that is not exactly one auditable direct submission.
    pub fn privacy_transaction_intent_binding_if_present_v1(
        &self,
    ) -> Result<
        Option<(PrivacyTransactionIntentDigestV1, &SubmitPrivacyProofV1)>,
        PrivacyTransactionIntentErrorV1,
    > {
        let scan = scan_privacy_transaction_intent_v1(&self.instructions);
        if scan.direct_submission_count == 0 {
            if scan.privacy_in_unsupported_path {
                return Err(PrivacyTransactionIntentErrorV1::UnsupportedPath {
                    path: scan
                        .unsupported_path
                        .expect("privacy observed in an unsupported path records that path"),
                });
            }
            return Ok(None);
        }
        let submission = validate_exact_direct_privacy_submission_v1(&scan)?;
        let digest = self.validate_privacy_transaction_intent_binding_v1()?;
        Ok(Some((digest, submission)))
    }

    /// Return transaction instructions.
    #[inline]
    pub fn instructions(&self) -> &Executable {
        &self.instructions
    }

    /// Return transaction authority.
    #[inline]
    pub fn authority(&self) -> &AccountId {
        &self.authority
    }

    /// Return the required signature-bound transaction lifetime.
    ///
    /// `None` identifies malformed decoded input; safe builders always assign
    /// a non-zero value and transaction admission rejects `None`.
    #[inline]
    pub fn time_to_live(&self) -> Option<Duration> {
        self.time_to_live_ms
            .map(|ttl| Duration::from_millis(ttl.into()))
    }

    /// Return transaction chain id.
    #[inline]
    pub fn chain(&self) -> &ChainId {
        &self.chain
    }
}

impl SignedTransaction {
    /// Derive the privacy intent from the canonical signed payload.
    ///
    /// Authorization signatures are excluded from the intent preimage, while
    /// execution-affecting proof attachments are included.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyTransactionIntentErrorV1`] if instruction inspection
    /// or canonical intent encoding fails.
    pub fn privacy_transaction_intent_digest_v1(
        &self,
    ) -> Result<PrivacyTransactionIntentDigestV1, PrivacyTransactionIntentErrorV1> {
        self.payload.privacy_transaction_intent_digest_v1()
    }

    /// Validate and borrow the optional direct privacy submission in this signed transaction.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyTransactionIntentErrorV1`] if the transaction contains
    /// an invalid privacy-instruction combination or its canonical intent
    /// cannot be derived.
    pub fn privacy_transaction_intent_binding_if_present_v1(
        &self,
    ) -> Result<
        Option<(PrivacyTransactionIntentDigestV1, &SubmitPrivacyProofV1)>,
        PrivacyTransactionIntentErrorV1,
    > {
        self.payload
            .privacy_transaction_intent_binding_if_present_v1()
    }

    /// Transaction payload. Used for tests
    pub fn payload(&self) -> &TransactionPayload {
        &self.payload
    }

    /// Return transaction instructions
    #[inline]
    pub fn instructions(&self) -> &Executable {
        self.payload.instructions()
    }

    /// Return transaction authority
    #[inline]
    pub fn authority(&self) -> &AccountId {
        self.payload.authority()
    }

    /// Return transaction metadata.
    #[inline]
    pub fn metadata(&self) -> &Metadata {
        &self.payload.metadata
    }

    /// Return the exact signature-bound fee payment intent.
    #[inline]
    pub fn fee_payment_intent(&self) -> &FeePaymentIntent {
        &self.payload.fee_payment
    }

    /// Multisig signature bundle attached to this transaction, if any.
    #[inline]
    pub fn multisig_signatures(&self) -> Option<&MultisigSignatures> {
        self.multisig_signatures.as_ref()
    }

    /// Creation timestamp as [`core::time::Duration`]
    #[inline]
    pub fn creation_time(&self) -> Duration {
        Duration::from_millis(self.payload.creation_time_ms)
    }

    /// Replace the transaction authority without re-signing the payload.
    ///
    /// Useful for tests that need to simulate malformed or unsupported
    /// authorities without going through the builder.
    #[inline]
    #[must_use]
    pub fn with_authority(mut self, authority: AccountId) -> Self {
        self.payload.authority = authority;
        self
    }

    /// Return the required signature-bound transaction lifetime.
    ///
    /// `None` identifies malformed decoded input.
    #[inline]
    pub fn time_to_live(&self) -> Option<Duration> {
        self.payload.time_to_live()
    }

    /// Transaction nonce
    #[inline]
    pub fn nonce(&self) -> Option<NonZeroU32> {
        self.payload.nonce
    }

    /// Transaction chain id
    #[inline]
    pub fn chain(&self) -> &ChainId {
        self.payload.chain()
    }

    /// Return the transaction signature
    #[inline]
    pub fn signature(&self) -> &TransactionSignature {
        &self.signature
    }

    /// Attach a multisig signature bundle, replacing any existing entry.
    #[inline]
    pub fn set_multisig_signatures(&mut self, signatures: MultisigSignatures) {
        self.multisig_signatures = Some(signatures);
    }

    /// Replace the transaction signature.
    #[cfg(feature = "transparent_api")]
    #[inline]
    pub fn set_signature(&mut self, signature: TransactionSignature) {
        self.signature = signature;
    }

    /// Number of signatures bundled with this transaction.
    ///
    /// Current transactions carry exactly one signature for single-key
    /// authorities. Multisig authorities count the raw signature entries in the
    /// multisig bundle (including duplicates) so admission can enforce bundle
    /// size limits.
    #[inline]
    pub fn signature_count(&self) -> usize {
        match self.payload.authority.controller() {
            AccountController::Single(_) => 1,
            AccountController::Multisig(_) => self
                .multisig_signatures
                .as_ref()
                .map_or(0, |bundle| bundle.signatures.len()),
        }
    }

    /// Optional proof attachments carried alongside the payload.
    #[inline]
    pub fn attachments(&self) -> Option<&crate::proof::ProofAttachmentList> {
        self.payload.attachments.as_ref()
    }

    /// Height-based TTL advertised via transaction metadata.
    ///
    /// Returns `Ok(None)` when the metadata key is absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the stored metadata value cannot be decoded as `u64`.
    pub fn expires_at_height(&self) -> Result<Option<u64>, norito::Error> {
        self.metadata()
            .get(&*EXPIRES_AT_HEIGHT_NAME)
            .map(Json::try_into_any_norito::<u64>)
            .transpose()
    }

    /// Per-authority transaction sequence advertised via metadata.
    ///
    /// Returns `Ok(None)` when the metadata key is absent.
    ///
    /// # Errors
    ///
    /// Returns an error if the stored metadata value cannot be decoded as `u64`.
    pub fn tx_sequence(&self) -> Result<Option<u64>, norito::Error> {
        self.metadata()
            .get(&*TX_SEQUENCE_NAME)
            .map(Json::try_into_any_norito::<u64>)
            .transpose()
    }

    /// Canonical identifier for this external transaction.
    ///
    /// The identifier commits to the exact signed intent and excludes the
    /// authorization proof. Adding or replacing a signature therefore cannot
    /// create a second identity for the same authorized action.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        let entry_hash = self.hash_as_entrypoint();
        HashOf::from_untyped_unchecked(Hash::from(entry_hash))
    }

    /// Hash for this external transaction as `TransactionEntrypoint`.
    ///
    /// This matches the canonical transaction hash returned by [`Self::hash`].
    #[inline]
    pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
        let entry_hash = HashOf::new(&ExternalEntrypointRef(self));
        HashOf::from_untyped_unchecked(Hash::from(entry_hash))
    }

    /// Injects a set of fictitious instructions into the transaction payload for testing.
    ///
    /// Only available when the `fault_injection` feature is enabled.
    #[cfg(feature = "fault_injection")]
    pub fn inject_instructions(
        &mut self,
        extra_instructions: impl IntoIterator<Item = impl Into<InstructionBox>>,
    ) {
        let additions: Vec<InstructionBox> =
            extra_instructions.into_iter().map(Into::into).collect();

        if additions.is_empty() {
            return;
        }

        match &mut self.payload.instructions {
            Executable::Instructions(instructions) => {
                let mut modified = instructions.clone().into_vec();
                modified.extend(additions);
                *instructions = modified.into();
            }
            Executable::Batch(items) => {
                let mut modified = items.clone().into_vec();
                modified.extend(
                    additions
                        .into_iter()
                        .map(crate::transaction::ExecutableBatchItem::Instruction),
                );
                *items = modified.into();
            }
            Executable::ContractCall(_) | Executable::Ivm(_) | Executable::IvmProved(_) => {
                Self::apply_fault_injection_overlay(&mut self.payload.metadata, additions);
            }
        }
    }

    #[cfg(feature = "fault_injection")]
    pub(crate) fn fault_injection_overlay(metadata: &Metadata) -> Option<Vec<String>> {
        metadata
            .get(&*FAULT_INJECTION_METADATA_NAME)
            .cloned()
            .and_then(|value| value.try_into_any_norito::<Vec<String>>().ok())
    }

    #[cfg(feature = "fault_injection")]
    pub(crate) fn apply_fault_injection_overlay(
        metadata: &mut Metadata,
        additions: Vec<InstructionBox>,
    ) {
        let mut combined = Self::fault_injection_overlay(metadata).unwrap_or_default();
        combined.extend(additions.into_iter().map(|instruction| {
            let bytes = norito::encode_canonical(&instruction)
                .expect("fault injection overlay instruction canonical encode");
            BASE64_STANDARD.encode(bytes)
        }));
        metadata.insert(FAULT_INJECTION_METADATA_NAME.clone(), Json::new(combined));
    }

    /// Verify transaction signature.
    ///
    /// # Errors
    ///
    /// Returns an error if signature verification fails.
    #[inline]
    pub fn verify_signature(&self) -> Result<(), TransactionSignatureError> {
        self.payload
            .validate_fee_payment_intent()
            .map_err(|err| TransactionSignatureError::InvalidFeePaymentIntent(err.to_string()))?;
        let TransactionSignature(signature) = &self.signature;
        match self.payload.authority.controller() {
            AccountController::Single(signatory) => {
                if self.multisig_signatures.is_some() {
                    return Err(TransactionSignatureError::UnexpectedMultisigSignatures);
                }
                verify_typed_signature_for_signer(signature, signatory, &self.payload)
                    .map_err(|err| TransactionSignatureError::CryptoError(err.to_string()))
            }
            AccountController::Multisig(policy) => self.verify_multisig_signatures(policy),
        }
    }
}

impl SealedTransactionCommitmentPayload {
    /// Construct a sealed transaction commitment payload.
    #[must_use]
    pub fn new(
        chain_id: ChainId,
        authority: AccountId,
        commitment: Hash,
        reveal_after_height: u64,
        reveal_deadline_height: u64,
        nonce: Option<NonZeroU64>,
    ) -> Self {
        Self {
            chain_id,
            authority,
            commitment,
            reveal_after_height,
            reveal_deadline_height,
            nonce,
        }
    }
}

impl SignedSealedTransactionCommitment {
    /// Try to sign a sealed transaction commitment payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured signature backend cannot sign the
    /// commitment payload with the supplied private key.
    pub fn try_sign(
        payload: SealedTransactionCommitmentPayload,
        private_key: &iroha_crypto::PrivateKey,
    ) -> Result<Self, iroha_crypto::Error> {
        Ok(Self {
            signature: SignatureOf::try_new(private_key, &payload)?,
            payload,
        })
    }

    /// Sign a sealed transaction commitment payload.
    #[must_use]
    pub fn sign(
        payload: SealedTransactionCommitmentPayload,
        private_key: &iroha_crypto::PrivateKey,
    ) -> Self {
        Self::try_sign(payload, private_key)
            .expect("signing should succeed for a valid key pair and sealed commitment payload")
    }

    /// Commitment payload.
    #[inline]
    #[must_use]
    pub fn payload(&self) -> &SealedTransactionCommitmentPayload {
        &self.payload
    }

    /// Account authorized to reveal the transaction.
    #[inline]
    #[must_use]
    pub fn authority(&self) -> &AccountId {
        &self.payload.authority
    }

    /// Commitment hash.
    #[inline]
    #[must_use]
    pub fn commitment(&self) -> &Hash {
        &self.payload.commitment
    }

    /// Signature over the commitment payload.
    #[inline]
    #[must_use]
    pub fn signature(&self) -> &SignatureOf<SealedTransactionCommitmentPayload> {
        &self.signature
    }

    /// Verify the commitment signature.
    ///
    /// # Errors
    ///
    /// Returns an error if the authority is multisig or signature verification fails.
    #[inline]
    pub fn verify_signature(&self) -> Result<(), TransactionSignatureError> {
        match self.payload.authority.controller() {
            AccountController::Single(signatory) => {
                verify_typed_signature_for_signer(&self.signature, signatory, &self.payload)
                    .map_err(|err| TransactionSignatureError::CryptoError(err.to_string()))
            }
            AccountController::Multisig(_) => {
                Err(TransactionSignatureError::UnsupportedMultisigAuthority)
            }
        }
    }
}

impl core::fmt::Display for SignedSealedTransactionCommitment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.commitment().fmt(f)
    }
}

impl SealedTransactionReveal {
    /// Construct a sealed transaction reveal.
    #[must_use]
    pub fn new(commitment: Hash, signed_transaction: SignedTransaction, salt: [u8; 32]) -> Self {
        Self {
            commitment,
            signed_transaction,
            salt,
        }
    }

    /// Revealed signed transaction.
    #[inline]
    #[must_use]
    pub fn signed_transaction(&self) -> &SignedTransaction {
        &self.signed_transaction
    }

    /// Recompute the expected commitment using the stored commitment deadline.
    #[must_use]
    pub fn expected_commitment_with_deadline(&self, reveal_deadline_height: u64) -> Hash {
        compute_sealed_transaction_commitment(
            self.signed_transaction.chain(),
            &self.signed_transaction,
            self.salt,
            reveal_deadline_height,
        )
    }
}

impl core::fmt::Display for SealedTransactionReveal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.commitment.fmt(f)
    }
}

/// Compute the canonical sealed transaction commitment.
///
/// The input is domain-separated and includes the chain id, the hash of canonical Norito
/// signed-transaction bytes, the salt, and the reveal deadline height.
#[must_use]
pub fn compute_sealed_transaction_commitment(
    chain_id: &ChainId,
    signed_transaction: &SignedTransaction,
    salt: [u8; 32],
    reveal_deadline_height: u64,
) -> Hash {
    let tx_bytes = norito::encode_canonical(signed_transaction)
        .expect("signed transaction must canonically encode to Norito");
    let tx_hash = Hash::new(tx_bytes);
    let chain_bytes =
        norito::encode_canonical(chain_id).expect("chain id must canonically encode to Norito");
    let mut bytes = Vec::with_capacity(
        SEALED_TRANSACTION_COMMITMENT_DOMAIN.len()
            + chain_bytes.len()
            + Hash::LENGTH
            + salt.len()
            + core::mem::size_of::<u64>(),
    );
    bytes.extend_from_slice(SEALED_TRANSACTION_COMMITMENT_DOMAIN);
    bytes.extend_from_slice(&chain_bytes);
    bytes.extend_from_slice(tx_hash.as_ref());
    bytes.extend_from_slice(&salt);
    bytes.extend_from_slice(&reveal_deadline_height.to_le_bytes());
    Hash::new(bytes)
}

impl SignedTransaction {
    fn verify_multisig_signatures(
        &self,
        policy: &MultisigPolicy,
    ) -> Result<(), TransactionSignatureError> {
        let Some(bundle) = self.multisig_signatures.as_ref() else {
            return Err(TransactionSignatureError::MissingMultisigSignatures);
        };
        if bundle.signatures.is_empty() {
            return Err(TransactionSignatureError::NoSignatures);
        }
        bundle.validate_canonical()?;
        let TransactionSignature(primary_signature) = &self.signature;
        if &bundle.signatures[0].signature != primary_signature {
            return Err(TransactionSignatureError::NonCanonicalMultisigSignatures);
        }

        let mut weights = BTreeMap::new();
        for member in policy.members() {
            weights.insert(member.public_key().clone(), member.weight());
        }

        let mut collected: u32 = 0;
        for entry in &bundle.signatures {
            let Some(weight) = weights.get(&entry.signer) else {
                return Err(TransactionSignatureError::UnknownMultisigSigner);
            };
            verify_typed_signature_for_signer(&entry.signature, &entry.signer, &self.payload)
                .map_err(|err| TransactionSignatureError::CryptoError(err.to_string()))?;
            collected = collected.saturating_add(u32::from(*weight));
        }

        if collected < u32::from(policy.threshold()) {
            return Err(TransactionSignatureError::InsufficientMultisigWeight {
                collected,
                required: policy.threshold(),
            });
        }

        Ok(())
    }
}

impl iroha_version::Version for SignedTransaction {
    fn version(&self) -> u8 {
        1
    }

    fn supported_versions() -> core::ops::Range<u8> {
        1..2
    }
}

fn encode_default_layout_versioned<T>(version: u8, value: &T) -> Vec<u8>
where
    T: norito::NoritoSerialize,
{
    let mut bytes = Vec::with_capacity(1 + value.encoded_len_hint().unwrap_or(0));
    bytes.push(version);
    let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    value
        .serialize(&mut bytes)
        .expect("versioned transaction encoding should not fail");
    bytes
}

impl iroha_version::codec::EncodeVersioned for SignedTransaction {
    fn encode_versioned(&self) -> Vec<u8> {
        encode_default_layout_versioned(self.version(), self)
    }
}

impl iroha_version::codec::DecodeVersioned for SignedTransaction {
    fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        iroha_version::codec::decode_exact_versioned(input)
    }
}

impl iroha_version::Version for TransactionEntrypoint {
    fn version(&self) -> u8 {
        1
    }

    fn supported_versions() -> core::ops::Range<u8> {
        1..2
    }
}

impl iroha_version::codec::EncodeVersioned for TransactionEntrypoint {
    fn encode_versioned(&self) -> Vec<u8> {
        encode_default_layout_versioned(self.version(), self)
    }
}

impl iroha_version::codec::DecodeVersioned for TransactionEntrypoint {
    fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        iroha_version::codec::decode_exact_versioned(input)
    }
}

#[cfg(feature = "transparent_api")]
impl From<SignedTransaction> for (AccountId, Executable) {
    fn from(source: SignedTransaction) -> Self {
        (source.payload.authority, source.payload.instructions)
    }
}

impl TransactionSignature {
    /// Signature itself
    pub fn payload(&self) -> &Signature {
        &self.0
    }
}

impl MultisigSignatures {
    /// Produce a multisig signature bundle by signing the given payload with each private key.
    ///
    /// # Errors
    ///
    /// Returns [`TransactionSignatureError::NoSignatures`] if no signers are provided.
    #[allow(single_use_lifetimes)]
    pub fn from_signers<'a>(
        payload: &TransactionPayload,
        signers: impl IntoIterator<Item = &'a iroha_crypto::PrivateKey>,
    ) -> Result<Self, TransactionSignatureError> {
        let signatures: Vec<MultisigSignature> = signers
            .into_iter()
            .map(|private_key| {
                let signer = PublicKey::from(private_key.clone());
                let signature = SignatureOf::try_new(private_key, payload)
                    .map_err(|err| TransactionSignatureError::CryptoError(err.to_string()))?;
                Ok(MultisigSignature::new(signer, signature))
            })
            .collect::<Result<_, _>>()?;

        if signatures.is_empty() {
            return Err(TransactionSignatureError::NoSignatures);
        }

        Ok(Self::new(signatures))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for TransactionEntrypoint {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        match self {
            TransactionEntrypoint::External(tx) => {
                norito::json::write_json_string("External", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(tx, out);
            }
            TransactionEntrypoint::SealedCommitment(commitment) => {
                norito::json::write_json_string("SealedCommitment", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(commitment, out);
            }
            TransactionEntrypoint::SealedReveal(reveal) => {
                norito::json::write_json_string("SealedReveal", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(reveal, out);
            }
            TransactionEntrypoint::PrivateKaigi(tx) => {
                norito::json::write_json_string("PrivateKaigi", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(tx, out);
            }
            TransactionEntrypoint::Time(trigger) => {
                norito::json::write_json_string("Time", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(trigger, out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TransactionEntrypoint {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        let key = parser.parse_key()?;
        let value = match key.as_str() {
            "External" => {
                let tx = SignedTransaction::json_deserialize(parser)?;
                TransactionEntrypoint::External(tx)
            }
            "SealedCommitment" => {
                let commitment = SignedSealedTransactionCommitment::json_deserialize(parser)?;
                TransactionEntrypoint::SealedCommitment(commitment)
            }
            "SealedReveal" => {
                let reveal = SealedTransactionReveal::json_deserialize(parser)?;
                TransactionEntrypoint::SealedReveal(reveal)
            }
            "PrivateKaigi" => {
                let tx = PrivateKaigiTransaction::json_deserialize(parser)?;
                TransactionEntrypoint::PrivateKaigi(tx)
            }
            "Time" => {
                let trigger = TimeTriggerEntrypoint::json_deserialize(parser)?;
                TransactionEntrypoint::Time(trigger)
            }
            other => {
                return Err(norito::json::Error::UnknownField {
                    field: other.to_owned(),
                });
            }
        };
        parser.skip_ws();
        parser.consume_char(b'}')?;
        Ok(value)
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for TransactionResult {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match &self.0 {
            Ok(sequence) => {
                norito::json::write_json_string("Ok", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(sequence, out);
            }
            Err(reason) => {
                norito::json::write_json_string("Err", out);
                out.push(':');
                norito::json::JsonSerialize::json_serialize(reason, out);
            }
        }
        out.push(',');
        norito::json::write_json_string("batch_transfer_outcomes", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.1, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for TransactionResult {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        parser.skip_ws();
        parser.consume_char(b'{')?;
        let mut inner = None;
        let mut batch_transfer_outcomes = None;
        loop {
            parser.skip_ws();
            if parser.try_consume_char(b'}')? {
                break;
            }

            let key = parser.parse_key()?;
            match key.as_str() {
                "Ok" => {
                    if inner.is_some() {
                        return Err(norito::json::Error::duplicate_field("Ok"));
                    }
                    inner = Some(TransactionResultInner::Ok(
                        DataTriggerSequence::json_deserialize(parser)?,
                    ));
                }
                "Err" => {
                    if inner.is_some() {
                        return Err(norito::json::Error::duplicate_field("Err"));
                    }
                    inner = Some(TransactionResultInner::Err(
                        error::TransactionRejectionReason::json_deserialize(parser)?,
                    ));
                }
                "batch_transfer_outcomes" => {
                    if batch_transfer_outcomes.is_some() {
                        return Err(norito::json::Error::duplicate_field(
                            "batch_transfer_outcomes",
                        ));
                    }
                    batch_transfer_outcomes =
                        Some(Vec::<AssetBatchTransferOutcome>::json_deserialize(parser)?);
                }
                other => return Err(norito::json::Error::unknown_field(other.to_owned())),
            }

            parser.skip_ws();
            if parser.try_consume_char(b',')? {
                continue;
            }
            parser.consume_char(b'}')?;
            break;
        }

        Ok(TransactionResult(
            inner.ok_or_else(|| norito::json::Error::missing_field("Ok or Err"))?,
            batch_transfer_outcomes.unwrap_or_default(),
        ))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ExecutionStep {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ExecutionStep {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        ConstVec::<InstructionBox>::json_deserialize(parser).map(ExecutionStep)
    }
}

struct ExternalEntrypointRef<'a>(&'a SignedTransaction);

impl norito::core::NoritoSerialize for ExternalEntrypointRef<'_> {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&0_u32, &mut writer)?;
        let mut tmp = norito::core::DeriveSmallBuf::new();
        norito::core::write_len_prefixed(&mut writer, self.0.payload(), &mut tmp)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0
            .payload()
            .encoded_len_hint()
            .map(|len| 4_usize.saturating_add(8).saturating_add(len))
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let len = self.0.payload().encoded_len_exact()?;
        Some(
            4_usize
                .saturating_add(norito::core::len_prefix_len(len))
                .saturating_add(len),
        )
    }
}

impl TransactionBuilder {
    fn new_with_time(
        chain: ChainId,
        authority: AccountId,
        creation_time_ms: u64,
        fee_payment: FeePaymentIntent,
    ) -> Self {
        Self {
            payload: TransactionPayload {
                chain,
                authority,
                creation_time_ms,
                nonce: None,
                time_to_live_ms: Some(
                    NonZeroU64::new(
                        u64::try_from(DEFAULT_TRANSACTION_TIME_TO_LIVE.as_millis())
                            .expect("default transaction TTL fits in u64 milliseconds"),
                    )
                    .expect("default transaction TTL is non-zero"),
                ),
                instructions: Vec::<InstructionBox>::new().into(),
                fee_payment,
                metadata: Metadata::default(),
                attachments: None,
            },
            multisig_signatures: None,
        }
    }

    /// Construct [`Self`], using the time from [`TimeSource`]
    // we don't want to expose this to non-tests
    #[inline]
    pub fn new_with_time_source(
        chain_id: ChainId,
        authority: AccountId,
        time_source: &TimeSource,
        fee_payment: FeePaymentIntent,
    ) -> Self {
        let creation_time_ms = time_source
            .get_unix_time()
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX");

        Self::new_with_time(chain_id, authority, creation_time_ms, fee_payment)
    }

    /// Construct [`Self`] with the exact signature-bound fee payment intent.
    #[inline]
    pub fn new(chain_id: ChainId, authority: AccountId, fee_payment: FeePaymentIntent) -> Self {
        Self::new_with_time_source(chain_id, authority, &TimeSource::new_system(), fee_payment)
    }
}

impl TransactionBuilder {
    fn validate_payload(payload: &TransactionPayload) -> Result<(), TransactionSignatureError> {
        if payload.time_to_live_ms.is_none() {
            return Err(TransactionSignatureError::MissingTimeToLive);
        }
        payload
            .validate_fee_payment_intent()
            .map_err(|err| TransactionSignatureError::InvalidFeePaymentIntent(err.to_string()))
    }

    /// Reconstruct a transaction builder from one exact unsigned payload.
    ///
    /// The payload retains its signature-bound proof attachments. Only the
    /// authorization-proof bundle starts empty.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload's fee intent or metadata violates the
    /// canonical signature-bound fee policy.
    pub fn from_payload(payload: TransactionPayload) -> Result<Self, TransactionSignatureError> {
        Self::validate_payload(&payload)?;
        Ok(Self {
            payload,
            multisig_signatures: None,
        })
    }

    /// Consume the builder and return its exact unsigned payload.
    ///
    /// Proof attachments are part of the returned signature preimage.
    /// Multisig authorization proofs remain outside it.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload's fee intent or metadata violates the
    /// canonical signature-bound fee policy.
    pub fn into_payload(self) -> Result<TransactionPayload, TransactionSignatureError> {
        Self::validate_payload(&self.payload)?;
        Ok(self.payload)
    }

    /// Borrow the exact unsigned payload currently held by this builder.
    #[must_use]
    pub const fn payload(&self) -> &TransactionPayload {
        &self.payload
    }

    /// Set instructions for this transaction
    pub fn with_instructions<I>(mut self, instructions: impl IntoIterator<Item = I>) -> Self
    where
        I: Into<InstructionBox>,
    {
        self.payload.instructions = instructions
            .into_iter()
            .map(Into::into)
            .collect::<Vec<InstructionBox>>()
            .into();
        self
    }

    /// Add IVM bytecode to this transaction
    pub fn with_bytecode(mut self, bytecode: IvmBytecode) -> Self {
        self.payload.instructions = bytecode.into();
        self
    }

    /// Set executable for this transaction
    pub fn with_executable(mut self, executable: Executable) -> Self {
        self.payload.instructions = executable;
        self
    }

    /// Set an ordered, atomic mix of instructions and deployed-contract calls.
    ///
    /// An empty iterator can be represented for decoding symmetry but is
    /// rejected by transaction admission.
    pub fn with_executable_batch<I>(mut self, items: impl IntoIterator<Item = I>) -> Self
    where
        I: Into<crate::transaction::ExecutableBatchItem>,
    {
        self.payload.instructions = Executable::Batch(
            items
                .into_iter()
                .map(Into::into)
                .collect::<Vec<crate::transaction::ExecutableBatchItem>>()
                .into(),
        );
        self
    }

    /// Adds metadata to this transaction
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.payload.metadata = metadata;
        self
    }

    /// Set the required signature-bound fee payer and charge limits.
    pub fn with_fee_payment_intent(mut self, intent: FeePaymentIntent) -> Self {
        self.payload.fee_payment = intent;
        self
    }

    /// Attach proof payloads to this transaction before signing.
    pub fn with_attachments(mut self, attachments: crate::proof::ProofAttachmentList) -> Self {
        self.payload.attachments = Some(attachments);
        self
    }

    /// Attach multisig signatures for a multisig authority.
    pub fn with_multisig_signatures(mut self, signatures: MultisigSignatures) -> Self {
        self.multisig_signatures = Some(signatures);
        self
    }

    /// Set nonce for this transaction
    pub fn set_nonce(&mut self, nonce: NonZeroU32) -> &mut Self {
        self.payload.nonce = Some(nonce);
        self
    }

    /// Set time-to-live for this transaction
    ///
    /// A zero duration leaves the builder with an invalid missing lifetime;
    /// fallible payload/signing workflows then return
    /// [`TransactionSignatureError::MissingTimeToLive`].
    pub fn set_ttl(&mut self, time_to_live: Duration) -> &mut Self {
        let ttl: u64 = time_to_live
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX");

        self.payload.time_to_live_ms = NonZeroU64::new(ttl);

        self
    }

    /// Set creation time of transaction
    pub fn set_creation_time(&mut self, value: Duration) -> &mut Self {
        self.payload.creation_time_ms = u64::try_from(value.as_millis())
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX");
        self
    }

    /// Encode the transaction payload to canonical Norito bytes.
    ///
    /// This is the byte sequence external signers should receive before
    /// applying Iroha's typed transaction prehash.
    #[must_use]
    pub fn encode_payload(&self) -> Vec<u8> {
        norito::codec::encode_adaptive(&self.payload)
    }

    /// Reconstruct a transaction builder from an exact canonical payload archive.
    ///
    /// This is the inverse of [`Self::encode_payload`] for external-signature
    /// workflows. Trailing bytes are rejected so callers cannot sign one payload
    /// while later submitting a different envelope suffix.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when `bytes` is malformed, non-canonical for the
    /// default v1 layout, or contains trailing bytes.
    pub fn decode_payload(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        let _guard = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let (payload, used) = TransactionPayload::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        let builder = Self {
            payload,
            multisig_signatures: None,
        };
        Self::validate_payload(&builder.payload)
            .map_err(|error| norito::core::Error::Message(error.to_string()))?;
        if builder.encode_payload() != bytes {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(builder)
    }

    /// Return the canonical prehash signed by transaction signatures.
    #[must_use]
    pub fn payload_hash(&self) -> Hash {
        Hash::from(HashOf::new(&self.payload))
    }

    /// Return the canonical prehash signed by transaction signatures as raw bytes.
    #[must_use]
    pub fn payload_hash_bytes(&self) -> [u8; Hash::LENGTH] {
        *HashOf::new(&self.payload).as_ref()
    }

    /// Build a signed transaction from a signature produced by an external signer.
    ///
    /// The signature is not trusted blindly by this constructor. Callers that
    /// receive signatures over the wire should verify the returned transaction
    /// with [`SignedTransaction::verify_signature`] before submitting it.
    #[must_use]
    pub fn build_with_signature(self, signature: Signature) -> SignedTransaction {
        SignedTransaction {
            signature: TransactionSignature(SignatureOf::from_signature(signature)),
            payload: self.payload,
            multisig_signatures: self.multisig_signatures,
        }
    }

    /// Try to sign transaction with provided key pair.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied key does not control the exact
    /// signature-bound authority or the signature backend cannot sign.
    pub fn try_sign(
        self,
        private_key: &iroha_crypto::PrivateKey,
    ) -> Result<SignedTransaction, TransactionSignatureError> {
        use iroha_crypto::PublicKey;
        let payload = self.payload;

        Self::validate_payload(&payload)?;

        let expected = payload
            .authority
            .try_signatory()
            .ok_or(TransactionSignatureError::UnsupportedMultisigAuthority)?;
        let derived = PublicKey::from(private_key.clone());
        if expected != &derived {
            return Err(TransactionSignatureError::AuthorityKeyMismatch);
        }

        let signature = TransactionSignature(
            SignatureOf::try_new(private_key, &payload)
                .map_err(|err| TransactionSignatureError::CryptoError(err.to_string()))?,
        );

        Ok(SignedTransaction {
            signature,
            payload,
            multisig_signatures: self.multisig_signatures,
        })
    }

    /// Sign transaction with provided key pair.
    #[must_use]
    pub fn sign(self, private_key: &iroha_crypto::PrivateKey) -> SignedTransaction {
        self.try_sign(private_key)
            .expect("signing should succeed for a valid private key and transaction payload")
    }

    /// Try to sign a transaction whose authority uses a multisig controller.
    ///
    /// The provided signer keys are used to produce a canonical multisig
    /// signature bundle. Duplicate signers are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error if no signer keys are provided or a signature backend
    /// fails while signing one of the multisig entries.
    #[allow(single_use_lifetimes)]
    pub fn try_sign_multisig<'a>(
        self,
        signers: impl IntoIterator<Item = &'a iroha_crypto::PrivateKey>,
    ) -> Result<SignedTransaction, TransactionSignatureError> {
        let payload = self.payload;
        Self::validate_payload(&payload)?;
        let mut bundle = self
            .multisig_signatures
            .unwrap_or_else(|| MultisigSignatures::new(Vec::new()));

        let produced = MultisigSignatures::from_signers(&payload, signers)?;
        bundle.signatures.extend(produced.signatures);
        bundle = MultisigSignatures::new(bundle.signatures);
        bundle.validate_canonical()?;

        let primary_signature = bundle
            .signatures
            .first()
            .expect("multisig signing requires at least one signer")
            .signature
            .clone();

        Ok(SignedTransaction {
            signature: TransactionSignature(primary_signature),
            payload,
            multisig_signatures: Some(bundle),
        })
    }

    /// Sign a transaction whose authority uses a multisig controller.
    ///
    /// The provided signer keys are used to produce a canonical multisig
    /// signature bundle. Duplicate signers are rejected.
    ///
    /// # Panics
    ///
    /// Panics if no signer keys are provided or a signature backend fails while
    /// signing one of the multisig entries.
    #[must_use]
    #[allow(single_use_lifetimes)]
    pub fn sign_multisig<'a>(
        self,
        signers: impl IntoIterator<Item = &'a iroha_crypto::PrivateKey>,
    ) -> SignedTransaction {
        self.try_sign_multisig(signers)
            .expect("multisig signing requires at least one valid signer")
    }
}

#[cfg(test)]
mod tests {
    use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        Domain, DomainId, Level,
        account::{MultisigMember, MultisigPolicy},
        prelude::{Log, Register, TriggerId},
        privacy::{
            IROHA_JINDO_FIELD_ELEMENT_BYTES_V1, IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1,
            IrohaIvmPrivateNoteStarkStatementV1, IrohaJindoPolynomialCommitmentStatementV1,
            PrivacyActionDigestV1, PrivacyChallengeV1, PrivacyCommitmentV1,
            PrivacyCredentialDocumentTypeV1, PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1,
            PrivacyJindoFieldElementV1, PrivacyJindoLatticeCommitmentV1, PrivacyNullifierV1,
            PrivacyP256PointV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
            PrivacyPolicyDigestV1, PrivacyPolicyIdV1, PrivacyPoolIdV1, PrivacyProgramIdV1,
            PrivacyProofBytesV1, PrivacyProofEnvelopeV1, PrivacyProofSystemIdV1, PrivacyProofV1,
            PrivacyProtocolIdV1, PrivacyRootV1, PrivacySessionTranscriptDigestV1,
            PrivacyStatementContextV1, PrivacyStatementDigestV1, PrivacyStatementSchemaDigestV1,
            PrivacyStatementV1, PrivacyTransactionIntentDigestV1, PrivacyValueBalanceV1,
            PrivacyVegaDeviceAuthenticationDigestV1, PrivacyVegaIssuerRecordDigestV1,
            PrivacyVegaMdlDateV1, PrivacyVegaMdlDigestAlgorithmV1, PrivacyVegaMdlNamespaceV1,
            PrivacyVegaMdlSignatureAlgorithmV1, PrivacyVerifierDigestV1,
            VegaExistingCredentialStatementV1, ZkAcePqAuthorizationStatementV1,
        },
        transaction::{
            ExecutableBatchItem,
            executable::{ContractInvocation, IvmProved},
            signed::{MultisigSignature, MultisigSignatures},
        },
        trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
    };

    fn sample_signed_transaction() -> SignedTransaction {
        let chain: ChainId = "test-chain".parse().unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(public_key);
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "exact slice".into())])
        .sign(&private_key)
    }

    fn sample_fee_asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("fees", "universal").expect("valid fee domain"),
            "xor".parse().expect("valid fee asset name"),
        )
    }

    fn privacy_test_authority() -> AccountId {
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("test public key");
        AccountId::new(public_key)
    }

    fn privacy_test_private_key() -> iroha_crypto::PrivateKey {
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .expect("test private key")
    }

    const fn privacy_test_bytes(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn draft_privacy_submission() -> SubmitPrivacyProofV1 {
        let protocol_id = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0;
        let parameter_id = PrivacyParameterIdV1::new(privacy_test_bytes(1));
        let parameter_digest = PrivacyParameterDigestV1::new(privacy_test_bytes(2));
        let verifier_digest = PrivacyVerifierDigestV1::new(privacy_test_bytes(3));
        let statement_schema_digest = PrivacyStatementSchemaDigestV1::new(privacy_test_bytes(4));
        let engine_manifest_digest = PrivacyEngineManifestDigestV1::new(privacy_test_bytes(5));
        let statement = PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(
            IrohaJindoPolynomialCommitmentStatementV1 {
                context: PrivacyStatementContextV1 {
                    chain_id: ChainId::from("privacy-intent-test"),
                    action_index: 0,
                    parameter_id,
                    parameter_digest,
                    verifier_digest,
                    statement_schema_digest,
                    engine_manifest_digest,
                    transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
                },
                polynomial_commitments: vec![PrivacyJindoLatticeCommitmentV1::new(vec![
                    6;
                    IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1
                ])],
                evaluation_point: PrivacyJindoFieldElementV1::new(
                    [7; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1],
                ),
                claimed_evaluations: vec![PrivacyJindoFieldElementV1::new(
                    [8; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1],
                )],
            },
        );
        SubmitPrivacyProofV1::new(PrivacyProofEnvelopeV1 {
            protocol_id,
            proof_system_id: PrivacyProofSystemIdV1::JindoPolynomialCommitment,
            engine_id: protocol_id.expected_engine(),
            parameter_id,
            parameter_digest,
            verifier_digest,
            statement_schema_digest,
            engine_manifest_digest,
            statement_digest: PrivacyStatementDigestV1::new([0; 32]),
            statement,
            proof: PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(PrivacyProofBytesV1::new(
                vec![0xA5, 0x5A, 1],
            )),
        })
    }

    fn privacy_payload_with_executable(executable: Executable) -> TransactionPayload {
        TransactionBuilder::new_with_time(
            ChainId::from("privacy-intent-test"),
            privacy_test_authority(),
            1_725_000_000_000,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(executable)
        .into_payload()
        .expect("valid test payload")
    }

    fn draft_privacy_payload() -> TransactionPayload {
        privacy_payload_with_executable(
            vec![InstructionBox::from(draft_privacy_submission())].into(),
        )
    }

    fn draft_zk_ace_privacy_payload() -> TransactionPayload {
        let mut payload = draft_privacy_payload();
        mutate_direct_privacy_submission(&mut payload, |submission| {
            let context = submission.envelope.statement.context().clone();
            let authority = privacy_test_authority();
            submission.envelope.protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
            submission.envelope.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks;
            submission.envelope.engine_id =
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0.expected_engine();
            submission.envelope.statement =
                PrivacyStatementV1::ZkAcePqAuthorizationV0(ZkAcePqAuthorizationStatementV1 {
                    context,
                    identity_commitment: crate::privacy::PrivacyCommitmentV1::new(
                        privacy_test_bytes(0x71),
                    ),
                    policy_id: PrivacyPolicyIdV1::new(privacy_test_bytes(0x72)),
                    policy_digest: PrivacyPolicyDigestV1::new(privacy_test_bytes(0x73)),
                    source: authority.clone(),
                    destination: authority,
                    asset_definition_id: sample_fee_asset(),
                    amount: 7,
                    authorization_epoch: 1,
                    replay_nullifier: PrivacyNullifierV1::new(privacy_test_bytes(0x74)),
                });
            submission.envelope.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
            submission.envelope.proof =
                PrivacyProofV1::ZkAcePqAuthorizationV0(PrivacyProofBytesV1::new(vec![0xA5, 0x5A]));
        });
        payload
    }

    fn draft_vega_privacy_payload() -> TransactionPayload {
        let mut payload = draft_privacy_payload();
        mutate_direct_privacy_submission(&mut payload, |submission| {
            let context = submission.envelope.statement.context().clone();
            let protocol_id = PrivacyProtocolIdV1::VegaExistingCredentialZkV0;
            submission.envelope.protocol_id = protocol_id;
            submission.envelope.proof_system_id =
                PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256;
            submission.envelope.engine_id = protocol_id.expected_engine();
            submission.envelope.statement =
                PrivacyStatementV1::VegaExistingCredentialZkV0(VegaExistingCredentialStatementV1 {
                    context,
                    issuer_id: PrivacyIssuerIdV1::new(privacy_test_bytes(0x81)),
                    issuer_record_epoch: 1,
                    issuer_record_digest: PrivacyVegaIssuerRecordDigestV1::new(privacy_test_bytes(
                        0x82,
                    )),
                    document_type: PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl,
                    namespace: PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1,
                    digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1::Sha256,
                    issuer_authentication_algorithm:
                        PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
                    device_authentication_algorithm:
                        PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
                    issuer_public_key: PrivacyP256PointV1::new([
                        0x02, 0x6f, 0xf0, 0x3b, 0x94, 0x92, 0x41, 0xce, 0x1d, 0xad, 0xd4, 0x35,
                        0x19, 0xe6, 0x96, 0x0e, 0x0a, 0x85, 0xb4, 0x1a, 0x69, 0xa0, 0x5c, 0x32,
                        0x81, 0x03, 0xaa, 0x2b, 0xce, 0x15, 0x94, 0xca, 0x16,
                    ]),
                    device_authentication_digest: PrivacyVegaDeviceAuthenticationDigestV1::new(
                        privacy_test_bytes(0x83),
                    ),
                    presentation_date: PrivacyVegaMdlDateV1 {
                        year: 2026,
                        month: 7,
                        day: 28,
                    },
                    minimum_age_years: 18,
                    reader_challenge: PrivacyChallengeV1::new(privacy_test_bytes(0x84)),
                    session_transcript_digest: PrivacySessionTranscriptDigestV1::new(
                        privacy_test_bytes(0x85),
                    ),
                });
            submission.envelope.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
            submission.envelope.proof =
                PrivacyProofV1::VegaExistingCredentialZkV0(PrivacyProofBytesV1::new(vec![
                    0xA5, 0x5A,
                ]));
        });
        payload
    }

    fn draft_ivm_private_note_privacy_payload() -> TransactionPayload {
        let mut payload = draft_privacy_payload();
        mutate_direct_privacy_submission(&mut payload, |submission| {
            let context = submission.envelope.statement.context().clone();
            let protocol_id = PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1;
            submission.envelope.protocol_id = protocol_id;
            submission.envelope.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks;
            submission.envelope.engine_id = protocol_id.expected_engine();
            let mut statement = IrohaIvmPrivateNoteStarkStatementV1 {
                context,
                asset_definition_id: sample_fee_asset(),
                pool_id: PrivacyPoolIdV1::new(privacy_test_bytes(0x91)),
                program_id: PrivacyProgramIdV1::new(privacy_test_bytes(0x92)),
                action_digest: PrivacyActionDigestV1::new([0; 32]),
                state_root: PrivacyRootV1::new(privacy_test_bytes(0x93)),
                root_epoch: 7,
                nullifiers: vec![PrivacyNullifierV1::new(privacy_test_bytes(0x94))],
                output_commitments: vec![PrivacyCommitmentV1::new(privacy_test_bytes(0x95))],
                encrypted_outputs: Vec::new(),
                value_balance: PrivacyValueBalanceV1::balanced(),
                execution_epoch: 7,
            };
            statement.action_digest = statement
                .computed_action_digest()
                .expect("draft IVM action digest");
            submission.envelope.statement =
                PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement);
            submission.envelope.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
            submission.envelope.proof =
                PrivacyProofV1::IrohaIvmPrivateNoteStarkV1(PrivacyProofBytesV1::new(vec![
                    0xA5, 0x5A,
                ]));
        });
        payload
    }

    fn mutate_direct_privacy_submission(
        payload: &mut TransactionPayload,
        mutate: impl FnOnce(&mut SubmitPrivacyProofV1),
    ) {
        let Executable::Instructions(instructions) = &payload.instructions else {
            panic!("test helper requires direct instructions");
        };
        let mut instructions = instructions.clone().into_vec();
        let index = instructions
            .iter()
            .position(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<SubmitPrivacyProofV1>()
                    .is_some()
            })
            .expect("test payload has a privacy submission");
        let mut submission = instructions[index]
            .as_any()
            .downcast_ref::<SubmitPrivacyProofV1>()
            .expect("located typed privacy submission")
            .clone();
        mutate(&mut submission);
        instructions[index] = submission.into();
        payload.instructions = Executable::Instructions(instructions.into());
    }

    fn finalized_privacy_payload() -> TransactionPayload {
        let mut payload = draft_privacy_payload();
        let intent = payload
            .privacy_transaction_intent_digest_v1()
            .expect("draft derives a canonical intent");
        mutate_direct_privacy_submission(&mut payload, |submission| {
            submission
                .envelope
                .statement
                .context_mut()
                .transaction_intent_digest = intent;
            submission.envelope.statement_digest = submission
                .envelope
                .statement
                .digest()
                .expect("final statement digest");
        });
        assert_eq!(
            payload
                .validate_privacy_transaction_intent_binding_v1()
                .expect("final payload binding"),
            intent
        );
        payload
    }

    fn privacy_test_contract_call() -> ContractInvocation {
        ContractInvocation {
            contract_address: crate::smart_contract::ContractAddress::derive(
                0,
                &privacy_test_authority(),
                0,
                crate::nexus::DataSpaceId::UNIVERSAL,
            )
            .expect("test contract address"),
            expected_code_hash: Hash::new(b"privacy intent test contract"),
            entrypoint: "run".to_owned(),
            arguments: None,
        }
    }

    fn legacy_proof_only_privacy_intent_digest(
        payload: &TransactionPayload,
    ) -> PrivacyTransactionIntentDigestV1 {
        let mut normalized = payload.clone();
        mutate_direct_privacy_submission(&mut normalized, |submission| {
            submission.envelope.proof.bytes_mut().bytes.clear();
        });
        let encoded = norito::to_bytes(&normalized).expect("legacy projection encodes");
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_TRANSACTION_INTENT_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("test payload length fits u64")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        PrivacyTransactionIntentDigestV1::new(*hasher.finalize().as_bytes())
    }

    fn assert_privacy_binding_absent(payload: &TransactionPayload, message: &str) {
        assert!(
            payload
                .privacy_transaction_intent_binding_if_present_v1()
                .expect(message)
                .is_none()
        );
    }

    fn assert_privacy_digest_rejects_path(
        payload: &TransactionPayload,
        path: PrivacyTransactionIntentUnsupportedPathV1,
        message: &str,
    ) {
        assert_eq!(
            payload
                .privacy_transaction_intent_digest_v1()
                .expect_err(message),
            PrivacyTransactionIntentErrorV1::UnsupportedPath { path }
        );
    }

    fn assert_privacy_binding_rejects_path(
        payload: &TransactionPayload,
        path: PrivacyTransactionIntentUnsupportedPathV1,
        message: &str,
    ) {
        assert_eq!(
            payload
                .privacy_transaction_intent_binding_if_present_v1()
                .expect_err(message),
            PrivacyTransactionIntentErrorV1::UnsupportedPath { path }
        );
    }

    fn assert_privacy_ivm_paths_rejected() {
        let raw_ivm =
            privacy_payload_with_executable(Executable::Ivm(IvmBytecode::from_compiled(vec![1])));
        assert_privacy_digest_rejects_path(
            &raw_ivm,
            PrivacyTransactionIntentUnsupportedPathV1::Ivm,
            "raw IVM is not a direct typed submission",
        );
        assert_privacy_binding_absent(
            &raw_ivm,
            "an ordinary IVM transaction has no privacy binding",
        );

        let ordinary_proved = privacy_payload_with_executable(Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![2]),
            overlay: vec![InstructionBox::from(Log::new(
                Level::INFO,
                "ordinary proved overlay".into(),
            ))]
            .into(),
            events_commitment: Hash::new(b"ordinary events"),
            gas_policy_commitment: Hash::new(b"ordinary gas"),
        }));
        assert_privacy_binding_absent(
            &ordinary_proved,
            "an ordinary proved transaction has no privacy binding",
        );

        let proved = privacy_payload_with_executable(Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![2]),
            overlay: vec![InstructionBox::from(draft_privacy_submission())].into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        }));
        assert_privacy_binding_rejects_path(
            &proved,
            PrivacyTransactionIntentUnsupportedPathV1::IvmProved,
            "proved overlays cannot carry a V1 privacy submission",
        );
    }

    fn assert_privacy_dynamic_dispatch_paths_rejected() {
        let contract =
            privacy_payload_with_executable(Executable::ContractCall(privacy_test_contract_call()));
        assert_privacy_digest_rejects_path(
            &contract,
            PrivacyTransactionIntentUnsupportedPathV1::ContractCall,
            "contract call is opaque to the V1 projection",
        );
        assert_privacy_binding_absent(
            &contract,
            "an ordinary contract transaction has no privacy binding",
        );

        let mixed_batch = privacy_payload_with_executable(Executable::Batch(
            vec![
                ExecutableBatchItem::Instruction(draft_privacy_submission().into()),
                ExecutableBatchItem::ContractCall(privacy_test_contract_call()),
            ]
            .into(),
        ));
        assert_privacy_binding_rejects_path(
            &mixed_batch,
            PrivacyTransactionIntentUnsupportedPathV1::BatchContractCall,
            "a mixed contract batch can enqueue unsigned instructions",
        );
        let ordinary_contract_batch = privacy_payload_with_executable(Executable::Batch(
            vec![ExecutableBatchItem::ContractCall(
                privacy_test_contract_call(),
            )]
            .into(),
        ));
        assert_privacy_binding_absent(
            &ordinary_contract_batch,
            "an ordinary contract batch has no privacy binding",
        );

        let custom = privacy_payload_with_executable(
            vec![
                InstructionBox::from(draft_privacy_submission()),
                InstructionBox::from(CustomInstruction::new(Json::new("opaque executor"))),
            ]
            .into(),
        );
        assert_privacy_binding_rejects_path(
            &custom,
            PrivacyTransactionIntentUnsupportedPathV1::CustomInstruction,
            "custom executor path",
        );
        let ordinary_custom = privacy_payload_with_executable(
            vec![InstructionBox::from(CustomInstruction::new(Json::new(
                "ordinary executor",
            )))]
            .into(),
        );
        assert_privacy_binding_absent(
            &ordinary_custom,
            "an ordinary custom instruction has no privacy binding",
        );

        let trigger = privacy_payload_with_executable(
            vec![
                InstructionBox::from(draft_privacy_submission()),
                InstructionBox::from(ExecuteTrigger::new(
                    TriggerId::from_str("privacy_dynamic").expect("trigger id"),
                )),
            ]
            .into(),
        );
        assert_privacy_binding_rejects_path(
            &trigger,
            PrivacyTransactionIntentUnsupportedPathV1::ExecuteTrigger,
            "by-call trigger path",
        );
        let ordinary_trigger = privacy_payload_with_executable(
            vec![InstructionBox::from(ExecuteTrigger::new(
                TriggerId::from_str("ordinary_dynamic").expect("trigger id"),
            ))]
            .into(),
        );
        assert_privacy_binding_absent(
            &ordinary_trigger,
            "an ordinary trigger instruction has no privacy binding",
        );
    }

    fn assert_opaque_privacy_instruction_rejected() {
        let submission = draft_privacy_submission();
        let framed = norito::to_bytes(&submission).expect("framed privacy instruction");
        let opaque = OpaqueInstruction::from_framed(SubmitPrivacyProofV1::WIRE_ID, &framed)
            .expect("opaque privacy instruction fixture");
        let payload = privacy_payload_with_executable(vec![InstructionBox::from(opaque)].into());
        assert_privacy_binding_rejects_path(
            &payload,
            PrivacyTransactionIntentUnsupportedPathV1::OpaqueInstruction,
            "opaque privacy wire id must fail closed",
        );
    }

    fn assert_canonical_privacy_intent_kat(
        payload: &TransactionPayload,
        expected: PrivacyTransactionIntentDigestV1,
    ) {
        let mut normalized = payload.clone();
        normalized.instructions =
            normalize_privacy_executable_for_intent_v1(&normalized.instructions)
                .expect("canonical normalized executable");
        let normalized_bytes = norito::to_bytes(&normalized).expect("canonical normalized payload");
        assert_eq!(
            normalized_bytes.len(),
            14_187,
            "the canonical fixture wire length is part of the cross-SDK KAT"
        );
        assert_eq!(
            hex::encode(expected.as_bytes()),
            "76fe315dd9a739d4a9b18f92959a258bbcaa2f420997972680416f7edb123552",
            "canonical privacy transaction-intent V1 digest"
        );
    }

    fn assert_privacy_proof_bytes_are_projected_out(
        payload: &TransactionPayload,
        expected: PrivacyTransactionIntentDigestV1,
    ) {
        let mut changed_proof = payload.clone();
        mutate_direct_privacy_submission(&mut changed_proof, |submission| {
            submission.envelope.proof.bytes_mut().bytes = vec![9, 8, 7, 6, 5];
        });
        assert_eq!(
            changed_proof
                .privacy_transaction_intent_digest_v1()
                .expect("proof bytes are projected out"),
            expected
        );
        changed_proof
            .validate_privacy_transaction_intent_binding_v1()
            .expect("proof bytes do not alter either derived digest");
    }

    fn assert_stored_privacy_digests_are_checked(
        payload: &TransactionPayload,
        expected: PrivacyTransactionIntentDigestV1,
    ) {
        let mut stale_intent = payload.clone();
        mutate_direct_privacy_submission(&mut stale_intent, |submission| {
            submission
                .envelope
                .statement
                .context_mut()
                .transaction_intent_digest =
                PrivacyTransactionIntentDigestV1::new(privacy_test_bytes(0xD1));
        });
        assert_eq!(
            stale_intent
                .privacy_transaction_intent_digest_v1()
                .expect("the derived intent field is projected out"),
            expected
        );
        assert!(matches!(
            stale_intent
                .validate_privacy_transaction_intent_binding_v1()
                .expect_err("stored intent is independently checked"),
            PrivacyTransactionIntentErrorV1::IntentDigestMismatch { .. }
        ));
        let mut zero_intent = payload.clone();
        mutate_direct_privacy_submission(&mut zero_intent, |submission| {
            submission
                .envelope
                .statement
                .context_mut()
                .transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
        });
        assert_eq!(
            zero_intent
                .validate_privacy_transaction_intent_binding_v1()
                .expect_err("zero stored intent"),
            PrivacyTransactionIntentErrorV1::ZeroIntentDigest
        );

        let mut stale_statement = payload.clone();
        mutate_direct_privacy_submission(&mut stale_statement, |submission| {
            submission.envelope.statement_digest =
                PrivacyStatementDigestV1::new(privacy_test_bytes(0xD2));
        });
        assert_eq!(
            stale_statement
                .privacy_transaction_intent_digest_v1()
                .expect("the derived statement digest is projected out"),
            expected
        );
        assert!(matches!(
            stale_statement
                .validate_privacy_transaction_intent_binding_v1()
                .expect_err("stored statement digest is independently checked"),
            PrivacyTransactionIntentErrorV1::StatementDigestMismatch { .. }
        ));
        let mut zero_statement = payload.clone();
        mutate_direct_privacy_submission(&mut zero_statement, |submission| {
            submission.envelope.statement_digest = PrivacyStatementDigestV1::new([0; 32]);
        });
        assert_eq!(
            zero_statement
                .validate_privacy_transaction_intent_binding_v1()
                .expect_err("zero stored statement digest"),
            PrivacyTransactionIntentErrorV1::ZeroStatementDigest
        );
    }

    fn assert_legacy_privacy_digest_cycle_is_broken(expected: PrivacyTransactionIntentDigestV1) {
        let draft = draft_privacy_payload();
        let first_legacy = legacy_proof_only_privacy_intent_digest(&draft);
        let mut inserted = draft;
        mutate_direct_privacy_submission(&mut inserted, |submission| {
            submission
                .envelope
                .statement
                .context_mut()
                .transaction_intent_digest = first_legacy;
            submission.envelope.statement_digest = submission
                .envelope
                .statement
                .digest()
                .expect("legacy-cycle statement digest");
        });
        let second_legacy = legacy_proof_only_privacy_intent_digest(&inserted);
        assert_ne!(
            first_legacy, second_legacy,
            "the old proof-only projection changes after inserting its own result and cannot construct the stored value"
        );
        assert_eq!(
            inserted
                .privacy_transaction_intent_digest_v1()
                .expect("canonical projection removes both derived fields"),
            expected
        );
    }

    #[test]
    fn transaction_payload_exposes_execution_identity_ttl_and_chain() {
        let chain: ChainId = "payload-accessors".parse().expect("chain id");
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key");
        let authority = AccountId::new(public_key);
        let instructions: Executable = vec![InstructionBox::from(Log::new(
            Level::INFO,
            "payload".into(),
        ))]
        .into();
        let time_to_live = Duration::from_secs(42);
        let mut builder = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(instructions.clone());
        builder.set_ttl(time_to_live);

        let payload = builder.payload();
        assert_eq!(payload.instructions(), &instructions);
        assert_eq!(payload.authority(), &authority);
        assert_eq!(payload.time_to_live(), Some(time_to_live));
        assert_eq!(payload.chain(), &chain);
    }

    #[test]
    fn privacy_transaction_intent_requires_one_direct_typed_submission() {
        let ordinary = privacy_payload_with_executable(
            vec![InstructionBox::from(Log::new(
                Level::INFO,
                "ordinary".into(),
            ))]
            .into(),
        );
        assert_eq!(
            ordinary
                .privacy_transaction_intent_digest_v1()
                .expect_err("zero submissions"),
            PrivacyTransactionIntentErrorV1::MissingSubmission
        );
        assert!(
            ordinary
                .privacy_transaction_intent_binding_if_present_v1()
                .expect("ordinary payload is not a privacy transaction")
                .is_none()
        );

        let duplicate = draft_privacy_submission();
        let duplicate_payload = privacy_payload_with_executable(
            vec![
                InstructionBox::from(duplicate.clone()),
                InstructionBox::from(duplicate),
            ]
            .into(),
        );
        assert_eq!(
            duplicate_payload
                .privacy_transaction_intent_digest_v1()
                .expect_err("two direct submissions"),
            PrivacyTransactionIntentErrorV1::MultipleSubmissions { count: 2 }
        );
        assert_eq!(
            duplicate_payload
                .privacy_transaction_intent_binding_if_present_v1()
                .expect_err("runtime must reject multiple direct submissions"),
            PrivacyTransactionIntentErrorV1::MultipleSubmissions { count: 2 }
        );
    }

    #[test]
    fn privacy_transaction_intent_rejects_dynamic_and_opaque_paths() {
        assert_privacy_ivm_paths_rejected();
        assert_privacy_dynamic_dispatch_paths_rejected();
        assert_opaque_privacy_instruction_rejected();
    }

    #[test]
    fn privacy_transaction_intent_projection_breaks_the_derived_digest_cycle_exactly() {
        let payload = finalized_privacy_payload();
        let canonical_projection = payload
            .privacy_transaction_intent_projection_bytes_v1()
            .expect("canonical finalized projection bytes");
        let expected = payload
            .privacy_transaction_intent_digest_v1()
            .expect("canonical finalized projection");
        {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                payload
                    .privacy_transaction_intent_projection_bytes_v1()
                    .expect("ambient layout flags cannot alter the canonical projection"),
                canonical_projection
            );
            assert_eq!(
                payload
                    .privacy_transaction_intent_digest_v1()
                    .expect("ambient layout flags cannot alter the canonical intent"),
                expected
            );
        }
        assert_canonical_privacy_intent_kat(&payload, expected);
        assert_privacy_proof_bytes_are_projected_out(&payload, expected);
        assert_stored_privacy_digests_are_checked(&payload, expected);
        assert_legacy_privacy_digest_cycle_is_broken(expected);
    }

    #[test]
    fn zk_ace_intent_projection_zeroes_the_derived_nullifier_and_binds_action_fields() {
        let payload = draft_zk_ace_privacy_payload();
        let expected = payload
            .privacy_transaction_intent_digest_v1()
            .expect("derive ZK-ACE draft intent");

        let mut changed_nullifier = payload.clone();
        mutate_direct_privacy_submission(&mut changed_nullifier, |submission| {
            let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) =
                &mut submission.envelope.statement
            else {
                panic!("ZK-ACE fixture statement");
            };
            statement.replay_nullifier = PrivacyNullifierV1::new(privacy_test_bytes(0x75));
        });
        assert_eq!(
            changed_nullifier
                .privacy_transaction_intent_digest_v1()
                .expect("derived replay nullifier is projected out"),
            expected
        );

        let mut changed_amount = payload.clone();
        mutate_direct_privacy_submission(&mut changed_amount, |submission| {
            let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) =
                &mut submission.envelope.statement
            else {
                panic!("ZK-ACE fixture statement");
            };
            statement.amount += 1;
        });
        assert_ne!(
            changed_amount
                .privacy_transaction_intent_digest_v1()
                .expect("independent action amount remains bound"),
            expected
        );

        let mut finalized = payload;
        mutate_direct_privacy_submission(&mut finalized, |submission| {
            submission
                .envelope
                .statement
                .context_mut()
                .transaction_intent_digest = expected;
            submission.envelope.statement_digest = submission
                .envelope
                .statement
                .digest()
                .expect("final ZK-ACE statement digest");
        });
        assert_eq!(
            finalized
                .validate_privacy_transaction_intent_binding_v1()
                .expect("final ZK-ACE intent binding"),
            expected
        );
    }

    #[test]
    fn vega_intent_projection_zeroes_only_the_derived_hdev_and_breaks_its_cycle() {
        let payload = draft_vega_privacy_payload();
        let expected = payload
            .privacy_transaction_intent_digest_v1()
            .expect("derive Vega draft intent");
        assert_eq!(
            hex::encode(expected.as_bytes()),
            "88a32ad2633e7740cdc680972dc738ce8d94a60f774cc4e8a4286f5a99f4fc66",
            "canonical Vega two-phase transaction-intent projection KAT"
        );

        let mut changed_hdev = payload.clone();
        mutate_direct_privacy_submission(&mut changed_hdev, |submission| {
            let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
                &mut submission.envelope.statement
            else {
                panic!("Vega fixture statement");
            };
            statement.device_authentication_digest =
                PrivacyVegaDeviceAuthenticationDigestV1::new(privacy_test_bytes(0x86));
        });
        assert_eq!(
            changed_hdev
                .privacy_transaction_intent_digest_v1()
                .expect("derived H_dev is projected out"),
            expected
        );

        let independent_mutations: [fn(&mut VegaExistingCredentialStatementV1); 3] = [
            |statement: &mut VegaExistingCredentialStatementV1| {
                statement.reader_challenge.0[0] ^= 1;
            },
            |statement: &mut VegaExistingCredentialStatementV1| {
                statement.issuer_record_digest.0[0] ^= 1;
            },
            |statement: &mut VegaExistingCredentialStatementV1| {
                statement.presentation_date.day += 1;
            },
        ];
        for mutate in independent_mutations {
            let mut changed = payload.clone();
            mutate_direct_privacy_submission(&mut changed, |submission| {
                let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
                    &mut submission.envelope.statement
                else {
                    panic!("Vega fixture statement");
                };
                mutate(statement);
            });
            assert_ne!(
                changed
                    .privacy_transaction_intent_digest_v1()
                    .expect("independent Vega statement field remains bound"),
                expected
            );
        }

        let mut finalized = payload;
        mutate_direct_privacy_submission(&mut finalized, |submission| {
            let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
                &mut submission.envelope.statement
            else {
                panic!("Vega fixture statement");
            };
            statement.context.transaction_intent_digest = expected;
            statement.device_authentication_digest =
                PrivacyVegaDeviceAuthenticationDigestV1::new(privacy_test_bytes(0x87));
            submission.envelope.statement_digest = submission
                .envelope
                .statement
                .digest()
                .expect("final Vega statement digest");
        });
        assert_eq!(
            finalized
                .validate_privacy_transaction_intent_binding_v1()
                .expect("final intent-bound Vega payload"),
            expected
        );
    }

    #[test]
    fn ivm_private_note_intent_projection_breaks_the_action_digest_fixed_point() {
        let payload = draft_ivm_private_note_privacy_payload();
        let expected = payload
            .privacy_transaction_intent_digest_v1()
            .expect("derive IVM private-note draft intent");

        let mut changed_action_digest = payload.clone();
        mutate_direct_privacy_submission(&mut changed_action_digest, |submission| {
            let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) =
                &mut submission.envelope.statement
            else {
                panic!("IVM private-note fixture statement");
            };
            statement.action_digest = PrivacyActionDigestV1::new(privacy_test_bytes(0x96));
        });
        assert_eq!(
            changed_action_digest
                .privacy_transaction_intent_digest_v1()
                .expect("derived IVM action digest is projected out"),
            expected
        );

        let independent_mutations: [fn(&mut IrohaIvmPrivateNoteStarkStatementV1); 3] = [
            |statement: &mut IrohaIvmPrivateNoteStarkStatementV1| {
                statement.state_root.0[0] ^= 1;
            },
            |statement: &mut IrohaIvmPrivateNoteStarkStatementV1| {
                statement.execution_epoch += 1;
            },
            |statement: &mut IrohaIvmPrivateNoteStarkStatementV1| {
                statement.output_commitments[0].0[0] ^= 1;
            },
        ];
        for mutate in independent_mutations {
            let mut changed = payload.clone();
            mutate_direct_privacy_submission(&mut changed, |submission| {
                let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) =
                    &mut submission.envelope.statement
                else {
                    panic!("IVM private-note fixture statement");
                };
                mutate(statement);
            });
            assert_ne!(
                changed
                    .privacy_transaction_intent_digest_v1()
                    .expect("independent IVM statement field remains bound"),
                expected
            );
        }

        let mut finalized = payload;
        mutate_direct_privacy_submission(&mut finalized, |submission| {
            let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) =
                &mut submission.envelope.statement
            else {
                panic!("IVM private-note fixture statement");
            };
            statement.context.transaction_intent_digest = expected;
            statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
            statement.action_digest = statement
                .computed_action_digest()
                .expect("intent-bound IVM action digest");
            assert!(!statement.action_digest.is_zero());
            assert_eq!(
                statement
                    .computed_action_digest()
                    .expect("stable IVM action digest"),
                statement.action_digest,
                "canonical two-phase construction reaches a stable action digest"
            );
            submission.envelope.statement_digest = submission
                .envelope
                .statement
                .digest()
                .expect("final IVM statement digest");
        });
        assert_eq!(
            finalized
                .validate_privacy_transaction_intent_binding_v1()
                .expect("final IVM intent binding"),
            expected
        );

        let mut stale_action_digest = finalized;
        mutate_direct_privacy_submission(&mut stale_action_digest, |submission| {
            let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) =
                &mut submission.envelope.statement
            else {
                panic!("IVM private-note fixture statement");
            };
            statement.action_digest.0[0] ^= 1;
            assert_ne!(
                statement
                    .computed_action_digest()
                    .expect("recompute adversarial IVM action digest"),
                statement.action_digest,
                "an independently drifted action digest cannot authenticate its statement"
            );
        });
    }

    #[test]
    #[ignore = "operator-only KAT regeneration after an intentional intent projection change"]
    fn print_vega_intent_projection_kat() {
        let digest = draft_vega_privacy_payload()
            .privacy_transaction_intent_digest_v1()
            .expect("Vega intent projection");
        eprintln!(
            "VEGA_TRANSACTION_INTENT_PROJECTION_KAT_V1={}",
            hex::encode(digest.as_bytes())
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn privacy_transaction_intent_binds_every_independent_payload_field() {
        let payload = finalized_privacy_payload();
        let expected = payload
            .privacy_transaction_intent_digest_v1()
            .expect("base intent");

        macro_rules! assert_bound {
            ($name:literal, $mutate:expr) => {{
                let mut changed = payload.clone();
                ($mutate)(&mut changed);
                assert_ne!(
                    changed
                        .privacy_transaction_intent_digest_v1()
                        .expect(concat!("derive changed ", $name)),
                    expected,
                    "{} must remain in the intent projection",
                    $name
                );
            }};
        }

        assert_bound!("payload chain", |changed: &mut TransactionPayload| {
            changed.chain = ChainId::from("privacy-intent-other-chain");
        });
        assert_bound!("payload authority", |changed: &mut TransactionPayload| {
            let key: iroha_crypto::PublicKey =
                "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                    .parse()
                    .expect("alternate key");
            changed.authority = AccountId::new(key);
        });
        assert_bound!("creation time", |changed: &mut TransactionPayload| {
            changed.creation_time_ms += 1;
        });
        assert_bound!("time to live", |changed: &mut TransactionPayload| {
            changed.time_to_live_ms = NonZeroU64::new(10);
        });
        assert_bound!("nonce", |changed: &mut TransactionPayload| {
            changed.nonce = NonZeroU32::new(7);
        });
        assert_bound!("fee intent", |changed: &mut TransactionPayload| {
            changed.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(11));
        });
        assert_bound!("metadata", |changed: &mut TransactionPayload| {
            changed.metadata.insert(
                Name::from_str("privacy_mutation").expect("metadata name"),
                Json::new(1_u32),
            );
        });
        assert_bound!("instruction ordinal", |changed: &mut TransactionPayload| {
            let Executable::Instructions(instructions) = &changed.instructions else {
                unreachable!()
            };
            let mut instructions = instructions.clone().into_vec();
            instructions.insert(0, Log::new(Level::INFO, "before privacy".into()).into());
            changed.instructions = Executable::Instructions(instructions.into());
        });

        macro_rules! assert_submission_bound {
            ($name:literal, $mutate:expr) => {
                assert_bound!($name, |changed: &mut TransactionPayload| {
                    mutate_direct_privacy_submission(changed, $mutate);
                });
            };
        }

        assert_submission_bound!("protocol tag", |submission: &mut SubmitPrivacyProofV1| {
            submission.envelope.protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
        });
        assert_submission_bound!(
            "proof-system tag",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.proof_system_id =
                    PrivacyProofSystemIdV1::StarkFriSha256Goldilocks;
            }
        );
        assert_submission_bound!("engine tag", |submission: &mut SubmitPrivacyProofV1| {
            submission.envelope.engine_id =
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0.expected_engine();
        });
        assert_submission_bound!(
            "envelope parameter id",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.parameter_id =
                    PrivacyParameterIdV1::new(privacy_test_bytes(0x21));
            }
        );
        assert_submission_bound!(
            "envelope parameter digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.parameter_digest =
                    PrivacyParameterDigestV1::new(privacy_test_bytes(0x22));
            }
        );
        assert_submission_bound!(
            "envelope verifier digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.verifier_digest =
                    PrivacyVerifierDigestV1::new(privacy_test_bytes(0x23));
            }
        );
        assert_submission_bound!(
            "envelope schema digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.statement_schema_digest =
                    PrivacyStatementSchemaDigestV1::new(privacy_test_bytes(0x24));
            }
        );
        assert_submission_bound!(
            "envelope engine-manifest digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.engine_manifest_digest =
                    PrivacyEngineManifestDigestV1::new(privacy_test_bytes(0x25));
            }
        );
        assert_submission_bound!(
            "proof variant tag",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.proof =
                    PrivacyProofV1::ZkAcePqAuthorizationV0(PrivacyProofBytesV1::new(vec![0xA5]));
            }
        );
        assert_submission_bound!("context chain", |submission: &mut SubmitPrivacyProofV1| {
            submission.envelope.statement.context_mut().chain_id =
                ChainId::from("privacy-context-other-chain");
        });
        assert_submission_bound!("action index", |submission: &mut SubmitPrivacyProofV1| {
            submission.envelope.statement.context_mut().action_index = 1;
        });
        assert_submission_bound!(
            "context parameter id",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.statement.context_mut().parameter_id =
                    PrivacyParameterIdV1::new(privacy_test_bytes(0x31));
            }
        );
        assert_submission_bound!(
            "context parameter digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.statement.context_mut().parameter_digest =
                    PrivacyParameterDigestV1::new(privacy_test_bytes(0x32));
            }
        );
        assert_submission_bound!(
            "context verifier digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission.envelope.statement.context_mut().verifier_digest =
                    PrivacyVerifierDigestV1::new(privacy_test_bytes(0x33));
            }
        );
        assert_submission_bound!(
            "context schema digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission
                    .envelope
                    .statement
                    .context_mut()
                    .statement_schema_digest =
                    PrivacyStatementSchemaDigestV1::new(privacy_test_bytes(0x34));
            }
        );
        assert_submission_bound!(
            "context engine-manifest digest",
            |submission: &mut SubmitPrivacyProofV1| {
                submission
                    .envelope
                    .statement
                    .context_mut()
                    .engine_manifest_digest =
                    PrivacyEngineManifestDigestV1::new(privacy_test_bytes(0x35));
            }
        );
        assert_submission_bound!(
            "statement polynomial commitment",
            |submission: &mut SubmitPrivacyProofV1| {
                let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) =
                    &mut submission.envelope.statement
                else {
                    unreachable!()
                };
                statement.polynomial_commitments[0].encoding[0] ^= 1;
            }
        );
        assert_submission_bound!(
            "statement query point",
            |submission: &mut SubmitPrivacyProofV1| {
                let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) =
                    &mut submission.envelope.statement
                else {
                    unreachable!()
                };
                statement.evaluation_point.encoding[0] ^= 1;
            }
        );
        assert_submission_bound!(
            "statement claimed evaluation",
            |submission: &mut SubmitPrivacyProofV1| {
                let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) =
                    &mut submission.envelope.statement
                else {
                    unreachable!()
                };
                statement.claimed_evaluations[0].encoding[0] ^= 1;
            }
        );
    }

    #[test]
    fn privacy_intent_is_independent_of_the_complete_signed_transaction_hash() {
        let payload = finalized_privacy_payload();
        let transaction = TransactionBuilder::from_payload(payload)
            .expect("valid final payload")
            .sign(&privacy_test_private_key());
        let expected_intent = transaction
            .privacy_transaction_intent_digest_v1()
            .expect("signed transaction intent");

        let mut altered_signature = transaction.clone();
        altered_signature.signature = sample_signed_transaction().signature().clone();
        assert_eq!(
            altered_signature.hash(),
            transaction.hash(),
            "transaction identity must exclude replaceable authorization proof"
        );
        assert_eq!(
            altered_signature
                .privacy_transaction_intent_digest_v1()
                .expect("intent depends only on unsigned payload"),
            expected_intent
        );
    }

    #[test]
    fn fee_payment_intent_requires_canonical_positive_component_limits() {
        let asset = sample_fee_asset();
        let nexus =
            FeeChargeLimit::new(FeeChargeKind::Nexus, asset.clone(), Quantity::from(10_u32));
        let pipeline = FeeChargeLimit::new(
            FeeChargeKind::PipelineGas,
            asset.clone(),
            Quantity::from(20_u32),
        );

        FeePaymentIntent::authority(vec![nexus.clone(), pipeline.clone()], None)
            .validate()
            .expect("ordered positive fee limits are valid");

        let err = FeePaymentIntent::authority(vec![pipeline, nexus.clone()], None)
            .validate()
            .expect_err("reversed component order must fail");
        assert_eq!(err, FeePaymentIntentError::NonCanonicalChargeLimitOrder);

        let err = FeePaymentIntent::authority(vec![nexus.clone(), nexus], None)
            .validate()
            .expect_err("duplicate component must fail");
        assert_eq!(
            err,
            FeePaymentIntentError::DuplicateChargeKind(FeeChargeKind::Nexus)
        );

        let err = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                asset.clone(),
                Quantity::zero(),
            )],
            None,
        )
        .validate()
        .expect_err("zero maximum must fail");
        assert_eq!(
            err,
            FeePaymentIntentError::ZeroChargeLimit {
                kind: FeeChargeKind::Nexus,
                asset_definition_id: asset,
            }
        );
    }

    #[test]
    fn fee_quote_selection_comparison_binds_payer_revision_and_gas() {
        let authority = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(100));
        assert!(
            authority.has_same_payer_and_gas_bound(&FeePaymentIntent::authority(
                vec![FeeChargeLimit::new(
                    FeeChargeKind::Nexus,
                    sample_fee_asset(),
                    Quantity::from(1_u32),
                )],
                NonZeroU64::new(100),
            ))
        );
        assert!(
            !authority.has_same_payer_and_gas_bound(&FeePaymentIntent::authority(
                Vec::new(),
                NonZeroU64::new(101),
            ))
        );

        let sponsor = sample_signed_transaction().authority().clone();
        let first = FeePaymentIntent::sponsor(
            FeeSponsorProgramId::new(sponsor.clone(), "wallet".parse().expect("program name")),
            1,
            Vec::new(),
            None,
        );
        let same = FeePaymentIntent::sponsor(
            FeeSponsorProgramId::new(sponsor.clone(), "wallet".parse().expect("program name")),
            1,
            Vec::new(),
            None,
        );
        let other_revision = FeePaymentIntent::sponsor(
            FeeSponsorProgramId::new(sponsor, "wallet".parse().expect("program name")),
            2,
            Vec::new(),
            None,
        );
        assert!(first.has_same_payer_and_gas_bound(&same));
        assert!(!first.has_same_payer_and_gas_bound(&other_revision));
        assert!(!first.has_same_payer_and_gas_bound(&authority));
    }

    #[test]
    fn legacy_fee_metadata_is_rejected_before_signing() {
        let mut metadata = Metadata::default();
        metadata.insert(
            "fee_sponsor".parse().expect("valid metadata key"),
            Json::new("legacy".to_owned()),
        );
        let err = FeePaymentIntent::validate_metadata(&metadata)
            .expect_err("legacy fee metadata must fail closed");
        assert_eq!(
            err,
            FeePaymentIntentError::LegacyMetadataKey("fee_sponsor".to_owned())
        );
    }

    #[test]
    fn transaction_payload_validates_typed_and_legacy_fee_invariants_together() {
        let mut payload = sample_signed_transaction().payload().clone();
        payload.fee_payment = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                sample_fee_asset(),
                Quantity::zero(),
            )],
            None,
        );
        assert!(matches!(
            payload.validate_fee_payment_intent(),
            Err(FeePaymentIntentError::ZeroChargeLimit { .. })
        ));

        payload.fee_payment = FeePaymentIntent::authority(Vec::new(), None);
        payload.metadata.insert(
            "gas_limit".parse().expect("valid metadata key"),
            Json::new(1_u64),
        );
        assert_eq!(
            payload
                .validate_fee_payment_intent()
                .expect_err("retired metadata must fail the combined validation"),
            FeePaymentIntentError::LegacyMetadataKey("gas_limit".to_owned())
        );
    }

    #[test]
    fn signed_transaction_exposes_signature_bound_fee_intent() {
        let transaction = sample_signed_transaction();
        assert_eq!(
            transaction.fee_payment_intent(),
            &FeePaymentIntent::authority(Vec::new(), None)
        );
        transaction
            .verify_signature()
            .expect("the signed fee intent must verify with the payload");
    }

    #[test]
    fn signed_contract_invocation_arguments_and_code_hash_are_signature_bound() {
        let chain: ChainId = "test-chain".parse().expect("chain id");
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key");
        let authority = AccountId::new(public_key);
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .expect("private key");
        let contract_address = crate::smart_contract::ContractAddress::derive(
            0,
            &authority,
            0,
            crate::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let arguments = crate::transaction::executable::ContractArgumentRecord::try_new(vec![
            0x4b, 0x4f, 0x54, 0x4f,
        ])
        .expect("bounded argument record");
        let mut transaction = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::ContractCall(
            crate::transaction::executable::ContractInvocation {
                contract_address,
                expected_code_hash: iroha_crypto::Hash::new(b"signed-contract-code"),
                entrypoint: "call".to_owned(),
                arguments: Some(arguments),
            },
        ))
        .sign(&private_key);
        let signed_hash = transaction.hash();
        transaction
            .verify_signature()
            .expect("original signature verifies");

        let Executable::ContractCall(invocation) = &mut transaction.payload.instructions else {
            panic!("contract call executable");
        };
        invocation
            .arguments
            .as_mut()
            .expect("argument record")
            .as_mut_bytes()[0] ^= 0x01;

        assert_ne!(transaction.hash(), signed_hash);
        transaction
            .verify_signature()
            .expect_err("mutating signed arguments must invalidate the signature");

        let Executable::ContractCall(invocation) = &mut transaction.payload.instructions else {
            panic!("contract call executable");
        };
        invocation
            .arguments
            .as_mut()
            .expect("argument record")
            .as_mut_bytes()[0] ^= 0x01;
        transaction
            .verify_signature()
            .expect("restoring signed arguments restores the original signature");
        let Executable::ContractCall(invocation) = &mut transaction.payload.instructions else {
            panic!("contract call executable");
        };
        invocation.expected_code_hash = iroha_crypto::Hash::new(b"rebound-contract-code");
        assert_ne!(transaction.hash(), signed_hash);
        transaction
            .verify_signature()
            .expect_err("mutating the expected code hash must invalidate the signature");
    }

    #[test]
    fn verify_proof_instruction_signed_tx_versioned_roundtrip() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(public_key);
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let proof_bytes = b"open-verify-envelope".to_vec();
        let mut attachment = crate::proof::ProofAttachment::new_ref(
            "halo2/ipa".into(),
            crate::proof::ProofBox::new("halo2/ipa".into(), proof_bytes.clone()),
            crate::proof::VerifyingKeyId::new("halo2/ipa", "component_verify_v1"),
        );
        attachment.envelope_hash = Some(iroha_crypto::Hash::new(&proof_bytes).into());
        let instruction: InstructionBox = crate::isi::zk::VerifyProof::new(attachment).into();

        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(&private_key);
        let bytes = tx.encode_versioned();
        let decoded = SignedTransaction::decode_all_versioned(&bytes)
            .expect("versioned VerifyProof transaction must decode");

        assert_eq!(decoded.hash(), tx.hash());
        decoded
            .verify_signature()
            .expect("decoded VerifyProof transaction signature must verify");
    }

    fn checked_transaction_payload_signature(
        private_key: &iroha_crypto::PrivateKey,
        payload: &model::TransactionPayload,
    ) -> SignatureOf<model::TransactionPayload> {
        SignatureOf::try_new(private_key, payload).expect("checked transaction fixture signature")
    }

    fn checked_random_keypair() -> iroha_crypto::KeyPair {
        iroha_crypto::KeyPair::try_random()
            .expect("test fixture random key generation should succeed")
    }

    fn checked_random_keypair_with_algorithm(algorithm: Algorithm) -> iroha_crypto::KeyPair {
        iroha_crypto::KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{algorithm:?} transaction fixture key generation should succeed: {err}")
        })
    }

    const SMALL_ORDER_ED25519_SIGNATURE_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const NONCANONICAL_ED25519_SIGNATURE_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn signature_of_with_malformed_ed25519_r<T>(
        signature: &SignatureOf<T>,
        replacement_r: &[u8; 32],
    ) -> SignatureOf<T> {
        let mut payload = signature.payload().to_vec();
        payload[..replacement_r.len()].copy_from_slice(replacement_r);
        SignatureOf::from_signature(iroha_crypto::Signature::from_bytes(&payload))
    }

    #[test]
    fn with_instructions_accepts_instruction_box() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();

        // Pre-boxed instruction
        let instruction: InstructionBox = Register::domain(Domain::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
        ))
        .into();
        let expected_id = crate::isi::Instruction::id(&*instruction);

        // Use a known matching keypair (values from project samples)
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let key_pair = iroha_crypto::KeyPair::new(public_key.clone(), private_key).unwrap();

        let authority = AccountId::new(public_key.clone());

        let tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(core::iter::once(instruction))
        .with_metadata(Metadata::default())
        .sign(key_pair.private_key());

        assert_eq!(
            tx.authority().expect_single_signatory(),
            key_pair.public_key()
        );

        if let Executable::Instructions(v) = tx.instructions() {
            assert_eq!(v.len(), 1);
            // Ensure the inner instruction wasn't double-boxed by verifying its type id.
            let instruction_id = crate::isi::Instruction::id(&*v[0]);
            assert_eq!(instruction_id, expected_id);
            assert_ne!(instruction_id, "iroha_data_model::isi::InstructionBox");
        } else {
            panic!("expected Instructions variant");
        }
    }

    #[test]
    fn with_executable_batch_preserves_mixed_item_order() {
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let invocation = crate::transaction::executable::ContractInvocation {
            contract_address: "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8"
                .parse()
                .expect("contract address"),
            expected_code_hash: Hash::new(b"builder-batch-contract"),
            entrypoint: "run".to_owned(),
            arguments: None,
        };
        let items = vec![
            crate::transaction::ExecutableBatchItem::Instruction(
                Log::new(Level::INFO, "before".into()).into(),
            ),
            crate::transaction::ExecutableBatchItem::ContractCall(invocation),
            crate::transaction::ExecutableBatchItem::Instruction(
                Log::new(Level::INFO, "after".into()).into(),
            ),
        ];
        let tx = TransactionBuilder::new(
            "test-chain".parse().expect("chain id"),
            authority,
            FeePaymentIntent::authority(
                Vec::new(),
                Some(NonZeroU64::new(100_000).expect("nonzero gas limit")),
            ),
        )
        .with_executable_batch(items)
        .sign(key_pair.private_key());

        let Executable::Batch(items) = tx.instructions() else {
            panic!("expected mixed executable batch");
        };
        assert!(matches!(
            items[0],
            crate::transaction::ExecutableBatchItem::Instruction(_)
        ));
        assert!(matches!(
            items[1],
            crate::transaction::ExecutableBatchItem::ContractCall(_)
        ));
        assert!(matches!(
            items[2],
            crate::transaction::ExecutableBatchItem::Instruction(_)
        ));
    }

    #[test]
    fn transaction_builder_exports_signable_payload_and_accepts_external_signature() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let builder = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "external signature".into())]);

        let payload_bytes = builder.encode_payload();
        let payload_hash = builder.payload_hash();
        assert_eq!(payload_hash, Hash::new(&payload_bytes));

        let payload_hash_bytes = builder.payload_hash_bytes();
        let signature = Signature::try_new(key_pair.private_key(), &payload_hash_bytes)
            .expect("checked external transaction fixture signature");
        signature
            .verify(key_pair.public_key(), &payload_hash_bytes)
            .expect("checked external transaction fixture signature verifies prehash");
        let signed = builder.build_with_signature(signature);
        assert!(signed.verify_signature().is_ok());
    }

    #[test]
    fn transaction_builder_decodes_exact_external_signing_payload() {
        let chain: ChainId = "external-payload-roundtrip".parse().unwrap();
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let mut builder = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "decode payload".into())]);
        builder.set_creation_time(Duration::from_millis(42));
        builder.set_nonce(NonZeroU32::new(7).unwrap());

        let encoded = builder.encode_payload();
        let decoded = TransactionBuilder::decode_payload(&encoded).unwrap();
        assert_eq!(decoded.encode_payload(), encoded);
        assert_eq!(decoded.payload_hash_bytes(), builder.payload_hash_bytes());

        let mut with_trailing = encoded;
        with_trailing.push(0);
        assert!(TransactionBuilder::decode_payload(&with_trailing).is_err());
        assert!(TransactionBuilder::decode_payload(&[]).is_err());

        let canonical = builder.encode_payload();
        assert!(
            canonical[0] < 0x80,
            "fixture starts with a compact field length"
        );
        let mut overlong = Vec::with_capacity(canonical.len() + 1);
        overlong.push(canonical[0] | 0x80);
        overlong.push(0);
        overlong.extend_from_slice(&canonical[1..]);
        assert!(TransactionBuilder::decode_payload(&overlong).is_err());
    }

    #[test]
    fn transaction_builder_payload_roundtrip_preserves_quote_to_sign_preimage() {
        let chain: ChainId = "quote-sign-payload".parse().unwrap();
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let intent = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                sample_fee_asset(),
                Quantity::from(10_u32),
            )],
            None,
        );
        let mut builder = TransactionBuilder::new(chain, authority, intent)
            .with_instructions([Log::new(Level::INFO, "quote then sign".into())]);
        builder.set_creation_time(Duration::from_millis(42));

        let payload = builder.into_payload().expect("valid unsigned payload");
        let expected = norito::codec::encode_adaptive(&payload);
        let rebuilt = TransactionBuilder::from_payload(payload.clone())
            .expect("quoted payload reconstructs a builder");
        assert_eq!(rebuilt.encode_payload(), expected);

        let signed = rebuilt
            .try_sign(key_pair.private_key())
            .expect("exact quoted payload signs");
        assert_eq!(signed.payload(), &payload);
        signed.verify_signature().expect("signature verifies");
    }

    #[test]
    fn transaction_builder_from_payload_rejects_retired_fee_metadata() {
        let chain: ChainId = "invalid-quoted-payload".parse().unwrap();
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let mut payload = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .into_payload()
        .expect("default payload is structurally valid");
        payload.metadata.insert(
            "gas_limit".parse().expect("metadata key"),
            Json::new(10_u64),
        );

        let error = TransactionBuilder::from_payload(payload)
            .expect_err("retired fee metadata must fail before signing");
        assert!(matches!(
            error,
            TransactionSignatureError::InvalidFeePaymentIntent(_)
        ));
    }

    #[test]
    fn transaction_builder_try_sign_matches_compatibility_sign() {
        let chain: ChainId = "try-sign-chain".parse().unwrap();
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let make_builder = || {
            let mut builder = TransactionBuilder::new(
                chain.clone(),
                authority.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "fallible tx signing".into())])
            .with_metadata(Metadata::default());
            builder.set_creation_time(Duration::from_millis(1_234));
            builder
        };

        let fallible = make_builder()
            .try_sign(key_pair.private_key())
            .expect("transaction signing should succeed");
        let compatibility = make_builder().sign(key_pair.private_key());

        assert_eq!(
            norito::to_bytes(&fallible).expect("encode fallible signed transaction"),
            norito::to_bytes(&compatibility).expect("encode compatibility signed transaction")
        );
        fallible
            .verify_signature()
            .expect("fallible signed transaction must verify");
    }

    #[test]
    fn transaction_signature_decode_from_slice_roundtrip() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(public_key.clone());
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let signed_tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(&private_key);
        let signature = signed_tx.signature().clone();

        let encoded = norito::to_bytes(&signature).expect("encode signature");
        let decoded: TransactionSignature =
            norito::core::decode_from_bytes(&encoded).expect("decode signature");
        assert_eq!(decoded, signature);

        let inner = signature.0.clone();
        let inner_encoded = norito::to_bytes(&inner).expect("encode inner signature");
        let decoded_inner: iroha_crypto::SignatureOf<TransactionPayload> =
            norito::core::decode_from_bytes(&inner_encoded).expect("decode inner signature");
        assert_eq!(decoded_inner, inner);
    }

    #[test]
    fn transaction_signature_decode_rejects_empty_signature_material() {
        let signature = TransactionSignature(SignatureOf::from_signature(
            iroha_crypto::Signature::from_bytes(&[]),
        ));
        let encoded = norito::to_bytes(&signature).expect("encode invalid transaction signature");

        let err = norito::core::decode_from_bytes::<TransactionSignature>(&encoded)
            .expect_err("empty transaction signature must fail closed");
        let message = err.to_string();
        assert!(
            message.contains("empty") || message.contains("length mismatch"),
            "unexpected transaction signature decode error: {message}"
        );
    }

    #[test]
    fn transaction_signature_decode_rejects_all_zero_signature_material() {
        let signature = TransactionSignature(SignatureOf::from_signature(
            iroha_crypto::Signature::from_bytes(&[0_u8; 64]),
        ));
        let encoded = norito::to_bytes(&signature).expect("encode invalid transaction signature");

        let err = norito::core::decode_from_bytes::<TransactionSignature>(&encoded)
            .expect_err("all-zero transaction signature must fail closed");
        let message = err.to_string();
        assert!(
            message.contains("all zero"),
            "unexpected transaction signature decode error: {message}"
        );
    }

    #[test]
    fn signed_transaction_decode_from_slice_rejects_trailing_bytes() {
        let signed_tx = sample_signed_transaction();
        let mut bytes = norito::codec::encode_adaptive(&signed_tx);
        bytes.push(0);

        let err = SignedTransaction::decode_from_slice(&bytes)
            .expect_err("signed transaction slice decoder must reject trailing bytes");

        assert!(matches!(err, norito::core::Error::LengthMismatch));
    }

    #[test]
    fn execution_step_decode_from_slice_rejects_trailing_bytes() {
        let step = ExecutionStep(ConstVec::from(vec![InstructionBox::from(Log::new(
            Level::INFO,
            "exact execution step".into(),
        ))]));
        let mut bytes = norito::codec::encode_adaptive(&step);
        bytes.push(0);

        let err = ExecutionStep::decode_from_slice(&bytes)
            .expect_err("execution step slice decoder must reject trailing bytes");

        assert!(matches!(err, norito::core::Error::LengthMismatch));
    }

    #[test]
    fn execution_step_decode_from_slice_roundtrips_instruction_vector() {
        let step = ExecutionStep(ConstVec::from(vec![
            InstructionBox::from(Log::new(Level::INFO, "first execution step".into())),
            InstructionBox::from(Log::new(Level::WARN, "second execution step".into())),
        ]));
        let bytes = norito::codec::encode_adaptive(&step);

        let (decoded, used) =
            ExecutionStep::decode_from_slice(&bytes).expect("decode exact execution step");

        assert_eq!(used, bytes.len());
        assert_eq!(decoded, step);
    }

    #[test]
    fn signed_transaction_versioned_decode_rejects_trailing_bytes() {
        let signed_tx = sample_signed_transaction();
        let mut bytes = signed_tx.encode_versioned();
        bytes.push(0);

        let err = SignedTransaction::decode_all_versioned(&bytes)
            .expect_err("versioned signed transaction decoder must reject trailing bytes");

        assert!(matches!(err, iroha_version::error::Error::NoritoCodec(_)));
    }

    #[test]
    fn signed_transaction_versioned_roundtrip() {
        let signed_tx = sample_signed_transaction();
        let bytes = signed_tx.encode_versioned();
        let decoded = SignedTransaction::decode_all_versioned(&bytes)
            .expect("versioned signed transaction must decode");

        assert_eq!(decoded, signed_tx);
    }

    #[test]
    fn signed_transaction_versioned_decode_rejects_empty_payload_without_body_decode() {
        let err = SignedTransaction::decode_all_versioned(&[])
            .expect_err("empty signed transaction payload must be rejected");

        assert!(matches!(err, iroha_version::error::Error::NotVersioned));
        assert!(
            !err.to_string().contains("panic during decode"),
            "empty payloads should not surface as decode panics: {err}"
        );
    }

    #[test]
    fn signed_transaction_versioned_decode_rejects_version_only_payload_without_decode_panic() {
        let err = SignedTransaction::decode_all_versioned(&[1])
            .expect_err("version-only signed transaction payload must be rejected");

        assert!(matches!(err, iroha_version::error::Error::NoritoCodec(_)));
        assert!(
            !err.to_string().contains("panic during decode"),
            "truncated payloads should not surface as decode panics: {err}"
        );
    }

    #[test]
    fn signed_transaction_versioned_decode_rejects_unsupported_version_without_body_decode() {
        let signed_tx = sample_signed_transaction();
        let mut bytes = signed_tx.encode_versioned();
        bytes[0] = 2;

        let err = SignedTransaction::decode_all_versioned(&bytes)
            .expect_err("unsupported signed transaction version must be rejected");

        assert!(matches!(
            err,
            iroha_version::error::Error::UnsupportedVersion(_)
        ));
        assert!(
            !err.to_string().contains("panic during decode"),
            "unsupported versions should not surface as decode panics: {err}"
        );
    }

    #[test]
    fn signed_transaction_decode_rejects_empty_signature_without_decode_panic() {
        let mut invalid_tx = sample_signed_transaction();
        invalid_tx.signature = TransactionSignature(iroha_crypto::SignatureOf::from_signature(
            iroha_crypto::Signature::from_bytes(&[]),
        ));

        let encoded = norito::to_bytes(&invalid_tx).expect("encode invalid transaction fixture");
        let err = norito::core::decode_from_bytes::<SignedTransaction>(&encoded)
            .expect_err("empty signed transaction signature must fail closed");
        let message = err.to_string();
        assert!(
            message.contains("empty") || message.contains("length mismatch"),
            "unexpected signed transaction decode error: {message}"
        );

        let err = SignedTransaction::decode_all_versioned(&invalid_tx.encode_versioned())
            .expect_err("empty signed transaction signature must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("empty") || message.contains("length mismatch"),
            "unexpected versioned signed transaction decode error: {message}"
        );
        assert!(
            !message.contains("panic during decode"),
            "empty signatures should not surface as decode panics: {message}"
        );
    }

    #[test]
    fn signed_transaction_decode_rejects_all_zero_signature_without_decode_panic() {
        let mut invalid_tx = sample_signed_transaction();
        invalid_tx.signature = TransactionSignature(iroha_crypto::SignatureOf::from_signature(
            iroha_crypto::Signature::from_bytes(&[0_u8; 64]),
        ));

        let encoded = norito::to_bytes(&invalid_tx).expect("encode invalid transaction fixture");
        let err = norito::core::decode_from_bytes::<SignedTransaction>(&encoded)
            .expect_err("all-zero signed transaction signature must fail closed");
        let message = err.to_string();
        assert!(
            message.contains("all zero"),
            "unexpected signed transaction decode error: {message}"
        );

        let err = SignedTransaction::decode_all_versioned(&invalid_tx.encode_versioned())
            .expect_err("all-zero signed transaction signature must be rejected");
        let message = err.to_string();
        assert!(
            message.contains("all zero"),
            "unexpected versioned signed transaction decode error: {message}"
        );
        assert!(
            !message.contains("panic during decode"),
            "all-zero signatures should not surface as decode panics: {message}"
        );
    }

    #[test]
    fn signed_transaction_versioned_decode_preserves_invalid_signature_for_validation() {
        let mut invalid_tx = sample_signed_transaction();
        let mut signature = invalid_tx.signature().0.payload().to_vec();
        let last = signature
            .last_mut()
            .expect("test signature payload is non-empty");
        *last ^= 0xFF;
        invalid_tx.signature = TransactionSignature(iroha_crypto::SignatureOf::from_signature(
            iroha_crypto::Signature::try_from_bytes(&signature)
                .expect("tampered transaction signature remains structurally admissible"),
        ));

        let decoded = SignedTransaction::decode_all_versioned(&invalid_tx.encode_versioned())
            .expect("well-formed transaction with invalid signature must still decode");
        let err = decoded
            .verify_signature()
            .expect_err("invalid transaction signature must fail verification");

        assert!(matches!(err, TransactionSignatureError::CryptoError(_)));
    }

    #[test]
    fn signed_transaction_rejects_malformed_ed25519_signature_r() {
        let tx = sample_signed_transaction();

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_SIGNATURE_R),
            ("noncanonical", NONCANONICAL_ED25519_SIGNATURE_R),
        ] {
            let mut invalid_tx = tx.clone();
            invalid_tx.signature = TransactionSignature(signature_of_with_malformed_ed25519_r(
                &tx.signature.0,
                &replacement_r,
            ));

            let err = invalid_tx
                .verify_signature()
                .expect_err("malformed Ed25519 transaction signature R must fail admission");

            assert_eq!(
                err,
                TransactionSignatureError::CryptoError("Signature verification failed".to_owned()),
                "{label} transaction signature R was not rejected"
            );
        }
    }

    #[test]
    fn signed_transaction_rejects_malformed_mldsa_signature_lengths() {
        let key_pair = checked_random_keypair_with_algorithm(Algorithm::MlDsa);
        let chain: ChainId = "mldsa-tx-signature-length".parse().expect("chain id");
        let authority = AccountId::new(key_pair.public_key().clone());
        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "mldsa tx".into())])
        .sign(key_pair.private_key());

        tx.verify_signature()
            .expect("valid ML-DSA transaction signature verifies");
        let valid_signature = tx.signature.0.payload().to_vec();

        for (label, replacement_signature) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x5A);
                payload
            }),
        ] {
            let mut invalid_tx = tx.clone();
            invalid_tx.signature = TransactionSignature(SignatureOf::from_signature(
                Signature::from_bytes(&replacement_signature),
            ));

            let err = invalid_tx
                .verify_signature()
                .expect_err("malformed ML-DSA transaction signature length must fail admission");
            assert!(
                matches!(err, TransactionSignatureError::CryptoError(_)),
                "{label} ML-DSA transaction signature length failed with unexpected error: {err:?}"
            );
        }
    }

    #[test]
    fn transaction_entrypoint_versioned_decode_rejects_trailing_bytes() {
        let entrypoint = TransactionEntrypoint::from(sample_signed_transaction());
        let mut bytes = entrypoint.encode_versioned();
        bytes.push(0);

        let err = TransactionEntrypoint::decode_all_versioned(&bytes)
            .expect_err("versioned transaction entrypoint decoder must reject trailing bytes");

        assert!(matches!(err, iroha_version::error::Error::NoritoCodec(_)));
    }

    #[test]
    fn signed_transaction_roundtrip_preserves_instruction_order() {
        use crate::parameter::{Parameter, system::SumeragiParameter};
        let chain: ChainId = "test-chain".parse().unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(public_key.clone());
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let ordered = vec![
            InstructionBox::from(crate::isi::SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::MaxClockDriftMs(667),
            ))),
            InstructionBox::from(crate::isi::SetParameter::new(Parameter::Transaction(
                crate::parameter::TransactionParameter::RequireHeightTtl(true),
            ))),
            InstructionBox::from(crate::isi::SetParameter::new(Parameter::Transaction(
                crate::parameter::TransactionParameter::RequireSequence(true),
            ))),
            InstructionBox::from(crate::isi::SetParameter::new(Parameter::Block(
                crate::parameter::BlockParameter::MaxTransactions(
                    core::num::NonZeroU64::new(10_000).unwrap(),
                ),
            ))),
        ];

        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(ordered.clone())
        .sign(&private_key);

        let bytes = norito::codec::encode_adaptive(&tx);
        let (decoded, used): (SignedTransaction, usize) =
            SignedTransaction::decode_from_slice(&bytes).expect("decode signed transaction");
        assert_eq!(
            used,
            bytes.len(),
            "signed transaction must consume full buffer"
        );

        let Executable::Instructions(actual) = decoded.instructions() else {
            panic!("expected instruction executable after roundtrip");
        };

        let actual = actual.iter().cloned().collect::<Vec<_>>();
        assert_eq!(
            actual, ordered,
            "instruction order must survive signed transaction roundtrip"
        );
    }

    #[test]
    fn sign_rejects_mismatched_signatory_without_rewriting_payload() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let stored_public_key: iroha_crypto::PublicKey =
            "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                .parse()
                .unwrap();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let key_pair = iroha_crypto::KeyPair::from_private_key(private_key).unwrap();
        let authority = AccountId::new(stored_public_key.clone());

        assert_ne!(authority.expect_single_signatory(), key_pair.public_key());
        let error = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .try_sign(key_pair.private_key())
        .expect_err("signing must preserve and reject a mismatched authority");
        assert_eq!(error, TransactionSignatureError::AuthorityKeyMismatch);
        assert_eq!(authority.expect_single_signatory(), &stored_public_key);
    }

    #[test]
    fn entrypoint_hashes_match_direct_encoding() {
        let chain: ChainId = "hash-chain".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(public_key);
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(&private_key);
        let entry = TransactionEntrypoint::External(tx.clone());
        assert_ne!(
            HashOf::new(&entry),
            entry.hash(),
            "raw envelope hashing must not define external transaction identity"
        );
        assert_eq!(tx.hash_as_entrypoint(), entry.hash());
        assert_eq!(Hash::from(tx.hash()), Hash::from(tx.hash_as_entrypoint()));

        let time_entry = TimeTriggerEntrypoint {
            id: "trigger".parse().unwrap(),
            instructions: ExecutionStep(ConstVec::from(vec![])),
            authority,
        };
        let entry_time = TransactionEntrypoint::Time(time_entry.clone());
        assert_eq!(HashOf::new(&entry_time), entry_time.hash());
        assert_eq!(time_entry.hash_as_entrypoint(), entry_time.hash());
    }

    #[test]
    fn verify_signature_rejects_missing_multisig_signatures() {
        let chain: ChainId = "multisig-chain".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let signer = checked_random_keypair();

        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Metadata::default(),
            attachments: None,
        };
        let signature = TransactionSignature(checked_transaction_payload_signature(
            signer.private_key(),
            &payload,
        ));
        let tx = SignedTransaction {
            signature,
            payload,
            multisig_signatures: None,
        };

        let err = tx
            .verify_signature()
            .expect_err("multisig must be rejected");
        assert!(
            matches!(err, TransactionSignatureError::MissingMultisigSignatures),
            "expected MissingMultisigSignatures, got {err:?}"
        );
        assert_eq!(
            err.to_string(),
            "missing multisig signatures for multisig authority",
            "expected stable multisig missing-signatures reason"
        );
    }

    #[test]
    fn verify_signature_accepts_multisig_with_quorum() {
        let chain: ChainId = "multisig-chain-ok".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let signer = checked_random_keypair();

        let member =
            MultisigMember::new(signer.public_key().clone(), 2).expect("multisig member valid");
        let policy = MultisigPolicy::new(2, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy.clone());

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Metadata::default(),
            attachments: None,
        };
        let member_sig = checked_transaction_payload_signature(signer.private_key(), &payload);
        let signature = TransactionSignature(member_sig.clone());
        let multisig_signatures = MultisigSignatures::new(vec![MultisigSignature::new(
            signer.public_key().clone(),
            member_sig,
        )]);
        let tx = SignedTransaction {
            signature,
            payload,
            multisig_signatures: Some(multisig_signatures),
        };

        tx.verify_signature()
            .expect("multisig with quorum must verify");

        let mut noncanonical = tx;
        let unrelated = checked_random_keypair();
        noncanonical.signature = TransactionSignature(checked_transaction_payload_signature(
            unrelated.private_key(),
            noncanonical.payload(),
        ));
        assert_eq!(
            noncanonical
                .verify_signature()
                .expect_err("the primary signature must duplicate the first canonical bundle item"),
            TransactionSignatureError::NonCanonicalMultisigSignatures
        );
    }

    #[test]
    fn verify_signature_rejects_multisig_bundle_for_single_controller() {
        let chain: ChainId = "single-with-multisig-bundle".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let keypair = checked_random_keypair();
        let authority = AccountId::new(keypair.public_key().clone());
        let mut tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "single authority".into())])
        .sign(keypair.private_key());

        // A proof bundle for a different controller shape must not create an
        // alternate accepted envelope for the same signed intent.
        let payload = tx.payload().clone();
        let extraneous_signer = checked_random_keypair();
        let stray_signature =
            checked_transaction_payload_signature(extraneous_signer.private_key(), &payload);
        tx.set_multisig_signatures(MultisigSignatures::new(vec![MultisigSignature::new(
            extraneous_signer.public_key().clone(),
            stray_signature,
        )]));

        assert_eq!(
            tx.signature_count(),
            1,
            "single controller counts only its own signature"
        );
        assert_eq!(
            tx.verify_signature()
                .expect_err("single authority must reject multisig proof data"),
            TransactionSignatureError::UnexpectedMultisigSignatures
        );
    }

    #[test]
    fn transaction_builder_try_sign_multisig_rejects_empty_signers() {
        let chain: ChainId = "multisig-empty-try-sign".parse().unwrap();
        let signer = checked_random_keypair();
        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy);
        let builder = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "empty multisig".into())]);

        let err = builder
            .try_sign_multisig(core::iter::empty::<&iroha_crypto::PrivateKey>())
            .expect_err("empty signer set must be rejected");

        assert!(matches!(err, TransactionSignatureError::NoSignatures));
    }

    #[test]
    fn verify_signature_rejects_empty_multisig_bundle() {
        let chain: ChainId = "multisig-chain-empty".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let signer = checked_random_keypair();

        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Metadata::default(),
            attachments: None,
        };
        let signature = TransactionSignature(checked_transaction_payload_signature(
            signer.private_key(),
            &payload,
        ));
        let tx = SignedTransaction {
            signature,
            payload,
            multisig_signatures: Some(MultisigSignatures::new(Vec::new())),
        };

        let err = tx
            .verify_signature()
            .expect_err("empty multisig bundle must fail");
        assert!(
            matches!(err, TransactionSignatureError::NoSignatures),
            "expected NoSignatures, got {err:?}"
        );
    }

    #[test]
    fn verify_signature_rejects_unknown_signer() {
        let chain: ChainId = "multisig-chain-unknown".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let member_key = checked_random_keypair();
        let unknown_key = checked_random_keypair();

        let member =
            MultisigMember::new(member_key.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Metadata::default(),
            attachments: None,
        };
        let unknown_signature =
            checked_transaction_payload_signature(unknown_key.private_key(), &payload);
        let signature = TransactionSignature(unknown_signature.clone());
        let multisig_signatures = MultisigSignatures::new(vec![MultisigSignature::new(
            unknown_key.public_key().clone(),
            unknown_signature,
        )]);

        let tx = SignedTransaction {
            signature,
            payload,
            multisig_signatures: Some(multisig_signatures),
        };

        let err = tx
            .verify_signature()
            .expect_err("unknown signer must be rejected");
        assert!(
            matches!(err, TransactionSignatureError::UnknownMultisigSigner),
            "expected UnknownMultisigSigner, got {err:?}"
        );
    }

    #[test]
    fn verify_signature_does_not_double_count_duplicates() {
        let chain: ChainId = "multisig-chain-duplicate".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let signer = checked_random_keypair();
        let other = checked_random_keypair();

        let members = vec![
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid"),
            MultisigMember::new(other.public_key().clone(), 1).expect("multisig member valid"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Metadata::default(),
            attachments: None,
        };
        let signature = TransactionSignature(checked_transaction_payload_signature(
            signer.private_key(),
            &payload,
        ));
        let duplicate_signature =
            checked_transaction_payload_signature(signer.private_key(), &payload);
        let multisig_signatures = MultisigSignatures::new(vec![
            MultisigSignature::new(signer.public_key().clone(), duplicate_signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), duplicate_signature),
        ]);

        let tx = SignedTransaction {
            signature,
            payload,
            multisig_signatures: Some(multisig_signatures),
        };

        assert_eq!(
            tx.verify_signature()
                .expect_err("duplicate signatures are a non-canonical proof"),
            TransactionSignatureError::NonCanonicalMultisigSignatures
        );
    }

    #[test]
    fn verify_signature_accepts_mixed_algorithms() {
        let chain: ChainId = "multisig-mixed-algo".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let ed = checked_random_keypair();
        let secp = checked_random_keypair_with_algorithm(Algorithm::Secp256k1);

        let members = vec![
            MultisigMember::new(ed.public_key().clone(), 1).expect("member"),
            MultisigMember::new(secp.public_key().clone(), 1).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(policy);

        let tx = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign_multisig(vec![ed.private_key(), secp.private_key()]);

        assert_eq!(tx.signature_count(), 2);
        tx.verify_signature()
            .expect("mixed-algorithm multisig should verify");
    }

    #[test]
    fn signature_count_tracks_all_multisig_entries() {
        let chain: ChainId = "multisig-count".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let signer = checked_random_keypair();

        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Metadata::default(),
            attachments: None,
        };
        let signature = checked_transaction_payload_signature(signer.private_key(), &payload);
        let multisig_signatures = MultisigSignatures::new(vec![
            MultisigSignature::new(signer.public_key().clone(), signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), signature.clone()),
        ]);

        let tx = SignedTransaction {
            signature: TransactionSignature(signature),
            payload,
            multisig_signatures: Some(multisig_signatures),
        };

        assert_eq!(tx.signature_count(), 3);
        assert_eq!(
            tx.verify_signature()
                .expect_err("duplicate multisig entries must fail closed"),
            TransactionSignatureError::NonCanonicalMultisigSignatures
        );
    }

    #[test]
    fn transaction_result_hash_matches_inner() {
        let ok_inner = DataTriggerSequence::default();
        let result_ok = TransactionResult::new(Ok(ok_inner.clone()));
        assert_eq!(HashOf::new(&result_ok), result_ok.hash());
        assert_eq!(
            result_ok.hash(),
            TransactionResult::hash_from_inner(&Ok(ok_inner))
        );

        let err_reason =
            error::TransactionRejectionReason::LimitCheck(error::TransactionLimitError {
                reason: "limit exceeded".into(),
            });
        let err_inner: TransactionResultInner = Err(err_reason.clone());
        let result_err = TransactionResult::new(err_inner.clone());
        assert_eq!(HashOf::new(&result_err), result_err.hash());
        assert_eq!(
            result_err.hash(),
            TransactionResult::hash_from_inner(&err_inner)
        );
    }

    #[test]
    fn sealed_transaction_commitment_signs_and_reveals_expected_hash() {
        let tx = sample_signed_transaction();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let salt = [0xA5; 32];
        let reveal_deadline_height = 42;
        let commitment =
            compute_sealed_transaction_commitment(tx.chain(), &tx, salt, reveal_deadline_height);
        {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                compute_sealed_transaction_commitment(
                    tx.chain(),
                    &tx,
                    salt,
                    reveal_deadline_height,
                ),
                commitment
            );
        }
        let payload = SealedTransactionCommitmentPayload::new(
            tx.chain().clone(),
            tx.authority().clone(),
            commitment,
            10,
            reveal_deadline_height,
            core::num::NonZeroU64::new(7),
        );
        let signed = SignedSealedTransactionCommitment::sign(payload.clone(), &private_key);

        signed
            .verify_signature()
            .expect("sealed commitment signature verifies");
        assert_eq!(signed.payload(), &payload);
        assert_eq!(signed.commitment(), &commitment);

        let reveal = SealedTransactionReveal::new(commitment, tx, salt);
        assert_eq!(
            reveal.expected_commitment_with_deadline(reveal_deadline_height),
            commitment
        );
        assert_ne!(
            reveal.expected_commitment_with_deadline(reveal_deadline_height + 1),
            commitment
        );
    }

    #[test]
    fn sealed_transaction_commitment_try_sign_matches_compatibility_sign() {
        let tx = sample_signed_transaction();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let payload = SealedTransactionCommitmentPayload::new(
            tx.chain().clone(),
            tx.authority().clone(),
            compute_sealed_transaction_commitment(tx.chain(), &tx, [0x5A; 32], 64),
            11,
            64,
            core::num::NonZeroU64::new(9),
        );

        let fallible = SignedSealedTransactionCommitment::try_sign(payload.clone(), &private_key)
            .expect("sealed commitment signing should succeed");
        let compatibility = SignedSealedTransactionCommitment::sign(payload, &private_key);

        assert_eq!(fallible, compatibility);
        fallible
            .verify_signature()
            .expect("fallible sealed commitment signature verifies");
    }

    #[cfg(feature = "json")]
    #[test]
    fn transaction_entrypoint_json_roundtrip() {
        let chain: ChainId = "json-chain".parse().unwrap();
        let _domain: DomainId = DomainId::try_new("default", "universal").unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let authority = AccountId::new(public_key);

        let tx = TransactionBuilder::new(
            chain,
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(Vec::new().into()))
        .sign(&private_key);
        let entry = TransactionEntrypoint::External(tx);
        let json = norito::json::to_json(&entry).expect("serialize external entrypoint");
        let decoded: TransactionEntrypoint =
            norito::json::from_str(&json).expect("deserialize external entrypoint");
        assert_eq!(entry, decoded);

        let time_entry = TimeTriggerEntrypoint {
            id: "trigger".parse().unwrap(),
            instructions: ExecutionStep(Vec::new().into()),
            authority,
        };
        let entry = TransactionEntrypoint::Time(time_entry);
        let json = norito::json::to_json(&entry).expect("serialize time entrypoint");
        let decoded: TransactionEntrypoint =
            norito::json::from_str(&json).expect("deserialize time entrypoint");
        assert_eq!(entry, decoded);
    }

    #[cfg(feature = "json")]
    #[test]
    fn transaction_result_json_roundtrip() {
        let ok_result = TransactionResult::new(Ok(DataTriggerSequence::default()));
        let json = norito::json::to_json(&ok_result).expect("serialize ok result");
        let decoded: TransactionResult =
            norito::json::from_str(&json).expect("deserialize ok result");
        assert_eq!(ok_result, decoded);

        let err_reason =
            error::TransactionRejectionReason::LimitCheck(error::TransactionLimitError {
                reason: "limit exceeded".into(),
            });
        let err_result = TransactionResult::new(Err(err_reason));
        let json = norito::json::to_json(&err_result).expect("serialize err result");
        let decoded: TransactionResult =
            norito::json::from_str(&json).expect("deserialize err result");
        assert_eq!(err_result, decoded);
    }
}

#[cfg(test)]
#[path = "signed/ttl_tests.rs"]
mod ttl_tests;

#[cfg(all(test, feature = "fault_injection"))]
#[path = "signed/fault_injection_tests.rs"]
mod fault_injection_tests;

#[cfg(test)]
mod attachments_tests {
    use super::*;

    #[test]
    fn signed_tx_with_attachments_roundtrip() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let authority = AccountId::new(iroha_crypto::PublicKey::from(private_key.clone()));

        let attachments =
            crate::proof::ProofAttachmentList(vec![crate::proof::ProofAttachment::new_ref(
                "halo2/ipa".into(),
                crate::proof::ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_1"),
            )]);

        let tx: SignedTransaction = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Instructions(Vec::new().into()))
        .with_attachments(attachments)
        .sign(&private_key);

        let bytes = norito::to_bytes(&tx).expect("encode");
        let archived = norito::from_bytes::<SignedTransaction>(&bytes).expect("archived");
        let decoded: SignedTransaction = norito::core::NoritoDeserialize::deserialize(archived);
        assert!(decoded.attachments().is_some());
        decoded
            .verify_signature()
            .expect("round-tripped attachment remains signature-bound");

        let original_hash = decoded.hash();
        let mut tampered = decoded;
        tampered.payload.attachments = Some(crate::proof::ProofAttachmentList(vec![
            crate::proof::ProofAttachment::new_ref(
                "halo2/ipa".into(),
                crate::proof::ProofBox::new("halo2/ipa".into(), vec![9, 9, 9]),
                crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_1"),
            ),
        ]));
        assert_ne!(
            tampered.hash(),
            original_hash,
            "execution-affecting attachments must change transaction identity"
        );
        tampered
            .verify_signature()
            .expect_err("an attachment mutation must invalidate authorization");
    }
}

impl TransactionEntrypoint {
    /// Account authorized to initiate this transaction when one exists.
    #[inline]
    pub fn authority_opt(&self) -> Option<&AccountId> {
        match self {
            TransactionEntrypoint::External(entrypoint) => Some(entrypoint.authority()),
            TransactionEntrypoint::SealedCommitment(entrypoint) => Some(entrypoint.authority()),
            TransactionEntrypoint::SealedReveal(entrypoint) => {
                Some(entrypoint.signed_transaction().authority())
            }
            TransactionEntrypoint::PrivateKaigi(_) => None,
            TransactionEntrypoint::Time(entrypoint) => Some(&entrypoint.authority),
        }
    }

    /// Account authorized to initiate this transaction.
    ///
    /// # Panics
    ///
    /// Panics for authority-free private Kaigi entrypoints. Call
    /// [`Self::authority_opt`] when the entrypoint kind is not known in advance.
    #[inline]
    pub fn authority(&self) -> &AccountId {
        match self {
            TransactionEntrypoint::External(entrypoint) => entrypoint.authority(),
            TransactionEntrypoint::SealedCommitment(entrypoint) => entrypoint.authority(),
            TransactionEntrypoint::SealedReveal(entrypoint) => {
                entrypoint.signed_transaction().authority()
            }
            TransactionEntrypoint::PrivateKaigi(_) => {
                panic!("private kaigi entrypoints do not carry a public authority")
            }
            TransactionEntrypoint::Time(entrypoint) => &entrypoint.authority,
        }
    }

    /// Creation timestamp in milliseconds when the entrypoint carries one.
    #[inline]
    pub fn creation_time_ms(&self) -> Option<u64> {
        match self {
            TransactionEntrypoint::External(entrypoint) => {
                u64::try_from(entrypoint.creation_time().as_millis()).ok()
            }
            TransactionEntrypoint::SealedReveal(entrypoint) => {
                u64::try_from(entrypoint.signed_transaction().creation_time().as_millis()).ok()
            }
            TransactionEntrypoint::PrivateKaigi(entrypoint) => Some(entrypoint.creation_time_ms),
            TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
        }
    }

    /// Metadata attached to the entrypoint when one exists.
    #[inline]
    pub fn metadata(&self) -> Option<&Metadata> {
        match self {
            TransactionEntrypoint::External(entrypoint) => Some(entrypoint.metadata()),
            TransactionEntrypoint::SealedReveal(entrypoint) => {
                Some(entrypoint.signed_transaction().metadata())
            }
            TransactionEntrypoint::PrivateKaigi(entrypoint) => Some(&entrypoint.metadata),
            TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
        }
    }

    /// Hash for this transaction entrypoint.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        match self {
            Self::External(transaction) => transaction.hash_as_entrypoint(),
            Self::SealedCommitment(_)
            | Self::SealedReveal(_)
            | Self::PrivateKaigi(_)
            | Self::Time(_) => HashOf::new(self),
        }
    }
}

impl TransactionResult {
    /// Construct a transaction result without independent-batch receipts.
    #[inline]
    #[must_use]
    pub fn new(inner: TransactionResultInner) -> Self {
        Self(inner, Vec::new())
    }

    /// Durable per-leg receipts emitted by an independently settled native transfer batch.
    #[inline]
    #[must_use]
    pub fn batch_transfer_outcomes(&self) -> &[AssetBatchTransferOutcome] {
        &self.1
    }

    /// Replace the durable per-leg receipts committed by this transaction-result leaf.
    #[inline]
    pub fn set_batch_transfer_outcomes(&mut self, outcomes: Vec<AssetBatchTransferOutcome>) {
        self.1 = outcomes;
    }

    /// Hash for this transaction result.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }

    /// Hash for this transaction result computed from its inner representation.
    #[inline]
    pub fn hash_from_inner(inner: &TransactionResultInner) -> HashOf<Self> {
        HashOf::new(&TransactionResult::new(inner.clone()))
    }
}

impl From<TransactionResultInner> for TransactionResult {
    #[inline]
    fn from(inner: TransactionResultInner) -> Self {
        Self::new(inner)
    }
}

impl core::ops::Deref for TransactionResult {
    type Target = TransactionResultInner;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::fmt::Display for TransactionResult {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("TransactionResult")
    }
}

#[cfg(test)]
mod norito_rpc_fixture_tests {
    use super::*;
    use crate::account::address::ChainDiscriminantGuard;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use iroha_crypto::Hash;
    use norito::{
        core::DecodeFromSlice,
        json::{self, Value},
    };
    use std::{collections::BTreeSet, fs, path::PathBuf};

    fn manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("norito_rpc")
            .join("transaction_fixtures.manifest.json")
    }

    fn compact_hash_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("norito_rpc")
            .join("iroha_compact_hash_vector.properties")
    }

    fn compact_hash_fixture() -> std::collections::BTreeMap<String, String> {
        let path = compact_hash_fixture_path();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        parse_compact_hash_fixture(&raw)
    }

    fn parse_compact_hash_fixture(raw: &str) -> std::collections::BTreeMap<String, String> {
        const EXPECTED_KEYS: [&str; 10] = [
            "schema.version",
            "source.fixture",
            "versioned.bytes",
            "versioned.sha256",
            "bare.bytes",
            "compact.length.hex",
            "canonical.prefix.hex",
            "canonical.hash",
            "payload.prehash",
            "versioned.base64",
        ];
        let mut properties = std::collections::BTreeMap::new();
        for line in raw
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("malformed compact hash fixture line: {line}"));
            assert!(
                !key.is_empty() && !value.is_empty(),
                "malformed compact hash fixture line: {line}"
            );
            assert!(
                properties
                    .insert(key.to_owned(), value.to_owned())
                    .is_none(),
                "duplicate compact hash fixture key: {key}"
            );
        }
        let actual_keys: BTreeSet<&str> = properties.keys().map(String::as_str).collect();
        let expected_keys: BTreeSet<&str> = EXPECTED_KEYS.into_iter().collect();
        assert_eq!(
            actual_keys, expected_keys,
            "compact hash fixture keys must match the required set"
        );
        let versioned_base64 = properties
            .get("versioned.base64")
            .expect("required versioned.base64 property");
        let versioned = BASE64
            .decode(versioned_base64)
            .expect("versioned.base64 must be valid canonical base64");
        assert_eq!(
            BASE64.encode(versioned),
            *versioned_base64,
            "versioned.base64 must be canonical"
        );
        properties
    }

    fn require_object<'a>(value: &'a Value, context: &str) -> &'a json::Map {
        value
            .as_object()
            .unwrap_or_else(|| panic!("{context} must be a JSON object"))
    }

    fn require_array<'a>(value: &'a Value, context: &str) -> &'a Vec<Value> {
        value
            .as_array()
            .unwrap_or_else(|| panic!("{context} must be a JSON array"))
    }

    fn require_str<'a>(map: &'a json::Map, key: &str, context: &str) -> &'a str {
        map.get(key)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{context}: missing {key} string"))
    }

    fn require_u64(map: &json::Map, key: &str, context: &str) -> u64 {
        map.get(key)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("{context}: missing {key} integer"))
    }

    fn optional_u64(map: &json::Map, key: &str, context: &str) -> Option<u64> {
        match map.get(key) {
            Some(Value::Null) | None => None,
            Some(Value::Number(number)) => number
                .as_u64()
                .or_else(|| panic!("{context}: {key} must be an integer or null")),
            Some(_) => panic!("{context}: {key} must be an integer or null"),
        }
    }

    fn authority_prefix(authority: &str) -> Option<u16> {
        if authority.starts_with("sora") {
            return Some(0x02F1);
        }
        if authority.starts_with("test") {
            return Some(0x0171);
        }
        if authority.starts_with("dev") {
            return Some(0x0000);
        }
        authority
            .strip_prefix('n')
            .and_then(|rest| {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    None
                } else {
                    Some(digits)
                }
            })
            .and_then(|digits| digits.parse::<u16>().ok())
    }

    #[allow(
        clippy::too_many_lines,
        clippy::explicit_iter_loop,
        clippy::collapsible_if,
        clippy::collapsible_match
    )]
    #[test]
    fn norito_rpc_fixture_manifest_roundtrips() {
        let path = manifest_path();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
        let manifest: Value =
            json::from_str(&raw).unwrap_or_else(|err| panic!("manifest JSON: {err}"));
        let manifest_obj = require_object(&manifest, "manifest");
        let fixtures = manifest_obj.get("fixtures").map_or_else(
            || panic!("manifest missing fixtures array"),
            |value| require_array(value, "manifest.fixtures"),
        );

        let mut names = BTreeSet::new();
        let mut encoded_files = BTreeSet::new();
        let mut payload_hashes = BTreeSet::new();
        let mut payload_bytes_values = BTreeSet::new();
        let mut signed_hashes = BTreeSet::new();
        let mut signed_bytes_values = BTreeSet::new();
        for fixture in fixtures {
            let entry = require_object(fixture, "fixture");
            let name = require_str(entry, "name", "fixture");
            let encoded_file = require_str(entry, "encoded_file", name);
            let payload_base64 = require_str(entry, "payload_base64", name);
            let signed_base64 = require_str(entry, "signed_base64", name);
            let payload_hash = require_str(entry, "payload_hash", name);
            let signed_hash = require_str(entry, "signed_hash", name);
            assert!(names.insert(name), "duplicate fixture name: {name}");
            assert!(
                encoded_files.insert(encoded_file),
                "duplicate fixture encoded_file: {encoded_file}"
            );
            assert!(
                payload_hashes.insert(payload_hash),
                "duplicate fixture payload_hash: {payload_hash}"
            );
            assert!(
                signed_hashes.insert(signed_hash),
                "duplicate fixture signed_hash: {signed_hash}"
            );
            let encoded_len = require_u64(entry, "encoded_len", name);
            let signed_len = require_u64(entry, "signed_len", name);
            let chain = require_str(entry, "chain", name);
            let authority = require_str(entry, "authority", name);
            let _chain_guard = authority_prefix(authority).map(ChainDiscriminantGuard::enter);
            let creation_time_ms = require_u64(entry, "creation_time_ms", name);
            let time_to_live_ms = optional_u64(entry, "time_to_live_ms", name);
            let nonce = optional_u64(entry, "nonce", name);

            let payload_bytes = BASE64
                .decode(payload_base64.as_bytes())
                .unwrap_or_else(|err| panic!("{name}: invalid payload_base64: {err}"));
            let signed_bytes = BASE64
                .decode(signed_base64.as_bytes())
                .unwrap_or_else(|err| panic!("{name}: invalid signed_base64: {err}"));
            assert_eq!(
                BASE64.encode(&payload_bytes),
                payload_base64,
                "{name}: payload_base64 must be canonical"
            );
            assert_eq!(
                BASE64.encode(&signed_bytes),
                signed_base64,
                "{name}: signed_base64 must be canonical"
            );
            assert!(
                payload_bytes_values.insert(payload_bytes.clone()),
                "duplicate fixture payload bytes: {name}"
            );
            assert!(
                signed_bytes_values.insert(signed_bytes.clone()),
                "duplicate fixture signed bytes: {name}"
            );
            assert_eq!(
                payload_bytes.len() as u64,
                encoded_len,
                "{name}: encoded_len mismatch"
            );
            assert_eq!(
                signed_bytes.len() as u64,
                signed_len,
                "{name}: signed_len mismatch"
            );

            let computed_payload_hash = Hash::new(&payload_bytes).to_string();
            assert_eq!(
                computed_payload_hash, payload_hash,
                "{name}: payload_hash mismatch"
            );

            let (signed_tx, used) = SignedTransaction::decode_from_slice(&signed_bytes)
                .unwrap_or_else(|err| panic!("{name}: signed transaction decode failed: {err}"));
            assert_eq!(
                used,
                signed_bytes.len(),
                "{name}: signed transaction has trailing bytes"
            );
            assert_eq!(
                signed_tx.hash_as_entrypoint().to_string(),
                signed_hash,
                "{name}: signed_hash mismatch"
            );
            assert_eq!(signed_tx.chain().as_str(), chain, "{name}: chain mismatch");
            let expected_authority = AccountId::parse_encoded(authority).map_or_else(
                |err| panic!("{name}: authority parse failed: {err}"),
                crate::account::ParsedAccountId::into_account_id,
            );
            assert_eq!(
                signed_tx.authority().to_string(),
                expected_authority.to_string(),
                "{name}: authority mismatch"
            );
            let creation_ms = u64::try_from(signed_tx.creation_time().as_millis())
                .expect("creation_time_ms fits u64");
            assert_eq!(
                creation_ms, creation_time_ms,
                "{name}: creation_time_ms mismatch"
            );
            let ttl_ms = signed_tx
                .time_to_live()
                .map(|ttl| u64::try_from(ttl.as_millis()).expect("time_to_live_ms fits u64"));
            assert_eq!(ttl_ms, time_to_live_ms, "{name}: time_to_live_ms mismatch");
            assert_eq!(
                signed_tx.nonce().map(NonZeroU32::get).map(u64::from),
                nonce,
                "{name}: nonce mismatch"
            );

            let signed_payload_bytes = norito::codec::encode_adaptive(signed_tx.payload());
            if signed_payload_bytes != payload_bytes {
                fn first_diff(left: &[u8], right: &[u8]) -> Option<(usize, u8, u8)> {
                    let shared_len = left.len().min(right.len());
                    for idx in 0..shared_len {
                        if left[idx] != right[idx] {
                            return Some((idx, left[idx], right[idx]));
                        }
                    }
                    None
                }

                let payload_from_fixture: TransactionPayload = {
                    let _guard = norito::core::PayloadCtxGuard::enter(&payload_bytes);
                    let mut cursor = std::io::Cursor::new(&payload_bytes);
                    let decoded: TransactionPayload = norito::codec::Decode::decode(&mut cursor)
                        .unwrap_or_else(|err| {
                            panic!("{name}: decode payload fixture bytes (bare): {err}")
                        });
                    let used =
                        usize::try_from(cursor.position()).expect("cursor.position fits usize");
                    assert_eq!(
                        used,
                        payload_bytes.len(),
                        "{name}: payload fixture contains trailing bytes"
                    );
                    decoded
                };

                let payload_equal = &payload_from_fixture == signed_tx.payload();
                let diff = first_diff(&signed_payload_bytes, &payload_bytes);
                let mut has_invalid_instruction = false;
                let mut register_role_stats: Option<(usize, usize)> = None;
                let mut instruction_count: Option<usize> = None;
                let mut instruction_types: Vec<&'static str> = Vec::new();
                if let Executable::Instructions(instrs) = signed_tx.instructions() {
                    instruction_count = Some(instrs.len());
                    for instr in instrs.iter() {
                        if instruction_types.len() < 16 {
                            instruction_types.push(crate::isi::Instruction::id(&**instr));
                        }
                        if instr
                            .as_any()
                            .downcast_ref::<crate::isi::InvalidInstruction>()
                            .is_some()
                        {
                            has_invalid_instruction = true;
                        }
                        if let Some(register) = instr
                            .as_any()
                            .downcast_ref::<crate::isi::Register<crate::role::Role>>()
                        {
                            let perms = register.object.inner.permissions.len();
                            let epochs = register.object.inner.permission_epochs.len();
                            register_role_stats = Some((perms, epochs));
                        }
                        if let Some(register_box) =
                            instr.as_any().downcast_ref::<crate::isi::RegisterBox>()
                        {
                            if let crate::isi::RegisterBox::Role(register) = register_box {
                                let perms = register.object.inner.permissions.len();
                                let epochs = register.object.inner.permission_epochs.len();
                                register_role_stats = Some((perms, epochs));
                            }
                        }
                    }
                }

                panic!(
                    "{name}: payload bytes mismatch after decode+re-encode (len encoded={}, len fixture={}, first_diff={diff:?}, payload_equal={payload_equal}, has_invalid_instruction={has_invalid_instruction}, register_role_stats={register_role_stats:?}, instruction_count={instruction_count:?}, instruction_types={instruction_types:?})",
                    signed_payload_bytes.len(),
                    payload_bytes.len(),
                );
            }

            let signed_reencoded = norito::codec::encode_adaptive(&signed_tx);
            assert_eq!(
                signed_reencoded, signed_bytes,
                "{name}: signed bytes mismatch after re-encode"
            );
        }
    }

    #[test]
    fn compact_hash_fixture_rejects_duplicate_property_keys() {
        let raw =
            fs::read_to_string(compact_hash_fixture_path()).expect("read compact hash fixture");
        let duplicated = format!("{raw}\ncanonical.hash=duplicate\n");
        let panic = std::panic::catch_unwind(|| parse_compact_hash_fixture(&duplicated));
        assert!(
            panic.is_err(),
            "duplicate compact fixture keys must fail closed"
        );
    }

    #[test]
    fn compact_external_entrypoint_golden_matches_native_hash_and_rejects_alias_encodings() {
        use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
        use sha2::Digest as _;
        let fixture = compact_hash_fixture();
        assert_eq!(fixture["schema.version"], "2");
        assert_eq!(fixture["source.fixture"], "transfer_asset");
        let versioned = BASE64
            .decode(fixture["versioned.base64"].as_bytes())
            .expect("compact hash fixture must contain valid base64");
        assert_eq!(
            versioned.len(),
            fixture["versioned.bytes"].parse::<usize>().unwrap()
        );
        assert_eq!(versioned.first(), Some(&1));
        assert_eq!(
            hex::encode(sha2::Sha256::digest(&versioned)),
            fixture["versioned.sha256"]
        );

        let transaction = SignedTransaction::decode_all_versioned(&versioned)
            .expect("compact hash fixture must decode as an exact versioned transaction");
        assert_eq!(transaction.encode_versioned(), versioned);
        assert_eq!(
            hex::encode(HashOf::new(transaction.payload()).as_ref()),
            fixture["payload.prehash"],
            "decoded payload prehash must match the shared signer golden"
        );
        let bare = norito::codec::encode_adaptive(&transaction);
        assert_eq!(bare.len(), fixture["bare.bytes"].parse::<usize>().unwrap());
        assert_eq!(bare, versioned[1..]);

        let payload = transaction.payload().encode();
        let mut canonical = 0_u32.to_le_bytes().to_vec();
        norito::core::write_len_to_vec(&mut canonical, payload.len() as u64);
        canonical.extend_from_slice(&payload);
        let entrypoint = TransactionEntrypoint::External(transaction);
        let expected_prefix = hex::decode(&fixture["canonical.prefix.hex"]).unwrap();
        assert!(canonical.starts_with(&expected_prefix));
        assert_eq!(
            hex::encode(iroha_crypto::Hash::new(&canonical).as_ref()),
            fixture["canonical.hash"],
            "the payload-only External identity preimage must match the shared golden"
        );
        assert_eq!(
            hex::encode(entrypoint.hash().as_ref()),
            fixture["canonical.hash"],
            "Rust entrypoint hash must match the shared Android/browser golden"
        );

        let mut overlong_signed = Vec::with_capacity(versioned.len() + 1);
        overlong_signed.extend_from_slice(&versioned[..2]);
        assert_eq!(versioned[1..3], [0x8a, 0x01]);
        overlong_signed.extend_from_slice(&[0x81, 0x00]);
        overlong_signed.extend_from_slice(&versioned[3..]);
        SignedTransaction::decode_all_versioned(&overlong_signed)
            .expect_err("overlong signed-transaction field length must be rejected");

        assert_eq!(
            expected_prefix.len(),
            6,
            "the shared fixture must exercise a two-byte External COMPACT_LEN"
        );
        assert_eq!(
            &canonical[..expected_prefix.len()],
            expected_prefix.as_slice()
        );
        let first_length_index = expected_prefix.len() - 2;
        assert_ne!(
            canonical[first_length_index] & 0x80,
            0,
            "the first External length byte must continue"
        );
        let terminal_index = expected_prefix.len() - 1;
        let terminal = canonical[terminal_index];
        assert_eq!(
            terminal & 0x80,
            0,
            "the second External length byte must terminate"
        );
        let mut overlong_entrypoint = Vec::with_capacity(canonical.len() + 1);
        overlong_entrypoint.extend_from_slice(&canonical[..terminal_index]);
        overlong_entrypoint.extend_from_slice(&[terminal | 0x80, 0x00]);
        overlong_entrypoint.extend_from_slice(&canonical[terminal_index + 1..]);
        assert_ne!(
            iroha_crypto::Hash::new(&overlong_entrypoint).as_ref(),
            entrypoint.hash().as_ref(),
            "an overlong External identity length must not alias the canonical hash"
        );

        let mut fixed_width_entrypoint = Vec::with_capacity(canonical.len() + 6);
        fixed_width_entrypoint.extend_from_slice(&canonical[..4]);
        fixed_width_entrypoint.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        fixed_width_entrypoint.extend_from_slice(&payload);
        assert_ne!(
            iroha_crypto::Hash::new(&fixed_width_entrypoint).as_ref(),
            entrypoint.hash().as_ref(),
            "fixed-u64 External identity length must not alias canonical COMPACT_LEN bytes"
        );

        let wire_entrypoint = norito::codec::encode_adaptive(&entrypoint);
        assert_ne!(
            wire_entrypoint, canonical,
            "the authorization-bearing entrypoint wire is distinct from its identity preimage"
        );
        assert_eq!(
            norito::codec::decode_adaptive::<TransactionEntrypoint>(&wire_entrypoint)
                .expect("canonical authorization-bearing entrypoint wire"),
            entrypoint
        );
    }
}
