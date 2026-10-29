//! Balanced-entry guard for SCCP value transitions (`specs/sccp.md` §4.15).
//!
//! Escrow movements ([`super::escrow`]) and the liability and `stranded` bookkeeping are written
//! by separate callers, so one forgotten, doubled or misordered update would let the recorded
//! backing drift from the escrow balance without any single write failing. [`ValueGuard`]
//! closes that gap at every boundary where SCCP value can move: it captures each route's
//! escrow balance, `Σ liability` and `stranded` before an SCCP value instruction or a
//! Parliament enactment, and afterwards refuses the transition unless, for every route,
//!
//! - the escrow balance changed by exactly the change of `Σ_r liability(r) + stranded(route)`,
//!   and
//! - every revision whose liability grew stays within its `max_wrapped_supply`.
//!
//! The check compares changes, not absolute values, so it holds each transition to the
//! invariant without letting an earlier inconsistency refuse unrelated work. A refusal is an
//! execution invariant violation that rolls the transition back: an accounting defect fails
//! closed instead of committing value that the escrow does not back.

use super::{Error, store};
use crate::state::WorldReadOnly;
use iroha_data_model::{
    account::AccountId, asset::AssetId, bridge::SccpNetworkV1,
    sccp::escrow::sccp_taira_xor_asset_definition_id,
};
use mv::storage::StorageReadOnly;

fn refuse(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("SCCP value conservation: {reason}").into())
}

/// One route's value position: its escrow balance and the backing it records.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RoutePosition {
    network: SccpNetworkV1,
    escrow: u128,
    /// `Σ_r liability(r) + stranded(route)`.
    backing: u128,
    /// `(revision, liability)` of every revision.
    liabilities: Vec<(u32, u128)>,
}

/// The value positions of every route, captured before an SCCP transition.
#[derive(Debug, Clone)]
#[must_use = "a captured guard must verify the transition"]
pub struct ValueGuard {
    routes: Vec<RoutePosition>,
}

impl ValueGuard {
    /// Capture the value position of every route.
    ///
    /// # Errors
    ///
    /// Fails when an escrow balance is not a canonical XOR amount or a route's backing
    /// overflows `u128`.
    pub fn capture(world: &(impl WorldReadOnly + ?Sized)) -> Result<Self, Error> {
        Ok(Self {
            routes: positions(world)?,
        })
    }

    /// Check that the transition since [`Self::capture`] was a balanced entry for every route
    /// and grew no revision's liability past its cap.
    ///
    /// # Errors
    ///
    /// Fails when a route's escrow balance and recorded backing changed by different amounts,
    /// a route disappeared, or a revision's liability grew past its `max_wrapped_supply`.
    pub fn verify(self, world: &(impl WorldReadOnly + ?Sized)) -> Result<(), Error> {
        if cfg!(all(test, sumeragi_core_mutation = "HC488")) {
            return Ok(());
        }
        let after = positions(world)?;
        for before in &self.routes {
            if !after.iter().any(|route| route.network == before.network) {
                return Err(refuse(format_args!(
                    "route {} disappeared",
                    before.network.profile_key()
                )));
            }
        }
        for route in &after {
            let before = self
                .routes
                .iter()
                .find(|before| before.network == route.network);
            let (escrow_before, backing_before) =
                before.map_or((0, 0), |before| (before.escrow, before.backing));
            let escrow_change = Change::between(escrow_before, route.escrow);
            let backing_change = Change::between(backing_before, route.backing);
            if escrow_change != backing_change {
                return Err(refuse(format_args!(
                    "route {} escrow changed by {escrow_change} but Σ liability + stranded \
                     changed by {backing_change}",
                    route.network.profile_key()
                )));
            }
            for &(revision, liability) in &route.liabilities {
                let previous = before
                    .and_then(|before| {
                        before
                            .liabilities
                            .iter()
                            .find(|(number, _)| *number == revision)
                    })
                    .map_or(0, |&(_, liability)| liability);
                if liability <= previous {
                    continue;
                }
                let cap = store::routes::get(world, &route.network)
                    .and_then(|stored| stored.revisions.get(&revision))
                    .map(|record| record.max_wrapped_supply)
                    .ok_or_else(|| refuse(format_args!("revision {revision} disappeared")))?;
                if liability > cap {
                    return Err(refuse(format_args!(
                        "route {} revision {revision} liability {liability} grew past its cap \
                         {cap}",
                        route.network.profile_key()
                    )));
                }
            }
        }
        Ok(())
    }
}

/// A signed change between two `u128` values, compared exactly without overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Unchanged,
    Up(u128),
    Down(u128),
}

impl Change {
    fn between(before: u128, after: u128) -> Self {
        match after.cmp(&before) {
            core::cmp::Ordering::Equal => Self::Unchanged,
            core::cmp::Ordering::Greater => Self::Up(after - before),
            core::cmp::Ordering::Less => Self::Down(before - after),
        }
    }
}

impl core::fmt::Display for Change {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unchanged => f.write_str("0"),
            Self::Up(amount) => write!(f, "+{amount}"),
            Self::Down(amount) => write!(f, "-{amount}"),
        }
    }
}

fn positions(world: &(impl WorldReadOnly + ?Sized)) -> Result<Vec<RoutePosition>, Error> {
    store::routes::iter(world)
        .map(|(network, route)| {
            let liabilities: Vec<(u32, u128)> = route
                .revisions
                .values()
                .map(|record| (record.revision, record.liability))
                .collect();
            let backing = liabilities
                .iter()
                .try_fold(route.stranded, |sum, &(_, liability)| {
                    sum.checked_add(liability)
                })
                .ok_or_else(|| {
                    refuse(format_args!(
                        "route {} backing overflows",
                        network.profile_key()
                    ))
                })?;
            Ok(RoutePosition {
                network: *network,
                escrow: escrow_units(world, &route.escrow)?,
                backing,
                liabilities,
            })
        })
        .collect()
}

/// The XOR balance of `escrow` in Taira units; an absent or zero balance is 0.
fn escrow_units(world: &(impl WorldReadOnly + ?Sized), escrow: &AccountId) -> Result<u128, Error> {
    let Some(value) = world.assets().get(&AssetId::of(
        sccp_taira_xor_asset_definition_id(),
        escrow.clone(),
    )) else {
        return Ok(0);
    };
    let quantity = value.as_ref();
    if quantity.is_zero() {
        return Ok(0);
    }
    iroha_sccp::v1::amount::taira_units(quantity.as_numeric())
        .map_err(|error| refuse(format_args!("escrow balance: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        ONE_XOR, authority, funded_xor_state, header, install_value_route, set_xor_balance,
    };
    use iroha_data_model::sccp::registry::SccpRouteActivationV1;

    const NETWORK: SccpNetworkV1 = SccpNetworkV1::EthereumMainnet;
    /// One XOR in Taira units, signed for liability deltas.
    const XOR: i128 = 1_000_000_000;

    fn escrow(world: &(impl WorldReadOnly + ?Sized)) -> AccountId {
        store::routes::get(world, &NETWORK)
            .expect("route")
            .escrow
            .clone()
    }

    /// Book `delta` on revision 1's liability and `stranded_delta` on the route, without moving
    /// escrow value.
    fn book(stx: &mut crate::state::StateTransaction<'_, '_>, delta: i128, stranded_delta: i128) {
        let mut route = store::routes::get(&*stx.world, &NETWORK)
            .cloned()
            .expect("route");
        let record = route.revisions.get_mut(&1).expect("revision");
        record.liability = record
            .liability
            .checked_add_signed(delta)
            .expect("liability");
        route.stranded = route
            .stranded
            .checked_add_signed(stranded_delta)
            .expect("stranded");
        store::routes::insert(stx, NETWORK, route).expect("route");
    }

    #[test]
    fn balanced_transitions_pass() {
        let holder = authority(1);
        let state = funded_xor_state(&holder, 100 * ONE_XOR);
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        install_value_route(
            &mut stx,
            NETWORK,
            1,
            SccpRouteActivationV1::Bidirectional,
            50 * ONE_XOR,
        );
        let escrow = escrow(&*stx.world);

        // A lock: escrow and liability both grow.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        set_xor_balance(&mut stx, &escrow, 10 * ONE_XOR);
        book(&mut stx, 10 * XOR, 0);
        guard.verify(&*stx.world).expect("a lock is balanced");

        // A strand: liability moves to `stranded`, the escrow keeps the value.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        book(&mut stx, -4 * XOR, 4 * XOR);
        guard.verify(&*stx.world).expect("a strand is balanced");

        // A release: escrow and liability both shrink.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        set_xor_balance(&mut stx, &escrow, 7 * ONE_XOR);
        book(&mut stx, -3 * XOR, 0);
        guard.verify(&*stx.world).expect("a release is balanced");

        // A transition that moves nothing.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        guard.verify(&*stx.world).expect("no movement is balanced");
    }

    #[test]
    fn unbalanced_transitions_are_refused() {
        let holder = authority(1);
        let state = funded_xor_state(&holder, 100 * ONE_XOR);
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        install_value_route(
            &mut stx,
            NETWORK,
            1,
            SccpRouteActivationV1::Bidirectional,
            50 * ONE_XOR,
        );
        let escrow = escrow(&*stx.world);
        set_xor_balance(&mut stx, &escrow, 10 * ONE_XOR);
        book(&mut stx, 10 * XOR, 0);

        // Value released from the escrow without debiting liability.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        set_xor_balance(&mut stx, &escrow, 9 * ONE_XOR);
        let refusal = guard.verify(&*stx.world);
        assert!(
            refusal.is_err(),
            "the value guard must refuse an escrow release that debits no liability"
        );
        let error = refusal.unwrap_err().to_string();
        assert!(error.contains("escrow changed by -1000000000"), "{error}");
        set_xor_balance(&mut stx, &escrow, 10 * ONE_XOR);

        // Liability debited without releasing value (the value would be unaccounted).
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        book(&mut stx, -XOR, 0);
        assert!(
            guard.verify(&*stx.world).is_err(),
            "unaccounted escrow value"
        );
        book(&mut stx, XOR, 0);

        // Liability double-debited while stranding once.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        book(&mut stx, -2 * XOR, XOR);
        assert!(guard.verify(&*stx.world).is_err(), "a doubled debit");
        book(&mut stx, 2 * XOR, -XOR);

        // Escrow credited twice for one lock.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        set_xor_balance(&mut stx, &escrow, 12 * ONE_XOR);
        book(&mut stx, XOR, 0);
        assert!(guard.verify(&*stx.world).is_err(), "a doubled credit");
    }

    #[test]
    fn liability_growth_past_the_cap_is_refused_but_existing_excess_is_not() {
        let holder = authority(1);
        let state = funded_xor_state(&holder, 100 * ONE_XOR);
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        install_value_route(
            &mut stx,
            NETWORK,
            1,
            SccpRouteActivationV1::Bidirectional,
            5 * ONE_XOR,
        );
        let escrow = escrow(&*stx.world);

        let guard = ValueGuard::capture(&*stx.world).unwrap();
        set_xor_balance(&mut stx, &escrow, 6 * ONE_XOR);
        book(&mut stx, 6 * XOR, 0);
        let error = guard.verify(&*stx.world).expect_err("growth past the cap");
        assert!(error.to_string().contains("grew past its cap"), "{error}");

        // An excess that already exists does not refuse a transition that shrinks it.
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        set_xor_balance(&mut stx, &escrow, 5 * ONE_XOR + 1);
        book(&mut stx, -XOR + 1, 0);
        guard.verify(&*stx.world).expect("a shrinking excess");
    }

    #[test]
    fn routes_created_in_the_transition_start_from_zero() {
        let holder = authority(1);
        let state = funded_xor_state(&holder, 100 * ONE_XOR);
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let guard = ValueGuard::capture(&*stx.world).unwrap();
        install_value_route(
            &mut stx,
            NETWORK,
            1,
            SccpRouteActivationV1::Bidirectional,
            5 * ONE_XOR,
        );
        guard
            .verify(&*stx.world)
            .expect("empty routes are balanced");

        let guard = ValueGuard::capture(&*stx.world).unwrap();
        let escrow = escrow(&*stx.world);
        set_xor_balance(&mut stx, &escrow, ONE_XOR);
        assert!(
            guard.verify(&*stx.world).is_err(),
            "value appearing in an escrow without backing"
        );
    }
}
