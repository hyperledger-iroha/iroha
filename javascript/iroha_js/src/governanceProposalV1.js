// SPDX-License-Identifier: Apache-2.0

import { Buffer } from "buffer";

import { AccountAddress } from "./address.js";
import { parseCanonicalContractAddress } from "./contractAddress.js";
import { getCurveEntryByPublicKeyMulticodec } from "./curveRegistry.js";
import { assertValidEd25519PublicKey } from "./ed25519Strict.js";
import { validateKagemushaReleaseSchemaV1 } from "./governanceKagemushaReleaseSchemaV1.js";
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
    case "KagemushaVerifierPolicyInstall":
      return {
        kind,
        payload: normalizeKagemushaVerifierPolicyInstall(record.payload, payloadContext),
      };
    case "KagemushaVerifierReleaseInstall":
      return {
        kind,
        payload: normalizeKagemushaVerifierReleaseInstall(record.payload, payloadContext),
      };
    case "KagemushaVerifierReleaseActivate":
      return {
        kind,
        payload: normalizeKagemushaVerifierReleaseActivate(record.payload, payloadContext),
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

function normalizeRuntimeUpgrade(value, context) {
  const record = exactRecord(value, ["proposal_operator", "manifest"], context);
  const manifestContext = `${context}.manifest`;
  const manifest = exactRecord(record.manifest, [
    "name",
    "description",
    "abi_version",
    "abi_hash",
    "added_syscalls",
    "added_pointer_types",
    "start_height",
    "end_height",
    "sbom_digests",
    "slsa_attestation",
    "provenance",
  ], manifestContext);
  if (manifest.abi_version !== 1) {
    throw new TypeError(`${manifestContext}.abi_version must be the number 1`);
  }
  const addedSyscalls = uint16Array(
    manifest.added_syscalls,
    `${manifestContext}.added_syscalls`,
  );
  const addedPointerTypes = uint16Array(
    manifest.added_pointer_types,
    `${manifestContext}.added_pointer_types`,
  );
  if (addedSyscalls.length !== 0 || addedPointerTypes.length !== 0) {
    throw new TypeError(`${manifestContext} ABI delta lists must be empty in V1`);
  }
  const startHeight = jsonUint(manifest.start_height, `${manifestContext}.start_height`);
  const endHeight = jsonUint(manifest.end_height, `${manifestContext}.end_height`);
  if (endHeight <= startHeight) {
    throw new TypeError(`${manifestContext}.end_height must be greater than start_height`);
  }
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    manifest: {
      name: nonEmptyString(manifest.name, `${manifestContext}.name`),
      description: exactString(manifest.description, `${manifestContext}.description`),
      abi_version: 1,
      abi_hash: byteArray(manifest.abi_hash, 32, `${manifestContext}.abi_hash`),
      added_syscalls: addedSyscalls,
      added_pointer_types: addedPointerTypes,
      start_height: startHeight,
      end_height: endHeight,
      sbom_digests: array(manifest.sbom_digests, `${manifestContext}.sbom_digests`)
        .map((item, index) => {
          const itemContext = `${manifestContext}.sbom_digests[${index}]`;
          const digest = exactRecord(item, ["algorithm", "digest"], itemContext);
          return {
            algorithm: nonEmptyString(digest.algorithm, `${itemContext}.algorithm`),
            digest: canonicalBase64(digest.digest, `${itemContext}.digest`),
          };
        }),
      slsa_attestation: canonicalBase64(
        manifest.slsa_attestation,
        `${manifestContext}.slsa_attestation`,
      ),
      provenance: array(manifest.provenance, `${manifestContext}.provenance`)
        .map((item, index) => normalizeManifestProvenance(
          item,
          `${manifestContext}.provenance[${index}]`,
        )),
    },
  };
}

const SCCP_GOVERNANCE_MAX_ACTIONS_V1 = 16;

function normalizeSccpRoute(value, context) {
  const record = exactRecord(value, ["proposal"], context);
  const proposalContext = `${context}.proposal`;
  const proposal = exactRecord(
    record.proposal,
    ["network_id", "base_revisions", "actions"],
    proposalContext,
  );
  const networkId = nonEmptyString(proposal.network_id, `${proposalContext}.network_id`);
  NetworkId.parse(networkId);
  const baseRevisions = array(proposal.base_revisions, `${proposalContext}.base_revisions`);
  const actions = array(proposal.actions, `${proposalContext}.actions`);
  if (actions.length === 0 || actions.length > SCCP_GOVERNANCE_MAX_ACTIONS_V1) {
    throw new TypeError(
      `${proposalContext}.actions must contain 1..${SCCP_GOVERNANCE_MAX_ACTIONS_V1} actions`,
    );
  }
  // TODO: validate each SccpGovernanceBaseRevisionV1 and SccpGovernanceActionV1
  // entry statically (specs/sccp.md §4.14.3) once the SDK carries typed SCCP
  // governance codecs; Torii and the node remain authoritative for them today.
  return {
    proposal: {
      network_id: networkId,
      base_revisions: baseRevisions,
      actions,
    },
  };
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

const KAGEMUSHA_KEY_ORDINALS = new Map([
  [0xed, 0], [0xe7, 1], [0xea, 2], [0xeb, 3], [0xee, 4],
  [0x1200, 5], [0x1201, 6], [0x1202, 7], [0x1203, 8],
  [0x1204, 9], [0x1306, 10],
]);

function kagemushaCanonicalVarint(bytes, start, context) {
  let value = 0n;
  let shift = 0n;
  for (let index = start; index < bytes.length && shift <= 63n; index += 1) {
    const byte = bytes[index];
    const chunk = BigInt(byte & 0x7f);
    if (shift === 63n && chunk > 1n) break;
    value |= chunk << shift;
    if ((byte & 0x80) === 0) {
      if (index > start && chunk === 0n) break;
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) break;
      return [Number(value), index + 1];
    }
    shift += 7n;
  }
  throw new TypeError(`${context} has a malformed or noncanonical multihash varint`);
}

function kagemushaCanonicalPublicKey(value, context) {
  if (
    typeof value !== "string" || value.length > 1_048_576 ||
    value.length % 2 !== 0 || !/^[0-9a-fA-F]+$/u.test(value)
  ) {
    throw new TypeError(`${context} must be a bare canonical public-key multihash`);
  }
  const bytes = Buffer.from(value, "hex");
  const [code, codeEnd] = kagemushaCanonicalVarint(bytes, 0, context);
  const [length, payloadStart] = kagemushaCanonicalVarint(bytes, codeEnd, context);
  const ordinal = KAGEMUSHA_KEY_ORDINALS.get(code);
  const curve = getCurveEntryByPublicKeyMulticodec(code);
  if (ordinal === undefined || !curve || length === 0 || length !== bytes.length - payloadStart) {
    throw new TypeError(`${context} has an unsupported public-key algorithm or length`);
  }
  const payload = bytes.subarray(payloadStart);
  const canonical = bytes.subarray(0, payloadStart).toString("hex") + payload.toString("hex").toUpperCase();
  if (canonical !== value) {
    throw new TypeError(`${context} must be an exact canonical public-key multihash`);
  }
  if (code === 0x1306) {
    const sec1Length = curve.publicKeyLength;
    if (payload.length < 2 + sec1Length) {
      throw new TypeError(`${context} has an invalid SM2 public-key payload`);
    }
    const distidLength = (payload[0] << 8) | payload[1];
    const sec1Start = 2 + distidLength;
    if (
      distidLength > 0xffff / 8 || payload.length !== sec1Start + sec1Length ||
      payload[sec1Start] !== 0x04
    ) {
      throw new TypeError(`${context} has an invalid SM2 public-key payload`);
    }
    try {
      new TextDecoder("utf-8", { fatal: true }).decode(payload.subarray(2, sec1Start));
    } catch {
      throw new TypeError(`${context} has an invalid UTF-8 SM2 distinguished ID`);
    }
  } else if (payload.length !== curve.publicKeyLength) {
    throw new TypeError(`${context} has an invalid public-key payload length`);
  }
  if (code === 0xe7 && payload[0] !== 0x02 && payload[0] !== 0x03) {
    throw new TypeError(`${context} has an invalid secp256k1 public-key envelope`);
  }
  if (code === 0xee && payload.every((byte) => byte === 0)) {
    throw new TypeError(`${context} has an all-zero ML-DSA public key`);
  }
  if (code === 0xed) assertValidEd25519PublicKey(payload);
  return { literal: value, ordinal, payload };
}

function normalizeKagemushaVerifierPolicyInstall(value, context) {
  const record = exactRecord(value, [
    "proposal_operator", "network_id", "expected_predecessor", "authority_policy",
  ], context);
  const networkId = nonEmptyString(record.network_id, `${context}.network_id`);
  NetworkId.parse(networkId);
  const predecessorContext = `${context}.expected_predecessor`;
  const predecessor = exactRecord(record.expected_predecessor, [
    "version", "authority_policy", "active_release_id", "releases",
  ], predecessorContext);
  if (
    predecessor.version !== 1 || predecessor.authority_policy !== null ||
    predecessor.active_release_id !== null ||
    array(predecessor.releases, `${predecessorContext}.releases`).length !== 0
  ) {
    throw new TypeError(`${predecessorContext} must be the exact empty V1 verifier registry`);
  }
  const policyContext = `${context}.authority_policy`;
  const policy = exactRecord(record.authority_policy, [
    "version", "authority_set_id", "threshold", "authorized_signers",
  ], policyContext);
  if (policy.version !== 1) {
    throw new TypeError(`${policyContext}.version must be the number 1`);
  }
  const signers = array(policy.authorized_signers, `${policyContext}.authorized_signers`);
  if (signers.length === 0 || signers.length > 32) {
    throw new TypeError(`${policyContext}.authorized_signers must contain 1..32 keys`);
  }
  const threshold = jsonUint(policy.threshold, `${policyContext}.threshold`, { allowZero: false });
  if (threshold > signers.length) {
    throw new TypeError(`${policyContext}.threshold must not exceed signer count`);
  }
  const parsed = signers.map((signer, index) => kagemushaCanonicalPublicKey(
    signer, `${policyContext}.authorized_signers[${index}]`,
  ));
  for (let index = 1; index < parsed.length; index += 1) {
    const left = parsed[index - 1];
    const right = parsed[index];
    if (
      left.ordinal > right.ordinal ||
      (left.ordinal === right.ordinal && Buffer.compare(left.payload, right.payload) >= 0)
    ) {
      throw new TypeError(`${policyContext}.authorized_signers must be strictly ordered and unique`);
    }
  }
  return {
    proposal_operator: canonicalAccountId(record.proposal_operator, `${context}.proposal_operator`),
    network_id: networkId,
    expected_predecessor: {
      version: 1, authority_policy: null, active_release_id: null, releases: [],
    },
    authority_policy: {
      version: 1,
      authority_set_id: byteArray(policy.authority_set_id, 32, `${policyContext}.authority_set_id`, { nonZero: true }),
      threshold,
      authorized_signers: parsed.map(({ literal }) => literal),
    },
  };
}

function normalizeKagemushaVerifierReleaseInstall(value, context) {
  const record = exactRecord(value, [
    "proposal_operator", "network_id", "expected_predecessor", "manifest", "receipt", "attestation",
  ], context);
  const networkId = nonEmptyString(record.network_id, `${context}.network_id`);
  NetworkId.parse(networkId);
  const roots = {
    expected_predecessor: "GovernanceKagemushaGovernedVerifierRegistryV1",
    manifest: "GovernanceKagemushaReleaseManifestV1",
    receipt: "GovernanceKagemushaInternalValidationReceiptV1",
    attestation: "GovernanceKagemushaReleaseAttestationV1",
  };
  for (const [field, schema] of Object.entries(roots)) {
    validateKagemushaReleaseSchemaV1(schema, record[field]);
  }
  if (record.expected_predecessor.authority_policy === null) {
    throw new TypeError(`${context}.expected_predecessor requires a governed signer policy`);
  }
  if (!equalByteArrays(record.attestation.subject.release_id, record.manifest.release_id)) {
    throw new TypeError(`${context}.attestation.subject.release_id must match manifest.release_id`);
  }
  return {
    proposal_operator: canonicalAccountId(record.proposal_operator, `${context}.proposal_operator`),
    network_id: networkId,
    expected_predecessor: structuredClone(record.expected_predecessor),
    manifest: structuredClone(record.manifest),
    receipt: structuredClone(record.receipt),
    attestation: structuredClone(record.attestation),
  };
}

function normalizeKagemushaVerifierReleaseActivate(value, context) {
  const record = exactRecord(value, [
    "proposal_operator", "network_id", "expected_predecessor", "successor_release_id",
  ], context);
  const networkId = nonEmptyString(record.network_id, `${context}.network_id`);
  NetworkId.parse(networkId);
  const predecessor = record.expected_predecessor;
  validateKagemushaReleaseSchemaV1("GovernanceKagemushaGovernedVerifierRegistryV1", predecessor);
  validateKagemushaReleaseSchemaV1("GovernanceKagemushaBytes32V1", record.successor_release_id);
  if (predecessor.authority_policy === null) {
    throw new TypeError(`${context}.expected_predecessor requires a governed signer policy`);
  }
  if (predecessor.active_release_id !== null) {
    throw new TypeError(`${context}.expected_predecessor must be inactive`);
  }
  if (predecessor.releases.length !== 1) {
    throw new TypeError(`${context}.expected_predecessor requires exactly one standby release`);
  }
  const standby = predecessor.releases[0];
  if (standby.status !== 2 || !equalByteArrays(standby.release_id, record.successor_release_id)) {
    throw new TypeError(`${context}.successor_release_id must select the sole standby release`);
  }
  return {
    proposal_operator: canonicalAccountId(record.proposal_operator, `${context}.proposal_operator`),
    network_id: networkId,
    expected_predecessor: structuredClone(predecessor),
    successor_release_id: [...record.successor_release_id],
  };
}

function equalByteArrays(left, right) {
  return left.length === right.length && left.every((byte, index) => byte === right[index]);
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
    bytes.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
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

function uint16Array(value, context) {
  return array(value, context).map((item, index) => {
    const normalized = jsonUint(item, `${context}[${index}]`);
    if (normalized > 0xffff) {
      throw new TypeError(`${context}[${index}] must fit in an unsigned 16-bit integer`);
    }
    return normalized;
  });
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
