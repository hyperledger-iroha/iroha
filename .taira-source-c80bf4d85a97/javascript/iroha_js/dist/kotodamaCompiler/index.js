import { getNativeBinding } from "../native.js";
import {
  KotodamaCompilerClient,
  selectCompilerCallOptions,
  selectCompilerRequestOptions,
  validateCompilerOptions,
  validateCompilerSource,
} from "./client.js";
import { compileKotodamaWithNativeBinding } from "./nativeBridge.js";

export { KotodamaCompilerClient } from "./client.js";

/** Compile with the canonical Rust compiler, locally in Node or through its service. */
export async function compileKotodamaProgram(source, options = {}) {
  validateCompilerSource(source);
  options = validateCompilerOptions(options);
  if (options.compilerUrl) {
    return new KotodamaCompilerClient(options.compilerUrl, {
      ...(options.fetchImpl === undefined ? {} : { fetchImpl: options.fetchImpl }),
    }).compile(
      source,
      selectCompilerCallOptions(options),
    );
  }
  if (options.fetchImpl !== undefined) {
    throw new TypeError("fetchImpl requires compilerUrl");
  }
  if (options.signal !== undefined || options.timeoutMs !== undefined) {
    throw new TypeError("signal and timeoutMs require compilerUrl");
  }
  return compileKotodamaWithNativeBinding(
    getNativeBinding(),
    source,
    selectCompilerRequestOptions(options),
  );
}
