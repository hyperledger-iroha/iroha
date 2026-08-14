// This file is part of MIDNIGHT-ZK.
// Copyright (C) 2025 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// You may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use std::{convert::TryInto, marker::PhantomData};
use ff::{Field, PrimeField};
use halo2_proofs::{
    circuit::{Chip, Layouter, Region, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, TableColumn},
    poly::Rotation,
};
use crate::zk::kagemusha_sha256_table16_v4::{
    AssignedBits, TABLE16_SPREAD_TABLE_ROWS,
    util::{lebs2ip, spread_bits},
};
const BITS_4: usize = 1 << 4;
const BITS_7: usize = 1 << 7;
const BITS_10: usize = 1 << 10;
const BITS_11: usize = 1 << 11;
const BITS_13: usize = 1 << 13;
const BITS_14: usize = 1 << 14;
const LAST_DENSE: u64 = (1 << 16) - 1;
const FIRST_TAIL_DENSE: u64 = TABLE16_SPREAD_TABLE_ROWS as u64;
const LAST_SPREAD: u64 = 0x5555_5555;
/// An input word into a lookup, containing (tag, dense, spread)
#[derive(Copy, Clone, Debug)]
pub(super) struct SpreadWord<const DENSE: usize, const SPREAD: usize> {
    pub tag: u8,
    pub dense: [bool; DENSE],
    pub spread: [bool; SPREAD],
}
/// Helper function that returns tag of 16-bit input
pub fn get_tag(input: u16) -> u8 {
    let input = input as usize;
    if input < BITS_4 {
        0
    } else if input < BITS_7 {
        1
    } else if input < BITS_10 {
        2
    } else if input < BITS_11 {
        3
    } else if input < BITS_13 {
        4
    } else if input < BITS_14 {
        5
    } else {
        6
    }
}
impl<const DENSE: usize, const SPREAD: usize> SpreadWord<DENSE, SPREAD> {
    pub(super) fn new(dense: [bool; DENSE]) -> Self {
        assert!(DENSE <= 16);
        SpreadWord {
            tag: get_tag(lebs2ip(&dense) as u16),
            dense,
            spread: spread_bits(dense),
        }
    }
    pub(super) fn try_new<T: TryInto<[bool; DENSE]> + std::fmt::Debug>(dense: T) -> Self
    where
        <T as TryInto<[bool; DENSE]>>::Error: std::fmt::Debug,
    {
        assert!(DENSE <= 16);
        let dense: [bool; DENSE] = dense.try_into().unwrap();
        SpreadWord {
            tag: get_tag(lebs2ip(&dense) as u16),
            dense,
            spread: spread_bits(dense),
        }
    }
}
/// A variable stored in advice columns corresponding to a row of
/// [`SpreadTableConfig`].
#[derive(Clone, Debug)]
pub(super) struct SpreadVar<const DENSE: usize, const SPREAD: usize, F: PrimeField> {
    pub dense: AssignedBits<DENSE, F>,
    pub spread: AssignedBits<SPREAD, F>,
}
impl<const DENSE: usize, const SPREAD: usize, F: PrimeField> SpreadVar<DENSE, SPREAD, F> {
    pub(super) fn with_lookup(
        region: &mut Region<'_, F>,
        cols: &SpreadInputs,
        row: usize,
        word: Value<SpreadWord<DENSE, SPREAD>>,
    ) -> Result<Self, Error> {
        let tag = word.map(|word| word.tag);
        let dense_val = word.map(|word| word.dense);
        let spread_val = word.map(|word| word.spread);
        let tail = word.map(|word| {
            F::from(u64::from(
                lebs2ip(&word.dense) >= TABLE16_SPREAD_TABLE_ROWS as u64,
            ))
        });
        region.assign_advice(cols.tag, row, tag.map(|tag| F::from(tag as u64)));
        region.assign_advice(cols.tail, row, tail);
        let dense =
            AssignedBits::<DENSE, F>::assign_bits(region, || "dense", cols.dense, row, dense_val)?;
        let spread = AssignedBits::<SPREAD, F>::assign_bits(
            region,
            || "spread",
            cols.spread,
            row,
            spread_val,
        )?;
        Ok(SpreadVar { dense, spread })
    }
    pub(super) fn without_lookup_fixed(
        region: &mut Region<'_, F>,
        dense_col: Column<Advice>,
        dense_row: usize,
        spread_col: Column<Advice>,
        spread_row: usize,
        word: SpreadWord<DENSE, SPREAD>,
    ) -> Result<Self, Error> {
        let dense_val = word.dense;
        let spread_val = word.spread;
        let dense = AssignedBits::<DENSE, F>::assign_bits_fixed(
            region,
            || "dense",
            dense_col,
            dense_row,
            dense_val,
        )?;
        let spread = AssignedBits::<SPREAD, F>::assign_bits_fixed(
            region,
            || "spread",
            spread_col,
            spread_row,
            spread_val,
        )?;
        Ok(SpreadVar { dense, spread })
    }
    pub(super) fn without_lookup(
        region: &mut Region<'_, F>,
        dense_col: Column<Advice>,
        dense_row: usize,
        spread_col: Column<Advice>,
        spread_row: usize,
        word: Value<SpreadWord<DENSE, SPREAD>>,
    ) -> Result<Self, Error> {
        let dense_val = word.map(|word| word.dense);
        let spread_val = word.map(|word| word.spread);
        let dense = AssignedBits::<DENSE, F>::assign_bits(
            region,
            || "dense",
            dense_col,
            dense_row,
            dense_val,
        )?;
        let spread = AssignedBits::<SPREAD, F>::assign_bits(
            region,
            || "spread",
            spread_col,
            spread_row,
            spread_val,
        )?;
        Ok(SpreadVar { dense, spread })
    }
}
#[derive(Clone, Debug)]
pub(super) struct SpreadInputs {
    pub(super) tag: Column<Advice>,
    pub(super) dense: Column<Advice>,
    pub(super) spread: Column<Advice>,
    pub(super) tail: Column<Advice>,
}
#[derive(Clone, Debug)]
pub(super) struct SpreadTable {
    pub(super) tag: TableColumn,
    pub(super) dense: TableColumn,
    pub(super) spread: TableColumn,
}
#[derive(Clone, Debug)]
pub(super) struct SpreadTableConfig {
    pub input: SpreadInputs,
    pub table: SpreadTable,
}
#[derive(Clone, Debug)]
pub(super) struct SpreadTableChip<F: Field> {
    config: SpreadTableConfig,
    _marker: PhantomData<F>,
}
impl<F: Field> Chip<F> for SpreadTableChip<F> {
    type Config = SpreadTableConfig;
    type Loaded = ();
    fn config(&self) -> &Self::Config {
        &self.config
    }
    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}
impl<F: PrimeField> SpreadTableChip<F> {
    pub(super) fn configure_table(meta: &mut ConstraintSystem<F>) -> SpreadTable {
        SpreadTable {
            tag: meta.lookup_table_column(),
            dense: meta.lookup_table_column(),
            spread: meta.lookup_table_column(),
        }
    }
    #[cfg(test)]
    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        input_tag: Column<Advice>,
        input_dense: Column<Advice>,
        input_spread: Column<Advice>,
    ) -> <Self as Chip<F>>::Config {
        let table = Self::configure_table(meta);
        let input_tail = meta.advice_column();
        Self::configure_with_table(
            meta,
            input_tag,
            input_dense,
            input_spread,
            input_tail,
            table,
        )
    }
    pub(super) fn configure_with_table(
        meta: &mut ConstraintSystem<F>,
        input_tag: Column<Advice>,
        input_dense: Column<Advice>,
        input_spread: Column<Advice>,
        input_tail: Column<Advice>,
        table: SpreadTable,
    ) -> <Self as Chip<F>>::Config {
        // The physical k=16 table omits dense words 65527..=65535. A valid
        // tail witness redirects only those tuples to the canonical zero row.
        meta.lookup("spread table or constrained tail", |meta| {
            let tag_cur = meta.query_advice(input_tag, Rotation::cur());
            let dense_cur = meta.query_advice(input_dense, Rotation::cur());
            let spread_cur = meta.query_advice(input_spread, Rotation::cur());
            let tail_cur = meta.query_advice(input_tail, Rotation::cur());
            let loaded = Expression::Constant(F::ONE) - tail_cur;
            vec![
                (loaded.clone() * tag_cur, table.tag),
                (loaded.clone() * dense_cur, table.dense),
                (loaded * spread_cur, table.spread),
            ]
        });
        // In the canonical table, dense == spread holds only for rows zero
        // and one, and both rows have tag zero. This proves tail is Boolean;
        // when it is one, the first component also forces input_tag == 6.
        // A lookup is used instead of an unselected gate so reserved rows are
        // handled by Halo2's lookup argument rather than activating a gate.
        meta.lookup("spread-table tail flag and tag", |meta| {
            let tag_cur = meta.query_advice(input_tag, Rotation::cur());
            let tail_cur = meta.query_advice(input_tail, Rotation::cur());
            vec![
                (
                    tail_cur.clone() * (Expression::Constant(F::from(6)) - tag_cur),
                    table.tag,
                ),
                (tail_cur.clone(), table.dense),
                (tail_cur, table.spread),
            ]
        });
        // For c = 65535 - dense, bit interleaving commutes with complement:
        // spread(dense) + spread(c) = spread(65535) = 0x5555_5555.
        // Looking up (tag=0, c, complement_spread) constrains c to 0..=15
        // and proves the exact spread polynomial without another table.
        meta.lookup("spread-table tail complement", |meta| {
            let dense_cur = meta.query_advice(input_dense, Rotation::cur());
            let spread_cur = meta.query_advice(input_spread, Rotation::cur());
            let tail_cur = meta.query_advice(input_tail, Rotation::cur());
            vec![
                (Expression::Constant(F::ZERO), table.tag),
                (
                    tail_cur.clone() * (Expression::Constant(F::from(LAST_DENSE)) - dense_cur),
                    table.dense,
                ),
                (
                    tail_cur * (Expression::Constant(F::from(LAST_SPREAD)) - spread_cur),
                    table.spread,
                ),
            ]
        });
        // Since the complement lookup proves c <= 15, requiring
        // dense - 65527 = 8 - c to occur in the dense table leaves c <= 8.
        // Together the two lookups prove dense is exactly 65527..=65535.
        meta.lookup("spread-table tail lower endpoint", |meta| {
            let dense_cur = meta.query_advice(input_dense, Rotation::cur());
            let tail_cur = meta.query_advice(input_tail, Rotation::cur());
            vec![(
                tail_cur * (dense_cur - Expression::Constant(F::from(FIRST_TAIL_DENSE))),
                table.dense,
            )]
        });
        SpreadTableConfig {
            input: SpreadInputs {
                tag: input_tag,
                dense: input_dense,
                spread: input_spread,
                tail: input_tail,
            },
            table,
        }
    }
    pub fn load(
        config: SpreadTableConfig,
        layouter: &mut impl Layouter<F>,
    ) -> Result<<Self as Chip<F>>::Loaded, Error> {
        Self::load_table(config.table, layouter)
    }
    pub(super) fn load_table(
        config: SpreadTable,
        layouter: &mut impl Layouter<F>,
    ) -> Result<<Self as Chip<F>>::Loaded, Error> {
        layouter.assign_table(
            || "spread table",
            |mut table| {
                // We generate the row values lazily (we only need them during keygen).
                let mut rows = SpreadTableConfig::generate::<F>();
                for index in 0..TABLE16_SPREAD_TABLE_ROWS {
                    let mut row = None;
                    table.assign_cell(
                        || "tag",
                        config.tag,
                        index,
                        || {
                            row = rows.next();
                            Value::known(row.map(|(tag, _, _)| tag).unwrap())
                        },
                    )?;
                    table.assign_cell(
                        || "dense",
                        config.dense,
                        index,
                        || Value::known(row.map(|(_, dense, _)| dense).unwrap()),
                    )?;
                    table.assign_cell(
                        || "spread",
                        config.spread,
                        index,
                        || Value::known(row.map(|(_, _, spread)| spread).unwrap()),
                    )?;
                }
                Ok(())
            },
        )
    }
}
impl SpreadTableConfig {
    fn generate<F: PrimeField>() -> impl Iterator<Item = (F, F, F)> {
        (1..=(1 << 16)).scan((F::ZERO, F::ZERO, F::ZERO), |(tag, dense, spread), i| {
            // We computed this table row in the previous iteration.
            let res = (*tag, *dense, *spread);
            // i holds the zero-indexed row number for the next table row.
            match i {
                BITS_4 | BITS_7 | BITS_10 | BITS_11 | BITS_13 | BITS_14 => *tag += F::ONE,
                _ => (),
            }
            *dense += F::ONE;
            if i & 1 == 0 {
                // On even-numbered rows we recompute the spread.
                *spread = F::ZERO;
                for b in 0..16 {
                    if (i >> b) & 1 != 0 {
                        *spread += F::from(1 << (2 * b));
                    }
                }
            } else {
                // On odd-numbered rows we add one.
                *spread += F::ONE;
            }
            Some(res)
        })
    }
}
#[cfg(test)]
mod tests {
    use halo2_proofs::halo2curves::pasta::Fp;
    use halo2_proofs::{
        circuit::{Layouter, V1, Value},
        dev::MockProver,
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use rand::Rng;
    use super::{
        FIRST_TAIL_DENSE, LAST_SPREAD, SpreadTableChip, SpreadTableConfig, SpreadVar, SpreadWord,
        get_tag,
    };
    use crate::zk::kagemusha_sha256_table16_v4::{TABLE16_SPREAD_TABLE_ROWS, util::i2lebsp};
    #[derive(Clone, Copy, Debug)]
    struct LookupWitness {
        tag: u64,
        dense: u64,
        spread: u64,
        tail: u64,
    }
    impl LookupWitness {
        fn canonical(word: u16) -> Self {
            Self {
                tag: u64::from(get_tag(word)),
                dense: u64::from(word),
                spread: u64::from(interleave_u16_with_zeros(word)),
                tail: u64::from(usize::from(word) >= TABLE16_SPREAD_TABLE_ROWS),
            }
        }
    }
    #[derive(Clone, Debug)]
    struct LookupCircuit {
        rows: Vec<LookupWitness>,
    }
    impl Circuit<Fp> for LookupCircuit {
        type Config = SpreadTableConfig;
        type FloorPlanner = V1;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            self.clone()
        }
        fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
            let input_tag = meta.advice_column();
            let input_dense = meta.advice_column();
            let input_spread = meta.advice_column();
            SpreadTableChip::configure(meta, input_tag, input_dense, input_spread)
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<Fp>,
        ) -> Result<(), Error> {
            SpreadTableChip::load(config.clone(), &mut layouter)?;
            layouter.assign_region(
                || "spread lookup witnesses",
                |mut region| {
                    for (row, witness) in self.rows.iter().enumerate() {
                        region.assign_advice(
                            config.input.tag,
                            row,
                            Value::known(Fp::from(witness.tag)),
                        );
                        region.assign_advice(
                            config.input.dense,
                            row,
                            Value::known(Fp::from(witness.dense)),
                        );
                        region.assign_advice(
                            config.input.spread,
                            row,
                            Value::known(Fp::from(witness.spread)),
                        );
                        region.assign_advice(
                            config.input.tail,
                            row,
                            Value::known(Fp::from(witness.tail)),
                        );
                    }
                    Ok(())
                },
            )
        }
    }
    fn interleave_u16_with_zeros(word: u16) -> u32 {
        let mut word: u32 = word.into();
        word = (word ^ (word << 8)) & 0x00ff00ff;
        word = (word ^ (word << 4)) & 0x0f0f0f0f;
        word = (word ^ (word << 2)) & 0x33333333;
        word = (word ^ (word << 1)) & 0x55555555;
        word
    }
    #[test]
    fn lookup_table_fits_k16_and_accepts_the_constrained_tail() {
        let mut rows = [
            0,
            1,
            2,
            3,
            4,
            5,
            (1 << 4) - 1,
            1 << 4,
            (1 << 7) - 1,
            1 << 7,
            (1 << 10) - 1,
            1 << 10,
            (1 << 11) - 1,
            1 << 11,
            (1 << 13) - 1,
            1 << 13,
            (1 << 14) - 1,
            1 << 14,
            TABLE16_SPREAD_TABLE_ROWS as u16 - 1,
        ]
        .map(LookupWitness::canonical)
        .to_vec();
        rows.extend((FIRST_TAIL_DENSE as u16..=u16::MAX).map(LookupWitness::canonical));
        let mut rng = rand::rng();
        rows.extend((0..10).map(|_| LookupWitness::canonical(rng.random())));
        let circuit = LookupCircuit { rows };
        MockProver::run(16, &circuit, vec![])
            .expect("k=16 spread-table synthesis")
            .assert_satisfied();
    }
    #[test]
    fn tags_cover_every_dense_width_boundary() {
        for (below, at, below_tag, at_tag) in [
            ((1 << 4) - 1, 1 << 4, 0, 1),
            ((1 << 7) - 1, 1 << 7, 1, 2),
            ((1 << 10) - 1, 1 << 10, 2, 3),
            ((1 << 11) - 1, 1 << 11, 3, 4),
            ((1 << 13) - 1, 1 << 13, 4, 5),
            ((1 << 14) - 1, 1 << 14, 5, 6),
        ] {
            assert_eq!(get_tag(below), below_tag);
            assert_eq!(get_tag(at), at_tag);
        }
        assert_eq!(get_tag(u16::MAX), 6);
    }
    #[test]
    fn malformed_loaded_or_tail_tuple_is_rejected() {
        let canonical_last = LookupWitness::canonical(u16::MAX);
        for (name, witness) in [
            (
                "wrong loaded tag",
                LookupWitness {
                    tag: 1,
                    dense: 15,
                    spread: 0x55,
                    tail: 0,
                },
            ),
            (
                "missing tail flag",
                LookupWitness {
                    tail: 0,
                    ..canonical_last
                },
            ),
            (
                "tail flag on loaded endpoint",
                LookupWitness {
                    tail: 1,
                    ..LookupWitness::canonical(TABLE16_SPREAD_TABLE_ROWS as u16 - 1)
                },
            ),
            (
                "non-Boolean tail flag",
                LookupWitness {
                    tail: 2,
                    ..canonical_last
                },
            ),
            (
                "wrong tail tag",
                LookupWitness {
                    tag: 5,
                    ..canonical_last
                },
            ),
            (
                "wrong tail spread",
                LookupWitness {
                    spread: LAST_SPREAD - 1,
                    ..canonical_last
                },
            ),
        ] {
            let prover = MockProver::run(
                16,
                &LookupCircuit {
                    rows: vec![witness],
                },
                vec![],
            )
            .unwrap_or_else(|error| panic!("{name} synthesis failed unexpectedly: {error:?}"));
            assert!(prover.verify().is_err(), "{name} must be rejected");
        }
    }
    #[test]
    fn spread_var_assigns_the_tail_flag_for_the_top_word() {
        #[derive(Clone)]
        struct SpreadVarCircuit;
        impl Circuit<Fp> for SpreadVarCircuit {
            type Config = SpreadTableConfig;
            type FloorPlanner = V1;
            type Params = ();
            fn without_witnesses(&self) -> Self {
                self.clone()
            }
            fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
                let tag = meta.advice_column();
                let dense = meta.advice_column();
                let spread = meta.advice_column();
                SpreadTableChip::configure(meta, tag, dense, spread)
            }
            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<Fp>,
            ) -> Result<(), Error> {
                SpreadTableChip::load(config.clone(), &mut layouter)?;
                layouter.assign_region(
                    || "assign top spread word",
                    |mut region| {
                        SpreadVar::<16, 32, Fp>::with_lookup(
                            &mut region,
                            &config.input,
                            0,
                            Value::known(SpreadWord::new(i2lebsp::<16>(u64::from(u16::MAX)))),
                        )?;
                        Ok(())
                    },
                )
            }
        }
        MockProver::run(16, &SpreadVarCircuit, vec![])
            .expect("top-word tail synthesis")
            .assert_satisfied();
    }
}
