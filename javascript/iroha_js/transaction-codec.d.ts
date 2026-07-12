import type { Buffer } from "buffer";
import type { NexusTransactionCodec } from "./nexus-app.js";

export type BrowserTransactionBytes =
  | Buffer
  | ArrayBuffer
  | ArrayBufferView;

export type BrowserTransactionUnsigned = bigint | number | string;

/** `number` leaves must be safe integers; encode decimals as strings. */
export type BrowserTransactionMetadataValue =
  | null
  | boolean
  | number
  | string
  | readonly BrowserTransactionMetadataValue[]
  | { readonly [key: string]: BrowserTransactionMetadataValue };

export interface BrowserTransferInput {
  chainId: string;
  authority: string;
  sourceAssetHoldingId?: string;
  sourceAssetId?: string;
  /** Positive canonical decimal, scale <= 28, and value <= 2^511 - 1. */
  quantity: bigint | number | string;
  destinationAccountId: string;
  metadata?: string | { readonly [key: string]: BrowserTransactionMetadataValue } | null;
  creationTimeMs?: BrowserTransactionUnsigned;
  ttlMs?: BrowserTransactionUnsigned | null;
  nonce?: BrowserTransactionUnsigned | null;
  networkPrefix?: BrowserTransactionUnsigned;
  chainDiscriminant?: BrowserTransactionUnsigned;
}

export interface BrowserTransactionSignable {
  payloadBytes: BrowserTransactionBytes;
  payloadHashHex?: string;
  authority: string;
  signingPublicKey?: BrowserTransactionBytes | string;
  signatureAlgorithm?: "ed25519" | 0;
}

export interface BrowserTransactionSignableConstraints {
  authority?: string | null;
  signingPublicKey?: BrowserTransactionBytes | string | null;
}

export interface ValidatedBrowserTransactionSignable {
  payloadBytes: Buffer;
  payloadHashHex: string;
  authority: string;
  signingPublicKey: Buffer;
  signatureAlgorithm: "ed25519";
}

export interface BrowserTransactionSignatureObject {
  algorithm?: "ed25519" | 0;
  alg?: "ed25519" | 0;
  signature?: BrowserTransactionBytes | string;
  bytes?: BrowserTransactionBytes | string;
  payload?: BrowserTransactionBytes | string;
}

export type BrowserTransactionSignature =
  | BrowserTransactionBytes
  | BrowserTransactionSignatureObject;

export interface BrowserFinalizedSignedTransaction {
  signedTransaction: Buffer;
  hash: Buffer;
  hashHex: string;
}

export class BrowserTransactionCodecError extends TypeError {
  readonly code: string;
}

export function buildBrowserTransferPayload(input: BrowserTransferInput): Buffer;

export function browserTransactionPayloadHashHex(
  payloadBytes: BrowserTransactionBytes,
): string;

export function validateBrowserTransferSignable(
  signable: BrowserTransactionSignable,
  constraints?: BrowserTransactionSignableConstraints,
): Readonly<ValidatedBrowserTransactionSignable>;

export function finalizeBrowserSignedTransaction(
  signable: BrowserTransactionSignable,
  signature: BrowserTransactionSignature,
  signingPublicKey: BrowserTransactionBytes | string,
): BrowserFinalizedSignedTransaction;

export function browserSignedTransactionHashHex(
  signedTransaction: BrowserTransactionBytes,
): string;

export const browserTransactionCodec: Readonly<NexusTransactionCodec> & Readonly<{
  buildTransferPayload: typeof buildBrowserTransferPayload;
  payloadHashHex: typeof browserTransactionPayloadHashHex;
  finalizeSignedTransaction: typeof finalizeBrowserSignedTransaction;
  validateSignable: typeof validateBrowserTransferSignable;
}>;
