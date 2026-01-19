//! RS16 chunk commitment helpers for DA ingest.

use axum::http::StatusCode;
use blake3::Hasher as Blake3Hasher;
use iroha_data_model::da::{
    ingest::DaIngestRequest,
    manifest::{ChunkCommitment, ChunkRole},
    types::ChunkDigest,
};
use iroha_primitives::erasure::rs16 as erasure_rs16;
use sorafs_car::ChunkStore;

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
    let mut commitments = Vec::with_capacity(
        chunks.len()
            + stripes.saturating_mul(parity_shards)
            + stripes
                .saturating_mul(usize::from(request.erasure_profile.row_parity_stripes))
                .saturating_mul(data_shards + parity_shards),
    );
    // For column parity, retain the symbol vectors per stripe/column.
    let mut stripe_symbols_matrix: Vec<Vec<Vec<u16>>> = Vec::with_capacity(stripes);
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
                let stripe_id = u32::try_from(stripe).unwrap_or(u32::MAX);
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
            stripe_symbols_matrix.push(stripe_symbols);
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
            let mut hasher = Blake3Hasher::new();
            for symbol in symbols {
                hasher.update(&symbol.to_le_bytes());
            }
            let digest = hasher.finalize();
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
            let stripe_id = u32::try_from(stripe).unwrap_or(u32::MAX);
            commitments.push(ChunkCommitment::new_with_role(
                index,
                offset,
                request.chunk_size,
                ChunkDigest::new(*digest.as_bytes()),
                ChunkRole::GlobalParity,
                stripe_id,
            ));
            stripe_symbols.push(symbols.clone());
        }

        stripe_symbols_matrix.push(stripe_symbols);
    }

    let row_parity = usize::from(request.erasure_profile.row_parity_stripes);
    if row_parity > 0 {
        let column_count = data_shards + parity_shards;
        let base_offset = request.total_size.saturating_add(
            stripes
                .saturating_mul(parity_shards)
                .saturating_mul(chunk_size) as u64,
        );
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
                let mut hasher = Blake3Hasher::new();
                for symbol in symbols {
                    hasher.update(&symbol.to_le_bytes());
                }
                let digest = hasher.finalize();

                let offset = base_offset.saturating_add(
                    ((row_parity_idx * column_count + column) as u64)
                        .saturating_mul(request.chunk_size as u64),
                );
                let index = allocate_chunk_index(&mut next_index)?;
                parity_observer(index, symbols)?;
                let column_id = u32::try_from(column).unwrap_or(u32::MAX);
                commitments.push(ChunkCommitment::new_with_role(
                    index,
                    offset,
                    request.chunk_size,
                    ChunkDigest::new(*digest.as_bytes()),
                    ChunkRole::StripeParity,
                    column_id,
                ));
            }
        }
    }

    Ok(commitments)
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
