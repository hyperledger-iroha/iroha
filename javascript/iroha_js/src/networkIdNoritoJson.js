import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { NetworkId, networkIdBytes } from "./networkId.js";

const NETWORK_ID_NORITO_JSON_PATTERN = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u;

/** Encode a typed NetworkId as canonical marked Norito JSON. */
export function networkIdToNoritoJson(value, context = "networkId") {
  const body = Array.from(
    networkIdBytes(value, context),
    (byte) => byte.toString(16).padStart(2, "0"),
  ).join("").toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

/** Parse a typed NetworkId from canonical marked Norito JSON. */
export function networkIdFromNoritoJson(value, context = "networkId") {
  const match = typeof value === "string" ? NETWORK_ID_NORITO_JSON_PATTERN.exec(value) : null;
  if (match === null) {
    throw new TypeError(`${context} must be a canonical marked Iroha NetworkId`);
  }
  const [, body, checksum] = match;
  if (computeHashLiteralCrc("hash", body) !== checksum) {
    throw new TypeError(`${context} has an invalid canonical marked Iroha NetworkId checksum`);
  }
  return NetworkId.parse(body.toLowerCase());
}
