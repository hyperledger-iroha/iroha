import { Buffer } from "buffer";

const NETWORK_ID_BYTES = 32;
const STATEMENT_FIELD_COUNTS = Object.freeze([
  11, 10, 6, 9, 15, 20, 4, 8, 9, 8, 13, 13,
]);
const STATEMENT_CONTEXT_FIELDS = 8;

export const PRIVACY_EXACT12_TRANSACTION_PAYLOAD_FIELD_NAMES_V1 = Object.freeze([
  "domain",
  "authority",
  "creation_time_ms",
  "instructions",
  "time_to_live_ms",
  "nonce",
  "fee_payment",
  "metadata",
  "attachments",
]);

function readCompactLength(buffer, cursor, context) {
  let value = 0;
  let multiplier = 1;
  for (let index = 0; index < 10; index += 1) {
    if (cursor.offset >= buffer.length) {
      throw new Error(`${context} is truncated`);
    }
    const byte = buffer[cursor.offset];
    cursor.offset += 1;
    const chunk = byte & 0x7f;
    if (index === 9 && chunk > 1) {
      throw new Error(`${context} exceeds an unsigned 64-bit length`);
    }
    value += chunk * multiplier;
    if (!Number.isSafeInteger(value)) {
      throw new Error(`${context} exceeds the safe decoder length`);
    }
    if ((byte & 0x80) === 0) {
      if (index > 0 && chunk === 0) {
        throw new Error(`${context} is not minimally compact-encoded`);
      }
      return value;
    }
    multiplier *= 128;
  }
  throw new Error(`${context} exceeds the compact-length limit`);
}

function readField(buffer, cursor, context) {
  const length = readCompactLength(buffer, cursor, `${context}.length`);
  const end = cursor.offset + length;
  if (end > buffer.length) {
    throw new Error(`${context} overruns its parent payload`);
  }
  const field = buffer.subarray(cursor.offset, end);
  cursor.offset = end;
  return field;
}

function decodeFields(payload, count, context) {
  const buffer = Buffer.from(payload);
  const cursor = { offset: 0 };
  const fields = Array.from({ length: count }, (_, index) =>
    readField(buffer, cursor, `${context}.field[${index}]`),
  );
  if (cursor.offset !== buffer.length) {
    throw new Error(`${context} has trailing or unknown fields`);
  }
  return fields;
}

function exactNetworkId(payload, context) {
  const networkId = Buffer.from(payload);
  if (
    networkId.length !== NETWORK_ID_BYTES ||
    (networkId[NETWORK_ID_BYTES - 1] & 1) === 0
  ) {
    throw new TypeError(
      `${context} must contain exactly 32 marked Iroha hash bytes`,
    );
  }
  return networkId;
}

function networkTransactionDomain(payload, context) {
  const buffer = Buffer.from(payload);
  if (buffer.length < 4) {
    throw new Error(`${context} is truncated`);
  }
  const tag = buffer.readUInt32LE(0);
  if (tag !== 0) {
    throw new TypeError(
      `${context} must use TransactionDomain::Network; genesis is not client-signable`,
    );
  }
  const cursor = { offset: 4 };
  const networkId = exactNetworkId(
    readField(buffer, cursor, `${context}.networkId`),
    `${context}.networkId`,
  );
  if (cursor.offset !== buffer.length) {
    throw new Error(`${context} has trailing or unknown fields`);
  }
  return networkId;
}

/** Validate the exact-network bindings inside one typed Exact12 fixture row. */
export function validatePrivacyExact12NetworkBindingsV1({
  statementTag,
  statementContent,
  projectionDomain,
  unsignedDomain,
  context,
}) {
  if (
    !Number.isInteger(statementTag) ||
    statementTag < 0 ||
    statementTag >= STATEMENT_FIELD_COUNTS.length
  ) {
    throw new TypeError(`${context}.statementTag is outside Exact12`);
  }
  const statementFields = decodeFields(
    statementContent,
    STATEMENT_FIELD_COUNTS[statementTag],
    `${context}.statementNorito.variant`,
  );
  const statementContext = decodeFields(
    statementFields[0],
    STATEMENT_CONTEXT_FIELDS,
    `${context}.statementNorito.context`,
  );
  const statementNetworkId = exactNetworkId(
    statementContext[0],
    `${context}.statementNorito.context.networkId`,
  );
  const projectionNetworkId = networkTransactionDomain(
    projectionDomain,
    `${context}.transactionIntentProjectionNorito.domain`,
  );
  const unsignedNetworkId = networkTransactionDomain(
    unsignedDomain,
    `${context}.unsignedTransactionPayloadNorito.domain`,
  );
  if (
    !projectionNetworkId.equals(statementNetworkId) ||
    !unsignedNetworkId.equals(statementNetworkId)
  ) {
    throw new TypeError(
      `${context} transaction NetworkId does not match its privacy statement context`,
    );
  }
}
