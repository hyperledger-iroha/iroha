"""Static contract tests for SoraFS rollout evidence gates."""

from __future__ import annotations

import ast
import hashlib
import inspect
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
SORAFS_AI_PRESCREEN_PLAN = DOCS_SOURCE_DIR / "sorafs_ai_prescreen_plan.md"
SORAFS_APPEAL_PRICING_PLAN = DOCS_SOURCE_DIR / "sorafs_appeal_pricing_plan.md"
SORAFS_CLI_DOC = DOCS_SOURCE_DIR / "sorafs_cli.md"
SORAFS_CLI_SDK_PLAN = DOCS_SOURCE_DIR / "sorafs_cli_sdk_plan.md"
SORAFS_EVIDENCE_VIEWER_PLAN = DOCS_SOURCE_DIR / "sorafs_evidence_viewer_plan.md"
SORAFS_GATEWAY_COMPLIANCE_PLAN = DOCS_SOURCE_DIR / "sorafs_gateway_compliance_plan.md"
SORAFS_GATEWAY_LOAD_PLAN = DOCS_SOURCE_DIR / "sorafs_gateway_load_tests.md"
SORAFS_GOVERNANCE_DAG_PLAN = DOCS_SOURCE_DIR / "sorafs_governance_dag_plan.md"
SORAFS_HEDGING_PLAN = DOCS_SOURCE_DIR / "sorafs_hedging_plan.md"
SORAFS_MODERATION_PANEL_PLAN = DOCS_SOURCE_DIR / "sorafs_moderation_panel_plan.md"
SORAFS_ORDERBOOK_PLAN = DOCS_SOURCE_DIR / "sorafs_orderbook_plan.md"
SORAFS_COMMIT_REVEAL_PLAN = DOCS_SOURCE_DIR / "sorafs_commit_reveal_plan.md"
SORAFS_POP_CREDENTIALS_PLAN = DOCS_SOURCE_DIR / "sorafs_pop_credentials_plan.md"
SORAFS_PDP_PLAN = DOCS_SOURCE_DIR / "sorafs_pdp_plan.md"
SORAFS_POR_PLAN = DOCS_SOURCE_DIR / "sorafs_por_plan.md"
SORAFS_POR_VALIDATOR_PLAN = DOCS_SOURCE_DIR / "sorafs_por_validator_plan.md"
SORAFS_POTR_PLAN = DOCS_SOURCE_DIR / "sorafs_potr_plan.md"
SORAFS_PROTO_PLAN = DOCS_SOURCE_DIR / "sorafs_proto_plan.md"
SORAFS_REPAIR_PLAN = DOCS_SOURCE_DIR / "sorafs_repair_plan.md"
SORAFS_REFERENCE_SDK_PLAN = DOCS_SOURCE_DIR / "sorafs_reference_sdk_plan.md"
SORAFS_RELEASE_PIPELINE_PLAN = DOCS_SOURCE_DIR / "sorafs_release_pipeline_plan.md"
SORAFS_REPUTATION_PLAN = DOCS_SOURCE_DIR / "sorafs_reputation_plan.md"
SORAFS_RESERVE_RENT_PLAN = DOCS_SOURCE_DIR / "sorafs_reserve_rent_plan.md"
SORAFS_TRANSPARENCY_PLAN = DOCS_SOURCE_DIR / "sorafs_transparency_plan.md"
PRODUCTION_READINESS_CHECKER = SCRIPTS_DIR / "check_sorafs_production_readiness.py"
PRODUCTION_READINESS_RUNNER = SCRIPTS_DIR / "run_sorafs_production_readiness.py"
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
POP_CREDENTIALS_RS = REPO_ROOT / "crates" / "sorafs_manifest" / "src" / "pop_credentials.rs"
TORII_SORAFS_API_RS = REPO_ROOT / "crates" / "iroha_torii" / "src" / "sorafs" / "api.rs"
TORII_OPENAPI_RS = REPO_ROOT / "crates" / "iroha_torii" / "src" / "openapi.rs"
IROHA_CLI_SORAFS_RS = REPO_ROOT / "crates" / "iroha_cli" / "src" / "commands" / "sorafs.rs"
SORAFS_CLI_RS = REPO_ROOT / "crates" / "sorafs_orchestrator" / "src" / "bin" / "sorafs_cli.rs"
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


def canary_builders_with_name_set_validator() -> list[Path]:
    return [
        path
        for path in sorted(SCRIPTS_DIR.glob("build_sorafs_*_canary.py"))
        if "def validate_name_set(" in read(path)
    ]


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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
    return set()


def string_not_equal_validation_checkers() -> set[str]:
    return {"check_sorafs_pop_credentials_rollout_evidence.py"}


def string_value_equal_validation_checkers() -> set[str]:
    return {"check_sorafs_reputation_rollout_evidence.py"}


def schema_string_type_validation_checkers() -> set[str]:
    return {
        "check_sorafs_ai_prescreen_rollout_evidence.py",
        "check_sorafs_appeal_finance_rollout_evidence.py",
        "check_sorafs_gateway_compliance_rollout_evidence.py",
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "check_sorafs_gateway_load_rollout_evidence.py",
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
        "run_sorafs_gateway_load_rollout_evidence.py",
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


def required_source_entry_kinds(module: object) -> set[str]:
    for attribute in (
        "DEFAULT_REQUIRED_SOURCE_KINDS",
        "REQUIRED_TRANSPARENCY_SOURCE_KINDS",
    ):
        value = getattr(module, attribute, None)
        if value is not None:
            return set(value)
    return set()


def required_field_contract_is_canonical(required_fields: object) -> bool:
    if not isinstance(required_fields, tuple) or not required_fields:
        return False
    seen: set[str] = set()
    for field in required_fields:
        if (
            not isinstance(field, str)
            or not field
            or field != field.strip()
            or any(ord(char) < 32 for char in field)
            or field in seen
        ):
            return False
        seen.add(field)
    return True


def render_runner_plan_json(module: object, plan: object, args: object) -> object:
    plan_json_parameters = inspect.signature(module.plan_json).parameters
    if len(plan_json_parameters) == 1:
        return module.plan_json(plan)
    if len(plan_json_parameters) == 2:
        return module.plan_json(plan, args)
    raise AssertionError("unexpected plan_json signature")


def command_option_values(command: object, option: str) -> list[str]:
    assert isinstance(command, list)
    values: list[str] = []
    prefix = f"{option}="
    index = 0
    while index < len(command):
        if command[index] == option:
            assert index + 1 < len(command)
            values.append(command[index + 1])
            index += 2
            continue
        if isinstance(command[index], str) and command[index].startswith(prefix):
            values.append(command[index].split("=", 1)[1])
        index += 1
    return values


def command_output_values(command: object) -> list[str]:
    values: list[str] = []
    for option in ("--out", "--json-out", "--summary-out", "--output"):
        values.extend(command_option_values(command, option))
    return values


def evidence_path_from_cli_spec(spec: str) -> str:
    return spec.split("=", 1)[1] if "=" in spec else spec


def evidence_kind_from_cli_spec(spec: str) -> str | None:
    return spec.split("=", 1)[0] if "=" in spec else None


def rendered_external_evidence_values(rendered: dict[str, object]) -> list[str]:
    external_evidence = rendered.get("external_evidence", {})
    assert isinstance(external_evidence, dict)
    values: list[str] = []
    for value in external_evidence.values():
        if isinstance(value, str):
            values.append(value)
        elif isinstance(value, list):
            values.extend(item for item in value if isinstance(item, str))
    return values


def rendered_plan_steps(rendered: dict[str, object]) -> list[dict[str, object]]:
    steps = rendered.get("steps")
    assert isinstance(steps, list)
    return [step for step in steps if isinstance(step, dict)]


def rendered_gate_steps(rendered: dict[str, object]) -> list[dict[str, object]]:
    return [
        step
        for step in rendered_plan_steps(rendered)
        if step.get("label") in {"rollout_evidence_gate", "release_evidence_gate"}
    ]


def expected_rendered_plan_steps(plan: object) -> list[dict[str, object]]:
    return [
        {
            "label": step.label,
            "artifact": None if step.artifact is None else str(step.artifact),
            "command": step.command,
        }
        for step in plan
    ]


def test_rollout_gate_contract_fixtures_cover_every_checker() -> None:
    assert CHECKERS
    assert len(checker_names()) == len(set(checker_names()))


def test_rollout_runner_contract_fixtures_cover_every_runner() -> None:
    assert RUNNERS
    assert len(runner_names()) == len(set(runner_names()))


def test_top_level_sorafs_plan_localized_hashes_match_source() -> None:
    mismatches: dict[str, str | None] = {}
    checked_sources = 0
    checked_localized = 0
    for source in sorted(DOCS_SOURCE_DIR.glob("sorafs*plan.md")):
        localized_paths = sorted(DOCS_SOURCE_DIR.glob(f"{source.stem}.*.md"))
        if not localized_paths:
            continue
        checked_sources += 1
        expected = hashlib.sha256(source.read_bytes()).hexdigest()
        for path in localized_paths:
            checked_localized += 1
            actual = next(
                (
                    line.split(":", 1)[1].strip()
                    for line in read(path).splitlines()
                    if line.startswith("source_hash:")
                ),
                None,
            )
            if actual != expected:
                mismatches[str(path.relative_to(REPO_ROOT))] = actual

    assert checked_sources
    assert checked_localized
    assert mismatches == {}


def test_rollout_evidence_gates_export_dry_run_field_contracts() -> None:
    checker_failures: dict[str, list[str]] = {}
    for path in CHECKERS:
        module = load_script_module(path, f"{path.stem}_field_contract")
        fields = getattr(module, "EVIDENCE_REQUIRED_FIELDS", None)
        if not isinstance(fields, dict):
            checker_failures[path.name] = ["EVIDENCE_REQUIRED_FIELDS"]
            continue
        allowed_kinds = set(getattr(module, "KIND_BY_NAME", {}))
        required_kinds = set(getattr(module, "DEFAULT_REQUIRED_KINDS", ()))
        missing = sorted(required_kinds - set(fields))
        unknown = sorted(set(fields) - allowed_kinds)
        malformed = sorted(
            kind
            for kind, required_fields in fields.items()
            if not required_field_contract_is_canonical(required_fields)
        )
        failures = [
            *[f"missing:{kind}" for kind in missing],
            *[f"unknown:{kind}" for kind in unknown],
            *[f"malformed:{kind}" for kind in malformed],
        ]
        if failures:
            checker_failures[path.name] = failures

    runner_failures = [
        path.name
        for path in RUNNERS
        if "EVIDENCE_REQUIRED_FIELDS" not in read(path)
        or '"evidence_contract"' not in read(path)
        or '"required_payload_fields"' not in read(path)
        or "list(EVIDENCE_REQUIRED_FIELDS" not in read(path)
    ]

    assert checker_failures == {}
    assert runner_failures == []


def test_rollout_evidence_contracts_disclose_reviewed_deployment_context() -> None:
    expected_names = environment_validation_checkers() & deployment_id_validation_checkers()
    required_context_fields = {
        "deployment_id",
        "environment",
        "deployment_context_reviewed",
    }
    failures: dict[str, list[str]] = {}

    for path in CHECKERS:
        if path.name not in expected_names:
            continue
        module = load_script_module(path, f"{path.stem}_deployment_contract")
        contracts = getattr(module, "EVIDENCE_REQUIRED_FIELDS", {})
        default_required = getattr(module, "DEFAULT_REQUIRED_KINDS", ())
        missing_for_checker = []
        for kind in default_required:
            required_fields = contracts.get(kind, ())
            missing_fields = sorted(required_context_fields - set(required_fields))
            if missing_fields:
                missing_for_checker.append(f"{kind}:{','.join(missing_fields)}")
        if missing_for_checker:
            failures[path.name] = missing_for_checker

    assert failures == {}


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
    assert "path_diagnostic_label" not in response_args
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


def test_flag_backed_rollout_collection_examples_cover_default_required_kinds() -> None:
    missing: dict[str, list[str]] = {}
    for path in RUNNERS:
        module = load_script_module(path, f"sorafs_runner_example_flags_{path.stem}")
        flags_by_kind = getattr(module, "EVIDENCE_FLAGS_BY_KIND", None)
        if not isinstance(flags_by_kind, dict):
            continue
        example = runner_example_candidates(path)[0]
        assert example.is_file()
        source = read(example)
        required_kinds = getattr(module, "DEFAULT_REQUIRED_KINDS", ())
        missing_flags = [
            flags_by_kind[kind]
            for kind in required_kinds
            if kind in flags_by_kind and flags_by_kind[kind] not in source
        ]
        if missing_flags:
            missing[path.name] = missing_flags

    assert missing == {}


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


def test_rollout_runners_parse_required_kinds_before_validation() -> None:
    missing_parse: list[str] = []
    late_parse: list[str] = []

    for path in RUNNERS:
        if "--require-kind" not in read(path):
            continue
        if "parse_required_evidence_kinds(" not in function_source(path, "parse_args"):
            missing_parse.append(path.name)
        if "parse_required_evidence_kinds(" in function_source(path, "main"):
            late_parse.append(path.name)

    assert missing_parse == []
    assert late_parse == []


def test_rollout_runner_example_dry_run_contracts_match_checker_fields() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_dry_run_contract_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        try:
            rendered = render_runner_plan_json(module, plan, args)
        except AssertionError as error:
            failures[path.name] = [str(error)]
            continue
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue

        expected_kinds = list(
            getattr(
                args,
                "required_kinds",
                getattr(module, "DEFAULT_REQUIRED_KINDS", ()),
            )
        )
        contract = rendered.get("evidence_contract")
        if not isinstance(contract, dict):
            failures[path.name] = ["evidence_contract shape"]
            continue

        runner_failures: list[str] = []
        if rendered.get("required_kinds", expected_kinds) != expected_kinds:
            runner_failures.append("required_kinds")
        if sorted(contract) != sorted(expected_kinds):
            runner_failures.append("evidence_contract keys")
        for kind in expected_kinds:
            entry = contract.get(kind)
            if not isinstance(entry, dict):
                runner_failures.append(f"{kind}:contract entry")
                continue
            expected_schema = module.KIND_BY_NAME[kind].schema
            expected_fields = list(module.EVIDENCE_REQUIRED_FIELDS[kind])
            if entry.get("schema") != expected_schema:
                runner_failures.append(f"{kind}:schema")
            if entry.get("required_payload_fields") != expected_fields:
                runner_failures.append(f"{kind}:required_payload_fields")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_dry_run_evidence_contract_schemas_are_canonical() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_contract_schema_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        contract = rendered.get("evidence_contract")
        if not isinstance(contract, dict):
            failures[path.name] = ["evidence_contract shape"]
            continue
        runner_failures: list[str] = []
        for kind, entry in contract.items():
            if not isinstance(kind, str) or not isinstance(entry, dict):
                runner_failures.append(f"{kind!r}:contract entry")
                continue
            schema = entry.get("schema")
            if (
                not isinstance(schema, str)
                or not schema.startswith("sorafs.")
                or not schema.endswith(".v1")
            ):
                runner_failures.append(f"{kind}:schema:{schema!r}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_dry_run_plan_schema_matches_checker_summary() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_plan_schema_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        runner_failures: list[str] = []
        plan_schema = rendered.get("schema")
        if not isinstance(plan_schema, str) or not plan_schema.startswith("sorafs."):
            runner_failures.append("schema prefix")
        if not isinstance(plan_schema, str) or not plan_schema.endswith(
            (
                ".rollout_evidence_collection_plan.v1",
                ".release_evidence_collection_plan.v1",
            )
        ):
            runner_failures.append("schema suffix")
        if rendered.get("verifier_summary_schema") != module.SUMMARY_SCHEMA:
            runner_failures.append("verifier_summary_schema")
        if len(rendered_gate_steps(rendered)) != 1:
            runner_failures.append("gate step")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_dry_run_plan_uses_reviewed_top_level_keys() -> None:
    allowed_keys = {
        "schema",
        "verifier_summary_schema",
        "required_kinds",
        "thresholds",
        "external_evidence",
        "deployment_context",
        "evidence_contract",
        "steps",
    }
    required_keys = {
        "schema",
        "verifier_summary_schema",
        "evidence_contract",
        "steps",
    }
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_plan_keys_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        runner_failures = [
            f"unexpected:{key}"
            for key in sorted(set(rendered) - allowed_keys)
        ]
        missing = sorted(required_keys - set(rendered))
        runner_failures.extend(f"missing:{key}" for key in missing)
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_dry_run_deployment_context_matches_args() -> None:
    failures: dict[str, list[str]] = {}
    checked: list[str] = []

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_deployment_context_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        context = rendered.get("deployment_context")
        if context is None:
            continue
        checked.append(path.name)
        runner_failures: list[str] = []
        if not isinstance(context, dict):
            runner_failures.append("deployment_context shape")
        elif not hasattr(args, "deployment_id") or not hasattr(args, "environment"):
            runner_failures.append("deployment args")
        else:
            expected_context = {
                "deployment_id": args.deployment_id,
                "environment": args.environment.lower(),
                "deployment_context_reviewed": True,
            }
            if context != expected_context:
                runner_failures.append(f"deployment_context:{context}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert checked
    assert failures == {}


def test_rollout_runner_dry_run_steps_match_built_command_plan() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_steps_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        if rendered.get("steps") != expected_rendered_plan_steps(plan):
            failures[path.name] = ["steps"]

    assert failures == {}


def test_rollout_runner_verifier_gate_is_final_step() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_final_gate_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        steps = rendered_plan_steps(rendered)
        gate_indices = [
            index
            for index, step in enumerate(steps)
            if step.get("label") in {"rollout_evidence_gate", "release_evidence_gate"}
        ]
        if gate_indices != [len(steps) - 1]:
            failures[path.name] = [f"gate_indices:{gate_indices}"]

    assert failures == {}


def test_rollout_runner_verifier_summary_artifact_matches_gate_output() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_gate_summary_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        gate_steps = rendered_gate_steps(rendered)
        runner_failures: list[str] = []
        if len(gate_steps) != 1:
            runner_failures.append("gate step")
        else:
            gate_step = gate_steps[0]
            artifact = gate_step.get("artifact")
            summary_outputs = command_option_values(gate_step.get("command"), "--summary-out")
            if not isinstance(artifact, str):
                runner_failures.append("gate artifact")
            elif summary_outputs != [artifact]:
                runner_failures.append(f"--summary-out:{summary_outputs}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_verifier_gate_uses_configured_checker() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_gate_checker_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        gate_steps = rendered_gate_steps(rendered)
        runner_failures: list[str] = []
        if len(gate_steps) != 1:
            runner_failures.append("gate step")
        else:
            command = gate_steps[0].get("command")
            expected_prefix = [sys.executable, str(args.verifier)]
            if not isinstance(command, list):
                runner_failures.append("gate command")
            elif command[:2] != expected_prefix:
                runner_failures.append(f"verifier prefix:{command[:2]}")
            elif command.count(str(args.verifier)) != 1:
                runner_failures.append("verifier path count")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_dry_run_thresholds_match_gate_command() -> None:
    failures: dict[str, list[str]] = {}
    checked: list[str] = []

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_gate_thresholds_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        thresholds = rendered.get("thresholds")
        if thresholds is None:
            continue
        checked.append(path.name)
        runner_failures: list[str] = []
        if not isinstance(thresholds, dict) or not thresholds:
            runner_failures.append("thresholds shape")
        gate_steps = rendered_gate_steps(rendered)
        if len(gate_steps) != 1:
            runner_failures.append("gate step")
        elif isinstance(thresholds, dict):
            command = gate_steps[0].get("command")
            for name, value in thresholds.items():
                if not isinstance(name, str) or not name:
                    runner_failures.append(f"threshold name:{name!r}")
                    continue
                option = f"--{name.replace('_', '-')}"
                values = command_option_values(command, option)
                if values != [str(value)]:
                    runner_failures.append(f"{option}:{values}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert checked
    assert failures == {}


def test_rollout_runner_dry_run_commands_match_rendered_evidence_inputs() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_dry_run_command_{path.stem}")
        if not hasattr(module, "evidence_paths_by_kind"):
            continue
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        assert isinstance(rendered, dict)
        expected_external = {
            kind: [str(evidence_path) for evidence_path in paths]
            for kind, paths in module.evidence_paths_by_kind(args).items()
            if paths
        }
        expected_evidence_args = [
            evidence_path
            for paths in expected_external.values()
            for evidence_path in paths
        ]
        expected_required_kinds = list(getattr(args, "required_kinds", ()))
        gate_steps = rendered_gate_steps(rendered)
        runner_failures: list[str] = []
        if rendered.get("external_evidence") != expected_external:
            runner_failures.append("external_evidence")
        if len(gate_steps) != 1:
            runner_failures.append("gate step")
        else:
            command = gate_steps[0].get("command")
            if command_option_values(command, "--evidence") != expected_evidence_args:
                runner_failures.append("--evidence")
            if command_option_values(command, "--require-kind") != expected_required_kinds:
                runner_failures.append("--require-kind")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runners_reject_evidence_for_unrequired_kinds() -> None:
    failures: dict[str, list[str]] = {}
    checked: list[str] = []

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_unrequired_evidence_{path.stem}")
        if not hasattr(module, "evidence_paths_by_kind"):
            continue
        args = module.parse_args([f"@{example}", "--dry-run"])
        supplied_kinds = [
            kind
            for kind, paths in module.evidence_paths_by_kind(args).items()
            if paths
        ]
        if len(supplied_kinds) < 2:
            continue
        checked.append(path.name)
        args.required_kinds = [supplied_kinds[0]]
        errors = module.validate_inputs(args)
        diagnostic = (
            "release evidence supplied for unrequired kind"
            if path.name == "run_sorafs_reference_sdk_release_evidence.py"
            else "rollout evidence supplied for unrequired kind"
        )
        runner_failures: list[str] = []
        if diagnostic not in errors:
            runner_failures.append("unrequired evidence diagnostic")
        diagnostics = "\n".join(errors)
        if supplied_kinds[1] in diagnostics:
            runner_failures.append("kind leak")
        if runner_failures:
            failures[path.name] = runner_failures

    assert checked
    assert failures == {}


def test_rollout_runner_generated_artifacts_are_under_verifier_evidence_dir() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        if "evidence_paths_by_kind" in read(path):
            continue
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_generated_artifacts_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        assert isinstance(rendered, dict)
        steps = rendered_plan_steps(rendered)
        gate_steps = rendered_gate_steps(rendered)
        runner_failures: list[str] = []
        if len(gate_steps) != 1:
            runner_failures.append("gate step")
        else:
            evidence_dirs = command_option_values(gate_steps[0].get("command"), "--evidence-dir")
            if evidence_dirs != [str(args.out_dir)]:
                runner_failures.append("--evidence-dir")
        generated_artifacts = [
            step.get("artifact")
            for step in steps
            if step.get("label") not in {"rollout_evidence_gate", "release_evidence_gate"}
        ]
        if not generated_artifacts:
            runner_failures.append("generated artifacts")
        for artifact in generated_artifacts:
            if not isinstance(artifact, str):
                runner_failures.append("artifact shape")
                continue
            artifact_path = Path(artifact)
            if artifact_path.parent != args.out_dir:
                runner_failures.append(f"artifact corridor:{artifact_path}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_rollout_runner_explicit_evidence_args_are_visible_in_dry_run_plan() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_explicit_evidence_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        assert isinstance(rendered, dict)
        external_values = rendered_external_evidence_values(rendered)
        missing: list[str] = []
        for gate_step in rendered_gate_steps(rendered):
            for spec in command_option_values(gate_step.get("command"), "--evidence"):
                evidence_path = evidence_path_from_cli_spec(spec)
                if evidence_path not in external_values:
                    missing.append(evidence_path)
        if missing:
            failures[path.name] = sorted(missing)

    assert failures == {}


def test_rollout_runner_external_evidence_is_distinct_from_outputs() -> None:
    failures: dict[str, list[str]] = {}
    checked: list[str] = []

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_external_output_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        external_evidence = rendered.get("external_evidence")
        if external_evidence is None:
            continue
        checked.append(path.name)
        runner_failures: list[str] = []
        external_paths: list[str] = []
        if not isinstance(external_evidence, dict):
            runner_failures.append("external_evidence shape")
        else:
            for key, value in external_evidence.items():
                if not isinstance(key, str) or not key:
                    runner_failures.append(f"external key:{key!r}")
                    continue
                if isinstance(value, str):
                    if value:
                        external_paths.append(value)
                    else:
                        runner_failures.append(f"{key}:empty path")
                elif isinstance(value, list) and value:
                    for item in value:
                        if isinstance(item, str) and item:
                            external_paths.append(item)
                        else:
                            runner_failures.append(f"{key}:malformed path")
                else:
                    runner_failures.append(f"{key}:evidence shape")

        duplicate_paths = sorted(
            path_value
            for path_value in set(external_paths)
            if external_paths.count(path_value) > 1
        )
        if duplicate_paths:
            runner_failures.append(f"duplicate external evidence:{duplicate_paths}")

        artifacts = {
            artifact
            for step in rendered_plan_steps(rendered)
            if isinstance((artifact := step.get("artifact")), str)
        }
        overlaps = sorted(set(external_paths) & artifacts)
        if overlaps:
            runner_failures.append(f"external/output overlap:{overlaps}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert checked
    assert failures == {}


def test_rollout_runner_external_evidence_keys_have_contract_entries() -> None:
    failures: dict[str, list[str]] = {}
    checked: list[str] = []

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_external_contract_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        external_evidence = rendered.get("external_evidence")
        if external_evidence is None:
            continue
        checked.append(path.name)
        contract = rendered.get("evidence_contract")
        runner_failures: list[str] = []
        if not isinstance(external_evidence, dict):
            runner_failures.append("external_evidence shape")
        if not isinstance(contract, dict):
            runner_failures.append("evidence_contract shape")
        if isinstance(external_evidence, dict) and isinstance(contract, dict):
            malformed_keys = [
                key
                for key in external_evidence
                if not isinstance(key, str) or not key
            ]
            missing_contracts = [
                key
                for key in external_evidence
                if isinstance(key, str) and key and key not in contract
            ]
            runner_failures.extend(f"malformed:{key!r}" for key in malformed_keys)
            runner_failures.extend(f"missing:{key}" for key in sorted(missing_contracts))
        if runner_failures:
            failures[path.name] = runner_failures

    assert checked
    assert failures == {}


def test_rollout_runner_typed_evidence_specs_match_external_evidence_keys() -> None:
    failures: dict[str, list[str]] = {}
    checked: list[str] = []

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_typed_evidence_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        if not isinstance(rendered, dict):
            failures[path.name] = ["plan_json shape"]
            continue
        typed_specs = [
            spec
            for gate_step in rendered_gate_steps(rendered)
            for spec in command_option_values(gate_step.get("command"), "--evidence")
            if evidence_kind_from_cli_spec(spec) is not None
        ]
        if not typed_specs:
            continue
        checked.append(path.name)
        external_evidence = rendered.get("external_evidence")
        contract = rendered.get("evidence_contract")
        runner_failures: list[str] = []
        if not isinstance(external_evidence, dict):
            runner_failures.append("external_evidence shape")
        if not isinstance(contract, dict):
            runner_failures.append("evidence_contract shape")
        if isinstance(external_evidence, dict) and isinstance(contract, dict):
            for spec in typed_specs:
                kind = evidence_kind_from_cli_spec(spec)
                evidence_path = evidence_path_from_cli_spec(spec)
                if not isinstance(kind, str) or not kind or not evidence_path:
                    runner_failures.append(f"typed evidence shape:{spec!r}")
                    continue
                if kind not in contract:
                    runner_failures.append(f"{kind}:missing contract")
                value = external_evidence.get(kind)
                if isinstance(value, str):
                    external_paths = [value]
                elif isinstance(value, list):
                    external_paths = [
                        item for item in value if isinstance(item, str)
                    ]
                else:
                    runner_failures.append(f"{kind}:external_evidence")
                    continue
                if evidence_path not in external_paths:
                    runner_failures.append(f"{kind}:missing path:{evidence_path}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert checked
    assert failures == {}


def test_rollout_runner_step_artifacts_match_command_outputs() -> None:
    failures: dict[str, list[str]] = {}

    for path in RUNNERS:
        example = runner_example(path)
        assert example is not None
        module = load_script_module(path, f"sorafs_runner_artifact_outputs_{path.stem}")
        args = module.parse_args([f"@{example}", "--dry-run"])
        plan = module.build_command_plan(args)
        rendered = render_runner_plan_json(module, plan, args)
        assert isinstance(rendered, dict)
        runner_failures: list[str] = []
        for step in rendered_plan_steps(rendered):
            label = step.get("label")
            artifact = step.get("artifact")
            if artifact is None:
                continue
            if not isinstance(artifact, str):
                runner_failures.append(f"{label}:artifact shape")
                continue
            outputs = command_output_values(step.get("command"))
            if artifact not in outputs:
                runner_failures.append(f"{label}:{artifact}")
        if runner_failures:
            failures[path.name] = runner_failures

    assert failures == {}


def test_source_entry_runner_examples_cover_required_source_entry_kinds() -> None:
    missing: dict[str, list[str]] = {}
    checked: list[str] = []
    for runner in RUNNERS:
        source = read(runner)
        if "--source-entry" not in source:
            continue
        module = load_script_module(runner, f"sorafs_source_entry_runner_{runner.stem}")
        required_kinds = required_source_entry_kinds(module)
        if not required_kinds:
            continue
        example = runner_example(runner)
        assert example is not None
        example_source = read(example)
        checked.append(runner.name)
        missing_kinds = sorted(
            source_kind
            for source_kind in required_kinds
            if f"{source_kind}=" not in example_source
        )
        if missing_kinds:
            missing[runner.name] = missing_kinds
        assert example_source.count("--source-entry") >= len(required_kinds)

    assert checked
    assert missing == {}


def test_runner_malformed_spec_diagnostics_are_payload_free() -> None:
    reputation_runner = read(SCRIPTS_DIR / "run_sorafs_reputation_rollout_evidence.py")
    ai_runner = read(SCRIPTS_DIR / "run_sorafs_ai_prescreen_rollout_evidence.py")
    transparency_runner = read(SCRIPTS_DIR / "run_sorafs_transparency_rollout_evidence.py")
    reputation_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_reputation_rollout_evidence_test.py"
    )
    ai_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_ai_prescreen_rollout_evidence_test.py"
    )
    transparency_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_transparency_rollout_evidence_test.py"
    )

    assert "--provider-proof must use PROVIDER_ID=PATH form" in reputation_runner
    assert "got `{spec}`" not in reputation_runner
    assert 'errors.append("duplicate --provider-id")' in reputation_runner
    assert "duplicate --provider-id `" not in reputation_runner
    assert 'errors.append("duplicate --provider-proof")' in reputation_runner
    assert "duplicate --provider-proof for `" not in reputation_runner
    assert "missing --provider-proof for requested provider" in reputation_runner
    assert "missing --provider-proof for `" not in reputation_runner
    assert "--provider-proof supplied for unrequested provider" in reputation_runner
    assert "unrequested provider `" not in reputation_runner
    assert "--source-entry must use KIND=PATH form" in ai_runner
    assert "--source-entry must use KIND=PATH form" in transparency_runner
    assert "source-entry supplied for unsupported kind" in ai_runner
    assert "source-entry supplied for unsupported kind" in transparency_runner
    assert "duplicate source-entry kind" in ai_runner
    assert "duplicate source-entry kind" in transparency_runner
    assert "CYCLE_ID_HEX_PATTERN" in transparency_runner
    assert "--cycle-id must be a 16-byte lowercase hex string" in transparency_runner
    assert "got `{spec}`" not in ai_runner
    assert "got `{spec}`" not in transparency_runner
    assert "has conflicting " in transparency_runner
    assert "DEPLOYMENT_CONTEXT_ARTIFACT_CONFLICT_DIAGNOSTIC" in transparency_runner
    assert '"deployment context".format(path_diagnostic_label(path), field)' not in (
        transparency_runner
    )
    assert "got `{existing}`" not in transparency_runner
    assert "test_malformed_provider_proof_does_not_echo_spec" in reputation_test
    assert "test_duplicate_provider_proof_does_not_echo_provider_id" in reputation_test
    assert "test_malformed_source_entry_does_not_echo_spec" in ai_test
    assert "test_malformed_source_entry_does_not_echo_spec" in transparency_test
    assert "test_unknown_source_kind_fails_before_plan_without_leaking" in ai_test
    assert (
        "test_unknown_source_kind_fails_before_plan_without_leaking"
        in transparency_test
    )
    assert "test_duplicate_source_kind_fails_before_plan_without_leaking" in ai_test
    assert (
        "test_duplicate_source_kind_fails_before_plan_without_leaking"
        in transparency_test
    )
    assert (
        "test_generated_artifact_context_conflict_does_not_echo_existing_value"
        in transparency_test
    )
    assert "test_cycle_id_must_be_lowercase_16_byte_hex" in transparency_test
    assert "bad_cycle_id not in captured.err" in transparency_test


def test_runner_missing_input_diagnostics_are_payload_free() -> None:
    runner_sources = {path.name: read(path) for path in RUNNERS}
    production_runner = read(PRODUCTION_READINESS_RUNNER)

    for name, source in runner_sources.items():
        assert "for required `{kind}`" not in source, name
        assert "coverage for `{source_kind}`" not in source, name
        assert "missing {EVIDENCE_FLAGS_BY_KIND[kind]}" not in source, name
        assert "for unrequired `{kind}`" not in source, name

    assert "missing required rollout evidence input" in "\n".join(
        runner_sources.values()
    )
    assert "rollout evidence supplied for unrequired kind" in "\n".join(
        runner_sources.values()
    )
    assert (
        "missing required release evidence input"
        in runner_sources["run_sorafs_reference_sdk_release_evidence.py"]
    )
    assert (
        "release evidence supplied for unrequired kind"
        in runner_sources["run_sorafs_reference_sdk_release_evidence.py"]
    )
    assert (
        "missing required source-entry coverage"
        in runner_sources["run_sorafs_ai_prescreen_rollout_evidence.py"]
    )
    assert (
        "missing required source-entry coverage"
        in runner_sources["run_sorafs_transparency_rollout_evidence.py"]
    )
    assert (
        "missing --provider-proof for requested provider"
        in runner_sources["run_sorafs_reputation_rollout_evidence.py"]
    )
    assert "missing --provider-proof for `" not in runner_sources[
        "run_sorafs_reputation_rollout_evidence.py"
    ]
    assert "missing required production readiness summary input" in production_runner
    assert (
        "summary supplied for unrequired production readiness gate"
        in production_runner
    )
    assert "for required `{gate}`" not in production_runner
    assert "for unrequired `{gate}`" not in production_runner
    helper_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_ai_prescreen_rollout_evidence_test.py"
    )
    assert "dataset_manifest\" not in captured.err" in helper_test
    helper_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_transparency_rollout_evidence_test.py"
    )
    assert "feed_source\" not in captured.err" in helper_test


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
    assert "validate_evidence_parent_chain" in checker
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
    assert "scan_generated_fixture_files" in checker
    assert "path.is_symlink()" in checker
    assert "test_full_mode_rejects_symlinked_generated_fixture_root" in checker_test
    assert (
        "test_full_mode_rejects_generated_fixture_root_parent_symlink"
        in checker_test
    )
    assert (
        "test_full_mode_rejects_symlinked_generated_fixture_inventory_entry"
        in checker_test
    )
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


def test_sorafs_hedging_docs_do_not_reopen_generated_fixture_work() -> None:
    stale_phrases = (
        "statement fixture byte suite",
        "line-item, statement, and negative fixture set that still needs to be generated",
    )
    current_phrase = (
        "generated `.to`/`.json` byte suite now define the canonical SFM-5 "
        "feed, reference-price, line-item, statement, and negative fixture set"
    )
    stale: dict[str, list[str]] = {}
    missing_current: list[str] = []

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_hedging_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        matched = [phrase for phrase in stale_phrases if phrase in normalized]
        if matched:
            stale[str(path.relative_to(REPO_ROOT))] = matched
        if current_phrase not in normalized:
            missing_current.append(str(path.relative_to(REPO_ROOT)))

    assert stale == {}
    assert missing_current == []


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

    assert "INPUT_FILE_MISSING_DIAGNOSTIC" in helper
    assert "input evidence file must exist and be a file" in helper
    assert "INPUT_FILE_DUPLICATE_DIAGNOSTIC" in helper
    assert "duplicate input evidence file" in helper
    assert "duplicate {path_label} input" not in helper
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
    assert 'label="reserved output path"' in helper
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
    assert "def validate_runner_plan_steps" in helper
    assert "runner plan must be an object" in helper
    assert "runner plan steps must match command plan" in helper
    assert "render_runner_plan(rendered_plan)" in helper
    assert 'json.dumps(plan, indent=2, sort_keys=True, allow_nan=False) + "\\n"' in helper
    assert "sys.stdout.write(render_runner_plan(plan))" in helper
    assert "except (TypeError, ValueError) as error" in helper
    assert "failed to render runner plan JSON" in helper
    assert "failed to render runner plan JSON: {error}" not in helper
    assert "failed to render runner plan JSON: {error_diagnostic_label(error)}" in helper
    assert "test_render_runner_plan_rejects_non_object_plan" in helper_test
    assert "test_validate_runner_plan_steps_matches_command_plan" in helper_test
    assert "test_validate_runner_plan_steps_rejects_non_object_and_drift" in helper_test
    assert "test_validate_runner_plan_steps_rejects_unrenderable_plan" in helper_test
    assert "test_write_runner_plan_reports_non_object_plan_without_stdout" in helper_test
    assert "test_write_runner_plan_sanitizes_malformed_render_error" in helper_test
    assert all("validate_runner_plan_steps," in read(path) for path in RUNNERS)
    assert all("rendered_plan = plan_json(plan, args)" in read(path) for path in RUNNERS)
    for path in RUNNERS:
        source = read(path)
        direct_marker = "plan_errors = validate_runner_plan_steps(rendered_plan, plan)"
        wrapped_marker = "plan_errors = validate_plan_json(rendered_plan, plan, args)"
        if path.name in {
            "run_sorafs_appeal_finance_rollout_evidence.py",
            "run_sorafs_ai_prescreen_rollout_evidence.py",
            "run_sorafs_gateway_compliance_rollout_evidence.py",
            "run_sorafs_gateway_load_rollout_evidence.py",
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
            "run_sorafs_transparency_rollout_evidence.py",
        }:
            assert wrapped_marker in source
            assert "errors.extend(validate_runner_plan_steps(rendered, plan))" in source
            assert source.index(wrapped_marker) < source.index("if args.dry_run:")
        else:
            assert direct_marker in source
            assert source.index(direct_marker) < source.index("if args.dry_run:")
    assert all("plan_errors = write_runner_plan" in read(path) for path in RUNNERS)
    assert all("write_runner_plan(rendered_plan)" in read(path) for path in RUNNERS)
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

    assert "path_diagnostic_label" not in runner
    assert "error_diagnostic_label(error, path_label=path_label)" not in runner
    assert "DEPLOYMENT_CONTEXT_ARTIFACT_READ_DIAGNOSTIC" in runner
    assert "DEPLOYMENT_CONTEXT_ARTIFACT_WRITE_DIAGNOSTIC" in runner
    assert "DEPLOYMENT_CONTEXT_ARTIFACT_PARENT_DIAGNOSTIC" in runner
    assert "validate_deployment_context_artifact_parent" in runner
    assert "def deployment_context_write_open_flags" in runner
    assert "def write_all_deployment_context_bytes" in runner
    assert "view = memoryview(payload)" in runner
    assert "written = os.write(fd, view)" in runner
    assert "if written <= 0:" in runner
    assert "write_all_deployment_context_bytes(fd, rendered)" in runner
    assert "validate_runner_input_parent_chain" in runner
    assert "os.open(path, deployment_context_write_open_flags())" in runner
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in runner
    assert "path.write_text(render_runner_plan(payload)" not in runner
    assert "failed to read generated evidence artifact `{path}`: {error}" not in runner
    assert "failed to write deployment context into `{path}`: {error}" not in runner
    assert "failed to read generated evidence artifact `{}" not in runner
    assert "failed to write deployment context into `{}" not in runner
    assert "test_generated_artifact_read_error_is_sanitized" in runner_test
    assert "test_deployment_context_write_uses_no_follow_descriptor_open" in runner_test
    assert "test_deployment_context_write_retries_short_os_write" in runner_test
    assert "test_deployment_context_write_error_is_sanitized" in runner_test
    assert "test_deployment_context_write_rejects_symlink_swap_before_open" in runner_test
    assert (
        "test_deployment_context_write_rejects_parent_symlink_swap_before_open"
        in runner_test
    )


def test_transparency_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_transparency_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_transparency_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def validate_plan_json" in runner
    assert "transparency rollout runner plan must be an object" in runner
    assert (
        "transparency rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "transparency rollout runner plan deployment_context must match args" in runner
    assert (
        "transparency rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_plan_json_deployment_context_must_stay_reviewed" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_reputation_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_reputation_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_reputation_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "EXTERNAL_EVIDENCE_FIELDS" in runner
    assert "def validate_plan_json" in runner
    assert "reputation rollout runner plan must be an object" in runner
    assert (
        "reputation rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "reputation rollout runner plan external_evidence must match args" in runner
    assert (
        "reputation rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_gateway_load_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_gateway_load_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_gateway_load_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "gateway load rollout runner plan must be an object" in runner
    assert (
        "gateway load rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "gateway load rollout runner plan required_kinds must match args" in runner
    assert "gateway load rollout runner plan thresholds must match args" in runner
    assert "gateway load rollout runner plan external_evidence must match args" in runner
    assert (
        "gateway load rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_gateway_compliance_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_gateway_compliance_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_gateway_compliance_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "gateway compliance rollout runner plan must be an object" in runner
    assert (
        "gateway compliance rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "gateway compliance rollout runner plan required_kinds must match args" in runner
    assert "gateway compliance rollout runner plan thresholds must match args" in runner
    assert (
        "gateway compliance rollout runner plan external_evidence must match args"
        in runner
    )
    assert (
        "gateway compliance rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_ai_prescreen_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_ai_prescreen_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_ai_prescreen_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def validate_plan_json" in runner
    assert "AI pre-screen rollout runner plan must be an object" in runner
    assert (
        "AI pre-screen rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "AI pre-screen rollout runner plan external_evidence must match args" in runner
    assert (
        "AI pre-screen rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_pdp_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_pdp_rollout_evidence.py")
    runner_test = read(SCRIPTS_DIR / "tests" / "run_sorafs_pdp_rollout_evidence_test.py")

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "PDP rollout runner plan must be an object" in runner
    assert (
        "PDP rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "PDP rollout runner plan required_kinds must match args" in runner
    assert "PDP rollout runner plan thresholds must match args" in runner
    assert "PDP rollout runner plan external_evidence must match args" in runner
    assert "PDP rollout runner plan evidence_contract must match checker fields" in runner
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_potr_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_potr_rollout_evidence.py")
    runner_test = read(SCRIPTS_DIR / "tests" / "run_sorafs_potr_rollout_evidence_test.py")

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "PoTR rollout runner plan must be an object" in runner
    assert (
        "PoTR rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "PoTR rollout runner plan required_kinds must match args" in runner
    assert "PoTR rollout runner plan thresholds must match args" in runner
    assert "PoTR rollout runner plan external_evidence must match args" in runner
    assert "PoTR rollout runner plan evidence_contract must match checker fields" in runner
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_por_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_por_rollout_evidence.py")
    runner_test = read(SCRIPTS_DIR / "tests" / "run_sorafs_por_rollout_evidence_test.py")

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "PoR rollout runner plan must be an object" in runner
    assert (
        "PoR rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "PoR rollout runner plan required_kinds must match args" in runner
    assert "PoR rollout runner plan thresholds must match args" in runner
    assert "PoR rollout runner plan external_evidence must match args" in runner
    assert "PoR rollout runner plan evidence_contract must match checker fields" in runner
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_repair_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_repair_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_repair_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "repair rollout runner plan must be an object" in runner
    assert (
        "repair rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "repair rollout runner plan required_kinds must match args" in runner
    assert "repair rollout runner plan thresholds must match args" in runner
    assert "repair rollout runner plan external_evidence must match args" in runner
    assert "repair rollout runner plan evidence_contract must match checker fields" in runner
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_governance_dag_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_governance_dag_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_governance_dag_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "Governance DAG rollout runner plan must be an object" in runner
    assert (
        "Governance DAG rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "Governance DAG rollout runner plan required_kinds must match args" in runner
    assert "Governance DAG rollout runner plan thresholds must match args" in runner
    assert (
        "Governance DAG rollout runner plan external_evidence must match args"
        in runner
    )
    assert (
        "Governance DAG rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_orderbook_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_orderbook_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_orderbook_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "orderbook rollout runner plan must be an object" in runner
    assert (
        "orderbook rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "orderbook rollout runner plan required_kinds must match args" in runner
    assert "orderbook rollout runner plan thresholds must match args" in runner
    assert "orderbook rollout runner plan external_evidence must match args" in runner
    assert (
        "orderbook rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_appeal_finance_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_appeal_finance_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_appeal_finance_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "appeal finance rollout runner plan must be an object" in runner
    assert (
        "appeal finance rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "appeal finance rollout runner plan required_kinds must match args" in runner
    assert "appeal finance rollout runner plan thresholds must match args" in runner
    assert (
        "appeal finance rollout runner plan external_evidence must match args"
        in runner
    )
    assert (
        "appeal finance rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_reserve_rent_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_reserve_rent_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_reserve_rent_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "reserve/rent rollout runner plan must be an object" in runner
    assert (
        "reserve/rent rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "reserve/rent rollout runner plan required_kinds must match args" in runner
    assert "reserve/rent rollout runner plan thresholds must match args" in runner
    assert "reserve/rent rollout runner plan external_evidence must match args" in runner
    assert (
        "reserve/rent rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_pop_credentials_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_pop_credentials_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_pop_credentials_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "PoP credential rollout runner plan must be an object" in runner
    assert (
        "PoP credential rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "PoP credential rollout runner plan required_kinds must match args" in runner
    assert "PoP credential rollout runner plan thresholds must match args" in runner
    assert (
        "PoP credential rollout runner plan external_evidence must match args"
        in runner
    )
    assert (
        "PoP credential rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_hedging_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_hedging_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_hedging_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "hedging/billing rollout runner plan must be an object" in runner
    assert (
        "hedging/billing rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "hedging/billing rollout runner plan required_kinds must match args" in runner
    assert "hedging/billing rollout runner plan thresholds must match args" in runner
    assert (
        "hedging/billing rollout runner plan external_evidence must match args"
        in runner
    )
    assert (
        "hedging/billing rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_moderation_panel_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_moderation_panel_rollout_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_moderation_panel_rollout_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def threshold_values" in runner
    assert "def validate_plan_json" in runner
    assert "moderation panel rollout runner plan must be an object" in runner
    assert (
        "moderation panel rollout runner plan fields must match the schema-closed contract"
        in runner
    )
    assert "moderation panel rollout runner plan required_kinds must match args" in runner
    assert "moderation panel rollout runner plan thresholds must match args" in runner
    assert (
        "moderation panel rollout runner plan external_evidence must match args"
        in runner
    )
    assert (
        "moderation panel rollout runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_sorafs_validate_release_packager_rejects_symlink_stage_entries() -> None:
    packager = read(SCRIPTS_DIR / "package_sorafs_validate_release.sh")
    packager_test = read(
        SCRIPTS_DIR / "tests" / "package_sorafs_validate_release_test.py"
    )

    assert "def validate_archive_path" in packager
    assert "def scan_stage_entries" in packager
    assert "def read_open_flags" in packager
    assert "def write_open_flags" in packager
    assert "def write_manifest_no_follow" in packager
    assert "def write_all(fd, chunk)" in packager
    assert "O_NOFOLLOW" in packager
    assert "path.lstat()" in packager
    assert "os.open(path, read_open_flags())" in packager
    assert "os.open(path, write_open_flags(), 0o666)" in packager
    assert "os.fstat(fd)" in packager
    assert "json.dumps(payload, indent=2, sort_keys=True, allow_nan=False)" in packager
    assert "written = os.write(fd, view)" in packager
    assert "write_all(fd, rendered)" in packager
    assert "write_manifest_no_follow(manifest_path, manifest)" in packager
    assert "validate_archive_path(archive_path, \"release package archive\")" in packager
    assert "os.open(archive_path, write_open_flags(), 0o666)" in packager
    assert "path.open(\"rb\")" not in packager
    assert "archive_path.open(\"wb\")" not in packager
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in packager
    assert "json.dump(payload" not in packager
    assert "with open(sys.argv[1]" not in packager
    assert "root.rglob(\"*\")" in packager
    assert "for path in scan_stage_entries(stage_dir)" in packager
    assert "test_release_packager_accepts_regular_staged_files" in packager_test
    assert "test_release_packager_rejects_symlinked_staged_entries" in packager_test
    assert (
        "test_release_packager_rejects_symlinked_output_parent_before_archive"
        in packager_test
    )


def test_sorafs_orchestrator_fixture_builder_uses_no_follow_io() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_orchestrator_fixture.py")
    builder_test = read(SCRIPTS_DIR / "tests" / "build_sorafs_orchestrator_fixture_test.py")

    assert "def validate_fixture_path" in builder
    assert "def ensure_fixture_directory" in builder
    assert "def read_open_flags" in builder
    assert "def write_open_flags" in builder
    assert "def write_all(fd: int, chunk: bytes) -> None" in builder
    assert "def fixture_file_size" in builder
    assert "os.open(plan_path, read_open_flags())" in builder
    assert "os.open(path, write_open_flags(), 0o666)" in builder
    assert "json.dumps(payload, indent=2, allow_nan=False)" in builder
    assert "written = os.write(fd, view)" in builder
    assert "write_all(fd, rendered)" in builder
    assert '"reputation_score_bps": 10_000' in builder
    assert "os.fstat(fd).st_size" in builder
    assert "plan_path.open(" not in builder
    assert "path.open(\"w\"" not in builder
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in builder
    assert "json.dump(payload" not in builder
    assert "stat().st_size" not in builder
    assert "test_load_chunker_fixture_rejects_symlink_before_open" in builder_test
    assert "test_write_json_uses_no_follow_descriptor_open" in builder_test
    assert "test_write_json_completes_partial_descriptor_writes" in builder_test
    assert "test_build_telemetry_includes_reputation_score" in builder_test
    assert "test_ensure_fixture_directory_rejects_symlink_before_create" in builder_test
    assert "test_fixture_file_size_uses_no_follow_descriptor_fstat" in builder_test


def test_android_codegen_sorafs_fixture_replay_uses_no_follow_io() -> None:
    replay = read(SCRIPTS_DIR / "android_codegen_replay_sorafs_fixture.py")
    replay_test = read(
        SCRIPTS_DIR / "tests" / "android_codegen_replay_sorafs_fixture_test.py"
    )

    assert "def validate_codegen_path" in replay
    assert "def ensure_codegen_directory" in replay
    assert "def require_codegen_file" in replay
    assert "def read_open_flags" in replay
    assert "def write_open_flags" in replay
    assert "def write_all(fd: int, chunk: bytes) -> None" in replay
    assert 'getattr(os, "O_NOFOLLOW", 0)' in replay
    assert "os.open(path, read_open_flags())" in replay
    assert "os.open(path, write_open_flags(), 0o666)" in replay
    assert "json.dumps(payload, indent=2, allow_nan=False)" in replay
    assert "written = os.write(fd, view)" in replay
    assert "write_all(fd, rendered)" in replay
    assert "payload_path = require_codegen_file(" in replay
    assert "plan_path = require_codegen_file(" in replay
    assert 'path.open("r"' not in replay
    assert 'path.open("w"' not in replay
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in replay
    assert "json.dump(payload" not in replay
    assert "test_load_json_uses_no_follow_descriptor_open" in replay_test
    assert "test_load_json_rejects_symlink_before_open" in replay_test
    assert "test_write_json_uses_no_follow_descriptor_open" in replay_test
    assert "test_write_json_completes_partial_descriptor_writes" in replay_test
    assert "test_write_json_rejects_symlinked_parent_before_create" in replay_test
    assert "test_require_codegen_file_rejects_symlink_before_subprocess" in replay_test


def test_sorafs_orchestrator_adoption_gate_uses_no_follow_io() -> None:
    adoption = read(REPO_ROOT / "ci" / "check_sorafs_orchestrator_adoption.sh")

    assert "def read_open_flags() -> int" in adoption
    assert "def write_open_flags() -> int" in adoption
    assert 'getattr(os, "O_NOFOLLOW", 0)' in adoption
    assert "path.lstat()" in adoption
    assert "stat.S_ISREG(path_stat.st_mode)" in adoption
    assert "os.open(path, read_open_flags())" in adoption
    assert "os.fstat(fd).st_size" in adoption
    assert "os.open(path, write_open_flags(), 0o666)" in adoption
    assert "def validate_adoption_path" in adoption
    assert "def ensure_adoption_directory" in adoption
    assert "def require_adoption_file" in adoption
    assert adoption.count("def write_all(fd: int, chunk: bytes) -> None") >= 2
    assert "view = memoryview(chunk)" in adoption
    assert "written = os.write(fd, view)" in adoption
    assert 'write_all(fd, body.encode("utf-8"))' in adoption
    assert "json.dumps(payload, indent=2, allow_nan=False)" in adoption
    assert "write_all(fd, rendered)" in adoption
    assert "write_adoption_text(config_path" in adoption
    assert "write_adoption_json(note_path" in adoption
    assert 'require_nonempty_file "${CONFIG_PATH}" "fixture config"' in adoption
    assert 'path.open("r"' not in adoption
    assert 'config_path.open("w"' not in adoption
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in adoption
    assert "json.dump(payload" not in adoption
    assert "note_path.write_text" not in adoption


def test_sorafs_orchestrator_sdk_parity_gate_uses_no_follow_io() -> None:
    sdk_gate = read(REPO_ROOT / "ci" / "sdk_sorafs_orchestrator.sh")

    assert "def read_open_flags() -> int" in sdk_gate
    assert "def write_open_flags() -> int" in sdk_gate
    assert "def append_open_flags() -> int" in sdk_gate
    assert "def validate_sdk_path" in sdk_gate
    assert "def ensure_sdk_directory" in sdk_gate
    assert "def require_sdk_file" in sdk_gate
    assert "def read_text_artifact" in sdk_gate
    assert "def write_text_artifact" in sdk_gate
    assert sdk_gate.count("def write_all(fd: int, chunk: bytes) -> None") >= 3
    assert 'getattr(os, "O_NOFOLLOW", 0)' in sdk_gate
    assert "stat.S_ISREG(path_stat.st_mode)" in sdk_gate
    assert "os.open(source, read_open_flags())" in sdk_gate
    assert "os.open(target, write_open_flags(), 0o666)" in sdk_gate
    assert "os.open(path, read_open_flags())" in sdk_gate
    assert "os.open(path, write_open_flags(), 0o666)" in sdk_gate
    assert "os.open(path, append_open_flags(), 0o666)" in sdk_gate
    assert "view = memoryview(chunk)" in sdk_gate
    assert "written = os.write(fd, view)" in sdk_gate
    assert "write_all(write_fd, chunk)" in sdk_gate
    assert 'write_all(fd, body.encode("utf-8"))' in sdk_gate
    assert 'write_all(fd, ("\\t".join(fields) + "\\n").encode("utf-8"))' in sdk_gate
    assert "write_text_artifact(\n    summary_path" in sdk_gate
    assert "write_text_artifact(\n    matrix_path" in sdk_gate
    assert "with open(" not in sdk_gate
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in sdk_gate
    assert ".write_text(" not in sdk_gate
    assert ".read_text(" not in sdk_gate
    assert 'cp "${source_file}" "${target_file}"' not in sdk_gate
    assert '>>"${RESULTS_TSV}"' not in sdk_gate
    assert ': >"${RESULTS_TSV}"' not in sdk_gate


def test_sorafs_gateway_denylist_gate_uses_no_follow_io() -> None:
    denylist_gate = read(REPO_ROOT / "ci" / "check_sorafs_gateway_denylist.sh")

    assert "copy_file_no_follow()" in denylist_gate
    assert "require_nonempty_file()" in denylist_gate
    assert "first_bundle_json()" in denylist_gate
    assert "run_xtask()" in denylist_gate
    assert 'cargo run -p xtask --bin xtask --quiet -- "$@"' in denylist_gate
    assert "cargo run -p iroha_cli --bin iroha3 --quiet --" in denylist_gate
    assert '-c "${ROOT_DIR}/defaults/client.toml"' in denylist_gate
    assert "app sorafs gateway evidence" in denylist_gate
    assert '--denylist "${new_json}"' in denylist_gate
    assert "def read_open_flags() -> int" in denylist_gate
    assert "def write_open_flags() -> int" in denylist_gate
    assert "def validate_path" in denylist_gate
    assert "def require_regular_file" in denylist_gate
    assert "def write_all(fd: int, chunk: bytes) -> None" in denylist_gate
    assert "view = memoryview(chunk)" in denylist_gate
    assert "written = os.write(fd, view)" in denylist_gate
    assert "if written <= 0:" in denylist_gate
    assert "write_all(write_fd, chunk)" in denylist_gate
    assert "json.dumps(payload, indent=2, sort_keys=True, allow_nan=False)" in denylist_gate
    assert "write_all(fd, rendered)" in denylist_gate
    assert "old_only_entry = {" in denylist_gate
    assert "Retired CI denylist diff control" in denylist_gate
    assert 'getattr(os, "O_NOFOLLOW", 0)' in denylist_gate
    assert "path.lstat()" in denylist_gate
    assert "stat.S_ISREG(path_stat.st_mode)" in denylist_gate
    assert "os.open(source, read_open_flags())" in denylist_gate
    assert "os.open(target, write_open_flags(), 0o666)" in denylist_gate
    assert "os.open(path, read_open_flags())" in denylist_gate
    assert "os.open(path, write_open_flags(), 0o666)" in denylist_gate
    assert "os.fstat(fd).st_size" in denylist_gate
    assert 'copy_file_no_follow "${SAMPLE_JSON}" "${new_json}"' in denylist_gate
    assert 'copy_file_no_follow "${evidence_json}" "${evidence_copy_path}"' in denylist_gate
    assert 'old_bundle="$(first_bundle_json "${old_out}" "old denylist")"' in denylist_gate
    assert 'new_bundle="$(first_bundle_json "${new_out}" "new denylist")"' in denylist_gate
    assert "require_nonempty_file \"${diff_report}\" \"diff report\"" in denylist_gate
    assert "with open(" not in denylist_gate
    assert "json.load(open(" not in denylist_gate
    assert "json.dump(payload" not in denylist_gate
    assert "cargo xtask sorafs-gateway" not in denylist_gate
    assert 'cp "${SAMPLE_JSON}" "${new_json}"' not in denylist_gate
    assert 'cp "${evidence_json}" "${evidence_copy_path}"' not in denylist_gate
    assert "os.write(write_fd, chunk)" not in denylist_gate
    assert "parent.is_symlink()" not in denylist_gate
    assert "parent must not be a symlink" not in denylist_gate
    assert '[[ ! -s "${diff_report}" ]]' not in denylist_gate
    assert 'ls "${old_out}"/*.json | head -n1' not in denylist_gate


def test_sorafs_reference_ffi_header_gate_uses_no_follow_io() -> None:
    header_gate = read(REPO_ROOT / "ci" / "check_sorafs_reference_ffi_header.sh")

    assert "def read_open_flags() -> int" in header_gate
    assert "def write_open_flags() -> int" in header_gate
    assert "def read_text_no_follow" in header_gate
    assert "copy_file_no_follow()" in header_gate
    assert 'getattr(os, "O_NOFOLLOW", 0)' in header_gate
    assert "path.lstat()" in header_gate
    assert "stat.S_ISREG(path_stat.st_mode)" in header_gate
    assert "os.open(path, read_open_flags())" in header_gate
    assert "os.fstat(fd)" in header_gate
    assert "os.open(source, read_open_flags())" in header_gate
    assert "os.open(target, write_open_flags(), 0o666)" in header_gate
    assert "copy_file_no_follow \"${RUST_FFI}\"" in header_gate
    assert "copy_file_no_follow \"${HEADER}\"" in header_gate
    assert "read_text(" not in header_gate
    assert 'cp "${RUST_FFI}"' not in header_gate
    assert 'cp "${HEADER}"' not in header_gate


def test_sorafs_fixtures_gate_uses_no_follow_alias_json_reads() -> None:
    fixtures_gate = read(REPO_ROOT / "ci" / "check_sorafs_fixtures.sh")

    assert "def read_open_flags() -> int" in fixtures_gate
    assert "def read_json_no_follow" in fixtures_gate
    assert 'getattr(os, "O_NOFOLLOW", 0)' in fixtures_gate
    assert "path.lstat()" in fixtures_gate
    assert "stat.S_ISREG(path_stat.st_mode)" in fixtures_gate
    assert "os.open(path, read_open_flags())" in fixtures_gate
    assert "os.fstat(fd)" in fixtures_gate
    assert "json.load(handle)" in fixtures_gate
    assert "data = read_json_no_follow(path)" in fixtures_gate
    assert "path.read_text()" not in fixtures_gate
    assert "json.loads(path.read_text())" not in fixtures_gate


def test_sorafs_pin_register_sdk_guards_use_no_follow_source_reads() -> None:
    main_guard = read(REPO_ROOT / "ci" / "check_sorafs_pin_register_sdk_guard.sh")
    swift_guard = read(REPO_ROOT / "ci" / "check_sorafs_pin_register_swift_sdk.sh")

    for guard in (main_guard, swift_guard):
        assert "def read_open_flags() -> int" in guard
        assert "def read_text_no_follow" in guard
        assert 'getattr(os, "O_NOFOLLOW", 0)' in guard
        assert "path.lstat()" in guard or "full_path.lstat()" in guard
        assert "stat.S_ISREG(path_stat.st_mode)" in guard
        assert "os.fstat(fd)" in guard
        assert "os.fdopen(fd, \"r\", encoding=\"utf-8\")" in guard
        assert ".read_text(" not in guard

    assert "if path in text_overrides:" in main_guard
    assert "return read_text_no_follow(full_path, path)" in main_guard
    assert "return read_text_no_follow(root / path)" in swift_guard


def test_sorafs_docs_portal_package_summary_uses_no_follow_read() -> None:
    packager = read(REPO_ROOT / "ci" / "package_docs_portal_sorafs.sh")

    assert "def read_open_flags() -> int" in packager
    assert "def read_summary_lines_no_follow" in packager
    assert 'getattr(os, "O_NOFOLLOW", 0)' in packager
    assert "os.path.islink(path)" in packager
    assert "os.lstat(path)" in packager
    assert "stat.S_ISREG(path_stat.st_mode)" in packager
    assert "os.open(path, read_open_flags())" in packager
    assert "os.fstat(fd)" in packager
    assert "os.fdopen(fd, \"r\", encoding=\"utf-8\")" in packager
    assert "for raw in read_summary_lines_no_follow(summary_path)" in packager
    assert "with open(summary_path" not in packager


def test_sorafs_docs_portal_pin_release_descriptor_uses_no_follow_io() -> None:
    pin_release = read(REPO_ROOT / "docs" / "portal" / "scripts" / "sorafs-pin-release.sh")

    assert "def read_open_flags() -> int" in pin_release
    assert "def write_open_flags() -> int" in pin_release
    assert "def validate_descriptor_path" in pin_release
    assert "def read_descriptor" in pin_release
    assert "def write_descriptor" in pin_release
    assert "def write_all(fd: int, chunk: bytes) -> None" in pin_release
    assert 'getattr(os, "O_NOFOLLOW", 0)' in pin_release
    assert "path.lstat()" in pin_release
    assert "stat.S_ISREG(path_stat.st_mode)" in pin_release
    assert "os.open(path, read_open_flags())" in pin_release
    assert "os.open(path, write_open_flags(), 0o666)" in pin_release
    assert "os.fstat(fd)" in pin_release
    assert "json.load(handle)" in pin_release
    assert "json.dumps(payload, indent=2, allow_nan=False)" in pin_release
    assert "written = os.write(fd, view)" in pin_release
    assert "write_all(fd, rendered)" in pin_release
    assert "target.read_text" not in pin_release
    assert "target.write_text" not in pin_release
    assert ".read_text(" not in pin_release
    assert ".write_text(" not in pin_release


def test_sorafs_shell_helpers_use_no_follow_json_reads() -> None:
    release_cli = read(SCRIPTS_DIR / "release_sorafs_cli.sh")
    direct_smoke = read(SCRIPTS_DIR / "sorafs_direct_mode_smoke.sh")
    gateway_probe = read(SCRIPTS_DIR / "telemetry" / "run_sorafs_gateway_probe.sh")

    for script in (release_cli, direct_smoke, gateway_probe):
        assert "def read_open_flags() -> int" in script
        assert 'getattr(os, "O_NOFOLLOW", 0)' in script
        assert "os.lstat(path)" in script
        assert "stat.S_ISREG(path_stat.st_mode)" in script
        assert "os.open(path, read_open_flags())" in script
        assert "os.fstat(fd)" in script
        assert "json.load(" in script
        assert "with open(" not in script

    assert "identity_token_hash_blake3_hex" in release_cli
    assert "sign summary must not be a symlink" in release_cli
    assert "policy path must not be a symlink" in direct_smoke
    assert "probe JSON report must not be a symlink" in gateway_probe


def test_sorafs_operator_helpers_do_not_reintroduce_plain_file_io() -> None:
    roots = (SCRIPTS_DIR, REPO_ROOT / "ci", REPO_ROOT / ".github")
    allowed_rglob = {
        SCRIPTS_DIR / "check_sorafs_hedging_fixture_manifest.py",
        SCRIPTS_DIR / "sorafs_evidence_paths.py",
        SCRIPTS_DIR / "package_sorafs_validate_release.sh",
    }
    allowed_tarfile_open = {SCRIPTS_DIR / "package_sorafs_validate_release.sh"}
    offenders: list[str] = []

    for root in roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*sorafs*")):
            if not path.is_file():
                continue
            if SCRIPTS_DIR / "tests" in path.parents:
                continue
            source = read(path)
            rel = path.relative_to(REPO_ROOT)
            for needle in (".read_text(", ".write_text(", "shutil.copy"):
                if needle in source:
                    offenders.append(f"{rel}: {needle}")
            if re.search(r"(?<![\w.])open\s*\(", source):
                offenders.append(f"{rel}: plain open(")
            if ".rglob(" in source and path not in allowed_rglob:
                offenders.append(f"{rel}: unreviewed rglob(")
            if "tarfile.open(" in source and path not in allowed_tarfile_open:
                offenders.append(f"{rel}: unreviewed tarfile.open(")

    assert offenders == []


def test_sorafs_operator_helpers_use_shared_path_identity_for_resolution() -> None:
    roots = (SCRIPTS_DIR, REPO_ROOT / "ci", REPO_ROOT / ".github")
    offenders: list[str] = []

    for root in roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*sorafs*")):
            if not path.is_file():
                continue
            if SCRIPTS_DIR / "tests" in path.parents:
                continue
            for line_number, line in enumerate(read(path).splitlines(), start=1):
                if ".resolve(" not in line and "os.path.realpath(" not in line:
                    continue
                stripped = line.strip()
                bootstrap_resolve = "__file__" in line and ".resolve()" in line
                shared_helper_resolve = (
                    path == PATH_IDENTITY_HELPER and stripped == "return path.resolve()"
                )
                if not (bootstrap_resolve or shared_helper_resolve):
                    rel = path.relative_to(REPO_ROOT)
                    offenders.append(f"{rel}:{line_number}: {stripped}")

    assert offenders == []


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
    assert "def inspect_runner_path_size" in helper
    assert "def runner_path_size_open_flags" in helper
    assert "os.open(path, runner_path_size_open_flags())" in helper
    assert "os.fstat(fd).st_size" in helper
    assert "PLAN_RENDERED_PATH_ERROR" in helper
    assert "RUNNER_URL_ARG_ERROR" in helper
    assert "RUNNER_PASSTHROUGH_ARG_ERROR" in helper
    assert "def plan_rendered_path_is_safe" in helper
    assert "def validate_plan_rendered_paths" in helper
    assert "def runner_url_arg_is_plan_safe" in helper
    assert "def require_runner_url_args" in helper
    assert "def runner_passthrough_arg_is_plan_safe" in helper
    assert "def require_runner_passthrough_args" in helper
    assert "def is_sensitive_path_component" in helper
    assert "HIGH_RISK_SENSITIVE_KEY_FRAGMENTS" in helper
    assert "drive-prefix" in helper
    assert "urlsplit(value)" in helper
    assert "parsed.username is not None" in helper
    assert "parsed.query or parsed.fragment" in helper
    assert "validate_plan_rendered_paths((verifier, out_dir, summary_out), errors)" in helper
    assert "not plan_rendered_path_is_safe(path)" in helper
    assert "PLAN_RENDERED_PATH_ERROR not in errors" in helper
    assert "path.stat().st_size" not in helper
    assert "def inspect_runner_path_is_dir" in helper
    assert "path_is_symlink = inspect_runner_path_is_symlink(" in helper
    assert "def validate_runner_input_parent_chain" in helper
    assert "validate_runner_input_parent_chain(path, error_list, label=path_label)" in helper
    assert "validate_runner_input_parent_chain(path, errors, label=path_label)" in helper
    assert "validate_runner_input_parent_chain(\n                verifier" in helper
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
    assert "test_verifier_symlink_fails_preflight" in helper_test
    assert "test_verifier_parent_symlink_fails_preflight" in helper_test
    assert "test_verifier_parent_chain_symlink_fails_preflight" in helper_test
    assert "test_verifier_symlink_inspection_failure_fails_preflight" in helper_test
    assert "test_out_dir_inspection_failure_fails_preflight" in helper_test
    assert "test_validate_runner_output_dir_rejects_non_path_without_traceback" in helper_test
    assert (
        "test_validate_runner_output_parent_rejects_non_path_without_traceback"
        in helper_test
    )
    assert "test_runner_preflight_sanitizes_malformed_non_path_targets" in helper_test
    assert "test_plan_rendered_path_safety_rejects_unsafe_components" in helper_test
    assert (
        "test_validate_plan_rendered_paths_rejects_unsafe_components_without_leaking"
        in helper_test
    )
    assert (
        "test_validate_runner_preflight_rejects_plan_rendered_out_dir_without_leaking"
        in helper_test
    )
    assert (
        "test_validate_runner_preflight_rejects_plan_rendered_summary_out_without_leaking"
        in helper_test
    )
    assert (
        "test_validate_runner_preflight_rejects_plan_rendered_verifier_without_leaking"
        in helper_test
    )
    assert (
        "test_validate_plan_rendered_paths_rejects_malformed_error_container"
        in helper_test
    )
    assert (
        "test_input_file_rejects_plan_rendered_unsafe_component_without_leaking"
        in helper_test
    )
    assert "test_input_file_accepts_payload_free_digest_label" in helper_test
    assert (
        "test_input_directory_rejects_plan_rendered_unsafe_component_without_leaking"
        in helper_test
    )
    assert "test_runner_url_arg_safety_rejects_secret_bearing_urls" in helper_test
    assert "test_require_runner_url_args_rejects_unsafe_urls_without_leaking" in helper_test
    assert "test_require_runner_url_args_rejects_malformed_field_name" in helper_test
    assert (
        "test_runner_passthrough_arg_safety_rejects_secret_like_arguments"
        in helper_test
    )
    assert (
        "test_require_runner_passthrough_args_rejects_unsafe_values_without_leaking"
        in helper_test
    )
    assert (
        "test_require_runner_passthrough_args_rejects_malformed_containers"
        in helper_test
    )
    assert (
        "test_require_runner_passthrough_args_rejects_malformed_field_name"
        in helper_test
    )
    assert "test_runner_path_inspectors_sanitize_malformed_path_labels" in helper_test
    assert "test_runner_path_inspectors_sanitize_noncanonical_failures" in helper_test
    assert "test_runner_path_size_rejects_symlink_before_stat" in helper_test
    assert "test_runner_path_size_rejects_parent_symlink_before_stat" in helper_test
    assert "test_runner_path_size_uses_no_follow_descriptor_open" in helper_test
    assert "test_runner_path_size_open_failure_is_sanitized" in helper_test
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
    assert "test_input_file_symlink_fails" in helper_test
    assert "test_input_file_parent_symlink_fails" in helper_test
    assert "test_input_file_parent_chain_symlink_fails" in helper_test
    assert "test_input_file_symlink_inspection_failure_is_reported" in helper_test
    assert "test_input_file_rejects_malformed_label" in helper_test
    assert "test_input_file_rejects_malformed_seen_identity_map" in helper_test
    assert (
        "test_input_directory_rejects_scalar_and_mapping_path_collections"
        in helper_test
    )
    assert "test_missing_input_directory_sanitizes_noncanonical_path" in helper_test
    assert "test_input_directory_symlink_fails" in helper_test
    assert "test_input_directory_parent_symlink_fails" in helper_test
    assert "test_input_directory_parent_chain_symlink_fails" in helper_test
    assert "test_input_directory_symlink_inspection_failure_is_reported" in helper_test
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
    assert "test_existing_out_dir_parent_chain_symlink_fails_preflight" in helper_test
    assert "test_summary_out_directory_inspection_failure_fails_preflight" in helper_test
    assert "test_summary_out_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_symlink_fails_preflight" in helper_test
    assert "test_summary_out_parent_chain_symlink_fails_preflight" in helper_test
    assert "test_existing_summary_out_parent_symlink_fails_preflight" in helper_test
    assert (
        "test_existing_summary_out_parent_chain_symlink_fails_preflight"
        in helper_test
    )
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
    assert (
        "test_validate_command_plan_artifacts_rejects_reserved_output_symlink"
        in helper_test
    )
    assert (
        "test_validate_command_plan_artifacts_rejects_reserved_output_parent_symlink"
        in helper_test
    )
    assert (
        "test_validate_command_plan_artifacts_reserved_output_symlink_inspection_failure"
        in helper_test
    )
    assert (
        "test_validate_command_plan_artifacts_stops_after_reserved_output_symlink"
        in helper_test
    )
    assert "test_planned_artifact_symlink_fails" in helper_test
    assert "test_planned_artifact_parent_symlink_fails" in helper_test
    assert "test_planned_artifact_parent_chain_symlink_fails" in helper_test
    assert "test_planned_artifact_existing_file_fails" in helper_test
    assert "test_run_command_plan_rejects_output_dir_symlink_before_launch" in helper_test
    assert (
        "test_run_command_plan_rejects_output_dir_symlink_written_by_command"
        in helper_test
    )
    assert (
        "test_run_command_plan_rejects_output_dir_removed_by_command" in helper_test
    )
    assert "test_run_command_plan_rejects_artifact_parent_symlink_before_create" in helper_test
    assert (
        "test_run_command_plan_rejects_artifact_parent_chain_symlink_before_create"
        in helper_test
    )
    assert (
        "test_run_command_plan_rejects_artifact_parent_symlink_written_by_command"
        in helper_test
    )
    assert "test_run_command_plan_rejects_empty_artifact_written_by_command" in helper_test
    assert "test_runner_path_resolution_uses_shared_identity_helper" in helper_test
    assert missing == []


def test_url_rendering_runners_use_shared_url_preflight() -> None:
    expected = {
        "run_sorafs_ai_prescreen_rollout_evidence.py": (
            "runner_url",
            "committee_url",
            "operator_url",
            "notification_webhook_url",
        ),
        "run_sorafs_reputation_rollout_evidence.py": ("torii_url",),
        "run_sorafs_transparency_rollout_evidence.py": ("torii_url",),
    }
    missing: list[str] = []
    for runner_name, fields in expected.items():
        source = read(SCRIPTS_DIR / runner_name)
        if "require_runner_url_args" not in source:
            missing.append(f"{runner_name}: missing require_runner_url_args")
        for field in fields:
            if field not in source:
                missing.append(f"{runner_name}: missing {field}")

    assert "test_service_url_rejects_secret_bearing_url_without_leaking" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_ai_prescreen_rollout_evidence_test.py"
    )
    assert "test_torii_url_rejects_secret_bearing_url_without_leaking" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_reputation_rollout_evidence_test.py"
    )
    assert "test_torii_url_rejects_secret_bearing_url_without_leaking" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_transparency_rollout_evidence_test.py"
    )
    assert missing == []


def test_passthrough_arg_runners_use_shared_passthrough_preflight() -> None:
    expected = {
        "run_sorafs_ai_prescreen_rollout_evidence.py": (
            "sorafs_cli_bin",
            "iroha_bin",
            "iroha_arg",
        ),
        "run_sorafs_reputation_rollout_evidence.py": ("sorafs_cli_bin",),
        "run_sorafs_transparency_rollout_evidence.py": ("iroha_bin", "iroha_arg"),
    }
    regressions = {
        "run_sorafs_ai_prescreen_rollout_evidence_test.py": (
            "test_iroha_arg_rejects_secret_bearing_value_without_leaking"
        ),
        "run_sorafs_reputation_rollout_evidence_test.py": (
            "test_sorafs_cli_bin_rejects_secret_bearing_path_without_leaking"
        ),
        "run_sorafs_transparency_rollout_evidence_test.py": (
            "test_iroha_arg_rejects_secret_bearing_value_without_leaking"
        ),
    }
    missing: list[str] = []
    for runner_name, fields in expected.items():
        source = read(SCRIPTS_DIR / runner_name)
        if "require_runner_passthrough_args" not in source:
            missing.append(f"{runner_name}: missing require_runner_passthrough_args")
        for field in fields:
            if field not in source:
                missing.append(f"{runner_name}: missing {field}")
    for test_name, regression in regressions.items():
        if regression not in read(SCRIPTS_DIR / "tests" / test_name):
            missing.append(f"{test_name}: missing {regression}")

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
    assert "import os" in helper
    assert "def convert_arg_line_to_args" in helper
    assert "shlex.split(line, comments=True)" in helper
    assert "MAX_RESPONSE_ARGFILE_BYTES" in helper
    assert "MAX_RESPONSE_ARGFILE_DEPTH" in helper
    assert "MAX_EXPANDED_ARGS" in helper
    assert "RESPONSE_ARGFILE_CHUNK_BYTES" in helper
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
    assert "def _response_argfile_open_flags" in helper
    assert "getattr(os, \"O_NOFOLLOW\", 0)" in helper
    assert "def _validate_response_argfile_parent_chain" in helper
    assert "def _read_response_argfile_bytes" in helper
    assert "path.is_symlink()" in helper
    assert (
        'ARGFILE_PARENT_SYMLINK_DIAGNOSTIC = '
        '"@ARGFILE parent must not be a symlink"'
    ) in helper
    assert 'ARGFILE_PARENT_DIRECTORY_DIAGNOSTIC = (' in helper
    assert '"@ARGFILE parent must be a directory when it exists"' in helper
    assert (
        'ARGFILE_PARENT_INSPECTION_DIAGNOSTIC = '
        '"@ARGFILE parent cannot be inspected"'
    ) in helper
    assert 'ARGFILE_SYMLINK_DIAGNOSTIC = "@ARGFILE must not be a symlink"' in helper
    assert 'ARGFILE_MISSING_DIAGNOSTIC = "@ARGFILE must exist and be a file"' in helper
    assert 'ARGFILE_INSPECTION_DIAGNOSTIC = "@ARGFILE cannot be inspected"' in helper
    assert 'ARGFILE_READ_DIAGNOSTIC = "@ARGFILE cannot be read"' in helper
    assert 'ARGFILE_RESOLUTION_DIAGNOSTIC = "@ARGFILE cannot be resolved"' in helper
    assert "os.open(path, _response_argfile_open_flags())" in helper
    assert "os.fstat(fd).st_size" in helper
    assert "os.fdopen(fd, \"rb\")" in helper
    assert "os.close(fd)" in helper
    assert "must exist and be a file" in helper
    assert "@ARGFILE `{path_label}` must not be a symlink" not in helper
    assert "@ARGFILE parent `{parent_label}` must not be a symlink" not in helper
    assert "failed to resolve @ARGFILE" not in helper
    assert "failed to stat @ARGFILE" not in helper
    assert "failed to read @ARGFILE" not in helper
    assert "failed to stat @ARGFILE `{path}`: {error}" not in helper
    assert "failed to read @ARGFILE `{path}`: {error}" not in helper
    assert "@ARGFILE `{path}` must be UTF-8: {error}" not in helper
    assert "@ARGFILE `{path}` line {line_number}: {error}" not in helper
    assert "error_diagnostic_label(error, path_label=path_label)" not in helper
    assert "path_diagnostic_label(path)" not in helper
    assert "path.read_bytes()" not in helper
    assert "path.stat().st_size" not in helper
    assert "RuntimeError" in helper
    assert '"@ARGFILE line {}: {}".format(' in helper
    assert "recursive @ARGFILE" in helper
    assert "must be UTF-8" in helper
    assert "test_response_file_stat_failure_is_stable_value_error" in helper_test
    assert "test_response_file_read_failure_is_stable_value_error" in helper_test
    assert "test_symlink_response_file_fails_before_read" in helper_test
    assert "test_response_file_parent_symlink_fails_before_read" in helper_test
    assert "test_response_file_read_uses_no_follow_open_flags" in helper_test
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
    assert "import os" in helper
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
    assert "def checker_summary_write_open_flags" in helper
    assert "def write_all_checker_summary_bytes" in helper
    assert "view = memoryview(payload)" in helper
    assert "written = os.write(fd, view)" in helper
    assert "os.open(summary_out, checker_summary_write_open_flags(), 0o666)" in helper
    assert 'write_all_checker_summary_bytes(fd, summary_text.encode("utf-8"))' in helper
    assert "os.fdopen(fd, \"w\", encoding=\"utf-8\")" not in helper
    assert "summary_out.write_text" not in helper
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
    assert "test_write_checker_summary_uses_no_follow_descriptor_open" in helper_test
    assert (
        "test_write_checker_summary_completes_partial_descriptor_writes"
        in helper_test
    )
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
    assert "EVIDENCE_FILE_SOURCE_OVERLAP_DIAGNOSTIC" in helper
    assert "evidence file provided by multiple evidence sources" in helper
    assert "both --evidence and --evidence-dir" not in helper
    assert "duplicate evidence file" in helper
    assert "def inspect_evidence_file" in helper
    assert "def validate_evidence_parent_chain" in helper
    assert "evidence_label = _require_label(label)" in helper
    assert "parent_label = f\"{evidence_label} parent\"" in helper
    assert "EVIDENCE_FILE_PATH_DIAGNOSTIC" in helper
    assert "path.is_symlink()" in helper
    assert "parent.is_symlink()" not in helper
    assert "EVIDENCE_FILE_PARENT_SYMLINK_DIAGNOSTIC" not in helper
    assert "EVIDENCE_DIRECTORY_PARENT_SYMLINK_DIAGNOSTIC" not in helper
    assert "EVIDENCE_FILE_SYMLINK_DIAGNOSTIC" in helper
    assert "must exist and be a file" in helper
    assert "must exist and be a directory" in helper
    assert "EVIDENCE_DIRECTORY_PATH_DIAGNOSTIC" in helper
    assert "EVIDENCE_DIRECTORY_SYMLINK_DIAGNOSTIC" in helper
    assert "def inspect_evidence_directory" in helper
    assert "def scan_evidence_directory_json" in helper
    assert "inspect_evidence_directory(directory, error_list)" in helper
    assert "cannot be inspected" in helper
    assert "EVIDENCE_DIRECTORY_SCAN_DIAGNOSTIC" in helper
    assert "failed to scan evidence directory" not in helper
    assert "reserved_output_paths" in helper
    assert "reserved_output_path_identities" in helper
    assert "validate_evidence_parent_chain(path, error_list, label=path_label)" in helper
    assert "reserved_error_count = len(error_list)" in helper
    assert "record_reserved_output_evidence_conflicts" in helper
    assert "inspect_evidence_file(path, error_list)" in helper
    assert "reserved output" in helper
    assert "conflicts with reserved output" in helper
    assert "resolve_path_identity" in helper
    assert "resolution_errors: list[str] = []" in helper
    assert "EVIDENCE_PATH_RESOLUTION_DIAGNOSTIC" in helper
    assert "resolve_path_identity(path, resolution_errors, label=path_label)" in helper
    assert "RuntimeError" in path_identity_helper
    assert "evidence_path_identities" in helper
    assert "if error_list:\n        return identities" in helper
    assert "is_explicit_evidence_path" in helper
    assert "if identities is None or not identities:" in helper
    assert "inspect_evidence_file(path, errors)" in helper
    assert "def _evidence_path_identity_set" in helper
    assert "test_missing_explicit_evidence_file_fails_closed" in helper_test
    assert "test_explicit_evidence_directory_fails_closed" in helper_test
    assert "test_explicit_evidence_symlink_fails_closed" in helper_test
    assert (
        "test_explicit_evidence_parent_symlink_directory_is_accepted"
        in helper_test
    )
    assert "test_discovered_json_directory_fails_closed_without_hiding_files" in helper_test
    assert "test_discovered_json_symlink_fails_closed_without_hiding_files" in helper_test
    assert "test_noncanonical_evidence_file_labels_are_sanitized" in helper_test
    assert "test_evidence_file_inspection_failure_fails_closed" in helper_test
    assert "test_evidence_file_symlink_inspection_failure_fails_closed" in helper_test
    assert "test_evidence_file_parent_inspection_failure_fails_closed" in helper_test
    assert "test_discovered_evidence_file_inspection_failure_fails_closed" in helper_test
    assert "test_evidence_directory_symlink_fails_closed" in helper_test
    assert (
        "test_evidence_directory_parent_symlink_directory_is_accepted"
        in helper_test
    )
    assert (
        "test_evidence_directory_symlink_inspection_failure_fails_closed"
        in helper_test
    )
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
        "test_discover_evidence_files_stops_after_reserved_output_symlink"
        in helper_test
    )
    assert (
        "test_evidence_path_identities_rejects_malformed_path_collections"
        in helper_test
    )
    assert "test_evidence_path_identities_rejects_symlink_identity" in helper_test
    assert (
        "test_is_explicit_evidence_path_rejects_symlink_candidate" in helper_test
    )
    assert (
        "test_is_explicit_evidence_path_accepts_parent_symlink_candidate"
        in helper_test
    )
    assert (
        "test_is_explicit_evidence_path_skips_empty_identity_set_without_resolving"
        in helper_test
    )
    assert (
        "test_evidence_path_identities_skip_after_discovery_errors" in helper_test
    )
    assert "test_non_path_evidence_directory_fails_closed_without_traceback" in helper_test
    assert "test_evidence_directory_inspection_failure_fails_closed" in helper_test
    assert "test_evidence_directory_scan_failure_fails_closed" in helper_test
    assert "test_scan_evidence_directory_json_rejects_file_path" in helper_test
    assert (
        "test_scan_evidence_directory_json_rejects_symlink_directory" in helper_test
    )
    assert (
        "test_scan_evidence_directory_json_accepts_parent_symlink_directory"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_non_path_directory_fails_closed" in helper_test
    )
    assert (
        "test_reserved_output_conflict_evidence_directory_symlink_fails_closed"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_evidence_directory_parent_symlink_is_accepted"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_explicit_evidence_directory_fails_closed"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_explicit_evidence_symlink_fails_closed"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_explicit_evidence_parent_symlink_is_accepted"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_discovered_json_directory_fails_closed"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_discovered_json_symlink_fails_closed"
        in helper_test
    )
    assert (
        "test_reserved_output_conflict_scan_inspection_failure_fails_closed"
        in helper_test
    )
    assert "test_reserved_output_conflict_scan_failure_fails_closed" in helper_test
    assert (
        "test_reserved_output_conflict_scan_accepts_reserved_output_parent_symlink"
        in helper_test
    )
    assert "test_reserved_output_path_identities_rejects_symlink" in helper_test
    assert (
        "test_reserved_output_path_identities_accepts_parent_symlink_directory"
        in helper_test
    )
    assert (
        "test_reserved_output_path_identities_symlink_inspection_failure"
        in helper_test
    )
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
    assert "import os" in helper
    assert "from sorafs_evidence_paths import (" in helper
    assert "EVIDENCE_FILE_INSPECTION_DIAGNOSTIC" in helper
    assert "EVIDENCE_FILE_MISSING_DIAGNOSTIC" in helper
    assert "EVIDENCE_FILE_SYMLINK_DIAGNOSTIC" in helper
    assert "validate_evidence_parent_chain" in helper
    assert "from sorafs_path_identity import" in helper
    assert "path_diagnostic_label(" not in helper
    assert "error_diagnostic_label(" in helper
    assert "def _canonical_diagnostic_text" not in helper
    assert "def _evidence_path_label" not in helper
    assert "def _error_label" in helper
    assert "evidence path must be a path" in helper
    assert "<non-path>" in path_identity_helper
    assert "<non-canonical-path>" in path_identity_helper
    assert "<non-canonical-error>" in path_identity_helper
    assert "evidence byte limit must be positive" in helper
    assert "def validate_evidence_file_for_read" in helper
    assert "path.is_symlink()" in helper
    assert "validate_evidence_parent_chain(" in helper
    assert "path.is_file()" in helper
    assert "validate_evidence_file_for_read(path)" in helper
    assert "def evidence_read_open_flags" in helper
    assert "getattr(os, \"O_NOFOLLOW\", 0)" in helper
    assert "os.open(path, evidence_read_open_flags())" in helper
    assert "os.fdopen(fd, \"rb\")" in helper
    assert "os.close(fd)" in helper
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
    assert "test_read_evidence_bytes_rejects_symlink_before_open" in helper_test
    assert "test_read_evidence_bytes_rejects_directory_before_open" in helper_test
    assert (
        "test_load_evidence_json_with_sha256_or_record_error_accepts_parent_symlink"
        in helper_test
    )
    assert (
        "test_validate_evidence_file_for_read_records_inspection_failure"
        in helper_test
    )
    assert "test_read_evidence_bytes_uses_no_follow_open_flags" in helper_test
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
    assert "unknown reputation evidence schema" in reputation
    assert "unknown schema `{schema}`" not in reputation
    assert "path_diagnostic_label" not in reputation
    assert "EXPLICIT_KIND_CONFLICT_DIAGNOSTIC" in reputation
    assert "EXPLICIT_KIND_SCHEMA_MISMATCH_DIAGNOSTIC" in reputation
    assert "evidence schema does not match explicit kind" in reputation
    assert "explicit kind `{}`" not in reputation
    assert "belongs to `{}`" not in reputation
    assert 'raise ValueError("unknown evidence kind")' in reputation
    assert "unknown evidence kind `{kind}`" not in reputation
    assert 'errors.append("unsupported evidence kind")' in reputation
    assert "unsupported evidence kind `{evidence.kind}`" not in reputation
    assert 'record_kind = "<unknown>"' in reputation
    assert "missing provider/proof evidence for required provider" in reputation
    assert "missing provider/proof evidence for `{provider_id}`" not in reputation
    assert 'f"{path}: schema' not in reputation
    assert 'f"{path}: cannot infer evidence kind"' not in reputation
    assert "json.JSONDecodeError" not in reputation
    assert "evidence file `{path}` must exist" not in reputation
    assert "if not path.is_file()" not in reputation
    assert "path.read_bytes()" not in reputation
    assert "hashlib.sha256(data).hexdigest()" not in reputation
    reputation_test = read(
        SCRIPTS_DIR / "tests" / "check_sorafs_reputation_rollout_evidence_test.py"
    )
    assert "private-key-placeholder" in reputation_test
    assert "unknown_schema not in" in reputation_test
    assert "test_unknown_explicit_evidence_kind_does_not_echo_kind" in reputation_test
    assert "test_unsupported_loaded_evidence_kind_is_sanitized" in reputation_test
    assert "provider-b-private-key-placeholder" in reputation_test
    assert missing == []
    assert local_load_error_recorders == []


def test_rollout_checkers_use_shared_artifact_rows_and_fingerprints() -> None:
    missing = [
        path.name
        for path in standard_artifact_checkers()
        if "archive_artifact_path_label," not in read(path)
        or "build_evidence_artifact," not in read(path)
        or "FINGERPRINT_FIELDS: tuple[str, ...]" not in read(path)
        or "build_evidence_artifact(" not in read(path)
        or "archive_artifact_path_label(path, evidence_dirs)" not in read(path)
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
    assert "if value is not None:" in helper
    assert "fingerprint[field] = value" in helper
    assert "test_artifact_fingerprint_selects_present_fields_in_order" in helper_test
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
    assert "def archive_artifact_path_label" in validation_helper
    assert "def is_archive_portable_artifact_path" in validation_helper
    assert "resolve_path_identity(" in validation_helper
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
        "test_archive_artifact_path_label_prefers_evidence_directory_relative_path"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_archive_artifact_path_label_falls_back_to_safe_basename"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_archive_artifact_path_rejects_nonportable_labels"
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
        or "recognized_evidence_artifacts," not in read(path)
        or (
            '"recognized_artifact_count": count_evidence_artifacts(artifacts_by_kind)'
            not in read(path)
        )
        or (
            '"recognized_artifacts": recognized_evidence_artifacts(artifacts_by_kind)'
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
    assert "def recognized_evidence_artifacts" in helper
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
    assert "test_recognized_evidence_artifacts_flatten_kind_buckets" in helper_test
    assert (
        "test_recognized_evidence_artifacts_rejects_malformed_buckets_without_traceback"
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
    reputation_checker = read(SCRIPTS_DIR / "check_sorafs_reputation_rollout_evidence.py")
    assert "count_evidence_files," in reputation_checker
    assert '"evidence_file_count": count_evidence_files(files)' in reputation_checker
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
            "validate_standard_evidence_payload("
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
    assert "schema is not a recognized {artifact_label}" in helper
    assert "schema `{schema}` is not a recognized" not in helper
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
    assert "private-key-placeholder" in helper_test
    assert "unknown_schema not in" in helper_test
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
    def uses_custom_shared_environment_validation(path: Path) -> bool:
        source = read(path)
        return (
            path.name == "check_sorafs_reputation_rollout_evidence.py"
            and "require_rollout_environment," in source
            and "require_rollout_environment(payload, errors)" in source
        )

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
        and not uses_custom_shared_environment_validation(path)
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
    def uses_custom_shared_deployment_id_validation(path: Path) -> bool:
        source = read(path)
        return (
            path.name == "check_sorafs_reputation_rollout_evidence.py"
            and "require_rollout_deployment_id," in source
            and "require_rollout_deployment_id(payload, errors)" in source
            and "require_rollout_deployment_context_review," in source
            and "require_rollout_deployment_context_review(payload, errors)" in source
        )

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
        and not uses_custom_shared_deployment_id_validation(path)
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
    assert '"canary"' in helper
    assert '"dryrun"' in helper
    assert '"experimental"' in helper
    assert '"nonproduction"' in helper
    assert '"notprod"' in helper
    assert "ROLLOUT_DEPLOYMENT_REVIEW_LABELS" in helper
    assert "FORBIDDEN_ROLLOUT_DEPLOYMENT_JOINED_MARKERS" in helper
    assert "joined_forbidden" in helper
    assert 'f"{marker}{label}"' in helper
    assert 'f"{label}{marker}"' in helper
    assert "marker not in tokens and joined in compact" in helper
    assert "compact = \"\".join(tokens)" in helper
    assert "require_rollout_deployment_id(payload, errors)" in helper
    assert (
        "test_require_rollout_deployment_id_rejects_noncanonical_values"
        in helper_test
    )
    assert (
        "test_require_rollout_deployment_id_rejects_joined_nonproduction_aliases"
        in helper_test
    )
    assert '"repair-testrelease-202606"' in helper_test
    assert '"localproduction-gateway-202606"' in helper_test
    assert '"releaselocal-reputation-202606"' in helper_test
    assert (
        "test_require_rollout_deployment_id_rejects_synthetic_rollout_markers"
        in helper_test
    )
    assert '"gateway-prod-dry-run-202606"' in helper_test
    assert '"reference-releaseexperimental-202606"' in helper_test
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
            and "archive_route_state must be `active` or `retired`" in read(path)
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
    assert "required {name} {evidence_label} artifacts" not in helper
    assert "{evidence_label} required artifacts must be a sequence" in helper
    assert "required `{name}` artifacts must be a sequence of artifact objects" in helper
    assert (
        "{evidence_label} required artifacts must be a sequence "
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
    assert "missing required {name} {evidence_label} evidence" not in helper
    assert "missing required {evidence_label} evidence" in helper
    assert "{name} {evidence_label} evidence has invalid artifact(s)" not in helper
    assert "{evidence_label} evidence has invalid artifact(s)" in helper
    assert "evidence_artifact_is_valid(artifact)" in helper
    assert (
        "test_build_required_evidence_summary_rejects_malformed_artifact_rows"
        in read(SCRIPTS_DIR / "tests" / "sorafs_evidence_validation_test.py")
    )
    assert (
        "test_build_required_evidence_summary_duplicate_diagnostics_are_payload_free"
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
    assert "def _diagnostic_mentions_missing_value" in helper
    assert "validation missing required evidence message" in helper
    assert "must not include missing value" in helper
    assert (
        "test_record_missing_required_evidence_value_errors_rejects_malformed_labels"
        in helper_test
    )
    assert (
        "test_record_missing_required_evidence_value_errors_rejects_value_echoing_messages"
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
    assert "does not match previous value" in helper
    assert "does not match `{previous}`" not in helper
    assert "`{value_label}`" not in helper
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
    assert "archive_artifact_path_label," in reputation
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
    assert "archive_artifact_path_label(evidence.path, evidence_dirs or [])" in reputation
    assert (
        'finalize_custom_required_evidence_rows(required, evidence_label="evidence")'
        in reputation
    )
    assert "def finalize_reputation_required_rows(" in reputation
    assert "finalize_reputation_required_rows(required)" in reputation
    assert 'row["present"] = artifact_count > 0' in reputation
    assert 'row["artifact_count"] = artifact_count' in reputation
    assert "validate_common_rollout_context(payload, errors)" in reputation
    assert '"deployment_id"' in reputation
    assert '"environment"' in reputation
    assert '"deployment_context_reviewed"' in reputation
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


def test_rollout_checkers_keep_detail_metadata_out_of_artifact_rows() -> None:
    helper = read(EVIDENCE_VALIDATION_HELPER)
    hedging = read(SCRIPTS_DIR / "check_sorafs_hedging_rollout_evidence.py")
    reserve = read(SCRIPTS_DIR / "check_sorafs_reserve_rent_rollout_evidence.py")
    appeal = read(SCRIPTS_DIR / "check_sorafs_appeal_finance_rollout_evidence.py")

    assert "def evidence_artifact_detail" in helper
    assert "field: Any" in helper
    assert "label_name=\"artifact detail field\"" in helper
    assert "artifact.get(field_label)" in helper
    assert 'artifact["cycle"]' not in hedging
    assert 'artifact["bake"]' not in reserve
    assert 'artifact["run"]' not in appeal
    assert "BILLING_CYCLE_DETAIL_FIELDS: tuple[str, ...]" in hedging
    assert "evidence_artifact_fingerprint(artifact)" in hedging
    assert "valid_provider_bakes.append(provider_bake_fingerprint(payload))" not in reserve
    assert "valid_provider_bakes.append(bake)" in reserve
    assert "valid_multi_peer_runs.append(reconciliation_fingerprint(payload))" not in appeal
    assert "valid_multi_peer_runs.append(run)" in appeal
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
    assert "duplicate default required evidence kind" in helper
    assert "unknown required evidence kind" in helper
    assert "duplicate required evidence kind `{candidate}`" not in helper
    assert "duplicate default required evidence kind `{candidate}`" not in helper
    assert "unknown required evidence kind `{candidate}`" not in helper
    assert "unknown default required evidence kind `{candidate}`" not in helper
    assert "test_malformed_required_kind_values_fail" in helper_test
    assert "test_non_string_required_kind_value_fails" in helper_test
    assert "test_malformed_required_kind_name_text_fails" in helper_test
    assert "unknown-private-key-placeholder" in helper_test
    assert "default-private-key-placeholder" in helper_test
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
    assert "def _diagnostic_path_segment" in helper
    assert "def _join_diagnostic_path" in helper
    assert "<non-canonical-key>" in helper
    assert "<non-string-key>" in helper
    assert "<sensitive-key>" in helper
    assert (
        'errors.append(f"{visit_path} must not be present in {evidence_label}")'
        in helper
    )
    assert (
        'errors.append(f"{child_path} must not be present in {evidence_label}")'
        not in helper
    )
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
    assert "def _is_sensitive_inclusion_marker" in helper
    assert "def _inclusion_marker_stem_variants" in helper
    assert "def _is_payload_free_sensitive_reference" in helper
    assert "fragment in normalized_key" in helper
    assert "normalized_key in normalized_keys" in helper
    assert "normalized_key.endswith(\"included\")" in helper
    assert "len(normalized_key) > len(\"included\")" not in helper
    assert "<sensitive-inclusion-marker>" in helper
    assert "marker_path = _join_diagnostic_path(" in helper
    assert "child_path} must be false" not in helper
    assert "marker_path} must be false" in helper
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
    assert "test_noncanonical_sensitive_key_paths_are_sanitized" in helper_test
    assert (
        "test_sensitive_parent_path_is_redacted_for_nested_diagnostics"
        in helper_test
    )
    assert "test_sensitive_inclusion_marker_key_names_are_redacted" in helper_test
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


def test_pop_credentials_docs_do_not_publish_unshipped_operator_commands() -> None:
    commands = (
        "sorafs pop sync",
        "sorafs pop status",
        "sorafs pop prove",
        "sorafs pop revoke",
    )
    violations: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs*.md")):
        lines = read(path).splitlines()
        for index, line in enumerate(lines):
            matched = [command for command in commands if command in line]
            if not matched:
                continue
            context = " ".join(lines[max(0, index - 1) : index + 3]).lower()
            if "do not document" in context and "shipped command" in context:
                continue
            violations.setdefault(str(path.relative_to(REPO_ROOT)), []).extend(matched)

    assert violations == {}


def test_pop_credentials_runtime_services_stay_open_in_docs() -> None:
    source = read(SORAFS_POP_CREDENTIALS_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "SFM-4b1 now has local PoP credential payload foundations, but it is not shipped as a complete SoraFS proof-of-personhood credential service.",
        "It does not yet contain the enrollment portal, credential issuer daemon, credential registry service, juror wallet, privacy-preserving ZK membership proof generator, or deployed SoraFS verifier service described by the original plan.",
        "This checker is a rollout gate; it does not replace the missing runtime services or privacy proof backend.",
        "Enrollment portal | Captures candidate attestations and issuer approvals. | Not shipped.",
        "Credential issuer | Signs credentials, updates commitment roots, and publishes rollups. | Payload signatures and a local issued-credential bundle helper are shipped; service is not shipped.",
        "Credential registry | Stores commitment roots, revocation updates, and event digests. | Payload schemas and local bundle validation are shipped; service is not shipped.",
        "Juror client | Stores credentials, syncs revocations, and generates proofs. | Not shipped.",
        "Verification service | Validates juror proofs for sortition, voting, and appeal panels. | Local transcript-policy payload verifier, production fail-closed proof verifier, `sorafs-validate pop`, and SDK/bridge reference gate shipped; deployed service and ZK verifier are not shipped.",
        "Build the issuer and registry services, including key management, revocation updates, commitment-root publication, and audit digests.",
        "Build juror client storage, revocation sync, proof generation, and local credential rotation.",
        "Publish operator and juror docs only after the service CLI/API and verifier paths exist.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_pop_credentials_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "prints a dry-run command plan with the checker-backed `evidence_contract` map for the selected required kinds",
        "The checker exports those required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS` for downstream automation.",
        "The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_pop_credentials_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_pop_credentials_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_pop_credentials_canary.py")
    builder_test = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_pop_credentials_canary_test.py"
    )
    issuer_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_pop_credentials_issuer_canary.args.example"
    )
    verifier_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_pop_credentials_verifier_canary.args.example"
    )
    docs = read(SORAFS_POP_CREDENTIALS_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_ENROLLMENT_ROUTES" in builder
    assert "REQUIRED_VERIFIER_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "credential_payloads_included" in builder
    assert "holder_identities_included" in builder
    assert "raw_proofs_included" in builder
    assert "response_bodies_included" in builder
    assert "test_generated_canaries_pass_full_pop_gate" in builder_test
    assert "test_transcript_digest_privacy_backend_fails_before_write" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind issuer_bundle" in issuer_example
    assert "--verified-claim issuer_key_policy_verified" in issuer_example
    assert "--kind verifier_service" in verifier_example
    assert "--route proof_verify" in verifier_example
    assert "--policy-digest-hex" in verifier_example
    assert "build_sorafs_pop_credentials_canary.py" in docs
    assert "payload-free SFM-4b1 PoP credential canary builder" in docs


def test_unshipped_pop_credentials_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/pop/enrollment",
        "/v1/sorafs/pop/enrollment-portal",
        "/v1/sorafs/pop/issuer",
        "/v1/sorafs/pop/credential-issuer",
        "/v1/sorafs/pop/registry",
        "/v1/sorafs/pop/credential-registry",
        "/v1/sorafs/pop/juror-client",
        "/v1/sorafs/pop/juror-wallet",
        "/v1/sorafs/pop/proof-generator",
        "/v1/sorafs/pop/verifier",
        "/v1/sorafs/pop/verifier-service",
        "/v1/sorafs/pop/promotion",
    )
    unshipped_cli_subcommands = (
        "pop-enrollment",
        "pop-enrollment-portal",
        "pop-issuer",
        "pop-issuer-serve",
        "pop-registry",
        "pop-registry-serve",
        "pop-juror-client",
        "pop-juror-wallet",
        "pop-proof-generator",
        "pop-verifier-service",
        "pop-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_pop_membership_production_verifier_remains_fail_closed() -> None:
    source = read(POP_CREDENTIALS_RS)
    start = source.index("pub fn verify_pop_membership_proof_v1")
    end = source.index("pub fn verify_pop_membership_transcript_policy_v1", start)
    production_verifier = source[start:end]

    assert "The production verifier is fail-closed" in source
    assert "PopMembershipProofSystemV1::TranscriptDigestV1" in production_verifier
    assert "Err(PopCredentialValidationError::PolicyOnlyProofSystem)" in production_verifier
    assert "verify_pop_membership_transcript_policy_v1(" not in production_verifier
    assert "Ok(())" not in production_verifier


def test_unshipped_sorafs_operator_commands_stay_warning_only() -> None:
    commands = (
        "sorafs pdp challenge",
        "sorafs pdp fetch",
        "sorafs pdp respond",
        "sorafs pdp verify",
        "sorafs pdp status",
        "sorafs pdp export",
        "sorafs moderation jury-accept",
        "sorafs moderation open-case",
    )
    violations: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs*.md")):
        lines = read(path).splitlines()
        for index, line in enumerate(lines):
            matched = [command for command in commands if command in line]
            if not matched:
                continue
            context = " ".join(lines[max(0, index - 12) : index + 6]).lower()
            warning_context = (
                "not shipped yet" in context
                or ("do not document" in context and "shipped" in context)
                or ("do not document" in context and "operator-ready" in context)
            )
            if not warning_context:
                violations.setdefault(str(path.relative_to(REPO_ROOT)), []).extend(
                    matched
                )

    assert violations == {}


def test_unshipped_public_routes_stay_warning_only() -> None:
    route_patterns = (
        "/v1/transparency/",
        "/v1/evidence/session",
        "/v1/evidence/manifest",
        "/v1/evidence/log",
        "/v1/evidence/audit",
    )
    violations: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.rglob("*sorafs*.md")):
        lines = read(path).splitlines()
        for index, line in enumerate(lines):
            matched = [route for route in route_patterns if route in line]
            if not matched:
                continue
            context = " ".join(lines[max(0, index - 2) : index + 3]).lower()
            transparency_warning = (
                "/v1/transparency/" in matched
                and "do not document generic" in context
                and "shipped until" in context
            )
            evidence_warning = (
                any(route.startswith("/v1/evidence/") for route in matched)
                and "no production route should claim support" in context
                and "until the" in context
            )
            if not (transparency_warning or evidence_warning):
                violations.setdefault(str(path.relative_to(REPO_ROOT)), []).extend(
                    matched
                )

    assert violations == {}


def test_unshipped_public_routes_are_not_exposed_by_torii() -> None:
    route_patterns = (
        "/v1/transparency/",
        "/v1/evidence/session",
        "/v1/evidence/manifest",
        "/v1/evidence/log",
        "/v1/evidence/audit",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_ai_prescreen_deployed_workflow_services_stay_open_in_docs() -> None:
    source = read(SORAFS_AI_PRESCREEN_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "It does not yet ship captured deployed juror notification transport service rollout evidence, captured deployed commit/reveal executor job rollout evidence, or end-to-end release workflow as runnable services.",
        "`scripts/check_sorafs_ai_prescreen_rollout_evidence.py` fails closed until the deployed runner, committee, operator workflow, juror notification transport, commit/reveal executor, moderation transparency source entries, Governance DAG binding, and full workflow artifacts are captured.",
        "requires operator workflow, notification transport, commit/reveal executor, transparency publication, and Governance DAG evidence to carry the same `workflow_digest_hex` as the end-to-end workflow artifact",
        "Captured deployed juror notification transport service rollout evidence and deployed commit/reveal executor job rollout evidence.",
        "End-to-end ingest -> quarantine -> appeal -> transparency workflow services and the corresponding live evidence bundle required by `scripts/check_sorafs_ai_prescreen_rollout_evidence.py`.",
        "These commands and service do not replace deployed juror notification transport or captured live executor job evidence.",
        "Wire deployed bridge automation to operate juror notification transport jobs, run the shipped notification transport canary against the deployed transport, install/run the generated commit/reveal executor job bundles, run the shipped executor canary against captured payload-free execution summaries, and publish transparency entries",
        "Update the portal and OpenAPI/operator docs only after the above commands and services exist.",
        "Remaining rollout work is captured deployed juror notification transport service rollout evidence, captured deployed commit/reveal executor job rollout evidence, and a live bundle that passes the AI pre-screening rollout evidence gate",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_ai_prescreen_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`.",
        "its dry-run JSON includes the checker-backed `evidence_contract` map with the schema and required payload fields for every SFM-4a evidence kind, and the runner validates the schema-closed collection plan, external evidence map, evidence contract, and command steps before dry-run output or live canaries.",
        "It also rejects duplicate or unsupported `--source-entry` kinds before dry-run output or live canaries.",
        "cross-artifact runner/workflow binding failures are reflected on the offending artifacts in the emitted summary.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_ai_prescreen_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_ai_prescreen_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_ai_prescreen_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_ai_prescreen_canary_test.py")
    plan = read(SORAFS_AI_PRESCREEN_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS AI pre-screening rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload)" in builder
    assert "REQUIRED_OPERATOR_ROUTES" in builder
    assert "REQUIRED_TRANSPARENCY_SOURCE_KINDS" in builder
    assert "REQUIRED_GOVERNANCE_PRODUCERS" in builder
    assert "REQUIRED_E2E_STEPS" in builder
    assert "test_generated_canaries_pass_full_ai_prescreen_gate" in builder_tests
    assert "scripts/build_sorafs_ai_prescreen_canary.py" in plan
    assert "scripts/build_sorafs_ai_prescreen_canary.py" in roadmap
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_ai_prescreen_notification_transport_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_ai_prescreen_commit_reveal_executor_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_ai_prescreen_end_to_end_canary.args.example"
    ).is_file()


def test_unshipped_ai_prescreen_deployed_workflow_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/moderation/deployed-runner",
        "/v1/sorafs/moderation/runner/deployed",
        "/v1/sorafs/moderation/deployed-committee",
        "/v1/sorafs/moderation/committee/deployed",
        "/v1/sorafs/moderation/deployed-juror-notification-transport",
        "/v1/sorafs/moderation/juror-notification-transport/service",
        "/v1/sorafs/moderation/deployed-commit-reveal-executor",
        "/v1/sorafs/moderation/commit-reveal-executor/service",
        "/v1/sorafs/moderation/end-to-end-workflow",
        "/v1/sorafs/moderation/release-workflow",
        "/v1/sorafs/moderation/workflow/promotion",
        "/v1/sorafs/moderation/ai-prescreen/promotion",
    )
    unshipped_cli_subcommands = (
        "deployed-runner-promote",
        "deployed-committee-promote",
        "juror-notification-transport-service",
        "deployed-juror-notification-transport",
        "commit-reveal-executor-service",
        "deployed-commit-reveal-executor",
        "moderation-release-workflow",
        "moderation-workflow-service",
        "moderation-workflow-promote",
        "ai-prescreen-release-workflow",
        "ai-prescreen-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_moderation_panel_parent_services_stay_open_in_docs() -> None:
    source = read(SORAFS_MODERATION_PANEL_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "It does not yet ship the full moderation appeal service, SoraFS juror panel engine, secure evidence viewer, durable voting orchestrator, or portal workflow described in the original plan.",
        "The production service still needs durable state that binds:",
        "evidence access attestation",
        "decision publication and appeal cache updates.",
        "Do not document `sorafs moderation jury-accept`, `sorafs moderation open-case`, or similar portal commands as shipped until the corresponding service and CLI handlers exist.",
        "Implement the moderation appeal intake API and persisted case lifecycle state.",
        "Adapt policy-jury sortition to SoraFS moderation cases, PoP snapshots, juror eligibility, no-show failover, and roster privacy requirements.",
        "Connect panel outcomes to gateway compliance caches, transparency publication, settlement reconciliation, and reputation scoring.",
        "Promote local Governance DAG moderation event publication into the durable contract-backed and public IPFS/IPNS decision trail.",
        "Capture reviewed, payload-free deployed evidence for appeal intake, sortition roster, evidence viewer, operator workflow, juror notification, commit/reveal, decision publication, settlement integration, transparency/reputation handoff, panel metrics, end-to-end panel simulation, and governance approval that passes the SFM-4b rollout evidence gate.",
        "Use the rollout gate after the deployed moderation appeal service, panel sortition, evidence viewer, operator workflow, juror notification transport, commit/reveal voting path, decision publication, settlement handoff, transparency/reputation handoff, metrics, end-to-end panel simulation, and governance packet have produced reviewed, payload-free JSON evidence",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_moderation_panel_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "runner dry-run emits the checker-backed `evidence_contract` map listing each selected evidence kind's schema and required payload fields.",
        "Every recognized rollout artifact must also carry reviewed `deployment_id` and `environment` context",
        "blocks mixed reviewed deployment contexts across the same rollout bundle.",
        "The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_moderation_panel_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_moderation_panel_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_moderation_panel_canary.py")
    builder_test = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_moderation_panel_canary_test.py"
    )
    intake_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_moderation_panel_appeal_intake_canary.args.example"
    )
    commit_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_moderation_panel_commit_reveal_canary.args.example"
    )
    docs = read(SORAFS_MODERATION_PANEL_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_INTAKE_ROUTES" in builder
    assert "REQUIRED_BALLOT_ROUTES" in builder
    assert "REQUIRED_VIEWER_EVENT_KINDS" in builder
    assert "REQUIRED_PUBLICATION_TARGETS" in builder
    assert "REQUIRED_METRICS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "raw_evidence_included" in builder
    assert "commit_payloads_included" in builder
    assert "signed_urls_included" in builder
    assert "watermark_secrets_included" in builder
    assert "test_generated_canaries_pass_full_moderation_panel_gate" in builder_test
    assert "test_under_replicated_e2e_panel_fails_before_write" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind\nappeal_intake" in intake_example
    assert "--verified-claim\nappellant_auth_enforced" in intake_example
    assert "--kind\ncommit_reveal" in commit_example
    assert "--verified-claim\nmismatched_reveal_rejected" in commit_example
    assert "build_sorafs_moderation_panel_canary.py" in docs
    assert "payload-free SFM-4b moderation panel canary builder" in docs


def test_unshipped_moderation_panel_parent_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/moderation/appeals/intake",
        "/v1/sorafs/moderation/appeals/cases",
        "/v1/sorafs/moderation/panel/service",
        "/v1/sorafs/moderation/panel/sortition",
        "/v1/sorafs/moderation/panel/roster",
        "/v1/sorafs/moderation/panel/decision",
        "/v1/sorafs/moderation/panel/promotion",
        "/v1/sorafs/moderation/portal",
        "/v1/sorafs/moderation/jury",
    )
    unshipped_cli_subcommands = (
        "open-case",
        "appeal-intake",
        "moderation-appeal-service",
        "panel-service",
        "panel-sortition",
        "panel-roster",
        "panel-decision-publish",
        "panel-promote",
        "jury-accept",
        "juror-panel",
        "moderation-portal",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_reputation_live_ingest_publisher_services_stay_open_in_docs() -> None:
    source = read(SORAFS_REPUTATION_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "Remaining rollout work is deploying the live ingest/publisher service and archiving production evidence that passes the rollout evidence gate, not the local scoring, proof, API, CLI, SDK, dashboard, or verifier foundations.",
        "Metrics ingest pipeline (`reputation_ingest`) | Streams PoR/PDP/PoTR verdicts, settlement logs, disputes, token violations from Governance DAG + telemetry exporters. | Validates payload signatures, persists raw events.",
        "Scoring engine (`reputation_engine`) | Aggregates metrics, runs scoring algorithm (EigenTrust-style), applies policy penalties, generates snapshots. | Runs hourly; writes outputs to database + object storage.",
        "Snapshot publisher (`reputation_publisher`) | Builds Merkle tree, updates Governance DAG, pushes snapshots to IPFS/S3, broadcasts Torii events. | Weekly full snapshot + daily incremental diff.",
        "API gateway (`sorafs_reputation_api`) | Exposes REST/GraphQL endpoints, WebSocket updates, CLI hooks. | Deployed regionally; uses caching with ETag.",
        "Implement ingestion (DAG listeners) and data schema; deploy staging environment drawing from test governance DAG.",
        "Build scoring engine and snapshot publisher; verify results with synthetic data.",
        "Production rollout:",
        "Deploy the live ingest/publisher service against production proof, dispute, settlement, and reserve/rent event sources.",
        "Capture live run evidence for snapshot freshness, ingest lag, low-score handling, SSE/WebSocket event delivery, and routing/incentive consumption",
        "Publish governance-approved weights and the first production snapshot with archived `.to`/JSON artifacts and proof replay evidence.",
        "Exercise rollback/stale-snapshot procedures before routing or incentives rely",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_reputation_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "allowing dry-run collection plans and downstream automation to inspect the exact SFM-3 evidence contract before live collection.",
        "Its `--dry-run` output includes the checker-backed `evidence_contract` map for publish/latest, provider, events, verify, metrics, transport, and consumption artifacts, and the runner validates the schema-closed collection plan, external evidence map, evidence contract, and command steps before dry-run output or live collection.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_reputation_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_unshipped_reputation_live_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/reputation/ingest",
        "/v1/sorafs/reputation/engine",
        "/v1/sorafs/reputation/publisher",
        "/v1/sorafs/reputation/api-gateway",
        "/v1/sorafs/reputation/public-api",
        "/v1/sorafs/reputation/graphql",
        "/v1/sorafs/reputation/storage",
        "/v1/sorafs/reputation/ipfs",
        "/v1/sorafs/reputation/promotion",
    )
    unshipped_cli_subcommands = (
        "reputation-ingest",
        "reputation-engine",
        "reputation-publisher",
        "reputation-api-gateway",
        "reputation-public-api",
        "reputation-graphql",
        "reputation-storage",
        "reputation-ipfs",
        "reputation-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_reputation_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_reputation_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_reputation_canary_test.py")
    docs = read(SORAFS_REPUTATION_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS reputation rollout canary artifacts." in builder
    assert "validate_evidence_set(" in builder
    assert "SNAPSHOT_ANCHOR_KINDS" in builder
    assert "SNAPSHOT_BOUND_KINDS" in builder
    assert "duplicate --sibling-hex" in builder
    assert "test_generated_canaries_pass_full_reputation_gate" in builder_tests
    assert "test_duplicate_provider_proof_sibling_fails_before_write" in builder_tests
    assert "scripts/build_sorafs_reputation_canary.py" in docs
    assert "unique provider proof sibling hashes" in docs
    assert "scripts/build_sorafs_reputation_canary.py" in roadmap
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_reputation_provider_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_reputation_metrics_canary.args.example"
    ).is_file()


def test_canary_name_set_validators_reject_duplicate_operator_values() -> None:
    validated: list[str] = []
    for path in canary_builders_with_name_set_validator():
        module = load_script_module(path, f"{path.stem}_duplicate_name_set_contract")
        errors: list[str] = []

        result = module.validate_name_set(
            ["alpha", "alpha"],
            allowed=("alpha",),
            option="--duplicate-test",
            errors=errors,
        )

        assert result == ["alpha"], path.name
        assert errors == ["--duplicate-test must not contain duplicates"], path.name
        validated.append(path.name)

    assert validated


def test_cli_sdk_distribution_and_live_governance_stay_open_in_docs() -> None:
    plan = read(SORAFS_CLI_SDK_PLAN)
    cli_doc = read(SORAFS_CLI_DOC)
    normalized_plan = re.sub(r"\s+", " ", plan)
    normalized_cli_doc = re.sub(r"\s+", " ", cli_doc)

    required_plan_open = (
        "Use `scripts/release_sorafs_cli.sh`, `ci/check_sorafs_cli_release.sh`, `scripts/sorafs_gateway_self_cert.sh`, and `cargo xtask sorafs-gateway-attest` for release and self-certification evidence.",
        "Release signing and manifest verification are wrapped by `scripts/release_sorafs_cli.sh`; gateway self-cert evidence is wrapped by `scripts/sorafs_gateway_self_cert.sh`.",
        "Runtime secrets such as identity tokens, private keys, and gateway bearer tokens must be supplied at execution time and not committed.",
        "Remaining release work is signed distribution evidence and live deployment capture, not missing local command surfaces.",
        "Package registries such as Homebrew, npm, crates.io, and Go modules should be populated only from signed release cuts using the existing release scripts and fixture smoke checks.",
    )
    required_cli_open = (
        "Remaining CLI work is release distribution and live-network governance evidence collection:",
        "Publish signed, reproducible release artefacts for the CLI and document the install path for Homebrew, npm, and crates.io consumers.",
        "Capture live governance proposal and council-signature runbooks once the production deployment publishes its operator signing process.",
    )
    missing_plan = [
        phrase for phrase in required_plan_open if phrase not in normalized_plan
    ]
    missing_cli = [
        phrase for phrase in required_cli_open if phrase not in normalized_cli_doc
    ]

    assert missing_plan == []
    assert missing_cli == []


def test_unshipped_cli_sdk_distribution_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/cli/releases",
        "/v1/sorafs/cli/distribution",
        "/v1/sorafs/cli/live-governance",
        "/v1/sorafs/cli/governance-runbooks",
        "/v1/sorafs/sdk/distribution",
        "/v1/sorafs/sdk/registries",
        "/v1/sorafs/sdk/live-deployment",
        "/v1/sorafs/release/promotion",
    )
    unshipped_cli_subcommands = (
        "release-distribute",
        "release-publish",
        "distribution-publish",
        "homebrew-publish",
        "npm-publish",
        "crates-publish",
        "go-module-publish",
        "sdk-distribute",
        "live-governance-capture",
        "governance-runbook-capture",
        "release-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_por_live_deployment_and_archive_work_stays_open_in_docs() -> None:
    scheduler = read(SORAFS_POR_PLAN)
    validator = read(SORAFS_POR_VALIDATOR_PLAN)
    normalized_scheduler = re.sub(r"\s+", " ", scheduler)
    normalized_validator = re.sub(r"\s+", " ", validator)

    required_scheduler_open = (
        "Remaining SF-9a rollout work is live deployment evidence for external drand, VRF, and auditor feeds, plus any production governance archive handoff required by the operator; each deployment's SQL/Parquet archive backend decision is now part of the checked reporting/archive evidence.",
        "The local SF-9 runtime integration is implemented. Remaining rollout work is live deployment evidence for external drand/VRF/auditor feeds and any production governance archive handoff required by the deployment operator.",
        "Operators should keep SF-9 promotion fail-closed until the payload-free deployment evidence passes the checked-in gate:",
        "The checker recognizes `sorafs.por.*` SF-9 rollout schemas for randomness, scheduler runtime, validator replay, reporting/archive handoff, observability, and governance approval.",
        "Archive a live drand/VRF/auditor run showing deterministic challenge generation and verdict replay that passes the SF-9 rollout evidence gate",
        "Capture each deployment's reviewed SQL/Parquet archive backend selection in the SF-9 reporting/archive evidence packet.",
        "Capture governance DAG archive handoff evidence for production operators and include it in the SF-9 reporting/archive evidence packet.",
    )
    required_validator_open = (
        "Remaining SF-9b work is live auditor rollout evidence, production archive handoff, and any richer proof-bundle inspection commands required by operators.",
        "The SF-9 validator/reporting release claim is tied to the same fail-closed gate used by the scheduler plan:",
        "The validator-specific evidence must prove `sorafs-validate por` challenge/proof replay, challenge/proof binding, exact sample coverage, deadline policy, Merkle/archive replay, `ValidationOutcomeV1` schema compatibility, bounded status/export/report route latency, weekly report generation, archive-retention policy, governance archive handoff, the exact `archive_backend` value (`sql` or `parquet`), and the explicit `retired` decision for the manual-trigger server route.",
        "Add proof-bundle fetch/show/offline replay commands if operators need them beyond `sorafs-validate por`.",
        "Archive live auditor, drand, VRF, report, and export evidence before treating SF-9 as fully released, and require that evidence to pass the SF-9 gate.",
    )
    missing_scheduler = [
        phrase
        for phrase in required_scheduler_open
        if phrase not in normalized_scheduler
    ]
    missing_validator = [
        phrase for phrase in required_validator_open if phrase not in normalized_validator
    ]

    assert missing_scheduler == []
    assert missing_validator == []


def test_por_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "planner includes the checker-backed `evidence_contract` map in dry-run output for the selected required kinds, and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
        "binding with per-artifact summary invalidation and dry-run export of the checker-backed evidence contract.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_por_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_por_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_por_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_por_canary_test.py")
    plan = read(SORAFS_POR_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS PoR rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "REQUIRED_RUNTIME_ROUTES" in builder
    assert "REQUIRED_REPORTING_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "SEED_REPLAY_BOUND_KINDS" in builder
    assert "test_generated_canaries_pass_full_por_gate" in builder_tests
    assert "scripts/build_sorafs_por_canary.py" in plan
    assert "scripts/build_sorafs_por_canary.py" in roadmap
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_por_randomness_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_por_scheduler_runtime_canary.args.example"
    ).is_file()


def test_por_validator_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The shared checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "planner includes the checker-backed `evidence_contract` map in `--dry-run` output so validator/reporting operators can review the exact SF-9 artifact contract before promotion, and the runner validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_por_validator_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_unshipped_por_live_deployment_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/por/live-deployment",
        "/v1/sorafs/por/external-drand",
        "/v1/sorafs/por/drand-feed",
        "/v1/sorafs/por/vrf-feed",
        "/v1/sorafs/por/auditor-feed",
        "/v1/sorafs/por/auditor-live",
        "/v1/sorafs/por/production-archive",
        "/v1/sorafs/por/archive-handoff",
        "/v1/sorafs/por/sql-warehouse",
        "/v1/sorafs/por/parquet-warehouse",
        "/v1/sorafs/por/proof-bundle",
        "/v1/sorafs/por/promotion",
    )
    unshipped_cli_subcommands = (
        "por-live-deployment",
        "por-external-drand",
        "por-drand-feed",
        "por-vrf-feed",
        "por-auditor-feed",
        "por-live-auditor",
        "por-production-archive",
        "por-archive-handoff",
        "por-sql-warehouse",
        "por-parquet-warehouse",
        "por-proof-bundle",
        "por-proof-bundle-fetch",
        "por-proof-bundle-show",
        "por-proof-bundle-replay",
        "por-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_potr_live_rollout_and_provider_key_work_stays_open_in_docs() -> None:
    source = read(SORAFS_POTR_PLAN)
    normalized = re.sub(r"\s+", " ", source.replace("> ", ""))

    required_open = (
        "SF-14 work is live multi-provider rollout evidence and PQ provider-signature key distribution, now represented by governance-bound key-roster and reputation-weight policy digests in rollout evidence, not local receipt capture, validation, or replay wiring.",
        "Operators should keep SF-14 promotion fail-closed until payload-free deployment evidence passes the checked-in gate:",
        "The checker recognizes `sorafs.potr.*` SF-14 rollout schemas for multi-provider probes, receipt validation, proof-stream replay, reputation integration, observability, and governance approval.",
        "missing governed ML-DSA provider key evidence, non-Norito proof-stream routes, missing proof-stream filters, missing reputation-weight governance",
        "PQ key-roster digest drift between receipt validation and governance approval, reputation-weight policy digest drift between reputation integration and governance approval",
        "The collection planner exposes those exact required payload fields through `--dry-run` and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before contacting live PoTR services.",
        "Future updates should track live rollout evidence, governed provider PQ keys, and reputation-weight changes that pass the SF-14 gate",
        "proof-stream, reputation, observability, and governance artifacts bound to the same multi-provider probe receipt summary digest, plus receipt-validation and reputation artifacts bound to governance-approved PQ key-roster and reputation-weight policy digests, rather than reintroducing draft local wiring tasks.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_potr_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "planner includes the checker-backed `evidence_contract` map in dry-run output for the selected required kinds, and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
        "The collection planner exposes those exact required payload fields through `--dry-run` and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before contacting live PoTR services.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_potr_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path).replace("> ", ""))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_potr_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_potr_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_potr_canary_test.py")
    plan = read(SORAFS_POTR_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS PoTR rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "REQUIRED_TIERS" in builder
    assert "REQUIRED_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "RECEIPT_SUMMARY_BOUND_KINDS" in builder
    assert "test_generated_canaries_pass_full_potr_gate" in builder_tests
    assert "scripts/build_sorafs_potr_canary.py" in plan
    assert "scripts/build_sorafs_potr_canary.py" in roadmap
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_potr_multi_provider_probe_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_potr_proof_stream_canary.args.example"
    ).is_file()


def test_unshipped_potr_live_rollout_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/potr/live-probes",
        "/v1/sorafs/potr/multi-provider-probes",
        "/v1/sorafs/potr/live-rollout",
        "/v1/sorafs/potr/provider-key-distribution",
        "/v1/sorafs/potr/ml-dsa-keys",
        "/v1/sorafs/potr/pq-provider-keys",
        "/v1/sorafs/potr/reputation-weights",
        "/v1/sorafs/potr/governance-approval",
        "/v1/sorafs/potr/promotion",
        "/v1/sorafs/proof/potr/promotion",
        "/v1/sorafs/proof/stream/potr/live",
    )
    unshipped_cli_subcommands = (
        "potr-live-probes",
        "potr-multi-provider-probes",
        "potr-live-rollout",
        "potr-provider-key-distribution",
        "potr-ml-dsa-keys",
        "potr-pq-provider-keys",
        "potr-reputation-weights",
        "potr-governance-approval",
        "potr-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_repair_live_operator_evidence_work_stays_open_in_docs() -> None:
    source = read(SORAFS_REPAIR_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "Remaining SF-8b work is live operator evidence: archive a production PoR/PoTR failure, repair, escalation, and governance handoff once the deployed auditor roster and SF-9 coordinator publish their runbooks.",
        "Use the rollout gate after the deployed auditor roster, SF-9 coordinator, PoR/PoTR failure capture, signed auditor API, repair worker lifecycle, repair event streams, governance handoff, observability, and governance packet have produced reviewed, payload-free JSON evidence:",
        "The checker recognizes `sorafs.repair.*` SF-8b rollout schemas for auditor roster, failure capture, signed auditor API, worker lifecycle, event streams, governance handoff, observability, and governance approval evidence.",
        "raw PoR/PoTR evidence, raw repair payloads, signed auditor requests, response bodies, signed transactions, secrets, and ledgers are absent",
        "matches a valid auditor-roster artifact, and worker lifecycle / event stream / governance handoff artifacts carry an `evidence_bundle_digest_hex` that matches a valid PoR/PoTR failure-capture artifact",
        "governance approval artifacts carry a `handoff_digest_hex` that matches a valid governance handoff artifact",
        "The SF-8b rollout evidence gate, collection planner, operator argfile templates, and focused tests are implemented for payload-free deployed evidence review",
        "Remaining rollout work is live operator evidence: collect production PoR failure, repair, and governance handoff artifacts once the deployed auditor roster and SF-9 coordinator publish their runbooks, then pass the SF-8b rollout evidence gate.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_repair_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "planner includes the checker-backed `evidence_contract` map in dry-run output for the selected required kinds, and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
        "Its collection planner exposes those exact required payload fields through `--dry-run` and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before touching live repair services.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_repair_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_repair_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_repair_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_repair_canary_test.py")
    docs = read(SORAFS_REPAIR_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS repair rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "REQUIRED_AUDITOR_ROUTES" in builder
    assert "REQUIRED_WORKER_ROUTES" in builder
    assert "REQUIRED_EVENT_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "ROSTER_BOUND_KINDS" in builder
    assert "FAILURE_BOUND_KINDS" in builder
    assert "test_generated_canaries_pass_full_repair_gate" in builder_tests
    assert "scripts/build_sorafs_repair_canary.py" in docs
    assert "scripts/build_sorafs_repair_canary.py" in roadmap
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_repair_auditor_roster_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_repair_worker_lifecycle_canary.args.example"
    ).is_file()


def test_unshipped_repair_live_operator_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/repair/live-operator-evidence",
        "/v1/sorafs/repair/deployed-auditor-roster",
        "/v1/sorafs/repair/auditor-roster-live",
        "/v1/sorafs/repair/sf9-coordinator-runbook",
        "/v1/sorafs/repair/por-potr-failure-capture",
        "/v1/sorafs/repair/live-failure-capture",
        "/v1/sorafs/repair/live-governance-handoff",
        "/v1/sorafs/repair/production-handoff",
        "/v1/sorafs/repair/promotion",
        "/v1/sorafs/audit/repair/deployed-roster",
        "/v1/sorafs/audit/repair/live-coordinator",
        "/v1/sorafs/audit/repair/production-handoff",
        "/v1/sorafs/audit/repair/promotion",
    )
    unshipped_cli_subcommands = (
        "repair-live-operator-evidence",
        "repair-deployed-auditor-roster",
        "repair-auditor-roster-live",
        "repair-sf9-coordinator-runbook",
        "repair-por-potr-failure-capture",
        "repair-live-failure-capture",
        "repair-live-governance-handoff",
        "repair-production-handoff",
        "repair-promote",
        "repair-promotion",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_reference_sdk_release_distribution_work_stays_open_in_docs() -> None:
    source = read(SORAFS_REFERENCE_SDK_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "Remaining SF-11 work is release evidence and SDK distribution: per-target published archives, signed release manifests, published downstream binding packages, and live operator smoke records.",
        "Remaining downstream work is signed release packaging, publication, and live SDK smoke evidence for those bindings.",
        "Cross-target release evidence is still a production gate; archive published checksums and smoke outputs for each supported release target and require the SF-11 release evidence gate to pass before declaring those artifacts production-ready.",
        "Final release-specific URLs, signatures, and package versions remain SF-11 release evidence.",
        "Operators should keep SF-11 release promotion fail-closed until payload-free release evidence passes the checked-in gate:",
        "Narrowed `--require-kind` release runs also reject evidence supplied for excluded kinds before the plan is rendered or the verifier starts.",
        "The checker recognizes `sorafs.reference_sdk.*` SF-11 release schemas for release archives, signed manifests, downstream bindings, cookbook smoke, FFI/header contract, and governance approval.",
        "missing JavaScript/Python/Kotlin/JVM/Java Android/Swift package publication evidence",
        "Run the packaging helper for the supported release targets and publish signed release manifests outside the repository using governed release keys",
        "Ship/publish downstream SDK binding packages and release artifacts for the local JavaScript, Python, Kotlin/JVM, Java Android, and Swift wrappers",
        "Archive live operator smoke evidence for the published `sorafs-validate` archives and cookbook replay before declaring SF-11 fully released",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_reference_sdk_docs_do_not_reopen_implemented_guides() -> None:
    stale_phrase = "Remaining: publish operator, metrics, and binding-generation guides"
    current_phrase = (
        "Implemented: the operator, metrics, and binding-generation guides "
        "below cover the local release helper"
    )
    stale: list[str] = []
    missing_current: list[str] = []

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_reference_sdk_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        if stale_phrase in normalized:
            stale.append(str(path.relative_to(REPO_ROOT)))
        if current_phrase not in normalized:
            missing_current.append(str(path.relative_to(REPO_ROOT)))

    assert stale == []
    assert missing_current == []


def test_reference_sdk_release_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_reference_sdk_release_canary.py")
    builder_tests = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_reference_sdk_release_canary_test.py"
    )
    docs = read(SORAFS_REFERENCE_SDK_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS reference SDK release evidence artifacts." in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "REQUIRED_RELEASE_TARGETS" in builder
    assert "REQUIRED_DOWNSTREAM_PACKAGES" in builder
    assert "RELEASE_MANIFEST_BOUND_KINDS" in builder
    assert "test_generated_canaries_pass_full_reference_sdk_release_gate" in builder_tests
    assert "scripts/build_sorafs_reference_sdk_release_canary.py" in docs
    assert "scripts/build_sorafs_reference_sdk_release_canary.py" in roadmap
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_reference_sdk_release_archive_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_reference_sdk_signed_manifest_canary.args.example"
    ).is_file()


def test_reference_sdk_release_runner_plan_envelope_is_schema_closed() -> None:
    runner = read(SCRIPTS_DIR / "run_sorafs_reference_sdk_release_evidence.py")
    runner_test = read(
        SCRIPTS_DIR / "tests" / "run_sorafs_reference_sdk_release_evidence_test.py"
    )

    assert "PLAN_SCHEMA" in runner
    assert "PLAN_FIELDS" in runner
    assert "def validate_plan_json" in runner
    assert "reference SDK release runner plan must be an object" in runner
    assert (
        "reference SDK release runner plan fields must match the schema-closed contract"
        in runner
    )
    assert (
        "reference SDK release runner plan evidence_contract must match checker fields"
        in runner
    )
    assert "plan_errors = validate_plan_json(rendered_plan, plan, args)" in runner
    assert "test_plan_json_shape_is_validated" in runner_test
    assert "test_subset_gate_rejects_evidence_for_unrequired_kind" in runner_test
    assert "test_execution_rejects_plan_validation_drift_before_running" in runner_test


def test_unshipped_reference_sdk_distribution_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/reference-sdk/release-archives",
        "/v1/sorafs/reference-sdk/signed-manifests",
        "/v1/sorafs/reference-sdk/downstream-bindings",
        "/v1/sorafs/reference-sdk/downstream-packages",
        "/v1/sorafs/reference-sdk/live-smoke",
        "/v1/sorafs/reference-sdk/published-cookbook-smoke",
        "/v1/sorafs/reference-sdk/package-publication",
        "/v1/sorafs/reference-sdk/release-promotion",
        "/v1/sorafs/reference-sdk/promotion",
        "/v1/sorafs/validate/published-archives",
        "/v1/sorafs/validate/release-promotion",
    )
    unshipped_cli_subcommands = (
        "reference-sdk-publish",
        "reference-sdk-release-archives",
        "reference-sdk-signed-manifests",
        "reference-sdk-downstream-bindings",
        "reference-sdk-downstream-packages",
        "reference-sdk-live-smoke",
        "reference-sdk-published-cookbook-smoke",
        "reference-sdk-package-publication",
        "reference-sdk-release-promote",
        "reference-sdk-promote",
        "sorafs-validate-publish",
        "sorafs-validate-release-promote",
        "published-archive-smoke",
        "downstream-bindings-publish",
        "release-manifest-publish",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_pdp_provider_protocol_work_stays_open_in_docs() -> None:
    source = read(SORAFS_PDP_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "the provider protocol is not production ready yet. Torii therefore rejects PDP proof-stream requests with `400 Bad Request` until real provider proof generation, signature verification, and governance archival are implemented.",
        "The PDP rollout evidence gate requires payload-free provider-transport, proof-generation, validator-replay, governance/repair, observability, and governance-approval artifacts before reporting `ready`",
        "Torii `/v1/sorafs/proof/stream` accepts PoR and PoTR only. It parses `pdp` but returns `400 Bad Request` so clients do not mistake PoR samples for PDP provider proofs.",
        "Do not remove the Torii fail-closed PDP guard until these local gates exist:",
        "Provider challenge queue:",
        "Deterministic proof generation from stored payloads",
        "Provider signature verification over canonical PDP proof bytes with governance-controlled key material.",
        "Governance DAG archival for accepted PDP proofs and PDP failure reports.",
        "Repair pipeline handoff for `pdp_failure` events.",
        "Do not document the unshipped `sorafs pdp ...` commands as operator-ready until they exist in the CLI and have focused tests.",
        "Required before production enablement:",
        "Storage-node integration tests that generate PDP proofs from persisted payloads and validate them against commitment roots.",
        "Torii endpoint tests for challenge issuance, proof submission, governance archival, repair handoff, and telemetry counters.",
        "Remaining production gates:",
        "Ship operator CLI commands and SDK validators.",
        "Update OpenAPI/portal docs and remove the Torii PDP fail-closed guard.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_pdp_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The PDP rollout evidence gate requires payload-free provider-transport, proof-generation, validator-replay, governance/repair, observability, and governance-approval artifacts before reporting `ready`",
        "Proof-summary mismatches are recorded on the offending artifact in the JSON summary before required-kind validity is reported.",
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "the collection runner includes the checker-backed `evidence_contract` map in dry-run output for the selected required kinds, and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
        "with proof-summary digest binding and rejection of evidence supplied for excluded `--require-kind` values.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_pdp_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_pdp_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_pdp_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_pdp_canary_test.py")
    plan = read(SORAFS_PDP_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS PDP rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "REQUIRED_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "PROOF_SUMMARY_BOUND_KINDS" in builder
    assert "test_generated_canaries_pass_full_pdp_gate" in builder_tests
    assert "scripts/build_sorafs_pdp_canary.py" in plan
    assert "scripts/build_sorafs_pdp_canary.py" in roadmap
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_pdp_provider_transport_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR / "examples" / "sorafs_pdp_proof_generation_canary.args.example"
    ).is_file()


def test_unshipped_pdp_provider_protocol_surface_is_not_exposed() -> None:
    route_patterns = (
        "/sorafs/pdp/challenge",
        "/sorafs/pdp/next",
        "/sorafs/pdp/proof",
        "/v1/sorafs/pdp/challenge",
        "/v1/sorafs/pdp/next",
        "/v1/sorafs/pdp/proof",
        "/v1/sorafs/pdp/provider-transport",
        "/v1/sorafs/pdp/proof-generation",
        "/v1/sorafs/pdp/provider-signatures",
        "/v1/sorafs/pdp/inclusion-witnesses",
        "/v1/sorafs/pdp/governance-archive",
        "/v1/sorafs/pdp/repair-handoff",
        "/v1/sorafs/pdp/operator-cli",
        "/v1/sorafs/pdp/promotion",
    )
    unshipped_cli_subcommands = (
        "pdp-challenge",
        "pdp-fetch",
        "pdp-respond",
        "pdp-verify",
        "pdp-status",
        "pdp-export",
        "pdp-provider-transport",
        "pdp-proof-generation",
        "pdp-governance-archive",
        "pdp-repair-handoff",
        "pdp-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_governance_dag_ipfs_ipns_work_stays_unshipped_in_docs() -> None:
    source = read(SORAFS_GOVERNANCE_DAG_PLAN)

    outstanding_start = source.index("Still outstanding:")
    outstanding_end = source.index("\n## Goals & Scope", outstanding_start)
    outstanding = source[outstanding_start:outstanding_end]
    normalized_outstanding = re.sub(r"\s+", " ", outstanding)

    required_outstanding = (
        "IPFS Cluster pinning and IPNS head publication",
        "Runtime RocksDB/IPLD mirror datastore and query service",
        "IPFS/IPNS-backed `sorafs governance dag` operations for live heads",
        "public checkpoint publication, and public checkpoint recovery",
        "Runtime/IPFS-backed dashboard REST/GraphQL API",
        "Live IPFS/IPNS publisher metrics",
        "End-to-end tests with local IPFS/IPNS infrastructure",
    )
    missing = [
        phrase for phrase in required_outstanding if phrase not in normalized_outstanding
    ]

    assert "does not yet ship the\nfull IPFS/IPNS governance DAG pipeline" in source
    assert "Add live-head, public checkpoint recovery, and dashboard runbooks only when" in source
    assert "the IPFS/IPNS pipeline and metrics actually exist" in source
    assert missing == []


def test_governance_dag_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "the runner dry-run emits the checker-backed `evidence_contract` map for selected SF-12 evidence kinds, and validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
        "Mirror datastore, checkpoint recovery, dashboard, observability, IPFS/IPNS end-to-end, and governance approval artifacts must carry the same `public_head_cid_hex` as a valid publisher-service artifact",
        "collection planner with dry-run evidence-contract export and schema-closed plan validation",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_governance_dag_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_governance_dag_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_governance_dag_canary.py")
    builder_test = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_governance_dag_canary_test.py"
    )
    publisher_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_governance_dag_publisher_canary.args.example"
    )
    dashboard_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_governance_dag_dashboard_canary.args.example"
    )
    docs = read(SORAFS_GOVERNANCE_DAG_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_PAYLOAD_KINDS" in builder
    assert "REQUIRED_DASHBOARD_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "raw_head_included" in builder
    assert "raw_car_included" in builder
    assert "raw_checkpoint_included" in builder
    assert "response_bodies_included" in builder
    assert "test_generated_canaries_pass_full_governance_dag_gate" in builder_test
    assert "test_missing_dashboard_route_coverage_fails_closed" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind publisher_service" in publisher_example
    assert "--verified-claim car_segments_pinned" in publisher_example
    assert "--payload-kind orderbook-settlement-receipt" in publisher_example
    assert "--kind dashboard_api" in dashboard_example
    assert "--route checkpoint" in dashboard_example
    assert "build_sorafs_governance_dag_canary.py" in docs
    assert "payload-free Governance DAG canary builder" in docs


def test_unshipped_governance_dag_public_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/governance/dag/ipfs",
        "/v1/sorafs/governance/dag/ipns",
        "/v1/sorafs/governance/dag/live",
        "/v1/sorafs/governance/dag/public",
        "/v1/sorafs/governance/dag/checkpoints/public",
        "/v1/sorafs/governance/dag/mirror-service",
        "/v1/sorafs/governance/dag/graphql",
    )
    unshipped_cli_subcommands = (
        "live-head",
        "fetch-head",
        "publish-checkpoint",
        "checkpoint-publish",
        "mirror-service",
        "ipfs-publish",
        "ipns-publish",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    cli_source = read(SORAFS_CLI_RS)
    matched_commands = [
        subcommand
        for subcommand in unshipped_cli_subcommands
        if f'"{subcommand}"' in cli_source or f"`{subcommand}`" in cli_source
    ]
    if matched_commands:
        exposed[str(SORAFS_CLI_RS.relative_to(REPO_ROOT))] = matched_commands

    assert exposed == {}


def test_orderbook_contract_and_daemon_work_stays_unshipped_in_docs() -> None:
    source = read(SORAFS_ORDERBOOK_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_unshipped = (
        "does not ship an on-chain SoraFS orderbook contract",
        "durable off-chain matcher service",
        "on-chain/daemonized streaming-settlement receipt service",
        "contract-backed authenticated orderbook streams",
        "On-chain orderbook contract | Store bids/asks, match orders, record fills, and enforce escrow requirements. | Not shipped.",
        "daemonized matcher service and contract submission are not shipped",
        "durable receipt daemon and escrow custody mutation are not shipped",
        "contract forwarding and durable streams are not shipped",
        "live dashboard wiring and rollout evidence are not shipped",
        "Remaining: implement on-chain contract surface, durable matcher service",
        "daemonized settlement receipt service with escrow custody mutation",
        "durable contract/matcher-backed WebSocket/SSE streams",
    )
    missing = [phrase for phrase in required_unshipped if phrase not in normalized]

    assert missing == []


def test_orderbook_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "the runner dry-run emits the checker-backed `evidence_contract` map for selected SFM-2 evidence kinds.",
        "Matcher, settlement, API gateway, event stream, SDK release, observability, reconciliation, and governance approval artifacts must carry a `contract_digest_hex` that matches a valid contract-surface artifact",
        "collection planner with dry-run evidence-contract export",
        "The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_orderbook_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_orderbook_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_orderbook_canary.py")
    builder_test = read(SCRIPTS_DIR / "tests" / "build_sorafs_orderbook_canary_test.py")
    contract_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_orderbook_contract_canary.args.example"
    )
    api_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_orderbook_api_canary.args.example"
    )
    docs = read(SORAFS_ORDERBOOK_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_API_ROUTES" in builder
    assert "REQUIRED_STREAMS" in builder
    assert "REQUIRED_SDK_LANGUAGES" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "raw_contract_state_included" in builder
    assert "raw_snapshot_included" in builder
    assert "raw_receipts_included" in builder
    assert "response_bodies_included" in builder
    assert "duplicate --artifact id" in builder
    assert "test_generated_canaries_pass_full_orderbook_gate" in builder_test
    assert "test_duplicate_sdk_artifact_id_fails_closed_without_leaking" in builder_test
    assert "test_missing_api_route_coverage_fails_closed" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind contract_surface" in contract_example
    assert "--verified-claim capability_policy_configured" in contract_example
    assert "--kind api_gateway" in api_example
    assert "--route events_get" in api_example
    assert "build_sorafs_orderbook_canary.py" in docs
    assert "payload-free SFM-2 orderbook canary builder" in docs


def test_unshipped_orderbook_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/orderbook/contract",
        "/v1/sorafs/orderbook/contracts",
        "/v1/sorafs/orderbook/matcher-service",
        "/v1/sorafs/orderbook/settlement-service",
        "/v1/sorafs/orderbook/escrow-custody",
        "/v1/sorafs/orderbook/dashboard",
        "/v1/sorafs/orderbook/contract-stream",
    )
    unshipped_cli_subcommands = (
        "match-daemon",
        "matcher-service",
        "settlement-daemon",
        "contract-submit",
        "contract-forward",
        "escrow-mutate",
        "dashboard-serve",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    cli_source = read(SORAFS_CLI_RS)
    matched_commands = [
        subcommand
        for subcommand in unshipped_cli_subcommands
        if f'"{subcommand}"' in cli_source or f"`{subcommand}`" in cli_source
    ]
    if matched_commands:
        exposed[str(SORAFS_CLI_RS.relative_to(REPO_ROOT))] = matched_commands

    assert exposed == {}


def test_hedging_runtime_services_stay_unshipped_in_docs() -> None:
    source = read(SORAFS_HEDGING_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_unshipped = (
        "This is not yet a production hedging and billing stack",
        "There is still no shipped `hedgingd`, price-feed collector service, `billingd`, statement publisher, SoraFS hedging/billing REST API, service-management CLI",
        "daemon, exposure tracking, and hedge execution are not shipped",
        "Price feed collectors | Fetch primary/secondary/tertiary feeds and normalize them into signed price payloads. | Not shipped for SoraFS hedging.",
        "event ingestion and accrual service are not shipped",
        "Statement publisher | Store, sign, publish, notify, and track acknowledgements for statements. | Not shipped.",
        "runtime service emission and service management are not shipped",
        "Automated hedge execution must remain off until governance approves venues",
        "No hedging or billing routes are currently shipped.",
        "Remaining: implement collector service, daemonized pricing/exposure engine, billing aggregator, statement publisher, signed APIs, runtime CLI helpers",
    )
    missing = [phrase for phrase in required_unshipped if phrase not in normalized]

    assert missing == []


def test_hedging_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "the collection planner dry-run JSON includes the checker-backed `evidence_contract` map for selected required kinds",
        "Production promotion remains blocked unless the summary status is `ready`, including at least two distinct staged billing cycles whose reference-decision ids match a valid reference-price artifact in the same evidence bundle.",
        "The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_hedging_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_hedging_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_hedging_canary.py")
    builder_test = read(SCRIPTS_DIR / "tests" / "build_sorafs_hedging_canary_test.py")
    reference_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_hedging_reference_price_canary.args.example"
    )
    billing_example = read(
        SCRIPTS_DIR / "examples" / "sorafs_billing_cycle_canary.args.example"
    )
    docs = read(SORAFS_HEDGING_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_PUBLICATION_ROUTES" in builder
    assert "REQUIRED_RECONCILIATION_SOURCES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "payload_bytes_included" in builder
    assert "raw_financial_records_included" in builder
    assert "response_bodies_included" in builder
    assert "debug_artifacts" in builder
    assert "duplicate --artifact id" in builder
    assert "test_generated_canaries_pass_full_hedging_gate" in builder_test
    assert (
        "test_duplicate_native_bridge_artifact_id_fails_closed_without_leaking"
        in builder_test
    )
    assert "test_hedge_execution_enabled_requires_governance_before_write" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind reference_price" in reference_example
    assert "--verified-claim signed_payload_verified" in reference_example
    assert "--kind billing_cycle" in billing_example
    assert "--verified-claim acknowledgement_required" in billing_example
    assert "build_sorafs_hedging_canary.py" in docs
    assert "payload-free SFM-5 hedging/billing canary builder" in docs


def test_unshipped_hedging_billing_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/hedging",
        "/v1/sorafs/billing",
    )
    unshipped_cli_subcommands = (
        "hedgingd",
        "billingd",
        "hedging-daemon",
        "billing-daemon",
        "price-feed-collector",
        "collector-service",
        "hedge-execute",
        "exposure-status",
        "statement-publish",
        "statement-ack",
        "billing-api",
        "hedging-status",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_evidence_viewer_runtime_services_stay_unshipped_in_docs() -> None:
    source = read(SORAFS_EVIDENCE_VIEWER_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_unshipped = (
        "SFM-4b3 is not yet shipped as a moderation evidence viewer.",
        "it does not contain the browser viewer, streaming backend, watermark engine, WebAuthn session flow, or access-log service",
        "That gate is a promotion blocker for deployed evidence; it does not replace the missing viewer service.",
        "It is a media validation harness, not a moderation evidence viewer.",
        "The production moderation evidence viewer still needs these services:",
        "Viewer frontend | Browser UI for jurors, auditors, and legal reviewers with strict CSP and disabled offline mode.",
        "Viewer backend | Authenticates sessions, issues short-lived segment URLs, and binds access to case and role scopes.",
        "Watermark engine | Generates per-session visual and optional audio watermarks tied to juror pseudonyms and nonces.",
        "Access logger | Writes append-only view, seek, pause, screenshot, download-attempt, and annotation events.",
        "Transparency exporter | Publishes anonymized access reports and daily digests to the Governance DAG.",
        "No production route should claim support for `/v1/evidence/session`, `/v1/evidence/manifest`, `/v1/evidence/log`, or `/v1/evidence/audit` until the service exists",
        "Existing adjacent checks do not prove evidence-viewer readiness.",
        "tests before removing the unshipped-service language from this page.",
    )
    missing = [phrase for phrase in required_unshipped if phrase not in normalized]

    assert missing == []


def test_unshipped_evidence_viewer_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/evidence-viewer",
        "/v1/sorafs/moderation/evidence-viewer",
        "/v1/sorafs/moderation/evidence/session",
        "/v1/sorafs/moderation/evidence/manifest",
        "/v1/sorafs/moderation/evidence/log",
        "/v1/sorafs/moderation/evidence/audit",
        "/v1/evidence/session",
        "/v1/evidence/manifest",
        "/v1/evidence/log",
        "/v1/evidence/audit",
    )
    unshipped_cli_subcommands = (
        "evidence-viewer",
        "viewer-serve",
        "viewer-session",
        "viewer-manifest",
        "viewer-audit",
        "watermark-engine",
        "access-log",
        "access-logger",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_evidence_viewer_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_evidence_viewer_canary.py")
    builder_test = read(SCRIPTS_DIR / "tests" / "build_sorafs_evidence_viewer_canary_test.py")
    example = read(SCRIPTS_DIR / "examples" / "sorafs_evidence_viewer_canary.args.example")
    docs = read(SORAFS_EVIDENCE_VIEWER_PLAN)

    assert "SCHEMA = KIND_BY_NAME[\"evidence_viewer\"].schema" in builder
    assert "VERIFIED_TRUE_CLAIMS" in builder
    assert "FORBIDDEN_PAYLOAD_CLAIMS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "session_tokens_included" in builder
    assert "signed_urls_included" in builder
    assert "watermark_secrets_included" in builder
    assert "test_generated_canary_passes_existing_evidence_viewer_gate" in builder_test
    assert "test_missing_verified_claim_fails_closed" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--verified-claim legal_hold_policy_bound" in example
    assert "--session-manifest-digest-hex" in example
    assert "build_sorafs_evidence_viewer_canary.py" in docs
    assert "payload-free `evidence_viewer` canary builder" in docs


def test_commit_reveal_production_services_stay_unshipped_in_docs() -> None:
    source = read(SORAFS_COMMIT_REVEAL_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_unshipped = (
        "repository now ships local ballot CLI/client readback, signed commit/reveal/tally submission, and payload-free executor automation for the local Torii API. It does not yet ship the SoraFS moderation voting contract, contract-backed durable ballot orchestrator, challenge monitor, public juror portal, or deployed production service needed to run appeal-panel ballots end to end.",
        "`iroha::client` and `iroha sorafs moderation ballots list|get|events|commit|reveal|tally` wrap the local readback and signed committee lifecycle endpoints",
        "`iroha sorafs moderation ballots execute|executor-bundle|executor-canary` provide local payload-free commit/reveal executor automation",
        "That gate blocks deployed promotion evidence; it does not replace the missing durable service or contract-backed workflow.",
        "Persist or contract-back the local ballot lifecycle store and add the production orchestrator for retries, no-show handling, challenge disputes, and durable contested-outcome workflows.",
        "Implement the on-chain contract or ledger workflow that records commitments, reveals, challenges, outcomes, and juror penalties.",
        "Promote the shipped local CLI/client bridge and executor automation into audited juror-facing deployment workflows, including challenge evidence export, portal UX, and public operations runbooks.",
        "Extend Governance DAG publication beyond local lifecycle events to durable challenge/dispute records, contract-backed decisions, and public IPFS/IPNS rollout evidence.",
        "Collect a passing payload-free `commit_reveal` canary through the SFM-4b rollout evidence gate after the durable service exists.",
        "Until then, do not document `sorafs-juror`, portal-only commands, or deployed ballot service commands as shipped.",
    )
    missing = [phrase for phrase in required_unshipped if phrase not in normalized]

    assert missing == []


def test_commit_reveal_docs_do_not_reopen_shipped_local_cli_bridge() -> None:
    stale_phrases = (
        "durable ballot orchestrator, juror CLI, challenge monitor",
        "Provide juror-facing CLI or portal commands for listing ballots, committing",
        "Until then, do not document `sorafs-juror` or SoraFS ballot service commands as shipped.",
    )
    required_current = (
        "repository now ships local ballot CLI/client readback, signed commit/reveal/tally submission, and payload-free executor automation for the local Torii API.",
        "`iroha::client` and `iroha sorafs moderation ballots list|get|events|commit|reveal|tally` wrap the local readback and signed committee lifecycle endpoints",
        "`iroha sorafs moderation ballots execute|executor-bundle|executor-canary` provide local payload-free commit/reveal executor automation",
        "Promote the shipped local CLI/client bridge and executor automation into audited juror-facing deployment workflows",
        "Until then, do not document `sorafs-juror`, portal-only commands, or deployed ballot service commands as shipped.",
    )
    stale: dict[str, list[str]] = {}
    missing: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_commit_reveal_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        matched_stale = [phrase for phrase in stale_phrases if phrase in normalized]
        missing_current = [
            phrase for phrase in required_current if phrase not in normalized
        ]
        if matched_stale:
            stale[str(path.relative_to(REPO_ROOT))] = matched_stale
        if missing_current:
            missing[str(path.relative_to(REPO_ROOT))] = missing_current

    assert stale == {}
    assert missing == {}


def test_unshipped_commit_reveal_production_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/moderation/voting-contract",
        "/v1/sorafs/moderation/ballots/contract",
        "/v1/sorafs/moderation/ballots/coordinator",
        "/v1/sorafs/moderation/ballots/challenges",
        "/v1/sorafs/moderation/ballots/disputes",
        "/v1/sorafs/moderation/decision-dag",
        "/v1/sorafs/moderation/challenge-dag",
        "/v1/sorafs/moderation/juror-portal",
        "/v1/sorafs/juror",
    )
    unshipped_cli_subcommands = (
        "voting-contract",
        "ballot-service",
        "ballot-orchestrator",
        "commit-reveal-coordinator",
        "challenge-monitor",
        "challenge-open",
        "dispute-open",
        "decision-dag-publish",
        "juror-portal",
        "sorafs-juror",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_appeal_finance_live_dashboard_and_reconciliation_stay_open_in_docs() -> None:
    source = read(SORAFS_APPEAL_PRICING_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "The repo still does not ship a standalone pricing daemon.",
        "Deposit custody uses the returned native `OpenAssetLock` instruction, which the authenticated payer must sign and submit through the normal transaction path",
        "The checker recognizes `sorafs.appeal_finance.*` SFM-4b2 rollout schemas for pricing config, quote APIs, deposit lifecycle, settlement execution, settlement submitter, moderation worker, Governance DAG publication, dashboard metrics, multi-peer reconciliation, and governance approval.",
        "It reports `ready` only when every required kind is present",
        "the multi-peer reconciliation run covers at least four peers",
        "Capture hosted live/public dashboard and alert evidence that passes the SFM-4b2 rollout gate once the public Governance DAG and ledger reconciliation paths are deployed.",
        "Capture end-to-end evidence that covers quote creation, deposit posting, decision ingestion, settlement submission, disbursement, and treasury reconciliation against a multi-peer runtime ledger with at least four peers",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_appeal_finance_docs_do_not_reopen_shipped_local_runtime_status() -> None:
    stale_phrases = (
        "summary: SFM-4b2 implementation status for appeal quote, settlement, and disbursement helpers plus the remaining escrow and service gates.",
        "reported, and `scripts/run_sorafs_appeal_finance_rollout_evidence.py` provides the matching reviewed evidence collection planner/runner.",
    )
    current_phrases = (
        "configured-signer settlement submitter that queues the next pending native",
        "Torii's local moderation ballot announcement API now performs the deposit confirmation gate before admitting a ballot",
        "its moderation settlement worker replays and subscribes to local tallied ballot events",
        "The checker also exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "`evidence_contract` map for the selected required kinds",
        "The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.",
    )
    stale: dict[str, list[str]] = {}
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_appeal_pricing_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        matched_stale = [phrase for phrase in stale_phrases if phrase in normalized]
        missing = [phrase for phrase in current_phrases if phrase not in normalized]
        if matched_stale:
            stale[str(path.relative_to(REPO_ROOT))] = matched_stale
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert stale == {}
    assert missing_current == {}


def test_appeal_finance_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_appeal_finance_canary.py")
    builder_test = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_appeal_finance_canary_test.py"
    )
    pricing_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_appeal_finance_pricing_config_canary.args.example"
    )
    reconciliation_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_appeal_finance_multi_peer_reconciliation_canary.args.example"
    )
    docs = read(SORAFS_APPEAL_PRICING_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_QUOTE_ROUTES" in builder
    assert "REQUIRED_DEPOSIT_ROUTES" in builder
    assert "REQUIRED_SETTLEMENT_ROUTES" in builder
    assert "REQUIRED_PAYLOAD_KINDS" in builder
    assert "REQUIRED_METRICS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "raw_instruction_included" in builder
    assert "signed_transaction_included" in builder
    assert "response_bodies_included" in builder
    assert "raw_ledger_included" in builder
    assert "test_generated_canaries_pass_full_appeal_finance_gate" in builder_test
    assert "test_under_replicated_multi_peer_run_fails_before_write" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind\npricing_config" in pricing_example
    assert "--verified-claim\npricing_config_present" in pricing_example
    assert "--kind\nmulti_peer_reconciliation" in reconciliation_example
    assert "--verified-claim\nqc_quorum_satisfied" in reconciliation_example
    assert "build_sorafs_appeal_finance_canary.py" in docs
    assert "payload-free SFM-4b2 appeal finance canary builder" in docs


def test_unshipped_appeal_finance_public_promotion_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/appeals/pricing/daemon",
        "/v1/sorafs/appeals/finance/pricing-daemon",
        "/v1/sorafs/appeals/finance/public-dashboard",
        "/v1/sorafs/appeals/finance/hosted-dashboard",
        "/v1/sorafs/appeals/finance/dashboard/public",
        "/v1/sorafs/appeals/finance/reconciliation/multi-peer",
        "/v1/sorafs/appeals/finance/multi-peer-reconciliation",
        "/v1/sorafs/appeals/finance/promotion",
    )
    unshipped_cli_subcommands = (
        "pricing-daemon",
        "appeal-pricing-daemon",
        "appeal-finance-dashboard",
        "appeal-dashboard-serve",
        "appeal-finance-public-dashboard",
        "appeal-finance-reconcile-multi-peer",
        "appeal-finance-multi-peer-reconcile",
        "appeal-finance-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_transparency_deployed_services_stay_open_in_docs() -> None:
    source = read(SORAFS_TRANSPARENCY_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "It does not yet ship deployed GAR/moderation/appeal producers that call the source-entry route, captured deployed aggregate producer/scheduler rollout evidence, proof service hardening, deployed public receipt explorer rollout evidence, deployed proof-token issuance producers/explorer-linking rollout evidence, or deployed moderation ledger publication service described by the original plan.",
        "The local ledger layer now covers bundle materialization and readback; remaining deployed-service work is to wire live producer, anchoring, explorer, proof-token, and hardening evidence for:",
        "deployed GAR/moderation/appeal/legal-hold/redaction/evidence-viewer/proof-token issuance service producers and captured rollout evidence remain open",
        "deployed anchoring and captured service rollout evidence remain open",
        "deployed service hardening and captured public rollout evidence are not shipped",
        "captured deployed public rollout evidence is not shipped",
        "captured deployed source-event producer and scheduler rollout evidence remains open",
        "Document only the local `/v1/sorafs/transparency/*` readback",
        "Do not document generic `/v1/transparency/*` endpoints or deployed public receipt explorer rollout as shipped until the live builder, deployment, and explorer paths exist.",
        "Wire deployed GAR receipt, moderation validator evidence, appeal outcome, legal-hold/redaction notice, and future evidence-viewer audit producers",
        "Attach deployed publisher identities, anchoring, and service rollout evidence",
        "Finish deployed proof API hardening and capture public receipt explorer rollout evidence",
        "Wire deployed proof-token issuance producers and public explorer linking",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_transparency_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "so dry-run collection plans and downstream automation can inspect the exact evidence contract before live collection.",
        "`--dry-run` emits the command plan plus the checker-backed `evidence_contract` field map without contacting live services.",
        "It also rejects duplicate or unsupported `--source-entry` kinds before rendering the plan or contacting live services.",
        "transparency rollout collection runner reject non-lowercase, wrong-length, or otherwise malformed `--cycle-id` values before rendering dry-run command plans or contacting deployed cycle-detail routes.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_transparency_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_transparency_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_transparency_canary.py")
    builder_tests = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_transparency_canary_test.py"
    )
    docs = read(SORAFS_TRANSPARENCY_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS transparency rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload)" in builder
    assert "DEFAULT_REQUIRED_SOURCE_KINDS" in builder
    assert "REQUIRED_PUBLICATION_ROUTES" in builder
    assert "REQUIRED_EXPLORER_ROUTES" in builder
    assert "REQUIRED_PRIVACY_AGGREGATE_ACTIONS" in builder
    assert "SOURCE_BOUND_KINDS" in builder
    assert "CYCLE_BOUND_KINDS" in builder
    assert "test_generated_canaries_pass_full_transparency_gate" in builder_tests
    assert "scripts/build_sorafs_transparency_canary.py" in docs
    assert "scripts/build_sorafs_transparency_canary.py" in roadmap
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_transparency_source_entry_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_transparency_publication_canary.args.example"
    ).is_file()


def test_transparency_docs_do_not_reopen_shipped_local_ledger_layer() -> None:
    stale_phrases = (
        "service still needs a live runtime ledger layer that ingests and publishes:",
    )
    required_current = (
        "The local ledger layer now covers bundle materialization and readback; remaining deployed-service work is to wire live producer, anchoring, explorer, proof-token, and hardening evidence for:",
        "local source-entry cycle builder, local node publication to filesystem/CAR",
        "Local Torii readback for published cycles, entry proofs, proof-token issuance indexes, explorer snapshots, and proof-token verification is shipped",
    )
    stale: dict[str, list[str]] = {}
    missing: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_transparency_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        matched_stale = [phrase for phrase in stale_phrases if phrase in normalized]
        missing_current = [
            phrase for phrase in required_current if phrase not in normalized
        ]
        if matched_stale:
            stale[str(path.relative_to(REPO_ROOT))] = matched_stale
        if missing_current:
            missing[str(path.relative_to(REPO_ROOT))] = missing_current

    assert stale == {}
    assert missing == {}


def test_unshipped_transparency_deployed_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/transparency/",
        "/v1/sorafs/transparency/deployed-producers",
        "/v1/sorafs/transparency/producer-service",
        "/v1/sorafs/transparency/anchoring-service",
        "/v1/sorafs/transparency/public-explorer",
        "/v1/sorafs/transparency/proof-api/public",
        "/v1/sorafs/transparency/proof-token-producers",
        "/v1/sorafs/transparency/moderation-ledger-service",
        "/v1/sorafs/transparency/promotion",
    )
    unshipped_cli_subcommands = (
        "transparency-producer-service",
        "transparency-anchoring-service",
        "transparency-proof-api-serve",
        "public-explorer-serve",
        "transparency-public-explorer",
        "proof-token-producer-service",
        "privacy-aggregate-scheduler-service",
        "moderation-ledger-service",
        "transparency-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_gateway_compliance_controller_services_stay_unshipped_in_docs() -> None:
    source = read(SORAFS_GATEWAY_COMPLIANCE_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_unshipped = (
        "The repository does not yet ship an always-on central compliance controller daemon, deployed moderation toggle service, deployed public receipt explorer, or full appeal-driven override workflow. The local SFM-4c transparency ledger builder and readback surface are shipped, but promotion evidence now has to prove controller runtime, moderation-toggle, deployed-publication, and multi-gateway boundaries before gateway compliance can be marked ready.",
        "Ship the always-on compliance controller daemon that fetches external feeds, normalizes updates, signs them, distributes them to gateways, and tracks acknowledgements.",
        "the daemon itself still needs production deployment",
        "Persist denylist/catalog state and update history through the configured production storage path instead of relying only on local bundle ingestion.",
        "Implement moderation toggle APIs, approval workflows, expiry handling, and operator audit trails.",
        "the service itself still needs production deployment",
        "Connect appeal outcomes to gateway policy overrides and cache invalidation.",
        "Wire deployed GAR receipts, proof-token indexes, and moderation events through the shipped local SFM-4c transparency source-entry and publication paths, then capture deployed publication evidence.",
        "Capture staged multi-gateway rollout artifacts that satisfy the SFM-4 evidence gate before promoting gateway compliance changes to production.",
    )
    missing = [phrase for phrase in required_unshipped if phrase not in normalized]

    assert missing == []


def test_gateway_compliance_docs_do_not_reopen_shipped_transparency_builder() -> None:
    stale_phrases = (
        "SFM-4c transparency ledger builder, public receipt explorer",
        "Publish GAR receipts, proof-token indexes, and moderation events through the SFM-4c transparency ledger once that builder exists.",
    )
    required_current = (
        "The local SFM-4c transparency ledger builder and readback surface are shipped",
        "promotion evidence now has to prove controller runtime, moderation-toggle, deployed-publication, and multi-gateway boundaries",
        "Wire deployed GAR receipts, proof-token indexes, and moderation events through the shipped local SFM-4c transparency source-entry and publication paths, then capture deployed publication evidence.",
    )
    stale: dict[str, list[str]] = {}
    missing: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_gateway_compliance_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        matched_stale = [phrase for phrase in stale_phrases if phrase in normalized]
        missing_current = [
            phrase for phrase in required_current if phrase not in normalized
        ]
        if matched_stale:
            stale[str(path.relative_to(REPO_ROOT))] = matched_stale
        if missing_current:
            missing[str(path.relative_to(REPO_ROOT))] = missing_current

    assert stale == {}
    assert missing == {}


def test_gateway_compliance_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "payload-free SFM-4 promotion evidence for feed promotion, controller runtime, moderation-toggle canaries, gateway reload",
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "including a checker-backed `evidence_contract` map with the schema and required payload fields for each selected evidence kind.",
        "Controller, moderation-toggle, reload, enforcement, honey-audit, appeal, transparency, observability, and governance artifacts must carry the same `bundle_digest_hex` as a valid feed-promotion artifact",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_gateway_compliance_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_gateway_compliance_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_gateway_compliance_canary.py")
    builder_test = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_gateway_compliance_canary_test.py"
    )
    controller_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_gateway_compliance_controller_canary.args.example"
    )
    toggle_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_gateway_compliance_moderation_toggle_canary.args.example"
    )
    docs = read(SORAFS_GATEWAY_COMPLIANCE_PLAN)

    assert "CANARY_KINDS = (\"controller_runtime\", \"moderation_toggle\")" in builder
    assert "CONTROLLER_TRUE_CLAIMS" in builder
    assert "MODERATION_TRUE_CLAIMS" in builder
    assert "FORBIDDEN_PAYLOAD_CLAIMS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "\"config_source\": \"iroha_config\"" in builder
    assert "raw_feeds_included" in builder
    assert "raw_toggle_payloads_included" in builder
    assert "test_generated_canaries_pass_gateway_gate_with_feed_promotion_anchor" in builder_test
    assert "test_missing_verified_claim_fails_closed" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind controller_runtime" in controller_example
    assert "--verified-claim rollback_plan_verified" in controller_example
    assert "--kind moderation_toggle" in toggle_example
    assert "--verified-claim rollback_verified" in toggle_example
    assert "build_sorafs_gateway_compliance_canary.py" in docs
    assert "payload-free controller-runtime and moderation-toggle canary builder" in docs


def test_unshipped_gateway_compliance_service_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/gateway/compliance/controller",
        "/v1/sorafs/gateway/compliance/controller-runtime",
        "/v1/sorafs/gateway/compliance/moderation-toggle",
        "/v1/sorafs/gateway/compliance/toggles",
        "/v1/sorafs/gateway/compliance/appeal-overrides",
        "/v1/sorafs/gateway/compliance/feed-sync",
        "/v1/sorafs/gateway/compliance/acknowledgements",
        "/v1/sorafs/gateway/compliance/history",
        "/v1/sorafs/gateway/compliance/promotion",
    )
    unshipped_cli_subcommands = (
        "compliance-controller",
        "controller-daemon",
        "gateway-compliance-daemon",
        "gateway-compliance-service",
        "moderation-toggle-service",
        "moderation-toggle",
        "appeal-override-service",
        "appeal-override-apply",
        "gateway-compliance-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_gateway_load_rollout_evidence_work_stays_open_in_docs() -> None:
    source = read(SORAFS_GATEWAY_LOAD_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "It does not open a live HTTP/3 gateway or sleep through a wall-clock soak test",
        "Live staging load evidence",
        "No committed SoraFS HTTP/3 gateway endpoint is present in this checkout; add HTTP/3 scenarios only after the gateway exposes that transport.",
        "Archive signed local conformance reports from `ci/check_sorafs_gateway_conformance.sh`",
        "Run a live staging load rig with the same fixture bundle",
        "Add a live-target adapter if operators need the integration test to exercise a deployed gateway instead of the fixture-backed adapter.",
        "Add HTTP/3 scenarios only after the SoraFS gateway exposes a committed HTTP/3 endpoint and configuration surface.",
        "Record cold-cache SLO baselines after the staging hardware profile is chosen.",
        "`scripts/check_sorafs_gateway_load_rollout_evidence.py` validates payload-free local conformance, live staging load, telemetry/SLO, transport-scope, and governance approval evidence before SF-5a load promotion.",
        "`scripts/run_sorafs_gateway_load_rollout_evidence.py` emits the matching collection plan and dry-run evidence contract",
        "runner validates the schema-closed collection plan, required kinds, thresholds, external evidence map, evidence contract, and command steps before dry-run output or verifier execution.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_gateway_load_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_gateway_load_canary.py")
    builder_tests = read(SCRIPTS_DIR / "tests" / "build_sorafs_gateway_load_canary_test.py")
    plan = read(SORAFS_GATEWAY_LOAD_PLAN)
    roadmap = read(REPO_ROOT / "roadmap.md")

    assert "Build payload-free SoraFS gateway load rollout canary artifacts." in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "REQUIRED_SCENARIOS" in builder
    assert "REQUIRED_METRICS" in builder
    assert "test_generated_canaries_pass_full_gateway_load_gate" in builder_tests
    assert "scripts/build_sorafs_gateway_load_canary.py" in plan
    assert "scripts/build_sorafs_gateway_load_canary.py" in roadmap
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_gateway_load_local_conformance_canary.args.example"
    ).is_file()
    assert (
        SCRIPTS_DIR
        / "examples"
        / "sorafs_gateway_load_staging_canary.args.example"
    ).is_file()


def test_unshipped_gateway_load_live_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/gateway/load/live",
        "/v1/sorafs/gateway/load/staging",
        "/v1/sorafs/gateway/load/http3",
        "/v1/sorafs/gateway/load/promotion",
        "/v1/sorafs/gateway/load/soak",
    )
    unshipped_cli_subcommands = (
        "gateway-load-live",
        "gateway-load-staging",
        "gateway-load-http3",
        "gateway-load-promote",
        "gateway-load-soak",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_sorafs_production_readiness_aggregate_gate_is_documented() -> None:
    plan = re.sub(r"\s+", " ", read(SORAFS_RELEASE_PIPELINE_PLAN))
    roadmap_source = re.sub(r"\s+", " ", read(REPO_ROOT / "roadmap.md"))
    checker = read(SCRIPTS_DIR / "check_sorafs_production_readiness.py")
    runner = read(SCRIPTS_DIR / "run_sorafs_production_readiness.py")
    direct_example = read(EXAMPLES_DIR / "sorafs_production_readiness.args.example")
    runner_example = read(
        EXAMPLES_DIR / "sorafs_production_readiness_collection.args.example"
    )

    required_markers = (
        "`scripts/check_sorafs_production_readiness.py` is the final aggregate SoraFS promotion gate",
        "sorafs.production_readiness.aggregate_gate.v1",
        "`scripts/run_sorafs_production_readiness.py` accepts reviewed per-lane summary paths",
        "requires exactly one summary input per required gate",
        "requires an explicit canonical `--deployment-id`/`--environment` pair",
        "validates the schema-closed collection plan envelope against the built command plan before dry-run output or execution",
        "rejects reviewed summary input paths with secret-looking, control-character, parent/current, or platform-specific components before they can be rendered into dry-run command plans",
        "rejects plan-rendered verifier, output-directory, and summary-output paths plus runner input files/directories with secret-looking, control-character, parent/current, drive-prefix, or platform-specific components before dry-run output through the shared runner preflight",
        "artifact/load-error lists",
        "no extra `required` rows",
        "top-level evidence/artifact counts consistent with the validated rows",
        "evidence file counts to match the distinct recognized artifact paths",
        "threshold metadata as a non-empty canonical non-negative integer map that the aggregate row preserves for release review",
        "reject extra top-level lane-summary fields outside the schema-closed payload-free lane summary contract",
        "validate allowed top-level lane metadata as payload-free canonical strings, non-negative integers, booleans, objects, and lists with expected container shapes",
        "validate exact lowercase-hex binding-list metadata shapes before aggregate promotion",
        "validate exact lowercase-hex and positive-integer scalar list metadata shapes before aggregate promotion",
        "validate governance public-head identifiers as lowercase hex list metadata before aggregate promotion",
        "validate exact object-list metadata shapes before aggregate promotion",
        "reject exact duplicate object-list metadata entries while preserving artifact order",
        "validate exact object metadata shapes before aggregate promotion",
        "require set-derived lane metadata lists to be duplicate-free and sorted in canonical order",
        "bind those metadata fields to the lane-specific contract that emits them",
        "canonical required-row schema labels when present",
        "reject extra required-row fields outside the schema-closed payload-free required-row contract",
        "canonical unique archive-relative paths without absolute, empty, current, parent, or platform-specific path segments",
        "canonical artifact schema/status labels when present",
        "reject extra artifact-row fields outside the schema-closed payload-free artifact contract",
        "per-lane rollout/release checkers to normalize artifact row paths through the shared archive-label helper before summary rendering",
        "deriving labels relative to evidence directories or safe explicit basenames",
        "require top-level recognized-artifact inventory and validate it against the per-kind required-row artifact counts and `(kind, path, sha256)` identities plus matching required artifact metadata instead of ignoring it",
        "archive-relative summary path labels derived from evidence-directory membership or safe explicit basenames",
        "validate the schema-closed aggregate summary envelope before writing the final production-readiness report",
        "require aggregate status to match canonical aggregate diagnostics",
        "ready aggregate summaries must carry complete deployment context with a reviewed deployment id, a final `prod`/`production` environment, and only present, valid required rows",
        "aggregate required row deployment_id must match aggregate deployment_id",
        "aggregate required row environment must match aggregate environment",
        "require aggregate recognized-summary counts to match present required rows",
        "validate final aggregate required rows for exact present and missing row output contracts",
        "validate invalid aggregate required-row metadata before blocked rows are emitted for release review",
        "pin deterministic missing-row diagnostics for absent lane summaries",
        "pin deterministic duplicate-summary diagnostics for duplicate lane summaries",
        "count every duplicate lane-summary input while keeping one duplicate row diagnostic per gate",
        "pin aggregate blockers for unknown schemas and explicit unrequired summaries",
        "reject unknown summary schemas discovered in summary directories",
        "rejects explicit summaries for lanes outside a narrowed `--require-gate` selection",
        "require an explicit final `--deployment-id`/`--environment` pair even for direct checker invocations",
        "SoraFS production promotion now has an aggregate readiness gate over the existing per-lane rollout/release evidence summaries",
        "The same shared validator now rejects compact and tokenized pre-release aliases such as `prerelease`, `releasecandidate`, `candidateproduction`, `productionpreview`, `preprodrelease`, `pre-production`, `production-candidate`, `prod-rc`, `prod-preview`, and `preprod-production`",
        "This does not close the live deployment gaps above",
    )
    missing = [
        marker
        for marker in required_markers
        if marker not in plan and marker not in roadmap_source
    ]

    assert missing == []
    assert 'SUMMARY_SCHEMA = "sorafs.production_readiness.aggregate_gate.v1"' in checker
    assert "DEFAULT_REQUIRED_GATES" in checker
    assert "visit_sensitive_fields(" in checker
    assert "test_sensitive_summary_key_diagnostic_is_sanitized" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "require_absent_or_empty_error_list(payload, \"load_errors\", errors)" in checker
    assert "require_threshold_map(payload, \"thresholds\", errors)" in checker
    assert "must be present" in checker
    assert "must not be empty" in checker
    assert "keys must be canonical strings" in checker
    assert "must be a non-negative integer" in checker
    assert '"thresholds": thresholds' in checker
    assert "PAYLOAD_FREE_SUMMARY_FIELDS" in checker
    assert "frozenset().union(" in checker
    assert "*GATE_METADATA_FIELDS.values()" in checker
    assert (
        "PAYLOAD_FREE_SUMMARY_CORE_FIELDS | PAYLOAD_FREE_SUMMARY_METADATA_FIELDS"
        in checker
    )
    assert "require_payload_free_summary_fields(payload, errors)" in checker
    assert "def is_sensitive_diagnostic_key" in checker
    assert "def payload_free_diagnostic_key_label" in checker
    assert "is not allowed in payload-free lane summary" in checker
    assert "<sensitive-key> is not allowed in payload-free lane summary" not in checker
    assert "{field}.{key_diagnostic_label} must not be present" in checker
    assert "test_sensitive_summary_key_diagnostic_sanitizes_canonical_key" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_sensitive_threshold_key_is_not_carried_into_summary" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "PAYLOAD_FREE_SUMMARY_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_HEX_LIST_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_HEX_BINDING_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_POSITIVE_INT_LIST_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_OBJECT_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_ORDERED_LIST_METADATA_FIELDS" in checker
    assert "PAYLOAD_FREE_SUMMARY_HEX_METADATA_LENGTHS" in checker
    assert "GATE_METADATA_FIELDS" in checker
    assert "validate_payload_free_summary_metadata(gate, payload, errors)" in checker
    assert "payload_free_summary_metadata_deployment_contexts(gate, payload)" in checker
    assert (
        "deployment context must match across artifacts and metadata" in checker
    )
    assert "validate_payload_free_summary_metadata_fingerprint_tethers" in checker
    assert "must match recognized artifact fingerprints" in checker
    assert "PAYLOAD_FREE_SUMMARY_OBJECT_LIST_REQUIRED_KIND_COUNTS" in checker
    assert "validate_payload_free_object_list_metadata_counts" in checker
    assert "length must match `{kind_name}` required artifact count" in checker
    assert "must be 64 lowercase hex characters" in checker
    assert "test_digest_list_metadata_entries_are_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "valid_reference_decision_ids" in checker
    assert "valid_public_head_cids" in checker
    assert "must be a positive integer" in checker
    assert "test_reference_decision_id_metadata_entries_are_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_public_head_cid_metadata_entries_are_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_hex_list_metadata_must_match_recognized_artifact_fingerprints" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_hex_binding_metadata_must_match_recognized_artifact_fingerprints" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_scalar_metadata_must_match_recognized_artifact_fingerprints" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_object_list_metadata_must_match_required_artifact_count" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_provider_count_values_metadata_entries_are_positive_ints" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "must be a payload-free metadata object" in checker
    assert "is not allowed in payload-free object metadata" in checker
    assert "test_object_list_metadata_entries_are_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_object_list_metadata_entries_must_not_duplicate" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_provider_bake_metadata_completed_at_must_not_precede_start" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "validate_payload_free_object_metadata" in checker
    assert "test_deployment_context_metadata_mismatch_fails" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_deployment_context_metadata_entries_are_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_object_list_metadata_deployment_context_mismatch_fails" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_multi_peer_run_metadata_deployment_context_mismatch_fails" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_provider_bake_metadata_deployment_context_mismatch_fails" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "validate_payload_free_ordered_list_metadata" in checker
    assert "must not contain duplicate metadata entries" in checker
    assert "must be sorted in canonical order" in checker
    assert "test_digest_list_metadata_entries_must_be_unique_and_sorted" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_binding_metadata_entries_must_be_unique_and_sorted" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_provider_count_values_metadata_must_be_unique_and_sorted" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "must be a payload-free binding object" in checker
    assert "is not allowed in payload-free binding metadata" in checker
    assert "test_hex_binding_metadata_entries_are_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "must be {expected_hex_length} lowercase hex characters" in checker
    assert "test_reputation_top_level_hex_metadata_is_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "is not allowed for `" in checker
    assert "must be a payload-free metadata list" in checker
    assert "must contain only payload-free canonical metadata" in checker
    assert "validate_payload_free_artifact_fingerprint(artifact, path, errors)" in checker
    assert "test_artifact_fingerprint_metadata_must_be_payload_free" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "AGGREGATE_REQUIRED_GATE_ROW_FIELDS" in checker
    assert "validate_aggregate_gate_row_output" in checker
    assert "test_aggregate_gate_row_output_shape_is_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "AGGREGATE_SUMMARY_FIELDS" in checker
    assert "AGGREGATE_MISSING_GATE_ROW_FIELDS" in checker
    assert "validate_aggregate_required_row_output" in checker
    assert (
        "aggregate invalid row newest_generated_at_unix must be >= oldest_generated_at_unix"
        in checker
    )
    assert "f\"{gate.name} aggregate invalid row deployment_id\"" in checker
    assert "aggregate invalid row environment must be canonical when present" in checker
    assert "aggregate invalid row {threshold_error}" in checker
    assert "deterministic missing summary diagnostic" in checker
    assert "validate_duplicate_summary_diagnostics" in checker
    assert "deterministic duplicate summary diagnostic exactly once" in checker
    assert "duplicate-summary diagnostics must match duplicate summary inputs" in checker
    assert "validate_disallowed_summary_diagnostics" in checker
    assert "unknown-schema diagnostics must match discovered unknown summaries" in checker
    assert "unrequired-gate diagnostics must match explicit unrequired summaries" in checker
    assert "validate_aggregate_summary_output" in checker
    assert "aggregate summary status must match aggregate diagnostics" in checker
    assert "summary.get(\"status\") != evidence_gate_status(error_values)" in checker
    assert (
        "aggregate summary ready deployment must include deployment_id and environment"
        in checker
    )
    assert "PRODUCTION_READY_ENVIRONMENTS" in checker
    assert "FORBIDDEN_PRODUCTION_DEPLOYMENT_MARKERS" in checker
    assert "def is_production_ready_environment" in checker
    assert "def require_reviewed_deployment_id_value" in checker
    assert "def require_production_deployment_id_value" in checker
    assert "require_rollout_deployment_id" in checker
    assert "aggregate environment must be production" in checker
    assert "aggregate row environment must be production" in checker
    assert "Required final deployment id shared by every lane summary artifact" in checker
    assert "Required final prod/production environment shared by every lane" in checker
    assert "Optional expected deployment id" not in checker
    assert "Optional expected environment" not in checker
    assert (
        "aggregate required row deployment_id must match aggregate deployment_id"
        in checker
    )
    assert (
        "aggregate required row environment must match aggregate environment"
        in checker
    )
    assert (
        "aggregate production readiness requires --deployment-id and --environment"
        in checker
    )
    assert "test_direct_checker_requires_explicit_deployment_context" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "non-production deployment markers" in checker
    assert "--environment must be production for this gate" in checker
    assert "test_unreviewed_deployment_id_cannot_promote_production_readiness" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_staging_deployment_id_cannot_promote_production_readiness" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert (
        "test_joined_nonproduction_alias_cannot_promote_production_readiness"
        in read(SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py")
    )
    assert "test_explicit_unreviewed_deployment_id_fails_before_validation" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_explicit_staging_deployment_id_fails_before_validation" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_staging_environment_cannot_promote_production_readiness" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_explicit_nonproduction_environment_fails_before_validation" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "is_production_ready_environment" in runner
    assert "require_production_deployment_id_value" in runner
    assert "production readiness runner environment must be production" in runner
    assert (
        "Required final deployment id shared by every required lane summary"
        in runner
    )
    assert (
        "Required final prod/production environment shared by every required"
        in runner
    )
    assert "Optional expected deployment id" not in runner
    assert "Optional expected environment" not in runner
    assert (
        "production readiness runner deployment_id must not contain" in read(
            SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
        )
    )
    assert "test_help_marks_final_deployment_context_required" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_nonproduction_environment_fails" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_staging_deployment_id_fails" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_joined_nonproduction_deployment_id_fails" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert (
        "aggregate summary ready recognized_summary_count must match required gate count"
        in checker
    )
    assert "aggregate summary ready rows must all be present and valid" in checker
    assert "recognized_summary_count must match present required rows" in checker
    assert "test_aggregate_required_row_output_shape_is_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_aggregate_summary_output_shape_is_validated" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_complete_lane_fixture_summaries_pass_aggregate_contract" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_complete_lane_fixture_summaries_pass_full_aggregate_cli" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "drifted aggregate diagnostic" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "invalid required row" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "runtime-only-deployment" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert ".schema must be canonical when present" in checker
    assert "PAYLOAD_FREE_REQUIRED_ROW_FIELDS" in checker
    assert "require_payload_free_required_row_fields" in checker
    assert "is not allowed in payload-free required row" in checker
    assert "deployment_context_reviewed" in checker
    assert "required row labels must be canonical strings" in checker
    assert "sanitized_required_row_labels" not in checker
    assert "required_kinds contains duplicate kind" in checker
    assert "required_kinds contains duplicate `{kind_label}`" not in checker
    assert "required_kinds contains unknown full-contract kinds {extra}" not in checker
    assert "test_malformed_extra_required_row_label_is_sanitized" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_extra_required_kind_labels_are_payload_free" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_extra_required_row_label_is_payload_free" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "required contains rows outside the full" in checker
    assert "f\"{sanitized_required_row_labels(extra_required_rows)}\"" not in checker
    assert "belongs to unrequired" in checker
    assert ".sha256 must be canonical lowercase SHA-256" in checker
    assert "def is_archive_portable_artifact_path" in checker
    assert "def aggregate_summary_path_label" in checker
    assert "aggregate_summary_path_label(path, evidence_dirs)" in checker
    assert "path.startswith((\"/\", \"\\\\\"))" in checker
    assert "part not in {\".\", \"..\"}" in checker
    assert "aggregate row path must be archive-relative without" in checker
    assert "test_artifact_paths_must_be_archive_portable" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_artifact_paths_reject_platform_specific_segments" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_aggregate_lane_summary_paths_are_archive_relative" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_explicit_lane_summary_path_uses_safe_basename" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert ".schema must be canonical when present" in checker
    assert 'require_optional_artifact_label(artifact, "status", path, errors)' in checker
    assert "PAYLOAD_FREE_ARTIFACT_FIELDS" in checker
    assert "require_payload_free_artifact_fields" in checker
    assert "is not allowed in payload-free artifact summary" in checker
    assert (
        "test_sensitive_required_and_artifact_field_diagnostics_are_sanitized"
        in read(SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py")
    )
    assert ".kind must match required row kind" in checker
    assert "unknown SoraFS readiness summary schema" in checker
    assert "unknown SoraFS readiness summary schema `{schema}`" not in checker
    assert (
        'f"{path_diagnostic_label(path)}: unknown SoraFS readiness summary schema"'
        not in checker
    )
    assert "test_unknown_sorafs_schema_in_summary_dir_fails" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert ".artifacts must not duplicate artifact paths" in checker
    assert ".artifacts must not duplicate artifact identities" in checker
    assert "recognized_artifacts must not duplicate artifact paths" in checker
    assert "test_required_artifact_duplicate_paths_fail" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "test_recognized_artifacts_duplicate_paths_fail" in read(
        SCRIPTS_DIR / "tests" / "check_sorafs_production_readiness_test.py"
    )
    assert "recognized_artifacts length must match recognized_artifact_count" in checker
    assert "recognized_artifacts must be present" in checker
    assert "must be a non-empty array" in checker
    assert "recognized_artifact_paths" in checker
    assert "evidence_file_count must match recognized artifact path count" in checker
    assert "recognized_artifacts must match required artifact counts" in checker
    assert "recognized_artifacts must match required artifact identities" in checker
    assert "required_artifacts_by_identity" in checker
    assert 'for metadata_field in ("schema", "status", "fingerprint")' in checker
    assert ".kind must be part of the full" in checker
    assert "recognized_artifact_count must match required row artifact total" in checker
    assert "max_summary_artifact_age_secs" in checker
    assert "summary_contract" in runner
    assert "supplied for unrequired" in runner
    assert "PLAN_FIELDS" in runner
    assert "def validate_plan_json" in runner
    assert "production readiness runner plan must be an object" in runner
    assert "render_runner_plan(rendered)" in runner
    assert "production readiness runner plan must be strict JSON renderable" in runner
    assert "production readiness runner plan fields must match the schema-closed contract" in runner
    assert "production readiness runner plan steps must match command plan" in runner
    assert "production readiness runner plan deployment_id" in runner
    assert "production readiness runner plan environment must be production" in runner
    assert "test_plan_json_deployment_context_must_be_final_production" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "def summary_input_path_is_plan_safe" in runner
    assert "plan_rendered_path_is_safe" in runner
    assert "PLAN_RENDERED_PATH_ERROR" in runner
    assert "def is_sensitive_path_component" not in runner
    assert "HIGH_RISK_SENSITIVE_KEY_FRAGMENTS" not in runner
    assert "summary input paths must not contain" in runner
    assert "test_summary_input_path_components_must_be_plan_safe" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_summary_input_path_safety_accepts_digest_labels" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_plan_rendered_output_path_components_must_be_plan_safe" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_plan_rendered_summary_output_path_components_must_be_plan_safe" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_plan_rendered_verifier_path_components_must_be_plan_safe" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_plan_rendered_path_safety_rejects_drive_prefix" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert runner.index(
        "plan_errors = validate_plan_json(rendered_plan, plan, args)"
    ) < runner.index("if args.dry_run:")
    assert "production readiness runner requires exactly one summary input per required gate" in runner
    assert "production readiness runner requires --deployment-id and --environment" in runner
    assert (
        "production readiness runner deployment context must use canonical labels"
        in runner
    )
    assert "canonical_string(args.deployment_id)" in runner
    assert "test_partial_deployment_context_fails" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_malformed_deployment_context_fails" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_duplicate_required_summary_flag_fails" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_plan_json_shape_is_validated" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_execution_rejects_non_object_plan_before_running" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_execution_rejects_unrenderable_plan_before_running" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "test_execution_rejects_plan_validation_drift_before_running" in read(
        SCRIPTS_DIR / "tests" / "run_sorafs_production_readiness_test.py"
    )
    assert "--evidence-dir artifacts/sorafs/production-readiness/summaries" in direct_example
    assert "--gateway-load-summary artifacts/sorafs/gateway-load/summary.json" in runner_example
    assert "--dry-run" in runner_example


def test_sorafs_production_readiness_aggregate_covers_every_lane_checker() -> None:
    aggregate = load_script_module(
        PRODUCTION_READINESS_CHECKER,
        "check_sorafs_production_readiness_contract",
    )
    runner = load_script_module(
        PRODUCTION_READINESS_RUNNER,
        "run_sorafs_production_readiness_contract",
    )
    checker_modules = {
        path.name: load_script_module(path, f"{path.stem}_aggregate_contract")
        for path in CHECKERS
    }
    checker_schema_to_name = {
        module.SUMMARY_SCHEMA: name for name, module in checker_modules.items()
    }
    aggregate_schema_to_gate = {
        gate.schema: gate for gate in aggregate.GATE_SUMMARY_KINDS
    }
    missing_from_aggregate = sorted(
        set(checker_schema_to_name) - set(aggregate_schema_to_gate)
    )
    missing_checker = sorted(
        set(aggregate_schema_to_gate) - set(checker_schema_to_name)
    )
    required_kind_mismatches = {
        gate.name: {
            "aggregate": list(gate.required_kinds),
            "checker": list(
                checker_modules[
                    checker_schema_to_name[gate.schema]
                ].DEFAULT_REQUIRED_KINDS
            ),
        }
        for gate in aggregate.GATE_SUMMARY_KINDS
        if tuple(gate.required_kinds)
        != tuple(
            checker_modules[
                checker_schema_to_name[gate.schema]
            ].DEFAULT_REQUIRED_KINDS
        )
    }
    missing_threshold_emitters = sorted(
        path.name for path in CHECKERS if '"thresholds": {' not in read(path)
    )
    missing_recognized_emitters = sorted(
        path.name
        for path in CHECKERS
        if '"recognized_artifacts":' not in read(path)
    )

    assert missing_from_aggregate == []
    assert missing_checker == []
    assert required_kind_mismatches == {}
    assert missing_threshold_emitters == []
    assert missing_recognized_emitters == []
    assert tuple(aggregate.DEFAULT_REQUIRED_GATES) == tuple(
        gate.name for gate in aggregate.GATE_SUMMARY_KINDS
    )
    assert set(runner.SUMMARY_OPTIONS_BY_GATE) == set(aggregate.DEFAULT_REQUIRED_GATES)
    assert set(runner.SUMMARY_FLAGS_BY_GATE) == set(aggregate.DEFAULT_REQUIRED_GATES)
    assert all(
        runner.SUMMARY_FLAGS_BY_GATE[gate].startswith("--")
        for gate in aggregate.DEFAULT_REQUIRED_GATES
    )


def test_reserve_rent_live_control_plane_stays_open_in_docs() -> None:
    source = read(SORAFS_RESERVE_RENT_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "The production reserve/rent control plane is still incomplete.",
        "Signed local movement routes now authenticate and record transfer intents, but they do not submit reserve transfers or verify chain finality.",
        "live chain submission, automatic finality polling, and live account mutation for credit lines are still target service work.",
        "Broader governance source-entry effects beyond current provider denylist projection and live scheduler canary evidence remain target downstream work.",
        "live account mutation for local credit-line state",
        "broader downstream compliance application evidence for governance source entries",
        "staged provider bake evidence, including live scheduled lifecycle canaries",
        "Remaining: live chain custody submission and automatic finality polling for signed movement intents, live account mutation for local credit-line state, broader downstream compliance application evidence for governance source entries, and staged provider bake evidence, including live scheduled lifecycle canaries, that passes the rollout gate.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_reserve_rent_docs_do_not_reopen_shipped_local_control_plane() -> None:
    stale_phrases = (
        "The production reserve/rent control plane is still outstanding.",
        "no Torii REST surface for reserve lifecycle management",
        "no shipped CLI for provider status",
        "Remaining: reserve lifecycle service, signed Torii routes",
        "runtime reserve movement/authentication, persisted lifecycle-stage automation",
    )
    current_phrases = (
        "Signed local movement routes now authenticate and record transfer intents",
        "now a local authenticated appeal/policy handoff surface",
        "config-backed scheduler can drive the same advancement path",
        "signed reserve top-up/withdrawal/status/movement/custody/credit-line/appeal/policy CLI commands",
        "Remaining: live chain custody submission and automatic finality polling for signed movement intents",
    )
    stale: dict[str, list[str]] = {}
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_reserve_rent_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        matched_stale = [phrase for phrase in stale_phrases if phrase in normalized]
        if matched_stale:
            stale[str(path.relative_to(REPO_ROOT))] = matched_stale
        missing = [phrase for phrase in current_phrases if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert stale == {}
    assert missing_current == {}


def test_reserve_rent_docs_keep_rollout_contract_markers() -> None:
    required_current = (
        "The checker exports its required top-level payload fields as `EVIDENCE_REQUIRED_FIELDS`",
        "the collection planner dry-run JSON includes the checker-backed `evidence_contract` map for selected required kinds",
        "provider-bake artifacts prove the config-backed reserve lifecycle scheduler canary ran recently enough before bake completion",
        "reserve-movement artifacts prove live chain submission coverage, submitted transaction-hash readback, automatic finality polling",
        "governance approval artifacts prove source-entry publication, downstream compliance application, consumer coverage",
        "The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier.",
    )
    missing_current: dict[str, list[str]] = {}

    for path in sorted(DOCS_SOURCE_DIR.glob("sorafs_reserve_rent_plan*.md")):
        normalized = re.sub(r"\s+", " ", read(path))
        missing = [phrase for phrase in required_current if phrase not in normalized]
        if missing:
            missing_current[str(path.relative_to(REPO_ROOT))] = missing

    assert missing_current == {}


def test_reserve_rent_canary_builder_is_checked_in() -> None:
    builder = read(SCRIPTS_DIR / "build_sorafs_reserve_rent_canary.py")
    builder_test = read(
        SCRIPTS_DIR / "tests" / "build_sorafs_reserve_rent_canary_test.py"
    )
    policy_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_reserve_rent_policy_config_canary.args.example"
    )
    bake_example = read(
        SCRIPTS_DIR
        / "examples"
        / "sorafs_reserve_rent_provider_bake_canary.args.example"
    )
    docs = read(SORAFS_RESERVE_RENT_PLAN)

    assert "CANARY_KINDS = tuple(KIND_BY_NAME)" in builder
    assert "TRUE_CLAIMS" in builder
    assert "FORCED_FALSE_FIELDS" in builder
    assert "REQUIRED_LIFECYCLE_ROUTES" in builder
    assert "REQUIRED_SIGNED_ROUTES" in builder
    assert "REQUIRED_METRICS" in builder
    assert "validate_evidence_payload(payload, validation_options(args))" in builder
    assert "write_payload_atomic" in builder
    assert "must not be a symlink" in builder
    assert "raw_transfer_included" in builder
    assert "raw_ledger_included" in builder
    assert "response_bodies_included" in builder
    assert "payloads_included" in builder
    assert "test_generated_canaries_pass_full_reserve_rent_gate" in builder_test
    assert "test_stale_scheduler_tick_fails_before_write" in builder_test
    assert "test_output_symlink_is_rejected" in builder_test
    assert "--kind\npolicy_config" in policy_example
    assert "--verified-claim\ngovernance_approved" in policy_example
    assert "--kind\nprovider_bake" in bake_example
    assert "--verified-claim\nscheduled_lifecycle_canary_passed" in bake_example
    assert "build_sorafs_reserve_rent_canary.py" in docs
    assert "payload-free SFM-6 reserve/rent canary builder" in docs


def test_unshipped_reserve_rent_live_control_plane_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/reserve/live-custody-submit",
        "/v1/sorafs/reserve/finality-poller",
        "/v1/sorafs/reserve/finality-service",
        "/v1/sorafs/reserve/credit-line-account-mutation",
        "/v1/sorafs/reserve/credit-line-mutator",
        "/v1/sorafs/reserve/provider-bake-service",
        "/v1/sorafs/reserve/provider-bake/live",
        "/v1/sorafs/reserve/governance-source-entries/apply",
        "/v1/sorafs/reserve/promotion",
    )
    unshipped_cli_subcommands = (
        "reserve-live-submit",
        "reserve-finality-poller",
        "reserve-finality-service",
        "reserve-credit-line-mutator",
        "reserve-account-mutator",
        "reserve-provider-bake-service",
        "reserve-provider-bake-live",
        "reserve-governance-apply",
        "reserve-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_sorafs_proto_plan_documents_only_active_fixture_generators() -> None:
    source = read(SORAFS_PROTO_PLAN)
    documented_bins = re.findall(r"--bin\s+([A-Za-z0-9_-]+)", source)
    documented_fixture_bins = [
        name
        for name in documented_bins
        if name.startswith("generate_")
        or name in {"provider_admission_fixtures", "sorafs_manifest_stub"}
    ]
    expected_fixture_bins = [
        "provider_admission_fixtures",
        "sorafs_manifest_stub",
        "generate_orderbook_fixtures",
        "generate_por_fixtures",
    ]
    existing_bins = {
        path.stem
        for crate in (REPO_ROOT / "crates").iterdir()
        for path in (crate / "src" / "bin").glob("*.rs")
        if (crate / "src" / "bin").is_dir()
    }

    assert documented_fixture_bins == expected_fixture_bins
    assert [name for name in documented_fixture_bins if name not in existing_bins] == []
    assert "Do not document retired generator names as required workflow" in source
    assert "not defining a separate\n`sora-proto` codec outside Norito" in source


def test_sorafs_proto_release_evidence_work_stays_open_in_docs() -> None:
    source = read(SORAFS_PROTO_PLAN)
    normalized = re.sub(r"\s+", " ", source)

    required_open = (
        "Remaining work is live release evidence and SDK distribution hygiene, not defining a separate `sora-proto` codec outside Norito.",
        "Every payload that crosses a SoraFS boundary must be encoded with Norito. JSON views are for operator readability and fixtures; they are not alternate wire formats.",
        "SDKs should call the reference validator or C ABI facade for schema checks rather than reimplementing Norito layout rules.",
        "Transport should preserve raw Norito bytes and may include decoded JSON only as commentary.",
        "Unknown versions, missing required fields, invalid signatures, and broken fixture cross-links must fail closed.",
        "Publish release bundles that include the refreshed `.to` fixtures, human-readable JSON commentary, validation outcomes, and digest manifests.",
        "Keep portal error-catalog links synchronized with `ValidationOutcomeV1` codes.",
        "Capture SDK smoke evidence that JavaScript/TypeScript, Python, Swift, Kotlin, Java, and C# consumers validate the same committed fixtures through shared validators or FFI bindings.",
        "Re-run fixture and reference-validator smoke tests whenever Norito payload layouts, signing domains, or governance payload variants change.",
    )
    missing = [phrase for phrase in required_open if phrase not in normalized]

    assert missing == []


def test_unshipped_sorafs_proto_release_surface_is_not_exposed() -> None:
    route_patterns = (
        "/v1/sorafs/proto",
        "/v1/sorafs/sora-proto",
        "/v1/sorafs/schema-registry",
        "/v1/sorafs/schema/service",
        "/v1/sorafs/wire-format/service",
        "/v1/sorafs/fixtures/release-bundle",
        "/v1/sorafs/fixtures/sdk-smoke",
        "/v1/sorafs/proto/release-bundle",
        "/v1/sorafs/proto/promotion",
    )
    unshipped_cli_subcommands = (
        "sora-proto",
        "sorafs-proto",
        "proto-schema-service",
        "schema-registry-service",
        "wire-format-service",
        "proto-release-bundle",
        "fixture-release-bundle",
        "fixture-bundle-publish",
        "sdk-smoke-publish",
        "proto-promote",
    )
    exposed: dict[str, list[str]] = {}

    for path in (TORII_SORAFS_API_RS, TORII_OPENAPI_RS):
        source = read(path)
        matched = [route for route in route_patterns if route in source]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    for path in (IROHA_CLI_SORAFS_RS, SORAFS_CLI_RS):
        source = read(path)
        matched = [
            subcommand
            for subcommand in unshipped_cli_subcommands
            if f'"{subcommand}"' in source or f"`{subcommand}`" in source
        ]
        if matched:
            exposed[str(path.relative_to(REPO_ROOT))] = matched

    assert exposed == {}


def test_pdp_proof_stream_remains_fail_closed_until_provider_protocol_ships() -> None:
    api = read(TORII_SORAFS_API_RS)
    openapi = read(TORII_OPENAPI_RS)

    precheck_start = api.index("if matches!(proof_kind, ProofStreamKind::Pdp)")
    precheck_end = api.index("let nonce = match decode_nonce", precheck_start)
    precheck = api[precheck_start:precheck_end]
    assert "return json_error(" in precheck
    assert "StatusCode::BAD_REQUEST" in precheck
    assert "unsupported proof_kind; expected `por` or `potr`" in precheck

    dispatch_start = api.index("ProofStreamKind::Pdp => json_error(")
    dispatch_end = api.index("\n    }\n}", dispatch_start)
    dispatch = api[dispatch_start:dispatch_end]
    assert "StatusCode::BAD_REQUEST" in dispatch
    assert "unsupported proof_kind; expected `por` or `potr`" in dispatch

    assert "`proof_kind=pdp` is reserved for future SF-13 work" in openapi
    assert "rejected as an unsupported proof kind" in openapi


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
