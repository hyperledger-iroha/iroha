//! Exact native Envelope framing only; this grants no monetary or delivery authority.
use super::*;

pub(super) fn convert(kind: u8, wrap: bool, original: &[u8], scheme: &[u8; 32]) -> Result<Vec<u8>> {
    if original.is_empty() || original.len() > KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1 {
        return Err(Failure::code(INVALID));
    }
    let envelope = if wrap {
        let message = match kind {
            1 => KagemushaWalletMessageV1::Offer {
                offer: decode(original)?,
            },
            2 => KagemushaWalletMessageV1::Request {
                request: decode(original)?,
            },
            3 => KagemushaWalletMessageV1::Payment {
                payment: decode(original)?,
            },
            4 => KagemushaWalletMessageV1::Credited {
                credited: decode(original)?,
            },
            _ => return Err(Failure::code(INVALID)),
        };
        KagemushaWalletEnvelopeV1::new(message)
    } else {
        KagemushaWalletEnvelopeV1::decode_canonical(original, scheme)
            .map_err(|_| Failure::code(INVALID))?
    };
    if envelope.message.tag() != kind || envelope.message.scheme_id() != scheme {
        return Err(Failure::code(INVALID));
    }
    // Enforce exact per-kind *full Envelope* bounds, including when wrapping a retained Payment.
    let frame = envelope
        .to_canonical_bytes()
        .map_err(|_| Failure::code(INVALID))?;
    if wrap {
        return Ok(frame);
    }
    let object = match envelope.message {
        KagemushaWalletMessageV1::Offer { offer } => norito::encode_canonical(&offer),
        KagemushaWalletMessageV1::Request { request } => norito::encode_canonical(&request),
        KagemushaWalletMessageV1::Payment { payment } => norito::encode_canonical(&payment),
        KagemushaWalletMessageV1::Credited { credited } => norito::encode_canonical(&credited),
        _ => return Err(Failure::code(INVALID)),
    };
    object.map_err(|_| Failure::code(INVALID))
}
fn decode<T>(original: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(original, norito::canonical_decode_limits(original.len()))
        .map_err(|_| Failure::code(INVALID))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_transport_roundtrip_and_replay_preserve_all_original_bytes() {
        let vectors: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))
        .unwrap();
        for (kind, name) in [
            (1, "KagemushaWalletOfferV1"),
            (2, "KagemushaWalletRequestV1"),
            (3, "KagemushaWalletPaymentV1"),
            (4, "KagemushaWalletCreditedV1"),
        ] {
            let row = vectors["objects"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["type"].as_str() == Some(name))
                .unwrap();
            let original = hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap();
            let scheme = match kind {
                1 => {
                    decode::<KagemushaWalletOfferV1>(&original)
                        .unwrap()
                        .body
                        .scheme_id
                }
                2 => {
                    decode::<KagemushaWalletRequestV1>(&original)
                        .unwrap()
                        .body
                        .scheme_id
                }
                3 => {
                    decode::<KagemushaWalletPaymentV1>(&original)
                        .unwrap()
                        .request
                        .body
                        .scheme_id
                }
                _ => {
                    decode::<KagemushaWalletCreditedV1>(&original)
                        .unwrap()
                        .scheme_id
                }
            };
            let frame = convert(kind, true, &original, &scheme).unwrap();
            assert_eq!(frame, convert(kind, true, &original, &scheme).unwrap());
            assert_eq!(original, convert(kind, false, &frame, &scheme).unwrap());
            assert!(frame.len() <= KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1);
            assert!(convert(kind, true, &original, &[0; 32]).is_err());
            assert!(convert(kind, false, &frame, &[0; 32]).is_err());
            assert!(convert(kind % 4 + 1, false, &frame, &scheme).is_err());
            let mut trailing = frame.clone();
            trailing.push(0);
            assert!(convert(kind, false, &trailing, &scheme).is_err());
            let mut trailing = original;
            trailing.push(0);
            assert!(convert(kind, true, &trailing, &scheme).is_err());
            assert!(convert(kind, false, &vec![0; 10_001], &scheme).is_err());
        }
    }
}
