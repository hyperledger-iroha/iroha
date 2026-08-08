import type { Buffer } from "buffer";

import type {
  BrowserFeePayment,
  BrowserTransactionMetadataValue,
  BrowserTransactionUnsigned,
} from "./transaction-codec.js";
import type { IvmArtifactAdmissionWasmVerifier } from "./ivm-artifact-admission-wasm.js";
import type { NetworkId } from "./index.js";

export const SMART_CONTRACT_CODE_CHUNK_BYTES: 65536;

export interface ContractAddressDerivationInput {
  chainId: string;
  chainDiscriminant: BrowserTransactionUnsigned;
  authority: string;
  deployNonce: BrowserTransactionUnsigned;
  dataspaceId: BrowserTransactionUnsigned;
}

export function deriveContractAddress(
  input: ContractAddressDerivationInput,
): string;

export interface BrowserContractArtifactInput {
  artifactBytes: Uint8Array | ArrayBuffer | ArrayBufferView | Buffer;
  manifest: Record<string, unknown>;
  compilerCodeHash: string | Uint8Array | ArrayBuffer | ArrayBufferView | Buffer;
  compilerAbiHash: string | Uint8Array | ArrayBuffer | ArrayBufferView | Buffer;
}

export interface BrowserContractDeploymentStep {
  readonly kind:
    | "upload_chunk"
    | "finalize_upload"
    | "register_manifest"
    | "commit_deployment";
  readonly index?: number;
  readonly instruction: object;
}

export interface PreparedBrowserContractArtifact {
  readonly artifactBytes: Uint8Array;
  readonly artifactSha256Hex: string;
  readonly codeHash: string;
  readonly abiHash: string;
  readonly manifest: Record<string, unknown>;
  readonly chunkCount: number;
  readonly steps: readonly BrowserContractDeploymentStep[];
}

export function prepareBrowserContractArtifact(
  input: BrowserContractArtifactInput,
): PreparedBrowserContractArtifact;

export interface BrowserDeploymentSignerInput {
  readonly payloadBytes: Uint8Array;
  readonly payloadHashHex: string;
  readonly payloadHashBytes: Uint8Array;
  readonly authority: string;
  readonly signingPublicKey: Uint8Array;
  readonly signatureAlgorithm: "ed25519";
  readonly step: BrowserContractDeploymentStep;
}

export interface BrowserManifestSignerInput {
  readonly payloadBytes: Uint8Array;
  readonly payloadBase64: string;
  readonly signingPublicKey: Uint8Array;
  readonly signatureAlgorithm: "ed25519";
  readonly manifest: Record<string, unknown>;
  readonly codeHash: string;
  readonly abiHash: string;
}

export type BrowserDeploymentSignature =
  | Uint8Array
  | ArrayBuffer
  | ArrayBufferView
  | Buffer
  | string
  | {
      algorithm?: "ed25519" | 0;
      signature?: Uint8Array | ArrayBuffer | ArrayBufferView | Buffer | string;
      bytes?: Uint8Array | ArrayBuffer | ArrayBufferView | Buffer | string;
      payload?: Uint8Array | ArrayBuffer | ArrayBufferView | Buffer | string;
    };

export interface BrowserDeploymentSubmission {
  readonly signedTransaction: Uint8Array;
  readonly hashHex: string;
  readonly step: BrowserContractDeploymentStep;
}

export interface BrowserDeploymentNodeCapabilities {
  abi_version: 1;
  data_model_version: 4;
  signed_transaction_schema_hash_hex: string;
  readonly [key: string]: unknown;
}

export interface ValidatedBrowserDeploymentNodeCapabilities {
  readonly abiVersion: 1;
  readonly dataModelVersion: 4;
  readonly signedTransactionSchemaHashHex: string;
}

export interface BrowserDeploymentStateRequest {
  authority: string;
  contract_alias: string;
}

export interface BrowserDeploymentStateResponse {
  authority: string;
  contract_alias: string;
  deploy_nonce: string;
  dataspace_alias: string;
  dataspace_id: string;
  previous_contract_address: string | null;
  observed_block_height: string;
  observed_block_hash: string;
  ledger_time_ms: string;
  chain_discriminant: string;
}

export interface BrowserContractDeploymentOptions
  extends BrowserContractArtifactInput {
  artifactAdmissionVerifier: IvmArtifactAdmissionWasmVerifier;
  /** Exact genesis-derived domain for every deployment transaction. */
  networkId: NetworkId;
  chain?: never;
  chain_id?: never;
  /** Canonical ChainId committed only into contract-address derivation. */
  chainId: string;
  chainDiscriminant: BrowserTransactionUnsigned;
  authority: string;
  contractAlias: string;
  leaseExpiryMs?: BrowserTransactionUnsigned | null;
  ttlMs?: BrowserTransactionUnsigned | null;
  /** Static signature-bound fee intent used for every deployment step. */
  feePayment?: BrowserFeePayment;
  /** Per-step fee intent, typically produced from an authenticated fee quote. */
  feePaymentForStep?: (
    step: BrowserContractDeploymentStep,
  ) => BrowserFeePayment | Promise<BrowserFeePayment>;
  metadata?:
    | string
    | { readonly [key: string]: BrowserTransactionMetadataValue }
    | null;
  clock?: () => BrowserTransactionUnsigned;
  nonceForStep?: (
    step: BrowserContractDeploymentStep,
    sequence: number,
  ) => BrowserTransactionUnsigned | null | Promise<BrowserTransactionUnsigned | null>;
  metadataForStep?: (
    step: BrowserContractDeploymentStep,
  ) =>
    | string
    | { readonly [key: string]: BrowserTransactionMetadataValue }
    | null
    | Promise<
        | string
        | { readonly [key: string]: BrowserTransactionMetadataValue }
        | null
      >;
  sign: (
    input: BrowserDeploymentSignerInput,
  ) => BrowserDeploymentSignature | Promise<BrowserDeploymentSignature>;
  signManifest: (
    input: BrowserManifestSignerInput,
  ) => BrowserDeploymentSignature | Promise<BrowserDeploymentSignature>;
  readNodeCapabilities: (input: {
    readonly chainId: string;
    readonly chainDiscriminant: string;
  }) =>
    | BrowserDeploymentNodeCapabilities
    | Promise<BrowserDeploymentNodeCapabilities>;
  submitAndWait: (
    input: BrowserDeploymentSubmission,
  ) => unknown | Promise<unknown>;
  readDeploymentState: (
    input: BrowserDeploymentStateRequest,
  ) => BrowserDeploymentStateResponse | Promise<BrowserDeploymentStateResponse>;
}

export interface BrowserContractDeploymentResult {
  readonly contractAddress: string;
  readonly contractAlias: string;
  readonly codeHash: string;
  readonly abiHash: string;
  readonly artifactSha256Hex: string;
  readonly deployNonce: string;
  readonly dataspaceId: string;
  readonly previousContractAddress: string | null;
  readonly observedBlockHeight: string;
  readonly observedBlockHash: string;
  readonly observedBlockHashHex: string;
  readonly ledgerTimeMs: string;
  readonly nodeCapabilities: ValidatedBrowserDeploymentNodeCapabilities;
  readonly artifactAdmission: Readonly<{
    verifierSha256Hex: string;
    headerLength: number;
    codeOffset: number;
    entrypointCount: number;
  }>;
  readonly transactions: readonly Readonly<{
    kind: BrowserContractDeploymentStep["kind"];
    hashHex: string;
    status: unknown;
  }>[];
}

export function deploySmartContractBrowser(
  options: BrowserContractDeploymentOptions,
): Promise<BrowserContractDeploymentResult>;
