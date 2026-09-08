//! Fixed SHAKE commitment and whole-tape transcript candidate for offline verification.
//!
//! The profile is not admitted. Full canonical public context is bound into
//! the prefix of every typed hash input; construction does not authenticate that context.
//! TODO: Review the projected-XOF assumptions, connect the bounded AIR verifier,
//! and qualify proof/resource/hardware behavior before production admission.

use std::sync::Arc;

use fastpq_isi::GoldilocksDigest384V1 as Digest;
use iroha_crypto::xof::Shake256Prefix;
#[cfg(test)]
use iroha_crypto::xof::shake256_into;
use norito::NoritoSerialize;

use crate::field::{GOLDILOCKS_MODULUS_V1 as MODULUS, GoldilocksFp4V1};

pub(super) const IDENTITY: &[u8] =
    b"fastpq:compact-shake256:h16:g375:c401:342cols:923slots:65536rows:8blowup:17folds:prefix-body:v1";
const MAX_CONTEXT_BYTES: usize = 256 * 1024;
const LDE_ROWS: u32 = 524_288;
const QUERY_COUNT: usize = 375;
const QUERY_CANDIDATES: usize = 401;
const QUERY_LABEL_BITS: usize = 19;
const QUERY_TAPE_BYTES: usize = (QUERY_CANDIDATES * QUERY_LABEL_BITS).div_ceil(8);
const H_TAPE_BYTES: usize = 128;

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

    /// Return the one-based verifier-message ordinal.
    #[cfg(test)]
    pub(super) const fn ordinal(self) -> u8 {
        self.0
    }

    /// Fixed whole raw-tape length, including unused samples and suffix bytes.
    pub(super) const fn tape_bytes(self) -> usize {
        match self.0 {
            1 => 48,
            2 => 10_992,
            3 => 29_584,
            4 => 112,
            5..=21 => 80,
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

/// Immutable, bounded full public context and its unfinished SHAKE state.
///
/// Every logical oracle input is the complete canonical prefix followed by one
/// canonical body. State reuse changes physical work, never those input bytes.
#[derive(Clone, Debug)]
pub(super) struct Context {
    prefix: Arc<AbsorbedPrefix>,
}

#[derive(Debug)]
struct AbsorbedPrefix {
    #[cfg(test)]
    encoded: Vec<u8>,
    state: Shake256Prefix,
}

// Two independently framed values allow prefix absorption to be reused. Each
// header advertises its own canonical schema, layout, byte length and CRC. The
// full context is present in P; neither P nor any raw tape is replaced by a hash.
#[derive(Clone, Debug, NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_candidate::ShakePrefixV1")]
struct PrefixFrame {
    version: u16,
    identity: Vec<u8>,
    context: Vec<u8>,
}

#[derive(Clone, Debug, NoritoSerialize)]
#[norito(schema_name = "fastpq_prover::compact_candidate::ShakeBodyV1")]
struct Frame {
    kind: u8,
    oracle: u8,
    round: u8,
    level: u32,
    position: u32,
    output_bytes: u32,
    fields: Vec<Vec<u8>>,
}

impl Context {
    /// Canonically frame and absorb one bounded immutable complete context.
    pub(super) fn new(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > MAX_CONTEXT_BYTES {
            return Err(CandidateError::Context);
        }
        let encoded = norito::encode_canonical(&PrefixFrame {
            version: 1,
            identity: IDENTITY.to_vec(),
            context: bytes.to_vec(),
        })?;
        let state = Shake256Prefix::new(&[&encoded]);
        Ok(Self {
            prefix: Arc::new(AbsorbedPrefix {
                #[cfg(test)]
                encoded,
                state,
            }),
        })
    }

    fn expand(&self, body: &[u8], output: &mut [u8]) {
        self.prefix.state.expand_into(&[body], output);
    }

    fn hash_frame(&self, frame: &Frame) -> Result<Digest> {
        let encoded = norito::encode_canonical(frame)?;
        let mut raw = [0; H_TAPE_BYTES];
        self.expand(&encoded, &mut raw);
        decode_root(&raw)
    }

    fn frame(
        &self,
        kind: u8,
        oracle: u8,
        round: u8,
        level: u32,
        position: u32,
        output_bytes: usize,
        fields: Vec<Vec<u8>>,
    ) -> Frame {
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
            H_TAPE_BYTES,
            vec![payload.to_vec()],
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
            H_TAPE_BYTES,
            vec![left.to_le_bytes().to_vec(), right.to_le_bytes().to_vec()],
        ))
    }

    fn challenge_frame(&self, round: Round, predecessor: Digest) -> Frame {
        self.frame(
            4,
            0,
            round.0,
            0,
            0,
            round.tape_bytes(),
            vec![predecessor.to_le_bytes().to_vec()],
        )
    }

    fn chain_frame(&self, round: Round, tape: Vec<u8>, root: Digest) -> Result<Frame> {
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
            H_TAPE_BYTES,
            vec![tape, root.to_le_bytes().to_vec()],
        ))
    }
}

fn decode_root(raw: &[u8]) -> Result<Digest> {
    if raw.len() != H_TAPE_BYTES {
        return Err(CandidateError::TapeLength);
    }
    let mut coordinates = [0; 6];
    let mut used = 0;
    for chunk in raw.chunks_exact(8) {
        let candidate = u64::from_le_bytes(chunk.try_into().expect("exact eight-byte chunk"));
        if candidate < MODULUS {
            coordinates[used] = candidate;
            used += 1;
            if used == coordinates.len() {
                return Ok(Digest::new(coordinates).expect("all selected values are canonical"));
            }
        }
    }
    Err(CandidateError::TapeExhausted)
}

fn decode_message(round: Round, raw: &[u8]) -> Result<Message> {
    if raw.len() != round.tape_bytes() {
        return Err(CandidateError::TapeLength);
    }
    if round.0 == 1 {
        return Ok(Message::Dummy);
    }
    if round.0 == 22 {
        let mut indices = Vec::with_capacity(QUERY_COUNT);
        for candidate in 0..QUERY_CANDIDATES {
            let mut value = 0_u32;
            for bit in 0..QUERY_LABEL_BITS {
                let offset = QUERY_LABEL_BITS * candidate + bit;
                value |= u32::from((raw[offset / 8] >> (offset % 8)) & 1) << bit;
            }
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
    let mut values = Vec::with_capacity(needed / 4);
    let mut coordinates = [0; 4];
    let mut used = 0;
    for chunk in raw.chunks_exact(8) {
        let candidate = u64::from_le_bytes(chunk.try_into().expect("exact eight-byte chunk"));
        if candidate < MODULUS {
            coordinates[used] = candidate;
            used += 1;
            if used == 4 {
                values.push(GoldilocksFp4V1::new(coordinates).expect("canonical coordinates"));
                used = 0;
                if values.len() == needed / 4 {
                    return Ok(Message::Fields(values));
                }
            }
        }
    }
    Err(CandidateError::TapeExhausted)
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
        self.challenge_with(|context, body, output| context.expand(body, output))
    }

    fn challenge_with(&mut self, fill: impl FnOnce(&Context, &[u8], &mut [u8])) -> Result<Message> {
        let Phase::Ready(round) = self.phase else {
            return Err(CandidateError::Phase);
        };
        let prepared = (|| {
            let encoded =
                norito::encode_canonical(&self.context.challenge_frame(round, self.predecessor))?;
            let mut raw = vec![0; round.tape_bytes()];
            fill(&self.context, &encoded, &mut raw);
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
        self.commit_with(root, |context, body, output| context.expand(body, output))
    }

    fn commit_with(
        &mut self,
        root: Digest,
        fill: impl FnOnce(&Context, &[u8], &mut [u8]),
    ) -> Result<()> {
        if !matches!(self.phase, Phase::Pending { .. }) {
            return Err(CandidateError::Phase);
        }
        let Phase::Pending { round, raw } = std::mem::replace(&mut self.phase, Phase::Aborted)
        else {
            unreachable!()
        };
        let frame = self.context.chain_frame(round, raw, root)?;
        let encoded = norito::encode_canonical(&frame)?;
        let mut output = [0; H_TAPE_BYTES];
        fill(&self.context, &encoded, &mut output);
        self.predecessor = decode_root(&output)?;
        self.phase = Phase::Ready(Round::new(round.0 + 1)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> Context {
        Context::new(b"fixed public candidate context").unwrap()
    }

    fn pack_indices(indices: impl IntoIterator<Item = u32>) -> Vec<u8> {
        let mut raw = vec![0; QUERY_TAPE_BYTES];
        for (position, value) in indices.into_iter().take(QUERY_CANDIDATES).enumerate() {
            assert!(value < LDE_ROWS);
            for bit in 0..QUERY_LABEL_BITS {
                raw[(QUERY_LABEL_BITS * position + bit) / 8] |=
                    (((value >> bit) & 1) as u8) << ((QUERY_LABEL_BITS * position + bit) % 8);
            }
        }
        raw
    }

    #[test]
    fn fixed_tape_table_and_context_caps_are_exact() {
        assert!(Context::new(&[]).is_err());
        assert!(Context::new(&vec![0; MAX_CONTEXT_BYTES + 1]).is_err());
        assert!(Context::new(&vec![0; MAX_CONTEXT_BYTES]).is_ok());
        assert!(Round::new(0).is_err());
        assert!(Round::new(23).is_err());
        let rounds: Vec<_> = (1..=22).map(|i| Round::new(i).unwrap()).collect();
        assert_eq!(rounds.iter().map(|r| r.tape_bytes()).sum::<usize>(), 43_049);
        for (i, round) in rounds.iter().enumerate() {
            assert_eq!(usize::from(round.ordinal()), i + 1);
            if let Some(required) = round.field_coordinates() {
                assert_eq!(round.tape_bytes(), (required + 6) * 8);
            }
        }
    }

    #[test]
    fn root_rejection_uses_first_six_canonical_words_and_explicit_abort() {
        let mut raw = [0; H_TAPE_BYTES];
        for chunk in raw.chunks_exact_mut(8) {
            chunk.copy_from_slice(&MODULUS.to_le_bytes());
        }
        assert!(matches!(
            decode_root(&raw),
            Err(CandidateError::TapeExhausted)
        ));
        for (i, value) in [0, 1, MODULUS - 1, 4, 5, 6].iter().enumerate() {
            raw[(i + 10) * 8..(i + 11) * 8].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(
            decode_root(&raw).unwrap().words(),
            [0, 1, MODULUS - 1, 4, 5, 6]
        );
        assert!(matches!(
            decode_root(&raw[..127]),
            Err(CandidateError::TapeLength)
        ));
    }

    #[test]
    fn field_tapes_accept_late_values_reject_exhaustion_and_ignore_only_decoded_suffix() {
        for ordinal in 2..=21 {
            let round = Round::new(ordinal).unwrap();
            let mut raw = vec![0; round.tape_bytes()];
            for chunk in raw[..48].chunks_exact_mut(8) {
                chunk.copy_from_slice(&MODULUS.to_le_bytes());
            }
            let Message::Fields(fields) = decode_message(round, &raw).unwrap() else {
                panic!("field message");
            };
            assert_eq!(fields.len(), round.field_coordinates().unwrap() / 4);
            raw[48..56].copy_from_slice(&MODULUS.to_le_bytes());
            assert!(matches!(
                decode_message(round, &raw),
                Err(CandidateError::TapeExhausted)
            ));
            assert!(matches!(
                decode_message(round, &raw[..raw.len() - 1]),
                Err(CandidateError::TapeLength)
            ));
        }
        assert_eq!(
            decode_message(Round(1), &[0xff; 48]).unwrap(),
            Message::Dummy
        );
    }

    #[test]
    fn query_tape_is_exact_sorted_distinct_and_bounded_at_last_candidate() {
        let round = Round::new(22).unwrap();
        let raw = pack_indices(
            (0..375)
                .rev()
                .chain(std::iter::repeat_n(0, QUERY_CANDIDATES - QUERY_COUNT)),
        );
        assert_eq!(
            decode_message(round, &raw).unwrap(),
            Message::Queries((0..375).collect())
        );
        let late =
            pack_indices(std::iter::repeat_n(0, QUERY_CANDIDATES - QUERY_COUNT + 1).chain(1..375));
        assert_eq!(
            decode_message(round, &late).unwrap(),
            Message::Queries((0..375).collect())
        );
        assert!(matches!(
            decode_message(round, &[0; QUERY_TAPE_BYTES]),
            Err(CandidateError::TapeExhausted)
        ));
        assert!(matches!(
            decode_message(round, &[0; QUERY_TAPE_BYTES - 1]),
            Err(CandidateError::TapeLength)
        ));
        let boundary = pack_indices([LDE_ROWS - 1].into_iter().chain(0..374));
        let Message::Queries(values) = decode_message(round, &boundary).unwrap() else {
            panic!("query message");
        };
        assert_eq!(values.last(), Some(&(LDE_ROWS - 1)));
    }

    #[test]
    fn successor_query_sampler_consumes_candidate_401_and_ignores_only_five_padding_bits() {
        let round = Round::new(22).unwrap();
        assert_eq!(QUERY_COUNT, 375);
        assert_eq!(QUERY_CANDIDATES, 401);
        assert_eq!(QUERY_TAPE_BYTES, 953);
        assert_eq!(QUERY_CANDIDATES * QUERY_LABEL_BITS, 7619);
        assert_eq!((QUERY_CANDIDATES - 1) * QUERY_LABEL_BITS, 950 * 8);
        assert_eq!(
            QUERY_TAPE_BYTES * 8 - QUERY_CANDIDATES * QUERY_LABEL_BITS,
            5
        );
        assert!(
            IDENTITY
                .windows(b":g375:c401:".len())
                .any(|part| part == b":g375:c401:")
        );
        let first_400 = || (0..374).chain(std::iter::repeat_n(0, 26));
        let raw = pack_indices(first_400().chain([374]));
        let expected = Message::Queries((0..375).collect());
        assert_eq!(decode_message(round, &raw).unwrap(), expected);
        for padding in 0_u8..32 {
            let mut padded = raw.clone();
            padded[952] = (padded[952] & 7) | (padding << 3);
            assert_eq!(decode_message(round, &padded).unwrap(), expected);
        }
        let repeated = pack_indices(first_400().chain([0]));
        assert!(matches!(
            decode_message(round, &repeated),
            Err(CandidateError::TapeExhausted)
        ));
        // The last label spans bytes 950..=952; its final three bits are significant.
        let high = pack_indices(first_400().chain([LDE_ROWS - 1]));
        assert_eq!(
            decode_message(round, &high).unwrap(),
            Message::Queries((0..374).chain([LDE_ROWS - 1]).collect())
        );
        let mut changed = high;
        changed[952] ^= 4;
        assert_eq!(
            decode_message(round, &changed).unwrap(),
            Message::Queries((0..374).chain([LDE_ROWS - 1 - (1 << 18)]).collect())
        );
        for length in [950, 952, 954] {
            assert!(matches!(
                decode_message(round, &vec![0; length]),
                Err(CandidateError::TapeLength)
            ));
        }
    }

    #[test]
    fn tree_roles_positions_and_canonical_payloads_are_bound() {
        let c = context();
        let zero = Digest::default();
        let row = c.hash_leaf(Oracle::Row, 0, &[0; 342 * 8]).unwrap();
        assert_ne!(row, c.hash_leaf(Oracle::Row, 1, &[0; 342 * 8]).unwrap());
        assert_ne!(
            c.hash_leaf(Oracle::Mixed, 0, &[0; 32]).unwrap(),
            c.hash_leaf(Oracle::Quotient, 0, &[0; 32]).unwrap()
        );
        assert_ne!(
            c.hash_leaf(Oracle::Fri(0), 0, &[0; 64]).unwrap(),
            c.hash_leaf(Oracle::Fri(1), 0, &[0; 64]).unwrap()
        );
        assert!(c.hash_leaf(Oracle::Fri(18), 0, &[0; 64]).is_err());
        assert!(c.hash_leaf(Oracle::Row, LDE_ROWS, &[0; 342 * 8]).is_err());
        let mut invalid = [0; 32];
        invalid[..8].copy_from_slice(&MODULUS.to_le_bytes());
        assert!(c.hash_leaf(Oracle::Mixed, 0, &invalid).is_err());
        assert!(c.hash_leaf(Oracle::Mixed, 0, &[0; 31]).is_err());
        assert!(c.hash_parent(Oracle::Row, 0, 0, zero, zero).is_err());
        assert!(c.hash_parent(Oracle::Row, 20, 0, zero, zero).is_err());
        assert!(c.hash_parent(Oracle::Row, 19, 1, zero, zero).is_err());
        let terminal = c.hash_leaf(Oracle::Fri(17), 0, &[0; 128]).unwrap();
        assert!(c.hash_leaf(Oracle::Fri(17), 1, &[0; 128]).is_err());
        assert!(
            c.hash_parent(Oracle::Fri(17), 1, 0, terminal, zero)
                .is_err()
        );
        assert!(
            c.hash_parent(Oracle::Fri(17), 1, 0, terminal, terminal)
                .is_ok()
        );
        assert_ne!(
            c.hash_parent(Oracle::Row, 1, 0, row, zero).unwrap(),
            c.hash_parent(Oracle::Row, 1, 0, zero, row).unwrap()
        );
    }

    #[test]
    fn every_prefix_and_body_field_is_bound_with_canonical_boundaries() {
        let c = context();
        let base = c.frame(1, 2, 0, 0, 0, H_TAPE_BYTES, vec![vec![0; 32]]);
        let expected = c.hash_frame(&base).unwrap();
        let changes: [fn(&mut Frame); 7] = [
            |f| f.kind += 1,
            |f| f.oracle += 1,
            |f| f.round += 1,
            |f| f.level += 1,
            |f| f.position += 1,
            |f| f.output_bytes += 1,
            |f| f.fields[0][0] = 1,
        ];
        for change in changes {
            let mut other = base.clone();
            change(&mut other);
            assert_ne!(c.hash_frame(&other).unwrap(), expected);
        }
        let prefix = PrefixFrame {
            version: 1,
            identity: IDENTITY.to_vec(),
            context: b"fixed public candidate context".to_vec(),
        };
        let body = norito::encode_canonical(&base).unwrap();
        assert_eq!(norito::encode_canonical(&prefix).unwrap(), c.prefix.encoded);
        let changes: [fn(&mut PrefixFrame); 3] = [
            |f| f.version += 1,
            |f| f.identity.push(0),
            |f| f.context.push(0),
        ];
        for change in changes {
            let mut other = prefix.clone();
            change(&mut other);
            let encoded = norito::encode_canonical(&other).unwrap();
            let mut raw = [0; H_TAPE_BYTES];
            shake256_into(&[&encoded, &body], &mut raw);
            assert_ne!(decode_root(&raw).unwrap(), expected);
        }
        let mut boundary = base.clone();
        boundary.fields = vec![b"a".to_vec(), b"bc".to_vec()];
        let a = c.hash_frame(&boundary).unwrap();
        boundary.fields = vec![b"ab".to_vec(), b"c".to_vec()];
        assert_ne!(a, c.hash_frame(&boundary).unwrap());
        assert!(
            c.prefix
                .encoded
                .windows(prefix.context.len())
                .any(|part| part == prefix.context)
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(norito::encode_canonical(&prefix).unwrap(), c.prefix.encoded);
        assert_eq!(norito::encode_canonical(&base).unwrap(), body);
        assert_eq!(
            Context::new(&prefix.context)
                .unwrap()
                .hash_frame(&base)
                .unwrap(),
            expected
        );
    }

    #[test]
    fn cached_candidate_frames_match_full_inputs_for_every_round_and_tree_role() {
        for size in [1, 135, 136, 137, 271, 272, MAX_CONTEXT_BYTES] {
            let context_bytes: Vec<_> = (0..=255).cycle().take(size).collect();
            let c = Context::new(&context_bytes).unwrap();
            let mut frames = Vec::new();
            for oracle in [Oracle::Row, Oracle::Mixed, Oracle::Quotient]
                .into_iter()
                .chain((0..=17).map(Oracle::Fri))
            {
                let (role, round, leaves, bytes) = oracle.shape().unwrap();
                frames.push(c.frame(
                    1,
                    role,
                    round,
                    0,
                    leaves - 1,
                    H_TAPE_BYTES,
                    vec![vec![0; bytes]],
                ));
                frames.push(c.frame(
                    2,
                    role,
                    round,
                    leaves.ilog2().max(1),
                    0,
                    H_TAPE_BYTES,
                    vec![vec![0; 48], vec![0; 48]],
                ));
            }
            for ordinal in 1..=22 {
                let round = Round::new(ordinal).unwrap();
                frames.push(c.challenge_frame(round, Digest::default()));
                if ordinal != 22 {
                    frames.push(
                        c.chain_frame(round, vec![0; round.tape_bytes()], Digest::default())
                            .unwrap(),
                    );
                }
            }
            for frame in frames {
                let body = norito::encode_canonical(&frame).unwrap();
                let mut cold = vec![0; frame.output_bytes as usize];
                let mut cached = cold.clone();
                shake256_into(&[&c.prefix.encoded, &body], &mut cold);
                c.expand(&body, &mut cached);
                assert_eq!(
                    cold, cached,
                    "context={size}, kind={}, round={}",
                    frame.kind, frame.round
                );
            }
        }
    }

    #[test]
    fn committing_binds_unused_and_rejected_tape_bytes() {
        let c = context();
        let root = Digest::default();
        let round = Round::new(5).unwrap();
        let mut a = vec![0; round.tape_bytes()];
        let original = decode_message(round, &a).unwrap();
        let original_hash = c
            .hash_frame(&c.chain_frame(round, a.clone(), root).unwrap())
            .unwrap();
        a[79] = 1; // unused suffix after the first extension value
        assert_eq!(decode_message(round, &a).unwrap(), original);
        assert_ne!(
            c.hash_frame(&c.chain_frame(round, a, root).unwrap())
                .unwrap(),
            original_hash
        );
        let mut b = vec![0; round.tape_bytes()];
        b[..8].copy_from_slice(&MODULUS.to_le_bytes());
        let bh = c
            .hash_frame(&c.chain_frame(round, b.clone(), root).unwrap())
            .unwrap();
        b[..8].copy_from_slice(&(MODULUS + 1).to_le_bytes());
        assert_eq!(decode_message(round, &b).unwrap(), original);
        assert_ne!(
            c.hash_frame(&c.chain_frame(round, b, root).unwrap())
                .unwrap(),
            bh
        );
        assert!(
            c.chain_frame(Round(22), vec![0; QUERY_TAPE_BYTES], root)
                .is_err()
        );
        assert!(c.chain_frame(Round(1), vec![0; 47], root).is_err());
    }

    #[test]
    fn transcript_schedule_is_deterministic_and_wrong_phase_never_advances() {
        let mut a = Transcript::new(context());
        let mut b = a.clone();
        assert_eq!(a.predecessor(), Digest::default());
        assert!(matches!(
            a.commit(Digest::default()),
            Err(CandidateError::Phase)
        ));
        for ordinal in 1_u8..=22 {
            assert_eq!(a.challenge().unwrap(), b.challenge().unwrap());
            assert!(matches!(a.challenge(), Err(CandidateError::Phase)));
            if ordinal < 22 {
                let root = Digest::new([u64::from(ordinal); 6]).unwrap();
                a.commit(root).unwrap();
                b.commit(root).unwrap();
                assert_eq!(a.predecessor(), b.predecessor());
                assert!(matches!(a.commit(root), Err(CandidateError::Phase)));
                assert_eq!(a.predecessor(), b.predecessor());
            }
        }
        assert!(matches!(a.phase, Phase::Complete));
        assert!(matches!(
            a.commit(Digest::default()),
            Err(CandidateError::Phase)
        ));
        assert!(matches!(a.challenge(), Err(CandidateError::Phase)));
    }

    #[test]
    fn actual_decode_or_hash_abort_is_permanent_and_has_no_retry_counter() {
        let mut query = Transcript::new(context());
        query.phase = Phase::Ready(Round(22));
        assert!(matches!(
            query.challenge_with(|_, _, out| out.fill(0)),
            Err(CandidateError::TapeExhausted)
        ));
        assert!(matches!(query.phase, Phase::Aborted));
        assert!(query.challenge().is_err());
        let mut hash = Transcript::new(context());
        hash.challenge().unwrap();
        assert!(matches!(
            hash.commit_with(Digest::default(), |_, _, out| {
                for chunk in out.chunks_exact_mut(8) {
                    chunk.copy_from_slice(&MODULUS.to_le_bytes());
                }
            }),
            Err(CandidateError::TapeExhausted)
        ));
        assert!(matches!(hash.phase, Phase::Aborted));
        assert_eq!(hash.predecessor(), Digest::default());
        assert!(hash.challenge().is_err());
    }

    #[test]
    fn independent_norito_frame_and_shake_root_known_answer() {
        let c = context();
        let encoded =
            norito::encode_canonical(&c.frame(1, 2, 0, 0, 0, H_TAPE_BYTES, vec![vec![0; 32]]))
                .unwrap();
        assert_eq!(
            hex::encode(&c.prefix.encoded),
            "4e52543000004fdd12ac5e6affa7dc0020d925a07956009200000000000000263b808fe774e20b02020100675f000000000000006661737470713a636f6d706163742d7368616b653235363a6831363a673337353a633430313a333432636f6c733a393233736c6f74733a3635353336726f77733a38626c6f7775703a3137666f6c64733a7072656669782d626f64793a7631261e000000000000006669786564207075626c69632063616e64696461746520636f6e74657874"
        );
        assert_eq!(
            hex::encode(&encoded),
            "4e525430000085e95e1826c2b67068a640994478399e0047000000000000002109e3fab35cbe42020101010201000400000000040000000004800000003101000000000000002820000000000000000000000000000000000000000000000000000000000000000000000000000000"
        );
        let mut raw = [0; H_TAPE_BYTES];
        c.expand(&encoded, &mut raw);
        assert_eq!(
            hex::encode(raw),
            "15a77b743718a950590e1883df6ec7985f7d134e4042ad579df0104539ea7acd4a606b8a981d289b74592a968f821a66baf7a2d27489d6ddba39f13f7cedf3df335af74ba20774c75fb28477021cc011292bd8eb309c594cad6a97f26df72fd301b899d6cee0ffee6e9417c9b83bcb1ec341845e4b3f0c98fc60dc178882cd9a"
        );
        let root = c.hash_leaf(Oracle::Mixed, 0, &[0; 32]).unwrap();
        assert_eq!(
            hex::encode(root.to_le_bytes()),
            "15a77b743718a950590e1883df6ec7985f7d134e4042ad579df0104539ea7acd4a606b8a981d289b74592a968f821a66"
        );
    }

    #[test]
    fn canonical_input_lengths_keep_context_once_and_body_work_bounded() {
        let c = Context::new(&vec![0; MAX_CONTEXT_BYTES]).unwrap();
        assert_eq!(c.prefix.encoded.len(), 262_302);
        let row = c.frame(1, 1, 0, 0, 0, H_TAPE_BYTES, vec![vec![0; 342 * 8]]);
        assert_eq!(norito::encode_canonical(&row).unwrap().len(), 2817);
        let parent = c.frame(2, 1, 0, 1, 0, H_TAPE_BYTES, vec![vec![0; 48], vec![0; 48]]);
        assert_eq!(norito::encode_canonical(&parent).unwrap().len(), 184);
        let chain = c
            .chain_frame(Round(3), vec![0; Round(3).tape_bytes()], Digest::default())
            .unwrap();
        assert_eq!(norito::encode_canonical(&chain).unwrap().len(), 29_724);
        let g = c.challenge_frame(Round(3), Digest::default());
        assert_eq!(norito::encode_canonical(&g).unwrap().len(), 127);
        let clone = c.clone();
        assert!(Arc::ptr_eq(&c.prefix, &clone.prefix));
    }

    #[test]
    #[ignore = "bounded cold/cached SHAKE comparison; excludes proof construction and decoding"]
    fn prefix_hash_work_measurement_preserves_exact_outputs() {
        use std::{hint::black_box, time::Instant};
        for size in [30, MAX_CONTEXT_BYTES] {
            let start = Instant::now();
            let c = Context::new(&vec![0; size]).unwrap();
            let preparation = start.elapsed();
            let bodies: Vec<_> = (0..128)
                .map(|index| {
                    norito::encode_canonical(&c.frame(
                        1,
                        1,
                        0,
                        0,
                        index,
                        H_TAPE_BYTES,
                        vec![vec![0; 342 * 8]],
                    ))
                    .unwrap()
                })
                .collect();
            let start = Instant::now();
            let cold: Vec<_> = bodies
                .iter()
                .map(|body| {
                    let mut out = [0; H_TAPE_BYTES];
                    shake256_into(&[black_box(&c.prefix.encoded), black_box(body)], &mut out);
                    out
                })
                .collect();
            let cold_elapsed = start.elapsed();
            let start = Instant::now();
            let cached: Vec<_> = bodies
                .iter()
                .map(|body| {
                    let mut out = [0; H_TAPE_BYTES];
                    c.expand(black_box(body), &mut out);
                    out
                })
                .collect();
            let cached_elapsed = start.elapsed();
            assert_eq!(cold, cached);
            eprintln!(
                "SHAKE_PREFIX context_bytes={size} prefix_bytes={} body_bytes={} expansions={} state_bytes={} prepare_ns={} cold_ns={} cached_ns={}",
                c.prefix.encoded.len(),
                bodies[0].len(),
                bodies.len(),
                std::mem::size_of::<Shake256Prefix>(),
                preparation.as_nanos(),
                cold_elapsed.as_nanos(),
                cached_elapsed.as_nanos()
            );
        }
    }

    #[test]
    fn independent_dummy_tape_and_first_chained_state_known_answers() {
        let mut transcript = Transcript::new(context());
        assert_eq!(transcript.challenge().unwrap(), Message::Dummy);
        let Phase::Pending { round, raw } = &transcript.phase else {
            panic!("pending positive-length dummy tape");
        };
        assert_eq!(round.ordinal(), 1);
        assert_eq!(
            hex::encode(raw),
            "343f90e3854092b180adf8b291a03857a47c62af13de8314cf9cb178720c656e42dd41dc0e245aa07bcae130182d9342"
        );
        transcript.commit(Digest::default()).unwrap();
        assert_eq!(
            hex::encode(transcript.predecessor().to_le_bytes()),
            "11cae1e579a7989a33ecb7357e6fbc3c49ff1e2fc71e5aeafbc38704800205875cf4167029e93ea5159df935f7755190"
        );
    }
}
