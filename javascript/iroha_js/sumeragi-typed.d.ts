import type { ToriiSumeragiStatus } from "./index.js";
/** Maximum bytes accepted from the native status endpoint. */
export const SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES: 1048576;
/** Decode strict unsigned JSON; this observation is not a finality proof. */
export function parseSumeragiStatusJson(text: string, context?: string): ToriiSumeragiStatus;
/** Validate the exact native protocol-8 payload. */
export function parseSumeragiStatusPayload(payload: unknown): ToriiSumeragiStatus;
