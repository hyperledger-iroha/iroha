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
import { normalizeSccpRouteGovernanceAction } from "./sccp.js";
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
  const record = exactRecord(value, ["manifest"], context);
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

function normalizeSccpRoute(value, context) {
  const record = exactRecord(value, ["anchor"], context);
  const anchorContext = `${context}.anchor`;
  const anchor = exactRecord(record.anchor, ["network_id", "action"], anchorContext);
  const networkId = nonEmptyString(anchor.network_id, `${anchorContext}.network_id`);
  NetworkId.parse(networkId);
  return {
    anchor: {
      network_id: networkId,
      action: normalizeSccpRouteGovernanceAction(anchor.action),
    },
  };
}

function normalizeValidationFeePolicy(value, context) {
  const record = exactRecord(value, [
    "proposal_operator",
    "policy",
    "payout_lifecycle_proposal_id",
  ], context);
  const policy = normalizeValidationFeePolicyValue(record.policy, `${context}.policy`);
  const lifecycleId = record.payout_lifecycle_proposal_id === null
    ? null
    : byteArray(
      record.payout_lifecycle_proposal_id,
      32,
      `${context}.payout_lifecycle_proposal_id`,
      { nonZero: true },
    );
  if ((policy.treasury_payout_binding === null) !== (lifecycleId === null)) {
    throw new TypeError(
      `${context}.payout_lifecycle_proposal_id must be present exactly when the policy has a payout binding`,
    );
  }
  return {
    proposal_operator: canonicalAccountId(
      record.proposal_operator,
      `${context}.proposal_operator`,
    ),
    policy,
    payout_lifecycle_proposal_id: lifecycleId,
  };
}

function normalizeValidationFeePolicyValue(value, context) {
  const record = exactRecord(value, [
    "schema_version",
    "network_id",
    "policy_version",
    "previous_policy_hash",
    "ds_asset_id",
    "ds_scale",
    "fee",
    "treasury_account_id",
    "charging_mode",
    "effective_from_height",
    "expires_after_height",
    "exemption_classes",
    "treasury_payout_binding",
  ], context);
  if (record.schema_version !== 1) {
    throw new TypeError(`${context}.schema_version must be the number 1`);
  }
  if (record.ds_scale !== 2) {
    throw new TypeError(`${context}.ds_scale must be the number 2`);
  }
  const networkId = nonEmptyString(record.network_id, `${context}.network_id`);
  NetworkId.parse(networkId);
  const policyVersion = uint64String(record.policy_version, `${context}.policy_version`, {
    allowZero: false,
  });
  const previousPolicyHash = record.previous_policy_hash === null
    ? null
    : byteArray(record.previous_policy_hash, 32, `${context}.previous_policy_hash`);
  if ((policyVersion === "1") !== (previousPolicyHash === null)) {
    throw new TypeError(`${context}.previous_policy_hash does not match policy_version`);
  }
  const chargingMode = normalizeValidationFeeChargingMode(
    record.charging_mode,
    `${context}.charging_mode`,
  );
  const fee = canonicalQuantity(record.fee, `${context}.fee`);
  const exemptions = array(record.exemption_classes, `${context}.exemption_classes`)
    .map((item, index) => nonEmptyString(
      item,
      `${context}.exemption_classes[${index}]`,
    ));
  if (
    exemptions.some((item) => item !== "TREASURY_PAYOUT") ||
    new Set(exemptions).size !== exemptions.length
  ) {
    throw new TypeError(`${context}.exemption_classes contains an unsupported or duplicate class`);
  }
  const payoutBinding = record.treasury_payout_binding === null
    ? null
    : normalizeValidationFeePayoutBinding(
      record.treasury_payout_binding,
      `${context}.treasury_payout_binding`,
    );
  if ((payoutBinding === null) !== !exemptions.includes("TREASURY_PAYOUT")) {
    throw new TypeError(`${context}.treasury_payout_binding does not match exemption_classes`);
  }
  if (chargingMode.charging_mode === "DISABLED") {
    if (fee !== "0" || exemptions.length !== 0 || payoutBinding !== null) {
      throw new TypeError(`${context} disabled charging mode requires zero fee and no exemptions`);
    }
  } else if (fee !== "0.1") {
    throw new TypeError(`${context}.fee must be exactly 0.1 for enabled V1 charging`);
  }
  const effectiveHeight = uint64String(
    record.effective_from_height,
    `${context}.effective_from_height`,
  );
  const expiresHeight = record.expires_after_height === null
    ? null
    : uint64String(record.expires_after_height, `${context}.expires_after_height`);
  if (expiresHeight !== null && BigInt(expiresHeight) <= BigInt(effectiveHeight)) {
    throw new TypeError(`${context}.expires_after_height must exceed effective_from_height`);
  }
  return {
    schema_version: 1,
    network_id: networkId,
    policy_version: policyVersion,
    previous_policy_hash: previousPolicyHash,
    ds_asset_id: canonicalAssetDefinitionId(record.ds_asset_id, `${context}.ds_asset_id`),
    ds_scale: 2,
    fee,
    treasury_account_id: canonicalAccountId(
      record.treasury_account_id,
      `${context}.treasury_account_id`,
    ),
    charging_mode: chargingMode,
    effective_from_height: effectiveHeight,
    expires_after_height: expiresHeight,
    exemption_classes: exemptions,
    treasury_payout_binding: payoutBinding,
  };
}

function normalizeValidationFeeChargingMode(value, context) {
  const record = exactRecord(value, ["charging_mode", "value"], context);
  if (
    record.charging_mode !== "DISABLED" &&
    record.charging_mode !== "PER_QUALIFYING_TRANSFER_INSTRUCTION"
  ) {
    throw new TypeError(`${context}.charging_mode contains an unsupported variant`);
  }
  if (record.value !== null) {
    throw new TypeError(`${context}.value must be null`);
  }
  return { charging_mode: record.charging_mode, value: null };
}

function normalizeValidationFeePayoutLifecycle(value, context) {
  const record = exactRecord(value, ["proposal_operator", "payout_binding"], context);
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

function normalizeValidationFeePayoutBinding(value, context) {
  const record = exactRecord(value, [
    "contract_address",
    "code_hash",
    "entrypoint",
    "treasury_account_id",
    "ds_asset_id",
    "xor_asset_id",
    "pool_vault_account_id",
    "batch_ds",
    "min_xor_out",
    "max_xor_out",
    "recipients",
  ], context);
  const contractAddress = nonEmptyString(record.contract_address, `${context}.contract_address`);
  parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
  if (record.entrypoint !== "autonomous_validation_fee_tick") {
    throw new TypeError(`${context}.entrypoint must be autonomous_validation_fee_tick`);
  }
  const treasury = canonicalAccountId(
    record.treasury_account_id,
    `${context}.treasury_account_id`,
  );
  const vault = canonicalAccountId(
    record.pool_vault_account_id,
    `${context}.pool_vault_account_id`,
  );
  if (treasury === vault) {
    throw new TypeError(`${context} treasury and pool vault accounts must differ`);
  }
  const dsAsset = canonicalAssetDefinitionId(record.ds_asset_id, `${context}.ds_asset_id`);
  const xorAsset = canonicalAssetDefinitionId(record.xor_asset_id, `${context}.xor_asset_id`);
  if (dsAsset === xorAsset) {
    throw new TypeError(`${context} DS and XOR assets must differ`);
  }
  if (
    canonicalQuantity(record.batch_ds, `${context}.batch_ds`) !== "10" ||
    canonicalQuantity(record.min_xor_out, `${context}.min_xor_out`) !== "4" ||
    canonicalQuantity(record.max_xor_out, `${context}.max_xor_out`) !== "100"
  ) {
    throw new TypeError(`${context} must use the exact V1 payout quantities`);
  }
  const recipients = array(record.recipients, `${context}.recipients`)
    .map((item, index) => {
      const itemContext = `${context}.recipients[${index}]`;
      const recipient = exactRecord(item, ["account_id", "share"], itemContext);
      const accountId = canonicalAccountId(recipient.account_id, `${itemContext}.account_id`);
      if (canonicalQuantity(recipient.share, `${itemContext}.share`) !== "0.25") {
        throw new TypeError(`${itemContext}.share must be exactly 0.25`);
      }
      return { account_id: accountId, share: "0.25" };
    });
  const recipientIds = recipients.map((recipient) => recipient.account_id);
  if (
    recipients.length !== 4 ||
    new Set(recipientIds).size !== 4 ||
    recipientIds.includes(treasury) ||
    recipientIds.includes(vault)
  ) {
    throw new TypeError(`${context}.recipients must contain four unique non-pool accounts`);
  }
  return {
    contract_address: contractAddress,
    code_hash: byteArray(record.code_hash, 32, `${context}.code_hash`, { nonZero: true }),
    entrypoint: "autonomous_validation_fee_tick",
    treasury_account_id: treasury,
    ds_asset_id: dsAsset,
    xor_asset_id: xorAsset,
    pool_vault_account_id: vault,
    batch_ds: "10",
    min_xor_out: "4",
    max_xor_out: "100",
    recipients,
  };
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
    ["contract_address", "expected_revision", "action"],
    context,
  );
  const contractAddress = nonEmptyString(record.contract_address, `${context}.contract_address`);
  parseCanonicalContractAddress(contractAddress, `${context}.contract_address`);
  return {
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
