//! Independent public inputs for the retained two-segment MixedScale DEEP fixture.
//!
//! These facts reproduce QuantityFixture::new(MixedScale, 2) from its source constants.
//! Neither the statement, roots, ordering hash nor AXT expectations are read from a proof.

use super::*;
use fastpq_prover::gadgets::public_transfer_statement::materialize_quantity_public_transfers;
use iroha_data_model::nexus::{
    AxtEffectBinding, AxtHandleIssuerContextV1, AxtHandleReplayKey, AxtRemoteSpendClaimV1,
    compute_remote_spend_claim_commitment_v1,
};
use iroha_model_base::topology::LaneId;
use iroha_primitives::{bigint::BigInt, numeric::Numeric};
use iroha_zkp_halo2::poseidon::PoseidonByteHasher;
use norito::codec::Encode;
use sha2::{Digest as _, Sha256};

/// Complete caller expectations derived before either retained artifact is read.
pub(super) struct CaptureFixture {
    pub(super) expected: ExpectedStatement,
    binding: AxtFastpqBinding,
    metadata: FastpqAxtPublicMetadataV1,
    mirrors: FastpqAxtPreProofMirrorsV1,
    remote: Vec<AxtRemoteSpendClaimV1>,
}

impl CaptureFixture {
    /// Reconstruct the generator's two ordered full-domain public transfer occurrences.
    pub(super) fn new() -> Self {
        let asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let mut maximum = [0xff; 64];
        maximum[63] = 0x7f;
        let mut sender = Quantity::from_canonical_numeric(
            Numeric::try_new(BigInt::from_twos_bytes(&maximum).unwrap(), 0).unwrap(),
        )
        .unwrap()
        .try_sub(&Quantity::one())
        .unwrap();
        let mut receiver =
            Quantity::from_canonical_numeric(Numeric::try_new(1_u32, 28).unwrap()).unwrap();
        let batch_hash = Hash::new(b"compact AXT public entry");
        let mut claims = Vec::new();
        let mut remote = Vec::new();
        for counter in 1..=2 {
            let next_sender = sender.try_sub(&Quantity::one()).unwrap();
            let next_receiver = receiver.try_add(&Quantity::one()).unwrap();
            let delta = FastpqPublicTransferDeltaV1 {
                from_account: (*ALICE_ID).clone(),
                to_account: (*BOB_ID).clone(),
                asset_definition: asset.clone(),
                amount: Quantity::one(),
                from_balance_before: sender,
                from_balance_after: next_sender.clone(),
                to_balance_before: receiver,
                to_balance_after: next_receiver.clone(),
            };
            let mut digest = PoseidonByteHasher::new();
            delta.from_account.encode_to(&mut digest);
            delta.to_account.encode_to(&mut digest);
            delta.asset_definition.encode_to(&mut digest);
            delta.amount.encode_to(&mut digest);
            digest.update(batch_hash.as_ref());
            remote.push(AxtRemoteSpendClaimV1::new(
                AxtHandleReplayKey::from_parts(
                    DataSpaceId::new(7),
                    AxtHandleIssuerContextV1::default().asset_definition_incarnation,
                    [8; 32],
                    counter,
                    1,
                    LaneId::new(0),
                ),
                asset.clone(),
                "transfer",
                delta.from_account.to_string(),
                delta.to_account.to_string(),
                delta.amount.clone(),
            ));
            claims.push(FastpqPublicTransferTranscriptV1 {
                batch_hash,
                deltas: vec![delta],
                authority_digest: Hash::new(b"caller-authenticated execution authority"),
                poseidon_preimage_digest: Some(Hash::prehashed(digest.finalize())),
            });
            sender = next_sender;
            receiver = next_receiver;
        }
        let mut dsid = [0; 16];
        dsid[..8].copy_from_slice(&7_u64.to_le_bytes());
        let inputs = PublicInputs {
            dsid,
            slot: 123,
            old_root: Hash::new(b"caller expected old touched-balance root").into(),
            new_root: Hash::new(b"caller expected new touched-balance root").into(),
            perm_root: Hash::new(b"caller expected permission context").into(),
            tx_set_hash: Hash::new(b"caller expected transaction set").into(),
        };
        // This builds only a bounded four-update touched tree, never a proof trace or LDE.
        let (rows, inputs, ordering_hash, private) = materialize_quantity_public_transfers(
            &claims,
            inputs,
            ProofSemantics::AxtTransferClaim,
            PublicTransferLimits::default(),
            TransferSmtBuildLimits::for_update_limit(4).unwrap(),
        )
        .unwrap()
        .into_parts();
        drop(private);
        let statement = FastpqPublicTransferStatementV1 {
            public_inputs: FastpqPublicInputs {
                dsid: inputs.dsid,
                slot: inputs.slot,
                old_root: inputs.old_root,
                new_root: inputs.new_root,
                perm_root: inputs.perm_root,
                tx_set_hash: inputs.tx_set_hash,
            },
            ordering_hash: ordering_hash.into(),
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
        };
        let expected = ExpectedStatement {
            inputs: statement.public_inputs,
            ordering_hash: statement.ordering_hash,
            public_statement_digest: Hash::new(norito::encode_canonical(&statement).unwrap())
                .into(),
        };
        remote.sort_by_key(compute_remote_spend_claim_commitment_v1);
        let mut binding = binding();
        binding.source_tx_commitment = hex::encode(batch_hash.as_ref());
        binding.claim_type = "tx_predicate".into();
        binding.claim_digest = hex::encode([2; 32]);
        binding.witness_commitment = hex::encode([3; 32]);
        binding.policy_commitment = hex::encode([4; 32]);
        binding.verified_effect_type = "transfer".into();
        binding.corridor = "corridor".into();
        binding.effect_binding = Some(AxtEffectBinding {
            destination_domain: None,
            destination_account_id: None,
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(asset.to_string()),
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        });
        binding.remote_spend_intent_commitments = remote
            .iter()
            .map(compute_remote_spend_claim_commitment_v1)
            .collect();
        Self {
            expected,
            binding,
            metadata: FastpqAxtPublicMetadataV1 {
                parameter: AXT_DEFAULT_PARAMETER.into(),
                entry_hash: batch_hash.into(),
                // Generator metadata retains the original two five-unit outer mirror.
                committed_amount: Some(10_u128.to_le_bytes()),
                expiry_slot: 456_u64.to_le_bytes(),
                manifest_root: [5; 32],
                da_commitment: core::array::from_fn(|index| if index == 0 { 1 } else { 6 }),
            },
            mirrors: FastpqAxtPreProofMirrorsV1 {
                dsid: DataSpaceId::new(7),
                manifest_root: [5; 32],
                da_commitment: Some([6; 32]),
                committed_amount: Some(10),
                expiry_slot: Some(456),
            },
            remote,
        }
    }

    /// Borrow the independently constructed context using only the public facade.
    pub(super) fn context(&self) -> ExpectedAxtContext<'_> {
        ExpectedAxtContext {
            binding: &self.binding,
            metadata: &self.metadata,
            mirrors: self.mirrors,
            remote_spend_claims: Some(&self.remote),
        }
    }
}

/// Finite public facade limits with the normal 512 KiB child and 64-query envelope.
pub(super) fn capture_policy() -> VerificationLimits {
    let mut limits = policy();
    limits.transport.max_wire_bytes = 1024 * 1024;
    limits.transport.max_bundle_frame_bytes = 1024 * 1024;
    limits.transport.norito = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        25 * 1024 * 1024,
        96 * 1024 * 1024,
        32,
    );
    limits.bundle.max_wire_bytes = 1024 * 1024;
    limits.bundle.max_total_segment_bytes = 1024 * 1024;
    limits.bundle.max_total_statement_bytes = 512 * 1024;
    limits.bundle.max_total_decode_allocation_charges = 128 * 1024 * 1024;
    limits.bundle.segment.max_proof_bytes = 512 * 1024;
    limits.max_segment_decode_allocation_charges = 64 * 1024 * 1024;
    limits.total_decode = DecodeLimits::new(
        20 * 1024 * 1024,
        20 * 1024 * 1024,
        30 * 1024 * 1024,
        192 * 1024 * 1024,
        32,
    );
    limits
}

/// Read a bounded, SHA-addressed output of the existing fresh public producer test.
pub(super) fn read_capture(variable: &str, label: &str) -> Vec<u8> {
    use std::io::Read;
    let path = std::path::PathBuf::from(std::env::var_os(variable).expect(variable))
        .canonicalize()
        .unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation")
        .canonicalize()
        .unwrap();
    assert_eq!(path.parent(), Some(directory.as_path()));
    let sha = path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix(&format!("quantity-public-producer-deep-{label}-"))
        .and_then(|name| name.strip_suffix(".bin"))
        .expect("fresh SHA-addressed DEEP public producer artifact");
    assert_eq!(sha.len(), 64);
    let cap = u64::try_from(capture_policy().transport.max_wire_bytes).unwrap();
    let file = std::fs::File::open(&path).unwrap();
    assert!(file.metadata().unwrap().len() <= cap);
    let mut bytes = Vec::new();
    file.take(cap + 1).read_to_end(&mut bytes).unwrap();
    assert!(u64::try_from(bytes.len()).unwrap() <= cap);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), sha);
    bytes
}
