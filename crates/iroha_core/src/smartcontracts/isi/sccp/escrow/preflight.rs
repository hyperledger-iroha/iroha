//! Exact original-pool owners for the bounded SCCP release input graph.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
use iroha_allocation::{
    AllocationCharge, AllocationRefusal, AllocationReservation, ChargedBuffer, ChargedBufferError,
};
use iroha_data_model::asset::{AssetBalanceScope, AssetDefinitionId};
use iroha_primitives::numeric::{ChargedQuantity, QuantityAccumulator, QuantityBackingError};
use std::alloc::Layout;

/// An asset identifier and the original charges of every nested account backing.
/// Field order destroys the value before refunding its immutable clone's ledger.
pub(in crate::smartcontracts::isi) struct FundedAssetId {
    value: AssetId,
    _charges: ChargedBuffer<AllocationCharge>,
}

fn refuse(message: &'static str) -> Error {
    Error::InvariantViolation(message.into())
}

fn local(tx: &StateTransaction<'_, '_>, error: impl Into<ExecutionDeferred>) -> Error {
    tx.world
        .attempt_error_to_instruction_error(ExecutionAttemptError::Deferred(error.into()))
}

pub(in crate::smartcontracts::isi) fn buffer_error(
    tx: &StateTransaction<'_, '_>,
    error: ChargedBufferError,
) -> Error {
    match error {
        ChargedBufferError::Admission(error) => local(tx, error),
        error @ ChargedBufferError::Allocator { .. } => local(
            tx,
            iroha_allocation::AllocatorRefusal::try_from(error).expect("matched physical refusal"),
        ),
    }
}

fn account_error(tx: &StateTransaction<'_, '_>, error: norito::core::DecodeAttemptError) -> Error {
    tx.world.attempt_error_to_instruction_error(
        crate::execution_attempt::canonical_decode_attempt_error(error, |_| {
            refuse("invalid SCCP preflight account clone")
        }),
    )
}

/// Exact immutable account-clone geometry, inspected without allocating a prefix.
#[derive(Clone, Copy)]
pub(in crate::smartcontracts::isi) struct AccountClonePlan<'account> {
    account: &'account AccountId,
    count: usize,
    bytes: usize,
}

impl<'account> AccountClonePlan<'account> {
    pub(in crate::smartcontracts::isi) fn new(
        tx: &StateTransaction<'_, '_>,
        account: &'account AccountId,
    ) -> Result<Self, Error> {
        let mut count = Some(0_usize);
        let mut bytes = Some(0_usize);
        norito::core::classify_decode_attempt(|| {
            account.for_each_admission_clone_layout(|layout| {
                count = count.and_then(|value| value.checked_add(1));
                bytes = bytes.and_then(|value| value.checked_add(layout.size()));
            })
        })
        .map_err(|error| account_error(tx, error))?;
        let count = count.ok_or_else(|| local(tx, AllocationRefusal::DemandOverflow))?;
        let ledger = Layout::array::<AllocationCharge>(count)
            .map_err(|_| local(tx, AllocationRefusal::DemandOverflow))?;
        let bytes = bytes
            .and_then(|value| value.checked_add(ledger.size()))
            .ok_or_else(|| local(tx, AllocationRefusal::DemandOverflow))?;
        Ok(Self {
            account,
            count,
            bytes,
        })
    }

    pub(in crate::smartcontracts::isi) fn bytes(&self) -> usize {
        self.bytes
    }
}

pub(in crate::smartcontracts::isi) fn add_preflight_demand(
    tx: &StateTransaction<'_, '_>,
    demand: &mut usize,
    bytes: usize,
) -> Result<(), Error> {
    *demand = demand
        .checked_add(bytes)
        .ok_or_else(|| local(tx, AllocationRefusal::DemandOverflow))?;
    Ok(())
}

pub(in crate::smartcontracts::isi) fn reserve_preflight(
    tx: &StateTransaction<'_, '_>,
    bytes: usize,
) -> Result<AllocationReservation, Error> {
    tx.world
        .sccp_committee_generations
        .allocation_budget()
        .try_reserve_bytes(bytes)
        .map_err(|error| local(tx, error))
}

impl FundedAssetId {
    pub(in crate::smartcontracts::isi) fn new(
        tx: &StateTransaction<'_, '_>,
        definition: &AssetDefinitionId,
        account: &AccountId,
        scope: AssetBalanceScope,
    ) -> Result<Self, Error> {
        let plan = AccountClonePlan::new(tx, account)?;
        let mut reservation = reserve_preflight(tx, plan.bytes())?;
        Self::from_reservation(tx, definition, plan, scope, &mut reservation)
    }

    pub(in crate::smartcontracts::isi) fn from_reservation(
        tx: &StateTransaction<'_, '_>,
        definition: &AssetDefinitionId,
        plan: AccountClonePlan<'_>,
        scope: AssetBalanceScope,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, Error> {
        let budget = tx.world.sccp_committee_generations.allocation_budget();
        if !reservation.belongs_to(budget) {
            return Err(local(
                tx,
                ivm::error::ExecutionDeferral::LocalInvariantViolation,
            ));
        }
        // SPEC: §13.4 HC973 retains real account backing and charges but replaces
        // only their original pool. Consume the original account slice so the
        // same aggregate inventory still closes; quantities retain their pool.
        #[cfg(all(test, sumeragi_core_mutation = "HC973"))]
        {
            let substituted = iroha_allocation::AllocationBudget::new(budget.limit_bytes());
            let mut replacement = substituted
                .try_reserve_bytes(plan.bytes())
                .map_err(|error| local(tx, error))?;
            let slice = Layout::from_size_align(plan.bytes(), 1)
                .map_err(|_| local(tx, AllocationRefusal::DemandOverflow))?;
            let unused = reservation
                .try_split(slice)
                .map_err(|_| refuse("SCCP preflight account reservation mismatch"))?;
            drop(unused);
            return Self::construct(tx, definition, plan, scope, &mut replacement);
        }
        #[cfg(not(all(test, sumeragi_core_mutation = "HC973")))]
        Self::construct(tx, definition, plan, scope, reservation)
    }

    fn construct(
        tx: &StateTransaction<'_, '_>,
        definition: &AssetDefinitionId,
        plan: AccountClonePlan<'_>,
        scope: AssetBalanceScope,
        reservation: &mut AllocationReservation,
    ) -> Result<Self, Error> {
        let before = reservation.remaining_bytes();
        if before < plan.bytes() {
            return Err(refuse("SCCP preflight account reservation mismatch"));
        }
        let mut charges = ChargedBuffer::from_reservation(plan.count, reservation).map_err(
            |error| match error {
                iroha_allocation::PrepaidBufferError::Allocation(error) => buffer_error(tx, error),
                iroha_allocation::PrepaidBufferError::Reservation(_) => {
                    refuse("SCCP preflight ledger mismatch")
                }
            },
        )?;
        let mut mismatch = false;
        norito::core::classify_decode_attempt(|| {
            plan.account.for_each_admission_clone_layout(|layout| {
                if charges.as_slice().len() == plan.count {
                    mismatch = true;
                    return;
                }
                match reservation.try_split(layout) {
                    Ok(charge) => charges.push_reserved(charge),
                    Err(_) => mismatch = true,
                }
            })
        })
        .map_err(|error| account_error(tx, error))?;
        if mismatch
            || charges.as_slice().len() != plan.count
            || before - reservation.remaining_bytes() != plan.bytes()
        {
            return Err(refuse("SCCP preflight account demand changed"));
        }
        let account =
            norito::core::classify_decode_attempt(|| plan.account.try_clone_for_admission())
                .map_err(|error| account_error(tx, error))?;
        Ok(Self {
            value: AssetId::with_scope(definition.clone(), account, scope),
            _charges: charges,
        })
    }

    pub(in crate::smartcontracts::isi) fn get(&self) -> &AssetId {
        &self.value
    }
}

fn quantity_layout(
    tx: &StateTransaction<'_, '_>,
    value: &QuantityAccumulator,
) -> Result<Layout, Error> {
    value
        .admission_layout()
        .map_err(|_| local(tx, AllocationRefusal::DemandOverflow))
}

fn materialize_quantity(
    tx: &StateTransaction<'_, '_>,
    value: QuantityAccumulator,
    reservation: &mut AllocationReservation,
) -> Result<ChargedQuantity, Error> {
    let budget = tx.world.sccp_committee_generations.allocation_budget();
    let charge = reservation
        .try_split(quantity_layout(tx, &value)?)
        .map_err(|_| refuse("SCCP preflight quantity reservation mismatch"))?;
    ChargedQuantity::try_from_accumulator_charge(value, budget, charge).map_err(
        |(charge, error)| {
            drop(charge);
            match error {
                QuantityBackingError::Allocation(
                    original @ iroha_allocation::ChargedBufferFromChargeError::Allocator { .. },
                ) => local(
                    tx,
                    iroha_allocation::AllocatorRefusal::try_from(original)
                        .expect("matched physical quantity refusal"),
                ),
                _ => local(tx, ivm::error::ExecutionDeferral::LocalInvariantViolation),
            }
        },
    )
}

/// Bounded immutable inputs prepared before recipient registration or pending writes.
pub(in crate::smartcontracts::isi::sccp) struct PreparedReleases {
    escrow: FundedAssetId,
    credits: [Option<(FundedAssetId, ChargedQuantity, ChargedQuantity)>; 2],
}

impl PreparedReleases {
    pub(in crate::smartcontracts::isi::sccp) fn new(
        tx: &StateTransaction<'_, '_>,
        network: SccpNetworkV1,
        credits: &[Option<(&AccountId, u128)>; 2],
    ) -> Result<Option<Self>, (usize, Error)> {
        if credits.iter().all(Option::is_none) {
            return Ok(None);
        }
        let route = store::routes::get(&*tx.world, &network)
            .ok_or_else(|| (0, refuse("SCCP route is not registered")))?;
        let definition = sccp_taira_xor_asset_definition_id();
        let escrow_plan = AccountClonePlan::new(tx, &route.escrow).map_err(|error| (0, error))?;
        let mut demand = escrow_plan.bytes();
        let mut plans = [None, None];
        let mut cumulative = QuantityAccumulator::zero();
        for (index, (to, value)) in credits.iter().flatten().enumerate() {
            let plan = (|| {
                let amount = QuantityAccumulator::from_scaled_u128(*value, SCCP_XOR_SCALE)
                    .map_err(|_| refuse("SCCP amount is outside the canonical quantity domain"))?;
                cumulative
                    .try_add_accumulator(&amount)
                    .map_err(|_| refuse("SCCP release amount overflow"))?;
                let account = AccountClonePlan::new(tx, to)?;
                add_preflight_demand(tx, &mut demand, account.bytes())?;
                add_preflight_demand(tx, &mut demand, quantity_layout(tx, &amount)?.size())?;
                add_preflight_demand(tx, &mut demand, quantity_layout(tx, &cumulative)?.size())?;
                Ok((account, amount, cumulative))
            })()
            .map_err(|error| (index, error))?;
            plans[index] = Some(plan);
        }
        // SPEC: §13.4 HC982 constructs a real escrow prefix before aggregate admission.
        #[cfg(all(test, sumeragi_core_mutation = "HC982"))]
        let _premature_escrow =
            FundedAssetId::new(tx, &definition, &route.escrow, AssetBalanceScope::Global)
                .map_err(|error| (0, error))?;
        // All account ledgers, nested keys and quantity digits are exact and
        // immutable here. Refuse before constructing any refundable prefix.
        let mut reservation = reserve_preflight(tx, demand).map_err(|error| (0, error))?;
        let escrow = FundedAssetId::from_reservation(
            tx,
            &definition,
            escrow_plan,
            AssetBalanceScope::Global,
            &mut reservation,
        )
        .map_err(|error| (0, error))?;
        let mut prepared = Self {
            escrow,
            credits: [None, None],
        };
        for (index, (account, amount, cumulative)) in plans.into_iter().flatten().enumerate() {
            let result = (|| {
                let destination = FundedAssetId::from_reservation(
                    tx,
                    &definition,
                    account,
                    AssetBalanceScope::Global,
                    &mut reservation,
                )?;
                let amount = materialize_quantity(tx, amount, &mut reservation)?;
                let cumulative = materialize_quantity(tx, cumulative, &mut reservation)?;
                Ok((destination, amount, cumulative))
            })()
            .map_err(|error| (index, error))?;
            prepared.credits[index] = Some(result);
        }
        if reservation.remaining_bytes() != 0 {
            return Err((
                0,
                refuse("SCCP preflight complete input reservation mismatch"),
            ));
        }
        Ok(Some(prepared))
    }

    pub(in crate::smartcontracts::isi::sccp) fn check(
        &self,
        tx: &StateTransaction<'_, '_>,
    ) -> Result<(), (usize, Error)> {
        let credits = self.credits.each_ref().map(|credit| {
            credit
                .as_ref()
                .map(|(id, amount, cumulative)| (id.get(), amount.get(), cumulative.get()))
        });
        asset_isi::precheck_sccp_escrow_releases(tx, self.escrow.get(), &credits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, funded_xor_state, header};
    use iroha_allocation::AllocationBudget;

    fn account_bytes(account: &AccountId) -> usize {
        let mut count = 0;
        let mut bytes = 0;
        account
            .for_each_admission_clone_layout(|layout| {
                count += 1;
                bytes += layout.size();
            })
            .unwrap();
        bytes + Layout::array::<AllocationCharge>(count).unwrap().size()
    }

    #[test]
    fn bounded_inputs_keep_exact_original_charges_until_drop_and_unwind() {
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let holder = authority(1);
        let state = funded_xor_state(&holder, 10_000_000_000);
        let mut block = state.block(header(1));
        let mut tx = block.transaction();
        create_route_escrows(&mut tx).unwrap();
        let network = SccpNetworkV1::EthereumMainnet;
        let route = store::routes::get(&*tx.world, &network).unwrap();
        let recipient = authority(9);
        let credits = [
            Some((&holder, 123_u128)),
            Some((&recipient, u128::MAX - 123)),
        ];
        let budget = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let baseline = budget.reserved_bytes();
        let quantity_bytes = [123, 123, u128::MAX - 123, u128::MAX]
            .into_iter()
            .map(|value| {
                QuantityAccumulator::from_scaled_u128(value, SCCP_XOR_SCALE)
                    .unwrap()
                    .admission_layout()
                    .unwrap()
                    .size()
            })
            .sum::<usize>();
        let expected = account_bytes(&route.escrow)
            + account_bytes(&holder)
            + account_bytes(&recipient)
            + quantity_bytes;
        let foreign = AllocationBudget::new(budget.limit_bytes());
        let prepared = PreparedReleases::new(&tx, network, &credits)
            .unwrap()
            .unwrap();
        assert!(
            prepared.escrow._charges.belongs_to(&budget),
            "HC973 preflight account backing must retain the original allocation pool"
        );
        for (id, amount, cumulative) in prepared.credits.iter().flatten() {
            assert!(
                id._charges.belongs_to(&budget),
                "HC973 preflight account backing must retain the original allocation pool"
            );
            assert!(amount.belongs_to(&budget));
            assert!(cumulative.belongs_to(&budget));
            assert!(!amount.belongs_to(&foreign));
        }
        assert_eq!(budget.reserved_bytes(), baseline + expected);
        assert_eq!(
            prepared.credits[1].as_ref().unwrap().2.get(),
            &xor_quantity(u128::MAX).unwrap()
        );
        drop(prepared);
        assert_eq!(budget.reserved_bytes(), baseline);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _prepared = PreparedReleases::new(&tx, network, &credits)
                .unwrap()
                .unwrap();
            panic!("controlled release-preflight unwind");
        }));
        assert!(unwound.is_err());
        assert_eq!(budget.reserved_bytes(), baseline);
        assert!(!tx.is_execution_deferred());
    }

    #[test]
    fn physical_ledger_account_and_quantity_refusals_release_partial_inputs() {
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let holder = authority(1);
        let mut account_layouts = Vec::new();
        holder
            .for_each_admission_clone_layout(|layout| account_layouts.push(layout))
            .unwrap();
        let quantity_layout = QuantityAccumulator::from_scaled_u128(u128::MAX, SCCP_XOR_SCALE)
            .unwrap()
            .admission_layout()
            .unwrap();
        let ledger_layout = Layout::array::<AllocationCharge>(account_layouts.len()).unwrap();
        for layout in [ledger_layout, account_layouts[0], quantity_layout] {
            let state = funded_xor_state(&holder, 10_000_000_000);
            let mut block = state.block(header(1));
            let mut tx = block.transaction();
            create_route_escrows(&mut tx).unwrap();
            let budget = tx
                .world
                .sccp_committee_generations
                .allocation_budget()
                .clone();
            let baseline = budget.reserved_bytes();
            let (result, refused) =
                crate::test_allocations::refuse_one_layout_during(layout, || {
                    PreparedReleases::new(
                        &tx,
                        SccpNetworkV1::EthereumMainnet,
                        &[Some((&holder, u128::MAX)), None],
                    )
                });
            assert!(
                refused,
                "selected concrete backing must reach the allocator"
            );
            assert!(result.is_err());
            assert!(tx.is_execution_deferred());
            let reason = tx.borrow_execution_deferral().unwrap();
            assert!(reason.allocator_refusal().is_some() || reason.decode_refusal().is_some());
            assert!(
                reason.allocation_refusal().is_none(),
                "physical failures have no fabricated pool wait"
            );
            assert_eq!(budget.reserved_bytes(), baseline);
        }
    }

    #[test]
    fn exhausted_original_pool_refuses_before_building_any_release_input() {
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let holder = authority(1);
        let state = funded_xor_state(&holder, 10_000_000_000);
        let mut block = state.block(header(1));
        let mut tx = block.transaction();
        create_route_escrows(&mut tx).unwrap();
        let budget = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let baseline = budget.reserved_bytes();
        let held = budget
            .try_reserve_bytes(budget.limit_bytes() - baseline)
            .unwrap();
        assert!(
            PreparedReleases::new(
                &tx,
                SccpNetworkV1::EthereumMainnet,
                &[Some((&holder, 1)), None]
            )
            .is_err()
        );
        let reason = tx.borrow_execution_deferral().unwrap();
        assert!(matches!(
            reason.allocation_refusal(),
            Some(AllocationRefusal::Capacity { .. })
        ));
        drop(reason);
        drop(held);
        assert_eq!(budget.reserved_bytes(), baseline);
    }
    #[test]
    fn complete_release_inputs_refuse_without_refunding_an_admitted_prefix() {
        use iroha_allocation::release::ReleaseRegistration;
        use std::task::{Context, Poll, Waker};
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let holder = authority(1);
        let recipient = authority(9);
        let state = funded_xor_state(&holder, 10_000_000_000);
        let before_state = crate::snapshot::canonical_state_snapshot_bytes(&state);
        let mut block = state.block(header(1));
        {
            let mut setup = block.transaction();
            create_route_escrows(&mut setup).unwrap();
            setup.apply();
        }
        let mut tx = block.transaction();
        let network = SccpNetworkV1::EthereumMainnet;
        let credits = [Some((&holder, 123)), Some((&recipient, u128::MAX - 123))];
        let route = store::routes::get(&*tx.world, &network).unwrap();
        let expected = account_bytes(&route.escrow)
            + account_bytes(&holder)
            + account_bytes(&recipient)
            + [123, 123, u128::MAX - 123, u128::MAX]
                .into_iter()
                .map(|value| {
                    QuantityAccumulator::from_scaled_u128(value, SCCP_XOR_SCALE)
                        .unwrap()
                        .admission_layout()
                        .unwrap()
                        .size()
                })
                .sum::<usize>();
        let budget = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let baseline = budget.reserved_bytes();
        let held = budget
            .try_reserve_bytes(budget.limit_bytes() - baseline - expected + 1)
            .unwrap();
        let occupied = budget.reserved_bytes();
        let peak_before = budget.peak_reserved_bytes();
        let watch = AllocationBudget::new(ReleaseRegistration::allocation_layout().size());
        let mut registration = crate::unit_test_support::release_registration(&watch);
        registration.bind_thread(std::thread::current());
        assert!(PreparedReleases::new(&tx, network, &credits).is_err());
        assert!(tx.is_execution_deferred());
        let original = tx.take_execution_deferral().unwrap();
        let Some(AllocationRefusal::Capacity {
            requested_bytes,
            release,
            ..
        }) = original.allocation_refusal()
        else {
            panic!("retain the actual original aggregate admission refusal");
        };
        assert_eq!(
            registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
            Poll::Pending,
            "SCCP input preparation must not satisfy its own original release wait"
        );
        assert_eq!(*requested_bytes, expected);
        assert_eq!(budget.reserved_bytes(), occupied);
        assert_eq!(
            budget.peak_reserved_bytes(),
            peak_before,
            "no input prefix may be admitted before complete demand"
        );
        let foreign = AllocationBudget::new(expected);
        drop(foreign.try_reserve_bytes(expected).unwrap());
        assert_eq!(
            registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
            Poll::Pending
        );
        assert!(tx.world.account(&recipient).is_err());
        drop(held);
        assert_eq!(
            registration.poll_wait(release, &mut Context::from_waker(Waker::noop())),
            Poll::Ready(())
        );
        // Abandon the refused transaction. The fresh transaction borrows the
        // same original State pool and its earlier separately applied route.
        drop(tx);
        let retry = block.transaction();
        let before_retry = budget.reserved_bytes();
        let prepared = PreparedReleases::new(&retry, network, &credits)
            .unwrap()
            .unwrap();
        assert_eq!(budget.reserved_bytes(), before_retry + expected);
        assert!(prepared.escrow._charges.belongs_to(&budget));
        assert!(!retry.is_execution_deferred());
        drop(prepared);
        assert_eq!(budget.reserved_bytes(), before_retry);
        drop(retry);
        drop(block);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes(&state),
            before_state
        );
    }

    #[test]
    fn late_quantity_physical_refusal_keeps_original_layout_and_retires_all_inputs() {
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let holder = authority(1);
        let recipient = authority(9);
        let state = funded_xor_state(&holder, 10_000_000_000);
        let mut block = state.block(header(1));
        let mut tx = block.transaction();
        create_route_escrows(&mut tx).unwrap();
        let mut total =
            QuantityAccumulator::from_scaled_u128(u128::MAX - 1, SCCP_XOR_SCALE).unwrap();
        let addend = total;
        total.try_add_accumulator(&addend).unwrap();
        let layout = total.admission_layout().unwrap();
        assert_ne!(
            layout,
            QuantityAccumulator::from_scaled_u128(u128::MAX - 1, SCCP_XOR_SCALE)
                .unwrap()
                .admission_layout()
                .unwrap()
        );
        let budget = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let baseline = budget.reserved_bytes();
        let route = store::routes::get(&*tx.world, &SccpNetworkV1::EthereumMainnet).unwrap();
        // A native quantity layout may equal an account charge-ledger layout.
        // Count only the real earlier matching allocations, then refuse the
        // final cumulative magnitude rather than an unrelated first match.
        let mut earlier = 0;
        for account in [&route.escrow, &holder, &recipient] {
            let mut count = 0;
            account
                .for_each_admission_clone_layout(|candidate| {
                    count += 1;
                    earlier += usize::from(candidate == layout);
                })
                .unwrap();
            earlier += usize::from(Layout::array::<AllocationCharge>(count).unwrap() == layout);
        }
        let amount = QuantityAccumulator::from_scaled_u128(u128::MAX - 1, SCCP_XOR_SCALE).unwrap();
        earlier += 3 * usize::from(amount.admission_layout().unwrap() == layout);
        let (result, refused) =
            crate::test_allocations::refuse_layout_occurrence_during(layout, earlier, || {
                PreparedReleases::new(
                    &tx,
                    SccpNetworkV1::EthereumMainnet,
                    &[
                        Some((&holder, u128::MAX - 1)),
                        Some((&recipient, u128::MAX - 1)),
                    ],
                )
            });
        assert!(
            refused,
            "the final cumulative quantity reaches its physical allocation"
        );
        assert!(matches!(result, Err((1, _))));
        let original = tx.borrow_execution_deferral().unwrap();
        let error = original
            .allocator_refusal()
            .expect("exact original physical quantity refusal");
        assert!(
            matches!(error.buffer_from_charge_error(), Some(iroha_allocation::ChargedBufferFromChargeError::Allocator { layout: actual }) if *actual == layout)
        );
        assert!(original.allocation_refusal().is_none());
        assert_eq!(budget.reserved_bytes(), baseline);
    }

    #[test]
    fn account_plan_constructor_preserves_foreign_short_and_overflow_ownership() {
        let _fixture_retirement_pin = crossbeam_epoch::pin();
        let holder = authority(1);
        let state = funded_xor_state(&holder, 10_000_000_000);
        let mut block = state.block(header(1));
        let tx = block.transaction();
        let definition = sccp_taira_xor_asset_definition_id();
        let plan = AccountClonePlan::new(&tx, &holder).unwrap();
        assert_eq!(plan.bytes(), account_bytes(&holder));
        let foreign = AllocationBudget::new(plan.bytes());
        let mut reservation = foreign.try_reserve_bytes(plan.bytes()).unwrap();
        assert!(
            FundedAssetId::from_reservation(
                &tx,
                &definition,
                plan,
                AssetBalanceScope::Global,
                &mut reservation
            )
            .is_err()
        );
        assert_eq!(reservation.remaining_bytes(), plan.bytes());
        assert_eq!(foreign.reserved_bytes(), plan.bytes());
        assert!(tx.is_execution_deferred());
        drop(reservation);
        assert_eq!(foreign.reserved_bytes(), 0);
        let mut short = reserve_preflight(&tx, plan.bytes() - 1).unwrap();
        let before = short.remaining_bytes();
        assert!(
            FundedAssetId::from_reservation(
                &tx,
                &definition,
                plan,
                AssetBalanceScope::Global,
                &mut short
            )
            .is_err()
        );
        assert_eq!(short.remaining_bytes(), before);
        let mut overflow = usize::MAX;
        assert!(add_preflight_demand(&tx, &mut overflow, 1).is_err());
        assert_eq!(overflow, usize::MAX);
    }
}
