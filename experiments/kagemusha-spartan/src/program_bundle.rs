//! Exact public-program transport for Native wallets and the offline compiler.
use super::{Error, MAX_PROGRAM_BUNDLE_BYTES, ProgramBundle, Result};
use crate::{fs::Scalar, full, generator_claim, r1cs::Circuit, uniform::UniformCircuit};
use iroha_pasta::{Fp, Fq};

/// The compiler's successor bundle, ordered Receive, Send, Bootstrap, Control,
/// ArchiveSent on each curve. This unsigned container cannot install a wallet.
#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.native.ProgramBundle.v2")]
pub struct ProgramBundleV2 {
    pub version: u16,
    pub primary: [Vec<u8>; 5],
    pub secondary: [Vec<u8>; 5],
}

fn decode_frame<T>(bytes: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    if bytes.len() > MAX_PROGRAM_BUNDLE_BYTES {
        return Err(Error::ResourceLimit);
    }
    let limits = norito::DecodeLimits::new(
        MAX_PROGRAM_BUNDLE_BYTES,
        MAX_PROGRAM_BUNDLE_BYTES,
        2 * MAX_PROGRAM_BUNDLE_BYTES,
        4 * MAX_PROGRAM_BUNDLE_BYTES,
        16,
    );
    // Keep stricter caller budgets and leave the process default untouched.
    norito::core::with_decode_limits(limits, || {
        let payload = norito::canonical_frame_payload_with_archive_limit::<T>(
            bytes,
            norito::core::V1_LAYOUT_FLAGS,
            MAX_PROGRAM_BUNDLE_BYTES as u64,
        )?;
        let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
        let (value, used) = norito::core::decode_field_canonical(payload)?;
        if used != payload.len() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(value)
    })
    .map_err(|_| Error::Malformed("bounded program bundle"))
}

// The two schemas keep their own fixed array widths. Only their framing,
// aggregate byte ceiling and field validation are shared.
macro_rules! program_bundle_codec {
    ($bundle:ty, $version:literal) => {
        impl $bundle {
            fn validate(&self) -> Result<()> {
                let programs = self.primary.iter().chain(&self.secondary);
                if self.version != $version || programs.clone().any(Vec::is_empty) {
                    return Err(Error::Malformed("program bundle"));
                }
                let mut total = 0usize;
                for program in programs {
                    total = total
                        .checked_add(program.len())
                        .ok_or(Error::ResourceLimit)?;
                }
                if total > MAX_PROGRAM_BUNDLE_BYTES {
                    return Err(Error::ResourceLimit);
                }
                Ok(())
            }

            pub fn encode(&self) -> Result<Vec<u8>> {
                self.validate()?;
                let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
                norito::core::to_bytes_bounded(self, MAX_PROGRAM_BUNDLE_BYTES).map_err(|error| {
                    match error {
                        norito::core::BoundedEncodeError::Serialization(error) => {
                            Error::Rejected(error.to_string())
                        }
                        _ => Error::ResourceLimit,
                    }
                })
            }

            pub fn decode(bytes: &[u8]) -> Result<Self> {
                let bundle: Self = decode_frame(bytes)?;
                bundle.validate()?;
                norito::verify_exact_canonical_frame_with_layout(
                    &bundle,
                    bytes,
                    norito::core::WireLayout::Fixed,
                )
                .map_err(|_| Error::Malformed("canonical program bundle"))?;
                Ok(bundle)
            }
        }
    };
}
program_bundle_codec!(ProgramBundle, 1);
program_bundle_codec!(ProgramBundleV2, 2);

/// Geometry shared by the five original verifier tapes on one curve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramBundleGeometry {
    pub constraint_bits: usize,
    pub row_bits: usize,
    pub column_bits: usize,
    pub first_phase_rows: usize,
    pub link_phase_rows: usize,
    pub fixed_rows: usize,
    pub folded_claim_words: usize,
}

impl ProgramBundleV2 {
    /// Compare every original with independently supplied compiler pins and
    /// measure the common recursive layout. The resulting geometry grants no
    /// Artifact-role signature, relation admission, or authority to spend.
    pub fn inspect_originals(
        bytes: &[u8],
        primary_pins: &[[u8; 32]; 5],
        secondary_pins: &[[u8; 32]; 5],
        accumulator_dimensions: [[u32; 2]; 2],
    ) -> Result<[ProgramBundleGeometry; 2]> {
        let bundle = Self::decode(bytes)?;
        Ok([
            inspect_side::<Fp>(
                bundle.primary,
                primary_pins,
                accumulator_dimensions[0],
                true,
            )?,
            inspect_side::<Fq>(
                bundle.secondary,
                secondary_pins,
                accumulator_dimensions[1],
                false,
            )?,
        ])
    }
}

fn inspect_side<F: Scalar>(
    originals: [Vec<u8>; 5],
    pins: &[[u8; 32]; 5],
    dimensions: [u32; 2],
    primary: bool,
) -> Result<ProgramBundleGeometry> {
    if dimensions.iter().any(|axis| !(1..=30).contains(axis)) {
        return Err(Error::Malformed("parameter accumulator dimensions"));
    }
    // Three selector coordinates plus rho, alpha, beta, the matrix point,
    // and the folded value. Every kind exposes these at the same positions.
    let [x, y] = dimensions.map(|axis| axis as usize);
    let folded_claim_words = 3 + 3 + x + y + 1;
    let end = full::LATE_IO + folded_claim_words;
    if end > full::LINK_RHO_IO {
        return Err(Error::Malformed("folded claim overlaps link IO"));
    }
    let mut common = None;
    for (kind, original) in originals.into_iter().enumerate() {
        if pins[..kind].contains(&pins[kind]) {
            return Err(Error::Malformed("duplicate five-kind program pin"));
        }
        // import_program checks field, side, exact format, 64 ordinary words,
        // degree bounds, the link slots and the reconstructed matrix tape.
        let tape =
            UniformCircuit::<F>::import_program(&original, primary).map_err(Error::Rejected)?;
        if tape.program_digest() != pins[kind] {
            return Err(Error::AuthorityMismatch("five-kind original digest"));
        }
        if tape.shape.sx > x || tape.shape.sz() > y {
            return Err(Error::Malformed("parameter accumulator dimensions"));
        }
        if tape.wallet_io() != primary.then_some(full::TABLE_IO)
            || (primary
                && tape.shape.active_cols()
                    < crate::nonnative::WALLET_CELLS * crate::nonnative::WALLET_ROWS)
        {
            return Err(Error::Malformed("five-kind wallet table geometry"));
        }
        let generators =
            generator_claim::IO_START..generator_claim::IO_START + 4 + tape.shape.col_bits;
        if !tape
            .late_slots()
            .into_iter()
            .eq((full::LATE_IO..end).chain(generators))
        {
            return Err(Error::Malformed("five-kind late IO layout"));
        }
        let layout = tape.layout();
        let geometry = ProgramBundleGeometry {
            constraint_bits: layout.sx,
            row_bits: layout.row_bits,
            column_bits: tape.shape.col_bits,
            first_phase_rows: layout.p1,
            link_phase_rows: layout.pl,
            fixed_rows: layout.fixed,
            folded_claim_words,
        };
        if common
            .replace(geometry)
            .is_some_and(|prior| prior != geometry)
        {
            return Err(Error::Malformed("five-kind common program layout"));
        }
    }
    common.ok_or(Error::Malformed("empty five-kind program set"))
}

#[cfg(test)]
#[path = "program_bundle_tests.rs"]
mod tests;
