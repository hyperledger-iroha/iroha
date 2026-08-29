import { Buffer } from "buffer";
import { sha256 } from "@noble/hashes/sha2";

import { assertCanonicalBls12381G1Compressed } from "./bls12381G1.js";
import { blake2b256 } from "./blake2b.js";
import { crc64Xz } from "./crc64Xz.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { NumericV1, NumericV1Error } from "./numericV1.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";
import {
  createValidationError,
  ValidationErrorCode,
} from "./validationError.js";
import {
  SUMERAGI_DIAGNOSTICS_TYPED_JSON_MAX_BYTES,
  SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES,
} from "./sumeragiTypedLimits.js";

export {
  SUMERAGI_DIAGNOSTICS_TYPED_JSON_MAX_BYTES,
  SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES,
};

function parseSumeragiNexusFeeSchedule(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    [
      "tx_bytes_len",
      "instruction_count",
      "gas_used",
      "base_fee",
      "per_byte_fee",
      "per_instruction_fee",
      "per_gas_unit_fee",
    ],
    context,
  );
  return Object.freeze({
    tx_bytes_len: parseSumeragiUnsigned(record.tx_bytes_len, `${context}.tx_bytes_len`),
    instruction_count: parseSumeragiUnsigned(
      record.instruction_count,
      `${context}.instruction_count`,
    ),
    gas_used: parseSumeragiUnsigned(record.gas_used, `${context}.gas_used`),
    base_fee: parseSumeragiQuantity(record.base_fee, `${context}.base_fee`),
    per_byte_fee: parseSumeragiQuantity(record.per_byte_fee, `${context}.per_byte_fee`),
    per_instruction_fee: parseSumeragiQuantity(
      record.per_instruction_fee,
      `${context}.per_instruction_fee`,
    ),
    per_gas_unit_fee: parseSumeragiQuantity(
      record.per_gas_unit_fee,
      `${context}.per_gas_unit_fee`,
    ),
  });
}

function parseSumeragiNexusFeeReceipt(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    [
      "version",
      "source_id",
      "dataspace_id",
      "lane_id",
      "block_height",
      "payer_account_id",
      "fee_asset_id",
      "fee_amount",
      "schedule",
    ],
    context,
  );
  const version = parseSumeragiUnsigned(record.version, `${context}.version`, { max: 0xffff });
  if (version !== 1) {
    throw new RangeError(`${context}.version must equal 1`);
  }
  return Object.freeze({
    version,
    source_id: parseSumeragiByte32(record.source_id, `${context}.source_id`),
    dataspace_id: parseSumeragiUnsigned(record.dataspace_id, `${context}.dataspace_id`),
    lane_id: parseSumeragiUnsigned(record.lane_id, `${context}.lane_id`, {
      max: 0xffffffff,
    }),
    block_height: parseSumeragiUnsigned(record.block_height, `${context}.block_height`),
    payer_account_id: requireExactNonEmptyString(
      record.payer_account_id,
      `${context}.payer_account_id`,
    ),
    fee_asset_id: requireExactNonEmptyString(record.fee_asset_id, `${context}.fee_asset_id`),
    fee_amount: parseSumeragiQuantity(record.fee_amount, `${context}.fee_amount`),
    schedule: parseSumeragiNexusFeeSchedule(record.schedule, `${context}.schedule`),
  });
}

const MAX_NATIVE_AMX_PARTICIPANT_SETTLEMENT_RECEIPTS = 4096;

function parseSumeragiNativeAmxBody(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    [
      "round",
      "epoch",
      "network_id",
      "source_id",
      "tx_entrypoint_hash",
      "plan_digest",
      "phase",
      "coordinator_lane_id",
      "coordinator_dataspace_id",
      "coordinator_lane_incarnation",
      "participant_lane_id",
      "participant_dataspace_id",
      "participant_lane_incarnation",
      "participant_previous_block_height",
      "participant_previous_block_descriptor_hash",
      "participant_lane_block_height",
      "participant_lane_block_view",
      "participant_proposal_hash",
      "participant_settlement_commitment",
      "participant_validator_set_hash",
      "participant_validator_count",
      "participant_min_quorum",
      "authority_context_height",
      "planned_coordinator_block_height",
      "coordinator_lane_block_view",
      "coordinator_proposal_hash",
    ],
    context,
  );
  assertExactSumeragiRecord(record.round, ["context_id", "height", "view"], `${context}.round`);
  const round = parseSumeragiRound(record.round, `${context}.round`);
  const phase = parseSumeragiTaggedUnitWithContent(
    record.phase,
    "phase",
    "detail",
    ["prepare", "commit"],
    `${context}.phase`,
  );
  const validatorCount = parseSumeragiUnsigned(
    record.participant_validator_count,
    `${context}.participant_validator_count`,
    { positive: true, max: 128 },
  );
  const minQuorum = parseSumeragiUnsigned(
    record.participant_min_quorum,
    `${context}.participant_min_quorum`,
    { positive: true, max: 128 },
  );
  const expectedQuorum = validatorCount - Math.floor((validatorCount - 1) / 3);
  const authorityHeight = parseSumeragiUnsigned(
    record.authority_context_height,
    `${context}.authority_context_height`,
    { positive: true },
  );
  const plannedHeight = parseSumeragiUnsigned(
    record.planned_coordinator_block_height,
    `${context}.planned_coordinator_block_height`,
    { positive: true },
  );
  const coordinatorView = parseSumeragiUnsigned(
    record.coordinator_lane_block_view,
    `${context}.coordinator_lane_block_view`,
  );
  const participantPreviousHeight = parseSumeragiUnsigned(
    record.participant_previous_block_height,
    `${context}.participant_previous_block_height`,
  );
  const participantPreviousDescriptorHash =
    record.participant_previous_block_descriptor_hash === null
      ? null
      : parseSumeragiNonzeroHash(
          record.participant_previous_block_descriptor_hash,
          `${context}.participant_previous_block_descriptor_hash`,
        );
  const participantHeight = parseSumeragiUnsigned(
    record.participant_lane_block_height,
    `${context}.participant_lane_block_height`,
    { positive: true },
  );
  const participantView = parseSumeragiUnsigned(
    record.participant_lane_block_view,
    `${context}.participant_lane_block_view`,
  );
  const sourceId = parseSumeragiByte32(record.source_id, `${context}.source_id`);
  const entrypointHash = parseSumeragiHash(
    record.tx_entrypoint_hash,
    `${context}.tx_entrypoint_hash`,
  );
  if (
    round.height !== authorityHeight ||
    !sumeragiUnsignedSuccessorOf(participantHeight, participantPreviousHeight) ||
    (participantPreviousHeight === 0) !== (participantPreviousDescriptorHash === null) ||
    minQuorum !== expectedQuorum
  ) {
    throw new RangeError(`${context} contains inconsistent round or quorum fields`);
  }
  return Object.freeze({
    round,
    epoch: parseSumeragiUnsigned(record.epoch, `${context}.epoch`),
    network_id: parseSumeragiHash(record.network_id, `${context}.network_id`),
    source_id: sourceId,
    tx_entrypoint_hash: entrypointHash,
    plan_digest: parseSumeragiHash(record.plan_digest, `${context}.plan_digest`),
    phase,
    coordinator_lane_id: parseSumeragiUnsigned(
      record.coordinator_lane_id,
      `${context}.coordinator_lane_id`,
      { max: 0xffffffff },
    ),
    coordinator_dataspace_id: parseSumeragiUnsigned(
      record.coordinator_dataspace_id,
      `${context}.coordinator_dataspace_id`,
    ),
    coordinator_lane_incarnation: parseSumeragiNonzeroHash(
      record.coordinator_lane_incarnation,
      `${context}.coordinator_lane_incarnation`,
    ),
    participant_lane_id: parseSumeragiUnsigned(
      record.participant_lane_id,
      `${context}.participant_lane_id`,
      { max: 0xffffffff },
    ),
    participant_dataspace_id: parseSumeragiUnsigned(
      record.participant_dataspace_id,
      `${context}.participant_dataspace_id`,
    ),
    participant_lane_incarnation: parseSumeragiNonzeroHash(
      record.participant_lane_incarnation,
      `${context}.participant_lane_incarnation`,
    ),
    participant_previous_block_height: participantPreviousHeight,
    participant_previous_block_descriptor_hash: participantPreviousDescriptorHash,
    participant_lane_block_height: participantHeight,
    participant_lane_block_view: participantView,
    participant_proposal_hash: parseSumeragiNonzeroHash(
      record.participant_proposal_hash,
      `${context}.participant_proposal_hash`,
    ),
    participant_settlement_commitment: parseSumeragiNonzeroHash(
      record.participant_settlement_commitment,
      `${context}.participant_settlement_commitment`,
    ),
    participant_validator_set_hash: parseSumeragiHash(
      record.participant_validator_set_hash,
      `${context}.participant_validator_set_hash`,
    ),
    participant_validator_count: validatorCount,
    participant_min_quorum: minQuorum,
    authority_context_height: authorityHeight,
    planned_coordinator_block_height: plannedHeight,
    coordinator_lane_block_view: coordinatorView,
    coordinator_proposal_hash: parseSumeragiNonzeroHash(
      record.coordinator_proposal_hash,
      `${context}.coordinator_proposal_hash`,
    ),
  });
}

function sumeragiNativeAmxBodyIdentityEqual(left, right) {
  const fields = [
    "epoch",
    "network_id",
    "source_id",
    "tx_entrypoint_hash",
    "plan_digest",
    "coordinator_lane_id",
    "coordinator_dataspace_id",
    "coordinator_lane_incarnation",
    "participant_lane_id",
    "participant_dataspace_id",
    "participant_lane_incarnation",
    "participant_previous_block_height",
    "participant_previous_block_descriptor_hash",
    "participant_lane_block_height",
    "participant_lane_block_view",
    "participant_proposal_hash",
    "participant_settlement_commitment",
    "participant_validator_set_hash",
    "participant_validator_count",
    "participant_min_quorum",
    "authority_context_height",
    "planned_coordinator_block_height",
    "coordinator_lane_block_view",
    "coordinator_proposal_hash",
  ];
  return sumeragiRoundsEqual(left.round, right.round)
    && fields.every((field) => left[field] === right[field]);
}

function countSumeragiBitmapSigners(bitmap) {
  return bitmap.reduce((total, byte) => {
    let value = byte;
    let count = 0;
    while (value !== 0) {
      count += value & 1;
      value >>>= 1;
    }
    return total + count;
  }, 0);
}

const SUMERAGI_BLS_NORMAL_PEER_ID_PATTERN =
  /^(?:bls_normal:)?(ea0130[0-9A-F]{96})$/;

const SUMERAGI_BLS_NORMAL_VALIDATION_CACHE_MAX = 256;

const validatedSumeragiBlsNormalPublicKeys = new Set();

const SUMERAGI_NATIVE_DESCRIPTOR_PREIMAGE_TYPE =
  "iroha_data_model::block::consensus::LaneBlockDescriptorPreimage";

const SUMERAGI_NATIVE_PROPOSAL_PREIMAGE_TYPE =
  "iroha_data_model::block::consensus::LaneBlockProposalPreimage";

const SUMERAGI_NATIVE_SETTLEMENT_TYPE =
  "iroha_data_model::block::consensus::LaneBlockCommitment";

const SUMERAGI_NATIVE_SETTLEMENT_HASH_DOMAIN = Buffer.from(
  "iroha.nexus.lane-relay.settlement.v1",
  "utf8",
);

function parseSumeragiBlsNormalPeerId(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical BLS-Normal PeerId string`);
  }
  if (value.trim() !== value) {
    throw new TypeError(`${context} must not contain surrounding whitespace`);
  }
  const matched = SUMERAGI_BLS_NORMAL_PEER_ID_PATTERN.exec(value);
  if (matched === null) {
    throw new TypeError(`${context} must be a canonical BLS-Normal PeerId`);
  }
  const publicKeyHex = matched[1].slice(6);
  const compressed = Buffer.from(publicKeyHex, "hex");
  try {
    if (!validatedSumeragiBlsNormalPublicKeys.has(publicKeyHex)) {
      assertCanonicalBls12381G1Compressed(compressed);
      if (
        validatedSumeragiBlsNormalPublicKeys.size
        >= SUMERAGI_BLS_NORMAL_VALIDATION_CACHE_MAX
      ) {
        validatedSumeragiBlsNormalPublicKeys.delete(
          validatedSumeragiBlsNormalPublicKeys.values().next().value,
        );
      }
      validatedSumeragiBlsNormalPublicKeys.add(publicKeyHex);
    }
  } catch (error) {
    throw new TypeError(`${context} contains an invalid BLS-Normal public key`, {
      cause: error,
    });
  }
  return Object.freeze({
    literal: matched[1],
    orderingKey: Buffer.concat([Buffer.from([2]), compressed]),
  });
}

function parseSumeragiBlsNormalValidatorSet(value, context) {
  const parsed = value.map((validator, index) =>
    parseSumeragiBlsNormalPeerId(validator, `${context}[${index}]`),
  );
  if (
    parsed.some(
      (validator, index) =>
        index > 0 &&
        Buffer.compare(parsed[index - 1].orderingKey, validator.orderingKey) >= 0,
    )
  ) {
    throw new TypeError(
      `${context} must be strictly ordered by canonical validator id`,
    );
  }
  return Object.freeze(parsed.map((validator) => validator.literal));
}

function encodeSumeragiNativeU32(value) {
  const buffer = Buffer.allocUnsafe(4);
  buffer.writeUInt32LE(value, 0);
  return buffer;
}

function encodeSumeragiNativeStruct(fields) {
  return Buffer.concat(fields.map((field) => encodeNoritoField(field, true)));
}

function encodeSumeragiNativeString(value) {
  const encoded = Buffer.from(value, "utf8");
  return Buffer.concat([encodeUnsignedLeb128(encoded.length), encoded]);
}

function encodeSumeragiNativeHash(value) {
  return Buffer.from(parseHashLiteralToHex(value, "Native AMX canonical hash"), "hex");
}

function encodeSumeragiNativeLaneId(value) {
  return encodeNoritoField(encodeSumeragiNativeU32(value), true);
}

function encodeSumeragiNativeDataspaceId(value) {
  return encodeNoritoField(u64ToLittleEndianBuffer(value), true);
}

function encodeSumeragiNativeOptionalHash(value) {
  if (value === null || value === undefined) {
    return Buffer.from([0]);
  }
  return Buffer.concat([
    Buffer.from([1]),
    encodeNoritoField(encodeSumeragiNativeHash(value), true),
  ]);
}

function encodeSumeragiNativePeerId(value) {
  const { orderingKey } = parseSumeragiBlsNormalPeerId(
    value,
    "Native AMX validator",
  );
  const compactKey = Buffer.concat([
    u64ToLittleEndianBuffer(orderingKey.length),
    ...Array.from(orderingKey, (byte) =>
      encodeNoritoField(Buffer.from([byte]), true),
    ),
  ]);
  return encodeNoritoField(compactKey, true);
}

function encodeSumeragiNativeValidatorSet(validators) {
  return encodeNoritoVec(
    validators,
    encodeSumeragiNativePeerId,
    true,
  );
}

function formatSumeragiNativeHash(payload) {
  return formatHashLiteral(irohaHashBytes([payload]).toString("hex"));
}

function computeSumeragiNativeValidatorSetHash(validators) {
  return formatSumeragiNativeHash(
    encodeSumeragiNativeValidatorSet(validators),
  );
}

function computeSumeragiNativeDescriptorHash(descriptor) {
  const payload = encodeSumeragiNativeStruct([
    encodeSumeragiNativeString("nexus:lane-block-descriptor:v1"),
    Buffer.from([1]),
    encodeSumeragiNativeLaneId(descriptor.lane_id),
    encodeSumeragiNativeDataspaceId(descriptor.dataspace_id),
    encodeSumeragiNativeHash(descriptor.lane_incarnation),
    u64ToLittleEndianBuffer(descriptor.proposal_height),
    u64ToLittleEndianBuffer(descriptor.previous_lane_block_height),
    encodeSumeragiNativeOptionalHash(
      descriptor.previous_lane_block_descriptor_hash,
    ),
    u64ToLittleEndianBuffer(descriptor.lane_block_height),
    u64ToLittleEndianBuffer(descriptor.lane_block_view),
    encodeSumeragiNativeHash(descriptor.subject_hash),
    encodeSumeragiNativeHash(descriptor.payload_ownership_hash),
    encodeSumeragiNativeHash(descriptor.rbc_instance_hash),
    encodeNoritoVec(
      descriptor.accepted_candidate_indices,
      u64ToLittleEndianBuffer,
      true,
    ),
    encodeNoritoVec(
      descriptor.accepted_transaction_hashes,
      encodeSumeragiNativeHash,
      true,
    ),
    Buffer.from([
      descriptor.validator_set_hash_version & 0xff,
      (descriptor.validator_set_hash_version >>> 8) & 0xff,
    ]),
    encodeSumeragiNativeHash(descriptor.validator_set_hash),
    encodeSumeragiNativeValidatorSet(descriptor.validator_set),
    encodeSumeragiNativeU32(descriptor.validator_count),
    encodeSumeragiNativeU32(descriptor.min_quorum),
    encodeSumeragiNativeString(descriptor.qc_mode_tag),
  ]);
  return formatSumeragiNativeHash(
    frameNoritoPayload(
      SUMERAGI_NATIVE_DESCRIPTOR_PREIMAGE_TYPE,
      payload,
      2,
    ),
  );
}

function computeSumeragiNativeProposalHash(descriptor) {
  const payload = encodeSumeragiNativeStruct([
    encodeSumeragiNativeString("nexus:lane-block-proposal:v1"),
    Buffer.from([1]),
    u64ToLittleEndianBuffer(descriptor.proposal_height),
    encodeSumeragiNativeHash(descriptor.descriptor_hash),
    encodeSumeragiNativeLaneId(descriptor.lane_id),
    encodeSumeragiNativeDataspaceId(descriptor.dataspace_id),
    encodeSumeragiNativeHash(descriptor.lane_incarnation),
    u64ToLittleEndianBuffer(descriptor.lane_block_height),
    u64ToLittleEndianBuffer(descriptor.lane_block_view),
    encodeSumeragiNativeHash(descriptor.subject_hash),
    encodeSumeragiNativeHash(descriptor.payload_ownership_hash),
    encodeSumeragiNativeHash(descriptor.rbc_instance_hash),
    encodeNoritoVec(
      descriptor.accepted_candidate_indices,
      u64ToLittleEndianBuffer,
      true,
    ),
    encodeNoritoVec(
      descriptor.accepted_transaction_hashes,
      encodeSumeragiNativeHash,
      true,
    ),
    Buffer.from([
      descriptor.validator_set_hash_version & 0xff,
      (descriptor.validator_set_hash_version >>> 8) & 0xff,
    ]),
    encodeSumeragiNativeHash(descriptor.validator_set_hash),
    encodeSumeragiNativeValidatorSet(descriptor.validator_set),
    encodeSumeragiNativeU32(descriptor.validator_count),
    encodeSumeragiNativeU32(descriptor.min_quorum),
    encodeSumeragiNativeString(descriptor.qc_mode_tag),
  ]);
  return formatSumeragiNativeHash(
    frameNoritoPayload(
      SUMERAGI_NATIVE_PROPOSAL_PREIMAGE_TYPE,
      payload,
      2,
    ),
  );
}

function encodeSumeragiNativeBigInt(value) {
  if (value === 0n) {
    return Buffer.alloc(0);
  }
  let hex = value.toString(16);
  if (hex.length % 2 !== 0) {
    hex = `0${hex}`;
  }
  const bigEndian = Buffer.from(hex, "hex");
  const littleEndian = Buffer.from(bigEndian).reverse();
  return (littleEndian[littleEndian.length - 1] & 0x80) !== 0
    ? Buffer.concat([littleEndian, Buffer.from([0])])
    : littleEndian;
}

function encodeSumeragiNativeQuantity(value) {
  const [whole, fraction = ""] = value.split(".");
  const mantissa = BigInt(`${whole}${fraction}`);
  const encoded = encodeSumeragiNativeBigInt(mantissa);
  return encodeSumeragiNativeStruct([
    Buffer.concat([encodeSumeragiNativeU32(encoded.length), encoded]),
    encodeSumeragiNativeU32(fraction.length),
  ]);
}

function encodeSumeragiNativeSettlementReceipt(receipt) {
  return encodeSumeragiNativeStruct([
    Buffer.from(receipt.source_id, "hex"),
    encodeSumeragiNativeQuantity(receipt.local_amount),
    encodeSumeragiNativeQuantity(receipt.xor_due),
    encodeSumeragiNativeQuantity(receipt.xor_after_haircut),
    encodeSumeragiNativeQuantity(receipt.xor_variance),
    u64ToLittleEndianBuffer(receipt.timestamp_ms),
  ]);
}

function computeSumeragiNativeParticipantSettlementHash(settlement) {
  const payload = encodeSumeragiNativeStruct([
    u64ToLittleEndianBuffer(settlement.block_height),
    encodeSumeragiNativeLaneId(settlement.lane_id),
    encodeSumeragiNativeHash(settlement.lane_incarnation),
    encodeSumeragiNativeDataspaceId(settlement.dataspace_id),
    u64ToLittleEndianBuffer(settlement.tx_count),
    encodeSumeragiNativeQuantity(settlement.total_local_amount),
    encodeSumeragiNativeQuantity(settlement.total_xor_due),
    encodeSumeragiNativeQuantity(settlement.total_xor_after_haircut),
    encodeSumeragiNativeQuantity(settlement.total_xor_variance),
    Buffer.from([0]),
    encodeNoritoVec(
      settlement.receipts,
      encodeSumeragiNativeSettlementReceipt,
      true,
    ),
    encodeNoritoVec([], (value) => value, true),
    encodeNoritoVec([], (value) => value, true),
  ]);
  const framedSettlement = frameNoritoPayload(
    SUMERAGI_NATIVE_SETTLEMENT_TYPE,
    payload,
    2,
  );
  return formatSumeragiNativeHash(Buffer.concat([
    u64ToLittleEndianBuffer(SUMERAGI_NATIVE_SETTLEMENT_HASH_DOMAIN.length),
    SUMERAGI_NATIVE_SETTLEMENT_HASH_DOMAIN,
    framedSettlement,
  ]));
}

function parseSumeragiNativeAmxQc(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    [
      "body",
      "validator_set_hash_version",
      "validator_set_hash",
      "validator_set",
      "validator_set_pops",
      "signers_bitmap",
      "bls_aggregate_signature",
    ],
    context,
  );
  const body = parseSumeragiNativeAmxBody(record.body, `${context}.body`);
  const version = parseSumeragiUnsigned(
    record.validator_set_hash_version,
    `${context}.validator_set_hash_version`,
    { max: 0xffff },
  );
  if (version !== 1) {
    throw new RangeError(`${context}.validator_set_hash_version must equal 1`);
  }
  const validators = parseSumeragiBlsNormalValidatorSet(
    assertSumeragiArrayBound(
      record.validator_set,
      128,
      `${context}.validator_set`,
      1,
    ),
    `${context}.validator_set`,
  );
  const validatorSetHash = parseSumeragiHash(
    record.validator_set_hash,
    `${context}.validator_set_hash`,
  );
  const computedValidatorSetHash =
    computeSumeragiNativeValidatorSetHash(validators);
  const expectedQuorum = validators.length - Math.floor((validators.length - 1) / 3);
  if (
    body.participant_validator_count !== validators.length ||
    body.participant_min_quorum !== expectedQuorum ||
    body.participant_validator_set_hash !== validatorSetHash ||
    validatorSetHash !== computedValidatorSetHash
  ) {
    throw new TypeError(`${context} committee fields differ from its signed body`);
  }
  const pops = Object.freeze(
    assertSumeragiArrayBound(
      record.validator_set_pops,
      validators.length,
      `${context}.validator_set_pops`,
      validators.length,
    ).map((pop, index) =>
      parseSumeragiByteVector(pop, 96, `${context}.validator_set_pops[${index}]`),
    ),
  );
  if (pops.some((pop) => pop.every((byte) => byte === 0))) {
    throw new TypeError(`${context}.validator_set_pops contains an all-zero proof`);
  }
  const bitmapLength = Math.ceil(validators.length / 8);
  const bitmap = parseSumeragiByteVector(
    record.signers_bitmap,
    bitmapLength,
    `${context}.signers_bitmap`,
  );
  const trailingBits = validators.length % 8;
  if (trailingBits !== 0 && (bitmap[bitmap.length - 1] & ~((1 << trailingBits) - 1)) !== 0) {
    throw new TypeError(`${context}.signers_bitmap addresses an unknown validator`);
  }
  if (countSumeragiBitmapSigners(bitmap) !== expectedQuorum) {
    throw new RangeError(`${context}.signers_bitmap does not carry the exact quorum`);
  }
  const signature = parseSumeragiByteVector(
    record.bls_aggregate_signature,
    96,
    `${context}.bls_aggregate_signature`,
  );
  if (signature.every((byte) => byte === 0)) {
    throw new TypeError(`${context}.bls_aggregate_signature must not be all zeroes`);
  }
  return Object.freeze({
    body,
    validator_set_hash_version: version,
    validator_set_hash: validatorSetHash,
    validator_set: validators,
    validator_set_pops: pops,
    signers_bitmap: bitmap,
    bls_aggregate_signature: signature,
  });
}

function parseSumeragiNativeAmxParticipantProposal(value, context) {
  const proposal = assertExactSumeragiRecord(
    value,
    ["descriptor", "proposal_hash", "payload_block_hint"],
    context,
  );
  if (proposal.payload_block_hint !== null) {
    throw new TypeError(`${context}.payload_block_hint must be null`);
  }
  const descriptorContext = `${context}.descriptor`;
  const descriptor = ensureRecord(proposal.descriptor, descriptorContext);
  const requiredFields = [
    "lane_id",
    "dataspace_id",
    "lane_incarnation",
    "proposal_height",
    "previous_lane_block_height",
    "lane_block_height",
    "lane_block_view",
    "subject_hash",
    "payload_ownership_hash",
    "rbc_instance_hash",
    "accepted_candidate_indices",
    "accepted_transaction_hashes",
    "validator_set_hash_version",
    "validator_set_hash",
    "validator_set",
    "validator_count",
    "min_quorum",
    "qc_mode_tag",
    "descriptor_hash",
  ];
  const allowedFields = new Set([
    ...requiredFields,
    "previous_lane_block_descriptor_hash",
  ]);
  for (const field of requiredFields) {
    if (!Object.prototype.hasOwnProperty.call(descriptor, field)) {
      throw new TypeError(`${descriptorContext} is missing required field ${field}`);
    }
  }
  for (const field of Object.keys(descriptor)) {
    if (!allowedFields.has(field)) {
      throw new TypeError(`${descriptorContext} contains unknown field ${field}`);
    }
  }

  const previousHeight = parseSumeragiUnsigned(
    descriptor.previous_lane_block_height,
    `${descriptorContext}.previous_lane_block_height`,
  );
  let previousDescriptorHash = null;
  if (previousHeight === 0) {
    if (Object.prototype.hasOwnProperty.call(descriptor, "previous_lane_block_descriptor_hash")) {
      throw new TypeError(`${descriptorContext} must omit the genesis predecessor hash`);
    }
  } else {
    if (!Object.prototype.hasOwnProperty.call(descriptor, "previous_lane_block_descriptor_hash")) {
      throw new TypeError(`${descriptorContext} must carry a predecessor descriptor hash`);
    }
    previousDescriptorHash = parseSumeragiNonzeroHash(
      descriptor.previous_lane_block_descriptor_hash,
      `${descriptorContext}.previous_lane_block_descriptor_hash`,
    );
  }
  const laneBlockHeight = parseSumeragiUnsigned(
    descriptor.lane_block_height,
    `${descriptorContext}.lane_block_height`,
    { positive: true },
  );
  if (!sumeragiUnsignedSuccessorOf(laneBlockHeight, previousHeight)) {
    throw new RangeError(`${descriptorContext} lane-block heights must be contiguous`);
  }

  const acceptedCandidateIndices = Object.freeze(
    assertSumeragiArrayBound(
      descriptor.accepted_candidate_indices,
      4096,
      `${descriptorContext}.accepted_candidate_indices`,
      1,
    ).map((candidate, index) =>
      parseSumeragiUnsigned(
        candidate,
        `${descriptorContext}.accepted_candidate_indices[${index}]`,
      ),
    ),
  );
  const acceptedTransactionHashes = Object.freeze(
    assertSumeragiArrayBound(
      descriptor.accepted_transaction_hashes,
      4096,
      `${descriptorContext}.accepted_transaction_hashes`,
      1,
    ).map((hash, index) =>
      parseSumeragiNonzeroHash(
        hash,
        `${descriptorContext}.accepted_transaction_hashes[${index}]`,
      ),
    ),
  );
  if (
    acceptedCandidateIndices.length !== acceptedTransactionHashes.length ||
    new Set(acceptedCandidateIndices).size !== acceptedCandidateIndices.length ||
    new Set(acceptedTransactionHashes).size !== acceptedTransactionHashes.length
  ) {
    throw new TypeError(`${descriptorContext} accepted work is inconsistent`);
  }

  const validators = parseSumeragiBlsNormalValidatorSet(
    assertSumeragiArrayBound(
      descriptor.validator_set,
      128,
      `${descriptorContext}.validator_set`,
      1,
    ),
    `${descriptorContext}.validator_set`,
  );
  const validatorCount = parseSumeragiExactUnsigned(
    descriptor.validator_count,
    `${descriptorContext}.validator_count`,
    { positive: true, max: 128 },
  );
  const minQuorum = parseSumeragiExactUnsigned(
    descriptor.min_quorum,
    `${descriptorContext}.min_quorum`,
    { positive: true, max: 128 },
  );
  const expectedQuorum = validators.length - Math.floor((validators.length - 1) / 3);
  const validatorSetHashVersion = parseSumeragiExactUnsigned(
    descriptor.validator_set_hash_version,
    `${descriptorContext}.validator_set_hash_version`,
    { max: 0xffff },
  );
  if (
    validatorSetHashVersion !== 1 ||
    validatorCount !== validators.length ||
    minQuorum !== expectedQuorum
  ) {
    throw new TypeError(`${descriptorContext} committee fields are inconsistent`);
  }

  const normalizedDescriptor = {
    lane_id: parseSumeragiExactUnsigned(descriptor.lane_id, `${descriptorContext}.lane_id`, {
      max: 0xffffffff,
    }),
    dataspace_id: parseSumeragiUnsigned(
      descriptor.dataspace_id,
      `${descriptorContext}.dataspace_id`,
    ),
    lane_incarnation: parseSumeragiNonzeroHash(
      descriptor.lane_incarnation,
      `${descriptorContext}.lane_incarnation`,
    ),
    proposal_height: parseSumeragiUnsigned(
      descriptor.proposal_height,
      `${descriptorContext}.proposal_height`,
      { positive: true },
    ),
    previous_lane_block_height: previousHeight,
    ...(previousDescriptorHash === null
      ? {}
      : { previous_lane_block_descriptor_hash: previousDescriptorHash }),
    lane_block_height: laneBlockHeight,
    lane_block_view: parseSumeragiUnsigned(
      descriptor.lane_block_view,
      `${descriptorContext}.lane_block_view`,
    ),
    subject_hash: parseSumeragiNonzeroHash(
      descriptor.subject_hash,
      `${descriptorContext}.subject_hash`,
    ),
    payload_ownership_hash: parseSumeragiNonzeroHash(
      descriptor.payload_ownership_hash,
      `${descriptorContext}.payload_ownership_hash`,
    ),
    rbc_instance_hash: parseSumeragiNonzeroHash(
      descriptor.rbc_instance_hash,
      `${descriptorContext}.rbc_instance_hash`,
    ),
    accepted_candidate_indices: acceptedCandidateIndices,
    accepted_transaction_hashes: acceptedTransactionHashes,
    validator_set_hash_version: validatorSetHashVersion,
    validator_set_hash: parseSumeragiHash(
      descriptor.validator_set_hash,
      `${descriptorContext}.validator_set_hash`,
    ),
    validator_set: validators,
    validator_count: validatorCount,
    min_quorum: minQuorum,
    qc_mode_tag: requireExactNonEmptyString(
      descriptor.qc_mode_tag,
      `${descriptorContext}.qc_mode_tag`,
    ),
    descriptor_hash: parseSumeragiNonzeroHash(
      descriptor.descriptor_hash,
      `${descriptorContext}.descriptor_hash`,
    ),
  };
  if (
    normalizedDescriptor.validator_set_hash !==
    computeSumeragiNativeValidatorSetHash(validators)
  ) {
    throw new TypeError(
      `${descriptorContext}.validator_set_hash does not match the canonical committee`,
    );
  }
  if (
    normalizedDescriptor.descriptor_hash !==
    computeSumeragiNativeDescriptorHash(normalizedDescriptor)
  ) {
    throw new TypeError(
      `${descriptorContext}.descriptor_hash does not match its canonical preimage`,
    );
  }
  const proposalHash = parseSumeragiNonzeroHash(
    proposal.proposal_hash,
    `${context}.proposal_hash`,
  );
  if (proposalHash !== computeSumeragiNativeProposalHash(normalizedDescriptor)) {
    throw new TypeError(
      `${context}.proposal_hash does not match its canonical preimage`,
    );
  }
  return Object.freeze({
    descriptor: Object.freeze(normalizedDescriptor),
    proposal_hash: proposalHash,
    payload_block_hint: null,
  });
}

function parseSumeragiNativeAmxLeg(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    [
      "lane_id",
      "dataspace_id",
      "participant_proposal",
      "participant_settlement",
      "participant_settlement_hash",
      "prepare_qc",
      "commit_qc",
    ],
    context,
  );
  const laneId = parseSumeragiUnsigned(record.lane_id, `${context}.lane_id`, {
    max: 0xffffffff,
  });
  const dataspaceId = parseSumeragiUnsigned(record.dataspace_id, `${context}.dataspace_id`);
  const participantProposal = parseSumeragiNativeAmxParticipantProposal(
    record.participant_proposal,
    `${context}.participant_proposal`,
  );
  const settlementWire = ensureRecord(
    record.participant_settlement,
    `${context}.participant_settlement`,
  );
  if (
    !Array.isArray(settlementWire.native_amx_receipts) ||
    settlementWire.native_amx_receipts.length !== 0
  ) {
    throw new TypeError(`${context}.participant_settlement must be terminal`);
  }
  if (
    !Array.isArray(settlementWire.nexus_fee_receipts) ||
    settlementWire.nexus_fee_receipts.length !== 0
  ) {
    throw new TypeError(`${context}.participant_settlement cannot contain fee receipts`);
  }
  assertSumeragiArrayBound(
    settlementWire.receipts,
    MAX_NATIVE_AMX_PARTICIPANT_SETTLEMENT_RECEIPTS,
    `${context}.participant_settlement.receipts`,
    1,
  );
  const participantSettlement = parseLaneSettlementCommitments([settlementWire])[0];
  const participantSettlementHash = parseSumeragiNonzeroHash(
    record.participant_settlement_hash,
    `${context}.participant_settlement_hash`,
  );
  if (
    participantSettlementHash !==
    computeSumeragiNativeParticipantSettlementHash(participantSettlement)
  ) {
    throw new TypeError(
      `${context}.participant_settlement_hash does not match its canonical commitment`,
    );
  }
  const prepareQc = parseSumeragiNativeAmxQc(record.prepare_qc, `${context}.prepare_qc`);
  const commitQc = parseSumeragiNativeAmxQc(record.commit_qc, `${context}.commit_qc`);
  if (prepareQc.body.phase.phase !== "prepare") {
    throw new TypeError(`${context}.prepare_qc carries the wrong phase`);
  }
  if (commitQc.body.phase.phase !== "commit") {
    throw new TypeError(`${context}.commit_qc carries the wrong phase`);
  }
  if (!sumeragiNativeAmxBodyIdentityEqual(prepareQc.body, commitQc.body)) {
    throw new TypeError(`${context} prepare and commit identities differ`);
  }
  for (const field of [
    "validator_set_hash_version",
    "validator_set_hash",
    "validator_set",
    "validator_set_pops",
  ]) {
    if (JSON.stringify(prepareQc[field]) !== JSON.stringify(commitQc[field])) {
      throw new TypeError(`${context} prepare and commit committees differ`);
    }
  }
  const body = prepareQc.body;
  const descriptor = participantProposal.descriptor;
  if (
    body.participant_lane_id !== laneId ||
    body.participant_dataspace_id !== dataspaceId ||
    descriptor.lane_id !== laneId ||
    descriptor.dataspace_id !== dataspaceId ||
    descriptor.lane_incarnation !== body.participant_lane_incarnation ||
    descriptor.proposal_height !== body.authority_context_height ||
    descriptor.previous_lane_block_height !== body.participant_previous_block_height ||
    (descriptor.previous_lane_block_descriptor_hash ?? null) !==
      body.participant_previous_block_descriptor_hash ||
    descriptor.lane_block_height !== body.participant_lane_block_height ||
    descriptor.lane_block_view !== body.participant_lane_block_view ||
    participantProposal.proposal_hash !== body.participant_proposal_hash ||
    descriptor.validator_set_hash_version !== prepareQc.validator_set_hash_version ||
    descriptor.validator_set_hash !== prepareQc.validator_set_hash ||
    JSON.stringify(descriptor.validator_set) !== JSON.stringify(prepareQc.validator_set) ||
    descriptor.validator_count !== body.participant_validator_count ||
    descriptor.min_quorum !== body.participant_min_quorum
  ) {
    throw new TypeError(`${context} participant proposal differs from its signed body`);
  }
  const receipts = participantSettlement.receipts;
  const receiptSources = receipts.map((receipt) => receipt.source_id);
  if (receiptSources.some((sourceId, index) => index > 0 && receiptSources[index - 1] >= sourceId)) {
    throw new TypeError(
      `${context}.participant_settlement.receipts must be strictly ordered by source_id`,
    );
  }
  const matchingEntrypointPositions = descriptor.accepted_transaction_hashes
    .flatMap((hash, index) => (hash === body.tx_entrypoint_hash ? [index] : []));
  if (matchingEntrypointPositions.length > 1) {
    throw new TypeError(
      `${context} participant descriptor repeats the current transaction entrypoint`,
    );
  }
  const requiresMixedRoleAnchorValidation = matchingEntrypointPositions.length === 0;
  if (
    !requiresMixedRoleAnchorValidation &&
    (
      descriptor.accepted_candidate_indices.length !== receipts.length ||
      descriptor.accepted_transaction_hashes.length !== receipts.length ||
      receiptSources[matchingEntrypointPositions[0]] !== body.source_id
    )
  ) {
    throw new TypeError(
      `${context} participant descriptor and grouped settlement are not aligned`,
    );
  }
  if (
    participantSettlementHash !== body.participant_settlement_commitment ||
    participantSettlement.block_height !== body.participant_lane_block_height ||
    participantSettlement.lane_id !== laneId ||
    participantSettlement.dataspace_id !== dataspaceId ||
    participantSettlement.lane_incarnation !== body.participant_lane_incarnation ||
    participantSettlement.tx_count !== receipts.length ||
    participantSettlement.total_local_amount !== "0" ||
    participantSettlement.total_xor_due !== "0" ||
    participantSettlement.total_xor_after_haircut !== "0" ||
    participantSettlement.total_xor_variance !== "0" ||
    participantSettlement.swap_metadata !== null ||
    new Set(receiptSources).size !== receiptSources.length ||
    receiptSources.filter((sourceId) => sourceId === body.source_id).length !== 1 ||
    receipts.some(
      (receipt) =>
        receipt.local_amount !== "0" ||
        receipt.xor_due !== "0" ||
        receipt.xor_after_haircut !== "0" ||
        receipt.xor_variance !== "0" ||
        receipt.timestamp_ms !== body.authority_context_height,
    ) ||
    participantSettlement.nexus_fee_receipts.length !== 0 ||
    participantSettlement.native_amx_receipts.length !== 0
  ) {
    throw new TypeError(`${context} participant settlement differs from its signed body`);
  }
  return Object.freeze({
    lane_id: laneId,
    dataspace_id: dataspaceId,
    participant_proposal: participantProposal,
    participant_settlement: participantSettlement,
    participant_settlement_hash: participantSettlementHash,
    prepare_qc: prepareQc,
    commit_qc: commitQc,
    requires_mixed_role_anchor_validation: requiresMixedRoleAnchorValidation,
  });
}

function parseSumeragiNativeAmxReceipt(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    [
      "version",
      "source_id",
      "network_id",
      "plan_digest",
      "lane_id",
      "dataspace_id",
      "lane_incarnation",
      "authority_context_height",
      "lane_block_height",
      "lane_block_view",
      "coordinator_proposal_hash",
      "legs",
    ],
    context,
  );
  const version = parseSumeragiUnsigned(record.version, `${context}.version`, { max: 0xffff });
  if (version !== 2) {
    throw new RangeError(`${context}.version must equal 2`);
  }
  const sourceId = parseSumeragiByte32(record.source_id, `${context}.source_id`);
  const networkId = parseSumeragiHash(record.network_id, `${context}.network_id`);
  const planDigest = parseSumeragiHash(record.plan_digest, `${context}.plan_digest`);
  const laneId = parseSumeragiUnsigned(record.lane_id, `${context}.lane_id`, {
    max: 0xffffffff,
  });
  const dataspaceId = parseSumeragiUnsigned(record.dataspace_id, `${context}.dataspace_id`);
  const laneIncarnation = parseSumeragiNonzeroHash(
    record.lane_incarnation,
    `${context}.lane_incarnation`,
  );
  const authorityHeight = parseSumeragiUnsigned(
    record.authority_context_height,
    `${context}.authority_context_height`,
    { positive: true },
  );
  const laneBlockHeight = parseSumeragiUnsigned(
    record.lane_block_height,
    `${context}.lane_block_height`,
    { positive: true },
  );
  const laneBlockView = parseSumeragiUnsigned(
    record.lane_block_view,
    `${context}.lane_block_view`,
  );
  const proposalHash = parseSumeragiNonzeroHash(
    record.coordinator_proposal_hash,
    `${context}.coordinator_proposal_hash`,
  );
  const legs = Object.freeze(
    assertSumeragiArrayBound(record.legs, 255, `${context}.legs`, 1).map((leg, index) =>
      parseSumeragiNativeAmxLeg(leg, `${context}.legs[${index}]`),
    ),
  );
  const routes = new Set(legs.map((leg) => `${leg.lane_id}:${leg.dataspace_id}`));
  if (routes.size !== legs.length) {
    throw new TypeError(`${context}.legs contains duplicate participant routes`);
  }
  const firstBody = legs[0].prepare_qc.body;
  for (const leg of legs) {
    const body = leg.prepare_qc.body;
    if (
      !sumeragiRoundsEqual(body.round, firstBody.round) ||
      body.epoch !== firstBody.epoch ||
      body.round.height !== authorityHeight ||
      body.network_id !== networkId ||
      body.source_id !== sourceId ||
      body.tx_entrypoint_hash !== firstBody.tx_entrypoint_hash ||
      body.plan_digest !== planDigest ||
      body.coordinator_lane_id !== laneId ||
      body.coordinator_dataspace_id !== dataspaceId ||
      body.coordinator_lane_incarnation !== laneIncarnation ||
      body.authority_context_height !== authorityHeight ||
      body.planned_coordinator_block_height !== laneBlockHeight ||
      body.coordinator_lane_block_view !== laneBlockView ||
      body.coordinator_proposal_hash !== proposalHash ||
      (leg.lane_id === laneId &&
        leg.dataspace_id === dataspaceId &&
        (
          leg.requires_mixed_role_anchor_validation ||
          leg.participant_proposal.descriptor.lane_incarnation !== laneIncarnation ||
          leg.participant_proposal.descriptor.lane_block_height !== laneBlockHeight ||
          leg.participant_proposal.descriptor.lane_block_view !== laneBlockView ||
          leg.participant_proposal.proposal_hash !== proposalHash
        ))
    ) {
      throw new TypeError(`${context}.legs contain mismatched signed identities`);
    }
  }
  return Object.freeze({
    version,
    source_id: sourceId,
    network_id: networkId,
    plan_digest: planDigest,
    lane_id: laneId,
    dataspace_id: dataspaceId,
    lane_incarnation: laneIncarnation,
    authority_context_height: authorityHeight,
    lane_block_height: laneBlockHeight,
    lane_block_view: laneBlockView,
    coordinator_proposal_hash: proposalHash,
    legs,
  });
}

function parseLaneSettlementCommitments(payload) {
  assertSumeragiArrayBound(
    payload,
    128,
    "status.lane_settlement_commitments",
  );
  return payload.map((entry, index) => {
    const context = `status.lane_settlement_commitments[${index}]`;
    const record = assertExactSumeragiRecord(
      entry,
      [
        "block_height",
        "lane_id",
        "lane_incarnation",
        "dataspace_id",
        "tx_count",
        "total_local_amount",
        "total_xor_due",
        "total_xor_after_haircut",
        "total_xor_variance",
        "swap_metadata",
        "receipts",
        "nexus_fee_receipts",
        "native_amx_receipts",
      ],
      context,
    );
    const swapMetadataRecord = record.swap_metadata;
    let swapMetadata = null;
    if (swapMetadataRecord != null) {
      const metadata = assertExactSumeragiRecord(
        swapMetadataRecord,
        [
          "epsilon_bps",
          "twap_window_seconds",
          "liquidity_profile",
          "twap_local_per_xor",
          "volatility_class",
        ],
        `status.lane_settlement_commitments[${index}].swap_metadata`,
      );
      swapMetadata = {
        epsilon_bps: parseSumeragiUnsigned(
          metadata.epsilon_bps,
          `status.lane_settlement_commitments[${index}].swap_metadata.epsilon_bps`,
          { max: 0xffff },
        ),
        twap_window_seconds: parseSumeragiUnsigned(
          metadata.twap_window_seconds,
          `status.lane_settlement_commitments[${index}].swap_metadata.twap_window_seconds`,
          { max: 0xffffffff },
        ),
        liquidity_profile: parseSumeragiTaggedUnitWithContent(
          metadata.liquidity_profile,
          "profile",
          "state",
          ["Tier1", "Tier2", "Tier3"],
          `status.lane_settlement_commitments[${index}].swap_metadata.liquidity_profile`,
        ),
        twap_local_per_xor: requireNonEmptyString(
          metadata.twap_local_per_xor,
          `status.lane_settlement_commitments[${index}].swap_metadata.twap_local_per_xor`,
        ),
        volatility_class: parseSumeragiTaggedUnitWithContent(
          metadata.volatility_class,
          "bucket",
          "state",
          ["Stable", "Elevated", "Dislocated"],
          `status.lane_settlement_commitments[${index}].swap_metadata.volatility_class`,
        ),
      };
    }
    const receiptsRecord = record.receipts;
    if (!Array.isArray(receiptsRecord)) {
      throw new TypeError(
        `status.lane_settlement_commitments[${index}].receipts must be an array`,
      );
    }
    const receipts = receiptsRecord.map((receipt, receiptIndex) => {
      const receiptRecord = assertExactSumeragiRecord(
        receipt,
        [
          "source_id",
          "local_amount",
          "xor_due",
          "xor_after_haircut",
          "xor_variance",
          "timestamp_ms",
        ],
        `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}]`,
      );
      return {
        source_id: parseSumeragiByte32(
          receiptRecord.source_id,
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}].source_id`,
        ),
        local_amount: requireCanonicalQuantity(
          receiptRecord.local_amount,
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}].local_amount`,
        ),
        xor_due: requireCanonicalQuantity(
          receiptRecord.xor_due,
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}].xor_due`,
        ),
        xor_after_haircut: requireCanonicalQuantity(
          receiptRecord.xor_after_haircut,
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}].xor_after_haircut`,
        ),
        xor_variance: requireCanonicalQuantity(
          receiptRecord.xor_variance,
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}].xor_variance`,
        ),
        timestamp_ms: parseSumeragiUnsigned(
          receiptRecord.timestamp_ms,
          `status.lane_settlement_commitments[${index}].receipts[${receiptIndex}].timestamp_ms`,
        ),
      };
    });
    const nexusFeeReceipts = Object.freeze(
      assertSumeragiArrayBound(
        record.nexus_fee_receipts,
        Number.MAX_SAFE_INTEGER,
        `${context}.nexus_fee_receipts`,
      ).map((receipt, receiptIndex) =>
        parseSumeragiNexusFeeReceipt(
          receipt,
          `${context}.nexus_fee_receipts[${receiptIndex}]`,
        ),
      ),
    );
    const nativeAmxReceipts = Object.freeze(
      assertSumeragiArrayBound(
        record.native_amx_receipts,
        MAX_NATIVE_AMX_PARTICIPANT_SETTLEMENT_RECEIPTS,
        `${context}.native_amx_receipts`,
      ).map((receipt, receiptIndex) =>
        parseSumeragiNativeAmxReceipt(
          receipt,
          `${context}.native_amx_receipts[${receiptIndex}]`,
        ),
      ),
    );
    const blockHeight = parseSumeragiUnsigned(record.block_height, `${context}.block_height`);
    const laneId = parseSumeragiUnsigned(record.lane_id, `${context}.lane_id`, {
      max: 0xffffffff,
    });
    const laneIncarnation = parseSumeragiNonzeroHash(
      record.lane_incarnation,
      `${context}.lane_incarnation`,
    );
    const dataspaceId = parseSumeragiUnsigned(record.dataspace_id, `${context}.dataspace_id`);
    if (new Set(nexusFeeReceipts.map((receipt) => receipt.source_id)).size !== nexusFeeReceipts.length) {
      throw new TypeError(`${context} contains duplicate Nexus fee receipt sources`);
    }
    if (new Set(nativeAmxReceipts.map((receipt) => receipt.source_id)).size !== nativeAmxReceipts.length) {
      throw new TypeError(`${context} contains duplicate native AMX receipt sources`);
    }
    const nativeAmxSources = nativeAmxReceipts.map((receipt) => receipt.source_id);
    if (nativeAmxSources.some(
      (sourceId, sourceIndex) =>
        sourceIndex > 0 && nativeAmxSources[sourceIndex - 1] >= sourceId,
    )) {
      throw new TypeError(`${context} native AMX receipt sources must be strictly ordered`);
    }
    if (
      nexusFeeReceipts.some(
        (receipt) =>
          receipt.lane_id !== laneId ||
          receipt.dataspace_id !== dataspaceId ||
          receipt.block_height !== blockHeight,
      )
    ) {
      throw new TypeError(`${context} Nexus fee receipt coordinates do not match`);
    }
    if (
      nativeAmxReceipts.some(
        (receipt) =>
          receipt.lane_id !== laneId ||
          receipt.dataspace_id !== dataspaceId ||
          receipt.lane_incarnation !== laneIncarnation ||
          receipt.lane_block_height !== blockHeight,
      )
    ) {
      throw new TypeError(`${context} native AMX receipt coordinates do not match`);
    }
    if (
      nativeAmxReceipts.some((receipt) =>
        receipt.legs.some((leg) =>
          JSON.stringify(
            leg.participant_settlement.receipts.map((entry) => entry.source_id),
          ) !== JSON.stringify(nativeAmxSources),
        ),
      )
    ) {
      throw new TypeError(
        `${context} native AMX receipts do not bind the exact ordered source group`,
      );
    }
    return {
      block_height: blockHeight,
      lane_id: laneId,
      lane_incarnation: laneIncarnation,
      dataspace_id: dataspaceId,
      tx_count: parseSumeragiUnsigned(
        record.tx_count,
        `status.lane_settlement_commitments[${index}].tx_count`,
      ),
      total_local_amount: requireCanonicalQuantity(
        record.total_local_amount,
        `status.lane_settlement_commitments[${index}].total_local_amount`,
      ),
      total_xor_due: requireCanonicalQuantity(
        record.total_xor_due,
        `status.lane_settlement_commitments[${index}].total_xor_due`,
      ),
      total_xor_after_haircut: requireCanonicalQuantity(
        record.total_xor_after_haircut,
        `status.lane_settlement_commitments[${index}].total_xor_after_haircut`,
      ),
      total_xor_variance: requireCanonicalQuantity(
        record.total_xor_variance,
        `status.lane_settlement_commitments[${index}].total_xor_variance`,
      ),
      swap_metadata: swapMetadata,
      receipts,
      nexus_fee_receipts: nexusFeeReceipts,
      native_amx_receipts: nativeAmxReceipts,
    };
  });
}

function parseLaneRelayEnvelopes(payload) {
  assertSumeragiArrayBound(payload, 64, "status.lane_relay_envelopes");
  return payload.map((entry, index) => {
    const context = `status.lane_relay_envelopes[${index}]`;
    const record = ensureRecord(entry, context);
    const blockHeader = ensureRecord(record.block_header, `${context}.block_header`);
    const qc =
      record.qc === undefined || record.qc === null ? null : ensureRecord(record.qc, `${context}.qc`);
    const settlementCommitments = parseLaneSettlementCommitments([
      record.settlement_commitment,
    ]);
    if (settlementCommitments.length !== 1) {
      throw new TypeError(`${context}.settlement_commitment must be an object`);
    }
    const laneId = parseSumeragiUnsigned(record.lane_id, `${context}.lane_id`, {
      max: 0xffffffff,
    });
    const laneIncarnation = parseSumeragiNonzeroHash(
      record.lane_incarnation,
      `${context}.lane_incarnation`,
    );
    const dataspaceId = parseSumeragiUnsigned(record.dataspace_id, `${context}.dataspace_id`);
    const blockHeight = parseSumeragiUnsigned(record.block_height, `${context}.block_height`);
    const settlementHash = parseSumeragiHash(
      record.settlement_hash,
      `${context}.settlement_hash`,
    );
    const manifestRoot = parseSumeragiOptionalByte32(
      record.manifest_root,
      `${context}.manifest_root`,
    );
    let fastpqProof = null;
    if (record.fastpq_proof !== undefined && record.fastpq_proof !== null) {
      const proof = ensureRecord(record.fastpq_proof, `${context}.fastpq_proof`);
      fastpqProof = {
        proof_digest: parseSumeragiHash(
          proof.proof_digest,
          `${context}.fastpq_proof.proof_digest`,
        ),
        verified_at_height: parseSumeragiUnsigned(
          proof.verified_at_height,
          `${context}.fastpq_proof.verified_at_height`,
        ),
      };
    }
    const settlement = settlementCommitments[0];
    if (
      settlement.lane_id !== laneId ||
      settlement.lane_incarnation !== laneIncarnation ||
      settlement.dataspace_id !== dataspaceId ||
      settlement.block_height !== blockHeight
    ) {
      throw new TypeError(`${context}.settlement_commitment identity must match its relay`);
    }
    return {
      lane_id: laneId,
      lane_incarnation: laneIncarnation,
      dataspace_id: dataspaceId,
      block_height: blockHeight,
      block_header: blockHeader,
      qc,
      da_commitment_hash:
        record.da_commitment_hash === undefined || record.da_commitment_hash === null
          ? null
          : parseSumeragiHash(record.da_commitment_hash, `${context}.da_commitment_hash`),
      lane_block_descriptor_hash:
        record.lane_block_descriptor_hash === undefined ||
        record.lane_block_descriptor_hash === null
          ? null
          : parseSumeragiHash(
              record.lane_block_descriptor_hash,
              `${context}.lane_block_descriptor_hash`,
            ),
      settlement_commitment: settlement,
      settlement_hash: settlementHash,
      rbc_bytes_total: parseSumeragiUnsigned(
        record.rbc_bytes_total,
        `${context}.rbc_bytes_total`,
      ),
      manifest_root: manifestRoot,
      fastpq_proof: fastpqProof,
    };
  });
}

function parseSumeragiStatusPayload(payload) {
  const record = ensureRecord(payload, "sumeragi status payload");
  const allowedFields = new Set([
    "protocol_version",
    "node_fingerprint",
    "build_fingerprint",
    "config_fingerprint",
    "restart_required",
    "height_context_id",
    "height",
    "view",
    "phase",
    "leader",
    "locked_prepare_qc",
    "highest_prepare_qc",
    "last_timeout_certificate",
    "body_state",
    "pending_persistence_id",
    "last_committed_height",
    "last_committed_subject",
    "height_context",
    "last_commit_qc",
    "liveness",
  ]);
  const unknownField = Object.keys(record).find((field) => !allowedFields.has(field));
  if (unknownField !== undefined) {
    throw new TypeError(`sumeragi status payload contains unknown field ${unknownField}`);
  }
  const protocolVersion = parseSumeragiUnsigned(
    record.protocol_version,
    "sumeragi.protocol_version",
    { max: 0xffff },
  );
  if (protocolVersion !== 4) {
    throw new RangeError("sumeragi.protocol_version must equal 4");
  }
  const height = parseSumeragiUnsigned(record.height, "sumeragi.height");
  const view = parseSumeragiUnsigned(record.view, "sumeragi.view");
  const heightContextId = parseSumeragiContextId(
    record.height_context_id,
    "sumeragi.height_context_id",
  );
  const leader = parseSumeragiUnsigned(record.leader, "sumeragi.leader", {
    max: 0xffffffff,
  });
  const restartRequired = parseSumeragiBoolean(
    record.restart_required,
    "sumeragi.restart_required",
  );
  const heightContext = parseSumeragiHeightContext(
    record.height_context,
    "sumeragi.height_context",
  );
  if (heightContext.epoch_end_height < height) {
    throw new RangeError("sumeragi.height_context.epoch_end_height must cover height");
  }
  if (leader >= heightContext.validator_count) {
    throw new RangeError("sumeragi.leader must index the frozen validator roster");
  }
  const liveness = parseSumeragiLivenessStatus(record.liveness, "sumeragi.liveness", {
    height,
    view,
    contextId: heightContextId,
    heightContext,
  });

  const lastCommittedHeight = parseSumeragiUnsigned(
    record.last_committed_height,
    "sumeragi.last_committed_height",
  );
  if (lastCommittedHeight > height) {
    throw new RangeError("sumeragi.last_committed_height must not exceed height");
  }
  const lastCommittedSubject =
    record.last_committed_subject == null
      ? null
      : parseSumeragiBlockSubject(
          record.last_committed_subject,
          "sumeragi.last_committed_subject",
        );
  const lastCommitQc =
    record.last_commit_qc == null
      ? null
      : parseSumeragiCommitQcStatus(record.last_commit_qc, "sumeragi.last_commit_qc");
  if (lastCommittedHeight === 0) {
    if (lastCommittedSubject !== null || lastCommitQc !== null) {
      throw new TypeError(
        "sumeragi last committed subject and QC must be absent at height zero",
      );
    }
  } else {
    if ((lastCommittedSubject === null) !== (lastCommitQc === null)) {
      throw new TypeError(
        "sumeragi last committed subject and QC are required together when either is present after height zero",
      );
    }
    if (lastCommittedSubject !== null && (
      lastCommitQc.certificate.phase.phase !== "commit" ||
      lastCommitQc.certificate.round.height !== lastCommittedHeight ||
      !sumeragiSubjectsEqual(lastCommitQc.certificate.subject, lastCommittedSubject)
    )) {
      throw new TypeError("sumeragi.last_commit_qc does not certify the committed subject");
    }
  }

  return Object.freeze({
    protocol_version: protocolVersion,
    node_fingerprint: parseSumeragiHash(
      record.node_fingerprint,
      "sumeragi.node_fingerprint",
    ),
    build_fingerprint: parseSumeragiHash(
      record.build_fingerprint,
      "sumeragi.build_fingerprint",
    ),
    config_fingerprint: parseSumeragiHash(
      record.config_fingerprint,
      "sumeragi.config_fingerprint",
    ),
    restart_required: restartRequired,
    height_context_id: heightContextId,
    height,
    view,
    phase: parseSumeragiTaggedUnit(
      record.phase,
      "phase",
      [
        "awaiting_proposal",
        "reconstructing_payload",
        "validating_payload",
        "prepare",
        "commit",
        "pending_apply",
      ],
      "sumeragi.phase",
    ),
    leader,
    locked_prepare_qc:
      record.locked_prepare_qc == null
        ? null
        : parseSumeragiQcReference(
            record.locked_prepare_qc,
            "sumeragi.locked_prepare_qc",
          ),
    highest_prepare_qc:
      record.highest_prepare_qc == null
        ? null
        : parseSumeragiQcReference(
            record.highest_prepare_qc,
            "sumeragi.highest_prepare_qc",
          ),
    last_timeout_certificate:
      record.last_timeout_certificate == null
        ? null
        : parseSumeragiTimeoutReference(
            record.last_timeout_certificate,
            "sumeragi.last_timeout_certificate",
          ),
    body_state: parseSumeragiTaggedUnit(
      record.body_state,
      "state",
      ["missing", "reconstructing", "stored", "validated", "pending_apply", "applied"],
      "sumeragi.body_state",
    ),
    pending_persistence_id:
      record.pending_persistence_id == null
        ? null
        : parseSumeragiUnsigned(
            record.pending_persistence_id,
            "sumeragi.pending_persistence_id",
          ),
    last_committed_height: lastCommittedHeight,
    last_committed_subject: lastCommittedSubject,
    height_context: heightContext,
    last_commit_qc: lastCommitQc,
    liveness,
  });
}

const SUMERAGI_PIPELINE_EXECUTION_FIELDS = Object.freeze([
  "tx_vertices_total",
  "tx_edges_total",
  "overlay_count_total",
  "overlay_instr_total",
  "overlay_bytes_total",
  "rbc_chunks_total",
  "rbc_bytes_total",
  "detached_prepared_total",
  "detached_merged_total",
  "detached_fallback_total",
  "detached_fallback_fee_postprocessing_total",
  "detached_fallback_user_executor_total",
  "detached_fallback_durable_state_total",
  "detached_fallback_unsupported_instruction_total",
  "detached_fallback_rejected_eval_total",
  "detached_fallback_overlay_error_total",
  "quarantine_executed_total",
]);

function parseSumeragiDiagnosticsPayload(payload) {
  const context = "sumeragi diagnostics";
  const record = ensureRecord(payload, context);
  const requiredFields = [
    "pipeline_execution",
    "tx_queue_depth",
    "tx_queue_capacity",
    "tx_queue_retained_bytes",
    "tx_queue_max_retained_bytes",
    "tx_queue_saturated",
    "tx_queue_saturated_by_count",
    "tx_queue_saturated_by_bytes",
    "tx_queue_saturated_by_age",
    "tx_queue_oldest_queued_age_ms",
    "lane_commitments",
    "dataspace_commitments",
    "lane_settlement_commitments",
    "lane_relay_envelopes",
    "lane_payload_ownerships",
    "committed_lane_blocks",
    "lane_block_sessions",
    "lane_governance_sealed_total",
    "lane_governance_sealed_aliases",
    "lane_governance",
    "native_amx_participant_applications",
    "autonomous_lane_executions",
  ];
  const allowedFields = new Set([...requiredFields, "npos"]);
  const unknown = Object.keys(record).find((field) => !allowedFields.has(field));
  if (unknown !== undefined) {
    throw new TypeError(`${context} contains unknown field ${unknown}`);
  }
  const missing = requiredFields.find(
    (field) => !Object.prototype.hasOwnProperty.call(record, field),
  );
  if (missing !== undefined) {
    throw new TypeError(`${context} is missing required field ${missing}`);
  }

  const pipelineRecord = assertExactSumeragiRecord(
    record.pipeline_execution,
    SUMERAGI_PIPELINE_EXECUTION_FIELDS,
    `${context}.pipeline_execution`,
  );
  const pipelineExecution = Object.freeze(Object.fromEntries(
    SUMERAGI_PIPELINE_EXECUTION_FIELDS.map((field) => [
      field,
      parseSumeragiUnsigned(
        pipelineRecord[field],
        `${context}.pipeline_execution.${field}`,
      ),
    ]),
  ));

  const txQueueDepth = parseSumeragiUnsigned(
    record.tx_queue_depth,
    `${context}.tx_queue_depth`,
  );
  const txQueueCapacity = parseSumeragiUnsigned(
    record.tx_queue_capacity,
    `${context}.tx_queue_capacity`,
  );
  const txQueueRetainedBytes = parseSumeragiUnsigned(
    record.tx_queue_retained_bytes,
    `${context}.tx_queue_retained_bytes`,
  );
  const txQueueMaxRetainedBytes = parseSumeragiUnsigned(
    record.tx_queue_max_retained_bytes,
    `${context}.tx_queue_max_retained_bytes`,
  );
  if (txQueueDepth > txQueueCapacity) {
    throw new RangeError(`${context} transaction queue depth exceeds capacity`);
  }
  if (txQueueRetainedBytes > txQueueMaxRetainedBytes) {
    throw new RangeError(`${context} retained queue bytes exceed the byte budget`);
  }
  const saturatedByCount = parseSumeragiBoolean(
    record.tx_queue_saturated_by_count,
    `${context}.tx_queue_saturated_by_count`,
  );
  const saturatedByBytes = parseSumeragiBoolean(
    record.tx_queue_saturated_by_bytes,
    `${context}.tx_queue_saturated_by_bytes`,
  );
  const saturatedByAge = parseSumeragiBoolean(
    record.tx_queue_saturated_by_age,
    `${context}.tx_queue_saturated_by_age`,
  );
  const saturated = parseSumeragiBoolean(
    record.tx_queue_saturated,
    `${context}.tx_queue_saturated`,
  );
  if (saturated !== (saturatedByCount || saturatedByBytes || saturatedByAge)) {
    throw new TypeError(`${context}.tx_queue_saturated disagrees with its causes`);
  }

  const sealedAliases = Object.freeze(
    assertSumeragiArrayBound(
      record.lane_governance_sealed_aliases,
      128,
      `${context}.lane_governance_sealed_aliases`,
    ).map((alias, index) =>
      requireExactNonEmptyString(
        alias,
        `${context}.lane_governance_sealed_aliases[${index}]`,
      ),
    ),
  );
  const sealedTotal = parseSumeragiUnsigned(
    record.lane_governance_sealed_total,
    `${context}.lane_governance_sealed_total`,
    { max: 0xffffffff },
  );
  if (sealedTotal !== sealedAliases.length || new Set(sealedAliases).size !== sealedAliases.length) {
    throw new TypeError(
      `${context} sealed lane aliases must be unique and match the sealed total`,
    );
  }

  return Object.freeze({
    pipeline_execution: pipelineExecution,
    tx_queue_depth: txQueueDepth,
    tx_queue_capacity: txQueueCapacity,
    tx_queue_retained_bytes: txQueueRetainedBytes,
    tx_queue_max_retained_bytes: txQueueMaxRetainedBytes,
    tx_queue_saturated: saturated,
    tx_queue_saturated_by_count: saturatedByCount,
    tx_queue_saturated_by_bytes: saturatedByBytes,
    tx_queue_saturated_by_age: saturatedByAge,
    tx_queue_oldest_queued_age_ms: parseSumeragiUnsigned(
      record.tx_queue_oldest_queued_age_ms,
      `${context}.tx_queue_oldest_queued_age_ms`,
    ),
    npos: record.npos == null ? null : parseSumeragiNposDiagnostics(record.npos),
    lane_commitments: parseSumeragiDiagnosticLaneCommitments(record.lane_commitments),
    dataspace_commitments: parseSumeragiDiagnosticDataspaceCommitments(
      record.dataspace_commitments,
    ),
    lane_settlement_commitments: parseLaneSettlementCommitments(
      record.lane_settlement_commitments,
    ),
    lane_relay_envelopes: parseLaneRelayEnvelopes(record.lane_relay_envelopes),
    lane_payload_ownerships: parseSumeragiLanePayloadOwnerships(
      record.lane_payload_ownerships,
    ),
    committed_lane_blocks: parseSumeragiCommittedLaneBlocks(
      record.committed_lane_blocks,
    ),
    lane_block_sessions: parseSumeragiLaneBlockSessions(record.lane_block_sessions),
    lane_governance_sealed_total: sealedTotal,
    lane_governance_sealed_aliases: sealedAliases,
    lane_governance: parseSumeragiDiagnosticLaneGovernance(record.lane_governance),
    native_amx_participant_applications:
      parseSumeragiNativeParticipantApplications(
        record.native_amx_participant_applications,
      ),
    autonomous_lane_executions: parseSumeragiAutonomousLaneExecutions(
      record.autonomous_lane_executions,
    ),
  });
}

function parseSumeragiNposDiagnostics(value) {
  const context = "sumeragi diagnostics.npos";
  const fields = [
    "epoch_length_blocks",
    "epoch_seed",
    "prf_height",
    "prf_view",
  ];
  const record = assertExactSumeragiRecord(value, fields, context);
  const epochLength = parseSumeragiUnsigned(
    record.epoch_length_blocks,
    `${context}.epoch_length_blocks`,
    { positive: true },
  );
  const epochSeed = parseSumeragiByteVector(
    record.epoch_seed,
    32,
    `${context}.epoch_seed`,
  );
  if (!epochSeed.some((byte) => byte !== 0)) {
    throw new TypeError(`${context}.epoch_seed must not be zero`);
  }
  return Object.freeze({
    epoch_length_blocks: epochLength,
    epoch_seed: epochSeed,
    prf_height: parseSumeragiUnsigned(record.prf_height, `${context}.prf_height`),
    prf_view: parseSumeragiUnsigned(record.prf_view, `${context}.prf_view`),
  });
}

function parseSumeragiDiagnosticLaneCommitments(value) {
  const context = "sumeragi diagnostics.lane_commitments";
  const fields = [
    "block_height",
    "lane_id",
    "tx_count",
    "total_chunks",
    "rbc_bytes_total",
    "teu_total",
    "block_hash",
  ];
  return Object.freeze(
    assertSumeragiArrayBound(value, 1024, context).map((item, index) => {
      const itemContext = `${context}[${index}]`;
      const record = assertExactSumeragiRecord(item, fields, itemContext);
      return Object.freeze({
        block_height: parseSumeragiUnsigned(
          record.block_height,
          `${itemContext}.block_height`,
        ),
        lane_id: parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, {
          max: 0xffffffff,
        }),
        tx_count: parseSumeragiUnsigned(record.tx_count, `${itemContext}.tx_count`),
        total_chunks: parseSumeragiUnsigned(
          record.total_chunks,
          `${itemContext}.total_chunks`,
        ),
        rbc_bytes_total: parseSumeragiUnsigned(
          record.rbc_bytes_total,
          `${itemContext}.rbc_bytes_total`,
        ),
        teu_total: parseSumeragiUnsigned(record.teu_total, `${itemContext}.teu_total`),
        block_hash: parseSumeragiHash(record.block_hash, `${itemContext}.block_hash`),
      });
    }),
  );
}

function parseSumeragiDiagnosticDataspaceCommitments(value) {
  const context = "sumeragi diagnostics.dataspace_commitments";
  const fields = [
    "block_height",
    "lane_id",
    "dataspace_id",
    "tx_count",
    "total_chunks",
    "rbc_bytes_total",
    "teu_total",
    "block_hash",
  ];
  return Object.freeze(
    assertSumeragiArrayBound(value, 128, context).map((item, index) => {
      const itemContext = `${context}[${index}]`;
      const record = assertExactSumeragiRecord(item, fields, itemContext);
      return Object.freeze({
        block_height: parseSumeragiUnsigned(
          record.block_height,
          `${itemContext}.block_height`,
        ),
        lane_id: parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, {
          max: 0xffffffff,
        }),
        dataspace_id: parseSumeragiUnsigned(
          record.dataspace_id,
          `${itemContext}.dataspace_id`,
        ),
        tx_count: parseSumeragiUnsigned(record.tx_count, `${itemContext}.tx_count`),
        total_chunks: parseSumeragiUnsigned(
          record.total_chunks,
          `${itemContext}.total_chunks`,
        ),
        rbc_bytes_total: parseSumeragiUnsigned(
          record.rbc_bytes_total,
          `${itemContext}.rbc_bytes_total`,
        ),
        teu_total: parseSumeragiUnsigned(record.teu_total, `${itemContext}.teu_total`),
        block_hash: parseSumeragiHash(record.block_hash, `${itemContext}.block_hash`),
      });
    }),
  );
}

function parseSumeragiDiagnosticLaneGovernance(value) {
  const context = "sumeragi diagnostics.lane_governance";
  const fields = [
    "lane_id",
    "alias",
    "governance",
    "manifest_required",
    "manifest_ready",
    "manifest_path",
    "validator_ids",
    "quorum",
    "protected_namespaces",
    "runtime_upgrade",
  ];
  return Object.freeze(
    assertSumeragiArrayBound(value, 128, context).map((item, index) => {
      const itemContext = `${context}[${index}]`;
      const record = assertExactSumeragiRecord(item, fields, itemContext);
      const validatorIds = parseSumeragiDiagnosticStringArray(
        record.validator_ids,
        `${itemContext}.validator_ids`,
      );
      const namespaces = parseSumeragiDiagnosticStringArray(
        record.protected_namespaces,
        `${itemContext}.protected_namespaces`,
      );
      if (new Set(validatorIds).size !== validatorIds.length) {
        throw new TypeError(`${itemContext}.validator_ids contains duplicates`);
      }
      if (new Set(namespaces).size !== namespaces.length) {
        throw new TypeError(`${itemContext}.protected_namespaces contains duplicates`);
      }
      const quorum = record.quorum == null
        ? null
        : parseSumeragiUnsigned(record.quorum, `${itemContext}.quorum`, {
          positive: true,
          max: 0xffffffff,
        });
      if (quorum !== null && quorum > validatorIds.length) {
        throw new RangeError(`${itemContext}.quorum exceeds the validator roster`);
      }
      return Object.freeze({
        lane_id: parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, {
          max: 0xffffffff,
        }),
        alias: requireExactNonEmptyString(record.alias, `${itemContext}.alias`),
        governance: record.governance == null
          ? null
          : requireExactNonEmptyString(record.governance, `${itemContext}.governance`),
        manifest_required: parseSumeragiBoolean(
          record.manifest_required,
          `${itemContext}.manifest_required`,
        ),
        manifest_ready: parseSumeragiBoolean(
          record.manifest_ready,
          `${itemContext}.manifest_ready`,
        ),
        manifest_path: record.manifest_path == null
          ? null
          : requireExactNonEmptyString(
            record.manifest_path,
            `${itemContext}.manifest_path`,
          ),
        validator_ids: validatorIds,
        quorum,
        protected_namespaces: namespaces,
        runtime_upgrade: record.runtime_upgrade == null
          ? null
          : parseSumeragiDiagnosticRuntimeUpgrade(
            record.runtime_upgrade,
            `${itemContext}.runtime_upgrade`,
          ),
      });
    }),
  );
}

function parseSumeragiDiagnosticRuntimeUpgrade(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    ["allow", "require_metadata", "metadata_key", "allowed_ids"],
    context,
  );
  const allowedIds = parseSumeragiDiagnosticStringArray(
    record.allowed_ids,
    `${context}.allowed_ids`,
  );
  if (new Set(allowedIds).size !== allowedIds.length) {
    throw new TypeError(`${context}.allowed_ids contains duplicates`);
  }
  return Object.freeze({
    allow: parseSumeragiBoolean(record.allow, `${context}.allow`),
    require_metadata: parseSumeragiBoolean(
      record.require_metadata,
      `${context}.require_metadata`,
    ),
    metadata_key: record.metadata_key == null
      ? null
      : requireExactNonEmptyString(record.metadata_key, `${context}.metadata_key`),
    allowed_ids: allowedIds,
  });
}

function parseSumeragiDiagnosticStringArray(value, context) {
  return Object.freeze(
    assertSumeragiArrayBound(value, 128, context).map((item, index) =>
      requireExactNonEmptyString(item, `${context}[${index}]`),
    ),
  );
}

function parseSumeragiNativeParticipantApplications(value) {
  const context = "sumeragi diagnostics.native_amx_participant_applications";
  const requiredFields = [
    "lane_id",
    "dataspace_id",
    "lane_incarnation",
    "participant_height",
    "participant_view",
    "predecessor_height",
    "descriptor_hash",
    "proposal_hash",
    "settlement_hash",
    "source_count",
    "state",
  ];
  const optionalFields = [
    "predecessor_descriptor_hash",
    "application_block_height",
    "application_block_hash",
  ];
  let previousKey = null;
  return Object.freeze(
    assertSumeragiArrayBound(value, 1024, context).map((item, index) => {
      const itemContext = `${context}[${index}]`;
      const record = ensureRecord(item, itemContext);
      const allowed = new Set([...requiredFields, ...optionalFields]);
      const unknown = Object.keys(record).find((field) => !allowed.has(field));
      const missing = requiredFields.find(
        (field) => !Object.prototype.hasOwnProperty.call(record, field),
      );
      if (unknown !== undefined || missing !== undefined) {
        throw new TypeError(
          unknown !== undefined
            ? `${itemContext} contains unknown field ${unknown}`
            : `${itemContext} is missing required field ${missing}`,
        );
      }
      const laneId = parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, {
        max: 0xffffffff,
      });
      const dataspaceId = parseSumeragiUnsigned(
        record.dataspace_id,
        `${itemContext}.dataspace_id`,
      );
      const laneIncarnation = parseSumeragiNonzeroHash(
        record.lane_incarnation,
        `${itemContext}.lane_incarnation`,
      );
      const key = [laneId, dataspaceId, laneIncarnation];
      if (previousKey !== null && compareSumeragiDiagnosticRouteKeys(previousKey, key) >= 0) {
        throw new TypeError(
          `${context} must be strictly ordered by route and incarnation`,
        );
      }
      previousKey = key;
      const participantHeight = parseSumeragiUnsigned(
        record.participant_height,
        `${itemContext}.participant_height`,
        { positive: true },
      );
      const predecessorHeight = parseSumeragiUnsigned(
        record.predecessor_height,
        `${itemContext}.predecessor_height`,
      );
      const predecessorHash = record.predecessor_descriptor_hash == null
        ? null
        : parseSumeragiNonzeroHash(
          record.predecessor_descriptor_hash,
          `${itemContext}.predecessor_descriptor_hash`,
        );
      if (
        !sumeragiUnsignedSuccessorOf(participantHeight, predecessorHeight) ||
        (predecessorHeight === 0) !== (predecessorHash === null)
      ) {
        throw new TypeError(`${itemContext} contains inconsistent predecessor geometry`);
      }
      const applicationHeight = record.application_block_height == null
        ? null
        : parseSumeragiUnsigned(
          record.application_block_height,
          `${itemContext}.application_block_height`,
          { positive: true },
        );
      const applicationHash = record.application_block_hash == null
        ? null
        : parseSumeragiNonzeroHash(
          record.application_block_hash,
          `${itemContext}.application_block_hash`,
        );
      if ((applicationHeight === null) !== (applicationHash === null)) {
        throw new TypeError(
          `${itemContext} application block height and hash must appear together`,
        );
      }
      const state = requireExactNonEmptyString(record.state, `${itemContext}.state`);
      const states = new Set([
        "certified_pending_carrier",
        "committed_evidence_pending",
        "durably_applied",
        "conflict",
      ]);
      if (!states.has(state)) {
        throw new TypeError(`${itemContext}.state has an unknown variant`);
      }
      const requiresApplicationBlock = state === "committed_evidence_pending"
        || state === "durably_applied";
      if ((applicationHeight !== null) !== requiresApplicationBlock) {
        throw new TypeError(
          `${itemContext} state and application block identity disagree`,
        );
      }
      return Object.freeze({
        lane_id: laneId,
        dataspace_id: dataspaceId,
        lane_incarnation: laneIncarnation,
        participant_height: participantHeight,
        participant_view: parseSumeragiUnsigned(
          record.participant_view,
          `${itemContext}.participant_view`,
        ),
        predecessor_height: predecessorHeight,
        predecessor_descriptor_hash: predecessorHash,
        descriptor_hash: parseSumeragiNonzeroHash(
          record.descriptor_hash,
          `${itemContext}.descriptor_hash`,
        ),
        proposal_hash: parseSumeragiNonzeroHash(
          record.proposal_hash,
          `${itemContext}.proposal_hash`,
        ),
        settlement_hash: parseSumeragiNonzeroHash(
          record.settlement_hash,
          `${itemContext}.settlement_hash`,
        ),
        source_count: parseSumeragiUnsigned(
          record.source_count,
          `${itemContext}.source_count`,
          { positive: true, max: 4096 },
        ),
        application_block_height: applicationHeight,
        application_block_hash: applicationHash,
        state,
      });
    }),
  );
}

function parseSumeragiAutonomousLaneExecutions(value) {
  const context = "sumeragi diagnostics.autonomous_lane_executions";
  const required = [
    "lane_id", "dataspace_id", "lane_incarnation", "lane_block_height",
    "lane_block_view", "proposal_height",
    "reservation_owner_hash", "proposal_identity_hash", "reservation_group_hash",
    "reservation_count", "transaction_count",
    "highest_durable_stage",
  ];
  const optional = [
    "proposal_view", "proposal_hash", "descriptor_hash",
    "executable_payload_hash", "source_bundle_hash", "merge_entry_hash",
    "application_block_height", "application_block_hash", "stuck_reason",
  ];
  const stages = new Set([
    "reservations_durable", "executable_payload_durable",
    "payload_availability_certified", "lane_certified",
    "certified_bundle_durable", "merge_candidate_durable",
    "global_carrier_committed", "kura_wsv_application_receipt_durable",
    "queue_finalized", "conflict",
  ]);
  const reasons = new Set([
    "awaiting_executable_payload", "awaiting_payload_availability",
    "awaiting_lane_certification",
    "certified_bundle_unavailable", "awaiting_merge_selection",
    "awaiting_global_carrier", "awaiting_application_receipt",
    "queue_finalization_unverifiable", "evidence_conflict",
  ]);
  let previousKey = null;
  return Object.freeze(assertSumeragiArrayBound(value, 128, context).map((item, index) => {
    const itemContext = `${context}[${index}]`;
    const record = ensureRecord(item, itemContext);
    const allowed = new Set([...required, ...optional]);
    const unknown = Object.keys(record).find((field) => !allowed.has(field));
    const missing = required.find(
      (field) => !Object.prototype.hasOwnProperty.call(record, field),
    );
    if (unknown !== undefined || missing !== undefined) {
      throw new TypeError(unknown !== undefined
        ? `${itemContext} contains unknown field ${unknown}`
        : `${itemContext} is missing required field ${missing}`);
    }
    const u64 = (field, options = {}) => parseSumeragiUnsigned(
      record[field], `${itemContext}.${field}`, options,
    );
    const hash = (field) => record[field] == null ? null : parseSumeragiNonzeroHash(
      record[field], `${itemContext}.${field}`,
    );
    const requiredHash = (field) => parseSumeragiNonzeroHash(
      record[field], `${itemContext}.${field}`,
    );
    const laneId = u64("lane_id", { max: 0xffffffff });
    const dataspaceId = u64("dataspace_id");
    const incarnation = requiredHash("lane_incarnation");
    const laneHeight = u64("lane_block_height", { positive: true });
    const laneView = u64("lane_block_view");
    const proposalHeight = u64("proposal_height", { positive: true });
    const proposalView = record.proposal_view == null ? null : u64("proposal_view");
    const reservationOwnerHash = requiredHash("reservation_owner_hash");
    const proposalIdentityHash = requiredHash("proposal_identity_hash");
    const reservationGroupHash = requiredHash("reservation_group_hash");
    const proposalHash = hash("proposal_hash");
    const descriptorHash = hash("descriptor_hash");
    const key = [
      laneId, dataspaceId, incarnation, laneHeight, laneView,
      proposalHeight, proposalIdentityHash,
    ];
    if (previousKey !== null && compareSumeragiDiagnosticKeys(previousKey, key) >= 0) {
      throw new TypeError(`${context} must be strictly ordered by exact identity`);
    }
    previousKey = key;
    const applicationHeight = record.application_block_height == null
      ? null : u64("application_block_height", { positive: true });
    const applicationHash = hash("application_block_hash");
    if ((applicationHeight === null) !== (applicationHash === null)) {
      throw new TypeError(`${itemContext} application block height and hash must appear together`);
    }
    const reservationCount = u64("reservation_count", { max: 4096 });
    const transactionCount = u64("transaction_count", { positive: true, max: 4096 });
    const stage = requireExactNonEmptyString(
      record.highest_durable_stage, `${itemContext}.highest_durable_stage`,
    );
    if (!stages.has(stage)) {
      throw new TypeError(`${itemContext}.highest_durable_stage has an unknown variant`);
    }
    const reason = record.stuck_reason == null ? null : requireExactNonEmptyString(
      record.stuck_reason, `${itemContext}.stuck_reason`,
    );
    if (reason !== null && !reasons.has(reason)) {
      throw new TypeError(`${itemContext}.stuck_reason has an unknown variant`);
    }
    const expectedReasons = {
      reservations_durable: "awaiting_executable_payload",
      executable_payload_durable: "awaiting_payload_availability",
      payload_availability_certified: "awaiting_lane_certification",
      lane_certified: "certified_bundle_unavailable",
      certified_bundle_durable: "awaiting_merge_selection",
      merge_candidate_durable: "awaiting_global_carrier",
      global_carrier_committed: "awaiting_application_receipt",
      kura_wsv_application_receipt_durable: "queue_finalization_unverifiable",
      queue_finalized: null,
      conflict: "evidence_conflict",
    };
    if (reason !== expectedReasons[stage]) {
      throw new TypeError(`${itemContext} stage and stuck reason disagree`);
    }
    if (stage !== "conflict" && reservationCount !== transactionCount) {
      throw new TypeError(`${itemContext} reservation and transaction counts disagree`);
    }
    if ((proposalHash === null) !== (descriptorHash === null)) {
      throw new TypeError(
        `${itemContext} proposal and descriptor hashes must appear together`,
      );
    }
    if (stage !== "conflict"
      && ((stage === "reservations_durable") !== (proposalHash === null))) {
      throw new TypeError(`${itemContext} finalized identity disagrees with durable stage`);
    }
    if (stage === "reservations_durable" && proposalView !== null) {
      throw new TypeError(`${itemContext} proposal view disagrees with durable stage`);
    }
    const payloadHash = hash("executable_payload_hash");
    const bundleHash = hash("source_bundle_hash");
    const mergeHash = hash("merge_entry_hash");
    if (stage !== "conflict") {
      const geometries = {
        reservations_durable: [false, false, false, false],
        executable_payload_durable: [true, false, false, false],
        payload_availability_certified: [true, false, false, false],
        lane_certified: [true, false, false, false],
        certified_bundle_durable: [true, true, false, false],
        merge_candidate_durable: [true, true, true, false],
        global_carrier_committed: [true, true, true, false],
        kura_wsv_application_receipt_durable: [true, true, true, true],
        queue_finalized: [true, true, true, true],
      };
      const observed = [
        payloadHash !== null, bundleHash !== null, mergeHash !== null,
        applicationHeight !== null,
      ];
      if (observed.some((present, offset) => present !== geometries[stage][offset])) {
        throw new TypeError(`${itemContext} evidence does not match durable stage`);
      }
    }
    return Object.freeze({
      lane_id: laneId, dataspace_id: dataspaceId, lane_incarnation: incarnation,
      lane_block_height: laneHeight, lane_block_view: laneView,
      proposal_height: proposalHeight, proposal_view: proposalView,
      reservation_owner_hash: reservationOwnerHash,
      proposal_identity_hash: proposalIdentityHash,
      reservation_group_hash: reservationGroupHash,
      proposal_hash: proposalHash, descriptor_hash: descriptorHash,
      executable_payload_hash: payloadHash,
      source_bundle_hash: bundleHash,
      merge_entry_hash: mergeHash,
      application_block_height: applicationHeight,
      application_block_hash: applicationHash,
      reservation_count: reservationCount, transaction_count: transactionCount,
      highest_durable_stage: stage, stuck_reason: reason,
    });
  }));
}

function compareSumeragiDiagnosticKeys(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] === right[index]) continue;
    return left[index] < right[index] ? -1 : 1;
  }
  return 0;
}

function compareSumeragiDiagnosticRouteKeys(left, right) {
  if (left[0] !== right[0]) {
    return left[0] - right[0];
  }
  if (left[1] !== right[1]) {
    return left[1] < right[1] ? -1 : 1;
  }
  return left[2].localeCompare(right[2]);
}

function parseSumeragiLivenessStatus(value, context, active) {
  const record = ensureRecord(value, context);
  const fields = new Set([
    "generation",
    "prepare_quorums",
    "commit_quorums",
    "timeout_quorums",
    "outbound_intents",
    "work",
    "queues",
    "last_progress",
    "no_progress_age_ms",
    "blocker",
    "ignore_counts",
  ]);
  const unknown = Object.keys(record).find((field) => !fields.has(field));
  if (unknown !== undefined) {
    throw new TypeError(`${context} contains unknown field ${unknown}`);
  }
  for (const field of fields) {
    if (field !== "last_progress" && field !== "blocker" &&
        !Object.prototype.hasOwnProperty.call(record, field)) {
      throw new TypeError(`${context} is missing required field ${field}`);
    }
  }

  const generation = parseSumeragiUnsigned(record.generation, `${context}.generation`);
  const boundRound = (raw, roundContext) => {
    const round = parseSumeragiRound(raw, roundContext);
    if (
      round.context_id[0] !== active.contextId[0] ||
      round.height !== active.height
    ) {
      throw new TypeError(`${roundContext} must match the active height context`);
    }
    return round;
  };
  const checkedRound = (raw, roundContext) => {
    const round = boundRound(raw, roundContext);
    if (round.view > active.view) {
      throw new RangeError(`${roundContext}.view must not exceed the active view`);
    }
    return round;
  };
  const checkedPartialQuorum = (
    raw,
    itemContext,
    { timeout = false, phase = null } = {},
  ) => {
    const expectedFields = timeout
      ? [
          "round",
          "signer_count",
          "signed_power",
          "min_signers",
          "total_power",
          "certificate_formed",
        ]
      : [
          "round",
          "proposal_round",
          "subject",
          "execution_commitment",
          "signer_count",
          "signed_power",
          "min_signers",
          "total_power",
        ];
    const item = assertExactSumeragiRecord(raw, expectedFields, itemContext);
    const signerCount = parseSumeragiUnsigned(
      item.signer_count,
      `${itemContext}.signer_count`,
      { max: active.heightContext.validator_count },
    );
    const signedPower = parseSumeragiUnsigned(
      item.signed_power,
      `${itemContext}.signed_power`,
    );
    const minSigners = parseSumeragiUnsigned(
      item.min_signers,
      `${itemContext}.min_signers`,
      { max: active.heightContext.validator_count },
    );
    const totalPower = parseSumeragiUnsigned(
      item.total_power,
      `${itemContext}.total_power`,
      { positive: true },
    );
    if (
      minSigners !== active.heightContext.quorum.min_signers ||
      totalPower !== active.heightContext.quorum.total_power ||
      signedPower !== signerCount
    ) {
      throw new RangeError(`${itemContext} disagrees with the frozen dual quorum`);
    }
    const round = checkedRound(item.round, `${itemContext}.round`);
    if (timeout) {
      const certificateFormed = parseSumeragiBoolean(
        item.certificate_formed,
        `${itemContext}.certificate_formed`,
      );
      if (
        certificateFormed &&
        (signerCount < minSigners || BigInt(signedPower) * 3n <= BigInt(totalPower) * 2n)
      ) {
        throw new RangeError(`${itemContext} does not form its advertised dual quorum`);
      }
      return Object.freeze({
        round,
        signer_count: signerCount,
        signed_power: signedPower,
        min_signers: minSigners,
        total_power: totalPower,
        certificate_formed: certificateFormed,
      });
    }
    const proposalRound = checkedRound(
      item.proposal_round,
      `${itemContext}.proposal_round`,
    );
    validateSumeragiProposalRound(proposalRound, round, itemContext);
    return Object.freeze({
      round,
      proposal_round: proposalRound,
      subject: parseSumeragiBlockSubject(item.subject, `${itemContext}.subject`),
      execution_commitment: parseSumeragiExecutionCommitment(
        item.execution_commitment,
        `${itemContext}.execution_commitment`,
      ),
      signer_count: signerCount,
      signed_power: signedPower,
      min_signers: minSigners,
      total_power: totalPower,
    });
  };
  const voteQuorums = (field, phase) => Object.freeze(
    assertSumeragiArrayBound(
      record[field],
      phase === "commit" ? 32 : 31,
      `${context}.${field}`,
    ).map(
      (item, index) => checkedPartialQuorum(
        item,
        `${context}.${field}[${index}]`,
        { phase },
      ),
    ),
  );
  const timeoutQuorums = Object.freeze(
    assertSumeragiArrayBound(
      record.timeout_quorums,
      31,
      `${context}.timeout_quorums`,
    ).map((item, index) => checkedPartialQuorum(
      item,
      `${context}.timeout_quorums[${index}]`,
      { timeout: true },
    )),
  );

  const subjectKinds = new Set([
    "proposal",
    "prepare_vote",
    "commit_vote",
    "prepare_qc",
    "commit_qc",
  ]);
  const outboundIntents = Object.freeze(
    assertSumeragiArrayBound(
      record.outbound_intents,
      7,
      `${context}.outbound_intents`,
    ).map((raw, index) => {
      const itemContext = `${context}.outbound_intents[${index}]`;
      const item = ensureRecord(raw, itemContext);
      const allowedFields = new Set([
        "kind",
        "round",
        "proposal_round",
        "subject",
        "execution_commitment",
        "stage",
      ]);
      const unknownField = Object.keys(item).find((field) => !allowedFields.has(field));
      if (unknownField !== undefined) {
        throw new TypeError(`${itemContext} contains unknown field ${unknownField}`);
      }
      for (const field of ["kind", "round", "stage"]) {
        if (!Object.prototype.hasOwnProperty.call(item, field)) {
          throw new TypeError(`${itemContext} is missing required field ${field}`);
        }
      }
      const kind = parseSumeragiTaggedUnit(
        item.kind,
        "kind",
        [
          "proposal",
          "prepare_vote",
          "commit_vote",
          "timeout_vote",
          "prepare_qc",
          "commit_qc",
          "timeout_certificate",
        ],
        `${itemContext}.kind`,
      );
      const stage = parseSumeragiTaggedUnit(
        item.stage,
        "stage",
        ["pending_persistence", "pending_signature", "queued", "sent"],
        `${itemContext}.stage`,
      );
      const subject = item.subject == null
        ? null
        : parseSumeragiBlockSubject(item.subject, `${itemContext}.subject`);
      const executionCommitment = item.execution_commitment == null
        ? null
        : parseSumeragiExecutionCommitment(
            item.execution_commitment,
            `${itemContext}.execution_commitment`,
          );
      const carriesProposalRound = subjectKinds.has(kind.kind);
      if (carriesProposalRound !== (item.proposal_round != null)) {
        throw new TypeError(`${itemContext} has inconsistent proposal_round for ${kind.kind}`);
      }
      const shapeIsValid =
        (kind.kind === "proposal" && subject !== null && executionCommitment === null) ||
        (subjectKinds.has(kind.kind) && kind.kind !== "proposal" &&
          subject !== null && executionCommitment !== null) ||
        (!subjectKinds.has(kind.kind) && subject === null && executionCommitment === null);
      if (!shapeIsValid) {
        throw new TypeError(`${itemContext} has inconsistent proposal fields`);
      }
      const round = boundRound(item.round, `${itemContext}.round`);
      if (kind.kind !== "commit_qc" && round.view > active.view) {
        throw new RangeError(`${itemContext}.round.view must not exceed the active view`);
      }
      const proposalRound = item.proposal_round == null
        ? null
        : boundRound(item.proposal_round, `${itemContext}.proposal_round`);
      if (proposalRound !== null) {
        validateSumeragiProposalRound(proposalRound, round, itemContext);
      }
      return Object.freeze({
        kind,
        round,
        proposal_round: proposalRound,
        subject,
        execution_commitment: executionCommitment,
        stage,
      });
    }),
  );

  const workRecord = assertExactSumeragiRecord(
    record.work,
    ["candidate", "body_recovery", "body_store", "validation", "application", "successor_height"],
    `${context}.work`,
  );
  const work = Object.freeze(Object.fromEntries(
    Object.keys(workRecord).map((field) => [
      field,
      parseSumeragiTaggedUnit(
        workRecord[field],
        "stage",
        ["idle", "queued", "running", "complete"],
        `${context}.work.${field}`,
      ),
    ]),
  ));

  const queueNames = new Set();
  const queues = Object.freeze(
    assertSumeragiArrayBound(record.queues, 10, `${context}.queues`).map((raw, index) => {
      const itemContext = `${context}.queues[${index}]`;
      const item = ensureRecord(raw, itemContext);
      const allowedFields = new Set([
        "queue",
        "depth",
        "capacity",
        "oldest_age_ms",
        "service_debt",
      ]);
      const unknownField = Object.keys(item).find((field) => !allowedFields.has(field));
      if (unknownField !== undefined) {
        throw new TypeError(`${itemContext} contains unknown field ${unknownField}`);
      }
      for (const field of ["queue", "depth", "capacity", "service_debt"]) {
        if (!Object.prototype.hasOwnProperty.call(item, field)) {
          throw new TypeError(`${itemContext} is missing required field ${field}`);
        }
      }
      const queue = parseSumeragiTaggedUnit(
        item.queue,
        "queue",
        [
          "ingress",
          "deferred_normal",
          "deferred_progress",
          "deferred_completion",
          "runtime_normal",
          "runtime_progress",
          "runtime_completion",
          "effect_completion",
          "network_ingress",
          "effect_dispatch",
        ],
        `${itemContext}.queue`,
      );
      if (queueNames.has(queue.queue)) {
        throw new TypeError(`${itemContext}.queue is duplicated`);
      }
      queueNames.add(queue.queue);
      const depth = parseSumeragiUnsigned(item.depth, `${itemContext}.depth`, {
        max: 0xffffffff,
      });
      const capacity = parseSumeragiUnsigned(item.capacity, `${itemContext}.capacity`, {
        positive: true,
        max: 0xffffffff,
      });
      const oldestAge = item.oldest_age_ms == null
        ? null
        : parseSumeragiUnsigned(item.oldest_age_ms, `${itemContext}.oldest_age_ms`);
      if (depth > capacity || ((depth === 0) !== (oldestAge === null))) {
        throw new RangeError(`${itemContext} has inconsistent occupancy and age`);
      }
      return Object.freeze({
        queue,
        depth,
        capacity,
        oldest_age_ms: oldestAge,
        service_debt: parseSumeragiUnsigned(
          item.service_debt,
          `${itemContext}.service_debt`,
        ),
      });
    }),
  );

  let lastProgress = null;
  if (record.last_progress != null) {
    const progress = assertExactSumeragiRecord(
      record.last_progress,
      ["generation", "round", "transition", "age_ms"],
      `${context}.last_progress`,
    );
    const progressGeneration = parseSumeragiUnsigned(
      progress.generation,
      `${context}.last_progress.generation`,
    );
    if (progressGeneration > generation) {
      throw new RangeError(`${context}.last_progress.generation is from the future`);
    }
    lastProgress = Object.freeze({
      generation: progressGeneration,
      round: checkedRound(progress.round, `${context}.last_progress.round`),
      transition: parseSumeragiTaggedUnit(
        progress.transition,
        "transition",
        [
          "proposal_admitted",
          "body_available",
          "body_stored",
          "body_validated",
          "prepare_vote_admitted",
          "commit_vote_admitted",
          "timeout_vote_admitted",
          "prepare_quorum",
          "lock_installed",
          "commit_quorum",
          "timeout_certificate_installed",
          "decision_persisted",
          "applied",
          "successor_height_activated",
          "recovery_replayed",
        ],
        `${context}.last_progress.transition`,
      ),
      age_ms: parseSumeragiUnsigned(progress.age_ms, `${context}.last_progress.age_ms`),
    });
  }
  const blocker = record.blocker == null
    ? null
    : parseSumeragiTaggedUnit(
        record.blocker,
        "blocker",
        [
          "missing_proposal",
          "body_unavailable",
          "prepare_quorum_missing",
          "commit_quorum_missing",
          "timeout_certificate_missing",
          "scheduler_starvation",
          "application_pending",
          "successor_activation_pending",
          "local_control_pending",
        ],
        `${context}.blocker`,
      );

  const ignoreReasons = new Set();
  const ignoreCounts = Object.freeze(
    assertSumeragiArrayBound(record.ignore_counts, 12, `${context}.ignore_counts`).map(
      (raw, index) => {
        const itemContext = `${context}.ignore_counts[${index}]`;
        const item = assertExactSumeragiRecord(raw, ["reason", "count"], itemContext);
        const reason = parseSumeragiTaggedUnit(
          item.reason,
          "reason",
          [
            "wrong_height",
            "wrong_view",
            "stale_generation",
            "busy",
            "duplicate",
            "no_matching_work",
            "observer",
            "view_closed",
            "already_decided",
            "recovery_pending",
            "irrelevant_view",
            "unsafe_proposal",
          ],
          `${itemContext}.reason`,
        );
        if (ignoreReasons.has(reason.reason)) {
          throw new TypeError(`${itemContext}.reason is duplicated`);
        }
        ignoreReasons.add(reason.reason);
        return Object.freeze({
          reason,
          count: parseSumeragiUnsigned(item.count, `${itemContext}.count`),
        });
      },
    ),
  );

  return Object.freeze({
    generation,
    prepare_quorums: voteQuorums("prepare_quorums", "prepare"),
    commit_quorums: voteQuorums("commit_quorums", "commit"),
    timeout_quorums: timeoutQuorums,
    outbound_intents: outboundIntents,
    work,
    queues,
    last_progress: lastProgress,
    no_progress_age_ms: parseSumeragiUnsigned(
      record.no_progress_age_ms,
      `${context}.no_progress_age_ms`,
    ),
    blocker,
    ignore_counts: ignoreCounts,
  });
}

const SUMERAGI_U64_MAX = (1n << 64n) - 1n;

function parseSumeragiUnsigned(value, context, options = {}) {
  if (
    (
      typeof value !== "number" ||
      !Number.isSafeInteger(value) ||
      Object.is(value, -0)
    ) &&
    typeof value !== "bigint"
  ) {
    throw new TypeError(`${context} must be an unsigned integer`);
  }
  if (value < 0) {
    throw new RangeError(`${context} must be >= 0`);
  }
  const integer = BigInt(value);
  if (options.positive === true && integer === 0n) {
    throw new RangeError(`${context} must be positive`);
  }
  const maximum =
    options.max === undefined ? SUMERAGI_U64_MAX : BigInt(options.max);
  if (integer > maximum) {
    throw new RangeError(`${context} exceeds its protocol bound`);
  }
  if (options.max !== undefined) {
    if (maximum > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new TypeError(`${context} has an invalid narrow protocol bound`);
    }
    return Number(integer);
  }
  return value;
}

function sumeragiUnsignedSuccessorOf(successor, predecessor) {
  return BigInt(predecessor) + 1n === BigInt(successor);
}

function sumeragiRoundsEqual(left, right) {
  return (
    left.height === right.height &&
    left.view === right.view &&
    left.context_id.length === right.context_id.length &&
    left.context_id.every((entry, index) => entry === right.context_id[index])
  );
}

function parseSumeragiExactUnsigned(value, context, options = {}) {
  if (typeof value !== "number" || !Number.isSafeInteger(value)) {
    throw new TypeError(`${context} must be an unsigned integer`);
  }
  return parseSumeragiUnsigned(value, context, options);
}

function parseSumeragiQuantity(value, context) {
  return requireCanonicalQuantity(value, context);
}

function parseSumeragiBoolean(value, context) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${context} must be a boolean`);
  }
  return value;
}

function parseSumeragiHash(value, context) {
  if (typeof value !== "string" || !/^hash:[0-9A-F]{64}#[0-9A-F]{4}$/u.test(value)) {
    throw new TypeError(`${context} must be a canonical Iroha hash literal`);
  }
  const body = parseHashLiteralToHex(value, context);
  if ((Number.parseInt(body.slice(-2), 16) & 1) === 0) {
    throw new TypeError(`${context} has an invalid Iroha hash marker bit`);
  }
  return value;
}

function parseSumeragiOptionalByte32(value, context) {
  if (value == null) {
    return null;
  }
  return parseSumeragiByte32(value, context);
}

function parseSumeragiByte32(value, context) {
  if (typeof value !== "string" || !/^[0-9A-F]{64}$/u.test(value)) {
    throw new TypeError(`${context} must be canonical uppercase 32-byte hex`);
  }
  return value;
}

function parseSumeragiByteVector(value, length, context) {
  if (!Array.isArray(value) || value.length !== length) {
    throw new TypeError(`${context} must contain exactly ${length} byte values`);
  }
  return Object.freeze(
    value.map((byte, index) => {
      if (!Number.isInteger(byte) || byte < 0 || byte > 0xff) {
        throw new TypeError(`${context}[${index}] must be an integer byte`);
      }
      return byte;
    }),
  );
}

function assertSumeragiArrayBound(value, maximum, context, minimum = 0) {
  if (!Array.isArray(value)) {
    throw new TypeError(`${context} must be an array`);
  }
  if (value.length < minimum) {
    throw new RangeError(`${context} contains fewer than ${minimum} items`);
  }
  if (value.length > maximum) {
    throw new RangeError(`${context} exceeds its protocol item bound`);
  }
  return value;
}

function assertExactSumeragiRecord(value, fields, context) {
  const record = ensureRecord(value, context);
  const expected = new Set(fields);
  for (const field of Object.keys(record)) {
    if (!expected.has(field)) {
      throw new TypeError(`${context} contains unknown field ${field}`);
    }
  }
  for (const field of fields) {
    if (!Object.prototype.hasOwnProperty.call(record, field)) {
      throw new TypeError(`${context} is missing required field ${field}`);
    }
  }
  return record;
}

function parseSumeragiContextId(value, context) {
  if (!Array.isArray(value) || value.length !== 1) {
    throw new TypeError(`${context} must be a one-element hash tuple`);
  }
  return Object.freeze([parseSumeragiHash(value[0], `${context}[0]`)]);
}

function parseSumeragiTaggedUnit(value, tag, allowed, context) {
  return parseSumeragiTaggedUnitWithContent(
    value,
    tag,
    "details",
    allowed,
    context,
  );
}

function parseSumeragiTaggedUnitWithContent(value, tag, content, allowed, context) {
  const record = ensureRecord(value, context);
  if (Object.keys(record).some((field) => field !== tag && field !== content)) {
    throw new TypeError(`${context} contains an unknown tagged-enum field`);
  }
  const variant = requireNonEmptyString(record[tag], `${context}.${tag}`);
  if (!allowed.includes(variant)) {
    throw new TypeError(`${context}.${tag} is not a supported v2 variant`);
  }
  if (!Object.prototype.hasOwnProperty.call(record, content) || record[content] !== null) {
    throw new TypeError(`${context}.${content} must be explicitly null`);
  }
  return Object.freeze({ [tag]: variant, [content]: null });
}

function parseSumeragiRound(value, context) {
  const record = ensureRecord(value, context);
  return Object.freeze({
    context_id: parseSumeragiContextId(record.context_id, `${context}.context_id`),
    height: parseSumeragiUnsigned(record.height, `${context}.height`),
    view: parseSumeragiUnsigned(record.view, `${context}.view`),
  });
}

function validateSumeragiProposalRound(proposalRound, round, context) {
  if (
    proposalRound.context_id[0] !== round.context_id[0] ||
    proposalRound.height !== round.height
  ) {
    throw new TypeError(`${context}.proposal_round must match round context and height`);
  }
  if (proposalRound.view !== round.view) {
    throw new TypeError(`${context}.proposal_round must equal round`);
  }
}

function parseSumeragiBlockSubject(value, context) {
  const record = ensureRecord(value, context);
  return Object.freeze({
    parent_block_hash:
      record.parent_block_hash == null
        ? null
        : parseSumeragiHash(record.parent_block_hash, `${context}.parent_block_hash`),
    block_hash: parseSumeragiHash(record.block_hash, `${context}.block_hash`),
    payload_hash: parseSumeragiHash(record.payload_hash, `${context}.payload_hash`),
  });
}

const SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION = 1;

const SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES = 1024;

const SUMERAGI_LANE_FINALITY_MANIFEST_MAX_LEAVES = 1024;

const SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION = 1;

const SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT =
  "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F";

function parseSumeragiExecutionCommitment(value, context) {
  const record = ensureRecord(value, context);
  const allowedFields = new Set([
    "parent_state_root",
    "post_state_root",
    "ordinary_writes_root",
    "topup_anchor_root",
    "topup_anchor_count",
    "native_amx_application_manifest_version",
    "native_amx_application_manifest_root",
    "native_amx_application_manifest_count",
    "lane_finality_manifest",
    "merge_carrier",
    "executed_block_wire_len",
    "executed_block_wire_hash",
  ]);
  const unknown = Object.keys(record).find((field) => !allowedFields.has(field));
  if (unknown !== undefined) {
    throw new TypeError(`${context} contains unknown field ${unknown}`);
  }
  for (const field of ["lane_finality_manifest", "merge_carrier"]) {
    if (!Object.prototype.hasOwnProperty.call(record, field)) {
      throw new TypeError(`${context}.${field} is required`);
    }
  }
  const topupAnchorCount = parseSumeragiUnsigned(
    record.topup_anchor_count,
    `${context}.topup_anchor_count`,
    { max: 16 },
  );
  const topupAnchorRoot =
    record.topup_anchor_root == null
      ? null
      : parseSumeragiHash(record.topup_anchor_root, `${context}.topup_anchor_root`);
  if ((topupAnchorCount === 0) !== (topupAnchorRoot === null)) {
    throw new TypeError(
      `${context}.topup_anchor_root must be present exactly when topup_anchor_count is positive`,
    );
  }
  const nativeManifestVersion = parseSumeragiUnsigned(
    record.native_amx_application_manifest_version,
    `${context}.native_amx_application_manifest_version`,
    { max: 0xffff },
  );
  if (nativeManifestVersion !== SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION) {
    throw new RangeError(
      `${context}.native_amx_application_manifest_version must equal ${SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_VERSION}`,
    );
  }
  const nativeManifestRoot = parseSumeragiHash(
    record.native_amx_application_manifest_root,
    `${context}.native_amx_application_manifest_root`,
  );
  const nativeManifestCount = parseSumeragiUnsigned(
    record.native_amx_application_manifest_count,
    `${context}.native_amx_application_manifest_count`,
    { max: SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES },
  );
  if (
    (nativeManifestCount === 0) !==
    (nativeManifestRoot === SUMERAGI_NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT)
  ) {
    throw new RangeError(
      `${context}.native_amx_application_manifest_count must be zero exactly for the canonical empty root`,
    );
  }
  let laneFinalityManifest = null;
  if (record.lane_finality_manifest !== null) {
    const laneContext = `${context}.lane_finality_manifest`;
    const laneRecord = ensureRecord(record.lane_finality_manifest, laneContext);
    const laneFields = new Set(["root", "leaf_count"]);
    const missingLaneField = [...laneFields].find(
      (field) => !Object.prototype.hasOwnProperty.call(laneRecord, field),
    );
    if (missingLaneField !== undefined) {
      throw new TypeError(`${laneContext}.${missingLaneField} is required`);
    }
    const unknownLaneField = Object.keys(laneRecord).find(
      (field) => !laneFields.has(field),
    );
    if (unknownLaneField !== undefined) {
      throw new TypeError(`${laneContext} contains unknown field ${unknownLaneField}`);
    }
    laneFinalityManifest = Object.freeze({
      root: parseSumeragiHash(laneRecord.root, `${laneContext}.root`),
      leaf_count: parseSumeragiUnsigned(
        laneRecord.leaf_count,
        `${laneContext}.leaf_count`,
        { max: SUMERAGI_LANE_FINALITY_MANIFEST_MAX_LEAVES, positive: true },
      ),
    });
  }
  let mergeCarrier = null;
  if (record.merge_carrier !== null) {
    const mergeContext = `${context}.merge_carrier`;
    const mergeRecord = ensureRecord(record.merge_carrier, mergeContext);
    const mergeFields = new Set(["version", "entry_hash"]);
    const missingMergeField = [...mergeFields].find(
      (field) => !Object.prototype.hasOwnProperty.call(mergeRecord, field),
    );
    if (missingMergeField !== undefined) {
      throw new TypeError(`${mergeContext}.${missingMergeField} is required`);
    }
    const unknownMergeField = Object.keys(mergeRecord).find(
      (field) => !mergeFields.has(field),
    );
    if (unknownMergeField !== undefined) {
      throw new TypeError(`${mergeContext} contains unknown field ${unknownMergeField}`);
    }
    const mergeVersion = parseSumeragiUnsigned(
      mergeRecord.version,
      `${mergeContext}.version`,
      { max: 0xffff },
    );
    if (mergeVersion !== SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION) {
      throw new RangeError(
        `${mergeContext}.version must equal ${SUMERAGI_MERGE_CARRIER_COMMITMENT_VERSION}`,
      );
    }
    mergeCarrier = Object.freeze({
      version: mergeVersion,
      entry_hash: parseSumeragiHash(
        mergeRecord.entry_hash,
        `${mergeContext}.entry_hash`,
      ),
    });
  }
  return Object.freeze({
    parent_state_root: parseSumeragiHash(
      record.parent_state_root,
      `${context}.parent_state_root`,
    ),
    post_state_root: parseSumeragiHash(
      record.post_state_root,
      `${context}.post_state_root`,
    ),
    ordinary_writes_root: parseSumeragiHash(
      record.ordinary_writes_root,
      `${context}.ordinary_writes_root`,
    ),
    topup_anchor_root: topupAnchorRoot,
    topup_anchor_count: topupAnchorCount,
    native_amx_application_manifest_version: nativeManifestVersion,
    native_amx_application_manifest_root: nativeManifestRoot,
    native_amx_application_manifest_count: nativeManifestCount,
    lane_finality_manifest: laneFinalityManifest,
    merge_carrier: mergeCarrier,
    executed_block_wire_len: parseSumeragiUnsigned(
      record.executed_block_wire_len,
      `${context}.executed_block_wire_len`,
      { positive: true },
    ),
    executed_block_wire_hash: parseSumeragiHash(
      record.executed_block_wire_hash,
      `${context}.executed_block_wire_hash`,
    ),
  });
}

function parseSumeragiQcReference(value, context) {
  const record = assertExactSumeragiRecord(
    value,
    ["round", "proposal_round", "phase", "subject", "execution_commitment"],
    context,
  );
  const round = parseSumeragiRound(record.round, `${context}.round`);
  const proposalRound = parseSumeragiRound(
    record.proposal_round,
    `${context}.proposal_round`,
  );
  const phase = parseSumeragiTaggedUnit(
    record.phase,
    "phase",
    ["prepare", "commit"],
    `${context}.phase`,
  );
  validateSumeragiProposalRound(proposalRound, round, context);
  return Object.freeze({
    round,
    proposal_round: proposalRound,
    phase,
    subject: parseSumeragiBlockSubject(record.subject, `${context}.subject`),
    execution_commitment: parseSumeragiExecutionCommitment(
      record.execution_commitment,
      `${context}.execution_commitment`,
    ),
  });
}

export function parseSumeragiV2QcResponse(payload) {
  const context = "sumeragi qc response";
  const record = ensureRecord(payload, context);
  const allowedFields = new Set(["highest_prepare_qc", "locked_prepare_qc"]);
  const unknownField = Object.keys(record).find((field) => !allowedFields.has(field));
  if (unknownField !== undefined) {
    throw new TypeError(`${context} contains unknown field ${unknownField}`);
  }
  const missingField = [...allowedFields].find(
    (field) => !Object.prototype.hasOwnProperty.call(record, field),
  );
  if (missingField !== undefined) {
    throw new TypeError(`${context}.${missingField} is required`);
  }

  const prepareQc = (field) => {
    if (record[field] == null) {
      return null;
    }
    const certificate = parseSumeragiQcReference(
      record[field],
      `${context}.${field}`,
    );
    if (certificate.phase.phase !== "prepare") {
      throw new TypeError(`${context}.${field} must reference a PrepareQC`);
    }
    return certificate;
  };

  return Object.freeze({
    highest_prepare_qc: prepareQc("highest_prepare_qc"),
    locked_prepare_qc: prepareQc("locked_prepare_qc"),
  });
}

function parseSumeragiTimeoutReference(value, context) {
  const record = ensureRecord(value, context);
  return Object.freeze({
    round: parseSumeragiRound(record.round, `${context}.round`),
    highest_prepare_qc:
      record.highest_prepare_qc == null
        ? null
        : parseSumeragiQcReference(
            record.highest_prepare_qc,
            `${context}.highest_prepare_qc`,
          ),
    certificate_hash: parseSumeragiHash(
      record.certificate_hash,
      `${context}.certificate_hash`,
    ),
  });
}

function parseSumeragiHeightContext(value, context) {
  const record = ensureRecord(value, context);
  const validatorCount = parseSumeragiUnsigned(
    record.validator_count,
    `${context}.validator_count`,
    { positive: true, max: 31 },
  );
  const quorumRecord = ensureRecord(record.quorum, `${context}.quorum`);
  const quorum = Object.freeze({
    min_signers: parseSumeragiUnsigned(
      quorumRecord.min_signers,
      `${context}.quorum.min_signers`,
      { positive: true, max: 31 },
    ),
    total_power: parseSumeragiUnsigned(
      quorumRecord.total_power,
      `${context}.quorum.total_power`,
      { positive: true },
    ),
  });
  const expectedMinSigners = Math.floor((validatorCount * 2) / 3) + 1;
  if (
    validatorCount < 4 ||
    (validatorCount - 1) % 3 !== 0 ||
    quorum.min_signers !== expectedMinSigners ||
    quorum.total_power !== validatorCount
  ) {
    throw new RangeError(`${context}.quorum is not canonical for validator_count`);
  }
  const mode = parseSumeragiTaggedUnit(
    record.mode,
    "mode",
    ["permissioned", "npos"],
    `${context}.mode`,
  );
  const epochSeed = parseSumeragiByte32(record.epoch_seed, `${context}.epoch_seed`);
  return Object.freeze({
    epoch: parseSumeragiUnsigned(record.epoch, `${context}.epoch`),
    epoch_end_height: parseSumeragiUnsigned(
      record.epoch_end_height,
      `${context}.epoch_end_height`,
    ),
    mode,
    epoch_seed: epochSeed,
    validator_count: validatorCount,
    quorum,
  });
}

function parseSumeragiCommitQcStatus(value, context) {
  const record = ensureRecord(value, context);
  const validatorCount = parseSumeragiUnsigned(
    record.validator_count,
    `${context}.validator_count`,
    { positive: true, max: 31 },
  );
  const signerCount = parseSumeragiUnsigned(
    record.signer_count,
    `${context}.signer_count`,
    { max: validatorCount },
  );
  const minSigners = parseSumeragiUnsigned(
    record.min_signers,
    `${context}.min_signers`,
    { positive: true, max: 31 },
  );
  const signedPower = parseSumeragiUnsigned(
    record.signed_power,
    `${context}.signed_power`,
  );
  const totalPower = parseSumeragiUnsigned(
    record.total_power,
    `${context}.total_power`,
    { positive: true },
  );
  if (
    validatorCount < 4 ||
    (validatorCount - 1) % 3 !== 0 ||
    signerCount > validatorCount ||
    minSigners !== Math.floor((validatorCount * 2) / 3) + 1 ||
    signedPower !== signerCount ||
    totalPower !== validatorCount ||
    signerCount !== minSigners ||
    BigInt(signedPower) * 3n <= BigInt(totalPower) * 2n
  ) {
    throw new RangeError(`${context} does not satisfy its exact frozen certificate quorum`);
  }
  return Object.freeze({
    certificate: parseSumeragiQcReference(record.certificate, `${context}.certificate`),
    validator_count: validatorCount,
    signer_count: signerCount,
    min_signers: minSigners,
    signed_power: signedPower,
    total_power: totalPower,
  });
}

function parseSumeragiLanePayloadOwnerships(value) {
  const context = "status.lane_payload_ownerships";
  assertSumeragiArrayBound(value, 128, context);
  return Object.freeze(value.map((entry, index) => {
    const itemContext = `${context}[${index}]`;
    const record = assertExactSumeragiRecord(
      entry,
      [
        "proposal_height",
        "proposal_view",
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "lane_block_height",
        "lane_block_view",
        "subject_hash",
        "qc_mode_tag",
        "accepted_candidate_indices",
        "accepted_transaction_hashes",
        "previous_lane_block_height",
        "previous_lane_block_descriptor_hash",
        "lane_block_descriptor_hash",
        "lane_block_descriptor_validator_set",
        "lane_block_descriptor_validator_count",
        "lane_block_descriptor_min_quorum",
        "payload_ownership_hash",
        "rbc_instance_hash",
      ],
      itemContext,
    );
    const laneBlockHeight = parseSumeragiUnsigned(
      record.lane_block_height,
      `${itemContext}.lane_block_height`,
      { positive: true },
    );
    if (!Array.isArray(record.accepted_candidate_indices)) {
      throw new TypeError(`${itemContext}.accepted_candidate_indices must be an array`);
    }
    const acceptedCandidateIndices = record.accepted_candidate_indices.map((candidate, offset) =>
      parseSumeragiUnsigned(candidate, `${itemContext}.accepted_candidate_indices[${offset}]`),
    );
    if (acceptedCandidateIndices.length === 0) {
      throw new TypeError(`${itemContext}.accepted_candidate_indices must not be empty`);
    }
    for (let offset = 1; offset < acceptedCandidateIndices.length; offset += 1) {
      if (acceptedCandidateIndices[offset - 1] >= acceptedCandidateIndices[offset]) {
        throw new TypeError(`${itemContext}.accepted_candidate_indices must be strictly ordered`);
      }
    }
    if (!Array.isArray(record.accepted_transaction_hashes)) {
      throw new TypeError(`${itemContext}.accepted_transaction_hashes must be an array`);
    }
    const acceptedTransactionHashes = record.accepted_transaction_hashes.map((hash, offset) =>
      parseSumeragiHash(hash, `${itemContext}.accepted_transaction_hashes[${offset}]`),
    );
    if (acceptedTransactionHashes.length !== acceptedCandidateIndices.length) {
      throw new TypeError(`${itemContext} candidate/hash counts must match`);
    }
    const validators = assertSumeragiArrayBound(
      record.lane_block_descriptor_validator_set,
      128,
      `${itemContext}.lane_block_descriptor_validator_set`,
      1,
    ).map((peer, offset) =>
      requireExactNonEmptyString(
        peer,
        `${itemContext}.lane_block_descriptor_validator_set[${offset}]`,
      ),
    );
    if (
      new Set(validators).size !== validators.length ||
      validators.some((validator, offset) => offset > 0 && validators[offset - 1] >= validator)
    ) {
      throw new TypeError(
        `${itemContext}.lane_block_descriptor_validator_set must be canonical and unique`,
      );
    }
    const validatorCount = parseSumeragiUnsigned(
      record.lane_block_descriptor_validator_count,
      `${itemContext}.lane_block_descriptor_validator_count`,
      { positive: true, max: 128 },
    );
    const minQuorum = parseSumeragiUnsigned(
      record.lane_block_descriptor_min_quorum,
      `${itemContext}.lane_block_descriptor_min_quorum`,
      { positive: true, max: 128 },
    );
    if (validatorCount !== validators.length || minQuorum > validatorCount) {
      throw new RangeError(`${itemContext} descriptor quorum does not match its validator set`);
    }
    const previousHeight = parseSumeragiUnsigned(
      record.previous_lane_block_height,
      `${itemContext}.previous_lane_block_height`,
    );
    if (!sumeragiUnsignedSuccessorOf(laneBlockHeight, previousHeight)) {
      throw new RangeError(`${itemContext}.previous_lane_block_height must precede lane_block_height`);
    }
    const previousDescriptor =
      record.previous_lane_block_descriptor_hash == null
        ? null
        : parseSumeragiHash(
            record.previous_lane_block_descriptor_hash,
            `${itemContext}.previous_lane_block_descriptor_hash`,
          );
    if (previousHeight === 0 && previousDescriptor !== null) {
      throw new TypeError(`${itemContext} genesis lane block must not name a predecessor descriptor`);
    }
    if (record.lane_block_descriptor_hash == null) {
      throw new TypeError(`${itemContext}.lane_block_descriptor_hash is required`);
    }
    return Object.freeze({
      proposal_height: parseSumeragiUnsigned(record.proposal_height, `${itemContext}.proposal_height`),
      proposal_view: parseSumeragiUnsigned(record.proposal_view, `${itemContext}.proposal_view`),
      lane_id: parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, { max: 0xffffffff }),
      dataspace_id: parseSumeragiUnsigned(record.dataspace_id, `${itemContext}.dataspace_id`),
      lane_incarnation: parseSumeragiNonzeroHash(record.lane_incarnation, `${itemContext}.lane_incarnation`),
      lane_block_height: laneBlockHeight,
      lane_block_view: parseSumeragiUnsigned(record.lane_block_view, `${itemContext}.lane_block_view`),
      subject_hash: parseSumeragiHash(record.subject_hash, `${itemContext}.subject_hash`),
      qc_mode_tag: requireNonEmptyString(record.qc_mode_tag, `${itemContext}.qc_mode_tag`),
      accepted_candidate_indices: Object.freeze(acceptedCandidateIndices),
      accepted_transaction_hashes: Object.freeze(acceptedTransactionHashes),
      previous_lane_block_height: previousHeight,
      previous_lane_block_descriptor_hash: previousDescriptor,
      lane_block_descriptor_hash: parseSumeragiHash(
        record.lane_block_descriptor_hash,
        `${itemContext}.lane_block_descriptor_hash`,
      ),
      lane_block_descriptor_validator_set: Object.freeze(validators),
      lane_block_descriptor_validator_count: validatorCount,
      lane_block_descriptor_min_quorum: minQuorum,
      payload_ownership_hash: parseSumeragiHash(
        record.payload_ownership_hash,
        `${itemContext}.payload_ownership_hash`,
      ),
      rbc_instance_hash: parseSumeragiHash(
        record.rbc_instance_hash,
        `${itemContext}.rbc_instance_hash`,
      ),
    });
  }));
}

function parseSumeragiCommittedLaneBlocks(value) {
  const context = "status.committed_lane_blocks";
  assertSumeragiArrayBound(value, 128, context);
  return Object.freeze(value.map((entry, index) => {
    const itemContext = `${context}[${index}]`;
    const record = assertExactSumeragiRecord(
      entry,
      [
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "lane_block_height",
        "lane_block_view",
        "descriptor_hash",
        "proposal_hash",
        "execution_status",
        "executable_payload_available",
        "subject_hash",
        "payload_ownership_hash",
        "rbc_instance_hash",
        "qc_mode_tag",
        "validator_count",
        "min_quorum",
        "prepare_qc_signer_count",
        "commit_qc_signer_count",
      ],
      itemContext,
    );
    const validatorCount = parseSumeragiUnsigned(
      record.validator_count,
      `${itemContext}.validator_count`,
      { positive: true, max: 128 },
    );
    const minQuorum = parseSumeragiUnsigned(
      record.min_quorum,
      `${itemContext}.min_quorum`,
      { positive: true, max: 128 },
    );
    const prepareSigners = parseSumeragiUnsigned(
      record.prepare_qc_signer_count,
      `${itemContext}.prepare_qc_signer_count`,
      { max: 128 },
    );
    const commitSigners = parseSumeragiUnsigned(
      record.commit_qc_signer_count,
      `${itemContext}.commit_qc_signer_count`,
      { max: 128 },
    );
    const executionStatus = requireExactNonEmptyString(
      record.execution_status,
      `${itemContext}.execution_status`,
    );
    const executablePayloadAvailable = parseSumeragiBoolean(
      record.executable_payload_available,
      `${itemContext}.executable_payload_available`,
    );
    const unavailableStatuses = new Set([
      "awaiting_executable_payload",
      "application_receipt_conflicts_with_preflight",
      "payload_preflight_rejected_awaiting_state_application",
      "awaiting_predecessor_application",
    ]);
    const availableStatuses = new Set([
      "payload_available_awaiting_executor",
      "payload_recovered_awaiting_state_application",
      "payload_preflighted_awaiting_state_application",
      "state_applied_by_canonical_block",
    ]);
    if (
      (!unavailableStatuses.has(executionStatus) && !availableStatuses.has(executionStatus)) ||
      (availableStatuses.has(executionStatus) !== executablePayloadAvailable)
    ) {
      throw new TypeError(
        `${itemContext}.execution_status disagrees with executable_payload_available`,
      );
    }
    if (
      minQuorum > validatorCount ||
      prepareSigners !== minQuorum ||
      commitSigners !== minQuorum
    ) {
      throw new RangeError(`${itemContext} carries an impossible certified quorum`);
    }
    return Object.freeze({
      lane_id: parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, { max: 0xffffffff }),
      dataspace_id: parseSumeragiUnsigned(record.dataspace_id, `${itemContext}.dataspace_id`),
      lane_incarnation: parseSumeragiNonzeroHash(record.lane_incarnation, `${itemContext}.lane_incarnation`),
      lane_block_height: parseSumeragiUnsigned(record.lane_block_height, `${itemContext}.lane_block_height`, { positive: true }),
      lane_block_view: parseSumeragiUnsigned(record.lane_block_view, `${itemContext}.lane_block_view`),
      descriptor_hash: parseSumeragiHash(record.descriptor_hash, `${itemContext}.descriptor_hash`),
      proposal_hash: parseSumeragiHash(record.proposal_hash, `${itemContext}.proposal_hash`),
      execution_status: executionStatus,
      executable_payload_available: executablePayloadAvailable,
      subject_hash: parseSumeragiHash(record.subject_hash, `${itemContext}.subject_hash`),
      payload_ownership_hash: parseSumeragiHash(record.payload_ownership_hash, `${itemContext}.payload_ownership_hash`),
      rbc_instance_hash: parseSumeragiHash(record.rbc_instance_hash, `${itemContext}.rbc_instance_hash`),
      qc_mode_tag: requireNonEmptyString(record.qc_mode_tag, `${itemContext}.qc_mode_tag`),
      validator_count: validatorCount,
      min_quorum: minQuorum,
      prepare_qc_signer_count: prepareSigners,
      commit_qc_signer_count: commitSigners,
    });
  }));
}

function parseSumeragiLaneBlockSessions(value) {
  const context = "status.lane_block_sessions";
  assertSumeragiArrayBound(value, 128, context);
  return Object.freeze(value.map((entry, index) => {
    const itemContext = `${context}[${index}]`;
    const record = assertExactSumeragiRecord(
      entry,
      [
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "lane_block_height",
        "lane_block_view",
        "proposal_hash",
        "has_proposal",
        "prepare_vote_count",
        "commit_vote_count",
        "has_prepare_qc",
        "has_commit_qc",
        "pending_commit_vote_request",
        "pending_committed_session_drain",
        "committed_session_drained",
        "validator_count",
        "min_quorum",
      ],
      itemContext,
    );
    const validatorCount = parseSumeragiUnsigned(
      record.validator_count,
      `${itemContext}.validator_count`,
      { max: 128 },
    );
    const minQuorum = parseSumeragiUnsigned(
      record.min_quorum,
      `${itemContext}.min_quorum`,
      { max: 128 },
    );
    const prepareVotes = parseSumeragiUnsigned(
      record.prepare_vote_count,
      `${itemContext}.prepare_vote_count`,
      { max: 128 },
    );
    const commitVotes = parseSumeragiUnsigned(
      record.commit_vote_count,
      `${itemContext}.commit_vote_count`,
      { max: 128 },
    );
    if (validatorCount === 0) {
      if (minQuorum !== 0 || prepareVotes !== 0 || commitVotes !== 0) {
        throw new RangeError(`${itemContext} carries impossible session quorum counts`);
      }
    } else if (
      minQuorum === 0 ||
      minQuorum > validatorCount ||
      prepareVotes > validatorCount ||
      commitVotes > validatorCount
    ) {
      throw new RangeError(`${itemContext} carries impossible session quorum counts`);
    }
    return Object.freeze({
      lane_id: parseSumeragiUnsigned(record.lane_id, `${itemContext}.lane_id`, { max: 0xffffffff }),
      dataspace_id: parseSumeragiUnsigned(record.dataspace_id, `${itemContext}.dataspace_id`),
      lane_incarnation: parseSumeragiNonzeroHash(record.lane_incarnation, `${itemContext}.lane_incarnation`),
      lane_block_height: parseSumeragiUnsigned(record.lane_block_height, `${itemContext}.lane_block_height`),
      lane_block_view: parseSumeragiUnsigned(record.lane_block_view, `${itemContext}.lane_block_view`),
      proposal_hash: parseSumeragiHash(record.proposal_hash, `${itemContext}.proposal_hash`),
      has_proposal: parseSumeragiBoolean(record.has_proposal, `${itemContext}.has_proposal`),
      prepare_vote_count: prepareVotes,
      commit_vote_count: commitVotes,
      has_prepare_qc: parseSumeragiBoolean(record.has_prepare_qc, `${itemContext}.has_prepare_qc`),
      has_commit_qc: parseSumeragiBoolean(record.has_commit_qc, `${itemContext}.has_commit_qc`),
      pending_commit_vote_request: parseSumeragiBoolean(
        record.pending_commit_vote_request,
        `${itemContext}.pending_commit_vote_request`,
      ),
      pending_committed_session_drain: parseSumeragiBoolean(
        record.pending_committed_session_drain,
        `${itemContext}.pending_committed_session_drain`,
      ),
      committed_session_drained: parseSumeragiBoolean(
        record.committed_session_drained,
        `${itemContext}.committed_session_drained`,
      ),
      validator_count: validatorCount,
      min_quorum: minQuorum,
    });
  }));
}

function parseSumeragiNonzeroHash(value, context) {
  const hash = parseSumeragiHash(value, context);
  if (/^0{64}$/u.test(hash.slice(5, 69))) {
    throw new TypeError(`${context} must not be the zero hash`);
  }
  return hash;
}

function sumeragiSubjectsEqual(left, right) {
  return (
    left.parent_block_hash === right.parent_block_hash &&
    left.block_hash === right.block_hash &&
    left.payload_hash === right.payload_hash
  );
}

function ensureRecord(value, context) {
  if (!isPlainObject(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

function requireNonEmptyString(value, name) {
  if (typeof value !== "string") {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must be a string`,
      name,
    );
  }
  const trimmed = value.trim();
  if (!trimmed) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must not be empty`,
      name,
    );
  }
  return trimmed;
}

function requireExactNonEmptyString(value, name) {
  const trimmed = requireNonEmptyString(value, name);
  if (trimmed !== value) {
    throw createValidationError(
      ValidationErrorCode.INVALID_STRING,
      `${name} must not contain surrounding whitespace`,
      name,
    );
  }
  return value;
}

function requireCanonicalQuantity(value, name) {
  if (typeof value !== "string") {
    throw createValidationError(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name} must be a canonical Kotodama V1 quantity string`,
      name,
    );
  }
  try {
    return NumericV1.decodeQuantityJson(value).toString();
  } catch (error) {
    if (!(error instanceof NumericV1Error)) throw error;
    throw createValidationError(
      ValidationErrorCode.INVALID_NUMERIC,
      `${name} must be a canonical non-negative Kotodama V1 quantity (${error.code})`,
      name,
    );
  }
}

function parseHashLiteralToHex(literal, name) {
  const match = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/.exec(literal);
  if (!match) {
    throw new TypeError(
      `${name} must be a canonical "hash:<HEX>#<CRC>" literal or hex string`,
    );
  }
  const [, body, checksum] = match;
  const expected = computeHashLiteralCrc("hash", body.toUpperCase());
  if (expected !== checksum.toUpperCase()) {
    throw new TypeError(`${name} has invalid checksum; expected ${expected}`);
  }
  const hex = body.toLowerCase();
  if ((Number.parseInt(hex.slice(-2), 16) & 1) !== 1) {
    throw new TypeError(`${name} must set the Iroha Hash marker bit`);
  }
  return hex;
}

function formatHashLiteral(bodyHex) {
  const upper = bodyHex.toUpperCase();
  const checksum = computeHashLiteralCrc("hash", upper);
  return `hash:${upper}#${checksum}`;
}

function u64ToLittleEndianBuffer(value) {
  const normalized = BigInt.asUintN(64, BigInt(value));
  const buffer = Buffer.alloc(8);
  buffer.writeBigUInt64LE(normalized);
  return buffer;
}

function irohaHashBytes(parts) {
  const digest = Buffer.from(blake2b256(Buffer.concat(parts.map((part) => Buffer.from(part)))));
  digest[digest.length - 1] |= 1;
  return digest;
}

function noritoSchemaHash(typeName) {
  const preimage = Buffer.concat([
    Buffer.from("norito:v1:type-name\0", "utf8"),
    Buffer.from(typeName, "utf8"),
  ]);
  return Buffer.from(sha256(preimage)).subarray(0, 16);
}

function frameNoritoPayload(typeName, payload, flags = 0) {
  const header = Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.from([0, 0]),
    noritoSchemaHash(typeName),
    Buffer.from([0]),
    u64ToLittleEndianBuffer(payload.length),
    u64ToLittleEndianBuffer(crc64Xz(payload)),
    Buffer.from([flags & 0xff]),
  ]);
  return Buffer.concat([header, payload]);
}

function encodeUnsignedLeb128(value) {
  const out = [];
  let remaining = BigInt(value);
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) {
      byte |= 0x80;
    }
    out.push(byte);
  } while (remaining !== 0n);
  return Buffer.from(out);
}

function encodeNoritoLength(value, compact) {
  return compact ? encodeUnsignedLeb128(value) : u64ToLittleEndianBuffer(value);
}

function encodeNoritoField(payload, compact = false) {
  return Buffer.concat([encodeNoritoLength(payload.length, compact), payload]);
}

function encodeNoritoVec(values, encode, compact = false) {
  const parts = [u64ToLittleEndianBuffer(values.length)];
  for (const value of values) {
    const payload = encode(value);
    parts.push(encodeNoritoLength(payload.length, compact), payload);
  }
  return Buffer.concat(parts);
}

export function parseSumeragiStatusJson(
  text,
  context = "Sumeragi typed status",
) {
  return parseSumeragiStatusPayload(
    parseStrictLosslessIntegerJson(text, context),
  );
}

export function parseSumeragiDiagnosticsJson(
  text,
  context = "Sumeragi typed diagnostics",
) {
  return parseSumeragiDiagnosticsPayload(
    parseStrictLosslessIntegerJson(text, context),
  );
}

export const __sumeragiNativeAmxTestHelpers = Object.freeze({
  computeDescriptorHash: computeSumeragiNativeDescriptorHash,
  computeParticipantSettlementHash:
    computeSumeragiNativeParticipantSettlementHash,
  computeProposalHash: computeSumeragiNativeProposalHash,
  computeValidatorSetHash: computeSumeragiNativeValidatorSetHash,
});

export {
  parseSumeragiDiagnosticsPayload,
  parseSumeragiStatusPayload,
};
