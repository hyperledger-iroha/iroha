IROHA_CARGO=(cargo)
if [[ -n "${IROHA_CARGO_BIN:-}" ]]; then
  IROHA_CARGO=("${IROHA_CARGO_BIN}")
fi

IROHA_CARGO_ENV=()
if [[ -n "${IROHA_CARGO_HOME:-}" ]]; then
  IROHA_CARGO_ENV+=("CARGO_HOME=${IROHA_CARGO_HOME}")
fi
if [[ -n "${IROHA_CARGO_TARGET_DIR:-}" ]]; then
  IROHA_CARGO_ENV+=("CARGO_TARGET_DIR=${IROHA_CARGO_TARGET_DIR}")
fi
if [[ -n "${IROHA_CARGO_NET_OFFLINE:-}" ]]; then
  IROHA_CARGO_ENV+=("CARGO_NET_OFFLINE=${IROHA_CARGO_NET_OFFLINE}")
fi
if [[ -n "${IROHA_CARGO_BUILD_JOBS:-}" ]]; then
  IROHA_CARGO_ENV+=("CARGO_BUILD_JOBS=${IROHA_CARGO_BUILD_JOBS}")
fi

if [[ -n "${IROHA_BIN:-}" ]]; then
  IROHA_CMD=("${IROHA_BIN}")
elif command -v iroha >/dev/null 2>&1; then
  IROHA_CMD=("$(command -v iroha)")
elif [[ -n "${IROHA_SOURCE_DIR:-}" && -f "${IROHA_SOURCE_DIR}/Cargo.toml" ]]; then
  IROHA_CMD=(env "${IROHA_CARGO_ENV[@]}" "${IROHA_CARGO[@]}" run --manifest-path "${IROHA_SOURCE_DIR}/Cargo.toml" -p iroha_cli --bin iroha --)
elif [[ -n "${IROHA_MANIFEST_PATH:-}" && -f "${IROHA_MANIFEST_PATH}" ]]; then
  IROHA_CMD=(env "${IROHA_CARGO_ENV[@]}" "${IROHA_CARGO[@]}" run --manifest-path "${IROHA_MANIFEST_PATH}" -p iroha_cli --bin iroha --)
else
  echo "Unable to locate iroha. Set IROHA_BIN to a packaged binary or IROHA_SOURCE_DIR to an Iroha checkout." >&2
  exit 1
fi
