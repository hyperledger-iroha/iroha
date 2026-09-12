/** Initialization or ABI failure of the package-owned Rust browser codec. */
export class BrowserCodecError extends Error {
  readonly code: "ERR_IROHA_CODEC_NOT_READY" | "ERR_IROHA_CODEC_INITIALIZATION" | "ERR_IROHA_CODEC_ABI";
}

/**
 * Load the packaged Rust Wasm codec before using synchronous account and
 * instruction APIs. Concurrent calls share initialization. A failed attempt
 * can be retried; a successful binding remains immutable for the page lifetime.
 */
export function initializeBrowserCodec(): Promise<void>;
