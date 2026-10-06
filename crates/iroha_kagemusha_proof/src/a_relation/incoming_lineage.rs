//! Total incoming lineage prefix with an exact original digest and safe view.

use ff::{Field, PrimeField};
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{
    Bit, GlueChip, UintChip, Word,
    bytes::element::{LeElement, scalar_bytes_canonical},
};

use super::{LINEAGE_FIELDS, LineagePublicCells};

/// Original incoming prefix and a fixed valid dummy for bounded operations.
///
/// The verifier hashes `fields`, never the selected dummy, and joins `valid`
/// internally. The supplied byte-decoding bit must be constrained from the
/// same external tape; noncanonical field bytes cannot be silently reduced.
#[derive(Clone, Debug)]
pub struct IncomingLineageCells {
    fields: [Word<Fp>; LINEAGE_FIELDS],
    checked: LineagePublicCells,
    valid: Bit<Fp>,
}
impl IncomingLineageCells {
    /// Check the hard prefix's exact version/width rules as a total predicate.
    ///
    /// `encoding_valid` is the mandatory verdict of the same-tape original
    /// byte decoder, including canonical-field encoding and framing/length.
    /// The decoded fields remain unchanged in every proof/context digest.
    /// After any failure, `checked` is fixed to `[1, 0, ..., 0]` before hard
    /// range checks. This view is arithmetic-safe, not an authenticated state.
    ///
    /// # Errors
    /// Layout failure; invalid version/width/encoding returns a false bit.
    pub fn constrain(
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        fields: &[Word<Fp>; LINEAGE_FIELDS],
        encoding_valid: &Bit<Fp>,
    ) -> Result<Self, Error> {
        let one = uint.glue().constant(region, Fp::ONE)?;
        let version = uint.glue().is_equal(region, &fields[0], &one)?;
        let mut valid = uint.glue().and(region, encoding_valid, &version)?;
        for index in [1, 2, 3, 4, 6, 7, 9, 10, 11, 12, 13, 14] {
            let element =
                LeElement::assign(uint, region, fields[index].value().map(|v| v.to_repr()))?;
            let canonical = scalar_bytes_canonical::<Fp, Fp>(uint, region, &element)?;
            GlueChip::assert_constant(region, canonical.word(), Fp::ONE)?;
            let joined = uint.glue().linear(
                region,
                &[
                    (Fp::ONE, element.lo().word()),
                    (Fp::from(2).pow_vartime([128]), element.hi().word()),
                ],
                Fp::ZERO,
            )?;
            GlueChip::assert_equal(region, &joined, &fields[index])?;
            let mut fits = uint.glue().is_zero(region, element.hi().word())?;
            if index == 13 {
                let low = uint.assign::<104>(
                    region,
                    element.lo().value().map(|v| v & ((1u128 << 104) - 1)),
                )?;
                let high = uint.assign::<24>(region, element.lo().value().map(|v| v >> 104))?;
                let joined = uint.glue().linear(
                    region,
                    &[
                        (Fp::ONE, low.word()),
                        (Fp::from_u128(1u128 << 104), high.word()),
                    ],
                    Fp::ZERO,
                )?;
                GlueChip::assert_equal(region, &joined, element.lo().word())?;
                let clear = uint.glue().is_zero(region, high.word())?;
                fits = uint.glue().and(region, &fits, &clear)?;
            }
            valid = uint.glue().and(region, &valid, &fits)?;
        }
        let zero = uint.glue().constant(region, Fp::ZERO)?;
        let safe = fields
            .iter()
            .enumerate()
            .map(|(i, field)| {
                uint.glue()
                    .select(region, &valid, field, if i == 0 { &one } else { &zero })
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let checked = LineagePublicCells::constrain(uint, region, &safe)?;
        Ok(Self {
            fields: fields.clone(),
            checked,
            valid,
        })
    }
    /// Original canonical-field prefix, including on a false verdict.
    pub const fn fields(&self) -> &[Word<Fp>; LINEAGE_FIELDS] {
        &self.fields
    }
    /// Total encoding/version/width validity, required by the soft verifier.
    pub const fn valid(&self) -> &Bit<Fp> {
        &self.valid
    }
    /// Hard-valid bounded arithmetic view, fixed to the dummy after failure.
    pub const fn checked(&self) -> &LineagePublicCells {
        &self.checked
    }
    /// Original carried Omega digest, not the selected dummy's digest.
    pub const fn omega_key_digest(&self) -> &Word<Fp> {
        &self.fields[17]
    }
}
