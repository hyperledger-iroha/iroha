export type CanonicalContractPayloadValue =
  | null
  | boolean
  | string
  | number
  | readonly CanonicalContractPayloadValue[]
  | { readonly [key: string]: CanonicalContractPayloadValue };

export const CONTRACT_PAYLOAD_MAX_CANONICAL_BYTES: 1048576;
export const CONTRACT_PAYLOAD_MAX_DEPTH: 128;
export const CONTRACT_PAYLOAD_MAX_NODES: 1000000;

/**
 * Return Torii's exact compact canonical JSON for the browser-safe contract payload profile.
 * Numbers must be safe integers other than negative zero; encode decimal and wide values as their
 * canonical contract-schema strings. `null` and `undefined` represent an absent optional payload.
 */
export function canonicalContractPayloadJson(
  payload?: CanonicalContractPayloadValue | null,
): string | null;

/** Return BLAKE3(canonical payload JSON), or BLAKE3(empty bytes) for an absent payload. */
export function contractPayloadDigestHex(
  payload?: CanonicalContractPayloadValue | null,
): string;
