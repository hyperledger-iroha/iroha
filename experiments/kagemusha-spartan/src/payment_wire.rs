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
//! original fields and all126 ordinary proof inputs are reconstructed exactly;
//! encoding refuses discrepancies instead of normalizing signed originals.

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
pub const PEER_SCHEMA: u16 = 1;
pub const QR_BUDGET: usize = 12_000;
pub const HEADER_BYTES: usize = 84;
const MAX_BYTES: usize = 65_536;
const REQUEST_BYTES: usize = 304;
const RECEIPT_BYTES: usize = 128;
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
#[norito_schema(name = "iroha.research.kagemusha_spartan.payment.CompleteSend.v1")]
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
/// same-profile malformed point/scalar remains available for a derived Burn.
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
fn limbs(bytes: &[u8]) -> [Fp; 2] {
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
    norito_flags(bytes, MAX_BYTES)?;
    let value: Envelope = norito::decode_from_bytes_with_limits(bytes, limits(MAX_BYTES))
        .map_err(|e| format!("Payment Norito: {e}"))?;
    if encode_norito(&value)? != bytes {
        return Err("noncanonical Payment envelope".into());
    }
    if value.version != 1
        || value.statement.len() != 832
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
    if value.version != 1 || value.credential.len() != 596 || value.certificate.len() != 172 {
        return Err("payer companion field version/length".into());
    }
    Ok(value)
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
    )
}

fn compact(o: &Originals, pair: Vec<u8>) -> Result<(Envelope, Companion), String> {
    let mut request = Vec::with_capacity(REQUEST_BYTES);
    for (start, len) in REQUEST_RANGES {
        request.extend_from_slice(&o.request.body[start..start + len]);
    }
    request.extend_from_slice(&o.request.signature);
    let restored = request_restore(&request, &o.payer, &o.statement)?;
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
        version: 1,
        statement: o.statement.iter().flat_map(|f| f.to_repr()).collect(),
        request,
        receipt,
        policy_epoch: o.policy_epoch,
        pending_root: o.pending_root.to_repr().to_vec(),
        credit_root: o.credit_root.to_repr().to_vec(),
        pair,
    };
    let companion = Companion {
        version: 1,
        credential: [o.payer.body.as_slice(), &o.payer.signature].concat(),
        certificate: [o.certificate.body.as_slice(), &o.certificate.signature].concat(),
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
fn peer_encode(envelope: &[u8], companion: &[u8]) -> Result<Vec<u8>, String> {
    if envelope.is_empty()
        || companion.is_empty()
        || companion.len() > COMPANION_LIMIT
        || envelope
            .len()
            .saturating_add(companion.len())
            .saturating_add(100)
            > MAX_BYTES
    {
        return Err("peer payload resource limit".into());
    }
    let mut payload = Vec::new();
    payload.extend_from_slice(PAYMENT_MAGIC);
    payload.extend_from_slice(&(envelope.len() as u32).to_be_bytes());
    payload.extend_from_slice(&(companion.len() as u32).to_be_bytes());
    payload.extend_from_slice(envelope);
    payload.extend_from_slice(companion);
    let mut prefix = Vec::new();
    prefix.extend_from_slice(b"IPM1");
    prefix.extend_from_slice(&[1, 0]);
    prefix.extend_from_slice(&PEER_PROFILE.to_be_bytes());
    prefix.extend_from_slice(&[3, 0]);
    prefix.extend_from_slice(&PEER_SCHEMA.to_be_bytes());
    prefix.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    prefix.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    prefix.extend_from_slice(&hash(&[
        PAYLOAD_DOMAIN,
        &PEER_PROFILE.to_be_bytes(),
        &[3],
        &PEER_SCHEMA.to_be_bytes(),
        &payload,
    ]));
    let wire_hash = hash(&[MESSAGE_DOMAIN, &prefix, &payload]);
    prefix.extend_from_slice(&wire_hash);
    prefix.extend_from_slice(&payload);
    Ok(prefix)
}
fn peer_decode(bytes: &[u8]) -> Result<(&[u8], &[u8]), String> {
    if bytes.len() < HEADER_BYTES + 16 || bytes.len() > MAX_BYTES {
        return Err("peer message length".into());
    }
    if &bytes[..4] != b"IPM1"
        || bytes[4..6] != [1, 0]
        || bytes[6..8] != PEER_PROFILE.to_be_bytes()
        || bytes[8..10] != [3, 0]
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
            &[3],
            &PEER_SCHEMA.to_be_bytes(),
            payload,
        ])
        || bytes[52..84] != hash(&[MESSAGE_DOMAIN, &bytes[..52], payload])
    {
        return Err("peer payload/message hash mismatch".into());
    }
    if &payload[..8] != PAYMENT_MAGIC {
        return Err("Payment companion wrapper magic".into());
    }
    let elen = u32::from_be_bytes(payload[8..12].try_into().unwrap()) as usize;
    let clen = u32::from_be_bytes(payload[12..16].try_into().unwrap()) as usize;
    if elen == 0
        || clen == 0
        || clen > COMPANION_LIMIT
        || elen.checked_add(clen).and_then(|n| n.checked_add(16)) != Some(payload.len())
    {
        return Err("Payment companion wrapper lengths".into());
    }
    Ok((&payload[16..16 + elen], &payload[16 + elen..]))
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
    let (envelope, companion) = compact(originals, pair)?;
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

pub fn decode_candidate(profile: &Profile<'_>, bytes: &[u8]) -> Result<Candidate, String> {
    let (ebytes, cbytes) = peer_decode(bytes)?;
    let envelope = read_envelope(ebytes)?;
    let companion = read_companion(cbytes)?;
    let payer = Credential {
        body: companion.credential[..532].try_into().unwrap(),
        signature: companion.credential[532..].try_into().unwrap(),
    };
    let certificate = Certificate {
        body: companion.certificate[..108].try_into().unwrap(),
        signature: companion.certificate[108..].try_into().unwrap(),
    };
    let statement: [Fp; 26] = envelope
        .statement
        .chunks_exact(32)
        .map(field)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .unwrap();
    let originals = Originals {
        request: request_restore(&envelope.request, &payer, &statement)?,
        receipt: receipt_restore(&envelope.receipt, &payer, &statement)?,
        payer,
        certificate,
        statement,
        policy_epoch: envelope.policy_epoch,
        pending_root: field(&envelope.pending_root)?,
        credit_root: field(&envelope.credit_root)?,
    };
    let projection = projection(profile, &originals)?;
    carrier::frame_projected_pair(profile.pair, &envelope.pair)?;
    let digests = payment::digests(
        &originals.request,
        &originals.payer,
        &originals.statement,
        &originals.receipt,
        &envelope.pair,
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
        pair_bytes: envelope.pair,
        digests,
        size,
    })
}

/// Reframe an exact-profile malformed candidate for native Burn integration tests.
#[cfg(test)]
pub(crate) fn test_replace_pair(profile: &Profile<'_>, payment: &[u8], pair: Vec<u8>) -> Result<Vec<u8>, String> {
    let candidate = decode_candidate(profile, payment)?;
    carrier::frame_projected_pair(profile.pair, &pair)?;
    let (envelope, companion) = compact(&candidate.originals, pair)?;
    peer_encode(&encode_norito(&envelope)?, &encode_norito(&companion)?)
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
        let (envelope, companion) = compact(&h.originals, raw.clone()).unwrap();
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
            "complete Payment codec with actual toy linked proofs: {} B IPM1 =84 header+16 wrapper+{} Norito envelope+{} Norito credential/certificate companion; pair{} B; originals832+304+128+72+596+172; fixed-point monetary proof size still requires integrated backend measurement",
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
        let (e, c) = compact(original, vec![1]).unwrap();
        assert_eq!(
            (
                e.request.len(),
                e.receipt.len(),
                c.credential.len(),
                c.certificate.len()
            ),
            (304, 128, 596, 172)
        );
        let mut unique_request = vec![false; 418];
        for (start, len) in REQUEST_RANGES {
            unique_request[start..start + len].fill(true);
        }
        for i in 0..418 {
            let mut changed = original.clone();
            changed.request.body[i] ^= 1;
            assert_eq!(
                compact(&changed, vec![1]).is_ok(),
                unique_request[i],
                "original Request byte{i}"
            );
        }
        for i in 0..338 {
            let mut changed = original.clone();
            changed.receipt.body[i] ^= 1;
            assert_eq!(
                compact(&changed, vec![1]).is_ok(),
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
        let golden = "49504d3101008001030000010000001c0000001c29b00f652bad4582435f2a39a929ed1e07b07eea09252041e8d903c5884e5b2bb407ab01101f19da6003ece0428784d1da90ea237102b007ec534fd9435600f04b575059504331000000000300000009656e76636f6d70616e696f6e";
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
        assert!(peer_encode(b"x", &vec![1; COMPANION_LIMIT + 1]).is_err());
    }

    #[test]
    fn canonical_norito_and_fixed_original_lengths_are_mandatory() {
        let h = Harness::new();
        let (e, c) = compact(&h.originals, vec![1]).unwrap();
        let eb = encode_norito(&e).unwrap();
        let cb = encode_norito(&c).unwrap();
        assert!(read_envelope(&eb).is_ok());
        assert!(read_companion(&cb).is_ok());
        for which in 0..8 {
            let mut e = e.clone();
            match which {
                0 => e.version = 2,
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
                assert!(result.is_err());
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
