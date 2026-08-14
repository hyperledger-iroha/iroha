//! Canonical private-witness codec for the native zk-X509 relation.
//!
//! This is deliberately not Norito and not a generic development envelope.
//! A prover accepts one fixed big-endian byte grammar so that malformed,
//! duplicate, oversized, truncated, or suffix-extended witnesses have no
//! alternate interpretation.  Proof bytes use a separate codec because they
//! are consensus-visible while this container is a local prover input.
use core::fmt;
use iroha_data_model::privacy::{ZK_X509_MAX_CERTIFICATE_BYTES_V1, ZK_X509_MAX_CHAIN_BYTES_V1};
use thiserror::Error;
use zeroize::Zeroize;
use super::{
    der_air::ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1,
    merkle::{
        ZK_X509_CA_COMPACT_TREE_CAPACITY_V1, ZK_X509_CA_COMPACT_TREE_DEPTH_V1,
        ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1, ZkX509CaMembershipPathV1,
    },
    profile::{
        ZK_X509_ATTRIBUTE_SALT_BYTES_V1, ZK_X509_MAX_CHAIN_DEPTH_V1, ZK_X509_MAX_CRL_BYTES_V1,
        ZK_X509_MIN_CHAIN_DEPTH_V1, ZK_X509_RELATION_VERSION_V1,
    },
};
const WITNESS_MAGIC_V1: [u8; 8] = *b"IRX509W1";
pub(crate) const ZK_X509_WALLET_SIGNATURE_RS_BYTES_V1: usize = 64;
const MAX_DISCLOSED_ATTRIBUTES_V1: usize = 4;
const _: () = {
    assert!(
        ZK_X509_MAX_CERTIFICATE_BYTES_V1 as usize
            == ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
    );
    assert!(
        ZK_X509_MAX_CHAIN_BYTES_V1 as usize
            == ZK_X509_MAX_CHAIN_DEPTH_V1 * ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
    );
    assert!(ZK_X509_MAX_CRL_BYTES_V1 == ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1);
    assert!(ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1 == ZK_X509_MAX_CRL_BYTES_V1);
};
/// One private opening of a publicly committed subject attribute.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ZkX509AttributeOpeningV1 {
    /// Closed attribute index (`0=C`, `1=O`, `2=OU`, `3=CN`).
    pub(crate) index: u8,
    /// Fixed-width private commitment salt.
    pub(crate) salt: [u8; ZK_X509_ATTRIBUTE_SALT_BYTES_V1],
}
impl fmt::Debug for ZkX509AttributeOpeningV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ZkX509AttributeOpeningV1")
            .field("index", &self.index)
            .field("salt", &"[REDACTED]")
            .finish()
    }
}
impl Zeroize for ZkX509AttributeOpeningV1 {
    fn zeroize(&mut self) {
        self.index.zeroize();
        self.salt.zeroize();
    }
}
/// Complete bounded private input to the native reference relation and prover.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ZkX509WitnessV1 {
    /// Exact DER certificates ordered leaf first and root last.
    pub(crate) certificate_chain_der: Vec<Vec<u8>>,
    /// Exact complete signed base-CRL DER for the leaf issuer.
    pub(crate) crl_der: Vec<u8>,
    /// Governed root-CA compact-tree membership witness.
    pub(crate) ca_membership_path: ZkX509CaMembershipPathV1,
    /// Fresh low-`s` P-256 signature by the leaf subject key as fixed
    /// canonical `r || s`.
    pub(crate) wallet_ownership_signature_rs: [u8; ZK_X509_WALLET_SIGNATURE_RS_BYTES_V1],
    /// Private attribute salts in strict disclosed-index order.
    pub(crate) attribute_openings: Vec<ZkX509AttributeOpeningV1>,
}
impl fmt::Debug for ZkX509WitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ZkX509WitnessV1 { [REDACTED] }")
    }
}
impl Zeroize for ZkX509WitnessV1 {
    fn zeroize(&mut self) {
        for certificate in &mut self.certificate_chain_der {
            certificate.zeroize();
        }
        self.certificate_chain_der.clear();
        self.crl_der.zeroize();
        self.ca_membership_path.index.zeroize();
        self.ca_membership_path.siblings.zeroize();
        self.wallet_ownership_signature_rs.zeroize();
        self.attribute_openings.zeroize();
    }
}
impl Drop for ZkX509WitnessV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}
/// Canonical witness decoding or encoding failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509WitnessCodecErrorV1 {
    /// The fixed witness magic or relation version does not match.
    #[error("zk-X509 witness header is not canonical")]
    InvalidHeader,
    /// The certificate count is outside the fixed two-to-three range.
    #[error("zk-X509 witness certificate-chain depth is invalid")]
    InvalidChainDepth,
    /// A certificate length is zero, oversized, or not representable.
    #[error("zk-X509 witness certificate length is invalid")]
    InvalidCertificateLength,
    /// The complete CRL length is zero, oversized, or not representable.
    #[error("zk-X509 witness CRL length is invalid")]
    InvalidCrlLength,
    /// The private sorted-leaf index is outside the twelve-bit tree.
    #[error("zk-X509 witness CA membership index is invalid")]
    InvalidCaPathIndex,
    /// Attribute openings are out of range, duplicated, or out of order.
    #[error("zk-X509 witness attribute openings are not canonical")]
    InvalidAttributeOpenings,
    /// A declared field extends beyond the available input.
    #[error("zk-X509 witness is truncated")]
    Truncated,
    /// Bytes remain after the sole canonical witness.
    #[error("zk-X509 witness has trailing bytes")]
    TrailingBytes,
    /// The encoded witness size overflows the platform representation.
    #[error("zk-X509 witness length overflows")]
    LengthOverflow,
}
impl ZkX509WitnessV1 {
    /// Encode one canonical local prover witness.
    pub(crate) fn encode_v1(&self) -> Result<Vec<u8>, ZkX509WitnessCodecErrorV1> {
        validate_witness_shape_v1(self)?;
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&WITNESS_MAGIC_V1);
        encoded.extend_from_slice(&ZK_X509_RELATION_VERSION_V1.to_be_bytes());
        encoded.push(
            u8::try_from(self.certificate_chain_der.len())
                .map_err(|_| ZkX509WitnessCodecErrorV1::InvalidChainDepth)?,
        );
        for certificate in &self.certificate_chain_der {
            push_u32_len_prefixed(&mut encoded, certificate)
                .map_err(|_| ZkX509WitnessCodecErrorV1::InvalidCertificateLength)?;
        }
        push_u32_len_prefixed(&mut encoded, &self.crl_der)
            .map_err(|_| ZkX509WitnessCodecErrorV1::InvalidCrlLength)?;
        encoded.extend_from_slice(&self.ca_membership_path.index.to_be_bytes());
        for sibling in &self.ca_membership_path.siblings {
            encoded.extend_from_slice(sibling);
        }
        encoded.extend_from_slice(&self.wallet_ownership_signature_rs);
        encoded.push(
            u8::try_from(self.attribute_openings.len())
                .map_err(|_| ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)?,
        );
        for opening in &self.attribute_openings {
            encoded.push(opening.index);
            encoded.extend_from_slice(&opening.salt);
        }
        Ok(encoded)
    }
    /// Decode exactly one canonical local prover witness.
    pub(crate) fn decode_exact_v1(encoded: &[u8]) -> Result<Self, ZkX509WitnessCodecErrorV1> {
        let mut reader = WitnessReaderV1::new(encoded);
        if reader.take(WITNESS_MAGIC_V1.len())? != WITNESS_MAGIC_V1
            || reader.read_u16()? != ZK_X509_RELATION_VERSION_V1
        {
            return Err(ZkX509WitnessCodecErrorV1::InvalidHeader);
        }
        let chain_depth = usize::from(reader.read_u8()?);
        if !(ZK_X509_MIN_CHAIN_DEPTH_V1..=ZK_X509_MAX_CHAIN_DEPTH_V1).contains(&chain_depth) {
            return Err(ZkX509WitnessCodecErrorV1::InvalidChainDepth);
        }
        let mut certificate_chain_der = Vec::with_capacity(chain_depth);
        for _ in 0..chain_depth {
            let certificate = reader.read_u32_len_prefixed(
                1,
                ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCertificateLength,
            )?;
            certificate_chain_der.push(certificate.to_vec());
        }
        let crl_der = reader
            .read_u32_len_prefixed(
                1,
                ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCrlLength,
            )?
            .to_vec();
        let index = reader.read_u16()?;
        if usize::from(index) >= ZK_X509_CA_COMPACT_TREE_CAPACITY_V1 {
            return Err(ZkX509WitnessCodecErrorV1::InvalidCaPathIndex);
        }
        let mut siblings = Vec::with_capacity(ZK_X509_CA_COMPACT_TREE_DEPTH_V1);
        for _ in 0..ZK_X509_CA_COMPACT_TREE_DEPTH_V1 {
            let sibling: [u8; 32] = reader
                .take(32)?
                .try_into()
                .map_err(|_| ZkX509WitnessCodecErrorV1::Truncated)?;
            siblings.push(sibling);
        }
        let wallet_ownership_signature_rs = reader
            .take(ZK_X509_WALLET_SIGNATURE_RS_BYTES_V1)?
            .try_into()
            .map_err(|_| ZkX509WitnessCodecErrorV1::Truncated)?;
        let opening_count = usize::from(reader.read_u8()?);
        if opening_count > MAX_DISCLOSED_ATTRIBUTES_V1 {
            return Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings);
        }
        let mut attribute_openings = Vec::with_capacity(opening_count);
        for _ in 0..opening_count {
            let index = reader.read_u8()?;
            let salt: [u8; ZK_X509_ATTRIBUTE_SALT_BYTES_V1] = reader
                .take(ZK_X509_ATTRIBUTE_SALT_BYTES_V1)?
                .try_into()
                .map_err(|_| ZkX509WitnessCodecErrorV1::Truncated)?;
            attribute_openings.push(ZkX509AttributeOpeningV1 { index, salt });
        }
        if !reader.is_empty() {
            return Err(ZkX509WitnessCodecErrorV1::TrailingBytes);
        }
        let witness = Self {
            certificate_chain_der,
            crl_der,
            ca_membership_path: ZkX509CaMembershipPathV1 {
                index,
                siblings: siblings
                    .try_into()
                    .map_err(|_: Vec<[u8; 32]>| ZkX509WitnessCodecErrorV1::InvalidCaPathIndex)?,
            },
            wallet_ownership_signature_rs,
            attribute_openings,
        };
        validate_witness_shape_v1(&witness)?;
        Ok(witness)
    }
}
fn validate_witness_shape_v1(witness: &ZkX509WitnessV1) -> Result<(), ZkX509WitnessCodecErrorV1> {
    if !(ZK_X509_MIN_CHAIN_DEPTH_V1..=ZK_X509_MAX_CHAIN_DEPTH_V1)
        .contains(&witness.certificate_chain_der.len())
    {
        return Err(ZkX509WitnessCodecErrorV1::InvalidChainDepth);
    }
    if witness.certificate_chain_der.iter().any(|certificate| {
        certificate.is_empty()
            || certificate.len() > ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
    }) {
        return Err(ZkX509WitnessCodecErrorV1::InvalidCertificateLength);
    }
    if witness.crl_der.is_empty() || witness.crl_der.len() > ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1
    {
        return Err(ZkX509WitnessCodecErrorV1::InvalidCrlLength);
    }
    if usize::from(witness.ca_membership_path.index) >= ZK_X509_CA_COMPACT_TREE_CAPACITY_V1 {
        return Err(ZkX509WitnessCodecErrorV1::InvalidCaPathIndex);
    }
    if witness.attribute_openings.len() > MAX_DISCLOSED_ATTRIBUTES_V1
        || witness
            .attribute_openings
            .iter()
            .any(|opening| opening.index >= MAX_DISCLOSED_ATTRIBUTES_V1 as u8)
        || witness
            .attribute_openings
            .windows(2)
            .any(|pair| pair[0].index >= pair[1].index)
    {
        return Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings);
    }
    Ok(())
}
fn push_u32_len_prefixed(
    encoded: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), ZkX509WitnessCodecErrorV1> {
    let len = u32::try_from(value.len()).map_err(|_| ZkX509WitnessCodecErrorV1::LengthOverflow)?;
    encoded.extend_from_slice(&len.to_be_bytes());
    encoded.extend_from_slice(value);
    Ok(())
}
struct WitnessReaderV1<'a> {
    remaining: &'a [u8],
}
impl<'a> WitnessReaderV1<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self { remaining: encoded }
    }
    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], ZkX509WitnessCodecErrorV1> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(len)
            .ok_or(ZkX509WitnessCodecErrorV1::Truncated)?;
        self.remaining = remaining;
        Ok(value)
    }
    fn read_u8(&mut self) -> Result<u8, ZkX509WitnessCodecErrorV1> {
        Ok(self.take(1)?[0])
    }
    fn read_u16(&mut self) -> Result<u16, ZkX509WitnessCodecErrorV1> {
        Ok(u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| ZkX509WitnessCodecErrorV1::Truncated)?,
        ))
    }
    fn read_u32(&mut self) -> Result<u32, ZkX509WitnessCodecErrorV1> {
        Ok(u32::from_be_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| ZkX509WitnessCodecErrorV1::Truncated)?,
        ))
    }
    fn read_u32_len_prefixed(
        &mut self,
        min: usize,
        max: usize,
        length_error: ZkX509WitnessCodecErrorV1,
    ) -> Result<&'a [u8], ZkX509WitnessCodecErrorV1> {
        let len = usize::try_from(self.read_u32()?)
            .map_err(|_| ZkX509WitnessCodecErrorV1::LengthOverflow)?;
        if !(min..=max).contains(&len) {
            return Err(length_error);
        }
        self.take(len)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn witness() -> ZkX509WitnessV1 {
        ZkX509WitnessV1 {
            certificate_chain_der: vec![vec![0x30, 0], vec![0x30, 0]],
            crl_der: vec![0x30, 0],
            ca_membership_path: ZkX509CaMembershipPathV1 {
                index: 4_095,
                siblings: core::array::from_fn(|index| [index as u8; 32]),
            },
            wallet_ownership_signature_rs: [0x30; ZK_X509_WALLET_SIGNATURE_RS_BYTES_V1],
            attribute_openings: vec![
                ZkX509AttributeOpeningV1 {
                    index: 0,
                    salt: [0x11; ZK_X509_ATTRIBUTE_SALT_BYTES_V1],
                },
                ZkX509AttributeOpeningV1 {
                    index: 3,
                    salt: [0x22; ZK_X509_ATTRIBUTE_SALT_BYTES_V1],
                },
            ],
        }
    }
    #[test]
    fn witness_codec_round_trips_exactly() {
        let witness = witness();
        let encoded = witness.encode_v1().expect("encode");
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&encoded).expect("decode"),
            witness
        );
    }
    #[test]
    fn witness_codec_rejects_every_truncation_and_suffix() {
        let encoded = witness().encode_v1().expect("encode");
        for length in 0..encoded.len() {
            assert!(matches!(
                ZkX509WitnessV1::decode_exact_v1(&encoded[..length]),
                Err(_)
            ));
        }
        let mut suffixed = encoded;
        suffixed.push(0);
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&suffixed),
            Err(ZkX509WitnessCodecErrorV1::TrailingBytes)
        );
    }
    #[test]
    fn witness_codec_rejects_noncanonical_counts_lengths_and_openings() {
        let mut shallow = witness();
        shallow.certificate_chain_der.pop();
        assert_eq!(
            shallow.encode_v1(),
            Err(ZkX509WitnessCodecErrorV1::InvalidChainDepth)
        );
        let mut oversized_crl = witness();
        oversized_crl.crl_der = vec![0; ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1 + 1];
        assert_eq!(
            oversized_crl.encode_v1(),
            Err(ZkX509WitnessCodecErrorV1::InvalidCrlLength)
        );
        let mut duplicate = witness();
        duplicate.attribute_openings[1].index = duplicate.attribute_openings[0].index;
        assert_eq!(
            duplicate.encode_v1(),
            Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );
        let mut reordered = witness();
        reordered.attribute_openings.swap(0, 1);
        assert_eq!(
            reordered.encode_v1(),
            Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );
        let mut invalid_index = witness();
        invalid_index.ca_membership_path.index =
            u16::try_from(ZK_X509_CA_COMPACT_TREE_CAPACITY_V1).expect("capacity fits u16");
        assert_eq!(
            invalid_index.encode_v1(),
            Err(ZkX509WitnessCodecErrorV1::InvalidCaPathIndex)
        );
    }
    #[test]
    fn witness_decoder_rejects_raw_header_count_and_length_attacks() {
        let encoded = witness().encode_v1().expect("encode");
        let chain_depth_offset = WITNESS_MAGIC_V1.len() + 2;
        let first_certificate_length_offset = chain_depth_offset + 1;
        let first_certificate_bytes = witness().certificate_chain_der[0].len();
        let second_certificate_length_offset =
            first_certificate_length_offset + 4 + first_certificate_bytes;
        let second_certificate_bytes = witness().certificate_chain_der[1].len();
        let crl_length_offset = second_certificate_length_offset + 4 + second_certificate_bytes;
        for offset in 0..WITNESS_MAGIC_V1.len() + 2 {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(
                ZkX509WitnessV1::decode_exact_v1(&changed).is_err(),
                "header byte {offset} mutation was accepted"
            );
        }
        for depth in [
            0,
            u8::try_from(ZK_X509_MIN_CHAIN_DEPTH_V1 - 1).expect("minimum depth"),
            u8::try_from(ZK_X509_MAX_CHAIN_DEPTH_V1 + 1).expect("maximum depth"),
            u8::MAX,
        ] {
            let mut changed = encoded.clone();
            changed[chain_depth_offset] = depth;
            assert_eq!(
                ZkX509WitnessV1::decode_exact_v1(&changed),
                Err(ZkX509WitnessCodecErrorV1::InvalidChainDepth)
            );
        }
        for (offset, maximum, expected) in [
            (
                first_certificate_length_offset,
                ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCertificateLength,
            ),
            (
                second_certificate_length_offset,
                ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCertificateLength,
            ),
            (
                crl_length_offset,
                ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCrlLength,
            ),
        ] {
            for declared in [
                0,
                u32::try_from(maximum + 1).expect("section maximum fits u32"),
                u32::MAX,
            ] {
                let mut changed = encoded.clone();
                changed[offset..offset + 4].copy_from_slice(&declared.to_be_bytes());
                assert_eq!(
                    ZkX509WitnessV1::decode_exact_v1(&changed),
                    Err(expected),
                    "declared length {declared} at {offset} was accepted"
                );
            }
        }
    }
    #[test]
    fn witness_decoder_rejects_duplicate_reordered_and_excess_openings() {
        let canonical = witness();
        let encoded = canonical.encode_v1().expect("encode");
        let opening_record_bytes = 1 + ZK_X509_ATTRIBUTE_SALT_BYTES_V1;
        let opening_count_offset =
            encoded.len() - 1 - canonical.attribute_openings.len() * opening_record_bytes;
        let first_index_offset = opening_count_offset + 1;
        let second_index_offset = first_index_offset + opening_record_bytes;
        let mut duplicate = encoded.clone();
        duplicate[second_index_offset] = duplicate[first_index_offset];
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&duplicate),
            Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );
        let mut reordered = encoded.clone();
        reordered[first_index_offset] = 3;
        reordered[second_index_offset] = 0;
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&reordered),
            Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );
        let mut excess = encoded;
        excess[opening_count_offset] =
            u8::try_from(MAX_DISCLOSED_ATTRIBUTES_V1 + 1).expect("opening cap fits u8");
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&excess),
            Err(ZkX509WitnessCodecErrorV1::InvalidAttributeOpenings)
        );
    }
    #[test]
    fn witness_codec_pins_big_endian_index_and_rejects_boundary_mutation() {
        let canonical = witness();
        let mut encoded = canonical.encode_v1().expect("encode");
        let index_offset = WITNESS_MAGIC_V1.len()
            + 2
            + 1
            + canonical
                .certificate_chain_der
                .iter()
                .map(|certificate| 4 + certificate.len())
                .sum::<usize>()
            + 4
            + canonical.crl_der.len();
        assert_eq!(&encoded[index_offset..index_offset + 2], &[0x0f, 0xff]);
        encoded[index_offset..index_offset + 2].copy_from_slice(&4_096_u16.to_be_bytes());
        assert_eq!(
            ZkX509WitnessV1::decode_exact_v1(&encoded),
            Err(ZkX509WitnessCodecErrorV1::InvalidCaPathIndex)
        );
        let mut first = witness();
        first.ca_membership_path.index = 0;
        let first_encoded = first.encode_v1().expect("first");
        assert_eq!(ZkX509WitnessV1::decode_exact_v1(&first_encoded), Ok(first));
    }
    #[test]
    fn private_witness_debug_is_redacted_and_recursive_zeroize_covers_every_field() {
        let mut witness = witness();
        let debug = format!("{witness:?}");
        assert_eq!(debug, "ZkX509WitnessV1 { [REDACTED] }");
        assert!(!debug.contains("111111"));
        assert!(!debug.contains("303030"));
        witness.zeroize();
        assert!(witness.certificate_chain_der.is_empty());
        assert!(witness.crl_der.is_empty());
        assert_eq!(witness.ca_membership_path.index, 0);
        assert!(
            witness
                .ca_membership_path
                .siblings
                .iter()
                .flatten()
                .all(|byte| *byte == 0)
        );
        assert!(
            witness
                .wallet_ownership_signature_rs
                .iter()
                .all(|byte| *byte == 0)
        );
        assert!(witness.attribute_openings.is_empty());
    }
}
