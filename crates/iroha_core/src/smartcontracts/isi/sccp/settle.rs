//! Settlement: inbound release and bounce, outbound refunds and `SettleSccpV1` retries
//! (`specs/sccp.md` §4.12.3, §4.12.5, §4.16). Owner: ws41.
//!
//! A proven inbound message settles when SCCP is enabled and its revision is `Bidirectional`
//! or `InboundOnly`; otherwise it stays `Pending` with a reason. A creditable recipient is
//! released `amount − fee_due` (the fee goes to the Nexus fee sink); an uncreditable one
//! bounces the value back to the source-chain sender through a new outbound message. A voided
//! outbound record is refunded to its sender, or moved to `stranded(route)` when the sender is
//! the escrow (a bounce) or cannot be credited. Every path keeps
//! `balance(escrow) = Σ liability + stranded`.

use super::{
    Error, escrow,
    outbound::{OutboundRecordArgsV1, record_outbound_message},
    recipients::{self, SccpRecipientClassV1},
    store,
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId,
    bridge::SccpNetworkV1,
    isi::sccp::{SccpSettleTargetV1, SettleSccpV1},
    sccp::{
        events::{
            SccpBounceReasonV1, SccpEvent, SccpInboundBouncedV1, SccpInboundLiabilityShortfallV1,
            SccpInboundReleasedV1, SccpOutboundRefundedV1, SccpOutboundStrandedV1,
        },
        inbound::{
            SccpBounceStatusV1, SccpInboundRecordV1, SccpInboundStatusV1, SccpPendingReasonV1,
        },
        outbound::{SccpOutboundStatusV1, SccpStatusHeightV1},
        registry::SccpRouteActivationV1,
    },
};
use iroha_sccp::v1::payload::SccpTransferPayloadV1;

fn refuse(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("SCCP settlement: {reason}").into())
}

/// Add `(inbound, refund)` to the pending counts of `(network, revision)`.
pub(super) fn adjust_pending(
    state_transaction: &mut StateTransaction<'_, '_>,
    key: (SccpNetworkV1, u32),
    inbound: i64,
    refund: i64,
) -> Result<(), Error> {
    let (current_inbound, current_refund) = store::pending_count(&*state_transaction.world, &key);
    let add = |value: u64, delta: i64| {
        value
            .checked_add_signed(delta)
            .ok_or_else(|| refuse("pending count underflows"))
    };
    let counts = (add(current_inbound, inbound)?, add(current_refund, refund)?);
    store::set_pending_counts(state_transaction, key, counts);
    Ok(())
}

/// Add `delta` to `liability(revision)` of `network`.
pub(super) fn adjust_liability(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
    revision: u32,
    delta: i128,
) -> Result<(), Error> {
    let mut route = store::routes::get(&*state_transaction.world, &network)
        .cloned()
        .ok_or_else(|| refuse("the route is unknown"))?;
    let record = route
        .revisions
        .get_mut(&revision)
        .ok_or_else(|| refuse(format_args!("revision {revision} is unknown")))?;
    record.liability = record
        .liability
        .checked_add_signed(delta)
        .ok_or_else(|| refuse("liability out of range"))?;
    store::routes::insert(state_transaction, network, route)?;
    Ok(())
}

fn activation(
    world: &(impl WorldReadOnly + ?Sized),
    network: SccpNetworkV1,
    revision: u32,
) -> Option<(SccpRouteActivationV1, u128)> {
    store::routes::get(world, &network)
        .and_then(|route| route.revisions.get(&revision))
        .map(|record| (record.activation, record.liability))
}

fn set_inbound_status(
    state_transaction: &mut StateTransaction<'_, '_>,
    message_id: [u8; 32],
    mut record: SccpInboundRecordV1,
    status: SccpInboundStatusV1,
) -> Result<(), Error> {
    record.status = status;
    store::inbound_messages::insert(state_transaction, message_id, record)?;
    Ok(())
}

/// Resolve the Nexus fee sink account under the executing block.
fn fee_sink(state_transaction: &StateTransaction<'_, '_>) -> Result<AccountId, Error> {
    crate::block::parse_account_literal_with_world(
        &*state_transaction.world,
        &state_transaction.nexus.dataspace_catalog,
        &state_transaction.nexus.fees.fee_sink_account_id,
        state_transaction.block_unix_timestamp_ms(),
    )
    .ok()
    .flatten()
    .ok_or_else(|| refuse("the Nexus fee sink account does not resolve"))
}

/// Attempt to settle the `Pending` inbound message `message_id` (§4.12.3, §4.12.5) and return
/// whether its record changed.
///
/// # Errors
///
/// Fails when the record is missing or not `Pending`, or a settlement effect fails.
pub fn settle_inbound(
    state_transaction: &mut StateTransaction<'_, '_>,
    message_id: [u8; 32],
) -> Result<bool, Error> {
    let world = &*state_transaction.world;
    let record = store::inbound_messages::get(world, &message_id)
        .cloned()
        .ok_or_else(|| refuse("unknown inbound message"))?;
    let SccpInboundStatusV1::Pending(pending) = record.status else {
        return Err(refuse("the inbound message is not pending"));
    };
    let (network, revision) = (record.network, record.revision);
    let enabled = store::parameters::get(world)
        .as_ref()
        .is_some_and(|params| params.enabled);
    let (activation, liability) =
        activation(world, network, revision).ok_or_else(|| refuse("the revision is unknown"))?;
    let hold = if !enabled {
        Some(SccpPendingReasonV1::Disabled)
    } else if !activation.settles() {
        Some(SccpPendingReasonV1::RevisionNotSettleable)
    } else {
        None
    };
    if let Some(reason) = hold {
        let changed = pending.reason != reason;
        if changed {
            set_inbound_status(
                state_transaction,
                message_id,
                record,
                SccpInboundStatusV1::pending(reason),
            )?;
        }
        return Ok(changed);
    }
    let payload = SccpTransferPayloadV1::decode(&record.payload)
        .map_err(|error| refuse(format_args!("stored payload: {error}")))?;
    let amount = payload.amount;
    if liability < amount {
        state_transaction
            .world
            .emit_events(Some(SccpEvent::InboundLiabilityShortfall(
                SccpInboundLiabilityShortfallV1 {
                    message_id,
                    network,
                    revision,
                    amount,
                    liability,
                },
            )));
        let changed = pending.reason != SccpPendingReasonV1::LiabilityShortfall;
        set_inbound_status(
            state_transaction,
            message_id,
            record,
            SccpInboundStatusV1::pending(SccpPendingReasonV1::LiabilityShortfall),
        )?;
        return Ok(changed);
    }
    let height = state_transaction._curr_block.height().get();
    match recipients::classify_recipient(state_transaction, &payload.recipient.bytes, amount) {
        SccpRecipientClassV1::Creditable {
            account,
            registered,
        } => {
            if !registered {
                recipients::register_recipient(
                    state_transaction,
                    &account,
                    network,
                    Some(message_id),
                )?;
            }
            let fee = record.fee_due.min(amount);
            escrow::release(
                state_transaction,
                network,
                &account,
                amount - fee,
                message_id,
            )?;
            if fee > 0 {
                let sink = fee_sink(state_transaction)?;
                escrow::release(state_transaction, network, &sink, fee, message_id)?;
            }
            adjust_liability(state_transaction, network, revision, -to_i128(amount)?)?;
            adjust_pending(state_transaction, (network, revision), -1, 0)?;
            set_inbound_status(
                state_transaction,
                message_id,
                record,
                SccpInboundStatusV1::Released(SccpStatusHeightV1 { height }),
            )?;
            state_transaction
                .world
                .emit_events(Some(SccpEvent::InboundReleased(SccpInboundReleasedV1 {
                    message_id,
                    network,
                    revision,
                    recipient: account,
                    amount,
                    fee,
                })));
        }
        class @ (SccpRecipientClassV1::Uncreditable { .. } | SccpRecipientClassV1::Undecodable) => {
            let reason = match class {
                SccpRecipientClassV1::Uncreditable { reason, .. } => reason,
                _ => SccpBounceReasonV1::UndecodableRecipient,
            };
            let bounce_revision =
                bounce_target(&*state_transaction.world, network, revision, amount);
            let escrow_account = store::routes::get(&*state_transaction.world, &network)
                .map(|route| route.escrow.clone())
                .ok_or_else(|| refuse("the route is unknown"))?;
            adjust_liability(state_transaction, network, revision, -to_i128(amount)?)?;
            adjust_liability(
                state_transaction,
                network,
                bounce_revision,
                to_i128(amount)?,
            )?;
            let bounce_message_id = record_outbound_message(
                state_transaction,
                OutboundRecordArgsV1 {
                    network,
                    revision: bounce_revision,
                    amount,
                    sender: escrow_account,
                    recipient: payload.sender.bytes.clone(),
                },
            )?;
            adjust_pending(state_transaction, (network, revision), -1, 0)?;
            set_inbound_status(
                state_transaction,
                message_id,
                record,
                SccpInboundStatusV1::Bounced(SccpBounceStatusV1 { bounce_message_id }),
            )?;
            state_transaction
                .world
                .emit_events(Some(SccpEvent::InboundBounced(SccpInboundBouncedV1 {
                    message_id,
                    network,
                    revision,
                    bounce_message_id,
                    bounce_revision,
                    amount,
                    reason,
                })));
        }
    }
    Ok(true)
}

fn to_i128(amount: u128) -> Result<i128, Error> {
    i128::try_from(amount).map_err(|_| refuse("amount out of range"))
}

/// The bounce target revision: the route's `Bidirectional` revision when its cap has room,
/// else the inbound revision itself (§4.12.5).
fn bounce_target(
    world: &(impl WorldReadOnly + ?Sized),
    network: SccpNetworkV1,
    revision: u32,
    amount: u128,
) -> u32 {
    store::routes::get(world, &network)
        .and_then(|route| route.bidirectional_revision())
        .filter(|live| {
            live.liability
                .checked_add(amount)
                .is_some_and(|liability| liability <= live.max_wrapped_supply)
        })
        .map_or(revision, |live| live.revision)
}

/// Attempt the refund of the voided outbound message `message_id` (§4.16) and return whether
/// its record changed.
///
/// A refund proceeds when SCCP is enabled and the revision is not `Paused`; otherwise it waits
/// for `SettleSccpV1::Refund`.
///
/// # Errors
///
/// Fails when the record is missing or has no pending refund, or a refund effect fails.
pub fn attempt_refund(
    state_transaction: &mut StateTransaction<'_, '_>,
    message_id: [u8; 32],
) -> Result<bool, Error> {
    let world = &*state_transaction.world;
    let mut record = store::outbound_messages::get(world, &message_id)
        .cloned()
        .ok_or_else(|| refuse("unknown outbound message"))?;
    if !record.status.is_refund_pending() {
        return Err(refuse("the outbound message has no pending refund"));
    }
    let (network, revision, amount) = (record.network, record.revision, record.amount);
    let enabled = store::parameters::get(world)
        .as_ref()
        .is_some_and(|params| params.enabled);
    let (activation, _) =
        activation(world, network, revision).ok_or_else(|| refuse("the revision is unknown"))?;
    if !enabled || activation == SccpRouteActivationV1::Paused {
        return Ok(false);
    }
    let height = state_transaction._curr_block.height().get();
    let creditable = if escrow::is_escrow(world, &record.sender) {
        None
    } else {
        match recipients::classify_account(state_transaction, record.sender.clone(), amount) {
            SccpRecipientClassV1::Creditable {
                account,
                registered,
            } => Some((account, registered)),
            SccpRecipientClassV1::Uncreditable { .. } | SccpRecipientClassV1::Undecodable => None,
        }
    };
    if let Some((account, registered)) = creditable {
        if !registered {
            recipients::register_recipient(state_transaction, &account, network, Some(message_id))?;
        }
        escrow::release(state_transaction, network, &account, amount, message_id)?;
        record.status = SccpOutboundStatusV1::Refunded(SccpStatusHeightV1 { height });
        state_transaction
            .world
            .emit_events(Some(SccpEvent::OutboundRefunded(SccpOutboundRefundedV1 {
                message_id,
                network,
                revision,
                nonce: record.nonce,
                recipient: account,
                amount,
            })));
    } else {
        escrow::strand(state_transaction, network, amount)?;
        record.status = SccpOutboundStatusV1::Stranded(SccpStatusHeightV1 { height });
        state_transaction
            .world
            .emit_events(Some(SccpEvent::OutboundStranded(SccpOutboundStrandedV1 {
                message_id,
                network,
                revision,
                nonce: record.nonce,
                amount,
            })));
    }
    adjust_liability(state_transaction, network, revision, -to_i128(amount)?)?;
    adjust_pending(state_transaction, (network, revision), 0, -1)?;
    store::outbound_messages::insert(state_transaction, message_id, record)?;
    Ok(true)
}

/// Execute `SettleSccpV1`: retry a `Pending` inbound settlement or outbound refund without a
/// proof (§4.12.1). A recipient settling its own message is charged the self-claim fee once,
/// at release (§4.12.4).
///
/// # Errors
///
/// Fails when the target is unknown or not pending, or when the retry changes nothing.
pub fn execute_settle(
    instruction: SettleSccpV1,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let changed = match instruction.target {
        SccpSettleTargetV1::Inbound(target) => {
            super::inbound::charge_self_claim_fee(state_transaction, target.message_id, authority)?;
            settle_inbound(state_transaction, target.message_id)?
        }
        SccpSettleTargetV1::Refund(target) => {
            let message_id = *store::outbound_by_nonce::get(
                &*state_transaction.world,
                &(target.network, target.revision, target.nonce),
            )
            .ok_or_else(|| refuse("unknown outbound nonce"))?;
            attempt_refund(state_transaction, message_id)?
        }
    };
    if changed {
        Ok(())
    } else {
        Err(refuse("nothing changed"))
    }
}
