"""Checker and runner inventory helpers for the SoraFS rollout contract tests."""

from __future__ import annotations

import inspect
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
EXAMPLES_DIR = SCRIPTS_DIR / "examples"
PRODUCTION_READINESS_RUNNER = SCRIPTS_DIR / "run_sorafs_production_readiness.py"
CHECKERS = sorted(SCRIPTS_DIR.glob("check_sorafs_*rollout_evidence.py")) + [
    SCRIPTS_DIR / "check_sorafs_reference_sdk_release_evidence.py"
]
RUNNERS = sorted(SCRIPTS_DIR.glob("run_sorafs_*rollout_evidence.py")) + [
    SCRIPTS_DIR / "run_sorafs_reference_sdk_release_evidence.py"
]
COLLECTION_RUNNERS = [*RUNNERS, PRODUCTION_READINESS_RUNNER]


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
    return CHECKERS


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
    return set()


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
    return set()


def maximum_int_validation_checkers() -> set[str]:
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
        "check_sorafs_reputation_rollout_evidence.py",
        "check_sorafs_reserve_rent_rollout_evidence.py",
        "check_sorafs_transparency_rollout_evidence.py",
        "check_sorafs_production_readiness.py",
    }


def non_negative_int_arg_checkers() -> set[str]:
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
        "check_sorafs_production_readiness.py",
    }


def positive_int_arg_runners() -> set[str]:
    return {path.name for path in COLLECTION_RUNNERS}


def non_negative_int_arg_runners() -> set[str]:
    return {
        "run_sorafs_ai_prescreen_rollout_evidence.py",
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
        "run_sorafs_transparency_rollout_evidence.py",
        "run_sorafs_production_readiness.py",
    }


def false_validation_checkers() -> set[str]:
    return checker_names_without("check_sorafs_reputation_rollout_evidence.py")


def false_or_absent_validation_checkers() -> set[str]:
    return set()


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


def runner_collection_example(path: Path) -> Path:
    if path == PRODUCTION_READINESS_RUNNER:
        return EXAMPLES_DIR / "sorafs_production_readiness_collection.args.example"
    return runner_example_candidates(path)[0]


def runner_plan_example(path: Path) -> Path | None:
    if path == PRODUCTION_READINESS_RUNNER:
        return next(
            (
                candidate
                for candidate in (
                    EXAMPLES_DIR / "sorafs_production_readiness_collection.args.example",
                    EXAMPLES_DIR / "sorafs_production_readiness.args.example",
                )
                if candidate.is_file()
            ),
            None,
        )
    return runner_example(path)


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


def argfile_option_values(path: Path, option: str) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    values: list[str] = []
    prefix = f"{option}="
    spaced_prefix = f"{option} "
    index = 0
    while index < len(lines):
        line = lines[index]
        if line == option:
            assert index + 1 < len(lines), f"{path.name} has dangling {option}"
            values.append(lines[index + 1])
            index += 2
            continue
        if line.startswith(prefix):
            values.append(line.split("=", 1)[1])
        elif line.startswith(spaced_prefix):
            values.append(line.split(maxsplit=1)[1])
        index += 1
    return values


def comma_split_argfile_values(values: list[str]) -> list[str]:
    return [
        item.strip()
        for value in values
        for item in value.split(",")
        if item.strip()
    ]


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


def runner_inventory_constant_fields(
    module: object,
    constant_name: str,
    failures: list[str],
) -> set[str] | None:
    value = getattr(module, constant_name, None)
    if not isinstance(value, frozenset):
        failures.append(f"{constant_name}:shape")
        return None
    malformed = [
        field
        for field in value
        if (
            not isinstance(field, str)
            or not field
            or field != field.strip()
            or any(ord(character) < 32 for character in field)
        )
    ]
    if malformed:
        failures.append(f"{constant_name}:field")
        return None
    return set(value)
