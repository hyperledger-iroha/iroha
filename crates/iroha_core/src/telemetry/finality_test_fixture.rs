// Actual execution and four-validator finality for telemetry publication fixtures.
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};

impl SystemUnderTest {
    pub(super) fn finality_context(&self, header: &BlockHeader) -> wire::HeightContext {
        if !header.is_genesis() {
            let parent = self
                .kura
                .v2_finality_artifact(header.height().get() - 1)
                .expect("read exact telemetry parent finality")
                .expect("parent finality is durable before a successor");
            return crate::sumeragi::v2_context::build_successor_height_context(
                &parent,
                parent.height_context.nexus_amx_context_hash,
                None,
            )
            .expect("successor inherits its authenticated parent authority");
        }
        let roster = self
            .finality_keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                self.network_id,
                0,
                &roster,
            );
        wire::HeightContext {
            network_id: self.network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("exact four-validator quorum"),
            roster,
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"telemetry finality nexus context"),
            execution_policy_hash: Hash::new(b"telemetry finality execution policy"),
            da_layout: wire::SumeragiV2GenesisContextParameters::recommended().da_layout,
            leader_seed: [0xD3; 32],
        }
    }

    pub(super) fn finalize_block(
        &self,
        block: SignedBlock,
        state_block: &mut crate::state::StateBlock<'_>,
        context: wire::HeightContext,
    ) -> CommittedBlock {
        let (committed, witness) = self.certify_block(block, state_block, context);
        self.persist_finality(&committed);
        state_block
            .authorize_execution_output_publication(&committed, &witness)
            .expect(
                "actual captured witness and durable finality authorize exact output publication",
            );
        committed
    }

    pub(super) fn certify_block(
        &self,
        block: SignedBlock,
        state_block: &mut crate::state::StateBlock<'_>,
        context: wire::HeightContext,
    ) -> (
        CommittedBlock,
        iroha_data_model::block::consensus::ExecWitness,
    ) {
        use crate::sumeragi::exec::{
            LaneFinalityManifestV1, NativeAmxApplicationManifestV1,
            execution_commitment_from_validated_block,
        };
        let witness = state_block
            .take_exec_witness()
            .expect("actual captured execution witness");
        let casting = state_block
            .take_parliament_timed_ovn_casting_bindings()
            .expect("actual captured casting bindings");
        let native =
            NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(&block, None)
                .expect("exact executed block manifest");
        let lanes = LaneFinalityManifestV1::from_result_bearing_block(&block)
            .expect("exact executed lane manifest");
        let execution_commitment =
            execution_commitment_from_validated_block(&witness, &native, &lanes, &block)
                .expect("finality votes bind actual execution and complete canonical block bytes");
        let subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("canonical proposal bytes"),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: block.header().view_change_index(),
        };
        let mut certificate = wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let signatures = self
            .finality_keys
            .iter()
            .take(3)
            .map(|key| {
                iroha_crypto::Signature::try_new(key.private_key(), &preimage)
                    .expect("sign actual telemetry execution")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate exactly three equal-power validator votes");
        let pops = self
            .finality_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("validator PoP")
            })
            .collect();
        let artifact = wire::finality::V2FinalityArtifact::new(context, subject, certificate, pops);
        let verified = crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())
            .expect("telemetry finality passes native cryptographic verification");
        let committed = crate::block::ValidBlock::new_unverified_for_tests(block)
            .commit_with_verified_v2_artifact(verified, execution_commitment)
            .unpack(|_| {})
            .expect("verified finality binds exact executed block");
        self.kura
            .stage_kagemusha_finality_sidecar(
                artifact.height,
                artifact.block_hash,
                &witness,
                execution_commitment,
                &casting,
            )
            .expect("retain actual finality witness before publication");
        (committed, witness)
    }

    pub(super) fn persist_finality(&self, block: &CommittedBlock) {
        let artifact = block
            .verified_v2_finality_artifact()
            .expect("verified committed authority");
        self.kura
            .store_block(block.clone())
            .expect("persist exact result-bearing block");
        let receipt = self
            .kura
            .store_v2_finality_artifact(artifact)
            .expect("persist exact finality before State apply");
        assert_eq!(receipt.block_hash(), artifact.block_hash);
        assert_eq!(receipt.artifact_hash(), iroha_crypto::HashOf::new(artifact));
    }

    pub(super) fn promote_finality(&self, block: &CommittedBlock) {
        let artifact = block
            .verified_v2_finality_artifact()
            .expect("verified committed authority");
        let receipt = self
            .kura
            .store_v2_finality_artifact(artifact)
            .expect("exact durable finality receipt");
        self.kura
            .promote_kagemusha_finality_sidecar(artifact, &receipt)
            .expect("publish finality witness only after its actual State publication");
    }
}
