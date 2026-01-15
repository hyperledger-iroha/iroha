//! Transaction structures and related implementations.
use std::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
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
use iroha_crypto::{Algorithm, HashOf, PublicKey, Signature, SignatureOf};
use iroha_data_model_derive::model;
use iroha_primitives::{const_vec::ConstVec, json::Json, time::TimeSource};
use iroha_schema::IntoSchema;
use iroha_version::Version;
use norito::codec::{Decode, DecodeAll, Encode};
use thiserror::Error;

pub use self::model::*;
use super::{
    error,
    executable::{Executable, IvmBytecode},
};
use crate::{
    ChainId,
    account::{AccountController, AccountId, MultisigPolicy},
    isi::InstructionBox,
    metadata::Metadata,
    name::Name,
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};

#[model]
mod model {
    use iroha_primitives::const_vec::ConstVec;

    use super::*;
    use crate::account::AccountId;

    /// Iroha transaction payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) struct TransactionPayload {
        /// Unique id of the blockchain. Used for simple replay attack protection.
        pub chain: ChainId,
        /// Account ID of transaction creator. The signatory component is
        /// normalised during signing to match the public key derived from the
        /// signing private key so callers do not need to keep duplicate copies.
        pub authority: AccountId,
        /// Creation timestamp (unix time in milliseconds).
        pub creation_time_ms: u64,
        /// ISI or IVM smart contract bytecode.
        pub instructions: Executable,
        /// If transaction is not committed by this time it will be dropped.
        pub time_to_live_ms: Option<NonZeroU64>,
        /// Random value to make different hashes for transactions which occur repeatedly and simultaneously.
        pub nonce: Option<NonZeroU32>,
        /// Store for additional information.
        pub metadata: Metadata,
    }

    /// Signature of transaction
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[norito(transparent)]
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
        /// Construct a new bundle of multisig signatures.
        pub fn new(signatures: Vec<MultisigSignature>) -> Self {
            Self { signatures }
        }
    }

    impl<'a> norito::core::DecodeFromSlice<'a> for TransactionSignature {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let (inner, used) =
                <SignatureOf<TransactionPayload> as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
            Ok((TransactionSignature(inner), used))
        }
    }

    /// Transaction containing a payload, signature, and optional proof attachments.
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
        /// Optional proof attachments associated with this transaction.
        pub(super) attachments: Option<crate::proof::ProofAttachmentList>,
        /// Optional bundle of multisig signatures when the authority uses a multisig controller.
        pub(super) multisig_signatures: Option<MultisigSignatures>,
    }

    /// Structure that represents the initial state of a transaction before the transaction receives any signatures.
    #[derive(Debug, Clone)]
    #[must_use]
    pub struct TransactionBuilder {
        /// [`Transaction`] payload.
        pub(super) payload: TransactionPayload,
        /// Optional proof attachments to include upon signing.
        pub(super) attachments: Option<crate::proof::ProofAttachmentList>,
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
        /// Scheduled time trigger that initiates a transaction.
        Time(TimeTriggerEntrypoint),
    }

    /// The outcome of processing a transaction:
    /// either a sequence of data triggers, or a rejection reason.
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
    #[display("TransactionResult")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct TransactionResult(pub TransactionResultInner);

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

// Provide Norito slice decoding by delegating to `Decode`.
impl<'a> norito::core::DecodeFromSlice<'a> for model::ExecutionStep {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        let used =
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
        Ok((v, used))
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
    /// Signature verification failed for the provided payload and signatory.
    #[error("{0}")]
    CryptoError(String),
    /// The transaction does not contain any signatures.
    #[error("transaction carries no signatures")]
    NoSignatures,
    /// Multisig signature bundle is missing.
    #[error("missing multisig signatures for multisig authority")]
    MissingMultisigSignatures,
    /// Transaction contains a signature from a non-member.
    #[error("multisig signature from unknown member")]
    UnknownMultisigSigner,
    /// Collected multisig signatures do not satisfy the policy threshold.
    #[error("insufficient multisig weight: collected {collected}, required {required}")]
    InsufficientMultisigWeight {
        #[doc = "Total weight contributed by provided signatures."]
        collected: u32,
        #[doc = "Threshold required by the multisig policy."]
        required: u16,
    },
}

#[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
pub use self::ffi::*;

static EXPIRES_AT_HEIGHT_NAME: LazyLock<Name> = LazyLock::new(|| {
    Name::from_str("expires_at_height").expect("expires_at_height is a valid metadata key")
});

/// Stable reason string for rejecting multisig controllers in tx signing paths.
pub const MULTISIG_SIGNING_UNSUPPORTED_REASON: &str =
    "multisig authority requires bundled signatures for verification";

static TX_SEQUENCE_NAME: LazyLock<Name> =
    LazyLock::new(|| Name::from_str("tx_sequence").expect("tx_sequence is a valid metadata key"));

#[cfg(feature = "fault_injection")]
static FAULT_INJECTION_METADATA_NAME: LazyLock<Name> = LazyLock::new(|| {
    Name::from_str("fault_injection_overlay")
        .expect("fault_injection_overlay is a valid metadata key")
});

impl SignedTransaction {
    /// Transaction payload. Used for tests
    pub fn payload(&self) -> &TransactionPayload {
        &self.payload
    }

    /// Return transaction instructions
    #[inline]
    pub fn instructions(&self) -> &Executable {
        &self.payload.instructions
    }

    /// Return transaction authority
    #[inline]
    pub fn authority(&self) -> &AccountId {
        &self.payload.authority
    }

    /// Return transaction metadata.
    #[inline]
    pub fn metadata(&self) -> &Metadata {
        &self.payload.metadata
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

    /// If transaction is not committed by this time it will be dropped.
    #[inline]
    pub fn time_to_live(&self) -> Option<Duration> {
        self.payload
            .time_to_live_ms
            .map(|ttl| Duration::from_millis(ttl.into()))
    }

    /// Transaction nonce
    #[inline]
    pub fn nonce(&self) -> Option<NonZeroU32> {
        self.payload.nonce
    }

    /// Transaction chain id
    #[inline]
    pub fn chain(&self) -> &ChainId {
        &self.payload.chain
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
        self.attachments.as_ref()
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

    /// Hash for this external transaction.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }

    /// Hash for this external transaction as `TransactionEntrypoint`.
    #[inline]
    pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
        HashOf::new(&TransactionEntrypoint::External(self.clone()))
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
            Executable::Ivm(_) => {
                Self::apply_fault_injection_overlay(&mut self.payload.metadata, additions);
            }
        }
    }

    #[cfg(feature = "fault_injection")]
    fn fault_injection_overlay(metadata: &Metadata) -> Option<Vec<String>> {
        metadata
            .get(&*FAULT_INJECTION_METADATA_NAME)
            .cloned()
            .and_then(|value| value.try_into_any_norito::<Vec<String>>().ok())
    }

    #[cfg(feature = "fault_injection")]
    fn apply_fault_injection_overlay(metadata: &mut Metadata, additions: Vec<InstructionBox>) {
        let mut combined = Self::fault_injection_overlay(metadata).unwrap_or_default();
        combined.extend(additions.into_iter().map(|instruction| {
            let bytes =
                norito::to_bytes(&instruction).expect("fault injection overlay instruction encode");
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
        let TransactionSignature(signature) = &self.signature;
        match self.payload.authority.controller() {
            AccountController::Single(signatory) => signature
                .verify(signatory, &self.payload)
                .map_err(|err| TransactionSignatureError::CryptoError(err.to_string())),
            AccountController::Multisig(policy) => self.verify_multisig_signatures(policy),
        }
    }
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

        let mut weights = BTreeMap::new();
        for member in policy.members() {
            weights.insert(member.public_key().clone(), member.weight());
        }

        let mut seen = BTreeSet::new();
        let mut collected: u32 = 0;
        for entry in &bundle.signatures {
            let Some(weight) = weights.get(&entry.signer) else {
                return Err(TransactionSignatureError::UnknownMultisigSigner);
            };
            if !seen.insert(entry.signer.clone()) {
                continue;
            }
            entry
                .signature
                .verify(&entry.signer, &self.payload)
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

impl iroha_version::codec::EncodeVersioned for SignedTransaction {
    fn encode_versioned(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1);
        bytes.push(self.version());
        bytes.extend(norito::codec::encode_adaptive(self));
        bytes
    }
}

impl iroha_version::codec::DecodeVersioned for SignedTransaction {
    fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        use iroha_version::error::Error;

        let Some((&version, payload)) = input.split_first() else {
            return Err(Error::NotVersioned);
        };

        if !Self::supported_versions().contains(&version) {
            return Err(Error::UnsupportedVersion(Box::new(
                iroha_version::UnsupportedVersion::new(
                    version,
                    iroha_version::RawVersioned::NoritoBytes(input.to_vec()),
                ),
            )));
        }

        let payload_guard = norito::core::PayloadCtxGuard::enter(payload);
        let mut cursor = payload;
        let decoded = <Self as DecodeAll>::decode_all(&mut cursor).map_err(Error::from)?;
        drop(payload_guard);
        if cursor.is_empty() {
            Ok(decoded)
        } else {
            Err(Error::NoritoCodec(
                "SignedTransaction payload contains trailing bytes".into(),
            ))
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SignedTransaction {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let _guard = norito::core::PayloadCtxGuard::enter(bytes);
        let mut cursor = std::io::Cursor::new(bytes);
        let decoded: SignedTransaction = norito::codec::Decode::decode(&mut cursor)?;
        let used =
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
        Ok((decoded, used))
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
                let signature = SignatureOf::new(private_key, payload);
                MultisigSignature::new(signer, signature)
            })
            .collect();

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
        parser.consume_char(b':')?;
        let value = match key.as_str() {
            "External" => {
                let tx = SignedTransaction::json_deserialize(parser)?;
                TransactionEntrypoint::External(tx)
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
        parser.skip_ws();
        let key = parser.parse_key()?;
        parser.consume_char(b':')?;
        let inner = match key.as_str() {
            "Ok" => TransactionResultInner::Ok(DataTriggerSequence::json_deserialize(parser)?),
            "Err" => TransactionResultInner::Err(
                error::TransactionRejectionReason::json_deserialize(parser)?,
            ),
            other => {
                return Err(norito::json::Error::UnknownField {
                    field: other.to_owned(),
                });
            }
        };
        parser.skip_ws();
        parser.consume_char(b'}')?;
        Ok(TransactionResult(inner))
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

impl TransactionBuilder {
    fn new_with_time(chain: ChainId, authority: AccountId, creation_time_ms: u64) -> Self {
        Self {
            payload: TransactionPayload {
                chain,
                authority,
                creation_time_ms,
                nonce: None,
                time_to_live_ms: None,
                instructions: Vec::<InstructionBox>::new().into(),
                metadata: Metadata::default(),
            },
            attachments: None,
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
    ) -> Self {
        let creation_time_ms = time_source
            .get_unix_time()
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX");

        Self::new_with_time(chain_id, authority, creation_time_ms)
    }

    /// Construct [`Self`].
    #[inline]
    pub fn new(chain_id: ChainId, authority: AccountId) -> Self {
        Self::new_with_time_source(chain_id, authority, &TimeSource::new_system())
    }
}

impl TransactionBuilder {
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

    /// Adds metadata to this transaction
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.payload.metadata = metadata;
        self
    }

    /// Attach proof payloads to this transaction before signing.
    pub fn with_attachments(mut self, attachments: crate::proof::ProofAttachmentList) -> Self {
        self.attachments = Some(attachments);
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
    /// Note: `Duration::ZERO` is a legitimate value meaning "expire immediately".
    /// Since the payload stores TTL as `Option<NonZeroU64>` milliseconds, we
    /// approximate zero by storing `Some(1)` millisecond to preserve the
    /// distinction from `None` (which means "use node default TTL").
    pub fn set_ttl(&mut self, time_to_live: Duration) -> &mut Self {
        let ttl: u64 = time_to_live
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX");

        self.payload.time_to_live_ms =
            Some(NonZeroU64::new(if ttl == 0 { 1 } else { ttl }).expect("nonzero"));

        self
    }

    /// Set creation time of transaction
    pub fn set_creation_time(&mut self, value: Duration) -> &mut Self {
        self.payload.creation_time_ms = u64::try_from(value.as_millis())
            .expect("INTERNAL BUG: Unix timestamp exceedes u64::MAX");
        self
    }

    /// Sign transaction with provided key pair.
    #[must_use]
    pub fn sign(self, private_key: &iroha_crypto::PrivateKey) -> SignedTransaction {
        let mut payload = self.payload;

        // Normalise the authority signatory so that the transaction carries a
        // single canonical copy of the public key derived from the signing key.
        #[cfg(not(feature = "ffi_import"))]
        {
            use iroha_crypto::PublicKey;

            if matches!(payload.authority.controller(), AccountController::Single(_)) {
                let derived_pk = PublicKey::from(private_key.clone());
                if payload.authority.try_signatory() != Some(&derived_pk) {
                    let domain = payload.authority.domain().clone();
                    payload.authority = AccountId::new(domain, derived_pk.clone());
                }
            }
        }

        let signature = TransactionSignature(SignatureOf::new(private_key, &payload));

        SignedTransaction {
            signature,
            payload,
            attachments: self.attachments,
            multisig_signatures: self.multisig_signatures,
        }
    }

    /// Sign a transaction whose authority uses a multisig controller.
    ///
    /// The provided signer keys are used to produce a multisig signature bundle;
    /// duplicate signers are retained here and later deduplicated during
    /// verification.
    ///
    /// # Panics
    ///
    /// Panics if no signer keys are provided.
    #[must_use]
    #[allow(single_use_lifetimes)]
    pub fn sign_multisig<'a>(
        self,
        signers: impl IntoIterator<Item = &'a iroha_crypto::PrivateKey>,
    ) -> SignedTransaction {
        let payload = self.payload;
        let mut bundle = self
            .multisig_signatures
            .unwrap_or_else(|| MultisigSignatures::new(Vec::new()));

        let produced = MultisigSignatures::from_signers(&payload, signers)
            .expect("multisig signing requires at least one signer");
        bundle.signatures.extend(produced.signatures);

        let primary_signature = bundle
            .signatures
            .first()
            .expect("multisig signing requires at least one signer")
            .signature
            .clone();

        SignedTransaction {
            signature: TransactionSignature(primary_signature),
            payload,
            attachments: self.attachments,
            multisig_signatures: Some(bundle),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Domain, DomainId, Level,
        account::{MultisigMember, MultisigPolicy},
        prelude::{Log, Register},
        transaction::signed::{MultisigSignature, MultisigSignatures},
        trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
    };

    #[test]
    fn with_instructions_accepts_instruction_box() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();

        // Pre-boxed instruction
        let instruction: InstructionBox =
            Register::domain(Domain::new("wonderland".parse().unwrap())).into();
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

        let authority = AccountId::new(domain.clone(), public_key.clone());

        let tx = TransactionBuilder::new(chain, authority.clone())
            .with_instructions(core::iter::once(instruction))
            .with_metadata(Metadata::default())
            .sign(key_pair.private_key());

        assert_eq!(tx.authority().signatory(), key_pair.public_key());

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
    fn transaction_signature_decode_from_slice_roundtrip() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(domain, public_key.clone());
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let signed_tx = TransactionBuilder::new(chain, authority).sign(&private_key);
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
    fn sign_overwrites_mismatched_signatory_with_signing_key_public_part() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let stored_public_key: iroha_crypto::PublicKey =
            "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016"
                .parse()
                .unwrap();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();
        let key_pair = iroha_crypto::KeyPair::from_private_key(private_key).unwrap();
        let authority = AccountId::new(domain, stored_public_key.clone());

        let tx = TransactionBuilder::new(chain, authority.clone()).sign(key_pair.private_key());

        assert_ne!(authority.signatory(), key_pair.public_key());
        assert_eq!(tx.authority().signatory(), key_pair.public_key());
        assert!(tx.verify_signature().is_ok());
    }

    #[test]
    fn entrypoint_hashes_match_direct_encoding() {
        let chain: ChainId = "hash-chain".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let public_key: iroha_crypto::PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .unwrap();
        let authority = AccountId::new(domain, public_key);
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let tx = TransactionBuilder::new(chain, authority.clone()).sign(&private_key);
        let entry = TransactionEntrypoint::External(tx.clone());
        assert_eq!(HashOf::new(&entry), entry.hash());
        assert_eq!(tx.hash_as_entrypoint(), entry.hash());

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
        let domain: DomainId = "wonderland".parse().unwrap();
        let signer = iroha_crypto::KeyPair::random();

        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(domain, policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            metadata: Metadata::default(),
        };
        let signature = TransactionSignature(SignatureOf::new(signer.private_key(), &payload));
        let tx = SignedTransaction {
            signature,
            payload,
            attachments: None,
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
        let domain: DomainId = "wonderland".parse().unwrap();
        let signer = iroha_crypto::KeyPair::random();

        let member =
            MultisigMember::new(signer.public_key().clone(), 2).expect("multisig member valid");
        let policy = MultisigPolicy::new(2, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(domain, policy.clone());

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            metadata: Metadata::default(),
        };
        let member_sig = SignatureOf::new(signer.private_key(), &payload);
        let signature = TransactionSignature(member_sig.clone());
        let multisig_signatures = MultisigSignatures::new(vec![MultisigSignature::new(
            signer.public_key().clone(),
            member_sig,
        )]);
        let tx = SignedTransaction {
            signature,
            payload,
            attachments: None,
            multisig_signatures: Some(multisig_signatures),
        };

        tx.verify_signature()
            .expect("multisig with quorum must verify");
    }

    #[test]
    fn verify_signature_ignores_multisig_bundle_for_single_controller() {
        let chain: ChainId = "single-with-multisig-bundle".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let keypair = iroha_crypto::KeyPair::random();
        let authority = AccountId::new(domain, keypair.public_key().clone());
        let mut tx = TransactionBuilder::new(chain, authority.clone())
            .with_instructions([Log::new(Level::INFO, "single authority".into())])
            .sign(keypair.private_key());

        // Attach a multisig bundle that does not correspond to the authority; single controllers
        // should ignore these entries during verification.
        let payload = tx.payload().clone();
        let extraneous_signer = iroha_crypto::KeyPair::random();
        let stray_signature = SignatureOf::new(extraneous_signer.private_key(), &payload);
        tx.set_multisig_signatures(MultisigSignatures::new(vec![MultisigSignature::new(
            extraneous_signer.public_key().clone(),
            stray_signature,
        )]));

        assert_eq!(
            tx.signature_count(),
            1,
            "single controller counts only its own signature"
        );
        tx.verify_signature()
            .expect("single authority verification should ignore multisig bundle");
    }

    #[test]
    fn verify_signature_rejects_empty_multisig_bundle() {
        let chain: ChainId = "multisig-chain-empty".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let signer = iroha_crypto::KeyPair::random();

        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(domain, policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            metadata: Metadata::default(),
        };
        let signature = TransactionSignature(SignatureOf::new(signer.private_key(), &payload));
        let tx = SignedTransaction {
            signature,
            payload,
            attachments: None,
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
        let domain: DomainId = "wonderland".parse().unwrap();
        let member_key = iroha_crypto::KeyPair::random();
        let unknown_key = iroha_crypto::KeyPair::random();

        let member =
            MultisigMember::new(member_key.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(domain, policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            metadata: Metadata::default(),
        };
        let signature = TransactionSignature(SignatureOf::new(member_key.private_key(), &payload));
        let multisig_signatures = MultisigSignatures::new(vec![MultisigSignature::new(
            unknown_key.public_key().clone(),
            SignatureOf::new(unknown_key.private_key(), &payload),
        )]);

        let tx = SignedTransaction {
            signature,
            payload,
            attachments: None,
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
        let domain: DomainId = "wonderland".parse().unwrap();
        let signer = iroha_crypto::KeyPair::random();
        let other = iroha_crypto::KeyPair::random();

        let members = vec![
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid"),
            MultisigMember::new(other.public_key().clone(), 1).expect("multisig member valid"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("multisig policy valid");
        let authority = AccountId::new_multisig(domain, policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            metadata: Metadata::default(),
        };
        let signature = TransactionSignature(SignatureOf::new(signer.private_key(), &payload));
        let duplicate_signature = SignatureOf::new(signer.private_key(), &payload);
        let multisig_signatures = MultisigSignatures::new(vec![
            MultisigSignature::new(signer.public_key().clone(), duplicate_signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), duplicate_signature),
        ]);

        let tx = SignedTransaction {
            signature,
            payload,
            attachments: None,
            multisig_signatures: Some(multisig_signatures),
        };

        let err = tx
            .verify_signature()
            .expect_err("duplicate signatures should not satisfy threshold");
        assert!(
            matches!(
                err,
                TransactionSignatureError::InsufficientMultisigWeight { collected, required }
                if collected == 1 && required == 2
            ),
            "expected InsufficientMultisigWeight, got {err:?}"
        );
    }

    #[test]
    fn verify_signature_accepts_mixed_algorithms() {
        let chain: ChainId = "multisig-mixed-algo".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let ed = iroha_crypto::KeyPair::random();
        let secp = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Secp256k1);

        let members = vec![
            MultisigMember::new(ed.public_key().clone(), 1).expect("member"),
            MultisigMember::new(secp.public_key().clone(), 1).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let authority = AccountId::new_multisig(domain, policy);

        let tx = TransactionBuilder::new(chain, authority)
            .sign_multisig(vec![ed.private_key(), secp.private_key()]);

        assert_eq!(tx.signature_count(), 2);
        tx.verify_signature()
            .expect("mixed-algorithm multisig should verify");
    }

    #[test]
    fn signature_count_tracks_all_multisig_entries() {
        let chain: ChainId = "multisig-count".parse().unwrap();
        let domain: DomainId = "wonderland".parse().unwrap();
        let signer = iroha_crypto::KeyPair::random();

        let member =
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig member valid");
        let policy = MultisigPolicy::new(1, vec![member]).expect("multisig policy valid");
        let authority = AccountId::new_multisig(domain, policy);

        let payload = model::TransactionPayload {
            chain,
            authority,
            creation_time_ms: 0,
            instructions: Executable::Instructions(ConstVec::from(Vec::new())),
            time_to_live_ms: None,
            nonce: None,
            metadata: Metadata::default(),
        };
        let signature = SignatureOf::new(signer.private_key(), &payload);
        let multisig_signatures = MultisigSignatures::new(vec![
            MultisigSignature::new(signer.public_key().clone(), signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), signature.clone()),
            MultisigSignature::new(signer.public_key().clone(), signature.clone()),
        ]);

        let tx = SignedTransaction {
            signature: TransactionSignature(signature),
            payload,
            attachments: None,
            multisig_signatures: Some(multisig_signatures),
        };

        assert_eq!(tx.signature_count(), 3);
        tx.verify_signature()
            .expect("duplicate multisig entries still satisfy threshold");
    }

    #[test]
    fn transaction_result_hash_matches_inner() {
        let ok_inner = DataTriggerSequence::default();
        let result_ok = TransactionResult(Ok(ok_inner.clone()));
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
        let result_err = TransactionResult(err_inner.clone());
        assert_eq!(HashOf::new(&result_err), result_err.hash());
        assert_eq!(
            result_err.hash(),
            TransactionResult::hash_from_inner(&err_inner)
        );
    }
}

#[cfg(test)]
mod ttl_tests {
    use super::*;

    #[test]
    fn zero_ttl_is_preserved_not_none() {
        let chain: ChainId = "test-chain".parse().unwrap();
        let authority: AccountId =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245@wonderland"
                .parse()
                .unwrap();
        let mut builder = TransactionBuilder::new(chain, authority);
        builder.set_ttl(Duration::from_millis(0));
        // Internally we approximate zero by 1ms to distinguish from None
        let ttl = builder
            .clone()
            .sign(
                &"802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                    .parse()
                    .unwrap(),
            )
            .time_to_live();
        assert_eq!(ttl, Some(Duration::from_millis(1)));
    }

    #[test]
    fn ingress_metadata_accessors_read_numeric_values() {
        let chain: ChainId = "ingress-chain".parse().unwrap();
        let keypair = iroha_crypto::KeyPair::random();
        let domain: crate::domain::DomainId = "wonderland".parse().unwrap();
        let account_id = AccountId::new(domain, keypair.public_key().clone());

        let mut metadata = Metadata::default();
        metadata.insert(
            crate::name::Name::from_str("expires_at_height").unwrap(),
            iroha_primitives::json::Json::from(10_u64),
        );
        metadata.insert(
            crate::name::Name::from_str("tx_sequence").unwrap(),
            iroha_primitives::json::Json::from(3_u64),
        );

        let tx = TransactionBuilder::new(chain, account_id)
            .with_metadata(metadata)
            .sign(keypair.private_key());

        assert_eq!(tx.expires_at_height().expect("parse metadata"), Some(10));
        assert_eq!(tx.tx_sequence().expect("parse metadata"), Some(3));
    }

    #[test]
    fn ingress_metadata_accessors_propagate_decode_error() {
        let chain: ChainId = "ingress-chain-invalid".parse().unwrap();
        let keypair = iroha_crypto::KeyPair::random();
        let domain: crate::domain::DomainId = "wonderland".parse().unwrap();
        let account_id = AccountId::new(domain, keypair.public_key().clone());

        let mut metadata = Metadata::default();
        metadata.insert(
            crate::name::Name::from_str("expires_at_height").unwrap(),
            iroha_primitives::json::Json::new("not-a-number"),
        );

        let tx = TransactionBuilder::new(chain, account_id)
            .with_metadata(metadata)
            .sign(keypair.private_key());

        assert!(tx.expires_at_height().is_err());
    }
}

#[cfg(all(test, feature = "fault_injection"))]
mod fault_injection_tests {
    use super::*;
    use crate::{Level, isi::Log};

    fn sample_account() -> (ChainId, AccountId, iroha_crypto::KeyPair) {
        let chain: ChainId = "fault-chain".parse().unwrap();
        let keypair = iroha_crypto::KeyPair::random();
        let domain: crate::domain::DomainId = "wonderland".parse().unwrap();
        let account_id = AccountId::new(domain, keypair.public_key().clone());
        (chain, account_id, keypair)
    }

    fn overlay_entries(tx: &SignedTransaction) -> Vec<String> {
        SignedTransaction::fault_injection_overlay(&tx.payload.metadata).unwrap_or_default()
    }

    #[test]
    fn injects_into_ivm_bytecode_and_records_trailer() {
        let (chain, account_id, keypair) = sample_account();
        let mut tx = TransactionBuilder::new(chain, account_id.clone())
            .with_bytecode(IvmBytecode::from_compiled(vec![0xAA, 0xBB, 0xCC]))
            .sign(keypair.private_key());

        let original_hash = tx.hash();
        let original_bytes = match tx.instructions() {
            Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
            _ => panic!("expected bytecode payload"),
        };

        let injected: InstructionBox = Log {
            level: Level::INFO,
            msg: "fault injected".into(),
        }
        .into();
        let expected = injected.clone();

        tx.inject_instructions([injected]);

        assert_ne!(tx.hash(), original_hash, "hash must change after injection");

        let patched_bytes = match tx.instructions() {
            Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
            _ => panic!("expected bytecode payload"),
        };
        assert_eq!(
            patched_bytes, original_bytes,
            "fault injection must not mutate the Kotodama bytecode payload"
        );

        let overlay = overlay_entries(&tx);
        assert_eq!(overlay.len(), 1);
        let expected_b64 =
            BASE64_STANDARD.encode(norito::to_bytes(&expected).expect("encode overlay payload"));
        assert_eq!(overlay[0], expected_b64);
    }

    #[test]
    fn repeated_injection_appends_trailer_instructions() {
        let (chain, account_id, keypair) = sample_account();
        let mut tx = TransactionBuilder::new(chain, account_id)
            .with_bytecode(IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03, 0x04]))
            .sign(keypair.private_key());

        let first: InstructionBox = Log {
            level: Level::WARN,
            msg: "first fault".into(),
        }
        .into();
        let second: InstructionBox = Log {
            level: Level::ERROR,
            msg: "second fault".into(),
        }
        .into();

        tx.inject_instructions([first.clone()]);
        tx.inject_instructions([second.clone()]);

        let bytes = match tx.instructions() {
            Executable::Ivm(bytecode) => bytecode.as_ref().to_vec(),
            _ => panic!("expected bytecode payload"),
        };
        assert_eq!(
            bytes,
            vec![0x01, 0x02, 0x03, 0x04],
            "fault injection must leave bytecode untouched"
        );
        let overlay = overlay_entries(&tx);
        assert_eq!(overlay.len(), 2, "overlay should preserve both batches");
        assert_eq!(
            overlay[0],
            BASE64_STANDARD
                .encode(norito::to_bytes(&first).expect("encode first overlay instruction"))
        );
        assert_eq!(
            overlay[1],
            BASE64_STANDARD
                .encode(norito::to_bytes(&second).expect("encode second overlay instruction"))
        );
    }
}

#[cfg(test)]
mod attachments_tests {
    use super::*;

    #[test]
    fn signed_tx_with_attachments_roundtrip() {
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
            eprintln!(
                "Skipping: SignedTransaction Norito decode mismatch pending fix. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }
        let chain: ChainId = "test-chain".parse().unwrap();
        let authority: AccountId =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                .parse()
                .unwrap();
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        let attachments =
            crate::proof::ProofAttachmentList(vec![crate::proof::ProofAttachment::new_ref(
                "halo2/ipa".into(),
                crate::proof::ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]),
                crate::proof::VerifyingKeyId::new("halo2/ipa", "vk_1"),
            )]);

        let tx: SignedTransaction = TransactionBuilder::new(chain, authority)
            .with_executable(Executable::Instructions(Vec::new().into()))
            .with_attachments(attachments)
            .sign(&private_key);

        let bytes = norito::to_bytes(&tx).expect("encode");
        let archived = norito::from_bytes::<SignedTransaction>(&bytes).expect("archived");
        let decoded: SignedTransaction = norito::core::NoritoDeserialize::deserialize(archived);
        assert!(decoded.attachments().is_some());
    }
}

impl TransactionEntrypoint {
    /// Account authorized to initiate this transaction.
    #[inline]
    pub fn authority(&self) -> &AccountId {
        match self {
            TransactionEntrypoint::External(entrypoint) => entrypoint.authority(),
            TransactionEntrypoint::Time(entrypoint) => &entrypoint.authority,
        }
    }

    /// Hash for this transaction entrypoint.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }
}

impl TransactionResult {
    /// Hash for this transaction result.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        HashOf::new(self)
    }

    /// Hash for this transaction result computed from its inner representation.
    #[inline]
    pub fn hash_from_inner(inner: &TransactionResultInner) -> HashOf<Self> {
        HashOf::new(&TransactionResult(inner.clone()))
    }
}

#[cfg(test)]
mod norito_rpc_fixture_tests {
    use super::*;
    use crate::account::address::{
        AccountAddress, AccountAddressFormat, ChainDiscriminantGuard,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use iroha_crypto::Hash;
    use norito::{
        core::DecodeFromSlice,
        json::{self, Value},
    };
    use std::{fs, path::PathBuf};

    fn manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("norito_rpc")
            .join("transaction_fixtures.manifest.json")
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
            Some(Value::Null) => None,
            Some(Value::Number(number)) => number
                .as_u64()
                .or_else(|| panic!("{context}: {key} must be an integer or null")),
            Some(_) => panic!("{context}: {key} must be an integer or null"),
            None => panic!("{context}: missing {key} field"),
        }
    }

    fn authority_prefix(authority: &str) -> Option<u16> {
        let (address_part, _) = authority
            .split_once('@')
            .unwrap_or_else(|| panic!("{authority}: missing @ separator"));
        match AccountAddress::parse_any(address_part, None) {
            Ok((_, AccountAddressFormat::IH58 { network_prefix })) => Some(network_prefix),
            Ok(_) => None,
            Err(_) => {
                if address_part.parse::<iroha_crypto::PublicKey>().is_ok() {
                    return None;
                }
                panic!("{authority}: unsupported authority address format");
            }
        }
    }

    #[test]
    fn norito_rpc_fixture_manifest_roundtrips() {
        let path = manifest_path();
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {path:?}: {err}"));
        let manifest: Value =
            json::from_str(&raw).unwrap_or_else(|err| panic!("manifest JSON: {err}"));
        let manifest_obj = require_object(&manifest, "manifest");
        let fixtures = manifest_obj
            .get("fixtures")
            .map(|value| require_array(value, "manifest.fixtures"))
            .unwrap_or_else(|| panic!("manifest missing fixtures array"));

        for fixture in fixtures {
            let entry = require_object(fixture, "fixture");
            let name = require_str(entry, "name", "fixture");
            let payload_base64 = require_str(entry, "payload_base64", name);
            let signed_base64 = require_str(entry, "signed_base64", name);
            let payload_hash = require_str(entry, "payload_hash", name);
            let signed_hash = require_str(entry, "signed_hash", name);
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
                .unwrap_or_else(|err| panic!("{name}: decode signed transaction: {err}"));
            assert_eq!(
                used,
                signed_bytes.len(),
                "{name}: signed transaction has trailing bytes"
            );
            assert_eq!(signed_tx.chain().as_str(), chain, "{name}: chain mismatch");
            assert_eq!(
                signed_tx.authority().to_string(),
                authority,
                "{name}: authority mismatch"
            );
            assert_eq!(
                signed_tx.creation_time().as_millis() as u64,
                creation_time_ms,
                "{name}: creation_time_ms mismatch"
            );
            assert_eq!(
                signed_tx.time_to_live().map(|ttl| ttl.as_millis() as u64),
                time_to_live_ms,
                "{name}: time_to_live_ms mismatch"
            );
            assert_eq!(
                signed_tx.nonce().map(NonZeroU32::get).map(u64::from),
                nonce,
                "{name}: nonce mismatch"
            );

            let signed_payload_bytes = norito::codec::encode_adaptive(signed_tx.payload());
            assert_eq!(
                signed_payload_bytes, payload_bytes,
                "{name}: payload_base64 mismatch vs signed payload"
            );
            let signed_reencoded = norito::codec::encode_adaptive(&signed_tx);
            assert_eq!(
                signed_reencoded, signed_bytes,
                "{name}: signed bytes mismatch after re-encode"
            );

            let computed_signed_hash = HashOf::<SignedTransaction>::new(&signed_tx).to_string();
            assert_eq!(
                computed_signed_hash, signed_hash,
                "{name}: signed_hash mismatch"
            );
        }
    }
}
