//! Incremental block builder that uses typed incremental Merkle updates during assembly.

use std::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};

use iroha_crypto::{HashOf, MerkleTree, SignatureOf};

use super::{BlockHeader, BlockPayload, BlockResult, BlockSignature, SignedBlock};
use crate::{
    consensus::PreviousRosterEvidence,
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    transaction::signed::{
        SignedTransaction, TransactionEntrypoint, TransactionResult, TransactionResultInner,
    },
    trigger::TimeTriggerEntrypoint,
};

/// Helper to incrementally assemble a block while maintaining Merkle roots.
#[derive(Debug, Clone)]
pub struct BlockBuilder {
    header: BlockHeader,
    transactions: Vec<SignedTransaction>,
    time_triggers: Vec<TimeTriggerEntrypoint>,
    results: Vec<TransactionResult>,
    entry_merkle: MerkleTree<TransactionEntrypoint>,
    result_merkle: MerkleTree<TransactionResult>,
    da_commitments: Option<DaCommitmentBundle>,
    da_proof_policies: Option<DaProofPolicyBundle>,
    da_pin_intents: Option<DaPinIntentBundle>,
    previous_roster_evidence: Option<PreviousRosterEvidence>,
}

impl BlockBuilder {
    /// Create a new builder with an initial header. Merkle roots are derived as
    /// items are pushed and written into the header on `build()`.
    pub fn new(header: BlockHeader) -> Self {
        Self {
            header,
            transactions: Vec::new(),
            time_triggers: Vec::new(),
            results: Vec::new(),
            entry_merkle: MerkleTree::default(),
            result_merkle: MerkleTree::default(),
            da_commitments: None,
            da_proof_policies: None,
            da_pin_intents: None,
            previous_roster_evidence: None,
        }
    }

    /// Push a signed transaction and update the entrypoint Merkle tree.
    pub fn push_transaction(&mut self, tx: SignedTransaction) -> usize {
        let idx = self.transactions.len();
        let h: HashOf<TransactionEntrypoint> = tx.hash_as_entrypoint();
        self.entry_merkle.add(h);
        self.transactions.push(tx);
        idx
    }

    /// Push a time trigger and update the entrypoint Merkle tree.
    pub fn push_time_trigger(&mut self, trig: TimeTriggerEntrypoint) -> usize {
        let idx = self.transactions.len() + self.time_triggers.len();
        let h: HashOf<TransactionEntrypoint> = trig.hash_as_entrypoint();
        self.entry_merkle.add(h);
        self.time_triggers.push(trig);
        idx
    }

    /// Push a transaction result and update the result Merkle tree.
    pub fn push_result(&mut self, inner: TransactionResultInner) -> usize {
        let idx = self.results.len();
        let h: HashOf<TransactionResult> = TransactionResult::hash_from_inner(&inner);
        self.result_merkle.add(h);
        self.results.push(TransactionResult::from(inner));
        idx
    }

    /// Attach a pre-built DA commitment bundle that will be embedded in the resulting block.
    pub fn set_da_commitments(&mut self, bundle: Option<DaCommitmentBundle>) {
        self.da_commitments = bundle;
    }

    /// Attach a pre-built DA proof policy bundle that will be embedded in the resulting block.
    pub fn set_da_proof_policies(&mut self, bundle: Option<DaProofPolicyBundle>) {
        self.da_proof_policies = bundle;
    }

    /// Attach a pre-built DA pin intent bundle that will be embedded in the resulting block.
    pub fn set_da_pin_intents(&mut self, bundle: Option<DaPinIntentBundle>) {
        self.da_pin_intents = bundle;
    }

    /// Attach previous-height roster evidence that will be embedded in the resulting block.
    pub fn set_previous_roster_evidence(&mut self, evidence: Option<PreviousRosterEvidence>) {
        self.previous_roster_evidence = evidence;
    }

    /// Build a `SignedBlock` with the provided signatures.
    pub fn build(mut self, signatures: BTreeSet<BlockSignature>) -> SignedBlock {
        // Write roots into header
        self.header.merkle_root = self.entry_merkle.root();
        self.header.result_merkle_root = self.result_merkle.root();
        let da_commitments = self.da_commitments.clone();
        let da_proof_policies = self.da_proof_policies.clone();
        let da_pin_intents = self.da_pin_intents.clone();
        let previous_roster_evidence = self.previous_roster_evidence.clone();
        let payload = BlockPayload {
            header: self.header,
            transactions: self.transactions,
            da_commitments,
            da_proof_policies,
            da_pin_intents,
            previous_roster_evidence,
        };
        let result = BlockResult {
            time_triggers: self.time_triggers,
            merkle: self.entry_merkle,
            result_merkle: self.result_merkle,
            transaction_results: self.results,
            fastpq_transcripts: BTreeMap::new(),
            axt_envelopes: Vec::new(),
            axt_policy_snapshot: None,
        };
        let mut block = SignedBlock {
            signatures,
            payload,
            result: Some(result),
        };
        block.set_da_proof_policies(self.da_proof_policies);
        block.set_da_commitments(self.da_commitments);
        block.set_da_pin_intents(self.da_pin_intents);
        block.set_previous_roster_evidence(self.previous_roster_evidence);
        block
    }

    /// Convenience: sign the built header hash with a single validator and return the block.
    pub fn build_with_signature(
        mut self,
        signatory_index: u64,
        private_key: &iroha_crypto::PrivateKey,
    ) -> SignedBlock {
        // Precompute roots and header hash
        self.header.merkle_root = self.entry_merkle.root();
        self.header.result_merkle_root = self.result_merkle.root();
        self.header
            .set_da_proof_policies_hash(self.da_proof_policies.as_ref().map(HashOf::new));
        self.header.da_commitments_hash = self.da_commitments.as_ref().and_then(|bundle| {
            if bundle.is_empty() {
                None
            } else {
                Some(bundle.canonical_hash())
            }
        });
        self.header.da_pin_intents_hash = self.da_pin_intents.as_ref().and_then(|bundle| {
            bundle
                .merkle_root()
                .map(HashOf::<DaPinIntentBundle>::from_untyped_unchecked)
        });
        self.header
            .set_prev_roster_evidence_hash(self.previous_roster_evidence.as_ref().map(HashOf::new));
        let sig = SignatureOf::from_hash(private_key, self.header.hash());
        let mut set = BTreeSet::new();
        set.insert(BlockSignature::new(signatory_index, sig));
        self.build(set)
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        da::{
            commitment::{
                DaCommitmentBundle, DaCommitmentRecord, DaProofPolicy, DaProofScheme, KzgCommitment,
            },
            prelude::RetentionPolicy,
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{DataSpaceId, LaneId},
        prelude::*,
        sorafs::pin_registry::ManifestDigest,
        transaction::signed::TransactionBuilder,
    };

    #[test]
    fn builder_roots_match_manual_construction() {
        // Minimal header
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
        let chain: ChainId = "test-chain".parse().unwrap();
        let authority = AccountId::new(
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key"),
        );
        let private_key: iroha_crypto::PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .unwrap();

        // Two txs
        let tx1 = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(&private_key);
        let tx2 = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions(core::iter::empty::<InstructionBox>())
            .sign(&private_key);
        // One trigger
        let trig = TimeTriggerEntrypoint {
            id: "test_trigger".parse().unwrap(),
            // Empty execution step
            instructions: ExecutionStep(Vec::<InstructionBox>::new().into()),
            authority: authority.clone(),
        };
        // Three results
        let r1 = TransactionResultInner::Ok(DataTriggerSequence::default());
        let r2 = TransactionResultInner::Err(
            crate::transaction::error::TransactionRejectionReason::Validation(
                ValidationFail::InternalError("bad_query".into()),
            ),
        );
        let r3 = TransactionResultInner::Ok(DataTriggerSequence::default());

        // Build incrementally
        let mut bb = BlockBuilder::new(header);
        bb.push_transaction(tx1.clone());
        bb.push_transaction(tx2.clone());
        let mut built = bb.build_with_signature(0, &private_key);

        // Manual construction
        let mut manual = SignedBlock::presigned(
            BlockSignature::new(
                0,
                SignatureOf::from_hash(&private_key, built.header().hash()),
            ),
            built.header(), // reuse header values (roots) for signature correctness
            vec![tx1.clone(), tx2.clone()],
        );
        let entry_hashes = vec![
            tx1.hash_as_entrypoint(),
            tx2.hash_as_entrypoint(),
            trig.hash_as_entrypoint(),
        ];
        let results = vec![r1.clone(), r2.clone(), r3.clone()];

        built.set_transaction_results(vec![trig.clone()], &entry_hashes, results.clone());
        manual.set_transaction_results(vec![trig], &entry_hashes, results);

        // Compare roots and contents
        assert_eq!(built.header().merkle_root(), manual.header().merkle_root());
        assert_eq!(
            built.header().result_merkle_root(),
            manual.header().result_merkle_root()
        );
        assert_eq!(
            built.payload().transactions.len(),
            manual.payload().transactions.len()
        );
        assert_eq!(
            built.entrypoint_hashes().collect::<Vec<_>>(),
            manual.entrypoint_hashes().collect::<Vec<_>>()
        );
        assert_eq!(
            built.result_hashes().collect::<Vec<_>>(),
            manual.result_hashes().collect::<Vec<_>>()
        );
    }

    #[test]
    fn builder_attaches_da_bundle_and_sets_header_hash() {
        let header = BlockHeader::new(nonzero!(5_u64), None, None, None, 0, 0);
        let mut builder = BlockBuilder::new(header);
        let bundle = sample_da_bundle();
        builder.set_da_commitments(Some(bundle.clone()));
        let block = builder.build(BTreeSet::new());
        assert_eq!(block.da_commitments().unwrap(), &bundle);
        assert!(block.header().da_commitments_hash().is_some());
    }

    #[test]
    fn build_with_signature_keeps_da_policy_hash_consistent() {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let policy = DaProofPolicy {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::GLOBAL,
            alias: "lane-1".to_string(),
            proof_scheme: DaProofScheme::MerkleSha256,
        };
        let bundle = DaProofPolicyBundle::new(vec![policy]);
        let mut builder = BlockBuilder::new(header);
        builder.set_da_proof_policies(Some(bundle.clone()));
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let block = builder.build_with_signature(0, keypair.private_key());
        let signature = block.signatures().next().expect("block signature exists");
        assert!(
            signature
                .signature()
                .verify_hash(keypair.public_key(), block.hash())
                .is_ok(),
            "block signature should verify after DA policy attachment"
        );
        assert_eq!(
            block.header().da_proof_policies_hash,
            Some(HashOf::new(&bundle))
        );
    }

    fn sample_da_bundle() -> DaCommitmentBundle {
        let record = DaCommitmentRecord::new(
            LaneId::new(7),
            1,
            1,
            BlobDigest::new([0x11; 32]),
            ManifestDigest::new([0x22; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0x33; 32]),
            Some(KzgCommitment::new([0x44; 48])),
            Some(Hash::prehashed([0x55; 32])),
            RetentionPolicy::default(),
            StorageTicketId::new([0x66; 32]),
            Signature::from_bytes(&[0x77; 64]),
        );
        DaCommitmentBundle::new(vec![record])
    }
}
