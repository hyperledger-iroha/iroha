import {
  createBrowserCodecRuntime,
  fetchBrowserCodecBytes,
} from "./browserCodecRuntime.js";
export { BrowserCodecError } from "./browserCodecRuntime.js";

const runtime = createBrowserCodecRuntime(async () => {
  const module = await import("./wasm/iroha_js_codec_wasm.js");
  const bytes = await fetchBrowserCodecBytes(
    new URL("./wasm/iroha_js_codec_wasm_bg.wasm", import.meta.url),
  );
  await module.default({ module_or_path: bytes });
  return module;
});

/** Load the package-owned Rust codec before synchronous account or instruction operations. */
export function initializeBrowserCodec(...args) {
  if (args.length !== 0) throw new TypeError("initializeBrowserCodec accepts no arguments");
  return runtime.initialize();
}

/** @internal Used only by the browser-selected canonical owner loader. */
export function getBrowserCodecBinding() {
  return runtime.binding();
}
