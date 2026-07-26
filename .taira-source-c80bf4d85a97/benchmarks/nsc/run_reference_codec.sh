#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: run_reference_codec.sh --codec <libaom|svt-av1|dav1d> --clip-set <name>
       --clips <dir> --csv <path> --tables <path> --output <dir> --bitstreams <dir>

Encodes or decodes the reference clip set using the specified codec. The script
expects the SignedRansTablesV1 CSV export to be generated ahead of time (via
benchmarks/nsc/rans_compare.py) but does not currently feed the tables to the
reference encoders; this hook exists so future libaom patches can ingest the CSV.

Environment overrides:
  AOMENC_BIN   - path to the libaom `aomenc` binary (default: aomenc in PATH)
  SVTAV1_BIN   - path to `SvtAv1EncApp` (default: SvtAv1EncApp in PATH)
  DAV1D_BIN    - path to `dav1d` (default: dav1d in PATH)
EOF
}

codec=""
clip_set=""
clips_dir=""
csv_path=""
tables_path=""
output_dir=""
bitstreams_dir=""

while (($#)); do
    case "$1" in
        --codec) codec=${2:?}; shift 2 ;;
        --clip-set) clip_set=${2:?}; shift 2 ;;
        --clips) clips_dir=${2:?}; shift 2 ;;
        --csv) csv_path=${2:?}; shift 2 ;;
        --tables) tables_path=${2:?}; shift 2 ;;
        --output) output_dir=${2:?}; shift 2 ;;
        --bitstreams) bitstreams_dir=${2:?}; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage >&2; exit 1 ;;
    esac
done

if [[ -z "$codec" || -z "$clip_set" || -z "$clips_dir" || -z "$csv_path" || -z "$tables_path" || -z "$output_dir" || -z "$bitstreams_dir" ]]; then
    echo "missing required arguments" >&2
    usage >&2
    exit 1
fi

manifest="${RANS_CLIP_MANIFEST:-$(cd -- \"$(dirname \"$0\")\" && pwd)/reference_clips.json}"
if [[ ! -f "$manifest" ]]; then
    echo "clip manifest not found: $manifest" >&2
    exit 1
fi

# macOS ships an older bash without readarray/mapfile, so rely on Python to emit newline-separated paths
clip_lines=$(python3 - "$manifest" "$clip_set" "$clips_dir" <<'PY'
import json, sys
from pathlib import Path
manifest = Path(sys.argv[1])
clip_set = sys.argv[2]
root = Path(sys.argv[3])
data = json.loads(manifest.read_text())
entries = data.get("clip_sets", {}).get(clip_set, [])
if not entries:
    raise SystemExit(f"clip set '{clip_set}' not found in {manifest}")
for entry in entries:
    path = root / entry["path"]
    print(path)
PY
)
if [[ -z "$clip_lines" ]]; then
    echo "no clips resolved for set ${clip_set} in ${manifest}" >&2
    exit 1
fi
clips=()
while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    clips+=("$line")
done <<<"${clip_lines}"

mkdir -p "$output_dir" "$bitstreams_dir"

log() { printf '[reference-%s] %s\n' "$codec" "$*"; }

case "$codec" in
    libaom)
        aomenc=${AOMENC_BIN:-aomenc}
        if ! command -v "$aomenc" >/dev/null 2>&1; then
            echo "aomenc binary not found; set AOMENC_BIN" >&2
            exit 127
        fi
        for clip in "${clips[@]}"; do
            base=$(basename "${clip%.*}")
            out="$bitstreams_dir/${base}.libaom.ivf"
            log "encoding ${clip} -> ${out}"
            "$aomenc" \
                --threads=1 \
                --cpu-used=6 \
                --fps=30/1 \
                --passes=1 \
                --end-usage=q \
                --cq-level=32 \
                -o "$out" \
                "$clip" >/dev/null
        done
        ;;
    svt-av1)
        svt=${SVTAV1_BIN:-SvtAv1EncApp}
        if ! command -v "$svt" >/dev/null 2>&1; then
            echo "SvtAv1EncApp binary not found; set SVTAV1_BIN" >&2
            exit 127
        fi
        svt_frames=${SVT_FRAMES:-4}
        svt_min_dim=${SVT_MIN_DIMENSION:-32}
        for clip in "${clips[@]}"; do
            base=$(basename "${clip%.*}")
            out="$bitstreams_dir/${base}.svt-av1.ivf"
            log "encoding ${clip} -> ${out}"
            width=$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$clip")
            height=$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "$clip")
            if [[ -z "$width" || -z "$height" ]]; then
                echo "[reference-svt-av1] failed to probe dimensions for ${clip}" >&2
                continue
            fi
            if (( width < svt_min_dim || height < svt_min_dim )); then
                echo "[reference-svt-av1] skipping ${clip} (dimensions ${width}x${height} below SVT_MIN_DIMENSION=${svt_min_dim})" >&2
                continue
            fi
            cmd=( "$svt"
                --preset 6 \
                --input "$clip" \
                --fps 30 \
                --rc 0 \
                --qp 33 \
                --irefresh-type 2 \
                --enable-qm 0 \
                --intra-period 16 \
                --output "$out" )
            if [[ "${svt_frames}" -gt 0 ]]; then
                cmd+=( --frames "${svt_frames}" )
            fi
            "${cmd[@]}" >/dev/null
        done
        ;;
    dav1d)
        dav=${DAV1D_BIN:-dav1d}
        if ! command -v "$dav" >/dev/null 2>&1; then
            echo "dav1d binary not found; set DAV1D_BIN" >&2
            exit 127
        fi
        shopt -s nullglob
        bitstreams=("$bitstreams_dir"/*.ivf)
        if ((${#bitstreams[@]} == 0)); then
            echo "no .ivf files found under $bitstreams_dir" >&2
            exit 1
        fi
        for bitstream in "${bitstreams[@]}"; do
            base=$(basename "${bitstream%.*}")
            out="$output_dir/${base}.yuv"
            log "decoding ${bitstream} -> ${out}"
            "$dav" -i "$bitstream" -o "$out" >/dev/null
        done
        ;;
    *)
        echo "unsupported codec: $codec" >&2
        exit 1
        ;;
esac

log "clips processed using CSV ${csv_path} and tables ${tables_path}"
