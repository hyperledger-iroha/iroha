//! Allocation-free canonical account validation using the native key and policy relations.
//!
//! The original enclosing decoder owns framing, layout and cumulative controls. Fixed key
//! scratch is reused for each member; no account, member vector, or compact key is allocated.

use iroha_crypto::{MAX_PUBLIC_KEY_PAYLOAD_BYTES, PreparedPublicKeyDecode, PublicKey};
use norito::core::{self as ncore, DecodeIntoError, Error};

use super::{MultisigMember, MultisigPolicy, controller::CanonicalPolicyValidation};

fn flatten(error: DecodeIntoError<Error>) -> Error {
    match error {
        DecodeIntoError::Codec(error) | DecodeIntoError::Destination(error) => error,
    }
}
fn field<T, R>(
    bytes: &[u8],
    offset: &mut usize,
    read: impl FnOnce(&[u8]) -> Result<R, Error>,
) -> Result<R, Error>
where
    T: for<'de> norito::DeserializePayload<'de> + norito::SerializePayload,
{
    ncore::framed_field::<T>(bytes, offset)?
        .with_payload(|bytes| read(bytes).map_err(DecodeIntoError::Codec))
        .map_err(flatten)
}
fn finish(bytes: &[u8], offset: usize) -> Result<(), Error> {
    if offset != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    ncore::note_payload_access(bytes, offset);
    Ok(())
}
fn policy_error(_: super::MultisigPolicyError) -> Error {
    Error::InvalidValue {
        context: "account controller",
    }
}

/// Check the exact native account payload without creating its owned identity graph.
/// Native field/sequence/depth gates and the caller's original counter scope remain active.
pub(crate) fn validate(bytes: &[u8]) -> Result<(), Error> {
    ncore::with_decode_depth_scope(|| {
        ncore::with_payload_ctx(bytes, || {
            let tag = bytes.get(..4).ok_or(Error::LengthMismatch)?;
            let tag = u32::from_le_bytes(tag.try_into().map_err(|_| Error::LengthMismatch)?);
            let mut offset = 4;
            match tag {
                0 => field::<PublicKey, _>(bytes, &mut offset, |key| {
                    PreparedPublicKeyDecode::with_validated_payload(key, |_, _| Ok::<_, Error>(()))
                })?,
                1 => field::<MultisigPolicy, _>(bytes, &mut offset, policy)?,
                _ => {
                    return Err(Error::InvalidValue {
                        context: "account controller discriminant",
                    });
                }
            }
            finish(bytes, offset)
        })
    })
}

fn policy(bytes: &[u8]) -> Result<(), Error> {
    let mut offset = 0;
    let version = ncore::framed_field::<u8>(bytes, &mut offset)?.decode_owned()?;
    let threshold = ncore::framed_field::<u16>(bytes, &mut offset)?.decode_owned()?;
    field::<Vec<MultisigMember>, _>(bytes, &mut offset, |members| {
        let (count, mut offset) = ncore::read_seq_len_slice(members)?;
        let mut validation =
            CanonicalPolicyValidation::new(version, threshold, count).map_err(policy_error)?;
        let mut previous = [0; MAX_PUBLIC_KEY_PAYLOAD_BYTES];
        let mut previous_key: Option<(&'static str, usize)> = None;
        for _ in 0..count {
            field::<MultisigMember, _>(members, &mut offset, |member| {
                let mut offset = 0;
                let key = ncore::framed_field::<PublicKey>(member, &mut offset)?;
                // Retain the ordinary key-before-weight leaf validation order. Only validated
                // temporary components enter the shared serialized-policy relation.
                let order = key
                    .with_payload(|key| {
                        PreparedPublicKeyDecode::with_validated_payload(
                            key,
                            |algorithm, payload| {
                                let order = previous_key.map(|(algorithm_before, len)| {
                                    (algorithm_before, &previous[..len])
                                        .cmp(&(algorithm.as_static_str(), payload))
                                });
                                previous[..payload.len()].copy_from_slice(payload);
                                previous_key = Some((algorithm.as_static_str(), payload.len()));
                                Ok::<_, Error>(order)
                            },
                        )
                        .map_err(DecodeIntoError::Codec)
                    })
                    .map_err(flatten)?;
                let weight = ncore::framed_field::<u16>(member, &mut offset)?.decode_owned()?;
                finish(member, offset)?;
                validation
                    .member(weight, || Ok(order))
                    .map_err(policy_error)
            })?;
        }
        finish(members, offset)?;
        validation.finish().map_err(policy_error)
    })?;
    finish(bytes, offset)
}
