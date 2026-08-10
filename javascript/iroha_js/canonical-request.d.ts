import type { Buffer } from "buffer";

/** Exact immutable genesis-header hash used as the canonical request domain. */
export class NetworkId {
  private constructor();
  static readonly BYTE_LENGTH: 32;
  static parse(literal: string): NetworkId;
  static fromBytes(value: ArrayBuffer | ArrayBufferView): NetworkId;
  readonly literal: string;
  toBytes(): Uint8Array;
  equals(other: unknown): other is NetworkId;
  toString(): string;
  toJSON(): string;
}

export type CanonicalRequestBytes = Buffer | ArrayBuffer | ArrayBufferView;

export interface CanonicalRequestMessageInput {
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: CanonicalRequestBytes | string;
}

export interface CanonicalRequestSignatureMessageInput
  extends CanonicalRequestMessageInput {
  /** Exact genesis-derived network identity included in the signed preimage. */
  networkId: NetworkId;
  timestampMs: number;
  nonce: string;
}

export interface CanonicalRequestHeadersInput
  extends CanonicalRequestMessageInput {
  /** Exact canonical I105 account or active ASCII alias used as X-Iroha-Account. */
  accountId: string;
  /** Exact genesis-derived network domain for every authenticated route. */
  networkId: NetworkId;
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
  networkId: NetworkId;
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
  /** Exact canonical I105 account or active ASCII alias used as X-Iroha-Account. */
  accountId: string;
  /** Exact genesis-derived network domain for the signed request. */
  networkId: NetworkId;
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
