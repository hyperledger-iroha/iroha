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
    /// A claimed `PublicIO` field does not match the batch reconstructed by the verifier.
    #[error("public_io field `{field}` mismatch")]
    PublicIoMismatch {
        /// Name of the mismatched field.
        field: &'static str,
    },
    /// Ordering hash mismatch detected during verification.
    #[error("ordering hash mismatch")]
    OrderingHashMismatch,
    /// Trace Merkle root mismatch detected during verification.
    #[error("trace root mismatch")]
    TraceRootMismatch,
    /// Low-degree extension Merkle root mismatch detected during verification.
    #[error("LDE root mismatch")]
    LdeRootMismatch,
    /// AIR trace Merkle root mismatch detected during verification.
    #[error("AIR trace root mismatch")]
    AirTraceRootMismatch,
    /// AIR composition Merkle root mismatch detected during verification.
    #[error("AIR composition root mismatch")]
    AirCompositionRootMismatch,
    /// AIR opening count mismatch between proof and verifier transcript.
    #[error("AIR opening count mismatch: expected {expected}, got {actual}")]
    AirOpeningCountMismatch {
        /// Expected number of openings.
        expected: usize,
        /// Actual number of openings in the proof.
        actual: usize,
    },
    /// AIR composition challenge count does not match the V1 transcript shape.
    #[error("AIR challenge count mismatch: expected {expected}, got {actual}")]
    AirChallengeCountMismatch {
        /// Expected number of transcript challenges.
        expected: usize,
        /// Actual number advertised by the proof.
        actual: usize,
    },
    /// A proof-carried AIR composition challenge differs from transcript replay.
    #[error("AIR challenge mismatch at index {index}")]
    AirChallengeMismatch {
        /// Index of the mismatched AIR composition challenge.
        index: usize,
    },
    /// AIR row or composition opening did not match the sampled statement.
    #[error("AIR opening mismatch at position {index}")]
    AirOpeningMismatch {
        /// Position of the failing query.
        index: usize,
    },
    /// AIR Merkle authentication path did not resolve to the advertised root.
    #[error("AIR Merkle path mismatch at position {index}")]
    AirMerklePathMismatch {
        /// Position of the failing query.
        index: usize,
    },
    /// AIR sampled constraint did not evaluate to zero.
    #[error("AIR constraint mismatch at position {index}")]
    AirConstraintMismatch {
        /// Position of the failing query.
        index: usize,
    },
    /// FRI layer vector length differs from the recomputed transcript.
    #[error("FRI layer length mismatch: expected {expected}, got {actual}")]
    FriLayerLengthMismatch {
        /// Expected number of layers.
        expected: usize,
        /// Actual number advertised by the proof.
        actual: usize,
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
    /// The configured reduction limit cannot expose a complete terminal FRI layer.
    #[error(
        "FRI reduction limit {max_reductions} leaves {remaining} values; terminal layer must contain at most {arity}"
    )]
    FriReductionLimit {
        /// Maximum reductions admitted by the parameter set.
        max_reductions: u32,
        /// Values remaining after exhausting the reduction limit.
        remaining: usize,
        /// Maximum complete terminal layer size.
        arity: usize,
    },
    /// The claimed terminal degree bound is not smaller than its evaluation domain.
    #[error(
        "FRI terminal degree bound {degree_bound} is not smaller than terminal domain length {domain_len}"
    )]
    FriTerminalDegreeBound {
        /// Exclusive polynomial degree bound after all reductions.
        degree_bound: usize,
        /// Number of terminal-domain evaluations.
        domain_len: usize,
    },
    /// Authenticated terminal evaluations do not satisfy the claimed degree bound.
    #[error("FRI terminal polynomial is not below exclusive degree bound {degree_bound}")]
    FriTerminalDegreeMismatch {
        /// Exclusive polynomial degree bound after all reductions.
        degree_bound: usize,
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
    /// Query Merkle authentication path did not resolve to the LDE root.
    #[error("query Merkle path mismatch at position {index}")]
    QueryMerklePathMismatch {
        /// Position of the failing query.
        index: usize,
    },
    /// A proof-carried Goldilocks element did not use its unique canonical representation.
    #[error("non-canonical Goldilocks field element in `{context}` at nested indices {indices:?}")]
    NonCanonicalGoldilocksElement {
        /// Stable proof-field name identifying the rejected value.
        context: &'static str,
        /// Outer-to-inner vector indices locating the rejected value within that field.
        indices: Vec<usize>,
    },
    /// FASTPQ verifier input exceeded a configured limit.
    #[error("FASTPQ verifier limit `{limit}` exceeded: {actual} > {max}")]
    VerifierLimitExceeded {
        /// Name of the rejected limit.
        limit: &'static str,
        /// Observed value.
        actual: usize,
        /// Maximum permitted value.
        max: usize,
    },
    /// Trace length exceeded the supported 32-bit representation.
    #[error("trace length {rows} exceeds 32-bit bound")]
    TraceLengthOverflow {
        /// Number of rows encountered during proving.
        rows: usize,
    },
    /// The padded trace does not fit the selected parameter set's trace domain.
    #[error(
        "trace requires {padded_rows} padded rows for {rows} transitions, exceeding parameter capacity {max_rows}"
    )]
    TraceDomainCapacityExceeded {
        /// Number of transition rows supplied by the caller.
        rows: usize,
        /// Power-of-two trace length required after mandatory padding.
        padded_rows: usize,
        /// Maximum trace length admitted by the selected parameter set.
        max_rows: usize,
    },
    /// A caller-supplied trace does not use the canonical padded column shape.
    #[error("invalid FASTPQ trace shape: {details}")]
    InvalidTraceShape {
        /// Human-readable description of the malformed shape.
        details: String,
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
    #[error("unsupported FRI arity {0}; first-release FASTPQ requires binary arity 2")]
    FriArity(u32),
    /// A FRI layer cannot be partitioned into complete multiplicative cosets.
    #[error("FRI layer length {length} is not compatible with effective arity {arity}")]
    FriDomainSize {
        /// Number of evaluations in the malformed layer.
        length: usize,
        /// Effective arity required for this reduction.
        arity: usize,
    },
    /// Value exceeds the supported width for the V1 trace encoding.
    #[error("value limb width `{length}` exceeds 64-bit limit")]
    ValueWidth {
        /// Number of bytes encountered in the value representation.
        length: usize,
    },
    /// A numeric asset value did not use the canonical fixed-width encoding.
    #[error("invalid asset value length {length}; expected exactly 8 little-endian bytes")]
    InvalidAssetValueLength {
        /// Number of bytes in the rejected value.
        length: usize,
    },
    /// A numeric asset operation did not use the canonical state-key shape.
    #[error("invalid asset operation key; expected `asset/<asset-id>/<account>`")]
    InvalidAssetKey,
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
    /// AXT `FastPQ` proof payload could not be decoded.
    #[error("failed to decode AXT FastPQ proof payload: {source}")]
    AxtProofPayloadDecode {
        /// Underlying Norito error.
        #[source]
        source: norito::core::Error,
    },
    /// Numeric value referenced by the transfer gadget cannot be normalized into witness units.
    #[error("transfer gadget numeric `{field}` cannot be normalized into 64-bit witness units")]
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
    /// A transition batch is not valid for the explicitly selected proof semantics.
    #[error("invalid FASTPQ `{profile}` proof semantics: {details}")]
    InvalidProofSemantics {
        /// Stable name of the selected proof semantics profile.
        profile: &'static str,
        /// Human-readable description of the rejected batch shape.
        details: String,
    },
    /// Structured AXT FASTPQ binding was malformed.
    #[error("invalid AXT FASTPQ binding: {details}")]
    InvalidAxtBinding {
        /// Human-readable description of the violation.
        details: String,
    },
}
