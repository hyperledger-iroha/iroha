use thiserror::Error;

/// Convenient alias for results produced by this crate.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Errors produced by the FASTPQ prover/verifier.
#[derive(Debug, Error)]
pub enum Error {
    /// The batch references a parameter set that does not exist.
    #[error("unknown FASTPQ parameter `{0}`")]
    UnknownParameter(String),
    /// Batch parameter does not match the prover configuration.
    #[error("parameter mismatch: expected `{expected}`, got `{actual}`")]
    ParameterMismatch {
        /// Expected parameter name.
        expected: String,
        /// Actual parameter requested in the batch/proof.
        actual: String,
    },
    /// Serialization failure while computing deterministic commitments.
    #[error("failed to encode batch: {0}")]
    Encode(#[from] norito::core::Error),
    /// The verifier recomputed a commitment that does not match the proof.
    #[error("trace commitment mismatch")]
    CommitmentMismatch,
    /// Protocol version advertised by the proof is unsupported.
    #[error("unsupported FASTPQ protocol version {version}")]
    UnsupportedProtocolVersion {
        /// Version advertised by the proof.
        version: u16,
    },
    /// Parameter catalogue version does not match the canonical table.
    #[error("parameter `{parameter}` expects version {expected}, proof advertised {actual}")]
    ParameterVersionMismatch {
        /// Parameter set name reported by the proof.
        parameter: String,
        /// Version derived from the canonical table.
        expected: u16,
        /// Version advertised by the proof artifact.
        actual: u16,
    },
    /// Ordering hash mismatch detected during verification.
    #[error("ordering hash mismatch")]
    OrderingHashMismatch,
    /// Permission lookup hashes do not match the reconstructed witness.
    #[error("permission lookup hashes mismatch")]
    PermissionHashMismatch,
    /// Trace Merkle root mismatch detected during verification.
    #[error("trace root mismatch")]
    TraceRootMismatch,
    /// Lookup Merkle root mismatch detected during verification.
    #[error("lookup root mismatch")]
    LookupRootMismatch,
    /// Lookup Fiat–Shamir challenge (`γ`) mismatch.
    #[error("lookup challenge mismatch")]
    LookupChallengeMismatch,
    /// Lookup grand-product does not match the recomputed accumulator.
    #[error("lookup grand product mismatch")]
    LookupGrandProductMismatch,
    /// FRI layer vector length differs from the recomputed transcript.
    #[error("FRI layer length mismatch: expected {expected}, got {actual}")]
    FriLayerLengthMismatch {
        /// Expected number of layers.
        expected: usize,
        /// Actual number advertised by the proof.
        actual: usize,
    },
    /// Specific FRI layer root mismatch.
    #[error("FRI layer mismatch at round {round}")]
    FriLayerMismatch {
        /// Round exhibiting the mismatch.
        round: usize,
    },
    /// FRI challenge vector length mismatch.
    #[error("FRI challenge length mismatch: expected {expected}, got {actual}")]
    FriChallengeLengthMismatch {
        /// Expected challenge count.
        expected: usize,
        /// Actual challenge count in the proof.
        actual: usize,
    },
    /// Specific FRI folding challenge mismatch.
    #[error("FRI challenge mismatch at round {round}")]
    FriChallengeMismatch {
        /// Round exhibiting the mismatch.
        round: usize,
    },
    /// Query count mismatch between proof and verifier transcript.
    #[error("query count mismatch: expected {expected}, got {actual}")]
    QueryCountMismatch {
        /// Expected number of queries.
        expected: usize,
        /// Actual number of queries in the proof.
        actual: usize,
    },
    /// Query index/value mismatch at a specific position.
    #[error("query mismatch at position {index}")]
    QueryMismatch {
        /// Position of the failing query.
        index: usize,
    },
    /// Trace length exceeded the supported 32-bit representation.
    #[error("trace length {rows} exceeds 32-bit bound")]
    TraceLengthOverflow {
        /// Number of rows encountered during proving.
        rows: usize,
    },
    /// Query index exceeded the 32-bit representation limit.
    #[error("query index {index} exceeds 32-bit bound")]
    QueryIndexOverflow {
        /// Index sampled by the transcript that cannot be represented.
        index: usize,
    },
    /// Query index fell outside the evaluation domain.
    #[error("query index {index} out of bounds (len {len})")]
    QueryIndexOutOfRange {
        /// Index sampled by the transcript.
        index: usize,
        /// Length of the evaluation domain.
        len: usize,
    },
    /// Internal payload length exceeded the supported 64-bit representation.
    #[error("payload length {length} exceeds 64-bit bound")]
    PayloadLengthOverflow {
        /// Number of bytes in the payload.
        length: usize,
    },
    /// Required trace column is missing from the canonical layout.
    #[error("trace column `{0}` missing from layout")]
    MissingColumn(String),
    /// Unsupported FRI arity advertised by the parameter set.
    #[error("unsupported FRI arity {0}; expected 8 or 16")]
    FriArity(u32),
    /// Value exceeds the supported width for the stage 1 trace encoding.
    #[error("value limb width `{length}` exceeds 64-bit limit")]
    ValueWidth {
        /// Number of bytes encountered in the value representation.
        length: usize,
    },
    /// Role identifiers must be fixed 32-byte little-endian strings.
    #[error("invalid role identifier length {length}; expected 32 bytes")]
    InvalidRoleIdLength {
        /// Observed number of bytes.
        length: usize,
    },
    /// Permission identifiers must be fixed 32-byte little-endian strings.
    #[error("invalid permission identifier length {length}; expected 32 bytes")]
    InvalidPermissionIdLength {
        /// Observed number of bytes.
        length: usize,
    },
    /// Metadata field has an unexpected length.
    #[error("metadata field `{key}` has length {actual}, expected {expected}")]
    MetadataLength {
        /// Field name.
        key: String,
        /// Expected byte length.
        expected: usize,
        /// Actual byte length encountered.
        actual: usize,
    },
    /// Mandatory metadata field is missing from the batch manifest.
    #[error("metadata field `{key}` is required")]
    MissingMetadata {
        /// Field name.
        key: String,
    },
    /// FASTPQ transfer gadget metadata could not be decoded.
    #[error("failed to decode transfer gadget metadata: {source}")]
    TransferMetadataDecode {
        /// Underlying Norito error.
        #[source]
        source: norito::core::Error,
    },
    /// Transfer gadget Merkle proof payload could not be decoded.
    #[error("failed to decode transfer merkle proof: {source}")]
    TransferProofDecode {
        /// Underlying Norito error.
        #[source]
        source: norito::core::Error,
    },
    /// Transfer gadget Merkle proof advertised an unsupported version.
    #[error("unsupported transfer merkle proof version {version}")]
    TransferProofUnsupportedVersion {
        /// Version identifier contained in the proof payload.
        version: u16,
    },
    /// Numeric value referenced by the transfer gadget exceeds the supported width.
    #[error("transfer gadget numeric `{field}` exceeds 64-bit bounds")]
    TransferNumericBounds {
        /// Field reporting the overflow.
        field: &'static str,
    },
    /// Transfer gadget invariant was violated while validating transcripts.
    #[error("transfer gadget invariant violated: {details}")]
    TransferInvariant {
        /// Human-readable description of the violation.
        details: String,
    },
    /// Structured AXT FASTPQ binding was malformed.
    #[error("invalid AXT FASTPQ binding: {details}")]
    InvalidAxtBinding {
        /// Human-readable description of the violation.
        details: String,
    },
}
