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
    local output_dir
    local output_name
    local staged_output

    output_dir="$(dirname "$target")"
    output_name="$(basename "$target")"
    staged_output="$(mktemp "${output_dir}/.${output_name}.XXXXXX")"

    if ! eval "$cmd" > "$staged_output"; then
        echo "[FAIL] generator command failed"
        echo "  $cmd"
        rm -f -- "$staged_output"
        exit_code=1
        return
    fi
    if [[ ! -s "$staged_output" ]]; then
        echo "[FAIL] generator produced empty output"
        echo "  $cmd"
        rm -f -- "$staged_output"
        exit_code=1
        return
    fi

    if [[ "$update" -eq 1 ]]; then
        chmod 0644 "$staged_output"
        mv -f -- "$staged_output" "$target"
        echo "[UPDATED] $target"
    else
        if ! diff "$staged_output" "$target" > /dev/null; then
            echo "[DIFF] $target is out of date"
            echo "Run with \"--update\" to regenerate automatically, or run manually:"
            echo "  $cmd > $target"
            exit_code=1
        else
            echo "[OK] $target is up to date"
        fi
        rm -f -- "$staged_output"
    fi
}

do_render_check() {
    local cmd="$1"
    if ! eval "$cmd" > /dev/null; then
        echo "[FAIL] unable to render live CLI help"
        echo "  $cmd"
        exit_code=1
    else
        echo "[OK] live CLI help renders successfully"
    fi
}

do_check_swarm() {
    local peers="$1"
    local image="$2"
    local extra="$3"
    local target="$4"
    local cmd_base="${bin_kagami[@]} docker --peers $peers --seed Iroha --healthcheck --config-dir ./defaults --image $image --print"
    do_check "$cmd_base --out-file $target $extra" "$target"
}

cmd_genesis="${bin_kagami[@]} genesis generate --ivm-dir . --genesis-public-key ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4"
cmd_schema="${bin_kagami[@]} advanced schema"
cmd_iroha_help="${bin_iroha[@]} tools markdown-help"
cmd_kagami_help="${bin_kagami[@]} advanced markdown-help"

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
            do_check "$cmd_schema" "specs/references/schema.json"
            ;;
        "cli-help")
            do_render_check "$cmd_iroha_help"
            do_check "$cmd_kagami_help" "crates/iroha_kagami/CommandLineHelp.md"
            ;;
        "docker-compose")
            do_check_swarm 4 hyperledger/iroha:local "--build ." "defaults/docker-compose.single.yml"
            do_check_swarm 4 hyperledger/iroha:local "--build ." "defaults/docker-compose.local.yml"
            do_check_swarm 4 hyperledger/iroha:dev "" "defaults/docker-compose.yml"
            ;;
    esac
done

exit "$exit_code"
