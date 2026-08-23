import { blake3 } from "@noble/hashes/blake3";

/** Browser profile for the canonical `IrohaJson` contract payload preimage. */
export const CONTRACT_PAYLOAD_MAX_CANONICAL_BYTES = 1_048_576;
export const CONTRACT_PAYLOAD_MAX_DEPTH = 128;
export const CONTRACT_PAYLOAD_MAX_NODES = 1_000_000;

const encoder = new TextEncoder();

function fail(path, message, ErrorType = TypeError) {
  throw new ErrorType(`contract payload ${path} ${message}`);
}

function isWellFormedUnicode(value) {
  for (let index = 0; index < value.length; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      if (index + 1 >= value.length) return false;
      const low = value.charCodeAt(index + 1);
      if (low < 0xdc00 || low > 0xdfff) return false;
      index += 1;
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      return false;
    }
  }
  return true;
}

function compareUtf8(left, right) {
  const leftBytes = encoder.encode(left);
  const rightBytes = encoder.encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = leftBytes[index] - rightBytes[index];
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

function quoteNoritoJsonString(value, path) {
  if (!isWellFormedUnicode(value)) {
    fail(path, "must contain only Unicode scalar values");
  }
  let output = '"';
  for (const character of value) {
    const codePoint = character.codePointAt(0);
    switch (character) {
      case '"':
        output += '\\"';
        break;
      case "\\":
        output += "\\\\";
        break;
      case "\n":
        output += "\\n";
        break;
      case "\r":
        output += "\\r";
        break;
      case "\t":
        output += "\\t";
        break;
      case "\b":
        output += "\\b";
        break;
      case "\f":
        output += "\\f";
        break;
      default:
        if (codePoint < 0x20) {
          output += `\\u00${codePoint.toString(16).padStart(2, "0")}`;
        } else {
          output += character;
        }
    }
  }
  return `${output}"`;
}

function canonicalize(value) {
  const state = {
    ancestors: new Set(),
    nodes: 0,
  };

  const encode = (current, depth, path) => {
    state.nodes += 1;
    if (state.nodes > CONTRACT_PAYLOAD_MAX_NODES) {
      fail(path, `exceeds the ${CONTRACT_PAYLOAD_MAX_NODES}-node browser limit`, RangeError);
    }
    if (depth > CONTRACT_PAYLOAD_MAX_DEPTH) {
      fail(path, `exceeds the ${CONTRACT_PAYLOAD_MAX_DEPTH}-level browser limit`, RangeError);
    }
    if (current === null) return "null";
    switch (typeof current) {
      case "boolean":
        return current ? "true" : "false";
      case "string":
        return quoteNoritoJsonString(current, path);
      case "number":
        if (!Number.isSafeInteger(current) || Object.is(current, -0)) {
          fail(path, "numbers must be canonical safe integers; encode decimals as strings");
        }
        return String(current);
      case "object": {
        if (state.ancestors.has(current)) fail(path, "must not contain cycles");
        state.ancestors.add(current);
        try {
          if (Array.isArray(current)) {
            if (Object.getPrototypeOf(current) !== Array.prototype) {
              fail(path, "arrays must use Array.prototype");
            }
            const ownKeys = Reflect.ownKeys(current);
            if (ownKeys.length !== current.length + 1 || !ownKeys.includes("length")) {
              fail(path, "arrays must be dense and contain no custom properties");
            }
            const items = [];
            for (let index = 0; index < current.length; index += 1) {
              const descriptor = Object.getOwnPropertyDescriptor(current, String(index));
              if (
                !descriptor
                || !descriptor.enumerable
                || !Object.prototype.hasOwnProperty.call(descriptor, "value")
              ) {
                fail(`${path}[${index}]`, "must be a dense data element");
              }
              items.push(encode(descriptor.value, depth + 1, `${path}[${index}]`));
            }
            return `[${items.join(",")}]`;
          }

          const prototype = Object.getPrototypeOf(current);
          if (prototype !== Object.prototype && prototype !== null) {
            fail(path, "objects must have the default or null prototype");
          }
          const entries = [];
          for (const key of Reflect.ownKeys(current)) {
            if (typeof key !== "string") {
              fail(path, "must not contain symbol keys");
            }
            if (!isWellFormedUnicode(key)) {
              fail(path, "keys must contain only Unicode scalar values");
            }
            const descriptor = Object.getOwnPropertyDescriptor(current, key);
            if (
              !descriptor
              || !descriptor.enumerable
              || !Object.prototype.hasOwnProperty.call(descriptor, "value")
            ) {
              fail(path, "objects must contain only enumerable data properties");
            }
            entries.push([key, descriptor.value]);
          }
          entries.sort(([left], [right]) => compareUtf8(left, right));
          return `{${entries
            .map(([key, entry]) =>
              `${quoteNoritoJsonString(key, `${path} key`)}:${encode(entry, depth + 1, `${path}.${key}`)}`)
            .join(",")}}`;
        } finally {
          state.ancestors.delete(current);
        }
      }
      default:
        fail(path, `contains unsupported ${typeof current} values`);
    }
  };

  return encode(value, 0, "root");
}

/**
 * Return the exact compact JSON text hashed by Torii for the current browser contract profile.
 *
 * `null` and `undefined` mean that the optional payload is absent. Raw floating-point and unsafe
 * integer values are rejected because the browser does not expose Rust's Ryu formatter; encode
 * decimal and wide numeric contract arguments as their canonical schema strings instead.
 */
export function canonicalContractPayloadJson(payload) {
  if (payload === undefined || payload === null) return null;
  const canonical = canonicalize(payload);
  const byteLength = encoder.encode(canonical).length;
  if (byteLength > CONTRACT_PAYLOAD_MAX_CANONICAL_BYTES) {
    throw new RangeError(
      `canonical contract payload exceeds ${CONTRACT_PAYLOAD_MAX_CANONICAL_BYTES} UTF-8 bytes`,
    );
  }
  return canonical;
}

/** Compute Torii's lowercase BLAKE3 digest over the exact canonical payload preimage. */
export function contractPayloadDigestHex(payload) {
  const canonical = canonicalContractPayloadJson(payload);
  const digest = blake3(encoder.encode(canonical ?? ""));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
