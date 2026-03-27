#!/usr/bin/env python3
"""Wrap a `fastpq_metal_bench` report with metadata for publication."""
from __future__ import annotations

import argparse
import json
import math
import platform
import re
import socket
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Tuple

SCRIPT_DIR = Path(__file__).resolve().parent
ACCEL_HELPERS_DIR = SCRIPT_DIR.parent / "acceleration"
if ACCEL_HELPERS_DIR.is_dir():
    sys.path.append(str(ACCEL_HELPERS_DIR))
try:
    import export_prometheus  # type: ignore
except Exception:
    export_prometheus = None

REQUIRED_LABEL_FIELDS = ("device_class", "gpu_kind")

SYSTEM_PROFILER_TIMEOUT = 5  # seconds
APPLE_CHIP_PATTERN = re.compile(r"apple\s+(m\d)(?:\s+(.*?))?\s*$", re.IGNORECASE)
BUILTIN_GPU_BUSES = {"spdisplays_builtin", "spdisplays_internal"}
POSEIDON_OPERATION = "poseidon_hash_columns"
POSEIDON_PIPELINE_METRIC = "fastpq_poseidon_pipeline_total"
EXECUTION_MODE_METRIC = "fastpq_execution_mode_total"
POSEIDON_LABEL_KEYS = (
    "requested",
    "resolved",
    "path",
    "device_class",
    "chip_family",
    "gpu_kind",
)
EXECUTION_LABEL_KEYS = (
    "requested",
    "resolved",
    "backend",
    "device_class",
    "chip_family",
    "gpu_kind",
)
METRIC_LINE_PATTERN = re.compile(
    r'^(?P<name>[a-zA-Z_:][a-zA-Z0-9_:]*)'
    r'(?:\{(?P<labels>[^}]*)\})?\s+'
    r'(?P<value>[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?)\s*$'
)
METRIC_LABEL_PATTERN = re.compile(r'(?P<key>[a-zA-Z_][a-zA-Z0-9_]*)="(?P<value>(?:[^"\\]|\\.)*)"')


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="Path to fastpq_metal_bench JSON output")
    parser.add_argument(
        "output",
        type=Path,
        help="Destination path for the wrapped benchmark JSON",
    )
    parser.add_argument(
        "--require-poseidon-mean-ms",
        type=float,
        help="Fail if the Poseidon GPU mean exceeds this many milliseconds (e.g., 1000).",
    )
    parser.add_argument(
        "--command",
        help="Command string to capture in the metadata (defaults to the canonical invocation)",
    )
    parser.add_argument(
        "--notes",
        help="Optional override for the metadata notes field",
    )
    parser.add_argument(
        "--host",
        help="Explicit host identifier to record (default: current hostname)",
    )
    parser.add_argument(
        "--platform",
        help="Explicit platform string to record (default: platform.platform())",
    )
    parser.add_argument(
        "--machine",
        help="Explicit machine/arch label (default: platform.machine())",
    )
    parser.add_argument(
        "--require-lde-mean-ms",
        type=float,
        help="Fail if the LDE GPU mean exceeds this many milliseconds (e.g., 950).",
    )
    parser.add_argument(
        "--sign-output",
        action="store_true",
        help="Generate a detached ASCII-armored signature (`<output>.asc`) with gpg.",
    )
    parser.add_argument(
        "--gpg-key",
        help="GPG key (fingerprint or email) to use when signing; defaults to gpg's default key.",
    )
    parser.add_argument(
        "--label",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="Override or add metadata labels (can be repeated).",
    )
    parser.add_argument(
        "--row-usage",
        type=Path,
        help=(
            "Optional path to an `iroha_cli audit witness --decode` JSON blob. "
            "When provided, the script extracts the FASTPQ row_usage summary and "
            "embeds it under metadata.row_usage_snapshot so release bundles always "
            "carry the transfer-gadget evidence."
        ),
    )
    parser.add_argument(
        "--poseidon-metrics",
        action="append",
        default=[],
        type=Path,
        metavar="FILE",
        help=(
            "Optional Prometheus scrape containing fastpq_poseidon_pipeline_total samples. "
            "Pass once per scrape to capture CUDA poseidon fallback telemetry."
        ),
    )
    parser.add_argument(
        "--require-zero-fill-max-ms",
        type=float,
        default=None,
        metavar="MILLISECONDS",
        help=(
            "Fail when zero-fill telemetry is missing or exceeds this latency. "
            "Use to enforce Stage7 zero-fill SLOs (example: 0.40)."
        ),
    )
    parser.add_argument(
        "--accel-state-json",
        type=Path,
        help=(
            "Path to an existing acceleration-state JSON (from `cargo xtask acceleration-state --format json`). "
            "If omitted, the wrapper attempts to capture the state automatically."
        ),
    )
    parser.add_argument(
        "--accel-state-prom",
        type=Path,
        help="Optional path for a Prometheus textfile export of the acceleration state (default: alongside output).",
    )
    parser.add_argument(
        "--accel-instance",
        help="Instance label to use in the acceleration Prometheus export (default: device_class or host).",
    )
    parser.add_argument(
        "--skip-acceleration-state",
        action="store_true",
        help="Skip capturing/embedding acceleration-state metadata and Prometheus textfiles.",
    )
    return parser.parse_args()


def iso_timestamp(epoch_seconds: int | None) -> str:
    if epoch_seconds is None:
        return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    return (
        datetime.fromtimestamp(epoch_seconds, tz=timezone.utc)
        .isoformat()
        .replace("+00:00", "Z")
    )


def round3(value: float | None) -> float | None:
    if value is None:
        return None
    return round(value, 3)


def _slugify_label(value: str) -> str:
    cleaned = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return cleaned or "unknown"


def classify_chip_descriptor(chip_type: str | None) -> dict[str, str] | None:
    if not chip_type:
        return None
    normalized = chip_type.strip()
    if not normalized:
        return None
    base = normalized.split("(")[0].strip()
    match = APPLE_CHIP_PATTERN.match(base)
    labels: dict[str, str]
    if match:
        family = match.group(1).lower()
        bin_raw = (match.group(2) or "base").strip()
        bin_slug = _slugify_label(bin_raw) if bin_raw else "base"
        device_class = f"apple-{family}"
        if bin_slug and bin_slug != "base":
            device_class = f"{device_class}-{bin_slug}"
        labels = {
            "chip_type": normalized,
            "chip_family": family,
            "chip_bin": bin_slug or "base",
            "device_class": device_class,
        }
    else:
        labels = {
            "chip_type": normalized,
            "device_class": _slugify_label(normalized),
        }
    return labels


def classify_gpu_devices(entries: list[dict[str, Any]] | None) -> dict[str, Any] | None:
    if not entries:
        return None
    candidates: list[dict[str, Any]] = []
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        device_type = entry.get("sppci_device_type") or entry.get("spdisplays_device_type")
        if device_type == "spdisplays_gpu":
            candidates.append(entry)
    if not candidates:
        candidates = [entry for entry in entries if isinstance(entry, dict)]
    if not candidates:
        return None
    primary = candidates[0]
    bus = primary.get("sppci_bus") or primary.get("spdisplays_bus")
    gpu_kind = "integrated" if bus in BUILTIN_GPU_BUSES or bus is None else "discrete"
    vendor_raw = primary.get("spdisplays_vendor") or primary.get("sppci_vendor")
    vendor = None
    if isinstance(vendor_raw, str) and vendor_raw:
        vendor = _slugify_label(vendor_raw.split("_")[-1])
    model = (
        primary.get("sppci_model")
        or primary.get("_name")
        or primary.get("spdisplays_model")
        or primary.get("spdisplays_device")
        or "unknown"
    )
    labels: dict[str, Any] = {
        "gpu_model": model,
        "gpu_kind": gpu_kind,
        "gpu_bus": _slugify_label(bus.replace("spdisplays_", "")) if isinstance(bus, str) else "unknown",
    }
    if vendor:
        labels["gpu_vendor"] = vendor
    if primary.get("sppci_cores"):
        labels["gpu_cores"] = str(primary["sppci_cores"])
    return labels


def _system_profiler_json(data_type: str) -> dict[str, Any] | None:
    try:
        output = subprocess.run(
            ["system_profiler", data_type, "-json"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=SYSTEM_PROFILER_TIMEOUT,
        )
    except (FileNotFoundError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return None
    try:
        return json.loads(output.stdout)
    except json.JSONDecodeError:
        return None


def _run_command(cmd: list[str], timeout: int = 2) -> str | None:
    try:
        result = subprocess.run(
            cmd,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=timeout,
        )
    except (FileNotFoundError, subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return None
    output = result.stdout.strip()
    return output or None


def detect_device_labels() -> dict[str, Any] | None:
    if platform.system() != "Darwin":
        if platform.system() == "Linux":
            return detect_linux_labels()
        return None
    labels: dict[str, Any] = {}
    hardware = _system_profiler_json("SPHardwareDataType")
    if hardware:
        entries = hardware.get("SPHardwareDataType")
        if isinstance(entries, list) and entries:
            entry = entries[0]
            chip = (
                entry.get("chip_type")
                or entry.get("current_processor")
                or entry.get("cpu_type")
            )
            chip_labels = classify_chip_descriptor(chip if isinstance(chip, str) else None)
            if chip_labels:
                labels.update(chip_labels)
    displays = _system_profiler_json("SPDisplaysDataType")
    if displays:
        entries = displays.get("SPDisplaysDataType")
        if isinstance(entries, list):
            gpu_labels = classify_gpu_devices(entries)
            if gpu_labels:
                labels.update(gpu_labels)
    return labels or None


def detect_linux_labels() -> dict[str, Any] | None:
    labels: dict[str, Any] = {}
    cpu_model = detect_linux_cpu_model()
    if cpu_model:
        labels["cpu_model"] = cpu_model
    cpu_arch = platform.machine()
    if cpu_arch:
        labels["cpu_arch"] = cpu_arch.lower()
    gpu_labels = detect_linux_gpu()
    if gpu_labels:
        labels.update(gpu_labels)
    device_class = classify_device_class(labels)
    if device_class:
        labels.setdefault("device_class", device_class)
    return labels or None


def detect_linux_cpu_model() -> str | None:
    cpuinfo_path = Path("/proc/cpuinfo")
    if cpuinfo_path.exists():
        try:
            for line in cpuinfo_path.read_text(encoding="utf-8").splitlines():
                if line.lower().startswith("model name"):
                    _, value = line.split(":", 1)
                    value = value.strip()
                    if value:
                        return value
        except OSError:
            pass
    output = _run_command(["lscpu"])
    if output:
        for line in output.splitlines():
            if "Model name:" in line:
                value = line.split(":", 1)[1].strip()
                if value:
                    return value
    proc_name = platform.processor()
    return proc_name or None


def detect_linux_gpu() -> dict[str, Any] | None:
    fetchers = [detect_nvidia_gpu, detect_rocm_gpu, detect_lspci_gpu]
    for fetch in fetchers:
        labels = fetch()
        if labels:
            return labels
    return None


def detect_nvidia_gpu() -> dict[str, Any] | None:
    output = _run_command(
        ["nvidia-smi", "--query-gpu=name,bus_type", "--format=csv,noheader"],
        timeout=3,
    )
    if not output:
        return None
    first_line = output.strip().splitlines()[0].strip()
    if not first_line:
        return None
    parts = [part.strip() for part in first_line.split(",")]
    model = parts[0]
    bus = parts[1] if len(parts) > 1 and parts[1] else "pci"
    return {
        "gpu_model": model,
        "gpu_vendor": "nvidia",
        "gpu_kind": "discrete",
        "gpu_bus": _slugify_label(bus),
    }


def detect_rocm_gpu() -> dict[str, Any] | None:
    output = _run_command(
        ["rocm-smi", "--showproductname"],
        timeout=3,
    )
    if not output:
        return None
    for line in output.splitlines():
        if ":" in line and ("GPU" in line or "Card" in line):
            parts = line.split(":", 1)
            model = parts[1].strip()
            if model:
                return {
                    "gpu_model": model,
                    "gpu_vendor": "amd",
                    "gpu_kind": "discrete",
                    "gpu_bus": "pci",
                }
    return None


def detect_lspci_gpu() -> dict[str, Any] | None:
    output = _run_command(["lspci", "-mm"])
    if not output:
        output = _run_command(["lspci"])
    if not output:
        return None
    for raw_line in output.splitlines():
        line = raw_line.strip()
        lowered = line.lower()
        if "vga" not in lowered and "3d controller" not in lowered:
            continue
        if '"' in line:
            segments = [segment.strip('"') for segment in line.split('"') if segment]
            if len(segments) >= 3:
                vendor_segment = segments[2]
            else:
                vendor_segment = segments[-1]
        else:
            _, vendor_segment = line.split(":", 1)
        vendor_segment = vendor_segment.strip()
        vendor_slug = _vendor_slug(vendor_segment)
        gpu_kind = "integrated" if vendor_slug == "intel" else "discrete"
        return {
            "gpu_model": vendor_segment,
            "gpu_vendor": vendor_slug,
            "gpu_kind": gpu_kind,
            "gpu_bus": "pci",
        }
    return None


def _vendor_slug(value: str) -> str:
    lower = value.lower()
    if "nvidia" in lower:
        return "nvidia"
    if "advanced micro devices" in lower or "amd" in lower or "ati" in lower:
        return "amd"
    if "intel" in lower:
        return "intel"
    return _slugify_label(value)


def classify_device_class(labels: dict[str, Any]) -> str | None:
    cpu_model = str(labels.get("cpu_model") or "").lower()
    gpu_vendor = str(labels.get("gpu_vendor") or "").lower()
    gpu_model = str(labels.get("gpu_model") or "").lower()
    arch = str(labels.get("cpu_arch") or "").lower()
    if "xeon" in cpu_model and gpu_vendor == "nvidia":
        if "6000" in gpu_model and "ada" in gpu_model:
            return "xeon-rtx-sm80"
        return "xeon-nvidia"
    if ("neoverse" in cpu_model or arch in {"aarch64", "arm64"}) and gpu_vendor in {
        "amd",
        "ati",
    }:
        if "mi300" in gpu_model:
            return "neoverse-mi300"
        return f"neoverse-{gpu_vendor}"
    if gpu_vendor:
        gpu_slug = _slugify_label(gpu_vendor)
        arch_slug = _slugify_label(arch) if arch else None
        if arch_slug:
            return f"{arch_slug}-{gpu_slug}"
        return gpu_slug
    if cpu_model:
        cpu_slug = cpu_model.split()[0]
        return _slugify_label(cpu_slug)
    return None


def parse_metric_labels(raw: str | None) -> dict[str, str]:
    if not raw:
        return {}
    labels: dict[str, str] = {}
    for match in METRIC_LABEL_PATTERN.finditer(raw):
        key = match.group("key")
        encoded = match.group("value")
        labels[key] = bytes(encoded, "utf-8").decode("unicode_escape")
    return labels


def parse_metric_value(raw: str) -> float | int:
    value = float(raw)
    if not math.isfinite(value):
        return value
    if value.is_integer():
        return int(value)
    return value


def collect_metric_samples(
    text: str,
    metric_names: tuple[str, ...],
) -> dict[str, list[dict[str, Any]]]:
    samples = {name: [] for name in metric_names}
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        match = METRIC_LINE_PATTERN.match(line)
        if not match:
            continue
        name = match.group("name")
        if name not in samples:
            continue
        labels = parse_metric_labels(match.group("labels"))
        samples[name].append(
            {
                "labels": labels,
                "value": parse_metric_value(match.group("value")),
            }
        )
    return samples


def filter_metric_samples(
    samples: list[dict[str, Any]],
    device_class: str | None,
) -> list[dict[str, Any]]:
    if not samples:
        return []
    if not device_class:
        return samples
    filtered = [sample for sample in samples if sample["labels"].get("device_class") == device_class]
    return filtered or samples


def summarize_metric_entries(
    samples: list[dict[str, Any]],
    keys: tuple[str, ...],
) -> list[dict[str, Any]]:
    formatted: list[dict[str, Any]] = []
    for sample in samples:
        entry = {key: sample["labels"].get(key) for key in keys if sample["labels"].get(key) is not None}
        entry["value"] = sample["value"]
        formatted.append(entry)
    return formatted


def build_poseidon_metric_summary(path: Path, device_class: str | None) -> dict[str, Any]:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise SystemExit(f"[poseidon metrics] failed to read {path}: {exc}") from exc
    metrics = collect_metric_samples(
        text,
        (
            POSEIDON_PIPELINE_METRIC,
            EXECUTION_MODE_METRIC,
        ),
    )
    pipeline_samples = filter_metric_samples(metrics.get(POSEIDON_PIPELINE_METRIC, []), device_class)
    if not pipeline_samples:
        raise SystemExit(
            f"[poseidon metrics] {path} is missing {POSEIDON_PIPELINE_METRIC} samples; "
            "scrape the node before wrapping the benchmark."
        )
    summary: dict[str, Any] = {
        "source": str(path),
        "poseidon_pipeline": summarize_metric_entries(pipeline_samples, POSEIDON_LABEL_KEYS),
    }
    execution_samples = filter_metric_samples(metrics.get(EXECUTION_MODE_METRIC, []), device_class)
    if execution_samples:
        summary["execution_mode"] = summarize_metric_entries(execution_samples, EXECUTION_LABEL_KEYS)
    return summary


def parse_label_overrides(pairs: list[str]) -> dict[str, str]:
    overrides: dict[str, str] = {}
    for pair in pairs:
        if "=" not in pair:
            raise SystemExit(f"label override '{pair}' must be in KEY=VALUE form")
        key, value = pair.split("=", 1)
        key = key.strip()
        if not key:
            raise SystemExit(f"label override '{pair}' is missing a key")
        overrides[key] = value.strip()
    return overrides


def stringify_labels(labels: dict[str, Any]) -> dict[str, str]:
    rendered: dict[str, str] = {}
    for key, value in labels.items():
        if value is None:
            continue
        if isinstance(value, bool):
            rendered[key] = "true" if value else "false"
        else:
            rendered[key] = str(value)
    return rendered


def ensure_required_labels(labels: dict[str, Any]) -> None:
    missing: list[str] = []
    for field in REQUIRED_LABEL_FIELDS:
        value = labels.get(field)
        if value is None:
            missing.append(field)
            continue
        if isinstance(value, str) and not value.strip():
            missing.append(field)
    if not missing:
        return
    suggestions = " ".join(f"--label {field}=<value>" for field in missing)
    formatted = ", ".join(missing)
    raise SystemExit(
        "FASTPQ benchmark metadata missing required label(s): "
        f"{formatted}. Rerun on the capture host so device detection can "
        "populate them or supply overrides via "
        f"{suggestions}."
    )


def load_json_file(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise SystemExit(f"[row_usage] file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise SystemExit(f"[row_usage] invalid JSON in {path}: {exc}") from exc


def capture_acceleration_state_from_xtask() -> dict[str, Any] | None:
    cmd = ["cargo", "xtask", "acceleration-state", "--format", "json"]
    try:
        result = subprocess.run(
            cmd,
            check=True,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError as exc:
        print(f"[acceleration-state] failed to run cargo: {exc}", file=sys.stderr)
        return None
    except subprocess.CalledProcessError as exc:
        stdout = exc.stdout or ""
        stderr = exc.stderr or ""
        print(
            "[acceleration-state] command failed; stdout/stderr follow:\n"
            f"{stdout}\n{stderr}",
            file=sys.stderr,
        )
        return None
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        print(f"[acceleration-state] invalid JSON output: {exc}", file=sys.stderr)
        return None


def persist_acceleration_state(
    state: dict[str, Any],
    output_path: Path,
    instance_label: str | None = None,
    prom_path: Path | None = None,
) -> tuple[Path, Path | None]:
    accel_json_path = output_path.with_suffix(output_path.suffix + ".accel.json")
    accel_json_path.parent.mkdir(parents=True, exist_ok=True)
    accel_json_path.write_text(json.dumps(state, indent=2))
    rendered_prom = None
    prom_output = None
    if export_prometheus:
        try:
            rendered_prom = export_prometheus.render_prometheus(
                state,
                instance=instance_label,
            )
        except Exception as exc:  # pragma: no cover - defensive logging
            print(f"[acceleration-state] failed to render Prometheus text: {exc}", file=sys.stderr)
        else:
            prom_output = prom_path or output_path.with_suffix(output_path.suffix + ".accel.prom")
            prom_output.parent.mkdir(parents=True, exist_ok=True)
            prom_output.write_text(rendered_prom)
    return accel_json_path, prom_output


def _gather_row_usage_batches(blob: Any) -> list[dict[str, Any]]:
    stack: list[Any] = [blob]
    batches: list[dict[str, Any]] = []
    while stack:
        current = stack.pop()
        if isinstance(current, dict):
            if isinstance(current.get("row_usage"), dict):
                batches.append(current)
                continue
            stack.extend(current.values())
        elif isinstance(current, list):
            stack.extend(current)
    if not batches:
        raise SystemExit("[row_usage] no batches with row_usage were found in the provided blob")
    return batches


def _row_usage_key(batch: dict[str, Any], index: int, seen: dict[str, int]) -> str:
    key = batch.get("entry_hash") or f"batch:{index}"
    if key in seen:
        seen[key] += 1
        key = f"{key}#{seen[key]}"
    else:
        seen[key] = 0
    return key


def _sanitize_row_usage(payload: dict[str, Any]) -> dict[str, Any]:
    sanitized: dict[str, Any] = {}
    for key, value in payload.items():
        if isinstance(value, (int, float)):
            if key.endswith("_rows") or key in {"total_rows", "transfer_rows", "non_transfer_rows"}:
                sanitized[key] = int(value)
            elif key.endswith("_ratio") or key == "transfer_ratio":
                sanitized[key] = round(float(value), 6)
            else:
                sanitized[key] = value
        else:
            sanitized[key] = value
    return sanitized


def summarize_row_usage_snapshot(path: Path) -> dict[str, Any]:
    blob = load_json_file(path)
    batches = _gather_row_usage_batches(blob)
    seen: dict[str, int] = {}
    summary_batches: list[dict[str, Any]] = []
    aggregate: dict[str, int] = {}

    def accumulate_counts(usage: dict[str, Any]) -> None:
        for key, value in usage.items():
            if key.endswith("_rows") or key in {"total_rows", "transfer_rows", "non_transfer_rows"}:
                aggregate[key] = aggregate.get(key, 0) + int(value)

    for index, batch in enumerate(batches):
        key = _row_usage_key(batch, index, seen)
        usage = batch.get("row_usage") or {}
        sanitized_usage = _sanitize_row_usage(usage)
        total_rows = int(usage.get("total_rows", 0))
        transfer_rows = int(usage.get("transfer_rows", 0))
        ratio = 0.0 if total_rows == 0 else transfer_rows / total_rows
        summary_batches.append(
            {
                "id": key,
                "parameter": batch.get("parameter"),
                "row_usage": sanitized_usage,
                "transfer_ratio": round(ratio, 6),
            }
        )
        accumulate_counts(usage)

    aggregate_summary = {key: value for key, value in aggregate.items()}
    total_rows = aggregate_summary.get("total_rows", 0)
    transfer_rows = aggregate_summary.get("transfer_rows", 0)
    aggregate_summary["transfer_ratio"] = (
        0.0 if not total_rows else round(transfer_rows / total_rows, 6)
    )

    return {
        "source": path.name,
        "batches": summary_batches,
        "aggregate": aggregate_summary,
    }


def summarize_zero_fill(entry: dict | None) -> dict | None:
    if not entry:
        return None
    bytes_total = entry.get("bytes")
    ms_summary = entry.get("ms") or {}
    mean_ms = ms_summary.get("mean_ms")
    min_ms = ms_summary.get("min_ms")
    max_ms = ms_summary.get("max_ms")
    bandwidth_gbps = None
    if bytes_total is not None and mean_ms:
        seconds = mean_ms / 1_000.0
        if seconds > 0:
            bandwidth_gbps = round3(bytes_total / seconds / 1_000_000_000)
    queue_delta = entry.get("queue_delta") or {}
    queue_summary = None
    if queue_delta:
        queue_summary = {
            "limit": queue_delta.get("limit"),
            "dispatch_count": queue_delta.get("dispatch_count"),
            "max_in_flight": queue_delta.get("max_in_flight"),
            "busy_ms": round3(queue_delta.get("busy_ms")),
            "overlap_ms": round3(queue_delta.get("overlap_ms")),
        }
    return {
        "bytes": bytes_total,
        "mean_ms": mean_ms,
        "min_ms": min_ms,
        "max_ms": max_ms,
        "bandwidth_gbps": bandwidth_gbps,
        "queue_delta": queue_summary,
    }


def summarize_operations(report: dict) -> Tuple[list[dict], list[dict]]:
    operations = []
    hotspots = []
    for entry in report.get("operations", []):
        zero_fill = summarize_zero_fill(entry.get("zero_fill"))
        summary: dict[str, Any] = {
            "operation": entry.get("operation"),
            "columns": entry.get("columns"),
            "input_len": entry.get("input_len"),
            "output_len": entry.get("output_len"),
            "input_bytes": entry.get("input_bytes"),
            "output_bytes": entry.get("output_bytes"),
            "estimated_gpu_transfer_bytes": entry.get("estimated_gpu_transfer_bytes"),
            "cpu_mean_ms": (entry.get("cpu") or {}).get("mean_ms"),
            "gpu_mean_ms": (entry.get("gpu") or {}).get("mean_ms"),
            "speedup_ratio": (entry.get("speedup") or {}).get("ratio"),
            "speedup_delta_ms": (entry.get("speedup") or {}).get("delta_ms"),
        }
        if zero_fill:
            summary["zero_fill"] = zero_fill
            hotspots.append(
                {
                    "operation": summary["operation"],
                    "bytes": zero_fill.get("bytes"),
                    "mean_ms": zero_fill.get("mean_ms"),
                    "bandwidth_gbps": zero_fill.get("bandwidth_gbps"),
                    "queue_delta": zero_fill.get("queue_delta"),
                }
            )
        operations.append(summary)
    hotspots.sort(
        key=lambda item: item.get("mean_ms") or 0.0,
        reverse=True,
    )
    return operations, hotspots


def normalize_report(payload: dict[str, Any]) -> dict[str, Any]:
    nested = payload.get("report")
    if not isinstance(nested, dict):
        return payload

    report = dict(nested)
    benchmarks = payload.get("benchmarks")
    if isinstance(benchmarks, dict):
        for field in (
            "rows",
            "padded_rows",
            "iterations",
            "warmups",
            "column_count",
            "execution_mode",
            "gpu_backend",
            "gpu_available",
            "operation_filter",
            "bn254_warnings",
        ):
            if report.get(field) is None and field in benchmarks:
                report[field] = benchmarks.get(field)
        benchmark_operations = benchmarks.get("operations")
        report_operations = report.get("operations")
        if isinstance(report_operations, list) and isinstance(benchmark_operations, list):
            benchmark_by_name = {
                entry.get("operation"): entry
                for entry in benchmark_operations
                if isinstance(entry, dict) and entry.get("operation") is not None
            }
            merged_operations = []
            for entry in report_operations:
                if not isinstance(entry, dict):
                    merged_operations.append(entry)
                    continue
                merged_entry = dict(entry)
                benchmark_entry = benchmark_by_name.get(entry.get("operation"))
                if isinstance(benchmark_entry, dict):
                    for field in (
                        "columns",
                        "input_len",
                        "output_len",
                        "input_bytes",
                        "output_bytes",
                        "estimated_gpu_transfer_bytes",
                    ):
                        if merged_entry.get(field) is None and field in benchmark_entry:
                            merged_entry[field] = benchmark_entry.get(field)
                merged_operations.append(merged_entry)
            report["operations"] = merged_operations
        elif "operations" not in report and "operations" in benchmarks:
            report["operations"] = benchmark_operations

    metadata = payload.get("metadata")
    if isinstance(metadata, dict):
        report_metadata = report.get("metadata")
        if isinstance(report_metadata, dict):
            merged_metadata = dict(report_metadata)
            for field in ("generated_at", "host", "platform", "machine", "command", "notes"):
                if merged_metadata.get(field) is None and field in metadata:
                    merged_metadata[field] = metadata.get(field)
            report["metadata"] = merged_metadata
        elif "metadata" not in report:
            report["metadata"] = metadata
    return report


def enforce_zero_fill_hotspots(hotspots: list[dict], threshold_ms: float | None) -> None:
    if threshold_ms is None:
        return
    if not hotspots:
        raise SystemExit(
            "Zero-fill telemetry missing from the benchmark report; rerun fastpq_metal_bench with zero-fill stats enabled."
        )
    worst_mean = None
    worst_label = None
    for entry in hotspots:
        value = entry.get("mean_ms")
        if value is None:
            continue
        if worst_mean is None or value > worst_mean:
            worst_mean = value
            worst_label = entry.get("operation") or "unknown"
    if worst_mean is None:
        raise SystemExit(
            "Zero-fill telemetry present but missing mean_ms values; cannot enforce the zero-fill SLO."
        )
    if worst_mean > threshold_ms:
        raise SystemExit(
            f"Zero-fill mean for {worst_label} ({worst_mean:.3f} ms) exceeds threshold {threshold_ms:.3f} ms."
        )


def summarize_dispatch_queue(entry: dict | None) -> dict | None:
    if not entry or not isinstance(entry, dict):
        return None
    summary: dict[str, Any] = {}
    for key in ("limit", "max_in_flight", "dispatch_count"):
        value = entry.get(key)
        if value is not None:
            summary[key] = value
    for key in ("busy_ms", "overlap_ms", "overlap_ratio"):
        value = entry.get(key)
        if value is not None:
            summary[key] = round3(value)
    poseidon = summarize_dispatch_queue(entry.get("poseidon"))
    if poseidon:
        summary["poseidon"] = poseidon
    return summary or None


def summarize_poseidon_microbench(entry: dict | None) -> dict | None:
    if not isinstance(entry, dict):
        return None

    def summarize_sample(sample: Any) -> dict | None:
        if not isinstance(sample, dict):
            return None
        summary: dict[str, Any] = {}
        for key in ("mean_ms", "min_ms", "max_ms"):
            value = sample.get(key)
            if value is not None:
                summary[key] = round3(value)
        for key in ("columns", "trace_log2", "states", "warmups", "iterations"):
            value = sample.get(key)
            if value is not None:
                summary[key] = value
        tuning = sample.get("tuning")
        if isinstance(tuning, dict):
            summary["tuning"] = {
                "threadgroup_lanes": tuning.get("threadgroup_lanes"),
                "states_per_lane": tuning.get("states_per_lane"),
            }
        return summary or None

    result: dict[str, Any] = {}
    default_summary = summarize_sample(entry.get("default"))
    if default_summary:
        result["default"] = default_summary
    scalar_summary = summarize_sample(entry.get("scalar_lane"))
    if scalar_summary:
        result["scalar_lane"] = scalar_summary
    speedup = entry.get("speedup_vs_scalar")
    if speedup is not None:
        result["speedup_vs_scalar"] = round3(speedup)
    return result or None


def summarize_trace_metadata(report: dict) -> dict | None:
    template = report.get("metal_trace_template")
    seconds = report.get("metal_trace_seconds")
    output = report.get("metal_trace_output")
    if not any((template, seconds, output)):
        return None
    return {
        "template": template,
        "seconds": seconds,
        "output": output,
    }


def summarize_range(entry: dict | None) -> dict | None:
    if not entry or not isinstance(entry, dict):
        return None
    summary: dict[str, float] = {}
    for key in ("mean", "min", "max"):
        value = entry.get(key)
        if value is not None:
            summary[key] = round3(value)  # type: ignore[assignment]
    return summary or None


def summarize_kernel_profiles(profiles: dict | None) -> dict | None:
    if not profiles:
        return None
    by_kind = profiles.get("by_kind")
    if not isinstance(by_kind, dict):
        return None
    kinds: list[dict[str, Any]] = []
    for kind, data in sorted(by_kind.items()):
        if not isinstance(data, dict):
            continue
        kinds.append(
            {
                "kind": kind,
                "sample_count": data.get("sample_count"),
                "bytes_total": data.get("bytes_total"),
                "elements_total": data.get("elements_total"),
                "duration_ms": summarize_range(data.get("duration_ms")),
                "bandwidth_gbps": summarize_range(data.get("bandwidth_gbps")),
                "occupancy_ratio": summarize_range(data.get("occupancy_ratio")),
            }
        )
    if not kinds:
        return None
    return {
        "total_samples": profiles.get("total_samples"),
        "kinds": kinds,
    }


def summarize_post_tile(dispatches: dict | None) -> dict | None:
    if not dispatches or not isinstance(dispatches, dict):
        return None
    total = dispatches.get("total_dispatches")
    kinds = dispatches.get("kinds")
    if total is None or kinds is None:
        return None
    kind_counts: dict[str, Any] = {}
    stage_means: dict[str, Any] = {}
    for entry in kinds:
        if not isinstance(entry, dict):
            continue
        kind = entry.get("kind")
        if not isinstance(kind, str):
            continue
        kind_counts[kind] = entry.get("dispatch_count")
        stage = entry.get("stage_start_log2") or {}
        if isinstance(stage, dict) and stage.get("mean") is not None:
            stage_means[kind] = round3(stage.get("mean"))
    summary: dict[str, Any] = {
        "total_dispatches": total,
        "per_kind": kind_counts,
    }
    if stage_means:
        summary["stage_start_log2_mean"] = stage_means
    return summary


def auto_notes(report: dict) -> str:
    backend = report.get("gpu_backend")
    if backend in (None, "none"):
        return (
            "FASTPQ_METAL_LIB missing on the capture host; GPU dispatch resolved to"
            " 'none' and timings represent the CPU fallback."
        )
    return f"GPU backend {backend} resolved successfully."


def enforce_lde_threshold(operations: list[dict], threshold_ms: float) -> None:
    for entry in operations:
        if entry.get("operation") == "lde":
            mean_ms = entry.get("gpu_mean_ms")
            if mean_ms is None:
                raise SystemExit(
                    "LDE operation missing GPU metrics; cannot verify sub-second requirement."
                )
            if mean_ms > threshold_ms:
                raise SystemExit(
                    f"LDE GPU mean {mean_ms:.3f} ms exceeds threshold {threshold_ms} ms."
                )
            return
    raise SystemExit("LDE operation not found in benchmark report; required for verification.")


def enforce_poseidon_threshold(operations: list[dict], threshold_ms: float) -> None:
    for entry in operations:
        if entry.get("operation") == POSEIDON_OPERATION:
            mean_ms = entry.get("gpu_mean_ms")
            if mean_ms is None:
                raise SystemExit(
                    "Poseidon operation missing GPU metrics; cannot verify sub-second requirement."
                )
            if mean_ms > threshold_ms:
                raise SystemExit(
                    f"Poseidon GPU mean {mean_ms:.3f} ms exceeds threshold {threshold_ms} ms."
                )
            return
    raise SystemExit(
        "Poseidon operation not found in benchmark report; required when enforcing poseidon thresholds."
    )


def require_poseidon_telemetry(report: dict) -> None:
    operations: list[dict] = report.get("operations") or []
    poseidon_present = any(op.get("operation") == POSEIDON_OPERATION for op in operations)
    if not poseidon_present:
        return
    backend = report.get("gpu_backend")
    if backend != "metal":
        return
    queue = report.get("metal_dispatch_queue")
    if not isinstance(queue, dict):
        raise SystemExit(
            "Poseidon capture missing `metal_dispatch_queue` telemetry; rerun fastpq_metal_bench with stats enabled."
        )
    poseidon_queue = queue.get("poseidon")
    if not isinstance(poseidon_queue, dict):
        raise SystemExit(
            "Poseidon capture missing per-operation queue stats (`metal_dispatch_queue.poseidon`)."
        )
    dispatch_count = poseidon_queue.get("dispatch_count")
    if not isinstance(dispatch_count, (int, float)) or int(dispatch_count) <= 0:
        raise SystemExit(
            "Poseidon queue telemetry reports zero dispatches; GPU captures must contain at least one Poseidon hash dispatch."
        )
    column_staging = report.get("column_staging")
    if not isinstance(column_staging, dict):
        raise SystemExit(
            "Poseidon capture missing `column_staging` telemetry; rerun the bench so staging overlap evidence is recorded."
        )
    if column_staging.get("batches") in (None, 0):
        raise SystemExit(
            "Poseidon capture reported zero staging batches; confirm FASTPQ_METAL_TRACE stats were enabled."
        )
    if not isinstance(report.get("poseidon_profiles"), dict):
        raise SystemExit(
            "Poseidon capture missing `poseidon_profiles`; ensure FASTPQ_METAL_GPU_STATS=1 is set."
        )
    if not isinstance(report.get("poseidon_microbench"), dict):
        raise SystemExit(
            "Poseidon capture missing `poseidon_microbench`; rerun fastpq_metal_bench so the scalar vs default comparison is captured."
        )


def sign_output(output_path: Path, gpg_key: str | None) -> None:
    signature = output_path.with_suffix(output_path.suffix + ".asc")
    cmd = ["gpg", "--detach-sign", "--armor", "--output", str(signature), str(output_path)]
    if gpg_key:
        cmd[1:1] = ["--local-user", gpg_key]
    try:
        subprocess.run(cmd, check=True)
    except FileNotFoundError as exc:
        raise SystemExit(f"failed to run gpg for signing: {exc}") from exc


def main() -> None:
    args = parse_args()
    report = normalize_report(json.loads(args.input.read_text()))
    operations, zero_fill_hotspots = summarize_operations(report)
    enforce_zero_fill_hotspots(zero_fill_hotspots, args.require_zero_fill_max_ms)
    trace_meta = summarize_trace_metadata(report)
    require_poseidon_telemetry(report)
    if args.require_lde_mean_ms is not None:
        enforce_lde_threshold(operations, args.require_lde_mean_ms)
    if args.require_poseidon_mean_ms is not None:
        enforce_poseidon_threshold(operations, args.require_poseidon_mean_ms)
    source_metadata = report.get("metadata") if isinstance(report.get("metadata"), dict) else {}
    metadata = {
        "generated_at": iso_timestamp(report.get("unix_epoch_secs")),
        "host": args.host or source_metadata.get("host") or socket.gethostname(),
        "platform": args.platform or source_metadata.get("platform") or platform.platform(),
        "machine": args.machine or source_metadata.get("machine") or platform.machine(),
        "command": args.command
        or source_metadata.get("command")
        or f"FASTPQ_GPU=gpu fastpq_metal_bench --rows {report.get('rows')} --iterations {report.get('iterations')}",
        "notes": args.notes or source_metadata.get("notes") or auto_notes(report),
    }
    if args.row_usage:
        metadata["row_usage_snapshot"] = summarize_row_usage_snapshot(args.row_usage)
    auto_labels = detect_device_labels() or {}
    label_overrides = parse_label_overrides(args.label)
    merged_labels: dict[str, Any] = {}
    if auto_labels:
        merged_labels.update(auto_labels)
    merged_labels.update(label_overrides)
    ensure_required_labels(merged_labels)
    device_class_label = merged_labels.get("device_class")
    metadata["labels"] = stringify_labels(merged_labels)
    if trace_meta:
        metadata["metal_trace"] = trace_meta

    accel_state: dict[str, Any] | None = None
    if not args.skip_acceleration_state:
        if args.accel_state_json:
            try:
                accel_state = json.loads(args.accel_state_json.read_text())
            except FileNotFoundError:
                print(f"[acceleration-state] file not found: {args.accel_state_json}", file=sys.stderr)
            except json.JSONDecodeError as exc:
                print(f"[acceleration-state] invalid JSON in {args.accel_state_json}: {exc}", file=sys.stderr)
        if accel_state is None:
            accel_state = capture_acceleration_state_from_xtask()
        if accel_state:
            instance_label = args.accel_instance or device_class_label or metadata["host"]
            accel_json_path, accel_prom_path = persist_acceleration_state(
                accel_state,
                args.output,
                instance_label=instance_label,
                prom_path=args.accel_state_prom,
            )
            metadata["acceleration_state"] = accel_state
            metadata["acceleration_state_path"] = str(accel_json_path)
            if accel_prom_path:
                metadata["acceleration_prometheus_path"] = str(accel_prom_path)

    benchmarks: dict[str, Any] = {
        "rows": report.get("rows"),
        "padded_rows": report.get("padded_rows"),
        "iterations": report.get("iterations"),
        "warmups": report.get("warmups"),
        "column_count": report.get("column_count"),
        "execution_mode": report.get("execution_mode"),
        "gpu_backend": report.get("gpu_backend"),
        "gpu_available": report.get("gpu_available"),
        "operation_filter": report.get("operation_filter"),
        "operations": operations,
    }
    if zero_fill_hotspots:
        benchmarks["zero_fill_hotspots"] = zero_fill_hotspots
    post_tile = report.get("post_tile_dispatches")
    if post_tile:
        benchmarks["post_tile_dispatches"] = post_tile
        post_tile_summary = summarize_post_tile(post_tile)
        if post_tile_summary:
            benchmarks["post_tile_summary"] = post_tile_summary
    queue_stats = summarize_dispatch_queue(report.get("metal_dispatch_queue"))
    if queue_stats:
        benchmarks["metal_dispatch_queue"] = queue_stats
    kernel_profiles = report.get("kernel_profiles")
    if kernel_profiles:
        benchmarks["kernel_profiles"] = kernel_profiles
        kernel_summary = summarize_kernel_profiles(kernel_profiles)
        if kernel_summary:
            benchmarks["kernel_summary"] = kernel_summary
    poseidon_profiles = report.get("poseidon_profiles")
    if poseidon_profiles:
        benchmarks["poseidon_profiles"] = poseidon_profiles
    poseidon_micro = summarize_poseidon_microbench(report.get("poseidon_microbench"))
    if poseidon_micro:
        benchmarks["poseidon_microbench"] = poseidon_micro
    column_staging = report.get("column_staging")
    if column_staging:
        benchmarks["column_staging"] = column_staging
    bn254_metrics = report.get("bn254_metrics")
    if isinstance(bn254_metrics, dict) and bn254_metrics:
        benchmarks["bn254_metrics"] = bn254_metrics
    bn254_warnings = report.get("bn254_warnings")
    if isinstance(bn254_warnings, list) and bn254_warnings:
        benchmarks["bn254_warnings"] = bn254_warnings
    bn254_dispatch = report.get("bn254_dispatch")
    if isinstance(bn254_dispatch, dict) and bn254_dispatch:
        benchmarks["bn254_dispatch"] = bn254_dispatch
    poseidon_summaries = []
    if args.poseidon_metrics:
        for metrics_path in args.poseidon_metrics:
            poseidon_summaries.append(
                build_poseidon_metric_summary(metrics_path, device_class_label)
            )
    if poseidon_summaries:
        benchmarks["poseidon_metrics"] = poseidon_summaries

    bundle = {
        "metadata": metadata,
        "benchmarks": benchmarks,
        "report": report,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(bundle, indent=2))
    if args.sign_output:
        sign_output(args.output, args.gpg_key)
    print(f"FASTPQ benchmark bundle written to {args.output}")


if __name__ == "__main__":
    main()
