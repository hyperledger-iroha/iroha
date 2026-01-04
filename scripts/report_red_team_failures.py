#!/usr/bin/env python3
"""Archive red-team logs and raise a templated GitHub issue on failure."""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import shutil
import sys
import textwrap
import urllib.error
import urllib.request
from pathlib import Path
from typing import Iterable, List


def _copy_logs(logs: Iterable[Path], artifact_dir: Path) -> List[Path]:
    artifact_dir.mkdir(parents=True, exist_ok=True)
    copied: List[Path] = []
    for path in logs:
        if not path:
            continue
        if not path.exists():
            print(f"[report-red-team] log missing: {path}", file=sys.stderr)
            continue
        dest = artifact_dir / path.name
        try:
            shutil.copy2(path, dest)
            copied.append(dest)
            print(f"[report-red-team] archived {path} -> {dest}")
        except OSError as exc:
            print(f"[report-red-team] failed to copy {path}: {exc}", file=sys.stderr)
    return copied


def _default_due_date(days: int) -> str:
    now = _dt.datetime.utcnow()
    due = now + _dt.timedelta(days=days)
    return due.strftime("%Y-%m-%d")


def _issue_body(
    surface: str,
    mitigation: str,
    owner: str,
    due_date: str,
    run_url: str | None,
    copied_logs: Iterable[Path],
    artifact_dir: Path,
) -> str:
    bullet_logs = "\n".join(
        f"- `{log.name}`" for log in copied_logs
    ) or "- (no logs captured)"
    run_line = run_url or "(not provided)"
    return textwrap.dedent(
        f"""
        ## Summary
        - **Surface:** {surface}
        - **Mitigation:** {mitigation}
        - **Owner:** {owner}
        - **Due date:** {due_date}

        ## Workflow run
        - {run_line}

        ## Collected logs
        {bullet_logs}

        _Logs are stored under `{artifact_dir}` in the workflow artifacts._
        """
    ).strip()


def _create_issue(
    token: str,
    repo: str,
    title: str,
    body: str,
    labels: Iterable[str],
) -> None:
    url = f"https://api.github.com/repos/{repo}/issues"
    payload = {
        "title": title,
        "body": body,
        "labels": list(labels),
    }
    data = json.dumps(payload).encode("utf-8")
    headers = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "Content-Type": "application/json",
        "User-Agent": "iroha-red-team-reporter",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req) as resp:
            if 200 <= resp.status < 300:
                print("[report-red-team] issue created successfully")
            else:
                print(
                    f"[report-red-team] unexpected response status {resp.status}",
                    file=sys.stderr,
                )
    except urllib.error.HTTPError as exc:
        message = exc.read().decode("utf-8", errors="replace")
        print(
            f"[report-red-team] failed to create issue: {exc.status} {exc.reason}: {message}",
            file=sys.stderr,
        )
    except urllib.error.URLError as exc:
        print(f"[report-red-team] network error: {exc}", file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifact-dir",
        default="artifacts/red-team",
        type=Path,
        help="Directory where logs should be archived (default: artifacts/red-team)",
    )
    parser.add_argument(
        "--logs",
        nargs="*",
        type=Path,
        default=[],
        help="Log files to archive and reference in the issue body",
    )
    parser.add_argument(
        "--surface",
        required=True,
        help="Short description of the failing surface",
    )
    parser.add_argument(
        "--mitigation",
        required=True,
        help="Expected mitigation or next steps",
    )
    parser.add_argument(
        "--owner",
        default="Unassigned",
        help="Default owner or team mention for the follow-up",
    )
    parser.add_argument(
        "--due-date",
        default=None,
        help="Due date in YYYY-MM-DD format (default: now + 1 day)",
    )
    parser.add_argument(
        "--due-days",
        type=int,
        default=1,
        help="When --due-date is omitted, add this many days from now (default: 1)",
    )
    parser.add_argument(
        "--run-url",
        default=os.environ.get("GITHUB_SERVER_URL", "")
        and f"{os.environ.get('GITHUB_SERVER_URL')}/{os.environ.get('GITHUB_REPOSITORY')}/actions/runs/{os.environ.get('GITHUB_RUN_ID')}",
        help="URL for the failing workflow run",
    )
    parser.add_argument(
        "--repo",
        default=os.environ.get("GITHUB_REPOSITORY"),
        help="GitHub repository in owner/name form (default: env GITHUB_REPOSITORY)",
    )
    parser.add_argument(
        "--token-env",
        default="GITHUB_TOKEN",
        help="Environment variable that contains a GitHub token (default: GITHUB_TOKEN)",
    )
    parser.add_argument(
        "--labels",
        nargs="*",
        default=["nightly", "red-team"],
        help="Labels to apply to created issues",
    )
    parser.add_argument(
        "--issue-title",
        default=None,
        help="Optional explicit issue title (default: Surface + timestamp)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Archive logs but skip the GitHub issue call",
    )
    args = parser.parse_args()

    copied_logs = _copy_logs(args.logs, args.artifact_dir)
    due_date = args.due_date or _default_due_date(args.due_days)
    run_url = args.run_url or ""

    title = args.issue_title
    if not title:
        timestamp = _dt.datetime.utcnow().strftime("%Y-%m-%d %H:%M UTC")
        title = f"{args.surface} failure ({timestamp})"

    body = _issue_body(
        surface=args.surface,
        mitigation=args.mitigation,
        owner=args.owner,
        due_date=due_date,
        run_url=run_url,
        copied_logs=copied_logs,
        artifact_dir=args.artifact_dir,
    )

    token = os.environ.get(args.token_env, "").strip()
    if args.dry_run:
        print("[report-red-team] dry run requested; skipping issue creation")
        print(body)
        return 0
    if not token:
        print(
            f"[report-red-team] token env `{args.token_env}` missing; skipping issue creation",
            file=sys.stderr,
        )
        return 0
    if not args.repo:
        print("[report-red-team] repository not specified; skipping issue creation", file=sys.stderr)
        return 0

    _create_issue(token=token, repo=args.repo, title=title, body=body, labels=args.labels)
    return 0


if __name__ == "__main__":
    sys.exit(main())
