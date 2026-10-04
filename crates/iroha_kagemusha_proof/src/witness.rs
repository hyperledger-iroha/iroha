//! **Prototype** native witnesses of the step relations and their reference
//! evaluation (spec `kagemusha_single_design_proposal.md` sections 3 and
//! 3.2, controls off).
//!
//! # State
//!
//! A wallet state has 40 fields: a 30-field core the step relations open
//! ([`CoreState`]) and a 10-field remainder no step relation reads
//! ([`StateRemainder`]). The core holds every field spec section 3 assigns
//! to it: lifecycle, `wallet_id` and credential digest, balance,
//! `burned_total`, sequence and the send, load and redeem ordinals, both
//! chains, the five map roots, the enabled-controls mask, the quota windows
//! root, the blacklist version and root, the lease expiry, the policy epoch,
//! the accepted-time floor and the state nonce. It also holds the scheme and
//! asset identifiers (four limbs beyond the spec's list): the predecessor's
//! lineage proof exposes no asset, so a step proof can bind the asset of the
//! incarnation only by opening it.
//!
//! Two commitment layouts are supported ([`StateLayout`]):
//!
//! - **two-level** (spec section 3): `H(kgspcor1, core || H(kgsprst1,
//!   remainder))`, 17 permutations per opening (16 with a folded prefix).
//!   The remainder digest is carried; no step relation opens it.
//! - **flat**: `H(kgspflt1, core || remainder)`, 22 permutations per opening
//!   (21 folded).
//!
//! Each `(step, layout)` pair has its own relation identifier
//! ([`relation_id`]), so a verifier allowlist cannot take one layout's proof
//! for the other's.
//!
//! # Relations
//!
//! Both steps open the predecessor commitment, require lifecycle Active, a
//! nonzero `u128` amount and `sequence + 1 < 2^128`, hash the 24-field
//! Request body under the credit domain (the credit identifier,
//! `credit_id = H(kgspcrd1, Request body)`, spec section 5.1), decompose it
//! into its canonical limbs, require distinct payer and receiver wallets,
//! append one chain and commit the successor with a fresh state nonce. The
//! identity limbs (scheme, asset, `wallet_id`, credential) of the Request and
//! the statement are the opened core cells.
//!
//! - `sigma_send` takes `burned_total` and the pending-outgoing root of the
//!   predecessor's lineage proof as public inputs ([`LineageInputs`]).
//!   It requires the enabled-controls mask to be empty and checks
//!   `amount + fee < 2^128` and `amount + fee <= balance - burned_total`
//!   (both checked). It advances the send ordinal without overflow and
//!   requires `request_policy_epoch <= policy_epoch` and
//!   `max(accepted_time_floor, request_time) <= lower <= upper` (all `u64`).
//!   The successor balance is `balance - amount - fee`, its `burned_total`
//!   the lineage input, its accepted-time floor `lower` (accepted time never
//!   goes back), and its pending-outgoing and fee-claim roots are carried
//!   witnesses. The Request's payer is the core `wallet_id` and its send
//!   ordinal the core's `next_send`. `send_chain' = H(kgspsnd1, send_chain,
//!   credit_id, receiver, ordinal, amount, fee)`.
//! - `sigma_recv` credits the amount (`balance + amount < 2^128`). The
//!   Request's receiver and receiver credential are the core `wallet_id` and
//!   credential digest. `recv_chain' = H(kgsprcv1, recv_chain, credit_id,
//!   payer, amount)`, and the successor's consumed-credit root is a carried
//!   witness. Its statement contains no Payment digest (it is precomputed
//!   at Request signing).
//!
//! Map roots that a step does not touch are copied; the roots it updates are
//! carried witnesses. Spec section 3.2 assigns their transitions to the
//! native Advance check and to the lineage relation.
//!
//! The public statement is the 25-field step encoding
//! ([`iroha_plonk_gadgets::statement`]) hashed under `kgspstm1`.
//!
//! # Native reference
//!
//! [`StepWitness::evaluate`] computes every digest and the successor core
//! with field arithmetic, exactly as the circuit does, and lists the
//! relation [`Violation`]s. An honest witness has none; the circuit has no
//! satisfying assignment for a witness with any.

use iroha_pasta::poseidon::{PoseidonField, hash_with_domain};
use iroha_plonk_gadgets::statement::{
    EFFECT_UNION_FIELDS, STATEMENT_DOMAIN, STATEMENT_FIELDS, STATEMENT_VERSION, StatementV1,
    StepRelation, bytes_to_limbs, foreign_limbs,
};

/// Core fields: the fields the step relations open.
pub const CORE_FIELDS: usize = 30;
/// Remainder fields: the state version, the regulatory policy digest limbs
/// and 7 other carried fields.
pub const REMAINDER_FIELDS: usize = 10;
/// Fields of the flat state commitment.
pub const STATE_FIELDS: usize = CORE_FIELDS + REMAINDER_FIELDS;
/// Other carried remainder fields (time anchor, quota share body, ...).
pub const OTHER_CARRIED_FIELDS: usize = 7;
/// Fields of the Request body (the `credit_id` preimage).
pub const REQUEST_FIELDS: usize = 24;
/// Fields of a `send_chain` entry.
pub const SEND_CHAIN_FIELDS: usize = 8;
/// Fields of a `recv_chain` entry.
pub const RECEIVE_CHAIN_FIELDS: usize = 6;
/// Effect fields of `sigma_send`.
pub const SEND_EFFECT_FIELDS: usize = 9;
/// Effect fields of `sigma_recv`.
pub const RECEIVE_EFFECT_FIELDS: usize = 5;
/// The state version.
pub const STATE_VERSION: u64 = 1;
/// The Request body version.
pub const REQUEST_VERSION: u64 = 1;
/// The Active lifecycle.
pub const LIFECYCLE_ACTIVE: u64 = 1;

/// Two-level core commitment domain (prototype label).
pub const CORE_DOMAIN: u64 = u64::from_le_bytes(*b"kgspcor1");
/// Two-level remainder digest domain (prototype label).
pub const REMAINDER_DOMAIN: u64 = u64::from_le_bytes(*b"kgsprst1");
/// Flat state commitment domain (prototype label).
pub const FLAT_STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgspflt1");
/// Credit identifier domain: `credit_id = H(credit, Request body)`
/// (prototype label).
pub const CREDIT_DOMAIN: u64 = u64::from_le_bytes(*b"kgspcrd1");
/// `send_chain` domain (prototype label).
pub const SEND_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"kgspsnd1");
/// `recv_chain` domain (prototype label).
pub const RECEIVE_CHAIN_DOMAIN: u64 = u64::from_le_bytes(*b"kgsprcv1");

const _: () = assert!(SEND_EFFECT_FIELDS <= EFFECT_UNION_FIELDS);
const _: () = assert!(RECEIVE_EFFECT_FIELDS <= EFFECT_UNION_FIELDS);
const _: () = assert!(1 + 2 + OTHER_CARRIED_FIELDS == REMAINDER_FIELDS);

/// Core field positions.
pub mod core_index {
    /// The lifecycle.
    pub const LIFECYCLE: usize = 0;
    /// The scheme identifier limbs.
    pub const SCHEME: usize = 1;
    /// The asset identifier limbs.
    pub const ASSET: usize = 3;
    /// The `wallet_id` limbs.
    pub const WALLET: usize = 5;
    /// The credential digest limbs.
    pub const CREDENTIAL: usize = 7;
    /// The `u128` balance.
    pub const BALANCE: usize = 9;
    /// The `u128` `burned_total`.
    pub const BURNED_TOTAL: usize = 10;
    /// The `u128` sequence number.
    pub const SEQUENCE: usize = 11;
    /// The next send ordinal.
    pub const NEXT_SEND: usize = 12;
    /// The next load ordinal.
    pub const NEXT_LOAD: usize = 13;
    /// The next redeem ordinal.
    pub const NEXT_REDEEM: usize = 14;
    /// The send chain accumulator.
    pub const SEND_CHAIN: usize = 15;
    /// The receive chain accumulator.
    pub const RECEIVE_CHAIN: usize = 16;
    /// The consumed-credit root.
    pub const CONSUMED_CREDIT_ROOT: usize = 17;
    /// The pending-outgoing root.
    pub const PENDING_OUTGOING_ROOT: usize = 18;
    /// The load/redeem recovery root.
    pub const LOAD_REDEEM_ROOT: usize = 19;
    /// The fee-claim recovery root.
    pub const FEE_CLAIM_ROOT: usize = 20;
    /// The quota-usage root.
    pub const QUOTA_USAGE_ROOT: usize = 21;
    /// The enabled-controls mask.
    pub const ENABLED_CONTROLS: usize = 22;
    /// The quota share/windows root.
    pub const QUOTA_WINDOWS_ROOT: usize = 23;
    /// The blacklist version.
    pub const BLACKLIST_VERSION: usize = 24;
    /// The blacklist root.
    pub const BLACKLIST_ROOT: usize = 25;
    /// The lease expiry.
    pub const LEASE_EXPIRY: usize = 26;
    /// The `u64` policy epoch.
    pub const POLICY_EPOCH: usize = 27;
    /// The `u64` accepted-time floor.
    pub const TIME_FLOOR: usize = 28;
    /// The state nonce.
    pub const STATE_NONCE: usize = 29;
}

/// How a state is committed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateLayout {
    /// `H(core || H(remainder))` (spec section 3).
    #[default]
    TwoLevel,
    /// `H(core || remainder)`.
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

/// The relation identifier of `step` under `layout` (prototype labels): one
/// per pair, so a verifier allowlist never takes one layout's relation for
/// the other's.
#[must_use]
pub const fn relation_id(step: StepRelation, layout: StateLayout) -> u128 {
    u128::from_le_bytes(match (step, layout) {
        (StepRelation::Send, StateLayout::TwoLevel) => *b"kgsp-send-2level",
        (StepRelation::Send, StateLayout::Flat) => *b"kgsp-send-flat-1",
        (StepRelation::Receive, StateLayout::TwoLevel) => *b"kgsp-recv-2level",
        (StepRelation::Receive, StateLayout::Flat) => *b"kgsp-recv-flat-1",
    })
}

/// The identity of a wallet incarnation (spec section 2.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    /// The scheme identifier.
    pub scheme_id: [u8; 32],
    /// The asset (incarnation) identifier.
    pub asset: [u8; 32],
    /// The `wallet_id`.
    pub wallet_id: [u8; 32],
    /// The credential digest.
    pub credential: [u8; 32],
}

/// The five map roots of the core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapRoots<F> {
    /// The consumed-credit root.
    pub consumed_credit: F,
    /// The pending-outgoing root.
    pub pending_outgoing: F,
    /// The load/redeem recovery root.
    pub load_redeem_recovery: F,
    /// The fee-claim recovery root.
    pub fee_claim_recovery: F,
    /// The quota-usage root.
    pub quota_usage: F,
}

/// The regulatory-control fields of the core (spec section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controls<F> {
    /// The enabled-controls mask (zero: every control off).
    pub enabled: u64,
    /// The quota share/windows root.
    pub quota_windows_root: F,
    /// The blacklist version.
    pub blacklist_version: u64,
    /// The blacklist root.
    pub blacklist_root: F,
    /// The lease expiry.
    pub lease_expiry: u64,
}

/// The 30 core fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreState<F> {
    /// The lifecycle ([`LIFECYCLE_ACTIVE`] for a usable state).
    pub lifecycle: u64,
    /// The incarnation identity.
    pub identity: Identity,
    /// The balance.
    pub balance: u128,
    /// The burned total.
    pub burned_total: u128,
    /// The sequence number.
    pub sequence: u128,
    /// The next send ordinal.
    pub next_send: u128,
    /// The next load ordinal.
    pub next_load: u128,
    /// The next redeem ordinal.
    pub next_redeem: u128,
    /// The send chain accumulator.
    pub send_chain: F,
    /// The receive chain accumulator.
    pub recv_chain: F,
    /// The map roots.
    pub roots: MapRoots<F>,
    /// The regulatory controls.
    pub controls: Controls<F>,
    /// The policy epoch.
    pub policy_epoch: u64,
    /// The accepted-time floor.
    pub accepted_time_floor: u64,
    /// The state nonce.
    pub state_nonce: F,
}

/// The two 128-bit limbs of a 32-byte value as field elements.
#[must_use]
pub fn limbs<F: PoseidonField>(bytes: &[u8; 32]) -> [F; 2] {
    bytes_to_limbs(bytes).map(F::from_u128)
}

impl<F: PoseidonField> CoreState<F> {
    /// The field encoding, in [`core_index`] order.
    #[must_use]
    pub fn fields(&self) -> [F; CORE_FIELDS] {
        let [scheme_lo, scheme_hi] = limbs::<F>(&self.identity.scheme_id);
        let [asset_lo, asset_hi] = limbs::<F>(&self.identity.asset);
        let [wallet_lo, wallet_hi] = limbs::<F>(&self.identity.wallet_id);
        let [credential_lo, credential_hi] = limbs::<F>(&self.identity.credential);
        [
            F::from(self.lifecycle),
            scheme_lo,
            scheme_hi,
            asset_lo,
            asset_hi,
            wallet_lo,
            wallet_hi,
            credential_lo,
            credential_hi,
            F::from_u128(self.balance),
            F::from_u128(self.burned_total),
            F::from_u128(self.sequence),
            F::from_u128(self.next_send),
            F::from_u128(self.next_load),
            F::from_u128(self.next_redeem),
            self.send_chain,
            self.recv_chain,
            self.roots.consumed_credit,
            self.roots.pending_outgoing,
            self.roots.load_redeem_recovery,
            self.roots.fee_claim_recovery,
            self.roots.quota_usage,
            F::from(self.controls.enabled),
            self.controls.quota_windows_root,
            F::from(self.controls.blacklist_version),
            self.controls.blacklist_root,
            F::from(self.controls.lease_expiry),
            F::from(self.policy_epoch),
            F::from(self.accepted_time_floor),
            self.state_nonce,
        ]
    }
}

/// The 10 remainder fields (opaque to the step relations; the lineage
/// relation opens them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateRemainder<F> {
    /// The state version.
    pub version: u64,
    /// The regulatory policy body digest.
    pub regulatory_policy: [u8; 32],
    /// The other carried fields.
    pub other: [F; OTHER_CARRIED_FIELDS],
}

impl<F: PoseidonField> StateRemainder<F> {
    /// The field encoding: version, regulatory policy limbs, then the other
    /// carried fields.
    #[must_use]
    pub fn fields(&self) -> [F; REMAINDER_FIELDS] {
        let mut fields = [F::ZERO; REMAINDER_FIELDS];
        fields[0] = F::from(self.version);
        let [policy_lo, policy_hi] = limbs::<F>(&self.regulatory_policy);
        fields[1] = policy_lo;
        fields[2] = policy_hi;
        fields[3..].copy_from_slice(&self.other);
        fields
    }

    /// The two-level remainder digest `H(kgsprst1, fields)`.
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

/// The Request terms neither wallet state holds (spec section 5.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestTerms {
    /// The amount.
    pub amount: u128,
    /// The exact fee.
    pub fee: u128,
    /// The signed fee schedule digest.
    pub fee_schedule: [u8; 32],
    /// The Request policy epoch.
    pub policy_epoch: u64,
    /// The scheme policy digest.
    pub scheme_policy: [u8; 32],
    /// The Request (receiver's authenticated accepted) time.
    pub request_time: u64,
    /// The certificates digest.
    pub certificates: [u8; 32],
    /// The fresh Request nonce.
    pub nonce: [u8; 32],
}

/// The canonical 24-field Request body (spec section 5.1): the `credit_id`
/// preimage, as both wallets and every consumer hold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestBody {
    /// The scheme identifier.
    pub scheme_id: [u8; 32],
    /// The asset identifier.
    pub asset: [u8; 32],
    /// The payer wallet.
    pub payer_wallet: [u8; 32],
    /// The receiver wallet.
    pub receiver_wallet: [u8; 32],
    /// The payer send ordinal `s`.
    pub send_ordinal: u128,
    /// The receiver credential digest.
    pub receiver_credential: [u8; 32],
    /// The remaining terms.
    pub terms: RequestTerms,
}

impl RequestBody {
    /// The field encoding.
    #[must_use]
    pub fn fields<F: PoseidonField>(&self) -> [F; REQUEST_FIELDS] {
        let pair = |bytes: &[u8; 32]| limbs::<F>(bytes);
        let [scheme_lo, scheme_hi] = pair(&self.scheme_id);
        let [asset_lo, asset_hi] = pair(&self.asset);
        let [payer_lo, payer_hi] = pair(&self.payer_wallet);
        let [receiver_lo, receiver_hi] = pair(&self.receiver_wallet);
        let [credential_lo, credential_hi] = pair(&self.receiver_credential);
        let terms = &self.terms;
        let [schedule_lo, schedule_hi] = pair(&terms.fee_schedule);
        let [policy_lo, policy_hi] = pair(&terms.scheme_policy);
        let [certificates_lo, certificates_hi] = pair(&terms.certificates);
        let [nonce_lo, nonce_hi] = pair(&terms.nonce);
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
            F::from_u128(self.send_ordinal),
            credential_lo,
            credential_hi,
            F::from_u128(terms.amount),
            schedule_lo,
            schedule_hi,
            F::from_u128(terms.fee),
            F::from(terms.policy_epoch),
            policy_lo,
            policy_hi,
            F::from(terms.request_time),
            certificates_lo,
            certificates_hi,
            nonce_lo,
            nonce_hi,
        ]
    }

    /// `credit_id = H(kgspcrd1, Request body)` in `F`.
    #[must_use]
    pub fn credit_id<F: PoseidonField>(&self) -> F {
        hash_with_domain(CREDIT_DOMAIN, &self.fields::<F>())
    }

    /// The canonical limbs of [`Self::credit_id`] (the halves of its
    /// canonical 32-byte encoding).
    #[must_use]
    pub fn credit_limbs<F: PoseidonField>(&self) -> [u128; 2] {
        foreign_limbs(&self.credit_id::<F>())
    }
}

/// The values a `sigma_send` takes from the predecessor's lineage proof
/// Ω(pred) (spec section 3.2): public inputs a consumer compares with Ω's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineageInputs<F> {
    /// The lineage-adjusted `burned_total`.
    pub burned_total: u128,
    /// The lineage-adjusted pending-outgoing root.
    pub pending_outgoing_root: F,
}

/// The `sigma_send` inputs beyond the predecessor state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendInputs<F> {
    /// The receiver wallet.
    pub receiver_wallet: [u8; 32],
    /// The receiver credential digest.
    pub receiver_credential: [u8; 32],
    /// The other Request terms.
    pub request: RequestTerms,
    /// The accepted lower time.
    pub accepted_lower: u64,
    /// The accepted upper time.
    pub accepted_upper: u64,
    /// The inputs taken from the predecessor's lineage proof.
    pub lineage: LineageInputs<F>,
    /// The successor's pending-outgoing root (the lineage input root with
    /// this credit's descriptor inserted; checked natively at Advance and by
    /// the lineage relation).
    pub successor_pending_outgoing: F,
    /// The successor's fee-claim recovery root (checked natively at Advance
    /// and by the lineage relation).
    pub successor_fee_claim: F,
}

/// The `sigma_recv` inputs beyond the predecessor state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReceiveInputs<F> {
    /// The payer wallet.
    pub payer_wallet: [u8; 32],
    /// The payer send ordinal `s`.
    pub send_ordinal: u128,
    /// The other Request terms.
    pub request: RequestTerms,
    /// The successor's consumed-credit root (the predecessor's with
    /// `credit_id` inserted; checked natively at Advance and by the lineage
    /// relation).
    pub successor_consumed_credit: F,
}

/// The step-specific inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepInputs<F> {
    /// `sigma_send`.
    Send(Box<SendInputs<F>>),
    /// `sigma_recv`.
    Receive(Box<ReceiveInputs<F>>),
}

impl<F> StepInputs<F> {
    /// The step relation these inputs belong to.
    #[must_use]
    pub const fn relation(&self) -> StepRelation {
        match self {
            Self::Send(_) => StepRelation::Send,
            Self::Receive(_) => StepRelation::Receive,
        }
    }

    /// The Request terms.
    #[must_use]
    pub fn terms(&self) -> &RequestTerms {
        match self {
            Self::Send(send) => &send.request,
            Self::Receive(receive) => &receive.request,
        }
    }

    /// The amount.
    #[must_use]
    pub fn amount(&self) -> u128 {
        self.terms().amount
    }
}

/// A full step witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepWitness<F> {
    /// The predecessor state.
    pub predecessor: StateV1<F>,
    /// The successor's fresh state nonce.
    pub successor_nonce: F,
    /// The step inputs.
    pub inputs: StepInputs<F>,
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
    /// The payer and receiver wallets are equal.
    SelfPayment,
    /// `sigma_send`: the enabled-controls mask is not empty (this relation
    /// enforces no control).
    ControlsEnabled,
    /// `sigma_send`: `amount + fee` reaches `2^128`.
    DebitOverflow,
    /// `sigma_send`: `amount + fee` exceeds `balance - burned_total` (with
    /// the lineage input `burned_total`), or `burned_total` exceeds the
    /// balance.
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
    /// `sigma_send` only: the credit identifier (also bound, as canonical
    /// limbs, in the statement effect).
    pub credit_id: Option<F>,
}

impl<F: Copy> StepPublic<F> {
    /// The instance column: the statement digest, then the credit
    /// identifier.
    #[must_use]
    pub fn instance(&self) -> Vec<F> {
        let mut instance = vec![self.statement];
        instance.extend(self.credit_id);
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
    /// The credit identifier `H(kgspcrd1, Request body)`.
    pub credit: F,
    /// The appended chain accumulator (`send_chain'` or `recv_chain'`).
    pub chain: F,
    /// The statement digest.
    pub statement: F,
}

/// The reference evaluation of a step (field arithmetic, as in circuit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeStep<F> {
    /// The step relation.
    pub relation: StepRelation,
    /// The successor core fields.
    pub successor_core: [F; CORE_FIELDS],
    /// The successor state, for a witness without violations.
    pub successor_state: Option<StateV1<F>>,
    /// The Request body.
    pub request: RequestBody,
    /// The canonical limbs of the credit identifier.
    pub credit_limbs: [u128; 2],
    /// The chain entry that was hashed.
    pub chain_entry: Vec<F>,
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
    pub fn public(&self) -> StepPublic<F> {
        StepPublic {
            statement: self.digests.statement,
            credit_id: (self.relation == StepRelation::Send).then_some(self.digests.credit),
        }
    }

    /// Whether the witness satisfies the relation.
    #[must_use]
    pub fn is_honest(&self) -> bool {
        self.violations.is_empty()
    }
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
    let counterparty = match &witness.inputs {
        StepInputs::Send(send) => &send.receiver_wallet,
        StepInputs::Receive(receive) => &receive.payer_wallet,
    };
    if *counterparty == core.identity.wallet_id {
        found.push(Violation::SelfPayment);
    }
    match &witness.inputs {
        StepInputs::Send(send) => {
            if core.controls.enabled != 0 {
                found.push(Violation::ControlsEnabled);
            }
            let debit = send.request.amount.checked_add(send.request.fee);
            if debit.is_none() {
                found.push(Violation::DebitOverflow);
            }
            let affordable = core
                .balance
                .checked_sub(send.lineage.burned_total)
                .zip(debit)
                .is_some_and(|(spendable, debit)| debit <= spendable);
            if !affordable {
                found.push(Violation::Overdraft);
            }
            if core.next_send == u128::MAX {
                found.push(Violation::SendOrdinalOverflow);
            }
            if send.request.policy_epoch > core.policy_epoch {
                found.push(Violation::PolicyEpochNewer);
            }
            if send.accepted_lower < core.accepted_time_floor {
                found.push(Violation::AcceptedTimeBelowFloor);
            }
            if send.accepted_lower < send.request.request_time {
                found.push(Violation::AcceptedTimeBelowRequest);
            }
            if send.accepted_upper < send.accepted_lower {
                found.push(Violation::AcceptedWindowInverted);
            }
        }
        StepInputs::Receive(receive) => {
            if core.balance.checked_add(receive.request.amount).is_none() {
                found.push(Violation::BalanceOverflow);
            }
        }
    }
    found
}

impl<F: PoseidonField> StepWitness<F> {
    /// The step relation of this witness.
    #[must_use]
    pub const fn relation(&self) -> StepRelation {
        self.inputs.relation()
    }

    /// The Request body: the identity fields come from the predecessor
    /// state (the payer's for `sigma_send`, the receiver's for
    /// `sigma_recv`), the rest from the inputs.
    #[must_use]
    pub fn request_body(&self) -> RequestBody {
        let core = &self.predecessor.core;
        let identity = &core.identity;
        match &self.inputs {
            StepInputs::Send(send) => RequestBody {
                scheme_id: identity.scheme_id,
                asset: identity.asset,
                payer_wallet: identity.wallet_id,
                receiver_wallet: send.receiver_wallet,
                send_ordinal: core.next_send,
                receiver_credential: send.receiver_credential,
                terms: send.request,
            },
            StepInputs::Receive(receive) => RequestBody {
                scheme_id: identity.scheme_id,
                asset: identity.asset,
                payer_wallet: receive.payer_wallet,
                receiver_wallet: identity.wallet_id,
                send_ordinal: receive.send_ordinal,
                receiver_credential: identity.credential,
                terms: receive.request,
            },
        }
    }

    /// The typed successor state of an honest witness, given its appended
    /// chain value (`None` when the integer arithmetic fails).
    fn successor_state(&self, chain: F) -> Option<StateV1<F>> {
        let mut core = self.predecessor.core;
        core.sequence = core.sequence.checked_add(1)?;
        core.state_nonce = self.successor_nonce;
        match &self.inputs {
            StepInputs::Send(send) => {
                let debit = send.request.amount.checked_add(send.request.fee)?;
                core.balance = core.balance.checked_sub(debit)?;
                core.burned_total = send.lineage.burned_total;
                core.next_send = core.next_send.checked_add(1)?;
                core.send_chain = chain;
                core.roots.pending_outgoing = send.successor_pending_outgoing;
                core.roots.fee_claim_recovery = send.successor_fee_claim;
                core.accepted_time_floor = send.accepted_lower;
            }
            StepInputs::Receive(receive) => {
                core.balance = core.balance.checked_add(receive.request.amount)?;
                core.recv_chain = chain;
                core.roots.consumed_credit = receive.successor_consumed_credit;
            }
        }
        Some(StateV1 {
            core,
            remainder: self.predecessor.remainder,
        })
    }

    /// The public statement of an honest witness under `layout` (`None`
    /// when the witness breaks the relation).
    #[must_use]
    pub fn statement(&self, layout: StateLayout) -> Option<StatementV1<F>> {
        let native = self.evaluate(layout);
        let successor = native.successor_state?;
        if !native.violations.is_empty() {
            return None;
        }
        let core = &self.predecessor.core;
        let (burned_total, pending_outgoing_root) = match &self.inputs {
            StepInputs::Send(send) => (
                send.lineage.burned_total,
                send.lineage.pending_outgoing_root,
            ),
            StepInputs::Receive(_) => (0, F::ZERO),
        };
        let effect_len = match self.relation() {
            StepRelation::Send => SEND_EFFECT_FIELDS,
            StepRelation::Receive => RECEIVE_EFFECT_FIELDS,
        };
        Some(StatementV1 {
            relation_id: relation_id(self.relation(), layout),
            step: self.relation(),
            scheme_id: core.identity.scheme_id,
            asset: core.identity.asset,
            credential: core.identity.credential,
            lifecycle: successor.core.lifecycle,
            sequence: successor.core.sequence,
            predecessor: native.digests.predecessor,
            successor: native.digests.successor,
            enabled_controls: core.controls.enabled,
            burned_total,
            pending_outgoing_root,
            effect: native.statement[STATEMENT_FIELDS - EFFECT_UNION_FIELDS..][..effect_len]
                .to_vec(),
        })
    }

    /// The reference evaluation under `layout`.
    #[must_use]
    pub fn evaluate(&self, layout: StateLayout) -> NativeStep<F> {
        let state = &self.predecessor;
        let core = state.core.fields();
        let predecessor = state.commitment(layout);
        let request = self.request_body();
        let credit = request.credit_id::<F>();
        let credit_limbs = foreign_limbs(&credit);
        let [credit_lo, credit_hi] = credit_limbs.map(F::from_u128);
        let mut successor_core = core;
        successor_core[core_index::SEQUENCE] += F::ONE;
        successor_core[core_index::STATE_NONCE] = self.successor_nonce;
        let amount = F::from_u128(request.terms.amount);
        let (chain_entry, chain_domain, effect, lineage) = match &self.inputs {
            StepInputs::Send(send) => {
                let [receiver_lo, receiver_hi] = limbs::<F>(&send.receiver_wallet);
                let ordinal = core[core_index::NEXT_SEND];
                let fee = F::from_u128(send.request.fee);
                let entry = vec![
                    core[core_index::SEND_CHAIN],
                    credit_lo,
                    credit_hi,
                    receiver_lo,
                    receiver_hi,
                    ordinal,
                    amount,
                    fee,
                ];
                let effect = vec![
                    credit_lo,
                    credit_hi,
                    receiver_lo,
                    receiver_hi,
                    ordinal,
                    amount,
                    fee,
                    F::from(send.accepted_lower),
                    F::from(send.accepted_upper),
                ];
                successor_core[core_index::BALANCE] -= amount + fee;
                successor_core[core_index::BURNED_TOTAL] =
                    F::from_u128(send.lineage.burned_total);
                successor_core[core_index::NEXT_SEND] += F::ONE;
                successor_core[core_index::PENDING_OUTGOING_ROOT] =
                    send.successor_pending_outgoing;
                successor_core[core_index::FEE_CLAIM_ROOT] = send.successor_fee_claim;
                successor_core[core_index::TIME_FLOOR] = F::from(send.accepted_lower);
                let lineage = [
                    F::from_u128(send.lineage.burned_total),
                    send.lineage.pending_outgoing_root,
                ];
                (entry, SEND_CHAIN_DOMAIN, effect, lineage)
            }
            StepInputs::Receive(receive) => {
                let [payer_lo, payer_hi] = limbs::<F>(&receive.payer_wallet);
                let entry = vec![
                    core[core_index::RECEIVE_CHAIN],
                    credit_lo,
                    credit_hi,
                    payer_lo,
                    payer_hi,
                    amount,
                ];
                let effect = vec![credit_lo, credit_hi, payer_lo, payer_hi, amount];
                successor_core[core_index::BALANCE] += amount;
                successor_core[core_index::CONSUMED_CREDIT_ROOT] =
                    receive.successor_consumed_credit;
                (entry, RECEIVE_CHAIN_DOMAIN, effect, [F::ZERO; 2])
            }
        };
        let chain = hash_with_domain(chain_domain, &chain_entry);
        let relation = self.relation();
        match relation {
            StepRelation::Send => successor_core[core_index::SEND_CHAIN] = chain,
            StepRelation::Receive => successor_core[core_index::RECEIVE_CHAIN] = chain,
        }
        let successor_preimage = commitment_preimage(layout, &successor_core, &state.remainder);
        let successor = hash_with_domain(layout.domain(), &successor_preimage);
        let mut statement = [F::ZERO; STATEMENT_FIELDS];
        let pick = |index: usize| core[index];
        let header = [
            F::from(STATEMENT_VERSION),
            F::from_u128(relation_id(relation, layout)),
            pick(core_index::SCHEME),
            pick(core_index::SCHEME + 1),
            pick(core_index::ASSET),
            pick(core_index::ASSET + 1),
            pick(core_index::CREDENTIAL),
            pick(core_index::CREDENTIAL + 1),
            successor_core[core_index::LIFECYCLE],
            successor_core[core_index::SEQUENCE],
            predecessor,
            successor,
            pick(core_index::ENABLED_CONTROLS),
            lineage[0],
            lineage[1],
            F::from(relation.effect_tag()),
        ];
        statement[..header.len()].copy_from_slice(&header);
        statement[header.len()..header.len() + effect.len()].copy_from_slice(&effect);
        let statement_digest = hash_with_domain(STATEMENT_DOMAIN, &statement);
        let violations = violations(self);
        let successor_state = if violations.is_empty() {
            self.successor_state(chain)
        } else {
            None
        };
        NativeStep {
            relation,
            successor_core,
            successor_state,
            request,
            credit_limbs,
            chain_entry,
            statement,
            digests: StepDigests {
                predecessor,
                successor,
                credit,
                chain,
                statement: statement_digest,
            },
            violations,
        }
    }
}

#[cfg(test)]
mod tests {
    use ff::{Field, PrimeField};
    use iroha_pasta::{Fp, Fq};

    use super::*;
    use crate::vectors::{Mutation, sample_witness};

    #[test]
    fn domains_and_relation_ids_are_distinct_prototype_labels() {
        assert_eq!(CORE_DOMAIN.to_le_bytes(), *b"kgspcor1");
        assert_eq!(REMAINDER_DOMAIN.to_le_bytes(), *b"kgsprst1");
        assert_eq!(FLAT_STATE_DOMAIN.to_le_bytes(), *b"kgspflt1");
        assert_eq!(CREDIT_DOMAIN.to_le_bytes(), *b"kgspcrd1");
        assert_eq!(SEND_CHAIN_DOMAIN.to_le_bytes(), *b"kgspsnd1");
        assert_eq!(RECEIVE_CHAIN_DOMAIN.to_le_bytes(), *b"kgsprcv1");
        assert_eq!(StateLayout::TwoLevel.commitment_arity(), 31);
        assert_eq!(StateLayout::Flat.commitment_arity(), 40);
        assert_eq!(StateLayout::Flat.domain(), FLAT_STATE_DOMAIN);
        assert_eq!(StateLayout::TwoLevel.label(), "two_level");
        assert_eq!(public_outputs(StepRelation::Send), 2);
        assert_eq!(public_outputs(StepRelation::Receive), 1);
        let mut ids = Vec::new();
        for step in [StepRelation::Send, StepRelation::Receive] {
            for layout in [StateLayout::TwoLevel, StateLayout::Flat] {
                ids.push(relation_id(step, layout));
            }
        }
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4, "one relation id per (step, layout)");
    }

    #[test]
    fn field_encodings_follow_the_core_layout() {
        let witness = sample_witness::<Fp>(3, StepRelation::Send, Mutation::None);
        let state = witness.predecessor;
        let core = state.core.fields();
        assert_eq!(core[core_index::BALANCE], Fp::from_u128(state.core.balance));
        assert_eq!(core[core_index::LIFECYCLE], Fp::ONE);
        assert_eq!(
            [core[core_index::WALLET], core[core_index::WALLET + 1]],
            limbs::<Fp>(&state.core.identity.wallet_id)
        );
        assert_eq!(
            [core[core_index::CREDENTIAL], core[core_index::CREDENTIAL + 1]],
            limbs::<Fp>(&state.core.identity.credential)
        );
        assert_eq!(
            core[core_index::BURNED_TOTAL],
            Fp::from_u128(state.core.burned_total)
        );
        assert_eq!(
            core[core_index::PENDING_OUTGOING_ROOT],
            state.core.roots.pending_outgoing
        );
        assert_eq!(
            core[core_index::ENABLED_CONTROLS],
            Fp::from(state.core.controls.enabled)
        );
        assert_eq!(
            core[core_index::TIME_FLOOR],
            Fp::from(state.core.accepted_time_floor)
        );
        assert_eq!(core[core_index::STATE_NONCE], state.core.state_nonce);
        let remainder = state.remainder.fields();
        assert_eq!(remainder[0], Fp::from(STATE_VERSION));
        assert_eq!(
            [remainder[1], remainder[2]],
            limbs::<Fp>(&state.remainder.regulatory_policy)
        );
        assert_eq!(remainder[3..], state.remainder.other);
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

    #[test]
    fn request_bodies_take_identity_from_the_state() {
        let send = sample_witness::<Fp>(4, StepRelation::Send, Mutation::None);
        let body = send.request_body();
        let core = &send.predecessor.core;
        assert_eq!(body.payer_wallet, core.identity.wallet_id);
        assert_eq!(body.scheme_id, core.identity.scheme_id);
        assert_eq!(body.asset, core.identity.asset);
        assert_eq!(body.send_ordinal, core.next_send);
        let fields = body.fields::<Fp>();
        assert_eq!(fields[0], Fp::from(REQUEST_VERSION));
        assert_eq!(fields[9], Fp::from_u128(core.next_send));
        assert_eq!(fields[12], Fp::from_u128(body.terms.amount));
        assert_eq!(
            body.credit_id::<Fp>(),
            hash_with_domain(CREDIT_DOMAIN, &fields)
        );
        assert_eq!(
            body.credit_limbs::<Fp>(),
            bytes_to_limbs(&body.credit_id::<Fp>().to_repr())
        );
        let receive = sample_witness::<Fq>(4, StepRelation::Receive, Mutation::None);
        let body = receive.request_body();
        let core = &receive.predecessor.core;
        assert_eq!(body.receiver_wallet, core.identity.wallet_id);
        assert_eq!(body.receiver_credential, core.identity.credential);
        assert_eq!(receive.inputs.terms(), &body.terms);
    }

    /// The statement encoding equals the gadgets' `StatementV1` for honest
    /// witnesses.
    fn statement_matches_gadgets<F: PoseidonField>(relation: StepRelation) {
        for layout in [StateLayout::TwoLevel, StateLayout::Flat] {
            let witness = sample_witness::<F>(11, relation, Mutation::None);
            let native = witness.evaluate(layout);
            assert!(native.is_honest(), "{:?}", native.violations);
            let statement = witness.statement(layout).expect("honest statement");
            assert_eq!(statement.encode(), Some(native.statement));
            assert_eq!(statement.digest(), Some(native.digests.statement));
            assert_eq!(statement.relation_id, relation_id(relation, layout));
            assert_eq!(native.public().statement, native.digests.statement);
            assert_eq!(
                native.public().credit_id,
                (relation == StepRelation::Send).then_some(native.digests.credit)
            );
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
        let successor = native.successor_state.expect("honest successor");
        assert_eq!(successor.core.fields(), native.successor_core);
        assert_eq!(
            successor.core.balance,
            core.balance - send.request.amount - send.request.fee
        );
        assert_eq!(successor.core.burned_total, send.lineage.burned_total);
        assert_eq!(successor.core.next_send, core.next_send + 1);
        assert_eq!(successor.core.accepted_time_floor, send.accepted_lower);
        assert_eq!(
            successor.core.roots.pending_outgoing,
            send.successor_pending_outgoing
        );
        assert_eq!(
            successor.core.roots.consumed_credit,
            core.roots.consumed_credit
        );
        assert_eq!(
            successor.core.send_chain,
            hash_with_domain(SEND_CHAIN_DOMAIN, &native.chain_entry)
        );
        assert_eq!(native.chain_entry.len(), SEND_CHAIN_FIELDS);
        assert_eq!(native.chain_entry[1..3], native.statement[16..18]);
        assert_eq!(
            native.digests.credit,
            hash_with_domain(CREDIT_DOMAIN, &native.request.fields::<Fp>())
        );
        assert_eq!(
            successor.commitment(StateLayout::TwoLevel),
            native.digests.successor
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
        let successor = native.successor_state.expect("honest successor");
        assert_eq!(successor.core.fields(), native.successor_core);
        assert_eq!(
            successor.core.balance,
            core.balance + receive.request.amount
        );
        assert_eq!(
            successor.core.roots.consumed_credit,
            receive.successor_consumed_credit
        );
        assert_eq!(successor.core.burned_total, core.burned_total);
        assert_eq!(
            successor.core.accepted_time_floor,
            core.accepted_time_floor
        );
        assert_eq!(native.chain_entry.len(), RECEIVE_CHAIN_FIELDS);
        assert_eq!(native.public().instance(), vec![native.digests.statement]);
        // The lineage inputs of a Receive statement are zero.
        assert_eq!(native.statement[13], Fq::ZERO);
        assert_eq!(native.statement[14], Fq::ZERO);
        assert_eq!(witness.relation(), StepRelation::Receive);
        assert_eq!(
            successor.commitment(StateLayout::Flat),
            native.digests.successor
        );
    }

    #[test]
    fn mutations_are_reported_as_violations() {
        let cases = [
            (
                StepRelation::Send,
                Mutation::Overdraft,
                Violation::Overdraft,
            ),
            (StepRelation::Send, Mutation::Burned, Violation::Overdraft),
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
                StepRelation::Send,
                Mutation::SelfPayment,
                Violation::SelfPayment,
            ),
            (
                StepRelation::Send,
                Mutation::ControlsEnabled,
                Violation::ControlsEnabled,
            ),
            (
                StepRelation::Receive,
                Mutation::Overflow,
                Violation::BalanceOverflow,
            ),
            (
                StepRelation::Receive,
                Mutation::SelfPayment,
                Violation::SelfPayment,
            ),
        ];
        for (relation, mutation, violation) in cases {
            let native =
                sample_witness::<Fp>(9, relation, mutation).evaluate(StateLayout::TwoLevel);
            assert_eq!(native.violations, vec![violation], "{mutation:?}");
            assert!(native.successor_state.is_none());
        }
        let mut witness = sample_witness::<Fp>(9, StepRelation::Send, Mutation::None);
        witness.predecessor.core.lifecycle = 2;
        witness.predecessor.core.sequence = u128::MAX;
        witness.predecessor.core.next_send = u128::MAX;
        if let StepInputs::Send(send) = &mut witness.inputs {
            send.request.amount = 0;
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
        assert_eq!(witness.statement(StateLayout::Flat), None);
        // The overflowing successor sequence is the field value 2^128.
        let native = witness.evaluate(StateLayout::Flat);
        assert_eq!(
            native.successor_core[core_index::SEQUENCE],
            Fp::from_u128(u128::MAX) + Fp::ONE
        );
        // A debit of 2^128 overflows and overdraws.
        let mut witness = sample_witness::<Fp>(9, StepRelation::Send, Mutation::None);
        if let StepInputs::Send(send) = &mut witness.inputs {
            send.request.amount = u128::MAX;
            send.request.fee = 1;
        }
        assert_eq!(
            witness.evaluate(StateLayout::TwoLevel).violations,
            vec![Violation::DebitOverflow, Violation::Overdraft]
        );
    }
}
