//! In-circuit verification of folds (output `IVC.md`).
//!
//! A circuit over `C::Base` verifies folds of instances committed on `C`: the A
//! circuits (over `Fp`) fold Q instances (committed on Pallas), the Q circuits
//! (over `Fq`) fold A instances (committed on Vesta). Group operations on `C`
//! are native there ([`EccChip`]); the relation's scalars (`C::ScalarExt`) are
//! foreign and use the FF-CRT chip ([`FfChip`]); the transcripts run natively on
//! the Pow5 duplex lane, bit for bit [`crate::transcript::Transcript`].
//!
//! [`FoldVerifierChips`] bundles the chips; [`transcript`] mirrors the native
//! transcript; [`verify`] recomputes a fresh instance's challenges and digest, an
//! accumulator's digest and a fold. The functions return the folded accumulator
//! as cells: they constrain the fold, never its acceptance, exactly as the native
//! verifier ([`crate::fold::verify_fold`]), whose result only a decider accepts.

pub mod claims;
pub mod ivc;
pub mod transcript;
pub mod verify;

use ff::{Field, PrimeField};
use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Error, Layouter, Region, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, GlueConfig, LimbBits, Pow5Columns, RoundConstantColumns, RunningSumChip, RunningSumConfig,
    UintChip, Word,
    ecc::{AssignedPoint, EccChip, EccConfig},
    ff::{CanonicalS6, FfChip, FfConfig, FfValue, ForeignModulus, Nat},
    pow5_fq::{DuplexChip, DuplexConfig},
};

use crate::transcript::scalar_is_one_word;

/// The FF modulus of the scalar field of `C`.
#[must_use]
pub fn scalar_modulus<C: PastaCurve>() -> ForeignModulus {
    CanonicalS6::<C::Base>::field_modulus::<C::ScalarExt>()
}

/// Columns of a fold verifier over `C::Base`: the glue, running-sum, FF-CRT,
/// ECC and Pow5 duplex lanes, each on its own columns.
#[derive(Clone, Debug)]
pub struct FoldVerifierConfig<C: PastaCurve> {
    glue: GlueConfig,
    range: RunningSumConfig,
    ff: FfConfig,
    ecc: EccConfig<C>,
    duplex: DuplexConfig<C::Base>,
}

impl<C: PastaCurve> FoldVerifierConfig<C> {
    /// Configures every lane on new columns. The FF and range tables have
    /// `2^15` rows, so circuits need `k >= 16`.
    pub fn configure(meta: &mut ConstraintSystem<C::Base>) -> Self {
        let glue_columns = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, glue_columns, constants);
        let range_column = meta.advice_column();
        let range = RunningSumConfig::configure(
            meta,
            range_column,
            LimbBits::new(15).expect("15 is a valid limb width"),
        );
        let ff_columns = core::array::from_fn(|_| meta.advice_column());
        let ff = FfConfig::configure(meta, ff_columns, &[scalar_modulus::<C>()]);
        let ecc_columns = core::array::from_fn(|_| meta.advice_column());
        let ecc = EccConfig::configure(meta, ecc_columns);
        let lane = Pow5Columns::allocate(meta);
        let round_constants = RoundConstantColumns::allocate(meta);
        let duplex = DuplexConfig::configure(meta, lane, round_constants, &[]);
        Self { glue, range, ff, ecc, duplex }
    }
}

/// The chips of a fold verifier; each keeps its own row cursor.
#[derive(Debug)]
pub struct FoldVerifierChips<C: PastaCurve> {
    /// Glue gate (linear combinations, constants, selections).
    pub glue: GlueChip<C::Base>,
    /// Running-sum range checks.
    pub range: RunningSumChip<C::Base>,
    /// Foreign arithmetic modulo the scalar field of `C`.
    pub ff: FfChip<C::Base>,
    /// Native group operations on `C`.
    pub ecc: EccChip<C>,
    /// The transcript lane.
    pub duplex: DuplexChip<C::Base>,
}

/// A scalar of `C` in a `C::Base` circuit: its canonical S6 limbs (absorbed and
/// compared) and an FF value congruent to it (used in arithmetic).
#[derive(Clone, Debug)]
pub struct ScalarCell<C: PastaCurve> {
    /// Canonical limbs `lo < 2^128`, `hi < 2^127`, integer below the modulus.
    pub s6: CanonicalS6<C::Base>,
    /// An FF value with the same residue.
    pub ff: FfValue<C::Base>,
}

impl<C: PastaCurve> FoldVerifierChips<C> {
    /// Chips on `config`, every lane starting at row zero.
    #[must_use]
    pub fn new(config: &FoldVerifierConfig<C>) -> Self {
        Self {
            glue: GlueChip::new(config.glue),
            range: RunningSumChip::new(config.range),
            ff: FfChip::new(config.ff.clone()),
            ecc: EccChip::new(&config.ecc),
            duplex: DuplexChip::new(config.duplex.clone()),
        }
    }

    /// Loads the running-sum and FF range tables.
    ///
    /// # Errors
    /// A table does not fit the circuit.
    pub fn load_tables(&self, layouter: &mut impl Layouter<C::Base>) -> Result<(), Error> {
        self.range.load_table(layouter)?;
        self.ff.load_table(layouter)
    }

    /// The lanes, borrowed together for the fold gadgets.
    pub fn lanes(&mut self) -> FoldLanes<'_, C> {
        FoldLanes {
            glue: &mut self.glue,
            range: &mut self.range,
            ff: &mut self.ff,
            ecc: &mut self.ecc,
            duplex: &mut self.duplex,
        }
    }
}

/// The five lanes a fold verifier drives, borrowed together: from
/// [`FoldVerifierChips::lanes`], or from another chip set that owns the same
/// kinds of lanes (a PIPA-R verifier's), so that a step circuit that already
/// verifies a proof does not carry a second set of columns. The FF lane must be
/// configured for the scalar field of `C`, the duplex lane must be clear.
#[derive(Debug)]
pub struct FoldLanes<'a, C: PastaCurve> {
    /// Glue gate (linear combinations, constants, selections).
    pub glue: &'a mut GlueChip<C::Base>,
    /// Running-sum range checks.
    pub range: &'a mut RunningSumChip<C::Base>,
    /// Foreign arithmetic modulo the scalar field of `C`.
    pub ff: &'a mut FfChip<C::Base>,
    /// Native group operations on `C`.
    pub ecc: &'a mut EccChip<C>,
    /// The transcript lane.
    pub duplex: &'a mut DuplexChip<C::Base>,
}

impl<C: PastaCurve> FoldLanes<'_, C> {
    /// The integer lane over the glue and running-sum chips.
    pub fn uint(&mut self) -> UintChip<'_, C::Base> {
        UintChip::new(self.glue, self.range)
    }

    /// A private scalar with its canonical certificate.
    ///
    /// # Errors
    /// Layout errors.
    pub fn assign_scalar(
        &mut self,
        region: &mut Region<'_, C::Base>,
        value: Value<C::ScalarExt>,
    ) -> Result<ScalarCell<C>, Error> {
        let mut uint = UintChip::new(self.glue, self.range);
        let s6 = CanonicalS6::assign_value::<C::ScalarExt>(&mut uint, region, value)?;
        self.import(region, s6)
    }

    /// Imports canonical limbs into the FF lane.
    ///
    /// # Errors
    /// Layout errors or limbs of another modulus.
    pub fn import(&mut self, region: &mut Region<'_, C::Base>, s6: CanonicalS6<C::Base>) -> Result<ScalarCell<C>, Error> {
        if s6.modulus() != scalar_modulus::<C>() {
            return Err(Error::Synthesis);
        }
        let mut uint = UintChip::new(self.glue, self.range);
        let ff = self.ff.import_s6(&mut uint, region, &s6)?;
        Ok(ScalarCell { s6, ff })
    }

    /// Canonicalizes an FF result into a scalar cell.
    ///
    /// # Errors
    /// Layout errors.
    pub fn export(&mut self, region: &mut Region<'_, C::Base>, ff: FfValue<C::Base>) -> Result<ScalarCell<C>, Error> {
        let mut uint = UintChip::new(self.glue, self.range);
        let s6 = self.ff.export_s6(&mut uint, region, &ff)?;
        Ok(ScalarCell { s6, ff })
    }

    /// The FF constant `value`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn constant(&mut self, region: &mut Region<'_, C::Base>, value: C::ScalarExt) -> Result<FfValue<C::Base>, Error> {
        self.ff.constant(
            self.glue,
            region,
            scalar_modulus::<C>(),
            &Nat::from_words(value.to_canonical_limbs()),
        )
    }

    /// `a + b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn add(&mut self, region: &mut Region<'_, C::Base>, a: &FfValue<C::Base>, b: &FfValue<C::Base>) -> Result<FfValue<C::Base>, Error> {
        self.ff.add(self.glue, region, a, b)
    }

    /// `a - b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn sub(&mut self, region: &mut Region<'_, C::Base>, a: &FfValue<C::Base>, b: &FfValue<C::Base>) -> Result<FfValue<C::Base>, Error> {
        self.ff.sub(self.glue, region, a, b)
    }

    /// `a b`.
    ///
    /// # Errors
    /// Layout errors.
    pub fn mul(&mut self, region: &mut Region<'_, C::Base>, a: &FfValue<C::Base>, b: &FfValue<C::Base>) -> Result<FfValue<C::Base>, Error> {
        self.ff.mul(region, a, b)
    }

    /// `a + x (b - a)`, the linear fold of one scalar.
    ///
    /// # Errors
    /// Layout errors.
    pub fn lerp(
        &mut self,
        region: &mut Region<'_, C::Base>,
        a: &FfValue<C::Base>,
        b: &FfValue<C::Base>,
        x: &FfValue<C::Base>,
    ) -> Result<FfValue<C::Base>, Error> {
        let difference = self.sub(region, b, a)?;
        let scaled = self.mul(region, x, &difference)?;
        self.add(region, a, &scaled)
    }

    /// The transcript words of a scalar: both S6 limbs for a Pallas relation,
    /// their one-word recomposition for a Vesta relation (`Fp < Fq`).
    ///
    /// # Errors
    /// Layout errors.
    pub fn scalar_words(&mut self, region: &mut Region<'_, C::Base>, scalar: &ScalarCell<C>) -> Result<Vec<Word<C::Base>>, Error> {
        let (lo, hi) = (scalar.s6.lo().word(), scalar.s6.hi().word());
        if scalar_is_one_word::<C>() {
            let two128 = C::Base::from_u128(1 << 64).square();
            Ok(vec![self.glue.linear(region, &[(C::Base::ONE, lo), (two128, hi)], C::Base::ZERO)?])
        } else {
            Ok(vec![lo.clone(), hi.clone()])
        }
    }

    /// A private point of `C` (on the curve, or the identity `(0, 0)`).
    ///
    /// # Errors
    /// Layout errors.
    pub fn assign_point(&mut self, region: &mut Region<'_, C::Base>, value: Value<C>) -> Result<AssignedPoint<C::Base>, Error> {
        self.ecc.witness_point(region, value)
    }
}
