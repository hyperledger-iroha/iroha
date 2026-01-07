//! This module contains [`Nft`] instructions and queries implementations.

use iroha_telemetry::metrics;

use super::prelude::*;

/// ISI module contains all instructions related to NFTs:
/// - register/unregister NFT
/// - update metadata
/// - transfer, etc.
pub mod isi {
    use iroha_data_model::{IntoKeyValue, isi::error::RepetitionError, query::error::FindError};
    use iroha_telemetry::metrics;

    use super::*;
    use crate::smartcontracts::isi::account_admission::ensure_receiving_account;

    impl Execute for Register<Nft> {
        #[metrics(+"register_nft")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let nft = self.object().clone().build(authority);
            let (nft_id, nft_value) = nft.clone().into_key_value();

            if state_transaction.world.nft(&nft_id).is_ok() {
                return Err(RepetitionError {
                    instruction: InstructionType::Register,
                    id: IdBox::NftId(nft_id),
                }
                .into());
            }
            let _ = state_transaction.world.domain(nft_id.domain())?;

            state_transaction.world.nfts.insert(nft_id, nft_value);

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
            let nft_id = self.object().clone();

            state_transaction
                .world
                .nfts
                .remove(nft_id.clone())
                .ok_or_else(|| FindError::Nft(nft_id.clone()))?;
            let _ = state_transaction.world.domain(nft_id.domain())?;

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
            let SetKeyValue {
                object: nft_id,
                key,
                value,
            } = self;
            crate::smartcontracts::limits::enforce_json_size(
                state_transaction,
                &value,
                "max_metadata_value_bytes",
                crate::smartcontracts::limits::DEFAULT_JSON_LIMIT,
            )?;

            state_transaction
                .world
                .nft_mut(&nft_id)
                .map_err(Error::from)
                .map(|nft| nft.content.insert(key.clone(), value.clone()))?;

            state_transaction
                .world
                .emit_events(Some(NftEvent::MetadataInserted(MetadataChanged {
                    target: nft_id,
                    key,
                    value,
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
            let nft_id = self.object().clone();

            let value = state_transaction.world.nft_mut(&nft_id).and_then(|nft| {
                nft.content
                    .remove(self.key().as_ref())
                    .ok_or_else(|| FindError::MetadataKey(self.key().clone()))
            })?;

            state_transaction
                .world
                .emit_events(Some(NftEvent::MetadataRemoved(MetadataChanged {
                    target: nft_id,
                    key: self.key().clone(),
                    value,
                })));

            Ok(())
        }
    }

    // centralized in smartcontracts::limits

    impl Execute for Transfer<Account, NftId, Account> {
        #[metrics(+"transfer_nft")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let Transfer {
                source,
                object,
                destination,
            } = self;

            state_transaction.world.account(&source)?;
            let _created =
                ensure_receiving_account(authority, &destination, None, state_transaction)?;

            let nft = state_transaction.world.nft_mut(&object)?;

            if nft.owned_by != source {
                return Err(Error::InvariantViolation(
                    format!("Can't transfer NFT {object} since {source} doesn't own it",).into(),
                ));
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

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_data_model::query::error::FindError;
        use iroha_test_samples::ALICE_ID;

        use super::*;
        use crate::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };

        fn new_dummy_block() -> crate::block::CommittedBlock {
            let (leader_public_key, leader_private_key) =
                iroha_crypto::KeyPair::random().into_parts();
            let peer_id = crate::PeerId::new(leader_public_key);
            let topology = crate::sumeragi::network_topology::Topology::new(vec![peer_id]);
            ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
                h.set_height(NonZeroU64::new(1).unwrap());
            })
            .commit(&topology)
            .unpack(|_| {})
            .unwrap()
        }

        #[test]
        fn register_nft_rejects_missing_domain() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let nft_id: NftId = "nft1#wonderland".parse().unwrap();
            let err = Register::nft(Nft::new(nft_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .expect_err("missing domain should be rejected");

            assert!(
                matches!(err, Error::Find(FindError::Domain(id)) if id == *nft_id.domain()),
                "expected missing-domain error, got {err:?}"
            );
        }

        #[test]
        fn unregister_nft_rejects_missing_domain() {
            let mut world = World::default();
            let nft_id: NftId = "nft1#wonderland".parse().unwrap();
            let nft = Nft::new(nft_id.clone(), Metadata::default()).build(&ALICE_ID);
            let (id, value) = nft.into_key_value();
            world.nfts.insert(id, value);

            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let err = Unregister::nft(nft_id.clone())
                .execute(&ALICE_ID, &mut stx)
                .expect_err("missing domain should be rejected");

            assert!(
                matches!(err, Error::Find(FindError::Domain(id)) if id == *nft_id.domain()),
                "expected missing-domain error, got {err:?}"
            );
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
            use iroha_data_model::query::dsl::EvaluatePredicate;

            Ok(state_ro.world().nfts_iter().filter_map(move |entry| {
                let details = entry.value().clone().into_inner();
                let nft = Nft {
                    id: entry.id().clone(),
                    content: details.content,
                    owned_by: details.owned_by,
                };
                filter.applies(&nft).then_some(nft)
            }))
        }
    }

    #[cfg(test)]
    mod tests {
        use core::num::NonZeroU64;

        use iroha_primitives::json::Json;
        use iroha_test_samples::ALICE_ID;

        use super::*;
        use crate::{
            block::ValidBlock,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };

        fn new_dummy_block() -> crate::block::CommittedBlock {
            let (leader_public_key, leader_private_key) =
                iroha_crypto::KeyPair::random().into_parts();
            let peer_id = crate::PeerId::new(leader_public_key);
            let topology = crate::sumeragi::network_topology::Topology::new(vec![peer_id]);
            ValidBlock::new_dummy_and_modify_header(&leader_private_key, |h| {
                h.set_height(NonZeroU64::new(1).unwrap());
            })
            .commit(&topology)
            .unpack(|_| {})
            .unwrap()
        }

        #[test]
        fn find_nfts_applies_predicate() {
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);

            let block = new_dummy_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();

            let domain_id: DomainId = "wonderland".parse().unwrap();
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let nft1_id: NftId = "nft1#wonderland".parse().unwrap();
            let nft2_id: NftId = "nft2#wonderland".parse().unwrap();
            Register::nft(Nft::new(nft1_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            Register::nft(Nft::new(nft2_id.clone(), Metadata::default()))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            let rarity_key: Name = "rarity".parse().unwrap();
            SetKeyValue::nft(nft1_id.clone(), rarity_key.clone(), Json::from("rare"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();
            SetKeyValue::nft(nft2_id.clone(), rarity_key, Json::from("common"))
                .execute(&ALICE_ID, &mut stx)
                .unwrap();

            stx.apply();
            state_block.commit().unwrap();

            let view = state.view();
            let predicate = CompoundPredicate::<Nft>::build(|p| p.equals("content.rarity", "rare"));
            let results: Vec<_> = FindNfts
                .execute(predicate, &view)
                .unwrap()
                .map(|nft| nft.id)
                .collect();
            assert_eq!(results, vec![nft1_id]);
        }
    }
}
