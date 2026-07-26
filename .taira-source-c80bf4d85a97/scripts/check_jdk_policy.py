#!/usr/bin/env python3
"""Validate the JDK posture for Android/SDK Norito workflows."""

from __future__ import annotations

import argparse
import json
import shlex
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, List, Optional


DEFAULT_EXPECTED_MAJOR = 21


@dataclass(frozen=True)
class ToolVersion:
    """Parsed version information for a Java tool."""

    name: str
    command: str
    raw: str
    major: int


def parse_version(stdout: str, stderr: str) -> int:
    """Extract the major version from java/javac output."""
    payload = (stdout + "\n" + stderr).strip()
    if not payload:
        raise RuntimeError("empty version output")
    # java -version prints the version on stderr; javac prints to stdout.
    for line in payload.splitlines():
        if "version" in line:
            parts = line.split()
        else:
            parts = line.split()
        for part in parts:
            if part.startswith('"') and part.endswith('"'):
                part = part.strip('"')
            if part[0].isdigit():
                version_bits = part.split(".")
                major_raw = version_bits[0]
                if major_raw == "1" and len(version_bits) > 1:
                    major_raw = version_bits[1]
                try:
                    return int(major_raw)
                except ValueError:
                    continue
    raise RuntimeError(f"unable to parse version from output: {payload}")


def capture_tool(name: str, command: str) -> ToolVersion:
    """Run a java/javac command and capture its version."""
    args = shlex.split(command)
    if not args:
        raise RuntimeError(f"{name} command is empty")
    proc = subprocess.run(
        args + ["-version"],
        check=False,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"{name} exited with {proc.returncode}: {proc.stderr.strip()}")
    major = parse_version(proc.stdout, proc.stderr)
    return ToolVersion(name=name, command=" ".join(args), raw=(proc.stdout + proc.stderr).strip(), major=major)


def isoformat_utc(now: datetime) -> str:
    return now.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def build_json_report(
    java: ToolVersion,
    javac: ToolVersion,
    *,
    checked_at: datetime,
    expected_major: int,
    allow_newer: bool,
    status: str,
    errors: List[str],
) -> dict:
    return {
        "checked_at": isoformat_utc(checked_at),
        "expected_major": expected_major,
        "allow_newer": allow_newer,
        "status": status,
        "java": {
            "command": java.command,
            "raw": java.raw,
            "major": java.major,
        },
        "javac": {
            "command": javac.command,
            "raw": javac.raw,
            "major": javac.major,
        },
        "errors": errors,
    }


def render_markdown(
    java: ToolVersion,
    javac: ToolVersion,
    *,
    checked_at: datetime,
    expected_major: int,
    allow_newer: bool,
    status: str,
    errors: List[str],
) -> str:
    lines = [
        "# Android JDK Policy Check",
        "",
        f"- Checked at: {isoformat_utc(checked_at)}",
        f"- Status: {status}",
        f"- Expected major: {expected_major}",
        f"- Allow newer: {allow_newer}",
        "",
        "| Tool | Command | Major | Raw Output |",
        "| --- | --- | ---: | --- |",
        f"| java | `{java.command}` | {java.major} | `{java.raw}` |",
        f"| javac | `{javac.command}` | {javac.major} | `{javac.raw}` |",
    ]
    if errors:
        lines.append("")
        lines.append("## Errors")
        lines.extend(f"- {err}" for err in errors)
    return "\n".join(lines) + "\n"


def check_policy(java: ToolVersion, javac: ToolVersion, *, expected_major: int, allow_newer: bool) -> List[str]:
    errors: List[str] = []
    for tool in (java, javac):
        if tool.major == expected_major:
            continue
        if allow_newer and tool.major > expected_major:
            continue
        errors.append(f"{tool.name} reports major {tool.major}, expected {expected_major}")
    return errors


def parse_args(argv: Optional[Iterable[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--java-cmd",
        default="java",
        help="Command to invoke for java (default: java)",
    )
    parser.add_argument(
        "--javac-cmd",
        default="javac",
        help="Command to invoke for javac (default: javac)",
    )
    parser.add_argument(
        "--expected-major",
        type=int,
        default=DEFAULT_EXPECTED_MAJOR,
        help="Required major JDK version (default: 21).",
    )
    parser.add_argument(
        "--allow-newer",
        action="store_true",
        help="Allow JDK versions newer than expected (still fails on older versions).",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Optional path to write a JSON report.",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        help="Optional path to write a Markdown summary.",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Iterable[str]] = None) -> int:
    args = parse_args(argv)
    now = datetime.now(timezone.utc)
    try:
        java = capture_tool("java", args.java_cmd)
        javac = capture_tool("javac", args.javac_cmd)
        errors = check_policy(
            java,
            javac,
            expected_major=args.expected_major,
            allow_newer=args.allow_newer,
        )
        status = "ok" if not errors else "error"
    except RuntimeError as exc:
        status = "error"
        errors = [str(exc)]
        # Provide sentinel versions when capture fails so reports are still usable.
        java = ToolVersion("java", args.java_cmd, "", -1)
        javac = ToolVersion("javac", args.javac_cmd, "", -1)

    print(
        "[jdk-policy] java=%s javac=%s expected=%s allow_newer=%s status=%s"
        % (java.major, javac.major, args.expected_major, args.allow_newer, status)
    )
    if errors:
        for err in errors:
            print(f"[jdk-policy] error: {err}")

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        payload = build_json_report(
            java,
            javac,
            checked_at=now,
            expected_major=args.expected_major,
            allow_newer=args.allow_newer,
            status=status,
            errors=errors,
        )
        args.json_out.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")

    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(
            render_markdown(
                java,
                javac,
                checked_at=now,
                expected_major=args.expected_major,
                allow_newer=args.allow_newer,
                status=status,
                errors=errors,
            ),
            encoding="utf-8",
        )

    return 0 if status == "ok" else 1


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
