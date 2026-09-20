//! Validate and consume the original source into its sole replay/session ingress.
use super::*;
impl<R: crate::vega::MaskedRelaxedRandomSourceV1, K, P> SourceReplayIngressV1<R, K, P> {
    pub(super) fn begin_v1(mut self) -> Result<SourceReplayAssemblyV1<R, K, P>, ZkAmsMkheErrorV1> {
        let mut prerequisite = self
            .prerequisite
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let sink = self
            .sink
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_prerequisite_record_v2(&prerequisite.record)?;
        let owner = prerequisite
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        owner.owner.validate_v1()?;
        let source_receipt_digest = owner.owner.source.receipt_v1().receipt_digest_v1();
        let topology_digest = global_lookup_topology_digest_v1();
        let plane_mapping_digest = exact_mapping_digest_v1()?;
        let axes = SourceReplayContextAxesV1 {
            source_receipt_digest,
            prerequisite_record_digest: prerequisite.record.record_digest,
            source_formula_digest: prerequisite.record.formula_digest,
            source_mapping_digest: prerequisite.record.mapping_digest,
            ordered_bundle_root: prerequisite.record.ordered_bundle_root,
            source_lineage_root: prerequisite.record.source_lineage_root,
            output_lineage_root: prerequisite.record.output_lineage_root,
            preflight_digest: prerequisite.record.preflight_digest,
            aggregate_schedule_digest: prerequisite.record.aggregate_schedule_digest,
        };
        let context_digest = spool_context_digest_v1(axes, plane_mapping_digest, topology_digest)?;
        let proof_session_entropy =
            GlobalLookupProofSessionEntropySealV1::from_original_materialized_source_v1(
                &mut prerequisite
                    .live
                    .as_mut()
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
                    .owner,
            )?;
        let directory = sink.into_directory_v1();
        let openings = SourceOpeningAssemblyV1::begin_v1(
            source_receipt_digest,
            prerequisite.record.record_digest,
            context_digest,
            proof_session_entropy,
            &directory,
        )?;
        let layout = ConfidentialSpoolLayoutV1::new_v1(
            COMPACT_PLANE_COUNT_V1 as u64,
            COMPACT_PLANE_BYTES_V1,
            context_digest,
        )
        .map_err(map_leaf_error_v1)?;
        if layout.slot_count_v1() != COMPACT_PLANE_COUNT_V1 as u64
            || layout.plaintext_len_v1() != COMPACT_PLANE_BYTES_V1
            || layout.file_len_v1() != COMPACT_SPOOL_FILE_BYTES_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let writer = ConfidentialSpoolWriterV1::create_in_v1(&directory, layout)
            .map_err(map_leaf_error_v1)?;
        let mut authenticated_read_schedule_hash = Keccak256::new();
        authenticated_read_schedule_hash.update(AUTHENTICATED_READ_SCHEDULE_DOMAIN_V1);
        authenticated_read_schedule_hash.update(&[SOURCE_REPLAY_VERSION_V1]);
        authenticated_read_schedule_hash.update(&source_receipt_digest);
        authenticated_read_schedule_hash.update(&topology_digest);
        authenticated_read_schedule_hash.update(&plane_mapping_digest);
        authenticated_read_schedule_hash.update(&context_digest);
        authenticated_read_schedule_hash
            .update(&(TOTAL_SOURCE_READ_BLOCKS_V1 as u32).to_be_bytes());
        Ok(SourceReplayAssemblyV1 {
            live: Some(SourceReplayLiveV1 {
                prerequisite,
                writer,
                openings,
                context_digest,
                topology_digest,
                plane_mapping_digest,
                source_receipt_digest,
                authenticated_read_schedule_hash,
                next_record: 0,
                next_canonical_block: 0,
                canonical_complete: false,
                next_role: 0,
                next_plane: 0,
                next_output_slot: 0,
            }),
        })
    }
}
