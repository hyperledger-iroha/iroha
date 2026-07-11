import { normalizeCompilerResult } from "./normalize.js";
import { buildCompilerRequest } from "./client.js";

/**
 * Delegate one compilation to the native `iroha_js_host` binding.
 *
 * This internal seam keeps the Node adapter independently testable without
 * embedding a second compiler or requiring a platform-specific `.node` file.
 */
export async function compileKotodamaWithNativeBinding(native, source, options = {}) {
  if (typeof native?.compileKotodama !== "function") {
    throw new Error(
      "native binding is missing compileKotodama; rebuild iroha_js_host for this SDK version",
    );
  }
  const request = buildCompilerRequest(source, options);
  return normalizeCompilerResult(await native.compileKotodama(request));
}
