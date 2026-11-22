//! ArchiveSent engineering component for the three wallet-wire evidence digests.
//! The surrounding recursive verifier owns the receiver statement, proof, receipt and
//! exact lineage digests. The component binds their values and the very same opening
//! cells used by membership, including on a false membership branch.
use super::*;
use crate::certificate::field_modulus;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_CREDIT_OPENING_DOMAIN_V1, KAGEMUSHA_WALLET_CREDIT_OPENING_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1, KAGEMUSHA_WALLET_CREDIT_STATUS_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1, KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1,
    KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1, kagemusha_wallet_poseidon_bytes_v1,
};
use num_bigint::BigUint;

/// A fixed transcript. Each byte comes from a constant or an existing constraint cell.
pub(crate) struct Transcript {
    cells: Cells,
    indices: Vec<usize>,
    raw: Vec<u8>,
}

impl Transcript {
    pub(crate) fn new(family: usize) -> Self {
        Self {
            cells: Cells::new(family),
            indices: Vec::new(),
            raw: Vec::new(),
        }
    }

    pub(crate) fn fixed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let k = self.cells.byte(byte);
            self.cells
                .eq(own(k, 1), constant(vec![Fp::from(u64::from(byte))]));
            self.indices.push(k);
            self.raw.push(byte);
        }
    }

    pub(crate) fn integer(&mut self, w: &Wiring<Fp>, src: Source, width: usize) {
        let repr = src.value(w).to_repr();
        let bytes = &repr[..width];
        let indices: Vec<_> = bytes.iter().map(|&byte| self.cells.byte(byte)).collect();
        self.cells.eq(self.cells.bytes(&indices), src.linear());
        if width == 32 {
            // Equality in Fp alone also admits the integer x + p.
            self.cells.at_most(
                &indices,
                &BigUint::from_bytes_le(bytes),
                &(field_modulus() - 1u32),
            );
        }
        self.indices.extend(indices);
        self.raw.extend_from_slice(bytes);
    }

    pub(crate) fn big_endian_limb(&mut self, w: &Wiring<Fp>, src: Source) {
        let at = self.indices.len();
        self.integer(w, src, 16);
        self.indices[at..].reverse();
        self.raw[at..].reverse();
    }

    fn digest(self, w: &mut Wiring<Fp>, family: usize, domain: u64, expected: usize) -> Source {
        self.digest_with_tail(w, family, domain, expected, None)
    }

    /// Append the recursive verifier's exact compressed proof bytes, which already
    /// have a byte range check. A 31-byte chunk may straddle the Ω header and proof.
    pub(crate) fn digest_with_tail(
        mut self,
        w: &mut Wiring<Fp>,
        family: usize,
        domain: u64,
        expected_header: usize,
        tail: Option<&crate::proof_bytes::SharedBytes>,
    ) -> Source {
        assert_eq!(self.raw.len(), expected_header);
        if let Some(tail) = tail {
            for i in 0..tail.len {
                self.raw
                    .push(w.arrays[tail.array].values[0][i].to_repr()[0]);
            }
        }
        let length = Fp::from(self.raw.len() as u64);
        let mut words = vec![(length, constant(vec![length]))];
        for (chunk, raw) in self
            .raw
            .chunks(KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1)
            .enumerate()
        {
            let start = chunk * KAGEMUSHA_WALLET_PACKED_CHUNK_BYTES_V1;
            let mut word = Vec::new();
            for j in 0..raw.len() {
                let i = start + j;
                let byte = if let Some(&k) = self.indices.get(i) {
                    own(k, 1)
                } else {
                    let value = tail.expect("exact proof byte").byte(i - expected_header);
                    Source {
                        array: value.array,
                        cell: value.cell,
                        instance: value.instance,
                    }
                    .linear()
                };
                word = plus(
                    &word,
                    &scaled(&byte, Fp::from(256u64).pow_vartime([j as u64])),
                );
            }
            words.push((certificate::fp(&BigUint::from_bytes_le(raw)), word));
        }
        let hashed = field(&kagemusha_wallet_poseidon_bytes_v1(domain, &self.raw)).unwrap();
        let output = [
            self.cells.cell(Fp::ZERO, 255),
            self.cells.cell(hashed, 255),
            self.cells.cell(Fp::ZERO, 255),
        ];
        let local = w.push(self.cells.a);
        certificate::hash_relation(
            w,
            family,
            local,
            domain,
            &words,
            output,
            &mut SpikeRng::new(b"ArchiveSent wallet wire transcript"),
        );
        source(local, output[1])
    }
}

/// Uses the authenticated statement, sigma proof, receipt and exact lineage digests
/// in wallet-wire order. These four incoming ports require their original verifiers
/// when composing the complete fifth operation.
pub(crate) fn status_chain(
    w: &mut Wiring<Fp>,
    family: usize,
    credit: Source,
    payment: Source,
    receiver: [Source; 4],
    o: &[Source],
) -> [Source; 3] {
    let [_, burned, next, slot, siblings @ ..] = o else {
        unreachable!()
    };
    assert_eq!(siblings.len(), 32);
    let mut t = Transcript::new(family);
    t.integer(w, credit, 32);
    t.integer(w, payment, 32);
    t.integer(w, *burned, 1);
    t.integer(w, *next, 32);
    t.integer(w, *slot, 4);
    for &sibling in siblings {
        t.integer(w, sibling, 32);
    }
    let opening_digest = t.digest(
        w,
        family,
        KAGEMUSHA_WALLET_CREDIT_OPENING_DOMAIN_V1,
        KAGEMUSHA_WALLET_CREDIT_OPENING_TRANSCRIPT_BYTES_V1,
    );
    let mut t = Transcript::new(family);
    t.fixed(&1u16.to_le_bytes());
    for digest in receiver.into_iter().chain([opening_digest]) {
        t.integer(w, digest, 32);
    }
    let status = t.digest(
        w,
        family,
        KAGEMUSHA_WALLET_CREDIT_STATUS_DOMAIN_V1,
        KAGEMUSHA_WALLET_CREDIT_STATUS_TRANSCRIPT_BYTES_V1,
    );
    let mut t = Transcript::new(family);
    t.fixed(&[1, 0, 2]); // LE16(version) followed by the folded Status evidence tag.
    for digest in [credit, payment, status] {
        t.integer(w, digest, 32);
    }
    let credited = t.digest(
        w,
        family,
        KAGEMUSHA_WALLET_CREDITED_DOMAIN_V1,
        KAGEMUSHA_WALLET_CREDITED_TRANSCRIPT_BYTES_V1,
    );
    [opening_digest, status, credited]
}
