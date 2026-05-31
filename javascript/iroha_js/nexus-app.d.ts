/// <reference types="node" />

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
  algorithm?: "ed25519" | "Ed25519" | 0;
  signature?: Buffer | Uint8Array | ArrayBuffer | string;
  bytes?: Buffer | Uint8Array | ArrayBuffer | string;
  payload?: Buffer | Uint8Array | ArrayBuffer | string;
}

export interface NexusFinalizeOptions {
  wait?: boolean;
  intervalMs?: number;
  timeoutMs?: number;
  maxAttempts?: number;
  scope?: "local" | "auto" | "global";
  successStatuses?: readonly string[];
  failureStatuses?: readonly string[];
  onStatus?: (status: unknown) => void;
  signingPublicKey?: Buffer | Uint8Array | ArrayBuffer | string;
  toriiClient?: NexusToriiClient;
}

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
  buildTransferPayload(input: Record<string, unknown>): Buffer | Uint8Array | ArrayBuffer | string;
  payloadHashHex?(payloadBytes: Buffer): string;
  finalizeSignedTransaction(
    signable: NexusSignableTransaction,
    signature: { algorithm: "ed25519"; signature: Buffer },
    signingPublicKey: Buffer,
  ): Buffer | { signedTransaction?: Buffer | Uint8Array | string; signed_transaction?: Buffer | Uint8Array | string; bytes?: Buffer | Uint8Array | string; hash?: Buffer | Uint8Array | string; hashHex?: string; hash_hex?: string };
}

export interface NexusToriiClient {
  submitTransaction(payload: Buffer): Promise<unknown>;
  waitForTransactionStatus?(hashHex: string, options?: Record<string, unknown>): Promise<unknown>;
}

export class NexusAppError extends Error {
  readonly code: string;
  readonly cause?: unknown;
  constructor(code: string, message: string, cause?: unknown);
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
