//! Exact public-program transport for Native wallets and the offline compiler.
use super::{Error, MAX_PROGRAM_BUNDLE_BYTES, ProgramBundle, Result};
use crate::{
    carrier::{ProjectedSide, TrustedProfile},
    fs::Scalar,
    full, generator_claim,
    r1cs::Circuit,
    uniform::UniformCircuit,
};
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
    pub sumcheck_variables: usize,
    pub row_bits: usize,
    pub column_bits: usize,
    pub first_phase_rows: usize,
    pub link_phase_rows: usize,
    pub fixed_rows: usize,
    pub folded_claim_words: usize,
}

/// Exact compact Send sizes and maximum-depth CreditStatus peer messages
/// under the authenticated uniform layout. All values are bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramTransportSizes {
    pub payment_pair: usize,
    pub payment_envelope: usize,
    pub payment_peer: usize,
    pub receive_status_peer: usize,
    pub later_send_status_peer: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SumcheckGroup {
    Three,
    Four,
}

impl SumcheckGroup {
    // The ordinary five-kind inspection API predates the independently selected
    // four-variable compiler. Its standalone profile is strictly group three.
    // A released producer overrides that choice only for its exact pins and axes.
    fn for_pins(
        primary: &[[u8; 32]; 5],
        secondary: &[[u8; 32]; 5],
        dimensions: [[u32; 2]; 2],
    ) -> [Self; 2] {
        Self::for_source(
            super::FIVE_KIND_PRODUCER.as_ref(),
            primary,
            secondary,
            dimensions,
        )
    }

    fn for_source(
        registered: Option<&super::FiveKindProducer>,
        primary: &[[u8; 32]; 5],
        secondary: &[[u8; 32]; 5],
        dimensions: [[u32; 2]; 2],
    ) -> [Self; 2] {
        registered
            .filter(|source| {
                source.programs.primary == *primary
                    && source.programs.secondary == *secondary
                    && source.programs.dimensions == dimensions
            })
            .map_or([Self::Three; 2], |source| source.grouping)
    }
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
        let (geometry, _) = Self::decode(bytes)?.inspect_contents(
            primary_pins,
            secondary_pins,
            accumulator_dimensions,
            SumcheckGroup::for_pins(primary_pins, secondary_pins, accumulator_dimensions),
            drop,
            drop,
        )?;
        Ok(geometry)
    }

    /// Qualify the pinned programs with the actual compact Payment and
    /// maximum-depth CreditStatus codecs for both receiver heads before signing or proving.
    pub fn inspect_retail_originals(
        bytes: &[u8],
        primary_pins: &[[u8; 32]; 5],
        secondary_pins: &[[u8; 32]; 5],
        dimensions: [[u32; 2]; 2],
    ) -> Result<([ProgramBundleGeometry; 2], ProgramTransportSizes)> {
        Self::inspect_retail_with(
            bytes,
            primary_pins,
            secondary_pins,
            dimensions,
            SumcheckGroup::for_pins(primary_pins, secondary_pins, dimensions),
        )
    }

    #[cfg(test)]
    pub(in crate::native) fn inspect_four_variable_retail_originals(
        bytes: &[u8],
        primary_pins: &[[u8; 32]; 5],
        secondary_pins: &[[u8; 32]; 5],
        dimensions: [[u32; 2]; 2],
    ) -> Result<([ProgramBundleGeometry; 2], ProgramTransportSizes)> {
        Self::inspect_retail_with(
            bytes,
            primary_pins,
            secondary_pins,
            dimensions,
            [SumcheckGroup::Four; 2],
        )
    }

    fn inspect_retail_with(
        bytes: &[u8],
        primary_pins: &[[u8; 32]; 5],
        secondary_pins: &[[u8; 32]; 5],
        dimensions: [[u32; 2]; 2],
        readers: [SumcheckGroup; 2],
    ) -> Result<([ProgramBundleGeometry; 2], ProgramTransportSizes)> {
        let (geometry, sides) = Self::decode(bytes)?.inspect_contents(
            primary_pins,
            secondary_pins,
            dimensions,
            readers,
            drop,
            drop,
        )?;
        let sizes =
            Self::admit_transports(primary_pins[full::RECEIVE], geometry, sides, dimensions)?;
        Ok((geometry, sizes))
    }

    /// Retain the verified programs for an authenticated installation. Offline
    /// inspection drops each tape immediately after measuring it.
    pub(super) fn import_retail_programs(
        bytes: &[u8],
        profile: &super::ProgramProfileV2,
    ) -> Result<([ProgramBundleGeometry; 2], Box<[super::engine::Program]>)> {
        Self::retain_programs(
            bytes,
            profile,
            SumcheckGroup::for_pins(&profile.primary, &profile.secondary, profile.dimensions),
        )
    }

    fn retain_programs(
        bytes: &[u8],
        profile: &super::ProgramProfileV2,
        groups: [SumcheckGroup; 2],
    ) -> Result<([ProgramBundleGeometry; 2], Box<[super::engine::Program]>)> {
        let mut primary = Vec::with_capacity(5);
        let mut secondary = Vec::with_capacity(5);
        let (geometry, sides) = Self::decode(bytes)?.inspect_contents(
            &profile.primary,
            &profile.secondary,
            profile.dimensions,
            groups,
            |tape| primary.push(std::sync::Arc::new(tape)),
            |tape| secondary.push(std::sync::Arc::new(tape)),
        )?;
        Self::admit_transports(
            profile.primary[full::RECEIVE],
            geometry,
            sides,
            profile.dimensions,
        )?;
        let programs = primary
            .into_iter()
            .zip(secondary)
            .map(|(primary, secondary)| super::engine::Program { primary, secondary })
            .collect();
        Ok((geometry, programs))
    }

    fn admit_transports(
        profile_id: [u8; 32],
        geometry: [ProgramBundleGeometry; 2],
        sides: [ProjectedSide; 2],
        dimensions: [[u32; 2]; 2],
    ) -> Result<ProgramTransportSizes> {
        let shapes = sides.each_ref().map(|side| side.shape);
        let profile =
            TrustedProfile::from_sides(profile_id, sides, true).map_err(Error::Rejected)?;
        let matrices = std::array::from_fn(|side| {
            let layout = geometry[side];
            let shape = shapes[side];
            super::engine::MatrixQueryLayout {
                words: layout.folded_claim_words - 1,
                dimensions: dimensions[side].map(|n| n as usize),
                occupied: [shape.sx, shape.sz()],
            }
        });
        let send = crate::payment_wire::admit_send_profile(&profile).map_err(Error::Rejected)?;
        let [receive, later_send] =
            super::engine::admit_common_program_geometry(&profile, matrices)?;
        Ok(ProgramTransportSizes {
            payment_pair: send.pair,
            payment_envelope: send.envelope,
            payment_peer: send.peer,
            receive_status_peer: receive,
            later_send_status_peer: later_send,
        })
    }

    fn inspect_contents(
        self,
        primary_pins: &[[u8; 32]; 5],
        secondary_pins: &[[u8; 32]; 5],
        accumulator_dimensions: [[u32; 2]; 2],
        readers: [SumcheckGroup; 2],
        retain_primary: impl FnMut(UniformCircuit<Fp>),
        retain_secondary: impl FnMut(UniformCircuit<Fq>),
    ) -> Result<([ProgramBundleGeometry; 2], [ProjectedSide; 2])> {
        let [(gp, p), (gs, s)] = [
            inspect_side::<Fp>(
                self.primary,
                primary_pins,
                accumulator_dimensions[0],
                true,
                readers[0],
                retain_primary,
            )?,
            inspect_side::<Fq>(
                self.secondary,
                secondary_pins,
                accumulator_dimensions[1],
                false,
                readers[1],
                retain_secondary,
            )?,
        ];
        Ok(([gp, gs], [p, s]))
    }
}

fn inspect_side<F: Scalar>(
    originals: [Vec<u8>; 5],
    pins: &[[u8; 32]; 5],
    dimensions: [u32; 2],
    primary: bool,
    reader: SumcheckGroup,
    mut retain: impl FnMut(UniformCircuit<F>),
) -> Result<(ProgramBundleGeometry, ProjectedSide)> {
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
        let tape = match reader {
            SumcheckGroup::Three => UniformCircuit::<F>::import_program(&original, primary),
            SumcheckGroup::Four => {
                UniformCircuit::<F>::import_four_variable_program(&original, primary)
            }
        }
        .map_err(Error::Rejected)?;
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
            sumcheck_variables: tape.shape.k,
            row_bits: layout.row_bits,
            column_bits: tape.shape.col_bits,
            first_phase_rows: layout.p1,
            link_phase_rows: layout.pl,
            fixed_rows: layout.fixed,
            folded_claim_words,
        };
        if let Some((prior, _)) = &common {
            if prior != &geometry {
                return Err(Error::Malformed("five-kind common program layout"));
            }
        } else {
            common = Some((geometry, ProjectedSide::measure(tape.shape, &tape)));
        }
        retain(tape);
    }
    common.ok_or(Error::Malformed("empty five-kind program set"))
}

#[cfg(test)]
#[path = "program_bundle_tests.rs"]
pub(super) mod tests;
