//! Original Executor body, effect queue and fee-work regression for the dormant candidate.
//!
//! Signed genesis supplies the ordinary committed monetary/scope policies. Candidate code and
//! selection live only in a disposable component block. The existing finite component source
//! owner proves direct Executor custody, not canonical producer/finality qualification,
//! activation, Network admission, paid settlement, replay or publication of the unsupported profile.

use super::*;
use crate::{
    runtime::generic_carriers::execution::tests::{fixture, program, source},
    state::{StateReadOnly, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_data_model::{
    block::BlockHeader,
    runtime::{
        carrier::{CarrierStorageKeyV1, CarrierValueV1, encode_carrier_key_v1, encode_carrier_v1},
        definition::{CarrierNodeV1, CarrierSchemaV1},
    },
    transaction::signed::TransactionEntrypoint,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use ivm_abi::{encoding::wide, instruction::wide as isa};

fn write_program(reject_after_write: bool, key: &[u8]) -> Vec<u8> {
    let literal =
        ivm_abi::pointer_abi::encode_tlv(ivm::pointer_abi::PointerType::NoritoBytes, key).unwrap();
    let mut image = program(&[]);
    let padding = (4 - ((24 + literal.len()) % 4)) % 4;
    image.extend_from_slice(&ivm_abi::metadata::LITERAL_SECTION_MAGIC);
    image.extend_from_slice(&1_u32.to_le_bytes());
    image.extend_from_slice(&(padding as u32).to_le_bytes());
    image.extend_from_slice(&(literal.len() as u32).to_le_bytes());
    image.extend_from_slice(
        &ivm_abi::metadata::encode_literal_descriptor(
            ivm_abi::metadata::LiteralKindV1::PointerTlv,
            24,
        )
        .unwrap()
        .to_le_bytes(),
    );
    image.extend_from_slice(&literal);
    image.extend(std::iter::repeat_n(0, padding));
    for word in [
        wide::encode_ri(isa::arithmetic::ADDI, 12, 10, 0),
        wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
        wide::encode_literal(isa::memory::LDLIT, 11, 0),
        wide::encode_scall(ivm::syscalls::SYSCALL_STATE_SET),
        wide::encode_ri(
            isa::arithmetic::ADDI,
            10,
            0,
            if reject_after_write { 1 } else { 0 },
        ),
        wide::encode_halt(),
    ] {
        image.extend_from_slice(&word.to_le_bytes());
    }
    image
}

#[test]
fn executor_keeps_actual_carrier_work_on_semantic_failure_and_only_applies_successful_writes() {
    let schema = CarrierSchemaV1 {
        nodes: vec![CarrierNodeV1::U16],
    };
    let bare_key = encode_carrier_key_v1(&schema, &CarrierValueV1::U16(7)).unwrap();
    let candidates = [false, true].map(|reject_after_write| {
        let (unused_state, instruction, candidate) =
            fixture(write_program(reject_after_write, &bare_key));
        drop(unused_state);
        (reject_after_write, instruction, candidate)
    });
    // Use the existing signed-genesis fixture's authored fee policy; no runtime exemption,
    // node-local policy replacement or reduced execution budget is introduced here.
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_key = ALICE_KEYPAIR.clone();
    let chain = CertifiedTestChain::start(config).unwrap();
    let state = chain.state();
    let mut observed = Vec::new();
    for (reject_after_write, instruction, candidate) in candidates {
        let header = BlockHeader::new(
            2.try_into().unwrap(),
            Some(chain.committed(1).block_hash()),
            None,
            2_000,
            0,
        );
        let mut block = state.block(header);
        candidate.install(&mut block);
        let original = {
            let reader = block.transaction();
            TransactionEntrypoint::External(source(&reader, &instruction))
        };
        // Retain this actual signed source's finite invocation before opening the business
        // overlay. The existing component owner grants no Network/publication authority.
        let mut transaction = block.transaction_for_fastpq_testing(iroha_crypto::Hash::from(
            original.execution_call_hash(),
        ));
        let TransactionEntrypoint::External(signed) = &original else {
            unreachable!("the component retains its original external signed source")
        };
        let storage = crate::smartcontracts::ivm::host::generic_carrier::tests::table(
            &mut transaction,
            false,
            true,
            1,
        );
        transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        // This component owns the exact signed input and its direct ordinal. The Executor
        // independently checks network, source hash and signature, then opens its real owners.
        transaction.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
        transaction.current_entrypoint_index = Some(0);
        let native_gas = isi_gas::meter_instruction(&instruction, &transaction.zk.gas).unwrap();
        let mut cache =
            IvmCache::with_prepared_contract_cache(64, transaction.prepared_contract_cache());
        let result =
            Executor::Initial.execute_transaction(&mut transaction, &ALICE_ID, signed, &mut cache);
        if reject_after_write {
            assert!(
                matches!(result, Err(ExecutionAttemptError::Rejected(_))),
                "{result:?}"
            );
        } else {
            result.unwrap();
        }
        assert!(transaction.last_tx_gas_used > native_gas);
        assert!(transaction.last_tx_gas_used < 10_000);
        observed.push(transaction.last_tx_gas_used);
        let key = CarrierStorageKeyV1 {
            storage,
            key: bare_key.clone(),
        };
        if reject_after_write {
            assert!(transaction.world.runtime_generic_values.get(&key).is_none());
            assert!(
                transaction
                    .take_execution_fee_settlement()
                    .unwrap()
                    .is_some(),
                "semantic failure must retain the original completed fee-work owner"
            );
        } else {
            assert_eq!(
                transaction
                    .world
                    .runtime_generic_values
                    .get(&key)
                    .unwrap()
                    .as_slice(),
                encode_carrier_v1(&schema, &CarrierValueV1::U16(256)).unwrap()
            );
        }
        transaction.require_storage_admission().unwrap();
        transaction.ensure_execution_ready().unwrap();
        drop(cache);
        drop(transaction);
        drop(block);
        assert!(
            state
                .world
                .runtime_generic_values
                .view()
                .get(&key)
                .is_none()
        );
    }
    assert_eq!(
        observed[0], observed[1],
        "same executed instructions retain the same work despite the final semantic return rejection"
    );
}

#[test]
fn executor_refuses_foreign_or_unsigned_carrier_sources_before_chargeable_work() {
    let (unused_state, instruction, candidate) = fixture(program(&[
        wide::encode_ri(isa::arithmetic::ADDI, 10, 0, 0),
        wide::encode_halt(),
    ]));
    drop(unused_state);
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_key = ALICE_KEYPAIR.clone();
    let chain = CertifiedTestChain::start(config).unwrap();
    let state = chain.state();
    for source_fault in ["outer hash", "ordinal", "signature"] {
        let header = BlockHeader::new(
            2.try_into().unwrap(),
            Some(chain.committed(1).block_hash()),
            None,
            2_000,
            0,
        );
        let mut block = state.block(header);
        candidate.install(&mut block);
        let mut transaction = block.transaction();
        transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
        let mut signed = source(&transaction, &instruction);
        transaction.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
        transaction.current_entrypoint_index = Some(0);
        match source_fault {
            "outer hash" => {
                transaction.current_network_entrypoint_hash =
                    Some(iroha_crypto::HashOf::from_untyped_unchecked(
                        iroha_crypto::Hash::new(b"foreign original carrier source"),
                    ));
            }
            "ordinal" => transaction.current_entrypoint_index = None,
            "signature" => {
                signed.set_signature(iroha_data_model::transaction::TransactionSignature(
                    iroha_crypto::SignatureOf::try_new(
                        iroha_test_samples::BOB_KEYPAIR.private_key(),
                        signed.payload(),
                    )
                    .unwrap(),
                ))
            }
            _ => unreachable!(),
        }
        let mut cache =
            IvmCache::with_prepared_contract_cache(64, transaction.prepared_contract_cache());
        let result =
            Executor::Initial.execute_transaction(&mut transaction, &ALICE_ID, &signed, &mut cache);
        assert!(
            matches!(result, Err(ExecutionAttemptError::Rejected(_))),
            "{source_fault}: {result:?}"
        );
        assert_eq!(transaction.last_tx_gas_used, 0, "{source_fault}");
        assert!(
            transaction
                .world
                .runtime_generic_values
                .iter()
                .next()
                .is_none()
        );
        assert!(
            transaction
                .take_execution_fee_settlement()
                .unwrap()
                .is_none()
        );
        transaction.require_storage_admission().unwrap();
        transaction.ensure_execution_ready().unwrap();
    }
}
