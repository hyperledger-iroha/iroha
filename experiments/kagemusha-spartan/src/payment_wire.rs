//! Complete future-generation Payment transport, including signed originals.
//!
//! This is a distinct experimental IPM1 profile, never an alternative parser
//! for the production wallet profile. IPM1 and KWPYPC1 framing/hash definitions
//! are identical to the generic Swift/Kotlin SDK transport. Only uncompressed
//! encoding is admitted, so the measured QR size includes every byte.
//!
//! Decoding is not admission: the parameter owner must verify both linked
//! proofs/accumulators and authenticate the root/issuer credential and its
//! payment-table commitment. No trust bit is manufactured here. All omitted
//! original fields and all 128 ordinary proof inputs are reconstructed exactly;
//! encoding refuses discrepancies instead of normalizing signed originals.

use crate::carrier::payment_words;
use blake2::{Blake2b, Digest, digest::consts::U32};
use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Eq, Fp, Fq, poseidon};
use num_bigint::BigUint;

use crate::{
    bootstrap::Receipt,
    carrier::{self, ProjectedInputs, TrustedProfile},
    certificate::{self, Certificate},
    credential::{self, Credential},
    payment,
    r1cs::Instance,
    request::{self, CompletedRequest},
    spartan::Proof,
    statement,
};

pub const PEER_PROFILE: u16 = 0x8001;
pub const PEER_SCHEMA: u16 = 3;
pub const QR_BUDGET: usize = 12_000;
pub const HEADER_BYTES: usize = 84;
const MAX_BYTES: usize = 12_182;
const MAX_ENVELOPE_BYTES: usize = 10_034;
const REQUEST_BYTES: usize = 304;
const RECEIPT_BYTES: usize = 128;
const CREDENTIAL_BYTES: usize = 530;
const CERTIFICATE_BYTES: usize = 137;
// Scheme/relation/asset/credential/kind and complete Request commitments are
// reconstructed from the authenticated profile and the carried originals.
const STATEMENT_FIELDS: [usize; 15] = [8, 9, 10, 11, 12, 13, 14, 15, 18, 19, 20, 21, 22, 24, 25];
const STATEMENT_BYTES: usize = STATEMENT_FIELDS.len() * 32;
const STATEMENT_WIDTHS: [usize; 15] = [1, 16, 16, 1, 16, 32, 32, 32, 16, 16, 16, 16, 16, 8, 8];

fn statement_positions() -> payment_words::Positions {
    STATEMENT_WIDTHS
        .into_iter()
        .enumerate()
        .filter_map(|(i, width)| {
            (width < 32).then_some((32 * i, payment_words::Encoding::Low(width)))
        })
        .collect()
}
fn statement_min_bytes() -> usize {
    statement_positions().len().div_ceil(8) + STATEMENT_WIDTHS.iter().sum::<usize>()
}

/// Check the genuine Send transport before an installed wallet can prepare a debit.
/// The Send relation bounds all statement integers and generator limbs. The fold
/// transcript supplies two short endoscalar coefficients for every challenge.
pub(crate) fn admit_send_profile(pair: &TrustedProfile) -> Result<Sizes, String> {
    let (total, positions) = pair.payment_generator_words()?;
    let pair_bytes = payment_words::all_short_size(total, &positions)?;
    genuine_sizes(pair_bytes)
}

pub(crate) fn genuine_sizes(pair: usize) -> Result<Sizes, String> {
    if pair > MAX_ENVELOPE_BYTES {
        return Err("installed Send exceeds Payment envelope limit".into());
    }
    // Use the actual fixed-layout Norito and IPM1 serializers. Only the vector
    // lengths affect sizing. This also counts the header and companion exactly.
    let envelope = encode_norito(&Envelope {
        version: 2,
        statement: vec![0; statement_min_bytes()],
        request: vec![0; REQUEST_BYTES],
        receipt: vec![0; RECEIPT_BYTES],
        policy_epoch: u64::MAX,
        pending_root: vec![0; 32],
        credit_root: vec![0; 32],
        pair: vec![0; pair],
    })?;
    let companion = encode_norito(&Companion {
        version: 1,
        credential: vec![0; CREDENTIAL_BYTES],
        certificate: vec![0; CERTIFICATE_BYTES],
    })?;
    let peer = peer_encode(&envelope, &companion)?;
    Ok(Sizes {
        peer: peer.len(),
        envelope: envelope.len(),
        companion: companion.len(),
        pair,
    })
}

fn compact_pair(profile: &TrustedProfile, pair: &[u8]) -> Result<Vec<u8>, String> {
    carrier::frame_projected_pair(profile, pair)?;
    let (_, positions) = profile.payment_generator_words()?;
    payment_words::pack(pair, &positions)
}
const COMPANION_LIMIT: usize = 2048;
const PAYLOAD_DOMAIN: &[u8] = b"IROHA-PEER-PAYLOAD-V1\0";
const MESSAGE_DOMAIN: &[u8] = b"IROHA-PEER-MESSAGE-V1\0";
const PAYMENT_MAGIC: &[u8; 8] = b"KWPYPC1\0";
const REQUEST_RANGES: [(usize, usize); 9] = [
    (98, 32),
    (162, 32),
    (210, 32),
    (258, 32),
    (306, 8),
    (314, 32),
    (346, 8),
    (354, 32),
    (386, 32),
];

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.payment.CompleteSend.v2")]
struct Envelope {
    version: u16,
    statement: Vec<u8>,
    request: Vec<u8>,
    receipt: Vec<u8>,
    policy_epoch: u64,
    pending_root: Vec<u8>,
    credit_root: Vec<u8>,
    pair: Vec<u8>,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::NoritoSerialize,
    norito::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.payment.PayerCompanion.v1")]
struct Companion {
    version: u16,
    credential: Vec<u8>,
    certificate: Vec<u8>,
}

/// Independently selected scheme/relation and authenticated pair parameters.
/// Constructing this view does not authenticate its inputs.
pub struct Profile<'a> {
    pub pair: &'a TrustedProfile,
    pub scheme: [u8; 32],
    pub relation: [u8; 32],
}

/// Every original needed to reproduce a complete Send Payment. Dynamic roots
/// are carried because they cannot be inferred from the public statement.
#[derive(Clone)]
pub struct Originals {
    pub payer: Credential,
    pub certificate: Certificate,
    pub request: CompletedRequest,
    pub statement: [Fp; 26],
    pub receipt: Receipt,
    pub policy_epoch: u64,
    pub pending_root: Fp,
    pub credit_root: Fp,
}

pub struct Decoded {
    pub originals: Originals,
    pub projection: ProjectedInputs,
    pub pair: carrier::Decoded,
    /// Exact completed pair carrier, including every final proof response.
    pub pair_bytes: Vec<u8>,
    pub digests: payment::Digests,
    pub size: Sizes,
}

/// Canonical, bounded transport with exact original proof bytes. Proof-word
/// parsing and proof acceptance belong to the total recursive relation. A
/// same-profile malformed point/scalar inside the admitted envelope remains
/// available for a derived Burn. Oversized transports are refused before custody.
pub struct Candidate {
    pub originals: Originals,
    pub projection: ProjectedInputs,
    pub pair_bytes: Vec<u8>,
    pub digests: payment::Digests,
    pub size: Sizes,
}

#[derive(Clone, Copy, Debug)]
pub struct Sizes {
    pub peer: usize,
    pub envelope: usize,
    pub companion: usize,
    pub pair: usize,
}

fn fp(bytes: &[u8]) -> Fp {
    certificate::fp(&BigUint::from_bytes_le(bytes))
}
fn field(bytes: &[u8]) -> Result<Fp, String> {
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| "field must be32 bytes")?;
    Option::<Fp>::from(Fp::from_repr(bytes)).ok_or_else(|| "noncanonical field".into())
}
fn short(value: Fp, bytes: usize) -> Result<Vec<u8>, String> {
    let raw = value.to_repr();
    if raw[bytes..].iter().any(|b| *b != 0) {
        return Err("bounded original field overflow".into());
    }
    Ok(raw[..bytes].to_vec())
}
fn identity(values: &[Fp]) -> Result<[u8; 32], String> {
    let mut out = [0; 32];
    out[..16].copy_from_slice(&short(values[0], 16)?);
    out[16..].copy_from_slice(&short(values[1], 16)?);
    Ok(out)
}
pub(super) fn limbs(bytes: &[u8]) -> [Fp; 2] {
    [fp(&bytes[..16]), fp(&bytes[16..])]
}

fn encode_norito<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    norito::to_bytes(value).map_err(|e| format!("Norito encode: {e}"))
}
fn norito_flags(bytes: &[u8], max: usize) -> Result<(), String> {
    if bytes.len() > max {
        return Err("bounded Norito object overflow".into());
    }
    let view = norito::core::from_bytes_view(bytes).map_err(|e| format!("Norito frame: {e}"))?;
    if view.flags() != norito::core::V1_LAYOUT_FLAGS {
        return Err("fixed Norito layout required".into());
    }
    Ok(())
}
fn limits(max: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(max, max, 4 * max, 8 * max, 16)
}
fn read_envelope(bytes: &[u8]) -> Result<Envelope, String> {
    norito_flags(bytes, MAX_ENVELOPE_BYTES)?;
    let value: Envelope = norito::decode_from_bytes_with_limits(bytes, limits(MAX_ENVELOPE_BYTES))
        .map_err(|e| format!("Payment Norito: {e}"))?;
    if encode_norito(&value)? != bytes {
        return Err("noncanonical Payment envelope".into());
    }
    let bitmap = statement_positions().len().div_ceil(8);
    let statement_min = statement_min_bytes();
    if value.version != 2
        || !(statement_min..=bitmap + STATEMENT_BYTES).contains(&value.statement.len())
        || value.request.len() != REQUEST_BYTES
        || value.receipt.len() != RECEIPT_BYTES
        || value.pending_root.len() != 32
        || value.credit_root.len() != 32
        || value.pair.is_empty()
    {
        return Err("Payment field version/length".into());
    }
    Ok(value)
}
fn read_companion(bytes: &[u8]) -> Result<Companion, String> {
    norito_flags(bytes, COMPANION_LIMIT)?;
    let value: Companion = norito::decode_from_bytes_with_limits(bytes, limits(COMPANION_LIMIT))
        .map_err(|e| format!("companion Norito: {e}"))?;
    if encode_norito(&value)? != bytes {
        return Err("noncanonical payer companion".into());
    }
    if value.version != 1
        || value.credential.len() != CREDENTIAL_BYTES
        || value.certificate.len() != CERTIFICATE_BYTES
    {
        return Err("payer companion field version/length".into());
    }
    Ok(value)
}

fn companion_restore(
    profile: &Profile<'_>,
    compact: &Companion,
) -> Result<(Credential, Certificate), String> {
    if compact.version != 1
        || compact.credential.len() != CREDENTIAL_BYTES
        || compact.certificate.len() != CERTIFICATE_BYTES
    {
        return Err("compact payer companion version/length".into());
    }
    let mut certificate = Certificate {
        body: [0; 108],
        signature: compact.certificate[73..].try_into().unwrap(),
    };
    certificate.body[0] = 1;
    certificate.body[2..34].copy_from_slice(&profile.scheme);
    certificate.body[34] = 1;
    certificate.body[35..].copy_from_slice(&compact.certificate[..73]);
    let mut payer = Credential {
        body: [0; 532],
        signature: compact.credential[466..].try_into().unwrap(),
    };
    payer.body[0] = 1;
    payer.body[2..34].copy_from_slice(&profile.scheme);
    payer.body[34..500].copy_from_slice(&compact.credential[..466]);
    payer.body[500..].copy_from_slice(&certificate::certificate_digest(&certificate).to_repr());
    Ok((payer, certificate))
}

fn companion_compact(profile: &Profile<'_>, o: &Originals) -> Result<Companion, String> {
    let compact = Companion {
        version: 1,
        credential: [&o.payer.body[34..500], &o.payer.signature].concat(),
        certificate: [&o.certificate.body[35..], &o.certificate.signature].concat(),
    };
    let (payer, certificate) = companion_restore(profile, &compact)?;
    if payer.body != o.payer.body
        || payer.signature != o.payer.signature
        || certificate.body != o.certificate.body
        || certificate.signature != o.certificate.signature
    {
        return Err("payer companion originals differ from compact reconstruction".into());
    }
    Ok(compact)
}

fn request_restore(
    compact: &[u8],
    payer: &Credential,
    s: &[Fp; 26],
) -> Result<CompletedRequest, String> {
    if compact.len() != REQUEST_BYTES {
        return Err("compact Request length".into());
    }
    let mut body = [0; 418];
    body[0] = 1;
    body[2..98].copy_from_slice(&payer.body[2..98]);
    body[130..162].copy_from_slice(&identity(&s[18..20])?);
    body[194..210].copy_from_slice(&short(s[20], 16)?);
    body[242..258].copy_from_slice(&short(s[21], 16)?);
    body[290..306].copy_from_slice(&short(s[22], 16)?);
    let mut offset = 0;
    for (start, len) in REQUEST_RANGES {
        body[start..start + len].copy_from_slice(&compact[offset..offset + len]);
        offset += len;
    }
    Ok(CompletedRequest {
        body,
        signature: compact[offset..].try_into().unwrap(),
    })
}
fn receipt_restore(compact: &[u8], payer: &Credential, s: &[Fp; 26]) -> Result<Receipt, String> {
    if compact.len() != RECEIPT_BYTES {
        return Err("compact receipt length".into());
    }
    let mut body = [0; 338];
    body[0] = 1;
    body[2..34].copy_from_slice(&payer.body[2..34]);
    body[34..66].copy_from_slice(&payer.body[66..98]);
    body[66..98].copy_from_slice(&payer.body[259..291]);
    body[98..114].copy_from_slice(&short(s[9], 16)?);
    let wallet = limbs(&payer.body[66..98]);
    let op = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[wallet[0], wallet[1], Fp::from(3), s[17]],
    );
    for (offset, value) in [
        (114, op),
        (146, s[14]),
        (178, s[15]),
        (210, statement::digest(s)),
    ] {
        body[offset..offset + 32].copy_from_slice(&value.to_repr());
    }
    body[242..306].copy_from_slice(&compact[..64]);
    Ok(Receipt {
        body,
        signature: compact[64..].try_into().unwrap(),
    })
}

fn statement_restore(
    profile: &Profile<'_>,
    payer: &Credential,
    compact: &[u8],
    request: &[u8],
) -> Result<([Fp; 26], CompletedRequest), String> {
    let compact = payment_words::unpack(compact, STATEMENT_BYTES, &statement_positions())?;
    let mut s = [Fp::ZERO; 26];
    s[0] = Fp::ONE;
    s[1..3].copy_from_slice(&limbs(&profile.relation));
    s[3..5].copy_from_slice(&limbs(&profile.scheme));
    s[5..7].copy_from_slice(&limbs(&payer.body[34..66]));
    s[7] = credential::digest(payer);
    s[16] = Fp::from(3);
    for (i, word) in STATEMENT_FIELDS.into_iter().zip(compact.chunks_exact(32)) {
        s[i] = field(word)?;
    }
    let request = request_restore(request, payer, &s)?;
    let commitments = request::commitments(&request).map_err(str::to_string)?;
    s[17] = commitments.credit;
    s[23] = commitments.digest;
    Ok((s, request))
}

/// Reconstruct the complete ordinary IO. No claimed proof inputs are read.
pub fn projection(profile: &Profile<'_>, o: &Originals) -> Result<ProjectedInputs, String> {
    let s = &o.statement;
    if profile.scheme == [0; 32]
        || profile.relation == [0; 32]
        || profile.pair.id() != &profile.relation
        || o.payer.body[..2] != [1, 0]
        || s[0] != Fp::ONE
        || s[16] != Fp::from(3)
        || o.payer.body[2..34] != profile.scheme
        || identity(&s[1..3])? != profile.relation
        || identity(&s[3..5])? != profile.scheme
        || identity(&s[5..7])? != o.payer.body[34..66]
        || s[7] != credential::digest(&o.payer)
    {
        return Err("Payment scheme/relation/credential binding".into());
    }
    if o.certificate.body[..2] != [1, 0]
        || o.certificate.body[2..34] != profile.scheme
        || o.certificate.body[34] != 1
        || o.payer.body[500..532] != certificate::certificate_digest(&o.certificate).to_repr()
    {
        return Err("Enrollment certificate companion binding".into());
    }
    let request = request::commitments(&o.request).map_err(str::to_string)?;
    if request.credit != s[17] || request.digest != s[23] {
        return Err("complete Request statement binding".into());
    }
    let mut context = [Fp::ZERO; 13];
    context[..2].copy_from_slice(&limbs(&profile.scheme));
    context[2..4].copy_from_slice(&limbs(&profile.relation));
    context[4..6].copy_from_slice(&limbs(&o.payer.body[34..66]));
    context[6..8].copy_from_slice(&limbs(&o.payer.body[66..98]));
    context[8] = credential::digest(&o.payer);
    for (i, start) in [147, 131, 179, 163].into_iter().enumerate() {
        context[9 + i] = certificate::fp(&BigUint::from_bytes_be(&o.payer.body[start..start + 16]));
    }
    let table = std::array::from_fn(|i| fp(&o.payer.body[195 + 16 * i..211 + 16 * i]));
    ProjectedInputs::from_authenticated_fields(
        s[15],
        statement::digest(s),
        crate::receipt::digest(&o.receipt),
        context,
        [
            s[8],
            Fp::from(o.policy_epoch),
            s[11],
            s[12],
            o.pending_root,
            o.credit_root,
        ],
        table,
        crate::full::SEND,
        [Fp::ZERO; 2],
    )
}

fn compact(
    profile: &Profile<'_>,
    o: &Originals,
    pair: Vec<u8>,
) -> Result<(Envelope, Companion), String> {
    let companion = companion_compact(profile, o)?;
    let mut request = Vec::with_capacity(REQUEST_BYTES);
    for (start, len) in REQUEST_RANGES {
        request.extend_from_slice(&o.request.body[start..start + len]);
    }
    request.extend_from_slice(&o.request.signature);
    let statement: Vec<_> = STATEMENT_FIELDS
        .iter()
        .flat_map(|i| o.statement[*i].to_repr())
        .collect();
    let statement = payment_words::pack(&statement, &statement_positions())?;
    let (restored_statement, restored) =
        statement_restore(profile, &o.payer, &statement, &request)?;
    if restored_statement != o.statement {
        return Err("statement original differs from compact reconstruction".into());
    }
    if restored.body != o.request.body || restored.signature != o.request.signature {
        return Err("Request original differs from compact reconstruction".into());
    }
    let mut receipt = o.receipt.body[242..306].to_vec();
    receipt.extend_from_slice(&o.receipt.signature);
    let restored = receipt_restore(&receipt, &o.payer, &o.statement)?;
    if restored.body != o.receipt.body || restored.signature != o.receipt.signature {
        return Err("receipt original differs from compact reconstruction".into());
    }
    let envelope = Envelope {
        version: 2,
        statement,
        request,
        receipt,
        policy_epoch: o.policy_epoch,
        pending_root: o.pending_root.to_repr().to_vec(),
        credit_root: o.credit_root.to_repr().to_vec(),
        pair,
    };
    Ok((envelope, companion))
}

fn hash(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Blake2b::<U32>::new();
    for part in parts {
        h.update(part);
    }
    h.finalize().into()
}
#[derive(Copy, Clone)]
pub(super) enum PeerOriginals {
    Payment,
    OpenRequest,
}
impl PeerOriginals {
    fn fields(self) -> (u8, &'static [u8; 8], usize, usize) {
        match self {
            Self::Payment => (3, PAYMENT_MAGIC, MAX_ENVELOPE_BYTES, COMPANION_LIMIT),
            Self::OpenRequest => (2, b"KWRQAC1\0", 1677, 4096),
        }
    }
    pub(super) fn encode(self, envelope: &[u8], companion: &[u8]) -> Result<Vec<u8>, String> {
        let (kind, magic, envelope_max, companion_max) = self.fields();
        if envelope.is_empty()
            || envelope.len() > envelope_max
            || companion.is_empty()
            || companion.len() > companion_max
            || envelope
                .len()
                .saturating_add(companion.len())
                .saturating_add(100)
                > MAX_BYTES
        {
            return Err("peer payload resource limit".into());
        }
        let mut payload = Vec::new();
        payload.extend_from_slice(magic);
        payload.extend_from_slice(&(envelope.len() as u32).to_be_bytes());
        payload.extend_from_slice(&(companion.len() as u32).to_be_bytes());
        payload.extend_from_slice(envelope);
        payload.extend_from_slice(companion);
        Ok(encode_peer_payload(kind, &payload))
    }
    pub(super) fn decode(self, bytes: &[u8]) -> Result<(&[u8], &[u8]), String> {
        let (kind, magic, envelope_max, companion_max) = self.fields();
        if bytes.len() < HEADER_BYTES + 16
            || bytes.len() > HEADER_BYTES + 16 + envelope_max + companion_max
        {
            return Err("peer message length".into());
        }
        let payload = decode_peer_payload(kind, bytes, 16 + envelope_max + companion_max)?;
        if &payload[..8] != magic {
            return Err("peer companion wrapper magic".into());
        }
        let elen = u32::from_be_bytes(payload[8..12].try_into().unwrap()) as usize;
        let clen = u32::from_be_bytes(payload[12..16].try_into().unwrap()) as usize;
        if elen == 0
            || elen > envelope_max
            || clen == 0
            || clen > companion_max
            || elen.checked_add(clen).and_then(|n| n.checked_add(16)) != Some(payload.len())
        {
            return Err("peer companion wrapper lengths".into());
        }
        Ok((&payload[16..16 + elen], &payload[16 + elen..]))
    }
}
/// IPM1 framing shared by signed Request, proven Payment and credit-membership evidence.
pub(super) fn encode_peer_payload(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut prefix = Vec::new();
    prefix.extend_from_slice(b"IPM1");
    prefix.extend_from_slice(&[1, 0]);
    prefix.extend_from_slice(&PEER_PROFILE.to_be_bytes());
    prefix.extend_from_slice(&[kind, 0]);
    prefix.extend_from_slice(&PEER_SCHEMA.to_be_bytes());
    prefix.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    prefix.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    prefix.extend_from_slice(&hash(&[
        PAYLOAD_DOMAIN,
        &PEER_PROFILE.to_be_bytes(),
        &[kind],
        &PEER_SCHEMA.to_be_bytes(),
        payload,
    ]));
    let wire_hash = hash(&[MESSAGE_DOMAIN, &prefix, payload]);
    prefix.extend_from_slice(&wire_hash);
    prefix.extend_from_slice(payload);
    prefix
}
pub(super) fn decode_peer_payload(kind: u8, bytes: &[u8], maximum: usize) -> Result<&[u8], String> {
    if bytes.len() <= HEADER_BYTES || bytes.len() > HEADER_BYTES + maximum {
        return Err("peer message length".into());
    }
    if &bytes[..4] != b"IPM1"
        || bytes[4..6] != [1, 0]
        || bytes[6..8] != PEER_PROFILE.to_be_bytes()
        || bytes[8..10] != [kind, 0]
        || bytes[10..12] != PEER_SCHEMA.to_be_bytes()
    {
        return Err("future IPM1 profile/schema/header required".into());
    }
    let read = |i| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
    let len = bytes.len() - HEADER_BYTES;
    if read(12) != len || read(16) != len {
        return Err("peer body length mismatch".into());
    }
    let payload = &bytes[HEADER_BYTES..];
    if bytes[20..52]
        != hash(&[
            PAYLOAD_DOMAIN,
            &PEER_PROFILE.to_be_bytes(),
            &[kind],
            &PEER_SCHEMA.to_be_bytes(),
            payload,
        ])
        || bytes[52..84] != hash(&[MESSAGE_DOMAIN, &bytes[..52], payload])
    {
        return Err("peer payload/message hash mismatch".into());
    }
    Ok(payload)
}
fn peer_encode(envelope: &[u8], companion: &[u8]) -> Result<Vec<u8>, String> {
    PeerOriginals::Payment.encode(envelope, companion)
}
fn peer_decode(bytes: &[u8]) -> Result<(&[u8], &[u8]), String> {
    PeerOriginals::Payment.decode(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode(
    profile: &Profile<'_>,
    originals: &Originals,
    ip: &Instance<Fp>,
    is: &Instance<Fq>,
    pp: &Proof<Eq>,
    ps: &Proof<Ep>,
    r: [u8; 32],
) -> Result<Vec<u8>, String> {
    let projection = projection(profile, originals)?;
    let pair = carrier::encode_projected_pair(profile.pair, ip, is, pp, ps, r, &projection)?;
    let (envelope, companion) = compact(profile, originals, compact_pair(profile.pair, &pair)?)?;
    peer_encode(&encode_norito(&envelope)?, &encode_norito(&companion)?)
}

pub fn decode(profile: &Profile<'_>, bytes: &[u8]) -> Result<Decoded, String> {
    let Candidate {
        originals,
        projection,
        pair_bytes,
        digests,
        size,
    } = decode_candidate(profile, bytes)?;
    let pair = carrier::decode_projected_pair(profile.pair, &pair_bytes, &projection)?;
    Ok(Decoded {
        originals,
        projection,
        pair,
        pair_bytes,
        digests,
        size,
    })
}

/// The account named by the canonical Payment companion and restored signed Request.
pub(crate) fn payer_policy_accounts(
    profile: &Profile<'_>,
    bytes: &[u8],
) -> Result<[[u8; 32]; 2], String> {
    let (envelope, companion) = peer_decode(bytes)?;
    let envelope = read_envelope(envelope)?;
    let (payer, _) = companion_restore(profile, &read_companion(companion)?)?;
    let (_, request) = statement_restore(profile, &payer, &envelope.statement, &envelope.request)?;
    Ok([
        request.body[98..130]
            .try_into()
            .expect("fixed Request account"),
        payer.body[98..130].try_into().expect("fixed payer account"),
    ])
}

pub fn decode_candidate(profile: &Profile<'_>, bytes: &[u8]) -> Result<Candidate, String> {
    let (ebytes, cbytes) = peer_decode(bytes)?;
    let envelope = read_envelope(ebytes)?;
    let companion = read_companion(cbytes)?;
    let (payer, certificate) = companion_restore(profile, &companion)?;
    let (statement, request) =
        statement_restore(profile, &payer, &envelope.statement, &envelope.request)?;
    let originals = Originals {
        request,
        receipt: receipt_restore(&envelope.receipt, &payer, &statement)?,
        payer,
        certificate,
        statement,
        policy_epoch: envelope.policy_epoch,
        pending_root: field(&envelope.pending_root)?,
        credit_root: field(&envelope.credit_root)?,
    };
    let projection = projection(profile, &originals)?;
    let (total, positions) = profile.pair.payment_generator_words()?;
    let pair_bytes = payment_words::unpack(&envelope.pair, total, &positions)?;
    carrier::frame_projected_pair(profile.pair, &pair_bytes)?;
    let digests = payment::digests(
        &originals.request,
        &originals.payer,
        &originals.statement,
        &originals.receipt,
        &pair_bytes,
    )
    .map_err(str::to_string)?;
    let size = Sizes {
        peer: bytes.len(),
        envelope: ebytes.len(),
        companion: cbytes.len(),
        pair: envelope.pair.len(),
    };
    Ok(Candidate {
        originals,
        projection,
        pair_bytes,
        digests,
        size,
    })
}

/// Reframe an exact-profile malformed candidate for native Burn integration tests.
#[cfg(test)]
pub(crate) fn test_replace_pair(
    profile: &Profile<'_>,
    payment: &[u8],
    pair: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let candidate = decode_candidate(profile, payment)?;
    test_reframe_originals(profile, &candidate.originals, pair)
}

/// Reframe changed signed originals for total-candidate integration tests.
/// Projection and exact compact restoration remain mandatory; signature and
/// proof validity are deliberately decided by the candidate relation.
#[cfg(test)]
pub(crate) fn test_reframe_originals(
    profile: &Profile<'_>,
    originals: &Originals,
    pair: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let _ = projection(profile, originals)?;
    carrier::frame_projected_pair(profile.pair, &pair)?;
    let (envelope, companion) = compact(profile, originals, compact_pair(profile.pair, &pair)?)?;
    peer_encode(&encode_norito(&envelope)?, &encode_norito(&companion)?)
}

#[cfg(test)]
#[path = "payment_transport_fixture_tests.rs"]
mod transport_fixture;
#[cfg(test)]
pub(crate) fn check_historical_payment_transport(profile: &Profile<'_>, old: &[u8]) {
    transport_fixture::check(profile, old);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fs::SpikeRng,
        ipa::Gens,
        r1cs::{self, Shape, ToyCircuit},
        verifier,
    };

    #[test]
    fn installed_send_must_fit_all_wire_limits_before_debit() {
        // The fixed framing is shared with the real encoder; derive the exact
        // last admitted pair for this schema, then exercise its next byte.
        let overhead = genuine_sizes(1).unwrap().envelope - 1;
        let last = MAX_ENVELOPE_BYTES - overhead;
        let sizes = genuine_sizes(last).unwrap();
        assert_eq!(sizes.envelope, MAX_ENVELOPE_BYTES);
        assert_eq!(
            sizes.peer,
            sizes.envelope + sizes.companion + HEADER_BYTES + 16
        );
        assert!(sizes.peer <= MAX_BYTES && sizes.companion <= COMPANION_LIMIT);
        assert!(genuine_sizes(last + 1).is_err());
        assert!(genuine_sizes(usize::MAX).is_err());
    }

    struct Harness {
        trusted: TrustedProfile,
        originals: Originals,
        scheme: [u8; 32],
        relation: [u8; 32],
        p: Shape,
        s: Shape,
        gp: Gens<Eq>,
        gs: Gens<Ep>,
    }
    impl Harness {
        fn new() -> Self {
            let fixture = crate::send::tests::fixture(6, false);
            let key_gens = Gens::<Eq>::new(1 << 16);
            let (context, initial) = crate::bootstrap::fixture(&fixture.key, &key_gens);
            let input = fixture.input;
            let originals = Originals {
                payer: input.credential,
                certificate: initial.certificate,
                request: input.request,
                statement: input.statement,
                receipt: input.receipt,
                policy_epoch: u64::from_le_bytes(
                    input.after_core[27].to_repr()[..8].try_into().unwrap(),
                ),
                pending_root: input.after_core[18],
                credit_root: fixture.predecessor[37],
            };
            let p = Shape {
                label: "complete-wire-test-p",
                sx: 10,
                row_bits: 2,
                col_bits: 8,
                k: 1,
            };
            let s = Shape {
                label: "complete-wire-test-s",
                ..p
            };
            let trusted = TrustedProfile::from_parameters(
                context.relation,
                p,
                &ToyCircuit,
                s,
                &ToyCircuit,
                crate::fs::lean_schedule(),
            )
            .unwrap();
            Self {
                trusted,
                originals,
                scheme: context.root.scheme,
                relation: context.relation,
                p,
                s,
                gp: Gens::new(p.cols()),
                gs: Gens::new(s.cols()),
            }
        }
        fn profile(&self) -> Profile<'_> {
            Profile {
                pair: &self.trusted,
                scheme: self.scheme,
                relation: self.relation,
            }
        }
        fn spec(&self) -> verifier::PairSpec<'_> {
            let mut rng = SpikeRng::new(b"complete Payment transport proof witnesses");
            let (_, wp) = r1cs::sample(&self.p, &mut rng);
            let (_, ws) = r1cs::sample(&self.s, &mut rng);
            let io = projection(&self.profile(), &self.originals).unwrap();
            verifier::PairSpec {
                shape_p: self.p,
                gens_p: &self.gp,
                inst_p: io.primary().clone(),
                wit_p: wp,
                claims_p: vec![],
                shape_s: self.s,
                gens_s: &self.gs,
                inst_s: io.secondary().clone(),
                wit_s: ws,
                claims_s: vec![],
            }
        }
    }

    #[test]
    fn candidate_keeps_malformed_fixed_profile_proof_bytes_for_total_verification() {
        let h = Harness::new();
        let (raw, _) = carrier::canonical_projected_template(
            &h.trusted,
            &vec![255; h.p.proof_bytes()],
            &vec![0; h.s.proof_bytes()],
            [0; 32],
        )
        .unwrap();
        let (envelope, companion) = compact(&h.profile(), &h.originals, raw.clone()).unwrap();
        let packet = peer_encode(
            &encode_norito(&envelope).unwrap(),
            &encode_norito(&companion).unwrap(),
        )
        .unwrap();
        let candidate = decode_candidate(&h.profile(), &packet).unwrap();
        assert_eq!(candidate.pair_bytes, raw);
        assert_eq!(
            candidate.digests.proof,
            payment::proof_digest(&raw).unwrap()
        );
        let selected =
            carrier::decode_projected_candidate(&h.trusted, &raw, &candidate.projection).unwrap();
        assert_eq!(selected.r, [0; 32]);
        assert!(
            selected
                .pp
                .to_bytes()
                .iter()
                .chain(&selected.ps.to_bytes())
                .all(|b| *b == 0)
        );
        assert!(
            decode(&h.profile(), &packet).is_err(),
            "strict proof decoding still refuses the invalid word"
        );
        let mut wrong = envelope;
        wrong.pair[0] ^= 1;
        let packet = peer_encode(
            &encode_norito(&wrong).unwrap(),
            &encode_norito(&companion).unwrap(),
        )
        .unwrap();
        assert!(
            decode_candidate(&h.profile(), &packet).is_err(),
            "profile framing is mandatory before a candidate exists"
        );
    }

    #[test]
    fn complete_peer_message_restores_originals_and_real_linked_proofs() {
        let h = Harness::new();
        let profile = h.profile();
        let mut spec = h.spec();
        let proved = verifier::prove_pair(&spec, b"complete Payment wire test", &|rho| {
            verifier::rlc_value(rho, Fq::from(19), &[11, 29]).to_repr()
        });
        let bytes = encode(
            &profile,
            &h.originals,
            &spec.inst_p,
            &spec.inst_s,
            &proved.p,
            &proved.s,
            proved.r,
        )
        .unwrap();
        let restored = decode(&profile, &bytes).unwrap();
        let o = &restored.originals;
        assert_ne!(
            o.receipt.body[274..306],
            [0; 32],
            "real capsule identity survives transport"
        );
        assert_eq!(
            o.receipt.body[306..338],
            [0; 32],
            "Send carries the canonical zero Payment field"
        );
        assert_ne!(
            o.payer.body[34..66],
            o.payer.body[66..98],
            "asset and wallet are distinct fixture identities"
        );
        assert_eq!(o.payer.body, h.originals.payer.body);
        assert_eq!(o.payer.signature, h.originals.payer.signature);
        assert_eq!(o.certificate.body, h.originals.certificate.body);
        assert_eq!(o.certificate.signature, h.originals.certificate.signature);
        assert_eq!(o.request.body, h.originals.request.body);
        assert_eq!(o.request.signature, h.originals.request.signature);
        assert_eq!(o.receipt.body, h.originals.receipt.body);
        assert_eq!(o.receipt.signature, h.originals.receipt.signature);
        assert_eq!(o.statement, h.originals.statement);
        assert_eq!(o.policy_epoch, h.originals.policy_epoch);
        assert_eq!(o.pending_root, h.originals.pending_root);
        assert_eq!(o.credit_root, h.originals.credit_root);
        assert_eq!(restored.pair.ip.io, spec.inst_p.io);
        assert_eq!(restored.pair.is.io, spec.inst_s.io);
        assert_eq!((restored.pair.d, restored.pair.rho), (proved.d, proved.rho));
        verifier::verify_pair(&spec, &restored.pair.pp, &restored.pair.ps, restored.pair.r)
            .unwrap();
        assert_eq!(
            restored.digests.proof,
            payment::proof_digest(&restored.pair_bytes).unwrap()
        );
        let (exact_pair, ranges) = carrier::canonical_projected_template(
            &h.trusted,
            &proved.p.to_bytes(),
            &proved.s.to_bytes(),
            proved.r,
        )
        .unwrap();
        assert_eq!(exact_pair, restored.pair_bytes);
        assert_eq!(&exact_pair[ranges.r.clone()], proved.r.as_slice());
        assert_eq!(&exact_pair[ranges.primary.clone()], proved.p.to_bytes());
        assert_eq!(&exact_pair[ranges.secondary.clone()], proved.s.to_bytes());
        assert_eq!(ranges.primary.start, 74);
        assert_eq!(
            ranges.total,
            74 + proved.p.to_bytes().len() + proved.s.to_bytes().len()
        );
        for which in [0, 8, 9, 10, 41] {
            let mut malformed = exact_pair.clone();
            malformed[which] ^= 1;
            assert!(
                carrier::decode_projected_pair(&h.trusted, &malformed, &restored.projection)
                    .is_err()
            );
        }
        assert_ne!(
            restored.digests.proof,
            field(&o.receipt.body[242..274]).unwrap(),
            "signed prep digest is separate from completed recursive proof digest"
        );
        assert_eq!(
            encode(
                &profile,
                o,
                &restored.pair.ip,
                &restored.pair.is,
                &restored.pair.pp,
                &restored.pair.ps,
                restored.pair.r
            )
            .unwrap(),
            bytes
        );
        assert_eq!(
            restored.size.peer,
            HEADER_BYTES + 16 + restored.size.envelope + restored.size.companion
        );
        println!(
            "complete Payment codec with actual toy linked proofs: {} B IPM1 =84 header+16 wrapper+{} Norito envelope+{} Norito credential/certificate companion; pair{} B; compact originals244+304+128+72+530+137 (eleven statement fields and101 companion bytes reconstructed exactly); fixed-point monetary proof size still requires integrated backend measurement",
            restored.size.peer, restored.size.envelope, restored.size.companion, restored.size.pair
        );
        // These are small genuine linked proofs for codec/verification coverage,
        // not a claim that the complete fixed-point monetary pair fits12KB.
        assert!(restored.size.peer < QR_BUDGET);
        let (eb, cb) = peer_decode(&bytes).unwrap();
        for which in 0..4 {
            let mut envelope = read_envelope(eb).unwrap();
            match which {
                0 => envelope.policy_epoch += 1,
                1 => {
                    envelope.pending_root = (h.originals.pending_root + Fp::ONE).to_repr().to_vec()
                }
                2 => envelope.credit_root = (h.originals.credit_root + Fp::ONE).to_repr().to_vec(),
                _ => envelope.receipt[64] ^= 1,
            }
            let altered = peer_encode(&encode_norito(&envelope).unwrap(), cb).unwrap();
            let d = decode(&profile, &altered).unwrap();
            spec.inst_p = d.pair.ip;
            spec.inst_s = d.pair.is;
            assert!(
                verifier::verify_pair(&spec, &d.pair.pp, &d.pair.ps, d.pair.r).is_err(),
                "altered reconstructed public source{which}"
            );
        }
        let mut wrong = profile;
        wrong.relation[0] ^= 1;
        assert!(decode(&wrong, &bytes).is_err());
    }

    #[test]
    fn compact_encoding_never_normalizes_any_omitted_signed_byte() {
        let h = Harness::new();
        let original = &h.originals;
        let (e, c) = compact(&h.profile(), original, vec![1]).unwrap();
        assert_eq!(
            (
                e.statement.len(),
                e.request.len(),
                e.receipt.len(),
                c.credential.len(),
                c.certificate.len()
            ),
            (244, 304, 128, 530, 137)
        );
        assert_eq!(encode_norito(&c).unwrap().len(), 749);
        let (payer, certificate) = companion_restore(&h.profile(), &c).unwrap();
        assert_eq!(payer.body, original.payer.body);
        assert_eq!(payer.signature, original.payer.signature);
        assert_eq!(certificate.body, original.certificate.body);
        assert_eq!(certificate.signature, original.certificate.signature);
        let mut omitted_companion_bytes = 0;
        for i in (0..34).chain(500..532) {
            let mut changed = original.clone();
            changed.payer.body[i] ^= 1;
            assert!(
                compact(&h.profile(), &changed, vec![1]).is_err(),
                "omitted original credential byte{i}"
            );
            omitted_companion_bytes += 1;
        }
        for i in 0..35 {
            let mut changed = original.clone();
            changed.certificate.body[i] ^= 1;
            assert!(
                compact(&h.profile(), &changed, vec![1]).is_err(),
                "omitted original certificate byte{i}"
            );
            omitted_companion_bytes += 1;
        }
        assert_eq!(omitted_companion_bytes, 101);
        for i in (0..26).filter(|i| !STATEMENT_FIELDS.contains(i)) {
            let mut changed = original.clone();
            changed.statement[i] += Fp::ONE;
            assert!(
                compact(&h.profile(), &changed, vec![1]).is_err(),
                "omitted original statement field {i} must never be normalized"
            );
        }
        let mut unique_request = vec![false; 418];
        for (start, len) in REQUEST_RANGES {
            unique_request[start..start + len].fill(true);
        }
        for i in 0..418 {
            let mut changed = original.clone();
            changed.request.body[i] ^= 1;
            let mut compact_request = Vec::new();
            for (start, len) in REQUEST_RANGES {
                compact_request.extend_from_slice(&changed.request.body[start..start + len]);
            }
            compact_request.extend_from_slice(&changed.request.signature);
            let restored =
                request_restore(&compact_request, &changed.payer, &changed.statement).unwrap();
            assert_eq!(
                restored.body == changed.request.body,
                unique_request[i],
                "original Request byte{i}"
            );
            if !unique_request[i] {
                assert!(compact(&h.profile(), &changed, vec![1]).is_err());
            }
        }
        for i in 0..338 {
            let mut changed = original.clone();
            changed.receipt.body[i] ^= 1;
            assert_eq!(
                compact(&h.profile(), &changed, vec![1]).is_ok(),
                (242..306).contains(&i),
                "original receipt byte{i}"
            );
        }
        let io = projection(&h.profile(), original).unwrap();
        // All dynamic public fields have their exact ABI positions.
        assert_eq!(
            &io.primary().io[32..38],
            &[
                original.statement[8],
                Fp::from(original.policy_epoch),
                original.statement[11],
                original.statement[12],
                original.pending_root,
                original.credit_root
            ]
        );
        let mut wrong = original.clone();
        wrong.certificate.signature[0] ^= 1;
        assert!(projection(&h.profile(), &wrong).is_err());
        wrong = original.clone();
        wrong.payer.body[195] ^= 1;
        assert!(projection(&h.profile(), &wrong).is_err());
    }

    #[test]
    fn peer_hashes_match_independent_golden_and_reject_every_corruption() {
        // Python hashlib.blake2b(digest_size=32), same domains/header ordering
        // as IrohaSwift and Kotlin IrohaPeerWireV1. No SDK schema1 fallback.
        let golden = "49504d3101008001030000030000001c0000001cf83aa04c53de28fe0dc2f08b7bde07f0c91bc10e06eafb5a2659a6a88d4fa5f5d2e9b15458e1fffb6df98f1955617a6865376ee955ecabc03f61c64dc5f8dc684b575059504331000000000300000009656e76636f6d70616e696f6e";
        let bytes = peer_encode(b"env", b"companion").unwrap();
        let expected: Vec<_> = golden
            .as_bytes()
            .chunks_exact(2)
            .map(|x| u8::from_str_radix(std::str::from_utf8(x).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(bytes, expected);
        assert_eq!(
            peer_decode(&bytes).unwrap(),
            (b"env".as_slice(), b"companion".as_slice())
        );
        for n in 0..bytes.len() {
            assert!(peer_decode(&bytes[..n]).is_err(), "prefix{n}");
        }
        for i in 0..bytes.len() {
            let mut b = bytes.clone();
            b[i] ^= 1;
            assert!(peer_decode(&b).is_err(), "byte{i}");
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(peer_decode(&trailing).is_err());
        assert!(peer_decode(&vec![0; MAX_BYTES + 1]).is_err());
        assert!(peer_encode(&vec![1; MAX_BYTES], b"x").is_err());
        let max = peer_encode(&vec![7; MAX_ENVELOPE_BYTES], &vec![8; COMPANION_LIMIT]).unwrap();
        assert_eq!(max.len(), MAX_BYTES);
        assert!(peer_decode(&max).is_ok());
        assert!(peer_encode(&vec![7; MAX_ENVELOPE_BYTES + 1], b"x").is_err());
        // Correct IPM1 hashes with one byte moved from companion to envelope.
        let mut over = max;
        over[HEADER_BYTES + 8..HEADER_BYTES + 12]
            .copy_from_slice(&((MAX_ENVELOPE_BYTES + 1) as u32).to_be_bytes());
        over[HEADER_BYTES + 12..HEADER_BYTES + 16]
            .copy_from_slice(&((COMPANION_LIMIT - 1) as u32).to_be_bytes());
        let digest = hash(&[
            PAYLOAD_DOMAIN,
            &PEER_PROFILE.to_be_bytes(),
            &[3],
            &PEER_SCHEMA.to_be_bytes(),
            &over[HEADER_BYTES..],
        ]);
        over[20..52].copy_from_slice(&digest);
        let digest = hash(&[MESSAGE_DOMAIN, &over[..52], &over[HEADER_BYTES..]]);
        over[52..84].copy_from_slice(&digest);
        assert!(peer_decode(&over).is_err());
        assert!(peer_encode(b"x", &vec![1; COMPANION_LIMIT + 1]).is_err());
    }

    #[test]
    fn canonical_norito_and_fixed_original_lengths_are_mandatory() {
        let h = Harness::new();
        let (e, c) = compact(&h.profile(), &h.originals, vec![1]).unwrap();
        let eb = encode_norito(&e).unwrap();
        let cb = encode_norito(&c).unwrap();
        assert!(read_envelope(&eb).is_ok());
        assert!(read_companion(&cb).is_ok());
        for which in 0..8 {
            let mut e = e.clone();
            match which {
                0 => e.version = 1,
                1 => {
                    e.statement.pop();
                }
                2 => {
                    e.request.push(0);
                }
                3 => {
                    e.receipt.pop();
                }
                4 => {
                    e.pending_root.pop();
                }
                5 => {
                    e.credit_root.push(0);
                }
                6 => e.pair.clear(),
                _ => e.statement[..32].fill(255),
            }
            let encoded = encode_norito(&e).unwrap();
            let result = read_envelope(&encoded);
            if which < 7 {
                assert!(result.is_err(), "accepted corrupt envelope field {which}");
            } else {
                let peer = peer_encode(&encoded, &cb).unwrap();
                assert!(decode(&h.profile(), &peer).is_err());
            }
        }
        for i in 0..3 {
            let mut c = c.clone();
            match i {
                0 => c.version = 2,
                1 => {
                    c.credential.pop();
                }
                _ => {
                    c.certificate.push(0);
                }
            }
            assert!(read_companion(&encode_norito(&c).unwrap()).is_err());
        }
        for mut bytes in [eb, cb] {
            bytes.push(0);
            assert!(read_envelope(&bytes).is_err());
            assert!(read_companion(&bytes).is_err());
        }
    }
}
