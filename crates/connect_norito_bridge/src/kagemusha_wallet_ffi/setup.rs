//! Typed setup input shared by C/JNI; no foreign clock or arbitrary signature body.

use super::*;

pub(crate) enum Setup {
    Bootstrap,
    UnloadClaim {
        request_id: [u8; 32],
        beneficiary: Option<Vec<u8>>,
    },
    FeeClaim {
        credit: [u8; 32],
    },
    FeeOriginal {
        request: bool,
        original: Vec<u8>,
    },
    FeeClaimTransport {
        original: Vec<u8>,
        beneficiary: Vec<u8>,
    },
    LedgerFinality(Vec<u8>),
    LedgerStatus,
    FeePayout {
        credit: [u8; 32],
        world: Vec<u8>,
        payout: Vec<u8>,
    },
    BackgroundStatus,
    Activation,
    CloseLoads {
        id: [u8; 32],
    },
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
    CreditedOriginal {
        status: bool,
        original: Vec<u8>,
    },
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
        0 | 1 | 4 | 6 | 15 | 18 | 19 | 20 | 24 => [0; 3],
        21 | 22 => [state::FEE_CLAIM_MAX_BYTES_V1, 0, 0],
        23 => [state::LEDGER_PROOF_MAX_BYTES_V1, 0, 0],
        27 => [KAGEMUSHA_WALLET_UNLOAD_CLAIM_MAX_BYTES_V1, 0, 0],
        26 => [
            state::FEE_CLAIM_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_FEE_CLAIM_MAX_BYTES_V1,
            0,
        ],
        25 => [
            iroha_data_model::sumeragi_finality::MAX_WORLD_STATE_SNAPSHOT_BYTES_V1,
            state::PAYOUT_RECORD_MAX_BYTES_V1,
            0,
        ],
        2 => [
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_FEE_SCHEDULE_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
        ],
        3 | 7..=14 | 16..=17 => [KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, 0, 0],
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
    if (matches!(selector, 1 | 2 | 19 | 20 | 25 | 27) != (id != [0; 32]))
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
        27 => Setup::UnloadClaim {
            request_id: id,
            beneficiary: (!first.is_empty()).then(|| first.to_vec()),
        },
        20 => Setup::FeeClaim { credit: id },
        21 | 22 if !first.is_empty() => Setup::FeeOriginal {
            request: selector == 22,
            original: first.to_vec(),
        },
        23 if !first.is_empty() => Setup::LedgerFinality(first.to_vec()),
        24 => Setup::LedgerStatus,
        26 if !first.is_empty() && !second.is_empty() => Setup::FeeClaimTransport {
            original: first.to_vec(),
            beneficiary: second.to_vec(),
        },
        25 if !first.is_empty() && !second.is_empty() => Setup::FeePayout {
            credit: id,
            world: first.to_vec(),
            payout: second.to_vec(),
        },
        15 => Setup::Activation,
        18 => Setup::BackgroundStatus,
        19 => Setup::CloseLoads { id },
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
        16..=17 if !first.is_empty() => Setup::CreditedOriginal {
            status: selector == 17,
            original: first.to_vec(),
        },
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
fn ledger_progress(progress: Option<state::LedgerProgressV1>) -> Response {
    match progress {
        Some(progress) => Response {
            kind: 33,
            sequence: u128::from(progress.height),
            bytes: progress.block_hash.to_vec(),
            ..Response::default()
        },
        None => Response {
            kind: 34,
            ..Response::default()
        },
    }
}
impl<P, S> NativeWallet<P, S>
where
    P: advance::KagemushaWalletPlatformV1,
    S: OriginalSourceV1 + Send,
{
    pub(super) fn setup_inner(&mut self, input: Setup) -> Result<Response> {
        let bytes = match input {
            Setup::UnloadClaim {
                request_id,
                beneficiary,
            } => {
                return Ok(Response {
                    kind: 37,
                    bytes: self
                        .wallet
                        .unload_claim_bytes(&request_id, beneficiary.as_deref())?,
                    ..Response::default()
                });
            }
            Setup::FeeClaim { credit } => {
                let original = self.wallet.fee_claim(credit)?;
                return Ok(match original {
                    None => Response {
                        kind: 32,
                        ..Response::default()
                    },
                    Some(original) => Response {
                        kind: 31,
                        bytes: original.to_canonical_bytes(&self.wallet.snapshot()?.scheme_id)?,
                        ..Response::default()
                    },
                });
            }
            Setup::FeeOriginal { request, original } => {
                let claim = state::RetainedFeeClaim::decode_canonical(
                    &original,
                    &self.wallet.snapshot()?.scheme_id,
                )?;
                if request {
                    claim.request
                } else {
                    claim.payment
                }
            }
            Setup::FeeClaimTransport {
                original,
                beneficiary,
            } => {
                let scheme = self.wallet.snapshot()?.scheme_id;
                let claim = state::RetainedFeeClaim::decode_canonical(&original, &scheme)?;
                return Ok(Response {
                    kind: 36,
                    bytes: claim.ledger_claim_bytes(&scheme, &beneficiary)?,
                    ..Response::default()
                });
            }
            Setup::LedgerFinality(original) => {
                return Ok(ledger_progress(Some(
                    self.wallet.ingest_ledger_finality(&original)?,
                )));
            }
            Setup::LedgerStatus => return Ok(ledger_progress(self.wallet.ledger_progress()?)),
            Setup::FeePayout {
                credit,
                world,
                payout,
            } => {
                self.wallet
                    .acknowledge_fee_payout_originals(credit, &world, &payout)?;
                return Ok(Response {
                    kind: 35,
                    ..Response::default()
                });
            }
            Setup::Transport {
                kind,
                wrap,
                original,
            } => {
                let scheme = self.wallet.snapshot()?.scheme_id;
                transport::convert(kind, wrap, &original, &scheme)?
            }
            Setup::CreditedOriginal { status, original } => {
                let scheme = self.wallet.snapshot()?.scheme_id;
                transport::credited(status, &original, &scheme)?
            }
            Setup::BackgroundStatus => return Err(Failure::code(INTERNAL)),
            Setup::Bootstrap => return Ok(completion(Some(self.wallet.bootstrap()?))),
            Setup::CloseLoads { id } => {
                return Ok(Response {
                    kind: 30,
                    bytes: self.wallet.close_loads(id)?,
                    ..Response::default()
                });
            }
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
        for selector in [0, 1, 3, 4, 15, 18, 19] {
            let id = if matches!(selector, 1 | 19) {
                [1; 32]
            } else {
                [0; 32]
            };
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
        for selector in (7..=14).chain(16..=17) {
            assert!(request(&[0; 32], selector, 0, 0, [&[7], &[], &[]]).is_ok());
            assert!(request(&[0; 32], selector, 0, 0, [&[]; 3]).is_err());
        }
        assert!(bounds(28).is_err());
    }
    #[test]
    fn unload_projection_requires_request_identity_and_only_optional_beneficiary() {
        let id = [7; 32];
        assert_eq!(bounds(27).unwrap(), [16_384, 0, 0]);
        assert!(request(&id, 27, 0, 0, [&[]; 3]).is_ok());
        assert!(request(&id, 27, 0, 0, [&vec![1; 16_384], &[], &[]]).is_ok());
        assert!(request(&[0; 32], 27, 0, 0, [&[]; 3]).is_err());
        assert!(request(&id, 27, 1, 0, [&[]; 3]).is_err());
        assert!(request(&id, 27, 0, 1, [&[]; 3]).is_err());
        assert!(request(&id, 27, 0, 0, [&vec![1; 16_385], &[], &[]]).is_err());
        assert!(request(&id, 27, 0, 0, [&[], &[1], &[]]).is_err());
        assert!(request(&id, 27, 0, 0, [&[], &[], &[1]]).is_err());
    }
    #[test]
    fn ledger_and_fee_setup_enforce_bounds_identity_and_progress_shape() {
        let zero = [0; 32];
        let credit = [7; 32];
        assert!(request(&credit, 20, 0, 0, [&[]; 3]).is_ok());
        assert!(request(&zero, 20, 0, 0, [&[]; 3]).is_err());
        for selector in 21..=23 {
            assert!(request(&zero, selector, 0, 0, [&[1], &[], &[]]).is_ok());
            assert!(request(&credit, selector, 0, 0, [&[1], &[], &[]]).is_err());
            assert!(request(&zero, selector, 0, 0, [&[]; 3]).is_err());
        }
        assert_eq!(bounds(26).unwrap(), [21_024, 16_384, 0]);
        assert!(request(&zero, 26, 0, 0, [&[1], &[2], &[]]).is_ok());
        for originals in [[&[1][..], &[][..], &[][..]], [&[1][..], &[2][..], &[3][..]]] {
            assert!(request(&zero, 26, 0, 0, originals).is_err());
        }
        assert!(request(&zero, 26, 0, 0, [&[1], &vec![0; 16_385], &[]]).is_err());
        assert_eq!(bounds(21).unwrap()[0], 21_024);
        assert_eq!(bounds(23).unwrap()[0], 36 * 1024 * 1024);
        assert_eq!(bounds(25).unwrap(), [32 * 1024 * 1024, 1024, 0]);
        assert!(request(&credit, 25, 0, 0, [&[1], &[2], &[]]).is_ok());
        assert!(request(&zero, 25, 0, 0, [&[1], &[2], &[]]).is_err());
        assert!(request(&credit, 25, 0, 0, [&[1], &[], &[]]).is_err());
        assert!(request(&credit, 25, 0, 1, [&[1], &[2], &[]]).is_err());
        assert!(request(&zero, 24, 0, 0, [&[]; 3]).is_ok());
        let result = ledger_progress(Some(state::LedgerProgressV1 {
            height: u64::MAX,
            block_hash: [9; 32],
        }));
        assert_eq!(
            (result.kind, result.sequence, result.detail, result.bytes),
            (33, u128::from(u64::MAX), 0, vec![9; 32])
        );
        let absent = ledger_progress(None);
        assert_eq!(
            (absent.kind, absent.sequence, absent.detail, absent.bytes),
            (34, 0, 0, vec![])
        );
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
