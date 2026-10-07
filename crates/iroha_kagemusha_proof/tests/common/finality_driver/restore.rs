//! Read-only proof reuse from one independently pinned completed engineering capture.
//!
//! Catalog reconstruction derives verifying keys only. These tooling inputs do not
//! authenticate a release catalog, genesis policy or production monetary SchemeID.

use std::{collections::BTreeMap, io::Cursor};

use iroha_kagemusha_proof::finality::{
    catalog::{ArtifactRecord, ReceiptVerifier, VerifierBlobSource, VerifierLimits},
    continuity::checkpoint,
    receipt_finality::{CONTEXT_DOMAIN, PROGRAM_ID},
};
use iroha_pasta::poseidon::hash_with_domain;

use super::*;

const RECORDS: usize = 4096;
const INVENTORY_BYTES: usize = RECORDS * 2048 + 4096;
const VERIFIER_BYTES: usize = 5_usize << 30;

/// Independently retained identities of the completed producer run.
#[derive(Clone, Copy)]
pub struct CaptureIdentity {
    /// SHA-256 of the immutable producer executable, not this loader executable.
    pub producer: [u8; 32],
    /// SHA-256 of the producer's retained source manifest.
    pub sources: [u8; 32],
    /// SHA-256 of the exact original JSON native capture.
    pub fixture: [u8; 32],
    /// SHA-256 of the complete, retained canonical artifact inventory.
    pub inventory: [u8; 32],
}

/// Exact verifier-qualified receipt and capture metadata for subsequent component tests.
pub struct RestoredReceipt {
    /// Original native receipt, complete proof and source for a genuine Load fixture.
    pub finalized: FinalizedReceipt,
    /// Opaque complete compiled receipt source under the selected independent anchor.
    pub verifier: ReceiptVerifier,
    /// Number of bounded descriptor/VK reads; no server PK is requested.
    pub verifier_reads: usize,
}

/// Restore refusal; genuine checkpoint absence is returned as `Ok(None)` instead.
#[derive(Debug)]
pub enum RestoreError {
    /// Pinned capture identity, canonical metadata, finite bound or storage failure.
    Capture,
    /// Complete compiled-source qualification refused the original catalog.
    Qualification(catalog::CompileError),
    /// Original receipt proof, its exact endpoint binding or a carried claim failed.
    Proof(Error),
}
impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Capture => f.write_str("receipt capture identity or storage failed"),
            Self::Qualification(error) => write!(f, "receipt source qualification failed: {error}"),
            Self::Proof(error) => write!(f, "receipt proof restoration failed: {error}"),
        }
    }
}
impl std::error::Error for RestoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capture => None,
            Self::Qualification(error) => Some(error),
            Self::Proof(error) => Some(error),
        }
    }
}

fn capture_records(
    root: &Path,
    capture: &str,
    expected: CaptureIdentity,
) -> Result<Vec<ArtifactRecord>, Error> {
    if [
        expected.producer,
        expected.sources,
        expected.fixture,
        expected.inventory,
    ]
    .contains(&[0; 32])
        || capture.len() > 1 << 20
        || <[u8; 32]>::from(Sha256::digest(capture.as_bytes())) != expected.fixture
        || read_bounded(&root.join("binary.sha256"), 32)? != expected.producer
        || read_bounded(&root.join("fixture.json"), 1 << 20)? != capture.as_bytes()
    {
        return Err(Error::Artifact);
    }
    let provenance = read_bounded(&root.join("provenance.norito"), 8192)?;
    let provenance: SourceProvenance = norito::decode_canonical_with_limits(
        &provenance,
        norito::canonical_decode_limits(provenance.len()),
    )
    .map_err(|_| Error::Artifact)?;
    if provenance.source_manifest_sha256 != expected.sources {
        return Err(Error::Artifact);
    }
    let originals = root.join("originals");
    if !directory_exists(&originals)? {
        return Err(Error::Artifact);
    }
    let inventory = read_bounded(&originals.join("inventory.norito"), INVENTORY_BYTES)?;
    if <[u8; 32]>::from(Sha256::digest(&inventory)) != expected.inventory {
        return Err(Error::Artifact);
    }
    let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
        &inventory,
        norito::canonical_decode_limits(inventory.len()),
    )
    .map_err(|_| Error::Artifact)?;
    if records.is_empty() || records.len() > RECORDS {
        return Err(Error::Artifact);
    }
    Ok(records)
}

struct VerifierFiles {
    files: BTreeMap<[u8; 32], (PathBuf, usize)>,
    reads: usize,
}
impl VerifierFiles {
    fn new(root: &Path, records: &[ArtifactRecord]) -> Result<Self, Error> {
        if !directory_exists(root)? || records.is_empty() || records.len() > RECORDS {
            return Err(Error::Artifact);
        }
        let mut files = BTreeMap::new();
        let mut previous: Option<&[u8]> = None;
        let mut total = 0usize;
        for record in records {
            record.validate_identity()?;
            if previous.is_some_and(|name| name >= record.name.as_slice()) {
                return Err(Error::Artifact);
            }
            previous = Some(&record.name);
            let stem = hex_out(&Sha256::digest(&record.name));
            // Deliberately exclude the PK address even when its file is present.
            for (index, (suffix, cap)) in [("descriptor", 1 << 20), ("vk", 1 << 18)]
                .into_iter()
                .enumerate()
            {
                let length = usize::try_from(record.lengths[index]).map_err(|_| Error::Artifact)?;
                let digest = record.sha256[index];
                if length == 0 || length > cap || digest == [0; 32] {
                    return Err(Error::Artifact);
                }
                total = total
                    .checked_add(length)
                    .filter(|n| *n <= VERIFIER_BYTES)
                    .ok_or(Error::Artifact)?;
                if let Some((_, old_length)) = files.get(&digest) {
                    if *old_length != length {
                        return Err(Error::Artifact);
                    }
                } else {
                    files.insert(digest, (root.join(format!("{stem}.{suffix}")), length));
                }
            }
        }
        Ok(Self { files, reads: 0 })
    }
}
impl VerifierBlobSource for VerifierFiles {
    fn open(&mut self, digest: &[u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
        let (path, length) = self.files.get(digest).ok_or(Error::Artifact)?;
        let bytes = read_bounded(path, *length)?;
        if bytes.len() != *length || <[u8; 32]>::from(Sha256::digest(&bytes)) != *digest {
            return Err(Error::Artifact);
        }
        self.reads += 1;
        Ok(Box::new(Cursor::new(bytes)))
    }
}

/// Source-qualify and restore one completed, independently identified receipt capture.
///
/// `capture` is the exact independently retained native fixture; its original anchor
/// and receipt are never selected from the proof checkpoint. A missing checkpoint
/// remains absence, while malformed/unavailable data fails. The directory must already
/// exist and be exclusively available; this never reopens a streaming PK store.
/// # Errors
/// Changed pins, unavailable/noncanonical originals, foreign compiled catalog, invalid
/// proof/claims, substituted receipt/endpoints or active producer directory custody.
pub fn load_completed_receipt(
    root: &Path,
    capture: &str,
    expected: CaptureIdentity,
) -> Result<Option<RestoredReceipt>, RestoreError> {
    if !directory_exists(root).map_err(|_| RestoreError::Capture)? {
        return Err(RestoreError::Capture);
    }
    let _lock = RunLock::acquire(root).map_err(|_| RestoreError::Capture)?;
    let records = capture_records(root, capture, expected).map_err(|_| RestoreError::Capture)?;
    let fixture = fixture(capture).map_err(|_| RestoreError::Capture)?;
    let mut originals =
        VerifierFiles::new(&root.join("originals"), &records).map_err(|_| RestoreError::Capture)?;
    let vesta = PinnedParams::<Eq>::derive(16).map_err(|_| RestoreError::Capture)?;
    let verifier = catalog::qualify_receipt(
        fixture.anchor,
        &records,
        &mut originals,
        Parameters {
            pallas: PinnedParams::derive(16).map_err(|_| RestoreError::Capture)?,
            vesta: vesta.clone(),
        },
        VerifierLimits {
            maximum_artifacts: RECORDS,
            maximum_verifier_bytes: VERIFIER_BYTES,
            msm_budget: MemoryBudget::DEFAULT,
        },
    )
    .map_err(RestoreError::Qualification)?;
    let context = hash_with_domain(
        CONTEXT_DOMAIN,
        &[fixture.anchor.digest(), fixture.receipt_digest],
    );
    let endpoints = [
        Fp::from(PROGRAM_ID),
        context,
        Fp::ZERO,
        Fp::ONE,
        Fp::ZERO,
        context,
    ];
    let proof_root = root.join("proofs");
    if !directory_exists(&proof_root).map_err(|_| RestoreError::Capture)? {
        return Ok(None);
    }
    // This exact read uses no directory cleanup, counter scan or publication path.
    let mut checkpoints = Checkpoints {
        root: proof_root,
        entries: 0,
        bytes: 0,
    };
    let Some(evidence) = checkpoint::restore(
        &mut checkpoints,
        verifier.source(),
        endpoints,
        &vesta,
        MemoryBudget::DEFAULT,
    )
    .map_err(RestoreError::Proof)?
    else {
        return Ok(None);
    };
    verifier
        .verify_receipt_evidence(fixture.receipt_digest, &evidence, MemoryBudget::DEFAULT)
        .map_err(RestoreError::Proof)?;
    Ok(Some(RestoredReceipt {
        finalized: FinalizedReceipt {
            anchor: fixture.anchor,
            source: verifier.source().clone(),
            receipt: fixture.load.receipt,
            evidence,
        },
        verifier,
        verifier_reads: originals.reads,
    }))
}

#[cfg(test)]
#[path = "restore/tests.rs"]
mod tests;
