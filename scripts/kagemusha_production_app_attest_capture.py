"""Validate a standalone physical-iPhone production App Attest capture."""

from __future__ import annotations

import base64
import hashlib
from pathlib import Path
from typing import Any, Optional


REQUEST_SCHEMA = "iroha.kagemusha.ios.app_attest_capture_request.v1"
CAPTURE_SCHEMA = "iroha.kagemusha.ios.app_attest_physical_capture.v1"
SUMMARY_SCHEMA = "iroha.kagemusha.ios.app_attest_capture_summary.v1"
QUALIFICATION_ATTESTATION_CHALLENGE_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_qualification_attestation_challenge.v1"
)
QUALIFICATION_ASSERTION_CHALLENGE_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_qualification_assertion_challenge.v1"
)
QUALIFICATION_MATERIAL_SCHEMA = (
    "iroha.kagemusha.ios.app_attest_qualification_material.v1"
)
QUALIFICATION_ATTESTATION_CHALLENGE_DOMAIN = (
    "iroha:kagemusha:ios:production:app-attest:qualification:attestation:v1"
)
QUALIFICATION_ASSERTION_CHALLENGE_DOMAIN = (
    "iroha:kagemusha:ios:production:app-attest:qualification:assertion:v1"
)
QUALIFICATION_ATTESTATION_CHALLENGE_FIELDS = frozenset(
    {"schema", "version", "domain", "issued_at_unix_ms", "nonce_base64"}
)
QUALIFICATION_ASSERTION_CHALLENGE_FIELDS = frozenset(
    set(QUALIFICATION_ATTESTATION_CHALLENGE_FIELDS)
    | {"attestation_object_sha256", "key_id"}
)

REQUEST_FIELDS = frozenset(
    {
        "schema",
        "version",
        "attestation_client_data_base64",
        "assertion_client_data_template",
    }
)
CAPTURE_SUCCESS_FIELDS = frozenset(
    {
        "schema",
        "version",
        "status",
        "app_attest_supported",
        "requested_environment",
        "started_at_unix_ms",
        "captured_at_unix_ms",
        "bundle_id",
        "bundle_version",
        "key_id",
        "attestation_client_data_base64",
        "attestation_object_base64",
        "assertion_client_data_base64",
        "assertion_object_base64",
    }
)
CAPTURE_FAILURE_FIELDS = frozenset(
    {
        "schema",
        "version",
        "status",
        "app_attest_supported",
        "requested_environment",
        "started_at_unix_ms",
        "captured_at_unix_ms",
        "bundle_id",
        "bundle_version",
        "error_domain",
        "error_code",
        "error_description",
    }
)


def _exact_fields(
    value: Any, expected: frozenset[str], label: str, errors: list[str]
) -> Optional[dict[str, Any]]:
    if not isinstance(value, dict):
        errors.append(f"{label} must be an object")
        return None
    observed = set(value)
    if observed != expected:
        errors.append(
            f"{label} fields are not exact "
            f"(missing={sorted(expected - observed)}, extra={sorted(observed - expected)})"
        )
        return None
    return value


def _positive_milliseconds(value: Any, label: str, errors: list[str]) -> Optional[int]:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        errors.append(f"{label} must be positive Unix milliseconds")
        return None
    return value


def _extract_attested_public_key(
    attestation_object: bytes, production_module: Any
) -> tuple[bytes, tuple[bytes, ...]]:
    """Extract the P-256 key and certificate chain from strict App Attest CBOR."""

    attestation = production_module._cbor_object(
        production_module._decode_cbor(
            attestation_object, "physical App Attest attestation object"
        ),
        {"fmt", "attStmt", "authData"},
        "physical App Attest attestation object",
    )
    if attestation["fmt"] != "apple-appattest" or not isinstance(
        attestation["authData"], bytes
    ):
        raise ValueError("physical App Attest object has an invalid format")
    statement = production_module._cbor_object(
        attestation["attStmt"],
        {"x5c", "receipt"},
        "physical App Attest attStmt",
    )
    chain = statement["x5c"]
    if (
        not isinstance(chain, tuple)
        or not 2 <= len(chain) <= production_module.MAX_X509_CHAIN_CERTIFICATES
        or any(not isinstance(value, bytes) for value in chain)
    ):
        raise ValueError("physical App Attest certificate chain is not bounded")
    auth_data = attestation["authData"]
    if len(auth_data) < 55:
        raise ValueError("physical App Attest authData is truncated")
    credential_length = int.from_bytes(auth_data[53:55], "big")
    credential_end = 55 + credential_length
    if credential_end > len(auth_data):
        raise ValueError("physical App Attest credential identifier is truncated")
    decoder = production_module._CborDecoder(auth_data[credential_end:])
    cose = production_module._cbor_object(
        decoder.value(), {1, 3, -1, -2, -3}, "physical App Attest COSE key"
    )
    if (
        cose[1] != 2
        or cose[3] != -7
        or cose[-1] != 1
        or not isinstance(cose[-2], bytes)
        or not isinstance(cose[-3], bytes)
        or len(cose[-2]) != 32
        or len(cose[-3]) != 32
    ):
        raise ValueError("physical App Attest COSE key is not P-256 ES256")
    return b"\x04" + cose[-2] + cose[-3], chain


def _validate_capture_challenge_pair(
    attestation_payload: bytes,
    assertion_payload: bytes,
    attestation_object: bytes,
    key_id_text: str,
    policy: dict[str, Any],
    policy_payload: bytes,
    capture_app_code_sign_measurements_sha256: Optional[str],
    candidate_module: Any,
    production_module: Any,
    errors: list[str],
) -> tuple[Optional[str], Optional[int]]:
    """Validate one exact qualification or release-bound challenge pair."""

    try:
        attestation = candidate_module.parse_strict_json(
            attestation_payload, "captured App Attest attestation client data"
        )
        assertion = candidate_module.parse_strict_json(
            assertion_payload, "captured App Attest assertion client data"
        )
    except candidate_module.EvidenceError as error:
        errors.append(str(error))
        return None, None

    schemas = (attestation.get("schema"), assertion.get("schema"))
    if schemas == (
        production_module.ATTESTATION_CHALLENGE_SCHEMA,
        production_module.ASSERTION_CHALLENGE_SCHEMA,
    ):
        evaluated_at = attestation.get("evaluated_at_unix_ms")
        if isinstance(evaluated_at, bool) or not isinstance(evaluated_at, int) or evaluated_at <= 0:
            errors.append(
                "release-bound App Attest challenge evaluated_at_unix_ms must be positive"
            )
            return "production-artifact-bound", None
        policy_sha256 = hashlib.sha256(policy_payload).hexdigest()
        release_manifest_sha256 = attestation.get("release_manifest_sha256")
        if (
            not isinstance(release_manifest_sha256, str)
            or production_module.SHA256_RE.fullmatch(release_manifest_sha256) is None
            or release_manifest_sha256 == "0" * 64
        ):
            errors.append(
                "release-bound App Attest challenge release manifest digest is invalid"
            )
            release_manifest_sha256 = ""
        if capture_app_code_sign_measurements_sha256 is None:
            errors.append(
                "release-bound capture requires prepared capture-app code-sign measurements"
            )
            capture_app_code_sign_measurements_sha256 = ""
        artifact_digests: dict[str, dict[str, Any]] = {}
        for field, artifact in production_module.ARTIFACT_CHALLENGE_BINDINGS.items():
            digest = attestation.get(field)
            if (
                not isinstance(digest, str)
                or production_module.SHA256_RE.fullmatch(digest) is None
                or digest == "0" * 64
            ):
                errors.append(
                    f"release-bound App Attest challenge {field} is not a nonzero SHA-256"
                )
            artifact_digests[artifact] = {"sha256": digest}
        production_module._validate_challenge(
            attestation_payload,
            assertion=False,
            artifact_digests=artifact_digests,
            policy_id=policy.get("policy_id"),
            policy_sha256=policy_sha256,
            release_manifest_sha256=release_manifest_sha256,
            capture_app_code_sign_measurements_sha256=(
                capture_app_code_sign_measurements_sha256
            ),
            evaluated_at_unix_ms=evaluated_at,
            attestation_object_sha256=None,
            key_id=None,
            candidate_module=candidate_module,
            errors=errors,
        )
        production_module._validate_challenge(
            assertion_payload,
            assertion=True,
            artifact_digests=artifact_digests,
            policy_id=policy.get("policy_id"),
            policy_sha256=policy_sha256,
            release_manifest_sha256=release_manifest_sha256,
            capture_app_code_sign_measurements_sha256=(
                capture_app_code_sign_measurements_sha256
            ),
            evaluated_at_unix_ms=evaluated_at,
            attestation_object_sha256=hashlib.sha256(attestation_object).hexdigest(),
            key_id=key_id_text,
            candidate_module=candidate_module,
            errors=errors,
        )
        if attestation.get("nonce_base64") == assertion.get("nonce_base64"):
            errors.append(
                "App Attest attestation and assertion challenges must use distinct nonces"
            )
        return "production-artifact-bound", evaluated_at

    if schemas == (
        QUALIFICATION_ATTESTATION_CHALLENGE_SCHEMA,
        QUALIFICATION_ASSERTION_CHALLENGE_SCHEMA,
    ):
        expected = (
            (
                attestation,
                QUALIFICATION_ATTESTATION_CHALLENGE_SCHEMA,
                QUALIFICATION_ATTESTATION_CHALLENGE_DOMAIN,
                QUALIFICATION_ATTESTATION_CHALLENGE_FIELDS,
                "qualification attestation challenge",
            ),
            (
                assertion,
                QUALIFICATION_ASSERTION_CHALLENGE_SCHEMA,
                QUALIFICATION_ASSERTION_CHALLENGE_DOMAIN,
                QUALIFICATION_ASSERTION_CHALLENGE_FIELDS,
                "qualification assertion challenge",
            ),
        )
        decoded_nonces: list[bytes] = []
        issued_times: list[int] = []
        for value, schema, domain, fields, label in expected:
            _exact_fields(value, fields, label, errors)
            if value.get("schema") != schema or value.get("domain") != domain:
                errors.append(f"{label} schema/domain is not exact")
            if value.get("version") != 1 or isinstance(value.get("version"), bool):
                errors.append(f"{label} version must be integer 1")
            issued_at = _positive_milliseconds(
                value.get("issued_at_unix_ms"), f"{label} issuance", errors
            )
            if issued_at is not None:
                issued_times.append(issued_at)
            nonce = production_module._require_base64(
                value.get("nonce_base64"), f"{label} nonce", 32, errors
            )
            if nonce is None or len(nonce) != 32:
                errors.append(f"{label} nonce must decode to exactly 32 bytes")
            else:
                decoded_nonces.append(nonce)
        if len(issued_times) == 2 and issued_times[0] != issued_times[1]:
            errors.append("qualification challenge pair issuance times differ")
        if len(decoded_nonces) == 2 and decoded_nonces[0] == decoded_nonces[1]:
            errors.append("qualification challenge pair must use distinct nonces")
        if assertion.get("attestation_object_sha256") != hashlib.sha256(
            attestation_object
        ).hexdigest():
            errors.append(
                "qualification assertion challenge does not bind the attestation object"
            )
        if assertion.get("key_id") != key_id_text:
            errors.append("qualification assertion challenge does not bind the App Attest key")
        return "qualification-only", None

    errors.append(
        "physical App Attest client data is neither the exact production "
        "challenge pair nor the explicit qualification pair"
    )
    return None, None


def validate_capture(
    capture_path: Path,
    request_path: Path,
    production_policy_path: Path,
    candidate_module: Any,
    production_module: Any,
    *,
    capture_app_code_sign_measurements_path: Optional[Path] = None,
) -> tuple[list[str], Optional[dict[str, Any]], Optional[dict[str, Any]]]:
    """Validate captured Apple objects with the production cryptographic parser."""

    errors: list[str] = []
    snapshots = []
    try:
        for path, label, maximum in (
            (capture_path, "physical App Attest capture", 1024 * 1024),
            (request_path, "physical App Attest request", 256 * 1024),
            (production_policy_path, "physical App Attest policy", 1024 * 1024),
        ):
            snapshot = candidate_module._snapshot_private_file(
                path.resolve(strict=True), label, maximum=maximum, retain_payload=True
            )
            snapshots.append((snapshot, label, maximum))
        capture = candidate_module.parse_strict_json(
            snapshots[0][0].payload, "physical App Attest capture"
        )
        request = candidate_module.parse_strict_json(
            snapshots[1][0].payload, "physical App Attest request"
        )
        policy = candidate_module.parse_strict_json(
            snapshots[2][0].payload, "physical App Attest policy"
        )
    except (OSError, candidate_module.EvidenceError) as error:
        return [str(error)], None, None

    _exact_fields(request, REQUEST_FIELDS, "physical App Attest request", errors)
    if request.get("schema") != REQUEST_SCHEMA:
        errors.append(f"physical App Attest request schema must be {REQUEST_SCHEMA}")
    if request.get("version") != 1 or isinstance(request.get("version"), bool):
        errors.append("physical App Attest request version must be integer 1")

    status = capture.get("status")
    expected_fields = (
        CAPTURE_SUCCESS_FIELDS if status == "captured" else CAPTURE_FAILURE_FIELDS
    )
    if _exact_fields(capture, expected_fields, "physical App Attest capture", errors) is None:
        return errors, None, None
    if capture.get("schema") != CAPTURE_SCHEMA:
        errors.append(f"physical App Attest capture schema must be {CAPTURE_SCHEMA}")
    if capture.get("version") != 1 or isinstance(capture.get("version"), bool):
        errors.append("physical App Attest capture version must be integer 1")
    if status != "captured":
        errors.append(
            "physical App Attest capture failed on device: "
            f"{capture.get('error_domain')} {capture.get('error_code')}: "
            f"{capture.get('error_description')}"
        )
        return errors, None, None
    if capture.get("app_attest_supported") is not True:
        errors.append("physical device did not report App Attest support")
    if capture.get("requested_environment") != "production":
        errors.append("physical capture did not request the production App Attest environment")
    started_at = _positive_milliseconds(
        capture.get("started_at_unix_ms"), "physical capture start", errors
    )
    captured_at = _positive_milliseconds(
        capture.get("captured_at_unix_ms"), "physical capture completion", errors
    )
    if started_at is not None and captured_at is not None and captured_at < started_at:
        errors.append("physical capture completion predates its start")

    if not isinstance(policy, dict):
        errors.append("physical App Attest policy must be an object")
        policy_valid = False
    else:
        policy_valid = production_module._validate_policy(
            policy, snapshots[2][0].payload, errors
        )
        if capture.get("bundle_id") != policy.get("bundle_id"):
            errors.append("physical capture bundle id does not match production policy")
        allowed_bundle_versions = policy.get("allowed_bundle_versions")
        if (
            not isinstance(allowed_bundle_versions, list)
            or capture.get("bundle_version") not in allowed_bundle_versions
        ):
            errors.append("physical capture bundle version is not allowed by policy")

    capture_app_measurements = None
    capture_app_measurements_sha256 = None
    if capture_app_code_sign_measurements_path is not None and isinstance(policy, dict):
        try:
            measurement_snapshot = candidate_module._snapshot_private_file(
                capture_app_code_sign_measurements_path.resolve(strict=True),
                "prepared capture-app code-sign measurements",
                maximum=64 * 1024,
                retain_payload=True,
            )
            capture_app_measurements = candidate_module.parse_strict_json(
                measurement_snapshot.payload,
                "prepared capture-app code-sign measurements",
            )
        except (OSError, candidate_module.EvidenceError) as error:
            errors.append(str(error))
        else:
            capture_app_measurements_sha256 = (
                production_module._validate_capture_app_code_sign_measurements(
                    capture_app_measurements,
                    policy,
                    candidate_module,
                    errors,
                )
            )
            if (
                capture_app_measurements_sha256 is not None
                and measurement_snapshot.sha256 != capture_app_measurements_sha256
            ):
                errors.append("prepared capture-app measurement bytes are not canonical")
            snapshots.append(
                (
                    measurement_snapshot,
                    "prepared capture-app code-sign measurements",
                    64 * 1024,
                )
            )

    key_id_text = capture.get("key_id")
    key_id = production_module._require_base64(
        key_id_text, "physical App Attest key id", 64, errors
    )
    attestation_client_data = production_module._require_base64(
        capture.get("attestation_client_data_base64"),
        "physical App Attest attestation client data",
        production_module.MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    attestation_object = production_module._require_base64(
        capture.get("attestation_object_base64"),
        "physical App Attest attestation object",
        production_module.MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    assertion_client_data = production_module._require_base64(
        capture.get("assertion_client_data_base64"),
        "physical App Attest assertion client data",
        production_module.MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    assertion_object = production_module._require_base64(
        capture.get("assertion_object_base64"),
        "physical App Attest assertion object",
        production_module.MAX_PLATFORM_OBJECT_BYTES,
        errors,
    )
    requested_attestation = production_module._decode_base64(
        request.get("attestation_client_data_base64"),
        "requested App Attest attestation client data",
        production_module.MAX_PLATFORM_OBJECT_BYTES,
    )
    if requested_attestation is None or requested_attestation != attestation_client_data:
        errors.append("physical capture substituted the attestation client data")

    challenge_kind = None
    challenge_evaluation_time = None
    if (
        attestation_client_data is not None
        and assertion_client_data is not None
        and attestation_object is not None
        and isinstance(key_id_text, str)
        and isinstance(policy, dict)
    ):
        challenge_kind, challenge_evaluation_time = _validate_capture_challenge_pair(
            attestation_client_data,
            assertion_client_data,
            attestation_object,
            key_id_text,
            policy,
            snapshots[2][0].payload,
            capture_app_measurements_sha256,
            candidate_module,
            production_module,
            errors,
        )

    public_key = None
    chain = None
    if attestation_object is not None:
        try:
            public_key, chain = _extract_attested_public_key(
                attestation_object, production_module
            )
        except ValueError as error:
            errors.append(str(error))
    if key_id is not None and len(key_id) != 32:
        errors.append("physical App Attest key id must contain exactly 32 bytes")
    if key_id is not None and public_key is not None:
        if key_id != hashlib.sha256(public_key).digest():
            errors.append("physical App Attest key id does not bind the attested P-256 key")

    if (
        isinstance(request.get("assertion_client_data_template"), dict)
        and attestation_object is not None
        and isinstance(key_id_text, str)
        and assertion_client_data is not None
    ):
        expected_assertion = dict(request["assertion_client_data_template"])
        if (
            "attestation_object_sha256" in expected_assertion
            or "key_id" in expected_assertion
        ):
            errors.append("assertion client-data template contains device-bound fields")
        expected_assertion["attestation_object_sha256"] = hashlib.sha256(
            attestation_object
        ).hexdigest()
        expected_assertion["key_id"] = key_id_text
        try:
            expected_assertion_bytes = candidate_module.canonical_json_bytes(
                expected_assertion
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))
        else:
            if expected_assertion_bytes != assertion_client_data:
                errors.append("physical capture substituted the assertion client data")
    else:
        errors.append("physical App Attest assertion template is invalid")

    attestation_result = None
    assertion_result = None
    evidence_evaluation_time = (
        challenge_evaluation_time
        if challenge_kind == "production-artifact-bound"
        else captured_at
    )
    if (
        policy_valid
        and evidence_evaluation_time is not None
        and key_id is not None
        and public_key is not None
        and attestation_object is not None
        and attestation_client_data is not None
    ):
        attestation_result = production_module._parse_attestation_object(
            attestation_object,
            attestation_client_data,
            key_id,
            public_key,
            policy,
            evidence_evaluation_time,
            errors,
        )
    if (
        policy_valid
        and public_key is not None
        and assertion_object is not None
        and assertion_client_data is not None
    ):
        assertion_result = production_module._parse_assertion_object(
            assertion_object,
            assertion_client_data,
            public_key,
            policy,
            errors,
        )

    platform_evidence = None
    summary = None
    if (
        not errors
        and captured_at is not None
        and isinstance(key_id_text, str)
        and public_key is not None
        and chain is not None
        and attestation_client_data is not None
        and attestation_object is not None
        and assertion_client_data is not None
        and assertion_object is not None
        and attestation_result is not None
        and assertion_result is not None
    ):
        platform_evidence = {
            "schema": (
                production_module.PLATFORM_EVIDENCE_SCHEMA
                if challenge_kind == "production-artifact-bound"
                else QUALIFICATION_MATERIAL_SCHEMA
            ),
            "version": 1,
            "evaluated_at_unix_ms": evidence_evaluation_time,
            "key_id": key_id_text,
            "assertion_public_key_sec1_base64": base64.b64encode(public_key).decode(
                "ascii"
            ),
            "attestation_client_data_base64": base64.b64encode(
                attestation_client_data
            ).decode("ascii"),
            "attestation_object_base64": base64.b64encode(attestation_object).decode(
                "ascii"
            ),
            "assertion_client_data_base64": base64.b64encode(
                assertion_client_data
            ).decode("ascii"),
            "assertion_object_base64": base64.b64encode(assertion_object).decode(
                "ascii"
            ),
        }
        if challenge_kind == "production-artifact-bound":
            platform_evidence["capture_app_code_sign_measurements"] = (
                capture_app_measurements
            )
        else:
            platform_evidence.update(
                {
                    "promotion_eligible": False,
                    "qualification_scope": "apple-chain-and-object-compatibility-only",
                }
            )
        summary = {
            "schema": SUMMARY_SCHEMA,
            "version": 1,
            "status": (
                "valid-production-artifact-bound-capture-requires-full-envelope-validation"
                if challenge_kind == "production-artifact-bound"
                else "valid-production-environment-app-attest-qualification"
            ),
            "promotion_eligible": False,
            "bundle_id": capture["bundle_id"],
            "bundle_version": capture["bundle_version"],
            "captured_at_unix_ms": captured_at,
            "key_id_sha256": hashlib.sha256(key_id).hexdigest(),
            "public_key_sha256": hashlib.sha256(public_key).hexdigest(),
            "attestation_object_sha256": hashlib.sha256(attestation_object).hexdigest(),
            "assertion_object_sha256": hashlib.sha256(assertion_object).hexdigest(),
            "certificate_chain_sha256": [
                hashlib.sha256(certificate).hexdigest() for certificate in chain
            ],
            "assertion_counter": assertion_result[0],
        }

    for snapshot, label, maximum in snapshots:
        try:
            candidate_module._require_private_file_snapshot_unchanged(
                snapshot, label, maximum=maximum
            )
        except candidate_module.EvidenceError as error:
            errors.append(str(error))
    if errors:
        return errors, None, None
    return [], platform_evidence, summary
