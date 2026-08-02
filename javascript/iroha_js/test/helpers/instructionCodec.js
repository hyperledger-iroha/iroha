import assert from "node:assert/strict";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
} from "../../src/norito.js";
import { nativeBinding } from "./native.js";

/** Run a callback with the native instruction codec disabled. */
export function withPureJsInstructionCodec(body) {
  const hadBinding = Object.prototype.hasOwnProperty.call(
    globalThis,
    "__IROHA_NORITO_BINDING__",
  );
  const previous = globalThis.__IROHA_NORITO_BINDING__;
  globalThis.__IROHA_NORITO_BINDING__ = {
    noritoEncodeInstruction() {
      throw new Error("unsupported instruction");
    },
    noritoDecodeInstruction() {
      throw new Error("unsupported instruction");
    },
  };
  try {
    return body();
  } finally {
    if (hadBinding) {
      globalThis.__IROHA_NORITO_BINDING__ = previous;
    } else {
      delete globalThis.__IROHA_NORITO_BINDING__;
    }
  }
}

/** Convert any supported binary container into an ordinary byte array. */
export function toByteArray(bytes) {
  return Array.from(Buffer.from(bytes));
}

/** Assert native/pure-JS instruction byte and decode parity. */
export function assertNativeAndPureInstructionParity(instruction, context) {
  const pureEncoded = Buffer.from(
    withPureJsInstructionCodec(() => noritoEncodeInstruction(instruction)),
  );
  const nativeEncoded = Buffer.from(
    nativeBinding.noritoEncodeInstruction(JSON.stringify(instruction)),
  );
  assert.deepEqual(pureEncoded, nativeEncoded, `${context} bytes`);
  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(pureEncoded)),
    instruction,
    `${context} native decode`,
  );
  assert.deepEqual(
    withPureJsInstructionCodec(() => noritoDecodeInstruction(nativeEncoded)),
    instruction,
    `${context} pure decode`,
  );
  return pureEncoded;
}

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let index = 0; index < 8; index += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return crc & 0xffff;
}

/** Render a canonical checksummed `HashOf` literal for 32 bytes. */
export function normalizedHashHex(bytes) {
  const buffer = Buffer.from(bytes);
  if (buffer.length !== 32) {
    throw new TypeError("hash literal test helper requires 32 bytes");
  }
  buffer[buffer.length - 1] |= 1;
  const body = buffer.toString("hex").toUpperCase();
  const checksum = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
  return `hash:${body}#${checksum}`;
}
