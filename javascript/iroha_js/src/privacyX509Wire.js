import { Buffer } from "buffer";
import { blake3 } from "@noble/hashes/blake3.js";
import {
  frameNoritoPayload,
  schemaHashForTypeName,
  validateNoritoFrame,
  u32ToLittleEndianBuffer,
  u64ToLittleEndianBuffer,
} from "./noritoValueCodecs.js";

/** Internal canonical byte owner; these projections never grant proof or State authority. */
export const PRIVACY_X509_WIRE_ID_V1 = "iroha.privacy.submit_zk_x509_certificate.v1";
const REQUEST_SCHEMA = "iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1";
const CONCRETE_SCHEMA = "iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1";
const STATEMENT_SCHEMA = "iroha.privacy.statement.v1";
const ACTION_SCHEMA = "iroha_data_model::transaction::signed_attachments::ActionStatementV1";
const PAYLOAD_SCHEMA = "iroha_data_model::transaction::signed::model::TransactionPayload";
const STATEMENT_LIMIT = 256 * 1024;
const PROOF_LIMIT = 9 * 1024 * 1024;
const TRANSACTION_LIMIT = 10 * 1024 * 1024;
const ZERO = Buffer.alloc(32);
const u32 = u32ToLittleEndianBuffer;
const u64 = u64ToLittleEndianBuffer;

function invalid(message) { throw new TypeError(`X509: ${message}`); }

function bytes(value, maximum, label) {
  if (!(value instanceof ArrayBuffer) && !ArrayBuffer.isView(value)) {
    invalid(`${label} must be bytes`);
  }
  if (value.byteLength > maximum) invalid(`${label} exceeds its byte ceiling`);
  const view = value instanceof ArrayBuffer
    ? new Uint8Array(value)
    : new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  return Buffer.from(view);
}

function length(value) {
  const out = [];
  do {
    const next = value % 128;
    value = Math.floor(value / 128);
    out.push(next | (value ? 128 : 0));
  } while (value);
  return Buffer.from(out);
}
function fields(values) {
  return Buffer.concat(values.flatMap((value) => [length(value.length), value]));
}
function sequence(values) { return Buffer.concat([u64(values.length), fields(values)]); }
function tagged(tag, value) { return Buffer.concat([u32(tag), fields([value])]); }
function string(value) {
  const encoded = Buffer.from(value, "utf8");
  return fields([encoded]);
}
function frame(payload, schema, alignment = 8) {
  return frameNoritoPayload(payload, schemaHashForTypeName(schema), 2, alignment === 16 ? 8 : 0);
}
const REQUEST_LIMIT = STATEMENT_LIMIT - frame(Buffer.alloc(0), STATEMENT_SCHEMA, 16).length - 4
  + fields([fields([u32(0), u32(0)])]).length + frame(Buffer.alloc(0), REQUEST_SCHEMA).length;
const fieldBytes = (value) => length(value).length + value;
const INSTRUCTION_TUPLE_LIMIT = fieldBytes(string(PRIVACY_X509_WIRE_ID_V1).length)
  + fieldBytes(8 + REQUEST_LIMIT);

function split(input, count, maximum, label) {
  const result = [];
  let cursor = 0;
  for (let index = 0; index < count; index += 1) {
    const start = cursor;
    let size = 0n;
    let shift = 0n;
    while (true) {
      if (cursor >= input.length || shift > 63n) invalid(`${label} has a truncated length`);
      const byte = input[cursor++];
      if (shift === 63n && byte > 1) invalid(`${label} has an overflowing length`);
      size |= BigInt(byte & 127) << shift;
      if (!(byte & 128)) break;
      shift += 7n;
    }
    if (size > BigInt(maximum) || size > BigInt(input.length - cursor)) {
      invalid(`${label} field exceeds its byte ceiling`);
    }
    const sizeNumber = Number(size);
    if (!input.subarray(start, cursor).equals(length(sizeNumber))) {
      invalid(`${label} has a noncanonical length`);
    }
    result.push(Buffer.from(input.subarray(cursor, cursor + sizeNumber)));
    cursor += sizeNumber;
  }
  if (cursor !== input.length) invalid(`${label} has trailing fields`);
  return result;
}
function exactFrame(value, schema, alignment, maximum, label) {
  const archive = bytes(value, maximum, label);
  const decoded = validateNoritoFrame(archive, {
    context: `X509 ${label}`, expectedTypeName: schema,
    expectedPaddingLength: alignment === 16 ? 8 : 0,
  });
  if (decoded.flags !== 2 || !frame(decoded.payload, schema, alignment).equals(archive)) {
    invalid(`${label} is not a canonical frame`);
  }
  return decoded.payload;
}
function digest(value, nonzero = false) {
  const [raw] = split(value, 1, 32, "digest");
  if (raw.length !== 32 || (nonzero && raw.equals(ZERO))) invalid("invalid digest");
  return raw;
}
function integer32(value) {
  if (value.length !== 4) invalid("invalid u32");
  return value.readUInt32LE(0);
}
function rawVector(value, maximum) {
  if (value.length < 8 || value.length - 8 > maximum
      || value.readBigUInt64LE(0) !== BigInt(value.length - 8)) invalid("invalid byte vector");
  return Buffer.from(value.subarray(8));
}
function domainHash(domain, value) {
  return Buffer.from(blake3(Buffer.concat([Buffer.from(domain, "utf8"), u64(value.length), value])));
}

/** Parse the sole proof-free request shape. Intrinsic semantics remain the model's responsibility. */
export function parsePrivacyX509RequestV1(value) {
  const payload = exactFrame(value, REQUEST_SCHEMA, 8, REQUEST_LIMIT, "request");
  const [statementBytes, reference] = split(payload, 2, STATEMENT_LIMIT, "request");
  const statement = split(statementBytes, 20, STATEMENT_LIMIT, "statement");
  const context = split(statement[0], 8, STATEMENT_LIMIT, "context");
  if (context[0].length !== 32 || (context[0][31] & 1) === 0
      || context[1].length !== 4 || !digest(context[2]).equals(ZERO)) {
    invalid("request requires exact network, ordinal and ZERO derived intent");
  }
  for (const value of context.slice(3)) digest(value, true);
  const [action, slot] = split(reference, 2, 4, "reference");
  if (integer32(slot) !== 0) invalid("proof slot must be zero");
  if (frame(tagged(5, fields(statement)), STATEMENT_SCHEMA, 16).length > STATEMENT_LIMIT) {
    invalid("statement exceeds its canonical ceiling");
  }
  return { statement, context, actionIndex: integer32(action) };
}

function singleInstruction(value) {
  if (value.length < 4 || value.readUInt32LE(0) !== 0) invalid("direct instructions required");
  const [list] = split(value.subarray(4), 1, 8 + fieldBytes(INSTRUCTION_TUPLE_LIMIT), "executable");
  if (list.length < 8 || list.readBigUInt64LE(0) !== 1n) invalid("one direct instruction required");
  const [boxed] = split(list.subarray(8), 1, INSTRUCTION_TUPLE_LIMIT, "instruction list");
  const [wire, archive] = split(boxed, 2, INSTRUCTION_TUPLE_LIMIT, "instruction");
  if (!wire.equals(string(PRIVACY_X509_WIRE_ID_V1))) invalid("retired or substituted instruction wire");
  return rawVector(archive, REQUEST_LIMIT);
}

function attachments(action, actionDigest, proof, proofHash) {
  const reference = fields([u32(0), u32(0), u32(0), u64(proof.length), fields([proofHash])]);
  const descriptor = fields([...action, fields([actionDigest]), sequence([reference])]);
  const occurrence = fields([u32(0), u32(0), u64(proof.length), fields([proofHash]),
    Buffer.concat([u64(proof.length), proof])]);
  return Buffer.concat([Buffer.of(1), fields([fields([sequence([descriptor]), sequence([occurrence])])])]);
}

/**
 * Compute one direct action's canonical projections from retained original bytes.
 * Returned copies are public data, not a capability to bypass native admission.
 */
export function preparePrivacyX509DirectBytesV1(requestValue, unsignedValue) {
  const requestArchive = bytes(requestValue, REQUEST_LIMIT, "request");
  const request = parsePrivacyX509RequestV1(requestArchive);
  const payload = split(bytes(unsignedValue, TRANSACTION_LIMIT, "unsigned payload"), 9,
    TRANSACTION_LIMIT, "unsigned payload");
  if (request.actionIndex !== 0 || integer32(request.context[1]) !== 0
      || !payload[8].equals(Buffer.of(0))
      || !payload[0].equals(tagged(0, request.context[0]))
      || !payload[1].equals(request.statement[17])
      || !singleInstruction(payload[3]).equals(requestArchive)) {
    invalid("unsupported direct action or substituted original network/authority/request");
  }
  const publicInputs = domainHash("iroha.privacy.zk-x509-request-public-inputs.v1",
    frame(fields(request.statement), CONCRETE_SCHEMA));
  const action = [u32(0), tagged(0, string(PRIVACY_X509_WIRE_ID_V1)), u32(0), u32(0),
    tagged(1, u32(0)), fields([publicInputs])];
  payload[8] = attachments(action, ZERO, Buffer.alloc(0), ZERO);
  const ordered = domainHash("iroha.transaction.ordered-intent-digest.v1", frame(fields(payload), PAYLOAD_SCHEMA));
  const actionStatement = frame(fields([fields([ordered]), ...action]), ACTION_SCHEMA);
  const actionDigest = domainHash("iroha.transaction.action-statement-digest.v1", actionStatement);
  const context = request.context.slice();
  context[2] = fields([actionDigest]);
  const statement = request.statement.slice();
  statement[0] = fields(context);
  const materializedStatement = frame(tagged(5, fields(statement)), STATEMENT_SCHEMA, 16);
  return Object.freeze({
    get orderedIntentDigest() { return Buffer.from(ordered); },
    get actionStatementNorito() { return Buffer.from(actionStatement); },
    get actionStatementDigest() { return Buffer.from(actionDigest); },
    get materializedStatementNorito() { return Buffer.from(materializedStatement); },
    completePayload(rawProof, limits) {
      const proof = bytes(rawProof, PROOF_LIMIT, "proof");
      const required = ["max_actions_per_transaction", "max_proof_bytes_per_action", "max_action_bytes",
        "max_privacy_bytes_per_transaction", "max_statement_and_encrypted_output_bytes_per_transaction"];
      for (const key of required) {
        if (!Number.isInteger(limits?.[key]) || limits[key] < 0 || limits[key] > 0xffff_ffff) {
          invalid(`invalid committed limit ${key}`);
        }
      }
      const charge = requestArchive.length + proof.length;
      if (proof.length === 0 || proof.every((b) => b === 0)
          || limits.max_actions_per_transaction < 1 || proof.length > limits.max_proof_bytes_per_action
          || charge > limits.max_action_bytes || charge > limits.max_privacy_bytes_per_transaction
          || materializedStatement.length > limits.max_statement_and_encrypted_output_bytes_per_transaction) {
        invalid("proof or action exceeds committed bounds");
      }
      const complete = payload.slice();
      complete[8] = attachments(action, actionDigest, proof, Buffer.from(blake3(proof)));
      const result = fields(complete);
      if (result.length > TRANSACTION_LIMIT) invalid("completed payload exceeds transaction ceiling");
      return result;
    },
  });
}

/** Verify complete original-byte bindings for the native-generated X509 fixture row. */
export function validatePrivacyX509FixtureBindingsV1(row) {
  if (row.submitProofWireId !== PRIVACY_X509_WIRE_ID_V1) invalid("wrong fixture instruction wire");
  const unsigned = split(bytes(row.unsignedTransactionPayloadNorito, TRANSACTION_LIMIT, "fixture payload"),
    9, TRANSACTION_LIMIT, "fixture payload");
  const original = unsigned[8];
  unsigned[8] = Buffer.of(0);
  const plan = preparePrivacyX509DirectBytesV1(row.submitProofInstructionNorito, fields(unsigned));
  const envelope = exactFrame(row.envelopeNorito, "iroha.privacy.proof-envelope.v1", 16,
    PROOF_LIMIT + STATEMENT_LIMIT + 4096, "fixture envelope");
  const envelopeFields = split(envelope, 13, PROOF_LIMIT + STATEMENT_LIMIT, "envelope");
  if (envelopeFields[12].length < 4 || envelopeFields[12].readUInt32LE(0) !== 5) invalid("proof protocol");
  const [wrapper] = split(envelopeFields[12].subarray(4), 1, PROOF_LIMIT + 32, "proof");
  const [vector] = split(wrapper, 1, PROOF_LIMIT + 8, "proof bytes");
  const proof = rawVector(vector, PROOF_LIMIT);
  const complete = plan.completePayload(proof, {
    max_actions_per_transaction: 1, max_proof_bytes_per_action: PROOF_LIMIT,
    max_action_bytes: PROOF_LIMIT + REQUEST_LIMIT, max_privacy_bytes_per_transaction: PROOF_LIMIT + REQUEST_LIMIT,
    max_statement_and_encrypted_output_bytes_per_transaction: STATEMENT_LIMIT,
  });
  const expected = split(complete, 9, TRANSACTION_LIMIT, "completed fixture payload");
  if (!original.equals(expected[8])
      || !Buffer.from(row.statementNorito).equals(plan.materializedStatementNorito)
      || !Buffer.from(row.transactionIntentProjectionNorito).equals(plan.actionStatementNorito)
      || !Buffer.from(row.transactionIntentDigest).equals(plan.actionStatementDigest)
      || !digest(envelopeFields[10]).equals(domainHash("iroha:privacy:statement:v1", plan.materializedStatementNorito))) {
    invalid("fixture request, occurrence, intent or materialized statement differs");
  }
}
