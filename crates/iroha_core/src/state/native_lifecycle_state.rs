//! Native selected-runtime evidence in the existing ordinary-write witness.
//!
//! The opaque witness owner retains the key/value backing with its other newly
//! admitted writes. Baseline selection emits no anchor. Guest recorders cannot
//! claim this binary namespace, and a collision is never silently overwritten.

use super::*;
use iroha_data_model::{
    execution_witness::TRIGGER_LIFECYCLE_SELECTION_WITNESS_KEY_V1 as KEY,
    sumeragi_finality::lifecycle_runtime::{
        LIFECYCLE_RUNTIME_ANCHOR_BYTES, lifecycle_runtime_anchor,
    },
};

impl StateBlock<'_> {
    pub(super) fn lifecycle_runtime_anchor(
        &self,
    ) -> Result<Option<[u8; LIFECYCLE_RUNTIME_ANCHOR_BYTES]>, String> {
        let Some(execution_selection) = self.lifecycle_execution_selection()? else {
            return Ok(None);
        };
        let post_selection = self
            .world
            .runtime_selection
            .get()
            .id()
            .map_err(|error| error.to_string())?;
        Ok(Some(lifecycle_runtime_anchor(
            self.network_id,
            self._curr_block.height().get(),
            execution_selection,
            post_selection,
        )))
    }

    pub(super) fn verify_lifecycle_runtime_witness(
        &self,
        witness: &ExecWitness,
    ) -> Result<(), String> {
        let expected = self.lifecycle_runtime_anchor()?;
        let mut entries = witness.writes.iter().filter(|entry| entry.key == KEY);
        match (expected, entries.next()) {
            (None, None) => Ok(()),
            (Some(value), Some(entry))
                if entry.key == KEY && entry.value == value && entries.next().is_none() =>
            {
                Ok(())
            }
            _ => {
                Err("execution witness differs from its native lifecycle runtime selection".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

    fn header(chain: &CertifiedTestChain) -> BlockHeader {
        BlockHeader::new(
            std::num::NonZeroU64::new(3).unwrap(),
            Some(chain.committed(2).block().hash()),
            None,
            3_000,
            0,
        )
    }

    #[test]
    fn lifecycle_native_anchor_is_successor_only_and_binds_both_actual_selections() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit_at(2_000, vec![]);
        {
            let block = chain.state().block(header(&chain));
            assert_eq!(block.lifecycle_runtime_anchor().unwrap(), None);
            assert!(
                block
                    .verify_lifecycle_runtime_witness(&ExecWitness::default())
                    .is_ok()
            );
            let offered = ExecWitness {
                writes: vec![ExecKv {
                    key: KEY.to_vec(),
                    value: vec![0; LIFECYCLE_RUNTIME_ANCHOR_BYTES],
                }],
                ..ExecWitness::default()
            };
            assert!(block.verify_lifecycle_runtime_witness(&offered).is_err());
        }
        let catalog = crate::runtime::release_catalog().unwrap();
        chain.setup_world_at(2_001, |tx| {
            for (id, value) in &catalog.definitions {
                tx.world.runtime_definitions.insert(*id, value.clone());
            }
            *tx.world.runtime_selection.get_mut() = catalog.lifecycle.clone();
        });
        let mut block = chain.state().block(header(&chain));
        let frozen = catalog.lifecycle.id().unwrap();
        let value = block.lifecycle_runtime_anchor().unwrap().unwrap();
        assert_eq!(
            value,
            lifecycle_runtime_anchor(chain.network_id(), 3, frozen, frozen)
        );
        let witness = ExecWitness {
            writes: vec![ExecKv {
                key: KEY.to_vec(),
                value: value.to_vec(),
            }],
            ..ExecWitness::default()
        };
        assert!(block.verify_lifecycle_runtime_witness(&witness).is_ok());
        assert!(
            block
                .verify_lifecycle_runtime_witness(&ExecWitness::default())
                .is_err()
        );
        let mut duplicate = witness.clone();
        duplicate.writes.push(duplicate.writes[0].clone());
        assert!(block.verify_lifecycle_runtime_witness(&duplicate).is_err());
        *block.world.runtime_selection.get_mut() = catalog.baseline.clone();
        assert!(block.verify_lifecycle_runtime_witness(&witness).is_err());
        assert_eq!(
            block.lifecycle_runtime_anchor().unwrap().unwrap(),
            lifecycle_runtime_anchor(
                chain.network_id(),
                3,
                frozen,
                catalog.baseline.id().unwrap()
            )
        );
    }
    #[test]
    fn lifecycle_native_branch_matches_existing_ordinary_write_root() {
        use iroha_data_model::sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofV1;

        let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"native lifecycle proof oracle",
        )));
        let selection = crate::runtime::release_catalog()
            .unwrap()
            .lifecycle
            .id()
            .unwrap();
        let mut witness = ExecWitness {
            reads: vec![ExecKv {
                key: b"incidental read".to_vec(),
                value: vec![99],
            }],
            writes: vec![ExecKv {
                key: KEY.to_vec(),
                value: lifecycle_runtime_anchor(network, 2, selection, selection).to_vec(),
            }],
            ..ExecWitness::default()
        };
        for index in 0..64_u64 {
            witness.writes.push(ExecKv {
                key: index.to_le_bytes().to_vec(),
                value: index.wrapping_mul(13).to_le_bytes().to_vec(),
            });
        }
        // Last-write semantics and incidental reads must match the existing
        // ordinary root implementation, independently of the new branch builder.
        witness.writes.push(ExecKv {
            key: 3_u64.to_le_bytes().to_vec(),
            value: vec![42],
        });
        let pool = iroha_allocation::AllocationBudget::new(1_000_000);
        let retained = LifecycleRuntimeProofV1::from_witness(&witness, network, 2, &pool).unwrap();
        let expected = crate::exec_witness::roots::post_state_from_witness(&witness);
        assert_eq!(retained.proof().computed_root(network), Some(expected));
        assert!(retained.proof().verify(network, 2, selection, expected));
        witness.writes[1].value.push(1);
        assert!(!retained.proof().verify(
            network,
            2,
            selection,
            crate::exec_witness::roots::post_state_from_witness(&witness)
        ));
        drop(retained);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn lifecycle_native_anchor_cannot_be_supplied_by_predecessor_recorder() {
        crate::state::native_capture_fixture::with_native_capture_source(
            false,
            |_, mut block, _recording, mut source, _| {
                crate::state::native_capture_fixture::seal_native_source(&mut block, &mut source)
                    .unwrap();
                let supplied = ExecWitness {
                    writes: vec![ExecKv {
                        key: KEY.to_vec(),
                        value: vec![0; LIFECYCLE_RUNTIME_ANCHOR_BYTES],
                    }],
                    ..ExecWitness::default()
                };
                let error = block.retain_quantity_source_witness(supplied).unwrap_err();
                assert!(
                    matches!(error, crate::execution_attempt::ExecutionAttemptError::Rejected(ref message)
                if message.contains("native lifecycle witness family")),
                    "{error:?}"
                );
                block.capture_exec_witness().unwrap();
                assert!(
                    block
                        .exec_witness
                        .as_ref()
                        .unwrap()
                        .writes
                        .iter()
                        .all(|write| write.key != KEY)
                );
            },
        );
    }
}
