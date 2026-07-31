import type { NexusTransactionCodec } from "./nexus-app.js";

/** Browser-safe structural view of the runtime KotodamaQuantity class. */
export interface BrowserKotodamaQuantity {
  readonly mantissa: bigint;
  readonly scale: number;
  toString(): string;
}

export type BrowserTransactionBytes =
  | Uint8Array
  | ArrayBuffer
  | ArrayBufferView;

export type BrowserTransactionUnsigned = bigint | number | string;

export type BrowserFeeChargeKind = "nexus" | "pipelineGas";

export interface BrowserFeeChargeLimit {
  kind: BrowserFeeChargeKind;
  assetDefinitionId: string;
  /** Positive canonical decimal maximum authorized by the signer. */
  maxAmount: BrowserKotodamaQuantity | bigint | string;
}

export type BrowserFeePayment =
  | {
      payer: "authority";
      chargeLimits: readonly BrowserFeeChargeLimit[];
      gasLimit?: BrowserTransactionUnsigned | null;
    }
  | {
      payer: "sponsor";
      programId: string;
      programRevision: BrowserTransactionUnsigned;
      chargeLimits: readonly BrowserFeeChargeLimit[];
      gasLimit?: BrowserTransactionUnsigned | null;
    };

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
  quantity: BrowserKotodamaQuantity | bigint | string;
  destinationAccountId: string;
  /** Required signature-bound fee payer, charge maxima, and gas bound. */
  feePayment: BrowserFeePayment;
  metadata?: string | { readonly [key: string]: BrowserTransactionMetadataValue } | null;
  creationTimeMs?: BrowserTransactionUnsigned;
  ttlMs?: BrowserTransactionUnsigned | null;
  nonce?: BrowserTransactionUnsigned | null;
  networkPrefix?: BrowserTransactionUnsigned;
  chainDiscriminant?: BrowserTransactionUnsigned;
}

export interface BrowserInstructionTransactionInput {
  chainId: string;
  authority: string;
  instructions: readonly object[];
  /** Required signature-bound fee payer, charge maxima, and gas bound. */
  feePayment: BrowserFeePayment;
  metadata?: string | { readonly [key: string]: BrowserTransactionMetadataValue } | null;
  creationTimeMs?: BrowserTransactionUnsigned;
  ttlMs?: BrowserTransactionUnsigned | null;
  nonce?: BrowserTransactionUnsigned | null;
  networkPrefix?: BrowserTransactionUnsigned;
  chainDiscriminant?: BrowserTransactionUnsigned;
}

export type BrowserExecutableBatchEntry =
  | {
      kind: "instruction";
      instruction: object;
    }
  | {
      kind: "contractCall";
      contractAddress: string;
      /** Exact marked 32-byte Iroha code hash. */
      expectedCodeHash: BrowserTransactionBytes | string;
      entrypoint: string;
      /** Canonical schema-bound argument-record bytes; maximum 1 MiB. */
      arguments?: BrowserTransactionBytes | null;
    };

export interface BrowserExecutableBatchInput {
  chainId: string;
  authority: string;
  entries: readonly BrowserExecutableBatchEntry[];
  /** Must include `gasLimit` when any entry is a contract call. */
  feePayment: BrowserFeePayment;
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
  payloadBytes: Uint8Array;
  payloadHashHex: string;
  authority: string;
  signingPublicKey: Uint8Array;
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
  signedTransaction: Uint8Array;
  hash: Uint8Array;
  hashHex: string;
}

export class BrowserTransactionCodecError extends TypeError {
  readonly code: string;
}

export function buildBrowserTransferPayload(input: BrowserTransferInput): Uint8Array;

export function buildBrowserInstructionTransactionPayload(
  input: BrowserInstructionTransactionInput,
): Uint8Array;

export function buildBrowserVerifyingKeyTransactionPayload(
  input: BrowserInstructionTransactionInput,
  operation: "register" | "update",
): Uint8Array;

export function decodeCanonicalVerifyingKeyTransactionPayload(
  payloadBytes: BrowserTransactionBytes,
  constraints: {
    expectedChainId: string;
    expectedAuthority: string;
    operation: "register" | "update";
  },
): {
  id: { backend: string; name: string };
  record: Record<string, unknown>;
};

export function buildBrowserExecutableBatchPayload(
  input: BrowserExecutableBatchInput,
): Uint8Array;

export function browserTransactionPayloadHashHex(
  payloadBytes: BrowserTransactionBytes,
): string;

export function validateBrowserTransferSignable(
  signable: BrowserTransactionSignable,
  constraints?: BrowserTransactionSignableConstraints,
): Readonly<ValidatedBrowserTransactionSignable>;

export function validateBrowserInstructionTransactionSignable(
  signable: BrowserTransactionSignable,
  constraints?: BrowserTransactionSignableConstraints,
): Readonly<ValidatedBrowserTransactionSignable>;

export function validateBrowserExecutableBatchSignable(
  signable: BrowserTransactionSignable,
  constraints?: BrowserTransactionSignableConstraints,
): Readonly<ValidatedBrowserTransactionSignable>;

export function finalizeBrowserSignedTransaction(
  signable: BrowserTransactionSignable,
  signature: BrowserTransactionSignature,
  signingPublicKey: BrowserTransactionBytes | string,
): BrowserFinalizedSignedTransaction;

export function finalizeBrowserInstructionTransaction(
  signable: BrowserTransactionSignable,
  signature: BrowserTransactionSignature,
  signingPublicKey: BrowserTransactionBytes | string,
): BrowserFinalizedSignedTransaction;

export function finalizeBrowserExecutableBatchTransaction(
  signable: BrowserTransactionSignable,
  signature: BrowserTransactionSignature,
  signingPublicKey: BrowserTransactionBytes | string,
): BrowserFinalizedSignedTransaction;

export function browserSignedTransactionHashHex(
  signedTransaction: BrowserTransactionBytes,
): string;

export const browserTransactionCodec: Readonly<NexusTransactionCodec> & Readonly<{
  buildTransferPayload: typeof buildBrowserTransferPayload;
  buildInstructionPayload: typeof buildBrowserInstructionTransactionPayload;
  buildExecutableBatchPayload: typeof buildBrowserExecutableBatchPayload;
  payloadHashHex: typeof browserTransactionPayloadHashHex;
  finalizeSignedTransaction: typeof finalizeBrowserSignedTransaction;
  finalizeInstructionTransaction: typeof finalizeBrowserInstructionTransaction;
  finalizeExecutableBatchTransaction: typeof finalizeBrowserExecutableBatchTransaction;
  validateSignable: typeof validateBrowserTransferSignable;
  validateInstructionSignable: typeof validateBrowserInstructionTransactionSignable;
  validateExecutableBatchSignable: typeof validateBrowserExecutableBatchSignable;
}>;
