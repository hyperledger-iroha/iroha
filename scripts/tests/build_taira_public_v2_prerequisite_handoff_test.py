from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
from dataclasses import dataclass
from pathlib import Path
import stat

import pytest

from scripts import build_taira_public_v2_prerequisite_handoff as handoff


NOW = 1_800_000_000
SOURCE = handoff.admission.SourceIdentity(
    "1" * 40,
    "2" * 40,
    hashlib.sha256(b"Cargo.lock").hexdigest(),
    hashlib.sha256(b"workspace source manifest").hexdigest(),
)
RECEIPT_ID = hashlib.sha256(b"qualification receipt").hexdigest()
VALIDATOR_SHA256 = hashlib.sha256(b"iroha3d production binary").hexdigest()
ED25519_SEED = hashlib.sha256(b"publisher test signing seed").digest()


def _sha(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _compact(value: object) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _pretty(value: object) -> bytes:
    return handoff.canonical_json_bytes(value)


def _write(path: Path, payload: bytes, mode: int = 0o444) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    if path.exists():
        path.chmod(0o600)
    path.write_bytes(payload)
    path.chmod(mode)


_Q = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _Q - 2, _Q)) % _Q
_I = pow(2, (_Q - 1) // 4, _Q)


def _xrecover(y: int) -> int:
    xx = ((y * y - 1) * pow(_D * y * y + 1, _Q - 2, _Q)) % _Q
    x = pow(xx, (_Q + 3) // 8, _Q)
    if (x * x - xx) % _Q:
        x = (x * _I) % _Q
    return _Q - x if x & 1 else x


_BY = (4 * pow(5, _Q - 2, _Q)) % _Q
_B = (_xrecover(_BY), _BY)


def _edwards(left: tuple[int, int], right: tuple[int, int]) -> tuple[int, int]:
    x1, y1 = left
    x2, y2 = right
    product = (_D * x1 * x2 * y1 * y2) % _Q
    return (
        ((x1 * y2 + x2 * y1) * pow(1 + product, _Q - 2, _Q)) % _Q,
        ((y1 * y2 + x1 * x2) * pow(1 - product, _Q - 2, _Q)) % _Q,
    )


def _scalarmult(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    if scalar == 0:
        return 0, 1
    half = _scalarmult(point, scalar // 2)
    doubled = _edwards(half, half)
    return _edwards(doubled, point) if scalar & 1 else doubled


def _encode_point(point: tuple[int, int]) -> bytes:
    x, y = point
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _ed25519_public_key(seed: bytes) -> bytes:
    expanded = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(expanded[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return _encode_point(_scalarmult(_B, scalar))


def _ed25519_sign(seed: bytes, message: bytes) -> bytes:
    expanded = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(expanded[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    public_key = _encode_point(_scalarmult(_B, scalar))
    nonce = int.from_bytes(hashlib.sha512(expanded[32:] + message).digest(), "little") % _L
    encoded_r = _encode_point(_scalarmult(_B, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public_key + message).digest(), "little"
    ) % _L
    return encoded_r + ((nonce + challenge * scalar) % _L).to_bytes(32, "little")


PUBLIC_KEY = _ed25519_public_key(ED25519_SEED)
SIGNING_FINGERPRINT = _sha(PUBLIC_KEY)


def _receipt_signers() -> dict[str, dict[str, object]]:
    result: dict[str, dict[str, object]] = {}
    x = 1
    for slug in handoff.admission.SLUGS:
        while True:
            payload = "02" + format(x, "064x")
            try:
                node_id = handoff.admission._receipt_node_id(payload, "test signer")
            except handoff.admission.TairaRolloutAdmissionError:
                x += 1
                continue
            break
        result[slug] = {
            "node_id": node_id,
            "public_key": {"algorithm": "secp256k1", "payload_hex": payload},
        }
        x += 1
    return result


def _admission_result(
    archive: Path,
    authority: Path,
    source: handoff.admission.SourceIdentity,
    receipt_id: str,
    fingerprint: str,
    verifier_sha256: str,
) -> dict[str, object]:
    fixed = _sha(b"fixed authenticated admission field")
    return {
        "artifact_handoff_sha256": fixed,
        "archive_sha256": _sha(archive.read_bytes()),
        "boi_artifact_inventory_sha256": fixed,
        "deployment_performed": False,
        "linux_authority_manifest_sha256": fixed,
        "macos_end_block_hash": fixed,
        "macos_end_height": 42,
        "peer_count": 4,
        "privacy_protocol_receipt_id": fixed,
        "receipt_id": receipt_id,
        "receipt_signers": _receipt_signers(),
        "release_manifest_sha256": _sha(
            (authority / "release_manifest.json").read_bytes()
        ),
        "release_manifest_verifier_sha256": verifier_sha256,
        "reset_manifest_sha256": fixed,
        "restart_generation": fixed,
        "schema": handoff.admission.VERIFICATION_SCHEMA,
        "schema_version": handoff.admission.VERIFICATION_SCHEMA_VERSION,
        "signer_fingerprint_sha256": fingerprint,
        "source": source.as_dict(),
        "supervisor_sha256": fixed,
        "validator_binary_sha256": VALIDATOR_SHA256,
        "validator_config_sha256": {
            slug: fixed for slug in handoff.admission.SLUGS
        },
        "verified": True,
    }


def _candidate(root: Path) -> Path:
    candidate = root / "publish-candidate-100-1"
    archive_name = (
        f"taira-admission-{SOURCE.workspace_source_manifest_sha256[:16]}-"
        "macos-arm64.tar.gz"
    )
    payloads = {
        f"admission/{archive_name}": b"authenticated admission archive\n",
        "authority/release_manifest.json": _pretty(
            {"schema": "iroha.release_manifest", "source": SOURCE.as_dict()}
        ),
        "authority/release_manifest.json.pub": PUBLIC_KEY,
        "authority/release_manifest.json.sig": _ed25519_sign(
            ED25519_SEED, b"candidate authority fixture"
        ),
        handoff.publisher.RECEIPT_ID_NAME: (RECEIPT_ID + "\n").encode("ascii"),
        handoff.publisher.SOURCE_IDENTITY_NAME: _compact(
            {"source": SOURCE.as_dict(), "source_date_epoch": 1_750_000_000}
        ),
    }
    for relative, payload in payloads.items():
        _write(candidate / relative, payload)
    rows = [
        {"path": relative, "sha256": _sha(payload), "size": len(payload)}
        for relative, payload in sorted(payloads.items())
    ]
    _write(
        candidate / handoff.publisher.HANDOFF_MANIFEST,
        _compact(
            {
                "files": rows,
                "kind": "candidate",
                "schema": "iroha.taira.release_handoff",
                "schema_version": 1,
            }
        ),
    )
    (candidate / "admission").chmod(0o555)
    (candidate / "authority").chmod(0o555)
    candidate.chmod(0o555)
    return candidate


def _oci_manifest(
    artifact_type: str,
    layers: list[handoff.publisher.Layer],
    created: str,
    *,
    subject: tuple[str, int] | None = None,
) -> bytes:
    value: dict[str, object] = {
        "annotations": {"org.opencontainers.image.created": created},
        "artifactType": artifact_type,
        "config": {
            "data": handoff.publisher.OCI_EMPTY_CONFIG_DATA,
            "digest": handoff.publisher.OCI_EMPTY_CONFIG_DIGEST,
            "mediaType": handoff.publisher.OCI_EMPTY_CONFIG_MEDIA_TYPE,
            "size": 2,
        },
        "layers": [
            {
                "annotations": {"org.opencontainers.image.title": layer.path},
                "digest": f"sha256:{layer.sha256}",
                "mediaType": layer.media_type,
                "size": layer.size,
            }
            for layer in layers
        ],
        "mediaType": handoff.publisher.OCI_MANIFEST_MEDIA_TYPE,
        "schemaVersion": 2,
    }
    if subject is not None:
        value["subject"] = {
            "digest": subject[0],
            "mediaType": handoff.publisher.OCI_MANIFEST_MEDIA_TYPE,
            "size": subject[1],
        }
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def _publication(
    handoff_root: Path,
    candidate: Path,
    admission_result: dict[str, object],
    verifier_sha256: str,
    oras_sha256: str,
) -> Path:
    publication = handoff_root / (
        handoff.publication_closer.OUTPUT_PREFIX + RECEIPT_ID
    )
    publication.mkdir(mode=0o700)
    captured = handoff.publisher._capture_candidate(candidate, SOURCE, RECEIPT_ID)
    candidate_layers = handoff.publisher._candidate_layers(captured)
    created = (
        dt.datetime(1970, 1, 1, tzinfo=dt.timezone.utc)
        + dt.timedelta(seconds=NOW)
    ).strftime("%Y-%m-%dT%H:%M:%SZ")
    primary_manifest = _oci_manifest(
        handoff.publisher.PRIMARY_ARTIFACT_TYPE, candidate_layers, created
    )
    primary_digest = "sha256:" + _sha(primary_manifest)
    repository = "registry.example/hyperledger/iroha-taira"
    suffix = "first-release"
    tag = f"taira-{handoff.publisher._source_identity_digest(SOURCE)}-{suffix}"
    receipt_value = {
        "admission_sha256": _sha(_pretty(admission_result)),
        "immutable_reference": f"{repository}@{primary_digest}",
        "issued_at_unix": NOW,
        "layers": [layer.receipt_row() for layer in candidate_layers],
        "oras": {"executable_sha256": oras_sha256, "version": "1.3.2"},
        "qualification_receipt_id": RECEIPT_ID,
        "repository": repository,
        "schema": handoff.publisher.PUBLICATION_SCHEMA,
        "schema_version": handoff.publisher.PUBLICATION_SCHEMA_VERSION,
        "signing": {
            "native_verifier_sha256": verifier_sha256,
            "signer_fingerprint_sha256": SIGNING_FINGERPRINT,
        },
        "source": SOURCE.as_dict(),
        "subject": {
            "digest": primary_digest,
            "media_type": handoff.publisher.OCI_MANIFEST_MEDIA_TYPE,
            "size": len(primary_manifest),
        },
        "suffix": suffix,
        "tag": tag,
        "tagged_reference": f"{repository}:{tag}",
    }
    receipt = _pretty(receipt_value)
    signature = _ed25519_sign(ED25519_SEED, receipt)
    receipt_layers = [
        handoff.publisher.Layer(
            handoff.publisher.PUBLICATION_RECEIPT_NAME,
            handoff.publisher.PUBLICATION_RECEIPT_MEDIA_TYPE,
            _sha(receipt),
            len(receipt),
        ),
        handoff.publisher.Layer(
            handoff.publisher.PUBLICATION_SIGNATURE_NAME,
            handoff.publisher.AUTHORITY_SIGNATURE_MEDIA_TYPE,
            _sha(signature),
            len(signature),
        ),
        handoff.publisher.Layer(
            handoff.publisher.PUBLICATION_PUBLIC_KEY_NAME,
            handoff.publisher.AUTHORITY_PUBLIC_KEY_MEDIA_TYPE,
            _sha(PUBLIC_KEY),
            len(PUBLIC_KEY),
        ),
    ]
    receipt_manifest = _oci_manifest(
        handoff.publisher.PUBLICATION_ARTIFACT_TYPE,
        receipt_layers,
        created,
        subject=(primary_digest, len(primary_manifest)),
    )
    receipt_digest = "sha256:" + _sha(receipt_manifest)
    payloads = {
        handoff.publisher.PUBLICATION_RECEIPT_NAME: receipt,
        handoff.publisher.PUBLICATION_SIGNATURE_NAME: signature,
        handoff.publisher.PUBLICATION_PUBLIC_KEY_NAME: PUBLIC_KEY,
        handoff.publisher.PRIMARY_MANIFEST_NAME: primary_manifest,
        handoff.publisher.RECEIPT_MANIFEST_NAME: receipt_manifest,
        handoff.publisher.PRIMARY_DIGEST_NAME: (primary_digest + "\n").encode(
            "ascii"
        ),
        handoff.publisher.RECEIPT_DIGEST_NAME: (receipt_digest + "\n").encode(
            "ascii"
        ),
    }
    for name, payload in payloads.items():
        _write(publication / name, payload)
    publication.chmod(0o555)
    return publication


@dataclass
class Harness:
    root: Path
    handoff_root: Path
    candidate: Path
    candidate_handoff: Path
    publication_root: Path
    attestation_path: Path
    attestation: dict[str, object]
    verifier: Path
    verifier_sha256: str
    oras_sha256: str
    admission_result: dict[str, object]


@pytest.fixture
def harness(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Harness:
    uid = os.geteuid()
    gid = os.getegid()
    if uid == 0:
        pytest.skip("owner-bound handoff fixtures require a non-root test process")
    monkeypatch.setattr(handoff.admission, "_require_privacy_protocol_controller_origin_authority", lambda: None)
    monkeypatch.setattr(handoff.admission, "_require_independent_native_evidence_authority", lambda: None)
    monkeypatch.setattr(handoff.publisher, "_require_authenticated_rollout_observation_authority", lambda: None)
    monkeypatch.setattr(
        handoff.publisher,
        "_is_root_owned",
        lambda info: info.st_uid in {0, uid},
    )
    monkeypatch.setattr(handoff.time, "time", lambda: NOW)

    handoff_root = tmp_path / "immutable-handoffs"
    handoff_root.mkdir(mode=0o711)
    for name in ("authority", "runtime", "staging"):
        (tmp_path / name).mkdir(mode=0o700)
    tools = tmp_path / "trusted-tools"
    tools.mkdir(mode=0o755)
    verifier = tools / "sorafs-validate"
    oras = tools / "oras"
    signer = tools / "external-signer"
    _write(verifier, b"pinned native release verifier", 0o555)
    _write(oras, b"pinned oras executable", 0o555)
    _write(signer, b"pinned external signer", 0o555)
    verifier_sha256 = _sha(verifier.read_bytes())
    oras_sha256 = _sha(oras.read_bytes())
    private = tmp_path / "authority" / "super-secret-registry-token"
    _write(private, b'{"auths":{"registry.example":{"auth":"secret"}}}\n', 0o400)
    signing_key = tmp_path / "authority" / "publisher.pub"
    _write(signing_key, PUBLIC_KEY, 0o444)
    trusted_values = [
        {
            "flag": "--expected-oras-version",
            "operation": "publish-rollout",
            "value": "1.3.2",
        },
        {
            "flag": "--repository",
            "operation": "publish-rollout",
            "value": "registry.example/hyperledger/iroha-taira",
        },
        {
            "flag": "--suffix",
            "operation": "publish-rollout",
            "value": "first-release",
        },
        {
            "flag": "--trusted-signing-fingerprint",
            "operation": "publish-rollout",
            "value": SIGNING_FINGERPRINT,
        },
    ]
    trusted_executables = [
        {
            "digest_flag": "--trusted-external-signer-sha256",
            "flag": "--external-signer",
            "operation": "publish-rollout",
            "path": str(signer),
            "run_as": "authority",
            "sha256": _sha(signer.read_bytes()),
        },
        {
            "digest_flag": "--trusted-oras-sha256",
            "flag": "--oras",
            "operation": "publish-rollout",
            "path": str(oras),
            "run_as": "authority",
            "sha256": oras_sha256,
        },
        {
            "digest_flag": "--trusted-release-manifest-verifier-sha256",
            "flag": "--release-manifest-verifier",
            "operation": "publish-rollout",
            "path": str(verifier),
            "run_as": "authority",
            "sha256": verifier_sha256,
        },
    ]
    attestation: dict[str, object] = {
        "authority_gid": gid,
        "authority_root": str(tmp_path / "authority"),
        "authority_uid": uid,
        "controller_digest": _sha(b"installed publisher controller closure"),
        "controller_gid": gid,
        "controller_manifest": str(tools / "authority-controller-v1.json"),
        "controller_root": str(tools),
        "controller_version": handoff.controllers.CONTROLLER_VERSION,
        "handoff_root": str(handoff_root),
        "host_id": "publisher-host-01",
        "installation_id": "publisher-installation-01",
        "invoking_gid": gid,
        "invoking_uid": uid,
        "launcher_sha256": _sha(b"installed controller launcher"),
        "platform": "macos",
        "role": "macos-publish",
        "runtime_gid": gid,
        "runtime_root": str(tmp_path / "runtime"),
        "runtime_uid": uid + 1,
        "source_commit": SOURCE.commit,
        "staging_gid": gid,
        "staging_root": str(tmp_path / "staging"),
        "staging_uid": uid + 2,
        "trusted_executables": trusted_executables,
        "trusted_inputs": [
            {
                "flag": "--registry-config",
                "operation": "publish-rollout",
                "path": str(private),
            },
            {
                "flag": "--signing-public-key",
                "operation": "publish-rollout",
                "path": str(signing_key),
            },
        ],
        "trusted_values": trusted_values,
        "uid": uid,
    }
    attestation_path = tmp_path / "publisher-attestation.json"
    _write(
        attestation_path,
        handoff.controllers.canonical_json_bytes(attestation),
        0o600,
    )
    monkeypatch.setattr(handoff.controllers, "_attest", lambda **_kwargs: attestation)

    candidate = _candidate(handoff_root)

    def admit(**kwargs):
        if (
            kwargs["expected_source"] != SOURCE
            or kwargs["expected_receipt_id"] != RECEIPT_ID
            or kwargs["trusted_signing_fingerprint"] != SIGNING_FINGERPRINT
            or kwargs["trusted_release_manifest_verifier_sha256"]
            != verifier_sha256
            or kwargs["release_manifest_verifier_path"] != verifier
            or kwargs["now_unix"] != NOW
        ):
            raise handoff.admission.TairaRolloutAdmissionError(
                "fixture admission trust differs"
            )
        return _admission_result(
            kwargs["archive_path"],
            kwargs["authority_dir"],
            kwargs["expected_source"],
            kwargs["expected_receipt_id"],
            kwargs["trusted_signing_fingerprint"],
            kwargs["trusted_release_manifest_verifier_sha256"],
        )

    monkeypatch.setattr(handoff.admission, "verify_admission", admit)

    def verify(manifest, signature, public_key, fingerprint, verifier_path, verifier_sha):
        manifest_payload = Path(manifest).read_bytes()
        if (
            Path(public_key).read_bytes() != PUBLIC_KEY
            or Path(signature).read_bytes()
            != _ed25519_sign(ED25519_SEED, manifest_payload)
            or fingerprint != SIGNING_FINGERPRINT
            or Path(verifier_path) != verifier
            or verifier_sha != verifier_sha256
        ):
            raise handoff.ReleaseManifestSignatureError(
                "fixture Ed25519 publication signature differs"
            )
        return {
            "manifest_sha256": _sha(manifest_payload),
            "native_verifier_sha256": verifier_sha256,
            "signature_verified": True,
            "signer_fingerprint_sha256": SIGNING_FINGERPRINT,
        }

    monkeypatch.setattr(handoff, "verify_release_manifest", verify)
    candidate_handoff = tmp_path / "candidate-prerequisite.json"
    candidate_document = handoff.build_candidate_handoff(
        candidate, attestation_path, candidate_handoff
    )
    archive = next((candidate / "admission").iterdir())
    admission_result = _admission_result(
        archive,
        candidate / "authority",
        SOURCE,
        RECEIPT_ID,
        SIGNING_FINGERPRINT,
        verifier_sha256,
    )
    assert candidate_document["identity"]["validator_binary_sha256"] == VALIDATOR_SHA256
    publication_root = _publication(
        handoff_root,
        candidate,
        admission_result,
        verifier_sha256,
        oras_sha256,
    )
    return Harness(
        tmp_path,
        handoff_root,
        candidate,
        candidate_handoff,
        publication_root,
        attestation_path,
        attestation,
        verifier,
        verifier_sha256,
        oras_sha256,
        admission_result,
    )


def _checker_accepts(path: Path, kind: str, source: dict[str, str]) -> dict[str, object]:
    payload = path.read_bytes()
    info = path.stat()
    artifact = handoff.soak_checker.Artifact(
        path,
        payload,
        _sha(payload),
        len(payload),
        info.st_dev,
        info.st_ino,
    )
    reference = {
        "kind": kind,
        "schema": handoff.HANDOFF_SCHEMA,
        "sha256": artifact.sha256,
        "size_bytes": artifact.size,
        "source": source,
    }
    fields = (
        set(handoff.CANDIDATE_FIELDS)
        if kind == "candidate"
        else set(handoff.PUBLICATION_FIELDS)
    )
    _digest, identity = handoff.soak_checker._validate_handoff(
        artifact,
        reference,
        kind=kind,
        identity_fields=fields,
        source=source,
    )
    return dict(identity)


def _publication_output(harness: Harness, name: str = "publication-prerequisite.json") -> Path:
    output = harness.root / name
    handoff.build_publication_handoff(
        harness.candidate,
        harness.candidate_handoff,
        harness.publication_root,
        harness.attestation_path,
        output,
    )
    return output


def _rewrite(path: Path, value: object, *, compact: bool = True, mode: int = 0o400) -> None:
    path.chmod(0o600)
    path.write_bytes(_compact(value) if compact else _pretty(value))
    path.chmod(mode)


def test_candidate_derives_exact_checker_document_from_admission(harness: Harness) -> None:
    identity = _checker_accepts(
        harness.candidate_handoff, "candidate", SOURCE.as_dict()
    )
    candidate = handoff.publisher._capture_candidate(
        harness.candidate, SOURCE, RECEIPT_ID
    )
    assert identity == {
        "admission_archive_sha256": candidate.files[
            candidate.archive_relative
        ].sha256,
        "admission_authority_manifest_sha256": candidate.files[
            "authority/release_manifest.json"
        ].sha256,
        "handoff_inventory_sha256": candidate.files[
            handoff.publisher.HANDOFF_MANIFEST
        ].sha256,
        "qualification_receipt_id": RECEIPT_ID,
        "validator_binary_sha256": VALIDATOR_SHA256,
    }
    assert stat.S_IMODE(harness.candidate_handoff.stat().st_mode) == 0o400


def test_publication_revalidates_signature_oci_and_controller_binding(
    harness: Harness,
) -> None:
    output = _publication_output(harness)
    identity = _checker_accepts(output, "publication", SOURCE.as_dict())
    assert identity["candidate_handoff_sha256"] == _sha(
        harness.candidate_handoff.read_bytes()
    )
    assert identity["publication_receipt_sha256"] == _sha(
        (
            harness.publication_root
            / handoff.publisher.PUBLICATION_RECEIPT_NAME
        ).read_bytes()
    )
    assert identity["publication_signature_sha256"] == _sha(
        (
            harness.publication_root
            / handoff.publisher.PUBLICATION_SIGNATURE_NAME
        ).read_bytes()
    )
    assert identity["publication_public_key_sha256"] == SIGNING_FINGERPRINT
    assert identity["publisher_controller_sha256"] == harness.attestation[
        "controller_digest"
    ]
    assert identity["validator_binary_sha256"] == VALIDATOR_SHA256


def test_output_is_atomic_no_overwrite(harness: Harness) -> None:
    before = harness.candidate_handoff.read_bytes()
    with pytest.raises(handoff.PrerequisiteHandoffError, match="already exists"):
        handoff.build_candidate_handoff(
            harness.candidate,
            harness.attestation_path,
            harness.candidate_handoff,
        )
    assert harness.candidate_handoff.read_bytes() == before
    assert not list(harness.root.glob(".*candidate-prerequisite.json.*.tmp"))


@pytest.mark.parametrize("kind", ("candidate", "publication"))
def test_output_cannot_modify_a_frozen_input_root(
    harness: Harness,
    kind: str,
) -> None:
    frozen = harness.candidate if kind == "candidate" else harness.publication_root
    output = frozen / "prerequisite-must-not-appear.json"
    with pytest.raises(handoff.PrerequisiteHandoffError, match="must not modify"):
        if kind == "candidate":
            handoff.build_candidate_handoff(
                harness.candidate,
                harness.attestation_path,
                output,
            )
        else:
            handoff.build_publication_handoff(
                harness.candidate,
                harness.candidate_handoff,
                harness.publication_root,
                harness.attestation_path,
                output,
            )
    assert not output.exists()


def test_candidate_handoff_mutation_is_not_promoted(harness: Harness) -> None:
    value = json.loads(harness.candidate_handoff.read_bytes())
    value["identity"]["validator_binary_sha256"] = _sha(b"substituted binary")
    _rewrite(harness.candidate_handoff, value)
    with pytest.raises(handoff.PrerequisiteHandoffError, match="replayed candidate"):
        _publication_output(harness, "mutated-candidate-publication.json")


@pytest.mark.parametrize(
    "mutation",
    (
        "signature",
        "public-key",
        "primary-manifest",
        "receipt-subject",
    ),
)
def test_publication_byte_mutations_are_rejected(
    harness: Harness,
    mutation: str,
) -> None:
    root = harness.publication_root
    root.chmod(0o755)
    if mutation == "signature":
        path = root / handoff.publisher.PUBLICATION_SIGNATURE_NAME
        payload = bytearray(path.read_bytes())
        payload[0] ^= 1
        _write(path, bytes(payload))
    elif mutation == "public-key":
        path = root / handoff.publisher.PUBLICATION_PUBLIC_KEY_NAME
        payload = bytearray(path.read_bytes())
        payload[0] ^= 1
        _write(path, bytes(payload))
    elif mutation == "primary-manifest":
        path = root / handoff.publisher.PRIMARY_MANIFEST_NAME
        value = json.loads(path.read_bytes())
        value["artifactType"] = "application/vnd.attacker.old-candidate"
        _write(path, json.dumps(value, sort_keys=True, separators=(",", ":")).encode())
    else:
        path = root / handoff.publisher.RECEIPT_MANIFEST_NAME
        value = json.loads(path.read_bytes())
        value["subject"]["digest"] = "sha256:" + _sha(b"other primary")
        payload = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
        _write(path, payload)
        digest = root / handoff.publisher.RECEIPT_DIGEST_NAME
        _write(digest, ("sha256:" + _sha(payload) + "\n").encode("ascii"))
    root.chmod(0o555)
    with pytest.raises(handoff.PrerequisiteHandoffError):
        _publication_output(harness, f"mutated-{mutation}.json")


def test_candidate_hardlink_alias_is_rejected(harness: Harness) -> None:
    authority = harness.candidate / "authority"
    authority.chmod(0o755)
    signature = authority / "release_manifest.json.sig"
    signature.unlink()
    os.link(authority / "release_manifest.json.pub", signature)
    authority.chmod(0o555)
    with pytest.raises(handoff.PrerequisiteHandoffError, match="unsafe or writable"):
        handoff.build_candidate_handoff(
            harness.candidate,
            harness.attestation_path,
            harness.root / "aliased-candidate.json",
        )


def test_publication_hardlink_alias_is_rejected(harness: Harness) -> None:
    root = harness.publication_root
    root.chmod(0o755)
    receipt_digest = root / handoff.publisher.RECEIPT_DIGEST_NAME
    receipt_digest.unlink()
    os.link(root / handoff.publisher.PRIMARY_DIGEST_NAME, receipt_digest)
    root.chmod(0o555)
    with pytest.raises(handoff.PrerequisiteHandoffError, match="file identity"):
        _publication_output(harness, "aliased-publication.json")


def test_symlinked_publication_root_is_rejected(harness: Harness) -> None:
    alias = harness.handoff_root / (
        handoff.publication_closer.OUTPUT_PREFIX + RECEIPT_ID + "-alias"
    )
    alias.symlink_to(harness.publication_root, target_is_directory=True)
    with pytest.raises(handoff.PrerequisiteHandoffError, match="symbolic links"):
        handoff.build_publication_handoff(
            harness.candidate,
            harness.candidate_handoff,
            alias,
            harness.attestation_path,
            harness.root / "symlink-publication.json",
        )


def test_controller_attestation_mutation_is_rejected(harness: Harness) -> None:
    value = json.loads(harness.attestation_path.read_bytes())
    value["controller_digest"] = _sha(b"caller asserted controller")
    _rewrite(harness.attestation_path, value, mode=0o600)
    with pytest.raises(handoff.PrerequisiteHandoffError, match="installed state"):
        handoff.build_candidate_handoff(
            harness.candidate,
            harness.attestation_path,
            harness.root / "forged-controller.json",
        )


def test_outputs_do_not_leak_private_paths_keys_or_signatures(harness: Harness) -> None:
    publication = _publication_output(harness, "private-leak-check.json")
    payloads = [harness.candidate_handoff.read_bytes(), publication.read_bytes()]
    for payload in payloads:
        lowered = payload.lower()
        assert b"secret" not in lowered
        assert str(harness.root).encode() not in payload
        assert PUBLIC_KEY.hex().encode() not in lowered
        assert _ed25519_sign(ED25519_SEED, b"candidate authority fixture").hex().encode() not in lowered
        value = json.loads(payload)
        assert set(value) == set(handoff.DOCUMENT_FIELDS)


def test_missing_native_authority_refuses_before_path_io(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def missing() -> None:
        raise handoff.admission.TairaRolloutAdmissionError(
            "missing authenticated native qualification producer"
        )

    monkeypatch.setattr(
        handoff.admission,
        "_require_privacy_protocol_controller_origin_authority",
        missing,
    )
    monkeypatch.setattr(
        handoff,
        "_authenticate_controller_attestation",
        lambda _path: pytest.fail("attestation path was inspected before authority"),
    )
    with pytest.raises(
        handoff.PrerequisiteHandoffError,
        match="missing authenticated native qualification producer",
    ):
        handoff.build_candidate_handoff(
            tmp_path / "absent-candidate",
            tmp_path / "absent-attestation",
            tmp_path / "must-not-exist.json",
        )
    assert not (tmp_path / "must-not-exist.json").exists()


def test_missing_rollout_observation_authority_refuses_before_path_io(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def missing() -> None:
        raise handoff.publisher.TairaPublicationError(
            "missing authenticated rollout observation producer"
        )

    monkeypatch.setattr(
        handoff.publisher,
        "_require_authenticated_rollout_observation_authority",
        missing,
    )
    monkeypatch.setattr(
        handoff,
        "_authenticate_controller_attestation",
        lambda _path: pytest.fail("attestation path was inspected before authority"),
    )
    output = tmp_path / "must-not-exist.json"
    with pytest.raises(
        handoff.PrerequisiteHandoffError,
        match="missing authenticated rollout observation producer",
    ):
        handoff.build_publication_handoff(
            tmp_path / "absent-candidate",
            tmp_path / "absent-candidate-handoff",
            tmp_path / "absent-publication",
            tmp_path / "absent-attestation",
            output,
        )
    assert not output.exists()


@pytest.mark.parametrize(
    "forbidden",
    (
        "--expected-source-commit",
        "--qualification-receipt-id",
        "--validator-binary-sha256",
        "--publisher-controller-sha256",
        "--private-key",
        "--external-signer",
    ),
)
def test_cli_has_no_self_asserted_digest_or_secret_surface(forbidden: str) -> None:
    with pytest.raises(SystemExit):
        handoff.build_parser().parse_args(["candidate", forbidden, "value"])
