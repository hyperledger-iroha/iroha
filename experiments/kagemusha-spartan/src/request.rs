//! Exact completed Request bytes and commitments for research composition.
//!
//! Ports below must be joined to authenticated credentials, the statement and
//! held fee/policy state by the eventual operation. This component checks only
//! the self-contained Request rules; it grants no admission or fee authority.
//! It does not verify ECDSA. Receive owns the receiver signature check, including
//! scalar canonicality/low-S, over `message` and the exported raw limbs; Send
//! must not add a redundant receiver-signature verification.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    certificate::{self, Cells, field_modulus, fp},
    fs::SpikeRng,
    full::Wiring,
    uniform::{Lin, constant, own, plus, scaled},
};

pub const BODY_BYTES: usize = 418;
pub const SIGNATURE_BYTES: usize = 64;
pub const CREDIT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwcrdt1");
pub const TERMS_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrqst2");
pub const OBJECT_DOMAIN: u64 = u64::from_le_bytes(*b"kgworeq1");

/// Word positions match the canonical 24-element credit transcript exactly.
pub mod word {
    pub const SCHEME: [usize; 2] = [1, 2];
    pub const ASSET: [usize; 2] = [3, 4];
    pub const PAYER_WALLET: [usize; 2] = [5, 6];
    pub const PAYER_ACCOUNT: [usize; 2] = [7, 8];
    pub const RECEIVER_WALLET: [usize; 2] = [9, 10];
    pub const RECEIVER_ACCOUNT: [usize; 2] = [11, 12];
    pub const ORDINAL: usize = 13;
    pub const RECEIVER_CREDENTIAL: usize = 14;
    pub const AMOUNT: usize = 15;
    pub const FEE_SCHEDULE: usize = 16;
    pub const FEE: usize = 17;
    pub const POLICY_EPOCH: usize = 18;
    pub const SCHEME_POLICY: usize = 19;
    pub const ACCEPTED_TIME: usize = 20;
    pub const CERTIFICATES: usize = 21;
    pub const NONCE: [usize; 2] = [22, 23];
}

const PAYER_WORDS: [usize; 5] = [5, 6, 7, 8, 13];
// (byte offset, byte width). Identifiers are two LE128 words; fields are
// canonical 32-byte little-endian Fp values, never reduced aliases.
const WORDS: [(usize, usize); 24] = [
    (0, 2),
    (2, 16),
    (18, 16),
    (34, 16),
    (50, 16),
    (66, 16),
    (82, 16),
    (98, 16),
    (114, 16),
    (130, 16),
    (146, 16),
    (162, 16),
    (178, 16),
    (194, 16),
    (210, 32),
    (242, 16),
    (258, 32),
    (290, 16),
    (306, 8),
    (314, 32),
    (346, 8),
    (354, 32),
    (386, 16),
    (402, 16),
];

/// Fixed transcript, not a Norito frame or an authenticated Request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedRequest {
    pub body: [u8; BODY_BYTES],
    pub signature: [u8; SIGNATURE_BYTES],
}

impl CompletedRequest {
    /// Exact tape boundary only. Semantic acceptance is proved by
    /// `append_completed`; external authority joins remain mandatory.
    pub fn from_tape(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != BODY_BYTES + SIGNATURE_BYTES {
            return Err("Request tape must be exactly 482 bytes");
        }
        Ok(Self {
            body: bytes[..BODY_BYTES].try_into().unwrap(),
            signature: bytes[BODY_BYTES..].try_into().unwrap(),
        })
    }
}

/// All ports refer to stable field cells in `local` (not packed byte cells).
/// `words` must bind both account IDs, scheme/asset and wallet IDs to the
/// applicable credentials, and bind the monetary terms to the statement.
/// Receive's quoted credential digest may differ from its renewed current
/// credential: its wallet and payment-key continuity require separate joins.
pub struct RequestCells {
    pub local: usize,
    pub words: [usize; 24],
    /// Unsigned [r_lo, r_hi, s_lo, s_hi], each exactly 128 bits.
    pub signature: [usize; 4],
    pub message: usize,
    pub digest: usize,
    pub credit: usize,
}

fn values(input: &CompletedRequest) -> [Fp; 24] {
    WORDS.map(|(start, len)| {
        let bytes = &input.body[start..start + len];
        if len == 32 {
            // A malformed field gets a total witness; canonicality and byte
            // equality below reject it in the relation, not just on the host.
            Option::<Fp>::from(Fp::from_repr(bytes.try_into().unwrap())).unwrap_or(Fp::ZERO)
        } else {
            fp(&BigUint::from_bytes_le(bytes))
        }
    })
}

fn nonzero(c: &mut Cells, expression: Lin<Fp>, value: Fp) {
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows
        .push([expression, own(inverse, 1), constant(vec![Fp::ONE])]);
}

fn is_zero(c: &mut Cells, expression: Lin<Fp>, value: Fp) -> usize {
    let bit = c.bit(value == Fp::ZERO);
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([expression.clone(), own(bit, 1), vec![]]);
    c.a.rows.push([
        expression,
        own(inverse, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(bit, 1), -Fp::ONE)),
    ]);
    bit
}

/// Append the exact hard-valid completed-Request predicate from
/// operation_relation/objects/request.rs. The caller supplies the common byte
/// range table (`nonnative::RANGE_TABLE`) when finishing its composed Wiring.
pub fn append_completed(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &CompletedRequest,
) -> RequestCells {
    let mut c = Cells::new(family);
    c.a.name = "completed Request bytes and words";
    for byte in input.body.iter().chain(&input.signature) {
        c.byte(*byte);
    }
    let native = values(input);
    let words: [usize; 24] = std::array::from_fn(|i| {
        let (start, len) = WORDS[i];
        let indices: Vec<_> = (start..start + len).collect();
        if len == 32 {
            c.at_most(
                &indices,
                &BigUint::from_bytes_le(&input.body[start..start + len]),
                &(field_modulus() - 1u32),
            );
        }
        let cell = c.cell(native[i], if len == 32 { 255 } else { (8 * len) as u16 });
        c.eq(own(cell, 1), c.bytes(&indices));
        cell
    });
    c.eq(own(words[0], 1), constant(vec![Fp::ONE]));
    // Each identifier is an exact pair of bounded positive integers; their
    // sum is below 2^129, so zero iff both halves are zero.
    for [lo, hi] in [
        word::SCHEME,
        word::ASSET,
        word::PAYER_WALLET,
        word::PAYER_ACCOUNT,
        word::RECEIVER_WALLET,
        word::RECEIVER_ACCOUNT,
        word::NONCE,
    ] {
        nonzero(
            &mut c,
            plus(&own(words[lo], 1), &own(words[hi], 1)),
            native[lo] + native[hi],
        );
    }
    for i in [word::RECEIVER_CREDENTIAL, word::AMOUNT, word::CERTIFICATES] {
        nonzero(&mut c, own(words[i], 1), native[i]);
    }
    let equal: [usize; 2] = std::array::from_fn(|i| {
        let (p, r) = (word::PAYER_WALLET[i], word::RECEIVER_WALLET[i]);
        is_zero(
            &mut c,
            plus(&own(words[p], 1), &scaled(&own(words[r], 1), -Fp::ONE)),
            native[p] - native[r],
        )
    });
    c.a.rows.push([own(equal[0], 1), own(equal[1], 1), vec![]]);
    let epoch_zero = is_zero(
        &mut c,
        own(words[word::POLICY_EPOCH], 1),
        native[word::POLICY_EPOCH],
    );
    let policy_zero = is_zero(
        &mut c,
        own(words[word::SCHEME_POLICY], 1),
        native[word::SCHEME_POLICY],
    );
    c.eq(own(epoch_zero, 1), own(policy_zero, 1));
    let schedule_zero = is_zero(
        &mut c,
        own(words[word::FEE_SCHEDULE], 1),
        native[word::FEE_SCHEDULE],
    );
    c.a.rows
        .push([own(schedule_zero, 1), own(words[word::FEE], 1), vec![]]);
    // Integer addition, with every sum byte and carry range constrained.
    // Field addition alone would not establish the u128 overflow boundary.
    let mut carry = 0u16;
    let mut previous = None;
    for i in 0..16 {
        let sum = input.body[242 + i] as u16 + input.body[290 + i] as u16 + carry;
        let byte = c.byte(sum as u8);
        let next = c.bit(sum > 255);
        let mut expression = plus(&own(242 + i, 1), &own(290 + i, 1));
        if let Some(k) = previous {
            expression = plus(&expression, &own(k, 1));
        }
        expression = plus(&expression, &scaled(&own(next, 1), -Fp::from(256)));
        c.eq(expression, own(byte, 1));
        previous = Some(next);
        carry = sum >> 8;
    }
    c.zero(own(previous.unwrap(), 1));
    let signature: [usize; 4] = std::array::from_fn(|i| {
        let start = BODY_BYTES
            + match i {
                0 => 16,
                1 => 0,
                2 => 48,
                _ => 32,
            };
        let indices: Vec<_> = (start..start + 16).rev().collect();
        let value = fp(&BigUint::from_bytes_be(
            &input.signature[start - BODY_BYTES..start - BODY_BYTES + 16],
        ));
        let cell = c.cell(value, 128);
        c.eq(own(cell, 1), c.bytes(&indices));
        cell
    });
    let terms: Vec<_> = (0..24)
        .filter(|i| !PAYER_WORDS.contains(i))
        .map(|i| (native[i], own(words[i], 1)))
        .collect();
    assert_eq!(terms.len(), 19);
    let message =
        poseidon::hash_with_domain(TERMS_DOMAIN, &terms.iter().map(|x| x.0).collect::<Vec<_>>());
    let message_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(message, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let object: Vec<_> = std::iter::once((message, own(message_state[1], 1)))
        .chain(signature.iter().map(|k| (c.a.values[*k][0], own(*k, 1))))
        .collect();
    let digest = poseidon::hash_with_domain(
        OBJECT_DOMAIN,
        &object.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let digest_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(digest, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let credit = poseidon::hash_with_domain(CREDIT_DOMAIN, &native);
    let credit_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(credit, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let credit_inputs: Vec<_> = (0..24).map(|i| (native[i], own(words[i], 1))).collect();
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"completed Request exact native sponge");
    for (domain, inputs, outputs) in [
        (TERMS_DOMAIN, &terms, message_state),
        (OBJECT_DOMAIN, &object, digest_state),
        (CREDIT_DOMAIN, &credit_inputs, credit_state),
    ] {
        certificate::hash_relation(w, family, local, domain, inputs, outputs, &mut rng);
    }
    RequestCells {
        local,
        words,
        signature,
        message: message_state[1],
        digest: digest_state[1],
        credit: credit_state[1],
    }
}

// Copied without transformation from fixtures/kagemusha/wallet_v1_vectors.json:
// poseidon.credit_id.request_body_hex and signatures[object=Request].
#[cfg(test)]
pub(crate) fn reference_request() -> CompletedRequest {
    fn decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    CompletedRequest {
        body: decode("01009692c8c83bef03c3991f61516a22cdb560ba5a980bfd4a9c3f27339c9d2dd4456e85849d458ab1a848d48e030c816121e9b08998cc5b2aff7481f2b93387510154b2461056dc535bf12f3fe2676c94b51077cc1c37770f454250395c678e5f54ff76ed91c5372035df12dc84c0d19010608d49ea63bd13635ef1a8282f92aa8ce2fb204ebf7bf3c3799e0666ea448aeb19cdfa386e080756ac5a993fff06c6f4db148ae54804f2a21d86940ff13a1f61942449b5e4e949a1ccca233854f793f40400000000000000000000000000000076f75a2b1213e64957ddceccde0b2b4159968bee3d4172b3c177c82dc1cfcf01e803000000000000000000000000000070eeae3bc864578883ff559ab9b67792e1aaa00a43bd0fb9a570a83790632527060000000000000000000000000000000100000000000000444e9f038b5d185c657f3280b13ac090de48e606059e1e211a64dc35e0b32900a0f251c4a00100009ca88b95110ff48045ec26c60dde98dec5ed2b8f099875771bff55798db5a53c6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e6e").try_into().unwrap(),
        signature: decode("2816474e60f4de46a130b497e9889200853697a03ab038d98ec450cd7c53800a292db6391e3f394bf4e68e94ca7130fe8977cef643ef1a5d911f21897b5b3aca").try_into().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        nonnative,
        r1cs::{Circuit, Instance, Witness},
        uniform::{Array, UniformCircuit, link},
    };

    type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    use super::reference_request as fixture;

    fn build(input: &CompletedRequest) -> Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let c = append_completed(&mut w, family, input);
        let range = w.push(Array {
            name: "Request byte range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|x| Fp::from(x as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let outputs: Vec<_> = c
            .words
            .into_iter()
            .chain(c.signature)
            .chain([c.message, c.digest, c.credit])
            .collect();
        let mut output_values: Vec<_> = outputs
            .iter()
            .map(|k| w.arrays[c.local].values[*k][0])
            .collect();
        output_values.resize(32, Fp::ZERO);
        let io = w.push(Array {
            name: "Request authenticated join ports",
            family,
            rho: 5,
            r_count: outputs.len(),
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![output_values],
        });
        w.io_arrays.push((io, 32));
        for (i, k) in outputs.into_iter().enumerate() {
            w.arrays[c.local].rows.push([
                plus(
                    &own(k, 1),
                    &scaled(&link(io, 0, vec![Some(i)], Fp::ONE), -Fp::ONE),
                ),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        let (arrays, pi, counts, tables, copies) = w.finish();
        UniformCircuit::build_with_copies(
            "completed Request",
            arrays,
            pi,
            counts,
            tables,
            copies,
            12,
            64,
            b"completed Request",
        )
    }

    fn holds((c, i, witness): &Built) -> bool {
        let mut values = witness.w.clone();
        let relation = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = relation.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }

    #[test]
    fn exact_native_golden_hashes_and_every_join_port() {
        let mut b = build(&fixture());
        assert!(holds(&b));
        for (offset, expected) in [
            (
                28,
                "cc9d8d9304ad330e1047a839321036e749967a85c1a1d49944fc32f8510c1f38",
            ),
            (
                29,
                "e5464b4cf8d9138c4a067296890c8d90da116a4b3e35394edb61e8d570cdd538",
            ),
            (
                30,
                "e37e9a5312d6fa9c64a62f9ef7fe1477be533854d2146a2dc13b02d7c4a5563f",
            ),
        ] {
            assert_eq!(b.1.io[32 + offset].to_repr().as_ref(), hex(expected));
        }
        for offset in 0..31 {
            b.1.io[32 + offset] += Fp::ONE;
            assert!(!holds(&b), "unbound Request port {offset}");
            b.1.io[32 + offset] -= Fp::ONE;
        }
    }

    #[test]
    fn payer_completion_preserves_terms_but_changes_credit() {
        let honest = fixture();
        let original = build(&honest);
        for offset in [66, 82, 98, 114, 194] {
            let mut different = honest.clone();
            different.body[offset] ^= 1;
            let changed = build(&different);
            assert!(holds(&changed));
            assert_eq!(&original.1.io[60..62], &changed.1.io[60..62]);
            assert_ne!(original.1.io[62], changed.1.io[62]);
        }
        let mut open = honest;
        open.body[66..130].fill(0);
        open.body[194..210].fill(0);
        let changed = build(&open);
        assert!(!holds(&changed), "open requests are not completed requests");
        assert_eq!(&original.1.io[60..62], &changed.1.io[60..62]);
    }

    #[test]
    fn identity_and_semantic_forgeries_fail_in_constraints() {
        let honest = fixture();
        for (start, len) in [
            (0, 2),
            (2, 32),
            (34, 32),
            (66, 32),
            (98, 32),
            (130, 32),
            (162, 32),
            (210, 32),
            (242, 16),
            (354, 32),
            (386, 32),
        ] {
            let mut bad = honest.clone();
            bad.body[start..start + len].fill(0);
            assert!(!holds(&build(&bad)), "missing field at {start}");
        }
        for start in [0, 1] {
            let mut bad = honest.clone();
            bad.body[start] = 2;
            assert!(!holds(&build(&bad)), "version byte {start}");
        }
        let mut same = honest.clone();
        same.body[130..162].copy_from_slice(&honest.body[66..98]);
        assert!(!holds(&build(&same)));
        for (start, len) in [(258, 32), (306, 8), (314, 32)] {
            let mut bad = honest.clone();
            bad.body[start..start + len].fill(0);
            assert!(!holds(&build(&bad)), "inconsistent pair at {start}");
        }
        let mut free = honest;
        for (start, len) in [(258, 32), (290, 16), (306, 8), (314, 32)] {
            free.body[start..start + len].fill(0);
        }
        assert!(holds(&build(&free)));
    }

    #[test]
    fn amount_fee_integer_boundary_and_canonical_fields() {
        let honest = fixture();
        for (amount, fee, accepted) in [
            (u128::MAX, 0, true),
            (u128::MAX - 1, 1, true),
            (u128::MAX, 1, false),
            (u128::MAX, u128::MAX, false),
        ] {
            let mut input = honest.clone();
            input.body[242..258].copy_from_slice(&amount.to_le_bytes());
            input.body[290..306].copy_from_slice(&fee.to_le_bytes());
            assert_eq!(
                holds(&build(&input)),
                accepted,
                "amount {amount}, fee {fee}"
            );
        }
        for start in [210, 258, 314, 354] {
            for value in [field_modulus(), field_modulus() + 1u32] {
                let mut input = honest.clone();
                let mut bytes = value.to_bytes_le();
                bytes.resize(32, 0);
                input.body[start..start + 32].copy_from_slice(&bytes);
                assert!(!holds(&build(&input)), "noncanonical field at {start}");
            }
        }
    }

    #[test]
    fn exact_tape_boundaries_and_raw_signature_ownership() {
        let honest = fixture();
        let tape: Vec<_> = honest
            .body
            .iter()
            .chain(&honest.signature)
            .copied()
            .collect();
        assert_eq!(CompletedRequest::from_tape(&tape).unwrap(), honest);
        for n in 0..tape.len() {
            assert!(CompletedRequest::from_tape(&tape[..n]).is_err());
        }
        let mut trailing = tape;
        trailing.push(0);
        assert!(CompletedRequest::from_tape(&trailing).is_err());
        let original = build(&honest);
        for offset in [0, 16, 32, 48, 63] {
            let mut changed = honest.clone();
            changed.signature[offset] ^= 1;
            let b = build(&changed);
            // This component proves byte binding; Receive separately verifies
            // the raw signature. Never mistake this acceptance for ECDSA.
            assert!(holds(&b));
            assert_eq!(b.1.io[60], original.1.io[60]);
            assert_eq!(b.1.io[62], original.1.io[62]);
            assert_ne!(b.1.io[61], original.1.io[61]);
        }
    }
}
