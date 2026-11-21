//! Spartan IPM1 profile 0x8001, peer schema 3, kind 4 (Credited). The folded
//! receiver-Send projection implements wallet wire v1 §3.4 CreditStatus and §6
//! framing. It uses the exact installed `full::SEND` program and generator geometry.
//! Inside the authenticated kind-4 envelope `101xxxxx` selects this projection.
//! The low bits encode Burned, lifecycle, two enabled controls and the dense opening.
//! Receivers that only implement the KGSCST3 Receive projection reject this prefix.
//! Conditional CreditStatus for a covered credit at a later folded receiver Send.
use super::*;
use crate::receipt;
use iroha_data_model::kagemusha::kagemusha_wallet_indexed_verify_membership_v1;
const SEND_PROFILE: u8 = 0xa0;
pub(super) fn recognizes(bytes: &[u8]) -> bool {
    bytes
        .first()
        .is_some_and(|byte| *byte & 0xe0 == SEND_PROFILE)
}

// Send statement outputs, preserving even those unrelated to the prior credit.
pub(in crate::native::engine) const WORDS: [(usize, usize); 16] = [
    (7, 32),
    (9, 16),
    (10, 16),
    (12, 16),
    (13, 32),
    (14, 32),
    (15, 32),
    (17, 32),
    (18, 16),
    (19, 16),
    (20, 16),
    (21, 16),
    (22, 16),
    (23, 32),
    (24, 8),
    (25, 8),
];

fn read_word(b: &mut &[u8], n: usize) -> Result<Fp> {
    let mut raw = [0; 32];
    raw[..n].copy_from_slice(take(b, n)?);
    field(&raw)
}
fn put_word(out: &mut Vec<u8>, v: Fp, n: usize) -> Result<()> {
    let raw = v.to_repr();
    if raw[n..].iter().any(|b| *b != 0) {
        return Err(Error::Malformed("later status field width"));
    }
    out.extend_from_slice(&raw[..n]);
    Ok(())
}
fn sibling_mask(path: &Opening) -> u32 {
    path.path
        .siblings
        .iter()
        .zip(*EMPTY)
        .enumerate()
        .fold(0u32, |mask, (i, (s, e))| mask | (u32::from(*s != e) << i))
}
fn pack_opening(out: &mut Vec<u8>, path: &Opening, mask: u32) {
    out.extend_from_slice(&path.next.to_repr());
    out.extend_from_slice(&path.path.index.to_le_bytes());
    if mask != u32::MAX {
        out.extend_from_slice(&mask.to_le_bytes());
    }
    for (i, v) in path.path.siblings.iter().enumerate() {
        if mask & (1 << i) != 0 {
            out.extend_from_slice(&v.to_repr());
        }
    }
}
// The exact opening original. The caller checks it under the credit and Payment digest.
fn read_opening(b: &mut &[u8], dense: bool) -> Result<Opening> {
    let next = read_word(b, 32)?;
    let index = u32::from_le_bytes(take(b, 4)?.try_into().unwrap());
    let mask = if dense {
        u32::MAX
    } else {
        let sparse = u32::from_le_bytes(take(b, 4)?.try_into().unwrap());
        if sparse == u32::MAX {
            return Err(Error::Malformed("later status noncanonical dense opening"));
        }
        sparse
    };
    let mut siblings = *EMPTY;
    for (i, sibling) in siblings.iter_mut().enumerate() {
        if mask & (1 << i) != 0 {
            let value = read_word(b, 32)?;
            if value == *sibling {
                return Err(Error::Malformed("later status redundant sibling"));
            }
            *sibling = value;
        }
    }

    Ok(Opening {
        next,
        path: Path { index, siblings },
    })
}

fn restore_receipt(s: &[Fp; 26], quote: &Credential, extra: &[u8]) -> Result<bootstrap::Receipt> {
    let mut body = [0; receipt::BODY_BYTES];
    body[0] = 1;
    body[2..34].copy_from_slice(&quote.body[2..34]);
    body[34..66].copy_from_slice(&quote.body[66..98]);
    body[66..98].copy_from_slice(&quote.body[259..291]);
    body[98..114].copy_from_slice(&s[9].to_repr()[..16]);
    let [a, b] = payment_wire::limbs(&quote.body[66..98]);
    let id = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[a, b, Fp::from(3), s[17]],
    );
    for (at, value) in [
        (114, id),
        (146, s[14]),
        (178, s[15]),
        (210, statement::digest(s)),
    ] {
        body[at..at + 32].copy_from_slice(&value.to_repr());
    }
    body[242..306].copy_from_slice(&extra[..64]);
    let r = bootstrap::Receipt {
        body,
        signature: extra[64..].try_into().unwrap(),
    };
    certificate::verify_native(
        &point(&quote.body[130..195])?,
        receipt::message(&r),
        &r.signature,
    )
    .map_err(|_| Error::AuthorityMismatch("later head receipt"))?;
    Ok(r)
}
#[path = "credit_status_generators.rs"]
mod generator_projection;

pub(super) struct Accepted {
    pair_digest: [u8; 32],
    pub(super) outcome: VerifiedCreditStatus,
    statement: [Fp; 26],
    // The later head's authenticated receiver originals and the proven credit opening.
    pub(super) evidence: crate::native::sent_delivery::ReceiverEvidence,
}

pub(super) fn verify(
    engine: &NativeEngine,
    mut b: &[u8],
    original: &VerifiedIncomingPayment,
    request: &[u8],
    parameters: &AuthenticatedParameters,
) -> Result<Accepted> {
    use num_bigint::BigUint;
    original.require_parameters(parameters)?;
    let flags = take(&mut b, 1)?[0];
    if flags & 0xe0 != SEND_PROFILE {
        return Err(Error::Malformed("later status profile"));
    }
    let burned = flags & 1 != 0;
    let outcome = if burned {
        VerifiedCreditStatus::Burned
    } else {
        VerifiedCreditStatus::Credited
    };
    let peer = engine.authenticate_open_request_peer(request, parameters)?;
    let quote = bind_request(&peer, original)?;
    let mut s = [Fp::ZERO; 26];
    s[..7].copy_from_slice(&original.statement()[..7]);
    s[16] = Fp::from(3);
    s[8] = Fp::from(u64::from(1 + ((flags >> 1) & 1)));
    s[11] = Fp::from(u64::from((flags >> 2) & 3));
    for (i, n) in WORDS {
        s[i] = read_word(&mut b, n)?;
    }
    let r = restore_receipt(&s, &quote, take(&mut b, 128)?)?;
    let epoch = read_word(&mut b, 8)?;
    let pending = read_word(&mut b, 32)?;
    // The signed scheme fixes the provider contract. Its unblinded key table is a
    // deterministic function of the installed generators and the receiver's payment key.
    // A renewal may change the current credential digest carried in s[7].
    let table = std::array::from_fn(|i| {
        certificate::fp(&BigUint::from_bytes_le(
            &quote.body[195 + 16 * i..211 + 16 * i],
        ))
    });
    let opening = read_opening(&mut b, flags & 0x10 != 0)?;
    let root = opening.root(original.statement()[17], original.digest(), outcome)?;
    let mut context = [Fp::ZERO; bc::FIELDS];
    context[..2].copy_from_slice(&s[3..5]);
    context[2..4].copy_from_slice(&s[1..3]);
    context[4..6].copy_from_slice(&s[5..7]);
    context[6..8].copy_from_slice(&payment_wire::limbs(&quote.body[66..98]));
    context[8] = s[7];
    for (i, at) in [147, 131, 179, 163].into_iter().enumerate() {
        context[9 + i] = certificate::fp(&BigUint::from_bytes_be(&quote.body[at..at + 16]));
    }
    let projection = carrier::ProjectedInputs::from_authenticated_fields(
        s[15],
        statement::digest(&s),
        receipt::digest(&r),
        context,
        [s[8], epoch, s[11], s[12], pending, root],
        table,
        full::SEND,
        [Fp::ZERO; 2],
    )
    .map_err(Error::Rejected)?;
    let profile = parameters.inner.profile(full::SEND)?;
    let codec =
        proof_codec::Codec::new(&profile, &parameters.inner.generators).map_err(Error::Rejected)?;
    let coordinates = generator_projection::Layout::new(&profile, &codec)?;
    qualified_maximum(coordinates.bytes())?;
    let pair = coordinates.restore(b, &codec, &profile, &projection, parameters)?;
    let pair_digest = *blake3::hash(&pair).as_bytes();
    let decoded =
        carrier::decode_projected_pair(&profile, &pair, &projection).map_err(Error::Rejected)?;
    verify_pair(&parameters.inner, full::SEND, decoded)?;
    // Every evidence word was an input of the decided pair or of the checked receipt.
    let words: [Fp; 64] = projection
        .primary()
        .io
        .as_slice()
        .try_into()
        .map_err(|_| Error::Malformed("later status projected profile"))?;
    Ok(Accepted {
        pair_digest,
        outcome,
        statement: s,
        evidence: crate::native::sent_delivery::ReceiverEvidence {
            credential: quote,
            receipt: r,
            statement: s,
            profile: words,
            exact_pair: pair,
            next: opening.next,
            slot: opening.path.index,
            siblings: opening.path.siblings,
        },
    })
}

fn qualified_maximum(encoded_pair: usize) -> Result<usize> {
    // Profile/flags, all bounded statement fields, receipt fragment, epoch and pending
    // root, next credit and slot, then all 32 siblings. Dense form has no redundant mask.
    let fixed =
        1 + WORDS.iter().map(|&(_, bytes)| bytes).sum::<usize>() + 128 + 8 + 32 + 32 + 4 + 32 * 32;
    let n = fixed
        .checked_add(encoded_pair)
        .ok_or(Error::ResourceLimit)?;
    if n > MAX_PAYLOAD {
        return Err(Error::ResourceLimit);
    }
    Ok(n + payment_wire::HEADER_BYTES)
}

fn pack_frame(
    later: &VerifiedIncomingPayment,
    opening: &Opening,
    outcome: VerifiedCreditStatus,
    parameters: &AuthenticatedParameters,
) -> Result<(Vec<u8>, usize, usize)> {
    later.require_parameters(parameters)?;
    let profile = parameters.inner.profile(full::SEND)?;
    let codec =
        proof_codec::Codec::new(&profile, &parameters.inner.generators).map_err(Error::Rejected)?;
    let layout = generator_projection::Layout::new(&profile, &codec)?;
    let full_len = qualified_maximum(layout.bytes())?;
    let pair = layout.compact(&codec, &profile, later.exact_pair())?;
    let p = later.profile();
    let s = later.statement();
    let r = later.receipt();
    let lifecycle = s[8].to_repr()[0];
    let controls = s[11].to_repr()[0];
    if !(1..=2).contains(&lifecycle)
        || s[8] != Fp::from(u64::from(lifecycle))
        || controls > 3
        || s[11] != Fp::from(u64::from(controls))
    {
        return Err(Error::Malformed("later Send state flags"));
    }
    let mask = sibling_mask(opening);
    let mut b = vec![
        SEND_PROFILE
            | outcome.bit()
            | ((lifecycle - 1) << 1)
            | (controls << 2)
            | (u8::from(mask == u32::MAX) << 4),
    ];
    for (i, n) in WORDS {
        put_word(&mut b, s[i], n)?;
    }
    b.extend_from_slice(&r.body[242..306]);
    b.extend_from_slice(&r.signature);
    put_word(&mut b, p[bc::LINEAGE_IO + bc::POLICY_EPOCH], 8)?;
    put_word(&mut b, p[bc::LINEAGE_IO + bc::PENDING_ROOT], 32)?;
    pack_opening(&mut b, opening, mask);
    b.extend_from_slice(&pair);
    if b.len() > MAX_PAYLOAD {
        return Err(Error::ResourceLimit);
    }
    Ok((
        payment_wire::encode_peer_payload(4, &b),
        pair.len(),
        full_len,
    ))
}

// The producer can publish only evidence re-verified against the retained credit and
// the exact selected later head. Raw packing is private to the projection and tests.
fn checked_frame(
    engine: &NativeEngine,
    later: &VerifiedIncomingPayment,
    opening: &Opening,
    outcome: VerifiedCreditStatus,
    original: &VerifiedIncomingPayment,
    request: &[u8],
    parameters: &AuthenticatedParameters,
) -> Result<(Vec<u8>, usize, usize)> {
    let (wire, pair_len, maximum) = pack_frame(later, opening, outcome, parameters)?;
    let payload =
        payment_wire::decode_peer_payload(4, &wire, MAX_PAYLOAD).map_err(Error::Rejected)?;
    let checked = verify(engine, payload, original, request, parameters)?;
    if checked.pair_digest != *blake3::hash(later.exact_pair()).as_bytes()
        || checked.statement != *later.statement()
        || checked.outcome != outcome
    {
        return Err(Error::AuthorityMismatch("later status reconstructed head"));
    }
    Ok((wire, pair_len, maximum))
}

impl NativeEngine {
    /// Publish read-only evidence for an original Payment at a verified later receiver Send.
    /// The current credit index supplies the opening.
    ///
    /// # Errors
    /// A wrong recipient, absent member,
    /// conflicting Payment, changed root, incompatible geometry or invalid proof is refused.
    pub fn encode_credit_status_at_send_head(
        &self,
        later: &VerifiedIncomingPayment,
        original: &VerifiedIncomingPayment,
        request: &[u8],
        maps: &mut dyn SpartanWalletMaps,
        parameters: &AuthenticatedParameters,
    ) -> Result<Vec<u8>> {
        original.require_parameters(parameters)?;
        later.require_parameters(parameters)?;
        let root = later.profile()[bc::LINEAGE_IO + bc::CREDIT_ROOT].to_repr();
        let credit = original.statement()[17];
        let (leaf, path) = maps.member_or_low(SpartanWalletMap::Credit, root, credit.to_repr())?;
        if leaf.key != credit.to_repr() {
            return Err(Error::AuthorityMismatch("later Send does not cover credit"));
        }
        kagemusha_wallet_indexed_verify_membership_v1(&root, &leaf, &path)
            .map_err(|_| Error::AuthorityMismatch("later Send credit opening"))?;
        let outcome = [VerifiedCreditStatus::Credited, VerifiedCreditStatus::Burned]
            .into_iter()
            .find(|status| {
                poseidon::hash_with_domain(
                    receive_lineage::CREDIT_DOMAIN,
                    &[credit, original.digest(), Fp::from(u64::from(status.bit()))],
                )
                .to_repr()
                    == leaf.value
            })
            .ok_or(Error::AuthorityMismatch(
                "later Send retained Payment digest",
            ))?;
        let mut siblings = [Fp::ZERO; 32];
        for (out, input) in siblings.iter_mut().zip(&path.siblings) {
            *out = field(input)?;
        }
        let opening = Opening {
            next: field(&leaf.next_key)?,
            path: Path {
                index: path.slot,
                siblings,
            },
        };
        checked_frame(
            self, later, &opening, outcome, original, request, parameters,
        )
        .map(|(wire, _, _)| wire)
    }
}

#[cfg(test)]
pub(in crate::native::engine) fn packed_fixture(
    later: &VerifiedIncomingPayment,
    next: Fp,
    index: u32,
    siblings: [Fp; 32],
    outcome: VerifiedCreditStatus,
    parameters: &AuthenticatedParameters,
) -> Result<(Vec<u8>, usize, usize)> {
    pack_frame(
        later,
        &Opening {
            next,
            path: Path { index, siblings },
        },
        outcome,
        parameters,
    )
}

#[cfg(test)]
mod opening_tests {
    use super::*;

    #[test]
    fn both_credit_status_profiles_enforce_member_shape_before_proof_decision() {
        let credit = Fp::from(7);
        let payment = Fp::from(11);
        for (key, digest, slot, next, valid) in [
            (credit, payment, 1, Fp::ZERO, true),
            (credit, payment, 1, Fp::from(8), true),
            (Fp::ZERO, payment, 1, Fp::ZERO, false),
            (credit, Fp::ZERO, 1, Fp::ZERO, false),
            (credit, payment, 0, Fp::ZERO, false),
            (credit, payment, 1, credit, false),
            (credit, payment, 1, Fp::from(6), false),
        ] {
            let opening = Opening {
                next,
                path: Path {
                    index: slot,
                    siblings: *EMPTY,
                },
            };
            let mut receive = Vec::new();
            opening.write(&mut receive);
            let parsed = Opening::read(&mut receive.as_slice()).unwrap();
            for status in [VerifiedCreditStatus::Credited, VerifiedCreditStatus::Burned] {
                let earlier = parsed.root(key, digest, status);
                assert_eq!(earlier.is_ok(), valid);
                let mut later = Vec::new();
                pack_opening(&mut later, &opening, sibling_mask(&opening));
                let result = read_opening(&mut later.as_slice(), false)
                    .and_then(|opening| opening.root(key, digest, status));
                assert_eq!(result.is_ok(), valid);
                assert_eq!(result.ok(), earlier.ok());
            }
        }
    }
}
