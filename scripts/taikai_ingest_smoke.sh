#!/usr/bin/env bash
set -euo pipefail

abs_path() {
  local input="$1"
  if [[ "$input" = /* ]]; then
    printf '%s\n' "$input"
  else
    local dir
    dir="$(cd "$(dirname "$input")" && pwd)"
    printf '%s/%s\n' "$dir" "$(basename "$input")"
  fi
}

usage() {
  cat <<'USAGE'
taikai_ingest_smoke.sh [--workspace PATH] [--fixtures DIR] [--out DIR] [--taikai-car PATH]

Runs the Taikai CAR bundler against the deterministic fixtures under
`fixtures/taikai/segments/*.json` and validates the generated CAR, envelope,
index, and ingest-metadata artefacts. The helper is used to satisfy SN13-A's
“ingest smoke-test harness” requirement and is safe to run in CI.

Options:
  --workspace PATH    Repository root (defaults to script/..).
  --fixtures DIR      Directory containing fixture JSON files
                      (default: <workspace>/fixtures/taikai/segments).
  --out DIR           Output directory for smoke artefacts
                      (default: <workspace>/artifacts/taikai/ingest_smoke).
  --taikai-car PATH   Path to the `taikai_car` binary. When omitted the helper
                      builds/uses <workspace>/target/debug/taikai_car.
  --help              Show this message.

Environment overrides:
  TAIKAI_FIXTURES   Same as --fixtures.
  TAIKAI_SMOKE_OUT  Same as --out.
  TAIKAI_CAR_BIN    Same as --taikai-car.
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="${WORKSPACE:-$SCRIPT_DIR/..}"
FIXTURES_OVERRIDE="${TAIKAI_FIXTURES:-}"
OUT_OVERRIDE="${TAIKAI_SMOKE_OUT:-}"
CAR_OVERRIDE="${TAIKAI_CAR_BIN:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace)
      [[ $# -lt 2 ]] && { echo "error: --workspace requires a path" >&2; exit 1; }
      WORKSPACE="$2"
      shift 2
      ;;
    --fixtures)
      [[ $# -lt 2 ]] && { echo "error: --fixtures requires a path" >&2; exit 1; }
      FIXTURES_OVERRIDE="$2"
      shift 2
      ;;
    --out)
      [[ $# -lt 2 ]] && { echo "error: --out requires a path" >&2; exit 1; }
      OUT_OVERRIDE="$2"
      shift 2
      ;;
    --taikai-car)
      [[ $# -lt 2 ]] && { echo "error: --taikai-car requires a path" >&2; exit 1; }
      CAR_OVERRIDE="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage
      exit 1
      ;;
  esac
done

WORKSPACE="$(abs_path "$WORKSPACE")"
FIXTURE_DIR="$(abs_path "${FIXTURES_OVERRIDE:-$WORKSPACE/fixtures/taikai/segments}")"
OUT_DIR="$(abs_path "${OUT_OVERRIDE:-$WORKSPACE/artifacts/taikai/ingest_smoke}")"
TAIKAI_CAR_BIN="$(abs_path "${CAR_OVERRIDE:-$WORKSPACE/target/debug/taikai_car}")"

if ! command -v python3 >/dev/null 2>&1; then
  echo "error: python3 is required for fixture parsing" >&2
  exit 1
fi

ensure_taikai_car() {
  if [[ -x "$TAIKAI_CAR_BIN" ]]; then
    return
  fi
  echo "building taikai_car under $WORKSPACE..."
  (cd "$WORKSPACE" && cargo build -p sorafs_car --bin taikai_car --features cli)
  if [[ ! -x "$TAIKAI_CAR_BIN" ]]; then
    echo "error: taikai_car binary not found at $TAIKAI_CAR_BIN after build" >&2
    exit 1
  fi
}

ensure_taikai_car

if [[ ! -d "$FIXTURE_DIR" ]]; then
  echo "error: fixture directory not found at $FIXTURE_DIR" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

shopt -s nullglob
fixtures=("$FIXTURE_DIR"/*.json)
shopt -u nullglob

if [[ ${#fixtures[@]} -eq 0 ]]; then
  echo "error: no fixture JSON files found in $FIXTURE_DIR" >&2
  exit 1
fi

echo "Running Taikai ingest smoke harness with $(basename "$TAIKAI_CAR_BIN")"
echo "Fixtures: $FIXTURE_DIR"
echo "Output:   $OUT_DIR"

for fixture in "${fixtures[@]}"; do
  label="$(python3 - <<'PY' "$fixture"
import json, sys, pathlib
path = pathlib.Path(sys.argv[1])
data = json.loads(path.read_text())
label = data.get("label") or path.stem
print(label)
PY
)"
  run_dir="$OUT_DIR/$label"
  rm -rf "$run_dir"
  mkdir -p "$run_dir"
  payload_path="$run_dir/payload.bin"
  args_file="$run_dir/cli_args.txt"
  stdout_path="$run_dir/bundle_stdout.txt"
  car_out="$run_dir/segment.car"
  envelope_out="$run_dir/segment.norito"
  indexes_out="$run_dir/segment.indexes.json"
  ingest_out="$run_dir/segment.ingest.json"

  python3 - <<'PY' "$fixture" "$payload_path" "$args_file" "$car_out" "$envelope_out" "$indexes_out" "$ingest_out"
import json, sys, pathlib

fixture_path = pathlib.Path(sys.argv[1])
payload_path = pathlib.Path(sys.argv[2])
args_path = pathlib.Path(sys.argv[3])
car_out = pathlib.Path(sys.argv[4])
envelope_out = pathlib.Path(sys.argv[5])
indexes_out = pathlib.Path(sys.argv[6])
ingest_out = pathlib.Path(sys.argv[7])

data = json.loads(fixture_path.read_text())
payload_hex = data["payload_hex"]
payload_path.write_bytes(bytes.fromhex(payload_hex))

args_cfg = data["args"]
cli = [
    "--payload", str(payload_path),
    "--car-out", str(car_out),
    "--envelope-out", str(envelope_out),
    "--indexes-out", str(indexes_out),
    "--ingest-metadata-out", str(ingest_out),
    "--event-id", args_cfg["event_id"],
    "--stream-id", args_cfg["stream_id"],
    "--rendition-id", args_cfg["rendition_id"],
    "--track-kind", args_cfg["track_kind"],
    "--codec", args_cfg["codec"],
    "--bitrate-kbps", str(args_cfg["bitrate_kbps"]),
    "--segment-sequence", str(args_cfg["segment_sequence"]),
    "--segment-start-pts", str(args_cfg["segment_start_pts"]),
    "--segment-duration", str(args_cfg["segment_duration"]),
    "--wallclock-unix-ms", str(args_cfg["wallclock_unix_ms"]),
    "--manifest-hash", args_cfg["manifest_hash"],
    "--storage-ticket", args_cfg["storage_ticket"],
]

if args_cfg["track_kind"] == "video":
    resolution = args_cfg.get("resolution")
    if not resolution:
        raise SystemExit("video fixtures must provide args.resolution")
    cli.extend(["--resolution", resolution])
elif args_cfg["track_kind"] == "audio":
    layout = args_cfg.get("audio_layout")
    if not layout:
        raise SystemExit("audio fixtures must provide args.audio_layout")
    cli.extend(["--audio-layout", layout])

if args_cfg.get("ingest_latency_ms") is not None:
    cli.extend(["--ingest-latency-ms", str(args_cfg["ingest_latency_ms"])])
if args_cfg.get("live_edge_drift_ms") is not None:
    cli.append(f"--live-edge-drift-ms={args_cfg['live_edge_drift_ms']}")
if args_cfg.get("ingest_node_id"):
    cli.extend(["--ingest-node-id", args_cfg["ingest_node_id"]])
if args_cfg.get("extra_metadata"):
    metadata_path = pathlib.Path(args_cfg["extra_metadata"])
    if not metadata_path.is_absolute():
        metadata_path = (fixture_path.parent / metadata_path).resolve()
    cli.extend(["--metadata-json", str(metadata_path)])

args_path.write_text("\n".join(cli))
PY

  cli_args=()
  while IFS= read -r arg || [[ -n "$arg" ]]; do
    [[ -z "$arg" ]] && continue
    cli_args+=("$arg")
  done < "$args_file"
  echo "→ Bundling fixture ${label}"
  if ! bundle_output="$("$TAIKAI_CAR_BIN" "${cli_args[@]}" 2>&1)"; then
    printf '%s\n' "$bundle_output" | tee "$stdout_path"
    echo "error: taikai_car failed for fixture $label" >&2
    exit 1
  fi
  printf '%s\n' "$bundle_output" | tee "$stdout_path" >/dev/null

  python3 - <<'PY' "$fixture" "$run_dir" "$stdout_path"
import json, sys, pathlib, re

fixture_path = pathlib.Path(sys.argv[1])
run_dir = pathlib.Path(sys.argv[2])
stdout_path = pathlib.Path(sys.argv[3])

fixture = json.loads(fixture_path.read_text())
expected = fixture.get("expected", {})
stdout_text = stdout_path.read_text()

def parse_stdout(text: str) -> dict:
    values = {}
    for raw in text.splitlines():
        line = raw.strip()
        if ":" not in line:
            continue
        key, val = line.split(":", 1)
        normalized = key.lower().strip()
        value = val.strip()
        if normalized.startswith("car_cid"):
            values["cid_multibase"] = value
        elif normalized.startswith("car_digest"):
            values["car_digest_hex"] = value.lower()
        elif normalized.startswith("car_size_bytes"):
            values["car_size_bytes"] = int(value)
        elif normalized.startswith("chunk_root"):
            values["chunk_root_hex"] = value.lower()
        elif normalized.startswith("chunk_count"):
            values["chunk_count"] = int(value)
    return values

stdout_metrics = parse_stdout(stdout_text)
errors = []

def expect_equal(key: str, actual, expected_value):
    if actual != expected_value:
        errors.append(f"{key} mismatch (expected {expected_value}, found {actual})")

if "car_digest_hex" in expected:
    expect_equal("car_digest_hex", stdout_metrics.get("car_digest_hex"), expected["car_digest_hex"].lower())
if "car_size_bytes" in expected:
    expect_equal("car_size_bytes", stdout_metrics.get("car_size_bytes"), expected["car_size_bytes"])
if "chunk_root_hex" in expected:
    expect_equal("chunk_root_hex", stdout_metrics.get("chunk_root_hex"), expected["chunk_root_hex"].lower())
if "chunk_count" in expected:
    expect_equal("chunk_count", stdout_metrics.get("chunk_count"), expected["chunk_count"])
if "cid_multibase" in expected:
    expect_equal("cid_multibase (stdout)", stdout_metrics.get("cid_multibase"), expected["cid_multibase"])

indexes_path = run_dir / "segment.indexes.json"
if indexes_path.exists():
    indexes = json.loads(indexes_path.read_text())
    cid_actual = indexes.get("cid_key", {}).get("cid_multibase")
    if "cid_multibase" in expected:
        expect_equal("cid_multibase (indexes)", cid_actual, expected["cid_multibase"])
    time_pts = indexes.get("time_key", {}).get("segment_start_pts")
    expected_pts = expected.get("indexes", {}).get("segment_start_pts")
    if expected_pts is not None:
        if not isinstance(time_pts, list) or not time_pts:
            errors.append("segment_start_pts missing from indexes")
        else:
            expect_equal("segment_start_pts", time_pts[0], expected_pts)
else:
    errors.append("segment.indexes.json missing")

ingest_path = run_dir / "segment.ingest.json"
if ingest_path.exists():
    ingest = json.loads(ingest_path.read_text())
    for key, expected_value in (expected.get("ingest_fields") or {}).items():
        actual = ingest.get(key)
        expect_equal(f"ingest field {key}", actual, expected_value)
else:
    errors.append("segment.ingest.json missing")

if errors:
    for err in errors:
        print(f"[fixture {fixture.get('label') or fixture_path.stem}] {err}", file=sys.stderr)
    raise SystemExit(1)
PY

  echo "✓ fixture ${label} passed"
done

echo "All Taikai ingest fixtures passed."
