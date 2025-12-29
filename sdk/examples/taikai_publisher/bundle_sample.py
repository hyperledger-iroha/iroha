#!/usr/bin/env python3
"""
Utility that demonstrates how to invoke the Taikai CAR bundler (`taikai_car`)
with a deterministic configuration. This script is part of roadmap item
SN13-F (Publisher SDK/CLI).
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional


@dataclass
class SampleConfig:
    payload: Path
    event_id: str
    stream_id: str
    rendition_id: str
    track_kind: str
    codec: str
    bitrate_kbps: int
    resolution: Optional[str]
    audio_layout: Optional[str]
    sequence: int
    start_pts: int
    duration: int
    wallclock_unix_ms: int
    manifest_hash: str
    storage_ticket: str
    ingest_latency_ms: Optional[int]
    live_edge_drift_ms: Optional[int]
    ingest_node_id: Optional[str]
    metadata_json: Optional[Path]


def load_config(path: Path) -> SampleConfig:
    raw = json.loads(path.read_text())
    base = path.parent

    track = raw.get("track", {})
    segment = raw.get("segment", {})
    ingest = raw.get("ingest", {})

    payload = base / raw["payload"]
    metadata = raw.get("extra_metadata")
    metadata_path = base / metadata if metadata else None

    resolution = track.get("resolution")
    audio_layout = track.get("audio_layout")
    if track.get("kind") == "video" and not resolution:
        raise ValueError("video tracks require `track.resolution`")
    if track.get("kind") == "audio" and not audio_layout:
        raise ValueError("audio tracks require `track.audio_layout`")

    return SampleConfig(
        payload=payload,
        event_id=raw["event_id"],
        stream_id=raw["stream_id"],
        rendition_id=raw["rendition_id"],
        track_kind=track["kind"],
        codec=track["codec"],
        bitrate_kbps=int(track["bitrate_kbps"]),
        resolution=resolution,
        audio_layout=audio_layout,
        sequence=int(segment["sequence"]),
        start_pts=int(segment["start_pts"]),
        duration=int(segment["duration"]),
        wallclock_unix_ms=int(segment["wallclock_unix_ms"]),
        manifest_hash=ingest["manifest_hash"],
        storage_ticket=ingest["storage_ticket"],
        ingest_latency_ms=_optional_int(ingest.get("ingest_latency_ms")),
        live_edge_drift_ms=_optional_int(ingest.get("live_edge_drift_ms")),
        ingest_node_id=ingest.get("ingest_node_id"),
        metadata_json=metadata_path,
    )


def _optional_int(value: Optional[int]) -> Optional[int]:
    return None if value is None else int(value)


def default_outputs(out_dir: Path, payload: Path) -> Dict[str, Path]:
    stem = payload.stem
    return {
        "car": out_dir / f"{stem}.car",
        "envelope": out_dir / f"{stem}.norito",
        "indexes": out_dir / f"{stem}.indexes.json",
        "ingest_metadata": out_dir / f"{stem}.ingest.json",
    }


def build_command(
    binary: str, config: SampleConfig, outputs: Dict[str, Path]
) -> List[str]:
    cmd = [
        binary,
        "--payload",
        str(config.payload),
        "--car-out",
        str(outputs["car"]),
        "--envelope-out",
        str(outputs["envelope"]),
        "--indexes-out",
        str(outputs["indexes"]),
        "--ingest-metadata-out",
        str(outputs["ingest_metadata"]),
        "--event-id",
        config.event_id,
        "--stream-id",
        config.stream_id,
        "--rendition-id",
        config.rendition_id,
        "--track-kind",
        config.track_kind,
        "--codec",
        config.codec,
        "--bitrate-kbps",
        str(config.bitrate_kbps),
        "--segment-sequence",
        str(config.sequence),
        "--segment-start-pts",
        str(config.start_pts),
        "--segment-duration",
        str(config.duration),
        "--wallclock-unix-ms",
        str(config.wallclock_unix_ms),
        "--manifest-hash",
        config.manifest_hash,
        "--storage-ticket",
        config.storage_ticket,
    ]

    if config.track_kind == "video" and config.resolution:
        cmd.extend(["--resolution", config.resolution])
    if config.track_kind == "audio":
        if not config.audio_layout:
            raise ValueError("audio tracks must set `track.audio_layout` in config")
        cmd.extend(["--audio-layout", config.audio_layout])
    if config.metadata_json:
        cmd.extend(["--metadata-json", str(config.metadata_json)])
    if config.ingest_latency_ms is not None:
        cmd.extend(["--ingest-latency-ms", str(config.ingest_latency_ms)])
    if config.live_edge_drift_ms is not None:
        cmd.append(f"--live-edge-drift-ms={config.live_edge_drift_ms}")
    if config.ingest_node_id:
        cmd.extend(["--ingest-node-id", config.ingest_node_id])

    return cmd


def summarise_ingest_metadata(path: Path) -> Dict[str, object]:
    data = json.loads(path.read_text())
    return {
        "manifest_hash": data.get("taikai.manifest_hash"),
        "storage_ticket": data.get("taikai.storage_ticket"),
        "event_id": data.get("taikai.event_id"),
        "stream_id": data.get("taikai.stream_id"),
        "rendition_id": data.get("taikai.rendition_id"),
        "chunk_count": data.get("taikai.chunk_count"),
        "car_size_bytes": data.get("taikai.car_size_bytes"),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the Taikai CAR sample workflow.")
    parser.add_argument(
        "--config",
        type=Path,
        action="append",
        default=None,
        help="Path to a JSON configuration file. Repeat for multiple ladder rungs.",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path(__file__).parent / "artifacts" / "sample_run",
        help="Directory for generated artefacts.",
    )
    parser.add_argument(
        "--taikai-car",
        default=os.environ.get("TAIKAI_CAR_BIN", "taikai_car"),
        help="Path to the `taikai_car` binary.",
    )
    parser.add_argument(
        "--print-command",
        action="store_true",
        help="Print the constructed `taikai_car` command before executing it.",
    )
    parser.add_argument(
        "--summary-json",
        type=Path,
        help="Optional path for a JSON summary covering every bundled segment.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    config_paths = args.config or [Path(__file__).with_name("sample_config.json")]
    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    summaries: List[Dict[str, object]] = []

    for index, path in enumerate(config_paths, start=1):
        config = load_config(path.resolve())
        outputs = default_outputs(out_dir, config.payload)
        command = build_command(args.taikai_car, config, outputs)

        if args.print_command:
            print(f"[{index}/{len(config_paths)}] " + " ".join(command))

        result = subprocess.run(command, check=False, capture_output=True, text=True)
        if result.returncode != 0:
            sys.stderr.write(result.stderr)
            raise SystemExit(result.returncode)

        summary = summarise_ingest_metadata(outputs["ingest_metadata"])
        summaries.append(
            {
                "config": str(path),
                "outputs": {key: str(value) for key, value in outputs.items()},
                "ingest": summary,
            }
        )
        print(f"Taikai sample bundle complete for {path}:")
        print(json.dumps(summary, indent=2))

    if args.summary_json:
        args.summary_json.parent.mkdir(parents=True, exist_ok=True)
        args.summary_json.write_text(json.dumps(summaries, indent=2))
        print(f"Wrote summary JSON to {args.summary_json}")


if __name__ == "__main__":
    main()
