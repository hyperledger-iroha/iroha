"""Authenticated in-process contract tests for the seven Python authority paths."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from typing import Optional

import pytest

from scripts import build_privacy_v1_boi_handoff as qualification
from scripts import deploy_taira_v21_reset as deploy
from scripts import taira_authority_client as client
from scripts import taira_privacy_governance_authority as governance
from scripts import taira_privacy_protocol_receipt as protocol
from scripts import taira_privacy_rollout_contract as rollout
from scripts import taira_public_soak_authority_contract as public_soak
from scripts import taira_release_authority as native
from scripts.tests import build_privacy_v1_boi_handoff_test as qualification_fixture
from scripts.tests import taira_privacy_governance_authority_test as governance_fixture
from scripts.tests import taira_privacy_protocol_receipt_test as protocol_fixture
from scripts.tests import taira_privacy_rollout_contract_test as rollout_fixture
from scripts.tests import taira_public_soak_authority_contract_test as soak_fixture
from scripts.tests import taira_release_authority_test as native_fixture


def _canonical(value: object) -> bytes:
    return client.canonical_json_bytes(value)


def _digest(value: object) -> str:
    return hashlib.sha256(_canonical(value)).hexdigest()


def _changed(value: object) -> object:
    if isinstance(value, bool):
        return not value
    if isinstance(value, int):
        return value + 1
    if value is None:
        return "mutated"
    if isinstance(value, str):
        if value and all(character in "0123456789abcdef" for character in value):
            return ("1" if value[0] != "1" else "2") + value[1:]
        return value + "-mutated"
    raise AssertionError(f"unsupported scalar mutation: {type(value).__name__}")


def _scalar_mutations(value: Mapping[str, object]) -> list[tuple[str, dict[str, object]]]:
    mutations: list[tuple[str, dict[str, object]]] = []

    def visit(node: object, path: tuple[object, ...]) -> None:
        if isinstance(node, dict):
            for key, item in node.items():
                visit(item, (*path, key))
            return
        if isinstance(node, list):
            for index, item in enumerate(node):
                visit(item, (*path, index))
            return
        changed = copy.deepcopy(value)
        target: object = changed
        for component in path[:-1]:
            if isinstance(component, int):
                assert isinstance(target, list)
                target = target[component]
            else:
                assert isinstance(target, dict)
                target = target[component]
        last = path[-1]
        if isinstance(last, int):
            assert isinstance(target, list)
            target[last] = _changed(node)
        else:
            assert isinstance(target, dict)
            target[last] = _changed(node)
        mutations.append((".".join(map(str, path)), changed))

    visit(value, ())
    return mutations


@dataclass(frozen=True)
class _Record:
    subject: dict[str, object]
    manifest: tuple[dict[str, object], ...]
    authority_envelope: dict[str, object]
    durable_receipt: dict[str, object]


ResultFactory = Callable[
    [str, dict[str, object], tuple[dict[str, object], ...], dict[str, object], dict[str, object]],
    tuple[dict[str, object], dict[str, object], Optional[dict[str, object]]],
]


class FakeAuthority:
    """Strict authenticated client double with exact historical recovery."""

    def __init__(self) -> None:
        self.available = True
        self.events: list[tuple[str, str, str | None]] = []
        self.records: dict[tuple[str, str], _Record] = {}
        self.deployment_results: dict[
            str, tuple[str, str, client.AuthorityResult]
        ] = {}
        self.factories: dict[str, ResultFactory] = {}
        self.apply_consumptions = 0
        self.finalizations = 0

    def install(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setattr(client, "preflight", self.preflight)
        monkeypatch.setattr(client, "authorize", self.authorize)
        monkeypatch.setattr(client, "verify_receipt", self.verify_receipt)
        monkeypatch.setattr(client, "finalize_deployment", self.finalize_deployment)

    def preflight(self, role: str) -> dict[str, object]:
        self.events.append(("preflight", role, None))
        if not self.available:
            raise client.TairaAuthorityClientError("authenticated fake unavailable")
        return {"role": role, "status": "ready"}

    @staticmethod
    def _manifest(
        artifacts: Sequence[client.Artifact],
    ) -> tuple[dict[str, object], ...]:
        rows: list[dict[str, object]] = []
        for ordinal, artifact in enumerate(artifacts):
            payload = Path(artifact.path).read_bytes()
            rows.append(
                {
                    "name": artifact.name,
                    "ordinal": ordinal,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "size": len(payload),
                }
            )
        return tuple(rows)

    @staticmethod
    def _ids(
        role: str,
        subject: Mapping[str, object],
        manifest: Sequence[Mapping[str, object]],
    ) -> tuple[str, str]:
        run_id = client.derive_run_id(role, subject)
        return run_id, client._operation_id(role, run_id, subject, manifest)

    @staticmethod
    def _generic_sidecars(
        role: str,
        subject: Mapping[str, object],
        manifest: Sequence[Mapping[str, object]],
    ) -> tuple[dict[str, object], dict[str, object]]:
        run_id, operation_id = FakeAuthority._ids(role, subject, manifest)
        envelope_body: dict[str, object] = {
            "artifact_manifest_sha256": _digest(list(manifest)),
            "issued_at_unix_millis": 1_900_000_000_000,
            "key_revision": 1,
            "operation_id": operation_id,
            "role": role,
            "run_id": run_id,
            "schema": "iroha.taira.test-authority-envelope.v1",
            "subject_sha256": _digest(subject),
        }
        envelope = {
            **envelope_body,
            "signature": hashlib.sha256(
                b"test-envelope\0" + _canonical(envelope_body)
            ).hexdigest(),
        }
        receipt_body: dict[str, object] = {
            "authority_envelope_sha256": _digest(envelope),
            "audit_sequence": 1,
            "operation_id": operation_id,
            "role": role,
            "run_id": run_id,
            "schema": "iroha.taira.test-durable-receipt.v1",
        }
        receipt = {
            **receipt_body,
            "signature": hashlib.sha256(
                b"test-receipt\0" + _canonical(receipt_body)
            ).hexdigest(),
        }
        return envelope, receipt

    def _issue(
        self,
        role: str,
        subject: Mapping[str, object],
        artifacts: Sequence[client.Artifact],
        *,
        disposition: str | None,
    ) -> client.AuthorityResult:
        normalized = json.loads(_canonical(subject))
        assert isinstance(normalized, dict)
        manifest = self._manifest(artifacts)
        run_id, operation_id = self._ids(role, normalized, manifest)
        envelope, receipt = self._generic_sidecars(role, normalized, manifest)
        historical_envelope: dict[str, object] | None = None
        factory = self.factories.get(role)
        if factory is not None:
            envelope, receipt, historical_envelope = factory(
                role, normalized, manifest, envelope, receipt
            )
        status = "authorized"
        if disposition == "dry-run":
            status = "verified"
        elif disposition == "apply":
            existing = self.records.get((role, operation_id))
            if existing is not None:
                if existing.subject != normalized or existing.manifest != manifest:
                    raise client.TairaAuthorityClientError(
                        "conflicting deployment lease reuse"
                    )
                status = "replayed"
            else:
                self.apply_consumptions += 1
        if disposition != "dry-run":
            self.records[(role, operation_id)] = _Record(
                subject=normalized,
                manifest=manifest,
                authority_envelope=(historical_envelope or envelope),
                durable_receipt=receipt,
            )
        return client.AuthorityResult(
            role=role,
            operation_id=operation_id,
            run_id=run_id,
            status=status,
            authority_envelope=envelope,
            durable_receipt=receipt,
            artifact_manifest=manifest,
        )

    def authorize(
        self,
        role: str,
        subject: Mapping[str, object],
        *,
        artifacts: Sequence[client.Artifact] = (),
        run_id: str | None = None,
        disposition: str | None = None,
    ) -> client.AuthorityResult:
        del run_id
        self.events.append(("authorize", role, disposition))
        return self._issue(role, subject, artifacts, disposition=disposition)

    def prime(
        self,
        role: str,
        subject: Mapping[str, object],
        *,
        authority_envelope: Mapping[str, object],
        durable_receipt: Mapping[str, object],
    ) -> None:
        normalized = json.loads(_canonical(subject))
        assert isinstance(normalized, dict)
        run_id, operation_id = self._ids(role, normalized, ())
        del run_id
        self.records[(role, operation_id)] = _Record(
            subject=normalized,
            manifest=(),
            authority_envelope=dict(authority_envelope),
            durable_receipt=dict(durable_receipt),
        )

    def verify_receipt(
        self,
        role: str,
        subject: Mapping[str, object],
        *,
        authority_envelope: Mapping[str, object],
        durable_receipt: Mapping[str, object],
        artifacts: Sequence[client.Artifact] = (),
        run_id: str | None = None,
        operation_id: str | None = None,
    ) -> client.AuthorityResult:
        del run_id
        normalized = json.loads(_canonical(subject))
        assert isinstance(normalized, dict)
        manifest = self._manifest(artifacts)
        derived_run, derived_operation = self._ids(role, normalized, manifest)
        if operation_id is not None and operation_id != derived_operation:
            raise client.TairaAuthorityClientError("operation binding differs")
        self.events.append(("verify", role, None))
        record = self.records.get((role, derived_operation))
        if record is None:
            raise client.TairaAuthorityClientError("historical record is absent")
        if (
            record.subject != normalized
            or record.manifest != manifest
            or record.authority_envelope != dict(authority_envelope)
            or record.durable_receipt != dict(durable_receipt)
        ):
            raise client.TairaAuthorityClientError("signed field binding differs")
        return client.AuthorityResult(
            role=role,
            operation_id=derived_operation,
            run_id=derived_run,
            status="valid",
            authority_envelope=dict(authority_envelope),
            durable_receipt=dict(durable_receipt),
            artifact_manifest=manifest,
        )

    def finalize_deployment(
        self,
        subject: Mapping[str, object],
        *,
        lease: client.AuthorityResult,
        outcome: str,
        result_sha256: str,
    ) -> client.AuthorityResult:
        self.events.append(("finalize", "deploy-issuance", outcome))
        record = self.records.get(("deploy-issuance", lease.operation_id))
        if record is None or lease.status not in {"authorized", "replayed"}:
            raise client.TairaAuthorityClientError("deployment lease was not consumed")
        run_id, operation_id = self._ids(
            "deploy-issuance", subject, lease.artifact_manifest
        )
        if run_id != lease.run_id or operation_id != lease.operation_id:
            raise client.TairaAuthorityClientError("deployment lease identity differs")
        existing = self.deployment_results.get(operation_id)
        if existing is not None:
            existing_outcome, existing_digest, existing_result = existing
            if (existing_outcome, existing_digest) != (outcome, result_sha256):
                raise client.TairaAuthorityClientError(
                    "conflicting deployment finalization"
                )
            return client.AuthorityResult(
                role=existing_result.role,
                operation_id=existing_result.operation_id,
                run_id=existing_result.run_id,
                status="replayed",
                authority_envelope=existing_result.authority_envelope,
                durable_receipt=existing_result.durable_receipt,
                artifact_manifest=existing_result.artifact_manifest,
            )
        self.finalizations += 1
        envelope, receipt = self._generic_sidecars(
            "deploy-issuance", subject, lease.artifact_manifest
        )
        receipt = {
            **receipt,
            "deployment_result": {
                "outcome": outcome,
                "result_sha256": result_sha256,
            },
        }
        result = client.AuthorityResult(
            role="deploy-issuance",
            operation_id=operation_id,
            run_id=run_id,
            status="finalized",
            authority_envelope=envelope,
            durable_receipt=receipt,
            artifact_manifest=lease.artifact_manifest,
        )
        self.deployment_results[operation_id] = (outcome, result_sha256, result)
        return result


def _native_cli_args(args: argparse.Namespace, output: Path, command: str) -> list[str]:
    values = [
        command,
        "--evidence-root",
        args.evidence_root,
        "--commit",
        args.commit,
        "--dpn-validator-release-commit",
        args.dpn_validator_release_commit,
        "--signing-fingerprint",
        args.signing_fingerprint,
        "--native-verifier-sha256",
        args.native_verifier_sha256,
        "--archive",
        args.archive,
    ]
    values.extend(("--output" if command == "create" else "--authority", str(output)))
    return values


def test_native_evidence_positive_and_every_sidecar_field_mutation(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.install(monkeypatch)
    args = native_fixture._args(tmp_path)
    output = tmp_path / "native-authority.json"
    assert native.main(_native_cli_args(args, output, "create")) == 0
    assert native.main(_native_cli_args(args, output, "verify")) == 0
    envelope_path, receipt_path = native._sidecar_paths(output)
    for path in (envelope_path, receipt_path):
        original = client.decode_canonical_json(path.read_bytes(), "test sidecar")
        for _label, mutated in _scalar_mutations(original):
            path.write_bytes(_canonical(mutated))
            assert native.main(_native_cli_args(args, output, "verify")) == 1
            path.write_bytes(_canonical(original))
    historical = [event for event in fake.events if event[0] != "preflight"][1:]
    assert historical and all(event[0] == "verify" for event in historical)


def test_privacy_protocol_positive_authorization_binds_every_subject_field(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.install(monkeypatch)
    root = tmp_path / "protocol-evidence"
    receipt_id = protocol_fixture.build_valid_evidence(root)
    result = protocol.validate_evidence_directory(
        root,
        expected_source=protocol_fixture.SOURCE,
        expected_validator_binary_sha256=protocol_fixture.BINDINGS[
            "validator_binary_sha256"
        ],
        expected_linux_release_archive_sha256=protocol_fixture.BINDINGS[
            "linux_release_archive_sha256"
        ],
        expected_exact12_matrix_sha256=protocol_fixture.BINDINGS[
            "exact12_matrix_sha256"
        ],
        expected_artifact_handoff_sha256=protocol_fixture.BINDINGS[
            "artifact_handoff_sha256"
        ],
        expected_receipt_id=receipt_id,
        now_unix=protocol_fixture.NOW,
    )
    assert isinstance(result, protocol.AuthenticatedPrivacyProtocolEvidence)
    assert result["case_count"] == 7
    assert len(result["outcomes"]) == 12
    authorization = next(
        record
        for (role, _operation), record in fake.records.items()
        if role == "privacy-protocol-origin"
    )
    assert result == authorization.subject["structural_subject"]
    envelope_path, receipt_path = result.persist_sidecars()
    assert envelope_path.parent == root.parent
    assert receipt_path.parent == root.parent
    assert envelope_path.read_bytes() == result.authority_envelope
    assert receipt_path.read_bytes() == result.durable_receipt
    assert set(protocol.scan_inventory_paths(root)) == set(protocol.EVIDENCE_NAMES)
    with pytest.raises(protocol.PrivacyProtocolEvidenceError):
        result.persist_sidecars()
    fake.events.clear()
    historical = protocol.verify_authenticated_evidence_directory(
        root,
        expected_source=protocol_fixture.SOURCE,
        expected_validator_binary_sha256=protocol_fixture.BINDINGS[
            "validator_binary_sha256"
        ],
        expected_linux_release_archive_sha256=protocol_fixture.BINDINGS[
            "linux_release_archive_sha256"
        ],
        expected_exact12_matrix_sha256=protocol_fixture.BINDINGS[
            "exact12_matrix_sha256"
        ],
        expected_artifact_handoff_sha256=protocol_fixture.BINDINGS[
            "artifact_handoff_sha256"
        ],
        expected_receipt_id=receipt_id,
        now_unix=protocol_fixture.NOW,
    )
    assert historical == result
    assert [event[0] for event in fake.events if event[0] != "preflight"] == [
        "verify"
    ]
    envelope_value = client.decode_canonical_json(
        result.authority_envelope, "protocol test envelope"
    )
    receipt_value = client.decode_canonical_json(
        result.durable_receipt, "protocol test receipt"
    )
    for path, original in (
        (envelope_path, envelope_value),
        (receipt_path, receipt_value),
    ):
        for _label, mutated in _scalar_mutations(original):
            path.write_bytes(_canonical(mutated))
            with pytest.raises(protocol.PrivacyProtocolEvidenceError):
                protocol.verify_authenticated_evidence_directory(
                    root,
                    expected_source=protocol_fixture.SOURCE,
                    expected_validator_binary_sha256=protocol_fixture.BINDINGS[
                        "validator_binary_sha256"
                    ],
                    expected_linux_release_archive_sha256=protocol_fixture.BINDINGS[
                        "linux_release_archive_sha256"
                    ],
                    expected_exact12_matrix_sha256=protocol_fixture.BINDINGS[
                        "exact12_matrix_sha256"
                    ],
                    expected_artifact_handoff_sha256=protocol_fixture.BINDINGS[
                        "artifact_handoff_sha256"
                    ],
                    expected_receipt_id=receipt_id,
                    now_unix=protocol_fixture.NOW,
                )
            path.write_bytes(_canonical(original))
    assert not any(event[0] == "authorize" for event in fake.events)
    baseline = authorization.authority_envelope["signature"]
    for _label, changed_subject in _scalar_mutations(authorization.subject):
        envelope, _receipt = fake._generic_sidecars(
            "privacy-protocol-origin", changed_subject, authorization.manifest
        )
        assert envelope["signature"] != baseline


def test_governance_positive_historical_and_every_sidecar_field_mutation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    request = governance_fixture._request()
    governance_receipt = governance_fixture._receipt_value(request)

    def result_factory(
        _role: str,
        _subject: dict[str, object],
        _manifest: tuple[dict[str, object], ...],
        envelope: dict[str, object],
        _receipt: dict[str, object],
    ) -> tuple[dict[str, object], dict[str, object], None]:
        return envelope, governance_receipt, None

    fake.factories["privacy-governance"] = result_factory
    fake.install(monkeypatch)
    issued = governance.request_authenticated_governance_transaction_v1(request)
    verified = governance.validate_authenticated_governance_receipt_v1(
        request,
        issued.durable_receipt,
        authority_envelope_payload=issued.authority_envelope,
    )
    assert verified == issued.receipt
    envelope = client.decode_canonical_json(
        issued.authority_envelope, "governance test envelope"
    )
    receipt = client.decode_canonical_json(
        issued.durable_receipt, "governance test receipt"
    )
    for target, original in (("envelope", envelope), ("receipt", receipt)):
        for _label, mutated in _scalar_mutations(original):
            with pytest.raises(governance.PrivacyGovernanceAuthorityError):
                governance.validate_authenticated_governance_receipt_v1(
                    request,
                    _canonical(mutated if target == "receipt" else receipt),
                    authority_envelope_payload=_canonical(
                        mutated if target == "envelope" else envelope
                    ),
                )
    history = fake.events[fake.events.index(("verify", "privacy-governance", None)) :]
    assert history and all(event[0] in {"preflight", "verify"} for event in history)


def _qualification_result_factory(
    fixture: qualification_fixture.Fixture,
) -> ResultFactory:
    def factory(
        _role: str,
        _subject: dict[str, object],
        _manifest: tuple[dict[str, object], ...],
        envelope: dict[str, object],
        receipt: dict[str, object],
    ) -> tuple[dict[str, object], dict[str, object], None]:
        native_member = "iroha_python/_crypto.cpython-312-aarch64-linux-gnu.so"
        catalog = hashlib.sha256(
            qualification_fixture.COMPILED_CATALOG
        ).hexdigest()
        binding = qualification_fixture._valid_capability_binding()
        abi_result = {
            "abi_version": 22,
            "compiled_profile_catalog_sha256": catalog,
            "library_sha256": hashlib.sha256(
                (fixture.root / qualification.ABI_LIBRARY_PATH).read_bytes()
            ).hexdigest(),
            "privacy_c_exports": list(
                qualification.abi22.APPROVED_PRIVACY_C_EXPORTS
            ),
            "result": "passed",
        }
        wheel_result = {
            "capability_binding": binding,
            "capability_binding_sha256": hashlib.sha256(
                qualification.canonical_json_bytes(binding)
            ).hexdigest(),
            "capability_manifest_sha256": hashlib.sha256(
                (fixture.root / qualification.CAPABILITY_PATH).read_bytes()
            ).hexdigest(),
            "compiled_profile_catalog_sha256": catalog,
            "native_member": native_member,
            "result": "passed",
            "wheel_sha256": hashlib.sha256(
                (fixture.root / qualification.WHEEL_PATH).read_bytes()
            ).hexdigest(),
        }
        envelope["claims"] = {
            "role_result": {
                "probe_results": {
                    "abi22": abi_result,
                    "python_wheel": wheel_result,
                }
            }
        }
        return envelope, receipt, None

    return factory


def test_qualification_positive_issuance_and_historical_verification(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    current_lock = hashlib.sha256(
        (qualification_fixture.ROOT / "Cargo.lock").read_bytes()
    ).hexdigest()
    monkeypatch.setattr(qualification, "FIXED_CARGO_LOCK_SHA256", current_lock)
    fixture = qualification_fixture._fixture(tmp_path)
    fake = FakeAuthority()
    fake.factories["qualification"] = _qualification_result_factory(fixture)
    fake.install(monkeypatch)
    qualification_fixture._assemble(fixture)
    fake.events.clear()
    snapshot = qualification_fixture._verify_qualified(fixture, monkeypatch)
    assert snapshot.qualification_receipt_id
    assert [event[0] for event in fake.events if event[0] != "preflight"] == [
        "verify"
    ]
    for relative in (
        qualification.NATIVE_AUTHORITY_ENVELOPE_PATH,
        qualification.NATIVE_AUTHORITY_RECEIPT_PATH,
    ):
        path = fixture.output / relative
        original_payload = path.read_bytes()
        original_mode = path.stat().st_mode & 0o777
        original = client.decode_canonical_json(
            original_payload, f"qualification test {relative}"
        )
        for _label, mutated in _scalar_mutations(original):
            path.chmod(0o600)
            path.write_bytes(_canonical(mutated))
            with pytest.raises(qualification.QualificationHandoffError):
                qualification_fixture._verify_qualified(
                    fixture, monkeypatch
                )
            path.write_bytes(original_payload)
            path.chmod(original_mode)
    assert not any(event[0] == "authorize" for event in fake.events)


def _deploy_plans(tmp_path: Path) -> tuple[SimpleNamespace, SimpleNamespace, SimpleNamespace]:
    archive = tmp_path / "candidate.tar.gz"
    archive.write_bytes(b"candidate")
    authority_dir = tmp_path / "candidate-authority"
    authority_dir.mkdir()
    authority_file = authority_dir / "release.json"
    authority_file.write_bytes(b"authority")
    qualified_root = tmp_path / "qualified"
    qualified_root.mkdir()
    qualified_manifest = qualified_root / qualification.QUALIFIED_HANDOFF_MANIFEST
    qualified_manifest.write_bytes(b"qualified")
    bundle_root = tmp_path / "bundle"
    bundle_root.mkdir()
    reset_manifest = bundle_root / "reset-manifest.json"
    reset_manifest.write_bytes(b"reset")
    peers: list[SimpleNamespace] = []
    for index, slug in enumerate(deploy.SLUGS):
        config = bundle_root / f"{slug}.toml"
        config.write_bytes(f"peer-{index}".encode("ascii"))
        peers.append(
            SimpleNamespace(
                slug=slug,
                config=config,
                config_sha256=hashlib.sha256(config.read_bytes()).hexdigest(),
            )
        )
    binary = tmp_path / "iroha3d"
    supervisor = tmp_path / "supervisor"
    binary.write_bytes(b"binary")
    supervisor.write_bytes(b"supervisor")
    admission = SimpleNamespace(
        archive=archive,
        archive_state=SimpleNamespace(size=archive.stat().st_size),
        archive_sha256=hashlib.sha256(archive.read_bytes()).hexdigest(),
        artifact_handoff_sha256="1" * 64,
        authority_dir=authority_dir,
        authority_state=(("release.json", SimpleNamespace()),),
        boi_artifact_inventory_sha256="2" * 64,
        boi_qualified_handoff=SimpleNamespace(root=qualified_root),
        boi_qualified_inventory_sha256="3" * 64,
        boi_qualification_receipt_id="4" * 64,
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        cargo_lock_sha256="5" * 64,
        dpn_validator_release_commit="6" * 40,
        receipt_id="7" * 64,
        release_manifest_sha256="8" * 64,
        reset_manifest_sha256="9" * 64,
        restart_generation="a" * 64,
        source_commit="b" * 40,
        supervisor_sha256=hashlib.sha256(supervisor.read_bytes()).hexdigest(),
        workspace_source_manifest_sha256="c" * 64,
    )
    bundle = SimpleNamespace(
        root=bundle_root,
        bundle_bytes=1,
        free_bytes=deploy.DEFAULT_MINIMUM_FREE_BYTES,
        fsync_latency_ms=1.0,
        manifest_sha256="d" * 64,
        peers=tuple(peers),
    )
    sources = SimpleNamespace(
        binary=binary,
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        supervisor=supervisor,
        supervisor_sha256=hashlib.sha256(supervisor.read_bytes()).hexdigest(),
    )
    return admission, bundle, sources


def test_deploy_public_dry_run_uses_authenticated_nonconsuming_lease(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.install(monkeypatch)
    admission, bundle, sources = _deploy_plans(tmp_path)
    monkeypatch.setattr(deploy, "require_sealed_external_tool_identity", lambda: None)
    monkeypatch.setattr(deploy, "validate_arguments", lambda _args: None)
    monkeypatch.setattr(
        deploy, "verify_deployment_admission", lambda _args: admission
    )
    monkeypatch.setattr(deploy, "validate_bundle", lambda *_args, **_kwargs: bundle)
    monkeypatch.setattr(deploy, "validate_sources", lambda *_args, **_kwargs: sources)
    monkeypatch.setattr(deploy, "require_inputs_match_admission", lambda *_args: None)
    monkeypatch.setattr(
        deploy, "require_admission_archive_unchanged", lambda _admission: None
    )
    monkeypatch.setattr(
        deploy,
        "capture_old_cohort",
        lambda *_args, **_kwargs: tuple(
            SimpleNamespace(
                path=tmp_path / f"{slug}.plist",
                managed=SimpleNamespace(child_was_present=True),
            )
            for slug in deploy.SLUGS
        ),
    )
    args = argparse.Namespace(
        allow_absent_old_child=False,
        apply=False,
        bundle=bundle.root,
        expected_dpn_validator_release_commit=(
            admission.dpn_validator_release_commit
        ),
        expected_production_reset_manifest_sha256=admission.reset_manifest_sha256,
        expected_source_commit=admission.source_commit,
        maximum_fsync_latency_ms=deploy.DEFAULT_MAXIMUM_FSYNC_LATENCY_MS,
        minimum_free_bytes=deploy.DEFAULT_MINIMUM_FREE_BYTES,
    )
    report = deploy.execute(args, ops=SimpleNamespace())
    assert report["mode"] == "verified-read-only-dry-run"
    assert report["admission_receipt_consumed"] is False
    assert report["deploy_authority_status"] == "verified"
    assert fake.apply_consumptions == 0
    assert fake.finalizations == 0
    assert [event[:2] for event in fake.events] == [
        ("preflight", "deploy-issuance"),
        ("authorize", "deploy-issuance"),
    ]


def test_deploy_fake_lease_dry_run_apply_and_finalize_semantics(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.install(monkeypatch)
    admission, bundle, sources = _deploy_plans(tmp_path)
    dry = deploy._authorize_deploy_lease(admission, bundle, sources, apply=False)
    assert dry.status == "verified"
    assert fake.apply_consumptions == 0
    assert fake.finalizations == 0
    applied = deploy._authorize_deploy_lease(admission, bundle, sources, apply=True)
    assert applied.operation_id == dry.operation_id
    assert fake.apply_consumptions == 1
    replayed_apply = deploy._authorize_deploy_lease(
        admission, bundle, sources, apply=True
    )
    assert replayed_apply.status == "replayed"
    assert replayed_apply.authority_envelope_bytes == applied.authority_envelope_bytes
    assert replayed_apply.durable_receipt_bytes == applied.durable_receipt_bytes
    assert fake.apply_consumptions == 1
    finalized = deploy._finalize_deploy_lease(
        admission,
        bundle,
        sources,
        applied,
        outcome="success",
        result={"applied": True},
    )
    assert finalized.status == "finalized"
    assert fake.finalizations == 1
    replayed_finalization = deploy._finalize_deploy_lease(
        admission,
        bundle,
        sources,
        applied,
        outcome="success",
        result={"applied": True},
    )
    assert replayed_finalization.status == "replayed"
    assert (
        replayed_finalization.durable_receipt_bytes
        == finalized.durable_receipt_bytes
    )
    assert fake.finalizations == 1
    with pytest.raises(deploy.DeploymentError, match="conflicting"):
        deploy._finalize_deploy_lease(
            admission,
            bundle,
            sources,
            applied,
            outcome="rolled-back",
            result={"applied": False},
        )
    assert [event[:2] for event in fake.events] == [
        ("authorize", "deploy-issuance"),
        ("authorize", "deploy-issuance"),
        ("authorize", "deploy-issuance"),
        ("finalize", "deploy-issuance"),
        ("finalize", "deploy-issuance"),
        ("finalize", "deploy-issuance"),
    ]


def test_rollout_positive_historical_and_every_sidecar_field_mutation(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.install(monkeypatch)
    plan = rollout.expected_plan()
    result = rollout_fixture._valid_result()
    issued = rollout.validate_result(result, plan=plan)
    fake.events.clear()
    assert rollout.verify_authenticated_result(
        result,
        plan=plan,
        authority_envelope=issued.authority_envelope,
        durable_receipt=issued.durable_receipt,
    ) == (issued.plan_sha256, issued.rollout_id)
    envelope = client.decode_canonical_json(
        issued.authority_envelope, "rollout test envelope"
    )
    receipt = client.decode_canonical_json(
        issued.durable_receipt, "rollout test receipt"
    )
    for target, original in (("envelope", envelope), ("receipt", receipt)):
        for _label, mutated in _scalar_mutations(original):
            with pytest.raises(rollout.RolloutContractError):
                rollout.verify_authenticated_result(
                    result,
                    plan=plan,
                    authority_envelope=_canonical(
                        mutated if target == "envelope" else envelope
                    ),
                    durable_receipt=_canonical(
                        mutated if target == "receipt" else receipt
                    ),
                )
    assert not any(event[0] == "authorize" for event in fake.events)


def test_public_soak_distinct_roles_fresh_consume_and_historical_only(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.install(monkeypatch)
    core = soak_fixture.subject_core()
    envelope_payload = soak_fixture.envelope(core)
    envelope_value = public_soak._decode_canonical(envelope_payload)
    observation_subject = public_soak._observation_subject(
        core, soak_fixture.COMPLETED_MS
    )
    fake.prime(
        "public-soak-observation",
        observation_subject,
        authority_envelope=envelope_value,
        durable_receipt={},
    )
    durable_payload = soak_fixture.durable_receipt(core, envelope_payload)
    durable_value = public_soak._decode_canonical(
        durable_payload, "test durable receipt"
    )

    def replay_factory(
        _role: str,
        _subject: dict[str, object],
        _manifest: tuple[dict[str, object], ...],
        envelope: dict[str, object],
        _receipt: dict[str, object],
    ) -> tuple[dict[str, object], dict[str, object], dict[str, object]]:
        return envelope, durable_value, envelope_value

    fake.factories["public-soak-replay-admission"] = replay_factory
    admitted = public_soak.consume_fresh_public_soak_admission(
        envelope_payload,
        subject_core=core,
        completed_at_unix_ms=soak_fixture.COMPLETED_MS,
    )
    assert admitted == durable_payload
    assert [event[:2] for event in fake.events if event[0] != "preflight"] == [
        ("verify", "public-soak-observation"),
        ("authorize", "public-soak-replay-admission"),
    ]
    fake.events.clear()
    claims = public_soak.verify_authenticated_public_soak_authority_envelope(
        envelope_payload,
        durable_admission_receipt=admitted,
        subject_core=core,
        completed_at_unix_ms=soak_fixture.COMPLETED_MS,
    )
    assert claims.replay_id == soak_fixture.digest("replay")
    assert [event[:2] for event in fake.events if event[0] != "preflight"] == [
        ("verify", "public-soak-observation"),
        ("verify", "public-soak-replay-admission"),
    ]
    for target, original in (
        ("envelope", envelope_value),
        ("receipt", durable_value),
    ):
        for _label, mutated in _scalar_mutations(original):
            with pytest.raises(public_soak.PublicSoakAuthorityError):
                public_soak.verify_authenticated_public_soak_authority_envelope(
                    _canonical(
                        mutated if target == "envelope" else envelope_value
                    ),
                    durable_admission_receipt=_canonical(
                        mutated if target == "receipt" else durable_value
                    ),
                    subject_core=core,
                    completed_at_unix_ms=soak_fixture.COMPLETED_MS,
                )
    assert not any(event[0] == "authorize" for event in fake.events)


def test_all_seven_public_boundaries_fail_before_untrusted_input(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    fake = FakeAuthority()
    fake.available = False
    fake.install(monkeypatch)
    calls: tuple[tuple[str, Callable[[], object], type[BaseException]], ...] = (
        (
            "native-evidence",
            lambda: native.build_authority(argparse.Namespace()),
            native.TairaReleaseAuthorityError,
        ),
        (
            "privacy-protocol-origin",
            lambda: protocol.validate_evidence_directory(
                Path("/attacker"),
                expected_source={},
                expected_validator_binary_sha256="bad",
                expected_linux_release_archive_sha256="bad",
                expected_exact12_matrix_sha256="bad",
                expected_artifact_handoff_sha256="bad",
                expected_receipt_id="bad",
                now_unix=1,
            ),
            protocol.PrivacyProtocolEvidenceError,
        ),
        (
            "privacy-governance",
            lambda: governance.request_authenticated_governance_transaction_v1(
                object()  # type: ignore[arg-type]
            ),
            governance.PrivacyGovernanceAuthorityError,
        ),
        (
            "qualification",
            lambda: qualification.assemble_qualification_handoff(
                Path("/attacker"),
                Path("/attacker-output"),
                object(),  # type: ignore[arg-type]
                qualification_external_signer=Path("/attacker-signer"),
                qualification_signing_public_key=Path("/attacker-key"),
                trusted_qualification_signing_fingerprint="bad",
                qualification_host_id="bad",
                qualification_installation_id="bad",
                controller_closure_digest="bad",
                workflow_run_id=1,
                workflow_run_attempt=1,
                release_manifest_verifier_path=Path("/attacker-verifier"),
                trusted_release_manifest_verifier_sha256="bad",
            ),
            qualification.QualificationHandoffError,
        ),
        (
            "deploy-issuance",
            lambda: deploy.execute(argparse.Namespace()),
            deploy.DeploymentError,
        ),
        (
            "rollout-observation",
            lambda: rollout.validate_result({}, plan={}),
            rollout.RolloutContractError,
        ),
        (
            "public-soak-observation",
            lambda: public_soak.consume_fresh_public_soak_admission(
                b"attacker",
                subject_core={},
                completed_at_unix_ms=1,
            ),
            public_soak.PublicSoakAuthorityError,
        ),
    )
    for expected_role, operation, error_type in calls:
        fake.events.clear()
        with pytest.raises(error_type):
            operation()
        assert fake.events[0] == ("preflight", expected_role, None)
        assert len(fake.events) == 1
