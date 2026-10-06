//! Exact native epoch fields and bounded roster seats from the original R tape.
//! These decoders establish byte provenance and shape, not genesis/QC authority.

use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip, Uint, Word, WordHasher};

use super::decode::{FieldSpan, ScheduleReader};

/// Original source ranges of an epoch, split in one fixed seven-field step.
#[derive(Clone, Debug)]
pub struct EpochRanges {
    body: FieldSpan,
    layout: FieldSpan,
    version: FieldSpan,
    network: FieldSpan,
    mode: FieldSpan,
    authorization: FieldSpan,
    committee: FieldSpan,
    seed: FieldSpan,
}

impl EpochRanges {
    /// Decode exactly the seven native fields of a present/absent epoch body.
    /// # Errors
    /// Layout errors; changed lengths, trailing fields or foreign source fail.
    pub fn parse<H: WordHasher<Fp>>(
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        body: &FieldSpan,
    ) -> Result<Self, Error> {
        let mut cursor = reader.enter(region, body)?;
        let layout = reader.field(region, &mut cursor)?;
        let version = reader.field(region, &mut cursor)?;
        let network = reader.field(region, &mut cursor)?;
        let mode = reader.field(region, &mut cursor)?;
        let authorization = reader.field(region, &mut cursor)?;
        let committee = reader.field(region, &mut cursor)?;
        let seed = reader.field(region, &mut cursor)?;
        reader.finish(region, &cursor)?;
        Ok(Self {
            body: body.clone(),
            layout,
            version,
            network,
            mode,
            authorization,
            committee,
            seed,
        })
    }

    /// Complete source body, retained for exact context identity and continuity.
    pub const fn body(&self) -> &FieldSpan {
        &self.body
    }
    /// Original availability-layout bytes, to retain exactly across transitions.
    pub const fn layout(&self) -> &FieldSpan {
        &self.layout
    }
    /// Original complete scheduling authorization (not a certificate).
    pub const fn authorization(&self) -> &FieldSpan {
        &self.authorization
    }

    /// Bind a retained decoded header to this exact original context body.
    /// # Errors
    /// A different source epoch is unsatisfied.
    pub fn bind_header<H: WordHasher<Fp>>(
        &self,
        reader: &ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        header: &EpochHeader,
    ) -> Result<(), Error> {
        reader.same_span(region, &self.body, &header.body)
    }

    /// Decode native version, network, policy, leader seed and committee count.
    /// # Errors
    /// Layout errors; unsupported version/policy or roster geometry fails.
    pub fn header<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
    ) -> Result<EpochHeader, Error> {
        let version = reader.scalar::<2>(region, &self.version)?;
        reader.constant_when(region, self.body.present(), version.word(), 1)?;
        reader.exact_len(region, &self.network, 32)?;
        let network = reader.bytes::<32>(region, &self.network, 0)?;
        let mode = reader.scalar::<4>(region, &self.mode)?;
        let mode = reader.uint.range_check::<1>(region, mode.word())?;
        reader.exact_len(region, &self.seed, 32)?;
        let seed = reader.bytes::<32>(region, &self.seed, 0)?;
        let mut sum = reader.uint.glue().constant(region, Fp::ZERO)?;
        for byte in &seed {
            sum = reader.uint.glue().add(region, &sum, byte)?;
        }
        let zero = reader.uint.glue().is_zero(region, &sum)?;
        reader.constant_when(region, self.body.present(), zero.word(), 0)?;
        let count = reader.bytes::<8>(region, &self.committee, 0)?;
        let count = reader.pack_le(region, &count)?;
        let count = reader.uint.range_check::<5>(region, count.word())?;
        let faults = reader.uint.assign::<4>(
            region,
            count.value().map(|count| count.saturating_sub(1) / 3),
        )?;
        let expected =
            reader
                .uint
                .glue()
                .linear(region, &[(Fp::from(3), faults.word())], Fp::ONE)?;
        reader.equal_when(region, self.body.present(), count.word(), &expected)?;
        let zero = reader.uint.glue().is_zero(region, faults.word())?;
        reader.constant_when(region, self.body.present(), zero.word(), 0)?;
        let expected_bytes =
            reader
                .uint
                .glue()
                .linear(region, &[(Fp::from(215), count.word())], Fp::from(8))?;
        reader.equal_when(
            region,
            self.body.present(),
            self.committee.len().word(),
            &expected_bytes,
        )?;
        Ok(EpochHeader {
            body: self.body.clone(),
            network,
            mode,
            seed,
            count,
            faults,
        })
    }

    /// Decode one of31 circuit-selected seats; inactive seats are exactly zero.
    /// The selected index is bounded independently of committee count and is part
    /// of the leaf's input/output context. No caller-supplied active flag is used.
    /// # Errors
    /// Layout errors; wrong native wrappers, algorithm or source bounds fail.
    pub fn member<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        header: &EpochHeader,
        index: &Uint<Fp, 5>,
    ) -> Result<MemberCells, Error> {
        reader.same_span(region, &self.body, &header.body)?;
        let thirty_one = reader.uint.constant::<5>(region, 31)?;
        reader.uint.assert_lt(region, index, &thirty_one)?;
        let active = reader.uint.lt(region, index, &header.count)?;
        let source = reader.only_when(region, &self.committee, &active)?;
        let relative =
            reader
                .uint
                .glue()
                .linear(region, &[(Fp::from(215), index.word())], Fp::from(8))?;
        let relative = reader.uint.range_check::<32>(region, &relative)?;
        let size = reader.uint.constant::<32>(region, 215)?;
        let member = reader.subrange(region, &source, &relative, &size)?;
        let mut bytes = Vec::with_capacity(215);
        for start in [0, 32, 64, 96, 128, 160] {
            bytes.extend(reader.bytes::<32>(region, &member, start)?);
        }
        let end = reader.bytes::<32>(region, &member, 183)?;
        // The first9 bytes overlap the last preceding window; equality makes
        // their same-source correspondence explicit at this parser interface.
        for i in 0..9 {
            GlueChip::assert_equal(region, &bytes[183 + i], &end[i])?;
        }
        bytes.extend_from_slice(&end[9..]);
        for (offset, value) in [
            (0, 213),
            (1, 1),
            (2, 107),
            (3, 106),
            (4, 49),
            (12, 1),
            (13, 2),
            (110, 104),
            (111, 96),
        ] {
            reader.constant_when(region, member.present(), &bytes[offset], value)?;
        }
        for offset in (5..12).chain(112..119) {
            reader.constant_when(region, member.present(), &bytes[offset], 0)?;
        }
        let mut key = Vec::with_capacity(48);
        for i in 0..48 {
            reader.constant_when(region, member.present(), &bytes[14 + 2 * i], 1)?;
            key.push(bytes[15 + 2 * i].clone());
        }
        Ok(MemberCells {
            active: member.present().clone(),
            index: index.clone(),
            key: key.try_into().map_err(|_| Error::Synthesis)?,
            proof_of_possession: bytes[119..215]
                .to_vec()
                .try_into()
                .map_err(|_| Error::Synthesis)?,
        })
    }
}

/// Scalar/header fields decoded from the same complete epoch source.
#[derive(Clone, Debug)]
pub struct EpochHeader {
    body: FieldSpan,
    network: [Word<Fp>; 32],
    mode: Uint<Fp, 1>,
    seed: [Word<Fp>; 32],
    count: Uint<Fp, 5>,
    faults: Uint<Fp, 4>,
}
impl EpochHeader {
    /// Original32-byte network identity.
    pub const fn network(&self) -> &[Word<Fp>; 32] {
        &self.network
    }
    /// Native policy tag:0 permissioned,1 NPoS.
    pub const fn mode(&self) -> &Uint<Fp, 1> {
        &self.mode
    }
    /// Complete leader randomness from the authenticated context.
    pub const fn seed(&self) -> &[Word<Fp>; 32] {
        &self.seed
    }
    /// Exact ordered roster count, constrained to3f+1 and at most31.
    pub const fn count(&self) -> &Uint<Fp, 5> {
        &self.count
    }
    /// Exact positive fault bound (one through10 for a present context).
    pub const fn faults(&self) -> &Uint<Fp, 4> {
        &self.faults
    }
}

/// Original BLS key and PoP at one exact roster position. No key/PoP cryptography
/// or authority is implied; the surrounding native BLS relation must verify them.
#[derive(Clone, Debug)]
pub struct MemberCells {
    active: Bit<Fp>,
    index: Uint<Fp, 5>,
    key: [Word<Fp>; 48],
    proof_of_possession: [Word<Fp>; 96],
}
impl MemberCells {
    /// True exactly for a present source context and position below its count.
    pub const fn active(&self) -> &Bit<Fp> {
        &self.active
    }
    /// Exact original roster position to bind to the quorum bitmap.
    pub const fn index(&self) -> &Uint<Fp, 5> {
        &self.index
    }
    /// Original compressed48-byte BLS-normal public key.
    pub const fn key(&self) -> &[Word<Fp>; 48] {
        &self.key
    }
    /// Original96-byte proof of possession of that same key.
    pub const fn proof_of_possession(&self) -> &[Word<Fp>; 96] {
        &self.proof_of_possession
    }
}
