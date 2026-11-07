//! Field-side lanes: circuits over a decider's scalar field, where its field
//! messages are native (output `IVC.md` section 11, the DESIGN.md section 9.0
//! split). Glue arithmetic, running-sum limbs and the Pow5 duplex over the
//! field; the transcript mirrors [`crate::transcript::FieldTranscript`] bit for
//! bit.
//!
//! Values cross between the field side and the commitment side as canonical
//! S6 limbs (`lo < 2^128`, `hi < 2^127`): [`FieldLanes::limbs`] exports a word,
//! and [`FieldLanes::recompose`] imports one (`lo + 2^128 hi`).


use iroha_pasta::{PastaField, poseidon::PoseidonField};
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Error, Layouter, Region},
};
use iroha_plonk_gadgets::{
    GlueChip, GlueConfig, LimbBits, Pow5Columns, RoundConstantColumns, RunningSumChip, RunningSumConfig,
    UintChip, Word,
    ff::CanonicalS6,
    pow5_fq::{DuplexChip, DuplexConfig},
};

/// Columns of a field-side verifier over `F`: glue, running-sum and Pow5
/// duplex lanes.
#[derive(Clone, Debug)]
pub struct FieldVerifierConfig<F: PastaField + PoseidonField> {
    glue: GlueConfig,
    range: RunningSumConfig,
    duplex: DuplexConfig<F>,
}

impl<F: PastaField + PoseidonField> FieldVerifierConfig<F> {
    /// Configures every lane on new columns. The range table has `2^15` rows,
    /// so circuits need `k >= 16`.
    pub fn configure(meta: &mut ConstraintSystem<F>) -> Self {
        let glue_columns = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, glue_columns, constants);
        let range_column = meta.advice_column();
        let range = RunningSumConfig::configure(meta, range_column, LimbBits::new(15).expect("15 is a valid limb width"));
        let lane = Pow5Columns::allocate(meta);
        let round_constants = RoundConstantColumns::allocate(meta);
        let duplex = DuplexConfig::configure(meta, lane, round_constants, &[]);
        Self { glue, range, duplex }
    }
}

/// The chips of a field-side verifier; each keeps its own row cursor.
#[derive(Debug)]
pub struct FieldVerifierChips<F: PastaField + PoseidonField> {
    /// Glue gate (arithmetic, constants, copies).
    pub glue: GlueChip<F>,
    /// Running-sum range checks (limb decompositions).
    pub range: RunningSumChip<F>,
    /// The transcript lane.
    pub duplex: DuplexChip<F>,
}

impl<F: PastaField + PoseidonField> FieldVerifierChips<F> {
    /// Chips on `config`, every lane starting at row zero.
    #[must_use]
    pub fn new(config: &FieldVerifierConfig<F>) -> Self {
        Self {
            glue: GlueChip::new(config.glue),
            range: RunningSumChip::new(config.range),
            duplex: DuplexChip::new(config.duplex.clone()),
        }
    }

    /// Loads the running-sum table.
    ///
    /// # Errors
    /// The table does not fit the circuit.
    pub fn load_tables(&self, layouter: &mut impl Layouter<F>) -> Result<(), Error> {
        self.range.load_table(layouter)
    }

    /// The lanes, borrowed together.
    pub fn lanes(&mut self) -> FieldLanes<'_, F> {
        FieldLanes { glue: &mut self.glue, range: &mut self.range, duplex: &mut self.duplex }
    }
}

/// The three lanes a field-side verifier drives, borrowed together.
#[derive(Debug)]
pub struct FieldLanes<'a, F: PastaField + PoseidonField> {
    /// Glue gate.
    pub glue: &'a mut GlueChip<F>,
    /// Running-sum range checks.
    pub range: &'a mut RunningSumChip<F>,
    /// The transcript lane.
    pub duplex: &'a mut DuplexChip<F>,
}

impl<F: PastaField + PoseidonField> FieldLanes<'_, F> {
    /// The constant `value`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn constant(&mut self, region: &mut Region<'_, F>, value: F) -> Result<Word<F>, Error> {
        self.glue.constant(region, value)
    }

    /// `a + b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn add(&mut self, region: &mut Region<'_, F>, a: &Word<F>, b: &Word<F>) -> Result<Word<F>, Error> {
        self.glue.add(region, a, b)
    }

    /// `a - b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn sub(&mut self, region: &mut Region<'_, F>, a: &Word<F>, b: &Word<F>) -> Result<Word<F>, Error> {
        self.glue.sub(region, a, b)
    }

    /// `a b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn mul(&mut self, region: &mut Region<'_, F>, a: &Word<F>, b: &Word<F>) -> Result<Word<F>, Error> {
        self.glue.mul(region, a, b)
    }

    /// `factor a + offset`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn affine(&mut self, region: &mut Region<'_, F>, a: &Word<F>, factor: F, offset: F) -> Result<Word<F>, Error> {
        self.glue.linear(region, &[(factor, a)], offset)
    }

    /// Constrains `a = b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn assert_equal(region: &mut Region<'_, F>, a: &Word<F>, b: &Word<F>) -> Result<(), Error> {
        GlueChip::assert_equal(region, a, b)
    }

    /// The canonical S6 limbs of `word`, for export to the commitment side.
    ///
    /// # Errors
    /// Layout errors.
    pub fn limbs(&mut self, region: &mut Region<'_, F>, word: &Word<F>) -> Result<CanonicalS6<F>, Error> {
        let mut uint = UintChip::new(self.glue, self.range);
        CanonicalS6::from_native_word(&mut uint, region, word)
    }

    /// `lo + 2^128 hi` for limbs imported from the commitment side (the caller
    /// range-checks them where they enter).
    ///
    /// # Errors
    /// Layout errors.
    pub fn recompose(&mut self, region: &mut Region<'_, F>, lo: &Word<F>, hi: &Word<F>) -> Result<Word<F>, Error> {
        let two64 = F::from_u128(1 << 64);
        self.glue.linear(region, &[(F::ONE, lo), (two64 * two64, hi)], F::ZERO)
    }

    /// Starts a transcript for role `tag` under `domain` on a clear lane, as
    /// [`crate::transcript::FieldTranscript::new`].
    ///
    /// # Errors
    /// [`Error::Synthesis`] when another transcript is live on the lane.
    pub fn start(&mut self, tag: u64, domain: &[u8; 32]) -> Result<(), Error> {
        if !self.duplex.is_clear() {
            return Err(Error::Synthesis);
        }
        self.duplex.absorb_constant(F::from(tag));
        for chunk in domain.as_chunks::<16>().0 {
            self.duplex.absorb_constant(F::from_u128(u128::from_le_bytes(*chunk)));
        }
        Ok(())
    }

    /// Absorbs a count or length (a circuit constant).
    pub fn absorb_u64(&mut self, value: u64) {
        self.duplex.absorb_constant(F::from(value));
    }

    /// Absorbs words.
    pub fn absorb(&mut self, words: &[Word<F>]) {
        self.duplex.absorb_words(words);
    }

    /// A challenge; the transcript continues.
    ///
    /// # Errors
    /// Layout errors.
    pub fn challenge(&mut self, region: &mut Region<'_, F>) -> Result<Word<F>, Error> {
        self.duplex.squeeze(region)
    }

    /// The final squeeze, which clears the lane.
    ///
    /// # Errors
    /// Layout errors.
    pub fn finish(&mut self, region: &mut Region<'_, F>) -> Result<Word<F>, Error> {
        self.duplex.squeeze_and_clear(region)
    }
}
