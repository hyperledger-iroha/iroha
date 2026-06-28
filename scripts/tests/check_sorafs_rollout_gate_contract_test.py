"""Static contract tests for SoraFS rollout evidence gates."""

from __future__ import annotations

import ast
import hashlib
import importlib.util
import json
import re
import sys
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
REPO_ROOT = SCRIPTS_DIR.parent
DOCS_SOURCE_DIR = REPO_ROOT / "docs" / "source"
EXAMPLES_DIR = SCRIPTS_DIR / "examples"
DASHBOARDS_DIR = REPO_ROOT / "dashboards"
TELEMETRY_METRICS_RS = REPO_ROOT / "crates" / "iroha_telemetry" / "src" / "metrics.rs"
HEDGING_FIXTURE_GENERATOR_RS = (
    REPO_ROOT
    / "crates"
    / "sorafs_manifest"
    / "src"
    / "bin"
    / "generate_hedging_fixtures.rs"
)
HEDGING_FIXTURE_ROOT = REPO_ROOT / "fixtures" / "sorafs_manifest" / "hedging"
HEDGING_FIXTURE_README = HEDGING_FIXTURE_ROOT / "README.md"
HEDGING_FIXTURE_MANIFEST = (
    HEDGING_FIXTURE_ROOT / "fixture_manifest.json"
)
HEDGING_FIXTURE_CHECKER = SCRIPTS_DIR / "check_sorafs_hedging_fixture_manifest.py"
HEDGING_FIXTURE_CHECKER_TEST = (
    SCRIPTS_DIR / "tests" / "check_sorafs_hedging_fixture_manifest_test.py"
)
CHECKER_PREFLIGHT = SCRIPTS_DIR / "sorafs_checker_preflight.py"
CHECKER_PREFLIGHT_TEST = SCRIPTS_DIR / "tests" / "sorafs_checker_preflight_test.py"
RUNNER_PREFLIGHT_TEST = SCRIPTS_DIR / "tests" / "sorafs_runner_preflight_test.py"
EVIDENCE_PATHS_HELPER = SCRIPTS_DIR / "sorafs_evidence_paths.py"
EVIDENCE_PATHS_TEST = SCRIPTS_DIR / "tests" / "sorafs_evidence_paths_test.py"
PATH_IDENTITY_HELPER = SCRIPTS_DIR / "sorafs_path_identity.py"
PATH_IDENTITY_TEST = SCRIPTS_DIR / "tests" / "sorafs_path_identity_test.py"
EVIDENCE_JSON_HELPER = SCRIPTS_DIR / "sorafs_evidence_json.py"
EVIDENCE_JSON_TEST = SCRIPTS_DIR / "tests" / "sorafs_evidence_json_test.py"
EVIDENCE_FINGERPRINT_HELPER = SCRIPTS_DIR / "sorafs_evidence_fingerprint.py"
EVIDENCE_VALIDATION_HELPER = SCRIPTS_DIR / "sorafs_evidence_validation.py"
SENSITIVITY_HELPER = SCRIPTS_DIR / "sorafs_evidence_sensitivity.py"
SENSITIVITY_TEST = SCRIPTS_DIR / "tests" / "sorafs_evidence_sensitivity_test.py"
RESPONSE_ARGS_HELPER = SCRIPTS_DIR / "sorafs_response_args.py"
RESPONSE_ARGS_TEST = SCRIPTS_DIR / "tests" / "sorafs_response_args_test.py"
REQUIRED_KINDS_HELPER = SCRIPTS_DIR / "sorafs_required_kinds.py"
CHECKERS = sorted(SCRIPTS_DIR.glob("check_sorafs_*rollout_evidence.py")) + [
    SCRIPTS_DIR / "check_sorafs_reference_sdk_release_evidence.py"
]
RUNNERS = sorted(SCRIPTS_DIR.glob("run_sorafs_*rollout_evidence.py")) + [
    SCRIPTS_DIR / "run_sorafs_reference_sdk_release_evidence.py"
]
COMMON_SENSITIVE_KEYS = (
    "authorization",
    "bearer_token",
    "private_key",
    "response_body",
    "secret",
)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def function_source(path: Path, function_name: str) -> str:
    source_text = read(path)
    module = ast.parse(source_text)
    for node in module.body:
        if isinstance(node, ast.FunctionDef) and node.name == function_name:
            return ast.get_source_segment(source_text, node) or ""
    return ""


def is_call(node: ast.AST, function_name: str) -> bool:
    return (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id == function_name
    )


def load_script_module(path: Path, module_name: str):
    script_dir = str(path.parent)
    if script_dir not in sys.path:
        sys.path.insert(0, script_dir)
    spec = importlib.util.spec_from_file_location(module_name, path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)
    return module


def response_args_module():
    return load_script_module(RESPONSE_ARGS_HELPER, "sorafs_response_args_contract")


def checker_names() -> list[str]:
    return [path.name for path in CHECKERS]


def runner_names() -> list[str]:
    return [path.name for path in RUNNERS]


def bounded_json_checkers() -> list[Path]:
    return CHECKERS


def standard_json_error_checkers() -> list[Path]:
    return [
        path
        for path in CHECKERS
        if path.name != "check_sorafs_reputation_rollout_evidence.py"
    ]


def fingerprint_checkers() -> list[Path]:
    return bounded_json_checkers()


def standard_artifact_checkers() -> list[Path]:
    return [
        path
        for path in CHECKERS
        if path.name != "check_sorafs_reputation_rollout_evidence.py"
    ]


def string_coverage_checkers() -> list[Path]:
    return [
        path
        for path in CHECKERS
        if path.name != "check_sorafs_reputation_rollout_evidence.py"
    ]


def basic_validation_checkers() -> list[Path]:
    return CHECKERS


def checker_names_without(*excluded: str) -> set[str]:
    return {path.name for path in CHECKERS if path.name not in excluded}


def timestamp_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def environment_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def deployment_id_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def iroha_config_binding_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def iroha_config_bound_checkers() -> set[str]:
    return iroha_config_binding_checkers() - {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
    }


def governance_approval_validation_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def config_backed_governance_approval_validation_checkers() -> set[str]:
    return governance_approval_validation_checkers() - {
        "check_sorafs_reference_sdk_release_evidence.py"
    }


def policy_digest_validation_checkers() -> set[str]:
    return governance_approval_validation_checkers() | {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def hex_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def direct_hex_validation_checkers() -> set[str]:
    return set()


def optional_hex_validation_checkers() -> set[str]:
    return {"check_sorafs_ai_prescreen_rollout_evidence.py"}


def hex_string_array_validation_checkers() -> set[str]:
    return {
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
    }


def score_bps_validation_checkers() -> set[str]:
    return {"check_sorafs_ai_prescreen_rollout_evidence.py"}


def int_range_validation_checkers() -> set[str]:
    return {"check_sorafs_reputation_rollout_evidence.py"}


def advancing_int_pair_validation_checkers() -> set[str]:
    return {"check_sorafs_reputation_rollout_evidence.py"}


def count_match_validation_checkers() -> set[str]:
    return {"check_sorafs_transparency_rollout_evidence.py"}


def count_value_equal_validation_checkers() -> set[str]:
    return {"check_sorafs_moderation_panel_rollout_evidence.py"}


def count_length_match_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
    }


def sum_equal_validation_checkers() -> set[str]:
    return {
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def zero_count_validation_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def minimum_int_validation_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def minimum_value_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def maximum_value_validation_checkers() -> set[str]:
    return {"check_sorafs_moderation_panel_rollout_evidence.py"}


def maximum_number_validation_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def maximum_int_validation_checkers() -> set[str]:
    return {
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
    }


def passed_status_validation_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def status_in_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
    }


def string_in_validation_checkers() -> set[str]:
    return {"check_sorafs_por_rollout_evidence.py"}


def string_not_equal_validation_checkers() -> set[str]:
    return {"check_sorafs_pop_credentials_rollout_evidence.py"}


def string_value_equal_validation_checkers() -> set[str]:
    return {"check_sorafs_reputation_rollout_evidence.py"}


def schema_string_type_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def tuple_binding_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def tuple_binding_error_recorder_checkers() -> set[str]:
    return set()


def tuple_bound_reference_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def scalar_binding_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def scalar_bound_digest_reference_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def scalar_binding_error_recorder_checkers() -> set[str]:
    return scalar_binding_validation_checkers()


def http_status_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }


def route_probe_object_array_checkers() -> set[str]:
    return http_status_validation_checkers()


def generic_object_array_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
    }


def positive_int_arg_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def non_negative_int_arg_checkers() -> set[str]:
    return {
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def positive_int_arg_runners() -> set[str]:
    return set(runner_names())


def non_negative_int_arg_runners() -> set[str]:
    return {
        "run_sorafs_appeal_finance_rollout_evidence.py",
        "run_sorafs_gateway_compliance_rollout_evidence.py",
        "run_sorafs_governance_dag_rollout_evidence.py",
        "run_sorafs_hedging_rollout_evidence.py",
        "run_sorafs_moderation_panel_rollout_evidence.py",
        "run_sorafs_orderbook_rollout_evidence.py",
        "run_sorafs_pdp_rollout_evidence.py",
        "run_sorafs_pop_credentials_rollout_evidence.py",
        "run_sorafs_por_rollout_evidence.py",
        "run_sorafs_potr_rollout_evidence.py",
        "run_sorafs_reference_sdk_release_evidence.py",
        "run_sorafs_repair_rollout_evidence.py",
        "run_sorafs_reputation_rollout_evidence.py",
        "run_sorafs_reserve_rent_rollout_evidence.py",
    }


def false_validation_checkers() -> set[str]:
    return checker_names_without("check_sorafs_reputation_rollout_evidence.py")


def false_or_absent_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
    }


def false_or_governed_validation_checkers() -> set[str]:
    return {"check_sorafs_hedging_rollout_evidence.py"}


def non_negative_number_validation_checkers() -> set[str]:
    return set()


def artifact_error_checkers() -> list[Path]:
    return CHECKERS


def required_summary_checkers() -> list[Path]:
    return standard_artifact_checkers()


def runner_example_candidates(path: Path) -> list[Path]:
    name = path.name.removeprefix("run_").removesuffix(".py")
    if name.endswith("_rollout_evidence"):
        stem = name.removesuffix("_rollout_evidence")
        return [
            EXAMPLES_DIR / f"{stem}_rollout_collection.args.example",
            EXAMPLES_DIR / f"{stem}_rollout_evidence.args.example",
        ]
    if name.endswith("_release_evidence"):
        stem = name.removesuffix("_release_evidence")
        return [
            EXAMPLES_DIR / f"{stem}_release_collection.args.example",
            EXAMPLES_DIR / f"{stem}_release_evidence.args.example",
        ]
    raise AssertionError(f"unexpected runner name: {path.name}")


def checker_example(path: Path) -> Path:
    name = path.name.removeprefix("check_").removesuffix(".py")
    if name.endswith("_rollout_evidence"):
        stem = name.removesuffix("_rollout_evidence")
        return EXAMPLES_DIR / f"{stem}_rollout_evidence.args.example"
    if name.endswith("_release_evidence"):
        stem = name.removesuffix("_release_evidence")
        return EXAMPLES_DIR / f"{stem}_release_evidence.args.example"
    raise AssertionError(f"unexpected checker name: {path.name}")


def runner_example(path: Path) -> Path | None:
    return next(
        (candidate for candidate in runner_example_candidates(path) if candidate.is_file()),
        None,
    )


def transparency_required_source_kinds() -> set[str]:
    source = ast.parse(read(SCRIPTS_DIR / "check_sorafs_transparency_rollout_evidence.py"))
    for node in source.body:
        if not isinstance(node, ast.Assign):
            continue
        if not any(
            isinstance(target, ast.Name)
            and target.id == "DEFAULT_REQUIRED_SOURCE_KINDS"
            for target in node.targets
        ):
            continue
        return set(ast.literal_eval(node.value))
    raise AssertionError("DEFAULT_REQUIRED_SOURCE_KINDS not found")


def test_rollout_gate_contract_fixtures_cover_every_checker() -> None:
    assert CHECKERS
    assert len(checker_names()) == len(set(checker_names()))


def test_rollout_runner_contract_fixtures_cover_every_runner() -> None:
    assert RUNNERS
    assert len(runner_names()) == len(set(runner_names()))


def test_rollout_checkers_accept_reviewed_response_files() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if "EvidenceArgumentParser" not in read(path)
        or "sorafs_response_args" not in read(path)
        or "expand_response_args(" not in read(path)
        or "fromfile_prefix_chars" in read(path)
    ]

    assert missing == []


def test_reviewed_response_files_use_shared_path_identity_resolution() -> None:
    helper = read(PATH_IDENTITY_HELPER)
    helper_test = read(PATH_IDENTITY_TEST)
    response_args = read(RESPONSE_ARGS_HELPER)
    response_args_test = read(RESPONSE_ARGS_TEST)

    assert "def resolve_path_identity" in helper
    assert "def _require_error_list" in helper
    assert "def _require_label" in helper
    assert "def _require_failure_template" in helper
    assert "class _PathIdentityFailureTemplateError(ValueError)" in helper
    assert 'str(error).startswith("path identity failure template")' not in helper
    assert "except _PathIdentityFailureTemplateError:" in helper
    assert "def path_diagnostic_label" in helper
    assert "def error_diagnostic_label" in helper
    assert "def _path_label" in helper
    assert "def _error_label" in helper
    assert "<non-path>" in helper
    assert "<non-canonical-path>" in helper
    assert "<non-canonical-error>" in helper
    assert "ALLOWED_FAILURE_TEMPLATE_FIELDS" in helper
    assert "Formatter().parse(failure_template)" in helper
    assert "path identity errors must be a list of strings" in helper
    assert "path identity errors must contain non-empty canonical strings" in helper
    assert "path identity label must be a non-empty canonical string" in helper
    assert "path identity failure template must be a non-empty string" in helper
    assert "path identity failure template must include {path} and {error}" in helper
    assert (
        "path identity failure template fields must be label, path, or error"
        in helper
    )
    assert "path identity failure template must be valid format text" in helper
    assert (
        "path identity failure template fields must not use format specifiers"
        in helper
    )
    assert "isinstance(path, Path)" in helper
    assert "must be a path" in helper
    assert "return path.resolve()" in helper
    assert "except (OSError, RuntimeError)" in helper
    assert "test_resolve_path_identity_rejects_non_path_without_traceback" in helper_test
    assert (
        "test_resolve_path_identity_sanitizes_malformed_non_path_labels"
        in helper_test
    )
    assert "test_path_diagnostic_label_sanitizes_malformed_values" in helper_test
    assert "test_error_diagnostic_label_sanitizes_malformed_values" in helper_test
    assert "test_resolve_path_identity_rejects_malformed_error_container" in helper_test
    assert (
        "test_resolve_path_identity_rejects_malformed_existing_error_text"
        in helper_test
    )
    assert (
        "test_resolve_path_identity_rejects_malformed_labels_before_resolution"
        in helper_test
    )
    assert (
        "test_resolve_path_identity_rejects_malformed_failure_templates_before_resolution"
        in helper_test
    )
    assert "test_resolve_path_identity_records_custom_failure" in helper_test
    assert (
        "test_resolve_path_identity_sanitizes_noncanonical_resolver_failure"
        in helper_test
    )
    assert "error_diagnostic_label" in response_args
    assert "path_diagnostic_label" in response_args
    assert "resolve_path_identity" in response_args
    assert "resolve_path_identity(" in response_args
    assert "path.resolve()" not in response_args
    assert "isinstance(values, (str, bytes, bytearray))" in response_args
    assert "def _require_argument_string" in response_args
    assert "argument must be a non-empty canonical string" in response_args
    assert "test_response_file_resolution_uses_shared_identity_helper" in response_args_test
    assert "test_response_file_stat_failure_sanitizes_malformed_error" in response_args_test
    assert "test_response_file_read_failure_sanitizes_malformed_error" in response_args_test
    assert (
        "test_response_file_line_parse_error_sanitizes_malformed_error"
        in response_args_test
    )
    assert (
        "test_raw_bytearray_argument_container_fails_without_byte_expansion"
        in response_args_test
    )
    assert "test_malformed_direct_argument_text_fails_closed" in response_args_test
    assert (
        "test_response_file_parser_returning_malformed_line_arg_fails_with_line"
        in response_args_test
    )


def test_rollout_checkers_have_operator_argfile_examples() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if not checker_example(path).is_file()
    ]

    assert missing == []


def test_rollout_checker_examples_document_runtime_only_evidence() -> None:
    missing: dict[str, str] = {}
    pattern = re.compile(
        r"payload-free|runtime-only|runtime|secret|bearer token|signing key|raw",
        re.I,
    )
    for path in CHECKERS:
        example = checker_example(path)
        assert example.is_file()
        if pattern.search(read(example)) is None:
            missing[path.name] = str(example.relative_to(SCRIPTS_DIR.parent))

    assert missing == {}


def test_rollout_runners_accept_reviewed_response_files() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "EvidenceArgumentParser" not in read(path)
        or "sorafs_response_args" not in read(path)
        or "expand_response_args(" not in read(path)
    ]

    assert missing == []


def test_rollout_runners_have_operator_argfile_examples() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if runner_example(path) is None
    ]

    assert missing == []


def test_rollout_runners_have_collection_argfile_examples() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if not runner_example_candidates(path)[0].is_file()
    ]

    assert missing == []


def test_rollout_runner_examples_show_dry_run_review() -> None:
    missing: dict[str, str] = {}
    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        source = read(example)
        if "--dry-run" not in source:
            missing[path.name] = str(example.relative_to(SCRIPTS_DIR.parent))

    assert missing == {}


def test_rollout_runner_examples_document_runtime_only_evidence() -> None:
    missing: dict[str, str] = {}
    pattern = re.compile(
        r"payload-free|runtime-only|runtime|secret|bearer token|signing key|raw",
        re.I,
    )
    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        if pattern.search(read(example)) is None:
            missing[path.name] = str(example.relative_to(SCRIPTS_DIR.parent))

    assert missing == {}


def test_rollout_example_argfiles_do_not_use_handoff_placeholders() -> None:
    stale: dict[str, list[str]] = {}
    pattern = re.compile(
        r"^#\s*Replace\b|TODO|replace-me|changeme|your-",
        re.I | re.M,
    )
    for path in sorted(EXAMPLES_DIR.glob("sorafs_*_*.args.example")):
        matches = pattern.findall(read(path))
        if matches:
            stale[str(path.relative_to(REPO_ROOT))] = matches

    assert stale == {}


def test_rollout_example_argfiles_do_not_use_all_zero_hex_placeholders() -> None:
    stale: dict[str, list[str]] = {}
    pattern = re.compile(r"\b(?:0{16}|0{32}|0{40}|0{64})\b")
    for path in sorted(EXAMPLES_DIR.glob("sorafs_*_*.args.example")):
        matches = pattern.findall(read(path))
        if matches:
            stale[str(path.relative_to(REPO_ROOT))] = matches

    assert stale == {}


def test_rollout_example_argfiles_do_not_embed_runtime_secrets() -> None:
    leaked: dict[str, list[tuple[int, str]]] = {}
    pattern = re.compile(
        r"(--(?:authorization|bearer-token|private-key|secret|signing-key|token)\b"
        r"|(?:authorization|bearer_token|private_key|secret|signing_key|token)="
        r"|BEGIN [A-Z ]*PRIVATE KEY)",
        re.I,
    )
    for path in sorted(EXAMPLES_DIR.glob("sorafs_*_*.args.example")):
        matches: list[tuple[int, str]] = []
        for line_number, line in enumerate(read(path).splitlines(), start=1):
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            match = pattern.search(stripped)
            if match is not None:
                matches.append((line_number, match.group(0)))
        if matches:
            leaked[str(path.relative_to(REPO_ROOT))] = matches

    assert leaked == {}


def test_rollout_example_argfiles_expand_with_shared_response_parser() -> None:
    helper = response_args_module()
    parser = helper.EvidenceArgumentParser()
    failures: dict[str, str] = {}
    empty: list[str] = []

    for path in sorted(EXAMPLES_DIR.glob("sorafs_*_*.args.example")):
        try:
            expanded_args = helper.expand_response_args([f"@{path}"], parser)
        except ValueError as error:
            failures[str(path.relative_to(REPO_ROOT))] = str(error)
            continue
        if not expanded_args:
            empty.append(str(path.relative_to(REPO_ROOT)))

    assert failures == {}
    assert empty == []


def test_rollout_runner_examples_parse_with_runner_parsers() -> None:
    failures: dict[str, str] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_contract_{path.stem}")
        stderr = StringIO()
        try:
            with redirect_stderr(stderr):
                module.parse_args([f"@{example}", "--dry-run"])
        except SystemExit as error:
            detail = stderr.getvalue().strip().splitlines()
            failures[path.name] = detail[-1] if detail else f"SystemExit({error.code})"
        except Exception as error:
            failures[path.name] = f"{type(error).__name__}: {error}"

    assert failures == {}


def test_transparency_runner_example_covers_required_source_entry_kinds() -> None:
    runner = SCRIPTS_DIR / "run_sorafs_transparency_rollout_evidence.py"
    example = runner_example(runner)
    assert example is not None
    source = read(example)
    required_kinds = transparency_required_source_kinds()
    missing = sorted(
        source_kind
        for source_kind in required_kinds
        if f"{source_kind}=" not in source
    )

    assert required_kinds
    assert source.count("--source-entry") >= len(required_kinds)
    assert missing == []


def test_sorafs_hedging_billing_observability_pack_is_checked_in() -> None:
    expected = [
        DASHBOARDS_DIR / "grafana" / "sorafs_hedging_billing.json",
        DASHBOARDS_DIR / "alerts" / "sorafs_hedging_billing_rules.yml",
        DASHBOARDS_DIR / "alerts" / "tests" / "sorafs_hedging_billing_rules.test.yml",
    ]
    missing = [str(path.relative_to(REPO_ROOT)) for path in expected if not path.is_file()]

    assert missing == []


def test_sorafs_hedging_billing_observability_metrics_are_registered() -> None:
    source = read(TELEMETRY_METRICS_RS)
    expected_names = [
        "torii_sorafs_hedging_xor_usd_reference_price_micro_usd",
        "torii_sorafs_hedging_feed_lag_seconds",
        "torii_sorafs_hedging_feed_divergence_bps",
        "torii_sorafs_hedging_exposure_drift_bps",
        "torii_sorafs_billing_statement_generation_total",
        "torii_sorafs_billing_statement_failure_total",
        "torii_sorafs_billing_statement_ack_backlog",
        "torii_sorafs_billing_escrow_runway_seconds",
    ]
    missing_names = [name for name in expected_names if name not in source]
    expected_helpers = [
        "set_sorafs_hedging_reference_price_micro_usd",
        "set_sorafs_hedging_feed_lag_seconds",
        "set_sorafs_hedging_feed_divergence_bps",
        "set_sorafs_hedging_exposure_drift_bps",
        "record_sorafs_billing_statement_generation",
        "set_sorafs_billing_statement_ack_backlog",
        "set_sorafs_billing_escrow_runway_seconds",
        "records_hedging_billing_metrics_used_by_dashboard_and_alerts",
    ]
    missing_helpers = [helper for helper in expected_helpers if helper not in source]

    assert missing_names == []
    assert missing_helpers == []


def test_sorafs_hedging_billing_fixture_generator_is_checked_in() -> None:
    assert HEDGING_FIXTURE_GENERATOR_RS.is_file()
    assert HEDGING_FIXTURE_README.is_file()
    assert HEDGING_FIXTURE_MANIFEST.is_file()
    assert HEDGING_FIXTURE_CHECKER.is_file()
    assert HEDGING_FIXTURE_CHECKER_TEST.is_file()

    generator = read(HEDGING_FIXTURE_GENERATOR_RS)
    readme = read(HEDGING_FIXTURE_README)
    manifest = json.loads(read(HEDGING_FIXTURE_MANIFEST))
    checker = read(HEDGING_FIXTURE_CHECKER)
    checker_test = read(HEDGING_FIXTURE_CHECKER_TEST)
    expected_payloads = [
        "HedgingPriceFeedV1",
        "HedgingReferencePriceDecisionV1",
        "BillingLineItemV1",
        "BillingStatementV1",
        "stale_reference_price_decision_v1",
        "line_usd_mismatch_statement_v1",
        "tampered_totals_statement_v1",
    ]
    missing_generator = [payload for payload in expected_payloads if payload not in generator]
    missing_readme = [payload for payload in expected_payloads if payload not in readme]

    assert "generate_hedging_fixtures" in readme
    assert "fixture_manifest.json" in readme
    assert "norito_bytes_hex" in generator
    assert "SUMMARY_SCHEMA" in checker
    assert "--manifest-only" in checker
    assert "--validator-bin" in checker
    assert "validate_fixture_manifest_preflight" in checker
    assert "read_file_bytes" in checker
    assert "failed to read {label}" in checker
    assert "load_evidence_json_with_sha256" in checker
    assert "read_evidence_bytes" in checker
    assert "EvidenceFileTooLargeError" in checker
    assert 'str(error) == f"evidence file exceeds' not in checker
    assert "manifest is not valid bounded JSON object: {_error_label(error)}" not in checker
    assert "is not a valid bounded JSON object: \"\n            f\"{_error_label(error)}" not in checker
    assert "_error_label(error, path_label=path_label)" in checker
    assert "from sorafs_path_identity import" in checker
    assert "path_diagnostic_label(" in checker
    assert "error_diagnostic_label(" in checker
    assert "path.read_bytes()" not in checker
    assert "max_bytes=max_bytes" in checker
    assert "load_generated_json_sidecar" in checker
    assert (
        "test_full_mode_rejects_oversized_norito_with_shared_byte_reader"
        in checker_test
    )
    assert "test_read_file_bytes_uses_typed_oversize_error" in checker_test
    assert (
        "test_load_manifest_sanitizes_noncanonical_path_decode_error"
        in checker_test
    )
    assert (
        "test_generated_json_sidecar_sanitizes_noncanonical_path_decode_error"
        in checker_test
    )
    assert "failed to scan generated fixture root" in checker
    assert "render_checker_summary" in checker
    assert "write_checker_summary" in checker
    assert "validate_checker_summary_output" in checker
    assert "args.summary_out.parent.mkdir" not in checker
    assert "args.summary_out.write_text" not in checker
    assert "must not be the same path as --manifest" in checker
    assert "subprocess.run" in checker
    assert "shell=True" not in checker
    assert "validation_command is not shell-tokenizable: {error}" not in checker
    assert "validator execution failed: {error}" not in checker
    assert "validation_command is not shell-tokenizable:" in checker
    assert "f\"{_error_label(error)}\"" in checker
    assert "validator execution failed: {_error_label(error)}" in checker
    assert "validate_generated_inventory" in checker
    assert "validate_json_sidecar" in checker
    assert "JSON_SIDE_CAR_KEYS" in checker
    assert "EXPECTED_NEGATIVE_CASES" in checker
    assert "validate_status_path_contract" in checker
    assert "parse_fixture_path" in checker
    assert "VALIDATED_NORITO_PATH" in checker
    assert "MAX_U128_DECIMAL" in checker
    assert "lowercase even-length hex norito_bytes_hex" in checker
    assert "missing generated Norito fixture" in checker_test
    assert "test_summary_out_same_as_manifest_fails_before_write" in checker_test
    assert "test_preflight_sanitizes_non_path_summary_out_label" in checker_test
    assert "test_summary_out_directory_fails_before_write" in checker_test
    assert "test_summary_out_symlink_fails_before_manifest_read" in checker_test
    assert "test_summary_out_parent_chain_symlink_fails_before_write" in checker_test
    assert (
        "test_inspect_regular_file_sanitizes_noncanonical_path_and_error"
        in checker_test
    )
    assert "test_inspect_directory_sanitizes_non_path_label" in checker_test
    assert "test_manifest_read_error_writes_blocked_summary_without_traceback" in checker_test
    assert "test_manifest_rejects_non_standard_json_constants" in checker_test
    assert (
        "test_full_mode_rejects_non_object_json_sidecar_with_shared_loader"
        in checker_test
    )
    assert "test_full_mode_rejects_unreadable_generated_fixture_without_traceback" in checker_test
    assert "test_read_file_bytes_sanitizes_noncanonical_path_and_error" in checker_test
    assert "test_generated_json_sidecar_missing_path_sanitizes_label" in checker_test
    assert "test_validation_command_tokenize_error_is_sanitized" in checker_test
    assert "test_validator_execution_error_is_sanitized" in checker_test
    assert (
        "test_full_mode_rejects_fixture_inventory_scan_errors_without_traceback"
        in checker_test
    )
    assert "test_manifest_only_rejects_negative_case_drift" in checker_test
    assert "test_manifest_only_rejects_rejected_fixtures_outside_negative_dir" in checker_test
    assert "test_full_mode_rejects_missing_fixture_paths_without_traceback" in checker_test
    assert "test_full_mode_rejects_absolute_fixture_paths_before_read" in checker_test
    assert "test_full_mode_enforces_expected_validator_statuses" in checker_test
    assert "test_full_mode_rejects_validator_status_mismatch" in checker_test
    assert "test_full_mode_resolves_repo_relative_validator_binary" in checker_test
    assert "test_full_mode_rejects_command_injection_before_validator_exec" in checker_test
    assert "test_full_mode_rejects_unmanifested_generated_fixtures" in checker_test
    assert "test_full_mode_rejects_malformed_json_sidecar" in checker_test
    assert "test_full_mode_rejects_odd_length_norito_hex_without_traceback" in checker_test
    assert "test_full_mode_rejects_malformed_nested_json_sidecar" in checker_test
    assert "test_full_mode_rejects_json_sidecar_version_drift" in checker_test
    assert "test_full_mode_rejects_statement_sidecar_value_invariants" in checker_test
    assert "test_full_mode_rejects_sidecar_numeric_policy_bounds" in checker_test
    assert missing_generator == []
    assert missing_readme == []
    assert manifest["schema_version"] == 1
    assert manifest["fixture_family"] == "sorafs_hedging_billing"
    assert manifest["validation_scope"] == "generated_bytes_required"

    expected_names = {
        "price_feed_primary_v1",
        "price_feed_secondary_v1",
        "reference_price_decision_v1",
        "billing_line_storage_v1",
        "billing_line_egress_v1",
        "billing_line_incentive_credit_v1",
        "billing_statement_v1",
        "stale_reference_price_decision_v1",
        "line_usd_mismatch_statement_v1",
        "tampered_totals_statement_v1",
    }
    fixtures = manifest["fixtures"]
    assert {fixture["name"] for fixture in fixtures} == expected_names
    assert {fixture["expected_status"] for fixture in fixtures} == {
        "accepted",
        "rejected",
    }
    assert {fixture["kind"] for fixture in fixtures} == {
        "billing-line-item",
        "billing-statement",
        "price-feed",
        "reference-price-decision",
    }
    for fixture in fixtures:
        assert fixture["norito_path"].endswith(f"{fixture['name']}.to")
        assert fixture["json_path"].endswith(f"{fixture['name']}.json")
        assert fixture["validation_command"].startswith("sorafs-validate hedging ")
        assert fixture["norito_path"] in fixture["validation_command"]


def test_sorafs_hedging_billing_generated_fixtures_are_checked_in() -> None:
    manifest = json.loads(read(HEDGING_FIXTURE_MANIFEST))
    expected_paths = {
        REPO_ROOT / fixture[path_key]
        for fixture in manifest["fixtures"]
        for path_key in ("norito_path", "json_path")
    }
    generated_paths = {
        path
        for pattern in ("**/*.to", "**/*.json")
        for path in HEDGING_FIXTURE_ROOT.glob(pattern)
        if path.name != "fixture_manifest.json"
    }

    assert generated_paths == expected_paths
    empty_paths = [path for path in sorted(expected_paths) if path.stat().st_size == 0]
    assert empty_paths == []


def test_rollout_runners_support_dry_run_plans() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "--dry-run" not in read(path) or "args.dry_run" not in read(path)
    ]

    assert missing == []


def test_rollout_runners_preflight_required_files() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "sorafs_runner_preflight" not in read(path)
        or "require_existing_files" not in read(path)
        or "def require_existing_files(" in read(path)
        or "seen_input_files: dict[Path, tuple[str, Path]] = {}" not in read(path)
        or "seen=seen_input_files" not in read(path)
    ]
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")

    assert "must exist and be a file" in helper
    assert "duplicate" in helper
    assert "RuntimeError" in helper
    assert "InputFileIdentities" in helper
    assert missing == []


def test_rollout_runners_preflight_required_directories_with_shared_helper() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "def require_existing_dirs(" in read(path)
    ]
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")
    ai_runner = read(SCRIPTS_DIR / "run_sorafs_ai_prescreen_rollout_evidence.py")

    assert "def require_existing_dirs(" in helper
    assert "must exist and be a directory" in helper
    assert "InputDirIdentities" in helper
    assert "duplicate" in helper
    assert "require_existing_dirs" in ai_runner
    assert "seen_input_dirs: dict[Path, tuple[str, Path]] = {}" in ai_runner
    assert "seen=seen_input_dirs" in ai_runner
    assert missing == []


def test_rollout_runners_use_shared_command_plan_execution() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "run_command_plan" not in read(path)
        or "subprocess.run" in read(path)
        or "out_dir.mkdir" in read(path)
        or "failed to launch" in read(path)
    ]
    local_stderr_emitters = [
        path.name
        for path in RUNNERS
        if re.search(r"\n\s*print\(", read(path))
        or "for error in errors:" in read(path)
        or 'print(f"- {error}"' in read(path)
        or 'print(f"ERROR: {error}"' in read(path)
    ]
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")
    helper_test = read(RUNNER_PREFLIGHT_TEST)

    assert "def command_plan_steps" in helper
    assert "def validate_command_plan_step_shapes" in helper
    assert "COMMAND_PLAN_SHAPE_DIAGNOSTIC" in helper
    assert "command plan must be a sequence of steps" in helper
    assert "command-plan step" in helper
    assert "command must be a non-empty list of strings" in helper
    assert "command executable must be a non-empty canonical string" in helper
    assert "must not contain NUL bytes" in helper
    assert "must not contain control characters" in helper
    assert "isinstance(plan, (str, bytes, bytearray, Mapping))" in helper
    assert "def run_command_plan" in helper
    assert "def emit_runner_error_lines" in helper
    assert "def emit_runner_error_block" in helper
    assert "def emit_runner_notice" in helper
    assert "runner notice message must be a non-empty canonical string" in helper
    assert "def validate_runner_output_dir" in helper
    assert "def validate_runner_output_parent" in helper
    assert "parent chain" in helper
    assert "validate_command_plan_artifacts" in helper
    assert "def _reserved_output_path_sequence" in helper
    assert "reserved_output_paths" in helper
    assert 'label="reserved output"' in helper
    assert "paths must be a sequence" in helper
    assert "reserved output path" in helper
    assert "duplicate reserved output path" in helper
    assert "duplicate planned artifact" in helper
    assert "path_diagnostic_label(" in helper
    assert "error_diagnostic_label(" in helper
    assert "must not be the same path as reserved output" in helper
    assert "must not be a symlink" in helper
    assert 'parent_label = f"{output_label} parent"' in helper
    assert 'label=f"{label} artifact"' in helper
    assert "must not already exist" in helper
    assert "wrote empty expected artifact" in helper
    assert "reserved_output_paths=(out_dir,)" in helper
    assert "shape_errors = validate_command_plan_step_shapes(plan)" in helper
    assert "failed to launch" in helper
    assert "failed to launch: {error}" not in helper
    assert "failed to launch: {error_diagnostic_label(error)}" in helper
    assert "failed to create --out-dir" in helper
    assert "emit_runner_error_lines(errors)" in helper
    assert "emit_runner_notice(" in helper
    assert "test_command_plan_steps_rejects_scalar_and_mapping_containers" in helper_test
    assert "test_validate_command_plan_artifacts_rejects_malformed_plan_shapes" in helper_test
    assert (
        "test_validate_command_plan_artifacts_rejects_malformed_reserved_outputs"
        in helper_test
    )
    assert (
        "test_validate_command_plan_artifacts_rejects_duplicate_reserved_outputs"
        in helper_test
    )
    assert "test_run_command_plan_sanitizes_launch_failure" in helper_test
    assert (
        "test_validate_command_plan_artifacts_stops_after_reserved_output_errors"
        in helper_test
    )
    assert (
        "test_validate_command_plan_step_shapes_rejects_malformed_fields"
        in helper_test
    )
    assert (
        "test_validate_command_plan_step_shapes_sanitizes_malformed_artifact_labels"
        in helper_test
    )
    assert (
        "test_run_command_plan_rejects_malformed_plan_before_output_creation"
        in helper_test
    )
    assert "test_run_command_plan_sanitizes_output_creation_failure" in helper_test
    assert (
        "test_run_command_plan_rejects_malformed_step_before_output_creation"
        in helper_test
    )
    assert (
        "test_run_command_plan_rejects_malformed_command_entries_before_output_creation"
        in helper_test
    )
    assert "test_emit_runner_notice_rejects_malformed_message" in helper_test
    assert missing == []
    assert local_stderr_emitters == []


def test_rollout_runners_use_shared_plan_rendering() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "write_runner_plan," not in read(path)
        or "write_runner_plan(" not in read(path)
        or "json.dumps(" in read(path)
        or "import json" in read(path)
    ]
    unguarded = []
    for path in RUNNERS:
        main_source = function_source(path, "main")
        main_tree = ast.parse(main_source)
        write_count = sum(1 for node in ast.walk(main_tree) if is_call(node, "write_runner_plan"))
        guarded_write_count = 0
        for node in ast.walk(main_tree):
            if not isinstance(node, ast.If):
                continue
            if not (
                isinstance(node.test, ast.Attribute)
                and node.test.attr == "dry_run"
                and isinstance(node.test.value, ast.Name)
                and node.test.value.id == "args"
            ):
                continue
            guarded_write_count += sum(
                1
                for statement in node.body
                for child in ast.walk(statement)
                if is_call(child, "write_runner_plan")
            )
        if write_count != 1 or guarded_write_count != write_count:
            unguarded.append(path.name)
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")
    helper_test = read(RUNNER_PREFLIGHT_TEST)

    assert "def render_runner_plan" in helper
    assert "def write_runner_plan" in helper
    assert "runner plan must be an object" in helper
    assert 'json.dumps(plan, indent=2, sort_keys=True, allow_nan=False) + "\\n"' in helper
    assert "sys.stdout.write(render_runner_plan(plan))" in helper
    assert "except (TypeError, ValueError) as error" in helper
    assert "failed to render runner plan JSON" in helper
    assert "failed to render runner plan JSON: {error}" not in helper
    assert "failed to render runner plan JSON: {error_diagnostic_label(error)}" in helper
    assert "test_render_runner_plan_rejects_non_object_plan" in helper_test
    assert "test_write_runner_plan_reports_non_object_plan_without_stdout" in helper_test
    assert "test_write_runner_plan_sanitizes_malformed_render_error" in helper_test
    assert all("plan_errors = write_runner_plan" in read(path) for path in RUNNERS)
    assert all("emit_runner_error_lines(plan_errors)" in read(path) for path in RUNNERS)
    assert missing == []
    assert unguarded == []


def test_rollout_runner_mains_return_argparse_error_codes() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "parser.error(" in read(path)
        and (
            "except SystemExit as error" not in read(path)
            or "return error.code if isinstance(error.code, int) else 1" not in read(path)
        )
    ]

    assert missing == []


def test_rollout_runners_use_shared_caught_argument_error_reporting() -> None:
    collected_raw_errors = [
        path.name for path in RUNNERS if "errors.append(str(error))" in read(path)
    ]
    local_parser_error_handlers = [
        path.name
        for path in RUNNERS
        if re.search(
            r"except ValueError as error:\n\s*parser\.error\(str\(error\)\)",
            function_source(path, "parse_args"),
        )
    ]
    missing_shared_raises = [
        path.name
        for path in RUNNERS
        if "except ValueError as error:" in function_source(path, "parse_args")
        and "emit_runner_exception(error)\n        raise SystemExit(2) from error"
        not in function_source(path, "parse_args")
    ]
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")
    helper_test = read(RUNNER_PREFLIGHT_TEST)
    collected_error_runners = (
        "run_sorafs_ai_prescreen_rollout_evidence.py",
        "run_sorafs_reputation_rollout_evidence.py",
        "run_sorafs_transparency_rollout_evidence.py",
    )

    assert "def emit_runner_error_lines" in helper
    assert "def emit_runner_exception" in helper
    assert "def _runner_error_messages" in helper
    assert "runner error messages must be a sequence of strings" in helper
    assert "isinstance(errors, (str, bytes, bytearray, Mapping))" in helper
    assert "test_emit_runner_exception_sanitizes_malformed_message" in helper_test
    assert "test_emit_runner_exception_preserves_canonical_message" in helper_test
    assert "emit_runner_error_lines((str(error),))" not in "\n".join(
        read(path) for path in RUNNERS
    )
    assert collected_raw_errors == []
    for script_name in collected_error_runners:
        source = read(SCRIPTS_DIR / script_name)
        assert "from sorafs_path_identity import error_diagnostic_label" in source
        assert "errors.append(error_diagnostic_label(error))" in source
    assert local_parser_error_handlers == []
    assert missing_shared_raises == []


def test_transparency_runner_sanitizes_generated_artifact_annotation_errors() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_transparency_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_transparency_rollout_evidence_test.py"
    )

    assert "path_diagnostic_label" in runner
    assert "error_diagnostic_label(error, path_label=path_label)" in runner
    assert "failed to read generated evidence artifact `{path}`: {error}" not in runner
    assert "failed to write deployment context into `{path}`: {error}" not in runner
    assert "test_generated_artifact_read_error_is_sanitized" in runner_test
    assert "test_deployment_context_write_error_is_sanitized" in runner_test


def test_rollout_runners_preflight_verifier_and_output_targets() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "validate_runner_preflight(args" not in read(path)
        or "sorafs_runner_preflight" not in read(path)
    ]

    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")
    helper_test = read(RUNNER_PREFLIGHT_TEST)
    assert "def _require_error_list" in helper
    assert "def _require_label" in helper
    assert "def _runner_path_sequence" in helper
    assert "def _runner_input_identity_map" in helper
    assert "runner preflight errors must be a list of strings" in helper
    assert "runner preflight errors must contain non-empty canonical strings" in helper
    assert "runner preflight label must be a non-empty canonical string" in helper
    assert "runner error message must be a non-empty canonical string" in helper
    assert "paths must be a sequence" in helper
    assert "identity map must be a dictionary" in helper
    assert "identity map entries must be path identities and " in helper
    assert "def inspect_runner_path_exists" in helper
    assert "def inspect_runner_path_is_symlink" in helper
    assert "def inspect_runner_path_is_file" in helper
    assert "def inspect_runner_path_is_dir" in helper
    assert "cannot be inspected" in helper
    assert "def _record_path_inspection_failure" in helper
    assert "path_diagnostic_label(" in helper
    assert "error_diagnostic_label(" in helper
    assert "must be a path" in helper
    assert "must exist and be a file" in helper
    assert "must be a directory when it exists" in helper
    assert "must not be a directory" in helper
    assert "must not be a symlink" in helper
    assert "must not be the same path as --out-dir" in helper
    assert "validate_runner_output_dir(out_dir, errors)" in helper
    assert 'validate_runner_output_parent(summary_out, errors, label="--summary-out")' in helper
    assert "from sorafs_path_identity import error_diagnostic_label" in helper
    assert "resolve_path_identity" in helper
    assert "resolve_path_identity" in helper
    assert 'resolve_path_identity(path, errors, label="input file")' in helper
    assert 'resolve_path_identity(path, errors, label="output path")' in helper
    assert "path.resolve()" not in helper
    assert "test_verifier_inspection_failure_fails_preflight" in helper_test
    assert "test_out_dir_inspection_failure_fails_preflight" in helper_test
    assert "test_validate_runner_output_dir_rejects_non_path_without_traceback" in helper_test
    assert (
        "test_validate_runner_output_parent_rejects_non_path_without_traceback"
        in helper_test
    )
    assert "test_runner_preflight_sanitizes_malformed_non_path_targets" in helper_test
    assert "test_runner_path_inspectors_sanitize_malformed_path_labels" in helper_test
    assert "test_runner_path_inspectors_sanitize_noncanonical_failures" in helper_test
    assert (
        "test_runner_path_inspectors_reject_malformed_error_container"
        in helper_test
    )
    assert (
        "test_runner_path_inspectors_reject_malformed_existing_error_text"
        in helper_test
    )
    assert "test_runner_path_inspectors_reject_malformed_labels" in helper_test
    assert "test_input_file_rejects_scalar_and_mapping_path_collections" in helper_test
    assert "test_missing_input_file_sanitizes_noncanonical_path" in helper_test
    assert "test_input_file_rejects_malformed_label" in helper_test
    assert "test_input_file_rejects_malformed_seen_identity_map" in helper_test
    assert (
        "test_input_directory_rejects_scalar_and_mapping_path_collections"
        in helper_test
    )
    assert "test_missing_input_directory_sanitizes_noncanonical_path" in helper_test
    assert "test_input_directory_rejects_malformed_label" in helper_test
    assert (
        "test_input_directory_rejects_malformed_seen_identity_map" in helper_test
    )
    assert (
        "test_validate_runner_output_parent_rejects_malformed_error_container"
        in helper_test
    )
    assert "test_validate_runner_output_parent_rejects_malformed_label" in helper_test
    assert (
        "test_validate_runner_output_dir_rejects_malformed_error_container"
        in helper_test
    )
    assert "test_validate_runner_output_dir_rejects_malformed_label" in helper_test
    assert "test_out_dir_symlink_fails_preflight" in helper_test
    assert "test_out_dir_parent_chain_symlink_fails_preflight" in helper_test
    assert "test_summary_out_directory_inspection_failure_fails_preflight" in helper_test
    assert "test_summary_out_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_chain_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_chain_file_fails_preflight" in helper_test
    assert "test_emit_runner_error_lines_rejects_malformed_messages" in helper_test
    assert (
        "test_emit_runner_error_lines_rejects_malformed_message_content"
        in helper_test
    )
    assert (
        "test_emit_runner_error_block_rejects_malformed_messages_before_heading"
        in helper_test
    )
    assert (
        "test_emit_runner_error_block_rejects_malformed_message_content_before_heading"
        in helper_test
    )
    assert "test_input_file_rejects_non_path_without_traceback" in helper_test
    assert "test_input_file_inspection_failure_is_reported" in helper_test
    assert "test_input_directory_rejects_non_path_without_traceback" in helper_test
    assert "test_input_directory_type_inspection_failure_is_reported" in helper_test
    assert "test_run_command_plan_reports_artifact_inspection_failure" in helper_test
    assert "test_run_command_plan_rejects_malformed_step_before_output_creation" in helper_test
    assert (
        "test_run_command_plan_rejects_malformed_command_entries_before_output_creation"
        in helper_test
    )
    assert "test_planned_artifact_symlink_fails" in helper_test
    assert "test_planned_artifact_parent_symlink_fails" in helper_test
    assert "test_planned_artifact_parent_chain_symlink_fails" in helper_test
    assert "test_planned_artifact_existing_file_fails" in helper_test
    assert "test_run_command_plan_rejects_output_dir_symlink_before_launch" in helper_test
    assert "test_run_command_plan_rejects_artifact_parent_symlink_before_create" in helper_test
    assert (
        "test_run_command_plan_rejects_artifact_parent_chain_symlink_before_create"
        in helper_test
    )
    assert "test_run_command_plan_rejects_empty_artifact_written_by_command" in helper_test
    assert "test_runner_path_resolution_uses_shared_identity_helper" in helper_test
    assert missing == []


def test_rollout_tools_use_bounded_shared_response_file_expansion() -> None:
    missing = [
        path.name
        for path in [*CHECKERS, *RUNNERS]
        if "sorafs_response_args" not in read(path)
        or "EvidenceArgumentParser" not in read(path)
        or "expand_response_args(" not in read(path)
        or "fromfile_prefix_chars" in read(path)
        or "def expand_response_args(" in read(path)
    ]
    local_response_parsers = [
        path.name
        for path in [*CHECKERS, *RUNNERS]
        if "class EvidenceArgumentParser(" in read(path)
        or "def convert_arg_line_to_args(" in read(path)
        or "import shlex" in read(path)
        or "shlex.split(" in read(path)
    ]
    helper = read(RESPONSE_ARGS_HELPER)
    helper_test = read(RESPONSE_ARGS_TEST)

    assert "class EvidenceArgumentParser" in helper
    assert "def convert_arg_line_to_args" in helper
    assert "shlex.split(line, comments=True)" in helper
    assert "MAX_RESPONSE_ARGFILE_BYTES" in helper
    assert "MAX_RESPONSE_ARGFILE_DEPTH" in helper
    assert "MAX_EXPANDED_ARGS" in helper
    assert "def require_expanded_arg_limit" in helper
    assert "require_expanded_arg_limit(expanded)" in helper
    assert "{label} must be a sequence of strings" in helper
    assert 'label="arguments"' in helper
    assert 'label="response-file line arguments"' in helper
    assert "response-file line must be a string" in helper
    assert "def _require_argument_string" in helper
    assert "argument `{value}` must be a string" in helper
    assert "argument must be a non-empty canonical string" in helper
    assert "expanded arguments must be <=" in helper
    assert "must exist and be a file" in helper
    assert "failed to resolve @ARGFILE" in helper
    assert "failed to stat @ARGFILE" in helper
    assert "failed to read @ARGFILE" in helper
    assert "failed to stat @ARGFILE `{path}`: {error}" not in helper
    assert "failed to read @ARGFILE `{path}`: {error}" not in helper
    assert "@ARGFILE `{path}` must be UTF-8: {error}" not in helper
    assert "@ARGFILE `{path}` line {line_number}: {error}" not in helper
    assert "error_diagnostic_label(error, path_label=path_label)" in helper
    assert "path_diagnostic_label(path)" in helper
    assert "RuntimeError" in helper
    assert '"@ARGFILE `{}` line {}: {}".format(' in helper
    assert "recursive @ARGFILE" in helper
    assert "must be UTF-8" in helper
    assert "test_response_file_stat_failure_is_stable_value_error" in helper_test
    assert "test_response_file_read_failure_is_stable_value_error" in helper_test
    assert "test_response_file_non_utf8_bytes_fail_stably" in helper_test
    assert "test_response_file_stat_failure_sanitizes_malformed_error" in helper_test
    assert "test_response_file_read_failure_sanitizes_malformed_error" in helper_test
    assert (
        "test_response_file_line_parse_error_sanitizes_malformed_error"
        in helper_test
    )
    assert "test_direct_non_string_argument_fails_without_traceback" in helper_test
    assert (
        "test_raw_string_argument_container_fails_without_character_expansion"
        in helper_test
    )
    assert (
        "test_raw_bytes_argument_container_fails_without_character_expansion"
        in helper_test
    )
    assert (
        "test_raw_bytearray_argument_container_fails_without_byte_expansion"
        in helper_test
    )
    assert "test_malformed_direct_argument_text_fails_closed" in helper_test
    assert "test_mapping_argument_container_fails_without_key_expansion" in helper_test
    assert (
        "test_response_file_parser_returning_scalar_line_args_fails_with_line"
        in helper_test
    )
    assert (
        "test_response_file_parser_returning_non_string_line_arg_fails_with_line"
        in helper_test
    )
    assert (
        "test_response_file_parser_returning_malformed_line_arg_fails_with_line"
        in helper_test
    )
    assert "test_convert_arg_line_to_args_rejects_non_string_line" in helper_test
    assert "test_shared_integer_arg_parsers_reject_non_string_values" in helper_test
    assert "test_direct_expanded_argument_limit_fails" in helper_test
    assert "test_response_file_expanded_argument_limit_fails" in helper_test
    assert missing == []
    assert local_response_parsers == []


def test_rollout_checkers_use_shared_integer_arg_parsers() -> None:
    missing_positive = [
        path.name
        for path in CHECKERS
        if path.name in positive_int_arg_checkers()
        and "positive_int_arg" not in read(path)
    ]
    missing_non_negative = [
        path.name
        for path in CHECKERS
        if path.name in non_negative_int_arg_checkers()
        and "non_negative_int_arg" not in read(path)
    ]
    missing_runner_positive = [
        path.name
        for path in RUNNERS
        if path.name in positive_int_arg_runners()
        and "type=positive_int_arg" not in read(path)
    ]
    missing_runner_non_negative = [
        path.name
        for path in RUNNERS
        if path.name in non_negative_int_arg_runners()
        and "type=non_negative_int_arg" not in read(path)
    ]
    local_parser_defs = [
        path.name
        for path in [*CHECKERS, *RUNNERS]
        if "def positive_int_arg(" in read(path)
        or "def non_negative_int_arg(" in read(path)
    ]
    local_parser_errors = [
        path.name
        for path in CHECKERS
        if 'ArgumentTypeError("must be positive")' in read(path)
        or 'ArgumentTypeError("must be non-negative")' in read(path)
        or "--max-snapshot-age-secs must be positive" in read(path)
        or "--max-ingest-lag-secs must be positive" in read(path)
    ]
    local_parser_predicates = [
        path.name
        for path in CHECKERS
        if "parsed <= 0" in read(path)
        or "parsed < 0" in read(path)
        or "args.max_snapshot_age_secs <= 0" in read(path)
        or "args.max_ingest_lag_secs <= 0" in read(path)
    ]
    raw_checker_int_arg_types = [
        path.name
        for path in CHECKERS
        if "type=int" in read(path)
    ]
    raw_runner_int_arg_types = [
        path.name
        for path in RUNNERS
        if "type=int" in read(path)
    ]
    helper = read(RESPONSE_ARGS_HELPER)

    assert "def parse_int_arg" in helper
    assert "def positive_int_arg" in helper
    assert "def non_negative_int_arg" in helper
    assert "CANONICAL_DECIMAL_INTEGER_RE" in helper
    assert ".fullmatch(value)" in helper
    assert 'value == "-0"' in helper
    assert 'ArgumentTypeError("must be an integer")' in helper
    assert 'ArgumentTypeError("must be positive")' in helper
    assert 'ArgumentTypeError("must be non-negative")' in helper
    assert missing_positive == []
    assert missing_non_negative == []
    assert missing_runner_positive == []
    assert missing_runner_non_negative == []
    assert local_parser_defs == []
    assert local_parser_errors == []
    assert local_parser_predicates == []
    assert raw_checker_int_arg_types == []
    assert raw_runner_int_arg_types == []


def test_rollout_runners_use_shared_namespace_integer_validators() -> None:
    runner_sources = {path: read(path) for path in RUNNERS}
    missing_positive = [
        path.name
        for path, source in runner_sources.items()
        if path.name in positive_int_arg_runners()
        and "require_runner_positive_int" not in source
    ]
    missing_non_negative = [
        path.name
        for path, source in runner_sources.items()
        if path.name in non_negative_int_arg_runners()
        and "require_runner_non_negative_int" not in source
    ]
    local_numeric_predicates = [
        path.name
        for path, source in runner_sources.items()
        if re.search(
            r"args\.[A-Za-z0-9_]+(?:\s+is\s+not\s+None\s+and\s+"
            r"args\.[A-Za-z0-9_]+)?\s*(?:<=|<)\s*0",
            source,
        )
    ]
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")
    helper_test = read(RUNNER_PREFLIGHT_TEST)

    assert "def runner_arg_label" in helper
    assert "RUNNER_ARG_FIELD_RE" in helper
    assert "def _require_runner_arg_field" in helper
    assert "runner argument field must be a snake_case string" in helper
    assert "runner preflight errors must be a list of strings" in helper
    assert "def require_runner_positive_int" in helper
    assert "def require_runner_non_negative_int" in helper
    assert "field_name = _require_runner_arg_field(field)" in helper
    assert "getattr(args, field_name, None)" in helper
    assert "not isinstance(value, bool)" in helper
    assert '" when supplied" if allow_none else ""' in helper
    assert "test_runner_arg_label_rejects_malformed_field_names" in helper_test
    assert "test_require_runner_positive_int_rejects_direct_non_int_values" in helper_test
    assert (
        "test_require_runner_positive_int_rejects_malformed_error_container"
        in helper_test
    )
    assert "test_require_runner_positive_int_rejects_malformed_field_name" in helper_test
    assert "test_require_runner_non_negative_int_rejects_direct_invalid_values" in helper_test
    assert (
        "test_require_runner_non_negative_int_rejects_malformed_error_container"
        in helper_test
    )
    assert (
        "test_require_runner_non_negative_int_rejects_malformed_field_name"
        in helper_test
    )
    assert missing_positive == []
    assert missing_non_negative == []
    assert local_numeric_predicates == []


def test_rollout_checkers_preflight_summary_output_targets() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if "validate_checker_preflight(args)" not in read(path)
        or "sorafs_checker_preflight" not in read(path)
        or "render_and_write_checker_summary," not in read(path)
        or "render_and_write_checker_summary(\n" not in read(path)
        or "summary_out.parent.mkdir" in read(path)
        or "summary_out.write_text" in read(path)
    ]
    local_summary_renderers = [
        path.name
        for path in CHECKERS
        if "json.dumps(summary" in read(path)
        or "output = json.dumps" in read(path)
        or "rendered = json.dumps" in read(path)
        or "render_checker_summary(summary)" in read(path)
        or re.search(r"(?<!render_and_)write_checker_summary\(", read(path))
        or 'output + "\\n"' in read(path)
        or 'rendered + "\\n"' in read(path)
    ]
    local_stderr_emitters = [
        path.name
        for path in CHECKERS
        if re.search(r"\n\s*print\(", read(path))
        or "for error in preflight_errors:" in read(path)
        or "for error in summary_errors:" in read(path)
        or 'print(f"- {error}"' in read(path)
        or 'print(f"ERROR: {error}"' in read(path)
    ]

    helper = read(CHECKER_PREFLIGHT)
    helper_test = read(CHECKER_PREFLIGHT_TEST)
    assert "def emit_checker_error_lines" in helper
    assert "def emit_checker_error_block" in helper
    assert "def _checker_error_messages" in helper
    assert "checker error messages must be a sequence of strings" in helper
    assert "checker error message must be a non-empty canonical string" in helper
    assert "isinstance(errors, (str, bytes, bytearray, Mapping))" in helper
    assert "error != error.strip()" in helper
    assert "error_messages = _checker_error_messages(errors)" in helper
    assert "def emit_checker_notice" in helper
    assert "checker notice message must be a non-empty canonical string" in helper
    assert "def _require_error_list" in helper
    assert "def _require_label" in helper
    assert "checker preflight errors must be a list of strings" in helper
    assert "checker preflight errors must contain non-empty canonical strings" in helper
    assert "checker preflight label must be a non-empty canonical string" in helper
    assert "def inspect_checker_preflight_path_exists" in helper
    assert "def inspect_checker_preflight_path_is_dir" in helper
    assert "def inspect_checker_preflight_path_is_symlink" in helper
    assert "def validate_checker_output_parent" in helper
    assert "def validate_checker_summary_output" in helper
    assert "cannot be inspected" in helper
    assert "path_diagnostic_label(" in helper
    assert "error_diagnostic_label(" in helper
    assert "<non-canonical-path>" in read(PATH_IDENTITY_HELPER)
    assert "<non-canonical-error>" in read(PATH_IDENTITY_HELPER)
    assert "parent chain" in helper
    assert "must be a path" in helper
    assert "must not be a symlink" in helper
    assert "def render_checker_summary" in helper
    assert "def _validate_checker_summary_keys" in helper
    assert "checker summary keys must be non-empty canonical strings" in helper
    assert "def render_and_write_checker_summary" in helper
    assert "checker summary must be an object" in helper
    assert "_validate_checker_summary_keys(summary)" in helper
    assert "rendered_summary = render_checker_summary(summary)" in helper
    assert "write_checker_summary(summary_out, rendered_summary)" in helper
    assert "sys.stdout.write(rendered_summary)" in helper
    assert 'json.dumps(summary, indent=2, sort_keys=True, allow_nan=False) + "\\n"' in helper
    assert "except (TypeError, ValueError) as error" in helper
    assert "failed to render checker summary JSON" in helper
    assert "failed to render checker summary JSON: {error}" not in helper
    assert "failed to render checker summary JSON: {error_diagnostic_label(error)}" in helper
    assert "def write_checker_summary" in helper
    assert "checker summary text must be a string" in helper
    assert "failed to create --summary-out parent" in helper
    assert "failed to write --summary-out" in helper
    assert "must not be a directory" in helper
    assert "must be a directory when it exists" in helper
    assert "must not be the same path as --evidence" in helper
    assert "record_reserved_output_evidence_conflicts" in helper
    assert 'reserved_label="--summary-out"' in helper
    assert "from sorafs_path_identity import error_diagnostic_label" in helper
    assert (
        "test_render_and_write_checker_summary_sanitizes_malformed_render_error"
        in helper_test
    )
    assert "from sorafs_path_identity import resolve_path_identity" in helper
    assert "resolve_path_identity" in helper
    assert "resolve_path_identity(path, errors, label=label)" in helper
    assert "path.resolve()" not in helper
    assert "validate_checker_summary_output(summary_out, errors)" in helper
    assert "def validate_checker_summary_output(summary_out: Path" in helper
    assert "test_summary_out_exists_inspection_failure_fails_preflight" in helper_test
    assert "test_summary_out_directory_inspection_failure_fails_preflight" in helper_test
    assert "test_summary_out_parent_inspection_failure_fails_preflight" in helper_test
    assert "test_summary_out_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_chain_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_chain_file_fails_preflight" in helper_test
    assert (
        "test_validate_checker_summary_output_rejects_non_path_without_traceback"
        in helper_test
    )
    assert (
        "test_validate_checker_output_parent_rejects_non_path_without_traceback"
        in helper_test
    )
    assert (
        "test_checker_preflight_path_inspectors_sanitize_malformed_path_labels"
        in helper_test
    )
    assert (
        "test_checker_preflight_path_inspectors_sanitize_noncanonical_failures"
        in helper_test
    )
    assert (
        "test_checker_preflight_path_inspectors_reject_malformed_error_container"
        in helper_test
    )
    assert (
        "test_checker_preflight_path_inspectors_reject_malformed_existing_error_text"
        in helper_test
    )
    assert "test_checker_preflight_path_inspectors_reject_malformed_labels" in helper_test
    assert (
        "test_validate_checker_output_parent_rejects_malformed_error_container"
        in helper_test
    )
    assert "test_validate_checker_output_parent_rejects_malformed_label" in helper_test
    assert (
        "test_validate_checker_summary_output_rejects_malformed_error_container"
        in helper_test
    )
    assert "test_render_checker_summary_rejects_non_object_summary" in helper_test
    assert (
        "test_render_and_write_checker_summary_reports_non_object_summary"
        in helper_test
    )
    assert "test_render_checker_summary_rejects_malformed_summary_keys" in helper_test
    assert (
        "test_render_and_write_checker_summary_reports_malformed_summary_keys"
        in helper_test
    )
    assert "test_write_checker_summary_rejects_non_string_text" in helper_test
    assert "test_write_checker_summary_rejects_summary_symlink" in helper_test
    assert (
        "test_write_checker_summary_rejects_parent_chain_symlink_before_create"
        in helper_test
    )
    assert (
        "test_summary_out_same_as_explicit_evidence_sanitizes_noncanonical_paths"
        in helper_test
    )
    assert "test_write_checker_summary_sanitizes_create_parent_failure" in helper_test
    assert "test_write_checker_summary_sanitizes_write_failure" in helper_test
    assert "test_emit_checker_error_lines_rejects_malformed_messages" in helper_test
    assert (
        "test_emit_checker_error_block_rejects_malformed_messages_before_heading"
        in helper_test
    )
    assert "test_emit_checker_notice_rejects_malformed_message" in helper_test
    assert "test_checker_path_resolution_uses_shared_identity_helper" in helper_test
    assert missing == []
    assert local_summary_renderers == []
    assert local_stderr_emitters == []
    assert [path.name for path in CHECKERS if "print(rendered_summary" in read(path)] == []


def test_rollout_checkers_use_shared_caught_argument_error_reporting() -> None:
    collected_raw_errors = [
        path.name for path in CHECKERS if "errors.append(str(error))" in read(path)
    ]
    local_parser_error_handlers = [
        path.name
        for path in CHECKERS
        if re.search(
            r"except ValueError as error:\n\s*parser\.error\(str\(error\)\)",
            read(path),
        )
    ]
    missing_shared_returns = [
        path.name
        for path in CHECKERS
        if "except ValueError as error:" in read(path)
        and "emit_checker_exception(error)\n        return 2" not in read(path)
    ]
    helper = read(CHECKER_PREFLIGHT)
    helper_test = read(CHECKER_PREFLIGHT_TEST)

    assert "def emit_checker_error_lines" in helper
    assert "def emit_checker_exception" in helper
    assert "test_emit_checker_exception_sanitizes_malformed_message" in helper_test
    assert "test_emit_checker_exception_preserves_canonical_message" in helper_test
    assert "emit_checker_error_lines((str(error),))" not in "\n".join(
        read(path) for path in CHECKERS
    )
    reputation_checker = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")
    assert collected_raw_errors == []
    assert "from sorafs_path_identity import error_diagnostic_label" in reputation_checker
    assert "errors.append(error_diagnostic_label(error))" in reputation_checker
    assert local_parser_error_handlers == []
    assert missing_shared_returns == []


def test_rollout_checker_mains_return_argparse_error_codes() -> None:
    missing = []
    for path in CHECKERS:
        main_source = function_source(path, "main")
        if "parser.parse_args(" not in main_source:
            continue
        if (
            "except SystemExit as error" not in main_source
            or "return error.code if isinstance(error.code, int) else 1" not in main_source
        ):
            missing.append(path.name)

    assert missing == []


def test_rollout_checkers_use_shared_evidence_input_preflight() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if "validate_checker_preflight(args)" not in read(path)
    ]
    local_evidence_input_checks = [
        path.name
        for path in CHECKERS
        if "provide --evidence-dir or --evidence" in read(path)
        or "at least one --evidence-dir or --evidence is required" in read(path)
        or "not args.evidence_dir and not args.evidence" in read(path)
    ]

    helper = read(CHECKER_PREFLIGHT)
    helper_test = read(CHECKER_PREFLIGHT_TEST)
    assert "def validate_checker_evidence_inputs" in helper
    assert "def _checker_path_sequence" in helper
    assert 'label="--evidence-dir"' in helper
    assert 'label="--evidence"' in helper
    assert "paths must be a sequence" in helper
    assert "path_diagnostic_label(evidence_dir)" in helper
    assert "path_diagnostic_label(evidence_file)" in helper
    assert "must be a path or evidence spec" in helper
    assert "errors = validate_checker_evidence_inputs(args)" in helper
    assert "if errors:\n        return errors" in helper
    assert 'return ["provide --evidence-dir or --evidence"]' in helper
    assert "test_present_evidence_spec_passes_input_check" in helper_test
    assert "test_evidence_input_check_rejects_malformed_collections" in helper_test
    assert "test_evidence_input_check_rejects_non_path_entries" in helper_test
    assert "test_evidence_input_check_sanitizes_malformed_entry_labels" in helper_test
    assert "test_checker_preflight_reports_malformed_evidence_inputs" in helper_test
    assert "test_checker_preflight_stops_after_malformed_evidence_inputs" in helper_test
    assert missing == []
    assert local_evidence_input_checks == []


def test_rollout_checkers_use_shared_evidence_file_discovery() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if "sorafs_evidence_paths" not in read(path)
        or "discover_evidence_files(" not in read(path)
        or "reserved_output_paths=" not in read(path)
        or "args.summary_out" not in read(path)
        or "def discover_files(" in read(path)
        or "evidence directory `{directory}` must exist" in read(path)
        or "if not root.is_dir()" in read(path)
        or "seen: set[Path]" in read(path)
        or '.rglob("*.json")' in read(path)
        or "path.resolve()" in read(path)
    ]
    helper = read(EVIDENCE_PATHS_HELPER)
    helper_test = read(EVIDENCE_PATHS_TEST)
    path_identity_helper = read(PATH_IDENTITY_HELPER)

    assert "duplicate explicit evidence file" in helper
    assert "def _require_error_list" in helper
    assert "def _require_label" in helper
    assert "def _path_label" in helper
    assert "def _error_label" in helper
    assert "from sorafs_path_identity import" in helper
    assert "path_diagnostic_label(" in helper
    assert "error_diagnostic_label(" in helper
    assert "def _canonical_diagnostic_text" not in helper
    assert "<non-path>" in path_identity_helper
    assert "<non-canonical-path>" in path_identity_helper
    assert "<non-canonical-error>" in path_identity_helper
    assert "evidence path errors must be a list of strings" in helper
    assert "evidence path errors must contain non-empty canonical strings" in helper
    assert "evidence path label must be a non-empty canonical string" in helper
    assert "def evidence_path_collection" in helper
    assert "paths must be a sequence" in helper
    assert "isinstance(paths, (str, bytes, bytearray, Mapping))" in helper
    assert "both --evidence and --evidence-dir" in helper
    assert "duplicate evidence file" in helper
    assert "must exist and be a directory" in helper
    assert "evidence directory `{_path_label(directory)}` must be a path" in helper
    assert "def inspect_evidence_directory" in helper
    assert "def scan_evidence_directory_json" in helper
    assert "cannot be inspected" in helper
    assert "failed to scan evidence directory" in helper
    assert "reserved_output_paths" in helper
    assert "reserved_output_path_identities" in helper
    assert "reserved_error_count = len(error_list)" in helper
    assert "record_reserved_output_evidence_conflicts" in helper
    assert "reserved output" in helper
    assert "conflicts with reserved output" in helper
    assert "resolve_path_identity" in helper
    assert "resolve_path_identity(path, errors, label=label)" in helper
    assert "RuntimeError" in path_identity_helper
    assert "evidence_path_identities" in helper
    assert "is_explicit_evidence_path" in helper
    assert "def _evidence_path_identity_set" in helper
    assert "test_noncanonical_evidence_directory_labels_are_sanitized" in helper_test
    assert (
        "test_noncanonical_missing_evidence_directory_label_is_sanitized"
        in helper_test
    )
    assert (
        "test_evidence_directory_inspection_failure_sanitizes_noncanonical_path"
        in helper_test
    )
    assert "identities must be a set of paths" in helper
    assert (
        "test_evidence_path_collection_rejects_scalar_and_mapping_containers"
        in helper_test
    )
    assert (
        "test_discover_evidence_files_rejects_malformed_path_collections"
        in helper_test
    )
    assert (
        "test_evidence_path_collection_rejects_malformed_error_container"
        in helper_test
    )
    assert (
        "test_evidence_path_collection_rejects_malformed_existing_error_text"
        in helper_test
    )
    assert "test_evidence_path_collection_rejects_malformed_labels" in helper_test
    assert (
        "test_discover_evidence_files_rejects_malformed_error_container"
        in helper_test
    )
    assert (
        "test_evidence_directory_helpers_reject_malformed_error_container"
        in helper_test
    )
    assert (
        "test_reserved_output_helpers_reject_malformed_error_container"
        in helper_test
    )
    assert "test_reserved_output_helpers_reject_malformed_labels" in helper_test
    assert (
        "test_reserved_output_helpers_reject_malformed_path_collections"
        in helper_test
    )
    assert (
        "test_discover_evidence_files_stops_after_malformed_reserved_outputs"
        in helper_test
    )
    assert (
        "test_discover_evidence_files_stops_after_non_path_reserved_outputs"
        in helper_test
    )
    assert (
        "test_evidence_path_identities_rejects_malformed_path_collections"
        in helper_test
    )
    assert "test_non_path_evidence_directory_fails_closed_without_traceback" in helper_test
    assert "test_evidence_directory_inspection_failure_fails_closed" in helper_test
    assert "test_evidence_directory_scan_failure_fails_closed" in helper_test
    assert (
        "test_reserved_output_conflict_non_path_directory_fails_closed" in helper_test
    )
    assert (
        "test_reserved_output_conflict_scan_inspection_failure_fails_closed"
        in helper_test
    )
    assert "test_reserved_output_conflict_scan_failure_fails_closed" in helper_test
    assert "test_evidence_path_resolution_uses_shared_identity_helper" in helper_test
    assert (
        "test_explicit_identity_helper_rejects_malformed_identity_container"
        in helper_test
    )
    assert missing == []


def test_rollout_checkers_use_shared_bounded_json_loader() -> None:
    missing = [
        path.name
        for path in bounded_json_checkers()
        if "sorafs_evidence_json" not in read(path)
        or "load_evidence_json_with_sha256_or_record_error(\n" not in read(path)
        or (
            '"sha256": digest' not in read(path)
            and "build_evidence_artifact(" not in read(path)
            and "build_kinded_evidence_artifact(" not in read(path)
        )
        or "def load_json(" in read(path)
        or "json.load(" in read(path)
        or "def sha256_hex(" in read(path)
        or "sha256_hex(path)" in read(path)
    ]
    local_load_error_recorders = [
        path.name
        for path in standard_json_error_checkers()
        if "failed to load evidence JSON" in read(path)
        or "json.JSONDecodeError" in read(path)
    ]
    helper = read(EVIDENCE_JSON_HELPER)
    helper_test = read(EVIDENCE_JSON_TEST)
    path_identity_helper = read(PATH_IDENTITY_HELPER)
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def load_evidence_json" in helper
    assert "class EvidenceFileTooLargeError" in helper
    assert "def load_evidence_json_with_sha256" in helper
    assert "def load_evidence_json_with_sha256_or_record_error" in helper
    assert "evidence JSON errors must be a list of strings" in helper
    assert "evidence JSON errors must contain non-empty canonical strings" in helper
    assert "failed to load evidence JSON" in helper
    assert "from sorafs_path_identity import" in helper
    assert "path_diagnostic_label(" in helper
    assert "error_diagnostic_label(" in helper
    assert "def _canonical_diagnostic_text" not in helper
    assert "def _evidence_path_label" in helper
    assert "def _error_label" in helper
    assert "evidence path must be a path" in helper
    assert "<non-path>" in path_identity_helper
    assert "<non-canonical-path>" in path_identity_helper
    assert "<non-canonical-error>" in path_identity_helper
    assert "evidence byte limit must be positive" in helper
    assert "evidence file exceeds" in helper
    assert "raise EvidenceFileTooLargeError(max_bytes)" in helper
    assert "evidence JSON bytes must be bytes" in helper
    assert "evidence root must be a JSON object" in helper
    assert "def _json_key_label" in helper
    assert "def json_object_without_duplicate_keys" in helper
    assert "object_pairs_hook=json_object_without_duplicate_keys" in helper
    assert "evidence JSON object contains duplicate key" in helper
    assert "`<non-canonical>`" in helper
    assert "hashlib.sha256(raw).hexdigest()" in helper
    assert "parse_constant=reject_non_standard_json_constant" in helper
    assert "non-standard JSON constant" in helper
    assert "RuntimeError" in helper
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_records_runtime_failure"
        in helper_test
    )
    assert "test_read_evidence_bytes_raises_typed_oversize_error" in helper_test
    assert "test_load_evidence_json_rejects_non_path_without_traceback" in helper_test
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_records_non_path"
        in helper_test
    )
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_sanitizes_malformed_path_label"
        in helper_test
    )
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_sanitizes_malformed_os_error"
        in helper_test
    )
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_rejects_malformed_errors"
        in helper_test
    )
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_rejects_malformed_existing_error_text"
        in helper_test
    )
    assert (
        "test_load_evidence_json_rejects_invalid_byte_limit_without_traceback"
        in helper_test
    )
    assert (
        "test_load_evidence_json_rejects_non_standard_numeric_constants"
        in helper_test
    )
    assert (
        "test_decode_evidence_json_rejects_non_byte_input_without_traceback"
        in helper_test
    )
    assert "test_load_evidence_json_rejects_duplicate_top_level_keys" in helper_test
    assert "test_load_evidence_json_rejects_duplicate_nested_keys" in helper_test
    assert (
        "test_decode_evidence_json_sanitizes_malformed_duplicate_key_label"
        in helper_test
    )
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_records_duplicate_key"
        in helper_test
    )
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_sanitizes_malformed_duplicate_key"
        in helper_test
    )
    assert "read_json_artifact" not in reputation
    assert "load_evidence_json_with_sha256_or_record_error(" in reputation
    assert "load_evidence_json_with_sha256(path, MAX_EVIDENCE_BYTES)" not in reputation
    assert "json.JSONDecodeError" not in reputation
    assert "evidence file `{path}` must exist" not in reputation
    assert "if not path.is_file()" not in reputation
    assert "path.read_bytes()" not in reputation
    assert "hashlib.sha256(data).hexdigest()" not in reputation
    assert missing == []
    assert local_load_error_recorders == []


def test_rollout_checkers_use_shared_artifact_rows_and_fingerprints() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "build_evidence_artifact," not in read(path)
        or "FINGERPRINT_FIELDS: tuple[str, ...]" not in read(path)
        or "build_evidence_artifact(" not in read(path)
        or "artifact_fingerprint(payload, FINGERPRINT_FIELDS)" in read(path)
        or '"path": str(path)' in read(path)
        or '"sha256": digest' in read(path)
        or '"schema": payload.get("schema")' in read(path)
        or '"status": payload.get("status")' in read(path)
        or '"valid": not validation_errors' in read(path)
        or '"errors": validation_errors' in read(path)
    ]
    helper = read(EVIDENCE_FINGERPRINT_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_fingerprint_test.py")
    validation_helper = read(EVIDENCE_VALIDATION_HELPER)
    pop_checker = read(SCRIPTS_DIR / "check_sorafs_pop_credentials_rollout_evidence.py")
    reputation_checker = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def artifact_fingerprint" in helper
    assert "artifact fingerprint payload must be an object" in helper
    assert "isinstance(fields, (str, bytes, bytearray))" in helper
    assert "artifact fingerprint fields must be a sequence of strings" in helper
    assert "artifact fingerprint fields must be non-empty strings" in helper
    assert "artifact fingerprint fields must be canonical strings" in helper
    assert (
        "artifact fingerprint fields must not contain control characters"
        in helper
    )
    assert "artifact fingerprint fields must not contain duplicates" in helper
    assert "for field in fields:" in helper
    assert "ord(character) < 32" in helper
    assert "seen_fields: set[str] = set()" in helper
    assert "fingerprint[field] = payload.get(field)" in helper
    assert (
        "test_artifact_fingerprint_rejects_non_object_payload_without_traceback"
        in helper_test
    )
    assert (
        "test_artifact_fingerprint_rejects_string_field_sequence_without_splitting"
        in helper_test
    )
    assert (
        "test_artifact_fingerprint_rejects_bytearray_fields_without_byte_iteration"
        in helper_test
    )
    assert (
        "test_artifact_fingerprint_rejects_non_string_field_without_traceback"
        in helper_test
    )
    assert "test_artifact_fingerprint_rejects_blank_field_without_traceback" in helper_test
    assert (
        "test_artifact_fingerprint_rejects_padded_field_without_drift"
        in helper_test
    )
    assert (
        "test_artifact_fingerprint_rejects_control_character_field_without_drift"
        in helper_test
    )
    assert (
        "test_artifact_fingerprint_rejects_duplicate_fields_without_overwrite"
        in helper_test
    )
    assert "def _artifact_validation_error_list" in validation_helper
    assert "SHA256_HEX_PATTERN = re.compile" in validation_helper
    assert "validation_errors: Any" in validation_helper
    assert (
        "not isinstance(validation_errors, (str, bytes, bytearray))"
        in validation_helper
    )
    assert (
        "artifact validation errors must be a sequence of strings"
        in validation_helper
    )
    assert "has_canonical_errors" in validation_helper
    assert (
        "artifact validation errors must be non-empty canonical strings"
        in validation_helper
    )
    assert "def _artifact_summary_field_or_error" in validation_helper
    assert "not isinstance(payload, Mapping)" in validation_helper
    assert "artifact summary field" in validation_helper
    assert "record_summary_field_errors = not errors" in validation_helper
    assert "def _artifact_sha256_or_error" in validation_helper
    assert "SHA256_HEX_PATTERN.fullmatch(digest)" in validation_helper
    assert (
        "artifact sha256 must be a 64-character lowercase hex string"
        in validation_helper
    )
    assert "def _artifact_path_or_error" in validation_helper
    assert "artifact path must be a path" in validation_helper
    assert "artifact path must be a canonical path" in validation_helper
    assert "def _artifact_kind_name_or_error" in validation_helper
    assert "kind_name.strip()" in validation_helper
    assert "artifact kind must be a non-empty string" in validation_helper
    assert "label_name=\"artifact kind\"" in validation_helper
    assert "must be a non-empty canonical string" in validation_helper
    assert "def _artifact_fingerprint_or_error" in validation_helper
    assert "except ValueError as exc" in validation_helper
    assert "def _merge_artifact_fingerprint_values" in validation_helper
    assert "artifact fingerprint values must be a mapping" in validation_helper
    assert "values_to_merge" in validation_helper
    assert "label_name=\"artifact fingerprint value key\"" in validation_helper
    assert (
        "artifact fingerprint value keys must be non-empty strings"
        in validation_helper
    )
    assert "label_errors[0]" in validation_helper
    assert "def build_evidence_artifact" in validation_helper
    assert "def build_kinded_evidence_artifact" in validation_helper
    assert "payload: Any" in validation_helper
    assert "kind_name: Any" in validation_helper
    assert "digest: Any" in validation_helper
    assert "fingerprint_fields: Any" in validation_helper
    assert "artifact_fingerprint(payload, fingerprint_fields)" in validation_helper
    assert "path_label = _artifact_path_or_error(path, errors)" in validation_helper
    assert '"path": path_label' in validation_helper
    assert '"kind": kind' in validation_helper
    assert '"sha256": sha256' in validation_helper
    assert '"schema": schema' in validation_helper
    assert '"status": status' in validation_helper
    assert (
        "fingerprint = _artifact_fingerprint_or_error(payload, fingerprint_fields, errors)"
        in validation_helper
    )
    assert "sha256 = _artifact_sha256_or_error(digest, errors)" in validation_helper
    assert "kind = _artifact_kind_name_or_error(kind_name, errors)" in validation_helper
    assert "_merge_artifact_fingerprint_values(" in validation_helper
    assert "errors = _artifact_validation_error_list(validation_errors)" in validation_helper
    assert '"valid": not errors' in validation_helper
    assert '"errors": errors' in validation_helper
    assert (
        "test_build_evidence_artifact_rejects_malformed_error_buckets"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_evidence_artifact_rejects_malformed_error_messages"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_evidence_artifact_rejects_malformed_summary_fields"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_evidence_artifact_sanitizes_malformed_summary_fields_after_validation_errors"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_kinded_evidence_artifact_rejects_malformed_error_buckets"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_kinded_evidence_artifact_rejects_malformed_error_messages"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_evidence_artifact_rejects_malformed_payload_or_fingerprint_fields"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_kinded_evidence_artifact_rejects_malformed_fingerprint_inputs"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_evidence_artifact_rejects_malformed_sha256"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_evidence_artifact_rejects_malformed_paths"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_kinded_evidence_artifact_rejects_malformed_kind_or_sha256"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_kinded_evidence_artifact_rejects_malformed_paths"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert "build_kinded_evidence_artifact(" in reputation_checker
    assert "artifact_fingerprint(payload, FINGERPRINT_FIELDS)" not in reputation_checker
    assert "synced_revocation_list_digest_hex" in pop_checker
    assert missing == []


def test_rollout_checkers_use_shared_recognized_artifact_count() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "count_evidence_artifacts," not in read(path)
        or (
            '"recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind)'
            not in read(path)
        )
        or "len(artifacts) for artifacts in artifacts_by_kind.values()" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    reputation_checker = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def _evidence_sequence_count" in helper
    assert "def _evidence_artifact_row_count" in helper
    assert "not isinstance(" in helper
    assert "value, (str, bytes, bytearray)" in helper
    assert "all(isinstance(artifact, Mapping) for artifact in value)" in helper
    assert "def count_evidence_artifacts" in helper
    assert "def count_recognized_evidence_artifacts" in helper
    assert "_evidence_artifact_rows(artifacts)" in helper
    assert "artifact_rows is None" in helper
    assert 'label_name="recognized evidence kind"' in helper
    assert "return _evidence_artifact_row_count(recognized)" in helper
    assert (
        "test_count_evidence_artifacts_rejects_malformed_buckets_without_traceback"
        in helper_test
    )
    assert (
        'count_evidence_artifacts({"bad\\nkind": [{"valid": True}]}) == 0'
        in helper_test
    )
    assert (
        "test_count_recognized_evidence_artifacts_rejects_strings_without_traceback"
        in helper_test
    )
    assert "count_recognized_evidence_artifacts," in reputation_checker
    assert (
        '"recognized_artifact_count": count_recognized_evidence_artifacts(recognized)'
        in reputation_checker
    )
    assert '"recognized_artifact_count": len(recognized)' not in reputation_checker
    assert missing == []


def test_rollout_checkers_use_shared_evidence_file_count() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "count_evidence_files," not in read(path)
        or '"evidence_file_count": count_evidence_files(files)' not in read(path)
        or '"evidence_file_count": len(files)' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def count_evidence_files" in helper
    assert "return _evidence_sequence_count(files)" in helper
    assert "not all(isinstance(item, Path) for item in value)" in helper
    assert "test_count_evidence_files_rejects_strings_without_traceback" in helper_test
    assert 'count_evidence_files([Path("one.json"), "two.json"]) == 0' in helper_test
    assert missing == []


def test_rollout_checkers_use_shared_evidence_gate_status() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "evidence_gate_status," not in read(path)
        or '"status": evidence_gate_status(errors)' not in read(path)
        or '"status": "ready" if not errors else "blocked"' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def evidence_gate_status" in helper
    assert "errors: Any" in helper
    assert "not isinstance(errors, list)" in helper
    assert "error == error.strip()" in helper
    assert "ord(character) < 32 or ord(character) == 127" in helper
    assert 'return "blocked" if errors else "ready"' in helper
    assert "test_evidence_gate_status_fails_closed_on_malformed_errors" in helper_test
    assert missing == []


def test_rollout_checkers_use_shared_validation_error_recorder() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "record_evidence_validation_errors," not in read(path)
        or (
            "record_evidence_validation_errors(path, validation_errors, errors)"
            not in read(path)
        )
        or 'errors.extend(f"{path}: {error}" for error in validation_errors)' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def _require_error_list" in helper
    assert "evidence validation summary errors must be a list of strings" in helper
    assert (
        "evidence validation summary errors must contain non-empty canonical strings"
        in helper
    )
    assert "def _evidence_validation_path_label" in helper
    assert "evidence validation path must be a path" in helper
    assert "evidence validation path must be a canonical path" in helper
    assert "def _validation_error_messages" in helper
    assert "def record_evidence_validation_errors" in helper
    assert "validation errors must be a sequence of strings" in helper
    assert "validation errors must be non-empty canonical strings" in helper
    assert "isinstance(validation_errors, (str, bytes, bytearray))" in helper
    assert "all(isinstance(error, str) for error in validation_errors)" in helper
    assert 'summary_errors.extend(f"{path_label}: {error}" for error in messages)' in helper
    assert (
        "test_record_evidence_validation_errors_rejects_string_without_character_split"
        in helper_test
    )
    assert (
        "test_record_evidence_validation_errors_rejects_scalar_without_traceback"
        in helper_test
    )
    assert "test_record_evidence_validation_errors_rejects_non_string_entry" in helper_test
    assert (
        "test_record_evidence_validation_errors_rejects_non_path_before_messages"
        in helper_test
    )
    assert (
        "test_record_evidence_validation_errors_rejects_malformed_path_label"
        in helper_test
    )
    assert (
        "test_record_evidence_validation_errors_rejects_malformed_messages"
        in helper_test
    )
    assert (
        "test_record_evidence_validation_errors_rejects_malformed_error_container"
        in helper_test
    )
    assert (
        "test_record_evidence_validation_errors_rejects_malformed_existing_summary_error_text"
        in helper_test
    )
    assert missing == []


def test_rollout_checkers_use_shared_explicit_validation_error_recorder() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "record_explicit_evidence_validation_errors," not in read(path)
        or "record_explicit_evidence_validation_errors(" not in read(path)
        or "is_explicit_evidence_path(" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def record_explicit_evidence_validation_errors" in helper
    assert "is_explicit_evidence_path(path, explicit_identities, errors)" in helper
    assert "record_evidence_validation_errors(path, validation_errors, errors)" in helper
    assert (
        "test_record_explicit_evidence_validation_errors_rejects_malformed_identities"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert missing == []


def test_rollout_checkers_use_shared_standard_payload_validator() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "validate_standard_evidence_payload," not in read(path)
        or (
            "return validate_standard_evidence_payload("
            not in function_source(path, "validate_evidence_payload")
        )
    ]
    local_wrappers = [
        path.name
        for path in standard_artifact_checkers()
        if "require_known_schema(" in function_source(path, "validate_evidence_payload")
        or "visit_sensitive_fields(" in function_source(path, "validate_evidence_payload")
        or "from sorafs_evidence_sensitivity import visit_sensitive_fields" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def require_string" in helper
    assert "not isinstance(payload, Mapping)" in helper
    assert "payload must be an object" in helper
    assert "def require_known_schema" in helper
    assert "schema_to_kind: Any" in helper
    assert "schema registry must be a mapping" in helper
    assert "def validate_standard_evidence_payload" in helper
    assert "payload: Any" in helper
    assert "require_known_schema(payload, schema_to_kind, artifact_label, errors)" in helper
    assert "if not isinstance(payload, dict)" in helper
    assert "kind_name = getattr(kind, \"name\", None)" in helper
    assert "schema kind must have a non-empty name" in helper
    assert "require_rollout_deployment_id(payload, errors)" in helper
    assert "require_rollout_environment(payload, errors)" in helper
    assert "def require_rollout_deployment_context_review" in helper
    assert "require_rollout_deployment_context_review(payload, errors)" in helper
    assert "deployment_context_reviewed" in helper
    assert "require_reviewed_deployment_context must be a boolean" in helper
    assert "visit_sensitive_fields(" in helper
    assert "validate_kind(kind, payload, errors)" in helper
    assert "return kind_name, errors" in helper
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    assert "test_require_string_helpers_reject_malformed_payloads" in helper_test
    assert "test_require_known_schema_rejects_malformed_schema_registry" in helper_test
    assert "test_validate_standard_evidence_payload_rejects_malformed_payload" in helper_test
    assert (
        "test_validate_standard_evidence_payload_requires_reviewed_deployment_context_marker"
        in helper_test
    )
    assert (
        "test_validate_standard_evidence_payload_rejects_malformed_context_option"
        in helper_test
    )
    assert (
        "test_validate_standard_evidence_payload_rejects_malformed_kind_name"
        in helper_test
    )
    assert missing == []
    assert local_wrappers == []


def test_rollout_checkers_use_shared_string_coverage_validation() -> None:
    missing = [
        path.name
        for path in string_coverage_checkers()
        if "sorafs_evidence_validation" not in read(path)
        or "require_string_coverage(" not in read(path)
        or "def collect_string_values(" in read(path)
        or "def require_string_coverage(" in read(path)
    ]
    local_route_coverage_errors = [
        path.name for path in CHECKERS if "routes must include `{required}`" in read(path)
    ]
    local_route_coverage_predicates = [
        path.name
        for path in CHECKERS
        if "for required in required_routes:" in read(path)
        and "required not in names" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    transparency = read(SCRIPTS_DIR / "check_sorafs_transparency_rollout_evidence.py")
    ai_prescreen = read(SCRIPTS_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py")

    assert "def collect_string_values" in helper
    assert "def _collect_canonical_string_values" in helper
    assert "def _required_string_values" in helper
    assert "def require_string_coverage" in helper
    assert 'label_name=f"validation {value_label}"' in helper
    assert "for index, item in enumerate(items)" in helper
    assert 'must be an object with `{field}`' in helper
    assert 'must be a string"' in helper
    assert "required values must be a sequence of strings" in helper
    assert "allow_scalar_items" in helper
    assert "trim_values" in helper
    assert "allow_scalar_items must be a boolean" in helper
    assert "trim_values must be a boolean" in helper
    assert "test_string_coverage_helpers_reject_malformed_payloads" in helper_test
    assert (
        "test_require_string_coverage_rejects_noncanonical_present_values"
        in helper_test
    )
    assert (
        "test_require_string_coverage_rejects_malformed_present_rows"
        in helper_test
    )
    assert (
        "test_require_string_coverage_rejects_malformed_scalar_rows"
        in helper_test
    )
    assert (
        "test_require_string_coverage_rejects_malformed_labels_before_scan"
        in helper_test
    )
    assert (
        "test_require_string_coverage_rejects_malformed_boolean_options"
        in helper_test
    )
    assert 'field_name or "value"' in helper
    assert "allow_scalar_items=False" in transparency
    assert "trim_values=False" in transparency
    assert 'require_string_coverage(payload, "routes", "name", required_routes, errors)' in ai_prescreen
    assert missing == []
    assert local_route_coverage_errors == []
    assert local_route_coverage_predicates == []


def test_rollout_checkers_use_shared_basic_validation_primitives() -> None:
    missing = [
        path.name
        for path in basic_validation_checkers()
        if "sorafs_evidence_validation" not in read(path)
        or "require_object," not in read(path)
        or "require_positive_int," not in read(path)
        or "require_string," not in read(path)
        or "def require_object(" in read(path)
        or "def require_positive_int(" in read(path)
        or "def require_string(" in read(path)
    ]
    local_object_errors = [
        path.name
        for path in CHECKERS
        if 'errors.append("execution_summary must be an object")' in read(path)
    ]
    local_object_predicates = [
        path.name
        for path in CHECKERS
        if "not isinstance(summary, dict)" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def _require_validation_label" in helper
    assert "label_name=\"validation field\"" in helper
    assert "label_name=\"validation path\"" in helper
    assert "must be a non-empty canonical string" in helper
    assert "def require_object" in helper
    assert "def require_string" in helper
    assert "def require_positive_int" in helper
    assert "must be an object" in helper
    assert "label_name=field_name" in helper
    assert "must be a non-empty string" in helper
    assert "must be a positive integer" in helper
    assert "test_require_string_returns_canonical_non_empty_values" in helper_test
    assert "test_require_string_reports_malformed_or_non_string_values" in helper_test
    assert "test_require_string_type_returns_canonical_string_values" in helper_test
    assert "test_require_string_type_rejects_malformed_string_values" in helper_test
    assert (
        "test_require_positive_int_rejects_malformed_label_before_lookup"
        in helper_test
    )
    assert "test_require_object_rejects_malformed_path_labels" in helper_test
    assert (
        "test_require_string_helpers_reject_malformed_labels_before_lookup"
        in helper_test
    )
    assert missing == []
    assert local_object_errors == []
    assert local_object_predicates == []


def test_rollout_checkers_use_shared_extended_validation_primitives() -> None:
    expected_by_helper = {
        "require_string_equal": checker_names_without(
            "check_sorafs_transparency_rollout_evidence.py",
        ),
        "require_bool_true": checker_names_without(),
        "require_non_negative_int": {
            "check_sorafs_ai_prescreen_rollout_evidence.py",
            "check_sorafs_appeal_finance_rollout_evidence.py",
            "check_sorafs_hedging_rollout_evidence.py",
            "check_sorafs_moderation_panel_rollout_evidence.py",
            "check_sorafs_orderbook_rollout_evidence.py",
            "check_sorafs_pop_credentials_rollout_evidence.py",
            "check_sorafs_repair_rollout_evidence.py",
            "check_sorafs_reserve_rent_rollout_evidence.py",
        },
        "require_count_equal": {
            "check_sorafs_ai_prescreen_rollout_evidence.py",
            "check_sorafs_appeal_finance_rollout_evidence.py",
            "check_sorafs_governance_dag_rollout_evidence.py",
            "check_sorafs_hedging_rollout_evidence.py",
            "check_sorafs_moderation_panel_rollout_evidence.py",
            "check_sorafs_orderbook_rollout_evidence.py",
            "check_sorafs_pdp_rollout_evidence.py",
            "check_sorafs_pop_credentials_rollout_evidence.py",
            "check_sorafs_por_rollout_evidence.py",
            "check_sorafs_potr_rollout_evidence.py",
            "check_sorafs_repair_rollout_evidence.py",
            "check_sorafs_reserve_rent_rollout_evidence.py",
        },
    }
    missing: dict[str, list[str]] = {}
    local_copies: dict[str, list[str]] = {}
    for helper, expected_names in expected_by_helper.items():
        absent = [
            path.name
            for path in CHECKERS
            if path.name in expected_names and f"{helper}," not in read(path)
        ]
        locals_for_helper = [
            path.name for path in CHECKERS if f"def {helper}(" in read(path)
        ]
        if absent:
            missing[helper] = absent
        if locals_for_helper:
            local_copies[helper] = locals_for_helper

    helper_source = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    local_indexed_bool_errors = [
        path.name
        for path in CHECKERS
        if (
            "routes[{index}].passed must be true" in read(path)
            or "routes[{index}].{field} must be true" in read(path)
            or "streams[{index}].passed must be true" in read(path)
            or "streams[{index}].{field} must be true" in read(path)
            or "steps[{index}].passed must be true" in read(path)
            or f"{{field}}[{{index}}].{{success_field}} must be true" in read(path)
        )
    ]
    local_indexed_bool_predicates = [
        path.name
        for path in CHECKERS
        if (
            'record.get("passed") is not True' in read(path)
            or "record.get(field) is not True" in read(path)
            or 'record.get("authz_enforced") is not True' in read(path)
            or 'record.get("signature_verified") is not True' in read(path)
            or 'record.get("norito_verified") is not True' in read(path)
            or 'record.get("http_success") is not True' in read(path)
            or "record.get(success_field) is not True" in read(path)
            or 'step.get("passed") is not True' in read(path)
        )
    ]
    local_payload_bool_true_predicates = [
        path.name
        for path in CHECKERS
        if re.search(r'payload\.get\("[^"]+"\) is not True', read(path))
    ]
    local_schema_equal_predicates = [
        path.name
        for path in CHECKERS
        if path.name != "check_sorafs_transparency_rollout_evidence.py"
        and 'payload.get("schema") !=' in read(path)
    ]
    local_route_schema_equal_errors = [
        path.name
        for path in CHECKERS
        if "routes[{index}].schema must be `" in read(path)
    ]
    local_route_schema_equal_predicates = [
        path.name
        for path in CHECKERS
        if 'record.get("schema") != expected_schema' in read(path)
    ]
    local_count_equal_errors = [
        path.name
        for path in CHECKERS
        if "reconciled_line_item_count must equal line_item_count" in read(path)
    ]
    local_count_equal_predicates = [
        path.name
        for path in CHECKERS
        if 'payload.get("reconciled_line_item_count") != line_item_count' in read(path)
    ]
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    for helper in expected_by_helper:
        assert f"def {helper}" in helper_source
    assert "must be true" in helper_source
    assert "diagnostic_label" in helper_source
    assert "field_name if path is None else path" in helper_source
    assert "quote_expected" in helper_source
    assert "quote_expected must be a boolean" in helper_source
    assert "test_require_string_equal_rejects_malformed_payloads" in helper_test
    assert "test_require_string_equal_rejects_malformed_quote_option" in helper_test
    assert (
        "test_require_string_equal_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert (
        "test_require_bool_true_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert (
        "test_require_non_negative_int_rejects_malformed_label_before_lookup"
        in helper_test
    )
    assert (
        "test_require_count_equal_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert "must be a non-negative integer" in helper_source
    assert "must equal" in helper_source
    assert missing == {}
    assert local_copies == {}
    assert local_indexed_bool_errors == []
    assert local_indexed_bool_predicates == []
    assert local_payload_bool_true_predicates == []
    assert local_schema_equal_predicates == []
    assert local_route_schema_equal_errors == []
    assert local_route_schema_equal_predicates == []
    assert local_count_equal_errors == []
    assert local_count_equal_predicates == []


def test_rollout_checkers_use_shared_timestamp_validation() -> None:
    expected_names = timestamp_validation_checkers()
    missing = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_recent_timestamp," not in read(path)
    ]
    local_copies = [
        path.name for path in CHECKERS if "def require_recent_timestamp(" in read(path)
    ]
    positional_options = [
        path.name
        for path in CHECKERS
        if re.search(
            r'require_recent_timestamp\\(payload, "[^"]+", errors, options\\)',
            read(path),
        )
    ]
    local_timestamp_errors = [
        path.name
        for path in CHECKERS
        if "must not be in the future" in read(path) or "is older than" in read(path)
    ]
    local_timestamp_predicates = [
        path.name
        for path in CHECKERS
        if "generated_at > now_unix" in read(path)
        or "now_unix - generated_at >" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_recent_timestamp" in helper
    assert "now_unix" in helper
    assert "max_age_secs" in helper
    assert "label_name=\"validation path\"" in helper
    assert "if diagnostic_label is None:\n        return generated_at" not in helper
    assert "must not be in the future" in helper
    assert (
        "test_require_recent_timestamp_rejects_malformed_path_before_compare"
        in helper_test
    )
    assert missing == []
    assert local_copies == []
    assert positional_options == []
    assert local_timestamp_errors == []
    assert local_timestamp_predicates == []


def test_rollout_checkers_use_shared_environment_validation() -> None:
    expected_names = environment_validation_checkers()
    missing_shared_context = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "require_reviewed_deployment_context=True"
        not in function_source(path, "validate_evidence_payload")
    ]
    unexpected_shared_context = [
        path.name
        for path in CHECKERS
        if path.name not in expected_names
        and "require_reviewed_deployment_context=True"
        in function_source(path, "validate_evidence_payload")
    ]
    unexpected_environment_fingerprints = [
        path.name
        for path in CHECKERS
        if '"environment"' in read(path) and path.name not in expected_names
    ]
    local_copies = [
        path.name
        for path in CHECKERS
        if "def require_rollout_environment(" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def _require_canonical_payload_string" in helper
    assert "label_name=field_name" in helper
    assert "def require_rollout_environment" in helper
    assert "ALLOWED_ROLLOUT_ENVIRONMENTS" in helper
    assert "require_rollout_environment(payload, errors)" in helper
    assert (
        "test_require_rollout_environment_rejects_noncanonical_values"
        in helper_test
    )
    assert missing_shared_context == []
    assert unexpected_shared_context == []
    assert unexpected_environment_fingerprints == []
    assert local_copies == []


def test_rollout_checkers_use_shared_deployment_id_validation() -> None:
    expected_names = deployment_id_validation_checkers()
    missing_shared_context = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "require_reviewed_deployment_context=True"
        not in function_source(path, "validate_evidence_payload")
    ]
    unexpected_shared_context = [
        path.name
        for path in CHECKERS
        if path.name not in expected_names
        and "require_reviewed_deployment_context=True"
        in function_source(path, "validate_evidence_payload")
    ]
    unexpected_deployment_fingerprints = [
        path.name
        for path in CHECKERS
        if '"deployment_id"' in read(path) and path.name not in expected_names
    ]
    local_copies = [
        path.name
        for path in CHECKERS
        if "def require_rollout_deployment_id(" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def _require_canonical_payload_string" in helper
    assert "label_name=field_name" in helper
    assert "def require_rollout_deployment_id" in helper
    assert "FORBIDDEN_ROLLOUT_DEPLOYMENT_MARKERS" in helper
    assert "FORBIDDEN_ROLLOUT_DEPLOYMENT_COMPACT_MARKERS" in helper
    assert '"development"' in helper
    assert '"nonproduction"' in helper
    assert '"notprod"' in helper
    assert "compact = \"\".join(tokens)" in helper
    assert "require_rollout_deployment_id(payload, errors)" in helper
    assert (
        "test_require_rollout_deployment_id_rejects_noncanonical_values"
        in helper_test
    )
    assert missing_shared_context == []
    assert unexpected_shared_context == []
    assert unexpected_deployment_fingerprints == []
    assert local_copies == []


def test_rollout_checkers_enforce_consistent_deployment_context() -> None:
    expected_names = environment_validation_checkers() & deployment_id_validation_checkers()
    missing_required_summary = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "build_required_evidence_summary(" not in function_source(
            path,
            "build_summary",
        )
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_summary = function_source(
        EVIDENCE_VALIDATION_HELPER,
        "build_required_evidence_summary",
    )
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def record_consistent_deployment_context" in helper
    assert "def deployment_context_summary" in helper
    assert "deployment_context_summary," in read(
        SCRIPTS_DIR / "check_sorafs_moderation_panel_rollout_evidence.py"
    )
    assert (
        '"deployment_context": deployment_context_summary(deployment_context)'
        in read(SCRIPTS_DIR / "check_sorafs_moderation_panel_rollout_evidence.py")
    )
    assert "values: Any" in helper
    assert "not isinstance(values, Mapping)" in helper
    assert 'label_name=f"deployment context {key}"' in helper
    assert "record_consistent_deployment_context(" in helper_summary
    assert "record_artifact_error(artifact, error, errors)" in helper
    assert '"deployment_id" not in fingerprint' in helper_summary
    assert '"environment" not in fingerprint' in helper_summary
    assert "row_errors: list[str] = []" in helper_summary
    assert '"errors": row_errors' in helper_summary
    assert "mark_required_evidence_summary_invalid(" in helper_summary
    assert "required," in helper_summary
    assert "evidence deployment context must match across artifacts" in helper_summary
    assert "test_build_required_evidence_summary_rejects_mixed_deployments" in helper_test
    assert "test_deployment_context_summary_rejects_malformed_values" in helper_test
    assert missing_required_summary == []


def test_rollout_checkers_use_shared_iroha_config_binding_validation() -> None:
    expected_names = iroha_config_binding_checkers()
    config_backed_names = config_backed_governance_approval_validation_checkers()
    missing_import = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "require_iroha_config_binding," not in read(path)
        and path.name not in config_backed_names
    ]
    missing_source_call = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "require_iroha_config_binding(payload, errors, bound_field=None)"
        not in read(path)
        and "require_config_backed_governance_approval(payload, errors)"
        not in read(path)
    ]
    missing_bound_call = [
        path.name
        for path in CHECKERS
        if path.name in iroha_config_bound_checkers()
        and "require_iroha_config_binding(payload, errors, source_field=None)"
        not in read(path)
        and "require_config_backed_governance_approval(payload, errors)"
        not in read(path)
    ]
    unexpected_config_source_checkers = [
        path.name
        for path in CHECKERS
        if '"config_source"' in read(path) and path.name not in expected_names
    ]
    local_bool_checks = [
        path.name
        for path in CHECKERS
        if 'require_bool_true(payload, "iroha_config_bound", errors)' in read(path)
    ]
    local_source_checks = [
        path.name
        for path in CHECKERS
        if 'require_string_equal(payload, "config_source", "iroha_config", errors)'
        in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_iroha_config_binding" in helper
    assert "test_config_and_governance_helpers_reject_malformed_payloads" in helper_test
    assert missing_import == []
    assert missing_source_call == []
    assert missing_bound_call == []
    assert unexpected_config_source_checkers == []
    assert local_bool_checks == []
    assert local_source_checks == []


def test_rollout_checkers_use_shared_governance_approval_validation() -> None:
    expected_names = governance_approval_validation_checkers()
    config_backed_names = config_backed_governance_approval_validation_checkers()
    release_source_names = expected_names - config_backed_names
    missing_config_backed_import = [
        path.name
        for path in CHECKERS
        if path.name in config_backed_names
        and "require_config_backed_governance_approval," not in read(path)
    ]
    missing_config_backed_call = [
        path.name
        for path in CHECKERS
        if path.name in config_backed_names
        and "require_config_backed_governance_approval(payload, errors)" not in read(path)
    ]
    unexpected_config_backed_call = [
        path.name
        for path in CHECKERS
        if path.name not in config_backed_names
        and "require_config_backed_governance_approval(payload, errors)" in read(path)
    ]
    missing_release_source_import = [
        path.name
        for path in CHECKERS
        if path.name in release_source_names
        and "require_governance_approval," not in read(path)
    ]
    missing_release_source_call = [
        path.name
        for path in CHECKERS
        if path.name in release_source_names
        and "require_governance_approval(payload, errors)" not in read(path)
    ]
    legacy_config_backed_governance_calls = [
        path.name
        for path in CHECKERS
        if path.name in config_backed_names
        and "require_governance_approval(payload, errors)" in read(path)
    ]
    local_config_source_checks = [
        path.name
        for path in CHECKERS
        if path.name in config_backed_names
        and "require_iroha_config_binding(payload, errors, source_field=None)"
        in read(path)
    ]
    missing_policy_digest_import = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_policy_digest," not in read(path)
    ]
    missing_policy_digest_call = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "require_policy_digest(payload, errors)"
        not in function_source(path, "validate_governance_approval")
    ]
    local_policy_digest_calls = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and 'require_hex(payload, "policy_digest_hex", HEX64_LEN, errors)'
        in function_source(path, "validate_governance_approval")
    ]
    unexpected_governance_validators = [
        path.name
        for path in CHECKERS
        if "def validate_governance_approval(" in read(path)
        and path.name not in expected_names
    ]
    local_approval_checks = [
        path.name
        for path in CHECKERS
        if 'require_bool_true(payload, "approved", errors)' in read(path)
    ]
    local_vote_checks = [
        path.name
        for path in CHECKERS
        if 'require_bool_true(payload, "governance_vote_recorded", errors)'
        in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_governance_approval" in helper
    assert "def require_config_backed_governance_approval" in helper
    assert "def require_policy_digest" in helper
    assert "test_config_and_governance_helpers_reject_malformed_payloads" in helper_test
    assert missing_config_backed_import == []
    assert missing_config_backed_call == []
    assert unexpected_config_backed_call == []
    assert missing_release_source_import == []
    assert missing_release_source_call == []
    assert legacy_config_backed_governance_calls == []
    assert local_config_source_checks == []
    assert missing_policy_digest_import == []
    assert missing_policy_digest_call == []
    assert local_policy_digest_calls == []
    assert unexpected_governance_validators == []
    assert local_approval_checks == []
    assert local_vote_checks == []


def test_rollout_checkers_use_shared_policy_digest_validation() -> None:
    expected_names = policy_digest_validation_checkers()
    missing_import = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_policy_digest," not in read(path)
    ]
    missing_call = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and "require_policy_digest(payload, errors)" not in read(path)
    ]
    local_policy_digest_hex = [
        path.name
        for path in CHECKERS
        if 'require_hex(payload, "policy_digest_hex", HEX64_LEN, errors)' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def require_policy_digest" in helper
    assert missing_import == []
    assert missing_call == []
    assert local_policy_digest_hex == []


def test_rollout_checkers_use_shared_hex_validation() -> None:
    expected_names = hex_validation_checkers()
    direct_names = direct_hex_validation_checkers()
    missing_require_hex = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_hex," not in read(path)
    ]
    missing_direct_is_hex = [
        path.name
        for path in CHECKERS
        if path.name in direct_names and "is_hex," not in read(path)
    ]
    local_copies = [
        path.name
        for path in CHECKERS
        if "def require_hex(" in read(path) or "def is_hex(" in read(path)
    ]
    direct_imports = [path.name for path in CHECKERS if "is_hex," in read(path)]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    transparency = read(SCRIPTS_DIR / "check_sorafs_transparency_rollout_evidence.py")

    assert "def is_hex" in helper
    assert "def require_hex" in helper
    assert "not isinstance(payload, Mapping)" in helper
    assert "payload must be an object" in helper
    assert "must be {hex_length} hex characters" in helper
    assert "return value.lower()" in helper
    assert "value = raw_value" in helper
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    assert "test_require_hex_rejects_malformed_payloads" in helper_test
    assert "test_require_hex_rejects_malformed_labels_before_lookup" in helper_test
    assert "test_require_hex_rejects_padded_values_before_normalization" in helper_test
    assert 'path=f"{field}[{index}].request_body_blake3"' in transparency
    assert 'path=f"{field}[{index}].response_body_blake3"' in transparency
    assert 'path=f"routes[{index}].body_blake3_hex"' in transparency
    assert missing_require_hex == []
    assert missing_direct_is_hex == []
    assert local_copies == []
    assert direct_imports == []


def test_rollout_checkers_use_shared_hex_string_array_validation() -> None:
    expected_names = hex_string_array_validation_checkers()
    missing = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_hex_string_array," not in read(path)
    ]
    local_hex_array_loops = [
        path.name
        for path in CHECKERS
        if "statement_digests_hex must be a non-empty array" in read(path)
        or "statement_digests_hex length must equal statement_count" in read(path)
        or "statement_digests_hex[{index}] must be unique" in read(path)
        or "proof.siblings_hex must be an array" in read(path)
        or "proof.siblings_hex[{index}] must be" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    hedging = read(SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py")
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_hex_string_array" in helper
    assert "expected_length_label" in helper
    assert "must be unique" in helper
    assert "has_array_error = False" in helper
    assert "if has_array_error:\n        return []" in helper
    assert "test_require_hex_string_array_rejects_malformed_payloads" in helper_test
    assert (
        "test_require_hex_string_array_returns_empty_on_dirty_values"
        in helper_test
    )
    assert (
        "test_require_hex_string_array_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert "path=\"proof.siblings_hex\"" in reputation
    assert "expected_length_label=\"statement_count\"" in hedging
    assert missing == []
    assert local_hex_array_loops == []


def test_rollout_checkers_use_shared_negative_validation_primitives() -> None:
    expected_by_helper = {
        "require_false": false_validation_checkers(),
        "require_false_or_absent": false_or_absent_validation_checkers(),
        "require_false_or_governed": false_or_governed_validation_checkers(),
        "require_non_negative_number": non_negative_number_validation_checkers(),
    }
    missing: dict[str, list[str]] = {}
    local_copies: dict[str, list[str]] = {}
    for helper, expected_names in expected_by_helper.items():
        absent = [
            path.name
            for path in CHECKERS
            if path.name in expected_names and f"{helper}," not in read(path)
        ]
        locals_for_helper = [
            path.name for path in CHECKERS if f"def {helper}(" in read(path)
        ]
        if absent:
            missing[helper] = absent
        if locals_for_helper:
            local_copies[helper] = locals_for_helper

    helper_source = read(EVIDENCE_VALIDATION_HELPER)
    local_latency_number_errors = [
        path.name
        for path in CHECKERS
        if "latency_ms must be a non-negative number" in read(path)
    ]
    local_latency_number_predicates = [
        path.name
        for path in CHECKERS
        if "isinstance(latency, (int, float))" in read(path)
    ]
    plain_latency_number_calls = [
        path.name
        for path in CHECKERS
        if 'require_non_negative_number(record, "latency_ms", errors)' in read(path)
    ]

    assert "def require_false" in helper_source
    assert "def require_false_or_absent" in helper_source
    assert "def require_false_or_governed" in helper_source
    assert "def require_non_negative_number" in helper_source
    assert "math.isfinite(value)" in helper_source
    assert "diagnostic_label" in helper_source
    assert "must be false" in helper_source
    assert "must be false when present" in helper_source
    assert "must be false or explicitly governed" in helper_source
    assert "payload must be an object" in helper_source
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    assert "test_require_false_rejects_malformed_payloads" in helper_test
    assert "test_require_false_or_absent_rejects_malformed_payloads" in helper_test
    assert "test_require_false_or_governed_rejects_malformed_payloads" in helper_test
    assert (
        "test_require_false_helpers_reject_malformed_labels_before_lookup"
        in helper_test
    )
    assert "test_require_status_and_number_helpers_reject_malformed_payloads" in helper_test
    assert (
        "test_require_non_negative_number_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert (
        "test_require_maximum_number_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert 'payload.get("hedge_execution_enabled")' not in read(
        SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py"
    )
    assert "not exactly true" not in helper_source
    assert "must be a non-negative number" in helper_source
    assert missing == {}
    assert local_copies == {}
    assert local_latency_number_errors == []
    assert local_latency_number_predicates == []
    assert plain_latency_number_calls == []


def test_rollout_checkers_use_shared_remaining_require_validation_primitives() -> None:
    expected_by_helper = {
        "require_optional_hex": optional_hex_validation_checkers(),
        "require_score_bps": score_bps_validation_checkers(),
        "require_int_range": int_range_validation_checkers(),
        "require_advancing_int_pair": advancing_int_pair_validation_checkers(),
        "require_count_match": count_match_validation_checkers(),
        "require_count_value_equal": count_value_equal_validation_checkers(),
        "require_count_length_match": count_length_match_validation_checkers(),
        "require_sum_equal": sum_equal_validation_checkers(),
        "require_zero_count": zero_count_validation_checkers(),
        "require_minimum_int": minimum_int_validation_checkers(),
        "require_minimum_value": minimum_value_validation_checkers(),
        "require_maximum_value": maximum_value_validation_checkers(),
        "require_maximum_int": maximum_int_validation_checkers(),
        "require_maximum_number": maximum_number_validation_checkers(),
        "require_passed_status": passed_status_validation_checkers(),
        "require_string_in": string_in_validation_checkers(),
        "require_string_not_equal": string_not_equal_validation_checkers(),
        "require_string_value_equal": string_value_equal_validation_checkers(),
    }
    missing: dict[str, list[str]] = {}
    local_copies: dict[str, list[str]] = {}
    for helper, expected_names in expected_by_helper.items():
        absent = [
            path.name
            for path in CHECKERS
            if path.name in expected_names and f"{helper}," not in read(path)
        ]
        locals_for_helper = [
            path.name for path in CHECKERS if f"def {helper}(" in read(path)
        ]
        if absent:
            missing[helper] = absent
        if locals_for_helper:
            local_copies[helper] = locals_for_helper

    local_int_range_errors = [
        path.name
        for path in CHECKERS
        if "must be an integer in 0..=10000" in read(path)
    ]
    local_int_range_predicates = [
        path.name for path in CHECKERS if "not (0 <=" in read(path)
    ]
    local_advancing_int_pair_errors = [
        path.name for path in CHECKERS if "must advance past since" in read(path)
    ]
    local_advancing_int_pair_predicates = [
        path.name for path in CHECKERS if "next_since <= since" in read(path)
    ]
    local_count_length_errors = [
        path.name
        for path in CHECKERS
        if (
            "route_count must equal routes length" in read(path)
            or "probe_count must equal probes length" in read(path)
            or "artifact_count must equal artifacts length" in read(path)
            or "count must equal events length" in read(path)
        )
    ]
    local_count_length_predicates = [
        path.name
        for path in CHECKERS
        if (
            "route_count != len(" in read(path)
            or "probe_count != len(" in read(path)
            or "artifact_count != len(" in read(path)
            or "count != len(event_records)" in read(path)
        )
    ]
    local_count_value_equal_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in count_value_equal_validation_checkers()
            and "logged_session_count must equal session_count" in read(path)
        )
    ]
    local_count_value_equal_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in count_value_equal_validation_checkers()
            and 'payload.get("logged_session_count") != session_count' in read(path)
        )
    ]
    local_sum_equal_errors = [
        path.name
        for path in CHECKERS
        if (
            "approved_appeal_count plus rejected_appeal_count must equal" in read(path)
            or "accepted_valid_proof_count plus rejected_invalid_proof_count must equal"
            in read(path)
        )
    ]
    local_sum_equal_predicates = [
        path.name
        for path in CHECKERS
        if "approved + rejected !=" in read(path) or "accepted + rejected !=" in read(path)
    ]
    local_zero_count_errors = [
        path.name
        for path in CHECKERS
        if "must be 0" in read(path)
    ]
    local_zero_count_predicates = [
        path.name
        for path in CHECKERS
        if (
            "require_non_negative_int(payload," in read(path)
            and ") != 0" in read(path)
        )
    ]
    local_minimum_int_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in minimum_int_validation_checkers()
            and (
                "gateway_ack_count must be at least" in read(path)
                or "denylist_entry_count must be at least" in read(path)
                or "reload_ack_count must be at least" in read(path)
                or "honey_probe_count must be at least" in read(path)
                or "provider_count must be at least" in read(path)
                or "challenge_count must be at least" in read(path)
                or "proof_count must be at least" in read(path)
                or "receipt_count must be at least" in read(path)
                or "auditor_count must be at least" in read(path)
                or "class_count must be at least" in read(path)
                or "peer_count must be at least" in read(path)
                or "validator_count must be at least" in read(path)
                or "block_count must be at least" in read(path)
                or "payload_kind_count must be at least" in read(path)
                or "bridge_abi_version must be at least" in read(path)
                or "panel_size must be at least" in read(path)
                or "target_count must be at least" in read(path)
                or "package_count must be at least" in read(path)
                or "tier_count must be at least" in read(path)
                or "storage_class_count must be at least" in read(path)
                or "duration_count must be at least" in read(path)
            )
        )
    ]
    local_minimum_int_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in minimum_int_validation_checkers()
            and (
                "provider_count < options.min_" in read(path)
                or "challenge_count < options.min_" in read(path)
                or "proof_count < options.min_" in read(path)
                or "receipt_count < options.min_" in read(path)
                or "auditor_count < options.min_" in read(path)
                or "require_positive_int(payload, \"gateway_ack_count\"" in read(path)
                or "require_positive_int(payload, \"denylist_entry_count\"" in read(path)
                or "require_positive_int(payload, \"reload_ack_count\"" in read(path)
                or "require_positive_int(payload, \"honey_probe_count\"" in read(path)
                or "require_positive_int(payload, \"class_count\"" in read(path)
                or "require_positive_int(payload, \"peer_count\"" in read(path)
                or "require_positive_int(payload, \"validator_count\"" in read(path)
                or "require_positive_int(payload, \"block_count\"" in read(path)
                or "require_positive_int(payload, \"payload_kind_count\"" in read(path)
                or "require_positive_int(payload, \"bridge_abi_version\"" in read(path)
                or "require_positive_int(payload, \"panel_size\"" in read(path)
                or "require_positive_int(payload, \"target_count\"" in read(path)
                or "require_positive_int(payload, \"package_count\"" in read(path)
                or "require_positive_int(payload, \"tier_count\"" in read(path)
                or "require_positive_int(payload, \"storage_class_count\"" in read(path)
                or "require_positive_int(payload, \"duration_count\"" in read(path)
            )
        )
    ]
    local_minimum_value_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in minimum_value_validation_checkers()
            and (
                'errors.append("result_count must be at least quorum")' in read(path)
                or 'errors.append("execution_summary.action_count must be at least 1")'
                in read(path)
                or 'errors.append("feed_count must be at least 2")' in read(path)
                or 'errors.append("feed_count and accepted_feed_count must both be at least 2")'
                in read(path)
                or 'errors.append("statement_count must be at least 1")' in read(path)
                or 'errors.append("scenario_count must cover at least the 3x3x3 policy matrix")'
                in read(path)
                or 'errors.append("credential_count must be at least 1")' in read(path)
                or 'errors.append("route_count must be at least 3")' in read(path)
                or 'errors.append("completed_at_unix must be >= started_at_unix")'
                in read(path)
                or 'errors.append(\n            f"producer_count must be at least'
                in read(path)
                or (
                    'errors.append(\n                "billing_cycle rollout evidence '
                    'must include at least "'
                )
                in read(path)
            )
        )
    ]
    local_minimum_value_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in minimum_value_validation_checkers()
            and (
                "result_count < quorum" in read(path)
                or "action_count < 1" in read(path)
                or "producer_count < len(REQUIRED_GOVERNANCE_PRODUCERS)" in read(path)
                or "feed_count < 2" in read(path)
                or "accepted < 2" in read(path)
                or "statement_count < 1" in read(path)
                or "len(distinct_cycle_ids) < options.min_billing_cycles"
                in read(path)
                or "scenario_count < 27" in read(path)
                or "credential_count < 1" in read(path)
                or "route_count < 3" in read(path)
                or "completed_at < started_at" in read(path)
            )
        )
    ]
    local_maximum_value_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in maximum_value_validation_checkers()
            and 'errors.append("quorum must be <= panel_size")' in read(path)
        )
    ]
    local_maximum_value_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in maximum_value_validation_checkers()
            and "quorum > panel_size" in read(path)
        )
    ]
    local_maximum_number_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in maximum_number_validation_checkers()
            and (
                "max_reload_latency_ms must be <=" in read(path)
                or "max_proof_latency_ms must be <=" in read(path)
                or "max_scheduler_lag_seconds must be <=" in read(path)
                or "report_latency_ms must be <=" in read(path)
                or "max_hot_latency_ms must be <=" in read(path)
                or "max_warm_latency_ms must be <=" in read(path)
                or "repair_latency_seconds must be <=" in read(path)
                or "event_lag_seconds must be <=" in read(path)
                or "routes[{index}].latency_ms must be <=" in read(path)
                or "max_route_latency_ms must be <=" in read(path)
                or "max_settlement_lag_seconds must be <=" in read(path)
                or "pin_lag_seconds must be <=" in read(path)
                or "head_age_seconds must be <=" in read(path)
                or "feed_lag_seconds must be <=" in read(path)
                or "divergence_bps must be <=" in read(path)
                or "decision_lag_seconds must be <=" in read(path)
                or "max_event_lag_seconds must be <=" in read(path)
                or "matcher_lag_ms must be <=" in read(path)
                or "streams[{index}].lag_ms must be <=" in read(path)
                or "smoke_duration_seconds must be <=" in read(path)
                or "max_lifecycle_lag_seconds must be <=" in read(path)
                or "snapshot_age_seconds exceeds" in read(path)
                or "ingest_lag_seconds exceeds" in read(path)
            )
        )
    ]
    local_maximum_number_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in maximum_number_validation_checkers()
            and (
                "reload_latency > options.max_reload_latency_ms" in read(path)
                or "proof_latency > options.max_proof_latency_ms" in read(path)
                or "lag > options.max_scheduler_lag_secs" in read(path)
                or "report_latency > options.max_report_latency_ms" in read(path)
                or "hot_latency > options.max_hot_latency_ms" in read(path)
                or "warm_latency > options.max_warm_latency_ms" in read(path)
                or "latency > options.max_repair_latency_secs" in read(path)
                or "lag > options.max_event_lag_secs" in read(path)
                or "latency > options.max_route_latency_ms" in read(path)
                or "max_latency > options.max_route_latency_ms" in read(path)
                or "lag > options.max_settlement_lag_secs" in read(path)
                or "pin_lag > options.max_pin_lag_secs" in read(path)
                or "head_age > options.max_head_age_secs" in read(path)
                or "lag > options.max_feed_lag_secs" in read(path)
                or "divergence > options.max_divergence_bps" in read(path)
                or "require_non_negative_number(payload, \"matcher_lag_ms\"" in read(path)
                or "require_non_negative_number(record, \"lag_ms\"" in read(path)
                or "duration > options.max_smoke_duration_secs" in read(path)
                or "lag > options.max_lifecycle_lag_secs" in read(path)
                or "snapshot_age > max_snapshot_age_secs" in read(path)
                or "ingest_lag > max_ingest_lag_secs" in read(path)
            )
        )
    ]
    local_maximum_int_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in maximum_int_validation_checkers()
            and (
                "max_verify_latency_ms must be <=" in read(path)
                or "max_service_lag_seconds must be <=" in read(path)
                or "max_url_ttl_secs must be <=" in read(path)
            )
        )
    ]
    local_maximum_int_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in maximum_int_validation_checkers()
            and (
                "latency > options.max_verify_latency_ms" in read(path)
                or "lag > options.max_service_lag_secs" in read(path)
                or "url_ttl > DEFAULT_MAX_VIEWER_URL_TTL_SECS" in read(path)
            )
        )
    ]
    local_passed_status_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in passed_status_validation_checkers()
            and "status must be passed" in read(path)
        )
    ]
    local_passed_status_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in passed_status_validation_checkers()
            and 'payload.get("status") != "passed"' in read(path)
        )
    ]
    local_string_in_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in string_in_validation_checkers()
            and "manual_trigger_route_state must be `wired` or `retired`" in read(path)
        )
    ]
    local_string_in_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in string_in_validation_checkers()
            and "state not in ALLOWED_MANUAL_TRIGGER_STATES" in read(path)
        )
    ]
    local_string_not_equal_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in string_not_equal_validation_checkers()
            and 'errors.append("privacy_proof_system must be production privacy-preserving proof backend")'
            in read(path)
        )
    ]
    local_string_not_equal_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in string_not_equal_validation_checkers()
            and 'proof_system == "transcript_digest_v1"' in read(path)
        )
    ]
    local_string_value_equal_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in string_value_equal_validation_checkers()
            and 'errors.append("proof.provider_id must match provider.provider_id")'
            in read(path)
        )
    ]
    local_string_value_equal_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in string_value_equal_validation_checkers()
            and "provider_id != proof_provider_id" in read(path)
        )
    ]
    local_schema_string_type_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in schema_string_type_validation_checkers()
            and "schema must be a string" in read(path)
        )
    ]
    local_schema_string_type_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in schema_string_type_validation_checkers()
            and 'schema = payload.get("schema")' in read(path)
            and "not isinstance(schema, str)" in read(path)
        )
    ]
    local_known_schema_errors = [
        path.name
        for path in CHECKERS
        if (
            path.name in schema_string_type_validation_checkers()
            and "is not a recognized SoraFS" in read(path)
        )
    ]
    local_known_schema_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in schema_string_type_validation_checkers()
            and "kind = SCHEMA_TO_KIND.get(schema)" in read(path)
        )
    ]
    helper_source = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_optional_hex" in helper_source
    assert "def require_score_bps" in helper_source
    assert "def require_int_range" in helper_source
    assert "def require_advancing_int_pair" in helper_source
    assert "def require_count_match" in helper_source
    assert "def require_count_value_equal" in helper_source
    assert "def require_count_length_match" in helper_source
    assert "def require_sum_equal" in helper_source
    assert "def require_zero_count" in helper_source
    assert "def require_minimum_int" in helper_source
    assert "def require_minimum_value" in helper_source
    assert "def require_maximum_value" in helper_source
    assert "def require_maximum_number" in helper_source
    assert "def require_passed_status" in helper_source
    assert "def require_status_in" in helper_source
    assert "payload must be an object" in helper_source
    assert "def require_string_in" in helper_source
    assert "allowed values must be a sequence of strings" in helper_source
    assert "validation value" in helper_source
    assert "quote_values must be a boolean" in helper_source
    assert "test_require_string_in_rejects_malformed_quote_option" in helper_test
    assert "test_require_string_in_rejects_malformed_labels_before_lookup" in helper_test
    assert (
        "test_require_string_in_rejects_noncanonical_values_before_membership"
        in helper_test
    )
    assert "def require_string_not_equal" in helper_source
    assert "def require_string_value_equal" in helper_source
    assert "validation expected value" in helper_source
    assert "validation disallowed value" in helper_source
    assert "must not be `{disallowed_value}`" in helper_source
    assert "value = _require_validation_label(" in helper_source
    assert "label_name=diagnostic_label" in helper_source
    assert "validation value label" in helper_source
    assert "validation expected label" in helper_source
    assert "label_name=value_label" in helper_source
    assert "label_name=expected_value_label" in helper_source
    assert "value_value = _require_validation_label(" in helper_source
    assert "expected_value = _require_validation_label(" in helper_source
    assert 'return "" if value_label is None else' not in helper_source
    assert "if expected_value is None:\n        return value_value" not in helper_source
    assert (
        "if message is not None and message_label is None:\n            return value_value"
        not in helper_source
    )
    assert "def require_string_type" in helper_source
    assert "label_name=diagnostic_label" in helper_source
    assert (
        "test_require_string_helpers_reject_malformed_labels_before_lookup"
        in helper_test
    )
    assert "test_require_known_schema_rejects_malformed_schema_before_lookup" in helper_test
    assert (
        "test_require_string_equal_rejects_malformed_expected_value_before_compare"
        in helper_test
    )
    assert "test_require_string_equal_rejects_malformed_values_before_compare" in helper_test
    assert (
        "test_require_string_not_equal_rejects_malformed_labels_before_compare"
        in helper_test
    )
    assert (
        "test_require_string_not_equal_reports_default_disallowed_value_error"
        in helper_test
    )
    assert (
        "test_require_string_value_equal_rejects_malformed_labels_before_compare"
        in helper_test
    )
    assert (
        "test_require_string_value_equal_reports_malformed_or_non_string_values"
        in helper_test
    )
    assert "def require_known_schema" in helper_source
    assert "must be null or {hex_length} hex characters" in helper_source
    assert "must be <= 10000" in helper_source
    assert "must be an integer in {min_value}..={max_value}" in helper_source
    assert "must advance past" in helper_source
    assert "must equal {collection_name} length" in helper_source
    assert " plus \".join" in helper_source
    assert "if total == 0:" in helper_source
    assert "must be at least {minimum}" in helper_source
    assert "must be <= {maximum}" in helper_source
    assert "validation threshold label" in helper_source
    assert 'require_status_in(payload, ("passed",), errors, path=path)' in helper_source
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    assert "test_require_status_helpers_reject_malformed_payloads" in helper_test
    assert "test_require_status_in_rejects_malformed_allow_absent_flag" in helper_test
    assert "allow_absent must be a boolean" in helper_source
    assert "test_require_optional_hex_rejects_malformed_payloads" in helper_test
    assert "test_require_optional_hex_rejects_malformed_label_before_lookup" in helper_test
    assert "test_require_hex_rejects_malformed_length_before_lookup" in helper_test
    assert (
        "test_require_optional_hex_rejects_malformed_length_before_lookup"
        in helper_test
    )
    assert (
        "test_require_hex_string_array_rejects_malformed_lengths_before_compare"
        in helper_test
    )
    assert (
        "test_require_hex_string_array_rejects_malformed_boolean_options"
        in helper_test
    )
    assert "def _require_hex_length" in helper_source
    assert "validation hex length must be a positive integer" in helper_source
    assert "expected length must be a non-negative integer" in helper_source
    assert "non_empty must be a boolean" in helper_source
    assert "unique must be a boolean" in helper_source
    assert "test_require_bool_true_rejects_malformed_payloads" in helper_test
    assert "test_require_numeric_integer_helpers_reject_malformed_payloads" in helper_test
    assert "test_require_minimum_int_rejects_malformed_label_before_lookup" in helper_test
    assert "test_require_int_range_rejects_malformed_labels_before_lookup" in helper_test
    assert "test_require_int_range_rejects_malformed_thresholds" in helper_test
    assert "test_require_int_range_rejects_malformed_custom_messages" in helper_test
    assert (
        "test_require_advancing_int_pair_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert "test_require_score_bps_rejects_malformed_label_before_lookup" in helper_test
    assert "test_require_maximum_int_rejects_malformed_labels_before_lookup" in helper_test
    assert "test_require_count_helpers_reject_malformed_payloads" in helper_test
    assert (
        "test_require_count_value_equal_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert "test_require_count_value_equal_rejects_bool_counts" in helper_test
    assert (
        "test_require_count_value_equal_rejects_malformed_expected_counts"
        in helper_test
    )
    assert "test_require_count_match_rejects_malformed_labels_before_lookup" in helper_test
    assert "test_require_count_equal_rejects_bool_passed_counts" in helper_test
    assert "test_require_count_match_rejects_bool_passed_counts" in helper_test
    assert "isinstance(passed, bool)" in helper_source
    assert "isinstance(value, bool)" in helper_source
    assert "isinstance(expected_count, bool)" in helper_source
    assert "expected_count <= 0" in helper_source
    assert (
        "test_require_count_length_match_rejects_malformed_labels_before_compare"
        in helper_test
    )
    assert "test_require_count_length_match_rejects_malformed_counts" in helper_test
    assert (
        "test_require_count_length_match_rejects_malformed_collections"
        in helper_test
    )
    assert "artifacts must be a sequence" in helper_test
    assert "isinstance(count, bool)" in helper_source
    assert "count < 0" in helper_source
    assert "test_require_sum_equal_rejects_malformed_labels_before_compare" in helper_test
    assert "test_require_sum_equal_rejects_malformed_total_counts" in helper_test
    assert "test_require_sum_equal_rejects_malformed_part_counts" in helper_test
    assert (
        "test_require_sum_equal_rejects_malformed_skip_zero_option" in helper_test
    )
    assert "skip_zero_total must be a boolean" in helper_source
    assert "isinstance(total, bool)" in helper_source
    assert "total < 0" in helper_source
    assert "value < 0" in helper_source
    assert (
        "test_require_minimum_value_rejects_malformed_label_before_compare"
        in helper_test
    )
    assert "test_require_minimum_value_rejects_malformed_numeric_inputs" in helper_test
    assert (
        "test_require_minimum_value_rejects_malformed_custom_messages"
        in helper_test
    )
    assert (
        "test_require_maximum_value_rejects_malformed_label_before_compare"
        in helper_test
    )
    assert "test_require_maximum_value_rejects_malformed_numeric_inputs" in helper_test
    assert (
        "test_require_maximum_value_rejects_malformed_custom_messages"
        in helper_test
    )
    assert "test_require_maximum_number_rejects_malformed_thresholds" in helper_test
    assert "test_require_maximum_int_rejects_malformed_thresholds" in helper_test
    assert "isinstance(minimum, bool)" in helper_source
    assert "isinstance(maximum, bool)" in helper_source
    assert "minimum threshold must be an integer" in helper_source
    assert "maximum threshold must be an integer" in helper_source
    assert "maximum threshold must be a non-negative number" in helper_source
    assert "maximum threshold must be >= minimum threshold" in helper_source
    assert "test_require_recent_timestamp_rejects_malformed_thresholds" in helper_test
    assert "current time must be a non-negative integer timestamp" in helper_source
    assert "maximum age must be a non-negative integer" in helper_source
    assert "if threshold_label is None:\n        return value" not in helper_source
    assert "label_name=\"validation message\"" in helper_source
    assert "must not be `{disallowed_value}`" in helper_source
    assert "f\"{field_name} must not be `{disallowed_value}`\"" not in helper_source
    assert "f\"{field} must not be `{disallowed_value}`\"" in helper_source
    assert "if disallowed_value is None:\n        return value" not in helper_source
    assert (
        "if message is not None and message_label is None:\n        return value"
        not in helper_source
    )
    assert "must match {expected_value_label}" in helper_source
    assert "must be a string" in helper_source
    assert "is not a recognized {artifact_label}" in helper_source
    assert missing == {}
    assert local_copies == {}
    assert local_int_range_errors == []
    assert local_int_range_predicates == []
    assert local_advancing_int_pair_errors == []
    assert local_advancing_int_pair_predicates == []
    assert local_count_length_errors == []
    assert local_count_length_predicates == []
    assert local_count_value_equal_errors == []
    assert local_count_value_equal_predicates == []
    assert local_sum_equal_errors == []
    assert local_sum_equal_predicates == []
    assert local_zero_count_errors == []
    assert local_zero_count_predicates == []
    assert local_minimum_int_errors == []
    assert local_minimum_int_predicates == []
    assert local_minimum_value_errors == []
    assert local_minimum_value_predicates == []
    assert local_maximum_value_errors == []
    assert local_maximum_value_predicates == []
    assert local_maximum_number_errors == []
    assert local_maximum_number_predicates == []
    assert local_maximum_int_errors == []
    assert local_maximum_int_predicates == []
    assert local_passed_status_errors == []
    assert local_passed_status_predicates == []
    assert local_string_in_errors == []
    assert local_string_in_predicates == []
    assert local_string_not_equal_errors == []
    assert local_string_not_equal_predicates == []
    assert local_string_value_equal_errors == []
    assert local_string_value_equal_predicates == []
    assert local_schema_string_type_errors == []
    assert local_schema_string_type_predicates == []
    assert local_known_schema_errors == []
    assert local_known_schema_predicates == []


def test_rollout_checkers_use_shared_tuple_binding_validation() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if (
            path.name in tuple_binding_validation_checkers()
            and (
                (
                    path.name == "check_sorafs_reputation_rollout_evidence.py"
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "validate_snapshot_bound_evidence_artifacts," not in read(path)
                        or "validate_snapshot_bound_evidence_artifacts(" not in read(path)
                    )
                )
                or (
                    path.name in tuple_bound_reference_checkers()
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "validate_bound_evidence_tuple_references," not in read(path)
                        or "validate_bound_evidence_tuple_references(" not in read(path)
                        or "record_string_tuple_binding_errors," in read(path)
                        or "record_string_tuple_binding_errors(" in read(path)
                    )
                )
                or (
                    path.name in tuple_binding_error_recorder_checkers()
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "record_string_tuple_binding_errors," not in read(path)
                        or "record_string_tuple_binding_errors(" not in read(path)
                    )
                )
                or (
                    path.name != "check_sorafs_reputation_rollout_evidence.py"
                    and path.name not in tuple_bound_reference_checkers()
                    and path.name not in tuple_binding_error_recorder_checkers()
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "require_string_tuple_in," not in read(path)
                        or "require_string_tuple_in(" not in read(path)
                    )
                )
                or "def require_string_tuple_in(" in read(path)
            )
        )
    ]
    local_tuple_binding_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in tuple_binding_validation_checkers()
            and re.search(r"binding not in valid_[a-z_]*bindings", read(path))
        )
    ]
    helper_source = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    helper_module = load_script_module(
        EVIDENCE_VALIDATION_HELPER, "sorafs_evidence_validation_tuple_contract"
    )
    errors: list[str] = []
    ai_prescreen = read(SCRIPTS_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py")
    hedging = read(SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py")
    moderation = read(SCRIPTS_DIR / "check_sorafs_moderation_panel_rollout_evidence.py")
    reserve = read(SCRIPTS_DIR / "check_sorafs_reserve_rent_rollout_evidence.py")

    assert "def require_string_tuple_in" in helper_source
    assert "def record_string_tuple_binding_errors" in helper_source
    assert "def validate_bound_evidence_tuple_references" in helper_source
    assert "def _canonical_tuple_binding_set" in helper_source
    assert "valid_anchor_binding_values = _canonical_tuple_binding_set" in helper_source
    assert "def _fingerprint_field_names" in helper_source
    assert "def _validate_kind_error_template" in helper_source
    assert "def _format_kind_error_template" in helper_source
    assert "def _evidence_kind_artifact_pairs" in helper_source
    assert "def _required_evidence_has_any_kind_or_error" in helper_source
    assert "validation required evidence kinds" in helper_source
    assert "validation missing-anchor required evidence kinds" in helper_source
    assert "_evidence_kind_artifact_pairs(" in helper_source
    assert 'label="bound evidence artifacts"' in helper_source
    assert "validation binding error template" in helper_source
    assert "validation missing-anchor error template" in helper_source
    assert "validation missing-anchor summary error" in helper_source
    assert "validation binding fields must be a sequence of strings" in helper_source
    assert "validation binding fields must not be empty" in helper_source
    assert "validation binding field" in helper_source
    assert (
        "validation anchor bindings must be a collection of canonical string sequences"
        in helper_source
    )
    assert "validation anchor binding value" in helper_source
    assert "must use only plain kind_name formatter fields" in helper_source
    assert "must be a valid formatter template" in helper_source
    assert "validation evidence kind" in helper_source
    assert ".format(kind_name=kind_name)" not in helper_source
    assert "values: Any" in helper_source
    assert "allowed: Any" in helper_source
    assert "isinstance(values, (str, bytes, bytearray))" in helper_source
    assert "isinstance(allowed, (str, bytes, bytearray, Mapping))" in helper_source
    assert "value.lower()" in helper_source
    assert "normalized_values: list[str] = []" in helper_source
    assert "allowed_bindings: set[tuple[str, ...]] = set()" in helper_source
    assert "normalized_allowed.append(allowed_label.lower())" in helper_source
    assert "allowed_bindings.add(tuple(normalized_allowed))" in helper_source
    assert "for value in values:" in helper_source
    assert "validation message" in helper_source
    assert "validation tuple value" in helper_source
    assert "validation allowed tuple value" in helper_source
    assert "record_artifact_error(artifact, error, errors)" in helper_source
    assert (
        "test_record_string_tuple_binding_errors_rejects_malformed_artifact_rows"
        in helper_test
    )
    assert (
        "test_require_string_tuple_in_rejects_malformed_labels_before_binding"
        in helper_test
    )
    assert "test_require_string_tuple_in_normalizes_allowed_bindings" in helper_test
    assert (
        "test_validate_bound_evidence_tuple_references_rejects_malformed_templates_before_mutation"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_tuple_references_rejects_malformed_binding_fields"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_tuple_references_rejects_malformed_anchor_bindings_before_mutation"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_tuple_references_rejects_malformed_required_kind_sets_before_missing_anchor_mutation"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_tuple_references_rejects_malformed_missing_anchor_messages"
        in helper_test
    )
    assert (
        helper_module.require_string_tuple_in(
            ("AA", "bb"), {("aa", "bb")}, errors, message="missing binding"
        )
        == ("aa", "bb")
    )
    assert errors == []
    assert (
        helper_module.require_string_tuple_in(
            ("aa", "bb"), {("AA", "BB")}, errors, message="missing binding"
        )
        == ("aa", "bb")
    )
    assert errors == []
    assert (
        helper_module.require_string_tuple_in(
            ("AA", None), {("aa", "bb")}, errors, message="missing binding"
        )
        is None
    )
    assert errors == ["missing binding"]
    assert (
        helper_module.require_string_tuple_in(
            ("AA", ""), {("aa", "")}, errors, message="missing binding"
        )
        is None
    )
    assert errors == ["missing binding", "missing binding"]
    assert (
        helper_module.require_string_tuple_in(
            "AB", {("a", "b")}, errors, message="missing binding"
        )
        is None
    )
    assert errors == ["missing binding", "missing binding", "missing binding"]
    assert (
        helper_module.require_string_tuple_in(
            ("AA", "BB"),
            {("aa", "bb"): True},
            errors,
            message="missing binding",
        )
        is None
    )
    assert errors == [
        "missing binding",
        "missing binding",
        "missing binding",
        "missing binding",
    ]
    assert "validate_bound_evidence_tuple_references(" in ai_prescreen
    assert "record_string_tuple_binding_errors(" not in ai_prescreen
    assert "require_string_tuple_in," not in ai_prescreen
    assert (
        "(manifest_id, runner_hash, subject_digest),\n                valid_runner_bindings,\n                binding_errors"
        not in ai_prescreen
    )
    assert "validate_bound_evidence_tuple_references(" in hedging
    assert "record_string_tuple_binding_errors(" not in hedging
    assert "require_string_tuple_in," not in hedging
    assert "statement_bundle, reconciliation_digest),\n                valid_cycle_bindings,\n                binding_errors" not in hedging
    assert "validate_bound_evidence_tuple_references(" in moderation
    assert "record_string_tuple_binding_errors(" not in moderation
    assert "require_string_tuple_in," not in moderation
    assert "valid_roster_bindings,\n                binding_errors" not in moderation
    assert "valid_tally_bindings,\n                binding_errors" not in moderation
    assert "validate_bound_evidence_tuple_references(" in reserve
    assert "record_string_tuple_binding_errors(" not in reserve
    assert "require_string_tuple_in," not in reserve
    assert "valid_policy_matrix_bindings,\n                binding_errors" not in reserve
    assert "valid_policy_matrix_ledger_bindings,\n                binding_errors" not in reserve
    assert missing == []
    assert local_tuple_binding_predicates == []


def test_rollout_checkers_use_shared_scalar_binding_validation() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if (
            path.name in scalar_binding_validation_checkers()
            and (
                (
                    path.name in scalar_bound_digest_reference_checkers()
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "validate_bound_evidence_digest_references," not in read(path)
                        or "validate_bound_evidence_digest_references(" not in read(path)
                        or "record_string_value_binding_errors," in read(path)
                        or "record_string_value_binding_errors(" in read(path)
                        or "record_artifact_error," in read(path)
                        or "record_artifact_error(" in read(path)
                        or "required_evidence_has_any_kind," in read(path)
                        or "required_evidence_has_any_kind(" in read(path)
                    )
                )
                or (
                    path.name not in scalar_bound_digest_reference_checkers()
                    and path.name in scalar_binding_error_recorder_checkers()
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "record_string_value_binding_errors," not in read(path)
                        or "record_string_value_binding_errors(" not in read(path)
                        or "require_string_value_in," in read(path)
                        or "require_string_value_in(" in read(path)
                    )
                )
                or (
                    path.name not in scalar_bound_digest_reference_checkers()
                    and path.name not in scalar_binding_error_recorder_checkers()
                    and (
                        "sorafs_evidence_validation" not in read(path)
                        or "require_string_value_in," not in read(path)
                        or "require_string_value_in(" not in read(path)
                    )
                )
                or "def require_string_value_in(" in read(path)
            )
        )
    ]
    local_scalar_binding_predicates = [
        path.name
        for path in CHECKERS
        if (
            path.name in scalar_binding_validation_checkers()
            and re.search(
                r"not isinstance\((?:digest|decision_id), str\)"
                r"[\s\S]{0,120}\.lower\(\) not in valid_",
                read(path),
            )
        )
    ]
    helper_source = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    helper_module = load_script_module(
        EVIDENCE_VALIDATION_HELPER, "sorafs_evidence_validation_scalar_contract"
    )
    errors: list[str] = []
    appeal = read(SCRIPTS_DIR / "check_sorafs_appeal_finance_rollout_evidence.py")
    gateway = read(SCRIPTS_DIR / "check_sorafs_gateway_compliance_rollout_evidence.py")
    governance = read(SCRIPTS_DIR / "check_sorafs_governance_dag_rollout_evidence.py")
    orderbook = read(SCRIPTS_DIR / "check_sorafs_orderbook_rollout_evidence.py")
    pdp = read(SCRIPTS_DIR / "check_sorafs_pdp_rollout_evidence.py")
    ai_prescreen = read(SCRIPTS_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py")
    hedging = read(SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py")
    moderation = read(SCRIPTS_DIR / "check_sorafs_moderation_panel_rollout_evidence.py")
    pop_credentials = read(
        SCRIPTS_DIR / "check_sorafs_pop_credentials_rollout_evidence.py"
    )
    por = read(SCRIPTS_DIR / "check_sorafs_por_rollout_evidence.py")
    potr = read(SCRIPTS_DIR / "check_sorafs_potr_rollout_evidence.py")
    reference_sdk = read(SCRIPTS_DIR / "check_sorafs_reference_sdk_release_evidence.py")
    repair = read(SCRIPTS_DIR / "check_sorafs_repair_rollout_evidence.py")
    reserve = read(SCRIPTS_DIR / "check_sorafs_reserve_rent_rollout_evidence.py")
    transparency = read(SCRIPTS_DIR / "check_sorafs_transparency_rollout_evidence.py")

    assert "def require_string_value_in" in helper_source
    assert "def record_string_value_binding_errors" in helper_source
    assert "def validate_bound_evidence_digest_references" in helper_source
    assert "def _fingerprint_field_name" in helper_source
    assert "def _digest_field_by_kind_labels" in helper_source
    assert "def _validate_kind_error_template" in helper_source
    assert "def _format_kind_error_template" in helper_source
    assert "def _evidence_kind_artifact_pairs" in helper_source
    assert "def _required_evidence_has_any_kind_or_error" in helper_source
    assert "validation required evidence kinds" in helper_source
    assert "validation missing-anchor required evidence kinds" in helper_source
    assert 'label="missing-anchor evidence artifacts"' in helper_source
    assert "validation binding error template" in helper_source
    assert "validation missing-anchor error template" in helper_source
    assert "validation missing-anchor summary error" in helper_source
    assert "validation digest field" in helper_source
    assert "validation digest field map must be a mapping" in helper_source
    assert "must use only plain kind_name formatter fields" in helper_source
    assert "must be a valid formatter template" in helper_source
    assert "validation evidence kind" in helper_source
    assert ".format(kind_name=kind_name)" not in helper_source
    assert "allowed: Any" in helper_source
    assert "isinstance(allowed, (str, bytes, bytearray, Mapping))" in helper_source
    assert "not isinstance(allowed_value, str) or not allowed_value" in helper_source
    assert "not value" in helper_source
    assert "value_label = _require_validation_label(" in helper_source
    assert "normalized = value_label.lower()" in helper_source
    assert "allowed_values: set[str] = set()" in helper_source
    assert "allowed_values.add(allowed_label.lower())" in helper_source
    assert "validation message" in helper_source
    assert "validation value" in helper_source
    assert "validation allowed value" in helper_source
    assert "record_artifact_error(artifact, error, errors)" in helper_source
    assert (
        "test_record_string_value_binding_errors_rejects_malformed_artifact_rows"
        in helper_test
    )
    assert (
        "test_require_string_value_in_rejects_malformed_labels_before_binding"
        in helper_test
    )
    assert (
        "test_require_string_value_in_rejects_malformed_value_before_allowed_container"
        in helper_test
    )
    assert "test_require_string_value_in_normalizes_allowed_values" in helper_test
    assert (
        "test_validate_bound_evidence_digest_references_rejects_malformed_templates_before_mutation"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_digest_references_rejects_malformed_missing_anchor_messages"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_digest_references_rejects_malformed_kind_labels_before_mutation"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_digest_references_rejects_malformed_field_selectors"
        in helper_test
    )
    assert (
        "test_validate_bound_evidence_digest_references_rejects_malformed_required_kind_sets_before_missing_anchor_mutation"
        in helper_test
    )
    assert (
        helper_module.require_string_value_in(
            "AA", {"aa"}, errors, message="missing digest"
        )
        == "aa"
    )
    assert errors == []
    assert (
        helper_module.require_string_value_in(
            "aa", {"AA"}, errors, message="missing digest"
        )
        == "aa"
    )
    assert errors == []
    assert (
        helper_module.require_string_value_in(
            None, {"aa"}, errors, message="missing digest"
        )
        is None
    )
    assert errors == ["missing digest"]
    assert (
        helper_module.require_string_value_in(
            "", {""}, errors, message="missing digest"
        )
        is None
    )
    assert errors == ["missing digest", "missing digest"]
    assert (
        helper_module.require_string_value_in(
            "AA", "aabb", errors, message="missing digest"
        )
        is None
    )
    assert errors == ["missing digest", "missing digest", "missing digest"]
    assert (
        helper_module.require_string_value_in(
            "AA", {"aa": True}, errors, message="missing digest"
        )
        is None
    )
    assert errors == [
        "missing digest",
        "missing digest",
        "missing digest",
        "missing digest",
    ]
    for source in (
        ai_prescreen,
        appeal,
        gateway,
        governance,
        moderation,
        orderbook,
        pdp,
        pop_credentials,
        por,
        potr,
        reference_sdk,
        repair,
        reserve,
        transparency,
    ):
        assert "validate_bound_evidence_digest_references(" in source
        assert "record_string_value_binding_errors(" not in source
        assert "record_artifact_error(" not in source
        assert "required_evidence_has_any_kind(" not in source
        assert "require_string_value_in," not in source
        assert "require_string_value_in(" not in source
        assert "binding_errors: list[str] = []" not in source
    assert "valid_public_head_cids,\n                binding_errors" not in governance
    assert "valid_bundle_digests,\n                binding_errors" not in gateway
    assert "valid_config_digests,\n                binding_errors" not in appeal
    assert "valid_contract_digests,\n                binding_errors" not in orderbook
    assert "valid_proof_summary_digests,\n                binding_errors" not in pdp
    for source in (hedging,):
        assert "record_string_value_binding_errors(" in source
        assert "require_string_value_in," not in source
        assert "require_string_value_in(" not in source
        assert "binding_errors: list[str] = []" not in source
    assert "valid_workflow_digests,\n                binding_errors" not in ai_prescreen
    assert "valid_reference_decision_ids,\n                binding_errors" not in hedging
    assert "valid_case_digests,\n                binding_errors" not in moderation
    assert "allowed, binding_errors" not in pop_credentials
    assert "valid_root_digests,\n                binding_errors" not in pop_credentials
    assert "valid_revocation_digests,\n                binding_errors" not in pop_credentials
    assert "valid_seed_replay_digests,\n                binding_errors" not in por
    assert "valid_receipt_summary_digests,\n                binding_errors" not in potr
    assert (
        "valid_release_manifest_digests,\n                binding_errors"
        not in reference_sdk
    )
    assert "valid_roster_digests,\n                binding_errors" not in repair
    assert "valid_failure_bundle_digests,\n                binding_errors" not in repair
    assert "valid_policy_digests,\n                binding_errors" not in reserve
    assert "valid_source_batch_digests,\n                binding_errors" not in transparency
    assert "valid_cycle_digests,\n                binding_errors" not in transparency
    assert missing == []
    assert local_scalar_binding_predicates == []


def test_rollout_checkers_use_shared_status_in_validation() -> None:
    expected_names = status_in_validation_checkers()
    missing_import = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_status_in," not in read(path)
    ]
    local_status_membership_checks = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and (
            'payload.get("status") not in' in read(path)
            or "status not in (None," in read(path)
        )
    ]
    helper_source = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_status_in" in helper_source
    assert "allowed statuses must be a sequence of strings" in helper_source
    assert "allowed_values: list[str]" in helper_source
    assert "validation allowed status" in helper_source
    assert "value_label = _require_validation_label(" in helper_source
    assert "return value_label" in helper_source
    assert "test_require_status_in_rejects_malformed_labels_before_lookup" in helper_test
    assert (
        "test_require_status_in_rejects_malformed_values_before_membership"
        in helper_test
    )
    assert missing_import == []
    assert local_status_membership_checks == []


def test_rollout_checkers_use_shared_http_status_validation() -> None:
    expected_names = http_status_validation_checkers()
    missing = [
        path.name
        for path in CHECKERS
        if path.name in expected_names
        and (
            "require_2xx_status," not in read(path)
            or "require_2xx_status(" not in read(path)
        )
    ]
    local_status_errors = [
        path.name for path in CHECKERS if "must be a 2xx status" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    ai_prescreen = read(SCRIPTS_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py")
    transparency = read(SCRIPTS_DIR / "check_sorafs_transparency_rollout_evidence.py")

    assert "def require_2xx_status" in helper
    assert "200 <= value < 300" in helper
    assert "diagnostic_label" in helper
    assert "must be a 2xx status" in helper
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    assert "test_require_2xx_status_rejects_malformed_labels_before_lookup" in helper_test
    assert 'path=f"{field}[{index}].{status_field}"' in ai_prescreen
    assert 'path=f"{field}[{index}].{status_field}"' in transparency
    assert missing == []
    assert local_status_errors == []


def test_rollout_checkers_use_shared_route_probe_object_array_validation() -> None:
    expected_names = route_probe_object_array_checkers()
    missing = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_object_array," not in read(path)
    ]
    local_route_probe_arrays = [
        path.name
        for path in CHECKERS
        if "routes = payload.get(\"routes\")" in read(path)
        or "probes = payload.get(field)" in read(path)
        or "routes must be a non-empty array" in read(path)
        or 'f"{field} must be a non-empty array"' in read(path)
        or "for index, route in enumerate(routes)" in read(path)
        or "for index, probe in enumerate(probes)" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    ai_prescreen = read(SCRIPTS_DIR / "check_sorafs_ai_prescreen_rollout_evidence.py")
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_object_array" in helper
    assert "must be a non-empty array" in helper
    assert "require_object(item" in helper
    assert "has_malformed_item = False" in helper
    assert "if has_malformed_item:\n        return []" in helper
    assert "test_require_object_array_rejects_malformed_payloads" in helper_test
    assert (
        "test_require_object_array_returns_empty_for_mixed_malformed_items"
        in helper_test
    )
    assert (
        "test_require_object_array_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert 'route_records = require_object_array(payload, "routes", errors)' in ai_prescreen
    assert 'probe_records = require_object_array(payload, field, errors)' in ai_prescreen
    assert missing == []
    assert local_route_probe_arrays == []


def test_rollout_checkers_use_shared_generic_object_array_validation() -> None:
    expected_names = generic_object_array_checkers()
    missing = [
        path.name
        for path in CHECKERS
        if path.name in expected_names and "require_object_array," not in read(path)
    ]
    local_generic_arrays = [
        path.name
        for path in CHECKERS
        if "artifacts must be a non-empty array" in read(path)
        or "streams must be a non-empty array" in read(path)
        or "events must be a non-empty array" in read(path)
        or "for index, artifact in enumerate(artifacts)" in read(path)
        or "for index, stream in enumerate(streams)" in read(path)
        or 'events = payload.get("events")' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    orderbook = read(SCRIPTS_DIR / "check_sorafs_orderbook_rollout_evidence.py")
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def require_object_array" in helper
    assert "test_require_object_array_rejects_malformed_payloads" in helper_test
    assert (
        "test_require_object_array_returns_empty_for_mixed_malformed_items"
        in helper_test
    )
    assert (
        "test_require_object_array_rejects_malformed_labels_before_lookup"
        in helper_test
    )
    assert 'stream_records = require_object_array(payload, "streams", errors)' in orderbook
    assert 'artifact_records = require_object_array(payload, "artifacts", errors)' in orderbook
    assert 'event_records = require_object_array(payload, "events", errors)' in reputation
    assert missing == []
    assert local_generic_arrays == []


def test_rollout_checkers_use_shared_artifact_error_recording() -> None:
    missing = [
        path.name
        for path in artifact_error_checkers()
        if (
            (
                path.name == "check_sorafs_reputation_rollout_evidence.py"
                and (
                    "sorafs_evidence_validation" not in read(path)
                    or "validate_snapshot_bound_evidence_artifacts," not in read(path)
                    or "validate_snapshot_bound_evidence_artifacts(" not in read(path)
                )
            )
            or (
                path.name in scalar_bound_digest_reference_checkers()
                and (
                    "sorafs_evidence_validation" not in read(path)
                    or "validate_bound_evidence_digest_references," not in read(path)
                    or "validate_bound_evidence_digest_references(" not in read(path)
                )
            )
            or (
                path.name in tuple_bound_reference_checkers()
                and (
                    "sorafs_evidence_validation" not in read(path)
                    or "validate_bound_evidence_tuple_references," not in read(path)
                    or "validate_bound_evidence_tuple_references(" not in read(path)
                )
            )
            or (
                path.name != "check_sorafs_reputation_rollout_evidence.py"
                and path.name not in scalar_bound_digest_reference_checkers()
                and path.name not in tuple_bound_reference_checkers()
                and (
                    "sorafs_checker_preflight" not in read(path)
                    or "record_artifact_error," not in read(path)
                    or "record_artifact_error(" not in read(path)
                )
            )
            or "def record_artifact_error(" in read(path)
            or 'artifact_errors = artifact.get("errors")' in read(path)
        )
    ]
    helper = read(CHECKER_PREFLIGHT)
    evidence_helper = read(EVIDENCE_VALIDATION_HELPER)
    preflight_test = read(SCRIPTS_DIR / "tests" / "sorafs_checker_preflight_test.py")

    assert "def artifact_path_label" in helper
    assert "artifact: Any" in helper
    assert "if not isinstance(artifact, Mapping)" in helper
    assert "artifact_path_label(artifact)" in helper
    assert "def _checker_artifact_error_message" in helper
    assert "label=\"artifact error\"" in helper
    assert "label=\"artifact summary error\"" in helper
    assert "must be a non-empty canonical string" in helper
    assert "artifact path label must be a non-empty canonical string" in helper
    assert "def _recordable_artifact_path_label" in helper
    assert "def _artifact_error_bucket_is_canonical" in helper
    assert 'label="artifact existing error"' in helper
    assert "def record_artifact_error" in helper
    assert "summary_error_list = _require_error_list(summary_errors)" in helper
    assert "_checker_artifact_error_message(" in helper
    assert "if not isinstance(artifact, dict)" in helper
    assert "artifact_errors = artifact.get(\"errors\")" in helper
    assert "_artifact_error_bucket_is_canonical(" in helper
    assert "summary_error: str | None = None" in helper
    assert "summary_error_list.append" in helper
    assert "test_record_artifact_error_rejects_malformed_artifact_rows" in preflight_test
    assert (
        "test_record_artifact_error_rebuilds_dirty_artifact_error_buckets"
        in preflight_test
    )
    assert (
        "test_record_artifact_error_rejects_malformed_summary_error_container"
        in preflight_test
    )
    assert (
        "test_record_artifact_error_rejects_malformed_error_messages"
        in preflight_test
    )
    assert (
        "test_record_artifact_error_rejects_malformed_summary_error_messages"
        in preflight_test
    )
    assert (
        "test_record_artifact_error_rejects_malformed_artifact_path_before_mutation"
        in preflight_test
    )
    assert "artifact['path']" not in helper
    assert "def validate_snapshot_bound_evidence_artifacts" in evidence_helper
    assert "def validate_bound_evidence_digest_references" in evidence_helper
    assert "def validate_bound_evidence_tuple_references" in evidence_helper
    assert "record_artifact_error(" in evidence_helper
    assert missing == []


def test_rollout_checkers_use_shared_required_summary_builder() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if "build_required_evidence_summary," not in read(path)
        or "build_required_evidence_summary(" not in read(path)
        or "present = bool(artifacts)" in read(path)
        or 'all(artifact["valid"] for artifact in artifacts)' in read(path)
        or 'errors.append(f"missing required {name}' in read(path)
        or 'evidence has invalid artifact(s)"' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def _required_evidence_kind_list" in helper
    assert "label_name=\"required evidence kind\"" in helper
    assert "def _evidence_artifact_rows" in helper
    assert "def build_required_evidence_summary" in helper
    assert "artifact_buckets = artifacts_by_kind" in helper
    assert "schema_map = schema_by_kind" in helper
    assert "artifacts by kind must be a mapping" in helper
    assert "schema by kind must be a mapping" in helper
    assert "raw_artifacts = artifact_buckets.get(name, [])" in helper
    assert "_evidence_artifact_rows(raw_artifacts)" in helper
    assert "required evidence kinds must not be empty" in helper
    assert "def _duplicate_required_evidence_kind_names" in helper
    assert "duplicate_kind_names = _duplicate_required_evidence_kind_names(" in helper
    assert "required evidence kinds must not contain duplicates" in helper
    assert "malformed_artifact_bucket = name in artifact_buckets" in helper
    assert "required `{name}` artifacts must be a sequence" in helper
    assert "required {name} {evidence_label} artifacts must be a sequence" in helper
    assert "required `{name}` artifacts must be a sequence of artifact objects" in helper
    assert (
        "required {name} {evidence_label} artifacts must be a sequence "
        in helper
    )
    assert "of artifact objects" in helper
    assert "for kind_name, artifacts in artifact_buckets.items()" in helper
    assert "artifact_rows = _evidence_artifact_rows(artifacts)" in helper
    assert "if artifact_rows is None:" in helper
    assert "schema_map.get(name)" in helper
    assert "schema_label = _require_validation_label(" in helper
    assert 'label_name=f"required `{name}` schema"' in helper
    assert "required `{name}` schema must be configured" in helper
    assert "required evidence kinds must be a sequence of strings" in helper
    assert "missing required {name} {evidence_label} evidence" in helper
    assert "{name} {evidence_label} evidence has invalid artifact(s)" in helper
    assert "evidence_artifact_is_valid(artifact)" in helper
    assert (
        "test_build_required_evidence_summary_rejects_malformed_artifact_rows"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "sorafs.control.v1\\nbad"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert "schema_by_kind[name]" not in helper
    assert 'all(artifact["valid"] for artifact in artifacts)' not in helper
    assert missing == []


def test_rollout_checkers_use_shared_required_kind_membership() -> None:
    expected_any = {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_governance_dag_rollout_evidence.py",
        "check_sorafs_hedging_rollout_evidence.py",
        "check_sorafs_moderation_panel_rollout_evidence.py",
        "check_sorafs_orderbook_rollout_evidence.py",
        "check_sorafs_pdp_rollout_evidence.py",
        "check_sorafs_pop_credentials_rollout_evidence.py",
        "check_sorafs_por_rollout_evidence.py",
        "check_sorafs_potr_rollout_evidence.py",
        "check_sorafs_reference_sdk_release_evidence.py",
        "check_sorafs_repair_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
    }
    expected_all = {"check_sorafs_hedging_rollout_evidence.py"}
    missing_any = [
        path.name
        for path in CHECKERS
        if path.name in expected_any
        and path.name not in scalar_bound_digest_reference_checkers()
        and (
            "required_evidence_has_any_kind," not in read(path)
            or "required_evidence_has_any_kind(" not in read(path)
        )
    ]
    missing_bound_reference = [
        path.name
        for path in CHECKERS
        if path.name in expected_any
        and path.name in scalar_bound_digest_reference_checkers()
        and (
            "validate_bound_evidence_digest_references," not in read(path)
            or "validate_bound_evidence_digest_references(" not in read(path)
        )
    ]
    missing_all = [
        path.name
        for path in CHECKERS
        if path.name in expected_all
        and (
            "required_evidence_has_all_kinds," not in read(path)
            or "required_evidence_has_all_kinds(\n" not in read(path)
        )
    ]
    local_required_kind_predicates = [
        path.name
        for path in CHECKERS
        if "any(kind in required_kinds for kind in" in read(path)
    ]
    direct_required_kind_predicates = [
        path.name
        for path in CHECKERS
        if re.search(r"[\"'][a-z0-9_]+[\"'] in required_kinds", read(path))
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")

    assert "def _evidence_kind_name_set" in helper
    assert "kinds: Any" in helper
    assert "not isinstance(" in helper
    assert "kinds, Collection" in helper
    assert "isinstance(kinds, (str, bytes, bytearray, Mapping))" in helper
    assert "label_name=\"validation evidence kind\"" in helper
    assert "def required_evidence_has_any_kind" in helper
    assert "def required_evidence_has_all_kinds" in helper
    assert "def validate_bound_evidence_digest_references" in helper
    assert "_evidence_kind_name_set(required_kinds)" in helper
    assert "_evidence_kind_name_set(candidate_kinds)" in helper
    assert (
        "any(kind in required_kind_names for kind in candidate_kind_names)" in helper
    )
    assert (
        "all(kind in required_kind_names for kind in candidate_kind_names)" in helper
    )
    assert "required_evidence_has_any_kind(" in helper
    assert (
        "test_required_evidence_has_any_kind_fails_closed_on_malformed_kinds"
        in helper_test
    )
    assert (
        "test_required_evidence_has_all_kinds_fails_closed_on_malformed_kinds"
        in helper_test
    )
    assert missing_any == []
    assert missing_bound_reference == []
    assert missing_all == []
    assert local_required_kind_predicates == []
    assert direct_required_kind_predicates == []


def test_reputation_uses_shared_required_row_invalidation() -> None:
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def mark_required_evidence_invalid" in helper
    assert "def mark_required_evidence_invalid_if_present" in helper
    assert "def mark_required_evidence_summary_invalid" in helper
    assert "UNKNOWN_REQUIRED_EVIDENCE_KIND" in helper
    assert "validation required evidence kind" in helper
    assert "validation required summary error" in helper
    assert "def build_kinded_evidence_artifact" in helper
    assert "def finalize_custom_required_evidence_rows" in helper
    assert "def record_custom_required_evidence_artifact" in helper
    assert "required `{kind_label}` row must be an object" in helper
    assert "required `{kind_label}` errors must be a list" in helper
    assert "required `{kind_label}` artifact must be an object" in helper
    assert "required `{kind_label}` artifacts must be a sequence" in helper
    assert (
        "required `{kind_label}` artifacts must be a sequence of artifact objects"
        in helper
    )
    assert "existing_artifact" in helper
    assert "validation evidence label" in helper
    assert "artifact errors must be a sequence of canonical strings" in helper
    assert "_evidence_artifact_rows(artifacts)" in helper
    assert (
        "test_finalize_custom_required_evidence_rows_rejects_malformed_evidence_label"
        in helper_test
    )
    assert (
        "test_finalize_custom_required_evidence_rows_rejects_malformed_artifact_rows"
        in helper_test
    )
    assert (
        "test_finalize_custom_required_evidence_rows_rejects_malformed_kind_labels"
        in helper_test
    )
    assert (
        "test_record_custom_required_evidence_artifact_rejects_error_shape_drift"
        in helper_test
    )
    assert (
        "test_record_custom_required_evidence_artifact_rejects_malformed_artifact_rows_before_append"
        in helper_test
    )
    assert (
        "test_record_custom_required_evidence_artifact_rejects_malformed_existing_artifacts_before_append"
        in helper_test
    )
    assert (
        "test_record_custom_required_evidence_artifact_rejects_malformed_row_before_append"
        in helper_test
    )
    assert (
        "test_record_custom_required_evidence_artifact_rejects_malformed_error_text"
        in helper_test
    )
    assert (
        "test_record_custom_required_evidence_artifact_rejects_malformed_kind_label"
        in helper_test
    )
    assert "required.pop(kind_name)" in helper
    assert "elif kind_label not in required" in helper
    assert (
        "test_mark_required_evidence_invalid_rejects_malformed_kind_labels"
        in helper_test
    )
    assert (
        "test_mark_required_evidence_invalid_rejects_unhashable_kind_labels"
        in helper_test
    )
    assert (
        "test_mark_required_evidence_summary_invalid_rejects_malformed_error_labels"
        in helper_test
    )
    assert "isinstance(kind_name, Hashable)" in helper
    assert "return False" in helper
    assert "return True" in helper
    assert "def required_evidence_summary_is_valid" in helper
    assert "required: Any" in helper
    assert "not isinstance(required, Mapping)" in helper
    assert "isinstance(row, Mapping)" in helper
    assert 'row.get("valid") is not True' in helper
    assert 'row.get("errors")' in helper
    assert "row_errors != []" in helper
    assert '_evidence_artifact_rows(row.get("artifacts"))' in helper
    assert "if not artifacts:" in helper
    assert "recognized_evidence_artifacts_are_valid(artifacts)" in helper
    assert "def required_evidence_has_any_kind" in helper
    assert "def hashable_evidence_values" in helper
    assert "def _hashable_evidence_value" in helper
    assert "def _evidence_value_items" in helper
    assert "values: Any" in helper
    assert "isinstance(values, (str, bytes, bytearray, Mapping))" in helper
    assert "_evidence_value_items(values)" in helper
    assert "not value or isinstance(value, bool)" in helper
    assert 'label_name="validation evidence value"' in helper
    assert "def missing_required_evidence_values" in helper
    assert "required_values: Any" in helper
    assert "observed_values: Any" in helper
    assert (
        "test_required_evidence_summary_is_valid_fails_closed_on_malformed_rows"
        in helper_test
    )
    assert (
        "test_hashable_evidence_values_rejects_scalar_or_mapping_containers"
        in helper_test
    )
    assert (
        "test_hashable_evidence_values_rejects_malformed_string_values"
        in helper_test
    )
    assert (
        "test_missing_required_evidence_values_rejects_scalar_containers"
        in helper_test
    )
    assert (
        "test_missing_required_evidence_values_rejects_malformed_observed_strings"
        in helper_test
    )
    assert "def record_missing_required_evidence_value_errors" in helper
    assert "validation missing required evidence message" in helper
    assert (
        "test_record_missing_required_evidence_value_errors_rejects_malformed_labels"
        in helper_test
    )
    assert "def required_or_observed_evidence_values_are_present" in helper
    assert "def _evidence_values_are_clean_and_present" in helper
    assert "_evidence_values_are_clean_and_present(\n        required_values" in helper
    assert "_evidence_values_are_clean_and_present(observed_values)" in helper
    assert "has_present_value = False" in helper
    assert "def record_missing_required_or_observed_evidence_error" in helper
    assert "validation missing evidence error" in helper
    assert (
        "test_record_missing_required_or_observed_evidence_error_rejects_malformed_labels"
        in helper_test
    )
    assert (
        "test_record_missing_required_or_observed_evidence_error_marks_mixed_malformed_values"
        in helper_test
    )
    assert "def distinct_evidence_values_are_consistent" in helper
    assert "def distinct_evidence_values_are_consistent(values: Any)" in helper
    assert "not isinstance(\n        values, Collection" in helper
    assert "normalized_values: list[Hashable]" in helper
    assert "_hashable_evidence_value(value)" in helper
    assert (
        "test_distinct_evidence_values_are_consistent_fails_closed_on_malformed_values"
        in helper_test
    )
    assert (
        "test_record_inconsistent_evidence_values_error_marks_malformed_values"
        in helper_test
    )
    assert "def record_inconsistent_evidence_values_error" in helper
    assert "validation inconsistent evidence error" in helper
    assert (
        "test_record_inconsistent_evidence_values_error_rejects_malformed_labels"
        in helper_test
    )
    assert "def record_consistent_evidence_value" in helper
    assert "isinstance(value, str)" in helper
    assert "validation evidence context" in helper
    assert "validation evidence key" in helper
    assert "validation evidence value" in helper
    assert 'errors.append(f"{context_label}.{key_label} must be a string")' in helper
    assert (
        "test_record_consistent_evidence_value_reports_malformed_values"
        in helper_test
    )
    assert (
        "test_record_consistent_evidence_value_rejects_malformed_labels"
        in helper_test
    )
    assert (
        "test_record_consistent_deployment_context_rejects_malformed_labels"
        in helper_test
    )
    assert "def record_observed_evidence_value" in helper
    assert "Hashable" in helper
    assert "def record_snapshot_bound_evidence_artifact" in helper
    assert "artifact valid flag must be a boolean" in helper
    assert 'label_name="snapshot binding evidence kind"' in helper
    assert "if kind_label is None:" in helper
    assert (
        "snapshot binding kind containers must be sequences of canonical strings"
        in helper
    )
    assert (
        "snapshot binding pairs must be a sequence of canonical string pairs"
        in helper
    )
    assert (
        "snapshot bound artifacts must be a sequence of artifact objects" in helper
    )
    assert (
        "test_record_snapshot_bound_evidence_artifact_rejects_malformed_valid_flags"
        in helper_test
    )
    assert (
        "test_record_snapshot_bound_evidence_artifact_rejects_malformed_kind_containers"
        in helper_test
    )
    assert (
        "test_record_snapshot_bound_evidence_artifact_rejects_malformed_kind_labels"
        in helper_test
    )
    assert (
        "test_record_snapshot_bound_evidence_artifact_rejects_malformed_anchor_values"
        in helper_test
    )
    assert "_snapshot_binding_pair_set" in helper
    assert "_snapshot_bound_artifact_rows" in helper
    assert "_evidence_kind_name_set(anchor_kinds)" in helper
    assert "_evidence_kind_name_set(bound_kinds)" in helper
    assert 'label_name="snapshot_id_hex"' in helper
    assert 'label_name="merkle_root_hex"' in helper
    assert "if snapshot_label is None or merkle_label is None:" in helper
    assert "for error in anchor_errors:" in helper
    assert "snapshot_label.lower()" in helper
    assert "merkle_label.lower()" in helper
    assert "def validate_snapshot_bound_evidence_artifacts" in helper
    assert "record_artifact_error(" in helper
    assert "require_string_tuple_in(" in helper
    assert (
        "test_validate_snapshot_bound_evidence_artifacts_rejects_malformed_kind_containers"
        in helper_test
    )
    assert (
        "test_validate_snapshot_bound_evidence_artifacts_rejects_malformed_binding_containers"
        in helper_test
    )
    assert (
        "test_validate_snapshot_bound_evidence_artifacts_rejects_malformed_bound_artifacts"
        in helper_test
    )
    assert (
        "test_validate_snapshot_bound_evidence_artifacts_rejects_malformed_labels"
        in helper_test
    )
    assert "mark_required_evidence_invalid_if_present(" in helper
    assert "evidence_artifact_fingerprint(" in helper
    assert "evidence_artifact_kind(" in helper
    assert "required[kind_label] = {\"valid\": False, \"errors\": [], \"artifacts\": []}" in helper
    assert "build_kinded_evidence_artifact," in reputation
    assert "finalize_custom_required_evidence_rows," in reputation
    assert "record_consistent_evidence_value," in reputation
    assert "record_custom_required_evidence_artifact," in reputation
    assert "record_inconsistent_evidence_values_error," in reputation
    assert "record_missing_required_evidence_value_errors," in reputation
    assert "record_missing_required_or_observed_evidence_error," in reputation
    assert "record_observed_evidence_value," in reputation
    assert "record_snapshot_bound_evidence_artifact," in reputation
    assert "validate_snapshot_bound_evidence_artifacts," in reputation
    assert "required_evidence_summary_is_valid," in reputation
    assert "record_custom_required_evidence_artifact(" in reputation
    assert "build_kinded_evidence_artifact(\n" in reputation
    assert (
        'finalize_custom_required_evidence_rows(required, evidence_label="evidence")'
        in reputation
    )
    assert 'mark_required_evidence_invalid(required, "provider")' not in reputation
    assert 'mark_required_evidence_invalid(required, "latest")' not in reputation
    assert "record_missing_required_evidence_value_errors(\n" in reputation
    assert "record_missing_required_or_observed_evidence_error(\n" in reputation
    assert "record_inconsistent_evidence_values_error(\n" in reputation
    assert "if artifact_kind in required" not in reputation
    assert "if provider_id not in provider_ids" not in reputation
    assert "for provider_id in required_providers" not in reputation
    assert "missing_provider_ids = " not in reputation
    assert "for provider_id in missing_provider_ids" not in reputation
    assert "missing_required_evidence_values(" not in reputation
    assert "if not required_or_observed_evidence_values_are_present(" not in reputation
    assert "required_or_observed_evidence_values_are_present(" not in reputation
    assert "not required_providers and not provider_ids" not in reputation
    assert "if provider_id:" not in reputation
    assert "provider_ids.add(provider_id)" not in reputation
    assert "if provider_count:" not in reputation
    assert "provider_counts.add(provider_count)" not in reputation
    assert "artifact_fingerprint(payload, FINGERPRINT_FIELDS)" not in reputation
    assert "fingerprint[\"snapshot_id_hex\"]" not in reputation
    assert "fingerprint[\"merkle_root_hex\"]" not in reputation
    assert "valid = not errors" not in reputation
    assert "record = {" not in reputation
    assert '"path": str(evidence.path)' not in reputation
    assert '"sha256": digest' not in reputation
    assert "len(provider_counts) > 1" not in reputation
    assert "distinct_evidence_values_are_consistent(provider_counts)" not in reputation
    assert "mark_required_evidence_summary_invalid(required)" not in reputation
    assert "record_consistent_evidence_value(\n" in reputation
    assert reputation.count(
        "record_observed_evidence_value(provider_ids, provider_id)"
    ) == 2
    assert "record_observed_evidence_value(provider_counts, provider_count)" in reputation
    assert "def append_match(" not in reputation
    assert "append_match(" not in reputation
    assert "previous = values.get(key)" not in reputation
    assert "does not match `{previous}`" not in reputation
    assert "record_snapshot_bound_evidence_artifact(\n" in reputation
    assert "validate_snapshot_bound_evidence_artifacts(\n" in reputation
    assert "valid_snapshot_bindings.add(" not in reputation
    assert "snapshot_bound_artifacts.append(record)" not in reputation
    assert "evidence.kind in SNAPSHOT_ANCHOR_KINDS and snapshot_id and merkle_root" not in reputation
    assert "if valid_snapshot_bindings:" not in reputation
    assert "for artifact in snapshot_bound_artifacts:" not in reputation
    assert "binding_errors: list[str] = []" not in reputation
    assert "record_artifact_error(" not in reputation
    assert "require_string_tuple_in(" not in reputation
    assert "required_evidence_has_any_kind," not in reputation
    assert "mark_required_evidence_invalid_if_present," not in reputation
    assert "mark_required_evidence_invalid(required, artifact_kind)" not in reputation
    assert (
        "any(\n"
        "        kind in required_kinds for kind in SNAPSHOT_BOUND_KINDS\n"
        "    )"
        not in reputation
    )
    assert "mark_required_evidence_summary_invalid(required)" not in reputation
    assert "for kind, record in required.items()" not in reputation
    assert "for record in required.values()" not in reputation
    assert 'record["valid"] = False' not in reputation
    assert "required[evidence.kind]" not in reputation
    assert 'record["artifacts"]' not in reputation
    assert 'record["errors"].append(f"missing required' not in reputation
    assert 'record["valid"] = all(' not in reputation
    assert "required_evidence_summary_is_valid(required)" in reputation
    assert "required.setdefault(" not in reputation
    assert "required[artifact_kind]" not in reputation
    assert 'all(record["valid"] for record in required.values())' not in reputation
    assert "summary_errors: list[str] = []" not in reputation


def test_rollout_checkers_use_shared_required_kind_names() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if "required_evidence_kind_names," not in read(path)
        or (
            '"required_kinds": required_evidence_kind_names(required_kinds)'
            not in read(path)
        )
        or '"required_kinds": list(required_kinds)' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def _required_evidence_kind_list" in helper
    assert "def _duplicate_required_evidence_kind_names" in helper
    assert "def required_evidence_kind_names" in helper
    assert "_required_evidence_kind_list(required_kinds)" in helper
    assert "not names" in helper
    assert "_duplicate_required_evidence_kind_names(names)" in helper
    assert "return list(required_kinds)" not in helper
    assert missing == []


def test_rollout_checkers_use_shared_evidence_schema_map() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if "evidence_schema_by_kind," not in read(path)
        or "evidence_schema_by_kind(KIND_BY_NAME)" not in read(path)
        or "{name: kind.schema for name, kind in KIND_BY_NAME.items()}" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def evidence_schema_by_kind" in helper
    assert "kind_by_name: Any" in helper
    assert "not isinstance(kind_by_name, Mapping)" in helper
    assert "getattr(kind, \"schema\", None)" in helper
    assert "label_name=\"evidence kind\"" in helper
    assert "label_name=\"evidence schema\"" in helper
    assert missing == []


def test_rollout_checkers_use_shared_artifact_bucket_initializer() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if "init_evidence_artifact_buckets," not in read(path)
        or (
            "artifacts_by_kind = init_evidence_artifact_buckets(DEFAULT_REQUIRED_KINDS)"
            not in read(path)
        )
        or "name: [] for name in DEFAULT_REQUIRED_KINDS" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def init_evidence_artifact_buckets" in helper
    assert "evidence_kind_names: Any" in helper
    assert "_required_evidence_kind_list(evidence_kind_names)" in helper
    assert "not names" in helper
    assert "_duplicate_required_evidence_kind_names(names)" in helper
    assert "return {name: [] for name in names}" in helper
    assert missing == []


def test_rollout_checkers_use_shared_artifact_recorder() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if "record_evidence_artifact," not in read(path)
        or (
            "record_evidence_artifact(artifacts_by_kind, kind_name, artifact, errors)"
            not in read(path)
        )
        or "artifacts_by_kind[kind_name].append(artifact)" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)

    assert "def record_evidence_artifact" in helper
    assert "artifacts_by_kind: Any" in helper
    assert "kind_name: Any" in helper
    assert "artifact: Any" in helper
    assert "not isinstance(artifacts_by_kind, Mapping)" in helper
    assert "recognized evidence artifacts by kind must be a mapping" in helper
    assert "not isinstance(kind_name, str) or not kind_name" in helper
    assert "recognized evidence kind must be a non-empty string" in helper
    assert "label_name=\"recognized evidence kind\"" in helper
    assert "not isinstance(artifact, dict)" in helper
    assert "evidence artifact must be an object" in helper
    assert "artifacts_by_kind.get(kind_label)" in helper
    assert "has no artifact bucket" in helper
    assert "artifact bucket must be a sequence of artifact objects" in helper
    assert "existing_artifact" in helper
    assert (
        "test_record_evidence_artifact_rejects_malformed_existing_bucket_rows"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert "artifacts_by_kind[kind_name].append(artifact)" not in helper
    assert missing == []


def test_rollout_checkers_use_shared_artifact_validity_helper() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if "evidence_artifact_is_valid," not in read(path)
        or "evidence_artifact_is_valid(artifact)" not in read(path)
        or 'artifact["valid"]' in read(path)
        or 'artifact.get("valid") is True' in read(path)
        or "not validation_errors" in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def evidence_artifact_is_valid" in helper
    assert "def recognized_evidence_artifacts_are_valid" in helper
    assert "recognized: Any" in helper
    assert "recognized, (str, bytes, bytearray)" in helper
    assert "if not isinstance(artifact, Mapping):" in helper
    assert (
        "test_evidence_artifact_accessors_reject_non_mapping_without_traceback"
        in helper_test
    )
    assert 'artifact.get("valid") is True' in helper
    assert "finalize_custom_required_evidence_rows," in reputation
    assert "recognized_evidence_artifacts_are_valid," in reputation
    assert "evidence_artifact_is_valid," not in reputation
    assert "evidence_artifact_is_valid(artifact)" not in reputation
    assert "recognized_evidence_artifacts_are_valid(recognized)" in reputation
    assert (
        "all(evidence_artifact_is_valid(artifact) for artifact in recognized)"
        not in reputation
    )
    assert 'artifact["valid"]' not in reputation
    assert missing == []


def test_rollout_checkers_use_shared_artifact_fingerprint_accessor() -> None:
    missing = [
        path.name
        for path in required_summary_checkers()
        if (
            (
                path.name == "check_sorafs_reputation_rollout_evidence.py"
                and (
                    "validate_snapshot_bound_evidence_artifacts," not in read(path)
                    or "validate_snapshot_bound_evidence_artifacts(" not in read(path)
                )
            )
            or (
                path.name != "check_sorafs_reputation_rollout_evidence.py"
                and path.name != "check_sorafs_pop_credentials_rollout_evidence.py"
                and (
                    "evidence_artifact_fingerprint," not in read(path)
                    or "evidence_artifact_fingerprint(artifact)" not in read(path)
                )
            )
            or (
                path.name == "check_sorafs_pop_credentials_rollout_evidence.py"
                and (
                    "evidence_artifact_digest_set," not in read(path)
                    or "evidence_artifact_digest_set(" not in read(path)
                    or "validate_bound_evidence_digest_references," not in read(path)
                    or "validate_bound_evidence_digest_references(" not in read(path)
                )
            )
            or 'artifact["fingerprint"]' in read(path)
        )
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def evidence_artifact_fingerprint" in helper
    assert 'artifact.get("fingerprint")' in helper
    assert "evidence_artifact_fingerprint(artifact)" in helper
    assert "def evidence_artifact_digest_set" in helper
    assert "_fingerprint_field_name(digest_field" in helper
    assert "artifacts, (str, bytes, bytearray, Mapping)" in helper
    assert "not isinstance(artifact, Mapping)" in helper
    assert 'label_name="validation digest value"' in helper
    assert "if digest_label is None:\n            return set()" in helper
    assert "digest_label.lower()" in helper
    assert "isinstance(digest, str) and digest" not in helper
    assert "def _canonical_digest_value_set" in helper
    assert (
        "validation anchor digests must be a collection of canonical strings"
        in helper
    )
    assert "valid_anchor_digest_values = _canonical_digest_value_set" in helper
    assert (
        "test_evidence_artifact_digest_set_rejects_malformed_digest_values"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_evidence_artifact_digest_set_rejects_malformed_artifact_rows"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert "def record_evidence_digest_mismatch_errors" in helper
    assert "test_evidence_artifact_digest_set_rejects_malformed_field_labels" in read(
        SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py"
    )
    assert (
        "test_record_evidence_digest_mismatch_errors_rejects_malformed_field_labels"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_record_evidence_digest_mismatch_errors_rejects_malformed_allowed_digest_inputs_before_mutation"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "evidence digest mismatch artifacts must be a sequence of artifact objects"
        in helper
    )
    assert "artifact_rows: list[dict[str, Any]] = []" in helper
    assert "if not isinstance(artifact, dict):" in helper
    assert (
        "test_record_evidence_digest_mismatch_errors_rejects_malformed_artifact_inputs_before_mutation"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_validate_bound_evidence_digest_references_rejects_malformed_anchor_digests_before_mutation"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert "validate_snapshot_bound_evidence_artifacts," in reputation
    assert "evidence_artifact_fingerprint," not in reputation
    assert 'artifact["fingerprint"]' not in reputation
    assert missing == []


def test_rollout_checkers_use_shared_artifact_kind_accessor() -> None:
    helper = read(EVIDENCE_VALIDATION_HELPER)
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")

    assert "def evidence_artifact_kind" in helper
    assert 'artifact.get("kind")' in helper
    assert "label_name=\"artifact kind\"" in helper
    assert "evidence_artifact_kind(artifact)" in helper
    assert "validate_snapshot_bound_evidence_artifacts," in reputation
    assert "evidence_artifact_kind," not in reputation
    assert 'artifact.get("kind")' not in reputation
    assert 'artifact["kind"]' not in reputation


def test_rollout_checkers_use_shared_artifact_detail_accessor() -> None:
    helper = read(EVIDENCE_VALIDATION_HELPER)
    hedging = read(SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py")
    reserve = read(SCRIPTS_DIR / "check_sorafs_reserve_rent_rollout_evidence.py")
    appeal = read(SCRIPTS_DIR / "check_sorafs_appeal_finance_rollout_evidence.py")

    assert "def evidence_artifact_detail" in helper
    assert "field: Any" in helper
    assert "label_name=\"artifact detail field\"" in helper
    assert "artifact.get(field_label)" in helper
    assert "evidence_artifact_detail," in hedging
    assert 'evidence_artifact_detail(artifact, "cycle")' in hedging
    assert 'cycle = artifact["cycle"]' not in hedging
    assert "evidence_artifact_detail," in reserve
    assert 'evidence_artifact_detail(artifact, "bake")' in reserve
    assert 'valid_provider_bakes.append(provider_bake_fingerprint(payload))' not in reserve
    assert 'valid_multi_peer_runs.append(reconciliation_fingerprint(payload))' not in appeal
    assert "from sorafs_evidence_fingerprint import artifact_fingerprint" in reserve
    assert "from sorafs_evidence_fingerprint import artifact_fingerprint" in appeal
    assert "PROVIDER_BAKE_FIELDS: tuple[str, ...]" in reserve
    assert "RECONCILIATION_RUN_FIELDS: tuple[str, ...]" in appeal
    assert "artifact_fingerprint(payload, PROVIDER_BAKE_FIELDS)" in reserve
    assert "artifact_fingerprint(payload, RECONCILIATION_RUN_FIELDS)" in appeal
    assert "def provider_bake_fingerprint" not in reserve
    assert "def reconciliation_fingerprint" not in appeal


def test_rollout_checkers_use_shared_artifact_schema_accessor() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if 'artifact["schema"]' in read(path)
    ]
    helper = read(EVIDENCE_VALIDATION_HELPER)
    reserve = read(SCRIPTS_DIR / "check_sorafs_reserve_rent_rollout_evidence.py")

    assert "def evidence_artifact_schema" in helper
    assert 'artifact.get("schema")' in helper
    assert "label_name=\"artifact schema\"" in helper
    assert "return \"<unknown>\"" in helper
    assert "evidence_artifact_schema," in reserve
    assert "evidence_artifact_schema(artifact)" in reserve
    assert missing == []


def test_rollout_checkers_use_shared_required_kind_parser() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if "sorafs_required_kinds" not in read(path)
        or "parse_required_evidence_kinds(" not in read(path)
        or "allowed_kinds=KIND_BY_NAME" not in read(path)
        or "default_required=DEFAULT_REQUIRED_KINDS" not in read(path)
        or "def parse_required_kinds(" in read(path)
        or "parse_required_kinds(args.require_kind)" in read(path)
        or "if not name:" in read(path)
        or "if not candidate:" in read(path)
        or "choices=tuple(KIND_BY_NAME)" in read(path)
    ]
    helper = read(REQUIRED_KINDS_HELPER)
    helper_test = read(SCRIPTS_DIR / "tests" / "sorafs_required_kinds_test.py")

    assert "def _validate_allowed_kinds" in helper
    assert "def _validate_default_required" in helper
    assert "def _is_canonical_kind_name" in helper
    assert "value == value.strip()" in helper
    assert "ord(character) < 32 or ord(character) == 127" in helper
    assert "--require-kind values must be a sequence" in helper
    assert "--require-kind values must be strings" in helper
    assert "--require-kind entries must be non-empty canonical strings" in helper
    assert "allowed required evidence kinds must be a mapping" in helper
    assert (
        "allowed required evidence kind names must be non-empty canonical strings"
        in helper
    )
    assert "default required evidence kinds must be a sequence" in helper
    assert (
        "default required evidence kind names must be non-empty canonical strings"
        in helper
    )
    assert "must be non-empty" in helper
    assert "duplicate required evidence kind" in helper
    assert "unknown required evidence kind" in helper
    assert "test_malformed_required_kind_values_fail" in helper_test
    assert "test_non_string_required_kind_value_fails" in helper_test
    assert "test_malformed_required_kind_name_text_fails" in helper_test
    assert "test_malformed_allowed_kind_registry_fails" in helper_test
    assert "test_malformed_default_required_kinds_fail" in helper_test
    assert missing == []


def test_rollout_runners_use_shared_required_kind_parser() -> None:
    missing = [
        path.name
        for path in RUNNERS
        if "--require-kind" in read(path)
        and (
            "sorafs_required_kinds" not in read(path)
            or "parse_required_evidence_kinds(" not in read(path)
            or "allowed_kinds=KIND_BY_NAME" not in read(path)
            or "default_required=DEFAULT_REQUIRED_KINDS" not in read(path)
            or "parse_required_kinds(args.require_kind)" in read(path)
        )
    ]

    assert missing == []


def test_rollout_checkers_fail_closed_on_missing_evidence_directories() -> None:
    missing = [
        path.name
        for path in CHECKERS
        if "discover_evidence_files(" not in read(path)
        or "evidence directory `{directory}` must exist" in read(path)
        or "if not root.is_dir()" in read(path)
    ]
    helper = read(EVIDENCE_PATHS_HELPER)

    assert "must exist and be a directory" in helper
    assert missing == []


def test_rollout_checkers_reject_common_sensitive_fields() -> None:
    missing: dict[str, list[str]] = {}
    for path in CHECKERS:
        source = read(path)
        match = re.search(r"SENSITIVE_KEYS\s*=\s*\{(?P<body>.*?)\n\}", source, re.S)
        if not match:
            missing[path.name] = ["SENSITIVE_KEYS"]
            continue
        body = match.group("body")
        absent = [key for key in COMMON_SENSITIVE_KEYS if f'"{key}"' not in body]
        if absent:
            missing[path.name] = absent

    assert missing == {}


def test_rollout_checkers_use_shared_sensitive_key_normalization() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "validate_standard_evidence_payload(" not in function_source(
            path, "validate_evidence_payload"
        )
        or "SENSITIVE_KEYS" not in function_source(path, "validate_evidence_payload")
    ]
    reputation = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")
    validation_helper = read(EVIDENCE_VALIDATION_HELPER)
    helper = read(SENSITIVITY_HELPER)
    helper_test = read(SENSITIVITY_TEST)

    assert "def normalize_sensitive_key" in helper
    assert "def _require_error_list" in helper
    assert "def _require_diagnostic_string" in helper
    assert "sensitive field errors must be a list of strings" in helper
    assert "sensitive field errors must contain non-empty canonical strings" in helper
    assert "label=\"sensitive field path\"" in helper
    assert (
        "label=\"sensitive field evidence label\""
        in helper
    )
    assert "must be a non-empty canonical string" in helper
    assert "isinstance(sensitive_keys, (str, bytes, bytearray, Mapping))" in helper
    assert "sensitive keys must be a sequence of strings" in helper
    assert "sensitive keys must be non-empty canonical strings" in helper
    assert "key must be a string" in helper
    assert "isinstance(value, Mapping)" in helper
    assert "MAX_SENSITIVE_FIELD_DEPTH" in helper
    assert "nesting exceeds" in helper
    assert "HIGH_RISK_SENSITIVE_KEY_FRAGMENTS" in helper
    assert "PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES" in helper
    assert "def _is_allowed_inclusion_marker_value" in helper
    assert "def _is_payload_free_sensitive_reference" in helper
    assert "fragment in normalized_key" in helper
    assert "normalized_key in normalized_keys" in helper
    assert "normalized_key.endswith(\"included\")" in helper
    assert "len(normalized_key) > len(\"included\")" not in helper
    assert "child_path} must be false" in helper
    assert "from sorafs_evidence_sensitivity import visit_sensitive_fields" in validation_helper
    assert "sensitive_keys=sensitive_keys" in validation_helper
    assert "sorafs_evidence_sensitivity" in reputation
    assert "sensitive_keys=SENSITIVE_KEYS" in reputation
    assert (
        "test_overly_deep_sensitive_scan_fails_closed_without_recursion_error"
        in helper_test
    )
    assert (
        "test_non_string_payload_keys_fail_closed_and_still_scan_children"
        in helper_test
    )
    assert "test_malformed_sensitive_key_configuration_fails_closed" in helper_test
    assert "test_sensitive_scan_rejects_malformed_error_container" in helper_test
    assert "test_sensitive_scan_rejects_malformed_existing_error_text" in helper_test
    assert (
        "test_sensitive_scan_rejects_malformed_path_before_payload_scan"
        in helper_test
    )
    assert (
        "test_sensitive_scan_rejects_malformed_evidence_label_before_payload_scan"
        in helper_test
    )
    assert missing == []


def test_sorafs_hedging_rollout_gate_rejects_sensitive_key_variants() -> None:
    checker = read(SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py")
    checker_test = read(SCRIPTS_DIR / "tests" / "check_sorafs_hedging_rollout_evidence_test.py")

    assert "validate_standard_evidence_payload(" in checker
    assert "SENSITIVE_KEYS" in function_source(
        SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py",
        "validate_evidence_payload",
    )
    assert "accessToken" in checker_test
    assert "api-key" in checker_test
    assert "payloadIncluded" in checker_test
    assert "privateKey" in checker_test
    assert "response-body" in checker_test


def test_sorafs_hedging_rollout_runner_preflights_verifier_and_outputs() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_hedging_rollout_evidence.py")
    runner_test = read(SCRIPTS_DIR / "tests" / "run_sorafs_hedging_rollout_evidence_test.py")
    helper = read(SCRIPTS_DIR / "sorafs_runner_preflight.py")

    assert "validate_runner_preflight(args" in runner
    assert "must be a directory when it exists" in helper
    assert "must not be a directory" in helper
    assert "test_missing_verifier_fails_before_plan" in runner_test
    assert "test_out_dir_file_fails_before_plan" in runner_test
    assert "test_summary_out_directory_fails_before_plan" in runner_test


def test_sorafs_plan_docs_do_not_reopen_shipped_rollout_gates() -> None:
    stale: dict[str, list[str]] = {}
    pattern = re.compile(r"\bAdd fail-closed .+ rollout evidence gate", re.I)
    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_*_plan.md")):
        matches = pattern.findall(read(path))
        if matches:
            stale[str(path.relative_to(REPO_ROOT))] = matches

    assert stale == {}


def test_sorafs_localized_plan_mirrors_have_single_front_matter_block() -> None:
    duplicate: list[str] = []
    localized_plan = re.compile(r"^sorafs_.+_plan\.[^.]+\.md$")
    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_*_plan.*.md")):
        if localized_plan.match(path.name) is None:
            continue
        source = read(path)
        if not source.startswith("---\n"):
            duplicate.append(str(path.relative_to(REPO_ROOT)))
            continue
        end = source.find("\n---\n", 4)
        if end == -1:
            duplicate.append(str(path.relative_to(REPO_ROOT)))
            continue
        body = source[end + len("\n---\n") :].lstrip()
        if body.startswith("---\n"):
            duplicate.append(str(path.relative_to(REPO_ROOT)))

    assert duplicate == []


def test_sorafs_localized_plan_mirrors_match_source_hashes() -> None:
    mismatches: dict[str, str] = {}
    localized_plan = re.compile(r"^sorafs_.+_plan\.[^.]+\.md$")
    source_re = re.compile(r"^source: (?P<source>.+)$", re.M)
    hash_re = re.compile(r"^source_hash: (?P<hash>[0-9a-f]{64})$", re.M)
    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_*_plan.*.md")):
        if localized_plan.match(path.name) is None:
            continue
        mirror = read(path)
        source_match = source_re.search(mirror)
        hash_match = hash_re.search(mirror)
        if source_match is None or hash_match is None:
            mismatches[str(path.relative_to(REPO_ROOT))] = "missing source metadata"
            continue
        source_path = REPO_ROOT / source_match.group("source")
        if not source_path.is_file():
            mismatches[str(path.relative_to(REPO_ROOT))] = "source file missing"
            continue
        source_hash = hashlib.sha256(read(source_path).encode("utf-8")).hexdigest()
        if hash_match.group("hash") != source_hash:
            mismatches[str(path.relative_to(REPO_ROOT))] = "stale source_hash"

    assert mismatches == {}
