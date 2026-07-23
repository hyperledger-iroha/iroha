#!/usr/bin/env python3
"""Validate multilane TLA+ model/config inventory and Rust source bindings.

This is a structural gate. It verifies that every finite model, positive
configuration, named mutation, and production item still exists with the
reviewed semantic anchors. It does not treat TLC output as deductive proof.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any


DEFAULT_ROOT = Path(__file__).resolve().parents[2]
FORMAL_RELATIVE = Path("docs/formal/sumeragi_v2")
BINDINGS_FILENAME = "multilane_source_bindings.json"
APALACHE_RUNNER_RELATIVE = Path(
    "scripts/formal/run_sumeragi_v2_multilane_apalache.sh"
)
APALACHE_RUNNER_TEST_RELATIVE = Path(
    "scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py"
)
APALACHE_INSTALLER_RELATIVE = Path("scripts/formal/install_apalache.sh")
TLC_RUNNER_RELATIVE = Path("scripts/formal/run_sumeragi_v2_tlc.sh")
TLC_MUTATION_RUNNER_RELATIVE = Path(
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh"
)
FORMAL_WORKFLOW_RELATIVES = (
    Path(".github/workflows/pr.yml"),
    Path(".github/workflows/nightly_sumeragi_formal.yml"),
)
README_RELATIVE = FORMAL_RELATIVE / "README.md"
APALACHE_VERSION = "0.52.2"
APALACHE_ARCHIVE_SHA256 = (
    "e0ebea7e45c8f99df8d92f2755101dda84ab71df06d1ec3a21955d3b53a886e2"
)
APALACHE_LAUNCHER_SHA256 = (
    "bda52d2dbdbc7f6e95289a69dfe7ddeb162493ddd3501898d33ea7d1da3a8cd7"
)
APALACHE_JAR_SHA256 = (
    "1ac65e9c16595c19241519b209c8055d1aa79bf718f23df7cde5cf9b3dd88f2a"
)
MODULE_RE = re.compile(r"(?m)^---- MODULE ([A-Za-z_][A-Za-z0-9_]*) ----$")
TLA_DECLARATION_TEMPLATE = (
    r"(?m)^[ \t]*(?:THEOREM[ \t]+)?{symbol}"
    r"\s*(?:\([^)=\n]*\))?\s*=="
)
RUST_DECLARATION_TEMPLATES = {
    "fn": (
        r"(?m)^[ \t]*(?:pub(?:\([^)\n]*\))?[ \t]+)?"
        r"(?:const[ \t]+)?(?:async[ \t]+)?fn[ \t]+{symbol}\b"
    ),
    "struct": (
        r"(?m)^[ \t]*(?:pub(?:\([^)\n]*\))?[ \t]+)?"
        r"struct[ \t]+{symbol}\b"
    ),
    "enum": (
        r"(?m)^[ \t]*(?:pub(?:\([^)\n]*\))?[ \t]+)?"
        r"enum[ \t]+{symbol}\b"
    ),
}


def _regular_file(path: Path, label: str, errors: list[str]) -> bool:
    if not path.is_file() or path.is_symlink():
        errors.append(f"{label} must be a regular non-symlink file: {path}")
        return False
    return True


def _extract_braced_item(source: str, declaration: re.Match[str]) -> str | None:
    """Return one brace-balanced Rust item while skipping comments and literals."""

    start = declaration.start()
    index = source.find("{", declaration.end())
    if index < 0:
        return None
    depth = 0
    state = "code"
    block_comment_depth = 0
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        pair = source[index : index + 2]
        if state == "line-comment":
            if char == "\n":
                state = "code"
            index += 1
            continue
        if state == "block-comment":
            if pair == "/*":
                block_comment_depth += 1
                index += 2
                continue
            if pair == "*/":
                block_comment_depth -= 1
                index += 2
                if block_comment_depth == 0:
                    state = "code"
                continue
            index += 1
            continue
        if state == "string":
            if char == "\\":
                index += 2
                continue
            if char == '"':
                state = "code"
            index += 1
            continue
        if state == "char":
            if char == "\\":
                index += 2
                continue
            if char == "'":
                state = "code"
            index += 1
            continue
        if state == "raw-string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                index += len(terminator)
                state = "code"
            else:
                index += 1
            continue

        if pair == "//":
            state = "line-comment"
            index += 2
            continue
        if pair == "/*":
            state = "block-comment"
            block_comment_depth = 1
            index += 2
            continue
        if char == "r":
            raw_end = index + 1
            while raw_end < len(source) and source[raw_end] == "#":
                raw_end += 1
            if raw_end < len(source) and source[raw_end] == '"':
                raw_hashes = raw_end - index - 1
                state = "raw-string"
                index = raw_end + 1
                continue
        if char == '"':
            state = "string"
            index += 1
            continue
        if char == "'" and index + 1 < len(source):
            # Rust lifetimes are followed by an identifier; character literals
            # have a closing quote nearby.
            closing = source.find("'", index + 1, min(index + 8, len(source)))
            if closing >= 0:
                state = "char"
                index += 1
                continue
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
        index += 1
    return None


def _nonempty_string(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _apalache_runner_source_errors(source: str) -> list[str]:
    """Validate the exact pinned multilane Apalache runner contract."""

    errors: list[str] = []
    required_once = (
        f'readonly APALACHE_VERSION="{APALACHE_VERSION}"',
        f'readonly APALACHE_LAUNCHER_SHA256="{APALACHE_LAUNCHER_SHA256}"',
        f'readonly APALACHE_JAR_SHA256="{APALACHE_JAR_SHA256}"',
        'readonly CONTRACT_CHECKER="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_models.py"',
        'readonly RUNNER_CONTRACT_TEST="${REPO_ROOT}/scripts/formal/check_sumeragi_v2_multilane_apalache_runner_contract.py"',
        'readonly EVIDENCE_PATH="${EVIDENCE_DIR}/multilane_apalache_evidence.tsv"',
        '\npython3 -I -S "$CONTRACT_CHECKER"\n',
        'python3 -I -S "$RUNNER_CONTRACT_TEST"',
        'tool_version="$("$RESOLVED_APALACHE_BIN" version)"',
        '[[ "$tool_version" != "$APALACHE_VERSION" ]]',
        '"$RESOLVED_APALACHE_BIN" --out-dir="$out" typecheck "${module}.tla"',
        '"$RESOLVED_APALACHE_BIN" --out-dir="$out" check',
        "--algo=incremental",
        '--config="$config"',
        '--length="$length"',
        "--no-deadlock",
        'grep -Fc "The outcome is: NoError"',
        'grep -Fc "Checker reports no error up to computation length ${length}"',
        'echo "multilane formal or production sources changed during the Apalache run"',
        "printf 'result_count\\t3\\n'",
        'mv -- "$evidence_tmp" "$EVIDENCE_PATH"',
        "[apalache] all 3 source-bound multilane kernels passed pinned",
    )
    for token in required_once:
        count = source.count(token)
        if count != 1:
            errors.append(
                f"multilane Apalache runner must contain {token!r} exactly once, "
                f"found {count}"
            )
    manifest_calls = source.count(
        'python3 -I -S "$CONTRACT_CHECKER" --print-source-manifest-sha256'
    )
    if manifest_calls != 2:
        errors.append(
            "multilane Apalache runner must source-seal before and after its "
            f"bounded checks, found {manifest_calls} manifest calls"
        )
    exit_marker_count = source.count('grep -Fxc "EXITCODE: OK"')
    if exit_marker_count != 2:
        errors.append(
            "multilane Apalache runner must require the exact EXITCODE: OK "
            f"marker in typecheck and bounded-check paths, found {exit_marker_count}"
        )

    expected_calls = (
        """run_positive \\
  autoscale-lifecycle \\
  "$AUTOSCALE_MODULE" \\
  multilane_autoscale_lifecycle_fixed.cfg \\
  8 \\
  "LifecycleTypeInvariant, StorageBeforeActivationInvariant, DrainEvidenceInvariant, ArchiveBeforeDestroyInvariant, NoIncarnationReuseInvariant\"""",
        """run_positive \\
  native-application-evidence \\
  "$NATIVE_MODULE" \\
  multilane_native_application_evidence_fixed.cfg \\
  5 \\
  "NativeEvidenceTypeInvariant, SidecarsRequireManifestInvariant, FrontierPublicationInvariant, PrunedEvidenceVerifiableInvariant, SameRouteControlOnlyInvariant\"""",
        """run_positive \\
  autonomous-reservation-carrier \\
  "$AUTONOMOUS_MODULE" \\
  multilane_autonomous_reservation_carrier_fixed.cfg \\
  10 \\
  "ReservationCarrierTypeInvariant, SingleOwnershipInvariant, ExactCarrierIdentityInvariant, ControlOnlyAnchorInvariant, CandidateAuthorizationInvariant, ReleaseOrderingInvariant, QueueReleaseCompletionInvariant, AtMostOnceApplicationInvariant, NoReleaseAfterApplicationInvariant, NoStaleIncarnationReleaseInvariant, ForgottenOnlyAfterApplicationInvariant\"""",
    )
    for call in expected_calls:
        if source.count(call) != 1:
            label = call.splitlines()[1].strip(" \\")
            errors.append(
                f"multilane Apalache runner must contain the exact {label} "
                "bounded positive contract"
            )

    for forbidden in (
        "APALACHE_LENGTH",
        "multilane_autoscale_early_drain_bug.cfg",
        "multilane_autoscale_destroy_before_archive_bug.cfg",
        "multilane_autoscale_incarnation_reuse_bug.cfg",
        "multilane_native_frontier_before_sidecars_bug.cfg",
        "multilane_native_hash_only_pruning_bug.cfg",
        "multilane_native_same_route_marker_bug.cfg",
        "multilane_autonomous_carrier_drift_bug.cfg",
        "multilane_autonomous_duplicate_application_bug.cfg",
        "multilane_autonomous_release_after_apply_bug.cfg",
        "multilane_autonomous_release_before_barrier_bug.cfg",
        "multilane_autonomous_aba_release_bug.cfg",
        "multilane_autonomous_digest_only_authorization_bug.cfg",
        "multilane_autonomous_ordinary_anchor_execution_bug.cfg",
    ):
        if forbidden in source:
            errors.append(
                f"multilane Apalache runner contains prohibited override or "
                f"TLC-owned mutation {forbidden!r}"
            )
    return errors


def _validate_apalache_gate(root: Path, errors: list[str]) -> None:
    runner = root / APALACHE_RUNNER_RELATIVE
    if _regular_file(runner, "multilane Apalache runner", errors):
        if runner.stat().st_mode & 0o111 == 0:
            errors.append(f"multilane Apalache runner must be executable: {runner}")
        errors.extend(_apalache_runner_source_errors(runner.read_text(encoding="utf-8")))

    runner_test = root / APALACHE_RUNNER_TEST_RELATIVE
    _regular_file(runner_test, "multilane Apalache runner contract test", errors)

    installer = root / APALACHE_INSTALLER_RELATIVE
    if _regular_file(installer, "pinned Apalache installer", errors):
        installer_source = installer.read_text(encoding="utf-8")
        installer_tokens = (
            f'readonly pinned_version="{APALACHE_VERSION}"',
            f'readonly pinned_archive_sha256="{APALACHE_ARCHIVE_SHA256}"',
            f'readonly pinned_launcher_sha256="{APALACHE_LAUNCHER_SHA256}"',
            f'readonly pinned_jar_sha256="{APALACHE_JAR_SHA256}"',
            'if [[ "$version" != "$pinned_version" ]]',
            '[[ "$expected_sha256" != "$pinned_archive_sha256" ]]',
            '[[ "$actual_sha256" != "$pinned_archive_sha256" ]]',
        )
        for token in installer_tokens:
            if installer_source.count(token) != 1:
                errors.append(
                    f"{installer}: pinned installer contract must contain "
                    f"{token!r} exactly once"
                )

    tlc_runner = root / TLC_RUNNER_RELATIVE
    if _regular_file(tlc_runner, "Sumeragi v2 TLC runner", errors):
        tlc_source = tlc_runner.read_text(encoding="utf-8")
        for token in (
            'readonly MULTILANE_APALACHE_RUNNER="${REPO_ROOT}/scripts/formal/run_sumeragi_v2_multilane_apalache.sh"',
            'bash "$MULTILANE_APALACHE_RUNNER"',
        ):
            if tlc_source.count(token) != 1:
                errors.append(
                    f"{tlc_runner}: default TLC release matrix must contain "
                    f"{token!r} exactly once"
                )

    workflow_install_block = """      - name: Install pinned formal tools
        run: |
          bash scripts/formal/install_sumeragi_v2_tlapm.sh
          bash scripts/formal/install_sumeragi_v2_tla2tools.sh
          bash scripts/formal/install_apalache.sh 0.52.2
          bash scripts/formal/install_sumeragi_v2_verus.sh
"""
    for workflow_relative in FORMAL_WORKFLOW_RELATIVES:
        workflow = root / workflow_relative
        if _regular_file(workflow, "Sumeragi v2 formal workflow", errors):
            workflow_source = workflow.read_text(encoding="utf-8")
            if workflow_source.count(workflow_install_block) != 1:
                errors.append(
                    f"{workflow}: pinned formal install block must contain the "
                    "Apalache 0.52.2 installer exactly once"
                )

    readme = root / README_RELATIVE
    if _regular_file(readme, "Sumeragi v2 formal README", errors):
        readme_source = readme.read_text(encoding="utf-8")
        for token in (
            "`run_sumeragi_v2_multilane_apalache.sh`",
            "pinned Apalache 0.52.2",
            "| autoscale lifecycle | `multilane_autoscale_lifecycle_fixed.cfg` | 8 |",
            "| Native application evidence | `multilane_native_application_evidence_fixed.cfg` | 5 |",
            "| autonomous reservation/carrier | `multilane_autonomous_reservation_carrier_fixed.cfg` | 10 |",
            "they are not TLAPS",
            "cross-tool proof evidence",
            "do not change proof-ledger status",
        ):
            if token not in readme_source:
                errors.append(
                    f"{readme}: missing multilane Apalache documentation contract "
                    f"{token!r}"
                )


def source_manifest_sha256(root: Path = DEFAULT_ROOT) -> str:
    """Hash every source/config/script and production item owned by this gate."""

    root = root.resolve()
    formal_dir = root / FORMAL_RELATIVE
    ledger_path = formal_dir / BINDINGS_FILENAME
    ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
    relative_paths = {
        FORMAL_RELATIVE / BINDINGS_FILENAME,
        README_RELATIVE,
        APALACHE_RUNNER_RELATIVE,
        APALACHE_RUNNER_TEST_RELATIVE,
        APALACHE_INSTALLER_RELATIVE,
        TLC_RUNNER_RELATIVE,
        TLC_MUTATION_RUNNER_RELATIVE,
        *FORMAL_WORKFLOW_RELATIVES,
        Path("scripts/formal/check_sumeragi_v2_multilane_models.py"),
    }
    for model in ledger["models"]:
        relative_paths.add(FORMAL_RELATIVE / f"{model['module']}.tla")
        relative_paths.add(FORMAL_RELATIVE / model["positive_config"])
        for mutation in model["mutations"]:
            relative_paths.add(FORMAL_RELATIVE / mutation["config"])
        for binding in model["production_symbols"]:
            relative_paths.add(Path(binding["path"]))

    digest = hashlib.sha256()
    for relative in sorted(relative_paths, key=lambda path: path.as_posix()):
        payload = (root / relative).read_bytes()
        encoded_path = relative.as_posix().encode("utf-8")
        digest.update(len(encoded_path).to_bytes(8, "big"))
        digest.update(encoded_path)
        digest.update(len(payload).to_bytes(8, "big"))
        digest.update(payload)
    return digest.hexdigest()


def _validate_model(
    root: Path, formal_dir: Path, model: Any, errors: list[str]
) -> None:
    if not isinstance(model, dict):
        errors.append("each multilane model binding must be an object")
        return
    expected_keys = {
        "module",
        "positive_config",
        "production_refinement_obligation",
        "mutations",
        "production_symbols",
    }
    if set(model) != expected_keys:
        errors.append(
            f"multilane model fields must equal {sorted(expected_keys)}, "
            f"found {sorted(model)}"
        )
        return
    module = model.get("module")
    positive_config = model.get("positive_config")
    obligation = model.get("production_refinement_obligation")
    if not all(
        _nonempty_string(value)
        for value in (module, positive_config, obligation)
    ):
        errors.append("module, positive_config, and obligation must be non-empty")
        return

    module_path = formal_dir / f"{module}.tla"
    if _regular_file(module_path, "multilane TLA+ module", errors):
        source = module_path.read_text(encoding="utf-8")
        header = MODULE_RE.search(source)
        if header is None or header.group(1) != module:
            errors.append(f"{module_path}: module header must declare {module}")
        if not source.rstrip().endswith("===="):
            errors.append(f"{module_path}: module must end with ====")
        obligation_re = re.compile(
            TLA_DECLARATION_TEMPLATE.format(symbol=re.escape(obligation))
        )
        if obligation_re.search(source) is None:
            errors.append(
                f"{module_path}: missing production refinement obligation {obligation}"
            )

    positive_path = formal_dir / positive_config
    if _regular_file(positive_path, "positive multilane TLC config", errors):
        positive_source = positive_path.read_text(encoding="utf-8")
        if not positive_source.startswith("INIT Init\nNEXT Next\n"):
            errors.append(
                f"{positive_path}: positive config must use the executable Init/Next kernel"
            )
        if "_fixed.cfg" not in positive_config:
            errors.append(
                f"{positive_path}: positive config name must end in _fixed.cfg"
            )

    mutations = model.get("mutations")
    if not isinstance(mutations, list) or not mutations:
        errors.append(f"{module}: mutations must be a non-empty array")
    else:
        seen_configs: set[str] = set()
        for mutation in mutations:
            if not isinstance(mutation, dict) or set(mutation) != {
                "config",
                "invariant",
            }:
                errors.append(
                    f"{module}: each mutation must contain only config and invariant"
                )
                continue
            config = mutation.get("config")
            invariant = mutation.get("invariant")
            if not _nonempty_string(config) or not _nonempty_string(invariant):
                errors.append(f"{module}: mutation config/invariant must be non-empty")
                continue
            if config in seen_configs:
                errors.append(f"{module}: duplicate mutation config {config}")
            seen_configs.add(config)
            config_path = formal_dir / config
            if not _regular_file(
                config_path, "multilane mutation TLC config", errors
            ):
                continue
            config_source = config_path.read_text(encoding="utf-8")
            if f'INVARIANT {invariant}\n' not in config_source:
                errors.append(
                    f"{config_path}: mutation must check named invariant {invariant}"
                )
            if "_bug.cfg" not in config:
                errors.append(f"{config_path}: mutation config must end in _bug.cfg")

    symbols = model.get("production_symbols")
    if not isinstance(symbols, list) or not symbols:
        errors.append(f"{module}: production_symbols must be a non-empty array")
        return
    seen_bindings: set[tuple[str, str]] = set()
    for binding in symbols:
        if not isinstance(binding, dict) or set(binding) != {
            "path",
            "kind",
            "symbol",
            "required_tokens",
        }:
            errors.append(
                f"{module}: each production binding must contain path, kind, "
                "symbol, and required_tokens"
            )
            continue
        relative = binding.get("path")
        kind = binding.get("kind")
        symbol = binding.get("symbol")
        tokens = binding.get("required_tokens")
        if (
            not _nonempty_string(relative)
            or kind not in RUST_DECLARATION_TEMPLATES
            or not _nonempty_string(symbol)
            or not isinstance(tokens, list)
            or not tokens
            or not all(_nonempty_string(token) for token in tokens)
        ):
            errors.append(f"{module}: malformed production binding {binding!r}")
            continue
        if Path(relative).is_absolute() or ".." in Path(relative).parts:
            errors.append(f"{module}: production path must stay within repo: {relative}")
            continue
        key = (relative, symbol)
        if key in seen_bindings:
            errors.append(f"{module}: duplicate production binding {relative}!{symbol}")
        seen_bindings.add(key)
        path = root / relative
        if not _regular_file(path, "production binding source", errors):
            continue
        source = path.read_text(encoding="utf-8")
        declaration_re = re.compile(
            RUST_DECLARATION_TEMPLATES[kind].format(symbol=re.escape(symbol))
        )
        declarations = list(declaration_re.finditer(source))
        if len(declarations) != 1:
            errors.append(
                f"{path}: production symbol {symbol} must have one {kind} "
                f"declaration, found {len(declarations)}"
            )
            continue
        item = _extract_braced_item(source, declarations[0])
        if item is None:
            errors.append(f"{path}: cannot extract production item {symbol}")
            continue
        for token in tokens:
            if token not in item:
                errors.append(
                    f"{path}: production item {symbol} is missing source-binding "
                    f"token {token!r}"
                )


def validate(root: Path = DEFAULT_ROOT) -> tuple[str, ...]:
    """Return structural/source-binding errors for the multilane model slice."""

    errors: list[str] = []
    root = root.resolve()
    formal_dir = root / FORMAL_RELATIVE
    bindings_path = formal_dir / BINDINGS_FILENAME
    if not _regular_file(bindings_path, "multilane source binding ledger", errors):
        return tuple(errors)
    try:
        ledger = json.loads(bindings_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        return (f"cannot load {bindings_path}: {error}",)
    if not isinstance(ledger, dict) or set(ledger) != {"schema_version", "models"}:
        return (
            "multilane binding ledger must contain exactly schema_version and models",
        )
    if ledger.get("schema_version") != 1:
        errors.append("multilane binding ledger schema_version must equal 1")
    models = ledger.get("models")
    if not isinstance(models, list) or len(models) != 3:
        errors.append("multilane binding ledger must contain exactly three models")
        return tuple(errors)
    modules = [model.get("module") for model in models if isinstance(model, dict)]
    if len(set(modules)) != len(modules):
        errors.append("multilane binding ledger contains duplicate model modules")
    for model in models:
        _validate_model(root, formal_dir, model, errors)
    _validate_apalache_gate(root, errors)
    return tuple(errors)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=DEFAULT_ROOT,
        help="repository root (defaults to the checker-derived root)",
    )
    parser.add_argument(
        "--print-source-manifest-sha256",
        action="store_true",
        help="print the current source-bound multilane gate manifest digest",
    )
    return parser


def main() -> int:
    args = _parser().parse_args()
    errors = validate(args.root)
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    if args.print_source_manifest_sha256:
        print(source_manifest_sha256(args.root))
        return 0
    print(
        "Sumeragi v2 multilane models are structurally valid and bound to "
        "current production symbols"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
