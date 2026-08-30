#!/usr/bin/env bash
# Validate the only native Linux host admitted for first-release Taira privacy
# expectation capture. The check is intentionally non-portable: it must run
# directly on the existing AWS Graviton3 c7g.4xlarge self-hosted runner.
# It also proves that the invoking identity can create the exact user/PID
# namespace and UID/GID maps required by every bounded release subprocess.
#
# Every external program is an explicit absolute-path argument whose bytes are
# bound by the package toolchain or candidate controller. The controller also
# supplies one fresh owner-private probe directory; this script never performs
# recursive path deletion.

set -Eeuo pipefail

usage() {
  printf '%s\n' \
    'Usage: check_taira_privacy_native_host.sh \' \
    '  --verified-iid PATH --cc PATH --readelf PATH \' \
    '  --python PATH --lscpu PATH --uname PATH --grep PATH --tr PATH \' \
    '  --probe-root DIRECTORY --metadata-out PATH --x509-environment-out PATH'
}

verified_iid=""
cc_path=""
readelf_path=""
metadata_out=""
x509_environment_out=""
python_path=""
lscpu_path=""
uname_path=""
grep_path=""
tr_path=""
probe_root=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --verified-iid)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "--verified-iid requires one non-empty value" >&2
        exit 1
      fi
      verified_iid="$2"
      shift 2
      ;;
    --cc)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "--cc requires one non-empty value" >&2
        exit 1
      fi
      cc_path="$2"
      shift 2
      ;;
    --readelf)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "--readelf requires one non-empty value" >&2
        exit 1
      fi
      readelf_path="$2"
      shift 2
      ;;
    --python|--lscpu|--uname|--grep|--tr|--probe-root)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "$1 requires one non-empty value" >&2
        exit 1
      fi
      case "$1" in
        --python) python_path="$2" ;;
        --lscpu) lscpu_path="$2" ;;
        --uname) uname_path="$2" ;;
        --grep) grep_path="$2" ;;
        --tr) tr_path="$2" ;;
        --probe-root) probe_root="$2" ;;
      esac
      shift 2
      ;;
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

if [[ "$python_path" != /* || ! -f "$python_path" || -L "$python_path" || ! -x "$python_path" ]]; then
  echo "--python must name an absolute executable non-symlink regular file" >&2
  exit 1
fi

validate_input_path() {
  local option_name="$1"
  local input_path="$2"
  if [[ -z "$input_path" || "$input_path" != /* ]]; then
    echo "$option_name must be an absolute path" >&2
    exit 1
  fi
  if [[ ! -f "$input_path" || -L "$input_path" ]]; then
    echo "$option_name must be a non-symlink regular file" >&2
    exit 1
  fi
  local canonical_input
  canonical_input="$(
    "$python_path" -I -S -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' \
      "$input_path"
  )"
  if [[ "$canonical_input" != "$input_path" ]]; then
    echo "$option_name must use its canonical physical path" >&2
    exit 1
  fi
}

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
  output_parent="$(
    "$python_path" -I -S -c 'import os, sys; print(os.path.dirname(sys.argv[1]))' \
      "$output_path"
  )"
  if [[ ! -d "$output_parent" || -L "$output_parent" ]]; then
    echo "$option_name parent must be a non-symlink directory" >&2
    exit 1
  fi
  local canonical_output_parent
  canonical_output_parent="$(
    "$python_path" -I -S -c 'import os, sys; print(os.path.realpath(sys.argv[1]))' \
      "$output_parent"
  )"
  if [[ "$canonical_output_parent" != "$output_parent" ]]; then
    echo "$option_name parent must use its canonical physical path" >&2
    exit 1
  fi
}

validate_input_path --verified-iid "$verified_iid"
validate_input_path --cc "$cc_path"
validate_input_path --readelf "$readelf_path"
validate_input_path --python "$python_path"
validate_input_path --lscpu "$lscpu_path"
validate_input_path --uname "$uname_path"
validate_input_path --grep "$grep_path"
validate_input_path --tr "$tr_path"
if [[ ! -x "$cc_path" || ! -x "$readelf_path" || ! -x "$lscpu_path" \
  || ! -x "$uname_path" || ! -x "$grep_path" || ! -x "$tr_path" ]]; then
  echo "all native-host tool paths must name executable files" >&2
  exit 1
fi
"$python_path" -I -S - "$probe_root" <<'PY'
import os
from pathlib import Path
import stat
import sys

path = Path(sys.argv[1])
if not path.is_absolute() or path != path.resolve(strict=True):
    raise SystemExit("--probe-root must be one canonical absolute path")
metadata = path.lstat()
if (
    not stat.S_ISDIR(metadata.st_mode)
    or stat.S_ISLNK(metadata.st_mode)
    or metadata.st_uid != os.geteuid()
    or stat.S_IMODE(metadata.st_mode) != 0o700
    or any(path.iterdir())
):
    raise SystemExit("--probe-root must be one fresh empty owner-private mode-0700 directory")
PY
validate_output_path --metadata-out "$metadata_out"
validate_output_path --x509-environment-out "$x509_environment_out"
if [[ "$metadata_out" == "$x509_environment_out" ]]; then
  echo "native host output paths must be distinct" >&2
  exit 1
fi

if [[ "$("$uname_path" -s)" != "Linux" || "$("$uname_path" -m)" != "aarch64" ]]; then
  echo "Taira privacy evidence requires native little-endian Linux aarch64" >&2
  exit 1
fi
"$python_path" -I -S - <<'PY'
import sys

if sys.byteorder != "little":
    raise SystemExit("Taira privacy evidence requires a little-endian host")
PY

if [[ -e /.dockerenv || -e /run/.containerenv ]]; then
  echo "containerized execution is forbidden for native Taira privacy evidence" >&2
  exit 1
fi
set +e
"$grep_path" -Eiq '(docker|containerd|kubepods|libpod|lxc)' /proc/1/cgroup
cgroup_marker_status=$?
set -e
if ((cgroup_marker_status == 0)); then
  echo "container cgroup detected; native Taira privacy evidence is forbidden" >&2
  exit 1
fi
if ((cgroup_marker_status != 1)); then
  echo "cannot inspect the init process cgroup for container markers" >&2
  exit 1
fi
kernel_release="$("$uname_path" -r)"
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
"$python_path" -I -S - <<'PY'
import ctypes
import os

CLONE_NEWUSER = 0x10000000
CLONE_NEWPID = 0x20000000

def fail(message: str) -> None:
    raise SystemExit(message)

def write_once(path: str, payload: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | getattr(os, "O_CLOEXEC", 0))
    try:
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                fail("Linux namespace identity-map write made no progress")
            view = view[written:]
    finally:
        os.close(descriptor)

library = ctypes.CDLL(None, use_errno=True)
try:
    unshare = library.unshare
    library.prctl
    library.capset
    library.capget
except AttributeError:
    fail("Linux user/PID namespace containment APIs are unavailable")
unshare.argtypes = [ctypes.c_int]
unshare.restype = ctypes.c_int
outer_uid = os.geteuid()
outer_gid = os.getegid()
if unshare(CLONE_NEWUSER | CLONE_NEWPID) != 0:
    fail("Linux user/PID namespace containment cannot be established")
try:
    try:
        write_once("/proc/self/setgroups", b"deny\n")
    except FileNotFoundError:
        pass
    write_once("/proc/self/uid_map", f"{outer_uid} {outer_uid} 1\n".encode("ascii"))
    write_once("/proc/self/gid_map", f"{outer_gid} {outer_gid} 1\n".encode("ascii"))
except OSError as error:
    fail(f"Linux user-namespace identity mapping is unavailable: {error}")
try:
    namespace_init = os.fork()
except OSError as error:
    fail(f"Linux PID-namespace init cannot be created: {error}")
if namespace_init == 0:
    if os.getpid() != 1:
        os._exit(91)
    try:
        target = os.fork()
    except OSError:
        os._exit(92)
    if target == 0:
        os._exit(0)
    _, target_status = os.waitpid(target, 0)
    os._exit(0 if os.WIFEXITED(target_status) and os.WEXITSTATUS(target_status) == 0 else 93)
_, init_status = os.waitpid(namespace_init, 0)
if not os.WIFEXITED(init_status) or os.WEXITSTATUS(init_status) != 0:
    fail("Linux trusted PID-namespace init qualification failed")
PY

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
memfd_noexec="$("$tr_path" -d '[:space:]' </proc/sys/vm/memfd_noexec)"
if [[ ! "$memfd_noexec" =~ ^[0-2]$ || "$memfd_noexec" == "2" ]]; then
  echo "vm.memfd_noexec forbids the mandatory explicit executable memfd" >&2
  exit 1
fi

read -r \
  iid_region \
  iid_availability_zone \
  iid_instance_id \
  instance_type \
  iid_document_sha256 \
  iid_certificate_sha256 \
  iid_verification_sha256 < <(
  "$python_path" -I -S - "$verified_iid" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

path = Path(sys.argv[1])
before = path.lstat()
if not stat.S_ISREG(before.st_mode) or stat.S_ISLNK(before.st_mode):
    raise SystemExit("verified IID must be a non-symlink regular file")
if before.st_uid != os.geteuid() or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
    raise SystemExit("verified IID must be owner-controlled and not group/world writable")
if before.st_size <= 0 or before.st_size > 256 * 1024:
    raise SystemExit("verified IID is empty or exceeds its size ceiling")
raw = path.read_bytes()
after = path.lstat()
if (before.st_dev, before.st_ino, before.st_size) != (
    after.st_dev,
    after.st_ino,
    after.st_size,
):
    raise SystemExit("verified IID changed while it was read")

def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key!r}")
        result[key] = value
    return result

try:
    payload = json.loads(raw, object_pairs_hook=reject_duplicates)
except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
    raise SystemExit("verified IID is not duplicate-free JSON") from error
expected_keys = {
    "schema",
    "verified",
    "verification_method",
    "trust_root_source",
    "region_pin",
    "document",
    "document_sha256",
    "rsa2048_body_sha256",
    "rsa2048_cms_der_sha256",
    "rsa2048_cms_structure_output_sha256",
    "regional_certificate_path",
    "regional_certificate_file_sha256",
    "regional_certificate_der_sha256",
    "regional_certificate_spki_der_sha256",
    "openssl_path",
    "openssl_sha256",
    "openssl_version_output_sha256",
    "trust_limits",
}
if not isinstance(payload, dict) or set(payload) != expected_keys:
    raise SystemExit("verified IID schema fields are not exact")
if payload["schema"] != "iroha.aws.ec2.instance-identity-verification.v1":
    raise SystemExit("verified IID has the wrong schema")
if payload["verified"] is not True:
    raise SystemExit("verified IID is not authenticated")
if payload["verification_method"] != "aws-rsa2048-cms-sha256":
    raise SystemExit("verified IID used an unreviewed authentication method")
if payload["trust_root_source"] != (
    "operator-supplied-oob-regional-aws-rsa2048-certificate"
):
    raise SystemExit("verified IID used an unreviewed trust-root source")
document = payload["document"]
if not isinstance(document, dict):
    raise SystemExit("verified IID document must be an object")
required_document = {
    "accountId",
    "architecture",
    "availabilityZone",
    "imageId",
    "instanceId",
    "instanceType",
    "pendingTime",
    "privateIp",
    "region",
    "version",
}
optional_document = {
    "billingProducts",
    "devpayProductCodes",
    "marketplaceProductCodes",
    "kernelId",
    "ramdiskId",
}
if not required_document <= set(document) or set(document) - required_document - optional_document:
    raise SystemExit("verified IID document fields are not within the reviewed schema")
if document["architecture"] != "arm64":
    raise SystemExit("verified IID does not identify an ARM64 instance")
if document["instanceType"] != "c7g.4xlarge":
    raise SystemExit("verified IID does not identify c7g.4xlarge")
if document["region"] != payload["region_pin"]:
    raise SystemExit("verified IID document region differs from its OOB region pin")
if not re.fullmatch(re.escape(document["region"]) + r"[a-z]", document["availabilityZone"]):
    raise SystemExit("verified IID availability zone is outside its pinned region")
digests = (
    payload["document_sha256"],
    payload["regional_certificate_file_sha256"],
)
if not all(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
    raise SystemExit("verified IID contains an invalid digest")
print(
    document["region"],
    document["availabilityZone"],
    document["instanceId"],
    document["instanceType"],
    payload["document_sha256"],
    payload["regional_certificate_file_sha256"],
    hashlib.sha256(raw).hexdigest(),
)
PY
)
if [[ "$instance_type" != "c7g.4xlarge" ]]; then
  echo "expected authenticated AWS c7g.4xlarge IID, observed: $instance_type" >&2
  exit 1
fi

set -o noclobber
"$lscpu_path" --json >"$probe_root/lscpu.json"

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
  "$python_path" -I -S - "$probe_root/lscpu.json" <<'PY'
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

if [[ "${BASH_SOURCE[0]}" != /* || "${BASH_SOURCE[0]}" != */* ]]; then
  echo "host checker must be invoked by its canonical absolute path" >&2
  exit 1
fi
script_directory="${BASH_SOURCE[0]%/*}"
"$cc_path" -std=c11 -O2 -Wall -Wextra -Werror -static -pthread \
  "$script_directory/taira_privacy_native_host_probe.c" \
  -o "$probe_root/native_primitives"
program_headers="$("$readelf_path" --program-headers --wide "$probe_root/native_primitives")"
dynamic_entries="$("$readelf_path" --dynamic --wide "$probe_root/native_primitives")"
set +e
"$grep_path" -E '(^|[[:space:]])INTERP([[:space:]]|$)' \
  <<<"$program_headers" >/dev/null
interp_status=$?
set -e
if ((interp_status == 0)); then
  echo "native prerequisite probe unexpectedly contains PT_INTERP" >&2
  exit 1
fi
if ((interp_status != 1)); then
  echo "cannot inspect native prerequisite probe program headers" >&2
  exit 1
fi
set +e
"$grep_path" -E '\(NEEDED\)' <<<"$dynamic_entries" >/dev/null
needed_status=$?
set -e
if ((needed_status == 0)); then
  echo "native prerequisite probe unexpectedly contains DT_NEEDED" >&2
  exit 1
fi
if ((needed_status != 1)); then
  echo "cannot inspect native prerequisite probe dynamic entries" >&2
  exit 1
fi
set +e
"$grep_path" -E \
  '(^|[[:space:]])(LOAD|GNU_STACK).*(RWE|R[[:space:]]+W[[:space:]]+E)' \
  <<<"$program_headers" >/dev/null
writable_executable_status=$?
set -e
if ((writable_executable_status == 0)); then
  echo "native prerequisite probe contains a writable executable segment" >&2
  exit 1
fi
if ((writable_executable_status != 1)); then
  echo "cannot inspect native prerequisite probe segment permissions" >&2
  exit 1
fi
landlock_abi="$("$probe_root/native_primitives")"
if [[ ! "$landlock_abi" =~ ^[0-9]+$ ]] || ((landlock_abi < 3)); then
  echo "native prerequisite probe returned invalid Landlock ABI: $landlock_abi" >&2
  exit 1
fi
probe_sha256="$(
  "$python_path" -I -S -c \
    'import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' \
    "$probe_root/native_primitives"
)"

RUNNER_NAME_VALUE="${RUNNER_NAME:-}" \
RUNNER_OS_VALUE="${RUNNER_OS:-}" \
RUNNER_ARCH_VALUE="${RUNNER_ARCH:-}" \
"$python_path" -I -S - \
  "$metadata_out" \
  "$x509_environment_out" \
  "$instance_type" \
  "$iid_region" \
  "$iid_availability_zone" \
  "$iid_instance_id" \
  "$iid_document_sha256" \
  "$iid_certificate_sha256" \
  "$iid_verification_sha256" \
  "$probe_sha256" \
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
    iid_region,
    iid_availability_zone,
    iid_instance_id,
    iid_document_sha256,
    iid_certificate_sha256,
    iid_verification_sha256,
    probe_sha256,
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
payload = {
    "schema": "iroha.taira.privacy-native-host.v1",
    "native_execution": True,
    "containerized": False,
    "architecture": "aarch64",
    "byte_order": "little",
    "instance_type": instance_type,
    "authenticated_iid_region": iid_region,
    "authenticated_iid_availability_zone": iid_availability_zone,
    "authenticated_iid_instance_id": iid_instance_id,
    "authenticated_iid_document_sha256": iid_document_sha256,
    "authenticated_iid_certificate_file_sha256": iid_certificate_sha256,
    "authenticated_iid_verification_sha256": iid_verification_sha256,
    "native_primitives_probe_sha256": probe_sha256,
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
