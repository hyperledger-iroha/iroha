#!/usr/bin/env bash
# Validate the only native Linux host admitted for first-release Taira privacy
# expectation capture. The check is intentionally non-portable: it must run
# directly on the existing AWS Graviton3 c7g.4xlarge self-hosted runner.
#
# Prerequisites: bash, curl, a C11 compiler with static libc/pthread support,
# readelf, lscpu, Python 3, procfs, AWS IMDSv2, and Linux kernel >= 6.3.
# The script creates only a temporary static syscall probe and the requested
# create-new JSON metadata file. It does not use containers or emulation.

set -Eeuo pipefail

usage() {
  cat <<'EOF'
Usage: check_taira_privacy_native_host.sh \
  --metadata-out ABSOLUTE_PATH \
  --x509-environment-out ABSOLUTE_PATH

Fail closed unless the current process is running natively on the existing
Linux ARM64 AWS Graviton3 c7g.4xlarge release-calibration runner and exposes
the kernel/resource primitives required by taira_privacy_release_runner.

Both output paths must be distinct absent files below existing canonical
directories. The environment output is the exact deterministic JSON type
consumed by the zero-pin native X.509 capture.
EOF
}

metadata_out=""
x509_environment_out=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --metadata-out)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "--metadata-out requires one non-empty value" >&2
        exit 1
      fi
      metadata_out="$2"
      shift 2
      ;;
    --x509-environment-out)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "--x509-environment-out requires one non-empty value" >&2
        exit 1
      fi
      x509_environment_out="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

validate_output_path() {
  local option_name="$1"
  local output_path="$2"
  if [[ -z "$output_path" || "$output_path" != /* ]]; then
    echo "$option_name must be an absolute path" >&2
    exit 1
  fi
  if [[ -e "$output_path" || -L "$output_path" ]]; then
    echo "$option_name must not already exist: $output_path" >&2
    exit 1
  fi
  local output_parent
  output_parent="$(dirname -- "$output_path")"
  if [[ ! -d "$output_parent" || -L "$output_parent" ]]; then
    echo "$option_name parent must be a non-symlink directory" >&2
    exit 1
  fi
  local canonical_output_parent
  canonical_output_parent="$(
    python3 -I -S -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' \
      "$output_parent"
  )"
  if [[ "$canonical_output_parent" != "$output_parent" ]]; then
    echo "$option_name parent must use its canonical physical path" >&2
    exit 1
  fi
}

validate_output_path --metadata-out "$metadata_out"
validate_output_path --x509-environment-out "$x509_environment_out"
if [[ "$metadata_out" == "$x509_environment_out" ]]; then
  echo "native host output paths must be distinct" >&2
  exit 1
fi

required_commands=(cc curl getconf ld lscpu python3 readelf uname)
for command_name in "${required_commands[@]}"; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "missing native-host prerequisite: $command_name" >&2
    exit 1
  fi
done

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "aarch64" ]]; then
  echo "Taira privacy evidence requires native little-endian Linux aarch64" >&2
  exit 1
fi
python3 -I -S - <<'PY'
import sys

if sys.byteorder != "little":
    raise SystemExit("Taira privacy evidence requires a little-endian host")
PY

if [[ -e /.dockerenv || -e /run/.containerenv ]]; then
  echo "containerized execution is forbidden for native Taira privacy evidence" >&2
  exit 1
fi
if grep -Eiq '(docker|containerd|kubepods|libpod|lxc)' /proc/1/cgroup; then
  echo "container cgroup detected; native Taira privacy evidence is forbidden" >&2
  exit 1
fi
if command -v systemd-detect-virt >/dev/null 2>&1; then
  container_kind="$(systemd-detect-virt --container 2>/dev/null || true)"
  if [[ -n "$container_kind" && "$container_kind" != "none" ]]; then
    echo "container runtime detected by systemd: $container_kind" >&2
    exit 1
  fi
fi

kernel_release="$(uname -r)"
if [[ ! "$kernel_release" =~ ^([0-9]+)\.([0-9]+)([^0-9].*)?$ ]]; then
  echo "cannot parse Linux kernel release: $kernel_release" >&2
  exit 1
fi
kernel_major="${BASH_REMATCH[1]}"
kernel_minor="${BASH_REMATCH[2]}"
if ((kernel_major < 6 || (kernel_major == 6 && kernel_minor < 3))); then
  echo "Taira privacy evidence requires Linux kernel >= 6.3" >&2
  exit 1
fi

for proc_path in /proc/self/status /proc/self/task; do
  if [[ ! -r "$proc_path" ]]; then
    echo "required procfs path is not readable: $proc_path" >&2
    exit 1
  fi
done
if [[ ! -r /proc/sys/vm/memfd_noexec ]]; then
  echo "vm.memfd_noexec is unavailable; executable memfd policy is unprovable" >&2
  exit 1
fi
memfd_noexec="$(tr -d '[:space:]' </proc/sys/vm/memfd_noexec)"
if [[ ! "$memfd_noexec" =~ ^[0-2]$ || "$memfd_noexec" == "2" ]]; then
  echo "vm.memfd_noexec forbids the mandatory explicit executable memfd" >&2
  exit 1
fi

imds_token="$(
  curl --proto '=http' --fail --silent --show-error \
    --connect-timeout 2 --max-time 5 \
    --request PUT \
    --header 'X-aws-ec2-metadata-token-ttl-seconds: 60' \
    http://169.254.169.254/latest/api/token
)"
if [[ -z "$imds_token" ]]; then
  echo "AWS IMDSv2 returned an empty token" >&2
  exit 1
fi
instance_type="$(
  curl --proto '=http' --fail --silent --show-error \
    --connect-timeout 2 --max-time 5 \
    --header "X-aws-ec2-metadata-token: $imds_token" \
    http://169.254.169.254/latest/meta-data/instance-type
)"
unset imds_token
if [[ "$instance_type" != "c7g.4xlarge" ]]; then
  echo "expected AWS c7g.4xlarge, observed: $instance_type" >&2
  exit 1
fi

probe_root="$(mktemp -d "${RUNNER_TEMP:-/tmp}/taira-privacy-host-probe.XXXXXXXXXX")"
cleanup() {
  rm -rf -- "$probe_root"
}
trap cleanup EXIT
lscpu --json >"$probe_root/lscpu.json"

read -r \
  cpu_model \
  logical_cpus \
  online_cpus \
  affinity_cpus \
  mem_total_bytes \
  mem_available_bytes \
  cgroup_memory_headroom_bytes \
  stack_hard_bytes \
  address_space_hard_bytes \
  nofile_hard_count \
  file_size_hard_bytes \
  cpu_hard_seconds \
  nproc_soft_count \
  nproc_hard_count < <(
  python3 -I -S - "$probe_root/lscpu.json" <<'PY'
import json
import os
import resource
import sys
from pathlib import Path

lscpu = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
fields = {
    item.get("field", "").rstrip(":"): item.get("data", "")
    for item in lscpu.get("lscpu", [])
    if isinstance(item, dict)
}
model = fields.get("Model name", "")
if model != "Neoverse-V1":
    raise SystemExit(f"expected Neoverse-V1 CPU model, observed {model!r}")
logical_cpus = os.sysconf("SC_NPROCESSORS_CONF")
if logical_cpus != 16:
    raise SystemExit(f"expected 16 configured logical CPUs, observed {logical_cpus!r}")
online_cpus = os.sysconf("SC_NPROCESSORS_ONLN")
if online_cpus != 16:
    raise SystemExit(f"expected 16 online logical CPUs, observed {online_cpus!r}")
affinity = os.sched_getaffinity(0)
if len(affinity) != 16:
    raise SystemExit(
        f"expected affinity to all 16 logical CPUs, observed {sorted(affinity)!r}"
    )

memory = {}
for line in Path("/proc/meminfo").read_text(encoding="ascii").splitlines():
    name, separator, raw_value = line.partition(":")
    if not separator:
        continue
    fields_value = raw_value.split()
    if len(fields_value) == 2 and fields_value[1] == "kB":
        memory[name] = int(fields_value[0]) * 1024
try:
    total = memory["MemTotal"]
    available = memory["MemAvailable"]
except KeyError as error:
    raise SystemExit(f"/proc/meminfo is missing {error.args[0]}") from error

# Canonical X509 maximum-stage bounds from
# crates/iroha_core/src/privacy_engines/zk_x509/profile.rs.
x509_peak_rss = 12 * 1024 * 1024 * 1024
x509_address_space = 32 * 1024 * 1024 * 1024
x509_stack = 8 * 1024 * 1024
x509_setup_open_files = 6
x509_child_result_bytes = 2 * 9 * 1024 * 1024 + 4 * 1024
x509_cpu_seconds = 300 * 6 + 1
x509_stage_tasks = 6
if total < x509_peak_rss:
    raise SystemExit("physical memory is below the canonical 12 GiB X509 RSS ceiling")
if available < x509_peak_rss:
    raise SystemExit("available memory is below the canonical 12 GiB X509 RSS ceiling")

cgroup_lines = Path("/proc/self/cgroup").read_text(encoding="ascii").splitlines()
if len(cgroup_lines) != 1 or not cgroup_lines[0].startswith("0::/"):
    raise SystemExit("the release runner requires one unified cgroup-v2 hierarchy")
cgroup_root = Path("/sys/fs/cgroup").resolve(strict=True)
relative_cgroup = cgroup_lines[0][3:]
cgroup_leaf = (cgroup_root / relative_cgroup.lstrip("/")).resolve(strict=True)
try:
    cgroup_leaf.relative_to(cgroup_root)
except ValueError as error:
    raise SystemExit("the current cgroup escapes the unified hierarchy") from error

def parse_cpu_set(raw: str) -> set[int]:
    cpus: set[int] = set()
    for component in raw.strip().split(","):
        if not component:
            continue
        lower_raw, separator, upper_raw = component.partition("-")
        lower = int(lower_raw)
        upper = int(upper_raw) if separator else lower
        if lower > upper:
            raise SystemExit(f"invalid cgroup CPU interval: {component!r}")
        cpus.update(range(lower, upper + 1))
    return cpus

effective_cpuset = parse_cpu_set(
    (cgroup_leaf / "cpuset.cpus.effective").read_text(encoding="ascii")
)
if effective_cpuset != set(affinity):
    raise SystemExit(
        "cgroup effective CPUs differ from the process affinity: "
        f"{sorted(effective_cpuset)!r} != {sorted(affinity)!r}"
    )

memory_headrooms: list[int] = []
current = cgroup_leaf
while True:
    cpu_max = (current / "cpu.max").read_text(encoding="ascii").strip().split()
    if len(cpu_max) != 2 or cpu_max[0] != "max" or int(cpu_max[1]) <= 0:
        raise SystemExit(f"cgroup CPU quota is not unlimited at {current}")
    memory_max = (current / "memory.max").read_text(encoding="ascii").strip()
    if memory_max != "max":
        maximum = int(memory_max)
        usage = int((current / "memory.current").read_text(encoding="ascii"))
        memory_headrooms.append(maximum - usage)
    if current == cgroup_root:
        break
    current = current.parent
if memory_headrooms and min(memory_headrooms) < x509_peak_rss:
    raise SystemExit(
        "cgroup memory headroom is below the canonical 12 GiB X509 RSS ceiling"
    )

stack_hard = resource.getrlimit(resource.RLIMIT_STACK)[1]
address_hard = resource.getrlimit(resource.RLIMIT_AS)[1]
nofile_hard = resource.getrlimit(resource.RLIMIT_NOFILE)[1]
file_size_hard = resource.getrlimit(resource.RLIMIT_FSIZE)[1]
cpu_hard = resource.getrlimit(resource.RLIMIT_CPU)[1]
nproc_soft, nproc_hard = resource.getrlimit(resource.RLIMIT_NPROC)
if stack_hard != resource.RLIM_INFINITY and stack_hard < x509_stack:
    raise SystemExit("hard stack limit is below the canonical 8 MiB stage stack")
if address_hard != resource.RLIM_INFINITY and address_hard < x509_address_space:
    raise SystemExit("hard address-space limit is below the canonical 32 GiB X509 limit")
if nofile_hard != resource.RLIM_INFINITY and nofile_hard < x509_setup_open_files:
    raise SystemExit("hard open-file limit is below the canonical six setup descriptors")
if file_size_hard != resource.RLIM_INFINITY and file_size_hard < x509_child_result_bytes:
    raise SystemExit("hard file-size limit is below the maximum stage result")
if cpu_hard != resource.RLIM_INFINITY and cpu_hard < x509_cpu_seconds:
    raise SystemExit("hard CPU limit is below the canonical X509 process allowance")
if nproc_soft != resource.RLIM_INFINITY and nproc_soft < x509_stage_tasks:
    raise SystemExit("soft process limit is below the canonical six stage tasks")
if nproc_hard != resource.RLIM_INFINITY and nproc_hard < x509_stage_tasks:
    raise SystemExit("hard process limit is below the canonical six stage tasks")

print(
    model,
    logical_cpus,
    online_cpus,
    len(affinity),
    total,
    available,
    min(memory_headrooms) if memory_headrooms else "unlimited",
    "unlimited" if stack_hard == resource.RLIM_INFINITY else stack_hard,
    "unlimited" if address_hard == resource.RLIM_INFINITY else address_hard,
    "unlimited" if nofile_hard == resource.RLIM_INFINITY else nofile_hard,
    "unlimited" if file_size_hard == resource.RLIM_INFINITY else file_size_hard,
    "unlimited" if cpu_hard == resource.RLIM_INFINITY else cpu_hard,
    "unlimited" if nproc_soft == resource.RLIM_INFINITY else nproc_soft,
    "unlimited" if nproc_hard == resource.RLIM_INFINITY else nproc_hard,
)
PY
)

cc -std=c11 -O2 -Wall -Wextra -Werror -static -pthread \
  ci/taira_privacy_native_host_probe.c \
  -o "$probe_root/native_primitives"
if readelf --program-headers --wide "$probe_root/native_primitives" \
  | grep -E '(^|[[:space:]])INTERP([[:space:]]|$)' >/dev/null; then
  echo "native prerequisite probe unexpectedly contains PT_INTERP" >&2
  exit 1
fi
if readelf --dynamic --wide "$probe_root/native_primitives" \
  | grep -E '\(NEEDED\)' >/dev/null; then
  echo "native prerequisite probe unexpectedly contains DT_NEEDED" >&2
  exit 1
fi
if readelf --program-headers --wide "$probe_root/native_primitives" \
  | grep -E \
    '(^|[[:space:]])(LOAD|GNU_STACK).*(RWE|R[[:space:]]+W[[:space:]]+E)' \
    >/dev/null; then
  echo "native prerequisite probe contains a writable executable segment" >&2
  exit 1
fi
landlock_abi="$("$probe_root/native_primitives")"
if [[ ! "$landlock_abi" =~ ^[0-9]+$ ]] || ((landlock_abi < 3)); then
  echo "native prerequisite probe returned invalid Landlock ABI: $landlock_abi" >&2
  exit 1
fi

RUNNER_NAME_VALUE="${RUNNER_NAME:-}" \
RUNNER_OS_VALUE="${RUNNER_OS:-}" \
RUNNER_ARCH_VALUE="${RUNNER_ARCH:-}" \
python3 -I -S - \
  "$metadata_out" \
  "$x509_environment_out" \
  "$instance_type" \
  "$cpu_model" \
  "$logical_cpus" \
  "$online_cpus" \
  "$affinity_cpus" \
  "$kernel_release" \
  "$landlock_abi" \
  "$memfd_noexec" \
  "$mem_total_bytes" \
  "$mem_available_bytes" \
  "$cgroup_memory_headroom_bytes" \
  "$stack_hard_bytes" \
  "$address_space_hard_bytes" \
  "$nofile_hard_count" \
  "$file_size_hard_bytes" \
  "$cpu_hard_seconds" \
  "$nproc_soft_count" \
  "$nproc_hard_count" <<'PY'
import json
import os
from pathlib import Path
import sys

(
    output,
    x509_environment_output,
    instance_type,
    cpu_model,
    logical_cpus,
    online_cpus,
    affinity_cpus,
    kernel_release,
    landlock_abi,
    memfd_noexec,
    mem_total,
    mem_available,
    cgroup_memory_headroom,
    stack_hard,
    address_space_hard,
    nofile_hard,
    file_size_hard,
    cpu_hard,
    nproc_soft,
    nproc_hard,
) = sys.argv[1:]
runner_os = os.environ["RUNNER_OS_VALUE"]
runner_arch = os.environ["RUNNER_ARCH_VALUE"]
if runner_os != "Linux" or runner_arch != "ARM64":
    raise SystemExit(
        f"GitHub runner identity must be Linux/ARM64, observed {runner_os!r}/{runner_arch!r}"
    )
payload = {
    "schema": "iroha.taira.privacy-native-host.v1",
    "native_execution": True,
    "containerized": False,
    "architecture": "aarch64",
    "byte_order": "little",
    "instance_type": instance_type,
    "cpu_model": cpu_model,
    "logical_cpu_count": int(logical_cpus),
    "online_cpu_count": int(online_cpus),
    "affinity_cpu_count": int(affinity_cpus),
    "kernel_release": kernel_release,
    "landlock_abi": int(landlock_abi),
    "landlock_restrict_self_probe_passed": True,
    "openat2_probe_passed": True,
    "memfd_exec_seal_probe_passed": True,
    "seccomp_tsync_probe_passed": True,
    "memfd_noexec": int(memfd_noexec),
    "mem_total_bytes": int(mem_total),
    "mem_available_bytes": int(mem_available),
    "cgroup_v2": True,
    "cgroup_cpu_quota_unlimited": True,
    "cgroup_memory_headroom_bytes": (
        cgroup_memory_headroom
        if cgroup_memory_headroom == "unlimited"
        else int(cgroup_memory_headroom)
    ),
    "stack_hard_bytes": stack_hard if stack_hard == "unlimited" else int(stack_hard),
    "address_space_hard_bytes": (
        address_space_hard
        if address_space_hard == "unlimited"
        else int(address_space_hard)
    ),
    "nofile_hard_count": (
        nofile_hard if nofile_hard == "unlimited" else int(nofile_hard)
    ),
    "file_size_hard_bytes": (
        file_size_hard if file_size_hard == "unlimited" else int(file_size_hard)
    ),
    "cpu_hard_seconds": (
        cpu_hard if cpu_hard == "unlimited" else int(cpu_hard)
    ),
    "nproc_soft_count": (
        nproc_soft if nproc_soft == "unlimited" else int(nproc_soft)
    ),
    "nproc_hard_count": (
        nproc_hard if nproc_hard == "unlimited" else int(nproc_hard)
    ),
    "runner_name": os.environ["RUNNER_NAME_VALUE"],
    "runner_os": runner_os,
    "runner_arch": runner_arch,
}
environment = {
    "operating_system": "linux",
    "architecture": "aarch64",
    "endianness": "little",
    "kernel_minimum_major": 6,
    "kernel_minimum_minor": 3,
    "rustc_release": "1.93.1",
    "rustc_host": "aarch64-unknown-linux-gnu",
    "rustc_commit_hash": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
    "rustc_commit_date": "2026-02-11",
    "instance_type": instance_type,
    "cpu_model": cpu_model,
    "logical_cpu_count": int(logical_cpus),
    "online_cpu_count": int(online_cpus),
    "affinity_cpu_count": int(affinity_cpus),
}

def write_create_new(path: Path, value: dict[str, object]) -> None:
    encoded = (json.dumps(value, indent=2) + "\n").encode("utf-8")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise

created = []
try:
    environment_path = Path(x509_environment_output)
    write_create_new(environment_path, environment)
    created.append(environment_path)
    metadata_path = Path(output)
    write_create_new(metadata_path, payload)
    created.append(metadata_path)
except BaseException:
    for path in reversed(created):
        path.unlink(missing_ok=True)
    raise
PY

echo "Taira native privacy host prerequisites verified: ${instance_type}, ${cpu_model}, kernel ${kernel_release}, Landlock ABI ${landlock_abi}"
