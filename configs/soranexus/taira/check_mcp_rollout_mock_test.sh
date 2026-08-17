#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_SCRIPT="${SCRIPT_DIR}/check_mcp_rollout.sh"
SOURCE_CONFIG="${SCRIPT_DIR}/config.toml"
readonly TEST_OPERATOR_NETWORK_ID="hash:0808080808080808080808080808080808080808080808080808080808080809#9F75"
readonly STALE_NETWORK_ID="hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"

cleanup_paths=()

make_temp_dir() {
  if [[ -n "${TMPDIR:-}" ]]; then
    [[ -d "$TMPDIR" ]] || {
      echo "TMPDIR is not an existing directory: ${TMPDIR}" >&2
      return 1
    }
    mktemp -d "${TMPDIR%/}/taira-mcp-rollout-mock.XXXXXX"
    return
  fi
  mktemp -d
}

cleanup() {
  local path
  for path in "${cleanup_paths[@]:-}"; do
    [[ -n "$path" && -e "$path" ]] && rm -rf "$path"
  done
}

trap cleanup EXIT

make_fake_repo() {
  local root="$1"

  mkdir -p \
    "${root}/configs/soranexus/taira" \
    "${root}/scripts" \
    "${root}/mockbin" \
    "${root}/state"
  cp "$SOURCE_SCRIPT" "${root}/configs/soranexus/taira/check_mcp_rollout.real.sh"
  cp "$SOURCE_CONFIG" "${root}/configs/soranexus/taira/config.toml"
  printf '%s\n' "$TEST_OPERATOR_NETWORK_ID" \
    >"${root}/state/operator-network-id"
  cat >"${root}/configs/soranexus/taira/check_mcp_rollout.sh" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
onboarding_token_args=(
  --onboarding-token-file
  "${SCRIPT_DIR}/../../../state/onboarding-token"
)
if [[ "${MOCK_OMIT_ONBOARDING_TOKEN_FILE:-0}" == "1" ]]; then
  onboarding_token_args=()
fi
VALIDATOR_ALIGNMENT_ATTEMPTS=1 VALIDATOR_PROGRESS_DELAY_SECONDS=0 \
exec "${SCRIPT_DIR}/check_mcp_rollout.real.sh" \
  --validator-root validator-1=https://validator-1.test \
  --validator-root validator-2=https://validator-2.test \
  --validator-root validator-3=https://validator-3.test \
  --validator-root validator-4=https://validator-4.test \
  --require-all-validators \
  --expected-git-sha 490dacc287f00d490dacc287f00d490dacc287f0 \
  --expected-dpn-validator-release-commit dddddddddddddddddddddddddddddddddddddddd \
  --operator-network-id "$(cat "${SCRIPT_DIR}/../../../state/operator-network-id")" \
  --operator-private-key-file "${SCRIPT_DIR}/../../../state/operator-private-key" \
  ${onboarding_token_args[@]+"${onboarding_token_args[@]}"} \
  "$@"
SH

  printf '%s\n' '0123456789abcdef0123456789ABCDEF' \
    >"${root}/state/onboarding-token"
  chmod 600 "${root}/state/onboarding-token"
  printf '%s\n' 'test-only-runtime-operator-private-key' \
    >"${root}/state/operator-private-key"
  chmod 600 "${root}/state/operator-private-key"

  cat >"${root}/scripts/operator_http_headers.py" <<'PY'
#!/usr/bin/env python3
print("X-Iroha-Operator-Public-Key: ed0120" + "11" * 32)
print("X-Iroha-Operator-Timestamp-Ms: 1800000000000")
print("X-Iroha-Operator-Nonce: " + "22" * 16)
print("X-Iroha-Operator-Signature: " + "33" * 64)
PY

  printf '%s\n' 'Ready' >"${root}/state/readyz.txt"

  cat >"${root}/state/nexus-lifecycle.json" <<'JSON'
{
  "version": 1,
  "nexus_enabled": true,
  "lane_count": 7,
  "lanes": [
    {"id": 0, "dataspace_id": 0, "alias": "core"},
    {"id": 1, "dataspace_id": 0, "alias": "governance"},
    {"id": 2, "dataspace_id": 0, "alias": "zk"},
    {"id": 3, "dataspace_id": 10, "alias": "dpn"},
    {"id": 4, "dataspace_id": 6647857470246403404, "alias": "external-poc"},
    {"id": 5, "dataspace_id": 8477022798449861195, "alias": "boi-mobile"},
    {"id": 6, "dataspace_id": 20, "alias": "cbsi"}
  ],
  "catalog_hash": "hash:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "incarnations": [],
  "incarnation_root": "hash:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
}
JSON

  cat >"${root}/scripts/taira_bootstrap_canary.py" <<'PY'
#!/usr/bin/env python3
import os
from pathlib import Path
import sys

output_path = None
onboarding_token_file = None
chain_id = "fc56984b-2be7-431d-840e-21514d1883f0"
network_ids = []
args = sys.argv[1:]
for index, value in enumerate(args):
    if value == "--output-config" and index + 1 < len(args):
        output_path = args[index + 1]
    elif value == "--onboarding-token-file" and index + 1 < len(args):
        onboarding_token_file = args[index + 1]
    elif value == "--chain-id" and index + 1 < len(args):
        chain_id = args[index + 1]
    elif value == "--network-id" and index + 1 < len(args):
        network_ids.append(args[index + 1])

if output_path is None:
    raise SystemExit("missing --output-config")
if onboarding_token_file is None:
    raise SystemExit("missing --onboarding-token-file")
if len(network_ids) != 1:
    raise SystemExit("expected exactly one --network-id")
network_id = network_ids[0]
token_path = Path(onboarding_token_file)
if not token_path.is_absolute() or not token_path.is_file():
    raise SystemExit("invalid --onboarding-token-file")
if token_path.stat().st_mode & 0o077:
    raise SystemExit("unsafe --onboarding-token-file")
state_dir = os.environ.get("MOCK_STATE_DIR")
if state_dir:
    Path(state_dir, "onboarding_token_seen").write_text(
        str(token_path) + "\n", encoding="utf-8"
    )
    Path(state_dir, "bootstrap_network_id_seen").write_text(
        network_id + "\n", encoding="utf-8"
    )

with open(output_path, "w", encoding="utf-8") as handle:
    handle.write(
        f'chain = "{chain_id}"\n'
        f'network_id = "{network_id}"\n'
        '\n'
        '[account]\n'
        'domain = "universal"\n'
        'public_key = "ED0120FAKEPUBLICKEY0123456789ABCDEF0123456789ABCDEF0123456789AB"\n'
        'private_key = "802620FAKEPRIVATEKEY0123456789ABCDEF0123456789ABCDEF0123456789"\n'
        'chain_discriminant = 369\n'
        '\n'
        '[transaction]\n'
        'nonce = true\n'
        'time_to_live_ms = 120000\n'
        'status_timeout_ms = 120000\n'
    )
PY

  cat >"${root}/scripts/taira_faucet_canary.py" <<'PY'
#!/usr/bin/env python3
import os
import sys
from pathlib import Path

status_timeout_ms = None
args = sys.argv[1:]
for index, value in enumerate(args):
    if value == "--status-timeout-ms" and index + 1 < len(args):
        status_timeout_ms = args[index + 1]

state_dir = os.environ.get("MOCK_STATE_DIR")
if state_dir:
    state = Path(state_dir)
    state.joinpath("faucet_seen").write_text("1\n", encoding="utf-8")
    if status_timeout_ms is not None:
        state.joinpath("faucet_status_timeout_seen").write_text(
            status_timeout_ms + "\n", encoding="utf-8"
        )
print("faucet bootstrap skipped in mock harness")
PY

  cat >"${root}/mockbin/curl" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

method="GET"
output_file=""
header_file=""
payload=""
url=""
connect_timeout=""
max_time=""
headers=()
accept_header=""
operator_header_names=" "

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      output_file="$2"
      shift 2
      ;;
    --dump-header)
      header_file="$2"
      shift 2
      ;;
    --resolve)
      shift 2
      ;;
    --connect-timeout)
      connect_timeout="$2"
      shift 2
      ;;
    --max-time)
      max_time="$2"
      shift 2
      ;;
    --write-out)
      shift 2
      ;;
    -X)
      method="$2"
      shift 2
      ;;
    -H)
      headers+=("$2")
      shift 2
      ;;
    --data)
      payload="$2"
      shift 2
      ;;
    --silent|--show-error)
      shift
      ;;
    *)
      url="$1"
      shift
      ;;
  esac
done

for header in "${headers[@]+"${headers[@]}"}"; do
  header_name="$(printf '%s' "${header%%:*}" | tr '[:upper:]' '[:lower:]')"
  if [[ "$header_name" == "accept" ]]; then
    accept_header="${header#*:}"
    accept_header="${accept_header# }"
  fi
  if [[ "$header_name" == "x-iroha-api-version" ]]; then
    echo "rollout must not send the retired x-iroha-api-version header" >&2
    exit 92
  fi
  operator_header_names+="${header_name} "
done

if [[ "$url" == */v1/sumeragi/status ]]; then
  for required_header in \
    x-iroha-operator-public-key \
    x-iroha-operator-timestamp-ms \
    x-iroha-operator-nonce \
    x-iroha-operator-signature; do
    if [[ "$operator_header_names" != *" ${required_header} "* ]]; then
      echo "operator-protected status omitted ${required_header}" >&2
      exit 94
    fi
  done
fi

if [[ "$url" == */health || "$url" == */readyz ]]; then
  if [[ "$accept_header" != "text/plain" ]]; then
    echo "ordinary health probes must request text/plain, saw ${accept_header:-<missing>}" >&2
    exit 93
  fi
fi

if [[ -n "${MOCK_STATE_DIR:-}" ]]; then
  printf '%s %s\n' "$connect_timeout" "$max_time" >> "${MOCK_STATE_DIR}/curl_timeouts"
fi
if [[ -z "$connect_timeout" || -z "$max_time" ]]; then
  echo "curl timeout flags were not provided" >&2
  exit 91
fi
if [[ -n "${MOCK_CURL_DELAY_SECONDS:-}" ]]; then
  sleep "$MOCK_CURL_DELAY_SECONDS"
fi

scenario="${MOCK_SCENARIO:-}"
validator_index=""
validator_sample=0
validator_height=0
validator_block_hash=""
if [[ "$url" =~ ^https://validator-([1-4])\.test/ ]]; then
  validator_index="${BASH_REMATCH[1]}"
  if [[ "$method" == "GET" && "$url" == "https://validator-1.test/status" ]]; then
    validator_sample="$(cat "${MOCK_STATE_DIR:?}/validator_sample" 2>/dev/null || printf '0')"
    validator_sample=$((validator_sample + 1))
    printf '%s\n' "$validator_sample" >"${MOCK_STATE_DIR}/validator_sample"
  else
    validator_sample="$(cat "${MOCK_STATE_DIR:?}/validator_sample" 2>/dev/null || printf '1')"
  fi
  validator_height=$((706 + validator_sample))
  validator_block_hash="$(printf '%064x' "$validator_height")"
  if [[ "$scenario" == "fleet_commit_mismatch" && "$validator_index" == "4" ]]; then
    validator_block_hash="ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
  elif [[ "$scenario" == "fleet_stale_commit_progress" && "$validator_sample" -gt 1 ]]; then
    validator_height=707
    validator_block_hash="$(printf '%064x' "$validator_height")"
  fi
fi
after_ping=0
if [[ -n "${MOCK_STATE_DIR:-}" && -f "${MOCK_STATE_DIR}/ping_seen" ]]; then
  after_ping=1
fi

status="200"
content_type="application/json"
body='{}'

if [[ -n "$validator_index" && "$method" == "GET" && "$url" == "https://validator-${validator_index}.test/health" ]]; then
  body='Healthy'
  content_type='text/plain'
  if [[ "$scenario" == "fleet_health_unavailable" && "$validator_index" == "4" ]]; then
    status='503'
    body='Unavailable'
  fi
elif [[ -n "$validator_index" && "$method" == "GET" && "$url" == "https://validator-${validator_index}.test/readyz" ]]; then
  body="$(cat "${MOCK_STATE_DIR:?}/readyz.txt")"
  content_type='text/plain'
  if [[ "$scenario" == "fleet_readyz_unavailable" && "$validator_index" == "4" ]]; then
    status='503'
    body='Unavailable'
  fi
  printf '%s\n' "$url" >>"${MOCK_STATE_DIR}/readyz_seen"
elif [[ -n "$validator_index" && "$method" == "GET" && "$url" == "https://validator-${validator_index}.test/v1/nexus/lifecycle" ]]; then
  body="$(cat "${MOCK_STATE_DIR:?}/nexus-lifecycle.json")"
  if [[ "$scenario" == "fleet_dataspace_mismatch" && "$validator_index" == "4" ]]; then
    body="${body/8477022798449861195/9}"
  elif [[ "$scenario" == "fleet_catalog_changes_between_samples" && "$validator_sample" -gt 1 ]]; then
    body="${body/cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee}"
  fi
elif [[ -n "$validator_index" && "$method" == "GET" && "$url" == "https://validator-${validator_index}.test/status" ]]; then
  body="$(python3 - "$validator_height" "$scenario" "$validator_sample" <<'PY'
import json
import sys

height = int(sys.argv[1])
scenario = sys.argv[2]
sample = int(sys.argv[3])
if scenario == "fleet_lagging_status_blocks":
    height -= 1

lane_specs = [
    (0, "core", 0, "universal"),
    (1, "governance", 0, "universal"),
    (2, "zk", 0, "universal"),
    (3, "dpn", 10, "dpn"),
    (4, "external-poc", 6647857470246403404, "is"),
    (5, "boi-mobile", 8477022798449861195, "is2"),
    (6, "cbsi", 20, "cbsi"),
]
rosters = {
    "universal": [f"universal-validator-{index}" for index in range(1, 5)],
    "dpn": [f"dpn-validator-{index}" for index in range(1, 5)],
    "is": [f"is-validator-{index}" for index in range(1, 5)],
    "is2": [f"is2-validator-{index}" for index in range(1, 5)],
    "cbsi": [f"cbsi-validator-{index}" for index in range(1, 5)],
}
if scenario == "fleet_repeated_dataspace_roster":
    rosters["is2"] = rosters["is"].copy()
if scenario == "fleet_repeated_universal_roster":
    rosters["dpn"] = rosters["universal"].copy()
if scenario == "fleet_partial_account_reuse":
    rosters["dpn"][0] = rosters["universal"][0]


def manifest_bindings(dataspace_alias, validators):
    return [
        {
            "validator": validator,
            "peer_id": f"{dataspace_alias}-peer-{index}",
            "torii_url": f"https://{dataspace_alias}-validator-{index}.test",
        }
        for index, validator in enumerate(validators, start=1)
    ]

teu_lane_commit = []
dataspace_catalog = []
for lane_id, lane_alias, dataspace_id, dataspace_alias in lane_specs:
    # Core and ZK inherit the universal cohort selected by governance. Every
    # non-universal physical dataspace projects its own ready manifest even
    # though only the governed governance lane marks a manifest as required.
    manifest_required = lane_alias == "governance"
    has_manifest = lane_alias == "governance" or dataspace_alias != "universal"
    if scenario == "fleet_missing_universal_roster" and dataspace_alias == "universal":
        has_manifest = False
    validators = rosters[dataspace_alias].copy() if has_manifest else []
    bindings = manifest_bindings(dataspace_alias, validators) if has_manifest else []
    quorum = 3 if has_manifest else None
    manifest_path = f"/manifests/{lane_alias}.manifest.json" if has_manifest else None
    if scenario == "fleet_missing_dataspace_roster" and dataspace_alias == "cbsi":
        validators = []
        bindings = []
        quorum = None
    if scenario == "fleet_invalid_dataspace_quorum" and dataspace_alias == "dpn":
        quorum = 2
    if scenario == "fleet_same_dataspace_roster_mismatch" and lane_alias == "zk":
        has_manifest = True
        validators = [f"other-universal-validator-{index}" for index in range(1, 5)]
        bindings = manifest_bindings("other-universal", validators)
        quorum = 3
        manifest_path = "/manifests/zk.manifest.json"
    if scenario == "fleet_missing_binding_projection" and lane_alias == "dpn":
        bindings = None
    if scenario == "fleet_binding_unknown_field" and lane_alias == "dpn":
        bindings[0]["unexpected"] = True
    if scenario == "fleet_binding_missing_field" and lane_alias == "dpn":
        bindings[0].pop("torii_url")
    if scenario == "fleet_binding_account_mismatch" and lane_alias == "dpn":
        bindings[0]["validator"] = "unlisted-validator"
    if scenario == "fleet_duplicate_binding_peer" and lane_alias == "dpn":
        bindings[1]["peer_id"] = bindings[0]["peer_id"]
    if scenario == "fleet_duplicate_binding_torii" and lane_alias == "dpn":
        bindings[1]["torii_url"] = bindings[0]["torii_url"]
    if scenario == "fleet_same_dataspace_binding_mismatch" and lane_alias == "zk":
        has_manifest = True
        validators = rosters["universal"].copy()
        bindings = manifest_bindings("universal", validators)
        bindings[0]["peer_id"] = "remapped-universal-peer"
        quorum = 3
        manifest_path = "/manifests/zk.manifest.json"
    if scenario == "fleet_partial_peer_reuse" and lane_alias == "dpn":
        bindings[0]["peer_id"] = "universal-peer-1"
    if scenario == "fleet_partial_torii_reuse" and lane_alias == "dpn":
        bindings[0]["torii_url"] = "https://universal-validator-1.test"
    if scenario == "fleet_noncanonical_torii" and lane_alias == "dpn":
        bindings[0]["torii_url"] = "https://DPN-validator-1.test:443/"
    if (
        scenario == "fleet_bindings_change_between_samples"
        and sample > 1
        and lane_alias == "dpn"
    ):
        bindings[0]["peer_id"] = "dpn-peer-1-rotated"
    lane = {
        "lane_id": lane_id,
        "alias": lane_alias,
        "dataspace_id": dataspace_id,
        "dataspace_alias": dataspace_alias,
        "manifest_required": manifest_required,
        "manifest_ready": has_manifest,
        "manifest_path": manifest_path,
        "manifest_validators": validators,
        "manifest_validator_bindings": bindings,
        "manifest_quorum": quorum,
    }
    if scenario == "fleet_missing_binding_projection" and lane_alias == "dpn":
        lane.pop("manifest_validator_bindings")
    teu_lane_commit.append(lane)
    dataspace_catalog.append({
        "lane_id": lane_id,
        "lane_alias": lane_alias,
        "dataspace_id": dataspace_id,
        "alias": dataspace_alias,
        "manifest_required": manifest_required,
        "manifest_ready": has_manifest,
        "manifest_path": manifest_path,
    })

routing_rule_tuples = [
    (3, 10, "account", "*@dpn"),
    (4, 6647857470246403404, "account", "*@wonderland.is"),
    (5, 8477022798449861195, "account", "*@boi.is2"),
    (5, 8477022798449861195, "account", "*@leumi.is2"),
    (5, 8477022798449861195, "account", "*@hapoalim.is2"),
    (5, 8477022798449861195, "account", "*@discount.is2"),
    (5, 8477022798449861195, "account", "*@mizrahi.is2"),
    (5, 8477022798449861195, "account", "*@fibi.is2"),
    (5, 8477022798449861195, "account", "*@onezero.is2"),
    (5, 8477022798449861195, "account", "*@jerusalem.is2"),
    (6, 20, "account", "*@cbsi"),
    (6, 20, "account", "*@pob.cbsi"),
    (6, 20, "account", "*@bred.cbsi"),
    (6, 20, "account", "*@anz.cbsi"),
    (6, 20, "account", "*@bsp.cbsi"),
    (6, 20, "account", "*@m-selen.cbsi"),
    (6, 20, "account", "*@ezipei.cbsi"),
    (1, 0, "instruction", "governance"),
    (2, 0, "instruction", "smartcontract::deploy"),
]
routing_rules = [
    {
        "lane": lane_id,
        "dataspace_id": dataspace_id,
        "matcher": {
            matcher_kind: matcher_value,
            "description": f"canonical Taira routing rule {position}",
        },
    }
    for position, (lane_id, dataspace_id, matcher_kind, matcher_value) in enumerate(
        routing_rule_tuples
    )
]
if scenario == "fleet_wrong_routing_matcher":
    routing_rules[1]["matcher"]["account"] = "*@is"

payload = {
    "build": {
        "dpn_validator_release_commit": "d" * 40,
        "git_commit_sha": "490dacc287f00d490dacc287f00d490dacc287f0",
    },
    "peers": 4,
    "blocks": height,
    "queue_size": 0,
    "teu_lane_commit": teu_lane_commit,
    "teu_dataspace_backlog": [{"backlog": 0}],
    "dataspace_catalog": dataspace_catalog,
}
if scenario == "fleet_missing_routing_policy":
    payload["nexus"] = {}
else:
    payload["nexus"] = {
        "routing_policy": {
            "default_lane": 0,
            "default_dataspace": 0,
            "rules": routing_rules,
        }
    }
print(json.dumps(payload, separators=(",", ":")))
PY
)"
elif [[ -n "$validator_index" && "$method" == "GET" && "$url" == */v1/sumeragi/status ]]; then
  body="$(python3 - "$validator_index" "$validator_height" "$validator_block_hash" "$scenario" <<'PY'
import json
import sys

validator_index, committed_height_raw, block_hash, scenario = sys.argv[1:]
committed_height = int(committed_height_raw)
node_hex = {"1": "A", "2": "B", "3": "C", "4": "D"}[validator_index] * 64
canonical_block_hash = "hash:" + block_hash.upper() + "#Ab01"
if scenario == "fleet_committed_hash_trailing_bytes" and validator_index == "4":
    canonical_block_hash += "trailing"
subject = {
    "block_hash": canonical_block_hash,
    "payload_hash": "hash:" + "F" * 64,
}
payload = {
    "protocol_version": 4,
    "restart_required": False,
    "node_fingerprint": "hash:" + node_hex,
    "build_fingerprint": "hash:" + "B" * 64,
    "config_fingerprint": "hash:" + "C" * 64,
    "height_context_id": ["hash:" + "D" * 64],
    "height": committed_height + 1,
    "view": 0,
    "phase": {"phase": "prepare", "details": None},
    "leader": 0,
    "body_state": {"state": "missing", "details": None},
    "last_committed_height": committed_height,
    "last_committed_subject": subject,
    "height_context": {
        "epoch": 1,
        "epoch_end_height": 9999,
        "mode": {"mode": "permissioned", "details": None},
        "epoch_seed": "0" * 64,
        "validator_count": 4,
        "quorum": {"min_signers": 3, "total_power": 4},
    },
    "last_commit_qc": {
        "certificate": {
            "round": {"height": committed_height, "view": 0},
            "phase": {"phase": "commit", "details": None},
            "subject": subject,
        },
        "validator_count": 4,
        "signer_count": 3,
        "min_signers": 3,
        "signed_power": 3,
        "total_power": 4,
    },
    "lane_settlement_commitments": [],
    "lane_relay_envelopes": [],
    "lane_payload_ownerships": [],
    "committed_lane_blocks": [],
    "lane_block_sessions": [],
    "local_peer_removed": False,
    "operator": {
        "view_change_install_total": 2,
        "busy_deferral_total": 0,
        "adapter_queues": {
            "ingress_keys": 0,
            "ingress_capacity": 64,
            "deferred_completion": 0,
            "deferred_progress": 0,
            "deferred_progress_capacity": 64,
            "deferred_normal": 0,
            "deferred_normal_capacity": 64,
        },
        "tx_queue": {
            "tracked_transactions": 1,
            "queued_transactions": 1,
            "capacity": 100,
            "retained_bytes": 128,
            "max_retained_bytes": 8192,
            "oldest_queued_age_ms": 5,
            "saturated_by_count": False,
            "saturated_by_bytes": False,
            "saturated_by_age": False,
        },
    },
}
print(json.dumps(payload, separators=(",", ":")))
PY
)"
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/mcp" ]]; then
  if [[ $after_ping -eq 1 && "$scenario" == "public_503_mcp" ]]; then
    status="503"
    content_type="text/plain"
    body='service unavailable'
  else
    body='{"capabilities":{"tools":{}}}'
  fi
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/mcp" && "$payload" == *'"method":"initialize"'* ]]; then
  body='{"result":{"protocolVersion":"2025-06-18"}}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/mcp" && "$payload" == *'"method":"notifications/initialized"'* ]]; then
  if [[ "$scenario" == "initialized_timeout" ]]; then
    echo "curl: (28) Operation timed out after mock timeout" >&2
    exit 28
  fi
  status="202"
  body=''
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/mcp" && "$payload" == *'"method":"tools/list"'* ]]; then
  body='{"result":{"tools":[{"name":"iroha.health","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.sumeragi.status","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.musubi.queries.exact_package","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.musubi.queries.exact_release","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.musubi.instructions.release_yank_set","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.transactions.submit","inputSchema":{"type":"object","properties":{}}},{"name":"iroha.transactions.submit_and_wait","inputSchema":{"type":"object","properties":{}}}]}}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/health" ]]; then
  body='Healthy'
  content_type='text/plain'
  if [[ "$scenario" == "public_health_unavailable" ]]; then
    status='503'
    body='Unavailable'
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/readyz" ]]; then
  body="$(cat "${MOCK_STATE_DIR:?}/readyz.txt")"
  content_type='text/plain'
  if [[ "$scenario" == "public_readyz_unavailable" ]]; then
    status='503'
    body='Unavailable'
  fi
  printf '%s\n' "$url" >>"${MOCK_STATE_DIR}/readyz_seen"
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/nexus/lifecycle" ]]; then
  body="$(cat "${MOCK_STATE_DIR:?}/nexus-lifecycle.json")"
  if [[ "$scenario" == "lane_count_regressed" ]]; then
    body="${body/\"lane_count\": 7/\"lane_count\": 5}"
  elif [[ "$scenario" == "governance_promoted_to_dataspace" ]]; then
    body="${body/\"id\": 1, \"dataspace_id\": 0/\"id\": 1, \"dataspace_id\": 1}"
  elif [[ "$scenario" == "zk_promoted_to_dataspace" ]]; then
    body="${body/\"id\": 2, \"dataspace_id\": 0/\"id\": 2, \"dataspace_id\": 2}"
  elif [[ "$scenario" == "dataspace_dpn_missing" ]]; then
    body="${body/\"alias\": \"dpn\"/\"alias\": \"wrong-dpn\"}"
  elif [[ "$scenario" == "dataspace_cbsi_missing" ]]; then
    body="${body/\"alias\": \"cbsi\"/\"alias\": \"wrong-cbsi\"}"
  elif [[ "$scenario" == "dataspace_is2_missing" ]]; then
    body="${body/\"alias\": \"boi-mobile\"/\"alias\": \"wrong-mobile\"}"
  elif [[ "$scenario" == "dataspace_is_id_mismatch" ]]; then
    body="${body/6647857470246403404/7}"
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/status" ]]; then
  if [[ $after_ping -eq 1 && "$scenario" == "public_502" ]]; then
    status="502"
    content_type="text/plain"
    body='bad gateway'
  elif [[ $after_ping -eq 1 && "$scenario" == "public_503" ]]; then
    status="503"
    content_type="text/plain"
    body='service unavailable'
  else
    body="$(python3 - "$scenario" "$after_ping" <<'PY'
import json
import sys

scenario = sys.argv[1]
after_ping = sys.argv[2] == "1"
routing_rule_tuples = [
    (3, 10, "account", "*@dpn"),
    (4, 6647857470246403404, "account", "*@wonderland.is"),
    (5, 8477022798449861195, "account", "*@boi.is2"),
    (5, 8477022798449861195, "account", "*@leumi.is2"),
    (5, 8477022798449861195, "account", "*@hapoalim.is2"),
    (5, 8477022798449861195, "account", "*@discount.is2"),
    (5, 8477022798449861195, "account", "*@mizrahi.is2"),
    (5, 8477022798449861195, "account", "*@fibi.is2"),
    (5, 8477022798449861195, "account", "*@onezero.is2"),
    (5, 8477022798449861195, "account", "*@jerusalem.is2"),
    (6, 20, "account", "*@cbsi"),
    (6, 20, "account", "*@pob.cbsi"),
    (6, 20, "account", "*@bred.cbsi"),
    (6, 20, "account", "*@anz.cbsi"),
    (6, 20, "account", "*@bsp.cbsi"),
    (6, 20, "account", "*@m-selen.cbsi"),
    (6, 20, "account", "*@ezipei.cbsi"),
    (1, 0, "instruction", "governance"),
    (2, 0, "instruction", "smartcontract::deploy"),
]
routing_rules = [
    {
        "lane": lane_id,
        "dataspace_id": dataspace_id,
        "matcher": {
            matcher_kind: matcher_value,
            "description": f"canonical Taira routing rule {position}",
        },
    }
    for position, (lane_id, dataspace_id, matcher_kind, matcher_value) in enumerate(
        routing_rule_tuples
    )
]
if scenario == "public_wrong_routing_matcher" or (
    scenario == "post_canary_wrong_routing_matcher" and after_ping
):
    routing_rules[1]["matcher"]["account"] = "*@is"

payload = {
    "build": {
        "dpn_validator_release_commit": "d" * 40,
        "git_commit_sha": "490dacc287f00d490dacc287f00d490dacc287f0",
    },
    "peers": 4,
    "blocks": 707,
    "queue_size": 0,
    "teu_dataspace_backlog": [{"backlog": 0}],
    "nexus": {
        "routing_policy": {
            "default_lane": 0,
            "default_dataspace": 0,
            "rules": routing_rules,
        }
    },
}
if scenario == "status_build_sha_missing":
    payload.pop("build")
elif scenario == "status_build_sha_too_short":
    payload["build"]["git_commit_sha"] = "490dac"
elif scenario == "status_build_sha_mismatch":
    payload["build"]["git_commit_sha"] = "94dcbf7c28"
elif scenario == "status_dpn_commit_mismatch":
    payload["build"]["dpn_validator_release_commit"] = "e" * 40
print(json.dumps(payload, separators=(",", ":")))
PY
)"
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sumeragi/status" ]]; then
  body='{"protocol_version":4,"restart_required":false,"node_fingerprint":"hash:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","build_fingerprint":"hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB","config_fingerprint":"hash:CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC","height_context_id":["hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD"],"height":708,"view":0,"phase":{"phase":"prepare","details":null},"leader":0,"body_state":{"state":"missing","details":null},"last_committed_height":707,"last_committed_subject":{"block_hash":"hash:EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE","payload_hash":"hash:FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"},"height_context":{"epoch":1,"epoch_end_height":720,"mode":{"mode":"permissioned","details":null},"epoch_seed":"0000000000000000000000000000000000000000000000000000000000000000","validator_count":4,"quorum":{"min_signers":3,"total_power":4}},"last_commit_qc":{"certificate":{"round":{"height":707,"view":0},"phase":{"phase":"commit","details":null},"subject":{"block_hash":"hash:EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE","payload_hash":"hash:FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"}},"validator_count":4,"signer_count":3,"min_signers":3,"signed_power":3,"total_power":4},"lane_settlement_commitments":[],"lane_relay_envelopes":[],"lane_payload_ownerships":[],"committed_lane_blocks":[],"lane_block_sessions":[],"local_peer_removed":false,"operator":{"view_change_install_total":2,"busy_deferral_total":0,"adapter_queues":{"ingress_keys":0,"ingress_capacity":64,"deferred_completion":0,"deferred_progress":0,"deferred_progress_capacity":64,"deferred_normal":0,"deferred_normal_capacity":64},"tx_queue":{"tracked_transactions":1,"queued_transactions":1,"capacity":100,"retained_bytes":128,"max_retained_bytes":8192,"oldest_queued_age_ms":5,"saturated_by_count":false,"saturated_by_bytes":false,"saturated_by_age":false}}}'
  if [[ "$scenario" == "sumeragi_missing_restart_required" ]]; then
    body="${body/\"restart_required\":false,/}"
  elif [[ "$scenario" == "sumeragi_invalid_restart_required" ]]; then
    body="${body/\"restart_required\":false/\"restart_required\":0}"
  elif [[ $after_ping -eq 1 && "$scenario" == "sumeragi_highest_qc_behind_commit" ]]; then
    body="${body/\"height\":707,\"view\":0/\"height\":6544,\"view\":0}"
  elif [[ $after_ping -eq 1 && "$scenario" == "sumeragi_locked_qc_behind_commit" ]]; then
    body="${body/\"signed_power\":3/\"signed_power\":2}"
  elif [[ $after_ping -eq 1 && "$scenario" == "sumeragi_idle_high_view_missing_qc" ]]; then
    body="${body/\"last_commit_qc\"/\"invalid_commit_qc\"}"
    body="${body/\"view\":0,\"phase\"/\"view\":42,\"phase\"}"
  elif [[ $after_ping -eq 1 && "$scenario" == "post_canary_sumeragi_missing_validator_set" ]]; then
    body="${body/\"validator_count\":4/\"validator_count\":0}"
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sccp/capabilities" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == */v1/time/now ]]; then
  if [[ "$scenario" == "time_unhealthy" ]]; then
    body='{"now":1785168000000,"offset_ms":0,"confidence_ms":0,"sample_count":0,"peer_count":3,"enforcement_mode":"reject","fallback":true,"health":{"healthy":false,"min_samples_ok":false,"offset_ok":true,"confidence_ok":true}}'
  elif [[ "$scenario" == "time_warn_enforcement" ]]; then
    body='{"now":1785168000000,"offset_ms":1,"confidence_ms":2,"sample_count":3,"peer_count":3,"enforcement_mode":"warn","fallback":false,"health":{"healthy":true,"min_samples_ok":true,"offset_ok":true,"confidence_ok":true}}'
  else
    body='{"now":1785168000000,"offset_ms":1,"confidence_ms":2,"sample_count":3,"peer_count":3,"enforcement_mode":"reject","fallback":false,"health":{"healthy":true,"min_samples_ok":true,"offset_ok":true,"confidence_ok":true}}'
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sccp/registry" ]]; then
  body='{"version":1,"lanes":[]}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/zk/proofs/count" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/sumeragi/validator-sets" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/nexus/public-lanes/0/validators" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/nexus/public-lanes/0/stake" ]]; then
  body='{}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/contracts/state" ]]; then
  status="400"
  body='{"error":"missing selectors"}'
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/pipeline/transactions/status" ]]; then
  if [[ "$scenario" == "canonical_status_missing" ]]; then
    status="404"
    body='{"code":"route_not_found","message":"route not found"}'
  elif [[ "$scenario" == "canonical_status_wrong_error" ]]; then
    status="400"
    body='{"code":"bad_request","message":"generic edge rejection"}'
  else
    status="400"
    body='{"code":"query_validation_failed","message":"missing hash query parameter"}'
  fi
elif [[ "$method" == "GET" && "$url" == "https://taira.sora.org/v1/transactions/status" ]]; then
  if [[ "$scenario" == "retired_status_alias_mounted" ]]; then
    status="200"
    body='{"status":{"kind":"Applied"}}'
  elif [[ "$scenario" == "retired_status_wrong_error" ]]; then
    status="404"
    body='{"code":"not_found","message":"non-canonical edge rejection"}'
  else
    status="404"
    body='{"code":"route_not_found","message":"route not found"}'
  fi
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/musubi/queries/ordered-prefix" ]]; then
  status="401"
  body='{"code":"canonical_authentication_required","message":"canonical account request authentication is required"}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/musubi/instructions/release-yank-set" ]]; then
  status="401"
  body='{"code":"canonical_authentication_required","message":"canonical account request authentication is required"}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/contracts/deploy" ]]; then
  status="404"
  body='{"code":"route_not_found","message":"route not found"}'
elif [[ "$method" == "POST" && "$url" == "https://taira.sora.org/v1/bridge/messages" ]]; then
  status="400"
  body='{"error":"empty bridge body"}'
else
  status="404"
  body='{"error":"unexpected mock route"}'
fi

printf 'HTTP/1.1 %s mock\r\nContent-Type: %s\r\n\r\n' "$status" "$content_type" >"$header_file"
printf '%s' "$body" >"$output_file"
printf '%s' "$status"
SH

  cat >"${root}/mockbin/iroha" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

if [[ "$*" == *"ledger transaction ping"* ]]; then
  mkdir -p "${MOCK_STATE_DIR:?}"
  : > "${MOCK_STATE_DIR}/ping_seen"
    case "${MOCK_SCENARIO:-}" in
      txn_expired)
        echo "Transaction expired"
        exit 1
        ;;
      permission_403)
        echo "HTTP 403 Forbidden"
        exit 1
        ;;
      public_502|public_503|public_503_mcp)
        echo "ping failed while public ingress was degrading"
        exit 1
        ;;
      sumeragi_highest_qc_behind_commit|sumeragi_locked_qc_behind_commit|sumeragi_idle_high_view_missing_qc|post_canary_sumeragi_missing_validator_set|post_canary_wrong_routing_matcher)
        echo "pong"
        exit 0
        ;;
      failed_asset_then_success)
        ping_calls_file="${MOCK_STATE_DIR}/ping_calls"
        ping_calls=0
        [[ -f "$ping_calls_file" ]] && ping_calls="$(cat "$ping_calls_file")"
        ping_calls=$((ping_calls + 1))
        printf '%s\n' "$ping_calls" >"$ping_calls_file"
        if [[ "$ping_calls" -eq 1 ]]; then
          echo "Failed to find asset"
          exit 1
        fi
        echo "pong"
        exit 0
        ;;
      success)
        echo "pong"
        exit 0
        ;;
    *)
      echo "unexpected mock scenario for ping: ${MOCK_SCENARIO:-}" >&2
      exit 1
      ;;
  esac
fi

if [[ "$*" == *"tools address convert"* ]]; then
  echo '{"i105":{"value":"testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV@universal"}}'
  exit 0
fi

echo "unexpected iroha invocation: $*" >&2
exit 1
SH

  cat >"${root}/mockbin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail

mkdir -p "${MOCK_STATE_DIR:?}"
: > "${MOCK_STATE_DIR}/cargo_seen"

if [[ "$*" == *"-p iroha_cli --bin iroha --"* && "$*" == *"ledger transaction ping"* ]]; then
  : > "${MOCK_STATE_DIR}/ping_seen"
  exit 0
fi

echo "unexpected cargo invocation: $*" >&2
exit 1
SH

  chmod +x \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.real.sh" \
    "${root}/scripts/taira_bootstrap_canary.py" \
    "${root}/scripts/taira_faucet_canary.py" \
    "${root}/mockbin/curl" \
    "${root}/mockbin/iroha" \
    "${root}/mockbin/cargo"
}

run_case() {
  local scenario="$1"
  local expected_pattern="$2"
  local extra_pattern="${3:-}"
  local expected_git_sha="${4:-}"
  local root output_file

  root="$(make_temp_dir)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  output_file="${root}/output.log"

  if PATH="${root}/mockbin:${PATH}" \
      MOCK_SCENARIO="$scenario" \
      MOCK_STATE_DIR="${root}/state" \
      EXPECTED_TAIRA_GIT_SHA="$expected_git_sha" \
      WRITE_CONFIG_DEFAULT="${root}/canary.toml" \
      POST_CANARY_STATUS_RECHECK_ATTEMPTS=2 \
      POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=0 \
      "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
        --skip-local \
        --public-root https://taira.sora.org \
        --iroha-bin "${root}/mockbin/iroha" \
        >"$output_file" 2>&1; then
    echo "case ${scenario} unexpectedly succeeded" >&2
    sed -n '1,160p' "$output_file" >&2 || true
    return 1
  fi

  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "case ${scenario} did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi

  if [[ -n "$extra_pattern" ]] && ! grep -q "$extra_pattern" "$output_file"; then
    echo "case ${scenario} did not emit expected extra pattern: ${extra_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
}

run_invalid_canary_identity_case() {
  local mutation="$1"
  local expected_pattern="$2"
  local root output_file config_path

  root="$(make_temp_dir)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  output_file="${root}/invalid-${mutation}.log"
  config_path="${root}/canary.toml"
  "${root}/scripts/taira_bootstrap_canary.py" \
    --onboarding-token-file "${root}/state/onboarding-token" \
    --network-id "$TEST_OPERATOR_NETWORK_ID" \
    --output-config "$config_path"
  python3 - "$config_path" "$mutation" "$TEST_OPERATOR_NETWORK_ID" "$STALE_NETWORK_ID" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
mutation = sys.argv[2]
expected_network_id = sys.argv[3]
stale_network_id = sys.argv[4]
text = path.read_text(encoding="utf-8")
if mutation == "archived-chain":
    text = text.replace(
        "fc56984b-2be7-431d-840e-21514d1883f0",
        "809574f5-fee7-5e69-bfcf-52451e42d50f",
        1,
    )
elif mutation == "wrong-discriminant":
    text = text.replace("chain_discriminant = 369", "chain_discriminant = 753", 1)
elif mutation == "wrong-network":
    text = text.replace(expected_network_id, stale_network_id, 1)
else:
    raise SystemExit(f"unknown mutation: {mutation}")
path.write_text(text, encoding="utf-8")
PY

  if PATH="${root}/mockbin:${PATH}" \
      MOCK_SCENARIO="cargo_success" \
      MOCK_STATE_DIR="${root}/state" \
      "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
        --skip-local \
        --public-root https://taira.sora.org \
        --write-config "$config_path" \
        --iroha-bin "${root}/mockbin/iroha" \
        >"$output_file" 2>&1; then
    echo "invalid canary identity case ${mutation} unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
  if ! grep -q "$expected_pattern" "$output_file"; then
    echo "invalid canary identity case ${mutation} did not emit expected pattern: ${expected_pattern}" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
}

run_invalid_topology_config_case() {
  local forbidden_dataspace="$1"
  local forbidden_id="$2"
  local root output_file config_path

  root="$(make_temp_dir)"
  cleanup_paths+=("$root")
  make_fake_repo "$root"
  output_file="${root}/invalid-${forbidden_dataspace}-dataspace.log"
  config_path="${root}/configs/soranexus/taira/config.toml"
  python3 - "$config_path" "$forbidden_dataspace" "$forbidden_id" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
alias = sys.argv[2]
dataspace_id = int(sys.argv[3])
text = path.read_text(encoding="utf-8")
marker = "\n[nexus.governance]\n"
if marker not in text:
    raise SystemExit("mock Taira config omitted the governance marker")
extra = (
    "\n[[nexus.dataspace_catalog]]\n"
    f'alias = "{alias}"\n'
    f"id = {dataspace_id}\n"
    f'description = "invalid promoted {alias} lane"\n'
)
path.write_text(text.replace(marker, extra + marker, 1), encoding="utf-8")
PY

  if PATH="${root}/mockbin:${PATH}" \
      MOCK_SCENARIO="cargo_success" \
      MOCK_STATE_DIR="${root}/state" \
      "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
        --skip-local \
        --public-root https://taira.sora.org \
        --skip-write-canary \
        --iroha-bin "${root}/mockbin/iroha" \
        >"$output_file" 2>&1; then
    echo "invalid ${forbidden_dataspace} dataspace config unexpectedly succeeded" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
  if ! grep -q \
      "logical lane aliases must not be physical dataspaces: ${forbidden_dataspace}" \
      "$output_file"; then
    echo "invalid ${forbidden_dataspace} dataspace config did not emit the topology error" >&2
    sed -n '1,200p' "$output_file" >&2 || true
    return 1
  fi
}

run_case txn_expired 'write canary failed: transaction expired' '"commit_qc_height": 707'
run_case permission_403 'write canary failed: signer or permission check returned 403' '"blocks": 707'
run_case public_502 'public Torii ingress looks degraded' 'HTTP 502'
run_case public_503 'public Torii ingress looks degraded' 'HTTP 503'
run_case public_503_mcp 'public MCP ingress looks degraded' 'HTTP 503'
run_case initialized_timeout 'initialized notification failed with HTTP curl_error_28' 'Operation timed out'
run_case public_health_unavailable '/health failed with HTTP 503'
run_case public_readyz_unavailable '/readyz failed with HTTP 503'
run_case fleet_health_unavailable 'validator validator-4: /health failed with HTTP 503'
run_case fleet_readyz_unavailable 'validator validator-4: /readyz failed with HTTP 503'
run_case lane_count_regressed 'Taira lane/dataspace topology mismatch: lane_count must be exactly 7'
run_case governance_promoted_to_dataspace 'Taira lane/dataspace topology mismatch'
run_case zk_promoted_to_dataspace 'Taira lane/dataspace topology mismatch'
run_case dataspace_dpn_missing 'Taira lane/dataspace topology mismatch'
run_case dataspace_cbsi_missing 'Taira lane/dataspace topology mismatch'
run_case dataspace_is2_missing 'Taira lane/dataspace topology mismatch'
run_case dataspace_is_id_mismatch 'Taira lane/dataspace topology mismatch'
run_case time_unhealthy '/v1/time/now is not release-ready: sample_count must be a positive integer'
run_case time_warn_enforcement '/v1/time/now is not release-ready: fail-closed time enforcement is not active'
run_case status_build_sha_missing '/status did not publish build.git_commit_sha' '' '490dacc'
run_case status_build_sha_too_short '/status build git SHA 490dac is not a 7 to 40 character hexadecimal SHA prefix' '' '490dacc'
run_case status_build_sha_mismatch '/status build git SHA 94dcbf7c28 does not exactly match release commit 490dacc287f00d490dacc287f00d490dacc287f0' '' '490dacc'
run_case status_dpn_commit_mismatch '/status DPN validator release commit eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee does not exactly match dddddddddddddddddddddddddddddddddddddddd'
run_case public_wrong_routing_matcher 'expected exact ordered rule tuples'
run_case sumeragi_missing_restart_required 'v2 status restart_required must be a boolean'
run_case sumeragi_invalid_restart_required 'v2 status restart_required must be a boolean'
run_case sumeragi_highest_qc_behind_commit 'durable CommitQC height does not match last_committed_height'
run_case sumeragi_locked_qc_behind_commit 'durable CommitQC does not satisfy its frozen dual quorum'
run_case sumeragi_idle_high_view_missing_qc 'v2 status omitted required last_commit_qc object'
run_case post_canary_sumeragi_missing_validator_set '/v1/sumeragi/status still did not publish a healthy commit QC snapshot after the signed write canary' 'reported invalid height_context.validator_count: 0'
run_case post_canary_wrong_routing_matcher '/status still did not publish a healthy snapshot after the signed write canary' 'expected exact ordered rule tuples'
run_case canonical_status_missing 'canonical pipeline transaction-status route should reject a missing hash failed with HTTP 404'
run_case canonical_status_wrong_error "canonical pipeline transaction-status route should reject a missing hash returned error code 'bad_request'"
run_case retired_status_alias_mounted 'retired transaction-status compatibility route must remain unmounted failed with HTTP 200'
run_case retired_status_wrong_error "retired transaction-status compatibility route must remain unmounted returned error code 'not_found'"
run_case fleet_commit_mismatch 'disagrees with validator-1 on committed_block_hash'
run_case fleet_dataspace_mismatch 'Taira lane/dataspace topology mismatch'
run_case fleet_catalog_changes_between_samples 'disagrees with validator-1 on dataspace_catalog'
run_case fleet_missing_dataspace_roster "physical dataspace 'cbsi' lacks a non-empty manifest validator roster"
run_case fleet_missing_universal_roster "lane 'governance' requires a ready manifest with a non-empty validator roster"
run_case fleet_invalid_dataspace_quorum "lane 'dpn' manifest quorum 2 is invalid for 4 validators"
run_case fleet_missing_binding_projection 'manifest_validator_bindings is not an array'
run_case fleet_binding_unknown_field 'fields must be exactly'
run_case fleet_binding_missing_field 'fields must be exactly'
run_case fleet_binding_account_mismatch 'roster does not exactly match its validator-binding account set'
run_case fleet_duplicate_binding_peer 'contains duplicate PeerIds'
run_case fleet_duplicate_binding_torii 'contains duplicate Torii origins'
run_case fleet_same_dataspace_binding_mismatch "lanes in physical dataspace 'universal' project different validator rosters, bindings, or quorums"
run_case fleet_partial_account_reuse 'same manifest validator account'
run_case fleet_partial_peer_reuse 'same manifest PeerId'
run_case fleet_partial_torii_reuse 'same manifest Torii origin'
run_case fleet_noncanonical_torii 'torii_url is not canonical'
run_case fleet_repeated_dataspace_roster "physical dataspaces 'is' and 'is2' reuse the same validator roster"
run_case fleet_repeated_universal_roster "physical dataspaces 'universal' and 'dpn' reuse the same validator roster"
run_case fleet_same_dataspace_roster_mismatch "lanes in physical dataspace 'universal' project different validator rosters, bindings, or quorums"
run_case fleet_missing_routing_policy '/status.nexus.routing_policy is not an object'
run_case fleet_wrong_routing_matcher 'expected exact ordered rule tuples'
run_case fleet_bindings_change_between_samples 'validator fleet changed dataspace_rosters between progress samples'
run_case fleet_stale_commit_progress 'validator fleet did not advance a common committed height'
run_case fleet_lagging_status_blocks '/status.blocks 706 does not match the durable committed height 707'
run_case fleet_committed_hash_trailing_bytes 'durable committed subject omitted a canonical block hash'
run_invalid_canary_identity_case \
  archived-chain \
  'write canary config must target the expected Taira chain'
run_invalid_canary_identity_case \
  wrong-discriminant \
  'write canary config must use Taira chain discriminant 369'
run_invalid_canary_identity_case \
  wrong-network \
  'write canary config `network_id` must match the exact operator NetworkId'
run_invalid_topology_config_case governance 1
run_invalid_topology_config_case zk 2

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="ordinary_health_ready" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --skip-write-canary \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/readyz-healthy-output.log" 2>&1; then
  echo "ordinary /health and /readyz case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/readyz-healthy-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' "${root}/readyz-healthy-output.log"
grep -qx 'https://taira.sora.org/readyz' "${root}/state/readyz_seen"
grep -qx 'https://validator-1.test/readyz' "${root}/state/readyz_seen"
grep -qx 'https://validator-2.test/readyz' "${root}/state/readyz_seen"
grep -qx 'https://validator-3.test/readyz' "${root}/state/readyz_seen"
grep -qx 'https://validator-4.test/readyz' "${root}/state/readyz_seen"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
archived_chain_id="809574f5-fee7-5e69-bfcf-52451e42d50f"
"${root}/scripts/taira_bootstrap_canary.py" \
  --onboarding-token-file "${root}/state/onboarding-token" \
  --chain-id "$archived_chain_id" \
  --network-id "$TEST_OPERATOR_NETWORK_ID" \
  --output-config "${root}/archived-canary.toml"
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="failed_asset_then_success" \
    MOCK_STATE_DIR="${root}/state" \
    POST_CANARY_STATUS_RECHECK_ATTEMPTS=2 \
    POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=0 \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --expected-chain-id "$archived_chain_id" \
      --write-config "${root}/archived-canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/archived-chain-override-output.log" 2>&1; then
  echo "explicit archived-chain override case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/archived-chain-override-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' \
  "${root}/archived-chain-override-output.log"
test -f "${root}/state/faucet_seen"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --expected-chain-id not-a-uuid \
      --skip-write-canary \
      >"${root}/invalid-chain-id-output.log" 2>&1; then
  echo "invalid expected chain ID case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-chain-id-output.log" >&2 || true
  exit 1
fi
grep -q -- '--expected-chain-id must be one canonical UUID' \
  "${root}/invalid-chain-id-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
release_script="${root}/configs/soranexus/taira/check_mcp_rollout.real.sh"
release_operator_args=(
  --operator-network-id
  "hash:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa#0000"
  --operator-private-key-file
  "${root}/state/operator-private-key"
)
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "$release_script" \
      --skip-local \
      --public-root https://taira.sora.org \
      "${release_operator_args[@]}" \
      --skip-write-canary \
      >"${root}/release-without-fleet-output.log" 2>&1; then
  echo "public release without validator fleet unexpectedly succeeded" >&2
  exit 1
fi
grep -q 'public Taira rollout requires --require-all-validators' \
  "${root}/release-without-fleet-output.log"

release_fleet_args=(
  --validator-root validator-1=https://validator-1.test
  --validator-root validator-2=https://validator-2.test
  --validator-root validator-3=https://validator-3.test
  --validator-root validator-4=https://validator-4.test
  --require-all-validators
)
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "$release_script" \
      --skip-local \
      --public-root https://taira.sora.org \
      "${release_fleet_args[@]}" \
      "${release_operator_args[@]}" \
      --skip-write-canary \
      >"${root}/release-without-sha-output.log" 2>&1; then
  echo "public release without exact git SHA unexpectedly succeeded" >&2
  exit 1
fi
grep -q 'public Taira rollout requires --expected-git-sha with the exact full 40-character commit' \
  "${root}/release-without-sha-output.log"

if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "$release_script" \
      --skip-local \
      --public-root https://taira.sora.org \
      "${release_fleet_args[@]}" \
      "${release_operator_args[@]}" \
      --expected-git-sha 490dacc287f00d490dacc287f00d490dacc287f0 \
      --skip-write-canary \
      >"${root}/release-without-dpn-output.log" 2>&1; then
  echo "public release without exact DPN release commit unexpectedly succeeded" >&2
  exit 1
fi
grep -q 'public Taira rollout requires --expected-dpn-validator-release-commit with the exact full 40-character commit' \
  "${root}/release-without-dpn-output.log"

if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    VALIDATOR_PROGRESS_SAMPLES=2 \
    "$release_script" \
      --skip-local \
      --public-root https://taira.sora.org \
      "${release_fleet_args[@]}" \
      "${release_operator_args[@]}" \
      --expected-git-sha 490dacc287f00d490dacc287f00d490dacc287f0 \
      --expected-dpn-validator-release-commit dddddddddddddddddddddddddddddddddddddddd \
      --skip-write-canary \
      >"${root}/release-too-few-samples-output.log" 2>&1; then
  echo "public release with fewer than three fleet samples unexpectedly succeeded" >&2
  exit 1
fi
grep -q 'public Taira rollout requires at least three advancing validator fleet samples' \
  "${root}/release-too-few-samples-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --write-config "${root}/missing-canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/missing-canary-output.log" 2>&1; then
  echo "explicit missing write-config case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/missing-canary-output.log" >&2 || true
  exit 1
fi
grep -q 'write canary config not found:' "${root}/missing-canary-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="success" \
    MOCK_STATE_DIR="${root}/state" \
    MOCK_OMIT_ONBOARDING_TOKEN_FILE=1 \
    WRITE_CONFIG_DEFAULT="${root}/automatic-canary.toml" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/missing-onboarding-token-output.log" 2>&1; then
  echo "automatic bootstrap without onboarding token unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/missing-onboarding-token-output.log" >&2 || true
  exit 1
fi
grep -q 'automatic canary bootstrap requires --onboarding-token-file ABSOLUTE_PATH' \
  "${root}/missing-onboarding-token-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
"${root}/scripts/taira_bootstrap_canary.py" \
  --onboarding-token-file "${root}/state/onboarding-token" \
  --network-id "$TEST_OPERATOR_NETWORK_ID" \
  --output-config "${root}/explicit-canary.toml"
before_hash="$(shasum -a 256 "${root}/explicit-canary.toml" | awk '{print $1}')"
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="success" \
    MOCK_STATE_DIR="${root}/state" \
    POST_CANARY_STATUS_RECHECK_ATTEMPTS=2 \
    POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=0 \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --write-config "${root}/explicit-canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/explicit-canary-output.log" 2>&1; then
  echo "explicit write-config preservation case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/explicit-canary-output.log" >&2 || true
  exit 1
fi
after_hash="$(shasum -a 256 "${root}/explicit-canary.toml" | awk '{print $1}')"
[[ "$before_hash" == "$after_hash" ]]
grep -q 'Taira MCP rollout checks passed.' "${root}/explicit-canary-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
"${root}/scripts/taira_bootstrap_canary.py" \
  --onboarding-token-file "${root}/state/onboarding-token" \
  --network-id "$TEST_OPERATOR_NETWORK_ID" \
  --output-config "${root}/asset-retry-canary.toml"
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="failed_asset_then_success" \
    MOCK_STATE_DIR="${root}/state" \
    ROLLOUT_CANARY_STATUS_TIMEOUT_MS=34567 \
    POST_CANARY_STATUS_RECHECK_ATTEMPTS=2 \
    POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=0 \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --write-config "${root}/asset-retry-canary.toml" \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/asset-retry-output.log" 2>&1; then
  echo "faucet asset-retry case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/asset-retry-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' "${root}/asset-retry-output.log"
test -f "${root}/state/faucet_seen"
grep -q '^34567$' "${root}/state/faucet_status_timeout_seen"
grep -q '^2$' "${root}/state/ping_calls"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --expected-git-sha bad \
      --skip-write-canary \
      >"${root}/invalid-sha-output.log" 2>&1; then
  echo "invalid expected git SHA case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-sha-output.log" >&2 || true
  exit 1
fi
grep -q -- '--expected-git-sha must be a 7 to 40 character hexadecimal git SHA prefix' \
  "${root}/invalid-sha-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --curl-connect-timeout-seconds 0 \
      --skip-write-canary \
      >"${root}/invalid-connect-timeout-output.log" 2>&1; then
  echo "invalid connect timeout case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-connect-timeout-output.log" >&2 || true
  exit 1
fi
grep -q 'MCP_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS must be a positive integer' \
  "${root}/invalid-connect-timeout-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --curl-max-time-seconds abc \
      --skip-write-canary \
      >"${root}/invalid-max-time-output.log" 2>&1; then
  echo "invalid max-time case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-max-time-output.log" >&2 || true
  exit 1
fi
grep -q 'MCP_ROLLOUT_CURL_MAX_TIME_SECONDS must be a positive integer' \
  "${root}/invalid-max-time-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --deadline-seconds 0 \
      --skip-write-canary \
      >"${root}/invalid-deadline-output.log" 2>&1; then
  echo "invalid rollout deadline case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-deadline-output.log" >&2 || true
  exit 1
fi
grep -q 'ROLLOUT_DEADLINE_SECONDS must be a positive integer' \
  "${root}/invalid-deadline-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    MOCK_CURL_DELAY_SECONDS=2 \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --deadline-seconds 2 \
      --skip-write-canary \
      >"${root}/deadline-output.log" 2>&1; then
  echo "absolute rollout deadline case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/deadline-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout exceeded its absolute 2-second deadline' \
  "${root}/deadline-output.log"
if ! awk 'NF != 2 || $1 < 1 || $1 > 2 || $2 < 1 || $2 > 2 { exit 1 }' \
    "${root}/state/curl_timeouts"; then
  echo "absolute deadline case did not clamp curl timeout flags to the remaining budget" >&2
  cat "${root}/state/curl_timeouts" >&2
  exit 1
fi

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    POST_CANARY_STATUS_RECHECK_DELAY_SECONDS=abc \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --skip-write-canary \
      >"${root}/invalid-recheck-delay-output.log" 2>&1; then
  echo "invalid recheck delay case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-recheck-delay-output.log" >&2 || true
  exit 1
fi
grep -q 'POST_CANARY_STATUS_RECHECK_DELAY_SECONDS must be a non-negative integer' \
  "${root}/invalid-recheck-delay-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    PUBLIC_LANE_ID='0/../../status' \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --skip-write-canary \
      >"${root}/invalid-public-lane-output.log" 2>&1; then
  echo "invalid public lane case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/invalid-public-lane-output.log" >&2 || true
  exit 1
fi
grep -q 'PUBLIC_LANE_ID must be a non-negative integer' \
  "${root}/invalid-public-lane-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="txn_expired" \
    MOCK_STATE_DIR="${root}/state" \
    WRITE_CONFIG_DEFAULT="${root}/canary.toml" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --resolve-host taira.sora.org:443:127.0.0.1 \
      --iroha-bin "${root}/mockbin/iroha" \
      >"${root}/resolve-output.log" 2>&1; then
  echo "resolve-host case unexpectedly succeeded" >&2
  sed -n '1,200p' "${root}/resolve-output.log" >&2 || true
  exit 1
fi
grep -q 'write canary failed: transaction expired' "${root}/resolve-output.log"

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
if ! PATH="${root}/mockbin:${PATH}" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      --curl-connect-timeout-seconds 7 \
      --curl-max-time-seconds 33 \
      --skip-write-canary \
      >"${root}/curl-timeout-output.log" 2>&1; then
  echo "custom curl timeout case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/curl-timeout-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' "${root}/curl-timeout-output.log"
if grep -vq '^7 33$' "${root}/state/curl_timeouts"; then
  echo "custom curl timeout case did not pass custom timeout flags to every curl call" >&2
  cat "${root}/state/curl_timeouts" >&2
  exit 1
fi

root="$(make_temp_dir)"
cleanup_paths+=("$root")
make_fake_repo "$root"
rm -f "${root}/mockbin/iroha"
mkdir -p "${root}/tmp"
mkdir -p "${root}/pythonbin"
ln -s "$(command -v python3)" "${root}/pythonbin/python3"
if ! PATH="${root}/mockbin:${root}/pythonbin:/usr/bin:/bin" \
    TMPDIR="${root}/tmp/" \
    MOCK_SCENARIO="cargo_success" \
    MOCK_STATE_DIR="${root}/state" \
    WRITE_CONFIG_DEFAULT="${root}/tmp/taira-canary-client.toml" \
    "${root}/configs/soranexus/taira/check_mcp_rollout.sh" \
      --skip-local \
      --public-root https://taira.sora.org \
      >"${root}/cargo-output.log" 2>&1; then
  echo "cargo fallback case unexpectedly failed" >&2
  sed -n '1,200p' "${root}/cargo-output.log" >&2 || true
  exit 1
fi
grep -q 'Taira MCP rollout checks passed.' "${root}/cargo-output.log"
test -f "${root}/state/cargo_seen"
test -f "${root}/tmp/taira-canary-client.toml"
test -f "${root}/state/onboarding_token_seen"
grep -Fqx "$TEST_OPERATOR_NETWORK_ID" \
  "${root}/state/bootstrap_network_id_seen"

echo "check_mcp_rollout mock tests passed."
