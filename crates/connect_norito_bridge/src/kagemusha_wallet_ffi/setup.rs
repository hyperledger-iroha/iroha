//! Typed setup input shared by C/JNI; no foreign clock or arbitrary signature body.

use super::*;

pub(crate) enum Setup {
    Bootstrap,
    Activation,
    Transport {
        kind: u8,
        wrap: bool,
        original: Vec<u8>,
    },
    Offer {
        id: [u8; 32],
        amount: u128,
    },
    Request {
        id: [u8; 32],
        offer: Vec<u8>,
        fee: Option<
            Box<(
                KagemushaWalletFeeScheduleV1,
                KagemushaWalletSignerCertificateV1,
            )>,
        >,
    },
    Credited(Vec<u8>),
    BeginTime,
    CancelTime {
        token: u64,
    },
    FinishTime {
        token: u64,
        anchor: Vec<u8>,
        certificate: Vec<u8>,
    },
}
pub(crate) fn bounds(selector: u32) -> Result<[usize; 3]> {
    Ok(match selector {
        0 | 1 | 4 | 6 | 15 => [0; 3],
        2 => [
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_FEE_SCHEDULE_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
        ],
        3 | 7..=14 => [KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, 0, 0],
        5 => [
            KAGEMUSHA_WALLET_TIME_ANCHOR_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
            0,
        ],
        _ => return Err(Failure::code(INVALID)),
    })
}
fn decode<T>(bytes: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| Failure::code(INVALID))
}
pub(crate) fn request(
    id: &[u8],
    selector: u32,
    amount: u128,
    token: u64,
    originals: [&[u8]; 3],
) -> Result<Setup> {
    let id: [u8; 32] = id.try_into().map_err(|_| Failure::code(INVALID))?;
    let bounds = bounds(selector)?;
    if ((1..=2).contains(&selector) != (id != [0; 32]))
        || ((selector == 1) != (amount != 0))
        || (matches!(selector, 5 | 6) != (token != 0))
        || originals
            .iter()
            .zip(bounds)
            .any(|(bytes, bound)| bytes.len() > bound)
    {
        return Err(Failure::code(INVALID));
    }
    let [first, second, third] = originals;
    Ok(match selector {
        0 => Setup::Bootstrap,
        15 => Setup::Activation,
        1 => Setup::Offer { id, amount },
        2 if !first.is_empty() && second.is_empty() == third.is_empty() => Setup::Request {
            id,
            offer: first.to_vec(),
            fee: if second.is_empty() {
                None
            } else {
                Some(Box::new((decode(second)?, decode(third)?)))
            },
        },
        3 if !first.is_empty() => Setup::Credited(first.to_vec()),
        4 => Setup::BeginTime,
        6 => Setup::CancelTime { token },
        7..=14 if !first.is_empty() => Setup::Transport {
            kind: ((selector - 7) % 4 + 1) as u8,
            wrap: selector < 11,
            original: first.to_vec(),
        },
        5 if !first.is_empty() && !second.is_empty() => Setup::FinishTime {
            token,
            // Decode only after the same owner's one-use exchange has been consumed.
            anchor: first.to_vec(),
            certificate: second.to_vec(),
        },
        _ => return Err(Failure::code(INVALID)),
    })
}
fn take_time<T>(times: &mut BTreeMap<u64, T>, token: u64) -> Result<T> {
    times.remove(&token).ok_or(Failure::code(INVALID))
}
fn take_time_response<T>(
    times: &mut BTreeMap<u64, T>,
    token: u64,
    anchor: &[u8],
    certificate: &[u8],
) -> Result<(
    T,
    KagemushaWalletTimeAnchorV1,
    KagemushaWalletSignerCertificateV1,
)> {
    // Malformed response bytes must retire the exchange too: the SDK has already
    // consumed its token, so leaving this entry live would make it unreachable.
    let exchange = take_time(times, token)?;
    Ok((exchange, decode(anchor)?, decode(certificate)?))
}
impl<P, S> NativeWallet<P, S>
where
    P: advance::KagemushaWalletPlatformV1,
    S: OriginalSourceV1 + Send,
{
    pub(super) fn setup_inner(&mut self, input: Setup) -> Result<Response> {
        let bytes = match input {
            Setup::Transport {
                kind,
                wrap,
                original,
            } => {
                let scheme = self.wallet.snapshot()?.scheme_id;
                transport::convert(kind, wrap, &original, &scheme)?
            }
            Setup::Bootstrap => return Ok(completion(Some(self.wallet.bootstrap()?))),
            Setup::Activation => {
                return Ok(Response {
                    kind: 17,
                    bytes: self.wallet.activation()?,
                    ..Response::default()
                });
            }
            Setup::Offer { id, amount } => self.wallet.offer(id, amount)?,
            Setup::Request { id, offer, fee } => {
                self.wallet.request(id, &offer, fee.map(|value| *value))?
            }
            Setup::Credited(bytes) => {
                return Ok(completion(Some(self.wallet.accept_credited(&bytes)?)));
            }
            Setup::BeginTime => {
                if self.times.len() >= 32 {
                    return Err(Failure::code(RESOURCE));
                }
                let token = self
                    .next_time
                    .checked_add(1)
                    .filter(|value| *value <= i64::MAX as u64)
                    .ok_or(Failure::code(RESOURCE))?;
                let exchange = self.wallet.begin_direct_time_exchange()?;
                let nonce = exchange.nonce().to_vec();
                self.next_time = token;
                self.times.insert(token, exchange);
                return Ok(Response {
                    kind: 13,
                    sequence: u128::from(token),
                    bytes: nonce,
                    ..Response::default()
                });
            }
            Setup::CancelTime { token } => {
                take_time(&mut self.times, token)?;
                return Ok(Response {
                    kind: 6,
                    ..Response::default()
                });
            }
            Setup::FinishTime {
                token,
                anchor,
                certificate,
            } => {
                let (exchange, anchor, certificate) =
                    take_time_response(&mut self.times, token, &anchor, &certificate)?;
                self.wallet
                    .finish_direct_time_exchange(exchange, anchor, &certificate)?;
                return Ok(Response {
                    kind: 14,
                    ..Response::default()
                });
            }
        };
        Ok(Response {
            kind: 12,
            bytes,
            ..Response::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_intake_rejects_unused_authority_fields_and_wrong_bounds() {
        for selector in [0, 1, 3, 4, 15] {
            let id = if selector == 1 { [1; 32] } else { [0; 32] };
            let amount = if selector == 1 { u128::MAX } else { 0 };
            let first: &[u8] = if selector == 3 { &[7] } else { &[] };
            assert!(request(&id, selector, amount, 0, [first, &[], &[]]).is_ok());
            assert!(request(&id, selector, amount, 1, [first, &[], &[]]).is_err());
            assert!(request(&id, selector, amount, 0, [first, &[7], &[]]).is_err());
        }
        assert!(request(&[1; 32], 2, 0, 0, [&[7], &[], &[]]).is_ok());
        assert!(request(&[1; 32], 2, 0, 0, [&[7], &[7], &[]]).is_err());
        assert!(request(&[0; 32], 2, 0, 0, [&[7], &[], &[]]).is_err());
        assert!(request(&[1; 31], 1, 1, 0, [&[]; 3]).is_err());
        assert!(request(&[0; 32], 5, 0, 1, [&[]; 3]).is_err());
        assert!(request(&[0; 32], 6, 0, 1, [&[]; 3]).is_ok());
        assert!(request(&[0; 32], 6, 0, 0, [&[]; 3]).is_err());
        assert!(request(&[1; 32], 6, 0, 1, [&[]; 3]).is_err());
        assert!(request(&[0; 32], 6, 1, 1, [&[]; 3]).is_err());
        assert!(request(&[0; 32], 6, 0, 1, [&[7], &[], &[]]).is_err());
        for selector in 7..=14 {
            assert!(request(&[0; 32], selector, 0, 0, [&[7], &[], &[]]).is_ok());
            assert!(request(&[0; 32], selector, 0, 0, [&[]; 3]).is_err());
        }
        assert!(bounds(16).is_err());
    }
    #[test]
    fn malformed_time_response_retires_only_its_owned_exchange_before_decoding() {
        let vectors: norito::json::Value = norito::json::from_str(include_str!(
            "../../../../fixtures/kagemusha/wallet_v1_vectors.json"
        ))
        .unwrap();
        let original = |name| {
            let row = vectors["objects"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["type"].as_str() == Some(name))
                .unwrap();
            hex::decode(row["canonical_hex"].as_str().unwrap()).unwrap()
        };
        let anchor = original("KagemushaWalletTimeAnchorV1");
        let certificate = original("KagemushaWalletSignerCertificateV1");
        let malformed = vec![7];
        let mut times = BTreeMap::from([(1, [11; 32]), (2, [22; 32]), (3, [33; 32])]);
        for (token, first, second) in [(1, &malformed, &certificate), (2, &anchor, &malformed)] {
            let Setup::FinishTime {
                token,
                anchor,
                certificate,
            } = request(&[0; 32], 5, 0, token, [first, second, &[]]).unwrap()
            else {
                panic!("bounded time originals must reach their native owner");
            };
            assert_eq!(
                take_time_response(&mut times, token, &anchor, &certificate)
                    .unwrap_err()
                    .status,
                INVALID,
            );
            assert!(!times.contains_key(&token));
            assert_eq!(times.get(&3), Some(&[33; 32]));
        }
        assert_eq!(times.len(), 1);
        assert_eq!(
            take_time_response(&mut times, 1, &anchor, &certificate)
                .unwrap_err()
                .status,
            INVALID,
        );
        let (exchange, decoded_anchor, decoded_certificate) =
            take_time_response(&mut times, 3, &anchor, &certificate).unwrap();
        assert_eq!(exchange, [33; 32]);
        assert_eq!(norito::encode_canonical(&decoded_anchor).unwrap(), anchor);
        assert_eq!(
            norito::encode_canonical(&decoded_certificate).unwrap(),
            certificate,
        );
        assert!(times.is_empty());
    }

    #[test]
    fn cancelling_time_discards_exact_token_once_without_replacing_other_exchanges() {
        let mut times = BTreeMap::from([(1, [11; 32]), (2, [22; 32])]);
        assert_eq!(take_time(&mut times, 1).unwrap(), [11; 32]);
        assert_eq!(take_time(&mut times, 1).unwrap_err().status, INVALID);
        assert_eq!(times.len(), 1);
        assert_eq!(take_time(&mut times, 2).unwrap(), [22; 32]);
        assert!(times.is_empty());
    }
}
