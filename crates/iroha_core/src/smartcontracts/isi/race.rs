//! Native, permissionless race lifecycle with wallet-bound funding and proof-only payouts.
use super::{Error, Execute};
use crate::state::{StateReadOnly, StateTransaction, WorldReadOnly};
use iroha_crypto::{Algorithm, Hash, Signature, derive_non_signing_ed25519_public_key};
use iroha_data_model::{
    IntoKeyValue, NetworkId,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    execution_proofs::{ExecutionProofEnvelopeV1, RACE_SKIN_COUNT_V1},
    isi::race::*,
    metadata::Metadata,
    race::*,
};
use iroha_primitives::numeric::{Numeric, Quantity, RoundingMode};
use mv::storage::StorageReadOnly;
use norito::codec::Encode;

/// Exact one-shot capability produced only after native race admission.
pub(in crate::smartcontracts::isi) struct VerifiedRaceMovement {
    race_id: Hash,
    authority: AccountId,
    funding: bool,
    legs: Vec<(AssetId, AssetId, Quantity)>,
}
impl VerifiedRaceMovement {
    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (Hash, AccountId, bool, Vec<(AssetId, AssetId, Quantity)>) {
        (self.race_id, self.authority, self.funding, self.legs)
    }
}
fn invalid(message: impl Into<String>) -> Error {
    Error::InvariantViolation(message.into().into())
}
/// Derive custody without ever deriving a signing scalar.
pub fn race_custody_account_v1(
    network: &NetworkId,
    race: &Hash,
    asset: &AssetDefinitionId,
) -> AccountId {
    AccountId::new(derive_non_signing_ed25519_public_key(
        b"iroha:race:custody:v1",
        &[
            network.as_bytes(),
            race.as_ref(),
            asset.to_string().as_bytes(),
        ],
    ))
}
/// Retain immutable payout destinations while their stakes remain in native custody.
pub(crate) fn retained_race_account(
    world: &impl WorldReadOnly,
    account: &AccountId,
) -> Option<Hash> {
    world.races().iter().find_map(|(id, race)| {
        (race.custody == *account
            || (!race.liability.is_zero()
                && race.participants.iter().any(|p| p.account == *account)))
        .then_some(*id)
    })
}
/// Retain the asset definition needed to return every outstanding stake.
pub(crate) fn retained_race_asset(
    world: &impl WorldReadOnly,
    asset: &AssetDefinitionId,
) -> Option<Hash> {
    world.races().iter().find_map(|(id, race)| {
        (!race.liability.is_zero() && race.asset_definition == *asset).then_some(*id)
    })
}
fn get(st: &StateTransaction<'_, '_>, id: &Hash) -> Result<RaceRecordV1, Error> {
    st.world
        .races
        .get(id)
        .cloned()
        .ok_or_else(|| invalid("race does not exist"))
}
fn next_deadline(st: &StateTransaction<'_, '_>, blocks: u64) -> Result<u64, Error> {
    st.block_height()
        .checked_add(blocks)
        .ok_or_else(|| invalid("race deadline height overflow"))
}
fn active_slots(race: &RaceRecordV1) -> Vec<usize> {
    race.participants
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.dnf_at_tick.is_none().then_some(i))
        .collect()
}
fn verify_signature(
    key: &iroha_crypto::PublicKey,
    signature: &Signature,
    hash: &Hash,
) -> Result<(), Error> {
    if key.algorithm() != Algorithm::Ed25519 || signature.payload().len() != 64 {
        return Err(invalid("race signatures require canonical Ed25519"));
    }
    signature
        .verify(key, hash.as_ref())
        .map_err(|_| invalid("invalid race gameplay signature"))
}
fn verify_joint(
    race: &RaceRecordV1,
    signatures: &[RaceSlotSignatureV1],
    hash: &Hash,
) -> Result<(), Error> {
    let slots = active_slots(race);
    if signatures.len() != slots.len() || slots.is_empty() {
        return Err(invalid("race certificate requires every active racer"));
    }
    for (signature, slot) in signatures.iter().zip(slots) {
        if usize::from(signature.slot) != slot {
            return Err(invalid("race certificate slots must be unique and ordered"));
        }
        verify_signature(
            &race.participants[slot].input_key,
            &signature.signature,
            hash,
        )?;
    }
    Ok(())
}
/// Hash the exact chain-selected history, not a relayer-provided projection.
pub fn race_dispute_root_v1(race: &RaceRecordV1) -> Hash {
    race_message_hash_v1(
        &race.network_id,
        "dispute-history",
        &(
            race.race_id,
            race.epoch,
            race.checkpoint.as_ref().map(|c| c.checkpoint.clone()),
            race.forced_batches.clone(),
        ),
    )
}
fn save(st: &mut StateTransaction<'_, '_>, mut race: RaceRecordV1) -> Result<(), Error> {
    race.revision = race
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("race revision overflow"))?;
    race.dispute_root = race_dispute_root_v1(&race);
    st.world
        .emit_events(Some(iroha_data_model::events::data::race::RaceEventV1 {
            race_id: race.race_id,
            revision: race.revision,
            phase: race.phase as u8,
            dispute_root: race.dispute_root,
        }));
    st.world.races.insert(race.race_id, race);
    Ok(())
}
fn validate_checkpoint(
    race: &RaceRecordV1,
    checkpoint: &SignedRaceCheckpointV1,
) -> Result<(), Error> {
    let cp = &checkpoint.checkpoint;
    if cp.race_id != race.race_id
        || cp.epoch != race.epoch
        || cp.tick < race.next_tick
        || cp.tick > RACE_MAX_TICKS_V1
        || (!cp.terminal && cp.tick % RACE_INPUT_BATCH_TICKS_V1 as u32 != 0)
    {
        return Err(invalid(
            "race checkpoint has wrong epoch or stale/noncanonical tick",
        ));
    }
    if let Some(old) = &race.checkpoint {
        if cp.tick == old.checkpoint.tick && cp != &old.checkpoint {
            return Err(invalid("conflicting race checkpoint at same tick"));
        }
    }
    verify_joint(
        race,
        &checkpoint.signatures,
        &race_message_hash_v1(&race.network_id, "checkpoint", cp),
    )
}
fn validate_frontier(
    race: &RaceRecordV1,
    checkpoint: &RaceCheckpointV1,
    frontier: &RaceCommitmentSetV1,
) -> Result<(), Error> {
    if frontier.race_id != race.race_id
        || frontier.epoch != race.epoch
        || frontier.start_tick != checkpoint.tick
        || frontier.parent_transcript_root != checkpoint.transcript_root
        || frontier.commitments.len() != race.participants.len()
        || checkpoint.terminal
    {
        return Err(invalid(
            "race commitment frontier does not extend exact checkpoint",
        ));
    }
    verify_joint(
        race,
        &frontier.signatures,
        &race_commitment_set_hash_v1(&race.network_id, frontier),
    )
}
impl Execute for OpenRaceV1 {
    fn execute(
        self,
        authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if self.rules.version != 1
            || !(2..=8).contains(&self.rules.max_racers)
            || self.stake.is_zero()
            || self.join_deadline_height <= st.block_height()
            || self.join_deadline_height > next_deadline(st, 300)?
        {
            return Err(invalid(
                "invalid race rules, positive stake, or bounded join deadline",
            ));
        }
        if st.world.races.get(&self.race_id).is_some() {
            return Err(invalid("race id already used"));
        }
        st.world.account(authority)?;
        let xor = crate::block::parse_asset_definition_literal_with_world(
            &st.world,
            &st.nexus.fees.fee_asset_id,
            st.block_unix_timestamp_ms(),
        )
        .ok_or_else(|| invalid("canonical fee XOR asset unavailable"))?;
        if self.asset_definition != xor {
            return Err(invalid(
                "race stakes must use exact configured XOR definition",
            ));
        }
        let spec = st.numeric_spec_for(&self.asset_definition)?;
        super::asset::isi::assert_numeric_spec_with(self.stake.as_numeric(), spec)?;
        let network = st.network_id().clone();
        let custody = race_custody_account_v1(&network, &self.race_id, &self.asset_definition);
        if st.world.account(&custody).is_err() {
            let (id, value) = Account {
                id: custody.clone(),
                metadata: Metadata::default(),
                label: None,
                uaid: None,
                opaque_ids: Vec::new(),
            }
            .into_key_value();
            st.world.accounts.insert(id, value);
        }
        let race = RaceRecordV1 {
            version: 1,
            network_id: network,
            race_id: self.race_id,
            rules: self.rules,
            profile_id: crate::execution_proofs::race_profile_id_v1(),
            rules_hash: crate::execution_proofs::race_rules_hash_v1(),
            asset_definition: self.asset_definition,
            stake: self.stake,
            custody,
            liability: Quantity::zero(),
            participants: Vec::new(),
            roster_hash: Hash::new([]),
            phase: RacePhaseV1::Lobby,
            revision: 0,
            epoch: 0,
            deadline_height: self.join_deadline_height,
            checkpoint: None,
            pending_certificate: None,
            next_tick: 0,
            input_commitments: Vec::new(),
            input_reveals: Vec::new(),
            forced_batches: Vec::new(),
            dispute_root: Hash::new([]),
            result: None,
        };
        save(st, race)
    }
}
impl Execute for JoinRaceV1 {
    fn execute(
        self,
        authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !crate::execution_proofs::race_profile_is_qualified_v1() {
            return Err(invalid(
                "native race proof profile has not passed qualification; funding disabled",
            ));
        }
        let mut race = get(st, &self.race_id)?;
        if race.phase != RacePhaseV1::Lobby
            || st.block_height() > race.deadline_height
            || race.participants.len() >= usize::from(race.rules.max_racers)
            || self.car_id >= RACE_SKIN_COUNT_V1
            || self.input_key.algorithm() != Algorithm::Ed25519
            || race
                .participants
                .iter()
                .any(|p| p.account == *authority || p.input_key == self.input_key)
        {
            return Err(invalid("race seat, wallet or input key is not admissible"));
        }
        let movement = VerifiedRaceMovement {
            race_id: race.race_id,
            authority: authority.clone(),
            funding: true,
            legs: vec![(
                AssetId::new(race.asset_definition.clone(), authority.clone()),
                AssetId::new(race.asset_definition.clone(), race.custody.clone()),
                race.stake.clone(),
            )],
        };
        super::asset::isi::execute_verified_race_movement(st, movement)?;
        race.liability = race
            .liability
            .checked_add(&race.stake)
            .map_err(|_| invalid("race stake liability overflow"))?;
        race.participants.push(RaceParticipantV1 {
            account: authority.clone(),
            input_key: self.input_key,
            car_id: self.car_id,
            dnf_at_tick: None,
        });
        save(st, race)
    }
}
impl Execute for StartRaceV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut race = get(st, &self.race_id)?;
        if race.phase != RacePhaseV1::Lobby
            || race.participants.len() < 2
            || (race.participants.len() < usize::from(race.rules.max_racers)
                && st.block_height() <= race.deadline_height)
        {
            return Err(invalid(
                "race starts when its grid is full or join deadline has passed",
            ));
        }
        race.roster_hash = race_message_hash_v1(
            &race.network_id,
            "roster",
            &(race.race_id, race.participants.clone()),
        );
        race.phase = RacePhaseV1::Racing;
        race.deadline_height = next_deadline(st, 300)?;
        race.input_commitments = vec![None; race.participants.len()];
        race.input_reveals = vec![None; race.participants.len()];
        save(st, race)
    }
}
impl Execute for CommitRaceCheckpointV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut race = get(st, &self.race_id)?;
        if !matches!(
            race.phase,
            RacePhaseV1::Racing | RacePhaseV1::SelectingCheckpoint | RacePhaseV1::ForcedCommit
        ) || (race.phase == RacePhaseV1::SelectingCheckpoint
            && st.block_height() > race.deadline_height)
        {
            return Err(invalid("race checkpoint phase is sealed"));
        }
        if race.phase == RacePhaseV1::ForcedCommit {
            // Resumption must certify newly resolved controls in a new epoch. A
            // pre-dispute certificate must never restart the same deadline cycle.
            let tick = self.checkpoint.checkpoint.tick;
            if tick != race.next_tick
                || !race.forced_batches.last().is_some_and(|batch| {
                    batch
                        .start_tick
                        .checked_add(RACE_INPUT_BATCH_TICKS_V1 as u32)
                        == Some(tick)
                        && batch.epoch.checked_add(1) == Some(race.epoch)
                })
                || race
                    .checkpoint
                    .as_ref()
                    .is_some_and(|old| tick <= old.checkpoint.tick)
                || race.input_commitments.iter().any(Option::is_some)
            {
                return Err(invalid(
                    "race resumption must certify a newly completed forced batch",
                ));
            }
        }
        validate_checkpoint(&race, &self.checkpoint)?;
        if let Some(frontier) = &self.frontier {
            validate_frontier(&race, &self.checkpoint.checkpoint, frontier)?;
        }
        if let Some(old) = &race.pending_certificate {
            if self.checkpoint.checkpoint.tick == old.start_tick
                && self.frontier.as_ref() != Some(old)
            {
                return Err(invalid(
                    "certified pending controls cannot be discarded or replaced",
                ));
            }
        }
        race.next_tick = self.checkpoint.checkpoint.tick;
        race.checkpoint = Some(self.checkpoint);
        race.pending_certificate = self.frontier;
        if race.phase == RacePhaseV1::ForcedCommit {
            race.phase = RacePhaseV1::Racing;
            race.deadline_height = next_deadline(st, 300)?;
        }
        // A challenge window always remains fixed even when a terminal certificate arrives.
        if race.phase == RacePhaseV1::Racing
            && race
                .checkpoint
                .as_ref()
                .is_some_and(|c| c.checkpoint.terminal)
        {
            race.phase = RacePhaseV1::AwaitingProof;
        }
        save(st, race)
    }
}
impl Execute for ChallengeRaceV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut race = get(st, &self.race_id)?;
        if race.phase != RacePhaseV1::Racing || self.epoch != race.epoch {
            return Err(invalid("race challenge is stale or already active"));
        }
        let participant = race
            .participants
            .get(usize::from(self.slot))
            .ok_or_else(|| invalid("unknown race challenger slot"))?;
        if participant.dnf_at_tick.is_some() {
            return Err(invalid("DNF racer cannot restart play"));
        }
        verify_signature(
            &participant.input_key,
            &self.signature,
            &race_message_hash_v1(
                &race.network_id,
                "challenge",
                &(race.race_id, race.epoch, self.slot),
            ),
        )?;
        race.phase = RacePhaseV1::SelectingCheckpoint;
        race.deadline_height = next_deadline(st, RACE_CHECKPOINT_WINDOW_BLOCKS_V1)?;
        save(st, race)
    }
}
impl Execute for CommitRaceInputsV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let input = self.input;
        let mut race = get(st, &input.race_id)?;
        if race.phase != RacePhaseV1::ForcedCommit
            || st.block_height() > race.deadline_height
            || input.epoch != race.epoch
            || input.start_tick != race.next_tick
        {
            return Err(invalid(
                "race input commitment is stale or outside its phase",
            ));
        }
        let slot = usize::from(input.slot);
        let participant = race
            .participants
            .get(slot)
            .ok_or_else(|| invalid("unknown input slot"))?;
        if participant.dnf_at_tick.is_some() {
            return Err(invalid("DNF slot has no gameplay authority"));
        }
        verify_signature(
            &participant.input_key,
            &input.signature,
            &race_input_message_hash_v1(&race.network_id, &input),
        )?;
        if race.input_commitments[slot].is_some_and(|old| old != input.commitment) {
            return Err(invalid("race input commitment cannot be replaced"));
        }
        race.input_commitments[slot] = Some(input.commitment);
        if active_slots(&race)
            .iter()
            .all(|slot| race.input_commitments[*slot].is_some())
        {
            race.phase = RacePhaseV1::ForcedReveal;
            race.deadline_height = next_deadline(st, RACE_INPUT_WINDOW_BLOCKS_V1)?;
        }
        save(st, race)
    }
}
impl Execute for RevealRaceInputsV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let input = self.reveal;
        let mut race = get(st, &input.race_id)?;
        if race.phase != RacePhaseV1::ForcedReveal
            || st.block_height() > race.deadline_height
            || input.epoch != race.epoch
            || input.start_tick != race.next_tick
            || input.controls.len() != RACE_INPUT_BATCH_TICKS_V1
            || input.controls.iter().any(|mask| mask & !0x3f != 0)
        {
            return Err(invalid(
                "race input reveal is stale, oversized or malformed",
            ));
        }
        let slot = usize::from(input.slot);
        if slot >= race.participants.len()
            || race.participants[slot].dnf_at_tick.is_some()
            || race.input_commitments[slot]
                != Some(race_input_commitment_v1(&race.network_id, &input))
        {
            return Err(invalid(
                "race reveal does not match exact retained commitment",
            ));
        }
        if race.input_reveals[slot]
            .as_ref()
            .is_some_and(|old| old != &input.controls)
        {
            return Err(invalid("race reveal conflicts with retained controls"));
        }
        race.input_reveals[slot] = Some(input.controls);
        if active_slots(&race)
            .iter()
            .all(|slot| race.input_reveals[*slot].is_some())
        {
            advance_batch(st, &mut race, false)?;
        }
        save(st, race)
    }
}
fn advance_batch(
    st: &StateTransaction<'_, '_>,
    race: &mut RaceRecordV1,
    expired: bool,
) -> Result<(), Error> {
    let mut dnf_slots = Vec::new();
    for slot in active_slots(race) {
        if race.input_reveals[slot].is_none() {
            if !expired {
                return Err(invalid("race batch lacks required reveal"));
            }
            race.participants[slot].dnf_at_tick = Some(race.next_tick);
            dnf_slots.push(slot as u8);
        }
    }
    race.forced_batches.push(RaceForcedBatchV1 {
        epoch: race.epoch,
        start_tick: race.next_tick,
        controls: race
            .input_reveals
            .iter()
            .map(|controls| {
                controls
                    .clone()
                    .unwrap_or_else(|| vec![0; RACE_INPUT_BATCH_TICKS_V1])
            })
            .collect(),
        dnf_slots,
    });
    race.next_tick = race
        .next_tick
        .checked_add(RACE_INPUT_BATCH_TICKS_V1 as u32)
        .ok_or_else(|| invalid("race tick overflow"))?;
    race.epoch = race
        .epoch
        .checked_add(1)
        .ok_or_else(|| invalid("race epoch overflow"))?;
    race.pending_certificate = None;
    race.input_commitments.fill(None);
    race.input_reveals.fill(None);
    race.phase = if race.next_tick >= RACE_MAX_TICKS_V1 || active_slots(race).is_empty() {
        RacePhaseV1::AwaitingProof
    } else {
        RacePhaseV1::ForcedCommit
    };
    race.deadline_height = next_deadline(st, RACE_INPUT_WINDOW_BLOCKS_V1)?;
    Ok(())
}
impl Execute for AdvanceRaceDeadlineV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut race = get(st, &self.race_id)?;
        if st.block_height() <= race.deadline_height {
            return Err(invalid("race deadline has not expired"));
        }
        match race.phase {
            RacePhaseV1::Racing => {
                race.phase = RacePhaseV1::SelectingCheckpoint;
                race.deadline_height = next_deadline(st, RACE_CHECKPOINT_WINDOW_BLOCKS_V1)?;
            }
            RacePhaseV1::SelectingCheckpoint => {
                if race
                    .checkpoint
                    .as_ref()
                    .is_some_and(|c| c.checkpoint.terminal)
                {
                    race.phase = RacePhaseV1::AwaitingProof;
                } else if let Some(frontier) = &race.pending_certificate {
                    race.input_commitments =
                        frontier.commitments.iter().copied().map(Some).collect();
                    race.phase = RacePhaseV1::ForcedReveal;
                    race.deadline_height = next_deadline(st, RACE_INPUT_WINDOW_BLOCKS_V1)?;
                } else {
                    race.phase = RacePhaseV1::ForcedCommit;
                    race.deadline_height = next_deadline(st, RACE_INPUT_WINDOW_BLOCKS_V1)?;
                }
            }
            RacePhaseV1::ForcedCommit => {
                race.phase = RacePhaseV1::ForcedReveal;
                race.deadline_height = next_deadline(st, RACE_INPUT_WINDOW_BLOCKS_V1)?;
            }
            RacePhaseV1::ForcedReveal => advance_batch(st, &mut race, true)?,
            _ => return Err(invalid("race has no advanceable deadline")),
        }
        save(st, race)
    }
}
fn payout(
    st: &mut StateTransaction<'_, '_>,
    race: &RaceRecordV1,
    recipients: &[usize],
) -> Result<(), Error> {
    if recipients.is_empty() {
        return Err(invalid("race payout recipients absent"));
    }
    let scale = st
        .numeric_spec_for(&race.asset_definition)?
        .scale()
        .unwrap_or(9);
    let share = race
        .liability
        .try_mul_div_decimal_round(
            &Numeric::one(),
            &Numeric::from(recipients.len() as u64),
            scale,
            RoundingMode::Floor,
        )
        .map_err(|_| invalid("race payout division overflow"))?;
    let mut remaining = race.liability.clone();
    let mut legs = Vec::new();
    for (index, slot) in recipients.iter().enumerate() {
        let amount = if index + 1 == recipients.len() {
            remaining.clone()
        } else {
            share.clone()
        };
        remaining = remaining
            .checked_sub(&amount)
            .map_err(|_| invalid("race payout liability underflow"))?;
        if !amount.is_zero() {
            legs.push((
                AssetId::new(race.asset_definition.clone(), race.custody.clone()),
                AssetId::new(
                    race.asset_definition.clone(),
                    race.participants[*slot].account.clone(),
                ),
                amount,
            ));
        }
    }
    super::asset::isi::execute_verified_race_movement(
        st,
        VerifiedRaceMovement {
            race_id: race.race_id,
            authority: race.custody.clone(),
            funding: false,
            legs,
        },
    )
}
fn validate_statement(race: &RaceRecordV1, proof: &ExecutionProofEnvelopeV1) -> Result<(), Error> {
    let s = &proof.statement;
    if proof.version != 1
        || proof.profile_id != race.profile_id
        || s.network_id != race.network_id
        || s.race_id != race.race_id
        || s.roster_hash != race.roster_hash
        || s.rules_hash != race.rules_hash
        || s.track != race.rules.track
        || s.dispute_root != race.dispute_root
        || s.result.standings.len() != race.participants.len()
    {
        return Err(invalid(
            "race proof statement does not match current native state",
        ));
    }
    if race.forced_batches.is_empty() {
        let cp = race
            .checkpoint
            .as_ref()
            .ok_or_else(|| invalid("race has no terminal certificate"))?;
        if !cp.checkpoint.terminal
            || s.transcript_root != cp.checkpoint.transcript_root
            || s.result.ticks != cp.checkpoint.tick
        {
            return Err(invalid(
                "race proof differs from terminal certified transcript",
            ));
        }
    }
    if s.result
        .winners
        .iter()
        .any(|slot| usize::from(*slot) >= race.participants.len())
        || s.result.winners.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("race winner slots not canonical"));
    }
    Ok(())
}
impl Execute for SubmitRaceProofV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut race = get(st, &self.race_id)?;
        if race.phase != RacePhaseV1::AwaitingProof {
            return Err(invalid("race not ready for terminal execution proof"));
        }
        validate_statement(&race, &self.proof)?;
        crate::execution_proofs::verify_race_proof_for_history_v1(
            &self.proof,
            race.checkpoint.as_ref().map(|c| &c.checkpoint),
            &race.forced_batches,
            &race.participants,
            race.epoch,
        )
        .map_err(|error| invalid(format!("race execution proof rejected: {error}")))?;
        let recipients = if self.proof.statement.result.winners.is_empty() {
            (0..race.participants.len()).collect::<Vec<_>>()
        } else {
            self.proof
                .statement
                .result
                .winners
                .iter()
                .map(|s| usize::from(*s))
                .collect()
        };
        payout(st, &race, &recipients)?;
        race.liability = Quantity::zero();
        race.phase = RacePhaseV1::Settled;
        race.result = Some(self.proof.statement.result);
        save(st, race)
    }
}
impl Execute for ExpireRaceV1 {
    fn execute(
        self,
        _authority: &AccountId,
        st: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut race = get(st, &self.race_id)?;
        if race.phase != RacePhaseV1::Lobby
            || st.block_height() <= race.deadline_height
            || race.participants.len() >= 2
        {
            return Err(invalid("only expired unstartable race lobbies refund"));
        }
        if !race.participants.is_empty() {
            payout(st, &race, &(0..race.participants.len()).collect::<Vec<_>>())?;
        }
        race.liability = Quantity::zero();
        race.phase = RacePhaseV1::Cancelled;
        save(st, race)
    }
}

impl crate::prelude::ValidSingularQuery for iroha_data_model::query::race::FindRaceById {
    fn execute(
        &self,
        state: &impl StateReadOnly,
    ) -> Result<RaceRecordV1, iroha_data_model::query::error::QueryExecutionFail> {
        state
            .world()
            .races()
            .get(&self.race_id)
            .ok_or_else(|| {
                iroha_data_model::query::error::QueryExecutionFail::Find(
                    iroha_data_model::query::error::FindError::Race(self.race_id),
                )
            })
            .and_then(crate::smartcontracts::isi::query::own_singular_query_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{HashOf, KeyPair};
    use iroha_data_model::{block::BlockHeader, domain::DomainId, execution_proofs::RaceTrackV1};

    fn key(slot: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![slot + 1; 32], Algorithm::Ed25519).unwrap()
    }
    fn fixture() -> RaceRecordV1 {
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([3; 32]),
        ));
        let race_id = Hash::new(b"race-protocol-adversarial-fixture");
        let asset_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("race", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
        let custody = race_custody_account_v1(&network, &race_id, &asset_definition);
        let participants = (0..2)
            .map(|slot| RaceParticipantV1 {
                account: AccountId::new(key(slot + 10).public_key().clone()),
                input_key: key(slot).public_key().clone(),
                car_id: slot,
                dnf_at_tick: None,
            })
            .collect::<Vec<_>>();
        RaceRecordV1 {
            version: 1,
            network_id: network,
            race_id,
            rules: RaceRulesV1 {
                version: 1,
                track: RaceTrackV1::NeonTokyo,
                max_racers: 2,
            },
            profile_id: crate::execution_proofs::race_profile_id_v1(),
            rules_hash: crate::execution_proofs::race_rules_hash_v1(),
            asset_definition,
            stake: Quantity::one(),
            custody,
            liability: Quantity::from(2_u32),
            participants,
            roster_hash: Hash::new(b"roster"),
            phase: RacePhaseV1::Racing,
            revision: 0,
            epoch: 0,
            deadline_height: 30,
            checkpoint: None,
            pending_certificate: None,
            next_tick: 0,
            input_commitments: vec![None; 2],
            input_reveals: vec![None; 2],
            forced_batches: vec![],
            dispute_root: Hash::new([]),
            result: None,
        }
    }
    fn state() -> State {
        State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }
    fn header() -> BlockHeader {
        BlockHeader::new(nonzero_ext::nonzero!(1_u64), None, None, None, 1_000, 0)
    }
    fn signed_checkpoint(race: &RaceRecordV1, tick: u32, terminal: bool) -> SignedRaceCheckpointV1 {
        let checkpoint = RaceCheckpointV1 {
            race_id: race.race_id,
            epoch: race.epoch,
            tick,
            transcript_root: Hash::new(b"inputs"),
            state_root: Hash::new(b"state"),
            terminal,
        };
        let digest = race_message_hash_v1(&race.network_id, "checkpoint", &checkpoint);
        let signatures = active_slots(race)
            .into_iter()
            .map(|slot| RaceSlotSignatureV1 {
                slot: slot as u8,
                signature: Signature::new(key(slot as u8).private_key(), digest.as_ref()),
            })
            .collect();
        SignedRaceCheckpointV1 {
            checkpoint,
            signatures,
        }
    }
    #[test]
    fn joint_checkpoint_rejects_missing_duplicate_wrong_domain_and_stale_signature() {
        let mut race = fixture();
        let valid = signed_checkpoint(&race, 6, false);
        validate_checkpoint(&race, &valid).unwrap();
        let mut missing = valid.clone();
        missing.signatures.pop();
        assert!(validate_checkpoint(&race, &missing).is_err());
        let mut duplicate = valid.clone();
        duplicate.signatures[1] = duplicate.signatures[0].clone();
        assert!(validate_checkpoint(&race, &duplicate).is_err());
        let mut wrong = valid.clone();
        wrong.signatures[0].signature = Signature::new(
            key(0).private_key(),
            race_message_hash_v1(&race.network_id, "challenge", &wrong.checkpoint).as_ref(),
        );
        assert!(validate_checkpoint(&race, &wrong).is_err());
        race.epoch = 1;
        assert!(validate_checkpoint(&race, &valid).is_err());
        race.epoch = 0;
        race.next_tick = 12;
        assert!(validate_checkpoint(&race, &valid).is_err());
    }
    #[test]
    fn certified_input_frontier_cannot_be_discarded_after_reveal() {
        let state = state();
        let mut block = state.block(header());
        let mut st = block.transaction();
        let mut race = fixture();
        let checkpoint = signed_checkpoint(&race, 0, false);
        let mut frontier = RaceCommitmentSetV1 {
            race_id: race.race_id,
            epoch: 0,
            start_tick: 0,
            parent_transcript_root: checkpoint.checkpoint.transcript_root,
            commitments: vec![Hash::new(b"a"), Hash::new(b"b")],
            signatures: vec![],
        };
        let digest = race_commitment_set_hash_v1(&race.network_id, &frontier);
        frontier.signatures = (0..2)
            .map(|slot| RaceSlotSignatureV1 {
                slot,
                signature: Signature::new(key(slot).private_key(), digest.as_ref()),
            })
            .collect();
        race.pending_certificate = Some(frontier.clone());
        race.checkpoint = Some(checkpoint.clone());
        st.world.races.insert(race.race_id, race.clone());
        let authority = AccountId::new(key(20).public_key().clone());
        assert!(
            CommitRaceCheckpointV1 {
                race_id: race.race_id,
                checkpoint: checkpoint.clone(),
                frontier: None
            }
            .execute(&authority, &mut st)
            .is_err()
        );
        assert_eq!(
            get(&st, &race.race_id).unwrap().pending_certificate,
            Some(frontier.clone())
        );
        CommitRaceCheckpointV1 {
            race_id: race.race_id,
            checkpoint,
            frontier: Some(frontier),
        }
        .execute(&authority, &mut st)
        .unwrap();
    }
    #[test]
    fn absent_peer_is_removed_only_by_expired_consensus_deadline_and_replay_is_deterministic() {
        let mut roots = Vec::new();
        for _ in 0..4 {
            let state = state();
            let mut block = state.block(header());
            let mut st = block.transaction();
            let mut race = fixture();
            race.phase = RacePhaseV1::ForcedReveal;
            race.deadline_height = 0;
            race.input_reveals[0] = Some(vec![1; RACE_INPUT_BATCH_TICKS_V1]);
            let authority = AccountId::new(key(20).public_key().clone());
            st.world.races.insert(race.race_id, race.clone());
            AdvanceRaceDeadlineV1 {
                race_id: race.race_id,
            }
            .execute(&authority, &mut st)
            .unwrap();
            let resolved = get(&st, &race.race_id).unwrap();
            assert_eq!(resolved.participants[0].dnf_at_tick, None);
            assert_eq!(resolved.participants[1].dnf_at_tick, Some(0));
            assert_eq!(resolved.next_tick, 6);
            assert_eq!(resolved.epoch, 1);
            assert_eq!(resolved.phase, RacePhaseV1::ForcedCommit);
            assert_eq!(resolved.forced_batches[0].dnf_slots, vec![1]);
            assert_eq!(
                resolved.forced_batches[0].controls,
                vec![vec![1; 6], vec![0; 6]]
            );
            assert!(
                AdvanceRaceDeadlineV1 {
                    race_id: race.race_id
                }
                .execute(&authority, &mut st)
                .is_err()
            );
            roots.push(resolved.encode());
        }
        assert!(
            roots.windows(2).all(|pair| pair[0] == pair[1]),
            "four independent state replicas must select identical history"
        );
    }
    #[test]
    fn gameplay_key_cannot_change_wallet_or_custody_and_closed_custody_remains_reserved() {
        let state = state();
        let mut block = state.block(header());
        let mut st = block.transaction();
        let race = fixture();
        st.world.races.insert(race.race_id, race.clone());
        assert_eq!(
            retained_race_account(&st.world, &race.participants[0].account),
            Some(race.race_id)
        );
        assert_eq!(
            retained_race_asset(&st.world, &race.asset_definition),
            Some(race.race_id)
        );
        assert!(super::super::escrow::is_protocol_escrow_custody_account(
            &st,
            &race.custody
        ));
        let mut closed = race.clone();
        closed.liability = Quantity::zero();
        closed.phase = RacePhaseV1::Settled;
        st.world.races.insert(race.race_id, closed);
        assert_eq!(
            retained_race_account(&st.world, &race.participants[0].account),
            None
        );
        assert_eq!(
            retained_race_account(&st.world, &race.custody),
            Some(race.race_id)
        );
        assert_eq!(retained_race_asset(&st.world, &race.asset_definition), None);
        assert_ne!(
            race.custody,
            race_custody_account_v1(
                &race.network_id,
                &Hash::new(b"other-race"),
                &race.asset_definition
            )
        );
    }
    #[test]
    fn challenge_is_relayable_once_but_cannot_extend_its_deadline() {
        let state = state();
        let mut block = state.block(header());
        let mut st = block.transaction();
        let race = fixture();
        st.world.races.insert(race.race_id, race.clone());
        let hash = race_message_hash_v1(
            &race.network_id,
            "challenge",
            &(race.race_id, race.epoch, 0_u8),
        );
        let challenge = ChallengeRaceV1 {
            race_id: race.race_id,
            epoch: 0,
            slot: 0,
            signature: Signature::new(key(0).private_key(), hash.as_ref()),
        };
        let relayer = AccountId::new(key(20).public_key().clone());
        challenge.clone().execute(&relayer, &mut st).unwrap();
        let first = get(&st, &race.race_id).unwrap();
        assert_eq!(first.phase, RacePhaseV1::SelectingCheckpoint);
        assert!(challenge.execute(&relayer, &mut st).is_err());
        assert_eq!(get(&st, &race.race_id).unwrap(), first);
    }
    #[test]
    fn old_checkpoint_cannot_restart_a_forced_deadline_cycle() {
        let state = state();
        let mut block = state.block(header());
        let mut st = block.transaction();
        let mut race = fixture();
        let checkpoint = signed_checkpoint(&race, 0, false);
        race.phase = RacePhaseV1::ForcedCommit;
        race.checkpoint = Some(checkpoint.clone());
        st.world.races.insert(race.race_id, race.clone());
        let relayer = AccountId::new(key(20).public_key().clone());
        assert!(
            CommitRaceCheckpointV1 {
                race_id: race.race_id,
                checkpoint,
                frontier: None
            }
            .execute(&relayer, &mut st)
            .is_err()
        );
        assert_eq!(get(&st, &race.race_id).unwrap(), race);
    }
    #[test]
    fn unqualified_proof_profile_never_debits_a_wallet() {
        if crate::execution_proofs::race_profile_is_qualified_v1() {
            return;
        }
        let state = state();
        let mut block = state.block(header());
        let mut st = block.transaction();
        let mut race = fixture();
        race.phase = RacePhaseV1::Lobby;
        st.world.races.insert(race.race_id, race.clone());
        let authority = AccountId::new(key(20).public_key().clone());
        let error = JoinRaceV1 {
            race_id: race.race_id,
            input_key: key(21).public_key().clone(),
            car_id: 0,
        }
        .execute(&authority, &mut st)
        .unwrap_err();
        assert!(error.to_string().contains("funding disabled"));
        assert_eq!(get(&st, &race.race_id).unwrap(), race);
    }
}
