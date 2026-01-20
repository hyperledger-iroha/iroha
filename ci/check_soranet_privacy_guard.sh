#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

TARGET="crates/iroha_torii/src/lib.rs"
PIN_TARGET="crates/iroha_torii/src/sorafs/api.rs"
DOC_PATH="docs/source/references/configuration.md"
RUNBOOK_PATH="docs/source/sorafs_authz_runbook.md"
OPS_PLAYBOOK_PATH="docs/source/sorafs_ops_playbook.md"

endpoints=(
	"handler_post_soranet_privacy_event"
	"handler_post_soranet_privacy_share"
)

for fn in "${endpoints[@]}"; do
	pattern="(?s)fn[[:space:]]+${fn}.*enforce_soranet_privacy_ingest"
	if ! rg --pcre2 --multiline -n "${pattern}" "${TARGET}" >/dev/null; then
		echo "error: ${fn} must call enforce_soranet_privacy_ingest to keep SoraNet privacy ingest authenticated." >&2
		exit 1
	fi
done

repair_handlers=(
	"handler_post_sorafs_repair_claim"
	"handler_post_sorafs_repair_heartbeat"
	"handler_post_sorafs_repair_complete"
	"handler_post_sorafs_repair_fail"
)

for fn in "${repair_handlers[@]}"; do
	pattern="(?s)fn[[:space:]]+${fn}.*enforce_sorafs_repair_worker_auth"
	if ! rg --pcre2 --multiline -n "${pattern}" "${TARGET}" >/dev/null; then
		echo "error: ${fn} must call enforce_sorafs_repair_worker_auth to keep SoraFS repair worker actions authenticated." >&2
		exit 1
	fi
done

if ! rg -q "soranet_privacy_ingest" "${DOC_PATH}"; then
	echo "error: ${DOC_PATH} is missing the torii.soranet_privacy_ingest docs; update the config reference when changing SoraNet privacy authz." >&2
	exit 1
fi

pin_handler="handle_post_sorafs_storage_pin"
pin_pattern="(?s)fn[[:space:]]+${pin_handler}.*sorafs_pin_policy\\.?enforce"
if ! rg --pcre2 --multiline -n "${pin_pattern}" "${PIN_TARGET}" >/dev/null; then
	echo "error: ${pin_handler} must enforce sorafs_pin_policy before accepting storage pins." >&2
	exit 1
fi

for doc_key in "sorafs.storage.pin" "governance.sorafs_telemetry"; do
	if ! rg -q "${doc_key}" "${DOC_PATH}"; then
		echo "error: ${DOC_PATH} is missing ${doc_key} authz documentation; keep docs aligned with Torii guards." >&2
		exit 1
	fi
done

for runbook_key in "X-SoraNet-Privacy-Token" "per_provider_submitters" "sorafs.storage.pin" "CanOperateSorafsRepair"; do
	if ! rg -q "${runbook_key}" "${RUNBOOK_PATH}"; then
		echo "error: ${RUNBOOK_PATH} is missing ${runbook_key} guidance; update the authz runbook alongside code changes." >&2
		exit 1
	fi
done

for playbook_key in "Auth & Governance Checklist" "torii.soranet_privacy_ingest" "gov.sorafs_telemetry" "CanOperateSorafsRepair"; do
	if ! rg -q "${playbook_key}" "${OPS_PLAYBOOK_PATH}"; then
		echo "error: ${OPS_PLAYBOOK_PATH} is missing ${playbook_key} coverage; align the ops playbook with authz changes." >&2
		exit 1
	fi
done

echo "soranet/sorafs auth ingest guard: ok"
