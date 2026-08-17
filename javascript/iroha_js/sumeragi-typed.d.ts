import type {
  ToriiSumeragiDiagnostics,
  ToriiSumeragiStatus,
  ToriiSumeragiV2QcResponse,
} from "./index.js";

/** Maximum bytes accepted from the typed Sumeragi status endpoint. */
export const SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES: 1048576;
/** Maximum bytes accepted from the typed Sumeragi diagnostics endpoint. */
export const SUMERAGI_DIAGNOSTICS_TYPED_JSON_MAX_BYTES: 16777216;

export function parseSumeragiStatusJson(
  text: string,
  context?: string,
): ToriiSumeragiStatus;
export function parseSumeragiDiagnosticsJson(
  text: string,
  context?: string,
): ToriiSumeragiDiagnostics;
export function parseSumeragiStatusPayload(payload: unknown): ToriiSumeragiStatus;
export function parseSumeragiDiagnosticsPayload(
  payload: unknown,
): ToriiSumeragiDiagnostics;
export function parseSumeragiV2QcResponse(
  payload: unknown,
): ToriiSumeragiV2QcResponse;

/** Native-AMX fixture resealing helpers used by the direct typed-parser tests. */
export const __sumeragiNativeAmxTestHelpers: Readonly<{
  computeDescriptorHash(value: unknown): string;
  computeParticipantSettlementHash(value: unknown): string;
  computeProposalHash(value: unknown): string;
  computeValidatorSetHash(value: unknown): string;
}>;
