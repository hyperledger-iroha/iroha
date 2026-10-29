// SPDX-License-Identifier: Apache-2.0

import { Buffer } from "buffer";

import { AccountAddress } from "./address.js";
import { parseCanonicalContractAddress } from "./contractAddress.js";
import { NetworkId } from "./networkId.js";
import {
  ensureCanonicalAccountId,
  normalizeAssetDefinitionId,
} from "./normalizers.js";
import { NumericV1, NumericV1Error } from "./numericV1.js";
import { strictDecodeBase64 } from "./toriiClientEncoding.js";

const MAX_UINT64_BIGINT = (1n << 64n) - 1n;

/**
 * Validate and rebuild one exact canonical GovernanceProposalKind V1 JSON value.
 *
 * This is the wire boundary. Tuple-struct fields remain one-field JSON arrays;
 * callers that want semantic projections must flatten them only after this
 * function has admitted the canonical wire representation.
 */
export function normalizeGovernanceProposalWireV1(value, context = "proposal") {
  const record = exactRecord(value, ["kind", "payload"], context);
  const kind = nonEmptyString(record.kind, `${context}.kind`);
  const payloadContext = `${context}.payload`;
  switch (kind) {
    case "DeployContract":
      return { kind, payload: normalizeDeployContract(record.payload, payloadContext) };
    case "RuntimeUpgrade":
      return { kind, payload: normalizeRuntimeUpgrade(record.payload, payloadContext) };
    case "SccpRouteGovernance":
      return { kind, payload: normalizeSccpRoute(record.payload, payloadContext) };
    case "ValidationFeePolicy":
      return { kind, payload: normalizeValidationFeePolicy(record.payload, payloadContext) };
    case "ValidationFeePayoutLifecycle":
      return {
        kind,
        payload: normalizeValidationFeePayoutLifecycle(record.payload, payloadContext),
      };
    case "MusubiRegistryGovernance":
      return { kind, payload: normalizeMusubiAction(record.payload, payloadContext) };
    case "SorafsProviderGovernance":
      return { kind, payload: normalizeSorafsProvider(record.payload, payloadContext) };
    case "ContractLifecycleGovernance":
      return { kind, payload: normalizeContractLifecycle(record.payload, payloadContext) };
    case "ContractEmergencyHold":
      return { kind, payload: normalizeContractEmergencyHold(record.payload, payloadContext) };
    case "GlobalDataTriggerPermissionGovernance":
      return {
        kind,
        payload: normalizeGlobalDataTriggerPermission(record.payload, payloadContext),
      };
    default:
      throw new TypeError(`${context}.kind contains an unsupported V1 proposal variant: ${kind}`);
  }
}

function normalizeDeployContract(value, context) {
  const record = exactRecord(value, [
    "proposal_operator",
    "contract_address",
    "code_hash",
    "abi_hash",
    "abi_version",
    "manifest_provenance",
  ], context);
  if (record.abi_version !== 1) {
    throw new TypeError(`${context}.abi_version must be the number 1`);
  }
  const contractAddress = nonEmptyString(record.contract_address, `${context}.contract_address`);
  parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    contract_address: contractAddress,
    code_hash: lowerHex32(record.code_hash, `${context}.code_hash`),
    abi_hash: lowerHex32(record.abi_hash, `${context}.abi_hash`),
    abi_version: 1,
    manifest_provenance: record.manifest_provenance === null
      ? null
      : normalizeManifestProvenance(
        record.manifest_provenance,
        `${context}.manifest_provenance`,
      ),
  };
}

function normalizeManifestProvenance(value, context) {
  const record = exactRecord(value, ["signer", "signature"], context);
  return {
    signer: nonEmptyString(record.signer, `${context}.signer`),
    signature: nonEmptyString(record.signature, `${context}.signature`),
  };
}

/**
 * Admit the exact `RuntimeUpgradeProposal`: the change set SORA Parliament schedules, or the
 * scheduled change set it withdraws, with the notice and proposal time frozen at admission.
 * Every number-encoded u64, including the change set's effective height, is a JSON safe integer.
 */
function normalizeRuntimeUpgrade(value, context) {
  const record = exactRecord(
    value,
    ["proposal_operator", "action", "notice", "proposed_ms"],
    context,
  );
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    action: normalizeRuntimeUpgradeAction(record.action, `${context}.action`),
    notice: normalizeRuntimeNotice(record.notice, `${context}.notice`),
    proposed_ms: jsonUint(record.proposed_ms, `${context}.proposed_ms`),
  };
}

function normalizeRuntimeUpgradeAction(value, context) {
  const record = exactRecord(value, ["kind", "content"], context);
  switch (record.kind) {
    case "Schedule":
      return {
        kind: "Schedule",
        content: normalizeRuntimeChangeSetV1(record.content, `${context}.content`),
      };
    case "Cancel":
      return { kind: "Cancel", content: lowerHex32(record.content, `${context}.content`) };
    default:
      throw new TypeError(`${context}.kind must be Schedule or Cancel`);
  }
}

function normalizeRuntimeNotice(value, context) {
  const record = exactRecord(value, ["blocks", "ms", "approval_cushion_ms"], context);
  return {
    blocks: jsonUint(record.blocks, `${context}.blocks`),
    ms: jsonUint(record.ms, `${context}.ms`),
    approval_cushion_ms: jsonUint(
      record.approval_cushion_ms,
      `${context}.approval_cushion_ms`,
    ),
  };
}

/**
 * Validate and rebuild one exact `ChangeSetV1` JSON object
 * (`specs/live_runtime_upgrades.md` §6).
 *
 * This release implements change sets of the universal root only. Published definitions and
 * transitions keep their canonical `{kind, content}` envelopes with opaque JSON content;
 * integers in that content must be safe JSON numbers or bigint. The node checks the
 * transitions against the selection the change set meets when the proposal is admitted.
 */
export function normalizeRuntimeChangeSetV1(value, context = "changeSet") {
  const record = exactRecord(value, [
    "network",
    "root",
    "predecessor",
    "successor",
    "effective_height",
    "publish",
    "transitions",
    "qualification",
  ], context);
  const network = nonEmptyString(record.network, `${context}.network`);
  try {
    NetworkId.parse(network);
  } catch (error) {
    throw new TypeError(
      `${context}.network must be an exact canonical NetworkId literal`,
      { cause: error },
    );
  }
  const root = exactRecord(record.root, ["kind", "content"], `${context}.root`);
  if (root.kind !== "Universal" || root.content !== null) {
    throw new TypeError(`${context}.root must be the universal root with null content`);
  }
  return {
    network,
    root: { kind: "Universal", content: null },
    predecessor: lowerHex32(record.predecessor, `${context}.predecessor`),
    successor: lowerHex32(record.successor, `${context}.successor`),
    effective_height: jsonUint(record.effective_height, `${context}.effective_height`, {
      allowZero: false,
    }),
    publish: taggedJsonArray(record.publish, `${context}.publish`),
    transitions: taggedJsonArray(record.transitions, `${context}.transitions`),
    qualification: lowerHex32(record.qualification, `${context}.qualification`),
  };
}

function taggedJsonArray(value, context) {
  return array(value, context).map((item, index) => {
    const itemContext = `${context}[${index}]`;
    const entry = exactRecord(item, ["kind", "content"], itemContext);
    return {
      kind: nonEmptyString(entry.kind, `${itemContext}.kind`),
      content: jsonValue(entry.content, `${itemContext}.content`, 0),
    };
  });
}

const OPAQUE_JSON_MAX_DEPTH = 128;

function jsonValue(value, context, depth) {
  if (depth > OPAQUE_JSON_MAX_DEPTH) {
    throw new RangeError(`${context} exceeds the ${OPAQUE_JSON_MAX_DEPTH}-level JSON nesting limit`);
  }
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || Object.is(value, -0)) {
      throw new TypeError(`${context} must contain only safe integer numbers or bigint`);
    }
    return value;
  }
  if (Array.isArray(value)) {
    return Array.from(value, (item, index) => jsonValue(item, `${context}[${index}]`, depth + 1));
  }
  if (!plainObject(value)) {
    throw new TypeError(`${context} must contain only JSON values`);
  }
  const result = {};
  for (const [key, item] of Object.entries(value)) {
    Object.defineProperty(result, key, {
      value: jsonValue(item, `${context}.${key}`, depth + 1),
      enumerable: true,
      writable: true,
      configurable: true,
    });
  }
  return result;
}

const SCCP_GOVERNANCE_MAX_ACTIONS_V1 = 16;
// Declaration order in SccpGovernanceSubjectV1 and SccpNetworkV1 fixes S(P).
const SCCP_SUBJECTS = Object.freeze([
  "route", "route_control", "light_client", "parameters", "forgery_hold",
  "fast_hold", "fast_pause_control",
]);
const SCCP_EXTERNAL_NETWORKS = Object.freeze([
  "ethereum_mainnet", "bsc_mainnet", "ton_mainnet",
]);
const SCCP_ACTION_SUBJECTS = Object.freeze({
  register_route: "route",
  activate_revision: "route",
  switch_revision: "route",
  deactivate_outbound: "route",
  retire_revision: "route",
  remove_staged: "route",
  release_stranded: "route",
  set_taira_paused: "route",
  set_destination_paused: "route_control",
  initialize_light_client: "light_client",
  install_trusted_checkpoint: "light_client",
  freeze_light_client: "light_client",
  set_parameters: "parameters",
  activate_light_client_profile: "light_client",
  attest_deployment_progress: "forgery_hold",
  quarantine_revision: "forgery_hold",
  open_reconciliation: "forgery_hold",
  set_fast_pause_suspended: "fast_pause_control",
});
const SCCP_ACTION_FIELDS = Object.freeze({
  register_route: ["revision", "deployment", "initial_committee", "max_wrapped_supply"],
  release_stranded: ["amount", "recipient", "memo"],
  initialize_light_client: ["expected", "params", "bootstrap"],
  activate_revision: ["revision"],
  deactivate_outbound: ["revision"],
  retire_revision: ["revision"],
  remove_staged: ["revision"],
  quarantine_revision: ["revision"],
  switch_revision: ["from", "to"],
  set_taira_paused: ["paused"],
  set_destination_paused: ["revision", "paused"],
  freeze_light_client: [],
  install_trusted_checkpoint: ["checkpoint"],
  activate_light_client_profile: ["version", "profile_hash"],
  attest_deployment_progress: ["revision", "at"],
  open_reconciliation: ["revision", "window_ms"],
  set_fast_pause_suspended: ["duration_ms"],
  set_parameters: ["next"],
});
// Three immutable maximum pause periods (sccp/params.rs), independent of live policy.
const SCCP_FAST_PAUSE_SUSPENSION_MAX_MS_V1 = 3 * 2_592_000_000;
// Own-field bounds from SccpParametersV1::check_own_bounds; joint rules are below.
const SCCP_PARAMETER_INTEGER_BOUNDS = Object.freeze({
  committee_validity_ms: [0, 2_592_000_000],
  committee_heartbeat_ms: [3_600_000, Number.MAX_SAFE_INTEGER],
  forgery_evidence_grace_ms: [86_400_000, 2_592_000_000],
  destination_progress_cap_ms: [0, 2_592_000_000],
  outbound_ttl_ms: [86_400_000, 7_776_000_000],
  max_exempt_transactions_per_block: [8, 1024],
  fast_pause_panel_seats: [5, 9],
  fast_pause_panel_term_ms: [3_600_000, Number.MAX_SAFE_INTEGER],
  fast_pause_seat_accept_window_ms: [600_000, Number.MAX_SAFE_INTEGER],
  fast_pause_endorse_window_ms: [300_000, 21_600_000],
  fast_pause_renew_lead_ms: [0, Number.MAX_SAFE_INTEGER],
  fast_pause_hold_ms: [0, 2_505_600_000],
});

/** Admit the SCCP proposal envelope, static action rules and exact subject-revision set. */
export function normalizeSccpRoute(value, context) {
  const record = exactRecord(value, ["proposal"], context);
  const proposalContext = `${context}.proposal`;
  const proposal = exactRecord(
    record.proposal,
    ["network_id", "base_revisions", "actions"],
    proposalContext,
  );
  const networkId = nonEmptyString(proposal.network_id, `${proposalContext}.network_id`);
  NetworkId.parse(networkId);
  const basesContext = `${proposalContext}.base_revisions`;
  const bases = array(proposal.base_revisions, basesContext);
  const actionsContext = `${proposalContext}.actions`;
  const rawActions = array(proposal.actions, actionsContext);
  for (const [entries, label] of [[bases, basesContext], [rawActions, actionsContext]]) {
    if (entries.length === 0 || entries.length > SCCP_GOVERNANCE_MAX_ACTIONS_V1) {
      throw new TypeError(
        `${label} must contain 1..${SCCP_GOVERNANCE_MAX_ACTIONS_V1} entries`,
      );
    }
  }
  const baseRevisions = Array.from(bases, (item, index) => {
    const label = `${basesContext}[${index}]`;
    const entry = exactRecord(item, ["subject", "revision"], label);
    return {
      subject: normalizeSccpSubject(entry.subject, `${label}.subject`),
      revision: jsonUint(entry.revision, `${label}.revision`),
    };
  });
  const actionSubjects = new Set();
  const destinations = new Set();
  const actions = Array.from(rawActions, (item, index) => {
    const label = `${actionsContext}[${index}]`;
    const entry = exactRecord(item, ["action", "payload"], label);
    const action = nonEmptyString(entry.action, `${label}.action`);
    if (!Object.hasOwn(SCCP_ACTION_SUBJECTS, action)) {
      throw new TypeError(`${label}.action contains an unsupported SCCP action: ${action}`);
    }
    if (!plainObject(entry.payload)) {
      throw new TypeError(`${label}.payload must be an object`);
    }
    const tag = SCCP_ACTION_SUBJECTS[action];
    const key = tag === "parameters"
      ? null
      : normalizeSccpNetwork(entry.payload.network, `${label}.payload.network`);
    actionSubjects.add(sccpSubjectOrder({ subject: tag, key }));
    const payload = normalizeSccpActionPayload(action, entry.payload, key, `${label}.payload`);
    if (action === "register_route") {
      const deployment = payload.deployment.deployment;
      const word = payload.deployment.family === "evm"
        ? [...Array(12).fill(0), ...deployment.address] : deployment.master_account;
      const destination = word.join(",");
      if (destinations.has(destination)) {
        throw new TypeError(`${label}.payload.deployment repeats a registered destination word`);
      }
      destinations.add(destination);
    }
    return {
      action,
      payload,
    };
  });
  const expected = [...actionSubjects].sort((left, right) => left - right);
  if (baseRevisions.length !== expected.length
      || baseRevisions.some((entry, index) => sccpSubjectOrder(entry.subject) !== expected[index])) {
    throw new TypeError(
      `${basesContext} must list exactly the action subjects in canonical order, without duplicates`,
    );
  }
  return {
    proposal: {
      network_id: networkId,
      base_revisions: baseRevisions,
      actions,
    },
  };
}

function normalizeSccpActionPayload(action, value, network, context) {
  const fields = SCCP_ACTION_FIELDS[action];
  const record = exactRecord(value, network === null ? fields : ["network", ...fields], context);
  const result = network === null ? { ...record } : { ...record, network };
  if (Object.hasOwn(record, "revision")) {
    result.revision = sccpUint32(record.revision, `${context}.revision`);
  }
  switch (action) {
    case "release_stranded": {
      result.amount = sccpAmount(record.amount, 1n, (1n << 128n) - 1n, `${context}.amount`);
      if (typeof record.memo !== "string" || record.memo.length > 256) {
        throw new TypeError(`${context}.memo must be a string of at most 256 UTF-8 bytes`);
      }
      for (const character of record.memo) {
        const codePoint = character.codePointAt(0);
        if (codePoint >= 0xd800 && codePoint <= 0xdfff) {
          throw new TypeError(`${context}.memo must not contain unpaired UTF-16 surrogates`);
        }
      }
      if (Buffer.byteLength(record.memo, "utf8") > 256) {
        throw new TypeError(`${context}.memo exceeds 256 UTF-8 bytes`);
      }
      result.recipient = canonicalAccountId(record.recipient, `${context}.recipient`);
      break;
    }
    case "register_route":
      result.deployment = normalizeSccpDeployment(record.deployment, network, `${context}.deployment`);
      result.initial_committee = normalizeSccpCommitteePin(record.initial_committee, `${context}.initial_committee`);
      result.max_wrapped_supply = sccpAmount(record.max_wrapped_supply, 1n,
        (1n << (network.network === "ton_mainnet" ? 96n : 128n)) - 1n, `${context}.max_wrapped_supply`);
      break;
    case "initialize_light_client": {
      const expected = exactRecord(record.expected, ["expected", "detail"], `${context}.expected`);
      if (!["absent", "unusable"].includes(expected.expected) || expected.detail !== null) {
        throw new TypeError(`${context}.expected must name absent or unusable with null detail`);
      }
      result.expected = { expected: expected.expected, detail: null };
      result.params = normalizeSccpLightClientParams(record.params, network, `${context}.params`);
      result.bootstrap = normalizeSccpBootstrap(record.bootstrap, network, `${context}.bootstrap`);
      break;
    }
    case "set_parameters":
      result.next = normalizeSccpParameters(record.next, `${context}.next`);
      break;
    case "switch_revision":
      result.from = sccpUint32(record.from, `${context}.from`);
      result.to = sccpUint32(record.to, `${context}.to`);
      if (result.from === result.to) {
        throw new TypeError(`${context}.from and .to must name distinct revisions`);
      }
      break;
    case "set_taira_paused":
    case "set_destination_paused":
      if (typeof record.paused !== "boolean") {
        throw new TypeError(`${context}.paused must be a boolean`);
      }
      break;
    case "activate_light_client_profile":
      result.version = sccpUint32(record.version, `${context}.version`);
      if (result.version < 2) {
        throw new TypeError(`${context}.version must be at least 2; version 1 is active from genesis`);
      }
      result.profile_hash = byteArray(record.profile_hash, 32, `${context}.profile_hash`, {
        nonZero: true,
      });
      break;
    case "install_trusted_checkpoint":
      if (network.network === "ton_mainnet") {
        throw new TypeError(`${context}.network cannot be TON; its light client reads no checkpoints`);
      }
      result.checkpoint = normalizeSccpCheckpoint(record.checkpoint, `${context}.checkpoint`);
      break;
    case "attest_deployment_progress":
      result.at = normalizeSccpProgress(record.at, `${context}.at`);
      break;
    case "open_reconciliation":
      result.window_ms = jsonUint(record.window_ms, `${context}.window_ms`, { allowZero: false });
      break;
    case "set_fast_pause_suspended":
      result.duration_ms = jsonUint(record.duration_ms, `${context}.duration_ms`);
      if (result.duration_ms > SCCP_FAST_PAUSE_SUSPENSION_MAX_MS_V1) {
        throw new TypeError(`${context}.duration_ms exceeds three maximum pause periods`);
      }
      break;
    default:
      break;
  }
  return result;
}

function normalizeSccpDeployment(value, network, context) {
  const record = exactRecord(value, ["family", "deployment"], context);
  const label = `${context}.deployment`;
  if (record.family === "evm" && network.network !== "ton_mainnet") {
    const deployed = exactRecord(record.deployment, ["address", "runtime_code_hash"], label);
    return { family: "evm", deployment: {
      address: byteArray(deployed.address, 20, `${label}.address`, { nonZero: true }),
      runtime_code_hash: byteArray(deployed.runtime_code_hash, 32, `${label}.runtime_code_hash`),
    } };
  }
  if (record.family === "ton" && network.network === "ton_mainnet") {
    const deployed = exactRecord(record.deployment, ["master_account", "minter_code", "wallet_code", "bucket_code"], label);
    return { family: "ton", deployment: {
      master_account: byteArray(deployed.master_account, 32, `${label}.master_account`, { nonZero: true }),
      minter_code: normalizeSccpTonCode(deployed.minter_code, `${label}.minter_code`),
      wallet_code: normalizeSccpTonCode(deployed.wallet_code, `${label}.wallet_code`),
      bucket_code: normalizeSccpTonCode(deployed.bucket_code, `${label}.bucket_code`),
    } };
  }
  throw new TypeError(`${context}.family must match the external network`);
}

function normalizeSccpTonCode(value, context) {
  const record = exactRecord(value, ["hash", "depth"], context);
  const depth = jsonUint(record.depth, `${context}.depth`);
  if (depth > 65535) throw new TypeError(`${context}.depth must fit in an unsigned 16-bit integer`);
  return { hash: byteArray(record.hash, 32, `${context}.hash`), depth };
}

function normalizeSccpCommitteePin(value, context) {
  const record = exactRecord(value, ["generation", "committee_root", "start_height", "until_ms"], context);
  return {
    generation: jsonUint(record.generation, `${context}.generation`, { allowZero: false }),
    committee_root: byteArray(record.committee_root, 32, `${context}.committee_root`, { nonZero: true }),
    start_height: jsonUint(record.start_height, `${context}.start_height`, { allowZero: false }),
    until_ms: jsonUint(record.until_ms, `${context}.until_ms`, { allowZero: false }),
  };
}

function normalizeSccpLightClientParams(value, network, context) {
  const jsonFields = ["ws_bound_ms", "set_retention_ms", "checkpoint_stride", "checkpoint_prune_after_ms"];
  const uint32Fields = ["max_updates_per_advance", "max_segment_headers", "max_ancestry_headers", "max_backfill_headers", "max_advance_bytes", "max_proof_bytes"];
  const record = exactRecord(value, ["network", ...jsonFields, ...uint32Fields], context);
  const result = { network: normalizeSccpNetwork(record.network, `${context}.network`) };
  if (result.network.network !== network.network) {
    throw new TypeError(`${context}.network must match the action network`);
  }
  for (const field of jsonFields) {
    const tonDerived = network.network === "ton_mainnet" && ["ws_bound_ms", "checkpoint_stride"].includes(field);
    result[field] = jsonUint(record[field], `${context}.${field}`, { allowZero: tonDerived });
    if (tonDerived && result[field] !== 0) {
      throw new TypeError(`${context}.${field} must be zero for TON`);
    }
  }
  for (const field of uint32Fields) result[field] = sccpUint32(record[field], `${context}.${field}`);
  return result;
}

function normalizeSccpBootstrap(value, network, context) {
  const record = exactRecord(value, ["network", "bytes"], context);
  const source = normalizeSccpNetwork(record.network, `${context}.network`);
  if (source.network !== network.network) {
    throw new TypeError(`${context}.network must match the action network`);
  }
  const maximum = 1_048_576;
  if (typeof record.bytes !== "string" || record.bytes.length === 0
      || record.bytes.length > 4 * Math.ceil(maximum / 3)) {
    throw new TypeError(`${context}.bytes must encode 1..${maximum} bytes`);
  }
  const bytes = canonicalBase64(record.bytes, `${context}.bytes`);
  const length = bytes.length / 4 * 3 - (bytes.endsWith("==") ? 2 : bytes.endsWith("=") ? 1 : 0);
  if (length > maximum) throw new TypeError(`${context}.bytes exceeds ${maximum} bytes`);
  // Per-chain native decoding and bootstrap authentication are node-owned checks.
  return { network: source, bytes };
}

function normalizeSccpParameters(value, context) {
  const record = exactRecord(value, [
    "enabled", ...Object.keys(SCCP_PARAMETER_INTEGER_BOUNDS),
    "min_outbound_amount", "inbound_self_claim_fee",
  ], context);
  if (typeof record.enabled !== "boolean") {
    throw new TypeError(`${context}.enabled must be a boolean`);
  }
  const result = { ...record };
  for (const [field, [minimum, maximum]] of Object.entries(SCCP_PARAMETER_INTEGER_BOUNDS)) {
    const integer = jsonUint(record[field], `${context}.${field}`);
    if (integer < minimum || integer > maximum) {
      throw new TypeError(`${context}.${field} must be within ${minimum}..${maximum}`);
    }
    result[field] = integer;
  }
  result.min_outbound_amount = sccpAmount(
    record.min_outbound_amount, 1n, 1_000_000_000_000_000n, `${context}.min_outbound_amount`,
  );
  result.inbound_self_claim_fee = sccpAmount(
    record.inbound_self_claim_fee, 0n, 10_000_000_000n, `${context}.inbound_self_claim_fee`,
  );
  if (![5, 7, 9].includes(result.fast_pause_panel_seats)) {
    throw new TypeError(`${context}.fast_pause_panel_seats must be 5, 7 or 9`);
  }
  // BigInt keeps sums and doubled values exact even at the JSON-safe input maximum.
  const validityFloor = BigInt(result.fast_pause_hold_ms)
    + 2n * BigInt(result.committee_heartbeat_ms) + 86_400_000n;
  if (BigInt(result.committee_validity_ms) < validityFloor) {
    throw new TypeError(`${context}.committee_validity_ms is below the hold, heartbeat and pin margin`);
  }
  if (2n * BigInt(result.fast_pause_seat_accept_window_ms) > BigInt(result.fast_pause_panel_term_ms)) {
    throw new TypeError(`${context}.fast_pause_seat_accept_window_ms exceeds half the panel term`);
  }
  const renewLead = BigInt(result.fast_pause_renew_lead_ms);
  if (renewLead < 2n * BigInt(result.fast_pause_endorse_window_ms)
      || 2n * renewLead > BigInt(result.fast_pause_hold_ms)) {
    throw new TypeError(`${context}.fast_pause_renew_lead_ms must span two endorsement windows and fit half the hold`);
  }
  // Enactment checks the committed staking delay and Parliament attempt latency.
  return result;
}

function sccpAmount(value, minimum, maximum, context) {
  if (typeof value !== "string" || value.length > maximum.toString().length
      || !/^(?:0|[1-9][0-9]*)$/u.test(value)) {
    throw new TypeError(`${context} must be a bounded canonical unsigned decimal string`);
  }
  const integer = BigInt(value);
  if (integer < minimum || integer > maximum) {
    throw new TypeError(`${context} must be within ${minimum}..${maximum}`);
  }
  return value;
}

function normalizeSccpCheckpoint(value, context) {
  const record = exactRecord(value, [
    "source_height", "block_hash", "state_root", "receipts_or_tx_root", "source_time_ms",
  ], context);
  return {
    source_height: jsonUint(record.source_height, `${context}.source_height`),
    block_hash: byteArray(record.block_hash, 32, `${context}.block_hash`),
    state_root: record.state_root === null ? null : byteArray(record.state_root, 32, `${context}.state_root`),
    receipts_or_tx_root: byteArray(record.receipts_or_tx_root, 32, `${context}.receipts_or_tx_root`),
    source_time_ms: jsonUint(record.source_time_ms, `${context}.source_time_ms`),
  };
}

function sccpUint32(value, context) {
  const integer = jsonUint(value, context, { allowZero: false });
  if (integer > 0xffff_ffff) {
    throw new TypeError(`${context} must fit in an unsigned 32-bit integer`);
  }
  return integer;
}

function normalizeSccpProgress(value, context) {
  const record = exactRecord(value, ["at", "detail"], context);
  if (record.at === "frozen") {
    if (record.detail !== null) throw new TypeError(`${context}.detail must be null`);
    return { at: "frozen", detail: null };
  }
  if (record.at !== "generation") {
    throw new TypeError(`${context}.at must be frozen or generation`);
  }
  const label = `${context}.detail`;
  const detail = exactRecord(record.detail, ["generation", "committee_root"], label);
  return {
    at: "generation",
    detail: {
      generation: jsonUint(detail.generation, `${label}.generation`, { allowZero: false }),
      committee_root: byteArray(detail.committee_root, 32, `${label}.committee_root`, { nonZero: true }),
    },
  };
}

function normalizeSccpSubject(value, context) {
  const record = exactRecord(value, ["subject", "key"], context);
  const tag = nonEmptyString(record.subject, `${context}.subject`);
  if (!SCCP_SUBJECTS.includes(tag)) {
    throw new TypeError(`${context}.subject contains an unsupported SCCP subject: ${tag}`);
  }
  if (tag === "parameters") {
    if (record.key !== null) throw new TypeError(`${context}.key must be null`);
    return { subject: tag, key: null };
  }
  return { subject: tag, key: normalizeSccpNetwork(record.key, `${context}.key`) };
}

function normalizeSccpNetwork(value, context) {
  const record = exactRecord(value, ["network", "profile"], context);
  if (!SCCP_EXTERNAL_NETWORKS.includes(record.network)) {
    throw new TypeError(`${context}.network must name a supported external SCCP network`);
  }
  if (record.profile !== null) throw new TypeError(`${context}.profile must be null`);
  return { network: record.network, profile: null };
}

function sccpSubjectOrder({ subject, key }) {
  return SCCP_SUBJECTS.indexOf(subject) * 4
    + (key === null ? 0 : SCCP_EXTERNAL_NETWORKS.indexOf(key.network) + 1);
}

function requireNativeFeeHash(value, context) {
  if (typeof value !== "string" || !/^[0-9A-F]{64}$/.test(value) || /^0+$/.test(value)) {
    rejectFeeType(`${context} must be a nonzero canonical uppercase 32-byte hex string`);
  }
  return value;
}

export function normalizeValidationFeePolicy(payload, context) {
  const record = exactRecord(
    payload,
    ["proposal_operator", "policy"],
    context,
  );
  const policy = normalizeValidationFeePolicyValue(
    record.policy,
    `${context}.policy`,
  );
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    policy,
  };
}

export function normalizeValidationFeePolicyValue(payload, context) {
  const record = exactRecord(payload, [
    "schema_version", "network_id", "policy_version", "previous_policy_hash",
    "ds_asset_id", "ds_scale", "retail_schedule", "effective_from_ms",
    "notice_published_at_ms", "fee", "treasury_account_id", "charging_mode",
    "exemption_classes",
    "reward_custody",
  ], context);
  if (record.schema_version !== 1 || record.ds_scale !== 2) {
    rejectFeeType(`${context} requires policy schema 1 and SBD scale 2`);
  }
  const networkId = nonEmptyString(record.network_id, `${context}.network_id`);
  NetworkId.parse(networkId);
  const policyVersion = uint64String(record.policy_version,
    `${context}.policy_version`, { allowZero: false });
  const previousPolicyHash = record.previous_policy_hash === null ? null
    : requireNativeFeeHash(record.previous_policy_hash, `${context}.previous_policy_hash`);
  if ((policyVersion === "1") !== (previousPolicyHash === null)) {
    rejectFeeType(`${context}.previous_policy_hash does not match policy_version`);
  }
  const fee = canonicalQuantity(record.fee, `${context}.fee`);
  if (fee === "0" || (fee.split(".")[1]?.length ?? 0) > 2) {
    rejectFeeType(`${context}.fee must be positive exact SBD minor units`);
  }
  const chargingMode = normalizeValidationFeeChargingMode(record.charging_mode,
    `${context}.charging_mode`);
  const schedule = exactRecord(record.retail_schedule,
    ["included_payments", "overage_minor", "maintenance_tiers"], `${context}.retail_schedule`);
  const included = feeJsonUint(schedule.included_payments,
    `${context}.retail_schedule.included_payments`, { allowZero: false });
  if (BigInt(included) > 0xffffffffn) rejectFeeType(`${context}.included_payments exceeds u32`);
  const overage = feeJsonUint(schedule.overage_minor,
    `${context}.retail_schedule.overage_minor`, { allowZero: false });
  const tiers = array(schedule.maintenance_tiers,
    `${context}.retail_schedule.maintenance_tiers`).map((tier, index) => {
    const label = `${context}.retail_schedule.maintenance_tiers[${index}]`;
    const value = exactRecord(tier,
      ["minimum_average_balance_minor", "monthly_fee_minor"], label);
    return {
      minimum_average_balance_minor: feeJsonUint(
        value.minimum_average_balance_minor, `${label}.minimum_average_balance_minor`),
      monthly_fee_minor: feeJsonUint(value.monthly_fee_minor,
        `${label}.monthly_fee_minor`, { allowZero: false }),
    };
  });
  if (tiers.length === 0 || tiers.length > 32 ||
      BigInt(tiers[0].minimum_average_balance_minor) !== 0n ||
      tiers.some((tier, i) => i > 0 && (
        BigInt(tier.minimum_average_balance_minor) <= BigInt(tiers[i - 1].minimum_average_balance_minor) ||
        BigInt(tier.monthly_fee_minor) < BigInt(tiers[i - 1].monthly_fee_minor)))) {
    rejectFeeType(`${context}.retail_schedule requires increasing thresholds and nondecreasing rates`);
  }
  const effective = feeJsonUint(record.effective_from_ms,
    `${context}.effective_from_ms`, { allowZero: false });
  const notice = feeJsonUint(record.notice_published_at_ms,
    `${context}.notice_published_at_ms`, { allowZero: false });
  const honiara = new Date(Number(BigInt(effective) + 39600000n));
  if (BigInt(effective) < BigInt(notice) + 2592000000n ||
      !Number.isFinite(honiara.getTime()) || honiara.getUTCDate() !== 1 ||
      honiara.getUTCHours() !== 0 || honiara.getUTCMinutes() !== 0 ||
      honiara.getUTCSeconds() !== 0 || honiara.getUTCMilliseconds() !== 0) {
    rejectFeeType(`${context} requires a Honiara month boundary after thirty calendar days notice`);
  }
  if (!Array.isArray(record.exemption_classes) || record.exemption_classes.length !== 1 ||
      record.exemption_classes[0] !== "TREASURY_PAYOUT") {
    rejectFeeType(`${context} requires governed reward custody and no automatic policy expiry`);
  }
  const payout = normalizeValidationFeeRewardCustody(record.reward_custody,
    `${context}.reward_custody`);
  const dsAsset = canonicalAssetDefinitionId(record.ds_asset_id, `${context}.ds_asset_id`);
  const treasury = canonicalAccountId(record.treasury_account_id,
    `${context}.treasury_account_id`);
  if (payout.ds_asset_id !== dsAsset || payout.treasury_account_id !== treasury) {
    rejectFeeType(`${context} payout custody must match the fee asset and treasury`);
  }
  return {
    schema_version: 1, network_id: networkId, policy_version: policyVersion,
    previous_policy_hash: previousPolicyHash, ds_asset_id: dsAsset, ds_scale: 2,
    retail_schedule: { included_payments: Number(included), overage_minor: overage, maintenance_tiers: tiers },
    effective_from_ms: effective, notice_published_at_ms: notice, fee,
    treasury_account_id: treasury, charging_mode: chargingMode,
    exemption_classes: ["TREASURY_PAYOUT"],
    reward_custody: payout,
  };
}

function normalizeValidationFeeChargingMode(payload, context) {
  const record = exactRecord(payload, ["charging_mode", "value"], context);
  if (record.charging_mode !== "RETAIL_MONTHLY_ALLOWANCE" || record.value !== null) {
    rejectFeeType(`${context} must be RETAIL_MONTHLY_ALLOWANCE with null value`);
  }
  return { charging_mode: "RETAIL_MONTHLY_ALLOWANCE", value: null };
}

export function normalizeValidationFeePayoutLifecycle(payload, context) {
  const record = exactRecord(
    payload,
    ["proposal_operator", "payout_binding"],
    context,
  );
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    payout_binding: normalizeValidationFeePayoutBinding(
      record.payout_binding,
      `${context}.payout_binding`,
    ),
  };
}

export function normalizeValidationFeePayoutBinding(payload, context) {
  const record = exactRecord(payload, [
    "contract_address", "code_hash", "entrypoint", "treasury_account_id", "ds_asset_id", "xor_asset_id",
    "pool_contract_address", "pool_code_hash", "pool_vault_account_id", "reward_pool_account_id",
    "reference_feed_id", "reference_feed_config_version", "reference_provider_accounts",
    "max_sbd_per_attempt_minor", "max_sbd_per_day_minor", "min_interval_ms", "max_source_age_ms",
    "max_slippage_bps", "validator_lane_id", "min_reward_claim_xor_minor",
  ], context);
  const result = {};
  for (const field of ["contract_address", "pool_contract_address"]) {
    result[field] = nonEmptyString(record[field], `${context}.${field}`);
    parseCanonicalContractAddress(result[field], `${context}.${field}`);
  }
  for (const field of ["code_hash", "pool_code_hash"]) {
    result[field] = requireNativeFeeHash(record[field], `${context}.${field}`);
  }
  if (record.entrypoint !== "autonomous_validation_fee_tick") {
    rejectFeeType(`${context}.entrypoint must be autonomous_validation_fee_tick`);
  }
  result.entrypoint = record.entrypoint;
  const custody = ["treasury_account_id", "pool_vault_account_id", "reward_pool_account_id"];
  for (const field of custody) {
    result[field] = canonicalAccountId(record[field], `${context}.${field}`);
  }
  if (new Set(custody.map((field) => result[field])).size !== 3) {
    rejectFeeType(`${context} treasury, pool and reward custody must differ`);
  }
  for (const field of ["ds_asset_id", "xor_asset_id"]) {
    result[field] = canonicalAssetDefinitionId(record[field], `${context}.${field}`);
  }
  if (result.ds_asset_id === result.xor_asset_id) rejectFeeType(`${context} SBD and XOR assets must differ`);
  if (!Array.isArray(record.reference_feed_id) || record.reference_feed_id.length !== 1) {
    rejectFeeType(`${context}.reference_feed_id must be the native one-element FeedId tuple`);
  }
  result.reference_feed_id = [canonicalName(record.reference_feed_id[0], `${context}.reference_feed_id[0]`)];
  result.reference_provider_accounts = array(record.reference_provider_accounts,
    `${context}.reference_provider_accounts`).map((item, index) =>
    canonicalAccountId(item, `${context}.reference_provider_accounts[${index}]`));
  if (result.reference_provider_accounts.length !== 5 || new Set(result.reference_provider_accounts).size !== 5) {
    rejectFeeType(`${context} requires five distinct reference providers`);
  }
  const providerKeys = result.reference_provider_accounts.map((account) => {
    const bytes = AccountAddress.parseEncoded(account).address.canonicalBytes();
    if (bytes[1] !== 0) throw new TypeError(`${context} reference providers must have single-signature controllers`);
    return Buffer.from(bytes).toString("hex");
  });
  if (new Set(providerKeys).size !== 5) throw new TypeError(`${context} reference providers must have independent signing keys`);
  for (const field of ["reference_feed_config_version", "max_sbd_per_attempt_minor", "max_sbd_per_day_minor",
    "min_interval_ms", "max_source_age_ms", "min_reward_claim_xor_minor", "max_slippage_bps", "validator_lane_id"]) {
    result[field] = feeJsonUint(record[field], `${context}.${field}`,
      { allowZero: field === "max_slippage_bps" || field === "validator_lane_id" });
  }
  if (BigInt(result.reference_feed_config_version) > 0xffffffffn ||
      BigInt(result.validator_lane_id) > 0xffffffffn || BigInt(result.max_slippage_bps) >= 10000n ||
      BigInt(result.max_sbd_per_day_minor) < BigInt(result.max_sbd_per_attempt_minor)) {
    rejectFeeType(`${context} conversion limits exceed native bounds`);
  }
  return result;
}


function rejectFeeType(message) { throw new TypeError(message); }
function feeJsonUint(value, context, options = {}) {
  if ((typeof value !== "number" || !Number.isSafeInteger(value)) && typeof value !== "bigint") {
    throw new TypeError(`${context} requires a lossless JSON integer token`);
  }
  const integer = BigInt(value);
  if (integer < 0n || integer > MAX_UINT64_BIGINT || (options.allowZero === false && integer === 0n)) {
    throw new TypeError(`${context} is outside unsigned 64-bit bounds`);
  }
  return integer <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(integer) : integer;
}
export function normalizeValidationFeeRewardCustody(value, context) {
  const custody = exactRecord(value, ["contract_address", "treasury_account_id", "ds_asset_id", "xor_asset_id", "reward_pool_account_id", "validator_lane_id"], context);
  const result = { ...custody };
  parseCanonicalContractAddress(nonEmptyString(custody.contract_address, `${context}.contract_address`), `${context}.contract_address`);
  for (const field of ["treasury_account_id", "reward_pool_account_id"]) result[field] = canonicalAccountId(custody[field], `${context}.${field}`);
  for (const field of ["ds_asset_id", "xor_asset_id"]) result[field] = canonicalAssetDefinitionId(custody[field], `${context}.${field}`);
  result.validator_lane_id = feeJsonUint(custody.validator_lane_id, `${context}.validator_lane_id`);
  if (result.ds_asset_id === result.xor_asset_id || result.treasury_account_id === result.reward_pool_account_id || BigInt(result.validator_lane_id) > 0xffff_ffffn) throw new TypeError(`${context} has invalid reward custody`);
  return result;
}

function normalizeMusubiAction(value, context) {
  const record = exactRecord(value, ["kind", "value"], context);
  const valueContext = `${context}.value`;
  switch (record.kind) {
    case "RecoverPackageOwners": {
      const action = exactRecord(
        record.value,
        ["package", "owners", "expected_revision"],
        valueContext,
      );
      const owners = array(action.owners, `${valueContext}.owners`)
        .map((owner, index) => canonicalAccountId(
          owner,
          `${valueContext}.owners[${index}]`,
        ));
      if (owners.length === 0 || owners.length > 64 || new Set(owners).size !== owners.length) {
        throw new TypeError(`${valueContext}.owners must contain 1-64 unique accounts`);
      }
      return {
        kind: "RecoverPackageOwners",
        value: {
          package: normalizeMusubiPackage(action.package, `${valueContext}.package`),
          owners,
          expected_revision: jsonUint(
            action.expected_revision,
            `${valueContext}.expected_revision`,
            { allowZero: false },
          ),
        },
      };
    }
    case "RetargetAlias": {
      const action = exactRecord(
        record.value,
        ["alias", "target", "expected_revision"],
        valueContext,
      );
      const alias = asciiKebab(
        stringTuple(action.alias, `${valueContext}.alias`),
        `${valueContext}.alias[0]`,
        32,
      );
      return {
        kind: "RetargetAlias",
        value: {
          alias: [alias],
          target: normalizeMusubiPackage(action.target, `${valueContext}.target`),
          expected_revision: jsonUint(
            action.expected_revision,
            `${valueContext}.expected_revision`,
            { allowZero: false },
          ),
        },
      };
    }
    case "TakedownArtifact": {
      const action = exactRecord(
        record.value,
        ["release", "reason", "expected_artifact_governance_revision"],
        valueContext,
      );
      const reason = boundedReason(
        stringTuple(action.reason, `${valueContext}.reason`),
        `${valueContext}.reason[0]`,
      );
      return {
        kind: "TakedownArtifact",
        value: {
          release: normalizeMusubiRelease(action.release, `${valueContext}.release`),
          reason: [reason],
          expected_artifact_governance_revision: jsonUint(
            action.expected_artifact_governance_revision,
            `${valueContext}.expected_artifact_governance_revision`,
            { allowZero: false },
          ),
        },
      };
    }
    case "SetRegistryPolicy": {
      const action = exactRecord(
        record.value,
        ["policy", "expected_revision"],
        valueContext,
      );
      const expectedRevision = jsonUint(
        action.expected_revision,
        `${valueContext}.expected_revision`,
        { allowZero: false },
      );
      const policy = normalizeMusubiRegistryPolicy(
        action.policy,
        `${valueContext}.policy`,
      );
      if (policy.revision !== expectedRevision + 1) {
        throw new TypeError(`${valueContext}.policy.revision must follow expected_revision`);
      }
      return {
        kind: "SetRegistryPolicy",
        value: { policy, expected_revision: expectedRevision },
      };
    }
    default:
      throw new TypeError(`${context}.kind contains an unsupported Musubi action`);
  }
}

function normalizeMusubiPackage(value, context) {
  const record = exactRecord(value, ["home_dataspace", "scope", "name"], context);
  const scope = exactRecord(record.scope, ["kind", "value"], `${context}.scope`);
  let normalizedScope;
  if (scope.kind === "DataspaceRoot" && scope.value === null) {
    normalizedScope = { kind: "DataspaceRoot", value: null };
  } else if (scope.kind === "Domain") {
    normalizedScope = {
      kind: "Domain",
      value: canonicalName(scope.value, `${context}.scope.value`),
    };
  } else {
    throw new TypeError(`${context}.scope contains an unsupported package scope`);
  }
  const name = asciiKebab(
    stringTuple(record.name, `${context}.name`),
    `${context}.name[0]`,
    64,
  );
  return {
    home_dataspace: jsonUint(record.home_dataspace, `${context}.home_dataspace`),
    scope: normalizedScope,
    name: [name],
  };
}

function normalizeMusubiRelease(value, context) {
  const record = exactRecord(value, ["package", "version"], context);
  const version = exactRecord(
    record.version,
    ["major", "minor", "patch", "prerelease"],
    `${context}.version`,
  );
  const prerelease = array(version.prerelease, `${context}.version.prerelease`)
    .map((item, index) => {
      const itemContext = `${context}.version.prerelease[${index}]`;
      const identifier = exactRecord(item, ["kind", "value"], itemContext);
      if (identifier.kind === "Numeric") {
        return {
          kind: "Numeric",
          value: jsonUint(identifier.value, `${itemContext}.value`),
        };
      }
      if (
        identifier.kind === "AlphaNumeric" &&
        typeof identifier.value === "string" &&
        identifier.value.length <= 64 &&
        /^(?=.*[A-Za-z-])[A-Za-z0-9-]+$/u.test(identifier.value)
      ) {
        return { kind: "AlphaNumeric", value: identifier.value };
      }
      throw new TypeError(`${itemContext} contains an unsupported prerelease identifier`);
    });
  if (prerelease.length > 16) {
    throw new TypeError(`${context}.version.prerelease exceeds the V1 bound`);
  }
  return {
    package: normalizeMusubiPackage(record.package, `${context}.package`),
    version: {
      major: jsonUint(version.major, `${context}.version.major`),
      minor: jsonUint(version.minor, `${context}.version.minor`),
      patch: jsonUint(version.patch, `${context}.version.patch`),
      prerelease,
    },
  };
}

function normalizeMusubiRegistryPolicy(value, context) {
  const record = exactRecord(value, [
    "version",
    "revision",
    "mode",
    "allowlisted_dataspaces",
    "alias_pricing",
  ], context);
  if (record.version !== 1) {
    throw new TypeError(`${context}.version must be the number 1`);
  }
  const mode = exactRecord(record.mode, ["kind", "value"], `${context}.mode`);
  if (!["Closed", "Allowlisted", "Open"].includes(mode.kind) || mode.value !== null) {
    throw new TypeError(`${context}.mode contains an unsupported registry mode`);
  }
  const allowlistedDataspaces = array(
    record.allowlisted_dataspaces,
    `${context}.allowlisted_dataspaces`,
  ).map((item, index) => jsonUint(
    item,
    `${context}.allowlisted_dataspaces[${index}]`,
  ));
  if (
    new Set(allowlistedDataspaces).size !== allowlistedDataspaces.length ||
    allowlistedDataspaces.some(
      (item, index) => index > 0 && allowlistedDataspaces[index - 1] >= item,
    )
  ) {
    throw new TypeError(`${context}.allowlisted_dataspaces must be sorted and unique`);
  }
  if (mode.kind !== "Allowlisted" && allowlistedDataspaces.length > 0) {
    throw new TypeError(`${context}.allowlisted_dataspaces does not match mode`);
  }
  const pricing = exactRecord(record.alias_pricing, [
    "revision",
    "length_1_xor",
    "length_2_xor",
    "length_3_xor",
    "length_4_xor",
    "length_5_to_32_xor",
  ], `${context}.alias_pricing`);
  const normalizedPricing = {};
  for (const field of [
    "revision",
    "length_1_xor",
    "length_2_xor",
    "length_3_xor",
    "length_4_xor",
    "length_5_to_32_xor",
  ]) {
    normalizedPricing[field] = jsonUint(
      pricing[field],
      `${context}.alias_pricing.${field}`,
      { allowZero: false },
    );
  }
  return {
    version: 1,
    revision: jsonUint(record.revision, `${context}.revision`, { allowZero: false }),
    mode: { kind: mode.kind, value: null },
    allowlisted_dataspaces: allowlistedDataspaces,
    alias_pricing: normalizedPricing,
  };
}

function normalizeSorafsProvider(value, context) {
  const record = exactRecord(value, ["action"], context);
  const actionContext = `${context}.action`;
  const action = exactRecord(record.action, ["action", "value"], actionContext);
  const valueContext = `${actionContext}.value`;
  if (action.action === "establish") {
    const actionValue = exactRecord(action.value, ["provider_id", "owner"], valueContext);
    return {
      action: {
        action: "establish",
        value: {
          provider_id: providerIdTuple(actionValue.provider_id, `${valueContext}.provider_id`),
          owner: canonicalAccountId(actionValue.owner, `${valueContext}.owner`),
        },
      },
    };
  }
  if (action.action === "rebind") {
    const actionValue = exactRecord(
      action.value,
      ["provider_id", "expected_owner", "next_owner"],
      valueContext,
    );
    const expectedOwner = canonicalAccountId(
      actionValue.expected_owner,
      `${valueContext}.expected_owner`,
    );
    const nextOwner = canonicalAccountId(
      actionValue.next_owner,
      `${valueContext}.next_owner`,
    );
    if (expectedOwner === nextOwner) {
      throw new TypeError(`${valueContext}.next_owner must differ from expected_owner`);
    }
    return {
      action: {
        action: "rebind",
        value: {
          provider_id: providerIdTuple(actionValue.provider_id, `${valueContext}.provider_id`),
          expected_owner: expectedOwner,
          next_owner: nextOwner,
        },
      },
    };
  }
  if (action.action === "remove") {
    const actionValue = exactRecord(
      action.value,
      ["provider_id", "expected_owner"],
      valueContext,
    );
    return {
      action: {
        action: "remove",
        value: {
          provider_id: providerIdTuple(actionValue.provider_id, `${valueContext}.provider_id`),
          expected_owner: canonicalAccountId(
            actionValue.expected_owner,
            `${valueContext}.expected_owner`,
          ),
        },
      },
    };
  }
  throw new TypeError(`${actionContext}.action contains an unsupported provider action`);
}

function normalizeContractLifecycle(value, context) {
  const record = exactRecord(
    value,
    ["proposal_operator", "contract_address", "expected_revision", "action"],
    context,
  );
  const contractAddress = nonEmptyString(record.contract_address, `${context}.contract_address`);
  parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    contract_address: contractAddress,
    expected_revision: jsonUint(
      record.expected_revision,
      `${context}.expected_revision`,
      { allowZero: false },
    ),
    action: normalizeContractLifecycleAction(record.action, `${context}.action`),
  };
}

function normalizeContractLifecycleAction(value, context) {
  if (!plainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const tag = nonEmptyString(value.action, `${context}.action`);
  if (tag === "CancelOwnershipOffer" || tag === "AcceptParliamentOwnership") {
    const unit = exactRecord(value, ["action", "payload"], context);
    if (unit.payload !== null) {
      throw new TypeError(`${context}.payload must be null for ${tag}`);
    }
    return { action: tag, payload: null };
  }
  const record = exactRecord(value, ["action", "payload"], context);
  const payloadContext = `${context}.payload`;
  switch (tag) {
    case "Activate": {
      const payload = exactRecord(record.payload, [
        "code_hash",
        "abi_hash",
        "abi_version",
        "manifest_provenance",
      ], payloadContext);
      if (payload.abi_version !== 1) {
        throw new TypeError(`${payloadContext}.abi_version must be the number 1`);
      }
      return {
        action: tag,
        payload: {
          code_hash: lowerHex32(payload.code_hash, `${payloadContext}.code_hash`),
          abi_hash: lowerHex32(payload.abi_hash, `${payloadContext}.abi_hash`),
          abi_version: 1,
          manifest_provenance: payload.manifest_provenance === null
            ? null
            : normalizeManifestProvenance(
              payload.manifest_provenance,
              `${payloadContext}.manifest_provenance`,
            ),
        },
      };
    }
    case "Deactivate": {
      const hasReason = plainObject(record.payload) && Object.hasOwn(record.payload, "reason");
      const payload = exactRecord(
        record.payload,
        hasReason ? ["expected_code_hash", "reason"] : ["expected_code_hash"],
        payloadContext,
      );
      return {
        action: tag,
        payload: {
          expected_code_hash: lowerHex32(
            payload.expected_code_hash,
            `${payloadContext}.expected_code_hash`,
          ),
          reason: !hasReason || payload.reason === null
            ? null
            : exactString(payload.reason, `${payloadContext}.reason`),
        },
      };
    }
    case "OfferOwnership": {
      const payload = exactRecord(record.payload, ["new_owner"], payloadContext);
      return {
        action: tag,
        payload: {
          new_owner: canonicalAccountId(payload.new_owner, `${payloadContext}.new_owner`),
        },
      };
    }
    case "CompleteEmergencyHoldRetrospective": {
      const payload = exactRecord(record.payload, [
        "hold_proposal_content_id",
        "hold_governance_attempt_id",
        "incident_digest",
        "retrospective_finding_root",
      ], payloadContext);
      return {
        action: tag,
        payload: {
          hold_proposal_content_id: byteArray(
            payload.hold_proposal_content_id,
            32,
            `${payloadContext}.hold_proposal_content_id`,
            { nonZero: true },
          ),
          hold_governance_attempt_id: byteArray(
            payload.hold_governance_attempt_id,
            32,
            `${payloadContext}.hold_governance_attempt_id`,
            { nonZero: true },
          ),
          incident_digest: byteArray(
            payload.incident_digest,
            32,
            `${payloadContext}.incident_digest`,
            { nonZero: true },
          ),
          retrospective_finding_root: byteArray(
            payload.retrospective_finding_root,
            32,
            `${payloadContext}.retrospective_finding_root`,
            { nonZero: true },
          ),
        },
      };
    }
    default:
      throw new TypeError(`${context}.action contains an unsupported lifecycle action`);
  }
}

function normalizeContractEmergencyHold(value, context) {
  const record = exactRecord(value, [
    "contract_address",
    "expected_revision",
    "expected_code_hash",
    "incident_digest",
    "reason",
    "duration_blocks",
  ], context);
  const contractAddress = nonEmptyString(record.contract_address, `${context}.contract_address`);
  parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
  const durationBlocks = jsonUint(
    record.duration_blocks,
    `${context}.duration_blocks`,
    { allowZero: false },
  );
  if (durationBlocks > 3_600) {
    throw new TypeError(`${context}.duration_blocks exceeds the V1 maximum of 3600`);
  }
  const reason = exactString(record.reason, `${context}.reason`);
  if (reason.trim().length === 0) {
    throw new TypeError(`${context}.reason must not be blank`);
  }
  return {
    contract_address: contractAddress,
    expected_revision: jsonUint(
      record.expected_revision,
      `${context}.expected_revision`,
      { allowZero: false },
    ),
    expected_code_hash: lowerHex32(record.expected_code_hash, `${context}.expected_code_hash`),
    incident_digest: byteArray(
      record.incident_digest,
      32,
      `${context}.incident_digest`,
      { nonZero: true },
    ),
    reason,
    duration_blocks: durationBlocks,
  };
}

function normalizeGlobalDataTriggerPermission(value, context) {
  const record = exactRecord(value, ["authority", "action"], context);
  const action = exactRecord(record.action, ["action", "value"], `${context}.action`);
  const kind = nonEmptyString(action.action, `${context}.action.action`);
  if (kind !== "grant" && kind !== "revoke") {
    throw new TypeError(`${context}.action.action must be grant or revoke`);
  }
  if (action.value !== null) {
    throw new TypeError(`${context}.action.value must be null`);
  }
  return {
    authority: ensureCanonicalAccountId(record.authority, `${context}.authority`),
    action: { action: kind, value: null },
  };
}

function exactRecord(value, fields, context) {
  if (!plainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  const expected = new Set(fields);
  const unknown = Object.keys(value).filter((field) => !expected.has(field));
  if (unknown.length !== 0) {
    throw new TypeError(`${context} contains unsupported fields: ${unknown.sort().join(", ")}`);
  }
  const missing = fields.filter((field) => !Object.hasOwn(value, field));
  if (missing.length !== 0) {
    throw new TypeError(`${context} is missing required fields: ${missing.join(", ")}`);
  }
  return value;
}

function plainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function array(value, context) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  return value;
}

function byteArray(value, length, context, options = {}) {
  const bytes = array(value, context);
  if (
    bytes.length !== length ||
    Array.from(bytes).some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
  ) {
    throw new TypeError(`${context} must contain exactly ${length} JSON byte values`);
  }
  if (options.nonZero === true && bytes.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must not be all zero`);
  }
  return [...bytes];
}

function providerIdTuple(value, context) {
  const tuple = array(value, context);
  if (tuple.length !== 1) {
    throw new TypeError(`${context} must be the exact one-field ProviderId tuple`);
  }
  return [byteArray(tuple[0], 32, `${context}[0]`, { nonZero: true })];
}

function stringTuple(value, context) {
  if (!Array.isArray(value) || value.length !== 1 || typeof value[0] !== "string") {
    throw new TypeError(`${context} must be the exact one-field string tuple`);
  }
  return value[0];
}

function jsonUint(value, context, options = {}) {
  const minimum = options.allowZero === false ? 1 : 0;
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum) {
    throw new TypeError(
      `${context} must be a ${minimum === 0 ? "non-negative" : "positive"} JSON safe integer`,
    );
  }
  return value;
}

function uint64String(value, context, options = {}) {
  if (typeof value !== "string" || !/^(?:0|[1-9][0-9]*)$/u.test(value)) {
    throw new TypeError(`${context} must be a canonical unsigned 64-bit decimal string`);
  }
  const parsed = BigInt(value);
  if (parsed > MAX_UINT64_BIGINT || (options.allowZero === false && parsed === 0n)) {
    throw new TypeError(`${context} is outside the supported unsigned 64-bit range`);
  }
  return value;
}

function lowerHex32(value, context) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be exactly 32 lowercase hexadecimal bytes`);
  }
  return value;
}

function canonicalAccountId(value, context) {
  const literal = nonEmptyString(value, context);
  const canonical = ensureCanonicalAccountId(literal, context);
  if (canonical !== literal) {
    throw new TypeError(`${context} must use the canonical account literal`);
  }
  const canonicalBytes = AccountAddress.parseEncoded(literal).address.canonicalBytes();
  const isSingleKey = canonicalBytes[1] === 0;
  const keyLength = canonicalBytes[3];
  if (
    isSingleKey &&
    canonicalBytes.length === 4 + keyLength &&
    canonicalBytes.slice(4).every((byte) => byte === 0)
  ) {
    throw new TypeError(`${context} must not contain a Rust-invalid all-zero public key`);
  }
  return literal;
}

function canonicalAssetDefinitionId(value, context) {
  const literal = nonEmptyString(value, context);
  const canonical = normalizeAssetDefinitionId(literal, context);
  if (canonical !== literal) {
    throw new TypeError(`${context} must use the canonical asset-definition literal`);
  }
  return literal;
}

function canonicalBase64(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical base64 string`);
  }
  if (value === "") return value;
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)) {
    throw new TypeError(`${context} must be canonical padded base64`);
  }
  try {
    strictDecodeBase64(value);
  } catch {
    throw new TypeError(`${context} must be canonical padded base64`);
  }
  return value;
}

function canonicalQuantity(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical Kotodama V1 quantity string`);
  }
  let canonical;
  try {
    canonical = NumericV1.decodeQuantityJson(value).toString();
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw new TypeError(`${context} must be a canonical non-negative Kotodama V1 quantity`);
  }
  if (canonical !== value) {
    throw new TypeError(`${context} must use the canonical quantity spelling`);
  }
  return canonical;
}

function exactString(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a string`);
  }
  return value;
}

function nonEmptyString(value, context) {
  if (typeof value !== "string" || value.length === 0) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value;
}

function asciiKebab(value, context, maxBytes) {
  const literal = nonEmptyString(value, context);
  if (
    Buffer.byteLength(literal, "utf8") > maxBytes ||
    !/^[a-z0-9]+(?:-[a-z0-9]+)*$/u.test(literal)
  ) {
    throw new TypeError(`${context} must be canonical lowercase ASCII kebab text`);
  }
  return literal;
}

function canonicalName(value, context) {
  const literal = nonEmptyString(value, context);
  if (
    Buffer.byteLength(literal, "utf8") > 255 ||
    literal.normalize("NFC") !== literal ||
    /[\s@#$\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(literal)
  ) {
    throw new TypeError(`${context} must be a canonical Iroha Name`);
  }
  return literal;
}

function boundedReason(value, context) {
  const literal = nonEmptyString(value, context);
  if (
    literal.trim() !== literal ||
    Buffer.byteLength(literal, "utf8") > 1024 ||
    /[\u0000-\u001f\u007f]/u.test(literal)
  ) {
    throw new TypeError(`${context} must be bounded canonical public text`);
  }
  return literal;
}
