//! RS16 chunk commitment helpers for DA ingest.

use axum::http::StatusCode;
use iroha_data_model::da::{
    ingest::DaIngestRequest,
    manifest::{ChunkCommitment, ChunkRole},
    types::ChunkDigest,
};
use iroha_primitives::erasure::rs16 as erasure_rs16;
use sorafs_car::ChunkStore;

const MAX_MANIFEST_CHUNK_COMMITMENTS: usize = u32::MAX as usize;

pub(super) fn build_chunk_commitments(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
) -> Result<Vec<ChunkCommitment>, (StatusCode, String)> {
    build_chunk_commitments_with_parity_observer(
        request,
        chunk_store,
        canonical_payload,
        |_index, _symbols| Ok(()),
    )
}

pub(super) fn build_chunk_commitments_with_parity_observer<F>(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
    mut parity_observer: F,
) -> Result<Vec<ChunkCommitment>, (StatusCode, String)>
where
    F: FnMut(u32, &[u16]) -> Result<(), (StatusCode, String)>,
{
    let chunk_size = usize::try_from(request.chunk_size).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "chunk_size exceeds supported host size".into(),
        )
    })?;
    if chunk_size < 2 || chunk_size % 2 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size must be an even number of bytes for RS(16) parity".into(),
        ));
    }

    let data_shards = usize::from(request.erasure_profile.data_shards);
    let parity_shards = usize::from(request.erasure_profile.parity_shards);
    if data_shards == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile must include at least one data shard".to_string(),
        ));
    }

    let symbol_count = chunk_size / 2;
    let chunks = chunk_store.chunks();
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let stripes = chunks.len().div_ceil(data_shards);
    let row_parity = usize::from(request.erasure_profile.row_parity_stripes);
    let commitment_count =
        chunk_commitment_capacity_hint(chunks.len(), data_shards, parity_shards, row_parity)?;
    let mut commitments = Vec::with_capacity(commitment_count);
    let retain_row_parity_matrix = row_parity > 0;
    let mut stripe_symbols_matrix: Vec<Vec<Vec<u16>>> = if retain_row_parity_matrix {
        Vec::with_capacity(stripes)
    } else {
        Vec::new()
    };
    let mut hash_scratch = Vec::with_capacity(symbol_count.saturating_mul(2));
    let mut next_index: u32 = 0;

    for stripe in 0..stripes {
        let mut stripe_symbols = Vec::with_capacity(data_shards + parity_shards);
        for shard_idx in 0..data_shards {
            let chunk_idx = stripe * data_shards + shard_idx;
            if let Some(chunk) = chunks.get(chunk_idx) {
                let offset = usize::try_from(chunk.offset).map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} offset exceeds host limits"),
                    )
                })?;
                let length = usize::try_from(chunk.length).map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} length exceeds host limits"),
                    )
                })?;
                if length > chunk_size {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("chunk length {length} exceeds configured chunk_size {chunk_size}"),
                    ));
                }
                let end = offset.checked_add(length).ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} offset+length overflow"),
                    )
                })?;
                if end > canonical_payload.len() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("chunk {chunk_idx} extends past canonical payload"),
                    ));
                }

                let symbols =
                    erasure_rs16::symbols_from_chunk(symbol_count, &canonical_payload[offset..end]);
                stripe_symbols.push(symbols.clone());

                let index = allocate_chunk_index(&mut next_index)?;
                let stripe_id = manifest_u32_index(stripe, "manifest stripe id")?;
                commitments.push(ChunkCommitment::new_with_role(
                    index,
                    chunk.offset,
                    chunk.length,
                    ChunkDigest::new(chunk.blake3),
                    ChunkRole::Data,
                    stripe_id,
                ));
            } else {
                stripe_symbols.push(vec![0u16; symbol_count]);
            }
        }

        if parity_shards == 0 {
            if retain_row_parity_matrix {
                stripe_symbols_matrix.push(stripe_symbols);
            }
            continue;
        }

        let parity_symbols: Vec<Vec<u16>> =
            erasure_rs16::encode_parity(&stripe_symbols, parity_shards).map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to compute parity shards".into(),
                )
            })?;

        for (parity_idx, symbols) in parity_symbols.iter().enumerate() {
            let digest = digest_symbols_le(symbols, &mut hash_scratch);
            let offset = erasure_rs16::parity_offset(
                request.total_size,
                stripe,
                parity_idx,
                parity_shards,
                request.chunk_size,
            )
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "parity chunk offset exceeded supported size".into(),
                )
            })?;

            let index = allocate_chunk_index(&mut next_index)?;
            parity_observer(index, symbols)?;
            let stripe_id = manifest_u32_index(stripe, "manifest stripe id")?;
            commitments.push(ChunkCommitment::new_with_role(
                index,
                offset,
                request.chunk_size,
                ChunkDigest::new(digest),
                ChunkRole::GlobalParity,
                stripe_id,
            ));
            stripe_symbols.push(symbols.clone());
        }

        if retain_row_parity_matrix {
            stripe_symbols_matrix.push(stripe_symbols);
        }
    }

    if row_parity > 0 {
        let column_count = data_shards + parity_shards;
        let global_parity_bytes = stripes
            .checked_mul(parity_shards)
            .and_then(|count| count.checked_mul(chunk_size))
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "stripe parity base offset exceeded supported size".into(),
                )
            })?;
        let base_offset = request
            .total_size
            .checked_add(global_parity_bytes)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "stripe parity base offset exceeded supported size".into(),
                )
            })?;
        for column in 0..column_count {
            // Collect the column symbols across stripes.
            let mut column_symbols = Vec::with_capacity(stripes);
            for stripe in 0..stripes {
                let stripe_row = stripe_symbols_matrix
                    .get(stripe)
                    .and_then(|row| row.get(column))
                    .cloned()
                    .unwrap_or_else(|| vec![0u16; symbol_count]);
                column_symbols.push(stripe_row);
            }

            let parity_cols: Vec<Vec<u16>> =
                erasure_rs16::encode_parity(&column_symbols, row_parity).map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to compute row-parity stripes".into(),
                    )
                })?;

            for (row_parity_idx, symbols) in parity_cols.iter().enumerate() {
                let digest = digest_symbols_le(symbols, &mut hash_scratch);

                let row_parity_chunk_index = row_parity_idx
                    .checked_mul(column_count)
                    .and_then(|base| base.checked_add(column))
                    .and_then(|index| u64::try_from(index).ok())
                    .ok_or_else(|| {
                        (
                            StatusCode::BAD_REQUEST,
                            "stripe parity chunk offset exceeded supported size".into(),
                        )
                    })?;
                let row_parity_bytes = row_parity_chunk_index
                    .checked_mul(u64::from(request.chunk_size))
                    .ok_or_else(|| {
                        (
                            StatusCode::BAD_REQUEST,
                            "stripe parity chunk offset exceeded supported size".into(),
                        )
                    })?;
                let offset = base_offset.checked_add(row_parity_bytes).ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        "stripe parity chunk offset exceeded supported size".into(),
                    )
                })?;
                let index = allocate_chunk_index(&mut next_index)?;
                parity_observer(index, symbols)?;
                let column_id = manifest_u32_index(column, "manifest stripe parity column id")?;
                commitments.push(ChunkCommitment::new_with_role(
                    index,
                    offset,
                    request.chunk_size,
                    ChunkDigest::new(digest),
                    ChunkRole::StripeParity,
                    column_id,
                ));
            }
        }
    }

    Ok(commitments)
}

fn digest_symbols_le(symbols: &[u16], scratch: &mut Vec<u8>) -> [u8; 32] {
    scratch.clear();
    scratch.reserve(symbols.len().saturating_mul(2));
    for symbol in symbols {
        scratch.extend_from_slice(&symbol.to_le_bytes());
    }
    *blake3::hash(scratch.as_slice()).as_bytes()
}

fn manifest_u32_index(value: usize, label: &str) -> Result<u32, (StatusCode, String)> {
    u32::try_from(value).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            format!("{label} exceeds supported u32 space"),
        )
    })
}

fn chunk_commitment_capacity_hint(
    data_chunk_count: usize,
    data_shards: usize,
    parity_shards: usize,
    row_parity: usize,
) -> Result<usize, (StatusCode, String)> {
    if data_chunk_count == 0 {
        return Ok(0);
    }
    if data_shards == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile must include at least one data shard".into(),
        ));
    }

    let stripes = data_chunk_count.div_ceil(data_shards);
    let column_count = data_shards.checked_add(parity_shards).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "manifest commitment count exceeds supported size".into(),
        )
    })?;
    let global_parity = stripes.checked_mul(parity_shards).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "manifest commitment count exceeds supported size".into(),
        )
    })?;
    let row_parity_chunks = row_parity.checked_mul(column_count).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "manifest commitment count exceeds supported size".into(),
        )
    })?;
    let total = data_chunk_count
        .checked_add(global_parity)
        .and_then(|count| count.checked_add(row_parity_chunks))
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "manifest commitment count exceeds supported size".into(),
            )
        })?;
    if total > MAX_MANIFEST_CHUNK_COMMITMENTS {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest would exceed supported chunk index space".into(),
        ));
    }
    Ok(total)
}

fn allocate_chunk_index(counter: &mut u32) -> Result<u32, (StatusCode, String)> {
    let idx = *counter;
    *counter = counter.checked_add(1).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "manifest would exceed supported chunk index space".into(),
        )
    })?;
    Ok(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_chunk_index_rejects_overflow_without_wrapping() {
        let mut counter = u32::MAX;

        let err = allocate_chunk_index(&mut counter).expect_err("exhausted index must reject");

        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(
            err.1.contains("chunk index space"),
            "unexpected error: {}",
            err.1
        );
        assert_eq!(counter, u32::MAX);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn manifest_u32_index_rejects_overflow_without_saturating() {
        let overflow = u32::MAX as usize + 1;

        let err = manifest_u32_index(overflow, "manifest stripe id")
            .expect_err("overflowed stripe id must reject");

        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(
            err.1
                .contains("manifest stripe id exceeds supported u32 space"),
            "unexpected error: {}",
            err.1
        );
    }

    #[test]
    fn chunk_commitment_capacity_hint_counts_row_parity_once_per_column() {
        let capacity =
            chunk_commitment_capacity_hint(5, 2, 1, 2).expect("capacity math should fit");

        assert_eq!(
            capacity, 14,
            "5 data chunks + 3 global parity chunks + 2 row parity stripes across 3 columns"
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn chunk_commitment_capacity_hint_rejects_index_space_overflow_before_allocation() {
        let data_chunks = (u32::MAX as usize / 2) + 1;

        let err = chunk_commitment_capacity_hint(data_chunks, 1, 1, 0)
            .expect_err("commitment count over u32 index space must reject");

        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(
            err.1.contains("chunk index space"),
            "unexpected error: {}",
            err.1
        );
    }
}
