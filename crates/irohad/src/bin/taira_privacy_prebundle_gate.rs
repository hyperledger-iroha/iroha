//! Fail-closed privacy capability gate for public Taira validator bundles.
use iroha_core::privacy_profiles::compiled_privacy_profile_v1;
#[cfg(test)]
use iroha_data_model::privacy::privacy_protocol_label_is_reserved_v1;
use iroha_data_model::privacy::{
    PRIVACY_RETIRED_PROTOCOL_LABELS_V1, PrivacyCompiledProfileSnapshotV1, PrivacyProtocolIdV1,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
const EXACT12_MATRIX: &str = include_str!("../../../../fixtures/privacy/exact12_v1.tsv");
const EXPECTED_PROFILE_COUNT: usize = 12;
const EXPECTED_REGISTRY_SHA256: &str =
    "734eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51";
// Derived from `privacy_exact12_matrix_bytes_v1`; the checked-in cross-SDK
// matrix and this deployment pin must be regenerated together.
const EXPECTED_MATRIX_SHA256: &str =
    "f75eeba824067aaf903fd8060c967190e37073dc07e487c81c265018a1c00f38";
fn is_canonical_nonzero_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Taira privacy prebundle gate failed: {error}");
        std::process::exit(1);
    }
}
fn validate_exact12_matrix_v1(matrix: &str) -> Result<String, String> {
    if !matrix.ends_with('\n')
        || matrix.contains('\r')
        || matrix
            .strip_suffix('\n')
            .is_none_or(|matrix| matrix.lines().any(str::is_empty))
    {
        return Err("exact12 matrix is not canonical LF-delimited text".to_owned());
    }
    let mut matrix_version = None;
    let mut expected_registry_digest = None;
    let mut protocol_rows = Vec::new();
    let mut typed_envelope_rows = Vec::new();
    let mut retired_labels = Vec::new();
    for (line_index, line) in matrix.lines().enumerate() {
        if line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        match fields.as_slice() {
            ["matrix-version", version] => {
                if matrix_version.replace(*version).is_some() {
                    return Err("exact12 matrix repeats its version".to_owned());
                }
            }
            ["registry-sha256", digest] => {
                if expected_registry_digest.replace(*digest).is_some() {
                    return Err("exact12 matrix repeats its registry digest".to_owned());
                }
            }
            ["protocol", index, label, statement_variant, proof_variant] => {
                let index = index.parse::<usize>().map_err(|_| {
                    format!("exact12 protocol row {} has a bad index", line_index + 1)
                })?;
                protocol_rows.push((index, *label, *statement_variant, *proof_variant));
            }
            [
                "typed-envelope",
                label,
                statement_variant,
                proof_variant,
                statement_digest,
                envelope_sha256,
            ] => typed_envelope_rows.push((
                *label,
                *statement_variant,
                *proof_variant,
                *statement_digest,
                *envelope_sha256,
            )),
            ["retired", label] => retired_labels.push(*label),
            _ => {
                return Err(format!(
                    "exact12 matrix row {} has an unsupported shape",
                    line_index + 1
                ));
            }
        }
    }
    if matrix_version != Some("1") {
        return Err("exact12 matrix version is not one".to_owned());
    }
    if expected_registry_digest != Some(EXPECTED_REGISTRY_SHA256) {
        return Err(
            "exact12 matrix does not carry the frozen first-release registry digest".into(),
        );
    }
    if PrivacyProtocolIdV1::ALL.len() != EXPECTED_PROFILE_COUNT
        || protocol_rows.len() != EXPECTED_PROFILE_COUNT
        || typed_envelope_rows.len() != EXPECTED_PROFILE_COUNT
    {
        return Err(format!(
            "privacy registry is not exact12: enum={}, protocols={}, typed_envelopes={}",
            PrivacyProtocolIdV1::ALL.len(),
            protocol_rows.len(),
            typed_envelope_rows.len()
        ));
    }
    let mut registry_preimage = String::new();
    let mut active_labels = BTreeSet::new();
    for (expected_index, protocol_id) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
        let (matrix_index, matrix_label, statement_variant, proof_variant) =
            protocol_rows[expected_index];
        if matrix_index != expected_index || matrix_label != protocol_id.canonical_label() {
            return Err(format!(
                "exact12 route {expected_index} does not match the compiled registry"
            ));
        }
        if !active_labels.insert(matrix_label) {
            return Err(format!(
                "exact12 active protocol label {matrix_label:?} is duplicated"
            ));
        }
        let (
            typed_label,
            typed_statement_variant,
            typed_proof_variant,
            statement_digest,
            envelope_sha256,
        ) = typed_envelope_rows[expected_index];
        if typed_label != matrix_label
            || typed_statement_variant != statement_variant
            || typed_proof_variant != proof_variant
        {
            return Err(format!(
                "exact12 typed-envelope route {expected_index} does not match its protocol row"
            ));
        }
        if !is_canonical_nonzero_sha256_hex(statement_digest)
            || !is_canonical_nonzero_sha256_hex(envelope_sha256)
        {
            return Err(format!(
                "exact12 typed-envelope route {expected_index} has a non-canonical digest"
            ));
        }
        registry_preimage.push_str(protocol_id.canonical_label());
        registry_preimage.push('\n');
    }
    let retired_set = retired_labels.iter().copied().collect::<BTreeSet<_>>();
    if retired_set.len() != retired_labels.len() {
        return Err("exact12 matrix repeats a retired protocol label".to_owned());
    }
    if retired_set != PRIVACY_RETIRED_PROTOCOL_LABELS_V1.into_iter().collect() {
        return Err("exact12 matrix does not carry the frozen retired-label set".to_owned());
    }
    for retired_label in retired_set {
        if active_labels.contains(retired_label)
            || PrivacyProtocolIdV1::from_canonical_label(retired_label).is_some()
        {
            return Err(format!(
                "retired protocol label {retired_label:?} is representable by the active registry"
            ));
        }
    }
    let registry_digest = hex::encode(Sha256::digest(registry_preimage.as_bytes()));
    if registry_digest != EXPECTED_REGISTRY_SHA256 {
        return Err(format!(
            "exact12 registry digest mismatch: expected={EXPECTED_REGISTRY_SHA256}, compiled={registry_digest}"
        ));
    }
    let matrix_digest = hex::encode(Sha256::digest(matrix.as_bytes()));
    if matrix_digest != EXPECTED_MATRIX_SHA256 {
        return Err(format!(
            "exact12 matrix artifact digest mismatch: expected={EXPECTED_MATRIX_SHA256}, compiled={matrix_digest}"
        ));
    }
    Ok(registry_digest)
}
fn validate_compiled_profile_row_v1(
    index: usize,
    expected_protocol_id: PrivacyProtocolIdV1,
    profile: &PrivacyCompiledProfileSnapshotV1,
) -> Result<(), String> {
    if profile.protocol_id != expected_protocol_id {
        return Err(format!(
            "compiled profile route {index} is {}, expected {}",
            profile.protocol_id.canonical_label(),
            expected_protocol_id.canonical_label()
        ));
    }
    profile.validate().map_err(|error| {
        format!(
            "compiled profile {} is not a valid closed snapshot: {error}",
            expected_protocol_id.canonical_label()
        )
    })?;
    let authoritative = compiled_privacy_profile_v1(expected_protocol_id)
        .map(PrivacyCompiledProfileSnapshotV1::from)
        .map_err(|error| {
            format!(
                "authoritative compiled profile {} is unavailable: {error}",
                expected_protocol_id.canonical_label()
            )
        })?;
    if *profile != authoritative {
        return Err(format!(
            "compiled profile route {index} for {} does not exactly match the authoritative compiled bindings",
            expected_protocol_id.canonical_label()
        ));
    }
    Ok(())
}
fn validate_compiled_profiles_v1(
    profiles: &[PrivacyCompiledProfileSnapshotV1],
) -> Result<String, String> {
    if profiles.len() != EXPECTED_PROFILE_COUNT {
        return Err(format!(
            "compiled profile report is not exact12: expected={EXPECTED_PROFILE_COUNT}, actual={}",
            profiles.len()
        ));
    }
    for (index, (expected_protocol_id, profile)) in PrivacyProtocolIdV1::ALL
        .into_iter()
        .zip(profiles)
        .enumerate()
    {
        validate_compiled_profile_row_v1(index, expected_protocol_id, profile)?;
    }
    let encoded = norito::to_bytes(&profiles.to_vec())
        .map_err(|error| format!("compiled profile rows are not canonically encodable: {error}"))?;
    Ok(hex::encode(Sha256::digest(encoded)))
}
fn build_release_report_v1(
    registry_digest: &str,
    profiles: &[PrivacyCompiledProfileSnapshotV1],
) -> Result<String, String> {
    let compiled_profiles_digest = validate_compiled_profiles_v1(profiles)?;
    let mut report = norito::json::Map::new();
    report.insert("schema_version".into(), norito::json::Value::from(1_u64));
    report.insert(
        "compiled_profile_count".into(),
        norito::json::Value::from(
            u64::try_from(profiles.len()).expect("exact12 profile count fits u64"),
        ),
    );
    report.insert(
        "compiled_profiles".into(),
        norito::json::to_value(&profiles.to_vec())
            .map_err(|error| format!("compiled profile rows are not JSON encodable: {error}"))?,
    );
    report.insert(
        "compiled_profiles_sha256".into(),
        norito::json::Value::from(compiled_profiles_digest),
    );
    report.insert(
        "typed_envelope_schema_kat_count".into(),
        norito::json::Value::from(
            u64::try_from(EXPECTED_PROFILE_COUNT).expect("exact12 count fits u64"),
        ),
    );
    report.insert(
        "retired_label_count".into(),
        norito::json::Value::from(
            u64::try_from(PRIVACY_RETIRED_PROTOCOL_LABELS_V1.len())
                .expect("retired label count fits u64"),
        ),
    );
    report.insert(
        "exact12_registry_sha256".into(),
        norito::json::Value::from(registry_digest.to_owned()),
    );
    report.insert(
        "exact12_matrix_sha256".into(),
        norito::json::Value::from(EXPECTED_MATRIX_SHA256),
    );
    norito::json::to_json(&norito::json::Value::Object(report))
        .map_err(|error| format!("privacy release report is not JSON encodable: {error}"))
}
fn run() -> Result<(), String> {
    let registry_digest = validate_exact12_matrix_v1(EXACT12_MATRIX)?;
    let profiles = PrivacyProtocolIdV1::ALL
        .into_iter()
        .map(|protocol_id| {
            compiled_privacy_profile_v1(protocol_id)
                .map(PrivacyCompiledProfileSnapshotV1::from)
                .map_err(|error| {
                    format!(
                        "compiled profile {} is unavailable: {error}",
                        protocol_id.canonical_label()
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    println!("{}", build_release_report_v1(&registry_digest, &profiles)?);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn assert_rejected(matrix: &str, expected: &str) {
        let error = validate_exact12_matrix_v1(matrix).expect_err("mutation must be rejected");
        assert!(
            error.contains(expected),
            "expected {expected:?} in rejection, got {error:?}"
        );
    }
    fn without_first_line_starting_with(matrix: &str, prefix: &str) -> String {
        let mut removed = false;
        let mut output = String::new();
        for line in matrix.lines() {
            if !removed && line.starts_with(prefix) {
                removed = true;
                continue;
            }
            output.push_str(line);
            output.push('\n');
        }
        assert!(removed, "fixture must contain line prefix {prefix:?}");
        output
    }
    #[test]
    fn frozen_matrix_shape_is_accepted_before_compiled_profile_checks() {
        assert_eq!(
            validate_exact12_matrix_v1(EXACT12_MATRIX).expect("frozen matrix"),
            EXPECTED_REGISTRY_SHA256
        );
    }
    #[test]
    fn noncanonical_text_framing_is_rejected() {
        assert_rejected(
            EXACT12_MATRIX.trim_end_matches('\n'),
            "canonical LF-delimited",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen('\n', "\r\n", 1),
            "canonical LF-delimited",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen("matrix-version\t1\n", "matrix-version\t1\n\n", 1),
            "canonical LF-delimited",
        );
    }
    #[test]
    fn duplicate_headers_and_bad_indices_are_rejected() {
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "matrix-version\t1\n",
                "matrix-version\t1\nmatrix-version\t1\n",
                1,
            ),
            "repeats its version",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "registry-sha256\t",
                "registry-sha256\tdeadbeef\nregistry-sha256\t",
                1,
            ),
            "repeats its registry digest",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen("protocol\t0\t", "protocol\tnot-decimal\t", 1),
            "bad index",
        );
    }
    #[test]
    fn reordered_or_missing_exact12_routes_are_rejected() {
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "protocol\t0\tzk-ace-pq-authorization-v0",
                "protocol\t0\tanonymous-pgc-k-out-of-n-v1",
                1,
            ),
            "does not match the compiled registry",
        );
        assert_rejected(
            &without_first_line_starting_with(EXACT12_MATRIX, "protocol\t11\tpq-masp-stark-v0"),
            "privacy registry is not exact12",
        );
        assert_rejected(
            &without_first_line_starting_with(EXACT12_MATRIX, "typed-envelope\tpq-masp-stark-v0"),
            "privacy registry is not exact12",
        );
    }
    #[test]
    fn typed_envelope_variant_and_digest_mutations_are_rejected() {
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "typed-envelope\tzk-ace-pq-authorization-v0\tZkAcePqAuthorizationV0",
                "typed-envelope\tzk-ace-pq-authorization-v0\tAnonymousPgcKOutOfNV1",
                1,
            ),
            "does not match its protocol row",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "0c322637967ee3593f91bf38cb22e1e53b6da82146dbe242e38664b4c4c450a9",
                "0C322637967ee3593f91bf38cb22e1e53b6da82146dbe242e38664b4c4c450a9",
                1,
            ),
            "non-canonical digest",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "0c322637967ee3593f91bf38cb22e1e53b6da82146dbe242e38664b4c4c450a9",
                "0000000000000000000000000000000000000000000000000000000000000000",
                1,
            ),
            "non-canonical digest",
        );
    }
    #[test]
    fn retired_protocol_set_is_exact_unique_and_unrepresentable() {
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "retired\tsis-with-hints\n",
                "retired\tsis-with-hints\nretired\tsis-with-hints\n",
                1,
            ),
            "repeats a retired protocol label",
        );
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "retired\tsis-with-hints\n",
                "retired\tsis-with-hints-v2\n",
                1,
            ),
            "frozen retired-label set",
        );
    }
    #[test]
    fn retired_matrix_rows_match_the_shared_data_model_reservation_in_order() {
        let matrix_retired = EXACT12_MATRIX
            .lines()
            .filter_map(|line| line.strip_prefix("retired\t"))
            .collect::<Vec<_>>();
        assert_eq!(
            matrix_retired.as_slice(),
            PRIVACY_RETIRED_PROTOCOL_LABELS_V1.as_slice()
        );
        for label in matrix_retired {
            assert!(privacy_protocol_label_is_reserved_v1(label));
            assert!(PrivacyProtocolIdV1::from_canonical_label(label).is_none());
        }
    }
    #[test]
    fn frozen_registry_digest_is_independently_recomputed() {
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                EXPECTED_REGISTRY_SHA256,
                "034eafb58f0c54f5319b9cc26557920e564453f689071931393dcdba91123e51",
                1,
            ),
            "frozen first-release registry digest",
        );
    }
    #[test]
    fn valid_looking_typed_kat_substitution_breaks_the_frozen_artifact_digest() {
        assert_rejected(
            &EXACT12_MATRIX.replacen(
                "0c322637967ee3593f91bf38cb22e1e53b6da82146dbe242e38664b4c4c450a9",
                "1c322637967ee3593f91bf38cb22e1e53b6da82146dbe242e38664b4c4c450a9",
                1,
            ),
            "matrix artifact digest mismatch",
        );
    }
    #[test]
    fn compiled_profile_report_rejects_missing_duplicate_and_reordered_routes() {
        let first = PrivacyCompiledProfileSnapshotV1::from(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
                .expect("ZK-ACE is compiled for this required-feature binary"),
        );
        assert!(
            validate_compiled_profiles_v1(&vec![first; EXPECTED_PROFILE_COUNT - 1])
                .expect_err("missing route must reject")
                .contains("not exact12")
        );
        let duplicate = vec![first; EXPECTED_PROFILE_COUNT];
        assert!(
            validate_compiled_profiles_v1(&duplicate)
                .expect_err("duplicate route must reject")
                .contains("route 1")
        );
        let second = PrivacyCompiledProfileSnapshotV1::from(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
                .expect("PGC is compiled"),
        );
        let reordered = [second, first];
        assert!(
            validate_compiled_profile_row_v1(
                0,
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                &reordered[0],
            )
            .expect_err("reordered route must reject")
            .contains("route 0")
        );
    }
    #[test]
    fn compiled_profile_report_rejects_each_consensus_binding_mutation() {
        let profile = PrivacyCompiledProfileSnapshotV1::from(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
                .expect("ZK-ACE is compiled for this required-feature binary"),
        );
        validate_compiled_profile_row_v1(0, PrivacyProtocolIdV1::ZkAcePqAuthorizationV0, &profile)
            .expect("canonical row");
        let mutations: [fn(&mut PrivacyCompiledProfileSnapshotV1); 5] = [
            |row| row.parameter_id.0[0] ^= 1,
            |row| row.parameter_digest.0[0] ^= 1,
            |row| row.verifier_digest.0[0] ^= 1,
            |row| row.statement_schema_digest.0[0] ^= 1,
            |row| row.engine_manifest_digest.0[0] ^= 1,
        ];
        for mutate in mutations {
            let mut changed = profile;
            mutate(&mut changed);
            assert!(
                validate_compiled_profile_row_v1(
                    0,
                    PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                    &changed,
                )
                .is_err(),
                "mutated consensus binding was accepted"
            );
        }
    }
}
