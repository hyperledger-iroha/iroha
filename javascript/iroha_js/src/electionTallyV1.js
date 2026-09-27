// Exact standalone-election tally projection from the canonical Torii V1 response.

const U64_MAX = (1n << 64n) - 1n;
const U128_MAX = (1n << 128n) - 1n;
const SAFE_MAX = BigInt(Number.MAX_SAFE_INTEGER);
const RESPONSE_FIELDS = [
  "evaluated_block_height", "evaluated_block_hash", "finalized", "tally",
];

function exactRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)
    || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    throw new TypeError("election tally response must be an object");
  }
  const keys = Reflect.ownKeys(value);
  if (keys.length !== RESPONSE_FIELDS.length
    || RESPONSE_FIELDS.some((field) => !Object.hasOwn(value, field))) {
    throw new TypeError("election tally response must contain exactly the V1 fields");
  }
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !descriptor.enumerable || !("value" in descriptor)) {
      throw new TypeError("election tally response must contain only data fields");
    }
  }
  return value;
}

function unsigned(value, maximum, context) {
  if (!(typeof value === "bigint"
    || (typeof value === "number" && Number.isSafeInteger(value) && !Object.is(value, -0)))) {
    throw new TypeError(`${context} must be an unsigned JSON integer token`);
  }
  const integer = BigInt(value);
  if (integer < 0n || integer > maximum) {
    throw new RangeError(`${context} exceeds its unsigned bound`);
  }
  return integer <= SAFE_MAX ? Number(integer) : integer;
}

/** Decode one exact, bounded V1 election tally without rounding its u128 weights. */
export function parseElectionTallyResponseV1(value) {
  const record = exactRecord(value);
  const evaluatedBlockHeight = unsigned(
    record.evaluated_block_height, U64_MAX, "election tally.evaluated_block_height",
  );
  const evaluatedBlockHash = record.evaluated_block_hash;
  if (typeof evaluatedBlockHash !== "string" || !/^[0-9a-f]{64}$/u.test(evaluatedBlockHash)
    || (evaluatedBlockHeight === 0) !== (evaluatedBlockHash === "0".repeat(64))) {
    throw new TypeError("election tally has invalid evaluated block coordinates");
  }
  if (typeof record.finalized !== "boolean") {
    throw new TypeError("election tally.finalized must be a boolean");
  }
  if (!Array.isArray(record.tally) || record.tally.length < 2 || record.tally.length > 64) {
    throw new RangeError("election tally.tally must contain 2–64 weights");
  }
  let total = 0n;
  const tally = [];
  for (let index = 0; index < record.tally.length; index += 1) {
    const exact = unsigned(record.tally[index], U128_MAX, `election tally.tally[${index}]`);
    total += BigInt(exact);
    if (total > U128_MAX) {
      throw new RangeError("election tally aggregate exceeds u128");
    }
    tally.push(exact);
  }
  return {
    evaluated_block_height: evaluatedBlockHeight,
    evaluated_block_hash: evaluatedBlockHash,
    finalized: record.finalized,
    tally,
  };
}
