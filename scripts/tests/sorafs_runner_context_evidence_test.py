"""Schema-closed context-evidence plan tests for SoraFS runners."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import sys


SCRIPTS_DIR = Path(__file__).resolve().parents[1]
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from sorafs_runner_preflight import validate_runner_context_evidence_plan  # noqa: E402


@dataclass(frozen=True)
class Step:
    """Minimal command-plan step for context-evidence validation."""

    label: str
    artifact: Path | None
    command: list[str]


@dataclass(frozen=True)
class Kind:
    """Minimal evidence-kind contract for context-evidence validation."""

    schema: str


def test_validate_runner_context_evidence_plan_accepts_schema_closed_plan(
    tmp_path: Path,
) -> None:
    command = [sys.executable, "-c", "pass"]
    artifact = tmp_path / "summary.json"
    plan = [Step("gate", artifact, command)]
    rendered = {
        "schema": "example.context.plan.v1",
        "verifier_summary_schema": "example.context.summary.v1",
        "deployment_context": {
            "deployment_id": "deployment-staging-a",
            "environment": "staging",
            "deployment_context_reviewed": True,
        },
        "external_evidence": {"publication": "/reviewed/publication.json"},
        "evidence_contract": {
            "publication": {
                "schema": "example.publication.v1",
                "required_payload_fields": ["schema", "deployment_id"],
            }
        },
        "steps": [
            {
                "label": "gate",
                "artifact": str(artifact),
                "command": command,
            }
        ],
    }

    assert (
        validate_runner_context_evidence_plan(
            rendered,
            plan,
            diagnostic_prefix="example context rollout runner plan",
            plan_schema="example.context.plan.v1",
            plan_fields=frozenset(rendered),
            summary_schema="example.context.summary.v1",
            deployment_context=rendered["deployment_context"],
            deployment_context_fields=frozenset(
                {"deployment_id", "environment", "deployment_context_reviewed"}
            ),
            deployment_context_errors=(),
            known_kinds={"publication": Kind("example.publication.v1")},
            evidence_contract=rendered["evidence_contract"],
            evidence_required_fields={"publication": ("schema", "deployment_id")},
            external_evidence=rendered["external_evidence"],
            external_evidence_fields=frozenset({"publication"}),
        )
        == []
    )
