//! One immutable optional CUDA owner, authenticated before device or private work.
//!
//! A missing genuine approved bundle preserves CPU behavior. Source approval and
//! all exact retained inputs precede native discovery; no path/env trust or raw
//! public artifact constructor can bypass the existing device/kernel admission.

use crate::{
    cuda::policy::Kernel,
    cuda_build_policy::{
        CUDA_SOURCES, PtxRefusal, REVIEWED_CUDA_BUNDLE_PINS, ReviewedCudaBundlePins, cuda_stems,
        validate_ptx,
    },
    cuda_provenance::{BorrowedCudaReader, ProvenanceRefusal},
};
use iroha_accel::PtxArtifact;
use std::{ffi::CStr, sync::OnceLock};

#[derive(Clone, Copy)]
struct FamilyInput {
    source: &'static [u8],
    ptx: &'static [u8],
    nul: &'static [u8],
}
struct Candidate {
    manifest: &'static [u8],
    public_key: &'static [u8],
    signature: &'static [u8],
    families: [FamilyInput; 10],
}
// Genuine absence emits only None, without any include of missing native bytes.
include!(concat!(env!("OUT_DIR"), "/cuda_bundle.rs"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactRefusal {
    Absent,
    Unreviewed,
    Provenance(ProvenanceRefusal),
    Source(&'static str),
    Ptx(&'static str, PtxRefusal),
    CStr(&'static str),
}
struct AdmittedBundle {
    // Keep every canonical input in the same immutable owner as its artifacts.
    candidate: &'static Candidate,
    artifacts: [PtxArtifact; 10],
}
static ADMITTED: OnceLock<Result<AdmittedBundle, ArtifactRefusal>> = OnceLock::new();

fn admit_retained(
    candidate: Option<&'static Candidate>,
    pins: Option<ReviewedCudaBundlePins>,
) -> Result<AdmittedBundle, ArtifactRefusal> {
    let candidate = candidate.ok_or(ArtifactRefusal::Absent)?;
    let pins = pins.ok_or(ArtifactRefusal::Unreviewed)?;
    let stems = cuda_stems();
    let mut reader = BorrowedCudaReader::new(
        candidate.manifest,
        candidate.public_key,
        candidate.signature,
        &stems,
        pins.public_key_sha256,
    )
    .map_err(ArtifactRefusal::Provenance)?;
    for family in &candidate.families {
        reader.next_row().map_err(ArtifactRefusal::Provenance)?;
        reader
            .accept_row(family.source, family.ptx)
            .map_err(ArtifactRefusal::Provenance)?;
    }
    reader
        .finish(pins.manifest_sha256)
        .map_err(ArtifactRefusal::Provenance)?;
    // All original canonical phases finish before additional exact source/CStr
    // ownership checks. No partial relation publishes a native artifact.
    for (source, family) in CUDA_SOURCES.iter().zip(&candidate.families) {
        if !crate::cuda_provenance::matches_sha256(family.source, source.sha256) {
            return Err(ArtifactRefusal::Source(source.stem));
        }
        validate_ptx(family.ptx).map_err(|error| ArtifactRefusal::Ptx(source.stem, error))?;
        let cstr = CStr::from_bytes_with_nul(family.nul)
            .map_err(|_| ArtifactRefusal::CStr(source.stem))?;
        if cstr.to_bytes() != family.ptx {
            return Err(ArtifactRefusal::CStr(source.stem));
        }
    }
    let artifacts = std::array::from_fn(|index| {
        // The exact same immutable range was checked before any owner exists.
        PtxArtifact::new(
            CStr::from_bytes_with_nul(candidate.families[index].nul)
                .expect("checked original CUDA CStr range"),
        )
    });
    Ok(AdmittedBundle {
        candidate,
        artifacts,
    })
}
fn admitted() -> Result<&'static AdmittedBundle, ArtifactRefusal> {
    ADMITTED
        .get_or_init(|| admit_retained(CANDIDATE.as_ref(), REVIEWED_CUDA_BUNDLE_PINS))
        .as_ref()
        .map_err(|error| *error)
}
/// Guard the IVM consumer before shared physical installation/discovery. This
/// neither clears quarantine nor disables other users of the existing process.
pub(crate) fn eligible() -> bool {
    admitted().is_ok()
}
fn family_index(kernel: Kernel) -> usize {
    match kernel {
        Kernel::AesEnc | Kernel::AesDec | Kernel::AesEncFused | Kernel::AesDecFused => 0,
        Kernel::Bitonic => 1,
        Kernel::BnAdd | Kernel::BnSub | Kernel::BnMul => 2,
        Kernel::Poseidon2 | Kernel::Poseidon6 => 3,
        Kernel::Sha256 => 4,
        Kernel::ShaLeaves => 5,
        Kernel::ShaPairs => 6,
        Kernel::Keccak => 7,
        Kernel::Ed25519 => 8,
        Kernel::Add32 | Kernel::Add64 | Kernel::And | Kernel::Xor | Kernel::Or => 9,
    }
}
/// Every consumer receives the exact original CStr retained in the one admitted
/// owner, then carries that same PtxArtifact through admission/stage/completion.
pub(crate) fn artifact(kernel: Kernel) -> Result<PtxArtifact, iroha_accel::cuda::CudaFailure> {
    let bundle = admitted().map_err(|_| iroha_accel::cuda::CudaFailure::Unavailable)?;
    debug_assert_eq!(bundle.candidate.families.len(), bundle.artifacts.len());
    Ok(bundle.artifacts[family_index(kernel)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_real_owner_refuses_all_twenty_artifacts_and_caches_same_refusal() {
        assert_eq!(REVIEWED_CUDA_BUNDLE_PINS, None);
        assert!(CANDIDATE.is_none());
        for _ in 0..2 {
            assert!(!eligible());
            for kernel in Kernel::ALL {
                assert_eq!(
                    artifact(kernel),
                    Err(iroha_accel::cuda::CudaFailure::Unavailable)
                );
            }
        }
        assert!(matches!(ADMITTED.get(), Some(Err(ArtifactRefusal::Absent))));
    }
    #[test]
    fn all_twenty_kernel_members_keep_original_ten_family_mapping() {
        assert_eq!(Kernel::ALL.len(), 20);
        let expected = [
            (Kernel::Add32, "vector"),
            (Kernel::Add64, "vector"),
            (Kernel::And, "vector"),
            (Kernel::Xor, "vector"),
            (Kernel::Or, "vector"),
            (Kernel::Sha256, "sha256"),
            (Kernel::Keccak, "sha3"),
            (Kernel::ShaLeaves, "sha256_leaves"),
            (Kernel::ShaPairs, "sha256_pairs_reduce"),
            (Kernel::Poseidon2, "poseidon"),
            (Kernel::Poseidon6, "poseidon"),
            (Kernel::AesEnc, "aes"),
            (Kernel::AesDec, "aes"),
            (Kernel::AesEncFused, "aes"),
            (Kernel::AesDecFused, "aes"),
            (Kernel::BnAdd, "bn254"),
            (Kernel::BnSub, "bn254"),
            (Kernel::BnMul, "bn254"),
            (Kernel::Ed25519, "signature"),
            (Kernel::Bitonic, "bitonic_sort"),
        ];
        for (kernel, stem) in expected {
            assert_eq!(CUDA_SOURCES[family_index(kernel)].stem, stem);
        }
    }
    #[test]
    fn original_directive_contract_remains_required_without_placeholder_artifacts() {
        let expected = [
            "aes",
            "bitonic_sort",
            "bn254",
            "poseidon",
            "sha256",
            "sha256_leaves",
            "sha256_pairs_reduce",
            "sha3",
            "signature",
            "vector",
        ];
        assert_eq!(cuda_stems(), expected);
        // Supersedes the retired unconditional OUT_DIR provisioning assertion:
        // actual presence belongs to the private admitted owner; None is explicit.
        assert!(matches!(
            admit_retained(None, None),
            Err(ArtifactRefusal::Absent)
        ));
        assert!(validate_ptx(b"// Placeholder PTX; CUDA stays disabled.\n").is_err());
        let ptx =
            b".version 7.8\n.target sm_86\n.address_size 64\n.visible .entry kernel() { ret; }\n";
        assert!(validate_ptx(ptx).is_ok());
        for directive in [".version", ".target", ".address_size", ".entry"] {
            assert!(
                std::str::from_utf8(ptx)
                    .unwrap()
                    .split_ascii_whitespace()
                    .any(|token| token == directive)
            );
            let missing = std::str::from_utf8(ptx)
                .unwrap()
                .replace(directive, "retired");
            assert_eq!(
                validate_ptx(missing.as_bytes()),
                Err(PtxRefusal::Directive(directive))
            );
        }
        assert!(matches!(validate_ptx(&[0xff]), Err(PtxRefusal::Utf8(_))));
        let mut interior_nul = ptx.to_vec();
        interior_nul.push(0);
        assert_eq!(validate_ptx(&interior_nul), Err(PtxRefusal::InteriorNul));
    }
    #[test]
    fn unreviewed_and_malformed_candidates_never_construct_an_admitted_owner() {
        // Synthetic empty bytes test only refusal. They are not a fabricated
        // authenticated bundle and cannot satisfy the actual source descriptor.
        static MALFORMED: Candidate = Candidate {
            manifest: &[],
            public_key: &[],
            signature: &[],
            families: [FamilyInput {
                source: &[],
                ptx: &[],
                nul: &[],
            }; 10],
        };
        assert!(matches!(
            admit_retained(Some(&MALFORMED), None),
            Err(ArtifactRefusal::Unreviewed)
        ));
        let test_shape = ReviewedCudaBundlePins {
            public_key_sha256: "1111111111111111111111111111111111111111111111111111111111111111",
            manifest_sha256: "2222222222222222222222222222222222222222222222222222222222222222",
        };
        assert!(matches!(
            admit_retained(Some(&MALFORMED), Some(test_shape)),
            Err(ArtifactRefusal::Provenance(
                ProvenanceRefusal::PublicKeyLength
            ))
        ));
        assert!(matches!(
            admit_retained(None, Some(test_shape)),
            Err(ArtifactRefusal::Absent)
        ));
        assert_eq!(REVIEWED_CUDA_BUNDLE_PINS, None);
        assert!(!eligible());
    }
}
