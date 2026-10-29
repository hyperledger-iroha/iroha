//! Exact original-pool owners for the bounded SCCP release input graph.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
use iroha_allocation::{AllocationCharge, AllocationRefusal, ChargedBuffer, ChargedBufferError};
use iroha_data_model::asset::{AssetBalanceScope, AssetDefinitionId};
use iroha_primitives::numeric::{ChargedQuantity, QuantityAccumulator};
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

impl FundedAssetId {
    pub(in crate::smartcontracts::isi) fn new(
        tx: &StateTransaction<'_, '_>,
        definition: &AssetDefinitionId,
        account: &AccountId,
        scope: AssetBalanceScope,
    ) -> Result<Self, Error> {
        let budget = tx.world.sccp_committee_generations.allocation_budget();
        // SPEC: §13.4 HC973 retains real charges but substitutes their original pool.
        #[cfg(all(test, sumeragi_core_mutation = "HC973"))]
        let substituted_budget = iroha_allocation::AllocationBudget::new(budget.limit_bytes());
        #[cfg(all(test, sumeragi_core_mutation = "HC973"))]
        let budget = &substituted_budget;
        let mut count = 0_usize;
        let mut bytes = Some(0_usize);
        norito::core::classify_decode_attempt(|| {
            account.for_each_admission_clone_layout(|layout| {
                count = count.saturating_add(1);
                bytes = bytes.and_then(|value| value.checked_add(layout.size()));
            })
        })
        .map_err(|error| account_error(tx, error))?;
        let ledger = Layout::array::<AllocationCharge>(count)
            .map_err(|_| local(tx, AllocationRefusal::DemandOverflow))?;
        let bytes = bytes
            .and_then(|value| value.checked_add(ledger.size()))
            .ok_or_else(|| local(tx, AllocationRefusal::DemandOverflow))?;
        let mut reservation = budget
            .try_reserve_bytes(bytes)
            .map_err(|error| local(tx, error))?;
        let mut charges = ChargedBuffer::from_reservation(count, &mut reservation).map_err(
            |error| match error {
                iroha_allocation::PrepaidBufferError::Allocation(error) => buffer_error(tx, error),
                iroha_allocation::PrepaidBufferError::Reservation(_) => {
                    refuse("SCCP preflight ledger mismatch")
                }
            },
        )?;
        let mut mismatch = false;
        norito::core::classify_decode_attempt(|| {
            account.for_each_admission_clone_layout(|layout| match reservation.try_split(layout) {
                Ok(charge) => charges.push_reserved(charge),
                Err(_) => mismatch = true,
            })
        })
        .map_err(|error| account_error(tx, error))?;
        if mismatch {
            return Err(refuse("SCCP preflight account demand changed"));
        }
        let account = norito::core::classify_decode_attempt(|| account.try_clone_for_admission())
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
        let escrow = FundedAssetId::new(tx, &definition, &route.escrow, AssetBalanceScope::Global)
            .map_err(|error| (0, error))?;
        let budget = tx.world.sccp_committee_generations.allocation_budget();
        let mut prepared = Self {
            escrow,
            credits: [None, None],
        };
        let mut cumulative = QuantityAccumulator::zero();
        for (index, (to, value)) in credits.iter().flatten().enumerate() {
            let result = (|| {
                let amount = QuantityAccumulator::from_scaled_u128(*value, SCCP_XOR_SCALE)
                    .map_err(|_| refuse("SCCP amount is outside the canonical quantity domain"))?;
                cumulative
                    .try_add_accumulator(&amount)
                    .map_err(|_| refuse("SCCP release amount overflow"))?;
                let destination =
                    FundedAssetId::new(tx, &definition, to, AssetBalanceScope::Global)?;
                let amount = ChargedQuantity::try_from_accumulator(amount, budget)
                    .map_err(|error| buffer_error(tx, error))?;
                let cumulative = ChargedQuantity::try_from_accumulator(cumulative, budget)
                    .map_err(|error| buffer_error(tx, error))?;
                Ok((destination, amount, cumulative))
            })()
            .map_err(|error| (index, error))?;
            prepared.credits[index] = Some(result);
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
}
