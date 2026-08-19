#!/usr/bin/env bash
set -euo pipefail

# Build the canonical daemon, Governance DAG, external signer, and client.
#
# Prerequisites: the repository Rust toolchain and Cargo dependencies. Set
# BUILD_PROFILE to select a non-default Cargo profile (for example `deploy`).

usage() {
  cat <<'EOF'
Usage: build_line.sh [-h|--help]

Build the canonical `iroha3d` daemon, `sorafs_governance_dag`, the Unix-only
`sorafs_external_software_signer`, and `iroha`. Set BUILD_PROFILE to select a
Cargo profile. Windows software-signer packaging is explicitly unsupported.
EOF
}

declare -a profile_flag=()
if [[ -n "${BUILD_PROFILE:-}" ]]; then
  profile_flag=(--profile "${BUILD_PROFILE}")
fi

case "${1-}" in
  "") ;;
  -h|--help)
    usage
    exit 0
    ;;
  *)
    printf 'Unknown argument: %s\n\n' "$1" >&2
    usage >&2
    exit 1
    ;;
esac

echo "Building canonical binaries (iroha, iroha3d, sorafs_governance_dag, external signer)..."
cargo build "${profile_flag[@]}" -p irohad -p iroha_cli --no-default-features \
  --features irohad/external-software-signer-bin,iroha_cli/cli \
  --bin iroha3d --bin sorafs_governance_dag \
  --bin sorafs_external_software_signer --bin iroha
