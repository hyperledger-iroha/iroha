//! Exact canonical signed requests become submit-eligible only after journal fsync.

use super::*;

pub(super) const MAX_SIGNED_REQUEST_BYTES: usize = 1024 * 1024;
const CHUNK_BYTES: usize = 4096;
const ENCODING: &str = "norito.canonical.signed_transaction.v1";

/// One immutable canonical frame bound to the actual SDK transport transaction.
pub(super) struct SignedRequest {
    index: usize,
    plan: Planned,
    hash: TransactionHash,
    bytes: Vec<u8>,
    digest: String,
}

/// Transport bytes are held separately until the writer returns durable authority.
pub(super) struct Captured {
    payload: PreparedTransactionPayload,
    request: SignedRequest,
}

/// This private, non-cloneable receipt is created only after a successful file sync.
pub(super) struct DurableReceipt {
    index: usize,
    hash: TransactionHash,
}

/// The production backend cannot submit a transport payload without its receipt.
pub(super) struct DurablyPrepared {
    payload: PreparedTransactionPayload,
    receipt: DurableReceipt,
}

pub(super) fn capture(
    transaction: SignedTransaction,
    plan: Planned,
    warmup_count: usize,
) -> Result<Captured> {
    let sequence_index = plan
        .sequence
        .checked_sub(1)
        .ok_or_else(|| eyre!("signed request sequence must be positive"))?;
    let index = match plan.cohort {
        Cohort::Warmup if plan.sequence <= warmup_count => sequence_index,
        Cohort::Warmup => bail!("signed warmup request exceeds its planned cohort"),
        Cohort::Measurement => warmup_count
            .checked_add(sequence_index)
            .ok_or_else(|| eyre!("signed request index overflow"))?,
    };
    if warmup_count > MAX_ROWS
        || index >= MAX_ROWS
        || plan.account_index >= MAX_ACCOUNTS
        || plan.logical_id.len() != 64
        || !plan
            .logical_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("signed request plan identity is outside its fixed bounds");
    }
    // Count the real canonical serialization, never an untrusted size hint, before
    // allocating the retained frame. No alternate or version-sniffed format exists.
    let length = norito::canonical_frame_len(&transaction)?;
    if length == 0 || length > MAX_SIGNED_REQUEST_BYTES {
        bail!("signed request exceeds its fixed canonical frame bound");
    }
    let bytes = norito::encode_canonical(&transaction)?;
    if bytes.len() != length {
        bail!("signed request canonical serialization changed after admission");
    }
    let hash = transaction.hash();
    let payload = PreparedTransactionPayload::from_transaction(&transaction);
    if payload.hash() != hash {
        bail!("signed request transport changed the canonical transaction identity");
    }
    let digest = hex::encode(Sha256::digest(&bytes));
    Ok(Captured {
        payload,
        request: SignedRequest {
            index,
            plan,
            hash,
            bytes,
            digest,
        },
    })
}

impl Captured {
    pub(super) async fn retain(self, journal: &JournalSender) -> Result<DurablyPrepared> {
        let expected_index = self.request.index;
        let expected_hash = self.request.hash;
        let receipt = journal.retain_signed_request(self.request).await?;
        if receipt.index != expected_index
            || receipt.hash != expected_hash
            || receipt.hash != self.payload.hash()
        {
            bail!("signed request acknowledgment changed its exact identity");
        }
        Ok(DurablyPrepared {
            payload: self.payload,
            receipt,
        })
    }
}

impl DurablyPrepared {
    pub(super) fn hash(&self) -> TransactionHash {
        self.receipt.hash
    }

    pub(super) fn into_transport(
        self,
        expected: TransactionHash,
    ) -> Result<PreparedTransactionPayload> {
        if self.receipt.hash != expected || self.payload.hash() != expected {
            bail!("submission does not own the retained signed request");
        }
        Ok(self.payload)
    }
}

/// Small seam for failure injection around the actual writer's flush and fsync.
pub(super) trait DurableWriter: Write {
    fn sync_request(&mut self) -> std::io::Result<()>;
}

impl DurableWriter for BufWriter<File> {
    fn sync_request(&mut self) -> std::io::Result<()> {
        self.flush()?;
        self.get_ref().sync_all()
    }
}

impl SignedRequest {
    pub(super) fn index(&self) -> usize {
        self.index
    }

    fn encoded_records(&self, mut consume: impl FnMut(&[u8]) -> Result<()>) -> Result<()> {
        if self.bytes.is_empty() || self.bytes.len() > MAX_SIGNED_REQUEST_BYTES {
            bail!("signed request exceeds its fixed canonical frame bound");
        }
        let chunk_count = self.bytes.len().div_ceil(CHUNK_BYTES);
        let mut record = |value: Value| -> Result<()> {
            let mut bytes = json::to_vec(&value)?;
            if bytes.len() > MAX_EVENT_BYTES {
                bail!("signed request event exceeds bounded record size");
            }
            bytes.push(b'\n');
            consume(&bytes)
        };
        record(
            norito::json!({"event": "signed_request_begin", "index": (self.index),
            "plan": (self.plan.value()), "hash": (self.hash.to_string()), "encoding": ENCODING,
            "byte_length": (self.bytes.len()), "canonical_sha256": (self.digest),
            "chunk_count": chunk_count}),
        )?;
        for (chunk_index, chunk) in self.bytes.chunks(CHUNK_BYTES).enumerate() {
            record(
                norito::json!({"event": "signed_request_chunk", "index": (self.index),
                "chunk_index": chunk_index, "offset": (chunk_index * CHUNK_BYTES),
                "bytes_hex": (hex::encode(chunk))}),
            )?;
        }
        record(
            norito::json!({"event": "signed_request_retained", "index": (self.index),
            "hash": (self.hash.to_string()), "byte_length": (self.bytes.len()),
            "canonical_sha256": (self.digest), "chunk_count": chunk_count}),
        )
    }

    pub(super) fn persist(
        self,
        writer: &mut impl DurableWriter,
        written: &mut usize,
        limit: usize,
    ) -> Result<DurableReceipt> {
        // Admit all hex expansion, field syntax and newlines before the first
        // record is written. Both passes use this same immutable request.
        let mut required = 0_usize;
        self.encoded_records(|bytes| {
            required = required
                .checked_add(bytes.len())
                .ok_or_else(|| eyre!("signed request journal size overflow"))?;
            Ok(())
        })?;
        if limit == 0
            || limit > MAX_FILE_BYTES
            || written
                .checked_add(required)
                .is_none_or(|total| total > limit)
        {
            bail!("signed request exceeds the remaining admitted journal allocation");
        }
        self.encoded_records(|bytes| bounded_write(writer, written, bytes, limit))?;
        writer.sync_request()?;
        Ok(DurableReceipt {
            index: self.index,
            hash: self.hash,
        })
    }
}

#[cfg(test)]
mod tests;
