#!/bin/sh
set -e

# Makes possible to set e.g. `BIN_KAGAMI=target/release/kagami` without running cargo
bin_kagami="${BIN_KAGAMI:-cargo run --release --bin kagami --}"
bin_iroha="${BIN_IROHA:-cargo run --release --bin iroha --}"

case $1 in
    "genesis")
        eval "$bin_kagami genesis generate --executor executor.wasm --wasm-dir libs --genesis-public-key ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4" \
            | diff - defaults/genesis.json || {
            echo "Please re-generate the default genesis with `$bin_kagami genesis --executor executor.wasm --wasm-dir libs --genesis-public-key ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4 > ./defaults/genesis.json`"
            echo 'The assumption here is that the authority of the default genesis transaction is `iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID`'
            exit 1
        };;
    "schema")
        eval "$bin_kagami schema" | diff - docs/source/references/schema.json || {
            echo "Please re-generate schema with `$bin_kagami schema > docs/source/references/schema.json`"
            exit 1
        };;
    "cli-help")
        eval "$bin_iroha markdown-help" | diff - crates/iroha_cli/CommandLineHelp.md || {
            echo "Please re-generate command-line help with `$bin_iroha markdown-help > crates/iroha_cli/CommandLineHelp.md`"
            exit 1
        }
        eval "$bin_kagami markdown-help" | diff - crates/iroha_kagami/CommandLineHelp.md || {
            echo "Please re-generate command-line help with `$bin_kagami -- markdown-help > crates/iroha_kagami/CommandLineHelp.md`"
            exit 1
        }
        ;;
    "docker-compose")
        do_check() {
            cmd_base=$1
            target=$2
            full_cmd="$cmd_base --out-file $target --print"
            diff <(eval "$full_cmd") "$target" || {
                echo "Please re-generate \`$target\` with \`$cmd_base --out-file $target\`"
                exit 1
            }
        }

        command_base_for_single() {
            echo "$bin_kagami swarm -p 1 -s Iroha -H -c ./defaults -i hyperledger/iroha:local -b ."
        }

        command_base_for_multiple_local() {
            echo "$bin_kagami swarm -p 4 -s Iroha -H -c ./defaults -i hyperledger/iroha:local -b ."
        }

        command_base_for_default() {
            echo "$bin_kagami swarm -p 4 -s Iroha -H -c ./defaults -i hyperledger/iroha:dev"
        }


        do_check "$(command_base_for_single)" "defaults/docker-compose.single.yml"
        do_check "$(command_base_for_multiple_local)" "defaults/docker-compose.local.yml"
        do_check "$(command_base_for_default)" "defaults/docker-compose.yml"
esac
