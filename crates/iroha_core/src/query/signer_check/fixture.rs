//! Shared exact signed native execution with fixed three-of-four BLS / RS16 test finality.
//!
//! Test-only World/frontier helpers do not prove a production consensus application-state root.
//! Closed native custody instructions execute through Core's initial executor; results are never
//! forged. Ordinary admission, fee settlement and genesis authentication are outside this fixture.
use crate::{smartcontracts::isi::triggers::set::SetReadOnly, state::State};
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    account::Account,
    block::{
        BlockHeader,
        builder::BlockBuilder,
        execution_output::{ExecutionOutputV1, NetworkExecutionOutputV1},
    },
    isi::{
        Log, Register, RegisterBox, Revoke, RevokeBox, Unregister, UnregisterBox,
        sorafs::{MutateSorafsFinalPromotionAccountCustody, MutateSorafsFinalPromotionAuthority},
    },
    permission::Permission,
    role::{Role, RoleId},
    transaction::{
        DataTriggerSequence, Executable, SignedTransaction, error::TransactionRejectionReason,
        signed::TransactionResult,
    },
};
#[cfg(test)]
use iroha_data_model::{
    account::AccountId,
    isi::InstructionBox,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_sccp::{SccpFinalizedBlockTestFixtureV1, sccp_finalize_taira_block_test_fixture_v1};
use std::sync::Arc;
#[cfg(test)]
use std::time::Duration;

pub(crate) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
}
#[cfg(test)]
pub(crate) fn sign(
    state: &Arc<State>,
    instruction: InstructionBox,
    seed: u8,
    now: u64,
) -> SignedTransaction {
    let key = key(seed);
    let mut builder = TransactionBuilder::new(
        *state.network_id_ref(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(now));
    builder
        .with_instructions([instruction])
        .try_sign(key.private_key())
        .unwrap()
}

// Execute actual typed instructions, then bind precisely their signed envelopes/results to
// the real deterministic test roster. Frontier/world helpers are explicitly test-only.
pub(crate) fn commit(
    state: &Arc<State>,
    finalized_blocks: &mut Vec<SccpFinalizedBlockTestFixtureV1>,
    now: u64,
    transactions: Vec<SignedTransaction>,
    membership: bool,
    finality: bool,
) -> Vec<bool> {
    // These fixtures exercise exactly the closed native custody transitions,
    // with no callback or arbitrary executor capable of producing omitted output.
    // They intentionally do not stand in for genesis or ordinary admission.
    let limits = {
        let view = state.view();
        assert!(matches!(
            &*view.world.executor,
            crate::executor::Executor::Initial
        ));
        assert!(view.world.triggers.triggers_iter().next().is_none());
        view.world
            .parameters
            .get()
            .block()
            .execution_output()
            .limits()
    };
    for transaction in &transactions {
        transaction.verify_signature().unwrap();
        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("native fixture");
        };
        assert!(
            instructions.iter().all(|instruction| {
                let instruction = instruction.as_any();
                instruction.is::<MutateSorafsFinalPromotionAuthority>()
                    || instruction.is::<MutateSorafsFinalPromotionAccountCustody>()
                    || instruction.is::<Log>()
                    // Existing adversarial cases execute observer/operator
                    // permission and account removal in the exact native cut.
                    || instruction.is::<Register<Role>>()
                    || matches!(instruction.downcast_ref::<RegisterBox>(), Some(RegisterBox::Role(_)))
                    || instruction.is::<Revoke<Permission, Account>>()
                    || instruction.is::<Revoke<RoleId, Account>>()
                    || instruction.is::<Revoke<Permission, Role>>()
                    || instruction.is::<RevokeBox>()
                    || instruction.is::<Unregister<Account>>()
                    || matches!(instruction.downcast_ref::<UnregisterBox>(), Some(UnregisterBox::Account(_)))
            }),
            "native fixture cannot omit callback or unrelated execution outputs"
        );
    }
    let header = BlockHeader::new(
        ((finalized_blocks.len() + 1) as u64).try_into().unwrap(),
        state.view().latest_block_hash(),
        None,
        now,
        0,
    );
    let mut builder = BlockBuilder::new(header.clone());
    let hashes = transactions
        .iter()
        .map(SignedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    let mut state_block = state.block(header);
    let mut outcomes = Vec::new();
    let mut outputs = Vec::new();
    for (entry_index, transaction) in transactions.into_iter().enumerate() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("native fixture");
        };
        let mut tx = state_block.transaction();
        let outer = transaction.hash_as_entrypoint();
        tx.current_network_entrypoint_hash = Some(outer);
        tx.tx_call_hash = Some(Hash::from(outer));
        tx.current_tx_hash = Some(transaction.hash());
        tx.current_entrypoint_index = Some(entry_index as u64);
        let executor = tx.world.executor.clone();
        let result = instructions.iter().try_for_each(|instruction| {
            tx.current_direct_final_promotion_operation_origin =
                crate::executor::Executor::direct_final_promotion_operation_origin(
                    &tx,
                    &transaction,
                    instruction,
                    true,
                );
            let result =
                executor.execute_instruction(&mut tx, transaction.authority(), instruction.clone());
            tx.current_direct_final_promotion_operation_origin = None;
            result
        });
        outcomes.push(result.is_ok());
        let result = match result {
            Ok(()) => {
                tx.apply();
                Ok(DataTriggerSequence::default())
            }
            Err(error) => {
                drop(tx);
                Err(TransactionRejectionReason::Validation(error))
            }
        };
        let input_index = builder.push_transaction(transaction).try_into().unwrap();
        outputs.push(ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
            input_index,
            result: TransactionResult::new(result),
            completions: Vec::new(),
        }));
    }
    let fragments = state_block.committed_fragment_count().try_into().unwrap();
    let mut signed = builder
        .try_build_with_signature(0, key(0xFE).private_key())
        .unwrap();
    signed
        .set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &limits,
        )
        .unwrap();
    state_block.commit_world_overlay_for_testing().unwrap();
    let finalized = sccp_finalize_taira_block_test_fixture_v1(&signed, finalized_blocks.last());
    let proof = &finalized.proof().finality_artifact;
    assert_eq!(proof.height_context.roster.len(), 4);
    assert_eq!(proof.commit_qc.signers.len(), 3);
    assert_eq!(
        proof.height_context.da_layout.encoding,
        iroha_data_model::block::consensus_v2::PayloadEncoding::ReedSolomon16
    );
    proof.verify().unwrap();
    state.kura().store_block(Arc::new(signed.clone())).unwrap();
    state.append_committed_block_header_for_tests(signed.header());
    if membership {
        state.record_committed_entrypoints_for_tests(
            hashes,
            (proof.height as usize).try_into().unwrap(),
        );
    }
    if finality {
        let receipt = state.kura().store_v2_finality_artifact(proof).unwrap();
        assert_eq!(receipt.height(), proof.height);
        assert_eq!(receipt.block_hash(), signed.hash());
    }
    finalized_blocks.push(finalized);
    outcomes
}
