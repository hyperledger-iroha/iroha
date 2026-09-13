/**
 * Canonical native account and instruction codecs are unavailable in browsers.
 */
export function getNativeBinding() {
  const error = new Error(
    "Native binding required; the canonical Rust codec is unavailable in browsers",
  );
  Object.defineProperties(error, {
    code: { value: "ERR_IROHA_NATIVE_BINDING", enumerable: true },
    nativeStatus: { value: "browser_unavailable", enumerable: true },
  });
  throw error;
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
