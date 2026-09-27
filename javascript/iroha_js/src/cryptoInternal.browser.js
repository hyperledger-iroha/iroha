// Internal browser resolution for source modules that import ./crypto.js.
// The public ./crypto browser export remains the closed crypto.browser.js surface.
export * from "./crypto.browser.js";

import { publicKeyFromPrivate } from "./crypto.browser.js";

/** @internal Transaction's browser context uses the local Ed25519 operation only. */
export function _createCryptoApi(_nativeRuntime) {
  return Object.freeze({ publicKeyFromPrivate });
}
