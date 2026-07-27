//! Canonical private-witness codec for the native zk-X509 relation.
//!
//! This is deliberately not Norito and not a generic development envelope.
//! A prover accepts one fixed big-endian byte grammar so that malformed,
//! duplicate, oversized, truncated, or suffix-extended witnesses have no
//! alternate interpretation.  Proof bytes use a separate codec because they
//! are consensus-visible while this container is a local prover input.

use thiserror::Error;

use super::{
    merkle::ZkX509CaMembershipPathV1,
    profile::{
        ZK_X509_ATTRIBUTE_SALT_BYTES_V1, ZK_X509_CA_TREE_DEPTH_V1, ZK_X509_MAX_CHAIN_DEPTH_V1,
        ZK_X509_MAX_CRL_BYTES_V1, ZK_X509_MIN_CHAIN_DEPTH_V1, ZK_X509_RELATION_VERSION_V1,
    },
};
use crate::privacy_engines::zk_x509::der::ZK_X509_DER_MAX_DOCUMENT_BYTES_V1;

const WITNESS_MAGIC_V1: [u8; 8] = *b"IRX509W1";
const MAX_P256_DER_SIGNATURE_BYTES_V1: usize = 72;
const MIN_P256_DER_SIGNATURE_BYTES_V1: usize = 8;
const MAX_DISCLOSED_ATTRIBUTES_V1: usize = 4;

/// One private opening of a publicly committed subject attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509AttributeOpeningV1 {
    /// Closed attribute index (`0=C`, `1=O`, `2=OU`, `3=CN`).
    pub(crate) index: u8,
    /// Fixed-width private commitment salt.
    pub(crate) salt: [u8; ZK_X509_ATTRIBUTE_SALT_BYTES_V1],
}

/// Complete bounded private input to the native reference relation and prover.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509WitnessV1 {
    /// Exact DER certificates ordered leaf first and root last.
    pub(crate) certificate_chain_der: Vec<Vec<u8>>,
    /// Exact complete signed base-CRL DER for the leaf issuer.
    pub(crate) crl_der: Vec<u8>,
    /// Governed root-CA sparse-tree membership witness.
    pub(crate) ca_membership_path: ZkX509CaMembershipPathV1,
    /// Fresh low-`s` P-256 signature by the leaf subject key.
    pub(crate) wallet_ownership_signature_der: Vec<u8>,
    /// Private attribute salts in strict disclosed-index order.
    pub(crate) attribute_openings: Vec<ZkX509AttributeOpeningV1>,
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
    /// The CA membership path does not contain exactly 256 siblings.
    #[error("zk-X509 witness CA membership path length is invalid")]
    InvalidCaPathLength,
    /// The wallet signature cannot be a canonical P-256 DER signature.
    #[error("zk-X509 witness wallet-signature length is invalid")]
    InvalidWalletSignatureLength,
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
        for sibling in &self.ca_membership_path.siblings {
            encoded.extend_from_slice(sibling);
        }
        let signature_len = u16::try_from(self.wallet_ownership_signature_der.len())
            .map_err(|_| ZkX509WitnessCodecErrorV1::InvalidWalletSignatureLength)?;
        encoded.extend_from_slice(&signature_len.to_be_bytes());
        encoded.extend_from_slice(&self.wallet_ownership_signature_der);
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
                ZK_X509_DER_MAX_DOCUMENT_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCertificateLength,
            )?;
            certificate_chain_der.push(certificate.to_vec());
        }
        let crl_der = reader
            .read_u32_len_prefixed(
                1,
                ZK_X509_MAX_CRL_BYTES_V1,
                ZkX509WitnessCodecErrorV1::InvalidCrlLength,
            )?
            .to_vec();

        let mut siblings = Vec::with_capacity(ZK_X509_CA_TREE_DEPTH_V1);
        for _ in 0..ZK_X509_CA_TREE_DEPTH_V1 {
            let sibling: [u8; 32] = reader
                .take(32)?
                .try_into()
                .map_err(|_| ZkX509WitnessCodecErrorV1::Truncated)?;
            siblings.push(sibling);
        }

        let signature_len = usize::from(reader.read_u16()?);
        if !(MIN_P256_DER_SIGNATURE_BYTES_V1..=MAX_P256_DER_SIGNATURE_BYTES_V1)
            .contains(&signature_len)
        {
            return Err(ZkX509WitnessCodecErrorV1::InvalidWalletSignatureLength);
        }
        let wallet_ownership_signature_der = reader.take(signature_len)?.to_vec();

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
            ca_membership_path: ZkX509CaMembershipPathV1 { siblings },
            wallet_ownership_signature_der,
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
        certificate.is_empty() || certificate.len() > ZK_X509_DER_MAX_DOCUMENT_BYTES_V1
    }) {
        return Err(ZkX509WitnessCodecErrorV1::InvalidCertificateLength);
    }
    if witness.crl_der.is_empty() || witness.crl_der.len() > ZK_X509_MAX_CRL_BYTES_V1 {
        return Err(ZkX509WitnessCodecErrorV1::InvalidCrlLength);
    }
    if witness.ca_membership_path.siblings.len() != ZK_X509_CA_TREE_DEPTH_V1 {
        return Err(ZkX509WitnessCodecErrorV1::InvalidCaPathLength);
    }
    if !(MIN_P256_DER_SIGNATURE_BYTES_V1..=MAX_P256_DER_SIGNATURE_BYTES_V1)
        .contains(&witness.wallet_ownership_signature_der.len())
    {
        return Err(ZkX509WitnessCodecErrorV1::InvalidWalletSignatureLength);
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
                siblings: (0..ZK_X509_CA_TREE_DEPTH_V1)
                    .map(|index| [index as u8; 32])
                    .collect(),
            },
            wallet_ownership_signature_der: vec![0x30; 70],
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
        oversized_crl.crl_der = vec![0; ZK_X509_MAX_CRL_BYTES_V1 + 1];
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
    }
}
