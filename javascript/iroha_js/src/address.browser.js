import { getNativeBinding } from "./native.browser.js";

export { AccountAddressError, AccountAddressErrorCode } from "./addressErrors.js";
export {
  curveIdFromAlgorithm, curveIdToAlgorithm, ensureCurveIdEnabled, normalizeBytes,
} from "./addressPrimitives.js";

/**
 * Browser platforms cannot load the canonical Rust account owner. Keep the
 * account API explicit while rejecting admission before inspecting any input.
 */
export class AccountAddress {
  constructor() { getNativeBinding(); }
  static fromAccount() { getNativeBinding(); }
  static fromCanonicalBytes() { getNativeBinding(); }
  static fromI105() { getNativeBinding(); }
  static fromAccountId() { getNativeBinding(); }
  static parseEncoded() { getNativeBinding(); }
  canonicalBytes() { getNativeBinding(); }
  canonicalHex() { getNativeBinding(); }
  toI105() { getNativeBinding(); }
  toString() { getNativeBinding(); }
  displayFormats() { getNativeBinding(); }
  controllerInfo() { getNativeBinding(); }
  multisigPolicyInfo() { getNativeBinding(); }
}

export function parseCanonicalI105AccountLiteral() { getNativeBinding(); }
export function encodeI105AccountAddress() { getNativeBinding(); }
export function decodeI105AccountAddress() { getNativeBinding(); }
export function inspectAccountId() { getNativeBinding(); }
export function validatePublicKeyForCurve() { getNativeBinding(); }
