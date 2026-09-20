//! Bounded one-shot terminal handoff of exact native load outputs.
use super::output::{RetainedLoadFile, canonical_inputs::RawFileIdentity};
use super::*;

pub(super) const MAX_REPLY_BYTES: usize = 4096;

/// Hash bytes accepted by the actual writer, independently of later readback.
pub(super) struct DigestWriter<W> {
    inner: W,
    digest: Sha256,
    length: u64,
}
impl<W: Write> DigestWriter<W> {
    pub(super) fn new(inner: W) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
            length: 0,
        }
    }
    pub(super) fn finish(mut self) -> Result<(W, RawFileIdentity)> {
        self.flush()?;
        Ok((
            self.inner,
            RawFileIdentity {
                raw_sha256: self.digest.finalize().into(),
                byte_length: self.length,
            },
        ))
    }
}
impl<W: Write> Write for DigestWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let count = self.inner.write(bytes)?;
        self.length = self
            .length
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("load byte count overflow"))?;
        self.digest.update(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
impl DigestWriter<BufWriter<File>> {
    pub(super) fn get_ref(&self) -> &File {
        self.inner.get_ref()
    }
}
impl signed_request::DurableWriter for DigestWriter<BufWriter<File>> {
    fn sync_request(&mut self) -> std::io::Result<()> {
        self.flush()?;
        self.get_ref().sync_all()?;
        Ok(())
    }
}

pub(super) fn validate_invocation(invocation: &str) -> Result<()> {
    if invocation.len() != 64
        || !invocation
            .bytes()
            .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))
        || invocation == "0".repeat(64)
    {
        bail!("load requires a fresh nonzero lowercase invocation identity");
    }
    Ok(())
}

/// Consume both original output owners only after the real reply writer flushes.
pub(super) fn emit(
    args: &Args,
    scheduled: usize,
    journal: RetainedLoadFile,
    trace: RetainedLoadFile,
    writer: &mut impl Write,
) -> Result<()> {
    validate_invocation(&args.invocation_id)?;
    let (original_journal, original_trace) = journal.pair_identity(&trace)?;
    let mut raw = norito::json::to_vec(&norito::json!({
        "version": 1, "operation": "transaction_load", "invocation_id": (args.invocation_id),
        "pair_index": (args.pair_index), "variant": (args.variant.text()), "seed": (args.seed),
        "resource_budget_sha256": (args.resource.resource_budget_sha256), "scheduled_requests": scheduled,
        "collector_journal_sha256": (hex::encode(original_journal.raw_sha256)),
        "collector_journal_bytes": (original_journal.byte_length),
        "trace_sha256": (hex::encode(original_trace.raw_sha256)), "trace_bytes": (original_trace.byte_length)
    }))?;
    raw.push(b'\n');
    if raw.len() > MAX_REPLY_BYTES {
        bail!("load terminal receipt exceeds fixed bound");
    }
    let verify = || -> Result<()> {
        if journal.pair_identity(&trace)? != (original_journal, original_trace) {
            bail!("load outputs changed during terminal receipt publication");
        }
        Ok(())
    };
    verify()?;
    writer.write_all(&raw)?;
    verify()?;
    writer.flush()?;
    verify()
}
