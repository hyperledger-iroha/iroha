//! Native differential, exact-count, canonical-padding and geometry tamper checks.

use core::marker::PhantomData;

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_pasta::{Fp, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value},
};

use super::*;
use crate::{
    GlueConfig, LimbBits, RunningSumChip, RunningSumConfig,
    blake2b::{BLAKE2B_ADVICE_COLUMNS, Blake2bConfig},
    tamper::undetected_tampers,
};

#[derive(Clone)]
struct GeometryCircuit<F> {
    index: u32,
    count: u64,
    steps: usize,
    wrong_presence: Option<usize>,
    known: bool,
    field: PhantomData<F>,
}

fn arithmetic<F: PastaField>(meta: &mut ConstraintSystem<F>) -> (GlueConfig, RunningSumConfig) {
    let columns = core::array::from_fn(|_| meta.advice_column());
    let constants = meta.fixed_column();
    let glue = GlueConfig::configure(meta, columns, constants);
    let range_column = meta.advice_column();
    let range = RunningSumConfig::configure(meta, range_column, LimbBits::new(4).unwrap());
    (glue, range)
}

fn known<T: Copy>(present: bool, value: T) -> Value<T> {
    if present {
        Value::known(value)
    } else {
        Value::unknown()
    }
}

impl<F: PastaField> Circuit<F> for GeometryCircuit<F> {
    type Config = (GlueConfig, RunningSumConfig);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        arithmetic(meta)
    }

    fn synthesize(
        &self,
        (glue, range): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(glue);
        let mut range = RunningSumChip::new(range);
        range.load_table(&mut layouter)?;
        layouter.assign_region(
            || "counted geometry",
            |mut region| {
                let mut uint = UintChip::new(&mut glue, &mut range);
                let index =
                    uint.assign::<32>(&mut region, known(self.known, u128::from(self.index)))?;
                let count =
                    uint.assign::<64>(&mut region, known(self.known, u128::from(self.count)))?;
                GlueChip::assert_constant(
                    &mut region,
                    index.word(),
                    F::from(u64::from(self.index)),
                )?;
                GlueChip::assert_constant(&mut region, count.word(), F::from(self.count))?;
                let mut current = Geometry::new(&mut uint, &mut region, &index, &count)?;
                let (mut native_index, mut width) = (u64::from(self.index), self.count);
                for level in 0..self.steps {
                    let step = current.step(&mut uint, &mut region)?;
                    let present = (native_index ^ 1) < width;
                    let expected = present ^ (self.wrong_presence == Some(level));
                    GlueChip::assert_constant(
                        &mut region,
                        step.sibling().word(),
                        F::from(u64::from(expected)),
                    )?;
                    GlueChip::assert_constant(
                        &mut region,
                        step.right().word(),
                        F::from(native_index & 1),
                    )?;
                    native_index >>= 1;
                    width = (width >> 1) + (width & 1);
                    GlueChip::assert_constant(
                        &mut region,
                        step.next().index().word(),
                        F::from(native_index),
                    )?;
                    GlueChip::assert_constant(
                        &mut region,
                        step.next().width().word(),
                        F::from(width),
                    )?;
                    current = step.next().clone();
                }
                current.finish(&mut region)
            },
        )
    }
}

fn geometry<F: PastaField>(index: u32, count: u64, steps: usize) -> GeometryCircuit<F> {
    GeometryCircuit {
        index,
        count,
        steps,
        wrong_presence: None,
        known: true,
        field: PhantomData,
    }
}

fn satisfies<F: PastaField, C: Circuit<F>>(circuit: &C, k: u32) -> bool {
    check_circuit(circuit, k, &[], CheckMode::Strict)
        .expect("strict layout")
        .is_satisfied()
}

fn geometry_cases<F: PastaField>() {
    for count in [1_u64, 2, 3, 5, 7, 8, 9, 65_536, u64::MAX] {
        let depth = (64 - (count - 1).leading_zeros()) as usize;
        for index in [0, u32::try_from(count - 1).unwrap_or(u32::MAX)] {
            let circuit = geometry::<F>(index, count, depth);
            assert!(satisfies(&circuit, 15), "index={index} count={count}");
            if depth > 0 {
                assert!(!satisfies(
                    &GeometryCircuit {
                        steps: depth - 1,
                        ..circuit.clone()
                    },
                    15
                ));
                assert!(!satisfies(
                    &GeometryCircuit {
                        wrong_presence: Some(0),
                        ..circuit
                    },
                    15
                ));
            }
        }
    }
    for (index, count, steps) in [(0, 0, 0), (3, 3, 2), (u32::MAX, 3, 2), (0, 1, 1), (0, 2, 2)] {
        assert!(!satisfies(&geometry::<F>(index, count, steps), 10));
    }
}

#[test]
fn geometry_matches_counted_tree_on_both_fields() {
    geometry_cases::<Fp>();
    geometry_cases::<Fq>();
}

#[test]
fn every_geometry_advice_cell_is_constrained() {
    assert!(
        undetected_tampers(&geometry::<Fp>(2, 3, 2), 10, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        undetected_tampers(&geometry::<Fq>(1, 2, 1), 9, &[])
            .unwrap()
            .is_empty()
    );
}

#[derive(Clone)]
struct PathCircuit<F> {
    index: u32,
    count: u64,
    leaf: [u8; 32],
    siblings: Vec<[u8; 32]>,
    root: [u8; 32],
    known: bool,
    field: PhantomData<F>,
}

impl<F: PastaField> Circuit<F> for PathCircuit<F> {
    type Config = (GlueConfig, RunningSumConfig, Blake2bConfig);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let (glue, range) = arithmetic(meta);
        let advice = core::array::from_fn::<_, BLAKE2B_ADVICE_COLUMNS, _>(|_| meta.advice_column());
        let constants = meta.fixed_column();
        (
            glue,
            range,
            Blake2bConfig::configure(meta, advice, constants),
        )
    }
    fn synthesize(
        &self,
        (glue, range, hash): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::new(glue);
        let mut range = RunningSumChip::new(range);
        let mut hash = Blake2bChip::new(&hash);
        range.load_table(&mut layouter)?;
        layouter.assign_region(
            || "native counted path",
            |mut region| {
                let mut uint = UintChip::new(&mut glue, &mut range);
                let index =
                    uint.assign::<32>(&mut region, known(self.known, u128::from(self.index)))?;
                let count =
                    uint.assign::<64>(&mut region, known(self.known, u128::from(self.count)))?;
                GlueChip::assert_constant(
                    &mut region,
                    index.word(),
                    F::from(u64::from(self.index)),
                )?;
                GlueChip::assert_constant(&mut region, count.word(), F::from(self.count))?;
                let leaf = assign_bytes(&mut uint, &mut region, self.known, &self.leaf)?;
                let mut path =
                    PathCells::start(&mut hash, &mut uint, &mut region, &index, &count, &leaf)?;
                for sibling in &self.siblings {
                    let sibling = assign_bytes(&mut uint, &mut region, self.known, sibling)?;
                    path = path.step(&mut hash, &mut uint, &mut region, &sibling)?;
                }
                let root = self
                    .root
                    .iter()
                    .map(|b| uint.glue().constant(&mut region, F::from(u64::from(*b))))
                    .collect::<Result<Vec<_>, _>>()?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                path.finish(&mut region, &root)
            },
        )
    }
}

fn assign_bytes<F: PastaField>(
    uint: &mut UintChip<'_, F>,
    region: &mut Region<'_, F>,
    present: bool,
    bytes: &[u8; 32],
) -> Result<[Word<F>; 32], Error> {
    bytes
        .iter()
        .map(|b| {
            uint.glue()
                .witness(region, known(present, F::from(u64::from(*b))))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Synthesis)
}

fn fixture<F: PastaField>(count: u32, index: u32) -> PathCircuit<F> {
    let leaves = (0..count)
        .map(|i| HashOf::<u32>::from_untyped_unchecked(Hash::new(i.to_le_bytes())))
        .collect::<Vec<_>>();
    let tree = leaves.iter().copied().collect::<MerkleTree<u32>>();
    let proof = tree.get_proof(index).unwrap();
    assert!(proof.verify(&leaves[index as usize], &tree.commitment().unwrap()));
    PathCircuit {
        index,
        count: u64::from(count),
        leaf: *leaves[index as usize].as_ref(),
        siblings: proof
            .audit_path()
            .iter()
            .map(|node| node.map_or([0; 32], |hash| *hash.as_ref()))
            .collect(),
        root: *tree.root().unwrap().as_ref(),
        known: true,
        field: PhantomData,
    }
}

#[test]
fn application_paths_match_native_blake2b_on_both_fields() {
    for (count, index) in [(1, 0), (2, 1), (3, 0), (3, 2), (5, 4)] {
        assert!(
            satisfies(&fixture::<Fp>(count, index), 16),
            "Fp {count}/{index}"
        );
        assert!(
            satisfies(&fixture::<Fq>(count, index), 16),
            "Fq {count}/{index}"
        );
    }
}

#[test]
fn paths_reject_wrong_count_order_padding_marker_and_root() {
    let honest = fixture::<Fp>(3, 2);
    let mut cases = Vec::new();
    let mut wrong = honest.clone();
    wrong.count = 4;
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.index = 1;
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.siblings[0][0] = 1;
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.siblings[1][31] &= !1;
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.siblings[1] = [0; 32];
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.leaf[31] &= !1;
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.root[0] ^= 1;
    cases.push(wrong);
    let mut wrong = honest.clone();
    wrong.siblings.swap(0, 1);
    cases.push(wrong);
    let mut wrong = honest;
    wrong.siblings.pop();
    cases.push(wrong);
    for (index, case) in cases.iter().enumerate() {
        assert!(!satisfies(case, 16), "consistent forgery {index}");
    }
}
