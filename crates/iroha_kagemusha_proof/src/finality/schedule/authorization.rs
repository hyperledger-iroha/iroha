//! Original epoch authorization fields, decoded in bounded fixed stages.
//!
//! The incumbent quorum authenticates native generation/selection/beacon choices.
//! These fields let the source prove exact epoch and height continuity; they do
//! not turn a decoded authorization into an independent signing authority.

use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, Uint, Word, WordHasher};

use super::{
    decode::{FieldSpan, ScheduleReader},
    epoch::{EpochHeader, EpochRanges},
};

/// Eleven exact original fields of a native scheduling authorization.
#[derive(Clone, Debug)]
pub struct AuthorizationRanges {
    epoch_body: FieldSpan,
    fields: [FieldSpan; 11],
}

impl AuthorizationRanges {
    /// Restore the full field split from a proved parser continuation.
    pub(crate) fn from_source_parts(epoch_body: FieldSpan, fields: [FieldSpan; 11]) -> Self {
        Self { epoch_body, fields }
    }
    /// Every original authorization range, in the sole native field order.
    pub(crate) fn source_parts(&self) -> &[FieldSpan; 11] {
        &self.fields
    }

    /// Decode exactly11 fields within the original epoch authorization.
    /// # Errors
    /// Layout errors; omitted/trailing fields or source substitutions fail.
    pub fn parse<H: WordHasher<Fp>>(
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        epoch: &EpochRanges,
    ) -> Result<Self, Error> {
        let mut cursor = reader.enter(region, epoch.authorization())?;
        let mut fields = Vec::with_capacity(11);
        for _ in 0..11 {
            fields.push(reader.field(region, &mut cursor)?);
        }
        reader.finish(region, &cursor)?;
        Ok(Self {
            epoch_body: epoch.body().clone(),
            fields: fields.try_into().map_err(|_| Error::Synthesis)?,
        })
    }

    /// Decode version, scheduling epoch, bounds, generation and decision.
    /// # Errors
    /// Layout errors; unsupported versions/decisions or invalid intervals fail.
    pub fn scalars<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
    ) -> Result<AuthorizationScalars, Error> {
        let version = reader.scalar::<2>(region, &self.fields[0])?;
        reader.constant_when(region, self.epoch_body.present(), version.word(), 1)?;
        let epoch = reader.scalar::<8>(region, &self.fields[2])?;
        let first = reader.scalar::<8>(region, &self.fields[3])?;
        let last = reader.scalar::<8>(region, &self.fields[4])?;
        let generation = reader.scalar::<8>(region, &self.fields[5])?;
        let decision = reader.scalar::<4>(region, &self.fields[10])?;
        let decision = reader.uint.range_check::<2>(region, decision.word())?;
        let zero = reader.uint.glue().is_zero(region, first.word())?;
        reader.constant_when(region, self.epoch_body.present(), zero.word(), 0)?;
        let backwards = reader.uint.lt(region, &last, &first)?;
        reader.constant_when(region, self.epoch_body.present(), backwards.word(), 0)?;
        Ok(AuthorizationScalars {
            epoch_body: self.epoch_body.clone(),
            epoch,
            first,
            last,
            generation,
            decision,
        })
    }

    /// Decode exact network, generation authority, predecessor and transition IDs.
    /// # Errors
    /// Layout errors; unsupported field widths or a different epoch network fail.
    pub fn identities<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        epoch: &EpochRanges,
        header: &EpochHeader,
    ) -> Result<AuthorizationIdentities, Error> {
        reader.same_span(region, &self.epoch_body, epoch.body())?;
        epoch.bind_header(reader, region, header)?;
        self.identities_for_source_network(reader, region, header.network())
    }

    /// Continue from the network proved in this parser's earlier header stage.
    /// The complete source context must bind that network across both stages.
    pub(crate) fn identities_for_source_network<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        expected_network: &[Word<Fp>; 32],
    ) -> Result<AuthorizationIdentities, Error> {
        let mut ids = Vec::with_capacity(4);
        for index in [1, 6, 8, 9] {
            reader.exact_len(region, &self.fields[index], 32)?;
            ids.push(reader.bytes::<32>(region, &self.fields[index], 0)?);
        }
        let [network, authority, previous, transition] =
            ids.try_into().map_err(|_| Error::Synthesis)?;
        for (a, b) in network.iter().zip(expected_network) {
            reader.equal_when(region, self.epoch_body.present(), a, b)?;
        }
        Ok(AuthorizationIdentities {
            network,
            authority,
            previous,
            transition,
        })
    }

    /// Decode Bootstrap or exact installed session/transcript from original bytes.
    /// # Errors
    /// Layout errors; malformed variant framing or session widths are unsatisfied.
    pub fn beacon<H: WordHasher<Fp>>(
        &self,
        reader: &mut ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
    ) -> Result<BeaconCells, Error> {
        let body = reader.tagged_body::<4>(region, &self.fields[7])?;
        reader.exact_len(region, &body, 66)?;
        let mut cursor = reader.enter(region, &body)?;
        let session = reader.field(region, &mut cursor)?;
        let transcript = reader.field(region, &mut cursor)?;
        reader.finish(region, &cursor)?;
        reader.exact_len(region, &session, 32)?;
        reader.exact_len(region, &transcript, 32)?;
        let session = reader.bytes::<32>(region, &session, 0)?;
        let transcript = reader.bytes::<32>(region, &transcript, 0)?;
        for identity in [&session, &transcript] {
            let mut sum = reader.uint.glue().constant(region, Fp::ZERO)?;
            for byte in identity {
                sum = reader.uint.glue().add(region, &sum, byte)?;
            }
            let zero = reader.uint.glue().is_zero(region, &sum)?;
            reader.constant_when(region, body.present(), zero.word(), 0)?;
        }
        Ok(BeaconCells {
            installed: body.present().clone(),
            session,
            transcript,
        })
    }
}

/// Native scalar scheduling bindings for one exact epoch body.
#[derive(Clone, Debug)]
pub struct AuthorizationScalars {
    epoch_body: FieldSpan,
    epoch: Uint<Fp, 64>,
    first: Uint<Fp, 64>,
    last: Uint<Fp, 64>,
    generation: Uint<Fp, 64>,
    decision: Uint<Fp, 2>,
}
impl AuthorizationScalars {
    /// Bind this decoded authorization back to the exact original context body.
    /// # Errors
    /// A different source epoch is unsatisfied.
    pub fn bind_epoch<H: WordHasher<Fp>>(
        &self,
        reader: &ScheduleReader<'_, '_, H>,
        region: &mut Region<'_, Fp>,
        epoch: &EpochRanges,
    ) -> Result<(), Error> {
        reader.same_span(region, &self.epoch_body, epoch.body())
    }
    /// Native scheduling epoch number.
    pub const fn epoch(&self) -> &Uint<Fp, 64> {
        &self.epoch
    }
    /// First governed height, inclusive.
    pub const fn first(&self) -> &Uint<Fp, 64> {
        &self.first
    }
    /// Last governed height, inclusive.
    pub const fn last(&self) -> &Uint<Fp, 64> {
        &self.last
    }
    /// Original validator generation number.
    pub const fn generation(&self) -> &Uint<Fp, 64> {
        &self.generation
    }
    /// Native decision0 genesis,1 activate,2 retain,3 retain-and-cancel.
    pub const fn decision(&self) -> &Uint<Fp, 2> {
        &self.decision
    }
}

/// Original32-byte identities, never caller-supplied decoded trust assertions.
#[derive(Clone, Debug)]
pub struct AuthorizationIdentities {
    network: [Word<Fp>; 32],
    authority: [Word<Fp>; 32],
    previous: [Word<Fp>; 32],
    transition: [Word<Fp>; 32],
}
impl AuthorizationIdentities {
    /// Exact genesis-derived network.
    pub const fn network(&self) -> &[Word<Fp>; 32] {
        &self.network
    }
    /// Exact native validator-generation identity.
    pub const fn authority(&self) -> &[Word<Fp>; 32] {
        &self.authority
    }
    /// Exact predecessor authorization identity.
    pub const fn previous(&self) -> &[Word<Fp>; 32] {
        &self.previous
    }
    /// Original activation/cancellation attempt identity.
    pub const fn transition(&self) -> &[Word<Fp>; 32] {
        &self.transition
    }
}

/// Original native beacon variant and installed bindings, zero when Bootstrap.
#[derive(Clone, Debug)]
pub struct BeaconCells {
    installed: Bit<Fp>,
    session: [Word<Fp>; 32],
    transcript: [Word<Fp>; 32],
}
impl BeaconCells {
    /// True only for the native Installed variant.
    pub const fn installed(&self) -> &Bit<Fp> {
        &self.installed
    }
    /// Original session identifier, zero for Bootstrap.
    pub const fn session(&self) -> &[Word<Fp>; 32] {
        &self.session
    }
    /// Original transcript identity, zero for Bootstrap.
    pub const fn transcript(&self) -> &[Word<Fp>; 32] {
        &self.transcript
    }
}
