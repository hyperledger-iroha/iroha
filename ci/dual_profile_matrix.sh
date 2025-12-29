#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ci/dual_profile_matrix.sh --output <path> <bundle.tar.zst> [bundle.tar.zst ...]

Inspect dual-track release bundles, verify required files/binaries, and emit a
JSON matrix summarising the metadata and validation checks. The resulting file
(`dual_profile_matrix.json`) is attached to the release approval ticket
alongside manifests and network profile diffs.

Options:
  --expect-version <ver>   Fail if PROFILE.toml does not report the expected version.
EOF
}

log() {
    printf '[dual-matrix] %s\n' "$*" >&2
}

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf '[dual-matrix] missing required command: %s\n' "$1" >&2
        exit 1
    fi
}

output=""
expected_version=""
declare -a bundles=()

while (($#)); do
    case "$1" in
        -o|--output)
            output="${2:-}"
            shift 2
            ;;
        --expect-version)
            expected_version="${2:-}"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            bundles+=("$@")
            break
            ;;
        -*)
            printf '[dual-matrix] unknown option: %s\n' "$1" >&2
            usage >&2
            exit 1
            ;;
        *)
            bundles+=("$1")
            shift
            ;;
    esac
done

if [[ -z "$output" || ${#bundles[@]} -eq 0 ]]; then
    usage >&2
    exit 1
fi

require_cmd tar
require_cmd zstd
require_cmd python3

tmp_root="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_root"
}
trap cleanup EXIT

entries_file="$(mktemp)"
bundle_count=0

abs_path() {
    python3 - "$1" <<'PY'
import os, sys
print(os.path.abspath(sys.argv[1]))
PY
}

for bundle in "${bundles[@]}"; do
    if [[ ! -f "$bundle" ]]; then
        printf '[dual-matrix] bundle not found: %s\n' "$bundle" >&2
        exit 1
    fi
    bundle_path="$(abs_path "$bundle")"
    workdir="$(mktemp -d "${tmp_root}/bundle.XXXXXX")"
    if ! zstd --long=31 -dc "$bundle_path" | tar -C "$workdir" -xf -; then
        printf '[dual-matrix] failed to extract %s\n' "$bundle_path" >&2
        exit 1
    fi
    profile_dir="$(find "$workdir" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
    if [[ -z "$profile_dir" ]]; then
        printf '[dual-matrix] unable to locate profile directory in %s\n' "$bundle_path" >&2
        exit 1
    fi
    profile_json="$(python3 - "$bundle_path" "$profile_dir" "$expected_version" <<'PY'
import json
import hashlib
import subprocess
import sys
from pathlib import Path

try:
    import tomllib  # Python 3.11+
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

bundle = Path(sys.argv[1]).resolve()
profile_dir = Path(sys.argv[2]).resolve()
expected_version = sys.argv[3] if len(sys.argv) > 3 else ""
def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()

profile_file = profile_dir / "PROFILE.toml"
if not profile_file.exists():
    raise SystemExit(f"PROFILE.toml missing in bundle {bundle}")
with profile_file.open("rb") as fh:
    meta = tomllib.load(fh)
if expected_version and meta.get("version") != expected_version:
    raise SystemExit(
        f"bundle {bundle} has version {meta.get('version')} but {expected_version} was requested"
    )
entry = {
    "bundle": str(bundle),
    "bundle_size": bundle.stat().st_size,
    "bundle_sha256": sha256_file(bundle),
    "profile_dir": str(profile_dir),
    "profile": meta.get("profile"),
    "config": meta.get("config"),
    "version": meta.get("version"),
    "commit": meta.get("commit"),
    "built_at": meta.get("built_at"),
    "os": meta.get("os"),
    "arch": meta.get("arch"),
    "features": meta.get("features"),
}

executables = []
commands = {
    "irohad": ["--version"],
    "iroha": ["--help"],
    "kagami": ["--help"],
}
missing_required = []
for name, args in commands.items():
    binary = profile_dir / "bin" / name
    info = {"name": name, "path": str(binary)}
    if not binary.exists():
        info["missing"] = True
        if name in ("irohad", "kagami"):
            missing_required.append(name)
        executables.append(info)
        continue
    info["size"] = binary.stat().st_size
    info["sha256"] = sha256_file(binary)
    cmd = [str(binary), *args]
    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        check=True,
        timeout=15,
    )
    combined = (result.stdout.strip() or result.stderr.strip()).splitlines()
    info["command"] = " ".join([binary.name, *args])
    info["command_output"] = combined[:3]
    executables.append(info)

if missing_required:
    raise SystemExit(f"missing required binaries in {bundle}: {', '.join(missing_required)}")

entry["executables"] = executables

config_dir = profile_dir / "config"
if not config_dir.exists():
    raise SystemExit(f"config directory missing in bundle {bundle}")

config_entries = []
for path in sorted(config_dir.rglob("*")):
    if path.is_file():
        config_entries.append(
            {
                "path": str(path.relative_to(profile_dir)),
                "sha256": sha256_file(path),
                "size": path.stat().st_size,
            }
        )
entry["config_files"] = config_entries

required_configs = ["config/genesis.json", "config/client.toml"]
if meta.get("config") == "nexus":
    required_configs.append("config/config.toml")

missing_configs = [cfg for cfg in required_configs if not (profile_dir / cfg).exists()]
if missing_configs:
    raise SystemExit(
        f"missing required config files in {bundle}: {', '.join(missing_configs)}"
    )
entry["required_config_files"] = required_configs

print(json.dumps(entry, separators=(",", ":")))
PY
)"
    printf '%s\n' "$profile_json" >>"$entries_file"
    bundle_count=$((bundle_count + 1))
done

python3 - "$entries_file" "$output" <<'PY'
import datetime
import json
import sys
from pathlib import Path

entries_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])
entries = []
for line in entries_path.read_text().splitlines():
    line = line.strip()
    if line:
        entries.append(json.loads(line))

payload = {
    "generated_at": datetime.datetime.utcnow().replace(microsecond=0).isoformat() + "Z",
    "entries": entries,
}
output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text(json.dumps(payload, indent=2) + "\n")
PY

log "wrote $(abs_path "$output") with ${bundle_count} entr$(test "$bundle_count" = 1 && printf 'y' || printf 'ies')"
