import assert from "node:assert/strict";
import {
  _createNoritoInstructionApi,
} from "../../src/norito.js";
import { createNativeRuntime } from "../../src/nativeRuntime.js";
import { nativeBinding } from "./native.js";

const NATIVE_INSTRUCTION_API = _createNoritoInstructionApi(
  createNativeRuntime(),
);

/** Exercise the JavaScript adapter backed by the canonical native instruction owner. */
export function withNativeInstructionCodec(body) {
  return body(NATIVE_INSTRUCTION_API);
}

/** Convert any supported binary container into an ordinary byte array. */
export function toByteArray(bytes) {
  return Array.from(Buffer.from(bytes));
}

/** Check adapter serialization and projection against direct calls to the same native owner. */
export function assertNativeInstructionAdapterRoundTrip(instruction, context) {
  const adapterEncoded = Buffer.from(
    withNativeInstructionCodec(({ noritoEncodeInstruction }) =>
      noritoEncodeInstruction(instruction, 753)),
  );
  const nativeEncoded = Buffer.from(
    nativeBinding.noritoEncodeInstruction(JSON.stringify(instruction), 753),
  );
  assert.deepEqual(adapterEncoded, nativeEncoded, `${context} bytes`);
  assert.deepEqual(
    JSON.parse(nativeBinding.noritoDecodeInstruction(adapterEncoded, 753)),
    instruction,
    `${context} native decode`,
  );
  assert.deepEqual(
    withNativeInstructionCodec(({ noritoDecodeInstruction }) =>
      noritoDecodeInstruction(nativeEncoded, 753)),
    instruction,
    `${context} adapter decode`,
  );
  return adapterEncoded;
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
