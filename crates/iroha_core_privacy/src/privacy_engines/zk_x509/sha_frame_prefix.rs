//! Verifier-owned constant bytes in the canonical SHA field frames.
//!
//! A fixed-width statement, governance, or private field advances the public offset without fixing its
//! contents. Only the known zero prefix of a variable-length header is fixed; later offsets
//! depend on statement, governance, or private lengths and remain the responsibility of their adapters.
//! The one-field CRL frame additionally binds its complete u64 field length
//! through the SHA call's carried 13-bit raw-length decomposition.
use super::{ZkX509ShaCallActivationV1, ZkX509ShaCallManifestV1, ZkX509ShaCallRoleV1};
use crate::privacy_engines::zk_x509::profile::{
    ZK_X509_ATTRIBUTE_DOMAIN_V1, ZK_X509_CA_LEAF_DOMAIN_V1, ZK_X509_CA_NODE_DOMAIN_V1,
    ZK_X509_CRL_DER_DIGEST_DOMAIN_V1, ZK_X509_CRL_ISSUER_SPKI_DIGEST_DOMAIN_V1,
    ZK_X509_HASH_FRAME_DOMAIN_V1, ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1,
    ZK_X509_MAX_SERIAL_BYTES_V1, ZK_X509_NULLIFIER_DOMAIN_V1, ZK_X509_OWNERSHIP_DOMAIN_V1,
    ZK_X509_RELATION_VERSION_V1, ZK_X509_SCOPED_KEY_DOMAIN_V1, ZK_X509_SOURCE_PROFILE_V1,
    ZK_X509_SUITE_V1,
};
use iroha_data_model::privacy::{
    ZK_X509_CERTIFICATE_POLICY_RECORD_DIGEST_DOMAIN_V1, ZK_X509_CRL_RECORD_DIGEST_DOMAIN_V1,
    ZK_X509_GOVERNANCE_RECORD_VERSION_V1, ZK_X509_TRUST_ANCHOR_RECORD_DIGEST_DOMAIN_V1,
};
/// Public expectation for one message byte, before the SHA padding byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShaFrameByteV1 {
    /// Constant byte from a domain, field header, or protocol constant.
    Constant(u8),
    /// Bits 8 through 12 of the call's private raw length.
    RawLengthHigh,
    /// Bits 0 through 7 of the call's private raw length.
    RawLengthLow,
}
#[derive(Clone, Copy)]
enum FieldV1<'a> {
    Constant(&'a [u8]),
    Unfixed(usize),
    Variable(usize),
    RawLength,
}
fn header_byte_v1(domain: &[u8], fields: u16, offset: usize) -> Option<u8> {
    let magic = ZK_X509_HASH_FRAME_DOMAIN_V1;
    if offset < magic.len() {
        return Some(magic[offset]);
    }
    let offset = offset - magic.len();
    if offset < 2 {
        return Some((domain.len() as u16).to_be_bytes()[offset]);
    }
    let offset = offset - 2;
    if offset < domain.len() {
        return Some(domain[offset]);
    }
    let offset = offset - domain.len();
    (offset < 2).then(|| fields.to_be_bytes()[offset])
}
fn field_frame_byte_v1(
    domain: &[u8],
    field_count: u16,
    fields: &[FieldV1<'_>],
    offset: usize,
) -> Option<ShaFrameByteV1> {
    let header = ZK_X509_HASH_FRAME_DOMAIN_V1.len() + 4 + domain.len();
    if offset < header {
        return header_byte_v1(domain, field_count, offset).map(ShaFrameByteV1::Constant);
    }
    let mut offset = offset - header;
    for field in fields {
        let len = match field {
            FieldV1::Constant(bytes) => bytes.len(),
            FieldV1::Unfixed(len) => *len,
            FieldV1::Variable(maximum) => {
                // All admitted lengths fit this public bound. Pin only the
                // leading zero bytes; the remaining length and later offsets
                // still require the source adapter's original-column link.
                let zero_bytes = (usize::BITS - maximum.leading_zeros()).div_ceil(8);
                return (offset < 8 - zero_bytes as usize).then_some(ShaFrameByteV1::Constant(0));
            }
            FieldV1::RawLength => {
                return match offset {
                    0..=5 => Some(ShaFrameByteV1::Constant(0)),
                    6 => Some(ShaFrameByteV1::RawLengthHigh),
                    7 => Some(ShaFrameByteV1::RawLengthLow),
                    _ => None,
                };
            }
        };
        if offset < 8 {
            return Some(ShaFrameByteV1::Constant((len as u64).to_be_bytes()[offset]));
        }
        offset -= 8;
        if offset < len {
            return match field {
                FieldV1::Constant(bytes) => Some(ShaFrameByteV1::Constant(bytes[offset])),
                _ => None,
            };
        }
        offset -= len;
    }
    None
}
/// Return a fixed byte expectation without allocating or reading private data.
///
/// Raw certificate/CRL TBS calls have no added frame. Publicly inactive
/// projection calls hash the empty message and therefore have no frame bytes.
pub(crate) fn sha_frame_byte_v1(
    manifest: ZkX509ShaCallManifestV1,
    offset: usize,
) -> Option<ShaFrameByteV1> {
    use FieldV1::{Constant as C, RawLength as R, Unfixed as P, Variable as V};
    use ZkX509ShaCallRoleV1 as Role;
    if manifest.activation == ZkX509ShaCallActivationV1::Inactive {
        return None;
    }
    let version = ZK_X509_RELATION_VERSION_V1.to_be_bytes();
    let governance_version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
    match manifest.role {
        Role::CertificateTbs(_) | Role::CrlTbs => None,
        Role::CrlCommitment => {
            field_frame_byte_v1(ZK_X509_CRL_DER_DIGEST_DOMAIN_V1, 1, &[R], offset)
        }
        Role::CrlIssuerSpki => field_frame_byte_v1(
            ZK_X509_CRL_ISSUER_SPKI_DIGEST_DOMAIN_V1,
            1,
            &[P(91)],
            offset,
        ),
        Role::Projection(0) => field_frame_byte_v1(
            ZK_X509_SCOPED_KEY_DOMAIN_V1,
            8,
            &[
                C(ZK_X509_SUITE_V1),
                C(ZK_X509_SOURCE_PROFILE_V1),
                C(&version),
                P(32),
                P(32),
                P(32),
                P(32),
                P(91),
            ],
            offset,
        ),
        Role::Projection(1) => field_frame_byte_v1(
            ZK_X509_NULLIFIER_DOMAIN_V1,
            5,
            &[
                C(ZK_X509_SUITE_V1),
                P(32),
                P(32),
                P(91),
                V(ZK_X509_MAX_SERIAL_BYTES_V1),
            ],
            offset,
        ),
        Role::Projection(2..=5) => field_frame_byte_v1(
            ZK_X509_ATTRIBUTE_DOMAIN_V1,
            6,
            &[
                C(ZK_X509_SUITE_V1),
                P(32),
                P(32),
                P(1),
                V(ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1),
            ],
            offset,
        ),
        Role::Projection(6) => field_frame_byte_v1(
            ZK_X509_OWNERSHIP_DOMAIN_V1,
            7,
            &[
                C(ZK_X509_SUITE_V1),
                C(ZK_X509_SOURCE_PROFILE_V1),
                C(&version),
                P(32),
                V(super::super::projection_air::ZK_X509_PROJECTION_HASH_BUFFER_BYTES_V1),
            ],
            offset,
        ),
        Role::Projection(_) => None,
        Role::TrustAnchorRecord => field_frame_byte_v1(
            ZK_X509_TRUST_ANCHOR_RECORD_DIGEST_DOMAIN_V1,
            8,
            &[
                C(&governance_version),
                P(32),
                P(8),
                P(32),
                P(32),
                P(8),
                P(33),
                P(1),
            ],
            offset,
        ),
        Role::CertificatePolicyRecord => field_frame_byte_v1(
            ZK_X509_CERTIFICATE_POLICY_RECORD_DIGEST_DOMAIN_V1,
            10,
            &[
                C(&governance_version),
                P(32),
                P(32),
                P(8),
                P(32),
                P(1),
                V(4),
            ],
            offset,
        ),
        Role::CrlRecord => field_frame_byte_v1(
            ZK_X509_CRL_RECORD_DIGEST_DOMAIN_V1,
            11,
            &[
                C(&governance_version),
                P(32),
                P(32),
                P(8),
                P(8),
                P(32),
                P(32),
                P(8),
                P(8),
                P(33),
                P(1),
            ],
            offset,
        ),
        Role::CaLeaf => field_frame_byte_v1(ZK_X509_CA_LEAF_DOMAIN_V1, 1, &[P(91)], offset),
        Role::CaNode(level) => field_frame_byte_v1(
            ZK_X509_CA_NODE_DOMAIN_V1,
            3,
            &[C(&(u16::from(level) + 1).to_be_bytes()), P(32), P(32)],
            offset,
        ),
    }
}
