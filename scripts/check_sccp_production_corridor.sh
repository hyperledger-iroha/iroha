#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/sccp-production-corridor}"
NORITO_SKIP_BINDINGS_SYNC="${NORITO_SKIP_BINDINGS_SYNC:-1}"
SCCP_CORRIDOR_NODE_BIN="${SCCP_CORRIDOR_NODE_BIN:-node}"
SCCP_CORRIDOR_PYTHON_BIN="${SCCP_CORRIDOR_PYTHON_BIN:-python3}"
DRY_RUN=0
LOG_DIR=""

ALL_PHASES=(
  rust-sccp
  evidence-scripts
  js-sdk
  python-sdk
  swift-sdk
  kotlin-sdk
  java-android
  dotnet-sdk
  contract-smoke
  core-admission
)

SELECTED_PHASES=()

usage() {
  cat <<'EOF'
Usage: scripts/check_sccp_production_corridor.sh [OPTIONS]

Run the focused SCCP production-readiness validation corridor.

Options:
  --phase NAME     Run one phase. Repeatable; comma-separated names are accepted.
  --dry-run        Print selected phase commands without executing them.
  --log-dir DIR    Run each selected phase in its own corridor invocation and
                   tee strict phase transcripts to DIR/<phase>.log.
  --list           Print available phases and exit.
  -h, --help       Show this help.

Environment:
  CARGO_TARGET_DIR             Cargo target directory for Rust phases.
                               Defaults to target/sccp-production-corridor.
  NORITO_SKIP_BINDINGS_SYNC    Defaults to 1 for focused Rust validation.
  JAVA_HOME                    JDK 21 for Gradle phases. Falls back to
                               target/java/jdk-21/Contents/Home, then
                               /usr/libexec/java_home -v 21 on macOS, then
                               Homebrew openjdk@21 installations.
  ANDROID_HOME                 Android SDK for the Java Android phase.
                               Defaults to ~/Library/Android/sdk when present.
  ANDROID_SDK_ROOT             Defaults to ANDROID_HOME when unset.
  DOTNET_ROOT                  .NET SDK root for the native C# SCCP phase.
                               Falls back to /tmp/iroha-dotnet/sdk, then dotnet
                               on PATH.
  SCCP_CORRIDOR_NODE_BIN       Node runtime for JavaScript and contract phases.
                               Defaults to node.
  SCCP_CORRIDOR_PYTHON_BIN     Python runtime for evidence and Python SDK phases.
                               Defaults to python3.
EOF
}

list_phases() {
  printf 'Available SCCP production corridor phases:\n'
  local phase
  for phase in "${ALL_PHASES[@]}"; do
    printf '  %s\n' "$phase"
  done
}

is_known_phase() {
  case "$1" in
    rust-sccp|evidence-scripts|js-sdk|python-sdk|swift-sdk|kotlin-sdk|java-android|dotnet-sdk|contract-smoke|core-admission)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

add_phase() {
  local value="$1"
  local parts=()
  local phase
  IFS=',' read -r -a parts <<<"$value"
  for phase in "${parts[@]}"; do
    if [[ -z "$phase" ]]; then
      echo "Empty --phase value is not valid." >&2
      exit 2
    fi
    if ! is_known_phase "$phase"; then
      echo "Unknown SCCP production corridor phase: $phase" >&2
      list_phases >&2
      exit 2
    fi
    SELECTED_PHASES+=("$phase")
  done
}

phase_selected() {
  local requested="$1"
  local phase
  if [[ ${#SELECTED_PHASES[@]} -eq 0 ]]; then
    return 0
  fi
  for phase in "${SELECTED_PHASES[@]}"; do
    if [[ "$phase" == "$requested" ]]; then
      return 0
    fi
  done
  return 1
}

selected_phases() {
  local phase
  for phase in "${ALL_PHASES[@]}"; do
    if phase_selected "$phase"; then
      printf '%s\n' "$phase"
    fi
  done
}

print_cmd() {
  printf '+'
  local arg
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf '\n'
}

run_cmd() {
  print_cmd "$@"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    return 0
  fi
  "$@"
}

run_in_dir() {
  local dir="$1"
  shift
  printf '+ (cd %q &&' "$dir"
  local arg
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf ')\n'
  if [[ "$DRY_RUN" -eq 1 ]]; then
    return 0
  fi
  (cd "$dir" && "$@")
}

ensure_swift_bridge_artifact() {
  local bridge_dir="$ROOT/dist/NoritoBridge.xcframework"
  local bridge_zip="$ROOT/dist/NoritoBridge.xcframework.zip"
  local rust_targets=(
    aarch64-apple-ios
    aarch64-apple-ios-sim
    x86_64-apple-ios
    aarch64-apple-darwin
  )

  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -f "$bridge_zip" ]]; then
      run_cmd rm -rf "$bridge_dir"
      run_cmd unzip -q -o "$bridge_zip" -d "$ROOT/dist"
    else
      run_cmd rustup target add "${rust_targets[@]}"
      run_cmd bash "$ROOT/scripts/build_norito_xcframework.sh"
    fi
    return 0
  fi

  if [[ -f "$bridge_dir/Info.plist" ]]; then
    return 0
  fi

  if [[ -f "$bridge_zip" ]]; then
    run_cmd rm -rf "$bridge_dir"
    run_cmd unzip -q -o "$bridge_zip" -d "$ROOT/dist"
  else
    run_cmd rustup target add "${rust_targets[@]}"
    run_cmd bash "$ROOT/scripts/build_norito_xcframework.sh"
  fi

  if [[ ! -f "$bridge_dir/Info.plist" ]]; then
    echo "NoritoBridge.xcframework was not materialized at $bridge_dir." >&2
    echo "Provide $bridge_zip or ensure scripts/build_norito_xcframework.sh can build it." >&2
    return 1
  fi
}

resolve_java_home() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -n "${JAVA_HOME:-}" ]]; then
      printf '%s\n' "$JAVA_HOME"
    else
      printf '%s\n' "$ROOT/target/java/jdk-21/Contents/Home"
    fi
    return 0
  fi

  if [[ -n "${JAVA_HOME:-}" ]] && is_java_21_home "$JAVA_HOME"; then
    printf '%s\n' "$JAVA_HOME"
    return 0
  fi

  local bundled="$ROOT/target/java/jdk-21/Contents/Home"
  if is_java_21_home "$bundled"; then
    printf '%s\n' "$bundled"
    return 0
  fi

  if command -v /usr/libexec/java_home >/dev/null 2>&1; then
    local macos_java_home
    if macos_java_home="$(/usr/libexec/java_home -v 21 2>/dev/null)" \
      && is_java_21_home "$macos_java_home"; then
      printf '%s\n' "$macos_java_home"
      return 0
    fi
  fi

  local homebrew_candidates=(
    /opt/homebrew/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
    /usr/local/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home
    /opt/homebrew/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home
    /usr/local/Cellar/openjdk@21/*/libexec/openjdk.jdk/Contents/Home
  )
  local candidate
  for candidate in "${homebrew_candidates[@]}"; do
    if is_java_21_home "$candidate"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  echo "JDK 21 not found. Set JAVA_HOME, install the repo-local target/java/jdk-21 bundle, or install Homebrew openjdk@21." >&2
  return 1
}

is_java_21_home() {
  local java_home="$1"
  local version_line
  [[ -x "$java_home/bin/java" ]] || return 1
  version_line="$("$java_home/bin/java" -version 2>&1 | head -n 1)"
  [[ "$version_line" =~ version[[:space:]]+\"21(\.|\") ]]
}

run_java_version_check() {
  local java_home="$1"
  run_cmd \
    env "JAVA_HOME=$java_home" "PATH=$java_home/bin:$PATH" \
    java -version
}

resolve_android_home() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -n "${ANDROID_HOME:-}" ]]; then
      printf '%s\n' "$ANDROID_HOME"
    else
      printf '%s\n' "$HOME/Library/Android/sdk"
    fi
    return 0
  fi

  if [[ -n "${ANDROID_HOME:-}" && -d "$ANDROID_HOME" ]]; then
    printf '%s\n' "$ANDROID_HOME"
    return 0
  fi

  local default_sdk="$HOME/Library/Android/sdk"
  if [[ -d "$default_sdk" ]]; then
    printf '%s\n' "$default_sdk"
    return 0
  fi

  echo "Android SDK not found. Set ANDROID_HOME for the java-android phase." >&2
  return 1
}

resolve_dotnet() {
  if [[ "$DRY_RUN" -eq 1 ]]; then
    if [[ -n "${DOTNET_ROOT:-}" ]]; then
      printf '%s\n' "$DOTNET_ROOT/dotnet"
    else
      printf '%s\n' "$ROOT/target/dotnet/dotnet"
    fi
    return 0
  fi

  if [[ -n "${DOTNET_ROOT:-}" && -x "$DOTNET_ROOT/dotnet" ]]; then
    printf '%s\n' "$DOTNET_ROOT/dotnet"
    return 0
  fi

  local local_dotnet="/tmp/iroha-dotnet/sdk/dotnet"
  if [[ -x "$local_dotnet" ]]; then
    printf '%s\n' "$local_dotnet"
    return 0
  fi

  if command -v dotnet >/dev/null 2>&1; then
    command -v dotnet
    return 0
  fi

  echo "dotnet not found. Set DOTNET_ROOT, install /tmp/iroha-dotnet/sdk, or install dotnet on PATH." >&2
  return 1
}

phase_rust_sccp() {
  run_cmd \
    env "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" "NORITO_SKIP_BINDINGS_SYNC=$NORITO_SKIP_BINDINGS_SYNC" \
    cargo test -p iroha_sccp -- --nocapture
}

phase_evidence_scripts() {
  local tests=(
    pytests/scripts/check_sccp_production_corridor_test.py
    pytests/scripts/sccp_release_bundle_test.py
    pytests/scripts/sccp_release_readiness_report_test.py
    pytests/scripts/sccp_all_lanes_evidence_test.py
    pytests/scripts/sccp_eth_source_bridge_evidence_test.py
    pytests/scripts/sccp_bsc_source_bridge_evidence_test.py
    pytests/scripts/sccp_evm_destination_evidence_test.py
    pytests/scripts/sccp_evm_live_evidence_test.py
    pytests/scripts/sccp_evm_receipt_proof_evidence_test.py
    pytests/scripts/sccp_evm_source_live_evidence_test.py
    pytests/scripts/sccp_solana_destination_evidence_test.py
    pytests/scripts/sccp_solana_live_evidence_test.py
    pytests/scripts/sccp_solana_source_state_evidence_test.py
    pytests/scripts/sccp_ton_destination_evidence_test.py
    pytests/scripts/sccp_ton_live_evidence_test.py
    pytests/scripts/sccp_ton_source_state_evidence_test.py
    pytests/scripts/sccp_tron_live_evidence_test.py
    pytests/scripts/sccp_tron_source_bridge_evidence_test.py
  )
  run_cmd "$SCCP_CORRIDOR_PYTHON_BIN" -m pytest -q "${tests[@]}"
}

phase_js_sdk() {
  run_cmd "$SCCP_CORRIDOR_NODE_BIN" --test \
    javascript/iroha_js/test/sccpSolanaProver.test.js \
    javascript/iroha_js/test/sccpEthereumMainnet.test.js \
    javascript/iroha_js/test/sccpBscMainnet.test.js \
    javascript/iroha_js/test/package_dist.test.js \
    javascript/iroha_js/test/sccpPackageExports.test.js
}

phase_python_sdk() {
  run_cmd "$SCCP_CORRIDOR_PYTHON_BIN" -m pytest -q python/iroha_torii_client/tests/sccp_test.py
}

phase_swift_sdk() {
  ensure_swift_bridge_artifact
  run_in_dir "$ROOT/IrohaSwift" \
    swift test --filter SccpSolanaProverTests --disable-swift-testing
  run_in_dir "$ROOT/IrohaSwift" \
    swift test --filter \
      ToriiClientTests/testBridgeProofSubmitRequestBuildsSccpPayloadsFromSubmissions \
      --disable-swift-testing
}

phase_kotlin_sdk() {
  local java_home
  java_home="$(resolve_java_home)"
  run_java_version_check "$java_home"
  run_in_dir "$ROOT/kotlin" \
    env "JAVA_HOME=$java_home" "PATH=$java_home/bin:$PATH" \
    ./gradlew :core-jvm:test --console=plain --tests 'org.hyperledger.iroha.sdk.sccp.*'
}

phase_java_android() {
  local java_home
  local android_home
  local android_sdk_root
  local android_harness_mains
  java_home="$(resolve_java_home)"
  android_home="$(resolve_android_home)"
  android_sdk_root="${ANDROID_SDK_ROOT:-$android_home}"
  android_harness_mains="org.hyperledger.iroha.android.sccp.EvmSccpProverTests,org.hyperledger.iroha.android.sccp.SourceSccpProofsTests,org.hyperledger.iroha.android.sccp.TonSccpProverTests,org.hyperledger.iroha.android.sccp.TronSccpProverTests"
  run_java_version_check "$java_home"
  run_in_dir "$ROOT/java/iroha_android" \
    env "JAVA_HOME=$java_home" "ANDROID_HOME=$android_home" "ANDROID_SDK_ROOT=$android_sdk_root" "PATH=$java_home/bin:$PATH" \
    "ANDROID_HARNESS_MAINS=$android_harness_mains" \
    ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.GradleHarnessTests
  run_in_dir "$ROOT/java/iroha_android" \
    env "JAVA_HOME=$java_home" "ANDROID_HOME=$android_home" "ANDROID_SDK_ROOT=$android_sdk_root" "PATH=$java_home/bin:$PATH" \
    ./gradlew :core:test --console=plain --tests org.hyperledger.iroha.android.sccp.SolanaSccpProverTests
}

phase_dotnet_sdk() {
  local dotnet_cli
  local dotnet_root
  dotnet_cli="$(resolve_dotnet)"
  if [[ "$DRY_RUN" -eq 1 ]]; then
    dotnet_root="$(dirname "$dotnet_cli")"
  else
    dotnet_root="$(cd "$(dirname "$dotnet_cli")" && pwd)"
  fi
  run_in_dir "$ROOT/csharp" \
    env "DOTNET_ROOT=$dotnet_root" "DOTNET_CLI_TELEMETRY_OPTOUT=1" "DOTNET_CLI_UI_LANGUAGE=en" \
    "$dotnet_cli" test tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj \
    --filter "FullyQualifiedName~SccpEthereumMainnetTests|FullyQualifiedName~SccpBscMainnetTests" \
    --nologo
}

phase_contract_smoke() {
  run_cmd "$SCCP_CORRIDOR_NODE_BIN" --test scripts/sccp_tron_taira_xor_deploy.test.mjs scripts/sccp_taira_xor_contract.test.mjs
  run_cmd "$SCCP_CORRIDOR_NODE_BIN" --check contracts/evm/sccp/test/sccp_message_bridge_smoke.js
  run_cmd bash scripts/sccp_evm_contract_smoke.sh
}

phase_core_admission() {
  run_cmd \
    env "CARGO_TARGET_DIR=$CARGO_TARGET_DIR" "NORITO_SKIP_BINDINGS_SYNC=$NORITO_SKIP_BINDINGS_SYNC" \
    cargo test -p iroha_core --test bridge_proofs -- --nocapture
}

run_with_log_dir() {
  local phase
  mkdir -p "$LOG_DIR"
  while IFS= read -r phase; do
    [[ -n "$phase" ]] || continue
    bash "$ROOT/scripts/check_sccp_production_corridor.sh" \
      --phase "$phase" 2>&1 | tee "$LOG_DIR/$phase.log"
  done < <(selected_phases)
  printf '\nSCCP production corridor logs written to %s\n' "$LOG_DIR"
}

main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --phase)
        if [[ $# -lt 2 ]]; then
          echo "--phase requires a value." >&2
          exit 2
        fi
        add_phase "$2"
        shift 2
        ;;
      --phase=*)
        add_phase "${1#--phase=}"
        shift
        ;;
      --list)
        list_phases
        exit 0
        ;;
      --dry-run)
        DRY_RUN=1
        shift
        ;;
      --log-dir)
        if [[ $# -lt 2 ]]; then
          echo "--log-dir requires a directory." >&2
          exit 2
        fi
        LOG_DIR="$2"
        shift 2
        ;;
      --log-dir=*)
        LOG_DIR="${1#--log-dir=}"
        if [[ -z "$LOG_DIR" ]]; then
          echo "--log-dir requires a directory." >&2
          exit 2
        fi
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown option: $1" >&2
        usage >&2
        exit 2
        ;;
    esac
  done

  if [[ -n "$LOG_DIR" && "$DRY_RUN" -eq 0 ]]; then
    run_with_log_dir
    return 0
  fi

  if [[ -n "$LOG_DIR" ]]; then
    printf 'SCCP production corridor logs would be written to %s\n' "$LOG_DIR"
  fi

  local phase
  for phase in "${ALL_PHASES[@]}"; do
    if ! phase_selected "$phase"; then
      continue
    fi

    printf '\n==> SCCP production corridor: %s\n' "$phase"
    case "$phase" in
      rust-sccp)
        phase_rust_sccp
        ;;
      evidence-scripts)
        phase_evidence_scripts
        ;;
      js-sdk)
        phase_js_sdk
        ;;
      python-sdk)
        phase_python_sdk
        ;;
      swift-sdk)
        phase_swift_sdk
        ;;
      kotlin-sdk)
        phase_kotlin_sdk
        ;;
      java-android)
        phase_java_android
        ;;
      dotnet-sdk)
        phase_dotnet_sdk
        ;;
      contract-smoke)
        phase_contract_smoke
        ;;
      core-admission)
        phase_core_admission
        ;;
    esac
  done

  if [[ "$DRY_RUN" -eq 1 ]]; then
    printf '\nSCCP production corridor dry run completed.\n'
  else
    printf '\nSCCP production corridor completed.\n'
  fi
}

main "$@"
