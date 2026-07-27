//! Native reference relation for the closed first-release zk-X509 profile.
//!
//! This module is the non-algebraic specification against which AIR witness
//! execution must be differentially tested.  It does not delegate path
//! validation to a platform trust API: the exact DER grammar, extension
//! allow-list, name linking, validity arithmetic, P-256 signatures, complete
//! CRL, sparse roots, projections, and holder ownership are checked here.
//!
//! The reference relation is not itself a privacy proof.  Consensus activation
//! remains gated until a purpose-built AIR proves the same predicates without
//! exposing this private witness.

use iroha_data_model::privacy::{
    IrohaZkX509StarkP256StatementV1, PrivacyAttributeDigestV1, PrivacyCertificateKeyDigestV1,
    PrivacyNullifierV1, PrivacyStatementV1, PrivacyX509CrlDerDigestV1,
    PrivacyX509CrlIssuerSpkiDigestV1, PrivacyX509ExtendedKeyUsageV1, PrivacyX509KeyUsageV1,
    PrivacyZkX509CertificatePolicyRecordV1, PrivacyZkX509CrlRecordV1,
    PrivacyZkX509RecordLifecycleV1, PrivacyZkX509TrustAnchorRecordV1,
};
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey,
    signature::{Verifier as _, hazmat::PrehashVerifier as _},
};
use thiserror::Error;
use time::{Date, Month, PrimitiveDateTime, Time};

use super::{
    codec::ZkX509WitnessV1,
    der::{
        ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1,
        ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1, ZkX509DerClassV1, ZkX509DerErrorV1,
        ZkX509DerLimitsV1, ZkX509DerTagV1, ZkX509DerValueV1, parse_single_der_value_v1,
        validate_ecdsa_with_sha256_algorithm_identifier_v1,
        validate_p256_public_key_algorithm_identifier_v1,
    },
    merkle::{
        ZkX509MerkleErrorV1, crl_root_from_complete_serials_v1, hash_frame_v1,
        verify_ca_membership_v1,
    },
    profile::{
        ZK_X509_ATTRIBUTE_DOMAIN_V1, ZK_X509_CLIENT_AUTHENTICATION_EKU_OID_V1,
        ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1, ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1,
        ZK_X509_MAX_CRL_AGE_SECONDS_V1, ZK_X509_MAX_CRL_ENTRIES_V1, ZK_X509_MAX_SERIAL_BYTES_V1,
        ZK_X509_NULLIFIER_DOMAIN_V1, ZK_X509_OWNERSHIP_DOMAIN_V1, ZK_X509_RELATION_VERSION_V1,
        ZK_X509_SCOPED_KEY_DOMAIN_V1, ZK_X509_SOURCE_PROFILE_V1, ZK_X509_SUITE_V1,
        ZK_X509_UNCOMPRESSED_P256_BYTES_V1, ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1,
    },
};

const OID_AUTHORITY_KEY_IDENTIFIER: &[u8] = &[0x55, 0x1d, 0x23];
const OID_SUBJECT_KEY_IDENTIFIER: &[u8] = &[0x55, 0x1d, 0x0e];
const OID_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x0f];
const OID_BASIC_CONSTRAINTS: &[u8] = &[0x55, 0x1d, 0x13];
const OID_EXTENDED_KEY_USAGE: &[u8] = &[0x55, 0x1d, 0x25];
const OID_CRL_NUMBER: &[u8] = &[0x55, 0x1d, 0x14];

const OID_COUNTRY_NAME: &[u8] = &[0x55, 0x04, 0x06];
const OID_ORGANIZATION_NAME: &[u8] = &[0x55, 0x04, 0x0a];
const OID_ORGANIZATIONAL_UNIT_NAME: &[u8] = &[0x55, 0x04, 0x0b];
const OID_COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];

const OID_CLIENT_AUTHENTICATION: &[u8] = &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x02];

const KEY_USAGE_DIGITAL_SIGNATURE: u16 = 1 << 0;
const KEY_USAGE_CONTENT_COMMITMENT: u16 = 1 << 1;
const KEY_USAGE_KEY_ENCIPHERMENT: u16 = 1 << 2;
const KEY_USAGE_KEY_AGREEMENT: u16 = 1 << 4;
const KEY_USAGE_KEY_CERT_SIGN: u16 = 1 << 5;
const KEY_USAGE_CRL_SIGN: u16 = 1 << 6;

/// Public governance objects selected by the statement and supplied by state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ZkX509GovernanceV1<'a> {
    /// Exact active trust-anchor revision.
    pub(crate) trust_anchor: &'a PrivacyZkX509TrustAnchorRecordV1,
    /// Exact active leaf certificate-policy revision.
    pub(crate) certificate_policy: &'a PrivacyZkX509CertificatePolicyRecordV1,
    /// Exact active signed-CRL revision.
    pub(crate) crl: &'a PrivacyZkX509CrlRecordV1,
}

/// Deterministic public projection obtained after a successful relation check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509RelationOutputV1 {
    /// Recomputed scoped subject-key commitment.
    pub(crate) subject_public_key_digest: PrivacyCertificateKeyDigestV1,
    /// Recomputed deterministic certificate nullifier.
    pub(crate) certificate_nullifier: PrivacyNullifierV1,
    /// Challenge digest signed by the certificate subject key.
    pub(crate) ownership_challenge_digest: [u8; 32],
    /// Root reconstructed from every entry in the complete signed CRL.
    pub(crate) complete_crl_root: [u8; 32],
}

/// Failure of the strict native reference relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509RelationErrorV1 {
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
    /// A sparse-tree computation or membership check failed.
    #[error("zk-X509 accumulator validation failed: {0}")]
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

/// Execute the complete strict native relation over one public statement and
/// private witness.
///
/// This function is deterministic and side-effect free.  Its successful
/// output is suitable for native/AIR differential tests, but it must never be
/// treated as a substitute for proof verification.
pub(crate) fn validate_reference_relation_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    governance: ZkX509GovernanceV1<'_>,
    witness: &ZkX509WitnessV1,
) -> Result<ZkX509RelationOutputV1, ZkX509RelationErrorV1> {
    validate_governance_binding_v1(statement, governance)?;
    validate_witness_statement_shape_v1(statement, witness)?;

    let mut chain = Vec::with_capacity(witness.certificate_chain_der.len());
    for certificate in &witness.certificate_chain_der {
        chain.push(parse_certificate_v1(certificate)?);
    }
    validate_certificate_path_v1(statement, &chain)?;

    let root = chain.last().ok_or(ZkX509RelationErrorV1::WitnessMismatch)?;
    verify_ca_membership_v1(
        *statement.ca_membership_root.as_bytes(),
        root.spki_der,
        &witness.ca_membership_path,
    )?;

    let parsed_crl = parse_crl_v1(&witness.crl_der)?;
    let complete_crl_root = validate_complete_crl_v1(
        statement,
        governance.crl,
        &chain,
        &parsed_crl,
        &witness.crl_der,
    )?;

    let subject_public_key_digest = derive_subject_public_key_digest_v1(statement, &chain)?;
    if subject_public_key_digest != statement.subject_public_key_digest {
        return Err(ZkX509RelationErrorV1::ProjectionMismatch);
    }
    let certificate_nullifier =
        derive_certificate_nullifier_v1(statement, chain[1].spki_der, chain[0].serial)?;
    if certificate_nullifier != statement.certificate_nullifier {
        return Err(ZkX509RelationErrorV1::ProjectionMismatch);
    }
    validate_attribute_projections_v1(statement, &chain[0], witness)?;

    let ownership_challenge_digest = derive_ownership_challenge_digest_v1(statement)?;
    verify_p256_prehash_signature_v1(
        chain[0].public_key,
        &ownership_challenge_digest,
        &witness.wallet_ownership_signature_der,
    )?;

    Ok(ZkX509RelationOutputV1 {
        subject_public_key_digest,
        certificate_nullifier,
        ownership_challenge_digest,
        complete_crl_root,
    })
}

fn validate_governance_binding_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    governance: ZkX509GovernanceV1<'_>,
) -> Result<(), ZkX509RelationErrorV1> {
    governance
        .trust_anchor
        .validate()
        .map_err(|_| ZkX509RelationErrorV1::InvalidGovernance)?;
    governance
        .certificate_policy
        .validate()
        .map_err(|_| ZkX509RelationErrorV1::InvalidGovernance)?;
    governance
        .crl
        .validate()
        .map_err(|_| ZkX509RelationErrorV1::InvalidGovernance)?;
    if governance.trust_anchor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
        || governance.certificate_policy.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
        || governance.crl.lifecycle != PrivacyZkX509RecordLifecycleV1::Active
    {
        return Err(ZkX509RelationErrorV1::InvalidGovernance);
    }

    let policy = governance.certificate_policy;
    let crl = governance.crl;
    if statement.trust_anchor_id != governance.trust_anchor.trust_anchor_id
        || statement.trust_anchor_record_digest != governance.trust_anchor.record_digest
        || statement.trust_anchor_record_epoch != governance.trust_anchor.record_epoch
        || statement.ca_membership_root != governance.trust_anchor.ca_membership_root
        || statement.ca_membership_root_epoch != governance.trust_anchor.ca_membership_root_epoch
        || statement.certificate_policy_id != policy.policy_id
        || statement.certificate_policy_record_digest != policy.record_digest
        || statement.certificate_policy_record_epoch != policy.record_epoch
        || policy.trust_anchor_id != statement.trust_anchor_id
        || statement.key_usage != policy.required_key_usage
        || statement.extended_key_usages != policy.required_extended_key_usages
        || statement.crl_record_digest != crl.record_digest
        || statement.crl_record_epoch != crl.record_epoch
        || crl.trust_anchor_id != statement.trust_anchor_id
        || crl.certificate_policy_id != statement.certificate_policy_id
        || statement.crl_nonmembership_root != crl.revoked_serials_root
        || statement.crl_nonmembership_root_epoch != crl.root_epoch
        || statement.disclosed_attributes.len() != policy.required_disclosed_attribute_indices.len()
        || statement
            .disclosed_attributes
            .iter()
            .zip(&policy.required_disclosed_attribute_indices)
            .any(|(disclosed, required)| disclosed.index != *required)
    {
        return Err(ZkX509RelationErrorV1::StatementMismatch);
    }
    Ok(())
}

fn validate_witness_statement_shape_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    witness: &ZkX509WitnessV1,
) -> Result<(), ZkX509RelationErrorV1> {
    let chain_depth = usize::from(statement.chain_depth);
    let leaf_bytes = witness
        .certificate_chain_der
        .first()
        .ok_or(ZkX509RelationErrorV1::WitnessMismatch)?
        .len();
    let chain_bytes =
        witness
            .certificate_chain_der
            .iter()
            .try_fold(0_usize, |sum, certificate| {
                sum.checked_add(certificate.len())
                    .ok_or(ZkX509RelationErrorV1::WitnessMismatch)
            })?;
    if witness.certificate_chain_der.len() != chain_depth
        || usize::try_from(statement.leaf_certificate_bytes).ok() != Some(leaf_bytes)
        || usize::try_from(statement.chain_certificate_bytes).ok() != Some(chain_bytes)
        || witness.attribute_openings.len() != statement.disclosed_attributes.len()
        || witness
            .attribute_openings
            .iter()
            .zip(&statement.disclosed_attributes)
            .any(|(opening, disclosed)| opening.index != disclosed.index)
    {
        return Err(ZkX509RelationErrorV1::WitnessMismatch);
    }
    Ok(())
}

fn validate_certificate_path_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    chain: &[ParsedCertificateV1<'_>],
) -> Result<(), ZkX509RelationErrorV1> {
    if chain.len() < 2 || chain.len() > 3 {
        return Err(ZkX509RelationErrorV1::InvalidCertificatePath);
    }
    for certificate in chain {
        if statement.validation_unix_seconds < certificate.not_before
            || statement.validation_unix_seconds > certificate.not_after
        {
            return Err(ZkX509RelationErrorV1::CertificateNotValid);
        }
    }
    let leaf = &chain[0];
    if leaf.not_before != statement.not_before_unix_seconds
        || leaf.not_after != statement.not_after_unix_seconds
        || leaf.extensions.basic_constraints.ca
        || leaf.extensions.basic_constraints.path_len.is_some()
        || leaf.extensions.key_usage != leaf_key_usage_flags_v1(statement.key_usage)
        || leaf.extensions.extended_key_usages.as_deref()
            != Some(statement.extended_key_usages.as_slice())
    {
        return Err(ZkX509RelationErrorV1::InvalidCertificatePath);
    }

    for index in 1..chain.len() {
        let ca = &chain[index];
        let subordinate_ca_count =
            u32::try_from(index - 1).map_err(|_| ZkX509RelationErrorV1::InvalidCertificatePath)?;
        if !ca.extensions.basic_constraints.ca
            || ca
                .extensions
                .basic_constraints
                .path_len
                .is_none_or(|path_len| path_len < subordinate_ca_count)
            || ca.extensions.key_usage != (KEY_USAGE_KEY_CERT_SIGN | KEY_USAGE_CRL_SIGN)
            || ca.extensions.extended_key_usages.is_some()
        {
            return Err(ZkX509RelationErrorV1::InvalidCertificatePath);
        }
    }

    for index in 0..chain.len() - 1 {
        let child = &chain[index];
        let parent = &chain[index + 1];
        if child.issuer.encoded != parent.subject.encoded
            || child.extensions.authority_key_identifier != parent.extensions.subject_key_identifier
        {
            return Err(ZkX509RelationErrorV1::InvalidCertificatePath);
        }
        verify_p256_sha256_signature_v1(parent.public_key, child.tbs_der, child.signature_der)?;
    }

    let root = chain
        .last()
        .ok_or(ZkX509RelationErrorV1::InvalidCertificatePath)?;
    if root.issuer.encoded != root.subject.encoded
        || root.extensions.authority_key_identifier != root.extensions.subject_key_identifier
    {
        return Err(ZkX509RelationErrorV1::InvalidCertificatePath);
    }
    verify_p256_sha256_signature_v1(root.public_key, root.tbs_der, root.signature_der)?;
    Ok(())
}

fn leaf_key_usage_flags_v1(key_usage: PrivacyX509KeyUsageV1) -> u16 {
    let mut flags = 0_u16;
    if key_usage.digital_signature {
        flags |= KEY_USAGE_DIGITAL_SIGNATURE;
    }
    if key_usage.content_commitment {
        flags |= KEY_USAGE_CONTENT_COMMITMENT;
    }
    if key_usage.key_encipherment {
        flags |= KEY_USAGE_KEY_ENCIPHERMENT;
    }
    if key_usage.key_agreement {
        flags |= KEY_USAGE_KEY_AGREEMENT;
    }
    flags
}

fn validate_complete_crl_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    governed_crl: &PrivacyZkX509CrlRecordV1,
    chain: &[ParsedCertificateV1<'_>],
    crl: &ParsedCrlV1<'_>,
    crl_der: &[u8],
) -> Result<[u8; 32], ZkX509RelationErrorV1> {
    let issuer = chain
        .get(1)
        .ok_or(ZkX509RelationErrorV1::InvalidCertificatePath)?;
    let leaf = chain
        .first()
        .ok_or(ZkX509RelationErrorV1::InvalidCertificatePath)?;
    if crl.issuer.encoded != issuer.subject.encoded
        || crl.authority_key_identifier != issuer.extensions.subject_key_identifier
        || issuer.extensions.key_usage & KEY_USAGE_CRL_SIGN == 0
        || crl.crl_number != governed_crl.crl_number
        || crl.this_update != governed_crl.this_update_unix_seconds
        || crl.next_update != governed_crl.next_update_unix_seconds
        || PrivacyX509CrlDerDigestV1::digest_exact_der(crl_der) != governed_crl.crl_der_digest
        || PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(issuer.spki_der)
            != governed_crl.issuer_spki_digest
        || statement.validation_unix_seconds < crl.this_update
        || statement.validation_unix_seconds >= crl.next_update
        || statement
            .validation_unix_seconds
            .checked_sub(crl.this_update)
            .is_none_or(|age| age > ZK_X509_MAX_CRL_AGE_SECONDS_V1)
    {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    verify_p256_sha256_signature_v1(issuer.public_key, crl.tbs_der, crl.signature_der)
        .map_err(|_| ZkX509RelationErrorV1::InvalidCrl)?;

    if crl
        .revoked_serials
        .iter()
        .any(|serial| *serial == leaf.serial)
    {
        return Err(ZkX509RelationErrorV1::CertificateRevoked);
    }
    let root = crl_root_from_complete_serials_v1(issuer.spki_der, &crl.revoked_serials)?;
    if root != *governed_crl.revoked_serials_root.as_bytes()
        || root != *statement.crl_nonmembership_root.as_bytes()
    {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    Ok(root)
}

fn derive_subject_public_key_digest_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    chain: &[ParsedCertificateV1<'_>],
) -> Result<PrivacyCertificateKeyDigestV1, ZkX509RelationErrorV1> {
    let relation_version = ZK_X509_RELATION_VERSION_V1.to_be_bytes();
    let mut fields: Vec<&[u8]> = vec![
        ZK_X509_SUITE_V1,
        ZK_X509_SOURCE_PROFILE_V1,
        &relation_version,
        statement.trust_anchor_id.as_bytes(),
        statement.certificate_policy_id.as_bytes(),
        statement.trust_anchor_record_digest.as_bytes(),
        statement.certificate_policy_record_digest.as_bytes(),
    ];
    fields.extend(chain.iter().map(|certificate| certificate.spki_der));
    Ok(PrivacyCertificateKeyDigestV1::new(hash_frame_v1(
        ZK_X509_SCOPED_KEY_DOMAIN_V1,
        &fields,
    )?))
}

fn derive_certificate_nullifier_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    issuer_spki_der: &[u8],
    serial: &[u8],
) -> Result<PrivacyNullifierV1, ZkX509RelationErrorV1> {
    Ok(PrivacyNullifierV1::new(hash_frame_v1(
        ZK_X509_NULLIFIER_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            statement.trust_anchor_id.as_bytes(),
            statement.certificate_policy_id.as_bytes(),
            issuer_spki_der,
            serial,
        ],
    )?))
}

fn validate_attribute_projections_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    leaf: &ParsedCertificateV1<'_>,
    witness: &ZkX509WitnessV1,
) -> Result<(), ZkX509RelationErrorV1> {
    for (disclosed, opening) in statement
        .disclosed_attributes
        .iter()
        .zip(&witness.attribute_openings)
    {
        let index = usize::from(disclosed.index);
        let value = leaf
            .subject
            .attributes
            .get(index)
            .copied()
            .flatten()
            .ok_or(ZkX509RelationErrorV1::ProjectionMismatch)?;
        let index_byte = [disclosed.index];
        let digest = PrivacyAttributeDigestV1::new(hash_frame_v1(
            ZK_X509_ATTRIBUTE_DOMAIN_V1,
            &[
                ZK_X509_SUITE_V1,
                statement.trust_anchor_id.as_bytes(),
                statement.certificate_policy_id.as_bytes(),
                &index_byte,
                value,
                &opening.salt,
            ],
        )?);
        if digest != disclosed.attribute_digest {
            return Err(ZkX509RelationErrorV1::ProjectionMismatch);
        }
    }
    Ok(())
}

fn derive_ownership_challenge_digest_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<[u8; 32], ZkX509RelationErrorV1> {
    let statement_digest = PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone())
        .digest()
        .map_err(|_| ZkX509RelationErrorV1::StatementEncoding)?;
    let account = norito::to_bytes(&statement.wallet_account)
        .map_err(|_| ZkX509RelationErrorV1::StatementEncoding)?;
    let relation_version = ZK_X509_RELATION_VERSION_V1.to_be_bytes();
    Ok(hash_frame_v1(
        ZK_X509_OWNERSHIP_DOMAIN_V1,
        &[
            ZK_X509_SUITE_V1,
            ZK_X509_SOURCE_PROFILE_V1,
            &relation_version,
            statement_digest.as_bytes(),
            &account,
            statement.wallet_challenge.as_bytes(),
            statement.context.transaction_intent_digest.as_bytes(),
        ],
    )?)
}

#[derive(Clone, Copy)]
struct ParsedNameV1<'a> {
    encoded: &'a [u8],
    attributes: [Option<&'a [u8]>; 4],
}

#[derive(Clone, Copy)]
struct BasicConstraintsV1 {
    ca: bool,
    path_len: Option<u32>,
}

#[derive(Clone)]
struct CertificateExtensionsV1<'a> {
    authority_key_identifier: &'a [u8],
    subject_key_identifier: &'a [u8],
    basic_constraints: BasicConstraintsV1,
    key_usage: u16,
    extended_key_usages: Option<Vec<PrivacyX509ExtendedKeyUsageV1>>,
}

#[derive(Clone)]
struct ParsedCertificateV1<'a> {
    tbs_der: &'a [u8],
    serial: &'a [u8],
    issuer: ParsedNameV1<'a>,
    subject: ParsedNameV1<'a>,
    not_before: u64,
    not_after: u64,
    spki_der: &'a [u8],
    public_key: &'a [u8],
    signature_der: &'a [u8],
    extensions: CertificateExtensionsV1<'a>,
}

fn parse_certificate_v1(der: &[u8]) -> Result<ParsedCertificateV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let certificate =
        parse_single_der_value_v1(der, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut outer = certificate.children(limits)?;
    let tbs = outer.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let outer_algorithm = outer.read_value()?;
    if outer_algorithm.encoded() != ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1 {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    validate_ecdsa_with_sha256_algorithm_identifier_v1(outer_algorithm.encoded(), limits)?;
    let signature_value = outer.read_value()?.as_bit_string()?;
    if !outer.is_empty() || signature_value.unused_bits() != 0 {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    validate_ecdsa_signature_der_v1(signature_value.bytes())?;

    let mut fields = tbs.children(limits)?;
    let version = fields.read_value()?;
    if version.tag() != context_tag_v1(0, true) || version.contents() != [0x02, 0x01, 0x02] {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    let serial = fields
        .read_value()?
        .as_integer()?
        .positive_unsigned(ZK_X509_MAX_SERIAL_BYTES_V1)?
        .bytes();
    let tbs_algorithm = fields.read_value()?;
    if tbs_algorithm.encoded() != outer_algorithm.encoded() {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    let issuer_value = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let issuer = parse_name_v1(issuer_value)?;
    let validity = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let (not_before, not_after) = parse_validity_v1(validity)?;
    if not_after < not_before {
        return Err(ZkX509RelationErrorV1::InvalidTime);
    }
    let subject_value = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let subject = parse_name_v1(subject_value)?;
    let spki = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let (spki_der, public_key) = parse_spki_v1(spki)?;
    let extension_wrapper = fields.read_value()?;
    if extension_wrapper.tag() != context_tag_v1(3, true) || !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidCertificateStructure);
    }
    let extensions = parse_certificate_extensions_v1(extension_wrapper.contents())?;

    Ok(ParsedCertificateV1 {
        tbs_der: tbs.encoded(),
        serial,
        issuer,
        subject,
        not_before,
        not_after,
        spki_der,
        public_key,
        signature_der: signature_value.bytes(),
        extensions,
    })
}

fn parse_spki_v1(spki: ZkX509DerValueV1<'_>) -> Result<(&[u8], &[u8]), ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let mut fields = spki.children(limits)?;
    let algorithm = fields.read_value()?;
    if algorithm.encoded() != ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1 {
        return Err(ZkX509RelationErrorV1::InvalidPublicKey);
    }
    validate_p256_public_key_algorithm_identifier_v1(algorithm.encoded(), limits)?;
    let key = fields.read_value()?.as_bit_string()?;
    if !fields.is_empty()
        || key.unused_bits() != 0
        || key.bytes().len() != ZK_X509_UNCOMPRESSED_P256_BYTES_V1
        || key.bytes().first() != Some(&0x04)
        || P256VerifyingKey::from_sec1_bytes(key.bytes()).is_err()
    {
        return Err(ZkX509RelationErrorV1::InvalidPublicKey);
    }
    Ok((spki.encoded(), key.bytes()))
}

fn parse_validity_v1(validity: ZkX509DerValueV1<'_>) -> Result<(u64, u64), ZkX509RelationErrorV1> {
    let mut fields = validity.children(ZkX509DerLimitsV1::profile())?;
    let not_before = parse_time_v1(fields.read_value()?)?;
    let not_after = parse_time_v1(fields.read_value()?)?;
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidTime);
    }
    Ok((not_before, not_after))
}

fn parse_time_v1(value: ZkX509DerValueV1<'_>) -> Result<u64, ZkX509RelationErrorV1> {
    let (year, offset) = match value.tag() {
        tag if tag == universal_primitive_tag_v1(23) => {
            if value.contents().len() != 13 || value.contents()[12] != b'Z' {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            let short = parse_decimal_v1(&value.contents()[..2])?;
            let year = if short >= 50 {
                1900 + i32::from(short)
            } else {
                2000 + i32::from(short)
            };
            if !(1970..=2049).contains(&year) {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            (year, 2)
        }
        tag if tag == universal_primitive_tag_v1(24) => {
            if value.contents().len() != 15 || value.contents()[14] != b'Z' {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            let year = i32::from(parse_decimal_v1(&value.contents()[..4])?);
            if !(2050..=9999).contains(&year) {
                return Err(ZkX509RelationErrorV1::InvalidTime);
            }
            (year, 4)
        }
        _ => return Err(ZkX509RelationErrorV1::InvalidTime),
    };
    let bytes = value.contents();
    let month = u8::try_from(parse_decimal_v1(&bytes[offset..offset + 2])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let day = u8::try_from(parse_decimal_v1(&bytes[offset + 2..offset + 4])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let hour = u8::try_from(parse_decimal_v1(&bytes[offset + 4..offset + 6])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let minute = u8::try_from(parse_decimal_v1(&bytes[offset + 6..offset + 8])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let second = u8::try_from(parse_decimal_v1(&bytes[offset + 8..offset + 10])?)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let month = Month::try_from(month).map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let date = Date::from_calendar_date(year, month, day)
        .map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    let time =
        Time::from_hms(hour, minute, second).map_err(|_| ZkX509RelationErrorV1::InvalidTime)?;
    u64::try_from(
        PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp(),
    )
    .map_err(|_| ZkX509RelationErrorV1::InvalidTime)
}

fn parse_decimal_v1(bytes: &[u8]) -> Result<u16, ZkX509RelationErrorV1> {
    let mut value = 0_u16;
    for byte in bytes {
        if !byte.is_ascii_digit() {
            return Err(ZkX509RelationErrorV1::InvalidTime);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u16::from(*byte - b'0')))
            .ok_or(ZkX509RelationErrorV1::InvalidTime)?;
    }
    Ok(value)
}

fn parse_name_v1(name: ZkX509DerValueV1<'_>) -> Result<ParsedNameV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let mut rdns = name.children(limits)?;
    if rdns.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidName);
    }
    let mut attributes = [None; 4];
    while !rdns.is_empty() {
        let rdn = rdns.read_value()?.require_tag(ZkX509DerTagV1::SET)?;
        let mut values = rdn.children(limits)?;
        if values.is_empty() {
            return Err(ZkX509RelationErrorV1::InvalidName);
        }
        while !values.is_empty() {
            let attribute = values.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
            let mut fields = attribute.children(limits)?;
            let oid = fields.read_value()?.as_object_identifier()?;
            let value = fields.read_value()?;
            if !fields.is_empty() {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
            let index = if oid.equals(OID_COUNTRY_NAME) {
                0
            } else if oid.equals(OID_ORGANIZATION_NAME) {
                1
            } else if oid.equals(OID_ORGANIZATIONAL_UNIT_NAME) {
                2
            } else if oid.equals(OID_COMMON_NAME) {
                3
            } else {
                return Err(ZkX509RelationErrorV1::InvalidName);
            };
            validate_directory_string_v1(index, value)?;
            if attributes[index].replace(value.encoded()).is_some() {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
        }
    }
    Ok(ParsedNameV1 {
        encoded: name.encoded(),
        attributes,
    })
}

fn validate_directory_string_v1(
    index: usize,
    value: ZkX509DerValueV1<'_>,
) -> Result<(), ZkX509RelationErrorV1> {
    if value.contents().is_empty() || value.contents().len() > ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1
    {
        return Err(ZkX509RelationErrorV1::InvalidName);
    }
    if index == 0 {
        if value.tag() != universal_primitive_tag_v1(19)
            || value.contents().len() != 2
            || !value
                .contents()
                .iter()
                .all(|byte| byte.is_ascii_uppercase())
        {
            return Err(ZkX509RelationErrorV1::InvalidName);
        }
        return Ok(());
    }
    match value.tag() {
        tag if tag == universal_primitive_tag_v1(12) => {
            let string = core::str::from_utf8(value.contents())
                .map_err(|_| ZkX509RelationErrorV1::InvalidName)?;
            if string.chars().any(|character| {
                matches!(
                    u32::from(character),
                    0x0000..=0x001f | 0x007f..=0x009f
                )
            }) {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
        }
        tag if tag == universal_primitive_tag_v1(19) => {
            if !value.contents().iter().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        *byte,
                        b' ' | b'\''
                            | b'('
                            | b')'
                            | b'+'
                            | b','
                            | b'-'
                            | b'.'
                            | b'/'
                            | b':'
                            | b'='
                            | b'?'
                    )
            }) {
                return Err(ZkX509RelationErrorV1::InvalidName);
            }
        }
        _ => return Err(ZkX509RelationErrorV1::InvalidName),
    }
    Ok(())
}

fn context_tag_v1(number: u32, constructed: bool) -> ZkX509DerTagV1 {
    ZkX509DerTagV1 {
        class: ZkX509DerClassV1::ContextSpecific,
        constructed,
        number,
    }
}

fn universal_primitive_tag_v1(number: u32) -> ZkX509DerTagV1 {
    ZkX509DerTagV1 {
        class: ZkX509DerClassV1::Universal,
        constructed: false,
        number,
    }
}

fn parse_certificate_extensions_v1(
    explicit_contents: &[u8],
) -> Result<CertificateExtensionsV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence = parse_single_der_value_v1(explicit_contents, limits)?
        .require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut extensions = sequence.children(limits)?;
    if extensions.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }

    let mut authority_key_identifier = None;
    let mut subject_key_identifier = None;
    let mut basic_constraints = None;
    let mut key_usage = None;
    let mut extended_key_usages = None;
    let mut previous_rank = None;
    while !extensions.is_empty() {
        let (oid, critical, value) = parse_extension_v1(extensions.read_value()?)?;
        let rank = if oid == OID_AUTHORITY_KEY_IDENTIFIER {
            0
        } else if oid == OID_SUBJECT_KEY_IDENTIFIER {
            1
        } else if oid == OID_KEY_USAGE {
            2
        } else if oid == OID_BASIC_CONSTRAINTS {
            3
        } else if oid == OID_EXTENDED_KEY_USAGE {
            4
        } else {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        };
        if previous_rank.is_some_and(|previous| previous >= rank) {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        }
        previous_rank = Some(rank);

        if rank == 0 {
            if critical || authority_key_identifier.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            authority_key_identifier = Some(parse_authority_key_identifier_v1(value)?);
        } else if rank == 1 {
            if critical || subject_key_identifier.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            subject_key_identifier = Some(parse_subject_key_identifier_v1(value)?);
        } else if rank == 2 {
            if !critical || key_usage.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            key_usage = Some(parse_key_usage_v1(value)?);
        } else if rank == 3 {
            if !critical || basic_constraints.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            basic_constraints = Some(parse_basic_constraints_v1(value)?);
        } else {
            if !critical || extended_key_usages.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidExtensions);
            }
            extended_key_usages = Some(parse_extended_key_usages_v1(value)?);
        }
    }

    Ok(CertificateExtensionsV1 {
        authority_key_identifier: authority_key_identifier
            .ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        subject_key_identifier: subject_key_identifier
            .ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        basic_constraints: basic_constraints.ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        key_usage: key_usage.ok_or(ZkX509RelationErrorV1::InvalidExtensions)?,
        extended_key_usages,
    })
}

fn parse_extension_v1(
    extension: ZkX509DerValueV1<'_>,
) -> Result<(&[u8], bool, &[u8]), ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let extension = extension.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = extension.children(limits)?;
    let oid = fields.read_value()?.as_object_identifier()?.contents();
    let critical_or_value = fields.read_value()?;
    let (critical, value) = if critical_or_value.tag() == ZkX509DerTagV1::BOOLEAN {
        if critical_or_value.contents() != [0xff] {
            // DER requires a DEFAULT FALSE field to be omitted.
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        }
        (
            true,
            fields
                .read_value()?
                .require_tag(ZkX509DerTagV1::OCTET_STRING)?
                .contents(),
        )
    } else {
        (
            false,
            critical_or_value
                .require_tag(ZkX509DerTagV1::OCTET_STRING)?
                .contents(),
        )
    };
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok((oid, critical, value))
}

fn parse_authority_key_identifier_v1(encoded: &[u8]) -> Result<&[u8], ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(encoded, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    let key_identifier = fields.read_value()?;
    if key_identifier.tag() != context_tag_v1(0, false)
        || key_identifier.contents().is_empty()
        || key_identifier.contents().len() > 64
        || !fields.is_empty()
    {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok(key_identifier.contents())
}

fn parse_subject_key_identifier_v1(encoded: &[u8]) -> Result<&[u8], ZkX509RelationErrorV1> {
    let identifier = parse_single_der_value_v1(encoded, ZkX509DerLimitsV1::profile())?
        .require_tag(ZkX509DerTagV1::OCTET_STRING)?;
    if identifier.contents().is_empty() || identifier.contents().len() > 64 {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok(identifier.contents())
}

fn parse_basic_constraints_v1(encoded: &[u8]) -> Result<BasicConstraintsV1, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(encoded, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    if fields.is_empty() {
        return Ok(BasicConstraintsV1 {
            ca: false,
            path_len: None,
        });
    }
    let first = fields.read_value()?;
    if first.tag() != ZkX509DerTagV1::BOOLEAN || first.contents() != [0xff] {
        // `cA DEFAULT FALSE` is either omitted or the canonical TRUE value.
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let path_len = if fields.is_empty() {
        None
    } else {
        let value = parse_nonnegative_integer_v1(fields.read_value()?, 4)?;
        Some(u32::try_from(value).map_err(|_| ZkX509RelationErrorV1::InvalidInteger)?)
    };
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    Ok(BasicConstraintsV1 { ca: true, path_len })
}

fn parse_key_usage_v1(encoded: &[u8]) -> Result<u16, ZkX509RelationErrorV1> {
    let bit_string =
        parse_single_der_value_v1(encoded, ZkX509DerLimitsV1::profile())?.as_bit_string()?;
    let bytes = bit_string.bytes();
    if bytes.is_empty()
        || bytes.len() > 2
        || (bytes.last().copied().unwrap_or_default() & (1 << bit_string.unused_bits())) == 0
    {
        // NamedBitList DER omits all trailing zero bits.
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let mut flags = 0_u16;
    for (byte_index, byte) in bytes.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (0x80 >> bit) != 0 {
                let flag_index = byte_index * 8 + bit;
                if flag_index >= u16::BITS as usize {
                    return Err(ZkX509RelationErrorV1::InvalidExtensions);
                }
                flags |= 1_u16 << flag_index;
            }
        }
    }
    Ok(flags)
}

fn parse_extended_key_usages_v1(
    encoded: &[u8],
) -> Result<Vec<PrivacyX509ExtendedKeyUsageV1>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(encoded, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    if fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidExtensions);
    }
    let mut usages = Vec::new();
    while !fields.is_empty() {
        let oid = fields.read_value()?.as_object_identifier()?.contents();
        let usage = if oid == OID_CLIENT_AUTHENTICATION {
            debug_assert_eq!(
                ZK_X509_CLIENT_AUTHENTICATION_EKU_OID_V1,
                "1.3.6.1.5.5.7.3.2"
            );
            PrivacyX509ExtendedKeyUsageV1::ClientAuthentication
        } else if oid == ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1 {
            PrivacyX509ExtendedKeyUsageV1::DocumentSigning
        } else if oid == ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1 {
            PrivacyX509ExtendedKeyUsageV1::WalletIdentity
        } else {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        };
        if usages.last().is_some_and(|previous| *previous >= usage) {
            return Err(ZkX509RelationErrorV1::InvalidExtensions);
        }
        usages.push(usage);
    }
    Ok(usages)
}

fn parse_nonnegative_integer_v1(
    value: ZkX509DerValueV1<'_>,
    max_bytes: usize,
) -> Result<u64, ZkX509RelationErrorV1> {
    let integer = value.as_integer()?.contents();
    if integer.first().is_some_and(|first| first & 0x80 != 0) {
        return Err(ZkX509RelationErrorV1::InvalidInteger);
    }
    let unsigned = if integer.len() > 1 && integer[0] == 0 {
        &integer[1..]
    } else {
        integer
    };
    if unsigned.len() > max_bytes || unsigned.len() > 8 {
        return Err(ZkX509RelationErrorV1::InvalidInteger);
    }
    let mut result = 0_u64;
    for byte in unsigned {
        result = result
            .checked_mul(256)
            .and_then(|value| value.checked_add(u64::from(*byte)))
            .ok_or(ZkX509RelationErrorV1::InvalidInteger)?;
    }
    Ok(result)
}

fn validate_ecdsa_signature_der_v1(
    signature_der: &[u8],
) -> Result<P256Signature, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence =
        parse_single_der_value_v1(signature_der, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut fields = sequence.children(limits)?;
    fields.read_value()?.as_integer()?.positive_unsigned(32)?;
    fields.read_value()?.as_integer()?.positive_unsigned(32)?;
    if !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidSignatureEncoding);
    }
    P256Signature::from_der(signature_der)
        .map_err(|_| ZkX509RelationErrorV1::InvalidSignatureEncoding)
}

fn verify_p256_sha256_signature_v1(
    public_key: &[u8],
    message: &[u8],
    signature_der: &[u8],
) -> Result<(), ZkX509RelationErrorV1> {
    let key = P256VerifyingKey::from_sec1_bytes(public_key)
        .map_err(|_| ZkX509RelationErrorV1::InvalidPublicKey)?;
    let signature = validate_ecdsa_signature_der_v1(signature_der)?;
    let normalized = signature.normalize_s();
    // RFC 5280 accepts both valid s halves.  RustCrypto verifies the
    // mathematically equivalent low-s representative for high-s inputs.
    let signature = normalized.unwrap_or(signature);
    key.verify(message, &signature)
        .map_err(|_| ZkX509RelationErrorV1::InvalidCertificateSignature)
}

fn verify_p256_prehash_signature_v1(
    public_key: &[u8],
    message_digest: &[u8; 32],
    signature_der: &[u8],
) -> Result<(), ZkX509RelationErrorV1> {
    let key = P256VerifyingKey::from_sec1_bytes(public_key)
        .map_err(|_| ZkX509RelationErrorV1::InvalidPublicKey)?;
    let signature = validate_ecdsa_signature_der_v1(signature_der)?;
    if signature.normalize_s().is_some() {
        return Err(ZkX509RelationErrorV1::InvalidWalletOwnership);
    }
    key.verify_prehash(message_digest, &signature)
        .map_err(|_| ZkX509RelationErrorV1::InvalidWalletOwnership)
}

struct ParsedCrlV1<'a> {
    tbs_der: &'a [u8],
    issuer: ParsedNameV1<'a>,
    this_update: u64,
    next_update: u64,
    revoked_serials: Vec<&'a [u8]>,
    authority_key_identifier: &'a [u8],
    crl_number: u64,
    signature_der: &'a [u8],
}

fn parse_crl_v1(der: &[u8]) -> Result<ParsedCrlV1<'_>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let crl = parse_single_der_value_v1(der, limits)?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut outer = crl.children(limits)?;
    let tbs = outer.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let outer_algorithm = outer.read_value()?;
    if outer_algorithm.encoded() != ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1 {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    validate_ecdsa_with_sha256_algorithm_identifier_v1(outer_algorithm.encoded(), limits)?;
    let signature = outer.read_value()?.as_bit_string()?;
    if !outer.is_empty() || signature.unused_bits() != 0 {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    validate_ecdsa_signature_der_v1(signature.bytes())?;

    let mut fields = tbs.children(limits)?;
    let version = fields.read_value()?.as_integer()?;
    if version.contents() != [1] {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let tbs_algorithm = fields.read_value()?;
    if tbs_algorithm.encoded() != outer_algorithm.encoded() {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let issuer_value = fields.read_value()?.require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let issuer = parse_name_v1(issuer_value)?;
    let this_update = parse_time_v1(fields.read_value()?)?;
    let next_update = parse_time_v1(fields.read_value()?)?;
    if next_update <= this_update {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }

    let entries_or_extensions = fields.read_value()?;
    let (revoked_serials, extension_wrapper) =
        if entries_or_extensions.tag() == ZkX509DerTagV1::SEQUENCE {
            let serials = parse_revoked_certificates_v1(entries_or_extensions, this_update)?;
            (serials, fields.read_value()?)
        } else {
            (Vec::new(), entries_or_extensions)
        };
    if extension_wrapper.tag() != context_tag_v1(0, true) || !fields.is_empty() {
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let (authority_key_identifier, crl_number) =
        parse_crl_extensions_v1(extension_wrapper.contents())?;

    Ok(ParsedCrlV1 {
        tbs_der: tbs.encoded(),
        issuer,
        this_update,
        next_update,
        revoked_serials,
        authority_key_identifier,
        crl_number,
        signature_der: signature.bytes(),
    })
}

fn parse_revoked_certificates_v1(
    sequence: ZkX509DerValueV1<'_>,
    this_update: u64,
) -> Result<Vec<&[u8]>, ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let mut entries = sequence.children(limits)?;
    if entries.is_empty() {
        // An empty OPTIONAL sequence has a unique DER representation: absent.
        return Err(ZkX509RelationErrorV1::InvalidCrl);
    }
    let mut serials = Vec::new();
    while !entries.is_empty() {
        if serials.len() >= ZK_X509_MAX_CRL_ENTRIES_V1 {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        let entry = entries
            .read_value()?
            .require_tag(ZkX509DerTagV1::SEQUENCE)?;
        let mut fields = entry.children(limits)?;
        let serial = fields
            .read_value()?
            .as_integer()?
            .positive_unsigned(ZK_X509_MAX_SERIAL_BYTES_V1)?
            .bytes();
        let revocation_time = parse_time_v1(fields.read_value()?)?;
        if revocation_time > this_update || !fields.is_empty() {
            // Entry extensions, including reasonCode/certificateIssuer, are
            // outside the complete direct-CRL v0 profile.
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        serials.push(serial);
    }
    Ok(serials)
}

fn parse_crl_extensions_v1(
    explicit_contents: &[u8],
) -> Result<(&[u8], u64), ZkX509RelationErrorV1> {
    let limits = ZkX509DerLimitsV1::profile();
    let sequence = parse_single_der_value_v1(explicit_contents, limits)?
        .require_tag(ZkX509DerTagV1::SEQUENCE)?;
    let mut extensions = sequence.children(limits)?;
    let mut authority_key_identifier = None;
    let mut crl_number = None;
    let mut previous_rank = None;
    while !extensions.is_empty() {
        let (oid, critical, value) = parse_extension_v1(extensions.read_value()?)?;
        if critical {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        let rank = if oid == OID_AUTHORITY_KEY_IDENTIFIER {
            0
        } else if oid == OID_CRL_NUMBER {
            1
        } else {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        };
        if previous_rank.is_some_and(|previous| previous >= rank) {
            return Err(ZkX509RelationErrorV1::InvalidCrl);
        }
        previous_rank = Some(rank);
        if rank == 0 {
            if authority_key_identifier.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidCrl);
            }
            authority_key_identifier = Some(parse_authority_key_identifier_v1(value)?);
        } else {
            if crl_number.is_some() {
                return Err(ZkX509RelationErrorV1::InvalidCrl);
            }
            let number = parse_single_der_value_v1(value, limits)?;
            crl_number = Some(parse_nonnegative_integer_v1(number, 8)?);
        }
    }
    Ok((
        authority_key_identifier.ok_or(ZkX509RelationErrorV1::InvalidCrl)?,
        crl_number.ok_or(ZkX509RelationErrorV1::InvalidCrl)?,
    ))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        privacy::{
            PrivacyChallengeV1, PrivacyEngineManifestDigestV1, PrivacyIssuerIdV1,
            PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyDigestV1,
            PrivacyPolicyIdV1, PrivacyRootV1, PrivacyStatementContextV1,
            PrivacyStatementSchemaDigestV1, PrivacyTransactionIntentDigestV1,
            PrivacyVerifierDigestV1, PrivacyX509TrustStoreDigestV1,
            PrivacyZkX509DisclosedAttributeV1,
        },
    };
    use p256::{
        ecdsa::{
            SigningKey as P256SigningKey,
            signature::{Signer as _, hazmat::PrehashSigner as _},
        },
        elliptic_curve::PrimeField as _,
    };

    use super::*;
    use crate::privacy_engines::zk_x509::{
        codec::ZkX509AttributeOpeningV1,
        merkle::{ZkX509CaMembershipPathV1, ca_root_from_path_v1},
    };

    const CERT_NOT_BEFORE: u64 = 1_640_995_200; // 2022-01-01T00:00:00Z
    const CERT_NOT_AFTER: u64 = 1_893_456_000; // 2030-01-01T00:00:00Z
    const CRL_THIS_UPDATE: u64 = 1_672_531_200; // 2023-01-01T00:00:00Z
    const CRL_NEXT_UPDATE: u64 = CRL_THIS_UPDATE + 300;
    const VALIDATION_TIME: u64 = CRL_THIS_UPDATE + 60;

    struct Fixture {
        statement: IrohaZkX509StarkP256StatementV1,
        trust_anchor: PrivacyZkX509TrustAnchorRecordV1,
        policy: PrivacyZkX509CertificatePolicyRecordV1,
        crl: PrivacyZkX509CrlRecordV1,
        witness: ZkX509WitnessV1,
    }

    impl Fixture {
        fn governance(&self) -> ZkX509GovernanceV1<'_> {
            ZkX509GovernanceV1 {
                trust_anchor: &self.trust_anchor,
                certificate_policy: &self.policy,
                crl: &self.crl,
            }
        }
    }

    #[test]
    fn strict_reference_relation_accepts_deterministic_known_answer_fixture() {
        let fixture = fixture();
        let output = validate_reference_relation_v1(
            &fixture.statement,
            fixture.governance(),
            &fixture.witness,
        )
        .expect("valid strict relation");
        assert_eq!(
            output.subject_public_key_digest,
            fixture.statement.subject_public_key_digest
        );
        assert_eq!(
            output.certificate_nullifier,
            fixture.statement.certificate_nullifier
        );
        assert_eq!(
            output.complete_crl_root,
            *fixture.statement.crl_nonmembership_root.as_bytes()
        );
        assert_ne!(output.ownership_challenge_digest, [0; 32]);
    }

    #[test]
    fn relation_binds_every_governed_revision_and_public_projection() {
        let fixture = fixture();

        let mut changed = fixture.statement.clone();
        changed.trust_anchor_record_epoch += 1;
        assert_eq!(
            validate_reference_relation_v1(&changed, fixture.governance(), &fixture.witness),
            Err(ZkX509RelationErrorV1::StatementMismatch)
        );

        let mut changed = fixture.statement.clone();
        changed.crl_nonmembership_root.0[0] ^= 1;
        assert_eq!(
            validate_reference_relation_v1(&changed, fixture.governance(), &fixture.witness),
            Err(ZkX509RelationErrorV1::StatementMismatch)
        );

        let mut changed = fixture.statement.clone();
        changed.subject_public_key_digest.0[0] ^= 1;
        assert_eq!(
            validate_reference_relation_v1(&changed, fixture.governance(), &fixture.witness),
            Err(ZkX509RelationErrorV1::ProjectionMismatch)
        );

        let mut changed = fixture.statement.clone();
        changed.certificate_nullifier.0[0] ^= 1;
        assert_eq!(
            validate_reference_relation_v1(&changed, fixture.governance(), &fixture.witness),
            Err(ZkX509RelationErrorV1::ProjectionMismatch)
        );

        let mut changed = fixture.statement.clone();
        changed.disclosed_attributes[0].attribute_digest.0[0] ^= 1;
        assert_eq!(
            validate_reference_relation_v1(&changed, fixture.governance(), &fixture.witness),
            Err(ZkX509RelationErrorV1::ProjectionMismatch)
        );
    }

    #[test]
    fn relation_rejects_certificate_crl_path_and_ownership_mutations() {
        let fixture = fixture();

        let mut changed = fixture.witness.clone();
        let leaf_last = changed.certificate_chain_der[0].len() - 1;
        changed.certificate_chain_der[0][leaf_last] ^= 1;
        assert!(matches!(
            validate_reference_relation_v1(&fixture.statement, fixture.governance(), &changed),
            Err(ZkX509RelationErrorV1::InvalidCertificateSignature
                | ZkX509RelationErrorV1::InvalidSignatureEncoding
                | ZkX509RelationErrorV1::Der(_))
        ));

        let mut changed = fixture.witness.clone();
        let crl_last = changed.crl_der.len() - 1;
        changed.crl_der[crl_last] ^= 1;
        assert!(matches!(
            validate_reference_relation_v1(&fixture.statement, fixture.governance(), &changed),
            Err(ZkX509RelationErrorV1::InvalidCrl
                | ZkX509RelationErrorV1::InvalidSignatureEncoding
                | ZkX509RelationErrorV1::Der(_))
        ));

        let mut changed = fixture.witness.clone();
        changed.ca_membership_path.siblings[127][9] ^= 1;
        assert!(matches!(
            validate_reference_relation_v1(&fixture.statement, fixture.governance(), &changed),
            Err(ZkX509RelationErrorV1::Merkle(
                ZkX509MerkleErrorV1::RootMismatch { tree: "CA" }
            ))
        ));

        let mut changed = fixture.witness.clone();
        changed.wallet_ownership_signature_der[8] ^= 1;
        assert!(matches!(
            validate_reference_relation_v1(&fixture.statement, fixture.governance(), &changed),
            Err(ZkX509RelationErrorV1::InvalidWalletOwnership
                | ZkX509RelationErrorV1::InvalidSignatureEncoding
                | ZkX509RelationErrorV1::Der(_))
        ));
    }

    #[test]
    fn strict_times_cover_leap_days_boundaries_and_malformed_encodings() {
        assert_eq!(
            parse_time_v1(der_value(&tlv(0x17, b"000229123456Z"))).expect("leap date"),
            951_827_696
        );
        assert!(parse_time_v1(der_value(&tlv(0x17, b"490101000000Z"))).is_ok());
        assert!(parse_time_v1(der_value(&tlv(0x18, b"20500101000000Z"))).is_ok());

        for malformed in [
            tlv(0x17, b"690101000000Z"),
            tlv(0x17, b"000230000000Z"),
            tlv(0x17, b"2301010000Z"),
            tlv(0x17, b"230101000000+"),
            tlv(0x18, b"20491231235959Z"),
            tlv(0x18, b"20500101000000.0Z"),
        ] {
            assert_eq!(
                parse_time_v1(der_value(&malformed)),
                Err(ZkX509RelationErrorV1::InvalidTime)
            );
        }
    }

    #[test]
    fn distinguished_names_use_a_version_independent_closed_string_profile() {
        let unicode = name(&[
            (OID_COUNTRY_NAME, 0x13, b"IL"),
            (OID_COMMON_NAME, 0x0c, "Alice \u{1f512}".as_bytes()),
        ]);
        parse_name_v1(der_value(&unicode)).expect("well-formed non-control UTF-8");

        for invalid in [
            name(&[(OID_COMMON_NAME, 0x0c, "Alice\u{0085}".as_bytes())]),
            name(&[(OID_COMMON_NAME, 0x0c, &[0xc0, 0x80])]),
            name(&[(OID_COMMON_NAME, 0x13, b"Alice@")]),
            name(&[(OID_COUNTRY_NAME, 0x13, b"il")]),
            name(&[
                (OID_COUNTRY_NAME, 0x13, b"IL"),
                (OID_COUNTRY_NAME, 0x13, b"US"),
            ]),
        ] {
            assert_eq!(
                parse_name_v1(der_value(&invalid)).map(|_| ()),
                Err(ZkX509RelationErrorV1::InvalidName)
            );
        }
    }

    #[test]
    fn extension_defaults_order_duplicates_and_named_bits_fail_closed() {
        let aki = extension(OID_AUTHORITY_KEY_IDENTIFIER, false, &aki_inner(&[1; 20]));
        let ski = extension(OID_SUBJECT_KEY_IDENTIFIER, false, &octet_string(&[2; 20]));
        let key_usage = extension(OID_KEY_USAGE, true, &bit_string(&[0x80], 7));
        let basic_constraints = extension(OID_BASIC_CONSTRAINTS, true, &sequence(&[]));
        let eku = extension(
            OID_EXTENDED_KEY_USAGE,
            true,
            &sequence(&[object_identifier(OID_CLIENT_AUTHENTICATION)]),
        );
        let valid = sequence(&[aki.clone(), ski.clone(), key_usage, basic_constraints, eku]);
        parse_certificate_extensions_v1(&valid).expect("canonical extensions");

        let reordered = sequence(&[
            ski.clone(),
            aki.clone(),
            extension(OID_KEY_USAGE, true, &bit_string(&[0x80], 7)),
            extension(OID_BASIC_CONSTRAINTS, true, &sequence(&[])),
            extension(
                OID_EXTENDED_KEY_USAGE,
                true,
                &sequence(&[object_identifier(OID_CLIENT_AUTHENTICATION)]),
            ),
        ]);
        assert_eq!(
            parse_certificate_extensions_v1(&reordered).map(|_| ()),
            Err(ZkX509RelationErrorV1::InvalidExtensions)
        );

        let duplicate = sequence(&[aki.clone(), aki]);
        assert_eq!(
            parse_certificate_extensions_v1(&duplicate).map(|_| ()),
            Err(ZkX509RelationErrorV1::InvalidExtensions)
        );

        let explicit_false = sequence(&[
            object_identifier(OID_AUTHORITY_KEY_IDENTIFIER),
            tlv(0x01, &[0]),
            octet_string(&aki_inner(&[1; 20])),
        ]);
        assert_eq!(
            parse_extension_v1(der_value(&explicit_false)),
            Err(ZkX509RelationErrorV1::InvalidExtensions)
        );

        assert_eq!(
            parse_key_usage_v1(&bit_string(&[0x80, 0], 0)),
            Err(ZkX509RelationErrorV1::InvalidExtensions)
        );
    }

    #[test]
    fn wallet_ownership_rejects_mathematically_valid_high_s_malleation() {
        let fixture = fixture();
        let signature = P256Signature::from_der(&fixture.witness.wallet_ownership_signature_der)
            .expect("fixture signature");
        let (r, s) = signature.split_scalars();
        let high_s = -*s;
        let malleated = P256Signature::from_scalars(r.to_repr(), high_s.to_repr())
            .expect("valid high-s counterpart");
        assert!(malleated.normalize_s().is_some());
        let mut witness = fixture.witness.clone();
        witness.wallet_ownership_signature_der = malleated.to_der().as_bytes().to_vec();
        assert_eq!(
            validate_reference_relation_v1(&fixture.statement, fixture.governance(), &witness),
            Err(ZkX509RelationErrorV1::InvalidWalletOwnership)
        );
    }

    #[test]
    fn rfc5280_signatures_accept_the_mathematically_equivalent_high_s_form() {
        let key = p256_key(9);
        let message = b"exact certificate or CRL tbs";
        let signature: P256Signature = key.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        let (r, s) = signature.split_scalars();
        let high_s = P256Signature::from_scalars(r.to_repr(), (-*s).to_repr())
            .expect("valid high-s counterpart");
        assert!(high_s.normalize_s().is_some());
        verify_p256_sha256_signature_v1(
            key.verifying_key().to_encoded_point(false).as_bytes(),
            message,
            high_s.to_der().as_bytes(),
        )
        .expect("RFC 5280 accepts either valid s half");
    }

    fn fixture() -> Fixture {
        let root_key = p256_key(1);
        let leaf_key = p256_key(2);
        let root_name = name(&[(OID_COMMON_NAME, 0x0c, b"Iroha Test Root")]);
        let leaf_name = name(&[
            (OID_COUNTRY_NAME, 0x13, b"IL"),
            (OID_ORGANIZATION_NAME, 0x0c, b"Iroha"),
            (OID_COMMON_NAME, 0x0c, b"Alice"),
        ]);
        let root_ski = [0x31; 20];
        let leaf_ski = [0x42; 20];
        let root_der = certificate(
            1, &root_name, &root_name, &root_key, &root_key, &root_ski, &root_ski, true,
        );
        let leaf_der = certificate(
            2, &root_name, &leaf_name, &leaf_key, &root_key, &leaf_ski, &root_ski, false,
        );
        let crl_der = crl(&root_name, &root_key, &root_ski, 7);

        let parsed_leaf = parse_certificate_v1(&leaf_der).expect("leaf");
        let parsed_root = parse_certificate_v1(&root_der).expect("root");
        let parsed_chain = vec![parsed_leaf, parsed_root];
        let parsed_crl = parse_crl_v1(&crl_der).expect("CRL");
        assert!(parsed_crl.revoked_serials.is_empty());

        let ca_membership_path = ZkX509CaMembershipPathV1 {
            siblings: (0..256).map(|index| [index as u8; 32]).collect(),
        };
        let ca_root =
            ca_root_from_path_v1(parsed_chain[1].spki_der, &ca_membership_path).expect("CA root");
        let complete_crl_root =
            crl_root_from_complete_serials_v1(parsed_chain[1].spki_der, &[]).expect("CRL root");
        let trust_anchor_id = PrivacyIssuerIdV1::new([0x51; 32]);
        let policy_id = PrivacyPolicyIdV1::new([0x52; 32]);
        let trust_anchor = PrivacyZkX509TrustAnchorRecordV1::new(
            trust_anchor_id,
            1,
            PrivacyX509TrustStoreDigestV1::new([0x53; 32]),
            PrivacyRootV1::new(ca_root),
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("trust anchor");
        let key_usage = PrivacyX509KeyUsageV1 {
            digital_signature: true,
            content_commitment: false,
            key_encipherment: false,
            key_agreement: false,
        };
        let extended_key_usages = vec![PrivacyX509ExtendedKeyUsageV1::ClientAuthentication];
        let policy = PrivacyZkX509CertificatePolicyRecordV1::new(
            trust_anchor_id,
            policy_id,
            1,
            PrivacyPolicyDigestV1::new([0x54; 32]),
            key_usage,
            extended_key_usages.clone(),
            vec![3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("policy");
        let crl = PrivacyZkX509CrlRecordV1::new(
            trust_anchor_id,
            policy_id,
            1,
            7,
            PrivacyX509CrlDerDigestV1::digest_exact_der(&crl_der),
            PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(parsed_chain[1].spki_der),
            CRL_THIS_UPDATE,
            CRL_NEXT_UPDATE,
            PrivacyRootV1::new(complete_crl_root),
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("CRL record");

        let wallet_key_pair =
            KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519).expect("wallet fixture key");
        let mut statement = IrohaZkX509StarkP256StatementV1 {
            context: PrivacyStatementContextV1 {
                chain_id: ChainId::from("taira-zk-x509-reference-test"),
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x62; 32]),
                parameter_id: PrivacyParameterIdV1::new([0x63; 32]),
                parameter_digest: PrivacyParameterDigestV1::new([0x64; 32]),
                verifier_digest: PrivacyVerifierDigestV1::new([0x65; 32]),
                statement_schema_digest: PrivacyStatementSchemaDigestV1::new([0x66; 32]),
                engine_manifest_digest: PrivacyEngineManifestDigestV1::new([0x67; 32]),
            },
            trust_anchor_id,
            certificate_policy_id: policy_id,
            trust_anchor_record_digest: trust_anchor.record_digest,
            trust_anchor_record_epoch: trust_anchor.record_epoch,
            certificate_policy_record_digest: policy.record_digest,
            certificate_policy_record_epoch: policy.record_epoch,
            crl_record_digest: crl.record_digest,
            crl_record_epoch: crl.record_epoch,
            subject_public_key_digest: PrivacyCertificateKeyDigestV1::new([0; 32]),
            ca_membership_root: PrivacyRootV1::new(ca_root),
            ca_membership_root_epoch: 1,
            crl_nonmembership_root: PrivacyRootV1::new(complete_crl_root),
            crl_nonmembership_root_epoch: 1,
            key_usage,
            extended_key_usages,
            disclosed_attributes: vec![PrivacyZkX509DisclosedAttributeV1 {
                index: 3,
                attribute_digest: PrivacyAttributeDigestV1::new([0; 32]),
            }],
            not_before_unix_seconds: CERT_NOT_BEFORE,
            not_after_unix_seconds: CERT_NOT_AFTER,
            validation_unix_seconds: VALIDATION_TIME,
            chain_depth: 2,
            leaf_certificate_bytes: u32::try_from(leaf_der.len()).expect("leaf length"),
            chain_certificate_bytes: u32::try_from(leaf_der.len() + root_der.len())
                .expect("chain length"),
            wallet_account: AccountId::new(wallet_key_pair.public_key().clone()),
            wallet_challenge: PrivacyChallengeV1::new([0x68; 32]),
            certificate_nullifier: PrivacyNullifierV1::new([0; 32]),
        };
        statement.subject_public_key_digest =
            derive_subject_public_key_digest_v1(&statement, &parsed_chain).expect("key digest");
        statement.certificate_nullifier = derive_certificate_nullifier_v1(
            &statement,
            parsed_chain[1].spki_der,
            parsed_chain[0].serial,
        )
        .expect("nullifier");
        let opening = ZkX509AttributeOpeningV1 {
            index: 3,
            salt: [0x71; 32],
        };
        let index = [opening.index];
        statement.disclosed_attributes[0].attribute_digest = PrivacyAttributeDigestV1::new(
            hash_frame_v1(
                ZK_X509_ATTRIBUTE_DOMAIN_V1,
                &[
                    ZK_X509_SUITE_V1,
                    statement.trust_anchor_id.as_bytes(),
                    statement.certificate_policy_id.as_bytes(),
                    &index,
                    parsed_chain[0].subject.attributes[3].expect("CN"),
                    &opening.salt,
                ],
            )
            .expect("attribute digest"),
        );
        let ownership_challenge =
            derive_ownership_challenge_digest_v1(&statement).expect("challenge");
        let ownership_signature: P256Signature = leaf_key
            .sign_prehash(&ownership_challenge)
            .expect("ownership prehash signing");
        assert!(ownership_signature.normalize_s().is_none());

        Fixture {
            statement,
            trust_anchor,
            policy,
            crl,
            witness: ZkX509WitnessV1 {
                certificate_chain_der: vec![leaf_der, root_der],
                crl_der,
                ca_membership_path,
                wallet_ownership_signature_der: ownership_signature.to_der().as_bytes().to_vec(),
                attribute_openings: vec![opening],
            },
        }
    }

    fn p256_key(seed: u8) -> P256SigningKey {
        let mut bytes = [0_u8; 32];
        bytes[31] = seed;
        P256SigningKey::from_slice(&bytes).expect("nonzero fixture key")
    }

    #[allow(clippy::too_many_arguments)]
    fn certificate(
        serial: u64,
        issuer: &[u8],
        subject: &[u8],
        subject_key: &P256SigningKey,
        issuer_key: &P256SigningKey,
        subject_key_identifier: &[u8],
        authority_key_identifier: &[u8],
        is_ca: bool,
    ) -> Vec<u8> {
        let spki = spki(subject_key);
        let basic_constraints = if is_ca {
            sequence(&[tlv(0x01, &[0xff]), integer(0)])
        } else {
            sequence(&[])
        };
        let key_usage = if is_ca {
            bit_string(&[0x06], 1)
        } else {
            bit_string(&[0x80], 7)
        };
        let mut extensions = vec![
            extension(
                OID_AUTHORITY_KEY_IDENTIFIER,
                false,
                &aki_inner(authority_key_identifier),
            ),
            extension(
                OID_SUBJECT_KEY_IDENTIFIER,
                false,
                &octet_string(subject_key_identifier),
            ),
            extension(OID_KEY_USAGE, true, &key_usage),
            extension(OID_BASIC_CONSTRAINTS, true, &basic_constraints),
        ];
        if !is_ca {
            extensions.push(extension(
                OID_EXTENDED_KEY_USAGE,
                true,
                &sequence(&[object_identifier(OID_CLIENT_AUTHENTICATION)]),
            ));
        }
        let tbs = sequence(&[
            tlv(0xa0, &integer(2)),
            integer(serial),
            ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
            issuer.to_vec(),
            sequence(&[tlv(0x17, b"220101000000Z"), tlv(0x17, b"300101000000Z")]),
            subject.to_vec(),
            spki,
            tlv(0xa3, &sequence(&extensions)),
        ]);
        let signature: P256Signature = issuer_key.sign(&tbs);
        sequence(&[
            tbs,
            ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
            bit_string(signature.to_der().as_bytes(), 0),
        ])
    }

    fn crl(
        issuer: &[u8],
        issuer_key: &P256SigningKey,
        authority_key_identifier: &[u8],
        number: u64,
    ) -> Vec<u8> {
        let extensions = sequence(&[
            extension(
                OID_AUTHORITY_KEY_IDENTIFIER,
                false,
                &aki_inner(authority_key_identifier),
            ),
            extension(OID_CRL_NUMBER, false, &integer(number)),
        ]);
        let tbs = sequence(&[
            integer(1),
            ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
            issuer.to_vec(),
            tlv(0x17, b"230101000000Z"),
            tlv(0x17, b"230101000500Z"),
            tlv(0xa0, &extensions),
        ]);
        let signature: P256Signature = issuer_key.sign(&tbs);
        sequence(&[
            tbs,
            ZK_X509_ECDSA_WITH_SHA256_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
            bit_string(signature.to_der().as_bytes(), 0),
        ])
    }

    fn spki(key: &P256SigningKey) -> Vec<u8> {
        let point = key.verifying_key().to_encoded_point(false);
        sequence(&[
            ZK_X509_P256_PUBLIC_KEY_ALGORITHM_IDENTIFIER_DER_V1.to_vec(),
            bit_string(point.as_bytes(), 0),
        ])
    }

    fn name(attributes: &[(&[u8], u8, &[u8])]) -> Vec<u8> {
        sequence(
            &attributes
                .iter()
                .map(|(oid, tag, value)| {
                    tlv(0x31, &sequence(&[object_identifier(oid), tlv(*tag, value)]))
                })
                .collect::<Vec<_>>(),
        )
    }

    fn extension(oid: &[u8], critical: bool, inner: &[u8]) -> Vec<u8> {
        let mut fields = vec![object_identifier(oid)];
        if critical {
            fields.push(tlv(0x01, &[0xff]));
        }
        fields.push(octet_string(inner));
        sequence(&fields)
    }

    fn aki_inner(identifier: &[u8]) -> Vec<u8> {
        sequence(&[tlv(0x80, identifier)])
    }

    fn object_identifier(contents: &[u8]) -> Vec<u8> {
        tlv(0x06, contents)
    }

    fn octet_string(contents: &[u8]) -> Vec<u8> {
        tlv(0x04, contents)
    }

    fn bit_string(bytes: &[u8], unused_bits: u8) -> Vec<u8> {
        let mut contents = Vec::with_capacity(bytes.len() + 1);
        contents.push(unused_bits);
        contents.extend_from_slice(bytes);
        tlv(0x03, &contents)
    }

    fn integer(value: u64) -> Vec<u8> {
        if value == 0 {
            return tlv(0x02, &[0]);
        }
        let bytes = value.to_be_bytes();
        let first = bytes
            .iter()
            .position(|byte| *byte != 0)
            .expect("nonzero integer");
        let mut magnitude = bytes[first..].to_vec();
        if magnitude[0] & 0x80 != 0 {
            magnitude.insert(0, 0);
        }
        tlv(0x02, &magnitude)
    }

    fn sequence(values: &[Vec<u8>]) -> Vec<u8> {
        let contents = values.concat();
        tlv(0x30, &contents)
    }

    fn tlv(tag: u8, contents: &[u8]) -> Vec<u8> {
        let mut encoded = vec![tag];
        if contents.len() < 128 {
            encoded.push(contents.len() as u8);
        } else {
            let bytes = contents.len().to_be_bytes();
            let first = bytes
                .iter()
                .position(|byte| *byte != 0)
                .expect("nonempty long length");
            let length = &bytes[first..];
            encoded.push(0x80 | u8::try_from(length.len()).expect("length width"));
            encoded.extend_from_slice(length);
        }
        encoded.extend_from_slice(contents);
        encoded
    }

    fn der_value(encoded: &[u8]) -> ZkX509DerValueV1<'_> {
        parse_single_der_value_v1(encoded, ZkX509DerLimitsV1::profile()).expect("test DER")
    }
}
