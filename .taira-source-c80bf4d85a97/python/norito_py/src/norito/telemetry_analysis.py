"""
Telemetry analysis helpers for Norito streaming.

This module ingests JSON telemetry samples emitted by the streaming runtime
and produces summaries highlighting bitrate/VMAF regressions together with
quantiser table tuning recommendations. The implementation is intentionally
dependency-free so it can run in constrained CI environments or on relay
operators' machines without additional packaging.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Optional, Tuple


@dataclass
class TelemetrySample:
    """Single telemetry emission collected from the streaming stack."""

    stream_id: str
    quant_table: str
    bitrate_bps: float
    timestamp: str
    relay_id: Optional[str] = None
    target_bitrate_bps: Optional[float] = None
    vmaf_score: Optional[float] = None

    @classmethod
    def from_dict(cls, payload: Dict[str, object]) -> "TelemetrySample":
        try:
            stream_id = str(payload["stream_id"])
            quant_table = str(payload["quant_table"])
            bitrate_bps = float(payload["bitrate_bps"])
        except KeyError as err:
            raise ValueError(f"missing required field {err.args[0]!r} in telemetry sample") from err
        except (TypeError, ValueError) as err:
            raise ValueError("telemetry sample contains invalid field types") from err

        timestamp = str(payload.get("timestamp", ""))
        relay_id = payload.get("relay_id")
        target = payload.get("target_bitrate_bps")
        vmaf = payload.get("vmaf")
        vmaf = payload.get("vmaf_score", vmaf)

        return cls(
            stream_id=stream_id,
            quant_table=quant_table,
            bitrate_bps=bitrate_bps,
            timestamp=timestamp,
            relay_id=str(relay_id) if relay_id is not None else None,
            target_bitrate_bps=float(target) if target is not None else None,
            vmaf_score=float(vmaf) if vmaf is not None else None,
        )


@dataclass
class BaselineRule:
    """Reference expectations for a stream or quantiser table."""

    target_bitrate_bps: Optional[float] = None
    tolerance: float = 0.05
    vmaf_floor: float = 92.0

    def merge(self, other: "BaselineRule") -> None:
        if other.target_bitrate_bps is not None:
            self.target_bitrate_bps = other.target_bitrate_bps
        self.tolerance = other.tolerance
        self.vmaf_floor = other.vmaf_floor


@dataclass
class BaselineConfig:
    """Collection of baseline rules used to judge regressions."""

    global_rule: BaselineRule
    stream_rules: Dict[str, BaselineRule]
    quant_rules: Dict[str, BaselineRule]

    @classmethod
    def from_dict(
        cls,
        data: Dict[str, object],
        *,
        default_tolerance: float,
        default_vmaf_floor: float,
    ) -> "BaselineConfig":
        def parse_rule(obj: Optional[Dict[str, object]]) -> BaselineRule:
            if obj is None:
                return BaselineRule(tolerance=default_tolerance, vmaf_floor=default_vmaf_floor)
            target = obj.get("target_bitrate_bps")
            tolerance = float(obj.get("tolerance", default_tolerance))
            vmaf_floor = float(obj.get("vmaf_floor", default_vmaf_floor))
            return BaselineRule(
                target_bitrate_bps=float(target) if target is not None else None,
                tolerance=tolerance,
                vmaf_floor=vmaf_floor,
            )

        stream_rules = {
            stream: parse_rule(rule)
            for stream, rule in data.get("streams", {}).items()
        }
        quant_rules = {
            quant: parse_rule(rule)
            for quant, rule in data.get("quant_tables", {}).items()
        }
        return cls(
            global_rule=parse_rule(data.get("global")),
            stream_rules=stream_rules,
            quant_rules=quant_rules,
        )

    def resolve(
        self,
        stream_id: str,
        quant_table: str,
        sample_target: Optional[float],
    ) -> BaselineRule:
        rule = BaselineRule(
            target_bitrate_bps=self.global_rule.target_bitrate_bps,
            tolerance=self.global_rule.tolerance,
            vmaf_floor=self.global_rule.vmaf_floor,
        )
        if quant_table in self.quant_rules:
            rule.merge(self.quant_rules[quant_table])
        if stream_id in self.stream_rules:
            rule.merge(self.stream_rules[stream_id])
        if sample_target is not None:
            rule.target_bitrate_bps = sample_target
        return rule


@dataclass
class StreamQuantStats:
    samples: int = 0
    bitrate_sum: float = 0.0
    bitrate_min: float = float("inf")
    bitrate_max: float = 0.0
    target_sum: float = 0.0
    target_count: int = 0
    above_tolerance: int = 0
    below_tolerance: int = 0
    vmaf_sum: float = 0.0
    vmaf_count: int = 0
    vmaf_min: Optional[float] = None
    low_vmaf_count: int = 0
    relays: set[str] = field(default_factory=set)
    last_timestamp: str = ""

    def update(self, sample: TelemetrySample, rule: BaselineRule) -> None:
        self.samples += 1
        self.bitrate_sum += sample.bitrate_bps
        self.bitrate_min = min(self.bitrate_min, sample.bitrate_bps)
        self.bitrate_max = max(self.bitrate_max, sample.bitrate_bps)
        if sample.relay_id is not None:
            self.relays.add(sample.relay_id)
        self.last_timestamp = sample.timestamp

        if rule.target_bitrate_bps is not None:
            target = rule.target_bitrate_bps
            self.target_sum += target
            self.target_count += 1
            ratio = sample.bitrate_bps / target if target else 1.0
            if ratio > 1 + rule.tolerance:
                self.above_tolerance += 1
            elif ratio < 1 - rule.tolerance:
                self.below_tolerance += 1

        if sample.vmaf_score is not None:
            self.vmaf_sum += sample.vmaf_score
            self.vmaf_count += 1
            self.vmaf_min = (
                sample.vmaf_score
                if self.vmaf_min is None
                else min(self.vmaf_min, sample.vmaf_score)
            )
            if sample.vmaf_score < rule.vmaf_floor:
                self.low_vmaf_count += 1


class TelemetryAnalyzer:
    """Aggregate telemetry samples and highlight regressions."""

    def __init__(
        self,
        *,
        baseline: Optional[BaselineConfig] = None,
        default_tolerance: float = 0.05,
        default_vmaf_floor: float = 92.0,
        min_samples: int = 12,
    ) -> None:
        self.baseline = baseline or BaselineConfig(
            global_rule=BaselineRule(
                target_bitrate_bps=None,
                tolerance=default_tolerance,
                vmaf_floor=default_vmaf_floor,
            ),
            stream_rules={},
            quant_rules={},
        )
        self.default_tolerance = default_tolerance
        self.default_vmaf_floor = default_vmaf_floor
        self.min_samples = min_samples
        self._series: Dict[Tuple[str, str], StreamQuantStats] = {}

    def ingest(self, sample: TelemetrySample) -> None:
        key = (sample.stream_id, sample.quant_table)
        stats = self._series.setdefault(key, StreamQuantStats())
        rule = self.baseline.resolve(
            sample.stream_id,
            sample.quant_table,
            sample.target_bitrate_bps,
        )
        if rule.tolerance <= 0:
            rule.tolerance = self.default_tolerance
        stats.update(sample, rule)

    def ingest_dict(self, payload: Dict[str, object]) -> None:
        self.ingest(TelemetrySample.from_dict(payload))

    def ingest_many(self, samples: Iterable[Dict[str, object]]) -> None:
        for sample in samples:
            self.ingest_dict(sample)

    def build_report(self) -> Dict[str, object]:
        series: List[Dict[str, object]] = []
        for (stream_id, quant_table), stats in sorted(self._series.items()):
            entry = self._build_entry(stream_id, quant_table, stats)
            series.append(entry)

        total_samples = sum(stat.samples for stat in self._series.values())
        distinct_streams = len({key[0] for key in self._series})
        distinct_tables = len({key[1] for key in self._series})

        return {
            "series": series,
            "summary": {
                "streams": distinct_streams,
                "quant_tables": distinct_tables,
                "samples": total_samples,
                "regressions": sum(1 for entry in series if entry["regression"]),
            },
        }

    def _build_entry(
        self,
        stream_id: str,
        quant_table: str,
        stats: StreamQuantStats,
    ) -> Dict[str, object]:
        avg_bitrate = stats.bitrate_sum / stats.samples if stats.samples else 0.0
        avg_target = (
            stats.target_sum / stats.target_count if stats.target_count else None
        )
        avg_vmaf = stats.vmaf_sum / stats.vmaf_count if stats.vmaf_count else None

        baseline_rule = self.baseline.resolve(stream_id, quant_table, avg_target)
        recommendation = self._make_recommendation(stats, avg_bitrate, avg_target, baseline_rule)

        regression = recommendation["action"] in {"tighten", "loosen"}
        if stats.low_vmaf_count > 0 and recommendation["action"] == "hold":
            regression = True

        return {
            "stream_id": stream_id,
            "quant_table": quant_table,
            "samples": stats.samples,
            "relays_observed": sorted(stats.relays),
            "avg_bitrate_bps": avg_bitrate,
            "min_bitrate_bps": stats.bitrate_min if stats.samples else 0.0,
            "max_bitrate_bps": stats.bitrate_max,
            "avg_target_bps": avg_target,
            "avg_vmaf": avg_vmaf,
            "min_vmaf": stats.vmaf_min,
            "low_vmaf_samples": stats.low_vmaf_count,
            "ratio_above_tolerance": (
                stats.above_tolerance / stats.target_count if stats.target_count else 0.0
            ),
            "ratio_below_tolerance": (
                stats.below_tolerance / stats.target_count if stats.target_count else 0.0
            ),
            "recommendation": recommendation,
            "regression": regression,
            "last_observed_at": stats.last_timestamp,
        }

    def _make_recommendation(
        self,
        stats: StreamQuantStats,
        avg_bitrate: float,
        avg_target: Optional[float],
        rule: BaselineRule,
    ) -> Dict[str, object]:
        if stats.samples < self.min_samples:
            return {
                "action": "insufficient-data",
                "delta_percent": 0.0,
                "reason": f"only {stats.samples} samples (<{self.min_samples})",
            }

        if avg_target is None or avg_target == 0:
            if stats.low_vmaf_count > 0:
                return {
                    "action": "loosen",
                    "delta_percent": 0.0,
                    "reason": "quality regression detected but target bitrate unavailable; lower quantiser manually",
                }
            return {
                "action": "monitor",
                "delta_percent": 0.0,
                "reason": "target bitrate not provided; monitor manually",
            }

        ratio = avg_bitrate / avg_target if avg_target else 1.0
        tolerance = rule.tolerance or self.default_tolerance
        over_limit = ratio > 1 + tolerance
        under_limit = ratio < 1 - tolerance

        if over_limit and stats.low_vmaf_count == 0:
            delta = round((ratio - 1) * 100, 2)
            return {
                "action": "tighten",
                "delta_percent": delta,
                "reason": (
                    f"average bitrate {avg_bitrate:.0f} bps exceeds target "
                    f"{avg_target:.0f} bps by {delta:.2f}% without VMAF degradation"
                ),
            }

        if under_limit or stats.low_vmaf_count > 0:
            delta = round(abs(1 - ratio) * 100, 2)
            reason_parts = []
            if under_limit:
                reason_parts.append(
                    f"average bitrate {avg_bitrate:.0f} bps under target {avg_target:.0f} bps"
                )
            if stats.low_vmaf_count > 0:
                reason_parts.append(
                    f"{stats.low_vmaf_count} samples fell below VMAF floor {rule.vmaf_floor}"
                )
            return {
                "action": "loosen",
                "delta_percent": delta,
                "reason": "; ".join(reason_parts),
            }

        return {
            "action": "hold",
            "delta_percent": 0.0,
            "reason": "within tolerance and VMAF floor",
        }

    def __len__(self) -> int:
        return len(self._series)
