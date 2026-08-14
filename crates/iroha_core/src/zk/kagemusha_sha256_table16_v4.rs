//! A compact, dual-field SHA-256 chip for Kagemusha recursion.
//!
//! The table layout originated in `zcash/halo2` commit
//! `d5e9ee8ed8072e5155acfa230a97ecb82f173115` and was subsequently hardened
//! and generalized by MIDNIGHT-ZK commit
//! `e235a4592f3aaba2fd9ce2bf93ebdbf7c7a6576b`. Both sources are available
//! under Apache-2.0. This port retains the source structure to make review
//! against those implementations straightforward.
//!
//! Unlike the original Zcash prototype, circuit boundaries use assigned cells,
//! not host-only [`Value`]s. Message words, chaining state, constants, and the
//! final digest therefore remain connected by permutation constraints.
use ff::PrimeField;
use halo2_proofs::{
    circuit::{Cell, Chip, Layouter, Region, Value},
    plonk::{Advice, Any, Assigned, Column, Error},
};
use std::{convert::TryInto, fmt::Debug, marker::PhantomData};
mod table16;
pub(crate) use table16::util;
#[allow(unused_imports)]
pub use table16::{Table16Chip, Table16Config};
/// Number of canonical spread-table rows that fit at `k = 16`.
///
/// Halo2 reserves nine rows for blinding and argument bookkeeping, so the
/// final nine 16-bit words are handled by the constrained tail relation.
pub(crate) const TABLE16_SPREAD_TABLE_ROWS: usize = (1 << 16) - 9;
/// Number of 32-bit words in one SHA-256 block.
pub const BLOCK_SIZE: usize = 16;
/// Number of bytes in one SHA-256 block.
pub const BLOCK_BYTE_SIZE: usize = 64;
/// Number of 32-bit words in a SHA-256 digest.
pub const DIGEST_SIZE: usize = 8;
/// Number of bytes in one SHA-256 word.
#[cfg(test)]
pub const BYTES_PER_WORD: usize = 4;
/// Number of bits in one byte.
pub const BITS_PER_BYTE: usize = 8;
pub(crate) const ROUNDS: usize = 64;
const STATE: usize = 8;
#[allow(clippy::unreadable_literal)]
pub(crate) const ROUND_CONSTANTS: [u32; ROUNDS] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
pub(crate) const IV: [u32; STATE] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct BlockWord(pub(crate) Value<u32>);
/// A source byte whose cell is already constrained to the 8-bit range.
///
/// Kagemusha constructs these from the range-checked Base-circuit message
/// bytes. The Table16 packer copy-binds this cell into its raw region.
#[derive(Clone, Debug)]
pub(crate) struct AssignedByte<F: PrimeField> {
    value: Value<u8>,
    cell: Cell,
    _marker: PhantomData<F>,
}
impl<F: PrimeField> AssignedByte<F> {
    pub(crate) fn from_range_checked_cell(value: Value<u8>, cell: Cell) -> Self {
        Self {
            value,
            cell,
            _marker: PhantomData,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum PaddedByte<F: PrimeField> {
    Source(AssignedByte<F>),
    Constant(u8),
}
impl<F: PrimeField> PaddedByte<F> {
    fn value(&self) -> Value<u8> {
        match self {
            Self::Source(byte) => byte.value,
            Self::Constant(byte) => Value::known(*byte),
        }
    }
}
/// Returns the canonical SHA-256 padding suffix for a message of `message_len`
/// bytes.
///
/// The suffix always contains the `0x80` marker, enough zero bytes to place the
/// length in the final eight bytes of a block, and the complete 64-bit
/// big-endian bit length. Lengths that SHA-256 cannot encode are rejected
/// instead of being truncated modulo `2^64`.
pub(crate) fn canonical_padding_suffix(message_len: usize) -> Option<Vec<u8>> {
    let message_len = u64::try_from(message_len).ok()?;
    let bit_len = message_len.checked_mul(BITS_PER_BYTE as u64)?;
    let remainder = usize::try_from(message_len % BLOCK_BYTE_SIZE as u64).ok()?;
    let zero_count = if remainder < BLOCK_BYTE_SIZE - 8 {
        BLOCK_BYTE_SIZE - 9 - remainder
    } else {
        2 * BLOCK_BYTE_SIZE - 9 - remainder
    };
    let mut suffix = Vec::with_capacity(1 + zero_count + 8);
    suffix.push(0x80);
    suffix.resize(1 + zero_count, 0);
    suffix.extend_from_slice(&bit_len.to_be_bytes());
    debug_assert_eq!((remainder + suffix.len()) % BLOCK_BYTE_SIZE, 0);
    Some(suffix)
}
#[derive(Clone, Debug)]
pub(crate) struct Bits<const LEN: usize>(pub(crate) [bool; LEN]);
impl<const LEN: usize> Bits<LEN> {
    fn spread<const SPREAD: usize>(&self) -> [bool; SPREAD] {
        table16::util::spread_bits(self.0)
    }
}
impl<const LEN: usize> std::ops::Deref for Bits<LEN> {
    type Target = [bool; LEN];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<const LEN: usize> From<[bool; LEN]> for Bits<LEN> {
    fn from(bits: [bool; LEN]) -> Self {
        Self(bits)
    }
}
impl<const LEN: usize> From<&Bits<LEN>> for [bool; LEN] {
    fn from(bits: &Bits<LEN>) -> Self {
        bits.0
    }
}
impl<const LEN: usize> From<Bits<LEN>> for [bool; LEN] {
    fn from(bits: Bits<LEN>) -> Self {
        bits.0
    }
}
impl<const LEN: usize, F: PrimeField> From<&Bits<LEN>> for Assigned<F> {
    fn from(bits: &Bits<LEN>) -> Self {
        assert!(LEN <= 64);
        F::from(table16::util::lebs2ip(&bits.0)).into()
    }
}
impl From<&Bits<16>> for u16 {
    fn from(bits: &Bits<16>) -> Self {
        table16::util::lebs2ip(&bits.0) as u16
    }
}
impl From<u16> for Bits<16> {
    fn from(value: u16) -> Self {
        Self(table16::util::i2lebsp(value.into()))
    }
}
impl From<&Bits<32>> for u32 {
    fn from(bits: &Bits<32>) -> Self {
        table16::util::lebs2ip(&bits.0) as u32
    }
}
impl From<u32> for Bits<32> {
    fn from(value: u32) -> Self {
        Self(table16::util::i2lebsp(value.into()))
    }
}
/// An assigned, bit-length-typed word.
///
/// Axiom Halo2's low-level assigner erases the typed witness from returned
/// cells. Keeping the typed [`Value`] beside the cell preserves the reviewable
/// API of the source gadget without weakening the permutation constraint.
#[derive(Clone, Debug)]
pub(crate) struct AssignedBits<const LEN: usize, F: PrimeField> {
    value: Value<Bits<LEN>>,
    cell: Cell,
    _marker: PhantomData<F>,
}
impl<const LEN: usize, F: PrimeField> AssignedBits<LEN, F> {
    pub(crate) fn value(&self) -> Value<&Bits<LEN>> {
        self.value.as_ref()
    }
    pub(crate) fn cell(&self) -> Cell {
        self.cell
    }
    fn copy_advice<A, AR>(
        &self,
        annotation: A,
        region: &mut Region<'_, F>,
        column: Column<Advice>,
        offset: usize,
    ) -> Result<Self, Error>
    where
        A: Fn() -> AR,
        AR: Into<String>,
    {
        let _ = annotation;
        let assigned =
            region.assign_advice(column, offset, self.value.as_ref().map(Assigned::<F>::from));
        region.constrain_equal(assigned.cell(), self.cell);
        Ok(Self {
            value: self.value.clone(),
            cell: assigned.cell(),
            _marker: PhantomData,
        })
    }
    pub(crate) fn assign_bits<A, AR, T>(
        region: &mut Region<'_, F>,
        annotation: A,
        column: impl Into<Column<Any>>,
        offset: usize,
        value: Value<T>,
    ) -> Result<Self, Error>
    where
        A: Fn() -> AR,
        AR: Into<String>,
        T: TryInto<[bool; LEN]> + Debug + Clone,
        T::Error: Debug,
    {
        let value = value.map(|word| Bits::from(word.try_into().expect("bit length checked")));
        let _ = annotation;
        let advice = Column::<Advice>::try_from(column.into())
            .expect("SHA-256 bit assignments require advice columns");
        let assigned =
            region.assign_advice(advice, offset, value.as_ref().map(Assigned::<F>::from));
        Ok(Self {
            value,
            cell: assigned.cell(),
            _marker: PhantomData,
        })
    }
    pub(crate) fn assign_bits_fixed<A, AR, T>(
        region: &mut Region<'_, F>,
        annotation: A,
        column: impl Into<Column<Any>>,
        offset: usize,
        value: T,
    ) -> Result<Self, Error>
    where
        A: Fn() -> AR,
        AR: Into<String>,
        T: TryInto<[bool; LEN]> + Debug + Clone,
        T::Error: Debug,
    {
        let value = Bits::from(value.try_into().expect("bit length checked"));
        let advice = Column::<Advice>::try_from(column.into())
            .expect("SHA-256 constants are copied into advice columns");
        let assigned =
            region.assign_advice_from_constant(annotation, advice, offset, value.clone())?;
        Ok(Self {
            value: Value::known(value),
            cell: assigned.cell(),
            _marker: PhantomData,
        })
    }
}
impl<F: PrimeField> AssignedBits<16, F> {
    pub(crate) fn value_u16(&self) -> Value<u16> {
        self.value().map(Into::into)
    }
    pub(crate) fn assign<A, AR>(
        region: &mut Region<'_, F>,
        annotation: A,
        column: impl Into<Column<Any>>,
        offset: usize,
        value: Value<u16>,
    ) -> Result<Self, Error>
    where
        A: Fn() -> AR,
        AR: Into<String>,
    {
        Self::assign_bits(
            region,
            annotation,
            column,
            offset,
            value.map(Bits::<16>::from),
        )
    }
}
impl<F: PrimeField> AssignedBits<32, F> {
    pub(crate) fn value_u32(&self) -> Value<u32> {
        self.value().map(Into::into)
    }
    pub(crate) fn assign<A, AR>(
        region: &mut Region<'_, F>,
        annotation: A,
        column: impl Into<Column<Any>>,
        offset: usize,
        value: Value<u32>,
    ) -> Result<Self, Error>
    where
        A: Fn() -> AR,
        AR: Into<String>,
    {
        Self::assign_bits(
            region,
            annotation,
            column,
            offset,
            value.map(Bits::<32>::from),
        )
    }
}
pub(crate) type AssignedBlockWord<F> = AssignedBits<32, F>;
pub(crate) type AssignedWord<F> = AssignedBits<32, F>;
/// Low-level instructions implemented by the Table16 chip.
pub(crate) trait Sha256Instructions<F: PrimeField>: Chip<F> + Clone + Debug {
    type State: Clone + Debug;
    fn initialization_vector(&self, layouter: &mut impl Layouter<F>) -> Result<Self::State, Error>;
    fn compress(
        &self,
        layouter: &mut impl Layouter<F>,
        chaining_state: &Self::State,
        input: [AssignedBlockWord<F>; BLOCK_SIZE],
    ) -> Result<Self::State, Error>;
    fn digest(
        &self,
        layouter: &mut impl Layouter<F>,
        state: &Self::State,
    ) -> Result<[AssignedBlockWord<F>; DIGEST_SIZE], Error>;
}
#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{
        circuit::{Layouter, V1, Value},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error},
    };
    use sha2::{Digest as _, Sha256};
    #[derive(Clone, Debug)]
    struct TestConfig {
        sha: Table16Config,
        input: Column<Advice>,
    }
    #[derive(Clone, Debug)]
    struct VectorCircuit {
        vectors: Vec<(Vec<u8>, [u8; 32])>,
        tamper_first_source_value: bool,
    }
    impl VectorCircuit {
        fn fips_vectors() -> Self {
            let messages = vec![
                Vec::new(),
                b"abc".to_vec(),
                vec![0xa5; 55],
                vec![0x3c; 56],
                vec![0x81; 63],
                vec![0x42; 64],
                vec![0x7e; 65],
                (0..=200).map(|index| index as u8).collect(),
            ];
            let vectors = messages
                .into_iter()
                .map(|message| {
                    let expected: [u8; 32] = Sha256::digest(&message).into();
                    (message, expected)
                })
                .collect();
            Self {
                vectors,
                tamper_first_source_value: false,
            }
        }
    }
    impl<F: PrimeField> Circuit<F> for VectorCircuit {
        type Config = TestConfig;
        type FloorPlanner = V1;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            self.clone()
        }
        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let input = meta.advice_column();
            meta.enable_equality(input);
            TestConfig {
                sha: Table16Chip::<F>::configure(meta),
                input,
            }
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            Table16Chip::<F>::load(config.sha.clone(), &mut layouter)?;
            let chip = Table16Chip::<F>::construct(config.sha);
            for (vector_index, (message, expected)) in self.vectors.iter().enumerate() {
                let assigned_input = layouter.assign_region(
                    || format!("range-checked input bytes for vector {vector_index}"),
                    |mut region| {
                        message
                            .iter()
                            .enumerate()
                            .map(|(offset, byte)| {
                                let assigned = region.assign_advice_from_constant(
                                    || format!("input byte {offset}"),
                                    config.input,
                                    offset,
                                    F::from(u64::from(*byte)),
                                )?;
                                let witness = if self.tamper_first_source_value
                                    && vector_index == 1
                                    && offset == 0
                                {
                                    byte ^ 1
                                } else {
                                    *byte
                                };
                                Ok(AssignedByte::from_range_checked_cell(
                                    Value::known(witness),
                                    assigned.cell(),
                                ))
                            })
                            .collect::<Result<Vec<_>, Error>>()
                    },
                )?;
                let blocks = chip.canonical_blocks(&mut layouter, &assigned_input)?;
                let mut state = chip.initialization_vector(&mut layouter)?;
                for block in blocks {
                    state = chip.compress(&mut layouter, &state, block)?;
                }
                let digest = chip.digest(&mut layouter, &state)?;
                let expected_words = expected
                    .chunks_exact(BYTES_PER_WORD)
                    .map(|word| u32::from_be_bytes(word.try_into().expect("digest word")))
                    .collect::<Vec<_>>();
                layouter.assign_region(
                    || format!("bind vector {vector_index} digest"),
                    |mut region| {
                        for (assigned, expected) in digest.iter().zip(expected_words) {
                            region.constrain_constant(assigned.cell(), F::from(expected as u64))?;
                        }
                        Ok(())
                    },
                )?;
            }
            Ok(())
        }
    }
    fn assert_fips_vectors<F: PrimeField + ff::FromUniformBytes<64> + Ord>() {
        let circuit = VectorCircuit::fips_vectors();
        MockProver::<F>::run(17, &circuit, vec![])
            .expect("Table16 synthesis")
            .assert_satisfied();
    }
    #[test]
    fn fips_vectors_fp() {
        assert_fips_vectors::<Fp>();
    }
    #[test]
    fn fips_vectors_fq() {
        assert_fips_vectors::<Fq>();
    }
    #[test]
    fn padding_uses_full_64_bit_length() {
        let max_len = usize::try_from(u64::MAX / BITS_PER_BYTE as u64)
            .expect("64-bit target can represent the SHA-256 maximum length");
        let suffix = canonical_padding_suffix(max_len).expect("maximum encodable length");
        assert_eq!(&suffix[suffix.len() - 8..], &(u64::MAX - 7).to_be_bytes());
        assert_eq!(canonical_padding_suffix(max_len + 1), None);
    }
    #[test]
    fn padding_boundaries_are_canonical() {
        for (message_len, suffix_len) in [
            (0, 64),
            (1, 63),
            (55, 9),
            (56, 72),
            (63, 65),
            (64, 64),
            (65, 63),
            (77, 51),
            (794, 38),
            (31_734, 10),
        ] {
            let suffix = canonical_padding_suffix(message_len).expect("encodable test length");
            assert_eq!(suffix.len(), suffix_len);
            assert_eq!(suffix[0], 0x80);
            assert!(suffix[1..suffix.len() - 8].iter().all(|byte| *byte == 0));
            assert_eq!(
                &suffix[suffix.len() - 8..],
                &(u64::try_from(message_len).unwrap() * 8).to_be_bytes()
            );
            assert_eq!((message_len + suffix.len()) % BLOCK_BYTE_SIZE, 0);
        }
    }
    #[test]
    fn source_value_is_copy_constrained() {
        let mut circuit = VectorCircuit::fips_vectors();
        circuit.tamper_first_source_value = true;
        let prover = MockProver::<Fp>::run(17, &circuit, vec![]).expect("Table16 synthesis");
        assert!(prover.verify().is_err());
    }
}
