//! This module contains [`Asset`] structure, it's implementation and related traits and
//! instructions implementations.

use iroha_data_model::{
    isi::error::{MathError, Mismatch, TypeError},
    prelude::*,
    query::error::FindError,
};
use iroha_telemetry::metrics;

use super::prelude::*;

impl Registrable for NewAssetDefinition {
    type Target = AssetDefinition;

    #[inline]
    fn build(self, authority: &AccountId) -> Self::Target {
        Self::Target {
            id: self.id,
            spec: self.spec,
            mintable: self.mintable,
            logo: self.logo,
            metadata: self.metadata,
            owned_by: authority.clone(),
            total_quantity: Numeric::ZERO,
        }
    }
}

/// ISI module contains all instructions related to assets:
/// - minting/burning assets
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use iroha_data_model::isi::error::MintabilityError;

    use super::*;
    use crate::smartcontracts::account::isi::forbid_minting;

    impl Execute for Mint<Numeric, Asset> {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_id = self.destination;

            let asset_definition = state_transaction
                .world
                .asset_definition(&asset_id.definition)?;
            assert_numeric_spec(&self.object, &asset_definition)?;

            assert_can_mint(&asset_definition, state_transaction)?;
            let asset = state_transaction
                .world
                .asset_or_insert(&asset_id, Numeric::ZERO)?;
            let quantity = &mut asset.value;
            *quantity = quantity
                .checked_add(self.object)
                .ok_or(MathError::Overflow)?;

            #[allow(clippy::float_arithmetic)]
            {
                #[cfg(feature = "telemetry")]
                state_transaction
                    .telemetry
                    .observe_tx_amount(self.object.to_f64());
                state_transaction
                    .world
                    .increase_asset_total_amount(&asset_id.definition, self.object)?;
            }

            state_transaction
                .world
                .emit_events(Some(AssetEvent::Added(AssetChanged {
                    asset: asset_id,
                    amount: self.object,
                })));

            Ok(())
        }
    }

    impl Execute for Burn<Numeric, Asset> {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let asset_id = self.destination;

            let asset_definition = state_transaction
                .world
                .asset_definition(&asset_id.definition)?;
            assert_numeric_spec(&self.object, &asset_definition)?;

            let asset = state_transaction
                .world
                .assets
                .get_mut(&asset_id)
                .ok_or_else(|| FindError::Asset(asset_id.clone().into()))?;
            let quantity = &mut asset.value;
            *quantity = quantity
                .checked_sub(self.object)
                .ok_or(MathError::NotEnoughQuantity)?;

            if asset.value.is_zero() {
                assert!(state_transaction
                    .world
                    .assets
                    .remove(asset_id.clone())
                    .is_some());
            }

            #[allow(clippy::float_arithmetic)]
            {
                #[cfg(feature = "telemetry")]
                state_transaction
                    .telemetry
                    .observe_tx_amount(self.object.to_f64());
                state_transaction
                    .world
                    .decrease_asset_total_amount(&asset_id.definition, self.object)?;
            }

            state_transaction
                .world
                .emit_events(Some(AssetEvent::Removed(AssetChanged {
                    asset: asset_id.clone(),
                    amount: self.object,
                })));

            Ok(())
        }
    }

    impl Execute for Transfer<Asset, Numeric, Account> {
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let source_id = self.source;
            let destination_id =
                AssetId::new(source_id.definition.clone(), self.destination.clone());

            let asset_definition = state_transaction
                .world
                .asset_definition(&source_id.definition)?;
            assert_numeric_spec(&self.object, &asset_definition)?;

            {
                let asset = state_transaction
                    .world
                    .assets
                    .get_mut(&source_id)
                    .ok_or_else(|| FindError::Asset(source_id.clone().into()))?;
                let quantity = &mut asset.value;
                *quantity = quantity
                    .checked_sub(self.object)
                    .ok_or(MathError::NotEnoughQuantity)?;
                if asset.value.is_zero() {
                    assert!(state_transaction
                        .world
                        .assets
                        .remove(source_id.clone())
                        .is_some());
                }
            }

            let destination_asset = state_transaction
                .world
                .asset_or_insert(&destination_id, Numeric::ZERO)?;
            {
                let quantity = &mut destination_asset.value;
                *quantity = quantity
                    .checked_add(self.object)
                    .ok_or(MathError::Overflow)?;
            }

            #[allow(clippy::float_arithmetic)]
            #[cfg(feature = "telemetry")]
            state_transaction
                .telemetry
                .observe_tx_amount(self.object.to_f64());

            state_transaction.world.emit_events([
                AssetEvent::Removed(AssetChanged {
                    asset: source_id,
                    amount: self.object,
                }),
                AssetEvent::Added(AssetChanged {
                    asset: destination_id,
                    amount: self.object,
                }),
            ]);

            Ok(())
        }
    }

    /// Assert that asset type is Numeric and that it satisfy asset definition spec
    pub(crate) fn assert_numeric_spec(
        object: &Numeric,
        asset_definition: &AssetDefinition,
    ) -> Result<NumericSpec, Error> {
        let object_spec = NumericSpec::fractional(object.scale());
        let asset_spec = asset_definition.spec;
        asset_spec.check(object).map_err(|_| {
            TypeError::from(Mismatch {
                expected: asset_spec,
                actual: object_spec,
            })
        })?;
        Ok(asset_spec)
    }

    /// Assert that this asset is `mintable`.
    fn assert_can_mint(
        asset_definition: &AssetDefinition,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        match asset_definition.mintable {
            Mintable::Infinitely => Ok(()),
            Mintable::Not => Err(Error::Mintability(MintabilityError::MintUnmintable)),
            Mintable::Once => {
                let asset_definition_id = asset_definition.id.clone();
                let asset_definition = state_transaction
                    .world
                    .asset_definition_mut(&asset_definition_id)?;
                forbid_minting(asset_definition)?;
                state_transaction.world.emit_events(Some(
                    AssetDefinitionEvent::MintabilityChanged(asset_definition_id),
                ));
                Ok(())
            }
        }
    }
}

/// Asset-related query implementations.
pub mod query {
    use eyre::Result;
    use iroha_data_model::{
        asset::{Asset, AssetDefinition},
        query::{dsl::CompoundPredicate, error::QueryExecutionFail as Error},
    };

    use super::*;
    use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

    impl ValidQuery for FindAssets {
        #[metrics(+"find_assets")]
        fn execute(
            self,
            filter: CompoundPredicate<Asset>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Asset>, Error> {
            Ok(state_ro
                .world()
                .assets_iter()
                .filter(move |&asset| filter.applies(asset))
                .cloned())
        }
    }
    impl ValidQuery for FindAssetsDefinitions {
        #[metrics(+"find_asset_definitions")]
        fn execute(
            self,
            filter: CompoundPredicate<AssetDefinition>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = AssetDefinition>, Error> {
            Ok(state_ro
                .world()
                .asset_definitions_iter()
                .filter(move |&asset_definition| filter.applies(asset_definition))
                .cloned())
        }
    }
}
