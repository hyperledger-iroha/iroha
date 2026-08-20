import { Buffer } from "node:buffer";

import { getNativeBinding } from "./native.js";
import { networkIdBytes } from "./networkId.js";

export const VALIDATION_FEE_LEDGER_BINDING_SCHEMA =
  "cbsi.mobile-validation-fee-ledger-binding.v1";
export const VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA =
  "iroha.validation_fee.verified_policy_projection.v1";
export const VALIDATION_FEE_CURRENT_POLICY_PROOF_PATH =
  "/v1/validation-fee/policy/current/proof";
export const VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES = 4 * 1024 * 1024;
export const VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION = 22;

const LOWER_HEX_32 = /^[0-9a-f]{64}$/u;
const BINDING_KEYS = Object.freeze([
  "checkpoint",
  "networkId",
  "policyChainGenesisHash",
  "schema",
]);
const CHECKPOINT_KEYS = Object.freeze(["contextId", "height"]);
const PROJECTION_KEYS = Object.freeze([
  "current_policy",
  "evaluated_block_hash",
  "evaluated_block_height",
  "evaluated_context_id",
  "head_policy_hash",
  "head_policy_version",
  "more_available",
  "network_id",
  "observed_ledger_tip_height",
  "policy_chain_genesis_hash",
  "registry_hash",
  "schema",
  "trusted_checkpoint_context_id",
  "trusted_checkpoint_height",
  "version",
]);
const CURRENT_POLICY_KEYS = Object.freeze([
  "activePolicyHash",
  "activePolicyVersion",
  "chargingMode",
  "effectiveFromHeight",
  "expiresAfterHeight",
  "feeAssetDefinitionId",
  "feeMinorUnits",
  "feeScale",
  "parliament",
  "payout",
]);
const PARLIAMENT_KEYS = Object.freeze([
  "payoutLifecycle",
  "payoutLifecycleSealHash",
  "validationFeePolicy",
]);
const PARLIAMENT_PROPOSAL_KEYS = Object.freeze([
  "enactment_window",
  "finalization",
  "parliament_roster_root",
  "payload_hash",
  "plainElectorateRules",
  "plainElectorateSnapshot",
  "proposal_id",
  "proposal_kind",
]);
const PLAIN_ELECTORATE_RULE_KEYS = Object.freeze([
  "approval_threshold_denominator",
  "approval_threshold_numerator",
  "ballot_amount",
  "ballot_duration_blocks",
  "bond_escrow_account",
  "citizenship_amount",
  "conviction_step_blocks",
  "eligibility_rule",
  "max_conviction",
  "max_members",
  "min_turnout",
  "slash_receiver_account",
  "voting_asset_id",
]);
const PLAIN_ELIGIBILITY_RULE_KEYS = Object.freeze(["rule", "value"]);
const PLAIN_ELECTORATE_SNAPSHOT_KEYS = Object.freeze([
  "approvalGateHeight",
  "capturedAtHeight",
  "memberCount",
  "rosterRoot",
]);
const ENACTMENT_WINDOW_KEYS = Object.freeze([
  "closes_at_height",
  "enacted_at_height",
  "opens_at_height",
]);
const FINALIZATION_KEYS = Object.freeze([
  "abstain",
  "approval_threshold_denominator",
  "approval_threshold_numerator",
  "approve",
  "approved",
  "finalized_at_height",
  "min_turnout",
  "mode",
  "proposal_id",
  "referendum_id",
  "reject",
]);
const PAYOUT_KEYS = Object.freeze([
  "batchDsMinorUnits",
  "codeHash",
  "contractAddress",
  "dsAssetDefinitionId",
  "dsScale",
  "entrypoint",
  "recipients",
  "treasuryAccountId",
  "vaultAccountId",
  "xorAssetDefinitionId",
  "xorOutputMax",
  "xorOutputMin",
]);
const PAYOUT_RECIPIENT_KEYS = Object.freeze([
  "account_id",
  "share_basis_points",
]);
const CANONICAL_UNSIGNED_DECIMAL = /^(?:0|[1-9][0-9]*)$/u;
const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_U128 = (1n << 128n) - 1n;
const VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS = 120_960n;
const VALIDATION_FEE_PLAIN_BALLOT_AMOUNT = 150n;
const VALIDATION_FEE_PLAIN_BALLOT_DURATION_BLOCKS = 3_600n;
const VALIDATION_FEE_PLAIN_MAX_MEMBERS = 256n;
const VALIDATION_FEE_PAYOUT_RECIPIENT_COUNT = 4;
const VALIDATION_FEE_PAYOUT_RECIPIENT_SHARE_BASIS_POINTS = 2_500;

function record(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    throw new TypeError(`${label} must be a plain object`);
  }
  return value;
}

function exactKeys(value, expected, label) {
  const keys = Object.keys(value).sort();
  if (
    keys.length !== expected.length ||
    keys.some((key, index) => key !== expected[index])
  ) {
    throw new TypeError(`${label} must contain exactly ${expected.join(", ")}`);
  }
}

function lowerHex32(value, label) {
  if (typeof value !== "string" || !LOWER_HEX_32.test(value)) {
    throw new TypeError(`${label} must be exactly 64 lowercase hexadecimal digits`);
  }
  if (/^0+$/u.test(value)) {
    throw new TypeError(`${label} must be non-zero`);
  }
  return value;
}

function irohaHash32(value, label) {
  const normalized = lowerHex32(value, label);
  if ((Number.parseInt(normalized.slice(-2), 16) & 1) === 0) {
    throw new TypeError(`${label} must carry the canonical Iroha hash marker`);
  }
  return normalized;
}

function positiveU64(value, label) {
  let parsed;
  if (typeof value === "bigint") {
    parsed = value;
  } else if (typeof value === "number" && Number.isSafeInteger(value)) {
    parsed = BigInt(value);
  } else if (typeof value === "string" && /^[1-9][0-9]*$/u.test(value)) {
    parsed = BigInt(value);
  } else {
    throw new TypeError(`${label} must be a positive uint64`);
  }
  if (parsed <= 0n || parsed > 0xffff_ffff_ffff_ffffn) {
    throw new TypeError(`${label} must be a positive uint64`);
  }
  return parsed;
}

function canonicalText(value, label) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 4_096 ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new TypeError(`${label} must be canonical bounded text`);
  }
  return value;
}

function unsignedDecimalString(value, maximum, label, positive = false) {
  if (
    typeof value !== "string" ||
    value.length > maximum.toString().length ||
    !CANONICAL_UNSIGNED_DECIMAL.test(value)
  ) {
    throw new TypeError(`${label} must be a canonical unsigned decimal string`);
  }
  const parsed = BigInt(value);
  if (parsed > maximum || (positive && parsed === 0n)) {
    throw new TypeError(
      `${label} must be a ${positive ? "positive" : "non-negative"} bounded integer`,
    );
  }
  return parsed;
}

function u64String(value, label, positive = false) {
  return unsignedDecimalString(value, MAX_U64, label, positive);
}

function u128String(value, label, positive = false) {
  return unsignedDecimalString(value, MAX_U128, label, positive);
}

function unsignedInteger(value, maximum, label) {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > maximum
  ) {
    throw new TypeError(`${label} must be an unsigned integer no greater than ${maximum}`);
  }
  return value;
}

function requireEqual(left, right, label) {
  if (left !== right) {
    throw new TypeError(`${label} must match its verified proposal evidence`);
  }
}

function validatePlainElectorateRules(value, label) {
  const rules = record(value, label);
  exactKeys(rules, PLAIN_ELECTORATE_RULE_KEYS, label);
  canonicalText(rules.voting_asset_id, `${label}.voting_asset_id`);
  canonicalText(rules.bond_escrow_account, `${label}.bond_escrow_account`);
  canonicalText(rules.slash_receiver_account, `${label}.slash_receiver_account`);
  const ballotAmount = u128String(rules.ballot_amount, `${label}.ballot_amount`, true);
  const ballotDuration = u64String(
    rules.ballot_duration_blocks,
    `${label}.ballot_duration_blocks`,
    true,
  );
  const citizenshipAmount = u128String(
    rules.citizenship_amount,
    `${label}.citizenship_amount`,
    true,
  );
  const maxMembers = u64String(rules.max_members, `${label}.max_members`, true);
  const convictionStep = u64String(
    rules.conviction_step_blocks,
    `${label}.conviction_step_blocks`,
    true,
  );
  const maxConviction = u64String(
    rules.max_conviction,
    `${label}.max_conviction`,
    true,
  );
  const minTurnout = u128String(rules.min_turnout, `${label}.min_turnout`, true);
  const approvalNumerator = u64String(
    rules.approval_threshold_numerator,
    `${label}.approval_threshold_numerator`,
    true,
  );
  const approvalDenominator = u64String(
    rules.approval_threshold_denominator,
    `${label}.approval_threshold_denominator`,
    true,
  );
  if (
    ballotAmount !== VALIDATION_FEE_PLAIN_BALLOT_AMOUNT ||
    ballotDuration !== VALIDATION_FEE_PLAIN_BALLOT_DURATION_BLOCKS ||
    maxMembers > VALIDATION_FEE_PLAIN_MAX_MEMBERS ||
    approvalNumerator > approvalDenominator
  ) {
    throw new TypeError(`${label} violates the bounded PLAIN electorate invariants`);
  }
  const eligibility = record(rules.eligibility_rule, `${label}.eligibility_rule`);
  exactKeys(
    eligibility,
    PLAIN_ELIGIBILITY_RULE_KEYS,
    `${label}.eligibility_rule`,
  );
  if (
    eligibility.rule !==
      "proposal_operator_at_or_before_gate_others_after_gate" ||
    eligibility.value !== null
  ) {
    throw new TypeError(`${label}.eligibility_rule is not the closed V1 rule`);
  }
  return {
    approvalDenominator,
    approvalNumerator,
    ballotAmount,
    ballotDuration,
    citizenshipAmount,
    convictionStep,
    maxConviction,
    maxMembers,
    minTurnout,
    rules,
  };
}

function validatePlainElectorateSnapshot(value, label) {
  const snapshot = record(value, label);
  exactKeys(snapshot, PLAIN_ELECTORATE_SNAPSHOT_KEYS, label);
  lowerHex32(snapshot.rosterRoot, `${label}.rosterRoot`);
  return {
    approvalGateHeight: u64String(
      snapshot.approvalGateHeight,
      `${label}.approvalGateHeight`,
    ),
    capturedAtHeight: u64String(
      snapshot.capturedAtHeight,
      `${label}.capturedAtHeight`,
      true,
    ),
    memberCount: u64String(snapshot.memberCount, `${label}.memberCount`, true),
  };
}

function validateEnactmentWindow(value, label) {
  const window = record(value, label);
  exactKeys(window, ENACTMENT_WINDOW_KEYS, label);
  return {
    closesAtHeight: u64String(
      window.closes_at_height,
      `${label}.closes_at_height`,
      true,
    ),
    enactedAtHeight: u64String(
      window.enacted_at_height,
      `${label}.enacted_at_height`,
      true,
    ),
    opensAtHeight: u64String(
      window.opens_at_height,
      `${label}.opens_at_height`,
      true,
    ),
  };
}

function validateFinalization(value, proposalId, rules, label) {
  const finalization = record(value, label);
  exactKeys(finalization, FINALIZATION_KEYS, label);
  const finalizationProposalId = lowerHex32(
    finalization.proposal_id,
    `${label}.proposal_id`,
  );
  const referendumId = lowerHex32(
    finalization.referendum_id,
    `${label}.referendum_id`,
  );
  requireEqual(finalizationProposalId, proposalId, `${label}.proposal_id`);
  requireEqual(referendumId, proposalId, `${label}.referendum_id`);
  const finalizedAtHeight = u64String(
    finalization.finalized_at_height,
    `${label}.finalized_at_height`,
    true,
  );
  if (finalization.mode !== "PLAIN") {
    throw new TypeError(`${label}.mode must be PLAIN`);
  }
  const approve = u128String(finalization.approve, `${label}.approve`);
  const reject = u128String(finalization.reject, `${label}.reject`);
  const abstain = u128String(finalization.abstain, `${label}.abstain`);
  const minTurnout = u128String(
    finalization.min_turnout,
    `${label}.min_turnout`,
    true,
  );
  const approvalNumerator = u64String(
    finalization.approval_threshold_numerator,
    `${label}.approval_threshold_numerator`,
    true,
  );
  const approvalDenominator = u64String(
    finalization.approval_threshold_denominator,
    `${label}.approval_threshold_denominator`,
    true,
  );
  requireEqual(minTurnout, rules.minTurnout, `${label}.min_turnout`);
  requireEqual(
    approvalNumerator,
    rules.approvalNumerator,
    `${label}.approval_threshold_numerator`,
  );
  requireEqual(
    approvalDenominator,
    rules.approvalDenominator,
    `${label}.approval_threshold_denominator`,
  );
  if (
    typeof finalization.approved !== "boolean" ||
    !finalization.approved ||
    approvalNumerator > approvalDenominator
  ) {
    throw new TypeError(`${label} must contain an approved PLAIN decision`);
  }
  const turnout = approve + reject + abstain;
  const weightedApprove = approve * approvalDenominator;
  const requiredApprove = turnout * approvalNumerator;
  if (
    turnout > MAX_U128 ||
    weightedApprove > MAX_U128 ||
    requiredApprove > MAX_U128 ||
    turnout < minTurnout ||
    weightedApprove < requiredApprove
  ) {
    throw new TypeError(`${label} does not recompute to an approved PLAIN decision`);
  }
  return { finalizedAtHeight };
}

function validateParliamentProposal(value, expectedKind, label) {
  const proposal = record(value, label);
  exactKeys(proposal, PARLIAMENT_PROPOSAL_KEYS, label);
  if (proposal.proposal_kind !== expectedKind) {
    throw new TypeError(`${label}.proposal_kind must be ${expectedKind}`);
  }
  const proposalId = lowerHex32(proposal.proposal_id, `${label}.proposal_id`);
  const payloadHash = lowerHex32(proposal.payload_hash, `${label}.payload_hash`);
  requireEqual(payloadHash, proposalId, `${label}.payload_hash`);
  lowerHex32(proposal.parliament_roster_root, `${label}.parliament_roster_root`);
  const rules = validatePlainElectorateRules(
    proposal.plainElectorateRules,
    `${label}.plainElectorateRules`,
  );
  const snapshot = validatePlainElectorateSnapshot(
    proposal.plainElectorateSnapshot,
    `${label}.plainElectorateSnapshot`,
  );
  const window = validateEnactmentWindow(
    proposal.enactment_window,
    `${label}.enactment_window`,
  );
  const finalization = validateFinalization(
    proposal.finalization,
    proposalId,
    rules,
    `${label}.finalization`,
  );
  if (
    snapshot.memberCount > rules.maxMembers ||
    snapshot.capturedAtHeight !== window.opensAtHeight ||
    snapshot.approvalGateHeight >= snapshot.capturedAtHeight ||
    window.closesAtHeight < window.opensAtHeight ||
    window.closesAtHeight - window.opensAtHeight + 1n !== rules.ballotDuration ||
    finalization.finalizedAtHeight !== window.closesAtHeight ||
    window.enactedAtHeight <= finalization.finalizedAtHeight
  ) {
    throw new TypeError(`${label} violates its frozen electorate or enactment anchors`);
  }
  return { proposalId, rules, window };
}

function equalPlainElectorateRules(left, right) {
  return PLAIN_ELECTORATE_RULE_KEYS.every((key) => {
    if (key !== "eligibility_rule") return left[key] === right[key];
    return (
      left.eligibility_rule.rule === right.eligibility_rule.rule &&
      left.eligibility_rule.value === right.eligibility_rule.value
    );
  });
}

function validateParliament(value, label) {
  const parliament = record(value, label);
  exactKeys(parliament, PARLIAMENT_KEYS, label);
  const policy = validateParliamentProposal(
    parliament.validationFeePolicy,
    "ValidationFeePolicyV1",
    `${label}.validationFeePolicy`,
  );
  const payout = validateParliamentProposal(
    parliament.payoutLifecycle,
    "ValidationFeePayoutLifecycleV1",
    `${label}.payoutLifecycle`,
  );
  irohaHash32(
    parliament.payoutLifecycleSealHash,
    `${label}.payoutLifecycleSealHash`,
  );
  if (
    policy.proposalId === payout.proposalId ||
    !equalPlainElectorateRules(policy.rules.rules, payout.rules.rules)
  ) {
    throw new TypeError(
      `${label} must bind distinct proposals to identical PLAIN electorate rules`,
    );
  }
  return { payout, policy };
}

function validatePayout(value, label) {
  const payout = record(value, label);
  exactKeys(payout, PAYOUT_KEYS, label);
  canonicalText(payout.contractAddress, `${label}.contractAddress`);
  lowerHex32(payout.codeHash, `${label}.codeHash`);
  if (payout.entrypoint !== "autonomous_validation_fee_tick") {
    throw new TypeError(
      `${label}.entrypoint must be autonomous_validation_fee_tick`,
    );
  }
  canonicalText(payout.dsAssetDefinitionId, `${label}.dsAssetDefinitionId`);
  canonicalText(payout.xorAssetDefinitionId, `${label}.xorAssetDefinitionId`);
  canonicalText(payout.treasuryAccountId, `${label}.treasuryAccountId`);
  canonicalText(payout.vaultAccountId, `${label}.vaultAccountId`);
  const batchDsMinorUnits = u128String(
    payout.batchDsMinorUnits,
    `${label}.batchDsMinorUnits`,
    true,
  );
  const dsScale = unsignedInteger(payout.dsScale, 255, `${label}.dsScale`);
  const xorOutputMin = u128String(
    payout.xorOutputMin,
    `${label}.xorOutputMin`,
    true,
  );
  const xorOutputMax = u128String(
    payout.xorOutputMax,
    `${label}.xorOutputMax`,
    true,
  );
  if (
    payout.dsAssetDefinitionId === payout.xorAssetDefinitionId ||
    payout.treasuryAccountId === payout.vaultAccountId ||
    batchDsMinorUnits !== 1_000n ||
    dsScale !== 2 ||
    xorOutputMin !== 4n ||
    xorOutputMax !== 100n
  ) {
    throw new TypeError(`${label} violates the immutable first-release payout binding`);
  }
  if (
    !Array.isArray(payout.recipients) ||
    payout.recipients.length !== VALIDATION_FEE_PAYOUT_RECIPIENT_COUNT
  ) {
    throw new TypeError(
      `${label}.recipients must contain exactly ${VALIDATION_FEE_PAYOUT_RECIPIENT_COUNT} entries`,
    );
  }
  const recipients = new Set();
  for (const [index, recipientValue] of payout.recipients.entries()) {
    const recipientLabel = `${label}.recipients[${index}]`;
    const recipient = record(recipientValue, recipientLabel);
    exactKeys(recipient, PAYOUT_RECIPIENT_KEYS, recipientLabel);
    const account = canonicalText(recipient.account_id, `${recipientLabel}.account_id`);
    const share = unsignedInteger(
      recipient.share_basis_points,
      0xffff,
      `${recipientLabel}.share_basis_points`,
    );
    if (
      share !== VALIDATION_FEE_PAYOUT_RECIPIENT_SHARE_BASIS_POINTS ||
      account === payout.treasuryAccountId ||
      account === payout.vaultAccountId ||
      recipients.has(account)
    ) {
      throw new TypeError(`${recipientLabel} violates the exact payout recipient plan`);
    }
    recipients.add(account);
  }
  return { dsScale };
}

function validateCurrentPolicy(value, label) {
  if (value === null) return;
  const policy = record(value, label);
  exactKeys(policy, CURRENT_POLICY_KEYS, label);
  u64String(policy.activePolicyVersion, `${label}.activePolicyVersion`, true);
  irohaHash32(policy.activePolicyHash, `${label}.activePolicyHash`);
  canonicalText(policy.feeAssetDefinitionId, `${label}.feeAssetDefinitionId`);
  const feeScale = unsignedInteger(policy.feeScale, 255, `${label}.feeScale`);
  const feeMinorUnits = u128String(
    policy.feeMinorUnits,
    `${label}.feeMinorUnits`,
    true,
  );
  if (
    feeScale !== 2 ||
    feeMinorUnits !== 10n ||
    policy.chargingMode !== "PER_QUALIFYING_TRANSFER_INSTRUCTION"
  ) {
    throw new TypeError(`${label} is not an enabled first-release policy`);
  }
  const effectiveFromHeight = u64String(
    policy.effectiveFromHeight,
    `${label}.effectiveFromHeight`,
    true,
  );
  if (policy.expiresAfterHeight !== null) {
    const expiresAfterHeight = u64String(
      policy.expiresAfterHeight,
      `${label}.expiresAfterHeight`,
      true,
    );
    if (expiresAfterHeight <= effectiveFromHeight) {
      throw new TypeError(`${label}.expiresAfterHeight must follow activation`);
    }
  }
  const parliament = validateParliament(policy.parliament, `${label}.parliament`);
  const payout = validatePayout(policy.payout, `${label}.payout`);
  if (
    policy.feeAssetDefinitionId !== policy.payout.dsAssetDefinitionId ||
    feeScale !== payout.dsScale ||
    parliament.policy.rules.rules.voting_asset_id !==
      policy.payout.xorAssetDefinitionId ||
    parliament.payout.rules.rules.voting_asset_id !==
      policy.payout.xorAssetDefinitionId ||
    parliament.policy.window.enactedAtHeight +
      VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS !==
      effectiveFromHeight
  ) {
    throw new TypeError(`${label} differs from its Parliament or payout binding`);
  }
}

/** Validate the exact immutable CBSI deployment binding. */
export function normalizeValidationFeeLedgerBindingV1(value) {
  const binding = record(value, "validation-fee ledger binding");
  exactKeys(binding, BINDING_KEYS, "validation-fee ledger binding");
  if (binding.schema !== VALIDATION_FEE_LEDGER_BINDING_SCHEMA) {
    throw new TypeError(
      `validation-fee ledger binding.schema must be ${VALIDATION_FEE_LEDGER_BINDING_SCHEMA}`,
    );
  }
  const checkpoint = normalizeValidationFeeCheckpointV1(binding.checkpoint);
  networkIdBytes(binding.networkId, "validation-fee ledger binding.networkId");
  return Object.freeze({
    schema: binding.schema,
    networkId: binding.networkId,
    policyChainGenesisHash: irohaHash32(
      binding.policyChainGenesisHash,
      "validation-fee ledger binding.policyChainGenesisHash",
    ),
    checkpoint,
  });
}

/** Normalize one durable checkpoint used for page promotion. */
export function normalizeValidationFeeCheckpointV1(value) {
  const checkpoint = record(value, "validation-fee checkpoint");
  exactKeys(checkpoint, CHECKPOINT_KEYS, "validation-fee checkpoint");
  return Object.freeze({
    height: positiveU64(checkpoint.height, "validation-fee checkpoint.height"),
    contextId: irohaHash32(
      checkpoint.contextId,
      "validation-fee checkpoint.contextId",
    ),
  });
}

function nativeBinding() {
  const native = globalThis.__IROHA_NATIVE_BINDING__ ?? getNativeBinding();
  if (
    typeof native?.connectNoritoBridgeAbiVersion !== "function" ||
    native.connectNoritoBridgeAbiVersion() !==
      VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION ||
    typeof native?.validationFeeCurrentPolicyProofRequestV1 !== "function" ||
    typeof native?.validationFeeVerifyCurrentPolicyProofV1 !== "function"
  ) {
    throw new Error(
      `native binding lacks the ABI ${VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION} validation-fee consensus proof verifier`,
    );
  }
  return native;
}

/** Encode the exact Norito V1 proof request for `checkpoint`. */
export function encodeValidationFeeCurrentPolicyProofRequestV1(checkpoint) {
  const normalized = normalizeValidationFeeCheckpointV1(checkpoint);
  const encoded = nativeBinding().validationFeeCurrentPolicyProofRequestV1(
    normalized.height,
    Buffer.from(normalized.contextId, "hex"),
  );
  if (!encoded || encoded.length === 0) {
    throw new Error("native validation-fee request encoder returned no bytes");
  }
  return Buffer.from(encoded);
}

function projectionHeight(value, label) {
  return positiveU64(value, label);
}

function freezeProjection(value) {
  const stack = [value];
  let visited = 0;
  while (stack.length > 0) {
    const next = stack.pop();
    if (next === null || typeof next !== "object" || Object.isFrozen(next)) continue;
    visited += 1;
    if (visited > 100_000) {
      throw new TypeError("validation-fee projection exceeds the object bound");
    }
    for (const child of Object.values(next)) stack.push(child);
    Object.freeze(next);
  }
  return value;
}

/**
 * Locally verify one canonical Norito proof page and return its immutable
 * policy projection. The native verifier performs all consensus cryptography.
 */
export function verifyValidationFeeCurrentPolicyProofV1(
  proofNorito,
  bindingValue,
  checkpointValue,
) {
  const binding = normalizeValidationFeeLedgerBindingV1(bindingValue);
  const checkpoint = normalizeValidationFeeCheckpointV1(checkpointValue);
  const proof = Buffer.from(proofNorito ?? []);
  if (
    proof.length === 0 ||
    proof.length > VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES
  ) {
    throw new TypeError(
      `proofNorito must contain 1..${VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES} bytes`,
    );
  }
  const json = nativeBinding().validationFeeVerifyCurrentPolicyProofV1(
    proof,
    Buffer.from(networkIdBytes(binding.networkId, "validation-fee ledger binding.networkId")),
    Buffer.from(binding.policyChainGenesisHash, "hex"),
    checkpoint.height,
    Buffer.from(checkpoint.contextId, "hex"),
  );
  if (typeof json !== "string" || json.length === 0) {
    throw new Error("native validation-fee verifier returned no projection");
  }
  const projection = record(JSON.parse(json), "validation-fee verified projection");
  exactKeys(
    projection,
    PROJECTION_KEYS,
    "validation-fee verified projection",
  );
  const projectedTrustedCheckpointHeight = projectionHeight(
    projection.trusted_checkpoint_height,
    "validation-fee projection.trusted_checkpoint_height",
  );
  if (
    projection.schema !== VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA ||
    projection.version !== 1 ||
    projection.network_id !== binding.networkId.toString() ||
    projection.policy_chain_genesis_hash !== binding.policyChainGenesisHash ||
    projection.trusted_checkpoint_context_id !== checkpoint.contextId ||
    projectedTrustedCheckpointHeight !== checkpoint.height
  ) {
    throw new TypeError(
      "validation-fee verified projection differs from its immutable binding or checkpoint",
    );
  }
  const normalized = {
    ...projection,
    head_policy_version: projectionHeight(
      projection.head_policy_version,
      "validation-fee projection.head_policy_version",
    ),
    trusted_checkpoint_height: projectedTrustedCheckpointHeight,
    evaluated_block_height: projectionHeight(
      projection.evaluated_block_height,
      "validation-fee projection.evaluated_block_height",
    ),
    observed_ledger_tip_height: projectionHeight(
      projection.observed_ledger_tip_height,
      "validation-fee projection.observed_ledger_tip_height",
    ),
  };
  irohaHash32(
    normalized.evaluated_context_id,
    "validation-fee projection.evaluated_context_id",
  );
  irohaHash32(
    normalized.evaluated_block_hash,
    "validation-fee projection.evaluated_block_hash",
  );
  irohaHash32(
    normalized.registry_hash,
    "validation-fee projection.registry_hash",
  );
  irohaHash32(
    normalized.head_policy_hash,
    "validation-fee projection.head_policy_hash",
  );
  if (typeof normalized.more_available !== "boolean") {
    throw new TypeError(
      "validation-fee projection.more_available must be boolean",
    );
  }
  validateCurrentPolicy(
    normalized.current_policy,
    "validation-fee projection.current_policy",
  );
  return freezeProjection(normalized);
}
