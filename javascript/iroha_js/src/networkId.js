const NETWORK_ID_BYTE_LENGTH = 32;
const HASH_LITERAL_PATTERN = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u;
const CONSTRUCTION_TOKEN = Symbol("NetworkId construction token");
const networkIdStorage = new WeakMap();

function crc16CcittFalse(bytes) {
  let crc = 0xffff;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  }
  return crc;
}

function copyBytes(value, context) {
  let bytes;
  if (value instanceof ArrayBuffer) {
    bytes = new Uint8Array(value);
  } else if (ArrayBuffer.isView(value)) {
    bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  } else {
    throw new TypeError(`${context} must be an ArrayBuffer or ArrayBuffer view`);
  }
  return Uint8Array.from(bytes);
}

function validateRawNetworkId(bytes, context) {
  if (bytes.length !== NETWORK_ID_BYTE_LENGTH) {
    throw new TypeError(
      `${context} must contain exactly ${NETWORK_ID_BYTE_LENGTH} bytes`,
    );
  }
  if ((bytes[NETWORK_ID_BYTE_LENGTH - 1] & 1) === 0) {
    throw new TypeError(`${context} must carry the canonical Iroha hash marker bit`);
  }
  return bytes;
}

function canonicalLiteral(bytes) {
  const body = Array.from(bytes, (byte) =>
    byte.toString(16).toUpperCase().padStart(2, "0"),
  ).join("");
  const prefix = `hash:${body}`;
  const checksum = crc16CcittFalse(new TextEncoder().encode(prefix))
    .toString(16)
    .toUpperCase()
    .padStart(4, "0");
  return `${prefix}#${checksum}`;
}

/**
 * Exact immutable identity of one Iroha network.
 *
 * A NetworkId is the marked 32-byte consensus hash of the genesis header. It
 * is deliberately not constructible from a human-readable chain label.
 */
export class NetworkId {
  constructor(token, bytes, literal) {
    if (token !== CONSTRUCTION_TOKEN) {
      throw new TypeError("NetworkId must be created with NetworkId.parse or NetworkId.fromBytes");
    }
    networkIdStorage.set(this, Uint8Array.from(bytes));
    Object.defineProperty(this, "literal", {
      value: literal,
      enumerable: true,
      configurable: false,
      writable: false,
    });
    Object.freeze(this);
  }

  /** Parse one exact checksummed uppercase Iroha hash literal. */
  static parse(literal) {
    if (typeof literal !== "string") {
      throw new TypeError("NetworkId literal must be a string");
    }
    const match = HASH_LITERAL_PATTERN.exec(literal);
    if (match === null) {
      throw new TypeError(
        "NetworkId must be an exact canonical checksummed 32-byte Iroha hash literal",
      );
    }
    const bytes = Uint8Array.from(
      match[1].match(/../gu),
      (pair) => Number.parseInt(pair, 16),
    );
    validateRawNetworkId(bytes, "NetworkId");
    if (canonicalLiteral(bytes) !== literal) {
      throw new TypeError("NetworkId literal checksum is invalid");
    }
    return new NetworkId(CONSTRUCTION_TOKEN, bytes, literal);
  }

  /** Create a NetworkId from exactly 32 marked genesis-header hash bytes. */
  static fromBytes(value) {
    const bytes = validateRawNetworkId(
      copyBytes(value, "NetworkId bytes"),
      "NetworkId bytes",
    );
    return new NetworkId(CONSTRUCTION_TOKEN, bytes, canonicalLiteral(bytes));
  }

  /** Return a defensive copy of the exact genesis-header hash bytes. */
  toBytes() {
    return Uint8Array.from(networkIdStorage.get(this));
  }

  equals(other) {
    if (!networkIdStorage.has(other)) return false;
    const left = networkIdStorage.get(this);
    const right = networkIdStorage.get(other);
    return left.every((byte, index) => byte === right[index]);
  }

  toString() {
    return this.literal;
  }

  toJSON() {
    return this.literal;
  }

  get [Symbol.toStringTag]() {
    return "NetworkId";
  }
}

/* @__PURE__ */ Object.defineProperty(NetworkId, "BYTE_LENGTH", {
  value: NETWORK_ID_BYTE_LENGTH,
  enumerable: true,
  configurable: false,
  writable: false,
});

/** Internal closed-world validation used by transaction entrypoints. */
export function networkIdBytes(value, context = "networkId") {
  if (!networkIdStorage.has(value)) {
    throw new TypeError(`${context} must be a NetworkId`);
  }
  return Uint8Array.from(networkIdStorage.get(value));
}
