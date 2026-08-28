#!/usr/bin/env python3
"""Enforce Rust-worker ownership of every generic Python privacy witness."""

from __future__ import annotations

import argparse
import ast
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Final

GENERIC11_PROTOCOLS: Final[tuple[str, ...]] = (
    "zk-ace-pq-authorization-v1",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v1",
    "iroha-jindo-polynomial-commitment-v1",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v1",
)
ZK_X509_PROTOCOL: Final = "iroha-zk-x509-stark-p256-v1"
DIRECT_BUNDLE_METHODS: Final[tuple[str, ...]] = (
    "sign_privacy_anonymous_pgc_payment_action_v1",
    "sign_privacy_orchard_note_action_v1",
    "sign_privacy_fcmp_membership_payment_action_v1",
    "sign_privacy_ivm_private_note_action_v1",
    "sign_privacy_pq_masp_note_action_v1",
)
ZK_X509_METHODS: Final[tuple[str, ...]] = (
    "prepare_privacy_zk_x509_identity_presentation_action_v1",
    "sign_privacy_zk_x509_identity_presentation_action_v1",
)
RAW_WITNESS_FAMILIES: Final[tuple[tuple[str, tuple[str, ...]], ...]] = (
    ("ZK-ACE", ("sign_privacy_zk_ace_transfer_action_v1",)),
    ("Jindo", ("sign_privacy_jindo_action_v1",)),
    ("VeRange", ("sign_privacy_verange_action_v1",)),
    (
        "ZK-AMS",
        (
            "sign_privacy_zk_ams_batch_admission_action_v1",
            "sign_privacy_zk_ams_provision_account_action_v1",
        ),
    ),
    (
        "Vega",
        (
            "prepare_privacy_vega_action_v1",
            "finalize_privacy_vega_action_v1",
        ),
    ),
    ("Bootle/Lantern", ("sign_privacy_bootle_lantern_presentation_action_v1",)),
)
RAW_WITNESS_PARAMETERS: Final[frozenset[str]] = frozenset(
    {
        "attributes",
        "birth_date_issuer_signed_item",
        "blindings",
        "credential_nonces",
        "device_signature",
        "identity_blinding",
        "identity_root",
        "issuer_authentication_sig_structure",
        "issuer_signature",
        "issuer_signatures",
        "mobile_security_object_payload",
        "replay_secret",
        "secret_polynomials",
        "seed_secret",
        "seed_secrets",
        "witness_polynomials",
    }
)
RETIRED_RAW_RESULT_TYPES: Final[tuple[str, ...]] = (
    "PrivacyBootleLanternPresentationActionBuildResultV1",
    "PrivacyJindoActionBuildResultV1",
    "PrivacyVeRangeActionBuildResultV1",
    "PrivacyVegaActionPreparationV1",
    "PrivacyVegaActionBuildResultV1",
    "PrivacyZkAceTransferActionBuildResultV1",
    "PrivacyZkAmsBatchAdmissionActionBuildResultV1",
    "PrivacyZkAmsProvisionAccountActionBuildResultV1",
)
WORKER_TOP_LEVEL_EXPORTS: Final[tuple[str, ...]] = (
    "PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1",
    "PRIVACY_WALLET_WORKER_MAX_EXECUTION_PLAN_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_FRAME_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_PUBLIC_INTENT_BYTES_V1",
    "PRIVACY_WALLET_WORKER_MAX_TTL_MILLIS_V1",
    "PRIVACY_WALLET_WORKER_MIN_TTL_MILLIS_V1",
    "PRIVACY_WALLET_WORKER_PROTOCOL_VERSION_V1",
    "PrivacyWalletSignedActionV1",
    "PrivacyWalletWitnessBindingV1",
    "PrivacyWalletWitnessHandleV1",
    "PrivacyWalletWitnessLeaseV1",
    "PrivacyWalletWorkerCommandV1",
    "PrivacyWalletWorkerControllerV1",
    "PrivacyWalletWorkerErrorCodeV1",
    "PrivacyWalletWorkerErrorV1",
    "PrivacyWalletWorkerRemoteErrorV1",
    "privacy_wallet_public_intent_digest_v1",
)


@dataclass(frozen=True)
class BoundaryReport:
    structural_errors: tuple[str, ...]
    raw_witness_families: tuple[str, ...]

    @property
    def release_ready(self) -> bool:
        return not self.structural_errors and not self.raw_witness_families


def _read(path: Path, errors: list[str]) -> str:
    try:
        if path.is_symlink() or not path.is_file():
            raise OSError("not a regular non-symlink file")
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        errors.append(f"cannot read {path}: {error}")
        return ""


def _rust_function_names(source: str) -> set[str]:
    return set(
        re.findall(
            r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-zA-Z0-9_]+)(?:\s*<[^\n{;]*>)?\s*\(",
            source,
        )
    )


def _python_class(tree: ast.Module, name: str) -> ast.ClassDef | None:
    for node in tree.body:
        if isinstance(node, ast.ClassDef) and node.name == name:
            return node
    return None


def _python_methods(class_node: ast.ClassDef | None) -> dict[str, ast.FunctionDef]:
    if class_node is None:
        return {}
    return {
        node.name: node
        for node in class_node.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }


def _parameters(function: ast.FunctionDef) -> tuple[str, ...]:
    arguments = function.args
    return tuple(
        argument.arg
        for argument in (
            *arguments.posonlyargs,
            *arguments.args,
            *arguments.kwonlyargs,
        )
    )


def _literal_string_sequence(
    tree: ast.Module, name: str, errors: list[str]
) -> tuple[str, ...]:
    for node in tree.body:
        if not isinstance(node, (ast.Assign, ast.AnnAssign)):
            continue
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        if not any(
            isinstance(target, ast.Name) and target.id == name for target in targets
        ):
            continue
        try:
            value = ast.literal_eval(node.value)
        except (TypeError, ValueError) as error:
            errors.append(f"{name} is not literal: {error}")
            return ()
        if not isinstance(value, (list, tuple)) or not all(
            isinstance(item, str) for item in value
        ):
            errors.append(f"{name} is not a string sequence")
            return ()
        return tuple(value)
    errors.append(f"{name} is missing")
    return ()


def _relative_import_names(tree: ast.Module, module: str) -> set[str]:
    return {
        alias.name
        for node in tree.body
        if isinstance(node, ast.ImportFrom)
        and node.level == 1
        and node.module == module
        for alias in node.names
    }


def _rust_pymethods_impl(source: str, type_name: str) -> str:
    marker = f"#[pymethods]\nimpl {type_name}"
    start = source.find(marker)
    if start < 0:
        return ""
    opening = source.find("{", start + len(marker))
    if opening < 0:
        return ""
    depth = 0
    for index in range(opening, len(source)):
        character = source[index]
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
    return ""


def _literal_registry(tree: ast.Module, errors: list[str]) -> tuple[str, ...]:
    for node in tree.body:
        if not isinstance(node, (ast.Assign, ast.AnnAssign)):
            continue
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        if not any(
            isinstance(target, ast.Name)
            and target.id == "PRIVACY_GENERIC11_WORKER_OPERATION_SCHEMAS_V1"
            for target in targets
        ):
            continue
        try:
            value = ast.literal_eval(node.value)
        except (TypeError, ValueError) as error:
            errors.append(f"generic-11 worker registry is not literal: {error}")
            return ()
        if not isinstance(value, dict) or not all(
            isinstance(key, str) and isinstance(item, str)
            for key, item in value.items()
        ):
            errors.append("generic-11 worker registry is not a string mapping")
            return ()
        return tuple(value)
    errors.append("generic-11 worker registry is missing")
    return ()


def inspect_repository(root: Path) -> BoundaryReport:
    errors: list[str] = []
    rust_binding = _read(
        root / "python/iroha_python/iroha_python_rs/src/lib.rs", errors
    )
    python_tx = _read(root / "python/iroha_python/src/iroha_python/tx.py", errors)
    rust_worker = _read(
        root / "python/iroha_python/iroha_python_rs/src/privacy_wallet_worker.rs",
        errors,
    )
    python_worker = _read(
        root / "python/iroha_python/src/iroha_python/privacy_wallet_worker.py",
        errors,
    )
    python_crypto = _read(root / "python/iroha_python/src/iroha_python/crypto.py", errors)
    python_init = _read(root / "python/iroha_python/src/iroha_python/__init__.py", errors)
    if errors:
        return BoundaryReport(tuple(errors), ())

    try:
        tx_tree = ast.parse(python_tx, filename="tx.py")
        worker_tree = ast.parse(python_worker, filename="privacy_wallet_worker.py")
        ast.parse(python_crypto, filename="crypto.py")
        init_tree = ast.parse(python_init, filename="__init__.py")
    except SyntaxError as error:
        return BoundaryReport((f"Python custody source does not parse: {error}",), ())

    rust_methods = _rust_function_names(rust_binding)
    tx_methods = _python_methods(_python_class(tx_tree, "TransactionDraft"))
    controller_methods = _python_methods(
        _python_class(worker_tree, "PrivacyWalletWorkerControllerV1")
    )

    for token in ("execution_bundle", "IPWB"):
        for label, source in (("PyO3", rust_binding), ("TransactionDraft", python_tx)):
            if token in source:
                errors.append(f"{label} still exposes the forbidden {token!r} token")
    for method in DIRECT_BUNDLE_METHODS:
        if method in rust_methods or method in tx_methods:
            errors.append(f"legacy direct owner-bundle method remains public: {method}")

    builder_pymethods = _rust_pymethods_impl(rust_binding, "TransactionBuilder")
    if not builder_pymethods:
        errors.append("PyO3 TransactionBuilder methods are missing or malformed")
    else:
        leaked_parameters = sorted(
            parameter
            for parameter in RAW_WITNESS_PARAMETERS
            if re.search(rf"\b{re.escape(parameter)}\b", builder_pymethods)
        )
        if leaked_parameters:
            errors.append(
                "PyO3 TransactionBuilder still names raw witness parameters: "
                + ", ".join(leaked_parameters)
            )
    for method in tx_methods.values():
        leaked = RAW_WITNESS_PARAMETERS.intersection(_parameters(method))
        if leaked:
            errors.append(
                f"TransactionDraft {method.name} accepts raw witness parameters: "
                + ", ".join(sorted(leaked))
            )

    for type_name in RETIRED_RAW_RESULT_TYPES:
        for label, source in (
            ("PyO3", rust_binding),
            ("TransactionDraft", python_tx),
            ("crypto module", python_crypto),
            ("top-level package", python_init),
        ):
            if type_name in source:
                errors.append(f"{label} still exposes retired raw result type: {type_name}")

    base_exports = _literal_string_sequence(init_tree, "_BASE_EXPORTS", errors)
    worker_imports = _relative_import_names(init_tree, "privacy_wallet_worker")
    missing_imports = sorted(set(WORKER_TOP_LEVEL_EXPORTS).difference(worker_imports))
    if missing_imports:
        errors.append(
            "top-level package does not import the complete worker contract: "
            + ", ".join(missing_imports)
        )
    missing_exports = sorted(set(WORKER_TOP_LEVEL_EXPORTS).difference(base_exports))
    if missing_exports:
        errors.append(
            "top-level package does not export the complete worker contract: "
            + ", ".join(missing_exports)
        )

    execute = controller_methods.get("execute")
    expected_execute = (
        "self",
        "handle",
        "binding",
        "canonical_public_intent",
        "canonical_execution_plan",
    )
    if execute is None or _parameters(execute) != expected_execute:
        errors.append(
            "worker execute API is not the exact opaque-handle/public-wire contract"
        )
    forbidden_controller_parameters = {
        "credential_bytes",
        "execution_bundle",
        "ipwb",
        "owner_bundle",
        "witness",
        "witness_bytes",
    }
    for method in controller_methods.values():
        leaked = forbidden_controller_parameters.intersection(_parameters(method))
        if leaked:
            errors.append(
                f"worker controller {method.name} accepts forbidden custody bytes: "
                + ", ".join(sorted(leaked))
            )

    registry = _literal_registry(worker_tree, errors)
    if registry != GENERIC11_PROTOCOLS:
        errors.append("Python worker registry is not the exact ordered generic 11")
    if ZK_X509_PROTOCOL in registry:
        errors.append("ZK-X509 is incorrectly routed through the generic worker")
    worker_exports = _literal_string_sequence(worker_tree, "__all__", errors)
    if worker_exports != WORKER_TOP_LEVEL_EXPORTS:
        errors.append("Python worker __all__ is not the exact public controller contract")

    for method in ZK_X509_METHODS:
        if method not in rust_methods or method not in tx_methods:
            errors.append(f"separate ZK-X509 transport method is missing: {method}")
    retained_start = rust_worker.find("fn retained_protocol(")
    retained_end = rust_worker.find("\nfn ", retained_start + 1)
    retained_source = (
        rust_worker[retained_start:retained_end]
        if retained_start >= 0 and retained_end > retained_start
        else ""
    )
    if (
        "PrivacyProtocolIdV1::IrohaZkX509StarkP256V1" not in retained_source
        or "return Err(WorkerError::UnsupportedProtocol)" not in retained_source
    ):
        errors.append("Rust generic worker does not explicitly reject ZK-X509")

    exposed = rust_methods.union(tx_methods)
    blockers = tuple(
        family
        for family, methods in RAW_WITNESS_FAMILIES
        if any(method in exposed for method in methods)
    )
    return BoundaryReport(tuple(errors), blockers)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="Iroha repository root",
    )
    args = parser.parse_args(argv)
    report = inspect_repository(args.root.resolve())
    for error in report.structural_errors:
        print(f"error: {error}", file=sys.stderr)
    for family in report.raw_witness_families:
        print(
            "error: raw witness constructor family still crosses the PyO3 boundary: "
            f"{family}",
            file=sys.stderr,
        )
    if not report.release_ready:
        return 1
    print("privacy Python witness boundary is release-ready")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
