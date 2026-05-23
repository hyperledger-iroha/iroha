#!/bin/bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh fork-fast
bash scripts/formal/sumeragi_apalache.sh fork-npos
bash scripts/formal/sumeragi_apalache.sh quorum-fast
bash scripts/formal/sumeragi_apalache.sh rbc-fast
bash scripts/formal/sumeragi_apalache.sh qc-signers-fast
bash scripts/formal/sumeragi_apalache.sh commit-roots-fast
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-fast
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-fast
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-fast
bash scripts/formal/sumeragi_apalache.sh precommit-fast
bash scripts/formal/sumeragi_apalache.sh proposal-fast
bash scripts/formal/sumeragi_apalache.sh engine-tick-fast
bash scripts/formal/sumeragi_apalache.sh engine-new-view-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-fast
bash scripts/formal/sumeragi_apalache.sh engine-prepare-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-fast
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-fast
bash scripts/formal/sumeragi_apalache.sh engine-payload-fast
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-fast
bash scripts/formal/sumeragi_apalache.sh reconfig-fast
bash scripts/formal/sumeragi_apalache.sh recovery-fast
bash scripts/formal/sumeragi_apalache.sh view-change-fast
bash scripts/formal/sumeragi_apalache.sh validation-fast
bash scripts/formal/sumeragi_apalache.sh admission-fast
bash scripts/formal/sumeragi_apalache.sh highest-fast
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
bash scripts/formal/sumeragi_tlc.sh frontier-small
bash ci/check_sumeragi_formal_expected_failures.sh

echo "[formal] sumeragi formal checks passed"
