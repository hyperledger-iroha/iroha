#!/bin/bash
set -euo pipefail

# Makes possible to set e.g. `BIN_KAGAMI=target/release/kagami` without running cargo
bin_kagami=("${BIN_KAGAMI:-cargo run --release --bin kagami --}")
bin_iroha=("${BIN_IROHA:-cargo run --release --bin iroha --}")

# Track overall success/failure
exit_code=0
update=0

if [[ "${1:-}" == "--update" ]]; then
    update=1
    shift
fi

do_check() {
    local cmd="$1"
    local target="$2"

    if [[ "$update" -eq 1 ]]; then
        eval "$cmd" > "$target"
        echo "[UPDATED] $target"
    else
        if ! diff <(eval "$cmd") "$target" > /dev/null; then
            echo "[DIFF] $target is out of date"
            echo "Run with \"--update\" to regenerate automatically, or run manually:"
            echo "  $cmd > $target"
            exit_code=1
        else
            echo "[OK] $target is up to date"
        fi
    fi
}

do_check_swarm() {
    local peers="$1"
    local image="$2"
    local extra="$3"
    local target="$4"
    local cmd_base="${bin_kagami[@]} swarm --peers $peers --seed Iroha --healthcheck --config-dir ./defaults --image $image --print"
    do_check "$cmd_base --out-file $target $extra" "$target"
}

cmd_genesis="${bin_kagami[@]} genesis --creation-time 1970-01-01T00:00:00Z --executor executor.wasm --wasm-dir libs"
cmd_schema="${bin_kagami[@]} schema"
cmd_iroha_help="${bin_iroha[@]} markdown-help"
cmd_kagami_help="${bin_kagami[@]} markdown-help"

tasks=()

case "${1:-}" in
    "all")
        tasks=(genesis schema cli-help docker-compose)
        ;;
    "genesis"|"schema"|"cli-help"|"docker-compose")
        tasks=("$1")
        ;;
    *)
        echo "Usage: $0 [--update] {all|genesis|schema|cli-help|docker-compose}"
        exit 2
        ;;
esac

for task in "${tasks[@]}"; do
    case "$task" in
        "genesis")
            do_check "$cmd_genesis" "defaults/genesis.json"
            ;;
        "schema")
            do_check "$cmd_schema" "docs/source/references/schema.json"
            ;;
        "cli-help")
            do_check "$cmd_iroha_help" "crates/iroha_cli/CommandLineHelp.md"
            do_check "$cmd_kagami_help" "crates/iroha_kagami/CommandLineHelp.md"
            ;;
        "docker-compose")
            do_check_swarm 1 hyperledger/iroha:local "--build ." "defaults/docker-compose.single.yml"
            do_check_swarm 4 hyperledger/iroha:local "--build ." "defaults/docker-compose.local.yml"
            do_check_swarm 4 hyperledger/iroha:dev "" "defaults/docker-compose.yml"
            ;;
    esac
done

exit "$exit_code"
