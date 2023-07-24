#!/bin/sh
set -e

KAGAMI_BIN_PATH=${2:-"./target/release/kagami"}

case $1 in
    "docs")
        cargo run --release --bin kagami -- docs | diff - docs/source/references/config.md || {
            echo 'Please re-generate docs using `cargo run --release --bin kagami -- docs > docs/source/references/config.md`'
            exit 1
        };;
    "genesis")
        cargo run --release --bin kagami -- genesis --compiled-validator-path ./validator.wasm | diff - configs/peer/genesis.json || {
            echo 'Please re-generate the genesis with `cargo run --release --bin kagami -- genesis > configs/peer/genesis.json`'
            exit 1
        };;
    "client")
        cargo run --release --bin kagami -- config client | diff - configs/client/config.json || {
            echo 'Please re-generate client config with `cargo run --release --bin kagami -- config client > configs/client/config.json`'
            exit 1
        };;
    "peer")
        cargo run --release --bin kagami -- config peer | diff - configs/peer/config.json || {
            echo 'Please re-generate peer config with `cargo run --release --bin kagami -- config peer > configs/peer/config.json`'
            exit 1
        };;
    "schema")
        cargo run --release --bin kagami -- schema | diff - docs/source/references/schema.json || {
            echo 'Please re-generate schema with `cargo run --release --bin kagami -- schema > docs/source/references/schema.json`'
            exit 1
        };;
    "validator")
        if [ ! -e "$KAGAMI_BIN_PATH" ]; then
            echo 'Please run this check from Iroha root dir after compiling with `--release` flag or provide a valid path to `kagami` binary as a second argument'
            exit 1
        fi
        $KAGAMI_BIN_PATH validator > /dev/null || {
            echo 'Failed to run `kagami validator` as a standalone binary without invoking `cargo`'
            exit 1
        };;
    "docker-compose")
        do_check() {
            cmd_base=$1
            target=$2
            # FIXME: not nice; add an option to `kagami swarm` to print content into stdout?
            #        it is not a default behaviour because Kagami resolves `build` path relative
            #        to the output file location
            temp_file="docker-compose.TMP.yml"

            eval "$cmd_base $temp_file"
            diff "$temp_file" "$target" || {
                echo "Please re-generate \`$target\` with \`$cmd_base $target\`"
                exit 1
            }
        }

        command_base_for_single() {
            echo "cargo run --release --bin kagami -- swarm -p 1 -s Iroha --force file --config-dir ./configs/peer --build ."
        }

        command_base_for_multiple_local() {
            echo "cargo run --release --bin kagami -- swarm -p 4 -s Iroha --force file --config-dir ./configs/peer --build ."
        }

        command_base_for_default() {
            echo "cargo run --release --bin kagami -- swarm -p 4 -s Iroha --force file --config-dir ./configs/peer --image hyperledger/iroha2:dev"
        }


        do_check "$(command_base_for_single)" "docker-compose.dev.single.yml"
        do_check "$(command_base_for_multiple_local)" "docker-compose.dev.local.yml"
        do_check "$(command_base_for_default)" "docker-compose.dev.yml"
esac
