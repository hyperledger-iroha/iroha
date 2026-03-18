#!/usr/bin/env python3
"""
Generate attendance and score summaries for the AND7 telemetry readiness quiz.

Roadmap item AND7 requires the Android telemetry enablement program to capture
knowledge‑check evidence before governance sign‑off. This helper scans the
CSV exports stored under ``docs/source/sdk/android/readiness/forms/responses/``
and emits structured JSON plus an optional Markdown report so the evidence can
be attached to the enablement bundle.
"""

from __future__ import annotations

import argparse
import csv
import json
from dataclasses import dataclass
from pathlib import Path
from statistics import mean, median
from typing import Dict, Iterable, List, Sequence

REQUIRED_COLUMNS = ("participant", "email", "score", "submitted_at_utc")
DEFAULT_QUESTION_PREFIXES = ("q", "question")
CORRECT_TOKENS = {"correct", "true", "1", "yes", "pass"}
INCORRECT_TOKENS = {"incorrect", "false", "0", "no", "fail"}


@dataclass(frozen=True)
class QuizResponse:
    """Normalized quiz response entry."""

    participant: str
    email: str
    score: float
    submitted_at_utc: str
    source: Path
    answers: Dict[str, str]


def _discover_csv_files(responses_dir: Path, glob: str) -> List[Path]:
    if not responses_dir.is_dir():
        raise FileNotFoundError(f"responses directory {responses_dir} does not exist")
    return sorted(responses_dir.glob(glob))


def _determine_question_columns(
    fieldnames: Sequence[str] | None,
    prefixes: Sequence[str],
    explicit: Sequence[str],
) -> List[str]:
    columns: List[str] = []
    normalized_prefixes = tuple(prefix.lower() for prefix in prefixes if prefix)
    explicit_set = {name for name in explicit if name}
    for name in explicit_set:
        if name not in columns:
            columns.append(name)
    if not fieldnames:
        return columns
    for name in fieldnames:
        if name in REQUIRED_COLUMNS:
            continue
        lower_name = name.lower()
        if any(lower_name.startswith(prefix) for prefix in normalized_prefixes):
            if name not in columns:
                columns.append(name)
    return columns


def _parse_row(
    row: dict,
    source: Path,
    score_scale: float,
    question_columns: Sequence[str],
) -> QuizResponse:
    missing = [column for column in REQUIRED_COLUMNS if not row.get(column)]
    if missing:
        raise ValueError(
            f"{source} is missing required column(s) {', '.join(missing)} "
            "for at least one row"
        )
    try:
        raw_score = float(row["score"])
    except ValueError as exc:
        raise ValueError(f"{source} has a non-numeric score: {row['score']}") from exc
    if score_scale <= 0:
        raise ValueError("score scale must be positive")
    normalized = raw_score / score_scale
    normalized = max(0.0, min(1.0, normalized))
    answers: Dict[str, str] = {}
    for column in question_columns:
        value = row.get(column)
        if value is None:
            answers[column] = ""
            continue
        stripped = value.strip()
        answers[column] = stripped
    return QuizResponse(
        participant=row["participant"].strip(),
        email=row["email"].strip(),
        score=normalized,
        submitted_at_utc=row["submitted_at_utc"].strip(),
        source=source,
        answers=answers,
    )


def _classify_answer(value: str) -> str:
    normalized = value.strip().lower()
    if normalized in CORRECT_TOKENS:
        return "correct"
    if normalized in INCORRECT_TOKENS:
        return "incorrect"
    return "unknown"


def _summarize_questions(responses: Sequence[QuizResponse]) -> List[dict]:
    stats: Dict[str, Dict[str, float | int]] = {}
    for resp in responses:
        for question, answer in resp.answers.items():
            entry = stats.setdefault(
                question,
                {"total": 0, "correct": 0, "incorrect": 0, "unknown": 0},
            )
            entry["total"] += 1
            classification = _classify_answer(answer)
            entry[classification] += 1
    question_rows: List[dict] = []
    for question, entry in sorted(stats.items()):
        total = entry["total"] or 0
        accuracy = (entry["correct"] / total) if total else None
        question_rows.append(
            {
                "question": question,
                "total": total,
                "correct": entry["correct"],
                "incorrect": entry["incorrect"],
                "unknown": entry["unknown"],
                "accuracy": accuracy,
            }
        )
    return question_rows


def load_responses(
    responses_dir: Path,
    glob: str = "*.csv",
    score_scale: float = 1.0,
    *,
    question_columns: Sequence[str] | None = None,
    question_prefixes: Sequence[str] | None = None,
) -> List[QuizResponse]:
    """Load quiz responses from CSV exports."""

    entries: List[QuizResponse] = []
    explicit_columns: Sequence[str] = question_columns or ()
    prefixes: Sequence[str] = question_prefixes or DEFAULT_QUESTION_PREFIXES
    for csv_path in _discover_csv_files(responses_dir, glob):
        if csv_path.stat().st_size == 0:
            continue
        with csv_path.open(newline="") as handle:
            reader = csv.DictReader(handle)
            if reader.fieldnames is None:
                continue
            question_cols = _determine_question_columns(
                reader.fieldnames, prefixes, explicit_columns
            )
            for row in reader:
                # Skip blank rows created by trailing newline.
                if not any(row.values()):
                    continue
                entries.append(
                    _parse_row(row, csv_path, score_scale, question_cols)
                )
    return entries


def summarize_responses(
    responses: Sequence[QuizResponse], pass_threshold: float
) -> dict:
    """Produce aggregate statistics for the supplied responses."""

    total = len(responses)
    scores = [resp.score for resp in responses]
    pass_threshold = max(0.0, min(1.0, pass_threshold))
    passes = [resp for resp in responses if resp.score >= pass_threshold]
    failures = [resp for resp in responses if resp.score < pass_threshold]

    per_file: dict[str, dict[str, float | int]] = {}
    for resp in responses:
        key = resp.source.name
        per_file.setdefault(key, {"count": 0, "pass_count": 0})
        per_file[key]["count"] += 1
        if resp.score >= pass_threshold:
            per_file[key]["pass_count"] += 1

    summary = {
        "total_participants": total,
        "pass_threshold": pass_threshold,
        "pass_count": len(passes),
        "fail_count": len(failures),
        "average_score": mean(scores) if scores else None,
        "median_score": median(scores) if scores else None,
        "min_score": min(scores) if scores else None,
        "max_score": max(scores) if scores else None,
        "per_file": per_file,
        "participants": [
            {
                "participant": resp.participant,
                "email": resp.email,
                "score": resp.score,
                "submitted_at_utc": resp.submitted_at_utc,
                "source": str(resp.source),
            }
            for resp in responses
        ],
        "below_threshold": [
            {
                "participant": resp.participant,
                "email": resp.email,
                "score": resp.score,
                "submitted_at_utc": resp.submitted_at_utc,
                "source": str(resp.source),
            }
            for resp in failures
        ],
    }
    summary["questions"] = _summarize_questions(responses)
    return summary


def _format_percent(value: float | None) -> str:
    if value is None:
        return "n/a"
    return f"{value * 100:.1f}%"


def render_markdown(summary: dict) -> str:
    """Render a Markdown report for the quiz summary."""

    total = summary["total_participants"]
    pass_count = summary["pass_count"]
    threshold_pct = _format_percent(summary["pass_threshold"])
    pass_rate = (
        f"{(pass_count / total) * 100:.1f}%"
        if total
        else "n/a"
    )

    lines = [
        "# AND7 Telemetry Quiz Summary",
        "",
        f"- Total participants: **{total}**",
        f"- Pass threshold: **{threshold_pct}**",
        f"- Pass rate: **{pass_rate}** ({pass_count} / {total} participants)",
        f"- Average score: **{_format_percent(summary['average_score'])}**",
        f"- Median score: **{_format_percent(summary['median_score'])}**",
    ]
    if total == 0:
        lines.append("\n_No quiz responses were found in the provided directory._")
        return "\n".join(lines)

    lines.append("")
    lines.append("| Participant | Email | Score | Submitted (UTC) | Source |")
    lines.append("|-------------|-------|-------|-----------------|--------|")
    for participant in summary["participants"]:
        lines.append(
            "| {participant} | {email} | {score:.1%} | {submitted} | `{source}` |".format(
                participant=participant["participant"] or "n/a",
                email=participant["email"] or "n/a",
                score=participant["score"],
                submitted=participant["submitted_at_utc"] or "n/a",
                source=participant["source"],
            )
        )
    questions = summary.get("questions", [])
    if questions:
        lines.extend([
            "",
            "## Question Accuracy",
            "",
            "| Question | Correct | Incorrect | Unknown | Accuracy |",
            "|----------|---------|-----------|---------|----------|",
        ])
        for entry in questions:
            lines.append(
                "| {question} | {correct} | {incorrect} | {unknown} | {accuracy} |".format(
                    question=entry["question"],
                    correct=entry["correct"],
                    incorrect=entry["incorrect"],
                    unknown=entry["unknown"],
                    accuracy=_format_percent(entry["accuracy"]),
                )
            )
    return "\n".join(lines)


def _write_text(path: Path | None, contents: str) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents, encoding="utf-8")


def _write_json(path: Path | None, payload: dict) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Summarize Android AND7 telemetry readiness quiz responses."
    )
    default_responses = (
        Path(__file__).resolve().parents[2]
        / "docs"
        / "source"
        / "sdk"
        / "android"
        / "readiness"
        / "forms"
        / "responses"
    )
    parser.add_argument(
        "--responses-dir",
        type=Path,
        default=default_responses,
        help=f"Directory containing quiz CSV exports (default: {default_responses})",
    )
    parser.add_argument(
        "--glob",
        default="*.csv",
        help="Glob for selecting CSV files inside the responses directory (default: *.csv)",
    )
    parser.add_argument(
        "--pass-threshold",
        type=float,
        default=0.9,
        help="Minimum normalized score (0-1) considered a passing grade (default: 0.9)",
    )
    parser.add_argument(
        "--score-scale",
        type=float,
        default=1.0,
        help=(
            "Scale of the values stored in the score column. "
            "Set to 100 when the CSV stores percentages instead of normalized scores."
        ),
    )
    parser.add_argument(
        "--output-json",
        type=Path,
        help="Optional path for writing a machine-readable JSON summary.",
    )
    parser.add_argument(
        "--output-md",
        type=Path,
        help="Optional path for writing the Markdown report.",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress printing the Markdown report to stdout.",
    )
    parser.add_argument(
        "--question-column",
        action="append",
        default=[],
        help=(
            "Column name containing per-question correctness markers. Use "
            "multiple times to collect additional columns."
        ),
    )
    parser.add_argument(
        "--question-prefix",
        action="append",
        default=[],
        help=(
            "Header prefix (case-insensitive) used to auto-detect question "
            "columns (default prefixes: 'q', 'question')."
        ),
    )
    parser.add_argument(
        "--disable-question-autodetect",
        action="store_true",
        help="Only use columns specified via --question-column.",
    )
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    parser = build_arg_parser()
    args = parser.parse_args(argv)
    prefixes: Sequence[str]
    if args.disable_question_autodetect:
        prefixes = tuple(args.question_prefix or [])
    else:
        prefixes = DEFAULT_QUESTION_PREFIXES + tuple(args.question_prefix or [])
    responses = load_responses(
        args.responses_dir,
        args.glob,
        args.score_scale,
        question_columns=args.question_column,
        question_prefixes=prefixes,
    )
    summary = summarize_responses(responses, args.pass_threshold)
    markdown = render_markdown(summary)
    _write_json(args.output_json, summary)
    _write_text(args.output_md, markdown)
    if not args.quiet:
        print(markdown)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
