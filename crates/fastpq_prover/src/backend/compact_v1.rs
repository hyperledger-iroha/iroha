//! Six-lane commitments and field-native tapes for offline compact verification.
//!
//! The profile is not admitted. Full canonical public context is bound into
//! the prefix of every typed hash input; construction does not authenticate that context.
//! TODO: Establish the implementation mapping to the internally reviewed conditional
//! block-oracle compiler result and independently qualify the concrete construction,
//! complete relation, privacy, resources and hardware before production admission.

use std::sync::{Arc, OnceLock};

use fastpq_isi::{
    FASTPQ_CATALOG_V1, FASTPQ_FINAL_V1, GoldilocksDigest384OwnedDomainPrefixV1,
    GoldilocksDigest384OwnedDomainV1, GoldilocksDigest384V1 as Digest,
};
use norito::NoritoSerialize;

use crate::field::{GOLDILOCKS_MODULUS_V1 as MODULUS, GoldilocksFp4V1};

pub(super) const IDENTITY: &[u8] =
    b"fastpq:compact:goldilocks-six-lane:h6:g-field-blocks:q375:c401:342cols:923slots:65536rows:8blowup:17folds:v1";
const MAX_CONTEXT_BYTES: usize = 256 * 1024;
const LDE_ROWS: u32 = 524_288;
const QUERY_COUNT: usize = 375;
const QUERY_CANDIDATES: usize = 401;

const QUERY_TAPE_BYTES: usize = QUERY_CANDIDATES.div_ceil(6) * 48;
const H_OUTPUT_BYTES: usize = 48;
// H includes 18 FRI rounds and all 21 transcript chain commitments. Row,
// mixed and quotient nodes use round zero and retain their oracle in the body.
const H_CACHE_ROUNDS: usize = 22;
const H_CACHE_LEVELS: usize = 20;
const G_CACHE_ROUNDS: usize = 22;
const PREFIX_CACHE_SLOTS: usize = H_CACHE_ROUNDS * H_CACHE_LEVELS + G_CACHE_ROUNDS;
const H_ROLE: &[u8] = b"compact-commitment";
const H_PHASE: &[u8] = b"typed-h";
const G_ROLE: &[u8] = b"compact-transcript";
const G_PHASE: &[u8] = b"whole-field-tape-block";

/// Failure of the isolated candidate's framing, decoding or state machine.
#[derive(Debug, thiserror::Error)]
pub(super) enum CandidateError {
    /// Context must be nonempty and satisfy the candidate's fixed byte ceiling.
    #[error("candidate context is empty or exceeds its fixed byte ceiling")]
    Context,
    /// Round must be one of the fixed 22 verifier messages.
    #[error("candidate verifier round is outside 1..=22")]
    Round,
    /// Oracle, leaf or parent geometry is not the fixed candidate geometry.
    #[error("candidate tree geometry or canonical leaf payload is invalid")]
    Tree,
    /// A tape has another fixed length.
    #[error("candidate tape has another fixed length")]
    TapeLength,
    /// A field-native tape contains a noncanonical base-field word.
    #[error("candidate tape contains a noncanonical field word")]
    TapeEncoding,
    /// A bounded tape did not contain enough accepted values.
    #[error("candidate fixed tape exhausted")]
    TapeExhausted,
    /// The operation does not follow the challenge/commit schedule.
    #[error("candidate transcript operation is out of sequence")]
    Phase,
    /// Canonical Norito framing failed.
    #[error("candidate Norito framing failed: {0}")]
    Encode(#[from] norito::core::Error),
}

type Result<T> = std::result::Result<T, CandidateError>;

/// Validated ordinal of one complete verifier message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Round(u8);

impl Round {
    /// Validate the exact initial-dummy through final-query round range.
    pub(super) fn new(ordinal: u8) -> Result<Self> {
        if (1..=22).contains(&ordinal) {
            Ok(Self(ordinal))
        } else {
            Err(CandidateError::Round)
        }
    }

    /// Fixed whole raw-tape length, including unused samples and suffix bytes.
    pub(super) const fn tape_bytes(self) -> usize {
        match self.0 {
            1 => 48,
            2 => 10_944,
            3 => 29_568,
            4 => 96,
            5..=21 => 48,
            22 => QUERY_TAPE_BYTES,
            _ => unreachable!(),
        }
    }

    fn field_coordinates(self) -> Option<usize> {
        match self.0 {
            2 => Some(1368),
            3 => Some(3692),
            4 => Some(8),
            5..=21 => Some(4),
            _ => None,
        }
    }
}

/// Complete decoded verifier message; it never exposes mutable pending tape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Message {
    /// Initial empty IOP message from an ignored positive-length random tape.
    Dummy,
    /// One complete vector in the fixed extension-field basis.
    Fields(Vec<GoldilocksFp4V1>),
    /// Exactly 375 ascending distinct initial LDE positions.
    Queries(Vec<u32>),
}

/// Oracle identity within this candidate's fixed commitment schedule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Oracle {
    /// Complete row of 342 canonical base-field coordinates.
    Row,
    /// One mixed extension-field value.
    Mixed,
    /// One quotient extension-field value.
    Quotient,
    /// Binary pair leaves in rounds 0..=16; one whole-vector leaf in round 17.
    Fri(u8),
}

impl Oracle {
    fn shape(self) -> Result<(u8, u8, u32, usize)> {
        match self {
            Self::Row => Ok((1, 0, LDE_ROWS, 342 * 8)),
            Self::Mixed => Ok((2, 0, LDE_ROWS, 32)),
            Self::Quotient => Ok((3, 0, LDE_ROWS, 32)),
            Self::Fri(round @ 0..=16) => Ok((4, round, LDE_ROWS >> (round + 1), 64)),
            Self::Fri(17) => Ok((4, 17, 1, 128)),
            Self::Fri(_) => Err(CandidateError::Tree),
        }
    }
}

/// Immutable, bounded complete canonical profile-context prefix.
///
/// Every logical oracle input is the complete canonical prefix followed by one
/// canonical body. State reuse changes physical work, never those input bytes.
#[derive(Clone, Debug)]
pub(super) struct Context {
    profile: FramingProfile,
    prefix: Arc<AbsorbedPrefix>,
}

// A closed internal profile selector preserves the single canonical framing owner.
#[derive(Clone, Copy, Debug)]
enum FramingProfile {
    Current,
    #[cfg(test)]
    Deep,
}

#[derive(Debug)]
struct AbsorbedPrefix {
    // One complete canonical frame shared by every cached domain descriptor.
    encoded: Arc<[u8]>,
    // Fixed geometry, lazy immutable snapshots, no process-global context cache.
    states: [OnceLock<Option<GoldilocksDigest384OwnedDomainPrefixV1>>; PREFIX_CACHE_SLOTS],
}

// Canonical profile/context and body frames fix schema, layout and lengths.
// The complete context and every protocol-tape word remain in the logical input.
// The profile field here is a typed complete profile-context descriptor, not
// the public short metadata profile ID. See specs/fastpq_compact_v1_framing.md.
#[derive(Clone, Debug, NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_v1::PrefixFrame",
    frame = "fastpq_prover::compact_v1::ProfileContextV1"
)]
struct PrefixFrame {
    version: u16,
    identity: Vec<u8>,
    context: Vec<u8>,
}

// These payload-only views preserve Vec<Vec<u8>> bytes without owning copies.
// The protocol has exactly one leaf/predecessor field or two child/tape-root fields.
/// Borrowed canonical body fields shared by the closed internal protocol owners.
#[derive(Clone, Copy, Debug)]
pub(super) enum BodyFields<'a> {
    /// One complete leaf or predecessor.
    One(&'a [u8]),
    /// Two complete children, or a whole tape followed by its committed root.
    Two(&'a [u8], &'a [u8]),
}

#[derive(Clone, Copy)]
struct ByteField<'a>(&'a [u8]);

impl norito::core::SerializePayload for ByteField<'_> {
    fn serialize(
        &self,
        writer: &mut norito::core::Encoder<'_>,
    ) -> std::result::Result<(), norito::core::Error> {
        // Vec<u8>'s specialization writes a fixed u64 count and the raw bytes.
        let length =
            u64::try_from(self.0.len()).map_err(|_| norito::core::Error::LengthMismatch)?;
        norito::core::write_seq_len(writer, length)?;
        writer.write_all(self.0)?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.len().checked_add(8)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.len().checked_add(8)
    }
}

impl norito::core::SerializePayload for BodyFields<'_> {
    fn serialize(
        &self,
        writer: &mut norito::core::Encoder<'_>,
    ) -> std::result::Result<(), norito::core::Error> {
        // Use Norito's existing sequence owner for counts, element lengths and
        // checked emission. Both variants are stack arrays of borrowed views.
        match self {
            Self::One(field) => norito::core::write_element_sequence::<ByteField<'_>, _>(
                writer,
                [ByteField(field)],
                norito::core::max_archive_len(),
            ),
            Self::Two(first, second) => norito::core::write_element_sequence::<ByteField<'_>, _>(
                writer,
                [ByteField(first), ByteField(second)],
                norito::core::max_archive_len(),
            ),
        }
    }
}

/// The sole canonical typed body layout; schema identity does not change.
#[derive(Clone, Debug, NoritoSerialize)]
pub(super) struct Frame<'a> {
    kind: u8,
    oracle: u8,
    round: u8,
    level: u32,
    position: u32,
    output_bytes: u32,
    fields: BodyFields<'a>,
}

// A lifetime is only an ownership detail. Preserve both existing identities;
// the schema derive would append its lifetime placeholder to the nominal name.
impl norito::NoritoSchema for Frame<'_> {
    fn nominal_name() -> String {
        "fastpq_prover::backend::compact_v1::Frame".to_owned()
    }

    fn frame_name() -> String {
        "fastpq_prover::compact_v1::BodyV1".to_owned()
    }
}

impl Context {
    /// Canonically frame and absorb one bounded immutable complete context.
    pub(super) fn new(bytes: &[u8]) -> Result<Self> {
        Self::with_profile(bytes, IDENTITY, FramingProfile::Current)
    }

    /// Construct only the fixed private DEEP profile, using the same frame owner.
    #[cfg(test)]
    pub(super) fn new_deep(bytes: &[u8]) -> Result<Self> {
        Self::with_profile(bytes, super::deep_binding::IDENTITY, FramingProfile::Deep)
    }

    fn with_profile(bytes: &[u8], identity: &[u8], profile: FramingProfile) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_CONTEXT_BYTES {
            return Err(CandidateError::Context);
        }
        let encoded = norito::encode_canonical(&PrefixFrame {
            version: 1,
            identity: identity.to_vec(),
            context: bytes.to_vec(),
        })?;
        Ok(Self {
            profile,
            prefix: Arc::new(AbsorbedPrefix {
                encoded: Arc::from(encoded),
                states: core::array::from_fn(|_| OnceLock::new()),
            }),
        })
    }

    // Each logical hash still includes the complete profile-context frame.
    // Only the existing canonical prefix owner's immutable lane state is cached.
    fn cache_slot(role: &[u8], phase: &[u8], round: u8, level: u32) -> Result<usize> {
        if role == H_ROLE
            && phase == H_PHASE
            && usize::from(round) < H_CACHE_ROUNDS
            && level < H_CACHE_LEVELS as u32
        {
            Ok(usize::from(round) * H_CACHE_LEVELS + level as usize)
        } else if role == G_ROLE
            && phase == G_PHASE
            && (1..=G_CACHE_ROUNDS as u8).contains(&round)
            && level == 0
        {
            Ok(H_CACHE_ROUNDS * H_CACHE_LEVELS + usize::from(round - 1))
        } else {
            Err(CandidateError::Tree)
        }
    }

    fn cached_prefix(
        &self,
        role: &[u8],
        phase: &[u8],
        round: u8,
        level: u32,
    ) -> Result<&GoldilocksDigest384OwnedDomainPrefixV1> {
        let slot = match self.profile {
            FramingProfile::Current => Self::cache_slot(role, phase, round, level)?,
            #[cfg(test)]
            FramingProfile::Deep => {
                const LEVELS: usize = super::deep_geometry::LDE_ROWS.ilog2() as usize + 1;
                const ROUNDS: usize = 10;
                const _: () = assert!(ROUNDS * LEVELS + ROUNDS <= PREFIX_CACHE_SLOTS);
                if role == H_ROLE
                    && phase == H_PHASE
                    && usize::from(round) < ROUNDS
                    && (level as usize) < LEVELS
                {
                    usize::from(round) * LEVELS + level as usize
                } else if role == G_ROLE
                    && phase == G_PHASE
                    && (1..=ROUNDS as u8).contains(&round)
                    && level == 0
                {
                    ROUNDS * LEVELS + usize::from(round - 1)
                } else {
                    return Err(CandidateError::Tree);
                }
            }
        };
        self.prefix.states[slot]
            .get_or_init(|| {
                GoldilocksDigest384OwnedDomainPrefixV1::new(GoldilocksDigest384OwnedDomainV1 {
                    catalog: Arc::from(FASTPQ_CATALOG_V1.as_bytes()),
                    protocol: Arc::from(FASTPQ_FINAL_V1.name.as_bytes()),
                    profile: self.prefix.encoded.clone(),
                    role: Arc::from(role),
                    phase: Arc::from(phase),
                    level: u64::from(level),
                    index: 0,
                    counter: u64::from(round),
                })
            })
            .as_ref()
            .ok_or(CandidateError::Tree)
    }

    fn digest(
        &self,
        role: &[u8],
        phase: &[u8],
        round: u8,
        level: u32,
        index: u64,
        body: &[u8],
    ) -> Result<Digest> {
        let prefix = self.cached_prefix(role, phase, round, level)?;
        let digest = prefix.hash_at(index, &[body]).ok_or(CandidateError::Tree)?;
        #[cfg(test)]
        super::compact_protocol::metal_diagnostic::observe(prefix, index, body, digest);
        Ok(digest)
    }

    fn expand(&self, round: Round, body: &[u8], output: &mut [u8]) -> Result<()> {
        if output.len() != round.tape_bytes() || output.len() % 48 != 0 {
            return Err(CandidateError::TapeLength);
        }
        self.expand_blocks(round.0, body, output)
    }

    /// Expand the complete tape of one fixed private DEEP message.
    #[cfg(test)]
    pub(super) fn expand_deep(
        &self,
        round: super::deep_binding::Round,
        body: &[u8],
        output: &mut [u8],
    ) -> Result<()> {
        if !matches!(self.profile, FramingProfile::Deep) || output.len() != round.tape_bytes() {
            return Err(CandidateError::TapeLength);
        }
        self.expand_blocks(round.ordinal(), body, output)
    }

    fn expand_blocks(&self, round: u8, body: &[u8], output: &mut [u8]) -> Result<()> {
        // All scheduled blocks are materialized, including unconsumed coordinates.
        // These are F_p^6 outputs, never uniform 384-bit strings.
        for (block, target) in output.chunks_exact_mut(48).enumerate() {
            let digest = self.digest(
                b"compact-transcript",
                b"whole-field-tape-block",
                round,
                0,
                block as u64,
                body,
            )?;
            target.copy_from_slice(&digest.to_le_bytes());
        }
        Ok(())
    }

    /// Hash a body through the sole canonical cached framing owner.
    pub(super) fn hash_frame(&self, frame: &Frame<'_>) -> Result<Digest> {
        let encoded = norito::encode_canonical(frame)?;
        self.digest(
            b"compact-commitment",
            b"typed-h",
            frame.round,
            frame.level,
            u64::from(frame.position),
            &encoded,
        )
    }

    /// Construct a body for a closed internal protocol owner.
    pub(super) fn frame<'a>(
        &self,
        kind: u8,
        oracle: u8,
        round: u8,
        level: u32,
        position: u32,
        output_bytes: usize,
        fields: BodyFields<'a>,
    ) -> Frame<'a> {
        Frame {
            kind,
            oracle,
            round,
            level,
            position,
            output_bytes: output_bytes
                .try_into()
                .expect("fixed candidate tape lengths fit u32"),
            fields,
        }
    }

    /// Hash a canonical complete leaf under its full context and exact position.
    pub(super) fn hash_leaf(&self, oracle: Oracle, index: u32, payload: &[u8]) -> Result<Digest> {
        let (role, round, leaves, bytes) = oracle.shape()?;
        if index >= leaves
            || payload.len() != bytes
            || payload.chunks_exact(8).any(|chunk| {
                u64::from_le_bytes(chunk.try_into().expect("exact eight-byte chunk")) >= MODULUS
            })
        {
            return Err(CandidateError::Tree);
        }
        self.hash_frame(&self.frame(
            1,
            role,
            round,
            0,
            index,
            H_OUTPUT_BYTES,
            BodyFields::One(payload),
        ))
    }

    /// Hash one valid parent; the sole terminal parent duplicates its child.
    pub(super) fn hash_parent(
        &self,
        oracle: Oracle,
        level: u32,
        index: u32,
        left: Digest,
        right: Digest,
    ) -> Result<Digest> {
        let (role, round, leaves, _) = oracle.shape()?;
        let depth = leaves.ilog2().max(1);
        if level == 0
            || level > depth
            || index >= (leaves >> level).max(1)
            || (leaves == 1 && left != right)
        {
            return Err(CandidateError::Tree);
        }
        self.hash_frame(&self.frame(
            2,
            role,
            round,
            level,
            index,
            H_OUTPUT_BYTES,
            BodyFields::Two(&left.to_le_bytes(), &right.to_le_bytes()),
        ))
    }

    fn challenge_frame<'a>(&self, round: Round, predecessor: &'a [u8; 48]) -> Frame<'a> {
        self.frame(
            4,
            0,
            round.0,
            0,
            0,
            round.tape_bytes(),
            BodyFields::One(predecessor),
        )
    }

    fn chain_frame<'a>(
        &self,
        round: Round,
        tape: &'a [u8],
        root: &'a [u8; 48],
    ) -> Result<Frame<'a>> {
        if round.0 == 22 {
            return Err(CandidateError::Phase);
        }
        if tape.len() != round.tape_bytes() {
            return Err(CandidateError::TapeLength);
        }
        Ok(self.frame(
            3,
            0,
            round.0,
            0,
            0,
            H_OUTPUT_BYTES,
            BodyFields::Two(tape, root),
        ))
    }
}

fn decode_message(round: Round, raw: &[u8]) -> Result<Message> {
    if raw.len() != round.tape_bytes() {
        return Err(CandidateError::TapeLength);
    }
    if raw
        .chunks_exact(8)
        .any(|chunk| u64::from_le_bytes(chunk.try_into().expect("exact field word")) >= MODULUS)
    {
        return Err(CandidateError::TapeEncoding);
    }
    if round.0 == 1 {
        return Ok(Message::Dummy);
    }
    if round.0 == 22 {
        let rejection_limit = MODULUS - MODULUS % u64::from(LDE_ROWS);
        let mut indices = Vec::with_capacity(QUERY_COUNT);
        for chunk in raw.chunks_exact(8).take(QUERY_CANDIDATES) {
            let candidate = u64::from_le_bytes(chunk.try_into().expect("exact field word"));
            if candidate >= rejection_limit {
                continue;
            }
            let value = (candidate % u64::from(LDE_ROWS)) as u32;
            match indices.binary_search(&value) {
                Ok(_) => {}
                Err(position) => indices.insert(position, value),
            }
            if indices.len() == QUERY_COUNT {
                return Ok(Message::Queries(indices));
            }
        }
        return Err(CandidateError::TapeExhausted);
    }
    let needed = round.field_coordinates().expect("validated field round");
    let values = raw[..needed * 8]
        .chunks_exact(32)
        .map(|group| {
            let coordinates = core::array::from_fn(|lane| {
                u64::from_le_bytes(
                    group[lane * 8..(lane + 1) * 8]
                        .try_into()
                        .expect("exact field word"),
                )
            });
            GoldilocksFp4V1::new(coordinates)
                .expect("the complete tape was canonical before decoding")
        })
        .collect();
    Ok(Message::Fields(values))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    Ready(Round),
    Pending { round: Round, raw: Vec<u8> },
    Complete,
    Aborted,
}

/// Fixed-anchor whole-message transcript; completion is not proof acceptance.
#[derive(Clone, Debug)]
pub(super) struct Transcript {
    context: Context,
    predecessor: Digest,
    phase: Phase,
}

impl Transcript {
    /// Start with the fixed zero anchor and the positive-tape dummy message.
    pub(super) fn new(context: Context) -> Self {
        Self {
            context,
            predecessor: Digest::default(),
            phase: Phase::Ready(Round(1)),
        }
    }

    /// Return the exact state preceding the next complete verifier message.
    #[cfg(test)]
    pub(super) fn predecessor(&self) -> Digest {
        self.predecessor
    }

    /// Sample and decode the next whole tape, retaining all its raw bytes.
    pub(super) fn challenge(&mut self) -> Result<Message> {
        self.challenge_with(|context, round, body, output| context.expand(round, body, output))
    }

    fn challenge_with(
        &mut self,
        fill: impl FnOnce(&Context, Round, &[u8], &mut [u8]) -> Result<()>,
    ) -> Result<Message> {
        let Phase::Ready(round) = self.phase else {
            return Err(CandidateError::Phase);
        };
        let prepared = (|| {
            let encoded = norito::encode_canonical(
                &self
                    .context
                    .challenge_frame(round, &self.predecessor.to_le_bytes()),
            )?;
            let mut raw = vec![0; round.tape_bytes()];
            fill(&self.context, round, &encoded, &mut raw)?;
            let message = decode_message(round, &raw)?;
            Ok::<_, CandidateError>((raw, message))
        })();
        match prepared {
            Ok((raw, message)) => {
                self.phase = if round.0 == 22 {
                    Phase::Complete
                } else {
                    Phase::Pending { round, raw }
                };
                Ok(message)
            }
            Err(error) => {
                self.phase = Phase::Aborted;
                Err(error)
            }
        }
    }

    /// Bind the complete pending tape and root before allowing the next round.
    pub(super) fn commit(&mut self, root: Digest) -> Result<()> {
        if !matches!(self.phase, Phase::Pending { .. }) {
            return Err(CandidateError::Phase);
        }
        let Phase::Pending { round, raw } = std::mem::replace(&mut self.phase, Phase::Aborted)
        else {
            unreachable!()
        };
        let root_bytes = root.to_le_bytes();
        let frame = self.context.chain_frame(round, &raw, &root_bytes)?;
        self.predecessor = self.context.hash_frame(&frame)?;
        self.phase = Phase::Ready(Round::new(round.0 + 1)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fastpq_isi::{GoldilocksDigestDomainV1, hash_bytes_384_v1};

    fn tape(round: Round, values: impl IntoIterator<Item = u64>) -> Vec<u8> {
        let mut output = vec![0; round.tape_bytes()];
        for (value, slot) in values.into_iter().zip(output.chunks_exact_mut(8)) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        output
    }

    #[test]
    fn fixed_deep_profile_reuses_one_shot_framing_without_changing_current_profile() {
        let current = Context::new(b"same statement bytes").unwrap();
        let deep = Context::new_deep(b"same statement bytes").unwrap();
        let expected_prefix = norito::encode_canonical(&PrefixFrame {
            version: 1,
            identity: super::super::deep_binding::IDENTITY.to_vec(),
            context: b"same statement bytes".to_vec(),
        })
        .unwrap();
        assert_eq!(deep.prefix.encoded.as_ref(), expected_prefix);
        assert_ne!(deep.prefix.encoded, current.prefix.encoded);
        for (role, phase, round, level) in [
            (H_ROLE, H_PHASE, 0, 23),
            (H_ROLE, H_PHASE, 9, 0),
            (G_ROLE, G_PHASE, 1, 0),
            (G_ROLE, G_PHASE, 10, 0),
        ] {
            let expected = hash_bytes_384_v1(
                GoldilocksDigestDomainV1 {
                    catalog: FASTPQ_CATALOG_V1.as_bytes(),
                    protocol: FASTPQ_FINAL_V1.name.as_bytes(),
                    profile: &expected_prefix,
                    role,
                    phase,
                    level: u64::from(level),
                    index: 7,
                    counter: u64::from(round),
                },
                &[b"same body"],
            )
            .unwrap();
            assert_eq!(
                deep.digest(role, phase, round, level, 7, b"same body")
                    .unwrap(),
                expected
            );
        }
        for (role, phase, round, level) in [
            (H_ROLE, H_PHASE, 10, 0),
            (H_ROLE, H_PHASE, 0, 24),
            (G_ROLE, G_PHASE, 0, 0),
            (G_ROLE, G_PHASE, 11, 0),
            (G_ROLE, G_PHASE, 1, 1),
            (H_ROLE, G_PHASE, 1, 0),
        ] {
            assert!(deep.cached_prefix(role, phase, round, level).is_err());
        }
        assert!(current.cached_prefix(H_ROLE, H_PHASE, 0, 23).is_err());
        assert!(current.cached_prefix(G_ROLE, G_PHASE, 22, 0).is_ok());
        assert!(deep.cached_prefix(G_ROLE, G_PHASE, 22, 0).is_err());
        assert!(
            current
                .expand_deep(
                    super::super::deep_binding::Round::new(1).unwrap(),
                    b"",
                    &mut [0; 48]
                )
                .is_err()
        );
    }

    #[test]
    fn fixed_cache_geometry_is_total_injective_and_rejects_every_other_role() {
        let mut seen = [false; PREFIX_CACHE_SLOTS];
        for round in 0..22 {
            for level in 0..20 {
                let slot = Context::cache_slot(H_ROLE, H_PHASE, round, level).unwrap();
                assert!(!seen[slot]);
                seen[slot] = true;
            }
        }
        for round in 1..=22 {
            let slot = Context::cache_slot(G_ROLE, G_PHASE, round, 0).unwrap();
            assert!(!seen[slot]);
            seen[slot] = true;
        }
        assert_eq!(PREFIX_CACHE_SLOTS, 462);
        assert!(seen.into_iter().all(|entry| entry));
        for (role, phase, round, level) in [
            (H_ROLE, H_PHASE, 22, 0),
            (H_ROLE, H_PHASE, 0, 20),
            (H_ROLE, H_PHASE, u8::MAX, u32::MAX),
            (G_ROLE, G_PHASE, 0, 0),
            (G_ROLE, G_PHASE, 23, 0),
            (G_ROLE, G_PHASE, 1, 1),
            (G_ROLE, H_PHASE, 1, 0),
            (H_ROLE, G_PHASE, 1, 0),
            (b"".as_slice(), H_PHASE, 1, 0),
        ] {
            assert!(Context::cache_slot(role, phase, round, level).is_err());
        }
    }

    #[test]
    fn cached_context_preserves_full_profile_and_protocol_with_concurrent_reuse() {
        let bytes = vec![0x9d; MAX_CONTEXT_BYTES];
        let context = Context::new(&bytes).unwrap();
        assert!(
            context
                .prefix
                .states
                .iter()
                .all(|entry| entry.get().is_none())
        );
        assert_eq!(Arc::strong_count(&context.prefix.encoded), 1);
        let expected_frame = norito::encode_canonical(&PrefixFrame {
            version: 1,
            identity: IDENTITY.to_vec(),
            context: bytes,
        })
        .unwrap();
        assert_eq!(context.prefix.encoded.as_ref(), expected_frame);
        let selected = GoldilocksDigestDomainV1 {
            catalog: FASTPQ_CATALOG_V1.as_bytes(),
            protocol: FASTPQ_FINAL_V1.name.as_bytes(),
            profile: &context.prefix.encoded,
            role: H_ROLE,
            phase: H_PHASE,
            level: 19,
            index: u64::MAX,
            counter: 21,
        };
        let expected = hash_bytes_384_v1(selected, &[b"complete body"]).unwrap();
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let worker = context.clone();
                scope.spawn(move || {
                    for _ in 0..4 {
                        assert_eq!(
                            worker
                                .digest(H_ROLE, H_PHASE, 21, 19, u64::MAX, b"complete body")
                                .unwrap(),
                            expected
                        );
                    }
                });
            }
        });
        assert_eq!(
            context
                .prefix
                .states
                .iter()
                .filter(|entry| entry.get().is_some())
                .count(),
            1
        );
        assert_eq!(Arc::strong_count(&context.prefix.encoded), 2);
        let h = context.cached_prefix(H_ROLE, H_PHASE, 21, 19).unwrap();
        let g = context.cached_prefix(G_ROLE, G_PHASE, 22, 0).unwrap();
        assert_eq!(h.domain().profile.as_ptr(), context.prefix.encoded.as_ptr());
        assert_eq!(g.domain().profile.as_ptr(), context.prefix.encoded.as_ptr());
        assert_eq!(h.domain().protocol, FASTPQ_FINAL_V1.name.as_bytes());
        assert_eq!(g.domain().protocol, FASTPQ_FINAL_V1.name.as_bytes());
        assert_eq!(Arc::strong_count(&context.prefix.encoded), 3);
        assert!(std::ptr::eq(
            h,
            context.cached_prefix(H_ROLE, H_PHASE, 21, 19).unwrap()
        ));
        assert_eq!(
            context
                .prefix
                .states
                .iter()
                .filter(|entry| entry.get().is_some())
                .count(),
            2
        );
        assert!(context.cached_prefix(H_ROLE, H_PHASE, 22, 0).is_err());
        assert_eq!(
            context
                .prefix
                .states
                .iter()
                .filter(|entry| entry.get().is_some())
                .count(),
            2
        );
    }

    #[test]
    fn complete_schedule_and_context_limits_preserve_permanent_terminal_state() {
        assert!(matches!(Context::new(b""), Err(CandidateError::Context)));
        assert!(matches!(
            Context::new(&vec![0; MAX_CONTEXT_BYTES + 1]),
            Err(CandidateError::Context)
        ));
        for ordinal in [0, 23, u8::MAX] {
            assert!(matches!(Round::new(ordinal), Err(CandidateError::Round)));
        }
        let mut transcript = Transcript::new(Context::new(b"whole schedule context").unwrap());
        for ordinal in 1..=22 {
            let round = Round::new(ordinal).unwrap();
            let message = transcript
                .challenge_with(|_, got, _, output| {
                    assert_eq!(got, round);
                    assert_eq!(output.len(), round.tape_bytes());
                    output.copy_from_slice(&tape(round, 0..(round.tape_bytes() / 8) as u64));
                    Ok(())
                })
                .unwrap();
            if ordinal == 1 {
                assert_eq!(message, Message::Dummy);
            } else if ordinal == 22 {
                assert_eq!(message, Message::Queries((0..375).collect()));
                assert_eq!(transcript.phase, Phase::Complete);
            } else {
                assert!(matches!(message, Message::Fields(_)));
            }
            assert!(transcript.challenge().is_err());
            if ordinal < 22 {
                transcript
                    .commit(Digest::new([u64::from(ordinal); 6]).unwrap())
                    .unwrap();
                assert!(transcript.commit(Digest::default()).is_err());
            }
        }
        assert!(transcript.challenge().is_err());
        assert!(transcript.commit(Digest::default()).is_err());
        assert_eq!(transcript.phase, Phase::Complete);
    }

    #[test]
    fn field_schedule_and_canonical_coordinates_are_exact() {
        assert_eq!(
            (1..=22).map(|j| Round(j).tape_bytes()).sum::<usize>(),
            44_688
        );
        for j in 2..=21 {
            let round = Round(j);
            let needed = round.field_coordinates().unwrap();
            let raw = tape(round, (0..round.tape_bytes() / 8).map(|i| i as u64));
            let Message::Fields(fields) = decode_message(round, &raw).unwrap() else {
                panic!("fields");
            };
            assert_eq!(fields.len(), needed / 4);
            for (index, value) in fields.into_iter().enumerate() {
                assert_eq!(
                    value,
                    GoldilocksFp4V1::new(core::array::from_fn(|lane| (4 * index + lane) as u64))
                        .unwrap()
                );
            }
            let mut invalid = raw;
            let last = invalid.len() - 8;
            invalid[last..].copy_from_slice(&MODULUS.to_le_bytes());
            assert!(matches!(
                decode_message(round, &invalid),
                Err(CandidateError::TapeEncoding)
            ));
        }
    }

    #[test]
    fn typed_h_matches_the_canonical_one_shot_and_binds_full_context() {
        let context = Context::new(b"complete caller context").unwrap();
        let frame = context.frame(1, 1, 0, 0, 7, 48, BodyFields::One(&[0; 342 * 8]));
        let body = norito::encode_canonical(&frame).unwrap();
        let expected = hash_bytes_384_v1(
            GoldilocksDigestDomainV1 {
                catalog: FASTPQ_CATALOG_V1.as_bytes(),
                protocol: FASTPQ_FINAL_V1.name.as_bytes(),
                profile: &context.prefix.encoded,
                role: b"compact-commitment",
                phase: b"typed-h",
                level: 0,
                index: 7,
                counter: 0,
            },
            &[&body],
        )
        .unwrap();
        assert_eq!(context.hash_frame(&frame).unwrap(), expected);
        assert_ne!(
            Context::new(b"complete caller context changed")
                .unwrap()
                .hash_frame(&frame)
                .unwrap(),
            expected
        );
        assert_ne!(
            context.hash_leaf(Oracle::Row, 8, &[0; 342 * 8]).unwrap(),
            expected
        );
        let _flags = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(
            Context::new(b"complete caller context")
                .unwrap()
                .hash_frame(&frame)
                .unwrap(),
            expected
        );
    }

    #[test]
    fn every_g_block_matches_the_exact_typed_frame_and_index() {
        let context = Context::new(b"complete caller context").unwrap();
        let round = Round(4);
        let body = norito::encode_canonical(
            &context.challenge_frame(round, &Digest::default().to_le_bytes()),
        )
        .unwrap();
        let mut raw = vec![0; round.tape_bytes()];
        context.expand(round, &body, &mut raw).unwrap();
        for (index, bytes) in raw.chunks_exact(48).enumerate() {
            let expected = hash_bytes_384_v1(
                GoldilocksDigestDomainV1 {
                    catalog: FASTPQ_CATALOG_V1.as_bytes(),
                    protocol: FASTPQ_FINAL_V1.name.as_bytes(),
                    profile: &context.prefix.encoded,
                    role: b"compact-transcript",
                    phase: b"whole-field-tape-block",
                    level: 0,
                    index: index as u64,
                    counter: 4,
                },
                &[&body],
            )
            .unwrap();
            assert_eq!(bytes, expected.to_le_bytes());
        }
        assert_ne!(&raw[..48], &raw[48..]);
    }

    #[test]
    fn unused_canonical_coordinates_remain_in_the_chain() {
        let context = Context::new(b"complete caller context").unwrap();
        let round = Round(4);
        let a = tape(round, 0..12);
        let mut b = a.clone();
        b[11 * 8..12 * 8].copy_from_slice(&999_u64.to_le_bytes());
        assert_eq!(
            decode_message(round, &a).unwrap(),
            decode_message(round, &b).unwrap()
        );
        let root = Digest::new([1; 6]).unwrap();
        assert_ne!(
            context
                .hash_frame(&context.chain_frame(round, &a, &root.to_le_bytes()).unwrap())
                .unwrap(),
            context
                .hash_frame(&context.chain_frame(round, &b, &root.to_le_bytes()).unwrap())
                .unwrap()
        );
    }

    #[test]
    fn query_401_is_used_and_last_six_lane_suffix_is_only_decoding_padding() {
        let round = Round(22);
        let mut words = vec![0; 402];
        for (i, word) in words.iter_mut().enumerate().take(374) {
            *word = i as u64;
        }
        words[400] = 374;
        words[401] = MODULUS - 1;
        let raw = tape(round, words.clone());
        assert_eq!(
            decode_message(round, &raw).unwrap(),
            Message::Queries((0..375).collect())
        );
        words[400] = MODULUS - 1;
        assert!(matches!(
            decode_message(round, &tape(round, words)),
            Err(CandidateError::TapeExhausted)
        ));
        assert!(matches!(
            decode_message(round, &raw[..raw.len() - 8]),
            Err(CandidateError::TapeLength)
        ));
    }

    #[test]
    fn sampler_abort_is_permanent_and_wrong_phase_never_retries() {
        let mut transcript = Transcript::new(Context::new(b"complete caller context").unwrap());
        assert!(transcript.commit(Digest::default()).is_err());
        transcript.phase = Phase::Ready(Round(22));
        let result = transcript.challenge_with(|_, _, _, output| {
            output.fill(0);
            Ok(())
        });
        assert!(matches!(result, Err(CandidateError::TapeExhausted)));
        assert_eq!(transcript.phase, Phase::Aborted);
        assert!(transcript.challenge().is_err());
        assert!(transcript.commit(Digest::default()).is_err());
    }

    #[test]
    fn terminal_root_keeps_the_single_leaf_duplicate_parent_and_geometry() {
        let context = Context::new(b"complete caller context").unwrap();
        let leaf = context.hash_leaf(Oracle::Fri(17), 0, &[0; 128]).unwrap();
        let root = context
            .hash_parent(Oracle::Fri(17), 1, 0, leaf, leaf)
            .unwrap();
        assert_ne!(root, leaf);
        assert!(
            context
                .hash_parent(Oracle::Fri(17), 1, 1, leaf, leaf)
                .is_err()
        );
        assert!(
            context
                .hash_parent(Oracle::Fri(17), 1, 0, leaf, Digest::default())
                .is_err()
        );
        assert!(context.hash_leaf(Oracle::Fri(17), 0, &[0; 64]).is_err());
        assert!(context.hash_leaf(Oracle::Fri(18), 0, &[0; 128]).is_err());
    }
    #[test]
    fn independent_full_context_dummy_tape_and_first_chain_known_answers() {
        // Python independently encoded all17 StatementContext fields and the
        // exact canonical frame. These are fixed external expected bytes.
        let statement=hex::decode("4e52543000002927e8b7bbf54c7a8c5ba0c9a3967f7d00b9000000000000008a36ae75d966e4b402232270726f66696c652d62696e64696e673a66697865642d63616e6469646174653a7631040000010004000008000456010000049b0300000801000000ffffffff08070000000000000008136edf57a368c4a904130000000846e98ea9f9690efd040800000004020000000411000000040400000004010000000477010000393100000000000000636f6d706c657465207075626c696320636f6e7465787420776974686f757420612070726976617465207769746e657373").unwrap();
        let context = Context::new(&statement).unwrap();
        let round = Round::new(1).unwrap();
        let body = norito::encode_canonical(
            &context.challenge_frame(round, &Digest::default().to_le_bytes()),
        )
        .unwrap();
        let mut tape = vec![0; round.tape_bytes()];
        context.expand(round, &body, &mut tape).unwrap();
        assert_eq!(
            hex::encode(&tape),
            "b4454c874d563358bc676f85551a9821c89074a265513fdd0a0d0da40b073826ee40df1bb65b7ae006aaceeb334ee770"
        );
        let mut transcript = Transcript::new(context);
        assert_eq!(transcript.challenge().unwrap(), Message::Dummy);
        assert_eq!(transcript.predecessor(), Digest::default());
        transcript.commit(Digest::default()).unwrap();
        assert_eq!(
            hex::encode(transcript.predecessor().to_le_bytes()),
            "b75fac39358455b1f84f7ba13ed581d7cf1301afa5aac0509a285816c272f8882949a842e18415391f314053ac29a9c2"
        );
    }

    // Test-only copy of the owned field shape; never a production encoder or decoder.
    #[derive(NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(
        name = "fastpq_prover::backend::compact_v1::tests::OwnedBodyReference",
        frame = "fastpq_prover::compact_v1::BodyV1"
    )]
    struct OwnedBodyReference {
        kind: u8,
        oracle: u8,
        round: u8,
        level: u32,
        position: u32,
        output_bytes: u32,
        fields: Vec<Vec<u8>>,
    }

    fn owned_body_reference(frame: &Frame<'_>) -> OwnedBodyReference {
        let fields = match frame.fields {
            BodyFields::One(field) => vec![field.to_vec()],
            BodyFields::Two(first, second) => vec![first.to_vec(), second.to_vec()],
        };
        OwnedBodyReference {
            kind: frame.kind,
            oracle: frame.oracle,
            round: frame.round,
            level: frame.level,
            position: frame.position,
            output_bytes: frame.output_bytes,
            fields,
        }
    }

    fn one_shot_owned_h(context: &Context, reference: &OwnedBodyReference) -> Digest {
        let body = norito::encode_canonical(reference).unwrap();
        hash_bytes_384_v1(
            GoldilocksDigestDomainV1 {
                catalog: FASTPQ_CATALOG_V1.as_bytes(),
                protocol: FASTPQ_FINAL_V1.name.as_bytes(),
                profile: &context.prefix.encoded,
                role: b"compact-commitment",
                phase: b"typed-h",
                level: u64::from(reference.level),
                index: u64::from(reference.position),
                counter: u64::from(reference.round),
            },
            &[&body],
        )
        .unwrap()
    }

    fn valid_layouts() -> impl Iterator<Item = u8> {
        (0..=norito::core::supported_header_flags())
            .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    }

    fn assert_owned_body_bytes(frame: &Frame<'_>) {
        let owned = owned_body_reference(frame);
        let expected = norito::encode_canonical(&owned).unwrap();
        // Comparing the entire frame includes schema, flags, lengths and CRC64.
        for flags in valid_layouts() {
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(norito::encode_canonical(frame).unwrap(), expected);
            assert_eq!(norito::canonical_frame_len(frame).unwrap(), expected.len());
        }
    }

    #[test]
    fn borrowed_byte_fields_match_vector_payloads_for_all_layouts_and_boundaries() {
        use norito::core::SerializePayload;
        for flags in valid_layouts() {
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            for len in [0, 1, 7, 8, 48, 119, 120, 127, 128, 2736, 29_568] {
                let bytes: Vec<u8> = (0..len).map(|index| index as u8).collect();
                let byte_field = ByteField(&bytes);
                let mut actual = Vec::new();
                let mut expected = Vec::new();
                norito::core::serialize_to_buffer(&byte_field, &mut actual).unwrap();
                norito::core::serialize_to_buffer(&bytes, &mut expected).unwrap();
                assert_eq!(actual, expected, "byte count {len}, flags {flags:#x}");
                assert_eq!(byte_field.encoded_len_hint(), Some(expected.len()));
                assert_eq!(byte_field.encoded_len_exact(), Some(expected.len()));
                assert_eq!(
                    norito::core::encoded_payload_len(&byte_field).unwrap(),
                    expected.len()
                );
                for fields in [BodyFields::One(&bytes), BodyFields::Two(&bytes, b"second")] {
                    let owned = match fields {
                        BodyFields::One(first) => vec![first.to_vec()],
                        BodyFields::Two(first, second) => vec![first.to_vec(), second.to_vec()],
                    };
                    actual.clear();
                    expected.clear();
                    norito::core::serialize_to_buffer(&fields, &mut actual).unwrap();
                    norito::core::serialize_to_buffer(&owned, &mut expected).unwrap();
                    assert_eq!(
                        actual, expected,
                        "field count, bytes {len}, flags {flags:#x}"
                    );
                    assert_eq!(
                        norito::core::encoded_payload_len(&fields).unwrap(),
                        expected.len()
                    );
                }
            }
        }
    }

    #[test]
    fn borrowed_body_frames_preserve_every_canonical_shape_and_identity() {
        assert_eq!(
            <Frame<'_> as norito::NoritoSchema>::nominal_name(),
            "fastpq_prover::backend::compact_v1::Frame"
        );
        assert_eq!(
            <Frame<'_> as norito::NoritoSchema>::frame_name(),
            "fastpq_prover::compact_v1::BodyV1"
        );
        let context = Context::new(b"borrowed frame coverage").unwrap();
        let left_digest = Digest::new([MODULUS - 1; 6]).unwrap();
        let right_digest = Digest::new([1, 2, 3, 4, 5, 6]).unwrap();
        let left = left_digest.to_le_bytes();
        let right = right_digest.to_le_bytes();
        // Independently declare expected coordinates and payload lengths rather
        // than deriving them from the dispatcher's Oracle::shape implementation.
        let oracles = [
            (Oracle::Row, 1, 0, 524_288_u32, 342 * 8),
            (Oracle::Mixed, 2, 0, 524_288, 32),
            (Oracle::Quotient, 3, 0, 524_288, 32),
        ]
        .into_iter()
        .chain((0..=17).map(|round| {
            if round == 17 {
                (Oracle::Fri(round), 4, round, 1, 128)
            } else {
                (Oracle::Fri(round), 4, round, 524_288 >> (round + 1), 64)
            }
        }));
        for (oracle, role, round, leaves, bytes) in oracles {
            let payload: Vec<u8> = (0..bytes / 8)
                .flat_map(|column| {
                    let word = if column == 0 {
                        MODULUS - 1
                    } else {
                        column as u64
                    };
                    word.to_le_bytes()
                })
                .collect();
            for position in [0, leaves - 1] {
                let frame = context.frame(
                    1,
                    role,
                    round,
                    0,
                    position,
                    H_OUTPUT_BYTES,
                    BodyFields::One(&payload),
                );
                let BodyFields::One(borrowed) = frame.fields else {
                    unreachable!()
                };
                assert!(std::ptr::eq(borrowed, payload.as_slice()));
                assert_owned_body_bytes(&frame);
                let reference = OwnedBodyReference {
                    kind: 1,
                    oracle: role,
                    round,
                    level: 0,
                    position,
                    output_bytes: 48,
                    fields: vec![payload.clone()],
                };
                assert_eq!(
                    context.hash_leaf(oracle, position, &payload).unwrap(),
                    one_shot_owned_h(&context, &reference),
                    "leaf {oracle:?}, position {position}"
                );
            }
            for level in [1, leaves.ilog2().max(1)] {
                // Every nonterminal case uses unequal children, and the sole
                // terminal parent uses the required duplicate child.
                let second_digest = if leaves == 1 {
                    left_digest
                } else {
                    right_digest
                };
                let second = second_digest.to_le_bytes();
                for position in [0, (leaves >> level).max(1) - 1] {
                    assert_owned_body_bytes(&context.frame(
                        2,
                        role,
                        round,
                        level,
                        position,
                        H_OUTPUT_BYTES,
                        BodyFields::Two(&left, &second),
                    ));
                    let reference = OwnedBodyReference {
                        kind: 2,
                        oracle: role,
                        round,
                        level,
                        position,
                        output_bytes: 48,
                        fields: vec![left.to_vec(), second.to_vec()],
                    };
                    assert_eq!(
                        context
                            .hash_parent(oracle, level, position, left_digest, second_digest)
                            .unwrap(),
                        one_shot_owned_h(&context, &reference),
                        "parent {oracle:?}, level {level}, position {position}"
                    );
                }
            }
        }
        for ordinal in 1..=22 {
            let round = Round::new(ordinal).unwrap();
            assert_owned_body_bytes(&context.challenge_frame(round, &left));
            if ordinal < 22 {
                let raw = tape(round, (0..round.tape_bytes() / 8).map(|index| index as u64));
                assert_owned_body_bytes(&context.chain_frame(round, &raw, &right).unwrap());
            }
        }
    }

    #[test]
    fn borrowed_body_frame_matches_fixed_header_and_checksum_known_answer() {
        // Independently encoded canonical BodyV1 request for round one's zero anchor.
        let context = Context::new(b"frame KAT context").unwrap();
        let predecessor = [0; 48];
        let frame = context.challenge_frame(Round(1), &predecessor);
        let expected = hex::decode("4e52543000009852866667ad6340eb539f01d0318999005700000000000000d7b9df9f0869d6bb02010401000101040000000004000000000430000000410100000000000000383000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap();
        assert_eq!(expected.len(), 127);
        assert_eq!(norito::encode_canonical(&frame).unwrap(), expected);
    }

    #[test]
    fn borrowed_chain_retains_the_owned_pending_tape_until_commit() {
        let context = Context::new(b"owned pending tape").unwrap();
        let mut transcript = Transcript::new(context.clone());
        let mut original_address = std::ptr::null();
        transcript
            .challenge_with(|_, _, _, output| {
                original_address = output.as_ptr();
                output.copy_from_slice(&tape(Round(1), [1, 2, 3, 4, 5, MODULUS - 1]));
                Ok(())
            })
            .unwrap();
        let root = Digest::new([7; 6]).unwrap();
        let root_bytes = root.to_le_bytes();
        let Phase::Pending { round, raw } = &transcript.phase else {
            panic!("pending")
        };
        assert_eq!(raw.as_ptr(), original_address);
        let frame = context.chain_frame(*round, raw, &root_bytes).unwrap();
        let BodyFields::Two(borrowed, borrowed_root) = frame.fields else {
            unreachable!()
        };
        assert!(std::ptr::eq(borrowed, raw.as_slice()));
        assert!(std::ptr::eq(borrowed_root, root_bytes.as_slice()));
        assert_owned_body_bytes(&frame);
        let expected = context.hash_frame(&frame).unwrap();
        transcript.commit(root).unwrap();
        assert_eq!(transcript.predecessor(), expected);
        assert_eq!(transcript.phase, Phase::Ready(Round(2)));
    }

    #[test]
    fn borrowed_chain_rejects_wrong_tape_lengths_and_terminal_round() {
        let context = Context::new(b"chain shape rejection").unwrap();
        let root = [0; 48];
        for ordinal in 1..22 {
            let round = Round(ordinal);
            for len in [0, round.tape_bytes() - 1, round.tape_bytes() + 1] {
                assert!(matches!(
                    context.chain_frame(round, &vec![0; len], &root),
                    Err(CandidateError::TapeLength)
                ));
            }
        }
        assert!(matches!(
            context.chain_frame(Round(22), &vec![0; Round(22).tape_bytes()], &root),
            Err(CandidateError::Phase)
        ));
    }
}
