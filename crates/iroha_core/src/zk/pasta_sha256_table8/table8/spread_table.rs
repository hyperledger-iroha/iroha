// Derived from MIDNIGHT-ZK under Apache-2.0.
// Copyright (C) 2025 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Exact 16-bit dense/spread checks backed by one complete eight-bit table.
//!
//! A witness row carries the original (tag, dense16, spread32) tuple plus the
//! low byte, its spread form and tag, and a Boolean high-byte-nonzero flag.
//! Two lookups prove the complete relation:
//!
//! - the low lookup proves (lo, spread(lo), tag(lo));
//! - the high lookup proves (hi, spread(hi), high_tag, hi != 0).
//!
//! The high byte and high spread are linear expressions
//! (dense - lo) / 2^8 and (spread - spread(lo)) / 2^16. If the high byte is
//! zero, high_tag = tag - tag(lo), forcing the complete tag to equal the low
//! tag. Otherwise high_tag = tag and the high table column contains
//! tag(hi << 8), which is exactly the tag of the complete 16-bit word.

use crate::zk::pasta_sha256_table8::{
    AssignedBits, TABLE8_SPREAD_TABLE_ROWS,
    util::{i2lebsp, lebs2ip, spread_bits},
};
use ff::{Field, PrimeField};
use halo2_proofs::{
    circuit::{Chip, Layouter, Region, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, TableColumn},
    poly::Rotation,
};
use std::{convert::TryInto, marker::PhantomData};

const BITS_4: usize = 1 << 4;
const BITS_7: usize = 1 << 7;
const BITS_10: usize = 1 << 10;
const BITS_11: usize = 1 << 11;
const BITS_13: usize = 1 << 13;
const BITS_14: usize = 1 << 14;

/// An input word into a lookup, containing (tag, dense, spread).
#[derive(Copy, Clone, Debug)]
pub(super) struct SpreadWord<const DENSE: usize, const SPREAD: usize> {
    pub tag: u8,
    pub dense: [bool; DENSE],
    pub spread: [bool; SPREAD],
}

/// Return the bit-width tag used by the SHA schedule and compression gates.
pub fn get_tag(input: u16) -> u8 {
    let input = usize::from(input);
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

fn spread_byte(byte: u8) -> u16 {
    let dense = i2lebsp::<8>(u64::from(byte));
    lebs2ip(&spread_bits::<8, 16>(dense)) as u16
}

impl<const DENSE: usize, const SPREAD: usize> SpreadWord<DENSE, SPREAD> {
    pub(super) fn new(dense: [bool; DENSE]) -> Self {
        assert!(DENSE <= 16);
        Self {
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
        Self::new(dense.try_into().unwrap())
    }
}

/// A variable stored in advice columns corresponding to one exact spread row.
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
        let low_dense = word.map(|word| lebs2ip(&word.dense) as u8);
        let low_spread = low_dense.map(spread_byte);
        let low_tag = low_dense.map(|byte| get_tag(u16::from(byte)));
        let high_nonzero = word.map(|word| u8::from((lebs2ip(&word.dense) >> 8) != 0));

        region.assign_advice(cols.tag, row, tag.map(|tag| F::from(u64::from(tag))));
        region.assign_advice(
            cols.low_dense,
            row,
            low_dense.map(|byte| F::from(u64::from(byte))),
        );
        region.assign_advice(
            cols.low_spread,
            row,
            low_spread.map(|spread| F::from(u64::from(spread))),
        );
        region.assign_advice(
            cols.low_tag,
            row,
            low_tag.map(|tag| F::from(u64::from(tag))),
        );
        region.assign_advice(
            cols.high_nonzero,
            row,
            high_nonzero.map(|bit| F::from(u64::from(bit))),
        );
        let dense =
            AssignedBits::<DENSE, F>::assign_bits(region, || "dense", cols.dense, row, dense_val)?;
        let spread = AssignedBits::<SPREAD, F>::assign_bits(
            region,
            || "spread",
            cols.spread,
            row,
            spread_val,
        )?;
        Ok(Self { dense, spread })
    }

    pub(super) fn without_lookup_fixed(
        region: &mut Region<'_, F>,
        dense_col: Column<Advice>,
        dense_row: usize,
        spread_col: Column<Advice>,
        spread_row: usize,
        word: SpreadWord<DENSE, SPREAD>,
    ) -> Result<Self, Error> {
        let dense = AssignedBits::<DENSE, F>::assign_bits_fixed(
            region,
            || "dense",
            dense_col,
            dense_row,
            word.dense,
        )?;
        let spread = AssignedBits::<SPREAD, F>::assign_bits_fixed(
            region,
            || "spread",
            spread_col,
            spread_row,
            word.spread,
        )?;
        Ok(Self { dense, spread })
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
        Ok(Self { dense, spread })
    }
}

#[derive(Clone, Debug)]
pub(super) struct SpreadInputs {
    pub(super) tag: Column<Advice>,
    pub(super) dense: Column<Advice>,
    pub(super) spread: Column<Advice>,
    pub(super) low_dense: Column<Advice>,
    pub(super) low_spread: Column<Advice>,
    pub(super) low_tag: Column<Advice>,
    pub(super) high_nonzero: Column<Advice>,
}

#[derive(Clone, Debug)]
pub(super) struct SpreadTable {
    pub(super) byte: TableColumn,
    pub(super) spread: TableColumn,
    pub(super) low_tag: TableColumn,
    pub(super) high_tag: TableColumn,
    pub(super) nonzero: TableColumn,
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
            byte: meta.lookup_table_column(),
            spread: meta.lookup_table_column(),
            low_tag: meta.lookup_table_column(),
            high_tag: meta.lookup_table_column(),
            nonzero: meta.lookup_table_column(),
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
        let low_dense = meta.advice_column();
        let low_spread = meta.advice_column();
        let low_tag = meta.advice_column();
        let high_nonzero = meta.advice_column();
        Self::configure_with_table(
            meta,
            input_tag,
            input_dense,
            input_spread,
            low_dense,
            low_spread,
            low_tag,
            high_nonzero,
            table,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn configure_with_table(
        meta: &mut ConstraintSystem<F>,
        input_tag: Column<Advice>,
        input_dense: Column<Advice>,
        input_spread: Column<Advice>,
        low_dense: Column<Advice>,
        low_spread: Column<Advice>,
        low_tag: Column<Advice>,
        high_nonzero: Column<Advice>,
        table: SpreadTable,
    ) -> <Self as Chip<F>>::Config {
        meta.lookup("Table8 SHA-256 low byte", |meta| {
            let low_dense = meta.query_advice(low_dense, Rotation::cur());
            let low_spread = meta.query_advice(low_spread, Rotation::cur());
            let low_tag = meta.query_advice(low_tag, Rotation::cur());
            vec![
                (low_dense, table.byte),
                (low_spread, table.spread),
                (low_tag, table.low_tag),
            ]
        });

        meta.lookup("Table8 SHA-256 high byte and complete tag", |meta| {
            let tag = meta.query_advice(input_tag, Rotation::cur());
            let dense = meta.query_advice(input_dense, Rotation::cur());
            let spread = meta.query_advice(input_spread, Rotation::cur());
            let low_dense = meta.query_advice(low_dense, Rotation::cur());
            let low_spread = meta.query_advice(low_spread, Rotation::cur());
            let low_tag = meta.query_advice(low_tag, Rotation::cur());
            let high_nonzero = meta.query_advice(high_nonzero, Rotation::cur());
            let one = Expression::Constant(F::ONE);
            let inv_two_8 = Expression::Constant(F::from(1_u64 << 8).invert().unwrap());
            let inv_two_16 = Expression::Constant(F::from(1_u64 << 16).invert().unwrap());
            let high_dense = (dense - low_dense) * inv_two_8;
            let high_spread = (spread - low_spread) * inv_two_16;
            let high_tag = tag - (one - high_nonzero.clone()) * low_tag;
            vec![
                (high_dense, table.byte),
                (high_spread, table.spread),
                (high_tag, table.high_tag),
                (high_nonzero, table.nonzero),
            ]
        });

        SpreadTableConfig {
            input: SpreadInputs {
                tag: input_tag,
                dense: input_dense,
                spread: input_spread,
                low_dense,
                low_spread,
                low_tag,
                high_nonzero,
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
            || "complete 8-bit SHA-256 spread table",
            |mut table| {
                for (index, (byte, spread, low_tag, high_tag, nonzero)) in
                    SpreadTableConfig::generate::<F>().enumerate()
                {
                    table.assign_cell(|| "byte", config.byte, index, || Value::known(byte))?;
                    table.assign_cell(
                        || "spread",
                        config.spread,
                        index,
                        || Value::known(spread),
                    )?;
                    table.assign_cell(
                        || "low-byte tag",
                        config.low_tag,
                        index,
                        || Value::known(low_tag),
                    )?;
                    table.assign_cell(
                        || "high-byte tag",
                        config.high_tag,
                        index,
                        || Value::known(high_tag),
                    )?;
                    table.assign_cell(
                        || "byte is nonzero",
                        config.nonzero,
                        index,
                        || Value::known(nonzero),
                    )?;
                }
                Ok(())
            },
        )
    }
}

impl SpreadTableConfig {
    fn generate<F: PrimeField>() -> impl Iterator<Item = (F, F, F, F, F)> {
        (0_u16..TABLE8_SPREAD_TABLE_ROWS as u16).map(|byte| {
            (
                F::from(u64::from(byte)),
                F::from(u64::from(spread_byte(byte as u8))),
                F::from(u64::from(get_tag(byte))),
                F::from(u64::from(get_tag(byte << 8))),
                F::from(u64::from(u8::from(byte != 0))),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SpreadTableChip, SpreadTableConfig, SpreadVar, SpreadWord, get_tag, spread_byte};
    use crate::zk::pasta_sha256_table8::util::{i2lebsp, lebs2ip, spread_bits};
    use ff::PrimeField;
    use halo2_proofs::{
        circuit::{Layouter, V1, Value},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use std::marker::PhantomData;

    #[derive(Clone, Copy, Debug)]
    struct LookupWitness {
        tag: u64,
        dense: u64,
        spread: u64,
        low_dense: u64,
        low_spread: u64,
        low_tag: u64,
        high_nonzero: u64,
    }

    impl LookupWitness {
        fn canonical(word: u16) -> Self {
            let low_dense = word as u8;
            Self {
                tag: u64::from(get_tag(word)),
                dense: u64::from(word),
                spread: lebs2ip(&spread_bits::<16, 32>(i2lebsp::<16>(u64::from(word)))),
                low_dense: u64::from(low_dense),
                low_spread: u64::from(spread_byte(low_dense)),
                low_tag: u64::from(get_tag(u16::from(low_dense))),
                high_nonzero: u64::from(u8::from((word >> 8) != 0)),
            }
        }
    }

    #[derive(Clone, Debug)]
    struct LookupCircuit<F: PrimeField> {
        rows: Vec<LookupWitness>,
        marker: PhantomData<F>,
    }

    impl<F: PrimeField> Circuit<F> for LookupCircuit<F> {
        type Config = SpreadTableConfig;
        type FloorPlanner = V1;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            self.clone()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let input_tag = meta.advice_column();
            let input_dense = meta.advice_column();
            let input_spread = meta.advice_column();
            SpreadTableChip::configure(meta, input_tag, input_dense, input_spread)
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            SpreadTableChip::load(config.clone(), &mut layouter)?;
            layouter.assign_region(
                || "Table8 lookup witnesses",
                |mut region| {
                    for (row, witness) in self.rows.iter().enumerate() {
                        for (column, value) in [
                            (config.input.tag, witness.tag),
                            (config.input.dense, witness.dense),
                            (config.input.spread, witness.spread),
                            (config.input.low_dense, witness.low_dense),
                            (config.input.low_spread, witness.low_spread),
                            (config.input.low_tag, witness.low_tag),
                            (config.input.high_nonzero, witness.high_nonzero),
                        ] {
                            region.assign_advice(column, row, Value::known(F::from(value)));
                        }
                    }
                    Ok(())
                },
            )
        }
    }

    fn representative_words() -> Vec<LookupWitness> {
        [
            0, 1, 15, 16, 127, 128, 255, 256, 257, 1023, 1024, 2047, 2048, 8191, 8192, 16383,
            16384, 0xff00, 0xffff,
        ]
        .map(LookupWitness::canonical)
        .to_vec()
    }

    fn assert_representatives<F>()
    where
        F: PrimeField + ff::FromUniformBytes<64> + Ord,
    {
        let circuit = LookupCircuit::<F> {
            rows: representative_words(),
            marker: PhantomData,
        };
        MockProver::run(9, &circuit, vec![])
            .expect("Table8 lookup synthesis")
            .assert_satisfied();
    }

    #[test]
    fn exact_lookup_accepts_both_pasta_parities() {
        assert_representatives::<Fp>();
        assert_representatives::<Fq>();
    }

    #[test]
    fn two_byte_identity_is_exhaustive_over_u16() {
        for word in 0_u16..=u16::MAX {
            let witness = LookupWitness::canonical(word);
            let high = u64::from(word >> 8);
            assert_eq!(witness.dense, witness.low_dense + (high << 8));
            assert_eq!(
                witness.spread,
                witness.low_spread + (u64::from(spread_byte(high as u8)) << 16)
            );
            let reconstructed_tag = if high == 0 {
                witness.low_tag
            } else {
                u64::from(get_tag((high as u16) << 8))
            };
            assert_eq!(witness.tag, reconstructed_tag);
        }
    }

    #[test]
    fn malformed_decomposition_or_relation_is_rejected() {
        let canonical = LookupWitness::canonical(0xabcd);
        let malformed = [
            LookupWitness {
                low_dense: canonical.low_dense ^ 1,
                ..canonical
            },
            LookupWitness {
                low_spread: canonical.low_spread ^ 1,
                ..canonical
            },
            LookupWitness {
                low_tag: canonical.low_tag ^ 1,
                ..canonical
            },
            LookupWitness {
                high_nonzero: 0,
                ..canonical
            },
            LookupWitness {
                tag: canonical.tag ^ 1,
                ..canonical
            },
            LookupWitness {
                dense: canonical.dense ^ (1 << 8),
                ..canonical
            },
            LookupWitness {
                spread: canonical.spread ^ (1 << 16),
                ..canonical
            },
        ];
        for witness in malformed {
            let circuit = LookupCircuit::<Fp> {
                rows: vec![witness],
                marker: PhantomData,
            };
            let prover =
                MockProver::run(9, &circuit, vec![]).expect("malformed Table8 lookup synthesis");
            assert!(prover.verify().is_err());
        }
    }

    #[test]
    fn spread_var_populates_exact_auxiliary_witnesses() {
        #[derive(Clone)]
        struct SpreadVarCircuit<F>(PhantomData<F>);

        impl<F: PrimeField> Circuit<F> for SpreadVarCircuit<F> {
            type Config = SpreadTableConfig;
            type FloorPlanner = V1;
            type Params = ();

            fn without_witnesses(&self) -> Self {
                self.clone()
            }

            fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
                let tag = meta.advice_column();
                let dense = meta.advice_column();
                let spread = meta.advice_column();
                SpreadTableChip::configure(meta, tag, dense, spread)
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<F>,
            ) -> Result<(), Error> {
                SpreadTableChip::load(config.clone(), &mut layouter)?;
                layouter.assign_region(
                    || "assign complete 16-bit spread word",
                    |mut region| {
                        SpreadVar::<16, 32, F>::with_lookup(
                            &mut region,
                            &config.input,
                            0,
                            Value::known(SpreadWord::new(i2lebsp::<16>(0xffff))),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        MockProver::run(9, &SpreadVarCircuit::<Fp>(PhantomData), vec![])
            .expect("Table8 spread-var synthesis")
            .assert_satisfied();
    }
}
