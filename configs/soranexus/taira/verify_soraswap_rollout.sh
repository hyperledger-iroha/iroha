#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IROHA_ROOT="$(cd -- "${SCRIPT_DIR}/../../.." && pwd)"
DEFAULT_SORASWAP_ROOT="${IROHA_ROOT}/../soraswap"
SORASWAP_ROOT="${SORASWAP_ROOT:-${DEFAULT_SORASWAP_ROOT}}"
SORASWAP_CLIENT_CONFIG="${SORASWAP_CLIENT_CONFIG:-}"
PUBLIC_TORII_ROOT="${PUBLIC_TORII_ROOT:-}"
LOCAL_TORII_ROOT="${LOCAL_TORII_ROOT:-}"
WRITE_CONFIG="${WRITE_CONFIG:-}"
WRITE_CONFIG_DEFAULT="${WRITE_CONFIG_DEFAULT:-/run/secrets/taira-canary-client.toml}"
IROHA_BIN="${IROHA_BIN:-}"
TRADER_APP_API_PROBE_ATTEMPTS="${TRADER_APP_API_PROBE_ATTEMPTS:-6}"
TRADER_APP_API_PROBE_INTERVAL_SECS="${TRADER_APP_API_PROBE_INTERVAL_SECS:-1}"
RUN_DEPLOY=0
RUN_SMOKE=0
RUN_RELEASE_CHECKLIST=0
ALLOW_TESTNET_MUTATIONS=0
SKIP_MCP_CHECK=0
SKIP_SORAFS_CHECK=0
SKIP_NESTED_CALL=0
SKIP_TRADER_APP_API_CHECK=0

usage() {
  cat <<'EOF'
Usage: verify_soraswap_rollout.sh --public-root URL --write-config PATH
                                  [--local-root URL]
                                  [--soraswap-root PATH]
                                  [--soraswap-client-config PATH]
                                  [--iroha-bin PATH]
                                  [--run-deploy]
                                  [--run-smoke]
                                  [--run-release-checklist]
                                  [--allow-testnet-mutations]
                                  [--skip-mcp-check]
                                  [--skip-sorafs-check]
                                  [--skip-nested-call]
                                  [--skip-trader-app-api-check]

Run the post-upgrade public-Taira validation chain in the canonical order:
  1. `check_mcp_rollout.sh` on the chosen public node
  2. `check_sorafs_rollout.sh` on the chosen public node
  3. trader app-api CID probe from `deployments/testnet/trader_api_bundle.latest.json` when present
  4. `make testnet-nested-call-probe` in `../soraswap`
  5. optional `make deploy-testnet`
  6. optional signed `make smoke-testnet`
  7. optional `make release-checklist`

`--run-smoke` implies deploy. `--run-release-checklist` implies both deploy
and smoke. Mutating smoke requires `--allow-testnet-mutations`.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --public-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --public-root" >&2
        exit 1
      }
      PUBLIC_TORII_ROOT="$2"
      shift 2
      ;;
    --local-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --local-root" >&2
        exit 1
      }
      LOCAL_TORII_ROOT="$2"
      shift 2
      ;;
    --write-config)
      [[ $# -ge 2 ]] || {
        echo "missing value for --write-config" >&2
        exit 1
      }
      WRITE_CONFIG="$2"
      shift 2
      ;;
    --soraswap-root)
      [[ $# -ge 2 ]] || {
        echo "missing value for --soraswap-root" >&2
        exit 1
      }
      SORASWAP_ROOT="$2"
      shift 2
      ;;
    --soraswap-client-config)
      [[ $# -ge 2 ]] || {
        echo "missing value for --soraswap-client-config" >&2
        exit 1
      }
      SORASWAP_CLIENT_CONFIG="$2"
      shift 2
      ;;
    --iroha-bin)
      [[ $# -ge 2 ]] || {
        echo "missing value for --iroha-bin" >&2
        exit 1
      }
      IROHA_BIN="$2"
      shift 2
      ;;
    --run-deploy)
      RUN_DEPLOY=1
      shift
      ;;
    --run-smoke)
      RUN_SMOKE=1
      shift
      ;;
    --run-release-checklist)
      RUN_RELEASE_CHECKLIST=1
      shift
      ;;
    --allow-testnet-mutations)
      ALLOW_TESTNET_MUTATIONS=1
      shift
      ;;
    --skip-mcp-check)
      SKIP_MCP_CHECK=1
      shift
      ;;
    --skip-sorafs-check)
      SKIP_SORAFS_CHECK=1
      shift
      ;;
    --skip-nested-call)
      SKIP_NESTED_CALL=1
      shift
      ;;
    --skip-trader-app-api-check)
      SKIP_TRADER_APP_API_CHECK=1
      shift
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

if [[ $RUN_RELEASE_CHECKLIST -eq 1 ]]; then
  RUN_SMOKE=1
  RUN_DEPLOY=1
fi
if [[ $RUN_SMOKE -eq 1 ]]; then
  RUN_DEPLOY=1
fi

if [[ $SKIP_MCP_CHECK -ne 1 || $SKIP_SORAFS_CHECK -ne 1 ]]; then
  if [[ -z "$PUBLIC_TORII_ROOT" ]]; then
    echo "--public-root is required unless both --skip-mcp-check and --skip-sorafs-check are set" >&2
    exit 1
  fi
fi

if [[ $SKIP_TRADER_APP_API_CHECK -ne 1 && -z "$PUBLIC_TORII_ROOT" ]]; then
  echo "--public-root is required unless --skip-trader-app-api-check is set" >&2
  exit 1
fi

if [[ $SKIP_MCP_CHECK -ne 1 || $SKIP_SORAFS_CHECK -ne 1 ]]; then
  if [[ -z "$WRITE_CONFIG" ]]; then
    WRITE_CONFIG="$WRITE_CONFIG_DEFAULT"
  fi
fi

if [[ $SKIP_NESTED_CALL -ne 1 || $RUN_DEPLOY -eq 1 || $RUN_SMOKE -eq 1 || $RUN_RELEASE_CHECKLIST -eq 1 ]]; then
  if [[ ! -d "$SORASWAP_ROOT" ]]; then
    echo "soraswap repo not found: $SORASWAP_ROOT" >&2
    exit 1
  fi
  if [[ -z "$SORASWAP_CLIENT_CONFIG" ]]; then
    default_client_config="${SORASWAP_ROOT}/config/testnet/taira.client.toml"
    if [[ -f "$default_client_config" ]]; then
      SORASWAP_CLIENT_CONFIG="$default_client_config"
    else
      echo "--soraswap-client-config is required when the default soraswap testnet config is absent" >&2
      exit 1
    fi
  fi
fi

if [[ $RUN_SMOKE -eq 1 && $ALLOW_TESTNET_MUTATIONS -ne 1 ]]; then
  echo "--run-smoke requires --allow-testnet-mutations" >&2
  exit 1
fi

run_step() {
  local label="$1"
  shift
  echo "==> $label"
  "$@"
}

probe_trader_app_api() {
  local trader_api_report="$SORASWAP_ROOT/deployments/testnet/trader_api_bundle.latest.json"
  local content_cid probe_url probe_body probe_http
  local probe_attempts probe_interval_secs attempt probe_results probe_error
  local success_count failure_count capacity_state_body capacity_state_http declaration_count

  if [[ ! -f "$trader_api_report" ]]; then
    echo "==> trader app-api CID route probe skipped: $trader_api_report not found"
    return 0
  fi

  content_cid="$(jq -r '.content_cid // empty' "$trader_api_report")"
  if [[ -z "$content_cid" ]]; then
    echo "trader app-api CID route probe failed: $trader_api_report does not include content_cid" >&2
    exit 1
  fi

  probe_url="${PUBLIC_TORII_ROOT%/}/v1/app-api/cid/${content_cid}"
  probe_body="$(mktemp)"
  probe_attempts="$TRADER_APP_API_PROBE_ATTEMPTS"
  probe_interval_secs="$TRADER_APP_API_PROBE_INTERVAL_SECS"
  probe_results=()
  success_count=0
  failure_count=0

  echo "==> trader app-api CID route probe: ${probe_url}"
  for ((attempt = 1; attempt <= probe_attempts; attempt++)); do
    if ! probe_http="$(curl --silent --show-error --output "$probe_body" --write-out '%{http_code}' "$probe_url")"; then
      probe_error="$(sed -n '1,40p' "$probe_body" 2>/dev/null || true)"
      probe_results+=("${attempt}:transport-error")
      if [[ -n "$probe_error" ]]; then
        probe_results+=("${attempt}:body=${probe_error}")
      fi
      ((failure_count += 1))
    elif [[ "$probe_http" == 2* ]]; then
      probe_results+=("${attempt}:${probe_http}")
      ((success_count += 1))
    else
      probe_results+=("${attempt}:${probe_http}")
      ((failure_count += 1))
    fi

    if [[ $attempt -lt $probe_attempts ]]; then
      sleep "$probe_interval_secs"
    fi
  done

  if [[ $failure_count -ne 0 ]]; then
    echo "trader app-api CID route probe saw inconsistent public responses (${success_count}/${probe_attempts} successes): ${probe_results[*]}" >&2
    sed -n '1,40p' "$probe_body" >&2 || true
    echo >&2
    capacity_state_body="$(mktemp)"
    if capacity_state_http="$(curl --silent --show-error --output "$capacity_state_body" --write-out '%{http_code}' "${PUBLIC_TORII_ROOT%/}/v1/sorafs/capacity/state")" \
      && [[ "$capacity_state_http" == "200" ]]; then
      declaration_count="$(python3 - "$capacity_state_body" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

declarations = payload.get("declarations")
if isinstance(declarations, list):
    print(len(declarations))
else:
    print(0)
PY
      )"
      echo "visible SoraFS capacity declarations on target: ${declaration_count}" >&2
    fi
    rm -f "$capacity_state_body"
    rm -f "$probe_body"
    exit 1
  fi

  rm -f "$probe_body"
}

if [[ $SKIP_MCP_CHECK -ne 1 ]]; then
  mcp_cmd=(
    "${SCRIPT_DIR}/check_mcp_rollout.sh"
    --public-root "$PUBLIC_TORII_ROOT"
    --write-config "$WRITE_CONFIG"
  )
  if [[ -n "$LOCAL_TORII_ROOT" ]]; then
    mcp_cmd+=(--local-root "$LOCAL_TORII_ROOT")
  else
    mcp_cmd+=(--skip-local)
  fi
  if [[ -n "$IROHA_BIN" ]]; then
    mcp_cmd+=(--iroha-bin "$IROHA_BIN")
  fi
  run_step "public Taira MCP + write canary" "${mcp_cmd[@]}"
fi

if [[ $SKIP_SORAFS_CHECK -ne 1 ]]; then
  sorafs_cmd=(
    "${SCRIPT_DIR}/check_sorafs_rollout.sh"
    --public-root "$PUBLIC_TORII_ROOT"
    --write-config "$WRITE_CONFIG"
  )
  if [[ -n "$IROHA_BIN" ]]; then
    sorafs_cmd+=(--iroha-bin "$IROHA_BIN")
  fi
  run_step "public Taira SoraFS + capacity canary" "${sorafs_cmd[@]}"
fi

if [[ $SKIP_TRADER_APP_API_CHECK -ne 1 ]]; then
  run_step "SoraSwap trader app-api CID route" probe_trader_app_api
fi

if [[ $SKIP_NESTED_CALL -ne 1 ]]; then
  run_step \
    "SoraSwap nested call probe" \
    env SORASWAP_CLIENT_CONFIG="$SORASWAP_CLIENT_CONFIG" make -C "$SORASWAP_ROOT" testnet-nested-call-probe
fi

if [[ $RUN_DEPLOY -eq 1 ]]; then
  run_step \
    "SoraSwap deploy-testnet" \
    env SORASWAP_CLIENT_CONFIG="$SORASWAP_CLIENT_CONFIG" make -C "$SORASWAP_ROOT" deploy-testnet
fi

if [[ $RUN_SMOKE -eq 1 ]]; then
  run_step \
    "SoraSwap smoke-testnet" \
    env SORASWAP_CLIENT_CONFIG="$SORASWAP_CLIENT_CONFIG" SORASWAP_ALLOW_TESTNET_MUTATIONS=1 make -C "$SORASWAP_ROOT" smoke-testnet
fi

if [[ $RUN_RELEASE_CHECKLIST -eq 1 ]]; then
  run_step \
    "SoraSwap release-checklist" \
    env SORASWAP_CLIENT_CONFIG="$SORASWAP_CLIENT_CONFIG" make -C "$SORASWAP_ROOT" release-checklist
fi

echo "Taira to SoraSwap rollout verification passed."
