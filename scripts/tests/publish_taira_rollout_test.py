from __future__ import annotations

import hashlib
import json
import os
import stat
from dataclasses import dataclass
from pathlib import Path

import pytest

from scripts import publish_taira_rollout as publisher


def _sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


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
    return publisher.canonical_json_bytes(value)


def _receipt_signers() -> dict[str, dict[str, object]]:
    result: dict[str, dict[str, object]] = {}
    x = 1
    for slug in publisher.admission.SLUGS:
        while True:
            payload = "02" + format(x, "064x")
            try:
                node_id = publisher.admission._receipt_node_id(payload, "test signer")
            except publisher.admission.TairaRolloutAdmissionError:
                x += 1
                continue
            break
        result[slug] = {
            "node_id": node_id,
            "public_key": {"algorithm": "secp256k1", "payload_hex": payload},
        }
        x += 1
    return result


def _write(path: Path, payload: bytes, mode: int) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    path.write_bytes(payload)
    path.chmod(mode)


@dataclass
class Harness:
    request: publisher.PublishRequest
    fake_oras: FakeOras
    candidate: Path
    output: Path
    tools: dict[str, Path]


class FakeOras:
    def __init__(self) -> None:
        self.calls: list[tuple[tuple[str, ...], Path, dict[str, str]]] = []
        self.primary_raw = b""
        self.receipt_raw = b""
        self.primary_digest = ""
        self.receipt_digest = ""
        self.primary_source: Path | None = None
        self.receipt_source: Path | None = None
        self.primary_layers: list[tuple[str, str, bytes]] = []
        self.receipt_layers: list[tuple[str, str, bytes]] = []
        self.mutation: str | None = None
        self.fail_stage: str | None = None

    @staticmethod
    def _annotation(arguments: list[str]) -> str:
        raw = arguments[arguments.index("--annotation") + 1]
        return raw.split("=", 1)[1]

    @staticmethod
    def _result(
        repository: str,
        digest: str,
        size: int,
        artifact_type: str,
        created: str,
        tag: str | None,
    ) -> bytes:
        value: dict[str, object] = {
            "annotations": {"org.opencontainers.image.created": created},
            "artifactType": artifact_type,
            "digest": digest,
            "mediaType": publisher.OCI_MANIFEST_MEDIA_TYPE,
            "reference": f"{repository}@{digest}",
            "size": size,
        }
        if tag is not None:
            value["referenceByTags"] = [tag]
        return _pretty(value)

    @staticmethod
    def _manifest(
        artifact_type: str,
        layers: list[tuple[str, str, bytes]],
        created: str,
        subject: tuple[str, int] | None = None,
    ) -> bytes:
        value: dict[str, object] = {
            "annotations": {"org.opencontainers.image.created": created},
            "artifactType": artifact_type,
            "config": {
                "data": publisher.OCI_EMPTY_CONFIG_DATA,
                "digest": publisher.OCI_EMPTY_CONFIG_DIGEST,
                "mediaType": publisher.OCI_EMPTY_CONFIG_MEDIA_TYPE,
                "size": 2,
            },
            "layers": [
                {
                    "annotations": {"org.opencontainers.image.title": path},
                    "digest": f"sha256:{_sha(payload)}",
                    "mediaType": media_type,
                    "size": len(payload),
                }
                for path, media_type, payload in layers
            ],
            "mediaType": publisher.OCI_MANIFEST_MEDIA_TYPE,
            "schemaVersion": 2,
        }
        if subject is not None:
            value["subject"] = {
                "digest": subject[0],
                "mediaType": publisher.OCI_MANIFEST_MEDIA_TYPE,
                "size": subject[1],
            }
        return json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")

    def _stage(self, stage: str) -> None:
        if self.fail_stage == stage:
            raise publisher.TairaPublicationError(f"forced {stage} failure")

    def __call__(
        self,
        argv,
        *,
        cwd: Path,
        environment,
        output_limit: int,
        timeout: int = publisher.CHILD_TIMEOUT_SECONDS,
    ) -> bytes:
        del output_limit, timeout
        arguments = list(argv[1:])
        self.calls.append((tuple(argv), cwd, dict(environment)))
        command = arguments[0]
        if command == "version":
            self._stage("version")
            return b"Version: 1.3.2\nGo version: go-test\nOS/Arch: darwin/arm64\n"
        if command == "push":
            self._stage("push")
            created = self._annotation(arguments)
            reference_index = arguments.index("json") + 1
            tagged = arguments[reference_index]
            repository = tagged.rsplit(":", 1)[0]
            self.primary_source = cwd
            self.primary_layers = []
            for spec in arguments[reference_index + 1 :]:
                path, media_type = spec.rsplit(":", 1)
                self.primary_layers.append((path, media_type, (cwd / path).read_bytes()))
            self.primary_raw = self._manifest(
                publisher.PRIMARY_ARTIFACT_TYPE,
                self.primary_layers,
                created,
            )
            self.primary_digest = f"sha256:{_sha(self.primary_raw)}"
            payload = self._result(
                repository,
                self.primary_digest,
                len(self.primary_raw),
                publisher.PRIMARY_ARTIFACT_TYPE,
                created,
                tagged,
            )
            return self._mutate_json_output("push-output", payload)
        if command == "attach":
            self._stage("attach")
            created = self._annotation(arguments)
            reference_index = arguments.index("json") + 1
            subject_reference = arguments[reference_index]
            repository = subject_reference.split("@", 1)[0]
            self.receipt_source = cwd
            self.receipt_layers = []
            for spec in arguments[reference_index + 1 :]:
                path, media_type = spec.rsplit(":", 1)
                self.receipt_layers.append((path, media_type, (cwd / path).read_bytes()))
            subject = (self.primary_digest, len(self.primary_raw))
            if self.mutation == "old-subject-swap":
                subject = ("sha256:" + "a" * 64, len(self.primary_raw))
            self.receipt_raw = self._manifest(
                publisher.PUBLICATION_ARTIFACT_TYPE,
                self.receipt_layers,
                created,
                subject,
            )
            self.receipt_digest = f"sha256:{_sha(self.receipt_raw)}"
            payload = self._result(
                repository,
                self.receipt_digest,
                len(self.receipt_raw),
                publisher.PUBLICATION_ARTIFACT_TYPE,
                created,
                None,
            )
            return self._mutate_json_output("attach-output", payload)
        if command == "resolve":
            reference = arguments[-1]
            receipt = bool(self.receipt_digest and reference.endswith(self.receipt_digest))
            self._stage("receipt-resolve" if receipt else "primary-resolve")
            digest = self.receipt_digest if receipt else self.primary_digest
            payload = (digest + "\n").encode("ascii")
            if self.mutation == "duplicate-resolve":
                payload += payload
            if self.mutation == "truncated-resolve":
                payload = payload.rstrip(b"\n")
            return payload
        if command == "manifest":
            reference = arguments[-1]
            receipt = bool(self.receipt_digest and reference.endswith(self.receipt_digest))
            stage = "receipt-fetch" if receipt else "primary-fetch"
            self._stage(stage)
            payload = self.receipt_raw if receipt else self.primary_raw
            return self._mutate_manifest(stage, payload)
        if command == "pull":
            reference = arguments[-1]
            receipt = bool(self.receipt_digest and reference.endswith(self.receipt_digest))
            stage = "receipt-pull" if receipt else "primary-pull"
            self._stage(stage)
            destination = Path(arguments[arguments.index("--output") + 1])
            layers = self.receipt_layers if receipt else self.primary_layers
            for relative, _media_type, payload in layers:
                target = destination / relative
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                target.parent.chmod(0o700)
                target.write_bytes(payload)
                target.chmod(0o600)
            if self.mutation == "pulled-byte-swap" and not receipt:
                target = destination / layers[0][0]
                target.write_bytes(b"old but previously valid candidate")
            if self.mutation == "pulled-receipt-swap" and receipt:
                target = destination / publisher.PUBLICATION_RECEIPT_NAME
                target.write_bytes(_pretty({"schema": "old-valid-receipt"}))
            if self.mutation == "pull-extra" and not receipt:
                (destination / "unexpected").write_bytes(b"unexpected")
            return b""
        raise AssertionError(f"unexpected fake ORAS command: {arguments}")

    def _mutate_json_output(self, stage: str, payload: bytes) -> bytes:
        if self.mutation == f"{stage}-duplicate":
            return b'{"digest":"sha256:' + b"0" * 64 + b'",' + payload[1:]
        if self.mutation == f"{stage}-truncated":
            return payload[:-2]
        if self.mutation == f"{stage}-extra":
            value = json.loads(payload)
            value["unexpected"] = True
            return _pretty(value)
        return payload

    def _mutate_manifest(self, stage: str, payload: bytes) -> bytes:
        if self.mutation == f"{stage}-duplicate":
            return b'{"schemaVersion":2,' + payload[1:]
        if self.mutation == f"{stage}-truncated":
            return payload[:-1]
        if self.mutation == f"{stage}-extra":
            value = json.loads(payload)
            value["unexpected"] = True
            mutated = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
            if stage == "primary-fetch":
                self.primary_digest = f"sha256:{_sha(mutated)}"
            return mutated
        return payload


def _admission_result(
    archive_path: Path,
    expected_source: publisher.admission.SourceIdentity,
    expected_receipt_id: str,
    trusted_signing_fingerprint: str,
    trusted_release_manifest_verifier_sha256: str,
) -> dict[str, object]:
    digest = _sha(archive_path.read_bytes())
    fixed = _sha(b"fixed")
    receipt_signers = _receipt_signers()
    return {
        "artifact_handoff_sha256": fixed,
        "archive_sha256": digest,
        "boi_artifact_inventory_sha256": fixed,
        "deployment_performed": False,
        "linux_authority_manifest_sha256": fixed,
        "macos_end_block_hash": fixed,
        "macos_end_height": 2,
        "peer_count": 4,
        "privacy_protocol_receipt_id": fixed,
        "receipt_id": expected_receipt_id,
        "receipt_signers": receipt_signers,
        "release_manifest_sha256": fixed,
        "release_manifest_verifier_sha256": (
            trusted_release_manifest_verifier_sha256
        ),
        "reset_manifest_sha256": fixed,
        "restart_generation": fixed,
        "schema": publisher.admission.VERIFICATION_SCHEMA,
        "schema_version": publisher.admission.VERIFICATION_SCHEMA_VERSION,
        "signer_fingerprint_sha256": trusted_signing_fingerprint,
        "source": expected_source.as_dict(),
        "supervisor_sha256": fixed,
        "validator_binary_sha256": fixed,
        "validator_config_sha256": {
            f"taira-validator-{index}": fixed for index in range(1, 5)
        },
        "verified": True,
    }


def _candidate(root: Path, source, receipt_id: str) -> Path:
    candidate = root / "candidate"
    admission_dir = candidate / "admission"
    authority = candidate / "authority"
    admission_dir.mkdir(mode=0o700, parents=True)
    authority.mkdir(mode=0o700)
    archive_name = (
        f"taira-admission-{source.workspace_source_manifest_sha256[:16]}-"
        "macos-arm64.tar.gz"
    )
    payloads = {
        f"admission/{archive_name}": b"authenticated candidate archive\n",
        "authority/release_manifest.json": _pretty({"signed": "candidate"}),
        "authority/release_manifest.json.pub": b"p" * 32,
        "authority/release_manifest.json.sig": b"s" * 64,
        publisher.RECEIPT_ID_NAME: (receipt_id + "\n").encode("ascii"),
        publisher.SOURCE_IDENTITY_NAME: _compact(
            {"source": source.as_dict(), "source_date_epoch": 1_750_000_000}
        ),
    }
    for relative, payload in payloads.items():
        _write(candidate / relative, payload, 0o444)
    rows = [
        {"path": path, "sha256": _sha(payload), "size": len(payload)}
        for path, payload in sorted(payloads.items())
    ]
    _write(
        candidate / publisher.HANDOFF_MANIFEST,
        _compact(
            {
                "files": rows,
                "kind": "candidate",
                "schema": "iroha.taira.release_handoff",
                "schema_version": 1,
            }
        ),
        0o444,
    )
    admission_dir.chmod(0o555)
    authority.chmod(0o555)
    candidate.chmod(0o555)
    return candidate


@pytest.fixture
def harness(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Harness:
    authority_uid = os.geteuid()
    if authority_uid == 0:
        pytest.skip("authority-UID ownership tests require a non-root test process")
    monkeypatch.setattr(
        publisher,
        "_is_root_owned",
        lambda info: info.st_uid in {0, authority_uid},
    )
    for name in publisher.FORBIDDEN_CREDENTIAL_ENV:
        monkeypatch.delenv(name, raising=False)
    source = publisher.admission.SourceIdentity(
        "1" * 40,
        "2" * 40,
        _sha(b"Cargo.lock"),
        _sha(b"source manifest"),
    )
    receipt_id = _sha(b"qualification receipt")
    candidate = _candidate(tmp_path, source, receipt_id)
    scratch = tmp_path / "authority-scratch"
    scratch.mkdir(mode=0o700)
    registry = tmp_path / "registry-private"
    registry.mkdir(mode=0o700)
    registry_config = registry / "config.json"
    _write(registry_config, b'{"auths":{"registry.example":{"auth":"secret"}}}\n', 0o400)
    tools_root = tmp_path / "trusted-tools"
    tools_root.mkdir(mode=0o755)
    tools = {
        "oras": tools_root / "oras",
        "signer": tools_root / "signer",
        "verifier": tools_root / "sorafs-validate",
        "key": tools_root / "release.pub",
    }
    _write(tools["oras"], b"pinned oras executable", 0o555)
    _write(tools["signer"], b"pinned hsm signer", 0o555)
    _write(tools["verifier"], b"pinned native verifier", 0o555)
    _write(tools["key"], bytes(range(1, 33)), 0o444)
    output = tmp_path / "publication-handoff"
    request = publisher.PublishRequest(
        candidate_root=candidate,
        expected_source=source,
        expected_qualification_receipt_id=receipt_id,
        repository="registry.example/hyperledger/iroha-taira",
        suffix="first-release",
        authority_uid=authority_uid,
        scratch_parent=scratch,
        registry_config=registry_config,
        oras_path=tools["oras"],
        trusted_oras_sha256=_sha(tools["oras"].read_bytes()),
        expected_oras_version="1.3.2",
        external_signer_path=tools["signer"],
        trusted_external_signer_sha256=_sha(tools["signer"].read_bytes()),
        signing_public_key_path=tools["key"],
        trusted_signing_fingerprint=_sha(tools["key"].read_bytes()),
        release_manifest_verifier_path=tools["verifier"],
        trusted_release_manifest_verifier_sha256=_sha(
            tools["verifier"].read_bytes()
        ),
        terminal_handoff=output,
    )
    fake_oras = FakeOras()
    monkeypatch.setattr(publisher, "_run_child", fake_oras)

    def admit(**kwargs):
        return _admission_result(
            kwargs["archive_path"],
            kwargs["expected_source"],
            kwargs["expected_receipt_id"],
            kwargs["trusted_signing_fingerprint"],
            kwargs["trusted_release_manifest_verifier_sha256"],
        )

    monkeypatch.setattr(publisher.admission, "verify_admission", admit)

    def sign(
        manifest_path,
        _signer,
        raw_public_key_path,
        _fingerprint,
        signature_output_path,
        public_key_output_path,
        _verifier,
        _verifier_sha,
    ):
        assert Path(manifest_path).name == publisher.PUBLICATION_RECEIPT_NAME
        _write(Path(signature_output_path), b"S" * 64, 0o644)
        _write(Path(public_key_output_path), Path(raw_public_key_path).read_bytes(), 0o644)
        return {"signature_verified": True}

    monkeypatch.setattr(publisher, "sign_release_manifest", sign)
    monkeypatch.setattr(
        publisher,
        "verify_release_manifest",
        lambda *_args, **_kwargs: {"signature_verified": True},
    )
    return Harness(request, fake_oras, candidate, output, tools)


def _rejects_without_handoff(harness: Harness, match: str | None = None) -> None:
    with pytest.raises(publisher.TairaPublicationError, match=match):
        publisher._publish_after_authenticated_rollout_observation(
            harness.request, now_unix=1_800_000_000
        )
    assert not harness.output.exists()


def test_publish_closes_exact_seven_file_handoff_and_fixed_child_surface(
    harness: Harness,
) -> None:
    result = publisher._publish_after_authenticated_rollout_observation(
        harness.request, now_unix=1_800_000_000
    )

    assert sorted(path.name for path in harness.output.iterdir()) == sorted(
        publisher.TERMINAL_FILES
    )
    assert stat.S_IMODE(harness.output.stat().st_mode) == 0o555
    assert all(
        stat.S_IMODE(path.stat().st_mode) == 0o444
        for path in harness.output.iterdir()
    )
    assert result["primary_digest"] == harness.fake_oras.primary_digest
    assert result["receipt_digest"] == harness.fake_oras.receipt_digest
    receipt = json.loads(
        (harness.output / publisher.PUBLICATION_RECEIPT_NAME).read_bytes()
    )
    assert receipt["source"] == harness.request.expected_source.as_dict()
    assert receipt["qualification_receipt_id"] == (
        harness.request.expected_qualification_receipt_id
    )
    assert receipt["repository"] == harness.request.repository
    assert receipt["suffix"] == harness.request.suffix
    assert receipt["issued_at_unix"] == 1_800_000_000
    assert len(receipt["layers"]) == 4
    for argv, _cwd, environment in harness.fake_oras.calls:
        assert argv[0] == str(harness.tools["oras"])
        assert set(environment) == {
            "HOME",
            "LANG",
            "LC_ALL",
            "PATH",
            "TMPDIR",
            "XDG_CACHE_HOME",
            "XDG_CONFIG_HOME",
        }
        assert not set(environment) & publisher.FORBIDDEN_CREDENTIAL_ENV
        assert not any(
            argument.split("=", 1)[0] in publisher.FORBIDDEN_ORAS_FLAGS
            for argument in argv
        )
    push = next(argv for argv, _cwd, _env in harness.fake_oras.calls if argv[1] == "push")
    assert len([arg for arg in push if ":application/" in arg]) == 4
    attach = next(
        argv for argv, _cwd, _env in harness.fake_oras.calls if argv[1] == "attach"
    )
    assert len([arg for arg in attach if ":application/" in arg]) == 3


@pytest.mark.parametrize(
    "forbidden",
    (
        "--manifest",
        "--json",
        "--username",
        "--password",
        "--identity-token",
        "--layer",
        "sign",
    ),
)
def test_cli_has_no_arbitrary_json_layer_credential_or_sign_surface(
    forbidden: str,
) -> None:
    parser = publisher._build_parser()
    with pytest.raises(SystemExit):
        parser.parse_args([forbidden])


@pytest.mark.parametrize(
    "field",
    (
        "commit",
        "dpn_validator_release_commit",
        "cargo_lock_sha256",
        "workspace_source_manifest_sha256",
        "qualification_receipt",
    ),
)
def test_expected_source_and_qualification_mutations_fail_before_publish(
    harness: Harness,
    field: str,
) -> None:
    source = harness.request.expected_source
    if field == "commit":
        source = publisher.admission.SourceIdentity(
            "a" * 40,
            source.dpn_validator_release_commit,
            source.cargo_lock_sha256,
            source.workspace_source_manifest_sha256,
        )
    elif field == "dpn_validator_release_commit":
        source = publisher.admission.SourceIdentity(
            source.commit,
            "b" * 40,
            source.cargo_lock_sha256,
            source.workspace_source_manifest_sha256,
        )
    elif field == "cargo_lock_sha256":
        source = publisher.admission.SourceIdentity(
            source.commit,
            source.dpn_validator_release_commit,
            "c" * 64,
            source.workspace_source_manifest_sha256,
        )
    elif field == "workspace_source_manifest_sha256":
        source = publisher.admission.SourceIdentity(
            source.commit,
            source.dpn_validator_release_commit,
            source.cargo_lock_sha256,
            "d" * 64,
        )
    else:
        harness.request = publisher.PublishRequest(
            **{
                **harness.request.__dict__,
                "expected_qualification_receipt_id": "e" * 64,
            }
        )
    if field != "qualification_receipt":
        harness.request = publisher.PublishRequest(
            **{**harness.request.__dict__, "expected_source": source}
        )
    _rejects_without_handoff(harness)


@pytest.mark.parametrize(
    "mutation",
    (
        "writable-root",
        "writable-file",
        "extra-file",
        "symlink-file",
        "hardlink-file",
    ),
)
def test_candidate_must_remain_exact_root_frozen_and_unaliased(
    harness: Harness,
    mutation: str,
) -> None:
    if mutation == "writable-root":
        harness.candidate.chmod(0o755)
    elif mutation == "writable-file":
        (harness.candidate / publisher.RECEIPT_ID_NAME).chmod(0o644)
    elif mutation == "extra-file":
        harness.candidate.chmod(0o755)
        _write(harness.candidate / "extra", b"extra", 0o444)
        harness.candidate.chmod(0o555)
    elif mutation == "symlink-file":
        authority = harness.candidate / "authority"
        authority.chmod(0o755)
        target = authority / "release_manifest.json.sig"
        target.unlink()
        target.symlink_to(authority / "release_manifest.json.pub")
        authority.chmod(0o555)
    else:
        authority = harness.candidate / "authority"
        authority.chmod(0o755)
        os.link(
            authority / "release_manifest.json.sig",
            authority / "signature-hardlink",
        )
        authority.chmod(0o555)
    _rejects_without_handoff(harness)


@pytest.mark.parametrize(
    "mutation",
    (
        "wrong-oras-digest",
        "writable-oras",
        "oras-symlink",
        "oras-hardlink",
        "writable-config",
        "config-symlink",
        "config-hardlink",
    ),
)
def test_tool_and_registry_config_substitution_is_rejected(
    harness: Harness,
    mutation: str,
) -> None:
    if mutation == "wrong-oras-digest":
        harness.request = publisher.PublishRequest(
            **{**harness.request.__dict__, "trusted_oras_sha256": "a" * 64}
        )
    elif mutation == "writable-oras":
        harness.tools["oras"].chmod(0o755)
    elif mutation == "oras-symlink":
        path = harness.tools["oras"]
        path.unlink()
        path.symlink_to(harness.tools["signer"])
    elif mutation == "oras-hardlink":
        os.link(harness.tools["oras"], harness.tools["oras"].with_suffix(".alias"))
    elif mutation == "writable-config":
        harness.request.registry_config.chmod(0o600)
    elif mutation == "config-symlink":
        path = harness.request.registry_config
        payload = path.read_bytes()
        path.unlink()
        target = path.with_suffix(".real")
        _write(target, payload, 0o400)
        path.symlink_to(target)
    else:
        os.link(
            harness.request.registry_config,
            harness.request.registry_config.with_suffix(".alias"),
        )
    _rejects_without_handoff(harness)


@pytest.mark.parametrize(
    "name",
    tuple(sorted(publisher.FORBIDDEN_CREDENTIAL_ENV)),
)
def test_secret_environment_injection_is_rejected_not_forwarded(
    harness: Harness,
    monkeypatch: pytest.MonkeyPatch,
    name: str,
) -> None:
    monkeypatch.setenv(name, "attacker-secret")
    _rejects_without_handoff(harness, match="credential environment")
    assert not harness.fake_oras.calls


@pytest.mark.parametrize(
    "mutation",
    (
        "push-output-duplicate",
        "push-output-truncated",
        "push-output-extra",
        "primary-fetch-duplicate",
        "primary-fetch-truncated",
        "primary-fetch-extra",
        "duplicate-resolve",
        "truncated-resolve",
        "attach-output-duplicate",
        "attach-output-truncated",
        "attach-output-extra",
        "receipt-fetch-duplicate",
        "receipt-fetch-truncated",
        "receipt-fetch-extra",
    ),
)
def test_oras_duplicate_noncanonical_and_truncated_output_is_rejected(
    harness: Harness,
    mutation: str,
) -> None:
    harness.fake_oras.mutation = mutation
    _rejects_without_handoff(harness)


@pytest.mark.parametrize(
    "mutation",
    (
        "old-subject-swap",
        "pulled-byte-swap",
        "pulled-receipt-swap",
        "pull-extra",
    ),
)
def test_old_valid_subject_receipt_and_layer_swaps_are_rejected(
    harness: Harness,
    mutation: str,
) -> None:
    harness.fake_oras.mutation = mutation
    _rejects_without_handoff(harness)


@pytest.mark.parametrize(
    "field",
    (
        "admission_sha256",
        "immutable_reference",
        "issued_at_unix",
        "layers",
        "oras",
        "qualification_receipt_id",
        "repository",
        "source",
        "subject",
        "suffix",
        "tag",
        "tagged_reference",
    ),
)
def test_publication_receipt_semantic_mutations_are_rejected(field: str) -> None:
    source = publisher.admission.SourceIdentity(
        "1" * 40, "2" * 40, "3" * 64, "4" * 64
    )
    layer = publisher.Layer("candidate", "application/test", "5" * 64, 9)
    request = publisher.PublishRequest(
        Path("/candidate"),
        source,
        "6" * 64,
        "registry.example/repo",
        "suffix",
        501,
        Path("/scratch"),
        Path("/config"),
        Path("/oras"),
        "7" * 64,
        "1.3.2",
        Path("/signer"),
        "8" * 64,
        Path("/key"),
        "9" * 64,
        Path("/verifier"),
        "a" * 64,
        Path("/output"),
    )
    expected = publisher._receipt_value(
        request=request,
        admission_payload=b"current admission",
        layers=[layer],
        primary_digest="sha256:" + "b" * 64,
        primary_size=100,
        tagged_reference="registry.example/repo:tag",
        immutable_reference="registry.example/repo@sha256:" + "b" * 64,
        tag="tag",
        issued_at_unix=1_800_000_000,
    )
    changed = json.loads(_pretty(expected))
    if field == "layers":
        changed[field][0]["sha256"] = "c" * 64
    elif field == "oras":
        changed[field]["version"] = "1.3.1"
    elif field == "source":
        changed[field]["dpn_validator_release_commit"] = "d" * 40
    elif field == "subject":
        changed[field]["digest"] = "sha256:" + "e" * 64
    elif field == "issued_at_unix":
        changed[field] += 1
    else:
        changed[field] = "mutated"
    with pytest.raises(publisher.TairaPublicationError, match="semantics"):
        publisher._validate_publication_receipt(_pretty(changed), expected=expected)


def test_old_admission_result_cannot_be_reused_for_current_pulled_subject(
    harness: Harness,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls = 0

    def admit(**kwargs):
        nonlocal calls
        calls += 1
        value = _admission_result(
            kwargs["archive_path"],
            kwargs["expected_source"],
            kwargs["expected_receipt_id"],
            kwargs["trusted_signing_fingerprint"],
            kwargs["trusted_release_manifest_verifier_sha256"],
        )
        if calls == 2:
            value["macos_end_height"] = 1
        return value

    monkeypatch.setattr(publisher.admission, "verify_admission", admit)
    _rejects_without_handoff(harness, match="admission differs")


def test_current_admission_shape_is_preserved_in_publication_binding(
    harness: Harness,
    tmp_path: Path,
) -> None:
    archive = next((harness.candidate / "admission").iterdir())
    scratch = tmp_path / "current-admission-shape"
    scratch.mkdir(mode=0o700)

    payload = publisher._admission_bytes(
        archive,
        harness.candidate / "authority",
        harness.request,
        scratch,
        1_800_000_000,
        "current",
    )

    result = json.loads(payload)
    assert result["boi_artifact_inventory_sha256"] == _sha(b"fixed")
    assert result["privacy_protocol_receipt_id"] == _sha(b"fixed")
    assert result["receipt_signers"] == _receipt_signers()


@pytest.mark.parametrize(
    "mutation",
    (
        "omit-boi-inventory",
        "omit-privacy-receipt",
        "omit-receipt-signers",
        "malformed-boi-inventory",
        "malformed-privacy-receipt",
        "tampered-receipt-signer",
    ),
)
def test_incomplete_or_tampered_current_admission_shape_is_rejected(
    harness: Harness,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
) -> None:
    archive = next((harness.candidate / "admission").iterdir())
    original = publisher.admission.verify_admission

    def admit(**kwargs):
        result = original(**kwargs)
        if mutation == "omit-boi-inventory":
            result.pop("boi_artifact_inventory_sha256")
        elif mutation == "omit-privacy-receipt":
            result.pop("privacy_protocol_receipt_id")
        elif mutation == "omit-receipt-signers":
            result.pop("receipt_signers")
        elif mutation == "malformed-boi-inventory":
            result["boi_artifact_inventory_sha256"] = "not-a-digest"
        elif mutation == "malformed-privacy-receipt":
            result["privacy_protocol_receipt_id"] = "not-a-digest"
        else:
            result["receipt_signers"][publisher.admission.SLUGS[0]]["node_id"] = (
                "taira-node:receipt-signer:secp256k1:sha256:" + "0" * 64
            )
        return result

    monkeypatch.setattr(publisher.admission, "verify_admission", admit)
    scratch = tmp_path / f"invalid-{mutation}"
    scratch.mkdir(mode=0o700)
    with pytest.raises(publisher.TairaPublicationError):
        publisher._admission_bytes(
            archive,
            harness.candidate / "authority",
            harness.request,
            scratch,
            1_800_000_000,
            "invalid",
        )


def test_executable_and_config_toctou_are_detected(
    harness: Harness,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    original = harness.fake_oras
    mutated = False

    def racing(*args, **kwargs):
        nonlocal mutated
        payload = original(*args, **kwargs)
        if not mutated:
            mutated = True
            path = harness.request.registry_config
            path.chmod(0o600)
            path.write_bytes(b'{"auths":{"attacker":{}}}\n')
            path.chmod(0o400)
        return payload

    monkeypatch.setattr(publisher, "_run_child", racing)
    _rejects_without_handoff(harness, match="registry config changed")


@pytest.mark.parametrize(
    "stage",
    (
        "version",
        "push",
        "primary-resolve",
        "primary-fetch",
        "primary-pull",
        "attach",
        "receipt-resolve",
        "receipt-fetch",
        "receipt-pull",
    ),
)
def test_partial_child_failures_never_create_signed_terminal_handoff(
    harness: Harness,
    stage: str,
) -> None:
    harness.fake_oras.fail_stage = stage
    _rejects_without_handoff(harness, match="forced")
    assert not list(harness.request.scratch_parent.glob("taira-publish-authority-*"))


def test_partial_signing_failure_never_creates_terminal_handoff(
    harness: Harness,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def fail_sign(*_args, **_kwargs):
        raise publisher.ReleaseManifestSignatureError("HSM refused")

    monkeypatch.setattr(publisher, "sign_release_manifest", fail_sign)
    with pytest.raises(publisher.ReleaseManifestSignatureError, match="HSM"):
        publisher._publish_after_authenticated_rollout_observation(
            harness.request, now_unix=1_800_000_000
        )
    assert not harness.output.exists()
    assert not list(harness.request.scratch_parent.glob("taira-publish-authority-*"))


def test_post_rename_failure_rolls_back_exact_terminal_inode(
    harness: Harness,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    real_rename = publisher.os.rename

    def rename_then_fail(source: Path, destination: Path) -> None:
        real_rename(source, destination)
        raise OSError("simulated post-rename failure")

    monkeypatch.setattr(publisher.os, "rename", rename_then_fail)
    with pytest.raises(OSError, match="post-rename"):
        publisher._publish_after_authenticated_rollout_observation(
            harness.request, now_unix=1_800_000_000
        )
    assert not harness.output.exists()
    assert not list(harness.output.parent.glob(f".{harness.output.name}.pending-*"))
    assert not list(harness.request.scratch_parent.glob("taira-publish-authority-*"))


@pytest.mark.parametrize(
    "now_unix",
    (True, 0, -1, publisher.MAX_PUBLICATION_UNIX + 1),
)
def test_noncanonical_publication_time_is_rejected_before_side_effects(
    harness: Harness,
    now_unix: int,
) -> None:
    with pytest.raises(publisher.TairaPublicationError, match="publication issue time"):
        publisher._publish_after_authenticated_rollout_observation(
            harness.request, now_unix=now_unix
        )
    assert harness.fake_oras.calls == []
    assert not harness.output.exists()
