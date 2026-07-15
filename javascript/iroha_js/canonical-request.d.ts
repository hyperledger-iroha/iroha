import type { Buffer } from "buffer";

export type CanonicalRequestBytes = Buffer | ArrayBuffer | ArrayBufferView;

export interface CanonicalRequestMessageInput {
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: CanonicalRequestBytes | string;
}

export interface CanonicalRequestSignatureMessageInput
  extends CanonicalRequestMessageInput {
  timestampMs: number;
  nonce: string;
}

export interface CanonicalRequestHeadersInput
  extends CanonicalRequestMessageInput {
  /** Exact canonical ASCII account alias used as the X-Iroha-Account credential. */
  accountId: string;
  privateKey: CanonicalRequestBytes;
  timestampMs?: number;
  nonce?: string;
}

export interface CanonicalRequestHeaders {
  "X-Iroha-Account": string;
  "X-Iroha-Signature": string;
  "X-Iroha-Timestamp-Ms": string;
  "X-Iroha-Nonce": string;
}

export interface CanonicalJsonRequestSignerInput {
  message: Buffer;
  messageBase64: string;
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body: string;
  timestampMs: number;
  nonce: string;
}

export type CanonicalJsonRequestSignature =
  | Buffer
  | Uint8Array
  | ArrayBuffer
  | ArrayBufferView
  | string;

export interface CanonicalJsonRequestInput {
  /** Exact canonical ASCII account alias used as the X-Iroha-Account credential. */
  accountId: string;
  method?: string;
  path: string;
  baseUrl?: string;
  query?: string | URLSearchParams;
  body?: unknown;
  headers?:
    | Headers
    | ReadonlyArray<readonly [string, string]>
    | Record<string, string>;
  privateKey?: CanonicalRequestBytes;
  sign?: (
    input: CanonicalJsonRequestSignerInput,
  ) => CanonicalJsonRequestSignature | Promise<CanonicalJsonRequestSignature>;
  timestampMs?: number;
  nonce?: string;
}

export interface CanonicalJsonRequest {
  method: string;
  headers: Record<string, string>;
  body: string;
}

export function canonicalQueryString(
  query?: string | URLSearchParams | null,
): string;

export function canonicalRequestMessage(
  params: CanonicalRequestMessageInput,
): Buffer;

export function canonicalRequestSignatureMessage(
  params: CanonicalRequestSignatureMessageInput,
): Buffer;

export function buildCanonicalRequestHeaders(
  params: CanonicalRequestHeadersInput,
): CanonicalRequestHeaders;

export function buildCanonicalJsonRequest(
  params: CanonicalJsonRequestInput,
): Promise<CanonicalJsonRequest>;
