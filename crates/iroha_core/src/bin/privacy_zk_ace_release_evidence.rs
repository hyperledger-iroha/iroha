//! Isolated deterministic ZK-ACE release-evidence capture and verification.
//!
//! Each invocation handles exactly one of the four mandatory evidence cases.
//! `capture <case>` writes only the canonical Norito stage archive to stdout
//! and reports its SHA-256 on stderr. `verify <case> <sha256>` reads that archive
//! from stdin, validates its closed semantics, regenerates the native proof in
//! a fresh process topology, and requires an exact byte-for-byte match.
//! `verify-pinned <case>` performs the same operation against the reviewed
//! source pin and fails while that pin is the all-zero sentinel.
//!
//! This binary is feature-isolated and never enables the production engine or
//! writes a source pin. A reviewer must retain the canonical archives, run the
//! independent verification invocations, and explicitly admit their digests.
use iroha_core::{
    privacy_engines::zk_ace::{
        ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1, ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2,
    },
    privacy_release_evidence::{
        PrivacyReleaseCaseKindV1, PrivacyReleaseStageEvidenceV1,
        initialize_privacy_release_rayon_pool_v1, privacy_release_stage_evidence_sha256_v1,
        run_privacy_release_stage_v1, validate_privacy_release_stage_evidence_v1,
    },
};
use iroha_data_model::privacy::PrivacyProtocolIdV1;
use std::{
    env,
    error::Error,
    io::{self, Read as _, Write as _},
};

const ARCHIVE_OVERHEAD_CEILING_BYTES: u64 = 256 * 1024;
const ARCHIVE_DECODE_ALLOCATION_MULTIPLIER: usize = 8;
const ARCHIVE_DECODE_ALLOCATION_CEILING_BYTES: usize = 13_514_416;

fn maximum_archive_bytes() -> u64 {
    u64::from(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1) + ARCHIVE_OVERHEAD_CEILING_BYTES
}

fn archive_decode_limits(
    maximum_archive_bytes: usize,
) -> Result<norito::DecodeLimits, Box<dyn Error>> {
    let allocation_ceiling = maximum_archive_bytes
        .checked_mul(ARCHIVE_DECODE_ALLOCATION_MULTIPLIER)
        .ok_or("ZK-ACE evidence decode-allocation ceiling overflowed")?;
    if allocation_ceiling != ARCHIVE_DECODE_ALLOCATION_CEILING_BYTES {
        return Err("ZK-ACE evidence archive or decode-allocation ceiling drifted".into());
    }
    Ok(norito::DecodeLimits::new(
        maximum_archive_bytes,
        maximum_archive_bytes,
        maximum_archive_bytes.saturating_add(64),
        // Valid canonical decoding deterministically crossed 4,285,245 bytes
        // under the former 2x cap and 7,139,578 bytes under the former 4x
        // cap. Those charges come from bounded nested owned fields containing
        // the same 1,427,158-byte proof. The fixed 8x ceiling remains far
        // below Norito's ordinary 64x canonical policy while preserving the
        // independent archive, field, sequence, element, and depth limits.
        allocation_ceiling,
        16,
    ))
}

enum OperationV1 {
    Capture {
        case_kind: PrivacyReleaseCaseKindV1,
    },
    Verify {
        case_kind: PrivacyReleaseCaseKindV1,
        expected_sha256: [u8; 32],
    },
    VerifyPinned {
        case_kind: PrivacyReleaseCaseKindV1,
    },
}

fn parse_case(label: &str) -> Result<PrivacyReleaseCaseKindV1, Box<dyn Error>> {
    PrivacyReleaseCaseKindV1::from_canonical_label(label)
        .ok_or_else(|| format!("unsupported ZK-ACE evidence case `{label}`").into())
}

fn parse_sha256(value: &str) -> Result<[u8; 32], Box<dyn Error>> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected SHA-256 must contain exactly 64 lowercase hexadecimal digits".into());
    }
    let decoded = hex::decode(value)?;
    let digest: [u8; 32] = decoded
        .try_into()
        .map_err(|_| "expected SHA-256 must decode to exactly 32 bytes")?;
    if hex::encode(digest) != value {
        return Err("expected SHA-256 must use canonical lowercase hexadecimal".into());
    }
    if digest == [0; 32] {
        return Err("the all-zero SHA-256 sentinel is not release evidence".into());
    }
    Ok(digest)
}

fn parse_operation() -> Result<OperationV1, Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let operation = arguments.next().ok_or(
        "usage: privacy_zk_ace_release_evidence <capture|verify|verify-pinned> <case> [sha256]",
    )?;
    let case_kind = parse_case(
        &arguments
            .next()
            .ok_or("missing canonical ZK-ACE evidence case")?,
    )?;
    let parsed = match operation.as_str() {
        "capture" => OperationV1::Capture { case_kind },
        "verify" => OperationV1::Verify {
            case_kind,
            expected_sha256: parse_sha256(
                &arguments
                    .next()
                    .ok_or("verify requires the reviewed expected SHA-256")?,
            )?,
        },
        "verify-pinned" => OperationV1::VerifyPinned { case_kind },
        _ => return Err(format!("unsupported operation `{operation}`").into()),
    };
    if arguments.next().is_some() {
        return Err("unexpected extra argument".into());
    }
    Ok(parsed)
}

fn reviewed_source_pin(case_kind: PrivacyReleaseCaseKindV1) -> Result<[u8; 32], Box<dyn Error>> {
    let index = PrivacyReleaseCaseKindV1::ALL
        .iter()
        .position(|candidate| *candidate == case_kind)
        .ok_or("requested case is absent from the closed ZK-ACE stage order")?;
    let digest = ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2[index];
    if digest == [0; 32] {
        return Err(
            "the requested ZK-ACE source pin is still the fail-closed zero sentinel".into(),
        );
    }
    Ok(digest)
}

fn read_bounded_archive() -> Result<Vec<u8>, Box<dyn Error>> {
    let maximum_archive_bytes = maximum_archive_bytes();
    let read_ceiling = maximum_archive_bytes
        .checked_add(1)
        .ok_or("ZK-ACE evidence read ceiling overflowed")?;
    let mut archive = Vec::with_capacity(usize::try_from(maximum_archive_bytes)?);
    io::stdin()
        .lock()
        .take(read_ceiling)
        .read_to_end(&mut archive)?;
    if archive.is_empty() {
        return Err("canonical ZK-ACE evidence archive is empty".into());
    }
    if u64::try_from(archive.len())? > maximum_archive_bytes {
        return Err("canonical ZK-ACE evidence archive exceeds its fixed ceiling".into());
    }
    Ok(archive)
}

fn capture(case_kind: PrivacyReleaseCaseKindV1) -> Result<(), Box<dyn Error>> {
    initialize_privacy_release_rayon_pool_v1()?;
    let evidence =
        run_privacy_release_stage_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0, case_kind)?;
    if !validate_privacy_release_stage_evidence_v1(&evidence) {
        return Err("generated ZK-ACE stage failed closed evidence admission".into());
    }
    let digest = privacy_release_stage_evidence_sha256_v1(&evidence)
        .ok_or("generated ZK-ACE stage has no canonical reviewed-pin digest")?;
    let canonical = norito::encode_canonical(&evidence)?;
    if u64::try_from(canonical.len())? > maximum_archive_bytes() {
        return Err("generated ZK-ACE evidence archive exceeds its fixed ceiling".into());
    }
    eprintln!("zk-ace-stage-evidence-sha256={}", hex::encode(digest));
    let mut stdout = io::stdout().lock();
    stdout.write_all(&canonical)?;
    stdout.flush()?;
    Ok(())
}

fn verify(
    case_kind: PrivacyReleaseCaseKindV1,
    expected_sha256: [u8; 32],
) -> Result<(), Box<dyn Error>> {
    let archive = read_bounded_archive()?;
    let maximum_archive_bytes = usize::try_from(maximum_archive_bytes())?;
    let decode_limits = archive_decode_limits(maximum_archive_bytes)?;
    let evidence = norito::decode_canonical_with_limits::<PrivacyReleaseStageEvidenceV1>(
        &archive,
        decode_limits,
    )?;
    if evidence.protocol_id != PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
        || evidence.case_kind != case_kind
        || !validate_privacy_release_stage_evidence_v1(&evidence)
    {
        return Err("input is not the exact requested ZK-ACE stage evidence".into());
    }
    let canonical = norito::encode_canonical(&evidence)?;
    if canonical != archive {
        return Err("input ZK-ACE evidence is not its exact canonical Norito archive".into());
    }
    let observed_sha256 = privacy_release_stage_evidence_sha256_v1(&evidence)
        .ok_or("input ZK-ACE evidence has no canonical reviewed-pin digest")?;
    if observed_sha256 != expected_sha256 {
        return Err("input ZK-ACE evidence SHA-256 differs from the reviewed digest".into());
    }

    initialize_privacy_release_rayon_pool_v1()?;
    let regenerated =
        run_privacy_release_stage_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0, case_kind)?;
    if regenerated != evidence || norito::encode_canonical(&regenerated)? != archive {
        return Err("fresh native ZK-ACE evidence regeneration was not byte-identical".into());
    }
    println!("{}", hex::encode(observed_sha256));
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    match parse_operation()? {
        OperationV1::Capture { case_kind } => capture(case_kind),
        OperationV1::Verify {
            case_kind,
            expected_sha256,
        } => verify(case_kind, expected_sha256),
        OperationV1::VerifyPinned { case_kind } => {
            verify(case_kind, reviewed_source_pin(case_kind)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::privacy_release_evidence::{
        PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1, PrivacyReleaseFailureClassV1,
        PrivacyReleaseProofArtifactEvidenceV1, privacy_release_protocol_descriptor_v1,
        privacy_release_resource_facts_v1, privacy_release_stage_ordinal_v1,
    };
    use sha2::{Digest as _, Sha256};

    fn synthetic_valid_archive() -> Vec<u8> {
        let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
        let case_kind = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let canonical_proof_bytes = vec![0xA5; ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1 as usize];
        let proof_sha256 = Sha256::digest(&canonical_proof_bytes).into();
        let evidence = PrivacyReleaseStageEvidenceV1 {
            schema_version: PRIVACY_RELEASE_EVIDENCE_SCHEMA_VERSION_V1,
            stage_ordinal: privacy_release_stage_ordinal_v1(protocol_id, case_kind),
            protocol_id,
            case_kind,
            protocol_descriptor: privacy_release_protocol_descriptor_v1(protocol_id).to_owned(),
            public_statement_sha256: [0x5A; 32],
            proof_artifacts: vec![PrivacyReleaseProofArtifactEvidenceV1 {
                artifact_ordinal: 0,
                canonical_proof_bytes,
                proof_sha256,
                proof_bytes_ceiling: u64::from(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1),
            }],
            failure_class: PrivacyReleaseFailureClassV1::NotApplicable,
            resources: privacy_release_resource_facts_v1(protocol_id, case_kind)
                .expect("ZK-ACE resource facts are frozen"),
        };
        assert!(validate_privacy_release_stage_evidence_v1(&evidence));
        norito::encode_canonical(&evidence).expect("encode synthetic admitted ZK-ACE archive")
    }

    #[test]
    fn exact_case_and_digest_parsers_reject_aliases_and_sentinels() {
        assert_eq!(
            parse_case("maximum-shape-resource").expect("canonical case"),
            PrivacyReleaseCaseKindV1::MaximumShapeResource
        );
        assert!(parse_case("MaximumShapeResource").is_err());
        assert_eq!(
            parse_sha256(&"01".repeat(32)).expect("canonical digest"),
            [1; 32]
        );
        assert!(parse_sha256(&"00".repeat(32)).is_err());
        assert!(parse_sha256(&"AB".repeat(32)).is_err());
        if ZK_ACE_RELEASE_STAGE_EVIDENCE_SHA256_V2 == [[0; 32]; 4] {
            assert!(
                reviewed_source_pin(PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd).is_err()
            );
        }
    }

    #[test]
    fn fixed_decode_budget_admits_one_valid_archive_and_rejects_cumulative_over_cap() {
        let archive = synthetic_valid_archive();
        assert!(
            u64::try_from(archive.len()).expect("archive length fits u64")
                <= maximum_archive_bytes()
        );
        let maximum_archive_bytes =
            usize::try_from(maximum_archive_bytes()).expect("archive ceiling fits usize");
        let limits = archive_decode_limits(maximum_archive_bytes).expect("fixed decode limits");
        assert_eq!(
            limits.max_total_allocated_bytes(),
            ARCHIVE_DECODE_ALLOCATION_CEILING_BYTES
        );
        let decoded =
            norito::decode_canonical_with_limits::<PrivacyReleaseStageEvidenceV1>(&archive, limits)
                .expect("the fixed 8x budget admits one valid maximum-proof archive");
        assert!(validate_privacy_release_stage_evidence_v1(&decoded));

        let cumulative = norito::with_decode_limits(limits, || {
            for _ in 0..3 {
                let _: PrivacyReleaseStageEvidenceV1 =
                    norito::decode_canonical_with_limits(&archive, limits)?;
            }
            Ok(())
        });
        assert!(matches!(
            cumulative,
            Err(norito::Error::TotalAllocationExceeded { attempted, limit })
                if attempted > limit
                    && limit
                        == u64::try_from(ARCHIVE_DECODE_ALLOCATION_CEILING_BYTES)
                            .expect("fixed allocation ceiling fits u64")
        ));
    }
}
