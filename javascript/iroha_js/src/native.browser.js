import { getBrowserCodecBinding } from "./browserCodec.js";

/**
 * Browser builds use the package-owned Rust Wasm codec after explicit initialization.
 */
export function getNativeBinding() {
  return getBrowserCodecBinding();
}

/**
 * Native binding verification is only meaningful in Node.js.
 */
export function verifyNativeBinding(
  bindingPath,
  { manifestPath, platformKey } = {},
) {
  return {
    ok: false,
    status: "browser_unavailable",
    path: bindingPath,
    manifestPath,
    platform: platformKey ?? "browser",
  };
}

/**
 * Reset cached native state (test helper).
 * @internal
 */
export function __resetNativeStateForTests() {}
