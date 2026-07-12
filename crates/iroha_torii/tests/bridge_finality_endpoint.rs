#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Bridge finality endpoints expose only Kura's exact Sumeragi-v2 artifact.

use std::{num::NonZeroU64, sync::Arc};

use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    extract::connect_info::ConnectInfo,
    http::Request,
};
use hyper::StatusCode;
use iroha_config::parameters::actual::Queue as QueueConfig;
use iroha_core::{
    kiso::KisoHandle,
    kura::Kura,
    query::store::LiveQueryStore,
    queue::Queue,
    state::{State, World},
};
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    block::{
        BlockHeader, SignedBlock,
        consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
        },
    },
    bridge::{
        BridgeFinalityBundle, BridgeFinalityProof, BridgeFinalityVerifier,
        BridgeFinalityVerifyError,
    },
    peer::PeerId,
};
use iroha_torii::{MaybeTelemetry, OnlinePeersProvider, Torii, test_utils};
use norito::codec::Encode as _;
use tower::ServiceExt as _;

struct EndpointFixture {
    app: Router,
    chain_id: ChainId,
    block: Arc<SignedBlock>,
    artifact: V2FinalityArtifact,
    kura: Arc<Kura>,
}

fn exact_v2_fixture(chain_id: ChainId) -> (Arc<SignedBlock>, V2FinalityArtifact) {
    let mut keys = (0..4)
        .map(|_| {
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("generate bridge finality BLS fixture key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = keys
        .iter()
        .zip([40_u64, 30, 20, 10])
        .map(|(key, power)| ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power,
        })
        .collect::<Vec<_>>();

    let block_key = KeyPair::try_random().expect("generate block fixture key");
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("non-zero height"),
        None,
        None,
        None,
        0,
        0,
    );
    let block = Arc::new(
        iroha_data_model::block::builder::BlockBuilder::new(header)
            .build_with_signature(0, block_key.private_key()),
    );
    let context = HeightContext {
        chain_id,
        protocol_version: PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Npos,
        parent_commit_qc: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid powered fixture roster"),
        roster,
        nexus_amx_context_hash: Hash::new(b"Torii exact-v2 bridge context"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::Plain,
            chunk_size_bytes: 1024,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 4096,
            max_chunk_count: 4,
        },
        leader_seed: [0x42; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(b"Torii exact-v2 bridge payload"),
    };
    let execution_commitment = ExecutionCommitment::without_topups(
        Hash::new(b"Torii exact-v2 parent state"),
        Hash::new(b"Torii exact-v2 post state"),
        Hash::new(b"Torii exact-v2 ordinary writes"),
    );
    let mut commit_qc = QuorumCertificate {
        round: ConsensusRound {
            context_id: context.id(),
            height: 1,
            view: 0,
        },
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(&context, 0)
        .expect("valid commit certificate signer");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keys[usize::try_from(*index).expect("fixture signer index")].private_key(),
                &preimage,
            )
            .expect("sign exact v2 commit vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate exact v2 commit votes");
    let validator_set_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("derive exact v2 validator PoP")
        })
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    artifact
        .verify()
        .expect("endpoint fixture is an exact cryptographically valid v2 artifact");
    (block, artifact)
}

fn endpoint_fixture(persist_artifact: bool) -> EndpointFixture {
    let cfg = test_utils::mk_minimal_root_cfg();
    let chain_id = cfg.common.chain.clone();
    let (block, artifact) = exact_v2_fixture(chain_id.clone());
    let kura = Kura::blank_kura_for_testing();
    kura.store_block(Arc::clone(&block))
        .expect("store canonical endpoint block");
    if persist_artifact {
        let receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("persist exact v2 finality artifact");
        assert_eq!(receipt.height(), artifact.height);
        assert_eq!(receipt.block_hash(), artifact.block_hash);
    }

    let state = Arc::new(State::new_with_chain_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        chain_id.clone(),
    ));
    let events_sender: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
    let queue = Arc::new(Queue::from_config(QueueConfig::default(), events_sender));
    let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
    drop(peers_tx);
    let torii = Torii::new_with_handle(
        chain_id.clone(),
        KisoHandle::mock(&cfg),
        cfg.torii.clone(),
        queue,
        tokio::sync::broadcast::channel(1).0,
        LiveQueryStore::start_test(),
        Arc::clone(&kura),
        state,
        cfg.common.key_pair.clone(),
        OnlinePeersProvider::new(peers_rx),
        None,
        MaybeTelemetry::disabled(),
    );
    EndpointFixture {
        app: torii.api_router_for_tests(),
        chain_id,
        block,
        artifact,
        kura,
    }
}

async fn get_norito(app: &Router, uri: &str) -> (StatusCode, Bytes) {
    let mut request = Request::builder()
        .uri(uri)
        .header(axum::http::header::ACCEPT, "application/x-norito")
        .body(Body::empty())
        .expect("build endpoint request");
    request
        .extensions_mut()
        .insert(ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))));
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("endpoint response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .expect("bounded endpoint body");
    (status, bytes)
}

#[tokio::test]
async fn proof_and_bundle_endpoints_return_the_exact_durable_v2_artifact() {
    let fixture = endpoint_fixture(true);
    let (status, bytes) = get_norito(&fixture.app, "/v1/bridge/finality/1").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let proof: BridgeFinalityProof =
        norito::decode_from_bytes(&bytes).expect("decode exact bridge finality proof");
    assert_eq!(proof.block_header, fixture.block.header());
    assert_eq!(proof.finality_artifact, fixture.artifact);

    let mut missing_anchor = BridgeFinalityVerifier::new(fixture.chain_id.clone());
    assert_eq!(
        missing_anchor.verify(&proof),
        Err(BridgeFinalityVerifyError::MissingContextAnchor)
    );
    let mut verifier = BridgeFinalityVerifier::with_context(
        fixture.chain_id.clone(),
        fixture.artifact.context_id(),
    );
    verifier
        .verify(&proof)
        .expect("trusted verifier accepts exact endpoint proof");

    let (status, bytes) = get_norito(&fixture.app, "/v1/bridge/finality/bundle/1").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected body: {}",
        String::from_utf8_lossy(&bytes)
    );
    let bundle: BridgeFinalityBundle =
        norito::decode_from_bytes(&bytes).expect("decode exact bridge finality bundle");
    assert_eq!(bundle.finality_proof, proof);
    assert_eq!(bundle.commitment.chain_id, fixture.chain_id);
    assert_eq!(bundle.commitment.block_height, 1);
    assert_eq!(bundle.commitment.block_hash, fixture.block.hash());
    assert_eq!(
        bundle.commitment.height_context_id,
        fixture.artifact.context_id()
    );
    let mut bundle_verifier =
        BridgeFinalityVerifier::with_context(fixture.chain_id, fixture.artifact.context_id());
    bundle_verifier
        .verify_bundle(&bundle)
        .expect("trusted verifier accepts exact endpoint bundle");
}

#[tokio::test]
async fn proof_and_bundle_endpoints_fail_closed_when_the_sidecar_is_missing() {
    let fixture = endpoint_fixture(false);
    for uri in ["/v1/bridge/finality/1", "/v1/bridge/finality/bundle/1"] {
        let (status, _) = get_norito(&fixture.app, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "unexpected status for {uri}");
    }
}

#[tokio::test]
async fn proof_and_bundle_endpoints_fail_closed_for_a_forged_durable_qc() {
    let fixture = endpoint_fixture(true);
    let mut forged = fixture.artifact.clone();
    forged.commit_qc.aggregate_signature[0] ^= 0x80;
    forged
        .validate()
        .expect("aggregate substitution remains structurally valid");
    let path = fixture
        .kura
        .store_root()
        .join("blocks")
        .join("v2_finality")
        .join("00000000000000000001.norito");
    std::fs::write(&path, forged.encode()).expect("substitute forged durable artifact bytes");

    for uri in ["/v1/bridge/finality/1", "/v1/bridge/finality/bundle/1"] {
        let (status, _) = get_norito(&fixture.app, uri).await;
        assert_eq!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "forged durable artifact must fail closed for {uri}"
        );
    }
}
