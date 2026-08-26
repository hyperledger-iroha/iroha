: "${IROHA_BIN:?Set IROHA_BIN to the absolute path of the qualified same-revision iroha binary}"
: "${IROHA_BIN_SHA256:?Set IROHA_BIN_SHA256 to the lowercase SHA-256 of IROHA_BIN}"

if [[ "$IROHA_BIN" != /* || ! -f "$IROHA_BIN" || ! -x "$IROHA_BIN" || -L "$IROHA_BIN" ]]; then
  echo "IROHA_BIN must be an absolute, executable, non-symlinked regular file" >&2
  exit 1
fi
if [[ ! "$IROHA_BIN_SHA256" =~ ^[0-9a-f]{64}$ ]]; then
  echo "IROHA_BIN_SHA256 must be exactly 64 lowercase hexadecimal characters" >&2
  exit 1
fi
if command -v sha256sum >/dev/null 2>&1; then
  IROHA_BIN_ACTUAL_SHA256="$(sha256sum "$IROHA_BIN" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  IROHA_BIN_ACTUAL_SHA256="$(shasum -a 256 "$IROHA_BIN" | awk '{print $1}')"
else
  echo "A SHA-256 tool (sha256sum or shasum) is required to qualify IROHA_BIN" >&2
  exit 1
fi
if [[ "$IROHA_BIN_ACTUAL_SHA256" != "$IROHA_BIN_SHA256" ]]; then
  echo "IROHA_BIN does not match the operator-qualified same-revision SHA-256" >&2
  exit 1
fi

IROHA_CMD=("$IROHA_BIN")
