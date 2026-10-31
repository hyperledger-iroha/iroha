//! Authenticated Cargo witness and anonymous kernel-sealed recursive execution image.
//!
//! Cargo 1.93.1 prefers a current executable or argv0 whose stem is `cargo` over
//! inherited CARGO. Only the anonymous image executes: its unrenameable memfd name
//! and the fixed non-cargo initial argv0 keep both pathname preference branches
//! false. The exact-byte filesystem batch remains a retained authentication witness;
//! its name can never select a recursive executable. The installed source stays held.

use std::{
    io::{self, Read, Seek as _, Write as _},
    sync::Arc,
};

use iroha_fs::{
    FileSnapshot, FlatBatchEntry, FlatBatchKind, FlatBatchSealStep, PrivateDirectory,
    RetainedBuildInput, SealedFlatBatch, SealedToolImage,
};
use sha2::{Digest as _, Sha256};

use super::invalid;

const IMAGE: &str = "retained-cargo";
const MAXIMUM: usize = 2 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    CopyChunk(u64),
    Copied,
    Sealing(FlatBatchSealStep),
    Authenticated,
    MemorySealed,
}

#[derive(Debug)]
pub(super) struct Image {
    pub(super) batch: Arc<SealedFlatBatch<1>>,
    pub(super) executable: Arc<SealedToolImage<1>>,
}

fn check_source(original: &RetainedBuildInput, snapshot: FileSnapshot) -> io::Result<()> {
    original.revalidate()?;
    if original.snapshot()? != snapshot {
        return Err(invalid(
            "Cargo source original changed during image publication",
        ));
    }
    Ok(())
}

pub(super) fn publish(
    original: &mut RetainedBuildInput,
    snapshot: FileSnapshot,
    expected: &super::super::provenance::Tool,
    parent: &PrivateDirectory,
    name: &str,
    mut observe: impl FnMut(Step) -> io::Result<()>,
) -> io::Result<Arc<Image>> {
    check_source(original, snapshot)?;
    parent.revalidate()?;
    let result = (|| {
        if expected.bytes == 0
            || expected.bytes > MAXIMUM as u64
            || original.len()? != expected.bytes
        {
            return Err(invalid("Cargo source extent differs from admitted tool"));
        }
        let mut pending = parent.create_flat_batch(
            name,
            [FlatBatchEntry::new(
                IMAGE,
                FlatBatchKind::Executable,
                MAXIMUM,
            )?],
        )?;
        // A newly created object, not another name for the installed executable.
        let source = snapshot.local_parts();
        let selected = pending.entry_identity(0)?.local_parts();
        if source.0 == selected.0 && source.1 == (selected.1, selected.2) {
            return Err(invalid("Cargo image aliases its source original"));
        }
        original.rewind()?;
        let mut copied = 0_u64;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 16 * 1024];
        {
            let mut writer = pending.writer(0)?;
            loop {
                let count = original.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                copied = copied
                    .checked_add(count as u64)
                    .filter(|value| *value <= expected.bytes)
                    .ok_or_else(|| invalid("Cargo source exceeded admitted extent"))?;
                writer.write_all(&buffer[..count])?;
                hash.update(&buffer[..count]);
                observe(Step::CopyChunk(copied))?;
            }
            writer.flush()?;
        }
        check_source(original, snapshot)?;
        if copied != expected.bytes || hex::encode(hash.finalize()) != expected.sha256 {
            return Err(invalid("Cargo copied bytes differ from admitted source"));
        }
        observe(Step::Copied)?;
        let batch = pending.seal_observed(|step| observe(Step::Sealing(step)))?;
        // Compare the sealed original itself to the still-held installed source.
        // Hash equality alone does not substitute for this complete byte readback.
        original.rewind()?;
        let mut sealed = batch.reader(0)?;
        let mut other = [0_u8; 16 * 1024];
        let mut compared = 0_u64;
        loop {
            let count = original.read(&mut buffer)?;
            if count == 0 {
                if sealed.read(&mut other[..1])? != 0 {
                    return Err(invalid("sealed Cargo image has an extra byte"));
                }
                break;
            }
            sealed.read_exact(&mut other[..count])?;
            if buffer[..count] != other[..count] {
                return Err(invalid("sealed Cargo image differs from source bytes"));
            }
            compared = compared
                .checked_add(count as u64)
                .filter(|value| *value <= expected.bytes)
                .ok_or_else(|| invalid("Cargo source exceeded readback extent"))?;
        }
        if compared != expected.bytes {
            return Err(invalid("Cargo source ended before complete readback"));
        }
        original.rewind()?;
        observe(Step::Authenticated)?;
        batch.revalidate()?;
        let batch = Arc::new(batch);
        let executable = batch.sealed_tool_image(0)?;
        observe(Step::MemorySealed)?;
        executable.revalidate()?;
        Ok(Arc::new(Image { batch, executable }))
    })();
    // Both final observations run even after copy/observer/sealing errors. Keep
    // the initiating error; partial claimed namespaces are never reopened/cleaned.
    let source_check = check_source(original, snapshot);
    let parent_check = parent.revalidate();
    let batch = result?;
    source_check?;
    parent_check?;
    Ok(batch)
}

#[cfg(test)]
#[path = "cargo_image_tests.rs"]
mod tests;
