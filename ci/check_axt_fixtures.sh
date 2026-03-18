#!/usr/bin/env bash
set -euo pipefail

cargo run -p iroha_data_model --features test-fixtures --bin axt_fixtures -- --check
