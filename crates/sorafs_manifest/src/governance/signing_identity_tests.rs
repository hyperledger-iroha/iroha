//! Capture the existing governance signing and CID projections with populated fixtures.

use super::*;
use crate::signing_identity_test_support::shapes_decodable;
use norito::json::Value;

pub(crate) fn record(rows: &mut Vec<Value>) {
    let root = tests::signed_governance_block(None, None, 0, 1_700_000_400);
    let child = tests::signed_governance_block(
        Some(root.block_cid.clone()),
        Some(root.node.node_cid.clone()),
        1,
        1_700_000_500,
    );
    let mut attributed = tests::signed_governance_block(
        Some(child.block_cid.clone()),
        Some(child.node.node_cid.clone()),
        2,
        1_700_000_600,
    );
    attributed.node.payload =
        GovernanceLogPayloadV1::AppealFinanceReport(tests::sample_appeal_finance_report());
    attributed.node.submission_provenance = Some(GovernanceDagSubmissionProvenanceV1 {
        publisher_account_digest: governance_dag_submission_account_digest_v1(
            b"canonical-norito-account",
        ),
        origin: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
    });
    attributed.node.node_cid = attributed
        .node
        .recompute_node_cid()
        .expect("fixture node CID");
    tests::sign_governance_node(&mut attributed.node, &[0xC7; 32]);
    attributed.block_cid = attributed.recompute_block_cid().expect("fixture block CID");
    tests::sign_governance_block(&mut attributed, &[0xC7; 32]);

    for (case, block) in [
        ("root", &root),
        ("child", &child),
        ("attributed", &attributed),
    ] {
        block
            .validate()
            .expect("complete signed governance fixture");
        let node = &block.node;
        shapes_decodable(
            rows,
            &format!("governance/node-cid/{case}"),
            || {
                GovernanceLogNodeCidPayloadViewV1(GovernanceLogNodeCidPayloadViewWireV1 {
                    version: GOVERNANCE_LOG_VERSION_V1,
                    prev_cid: borrowed_norito::Option(node.prev_cid.as_deref()),
                    timestamp: node.timestamp,
                    publisher_peer_id: borrowed_norito::Vec(&node.publisher_peer_id),
                    submission_provenance: node
                        .submission_provenance
                        .as_ref()
                        .map(borrowed_norito::Value),
                    payload: borrowed_norito::Value(&node.payload),
                })
            },
            GovernanceLogNodeCidPayloadV1 {
                version: GOVERNANCE_LOG_VERSION_V1,
                prev_cid: node.prev_cid.clone(),
                timestamp: node.timestamp,
                publisher_peer_id: node.publisher_peer_id.clone(),
                submission_provenance: node.submission_provenance.clone(),
                payload: node.payload.clone(),
            },
        );
        shapes_decodable(
            rows,
            &format!("governance/node-signature/{case}"),
            || GovernanceLogSignaturePayloadViewV1::from(node),
            GovernanceLogSignaturePayloadV1::from(node),
        );
        shapes_decodable(
            rows,
            &format!("governance/block-cid/{case}"),
            || {
                GovernanceDagBlockCidPayloadViewV1(GovernanceDagBlockCidPayloadViewWireV1 {
                    version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
                    prev_block_cid: borrowed_norito::Option(block.prev_block_cid.as_deref()),
                    sequence: block.sequence,
                    timestamp: block.timestamp,
                    publisher_peer_id: borrowed_norito::Vec(&block.publisher_peer_id),
                    node: borrowed_norito::Value(&block.node),
                })
            },
            GovernanceDagBlockCidPayloadV1 {
                version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
                prev_block_cid: block.prev_block_cid.clone(),
                sequence: block.sequence,
                timestamp: block.timestamp,
                publisher_peer_id: block.publisher_peer_id.clone(),
                node: block.node.clone(),
            },
        );
        shapes_decodable(
            rows,
            &format!("governance/block-signature/{case}"),
            || GovernanceDagBlockSignaturePayloadViewV1::from(block),
            GovernanceDagBlockSignaturePayloadV1::from(block),
        );
    }
    let head = tests::signed_governance_head(&[root.clone(), child]);
    let mut checkpoint = head.clone();
    checkpoint.block_count = 65;
    checkpoint.generated_at += 1;
    checkpoint.checkpoint_cid = Some(root.block_cid);
    tests::sign_governance_head(&mut checkpoint, &[0xC7; 32]);
    for (case, value) in [("no-checkpoint", &head), ("checkpoint", &checkpoint)] {
        value.validate().expect("complete signed head fixture");
        shapes_decodable(
            rows,
            &format!("governance/head-signature/{case}"),
            || GovernanceDagHeadSignaturePayloadViewV1::from(value),
            GovernanceDagHeadSignaturePayloadV1::from(value),
        );
    }
    tests::governance_signing_payload_requires_allocation_free_exact_size();
    tests::governance_signing_payload_rejects_oversize_before_serialize_or_allocate();
}
