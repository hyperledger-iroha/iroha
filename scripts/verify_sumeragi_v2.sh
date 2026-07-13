#!/usr/bin/env bash
set -euo pipefail

EXPECTED_VERUS_VERSION="0.2026.05.31.5dd6d83"
EXPECTED_VERUS_TOOLCHAIN_VERSION="1.95.0"
EXPECTED_VSTD_VERSION="0.0.0-2026-05-31-0205"
REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PRODUCTION_CORE_DIR="$REPO_ROOT/crates/iroha_core/src/sumeragi/v2_core"
VERUS_LOG="${REPO_ROOT}/target/formal/sumeragi_v2/verus.log"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

command -v verus >/dev/null 2>&1 || {
  echo "verus ${EXPECTED_VERUS_VERSION} is required in PATH" >&2
  exit 1
}
command -v cargo-verus >/dev/null 2>&1 || {
  echo "cargo-verus ${EXPECTED_VERUS_VERSION} is required in PATH" >&2
  exit 1
}

actual_version="$(verus --version | awk '/Version:/ {print $2; exit}')"
if [[ "$actual_version" != "$EXPECTED_VERUS_VERSION" ]]; then
  echo "expected Verus ${EXPECTED_VERUS_VERSION}, found ${actual_version:-unknown}" >&2
  exit 1
fi

actual_toolchain="$(verus --version | awk '/Toolchain:/ {print $2; exit}')"
if [[ "${actual_toolchain%%-*}" != "$EXPECTED_VERUS_TOOLCHAIN_VERSION" ]]; then
  echo "expected Verus Rust toolchain ${EXPECTED_VERUS_TOOLCHAIN_VERSION}, found ${actual_toolchain:-unknown}" >&2
  exit 1
fi

platform="$(verus --version | awk '/Platform:/ {print $2; exit}')"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    expected_verus_sha256="f11f8a863103a3c8fcaf27e6189edfdba31081516591365b5e29b0a66f570451"
    expected_cargo_verus_sha256="f918c6229c8d714640c9c9ec3d60b9c1d2e0aafc09bba8ff037332b04f85d078"
    ;;
  Linux-x86_64)
    expected_verus_sha256="c5911ee43c7a92c49a48d2c8646c604d252a38c71c87bda88ad4d33eb9e7e0fc"
    expected_cargo_verus_sha256="42a79c9afd700f8312a9ac7ab212070723e71beeb07f5ab855453010455bdc6d"
    ;;
  *)
    expected_verus_sha256="${SUMERAGI_VERUS_SHA256:-}"
    expected_cargo_verus_sha256="${SUMERAGI_CARGO_VERUS_SHA256:-}"
    if [[ -z "$expected_verus_sha256" || -z "$expected_cargo_verus_sha256" ]]; then
      echo "no pinned Verus checksums for host $(uname -s)-$(uname -m) (${platform:-unknown}); set SUMERAGI_VERUS_SHA256 and SUMERAGI_CARGO_VERUS_SHA256" >&2
      exit 1
    fi
    ;;
esac

actual_verus_sha256="$(sha256_file "$(command -v verus)")"
actual_cargo_verus_sha256="$(sha256_file "$(command -v cargo-verus)")"
if [[ "$actual_verus_sha256" != "$expected_verus_sha256" ]]; then
  echo "Verus binary checksum mismatch for ${platform}" >&2
  exit 1
fi
if [[ "$actual_cargo_verus_sha256" != "$expected_cargo_verus_sha256" ]]; then
  echo "cargo-verus binary checksum mismatch for ${platform}" >&2
  exit 1
fi

if ! rg -q "vstd = \{ version = \"=${EXPECTED_VSTD_VERSION}\", optional = true \}" \
  "$REPO_ROOT/crates/iroha_sumeragi_core/Cargo.toml"; then
  echo "iroha_sumeragi_core must pin vstd ${EXPECTED_VSTD_VERSION}" >&2
  exit 1
fi

bash "$REPO_ROOT/scripts/check_sumeragi_v2_package_layout.sh"

if rg -n '\b(assume|admit)\s*\(|external_body|external_[[:alnum:]_]*specification|assume_specification' \
  "$PRODUCTION_CORE_DIR" \
  "$REPO_ROOT/crates/iroha_sumeragi_core/src/verus_proofs.rs"; then
  echo "unreviewed Verus trust escape hatch found in iroha_sumeragi_core" >&2
  exit 1
fi

# TimeoutIntent is safety-critical for view closure and replay. Keep its WAL
# guard derived from the carried vote/context/QC primitives; a compressed
# caller-supplied validity or high-QC-match bit would reopen the projection
# gap even if the downstream theorem remained satisfiable.
if rg -U -n '(?s)TimeoutIntent\s*\{[^}]*\b(local_vote_valid|high_reference_matches)\b' \
  "$REPO_ROOT/crates/iroha_sumeragi_core/src/verus_proofs.rs"; then
  echo "TimeoutIntent WAL guard contains a caller-supplied predicate" >&2
  exit 1
fi
if ! rg -q 'pub proof fn timeout_intent_guard_is_derived_from_vote_and_frozen_context' \
  "$REPO_ROOT/crates/iroha_sumeragi_core/src/verus_proofs.rs"; then
  echo "TimeoutIntent primitive-guard proof is missing" >&2
  exit 1
fi

# Keep the explicit Verus-to-TLA action-name table from silently drifting.
# This is a spelling/existence guard, not a claim that two independently
# parsed action bodies are already proved equivalent.
tla_core="$REPO_ROOT/docs/formal/sumeragi_v2/SumeragiV2Core.tla"
verus_model="$REPO_ROOT/crates/iroha_sumeragi_core/src/verus_proofs.rs"
for action in \
  BeginLocalProposal BeginPrepare BeginObservePrepare BeginLockCommit \
  BeginTimeout BeginInstallTC BeginDecision PersistProposal PersistPrepare \
  PersistObservePrepare PersistLockCommit PersistTimeout PersistInstallTC \
  PersistDecision DeliverProposal DeliverVote DeliverQC DeliverTimeout \
  DeliverTC FetchBody StoreBody ValidateBody CompleteProposalSignature \
  CompleteVoteSignature CompleteTimeoutSignature FormPrepareQC FormCommitQC \
  FormTC ApplyDecision; do
  if ! rg -q "^${action}\\(" "$tla_core"; then
    echo "mapped TLA+ action ${action} is missing from SumeragiV2Core.tla" >&2
    exit 1
  fi
  if ! rg -q "^[[:space:]]*${action}," "$verus_model"; then
    echo "mapped TLA+ action ${action} is missing from verus_proofs.rs" >&2
    exit 1
  fi
done

cd "$REPO_ROOT"
mkdir -p "$(dirname -- "$VERUS_LOG")"

# A clean target is intentional.  Pinned vstd uses reviewed trusted
# specifications, so root-only forwarding is required: `--no-cheating` applies
# to this crate while dependencies are still verified under their upstream
# trust policy.  The source scan remains a defense against unsupported syntax
# that a future Verus parser might otherwise stop classifying as cheating.
verify_workspace="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-verus-workspace.XXXXXX")"
cleanup_paths=("$verify_workspace")
if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
  CARGO_TARGET_DIR="$(mktemp -d "${TMPDIR:-/tmp}/sumeragi-v2-verus.XXXXXX")"
  cleanup_paths+=("$CARGO_TARGET_DIR")
  export CARGO_TARGET_DIR
fi
cleanup() {
  rm -rf -- "${cleanup_paths[@]}"
}
trap cleanup EXIT

# Keep verification-only vstd resolution and its generated lockfile out of the
# production workspace. The temporary tree preserves the repository-relative
# relationship between the formal harness and the authoritative package-local
# reducer modules.
mkdir -p \
  "$verify_workspace/crates" \
  "$verify_workspace/crates/iroha_core/src/sumeragi"
cp -R \
  "$REPO_ROOT/crates/iroha_sumeragi_core" \
  "$verify_workspace/crates/iroha_sumeragi_core"
cp -R \
  "$PRODUCTION_CORE_DIR" \
  "$verify_workspace/crates/iroha_core/src/sumeragi/v2_core"
cat >"$verify_workspace/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/iroha_sumeragi_core"]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.92"
version = "2.0.0-rc.2.0"
authors = ["Iroha 2 contributors"]
description = "Pure Sumeragi v2 verification workspace"
repository = "https://github.com/hyperledger-iroha/iroha"
documentation = "https://docs.iroha.tech"
homepage = "https://iroha.tech"
license = "Apache-2.0"
keywords = ["blockchain", "consensus"]
categories = ["algorithms"]

[workspace.lints.rust]
unsafe_code = "deny"
EOF

cd "$verify_workspace"

# Resolve the exact pinned optional vstd dependency only inside the disposable
# workspace, freeze that resolution, and first run precisely the six
# deterministic lossy/partition/crash simulations against the production
# reducer. `--offline` on both test invocations prevents resolution drift after
# the lock has been created.
cargo generate-lockfile
network_test_list="$(
  cargo test --locked --offline -p iroha_sumeragi_core \
    --test network_simulation -- --list
)"
network_test_count="$(
  printf '%s\n' "$network_test_list" |
    awk '/: test$/ { count += 1 } END { print count + 0 }'
)"
if [[ "$network_test_count" != "6" ]]; then
  echo "expected six Sumeragi v2 network simulations, found $network_test_count" >&2
  printf '%s\n' "$network_test_list" >&2
  exit 1
fi
cargo test --locked --offline -p iroha_sumeragi_core \
  --test network_simulation -- --test-threads=1

cargo verus verify -p iroha_sumeragi_core --features verus \
  --fwd-verus-args-to roots -- \
  --rlimit 60 \
  --expand-errors \
  --no-cheating \
  2>&1 | tee "$VERUS_LOG"
