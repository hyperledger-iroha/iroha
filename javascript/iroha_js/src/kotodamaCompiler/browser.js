import {
  KotodamaCompilerClient,
  selectCompilerRequestOptions,
  validateCompilerOptions,
  validateCompilerSource,
} from "./client.js";

export { KotodamaCompilerClient } from "./client.js";

/** Compile through an explicitly configured canonical Rust compiler service. */
export async function compileKotodamaProgram(source, options = {}) {
  validateCompilerSource(source);
  options = validateCompilerOptions(options);
  if (!options.compilerUrl) {
    throw new Error(
      "browser Kotodama compilation requires compilerUrl; offline compilation is unsupported",
    );
  }
  return new KotodamaCompilerClient(options.compilerUrl, options).compile(
    source,
    selectCompilerRequestOptions(options),
  );
}
