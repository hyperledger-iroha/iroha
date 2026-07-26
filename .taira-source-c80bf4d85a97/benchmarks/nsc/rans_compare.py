#!/usr/bin/env python3
"""Harness for validating NSC rANS tables against external encoders and collecting RD metrics."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, List, Mapping, Optional, Sequence, Union

try:  # Python 3.11+
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - fallback for older toolchains
    import tomli as tomllib  # type: ignore[no-redef]


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent.parent
DEFAULT_TABLES = REPO_ROOT / "artifacts" / "nsc" / "rans_tables.toml"
DEFAULT_OUTPUT_ROOT = REPO_ROOT / "artifacts" / "nsc" / "rans_compare"
DEFAULT_CLIP_DIR = REPO_ROOT / "benchmarks" / "nsc" / "clips"
DEFAULT_PRESET = REPO_ROOT / "benchmarks" / "nsc" / "rans_compare_presets.toml"
DEFAULT_CLIP_MANIFEST = REPO_ROOT / "benchmarks" / "nsc" / "reference_clips.json"


@dataclass(frozen=True)
class RansGroup:
    group_size: int
    precision_bits: int
    frequencies: List[int]
    cumulative: List[int]


@dataclass(frozen=True)
class RansTables:
    path: Path
    version: int
    generated_at: Optional[str]
    generator_commit: Optional[str]
    checksum_sha256: Optional[Sequence[int]]
    seed: Optional[int]
    bundle_width: Optional[int]
    groups: List[RansGroup]


@dataclass(frozen=True)
class ClipEntry:
    clip_set: str
    clip_id: str
    path: Path
    frames: int
    fps: str
    sha256: Optional[str]


@dataclass(frozen=True)
class ClipCheck:
    clip_set: str
    clip_id: str
    path: Path
    exists: bool
    bytes: int
    sha256_expected: Optional[str]
    sha256_actual: Optional[str]

    @property
    def ok(self) -> bool:
        return self.exists and (self.sha256_expected is None or self.sha256_expected == self.sha256_actual)


@dataclass(frozen=True)
class RunnerSpec:
    label: str
    command: Union[str, Sequence[str]]


@dataclass(frozen=True)
class RunnerResult:
    label: str
    command: Union[str, Sequence[str]]
    return_code: int
    duration_seconds: float
    stdout_path: Path
    stderr_path: Path
    skipped: bool = False


@dataclass(frozen=True)
class NoritoResult:
    command: Optional[Union[str, Sequence[str]]]
    json_path: Path
    duration_seconds: Optional[float]
    payload: Mapping[str, object]
    skipped: bool = False
    clip_id: Optional[str] = None


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert SignedRansTablesV1 artefacts to CSV and run reference encoders/decoders with RD metrics."
    )
    parser.add_argument("--norito", action="store_true", help="Run Norito entropy bench once.")
    parser.add_argument("--norito-per-clip", action="store_true", help="Run Norito entropy bench per clip.")
    parser.add_argument("--norito-json", type=Path, help="Precomputed Norito entropy JSON.")
    parser.add_argument("--norito-json-out", type=Path, help="Where to write Norito JSON (default output-dir/norito_entropy.json).")
    parser.add_argument("--norito-frames", type=int, default=3, help="Frames per segment for Norito bench.")
    parser.add_argument("--norito-segments", type=int, default=2, help="Segments for Norito bench.")
    parser.add_argument("--norito-width", type=int, default=3, help="Bundle width for Norito bench.")
    parser.add_argument("--norito-y4m-out", type=Path, help="Optional Y4M output for Norito bench.")
    parser.add_argument("--norito-y4m-in", type=Path, help="Y4M input to drive Norito bench.")
    parser.add_argument("--norito-chunk-dir", type=Path, help="Directory for Norito chunk outputs.")
    parser.add_argument("--vmaf", action="store_true", help="Compute VMAF (requires ffmpeg+libvmaf).")
    parser.add_argument("--psnr-mode", choices=["y", "yuv"], default="y", help="PSNR mode: luma-only (y) or full YUV (yuv).")
    parser.add_argument("--tables", type=Path, default=DEFAULT_TABLES, help=f"SignedRansTablesV1 artefact (default {DEFAULT_TABLES}).")
    parser.add_argument("--output-dir", type=Path, help=f"Output directory (default {DEFAULT_OUTPUT_ROOT}/<timestamp>).")
    parser.add_argument("--resume", action="store_true", help="Reuse an existing output directory.")
    parser.add_argument("--clips-dir", type=Path, default=DEFAULT_CLIP_DIR, help=f"Directory with clips (default {DEFAULT_CLIP_DIR}).")
    parser.add_argument("--clip-manifest", type=Path, default=DEFAULT_CLIP_MANIFEST, help=f"Clip manifest (default {DEFAULT_CLIP_MANIFEST}).")
    parser.add_argument("--runner", action="append", default=[], metavar="label=command", help="Reference command with placeholders {csv},{tables},{clips},{output}.")
    parser.add_argument("--preset", action="append", default=[], help=f"Preset TOML files (default {DEFAULT_PRESET} if none supplied).")
    parser.add_argument("--skip-verify", action="store_true", help="Skip reference runner errors (best-effort).")
    parser.add_argument("--skip-table-verify", action="store_true", help="Skip `cargo xtask codec rans-tables --verify`.")
    parser.add_argument("--norito-args", action="append", default=[], help="Extra args to forward to Norito bench (repeatable).")
    parser.add_argument("--notes", action="append", default=[], help="Notes to include in report.json (repeatable).")
    parser.add_argument("--norito-quantizer", type=int, help="Override quantizer for Norito bench (forwards to --quantizer).")
    parser.add_argument("--enable-bundles", action="store_true", help="Set ENABLE_RANS_BUNDLES=1 when invoking Norito bench.")
    parser.add_argument("--dry-run", action="store_true", help="Print commands but do not execute them.")
    return parser.parse_args(argv)


def parse_runner_specs(args: Sequence[str]) -> List[RunnerSpec]:
    specs: List[RunnerSpec] = []
    for item in args:
        if "=" not in item:
            raise ValueError(f"invalid runner spec '{item}', expected label=command")
        label, command = item.split("=", 1)
        specs.append(RunnerSpec(label=label, command=command))
    return specs


def load_tables(path: Path) -> RansTables:
    if not path.exists():
        raise FileNotFoundError(f"rANS tables artifact not found: {path}")
    contents = path.read_text(encoding="utf-8")
    try:
        raw = tomllib.loads(contents)
    except tomllib.TOMLDecodeError:
        raw = json.loads(contents)
    payload = raw.get("payload") or raw
    body = payload.get("body") or payload
    groups = []
    for group in body.get("groups", []):
        groups.append(
            RansGroup(
                group_size=group["group_size"],
                precision_bits=group["precision_bits"],
                frequencies=group["frequencies"],
                cumulative=group["cumulative"],
            )
        )
    checksum = payload.get("checksum_sha256")
    if checksum is None and "checksum_sha256" in payload:
        checksum = payload["checksum_sha256"]
    return RansTables(
        path=path,
        version=payload.get("version", 0),
        generated_at=payload.get("generated_at"),
        generator_commit=payload.get("generator_commit"),
        checksum_sha256=checksum,
        seed=body.get("seed"),
        bundle_width=body.get("bundle_width") or payload.get("bundle_width"),
        groups=groups,
    )


def write_csv(tables: RansTables, csv_path: Path) -> None:
    import csv

    with csv_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(["group_size", "symbol_index", "frequency", "cumulative"])
        for group in tables.groups:
            for symbol_idx, freq in enumerate(group.frequencies):
                cumulative = group.cumulative[symbol_idx + 1] if symbol_idx + 1 < len(group.cumulative) else group.cumulative[-1]
                writer.writerow([group.group_size, symbol_idx, freq, cumulative])


def _hash_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(8192):
            hasher.update(chunk)
    return hasher.hexdigest()


def verify_clips(entries: Sequence[ClipEntry]) -> List[ClipCheck]:
    checks: List[ClipCheck] = []
    for entry in entries:
        exists = entry.path.exists()
        sha_actual = _hash_file(entry.path) if exists else None
        checks.append(
            ClipCheck(
                clip_set=entry.clip_set,
                clip_id=entry.clip_id,
                path=entry.path,
                exists=exists,
                bytes=entry.path.stat().st_size if exists else 0,
                sha256_expected=entry.sha256,
                sha256_actual=sha_actual,
            )
        )
    return checks


def _load_norito_payload(path: Path) -> Mapping[str, object]:
    if not path.exists():
        raise FileNotFoundError(f"Norito entropy JSON not found: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def run_vmaf(decoded: Path, reference: Path) -> Optional[float]:
    if shutil.which("ffmpeg") is None:
        print("[rans-compare] ffmpeg not found; skipping VMAF", file=sys.stderr)
        return None
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tmp:
        log_path = Path(tmp.name)
    cmd = [
        "ffmpeg",
        "-y",
        "-i",
        str(decoded),
        "-i",
        str(reference),
        "-lavfi",
        f"libvmaf=log_fmt=json:log_path={log_path}",
        "-f",
        "null",
        "-",
    ]
    try:
        subprocess.run(cmd, check=True, cwd=REPO_ROOT)
        data = json.loads(log_path.read_text(encoding="utf-8"))
        return float(data.get("pooled_metrics", {}).get("vmaf", {}).get("mean"))
    except Exception as exc:  # pragma: no cover - defensive
        print(f"[rans-compare] VMAF failed: {exc}", file=sys.stderr)
        return None
    finally:
        try:
            log_path.unlink()
        except OSError:
            pass


def run_psnr(decoded: Path, reference: Path, width: int, height: int, fps: float, mode: str = "y") -> Optional[float]:
    """Compute PSNR using ffmpeg; mode `y` is luma-only, `yuv` is full."""
    if shutil.which("ffmpeg") is None:
        print("[rans-compare] ffmpeg not found; skipping PSNR", file=sys.stderr)
        return None
    filter_arg = "psnr" if mode == "yuv" else "psnr=stats_file=-:stats_version=2:all=0"
    cmd = [
        "ffmpeg",
        "-v",
        "error",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "yuv420p",
        "-s",
        f"{width}x{height}",
        "-r",
        str(fps),
        "-i",
        str(decoded),
        "-i",
        str(reference),
        "-lavfi",
        filter_arg,
        "-f",
        "null",
        "-",
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        return None
    match = re.search(r"average:([0-9.]+)", proc.stderr + proc.stdout)
    return float(match.group(1)) if match else None


def run_ssim(decoded: Path, reference: Path, mode: str = "y") -> Optional[float]:
    """Compute SSIM using ffmpeg; mode `y` parses luma, `yuv` parses the combined score."""
    if shutil.which("ffmpeg") is None:
        print("[rans-compare] ffmpeg not found; skipping SSIM", file=sys.stderr)
        return None
    cmd = [
        "ffmpeg",
        "-v",
        "error",
        "-i",
        str(decoded),
        "-i",
        str(reference),
        "-lavfi",
        "ssim=stats_file=-",
        "-f",
        "null",
        "-",
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        return None
    text = proc.stderr + proc.stdout
    pattern = r"All:([0-9.]+)" if mode == "yuv" else r"Y:([0-9.]+)"
    match = re.search(pattern, text)
    return float(match.group(1)) if match else None


def run_norito_bench(
    json_out: Path,
    *,
    frames: int,
    segments: int,
    width: int,
    quantizer: Optional[int],
    y4m_out: Optional[Path],
    dry_run: bool = False,
    y4m_in: Optional[Path] = None,
    chunk_out: Optional[Path] = None,
    clip_id: Optional[str] = None,
    compute_vmaf: bool = False,
    psnr_mode: str = "y",
    norito_args: Optional[Sequence[str]] = None,
    enable_bundles: bool = False,
) -> NoritoResult:
    if frames <= 0 or segments <= 0 or width <= 0:
        raise ValueError("Norito bench parameters must be positive")
    json_out.parent.mkdir(parents=True, exist_ok=True)

    def attach_vmaf(payload: Mapping[str, object]) -> Mapping[str, object]:
        if not compute_vmaf or y4m_out is None or y4m_in is None:
            return payload
        score = run_vmaf(y4m_out, y4m_in)
        if score is None:
            return payload
        if isinstance(payload, Mapping):
            updated = dict(payload)
            updated["vmaf"] = score
            return updated
        return payload

    def attach_ssim(payload: Mapping[str, object]) -> Mapping[str, object]:
        if y4m_out is None or y4m_in is None:
            return payload
        score = run_ssim(y4m_out, y4m_in, mode=psnr_mode)
        if score is None:
            return payload
        if isinstance(payload, Mapping):
            updated = dict(payload)
            updated["ssim"] = score
            return updated
        return payload

    if json_out.exists():
        payload = attach_ssim(attach_vmaf(_load_norito_payload(json_out)))
        return NoritoResult(command=[], json_path=json_out, duration_seconds=0.0, payload=payload, skipped=True, clip_id=clip_id)

    if y4m_out:
        y4m_out.parent.mkdir(parents=True, exist_ok=True)

    cmd: List[str] = [
        "cargo",
        "xtask",
        "streaming-entropy-bench",
        "--frames",
        str(frames),
        "--segments",
        str(segments),
        "--width",
        str(width),
        "--json-out",
        str(json_out),
        "--psnr-mode",
        psnr_mode,
    ]
    if y4m_out:
        cmd.extend(["--y4m-out", str(y4m_out)])
    if y4m_in:
        cmd.extend(["--y4m-in", str(y4m_in)])
    if chunk_out:
        chunk_out.parent.mkdir(parents=True, exist_ok=True)
        cmd.extend(["--chunk-out", str(chunk_out)])
    if quantizer is not None:
        cmd.extend(["--quantizer", str(quantizer)])
    if norito_args:
        cmd.extend(norito_args)

    env = os.environ.copy()
    env.setdefault("NORITO_SKIP_BINDINGS_SYNC", "1")
    if enable_bundles:
        env["ENABLE_RANS_BUNDLES"] = env.get("ENABLE_RANS_BUNDLES", "1")

    if dry_run:
        return NoritoResult(command=cmd, json_path=json_out, duration_seconds=0.0, payload={"dry_run": True}, skipped=True, clip_id=clip_id)

    print(f"[rans-compare] running Norito entropy bench -> {json_out}")
    start = time.perf_counter()
    subprocess.run(cmd, check=True, cwd=REPO_ROOT, env=env)
    duration = time.perf_counter() - start
    payload = attach_ssim(attach_vmaf(_load_norito_payload(json_out)))
    return NoritoResult(command=cmd, json_path=json_out, duration_seconds=duration, payload=payload, skipped=False, clip_id=clip_id)


def parse_runner_presets(paths: Sequence[Path]) -> List[RunnerSpec]:
    specs: List[RunnerSpec] = []
    for path in paths:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        for entry in data.get("runner", []):
            specs.append(RunnerSpec(label=entry["label"], command=entry["command"]))
    return specs


def load_runner_presets(paths: Sequence[Path]) -> List[RunnerSpec]:
    resolved = [Path(p).resolve() for p in paths]
    return parse_runner_presets(resolved)


def substitute_placeholders(
    command: Union[str, Sequence[str]],
    *,
    csv_path: Path,
    tables_path: Path,
    clips_dir: Optional[Path],
    output_dir: Path,
) -> Union[str, List[str]]:
    replacements = {
        "{csv}": str(csv_path),
        "{tables}": str(tables_path),
        "{clips}": str(clips_dir) if clips_dir else "",
        "{output}": str(output_dir),
    }
    if isinstance(command, str):
        result = command
        for placeholder, value in replacements.items():
            result = result.replace(placeholder, value)
        return result
    substituted: List[str] = []
    for item in command:
        new_item = item
        for placeholder, value in replacements.items():
            new_item = new_item.replace(placeholder, value)
        substituted.append(new_item)
    return substituted


def run_runner(
    spec: RunnerSpec,
    *,
    env: Mapping[str, str],
    workdir: Path,
    log_dir: Path,
    dry_run: bool,
) -> RunnerResult:
    if dry_run:
        log_dir.mkdir(parents=True, exist_ok=True)
        marker = log_dir / f"{spec.label}.dry_run.log"
        marker.write_text("dry-run\n", encoding="utf-8")
        return RunnerResult(label=spec.label, command=spec.command, return_code=0, duration_seconds=0.0, stdout_path=marker, stderr_path=marker, skipped=True)
    log_dir.mkdir(parents=True, exist_ok=True)
    stdout_path = log_dir / f"{spec.label}.stdout.log"
    stderr_path = log_dir / f"{spec.label}.stderr.log"
    stdout_handle = stdout_path.open("w", encoding="utf-8")
    stderr_handle = stderr_path.open("w", encoding="utf-8")
    command: Union[str, Sequence[str]] = spec.command
    if isinstance(command, str):
        command = shlex.split(command)
    start = time.perf_counter()
    proc = subprocess.run(command, cwd=workdir, env=env, stdout=stdout_handle, stderr=stderr_handle)
    duration = time.perf_counter() - start
    stdout_handle.close()
    stderr_handle.close()
    return RunnerResult(label=spec.label, command=command, return_code=proc.returncode, duration_seconds=duration, stdout_path=stdout_path, stderr_path=stderr_path, skipped=False)


def load_clip_manifest(manifest: Path, clips_dir: Path) -> List[ClipEntry]:
    raw = json.loads(manifest.read_text(encoding="utf-8"))
    out: List[ClipEntry] = []
    for clip_set, entries in raw.get("clip_sets", {}).items():
        for entry in entries:
            out.append(
                ClipEntry(
                    clip_set=clip_set,
                    clip_id=entry["id"],
                    path=clips_dir / entry["path"],
                    frames=entry.get("frames", 0),
                    fps=entry.get("fps", "0:0"),
                    sha256=entry.get("sha256"),
                )
            )
    return out


def build_report(
    tables: RansTables,
    csv_path: Path,
    runners: Sequence[RunnerResult],
    output_dir: Path,
    notes: Sequence[str],
    norito: Optional[NoritoResult],
    norito_per_clip: Optional[Sequence[NoritoResult]],
    psnr_mode: str,
) -> Mapping[str, object]:
    def _extract_number(payload: Mapping[str, object] | NoritoResult | None, section: Optional[str], key: str) -> Optional[float]:
        if payload is None:
            return None
        if isinstance(payload, NoritoResult):
            payload = payload.payload
        if not isinstance(payload, Mapping):
            return None
        focus: Mapping[str, object] | object = payload
        if section:
            focus = payload.get(section)  # type: ignore[index]
        if isinstance(focus, Mapping):
            value = focus.get(key)
            if isinstance(value, (int, float)):
                return float(value)
        return None

    def _extract_metric(payload: Mapping[str, object] | NoritoResult | None, key: str) -> Optional[float]:
        direct = _extract_number(payload, None, key)
        if direct is not None:
            return direct
        if payload is None:
            return None
        if isinstance(payload, NoritoResult):
            payload = payload.payload
        if not isinstance(payload, Mapping):
            return None
        baseline = payload.get("baseline")
        if isinstance(baseline, Mapping):
            value = baseline.get(key)
            if isinstance(value, (int, float)):
                return float(value)
        return None

    def _summarize_per_clip(entries: Sequence[NoritoResult]) -> List[Mapping[str, object]]:
        summary: List[Mapping[str, object]] = []
        for entry in entries:
            summary.append(
                {
                    "clip_id": entry.clip_id,
                    "skipped": entry.skipped,
                    "psnr_y": _extract_metric(entry, "psnr_y"),
                    "psnr_yuv": _extract_number(entry, None, "psnr_yuv"),
                    "vmaf": _extract_metric(entry, "vmaf"),
                    "baseline_chunk_bytes": _extract_number(entry, "baseline", "chunk_bytes"),
                    "bundled_psnr_y": _extract_number(entry, "bundled", "psnr_y"),
                    "bundled_psnr_yuv": _extract_number(entry, "bundled", "psnr_yuv"),
                    "bundled_chunk_bytes": _extract_number(entry, "bundled", "chunk_bytes"),
                }
            )
        return summary

    def _summarize_norito(payload: NoritoResult | Mapping[str, object] | None) -> Optional[Mapping[str, object]]:
        if payload is None:
            return None
        skipped = payload.skipped if isinstance(payload, NoritoResult) else False
        if isinstance(payload, NoritoResult):
            payload = payload.payload
        if not isinstance(payload, Mapping):
            return None

        def section_value(section: str, key: str) -> Optional[float]:
            return _extract_number(payload, section, key)

        summary = {
            "skipped": skipped,
            "psnr_mode": psnr_mode,
            "quantizer": payload.get("quantizer"),
            "tiny_clip_preset": payload.get("tiny_clip_preset"),
            "target_bitrates_mbps": payload.get("target_bitrates_mbps") or payload.get("target_bitrate_mbps"),
            "baseline": {
                "chunk_bytes": section_value("baseline", "chunk_bytes"),
                "psnr_y": section_value("baseline", "psnr_y"),
                "psnr_yuv": section_value("baseline", "psnr_yuv"),
                "bitrate_mbps": section_value("baseline", "bitrate_mbps"),
                "encode_ms": section_value("baseline", "encode_ms"),
            },
            "bundled_available": bool(payload.get("bundled_available")),
            "bundled": {
                "chunk_bytes": section_value("bundled", "chunk_bytes"),
                "psnr_y": section_value("bundled", "psnr_y"),
                "psnr_yuv": section_value("bundled", "psnr_yuv"),
                "bitrate_mbps": section_value("bundled", "bitrate_mbps"),
                "encode_ms": section_value("bundled", "encode_ms"),
            },
            "ssim": _extract_metric(payload, "ssim"),
            "vmaf": _extract_metric(payload, "vmaf"),
        }
        return summary

    report: Mapping[str, object] = {}
    report["generated_at"] = datetime.now(timezone.utc).isoformat()
    report["output_dir"] = str(output_dir)
    report["csv_path"] = str(csv_path)
    report["tables"] = {
        "path": str(tables.path),
        "version": tables.version,
        "generated_at": tables.generated_at,
        "generator_commit": tables.generator_commit,
        "checksum_sha256": tables.checksum_sha256,
        "seed": tables.seed,
        "bundle_width": tables.bundle_width,
        "groups": len(tables.groups),
    }
    report["notes"] = list(notes)
    report["runner_count"] = len(runners)
    report["runners"] = [
        {
            "label": runner.label,
            "command": runner.command,
            "return_code": runner.return_code,
            "duration_seconds": runner.duration_seconds,
            "stdout_path": str(runner.stdout_path),
            "stderr_path": str(runner.stderr_path),
            "skipped": runner.skipped,
        }
        for runner in runners
    ]
    if norito is not None:
        report["norito"] = {
            "command": norito.command,
            "json_path": str(norito.json_path),
            "duration_seconds": norito.duration_seconds,
            "payload": norito.payload,
            "skipped": norito.skipped,
            "clip_id": norito.clip_id,
        }
        report["norito_psnr_y"] = _extract_metric(norito, "psnr_y")
        report["norito_vmaf"] = _extract_metric(norito, "vmaf")
        report["norito_ssim"] = _extract_metric(norito, "ssim")
        report["norito_psnr_mode"] = psnr_mode
        report["norito_bundled_psnr_y"] = _extract_number(norito, "bundled", "psnr_y")
        report["norito_bundled_psnr_yuv"] = _extract_number(norito, "bundled", "psnr_yuv")
        report["norito_bundled_chunk_bytes"] = _extract_number(norito, "bundled", "chunk_bytes")
        if isinstance(norito.payload, Mapping):
            report["norito_bitrate_targets"] = norito.payload.get("bitrate_targets")
        else:
            report["norito_bitrate_targets"] = None
        report["norito_summary"] = _summarize_norito(norito)
    else:
        report["norito"] = None
        report["norito_psnr_y"] = None
        report["norito_vmaf"] = None
        report["norito_ssim"] = None
        report["norito_psnr_mode"] = None
        report["norito_bundled_psnr_y"] = None
        report["norito_bundled_psnr_yuv"] = None
        report["norito_bundled_chunk_bytes"] = None
        report["norito_bitrate_targets"] = None
        report["norito_summary"] = None
    if norito_per_clip is not None:
        report["norito_per_clip"] = [
            {
                "clip_id": entry.clip_id,
                "command": entry.command,
                "json_path": str(entry.json_path),
                "duration_seconds": entry.duration_seconds,
                "payload": entry.payload,
                "skipped": entry.skipped,
            }
            for entry in norito_per_clip
        ]
        report["norito_per_clip_psnr_y"] = [_extract_metric(entry, "psnr_y") for entry in norito_per_clip]
        report["norito_per_clip_vmaf"] = [_extract_metric(entry, "vmaf") for entry in norito_per_clip]
        report["norito_per_clip_ssim"] = [_extract_metric(entry, "ssim") for entry in norito_per_clip]
        report["norito_per_clip_psnr_mode"] = psnr_mode
        report["norito_per_clip_summary"] = _summarize_per_clip(norito_per_clip)
    else:
        report["norito_per_clip"] = None
        report["norito_per_clip_psnr_y"] = None
        report["norito_per_clip_vmaf"] = None
        report["norito_per_clip_ssim"] = None
        report["norito_per_clip_psnr_mode"] = None
        report["norito_per_clip_summary"] = None
    return report


def write_readme(output_dir: Path, csv_path: Path) -> None:
    readme = output_dir / "README.md"
    contents = f"""# rANS Compare Artefacts

These files are generated via `benchmarks/nsc/rans_compare.py`. Reference runners can be
supplied with `--runner label="command"`. The script exposes the following placeholders
inside each command:

| Placeholder | Description |
|-------------|-------------|
| `{{csv}}`   | Absolute path to the generated CSV export. |
| `{{tables}}`| Path to the SignedRansTablesV1 artefact. |
| `{{clips}}` | Canonical clip directory (only when `--clips-dir` is provided). |
| `{{output}}`| Root directory for this comparison run. |

Commands execute inside the repository root with `RANS_TABLES_CSV`, `RANS_TABLES_PATH`,
and (optionally) `RANS_REFERENCE_CLIPS` exported for convenience.
"""
    readme.write_text(contents, encoding="utf-8")


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    output_dir = args.output_dir or (DEFAULT_OUTPUT_ROOT / datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
    output_dir = output_dir.resolve()
    if output_dir.exists():
        if not args.resume:
            raise SystemExit(f"output directory already exists: {output_dir}")
        print(f"[rans-compare] resuming existing output directory {output_dir}")
    else:
        output_dir.mkdir(parents=True)
    logs_dir = output_dir / "logs"
    logs_dir.mkdir(parents=True, exist_ok=True)

    tables = load_tables(args.tables.resolve())
    csv_path = output_dir / "rans_tables.csv"
    write_csv(tables, csv_path)
    if not args.skip_table_verify:
        verify_tables_cmd = ["cargo", "xtask", "codec", "rans-tables", "--verify", str(tables.path)]
        try:
            subprocess.run(verify_tables_cmd, check=True, cwd=REPO_ROOT)
        except subprocess.CalledProcessError as exc:
            raise SystemExit(f"table verification failed: {exc}") from exc
    else:
        print("[rans-compare] skipping SignedRansTablesV1 verification")

    clips_dir = args.clips_dir.resolve() if args.clips_dir else None
    clip_manifest = args.clip_manifest.resolve() if args.clip_manifest else None
    clip_checks: List[ClipCheck] = []
    if clips_dir and clip_manifest:
        clip_entries = load_clip_manifest(clip_manifest, clips_dir)
        clip_checks = verify_clips(clip_entries)
        failures = [check for check in clip_checks if not check.ok]
        if failures:
            problems = "\n".join(
                f"- {entry.clip_set}/{entry.clip_id}: exists={entry.exists}, expected={entry.sha256_expected}, actual={entry.sha256_actual}"
                for entry in failures
            )
            raise SystemExit(f"clip validation failed:\n{problems}")

    if sum(bool(x) for x in [args.norito, args.norito_json, args.norito_per_clip]) > 1:
        raise SystemExit("use only one of --norito, --norito-json, or --norito-per-clip")

    norito_result: Optional[NoritoResult] = None
    norito_per_clip: Optional[List[NoritoResult]] = None
    norito_chunk_dir = args.norito_chunk_dir.resolve() if args.norito_chunk_dir else output_dir / "norito" / "chunks"
    norito_json_single = args.norito_json_out or (output_dir / "norito_entropy.json")

    extra_args = args.norito_args if args.norito_args else []

    if args.norito_per_clip:
        if not clips_dir or not clip_checks:
            raise SystemExit("--norito-per-clip requires clips and a manifest")
        norito_per_clip = []
        per_clip_json_dir = output_dir / "norito"
        per_clip_json_dir.mkdir(parents=True, exist_ok=True)
        for check in clip_checks:
            json_path = per_clip_json_dir / f"{check.clip_id}.json"
            chunk_path = norito_chunk_dir / f"{check.clip_id}.norito"
            result = run_norito_bench(
                json_path,
                frames=args.norito_frames,
                segments=args.norito_segments,
                width=args.norito_width,
                quantizer=args.norito_quantizer,
                y4m_out=None,
                dry_run=args.dry_run,
                y4m_in=Path(check.path),
                chunk_out=chunk_path,
                clip_id=check.clip_id,
                compute_vmaf=args.vmaf,
                psnr_mode=args.psnr_mode,
                norito_args=extra_args,
                enable_bundles=args.enable_bundles,
            )
            norito_per_clip.append(result)
    elif args.norito or args.norito_json:
        target_json = norito_json_single.resolve()
        norito_y4m = args.norito_y4m_out.resolve() if args.norito_y4m_out else None
        norito_y4m_in = args.norito_y4m_in.resolve() if args.norito_y4m_in else None
        chunk_out = norito_chunk_dir / "norito_chunk.norito" if args.norito_y4m_in else None
        if args.norito_json:
            resolved = args.norito_json.resolve()
            norito_result = NoritoResult(command=None, json_path=resolved, duration_seconds=None, payload=_load_norito_payload(resolved), skipped=False)
        else:
            norito_result = run_norito_bench(
                target_json,
                frames=args.norito_frames,
                segments=args.norito_segments,
                width=args.norito_width,
                quantizer=args.norito_quantizer,
                y4m_out=norito_y4m,
                dry_run=args.dry_run,
                y4m_in=norito_y4m_in,
                chunk_out=chunk_out,
                compute_vmaf=args.vmaf,
                psnr_mode=args.psnr_mode,
                norito_args=extra_args,
                enable_bundles=args.enable_bundles,
            )

    specs = parse_runner_specs(args.runner)
    preset_paths = [Path(p).resolve() for p in args.preset]
    if not specs and not preset_paths and DEFAULT_PRESET.exists():
        preset_paths.append(DEFAULT_PRESET)
    if preset_paths:
        specs.extend(parse_runner_presets(preset_paths))
    if not specs and norito_result is None and not norito_per_clip:
        raise SystemExit("no runner commands supplied; use --runner or --preset or Norito flags")

    env = os.environ.copy()
    env["RANS_TABLES_PATH"] = str(tables.path)
    env["RANS_TABLES_CSV"] = str(csv_path)
    env["RANS_COMPARE_OUTPUT"] = str(output_dir)
    if clips_dir:
        env["RANS_REFERENCE_CLIPS"] = str(clips_dir)
    if clip_manifest:
        env["RANS_CLIP_MANIFEST"] = str(clip_manifest)

    results: List[RunnerResult] = []
    for spec in specs:
        command = substitute_placeholders(spec.command, csv_path=csv_path, tables_path=tables.path, clips_dir=clips_dir, output_dir=output_dir)
        substituted = RunnerSpec(label=spec.label, command=command)
        print(f"[rans-compare] running '{spec.label}'")
        try:
            result = run_runner(substituted, env=env, workdir=REPO_ROOT, log_dir=logs_dir, dry_run=args.dry_run)
        except subprocess.CalledProcessError as exc:
            if args.skip_verify:
                print(f"[rans-compare] runner '{spec.label}' failed: {exc}, skipping", file=sys.stderr)
                continue
            raise
        results.append(result)

    report = build_report(tables, csv_path, results, output_dir, args.notes, norito_result, norito_per_clip, args.psnr_mode)
    report["svt_min_dimension"] = os.environ.get("SVT_MIN_DIMENSION")
    report_path = output_dir / "report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    write_readme(output_dir, csv_path)
    print(f"[rans-compare] artefacts stored under {output_dir}")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    sys.exit(main())
