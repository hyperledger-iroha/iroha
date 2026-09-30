//! One authoritative parameter carrier for builder-owned custom genesis normalization.

use super::*;
use iroha_core::block::{check_genesis_block, check_genesis_block_intents};
use iroha_data_model::transaction::SignedTransaction;

/// Normalize and re-sign the builder-owned carrier while preserving ordinary source payloads.
pub(super) fn normalize_genesis_parameters(
    source: &GenesisBlock,
    genesis_isi: &[Vec<InstructionBox>],
    genesis_post_topology_isi: &[Vec<InstructionBox>],
    consensus_handshake_meta: &Parameter,
    genesis_key_pair: &KeyPair,
    da_proof_policies: Option<&DaProofPolicyBundle>,
    confidential_policy_hash: Option<[u8; 32]>,
) -> GenesisBlock {
    let authority = AccountId::new(genesis_key_pair.public_key().clone());
    // Construction sources may omit outputs. A supplied complete envelope must
    // retain canonical successful results, even though normalization re-executes it.
    if source.0.has_results() {
        check_genesis_block(&source.0, &authority)
    } else {
        check_genesis_block_intents(&source.0, &authority)
    }
    .expect("original custom genesis must authenticate before normalization");
    let overrides = genesis_isi
        .iter()
        .chain(genesis_post_topology_isi)
        .flat_map(|batch| batch.iter())
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .map(|set| set.inner().clone());
    let source_transactions = source
        .0
        .external_transactions()
        .cloned()
        .collect::<Vec<_>>();
    let transactions = normalize_parameter_transactions(
        &source_transactions,
        overrides,
        consensus_handshake_meta,
        genesis_key_pair,
    );
    let external_merkle: iroha_crypto::MerkleTree<
        iroha_data_model::transaction::TransactionEntrypoint,
    > = transactions
        .iter()
        .map(iroha_data_model::transaction::SignedTransaction::hash_as_entrypoint)
        .collect();
    let mut header = source.0.header();
    header.merkle_root = external_merkle.root();
    let da_proof_policies = da_proof_policies
        .cloned()
        .or_else(|| source.0.da_proof_policies().cloned());
    header.set_da_proof_policies_hash(da_proof_policies.as_ref().map(iroha_crypto::HashOf::new));
    if let Some(zk_policy_hash) = confidential_policy_hash {
        let mut confidential_features = header
            .confidential_features()
            .unwrap_or(iroha_data_model::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST);
        confidential_features.zk_policy_hash = Some(zk_policy_hash);
        header.set_confidential_features(Some(confidential_features));
    }
    let signer_index = source
        .0
        .signatures()
        .next()
        .map(|sig| sig.index())
        .unwrap_or(0);
    let proposal_signature = iroha_data_model::block::BlockSignature::new(
        signer_index,
        iroha_crypto::SignatureOf::try_from_hash(genesis_key_pair.private_key(), header.hash())
            .expect("sign normalized resultless genesis header"),
    );
    let mut proposal =
        iroha_data_model::block::SignedBlock::presigned(proposal_signature, header, transactions);
    proposal.set_da_commitments(source.0.da_commitments().cloned());
    proposal.set_da_proof_policies(da_proof_policies);
    proposal.set_da_pin_intents(source.0.da_pin_intents().cloned());
    GenesisBlock(proposal)
}

/// Match the exact parameter slot without conflating values or unrelated custom IDs.
fn same_slot(left: &Parameter, right: &Parameter) -> bool {
    use core::mem::discriminant;
    match (left, right) {
        (Parameter::Sumeragi(a), Parameter::Sumeragi(b)) => discriminant(a) == discriminant(b),
        (Parameter::Block(a), Parameter::Block(b)) => discriminant(a) == discriminant(b),
        (Parameter::Transaction(a), Parameter::Transaction(b)) => {
            discriminant(a) == discriminant(b)
        }
        (Parameter::Executor(a), Parameter::Executor(b)) => discriminant(a) == discriminant(b),
        (Parameter::SmartContract(a), Parameter::SmartContract(b)) => {
            discriminant(a) == discriminant(b)
        }
        (Parameter::Custom(a), Parameter::Custom(b)) => a.id() == b.id(),
        _ => false,
    }
}

fn upsert_parameter(parameters: &mut Vec<Parameter>, next: Parameter) {
    if let Some(previous) = parameters
        .iter_mut()
        .find(|previous| same_slot(previous, &next))
    {
        *previous = next;
    } else {
        parameters.push(next);
    }
}

/// Normalize builder-owned parameters before any ordinary genesis instructions.
/// Existing slots retain deterministic first-occurrence order; later supplied values win.
/// No default snapshot is expanded into retired executor/transaction parameter instructions.
fn normalize_parameter_transactions(
    transactions: &[SignedTransaction],
    overrides: impl IntoIterator<Item = Parameter>,
    handshake: &Parameter,
    key: &KeyPair,
) -> Vec<SignedTransaction> {
    let source = transactions
        .iter()
        .map(|transaction| {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                panic!("genesis normalization requires instruction-only sources");
            };
            instructions
        })
        .collect::<Vec<_>>();
    let carrier = usize::from(source.first().is_some_and(|instructions| {
        instructions.len() == 1
            && instructions[0]
                .as_any()
                .is::<iroha_data_model::isi::Upgrade>()
    }));
    assert!(
        carrier < transactions.len(),
        "genesis requires a parameter carrier after its optional executor upgrade"
    );
    let mut effective = Vec::new();
    for parameter in source
        .iter()
        .flat_map(|instructions| instructions.iter())
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .map(|set| set.inner().clone())
        .chain(overrides)
    {
        if !same_slot(&parameter, handshake) {
            upsert_parameter(&mut effective, parameter);
        }
    }
    // The exact newly computed handshake follows the parameters it commits to.
    effective.push(handshake.clone());
    let mut normalized = Vec::with_capacity(transactions.len());
    for (index, (transaction, instructions)) in transactions.iter().zip(source).enumerate() {
        let mut replacement = if index == carrier {
            effective
                .iter()
                .cloned()
                .map(|parameter| SetParameter::new(parameter).into())
                .collect::<Vec<InstructionBox>>()
        } else {
            Vec::new()
        };
        replacement.extend(
            instructions
                .iter()
                .filter(|instruction| !instruction.as_any().is::<SetParameter>())
                .cloned(),
        );
        if instructions.iter().eq(replacement.iter()) {
            normalized.push(transaction.clone());
            continue;
        }
        // These checks also cover a removed parameter-only source. Removing a
        // signed source is a mutation, even when no replacement bytes are emitted.
        assert_eq!(
            transaction.authority().try_signatory(),
            Some(key.public_key()),
            "cannot normalize parameters in a genesis transaction signed by another authority"
        );
        assert!(
            transaction.attachments().is_none(),
            "cannot normalize parameters inside a proof-attached genesis transaction"
        );
        assert!(
            transaction.multisig_signatures().is_none(),
            "cannot normalize parameters inside a multisig genesis transaction"
        );
        assert_eq!(
            transaction.domain(),
            &iroha_data_model::transaction::TransactionDomain::Genesis,
            "cannot normalize a non-genesis parameter source"
        );
        transaction
            .verify_signature()
            .expect("original genesis parameter source signature must verify");
        if !replacement.is_empty() {
            let payload = norito::codec::encode_adaptive(transaction.payload());
            normalized.push(
                iroha_data_model::transaction::TransactionBuilder::decode_genesis_payload(&payload)
                    .expect("canonical genesis source payload must decode")
                    .with_instructions(replacement)
                    .try_sign(key.private_key())
                    .expect("re-sign original genesis payload after parameter normalization"),
            );
        }
    }
    normalized
}

#[cfg(test)]
#[path = "genesis_parameter_normalization_tests.rs"]
mod tests;
