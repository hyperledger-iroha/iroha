//! Unregistered typed state-packet permutation and sorted-consistency substrate.
//!
//! This component binds public qualification events to one native masked STARK.
//! It proves exact packet multiset equality, canonical typed cells, zero first
//! state, read preservation, strict address/time order and state continuity.
//! It does not authorize initialization, memory requests, frame ownership,
//! private inputs, register semantics or any complete IVM invocation.
// TODO: Connect these equations to the sole complete machine's constrained
// request/initializer/owner ports before any production verifier admission.

mod packet;
mod permutation;
mod sorted;

use super::{
    AggregateStarkDomainsV1, AggregateStarkParametersV1, F, GoldilocksDigest384V1,
    NOTE_COPY_AUX_WIDTH_V1, NOTE_COPY_FIXED_WIDTH_V1, NOTE_COPY_WIDTH_V1, NoteCopyCellPolicyV1,
    NoteCopyChallengesV1, NoteCopyScheduleV1, ProofManagedNoteStarkAdapterV1,
    ProofManagedNoteStarkErrorV1 as Error, ProofManagedNoteStarkProtocolV1, Sources,
    TransparentStarkDigestContextV1, TransparentTranscriptV1, bit, branch,
    goldilocks_digest384_frame_v1, wide, word,
};
use crate::execution_proofs::stark::transparent_stark::GoldilocksFp4V1 as E;

const PHASES: usize = 8;
const MIN_LOG: u8 = 13;
const MAX_LOG: u8 = 17;
const MAX_PACKETS: usize = (1 << MAX_LOG) / PHASES;
const ORDERED: usize = 0;
const SORTED: usize = ORDERED + packet::WIDTH;
const PREVIOUS: usize = SORTED + packet::WIDTH;
const PREV_KEY: usize = 0;
const PREV_CLOCK: usize = 1;
const PREV_PAYLOAD: usize = 2;
const PREV_TAG: usize = 10;
const PREV_VALID: usize = 11;
const PREV_WIDTH: usize = 12;
const SOURCES: usize = PREVIOUS + PREV_WIDTH;
const COMPARE: usize = SOURCES + word::WIDTH;
const CONTROLS: usize = COMPARE + branch::BANK_WIDTH;
const SAME_KEY: usize = CONTROLS;
const NEW_KEY: usize = SAME_KEY + 1;
const KEY_LESS: usize = NEW_KEY + 1;
const TIME_LESS: usize = KEY_LESS + 1;
const ORDERED_COUNT: usize = TIME_LESS + 1;
const SORTED_COUNT: usize = ORDERED_COUNT + 1;
const SORTED_ENDED: usize = SORTED_COUNT + 1;
const RESERVED: usize = SORTED_ENDED + 1;
const TYPES: usize = RESERVED + 1;
const ROW_WIDTH: usize = TYPES + 4;
const PHASE_OFFSET: usize = packet::WIDTH;
const TRANSITION: usize = PHASE_OFFSET + PHASES;
const FIRST: usize = TRANSITION + 1;
const LAST: usize = FIRST + 1;
const SLOT: usize = LAST + 1;
const TOTAL: usize = SLOT + 1;
const FIXED_WIDTH: usize = TOTAL + 1;
const CONSTRAINTS: usize = packet::WIDTH + sorted::CONSTRAINTS + permutation::CONSTRAINTS;
const CONTEXT: TransparentStarkDigestContextV1 =
    TransparentStarkDigestContextV1::execution_v1(b"ivm-machine-packet-bus-v1");
const DOMAINS: AggregateStarkDomainsV1 = AggregateStarkDomainsV1 {
    digest_context: CONTEXT,
    base_leaf: b"ivm-machine-bus-base-leaf-v1",
    base_node: b"ivm-machine-bus-base-node-v1",
    aux_leaf: b"ivm-machine-bus-aux-leaf-v1",
    aux_node: b"ivm-machine-bus-aux-node-v1",
    composition_leaf: b"ivm-machine-bus-composition-leaf-v1",
    composition_node: b"ivm-machine-bus-composition-node-v1",
    fri_leaf: b"ivm-machine-bus-fri-leaf-v1",
    fri_node: b"ivm-machine-bus-fri-node-v1",
    layout_label: b"ivm-machine-bus-layout-v1",
    base_root_label: b"ivm-machine-bus-base-root-v1",
    aux_root_label: b"ivm-machine-bus-aux-root-v1",
    composition_root_label: b"ivm-machine-bus-composition-root-v1",
    fri_root_label: b"ivm-machine-bus-fri-root-v1",
    fri_beta_label: b"ivm-machine-bus-fri-beta-v1",
    query_seed: b"ivm-machine-bus-query-seed-v1",
};
const PROFILE: &[u8] = b"ivm-machine-packet-bus-v1:unregistered-public-component:one-trace:rows=8192..131072:8-stages:16384-packets:26-full-tuple-fields:4-one-hot-spaces:u32-index:u16-generation:u8-vm:60bit-key:u32-clock:128bit-cell:16-private-bits:register64-tagbit0:init16:owner64:zero-initial-state:strict-key-time:read-preservation:post-base-fp4-products:all-zero-factor-risk-accounted:degree4:no-initializer-request-frame-invocation-authority";

/// Expected public component input; construction supplies no execution authority.
#[derive(Clone)]
struct PublicPacketBus {
    events: Vec<Option<packet::Event>>,
    trace_log2: u8,
    total: usize,
}

impl PublicPacketBus {
    fn new(events: Vec<Option<packet::Event>>) -> Result<Self, Error> {
        if events.len() > MAX_PACKETS {
            return Err(Error::InvalidProfile);
        }
        let rows = events.len().max(1) * PHASES;
        let trace_log2 = u8::try_from(rows.next_power_of_two().ilog2())
            .map_err(|_| Error::InvalidProfile)?
            .max(MIN_LOG);
        let total = events.iter().filter(|event| event.is_some()).count();
        Ok(Self {
            events,
            trace_log2,
            total,
        })
    }

    fn size(&self) -> usize {
        1 << self.trace_log2
    }

    fn total(&self) -> usize {
        self.total
    }

    fn ordered(&self, slot: usize) -> [F; packet::WIDTH] {
        self.events
            .get(slot)
            .and_then(Option::as_ref)
            .map_or([F::ZERO; packet::WIDTH], |event| event.fields(slot))
    }

    fn fixed(&self, index: usize) -> [F; FIXED_WIDTH] {
        let mut row = [F::ZERO; FIXED_WIDTH];
        row[..packet::WIDTH].copy_from_slice(&self.ordered(index / PHASES));
        row[PHASE_OFFSET + index % PHASES] = F::ONE;
        row[TRANSITION] = F(u64::from(index + 1 < self.size()));
        row[FIRST] = F(u64::from(index == 0));
        row[LAST] = F(u64::from(index + 1 == self.size()));
        row[SLOT] = F((index / PHASES) as u64);
        row[TOTAL] = F(self.total() as u64);
        row
    }

    fn digest(&self) -> Result<GoldilocksDigest384V1, Error> {
        let mut bytes = Vec::with_capacity(16 + self.events.len() * packet::WIDTH * 8);
        bytes.extend_from_slice(&(self.events.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&u64::from(self.trace_log2).to_le_bytes());
        for slot in 0..self.events.len() {
            for value in self.ordered(slot) {
                bytes.extend_from_slice(&value.0.to_le_bytes());
            }
        }
        goldilocks_digest384_frame_v1(
            CONTEXT,
            b"ivm-machine-bus-public-input-v1",
            b"ordered-complete-packets",
            0,
            0,
            0,
            &[&bytes],
        )
        .map_err(|_| Error::InvalidProfile)
    }

    fn columns(&self) -> Vec<Vec<F>> {
        let mut sorted = (0..self.events.len())
            .filter(|slot| self.events[*slot].is_some())
            .map(|slot| self.ordered(slot))
            .collect::<Vec<_>>();
        sorted.sort_unstable_by_key(|packet| (packet[packet::KEY].0, packet[packet::CLOCK].0));
        self.witness_columns(&sorted)
    }

    fn witness_columns(&self, sorted: &[[F; packet::WIDTH]]) -> Vec<Vec<F>> {
        let mut columns = vec![vec![F::ZERO; self.size()]; NOTE_COPY_WIDTH_V1 + ROW_WIDTH];
        let mut previous = [F::ZERO; PREV_WIDTH];
        let mut ordered_count = 0;
        let mut sorted_count = 0;
        let mut ended = false;
        for slot in 0..self.size() / PHASES {
            let a = self.ordered(slot);
            let b = sorted
                .get(slot)
                .copied()
                .unwrap_or([F::ZERO; packet::WIDTH]);
            for phase in 0..PHASES {
                let mut row = [F::ZERO; ROW_WIDTH];
                row[ORDERED..SORTED].copy_from_slice(&a);
                row[SORTED..PREVIOUS].copy_from_slice(&b);
                row[PREVIOUS..SOURCES].copy_from_slice(&previous);
                let [left, right] = sorted::source_words(&b, &previous, phase);
                row[SOURCES..COMPARE].copy_from_slice(&word::witness(left, right));
                row[COMPARE..CONTROLS].copy_from_slice(&branch::bank_witness(
                    wide::control::BLTU,
                    left,
                    right,
                ));
                let same = b[packet::ENABLED] == F::ONE
                    && previous[PREV_VALID] == F::ONE
                    && previous[PREV_KEY] == b[packet::KEY];
                row[SAME_KEY] = F(u64::from(same));
                row[NEW_KEY] = b[packet::ENABLED].sub(row[SAME_KEY]);
                row[KEY_LESS] = F(u64::from(previous[PREV_KEY].0 < b[packet::KEY].0));
                row[TIME_LESS] = F(u64::from(previous[PREV_CLOCK].0 < b[packet::CLOCK].0));
                row[ORDERED_COUNT] = F(ordered_count);
                row[SORTED_COUNT] = F(sorted_count);
                row[SORTED_ENDED] = F(u64::from(ended));
                if b[packet::ENABLED] == F::ONE {
                    row[TYPES + b[packet::SPACE].0 as usize - 1] = F::ONE;
                }
                for (column, value) in row.into_iter().enumerate() {
                    columns[NOTE_COPY_WIDTH_V1 + column][slot * PHASES + phase] = value;
                }
            }
            ordered_count += a[packet::ENABLED].0;
            sorted_count += b[packet::ENABLED].0;
            if b[packet::ENABLED] == F::ONE {
                previous[PREV_KEY] = b[packet::KEY];
                previous[PREV_CLOCK] = b[packet::CLOCK];
                previous[PREV_PAYLOAD..PREV_TAG]
                    .copy_from_slice(&b[packet::AFTER..packet::BEFORE_TAG]);
                previous[PREV_TAG] = b[packet::AFTER_TAG];
                previous[PREV_VALID] = F::ONE;
            } else {
                ended = true;
            }
        }
        columns
    }
}

fn residues(
    row: &[F],
    next: &[F],
    aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    challenges: &permutation::Challenges,
) -> Vec<F> {
    let mut out = Vec::with_capacity(CONSTRAINTS);
    for offset in 0..packet::WIDTH {
        out.push(row[ORDERED + offset].sub(fixed[offset]));
    }
    sorted::append_residues(&mut out, row, next, fixed);
    permutation::append_residues(
        &mut out,
        aux,
        next_aux,
        &row[ORDERED..SORTED],
        &row[SORTED..PREVIOUS],
        [
            fixed[PHASE_OFFSET + PHASES - 1],
            fixed[TRANSITION],
            fixed[FIRST],
            fixed[LAST],
        ],
        challenges,
    );
    debug_assert_eq!(out.len(), CONSTRAINTS);
    out
}

impl ProofManagedNoteStarkAdapterV1 for PublicPacketBus {
    type ProfileChallenges = permutation::Challenges;

    fn protocol_v1(&self) -> ProofManagedNoteStarkProtocolV1 {
        ProofManagedNoteStarkProtocolV1 {
            parameters: AggregateStarkParametersV1 {
                proof_magic: *b"IMB1",
                proof_version: 1,
                security_lanes: 1,
                query_count: 136,
                blowup_log2: 3,
                terminal_log2: 10,
                terminal_degree_bound: 143,
                composition_degree_chunks: 4,
                minimum_trace_log2: MIN_LOG,
                maximum_trace_log2: MAX_LOG,
                maximum_trace_groups: 1,
                maximum_segment_instances: 1,
                maximum_base_columns_per_instance: NOTE_COPY_WIDTH_V1 + ROW_WIDTH,
                maximum_aux_columns_per_instance: NOTE_COPY_AUX_WIDTH_V1 + permutation::WIDTH,
                maximum_proof_bytes: 4 * 1024 * 1024,
            },
            domains: DOMAINS,
            maximum_constraint_degree: 4,
            profile_binding_label: b"ivm-machine-bus-profile-binding-v1",
            profile_descriptor: PROFILE,
            relation_layout_domain: b"ivm-machine-bus-relation-layout-v1",
        }
    }

    fn public_input_digest_v1(&self) -> Result<GoldilocksDigest384V1, Error> {
        self.digest()
    }
    fn trace_log2_v1(&self) -> u8 {
        self.trace_log2
    }
    fn base_width_v1(&self) -> usize {
        NOTE_COPY_WIDTH_V1 + ROW_WIDTH
    }
    fn profile_aux_width_v1(&self) -> usize {
        permutation::WIDTH
    }
    fn profile_fixed_width_v1(&self) -> usize {
        FIXED_WIDTH
    }
    fn profile_constraint_count_v1(&self) -> usize {
        CONSTRAINTS
    }
    fn copy_schedule_v1(&self) -> Result<NoteCopyScheduleV1, Error> {
        Ok(NoteCopyScheduleV1 {
            policies: vec![[NoteCopyCellPolicyV1::Inactive; NOTE_COPY_WIDTH_V1]; self.size()],
            sigma: (0..self.size())
                .map(|row| {
                    std::array::from_fn(|column| (row * NOTE_COPY_WIDTH_V1 + column + 1) as u32)
                })
                .collect(),
        })
    }
    fn profile_fixed_columns_v1(&self) -> Result<Vec<Vec<F>>, Error> {
        Ok((0..FIXED_WIDTH)
            .map(|column| {
                (0..self.size())
                    .map(|row| self.fixed(row)[column])
                    .collect()
            })
            .collect())
    }
    fn derive_profile_challenges_v1(
        &self,
        transcript: &mut TransparentTranscriptV1,
        _: NoteCopyChallengesV1,
    ) -> Result<Self::ProfileChallenges, Error> {
        permutation::Challenges::derive(transcript)
    }
    fn build_profile_aux_columns_v1(
        &self,
        base: &[Vec<F>],
        _: &[Vec<F>],
        _: &[Vec<F>],
        _: NoteCopyChallengesV1,
        challenges: &Self::ProfileChallenges,
    ) -> Result<Vec<Vec<F>>, Error> {
        let start = NOTE_COPY_WIDTH_V1;
        if base.len() != self.base_width_v1() {
            return Err(Error::InvalidTrace);
        }
        permutation::columns(
            &base[start + ORDERED..start + SORTED],
            &base[start + SORTED..start + PREVIOUS],
            challenges,
            self.size(),
        )
    }
    fn profile_constraint_residues_v1(
        &self,
        row: &[F],
        next: &[F],
        aux: &[F],
        next_aux: &[F],
        fixed: &[F],
        _: NoteCopyChallengesV1,
        challenges: &Self::ProfileChallenges,
    ) -> Result<Vec<F>, Error> {
        if row.len() != self.base_width_v1()
            || next.len() != self.base_width_v1()
            || aux.len() != NOTE_COPY_AUX_WIDTH_V1 + permutation::WIDTH
            || next_aux.len() != aux.len()
            || fixed.len() != NOTE_COPY_FIXED_WIDTH_V1 + FIXED_WIDTH
        {
            return Err(Error::InvalidTrace);
        }
        Ok(residues(
            &row[NOTE_COPY_WIDTH_V1..],
            &next[NOTE_COPY_WIDTH_V1..],
            &aux[NOTE_COPY_AUX_WIDTH_V1..],
            &next_aux[NOTE_COPY_AUX_WIDTH_V1..],
            &fixed[NOTE_COPY_FIXED_WIDTH_V1..],
            challenges,
        ))
    }
}

#[cfg(test)]
mod tests;
