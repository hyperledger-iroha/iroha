import { Buffer } from "buffer";

export const CONTRACT_ADDRESS_V1_VERSION = 1;
export const CONTRACT_ADDRESS_HRP = "irohac";

const BECH32M_CONSTANT = 0x2bc830a3;
const BECH32_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_GENERATORS = Object.freeze([
  0x3b6a57b2,
  0x26508e6d,
  0x1ea119fa,
  0x3d4233dd,
  0x2a1462b3,
]);

function convertToBase32(bytes) {
  let accumulator = 0;
  let bits = 0;
  const output = [];
  for (const byte of bytes) {
    accumulator = (accumulator << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      output.push((accumulator >>> bits) & 0x1f);
    }
    accumulator &= bits === 0 ? 0 : (1 << bits) - 1;
  }
  if (bits > 0) output.push((accumulator << (5 - bits)) & 0x1f);
  return output;
}

function bech32Polymod(values) {
  let checksum = 1;
  for (const value of values) {
    const top = checksum >>> 25;
    checksum = ((checksum & 0x1ff_ffff) << 5) ^ value;
    for (let index = 0; index < BECH32_GENERATORS.length; index += 1) {
      if (((top >>> index) & 1) !== 0) checksum ^= BECH32_GENERATORS[index];
    }
  }
  return checksum >>> 0;
}

function decodeBech32Payload(values, context) {
  const output = [];
  let accumulator = 0;
  let bits = 0;
  for (const value of values) {
    accumulator = (accumulator << 5) | value;
    bits += 5;
    while (bits >= 8) {
      bits -= 8;
      output.push((accumulator >>> bits) & 0xff);
    }
    accumulator &= bits === 0 ? 0 : (1 << bits) - 1;
  }
  if (bits >= 5 || (bits > 0 && accumulator !== 0)) {
    throw new TypeError(`${context} has noncanonical Bech32 padding`);
  }
  return Buffer.from(output);
}

function hrpExpand(hrp) {
  return [
    ...Array.from(hrp, (character) => character.codePointAt(0) >>> 5),
    0,
    ...Array.from(hrp, (character) => character.codePointAt(0) & 0x1f),
  ];
}

/** Parse an exact canonical lowercase V1 Bech32m contract address. */
export function parseCanonicalContractAddress(value, context = "contractAddress") {
  if (typeof value !== "string" || value.length === 0 || value.length > 90) {
    throw new TypeError(`${context} must be a canonical V1 Bech32m contract address`);
  }
  if (value !== value.toLowerCase()) {
    throw new TypeError(`${context} must use canonical lowercase Bech32m`);
  }
  const separator = value.lastIndexOf("1");
  if (separator <= 0 || separator > 83 || separator + 7 > value.length) {
    throw new TypeError(`${context} is not a canonical contract address`);
  }
  const hrp = value.slice(0, separator);
  if (![...hrp].every((character) => {
    const codePoint = character.codePointAt(0);
    return codePoint >= 33 && codePoint <= 126;
  })) {
    throw new TypeError(`${context} has an invalid Bech32 human-readable prefix`);
  }
  if (hrp !== CONTRACT_ADDRESS_HRP) {
    throw new TypeError(
      `${context} must use the canonical ${CONTRACT_ADDRESS_HRP} prefix`,
    );
  }
  const values = [];
  for (const character of value.slice(separator + 1)) {
    const index = BECH32_CHARSET.indexOf(character);
    if (index < 0) {
      throw new TypeError(`${context} contains a non-Bech32 character`);
    }
    values.push(index);
  }
  if (
    bech32Polymod([...hrpExpand(hrp), ...values]) !==
    BECH32M_CONSTANT
  ) {
    throw new TypeError(`${context} has an invalid Bech32m checksum`);
  }
  const payload = decodeBech32Payload(values.slice(0, -6), context);
  if (payload.length !== 29 || payload[0] !== CONTRACT_ADDRESS_V1_VERSION) {
    throw new TypeError(`${context} has an unsupported contract-address payload`);
  }
  return Object.freeze({
    literal: value,
    hrp,
    dataspaceId: payload.readBigUInt64BE(1),
  });
}

/** Encode a canonical Bech32m literal from a validated HRP and V1 payload. */
export function encodeContractAddressBech32m(hrp, payload) {
  const data = convertToBase32(payload);
  const checksumInput = [
    ...hrpExpand(hrp),
    ...data,
    0,
    0,
    0,
    0,
    0,
    0,
  ];
  const polymod = (bech32Polymod(checksumInput) ^ BECH32M_CONSTANT) >>> 0;
  const checksum = Array.from(
    { length: 6 },
    (_, index) => (polymod >>> (5 * (5 - index))) & 0x1f,
  );
  return `${hrp}1${[...data, ...checksum]
    .map((value) => BECH32_CHARSET[value])
    .join("")}`;
}
