#!/usr/bin/env python3
"""Capture iroha_monitor demo frames and export SVG snapshots.

This helper automates the monitor docs walkthrough by running the monitor in
spawn-lite mode and capturing two frames:
  * overview layout
  * focused/search view

It executes the binary via a pseudo-terminal so the TUI renders normally,
collects the ANSI stream, reduces it to a character grid, and writes SVG
artifacts under docs/source/images/.

Usage:
  python3 scripts/run_iroha_monitor_demo.py

Artefacts:
  docs/source/images/iroha_monitor_demo_overview.svg
  docs/source/images/iroha_monitor_demo_pipeline.svg
  docs/source/images/iroha_monitor_demo_overview.ans  (raw capture, debugging)
  docs/source/images/iroha_monitor_demo_pipeline.ans  (raw capture, debugging)

Requirements:
  * Python 3.10+
  * Cargo workspace already built (script does not run cargo build)
  * iroha_monitor binary available via `cargo run -p iroha_monitor -- ...`

The SVG output is intentionally monochrome and relies on monospace fonts so it
renders well in docs and PDFs without shipping raster screenshots.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pty
import select
import subprocess
import sys
import tempfile
import time
import unicodedata
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Sequence

DEFAULT_SEED = "iroha-monitor-demo"

# Default viewport that fits nicely in docs without horizontal scrolling.
# Default viewport tuned to the overview deck layout.
DEFAULT_COLS = 120
DEFAULT_ROWS = 48

# Character/line metrics for SVG rendering (monospace assumption).
CHAR_WIDTH = 7.5
LINE_HEIGHT = 15
PADDING = 12

FALLBACK_OVERVIEW = [
    "+----------------------------------------------------------------------------------------------------+",
    "| Iroha Monitor                                                                   refresh 500 ms   |",
    "| Peers 3/3 online  Healthy 3  Degraded 0  Down 0  Reported 3                                        |",
    "| Blocks 129 total / 123 non-empty  Tx 972 ok / 3 rej                                                |",
    "| Queue 3  Gas 351  Avg latency 29 ms  Refresh 500 ms                                                |",
    "| Sort health  Filter all  search off                                                                 |",
    "| Focus 祭りノード  healthy  commit 95 ms                                                              |",
    "|----------------------------------------------------------------------------------------------------|",
    "| Peers 1-3/3  sort health                                                                            |",
    "| Peer         Height   Tx        Queue   Gas   Latency   Status                                      |",
    "| 祭りノード     44       328/2     2       134   34ms      healthy commit 95 ms                      |",
    "| 祭りノード     43       324/1     1       117   29ms      healthy commit 95 ms                      |",
    "| 祭りノード     42       320/0     0       100   24ms      healthy commit 95 ms                      |",
    "|----------------------------------------------------------------------------------------------------|",
    "| Selected Peer                                                                                       |",
    "| Peer 祭りノード  healthy  (1/3)                                                                     |",
    "| Endpoint stub://peer/2/3                                                                            |",
    "| Height 44  Tx 328 / 2  Queue 2                                                                      |",
    "| Latency 34 ms  Commit 95 ms  Views 0                                                                |",
    "| Uptime 1 s  Gas 134  Fees 64                                                                         |",
    "| Note commit 95 ms                                                                                   |",
    "|----------------------------------------------------------------------------------------------------|",
    "| Alerts & Activity                                                                                   |",
    "| [OK] 祭りノード: telemetry online                                                                   |",
    "| [OK] stub://peer/1/3: telemetry online                                                              |",
    "| [OK] stub://peer/2/3: telemetry online                                                              |",
    "+----------------------------------------------------------------------------------------------------+",
]

FALLBACK_PIPELINE = [
    "+----------------------------------------------------------------------------------------------------+",
    "| Iroha Monitor                                                                   refresh 500 ms   |",
    "| Peers 3/3 online  Healthy 3  Degraded 0  Down 0  Reported 3                                        |",
    "| Blocks 129 total / 123 non-empty  Tx 972 ok / 3 rej                                                |",
    "| Queue 3  Gas 351  Avg latency 29 ms  Refresh 500 ms                                                |",
    "| Sort height  Filter all  search /2                                                                  |",
    "| Focus 祭りノード  healthy  commit 95 ms                                                              |",
    "|----------------------------------------------------------------------------------------------------|",
    "| Peers 1-1/1  sort height                                                                            |",
    "| Peer         Height   Tx        Queue   Gas   Latency   Status                                      |",
    "| 祭りノード     44       328/2     2       134   34ms      healthy commit 95 ms                      |",
    "|----------------------------------------------------------------------------------------------------|",
    "| Selected Peer                                                                                       |",
    "| Peer 祭りノード  healthy  (3/3)                                                                     |",
    "| Endpoint stub://peer/2/3                                                                            |",
    "| Height 44  Tx 328 / 2  Queue 2                                                                      |",
    "| Latency 34 ms  Commit 95 ms  Views 0                                                                |",
    "| Uptime 1 s  Gas 134  Fees 64                                                                         |",
    "| Note commit 95 ms                                                                                   |",
    "|----------------------------------------------------------------------------------------------------|",
    "| Alerts & Activity                                                                                   |",
    "| [OK] 祭りノード: telemetry online                                                                   |",
    "| [OK] stub://peer/1/3: telemetry online                                                              |",
    "| [OK] stub://peer/2/3: telemetry online                                                              |",
    "+----------------------------------------------------------------------------------------------------+",
]

FALLBACK_FRAMES = {
    "iroha_monitor_demo_overview": FALLBACK_OVERVIEW,
    "iroha_monitor_demo_pipeline": FALLBACK_PIPELINE,
}

EXPECTED_MARKERS = {
    "iroha_monitor_demo_overview": [
        "Iroha Monitor",
        "Selected Peer",
        "Alerts & Activity",
    ],
    "iroha_monitor_demo_pipeline": [
        "Iroha Monitor",
        "search /2",
        "Peers 1-1/1",
    ],
}


def normalize_capture(
    spec_name: str,
    transcript: bytes,
    lines: Sequence[str],
    ans_path: Path,
    allow_fallback: bool,
) -> tuple[list[str], bool]:
    """Validate a capture, rewriting to fallback frames when allowed."""

    try:
        validate_capture(lines, spec_name)
        ans_path.write_bytes(transcript)
        return list(lines), False
    except RuntimeError as err:
        fallback = FALLBACK_FRAMES.get(spec_name)
        if allow_fallback and fallback is not None:
            print(f"Warning: {err}; using fallback frame for {spec_name}")
            ans_text = "\n".join(fallback) + "\n"
            ans_path.write_text(ans_text, encoding="utf-8")
            return list(fallback), True
        raise


@dataclass
class Action:
    """Keys to inject at a relative timestamp (seconds)."""

    at_s: float
    keys: bytes


@dataclass
class CaptureSpec:
    """What to run and how to post-process it."""

    name: str
    actions: Sequence[Action]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Automate iroha_monitor demo captures")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("docs/source/images"),
        help="Directory where SVG and .ans dumps are written (default: docs/source/images)",
    )
    parser.add_argument(
        "--duration",
        type=float,
        default=4.0,
        help="Runtime (seconds) before sending quit; defaults to 4 seconds",
    )
    parser.add_argument(
        "--cols",
        type=int,
        default=DEFAULT_COLS,
        help=f"Pseudo-terminal column count (default: {DEFAULT_COLS})",
    )
    parser.add_argument(
        "--rows",
        type=int,
        default=DEFAULT_ROWS,
        help=f"Pseudo-terminal row count (default: {DEFAULT_ROWS})",
    )
    parser.add_argument(
        "--binary",
        default="cargo",
        help="Command used to start iroha_monitor (default runs via cargo)",
    )
    parser.add_argument(
        "--mode",
        choices=["spawn-lite", "attach", "spawn"],
        default="spawn-lite",
        help="How to launch the monitor: spawn-lite stubs, attach to peers, or spawn full nodes",
    )
    parser.add_argument(
        "--peers",
        type=int,
        default=3,
        help="Peer count for spawn-lite mode (default: 3)",
    )
    parser.add_argument(
        "--interval",
        type=int,
        default=500,
        help="Refresh interval passed to the monitor in milliseconds (default: 500)",
    )
    parser.add_argument(
        "--art-theme",
        choices=["night", "dawn", "sakura"],
        default="dawn",
        help="ASCII art theme to render during capture (default: dawn)",
    )
    parser.add_argument(
        "--art-speed",
        type=int,
        default=1,
        help="Animation speed multiplier (default: 1)",
    )
    parser.add_argument(
        "--headless-max-frames",
        type=int,
        default=24,
        help="Cap the headless fallback to N frames (default: 24)",
    )
    parser.add_argument(
        "--with-intro",
        action="store_true",
        help="Render the intro/theme instead of skipping it",
    )
    parser.add_argument(
        "--audio",
        action="store_true",
        help="Enable audio playback during capture (default: muted)",
    )
    parser.add_argument(
        "--show-gas-trend",
        action="store_true",
        help="Render the aggregate gas trend panel during captures",
    )
    parser.add_argument(
        "--attach",
        nargs="+",
        default=[],
        metavar="URL",
        help="Torii endpoints to attach to when running in attach mode",
    )
    parser.add_argument(
        "--use-alice",
        action="store_true",
        help="Forward --use-alice to the monitor when attaching",
    )
    parser.add_argument(
        "--seed",
        default=DEFAULT_SEED,
        help=f"Deterministic seed forwarded as IROHA_MONITOR_DEMO_SEED (default: {DEFAULT_SEED})",
    )
    parser.add_argument(
        "--monitor-arg",
        dest="monitor_args",
        action="append",
        default=[],
        metavar="ARG",
        help="Additional raw arguments to append to the monitor invocation",
    )
    parser.add_argument(
        "--pass-through",
        action="store_true",
        help="Stream live output to stdout while capturing (debugging)",
    )
    parser.add_argument(
        "--allow-fallback",
        action="store_true",
        help="Use baked-in fallback SVGs if capture is empty instead of failing",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        help="Optional manifest JSON output describing the capture and digests",
    )
    parser.add_argument(
        "--checksums",
        type=Path,
        help="Optional checksum JSON output describing the assets (defaults to <output-dir>/checksums.json).",
    )
    parser.add_argument(
        "--deterministic",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Pin intro/audio/theme flags for deterministic captures (default: enabled)",
    )
    return parser.parse_args()


def prepare_env(seed: str) -> dict[str, str]:
    env = os.environ.copy()
    env.setdefault("LANG", "C.UTF-8")
    env.setdefault("LC_ALL", env["LANG"])
    env.setdefault("TERM", "xterm-256color")
    env.setdefault("RUST_LOG", "warn")
    env.setdefault("PYTHONHASHSEED", "0")
    if seed:
        env["IROHA_MONITOR_DEMO_SEED"] = seed
    return env


def build_monitor_command(args: argparse.Namespace) -> tuple[list[str], list[str]]:
    if args.binary == "cargo":
        base_cmd = [
            "cargo",
            "run",
            "-p",
            "iroha_monitor",
            "--",
        ]
    else:
        base_cmd = [
            args.binary,
        ]

    monitor_args: list[str] = []
    if args.deterministic:
        if not args.with_intro:
            monitor_args.append("--no-theme")
        if not args.audio:
            monitor_args.append("--no-audio")
    if args.show_gas_trend:
        monitor_args.append("--show-gas-trend")
    monitor_args.extend(
        [
            "--art-theme",
            args.art_theme,
            "--art-speed",
            str(args.art_speed),
            "--headless-max-frames",
            str(args.headless_max_frames),
        ]
    )
    monitor_args.extend(args.monitor_args)
    if args.mode == "spawn-lite":
        monitor_args.extend(
            [
                "--spawn-lite",
                "--peers",
                str(args.peers),
                "--interval",
                str(args.interval),
            ]
        )
    elif args.mode == "attach":
        if not args.attach:
            raise SystemExit("--attach must be provided when --mode attach is selected")
        monitor_args.extend(["--attach", *args.attach, "--interval", str(args.interval)])
        if args.use_alice:
            monitor_args.append("--use-alice")
    else:
        monitor_args.extend(
            [
                "--spawn",
                "--peers",
                str(args.peers),
                "--interval",
                str(args.interval),
            ]
        )

    monitor_args = [arg for arg in monitor_args if arg]
    base_cmd.extend(monitor_args)
    return base_cmd, monitor_args


def run_capture(
    cmd: Sequence[str],
    env: dict[str, str],
    cols: int,
    rows: int,
    duration: float,
    actions: Sequence[Action],
    pass_through: bool,
) -> bytes:
    """Run command inside a PTY, inject actions, and return the full ANSI transcript."""

    master_fd, slave_fd = pty.openpty()
    try:
        # Resize PTY early so the TUI uses predictable dimensions.
        set_winsize(master_fd, rows, cols)
        proc = subprocess.Popen(
            cmd,
            stdin=slave_fd,
            stdout=slave_fd,
            stderr=slave_fd,
            env=env,
            close_fds=True,
        )
    except Exception:
        os.close(master_fd)
        os.close(slave_fd)
        raise
    finally:
        os.close(slave_fd)

    buffer = bytearray()
    start = time.monotonic()
    pending: List[Action] = sorted(actions, key=lambda action: action.at_s)
    quit_sent = False

    try:
        while True:
            now = time.monotonic() - start
            # Inject scheduled actions when their timestamps elapse.
            while pending and now >= pending[0].at_s:
                action = pending.pop(0)
                os.write(master_fd, action.keys)

            # After duration, exit gracefully if we have not already.
            if not quit_sent and now >= duration:
                os.write(master_fd, b"q")
                quit_sent = True

            # Collect output with a small timeout to avoid busy waiting.
            rlist, _, _ = select.select([master_fd], [], [], 0.05)
            if rlist:
                try:
                    chunk = os.read(master_fd, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                buffer.extend(chunk)
                if pass_through:
                    try:
                        sys.stdout.buffer.write(chunk)
                        sys.stdout.buffer.flush()
                    except BrokenPipeError:
                        pass

            # Exit when process finishes and no more data is pending.
            if proc.poll() is not None:
                # Drain any remaining bytes.
                while True:
                    rlist, _, _ = select.select([master_fd], [], [], 0.05)
                    if not rlist:
                        break
                    try:
                        chunk = os.read(master_fd, 65536)
                    except OSError:
                        chunk = b""
                    if not chunk:
                        break
                    buffer.extend(chunk)
                break
    finally:
        os.close(master_fd)
        try:
            proc.terminate()
        except Exception:
            pass
        try:
            proc.wait(timeout=1.0)
        except subprocess.TimeoutExpired:
            proc.kill()
    return bytes(buffer)


def validate_capture(lines: Sequence[str], spec_name: str) -> None:
    """Ensure capture looks like a real monitor frame."""

    markers = EXPECTED_MARKERS.get(spec_name, [])
    joined = "\n".join(lines)
    if markers:
        missing = [marker for marker in markers if marker not in joined]
        if missing:
            raise RuntimeError(
                f"capture for {spec_name} missing: {', '.join(missing)}"
            )
    elif not any(line.strip() for line in lines):
        raise RuntimeError(f"capture for {spec_name} is empty")


def set_winsize(fd: int, rows: int, cols: int) -> None:
    """Set terminal size for a PTY."""

    import fcntl
    import struct
    import termios

    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)


class Screen:
    def __init__(self, rows: int, cols: int) -> None:
        self.rows = rows
        self.cols = cols
        self.grid: List[List[str]] = [[" "] * cols for _ in range(rows)]
        self.row = 0
        self.col = 0

    def clear(self) -> None:
        self.grid = [[" "] * self.cols for _ in range(self.rows)]
        self.row = 0
        self.col = 0

    def home(self) -> None:
        self.row = 0
        self.col = 0

    def move(self, row: int, col: int) -> None:
        self.row = min(max(row, 0), self.rows - 1)
        self.col = min(max(col, 0), self.cols - 1)

    def write(self, ch: str) -> None:
        width = cell_width(ch)
        if self.row >= self.rows:
            return
        if self.col >= self.cols:
            return
        self.grid[self.row][self.col] = ch
        # Fill the continuation cells for wide glyphs with spaces.
        for offset in range(1, width):
            idx = self.col + offset
            if idx < self.cols:
                self.grid[self.row][idx] = " "
        advance = max(width, 1)
        self.col += advance
        if self.col >= self.cols:
            self.col = self.cols - 1

    def erase_line(self, mode: int) -> None:
        if mode == 0:
            for c in range(self.col, self.cols):
                self.grid[self.row][c] = " "
        elif mode == 1:
            for c in range(0, self.col + 1):
                self.grid[self.row][c] = " "
        elif mode == 2:
            for c in range(self.cols):
                self.grid[self.row][c] = " "

    def as_lines(self) -> List[str]:
        # Strip trailing spaces but keep at least one char so the line exists.
        result = []
        for row in self.grid:
            line = "".join(row).rstrip()
            result.append(line)
        return result


def cell_width(ch: str) -> int:
    if ch == "":
        return 0
    if ch == "\n" or ch == "\r":
        return 0
    if unicodedata.east_asian_width(ch) in {"F", "W"}:
        return 2
    return 1


def decode_ansi(data: bytes, rows: int, cols: int) -> List[str]:
    screen = Screen(rows, cols)
    text = data.decode("utf-8", errors="ignore")
    i = 0
    saved_pos: tuple[int, int] | None = None
    last_nonempty: List[str] = []
    header_snapshot: List[str] = []

    def snapshot_if_nonempty() -> None:
        nonlocal last_nonempty, header_snapshot
        lines = screen.as_lines()
        if any(line.strip() for line in lines):
            last_nonempty = list(lines)
            joined = "\n".join(lines)
            if "iroha monitor" in joined.casefold():
                header_snapshot = list(lines)
    while i < len(text):
        ch = text[i]
        i += 1
        if ch == "\x1b":
            if i >= len(text):
                break
            next_ch = text[i]
            i += 1
            if next_ch == "[":
                params = ""
                while i < len(text):
                    c = text[i]
                    i += 1
                    if c.isalpha() or c in "@`~":
                        command = c
                        break
                    params += c
                else:
                    continue
                params = params.strip()
                if command in {"H", "f"}:
                    if not params:
                        row = 0
                        col = 0
                    else:
                        parts = [p for p in params.split(";") if p]
                        row = int(parts[0]) - 1 if parts else 0
                        col = int(parts[1]) - 1 if len(parts) > 1 else 0
                    screen.move(row, col)
                elif command == "J":
                    # Clear screen variants: treat 2 or no param as full clear.
                    mode = int(params or "0")
                    if mode == 2:
                        snapshot_if_nonempty()
                        screen.clear()
                    elif mode == 0:
                        # Clear to end: approximate by clearing from cursor onward.
                        snapshot_if_nonempty()
                        screen.erase_line(0)
                        for r in range(screen.row + 1, screen.rows):
                            for c in range(screen.cols):
                                screen.grid[r][c] = " "
                elif command == "K":
                    snapshot_if_nonempty()
                    mode = int(params or "0")
                    screen.erase_line(mode)
                elif command == "A":
                    delta = int(params or "1")
                    screen.move(screen.row - delta, screen.col)
                elif command == "B":
                    delta = int(params or "1")
                    screen.move(screen.row + delta, screen.col)
                elif command == "C":
                    delta = int(params or "1")
                    screen.move(screen.row, screen.col + delta)
                elif command == "D":
                    delta = int(params or "1")
                    screen.move(screen.row, screen.col - delta)
                elif command == "s":
                    saved_pos = (screen.row, screen.col)
                elif command == "u":
                    if saved_pos:
                        screen.move(*saved_pos)
                else:
                    # Ignore color and other SGR/state commands.
                    pass
            elif next_ch == "]":
                # Operating System Command - skip until BEL or ST.
                while i < len(text) and text[i] not in {"\x07", "\x1b"}:
                    i += 1
                if i < len(text) and text[i] == "\x1b":
                    i += 1  # consume ESC
                    if i < len(text) and text[i] == "\\":
                        i += 1
            else:
                # Other escape sequences (e.g., \x1b) – ignore.
                continue
        elif ch == "\r":
            screen.move(screen.row, 0)
        elif ch == "\n":
            screen.move(screen.row + 1, screen.col)
            snapshot_if_nonempty()
        elif ch == "\t":
            screen.move(screen.row, screen.col + 4)
        else:
            screen.write(ch)
            snapshot_if_nonempty()

    lines = screen.as_lines()
    if any(line.strip() for line in lines):
        if "iroha monitor" in "\n".join(lines).casefold():
            return lines
        if header_snapshot:
            return header_snapshot
        return lines
    if last_nonempty:
        if "iroha monitor" in "\n".join(last_nonempty).casefold() and header_snapshot:
            return header_snapshot
        return last_nonempty
    return lines


def write_svg(lines: Sequence[str], path: Path) -> None:
    height = PADDING * 2 + LINE_HEIGHT * len(lines)
    width = PADDING * 2 + int(CHAR_WIDTH * max((len(line) for line in lines), default=0))
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        handle.write(
            f"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" "
            f"viewBox=\"0 0 {width} {height}\">\n"
        )
        handle.write("  <defs/>\n")
        handle.write("  <rect width=\"100%\" height=\"100%\" fill=\"#070312\" rx=\"6\"/>\n")
        handle.write(
            "  <g font-family=\"'JetBrains Mono','Fira Code','Menlo',monospace\" "
            "font-size=\"12\" fill=\"#f6f6f6\" xml:space=\"preserve\">\n"
        )
        y = PADDING + 2
        for line in lines:
            esc = (
                line.replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;")
            )
            handle.write(f"    <text x=\"{PADDING}\" y=\"{y}\">{esc}</text>\n")
            y += LINE_HEIGHT
        handle.write("  </g>\n</svg>\n")


def hash_file(path: Path) -> str | None:
    if not path.exists() or not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def relative_to_root(path: Path, root: Path) -> str:
    _ = root  # root reserved for future extensions
    return path.name


def write_checksums(record_path: Path, assets: Sequence[tuple[str, Path]]) -> None:
    payload = {
        "files": {
            name: hash_file(asset_path) for name, asset_path in assets if asset_path.exists()
        }
    }
    record_path.parent.mkdir(parents=True, exist_ok=True)
    record_path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")


def resolve_binary_metadata(binary_arg: str, repo_root: Path) -> dict[str, str | int | None]:
    candidates: list[Path] = []
    explicit = Path(binary_arg)
    if explicit.exists():
        candidates.append(explicit)
    default_target = Path("target/debug/iroha_monitor")
    if binary_arg == "cargo" and default_target.exists():
        candidates.append(default_target)
    path = candidates[0] if candidates else explicit
    display_path: Path = path
    for base in (repo_root, Path.cwd()):
        try:
            display_path = path.relative_to(base)
            break
        except ValueError:
            continue
    else:
        display_path = path if path.is_absolute() else Path(path.name)

    return {
        "path": str(display_path),
        "sha256": hash_file(path) if path.exists() else None,
        "size": path.stat().st_size if path.exists() else None,
    }


def write_manifest(
    path: Path,
    assets: Sequence[tuple[str, Path]],
    args: argparse.Namespace,
    monitor_args: Sequence[str],
    env: dict[str, str],
    generator_path: Path,
    binary_metadata: dict[str, str | int | None],
    repo_root: Path,
) -> None:
    entries: Dict[str, dict[str, object]] = {}
    for name, asset_path in sorted(assets, key=lambda item: item[0]):
        entries[name] = {
            "path": relative_to_root(asset_path, repo_root),
            "sha256": hash_file(asset_path),
            "size": asset_path.stat().st_size if asset_path.exists() else None,
        }

    env_snapshot = {
        key: value
        for key, value in env.items()
        if key in {"LANG", "LC_ALL", "TERM", "RUST_LOG", "IROHA_MONITOR_DEMO_SEED"}
    }

    generator_display: Path
    try:
        generator_display = generator_path.relative_to(generator_path.parent.parent)
    except ValueError:
        generator_display = generator_path

    generator_entries = {
        "scripts/iroha_monitor_demo.sh": hash_file(repo_root / "scripts" / "iroha_monitor_demo.sh"),
        str(generator_display): hash_file(generator_path),
    }

    manifest = {
        "seed": args.seed,
        "mode": args.mode,
        "peers": args.peers,
        "interval_ms": args.interval,
        "duration_s": args.duration,
        "cols": args.cols,
        "rows": args.rows,
        "deterministic": args.deterministic,
        "allow_fallback": args.allow_fallback,
        "monitor_args": list(monitor_args),
        "env": env_snapshot,
        "binary": binary_metadata,
        "generator_sha256": generator_entries,
        "assets": entries,
    }

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True), encoding="utf-8")


def main() -> int:
    args = parse_args()
    if args.art_speed < 1 or args.art_speed > 8:
        raise SystemExit("--art-speed must be between 1 and 8")
    if args.headless_max_frames < 0:
        raise SystemExit("--headless-max-frames must be non-negative")
    output_dir = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)

    env = prepare_env(args.seed)
    repo_root = Path(__file__).resolve().parent.parent
    base_cmd, monitor_args = build_monitor_command(args)
    binary_metadata = resolve_binary_metadata(args.binary, repo_root)

    captures = [
        CaptureSpec(
            name="iroha_monitor_demo_overview",
            actions=[],
        ),
        CaptureSpec(
            name="iroha_monitor_demo_pipeline",
            actions=[
                Action(at_s=0.9, keys=b"s"),
                Action(at_s=1.2, keys=b"/2\r"),
            ],
        ),
    ]

    generated: list[tuple[str, Path]] = []
    for spec in captures:
        transcript = run_capture(
            base_cmd,
            env,
            cols=args.cols,
            rows=args.rows,
            duration=args.duration,
            actions=spec.actions,
            pass_through=args.pass_through,
        )
        ans_path = output_dir / f"{spec.name}.ans"
        lines = decode_ansi(transcript, args.rows, args.cols)
        lines, _ = normalize_capture(
            spec.name,
            transcript=transcript,
            lines=lines,
            ans_path=ans_path,
            allow_fallback=args.allow_fallback,
        )
        svg_path = output_dir / f"{spec.name}.svg"
        write_svg(lines, svg_path)
        generated.append((f"{spec.name}.ans", ans_path))
        generated.append((f"{spec.name}.svg", svg_path))
        print(f"Captured {spec.name}: {svg_path} (and raw {ans_path})")

    manifest_path = args.manifest or (output_dir / "manifest.json")
    checksums_path = args.checksums or (output_dir / "checksums.json")
    if manifest_path:
        write_manifest(
            manifest_path,
            generated,
            args,
            monitor_args,
            env,
            Path(__file__).resolve(),
            binary_metadata,
            repo_root,
        )
        print(f"Wrote manifest: {manifest_path}")
    if checksums_path:
        write_checksums(checksums_path, generated)
        print(f"Wrote checksums: {checksums_path}")

    return 0


class DecodeAnsiTests(unittest.TestCase):
    def test_header_snapshot_restored_after_clear(self) -> None:
        transcript = (
            "\x1b[2J\x1b[H"
            "Iroha Monitor\n"
            "Peers 3/3 online\n"
            "\x1b[2J"
        ).encode("utf-8")

        lines = decode_ansi(transcript, rows=6, cols=80)
        joined = "\n".join(lines)
        self.assertIn("Iroha Monitor", joined)
        self.assertIn("Peers 3/3 online", joined)

    def test_header_snapshot_used_after_trailing_logs(self) -> None:
        transcript = (
            "\x1b[2J\x1b[H"
            "Iroha Monitor\n"
            "Peers 3/3 online\n"
            "\x1b[2J\x1b[H"
            "[INFO] rebuilding widgets\n"
        ).encode("utf-8")

        lines = decode_ansi(transcript, rows=6, cols=80)
        joined = "\n".join(lines)
        self.assertIn("Iroha Monitor", joined)
        self.assertNotIn("[INFO] rebuilding widgets", joined)

    def test_validate_capture_passes_for_expected_markers(self) -> None:
        lines = ["Iroha Monitor", "Selected Peer", "Alerts & Activity"]
        validate_capture(lines, "iroha_monitor_demo_overview")

    def test_validate_capture_raises_on_missing_marker(self) -> None:
        lines = ["Iroha Monitor", "Selected Peer"]
        with self.assertRaises(RuntimeError):
            validate_capture(lines, "iroha_monitor_demo_overview")

    def test_normalize_capture_writes_fallback_when_allowed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            ans_path = Path(tmpdir) / "capture.ans"
            lines, used_fallback = normalize_capture(
                "iroha_monitor_demo_pipeline",
                transcript=b"",
                lines=["missing markers"],
                ans_path=ans_path,
                allow_fallback=True,
            )
            self.assertTrue(used_fallback)
            expected = FALLBACK_FRAMES["iroha_monitor_demo_pipeline"]
            self.assertEqual(lines, expected)
            self.assertEqual(ans_path.read_text(encoding="utf-8").splitlines(), expected)

    def test_normalize_capture_preserves_transcript_on_success(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            ans_path = Path(tmpdir) / "capture_ok.ans"
            transcript = b"line1\nline2\n"
            lines, used_fallback = normalize_capture(
                "iroha_monitor_demo_overview",
                transcript=transcript,
                lines=["Iroha Monitor", "Selected Peer", "Alerts & Activity"],
                ans_path=ans_path,
                allow_fallback=False,
            )
            self.assertFalse(used_fallback)
            self.assertEqual(
                lines, ["Iroha Monitor", "Selected Peer", "Alerts & Activity"]
            )
            self.assertEqual(ans_path.read_bytes(), transcript)


class MonitorCommandTests(unittest.TestCase):
    def test_build_monitor_command_injects_deterministic_args(self) -> None:
        args = argparse.Namespace(
            binary="cargo",
            deterministic=True,
            with_intro=False,
            audio=False,
            show_gas_trend=False,
            art_theme="night",
            art_speed=1,
            headless_max_frames=7,
            monitor_args=[],
            mode="spawn-lite",
            peers=3,
            interval=500,
            attach=[],
            use_alice=False,
        )

        cmd, monitor_args = build_monitor_command(args)
        self.assertTrue(cmd[:2] == ["cargo", "run"])
        self.assertIn("--spawn-lite", monitor_args)
        self.assertIn("--no-theme", monitor_args)
        self.assertIn("--no-audio", monitor_args)
        self.assertIn("night", monitor_args)
        self.assertIn("7", monitor_args)

    def test_write_manifest_records_assets(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            asset_a = tmp / "a.ans"
            asset_b = tmp / "b.svg"
            asset_a.write_text("alpha", encoding="utf-8")
            asset_b.write_text("beta", encoding="utf-8")
            generator = tmp / "generator.py"
            generator.write_text("# generator\n", encoding="utf-8")
            manifest_path = tmp / "manifest.json"
            env = prepare_env("seed-demo")
            args = argparse.Namespace(
                seed="seed-demo",
                mode="spawn-lite",
                peers=3,
                interval=500,
                duration=4.0,
                cols=DEFAULT_COLS,
                rows=DEFAULT_ROWS,
                deterministic=True,
                allow_fallback=False,
            )
            write_manifest(
                manifest_path,
                [("a.ans", asset_a), ("b.svg", asset_b)],
                args,
                ["--spawn-lite"],
                env,
                generator,
                {"path": "dummy", "sha256": "abc", "size": 1},
                tmp,
            )

            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(manifest["assets"]["a.ans"]["sha256"], hash_file(asset_a))
            self.assertIn(hash_file(generator), manifest["generator_sha256"].values())
            self.assertEqual(manifest["env"]["IROHA_MONITOR_DEMO_SEED"], "seed-demo")

    def test_write_checksums_records_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp = Path(tmpdir)
            asset_a = tmp / "a.ans"
            asset_a.write_text("alpha", encoding="utf-8")
            record = tmp / "checksums.json"
            write_checksums(record, [("a.ans", asset_a)])
            payload = json.loads(record.read_text(encoding="utf-8"))
            self.assertEqual(payload["files"]["a.ans"], hash_file(asset_a))


if __name__ == "__main__":
    sys.exit(main())
