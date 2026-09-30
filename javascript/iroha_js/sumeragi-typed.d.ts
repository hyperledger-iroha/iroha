import type { ToriiSumeragiLaneStatus, ToriiSumeragiStatus } from "./index.js";
/** Maximum bytes accepted from the native status endpoint. */
export const SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES: 1048576;
/** Decode strict unsigned JSON; this observation is not a finality proof. */
export function parseSumeragiStatusJson(text: string, context?: string): ToriiSumeragiStatus;
/** Validate the exact native protocol-1 payload. */
export function parseSumeragiStatusPayload(payload: unknown): ToriiSumeragiStatus;
/** Maximum bytes accepted from the native lane list endpoint. */
export const SUMERAGI_LANES_TYPED_JSON_MAX_BYTES: 16777216;
/** Decode the strict `GET /v1/sumeragi/lanes` JSON list; lane statuses are not finality proofs. */
export function parseSumeragiLanesJson(text: string, context?: string): ReadonlyArray<ToriiSumeragiLaneStatus>;
/** Validate the exact native lane list payload. */
export function parseSumeragiLanesPayload(payload: unknown): ReadonlyArray<ToriiSumeragiLaneStatus>;
