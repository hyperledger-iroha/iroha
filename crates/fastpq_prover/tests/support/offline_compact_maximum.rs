//! Maximum default application occupancy under the unchanged masked proof geometry.
//!
//! Four complete account keys occupy distinct top path quadrants. This maximizes
//! the touched-tree node census for four updates; it does not increase the fixed
//! physical trace. AXT additionally fills the exact canonical carrier admission
//! boundary. Caller facts are always selected before any retained artifact read.

use super::*;
use fastpq_prover::gadgets::public_transfer_statement::materialize_quantity_public_transfers;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{account::AccountId, fastpq::transfer_balance_key};
use iroha_zkp_halo2::poseidon::PoseidonByteHasher;
use norito::codec::Encode;

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
const SHAPE_FACTS: &str = "application_shape=four-quadrant-keys\nupdates=4\nunique_keys=4\nretained_touched_nodes=127\nretained_sibling_hashes=128\ntouched_node_hashes=251\n";

fn four_quadrant_accounts() -> [AccountId; 4] {
    let asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let mut accounts: [Option<AccountId>; 4] = core::array::from_fn(|_| None);
    // Public deterministic fixture keys, never runtime signing material. Search
    // only a bounded public starting-position property, not a hash collision.
    for counter in 0_u32..4096 {
        let mut seed = b"FASTPQ four-quadrant fixture v1".to_vec();
        seed.extend_from_slice(&counter.to_le_bytes());
        let key = KeyPair::from_seed(seed, Algorithm::Ed25519);
        let account = AccountId::new(key.public_key().clone());
        let frame = transfer_balance_key(&asset, &account).unwrap();
        let hash: [u8; 32] = Hash::new_from_chunks(&[b"fastpq:v1:smt:key|", &frame]).into();
        let path = u32::from_le_bytes(hash[..4].try_into().unwrap());
        accounts[(path >> 30) as usize].get_or_insert(account);
        if accounts.iter().all(Option::is_some) {
            return accounts.map(Option::unwrap);
        }
    }
    panic!("bounded canonical key fixture did not cover four quadrants");
}

fn fixture() -> capture::CaptureFixture {
    capture::CaptureFixture::with_independent_accounts(four_quadrant_accounts())
}

fn inputs(statement: &FastpqPublicTransferStatementV1) -> PublicInputs {
    let source = statement.public_inputs;
    PublicInputs {
        dsid: source.dsid,
        slot: source.slot,
        old_root: source.old_root,
        new_root: source.new_root,
        perm_root: source.perm_root,
        tx_set_hash: source.tx_set_hash,
    }
}

fn empty_axt_frame_bytes(fixture: &capture::CaptureFixture, binding: &AxtFastpqBinding) -> usize {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let context = fixture.context();
    let empty = FastpqAxtCompactArtifactV1 {
        profile_id: quantity_profile_id(),
        statement: fixture.statement.clone(),
        binding: binding.clone(),
        metadata: context.metadata.clone(),
        mirrors: context.mirrors,
        remote_spend_claims: context.remote_spend_claims.map(<[_]>::to_vec),
        bundle_frame: Vec::new(),
    };
    norito::core::encoded_frame_len(&empty).unwrap()
}

fn artifact_admission_bytes(
    fixture: &capture::CaptureFixture,
    binding: &AxtFastpqBinding,
) -> usize {
    // Match the actual public producer's conservative carrier preflight, using
    // its public resource report for the canonical child bound. No dummy proof.
    empty_axt_frame_bytes(fixture, binding)
        + quantity_artifact_resources(2, 0)
            .unwrap()
            .maximum_bundle_frame_bytes
        + 32
}

fn maximum_axt_fixture() -> capture::CaptureFixture {
    let mut fixture = fixture();
    let mut binding = fixture.context().binding.clone();
    let cap = VerificationLimits::default().transport.max_wire_bytes;
    let (mut low, mut high) = (1, cap);
    while low < high {
        let midpoint = low + (high - low).div_ceil(2);
        binding.source_receipt_id = "r".repeat(midpoint);
        if artifact_admission_bytes(&fixture, &binding) <= cap {
            low = midpoint;
        } else {
            high = midpoint - 1;
        }
    }
    binding.source_receipt_id = "r".repeat(low);
    // This chosen string lands away from a compact-length width transition,
    // so an actual one-byte canonical change reaches precisely cap+1.
    assert_eq!(artifact_admission_bytes(&fixture, &binding), cap);
    binding.source_receipt_id.push('r');
    assert_eq!(artifact_admission_bytes(&fixture, &binding), cap + 1);
    binding.source_receipt_id.pop();
    fixture.replace_binding(binding);
    fixture
}

#[test]
fn four_keys_reach_the_exact_maximum_touched_tree_under_default_updates() {
    let fixture = fixture();
    let statement = &fixture.statement;
    assert_eq!(statement.transcripts.len(), 2);
    assert_eq!(statement.transitions.len(), 4);
    let context = fixture.context();
    let remote = context.remote_spend_claims.unwrap();
    assert_eq!(
        context.metadata.source_transfer_occurrences.len(),
        remote.len()
    );
    for (occurrence, claim) in context
        .metadata
        .source_transfer_occurrences
        .iter()
        .zip(remote)
    {
        let delta = &statement.transcripts[occurrence.transcript_index as usize].deltas
            [occurrence.delta_index as usize];
        assert_eq!(occurrence.pair_ordinal, occurrence.transcript_index);
        assert_eq!(
            occurrence.remote_spend_claim_commitment,
            iroha_data_model::nexus::compute_remote_spend_claim_commitment_v1(claim)
        );
        assert_eq!(
            occurrence.transfer_digest,
            iroha_data_model::nexus::axt_source_transfer_digest_v1(delta)
        );
        assert_eq!(claim.from, delta.from_account.to_string());
        assert_eq!(claim.to, delta.to_account.to_string());
        assert_eq!(claim.effective_amount, delta.amount);
    }
    let rows = quantity_rows_for_public_preparation(
        &statement.transcripts,
        inputs(statement),
        PublicTransferLimits::default(),
        4,
    )
    .unwrap();
    let table = prepare_quantity_public_transfers(
        &rows,
        &statement.transcripts,
        inputs(statement),
        ProofSemantics::AxtTransferClaim,
        PublicTransferLimits::default(),
    )
    .unwrap();
    assert_eq!(table.keys().len(), 4);
    assert_eq!(table.pairs().len(), 2);
    assert_eq!(table.work().allocation_steps, 12);
    let mut quadrants = [false; 4];
    for key in table.keys() {
        let initial_path = u32::from_le_bytes(key.key_hash[..4].try_into().unwrap());
        assert_eq!(
            key.path, initial_path,
            "four distinct quadrants cannot collide"
        );
        quadrants[(key.path >> 30) as usize] = true;
        let decoded: iroha_data_model::fastpq::FastpqBalanceKeyV1 =
            norito::decode_canonical(&key.key).unwrap();
        assert_eq!(
            transfer_balance_key(&decoded.asset_definition, &decoded.account).unwrap(),
            key.key
        );
    }
    assert_eq!(quadrants, [true; 4]);
    let witnesses = table
        .build_smt_witnesses(ProvingLimits::default().private_smt)
        .unwrap();
    let work = witnesses.work();
    assert_eq!((work.updates, work.unique_keys), (4, 4));
    // Four nodes at each level0..30, two at31 and one at32.
    assert_eq!(work.retained_nodes, 31 * 4 + 2 + 1);
    assert_eq!(work.sibling_hashes, 4 * 32);
    assert_eq!(work.node_hashes, 127 - 4 + 128);
    let exact = TransferSmtBuildLimits {
        max_updates: 4,
        max_unique_keys: 4,
        max_retained_nodes: 127,
        max_sibling_hashes: 128,
        max_node_hashes: 251,
    };
    assert_eq!(table.build_smt_witnesses(exact).unwrap().work(), work);
    for dimension in 0..5 {
        let mut short = exact;
        let name = match dimension {
            0 => {
                short.max_updates -= 1;
                "max_transfer_smt_updates"
            }
            1 => {
                short.max_unique_keys -= 1;
                "max_transfer_smt_keys"
            }
            2 => {
                short.max_retained_nodes -= 1;
                "max_transfer_smt_nodes"
            }
            3 => {
                short.max_sibling_hashes -= 1;
                "max_transfer_smt_siblings"
            }
            _ => {
                short.max_node_hashes -= 1;
                "max_transfer_smt_node_hashes"
            }
        };
        assert!(matches!(table.build_smt_witnesses(short),
            Err(Error::VerifierLimitExceeded { limit, .. }) if limit == name));
    }
    let resources = quantity_artifact_resources(2, 0).unwrap();
    assert_eq!(
        resources.total_trace_cells,
        ProvingLimits::default().max_total_trace_cells
    );
    assert_eq!(
        resources.total_queries,
        VerificationLimits::default().bundle.max_total_queries
    );
    assert_eq!(resources.maximum_segment_frame_bytes, 502_895);
    assert_eq!(resources.maximum_bundle_frame_bytes, 1_006_942);
}

fn three_delta_statement() -> FastpqPublicTransferStatementV1 {
    let fixture = fixture();
    let mut claims = fixture.statement.transcripts.clone();
    let mut third = claims[0].clone();
    let delta = &mut third.deltas[0];
    delta.from_balance_before = delta.from_balance_after.clone();
    delta.to_balance_before = delta.to_balance_after.clone();
    delta.from_balance_after = delta.from_balance_before.try_sub(&delta.amount).unwrap();
    delta.to_balance_after = delta.to_balance_before.try_add(&delta.amount).unwrap();
    let mut digest = PoseidonByteHasher::new();
    delta.from_account.encode_to(&mut digest);
    delta.to_account.encode_to(&mut digest);
    delta.asset_definition.encode_to(&mut digest);
    delta.amount.encode_to(&mut digest);
    digest.update(third.batch_hash.as_ref());
    third.poseidon_preimage_digest = Some(Hash::prehashed(digest.finalize()));
    claims.push(third);
    // Construct genuinely consistent public inputs outside the production
    // default; the request below still passes unchanged default proving limits.
    let (rows, input, ordering, private) = materialize_quantity_public_transfers(
        &claims,
        inputs(&fixture.statement),
        ProofSemantics::StateTransition,
        PublicTransferLimits::default(),
        TransferSmtBuildLimits::for_update_limit(6).unwrap(),
    )
    .unwrap()
    .into_parts();
    drop(private);
    FastpqPublicTransferStatementV1 {
        public_inputs: FastpqPublicInputs {
            dsid: input.dsid,
            slot: input.slot,
            old_root: input.old_root,
            new_root: input.new_root,
            perm_root: input.perm_root,
            tx_set_hash: input.tx_set_hash,
        },
        ordering_hash: ordering.into(),
        transitions: rows
            .into_iter()
            .map(|row| FastpqStateTransition {
                key: row.key,
                pre_value: row.pre_value,
                post_value: row.post_value,
                operation: FastpqOperationKind::Transfer,
            })
            .collect(),
        transcripts: claims,
    }
}

/// Called by the existing serial public-producer negative test to avoid Busy races.
pub fn assert_maximum_context_preflight() {
    let fixture = maximum_axt_fixture();
    let limits = VerificationLimits::default();
    let mut proving = ProvingLimits::default();
    // The exact positive boundary must reach private-tree admission. This zero
    // private budget prevents any witness/FFT even if the expected error regresses.
    proving.private_smt.max_updates = 0;
    let result = prove_quantity_axt_artifact(
        &fixture.statement,
        fixture.expected,
        fixture.context(),
        proving,
        limits,
    );
    assert!(
        matches!(
            &result,
            Err(ProvingError::Prove(Error::VerifierLimitExceeded {
                limit: "max_transfer_smt_updates",
                actual: 4,
                max: 0,
            }))
        ),
        "exact AXT context boundary returned {result:?}"
    );
    let mut oversized = fixture.context().binding.clone();
    oversized.source_receipt_id.push('r');
    assert_eq!(artifact_admission_bytes(&fixture, &oversized), 1_048_577);
    let context = ExpectedAxtContext {
        binding: &oversized,
        ..fixture.context()
    };
    let result = prove_quantity_axt_artifact(
        &fixture.statement,
        fixture.expected,
        context,
        proving,
        limits,
    );
    assert!(
        matches!(
            &result,
            Err(ProvingError::Prove(Error::VerifierLimitExceeded {
                limit: "max_compact_producer_artifact_bytes",
                actual: 1_048_577,
                max: 1_048_576,
            }))
        ),
        "one-byte excessive AXT context returned {result:?}"
    );
    let three = three_delta_statement();
    let expected = ExpectedStatement::from_statement(&three).unwrap();
    assert_eq!(three.transitions.len(), 6);
    assert!(matches!(
        prove_quantity_ordinary_artifact(
            &three,
            expected,
            ProvingLimits::default(),
            VerificationLimits::default(),
        ),
        Err(ProvingError::Prove(Error::VerifierLimitExceeded {
            limit: "max_bundle_segments",
            actual: 3,
            max: 2,
        }))
    ));
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "complete maximum four-key ordinary public producer on required Metal"]
fn required_metal_maximum_ordinary_producer_and_reused_verifier_controls() {
    two::produce_fixture(false, "maximum-ordinary", fixture, SHAPE_FACTS);
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "complete maximum four-key AXT public producer at exact default context admission"]
fn required_metal_maximum_axt_producer_and_reused_verifier_controls() {
    two::produce_fixture(
        true,
        "maximum-axt",
        maximum_axt_fixture,
        concat!(
            "application_shape=four-quadrant-keys-maximum-axt-context\n",
            "updates=4\nunique_keys=4\nretained_touched_nodes=127\n",
            "retained_sibling_hashes=128\ntouched_node_hashes=251\n",
            "conservative_artifact_preflight_bytes=1048576\n"
        ),
    );
}

#[test]
#[ignore = "requires FASTPQ_TEST_MAXIMUM_ORDINARY_ARTIFACT; no witness or prover"]
fn captured_maximum_ordinary_artifact_verifies_without_reproving() {
    two::replay_fixture(
        false,
        "maximum-ordinary",
        "FASTPQ_TEST_MAXIMUM_ORDINARY_ARTIFACT",
        &fixture(),
    );
}

#[test]
#[ignore = "requires FASTPQ_TEST_MAXIMUM_AXT_ARTIFACT; no witness or prover"]
fn captured_maximum_axt_artifact_verifies_without_reproving() {
    two::replay_fixture(
        true,
        "maximum-axt",
        "FASTPQ_TEST_MAXIMUM_AXT_ARTIFACT",
        &maximum_axt_fixture(),
    );
}
