#!/usr/bin/env python3
"""
Check Norito fixture manifest alignment across SDK copies.

This helper compares the canonical Norito-RPC manifest with the SDK-local
copies (Android/Python/Swift by default), reports any drift, and can emit both
JSON and Markdown summaries for governance evidence.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Mapping, MutableMapping, Optional, Sequence


DEFAULT_CANONICAL = Path("fixtures/norito_rpc/transaction_fixtures.manifest.json")
DEFAULT_TARGETS: Mapping[str, Path] = {
    "android": Path("java/iroha_android/src/test/resources/transaction_fixtures.manifest.json"),
    "python": Path("python/iroha_python/tests/fixtures/transaction_fixtures.manifest.json"),
    "swift": Path("IrohaSwift/Fixtures/transaction_fixtures.manifest.json"),
}


@dataclass(frozen=True)
class FixtureDigest:
    name: str
    chain: str
    authority: str
    payload_hash: str
    signed_hash: str
    encoded_len: int
    signed_len: int
    creation_time_ms: int
    time_to_live_ms: Optional[int]
    nonce: Optional[int]


@dataclass(frozen=True)
class ManifestSnapshot:
    path: Path
    fingerprint: str
    fixtures: Mapping[str, FixtureDigest]
    age_hours: float


@dataclass(frozen=True)
class FixtureMismatch:
    name: str
    differences: Mapping[str, str]


@dataclass(frozen=True)
class AlignmentResult:
    label: str
    snapshot: ManifestSnapshot
    missing: Sequence[str]
    extra: Sequence[str]
    mismatched: Sequence[FixtureMismatch]

    @property
    def ok(self) -> bool:
        return not (self.missing or self.extra or self.mismatched)


def _fingerprint_manifest(payload: MutableMapping[str, object]) -> str:
    normalized = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.blake2b(normalized, digest_size=16).hexdigest()


def _fixture_digest(entry: Mapping[str, object]) -> FixtureDigest:
    try:
        name = str(entry["name"])
        chain = str(entry["chain"])
        authority = str(entry["authority"])
        payload_hash = str(entry["payload_hash"])
        signed_hash = str(entry["signed_hash"])
        encoded_len = int(entry["encoded_len"])
        signed_len = int(entry["signed_len"])
        creation_time_ms = int(entry["creation_time_ms"])
        if "time_to_live_ms" not in entry or "nonce" not in entry:
            raise KeyError("missing time_to_live_ms/nonce")
        time_to_live_ms = entry.get("time_to_live_ms")
        nonce = entry.get("nonce")
    except (KeyError, TypeError, ValueError) as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] malformed fixture entry: {entry}") from exc
    if not isinstance(time_to_live_ms, (int, type(None))) or isinstance(time_to_live_ms, bool):
        raise SystemExit(f"[error] malformed fixture entry: {entry}")
    if not isinstance(nonce, (int, type(None))) or isinstance(nonce, bool):
        raise SystemExit(f"[error] malformed fixture entry: {entry}")
    return FixtureDigest(
        name=name,
        chain=chain,
        authority=authority,
        payload_hash=payload_hash,
        signed_hash=signed_hash,
        encoded_len=encoded_len,
        signed_len=signed_len,
        creation_time_ms=creation_time_ms,
        time_to_live_ms=time_to_live_ms,
        nonce=nonce,
    )


def load_manifest(path: Path) -> ManifestSnapshot:
    if not path.exists():
        raise SystemExit(f"[error] manifest not found: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        raise SystemExit(f"[error] failed to parse JSON from {path}: {exc}") from exc
    fixtures_raw = payload.get("fixtures")
    if not isinstance(fixtures_raw, list):
        raise SystemExit(f"[error] manifest {path} missing fixtures array")
    fixtures: Dict[str, FixtureDigest] = {}
    for entry in fixtures_raw:
        if not isinstance(entry, dict):
            raise SystemExit(f"[error] fixture entry in {path} was not an object: {entry!r}")
        digest = _fixture_digest(entry)
        fixtures[digest.name] = digest
    fingerprint = _fingerprint_manifest(payload)
    age_hours = (dt.datetime.now(dt.timezone.utc) - _stat_mtime(path)).total_seconds() / 3600.0
    return ManifestSnapshot(path=path, fingerprint=fingerprint, fixtures=fixtures, age_hours=age_hours)


def _stat_mtime(path: Path) -> dt.datetime:
    stat = path.stat()
    return dt.datetime.fromtimestamp(stat.st_mtime, tz=dt.timezone.utc)


def compare_manifests(
    label: str, canonical: ManifestSnapshot, target: ManifestSnapshot
) -> AlignmentResult:
    missing = sorted(set(canonical.fixtures) - set(target.fixtures))
    extra = sorted(set(target.fixtures) - set(canonical.fixtures))
    mismatched: List[FixtureMismatch] = []
    for name, expected in canonical.fixtures.items():
        if name not in target.fixtures:
            continue
        found = target.fixtures[name]
        differences: Dict[str, str] = {}
        if expected.payload_hash != found.payload_hash:
            differences["payload_hash"] = f"{found.payload_hash} != {expected.payload_hash}"
        if expected.signed_hash != found.signed_hash:
            differences["signed_hash"] = f"{found.signed_hash} != {expected.signed_hash}"
        if expected.encoded_len != found.encoded_len:
            differences["encoded_len"] = f"{found.encoded_len} != {expected.encoded_len}"
        if expected.signed_len != found.signed_len:
            differences["signed_len"] = f"{found.signed_len} != {expected.signed_len}"
        if expected.creation_time_ms != found.creation_time_ms:
            differences["creation_time_ms"] = (
                f"{found.creation_time_ms} != {expected.creation_time_ms}"
            )
        if expected.chain != found.chain:
            differences["chain"] = f"{found.chain} != {expected.chain}"
        if expected.authority != found.authority:
            differences["authority"] = f"{found.authority} != {expected.authority}"
        if expected.time_to_live_ms != found.time_to_live_ms:
            differences["time_to_live_ms"] = (
                f"{found.time_to_live_ms} != {expected.time_to_live_ms}"
            )
        if expected.nonce != found.nonce:
            differences["nonce"] = f"{found.nonce} != {expected.nonce}"
        if differences:
            mismatched.append(FixtureMismatch(name=name, differences=differences))
    return AlignmentResult(label=label, snapshot=target, missing=missing, extra=extra, mismatched=sorted(mismatched, key=lambda m: m.name))


def build_alignment_report(
    canonical: ManifestSnapshot, targets: Mapping[str, ManifestSnapshot]
) -> Mapping[str, object]:
    results = [compare_manifests(label, canonical, snapshot) for label, snapshot in targets.items()]
    return {
        "canonical": {
            "path": str(canonical.path),
            "fingerprint": canonical.fingerprint,
            "fixtures": len(canonical.fixtures),
            "age_hours": round(canonical.age_hours, 2),
        },
        "targets": [
            {
                "label": result.label,
                "path": str(result.snapshot.path),
                "fingerprint": result.snapshot.fingerprint,
                "fixtures": len(result.snapshot.fixtures),
                "age_hours": round(result.snapshot.age_hours, 2),
                "missing": result.missing,
                "extra": result.extra,
                "mismatched": [
                    {"name": mismatch.name, "differences": mismatch.differences}
                    for mismatch in result.mismatched
                ],
                "status": "ok" if result.ok else "drift",
            }
            for result in results
        ],
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    }


def render_markdown(report: Mapping[str, object]) -> str:
    canonical = report.get("canonical", {})
    targets = report.get("targets", [])
    lines = [
        "# Norito fixture alignment",
        "",
        f"- Canonical: `{canonical.get('path', '')}` (fixtures: {canonical.get('fixtures', '?')}, "
        f"fingerprint: `{canonical.get('fingerprint', '')}`, "
        f"age_hours: {canonical.get('age_hours', '?')})",
        "",
        "| SDK | Status | Missing | Extra | Mismatched | Age (h) | Fingerprint |",
        "|-----|--------|---------|-------|------------|---------|-------------|",
    ]
    for target in targets:
        status = target.get("status", "?")
        missing = target.get("missing", [])
        extra = target.get("extra", [])
        mismatched = target.get("mismatched", [])
        fingerprint = target.get("fingerprint", "")
        age = target.get("age_hours", "?")
        lines.append(
            f"| {target.get('label','?')} | {status} | "
            f"{_fmt_list(missing)} | {_fmt_list(extra)} | {_fmt_mismatch(mismatched)} | {age} | `{fingerprint}` |"
        )
    return "\n".join(lines) + "\n"


def _fmt_list(entries: Iterable[str]) -> str:
    items = list(entries)
    if not items:
        return "—"
    return "<br>".join(items)


def _fmt_mismatch(entries: Iterable[Mapping[str, object]]) -> str:
    rows: List[str] = []
    for entry in entries:
        name = entry.get("name", "?")
        diffs = entry.get("differences", {})
        if isinstance(diffs, dict):
            joined = "; ".join(f"{k}: {v}" for k, v in diffs.items())
        else:
            joined = str(diffs)
        rows.append(f"{name} ({joined})")
    return "<br>".join(rows) if rows else "—"


def parse_target_overrides(raw_targets: Sequence[str]) -> Mapping[str, Path]:
    targets: Dict[str, Path] = {}
    for raw in raw_targets:
        if "=" not in raw:
            raise SystemExit(f"[error] expected --target entries in label=path form, got {raw!r}")
        label, path = raw.split("=", 1)
        label = label.strip()
        resolved = Path(path.strip())
        if not label:
            raise SystemExit("[error] target label may not be empty")
        targets[label] = resolved
    return targets


def build_targets(defaults: Mapping[str, Path], overrides: Mapping[str, Path], drop_defaults: bool) -> Mapping[str, Path]:
    if drop_defaults:
        return overrides
    merged = dict(defaults)
    merged.update(overrides)
    return merged


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Verify Norito fixture manifest alignment across SDK copies.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument(
        "--canonical",
        type=Path,
        default=DEFAULT_CANONICAL,
        help="Path to the canonical Norito fixture manifest.",
    )
    parser.add_argument(
        "--target",
        action="append",
        default=[],
        help="SDK manifest in label=path form (e.g., android=java/.../transaction_fixtures.manifest.json).",
    )
    parser.add_argument(
        "--no-default-targets",
        action="store_true",
        help="Ignore built-in target paths and rely solely on --target values.",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        help="Write a machine-readable JSON report to the given path.",
    )
    parser.add_argument(
        "--markdown-out",
        type=Path,
        help="Write a Markdown summary to the given path instead of stdout.",
    )
    parser.add_argument(
        "--allow-drift",
        action="store_true",
        help="Exit with status 0 even when drift is detected.",
    )
    args = parser.parse_args(argv)

    override_targets = parse_target_overrides(args.target)
    target_paths = build_targets(DEFAULT_TARGETS, override_targets, args.no_default_targets)
    canonical_snapshot = load_manifest(args.canonical)
    target_snapshots = {label: load_manifest(path) for label, path in target_paths.items()}
    report = build_alignment_report(canonical_snapshot, target_snapshots)
    markdown = render_markdown(report)

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    if args.markdown_out:
        args.markdown_out.parent.mkdir(parents=True, exist_ok=True)
        args.markdown_out.write_text(markdown, encoding="utf-8")
    if not args.json_out and not args.markdown_out:
        sys.stdout.write(markdown)

    has_drift = any(target.get("status") != "ok" for target in report.get("targets", []))
    return 0 if (args.allow_drift or not has_drift) else 1


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
