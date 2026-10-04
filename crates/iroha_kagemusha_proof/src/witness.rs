//! **Prototype** native witnesses of the step relations and their reference
//! evaluation (M7 semantics, `g3_proof_scaling_measurement_tests.rs`
//! `m7_step`).
//!
//! # State
//!
//! A wallet state has 40 fields: a 10-field core the step relations read or
//! write ([`CoreState`]) and a 30-field remainder they carry
//! ([`StateRemainder`]). Two commitment layouts are supported
//! ([`StateLayout`]):
//!
//! - **two-level** (the M7 recommendation):
//!   `H(m7score1, core || H(m7srest1, remainder))`, 7 permutations per
//!   opening (6 with a folded prefix). The remainder digest is a carried
//!   witness; identity and policy fields that live only in the remainder
//!   (scheme, asset, wallet, credential, fee schedule) are witnesses bound by
//!   the public digests. The split-lineage review (change 1) binds them
//!   natively through the predecessor lineage proof's public outputs.
//! - **flat** (M6): `H(kgmstate, core || remainder)`, 22 permutations per
//!   opening (21 folded); identity fields are the opened cells.
//!
//! # Relations
//!
//! Both steps open the predecessor commitment, require lifecycle Active, a
//! nonzero `u128` amount and `sequence + 1 < 2^128`, and commit the
//! successor with a fresh state nonce.
//!
//! - `sigma_send` debits `amount + fee` without overdraft
//!   (`balance - amount - fee` is a `u128`), advances the send ordinal
//!   (`next_send + 1 < 2^128`), requires the Request policy epoch not newer
//!   than the payer's (`request_epoch <= policy_epoch`, `u64`) and the
//!   accepted-time window `max(accepted_time_floor, request_time) <= lower`
//!   and `lower <= upper` (`u64`), appends `send_chain' = H(m7sendc1, ...)`
//!   over the previous chain, `credit_id`, receiver, send ordinal, amount,
//!   fee and Request digest, and hashes the 24-field Request body (the
//!   `credit_id` preimage) under `m7reqst1`, which is public.
//! - `sigma_recv` credits the amount (`balance + amount < 2^128`) and appends
//!   `recv_chain' = H(m7recvc1, recv_chain, credit_id, payer, amount)`. It is
//!   the precomputed form: the payment digest in its effect is zero (review
//!   change 2); the lineage relation binds it.
//!
//! The public statement is the 32-field G1 encoding
//! ([`iroha_plonk_gadgets::statement`]) hashed under `m7stmnt1`.
//!
//! # Native reference
//!
//! [`StepWitness::evaluate`] computes every digest and the successor core
//! with field arithmetic, exactly as the circuit does, and lists the
//! relation [`Violation`]s. An honest witness has none; the circuit has no
//! satisfying assignment for a witness with any.

use iroha_pasta::poseidon::{PoseidonField, hash_with_domain};
use iroha_plonk_gadgets::statement::{
    EFFECT_UNION_FIELDS, STATEMENT_DOMAIN, STATEMENT_FIELDS, STATEMENT_VERSION, StepRelation,
    bytes_to_limbs,
};

/// Core fields: the fields the step relations read or write.
pub const CORE_FIELDS: usize = 10;
/// Remainder fields: the state version, five 128-bit limb pairs and 19
/// other carried fields.
pub const REMAINDER_FIELDS: usize = 30;
/// Fields of the flat state commitment.
pub const STATE_FIELDS: usize = CORE_FIELDS + REMAINDER_FIELDS;
/// Other carried remainder fields (map roots, remaining policy fields).
pub const OTHER_CARRIED_FIELDS: usize = 19;
/// Fields of the Request body (the `credit_id` preimage).
pub const REQUEST_FIELDS: usize = 24;
/// Fields of a `send_chain` entry.
pub const SEND_CHAIN_FIELDS: usize = 10;
/// Fields of a `recv_chain` entry.
pub const RECEIVE_CHAIN_FIELDS: usize = 6;
/// Effect fields of `sigma_send`.
pub const SEND_EFFECT_FIELDS: usize = 13;
/// Effect fields of `sigma_recv`.
pub const RECEIVE_EFFECT_FIELDS: usize = 7;
/// The state version.
pub const STATE_VERSION: u64 = 1;
/// The Request body version.
pub const REQUEST_VERSION: u64 = 1;
/// The Active lifecycle.
pub const LIFECYCLE_ACTIVE: u64 = 1;

/// Two-level core commitment domain (M7 label).
pub const CORE_DOMAIN: u64 = u64::from_le_bytes(*b"m7score1");
/// Two-level remainder digest domain (M7 label).
pub const REMAINDER_DOMAIN: u64 = u64::from_le_bytes(*b"m7srest1");
/// Flat state commitment domain (`kagemusha_v1_poseidon` state domain).
pub const FLAT_STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgmstate");
/// Request body domain (M7 label).
pub const REQUEST_DOMAIN: u64 = u64::from_le_bytes(*b"m7reqst1");
/// `send_chain` domain (M7 label).
pub const SEND_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"m7sendc1");
/// `recv_chain` domain (M7 label).
pub const RECEIVE_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"m7recvc1");

const _: () = assert!(SEND_EFFECT_FIELDS <= EFFECT_UNION_FIELDS);
const _: () = assert!(RECEIVE_EFFECT_FIELDS <= EFFECT_UNION_FIELDS);

/// Core field positions.
pub mod core_index {
    /// The `u128` balance.
    pub const BALANCE: usize = 0;
    /// The `u128` sequence number.
    pub const SEQUENCE: usize = 1;
    /// The next send ordinal.
    pub const NEXT_SEND: usize = 2;
    /// The next load ordinal.
    pub const NEXT_LOAD: usize = 3;
    /// The send chain accumulator.
    pub const SEND_CHAIN: usize = 4;
    /// The receive chain accumulator.
    pub const RECEIVE_CHAIN: usize = 5;
    /// The state nonce.
    pub const STATE_NONCE: usize = 6;
    /// The lifecycle.
    pub const LIFECYCLE: usize = 7;
    /// The `u64` policy epoch.
    pub const POLICY_EPOCH: usize = 8;
    /// The `u64` accepted-time floor.
    pub const TIME_FLOOR: usize = 9;
}

/// How a state is committed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateLayout {
    /// `H(core || H(remainder))` (the M7 recommendation).
    #[default]
    TwoLevel,
    /// `H(core || remainder)` (M6).
    Flat,
}

impl StateLayout {
    /// The state commitment domain.
    #[must_use]
    pub const fn domain(self) -> u64 {
        match self {
            Self::TwoLevel => CORE_DOMAIN,
            Self::Flat => FLAT_STATE_DOMAIN,
        }
    }

    /// The arity of the state commitment hash.
    #[must_use]
    pub const fn commitment_arity(self) -> usize {
        match self {
            Self::TwoLevel => CORE_FIELDS + 1,
            Self::Flat => STATE_FIELDS,
        }
    }

    /// A short label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TwoLevel => "two_level",
            Self::Flat => "flat",
        }
    }
}

/// The 10 core fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreState<F> {
    /// The balance.
    pub balance: u128,
    /// The sequence number.
    pub sequence: u128,
    /// The next send ordinal.
    pub next_send: u128,
    /// The next load ordinal.
    pub next_load: u128,
    /// The send chain accumulator.
    pub send_chain: F,
    /// The receive chain accumulator.
    pub recv_chain: F,
    /// The state nonce.
    pub state_nonce: F,
    /// The lifecycle ([`LIFECYCLE_ACTIVE`] for a usable state).
    pub lifecycle: u64,
    /// The policy epoch.
    pub policy_epoch: u64,
    /// The accepted-time floor.
    pub accepted_time_floor: u64,
}

impl<F: PoseidonField> CoreState<F> {
    /// The field encoding, in [`core_index`] order.
    #[must_use]
    pub fn fields(&self) -> [F; CORE_FIELDS] {
        [
            F::from_u128(self.balance),
            F::from_u128(self.sequence),
            F::from_u128(self.next_send),
            F::from_u128(self.next_load),
            self.send_chain,
            self.recv_chain,
            self.state_nonce,
            F::from(self.lifecycle),
            F::from(self.policy_epoch),
            F::from(self.accepted_time_floor),
        ]
    }
}

/// The 30 remainder fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRemainder<F> {
    /// The state version.
    pub version: u64,
    /// The scheme identifier.
    pub scheme_id: [u8; 32],
    /// The asset identifier.
    pub asset: [u8; 32],
    /// The wallet identifier.
    pub wallet_id: [u8; 32],
    /// The credential digest.
    pub credential: [u8; 32],
    /// The fee schedule digest.
    pub fee_schedule: [u8; 32],
    /// The other carried fields.
    pub other: [F; OTHER_CARRIED_FIELDS],
}

/// The two 128-bit limbs of a 32-byte value as field elements.
#[must_use]
pub fn limbs<F: PoseidonField>(bytes: &[u8; 32]) -> [F; 2] {
    bytes_to_limbs(bytes).map(F::from_u128)
}

impl<F: PoseidonField> StateRemainder<F> {
    /// The field encoding: version, scheme, asset, wallet, credential and fee
    /// schedule limbs, then the other carried fields.
    #[must_use]
    pub fn fields(&self) -> [F; REMAINDER_FIELDS] {
        let mut fields = [F::ZERO; REMAINDER_FIELDS];
        fields[0] = F::from(self.version);
        let pairs = [
            &self.scheme_id,
            &self.asset,
            &self.wallet_id,
            &self.credential,
            &self.fee_schedule,
        ];
        for (index, bytes) in pairs.into_iter().enumerate() {
            let [lo, hi] = limbs::<F>(bytes);
            fields[1 + 2 * index] = lo;
            fields[2 + 2 * index] = hi;
        }
        fields[1 + 2 * pairs.len()..].copy_from_slice(&self.other);
        fields
    }

    /// The two-level remainder digest `H(m7srest1, fields)`.
    #[must_use]
    pub fn digest(&self) -> F {
        hash_with_domain(REMAINDER_DOMAIN, &self.fields())
    }
}

/// A wallet state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateV1<F> {
    /// The core.
    pub core: CoreState<F>,
    /// The remainder.
    pub remainder: StateRemainder<F>,
}

/// The state commitment preimage of `core` under `layout`.
fn commitment_preimage<F: PoseidonField>(
    layout: StateLayout,
    core: &[F; CORE_FIELDS],
    remainder: &StateRemainder<F>,
) -> Vec<F> {
    let mut inputs = core.to_vec();
    match layout {
        StateLayout::TwoLevel => inputs.push(remainder.digest()),
        StateLayout::Flat => inputs.extend_from_slice(&remainder.fields()),
    }
    inputs
}

impl<F: PoseidonField> StateV1<F> {
    /// The state commitment under `layout`.
    #[must_use]
    pub fn commitment(&self, layout: StateLayout) -> F {
        let preimage = commitment_preimage(layout, &self.core.fields(), &self.remainder);
        hash_with_domain(layout.domain(), &preimage)
    }
}

/// The `sigma_send` inputs beyond the predecessor state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendInputs {
    /// The amount.
    pub amount: u128,
    /// The fee.
    pub fee: u128,
    /// The credit identifier.
    pub credit_id: [u8; 32],
    /// The receiver wallet.
    pub receiver_wallet: [u8; 32],
    /// The receiver credential.
    pub receiver_credential: [u8; 32],
    /// The Request policy epoch.
    pub request_policy_epoch: u64,
    /// The Request (receiver) time.
    pub request_time: u64,
    /// The accepted lower time.
    pub accepted_lower: u64,
    /// The accepted upper time.
    pub accepted_upper: u64,
    /// The scheme policy digest.
    pub scheme_policy: [u8; 32],
    /// The certificates digest.
    pub certificates: [u8; 32],
    /// The Request nonce.
    pub request_nonce: [u8; 32],
    /// The Request byte digest carried in the pending-outgoing leaf.
    pub request_digest: [u8; 32],
    /// The dependencies digest.
    pub dependencies: [u8; 32],
}

/// The `sigma_recv` inputs beyond the predecessor state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveInputs {
    /// The amount.
    pub amount: u128,
    /// The credit identifier.
    pub credit_id: [u8; 32],
    /// The payer wallet.
    pub payer_wallet: [u8; 32],
}

/// The step-specific inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepInputs {
    /// `sigma_send`.
    Send(Box<SendInputs>),
    /// `sigma_recv`.
    Receive(Box<ReceiveInputs>),
}

impl StepInputs {
    /// The step relation these inputs belong to.
    #[must_use]
    pub const fn relation(&self) -> StepRelation {
        match self {
            Self::Send(_) => StepRelation::Send,
            Self::Receive(_) => StepRelation::Receive,
        }
    }

    /// The amount.
    #[must_use]
    pub const fn amount(&self) -> u128 {
        match self {
            Self::Send(send) => send.amount,
            Self::Receive(receive) => receive.amount,
        }
    }
}

/// A full step witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepWitness<F> {
    /// The predecessor state.
    pub predecessor: StateV1<F>,
    /// The successor's fresh state nonce.
    pub successor_nonce: F,
    /// The canonical encoding of the predecessor's other-parity commitment
    /// component.
    pub predecessor_other: [u8; 32],
    /// The canonical encoding of the successor's other-parity commitment
    /// component.
    pub successor_other: [u8; 32],
    /// The step inputs.
    pub inputs: StepInputs,
}

/// A relation rule the witness breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Violation {
    /// The predecessor lifecycle is not Active.
    LifecycleNotActive,
    /// The amount is zero.
    ZeroAmount,
    /// `sequence + 1` reaches `2^128`.
    SequenceOverflow,
    /// `sigma_send`: `amount + fee` exceeds the balance.
    Overdraft,
    /// `sigma_send`: `next_send + 1` reaches `2^128`.
    SendOrdinalOverflow,
    /// `sigma_send`: the Request policy epoch is newer than the payer's.
    PolicyEpochNewer,
    /// `sigma_send`: the accepted lower time is below the accepted-time
    /// floor.
    AcceptedTimeBelowFloor,
    /// `sigma_send`: the accepted lower time is below the Request time.
    AcceptedTimeBelowRequest,
    /// `sigma_send`: the accepted upper time is below the lower time.
    AcceptedWindowInverted,
    /// `sigma_recv`: `balance + amount` reaches `2^128`.
    BalanceOverflow,
}

/// The public outputs of a step proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepPublic<F> {
    /// The statement digest.
    pub statement: F,
    /// `sigma_send` only: the Request digest.
    pub request: Option<F>,
}

impl<F: Copy> StepPublic<F> {
    /// The instance column: the statement digest, then the Request digest.
    #[must_use]
    pub fn instance(&self) -> Vec<F> {
        let mut instance = vec![self.statement];
        instance.extend(self.request);
        instance
    }
}

/// The number of public outputs of `relation`.
#[must_use]
pub const fn public_outputs(relation: StepRelation) -> usize {
    match relation {
        StepRelation::Send => 2,
        StepRelation::Receive => 1,
    }
}

/// Every digest a step computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepDigests<F> {
    /// The predecessor commitment.
    pub predecessor: F,
    /// The successor commitment.
    pub successor: F,
    /// The appended chain accumulator (`send_chain'` or `recv_chain'`).
    pub chain: F,
    /// `sigma_send` only: the Request digest.
    pub request: Option<F>,
    /// The statement digest.
    pub statement: F,
}

/// The reference evaluation of a step (field arithmetic, as in circuit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStep<F> {
    /// The successor core fields.
    pub successor_core: [F; CORE_FIELDS],
    /// The chain entry that was hashed.
    pub chain_entry: Vec<F>,
    /// `sigma_send` only: the Request body.
    pub request: Option<[F; REQUEST_FIELDS]>,
    /// The statement encoding.
    pub statement: [F; STATEMENT_FIELDS],
    /// The digests.
    pub digests: StepDigests<F>,
    /// The rules the witness breaks (empty for an honest witness).
    pub violations: Vec<Violation>,
}

impl<F: Copy> NativeStep<F> {
    /// The public outputs the circuit computes from the witness.
    #[must_use]
    pub const fn public(&self) -> StepPublic<F> {
        StepPublic {
            statement: self.digests.statement,
            request: self.digests.request,
        }
    }

    /// Whether the witness satisfies the relation.
    #[must_use]
    pub fn is_honest(&self) -> bool {
        self.violations.is_empty()
    }
}

/// The 13-field `sigma_send` effect.
fn send_effect<F: PoseidonField>(core: &CoreState<F>, send: &SendInputs) -> Vec<F> {
    let [credit_lo, credit_hi] = limbs::<F>(&send.credit_id);
    let [receiver_lo, receiver_hi] = limbs::<F>(&send.receiver_wallet);
    let [digest_lo, digest_hi] = limbs::<F>(&send.request_digest);
    let [dependencies_lo, dependencies_hi] = limbs::<F>(&send.dependencies);
    vec![
        credit_lo,
        credit_hi,
        receiver_lo,
        receiver_hi,
        F::from_u128(core.next_send),
        F::from_u128(send.amount),
        F::from_u128(send.fee),
        digest_lo,
        digest_hi,
        dependencies_lo,
        dependencies_hi,
        F::from(send.accepted_lower),
        F::from(send.accepted_upper),
    ]
}

/// The 7-field `sigma_recv` effect (precomputed: the payment digest is
/// zero).
fn receive_effect<F: PoseidonField>(receive: &ReceiveInputs) -> Vec<F> {
    let [credit_lo, credit_hi] = limbs::<F>(&receive.credit_id);
    let [payer_lo, payer_hi] = limbs::<F>(&receive.payer_wallet);
    vec![
        credit_lo,
        credit_hi,
        payer_lo,
        payer_hi,
        F::ZERO,
        F::ZERO,
        F::from_u128(receive.amount),
    ]
}

/// The 24-field Request body.
fn request_fields<F: PoseidonField>(state: &StateV1<F>, send: &SendInputs) -> [F; REQUEST_FIELDS] {
    let remainder = &state.remainder;
    let pair = |bytes: &[u8; 32]| limbs::<F>(bytes);
    let [scheme_lo, scheme_hi] = pair(&remainder.scheme_id);
    let [asset_lo, asset_hi] = pair(&remainder.asset);
    let [payer_lo, payer_hi] = pair(&remainder.wallet_id);
    let [receiver_lo, receiver_hi] = pair(&send.receiver_wallet);
    let [credential_lo, credential_hi] = pair(&send.receiver_credential);
    let [schedule_lo, schedule_hi] = pair(&remainder.fee_schedule);
    let [policy_lo, policy_hi] = pair(&send.scheme_policy);
    let [certificates_lo, certificates_hi] = pair(&send.certificates);
    let [nonce_lo, nonce_hi] = pair(&send.request_nonce);
    [
        F::from(REQUEST_VERSION),
        scheme_lo,
        scheme_hi,
        asset_lo,
        asset_hi,
        payer_lo,
        payer_hi,
        receiver_lo,
        receiver_hi,
        F::from_u128(state.core.next_send),
        credential_lo,
        credential_hi,
        F::from_u128(send.amount),
        schedule_lo,
        schedule_hi,
        F::from_u128(send.fee),
        F::from(send.request_policy_epoch),
        policy_lo,
        policy_hi,
        F::from(send.request_time),
        certificates_lo,
        certificates_hi,
        nonce_lo,
        nonce_hi,
    ]
}

/// The relation rules `witness` breaks.
fn violations<F>(witness: &StepWitness<F>) -> Vec<Violation> {
    let core = &witness.predecessor.core;
    let mut found = Vec::new();
    if core.lifecycle != LIFECYCLE_ACTIVE {
        found.push(Violation::LifecycleNotActive);
    }
    if witness.inputs.amount() == 0 {
        found.push(Violation::ZeroAmount);
    }
    if core.sequence == u128::MAX {
        found.push(Violation::SequenceOverflow);
    }
    match &witness.inputs {
        StepInputs::Send(send) => {
            let affordable = send
                .amount
                .checked_add(send.fee)
                .is_some_and(|debit| debit <= core.balance);
            if !affordable {
                found.push(Violation::Overdraft);
            }
            if core.next_send == u128::MAX {
                found.push(Violation::SendOrdinalOverflow);
            }
            if send.request_policy_epoch > core.policy_epoch {
                found.push(Violation::PolicyEpochNewer);
            }
            if send.accepted_lower < core.accepted_time_floor {
                found.push(Violation::AcceptedTimeBelowFloor);
            }
            if send.accepted_lower < send.request_time {
                found.push(Violation::AcceptedTimeBelowRequest);
            }
            if send.accepted_upper < send.accepted_lower {
                found.push(Violation::AcceptedWindowInverted);
            }
        }
        StepInputs::Receive(receive) => {
            if core.balance.checked_add(receive.amount).is_none() {
                found.push(Violation::BalanceOverflow);
            }
        }
    }
    found
}

/// The 32-field statement encoding (the layout of
/// [`iroha_plonk_gadgets::statement::StatementV1::encode`], with field
/// arithmetic for the successor sequence).
fn statement_fields<F: PoseidonField>(
    relation: StepRelation,
    witness: &StepWitness<F>,
    sequence_after: F,
    predecessor: F,
    successor: F,
    effect: &[F],
) -> [F; STATEMENT_FIELDS] {
    let state = &witness.predecessor;
    let remainder = &state.remainder;
    let [scheme_lo, scheme_hi] = limbs::<F>(&remainder.scheme_id);
    let [credential_lo, credential_hi] = limbs::<F>(&remainder.credential);
    let [asset_lo, asset_hi] = limbs::<F>(&remainder.asset);
    let [predecessor_lo, predecessor_hi] = limbs::<F>(&witness.predecessor_other);
    let [successor_lo, successor_hi] = limbs::<F>(&witness.successor_other);
    let mut fields = [F::ZERO; STATEMENT_FIELDS];
    let head = [
        F::from(STATEMENT_VERSION),
        scheme_lo,
        scheme_hi,
        F::from_u128(relation.relation_id()),
        F::ZERO,
        credential_lo,
        credential_hi,
        asset_lo,
        asset_hi,
        F::from(state.core.lifecycle),
        sequence_after,
        F::from_u128(state.core.next_load),
        predecessor,
        predecessor_lo,
        predecessor_hi,
        successor,
        successor_lo,
        successor_hi,
        F::from(relation.effect_tag()),
    ];
    fields[..head.len()].copy_from_slice(&head);
    fields[head.len()..head.len() + effect.len()].copy_from_slice(effect);
    fields
}

impl<F: PoseidonField> StepWitness<F> {
    /// The step relation of this witness.
    #[must_use]
    pub const fn relation(&self) -> StepRelation {
        self.inputs.relation()
    }

    /// The reference evaluation under `layout`.
    #[must_use]
    pub fn evaluate(&self, layout: StateLayout) -> NativeStep<F> {
        let state = &self.predecessor;
        let core = state.core.fields();
        let predecessor = state.commitment(layout);
        let mut successor_core = core;
        successor_core[core_index::SEQUENCE] += F::ONE;
        successor_core[core_index::STATE_NONCE] = self.successor_nonce;
        let relation = self.relation();
        let (chain_entry, chain_domain, request, effect) = match &self.inputs {
            StepInputs::Send(send) => {
                let [credit_lo, credit_hi] = limbs::<F>(&send.credit_id);
                let [receiver_lo, receiver_hi] = limbs::<F>(&send.receiver_wallet);
                let [digest_lo, digest_hi] = limbs::<F>(&send.request_digest);
                let entry = vec![
                    core[core_index::SEND_CHAIN],
                    credit_lo,
                    credit_hi,
                    receiver_lo,
                    receiver_hi,
                    core[core_index::NEXT_SEND],
                    F::from_u128(send.amount),
                    F::from_u128(send.fee),
                    digest_lo,
                    digest_hi,
                ];
                let debit = F::from_u128(send.amount) + F::from_u128(send.fee);
                successor_core[core_index::BALANCE] -= debit;
                successor_core[core_index::NEXT_SEND] += F::ONE;
                (
                    entry,
                    SEND_CHAIN_DOMAIN,
                    Some(request_fields(state, send)),
                    send_effect(&state.core, send),
                )
            }
            StepInputs::Receive(receive) => {
                let [credit_lo, credit_hi] = limbs::<F>(&receive.credit_id);
                let [payer_lo, payer_hi] = limbs::<F>(&receive.payer_wallet);
                let entry = vec![
                    core[core_index::RECEIVE_CHAIN],
                    credit_lo,
                    credit_hi,
                    payer_lo,
                    payer_hi,
                    F::from_u128(receive.amount),
                ];
                successor_core[core_index::BALANCE] += F::from_u128(receive.amount);
                (entry, RECEIVE_CHAIN_DOMAIN, None, receive_effect(receive))
            }
        };
        let chain = hash_with_domain(chain_domain, &chain_entry);
        match relation {
            StepRelation::Send => successor_core[core_index::SEND_CHAIN] = chain,
            StepRelation::Receive => successor_core[core_index::RECEIVE_CHAIN] = chain,
        }
        let successor_preimage = commitment_preimage(layout, &successor_core, &state.remainder);
        let successor = hash_with_domain(layout.domain(), &successor_preimage);
        let request_digest = request
            .as_ref()
            .map(|fields| hash_with_domain(REQUEST_DOMAIN, fields));
        let statement = statement_fields(
            relation,
            self,
            successor_core[core_index::SEQUENCE],
            predecessor,
            successor,
            &effect,
        );
        let statement_digest = hash_with_domain(STATEMENT_DOMAIN, &statement);
        NativeStep {
            successor_core,
            chain_entry,
            request,
            statement,
            digests: StepDigests {
                predecessor,
                successor,
                chain,
                request: request_digest,
                statement: statement_digest,
            },
            violations: violations(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use ff::{Field, PrimeField};
    use iroha_pasta::{Fp, Fq};
    use iroha_plonk_gadgets::statement::StatementV1;

    use super::*;
    use crate::vectors::{Mutation, sample_witness};

    #[test]
    fn domains_are_the_m7_labels() {
        assert_eq!(CORE_DOMAIN.to_le_bytes(), *b"m7score1");
        assert_eq!(REMAINDER_DOMAIN.to_le_bytes(), *b"m7srest1");
        assert_eq!(FLAT_STATE_DOMAIN.to_le_bytes(), *b"kgmstate");
        assert_eq!(REQUEST_DOMAIN.to_le_bytes(), *b"m7reqst1");
        assert_eq!(SEND_CHAIN_DOMAIN.to_le_bytes(), *b"m7sendc1");
        assert_eq!(RECEIVE_CHAIN_DOMAIN.to_le_bytes(), *b"m7recvc1");
        assert_eq!(StateLayout::TwoLevel.commitment_arity(), 11);
        assert_eq!(StateLayout::Flat.commitment_arity(), 40);
        assert_eq!(StateLayout::Flat.domain(), FLAT_STATE_DOMAIN);
        assert_eq!(StateLayout::TwoLevel.label(), "two_level");
        assert_eq!(public_outputs(StepRelation::Send), 2);
        assert_eq!(public_outputs(StepRelation::Receive), 1);
    }

    #[test]
    fn field_encodings_follow_the_m7_layout() {
        let witness = sample_witness::<Fp>(3, StepRelation::Send, Mutation::None);
        let state = witness.predecessor;
        let core = state.core.fields();
        assert_eq!(core[core_index::BALANCE], Fp::from_u128(state.core.balance));
        assert_eq!(core[core_index::LIFECYCLE], Fp::ONE);
        assert_eq!(
            core[core_index::TIME_FLOOR],
            Fp::from(state.core.accepted_time_floor)
        );
        let remainder = state.remainder.fields();
        assert_eq!(remainder[0], Fp::from(STATE_VERSION));
        assert_eq!(
            [remainder[1], remainder[2]],
            limbs::<Fp>(&state.remainder.scheme_id)
        );
        assert_eq!(
            [remainder[9], remainder[10]],
            limbs::<Fp>(&state.remainder.fee_schedule)
        );
        assert_eq!(remainder[11..], state.remainder.other);
        assert_eq!(
            state.commitment(StateLayout::TwoLevel),
            hash_with_domain(
                CORE_DOMAIN,
                &[core.as_slice(), &[state.remainder.digest()]].concat()
            )
        );
        assert_eq!(
            state.commitment(StateLayout::Flat),
            hash_with_domain(FLAT_STATE_DOMAIN, &[core.as_slice(), &remainder].concat())
        );
    }

    /// The statement encoding equals the gadgets' `StatementV1` for honest
    /// witnesses (where the successor sequence fits a `u128`).
    fn statement_matches_gadgets<F: PoseidonField>(relation: StepRelation) {
        for layout in [StateLayout::TwoLevel, StateLayout::Flat] {
            let witness = sample_witness::<F>(11, relation, Mutation::None);
            let native = witness.evaluate(layout);
            assert!(native.is_honest(), "{:?}", native.violations);
            let state = &witness.predecessor;
            let effect = native.statement[19..]
                .iter()
                .copied()
                .take(match relation {
                    StepRelation::Send => SEND_EFFECT_FIELDS,
                    StepRelation::Receive => RECEIVE_EFFECT_FIELDS,
                })
                .collect();
            let reference = StatementV1 {
                relation,
                scheme_id: state.remainder.scheme_id,
                credential: state.remainder.credential,
                asset: state.remainder.asset,
                lifecycle: state.core.lifecycle,
                sequence: state.core.sequence + 1,
                next_load: state.core.next_load,
                predecessor: native.digests.predecessor,
                predecessor_other: witness.predecessor_other,
                successor: native.digests.successor,
                successor_other: witness.successor_other,
                effect,
            };
            assert_eq!(reference.encode(), Some(native.statement));
            assert_eq!(reference.digest(), Some(native.digests.statement));
            assert_eq!(native.public().statement, native.digests.statement);
            assert_eq!(native.public().request, native.digests.request);
        }
    }

    #[test]
    fn statements_match_the_gadget_encoding_on_both_fields() {
        for relation in [StepRelation::Send, StepRelation::Receive] {
            statement_matches_gadgets::<Fp>(relation);
            statement_matches_gadgets::<Fq>(relation);
        }
    }

    #[test]
    fn send_reference_debits_and_appends() {
        let witness = sample_witness::<Fp>(5, StepRelation::Send, Mutation::None);
        let StepInputs::Send(send) = &witness.inputs else {
            panic!("send witness");
        };
        let native = witness.evaluate(StateLayout::TwoLevel);
        let core = witness.predecessor.core;
        assert_eq!(
            native.successor_core[core_index::BALANCE],
            Fp::from_u128(core.balance - send.amount - send.fee)
        );
        assert_eq!(
            native.successor_core[core_index::NEXT_SEND],
            Fp::from_u128(core.next_send + 1)
        );
        assert_eq!(
            native.successor_core[core_index::SEND_CHAIN],
            hash_with_domain(SEND_CHAIN_DOMAIN, &native.chain_entry)
        );
        assert_eq!(native.chain_entry.len(), SEND_CHAIN_FIELDS);
        let request = native.request.expect("send request");
        assert_eq!(request[9], Fp::from_u128(core.next_send));
        assert_eq!(
            native.digests.request,
            Some(hash_with_domain(REQUEST_DOMAIN, &request))
        );
        assert_eq!(native.public().instance().len(), 2);
    }

    #[test]
    fn receive_reference_credits_and_appends() {
        let witness = sample_witness::<Fq>(6, StepRelation::Receive, Mutation::None);
        let StepInputs::Receive(receive) = &witness.inputs else {
            panic!("receive witness");
        };
        let native = witness.evaluate(StateLayout::Flat);
        let core = witness.predecessor.core;
        assert_eq!(
            native.successor_core[core_index::BALANCE],
            Fq::from_u128(core.balance + receive.amount)
        );
        assert_eq!(native.chain_entry.len(), RECEIVE_CHAIN_FIELDS);
        assert_eq!(native.request, None);
        assert_eq!(native.public().instance(), vec![native.digests.statement]);
        assert_eq!(witness.relation(), StepRelation::Receive);
    }

    #[test]
    fn mutations_are_reported_as_violations() {
        let cases = [
            (
                StepRelation::Send,
                Mutation::Overdraft,
                Violation::Overdraft,
            ),
            (
                StepRelation::Send,
                Mutation::StaleEpoch,
                Violation::PolicyEpochNewer,
            ),
            (
                StepRelation::Send,
                Mutation::EarlyTime,
                Violation::AcceptedTimeBelowFloor,
            ),
            (
                StepRelation::Receive,
                Mutation::Overflow,
                Violation::BalanceOverflow,
            ),
        ];
        for (relation, mutation, violation) in cases {
            let native =
                sample_witness::<Fp>(9, relation, mutation).evaluate(StateLayout::TwoLevel);
            assert!(native.violations.contains(&violation), "{mutation:?}");
        }
        let mut witness = sample_witness::<Fp>(9, StepRelation::Send, Mutation::None);
        witness.predecessor.core.lifecycle = 2;
        witness.predecessor.core.sequence = u128::MAX;
        witness.predecessor.core.next_send = u128::MAX;
        if let StepInputs::Send(send) = &mut witness.inputs {
            send.amount = 0;
            send.accepted_upper = 0;
        }
        let found = witness.evaluate(StateLayout::Flat).violations;
        for violation in [
            Violation::LifecycleNotActive,
            Violation::ZeroAmount,
            Violation::SequenceOverflow,
            Violation::SendOrdinalOverflow,
            Violation::AcceptedWindowInverted,
        ] {
            assert!(found.contains(&violation), "{violation:?}");
        }
        // The overflowing successor sequence is the field value 2^128.
        let native = witness.evaluate(StateLayout::Flat);
        assert_eq!(
            native.successor_core[core_index::SEQUENCE],
            Fp::from_u128(u128::MAX) + Fp::ONE
        );
    }
}
