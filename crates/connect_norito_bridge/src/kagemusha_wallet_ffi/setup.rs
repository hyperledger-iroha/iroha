//! Typed setup input shared by C/JNI; no foreign clock or arbitrary signature body.

use super::*;

pub(crate) enum Setup {
    Bootstrap,
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
    FinishTime {
        token: u64,
        anchor: Box<KagemushaWalletTimeAnchorV1>,
        certificate: Box<KagemushaWalletSignerCertificateV1>,
    },
}
pub(crate) fn bounds(selector: u32) -> Result<[usize; 3]> {
    Ok(match selector {
        0 | 1 | 4 => [0; 3],
        2 => [
            KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_FEE_SCHEDULE_MAX_BYTES_V1,
            KAGEMUSHA_WALLET_CERTIFICATE_MAX_BYTES_V1,
        ],
        3 => [KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1, 0, 0],
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
        || ((selector == 5) != (token != 0))
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
        5 if !first.is_empty() && !second.is_empty() => Setup::FinishTime {
            token,
            anchor: Box::new(decode(first)?),
            certificate: Box::new(decode(second)?),
        },
        _ => return Err(Failure::code(INVALID)),
    })
}
impl<P, S> NativeWallet<P, S>
where
    P: advance::KagemushaWalletPlatformV1,
    S: OriginalSourceV1 + Send,
{
    pub(super) fn setup_inner(&mut self, input: Setup) -> Result<Response> {
        let bytes = match input {
            Setup::Bootstrap => return Ok(completion(Some(self.wallet.bootstrap()?))),
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
            Setup::FinishTime {
                token,
                anchor,
                certificate,
            } => {
                let exchange = self.times.remove(&token).ok_or(Failure::code(INVALID))?;
                self.wallet
                    .finish_direct_time_exchange(exchange, *anchor, &certificate)?;
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
        for selector in [0, 1, 3, 4] {
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
        assert!(bounds(6).is_err());
    }
}
