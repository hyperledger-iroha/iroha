//! The in-circuit **prototype** step relations (M7 `m7_step`, two-level and
//! flat layouts) on the `iroha_plonk_gadgets` chips.
//!
//! Every value is either range-checked where it is assigned (`u128` and
//! `u64` fields, via [`UintChip`]), a constant pinned through the constants
//! column, or a free glue witness pinned by the copies that hash it. Every
//! hash output is compared with the native reference
//! ([`StepWitness::evaluate`]) while the witness is known, so a successful
//! synthesis also proves digest parity.

use iroha_pasta::poseidon::PoseidonField;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{
    AbsorbInput, GlueChip, RunningSumChip, SpongeChip, U64, U128, UintChip, Word,
    statement::{StatementCells, StepRelation, statement_digest},
};

use crate::{
    circuit::{HashSite, Inventory, LanePlan, RelationOutput, RelationShape},
    witness::{
        CORE_FIELDS, LIFECYCLE_ACTIVE, NativeStep, REMAINDER_FIELDS, REQUEST_VERSION, SendInputs,
        StateLayout, StateRemainder, StepDigests, StepInputs, StepWitness, core_index, limbs,
    },
};

/// The chips a step lays out on.
pub struct Chips<F: PoseidonField> {
    /// The glue chip (on the glue lane's columns).
    pub glue: GlueChip<F>,
    /// The running-sum chip.
    pub range: RunningSumChip<F>,
    /// One sponge per lane.
    pub sponges: Vec<SpongeChip<F>>,
}

/// A value of the witness (unknown during key generation).
fn value<T, V>(source: Option<&T>, read: impl FnOnce(&T) -> V) -> Value<V> {
    source.map_or_else(Value::unknown, |source| Value::known(read(source)))
}

/// Free witnesses laid out four per glue row.
struct Batch<F> {
    values: Vec<Value<F>>,
}

impl<F: Copy> Batch<F> {
    /// Queues one value and returns its index.
    fn push(&mut self, value: Value<F>) -> usize {
        self.values.push(value);
        self.values.len() - 1
    }

    /// Queues two limbs.
    fn pair(&mut self, value: Value<[F; 2]>) -> [usize; 2] {
        let [lo, hi] = value.transpose_array();
        [self.push(lo), self.push(hi)]
    }
}

/// The word at `index` of a laid-out batch.
fn at<F: PoseidonField>(words: &[Word<F>], index: usize) -> Result<&Word<F>, Error> {
    words.get(index).ok_or(Error::Synthesis)
}

/// The two words of a laid-out pair.
fn pair_at<F: PoseidonField>(words: &[Word<F>], pair: [usize; 2]) -> Result<[&Word<F>; 2], Error> {
    Ok([at(words, pair[0])?, at(words, pair[1])?])
}

/// Fails when a known digest differs from the native reference.
fn check_digest<F: PoseidonField>(digest: &Word<F>, expected: Option<F>) -> Result<(), Error> {
    expected.map_or(Ok(()), |expected| {
        digest
            .value()
            .error_if_known_and(|value| *value != expected)
    })
}

/// Hashes `inputs` under the domain of `site` on the site's lane.
fn hash_site<F: PoseidonField>(
    sponges: &mut [SpongeChip<F>],
    region: &mut Region<'_, F>,
    shape: RelationShape,
    plan: &LanePlan,
    site: HashSite,
    inputs: &[AbsorbInput<'_, F>],
) -> Result<Word<F>, Error> {
    let (domain, arity) = shape.site_domain(site);
    if inputs.len() != arity {
        return Err(Error::Synthesis);
    }
    sponges
        .get_mut(plan.lane_of(site))
        .ok_or(Error::Synthesis)?
        .hash(region, domain, inputs)
}

/// The words of `words`, as absorbed inputs.
fn absorb<'w, F: PoseidonField>(words: &[&'w Word<F>]) -> Vec<AbsorbInput<'w, F>> {
    words.iter().map(|word| AbsorbInput::Word(word)).collect()
}

/// A state commitment preimage: the core, then the remainder digest or
/// fields.
fn preimage<'w, F: PoseidonField>(
    core: &[&'w Word<F>; CORE_FIELDS],
    rest: &[&'w Word<F>],
) -> Vec<&'w Word<F>> {
    let mut words = core.to_vec();
    words.extend_from_slice(rest);
    words
}

/// Lays out one step relation and returns its public outputs, in-circuit
/// digests and inventory.
#[allow(
    clippy::too_many_lines,
    reason = "one straight-line port of the M7 relation, in its order"
)]
pub fn assign<F: PoseidonField>(
    chips: &mut Chips<F>,
    region: &mut Region<'_, F>,
    shape: RelationShape,
    plan: &LanePlan,
    witness: Option<(&StepWitness<F>, &NativeStep<F>)>,
) -> Result<RelationOutput<F>, Error> {
    if witness.is_some_and(|(witness, _)| witness.relation() != shape.step) {
        return Err(Error::Synthesis);
    }
    let digests = witness.map(|(_, native)| native.digests);
    let witness = witness.map(|(witness, _)| witness);
    let Chips {
        glue,
        range,
        sponges,
    } = chips;
    let glue_start = glue.next_row();
    let mut uint = UintChip::new(glue, range);
    let state = witness.map(|witness| &witness.predecessor);
    let core = state.map(|state| &state.core);
    let remainder = state.map(|state| &state.remainder);
    let send = witness.and_then(|witness| match &witness.inputs {
        StepInputs::Send(send) => Some(send),
        StepInputs::Receive(_) => None,
    });
    let is_send = shape.step == StepRelation::Send;
    let two_level = shape.layout == StateLayout::TwoLevel;

    // Range-checked predecessor fields (M7: balance and sequence always;
    // the send ordinal, policy epoch and time floor only where `sigma_send`
    // compares them).
    let balance: U128<F> = uint.assign_u128(region, value(core, |core| core.balance))?;
    let sequence: U128<F> = uint.assign_u128(region, value(core, |core| core.sequence))?;
    let checked_send: Option<(U128<F>, U64<F>, U64<F>)> = if is_send {
        Some((
            uint.assign_u128(region, value(core, |core| core.next_send))?,
            uint.assign_u64(region, value(core, |core| core.policy_epoch))?,
            uint.assign_u64(region, value(core, |core| core.accepted_time_floor))?,
        ))
    } else {
        None
    };
    let amount: U128<F> =
        uint.assign_u128(region, value(witness, |witness| witness.inputs.amount()))?;

    // Free witnesses, four per glue row.
    let mut batch = Batch { values: Vec::new() };
    let core_field = |index: usize| value(core, |core| core.fields()[index]);
    let free_next_send = (!is_send).then(|| batch.push(core_field(core_index::NEXT_SEND)));
    let next_load = batch.push(core_field(core_index::NEXT_LOAD));
    let send_chain = batch.push(core_field(core_index::SEND_CHAIN));
    let recv_chain = batch.push(core_field(core_index::RECEIVE_CHAIN));
    let state_nonce = batch.push(core_field(core_index::STATE_NONCE));
    let lifecycle = batch.push(core_field(core_index::LIFECYCLE));
    let free_policy = (!is_send).then(|| batch.push(core_field(core_index::POLICY_EPOCH)));
    let free_floor = (!is_send).then(|| batch.push(core_field(core_index::TIME_FLOOR)));
    let successor_nonce = batch.push(value(witness, |witness| witness.successor_nonce));
    // The remainder: its digest (two-level) or all 30 fields (flat).
    let (rest_digest, rest_fields) = if two_level {
        (
            Some(batch.push(value(remainder, StateRemainder::digest))),
            Vec::new(),
        )
    } else {
        let fields = value(remainder, StateRemainder::fields).transpose_array();
        (
            None,
            fields.into_iter().map(|field| batch.push(field)).collect(),
        )
    };
    // Identity limbs: fresh witnesses (two-level) or the opened cells (flat).
    let identity =
        |batch: &mut Batch<F>, offset: usize, bytes: fn(&StateRemainder<F>) -> &[u8; 32]| {
            if two_level {
                batch.pair(value(remainder, |remainder| limbs::<F>(bytes(remainder))))
            } else {
                [rest_fields[offset], rest_fields[offset + 1]]
            }
        };
    let scheme = identity(&mut batch, 1, |remainder| &remainder.scheme_id);
    let asset = identity(&mut batch, 3, |remainder| &remainder.asset);
    let credential = identity(&mut batch, 7, |remainder| &remainder.credential);
    let payer = is_send.then(|| identity(&mut batch, 5, |remainder| &remainder.wallet_id));
    let fee_schedule =
        is_send.then(|| identity(&mut batch, 9, |remainder| &remainder.fee_schedule));
    let credit_id = batch.pair(value(witness, |witness| match &witness.inputs {
        StepInputs::Send(send) => limbs::<F>(&send.credit_id),
        StepInputs::Receive(receive) => limbs::<F>(&receive.credit_id),
    }));
    // The receiver wallet (`sigma_send`) or the payer wallet (`sigma_recv`).
    let counterparty = batch.pair(value(witness, |witness| match &witness.inputs {
        StepInputs::Send(send) => limbs::<F>(&send.receiver_wallet),
        StepInputs::Receive(receive) => limbs::<F>(&receive.payer_wallet),
    }));
    let predecessor_other = batch.pair(value(witness, |witness| {
        limbs::<F>(&witness.predecessor_other)
    }));
    let successor_other = batch.pair(value(witness, |witness| {
        limbs::<F>(&witness.successor_other)
    }));
    let send_limbs = is_send.then(|| {
        let mut pair = |read: fn(&SendInputs) -> &[u8; 32]| {
            batch.pair(value(send, |send| limbs::<F>(read(send))))
        };
        [
            pair(|send| &send.request_digest),
            pair(|send| &send.dependencies),
            pair(|send| &send.receiver_credential),
            pair(|send| &send.scheme_policy),
            pair(|send| &send.certificates),
            pair(|send| &send.request_nonce),
        ]
    });
    let words = uint.glue().witnesses(region, &batch.values)?;
    let word = |index: usize| at(&words, index);

    // Lifecycle Active (M7 `assert_is_const`).
    let lifecycle = word(lifecycle)?;
    GlueChip::assert_constant(region, lifecycle, F::from(LIFECYCLE_ACTIVE))?;
    // A nonzero amount and a sequence that does not overflow.
    uint.assert_nonzero(region, &amount)?;
    let sequence_after = uint.checked_add_constant(region, &sequence, 1)?;

    let next_load = word(next_load)?;
    let send_chain = word(send_chain)?;
    let recv_chain = word(recv_chain)?;
    let state_nonce = word(state_nonce)?;
    let successor_nonce = word(successor_nonce)?;
    let credit_id = pair_at(&words, credit_id)?;
    let counterparty = pair_at(&words, counterparty)?;
    let scheme = pair_at(&words, scheme)?;
    let asset = pair_at(&words, asset)?;
    let credential = pair_at(&words, credential)?;
    let rest: Vec<&Word<F>> = match rest_digest {
        Some(index) => vec![word(index)?],
        None => rest_fields
            .iter()
            .map(|index| word(*index))
            .collect::<Result<_, _>>()?,
    };
    if !two_level && rest.len() != REMAINDER_FIELDS {
        return Err(Error::Synthesis);
    }

    // The step: balance, ordinal and window checks, chain append, Request.
    let mut request_digest = None;
    let (next_send, policy_epoch, time_floor, successor_balance, next_send_after, chain, effect);
    match (checked_send, send_limbs, payer, fee_schedule) {
        (Some((ordinal, policy, floor)), Some(send_limbs), Some(payer), Some(fee_schedule)) => {
            let [
                digest_pair,
                dependencies,
                receiver_credential,
                scheme_policy,
                certificates,
                nonce,
            ] = send_limbs;
            let fee: U128<F> = uint.assign_u128(region, value(send, |send| send.fee))?;
            // balance' = balance - (amount + fee) >= 0 (M7: the debit itself is
            // not range-checked; balance - debit < 0 wraps above 2^128).
            let debit = uint.glue().add(region, amount.word(), fee.word())?;
            let difference = uint.glue().sub(region, balance.word(), &debit)?;
            let balance_after: U128<F> = uint.range_check::<128>(region, &difference)?;
            let ordinal_after = uint.checked_add_constant(region, &ordinal, 1)?;
            // The Request policy epoch is not newer than the payer's.
            let request_epoch: U64<F> =
                uint.assign_u64(region, value(send, |send| send.request_policy_epoch))?;
            uint.assert_le(region, &request_epoch, &policy)?;
            // max(floor, request time) <= lower <= upper.
            let request_time: U64<F> =
                uint.assign_u64(region, value(send, |send| send.request_time))?;
            let lower: U64<F> = uint.assign_u64(region, value(send, |send| send.accepted_lower))?;
            let upper: U64<F> = uint.assign_u64(region, value(send, |send| send.accepted_upper))?;
            uint.assert_le(region, &floor, &lower)?;
            uint.assert_le(region, &request_time, &lower)?;
            uint.assert_le(region, &lower, &upper)?;

            let digest_pair = pair_at(&words, digest_pair)?;
            let entry = absorb(&[
                send_chain,
                credit_id[0],
                credit_id[1],
                counterparty[0],
                counterparty[1],
                ordinal.word(),
                amount.word(),
                fee.word(),
                digest_pair[0],
                digest_pair[1],
            ]);
            let send_chain_after =
                hash_site(sponges, region, shape, plan, HashSite::Chain, &entry)?;
            check_digest(&send_chain_after, digests.map(|digests| digests.chain))?;

            let payer = pair_at(&words, payer)?;
            let fee_schedule = pair_at(&words, fee_schedule)?;
            let receiver_credential = pair_at(&words, receiver_credential)?;
            let scheme_policy = pair_at(&words, scheme_policy)?;
            let certificates = pair_at(&words, certificates)?;
            let nonce = pair_at(&words, nonce)?;
            let mut request = vec![AbsorbInput::Constant(F::from(REQUEST_VERSION))];
            request.extend(absorb(&[
                scheme[0],
                scheme[1],
                asset[0],
                asset[1],
                payer[0],
                payer[1],
                counterparty[0],
                counterparty[1],
                ordinal.word(),
                receiver_credential[0],
                receiver_credential[1],
                amount.word(),
                fee_schedule[0],
                fee_schedule[1],
                fee.word(),
                request_epoch.word(),
                scheme_policy[0],
                scheme_policy[1],
                request_time.word(),
                certificates[0],
                certificates[1],
                nonce[0],
                nonce[1],
            ]));
            let request = hash_site(sponges, region, shape, plan, HashSite::Request, &request)?;
            check_digest(&request, digests.and_then(|digests| digests.request))?;
            request_digest = Some(request);

            let dependencies = pair_at(&words, dependencies)?;
            effect = vec![
                credit_id[0].clone(),
                credit_id[1].clone(),
                counterparty[0].clone(),
                counterparty[1].clone(),
                ordinal.word().clone(),
                amount.word().clone(),
                fee.word().clone(),
                digest_pair[0].clone(),
                digest_pair[1].clone(),
                dependencies[0].clone(),
                dependencies[1].clone(),
                lower.word().clone(),
                upper.word().clone(),
            ];
            next_send = ordinal.word().clone();
            next_send_after = ordinal_after.word().clone();
            policy_epoch = policy.word().clone();
            time_floor = floor.word().clone();
            successor_balance = balance_after.word().clone();
            chain = send_chain_after;
        }
        (None, None, None, None) => {
            let (Some(ordinal), Some(policy), Some(floor)) =
                (free_next_send, free_policy, free_floor)
            else {
                return Err(Error::Synthesis);
            };
            // balance + amount < 2^128.
            let balance_after = uint.checked_add(region, &balance, &amount)?;
            let entry = absorb(&[
                recv_chain,
                credit_id[0],
                credit_id[1],
                counterparty[0],
                counterparty[1],
                amount.word(),
            ]);
            let recv_chain_after =
                hash_site(sponges, region, shape, plan, HashSite::Chain, &entry)?;
            check_digest(&recv_chain_after, digests.map(|digests| digests.chain))?;
            // Precomputed sigma_recv: the payment digest is zero.
            let zero = uint.glue().constant(region, F::ZERO)?;
            effect = vec![
                credit_id[0].clone(),
                credit_id[1].clone(),
                counterparty[0].clone(),
                counterparty[1].clone(),
                zero.clone(),
                zero.clone(),
                amount.word().clone(),
            ];
            next_send = word(ordinal)?.clone();
            next_send_after = next_send.clone();
            policy_epoch = word(policy)?.clone();
            time_floor = word(floor)?.clone();
            successor_balance = balance_after.word().clone();
            chain = recv_chain_after;
        }
        _ => return Err(Error::Synthesis),
    }

    // Predecessor and successor commitments.
    let predecessor_core = [
        balance.word(),
        sequence.word(),
        &next_send,
        next_load,
        send_chain,
        recv_chain,
        state_nonce,
        lifecycle,
        &policy_epoch,
        &time_floor,
    ];
    let (successor_send_chain, successor_recv_chain) = if is_send {
        (&chain, recv_chain)
    } else {
        (send_chain, &chain)
    };
    let successor_core = [
        &successor_balance,
        sequence_after.word(),
        &next_send_after,
        next_load,
        successor_send_chain,
        successor_recv_chain,
        successor_nonce,
        lifecycle,
        &policy_epoch,
        &time_floor,
    ];
    let predecessor = hash_site(
        sponges,
        region,
        shape,
        plan,
        HashSite::Predecessor,
        &absorb(&preimage(&predecessor_core, &rest)),
    )?;
    check_digest(&predecessor, digests.map(|digests| digests.predecessor))?;
    let successor = hash_site(
        sponges,
        region,
        shape,
        plan,
        HashSite::Successor,
        &absorb(&preimage(&successor_core, &rest)),
    )?;
    check_digest(&successor, digests.map(|digests| digests.successor))?;

    // The G1 statement digest.
    let predecessor_other = pair_at(&words, predecessor_other)?;
    let successor_other = pair_at(&words, successor_other)?;
    let cells = StatementCells {
        scheme_id: scheme,
        credential,
        asset,
        lifecycle,
        sequence: sequence_after.word(),
        next_load,
        predecessor: &predecessor,
        predecessor_other,
        successor: &successor,
        successor_other,
        effect: &effect,
    };
    let statement_lane = plan.lane_of(HashSite::Statement);
    let statement = statement_digest(
        sponges.get_mut(statement_lane).ok_or(Error::Synthesis)?,
        region,
        shape.step,
        &cells,
    )?;
    check_digest(&statement, digests.map(|digests| digests.statement))?;

    let in_circuit = statement
        .value()
        .zip(predecessor.value())
        .zip(successor.value())
        .zip(chain.value())
        .map(
            |(((statement, predecessor), successor), chain)| StepDigests {
                predecessor,
                successor,
                chain,
                request: None,
                statement,
            },
        )
        .zip(
            request_digest
                .as_ref()
                .map_or_else(|| Value::known(None), |request| request.value().map(Some)),
        )
        .map(|(digests, request)| StepDigests { request, ..digests });
    let mut public = vec![statement];
    public.extend(request_digest);
    let glue_end = uint.glue().next_row();
    let range_rows = uint.range().next_row();
    let glue_rows = glue_end
        .checked_sub(glue_start)
        .ok_or(Error::BoundsFailure)?;
    let lane_permutations: Vec<usize> = sponges
        .iter()
        .map(|sponge| sponge.lane().next_block())
        .collect();
    let mut lane_rows: Vec<usize> = sponges
        .iter()
        .map(|sponge| sponge.lane().rows_used())
        .collect();
    if let Some(rows) = lane_rows.get_mut(plan.glue_lane()) {
        *rows = glue_end.max(*rows);
    }
    let cells = lane_permutations
        .iter()
        .sum::<usize>()
        .checked_mul(iroha_plonk_gadgets::poseidon::CELLS_PER_PERMUTATION)
        .and_then(|cells| cells.checked_add(glue_rows.checked_mul(4)?))
        .and_then(|cells| cells.checked_add(range_rows))
        .ok_or(Error::BoundsFailure)?;
    Ok(RelationOutput {
        public,
        digests: in_circuit,
        inventory: Inventory {
            lane_permutations,
            lane_rows,
            glue_rows,
            range_rows,
            cells,
        },
    })
}
