//! Typed failures shared by strict X.509 preparation and the complete reference relation.
//!
//! Presentation preflight returns only parsing and interval failures. A successful
//! interval result establishes none of the relation's signature or authority checks.
use super::merkle::ZkX509MerkleErrorV1;
use thiserror::Error;
/// ASN.1 tag class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkX509DerClassV1 {
    /// Universal ASN.1 tag.
    Universal,
    /// Application-specific tag.
    Application,
    /// Context-specific tag.
    ContextSpecific,
    /// Private tag.
    Private,
}
/// Canonically decoded DER identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[doc(hidden)]
pub struct ZkX509DerTagV1 {
    /// ASN.1 class.
    pub(crate) class: ZkX509DerClassV1,
    /// Whether the value uses constructed form.
    pub(crate) constructed: bool,
    /// Decoded tag number.
    pub(crate) number: u32,
}
impl ZkX509DerTagV1 {
    /// Universal BOOLEAN.
    pub(crate) const BOOLEAN: Self = Self::universal(false, 1);
    /// Universal INTEGER.
    pub(crate) const INTEGER: Self = Self::universal(false, 2);
    /// Universal BIT STRING.
    pub(crate) const BIT_STRING: Self = Self::universal(false, 3);
    /// Universal OCTET STRING.
    pub(crate) const OCTET_STRING: Self = Self::universal(false, 4);
    /// Universal OBJECT IDENTIFIER.
    pub(crate) const OBJECT_IDENTIFIER: Self = Self::universal(false, 6);
    /// Universal SEQUENCE.
    pub(crate) const SEQUENCE: Self = Self::universal(true, 16);
    /// Universal SET.
    pub(crate) const SET: Self = Self::universal(true, 17);
    const fn universal(constructed: bool, number: u32) -> Self {
        Self {
            class: ZkX509DerClassV1::Universal,
            constructed,
            number,
        }
    }
}
/// Exact failure returned by the bounded DER parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[doc(hidden)]
pub enum ZkX509DerErrorV1 {
    /// A DER document must contain one value.
    #[error("zk-X509 DER document is empty")]
    EmptyInput,
    /// The complete document exceeds its fixed byte limit.
    #[error("zk-X509 DER document has {actual} bytes; maximum is {max}")]
    InputTooLarge {
        /// Observed input bytes.
        actual: usize,
        /// Maximum input bytes.
        max: usize,
    },
    /// One DER value exceeds its fixed content limit.
    #[error("zk-X509 DER value has {actual} content bytes; maximum is {max}")]
    ValueTooLarge {
        /// Declared content bytes.
        actual: usize,
        /// Maximum content bytes.
        max: usize,
    },
    /// The recursive value count exceeds its fixed limit.
    #[error("zk-X509 DER document exceeds its {max}-value limit")]
    TooManyValues {
        /// Maximum values.
        max: usize,
    },
    /// Constructed values exceed the fixed nesting limit.
    #[error("zk-X509 DER nesting depth {depth} exceeds maximum {max}")]
    NestingTooDeep {
        /// Rejected depth.
        depth: usize,
        /// Maximum depth.
        max: usize,
    },
    /// A high-tag-number identifier ended before its terminating octet.
    #[error("zk-X509 DER identifier is truncated")]
    TruncatedIdentifier,
    /// A high-tag-number identifier uses a leading zero group or encodes a low tag.
    #[error("zk-X509 DER identifier uses a non-minimal tag encoding")]
    NonMinimalTag,
    /// A decoded high tag cannot fit the fixed tag-number representation.
    #[error("zk-X509 DER tag number overflows u32")]
    TagNumberOverflow,
    /// End-of-contents is forbidden in DER.
    #[error("zk-X509 DER forbids the end-of-contents tag")]
    EndOfContentsForbidden,
    /// The closed X.509 primitive layer does not admit this universal tag.
    #[error("zk-X509 DER universal tag {number} is unsupported")]
    UnsupportedUniversalTag {
        /// Rejected universal tag number.
        number: u32,
    },
    /// A universal value uses the wrong primitive/constructed form.
    #[error(
        "zk-X509 DER universal tag {number} constructed={actual_constructed}; expected constructed={expected_constructed}"
    )]
    InvalidUniversalTagForm {
        /// Universal tag number.
        number: u32,
        /// Required form.
        expected_constructed: bool,
        /// Observed form.
        actual_constructed: bool,
    },
    /// A value is missing its first length octet or long-form length body.
    #[error("zk-X509 DER length is truncated")]
    TruncatedLength,
    /// BER indefinite length is forbidden.
    #[error("zk-X509 DER forbids indefinite length")]
    IndefiniteLength,
    /// A length cannot fit the platform-independent parser representation.
    #[error("zk-X509 DER length overflows usize")]
    LengthOverflow,
    /// Long-form length has a leading zero or encodes a short-form value.
    #[error("zk-X509 DER length is not minimally encoded")]
    NonMinimalLength,
    /// The declared value extends past the available bytes.
    #[error("zk-X509 DER value declares {declared} bytes but only {remaining} remain")]
    TruncatedValue {
        /// Declared content bytes.
        declared: usize,
        /// Available content bytes.
        remaining: usize,
    },
    /// Bytes remain after the sole top-level value.
    #[error("zk-X509 DER document has {bytes} trailing bytes")]
    TrailingData {
        /// Unconsumed bytes.
        bytes: usize,
    },
    /// A required child value is absent.
    #[error("zk-X509 DER container ended before a required value")]
    UnexpectedEndOfContainer,
    /// A typed parser encountered a different tag.
    #[error("zk-X509 DER expected tag {expected:?}, found {actual:?}")]
    UnexpectedTag {
        /// Required tag.
        expected: ZkX509DerTagV1,
        /// Observed tag.
        actual: ZkX509DerTagV1,
    },
    /// A caller requested children from a primitive value.
    #[error("zk-X509 DER value is not constructed")]
    ExpectedConstructedValue,
    /// DER `SET OF` children are not lexicographically ordered.
    #[error("zk-X509 DER SET OF elements are not in canonical order")]
    SetElementsOutOfOrder,
    /// A BOOLEAN is not exactly `00` or `ff`.
    #[error("zk-X509 DER BOOLEAN is not canonical")]
    InvalidBoolean,
    /// NULL has non-empty content.
    #[error("zk-X509 DER NULL must have empty content")]
    InvalidNull,
    /// INTEGER has no content octets.
    #[error("zk-X509 DER INTEGER is empty")]
    EmptyInteger,
    /// INTEGER has an unnecessary sign-extension octet.
    #[error("zk-X509 DER INTEGER is not minimally encoded")]
    NonMinimalInteger,
    /// A positive-only INTEGER is negative.
    #[error("zk-X509 DER INTEGER must be non-negative")]
    NegativeInteger,
    /// A positive-only INTEGER is zero.
    #[error("zk-X509 DER INTEGER must be non-zero")]
    ZeroInteger,
    /// A positive INTEGER exceeds its caller-provided unsigned width.
    #[error("zk-X509 DER INTEGER has {actual} unsigned bytes; maximum is {max}")]
    IntegerTooLarge {
        /// Observed unsigned bytes.
        actual: usize,
        /// Maximum unsigned bytes.
        max: usize,
    },
    /// OBJECT IDENTIFIER has no subidentifier.
    #[error("zk-X509 DER OBJECT IDENTIFIER is empty")]
    EmptyObjectIdentifier,
    /// An OBJECT IDENTIFIER subidentifier has a leading zero base-128 group.
    #[error("zk-X509 DER OBJECT IDENTIFIER has a non-minimal subidentifier")]
    NonMinimalObjectIdentifier,
    /// The last OBJECT IDENTIFIER subidentifier has no terminating octet.
    #[error("zk-X509 DER OBJECT IDENTIFIER is truncated")]
    TruncatedObjectIdentifier,
    /// BIT STRING omits its unused-bit count.
    #[error("zk-X509 DER BIT STRING is empty")]
    EmptyBitString,
    /// BIT STRING's unused-bit count is outside 0..=7.
    #[error("zk-X509 DER BIT STRING unused-bit count {unused_bits} exceeds seven")]
    InvalidUnusedBitCount {
        /// Rejected unused-bit count.
        unused_bits: u8,
    },
    /// An empty BIT STRING claims unused bits.
    #[error("zk-X509 DER empty BIT STRING must have zero unused bits")]
    UnusedBitsWithoutPayload,
    /// Bits declared unused are not zero.
    #[error("zk-X509 DER BIT STRING has non-zero unused bits")]
    NonZeroUnusedBits,
    /// `AlgorithmIdentifier` omits its algorithm OID.
    #[error("zk-X509 AlgorithmIdentifier omits its algorithm OID")]
    MissingAlgorithmObjectIdentifier,
    /// The signature `AlgorithmIdentifier` is not exactly ECDSA with SHA-256.
    #[error("zk-X509 signature AlgorithmIdentifier is not ecdsa-with-SHA256")]
    UnsupportedSignatureAlgorithm,
    /// ECDSA-with-SHA256 parameters must be absent, not NULL.
    #[error("zk-X509 ecdsa-with-SHA256 AlgorithmIdentifier parameters must be absent")]
    ForbiddenSignatureAlgorithmParameters,
    /// The subject-public-key algorithm is not `id-ecPublicKey`.
    #[error("zk-X509 subject-public-key AlgorithmIdentifier is not id-ecPublicKey")]
    UnsupportedPublicKeyAlgorithm,
    /// The subject-public-key `AlgorithmIdentifier` omits its named curve.
    #[error("zk-X509 id-ecPublicKey AlgorithmIdentifier omits its named curve")]
    MissingPublicKeyAlgorithmParameters,
    /// The named curve is not exactly `prime256v1`.
    #[error("zk-X509 subject-public-key AlgorithmIdentifier is not prime256v1")]
    UnsupportedNamedCurve,
    /// `AlgorithmIdentifier` contains fields beyond the exact profile shape.
    #[error("zk-X509 AlgorithmIdentifier contains unexpected trailing fields")]
    UnexpectedAlgorithmIdentifierFields,
}
/// Failure of the strict native reference relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[doc(hidden)]
pub enum ZkX509RelationErrorV1 {
    /// A governed record is malformed, inactive, or unrelated to the statement.
    #[error("zk-X509 authoritative governance binding is invalid")]
    InvalidGovernance,
    /// Public statement lengths, order, epochs, predicates, or roots mismatch.
    #[error("zk-X509 public statement does not match the relation inputs")]
    StatementMismatch,
    /// The private witness shape does not match the statement.
    #[error("zk-X509 private witness shape does not match the statement")]
    WitnessMismatch,
    /// The authoritative DER parser rejected a certificate or CRL.
    #[error("zk-X509 strict DER validation failed: {0}")]
    Der(#[from] ZkX509DerErrorV1),
    /// A certificate does not have the exact closed v3 wire structure.
    #[error("zk-X509 certificate structure is outside the closed profile")]
    InvalidCertificateStructure,
    /// A distinguished name uses an unsupported attribute, value, or duplicate.
    #[error("zk-X509 distinguished name is outside the closed profile")]
    InvalidName,
    /// A time is not the exact RFC 5280 UTC encoding or is out of range.
    #[error("zk-X509 time is not canonical or is out of range")]
    InvalidTime,
    /// An extension is missing, duplicated, unknown, malformed, or has wrong criticality.
    #[error("zk-X509 certificate or CRL extension set is invalid")]
    InvalidExtensions,
    /// A serial or nonnegative integer is malformed or out of range.
    #[error("zk-X509 integer is outside the closed unsigned range")]
    InvalidInteger,
    /// A subject public key is not the exact canonical uncompressed P-256 form.
    #[error("zk-X509 subject public key is invalid")]
    InvalidPublicKey,
    /// An ECDSA signature is not a strict DER P-256 signature.
    #[error("zk-X509 ECDSA signature encoding is invalid")]
    InvalidSignatureEncoding,
    /// A certificate signature is invalid for its exact parent.
    #[error("zk-X509 certificate signature verification failed")]
    InvalidCertificateSignature,
    /// Certificate names, AKI/SKI, constraints, usages, or depth do not form the chain.
    #[error("zk-X509 certification path validation failed")]
    InvalidCertificatePath,
    /// One certificate is invalid at the trusted statement time.
    #[error("zk-X509 certificate validity predicate failed")]
    CertificateNotValid,
    /// The complete CRL is malformed, stale, incorrectly signed, or unrelated.
    #[error("zk-X509 complete signed CRL validation failed")]
    InvalidCrl,
    /// The leaf serial occurs in the complete governed CRL.
    #[error("zk-X509 leaf certificate is revoked")]
    CertificateRevoked,
    /// A compact CA-tree computation or membership check failed.
    #[error("zk-X509 compact CA membership validation failed: {0}")]
    Merkle(#[from] ZkX509MerkleErrorV1),
    /// A deterministic public digest or disclosure does not match the statement.
    #[error("zk-X509 public projection mismatch")]
    ProjectionMismatch,
    /// Canonical statement encoding failed.
    #[error("zk-X509 canonical statement encoding failed")]
    StatementEncoding,
    /// The fresh wallet-ownership signature is invalid or not low-s.
    #[error("zk-X509 wallet ownership proof failed")]
    InvalidWalletOwnership,
}
