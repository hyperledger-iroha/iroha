//! **Prototype** native consumer checks of a step statement (spec section
//! 3.2): what a package consumer compares before mutation, and the public
//! outputs it then verifies the step proof against.
//!
//! A package carries its statement in canonical form ([`StatementV1`]); the
//! step proof's public outputs are the statement digest and, for
//! `sigma_send`, the credit identifier. A consumer never takes a field from
//! the statement on trust:
//!
//! - the relation identity must be the one its allowlist selects for the
//!   step and the state layout ([`crate::witness::relation_id`]);
//! - for `sigma_send`, the predecessor, credential digest, scheme,
//!   enabled-controls mask, `burned_total` and pending-outgoing root must
//!   equal the predecessor's lineage proof Ω(pred) ([`LineageView`]); the
//!   mask must be empty, since this relation enforces no control;
//! - the scheme, asset, wallets, send ordinal, amount and fee must equal the
//!   Request body the consumer holds, and the credit identifier is
//!   recomputed from that body, never read from the statement.
//!
//! Only then does it verify the proof against
//! `(statement.digest(), credit_id)`. A statement that claims another
//! credential, wallet, credit or `burned_total` than Ω(pred) and the
//! Request therefore fails here. A statement that passes here but is not
//! what the prover's predecessor opened (for example another asset, which
//! Ω does not expose) fails verification.

use iroha_pasta::poseidon::PoseidonField;
use iroha_plonk_gadgets::statement::{StatementV1, StepRelation};

use crate::witness::{
    LIFECYCLE_ACTIVE, RECEIVE_EFFECT_FIELDS, RequestBody, SEND_EFFECT_FIELDS, StateLayout,
    StepPublic, limbs, relation_id,
};

/// The public outputs of the predecessor's lineage proof Ω(pred) that a
/// `sigma_send` consumer compares (spec section 3.2; prototype view).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineageView<F> {
    /// The head commitment (the predecessor state commitment, in the step
    /// proof's field; Ω exposes it as canonical limbs).
    pub head: F,
    /// The `wallet_id`.
    pub wallet_id: [u8; 32],
    /// The credential digest.
    pub credential: [u8; 32],
    /// The scheme identifier.
    pub scheme_id: [u8; 32],
    /// The enabled-controls mask.
    pub enabled_controls: u64,
    /// The lineage-adjusted `burned_total`.
    pub burned_total: u128,
    /// The lineage-adjusted pending-outgoing root.
    pub pending_outgoing_root: F,
}

/// Why a consumer rejects a statement before verifying its proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConsumerError {
    /// The relation identity or step is not the allowlisted one.
    Relation,
    /// The statement's lifecycle is not Active.
    Lifecycle,
    /// The predecessor is not Ω(pred)'s head.
    Predecessor,
    /// The credential digest differs from Ω(pred)'s (Send) or the Request's
    /// receiver credential (Receive).
    Credential,
    /// The scheme differs from Ω(pred)'s or the Request's.
    Scheme,
    /// The asset differs from the Request's.
    Asset,
    /// The enabled-controls mask differs from Ω(pred)'s or is not empty.
    Controls,
    /// The `burned_total` input differs from Ω(pred)'s.
    BurnedTotal,
    /// The pending-outgoing input differs from Ω(pred)'s.
    PendingOutgoing,
    /// The Request's payer is not Ω(pred)'s wallet.
    Payer,
    /// The Request's payer and receiver wallets are equal.
    SelfPayment,
    /// The effect does not match the Request body.
    Effect,
}

impl core::fmt::Display for ConsumerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let what = match self {
            Self::Relation => "relation identity",
            Self::Lifecycle => "lifecycle",
            Self::Predecessor => "predecessor head",
            Self::Credential => "credential digest",
            Self::Scheme => "scheme",
            Self::Asset => "asset",
            Self::Controls => "enabled-controls mask",
            Self::BurnedTotal => "burned_total input",
            Self::PendingOutgoing => "pending-outgoing input",
            Self::Payer => "payer wallet",
            Self::SelfPayment => "payer equals receiver",
            Self::Effect => "effect",
        };
        write!(f, "statement rejected: {what}")
    }
}

impl std::error::Error for ConsumerError {}

/// Fails with `error` unless `ok`.
const fn require(ok: bool, error: ConsumerError) -> Result<(), ConsumerError> {
    if ok { Ok(()) } else { Err(error) }
}

/// The effect prefix a Request body determines: the credit identifier's
/// canonical limbs, the counterparty limbs, then the step's amounts.
fn expected_effect<F: PoseidonField>(step: StepRelation, request: &RequestBody) -> Vec<F> {
    let [credit_lo, credit_hi] = request.credit_limbs::<F>().map(F::from_u128);
    let amount = F::from_u128(request.terms.amount);
    match step {
        StepRelation::Send => {
            let [receiver_lo, receiver_hi] = limbs::<F>(&request.receiver_wallet);
            vec![
                credit_lo,
                credit_hi,
                receiver_lo,
                receiver_hi,
                F::from_u128(request.send_ordinal),
                amount,
                F::from_u128(request.terms.fee),
            ]
        }
        StepRelation::Receive => {
            let [payer_lo, payer_hi] = limbs::<F>(&request.payer_wallet);
            vec![credit_lo, credit_hi, payer_lo, payer_hi, amount]
        }
    }
}

/// The checks both steps share: relation identity, lifecycle, scheme and
/// asset against the Request, distinct wallets and the effect.
fn check_common<F: PoseidonField>(
    step: StepRelation,
    layout: StateLayout,
    request: &RequestBody,
    statement: &StatementV1<F>,
) -> Result<(), ConsumerError> {
    require(
        statement.step == step && statement.relation_id == relation_id(step, layout),
        ConsumerError::Relation,
    )?;
    require(
        statement.lifecycle == LIFECYCLE_ACTIVE,
        ConsumerError::Lifecycle,
    )?;
    require(
        statement.scheme_id == request.scheme_id,
        ConsumerError::Scheme,
    )?;
    require(statement.asset == request.asset, ConsumerError::Asset)?;
    require(
        request.payer_wallet != request.receiver_wallet,
        ConsumerError::SelfPayment,
    )?;
    let effect_len = match step {
        StepRelation::Send => SEND_EFFECT_FIELDS,
        StepRelation::Receive => RECEIVE_EFFECT_FIELDS,
    };
    let expected = expected_effect::<F>(step, request);
    require(
        statement.effect.len() == effect_len && statement.effect.starts_with(&expected),
        ConsumerError::Effect,
    )
}

/// The checks of a `sigma_send` statement against Ω(pred) (`view`) and the
/// Request body the consumer holds (its own, for a receiver). Returns the
/// public outputs to verify the proof against.
///
/// # Errors
///
/// The first [`ConsumerError`] found.
pub fn check_send<F: PoseidonField>(
    view: &LineageView<F>,
    layout: StateLayout,
    request: &RequestBody,
    statement: &StatementV1<F>,
) -> Result<StepPublic<F>, ConsumerError> {
    check_common(StepRelation::Send, layout, request, statement)?;
    require(
        statement.predecessor == view.head,
        ConsumerError::Predecessor,
    )?;
    require(
        statement.credential == view.credential,
        ConsumerError::Credential,
    )?;
    require(statement.scheme_id == view.scheme_id, ConsumerError::Scheme)?;
    require(
        statement.enabled_controls == view.enabled_controls && statement.enabled_controls == 0,
        ConsumerError::Controls,
    )?;
    require(
        statement.burned_total == view.burned_total,
        ConsumerError::BurnedTotal,
    )?;
    require(
        statement.pending_outgoing_root == view.pending_outgoing_root,
        ConsumerError::PendingOutgoing,
    )?;
    require(request.payer_wallet == view.wallet_id, ConsumerError::Payer)?;
    let digest = statement.digest().ok_or(ConsumerError::Effect)?;
    Ok(StepPublic {
        statement: digest,
        credit_id: Some(request.credit_id::<F>()),
    })
}

/// The checks of a `sigma_recv` statement (for example Credited evidence)
/// against the Request body the consumer holds: its credential digest must
/// be the Request's receiver credential. Returns the public outputs to
/// verify the proof against.
///
/// # Errors
///
/// The first [`ConsumerError`] found.
pub fn check_receive<F: PoseidonField>(
    layout: StateLayout,
    request: &RequestBody,
    statement: &StatementV1<F>,
) -> Result<StepPublic<F>, ConsumerError> {
    check_common(StepRelation::Receive, layout, request, statement)?;
    require(
        statement.credential == request.receiver_credential,
        ConsumerError::Credential,
    )?;
    require(
        statement.burned_total == 0 && statement.pending_outgoing_root == F::ZERO,
        ConsumerError::BurnedTotal,
    )?;
    let digest = statement.digest().ok_or(ConsumerError::Effect)?;
    Ok(StepPublic {
        statement: digest,
        credit_id: None,
    })
}

#[cfg(test)]
mod tests {
    use iroha_pasta::Fp;

    use super::*;
    use crate::{
        vectors::{Mutation, sample_witness},
        witness::{StepInputs, StepWitness},
    };

    /// The Ω(pred) view of an honest send witness.
    fn view_of(witness: &StepWitness<Fp>, layout: StateLayout) -> LineageView<Fp> {
        let StepInputs::Send(send) = &witness.inputs else {
            panic!("send witness");
        };
        let core = &witness.predecessor.core;
        LineageView {
            head: witness.predecessor.commitment(layout),
            wallet_id: core.identity.wallet_id,
            credential: core.identity.credential,
            scheme_id: core.identity.scheme_id,
            enabled_controls: core.controls.enabled,
            burned_total: send.lineage.burned_total,
            pending_outgoing_root: send.lineage.pending_outgoing_root,
        }
    }

    #[test]
    fn honest_statements_pass_and_yield_the_proof_outputs() {
        let layout = StateLayout::TwoLevel;
        let send = sample_witness::<Fp>(2, StepRelation::Send, Mutation::None);
        let statement = send.statement(layout).expect("statement");
        let public = check_send(
            &view_of(&send, layout),
            layout,
            &send.request_body(),
            &statement,
        )
        .expect("accepted");
        assert_eq!(public, send.evaluate(layout).public());
        let receive = sample_witness::<Fp>(2, StepRelation::Receive, Mutation::None);
        let statement = receive.statement(layout).expect("statement");
        let public = check_receive(layout, &receive.request_body(), &statement).expect("accepted");
        assert_eq!(public, receive.evaluate(layout).public());
    }

    /// A statement edit of [`every_bound_field_is_compared`].
    type Edit = fn(&mut StatementV1<Fp>);

    #[test]
    fn every_bound_field_is_compared() {
        let layout = StateLayout::TwoLevel;
        let send = sample_witness::<Fp>(3, StepRelation::Send, Mutation::None);
        let view = view_of(&send, layout);
        let request = send.request_body();
        let honest = send.statement(layout).expect("statement");
        let edits: [(Edit, ConsumerError); 10] = [
            (|s| s.relation_id[0] ^= 1, ConsumerError::Relation),
            (|s| s.lifecycle = 2, ConsumerError::Lifecycle),
            (
                |s| s.predecessor += Fp::from(1u64),
                ConsumerError::Predecessor,
            ),
            (|s| s.credential[0] ^= 1, ConsumerError::Credential),
            (|s| s.scheme_id[0] ^= 1, ConsumerError::Scheme),
            (|s| s.asset[0] ^= 1, ConsumerError::Asset),
            (|s| s.enabled_controls = 1, ConsumerError::Controls),
            (|s| s.burned_total = 0, ConsumerError::BurnedTotal),
            (
                |s| s.pending_outgoing_root += Fp::from(1u64),
                ConsumerError::PendingOutgoing,
            ),
            (|s| s.effect[0] += Fp::from(1u64), ConsumerError::Effect),
        ];
        for (edit, error) in edits {
            let mut statement = honest.clone();
            edit(&mut statement);
            assert_eq!(
                check_send(&view, layout, &request, &statement),
                Err(error),
                "{error}"
            );
        }
        // The flat relation's statement is not the two-level relation's.
        let flat = send.statement(StateLayout::Flat).expect("statement");
        assert_eq!(
            check_send(&view, layout, &request, &flat),
            Err(ConsumerError::Relation)
        );
        // A Request whose payer is another wallet, or the receiver itself.
        let mut other = request;
        other.payer_wallet[0] ^= 1;
        assert!(check_send(&view, layout, &other, &honest).is_err());
        let mut own = request;
        own.receiver_wallet = own.payer_wallet;
        assert!(check_send(&view, layout, &own, &honest).is_err());
        assert!(!ConsumerError::Payer.to_string().is_empty());
    }
}
