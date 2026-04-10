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
IROHA_BIN="${IROHA_BIN:-}"
RUN_DEPLOY=0
RUN_SMOKE=0
RUN_RELEASE_CHECKLIST=0
ALLOW_TESTNET_MUTATIONS=0
SKIP_MCP_CHECK=0
SKIP_NESTED_CALL=0

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
                                  [--skip-nested-call]

Run the post-upgrade public-Taira validation chain in the canonical order:
  1. `check_mcp_rollout.sh` on the chosen public node
  2. `make testnet-nested-call-probe` in `../soraswap`
  3. optional `make deploy-testnet`
  4. optional signed `make smoke-testnet`
  5. optional `make release-checklist`

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
    --skip-nested-call)
      SKIP_NESTED_CALL=1
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

if [[ $SKIP_MCP_CHECK -ne 1 ]]; then
  if [[ -z "$PUBLIC_TORII_ROOT" ]]; then
    echo "--public-root is required unless --skip-mcp-check is set" >&2
    exit 1
  fi
  if [[ -z "$WRITE_CONFIG" ]]; then
    echo "--write-config is required unless --skip-mcp-check is set" >&2
    exit 1
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
