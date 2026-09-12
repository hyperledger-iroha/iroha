import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve } from "node:path";
import { createBrowserCodecRuntime } from "../src/browserCodecRuntime.js";

/** Verify actual packaged glue and Wasm before packing, including all six exports. */
export async function verifyBrowserCodec(directory) {
  const glue = resolve(directory, "wasm/iroha_js_codec_wasm.js");
  const wasm = resolve(directory, "wasm/iroha_js_codec_wasm_bg.wasm");
  const names = readdirSync(resolve(directory, "wasm")).sort();
  if (names.join(",") !== "iroha_js_codec_wasm.js,iroha_js_codec_wasm_bg.wasm") {
    throw new Error("Browser codec package must contain exactly the reviewed glue and Wasm pair");
  }
  for (const path of [glue, wasm]) {
    const metadata = lstatSync(path);
    if (!metadata.isFile() || metadata.isSymbolicLink()) {
      throw new Error("Browser codec package artifacts must be regular files");
    }
  }
  const bytes = readFileSync(wasm);
  if (bytes.length > 64 * 1024 * 1024 || !WebAssembly.validate(bytes)) {
    throw new Error("Browser codec package requires a valid Wasm module within the 64 MiB download budget");
  }
  const runtime = createBrowserCodecRuntime(async () => {
    const module = await import(pathToFileURL(glue).href);
    await module.default({ module_or_path: bytes });
    return module;
  });
  await runtime.initialize();
  runtime.binding();
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await verifyBrowserCodec(fileURLToPath(new URL("../dist", import.meta.url)));
  process.stdout.write("[browser-codec] packaged Wasm initialized with all six canonical owner methods\n");
}
