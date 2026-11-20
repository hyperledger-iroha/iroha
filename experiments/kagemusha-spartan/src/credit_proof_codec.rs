//! A compact encoding of an already decided Receive pair. Reconstruct the exact original before
//! linked proof verification. Every omitted byte follows from the installed programs and the
//! transmitted public queries. No transcript, sibling, signature or private custody input is an oracle.
use crate::{carrier, deferral, fs::Scalar, full, generator_claim, ipa, spartan};
use ff::{Field, PrimeField};
use group::{Curve, Group};
use iroha_pasta::{
    PastaCurve,
    msm::{MemoryBudget, msm_public},
};
use std::ops::Range;

struct Side {
    query: Vec<usize>,
    value: usize,
    generator: Vec<usize>,
    zeroes: Vec<usize>,
}

impl Side {
    fn new<S: Scalar>(
        layout: (usize, &[usize], usize),
        poly: &deferral::MatPoly<S>,
    ) -> Result<Self, String> {
        let (start, slots, columns) = layout;
        let offset = |slot: usize| {
            slots
                .iter()
                .position(|candidate| *candidate == slot)
                .map(|i| start + 32 * i)
                .ok_or_else(|| "missing installed Receive late slot".to_owned())
        };
        let query = (0..poly.n())
            .map(|i| offset(full::LATE_IO + i))
            .collect::<Result<Vec<_>, _>>()?;
        let value = offset(full::LATE_IO + poly.n())?;
        let generator = (0..4 + columns)
            .map(|i| offset(generator_claim::IO_START + i))
            .collect::<Result<Vec<_>, _>>()?;
        // A fresh matrix query pads the unused x/y coordinates with zero. Empty claims do too.
        // Cubic interpolation through four such claims keeps each of those coordinates at zero.
        let fixed: Vec<_> = (0..poly.kinds.len())
            .map(|k| poly.fixed_coords(k))
            .collect();
        let zeroes = (4..4 + poly.x + poly.y)
            .filter(|&i| fixed.iter().all(|row| row[i] == Some(S::ZERO)))
            .map(|i| query[i])
            .collect();
        Ok(Self {
            query,
            value,
            generator,
            zeroes,
        })
    }

    fn omitted(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.zeroes
            .iter()
            .chain(std::iter::once(&self.value))
            .chain(&self.generator[..4])
            .map(|&at| at..at + 32)
    }

    fn restore<C: PastaCurve>(
        &self,
        raw: &mut [u8],
        poly: &deferral::MatPoly<C::ScalarExt>,
        generators: &ipa::Gens<C>,
    ) -> Result<(), String>
    where
        C::ScalarExt: Scalar,
    {
        let words = |positions: &[usize]| {
            positions
                .iter()
                .map(|&at| {
                    let repr: [u8; 32] = raw[at..at + 32].try_into().unwrap();
                    Option::from(C::ScalarExt::from_repr(repr))
                        .ok_or_else(|| "noncanonical Receive accumulator word".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()
        };
        let query = words(&self.query)?;
        let challenges = words(&self.generator[4..])?;
        let n = 1usize
            .checked_shl(challenges.len() as u32)
            .ok_or("Receive generator dimension")?;
        let basis = generators.g.get(..n).ok_or("Receive generator basis")?;
        let point = msm_public::<C>(&ipa::s_vector(&challenges), basis, MemoryBudget::DEFAULT)
            .map_err(|_| "Receive generator reconstruction")?;
        let limbs = if bool::from(point.is_identity()) {
            [C::ScalarExt::ZERO; 4]
        } else {
            spartan::limbs_of_point::<C>(&point.to_affine())
        };
        for (&at, value) in self.generator.iter().zip(limbs) {
            raw[at..at + 32].copy_from_slice(&value.to_repr());
        }
        raw[self.value..self.value + 32].copy_from_slice(&poly.eval(&query).to_repr());
        Ok(())
    }
}

pub(super) struct Codec {
    prefix: Vec<u8>,
    primary: Side,
    secondary: Side,
    keeps: Vec<Range<usize>>,
    positions: carrier::payment_words::Positions,
    raw_len: usize,
    stripped_len: usize,
}

impl Codec {
    pub(super) fn new(
        profile: &carrier::TrustedProfile,
        gens: &full::PairGens,
    ) -> Result<Self, String> {
        let (raw_len, entries) = profile.payment_generator_words()?;
        let prefix = carrier::projected_prefix(profile);
        let primary = Side::new(profile.projected_late_layout(true)?, &gens.defer_p)?;
        let secondary = Side::new(profile.projected_late_layout(false)?, &gens.defer_s)?;
        let mut omitted: Vec<_> = std::iter::once(0..prefix.len())
            .chain(primary.omitted())
            .chain(secondary.omitted())
            .collect();
        omitted.sort_by_key(|range| range.start);
        let mut keeps = Vec::new();
        let mut positions = Vec::new();
        let (mut cursor, mut stripped_len) = (0, 0);
        for hole in omitted.into_iter().chain(std::iter::once(raw_len..raw_len)) {
            if hole.start < cursor || hole.end > raw_len {
                return Err("overlapping Receive pair projection".into());
            }
            if cursor < hole.start {
                for &(at, encoding) in &entries {
                    if at >= cursor && at + 32 <= hole.start {
                        positions.push((stripped_len + at - cursor, encoding));
                    }
                }
                keeps.push(cursor..hole.start);
                stripped_len += hole.start - cursor;
            }
            cursor = hole.end;
        }
        Ok(Self {
            prefix,
            primary,
            secondary,
            keeps,
            positions,
            raw_len,
            stripped_len,
        })
    }

    pub(super) fn encoded_len(&self) -> Result<usize, String> {
        carrier::payment_words::all_short_size(self.stripped_len, &self.positions)
    }

    pub(super) fn pack(&self, raw: &[u8]) -> Result<Vec<u8>, String> {
        if raw.len() != self.raw_len
            || !raw.starts_with(&self.prefix)
            || self
                .primary
                .zeroes
                .iter()
                .chain(&self.secondary.zeroes)
                .any(|&at| raw[at..at + 32] != [0; 32])
        {
            return Err("Receive pair profile or matrix padding".into());
        }
        let mut reduced = Vec::with_capacity(self.stripped_len);
        for keep in &self.keeps {
            reduced.extend_from_slice(&raw[keep.clone()]);
        }
        let packed = carrier::payment_words::pack(&reduced, &self.positions)?;
        // An actual verified fold derives every retained challenge from 126 transcript bits.
        if packed.len() != self.encoded_len()? {
            return Err("Receive generator challenge width".into());
        }
        Ok(packed)
    }

    pub(super) fn unpack(&self, encoded: &[u8], gens: &full::PairGens) -> Result<Vec<u8>, String> {
        if encoded.len() != self.encoded_len()? {
            return Err("Receive pair encoded length".into());
        }
        let reduced = carrier::payment_words::unpack(encoded, self.stripped_len, &self.positions)?;
        let mut raw = vec![0; self.raw_len];
        raw[..self.prefix.len()].copy_from_slice(&self.prefix);
        let mut cursor = 0;
        for keep in &self.keeps {
            raw[keep.clone()].copy_from_slice(&reduced[cursor..cursor + keep.len()]);
            cursor += keep.len();
        }
        self.primary.restore(&mut raw, &gens.defer_p, &gens.p)?;
        self.secondary.restore(&mut raw, &gens.defer_s, &gens.s)?;
        Ok(raw)
    }
}
