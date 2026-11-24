//! Total credit-digest membership for folded ArchiveSent evidence.
//! A malformed route can yield a false verdict for an exact original.
use super::*;
use crate::{
    credential_policy, indexed_map,
    native::{Error, Result, VerifiedCreditStatus, VerifiedSentDelivery},
    payment_wire, receive_lineage,
};

#[derive(Clone)]
pub(in crate::native::engine) struct StatusOpening {
    pub(in crate::native::engine) root: Fp,
    pub(in crate::native::engine) burned: bool,
    pub(in crate::native::engine) next: Fp,
    pub(in crate::native::engine) index: u32,
    pub(in crate::native::engine) siblings: [Fp; 32],
}

impl StatusOpening {
    // This fixture reads only an already authenticated representation. The two program
    // inputs are the opening carried by that exact original and the receiver's folded root.
    #[cfg(test)]
    pub(in crate::native::engine) fn from_verified(
        delivery: &VerifiedSentDelivery<'_>,
        root: Fp,
    ) -> Result<Self> {
        fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
            let (head, tail) = bytes
                .split_at_checked(n)
                .ok_or(Error::Malformed("archive status opening"))?;
            *bytes = tail;
            Ok(head)
        }
        fn compact(bytes: &mut &[u8]) -> Result<Fp> {
            let n = usize::from(take(bytes, 1)?[0]);
            if n > 32 {
                return Err(Error::Malformed("archive status opening field width"));
            }
            let raw = take(bytes, n)?;
            if raw.last() == Some(&0) {
                return Err(Error::Malformed("archive status opening field encoding"));
            }
            let mut b = [0; 32];
            b[..n].copy_from_slice(raw);
            field(&b)
        }
        let mut b = payment_wire::decode_peer_payload(
            4,
            delivery.credit_status(),
            10_084 - payment_wire::HEADER_BYTES,
        )
        .map_err(Error::Rejected)?;
        if take(&mut b, 8)? != b"KGSCST3\0" {
            return Err(Error::Malformed("archive status original"));
        }
        let burned = matches!(delivery.outcome(), VerifiedCreditStatus::Burned);
        if take(&mut b, 1)?[0] != u8::from(burned) {
            return Err(Error::AuthorityMismatch("archive status outcome"));
        }
        for _ in 0..9 {
            compact(&mut b)?;
        }
        take(&mut b, 160)?;
        for _ in 0..5 {
            compact(&mut b)?;
        }
        let next = compact(&mut b)?;
        let index = u32::from_le_bytes(take(&mut b, 4)?.try_into().unwrap());
        let mask = u32::from_le_bytes(take(&mut b, 4)?.try_into().unwrap());
        let mut empty = Fp::ZERO;
        let mut siblings = [Fp::ZERO; 32];
        for (i, sibling) in siblings.iter_mut().enumerate() {
            *sibling = if mask & (1u32 << i) != 0 {
                let value = field(take(&mut b, 32)?.try_into().unwrap())?;
                if value == empty {
                    return Err(Error::Malformed("archive status redundant sibling"));
                }
                value
            } else {
                empty
            };
            empty = poseidon::hash_with_domain(indexed_map::NODE_DOMAIN, &[empty, empty]);
        }
        Ok(Self {
            root,
            burned,
            next,
            index,
            siblings,
        })
    }

    #[cfg(test)]
    pub(in crate::native::engine) fn words(&self) -> Vec<Fp> {
        [
            self.root,
            Fp::from(u64::from(self.burned)),
            self.next,
            Fp::from(self.index as u64),
        ]
        .into_iter()
        .chain(self.siblings)
        .collect()
    }

    // Authenticate a raw opening with the same leaf and node domains as the compact
    // verifier. Root agreement is a total predicate, never a hard success shortcut.
    pub(in crate::native::engine) fn append(
        &self,
        w: &mut Wiring<Fp>,
        family: usize,
        credit: Source,
        payment: Source,
        evidence: Source,
    ) -> (Source, Vec<Source>) {
        let mut c = Cells::new(family);
        c.a.name = "ArchiveSent credit status opening";
        let root = c.cell(self.root, 255);
        let flag = c.bit(self.burned);
        let next = c.cell(self.next, 255);
        let index = c.cell(Fp::from(self.index as u64), 32);
        let bits: Vec<_> = (0..32)
            .map(|i| c.bit(self.index & (1u32 << i) != 0))
            .collect();
        let packed = bits.iter().enumerate().fold(vec![], |a, (i, &bit)| {
            plus(&a, &scaled(&own(bit, 1), Fp::from(1u64 << i)))
        });
        c.eq(own(index, 1), packed);
        let siblings = self.siblings.map(|s| c.cell(s, 255));
        let key = c.cell(credit.value(w), 255);
        c.eq(own(key, 1), credit.linear());
        let digest = c.cell(payment.value(w), 255);
        c.eq(own(digest, 1), payment.linear());
        let mut hashes = vec![];
        let mut hash = |c: &mut Cells, domain: u64, inputs: &[usize]| {
            let terms: Vec<_> = inputs
                .iter()
                .map(|&i| (c.a.values[i][0], own(i, 1)))
                .collect();
            let vals: Vec<_> = terms.iter().map(|(v, _)| *v).collect();
            let result = poseidon::hash_with_domain(domain, &vals);
            let state = [
                c.cell(Fp::ZERO, 255),
                c.cell(result, 255),
                c.cell(Fp::ZERO, 255),
            ];
            hashes.push((domain, terms, state));
            state[1]
        };
        let value = hash(&mut c, receive_lineage::CREDIT_DOMAIN, &[key, digest, flag]);
        let mut node = hash(&mut c, indexed_map::LEAF_DOMAIN, &[key, value, next]);
        for (&bit, &sibling) in bits.iter().zip(&siblings) {
            let a = c.a.values[node][0];
            let b = c.a.values[sibling][0];
            let change = c.a.values[bit][0] * (b - a);
            let swap = c.cell(change, 255);
            c.a.rows.push([
                own(bit, 1),
                plus(&own(sibling, 1), &scaled(&own(node, 1), -Fp::ONE)),
                own(swap, 1),
            ]);
            let left = c.cell(a + change, 255);
            let right = c.cell(b - change, 255);
            c.eq(own(left, 1), plus(&own(node, 1), &own(swap, 1)));
            c.eq(
                own(right, 1),
                plus(&own(sibling, 1), &scaled(&own(swap, 1), -Fp::ONE)),
            );
            node = hash(&mut c, indexed_map::NODE_DOMAIN, &[left, right]);
        }
        let d = c.a.values[node][0] - self.root;
        let agrees = credential_policy::zero_predicate(
            &mut c,
            plus(&own(node, 1), &scaled(&own(root, 1), -Fp::ONE)),
            d,
        );
        let ok = c.bit(evidence.value(w) == Fp::ONE && c.a.values[agrees][0] == Fp::ONE);
        c.a.rows
            .push([own(agrees, 1), evidence.linear(), own(ok, 1)]);
        let local = w.push(c.a);
        let mut rng = SpikeRng::new(b"ArchiveSent credit path");
        for (domain, terms, state) in hashes {
            certificate::hash_relation(w, family, local, domain, &terms, state, &mut rng);
        }
        let ports = [root, flag, next, index]
            .into_iter()
            .chain(siblings)
            .map(|i| source(local, i))
            .collect();
        (source(local, ok), ports)
    }
}
