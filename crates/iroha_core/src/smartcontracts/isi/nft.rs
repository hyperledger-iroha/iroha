//! This module contains [`Nft`] instructions and queries implementations.

use iroha_telemetry::metrics;

use super::prelude::*;

impl Registrable for NewNft {
    type Target = Nft;

    #[inline]
    fn build(self, authority: &AccountId) -> Self::Target {
        Self::Target {
            id: self.id,
            content: self.content,
            owned_by: authority.clone(),
        }
    }
}

/// ISI module contains all instructions related to NFTs:
/// - register/unregister NFT
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use iroha_data_model::{isi::error::RepetitionError, query::error::FindError};
    use iroha_telemetry::metrics;

    use super::*;

    impl Execute for Register<Nft> {
        #[metrics(+"register_nft")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft = self.object.build(authority);
            let nft_id = nft.id.clone();

            if state_transaction.world.nft(&nft_id).is_ok() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::NftId(nft_id),
                }
                .into());
            }
            state_transaction
                .world
                .domain(&nft_id.domain)
                .expect("INTERNAL BUG: Can't find domain of NFT to register");

            state_transaction.world.nfts.insert(nft_id, nft.clone());

            state_transaction
                .world
                .emit_events(Some(DomainEvent::Nft(NftEvent::Created(nft))));

            Ok(())
        }
    }

    impl Execute for Unregister<Nft> {
        #[metrics(+"unregister_nft")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft_id = self.object;

            state_transaction
                .world
                .nfts
                .remove(nft_id.clone())
                .ok_or_else(|| FindError::Nft(nft_id.clone()))?;
            state_transaction
                .world
                .domain(&nft_id.domain)
                .expect("INTERNAL BUG: Can't find domain of NFT to unregister");

            state_transaction
                .world
                .emit_events(Some(DomainEvent::Nft(NftEvent::Deleted(nft_id))));

            Ok(())
        }
    }

    impl Execute for SetKeyValue<Nft> {
        #[metrics(+"set_nft_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft_id = self.object;

            state_transaction
                .world
                .nft_mut(&nft_id)
                .map_err(Error::from)
                .map(|nft| nft.content.insert(self.key.clone(), self.value.clone()))?;

            state_transaction
                .world
                .emit_events(Some(NftEvent::MetadataInserted(MetadataChanged {
                    target: nft_id,
                    key: self.key,
                    value: self.value,
                })));

            Ok(())
        }
    }

    impl Execute for RemoveKeyValue<Nft> {
        #[metrics(+"remove_nft_key_value")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft_id = self.object;

            let value = state_transaction.world.nft_mut(&nft_id).and_then(|nft| {
                nft.content
                    .remove(&self.key)
                    .ok_or_else(|| FindError::MetadataKey(self.key.clone()))
            })?;

            state_transaction
                .world
                .emit_events(Some(NftEvent::MetadataRemoved(MetadataChanged {
                    target: nft_id,
                    key: self.key,
                    value,
                })));

            Ok(())
        }
    }

    impl Execute for Transfer<Account, NftId, Account> {
        #[metrics(+"transfer_nft")]
        fn execute(
            self,
            _authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let Transfer {
                source,
                object,
                destination,
            } = self;

            state_transaction.world.account(&source)?;
            state_transaction.world.account(&destination)?;

            let nft = state_transaction.world.nft_mut(&object)?;

            if nft.owned_by != source {
                return Err(Error::InvariantViolation(format!(
                    "Can't transfer NFT {object} since {source} doesn't own it",
                )));
            }

            nft.owned_by = destination.clone();
            state_transaction
                .world
                .emit_events(Some(NftEvent::OwnerChanged(NftOwnerChanged {
                    nft: object,
                    new_owner: destination,
                })));

            Ok(())
        }
    }
}

/// NFT-related query implementations.
pub mod query {
    use eyre::Result;
    use iroha_data_model::query::{dsl::CompoundPredicate, error::QueryExecutionFail as Error};

    use super::*;
    use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

    impl ValidQuery for FindNfts {
        #[metrics(+"find_nfts")]
        fn execute(
            self,
            filter: CompoundPredicate<Nft>,
            state_ro: &impl StateReadOnly,
        ) -> Result<impl Iterator<Item = Nft>, Error> {
            Ok(state_ro
                .world()
                .nfts_iter()
                .filter(move |&nft| filter.applies(nft))
                .cloned())
        }
    }
}
