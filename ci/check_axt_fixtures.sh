#!/usr/bin/env bash
set -euo pipefail

cargo run -p iroha_data_model --features dev-tools,test-fixtures --bin axt_fixtures -- --check
