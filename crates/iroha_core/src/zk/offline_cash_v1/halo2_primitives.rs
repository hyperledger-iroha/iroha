//! Strict low-level primitives for the Offline Cash V1 Pasta verifier.
//!
//! These helpers safely parse governed transparent parameters and processed
//! verifier keys. Ordinary Poseidon proof parsing and terminal outer-plus-
//! carried-lineage decisions live in the shared recursion adapter and
//! `helper_recursion`; no augmented proof or delayed-history ABI remains.

use core::fmt;
use std::{
    io::{self, Cursor, Write},
    panic::{AssertUnwindSafe, catch_unwind},
};

use halo2_proofs::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    halo2curves::{
        CurveAffine,
        ff::{Field, FromUniformBytes},
        pasta::{EpAffine, EqAffine},
    },
    plonk::{Circuit, ConstraintSystem, VerifyingKey},
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use iroha_data_model::offline::{
    OFFLINE_CASH_HALO2_K_V1, OFFLINE_CASH_P256_V3_HALO2_K_V1, OFFLINE_CASH_PARAMS_BYTES_V1,
    OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1,
};

const POINT_BYTES: usize = 32;
const PROCESSED_VK_VERSION: u8 = 0x02;
const PROCESSED_VK_HEADER_BYTES: usize = 10;
const UNCOMPRESSED_SELECTORS: u8 = 0;

const _: () = assert!(OFFLINE_CASH_HALO2_K_V1 == 16);

/// Strict parsing or native-verification failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OfflineCashHalo2PrimitiveErrorV1 {
    /// The serialized parameters have the wrong fixed shape.
    InvalidParameterShape,
    /// A bounded parameter payload could not be decoded.
    InvalidParameterEncoding,
    /// Decoding and canonical reserialization differed.
    NonCanonicalParameterEncoding,
    /// Parameters differ from the transparent deterministic derivation.
    NonTransparentParameters,
    /// The processed verifier key has the wrong bounded header or circuit shape.
    InvalidVerifierKeyShape,
    /// A processed verifier key could not be decoded.
    InvalidVerifierKeyEncoding,
    /// Decoding and processed reserialization differed.
    NonCanonicalVerifierKeyEncoding,
}

impl fmt::Display for OfflineCashHalo2PrimitiveErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidParameterShape => "invalid offline-cash IPA parameter shape",
            Self::InvalidParameterEncoding => "invalid offline-cash IPA parameter encoding",
            Self::NonCanonicalParameterEncoding => {
                "non-canonical offline-cash IPA parameter encoding"
            }
            Self::NonTransparentParameters => {
                "offline-cash IPA parameters differ from transparent derivation"
            }
            Self::InvalidVerifierKeyShape => "invalid offline-cash processed verifier-key shape",
            Self::InvalidVerifierKeyEncoding => {
                "invalid offline-cash processed verifier-key encoding"
            }
            Self::NonCanonicalVerifierKeyEncoding => {
                "non-canonical offline-cash processed verifier-key encoding"
            }
        })
    }
}

impl std::error::Error for OfflineCashHalo2PrimitiveErrorV1 {}

struct ExactBytesWriter<'a> {
    expected: &'a [u8],
    offset: usize,
}

impl ExactBytesWriter<'_> {
    fn finish(self) -> io::Result<()> {
        if self.offset == self.expected.len() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialization ended before the expected bytes",
            ))
        }
    }
}

impl Write for ExactBytesWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self.offset.checked_add(bytes.len()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "serialization length overflow")
        })?;
        if self.expected.get(self.offset..end) != Some(bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialization differs from the expected bytes",
            ));
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn exact_params_length(k: u32) -> Option<usize> {
    if k >= usize::BITS {
        return None;
    }
    let rows = 1_usize.checked_shl(k)?;
    rows.checked_mul(2)?
        .checked_add(2)?
        .checked_mul(POINT_BYTES)?
        .checked_add(4)
}

fn params_write_matches<C: CurveAffine>(params: &ParamsIPA<C>, expected: &[u8]) -> bool {
    let mut writer = ExactBytesWriter {
        expected,
        offset: 0,
    };
    params.write(&mut writer).is_ok() && writer.finish().is_ok()
}

pub(super) fn parse_params_exact_for_k<C>(
    bytes: &[u8],
    expected_k: u32,
) -> Result<ParamsIPA<C>, OfflineCashHalo2PrimitiveErrorV1>
where
    C: CurveAffine,
{
    let expected_len = exact_params_length(expected_k)
        .ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape)?;
    if expected_k >= 32 || bytes.len() != expected_len || bytes.len() < 4 {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    let encoded_k = u32::from_le_bytes(
        bytes[..4]
            .try_into()
            .expect("four-byte preflight slice has exact length"),
    );
    if encoded_k != expected_k {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }

    let mut cursor = Cursor::new(bytes);
    let parsed = catch_unwind(AssertUnwindSafe(|| ParamsIPA::<C>::read(&mut cursor)))
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidParameterEncoding)?
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidParameterEncoding)?;
    if cursor.position() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || parsed.k() != expected_k
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterEncoding);
    }
    if !params_write_matches(&parsed, bytes) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::NonCanonicalParameterEncoding);
    }
    drop(parsed);

    let canonical = catch_unwind(AssertUnwindSafe(|| ParamsIPA::<C>::new(expected_k)))
        .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::NonTransparentParameters)?;
    if !params_write_matches(&canonical, bytes) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::NonTransparentParameters);
    }
    Ok(canonical)
}

/// Parse the exact authenticated common-k16 Eq parameters.
pub(super) fn parse_offline_cash_eq_params_v1(
    bytes: &[u8],
) -> Result<ParamsIPA<EqAffine>, OfflineCashHalo2PrimitiveErrorV1> {
    if u64::try_from(bytes.len()).ok() != Some(OFFLINE_CASH_PARAMS_BYTES_V1) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    parse_params_exact_for_k(bytes, OFFLINE_CASH_HALO2_K_V1)
}

/// Parse the exact authenticated common-k16 Ep parameters.
pub(super) fn parse_offline_cash_ep_params_v1(
    bytes: &[u8],
) -> Result<ParamsIPA<EpAffine>, OfflineCashHalo2PrimitiveErrorV1> {
    if u64::try_from(bytes.len()).ok() != Some(OFFLINE_CASH_PARAMS_BYTES_V1) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    parse_params_exact_for_k(bytes, OFFLINE_CASH_HALO2_K_V1)
}

/// Parse the exact authenticated common-k16 Eq parameters for P-256 V3.
pub(super) fn parse_offline_cash_eq_p256_v3_params_v1(
    bytes: &[u8],
) -> Result<ParamsIPA<EqAffine>, OfflineCashHalo2PrimitiveErrorV1> {
    if u64::try_from(bytes.len()).ok() != Some(OFFLINE_CASH_PARAMS_BYTES_V1) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    parse_params_exact_for_k(bytes, OFFLINE_CASH_P256_V3_HALO2_K_V1)
}

/// Parse the exact authenticated common-k16 Ep parameters for P-256 V3.
pub(super) fn parse_offline_cash_ep_p256_v3_params_v1(
    bytes: &[u8],
) -> Result<ParamsIPA<EpAffine>, OfflineCashHalo2PrimitiveErrorV1> {
    if u64::try_from(bytes.len()).ok() != Some(OFFLINE_CASH_PARAMS_BYTES_V1) {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidParameterShape);
    }
    parse_params_exact_for_k(bytes, OFFLINE_CASH_P256_V3_HALO2_K_V1)
}

fn configured_uncompressed_fixed_columns<F, ConcreteCircuit>() -> Option<usize>
where
    F: Field,
    ConcreteCircuit: Circuit<F>,
    ConcreteCircuit::Params: Default,
{
    let mut cs = ConstraintSystem::<F>::default();
    #[cfg(feature = "circuit-params")]
    let _ = ConcreteCircuit::configure_with_params(&mut cs, ConcreteCircuit::Params::default());
    #[cfg(not(feature = "circuit-params"))]
    let _ = ConcreteCircuit::configure(&mut cs);
    cs.num_fixed_columns().checked_add(cs.num_selectors())
}

/// Preflight, parse, and canonically round-trip one processed verifier key.
///
/// This remains generic across authenticated Offline Cash circuit roles, but
/// every caller must instantiate the exact compiled `Circuit` type and degree;
/// no placeholder circuit may be used to activate production parsing.
pub(super) fn parse_processed_verifier_key_v1<C, ConcreteCircuit>(
    bytes: &[u8],
    expected_k: u32,
) -> Result<VerifyingKey<C>, OfflineCashHalo2PrimitiveErrorV1>
where
    C: SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
    ConcreteCircuit::Params: Default,
{
    if expected_k >= 32
        || bytes.len() < PROCESSED_VK_HEADER_BYTES
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX) > OFFLINE_CASH_VERIFYING_KEY_MAX_BYTES_V1
        || bytes[0] != PROCESSED_VK_VERSION
        || u32::from_le_bytes(bytes[1..5].try_into().expect("VK k slice has four bytes"))
            != expected_k
        || bytes[5] != UNCOMPRESSED_SELECTORS
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape);
    }
    let expected_fixed = configured_uncompressed_fixed_columns::<C::ScalarExt, ConcreteCircuit>()
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape)?;
    let encoded_fixed = u32::from_le_bytes(
        bytes[6..10]
            .try_into()
            .expect("VK fixed-count slice has four bytes"),
    );
    if encoded_fixed != expected_fixed {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyShape);
    }

    let mut cursor = Cursor::new(bytes);
    let key = catch_unwind(AssertUnwindSafe(|| {
        #[cfg(feature = "circuit-params")]
        {
            VerifyingKey::<C>::read::<_, ConcreteCircuit>(
                &mut cursor,
                SerdeFormat::Processed,
                ConcreteCircuit::Params::default(),
            )
        }
        #[cfg(not(feature = "circuit-params"))]
        {
            VerifyingKey::<C>::read::<_, ConcreteCircuit>(&mut cursor, SerdeFormat::Processed)
        }
    }))
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding)?
    .map_err(|_| OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding)?;
    if cursor.position() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || key.get_domain().k() != expected_k
        || key.fixed_commitments().len() != usize::try_from(expected_fixed).unwrap_or(usize::MAX)
    {
        return Err(OfflineCashHalo2PrimitiveErrorV1::InvalidVerifierKeyEncoding);
    }
    if key.to_bytes(SerdeFormat::Processed) != bytes {
        return Err(OfflineCashHalo2PrimitiveErrorV1::NonCanonicalVerifierKeyEncoding);
    }
    Ok(key)
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    pub(in crate::zk::offline_cash_v1) fn parse_params_for_k<C>(
        bytes: &[u8],
        expected_k: u32,
    ) -> Result<ParamsIPA<C>, OfflineCashHalo2PrimitiveErrorV1>
    where
        C: CurveAffine,
    {
        parse_params_exact_for_k(bytes, expected_k)
    }
}
