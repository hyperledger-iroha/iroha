//! Typed X.509 native-resource capture and installed-pin enforcement.

use iroha_core::privacy_release_evidence::{
    PrivacyReleaseCaseKindV1, PrivacyReleaseZkX509ResourceCertificateV1,
    PrivacyReleaseZkX509ResourceEnvironmentV1, PrivacyReleaseZkX509ResourceObservationV1,
    build_privacy_release_zk_x509_resource_certificate_v1,
    privacy_release_zk_x509_resource_certificate_matches_source_v1,
    privacy_release_zk_x509_resource_environment_v1,
    validate_privacy_release_zk_x509_resource_capture_v1,
};
use iroha_data_model::privacy::PrivacyProtocolIdV1;

use super::*;

const MAX_X509_RESOURCE_NORITO_BYTES_V1: u64 = 64 * 1024;
const MAX_X509_RESOURCE_JSON_BYTES_V1: u64 = 64 * 1024;
const MAX_X509_HOST_METADATA_JSON_BYTES_V1: u64 = 16 * 1024;

pub(super) fn capture_option_names() -> Vec<&'static str> {
    vec![
        "exact12-matrix",
        "expectations-norito-out",
        "expectations-json-out",
        "x509-resource-host-metadata",
        "x509-resource-norito-out",
        "x509-resource-json-out",
        "elapsed-ceiling-ms",
        "peak-rss-ceiling-bytes",
        "address-space-ceiling-bytes",
    ]
}

#[derive(Clone)]
pub(super) struct ResourceInputPathsV1 {
    pub(super) norito: PathBuf,
    pub(super) json: PathBuf,
}

impl ResourceInputPathsV1 {
    pub(super) fn parse(options: &BTreeMap<String, String>) -> Result<Self, DynError> {
        Ok(Self {
            norito: path_option(options, "x509-resource-norito")?,
            json: path_option(options, "x509-resource-json")?,
        })
    }
}

#[derive(Clone)]
pub(super) struct CaptureResourceOptionsV1 {
    pub(super) host_metadata_json: PathBuf,
    pub(super) norito_out: PathBuf,
    pub(super) json_out: PathBuf,
}

impl CaptureResourceOptionsV1 {
    pub(super) fn parse(options: &BTreeMap<String, String>) -> Result<Self, DynError> {
        Ok(Self {
            host_metadata_json: path_option(options, "x509-resource-host-metadata")?,
            norito_out: path_option(options, "x509-resource-norito-out")?,
            json_out: path_option(options, "x509-resource-json-out")?,
        })
    }

    pub(super) fn output_paths(&self) -> [PathBuf; 2] {
        [self.norito_out.clone(), self.json_out.clone()]
    }
}

pub(super) struct CaptureResourceArtifactsV1 {
    pub(super) norito: Vec<u8>,
    pub(super) json: Vec<u8>,
}

pub(super) struct CaptureResourceMeasurementsV1 {
    positive: PrivacyReleaseZkX509ResourceObservationV1,
    maximum: PrivacyReleaseZkX509ResourceObservationV1,
    kat_proof_bytes: u32,
    kat_proof_sha256: [u8; 32],
}

pub(super) struct LoadedResourceCertificateV1 {
    pub(super) certificate: PrivacyReleaseZkX509ResourceCertificateV1,
    pub(super) norito_bytes: Vec<u8>,
    pub(super) json_bytes: Vec<u8>,
    pub(super) norito_identity: FileIdentityV1,
    pub(super) json_identity: FileIdentityV1,
}

pub(super) fn load_capture_environment_v1(
    options: &CaptureResourceOptionsV1,
    exact12: &SecureInputV1,
    runner: &ImmutableRunnerV1,
) -> Result<PrivacyReleaseZkX509ResourceEnvironmentV1, DynError> {
    reject_lexical_path_aliases(&[
        options.host_metadata_json.clone(),
        options.norito_out.clone(),
        options.json_out.clone(),
        runner.source_path.clone(),
    ])?;
    let input = secure_read(
        &options.host_metadata_json,
        MAX_X509_HOST_METADATA_JSON_BYTES_V1,
        "X.509 resource host metadata JSON",
    )?;
    if input.identity == exact12.identity || input.identity == runner.source_identity {
        return Err(
            "X.509 resource host metadata aliases another capture input or the runner".into(),
        );
    }
    let environment: PrivacyReleaseZkX509ResourceEnvironmentV1 =
        decode_canonical_json(&input.bytes, "X.509 resource host metadata JSON")?;
    if environment != privacy_release_zk_x509_resource_environment_v1() {
        return Err(
            "X.509 resource host metadata does not equal the compiled native environment".into(),
        );
    }
    Ok(environment)
}

pub(super) fn build_capture_artifacts_v1(
    measurements: CaptureResourceMeasurementsV1,
    expectations_norito: &[u8],
    expectations_json: &[u8],
    environment: PrivacyReleaseZkX509ResourceEnvironmentV1,
) -> Result<CaptureResourceArtifactsV1, DynError> {
    let certificate = build_privacy_release_zk_x509_resource_certificate_v1(
        environment,
        sha256_bytes(expectations_norito),
        sha256_bytes(expectations_json),
        measurements.kat_proof_bytes,
        measurements.kat_proof_sha256,
        measurements.positive,
        measurements.maximum,
    )
    .map_err(|error| format!("cannot build X.509 resource certificate: {error:?}"))?;
    if !validate_privacy_release_zk_x509_resource_capture_v1(&certificate) {
        return Err("new X.509 resource certificate failed independent validation".into());
    }
    let norito = canonical_norito_bytes(&certificate, "X.509 resource certificate")?;
    let json = canonical_json_bytes(&certificate, "X.509 resource certificate")?;
    enforce_encoded_size(
        norito.len(),
        MAX_X509_RESOURCE_NORITO_BYTES_V1,
        "X.509 resource certificate Norito",
    )?;
    enforce_encoded_size(
        json.len(),
        MAX_X509_RESOURCE_JSON_BYTES_V1,
        "X.509 resource certificate JSON",
    )?;
    Ok(CaptureResourceArtifactsV1 { norito, json })
}

pub(super) fn capture_measurements_v1(
    measured: &[MeasuredStageV1],
) -> Result<CaptureResourceMeasurementsV1, DynError> {
    let positive = exact_observation_v1(
        measured,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    )?;
    let maximum = exact_observation_v1(measured, PrivacyReleaseCaseKindV1::MaximumShapeResource)?;
    let (kat_proof_bytes, kat_proof_sha256) = exact_positive_kat_v1(measured)?;
    Ok(CaptureResourceMeasurementsV1 {
        positive,
        maximum,
        kat_proof_bytes,
        kat_proof_sha256,
    })
}

fn exact_observation_v1(
    measured: &[MeasuredStageV1],
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<PrivacyReleaseZkX509ResourceObservationV1, DynError> {
    let stage = exact_stage_v1(measured, case_kind)?;
    let resources = stage.evidence.resources;
    Ok(PrivacyReleaseZkX509ResourceObservationV1 {
        case_kind,
        elapsed_millis: stage.elapsed_millis,
        peak_rss_bytes: stage.peak_rss_bytes,
        peak_address_space_bytes: stage.peak_address_space_bytes,
        primary_units: resources.primary_units,
        primary_ceiling: resources.primary_ceiling,
        secondary_units: resources.secondary_units,
        secondary_ceiling: resources.secondary_ceiling,
        relation_depth: resources.relation_depth,
        relation_depth_ceiling: resources.relation_depth_ceiling,
    })
}

fn exact_stage_v1(
    measured: &[MeasuredStageV1],
    case_kind: PrivacyReleaseCaseKindV1,
) -> Result<&MeasuredStageV1, DynError> {
    let mut matches = measured.iter().filter(|stage| {
        stage.evidence.protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
            && stage.evidence.case_kind == case_kind
    });
    let stage = matches
        .next()
        .ok_or("X.509 resource capture is missing a mandatory measured stage")?;
    if matches.next().is_some() {
        return Err("X.509 resource capture contains a duplicate measured stage".into());
    }
    Ok(stage)
}

fn exact_positive_kat_v1(measured: &[MeasuredStageV1]) -> Result<(u32, [u8; 32]), DynError> {
    let stage = exact_stage_v1(
        measured,
        PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
    )?;
    let [artifact] = stage.evidence.proof_artifacts.as_slice() else {
        return Err("X.509 positive stage must contain exactly one KAT proof artifact".into());
    };
    if artifact.artifact_ordinal != 0
        || sha256_bytes(&artifact.canonical_proof_bytes) != artifact.proof_sha256
    {
        return Err("X.509 positive KAT proof artifact is not canonical".into());
    }
    let proof_bytes = u32::try_from(artifact.canonical_proof_bytes.len())
        .map_err(|_| "X.509 positive KAT proof length exceeds u32")?;
    Ok((proof_bytes, artifact.proof_sha256))
}

pub(super) fn load_pinned_pair_v1(
    norito_path: &Path,
    json_path: &Path,
) -> Result<LoadedResourceCertificateV1, DynError> {
    let norito = secure_read(
        norito_path,
        MAX_X509_RESOURCE_NORITO_BYTES_V1,
        "X.509 resource certificate Norito",
    )?;
    let json = secure_read(
        json_path,
        MAX_X509_RESOURCE_JSON_BYTES_V1,
        "X.509 resource certificate JSON",
    )?;
    if norito.identity == json.identity {
        return Err("X.509 resource certificate projections alias one inode".into());
    }
    let certificate: PrivacyReleaseZkX509ResourceCertificateV1 = decode_canonical_norito(
        &norito.bytes,
        MAX_X509_RESOURCE_NORITO_BYTES_V1,
        "X.509 resource certificate Norito",
    )?;
    let json_certificate: PrivacyReleaseZkX509ResourceCertificateV1 =
        decode_canonical_json(&json.bytes, "X.509 resource certificate JSON")?;
    if certificate != json_certificate {
        return Err(
            "X.509 resource certificate JSON is not typed-equal to authoritative Norito".into(),
        );
    }
    if !privacy_release_zk_x509_resource_certificate_matches_source_v1(&certificate) {
        return Err(
            "X.509 resource certificate does not match every compiled field and pin".into(),
        );
    }
    Ok(LoadedResourceCertificateV1 {
        certificate,
        norito_bytes: norito.bytes,
        json_bytes: json.bytes,
        norito_identity: norito.identity,
        json_identity: json.identity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured_stage_v1(case_kind: PrivacyReleaseCaseKindV1) -> MeasuredStageV1 {
        MeasuredStageV1 {
            evidence: expectation_pins::empty_expected_evidence(
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                case_kind,
            ),
            elapsed_millis: if case_kind == PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd {
                1
            } else {
                2
            },
            peak_rss_bytes: 1024 * 1024,
            peak_address_space_bytes: 64 * 1024 * 1024,
        }
    }

    #[test]
    fn capture_derives_the_kat_and_both_exact_resource_observations() {
        let measured = [
            measured_stage_v1(PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd),
            measured_stage_v1(PrivacyReleaseCaseKindV1::MaximumShapeResource),
        ];
        let measurements =
            capture_measurements_v1(&measured).expect("closed X.509 capture measurements");
        let artifacts = build_capture_artifacts_v1(
            measurements,
            b"canonical expectations Norito",
            b"canonical expectations JSON",
            privacy_release_zk_x509_resource_environment_v1(),
        )
        .expect("typed X.509 resource capture");
        let certificate: PrivacyReleaseZkX509ResourceCertificateV1 = decode_canonical_norito(
            &artifacts.norito,
            MAX_X509_RESOURCE_NORITO_BYTES_V1,
            "X.509 resource test capture",
        )
        .expect("canonical resource Norito");
        let projection: PrivacyReleaseZkX509ResourceCertificateV1 =
            decode_canonical_json(&artifacts.json, "X.509 resource test capture")
                .expect("canonical resource JSON");
        assert_eq!(certificate, projection);
        assert_eq!(certificate.kat_proof_bytes, 1);
        assert_eq!(certificate.kat_proof_sha256, sha256_bytes(&[1]));
        assert_eq!(
            certificate.positive.case_kind,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd
        );
        assert_eq!(
            certificate.maximum.case_kind,
            PrivacyReleaseCaseKindV1::MaximumShapeResource
        );
    }

    #[test]
    fn capture_rejects_missing_duplicate_and_corrupt_positive_kat_stages() {
        let positive = measured_stage_v1(PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd);
        let maximum = measured_stage_v1(PrivacyReleaseCaseKindV1::MaximumShapeResource);
        assert!(exact_observation_v1(&[maximum.clone()], positive.evidence.case_kind).is_err());
        assert!(
            exact_observation_v1(
                &[positive.clone(), positive.clone()],
                positive.evidence.case_kind,
            )
            .is_err()
        );
        let mut corrupt = positive;
        corrupt.evidence.proof_artifacts[0].proof_sha256[0] ^= 1;
        assert!(exact_positive_kat_v1(&[corrupt, maximum]).is_err());
    }
}
