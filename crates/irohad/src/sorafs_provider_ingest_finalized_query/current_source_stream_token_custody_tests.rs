// Exact-head StreamToken control joins without a synthetic admission or grant.

use iroha_core::query::stream_token_custody::StreamTokenCustodyControlSnapshotV1;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid},
};
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyActiveHeadV1, SignerCustodyAnchorV1, SignerCustodyAuthorityV1,
        SignerCustodyBindingV1,
    },
    custody_control::{SignerCustodyControlStateV1, SignerCustodyPolicyV1},
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1},
};
use sorafs_node::FinalizedProviderIngestAuthorizationV1;

#[expect(
    clippy::too_many_lines,
    reason = "the fixture builds one exact source request and independently governed custody control"
)]
fn current_source_and_control_fixture() -> (
    ProviderIngestCurrentSourceAssignmentV1,
    SignerCustodyBindingV1,
    StreamTokenCustodyControlSnapshotV1,
) {
    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new([0x70; 32]),
    ));
    let source = [0x52; 32];
    let head = ProviderIngestFinalizedCursorV1 {
        height: 7,
        block_hash: [0x71; 32],
    };
    let root = ManifestRootCid::from_blake3_digest([0x72; 32]).expect("root CID");
    let chunker = ChunkerProfileHandle {
        profile_id: 1,
        namespace: "sorafs".to_owned(),
        name: "sf1".to_owned(),
        semver: "1.0.0".to_owned(),
        multihash_code: 0x1f,
    };
    let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        head.height,
        head.block_hash,
        [0x51; 32],
        [0x61; 32],
        [0x73; 32],
        root.as_bytes().to_vec(),
        chunker.to_handle(),
        [0x74; 32],
        [0x75; 32],
        4_096,
    )
    .expect("canonical immutable order authorization");
    let canonical_request = ProviderIngestSourceRequestV1::new(authorization, vec![source], None)
        .expect("canonical source inventory");
    let assignment = ProviderIngestCurrentSourceAssignmentV1 {
        network_id,
        source_provider_id: source,
        finalized_head: head,
        finalized_at_unix_ms: 7_000,
        provider_state_root: [0x76; 32],
        assignment_revision: 1,
        canonical_request,
    };
    let signer = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::Ed25519).expect("signer key");
    let attester =
        KeyPair::try_from_seed(vec![0x32; 32], Algorithm::Ed25519).expect("attester key");
    let binding = SignerCustodyBindingV1 {
        chain_id: "sorafs-reference".to_owned(),
        network_id: *network_id.as_bytes(),
        runtime_handle: "software://sorafs/stream-token/source".to_owned(),
        key_handle: "software://sorafs/stream-token/key-1".to_owned(),
        service_id: "source-token-signer".to_owned(),
        administrator_id: "source-token-administrator".to_owned(),
        role: SignerRoleV1::StreamToken,
        purpose: SignerPurposeBindingV1::StreamToken {
            provider_id: source,
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: signer.public_key().clone(),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x41; 32],
    };
    let policy = SignerCustodyPolicyV1 {
        binding: binding.clone(),
        attester_authority: SignerCustodyAuthorityV1 {
            service_id: "source-token-attester".to_owned(),
            administrator_id: "source-token-attester-administrator".to_owned(),
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x42; 32],
        },
        attester_public_key: attester.public_key().clone(),
        active_from_unix_ms: 1_000,
        active_until_unix_ms: 20_000,
        max_validity_ms: 5_000,
        max_anchor_age_ms: 5_000,
    };
    let state = SignerCustodyControlStateV1 {
        policy,
        next_sequence: 2,
        predecessor_digest: [0x43; 32],
        active_head: Some(SignerCustodyActiveHeadV1 {
            record_digest: [0x43; 32],
            sequence: 1,
            approved_anchor: SignerCustodyAnchorV1 {
                height: 6,
                block_hash: [0x44; 32],
                state_digest: [0x45; 32],
            },
            key_revision: 1,
            policy_revision: 1,
            policy_digest: [0x41; 32],
        }),
        signer_revoked: false,
        attester_revoked: false,
    };
    state.validate().expect("well-formed native control");
    let control = StreamTokenCustodyControlSnapshotV1 {
        state,
        anchor: SignerCustodyAnchorV1 {
            height: head.height,
            block_hash: head.block_hash,
            state_digest: [0x46; 32],
        },
    };
    (assignment, binding, control)
}

#[test]
fn source_stream_token_control_requires_exact_current_head_and_provider() {
    let (assignment, binding, control) = current_source_and_control_fixture();
    let head = assignment.finalized_head();
    let joined = join_current_source_stream_token_custody(
        assignment.clone(),
        &binding,
        head,
        control.clone(),
    )
    .expect("same-head unrevoked native control");
    assert_eq!(joined.assignment(), &assignment);
    assert_eq!(joined.control(), &control);
    assert_eq!(joined.control_anchor(), control.anchor);

    let stale = ProviderIngestFinalizedCursorV1 {
        height: head.height - 1,
        block_hash: [0x47; 32],
    };
    assert_eq!(
        join_current_source_stream_token_custody(
            assignment.clone(),
            &binding,
            stale,
            control.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut substituted_control = control.clone();
    substituted_control.anchor.block_hash = [0x48; 32];
    assert_eq!(
        join_current_source_stream_token_custody(
            assignment.clone(),
            &binding,
            head,
            substituted_control,
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut other_source = binding.clone();
    other_source.purpose = SignerPurposeBindingV1::StreamToken {
        provider_id: [0x53; 32],
    };
    assert_eq!(
        join_current_source_stream_token_custody(assignment, &other_source, head, control),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
}

#[test]
fn source_stream_token_control_rejects_revocation_and_stale_key() {
    let (assignment, binding, control) = current_source_and_control_fixture();
    let head = assignment.finalized_head();
    for revoked_signer in [true, false] {
        let mut revoked = control.clone();
        revoked.state.signer_revoked = revoked_signer;
        revoked.state.attester_revoked = !revoked_signer;
        assert_eq!(
            join_current_source_stream_token_custody(assignment.clone(), &binding, head, revoked,),
            Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
        );
    }
    let mut unenrolled = control.clone();
    unenrolled.state.active_head = None;
    assert_eq!(
        join_current_source_stream_token_custody(assignment.clone(), &binding, head, unenrolled),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut stale_binding = binding.clone();
    stale_binding.key_revision += 1;
    assert_eq!(
        join_current_source_stream_token_custody(
            assignment.clone(),
            &stale_binding,
            head,
            control.clone(),
        ),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
    let mut other_network = binding;
    other_network.network_id[0] ^= 1;
    assert_eq!(
        join_current_source_stream_token_custody(assignment, &other_network, head, control),
        Err(ProviderIngestFinalizedLedgerErrorV1::Rejected)
    );
}
