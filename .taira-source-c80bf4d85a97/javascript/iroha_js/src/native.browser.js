function nativeBindingError(reason) {
  return new Error(`Native binding required; ${reason}`);
}

/**
 * Browser builds cannot load the optional `iroha_js_host.node` binding.
 */
export function getNativeBinding() {
  throw nativeBindingError("iroha_js_host is unavailable in browser builds.");
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
