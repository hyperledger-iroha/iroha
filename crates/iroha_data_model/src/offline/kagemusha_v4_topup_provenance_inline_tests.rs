// Kagemusha V4 provenance regressions are included at offline-model module scope.
#[cfg(test)]
mod kagemusha_v4_topup_provenance_tests {
    use iroha_crypto::HashOf;

    use super::*;
    use crate::{
        block::consensus_v2::{BlockSubject, ConsensusRound, ExecutionCommitment},
        domain::DomainId,
        peer::PeerId,
    };

    struct Fixture {
        statement: KagemushaRecursiveSpendPublicStatementV4,
        provenance: KagemushaRecursiveSpendTopUpProvenanceV4,
    }

    fn execution_commitment(seed: u8) -> ExecutionCommitment {
        let ordinary_writes_root = Hash::new([seed, 3]);
        let topup_anchor_root = Hash::new([seed, 4]);
        ExecutionCommitment::new_without_merge_carrier(
            Hash::new([seed, 1]),
            ExecutionCommitment::topup_post_state_root(1, ordinary_writes_root, topup_anchor_root),
            ordinary_writes_root,
            Some(topup_anchor_root),
            1,
            1,
            Hash::new([seed, 5]),
        )
        .expect("test execution commitment")
    }

    fn evidence(
        chain_id: &ChainId,
        asset: &AssetDefinitionId,
        binding: &KagemushaRecursiveSpendArtifactBindingV4,
        seed: u8,
    ) -> KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
        let payer_key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic payer key");
        let payer = AccountId::new(payer_key.public_key().clone());
        let amount = KagemushaScaledAmountV2::new(500, 2).expect("test amount");
        let anchor = KagemushaRecursiveSpendTopUpAnchorV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_TOPUP_ANCHOR_VERSION_V4,
            chain_id: chain_id.clone(),
            payer: payer.clone(),
            asset: AssetId::new(asset.clone(), payer),
            asset_scale: 2,
            amount,
            initial_root: [seed.wrapping_add(1); 32],
            finalized_root: [seed.wrapping_add(2); 32],
            shield_leaf_index: u32::from(seed),
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id: chain_id.clone(),
                asset: asset.clone(),
                note_commitment: [seed.wrapping_add(3); 32],
                spend_nullifier: [seed.wrapping_add(4); 32],
                amount,
            },
            topup_operation_id: [seed.wrapping_add(5); 32],
            shield_verifier_id: VerifyingKeyId::new("halo2/ipa", "topup-shield-v2"),
            shield_verifier_commitment: [seed.wrapping_add(6); 32],
            artifact_binding: binding.clone(),
            finalized_height: 42,
            finalized_tx_hash: [seed.wrapping_add(7); 32],
            anchor_digest: [0; 32],
        }
        .finalize_digest()
        .expect("test anchor");
        let context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new([seed, 8])));
        let round = ConsensusRound {
            context_id,
            height: anchor.finalized_height,
            view: 0,
        };
        let certificate = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject: BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 9])),
                payload_hash: Hash::new([seed, 10]),
            },
            execution_commitment: execution_commitment(seed),
            signers: vec![0],
            aggregate_signature: vec![seed; 96],
        };
        let proof = KagemushaTopUpFinalityProofV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_PROOF_VERSION_V2,
            anchor: anchor.compact_ref().expect("test anchor ref"),
            commit_qc: KagemushaTopUpFinalityCompactQcV2 {
                height_context: KagemushaTopUpFinalityHeightContextV2 {
                    context_id,
                    chain_id: chain_id.clone(),
                    protocol_version: PROTOCOL_VERSION,
                    height: anchor.finalized_height,
                    epoch: 0,
                    epoch_end_height: 100,
                    next_epoch_snapshot: None,
                    mode: ConsensusMode::Permissioned,
                    parent_commit_qc: None,
                    snapshot_bootstrap: None,
                    nexus_amx_context_hash: Hash::new([seed, 11]),
                    execution_policy_hash: Hash::new([seed, 12]),
                    da_layout: DataAvailabilityLayout {
                        encoding: crate::block::consensus_v2::PayloadEncoding::Plain,
                        chunk_size_bytes: 1024,
                        data_shards: 0,
                        parity_shards: 0,
                        max_payload_size_bytes: 4096,
                        max_chunk_count: 4,
                    },
                    leader_seed: [seed.wrapping_add(12); 32],
                },
                certificate,
            },
            anchor_path: KagemushaTopUpAnchorMerkleProofV2 {
                leaf_index: 0,
                leaf_count: 1,
                siblings: Vec::new(),
            },
        };
        KagemushaRecursiveSpendTopUpFinalityEvidenceV4 {
            topup_anchor: anchor,
            topup_finality_proof: proof,
        }
    }

    fn fixture_with_seeds(seeds: &[u8]) -> Fixture {
        let chain_id = ChainId::from("kagemusha-provenance-test-chain");
        let asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("test domain"),
            "rose".parse().expect("test asset name"),
        );
        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "provenance-test-release".to_owned(),
            manifest_sha256: [0x51; 32],
        };
        let validator_key = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::BlsNormal)
            .expect("deterministic validator key");
        let roster = KagemushaTopUpFinalityRosterArtifactV2 {
            version: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_VERSION_V2,
            chain_id: chain_id.clone(),
            artifact_generation: binding.generation.clone(),
            windows: vec![KagemushaTopUpFinalityRosterWindowV2 {
                activates_at_height: 1,
                withdraws_at_height: 100,
                consensus_mode: ConsensusMode::Permissioned,
                validator_set: vec![ValidatorPower {
                    validator: PeerId::new(validator_key.public_key().clone()),
                    power: 1,
                }],
                validator_set_pops: vec![[0x62; 96]],
            }],
        };
        let mut evidence = seeds
            .iter()
            .map(|seed| evidence(&chain_id, &asset, &binding, *seed))
            .collect::<Vec<_>>();
        evidence.sort_unstable_by_key(|item| item.topup_anchor.compact_ref().expect("anchor ref"));
        let topup_anchor_refs = evidence
            .iter()
            .map(|item| item.topup_anchor.compact_ref().expect("anchor ref"))
            .collect();
        let statement = KagemushaRecursiveSpendPublicStatementV4 {
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            asset_scale: 2,
            final_root: [0x71; 32],
            next_zero_leaf_index: 7,
            topup_anchor_refs,
            proof_step_count: 2,
            peer_hop_count: 1,
            current_note: KagemushaSpendableNoteDescriptorV2 {
                chain_id,
                asset,
                note_commitment: [0x72; 32],
                spend_nullifier: [0x73; 32],
                amount: KagemushaScaledAmountV2::new(400, 2).expect("branch amount"),
            },
            branch_claims: Vec::new(),
            transition: None,
            artifact_binding: binding.clone(),
            verifier_key_id: kagemusha_recursive_spend_verifier_key_id_v4(
                KagemushaPastaCycleParityV1::StepEq,
                binding.manifest_sha256,
            ),
        };
        Fixture {
            statement,
            provenance: KagemushaRecursiveSpendTopUpProvenanceV4 {
                topup_finality_roster_artifact: roster,
                topup_finality_evidence: evidence,
            },
        }
    }

    fn rejects(
        fixture: &Fixture,
        provenance: &KagemushaRecursiveSpendTopUpProvenanceV4,
        height: u64,
    ) {
        assert!(
            provenance
                .validate_for_statement_at_height(&fixture.statement, Some(height))
                .is_err()
        );
    }

    #[test]
    fn provenance_rejects_zero_many_duplicate_reordered_and_wrong_refs() {
        let fixture = fixture_with_seeds(&[0x11, 0x21]);
        fixture
            .provenance
            .validate_for_statement_at_height(&fixture.statement, Some(50))
            .expect("canonical two-origin provenance");

        let mut zero = fixture.provenance.clone();
        zero.topup_finality_evidence.clear();
        rejects(&fixture, &zero, 50);

        let mut many = fixture.provenance.clone();
        many.topup_finality_evidence
            .push(fixture.provenance.topup_finality_evidence[0].clone());
        rejects(&fixture, &many, 50);

        let mut duplicate = fixture.provenance.clone();
        duplicate.topup_finality_evidence[1] = duplicate.topup_finality_evidence[0].clone();
        rejects(&fixture, &duplicate, 50);

        let mut reordered = fixture.provenance.clone();
        reordered.topup_finality_evidence.reverse();
        rejects(&fixture, &reordered, 50);

        let mut wrong_ref_fixture = fixture_with_seeds(&[0x11]);
        wrong_ref_fixture.statement.topup_anchor_refs[0] =
            KagemushaRecursiveSpendTopUpAnchorRefV2 {
                topup_operation_id: [0xe1; 32],
                anchor_digest: [0xe2; 32],
            };
        rejects(&wrong_ref_fixture, &wrong_ref_fixture.provenance, 50);
    }

    #[test]
    fn topup_finality_height_context_rejects_zero_execution_policy_hash() {
        let fixture = fixture_with_seeds(&[0x22]);
        let mut context = fixture.provenance.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .height_context
            .clone();
        context.execution_policy_hash = Hash::prehashed([0; Hash::LENGTH]);

        assert!(matches!(
            context.validate_structure(),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "topup_finality.height_context.execution_policy_hash"
            })
        ));
    }

    #[test]
    fn provenance_rejects_wrong_context_binding_window_height_qc_and_size() {
        let fixture = fixture_with_seeds(&[0x31]);

        let mut wrong_chain = fixture.statement.clone();
        wrong_chain.chain_id = ChainId::from("wrong-chain");
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_chain, Some(50))
                .is_err()
        );

        let mut wrong_asset = fixture.statement.clone();
        wrong_asset.asset = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("test domain"),
            "wrong".parse().expect("test asset name"),
        );
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_asset, Some(50))
                .is_err()
        );

        let mut wrong_scale = fixture.statement.clone();
        wrong_scale.asset_scale = 3;
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_scale, Some(50))
                .is_err()
        );

        let mut wrong_binding = fixture.statement.clone();
        wrong_binding.artifact_binding.manifest_sha256[0] ^= 1;
        assert!(
            fixture
                .provenance
                .validate_for_statement_at_height(&wrong_binding, Some(50))
                .is_err()
        );

        let mut wrong_generation = fixture.provenance.clone();
        wrong_generation
            .topup_finality_roster_artifact
            .artifact_generation = "other-release".to_owned();
        rejects(&fixture, &wrong_generation, 50);

        let mut wrong_window = fixture.provenance.clone();
        wrong_window.topup_finality_roster_artifact.windows[0].withdraws_at_height = 42;
        rejects(&fixture, &wrong_window, 50);

        rejects(&fixture, &fixture.provenance, 41);

        let mut wrong_qc = fixture.provenance.clone();
        wrong_qc.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .height_context
            .height = 43;
        rejects(&fixture, &wrong_qc, 50);

        let mut oversized = fixture.provenance.clone();
        let mut parent_qc = oversized.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .certificate
            .clone();
        parent_qc.aggregate_signature =
            vec![0x81; KAGEMUSHA_TOPUP_FINALITY_PROOF_MAX_BYTES_USIZE_V2 + 1];
        oversized.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .height_context
            .parent_commit_qc = Some(parent_qc);
        rejects(&fixture, &oversized, 50);
    }

    #[test]
    fn compact_qc_rejects_foreign_or_future_proposal_origin() {
        let fixture = fixture_with_seeds(&[0x32]);
        let compact_qc = &fixture.provenance.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc;
        compact_qc
            .validate_structure()
            .expect("fixture compact QC structure");

        let mut future = compact_qc.clone();
        future.certificate.proposal_round.view = future.certificate.round.view.saturating_add(1);
        assert!(future.validate_structure().is_err());

        let mut foreign_context = compact_qc.clone();
        foreign_context.certificate.proposal_round.context_id = HeightContextId(
            HashOf::from_untyped_unchecked(Hash::new(b"foreign compact QC proposal context")),
        );
        assert!(foreign_context.validate_structure().is_err());

        let mut foreign_height = compact_qc.clone();
        foreign_height.certificate.proposal_round.height =
            foreign_height.certificate.round.height.saturating_add(1);
        assert!(foreign_height.validate_structure().is_err());
    }

    #[test]
    fn provenance_merge_requires_one_exact_roster_and_exact_shared_evidence() {
        let left = fixture_with_seeds(&[0x11]);
        let right = fixture_with_seeds(&[0x21]);
        let merged = KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
            &[
                (&left.statement, &left.provenance),
                (&right.statement, &right.provenance),
            ],
            50,
        )
        .expect("two exact inventories merge canonically");
        assert_eq!(merged.topup_finality_evidence.len(), 2);
        assert!(
            merged
                .anchor_refs()
                .expect("merged refs")
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );

        let mut wrong_roster = right.provenance.clone();
        wrong_roster.topup_finality_roster_artifact.windows[0].validator_set_pops[0][0] ^= 1;
        assert!(
            KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
                &[
                    (&left.statement, &left.provenance),
                    (&right.statement, &wrong_roster),
                ],
                50,
            )
            .is_err()
        );

        let shared = fixture_with_seeds(&[0x41]);
        let coalesced = KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
            &[
                (&shared.statement, &shared.provenance),
                (&shared.statement, &shared.provenance),
            ],
            50,
        )
        .expect("identical shared origin is coalesced");
        assert_eq!(coalesced.topup_finality_evidence.len(), 1);

        let mut conflicting = shared.provenance.clone();
        conflicting.topup_finality_evidence[0]
            .topup_finality_proof
            .commit_qc
            .certificate
            .aggregate_signature[0] ^= 1;
        assert!(
            KagemushaRecursiveSpendTopUpProvenanceV4::merge_for_statements_at(
                &[
                    (&shared.statement, &shared.provenance),
                    (&shared.statement, &conflicting),
                ],
                50,
            )
            .is_err()
        );
    }
}

impl KagemushaRecursiveSpendVerifyRequestV4 {
    /// Validate the terminal receiver request and every V4 proof/provenance binding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.artifact_binding.validate()?;
        self.recipient_request.validate_at(self.verified_at_ms)?;
        let statement = &self.bundle.statement;
        if self.verified_at_ms == 0
            || self.block_height == 0
            || self.maximum_hops == 0
            || self.maximum_hops > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || statement.peer_hop_count > self.maximum_hops
            || self.artifact_binding != statement.artifact_binding
            || self.recipient_request.chain_id != statement.chain_id
            || self.recipient_request.asset != statement.asset
            || self.recipient_request.amount.scale != statement.asset_scale
            || statement.current_note != self.recipient_request.recipient_output
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4",
            });
        }
        self.topup_provenance
            .validate_for_bundle_at(&self.bundle, self.block_height)?;
        let Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) =
            statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.transition",
            });
        };
        if transition.branch != KagemushaRecursiveSpendBranchV2::Recipient
            || transition.recipient_request_digest != self.recipient_request.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_request.v4.recipient_binding",
            });
        }
        let encoded_len = norito::encode_canonical(self)?.len();
        if encoded_len > KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V4 {
            return Err(KagemushaValidationError::EncodedSizeExceeded {
                actual: encoded_len,
                max: KAGEMUSHA_RECURSIVE_SPEND_VERIFY_REQUEST_MAX_BYTES_V4,
            });
        }
        Ok(())
    }

    /// Return the V4-domain binding of request, exact output note, and opaque bundle.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn request_output_binding_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRequestOutputBindingDigestPreimageV4 {
            domain: KAGEMUSHA_REQUEST_OUTPUT_BINDING_DIGEST_DOMAIN_V4.to_owned(),
            recipient_request_digest: self.recipient_request.digest()?,
            recipient_output: self.bundle.statement.current_note.clone(),
            bundle_digest: self.bundle.digest()?,
        })
    }
}

impl KagemushaRecursiveSpendVerifyResultV4 {
    /// Enforce the single successful ABI-21 receiver-acceptance contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.summary.amount.validate()?;
        self.summary.artifact_binding.validate()?;
        let expected_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            self.summary.artifact_binding.manifest_sha256,
        );
        validate_kagemusha_recursive_spend_branch_claims_v2(&self.summary.branch_claims)?;
        let activation_window_valid = matches!(
            (self.verifier_activation_height, self.verifier_withdraw_height),
            (Some(activation), Some(withdrawal))
                if activation > 0
                    && activation < withdrawal
                    && self.verified_at_block_height >= activation
                    && self.verified_at_block_height < withdrawal
        );
        if !self.valid
            || !self.chain_admissible
            || !self.lineage_redeemable
            || !self.witnessless_redemption_supported
            || self.recipient_request_digest == [0; 32]
            || self.request_output_binding_digest == [0; 32]
            || self.summary.bundle_digest == [0; 32]
            || self.verifier_circuit_id != KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4
            || self.verified_at_block_height == 0
            || self.verified_at_ms == 0
            || !activation_window_valid
            || self.summary.verifier_key_id != self.verifier_key_id
            || self.verifier_key_id != expected_verifier_key_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "verify_result.v4",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendRedeemBuildRequestV4 {
    /// Validate the common full/partial ABI-21 redemption-builder input.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.public_amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.unshield_proof)?;
        let statement = &self.bundle.statement;
        if self.block_height == 0
            || self.operation_id == [0; 32]
            || self.operation_id != self.redemption.operation_id
            || self.recipient != self.redemption.recipient
            || self.public_amount != self.redemption.public_amount
            || self.redemption.chain_id != statement.chain_id
            || self.redemption.asset != statement.asset
            || self.redemption.parent_bundle_digest != self.bundle.digest()?
            || self.redemption.input_note != statement.current_note
            || self.redemption.input_root != statement.final_root
            || self.redemption.parent_branch_claims != statement.branch_claims
            || self.redemption.parent_topup_anchor_refs != statement.topup_anchor_refs
            || self.redemption.parent_proof_step_count != statement.proof_step_count
            || self.redemption.parent_peer_hop_count != statement.peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_request.v4",
            });
        }
        match (
            &self.redemption.change_output,
            &self.redemption.change_artifact_binding,
        ) {
            (None, None) => Ok(()),
            (Some(_), Some(binding)) if binding == &statement.artifact_binding => Ok(()),
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_request.v4.change",
            }),
        }
    }
}

impl KagemushaRecursiveSpendRedeemChangeBranchV4 {
    /// Validate the sole continuing child of a partial redemption.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_redemption(
        &self,
        input_bundle: &KagemushaRecursiveSpendBundleV4,
        redemption: &KagemushaRecursiveSpendRedemptionIntentV4,
    ) -> Result<(), KagemushaValidationError> {
        input_bundle.validate_public_binding()?;
        redemption.validate_public_binding()?;
        self.output.validate_public_binding()?;
        self.bundle.validate_public_binding()?;
        let expected_output = redemption.change_output.as_ref().ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.v4",
            },
        )?;
        let expected_binding = redemption.change_artifact_binding.as_ref().ok_or(
            KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.v4",
            },
        )?;
        let expected_claims = redemption.change_branch_claims()?;
        if &self.output != expected_output
            || self.branch_claims != expected_claims
            || self.bundle.statement.current_note != self.output
            || self.bundle.statement.branch_claims != self.branch_claims
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendNote {
                field: "offline_change.v4.branch",
            });
        }
        let Some(KagemushaRecursiveSpendTransitionV4::RedemptionChange(transition)) =
            self.bundle.statement.transition.as_ref()
        else {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.v4.transition",
            });
        };
        if transition.binding_digest != redemption.binding_digest()?
            || transition.parent_bundle_digest != redemption.parent_bundle_digest
            || transition.operation_id != redemption.operation_id
            || transition.parent_proof_step_count != redemption.parent_proof_step_count
            || transition.parent_peer_hop_count != redemption.parent_peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.v4.transition",
            });
        }
        let input = &input_bundle.statement;
        let change = &self.bundle.statement;
        let expected_verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            expected_binding.manifest_sha256,
        );
        if redemption.parent_bundle_digest != input_bundle.digest()?
            || redemption.input_note != input.current_note
            || redemption.parent_branch_claims != input.branch_claims
            || redemption.parent_topup_anchor_refs != input.topup_anchor_refs
            || redemption.parent_proof_step_count != input.proof_step_count
            || redemption.parent_peer_hop_count != input.peer_hop_count
            || redemption.input_root != input.final_root
            || change.chain_id != input.chain_id
            || change.asset != input.asset
            || change.asset_scale != input.asset_scale
            || change.final_root == input.final_root
            || input.next_zero_leaf_index.checked_add(1) != Some(change.next_zero_leaf_index)
            || change.topup_anchor_refs != input.topup_anchor_refs
            || input.proof_step_count == 0
            || input.proof_step_count.checked_add(1) != Some(change.proof_step_count)
            || change.peer_hop_count != input.peer_hop_count
            || &change.artifact_binding != expected_binding
            || change.verifier_key_id != expected_verifier_key_id
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "offline_change.v4.parent_binding",
            });
        }
        Ok(())
    }
}

impl KagemushaRecursiveSpendRedeemUnsignedV4 {
    /// Validate exact full-terminal or partial-with-one-change redemption semantics.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.bundle.validate_public_binding()?;
        self.redemption.validate_public_binding()?;
        self.amount.validate()?;
        validate_kagemusha_redeem_proof_attachment_v2(&self.redeem_proof)?;
        let statement = &self.bundle.statement;
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4
            || self.block_height == 0
            || self.operation_id == [0; 32]
            || self.operation_id != self.redemption.operation_id
            || self.recipient != self.redemption.recipient
            || self.amount != self.redemption.public_amount
            || self.redemption.chain_id != statement.chain_id
            || self.redemption.asset != statement.asset
            || self.redemption.parent_bundle_digest != self.bundle.digest()?
            || self.redemption.input_note != statement.current_note
            || self.redemption.input_root != statement.final_root
            || self.redemption.parent_branch_claims != statement.branch_claims
            || self.redemption.parent_topup_anchor_refs != statement.topup_anchor_refs
            || self.redemption.parent_proof_step_count != statement.proof_step_count
            || self.redemption.parent_peer_hop_count != statement.peer_hop_count
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_request.v4",
            });
        }
        match (&self.redemption.change_output, &self.offline_change) {
            (None, None) => Ok(()),
            (Some(_), Some(change)) => {
                change.validate_for_redemption(&self.bundle, &self.redemption)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_request.v4.offline_change",
            }),
        }
    }

    /// Return the exact V4 authorization payload digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_public_binding()?;
        kagemusha_poseidon_preimage(&KagemushaRedeemUnsignedPayloadDigestPreimageV4 {
            domain: KAGEMUSHA_REDEEM_PAYLOAD_DIGEST_DOMAIN_V4.to_owned(),
            version: self.version,
            bundle: self.bundle.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            redeem_proof: self.redeem_proof.clone(),
            redemption: self.redemption.clone(),
            offline_change: self.offline_change.clone(),
            block_height: self.block_height,
            operation_id: self.operation_id,
        })
    }

    /// Attach the matching recipient authorization without altering any signed field.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn into_request(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemRequestV4, KagemushaValidationError> {
        let request = KagemushaRecursiveSpendRedeemRequestV4 {
            version: self.version,
            bundle: self.bundle,
            recipient: self.recipient,
            amount: self.amount,
            redeem_proof: self.redeem_proof,
            redemption: self.redemption,
            offline_change: self.offline_change,
            block_height: self.block_height,
            operation_id: self.operation_id,
            authorization,
        };
        request.validate_public_binding()?;
        Ok(request)
    }
}

impl KagemushaRecursiveSpendRedeemBuildResultV4 {
    /// Validate the atomic unsigned request plus its optional change/witness package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        self.unsigned.validate_public_binding()?;
        if self.operation_id == [0; 32]
            || self.operation_id != self.unsigned.operation_id
            || self.authorization_digest != self.unsigned.digest()?
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4",
            });
        }
        match (
            &self.unsigned.offline_change,
            &self.offline_change_bundle,
            &self.offline_change_membership_witness,
            &self.offline_change_topup_provenance,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness), Some(provenance))
                if &branch.bundle == bundle =>
            {
                branch.validate_for_redemption(&self.unsigned.bundle, &self.unsigned.redemption)?;
                witness.validate_for_statement_v4(&bundle.statement)?;
                provenance.validate_for_bundle(bundle)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4.offline_change",
            }),
        }
    }

    /// Validate this prepared result against the exact builder input.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_for_request(
        &self,
        request: &KagemushaRecursiveSpendRedeemBuildRequestV4,
    ) -> Result<(), KagemushaValidationError> {
        request.validate_public_binding()?;
        self.validate_public_binding()?;
        if self.operation_id != request.operation_id
            || self.unsigned.bundle != request.bundle
            || self.unsigned.recipient != request.recipient
            || self.unsigned.amount != request.public_amount
            || self.unsigned.redeem_proof != request.unshield_proof
            || self.unsigned.redemption != request.redemption
            || self.unsigned.block_height != request.block_height
            || self.offline_change_bundle.is_some() != request.redemption.change_output.is_some()
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_build_result.v4.request",
            });
        }
        Ok(())
    }

    /// Attach authorization and retain local change membership state.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn into_redeem_result(
        self,
        authorization: KagemushaRequestAuthorizationV2,
    ) -> Result<KagemushaRecursiveSpendRedeemResultV4, KagemushaValidationError> {
        self.validate_public_binding()?;
        let operation_id = self.operation_id;
        let offline_change_bundle = self.offline_change_bundle;
        let offline_change_membership_witness = self.offline_change_membership_witness;
        let offline_change_topup_provenance = self.offline_change_topup_provenance;
        let request = self.unsigned.into_request(authorization)?;
        let result = KagemushaRecursiveSpendRedeemResultV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            redeem_request_archive: norito::encode_canonical(&request)?,
            offline_change_bundle,
            offline_change_membership_witness,
            offline_change_topup_provenance,
            operation_id,
        };
        result.validate_public_binding()?;
        Ok(result)
    }
}

impl KagemushaRecursiveSpendRedeemRequestV4 {
    /// Reconstruct the exact canonical V4 fields covered by authorization.
    #[must_use]
    pub fn unsigned_payload(&self) -> KagemushaRecursiveSpendRedeemUnsignedV4 {
        KagemushaRecursiveSpendRedeemUnsignedV4 {
            version: self.version,
            bundle: self.bundle.clone(),
            recipient: self.recipient.clone(),
            amount: self.amount,
            redeem_proof: self.redeem_proof.clone(),
            redemption: self.redemption.clone(),
            offline_change: self.offline_change.clone(),
            block_height: self.block_height,
            operation_id: self.operation_id,
        }
    }

    /// Validate exact conservation and the self-contained recipient authorization.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        let encoded_len = norito::encode_canonical(self)?.len();
        ensure_kagemusha_encoded_size_at_most(
            encoded_len,
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4,
        )?;
        let unsigned = self.unsigned_payload();
        unsigned.validate_public_binding()?;
        if self.authorization.operation_id != self.operation_id
            || self.authorization.authority != self.recipient
            || self.authorization.asset_definition_id != self.bundle.statement.asset
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "authorization.v4",
            });
        }
        self.authorization.validate_for_payload(unsigned.digest()?)
    }

    /// Return the digest of every unsigned V4 redemption field.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn unsigned_payload_digest(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.unsigned_payload().digest()
    }

    /// Verify recipient authorization at authoritative Torii time.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_authorization_at(&self, now_ms: u64) -> Result<(), KagemushaValidationError> {
        self.validate_public_binding()?;
        self.authorization
            .validate_for_payload_at(self.unsigned_payload_digest()?, now_ms)
    }
}

impl KagemushaRecursiveSpendRedeemResultV4 {
    /// Validate the canonical request archive and terminal/change result shape.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaValidationError> {
        if self.version != KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4
            || self.operation_id == [0; 32]
            || self.redeem_request_archive.is_empty()
            || self.redeem_request_archive.len()
                > KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4",
            });
        }
        preflight_kagemusha_redeem_request_archive_v4(&self.redeem_request_archive).map_err(
            |_| KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.request_archive",
            },
        )?;
        let request: KagemushaRecursiveSpendRedeemRequestV4 = norito::decode_canonical_with_limits(
            &self.redeem_request_archive,
            kagemusha_recursive_spend_redeem_decode_limits_v4(self.redeem_request_archive.len()),
        )
        .map_err(|_| KagemushaValidationError::InvalidRecursiveSpendProof {
            field: "redeem_result.v4.request_archive",
        })?;
        request.validate_public_binding()?;
        if request.operation_id != self.operation_id {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.operation_id",
            });
        }
        match (
            &request.offline_change,
            &self.offline_change_bundle,
            &self.offline_change_membership_witness,
            &self.offline_change_topup_provenance,
        ) {
            (None, None, None, None) => Ok(()),
            (Some(branch), Some(bundle), Some(witness), Some(provenance))
                if &branch.bundle == bundle =>
            {
                branch.validate_for_redemption(&request.bundle, &request.redemption)?;
                witness.validate_for_statement_v4(&bundle.statement)?;
                provenance.validate_for_bundle(bundle)
            }
            _ => Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.offline_change",
            }),
        }
    }
}
