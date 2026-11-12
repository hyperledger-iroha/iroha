//! Exact indexed-map insertion for future monetary research relations.
//!
//! This component authenticates a unique bracketing leaf and an empty destination
//! against the intermediate root. Callers must bind its returned cells to their
//! authenticated state and operation; this component alone authorizes no payment.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    certificate::{self, Cells, field_modulus},
    fs::SpikeRng,
    full::Wiring,
    uniform::{Lin, constant, own, plus, scaled},
};

pub const LEAF_DOMAIN: u64 = u64::from_le_bytes(*b"kgwimlf1");
pub const NODE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwimnd1");

/// A canonical indexed leaf, including the sentinel `(0, 0, next)`.
#[derive(Clone, Copy, Debug)]
pub struct Leaf {
    pub key: Fp,
    pub value: Fp,
    pub next: Fp,
}

/// A binary opening; all index bits are constrained, including bit 31.
#[derive(Clone, Debug)]
pub struct Path<const D: usize> {
    pub index: u32,
    pub siblings: [Fp; D],
}

/// Exact private inputs for one insertion. `slot` opens zero after relinking low.
#[derive(Clone, Debug)]
pub struct InsertWitness<const D: usize> {
    pub old_root: Fp,
    pub key: Fp,
    pub value: Fp,
    pub low: Leaf,
    pub low_path: Path<D>,
    pub slot: Path<D>,
}

/// Cells that the caller must join to authenticated operation/state sources.
pub struct InsertCells {
    pub local: usize,
    pub old_root: usize,
    pub key: usize,
    pub value: usize,
    pub new_root: usize,
}

/// A claimed search route: either the queried leaf or its unique predecessor.
#[derive(Clone, Debug)]
pub struct SearchWitness<const D: usize> {
    pub root: Fp,
    pub key: Fp,
    pub leaf: Leaf,
    pub path: Path<D>,
}

/// Mandatory context joins and a total, constrained membership result.
/// `value` is the opened leaf value; consumers may use it as the query's value
/// only when `present` is one. A bad route or path cannot return absence.
pub struct SearchCells {
    pub local: usize,
    pub root: usize,
    pub key: usize,
    pub value: usize,
    pub present: usize,
}

pub fn leaf_hash(leaf: Leaf) -> Fp {
    poseidon::hash_with_domain(LEAF_DOMAIN, &[leaf.key, leaf.value, leaf.next])
}

pub fn path_root<const D: usize>(mut node: Fp, path: &Path<D>) -> Fp {
    assert!((1..=32).contains(&D));
    for (height, sibling) in path.siblings.iter().enumerate() {
        let pair = if (path.index >> height) & 1 == 0 {
            [node, *sibling]
        } else {
            [*sibling, node]
        };
        node = poseidon::hash_with_domain(NODE_DOMAIN, &pair);
    }
    node
}

struct Hash {
    domain: u64,
    inputs: Vec<(Fp, Lin<Fp>)>,
    outputs: [usize; 3],
}

struct Builder {
    c: Cells,
    hashes: Vec<Hash>,
}

impl Builder {
    fn value(&self, k: usize) -> Fp {
        self.c.a.values[k][0]
    }

    fn nonzero(&mut self, k: usize) {
        let inv = self.c.cell(
            Option::<Fp>::from(self.value(k).invert()).unwrap_or(Fp::ZERO),
            255,
        );
        self.c
            .a
            .rows
            .push([own(k, 1), own(inv, 1), constant(vec![Fp::ONE])]);
    }

    fn is_zero(&mut self, k: usize) -> usize {
        let z = self.c.bit(self.value(k) == Fp::ZERO);
        let inv = self.c.cell(
            Option::<Fp>::from(self.value(k).invert()).unwrap_or(Fp::ZERO),
            255,
        );
        self.c.a.rows.push([own(k, 1), own(z, 1), vec![]]);
        self.c.a.rows.push([
            own(k, 1),
            own(inv, 1),
            plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
        ]);
        z
    }

    fn canonical(&mut self, k: usize) -> Vec<usize> {
        let bytes = self.value(k).to_repr();
        let indices: Vec<_> = bytes.iter().map(|b| self.c.byte(*b)).collect();
        self.c.eq(own(k, 1), self.c.bytes(&indices));
        self.c.at_most(
            &indices,
            &BigUint::from_bytes_le(&bytes),
            &(field_modulus() - 1u32),
        );
        indices
    }

    /// Integer subtraction with byte-sized differences and boolean borrows.
    /// The final borrow is exactly `x < y`; there is no field-difference shortcut.
    fn less(&mut self, x: &[usize], y: &[usize]) -> usize {
        assert_eq!(x.len(), 32);
        assert_eq!(y.len(), 32);
        let mut prior = None;
        let mut borrow = 0i16;
        for (&x, &y) in x.iter().zip(y) {
            let xi = self.value(x).to_repr()[0] as i16;
            let yi = self.value(y).to_repr()[0] as i16;
            let d = xi - yi - borrow;
            let next = d < 0;
            let difference = self.c.byte(d.rem_euclid(256) as u8);
            let bit = self.c.bit(next);
            let mut equation = plus(&own(x, 1), &scaled(&own(y, 1), -Fp::ONE));
            if let Some(k) = prior {
                equation = plus(&equation, &scaled(&own(k, 1), -Fp::ONE));
            }
            equation = plus(&equation, &scaled(&own(bit, 1), Fp::from(256)));
            self.c.eq(equation, own(difference, 1));
            prior = Some(bit);
            borrow = i16::from(next);
        }
        prior.expect("32 bytes")
    }

    fn hash(&mut self, domain: u64, indices: &[usize]) -> usize {
        let values: Vec<_> = indices.iter().map(|k| self.value(*k)).collect();
        let digest = poseidon::hash_with_domain(domain, &values);
        let outputs = [
            self.c.cell(Fp::ZERO, 255),
            self.c.cell(digest, 255),
            self.c.cell(Fp::ZERO, 255),
        ];
        self.hashes.push(Hash {
            domain,
            inputs: indices
                .iter()
                .zip(values)
                .map(|(k, value)| (value, own(*k, 1)))
                .collect(),
            outputs,
        });
        outputs[1]
    }

    fn path<const D: usize>(&mut self, path: &Path<D>) -> (usize, Vec<usize>, Vec<usize>) {
        assert!((1..=32).contains(&D));
        let index = self.c.cell(Fp::from(path.index as u64), 32);
        let bits: Vec<_> = (0..D)
            .map(|i| self.c.bit((path.index >> i) & 1 != 0))
            .collect();
        let composed = bits.iter().enumerate().fold(vec![], |a, (i, k)| {
            plus(&a, &scaled(&own(*k, 1), Fp::from(1u64 << i)))
        });
        self.c.eq(own(index, 1), composed);
        let siblings = path.siblings.iter().map(|v| self.c.cell(*v, 255)).collect();
        (index, bits, siblings)
    }

    fn root(&mut self, mut node: usize, bits: &[usize], siblings: &[usize]) -> usize {
        for (&bit, &sibling) in bits.iter().zip(siblings) {
            let swap = self.value(bit) * (self.value(sibling) - self.value(node));
            let swap_cell = self.c.cell(swap, 255);
            self.c.a.rows.push([
                own(bit, 1),
                plus(&own(sibling, 1), &scaled(&own(node, 1), -Fp::ONE)),
                own(swap_cell, 1),
            ]);
            let left = self.c.cell(self.value(node) + swap, 255);
            let right = self.c.cell(self.value(sibling) - swap, 255);
            self.c
                .eq(own(left, 1), plus(&own(node, 1), &own(swap_cell, 1)));
            self.c.eq(
                own(right, 1),
                plus(&own(sibling, 1), &scaled(&own(swap_cell, 1), -Fp::ONE)),
            );
            node = self.hash(NODE_DOMAIN, &[left, right]);
        }
        node
    }

    fn authenticate(
        &mut self,
        root: usize,
        low: [usize; 3],
        index: usize,
        bits: &[usize],
        siblings: &[usize],
    ) -> (usize, Vec<usize>, Vec<usize>) {
        let key_zero = self.is_zero(low[0]);
        let value_zero = self.is_zero(low[1]);
        let index_zero = self.is_zero(index);
        self.c.eq(own(key_zero, 1), own(value_zero, 1));
        self.c.eq(own(key_zero, 1), own(index_zero, 1));
        let next_zero = self.is_zero(low[2]);
        let low_bytes = self.canonical(low[0]);
        let next_bytes = self.canonical(low[2]);
        let low_next = self.less(&low_bytes, &next_bytes);
        self.c.eq(
            plus(&own(next_zero, 1), &own(low_next, 1)),
            constant(vec![Fp::ONE]),
        );
        let leaf = self.hash(LEAF_DOMAIN, &low);
        let actual = self.root(leaf, bits, siblings);
        self.c.eq(own(root, 1), own(actual, 1));
        (next_zero, low_bytes, next_bytes)
    }

    fn finish(self, w: &mut Wiring<Fp>, family: usize) -> usize {
        let local = w.push(self.c.a);
        let mut rng = SpikeRng::new(b"exact indexed map native sponge");
        for hash in self.hashes {
            certificate::hash_relation(
                w,
                family,
                local,
                hash.domain,
                &hash.inputs,
                hash.outputs,
                &mut rng,
            );
        }
        local
    }
}

/// Authenticate the unique search route. Unlike a soft host verdict, a failed
/// opening or unrelated valid leaf is unsatisfiable; `present = 0` proves the
/// actual absence of the nonzero queried key from the authenticated map.
pub fn append_search<const D: usize>(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &SearchWitness<D>,
) -> SearchCells {
    let mut b = Builder {
        c: Cells::new(family),
        hashes: vec![],
    };
    b.c.a.name = "exact indexed-map search";
    let root = b.c.cell(witness.root, 255);
    let key = b.c.cell(witness.key, 255);
    let low = [witness.leaf.key, witness.leaf.value, witness.leaf.next].map(|v| b.c.cell(v, 255));
    let (index, bits, siblings) = b.path(&witness.path);
    b.nonzero(key);
    let (next_zero, low_bytes, next_bytes) = b.authenticate(root, low, index, &bits, &siblings);
    let key_bytes = b.canonical(key);
    let lower = b.less(&low_bytes, &key_bytes);
    let upper = b.less(&key_bytes, &next_bytes);
    let absent_value = b.value(lower) * (b.value(upper) + b.value(next_zero));
    let absent = b.c.bit(absent_value == Fp::ONE);
    b.c.a.rows.push([
        own(lower, 1),
        plus(&own(upper, 1), &own(next_zero, 1)),
        own(absent, 1),
    ]);
    let difference = b.c.cell(b.value(low[0]) - b.value(key), 255);
    b.c.eq(
        own(difference, 1),
        plus(&own(low[0], 1), &scaled(&own(key, 1), -Fp::ONE)),
    );
    let present = b.is_zero(difference);
    b.c.eq(
        plus(&own(present, 1), &own(absent, 1)),
        constant(vec![Fp::ONE]),
    );
    let local = b.finish(w, family);
    SearchCells {
        local,
        root,
        key,
        value: low[1],
        present,
    }
}

/// Append hard nonmembership and insertion constraints. A false witness still
/// builds the same shape, but has no satisfying assignment. The caller supplies
/// the shared byte range table and joins the returned cells to trusted context.
pub fn append_insert<const D: usize>(
    w: &mut Wiring<Fp>,
    family: usize,
    witness: &InsertWitness<D>,
) -> InsertCells {
    let mut b = Builder {
        c: Cells::new(family),
        hashes: vec![],
    };
    b.c.a.name = "exact indexed-map insertion";
    let old_root = b.c.cell(witness.old_root, 255);
    let key = b.c.cell(witness.key, 255);
    let value = b.c.cell(witness.value, 255);
    let low = [witness.low.key, witness.low.value, witness.low.next].map(|x| b.c.cell(x, 255));
    let (low_index, low_bits, low_siblings) = b.path(&witness.low_path);
    let (_, slot_bits, slot_siblings) = b.path(&witness.slot);
    b.nonzero(key);
    b.nonzero(value);
    let (next_zero, low_bytes, next_bytes) =
        b.authenticate(old_root, low, low_index, &low_bits, &low_siblings);
    let key_bytes = b.canonical(key);
    let lower = b.less(&low_bytes, &key_bytes);
    let upper = b.less(&key_bytes, &next_bytes);
    b.c.eq(own(lower, 1), constant(vec![Fp::ONE]));
    b.c.eq(
        plus(&own(next_zero, 1), &own(upper, 1)),
        constant(vec![Fp::ONE]),
    );

    let linked_leaf = b.hash(LEAF_DOMAIN, &[low[0], low[1], key]);
    let intermediate = b.root(linked_leaf, &low_bits, &low_siblings);
    let zero = b.c.cell(Fp::ZERO, 0);
    b.c.eq(own(zero, 1), constant(vec![Fp::ZERO]));
    let empty_root = b.root(zero, &slot_bits, &slot_siblings);
    b.c.eq(own(intermediate, 1), own(empty_root, 1));
    let new_leaf = b.hash(LEAF_DOMAIN, &[key, value, low[2]]);
    let new_root = b.root(new_leaf, &slot_bits, &slot_siblings);

    let local = b.finish(w, family);
    InsertCells {
        local,
        old_root,
        key,
        value,
        new_root,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        nonnative,
        r1cs::{Circuit, Instance, Witness},
        spartan,
        uniform::{Array, UniformCircuit, link},
    };
    use iroha_pasta::Eq as Vesta;
    use std::collections::BTreeMap;

    struct Tree<const D: usize> {
        nodes: BTreeMap<(usize, u32), Fp>,
        empty: Vec<Fp>,
    }
    impl<const D: usize> Tree<D> {
        fn new() -> Self {
            let mut empty = vec![Fp::ZERO];
            for i in 0..D {
                empty.push(poseidon::hash_with_domain(NODE_DOMAIN, &[empty[i]; 2]));
            }
            Self {
                nodes: BTreeMap::new(),
                empty,
            }
        }
        fn get(&self, height: usize, index: u32) -> Fp {
            *self
                .nodes
                .get(&(height, index))
                .unwrap_or(&self.empty[height])
        }
        fn put(&mut self, mut index: u32, leaf: Leaf) {
            self.nodes.insert((0, index), leaf_hash(leaf));
            for height in 0..D {
                let pair = [self.get(height, index & !1), self.get(height, index | 1)];
                index >>= 1;
                self.nodes.insert(
                    (height + 1, index),
                    poseidon::hash_with_domain(NODE_DOMAIN, &pair),
                );
            }
        }
        fn path(&self, index: u32) -> Path<D> {
            Path {
                index,
                siblings: std::array::from_fn(|h| self.get(h, (index >> h) ^ 1)),
            }
        }
        fn root(&self) -> Fp {
            self.get(D, 0)
        }
    }

    fn field(hex: &str) -> Fp {
        let bytes: [u8; 32] =
            std::array::from_fn(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap());
        Option::from(Fp::from_repr(bytes)).unwrap()
    }

    fn fixture<const D: usize>() -> (InsertWitness<D>, Fp) {
        let mut tree = Tree::<D>::new();
        let low = Leaf {
            key: Fp::from(10),
            value: Fp::from(20),
            next: Fp::from(100),
        };
        tree.put(
            0,
            Leaf {
                key: Fp::ZERO,
                value: Fp::ZERO,
                next: low.key,
            },
        );
        tree.put(1, low);
        tree.put(
            2,
            Leaf {
                key: low.next,
                value: Fp::from(30),
                next: Fp::ZERO,
            },
        );
        let old_root = tree.root();
        let low_path = tree.path(1);
        let key = Fp::from(50);
        let value = Fp::from(70);
        tree.put(1, Leaf { next: key, ..low });
        let slot = tree.path(3);
        let witness = InsertWitness {
            old_root,
            key,
            value,
            low,
            low_path,
            slot,
        };
        tree.put(
            3,
            Leaf {
                key,
                value,
                next: low.next,
            },
        );
        (witness, tree.root())
    }

    fn build<const D: usize>(
        input: &InsertWitness<D>,
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, InsertCells) {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let cells = append_insert(&mut w, family, input);
        let outputs = [cells.old_root, cells.key, cells.value, cells.new_root];
        let (c, i, w) = finish_test(w, family, cells.local, outputs);
        (c, i, w, cells)
    }

    fn build_search<const D: usize>(
        input: &SearchWitness<D>,
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, SearchCells) {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let cells = append_search(&mut w, family, input);
        let outputs = [cells.root, cells.key, cells.value, cells.present];
        let (c, i, w) = finish_test(w, family, cells.local, outputs);
        (c, i, w, cells)
    }

    fn finish_test(
        mut w: Wiring<Fp>,
        family: usize,
        local: usize,
        outputs: [usize; 4],
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>) {
        let range = w.push(Array {
            name: "indexed-map range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|i| Fp::from(i as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let values = outputs.map(|k| w.arrays[local].values[k][0]);
        let io = w.push(Array {
            name: "indexed-map authenticated join ports",
            family,
            rho: 2,
            r_count: 4,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![values.to_vec()],
        });
        w.io_arrays.push((io, 4));
        for (i, k) in outputs.into_iter().enumerate() {
            w.arrays[local].rows.push([
                plus(
                    &own(k, 1),
                    &scaled(&link(io, 0, vec![Some(i)], Fp::ONE), -Fp::ONE),
                ),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        let (arrays, pi, counts, tables, copies) = w.finish();
        let (c, i, w) = UniformCircuit::build_with_copies(
            "exact indexed insertion",
            arrays,
            pi,
            counts,
            tables,
            copies,
            12,
            8,
            b"exact indexed insertion",
        );
        (c, i, w)
    }

    fn holds(c: &UniformCircuit<Fp>, i: &Instance<Fp>, w: &Witness<Fp>) -> bool {
        let mut values = w.w.clone();
        let rel = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = rel.abc(&c.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }

    #[test]
    fn native_depth32_matches_g1_golden_insertion() {
        // Golden values originate in fixtures/kagemusha/wallet_v1_vectors.json,
        // poseidon.indexed_tree, also consumed by native digest_parity.rs.
        let mut tree = Tree::<32>::new();
        let sentinel = Leaf {
            key: Fp::ZERO,
            value: Fp::ZERO,
            next: Fp::ZERO,
        };
        assert_eq!(
            leaf_hash(sentinel),
            field("d3f1ff2c55cd0da336138793d486261211bfdbc7a4b6e9be336b8b09bc0e3a1c")
        );
        tree.put(0, sentinel);
        assert_eq!(
            tree.root(),
            field("e1844c5683af83c12383e719e37a29a9b1867d1c171e77e1d060bafc401d5118")
        );
        let old_root = tree.root();
        let low_path = tree.path(0);
        let key = field("0000000000000000000000000000000002000000000000000000000000000000");
        let value = field("194998246e7149eb6be1778837a5c3e0549e4bad56edc403c7f93779d4de3027");
        tree.put(
            0,
            Leaf {
                next: key,
                ..sentinel
            },
        );
        assert_eq!(
            tree.root(),
            field("a3b1b80aeaeebd5554bdbd55733e2943387c82b268d0b8c83dd989bbc9a68311")
        );
        let input = InsertWitness {
            old_root,
            key,
            value,
            low: sentinel,
            low_path,
            slot: tree.path(1),
        };
        tree.put(
            1,
            Leaf {
                key,
                value,
                next: Fp::ZERO,
            },
        );
        assert_eq!(
            tree.root(),
            field("b2d465287fcd2a1f935bd73eac7bdd46aeeb16a97a5ebe8813ea0663e603e62c")
        );
        assert_eq!(path_root(leaf_hash(input.low), &input.low_path), old_root);
        let (c, i, w, _) = build(&input);
        assert_eq!(i.io[7], tree.root());
        assert!(holds(&c, &i, &w));
    }

    #[test]
    fn valid_insertion_proves_and_binds_every_public_port() {
        let (input, expected) = fixture::<3>();
        let (c, i, w, ports) = build(&input);
        assert!(holds(&c, &i, &w));
        assert_eq!(i.io[7], expected);
        let gens = Gens::<Vesta>::new(c.shape.cols());
        let (proof, _) = spartan::prove_circuit(
            &c.shape,
            &gens,
            &i,
            &w,
            &mut SpikeRng::new(b"indexed map proof"),
            &c,
        );
        assert!(spartan::verify_circuit(&c.shape, &gens, &i, &proof.to_bytes(), &c).is_ok());
        for p in 4..8 {
            let mut wrong = i.clone();
            wrong.io[p] += Fp::ONE;
            assert!(
                spartan::verify_circuit(&c.shape, &gens, &wrong, &proof.to_bytes(), &c).is_err()
            );
        }
        for k in [ports.old_root, ports.key, ports.value, ports.new_root] {
            let mut wrong = Witness { w: w.w.clone() };
            wrong.w[c.cell_index(ports.local, k, 0)] += Fp::ONE;
            assert!(!holds(&c, &i, &wrong));
        }
    }

    #[test]
    fn forged_search_routes_and_nonempty_destinations_are_rejected() {
        let (input, _) = fixture::<3>();
        let mut cases = Vec::new();
        for (name, key) in [
            ("duplicate low", input.low.key),
            ("duplicate successor", input.low.next),
            ("zero key", Fp::ZERO),
            ("outside gap", Fp::from(101)),
        ] {
            let mut x = input.clone();
            x.key = key;
            cases.push((name, x));
        }
        let mut x = input.clone();
        x.value = Fp::ZERO;
        cases.push(("zero value", x));
        let mut x = input.clone();
        x.old_root += Fp::ONE;
        cases.push(("wrong authenticated root", x));
        let mut x = input.clone();
        x.low_path.index = 9;
        cases.push(("index exceeds depth", x));
        let mut x = input.clone();
        x.low_path.index = 0;
        cases.push(("nonsentinel in slot zero", x));
        let mut x = input.clone();
        x.low.value = Fp::ZERO;
        cases.push(("nonsentinel zero value", x));
        let mut x = input.clone();
        x.slot = input.low_path.clone();
        cases.push(("occupied low slot", x));
        let mut x = input.clone();
        x.slot.siblings[0] += Fp::ONE;
        cases.push(("forged intermediate path", x));
        let mut x = input.clone();
        x.low.next = Fp::from(5);
        cases.push(("nonincreasing successor", x));
        for (name, input) in cases {
            let (c, i, w, _) = build(&input);
            assert!(!holds(&c, &i, &w), "{name}");
        }
    }

    #[test]
    fn ordering_uses_the_entire_canonical_field_integer() {
        let (mut input, _) = fixture::<3>();
        // Equal low 128 bits must not compare as equal; p-1 is also canonical.
        input.key = Fp::from_u128(50) + Fp::from_u128(1u128 << 127).double();
        let (c, i, w, _) = build(&input);
        assert!(
            !holds(&c, &i, &w),
            "high half puts key beyond the successor"
        );
        input.key = -Fp::ONE;
        let (c, i, w, _) = build(&input);
        assert!(!holds(&c, &i, &w), "canonical p-1 cannot wrap into the gap");
    }

    #[test]
    fn search_proves_membership_or_real_absence_with_identical_matrices() {
        let (input, _) = fixture::<3>();
        let mut query = SearchWitness {
            root: input.old_root,
            key: input.low.key,
            leaf: input.low,
            path: input.low_path,
        };
        let (present, pi, pw, pc) = build_search(&query);
        assert!(holds(&present, &pi, &pw));
        assert_eq!(pi.io[7], Fp::ONE);
        assert_eq!(pi.io[6], input.low.value);
        query.key = Fp::from(50);
        let (absent, ai, aw, _) = build_search(&query);
        assert!(holds(&absent, &ai, &aw));
        assert_eq!(ai.io[7], Fp::ZERO);
        assert_eq!(
            present.entries(&[Fp::from(31), Fp::from(47)]),
            absent.entries(&[Fp::from(31), Fp::from(47)])
        );
        let gens = Gens::<Vesta>::new(present.shape.cols());
        for (i, w) in [(&pi, &pw), (&ai, &aw)] {
            let (proof, _) = spartan::prove_circuit(
                &present.shape,
                &gens,
                i,
                w,
                &mut SpikeRng::new(b"indexed search"),
                &present,
            );
            assert!(
                spartan::verify_circuit(&present.shape, &gens, i, &proof.to_bytes(), &present)
                    .is_ok()
            );
            let mut wrong = i.clone();
            wrong.io[7] = Fp::ONE - wrong.io[7];
            assert!(
                spartan::verify_circuit(&present.shape, &gens, &wrong, &proof.to_bytes(), &present)
                    .is_err()
            );
        }
        let mut wrong = Witness { w: pw.w.clone() };
        wrong.w[present.cell_index(pc.local, pc.present, 0)] = Fp::ZERO;
        assert!(!holds(&present, &pi, &wrong));
    }

    #[test]
    fn unrelated_authenticated_leaves_cannot_select_absence() {
        let (input, _) = fixture::<3>();
        let honest = SearchWitness {
            root: input.old_root,
            key: input.low.key,
            leaf: input.low,
            path: input.low_path,
        };
        for key in [
            Fp::ZERO,
            Fp::from(5),
            Fp::from(100),
            Fp::from(101),
            -Fp::ONE,
        ] {
            let mut query = honest.clone();
            query.key = key;
            assert_eq!(path_root(leaf_hash(query.leaf), &query.path), query.root);
            let (c, i, w, _) = build_search(&query);
            assert!(
                !holds(&c, &i, &w),
                "valid path cannot answer an unrelated query {key:?}"
            );
        }
        let mut bad_path = honest.clone();
        bad_path.path.siblings[1] += Fp::ONE;
        let (c, i, w, _) = build_search(&bad_path);
        assert!(!holds(&c, &i, &w));
        let mut bad_shape = honest;
        bad_shape.leaf.value = Fp::ZERO;
        bad_shape.root = path_root(leaf_hash(bad_shape.leaf), &bad_shape.path);
        let (c, i, w, _) = build_search(&bad_shape);
        assert!(!holds(&c, &i, &w), "coherently rehashed malformed leaf");
    }

    #[test]
    fn search_accepts_empty_sentinel_and_terminal_gaps() {
        let sentinel = Leaf {
            key: Fp::ZERO,
            value: Fp::ZERO,
            next: Fp::ZERO,
        };
        let terminal = Leaf {
            key: Fp::from(10),
            value: Fp::from(20),
            next: Fp::ZERO,
        };
        for (leaf, index) in [(sentinel, 0), (terminal, 1)] {
            let mut tree = Tree::<3>::new();
            if index != 0 {
                tree.put(
                    0,
                    Leaf {
                        next: terminal.key,
                        ..sentinel
                    },
                );
            }
            tree.put(index, leaf);
            let query = SearchWitness {
                root: tree.root(),
                key: Fp::from(50),
                leaf,
                path: tree.path(index),
            };
            let (c, i, w, _) = build_search(&query);
            assert!(holds(&c, &i, &w));
            assert_eq!(i.io[7], Fp::ZERO);
        }
    }

    fn coherent<const D: usize>(
        low: Leaf,
        index: u32,
        slot: u32,
        key: Fp,
    ) -> (InsertWitness<D>, Fp) {
        let mut tree = Tree::<D>::new();
        tree.put(index, low);
        let old_root = tree.root();
        let low_path = tree.path(index);
        tree.put(index, Leaf { next: key, ..low });
        let slot_path = tree.path(slot);
        let value = Fp::from(73);
        let witness = InsertWitness {
            old_root,
            key,
            value,
            low,
            low_path,
            slot: slot_path,
        };
        tree.put(
            slot,
            Leaf {
                key,
                value,
                next: low.next,
            },
        );
        (witness, tree.root())
    }

    #[test]
    fn malformed_leaf_shape_fails_even_with_valid_hash_paths() {
        let ordinary = Leaf {
            key: Fp::from(10),
            value: Fp::from(20),
            next: Fp::from(100),
        };
        let sentinel = Leaf {
            key: Fp::ZERO,
            value: Fp::ZERO,
            next: Fp::from(100),
        };
        for (name, low, index) in [
            ("nonsentinel in slot zero", ordinary, 0),
            ("sentinel outside slot zero", sentinel, 1),
            (
                "zero nonsentinel value",
                Leaf {
                    value: Fp::ZERO,
                    ..ordinary
                },
                1,
            ),
            (
                "nonzero sentinel value",
                Leaf {
                    value: Fp::ONE,
                    ..sentinel
                },
                0,
            ),
            (
                "nonincreasing successor",
                Leaf {
                    next: Fp::from(5),
                    ..ordinary
                },
                1,
            ),
        ] {
            let (input, _) = coherent::<3>(low, index, 3, Fp::from(50));
            assert_eq!(
                path_root(leaf_hash(input.low), &input.low_path),
                input.old_root
            );
            let intermediate = path_root(
                leaf_hash(Leaf {
                    next: input.key,
                    ..input.low
                }),
                &input.low_path,
            );
            assert_eq!(path_root(Fp::ZERO, &input.slot), intermediate);
            let (c, i, w, _) = build(&input);
            assert!(!holds(&c, &i, &w), "{name}: authenticated malformed leaf");
        }
    }

    #[test]
    fn final_u32_slot_is_valid_and_high_direction_bit_is_bound() {
        let sentinel = Leaf {
            key: Fp::ZERO,
            value: Fp::ZERO,
            next: Fp::ZERO,
        };
        let (input, expected) = coherent::<32>(sentinel, 0, u32::MAX, Fp::from(50));
        let (c, i, w, _) = build(&input);
        assert_eq!(i.io[7], expected);
        assert!(holds(&c, &i, &w));
        let mut wrong = input;
        wrong.slot.index ^= 1 << 31;
        let (c, i, w, _) = build(&wrong);
        assert!(!holds(&c, &i, &w), "bit31 cannot redirect a slot opening");
    }

    #[test]
    fn different_valid_insertions_keep_identical_constraint_matrices() {
        let sentinel = Leaf {
            key: Fp::ZERO,
            value: Fp::ZERO,
            next: Fp::ZERO,
        };
        let a = coherent::<3>(sentinel, 0, 1, Fp::from(50)).0;
        let b = coherent::<3>(sentinel, 0, 7, Fp::from(60)).0;
        let (ca, ia, wa, _) = build(&a);
        let (cb, ib, wb, _) = build(&b);
        assert!(holds(&ca, &ia, &wa) && holds(&cb, &ib, &wb));
        assert_eq!(
            ca.entries(&[Fp::from(31), Fp::from(47)]),
            cb.entries(&[Fp::from(31), Fp::from(47)])
        );
    }
}
