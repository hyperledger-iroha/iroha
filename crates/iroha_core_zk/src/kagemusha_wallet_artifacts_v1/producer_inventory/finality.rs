//! Bind a complete verifier-only finality graph to signed inventory and native genesis.

use super::*;
use crate::kagemusha_wallet_finality_v1::{HistoryAnchorError, derive_history_anchor};
use iroha_data_model::sumeragi_finality::SumeragiFinalityVerifier;
use iroha_kagemusha_proof::finality::{
    catalog::{CompileError, ReceiptVerifier, VerifierBlobSource, VerifierLimits, qualify_receipt},
    continuity::{SourceVerifier, producer::Error as SourceError},
    history::HistoryAnchor,
    native::Parameters,
};

/// Failure to bind or reconstruct the exact independently installed receipt source.
#[derive(Debug, thiserror::Error)]
pub enum FinalityQualificationErrorV1 {
    /// Reinstallable verifier originals could not be read from storage.
    #[error(transparent)]
    Original(#[from] Error),
    /// The native original signed root cannot supply the global history policy.
    #[error(transparent)]
    Anchor(#[from] HistoryAnchorError),
    /// A signed catalog selected a different network, root, epoch or parameter set.
    #[error("producer inventory does not match independently selected native genesis")]
    AnchorMismatch,
    /// An exact compiled graph node or bounded original failed qualification.
    #[error(transparent)]
    Source(#[from] CompileError),
}

/// Complete compiled receipt source bound to this one authenticated installation.
/// It provides no proving key, wallet-open grant, accepted receipt or server prover.
pub struct QualifiedReceiptSourceV1 {
    receipt: ReceiptVerifier,
    scheme_id: [u8; 32],
    manifest_digest: [u8; 32],
}
impl QualifiedReceiptSourceV1 {
    /// Exact authenticated wallet installation that selected the complete graph.
    pub const fn installation(&self) -> ([u8; 32], [u8; 32]) {
        (self.scheme_id, self.manifest_digest)
    }

    /// Complete native genesis policy derived from the independently selected root.
    pub const fn anchor(&self) -> &HistoryAnchor {
        self.receipt.anchor()
    }

    /// Exact complete receipt verifier for the compiled Load source constructor.
    pub const fn source(&self) -> &SourceVerifier {
        self.receipt.source()
    }

    /// Authenticate the exact terminal history state through the complete verifier-only graph.
    /// This grants no wallet producer and authenticates no earlier result or caller checkpoint.
    /// # Errors
    /// Wrong state, endpoints or proof/claims, or cooperative cancellation.
    pub fn restore_history(
        &self,
        state: &iroha_kagemusha_proof::finality::history::HistoryState,
        evidence: iroha_kagemusha_proof::finality::continuity::SourceNodeEvidence,
        budget: iroha_pasta::msm::MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<iroha_kagemusha_proof::finality::native::HistoryPrefix, SourceError> {
        self.receipt
            .restore_history(state, evidence, budget, cancellation)
    }

    /// Fully verify the exact receipt statement and both original carried claims.
    /// The operation owner must separately bind the original receipt transcript,
    /// wallet terms and this installation to its own authenticated state.
    /// # Errors
    /// Changed receipt/anchor/endpoints, malformed proof or failed claim decision.
    pub fn verify_receipt_evidence(
        &self,
        receipt_digest: Fp,
        evidence: &iroha_kagemusha_proof::finality::continuity::SourceNodeEvidence,
        budget: iroha_pasta::msm::MemoryBudget,
    ) -> Result<(), SourceError> {
        self.receipt
            .verify_receipt_evidence(receipt_digest, evidence, budget)
    }
}

struct Reader<'a> {
    source: &'a mut dyn OriginalSourceV1,
    unavailable: std::cell::Cell<bool>,
}
struct TrackedRead<'a> {
    reader: Box<dyn Read + 'a>,
    unavailable: &'a std::cell::Cell<bool>,
}
impl Read for TrackedRead<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buffer).inspect_err(|error| {
            // A retained original marks failed identity/ancestry checks with a
            // private typed cause. This is an integrity refusal, even when the
            // underlying native operation reports NotFound or PermissionDenied.
            if error.kind() != std::io::ErrorKind::Interrupted
                && !error
                    .get_ref()
                    .is_some_and(|cause| cause.is::<OriginalCustodyFailure>())
            {
                self.unavailable.set(true);
            }
        })
    }
}
impl VerifierBlobSource for Reader<'_> {
    fn open(&mut self, hash: &[u8; 32]) -> Result<Box<dyn Read + '_>, SourceError> {
        match self.source.open(*hash) {
            Ok(reader) => Ok(Box::new(TrackedRead {
                reader,
                unavailable: &self.unavailable,
            })),
            Err(error) => {
                if error == Error::Unavailable {
                    self.unavailable.set(true);
                }
                Err(SourceError::Artifact)
            }
        }
    }
}

impl FinalityV1 {
    fn require_anchor(
        &self,
        verifier: &SumeragiFinalityVerifier,
    ) -> Result<HistoryAnchor, FinalityQualificationErrorV1> {
        let anchor = derive_history_anchor(verifier)?;
        if anchor
            != (HistoryAnchor {
                network: self.network,
                instance: self.instance,
                initial_context: self.initial_context,
                initial_epoch: self.initial_epoch,
                parameters: self.parameters,
            })
        {
            return Err(FinalityQualificationErrorV1::AnchorMismatch);
        }
        Ok(anchor)
    }
}

impl AuthenticatedProducerInventoryV1 {
    /// Reconstruct every finality source and wrapper from compiled owners after
    /// binding all anchor fields to the native verifier's original signed root.
    /// The caller must independently select/authenticate that verifier's genesis
    /// and chain label; the signed inventory cannot supply a replacement root.
    /// Only descriptor/VK originals are read. The returned purpose-limited owner
    /// cannot upgrade this installation to wallet or server proving readiness.
    /// # Errors
    /// Wrong native root/parameters, incomplete graph, source/key mismatch or
    /// unavailable, malformed, extended, truncated or oversized verifier originals.
    pub fn qualify_finality(
        &self,
        verifier: &SumeragiFinalityVerifier,
        originals: &mut dyn OriginalSourceV1,
        params: Parameters,
        limits: VerifierLimits,
    ) -> Result<QualifiedReceiptSourceV1, FinalityQualificationErrorV1> {
        let anchor = self.inventory.finality.require_anchor(verifier)?;
        let mut reader = Reader {
            source: originals,
            unavailable: std::cell::Cell::new(false),
        };
        let receipt = qualify_receipt(
            anchor,
            &self.inventory.finality.originals,
            &mut reader,
            params,
            limits,
        );
        if reader.unavailable.get() {
            return Err(Error::Unavailable.into());
        }
        let receipt = receipt?;
        Ok(QualifiedReceiptSourceV1 {
            receipt,
            scheme_id: self.scheme_id,
            manifest_digest: self.manifest_digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

    #[test]
    fn metadata_io_failure_stays_unavailable_without_relabeling_bad_source() {
        struct Broken(bool);
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                if self.0 {
                    self.0 = false;
                    Err(std::io::ErrorKind::Interrupted.into())
                } else {
                    Err(std::io::ErrorKind::PermissionDenied.into())
                }
            }
        }
        let unavailable = std::cell::Cell::new(false);
        let mut reader = TrackedRead {
            reader: Box::new(Broken(true)),
            unavailable: &unavailable,
        };
        let mut buffer = [0];
        assert_eq!(
            reader.read(&mut buffer).unwrap_err().kind(),
            std::io::ErrorKind::Interrupted
        );
        assert!(!unavailable.get());
        assert_eq!(
            reader.read(&mut buffer).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(unavailable.get());

        struct Absent(Error);
        impl OriginalSourceV1 for Absent {
            fn open(&mut self, _: [u8; 32]) -> Result<Box<dyn Read + '_>, Error> {
                Err(self.0)
            }
        }
        for error in [Error::Unavailable, Error::Inventory, Error::Profile] {
            let mut source = Absent(error);
            let mut reader = Reader {
                source: &mut source,
                unavailable: std::cell::Cell::new(false),
            };
            assert!(reader.open(&[1; 32]).is_err());
            assert_eq!(reader.unavailable.get(), error == Error::Unavailable);
        }
    }

    #[test]
    fn metadata_late_custody_failure_never_becomes_reinstallable_unavailability() {
        struct Changed(std::io::ErrorKind);
        impl Read for Changed {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other(OriginalCustodyFailure(self.0.into())))
            }
        }
        for native_kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::InvalidData,
        ] {
            let unavailable = std::cell::Cell::new(false);
            let mut reader = TrackedRead {
                reader: Box::new(Changed(native_kind)),
                unavailable: &unavailable,
            };
            let error = reader.read(&mut [0]).unwrap_err();
            assert!(
                error
                    .get_ref()
                    .is_some_and(|cause| cause.is::<OriginalCustodyFailure>())
            );
            assert!(!unavailable.get());
        }
    }

    #[test]
    fn signed_metadata_cannot_replace_any_native_genesis_anchor_field() {
        let fixture = NativeFinalityFixture::new_with_explicit_parameters();
        let verifier = fixture.verifier();
        let anchor = derive_history_anchor(&verifier).unwrap();
        let finality = FinalityV1 {
            network: anchor.network,
            instance: anchor.instance,
            initial_context: anchor.initial_context,
            initial_epoch: anchor.initial_epoch,
            parameters: anchor.parameters,
            originals: Vec::new(),
        };
        assert_eq!(finality.require_anchor(&verifier).unwrap(), anchor);
        // Every byte/parameter remains independently fixed, not only a digest,
        // network name, first epoch or a current ledger configuration snapshot.
        for identity in 0..3 {
            for byte in 0..32 {
                let mut changed = finality.clone();
                match identity {
                    0 => changed.network[byte] ^= 1,
                    1 => changed.instance[byte] ^= 1,
                    _ => changed.initial_context[byte] ^= 1,
                }
                assert!(matches!(
                    changed.require_anchor(&verifier),
                    Err(FinalityQualificationErrorV1::AnchorMismatch)
                ));
            }
        }
        let mut changed = finality.clone();
        changed.initial_epoch += 1;
        assert!(changed.require_anchor(&verifier).is_err());
        for index in 0..6 {
            let mut changed = finality.clone();
            changed.parameters[index] ^= 1;
            assert!(changed.require_anchor(&verifier).is_err());
        }
    }
}
