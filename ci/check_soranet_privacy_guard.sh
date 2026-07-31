#!/usr/bin/env bash

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd -- "${ROOT}"

TARGET="crates/iroha_torii/src/lib.rs"
PIN_TARGET="crates/iroha_torii/src/sorafs/api.rs"
DOC_PATH="specs/references/configuration.md"
RUNBOOK_PATH="specs/sorafs_authz_runbook.md"
OPS_PLAYBOOK_PATH="specs/sorafs_ops_playbook.md"

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

repair_routes=(
	"handle_post_sorafs_repair_report:Report"
	"handle_post_sorafs_repair_slash:Escalate"
	"handle_post_sorafs_repair_claim:Claim"
	"handle_post_sorafs_repair_heartbeat:Renew"
	"handle_post_sorafs_repair_complete:Complete"
	"handle_post_sorafs_repair_fail:Fail"
	"handle_post_sorafs_repair_appeal:Appeal"
)

for route_spec in "${repair_routes[@]}"; do
	fn="${route_spec%%:*}"
	route="${route_spec#*:}"
	pattern="(?s)fn[[:space:]]+${fn}\\([^}]*JsonOrNoritoVersioned\\(transaction\\):[[:space:]]+JsonOrNoritoVersioned<SignedTransaction>[^}]*submit_repair_signed_transaction\\(.*?RepairCommandRouteV1::${route}"
	if ! rg --pcre2 --multiline -n "${pattern}" "${PIN_TARGET}" >/dev/null; then
		echo "error: ${fn} must forward a caller-signed transaction through the ${route} native repair route." >&2
		exit 1
	fi
done

repair_ingress_pattern="(?s)async[[:space:]]+fn[[:space:]]+submit_repair_signed_transaction\\(.*?validate_repair_signed_transaction\\(.*?submit_signed_transaction_for_ingress_strict_durable\\("
if ! rg --pcre2 --multiline -n "${repair_ingress_pattern}" "${PIN_TARGET}" >/dev/null; then
	echo "error: SoraFS repair commands must validate their native instruction and use strict durable transaction ingress." >&2
	exit 1
fi

for matcher_token in \
	"Executable::Instructions(instructions)" \
	"instructions.len() != 1" \
	"downcast_ref::<SubmitSorafsRepairTask>()" \
	"downcast_ref::<SubmitSorafsRepairAppeal>()" \
	"SorafsRepairTaskActionV1::Escalate(_)" \
	"SorafsRepairTaskActionV1::Claim(_)" \
	"SorafsRepairTaskActionV1::Renew(_)" \
	"SorafsRepairTaskActionV1::Complete(_)" \
	"SorafsRepairTaskActionV1::Fail(_)"
do
	if ! rg -Fq "${matcher_token}" "${PIN_TARGET}"; then
		echo "error: native SoraFS repair route matcher is missing ${matcher_token}." >&2
		exit 1
	fi
done

repair_matcher_routes=(
	"Report:SubmitSorafsRepairTask"
	"Appeal:SubmitSorafsRepairAppeal"
)
for matcher_spec in "${repair_matcher_routes[@]}"; do
	route="${matcher_spec%%:*}"
	instruction="${matcher_spec#*:}"
	pattern="(?s)RepairCommandRouteV1::${route}[[:space:]]*=>.*?downcast_ref::<${instruction}>\\(\\)"
	if ! rg --pcre2 --multiline -n "${pattern}" "${PIN_TARGET}" >/dev/null; then
		echo "error: native SoraFS repair ${route} route must match only ${instruction}." >&2
		exit 1
	fi
done

repair_action_routes=(
	"Escalate:Escalate"
	"Claim:Claim"
	"Renew:Renew"
	"Complete:Complete"
	"Fail:Fail"
)
for matcher_spec in "${repair_action_routes[@]}"; do
	route="${matcher_spec%%:*}"
	action="${matcher_spec#*:}"
	pattern="(?s)RepairCommandRouteV1::${route},[[:space:]]*SorafsRepairTaskActionV1::${action}\\(_\\)"
	if ! rg --pcre2 --multiline -n "${pattern}" "${PIN_TARGET}" >/dev/null; then
		echo "error: native SoraFS repair ${route} route must match only the ${action} action." >&2
		exit 1
	fi
done

retired_repair_symbols=(
	"SignedAuditorRequestV1"
	"RepairWorkerSignaturePayloadV1"
	"enforce_sorafs_repair_worker_auth"
	"handle_get_sorafs_repair_status_by_manifest"
	"handle_get_sorafs_repair_events_stream"
	"handle_get_sorafs_repair_events_ws"
)
for symbol in "${retired_repair_symbols[@]}"; do
	if rg -Fq "${symbol}" "${TARGET}" "${PIN_TARGET}"; then
		echo "error: retired pre-release SoraFS repair symbol ${symbol} must not return." >&2
		exit 1
	fi
done

if ! rg -q "soranet_privacy_ingest" "${DOC_PATH}"; then
	echo "error: ${DOC_PATH} is missing the torii.soranet_privacy_ingest docs; update the config reference when changing SoraNet privacy authz." >&2
	exit 1
fi

if rg -Fq "handle_post_sorafs_storage_pin" "${PIN_TARGET}"; then
	echo "error: retired public SoraFS storage-ingest handler must not return." >&2
	exit 1
fi

for doc_key in "no public storage-ingest route" "governance.sorafs_telemetry"; do
	if ! rg -q "${doc_key}" "${DOC_PATH}"; then
		echo "error: ${DOC_PATH} is missing ${doc_key} authz documentation; keep docs aligned with Torii guards." >&2
		exit 1
	fi
done

for runbook_key in "X-SoraNet-Privacy-Token" "per_provider_submitters" "Storage ingest is not an ingress route" "CanOperateSorafsRepair"; do
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
