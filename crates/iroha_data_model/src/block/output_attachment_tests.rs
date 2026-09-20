//! Atomic sole-output attachment and exact canonical wire budget controls.
use super::output_test_support as fixture;
use super::*;
use execution_output::ExecutionOutputV1;
use norito::codec::Encode;
fn rows(block: &SignedBlock) -> Vec<ExecutionOutputV1> {
    vec![
        fixture::network(0, Ok(Default::default())),
        fixture::simple_time(block, 0),
    ]
}
#[test]
fn borrowed_output_candidate_has_exact_canonical_signed_wire_layout() {
    let mut block = fixture::proposal(1);
    let outputs = rows(&block);
    fixture::install(&mut block, outputs, 3).unwrap();
    let candidate = SignedBlockOutputCandidate {
        signatures: OutputFieldRef(&block.signatures),
        payload: OutputFieldRef(&block.payload),
        result: block.result.as_ref().map(OutputFieldRef),
    };
    assert_eq!(
        norito::encode_canonical(&candidate).unwrap(),
        norito::encode_canonical(&block).unwrap()
    );
    assert_eq!(candidate.encode(), block.encode());
    assert_eq!(
        norito::canonical_frame_len(&candidate).unwrap() + 1,
        block.encode_wire().unwrap().len()
    );
    assert_eq!(
        <SignedBlockOutputCandidate<'_> as norito::NoritoSchema>::frame_name(),
        <SignedBlock as norito::NoritoSchema>::frame_name()
    );
}
#[test]
fn full_output_setter_rejects_size_shape_and_policy_without_mutation() {
    let mut block = fixture::proposal(1);
    let outputs = rows(&block);
    fixture::install(&mut block, outputs.clone(), 3).unwrap();
    let before = block.encode_wire().unwrap();
    let actual = before.len() as u64;
    for mutation in 0..4 {
        let mut limits = fixture::limits();
        let mut bad = outputs.clone();
        match mutation {
            0 => {
                limits.max_executed_wire_bytes = actual - 1;
                limits.max_total_output_bytes = actual - 1;
                limits.max_output_bytes = actual - 1;
            }
            1 => limits.max_output_bytes = 1,
            2 => {
                bad.remove(0);
            }
            _ => limits.max_outputs = 0,
        }
        let error = block
            .set_execution_outputs(
                bad,
                3,
                Default::default(),
                vec![],
                Default::default(),
                Default::default(),
                vec![],
                &limits,
            )
            .unwrap_err();
        if mutation == 0 {
            assert!(
                matches!(error,SetExecutionOutputsError::ExecutedWireTooLarge {actual: measured,limit} if measured == actual && limit == actual - 1)
            );
        }
        assert_eq!(block.encode_wire().unwrap(), before);
    }
    let mut exact = fixture::limits();
    exact.max_executed_wire_bytes = actual;
    exact.max_total_output_bytes = actual;
    exact.max_output_bytes = actual;
    block
        .set_execution_outputs(
            outputs,
            3,
            Default::default(),
            vec![],
            Default::default(),
            Default::default(),
            vec![],
            &exact,
        )
        .unwrap();
    assert_eq!(block.encode_wire().unwrap(), before);
    block.validate_execution_outputs(&exact).unwrap();
}
#[test]
fn cached_output_substitution_is_refused_without_repair_or_mutation() {
    let mut block = fixture::proposal(1);
    let outputs = rows(&block);
    fixture::install(&mut block, outputs, 3).unwrap();
    block.result.as_mut().unwrap().output_merkle = MerkleTree::default();
    let before = block.encode_wire().unwrap();
    assert!(
        block
            .validate_execution_outputs(&fixture::limits())
            .is_err()
    );
    assert!(
        block
            .network_execution_proof(&block.network_input_hashes().next().unwrap())
            .is_none()
    );
    assert_eq!(block.encode_wire().unwrap(), before);
}
#[test]
fn full_output_attachment_preserves_sccp_header_and_exact_proposal() {
    let mut block = fixture::proposal(1);
    block.set_sccp_commitment_root(Some([0x72; 32]));
    let proposal = block.clone();
    let outputs = rows(&block);
    fixture::install(&mut block, outputs, 9).unwrap();
    assert_eq!(block.header(), proposal.header());
    assert_eq!(block.hash(), proposal.hash());
    assert_eq!(block.canonical_resultless_proposal(), proposal);
    assert_eq!(
        block.canonical_proposal_wire_hash().unwrap(),
        proposal.canonical_proposal_wire_hash().unwrap()
    );
    assert_eq!(block.committed_fragment_count(), Some(9));
}
#[test]
fn final_wire_validation_accounts_for_post_attachment_signature_growth() {
    let mut block = fixture::proposal(1);
    let outputs = rows(&block);
    fixture::install(&mut block, outputs, 3).unwrap();
    let mut exact = fixture::limits();
    exact.max_executed_wire_bytes = block.encode_wire().unwrap().len() as u64;
    exact.max_total_output_bytes = exact.max_executed_wire_bytes;
    exact.max_output_bytes = exact.max_executed_wire_bytes;
    block.validate_execution_outputs(&exact).unwrap();
    let key =
        iroha_crypto::KeyPair::try_from_seed(vec![0x59; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap();
    block
        .add_signature(BlockSignature::new(
            1,
            SignatureOf::try_from_hash(key.private_key(), block.hash()).unwrap(),
        ))
        .unwrap();
    assert!(matches!(
        block.validate_execution_outputs(&exact),
        Err(SetExecutionOutputsError::ExecutedWireTooLarge { .. })
    ));
    block
        .validate_execution_outputs(&fixture::limits())
        .unwrap();
}
