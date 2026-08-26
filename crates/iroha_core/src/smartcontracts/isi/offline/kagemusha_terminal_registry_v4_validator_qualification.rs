use super::VerifiedKagemushaV4RuntimeEffectiveConfigV1;
use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    isi::SetParameter,
    offline::{
        KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT,
        KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS,
        KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES,
        KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION, KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES,
        KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS,
        KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA, KagemushaExactBytesDigestV1,
        KagemushaV4PromotionBindingV1, KagemushaV4PromotionReservationV1,
        KagemushaV4RuntimeEffectiveConfigProjectionV1, KagemushaV4ValidatorQualificationSealBodyV1,
        KagemushaV4ValidatorQualificationSealV1, OfflineDeviceAttestationPolicy,
    },
    parameter::{
        Parameter,
        system::{ConsensusHandshakeMetadata, SumeragiConsensusMode, consensus_metadata},
    },
    peer::PeerId,
    transaction::Executable,
};

/// Trusted promotion-controller input for one validator qualification attempt.
///
/// The protected promotion controller is responsible for the exact signed
/// reservation, `promotion_id`, and policy. This type deliberately accepts no
/// release, catalog, executable, configuration, genesis, network, or
/// execution-policy digest; those identities are derived from authenticated
/// runtime objects.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaV4ValidatorQualificationSubjectV1 {
    promotion_controller: PublicKey,
    promotion_reservation: KagemushaExactBytesDigestV1,
    promotion_id: [u8; 32],
    manifest_sha256: [u8; 32],
    device_attestation_policy: OfflineDeviceAttestationPolicy,
    catalog_revalidation_issued_at_unix_ms: u64,
    validator_qualification_expires_at_unix_ms: u64,
}

impl KagemushaV4ValidatorQualificationSubjectV1 {
    /// Validate and retain one protected promotion-controller subject.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero identifier or manifest selector, or when
    /// the governed device policy is not activation eligible at
    /// `evaluation_time_ms`.
    fn try_new(
        promotion_controller: PublicKey,
        promotion_reservation: KagemushaExactBytesDigestV1,
        promotion_id: [u8; 32],
        manifest_sha256: [u8; 32],
        device_attestation_policy: OfflineDeviceAttestationPolicy,
        evaluation_time_ms: u64,
        catalog_revalidation_issued_at_unix_ms: u64,
        validator_qualification_expires_at_unix_ms: u64,
    ) -> Result<Self, String> {
        if !matches!(
            promotion_controller.try_algorithm(),
            Ok(iroha_crypto::Algorithm::Ed25519)
        ) {
            return Err("Kagemusha V4 promotion controller must be Ed25519".to_owned());
        }
        if promotion_reservation.byte_len == 0 || promotion_reservation.sha256 == [0; 32] {
            return Err("Kagemusha V4 promotion reservation identity must be nonzero".to_owned());
        }
        if promotion_id == [0; 32] {
            return Err("Kagemusha V4 promotion id must be nonzero".to_owned());
        }
        if manifest_sha256 == [0; 32] {
            return Err("Kagemusha V4 promotion manifest selector must be nonzero".to_owned());
        }
        super::isi::validate_offline_attestation_policy_for_release_activation(
            &device_attestation_policy,
            evaluation_time_ms,
        )
        .map_err(|error| format!("invalid Kagemusha V4 promotion device policy: {error}"))?;
        norito::encode_canonical(&device_attestation_policy).map_err(|error| {
            format!("failed to encode the canonical Kagemusha V4 device policy: {error}")
        })?;
        if catalog_revalidation_issued_at_unix_ms == 0
            || catalog_revalidation_issued_at_unix_ms >= validator_qualification_expires_at_unix_ms
            || validator_qualification_expires_at_unix_ms <= evaluation_time_ms
            || validator_qualification_expires_at_unix_ms.saturating_sub(evaluation_time_ms)
                > KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS
        {
            return Err(
                "Kagemusha V4 validator-qualification receipt lifetime is invalid".to_owned(),
            );
        }
        // Qualification expiry is inclusive at the signing-clock boundary,
        // while Android status freshness is half-open. Convert the inclusive
        // millisecond to the exclusive endpoint expected by the shared policy
        // helper so a seal cannot be signed at the first stale millisecond.
        let status_coverage_exclusive_end_ms = validator_qualification_expires_at_unix_ms
            .checked_add(1)
            .ok_or_else(|| {
                "Kagemusha V4 validator-qualification expiry overflows Android status coverage"
                    .to_owned()
            })?;
        super::isi::validate_offline_attestation_policy_status_coverage(
            &device_attestation_policy,
            status_coverage_exclusive_end_ms,
        )
        .map_err(|error| {
            format!("Kagemusha V4 promotion Android status coverage is invalid: {error}")
        })?;
        Ok(Self {
            promotion_controller,
            promotion_reservation,
            promotion_id,
            manifest_sha256,
            device_attestation_policy,
            catalog_revalidation_issued_at_unix_ms,
            validator_qualification_expires_at_unix_ms,
        })
    }

    /// Independently pinned promotion-controller identity.
    #[must_use]
    const fn promotion_controller(&self) -> &PublicKey {
        &self.promotion_controller
    }

    /// Exact canonical controller-signed reservation identity.
    #[must_use]
    const fn promotion_reservation(&self) -> KagemushaExactBytesDigestV1 {
        self.promotion_reservation
    }

    /// Exact protected promotion-run identity.
    #[must_use]
    #[cfg(test)]
    const fn promotion_id(&self) -> [u8; 32] {
        self.promotion_id
    }

    /// Canonical governed device-attestation policy selected by the controller.
    #[must_use]
    #[cfg(test)]
    const fn device_attestation_policy(&self) -> &OfflineDeviceAttestationPolicy {
        &self.device_attestation_policy
    }

    /// Catalog-revalidation receipt issuance time.
    #[must_use]
    const fn catalog_revalidation_issued_at_unix_ms(&self) -> u64 {
        self.catalog_revalidation_issued_at_unix_ms
    }

    /// Signed deadline for invoking the validator signer.
    #[must_use]
    const fn validator_qualification_expires_at_unix_ms(&self) -> u64 {
        self.validator_qualification_expires_at_unix_ms
    }
}

const KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_SCHEMA_V1: &str =
    "iroha.kagemusha.ios.app_attest_catalog_revalidation_receipt.v1";
const KAGEMUSHA_V4_CATALOG_REVALIDATION_BINDING_SCHEMA_V1: &str =
    "iroha.kagemusha.ios.app_attest_catalog_revalidation_binding.v1";
const KAGEMUSHA_V4_CATALOG_REVALIDATION_STATUS_V1: &str = "catalog-revalidated-for-one-promotion";
const KAGEMUSHA_V4_CATALOG_REVALIDATION_APPLE_STATUS_SOURCE_V1: &str =
    "apple-app-attest-online-status-authority-v1";
const ED25519_SUBJECT_PUBLIC_KEY_INFO_DER_PREFIX_V1: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];
const KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_FIELDS_V1: [&str; 14] = [
    "catalog_sha256",
    "expires_at_unix_ms",
    "issued_at_unix_ms",
    "promotion_id",
    "receipt_id",
    "release_statuses",
    "schema",
    "signature",
    "signature_algorithm",
    "signature_payload_sha256",
    "signer_key_id",
    "signer_public_key_sha256",
    "status",
    "version",
];
const KAGEMUSHA_V4_CATALOG_REVALIDATION_RELEASE_STATUS_FIELDS_V1: [&str; 9] = [
    "app_attest_key_id",
    "apple_status",
    "apple_status_checked_at_unix_ms",
    "apple_status_source",
    "consumption_receipt_sha256",
    "evidence_sha256",
    "refreshed_apple_receipt_sha256",
    "release_manifest_sha256",
    "risk_metric",
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaV4CatalogRevalidationReceiptFactsV1 {
    issued_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    release_manifest_sha256: Vec<[u8; 32]>,
}

fn json_object_has_exact_fields_v1(object: &norito::json::Map, fields: &[&str]) -> bool {
    object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
}

fn json_required_string_v1<'a>(
    object: &'a norito::json::Map,
    field: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| {
            format!("Kagemusha catalog-revalidation receipt field `{field}` must be a string")
        })
}

fn json_required_positive_u64_v1(object: &norito::json::Map, field: &str) -> Result<u64, String> {
    object
        .get(field)
        .and_then(norito::json::Value::as_u64)
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            format!(
                "Kagemusha catalog-revalidation receipt field `{field}` must be a positive integer"
            )
        })
}

fn parse_lowercase_sha256_v1(value: &str, field: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || value.bytes().all(|byte| byte == b'0')
    {
        return Err(format!(
            "Kagemusha catalog-revalidation receipt field `{field}` must be nonzero lowercase SHA-256"
        ));
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(value, &mut digest).map_err(|_| {
        format!("Kagemusha catalog-revalidation receipt field `{field}` is not lowercase SHA-256")
    })?;
    Ok(digest)
}

fn json_required_sha256_v1(object: &norito::json::Map, field: &str) -> Result<[u8; 32], String> {
    parse_lowercase_sha256_v1(json_required_string_v1(object, field)?, field)
}

fn valid_catalog_revalidation_key_id_v1(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'.' | b'_' | b'-'))
}

fn catalog_revalidation_authority_spki_sha256_v1(
    authority: &PublicKey,
) -> Result<[u8; 32], String> {
    let (algorithm, raw_public_key) = authority.try_to_bytes().map_err(|error| {
        format!("invalid Kagemusha catalog-revalidation authority public key: {error}")
    })?;
    if algorithm != Algorithm::Ed25519 || raw_public_key.len() != 32 {
        return Err(
            "Kagemusha catalog-revalidation authority public key must be Ed25519".to_owned(),
        );
    }
    let mut hasher = Sha256::new();
    hasher.update(ED25519_SUBJECT_PUBLIC_KEY_INFO_DER_PREFIX_V1);
    hasher.update(raw_public_key);
    Ok(hasher.finalize().into())
}

fn catalog_revalidation_signature_payload_v1(
    object: &norito::json::Map,
) -> Result<Vec<u8>, String> {
    let mut unsigned = object.clone();
    unsigned.remove("signature");
    unsigned.remove("signature_payload_sha256");
    // Every accepted string in this schema is ASCII and every number is an
    // integer, so Norito's compact BTreeMap encoding is byte-identical to the
    // Python producer's sort_keys/separators/ensure_ascii canonical payload.
    norito::json::to_string(&norito::json::Value::Object(unsigned))
        .map(String::into_bytes)
        .map_err(|error| {
            format!("failed to encode Kagemusha catalog receipt signature payload: {error}")
        })
}

fn validate_catalog_revalidation_authority_v1(
    object: &norito::json::Map,
    signature_payload: &[u8],
    trusted_authority_key_id: &str,
    trusted_authority_public_key: &PublicKey,
) -> Result<(), String> {
    if !valid_catalog_revalidation_key_id_v1(trusted_authority_key_id) {
        return Err(
            "configured Kagemusha catalog-revalidation authority key id is invalid".to_owned(),
        );
    }
    if json_required_string_v1(object, "signer_key_id")? != trusted_authority_key_id {
        return Err(
            "Kagemusha catalog-revalidation receipt signer key id is not the configured authority"
                .to_owned(),
        );
    }
    let expected_spki_sha256 =
        catalog_revalidation_authority_spki_sha256_v1(trusted_authority_public_key)?;
    if json_required_sha256_v1(object, "signer_public_key_sha256")? != expected_spki_sha256 {
        return Err(
            "Kagemusha catalog-revalidation receipt signer SPKI digest is not the configured authority"
                .to_owned(),
        );
    }
    if json_required_string_v1(object, "signature_algorithm")? != "ed25519" {
        return Err(
            "Kagemusha catalog-revalidation receipt signature algorithm must be ed25519".to_owned(),
        );
    }
    let signature_text = json_required_string_v1(object, "signature")?;
    if signature_text.len() != 128
        || !signature_text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(
            "Kagemusha catalog-revalidation receipt signature must be 64 lowercase-hex bytes"
                .to_owned(),
        );
    }
    let mut signature_bytes = [0_u8; 64];
    hex::decode_to_slice(signature_text, &mut signature_bytes).map_err(|_| {
        "Kagemusha catalog-revalidation receipt signature is not lowercase hex".to_owned()
    })?;
    let signature = iroha_crypto::ed25519_parse_signature(&signature_bytes).map_err(|error| {
        format!("invalid Kagemusha catalog-revalidation authority signature: {error}")
    })?;
    signature
        .verify(trusted_authority_public_key, signature_payload)
        .map_err(|error| {
            format!("invalid Kagemusha catalog-revalidation authority signature: {error}")
        })
}

fn canonical_json_bytes_v1(value: &norito::json::Value) -> Result<Vec<u8>, String> {
    let mut canonical = norito::json::to_string(value)
        .map_err(|error| format!("failed to encode canonical Kagemusha JSON: {error}"))?
        .into_bytes();
    canonical.push(b'\n');
    Ok(canonical)
}

fn validate_exact_catalog_revalidation_receipt_v1(
    reservation: &KagemushaV4PromotionReservationV1,
    exact_receipt_json: &[u8],
    trusted_authority_key_id: &str,
    trusted_authority_public_key: &PublicKey,
) -> Result<KagemushaV4CatalogRevalidationReceiptFactsV1, String> {
    if exact_receipt_json.is_empty()
        || exact_receipt_json.len() > KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_MAX_BYTES
        || !reservation
            .body
            .catalog_revalidation_receipt_json
            .matches_bytes(exact_receipt_json)
    {
        return Err(
            "exact Kagemusha catalog-revalidation receipt is absent, oversized, or differs from the signed reservation"
                .to_owned(),
        );
    }
    let value: norito::json::Value = norito::json::from_slice(exact_receipt_json)
        .map_err(|error| format!("invalid Kagemusha catalog-revalidation receipt JSON: {error}"))?;
    if canonical_json_bytes_v1(&value)? != exact_receipt_json {
        return Err(
            "Kagemusha catalog-revalidation receipt JSON is not exact canonical sorted-key JSON plus LF"
                .to_owned(),
        );
    }
    let norito::json::Value::Object(object) = &value else {
        return Err("Kagemusha catalog-revalidation receipt must be a JSON object".to_owned());
    };
    if !json_object_has_exact_fields_v1(
        object,
        &KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_FIELDS_V1,
    ) {
        return Err("Kagemusha catalog-revalidation receipt fields are not exact".to_owned());
    }
    if json_required_string_v1(object, "schema")?
        != KAGEMUSHA_V4_CATALOG_REVALIDATION_RECEIPT_SCHEMA_V1
        || object.get("version").and_then(norito::json::Value::as_u64)
            != Some(u64::from(KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION))
    {
        return Err("Kagemusha catalog-revalidation receipt schema/version is invalid".to_owned());
    }
    let promotion_id = json_required_sha256_v1(object, "promotion_id")?;
    let receipt_id = json_required_sha256_v1(object, "receipt_id")?;
    if promotion_id != reservation.body.promotion_id || receipt_id == promotion_id {
        return Err(
            "Kagemusha catalog-revalidation receipt promotion/receipt identity is invalid"
                .to_owned(),
        );
    }
    let catalog_sha256 = json_required_sha256_v1(object, "catalog_sha256")?;
    if catalog_sha256 != reservation.body.catalog_revalidation_catalog_sha256 {
        return Err(
            "Kagemusha App-Attest revalidation catalog digest differs from the signed reservation (it is distinct from the consensus-policy digest)"
                .to_owned(),
        );
    }
    let issued_at_unix_ms = json_required_positive_u64_v1(object, "issued_at_unix_ms")?;
    let expires_at_unix_ms = json_required_positive_u64_v1(object, "expires_at_unix_ms")?;
    if issued_at_unix_ms >= expires_at_unix_ms
        || expires_at_unix_ms.saturating_sub(issued_at_unix_ms)
            > KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS
        || expires_at_unix_ms != reservation.body.validator_qualification_expires_at_unix_ms
        || issued_at_unix_ms
            > reservation
                .body
                .policy_evaluation_time_ms
                .saturating_add(KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS)
        || reservation.body.policy_evaluation_time_ms >= expires_at_unix_ms
    {
        return Err(
            "Kagemusha catalog-revalidation receipt issuance, evaluation, or signed qualification expiry is invalid"
                .to_owned(),
        );
    }
    if json_required_string_v1(object, "status")? != KAGEMUSHA_V4_CATALOG_REVALIDATION_STATUS_V1 {
        return Err("Kagemusha catalog-revalidation receipt status is invalid".to_owned());
    }
    let signature_payload_sha256 = json_required_sha256_v1(object, "signature_payload_sha256")?;
    let signature_payload = catalog_revalidation_signature_payload_v1(object)?;
    if <[u8; 32]>::from(Sha256::digest(&signature_payload)) != signature_payload_sha256 {
        return Err(
            "Kagemusha catalog-revalidation receipt signature payload digest is invalid".to_owned(),
        );
    }
    validate_catalog_revalidation_authority_v1(
        object,
        &signature_payload,
        trusted_authority_key_id,
        trusted_authority_public_key,
    )?;

    let statuses = object
        .get("release_statuses")
        .and_then(norito::json::Value::as_array)
        .filter(|statuses| {
            !statuses.is_empty()
                && statuses.len() <= KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_RELEASES_V1
        })
        .ok_or_else(|| {
            "Kagemusha catalog-revalidation release statuses have invalid cardinality".to_owned()
        })?;
    let mut release_manifest_sha256 = Vec::with_capacity(statuses.len());
    let mut prior_manifest = None;
    let mut evidence_digests = std::collections::BTreeSet::new();
    let mut consumption_digests = std::collections::BTreeSet::new();
    let mut catalog_bindings = Vec::with_capacity(statuses.len());
    for (index, status) in statuses.iter().enumerate() {
        let norito::json::Value::Object(status) = status else {
            return Err(format!(
                "Kagemusha catalog-revalidation release status {index} must be an object"
            ));
        };
        if !json_object_has_exact_fields_v1(
            status,
            &KAGEMUSHA_V4_CATALOG_REVALIDATION_RELEASE_STATUS_FIELDS_V1,
        ) {
            return Err(format!(
                "Kagemusha catalog-revalidation release status {index} fields are not exact"
            ));
        }
        let manifest = json_required_sha256_v1(status, "release_manifest_sha256")?;
        let evidence = json_required_sha256_v1(status, "evidence_sha256")?;
        let consumption = json_required_sha256_v1(status, "consumption_receipt_sha256")?;
        if prior_manifest.is_some_and(|prior| prior >= manifest)
            || !evidence_digests.insert(evidence)
            || !consumption_digests.insert(consumption)
        {
            return Err(
                "Kagemusha catalog-revalidation releases are unordered or reuse evidence"
                    .to_owned(),
            );
        }
        prior_manifest = Some(manifest);
        release_manifest_sha256.push(manifest);
        let app_attest_key_id = json_required_string_v1(status, "app_attest_key_id")?;
        if app_attest_key_id.is_empty()
            || app_attest_key_id.len() > 1024
            || !app_attest_key_id.is_ascii()
        {
            return Err(format!(
                "Kagemusha catalog-revalidation release status {index} App Attest key id is invalid"
            ));
        }
        let checked_at = json_required_positive_u64_v1(status, "apple_status_checked_at_unix_ms")?;
        if json_required_string_v1(status, "apple_status")? != "good"
            || json_required_string_v1(status, "apple_status_source")?
                != KAGEMUSHA_V4_CATALOG_REVALIDATION_APPLE_STATUS_SOURCE_V1
            || checked_at
                > issued_at_unix_ms
                    .saturating_add(KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS)
            || issued_at_unix_ms
                > checked_at.saturating_add(KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_MAX_LIFETIME_MS)
        {
            return Err(format!(
                "Kagemusha catalog-revalidation release status {index} is not fresh and good"
            ));
        }
        json_required_sha256_v1(status, "refreshed_apple_receipt_sha256")?;
        if status
            .get("risk_metric")
            .and_then(norito::json::Value::as_u64)
            .is_none_or(|risk| risk > 0x7fff_ffff)
        {
            return Err(format!(
                "Kagemusha catalog-revalidation release status {index} risk metric is invalid"
            ));
        }
        let mut binding = norito::json::Map::new();
        for field in [
            "consumption_receipt_sha256",
            "evidence_sha256",
            "release_manifest_sha256",
        ] {
            binding.insert(
                field.to_owned(),
                norito::json::Value::String(json_required_string_v1(status, field)?.to_owned()),
            );
        }
        catalog_bindings.push(norito::json::Value::Object(binding));
    }
    let mut catalog = norito::json::Map::new();
    catalog.insert(
        "releases".to_owned(),
        norito::json::Value::Array(catalog_bindings),
    );
    catalog.insert(
        "schema".to_owned(),
        norito::json::Value::String(KAGEMUSHA_V4_CATALOG_REVALIDATION_BINDING_SCHEMA_V1.to_owned()),
    );
    catalog.insert(
        "version".to_owned(),
        norito::json::Value::from(u64::from(KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION)),
    );
    let canonical_catalog = canonical_json_bytes_v1(&norito::json::Value::Object(catalog))?;
    if <[u8; 32]>::from(Sha256::digest(canonical_catalog)) != catalog_sha256 {
        return Err(
            "Kagemusha catalog-revalidation receipt catalog digest is internally inconsistent"
                .to_owned(),
        );
    }
    Ok(KagemushaV4CatalogRevalidationReceiptFactsV1 {
        issued_at_unix_ms,
        expires_at_unix_ms,
        release_manifest_sha256,
    })
}

fn validate_validator_qualification_freshness_at_v1(
    subject: &KagemushaV4ValidatorQualificationSubjectV1,
    current_time_ms: u64,
) -> Result<(), String> {
    if current_time_ms == 0
        || subject.catalog_revalidation_issued_at_unix_ms()
            > current_time_ms.saturating_add(KAGEMUSHA_V4_CATALOG_REVALIDATION_MAX_CLOCK_SKEW_MS)
        || current_time_ms > subject.validator_qualification_expires_at_unix_ms()
    {
        return Err(
            "Kagemusha validator qualification is outside the signed catalog-revalidation freshness window"
                .to_owned(),
        );
    }
    Ok(())
}

fn current_unix_time_ms_v1() -> Result<u64, String> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes the Unix epoch: {error}"))?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| "current Unix time does not fit u64 milliseconds".to_owned())
}

/// Opaque pairing of one fully qualified catalog and the seal created from the
/// same authenticated load.
///
/// The private fields prevent a caller from pairing a catalog with an asserted
/// or stale seal before requesting a validator signature.
pub struct KagemushaValidatorQualificationCatalogCaptureV1 {
    catalog: KagemushaReleaseCatalogV4,
    seal: KagemushaCatalogQualificationSealV1,
    policy_path: PathBuf,
    artifact_dir: PathBuf,
}

impl KagemushaReleaseCatalogV4 {
    /// Fully authenticate one catalog and retain its same-load qualification
    /// seal as an opaque validator-qualification capture.
    ///
    /// # Errors
    ///
    /// Returns an error under the same fail-closed conditions as
    /// [`Self::load_and_build_qualification_seal`].
    pub fn load_and_build_validator_qualification_capture(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<KagemushaValidatorQualificationCatalogCaptureV1, String> {
        let (catalog, seal) =
            Self::load_and_build_qualification_seal(policy_path, artifact_dir, max_decoded_bytes)?;
        Ok(KagemushaValidatorQualificationCatalogCaptureV1 {
            catalog,
            seal,
            policy_path: policy_path.to_owned(),
            artifact_dir: artifact_dir.to_owned(),
        })
    }
}

impl KagemushaValidatorQualificationCatalogCaptureV1 {
    /// Borrow the catalog qualification seal for the existing explicit
    /// no-replace catalog publication path.
    #[must_use]
    pub const fn catalog_qualification_seal(&self) -> &KagemushaCatalogQualificationSealV1 {
        &self.seal
    }

    /// Return an Arc-backed shallow clone for disposable genesis validation.
    ///
    /// The capture retains the original catalog/seal pairing for the later
    /// signing boundary. Cloning only the catalog's `Arc`-backed authenticated
    /// releases lets the daemon execute the same load in disposable state
    /// without reloading any filesystem source or consuming this capture.
    #[must_use]
    pub fn catalog_for_validation(&self) -> KagemushaReleaseCatalogV4 {
        self.catalog.clone()
    }

    /// Consume the capture and recover its authenticated catalog.
    #[must_use]
    pub fn into_catalog(self) -> KagemushaReleaseCatalogV4 {
        self.catalog
    }

    /// Construct and sign exactly one validator qualification seal.
    ///
    /// All digests other than the protected promotion id and governed device
    /// policy are derived here from this opaque catalog/seal pairing, the
    /// decoded signed genesis and its exact same-read bytes, and the exact
    /// same-read flattened TOML bytes. No caller-supplied digest is accepted.
    ///
    /// # Errors
    ///
    /// Returns an error if any sealed path became stale, the selected release
    /// is absent or mismatched, genesis bytes are not the canonical bytes of
    /// `genesis`, signed genesis is not permissioned with exactly four
    /// unit-power voters, or `validator_signer` does not match `validator_id`.
    fn build_and_sign_validator_qualification_seal_v1(
        &self,
        subject: &KagemushaV4ValidatorQualificationSubjectV1,
        reservation: &KagemushaV4PromotionReservationV1,
        genesis: &iroha_genesis::GenesisBlock,
        signed_genesis_source: &[u8],
        flattened_toml_config_source: &[u8],
        runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1,
        validator_id: &PeerId,
        validator_signer: &KeyPair,
    ) -> Result<KagemushaV4ValidatorQualificationSealV1, String> {
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            self.seal
                .validate_for_configured_runtime(&self.policy_path, &self.artifact_dir)?;
            verify_kagemusha_catalog_sealed_paths_v1(&self.seal.paths, 0)?;
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (
                subject,
                reservation,
                genesis,
                signed_genesis_source,
                flattened_toml_config_source,
                runtime_effective_config,
                validator_id,
                validator_signer,
            );
            return Err(
                "Kagemusha V4 validator qualification is unsupported on this platform".to_owned(),
            );
        }

        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            let cached = self.catalog.get(&subject.manifest_sha256).ok_or_else(|| {
                "Kagemusha V4 validator qualification release is absent from the authenticated catalog"
                    .to_owned()
            })?;
            let catalog_consensus_policy_digest =
                self.catalog.consensus_policy_digest.ok_or_else(|| {
                    "Kagemusha V4 validator qualification requires a configured catalog identity"
                        .to_owned()
                })?;
            let body = build_validator_qualification_body_from_verified_release_v1(
                &self.seal,
                cached.resolved.release(),
                cached.release_record(),
                cached.qualification_receipt_sha256,
                catalog_consensus_policy_digest,
                subject,
                genesis,
                signed_genesis_source,
                flattened_toml_config_source,
                runtime_effective_config.projection(),
                validator_id,
            )?;
            validate_validator_qualification_body_matches_reservation_v1(&body, reservation)?;

            self.seal
                .validate_for_configured_runtime(&self.policy_path, &self.artifact_dir)?;
            verify_kagemusha_catalog_sealed_paths_v1(&self.seal.paths, 0)?;
            let current_time_ms = current_unix_time_ms_v1()?;
            validate_validator_qualification_freshness_at_v1(subject, current_time_ms)?;
            KagemushaV4ValidatorQualificationSealV1::try_sign(body, validator_signer)
                .map_err(|error| error.to_string())
        }
    }

    /// Authenticate one controller-signed reservation and construct exactly
    /// one validator qualification seal from the same-load catalog.
    ///
    /// Unlike the lower-level subject seam used by same-module tests, this is
    /// the production entry point. It preserves the complete reservation and
    /// verifies every identity derivable by the validator: promoted release
    /// and promotion-record bytes, release-policy source, exact signed genesis,
    /// network and execution policy, catalog policy, and governed device policy.
    /// It also bounds and strict-parses the exact promotion-scoped
    /// catalog-revalidation JSON, verifies its canonical payload digest and
    /// Ed25519 signature against the independently pinned authority key id and
    /// SPKI digest, then checks status, catalog coverage, issuance/expiry, and
    /// the controller-reserved digest. That receipt remains distinct from the
    /// recursive proof receipt.
    ///
    /// # Errors
    ///
    /// Returns an error when the reservation signature/controller is invalid,
    /// any reserved identity differs from authenticated same-load evidence, or
    /// validator qualification/signing fails closed.
    #[allow(clippy::too_many_arguments)]
    pub fn build_and_sign_validator_qualification_from_reservation_v1(
        &self,
        exact_reservation_bytes: &[u8],
        pinned_controller: &PublicKey,
        catalog_revalidation_receipt_json: &[u8],
        catalog_revalidation_authority_key_id: &str,
        catalog_revalidation_authority_public_key: &PublicKey,
        genesis: &iroha_genesis::GenesisBlock,
        signed_genesis_source: &[u8],
        flattened_toml_config_source: &[u8],
        runtime_effective_config: &VerifiedKagemushaV4RuntimeEffectiveConfigV1,
        validator_id: &PeerId,
        validator_signer: &KeyPair,
    ) -> Result<KagemushaV4ValidatorQualificationSealV1, String> {
        let reservation = KagemushaV4PromotionReservationV1::decode_and_verify_canonical(
            exact_reservation_bytes,
            pinned_controller,
        )
        .map_err(|error| format!("invalid Kagemusha V4 promotion reservation: {error}"))?;
        if pinned_controller == catalog_revalidation_authority_public_key {
            return Err(
                "Kagemusha catalog-revalidation authority must differ from the promotion controller"
                    .to_owned(),
            );
        }
        let (reservation_identity, catalog_revalidation) =
            validate_exact_kagemusha_promotion_sources_v1(
                &reservation,
                exact_reservation_bytes,
                catalog_revalidation_receipt_json,
                catalog_revalidation_authority_key_id,
                catalog_revalidation_authority_public_key,
            )?;
        if catalog_revalidation.release_manifest_sha256.len() != self.catalog.releases.len()
            || catalog_revalidation
                .release_manifest_sha256
                .iter()
                .copied()
                .zip(self.catalog.releases.keys().copied())
                .any(|(receipt, catalog)| receipt != catalog)
        {
            return Err(
                "Kagemusha catalog-revalidation receipt does not exactly cover the authenticated catalog"
                    .to_owned(),
            );
        }
        let subject = KagemushaV4ValidatorQualificationSubjectV1::try_new(
            pinned_controller.clone(),
            reservation_identity,
            reservation.body.promotion_id,
            reservation.body.manifest_sha256,
            reservation.body.device_attestation_policy.clone(),
            reservation.body.policy_evaluation_time_ms,
            catalog_revalidation.issued_at_unix_ms,
            catalog_revalidation.expires_at_unix_ms,
        )?;
        let cached = self.catalog.get(&subject.manifest_sha256).ok_or_else(|| {
            "Kagemusha V4 reservation release is absent from the authenticated catalog".to_owned()
        })?;
        let catalog_consensus_policy_digest =
            self.catalog.consensus_policy_digest.ok_or_else(|| {
                "Kagemusha V4 reservation requires a configured catalog identity".to_owned()
            })?;
        validate_kagemusha_promotion_reservation_against_verified_release_v1(
            &reservation,
            &self.seal,
            cached.resolved.release(),
            cached.release_record(),
            catalog_consensus_policy_digest,
            genesis,
            signed_genesis_source,
            validator_id,
        )?;
        self.build_and_sign_validator_qualification_seal_v1(
            &subject,
            &reservation,
            genesis,
            signed_genesis_source,
            flattened_toml_config_source,
            runtime_effective_config,
            validator_id,
            validator_signer,
        )
    }
}

fn validate_exact_kagemusha_promotion_sources_v1(
    reservation: &KagemushaV4PromotionReservationV1,
    exact_reservation_bytes: &[u8],
    catalog_revalidation_receipt_json: &[u8],
    catalog_revalidation_authority_key_id: &str,
    catalog_revalidation_authority_public_key: &PublicKey,
) -> Result<
    (
        KagemushaExactBytesDigestV1,
        KagemushaV4CatalogRevalidationReceiptFactsV1,
    ),
    String,
> {
    let canonical_reservation = norito::encode_canonical(reservation).map_err(|error| {
        format!("failed to encode canonical Kagemusha V4 promotion reservation: {error}")
    })?;
    if exact_reservation_bytes.is_empty()
        || exact_reservation_bytes.len() > KAGEMUSHA_V4_PROMOTION_RESERVATION_MAX_BYTES
        || canonical_reservation != exact_reservation_bytes
    {
        return Err(
            "exact Kagemusha V4 promotion reservation is noncanonical or outside its fixed bound"
                .to_owned(),
        );
    }
    let receipt = validate_exact_catalog_revalidation_receipt_v1(
        reservation,
        catalog_revalidation_receipt_json,
        catalog_revalidation_authority_key_id,
        catalog_revalidation_authority_public_key,
    )?;
    let identity = KagemushaExactBytesDigestV1::from_bytes(exact_reservation_bytes)
        .map_err(|error| error.to_string())?;
    Ok((identity, receipt))
}

#[allow(clippy::too_many_arguments)]
fn validate_kagemusha_promotion_reservation_against_verified_release_v1(
    reservation: &KagemushaV4PromotionReservationV1,
    seal: &KagemushaCatalogQualificationSealV1,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    release_record: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    catalog_consensus_policy_digest: [u8; 32],
    genesis: &iroha_genesis::GenesisBlock,
    signed_genesis_source: &[u8],
    validator_id: &PeerId,
) -> Result<(), String> {
    seal.validate_layout()?;
    release_record
        .validate_structure()
        .map_err(|error| format!("invalid authenticated Kagemusha V4 release record: {error}"))?;
    let manifest = authenticated.manifest();
    let release_record_bytes = norito::encode_canonical(release_record)
        .map_err(|error| format!("failed to encode Kagemusha V4 release record: {error}"))?;
    let promotion_record_bytes = norito::encode_canonical(&release_record.promotion_record)
        .map_err(|error| format!("failed to encode Kagemusha V4 promotion record: {error}"))?;
    let reviewed_source_closure_descriptor = manifest
        .reviewed_source_closure
        .canonical_descriptor_bytes()
        .map_err(|error| format!("invalid Kagemusha V4 reviewed source closure: {error}"))?;
    let (network_id, execution_policy_hash) =
        exact_genesis_qualification_identity_v1(genesis, signed_genesis_source, validator_id)?;
    let release_policy_source = exact_sealed_file_identity_v1(
        seal,
        &seal.canonical_policy_path,
        seal.configured_policy_sha256,
        "release policy",
    )?;
    let reserved = &reservation.body;
    if authenticated.manifest_sha256() != reserved.manifest_sha256
        || &release_record.manifest != manifest
        || manifest.network_id != network_id
        || reserved.network_id != network_id
        || !reserved
            .reviewed_source_closure_descriptor
            .matches_bytes(&reviewed_source_closure_descriptor)
        || reserved.reviewed_source_closure_descriptor.sha256
            != manifest.reviewed_source_closure_descriptor_sha256
        || reserved.release_record_sha256 != <[u8; 32]>::from(Sha256::digest(&release_record_bytes))
        || !reserved
            .promotion_record_norito
            .matches_bytes(&promotion_record_bytes)
        || reserved.release_policy_source != release_policy_source
        || !reserved.signed_genesis.matches_bytes(signed_genesis_source)
        || reserved.catalog_consensus_policy_digest != catalog_consensus_policy_digest
        || reserved.execution_policy_hash != execution_policy_hash
    {
        return Err(
            "Kagemusha V4 promotion reservation differs from authenticated validator evidence"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(all(
    test,
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn validate_validator_qualification_matches_reservation_v1(
    seal: &KagemushaV4ValidatorQualificationSealV1,
    reservation: &KagemushaV4PromotionReservationV1,
) -> Result<(), String> {
    seal.verify()
        .map_err(|error| format!("invalid Kagemusha V4 validator qualification: {error}"))?;
    validate_validator_qualification_body_matches_reservation_v1(&seal.body, reservation)
}

fn validate_validator_qualification_body_matches_reservation_v1(
    body: &KagemushaV4ValidatorQualificationSealBodyV1,
    reservation: &KagemushaV4PromotionReservationV1,
) -> Result<(), String> {
    let reserved = &reservation.body;
    let binding = &body.binding;
    let reservation_bytes = norito::encode_canonical(reservation).map_err(|error| {
        format!("failed to encode canonical Kagemusha V4 promotion reservation: {error}")
    })?;
    let reservation_identity = KagemushaExactBytesDigestV1::from_bytes(&reservation_bytes)
        .map_err(|error| error.to_string())?;
    let device_policy_bytes = norito::encode_canonical(&reserved.device_attestation_policy)
        .map_err(|error| format!("failed to encode Kagemusha V4 device policy: {error}"))?;
    if binding.promotion_controller != reserved.promotion_controller
        || binding.promotion_reservation != reservation_identity
        || binding.promotion_id != reserved.promotion_id
        || binding.network_id != reserved.network_id
        || binding.reviewed_source_closure_descriptor_sha256
            != reserved.reviewed_source_closure_descriptor.sha256
        || binding.manifest_sha256 != reserved.manifest_sha256
        || binding.release_record_sha256 != reserved.release_record_sha256
        || binding.release_policy_source != reserved.release_policy_source
        || !binding
            .device_attestation_policy_norito
            .matches_bytes(&device_policy_bytes)
        || binding.signed_genesis != reserved.signed_genesis
        || binding.catalog_consensus_policy_digest != reserved.catalog_consensus_policy_digest
        || binding.execution_policy_hash != reserved.execution_policy_hash
    {
        return Err(
            "Kagemusha V4 signed validator qualification differs from its reservation".to_owned(),
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_validator_qualification_body_from_verified_release_v1(
    seal: &KagemushaCatalogQualificationSealV1,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    release_record: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    qualification_receipt_sha256: [u8; 32],
    catalog_consensus_policy_digest: [u8; 32],
    subject: &KagemushaV4ValidatorQualificationSubjectV1,
    genesis: &iroha_genesis::GenesisBlock,
    signed_genesis_source: &[u8],
    flattened_toml_config_source: &[u8],
    runtime_effective_config: &KagemushaV4RuntimeEffectiveConfigProjectionV1,
    validator_id: &PeerId,
) -> Result<KagemushaV4ValidatorQualificationSealBodyV1, String> {
    seal.validate_layout()?;
    release_record
        .validate_structure()
        .map_err(|error| format!("invalid authenticated Kagemusha V4 release record: {error}"))?;
    if authenticated.manifest_sha256() != subject.manifest_sha256
        || &release_record.manifest != authenticated.manifest()
        || seal.configured_policy_sha256 != authenticated.release_policy_sha256()
    {
        return Err(
            "Kagemusha V4 validator qualification release, policy, or selector mismatch".to_owned(),
        );
    }
    let promotion_bytes = norito::encode_canonical(&release_record.promotion_record)
        .map_err(|error| format!("failed to encode Kagemusha V4 promotion record: {error}"))?;
    let sealed_release = seal
        .releases
        .iter()
        .find(|release| release.manifest_sha256 == subject.manifest_sha256)
        .ok_or_else(|| {
            "Kagemusha V4 catalog qualification seal omits the promoted release".to_owned()
        })?;
    validate_sealed_release_qualification_v1(
        sealed_release,
        authenticated,
        &promotion_bytes,
        qualification_receipt_sha256,
    )?;
    let release_record_bytes = norito::encode_canonical(release_record)
        .map_err(|error| format!("failed to encode Kagemusha V4 release record: {error}"))?;
    let device_policy_bytes = norito::encode_canonical(&subject.device_attestation_policy)
        .map_err(|error| format!("failed to encode Kagemusha V4 device policy: {error}"))?;
    let seal_bytes = seal.canonical_bytes()?;
    let (network_id, execution_policy_hash) =
        exact_genesis_qualification_identity_v1(genesis, signed_genesis_source, validator_id)?;
    validate_runtime_effective_config_against_genesis_v1(
        runtime_effective_config,
        genesis,
        validator_id,
    )?;
    if authenticated.manifest().network_id != network_id {
        return Err(
            "Kagemusha V4 promoted release network differs from exact signed genesis".to_owned(),
        );
    }
    if catalog_consensus_policy_digest == [0; 32] {
        return Err("Kagemusha V4 catalog consensus identity must be nonzero".to_owned());
    }
    Ok(KagemushaV4ValidatorQualificationSealBodyV1 {
        schema: KAGEMUSHA_V4_VALIDATOR_QUALIFICATION_SEAL_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        binding: KagemushaV4PromotionBindingV1 {
            promotion_controller: subject.promotion_controller().clone(),
            promotion_reservation: subject.promotion_reservation(),
            promotion_id: subject.promotion_id,
            network_id,
            reviewed_source_closure_descriptor_sha256: authenticated
                .manifest()
                .reviewed_source_closure_descriptor_sha256,
            manifest_sha256: authenticated.manifest_sha256(),
            release_record_sha256: Sha256::digest(release_record_bytes).into(),
            release_policy_source: exact_sealed_file_identity_v1(
                seal,
                &seal.canonical_policy_path,
                seal.configured_policy_sha256,
                "release policy",
            )?,
            device_attestation_policy_norito: KagemushaExactBytesDigestV1::from_bytes(
                &device_policy_bytes,
            )
            .map_err(|error| error.to_string())?,
            signed_genesis: KagemushaExactBytesDigestV1::from_bytes(signed_genesis_source)
                .map_err(|error| error.to_string())?,
            catalog_consensus_policy_digest,
            execution_policy_hash,
        },
        validator_id: validator_id.clone(),
        iroha3d_executable: exact_sealed_file_identity_v1(
            seal,
            &seal.canonical_executable_path,
            seal.executable_sha256,
            "iroha3d executable",
        )?,
        flattened_toml_config_source: KagemushaExactBytesDigestV1::from_bytes(
            flattened_toml_config_source,
        )
        .map_err(|error| error.to_string())?,
        runtime_effective_config: runtime_effective_config.clone(),
        catalog_qualification_seal: KagemushaExactBytesDigestV1::from_bytes(&seal_bytes)
            .map_err(|error| error.to_string())?,
    })
}

#[cfg(all(
    test,
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[allow(clippy::too_many_arguments)]
fn build_and_sign_validator_qualification_from_verified_release_v1(
    seal: &KagemushaCatalogQualificationSealV1,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    release_record: &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    qualification_receipt_sha256: [u8; 32],
    catalog_consensus_policy_digest: [u8; 32],
    subject: &KagemushaV4ValidatorQualificationSubjectV1,
    genesis: &iroha_genesis::GenesisBlock,
    signed_genesis_source: &[u8],
    flattened_toml_config_source: &[u8],
    runtime_effective_config: &KagemushaV4RuntimeEffectiveConfigProjectionV1,
    validator_id: &PeerId,
    validator_signer: &KeyPair,
) -> Result<KagemushaV4ValidatorQualificationSealV1, String> {
    let body = build_validator_qualification_body_from_verified_release_v1(
        seal,
        authenticated,
        release_record,
        qualification_receipt_sha256,
        catalog_consensus_policy_digest,
        subject,
        genesis,
        signed_genesis_source,
        flattened_toml_config_source,
        runtime_effective_config,
        validator_id,
    )?;
    KagemushaV4ValidatorQualificationSealV1::try_sign(body, validator_signer)
        .map_err(|error| error.to_string())
}

fn exact_sealed_file_identity_v1(
    seal: &KagemushaCatalogQualificationSealV1,
    canonical_path: &str,
    sha256: [u8; 32],
    label: &str,
) -> Result<KagemushaExactBytesDigestV1, String> {
    let sealed = seal
        .paths
        .iter()
        .find(|path| path.canonical_path == canonical_path)
        .ok_or_else(|| format!("Kagemusha V4 qualification seal omits {label}"))?;
    if sealed.kind != KagemushaCatalogSealedPathKindV1::File
        || sealed.stat.length == 0
        || sha256 == [0; 32]
    {
        return Err(format!(
            "Kagemusha V4 qualification seal has an invalid {label} identity"
        ));
    }
    Ok(KagemushaExactBytesDigestV1 {
        byte_len: sealed.stat.length,
        sha256,
    })
}

fn exact_genesis_qualification_identity_v1(
    genesis: &iroha_genesis::GenesisBlock,
    signed_genesis_source: &[u8],
    validator_id: &PeerId,
) -> Result<(NetworkId, Hash), String> {
    let canonical = genesis
        .0
        .encode_wire()
        .map_err(|error| format!("failed to encode canonical signed genesis: {error}"))?;
    if canonical != signed_genesis_source {
        return Err(
            "Kagemusha V4 signed-genesis source is stale or differs from the decoded genesis"
                .to_owned(),
        );
    }
    let voters = crate::sumeragi::signed_genesis_voting_peers(genesis)
        .map_err(|error| format!("invalid signed genesis voting roster: {error}"))?;
    if voters.len() != KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT
        || !voters.iter().any(|peer| peer == validator_id)
    {
        return Err(format!(
            "Kagemusha V4 qualification requires the signer in the exact {KAGEMUSHA_V4_ACTIVATION_VALIDATOR_COUNT}-validator signed genesis roster"
        ));
    }
    let execution_policy_hash = exact_genesis_execution_policy_hash_v1(genesis)?;
    Ok((
        NetworkId::from_genesis_hash(genesis.0.hash()),
        execution_policy_hash,
    ))
}

fn exact_genesis_execution_policy_hash_v1(
    genesis: &iroha_genesis::GenesisBlock,
) -> Result<Hash, String> {
    let metadata = exact_genesis_consensus_metadata_v1(genesis)?;
    if metadata.mode != SumeragiConsensusMode::Permissioned {
        return Err(
            "Kagemusha V4 qualification requires signed permissioned consensus with unit-power validators"
                .to_owned(),
        );
    }
    let hash = Hash::prehashed(metadata.sumeragi_v2.execution_policy_hash);
    if hash == Hash::prehashed([0; Hash::LENGTH]) {
        return Err("Kagemusha V4 execution-policy hash must be nonzero".to_owned());
    }
    Ok(hash)
}

fn exact_genesis_consensus_metadata_v1(
    genesis: &iroha_genesis::GenesisBlock,
) -> Result<ConsensusHandshakeMetadata, String> {
    let mut metadata_entries = Vec::new();
    for transaction in genesis.0.external_transactions() {
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(
                "Kagemusha V4 genesis metadata must be carried by instruction batches".to_owned(),
            );
        };
        for instruction in instructions {
            let Some(set_parameter) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            let Parameter::Custom(custom) = set_parameter.inner() else {
                continue;
            };
            if custom.id() == &consensus_metadata::handshake_meta_id() {
                let metadata: ConsensusHandshakeMetadata = custom
                    .payload()
                    .try_into_any()
                    .map_err(|error| format!("invalid signed consensus metadata: {error}"))?;
                metadata
                    .validate()
                    .map_err(|error| format!("invalid signed consensus metadata: {error}"))?;
                metadata_entries.push(metadata);
            }
        }
    }
    let [metadata] = metadata_entries.as_slice() else {
        return Err(format!(
            "Kagemusha V4 qualification requires exactly one signed consensus metadata entry, found {}",
            metadata_entries.len()
        ));
    };
    Ok(*metadata)
}

fn validate_runtime_effective_config_against_genesis_v1(
    projection: &KagemushaV4RuntimeEffectiveConfigProjectionV1,
    genesis: &iroha_genesis::GenesisBlock,
    validator_id: &PeerId,
) -> Result<(), String> {
    projection.validate().map_err(|error| error.to_string())?;
    let metadata = exact_genesis_consensus_metadata_v1(genesis)?;
    let signed_validators = crate::sumeragi::signed_genesis_validator_pops(genesis)
        .map_err(|error| format!("invalid signed genesis voting authority: {error}"))?;
    let exact_authority = signed_validators.len() == projection.validators.len()
        && signed_validators.iter().zip(&projection.validators).all(
            |((signed_id, signed_pop), projected)| {
                signed_id == &projected.validator_id && signed_pop == &projected.bls_pop
            },
        );
    let genesis_public_key = genesis
        .0
        .external_transactions()
        .next()
        .and_then(|transaction| transaction.authority().try_signatory())
        .ok_or_else(|| "Kagemusha V4 signed genesis has no single-key root authority".to_owned())?;
    if projection.genesis_expected_hash != genesis.0.hash()
        || &projection.genesis_public_key != genesis_public_key
        || projection.genesis_context != metadata.sumeragi_v2
        || !exact_authority
        || !projection
            .validators
            .iter()
            .any(|projected| &projected.validator_id == validator_id)
    {
        return Err(
            "Kagemusha V4 runtime-effective config differs from exact signed genesis".to_owned(),
        );
    }
    Ok(())
}
