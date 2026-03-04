//! Proof registry query helpers shared with Torii.

use iroha_data_model::proof::{ProofId, ProofRecord, ProofStatus};
use mv::storage::StorageReadOnly;

use crate::state::{State, WorldReadOnly};

/// Filters applied when querying proof records.
#[derive(Debug, Clone)]
pub struct ProofFilters<'a> {
    /// Restrict results to a specific backend (e.g., `halo2/ipa`).
    pub backend: Option<&'a str>,
    /// Restrict results to a specific verification status.
    pub status: Option<ProofStatus>,
    /// When true, only bridge proof records are returned.
    pub bridge_only: bool,
    /// When true, only pinned bridge proof records are returned (implies `bridge_only`).
    pub pinned_only: bool,
    /// Minimum bridge range start height (inclusive) when `bridge_only` is set.
    pub bridge_min_range_start: Option<u64>,
    /// Maximum bridge range end height (inclusive) when `bridge_only` is set.
    pub bridge_max_range_end: Option<u64>,
    /// Require the proof to carry a specific ZK1 TLV tag.
    pub has_tag: Option<[u8; 4]>,
    /// Minimum `verified_at_height` (inclusive).
    pub min_height: Option<u64>,
    /// Maximum `verified_at_height` (inclusive).
    pub max_height: Option<u64>,
}

/// Pagination controls for proof listings.
#[derive(Debug, Clone)]
pub struct ProofListParams<'a> {
    /// Filter set applied before ordering/pagination.
    pub filters: ProofFilters<'a>,
    /// When true, results are returned in descending order of verification height.
    pub descending: bool,
    /// Optional offset applied after ordering.
    pub offset: Option<u32>,
    /// Optional limit applied after offset (server-side cap enforced at 1000).
    pub limit: Option<u32>,
}

/// Materialised proof entry returned by the listing helper.
#[derive(Debug, Clone)]
pub struct ProofListItem {
    /// Stable proof identifier (backend + proof hash).
    pub id: ProofId,
    /// Stored proof metadata (status, VK references, height).
    pub record: ProofRecord,
}

/// List proof records using the supplied filters and pagination controls.
pub fn list_proofs(state: &State, params: &ProofListParams<'_>) -> Vec<ProofListItem> {
    let world = state.world_view();
    let mut entries = collect_filtered(&world, &params.filters);
    entries.sort_by(|a, b| {
        let ha = a.record.verified_at_height.unwrap_or(0);
        let hb = b.record.verified_at_height.unwrap_or(0);
        ha.cmp(&hb).then_with(|| a.id.cmp(&b.id))
    });
    if params.descending {
        entries.reverse();
    }
    let start = params.offset.unwrap_or(0) as usize;
    if start >= entries.len() {
        return Vec::new();
    }
    let cap = params.limit.unwrap_or(0).min(1000) as usize;
    let end = if cap == 0 {
        entries.len()
    } else {
        (start + cap).min(entries.len())
    };
    entries[start..end].to_vec()
}

/// Count proof records matching the supplied filters (ignores pagination controls).
pub fn count_proofs(state: &State, filters: &ProofFilters<'_>) -> u64 {
    let world = state.world_view();
    collect_filtered(&world, filters).len() as u64
}

fn collect_filtered(world: &impl WorldReadOnly, filters: &ProofFilters<'_>) -> Vec<ProofListItem> {
    let mut items: Vec<(ProofId, ProofRecord)> = Vec::new();

    let matches = |id: &ProofId, record: &ProofRecord| -> bool {
        if let Some(backend) = filters.backend
            && id.backend != backend
        {
            return false;
        }
        if let Some(status) = filters.status
            && record.status != status
        {
            return false;
        }
        if let Some(min_height) = filters.min_height {
            match record.verified_at_height {
                Some(h) if h >= min_height => {}
                _ => return false,
            }
        }
        if let Some(max_height) = filters.max_height {
            match record.verified_at_height {
                Some(h) if h <= max_height => {}
                _ => return false,
            }
        }
        if filters.bridge_only || filters.pinned_only {
            match record.bridge.as_ref() {
                Some(bridge) => {
                    if filters.pinned_only && !bridge.proof.pinned {
                        return false;
                    }
                    if let Some(min_range_start) = filters.bridge_min_range_start
                        && bridge.proof.range.start_height < min_range_start
                    {
                        return false;
                    }
                    if let Some(max_range_end) = filters.bridge_max_range_end
                        && bridge.proof.range.end_height > max_range_end
                    {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    };

    if let Some(tag) = filters.has_tag {
        let tag_slice: &[u8] = &tag;
        if let Some(ids) = world.proofs_by_tag().get(tag_slice) {
            for proof_id in ids {
                if let Some(record) = world.proofs().get(proof_id)
                    && matches(proof_id, record)
                {
                    items.push((proof_id.clone(), record.clone()));
                }
            }
        }
    } else {
        for (id, record) in world.proofs().iter() {
            if matches(id, record) {
                items.push((id.clone(), record.clone()));
            }
        }
    }

    items
        .into_iter()
        .map(|(id, record)| ProofListItem { id, record })
        .collect()
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        bridge::{
            BridgeProof, BridgeProofPayload, BridgeProofRange, BridgeProofRecord,
            BridgeTransparentProof,
        },
        proof::{ProofId, ProofRecord, ProofStatus},
    };
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn blank_state() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        State::new(World::new(), kura, query)
    }

    fn list_for_filters(state: &State, filters: ProofFilters<'_>) -> Vec<ProofListItem> {
        list_proofs(
            state,
            &ProofListParams {
                filters,
                descending: false,
                offset: None,
                limit: None,
            },
        )
    }

    fn bridge_proof_record(
        range: BridgeProofRange,
        pinned: bool,
        payload: BridgeProofPayload,
        manifest_hash: [u8; 32],
        commitment: [u8; 32],
        size_bytes: u32,
    ) -> BridgeProofRecord {
        BridgeProofRecord {
            proof: BridgeProof {
                range,
                manifest_hash,
                payload,
                pinned,
            },
            commitment,
            size_bytes,
        }
    }

    fn bridge_record(
        backend: &str,
        proof_hash: [u8; 32],
        proof: BridgeProofRecord,
        verified_at_height: u64,
    ) -> (ProofId, ProofRecord) {
        let id = ProofId {
            backend: backend.into(),
            proof_hash,
        };
        let record = ProofRecord {
            id: id.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(verified_at_height),
            bridge: Some(proof),
        };
        (id, record)
    }

    fn plain_record(
        backend: &str,
        proof_hash: [u8; 32],
        status: ProofStatus,
        verified_at_height: Option<u64>,
    ) -> (ProofId, ProofRecord) {
        let id = ProofId {
            backend: backend.into(),
            proof_hash,
        };
        let record = ProofRecord {
            id: id.clone(),
            vk_ref: None,
            vk_commitment: None,
            status,
            verified_at_height,
            bridge: None,
        };
        (id, record)
    }

    #[tokio::test]
    async fn list_and_count_filter_by_tag_and_status() {
        let state = blank_state();
        let backend = "halo2/ipa";
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let id_verified = ProofId {
            backend: backend.into(),
            proof_hash: [0x11; 32],
        };
        let rec_verified = ProofRecord {
            id: id_verified.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(42),
            bridge: None,
        };
        stx.world.proofs.insert(id_verified.clone(), rec_verified);
        stx.world
            .proof_tags
            .insert(id_verified.clone(), vec![*b"PROF"]);
        stx.world
            .proofs_by_tag
            .insert(*b"PROF", vec![id_verified.clone()]);

        let id_rejected = ProofId {
            backend: backend.into(),
            proof_hash: [0x22; 32],
        };
        let rec_rejected = ProofRecord {
            id: id_rejected.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Rejected,
            verified_at_height: Some(43),
            bridge: None,
        };
        stx.world.proofs.insert(id_rejected.clone(), rec_rejected);
        stx.apply();
        block.commit().expect("commit proof registry snapshot");

        let filters = ProofFilters {
            backend: Some(backend),
            status: Some(ProofStatus::Verified),
            bridge_only: false,
            pinned_only: false,
            bridge_min_range_start: None,
            bridge_max_range_end: None,
            has_tag: Some(*b"PROF"),
            min_height: None,
            max_height: None,
        };
        let params = ProofListParams {
            filters,
            descending: false,
            offset: None,
            limit: None,
        };
        let rows = list_proofs(&state, &params);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id_verified);
        assert_eq!(rows[0].record.status, ProofStatus::Verified);

        let total = count_proofs(&state, &params.filters);
        assert_eq!(total, 1);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn list_filters_respect_height_ranges() {
        let state = blank_state();
        let backend = "halo2/ipa";
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        // Proof with verified height 10
        let id_early = ProofId {
            backend: backend.into(),
            proof_hash: [0xAA; 32],
        };
        let rec_early = ProofRecord {
            id: id_early.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(10),
            bridge: None,
        };
        stx.world.proofs.insert(id_early.clone(), rec_early);

        // Proof with verified height 25
        let id_late = ProofId {
            backend: backend.into(),
            proof_hash: [0xBB; 32],
        };
        let rec_late = ProofRecord {
            id: id_late.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(25),
            bridge: None,
        };
        stx.world.proofs.insert(id_late.clone(), rec_late);

        // Submitted proof (no height) should only appear when no bounds are requested.
        let id_submitted = ProofId {
            backend: backend.into(),
            proof_hash: [0xCC; 32],
        };
        let rec_submitted = ProofRecord {
            id: id_submitted.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Submitted,
            verified_at_height: None,
            bridge: None,
        };
        stx.world.proofs.insert(id_submitted.clone(), rec_submitted);
        stx.apply();
        block.commit().expect("commit proof registry snapshot");

        // Filter proofs verified at or above height 20 -> should only include id_late
        let filters_min_only = ProofFilters {
            backend: Some(backend),
            status: Some(ProofStatus::Verified),
            bridge_only: false,
            pinned_only: false,
            bridge_min_range_start: None,
            bridge_max_range_end: None,
            has_tag: None,
            min_height: Some(20),
            max_height: None,
        };
        let params_min = ProofListParams {
            filters: filters_min_only,
            descending: false,
            offset: None,
            limit: None,
        };
        let rows_min = list_proofs(&state, &params_min);
        assert_eq!(rows_min.len(), 1);
        assert_eq!(rows_min[0].id, id_late);

        // Filter proofs verified at or below height 12 -> should only include id_early
        let filters_max_only = ProofFilters {
            backend: Some(backend),
            status: Some(ProofStatus::Verified),
            bridge_only: false,
            pinned_only: false,
            bridge_min_range_start: None,
            bridge_max_range_end: None,
            has_tag: None,
            min_height: None,
            max_height: Some(12),
        };
        let params_max = ProofListParams {
            filters: filters_max_only,
            descending: false,
            offset: None,
            limit: None,
        };
        let rows_max = list_proofs(&state, &params_max);
        assert_eq!(rows_max.len(), 1);
        assert_eq!(rows_max[0].id, id_early);

        // Narrow window should exclude submitted proof with no height.
        let filters_window = ProofFilters {
            backend: Some(backend),
            status: None,
            bridge_only: false,
            pinned_only: false,
            bridge_min_range_start: None,
            bridge_max_range_end: None,
            has_tag: None,
            min_height: Some(5),
            max_height: Some(15),
        };
        let params_window = ProofListParams {
            filters: filters_window,
            descending: false,
            offset: None,
            limit: None,
        };
        let rows_window = list_proofs(&state, &params_window);
        assert_eq!(rows_window.len(), 1);
        assert_eq!(rows_window[0].id, id_early);

        // Count helper should reflect the same filtering.
        let count = count_proofs(
            &state,
            &ProofFilters {
                backend: Some(backend),
                status: Some(ProofStatus::Verified),
                bridge_only: false,
                pinned_only: false,
                bridge_min_range_start: None,
                bridge_max_range_end: None,
                has_tag: None,
                min_height: Some(0),
                max_height: Some(30),
            },
        );
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn bridge_filters_only_bridge_records() {
        use iroha_data_model::proof::{ProofBox, ProofStatus};

        let state = blank_state();
        let backend = "bridge/test";
        let header =
            iroha_data_model::block::BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();

        let payload = BridgeProofPayload::TransparentZk(BridgeTransparentProof {
            proof: ProofBox::new(backend.into(), vec![0xAA, 0xBB]),
            recursion_depth: Some(1),
        });

        // Unpinned bridge proof
        let bridge_proof = bridge_proof_record(
            BridgeProofRange {
                start_height: 1,
                end_height: 3,
            },
            false,
            payload.clone(),
            [0x11; 32],
            [0x10; 32],
            2,
        );
        let (id_bridge, rec_bridge) = bridge_record(backend, [0x01; 32], bridge_proof, 5);
        stx.world.proofs.insert(id_bridge.clone(), rec_bridge);

        // Pinned bridge proof with later range
        let pinned_proof = bridge_proof_record(
            BridgeProofRange {
                start_height: 20,
                end_height: 25,
            },
            true,
            payload,
            [0x22; 32],
            [0x11; 32],
            3,
        );
        let (id_pinned, rec_pinned) = bridge_record(backend, [0x02; 32], pinned_proof, 6);
        stx.world.proofs.insert(id_pinned.clone(), rec_pinned);

        // Non-bridge proof should be filtered out.
        let (plain_id, plain) =
            plain_record("halo2/ipa", [0x03; 32], ProofStatus::Verified, Some(7));
        stx.world.proofs.insert(plain_id, plain);
        stx.apply();
        block.commit().expect("commit bridge filter snapshot");

        let rows = list_for_filters(
            &state,
            ProofFilters {
                backend: None,
                status: Some(ProofStatus::Verified),
                bridge_only: true,
                pinned_only: false,
                bridge_min_range_start: None,
                bridge_max_range_end: None,
                has_tag: None,
                min_height: None,
                max_height: None,
            },
        );
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.record.bridge.is_some()));

        let pinned_rows = list_for_filters(
            &state,
            ProofFilters {
                backend: None,
                status: None,
                bridge_only: true,
                pinned_only: true,
                bridge_min_range_start: None,
                bridge_max_range_end: None,
                has_tag: None,
                min_height: None,
                max_height: None,
            },
        );
        assert_eq!(pinned_rows.len(), 1);
        assert_eq!(pinned_rows[0].id, id_pinned);

        let range_filtered = list_for_filters(
            &state,
            ProofFilters {
                backend: None,
                status: None,
                bridge_only: true,
                pinned_only: false,
                bridge_min_range_start: Some(10),
                bridge_max_range_end: Some(30),
                has_tag: None,
                min_height: None,
                max_height: None,
            },
        );
        assert_eq!(range_filtered.len(), 1);
        assert_eq!(range_filtered[0].id, id_pinned);
    }
}
