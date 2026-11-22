//! Schema-3 read-only delivery evidence. A genuine recursive Receive head and an exact
//! indexed-map opening distinguish credited value from a Burn, including on redelivery.
use super::*;
use crate::{
    bootstrap_context as bc,
    indexed_map::{self, Path},
    payment_wire::limbs,
    receive_lineage, request, statement,
};
use iroha_pasta::poseidon;
use std::sync::LazyLock;
#[path = "credit_proof_codec.rs"]
mod proof_codec;
#[path = "credit_status_identity.rs"]
mod status_identity;
#[path = "credit_status_later_send.rs"]
pub(super) mod later_send;

const MAGIC: &[u8; 8] = b"KGSCST3\0";
/// Wire v1 sections 2 and 6 allow 10,034 canonical bytes and 10,084 including IPM1 on NFC.
const MAX_PAYLOAD: usize = super::super::MAX_CREDIT_STATUS_ORIGINAL_BYTES - payment_wire::HEADER_BYTES;
const DYNAMIC: [usize; 9] = [7, 8, 9, 10, 11, 12, 13, 14, 15];
// Statement and folded lineage bounds from the authenticated Receive relation. Each minimal field
// also carries its one-byte width. The opening's next key may use all 32 bytes.
const STATEMENT_WIDTHS: [usize; 9] = [32, 1, 16, 16, 1, 0, 0, 32, 32];
const LINEAGE_WIDTHS: [usize; 5] = [1, 8, 1, 16, 32];

fn worst_payload(pair: usize) -> usize {
    MAGIC.len() + 1 + STATEMENT_WIDTHS.len() + STATEMENT_WIDTHS.iter().sum::<usize>() + 160
        + LINEAGE_WIDTHS.len() + LINEAGE_WIDTHS.iter().sum::<usize>() + 33 + 4 + 4 + 32 * 32 + pair
}

/// Qualify all thirty-two populated siblings and maximum legal terms under the installed geometry.
pub(super) fn admit_profile(profile: &carrier::TrustedProfile, gens: &full::PairGens) -> std::result::Result<usize, String> {
    let pair = proof_codec::Codec::new(profile, gens)?.encoded_len()?;
    let maximum = worst_payload(pair);
    if maximum > MAX_PAYLOAD { return Err(format!("installed Receive needs {maximum} canonical credit-status bytes")); }
    Ok(maximum)
}

static EMPTY: LazyLock<[Fp; 32]> = LazyLock::new(|| {
    let mut value = Fp::ZERO;
    std::array::from_fn(|_| {
        let current = value;
        value = poseidon::hash_with_domain(indexed_map::NODE_DOMAIN, &[current, current]);
        current
    })
});

/// A read-only verified delivery result. Neither result authorizes refund or another transfer.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum VerifiedCreditStatus {
    Credited,
    Burned,
}
impl VerifiedCreditStatus {
    fn bit(self) -> u8 {
        match self {
            Self::Credited => 0,
            Self::Burned => 1,
        }
    }
}

fn put_field(out: &mut Vec<u8>, value: Fp) {
    let bytes = value.to_repr();
    let used = bytes.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
    out.push(used as u8);
    out.extend_from_slice(&bytes[..used]);
}
fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    let (v, rest) = input
        .split_at_checked(n)
        .ok_or(Error::Malformed("truncated credit status"))?;
    *input = rest;
    Ok(v)
}
fn get_field(input: &mut &[u8]) -> Result<Fp> {
    let used = usize::from(take(input, 1)?[0]);
    if used > 32 {
        return Err(Error::Malformed("credit status field width"));
    }
    let bytes = take(input, used)?;
    if bytes.last() == Some(&0) {
        return Err(Error::Malformed("nonminimal credit status field"));
    }
    let mut field_bytes = [0; 32];
    field_bytes[..used].copy_from_slice(bytes);
    field(&field_bytes)
}

struct Opening {
    next: Fp,
    path: Path<32>,
}
impl Opening {
    fn root(&self, credit: Fp, payment: Fp, status: VerifiedCreditStatus) -> Result<Fp> {
        // Both Spartan transports use the wallet wire v1 §3.4 membership rules.
        let opening = iroha_data_model::kagemusha::KagemushaWalletCreditOpeningV1 {
            credit_id: credit.to_repr(),
            payment_digest: payment.to_repr(),
            burned: status == VerifiedCreditStatus::Burned,
            next_key: self.next.to_repr(),
            slot: self.path.index,
            siblings: self.path.siblings.iter().flat_map(|s| s.to_repr()).collect(),
        };
        let root = opening.root().map_err(|_| Error::Malformed("credit status membership opening"))?;
        field(&root)
    }
    fn write(&self, out: &mut Vec<u8>) {
        put_field(out, self.next);
        out.extend_from_slice(&self.path.index.to_le_bytes());
        let mask = self
            .path
            .siblings
            .iter()
            .zip(EMPTY.iter())
            .enumerate()
            .fold(0u32, |bits, (i, (s, e))| bits | (u32::from(s != e) << i));
        out.extend_from_slice(&mask.to_le_bytes());
        for (i, sibling) in self.path.siblings.iter().enumerate() {
            if mask & (1 << i) != 0 {
                out.extend_from_slice(&sibling.to_repr());
            }
        }
    }
    fn read(input: &mut &[u8]) -> Result<Self> {
        let next = get_field(input)?;
        let index = u32::from_le_bytes(take(input, 4)?.try_into().unwrap());
        let mask = u32::from_le_bytes(take(input, 4)?.try_into().unwrap());
        let mut siblings = *EMPTY;
        for (i, sibling) in siblings.iter_mut().enumerate() {
            if mask & (1 << i) != 0 {
                let value = field(take(input, 32)?.try_into().unwrap())?;
                if value == *sibling {
                    return Err(Error::Malformed("nonminimal credit status sibling"));
                }
                *sibling = value;
            }
        }
        Ok(Self {
            next,
            path: Path { index, siblings },
        })
    }
}

fn bind_request(
    quote: &OpenRequestPeer<'_>,
    payment: &VerifiedIncomingPayment,
) -> Result<Credential> {
    let req = payment.request();
    if quote.request[..66] != req.body[..66]
        || quote.request[130..194] != req.body[130..194]
        || quote.request[210..418] != req.body[210..]
        || quote.request[418..] != req.signature
    {
        return Err(Error::AuthorityMismatch(
            "credit status retained Request and Payment",
        ));
    }
    read_credential(quote.receiver_credential)
}

fn statement_base(receiver: &Credential, payment: &VerifiedIncomingPayment) -> Result<[Fp; 26]> {
    let mut s = [Fp::ZERO; 26];
    s[..7].copy_from_slice(&payment.statement()[..7]);
    s[16] = Fp::from(4);
    s[17] = request::commitments(payment.request())
        .map_err(Error::Malformed)?
        .credit;
    s[18..20].copy_from_slice(&limbs(&payment.request().body[66..98]));
    s[20] = certificate::fp(&BigUint::from_bytes_le(&payment.request().body[242..258]));
    if receiver.body[2..66] != payment.request().body[2..66] {
        return Err(Error::AuthorityMismatch(
            "credit status receiver asset and scheme",
        ));
    }
    Ok(s)
}

fn restore_receipt(
    receiver: &Credential,
    s: &[Fp; 26],
    payment: Fp,
    extra: &[u8],
) -> Result<bootstrap::Receipt> {
    if extra.len() != 160 {
        return Err(Error::Malformed("credit status receipt"));
    }
    let mut body = [0; 338];
    body[0] = 1;
    body[2..34].copy_from_slice(&receiver.body[2..34]);
    body[34..66].copy_from_slice(&receiver.body[66..98]);
    body[66..98].copy_from_slice(&extra[..32]);
    let sequence = s[9].to_repr();
    if sequence[16..] != [0; 16] {
        return Err(Error::Malformed("credit status sequence"));
    }
    body[98..114].copy_from_slice(&sequence[..16]);
    let wallet = limbs(&receiver.body[66..98]);
    let operation = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[wallet[0], wallet[1], Fp::from(4), s[17]],
    );
    for (at, value) in [
        (114, operation),
        (146, s[14]),
        (178, s[15]),
        (210, statement::digest(s)),
        (306, payment),
    ] {
        body[at..at + 32].copy_from_slice(&value.to_repr());
    }
    body[242..306].copy_from_slice(&extra[32..96]);
    Ok(bootstrap::Receipt {
        body,
        signature: extra[96..].try_into().unwrap(),
    })
}

fn projected(
    receiver: &Credential,
    s: &[Fp; 26],
    receipt: &bootstrap::Receipt,
    lineage: [Fp; 6],
) -> Result<carrier::ProjectedInputs> {
    let mut context = [Fp::ZERO; bc::FIELDS];
    context[..2].copy_from_slice(&s[3..5]);
    context[2..4].copy_from_slice(&s[1..3]);
    context[4..6].copy_from_slice(&s[5..7]);
    context[6..8].copy_from_slice(&limbs(&receiver.body[66..98]));
    context[8] = s[7];
    for (i, start) in [147, 131, 179, 163].into_iter().enumerate() {
        context[9 + i] =
            certificate::fp(&BigUint::from_bytes_be(&receiver.body[start..start + 16]));
    }
    let table = std::array::from_fn(|i| {
        certificate::fp(&BigUint::from_bytes_le(
            &receiver.body[195 + 16 * i..211 + 16 * i],
        ))
    });
    carrier::ProjectedInputs::from_authenticated_fields(
        s[15],
        statement::digest(s),
        crate::receipt::digest(receipt),
        context,
        lineage,
        table,
        full::RECEIVE,
        [Fp::ZERO; 2],
    )
    .map_err(Error::Rejected)
}

impl NativeEngine {
    /// Confirm delivery against the exact retained Payment and original hosted Request. All
    /// matrices and inherited generator accumulators of the receiver head are decided here.
    pub fn verify_credit_status_peer(
        &self,
        wire: &[u8],
        open_request: &[u8],
        payment: &VerifiedIncomingPayment,
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedCreditStatus> {
        self.verify_credit_status_evidence(wire, open_request, payment, parameters)
            .map(|(status, _)| status)
    }

    /// The same complete verification, also returning the receiver originals that the
    /// accepted projection authenticated: the Receive head of this Payment, or a later
    /// receiver Send head whose credit root covers it.
    pub(crate) fn verify_credit_status_evidence(
        &self,
        wire: &[u8],
        open_request: &[u8],
        payment: &VerifiedIncomingPayment,
        parameters: &AuthenticatedParameters,
    ) -> Result<(
        VerifiedCreditStatus,
        crate::native::sent_delivery::ReceiverEvidence,
    )> {
        payment.require_parameters(parameters)?;
        let mut bytes =
            payment_wire::decode_peer_payload(4, wire, MAX_PAYLOAD).map_err(Error::Rejected)?;
        if later_send::recognizes(bytes) {
            return later_send::verify(self, bytes, payment, open_request, parameters)
                .map(|verified| (verified.outcome, verified.evidence));
        }
        if take(&mut bytes, 8)? != MAGIC {
            return Err(Error::Malformed("credit status magic"));
        }
        let status = match take(&mut bytes, 1)?[0] {
            0 => VerifiedCreditStatus::Credited,
            1 => VerifiedCreditStatus::Burned,
            _ => return Err(Error::Malformed("credit status verdict")),
        };
        let quote = self.authenticate_open_request_peer(open_request, parameters)?;
        let receiver = bind_request(&quote, payment)?;
        let mut s = statement_base(&receiver, payment)?;
        for i in DYNAMIC {
            s[i] = get_field(&mut bytes)?;
        }
        let receipt = restore_receipt(&receiver, &s, payment.digest(), take(&mut bytes, 160)?)?;
        let q = point(&receiver.body[130..195])?;
        certificate::verify_native(&q, crate::receipt::message(&receipt), &receipt.signature)
            .map_err(|_| Error::AuthorityMismatch("credit status receiver receipt signature"))?;
        let mut lineage = [Fp::ZERO; 6];
        for entry in &mut lineage[..5] {
            *entry = get_field(&mut bytes)?;
        }
        let opening = Opening::read(&mut bytes)?;
        lineage[bc::CREDIT_ROOT] = opening.root(s[17], payment.digest(), status)?;
        let projection = projected(&receiver, &s, &receipt, lineage)?;
        let trusted = parameters.inner.profile(full::RECEIVE)?;
        let codec = proof_codec::Codec::new(&trusted, &parameters.inner.generators).map_err(Error::Rejected)?;
        let exact = codec.unpack(bytes, &parameters.inner.generators).map_err(Error::Rejected)?;
        let decoded = carrier::decode_projected_pair(&trusted, &exact, &projection)
            .map_err(Error::Rejected)?;
        let pair = verify_pair(&parameters.inner, full::RECEIVE, decoded)?;
        #[cfg(not(test))]
        drop(pair);
        // Every word below was an input of the decided pair or of the checked receipt.
        let profile: [Fp; 64] = projection
            .primary()
            .io
            .as_slice()
            .try_into()
            .map_err(|_| Error::Malformed("credit status projected profile"))?;
        Ok((
            status,
            crate::native::sent_delivery::ReceiverEvidence {
                credential: receiver,
                receipt,
                statement: s,
                profile,
                exact_pair: exact,
                #[cfg(test)]
                pair,
                next: opening.next,
                slot: opening.path.index,
                siblings: opening.path.siblings,
            },
        ))
    }

    /// Return the authenticated delivery status for this Released Receive. Caller retains
    /// the underlying Payment for exact-byte retry. A conflicting credit returns no status.
    pub fn encode_credit_status_peer(
        &self,
        capsule: &RecoveryCapsule,
        completion: &CompletionRecord,
        open_request: &[u8],
        payment: &VerifiedIncomingPayment,
        parameters: &AuthenticatedParameters,
    ) -> Result<Vec<u8>> {
        payment.require_parameters(parameters)?;
        self.credit_status_from_release(
            capsule,
            completion,
            open_request,
            Some(payment),
            parameters,
        )
    }

    /// Generate evidence using the complete Payment original in the verified Receive capsule.
    pub fn encode_retained_credit_status_peer(
        &self,
        capsule: &RecoveryCapsule,
        completion: &CompletionRecord,
        open_request: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<Vec<u8>> {
        self.credit_status_from_release(capsule, completion, open_request, None, parameters)
    }

    fn credit_status_from_release(
        &self,
        capsule: &RecoveryCapsule,
        completion: &CompletionRecord,
        open_request: &[u8],
        verified_payment: Option<&VerifiedIncomingPayment>,
        parameters: &AuthenticatedParameters,
    ) -> Result<Vec<u8>> {
        if capsule.kind() != 4 {
            return Err(Error::AuthorityMismatch("credit status Receive required"));
        }
        let quote = self.authenticate_open_request_peer(open_request, parameters)?;
        let (prepared, released) = self.verify_released_operation(capsule, completion, parameters)?;
        let head = released.head();
        let ExactOperationWitness::Receive {
            step,
            incoming_payment,
        } = prepared.witness.as_ref()
        else {
            return Err(Error::AuthorityMismatch(
                "credit status Receive witness required",
            ));
        };
        let retained_payment;
        let payment = match verified_payment {
            Some(payment) => payment,
            None => {
                retained_payment = self.verify_payment(incoming_payment, parameters)?;
                &retained_payment
            }
        };
        let receiver = bind_request(&quote, payment)?;
        if incoming_payment.as_ref() != payment.exact_bytes() || step.request != *payment.request()
        {
            return Err(Error::AuthorityMismatch(
                "credit status exact retained Payment",
            ));
        }
        let mut s = statement_base(&receiver, payment)?;
        for i in DYNAMIC {
            s[i] = step.statement[i];
        }
        if s != step.statement {
            return Err(Error::AuthorityMismatch("credit status Receive statement"));
        }
        let r = completion.exact_receipt();
        let extra = [&r[66..98], &r[242..306], &r[338..402]].concat();
        let receipt = restore_receipt(&receiver, &s, payment.digest(), &extra)?;
        if receipt.body.as_slice() != &r[..338] || receipt.signature.as_slice() != &r[338..] {
            return Err(Error::AuthorityMismatch("credit status retained receipt"));
        }
        let maps = &step.maps;
        let opening = if maps.credit_search.leaf.key == s[17] {
            Opening {
                next: maps.credit_search.leaf.next,
                path: maps.credit_search.path.clone(),
            }
        } else {
            Opening {
                next: maps.credit_insert.low.next,
                path: maps.credit_insert.slot.clone(),
            }
        };
        let lineage: [Fp; 6] = head.profile[bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS]
            .try_into()
            .unwrap();
        let status = [VerifiedCreditStatus::Credited, VerifiedCreditStatus::Burned]
            .into_iter()
            .find(|status| {
                opening.root(s[17], payment.digest(), *status).is_ok_and(|root| root == lineage[bc::CREDIT_ROOT])
            })
            .ok_or(Error::AuthorityMismatch("exact retained credit membership"))?;
        let projection = projected(&receiver, &s, &receipt, lineage)?;
        if projection.primary().io != head.profile {
            return Err(Error::AuthorityMismatch(
                "credit status complete folded profile",
            ));
        }
        let trusted = parameters.inner.profile(full::RECEIVE)?;
        let codec = proof_codec::Codec::new(&trusted, &parameters.inner.generators).map_err(Error::Rejected)?;
        // A released head retains the whole canonical HeadFrame (including its credential and
        // 64 public words). Only its inner projected pair is reconstructed from the peer's scoped originals.
        let folded: HeadFrame = super::super::decode(head.exact_pair())?;
        if usize::from(folded.kind) != full::RECEIVE || folded.parameters != parameters.id {
            return Err(Error::Malformed("credit status folded pair scope or length"));
        }
        let pair = codec.pack(&folded.pair).map_err(Error::Rejected)?;
        #[cfg(any(test, feature = "device-fixture"))]
        if codec.unpack(&pair, &parameters.inner.generators).map_err(Error::Rejected)? != folded.pair {
            return Err(Error::Malformed("nonidentical Receive pair after projection"));
        }
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.push(status.bit());
        for i in DYNAMIC {
            put_field(&mut out, s[i]);
        }
        out.extend_from_slice(&extra);
        for value in &lineage[..5] {
            put_field(&mut out, *value);
        }
        opening.write(&mut out);
        out.extend_from_slice(&pair);
        #[cfg(any(test, feature = "device-fixture"))]
        eprintln!(
            "Spartan CreditStatus {:?}: canonical {} B, IPM1 {} B, folded pair {} -> {} B, explicit siblings {}, canonical cap {} B",
            status,
            out.len(),
            out.len() + payment_wire::HEADER_BYTES,
            folded.pair.len(),
            pair.len(),
            opening
                .path
                .siblings
                .iter()
                .zip(EMPTY.iter())
                .filter(|(a, b)| a != b)
                .count(),
            MAX_PAYLOAD
        );
        if out.len() > MAX_PAYLOAD {
            return Err(Error::ResourceLimit);
        }
        Ok(payment_wire::encode_peer_payload(4, &out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_credit_status_accounts_for_every_field_and_all_siblings() {
        let full = -Fp::ONE;
        let sequence = Fp::from_u128(u128::MAX);
        let mut out = MAGIC.to_vec();
        out.push(VerifiedCreditStatus::Burned.bit());
        for value in [full, Fp::from(2), sequence, sequence, Fp::from(3), Fp::ZERO, Fp::ZERO, full, full] {
            put_field(&mut out, value);
        }
        out.extend_from_slice(&[0xff;160]);
        for value in [Fp::from(2), Fp::from(u64::MAX), Fp::from(3), sequence, full] {
            put_field(&mut out, value);
        }
        let opening = Opening {
            next: full,
            path: Path {index: u32::MAX, siblings: std::array::from_fn(|i| EMPTY[i] + Fp::ONE)},
        };
        opening.write(&mut out);
        // Real p18-s17: 9,642 raw pair bytes, 35 endoscalars encoded under five bitmap bytes.
        // All 35 challenges simultaneously fit their 16-byte positive GLV coefficients.
        const PAIR: usize = 8501;
        out.resize(out.len() + PAIR, 0);
        assert_eq!(out.len(), worst_payload(PAIR));
        assert_eq!(out.len(), 9937);
        let wire = payment_wire::encode_peer_payload(4, &out);
        assert_eq!(wire.len(), 10021);
        assert_eq!(payment_wire::decode_peer_payload(4, &wire, MAX_PAYLOAD).unwrap(), out);
    }

    #[test]
    #[ignore = "requires KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS with the signed p18-s17 programs"]
    fn signed_programs_qualify_the_full_depth_credit_status_bound() {
        let directory = std::path::PathBuf::from(std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap());
        let read = |name: &str| std::fs::read(directory.join(name)).unwrap();
        let parameters = crate::native::device_fixture::install(
            &read("program-bundle.norito"), &read("parameter-manifest.norito"),
            read("parameter-signature.bin").try_into().unwrap(),
        ).unwrap();
        let profile = parameters.inner.profile(full::RECEIVE).unwrap();
        assert_eq!(admit_profile(&profile, &parameters.inner.generators).unwrap(), 9937);
    }
    #[test]
    fn credit_status_sparse_membership_opens_only_the_exact_payment_and_burn_flag() {
        let credit = Fp::from(7);
        let payment = Fp::from(11);
        for status in [VerifiedCreditStatus::Credited, VerifiedCreditStatus::Burned] {
            let value = poseidon::hash_with_domain(
                receive_lineage::CREDIT_DOMAIN,
                &[credit, payment, Fp::from(status.bit() as u64)],
            );
            let (insert, root) = indexed_map::empty_insert::<32>(credit, value);
            let opening = Opening {
                next: insert.low.next,
                path: insert.slot,
            };
            assert_eq!(opening.root(credit, payment, status).unwrap(), root);
            let mut frame = Vec::new();
            opening.write(&mut frame);
            assert_eq!(frame.len(), 41);
            let mut view = frame.as_slice();
            let decoded = Opening::read(&mut view).unwrap();
            assert!(view.is_empty());
            assert_eq!(decoded.root(credit, payment, status).unwrap(), root);
            assert_ne!(decoded.root(credit, payment + Fp::ONE, status).unwrap(), root);
            assert_ne!(
                decoded.root(
                    credit,
                    payment,
                    if status == VerifiedCreditStatus::Credited {
                        VerifiedCreditStatus::Burned
                    } else {
                        VerifiedCreditStatus::Credited
                    }
                ).unwrap(),
                root
            );
            // Explicitly serializing the default sibling has a second encoding of the same opening.
            frame[5] |= 2;
            frame.extend_from_slice(&EMPTY[1].to_repr());
            assert!(Opening::read(&mut frame.as_slice()).is_err());
        }
    }
}
