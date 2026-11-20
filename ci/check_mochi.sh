#!/usr/bin/env bash
set -euo pipefail

# Mochi delegates generation/lifecycle and deployment to these canonical owners.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ $# == 1 && $1 == --ui-fast ]]; then
    exec scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked \
        -p mochi-ui -p izanami --features mochi-ui/gui --bin mochi --bin izanami -- \
        gui::tests::activity_ every_empty_workspace_view_renders gui::widgets::tests:: \
        deployment_review_keeps_decision_buttons_reachable_in_a_small_viewport \
        publication_form_requires_explicit_click_and_respects_disabled_actions tui::tests::
elif [[ $# != 0 ]]; then
    echo 'Usage: ci/check_mochi.sh [--ui-fast]' >&2
    exit 2
fi

cargo test --locked -p iroha_fs --lib
cargo test --locked -p iroha_managed_ipc --lib
cargo test --locked -p iroha_deploy --lib managed
cargo test --locked -p mochi-core
cargo test --locked -p mochi-integration
cargo check --locked -p mochi-ui --features gui --bin mochi
cargo test --locked -p mochi-ui --features gui --bin mochi
cargo run --locked -p mochi-ui --features gui --bin mochi -- --help
