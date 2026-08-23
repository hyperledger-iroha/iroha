"""Focused cases for the standalone protected release-approval contract."""

from __future__ import annotations

from contextlib import contextmanager
import copy
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile

import pytest


_APPROVAL_REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
_APPROVAL_COMPONENT = (
    _APPROVAL_REPOSITORY_ROOT
    / "scripts"
    / "sumeragi_v2_release_approval_contract.py"
)
_APPROVAL_CANDIDATE_OID = "1" * 40
_APPROVAL_CANDIDATE_TREE = "2" * 40
_APPROVAL_TOOL_MANIFEST_SHA256 = "3" * 64
_APPROVAL_EVIDENCE_ROOT_ID = "release-evidence-root-2026-08-immutable"
_APPROVAL_DURATIONS = (900, 901, 902, 903)


def _load_approval_contract() -> object:
    spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_approval_contract_cases_module",
        _APPROVAL_COMPONENT,
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@contextmanager
def _approval_protected_root() -> object:
    root = Path(
        tempfile.mkdtemp(
            prefix=".iroha-release-approval-contract-test-",
            dir=Path.home(),
        )
    ).resolve(strict=True)
    root.chmod(0o700)
    try:
        yield root
    finally:
        root.chmod(0o700)
        shutil.rmtree(root)


def _approval_canonical(value: object) -> bytes:
    return (
        json.dumps(
            value,
            allow_nan=False,
            ensure_ascii=True,
            separators=(",", ":"),
            sort_keys=True,
        )
        + "\n"
    ).encode("ascii")


def _approval_document(
    module: object,
    approval_class: object,
    ordinal: int,
) -> dict[str, object]:
    expectation = _approval_expectations(module)[approval_class]
    class_id = approval_class.value
    return {
        "approval_id": f"approval-{ordinal}-{class_id}",
        "approved_at": f"2026-08-{ordinal + 1:02d}T01:02:03Z",
        "candidate_oid": _APPROVAL_CANDIDATE_OID,
        "candidate_tree": _APPROVAL_CANDIDATE_TREE,
        "class_id": class_id,
        "evidence_root_id": expectation.evidence_root_id,
        "expected_duration_seconds": expectation.expected_duration_seconds,
        "format": module.APPROVAL_FORMAT,
        "operations": [operation.value() for operation in expectation.operations],
        "profile": expectation.profile,
        "protected_tool_manifest_sha256": (
            expectation.protected_tool_manifest_sha256
        ),
        "schema_version": module.APPROVAL_SCHEMA_VERSION,
    }


def _approval_expectations(module: object) -> dict[object, object]:
    return module.build_release_approval_expectations(
        candidate_oid=_APPROVAL_CANDIDATE_OID,
        candidate_tree=_APPROVAL_CANDIDATE_TREE,
        protected_tool_manifest_sha256=_APPROVAL_TOOL_MANIFEST_SHA256,
        evidence_root_id=_APPROVAL_EVIDENCE_ROOT_ID,
        offline_toolchain_sdk_duration_seconds=_APPROVAL_DURATIONS[0],
        formal_proof_tools_duration_seconds=_APPROVAL_DURATIONS[1],
        network_scale_soak_duration_seconds=_APPROVAL_DURATIONS[2],
        final_bootstrap_publication_duration_seconds=_APPROVAL_DURATIONS[3],
    )


def _approval_write(path: Path, data: bytes) -> Path:
    if path.exists() and not path.is_symlink():
        path.chmod(0o600)
    path.write_bytes(data)
    path.chmod(0o400)
    return path


def _approval_operations(module: object, document: dict[str, object]) -> tuple[object, ...]:
    records = document["operations"]
    assert isinstance(records, list)
    return tuple(
        module.ApprovalOperation(
            ordinal=record["ordinal"],
            operation_id=record["operation_id"],
            tool_id=record["tool_id"],
            arguments=tuple(record["arguments"]),
        )
        for record in records
    )


def _approval_expectation(
    module: object,
    approval_class: object,
    document: dict[str, object],
) -> object:
    return module.ReleaseApprovalExpectation(
        class_id=approval_class,
        candidate_oid=document["candidate_oid"],
        candidate_tree=document["candidate_tree"],
        profile=document["profile"],
        operations=_approval_operations(module, document),
        protected_tool_manifest_sha256=document[
            "protected_tool_manifest_sha256"
        ],
        evidence_root_id=document["evidence_root_id"],
        expected_duration_seconds=document["expected_duration_seconds"],
    )


def _case_release_approval_four_class_binding_and_path_free_archives() -> None:
    module = _load_approval_contract()
    expectations = _approval_expectations(module)
    assert tuple(expectations) == tuple(module.APPROVAL_CLASS_ORDER)
    expected_operation_ids = {
        "offline-toolchain-sdk": (
            "offline-rustc-version",
            "offline-cargo-version",
            "offline-workspace-build",
            "g-unit-production-864",
            "g-unit-focused-522",
            "offline-workspace-clippy",
            "offline-workspace-format",
            "offline-no-legacy-codec",
            "sdk-rust-regeneration-first",
            "sdk-rust-regeneration-second",
            "sdk-regeneration-byte-identity",
            "sdk-grouped-openapi",
            "sdk-grouped-python",
            "sdk-grouped-javascript",
            "sdk-grouped-swift",
            "sdk-grouped-kotlin",
            "sdk-grouped-java",
            "sdk-diagnostics-rust",
            "sdk-diagnostics-python",
            "sdk-diagnostics-javascript",
            "sdk-diagnostics-swift",
            "sdk-diagnostics-kotlin",
            "sdk-diagnostics-java",
        ),
        "formal-proof-tools": (
            "formal-proof-ledger",
            "formal-tlaps",
            "formal-mutation-service-rank",
            "formal-mutation-productive",
            "formal-mutation-candidate-restart",
            "formal-mutation-commit-import-provenance",
            "formal-mutation-restart-locked-fetch-order",
            "formal-mutation-persist-install-generation",
            "formal-mutation-persist-install-validation",
            "formal-mutation-apply-authority",
            "formal-mutation-replay-locked-body-carrier",
            "formal-mutation-certificate-ref-recovery",
            "formal-mutation-certified-response-source-lineage",
            "formal-mutation-certified-response-identity-separation",
            "formal-mutation-progress",
            "formal-mutation-begin-timeout-ready",
            "formal-mutation-command-execution-ready",
            "formal-mutation-post-decision-timeout",
            "formal-mutation-decision-recovery-lifecycle",
            "formal-mutation-certified-response-registration",
            "formal-mutation-effect-capacity-ownership",
            "formal-mutation-applied-phase-admission",
            "formal-mutation-ingress-causal-freshness",
            "formal-mutation-liveness-ownership",
            "formal-mutation-serve-scheduler-ordinal",
            "formal-mutation-indexed-service-activation",
            "formal-mutation-adequate-leader-readiness",
            "formal-mutation-indexed-height",
            "formal-mutation-item-carrier-typing",
            "formal-mutation-reply-writer-deadline",
            "formal-mutation-historical-discovery-occurrence-rank",
            "formal-mutation-typed-rollover-handoff",
            "formal-tlc-positive-and-mutations",
            "formal-apalache-refinement",
            "formal-production-trace-replay",
            "formal-rust-verus-correspondence",
            "formal-verus-evidence-validation",
            "formal-cross-tool-evidence",
        ),
        "network-scale-soak": (
            "network-release-seed-matrix",
            "network-g4p-mandatory-cases",
            "network-g12p-ten-seeds",
            "network-g12p-rotating-fault-soak",
            "scale-five-paired-trials",
            "scale-evidence-validation",
            "network-chaos-100000-height",
        ),
        "final-bootstrap-publication": (
            "final-protected-bootstrap",
            "final-release-runner",
            "final-canonical-receipt-publication",
            "final-no-clobber-validator-acknowledgment",
            "final-private-state-prune",
            "final-retained-inventory-and-result-publication",
            "final-bootstrap-independent-authentication",
            "final-external-completion-publication",
        ),
    }
    expected_plan_digests = {
        "offline-toolchain-sdk": (
            "ec23b831d3c9359fc94952bbda3cccb92e84a39645bb14e5c19f130d01ded95b"
        ),
        "formal-proof-tools": (
            "eb9f0283898f09d23970f1d6511d250b17107a0ad80fc65e1adbe1ef0b1b19bb"
        ),
        "network-scale-soak": (
            "a72659ea6af739910412dfe36687d8f512cc699b63c566f941089f2fcb028663"
        ),
        "final-bootstrap-publication": (
            "76be51f1583e2d49c8b9ac85f9218a0a0b5a3334f1923dad39aa13ec8e7768fd"
        ),
    }
    assert {
        approval_class.value: digest
        for approval_class, digest in module.APPROVAL_OPERATION_PLAN_SHA256.items()
    } == expected_plan_digests
    all_operation_ids: list[str] = []
    for approval_class in module.APPROVAL_CLASS_ORDER:
        expectation = expectations[approval_class]
        operation_ids = tuple(
            operation.operation_id for operation in expectation.operations
        )
        assert operation_ids == expected_operation_ids[approval_class.value]
        assert tuple(operation.ordinal for operation in expectation.operations) == tuple(
            range(len(expectation.operations))
        )
        assert expectation.expected_duration_seconds == _APPROVAL_DURATIONS[
            module.APPROVAL_CLASS_ORDER.index(approval_class)
        ]
        for operation in expectation.operations:
            assert not operation.tool_id.startswith(("/", "~", "file:"))
            assert all(
                not argument.startswith(("/", "~/", "file://"))
                and "{candidate_" not in argument
                and "{evidence_" not in argument
                and "{protected_" not in argument
                for argument in operation.arguments
            )
        all_operation_ids.extend(operation_ids)
    assert len(all_operation_ids) == len(set(all_operation_ids))
    assert {
        approval_class.value: len(expectations[approval_class].operations)
        for approval_class in module.APPROVAL_CLASS_ORDER
    } == {
        "offline-toolchain-sdk": 23,
        "formal-proof-tools": 38,
        "network-scale-soak": 7,
        "final-bootstrap-publication": 8,
    }
    final_arguments = tuple(
        argument
        for operation in expectations[
            module.ReleaseApprovalClass.FINAL_BOOTSTRAP_PUBLICATION
        ].operations
        for argument in operation.arguments
    )
    assert _APPROVAL_CANDIDATE_OID in final_arguments
    assert _APPROVAL_CANDIDATE_TREE in final_arguments
    assert _APPROVAL_TOOL_MANIFEST_SHA256 in final_arguments
    assert _APPROVAL_EVIDENCE_ROOT_ID in final_arguments
    assert "archive:release-candidate.signed-immutable.v1" in final_arguments
    assert "archive:release-receipt.canonical.v1" in final_arguments
    assert "archive:release-retained.inventory.v2" in final_arguments
    assert dict(module.APPROVAL_REQUIRED_CONSUMER_APIS) == {
        "scripts/bootstrap_sumeragi_v2_release.py": (
            "build_release_approval_expectations",
            "load_protected_release_approval_set",
            "sanitized_release_approval_set_archive",
        ),
        "scripts/validate_sumeragi_v2_release_bootstrap.py": (
            "build_release_approval_expectations",
            "load_protected_release_approval_set",
            "sanitized_release_approval_set_archive",
        ),
        "scripts/write_sumeragi_v2_release_receipt.py": (
            "require_release_approval_binding",
            "sanitized_release_approval_set_archive",
        ),
    }
    assert dict(module.APPROVAL_REQUIRED_CONSUMER_ACTIONS) == {
        "scripts/bootstrap_sumeragi_v2_release.py": (
            "import-component",
            "protected-load-and-exact-bind",
            "publish-sanitized-archive",
        ),
        "scripts/validate_sumeragi_v2_release_bootstrap.py": (
            "import-component",
            "independently-replay-sanitized-archive",
        ),
        "scripts/write_sumeragi_v2_release_receipt.py": (
            "import-component",
            "retain-sanitized-archive-and-digests",
        ),
    }
    for relative in module.APPROVAL_REQUIRED_CONSUMER_APIS:
        consumer = _APPROVAL_REPOSITORY_ROOT / relative
        assert consumer.is_file() and not consumer.is_symlink()
    runbook = (
        _APPROVAL_REPOSITORY_ROOT
        / "specs"
        / "runbooks"
        / "nexus_multilane_rehearsal.md"
    ).read_text(encoding="utf-8")
    liveness = (
        _APPROVAL_REPOSITORY_ROOT / "specs" / "sumeragi_v2_liveness.md"
    ).read_text(encoding="utf-8")
    schema_fields = (
        "approval_id",
        "approved_at",
        "candidate_oid",
        "candidate_tree",
        "class_id",
        "evidence_root_id",
        "expected_duration_seconds",
        "format",
        "operations",
        "profile",
        "protected_tool_manifest_sha256",
        "schema_version",
        "arguments",
        "operation_id",
        "ordinal",
        "tool_id",
    )
    for field in schema_fields:
        assert f"`{field}`" in runbook
        assert f"`{field}`" in liveness
    for approval_class in module.APPROVAL_CLASS_ORDER:
        assert f"`{approval_class.value}`" in runbook
        assert f"`{approval_class.value}`" in liveness
        plan_digest = expected_plan_digests[approval_class.value]
        assert f"`{plan_digest}`" in runbook
        assert f"`{plan_digest}`" in liveness
        for operation in expectations[approval_class].operations:
            assert f"`{operation.operation_id}`" in runbook
    assert "operator decisions, not digital signatures" in runbook
    assert "operator decisions; they are not signatures" in liveness
    assert "independently replay the four records" in liveness
    assert "does not by itself close a release gate" in liveness

    with pytest.raises(TypeError):
        module.APPROVAL_OPERATION_PLANS[
            module.ReleaseApprovalClass.OFFLINE_TOOLCHAIN_SDK
        ] = ()

    with _approval_protected_root() as root:
        paths: dict[object, Path] = {}
        documents: dict[object, dict[str, object]] = {}
        for ordinal, approval_class in enumerate(module.APPROVAL_CLASS_ORDER):
            document = _approval_document(module, approval_class, ordinal)
            path = _approval_write(
                root / module.APPROVAL_FILENAMES[approval_class],
                _approval_canonical(document),
            )
            expectation = _approval_expectation(
                module, approval_class, document
            )
            assert expectation == expectations[approval_class]
            approval = module.load_protected_release_approval(
                path,
                expected_class=approval_class,
                expectation=expectation,
            )
            archive = approval.sanitized_archive()
            assert approval.class_id is approval_class
            assert approval.operations == expectation.operations
            assert approval.source_mode == 0o400
            assert approval.source_nlink == 1
            assert approval.source_owner_uid == os.geteuid()
            assert archive.canonical_bytes == _approval_canonical(archive.value)
            assert archive.sha256 == __import__("hashlib").sha256(
                archive.canonical_bytes
            ).hexdigest()
            assert "arguments" not in archive.value["ordered_operations"][0]
            assert str(root) not in archive.canonical_bytes.decode("ascii")
            assert archive.value["protected_approval"] == {
                "mode": "0400",
                "nlink": 1,
                "owner_contract": "release-host-effective-uid",
                "sha256": approval.approval_sha256,
                "size_bytes": approval.size_bytes,
            }
            paths[approval_class] = path
            documents[approval_class] = document

        approvals = module.load_protected_release_approval_set(
            paths, expectations=expectations
        )
        assert tuple(value.class_id for value in approvals) == tuple(
            module.APPROVAL_CLASS_ORDER
        )
        set_archive = module.sanitized_release_approval_set_archive(approvals)
        assert set_archive.canonical_bytes == _approval_canonical(set_archive.value)
        assert [
            value["class_id"] for value in set_archive.value["approvals"]
        ] == list(module.APPROVAL_CLASS_IDS)
        assert str(root) not in set_archive.canonical_bytes.decode("ascii")

        network_class = module.ReleaseApprovalClass.NETWORK_SCALE_SOAK
        extra_network_operation = copy.deepcopy(documents[network_class])
        eighth_operation = copy.deepcopy(extra_network_operation["operations"][-1])
        eighth_operation["ordinal"] = 7
        eighth_operation["operation_id"] = "network-unapproved-eighth-operation"
        extra_network_operation["operations"].append(eighth_operation)
        extra_network_path = _approval_write(
            root / "network-eighth-operation.json",
            _approval_canonical(extra_network_operation),
        )
        with pytest.raises(
            module.ReleaseApprovalError, match="exact planned invocation"
        ):
            module.load_protected_release_approval(
                extra_network_path,
                expected_class=network_class,
                expectation=expectations[network_class],
            )

        first_class = module.APPROVAL_CLASS_ORDER[0]
        mismatched = copy.deepcopy(expectations[first_class])
        wrong_operations = list(mismatched.operations)
        wrong_operations[0] = module.ApprovalOperation(
            ordinal=0,
            operation_id=wrong_operations[0].operation_id,
            tool_id=wrong_operations[0].tool_id,
            arguments=(*wrong_operations[0].arguments, "--unexpected"),
        )
        mismatched = module.ReleaseApprovalExpectation(
            class_id=mismatched.class_id,
            candidate_oid=mismatched.candidate_oid,
            candidate_tree=mismatched.candidate_tree,
            profile=mismatched.profile,
            operations=tuple(wrong_operations),
            protected_tool_manifest_sha256=mismatched.protected_tool_manifest_sha256,
            evidence_root_id=mismatched.evidence_root_id,
            expected_duration_seconds=mismatched.expected_duration_seconds,
        )
        with pytest.raises(
            module.ReleaseApprovalError, match="exact planned invocation"
        ):
            module.require_release_approval_binding(approvals[0], mismatched)

        duration_mismatch = module.ReleaseApprovalExpectation(
            class_id=expectations[first_class].class_id,
            candidate_oid=expectations[first_class].candidate_oid,
            candidate_tree=expectations[first_class].candidate_tree,
            profile=expectations[first_class].profile,
            operations=expectations[first_class].operations,
            protected_tool_manifest_sha256=expectations[
                first_class
            ].protected_tool_manifest_sha256,
            evidence_root_id=expectations[first_class].evidence_root_id,
            expected_duration_seconds=expectations[
                first_class
            ].expected_duration_seconds
            + 1,
        )
        with pytest.raises(
            module.ReleaseApprovalError, match="exact planned invocation"
        ):
            module.require_release_approval_binding(
                approvals[0], duration_mismatch
            )

        with pytest.raises(
            module.ReleaseApprovalError, match="exactly four approval classes"
        ):
            module.load_protected_release_approval_set(
                {key: value for key, value in paths.items() if key is not first_class}
            )

        last_class = module.APPROVAL_CLASS_ORDER[-1]
        divergent = copy.deepcopy(documents[last_class])
        divergent["candidate_tree"] = "4" * 40
        _approval_write(paths[last_class], _approval_canonical(divergent))
        with pytest.raises(
            module.ReleaseApprovalError, match="more than one candidate"
        ):
            module.load_protected_release_approval_set(paths)


def _case_release_approval_schema_canonicality_and_path_disclosure_fail_closed() -> None:
    module = _load_approval_contract()
    approval_class = module.ReleaseApprovalClass.OFFLINE_TOOLCHAIN_SDK
    base = _approval_document(module, approval_class, 0)
    builder_arguments = {
        "candidate_oid": _APPROVAL_CANDIDATE_OID,
        "candidate_tree": _APPROVAL_CANDIDATE_TREE,
        "protected_tool_manifest_sha256": _APPROVAL_TOOL_MANIFEST_SHA256,
        "evidence_root_id": _APPROVAL_EVIDENCE_ROOT_ID,
        "offline_toolchain_sdk_duration_seconds": _APPROVAL_DURATIONS[0],
        "formal_proof_tools_duration_seconds": _APPROVAL_DURATIONS[1],
        "network_scale_soak_duration_seconds": _APPROVAL_DURATIONS[2],
        "final_bootstrap_publication_duration_seconds": _APPROVAL_DURATIONS[3],
    }
    builder_mutations = (
        ("candidate_oid", "A" * 40, "Git object ID"),
        ("candidate_tree", "2" * 64, "mixes object formats"),
        ("protected_tool_manifest_sha256", "3" * 63, "SHA-256"),
        ("evidence_root_id", "/private/evidence", "path-free identifier"),
        ("offline_toolchain_sdk_duration_seconds", 0, "outside its bound"),
        ("formal_proof_tools_duration_seconds", True, "outside its bound"),
        (
            "network_scale_soak_duration_seconds",
            module.MAX_EXPECTED_DURATION_SECONDS + 1,
            "outside its bound",
        ),
    )
    for field, value, message in builder_mutations:
        mutated_arguments = dict(builder_arguments)
        mutated_arguments[field] = value
        with pytest.raises(module.ReleaseApprovalError, match=message):
            module.build_release_approval_expectations(**mutated_arguments)

    mutations: list[tuple[str, dict[str, object]]] = []

    extra_signature = copy.deepcopy(base)
    extra_signature["signature"] = "not-a-real-signature"
    mutations.append(("wrong exact schema", extra_signature))

    wrong_schema = copy.deepcopy(base)
    wrong_schema["schema_version"] = True
    mutations.append(("schema version 1", wrong_schema))

    wrong_class = copy.deepcopy(base)
    wrong_class["class_id"] = "formal-proof-tools"
    mutations.append(("wrong approval class", wrong_class))

    wrong_oid = copy.deepcopy(base)
    wrong_oid["candidate_oid"] = "A" * 40
    mutations.append(("Git object ID", wrong_oid))

    mixed_objects = copy.deepcopy(base)
    mixed_objects["candidate_tree"] = "2" * 64
    mutations.append(("mixes Git object formats", mixed_objects))

    bad_duration = copy.deepcopy(base)
    bad_duration["expected_duration_seconds"] = 0
    mutations.append(("outside its bound", bad_duration))

    boolean_duration = copy.deepcopy(base)
    boolean_duration["expected_duration_seconds"] = True
    mutations.append(("outside its bound", boolean_duration))

    non_utc = copy.deepcopy(base)
    non_utc["approved_at"] = "2026-08-01T01:02:03+00:00"
    mutations.append(("UTC", non_utc))

    impossible_date = copy.deepcopy(base)
    impossible_date["approved_at"] = "2026-02-30T01:02:03Z"
    mutations.append(("valid UTC instant", impossible_date))

    bad_ordinal = copy.deepcopy(base)
    bad_ordinal["operations"][1]["ordinal"] = 7
    mutations.append(("contiguous and ordered", bad_ordinal))

    duplicate_operation = copy.deepcopy(base)
    duplicate_operation["operations"][1]["operation_id"] = duplicate_operation[
        "operations"
    ][0]["operation_id"]
    mutations.append(("repeats an operation_id", duplicate_operation))

    absolute_path = copy.deepcopy(base)
    absolute_path["operations"][0]["arguments"].append(
        "/private/tmp/original-checkout"
    )
    mutations.append(("original or escaping path", absolute_path))

    option_path = copy.deepcopy(base)
    option_path["operations"][0]["arguments"].append(
        "--cargo-home=/Users/operator/.cargo"
    )
    mutations.append(("original or escaping path", option_path))

    parent_escape = copy.deepcopy(base)
    parent_escape["operations"][0]["arguments"].append("../caller/tool")
    mutations.append(("original or escaping path", parent_escape))

    bad_evidence_id = copy.deepcopy(base)
    bad_evidence_id["evidence_root_id"] = "/private/evidence"
    mutations.append(("path-free identifier", bad_evidence_id))

    missing_operations = copy.deepcopy(base)
    missing_operations["operations"] = []
    mutations.append(("invalid operation count", missing_operations))

    with _approval_protected_root() as root:
        for index, (message, value) in enumerate(mutations):
            path = _approval_write(
                root / f"semantic-{index}.json", _approval_canonical(value)
            )
            with pytest.raises(module.ReleaseApprovalError, match=message):
                module.load_protected_release_approval(
                    path, expected_class=approval_class
                )

        canonical = _approval_canonical(base)
        raw_mutations = (
            canonical.replace(
                b'{"approval_id":',
                b'{"approval_id":"shadow","approval_id":',
                1,
            ),
            canonical.replace(
                b'"expected_duration_seconds":900',
                b'"expected_duration_seconds":NaN',
                1,
            ),
            json.dumps(base, indent=2, sort_keys=True).encode("utf-8") + b"\n",
        )
        for index, data in enumerate(raw_mutations):
            path = _approval_write(root / f"encoding-{index}.json", data)
            with pytest.raises(module.ReleaseApprovalError):
                module.load_protected_release_approval(
                    path, expected_class=approval_class
                )


def _case_release_approval_file_protection_and_bounds_fail_closed() -> None:
    module = _load_approval_contract()
    approval_class = module.ReleaseApprovalClass.FINAL_BOOTSTRAP_PUBLICATION
    document = _approval_document(module, approval_class, 3)
    canonical = _approval_canonical(document)
    with _approval_protected_root() as root:
        path = _approval_write(root / "approval.json", canonical)
        module.load_protected_release_approval(
            path, expected_class=approval_class
        )

        path.chmod(0o600)
        with pytest.raises(module.ReleaseApprovalError, match="0400 single-link"):
            module.load_protected_release_approval(
                path, expected_class=approval_class
            )
        path.chmod(0o400)

        hardlink = root / "approval-hardlink.json"
        os.link(path, hardlink)
        with pytest.raises(module.ReleaseApprovalError, match="0400 single-link"):
            module.load_protected_release_approval(
                path, expected_class=approval_class
            )
        hardlink.unlink()

        symlink = root / "approval-symlink.json"
        symlink.symlink_to(path.name)
        with pytest.raises(module.ReleaseApprovalError, match="symlinks"):
            module.load_protected_release_approval(
                symlink, expected_class=approval_class
            )
        symlink.unlink()

        root.chmod(0o720)
        with pytest.raises(module.ReleaseApprovalError, match="writable.*ancestor"):
            module.load_protected_release_approval(
                path, expected_class=approval_class
            )
        root.chmod(0o700)

        oversized = root / "oversized.json"
        _approval_write(oversized, b" " * (module.MAX_APPROVAL_BYTES + 1))
        with pytest.raises(module.ReleaseApprovalError, match="bounded regular file"):
            module.load_protected_release_approval(
                oversized, expected_class=approval_class
            )

        assert stat.S_IMODE(path.stat().st_mode) == 0o400


# The receipt aggregate lexically executes this component and calls the three
# case helpers from its existing approval/provenance selectors, preserving the
# exact 368-selector aggregate.  Direct collection retains the focused 3-case
# component surface for isolated validation.
if "RELEASE_RECEIPT_TEST_COMPONENT_FILES" not in globals():

    def test_release_approval_four_class_binding_and_path_free_archives() -> None:
        _case_release_approval_four_class_binding_and_path_free_archives()


    def test_release_approval_schema_canonicality_and_path_disclosure_fail_closed() -> None:
        _case_release_approval_schema_canonicality_and_path_disclosure_fail_closed()


    def test_release_approval_file_protection_and_bounds_fail_closed() -> None:
        _case_release_approval_file_protection_and_bounds_fail_closed()
