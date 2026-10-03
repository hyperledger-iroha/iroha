// Canonical compact-field readers shared by the embedded V1 artifact schemas.
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true });
const TEXT_LENGTH = "length";
const TEXT_IS_TRUNCATED = " is truncated";
const TEXT_EXCEEDS_THE = " exceeds the ";
function rejectAt(label, message) { throw new TypeError(`${label}${message}`); }
function rejectRange(message) { throw new RangeError(message); }

export function readU32Le(bytes, offset, label) {
  if (offset < 0 || offset + 4 > bytes.length) {
    rejectAt(label, `${TEXT_IS_TRUNCATED}`);
  }
  return (
    bytes[offset] |
    (bytes[offset + 1] << 8) |
    (bytes[offset + 2] << 16) |
    (bytes[offset + 3] * 0x1000000)
  ) >>> 0;
}

export function readU64Le(bytes, offset, label) {
  if (offset < 0 || offset + 8 > bytes.length) {
    rejectAt(label, `${TEXT_IS_TRUNCATED}`);
  }
  let value = 0n;
  for (let index = 7; index >= 0; index -= 1) {
    value = (value << 8n) | BigInt(bytes[offset + index]);
  }
  return value;
}

export function readCompactLength(bytes, state, label) {
  let value = 0n;
  let shift = 0n;
  const start = state.offset;
  for (;;) {
    if (state.offset >= bytes.length || state.offset - start >= 8) {
      rejectAt(label, " contains a truncated or oversized compact length");
    }
    const byte = bytes[state.offset];
    state.offset += 1;
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      if (state.offset - start > 1 && byte === 0) {
        rejectAt(label, " contains a noncanonical compact length");
      }
      if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
        rejectRange(`${label} compact length exceeds the safe integer range`);
      }
      return Number(value);
    }
    shift += 7n;
  }
}

export function readCompactField(bytes, state, label) {
  const length = readCompactLength(bytes, state, `${label}.${TEXT_LENGTH}`);
  const end = state.offset + length;
  if (end > bytes.length) {
    rejectAt(label, ` payload${TEXT_IS_TRUNCATED}`);
  }
  const field = bytes.subarray(state.offset, end);
  state.offset = end;
  return field;
}

export function decodeEmbeddedString(field, label) {
  const state = { offset: 0 };
  const encoded = readCompactField(field, state, label);
  if (state.offset !== field.length) {
    rejectAt(label, " contains trailing bytes");
  }
  try {
    return UTF8_DECODER.decode(encoded);
  } catch {
    rejectAt(label, " is not valid UTF-8");
  }
}

export function visitEmbeddedVector(field, label, maximum, visit = () => {}) {
  const count = readU64Le(field, 0, `${label}.count`);
  if (count > BigInt(maximum)) {
    rejectRange(`${label}${TEXT_EXCEEDS_THE}${maximum}-item limit`);
  }
  const state = { offset: 8 };
  for (let index = 0; index < Number(count); index += 1) {
    const itemLabel = `${label}[${index}]`;
    visit(readCompactField(field, state, itemLabel), itemLabel);
  }
  if (state.offset !== field.length) {
    rejectAt(label, " has trailing or missing vector bytes");
  }
  return Number(count);
}
