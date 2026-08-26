#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::codec::DecodeAll as _;
    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }
    #[test]
    fn consensus_modes_project_canonical_protocol_identities() {
        assert_eq!(ConsensusMode::Permissioned.tag(), PERMISSIONED_TAG);
        assert_eq!(ConsensusMode::Npos.tag(), NPOS_TAG);
        assert_eq!(
            ConsensusMode::Permissioned.bls_domain(),
            PERMISSIONED_BLS_DOMAIN
        );
        assert_eq!(ConsensusMode::Npos.bls_domain(), NPOS_BLS_DOMAIN);
        assert!(ConsensusMode::Permissioned.is_permissioned());
        assert!(!ConsensusMode::Npos.is_permissioned());
        for mode in [ConsensusMode::Permissioned, ConsensusMode::Npos] {
            let parameter_mode = crate::parameter::system::SumeragiConsensusMode::from(mode);
            assert_eq!(ConsensusMode::from(parameter_mode), mode);
        }
    }
    #[test]
    fn global_phase_wire_tags_are_explicit_and_schema_aligned() {
        let prepare = GlobalPhase::Prepare.encode();
        let commit = GlobalPhase::Commit.encode();
        assert_eq!(prepare, u32::from(GlobalPhase::Prepare as u8).to_le_bytes());
        assert_eq!(commit, u32::from(GlobalPhase::Commit as u8).to_le_bytes());
        assert_eq!(prepare, 1_u32.to_le_bytes());
        assert_eq!(commit, 2_u32.to_le_bytes());
        let mut prepare_cursor = prepare.as_slice();
        let mut commit_cursor = commit.as_slice();
        assert_eq!(
            GlobalPhase::decode_all(&mut prepare_cursor).expect("decode Prepare"),
            GlobalPhase::Prepare
        );
        assert_eq!(
            GlobalPhase::decode_all(&mut commit_cursor).expect("decode Commit"),
            GlobalPhase::Commit
        );
        let legacy_implicit_zero_bytes = 0_u32.to_le_bytes();
        let mut legacy_implicit_zero = legacy_implicit_zero_bytes.as_slice();
        assert!(GlobalPhase::decode_all(&mut legacy_implicit_zero).is_err());
    }
    #[test]
    fn payload_encoding_uses_natural_zero_tag_and_rejects_retired_tag_one() {
        let canonical = PayloadEncoding::ReedSolomon16.encode();
        assert_eq!(canonical, 0_u32.to_le_bytes());
        assert_eq!(
            PayloadEncoding::decode_all(&mut canonical.as_slice())
                .expect("decode canonical RS16 payload encoding"),
            PayloadEncoding::ReedSolomon16
        );
        let retired_tag = 1_u32.to_le_bytes();
        assert!(
            PayloadEncoding::decode_all(&mut retired_tag.as_slice()).is_err(),
            "retired payload-encoding tag 1 must fail closed"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn payload_encoding_json_rejects_retired_plain_variant() {
        let canonical = norito::json::to_value(&PayloadEncoding::ReedSolomon16)
            .expect("serialize canonical RS16 payload encoding");
        assert_eq!(
            norito::json::from_value::<PayloadEncoding>(canonical.clone())
                .expect("decode canonical RS16 payload encoding"),
            PayloadEncoding::ReedSolomon16
        );
        let mut retired = canonical;
        let encoding = retired
            .as_object_mut()
            .expect("adjacently tagged payload encoding")
            .get_mut("encoding")
            .expect("payload encoding tag");
        assert_eq!(encoding.as_str(), Some("reed_solomon16"));
        *encoding = norito::json::Value::String("plain".to_owned());
        assert!(
            norito::json::from_value::<PayloadEncoding>(retired).is_err(),
            "retired Plain payload encoding must fail closed"
        );
    }
    #[test]
    fn execution_commitment_enforces_topup_shape_count_and_combined_root() {
        let parent = Hash::new(b"parent");
        let ordinary = Hash::new(b"ordinary writes");
        let topup = Hash::new(b"topup tree");
        let executed_block_wire = b"executed block wire";
        let executed_block_wire_len =
            u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64");
        let executed = Hash::new(executed_block_wire);
        let post = ExecutionCommitment::topup_post_state_root(2, ordinary, topup);
        let canonical = ExecutionCommitment::new_without_merge_carrier(
            parent,
            post,
            ordinary,
            Some(topup),
            2,
            executed_block_wire_len,
            executed,
        )
        .expect("canonical top-up commitment");
        assert_eq!(canonical.validate(), Ok(()));
        assert_eq!(canonical.executed_block_wire_hash, executed);
        let encoded = canonical.encode();
        let mut cursor = encoded.as_slice();
        assert_eq!(
            ExecutionCommitment::decode_all(&mut cursor).expect("decode execution commitment"),
            canonical
        );
        assert_eq!(
            ExecutionCommitment::new_without_merge_carrier(
                parent,
                Hash::new(b"wrong"),
                ordinary,
                Some(topup),
                2,
                executed_block_wire_len,
                executed,
            ),
            Err(ValidationError::ExecutionCommitmentPostRootMismatch)
        );
        assert_eq!(
            ExecutionCommitment::new_without_merge_carrier(
                parent,
                post,
                ordinary,
                Some(topup),
                0,
                executed_block_wire_len,
                executed,
            ),
            Err(ValidationError::InvalidExecutionCommitment)
        );
        assert_eq!(
            ExecutionCommitment::new_without_merge_carrier(
                parent,
                post,
                ordinary,
                Some(topup),
                MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK + 1,
                executed_block_wire_len,
                executed,
            ),
            Err(ValidationError::TooManyKagemushaTopupAnchors)
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the test exhaustively checks the first-release execution commitment shape"
    )]
    fn execution_commitment_enforces_native_amx_manifest_shape_and_bound() {
        #[derive(Encode)]
        struct LegacyExecutionCommitment {
            parent_state_root: Hash,
            post_state_root: Hash,
            ordinary_writes_root: Hash,
            topup_anchor_root: Option<Hash>,
            topup_anchor_count: u32,
            executed_block_wire_hash: Hash,
        }
        #[derive(Encode)]
        struct ExecutionCommitmentWithoutLaneFinalityManifest {
            parent_state_root: Hash,
            post_state_root: Hash,
            ordinary_writes_root: Hash,
            topup_anchor_root: Option<Hash>,
            topup_anchor_count: u32,
            native_amx_application_manifest_version: u16,
            native_amx_application_manifest_root: Hash,
            native_amx_application_manifest_count: u32,
            merge_carrier: Option<MergeCarrierCommitmentV1>,
            executed_block_wire_len: u64,
            executed_block_wire_hash: Hash,
        }
        let parent = Hash::new(b"native manifest parent");
        let post = Hash::new(b"native manifest post");
        let ordinary = Hash::new(b"native manifest ordinary");
        let executed_block_wire = b"native manifest executed wire";
        let executed_block_wire_len =
            u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64");
        let executed = Hash::new(executed_block_wire);
        let root = Hash::new(b"native manifest non-empty root");
        let empty = ExecutionCommitment::without_topups_or_merge_carrier(
            parent,
            post,
            ordinary,
            executed_block_wire_len,
            executed,
        );
        assert_eq!(
            empty.native_amx_application_manifest_root,
            native_amx_application_manifest_empty_root()
        );
        assert_eq!(empty.native_amx_application_manifest_count, 0);
        assert_eq!(empty.validate(), Ok(()));
        let legacy = LegacyExecutionCommitment {
            parent_state_root: parent,
            post_state_root: post,
            ordinary_writes_root: ordinary,
            topup_anchor_root: None,
            topup_anchor_count: 0,
            executed_block_wire_hash: executed,
        }
        .encode();
        let mut legacy_cursor = legacy.as_slice();
        assert!(
            ExecutionCommitment::decode_all(&mut legacy_cursor).is_err(),
            "the pre-manifest execution commitment must not decode implicitly"
        );
        let omitted_lane_finality = ExecutionCommitmentWithoutLaneFinalityManifest {
            parent_state_root: parent,
            post_state_root: post,
            ordinary_writes_root: ordinary,
            topup_anchor_root: None,
            topup_anchor_count: 0,
            native_amx_application_manifest_version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            native_amx_application_manifest_root: native_amx_application_manifest_empty_root(),
            native_amx_application_manifest_count: 0,
            merge_carrier: None,
            executed_block_wire_len,
            executed_block_wire_hash: executed,
        }
        .encode();
        let mut omitted_lane_finality_cursor = omitted_lane_finality.as_slice();
        assert!(
            ExecutionCommitment::decode_all(&mut omitted_lane_finality_cursor).is_err(),
            "the lane-finality manifest is a required execution-commitment wire field"
        );
        let canonical =
            ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
                parent,
                post,
                ordinary,
                None,
                0,
                NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                root,
                1,
                executed_block_wire_len,
                executed,
            )
            .expect("canonical Native AMX manifest commitment");
        assert_eq!(canonical.validate(), Ok(()));
        assert_eq!(
            ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
                parent,
                post,
                ordinary,
                None,
                0,
                NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                root,
                0,
                executed_block_wire_len,
                executed,
            ),
            Err(ValidationError::InvalidNativeAmxApplicationManifestCommitment)
        );
        assert_eq!(
            ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
                parent,
                post,
                ordinary,
                None,
                0,
                NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                root,
                MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES + 1,
                executed_block_wire_len,
                executed,
            ),
            Err(ValidationError::TooManyNativeAmxApplicationManifestLeaves)
        );
        assert_eq!(
            ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
                parent,
                post,
                ordinary,
                None,
                0,
                NATIVE_AMX_APPLICATION_MANIFEST_VERSION + 1,
                root,
                1,
                executed_block_wire_len,
                executed,
            ),
            Err(ValidationError::InvalidNativeAmxApplicationManifestVersion)
        );
    }
    #[test]
    fn native_amx_manifest_leaf_rejects_reordered_or_duplicate_members() {
        let member = |index: u64, source: u8| NativeAmxApplicationManifestMemberV1 {
            entrypoint_index: index,
            source_id: [source; Hash::LENGTH],
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::new([source, 1])),
            result_hash: HashOf::from_untyped_unchecked(Hash::new([source, 2])),
        };
        let mut leaf = NativeAmxApplicationManifestLeafV1 {
            version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            lane_id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(3),
            lane_incarnation: Hash::new(b"native leaf incarnation"),
            participant_height: 8,
            participant_view: 4,
            predecessor_height: 7,
            predecessor_descriptor_hash: Some(Hash::new(b"native leaf predecessor")),
            descriptor_hash: Hash::new(b"native leaf descriptor"),
            proposal_hash: Hash::new(b"native leaf proposal"),
            settlement_hash: HashOf::from_untyped_unchecked(Hash::new(b"native leaf settlement")),
            members: vec![member(1, 0x11), member(2, 0x22)],
            application_block_height: 21,
            application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"native leaf application",
            )),
            executed_block_wire_hash: Hash::new(b"native leaf executed wire"),
        };
        assert_eq!(leaf.validate(), Ok(()));
        leaf.members.swap(0, 1);
        assert_eq!(
            leaf.validate(),
            Err(ValidationError::InvalidNativeAmxApplicationManifestMembership)
        );
        leaf.members = vec![member(1, 0x11), member(2, 0x11)];
        assert_eq!(
            leaf.validate(),
            Err(ValidationError::InvalidNativeAmxApplicationManifestMembership)
        );
        leaf.members = vec![member(1, 0x11), member(2, 0x22)];
        leaf.predecessor_descriptor_hash = Some(Hash::prehashed([0; Hash::LENGTH]));
        assert_eq!(
            leaf.validate(),
            Err(ValidationError::InvalidNativeAmxApplicationManifestLeaf)
        );
        leaf.predecessor_descriptor_hash = Some(Hash::new(b"native leaf predecessor"));
        leaf.members[0].entrypoint_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
        assert_eq!(
            leaf.validate(),
            Err(ValidationError::InvalidNativeAmxApplicationManifestLeaf)
        );
        leaf.members[0] = member(1, 0x11);
        leaf.members[1].result_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
        assert_eq!(
            leaf.validate(),
            Err(ValidationError::InvalidNativeAmxApplicationManifestLeaf)
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn genesis_context_json_uses_explicit_policy_hash_names_only() {
        let parameters = SumeragiV2GenesisContextParameters::recommended();
        let json = norito::json::to_json(&parameters).expect("serialize v2 genesis context");
        assert!(json.contains("\"nexus_amx_context_hash\""));
        assert!(json.contains("\"execution_policy_hash\""));
        assert!(!json.contains("active_nexus_lane_hash"));
        let obsolete = json.replace("nexus_amx_context_hash", "active_nexus_lane_hash");
        assert!(
            norito::json::from_str::<SumeragiV2GenesisContextParameters>(&obsolete).is_err(),
            "the unreleased misleading field name must not remain an accepted live schema"
        );
        let missing_execution_policy = json.replace(
            "\"execution_policy_hash\"",
            "\"obsolete_execution_policy_hash\"",
        );
        assert!(
            norito::json::from_str::<SumeragiV2GenesisContextParameters>(&missing_execution_policy)
                .is_err(),
            "signed v2 genesis context must require the execution-policy commitment"
        );
        let unknown = json.replacen('{', "{\"unknown\":1,", 1);
        assert!(
            norito::json::from_str::<SumeragiV2GenesisContextParameters>(&unknown).is_err(),
            "signed v2 genesis context must reject unknown fields"
        );
    }
    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic Sumeragi v2 fixture keypair");
        PeerId::new(key_pair.public_key().clone())
    }
    fn roster(powers: &[u64]) -> Vec<ValidatorPower> {
        let mut validators = (0..powers.len())
            .map(|index| peer(u8::try_from(index + 1).expect("small fixture roster")))
            .collect::<Vec<_>>();
        validators.sort();
        validators
            .into_iter()
            .zip(powers.iter().copied())
            .map(|(validator, power)| ValidatorPower { validator, power })
            .collect()
    }
    fn context(powers: &[u64]) -> HeightContext {
        let roster = roster(powers);
        HeightContext {
            network_id: network_id(0xA1),
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 2,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 4,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1024,
                max_chunk_count: 512,
            },
            leader_seed: [0xA5; 32],
        }
    }
    fn round(context: &HeightContext, view: View) -> ConsensusRound {
        ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        }
    }
    fn subject(seed: u8) -> BlockSubject {
        BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([seed, 0]))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 1])),
            payload_hash: Hash::new([seed, 2]),
        }
    }
    fn rs16_fixture_chunks(context: &HeightContext, payload: &[u8]) -> Vec<Vec<u8>> {
        encode_payload_chunks(context.da_layout, payload)
            .expect("fixture payload encoding succeeds")
    }
    fn execution_commitment(seed: u8) -> ExecutionCommitment {
        let executed_block_wire = [seed, 6];
        ExecutionCommitment::new_without_merge_carrier(
            Hash::new([seed, 3]),
            Hash::new([seed, 4]),
            Hash::new([seed, 5]),
            None,
            0,
            u64::try_from(executed_block_wire.len()).expect("fixture wire length fits u64"),
            Hash::new(executed_block_wire),
        )
        .expect("canonical fixture execution commitment")
    }
    #[test]
    fn current_consensus_nullable_layouts_roundtrip_exactly() {
        macro_rules! assert_roundtrip {
            ($ty:ty, $value:expr) => {{
                let value: $ty = $value;
                let encoded = value.encode();
                let mut cursor = encoded.as_slice();
                let decoded = <$ty>::decode_all(&mut cursor).expect("decode current layout");
                assert_eq!(decoded, value);
            }};
        }

        let context = context(&[1, 1, 1, 1]);
        let round = round(&context, 0);
        let subject = BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"current genesis subject")),
            payload_hash: Hash::new(b"current genesis payload"),
        };
        let commitment = execution_commitment(0x51);
        let timeout_vote = TimeoutVote {
            round,
            highest_prepare_qc: None,
            signer: 0,
            signature: vec![0x52; 48],
        };
        let timeout_signature = TimeoutVoteSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            round,
            highest_prepare_qc: None,
        };
        let timeout_certificate = TimeoutCertificate {
            round,
            groups: vec![TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x53; 48],
            }],
        };
        let timeout_ref = timeout_certificate.as_ref();
        let parent_justification = ParentCommitJustification { certificate: None };
        let timeout_justification = TimeoutJustification {
            timeout_certificate: timeout_certificate.clone(),
            highest_prepare_qc: None,
        };
        let native_leaf = NativeAmxApplicationManifestLeafV1 {
            version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(8),
            lane_incarnation: Hash::new(b"current native leaf incarnation"),
            participant_height: 1,
            participant_view: 0,
            predecessor_height: 0,
            predecessor_descriptor_hash: None,
            descriptor_hash: Hash::new(b"current native leaf descriptor"),
            proposal_hash: Hash::new(b"current native leaf proposal"),
            settlement_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"current native leaf settlement",
            )),
            members: Vec::new(),
            application_block_height: 1,
            application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"current native leaf application",
            )),
            executed_block_wire_hash: Hash::new(b"current native leaf wire"),
        };

        assert_roundtrip!(HeightContext, context);
        assert_roundtrip!(BlockSubject, subject);
        assert_roundtrip!(NativeAmxApplicationManifestLeafV1, native_leaf);
        assert_roundtrip!(ExecutionCommitment, commitment);
        assert_roundtrip!(TimeoutVote, timeout_vote);
        assert_roundtrip!(TimeoutVoteSignaturePayload, timeout_signature);
        assert_roundtrip!(TimeoutCertificate, timeout_certificate);
        assert_roundtrip!(TimeoutCertificateRef, timeout_ref);
        assert_roundtrip!(ParentCommitJustification, parent_justification);
        assert_roundtrip!(TimeoutJustification, timeout_justification);
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the fail-closed audit enumerates every retired nullable consensus prefix in one schema test"
    )]
    fn pre_release_consensus_layouts_cannot_omit_nullable_slots() {
        #[derive(Encode)]
        struct PreReleaseHeightContextPrefix {
            network_id: NetworkId,
            protocol_version: u16,
            height: Height,
            epoch: u64,
            epoch_end_height: Height,
        }
        #[derive(Encode)]
        struct PreReleaseBlockSubject {
            block_hash: HashOf<BlockHeader>,
            payload_hash: Hash,
        }
        #[derive(Encode)]
        struct PreReleaseNativeLeafPrefix {
            version: u16,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            lane_incarnation: Hash,
            participant_height: u64,
            participant_view: u64,
            predecessor_height: u64,
        }
        #[derive(Encode)]
        #[expect(
            clippy::struct_field_names,
            reason = "the retired prefix field names document the exact consensus roots omitted by the hostile layout"
        )]
        struct PreReleaseExecutionCommitmentPrefix {
            parent_state_root: Hash,
            post_state_root: Hash,
            ordinary_writes_root: Hash,
        }
        #[derive(Encode)]
        struct PreReleaseTimeoutVote {
            round: ConsensusRound,
            signer: ValidatorIndex,
            signature: Vec<u8>,
        }
        #[derive(Encode)]
        struct PreReleaseTimeoutVoteSignaturePayload {
            protocol_version: u16,
            round: ConsensusRound,
        }
        #[derive(Encode)]
        struct PreReleaseTimeoutVoteGroup {
            signers: Vec<ValidatorIndex>,
            aggregate_signature: Vec<u8>,
        }
        #[derive(Encode)]
        struct PreReleaseTimeoutCertificateRefPrefix {
            round: ConsensusRound,
        }
        #[derive(Encode)]
        struct PreReleaseTimeoutJustification {
            timeout_certificate: TimeoutCertificate,
        }
        macro_rules! assert_rejected {
            ($ty:ty, $encoded:expr, $label:literal) => {{
                let encoded = $encoded;
                let mut cursor = encoded.as_slice();
                assert!(<$ty>::decode_all(&mut cursor).is_err(), $label);
            }};
        }

        let context = context(&[1, 1, 1, 1]);
        let round = round(&context, 0);
        let timeout_certificate = TimeoutCertificate {
            round,
            groups: vec![TimeoutVoteGroup {
                highest_prepare_qc: None,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x61; 48],
            }],
        };
        assert_rejected!(
            HeightContext,
            PreReleaseHeightContextPrefix {
                network_id: context.network_id,
                protocol_version: context.protocol_version,
                height: context.height,
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
            }
            .encode(),
            "a shortened height context must not infer nullable consensus anchors"
        );
        assert_rejected!(
            BlockSubject,
            PreReleaseBlockSubject {
                block_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xFE; Hash::LENGTH])),
                payload_hash: Hash::new(b"pre-release subject payload"),
            }
            .encode(),
            "a subject without its parent slot must fail closed"
        );
        assert_rejected!(
            NativeAmxApplicationManifestLeafV1,
            PreReleaseNativeLeafPrefix {
                version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                lane_id: LaneId::new(7),
                dataspace_id: DataSpaceId::new(8),
                lane_incarnation: Hash::new(b"pre-release native leaf"),
                participant_height: 1,
                participant_view: 0,
                predecessor_height: 0,
            }
            .encode(),
            "a Native AMX leaf without its predecessor slot must fail closed"
        );
        assert_rejected!(
            ExecutionCommitment,
            PreReleaseExecutionCommitmentPrefix {
                parent_state_root: Hash::new(b"pre-release parent state"),
                post_state_root: Hash::new(b"pre-release post state"),
                ordinary_writes_root: Hash::new(b"pre-release ordinary writes"),
            }
            .encode(),
            "an execution commitment without its top-up slot must fail closed"
        );
        assert_rejected!(
            TimeoutVote,
            PreReleaseTimeoutVote {
                round,
                signer: 7,
                signature: vec![0x62; 48],
            }
            .encode(),
            "a timeout vote without its highest-PrepareQC slot must fail closed"
        );
        assert_rejected!(
            TimeoutVoteSignaturePayload,
            PreReleaseTimeoutVoteSignaturePayload {
                protocol_version: PROTOCOL_VERSION,
                round,
            }
            .encode(),
            "a timeout signature payload without its highest-PrepareQC slot must fail closed"
        );
        assert_rejected!(
            TimeoutVoteGroup,
            PreReleaseTimeoutVoteGroup {
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x63; 48],
            }
            .encode(),
            "a timeout group without its highest-PrepareQC slot must fail closed"
        );
        assert_rejected!(
            TimeoutCertificateRef,
            PreReleaseTimeoutCertificateRefPrefix { round }.encode(),
            "a timeout reference without its highest-PrepareQC slot must fail closed"
        );
        assert_rejected!(
            ParentCommitJustification,
            Vec::<u8>::new(),
            "a parent justification without its certificate slot must fail closed"
        );
        assert_rejected!(
            TimeoutJustification,
            PreReleaseTimeoutJustification {
                timeout_certificate,
            }
            .encode(),
            "a timeout justification without its highest-PrepareQC slot must fail closed"
        );
    }
    fn qc(
        context: &HeightContext,
        view: View,
        phase: GlobalPhase,
        signers: Vec<ValidatorIndex>,
    ) -> QuorumCertificate {
        let round = round(context, view);
        QuorumCertificate {
            round,
            proposal_round: round,
            phase,
            subject: subject(u8::try_from(view + 1).expect("small fixture view")),
            execution_commitment: execution_commitment(
                u8::try_from(view + 1).expect("small fixture view"),
            ),
            signers,
            aggregate_signature: vec![0x5A; 48],
        }
    }
    fn manifest(context: &HeightContext) -> PayloadManifest {
        let subject = subject(9);
        let encoded_chunks = rs16_fixture_chunks(context, b"body");
        PayloadManifest::derive(context, round(context, 1), subject, 4, &encoded_chunks)
            .expect("valid canonical manifest")
    }
    struct TimeoutProposalFixture {
        context: HeightContext,
        timeout_round: ConsensusRound,
        proposal: Proposal,
        highest_prepare: QuorumCertificate,
    }
    fn timeout_proposal_fixture() -> TimeoutProposalFixture {
        let context = context(&[1, 1, 1, 1]);
        let payload_manifest = manifest(&context);
        let timeout_round = round(&context, 0);
        let proposal = Proposal {
            round: payload_manifest.round,
            proposer: context.leader(payload_manifest.round.view),
            subject: payload_manifest.subject,
            manifest: payload_manifest,
            justification: timeout_justification(timeout_round, None, None, 0x41),
            signature: vec![0x42; 48],
        };
        let mut highest_prepare = qc(&context, 0, GlobalPhase::Prepare, vec![0, 1, 2]);
        highest_prepare.subject = proposal.subject;
        TimeoutProposalFixture {
            context,
            timeout_round,
            proposal,
            highest_prepare,
        }
    }
    fn timeout_justification(
        timeout_round: ConsensusRound,
        certificate_high: Option<QuorumCertificate>,
        proposal_high: Option<QuorumCertificate>,
        signature_seed: u8,
    ) -> ProposalJustification {
        ProposalJustification::Timeout(TimeoutJustification {
            timeout_certificate: TimeoutCertificate {
                round: timeout_round,
                groups: vec![TimeoutVoteGroup {
                    highest_prepare_qc: certificate_high,
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![signature_seed; 48],
                }],
            },
            highest_prepare_qc: proposal_high,
        })
    }
    struct ParentReproposalFixture {
        context: HeightContext,
        parent_round: ConsensusRound,
        proposal: Proposal,
    }
    fn parent_reproposal_fixture() -> ParentReproposalFixture {
        let mut context = context(&[1, 1, 1, 1]);
        context.height = 2;
        let parent_subject = subject(0x70);
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"proposal parent context",
            ))),
            height: context.height - 1,
            view: 2,
        };
        context.parent_commit_qc = Some(QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x70),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x31; 48],
        });
        let proposal_round = round(&context, 0);
        let mut payload_manifest = manifest(&context);
        payload_manifest.round = proposal_round;
        let carried = QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x70),
            signers: vec![0, 1, 3],
            aggregate_signature: vec![0x32; 48],
        };
        let frozen_parent = context
            .parent_commit_qc
            .as_ref()
            .expect("fixture parent certificate");
        assert!(
            carried
                .as_ref()
                .same_commit_decision(frozen_parent.as_ref())
        );
        let mut prepare_ref = carried.as_ref();
        prepare_ref.phase = GlobalPhase::Prepare;
        assert!(!prepare_ref.same_commit_decision(frozen_parent.as_ref()));
        let proposal = Proposal {
            round: proposal_round,
            proposer: context.leader(0),
            subject: payload_manifest.subject,
            manifest: payload_manifest,
            justification: ProposalJustification::ParentCommit(ParentCommitJustification {
                certificate: Some(carried),
            }),
            signature: vec![0x33; 48],
        };
        ParentReproposalFixture {
            context,
            parent_round,
            proposal,
        }
    }
    fn carried_parent_certificate(proposal: &mut Proposal) -> &mut QuorumCertificate {
        let ProposalJustification::ParentCommit(parent) = &mut proposal.justification else {
            unreachable!("fixture uses a parent justification");
        };
        parent
            .certificate
            .as_mut()
            .expect("carried parent certificate")
    }
    #[test]
    fn equal_vote_quorum_requires_two_f_plus_one_distinct_signers() {
        let context = context(&[1, 1, 1, 1]);
        assert_eq!(context.quorum.min_signers, 3);
        assert_eq!(context.validate_signers(&[0, 1, 2]), Ok(()));
        assert_eq!(context.validate_signers(&[1, 2, 3]), Ok(()));
        assert_eq!(context.validate_signers(&[0, 1, 2, 3]), Ok(()));
        assert_eq!(context.validate_certificate_signers(&[0, 1, 2]), Ok(()));
        assert_eq!(
            ValidationError::TooManySigners.to_string(),
            "signer count exceeds the wire range"
        );
        assert_eq!(
            ValidationError::SignerCountMismatch {
                expected: 3,
                actual: 4,
            }
            .to_string(),
            "certificate signer count mismatch: expected exactly 3, got 4"
        );
        assert_eq!(
            context.validate_certificate_signers(&[0, 1, 2, 3]),
            Err(ValidationError::SignerCountMismatch {
                expected: 3,
                actual: 4,
            })
        );
        assert_eq!(
            context.validate_signers(&[0, 1]),
            Err(ValidationError::InsufficientSignerCount)
        );
        assert_eq!(
            context.validate_signers(&[0, 1, 1]),
            Err(ValidationError::SignersNotStrictlySorted)
        );
        assert_eq!(
            qc(&context, 0, GlobalPhase::Commit, vec![0, 1, 2, 3]).validate(&context),
            Err(ValidationError::SignerCountMismatch {
                expected: 3,
                actual: 4,
            })
        );
    }
    #[test]
    fn height_context_rejects_weighted_consensus_votes_in_all_modes() {
        for mode in [ConsensusMode::Permissioned, ConsensusMode::Npos] {
            let mut invalid = context(&[1, 1, 1, 1]);
            invalid.mode = mode;
            invalid.roster[0].power = 2;
            invalid.quorum =
                DualQuorum::from_roster(&invalid.roster).expect("structural weighted quorum");
            assert_eq!(invalid.validate(), Err(ValidationError::VotingPowerNotOne));
        }
    }
    #[test]
    fn height_context_rejects_zero_execution_policy_hash() {
        let mut invalid = context(&[1, 1, 1, 1]);
        invalid.execution_policy_hash = Hash::prehashed([0; Hash::LENGTH]);
        assert_eq!(
            invalid.validate(),
            Err(ValidationError::InvalidExecutionPolicyHash)
        );
    }
    #[test]
    fn data_availability_layout_enforces_protocol_resource_caps() {
        let maximum = DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES,
            data_shards: MAX_DA_DATA_SHARDS,
            parity_shards: MAX_DA_PARITY_SHARDS,
            max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES,
            max_chunk_count: MAX_DA_CHUNK_COUNT,
        };
        assert_eq!(validate_data_availability_layout(maximum), Ok(()));
        let mut invalid_layouts = Vec::new();
        invalid_layouts.push(DataAvailabilityLayout {
            chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES + 2,
            ..maximum
        });
        invalid_layouts.push(DataAvailabilityLayout {
            data_shards: MAX_DA_DATA_SHARDS + 1,
            ..maximum
        });
        invalid_layouts.push(DataAvailabilityLayout {
            parity_shards: MAX_DA_PARITY_SHARDS + 1,
            ..maximum
        });
        invalid_layouts.push(DataAvailabilityLayout {
            max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES + 1,
            ..maximum
        });
        invalid_layouts.push(DataAvailabilityLayout {
            max_chunk_count: MAX_DA_CHUNK_COUNT + 1,
            ..maximum
        });
        invalid_layouts.push(DataAvailabilityLayout {
            data_shards: 1,
            parity_shards: 15,
            ..maximum
        });
        invalid_layouts.push(DataAvailabilityLayout {
            data_shards: 1_024,
            parity_shards: 1_024,
            max_chunk_count: u32::MAX,
            ..maximum
        });
        for invalid in invalid_layouts {
            assert_eq!(
                validate_data_availability_layout(invalid),
                Err(ValidationError::InvalidDataAvailabilityLayout)
            );
        }
    }
    #[test]
    fn payload_chunk_encoding_is_complete_padded_and_deterministic() {
        let context = context(&[1, 1, 1, 1]);
        let payload = b"abcdef";
        let chunks = encode_payload_chunks(context.da_layout, payload)
            .expect("canonical payload encoding succeeds");
        assert_eq!(chunks.len(), 4);
        assert!(chunks.iter().all(|chunk| chunk.len() == 4));
        assert_eq!(chunks[0], b"abcd");
        assert_eq!(chunks[1], chunks[0]);
        assert_eq!(chunks[2], [b'e', b'f', 0, 0]);
        assert_eq!(chunks[3], chunks[2]);
        assert_eq!(
            chunks,
            encode_payload_chunks(context.da_layout, payload)
                .expect("repeated canonical encoding succeeds")
        );
        let manifest = PayloadManifest::derive(
            &context,
            round(&context, 1),
            subject(9),
            u64::try_from(payload.len()).expect("fixture payload length fits u64"),
            &chunks,
        )
        .expect("complete encoded chunks derive a valid manifest");
        assert_eq!(manifest.validate(&context), Ok(()));
    }
    #[test]
    fn height_context_rejects_noncanonical_rosters_and_quorums() {
        let mut empty = context(&[1, 1, 1, 1]);
        empty.roster.clear();
        assert_eq!(empty.leader(u64::MAX), 0);
        assert_eq!(empty.validate(), Err(ValidationError::EmptyRoster));
        let mut too_small = context(&[1, 1, 1, 1]);
        too_small.roster.truncate(MIN_VALIDATORS_PER_HEIGHT - 1);
        assert_eq!(too_small.validate(), Err(ValidationError::RosterTooSmall));
        let mut invalid_geometry = context(&[1, 1, 1, 1]);
        invalid_geometry.roster.push(ValidatorPower {
            validator: peer(0xFE),
            power: 1,
        });
        invalid_geometry.roster.sort();
        assert_eq!(
            invalid_geometry.validate(),
            Err(ValidationError::InvalidCommitteeGeometry)
        );
        let mut invalid = context(&[1, 1, 1, 1]);
        invalid.roster[1].validator = invalid.roster[0].validator.clone();
        assert_eq!(invalid.validate(), Err(ValidationError::DuplicateValidator));
        let mut invalid = context(&[1, 1, 1, 1]);
        invalid.quorum.min_signers = 2;
        assert_eq!(
            invalid.validate(),
            Err(ValidationError::CountThresholdMismatch)
        );
        let mut oversized = context(&[1, 1, 1, 1]);
        let repeated = oversized.roster[0].clone();
        oversized
            .roster
            .resize(MAX_VALIDATORS_PER_HEIGHT + 1, repeated);
        assert_eq!(oversized.validate(), Err(ValidationError::RosterTooLarge));
        let largest = context(&vec![1; MAX_VALIDATORS_PER_HEIGHT]);
        assert_eq!(largest.validate(), Ok(()));
        let mut odd_rs16_symbols = context(&[1, 1, 1, 1]);
        odd_rs16_symbols.da_layout.chunk_size_bytes = 3;
        assert_eq!(
            odd_rs16_symbols.validate(),
            Err(ValidationError::InvalidDataAvailabilityLayout)
        );
        let mut insufficient_chunk_capacity = context(&[1, 1, 1, 1]);
        insufficient_chunk_capacity.da_layout.max_chunk_count -= 1;
        assert_eq!(
            insufficient_chunk_capacity.validate(),
            Err(ValidationError::InvalidDataAvailabilityLayout)
        );
        let mut invalid_parent_execution = context(&[1, 1, 1, 1]);
        invalid_parent_execution.height = 2;
        let invalid_parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"invalid parent execution context",
            ))),
            height: 1,
            view: 0,
        };
        let invalid_parent_executed_block_wire = b"executed block wire";
        invalid_parent_execution.parent_commit_qc = Some(QuorumCertificate {
            round: invalid_parent_round,
            proposal_round: invalid_parent_round,
            phase: GlobalPhase::Commit,
            subject: subject(0x61),
            execution_commitment: ExecutionCommitment {
                parent_state_root: Hash::new(b"parent state"),
                post_state_root: Hash::new(b"post state"),
                ordinary_writes_root: Hash::new(b"ordinary writes"),
                topup_anchor_root: None,
                topup_anchor_count: 1,
                native_amx_application_manifest_version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
                native_amx_application_manifest_root: native_amx_application_manifest_empty_root(),
                native_amx_application_manifest_count: 0,
                lane_finality_manifest: None,
                merge_carrier: None,
                executed_block_wire_len: u64::try_from(invalid_parent_executed_block_wire.len())
                    .expect("fixture wire length fits u64"),
                executed_block_wire_hash: Hash::new(invalid_parent_executed_block_wire),
            },
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x62; 48],
        });
        assert_eq!(
            invalid_parent_execution.validate(),
            Err(ValidationError::InvalidExecutionCommitment)
        );
    }
    #[test]
    fn snapshot_bootstrap_is_an_explicit_mutually_exclusive_parent_authority() {
        let mut anchored = context(&[1, 1, 1, 1]);
        anchored.height = 11;
        anchored.snapshot_bootstrap = Some(SnapshotBootstrapAnchor {
            snapshot_height: 10,
            snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"audited snapshot tip")),
            snapshot_block_creation_time_ms: 1_000,
            snapshot_state_hash: Hash::new(b"audited snapshot WSV"),
        });
        anchored
            .validate()
            .expect("exact post-snapshot context is structurally valid");
        let record = SnapshotV2BootstrapRecord {
            version: SnapshotV2BootstrapRecord::VERSION,
            context: anchored.clone(),
            validator_set_pops: vec![vec![0xA5]; anchored.roster.len()],
        };
        record.validate().expect("complete bootstrap record");
        let mut wrong_height = record.clone();
        wrong_height.context.height = 12;
        assert_eq!(
            wrong_height.validate(),
            Err(ValidationError::InvalidParentCommit)
        );
        let mut ambiguous = anchored;
        ambiguous.parent_commit_qc = Some(qc(
            &context(&[1, 1, 1, 1]),
            0,
            GlobalPhase::Commit,
            vec![0, 1, 2],
        ));
        assert_eq!(
            ambiguous.validate(),
            Err(ValidationError::InvalidParentCommit)
        );
        let mut unsupported = record;
        unsupported.version = SnapshotV2BootstrapRecord::VERSION + 1;
        assert_eq!(
            unsupported.validate(),
            Err(ValidationError::InvalidSnapshotBootstrap)
        );
    }
    #[test]
    fn non_boundary_height_context_id_is_pinned() {
        let context = context(&[1, 1, 1, 1]);
        context.validate().expect("valid non-boundary context");
        assert_eq!(
            *context.id().0.as_ref(),
            [
                0x6e, 0x27, 0x3d, 0x47, 0xa7, 0x42, 0xec, 0xa0, 0x81, 0x97, 0xeb, 0x84, 0x26, 0x0f,
                0x6d, 0xe2, 0x63, 0x5a, 0x7e, 0x08, 0xb4, 0x6c, 0xc5, 0xa8, 0xce, 0x05, 0xa1, 0x54,
                0x04, 0xf4, 0x1e, 0x1f,
            ],
            "intentional identity-projection changes require updating this golden"
        );
    }
    #[test]
    fn boundary_height_context_id_pins_the_complete_transition() {
        let mut context = context(&[1, 1, 1, 1]);
        context.epoch_end_height = context.height;
        let next_roster = roster(&[1, 1, 1, 1]);
        context.next_epoch_snapshot = Some(finality::FinalizedNextEpochSnapshot {
            epoch: context.epoch + 1,
            epoch_end_height: 41,
            mode: context.mode,
            quorum: DualQuorum::from_roster(&next_roster).expect("valid next-epoch quorum"),
            roster: next_roster,
            validator_set_pops: vec![vec![0x81], vec![0x82, 0x83], vec![0x84], vec![0x85, 0x86]],
            leader_seed: [0x87; 32],
        });
        context.validate().expect("valid boundary context");
        assert_eq!(
            *context.id().0.as_ref(),
            [
                0xa1, 0x53, 0x7a, 0xd0, 0x9e, 0x51, 0xcf, 0xd1, 0x1c, 0x5a, 0xac, 0xcb, 0x5d, 0x44,
                0x16, 0xdb, 0xba, 0xfa, 0x3b, 0x1c, 0xd2, 0x3b, 0xae, 0xf5, 0x07, 0x66, 0x6e, 0x33,
                0x85, 0xa4, 0x4c, 0x6b,
            ],
            "intentional transition-identity changes require updating this golden"
        );
    }
    #[test]
    fn height_context_id_ignores_equivalent_parent_qc_round_and_signer_evidence() {
        let mut left = context(&[1, 1, 1, 1]);
        left.height = 2;
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"parent context",
            ))),
            height: left.height - 1,
            view: 3,
        };
        let parent_subject = subject(0x44);
        left.parent_commit_qc = Some(QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x44),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x11; 48],
        });
        let mut right = left.clone();
        let redecided_round = ConsensusRound {
            view: parent_round.view + 1,
            ..parent_round
        };
        right.parent_commit_qc = Some(QuorumCertificate {
            round: redecided_round,
            proposal_round: redecided_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x44),
            signers: vec![0, 1, 3],
            aggregate_signature: vec![0x22; 48],
        });
        assert_ne!(left.parent_commit_qc, right.parent_commit_qc);
        assert_eq!(left.id(), right.id());
        let mut different_execution = right.clone();
        different_execution
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .execution_commitment = execution_commitment(0x45);
        assert_ne!(left.id(), different_execution.id());
        let mut different_subject = right.clone();
        different_subject
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .subject = subject(0x45);
        assert_ne!(left.id(), different_subject.id());
        right
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .round
            .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"different parent context",
        )));
        assert_ne!(left.id(), right.id());
        let mut oversized_parent = left;
        oversized_parent
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .aggregate_signature = vec![0x33; MAX_CONSENSUS_SIGNATURE_BYTES + 1];
        assert_eq!(
            oversized_parent.validate(),
            Err(ValidationError::SignatureTooLarge)
        );
    }
    #[test]
    fn height_context_identity_ignores_reproposal_round_and_rejects_split_rounds() {
        let mut original = context(&[1, 1, 1, 1]);
        original.height = 2;
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"parent proposal-origin context",
            ))),
            height: 1,
            view: 5,
        };
        original.parent_commit_qc = Some(QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: GlobalPhase::Commit,
            subject: subject(0x47),
            execution_commitment: execution_commitment(0x47),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x47; 48],
        });
        original.validate().expect("valid parent decision");
        let mut redecided = original.clone();
        let certificate = redecided
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate");
        certificate.round.view += 1;
        certificate.proposal_round = certificate.round;
        redecided
            .validate()
            .expect("unchanged re-proposal may decide in another round");
        assert_eq!(original.id(), redecided.id());
        let mut cross_context_origin = original.clone();
        let certificate = cross_context_origin
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate");
        certificate.round.context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign proposal-origin context",
        )));
        certificate.proposal_round = certificate.round;
        cross_context_origin
            .validate()
            .expect("a structurally valid parent can belong to another prior context");
        assert_ne!(original.id(), cross_context_origin.id());
        let mut wrong_height_origin = original.clone();
        let certificate = wrong_height_origin
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate");
        certificate.round.height = 2;
        certificate.proposal_round = certificate.round;
        assert_eq!(
            wrong_height_origin.validate(),
            Err(ValidationError::InvalidParentCommit)
        );
        let mut split_round = original;
        let parent = split_round
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate");
        parent.proposal_round.view = parent.round.view + 1;
        assert_eq!(
            split_round.validate(),
            Err(ValidationError::InvalidParentCommit)
        );
    }
    #[test]
    fn timeout_certificate_requires_disjoint_dual_quorum() {
        let context = context(&[1, 1, 1, 1]);
        let prepare = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let certificate = TimeoutCertificate {
            round: round(&context, 2),
            groups: vec![
                TimeoutVoteGroup {
                    highest_prepare_qc: None,
                    signers: vec![0],
                    aggregate_signature: vec![1],
                },
                TimeoutVoteGroup {
                    highest_prepare_qc: Some(prepare.clone()),
                    signers: vec![1, 2],
                    aggregate_signature: vec![2],
                },
            ],
        };
        assert_eq!(certificate.validate(&context), Ok(()));
        assert_eq!(certificate.highest_prepare_qc(), Some(&prepare));
        let mut superset = certificate.clone();
        superset.groups[1].signers.push(3);
        assert_eq!(
            superset.validate(&context),
            Err(ValidationError::SignerCountMismatch {
                expected: 3,
                actual: 4,
            })
        );
        let mut overlapping = certificate.clone();
        overlapping.groups[1].signers = vec![0, 2];
        assert_eq!(
            overlapping.validate(&context),
            Err(ValidationError::OverlappingTimeoutSigners)
        );
    }
    #[test]
    fn highest_prepare_qc_uses_view_then_semantic_reference() {
        let context = context(&[1, 1, 1, 1]);
        let lower = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let higher = qc(&context, 3, GlobalPhase::Prepare, vec![0, 1, 2]);
        let certificate = TimeoutCertificate {
            round: round(&context, 4),
            groups: vec![
                TimeoutVoteGroup {
                    highest_prepare_qc: Some(lower),
                    signers: vec![0],
                    aggregate_signature: vec![1],
                },
                TimeoutVoteGroup {
                    highest_prepare_qc: Some(higher.clone()),
                    signers: vec![1, 2],
                    aggregate_signature: vec![2],
                },
            ],
        };
        assert_eq!(certificate.highest_prepare_qc(), Some(&higher));
    }
    #[test]
    fn timeout_certificate_rejects_conflicting_prepare_qcs_at_one_view() {
        let context = context(&[1, 1, 1, 1]);
        let left = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let mut right = left.clone();
        right.subject = subject(0x7E);
        right.aggregate_signature = vec![0x7E; 48];
        let mut groups = vec![
            TimeoutVoteGroup {
                highest_prepare_qc: Some(left),
                signers: vec![0],
                aggregate_signature: vec![1],
            },
            TimeoutVoteGroup {
                highest_prepare_qc: Some(right),
                signers: vec![1, 2],
                aggregate_signature: vec![2],
            },
        ];
        groups.sort_by_key(|group| {
            group
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref)
        });
        let certificate = TimeoutCertificate {
            round: round(&context, 2),
            groups,
        };
        assert_eq!(
            certificate.validate(&context),
            Err(ValidationError::ConflictingHighestPrepare)
        );
    }
    #[test]
    fn timeout_certificate_rejects_conflicting_prepare_execution_at_one_view() {
        let context = context(&[1, 1, 1, 1]);
        let left = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let mut right = left.clone();
        right.execution_commitment = execution_commitment(0x7E);
        right.aggregate_signature = vec![0x7E; 48];
        let mut groups = vec![
            TimeoutVoteGroup {
                highest_prepare_qc: Some(left),
                signers: vec![0],
                aggregate_signature: vec![1],
            },
            TimeoutVoteGroup {
                highest_prepare_qc: Some(right),
                signers: vec![1, 2],
                aggregate_signature: vec![2],
            },
        ];
        groups.sort_by_key(|group| {
            group
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref)
        });
        let certificate = TimeoutCertificate {
            round: round(&context, 2),
            groups,
        };

        assert_eq!(
            certificate.validate(&context),
            Err(ValidationError::ConflictingHighestPrepare)
        );
    }

    #[test]
    fn qc_reference_and_timeout_preimage_ignore_equivalent_quorum_subsets() {
        let context = context(&[1, 1, 1, 1]);
        let left = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let right = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 3]);
        assert_ne!(HashOf::new(&left), HashOf::new(&right));
        assert_eq!(left.as_ref(), right.as_ref());
        let left_vote = TimeoutVote {
            round: round(&context, 2),
            highest_prepare_qc: Some(left),
            signer: 0,
            signature: vec![1],
        };
        let right_vote = TimeoutVote {
            round: round(&context, 2),
            highest_prepare_qc: Some(right),
            signer: 1,
            signature: vec![2],
        };
        assert_eq!(
            left_vote.signature_preimage(),
            right_vote.signature_preimage()
        );
    }
    #[test]
    fn v2_envelope_norito_roundtrip() {
        let context = context(&[1, 1, 1, 1]);
        let manifest = manifest(&context);
        let proposal = Proposal {
            round: manifest.round,
            proposer: 2,
            subject: manifest.subject,
            manifest,
            justification: ProposalJustification::ParentCommit(ParentCommitJustification {
                certificate: None,
            }),
            signature: vec![0x22; 48],
        };
        let message = ConsensusMessageV2::new(ConsensusMessageV2Payload::Proposal(proposal));
        let encoded = message.encode();
        let decoded = ConsensusMessageV2::decode(&mut &encoded[..])
            .expect("decode canonical Sumeragi v2 envelope");
        assert_eq!(decoded, message);
        assert_eq!(decoded.validate_version(), Ok(()));
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the test keeps every consensus payload variant in one round-trip matrix"
    )]
    fn every_v2_payload_variant_roundtrips() {
        let context = context(&[1, 1, 1, 1]);
        let prepare = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let timeout = TimeoutCertificate {
            round: round(&context, 2),
            groups: vec![TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare.clone()),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x33; 48],
            }],
        };
        let manifest = manifest(&context);
        let request = CertifiedBodyRequest {
            round: manifest.round,
            subject: manifest.subject,
            certificate: prepare.clone(),
            requester: context.roster[3].validator.clone(),
            signature: vec![0x44; 48],
        };
        let commit = QuorumCertificate {
            phase: GlobalPhase::Commit,
            ..prepare.clone()
        };
        let commit_request = CommitCertificateRequest {
            protocol_version: PROTOCOL_VERSION,
            network_id: context.network_id,
            context_id: context.id(),
            height: context.height,
            requester: context.roster[3].validator.clone(),
            signature: vec![0x45; 48],
        };
        let proposal = Proposal {
            round: manifest.round,
            proposer: 2,
            subject: manifest.subject,
            manifest: manifest.clone(),
            justification: ProposalJustification::Timeout(TimeoutJustification {
                timeout_certificate: timeout.clone(),
                highest_prepare_qc: Some(prepare.clone()),
            }),
            signature: vec![0x55; 48],
        };
        let beacon_partial = GlobalBeaconPartialSignature {
            round: round(&context, 3),
            partial: GlobalThresholdBeaconPartialSignatureV1 {
                session_id: [0xA1; 32],
                signer_index: 2,
                signature_share: [0xA2; 48],
                proof: crate::consensus::GlobalThresholdBeaconPartialSignatureProofV1 {
                    x: [0xA3; 96],
                    y: [0xA4; 48],
                    z_s: [0xA5; 32],
                    z_r: [0xA6; 32],
                    z_u: [0xA7; 32],
                },
            },
        };
        assert_eq!(beacon_partial.validate(&context), Ok(()));
        let mut zero_seat = beacon_partial.clone();
        zero_seat.partial.signer_index = 0;
        assert_eq!(
            zero_seat.validate(&context),
            Err(ValidationError::SignerOutOfRange)
        );
        let variants = vec![
            ConsensusMessageV2Payload::Proposal(proposal),
            ConsensusMessageV2Payload::Vote(Vote {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: prepare.execution_commitment,
                signer: 0,
                signature: vec![1],
            }),
            ConsensusMessageV2Payload::QuorumCertificate(prepare.clone()),
            ConsensusMessageV2Payload::TimeoutVote(TimeoutVote {
                round: timeout.round,
                highest_prepare_qc: Some(prepare.clone()),
                signer: 0,
                signature: vec![2],
            }),
            ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ConsensusMessageV2Payload::PayloadManifest(manifest.clone()),
            ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
                manifest_hash: HashOf::new(&manifest),
                index: 0,
                bytes: b"body".to_vec(),
                sender: 0,
                signature: vec![0x66; 48],
            }),
            ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
            ConsensusMessageV2Payload::CertifiedBodyResponse(CertifiedBodyResponse {
                request_hash: HashOf::new(&request),
                manifest,
                body: b"body".to_vec(),
                responder: 0,
                signature: vec![3],
            }),
            ConsensusMessageV2Payload::CommitCertificateRequest(commit_request.clone()),
            ConsensusMessageV2Payload::CommitCertificateResponse(CommitCertificateResponse {
                request_hash: HashOf::new(&commit_request),
                certificate: commit,
                responder: context.roster[0].validator.clone(),
                signature: vec![4],
            }),
            ConsensusMessageV2Payload::GlobalBeaconPartialSignature(beacon_partial),
        ];
        for payload in variants {
            let message = ConsensusMessageV2::new(payload);
            let encoded = message.encode();
            let decoded = ConsensusMessageV2::decode(&mut &encoded[..])
                .expect("decode Sumeragi v2 payload variant");
            assert_eq!(decoded, message);
        }
    }
    #[test]
    fn voting_power_sum_fails_closed_on_u64_overflow() {
        let mut roster = vec![
            ValidatorPower {
                validator: peer(1),
                power: u64::MAX,
            },
            ValidatorPower {
                validator: peer(2),
                power: 1,
            },
            ValidatorPower {
                validator: peer(3),
                power: 1,
            },
            ValidatorPower {
                validator: peer(4),
                power: 1,
            },
        ];
        roster.sort();
        assert_eq!(
            DualQuorum::from_roster(&roster),
            Err(ValidationError::VotingPowerOverflow)
        );
    }
    #[test]
    fn signed_payload_chunk_binds_session_and_manifest_fields() {
        let context = context(&[1, 1, 1, 1]);
        let manifest = manifest(&context);
        let chunk = PayloadChunk {
            manifest_hash: HashOf::new(&manifest),
            index: 0,
            bytes: b"body".to_vec(),
            sender: 1,
            signature: vec![0x77; 48],
        };
        let payload = chunk
            .signature_payload(&context, &manifest)
            .expect("valid chunk signature payload");
        assert_eq!(payload.context_id, context.id());
        assert_eq!(payload.epoch, context.epoch);
        assert_eq!(payload.height, context.height);
        assert_eq!(payload.view, manifest.round.view);
        assert_eq!(payload.subject, manifest.subject);
        assert_eq!(
            payload.total_chunks,
            u32::try_from(manifest.chunk_hashes.len()).expect("fixture chunk count fits u32")
        );
        assert_eq!(payload.chunk_hash, Hash::new(b"body"));
        assert!(
            chunk
                .signature_preimage(&context, &manifest)
                .expect("valid signature preimage")
                .starts_with(b"iroha:sumeragi:v2:payload-chunk")
        );
        let mut unsigned = chunk.clone();
        unsigned.signature.clear();
        assert!(unsigned.signature_preimage(&context, &manifest).is_ok());
        assert_eq!(
            unsigned.validate(&context, &manifest),
            Err(ValidationError::MissingChunkSignature)
        );
        let mut corrupted = chunk.clone();
        corrupted.bytes.push(0);
        assert_eq!(
            corrupted.signature_payload(&context, &manifest),
            Err(ValidationError::InvalidChunkLength)
        );
    }
    #[test]
    fn manifest_rejects_mutated_root_size_count_and_chunk_length() {
        let context = context(&[1, 1, 1, 1]);
        let canonical = manifest(&context);
        assert_eq!(canonical.validate(&context), Ok(()));
        let mut wrong_root = canonical.clone();
        wrong_root.chunk_root = Hash::new(b"not the canonical root");
        assert_eq!(
            wrong_root.validate(&context),
            Err(ValidationError::ChunkRootMismatch)
        );
        let mut wrong_count = canonical.clone();
        wrong_count.payload_size_bytes = 5;
        assert_eq!(
            wrong_count.validate(&context),
            Err(ValidationError::PayloadSizeMismatch)
        );
        let mut oversized = canonical.clone();
        oversized.payload_size_bytes = context.da_layout.max_payload_size_bytes + 1;
        assert_eq!(
            oversized.validate(&context),
            Err(ValidationError::PayloadTooLarge)
        );
        let short_chunk = PayloadChunk {
            manifest_hash: HashOf::new(&canonical),
            index: 0,
            bytes: b"bod".to_vec(),
            sender: 0,
            signature: vec![0x44; 48],
        };
        assert_eq!(
            short_chunk.validate(&context, &canonical),
            Err(ValidationError::InvalidChunkLength)
        );
    }
    #[test]
    fn compact_v2_status_norito_roundtrip() {
        let context = context(&[1, 1, 1, 1]);
        let prepare = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let timeout = TimeoutCertificate {
            round: round(&context, 2),
            groups: vec![TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare.clone()),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x88; 48],
            }],
        };
        let status = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"node"),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: context.id(),
            height: context.height,
            view: 3,
            phase: SumeragiV2StatusPhase::Prepare,
            leader: 2,
            locked_prepare_qc: Some(prepare.as_ref()),
            highest_prepare_qc: Some(prepare.as_ref()),
            last_timeout_certificate: Some(timeout.as_ref()),
            body_state: SumeragiV2BodyState::Validated,
            pending_persistence_id: Some(17),
            last_committed_height: context.height - 1,
            last_committed_subject: None,
            height_context: SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: 4,
                quorum: context.quorum,
            },
            last_commit_qc: None,
            liveness: SumeragiV2LivenessStatus::default(),
        };
        let encoded = status.encode();
        let decoded =
            SumeragiV2Status::decode(&mut &encoded[..]).expect("decode compact Sumeragi v2 status");
        assert_eq!(decoded, status);
    }
    #[test]
    fn leader_rotation_is_cyclic_and_wraps_roster() {
        let context = context(&[1, 1, 1, 1]);
        let start = context.leader(0);
        assert_eq!(context.leader(4), start);
        assert_eq!(
            (0..4)
                .map(|view| context.leader(view))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1, 2, 3])
        );
    }
    #[test]
    fn leader_rotation_reduces_the_maximum_view_without_truncation() {
        let context = context(&[1, 1, 1, 1]);
        let roster_len = u64::try_from(context.roster.len()).expect("fixture roster fits u64");
        assert_eq!(
            context.leader(u64::MAX),
            context.leader(u64::MAX % roster_len),
            "view rotation must reduce at the roster boundary before selecting an index"
        );
    }
    #[test]
    fn timeout_proposal_accepts_only_the_selected_prepare_subject() {
        let TimeoutProposalFixture {
            context,
            timeout_round,
            mut proposal,
            highest_prepare,
        } = timeout_proposal_fixture();
        assert_eq!(proposal.validate(&context), Ok(()));
        proposal.justification =
            timeout_justification(timeout_round, None, Some(highest_prepare.clone()), 0x43);
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification),
            "a proposal cannot invent a repeated high absent from its TC"
        );
        proposal.justification = timeout_justification(
            timeout_round,
            Some(highest_prepare.clone()),
            Some(highest_prepare.clone()),
            0x43,
        );
        assert_eq!(proposal.validate(&context), Ok(()));
        let timeout_certificate = match &proposal.justification {
            ProposalJustification::Timeout(timeout) => timeout.timeout_certificate.clone(),
            ProposalJustification::ParentCommit(_) => {
                unreachable!("prepared fixture carries a timeout")
            }
        };
        proposal.justification = ProposalJustification::Timeout(TimeoutJustification {
            timeout_certificate: timeout_certificate.clone(),
            highest_prepare_qc: None,
        });
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification),
            "a proposal cannot omit the exact high selected by its TC"
        );
        let mut alternate_evidence = highest_prepare.clone();
        alternate_evidence.signers = vec![0, 1, 3];
        alternate_evidence.aggregate_signature = vec![0x46; 48];
        assert_eq!(alternate_evidence.as_ref(), highest_prepare.as_ref());
        assert_ne!(alternate_evidence, highest_prepare);
        proposal.justification = ProposalJustification::Timeout(TimeoutJustification {
            timeout_certificate,
            highest_prepare_qc: Some(alternate_evidence),
        });
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification),
            "the repeated high must preserve the TC-selected full evidence"
        );
        let mismatched_prepare = qc(&context, 0, GlobalPhase::Prepare, vec![0, 1, 2]);
        proposal.justification = timeout_justification(
            timeout_round,
            Some(mismatched_prepare.clone()),
            Some(mismatched_prepare),
            0x44,
        );
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification)
        );
        let mut altered_carried = highest_prepare;
        altered_carried.round.view = 1;
        proposal.justification = timeout_justification(
            timeout_round,
            Some(qc(&context, 0, GlobalPhase::Prepare, vec![0, 1, 2])),
            Some(altered_carried),
            0x45,
        );
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification)
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the complete control-message vector keeps every canonical domain-separated signature preimage binding visible in one test"
    )]
    fn signed_control_messages_have_canonical_domain_separated_preimages() {
        let context = context(&[1, 1, 1, 1]);
        let proposal_round = round(&context, 0);
        let mut manifest = manifest(&context);
        manifest.round = proposal_round;
        let proposal = Proposal {
            round: proposal_round,
            proposer: context.leader(0),
            subject: manifest.subject,
            manifest: manifest.clone(),
            justification: ProposalJustification::ParentCommit(ParentCommitJustification {
                certificate: None,
            }),
            signature: vec![0x11; 48],
        };
        assert_eq!(proposal.validate(&context), Ok(()));
        assert!(
            proposal
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:proposal")
        );
        let mut changed_signature = proposal.clone();
        changed_signature.signature = vec![0x22; 48];
        assert_eq!(
            changed_signature.signature_preimage(),
            proposal.signature_preimage()
        );
        let vote = Vote {
            round: proposal_round,
            proposal_round,
            phase: GlobalPhase::Prepare,
            subject: proposal.subject,
            execution_commitment: execution_commitment(0x33),
            signer: 0,
            signature: vec![0x33; 48],
        };
        assert_eq!(vote.validate(&context), Ok(()));
        let mut prepare_with_other_origin = vote.clone();
        prepare_with_other_origin.proposal_round.view = prepare_with_other_origin
            .proposal_round
            .view
            .checked_add(1)
            .expect("fixture view increment");
        assert_eq!(
            prepare_with_other_origin.validate(&context),
            Err(ValidationError::InvalidProposalRound)
        );
        let mut commit_with_future_origin = vote.clone();
        commit_with_future_origin.phase = GlobalPhase::Commit;
        commit_with_future_origin.proposal_round.view = commit_with_future_origin
            .round
            .view
            .checked_add(1)
            .expect("fixture view increment");
        assert_eq!(
            commit_with_future_origin.validate(&context),
            Err(ValidationError::InvalidProposalRound)
        );
        let mut cross_context_origin = vote.clone();
        cross_context_origin.proposal_round.context_id = HeightContextId(
            HashOf::from_untyped_unchecked(Hash::new(b"foreign proposal origin context")),
        );
        assert_eq!(
            cross_context_origin.validate(&context),
            Err(ValidationError::WrongHeightContext)
        );
        let mut cross_height_origin = vote.clone();
        cross_height_origin.proposal_round.height = context.height + 1;
        assert_eq!(
            cross_height_origin.validate(&context),
            Err(ValidationError::WrongHeightContext)
        );
        assert!(
            vote.signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:vote")
        );
        let mut different_execution = vote.clone();
        different_execution.execution_commitment = execution_commitment(0x34);
        assert_ne!(
            different_execution.signature_preimage(),
            vote.signature_preimage(),
            "vote signatures must authenticate the deterministic execution result"
        );
        let timeout = TimeoutVote {
            round: proposal_round,
            highest_prepare_qc: None,
            signer: 1,
            signature: vec![0x44; 48],
        };
        assert_eq!(timeout.validate(&context), Ok(()));
        assert!(
            timeout
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:timeout-vote")
        );
        let mut oversized = vote.clone();
        oversized.signature = vec![0x45; MAX_CONSENSUS_SIGNATURE_BYTES + 1];
        assert_eq!(
            oversized.validate(&context),
            Err(ValidationError::SignatureTooLarge)
        );
        let mut unsigned = vote;
        unsigned.signature.clear();
        assert_eq!(
            unsigned.validate(&context),
            Err(ValidationError::MissingSignature)
        );
    }
    #[test]
    fn view_zero_proposal_accepts_equivalent_parent_decision_across_reproposal_rounds() {
        let ParentReproposalFixture {
            context,
            parent_round,
            mut proposal,
        } = parent_reproposal_fixture();
        assert_eq!(proposal.validate(&context), Ok(()));
        {
            let certificate = carried_parent_certificate(&mut proposal);
            certificate.round.view += 1;
            certificate.proposal_round = certificate.round;
        }
        assert_eq!(
            proposal.validate(&context),
            Ok(()),
            "an unchanged re-proposal may decide the same parent body in another round"
        );
        {
            let carried = carried_parent_certificate(&mut proposal);
            carried.round = parent_round;
            carried.round.context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"different proposal parent context",
            )));
            carried.proposal_round = carried.round;
        }
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification)
        );
        {
            let carried = carried_parent_certificate(&mut proposal);
            carried.round = parent_round;
            carried.proposal_round = parent_round;
            carried.subject = subject(0x71);
        }
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification)
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the complete certified-body mutation matrix keeps request, manifest, chunk, and body-hash bindings together as one protocol vector"
    )]
    fn certified_body_response_binds_request_manifest_and_body_hash() {
        let context = context(&[1, 1, 1, 1]);
        let body = b"certified body".to_vec();
        let round = round(&context, 1);
        let mut response_subject = subject(9);
        response_subject.payload_hash = Hash::new(&body);
        let chunks = rs16_fixture_chunks(&context, &body);
        let manifest = PayloadManifest::derive(
            &context,
            round,
            response_subject,
            u64::try_from(body.len()).expect("small body"),
            &chunks,
        )
        .expect("valid response manifest");
        let request = CertifiedBodyRequest {
            round: manifest.round,
            subject: manifest.subject,
            certificate: QuorumCertificate {
                round: manifest.round,
                proposal_round: manifest.round,
                phase: GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: execution_commitment(9),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x55; 48],
            },
            requester: context.roster[3].validator.clone(),
            signature: vec![0x66; 48],
        };
        assert_eq!(request.validate(&context), Ok(()));
        let mut split_round_request = request.clone();
        split_round_request.certificate.phase = GlobalPhase::Commit;
        split_round_request.certificate.round.view = split_round_request
            .certificate
            .round
            .view
            .checked_add(1)
            .expect("fixture split-round view increment");
        assert_eq!(
            split_round_request.validate(&context),
            Err(ValidationError::InvalidProposalRound)
        );
        let mut reproposal_request = request.clone();
        reproposal_request.round.view = reproposal_request
            .round
            .view
            .checked_add(1)
            .expect("fixture reproposal view increment");
        reproposal_request.certificate.round = reproposal_request.round;
        reproposal_request.certificate.proposal_round = reproposal_request.round;
        reproposal_request.certificate.phase = GlobalPhase::Commit;
        assert_eq!(reproposal_request.validate(&context), Ok(()));
        let mut mismatched_request_round = reproposal_request.clone();
        mismatched_request_round.round.view = mismatched_request_round
            .round
            .view
            .checked_add(1)
            .expect("fixture request-round increment");
        assert_eq!(
            mismatched_request_round.validate(&context),
            Err(ValidationError::CertifiedBodyCertificateMismatch)
        );
        let observer_request = CertifiedBodyRequest {
            requester: peer(99),
            ..request.clone()
        };
        assert_eq!(observer_request.validate(&context), Ok(()));
        let response = CertifiedBodyResponse {
            request_hash: HashOf::new(&request),
            manifest,
            body,
            responder: 0,
            signature: vec![0x77; 48],
        };
        assert_eq!(response.validate(&context), Ok(()));
        assert_eq!(
            response.validate_against(&context, &request, &context.roster[0].validator),
            Ok(())
        );
        let reproposal_manifest = PayloadManifest::derive(
            &context,
            reproposal_request.round,
            response_subject,
            u64::try_from(response.body.len()).expect("small body"),
            &chunks,
        )
        .expect("valid reproposal response manifest");
        let reproposal_response = CertifiedBodyResponse {
            request_hash: HashOf::new(&reproposal_request),
            manifest: reproposal_manifest,
            ..response.clone()
        };
        assert_eq!(
            reproposal_response.validate_against(
                &context,
                &reproposal_request,
                &context.roster[0].validator,
            ),
            Ok(())
        );
        assert_eq!(
            response.validate_against(&context, &request, &context.roster[1].validator),
            Err(ValidationError::ResponderIdentityMismatch)
        );
        let mut archive_response = response.clone();
        archive_response.responder = 3;
        assert_eq!(
            archive_response.validate_against(&context, &request, &context.roster[3].validator,),
            Ok(())
        );
        assert_eq!(
            archive_response.validate_against(&context, &request, &context.roster[2].validator,),
            Err(ValidationError::ResponderIdentityMismatch)
        );
        let mut wrong_request = response.clone();
        wrong_request.request_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong request"));
        assert_eq!(
            wrong_request.validate_against(&context, &request, &context.roster[0].validator),
            Err(ValidationError::CertifiedBodyRequestMismatch)
        );
        assert!(
            response
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:certified-body-response")
        );
        let mut corrupted = response;
        corrupted.body.push(0);
        assert_eq!(
            corrupted.validate(&context),
            Err(ValidationError::CertifiedBodyHashMismatch)
        );
    }
    #[test]
    fn commit_certificate_discovery_binds_network_context_request_and_commit_phase() {
        let context = context(&[1, 1, 1, 1]);
        let commit = qc(&context, 9, GlobalPhase::Commit, vec![0, 1, 2]);
        let request = CommitCertificateRequest {
            protocol_version: PROTOCOL_VERSION,
            network_id: context.network_id,
            context_id: context.id(),
            height: context.height,
            requester: peer(99),
            signature: vec![0x81; 48],
        };
        assert_eq!(request.validate(&context), Ok(()));
        assert!(
            request
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:commit-certificate-request")
        );
        let response = CommitCertificateResponse {
            request_hash: HashOf::new(&request),
            certificate: commit.clone(),
            responder: peer(100),
            signature: vec![0x82; 48],
        };
        assert_eq!(response.validate_against(&context, &request), Ok(()));
        assert!(
            response
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:commit-certificate-response")
        );
        let mut cross_network = request.clone();
        cross_network.network_id = network_id(0xA2);
        assert_eq!(
            cross_network.validate(&context),
            Err(ValidationError::WrongHeightContext)
        );
        let mut wrong_height = request.clone();
        wrong_height.height += 1;
        assert_eq!(
            wrong_height.validate(&context),
            Err(ValidationError::WrongHeightContext)
        );
        let mut wrong_protocol = request.clone();
        wrong_protocol.protocol_version += 1;
        assert!(matches!(
            wrong_protocol.validate(&context),
            Err(ValidationError::UnsupportedProtocolVersion { .. })
        ));
        let mut wrong_request = response.clone();
        wrong_request.request_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"another exact request"));
        assert_eq!(
            wrong_request.validate_against(&context, &request),
            Err(ValidationError::CommitCertificateRequestMismatch)
        );
        let mut prepare = response;
        prepare.certificate.phase = GlobalPhase::Prepare;
        assert_eq!(
            prepare.validate(&context),
            Err(ValidationError::CommitCertificateMismatch)
        );
        let mut changed_responder = CommitCertificateResponse {
            request_hash: HashOf::new(&request),
            certificate: commit,
            responder: peer(100),
            signature: vec![0x82; 48],
        };
        let original_preimage = changed_responder.signature_preimage();
        changed_responder.responder = peer(101);
        assert_ne!(changed_responder.signature_preimage(), original_preimage);
    }
    fn status(context: &HeightContext) -> SumeragiV2Status {
        SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"status-node"),
            build_fingerprint: Hash::new(b"status-build"),
            config_fingerprint: Hash::new(b"status-config"),
            restart_required: false,
            height_context_id: context.id(),
            height: context.height,
            view: 3,
            phase: SumeragiV2StatusPhase::AwaitingProposal,
            leader: 0,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Missing,
            pending_persistence_id: None,
            last_committed_height: 0,
            last_committed_subject: None,
            height_context: SumeragiV2HeightContextStatus {
                epoch: context.epoch,
                epoch_end_height: context.epoch_end_height,
                mode: context.mode,
                epoch_seed: context.leader_seed,
                validator_count: u32::try_from(context.roster.len())
                    .expect("test roster fits status count"),
                quorum: context.quorum,
            },
            last_commit_qc: None,
            liveness: SumeragiV2LivenessStatus::default(),
        }
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the complete status rejection matrix documents the ordered scalar, frontier, and phase invariants in one canonical protocol vector"
    )]
    fn status_validation_rejects_impossible_scalar_and_phase_states() {
        use SumeragiV2StatusValidationError as Error;
        let context = context(&[1, 1, 1, 1]);
        let baseline = status(&context);
        assert_eq!(baseline.validate(), Ok(()));
        let mut wrong_protocol = baseline.clone();
        wrong_protocol.protocol_version += 1;
        assert!(matches!(
            wrong_protocol.validate(),
            Err(Error::UnsupportedProtocolVersion { .. })
        ));
        let mut wrong_body = baseline.clone();
        wrong_body.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(wrong_body.validate(), Err(Error::PhaseBodyMismatch));
        let mut commit_without_lock = baseline.clone();
        commit_without_lock.phase = SumeragiV2StatusPhase::Commit;
        commit_without_lock.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(
            commit_without_lock.validate(),
            Err(Error::CommitWithoutLock)
        );
        let mut zero_persistence = baseline.clone();
        zero_persistence.pending_persistence_id = Some(0);
        assert_eq!(zero_persistence.validate(), Err(Error::ZeroPersistenceId));
        let mut committed_ahead = baseline.clone();
        committed_ahead.last_committed_height = committed_ahead.height;
        assert_eq!(
            committed_ahead.validate(),
            Err(Error::CommittedHeightNotBehindActiveHeight)
        );
        let mut pending_apply = baseline;
        pending_apply.phase = SumeragiV2StatusPhase::PendingApply;
        pending_apply.body_state = SumeragiV2BodyState::PendingApply;
        assert_eq!(
            pending_apply.validate(),
            Err(Error::PendingApplyCommitMismatch)
        );
        pending_apply.last_committed_height = pending_apply.height;
        let committed = qc(
            &context,
            pending_apply.view,
            GlobalPhase::Commit,
            vec![0, 1, 2],
        );
        pending_apply.last_committed_subject = Some(committed.subject);
        pending_apply.last_commit_qc = Some(SumeragiV2CommitQcStatus {
            certificate: committed.as_ref(),
            validator_count: 4,
            signer_count: 3,
            min_signers: 3,
            signed_power: 3,
            total_power: 4,
        });
        assert_eq!(pending_apply.validate(), Ok(()));
        let mut invalid_commit_origin = pending_apply.clone();
        let invalid_certificate = &mut invalid_commit_origin
            .last_commit_qc
            .as_mut()
            .expect("commit summary")
            .certificate;
        invalid_certificate.proposal_round.view = invalid_certificate.round.view + 1;
        assert_eq!(
            invalid_commit_origin.validate(),
            Err(Error::CommitSummaryCertificateMismatch)
        );
        let mut invalid_context = status(&context);
        invalid_context.height_context.epoch_end_height = invalid_context.height - 1;
        assert_eq!(
            invalid_context.validate(),
            Err(Error::EpochEndsBeforeHeight)
        );
        let mut invalid_leader = status(&context);
        invalid_leader.leader = invalid_leader.height_context.validator_count;
        assert_eq!(invalid_leader.validate(), Err(Error::LeaderOutOfRange));
        let mut invalid_quorum = status(&context);
        invalid_quorum.height_context.quorum.min_signers -= 1;
        assert_eq!(
            invalid_quorum.validate(),
            Err(Error::InvalidHeightContextQuorum)
        );
        let mut invalid_commit_summary = pending_apply.clone();
        invalid_commit_summary
            .last_commit_qc
            .as_mut()
            .expect("commit summary")
            .signed_power = 2;
        assert_eq!(
            invalid_commit_summary.validate(),
            Err(Error::InvalidCommitSummaryQuorum)
        );
        let mut impossible_signer_power = pending_apply;
        let impossible_summary = impossible_signer_power
            .last_commit_qc
            .as_mut()
            .expect("commit summary");
        impossible_summary.signer_count = 4;
        impossible_summary.signed_power = 3;
        assert_eq!(
            impossible_signer_power.validate(),
            Err(Error::InvalidCommitSummaryQuorum),
            "the redundant signed-vote projection must equal the authenticated signer count"
        );
        let mut one_sided_commit = status(&context);
        one_sided_commit.height = 2;
        one_sided_commit.last_committed_height = 1;
        one_sided_commit.last_committed_subject = Some(subject(91));
        assert_eq!(
            one_sided_commit.validate(),
            Err(Error::CommitFrontierAuthenticationMismatch)
        );
    }
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "the complete liveness status matrix keeps round, quorum, timeout, and queue-ownership invariants together as one canonical protocol vector"
    )]
    fn status_validation_checks_liveness_rounds_quorums_and_queue_ownership() {
        use SumeragiV2StatusValidationError as Error;
        let context = context(&[1, 1, 1, 1]);
        let mut baseline = status(&context);
        let active_round = round(&context, 2);
        baseline.liveness = SumeragiV2LivenessStatus {
            generation: 4,
            prepare_quorums: vec![SumeragiV2VoteQuorumStatus {
                round: active_round,
                proposal_round: active_round,
                subject: subject(41),
                execution_commitment: execution_commitment(42),
                signer_count: 2,
                signed_power: 2,
                min_signers: 3,
                total_power: 4,
            }],
            outbound_intents: vec![SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::Proposal,
                round: active_round,
                proposal_round: Some(active_round),
                subject: Some(subject(41)),
                execution_commitment: None,
                stage: SumeragiV2OutboundIntentStage::Sent,
            }],
            queues: vec![SumeragiV2QueueStatus {
                queue: SumeragiV2QueueKind::RuntimeProgress,
                depth: 1,
                capacity: 4,
                oldest_age_ms: Some(7),
                service_debt: 2,
            }],
            last_progress: Some(SumeragiV2ProgressTransitionStatus {
                generation: 4,
                round: active_round,
                transition: SumeragiV2ProgressTransition::PrepareVoteAdmitted,
                age_ms: 7,
            }),
            ..SumeragiV2LivenessStatus::default()
        };
        assert_eq!(baseline.validate(), Ok(()));
        let mut future_round = baseline.clone();
        future_round.liveness.prepare_quorums[0].round.view = future_round.view + 1;
        assert_eq!(
            future_round.validate(),
            Err(Error::LivenessRoundFromFutureView)
        );
        let mut cross_context_round = baseline.clone();
        cross_context_round.liveness.prepare_quorums[0]
            .round
            .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign liveness quorum context",
        )));
        assert_eq!(
            cross_context_round.validate(),
            Err(Error::LivenessRoundMismatch),
            "a liveness round must bind to the status height-context identity"
        );
        let mut cross_height_round = baseline.clone();
        cross_height_round.liveness.prepare_quorums[0].round.height =
            cross_height_round.liveness.prepare_quorums[0]
                .round
                .height
                .saturating_add(1);
        assert_eq!(
            cross_height_round.validate(),
            Err(Error::LivenessRoundMismatch),
            "a liveness round must bind independently to the status height"
        );
        let mut wrong_origin = baseline.clone();
        wrong_origin.liveness.prepare_quorums[0].proposal_round.view -= 1;
        assert_eq!(wrong_origin.validate(), Err(Error::InvalidProposalRound));
        let mut wrong_quorum = baseline.clone();
        wrong_quorum.liveness.prepare_quorums[0].total_power = 5;
        assert_eq!(wrong_quorum.validate(), Err(Error::InvalidLivenessQuorum));
        let mut invalid_queue = baseline.clone();
        invalid_queue.liveness.queues[0].depth = 0;
        assert_eq!(invalid_queue.validate(), Err(Error::InvalidLivenessQueue));
        let mut every_queue_kind = baseline.clone();
        every_queue_kind.liveness.queues = [
            SumeragiV2QueueKind::Ingress,
            SumeragiV2QueueKind::DeferredNormal,
            SumeragiV2QueueKind::DeferredProgress,
            SumeragiV2QueueKind::DeferredCompletion,
            SumeragiV2QueueKind::RuntimeNormal,
            SumeragiV2QueueKind::RuntimeProgress,
            SumeragiV2QueueKind::RuntimeCompletion,
            SumeragiV2QueueKind::EffectCompletion,
            SumeragiV2QueueKind::NetworkIngress,
            SumeragiV2QueueKind::EffectDispatch,
        ]
        .into_iter()
        .map(|queue| SumeragiV2QueueStatus {
            queue,
            depth: 0,
            capacity: 1,
            oldest_age_ms: None,
            service_debt: 0,
        })
        .collect();
        assert_eq!(every_queue_kind.validate(), Ok(()));
        let mut too_many_queues = every_queue_kind;
        too_many_queues.liveness.queues.push(SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::NetworkIngress,
            depth: 0,
            capacity: 1,
            oldest_age_ms: None,
            service_debt: 0,
        });
        assert_eq!(
            too_many_queues.validate(),
            Err(Error::LivenessCollectionTooLarge)
        );
        let mut invalid_intent = baseline.clone();
        invalid_intent.liveness.outbound_intents[0].execution_commitment =
            Some(execution_commitment(42));
        assert_eq!(
            invalid_intent.validate(),
            Err(Error::InvalidOutboundIntentShape)
        );
        let mut missing_intent_origin = baseline.clone();
        missing_intent_origin.liveness.outbound_intents[0].proposal_round = None;
        assert_eq!(
            missing_intent_origin.validate(),
            Err(Error::InvalidOutboundIntentShape)
        );
        let mut mismatched_prepare_origin = baseline.clone();
        mismatched_prepare_origin.liveness.outbound_intents[0]
            .proposal_round
            .as_mut()
            .expect("proposal origin")
            .view -= 1;
        assert_eq!(
            mismatched_prepare_origin.validate(),
            Err(Error::InvalidProposalRound)
        );
        let mut cross_context_intent_origin = baseline.clone();
        cross_context_intent_origin.liveness.outbound_intents[0]
            .proposal_round
            .as_mut()
            .expect("proposal origin")
            .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign outbound-intent origin",
        )));
        assert_eq!(
            cross_context_intent_origin.validate(),
            Err(Error::LivenessRoundMismatch)
        );
        let mut timeout_with_origin = baseline.clone();
        let intent = &mut timeout_with_origin.liveness.outbound_intents[0];
        intent.kind = SumeragiV2OutboundIntentKind::TimeoutVote;
        intent.subject = None;
        assert_eq!(
            timeout_with_origin.validate(),
            Err(Error::InvalidOutboundIntentShape)
        );
        let mut same_round_commit = baseline.clone();
        let intent = &mut same_round_commit.liveness.outbound_intents[0];
        intent.kind = SumeragiV2OutboundIntentKind::CommitVote;
        intent.execution_commitment = Some(execution_commitment(42));
        assert_eq!(same_round_commit.validate(), Ok(()));
        let mut stale_commit_round = same_round_commit.clone();
        let intent = &mut stale_commit_round.liveness.outbound_intents[0];
        intent.proposal_round.as_mut().expect("proposal round").view -= 1;
        assert_eq!(
            stale_commit_round.validate(),
            Err(Error::InvalidProposalRound)
        );
        let mut future_commit_origin = same_round_commit;
        future_commit_origin.liveness.outbound_intents[0]
            .proposal_round
            .as_mut()
            .expect("proposal origin")
            .view = active_round.view + 1;
        assert_eq!(
            future_commit_origin.validate(),
            Err(Error::InvalidProposalRound)
        );
        let mut future_generation = baseline;
        future_generation
            .liveness
            .last_progress
            .as_mut()
            .expect("progress record")
            .generation += 1;
        assert_eq!(
            future_generation.validate(),
            Err(Error::LivenessGenerationFromFuture)
        );
    }
    #[test]
    fn status_validation_accepts_all_ignore_reasons_and_rejects_a_thirteenth_entry() {
        use SumeragiV2StatusValidationError as Error;
        let context = context(&[1, 1, 1, 1]);
        let mut exact_bound = status(&context);
        exact_bound.liveness.ignore_counts = [
            SumeragiV2IgnoreReason::WrongHeight,
            SumeragiV2IgnoreReason::WrongView,
            SumeragiV2IgnoreReason::StaleGeneration,
            SumeragiV2IgnoreReason::Busy,
            SumeragiV2IgnoreReason::Duplicate,
            SumeragiV2IgnoreReason::NoMatchingWork,
            SumeragiV2IgnoreReason::Observer,
            SumeragiV2IgnoreReason::ViewClosed,
            SumeragiV2IgnoreReason::AlreadyDecided,
            SumeragiV2IgnoreReason::RecoveryPending,
            SumeragiV2IgnoreReason::IrrelevantView,
            SumeragiV2IgnoreReason::UnsafeProposal,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, reason)| SumeragiV2IgnoreCount {
            reason,
            count: u64::try_from(index + 1).expect("ignore reason count fits u64"),
        })
        .collect();
        assert_eq!(exact_bound.liveness.ignore_counts.len(), 12);
        assert_eq!(exact_bound.validate(), Ok(()));
        let mut oversized = exact_bound;
        oversized
            .liveness
            .ignore_counts
            .push(SumeragiV2IgnoreCount {
                reason: SumeragiV2IgnoreReason::UnsafeProposal,
                count: 13,
            });
        assert_eq!(oversized.validate(), Err(Error::LivenessCollectionTooLarge));
    }
    #[test]
    fn status_validation_accepts_later_view_active_height_finality_evidence() {
        use SumeragiV2StatusValidationError as Error;
        let context = context(&[1, 1, 1, 1]);
        let mut lagging = status(&context);
        let later_commit = qc(
            &context,
            lagging.view + 1,
            GlobalPhase::Commit,
            vec![0, 1, 2],
        );
        lagging.phase = SumeragiV2StatusPhase::PendingApply;
        lagging.body_state = SumeragiV2BodyState::PendingApply;
        lagging.last_committed_height = lagging.height;
        lagging.last_committed_subject = Some(later_commit.subject);
        lagging.last_commit_qc = Some(SumeragiV2CommitQcStatus {
            certificate: later_commit.as_ref(),
            validator_count: 4,
            signer_count: 3,
            min_signers: 3,
            signed_power: 3,
            total_power: 4,
        });
        lagging.liveness = SumeragiV2LivenessStatus {
            generation: 8,
            outbound_intents: vec![SumeragiV2OutboundIntentStatus {
                kind: SumeragiV2OutboundIntentKind::CommitQc,
                round: later_commit.round,
                proposal_round: Some(later_commit.proposal_round),
                subject: Some(later_commit.subject),
                execution_commitment: Some(later_commit.execution_commitment),
                stage: SumeragiV2OutboundIntentStage::Sent,
            }],
            last_progress: Some(SumeragiV2ProgressTransitionStatus {
                generation: 8,
                round: later_commit.round,
                transition: SumeragiV2ProgressTransition::DecisionPersisted,
                age_ms: 1,
            }),
            ..SumeragiV2LivenessStatus::default()
        };
        assert_eq!(lagging.validate(), Ok(()));
        lagging
            .liveness
            .last_progress
            .as_mut()
            .expect("progress record")
            .transition = SumeragiV2ProgressTransition::CommitQuorum;
        assert_eq!(lagging.validate(), Ok(()));
        let mut wrong_height_finality = lagging.clone();
        wrong_height_finality.liveness.outbound_intents[0]
            .round
            .height += 1;
        assert_eq!(
            wrong_height_finality.validate(),
            Err(Error::LivenessRoundMismatch)
        );
        for kind in [
            SumeragiV2OutboundIntentKind::Proposal,
            SumeragiV2OutboundIntentKind::PrepareVote,
            SumeragiV2OutboundIntentKind::CommitVote,
            SumeragiV2OutboundIntentKind::PrepareQc,
            SumeragiV2OutboundIntentKind::TimeoutVote,
            SumeragiV2OutboundIntentKind::TimeoutCertificate,
        ] {
            let mut future_nonfinality_intent = lagging.clone();
            future_nonfinality_intent.liveness.outbound_intents[0].kind = kind;
            assert_eq!(
                future_nonfinality_intent.validate(),
                Err(Error::LivenessRoundFromFutureView)
            );
        }
        for transition in [
            SumeragiV2ProgressTransition::ProposalAdmitted,
            SumeragiV2ProgressTransition::BodyAvailable,
            SumeragiV2ProgressTransition::BodyStored,
            SumeragiV2ProgressTransition::BodyValidated,
            SumeragiV2ProgressTransition::PrepareVoteAdmitted,
            SumeragiV2ProgressTransition::CommitVoteAdmitted,
            SumeragiV2ProgressTransition::TimeoutVoteAdmitted,
            SumeragiV2ProgressTransition::PrepareQuorum,
            SumeragiV2ProgressTransition::LockInstalled,
            SumeragiV2ProgressTransition::TimeoutCertificateInstalled,
            SumeragiV2ProgressTransition::Applied,
            SumeragiV2ProgressTransition::SuccessorHeightActivated,
            SumeragiV2ProgressTransition::RecoveryReplayed,
        ] {
            let mut future_nonfinality_progress = lagging.clone();
            future_nonfinality_progress
                .liveness
                .last_progress
                .as_mut()
                .expect("progress record")
                .transition = transition;
            assert_eq!(
                future_nonfinality_progress.validate(),
                Err(Error::LivenessRoundFromFutureView)
            );
        }
    }
    #[test]
    fn status_validation_bounds_current_commit_groups_plus_historical_lock() {
        use SumeragiV2StatusValidationError as Error;
        let powers = vec![1; MAX_VALIDATORS_PER_HEIGHT];
        let context = context(&powers);
        let mut snapshot = status(&context);
        let quorum = SumeragiV2VoteQuorumStatus {
            round: round(&context, snapshot.view),
            proposal_round: round(&context, snapshot.view),
            subject: subject(71),
            execution_commitment: execution_commitment(72),
            signer_count: 1,
            signed_power: 1,
            min_signers: context.quorum.min_signers,
            total_power: context.quorum.total_power,
        };
        snapshot.liveness.commit_quorums = vec![quorum; MAX_COMMIT_QUORUM_GROUPS_PER_HEIGHT];
        assert_eq!(snapshot.validate(), Ok(()));
        snapshot.liveness.commit_quorums.push(quorum);
        assert_eq!(snapshot.validate(), Err(Error::LivenessCollectionTooLarge));
    }
    #[test]
    fn status_validation_rejects_cross_context_and_future_certificates() {
        use SumeragiV2StatusValidationError as Error;
        let context = context(&[1, 1, 1, 1]);
        let baseline = status(&context);
        let prepare = qc(&context, 2, GlobalPhase::Prepare, vec![0, 1, 2]).as_ref();
        let mut with_certificates = baseline.clone();
        with_certificates.locked_prepare_qc = Some(prepare);
        with_certificates.highest_prepare_qc = Some(prepare);
        assert_eq!(with_certificates.validate(), Ok(()));
        let mut prepare_with_lock = with_certificates.clone();
        prepare_with_lock.phase = SumeragiV2StatusPhase::Prepare;
        prepare_with_lock.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(prepare_with_lock.validate(), Err(Error::PrepareWithLock));
        let mut conflicting_same_view = with_certificates.clone();
        conflicting_same_view
            .highest_prepare_qc
            .as_mut()
            .unwrap()
            .subject = subject(91);
        assert_eq!(
            conflicting_same_view.validate(),
            Err(Error::ConflictingCertificatesAtSameView)
        );
        let mut missing_highest = with_certificates.clone();
        missing_highest.highest_prepare_qc = None;
        assert_eq!(
            missing_highest.validate(),
            Err(Error::LockedCertificateWithoutHighest)
        );
        let mut wrong_phase = with_certificates.clone();
        wrong_phase.highest_prepare_qc.as_mut().unwrap().phase = GlobalPhase::Commit;
        assert_eq!(wrong_phase.validate(), Err(Error::CertificatePhaseMismatch));
        let mut wrong_context = with_certificates.clone();
        wrong_context
            .highest_prepare_qc
            .as_mut()
            .unwrap()
            .round
            .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"wrong-status-context",
        )));
        assert_eq!(
            wrong_context.validate(),
            Err(Error::CertificateContextMismatch)
        );
        let mut future = with_certificates.clone();
        let future_certificate = future.highest_prepare_qc.as_mut().unwrap();
        future_certificate.round.view = future.view + 1;
        future_certificate.proposal_round = future_certificate.round;
        assert_eq!(future.validate(), Err(Error::CertificateFromFutureView));
        let mut wrong_origin = with_certificates.clone();
        wrong_origin
            .highest_prepare_qc
            .as_mut()
            .unwrap()
            .proposal_round
            .view -= 1;
        assert_eq!(wrong_origin.validate(), Err(Error::InvalidProposalRound));
        let mut timeout_not_past = baseline;
        timeout_not_past.last_timeout_certificate = Some(TimeoutCertificateRef {
            round: round(&context, timeout_not_past.view),
            highest_prepare_qc: Some(prepare),
            certificate_hash: HashOf::from_untyped_unchecked(Hash::new(b"status-timeout")),
        });
        assert_eq!(
            timeout_not_past.validate(),
            Err(Error::TimeoutNotBeforeCurrentView)
        );
    }
    // Keep the JSON wire-contract matrix isolated from the core consensus tests.
    include!("consensus_v2_json_tests.rs");
    #[test]
    fn leader_rotation_is_power_independent_and_wraps_roster() {
        let equal = context(&[1, 1, 1, 1]);
        let weighted = context(&[70, 10, 10, 10]);
        let start = equal.leader(0);
        assert_eq!(weighted.leader(0), start);
        assert_eq!(equal.leader(4), start);
        assert_eq!(weighted.leader(17), equal.leader(17));
        assert_eq!(
            (0..4)
                .map(|view| equal.leader(view))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1, 2, 3])
        );
    }
}
