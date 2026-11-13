//! Complete Payment commitment binding for the future proof backend.
//!
//! The raw completed Send pair has its own length-delimited proof digest. The
//! signed preparation receipt remains part of the package; it cannot name the
//! recursive proof that includes that receipt. Every byte of the completed pair
//! is hashed, including final IPA responses. No transcript-shortcut assumption
//! is used. Structural transport decoding is separate from native admission.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    bootstrap::Receipt,
    certificate::{self, Cells},
    credential::{self, Credential},
    fs::SpikeRng,
    full::{IncomingProofFacts, Wiring},
    operation_auth::CredentialOpening,
    receipt::ReceiptOpening,
    receive_originals::IncomingStatement,
    request::{self, CompletedRequest, RequestCells},
    send_arithmetic::Source,
    statement,
    uniform::{Lin, own},
};

pub const PROOF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwprf_1");
pub const PACKAGE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwpkg_1");
pub const PAYMENT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwpay_1");
pub const TRANSCRIPT_BYTES: usize = 163;

pub fn byte_digest(domain: u64, bytes: &[u8]) -> Fp {
    let values: Vec<_> = std::iter::once(Fp::from(bytes.len() as u64))
        .chain(
            bytes
                .chunks(31)
                .map(|b| certificate::fp(&BigUint::from_bytes_le(b))),
        )
        .collect();
    poseidon::hash_with_domain(domain, &values)
}

/// Exact pair bytes, with an unambiguous length. No proof-acceptance authority.
pub fn proof_digest(pair: &[u8]) -> Result<Fp, &'static str> {
    let len = u32::try_from(pair.len()).map_err(|_| "pair length exceeds u32")?;
    let mut bytes = Vec::with_capacity(pair.len() + 4);
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(pair);
    Ok(byte_digest(PROOF_DOMAIN, &bytes))
}

pub struct Digests {
    pub proof: Fp,
    pub package: Fp,
    pub payment: Fp,
}

/// Native commitment reconstruction; callers must still authenticate all
/// originals, the parameter set, the linked proofs and both accumulated claims.
pub fn digests(
    request: &CompletedRequest,
    payer: &Credential,
    send_statement: &[Fp; 26],
    receipt: &Receipt,
    pair: &[u8],
) -> Result<Digests, &'static str> {
    let request = request::commitments(request)?;
    let proof = proof_digest(pair)?;
    let package = poseidon::hash_with_domain(
        PACKAGE_DOMAIN,
        &[
            statement::digest(send_statement),
            proof,
            crate::receipt::digest(receipt),
        ],
    );
    let mut transcript = Vec::with_capacity(TRANSCRIPT_BYTES);
    transcript.extend_from_slice(&1u16.to_le_bytes());
    transcript.extend_from_slice(&request.digest.to_repr());
    transcript.extend_from_slice(&payer.body[130..195]);
    transcript.extend_from_slice(&credential::digest(payer).to_repr());
    transcript.extend_from_slice(&package.to_repr());
    assert_eq!(transcript.len(), TRANSCRIPT_BYTES);
    Ok(Digests {
        proof,
        package,
        payment: byte_digest(PAYMENT_DOMAIN, &transcript),
    })
}

/// Complete derived Payment digest consumed by the own Receive receipt and the
/// permanent credit record. Its private fields forbid substituting a claimed
/// digest for the exact composition of constrained original/proof sources.
pub struct BoundPayment {
    package: Source,
    payment: Source,
}
impl BoundPayment {
    pub fn digest(&self) -> Source {
        self.payment
    }
    pub fn package_digest(&self) -> Source {
        self.package
    }
}

fn port(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

fn encoded_field(c: &mut Cells, w: &Wiring<Fp>, source: Source) -> Vec<usize> {
    let raw = source.value(w).to_repr();
    let indices: Vec<_> = raw.iter().map(|b| c.byte(*b)).collect();
    c.at_most(
        &indices,
        &BigUint::from_bytes_le(&raw),
        &(certificate::field_modulus() - 1u32),
    );
    c.eq(c.bytes(&indices), source.linear());
    indices
}

/// Join exact original-object hashes to the total proof owner's digest of the
/// complete raw pair. Object/proof validity is derived independently and may
/// be false; that does not erase or retarget the Payment's byte identity.
pub fn append_digest(
    w: &mut Wiring<Fp>,
    family: usize,
    request: &RequestCells,
    payer: &CredentialOpening,
    statement: &IncomingStatement,
    receipt: &ReceiptOpening,
    proof: &IncomingProofFacts,
) -> BoundPayment {
    let sources = [statement.digest(), proof.proof_digest(), receipt.digest()];
    for source in sources
        .into_iter()
        .chain([port(request.local, request.digest), payer.digest()])
    {
        assert_eq!(
            w.arrays[source.array].family, family,
            "same global Payment sources"
        );
    }
    let package_value = poseidon::hash_with_domain(PACKAGE_DOMAIN, &sources.map(|s| s.value(w)));
    let mut c = Cells::new(family);
    c.a.name = "complete Payment byte transcript";
    let package_output = [
        c.cell(Fp::ZERO, 255),
        c.cell(package_value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let package_bytes = package_value.to_repr();
    let package_indices: Vec<_> = package_bytes.iter().map(|b| c.byte(*b)).collect();
    c.at_most(
        &package_indices,
        &BigUint::from_bytes_le(&package_bytes),
        &(certificate::field_modulus() - 1u32),
    );
    c.eq(c.bytes(&package_indices), own(package_output[1], 1));
    let mut transcript = vec![c.byte(1), c.byte(0)];
    // Constants are equalities, not values chosen only by witness generation.
    c.eq(
        own(transcript[0], 1),
        crate::uniform::constant(vec![Fp::ONE]),
    );
    c.zero(own(transcript[1], 1));
    transcript.extend(encoded_field(
        &mut c,
        w,
        port(request.local, request.digest),
    ));
    for i in 130..195 {
        let source = payer.byte(i);
        let byte = c.byte(source.value(w).to_repr()[0]);
        c.eq(own(byte, 1), source.linear());
        transcript.push(byte);
    }
    transcript.extend(encoded_field(&mut c, w, payer.digest()));
    transcript.extend(package_indices);
    assert_eq!(transcript.len(), TRANSCRIPT_BYTES);
    let bytes: Vec<_> = transcript
        .iter()
        .map(|i| c.a.values[*i][0].to_repr()[0])
        .collect();
    let payment_value = byte_digest(PAYMENT_DOMAIN, &bytes);
    let payment_output = [
        c.cell(Fp::ZERO, 255),
        c.cell(payment_value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let mut payment_inputs: Vec<(Fp, Lin<Fp>)> = vec![(
        Fp::from(TRANSCRIPT_BYTES as u64),
        crate::uniform::constant(vec![Fp::from(TRANSCRIPT_BYTES as u64)]),
    )];
    for chunk in transcript.chunks(31) {
        let raw: Vec<_> = chunk
            .iter()
            .map(|i| c.a.values[*i][0].to_repr()[0])
            .collect();
        payment_inputs.push((
            certificate::fp(&BigUint::from_bytes_le(&raw)),
            c.bytes(chunk),
        ));
    }
    let package_inputs: Vec<_> = sources
        .into_iter()
        .map(|s| (s.value(w), s.linear()))
        .collect();
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"exact full Payment commitments");
    certificate::hash_relation(
        w,
        family,
        local,
        PACKAGE_DOMAIN,
        &package_inputs,
        package_output,
        &mut rng,
    );
    certificate::hash_relation(
        w,
        family,
        local,
        PAYMENT_DOMAIN,
        &payment_inputs,
        payment_output,
        &mut rng,
    );
    BoundPayment {
        package: port(local, package_output[1]),
        payment: port(local, payment_output[1]),
    }
}
