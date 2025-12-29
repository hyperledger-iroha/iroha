from pathlib import Path

import pytest

from scripts.telemetry.generate_and7_quiz_summary import (
    load_responses,
    render_markdown,
    summarize_responses,
)


def write_csv(path: Path, rows: list[tuple[str, str, str, str]]) -> None:
    lines = ["participant,email,score,submitted_at_utc"]
    for row in rows:
        lines.append(",".join(row))
    path.write_text("\n".join(lines), encoding="utf-8")


def test_load_and_summarize(tmp_path: Path) -> None:
    csv_path = tmp_path / "2026-02.csv"
    write_csv(
        csv_path,
        [
            ("Alice", "alice@example.com", "0.95", "2026-02-01T10:00:00Z"),
            ("Bob", "bob@example.com", "0.75", "2026-02-01T11:00:00Z"),
        ],
    )
    responses = load_responses(tmp_path)
    assert len(responses) == 2
    assert responses[0].participant == "Alice"
    assert pytest.approx(responses[0].score, rel=1e-6) == 0.95

    summary = summarize_responses(responses, pass_threshold=0.9)
    assert summary["total_participants"] == 2
    assert summary["pass_count"] == 1
    assert summary["fail_count"] == 1
    assert summary["per_file"]["2026-02.csv"]["count"] == 2
    assert summary["per_file"]["2026-02.csv"]["pass_count"] == 1


def test_load_with_score_scale(tmp_path: Path) -> None:
    csv_path = tmp_path / "responses.csv"
    write_csv(
        csv_path,
        [
            ("Carol", "carol@example.com", "95", "2026-02-02T08:00:00Z"),
        ],
    )
    responses = load_responses(tmp_path, score_scale=100.0)
    assert len(responses) == 1
    assert pytest.approx(responses[0].score, rel=1e-6) == 0.95


def test_render_markdown_includes_table(tmp_path: Path) -> None:
    csv_path = tmp_path / "responses.csv"
    write_csv(
        csv_path,
        [
            ("Dana", "dana@example.com", "1.0", "2026-02-03T09:30:00Z"),
        ],
    )
    responses = load_responses(tmp_path)
    summary = summarize_responses(responses, pass_threshold=0.9)
    markdown = render_markdown(summary)
    assert "Total participants: **1**" in markdown
    assert "| Dana | dana@example.com | 100.0% |" in markdown


def test_question_summary_detects_question_columns(tmp_path: Path) -> None:
    csv_path = tmp_path / "2026-02.csv"
    csv_path.write_text(
        "\n".join(
            [
                "participant,email,score,submitted_at_utc,q1,q2",
                "Eve,eve@example.com,95,2026-02-04T10:00:00Z,correct,incorrect",
                "Frank,frank@example.com,85,2026-02-04T11:00:00Z,correct,correct",
            ]
        ),
        encoding="utf-8",
    )
    responses = load_responses(tmp_path, score_scale=100.0)
    summary = summarize_responses(responses, pass_threshold=0.9)
    questions = summary.get("questions")
    assert questions
    q1 = next(entry for entry in questions if entry["question"] == "q1")
    q2 = next(entry for entry in questions if entry["question"] == "q2")
    assert q1["correct"] == 2 and q1["incorrect"] == 0
    assert q2["correct"] == 1 and q2["incorrect"] == 1
