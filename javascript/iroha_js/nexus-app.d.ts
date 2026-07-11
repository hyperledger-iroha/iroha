import type { Buffer } from "buffer";

export interface NexusAppConfig {
  chainId?: string;
  baseUrl?: string;
  toriiBaseUrl?: string;
  connectBaseUrl?: string;
  node?: string | null;
  authority?: string;
  accountId?: string;
  signingPublicKey?: Buffer | Uint8Array | ArrayBuffer | string | null;
  fetchImpl?: typeof fetch;
  webSocketImpl?: unknown;
  allowInsecure?: boolean;
  appMeta?: unknown;
  appMetadata?: unknown;
  permissions?: unknown;
  connectTransport?: NexusConnectTransport;
  connect?: NexusConnectTransport;
  transactionCodec?: NexusTransactionCodec;
  toriiClient?: NexusToriiClient;
}

export interface NexusConnectOptions {
  sid?: string;
  chainId?: string;
  node?: string | null;
  appKeyPair?: unknown;
  nonce?: Buffer | Uint8Array | ArrayBuffer | string;
  protocol?: string;
}

export interface NexusConnectSession {
  sid: string;
  walletLaunchUri?: string | null;
  appLaunchUri?: string | null;
  tokenApp?: string | null;
  tokenWallet?: string | null;
  tokenManagement?: string | null;
  tokenRelay?: string | null;
  approvedAccountId?: string | null;
  approvedAccount?: string | null;
  approved_account?: string | null;
  signingPublicKey?: Buffer | null;
  signing_public_key?: Buffer | Uint8Array | ArrayBuffer | string | null;
  appSession?: unknown;
  preview?: unknown;
}

export interface NexusApprovedAccount {
  accountId: string;
  signingPublicKey: Buffer;
  session: NexusConnectSession;
}

export interface NexusTransferInput {
  chainId?: string;
  authority?: string;
  accountId?: string;
  sourceAccountId?: string;
  sourceAssetHoldingId?: string;
  sourceAssetId?: string;
  assetId?: string;
  quantity: string | number | bigint;
  destinationAccountId?: string;
  destination?: string;
  to?: string;
  metadata?: Record<string, unknown> | string | null;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  signingPublicKey?: Buffer | Uint8Array | ArrayBuffer | string | null;
}

export interface NexusSignableTransaction {
  payloadBytes: Buffer;
  payloadHashHex: string;
  authority: string;
  signingPublicKey: Buffer | null;
  signatureAlgorithm: "ed25519";
}

export interface NexusTransferDraft {
  input: Record<string, unknown>;
  signable: NexusSignableTransaction;
}

export interface NexusWalletSignature {
  algorithm?: "ed25519" | "0" | 0;
  signature?: Buffer | Uint8Array | ArrayBuffer | string;
  bytes?: Buffer | Uint8Array | ArrayBuffer | string;
  payload?: Buffer | Uint8Array | ArrayBuffer | string;
}

export type NexusBytes = Buffer | Uint8Array | ArrayBuffer | string;

export interface NexusTransactionPayloadResult {
  payloadBytes?: NexusBytes;
  payload_bytes?: NexusBytes;
  bytes?: NexusBytes;
  payloadHashHex?: string;
  payload_hash_hex?: string;
  hashHex?: string;
  hash_hex?: string;
  hash?: string | Buffer | Uint8Array | ArrayBuffer;
}

export interface NexusFinalizedTransactionResult {
  signedTransaction?: NexusBytes;
  signed_transaction?: NexusBytes;
  bytes?: NexusBytes;
  hashHex?: string;
  hash_hex?: string;
  transactionHashHex?: string;
  transaction_hash_hex?: string;
  signedTransactionHashHex?: string;
  signed_transaction_hash_hex?: string;
  signedTransactionHash?: string | Buffer | Uint8Array | ArrayBuffer;
  signed_transaction_hash?: string | Buffer | Uint8Array | ArrayBuffer;
  hash?: string | Buffer | Uint8Array | ArrayBuffer;
}

export interface NexusFinalizeBaseOptions {
  signingPublicKey?: Buffer | Uint8Array | ArrayBuffer | string;
  toriiClient?: NexusToriiClient;
}

/** A non-string iterable of exact Torii pipeline status labels. */
export type NexusStatusIterable = Iterable<string> & object;

export interface NexusWaitFinalizeOptions extends NexusFinalizeBaseOptions {
  wait?: true;
  intervalMs?: number;
  timeoutMs?: number | null;
  maxAttempts?: number | null;
  scope?: "local" | "auto" | "global";
  /** At most 32 raw entries are consumed before duplicate removal. */
  successStatuses?: NexusStatusIterable;
  /** At most 32 raw entries are consumed before duplicate removal. */
  failureStatuses?: NexusStatusIterable;
  onStatus?: (
    status: string | null,
    payload: unknown,
    attempt: number,
  ) => void | Promise<void>;
  signal?: AbortSignal;
}

export interface NexusNoWaitFinalizeOptions extends NexusFinalizeBaseOptions {
  wait: false;
  intervalMs?: never;
  timeoutMs?: never;
  maxAttempts?: never;
  scope?: never;
  successStatuses?: never;
  failureStatuses?: never;
  onStatus?: never;
  signal?: never;
}

export type NexusFinalizeOptions =
  | NexusWaitFinalizeOptions
  | NexusNoWaitFinalizeOptions;

export interface NexusTransferReceipt {
  signedTransaction: Buffer;
  signedTransactionHashHex: string;
  submission: unknown;
  status: unknown;
}

export interface NexusConnectTransport {
  startConnect?(
    options: NexusConnectOptions,
    config: NexusAppConfig,
  ): Promise<NexusConnectSession> | NexusConnectSession;
  awaitApproval?(
    session: NexusConnectSession,
    config: NexusAppConfig,
  ): Promise<{ accountId?: string; account_id?: string; signingPublicKey?: unknown; signing_public_key?: unknown; session?: NexusConnectSession }> | { accountId?: string; account_id?: string; signingPublicKey?: unknown; signing_public_key?: unknown; session?: NexusConnectSession };
  requestSignature?(
    session: NexusConnectSession,
    signable: NexusSignableTransaction,
    config: NexusAppConfig,
  ): Promise<NexusWalletSignature | Buffer | Uint8Array | string> | NexusWalletSignature | Buffer | Uint8Array | string;
}

export interface NexusTransactionCodec {
  /** Returned hash aliases, when present, must exactly match the canonical payload prehash. */
  buildTransferPayload(input: Record<string, unknown>): NexusBytes | NexusTransactionPayloadResult;
  /** Must return exactly 64 lowercase hex characters matching the supplied payload bytes. */
  payloadHashHex?(payloadBytes: Buffer): string;
  /** Must return canonical version-1 single-signature Transfer::Asset bytes and their exact hash. */
  finalizeSignedTransaction(
    signable: NexusSignableTransaction,
    signature: { algorithm: "ed25519"; signature: Buffer },
    signingPublicKey: Buffer,
  ): NexusFinalizedTransactionResult;
}

export interface NexusToriiClient {
  submitTransaction(payload: Buffer): Promise<unknown>;
  /** Required when finalize options omit `wait` or set it to `true`. */
  waitForTransactionStatus?(hashHex: string, options?: Record<string, unknown>): Promise<unknown>;
}

export type NexusAppErrorPhase =
  | "validation"
  | "finalization"
  | "submission"
  | "status_wait";

export type NexusSubmissionState =
  | "not_submitted"
  | "unknown"
  | "submitted";

export interface NexusAppErrorContext {
  phase?: NexusAppErrorPhase;
  submissionState?: NexusSubmissionState;
  signedTransactionHashHex?: string;
  submission?: unknown;
  status?: unknown;
}

export class NexusAppError extends Error {
  readonly code: string;
  readonly cause?: unknown;
  readonly phase: NexusAppErrorPhase;
  readonly submissionState: NexusSubmissionState;
  readonly signedTransactionHashHex?: string;
  readonly submission?: unknown;
  readonly status?: unknown;
  constructor(
    code: string,
    message: string,
    cause?: unknown,
    context?: NexusAppErrorContext,
  );
}

export class NexusAppClient {
  constructor(config?: NexusAppConfig);
  startConnect(options?: NexusConnectOptions): Promise<NexusConnectSession>;
  awaitApproval(session: NexusConnectSession): Promise<NexusApprovedAccount>;
  buildTransferDraft(input: NexusTransferInput): NexusTransferDraft;
  requestSignature(
    session: NexusConnectSession,
    signable: NexusSignableTransaction,
  ): Promise<{ algorithm: "ed25519"; signature: Buffer }>;
  finalizeAndSubmit(
    signable: NexusSignableTransaction,
    signature: NexusWalletSignature | Buffer | Uint8Array | string,
    options?: NexusFinalizeOptions,
  ): Promise<NexusTransferReceipt>;
  transferWithWallet(
    session: NexusConnectSession,
    input: NexusTransferInput,
    options?: NexusFinalizeOptions,
  ): Promise<NexusTransferReceipt>;
}

export const NexusSignatureAlgorithmEd25519: "ed25519";
export function nexusPayloadHashHex(payloadBytes: Buffer | Uint8Array | ArrayBuffer): string;
export { validateBrowserTransferSignable } from "./transaction-codec.js";
