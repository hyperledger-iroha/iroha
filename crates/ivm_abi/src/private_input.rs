//! Canonical typed private-input records for Kotodama ABI V1.
//!
//! A host supplies one canonical Norito record per private input.  The record
//! carries an explicit nominal numeric kind and that kind's complete canonical
//! schema-bound numeric frame.  Runtime validation must compare the requested
//! kind before publishing a private pointer into guest memory.
use norito::{Decode, Encode};
use crate::pointer_abi::PointerType;
/// ABI/domain version bound into private numeric projections and commitments.
pub const PRIVATE_INPUT_ABI_VERSION_V1: u16 = 1;
/// Nominal Norito schema name for one typed private-input record.
pub const PRIVATE_INPUT_RECORD_NAME_V1: &str = "iroha.kotodama.PrivateInputRecordV1";
/// Hard bound for one encoded outer private-input record.
///
/// The largest V1 numeric frame is substantially smaller.  The additional
/// room covers the fixed Norito header and record fields without permitting an
/// attacker-controlled allocation proportional to an unbounded length.
pub const MAX_PRIVATE_INPUT_RECORD_BYTES_V1: usize = 512;
/// Maximum private-input records accepted by one V1 host invocation.
pub const MAX_PRIVATE_INPUTS_V1: usize = 64;
/// Maximum encoded private-input bytes retained by one V1 host invocation.
///
/// This transport bound is checked before the host retains any caller-owned
/// record bytes. It is intentionally derived from consensus-visible V1 limits
/// rather than allocator behaviour.
pub const MAX_PRIVATE_INPUT_TRANSPORT_BYTES_V1: usize =
    MAX_PRIVATE_INPUTS_V1 * MAX_PRIVATE_INPUT_RECORD_BYTES_V1;
/// Domain separator for the opaque, still-private numeric projection.
pub const PRIVATE_NUMERIC_PROJECTION_DOMAIN_V1: &[u8] = b"KOTODAMA_PRIVATE_NUMERIC_PROJECTION_V1\0";
/// Domain separator for the approved full-width Pedersen commitment flow.
pub const PRIVATE_NUMERIC_VALCOM_DOMAIN_V1: &[u8] = b"KOTODAMA_PRIVATE_NUMERIC_VALCOM_V1\0";
/// BLS12-381 hash-to-curve domain used to derive the independent V1 blinding generator.
pub const PRIVATE_NUMERIC_VALCOM_H_DST_V1: &[u8] = b"IROHA_IVM_PEDERSEN_G1_XMD:SHA-256_SSWU_RO_V1";
/// Fixed hash-to-curve message used to derive the independent V1 blinding generator.
pub const PRIVATE_NUMERIC_VALCOM_H_MESSAGE_V1: &[u8] = b"KOTODAMA_PRIVATE_NUMERIC_VALCOM_H_V1";
/// Canonical compressed BLS12-381 G1 encoding of the V1 blinding generator.
///
/// Runtime code decodes this fixed consensus value directly. Tests separately
/// prove that it equals the specified hash-to-curve derivation, preventing a
/// dependency upgrade from silently changing deployed commitment semantics.
pub const PRIVATE_NUMERIC_VALCOM_H_COMPRESSED_V1: [u8; 48] = [
    0x89, 0x2a, 0x15, 0x52, 0x9e, 0x5d, 0x0a, 0x92, 0x0b, 0x47, 0x65, 0xf5, 0x78, 0x51, 0x9d, 0x79,
    0xa1, 0x93, 0x90, 0x35, 0x53, 0xc1, 0xe6, 0x67, 0x6a, 0x3c, 0xc9, 0xb8, 0xc8, 0x9f, 0xc1, 0x97,
    0x0c, 0xba, 0x9e, 0x5f, 0x64, 0x8b, 0xdf, 0xd7, 0x44, 0x0d, 0xd7, 0xd9, 0xef, 0xb6, 0x2e, 0x26,
];
/// Exact numeric payload kind carried by a private-input record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum PrivateInputKindV1 {
    /// Kotodama `int` encoded as `IntValueV1`.
    #[codec(index = 0)]
    Int,
    /// Kotodama `decimal` encoded as `DecimalValueV1`.
    #[codec(index = 1)]
    Decimal,
    /// Kotodama `quantity` encoded as `QuantityValueV1`.
    #[codec(index = 2)]
    Quantity,
}
impl PrivateInputKindV1 {
    /// Decode the stable register tag used by `GET_PRIVATE_INPUT`.
    #[must_use]
    pub const fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            0 => Self::Int,
            1 => Self::Decimal,
            2 => Self::Quantity,
            _ => return None,
        })
    }
    /// Return the stable register/Norito tag for this kind.
    #[must_use]
    pub const fn tag(self) -> u64 {
        match self {
            Self::Int => 0,
            Self::Decimal => 1,
            Self::Quantity => 2,
        }
    }
    /// Return the canonical pointer-ABI type published for this kind.
    #[must_use]
    pub const fn pointer_type(self) -> PointerType {
        match self {
            Self::Int => PointerType::Int,
            Self::Decimal => PointerType::Decimal,
            Self::Quantity => PointerType::Quantity,
        }
    }
}
/// One canonical typed private input.
///
/// `payload` is a complete schema-bound V1 numeric frame, not an unframed
/// mantissa and not JSON.  Constructing this Rust value does not validate
/// untrusted bytes; the VM host always performs bounded canonical validation
/// after gas has been debited.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kotodama.PrivateInputRecordV1")]
pub struct PrivateInputRecordV1 {
    /// Nominal numeric payload kind.
    pub kind: PrivateInputKindV1,
    /// Complete canonical `IntValueV1`, `DecimalValueV1`, or `QuantityValueV1` frame.
    pub payload: Vec<u8>,
}
impl PrivateInputRecordV1 {
    /// Construct an unvalidated record for later canonical host validation.
    #[must_use]
    pub const fn new(kind: PrivateInputKindV1, payload: Vec<u8>) -> Self {
        Self { kind, payload }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kind_tags_and_pointer_types_are_stable() {
        for (kind, tag, pointer_type) in [
            (PrivateInputKindV1::Int, 0, PointerType::Int),
            (PrivateInputKindV1::Decimal, 1, PointerType::Decimal),
            (PrivateInputKindV1::Quantity, 2, PointerType::Quantity),
        ] {
            assert_eq!(kind.tag(), tag);
            assert_eq!(PrivateInputKindV1::from_tag(tag), Some(kind));
            assert_eq!(kind.pointer_type(), pointer_type);
        }
        assert_eq!(PrivateInputKindV1::from_tag(3), None);
    }
    #[test]
    fn norito_kind_discriminants_match_register_tags() {
        for kind in [
            PrivateInputKindV1::Int,
            PrivateInputKindV1::Decimal,
            PrivateInputKindV1::Quantity,
        ] {
            let encoded = norito::codec::Encode::encode(&kind);
            assert_eq!(
                u32::from_le_bytes(encoded[..4].try_into().unwrap()),
                kind.tag() as u32
            );
        }
    }
    #[test]
    fn private_input_record_uses_its_nominal_v1_schema() {
        assert_eq!(
            <PrivateInputRecordV1 as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(PRIVATE_INPUT_RECORD_NAME_V1)
        );
    }
}
