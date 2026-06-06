/// <reference types="node" />

export * from "./kotodama-compiler";

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export type CryptoAlgorithm =
  | "ed25519"
  | "secp256k1"
  | "bls_normal"
  | "bls_small"
  | "ml-dsa"
  | "gost3410-2012-256-paramset-a"
  | "gost3410-2012-256-paramset-b"
  | "gost3410-2012-256-paramset-c"
  | "gost3410-2012-512-paramset-a"
  | "gost3410-2012-512-paramset-b"
  | "sm2";

export type PrivacyCriterionKey =
  | "hide_amount"
  | "hide_sender"
  | "hide_receiver"
  | "hide_asset_type"
  | "post_quantum";

export interface PrivacyPqLayers {
  readonly proof: boolean;
  readonly authorization: boolean;
  readonly noteEncryption: boolean;
}

export interface PrivacyProductionGate {
  readonly version: string;
  readonly ready: boolean;
  readonly gates: Readonly<Record<string, boolean>>;
  readonly missing: readonly string[];
  readonly auditReferences: readonly Readonly<{ label: string; url: string }>[];
}

export type PrivacyAlgorithmCategory =
  | "payment"
  | "authorization"
  | "credential"
  | "admission"
  | "identity"
  | "proof_backend";

export type PrivacyAlgorithmMaturity =
  | "peer_reviewed"
  | "accepted_conference"
  | "technical_report"
  | "arxiv_preprint"
  | "specification";

export interface PrivacyAlgorithmDescriptor {
  readonly id: string;
  readonly name: string;
  readonly shortName: string;
  readonly summary: string;
  readonly category: PrivacyAlgorithmCategory;
  readonly maturity: PrivacyAlgorithmMaturity;
  readonly coveredCriteria: readonly PrivacyCriterionKey[];
  readonly proofFamily: string;
  readonly publicInputsSchema: string | null;
  readonly verifierKeyId: string | null;
  readonly backendFamily: string;
  readonly pqLayers: PrivacyPqLayers;
  readonly implementationStage?: string | null;
  readonly recommendedFor?: readonly string[];
  readonly sourceReferences?: readonly Readonly<{
    label: string;
    url: string;
  }>[];
  readonly securityNotes?: readonly string[];
  readonly requiredState?: readonly string[];
  readonly failureModes?: readonly string[];
  readonly setupSteps?: readonly string[];
  readonly executionSteps?: readonly string[];
  readonly sdkEntrypoints: readonly string[];
  readonly plannedSdkEntrypoints?: readonly string[];
  readonly chainRequirements: readonly string[];
  readonly productionReady: boolean;
  readonly productionGate: PrivacyProductionGate;
}

export interface PrivacyCapabilities {
  readonly javascriptSdkAvailable: boolean;
  readonly bridgeAvailable: boolean;
  readonly privacyAlgorithms: readonly PrivacyAlgorithmDescriptor[];
  readonly privacyCriteria: readonly PrivacyCriterionKey[];
}

export function getPrivacyCriteria(): readonly PrivacyCriterionKey[];
export function getPrivacyAlgorithmDescriptors(): readonly PrivacyAlgorithmDescriptor[];
export function getPrivacyAlgorithmDescriptor(
  id: string,
): PrivacyAlgorithmDescriptor | null;
export function getPrivacyCapabilities(): PrivacyCapabilities;
export function validatePrivacyAlgorithmDescriptor(
  descriptor: unknown,
  index?: number,
): PrivacyAlgorithmDescriptor;

export interface CryptoKeyPair {
  algorithm: CryptoAlgorithm;
  publicKey: Buffer;
  privateKey: Buffer;
  distid?: string | null;
}

export interface KeyPair extends CryptoKeyPair {
  algorithm: "ed25519";
}

export interface Sm2KeyPair extends CryptoKeyPair {
  algorithm: "sm2";
  distid: string;
}

export const CRYPTO_ALGORITHMS: Readonly<Record<string, CryptoAlgorithm>>;
export const SUPPORTED_CRYPTO_ALGORITHMS: readonly CryptoAlgorithm[];
export const SM2_PRIVATE_KEY_LENGTH: number;
export const SM2_PUBLIC_KEY_LENGTH: number;
export const SM2_SIGNATURE_LENGTH: number;
export const SM2_DEFAULT_DISTINGUISHED_ID: string;
export const PRIVACY_FFI_VERSION_V1: 1;
export const PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: 6;
export const PRIVACY_FFI_STATUS_ERROR: 1;
export const PRIVACY_FFI_ERROR_NULL_POINTER: 1;
export const PRIVACY_FFI_ERROR_MALFORMED_NORITO: 2;
export const PRIVACY_FFI_ERROR_UNSUPPORTED_ALGORITHM: 3;
export const PRIVACY_FFI_ERROR_PRODUCTION_DISABLED: 4;
export const PRIVACY_FFI_ERROR_INVALID_REQUEST: 5;

export interface SignedTransactionResult {
  signedTransaction: Buffer;
  hash: Buffer;
}

export const AccountAddressErrorCode: {
  readonly UNSUPPORTED_ALGORITHM: "ERR_UNSUPPORTED_ALGORITHM";
  readonly KEY_PAYLOAD_TOO_LONG: "ERR_KEY_PAYLOAD_TOO_LONG";
  readonly INVALID_HEADER_VERSION: "ERR_INVALID_HEADER_VERSION";
  readonly INVALID_NORM_VERSION: "ERR_INVALID_NORM_VERSION";
  readonly INVALID_I105_DISCRIMINANT: "ERR_INVALID_I105_DISCRIMINANT";
  readonly CANONICAL_HASH_FAILURE: "ERR_CANONICAL_HASH_FAILURE";
  readonly INVALID_LENGTH: "ERR_INVALID_LENGTH";
  readonly CHECKSUM_MISMATCH: "ERR_CHECKSUM_MISMATCH";
  readonly INVALID_HEX_ADDRESS: "ERR_INVALID_HEX_ADDRESS";
  readonly DOMAIN_MISMATCH: "ERR_DOMAIN_MISMATCH";
  readonly INVALID_DOMAIN_LABEL: "ERR_INVALID_DOMAIN_LABEL";
  readonly INVALID_REGISTRY_ID: "ERR_INVALID_REGISTRY_ID";
  readonly UNEXPECTED_NETWORK_PREFIX: "ERR_UNEXPECTED_NETWORK_PREFIX";
  readonly UNKNOWN_ADDRESS_CLASS: "ERR_UNKNOWN_ADDRESS_CLASS";
  readonly UNKNOWN_DOMAIN_TAG: "ERR_UNKNOWN_DOMAIN_TAG";
  readonly UNEXPECTED_EXTENSION_FLAG: "ERR_UNEXPECTED_EXTENSION_FLAG";
  readonly UNKNOWN_CONTROLLER_TAG: "ERR_UNKNOWN_CONTROLLER_TAG";
  readonly INVALID_PUBLIC_KEY: "ERR_INVALID_PUBLIC_KEY";
  readonly UNKNOWN_CURVE: "ERR_UNKNOWN_CURVE";
  readonly UNEXPECTED_TRAILING_BYTES: "ERR_UNEXPECTED_TRAILING_BYTES";
  readonly I105_TOO_SHORT: "ERR_I105_TOO_SHORT";
  readonly INVALID_I105_CHAR: "ERR_INVALID_I105_CHAR";
  readonly LOCAL_DIGEST_TOO_SHORT: "ERR_LOCAL8_DEPRECATED";
  readonly UNSUPPORTED_ADDRESS_FORMAT: "ERR_UNSUPPORTED_ADDRESS_FORMAT";
  readonly MULTISIG_MEMBER_OVERFLOW: "ERR_MULTISIG_MEMBER_OVERFLOW";
  readonly INVALID_MULTISIG_POLICY: "ERR_INVALID_MULTISIG_POLICY";
};

export class AccountAddressError extends Error {
  readonly code: (typeof AccountAddressErrorCode)[keyof typeof AccountAddressErrorCode];
  readonly details?: Record<string, unknown>;
  readonly cause?: unknown;
}

export const ValidationErrorCode: {
  readonly INVALID_STRING: "ERR_INVALID_STRING";
  readonly INVALID_HEX: "ERR_INVALID_HEX";
  readonly INVALID_MULTIHASH: "ERR_INVALID_MULTIHASH";
  readonly INVALID_ACCOUNT_ID: "ERR_INVALID_ACCOUNT_ID";
  readonly INVALID_ASSET_ID: "ERR_INVALID_ASSET_ID";
  readonly INVALID_IBAN: "ERR_INVALID_IBAN";
  readonly INVALID_OBJECT: "ERR_INVALID_OBJECT";
  readonly INVALID_METADATA: "ERR_INVALID_METADATA";
  readonly INVALID_JSON_VALUE: "ERR_INVALID_JSON_VALUE";
  readonly INVALID_NUMERIC: "ERR_INVALID_NUMERIC";
  readonly VALUE_OUT_OF_RANGE: "ERR_VALUE_OUT_OF_RANGE";
};

export class ValidationError extends TypeError {
  readonly code: (typeof ValidationErrorCode)[keyof typeof ValidationErrorCode];
  readonly path: string | null;
  readonly cause?: unknown;
}

export interface AccountAddressDisplay {
  i105: string;
  chainDiscriminant: number;
  i105Warning: string;
}

export interface CurveSupportOptions {
  allowMlDsa?: boolean;
  allowBls?: boolean;
  allowGost?: boolean;
  allowSm2?: boolean;
}

export function configureCurveSupport(options?: CurveSupportOptions): void;

export class AccountAddress {
  static fromAccount(options: {
    publicKey:
      | Buffer
      | Uint8Array
      | ArrayBuffer
      | ArrayBufferView
      | number[]
      | string;
    algorithm?: string;
  }): AccountAddress;
  static fromCanonicalBytes(
    bytes: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
  ): AccountAddress;
  static fromI105(
    encoded: string,
    expectedPrefix?: number | string | bigint,
  ): AccountAddress;
  static fromAccountId(
    accountId: string,
    expectedPrefix?: number | string | bigint,
  ): AccountAddress;
  static parseEncoded(
    input: string,
    expectedPrefix?: number | string | bigint,
  ): { address: AccountAddress; chainDiscriminant?: number };
  canonicalBytes(): Uint8Array;
  canonicalHex(): string;
  toI105(prefix?: number | string | bigint): string;
  toString(): string;
  displayFormats(
    chainDiscriminant?: number | string | bigint,
  ): AccountAddressDisplay;
}

export function encodeI105AccountAddress(
  canonicalBytes: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
  options?: { chainDiscriminant?: number | string | bigint },
): string;
export function decodeI105AccountAddress(
  encoded: string,
  options?: { expectDiscriminant?: number | string | bigint },
): Uint8Array;

export interface InspectAccountIdOptions {
  chainDiscriminant?: number | string | bigint;
  expectDiscriminant?: number | string | bigint;
}

export interface AccountIdInspection {
  detectedFormat: { kind: string; chainDiscriminant?: number };
  canonicalHex: string;
  i105: { value: string; chainDiscriminant: number };
  i105Warning: string;
  warnings: string[];
}

export function inspectAccountId(
  accountId: string,
  options?: InspectAccountIdOptions,
): AccountIdInspection;

export interface MultisigProposalTtlPreview {
  effectiveTtlMs: number;
  policyCapMs: number;
  expiresAtMs: number;
  wasCapped: boolean;
}

export class MultisigSpec {
  readonly signatories: ReadonlyMap<string, number>;
  readonly quorum: number;
  readonly transactionTtlMs: number;
  previewProposalExpiry(options?: {
    requestedTtlMs?: number | bigint | null;
    nowMs?: number | bigint;
  }): MultisigProposalTtlPreview;
  enforceProposalTtl(options?: {
    requestedTtlMs?: number | bigint | null;
    nowMs?: number | bigint;
  }): MultisigProposalTtlPreview;
  toPayload(): {
    signatories: Record<string, number>;
    quorum: number;
    transaction_ttl_ms: number;
  };
  toJSON(prettyPrinted?: boolean): string;
}

export class MultisigSpecBuilder {
  setQuorum(quorum: number | bigint): this;
  setTransactionTtlMs(ttlMs: number | bigint): this;
  addSignatory(accountId: string, weight: number | bigint): this;
  removeSignatory(accountId: string): this;
  build(): MultisigSpec;
  toJSON(prettyPrinted?: boolean): string;
}

export interface MultisigSpecPayload {
  signatories: Record<string, number>;
  quorum: number;
  transaction_ttl_ms: number;
}

export type MultisigSpecLike = MultisigSpec | MultisigSpecPayload;

export type MultisigTriggerArgsPreset = "lifecycle" | "lookup";

export interface MultisigLifecycleTriggerArgsInput {
  action: string;
  requestId?: string;
  request_id?: string;
  fiId?: string | null;
  fi_id?: string | null;
  toAccountId?: string | null;
  to_account_id?: string | null;
  amountI64?: number | string | bigint | null;
  amount_i64?: number | string | bigint | null;
  requestedByActorId?: JsonValue;
  requested_by_actor_id?: JsonValue;
  createdAtMs?: number | string | bigint | null;
  created_at_ms?: number | string | bigint | null;
  expiresAtMs?: number | string | bigint | null;
  expires_at_ms?: number | string | bigint | null;
}

export interface MultisigLookupTriggerArgsInput {
  requestId?: string;
  request_id?: string;
  requestedByActorId?: JsonValue;
  requested_by_actor_id?: JsonValue;
}

export interface ExecuteTriggerInstructionPayload {
  ExecuteTrigger: {
    trigger: string;
    args: JsonValue | null;
  };
}

export interface MultisigExecuteTriggerOptions {
  trigger: string;
  args?: JsonValue;
  argPreset?: MultisigTriggerArgsPreset;
  preset?: MultisigTriggerArgsPreset;
  argInput?: MultisigLifecycleTriggerArgsInput | MultisigLookupTriggerArgsInput;
  presetInput?:
    | MultisigLifecycleTriggerArgsInput
    | MultisigLookupTriggerArgsInput;
  signerAccountId?: string;
  multisigSpec?: MultisigSpecLike;
  spec?: MultisigSpecLike;
  strictSignerCheck?: boolean;
}

export interface ProposeMultisigExecuteTriggerOptions
  extends MultisigExecuteTriggerOptions {
  accountId: string;
  spec: MultisigSpecLike;
  transactionTtlMs?: number | null;
}

export interface MultisigAccountSelector {
  multisigAccountId?: string;
  multisigAccountAlias?: string;
}

export type MultisigProposeInstructionInput =
  | object
  | string
  | BinaryLike
  | number[];

export interface MultisigProposeRequest extends MultisigAccountSelector {
  signerAccountId: string;
  instructions: MultisigProposeInstructionInput[];
  feeSponsor?: string | null;
  publicKeyHex?: string | null;
  signatureB64?: string | null;
  creationTimeMs?: number | string | bigint | null;
  privateKey?: string | BinaryLike | null;
  privateKeyHex?: string | null;
  privateKeyMultihash?: string | null;
  privateKeyBytes?: BinaryLike | number[] | null;
  privateKeyAlgorithm?: string | null;
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id?: string;
  fee_sponsor?: string | null;
  public_key_hex?: string | null;
  signature_b64?: string | null;
  creation_time_ms?: number | string | bigint | null;
  private_key?: string | BinaryLike | null;
  private_key_hex?: string | null;
  private_key_multihash?: string | null;
  private_key_bytes?: BinaryLike | number[] | null;
  private_key_algorithm?: string | null;
}

export interface MultisigProposePayload {
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id: string;
  instructions: string[];
  fee_sponsor?: string;
  public_key_hex?: string;
  signature_b64?: string;
  creation_time_ms?: number;
  private_key?: string | BinaryLike;
}

export interface MultisigContractCallProposeRequest
  extends MultisigAccountSelector {
  signerAccountId: string;
  contractAddress?: string;
  contractAlias?: string;
  entrypoint: string;
  payload?: JsonValue;
  gasAssetId?: string | null;
  feeSponsor?: string | null;
  gasLimit?: number | string | bigint | null;
  publicKeyHex?: string | null;
  signatureB64?: string | null;
  creationTimeMs?: number | string | bigint | null;
  privateKey?: string | BinaryLike | null;
  privateKeyHex?: string | null;
  privateKeyMultihash?: string | null;
  privateKeyBytes?: BinaryLike | number[] | null;
  privateKeyAlgorithm?: string | null;
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id?: string;
  contract_address?: string;
  contract_alias?: string;
  gas_asset_id?: string | null;
  fee_sponsor?: string | null;
  gas_limit?: number | string | bigint | null;
  public_key_hex?: string | null;
  signature_b64?: string | null;
  creation_time_ms?: number | string | bigint | null;
  private_key?: string | BinaryLike | null;
  private_key_hex?: string | null;
  private_key_multihash?: string | null;
  private_key_bytes?: BinaryLike | number[] | null;
  private_key_algorithm?: string | null;
}

export interface MultisigContractCallProposePayload {
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id: string;
  contract_address?: string;
  contract_alias?: string;
  entrypoint: string;
  payload: JsonValue;
  gas_asset_id?: string;
  fee_sponsor?: string;
  gas_limit?: number;
  public_key_hex?: string;
  signature_b64?: string;
  creation_time_ms?: number;
  private_key?: string | BinaryLike;
}

export interface MultisigContractCallApproveRequest
  extends MultisigAccountSelector {
  signerAccountId: string;
  proposalId?: string | null;
  instructionsHash?: string | null;
  feeSponsor?: string | null;
  publicKeyHex?: string | null;
  signatureB64?: string | null;
  creationTimeMs?: number | string | bigint | null;
  privateKey?: string | BinaryLike | null;
  privateKeyHex?: string | null;
  privateKeyMultihash?: string | null;
  privateKeyBytes?: BinaryLike | number[] | null;
  privateKeyAlgorithm?: string | null;
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id?: string;
  proposal_id?: string | null;
  instructions_hash?: string | null;
  fee_sponsor?: string | null;
  public_key_hex?: string | null;
  signature_b64?: string | null;
  creation_time_ms?: number | string | bigint | null;
  private_key?: string | BinaryLike | null;
  private_key_hex?: string | null;
  private_key_multihash?: string | null;
  private_key_bytes?: BinaryLike | number[] | null;
  private_key_algorithm?: string | null;
}

export interface MultisigContractCallApprovePayload {
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id: string;
  proposal_id?: string;
  instructions_hash?: string;
  fee_sponsor?: string;
  public_key_hex?: string;
  signature_b64?: string;
  creation_time_ms?: number;
  private_key?: string | BinaryLike;
}

export interface MultisigContractCallResponse {
  ok: boolean;
  resolved_multisig_account_id: string;
  submitted: boolean | null;
  proposal_id: string | null;
  instructions_hash: string | null;
  tx_hash_hex: string | null;
  executed_tx_hash_hex: string | null;
  creation_time_ms: number | null;
  signing_message_b64: string | null;
}

export interface MultisigSpecResponse {
  resolved_multisig_account_id: string;
  spec: JsonValue;
}

export interface MultisigProposalEntry {
  proposal_id: string;
  instructions_hash: string;
  proposal: JsonValue;
}

export interface MultisigProposalListResponse {
  resolved_multisig_account_id: string;
  proposals: ReadonlyArray<MultisigProposalEntry>;
}

export interface MultisigProposalGetResponse extends MultisigProposalEntry {
  resolved_multisig_account_id: string;
}

export interface SoradnsGatewayHosts {
  readonly normalizedName: string;
  readonly canonicalLabel: string;
  readonly canonicalHost: string;
  readonly canonicalWildcard: string;
  readonly prettyHost: string;
  readonly hostPatterns: ReadonlyArray<string>;
  matchesHost(host: string): boolean;
}

export interface SoradnsGatewayHostOptions {
  prettySuffix?: string;
}

export function deriveSoradnsGatewayHosts(
  fqdn: string,
  options?: SoradnsGatewayHostOptions,
): SoradnsGatewayHosts;
export function hostPatternsCoverDerivedHosts(
  patterns: Iterable<string>,
  derived: SoradnsGatewayHosts,
): boolean;
export function canonicalGatewaySuffix(): string;
export function canonicalGatewayWildcard(): string;
export function prettyGatewaySuffix(): string;
export function tairaMonPrettyGatewaySuffix(): string;

export interface IsoBridgeAgent {
  bic: string;
  lei?: string;
}

export interface IsoBridgeProxy {
  id: string;
  typeCode?: string;
  typeProprietary?: string;
}

export interface IsoBridgeAccount {
  iban?: string;
  otherId?: string;
  proxy?: IsoBridgeProxy;
}

export interface IsoBridgeParty {
  name: string;
  lei?: string;
  identifier?: string;
  identifierScheme?: string;
}

export interface IsoBridgeAmount {
  currency: string;
  value?: string | number;
  amount?: string | number;
}

export const SCCP_DOMAIN_SORA: number;
export const SCCP_DOMAIN_ETH: number;
export const SCCP_DOMAIN_BSC: number;
export const SCCP_DOMAIN_SOL: number;
export const SCCP_DOMAIN_TON: number;
export const SCCP_DOMAIN_TRON: number;
export const SCCP_DOMAIN_SORA_KUSAMA: number;
export const SCCP_DOMAIN_SORA_POLKADOT: number;
export const SCCP_DOMAIN_SORA2: number;
export const SCCP_CODEC_TEXT_UTF8: 1;
export const SCCP_CODEC_EVM_HEX: 2;
export const SCCP_CODEC_SOLANA_BASE58: 3;
export const SCCP_CODEC_TON_RAW: 4;
export const SCCP_CODEC_TRON_BASE58CHECK: 5;
export const SCCP_CODEC_SORA_ASSET_ID: 6;
export const SCCP_ETH_MAINNET_EVM_CHAIN_ID: 1;
export const SCCP_ETH_MAINNET_NETWORK_ID: string;
export const SCCP_BSC_MAINNET_EVM_CHAIN_ID: 56;
export const SCCP_BSC_MAINNET_NETWORK_ID: string;
export const SCCP_BSC_TESTNET_EVM_CHAIN_ID: 97;
export const SCCP_BSC_TESTNET_NETWORK_ID: string;
export const SCCP_STARK_FRI_PROOF_FAMILY_V1: string;
export const SCCP_SOURCE_STATE_MAX_PROOF_BYTES: number;
export const SCCP_SOURCE_STATE_MAX_PROOF_LABEL_BYTES: number;
export const SCCP_NATIVE_RECURSIVE_MAX_PROOF_BYTES: number;
export const SCCP_SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-source-adapter-v1";
export const SCCP_SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1: "fastpq-lane-balanced";
export const SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1: "evm-groth16-bn254-v1";
export const SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1: "sccp-native-evm-groth16-prover-bundle-v1";
export const SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1: "sccp-ethereum-mainnet-native-evm-cross-sdk-fixture-parity-v1";
export const SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1: "sccp-ethereum-mainnet-native-evm-prover-self-test-v1";
export const SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1: "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1";
export const SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1: "sccp-bsc-testnet-native-evm-cross-sdk-fixture-parity-v1";
export const SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1: "sccp-bsc-testnet-native-evm-prover-self-test-v1";
export const SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1: "sccp:bsc:native-evm-groth16-prover:bsc-testnet:v1";
export const SCCP_ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1: Readonly<{
  javascript: "pure-typescript";
  swift: "native-swift";
  kotlin: "native-kotlin";
  "java-android": "native-java";
  dotnet: "native-csharp";
}>;
export const SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1: "sha256";
export const SCCP_GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1: 384;
export const SCCP_EVM_CONTRACT_CALL_ABI_TUPLE_V1: "abi_tuple_v1";
export const SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1: "norito:sccp-local-admission:v1";
export const SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1: "local_admission";
export const SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1: "SubmitBridgeProof";
export const SCCP_TRON_CONTRACT_CALL_ABI_TUPLE_V1: "tron_abi_tuple_v1";
export const SCCP_SUBMIT_MESSAGE_PROOF_ABI_V1: "submitSccpMessageProof(bytes,bytes32[6],bytes32)";
export const SCCP_SUBMIT_MESSAGE_PROOF_SELECTOR_V1: string;
export const SCCP_MESSAGE_TRANSPARENT_PUBLIC_INPUTS_BYTES_V1_LEN: 141;
export const SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1: "sccp-solana-recursive-mainnet-v1";
export const SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-solana-accounts-lt-hash-v1";
export const SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-solana-tower-replay-v1";
export const SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-solana-full-accountsdb-lattice-v1";
export const SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-solana-bank-fork-choice-v1";
export const SCCP_SOLANA_MAINNET_GENESIS_HASH: string;
export const SCCP_SOLANA_MAINNET_ACCOUNTS_DB_VERIFIER_ID_V1: string;
export const SCCP_SOLANA_UPGRADEABLE_LOADER_ID: string;
export const SCCP_SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1: string;
export const SCCP_SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1: string;
export const SCCP_SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1: string;
export const SCCP_SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1: string;
export const SCCP_SOLANA_MAINNET_SLOTS_PER_EPOCH: bigint;
export const SCCP_SOLANA_TOWER_LOCKOUT_CONFIRMATION_DEPTH: bigint;
export const SCCP_SOLANA_TOWER_VOTE_STACK_DEPTH: bigint;
export const SCCP_SOLANA_TOWER_WARMUP_COOLDOWN_RATE_BPS: bigint;
export const SCCP_SOLANA_MAX_VALIDATORS: number;
export const SCCP_SOLANA_VOTE_PROGRAM_ID: string;
export const SCCP_SOLANA_STAKE_PROGRAM_ID: string;
export const SCCP_SOLANA_SYSVAR_PROGRAM_ID: string;
export const SCCP_SOLANA_STAKE_HISTORY_SYSVAR_ID: string;
export const SCCP_SOLANA_BORSH_INSTRUCTION_V1: string;
export const SCCP_SOLANA_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: "submit_sccp_message_proof";
export const SCCP_ZERO_HASH_V1: string;
export const SCCP_TON_CONTRACT_PROOF_BACKEND_V1: "ton-contract-v1";
export const SCCP_TON_MESSAGE_BODY_BOC_V1: string;
export const SCCP_TON_MAINNET_SHARD_STATE_VERIFIER_ID_V1: string;
export const SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-ton-shard-state-light-client-v1";
export const SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-ton-masterchain-config-v1";
export const SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-ton-validator-set-transition-v1";
export const SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-ton-shard-accounts-dictionary-v1";
export const SCCP_TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1: string;
export const SCCP_TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1: string;
export const SCCP_TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1: string;
export const SCCP_TON_CURRENT_VALIDATOR_SET_CONFIG_PARAM: bigint;
export const SCCP_TON_CONFIG_PARAM_KEY_BITS: number;
export const SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1: "tron-groth16-bn254-v1";
export const SCCP_TAIRA_CHAIN_ID_V1: "809574f5-fee7-5e69-bfcf-52451e42d50f";
export const SCCP_TAIRA_NETWORK_PREFIX_V1: 369;
export const SCCP_TAIRA_TRON_XOR_ROUTE_ID_V1: "taira_tron_xor";
export const SCCP_TAIRA_BSC_XOR_ROUTE_ID_V1: "taira_bsc_xor";
export const SCCP_TAIRA_XOR_ASSET_KEY_V1: "xor";
export const SCCP_TAIRA_XOR_MAX_TAIRA_RECIPIENT_BYTES_V1: 256;
export const SCCP_TAIRA_XOR_RECORD_EXECUTION_KIND_V1: "ivm_proved_record_sccp_message_v1";
export const SCCP_TAIRA_XOR_BURN_RECORD_ENTRYPOINT_V1: "burn_and_record";
export const TAIRA_XOR_FINALIZE_FROM_TAIRA_ABI_V1: "finalizeFromTaira(bytes,bytes32[6],bytes32,bytes)";
export const TAIRA_XOR_BURN_TO_TAIRA_ABI_V1: "burnToTaira(bytes32,bytes32,bytes,uint256)";
export const TAIRA_XOR_FINALIZE_FROM_TAIRA_SELECTOR_V1: string;
export const TAIRA_XOR_BURN_TO_TAIRA_SELECTOR_V1: string;
export const SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1: "substrate-runtime-v1";
export const SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1: "scale_call_v1";
export const SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: "SccpBridge.submit_message_proof";
export const SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1: "sccp-substrate-runtime-storage-v1";
export const SCCP_CORE_REMOTE_DOMAINS: number[];

export interface SccpBurnPayload {
  version: number;
  source_domain: number;
  dest_domain: number;
  nonce: string | number | bigint;
  sora_asset_id: string;
  amount: string | number | bigint;
  recipient: string;
}

export interface SccpTransferPayload {
  version: number;
  source_domain: number;
  dest_domain: number;
  nonce: string | number | bigint;
  asset_home_domain: number;
  asset_id_codec: number;
  asset_id: string | BinaryLike | number[];
  amount: string | number | bigint;
  sender_codec: number;
  sender: string | BinaryLike | number[];
  recipient_codec: number;
  recipient: string | BinaryLike | number[];
  route_id_codec: number;
  route_id: string | BinaryLike | number[];
}

export interface TairaXorTransferPayloadInput {
  routeId?: string;
  route_id?: string;
  assetKey?: string;
  asset_key?: string;
  assetId?: string;
  asset_id?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  sender?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaSender?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_sender?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  recipientAddress?: string;
  recipient_address?: string;
  recipient?: string;
  amount: string | number | bigint;
  nonce: string | number | bigint;
}

export interface TairaXorBscTransferPayloadInput
  extends TairaXorTransferPayloadInput {
  bscRecipient?: string;
  bsc_recipient?: string;
  evmRecipient?: string;
  evm_recipient?: string;
}

export interface TairaXorTronToTairaTransferPayloadInput {
  routeId?: string;
  route_id?: string;
  assetKey?: string;
  asset_key?: string;
  assetId?: string;
  asset_id?: string;
  tronSender?: string;
  tron_sender?: string;
  sender?: string;
  senderAddress?: string;
  sender_address?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaRecipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
  nonce: string | number | bigint;
}

export interface TairaXorBscToTairaTransferPayloadInput {
  routeId?: string;
  route_id?: string;
  assetKey?: string;
  asset_key?: string;
  assetId?: string;
  asset_id?: string;
  bscSender?: string;
  bsc_sender?: string;
  evmSender?: string;
  evm_sender?: string;
  sender?: string;
  senderAddress?: string;
  sender_address?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaRecipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
  nonce: string | number | bigint;
}

export interface TairaXorTronToTairaSettlementFragment {
  entrypoint?: "finalize_inbound";
  route?: "taira_tron_xor";
  route_id?: "taira_tron_xor";
  payload?: never;
  payload_json?: never;
  payloadJson?: never;
  payload_bytes?: never;
  payloadBytes?: never;
  [key: string]: unknown;
}

export interface TairaXorBscToTairaSettlementFragment {
  entrypoint?: "finalize_inbound";
  route?: "taira_bsc_xor";
  route_id?: "taira_bsc_xor";
  payload?: never;
  payload_json?: never;
  payloadJson?: never;
  payload_bytes?: never;
  payloadBytes?: never;
  [key: string]: unknown;
}

export interface TairaXorTronToTairaSourceProofPackageInput {
  proofPackage?: Record<string, unknown>;
  proof_package?: Record<string, unknown>;
  txId?: string;
  txID?: string;
  transactionId?: string;
  transaction_id?: string;
  tronSender?: string;
  tron_sender?: string;
  sender?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaRecipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
  /** Optional TRON bridge contract address; when supplied, sourceEventDigest is bound to the burn event. */
  bridgeAddress?: string | BinaryLike | number[];
  bridge_address?: string | BinaryLike | number[];
  tronBridgeAddress?: string | BinaryLike | number[];
  tron_bridge_address?: string | BinaryLike | number[];
  settlementDefaults?: TairaXorTronToTairaSettlementFragment;
  settlement_defaults?: TairaXorTronToTairaSettlementFragment;
}

export interface TairaXorBscToTairaSourceProofPackageInput {
  proofPackage?: Record<string, unknown>;
  proof_package?: Record<string, unknown>;
  txId?: string;
  txID?: string;
  transactionHash?: string;
  transaction_hash?: string;
  transactionId?: string;
  transaction_id?: string;
  bscSender?: string;
  bsc_sender?: string;
  evmSender?: string;
  evm_sender?: string;
  sender?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaRecipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
  /** Optional BSC bridge contract address; when supplied, sourceEventDigest is bound to the burn event. */
  bridgeAddress?: string | BinaryLike | number[];
  bridge_address?: string | BinaryLike | number[];
  bscBridgeAddress?: string | BinaryLike | number[];
  bsc_bridge_address?: string | BinaryLike | number[];
  evmBridgeAddress?: string | BinaryLike | number[];
  evm_bridge_address?: string | BinaryLike | number[];
  settlementDefaults?: TairaXorBscToTairaSettlementFragment;
  settlement_defaults?: TairaXorBscToTairaSettlementFragment;
}

export interface TairaXorTronToTairaBoundSourceProofPackage {
  readonly messageBundle: Record<string, unknown>;
  readonly settlement: Readonly<
    Record<string, unknown> & {
      entrypoint: "finalize_inbound";
      route: "taira_tron_xor";
    }
  >;
  readonly sourceEventDigest: string;
  readonly txId: string;
  readonly messageId: string;
  readonly commitmentRoot: string;
  readonly amount: string;
}

export interface TairaXorBscToTairaBoundSourceProofPackage {
  readonly messageBundle: Record<string, unknown>;
  readonly settlement: Readonly<
    Record<string, unknown> & {
      entrypoint: "finalize_inbound";
      route: "taira_bsc_xor";
    }
  >;
  readonly sourceEventDigest: string;
  readonly txId: string;
  readonly messageId: string;
  readonly commitmentRoot: string;
  readonly amount: string;
}

export interface TairaXorTronBurnStartedEventInput
  extends TairaXorRouteHashInput {
  event: Record<string, unknown>;
  sourceEventDigest?: string;
  source_event_digest?: string;
  expectedSourceEventDigest?: string;
  expected_source_event_digest?: string;
  burnerAddress?: string | BinaryLike | number[];
  burner_address?: string | BinaryLike | number[];
  burner?: string | BinaryLike | number[];
  tronSender?: string | BinaryLike | number[];
  tron_sender?: string | BinaryLike | number[];
  sender?: string | BinaryLike | number[];
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaRecipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  recipient?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount?: string | number | bigint;
  /** Optional TRON bridge contract address; when supplied, sourceEventDigest is recomputed from event fields. */
  bridgeAddress?: string | BinaryLike | number[];
  bridge_address?: string | BinaryLike | number[];
  tronBridgeAddress?: string | BinaryLike | number[];
  tron_bridge_address?: string | BinaryLike | number[];
}

export interface TairaXorTronBoundBurnStartedEvent {
  readonly eventName: "tairaxorburnstarted" | "burntotaira";
  readonly sourceEventDigest: string;
  readonly routeIdHash: string;
  readonly assetKeyHash: string;
  readonly bridgeAddress: string | null;
  readonly burnerAddress: string;
  readonly tairaRecipient: string;
  readonly tairaRecipientHash: string;
  readonly amount: string;
  readonly nonce: string;
}

export interface TairaXorSccpRecordDescriptorInput
  extends TairaXorTransferPayloadInput {
  chainId?: string;
  chain_id?: string;
  tairaChainId?: string;
  taira_chain_id?: string;
  networkPrefix?: string | number | bigint;
  network_prefix?: string | number | bigint;
  tairaNetworkPrefix?: string | number | bigint;
  taira_network_prefix?: string | number | bigint;
  expectedMessageId?: string;
  expected_message_id?: string;
  messageId?: string;
  message_id?: string;
  expectedCanonicalPayloadBytes?: BinaryLike | number[];
  expected_canonical_payload_bytes?: BinaryLike | number[];
  expectedCanonicalPayloadHex?: string;
  expected_canonical_payload_hex?: string;
  expectedPayloadBytes?: BinaryLike | number[];
  expected_payload_bytes?: BinaryLike | number[];
}

export interface TairaXorBscSccpRecordDescriptorInput
  extends TairaXorSccpRecordDescriptorInput {
  bscRecipient?: string;
  bsc_recipient?: string;
  evmRecipient?: string;
  evm_recipient?: string;
}

export interface TairaXorSccpRecordDescriptor {
  readonly version: 1;
  readonly kind: "TairaXorSccpRecordDescriptor";
  readonly execution_kind: "ivm_proved_record_sccp_message_v1";
  readonly chain_id: "809574f5-fee7-5e69-bfcf-52451e42d50f";
  readonly network_prefix: 369;
  readonly route_id: "taira_tron_xor";
  readonly asset_key: "xor";
  readonly message_kind: "Transfer";
  readonly source_domain: number;
  readonly dest_domain: number;
  readonly message_id: string;
  readonly canonical_payload_hex: string;
  readonly canonicalPayloadBytes: Uint8Array;
  readonly payload: Readonly<SccpTransferPayload>;
  readonly record_instruction: Readonly<{
    kind: "RecordSccpMessage";
    payload_bytes_hex: string;
  }>;
  readonly execution_requirements: Readonly<{
    executable: "IvmProved";
    overlay_instruction: "RecordSccpMessage";
    settlement_instruction: "Burn<Numeric, Asset>";
    settlement_asset_selector: "nexus.fees.fee_asset_id";
    settlement_asset_key: "xor";
    settlement_account_binding: "burn.destination.account == payload.sender";
    settlement_amount_binding: "sum(whole-unit burns) >= sum(recorded amounts) per sender";
    proof_gate: "sccp_recording_proof_verified";
    normal_transaction_supported: false;
  }>;
}

export interface TairaXorBscSccpRecordDescriptor
  extends Omit<TairaXorSccpRecordDescriptor, "route_id"> {
  readonly route_id: "taira_bsc_xor";
}

export interface TairaXorSccpBurnRecordInput
  extends TairaXorSccpRecordDescriptorInput {
  descriptor?: TairaXorSccpRecordDescriptor;
  recordDescriptor?: TairaXorSccpRecordDescriptor;
  record_descriptor?: TairaXorSccpRecordDescriptor;
  settlementAssetDefinitionId?: string;
  settlement_asset_definition_id?: string;
  settlementAsset?: string;
  settlement_asset?: string;
  authority?: string;
}

export interface TairaXorBscSccpBurnRecordInput
  extends TairaXorBscSccpRecordDescriptorInput {
  descriptor?: TairaXorBscSccpRecordDescriptor;
  recordDescriptor?: TairaXorBscSccpRecordDescriptor;
  record_descriptor?: TairaXorBscSccpRecordDescriptor;
  settlementAssetDefinitionId?: string;
  settlement_asset_definition_id?: string;
  settlementAsset?: string;
  settlement_asset?: string;
  authority?: string;
}

export interface TairaXorSccpBurnRecordContractPayload {
  readonly version: 1;
  readonly entrypoint: "burn_and_record";
  readonly descriptor: Readonly<TairaXorSccpRecordDescriptor>;
  readonly payload: Readonly<{
    sender: string;
    settlement_asset: string;
    amount: string;
    record_instruction: string;
  }>;
  readonly record_instruction_hex: string;
}

export interface TairaXorBscSccpBurnRecordContractPayload
  extends Omit<TairaXorSccpBurnRecordContractPayload, "descriptor"> {
  readonly descriptor: Readonly<TairaXorBscSccpRecordDescriptor>;
}

export interface TairaXorSccpBurnRecordZkIvmRequestInput
  extends TairaXorSccpBurnRecordInput {
  vkRef?: { backend?: string; name?: string };
  vk_ref?: { backend?: string; name?: string };
  bytecode?: string;
  artifactB64?: string;
  artifact_b64?: string;
  contractArtifact?: {
    artifact_b64?: string;
    artifactB64?: string;
    bytecode?: string;
  };
  contract_artifact?: {
    artifact_b64?: string;
    artifactB64?: string;
    bytecode?: string;
  };
  artifact?: { artifact_b64?: string; artifactB64?: string; bytecode?: string };
  gasLimit?: string | number | bigint;
  gas_limit?: string | number | bigint;
  metadata?: Record<string, JsonValue>;
}

export interface TairaXorBscSccpBurnRecordZkIvmRequestInput
  extends TairaXorBscSccpBurnRecordInput {
  vkRef?: { backend?: string; name?: string };
  vk_ref?: { backend?: string; name?: string };
  bytecode?: string;
  artifactB64?: string;
  artifact_b64?: string;
  contractArtifact?: {
    artifact_b64?: string;
    artifactB64?: string;
    bytecode?: string;
  };
  contract_artifact?: {
    artifact_b64?: string;
    artifactB64?: string;
    bytecode?: string;
  };
  artifact?: { artifact_b64?: string; artifactB64?: string; bytecode?: string };
  gasLimit?: string | number | bigint;
  gas_limit?: string | number | bigint;
  metadata?: Record<string, JsonValue>;
}

export interface TairaXorSccpBurnRecordZkIvmRequest {
  readonly version: 1;
  readonly route_id: "taira_tron_xor";
  readonly asset_key: "xor";
  readonly descriptor: Readonly<TairaXorSccpRecordDescriptor>;
  readonly contract: Readonly<TairaXorSccpBurnRecordContractPayload>;
  readonly request: Readonly<{
    vkRef: Readonly<{ backend: string; name: string }>;
    authority: string;
    metadata: Readonly<Record<string, JsonValue>>;
    bytecode: string;
  }>;
}

export interface TairaXorBscSccpBurnRecordZkIvmRequest
  extends Omit<
    TairaXorSccpBurnRecordZkIvmRequest,
    "route_id" | "descriptor" | "contract"
  > {
  readonly route_id: "taira_bsc_xor";
  readonly descriptor: Readonly<TairaXorBscSccpRecordDescriptor>;
  readonly contract: Readonly<TairaXorBscSccpBurnRecordContractPayload>;
}

export interface SccpTokenAddPayload {
  version: number;
  target_domain: number;
  nonce: string | number | bigint;
  sora_asset_id: string;
  decimals: number;
  name: string;
  symbol: string;
}

export interface SccpTokenControlPayload {
  version: number;
  target_domain: number;
  nonce: string | number | bigint;
  sora_asset_id: string;
}

export type SccpTokenMessagePayload =
  | { kind: "TokenAdd"; value: SccpTokenAddPayload }
  | { kind: "TokenPause"; value: SccpTokenControlPayload }
  | { kind: "TokenResume"; value: SccpTokenControlPayload }
  | { TokenAdd: SccpTokenAddPayload }
  | { TokenPause: SccpTokenControlPayload }
  | { TokenResume: SccpTokenControlPayload };

export type SccpGovernancePayload =
  | { Add: SccpTokenAddPayload }
  | { Pause: SccpTokenControlPayload }
  | { Resume: SccpTokenControlPayload }
  | SccpTokenMessagePayload;

export type SccpHubMessageKind =
  | "Burn"
  | "TokenAdd"
  | "TokenPause"
  | "TokenResume"
  | "AssetRegister"
  | "RouteActivate"
  | "Transfer";

export interface SccpHubCommitment {
  version: number;
  kind: SccpHubMessageKind;
  target_domain: number;
  message_id: string;
  payload_hash: string;
}

export interface SccpMerkleStep {
  sibling_hash: string;
  sibling_is_left: boolean;
}

export interface SccpMerkleProof {
  steps: SccpMerkleStep[];
}

export interface SccpBurnBundle {
  version: number;
  commitment_root: string;
  commitment: SccpHubCommitment;
  merkle_proof: SccpMerkleProof;
  payload: SccpBurnPayload;
  finality_proof: string;
}

export interface SccpTokenMessageBundle {
  version: number;
  commitment_root: string;
  commitment: SccpHubCommitment;
  merkle_proof: SccpMerkleProof;
  payload: SccpTokenMessagePayload;
  finality_proof: string;
}

export interface SccpGovernanceBundle {
  version: number;
  commitment_root: string;
  commitment: SccpHubCommitment & { parliament_certificate_hash: string };
  merkle_proof: SccpMerkleProof;
  payload: SccpGovernancePayload;
  parliament_certificate: string;
  finality_proof: string;
}

export interface SccpBundleSurfaceValidation {
  ok: boolean;
  expectedMessageId: string;
  expectedPayloadHash: string;
  expectedMerkleRoot: string;
  checks: Record<string, boolean>;
}

export interface SccpGovernanceBundleSurfaceValidation
  extends SccpBundleSurfaceValidation {
  expectedCertificateHash: string;
}

export type SccpDomainIdInput = number | string | bigint;
export type SccpVersionInput = 1 | "1" | 1n;

export interface SolanaSccpWitnessInput {
  bundle?: {
    commitment?: Partial<SccpHubCommitment>;
    payload?: unknown;
    commitment_root?: string;
  };
  commitment?: Partial<SccpHubCommitment>;
  payload?: unknown;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  mainnetGenesisHash?: string;
  mainnet_genesis_hash?: string;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  slot?: string | number | bigint;
  parentSlot?: string | number | bigint;
  parent_slot?: string | number | bigint;
  bankSignatureCount?: string | number | bigint;
  bank_signature_count?: string | number | bigint;
  parentBankHash?: string;
  parent_bank_hash?: string;
  blockhash: string;
  blockhashBytes?: BinaryLike;
  blockhash_bytes?: BinaryLike;
  bankHash?: string;
  bank_hash?: string;
  transactionStatusRoot?: string;
  transaction_status_root?: string;
  accountInclusionRoot?: string;
  account_inclusion_root?: string;
  accountsLtHashChecksum?: string;
  accounts_lt_hash_checksum?: string;
  accountsLtHashProofPublicInputsHash?: string;
  accounts_lt_hash_proof_public_inputs_hash?: string;
  bankHashHardForkData?: BinaryLike;
  bank_hash_hard_fork_data?: BinaryLike;
  accountsLtHash?: BinaryLike;
  accounts_lt_hash?: BinaryLike;
  messageProofHash?: string;
  message_proof_hash?: string;
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  transactionSignature?: string;
  transaction_signature?: string;
  emitterProgramId?: string;
  emitter_program_id?: string;
  messageId?: string;
  message_id?: string;
  payloadHash?: string;
  payload_hash?: string;
  commitmentRoot?: string;
  commitment_root?: string;
  sourceEventDigest?: string;
  source_event_digest?: string;
  sourceStateVerifierId?: string;
  source_state_verifier_id?: string;
  sourceStateVerifierHash?: string;
  source_state_verifier_hash?: string;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  sourceAdapterDeploymentHash?: string;
  source_adapter_deployment_hash?: string;
  sourceAdapterDeploymentReceiptHash?: string;
  source_adapter_deployment_receipt_hash?: string;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
}

export interface SolanaSccpWitness {
  version: 1;
  sourceDomain: typeof SCCP_DOMAIN_SOL;
  targetDomain: number;
  mainnetGenesisHash: string;
  finalizedSlot: string;
  parentSlot: string;
  bankSignatureCount: string;
  parentBankHash: string;
  blockhash: string;
  bankHash: string;
  transactionStatusRoot: string;
  messageProofHash: string;
  accountInclusionRoot: string;
  accountsLtHashChecksum: string;
  accountsLtHashProofPublicInputsHash: string;
  bankHashHardForkData: string;
  accountsLtHash?: string;
  transactionSignature: string;
  emitterProgramId: string;
  messageId: string;
  payloadHash: string;
  commitmentRoot: string;
  sourceEventDigest: string;
  sourceStateVerifierId: string;
  sourceStateVerifierHash: string;
  sourceAdapterDeploymentHash: string;
  sourceAdapterDeploymentReceiptHash: string;
  inclusionBranch?: string[];
  payload: unknown;
}

export interface SolanaSccpEpochStakeRootInput {
  epoch?: string | number | bigint;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  slot?: string | number | bigint;
  validatorPublicKeys?: readonly BinaryLike[];
  validator_public_keys?: readonly BinaryLike[];
  validatorStakes?: readonly (string | number | bigint)[];
  validator_stakes?: readonly (string | number | bigint)[];
}

export interface SolanaSccpStakeActivationInput
  extends SolanaSccpEpochStakeRootInput {
  validatorActivationEpochs?: readonly (string | number | bigint)[];
  validator_activation_epochs?: readonly (string | number | bigint)[];
  activationEpochs?: readonly (string | number | bigint)[];
  activation_epochs?: readonly (string | number | bigint)[];
  validatorDeactivationEpochs?: readonly (string | number | bigint)[];
  validator_deactivation_epochs?: readonly (string | number | bigint)[];
  deactivationEpochs?: readonly (string | number | bigint)[];
  deactivation_epochs?: readonly (string | number | bigint)[];
}

export interface SolanaSccpAccountOpeningInput {
  address?: BinaryLike;
  accountAddress?: BinaryLike;
  account_address?: BinaryLike;
  owner?: BinaryLike;
  ownerProgramId?: BinaryLike;
  owner_program_id?: BinaryLike;
  lamports: string | number | bigint;
  rentEpoch?: string | number | bigint;
  rent_epoch?: string | number | bigint;
  executable?: boolean;
  dataHash?: string;
  data_hash?: string;
}

export interface SolanaSccpAccountInclusionLeafInput {
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  opening?: SolanaSccpAccountOpeningInput;
  accountOpening?: SolanaSccpAccountOpeningInput;
  account_opening?: SolanaSccpAccountOpeningInput;
  rawData?: BinaryLike;
  raw_data?: BinaryLike;
  rawDataHash?: string;
  raw_data_hash?: string;
}

export interface SolanaSccpAccountInclusionRootAndBranches {
  readonly root: string;
  readonly branches: ReadonlyArray<ReadonlyArray<string>>;
}

export function solanaSccpAccountLtHash(
  opening: SolanaSccpAccountOpeningInput,
  rawData: BinaryLike,
): Uint8Array;
export function solanaSccpAccountsLtHashFromOpenings(
  openings: readonly SolanaSccpAccountOpeningInput[],
  rawDataValues: readonly BinaryLike[],
): Uint8Array;
export function solanaSccpAccountsLtHashChecksum(
  accountsLtHash: BinaryLike,
): string;
export interface SolanaSccpAccountsLtHashOpenedContributionsInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  slot?: string | number | bigint;
  accountInclusionRoot?: string;
  account_inclusion_root?: string;
  accountsRoot?: string;
  accounts_root?: string;
  accountsLtHashChecksum?: string;
  accounts_lt_hash_checksum?: string;
  accountsLtHashRoot?: string;
  accounts_lt_hash_root?: string;
  accountsLtHash?: BinaryLike | number[];
  accounts_lt_hash?: BinaryLike | number[];
  validatorVoteAccountOpenings?: readonly SolanaSccpAccountOpeningInput[];
  validator_vote_account_openings?: readonly SolanaSccpAccountOpeningInput[];
  voteAccountOpenings?: readonly SolanaSccpAccountOpeningInput[];
  vote_account_openings?: readonly SolanaSccpAccountOpeningInput[];
  validatorVoteAccountRawData?: readonly BinaryLike[];
  validator_vote_account_raw_data?: readonly BinaryLike[];
  voteAccountRawData?: readonly BinaryLike[];
  vote_account_raw_data?: readonly BinaryLike[];
  validatorStakeAccountOpenings?: readonly SolanaSccpAccountOpeningInput[];
  validator_stake_account_openings?: readonly SolanaSccpAccountOpeningInput[];
  stakeAccountOpenings?: readonly SolanaSccpAccountOpeningInput[];
  stake_account_openings?: readonly SolanaSccpAccountOpeningInput[];
  validatorStakeAccountRawData?: readonly BinaryLike[];
  validator_stake_account_raw_data?: readonly BinaryLike[];
  stakeAccountRawData?: readonly BinaryLike[];
  stake_account_raw_data?: readonly BinaryLike[];
  stakeHistorySysvarOpening?: SolanaSccpAccountOpeningInput;
  stake_history_sysvar_opening?: SolanaSccpAccountOpeningInput;
  stakeHistorySysvarRawData?: BinaryLike;
  stake_history_sysvar_raw_data?: BinaryLike;
}
export function solanaSccpAccountsLtHashOpenedResidual(
  input: SolanaSccpAccountsLtHashOpenedContributionsInput,
): Uint8Array;
export function solanaSccpAccountsLtHashOpenedResidualChecksum(
  input: SolanaSccpAccountsLtHashOpenedContributionsInput,
): string;
export function canonicalSolanaSccpAccountsLtHashOpenedContributionsBytes(
  input: SolanaSccpAccountsLtHashOpenedContributionsInput,
): Uint8Array;
export function solanaSccpAccountsLtHashOpenedContributionsHash(
  input: SolanaSccpAccountsLtHashOpenedContributionsInput,
): string;
export interface SolanaSccpOpenedAccountInclusionWitness {
  readonly root: string;
  readonly branches: ReadonlyArray<ReadonlyArray<string>>;
  readonly validatorVoteAccountBranches: ReadonlyArray<ReadonlyArray<string>>;
  readonly validatorStakeAccountBranches: ReadonlyArray<ReadonlyArray<string>>;
  readonly stakeHistorySysvarBranch: ReadonlyArray<string>;
}
export function solanaSccpOpenedAccountInclusionWitness(
  input: SolanaSccpAccountsLtHashOpenedContributionsInput,
): SolanaSccpOpenedAccountInclusionWitness;

export interface SolanaSccpAccountsLtHashProofRequestInput
  extends SolanaSccpAccountsLtHashOpenedContributionsInput {
  parentSlot?: string | number | bigint;
  parent_slot?: string | number | bigint;
  bankSignatureCount?: string | number | bigint;
  bank_signature_count?: string | number | bigint;
  parentBankHash?: string;
  parent_bank_hash?: string;
  blockhash?: string;
  blockhashBytes?: BinaryLike;
  blockhash_bytes?: BinaryLike;
  bankHash?: string;
  bank_hash?: string;
  transactionStatusRoot?: string;
  transaction_status_root?: string;
  bankHashHardForkData?: BinaryLike | number[];
  bank_hash_hard_fork_data?: BinaryLike | number[];
  sourceStateVerifierId?: string;
  source_state_verifier_id?: string;
  sourceStateVerifierHash?: string;
  source_state_verifier_hash?: string;
  accountsLtHashProofPublicInputsHash?: string;
  accounts_lt_hash_proof_public_inputs_hash?: string;
}

export interface SolanaSccpAccountsLtHashFastpqPublicInputs {
  readonly dsid: string;
  readonly slot: string;
  readonly oldRoot: string;
  readonly newRoot: string;
  readonly permRoot: string;
  readonly txSetHash: string;
}

export interface SolanaSccpAccountsLtHashFastpqTransition {
  readonly key: string;
  readonly operation: "meta_set";
  readonly oldValue: string;
  readonly newValue: string;
}

export type SccpSourceStateResultAlias<
  Camel extends string,
  Snake extends string,
  Value,
> =
  | ({ [Key in Camel]: Value } & { [Key in Snake]?: never })
  | ({ [Key in Snake]: Value } & { [Key in Camel]?: never });

export type SccpSourceStateOptionalResultAlias<
  Camel extends string,
  Snake extends string,
  Value,
> =
  | ({ [Key in Camel]?: Value } & { [Key in Snake]?: never })
  | ({ [Key in Snake]: Value } & { [Key in Camel]?: never });

export type SccpSourceStateOptionalVersionResultMetadata =
  | {
      version?: SccpVersionInput;
      proofVersion?: never;
      proof_version?: never;
    }
  | {
      version?: never;
      proofVersion: SccpVersionInput;
      proof_version?: never;
    }
  | {
      version?: never;
      proofVersion?: never;
      proof_version: SccpVersionInput;
    };

export type SccpSourceStateProofBytesResultMetadata =
  | {
      proofBytes: BinaryLike | number[];
      proof_bytes?: never;
      proof?: never;
    }
  | {
      proofBytes?: never;
      proof_bytes: BinaryLike | number[];
      proof?: never;
    }
  | {
      proofBytes?: never;
      proof_bytes?: never;
      proof: BinaryLike | number[];
    };

export type SccpSourceStateProofCapsuleMetadata<
  CircuitId extends string = string,
> = SccpSourceStateOptionalVersionResultMetadata &
  SccpSourceStateOptionalResultAlias<
    "proofFamily",
    "proof_family",
    typeof SCCP_STARK_FRI_PROOF_FAMILY_V1
  > &
  SccpSourceStateResultAlias<"circuitId", "circuit_id", CircuitId> &
  SccpSourceStateProofBytesResultMetadata &
  SccpSourceStateOptionalResultAlias<"proofBase64", "proof_base64", string>;

export type SccpSourceStateProverResultProofMetadata<
  CircuitId extends string = string,
> = SccpSourceStateOptionalVersionResultMetadata &
  SccpSourceStateOptionalResultAlias<
    "proofFamily",
    "proof_family",
    typeof SCCP_STARK_FRI_PROOF_FAMILY_V1
  > &
  SccpSourceStateOptionalResultAlias<"circuitId", "circuit_id", CircuitId> &
  SccpSourceStateProofBytesResultMetadata &
  SccpSourceStateOptionalResultAlias<"proofBase64", "proof_base64", string>;

export type SccpSourceStateFastpqPublicInputsResultMetadata = {
  dsid: string;
  slot: string | number | bigint;
} & SccpSourceStateResultAlias<"oldRoot", "old_root", string> &
  SccpSourceStateResultAlias<"newRoot", "new_root", string> &
  SccpSourceStateResultAlias<"permRoot", "perm_root", string> &
  SccpSourceStateResultAlias<"txSetHash", "tx_set_hash", string>;

export type SccpSourceStateFastpqTransitionResultMetadata = {
  key: string;
  operation: string;
} & SccpSourceStateResultAlias<"oldValue", "old_value", BinaryLike> &
  SccpSourceStateResultAlias<"newValue", "new_value", BinaryLike>;

export interface SolanaSccpAccountsLtHashProofRequest {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId: typeof SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly parameterSet: "fastpq-lane-balanced";
  readonly sourceDomain: typeof SCCP_DOMAIN_SOL;
  readonly finalizedSlot: string;
  readonly parentSlot: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly accountsLtHashProofPublicInputsHash: string;
  readonly openedAccountsLtHashContributionsHash: string;
  readonly openedAccountsLtHashResidualChecksum: string;
  readonly statementBytes: Uint8Array;
  readonly accountCommitmentBytes: Uint8Array;
  readonly verificationContextBytes: Uint8Array;
  readonly schemaDescriptor: Uint8Array;
  readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;
  readonly fastpqPublicInputs: Readonly<SolanaSccpAccountsLtHashFastpqPublicInputs>;
  readonly fastpqTransitions: ReadonlyArray<
    Readonly<SolanaSccpAccountsLtHashFastpqTransition>
  >;
}

export function canonicalSolanaSccpAccountsLtHashCommitmentBytes(
  input: SolanaSccpAccountsLtHashProofRequestInput,
): Uint8Array;
export function canonicalSolanaSccpAccountsLtHashVerificationContextBytes(
  input: SolanaSccpAccountsLtHashProofRequestInput,
): Uint8Array;
export function solanaSccpAccountsLtHashPublicInputColumns(
  input: SolanaSccpAccountsLtHashProofRequestInput,
): string[][];
export function solanaSccpAccountsLtHashOpenVerifySchemaDescriptor(
  input: SolanaSccpAccountsLtHashProofRequestInput,
): Uint8Array;
export function buildSolanaSccpAccountsLtHashProofRequest(
  input: SolanaSccpAccountsLtHashProofRequestInput,
): SolanaSccpAccountsLtHashProofRequest;

export type SolanaSccpSourceStateVerificationProof =
  SccpSourceStateProofCapsuleMetadata<
    | typeof SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
  >;

export interface WrappedSolanaSccpSourceStateVerificationProof {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId:
    | typeof SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
}

export function wrapSolanaSccpSourceStateVerificationProof(
  proofBytes: BinaryLike | number[],
  request:
    | SolanaSccpAccountsLtHashProofRequest
    | SolanaSccpFullLightClientAuditProofRequest,
): WrappedSolanaSccpSourceStateVerificationProof;

export type SolanaSccpSourceStateProofRequest =
  | SolanaSccpAccountsLtHashProofRequest
  | SolanaSccpFullLightClientAuditProofRequest;

export type SolanaSccpSourceStateProverResultObject =
  SccpSourceStateProverResultProofMetadata<
    | typeof SCCP_SOLANA_ACCOUNTS_LT_HASH_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1
  > & {
    parameterSet?: "fastpq-lane-balanced";
    parameter_set?: "fastpq-lane-balanced";
    sourceDomain?: typeof SCCP_DOMAIN_SOL;
    source_domain?: typeof SCCP_DOMAIN_SOL;
    finalizedSlot?: string | number | bigint;
    finalized_slot?: string | number | bigint;
    sourceStateVerifierId?: string;
    source_state_verifier_id?: string;
    sourceStateVerifierHash?: string;
    source_state_verifier_hash?: string;
    accountsLtHashProofPublicInputsHash?: string;
    accounts_lt_hash_proof_public_inputs_hash?: string;
    openedAccountsLtHashContributionsHash?: string;
    opened_accounts_lt_hash_contributions_hash?: string;
    openedAccountsLtHashResidualChecksum?: string;
    opened_accounts_lt_hash_residual_checksum?: string;
    role?: SolanaSccpFullLightClientAuditRole;
    audit_role?: SolanaSccpFullLightClientAuditRole;
    roleCode?: 1 | 2 | 3 | string | number | bigint;
    role_code?: 1 | 2 | 3 | string | number | bigint;
    verifierId?: string;
    verifier_id?: string;
    verifierHash?: string;
    verifier_hash?: string;
    sourceVerifierMaterialHash?: string;
    source_verifier_material_hash?: string;
    sourceAdapterDeploymentHash?: string;
    source_adapter_deployment_hash?: string;
    fullLightClientGateHash?: string;
    full_light_client_gate_hash?: string;
    finalityContextHash?: string;
    finality_context_hash?: string;
    voteMessageHash?: string;
    vote_message_hash?: string;
    accountsLtHashProofHash?: string;
    accounts_lt_hash_proof_hash?: string;
    auditStatementHash?: string;
    audit_statement_hash?: string;
    publicInputColumns?: ReadonlyArray<ReadonlyArray<string>>;
    public_input_columns?: ReadonlyArray<ReadonlyArray<string>>;
    fastpqPublicInputs?: SccpSourceStateFastpqPublicInputsResultMetadata;
    fastpq_public_inputs?: SccpSourceStateFastpqPublicInputsResultMetadata;
    fastpqTransitions?: ReadonlyArray<SccpSourceStateFastpqTransitionResultMetadata>;
    fastpq_transitions?: ReadonlyArray<SccpSourceStateFastpqTransitionResultMetadata>;
    statementBytes?: BinaryLike | number[];
    statement_bytes?: BinaryLike | number[];
    accountCommitmentBytes?: BinaryLike | number[];
    account_commitment_bytes?: BinaryLike | number[];
    verificationContextBytes?: BinaryLike | number[];
    verification_context_bytes?: BinaryLike | number[];
    schemaDescriptor?: BinaryLike | number[];
    schema_descriptor?: BinaryLike | number[];
  };

export type SolanaSccpSourceStateProveResult =
  | BinaryLike
  | number[]
  | SolanaSccpSourceStateProverResultObject;

export type SolanaSccpSourceStateProveFn = (
  request: SolanaSccpSourceStateProofRequest,
  options?: Record<string, unknown>,
) =>
  | SolanaSccpSourceStateProveResult
  | Promise<SolanaSccpSourceStateProveResult>;

export type SolanaSccpSourceStateProverOptions =
  SccpProverProveOption<SolanaSccpSourceStateProveFn>;

export class SolanaSccpSourceStateProver {
  constructor(options?: SolanaSccpSourceStateProverOptions);
  proveRequest(
    request: SolanaSccpSourceStateProofRequest,
    options?: Record<string, unknown>,
  ): Promise<WrappedSolanaSccpSourceStateVerificationProof>;
  proveAccountsLtHash(
    input: SolanaSccpAccountsLtHashProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<WrappedSolanaSccpSourceStateVerificationProof>;
  proveFullLightClientAudit(
    input: SolanaSccpFullLightClientAuditProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<
    Readonly<{
      readonly towerReplay: WrappedSolanaSccpSourceStateVerificationProof;
      readonly fullAccountsdbLattice: WrappedSolanaSccpSourceStateVerificationProof;
      readonly bankForkChoice: WrappedSolanaSccpSourceStateVerificationProof;
    }>
  >;
}

export type TonSccpSourceStateVerificationProof =
  SccpSourceStateProofCapsuleMetadata<
    | typeof SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1
  >;

export interface WrappedTonSccpSourceStateVerificationProof {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId:
    | typeof SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
}

export type SolanaSccpFullLightClientAuditRole =
  | "towerReplay"
  | "tower_replay"
  | "fullAccountsdbLattice"
  | "full_accountsdb_lattice"
  | "bankForkChoice"
  | "bank_fork_choice";

export interface SolanaSccpFullLightClientAuditProofRequestBaseInput
  extends SolanaSccpAccountsLtHashProofRequestInput {
  sourceVerifierMaterial?: unknown;
  source_verifier_material?: unknown;
  sourceAdapterDeployment?: unknown;
  source_adapter_deployment?: unknown;
  sourceTrustAnchorHash?: string;
  source_trust_anchor_hash?: string;
  consensusVerifierHash?: string;
  consensus_verifier_hash?: string;
  messageInclusionVerifierHash?: string;
  message_inclusion_verifier_hash?: string;
  finalityPolicyHash?: string;
  finality_policy_hash?: string;
  adapterVerifierVkHash?: string;
  adapter_verifier_vk_hash?: string;
  sourceVerifierMaterialHash?: string;
  source_verifier_material_hash?: string;
  sourceAdapterDeploymentHash?: string;
  source_adapter_deployment_hash?: string;
  sourceAdapterDeploymentReceiptHash?: string;
  source_adapter_deployment_receipt_hash?: string;
  fullLightClientGateHash?: string;
  full_light_client_gate_hash?: string;
  solanaTowerReplayVerifierHash?: string;
  solana_tower_replay_verifier_hash?: string;
  solanaFullAccountsdbLatticeVerifierHash?: string;
  solana_full_accountsdb_lattice_verifier_hash?: string;
  solanaBankForkChoiceVerifierHash?: string;
  solana_bank_fork_choice_verifier_hash?: string;
  deploymentReceiptHash?: string;
  deployment_receipt_hash?: string;
  epoch?: string | number | bigint;
  rootedSlot?: string | number | bigint;
  rooted_slot?: string | number | bigint;
  towerVoteSlots?: readonly (string | number | bigint)[];
  tower_vote_slots?: readonly (string | number | bigint)[];
  epochStakeRoot?: string;
  epoch_stake_root?: string;
  stakeActivationHash?: string;
  stake_activation_hash?: string;
  stakeAccountStateHash?: string;
  stake_account_state_hash?: string;
  stakeHistoryHash?: string;
  stake_history_hash?: string;
  stakeHistorySysvarAccountHash?: string;
  stake_history_sysvar_account_hash?: string;
  towerLockoutHash?: string;
  tower_lockout_hash?: string;
  towerReplayHash?: string;
  tower_replay_hash?: string;
  bankForkHash?: string;
  bank_fork_hash?: string;
  finalityContextHash?: string;
  finality_context_hash?: string;
  voteMessageHash?: string;
  vote_message_hash?: string;
  accountsLtHashProofHash?: string;
  accounts_lt_hash_proof_hash?: string;
  openedAccountsLtHashContributionsHash?: string;
  opened_accounts_lt_hash_contributions_hash?: string;
  openedAccountsLtHashResidualChecksum?: string;
  opened_accounts_lt_hash_residual_checksum?: string;
}

export type SolanaSccpFullLightClientAuditProofRequestInput =
  SolanaSccpFullLightClientAuditProofRequestBaseInput &
    (
      | {
          accountsLtHashProof: SolanaSccpSourceStateVerificationProof;
          accounts_lt_hash_proof?: never;
        }
      | {
          accountsLtHashProof?: never;
          accounts_lt_hash_proof: SolanaSccpSourceStateVerificationProof;
        }
    );

export interface SolanaSccpFullLightClientAuditProofRequest {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId:
    | typeof SCCP_SOLANA_TOWER_REPLAY_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_FULL_ACCOUNTSDB_LATTICE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_SOLANA_BANK_FORK_CHOICE_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly parameterSet: "fastpq-lane-balanced";
  readonly role:
    | "tower_replay"
    | "full_accountsdb_lattice"
    | "bank_fork_choice";
  readonly roleCode: 1 | 2 | 3;
  readonly sourceDomain: typeof SCCP_DOMAIN_SOL;
  readonly finalizedSlot: string;
  readonly verifierId: string;
  readonly verifierHash: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly sourceVerifierMaterialHash: string;
  readonly sourceAdapterDeploymentHash: string;
  readonly fullLightClientGateHash: string;
  readonly finalityContextHash: string;
  readonly voteMessageHash: string;
  readonly accountsLtHashProofHash: string;
  readonly auditStatementHash: string;
  readonly statementBytes: Uint8Array;
  readonly verificationContextBytes: Uint8Array;
  readonly schemaDescriptor: Uint8Array;
  readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;
  readonly fastpqPublicInputs: Readonly<SolanaSccpAccountsLtHashFastpqPublicInputs>;
  readonly fastpqTransitions: ReadonlyArray<
    Readonly<SolanaSccpAccountsLtHashFastpqTransition>
  >;
}

export function canonicalSolanaSccpSourceStateVerificationProofBytes(
  input: SolanaSccpSourceStateVerificationProof,
): Uint8Array;
export function solanaSccpAccountsLtHashProofHash(
  input: SolanaSccpSourceStateVerificationProof,
): string;
export function canonicalSolanaSccpFinalityContextBytes(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
): Uint8Array;
export function solanaSccpFinalityContextHash(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
): string;
export function canonicalSolanaSccpVoteMessageBytes(input: unknown): Uint8Array;
export function solanaSccpVoteMessageHash(input: unknown): string;
export function canonicalSolanaSccpFullLightClientAuditStatementBytes(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
  role: SolanaSccpFullLightClientAuditRole,
): Uint8Array;
export function solanaSccpFullLightClientAuditStatementHash(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
  role: SolanaSccpFullLightClientAuditRole,
): string;
export function solanaSccpFullLightClientAuditPublicInputColumns(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
  role: SolanaSccpFullLightClientAuditRole,
): string[][];
export function solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
  role: SolanaSccpFullLightClientAuditRole,
): Uint8Array;
export function buildSolanaSccpFullLightClientAuditProofRequest(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
  role: SolanaSccpFullLightClientAuditRole,
): SolanaSccpFullLightClientAuditProofRequest;
export function buildSolanaSccpTowerReplayProofRequest(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
): SolanaSccpFullLightClientAuditProofRequest;
export function buildSolanaSccpFullAccountsdbLatticeProofRequest(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
): SolanaSccpFullLightClientAuditProofRequest;
export function buildSolanaSccpBankForkChoiceProofRequest(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
): SolanaSccpFullLightClientAuditProofRequest;
export function buildSolanaSccpFullLightClientAuditProofRequests(
  input: SolanaSccpFullLightClientAuditProofRequestInput,
): Readonly<{
  readonly towerReplay: SolanaSccpFullLightClientAuditProofRequest;
  readonly fullAccountsdbLattice: SolanaSccpFullLightClientAuditProofRequest;
  readonly bankForkChoice: SolanaSccpFullLightClientAuditProofRequest;
}>;

export interface SolanaSccpVoteAccountDataInput {
  nodePubkey?: BinaryLike;
  node_pubkey?: BinaryLike;
  authorizedVoter?: BinaryLike;
  authorized_voter?: BinaryLike;
  authorizedWithdrawer?: BinaryLike;
  authorized_withdrawer?: BinaryLike;
  voteAccountAddress?: BinaryLike;
  vote_account_address?: BinaryLike;
  inflationRewardsCollector?: BinaryLike;
  inflation_rewards_collector?: BinaryLike;
  blockRevenueCollector?: BinaryLike;
  block_revenue_collector?: BinaryLike;
  commission?: string | number | bigint;
  inflationRewardsCommissionBps?: string | number | bigint;
  inflation_rewards_commission_bps?: string | number | bigint;
  blockRevenueCommissionBps?: string | number | bigint;
  block_revenue_commission_bps?: string | number | bigint;
  pendingDelegatorRewards?: string | number | bigint;
  pending_delegator_rewards?: string | number | bigint;
  blsPubkeyCompressed?: BinaryLike;
  bls_pubkey_compressed?: BinaryLike;
  rootSlot?: string | number | bigint;
  root_slot?: string | number | bigint;
  towerVoteSlots?: readonly (string | number | bigint)[];
  tower_vote_slots?: readonly (string | number | bigint)[];
}

export interface SolanaSccpParsedVoteStateAccountData {
  nodePubkey: Uint8Array;
  authorizedVoter: Uint8Array;
  authorizedWithdrawer: Uint8Array;
  inflationRewardsCollector: Uint8Array;
  blockRevenueCollector: Uint8Array;
  inflationRewardsCommissionBps: bigint;
  blockRevenueCommissionBps: bigint;
  pendingDelegatorRewards: bigint;
  blsPubkeyCompressed: Uint8Array;
  rootSlot: bigint;
  towerVoteSlots: bigint[];
}

export type SolanaSccpParsedVoteStateV1OrV3AccountData =
  SolanaSccpParsedVoteStateAccountData;

export interface SolanaSccpStakeAccountDataInput {
  staker: BinaryLike;
  withdrawer: BinaryLike;
  voterPubkey?: BinaryLike;
  voter_pubkey?: BinaryLike;
  delegatedStake?: string | number | bigint;
  delegated_stake?: string | number | bigint;
  activationEpoch?: string | number | bigint;
  activation_epoch?: string | number | bigint;
  deactivationEpoch?: string | number | bigint;
  deactivation_epoch?: string | number | bigint;
  warmupCooldownRateBytes?: BinaryLike;
  warmup_cooldown_rate_bytes?: BinaryLike;
  creditsObserved?: string | number | bigint;
  credits_observed?: string | number | bigint;
  stakeFlags?: string | number | bigint;
  stake_flags?: string | number | bigint;
}

export interface SolanaSccpParsedStakeStateV2StakeAccountData {
  staker: Uint8Array;
  withdrawer: Uint8Array;
  voterPubkey: Uint8Array;
  delegatedStake: bigint;
  activationEpoch: bigint;
  deactivationEpoch: bigint;
  warmupCooldownRateBytes: Uint8Array;
  creditsObserved: bigint;
  stakeFlags: bigint;
}

export interface SolanaSccpStakeAccountStateInput
  extends SolanaSccpStakeActivationInput {
  validatorVoteAccountAddresses?: readonly BinaryLike[];
  validator_vote_account_addresses?: readonly BinaryLike[];
  voteAccountAddresses?: readonly BinaryLike[];
  vote_account_addresses?: readonly BinaryLike[];
  validatorStakeAccountAddresses?: readonly BinaryLike[];
  validator_stake_account_addresses?: readonly BinaryLike[];
  stakeAccountAddresses?: readonly BinaryLike[];
  stake_account_addresses?: readonly BinaryLike[];
  validatorVoteAccountHashes?: readonly BinaryLike[];
  validator_vote_account_hashes?: readonly BinaryLike[];
  voteAccountHashes?: readonly BinaryLike[];
  vote_account_hashes?: readonly BinaryLike[];
  validatorStakeAccountHashes?: readonly BinaryLike[];
  validator_stake_account_hashes?: readonly BinaryLike[];
  stakeAccountHashes?: readonly BinaryLike[];
  stake_account_hashes?: readonly BinaryLike[];
}

export interface SolanaSccpStakeHistoryEntryInput {
  epoch: string | number | bigint;
  effective: string | number | bigint;
  activating: string | number | bigint;
  deactivating: string | number | bigint;
}

export interface SolanaSccpStakeHistoryInput
  extends SolanaSccpStakeAccountStateInput {
  validatorDelegatedStakes?: readonly (string | number | bigint)[];
  validator_delegated_stakes?: readonly (string | number | bigint)[];
  delegatedStakes?: readonly (string | number | bigint)[];
  delegated_stakes?: readonly (string | number | bigint)[];
  stakeHistoryEntries?: readonly SolanaSccpStakeHistoryEntryInput[];
  stake_history_entries?: readonly SolanaSccpStakeHistoryEntryInput[];
  stakeHistory?: readonly SolanaSccpStakeHistoryEntryInput[];
  stake_history?: readonly SolanaSccpStakeHistoryEntryInput[];
}

export interface SolanaSccpStakeHistorySysvarDataInput {
  stakeHistoryEntries?: readonly SolanaSccpStakeHistoryEntryInput[];
  stake_history_entries?: readonly SolanaSccpStakeHistoryEntryInput[];
  stakeHistory?: readonly SolanaSccpStakeHistoryEntryInput[];
  stake_history?: readonly SolanaSccpStakeHistoryEntryInput[];
}

export interface SolanaSccpTowerLockoutInput {
  epoch?: string | number | bigint;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  rootedSlot?: string | number | bigint;
  rooted_slot?: string | number | bigint;
  parentSlot?: string | number | bigint;
  parent_slot?: string | number | bigint;
  parentBankHash?: string;
  parent_bank_hash?: string;
}

export interface SolanaSccpTowerReplayInput {
  epoch?: string | number | bigint;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  rootedSlot?: string | number | bigint;
  rooted_slot?: string | number | bigint;
  parentSlot?: string | number | bigint;
  parent_slot?: string | number | bigint;
  bankForkHash?: string;
  bank_fork_hash?: string;
  towerVoteSlots?: readonly (string | number | bigint)[];
  tower_vote_slots?: readonly (string | number | bigint)[];
  voteSlots?: readonly (string | number | bigint)[];
  vote_slots?: readonly (string | number | bigint)[];
}

export interface SolanaSccpBankForkInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  epoch?: string | number | bigint;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  parentSlot?: string | number | bigint;
  parent_slot?: string | number | bigint;
  bankSignatureCount?: string | number | bigint;
  bank_signature_count?: string | number | bigint;
  parentBankHash?: string;
  parent_bank_hash?: string;
  bankHash?: string;
  bank_hash?: string;
  blockhash?: string;
  blockhashBytes?: BinaryLike;
  blockhash_bytes?: BinaryLike;
  accountsLtHash?: BinaryLike | number[];
  accounts_lt_hash?: BinaryLike | number[];
  bankHashHardForkData?: BinaryLike | number[];
  bank_hash_hard_fork_data?: BinaryLike | number[];
  transactionStatusRoot?: string;
  transaction_status_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  accountInclusionRoot?: string;
  account_inclusion_root?: string;
  accountsRoot?: string;
  accounts_root?: string;
  accountsLtHashChecksum?: string;
  accounts_lt_hash_checksum?: string;
  accountsLtHashRoot?: string;
  accounts_lt_hash_root?: string;
}

export type SolanaSccpAccountsLtHashProofPublicInputsInput =
  SolanaSccpBankForkInput;

export interface SolanaSccpAgaveBankHashInput {
  parentBankHash?: string;
  parent_bank_hash?: string;
  bankSignatureCount?: string | number | bigint;
  bank_signature_count?: string | number | bigint;
  blockhash?: string;
  accountsLtHash?: BinaryLike | number[];
  accounts_lt_hash?: BinaryLike | number[];
  bankHashHardForkData?: BinaryLike | number[];
  bank_hash_hard_fork_data?: BinaryLike | number[];
}

export interface SolanaSccpProofContextInput {
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  destinationBinding?:
    | TonSccpDestinationBindingInput
    | EvmSccpDestinationBindingInput
    | TronSccpDestinationBindingInput;
  destination_binding?:
    | TonSccpDestinationBindingInput
    | EvmSccpDestinationBindingInput
    | TronSccpDestinationBindingInput;
}

export interface SolanaSccpProofContext {
  version: 1;
  statementHash: string;
  destinationBindingHash: string;
}

export interface SccpSourceAdapterDeploymentBindingInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  sourceAdapterDeploymentHash?: string;
  source_adapter_deployment_hash?: string;
  sourceAdapterDeploymentReceiptHash?: string;
  source_adapter_deployment_receipt_hash?: string;
}

export interface SccpSourceAdapterDeploymentBinding {
  version: 1;
  sourceDomain: number;
  targetDomain: number;
  sourceAdapterDeploymentHash: string;
  sourceAdapterDeploymentReceiptHash: string;
}

export type SccpDestinationBindingDomainInput =
  | SccpDomainIdInput
  | {
      targetDomain?: SccpDomainIdInput;
      target_domain?: SccpDomainIdInput;
      domain?: SccpDomainIdInput;
    };

export interface EvmSccpDestinationBindingInput {
  version?: SccpVersionInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  networkId?: string;
  network_id?: string;
  networkIdHex?: string;
  network_id_hex?: string;
  verifierAddress?: string;
  verifier_address?: string;
  verifierAddressHex?: string;
  verifier_address_hex?: string;
  bridgeAddress?: string;
  bridge_address?: string;
  bridgeAddressHex?: string;
  bridge_address_hex?: string;
  verifierCodeHash?: string;
  verifier_code_hash?: string;
  verifierCodeHashHex?: string;
  verifier_code_hash_hex?: string;
  verifierKeyHash?: string;
  verifier_key_hash?: string;
  verifierKeyHashHex?: string;
  verifier_key_hash_hex?: string;
  verifierBackend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  verifier_backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proofFamily?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  proof_family?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  key?: string;
  bindingKey?: string;
  binding_key?: string;
  bindingHash?: string;
  binding_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
}

export interface EvmSccpDestinationBinding {
  readonly version: 1;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly networkId: string;
  readonly verifierAddress: string;
  readonly bridgeAddress: string;
  readonly verifierCodeHash: string;
  readonly verifierKeyHash: string;
  readonly verifierBackend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly key: string;
  readonly bindingHash: string;
}

export interface TronSccpDestinationBindingInput {
  version?: SccpVersionInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  networkId?: string;
  network_id?: string;
  networkIdHex?: string;
  network_id_hex?: string;
  verifierAddress?: string;
  verifier_address?: string;
  verifierCodeHash?: string;
  verifier_code_hash?: string;
  verifierCodeHashHex?: string;
  verifier_code_hash_hex?: string;
  verifierKeyHash?: string;
  verifier_key_hash?: string;
  verifierKeyHashHex?: string;
  verifier_key_hash_hex?: string;
  verifierBackend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  verifier_backend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  backend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  proofFamily?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  proof_family?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  key?: string;
  bindingKey?: string;
  binding_key?: string;
  bindingHash?: string;
  binding_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
}

export interface TronSccpDestinationBinding {
  readonly version: 1;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly networkId: string;
  readonly verifierAddress: string;
  readonly verifierCodeHash: string;
  readonly verifierKeyHash: string;
  readonly verifierBackend: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly key: string;
  readonly bindingHash: string;
}

export interface SccpSourceVerifierMaterialInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  sourceTrustAnchorHash?: string;
  source_trust_anchor_hash?: string;
  consensusVerifierHash?: string;
  consensus_verifier_hash?: string;
  messageInclusionVerifierHash?: string;
  message_inclusion_verifier_hash?: string;
  finalityPolicyHash?: string;
  finality_policy_hash?: string;
  sourceStateVerifierHash?: string;
  source_state_verifier_hash?: string;
  sourceBridgeEmitterAddress?: string;
  source_bridge_emitter_address?: string;
  bridgeAddress?: string;
  bridge_address?: string;
  sourceBridgeEmitterCodeHash?: string;
  source_bridge_emitter_code_hash?: string;
  sourceBridgeNetworkId?: string;
  source_bridge_network_id?: string;
  networkId?: string;
  network_id?: string;
  sourceBridgeOwnerAddress?: string;
  source_bridge_owner_address?: string;
  ownerAddress?: string;
  owner_address?: string;
  sourceBridgeConfigHash?: string;
  source_bridge_config_hash?: string;
  configHash?: string;
  config_hash?: string;
}

export interface SccpSourceAdapterEngineDeploymentInput
  extends SccpSourceVerifierMaterialInput {
  adapterProofFamily?: string;
  adapter_proof_family?: string;
  adapterVerifierVkHash?: string;
  adapter_verifier_vk_hash?: string;
  solanaTowerReplayVerifierHash?: string;
  solana_tower_replay_verifier_hash?: string;
  solanaFullAccountsdbLatticeVerifierHash?: string;
  solana_full_accountsdb_lattice_verifier_hash?: string;
  solanaBankForkChoiceVerifierHash?: string;
  solana_bank_fork_choice_verifier_hash?: string;
  tonMasterchainConfigVerifierHash?: string;
  ton_masterchain_config_verifier_hash?: string;
  tonValidatorSetTransitionVerifierHash?: string;
  ton_validator_set_transition_verifier_hash?: string;
  tonShardAccountsDictionaryVerifierHash?: string;
  ton_shard_accounts_dictionary_verifier_hash?: string;
  deploymentReceiptHash?: string;
  deployment_receipt_hash?: string;
}

export interface SccpSourceVerifierMaterial {
  version: 1;
  sourceDomain: number;
  sourceChain: string;
  sourceProofPlan: number;
  finalityModel: number;
  adapterCircuitId: string;
  sourceTrustAnchorId: string;
  sourceTrustAnchorHash: string;
  consensusVerifierId: string;
  consensusVerifierHash: string;
  messageInclusionVerifierId: string;
  messageInclusionVerifierHash: string;
  finalityPolicyId: string;
  finalityPolicyHash: string;
  sourceStateVerifierId: string;
  sourceStateVerifierHash: string;
  sourceBridgeEmitterId: string;
  sourceBridgeEmitterAddress: string;
  sourceBridgeEmitterCodeHash: string;
  sourceBridgeNetworkId: string;
  sourceBridgeOwnerAddress: string;
  sourceBridgeConfigHash: string;
  placeholderMaterial: false;
}

export interface SccpSourceAdapterEngineDeployment
  extends SccpSourceVerifierMaterial {
  targetDomain: number;
  adapterProofFamily: string;
  adapterVerifierVkHash: string;
  solanaTowerReplayVerifierHash: string;
  solanaFullAccountsdbLatticeVerifierHash: string;
  solanaBankForkChoiceVerifierHash: string;
  tonMasterchainConfigVerifierHash: string;
  tonValidatorSetTransitionVerifierHash: string;
  tonShardAccountsDictionaryVerifierHash: string;
  deploymentReceiptHash: string;
}

export interface SolanaSccpMessageProofInput {
  sourceEventDigest?: string;
  source_event_digest?: string;
  transactionStatusRoot?: string;
  transaction_status_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  transactionSignature?: string;
  transaction_signature?: string;
  emitterProgramId?: string;
  emitter_program_id?: string;
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TonSccpShardProofInput {
  sourceEventDigest?: string;
  source_event_digest?: string;
  masterchainSeqno?: string | number | bigint;
  masterchain_seqno?: string | number | bigint;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  masterchainBlockHash?: string;
  masterchain_block_hash?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  shardWorkchainId?: string | number | bigint;
  shard_workchain_id?: string | number | bigint;
  shardShard?: string | number | bigint;
  shard_shard?: string | number | bigint;
  shardSeqno?: string | number | bigint;
  shard_seqno?: string | number | bigint;
  shardBlockHash?: string;
  shard_block_hash?: string;
  shardFileHash?: string;
  shard_file_hash?: string;
  shardStateRoot?: string;
  shard_state_root?: string;
  transactionRoot?: string;
  transaction_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  transactionLt?: string | number | bigint;
  transaction_lt?: string | number | bigint;
  shardStateLeafIndex?: string | number | bigint;
  shard_state_leaf_index?: string | number | bigint;
  shardStateInclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  shard_state_inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  shardStateProofBoc?: BinaryLike;
  shard_state_proof_boc?: BinaryLike;
  shardStateDictionaryRoot?: string;
  shard_state_dictionary_root?: string;
  shardStateDictionaryKeyBitLen?: string | number | bigint;
  shard_state_dictionary_key_bit_len?: string | number | bigint;
  shardStateDictionaryKey?: BinaryLike;
  shard_state_dictionary_key?: BinaryLike;
  shardStateDictionaryProofBoc?: BinaryLike;
  shard_state_dictionary_proof_boc?: BinaryLike;
}

export interface TonShardStateTransitionProofInput
  extends TonValidatorSetTransitionMessageInput {
  version?: SccpVersionInput;
  transitionMessageHash?: string;
  transition_message_hash?: string;
  transitionSignatureHash?: string;
  transition_signature_hash?: string;
  validatorSignatureProof?: TonValidatorSignatureProofInput;
  validator_signature_proof?: TonValidatorSignatureProofInput;
}

export interface TonShardStateProofRequestInput
  extends TonMasterchainBlockMessageInput {
  transactionLt?: string | number | bigint;
  transaction_lt?: string | number | bigint;
  shardStateProofBoc?: BinaryLike;
  shard_state_proof_boc?: BinaryLike;
  shardStateDictionaryRoot?: string;
  shard_state_dictionary_root?: string;
  shardStateDictionaryKeyBitLen?: string | number | bigint;
  shard_state_dictionary_key_bit_len?: string | number | bigint;
  shardStateDictionaryKey?: BinaryLike;
  shard_state_dictionary_key?: BinaryLike;
  shardStateDictionaryProofBoc?: BinaryLike;
  shard_state_dictionary_proof_boc?: BinaryLike;
  configDictionaryProofBoc?: BinaryLike;
  config_dictionary_proof_boc?: BinaryLike;
  masterchainConfigProofBoc?: BinaryLike;
  masterchain_config_proof_boc?: BinaryLike;
  masterchainConfigProof?: {
    configDictionaryProofBoc?: BinaryLike;
    config_dictionary_proof_boc?: BinaryLike;
    validatorSetPayloadHash?: string;
    validator_set_payload_hash?: string;
    configLeafHash?: string;
    config_leaf_hash?: string;
    configLeafIndex?: string | number | bigint;
    config_leaf_index?: string | number | bigint;
    configValueHash?: string;
    config_value_hash?: string;
  };
  masterchain_config_proof?: {
    configDictionaryProofBoc?: BinaryLike;
    config_dictionary_proof_boc?: BinaryLike;
    validatorSetPayloadHash?: string;
    validator_set_payload_hash?: string;
    configLeafHash?: string;
    config_leaf_hash?: string;
    configLeafIndex?: string | number | bigint;
    config_leaf_index?: string | number | bigint;
    configValueHash?: string;
    config_value_hash?: string;
  };
  validatorSetPayloadHash?: string;
  validator_set_payload_hash?: string;
  configLeafHash?: string;
  config_leaf_hash?: string;
  configLeafIndex?: string | number | bigint;
  config_leaf_index?: string | number | bigint;
  configValueHash?: string;
  config_value_hash?: string;
  masterchainSignatureHash?: string;
  masterchain_signature_hash?: string;
  validatorSetTransitionProofs?: readonly TonShardStateTransitionProofInput[];
  validator_set_transition_proofs?: readonly TonShardStateTransitionProofInput[];
  sourceStateVerifierId?: string;
  source_state_verifier_id?: string;
  sourceStateVerifierHash?: string;
  source_state_verifier_hash?: string;
  sourceTrustAnchorId?: string;
  source_trust_anchor_id?: string;
  sourceTrustAnchorHash?: string;
  source_trust_anchor_hash?: string;
  consensusVerifierId?: string;
  consensus_verifier_id?: string;
  consensusVerifierHash?: string;
  consensus_verifier_hash?: string;
  messageInclusionVerifierId?: string;
  message_inclusion_verifier_id?: string;
  messageInclusionVerifierHash?: string;
  message_inclusion_verifier_hash?: string;
  finalityPolicyId?: string;
  finality_policy_id?: string;
  finalityPolicyHash?: string;
  finality_policy_hash?: string;
}

export interface TonShardStateFastpqPublicInputs {
  readonly dsid: string;
  readonly slot: string;
  readonly oldRoot: string;
  readonly newRoot: string;
  readonly permRoot: string;
  readonly txSetHash: string;
}

export interface TonShardStateFastpqTransition {
  readonly key: string;
  readonly operation: "meta_set";
  readonly oldValue: "0x";
  readonly newValue: string;
}

export interface TonShardStateProofRequest {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId: typeof SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly parameterSet: "fastpq-lane-balanced";
  readonly sourceDomain: typeof SCCP_DOMAIN_TON;
  readonly masterchainSeqno: string;
  readonly shardSeqno: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly shardStateProofPublicInputsHash: string;
  readonly statementBytes: Uint8Array;
  readonly witnessCommitmentBytes: Uint8Array;
  readonly verificationContextBytes: Uint8Array;
  readonly schemaDescriptor: Uint8Array;
  readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;
  readonly fastpqPublicInputs: Readonly<TonShardStateFastpqPublicInputs>;
  readonly fastpqTransitions: ReadonlyArray<
    Readonly<TonShardStateFastpqTransition>
  >;
}

export type TonSccpFullLightClientAuditRole =
  | "masterchainConfig"
  | "masterchain_config"
  | "validatorSetTransition"
  | "validator_set_transition"
  | "shardAccountsDictionary"
  | "shard_accounts_dictionary";

export interface TonSccpFullLightClientAuditProofRequestBaseInput
  extends TonShardStateProofRequestInput {
  sourceVerifierMaterial?: unknown;
  source_verifier_material?: unknown;
  sourceAdapterDeployment?: unknown;
  source_adapter_deployment?: unknown;
  tonMasterchainConfigVerifierHash?: string;
  ton_masterchain_config_verifier_hash?: string;
  tonValidatorSetTransitionVerifierHash?: string;
  ton_validator_set_transition_verifier_hash?: string;
  tonShardAccountsDictionaryVerifierHash?: string;
  ton_shard_accounts_dictionary_verifier_hash?: string;
  deploymentReceiptHash?: string;
  deployment_receipt_hash?: string;
  shardStateVerificationProofHash?: string;
  shard_state_verification_proof_hash?: string;
  shardStateProofPublicInputsHash?: string;
  shard_state_proof_public_inputs_hash?: string;
}

export type TonSccpFullLightClientAuditProofRequestInput =
  TonSccpFullLightClientAuditProofRequestBaseInput &
    (
      | {
          shardStateVerificationProof: TonSccpSourceStateVerificationProof;
          shard_state_verification_proof?: never;
        }
      | {
          shardStateVerificationProof?: never;
          shard_state_verification_proof: TonSccpSourceStateVerificationProof;
        }
    );

export function canonicalTonSccpSourceStateVerificationProofBytes(
  input: TonSccpSourceStateVerificationProof,
): Uint8Array;

export interface TonSccpFullLightClientAuditProofRequest {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId:
    | typeof SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly parameterSet: "fastpq-lane-balanced";
  readonly role:
    | "masterchain_config"
    | "validator_set_transition"
    | "shard_accounts_dictionary";
  readonly roleCode: 1 | 2 | 3;
  readonly sourceDomain: typeof SCCP_DOMAIN_TON;
  readonly masterchainSeqno: string;
  readonly shardSeqno: string;
  readonly verifierId: string;
  readonly verifierHash: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly sourceVerifierMaterialHash: string;
  readonly sourceAdapterDeploymentHash: string;
  readonly fullLightClientGateHash: string;
  readonly shardStateProofPublicInputsHash: string;
  readonly shardStateVerificationProofHash: string;
  readonly auditStatementHash: string;
  readonly statementBytes: Uint8Array;
  readonly verificationContextBytes: Uint8Array;
  readonly schemaDescriptor: Uint8Array;
  readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;
  readonly fastpqPublicInputs: Readonly<TonShardStateFastpqPublicInputs>;
  readonly fastpqTransitions: ReadonlyArray<
    Readonly<TonShardStateFastpqTransition>
  >;
}

export interface TonValidatorSetPayloadInput {
  validatorPublicKeys?: readonly string[];
  validator_public_keys?: readonly string[];
  validatorWeights?: readonly (string | number | bigint)[];
  validator_weights?: readonly (string | number | bigint)[];
}

export interface TonMasterchainConfigLeafInput {
  version?: SccpVersionInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  masterchainSeqno?: string | number | bigint;
  masterchain_seqno?: string | number | bigint;
  masterchainBlockHash?: string;
  masterchain_block_hash?: string;
  shardStateRoot?: string;
  shard_state_root?: string;
  validatorSetHash?: string;
  validator_set_hash?: string;
  validatorSetPayloadHash?: string;
  validator_set_payload_hash?: string;
}

export interface TonMasterchainConfigProofInput
  extends TonMasterchainConfigLeafInput {
  configRoot?: string;
  config_root?: string;
  configLeafHash?: string;
  config_leaf_hash?: string;
  configLeafIndex?: string | number | bigint;
  config_leaf_index?: string | number | bigint;
  configValueHash?: string;
  config_value_hash?: string;
  configDictionaryProofBoc?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  config_dictionary_proof_boc?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  configInclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  config_inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TonMasterchainBlockMessageInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  masterchainSeqno?: string | number | bigint;
  masterchain_seqno?: string | number | bigint;
  masterchainWorkchainId?: string | number | bigint;
  masterchain_workchain_id?: string | number | bigint;
  masterchainShard?: string | number | bigint;
  masterchain_shard?: string | number | bigint;
  masterchainBlockHash?: string;
  masterchain_block_hash?: string;
  masterchainFileHash?: string;
  masterchain_file_hash?: string;
  validatorSetHash?: string;
  validator_set_hash?: string;
  masterchainConfigRoot?: string;
  masterchain_config_root?: string;
  masterchainConfigProofHash?: string;
  masterchain_config_proof_hash?: string;
  shardWorkchainId?: string | number | bigint;
  shard_workchain_id?: string | number | bigint;
  shardShard?: string | number | bigint;
  shard_shard?: string | number | bigint;
  shardSeqno?: string | number | bigint;
  shard_seqno?: string | number | bigint;
  shardBlockHash?: string;
  shard_block_hash?: string;
  shardFileHash?: string;
  shard_file_hash?: string;
  shardStateRoot?: string;
  shard_state_root?: string;
  transactionRoot?: string;
  transaction_root?: string;
  shardProofHash?: string;
  shard_proof_hash?: string;
}

export interface TonValidatorSetTransitionMessageInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  fromValidatorSetSeqno?: string | number | bigint;
  from_validator_set_seqno?: string | number | bigint;
  toValidatorSetSeqno?: string | number | bigint;
  to_validator_set_seqno?: string | number | bigint;
  masterchainSeqno?: string | number | bigint;
  masterchain_seqno?: string | number | bigint;
  masterchainWorkchainId?: string | number | bigint;
  masterchain_workchain_id?: string | number | bigint;
  masterchainShard?: string | number | bigint;
  masterchain_shard?: string | number | bigint;
  masterchainBlockHash?: string;
  masterchain_block_hash?: string;
  masterchainFileHash?: string;
  masterchain_file_hash?: string;
  parentValidatorSetHash?: string;
  parent_validator_set_hash?: string;
  nextValidatorSetHash?: string;
  next_validator_set_hash?: string;
  nextValidatorSetPayload?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  next_validator_set_payload?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  nextValidatorSetPayloadHash?: string;
  next_validator_set_payload_hash?: string;
  nextValidatorSetConfigHash?: string;
  next_validator_set_config_hash?: string;
}

export interface TonValidatorSignatureProofInput {
  version?: SccpVersionInput;
  totalWeight?: string | number | bigint;
  total_weight?: string | number | bigint;
  signedWeight?: string | number | bigint;
  signed_weight?: string | number | bigint;
  blockMessageHash?: string;
  block_message_hash?: string;
  validatorPublicKeys?: readonly string[];
  validator_public_keys?: readonly string[];
  validatorWeights?: readonly (string | number | bigint)[];
  validator_weights?: readonly (string | number | bigint)[];
  validatorSetHash?: string;
  validator_set_hash?: string;
  signersBitmap?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  signers_bitmap?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  signatures?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TonValidatorSetTransitionSignatureInput
  extends TonValidatorSetTransitionMessageInput {
  version?: SccpVersionInput;
  transitionMessageHash?: string;
  transition_message_hash?: string;
  validatorSignatureProof?: TonValidatorSignatureProofInput;
  validator_signature_proof?: TonValidatorSignatureProofInput;
}

export interface EvmSccpReceiptProofInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  sourceEventDigest?: string;
  source_event_digest?: string;
  beaconSlot?: string | number | bigint;
  beacon_slot?: string | number | bigint;
  executionBlockNumber?: string | number | bigint;
  execution_block_number?: string | number | bigint;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  executionBlockHash?: string;
  execution_block_hash?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  executionReceiptsRoot?: string;
  execution_receipts_root?: string;
  receiptsRoot?: string;
  receipts_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  beaconFinalizedRoot?: string;
  beacon_finalized_root?: string;
  syncCommitteeRoot?: string;
  sync_committee_root?: string;
  receiptRootIndex?: string | number | bigint;
  receipt_root_index?: string | number | bigint;
  receiptTrieProofNodes?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  receipt_trie_proof_nodes?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface EthSyncCommitteePayloadInput {
  syncCommitteePublicKeys?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  sync_committee_public_keys?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  syncCommitteeWeights?: readonly (string | number | bigint)[];
  sync_committee_weights?: readonly (string | number | bigint)[];
  syncCommitteePops?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  sync_committee_pops?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface EthBeaconBlockHeaderRootInput {
  slot?: string | number | bigint;
  beaconSlot?: string | number | bigint;
  beacon_slot?: string | number | bigint;
  proposerIndex?: string | number | bigint;
  proposer_index?: string | number | bigint;
  beaconProposerIndex?: string | number | bigint;
  beacon_proposer_index?: string | number | bigint;
  parentRoot?: string;
  parent_root?: string;
  beaconParentRoot?: string;
  beacon_parent_root?: string;
  stateRoot?: string;
  state_root?: string;
  beaconStateRoot?: string;
  beacon_state_root?: string;
  bodyRoot?: string;
  body_root?: string;
  beaconBodyRoot?: string;
  beacon_body_root?: string;
}

export interface EthSyncCommitteeTransitionMessageInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  fromSyncPeriod?: string | number | bigint;
  from_sync_period?: string | number | bigint;
  toSyncPeriod?: string | number | bigint;
  to_sync_period?: string | number | bigint;
  transitionSlot?: string | number | bigint;
  transition_slot?: string | number | bigint;
  finalizedBeaconRoot?: string;
  finalized_beacon_root?: string;
  parentSyncCommitteeHash?: string;
  parent_sync_committee_hash?: string;
  nextSyncCommitteeHash?: string;
  next_sync_committee_hash?: string;
  nextSyncCommitteePayload?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  next_sync_committee_payload?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  nextSyncCommitteePayloadHash?: string;
  next_sync_committee_payload_hash?: string;
  nextSyncCommitteeBranchHash?: string;
  next_sync_committee_branch_hash?: string;
}

export interface EthBeaconSyncCommitteeProofInput
  extends EthSyncCommitteePayloadInput {
  version?: SccpVersionInput;
  totalWeight?: string | number | bigint;
  total_weight?: string | number | bigint;
  signedWeight?: string | number | bigint;
  signed_weight?: string | number | bigint;
  syncCommitteeMessageHash?: string;
  sync_committee_message_hash?: string;
  signersBitmap?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  signers_bitmap?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  aggregateSignature?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  aggregate_signature?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
}

export interface EthSyncCommitteeTransitionSignatureInput
  extends EthSyncCommitteeTransitionMessageInput {
  version?: SccpVersionInput;
  transitionMessageHash?: string;
  transition_message_hash?: string;
  syncCommitteeProof?: EthBeaconSyncCommitteeProofInput;
  sync_committee_proof?: EthBeaconSyncCommitteeProofInput;
}

export interface BscSccpReceiptProofInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  sourceEventDigest?: string;
  source_event_digest?: string;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  blockNumber?: string | number | bigint;
  block_number?: string | number | bigint;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  blockHash?: string;
  block_hash?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  receiptsRoot?: string;
  receipts_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  validatorSetHash?: string;
  validator_set_hash?: string;
  commitSealHash?: string;
  commit_seal_hash?: string;
  receiptRootIndex?: string | number | bigint;
  receipt_root_index?: string | number | bigint;
  receiptTrieProofNodes?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  receipt_trie_proof_nodes?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface BscValidatorSetPayloadInput {
  validatorAddresses?: readonly string[];
  validator_addresses?: readonly string[];
  validatorPowers?: readonly (string | number | bigint)[];
  validator_powers?: readonly (string | number | bigint)[];
}

export interface BscValidatorStorageProofInput {
  version?: SccpVersionInput;
  validatorIndex?: number;
  validator_index?: number;
  storageSlot?: string;
  storage_slot?: string;
  storageValue?: BinaryLike;
  storage_value?: BinaryLike;
  storageValueHash?: string;
  storage_value_hash?: string;
  storageProofNodes?: readonly BinaryLike[];
  storage_proof_nodes?: readonly BinaryLike[];
}

export interface BscValidatorSetMetadataProofInput {
  version?: SccpVersionInput;
  stateRoot?: string;
  state_root?: string;
  nextValidatorSetPayloadHash?: string;
  next_validator_set_payload_hash?: string;
  validatorContractAddress?: BinaryLike;
  validator_contract_address?: BinaryLike;
  accountProofNodes?: readonly BinaryLike[];
  account_proof_nodes?: readonly BinaryLike[];
  storageRoot?: string;
  storage_root?: string;
  validatorSetLengthSlot?: string;
  validator_set_length_slot?: string;
  validatorSetLengthValue?: BinaryLike;
  validator_set_length_value?: BinaryLike;
  validatorSetLengthValueHash?: string;
  validator_set_length_value_hash?: string;
  validatorSetLengthProofNodes?: readonly BinaryLike[];
  validator_set_length_proof_nodes?: readonly BinaryLike[];
  validatorStorageProofs?: readonly BscValidatorStorageProofInput[];
  validator_storage_proofs?: readonly BscValidatorStorageProofInput[];
}

export interface BscValidatorSetTransitionMessageInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  fromValidatorEpoch?: string | number | bigint;
  from_validator_epoch?: string | number | bigint;
  toValidatorEpoch?: string | number | bigint;
  to_validator_epoch?: string | number | bigint;
  transitionBlockNumber?: string | number | bigint;
  transition_block_number?: string | number | bigint;
  transitionBlockHash?: string;
  transition_block_hash?: string;
  parentValidatorSetHash?: string;
  parent_validator_set_hash?: string;
  nextValidatorSetHash?: string;
  next_validator_set_hash?: string;
  nextValidatorSetPayloadHash?: string;
  next_validator_set_payload_hash?: string;
  validatorSetMetadataProofHash?: string;
  validator_set_metadata_proof_hash?: string;
}

export interface BscCommitMessageInput {
  version?: SccpVersionInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  blockNumber?: string | number | bigint;
  block_number?: string | number | bigint;
  blockHash?: string;
  block_hash?: string;
  receiptsRoot?: string;
  receipts_root?: string;
  validatorSetHash?: string;
  validator_set_hash?: string;
}

export interface BscCommitSealInput {
  version?: SccpVersionInput;
  totalPower?: string | number | bigint;
  total_power?: string | number | bigint;
  signedPower?: string | number | bigint;
  signed_power?: string | number | bigint;
  commitMessageHash?: string;
  commit_message_hash?: string;
  validatorPublicKeys?: ReadonlyArray<BinaryLike>;
  validator_public_keys?: ReadonlyArray<BinaryLike>;
  validatorPowers?: readonly (string | number | bigint)[];
  validator_powers?: readonly (string | number | bigint)[];
  validatorSetHash?: string;
  validator_set_hash?: string;
  signersBitmap?: BinaryLike;
  signers_bitmap?: BinaryLike;
  signatures?: ReadonlyArray<BinaryLike>;
}

export interface SubstrateAuthoritySetPayloadInput {
  authorityPublicKeys?: readonly string[];
  authority_public_keys?: readonly string[];
  authorityWeights?: readonly (string | number | bigint)[];
  authority_weights?: readonly (string | number | bigint)[];
}

export interface SubstrateGrandpaJustificationProofInput
  extends SubstrateAuthoritySetPayloadInput {
  version?: SccpVersionInput;
  totalWeight?: string | number | bigint;
  total_weight?: string | number | bigint;
  signedWeight?: string | number | bigint;
  signed_weight?: string | number | bigint;
  precommitMessageHash?: string;
  precommit_message_hash?: string;
  signersBitmap?: BinaryLike;
  signers_bitmap?: BinaryLike;
  signatures?: readonly BinaryLike[];
}

export interface SubstrateAuthoritySetTransitionMessageInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  fromGrandpaSetId?: string | number | bigint;
  from_grandpa_set_id?: string | number | bigint;
  toGrandpaSetId?: string | number | bigint;
  to_grandpa_set_id?: string | number | bigint;
  transitionBlockNumber?: string | number | bigint;
  transition_block_number?: string | number | bigint;
  transitionBlockHash?: string;
  transition_block_hash?: string;
  parentAuthoritySetHash?: string;
  parent_authority_set_hash?: string;
  nextAuthoritySetHash?: string;
  next_authority_set_hash?: string;
  nextAuthoritySetPayload?: BinaryLike | SubstrateAuthoritySetPayloadInput;
  next_authority_set_payload?: BinaryLike | SubstrateAuthoritySetPayloadInput;
  nextAuthoritySetPayloadHash?: string;
  next_authority_set_payload_hash?: string;
  nextAuthoritySetProofHash?: string;
  next_authority_set_proof_hash?: string;
}

export interface SubstrateAuthoritySetTransitionJustificationInput
  extends SubstrateAuthoritySetTransitionMessageInput {
  version?: SccpVersionInput;
  transitionMessageHash?: string;
  transition_message_hash?: string;
  grandpaJustification?: SubstrateGrandpaJustificationProofInput;
  grandpa_justification?: SubstrateGrandpaJustificationProofInput;
}

export interface TronSccpReceiptProofInput {
  sourceEventDigest?: string;
  source_event_digest?: string;
  receiptRoot?: string;
  receipt_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  transactionRoot?: string;
  transaction_root?: string;
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TronSccpReceiptStateProofInput
  extends TronSccpReceiptProofInput {
  receiptRootIndex?: string | number | bigint;
  receipt_root_index?: string | number | bigint;
  receiptTrieProofNodes?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  receipt_trie_proof_nodes?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TronSccpTransactionSourceProofInput
  extends TronSccpReceiptProofInput {
  transactionIndex?: string | number | bigint;
  transaction_index?: string | number | bigint;
  transactionCount?: string | number | bigint;
  transaction_count?: string | number | bigint;
  transactionBytes?: BinaryLike;
  transaction_bytes?: BinaryLike;
  transactionMerkleBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  transaction_merkle_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TronRawBlockHeaderInput {
  number?: string | number | bigint;
  blockNumber?: string | number | bigint;
  block_number?: string | number | bigint;
  txTrieRoot?: string;
  tx_trie_root?: string;
  accountStateRoot?: string;
  account_state_root?: string;
  parentBlockId?: string;
  parent_block_id?: string;
  witnessAddress?: string;
  witness_address?: string;
  headerVersion?: string | number | bigint;
  header_version?: string | number | bigint;
  timestampMs?: string | number | bigint;
  timestamp_ms?: string | number | bigint;
}

export interface TronSolidBlockHeaderProofInput {
  version?: SccpVersionInput;
  rawData?: string | Uint8Array | ArrayBuffer | ArrayBufferView | number[];
  raw_data?: string | Uint8Array | ArrayBuffer | ArrayBufferView | number[];
  witnessSignature?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  witness_signature?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  parentRawData?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  parent_raw_data?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  parentWitnessSignature?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  parent_witness_signature?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  rawDataHash?: string;
  raw_data_hash?: string;
  parentRawDataHash?: string;
  parent_raw_data_hash?: string;
  blockId?: string;
  block_id?: string;
  txTrieRoot?: string;
  tx_trie_root?: string;
  accountStateRoot?: string;
  account_state_root?: string;
  parentBlockId?: string;
  parent_block_id?: string;
  witnessAddress?: string;
  witness_address?: string;
  timestampMs?: string | number | bigint;
  timestamp_ms?: string | number | bigint;
  headerVersion?: string | number | bigint;
  header_version?: string | number | bigint;
}

export interface TronWitnessSchedulePayloadInput {
  witnessAddresses?: readonly string[];
  witness_addresses?: readonly string[];
  witnessWeights?: readonly (string | number | bigint)[];
  witness_weights?: readonly (string | number | bigint)[];
}

export interface TronSolidBlockMessageInput {
  version?: SccpVersionInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  solidBlockNumber?: string | number | bigint;
  solid_block_number?: string | number | bigint;
  blockHash?: string;
  block_hash?: string;
  witnessScheduleHash?: string;
  witness_schedule_hash?: string;
  receiptRoot?: string;
  receipt_root?: string;
  transactionRoot?: string;
  transaction_root?: string;
  receiptProofHash?: string;
  receipt_proof_hash?: string;
}

export interface TronWitnessSealInput {
  version?: SccpVersionInput;
  totalWeight?: string | number | bigint;
  total_weight?: string | number | bigint;
  signedWeight?: string | number | bigint;
  signed_weight?: string | number | bigint;
  solidBlockMessageHash?: string;
  solid_block_message_hash?: string;
  witnessAddresses?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  witness_addresses?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  witnessWeights?: readonly (string | number | bigint)[];
  witness_weights?: readonly (string | number | bigint)[];
  signersBitmap?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  signers_bitmap?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  signatures?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface TronWitnessScheduleTransitionMessageInput {
  version?: SccpVersionInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  fromWitnessScheduleEpoch?: string | number | bigint;
  from_witness_schedule_epoch?: string | number | bigint;
  toWitnessScheduleEpoch?: string | number | bigint;
  to_witness_schedule_epoch?: string | number | bigint;
  transitionBlockNumber?: string | number | bigint;
  transition_block_number?: string | number | bigint;
  transitionBlockHash?: string;
  transition_block_hash?: string;
  parentWitnessScheduleHash?: string;
  parent_witness_schedule_hash?: string;
  nextWitnessScheduleHash?: string;
  next_witness_schedule_hash?: string;
  nextWitnessSchedulePayload?:
    | TronWitnessSchedulePayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  next_witness_schedule_payload?:
    | TronWitnessSchedulePayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  nextWitnessSchedulePayloadHash?: string;
  next_witness_schedule_payload_hash?: string;
}

export interface TronWitnessScheduleTransitionSealInput
  extends TronWitnessScheduleTransitionMessageInput {
  transitionMessageHash?: string;
  transition_message_hash?: string;
  sealProof?: TronWitnessSealInput;
  seal_proof?: TronWitnessSealInput;
  witnessSealProof?: TronWitnessSealInput;
  witness_seal_proof?: TronWitnessSealInput;
}

export interface SubstrateSccpStorageProofInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  sourceEventDigest?: string;
  source_event_digest?: string;
  sourceEventLeafIndex?: string | number | bigint;
  source_event_leaf_index?: string | number | bigint;
  leafIndex?: string | number | bigint;
  leaf_index?: string | number | bigint;
  finalizedBlockNumber?: string | number | bigint;
  finalized_block_number?: string | number | bigint;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  grandpaSetId?: string | number | bigint;
  grandpa_set_id?: string | number | bigint;
  blockHash?: string;
  block_hash?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  authoritySetHash?: string;
  authority_set_hash?: string;
  eventsRoot?: string;
  events_root?: string;
  receiptOrMessageRoot?: string;
  receipt_or_message_root?: string;
  inclusionBranch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
  inclusion_branch?: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[];
}

export interface SubstrateSccpRuntimeStorageProofRequestInput
  extends SubstrateSccpStorageProofInput,
    SccpSourceVerifierMaterialInput {
  storageProofHash?: string;
  storage_proof_hash?: string;
  sourceVerifierMaterial?: SccpSourceVerifierMaterialInput;
  source_verifier_material?: SccpSourceVerifierMaterialInput;
  material?: SccpSourceVerifierMaterialInput;
}

export interface SubstrateSccpRuntimeStorageFastpqPublicInputs {
  readonly dsid: string;
  readonly slot: string;
  readonly oldRoot: string;
  readonly newRoot: string;
  readonly permRoot: string;
  readonly txSetHash: string;
}

export interface SubstrateSccpRuntimeStorageFastpqTransition {
  readonly key: string;
  readonly operation: "meta_set";
  readonly oldValue: "0x";
  readonly newValue: string;
}

export interface SubstrateSccpRuntimeStorageProofRequest {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly circuitId: typeof SCCP_SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1;
  readonly parameterSet: "fastpq-lane-balanced";
  readonly sourceDomain: number;
  readonly finalizedBlockNumber: string;
  readonly grandpaSetId: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly runtimeStorageProofPublicInputsHash: string;
  readonly storageProofHash: string;
  readonly statementBytes: Uint8Array;
  readonly verificationContextBytes: Uint8Array;
  readonly schemaDescriptor: Uint8Array;
  readonly publicInputColumns: ReadonlyArray<ReadonlyArray<string>>;
  readonly fastpqPublicInputs: Readonly<SubstrateSccpRuntimeStorageFastpqPublicInputs>;
  readonly fastpqTransitions: ReadonlyArray<
    Readonly<SubstrateSccpRuntimeStorageFastpqTransition>
  >;
}

export interface SolanaSccpProofPublicInputs {
  readonly messageId: string;
  readonly payloadHash: string;
  readonly commitmentRoot: string;
  readonly finalizedSlot: string;
  readonly parentSlot: string;
  readonly bankSignatureCount: string;
  readonly parentBankHash: string;
  readonly blockhash: string;
  readonly bankHash: string;
  readonly transactionStatusRoot: string;
  readonly messageProofHash: string;
  readonly accountInclusionRoot: string;
  readonly accountsLtHashChecksum: string;
  readonly accountsLtHashProofPublicInputsHash: string;
  readonly sourceEventDigest: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly sourceAdapterDeploymentHash: string;
  readonly sourceAdapterDeploymentReceiptHash: string;
  readonly sourceAdapterDeploymentBindingHash: string;
}

export interface SolanaSccpProofPublicInputsSnakeCase {
  readonly message_id: string;
  readonly payload_hash: string;
  readonly commitment_root: string;
  readonly finalized_slot: string | number | bigint;
  readonly parent_slot: string | number | bigint;
  readonly bank_signature_count: string | number | bigint;
  readonly parent_bank_hash: string;
  readonly blockhash: string;
  readonly bank_hash: string;
  readonly transaction_status_root: string;
  readonly message_proof_hash: string;
  readonly account_inclusion_root: string;
  readonly accounts_lt_hash_checksum: string;
  readonly accounts_lt_hash_proof_public_inputs_hash: string;
  readonly source_event_digest: string;
  readonly source_state_verifier_id: string;
  readonly source_state_verifier_hash: string;
  readonly statement_hash: string;
  readonly destination_binding_hash: string;
  readonly source_adapter_deployment_hash: string;
  readonly source_adapter_deployment_receipt_hash: string;
  readonly source_adapter_deployment_binding_hash: string;
}

export type SolanaSccpProofPublicInputsInput =
  | SolanaSccpProofPublicInputs
  | SolanaSccpProofPublicInputsSnakeCase;

export interface SolanaSccpRouteCanaryEvidenceInput {
  routeAllowlistHash?: string;
  route_allowlist_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  expectedDestinationBindingHash?: string;
  expected_destination_binding_hash?: string;
  sourceVerifierMaterialHash?: string;
  source_verifier_material_hash?: string;
  sourceAdapterEngineDeploymentHash?: string;
  source_adapter_engine_deployment_hash?: string;
  verifierIdentity?: string;
  verifier_identity?: string;
  verifierProgramId?: string;
  verifier_program_id?: string;
  verifierCodeHash?: string;
  verifier_code_hash?: string;
  solanaRpcCommitment?: "finalized";
  solana_rpc_commitment?: "finalized";
  solanaProgramOwner?: string;
  solana_program_owner?: string;
  solanaProgramdataOwner?: string;
  solana_programdata_owner?: string;
  solanaProgramImmutable?: boolean;
  solana_program_immutable?: boolean;
  solanaProgramAccountDataBase64?: string;
  solana_program_account_data_base64?: string;
  solanaProgramdataAddress?: string;
  solana_programdata_address?: string;
  solanaProgramdataSlot?: string | number | bigint;
  solana_programdata_slot?: string | number | bigint;
  solanaExpectedProgramdataSlot?: string | number | bigint;
  solana_expected_programdata_slot?: string | number | bigint;
  solanaProgramAccountContextSlot?: string | number | bigint;
  solana_program_account_context_slot?: string | number | bigint;
  solanaProgramdataAccountContextSlot?: string | number | bigint;
  solana_programdata_account_context_slot?: string | number | bigint;
  solanaProgramdataMetadataBlake2b256?: string;
  solana_programdata_metadata_blake2b256?: string;
  solanaProgramdataMetadataBase64?: string;
  solana_programdata_metadata_base64?: string;
  solanaProgramdataExecutableBlake2b256?: string;
  solana_programdata_executable_blake2b256?: string;
  solanaProgramdataExecutableBase64?: string;
  solana_programdata_executable_base64?: string;
}

export interface TonSccpRouteCanaryEvidenceInput {
  routeAllowlistHash?: string;
  route_allowlist_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  expectedDestinationBindingHash?: string;
  expected_destination_binding_hash?: string;
  sourceVerifierMaterialHash?: string;
  source_verifier_material_hash?: string;
  sourceAdapterEngineDeploymentHash?: string;
  source_adapter_engine_deployment_hash?: string;
  verifierContractAddress?: string;
  verifier_contract_address?: string;
  verifierIdentity?: string;
  verifier_identity?: string;
  verifierCodeHash?: string;
  verifier_code_hash?: string;
  accountStatus?: "active";
  account_status?: "active";
  tonAccountStatus?: "active";
  ton_account_status?: "active";
  accountStateHash?: string;
  account_state_hash?: string;
  tonAccountStateHash?: string;
  ton_account_state_hash?: string;
  lastTransactionLt?: string;
  last_transaction_lt?: string;
  tonLastTransactionLt?: string;
  ton_last_transaction_lt?: string;
  lastTransactionHash?: string;
  last_transaction_hash?: string;
  tonLastTransactionHash?: string;
  ton_last_transaction_hash?: string;
  verifierCodeBocRootHash?: string;
  verifier_code_boc_root_hash?: string;
  tonVerifierCodeBocRootHash?: string;
  ton_verifier_code_boc_root_hash?: string;
}

export interface TronSccpRouteCanaryEvidenceInput {
  routeAllowlistHash?: string;
  route_allowlist_hash?: string;
  expectedRouteAllowlistHash?: string;
  expected_route_allowlist_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  expectedDestinationBindingHash?: string;
  expected_destination_binding_hash?: string;
  sourceVerifierMaterialHash?: string;
  source_verifier_material_hash?: string;
  sourceAdapterEngineDeploymentHash?: string;
  source_adapter_engine_deployment_hash?: string;
  networkId?: string;
  network_id?: string;
  networkIdHex?: string;
  network_id_hex?: string;
  verifierAddress?: string;
  verifier_address?: string;
  verifierCodeHash?: string;
  verifier_code_hash?: string;
  verifierCodeHashHex?: string;
  verifier_code_hash_hex?: string;
  verifierKeyHash?: string;
  verifier_key_hash?: string;
  verifierKeyHashHex?: string;
  verifier_key_hash_hex?: string;
  verifierBackend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  verifier_backend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  backend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  proofFamily?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  proof_family?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  destinationSourceDomain?: SccpDomainIdInput;
  destination_source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  destinationTargetDomain?: SccpDomainIdInput;
  destination_target_domain?: SccpDomainIdInput;
  routeCanaryTargetDomain?: SccpDomainIdInput;
  route_canary_target_domain?: SccpDomainIdInput;
  transactionId?: string;
  transaction_id?: string;
  routeCanaryTransactionId?: string;
  route_canary_transaction_id?: string;
  transactionOwnerAddress?: string;
  transaction_owner_address?: string;
  routeCanaryTransactionOwnerAddress?: string;
  route_canary_transaction_owner_address?: string;
  blockNumber?: string | number | bigint;
  block_number?: string | number | bigint;
  routeCanaryBlockNumber?: string | number | bigint;
  route_canary_block_number?: string | number | bigint;
  blockTimestamp?: string | number | bigint;
  block_timestamp?: string | number | bigint;
  routeCanaryBlockTimestamp?: string | number | bigint;
  route_canary_block_timestamp?: string | number | bigint;
  logIndex?: SccpDomainIdInput;
  log_index?: SccpDomainIdInput;
  routeCanaryLogIndex?: SccpDomainIdInput;
  route_canary_log_index?: SccpDomainIdInput;
  messageId?: string;
  message_id?: string;
  routeCanaryMessageId?: string;
  route_canary_message_id?: string;
  callDataSha256?: string;
  call_data_sha256?: string;
  routeCanaryCallDataSha256?: string;
  route_canary_call_data_sha256?: string;
  payloadHash?: string;
  payload_hash?: string;
  routeCanaryPayloadHash?: string;
  route_canary_payload_hash?: string;
  commitmentRoot?: string;
  commitment_root?: string;
  routeCanaryCommitmentRoot?: string;
  route_canary_commitment_root?: string;
  finalityHeight?: string;
  finality_height?: string;
  routeCanaryFinalityHeight?: string;
  route_canary_finality_height?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  routeCanaryFinalityBlockHash?: string;
  route_canary_finality_block_hash?: string;
  statementHash?: string;
  statement_hash?: string;
  routeCanaryStatementHash?: string;
  route_canary_statement_hash?: string;
  proofVersion?: SccpVersionInput;
  proof_version?: SccpVersionInput;
  routeCanaryProofVersion?: SccpVersionInput;
  route_canary_proof_version?: SccpVersionInput;
  proofSourceDomain?: SccpDomainIdInput;
  proof_source_domain?: SccpDomainIdInput;
  routeCanaryProofSourceDomain?: SccpDomainIdInput;
  route_canary_proof_source_domain?: SccpDomainIdInput;
  usedMessageProof?: boolean;
  used_message_proof?: boolean;
  routeCanaryUsedMessageProof?: boolean;
  route_canary_used_message_proof?: boolean;
  rawDataOwnerMatchesTransaction?: boolean;
  raw_data_owner_matches_transaction?: boolean;
  routeCanaryRawDataOwnerMatchesTransaction?: boolean;
  route_canary_raw_data_owner_matches_transaction?: boolean;
  signatureSha256?: string;
  signature_sha256?: string;
  routeCanarySignatureSha256?: string;
  route_canary_signature_sha256?: string;
  signatureRecoveredAddress?: string;
  signature_recovered_address?: string;
  routeCanarySignatureRecoveredAddress?: string;
  route_canary_signature_recovered_address?: string;
  signatureRecoversToOwner?: boolean;
  signature_recovers_to_owner?: boolean;
  routeCanarySignatureRecoversToOwner?: boolean;
  route_canary_signature_recovers_to_owner?: boolean;
  routeCanaryEvidenceHash?: string;
  route_canary_evidence_hash?: string;
  expectedRouteCanaryEvidenceHash?: string;
  expected_route_canary_evidence_hash?: string;
}

export interface SolanaSccpProofRequest {
  readonly version: 1;
  readonly backend: typeof SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1;
  readonly sourceDomain: typeof SCCP_DOMAIN_SOL;
  readonly targetDomain: number;
  readonly mainnetGenesisHash: string;
  readonly witnessHash: string;
  readonly proofContextHash: string;
  readonly sourceAdapterDeploymentBindingHash: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly publicInputs: Readonly<SolanaSccpProofPublicInputs>;
  readonly witness: Readonly<SolanaSccpWitness>;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly sourceAdapterDeploymentBinding: Readonly<SccpSourceAdapterDeploymentBinding>;
}

export interface SolanaSccpProofResult {
  readonly version: 1;
  readonly backend: typeof SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
  readonly publicInputs: Readonly<SolanaSccpProofPublicInputs>;
  readonly witnessHash: string;
  readonly proofContextHash: string;
  readonly sourceAdapterDeploymentBindingHash: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly sourceAdapterDeploymentBinding: Readonly<SccpSourceAdapterDeploymentBinding>;
  readonly envelopeHash: string;
}

export interface SolanaSccpSubmissionInputBase {
  /**
   * Transparent SCCP message public inputs are required. Solana proof-result
   * publicInputs are source-proof inputs and are not accepted as a substitute.
   */
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  transparentPublicInputs?: SccpMessageTransparentPublicInputsInput;
  transparent_public_inputs?: SccpMessageTransparentPublicInputsInput;
  publicInputsBytes?: BinaryLike;
  public_inputs_bytes?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  proofContextHash?: string;
  proof_context_hash?: string;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
}

export type SolanaSccpSubmissionInput = SolanaSccpSubmissionInputBase &
  (
    | {
        proofResult: SolanaSccpProofResult;
        proof_result?: never;
      }
    | {
        proofResult?: never;
        proof_result: SolanaSccpProofResult;
      }
  );

export interface SolanaSccpSubmissionInputWithProofResult
  extends SolanaSccpSubmissionInputBase {
  /**
   * Wrapped result returned by wrapSolanaSccpProofResult or SolanaSccpProver.prove.
   * Its source public inputs, proof bytes, and recomputed envelope hash must
   * match the transparent publicInputs and proof bytes submitted on-chain.
   */
  proofResult: SolanaSccpProofResult;
  proof_result?: never;
}

export interface SolanaSccpSubmission {
  readonly version: 1;
  readonly envelopeEncoding: typeof SCCP_SOLANA_BORSH_INSTRUCTION_V1;
  readonly submissionKind: "program_instruction";
  readonly verifierEntrypoint: "submit_sccp_message_proof";
  readonly proofBytes: Uint8Array;
  readonly publicInputs: Readonly<
    Required<{
      version: number;
      messageId: string;
      payloadHash: string;
      targetDomain: number;
      commitmentRoot: string;
      finalityHeight: string;
      finalityBlockHash: string;
    }>
  >;
  readonly publicInputsBytes: Uint8Array;
  readonly bundleBytes: Uint8Array;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly proofContextHash: string;
  readonly arguments: ReadonlyArray<
    | { key: "proof_bytes"; encoding: "raw_bytes"; bytes: string }
    | { key: "public_inputs"; encoding: "raw_bytes"; bytes: string }
    | { key: "bundle_bytes"; encoding: "raw_bytes"; bytes: string }
    | { key: "statement_hash"; encoding: "raw_bytes"; bytes: string }
    | { key: "destination_binding_hash"; encoding: "raw_bytes"; bytes: string }
    | { key: "proof_context_hash"; encoding: "raw_bytes"; bytes: string }
  >;
  readonly instructionData: Uint8Array;
  readonly instructionDataHex: string;
  readonly envelopeBytes: Uint8Array;
  readonly envelopeHex: string;
}

export interface SccpMessageTransparentPublicInputsInput {
  version?: SccpVersionInput;
  messageId?: string;
  message_id?: string;
  payloadHash?: string;
  payload_hash?: string;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  commitmentRoot?: string;
  commitment_root?: string;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  finalityBlockHash?: string;
  finality_block_hash?: string;
}

export interface SccpGroth16Bn254PublicSignalsInput {
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
}

export interface TonSccpDestinationBindingInput {
  key: string;
  bindingHash?: string;
  binding_hash?: string;
}

export interface TonSccpManifestInput {
  version: SccpVersionInput;
  localDomain?: SccpDomainIdInput;
  local_domain?: SccpDomainIdInput;
  counterpartyDomain?: SccpDomainIdInput;
  counterparty_domain?: SccpDomainIdInput;
  securityModel?: "RecursiveZk";
  security_model?: "RecursiveZk";
  anchorGovernance?: "CryptographicProof";
  anchor_governance?: "CryptographicProof";
  verifierTarget?: "TonContract";
  verifier_target?: "TonContract";
  verifierBackendFamily?: "TonContract";
  verifier_backend_family?: "TonContract";
  proofFamily?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  proof_family?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  verifierBackendKey?: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
  verifier_backend_key?: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
  verifierBackend?: {
    key: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
    family?: "TonContract";
  };
  verifier_backend?: {
    key: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
    family?: "TonContract";
  };
  messageBackend?: string;
  message_backend?: string;
  registryBackend?: string;
  registry_backend?: string;
  manifestSeed?: string;
  manifest_seed?: string;
  destinationBinding?: TonSccpDestinationBindingInput;
  destination_binding?: TonSccpDestinationBindingInput;
}

export interface TonSccpProofRequestInput {
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  sourceProofBytes?: BinaryLike;
  source_proof_bytes?: BinaryLike;
  backend?: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  destinationBinding?: TonSccpDestinationBindingInput;
  destination_binding?: TonSccpDestinationBindingInput;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
  sourceStateVerifierId?: string;
  source_state_verifier_id?: string;
  sourceStateVerifierHash?: string;
  source_state_verifier_hash?: string;
  sourceAdapterDeploymentHash?: string;
  source_adapter_deployment_hash?: string;
  sourceAdapterDeploymentReceiptHash?: string;
  source_adapter_deployment_receipt_hash?: string;
  sourceAdapterDeploymentBinding?: SccpSourceAdapterDeploymentBindingInput;
  source_adapter_deployment_binding?: SccpSourceAdapterDeploymentBindingInput;
}

export interface TonSccpMessageBodyInputBase extends TonSccpProofRequestInput {
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  metadataBytes?: BinaryLike;
  metadata_bytes?: BinaryLike;
  manifest?: TonSccpManifestInput;
  queryId?: string | number | bigint;
  query_id?: string | number | bigint;
}

export type TonSccpMessageBodyInput = TonSccpMessageBodyInputBase &
  (
    | {
        proofResult: TonSccpProofResult;
        proof_result?: TonSccpProofResult;
      }
    | {
        proofResult?: TonSccpProofResult;
        proof_result: TonSccpProofResult;
      }
  );

export interface TonSccpMessageBodyInputWithProofResult
  extends TonSccpMessageBodyInputBase {
  /**
   * Wrapped result returned by wrapTonSccpProofResult or TonSccpProver.prove.
   * TON native-recursive proof bytes are only accepted when they match this
   * wrapped result and its request, envelope, verifier, and deployment binding.
   */
  proofResult: TonSccpProofResult;
  proof_result?: TonSccpProofResult;
}

export interface TonSccpProofRequest {
  readonly version: 1;
  readonly backend: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
  readonly sourceDomain: typeof SCCP_DOMAIN_TON;
  readonly targetDomain: number;
  readonly publicInputs: Readonly<
    Required<{
      version: number;
      messageId: string;
      payloadHash: string;
      targetDomain: number;
      commitmentRoot: string;
      finalityHeight: string;
      finalityBlockHash: string;
    }>
  >;
  readonly publicInputsBytes: Uint8Array;
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly sourceAdapterDeploymentBindingHash: string;
  readonly sourceAdapterDeploymentBinding: Readonly<SccpSourceAdapterDeploymentBinding>;
  readonly requestHash: string;
}

export interface TonSccpSubmission {
  readonly version: 1;
  readonly envelopeEncoding: typeof SCCP_TON_MESSAGE_BODY_BOC_V1;
  readonly submissionKind: "internal_message";
  readonly verifierEntrypoint: "op::submit_sccp_message_proof";
  readonly messageBodyBoc: Uint8Array;
  readonly messageBodyBocHex: string;
  readonly arguments: ReadonlyArray<
    Readonly<{ key: "message_body_boc"; encoding: "ton_boc"; bytes: string }>
  >;
  readonly envelopeBytes: Uint8Array;
  readonly envelopeHex: string;
}

export interface TonSccpProofResult {
  readonly version: 1;
  readonly backend: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
  readonly publicInputs: TonSccpProofRequest["publicInputs"];
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly sourceStateVerifierId: string;
  readonly sourceStateVerifierHash: string;
  readonly sourceAdapterDeploymentBindingHash: string;
  readonly sourceAdapterDeploymentBinding: Readonly<SccpSourceAdapterDeploymentBinding>;
  readonly requestHash: string;
  readonly envelopeHash: string;
}

export interface EthereumMainnetNativeEvmProverBundleSdkArtifactInput {
  sdk?: string;
  implementation?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
  implementationArtifact?: string;
  implementation_artifact?: string;
  implementationPath?: string;
  implementation_path?: string;
  implementationHash?: string;
  implementation_hash?: string;
}

export interface EthereumMainnetNativeEvmProverAuditHashesInput {
  circuit_security_audit: string;
  native_implementation_audit: string;
  reproducible_build_attestation: string;
  cross_sdk_fixture_parity: string;
  native_prover_self_test: string;
  no_wasm_no_remote_scan: string;
}

export interface EthereumMainnetNativeEvmProverAuditHashes {
  readonly circuit_security_audit: string;
  readonly native_implementation_audit: string;
  readonly reproducible_build_attestation: string;
  readonly cross_sdk_fixture_parity: string;
  readonly native_prover_self_test: string;
  readonly no_wasm_no_remote_scan: string;
}

export interface EthereumMainnetNativeEvmProverBundleInput {
  schema?: typeof SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1;
  bundleId?: typeof SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1;
  bundle_id?: typeof SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1;
  domain?: SccpDomainIdInput;
  chain?: "eth";
  proofBackend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proof_backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  circuitArtifactHash?: string;
  circuit_artifact_hash?: string;
  proofArtifact?: string;
  proof_artifact?: string;
  proverArtifact?: string;
  prover_artifact?: string;
  circuitArtifact?: string;
  circuit_artifact?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
  provingKey?: string;
  proving_key?: string;
  verifierKeyHash?: string;
  verifier_key_hash?: string;
  verifierKey?: string;
  verifier_key?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  noWasm?: true;
  no_wasm?: true;
  remoteProverRequired?: false;
  remote_prover_required?: false;
  browserImplementation?: "pure-typescript";
  browser_implementation?: "pure-typescript";
  nativeSdkArtifacts?: readonly EthereumMainnetNativeEvmProverBundleSdkArtifactInput[];
  native_sdk_artifacts?: readonly EthereumMainnetNativeEvmProverBundleSdkArtifactInput[];
  sdkArtifacts?: readonly EthereumMainnetNativeEvmProverBundleSdkArtifactInput[];
  sdk_artifacts?: readonly EthereumMainnetNativeEvmProverBundleSdkArtifactInput[];
  crossSdkFixtureParityArtifact?: string;
  cross_sdk_fixture_parity_artifact?: string;
  nativeProverSelfTestArtifact?: string;
  native_prover_self_test_artifact?: string;
  selfTestArtifact?: string;
  self_test_artifact?: string;
  auditHashes?: EthereumMainnetNativeEvmProverAuditHashesInput;
  audit_hashes?: EthereumMainnetNativeEvmProverAuditHashesInput;
}

export interface EthereumMainnetNativeEvmProverBundleSdkArtifact {
  readonly sdk: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  readonly implementation:
    | "native-csharp"
    | "native-java"
    | "native-kotlin"
    | "native-swift"
    | "pure-typescript";
  readonly proofArtifactHash: string;
  readonly proofArtifact: string;
  readonly provingKeyHash: string;
  readonly provingKey: string;
  readonly implementationArtifact: string;
  readonly implementationHash: string;
}

export interface EthereumMainnetNativeEvmProverBundle {
  readonly schema: typeof SCCP_NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1;
  readonly bundleId: typeof SCCP_ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1;
  readonly domain: number;
  readonly chain: "eth";
  readonly proofBackend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofArtifactHash: string;
  readonly proofArtifact: string;
  readonly provingKeyHash: string;
  readonly provingKey: string;
  readonly verifierKeyHash: string;
  readonly verifierKey: string;
  readonly destinationBindingHash: string;
  readonly noWasm: true;
  readonly remoteProverRequired: false;
  readonly browserImplementation: "pure-typescript";
  readonly nativeSdkArtifacts: readonly Readonly<EthereumMainnetNativeEvmProverBundleSdkArtifact>[];
  readonly crossSdkFixtureParityArtifact: string;
  readonly nativeProverSelfTestArtifact: string;
  readonly auditHashes: Readonly<EthereumMainnetNativeEvmProverAuditHashes>;
}

export function validateEthereumMainnetNativeEvmProverBundle(
  manifest: EthereumMainnetNativeEvmProverBundleInput,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): EthereumMainnetNativeEvmProverBundle;

export function parseEthereumMainnetNativeEvmProverBundleManifest(
  json: string,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): EthereumMainnetNativeEvmProverBundle;

export interface EthereumMainnetNativeEvmProverParitySdkResultInput {
  receiptProofHash?: string;
  receipt_proof_hash?: string;
  sourceProofHash?: string;
  source_proof_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
  calldataHash?: string;
  calldata_hash?: string;
  toriiSubmitPayloadHash?: string;
  torii_submit_payload_hash?: string;
}

export interface EthereumMainnetNativeEvmProverParityFixtureInput {
  schema?: typeof SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1;
  domain?: SccpDomainIdInput;
  chain?: "eth";
  proofBackend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proof_backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  circuitArtifactHash?: string;
  circuit_artifact_hash?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
  verifierKeyHash?: string;
  verifier_key_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  receiptProofHash?: string;
  receipt_proof_hash?: string;
  sourceProofHash?: string;
  source_proof_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
  calldataHash?: string;
  calldata_hash?: string;
  toriiSubmitPayloadHash?: string;
  torii_submit_payload_hash?: string;
  sdkResults?: Partial<
    Record<
      "dotnet" | "java-android" | "javascript" | "kotlin" | "swift",
      EthereumMainnetNativeEvmProverParitySdkResultInput
    >
  >;
  sdk_results?: Partial<
    Record<
      "dotnet" | "java-android" | "javascript" | "kotlin" | "swift",
      EthereumMainnetNativeEvmProverParitySdkResultInput
    >
  >;
}

export interface EthereumMainnetNativeEvmProverParitySdkResult {
  readonly receiptProofHash: string;
  readonly sourceProofHash: string;
  readonly destinationBindingHash: string;
  readonly publicSignalWords: readonly string[];
  readonly calldataHash: string;
  readonly toriiSubmitPayloadHash: string;
}

export interface EthereumMainnetNativeEvmProverParityFixture {
  readonly schema: typeof SCCP_ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1;
  readonly domain: number;
  readonly chain: "eth";
  readonly proofBackend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofArtifactHash: string;
  readonly provingKeyHash: string;
  readonly verifierKeyHash: string;
  readonly destinationBindingHash: string;
  readonly receiptProofHash: string;
  readonly sourceProofHash: string;
  readonly publicSignalWords: readonly string[];
  readonly calldataHash: string;
  readonly toriiSubmitPayloadHash: string;
  readonly sdkResults: Readonly<
    Record<
      "dotnet" | "java-android" | "javascript" | "kotlin" | "swift",
      Readonly<EthereumMainnetNativeEvmProverParitySdkResult>
    >
  >;
}

export function validateEthereumMainnetNativeEvmProverParityFixture(
  fixture: EthereumMainnetNativeEvmProverParityFixtureInput,
  nativeProverBundle: EthereumMainnetNativeEvmProverBundleInput,
): EthereumMainnetNativeEvmProverParityFixture;

export function parseEthereumMainnetNativeEvmProverParityFixture(
  json: string,
  nativeProverBundle: EthereumMainnetNativeEvmProverBundleInput,
): EthereumMainnetNativeEvmProverParityFixture;

export interface EthereumMainnetNativeEvmProverSelfTestSdkResultInput {
  requestHash?: string;
  request_hash?: string;
  witnessHash?: string;
  witness_hash?: string;
  sourceProofHash?: string;
  source_proof_hash?: string;
  proofHash?: string;
  proof_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
  calldataHash?: string;
  calldata_hash?: string;
  toriiSubmitPayloadHash?: string;
  torii_submit_payload_hash?: string;
}

export interface EthereumMainnetNativeEvmProverSelfTestFixtureInput {
  schema?: typeof SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1;
  domain?: SccpDomainIdInput;
  chain?: "eth";
  proofBackend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proof_backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  circuitArtifactHash?: string;
  circuit_artifact_hash?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
  verifierKeyHash?: string;
  verifier_key_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  requestHash?: string;
  request_hash?: string;
  witnessHash?: string;
  witness_hash?: string;
  sourceProofHash?: string;
  source_proof_hash?: string;
  proofHash?: string;
  proof_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
  calldataHash?: string;
  calldata_hash?: string;
  toriiSubmitPayloadHash?: string;
  torii_submit_payload_hash?: string;
  sdkResults?: Partial<
    Record<
      "dotnet" | "java-android" | "javascript" | "kotlin" | "swift",
      EthereumMainnetNativeEvmProverSelfTestSdkResultInput
    >
  >;
  sdk_results?: Partial<
    Record<
      "dotnet" | "java-android" | "javascript" | "kotlin" | "swift",
      EthereumMainnetNativeEvmProverSelfTestSdkResultInput
    >
  >;
}

export interface EthereumMainnetNativeEvmProverSelfTestSdkResult {
  readonly requestHash: string;
  readonly witnessHash: string;
  readonly sourceProofHash: string;
  readonly proofHash: string;
  readonly publicSignalWords: readonly string[];
  readonly calldataHash: string;
  readonly toriiSubmitPayloadHash: string;
}

export interface EthereumMainnetNativeEvmProverSelfTestFixture {
  readonly schema: typeof SCCP_ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1;
  readonly domain: number;
  readonly chain: "eth";
  readonly proofBackend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofArtifactHash: string;
  readonly provingKeyHash: string;
  readonly verifierKeyHash: string;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
  readonly witnessHash: string;
  readonly sourceProofHash: string;
  readonly proofHash: string;
  readonly publicSignalWords: readonly string[];
  readonly calldataHash: string;
  readonly toriiSubmitPayloadHash: string;
  readonly sdkResults: Readonly<
    Record<
      "dotnet" | "java-android" | "javascript" | "kotlin" | "swift",
      Readonly<EthereumMainnetNativeEvmProverSelfTestSdkResult>
    >
  >;
}

export function validateEthereumMainnetNativeEvmProverSelfTestFixture(
  fixture: EthereumMainnetNativeEvmProverSelfTestFixtureInput,
  nativeProverBundle: EthereumMainnetNativeEvmProverBundleInput,
): EthereumMainnetNativeEvmProverSelfTestFixture;

export function parseEthereumMainnetNativeEvmProverSelfTestFixture(
  json: string,
  nativeProverBundle: EthereumMainnetNativeEvmProverBundleInput,
): EthereumMainnetNativeEvmProverSelfTestFixture;

export interface EthereumMainnetNativeProverSelfTestContext {
  readonly sdk: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  readonly nativeProverArtifacts: Readonly<EthereumMainnetNativeEvmProverArtifacts>;
  readonly nativeProverSelfTest: Readonly<EthereumMainnetNativeEvmProverSelfTestFixture>;
  readonly expectedResult: Readonly<EthereumMainnetNativeEvmProverSelfTestSdkResult>;
}

export type EthereumMainnetNativeProverSelfTestFn = (
  context: Readonly<EthereumMainnetNativeProverSelfTestContext>,
  options?: Record<string, unknown>,
) =>
  | EthereumMainnetNativeEvmProverSelfTestSdkResultInput
  | Promise<EthereumMainnetNativeEvmProverSelfTestSdkResultInput>;

export interface EthereumMainnetNativeProverSelfTestRunInput {
  nativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
  native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
  verifiedNativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
  verified_native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
  destinationBinding?: EvmSccpDestinationBindingInput;
  destination_binding?: EvmSccpDestinationBindingInput;
  nativeProverSelfTest?: EthereumMainnetNativeProverSelfTestFn;
  native_prover_self_test?: EthereumMainnetNativeProverSelfTestFn;
  selfTestNativeProver?: EthereumMainnetNativeProverSelfTestFn;
  self_test_native_prover?: EthereumMainnetNativeProverSelfTestFn;
}

export function runEthereumMainnetNativeProverSelfTest(
  input: EthereumMainnetNativeProverSelfTestRunInput,
  options?: {
    nativeProverSelfTest?: EthereumMainnetNativeProverSelfTestFn;
    native_prover_self_test?: EthereumMainnetNativeProverSelfTestFn;
    selfTestNativeProver?: EthereumMainnetNativeProverSelfTestFn;
    self_test_native_prover?: EthereumMainnetNativeProverSelfTestFn;
  } & Record<string, unknown>,
): Promise<EthereumMainnetNativeEvmProverSelfTestSdkResult>;

export interface EthereumMainnetNativeEvmProverArtifactsInput {
  nativeProverBundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  native_prover_bundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  proverBundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  prover_bundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  manifest?: EthereumMainnetNativeEvmProverBundleInput | string;
  proofArtifactBytes?: BinaryLike;
  proof_artifact_bytes?: BinaryLike;
  proverArtifactBytes?: BinaryLike;
  prover_artifact_bytes?: BinaryLike;
  circuitArtifactBytes?: BinaryLike;
  circuit_artifact_bytes?: BinaryLike;
  provingKeyBytes?: BinaryLike;
  proving_key_bytes?: BinaryLike;
  verifierKeyBytes?: BinaryLike;
  verifier_key_bytes?: BinaryLike;
  crossSdkFixtureParityBytes?: BinaryLike;
  cross_sdk_fixture_parity_bytes?: BinaryLike;
  parityFixtureBytes?: BinaryLike;
  parity_fixture_bytes?: BinaryLike;
  nativeProverSelfTestBytes?: BinaryLike;
  native_prover_self_test_bytes?: BinaryLike;
  selfTestBytes?: BinaryLike;
  self_test_bytes?: BinaryLike;
  sdk: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  implementationBytes?: BinaryLike;
  implementation_bytes?: BinaryLike;
  nativeImplementationBytes?: BinaryLike;
  native_implementation_bytes?: BinaryLike;
}

export interface EthereumMainnetNativeEvmProverArtifacts {
  readonly hashAlgorithm: typeof SCCP_NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1;
  readonly nativeProverBundle: Readonly<EthereumMainnetNativeEvmProverBundle>;
  readonly proofArtifactHash: string;
  readonly provingKeyHash: string;
  readonly verifierKeyHash: string;
  readonly crossSdkFixtureParityHash: string;
  readonly crossSdkFixtureParity: Readonly<EthereumMainnetNativeEvmProverParityFixture>;
  readonly nativeProverSelfTestHash: string;
  readonly nativeProverSelfTest: Readonly<EthereumMainnetNativeEvmProverSelfTestFixture>;
  readonly sdk: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  readonly implementation:
    | "native-csharp"
    | "native-java"
    | "native-kotlin"
    | "native-swift"
    | "pure-typescript";
  readonly implementationHash: string;
}

export function verifyEthereumMainnetNativeEvmProverArtifacts(
  input: EthereumMainnetNativeEvmProverArtifactsInput,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): EthereumMainnetNativeEvmProverArtifacts;

export interface EthereumMainnetNativeEvmProverArtifactResolverMetadata {
  readonly path: string;
  readonly label:
    | "crossSdkFixtureParityBytes"
    | "implementationBytes"
    | "nativeProverSelfTestBytes"
    | "proofArtifactBytes"
    | "provingKeyBytes"
    | "verifierKeyBytes";
  readonly role:
    | "crossSdkFixtureParityArtifact"
    | "implementationArtifact"
    | "nativeProverSelfTestArtifact"
    | "proofArtifact"
    | "provingKey"
    | "verifierKey";
  readonly sdk: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  readonly implementation:
    | "native-csharp"
    | "native-java"
    | "native-kotlin"
    | "native-swift"
    | "pure-typescript";
  readonly nativeProverBundle: Readonly<EthereumMainnetNativeEvmProverBundle>;
}

export interface EthereumMainnetNativeEvmProverArtifactBundleInput {
  nativeProverBundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  native_prover_bundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  proverBundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  prover_bundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  manifest?: EthereumMainnetNativeEvmProverBundleInput | string;
  sdk: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  artifactResolver?: (
    path: string,
    metadata: EthereumMainnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  artifact_resolver?: (
    path: string,
    metadata: EthereumMainnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolveArtifact?: (
    path: string,
    metadata: EthereumMainnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolve_artifact?: (
    path: string,
    metadata: EthereumMainnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolveArtifactBytes?: (
    path: string,
    metadata: EthereumMainnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolve_artifact_bytes?: (
    path: string,
    metadata: EthereumMainnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
}

export function verifyEthereumMainnetNativeEvmProverArtifactsFromBundle(
  input: EthereumMainnetNativeEvmProverArtifactBundleInput,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): Promise<EthereumMainnetNativeEvmProverArtifacts>;

export type BscTestnetNativeEvmProverBundleSdkArtifactInput =
  EthereumMainnetNativeEvmProverBundleSdkArtifactInput;
export type BscTestnetNativeEvmProverAuditHashesInput =
  EthereumMainnetNativeEvmProverAuditHashesInput;
export type BscTestnetNativeEvmProverAuditHashes =
  EthereumMainnetNativeEvmProverAuditHashes;

export interface BscTestnetNativeEvmProverBundleInput
  extends Omit<
    EthereumMainnetNativeEvmProverBundleInput,
    "bundleId" | "bundle_id" | "chain"
  > {
  bundleId?: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1;
  bundle_id?: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1;
  chain?: "bsc-testnet";
}

export type BscTestnetNativeEvmProverBundleSdkArtifact =
  EthereumMainnetNativeEvmProverBundleSdkArtifact;

export interface BscTestnetNativeEvmProverBundle
  extends Omit<EthereumMainnetNativeEvmProverBundle, "bundleId" | "chain"> {
  readonly bundleId: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_BUNDLE_ID_V1;
  readonly chain: "bsc-testnet";
}

export function validateBscTestnetNativeEvmProverBundle(
  manifest: BscTestnetNativeEvmProverBundleInput,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): BscTestnetNativeEvmProverBundle;

export function parseBscTestnetNativeEvmProverBundleManifest(
  json: string,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): BscTestnetNativeEvmProverBundle;

export type BscTestnetNativeEvmProverParitySdkResultInput =
  EthereumMainnetNativeEvmProverParitySdkResultInput;
export type BscTestnetNativeEvmProverParitySdkResult =
  EthereumMainnetNativeEvmProverParitySdkResult;

export interface BscTestnetNativeEvmProverParityFixtureInput
  extends Omit<
    EthereumMainnetNativeEvmProverParityFixtureInput,
    "schema" | "chain"
  > {
  schema?: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1;
  chain?: "bsc-testnet";
}

export interface BscTestnetNativeEvmProverParityFixture
  extends Omit<
    EthereumMainnetNativeEvmProverParityFixture,
    "schema" | "chain"
  > {
  readonly schema: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1;
  readonly chain: "bsc-testnet";
}

export function validateBscTestnetNativeEvmProverParityFixture(
  fixture: BscTestnetNativeEvmProverParityFixtureInput,
  nativeProverBundle: BscTestnetNativeEvmProverBundleInput,
): BscTestnetNativeEvmProverParityFixture;

export function parseBscTestnetNativeEvmProverParityFixture(
  json: string,
  nativeProverBundle: BscTestnetNativeEvmProverBundleInput,
): BscTestnetNativeEvmProverParityFixture;

export type BscTestnetNativeEvmProverSelfTestSdkResultInput =
  EthereumMainnetNativeEvmProverSelfTestSdkResultInput;
export type BscTestnetNativeEvmProverSelfTestSdkResult =
  EthereumMainnetNativeEvmProverSelfTestSdkResult;

export interface BscTestnetNativeEvmProverSelfTestFixtureInput
  extends Omit<
    EthereumMainnetNativeEvmProverSelfTestFixtureInput,
    "schema" | "chain"
  > {
  schema?: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1;
  chain?: "bsc-testnet";
}

export interface BscTestnetNativeEvmProverSelfTestFixture
  extends Omit<
    EthereumMainnetNativeEvmProverSelfTestFixture,
    "schema" | "chain"
  > {
  readonly schema: typeof SCCP_BSC_TESTNET_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1;
  readonly chain: "bsc-testnet";
}

export function validateBscTestnetNativeEvmProverSelfTestFixture(
  fixture: BscTestnetNativeEvmProverSelfTestFixtureInput,
  nativeProverBundle: BscTestnetNativeEvmProverBundleInput,
): BscTestnetNativeEvmProverSelfTestFixture;

export function parseBscTestnetNativeEvmProverSelfTestFixture(
  json: string,
  nativeProverBundle: BscTestnetNativeEvmProverBundleInput,
): BscTestnetNativeEvmProverSelfTestFixture;

export interface BscTestnetNativeEvmProverArtifacts
  extends Omit<
    EthereumMainnetNativeEvmProverArtifacts,
    "nativeProverBundle" | "crossSdkFixtureParity" | "nativeProverSelfTest"
  > {
  readonly nativeProverBundle: Readonly<BscTestnetNativeEvmProverBundle>;
  readonly crossSdkFixtureParity: Readonly<BscTestnetNativeEvmProverParityFixture>;
  readonly nativeProverSelfTest: Readonly<BscTestnetNativeEvmProverSelfTestFixture>;
}

export interface BscTestnetNativeEvmProverArtifactsInput
  extends Omit<EthereumMainnetNativeEvmProverArtifactsInput, "nativeProverBundle" | "native_prover_bundle" | "proverBundle" | "prover_bundle" | "manifest"> {
  nativeProverBundle?: BscTestnetNativeEvmProverBundleInput | string;
  native_prover_bundle?: BscTestnetNativeEvmProverBundleInput | string;
  proverBundle?: BscTestnetNativeEvmProverBundleInput | string;
  prover_bundle?: BscTestnetNativeEvmProverBundleInput | string;
  manifest?: BscTestnetNativeEvmProverBundleInput | string;
}

export function verifyBscTestnetNativeEvmProverArtifacts(
  input: BscTestnetNativeEvmProverArtifactsInput,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): BscTestnetNativeEvmProverArtifacts;

export interface BscTestnetNativeEvmProverArtifactResolverMetadata
  extends Omit<
    EthereumMainnetNativeEvmProverArtifactResolverMetadata,
    "nativeProverBundle"
  > {
  readonly nativeProverBundle: Readonly<BscTestnetNativeEvmProverBundle>;
}

export interface BscTestnetNativeEvmProverArtifactBundleInput
  extends Omit<
    EthereumMainnetNativeEvmProverArtifactBundleInput,
    "nativeProverBundle" | "native_prover_bundle" | "proverBundle" | "prover_bundle" | "manifest" | "artifactResolver" | "artifact_resolver" | "resolveArtifact" | "resolve_artifact" | "resolveArtifactBytes" | "resolve_artifact_bytes"
  > {
  nativeProverBundle?: BscTestnetNativeEvmProverBundleInput | string;
  native_prover_bundle?: BscTestnetNativeEvmProverBundleInput | string;
  proverBundle?: BscTestnetNativeEvmProverBundleInput | string;
  prover_bundle?: BscTestnetNativeEvmProverBundleInput | string;
  manifest?: BscTestnetNativeEvmProverBundleInput | string;
  artifactResolver?: (
    path: string,
    metadata: BscTestnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  artifact_resolver?: (
    path: string,
    metadata: BscTestnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolveArtifact?: (
    path: string,
    metadata: BscTestnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolve_artifact?: (
    path: string,
    metadata: BscTestnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolveArtifactBytes?: (
    path: string,
    metadata: BscTestnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
  resolve_artifact_bytes?: (
    path: string,
    metadata: BscTestnetNativeEvmProverArtifactResolverMetadata,
  ) => BinaryLike | Promise<BinaryLike>;
}

export function verifyBscTestnetNativeEvmProverArtifactsFromBundle(
  input: BscTestnetNativeEvmProverArtifactBundleInput,
  options?: {
    expectedDestinationBindingHash?: string;
    expected_destination_binding_hash?: string;
    destinationBindingHash?: string;
    destination_binding_hash?: string;
    destinationBinding?: EvmSccpDestinationBindingInput;
    destination_binding?: EvmSccpDestinationBindingInput;
  },
): Promise<BscTestnetNativeEvmProverArtifacts>;

export interface BscTestnetNativeProverSelfTestContext
  extends Omit<
    EthereumMainnetNativeProverSelfTestContext,
    "nativeProverArtifacts" | "nativeProverSelfTest" | "expectedResult"
  > {
  readonly nativeProverArtifacts: Readonly<BscTestnetNativeEvmProverArtifacts>;
  readonly nativeProverSelfTest: Readonly<BscTestnetNativeEvmProverSelfTestFixture>;
  readonly expectedResult: Readonly<BscTestnetNativeEvmProverSelfTestSdkResult>;
}

export type BscTestnetNativeProverSelfTestFn = (
  context: Readonly<BscTestnetNativeProverSelfTestContext>,
  options?: Record<string, unknown>,
) =>
  | BscTestnetNativeEvmProverSelfTestSdkResultInput
  | Promise<BscTestnetNativeEvmProverSelfTestSdkResultInput>;

export interface BscTestnetNativeProverSelfTestRunInput
  extends Omit<
    EthereumMainnetNativeProverSelfTestRunInput,
    "nativeProverArtifacts" | "native_prover_artifacts" | "verifiedNativeProverArtifacts" | "verified_native_prover_artifacts" | "nativeProverSelfTest" | "native_prover_self_test" | "selfTestNativeProver" | "self_test_native_prover"
  > {
  nativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
  native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
  verifiedNativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
  verified_native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
  nativeProverSelfTest?: BscTestnetNativeProverSelfTestFn;
  native_prover_self_test?: BscTestnetNativeProverSelfTestFn;
  selfTestNativeProver?: BscTestnetNativeProverSelfTestFn;
  self_test_native_prover?: BscTestnetNativeProverSelfTestFn;
}

export function runBscTestnetNativeProverSelfTest(
  input: BscTestnetNativeProverSelfTestRunInput,
  options?: {
    nativeProverSelfTest?: BscTestnetNativeProverSelfTestFn;
    native_prover_self_test?: BscTestnetNativeProverSelfTestFn;
    selfTestNativeProver?: BscTestnetNativeProverSelfTestFn;
    self_test_native_prover?: BscTestnetNativeProverSelfTestFn;
  } & Record<string, unknown>,
): Promise<BscTestnetNativeEvmProverSelfTestSdkResult>;

export interface EvmSccpProofRequestInput {
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  sourceProofBytes?: BinaryLike;
  source_proof_bytes?: BinaryLike;
  backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  destinationBinding?: EvmSccpDestinationBindingInput;
  destination_binding?: EvmSccpDestinationBindingInput;
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  circuitArtifactHash?: string;
  circuit_artifact_hash?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
  nativeProverBundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  native_prover_bundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  proverBundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  prover_bundle?: EthereumMainnetNativeEvmProverBundleInput | string;
  manifest?: EthereumMainnetNativeEvmProverBundleInput | string;
  nativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
  native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
  verifiedNativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
  verified_native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
  proofArtifactBytes?: BinaryLike;
  proof_artifact_bytes?: BinaryLike;
  proverArtifactBytes?: BinaryLike;
  prover_artifact_bytes?: BinaryLike;
  circuitArtifactBytes?: BinaryLike;
  circuit_artifact_bytes?: BinaryLike;
  provingKeyBytes?: BinaryLike;
  proving_key_bytes?: BinaryLike;
  verifierKeyBytes?: BinaryLike;
  verifier_key_bytes?: BinaryLike;
  sdk?: "dotnet" | "java-android" | "javascript" | "kotlin" | "swift";
  implementationBytes?: BinaryLike;
  implementation_bytes?: BinaryLike;
  nativeImplementationBytes?: BinaryLike;
  native_implementation_bytes?: BinaryLike;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
}

export interface EvmSccpProofRequest {
  readonly version: 1;
  readonly backend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly publicInputs: Readonly<
    Required<{
      version: number;
      messageId: string;
      payloadHash: string;
      targetDomain: number;
      commitmentRoot: string;
      finalityHeight: string;
      finalityBlockHash: string;
    }>
  >;
  readonly publicInputsBytes: Uint8Array;
  readonly publicSignalWords: readonly string[];
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBinding?: Readonly<EvmSccpDestinationBinding>;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
  readonly proofArtifactHash?: string;
  readonly provingKeyHash?: string;
}

export interface EvmSccpProofResult {
  readonly version: 1;
  readonly backend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
  readonly publicInputs: EvmSccpProofRequest["publicInputs"];
  readonly publicSignalWords: readonly string[];
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBinding: Readonly<EvmSccpDestinationBinding>;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
  readonly envelopeHash: string;
  readonly proofArtifactHash?: string;
  readonly provingKeyHash?: string;
}

export interface EvmSccpSubmissionInput {
  proofResult?: EvmSccpProofResult;
  proof_result?: EvmSccpProofResult;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  /** Accepted only with proofResult; raw contract-call submissions cannot bind bundle bytes. */
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  /** Accepted only with proofResult; raw contract-call submissions cannot bind source proof bytes. */
  sourceProofBytes?: BinaryLike;
  source_proof_bytes?: BinaryLike;
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  circuitArtifactHash?: string;
  circuit_artifact_hash?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
}

export interface EvmSccpSubmission {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly verifierBackend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly platformPayload: "evm_groth16_contract_call";
  readonly envelopeEncoding: typeof SCCP_EVM_CONTRACT_CALL_ABI_TUPLE_V1;
  readonly submissionKind: "contract_call";
  readonly verifierEntrypoint: "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)";
  readonly contractMethod: typeof SCCP_SUBMIT_MESSAGE_PROOF_ABI_V1;
  readonly functionSelector: string;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly publicInputs: EvmSccpProofRequest["publicInputs"];
  readonly publicInputWords: readonly string[];
  readonly publicSignalWords: readonly string[];
  readonly proofBytes: Uint8Array;
  readonly publicInputWordsBytes: Uint8Array;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly arguments: ReadonlyArray<
    Readonly<{
      key: "proof_bytes" | "public_inputs" | "statement_hash";
      encoding: "raw_bytes" | "abi_bytes32x6" | "abi_bytes32";
      bytes: string;
    }>
  >;
  readonly callData: Uint8Array;
  readonly callDataHex: string;
  readonly envelopeBytes: Uint8Array;
  readonly envelopeHex: string;
}

export type EthereumMainnetSccpSubmissionInput = EvmSccpSubmissionInput &
  ({ proofResult: EvmSccpProofResult } | { proof_result: EvmSccpProofResult });

export interface EvmMainnetLocalAdmissionSubmissionInput {
  proofResult?: {
    proofBytes?: BinaryLike;
    proof_bytes?: BinaryLike;
    publicInputsBytes?: BinaryLike;
    public_inputs_bytes?: BinaryLike;
    bundleBytes?: BinaryLike;
    bundle_bytes?: BinaryLike;
    envelopeBytes?: BinaryLike;
    envelope_bytes?: BinaryLike;
    statementHash?: string;
    statement_hash?: string;
    sourceVerifierMaterialHash?: string;
    source_verifier_material_hash?: string;
    sourceAdapterEngineDeploymentHash?: string;
    source_adapter_engine_deployment_hash?: string;
    submissionPackage?: Record<string, unknown>;
    submission_package?: Record<string, unknown>;
  };
  proof_result?: EvmMainnetLocalAdmissionSubmissionInput["proofResult"];
  submissionPackage?: Record<string, unknown>;
  submission_package?: Record<string, unknown>;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  publicInputsBytes?: BinaryLike;
  public_inputs_bytes?: BinaryLike;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  envelopeBytes?: BinaryLike;
  envelope_bytes?: BinaryLike;
  statementHash?: string;
  statement_hash?: string;
  sourceVerifierMaterialHash?: string;
  source_verifier_material_hash?: string;
  sourceAdapterEngineDeploymentHash?: string;
  source_adapter_engine_deployment_hash?: string;
  proofFamily?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  proof_family?: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  verifierBackend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  verifier_backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  envelopeEncoding?: typeof SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1;
  envelope_encoding?: typeof SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1;
  submissionKind?: typeof SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1;
  submission_kind?: typeof SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1;
  verifierEntrypoint?: typeof SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1;
  verifier_entrypoint?: typeof SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1;
}

export interface EvmMainnetLocalAdmissionPayload {
  readonly version: 1;
  readonly proofBytes: Uint8Array;
  readonly proofBytesHex: string;
  readonly publicInputsBytes: Uint8Array;
  readonly publicInputsBytesHex: string;
  readonly bundleBytes: Uint8Array;
  readonly bundleBytesHex: string;
  readonly statementHash: string;
  readonly sourceVerifierMaterialHash: string;
  readonly sourceAdapterEngineDeploymentHash: string;
}

export interface EvmMainnetLocalAdmissionSubmission {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly verifierBackend: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly platformPayload: typeof SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1;
  readonly envelopeEncoding: typeof SCCP_LOCAL_ADMISSION_ENVELOPE_ENCODING_V1;
  readonly submissionKind: typeof SCCP_LOCAL_ADMISSION_SUBMISSION_KIND_V1;
  readonly verifierEntrypoint: typeof SCCP_LOCAL_ADMISSION_ENTRYPOINT_V1;
  readonly sourceDomain: number;
  readonly targetDomain: typeof SCCP_DOMAIN_SORA;
  readonly statementHash: string;
  readonly sourceVerifierMaterialHash: string;
  readonly sourceAdapterEngineDeploymentHash: string;
  readonly arguments: readonly [];
  readonly localAdmission: Readonly<EvmMainnetLocalAdmissionPayload>;
  readonly proofBytes: Uint8Array;
  readonly proofBytesHex: string;
  readonly publicInputsBytes: Uint8Array;
  readonly publicInputsBytesHex: string;
  readonly bundleBytes: Uint8Array;
  readonly bundleBytesHex: string;
  readonly envelopeBytes: Uint8Array;
  readonly envelopeHex: string;
}

export type EthereumMainnetLocalAdmissionSubmissionInput =
  EvmMainnetLocalAdmissionSubmissionInput;
export type EthereumMainnetLocalAdmissionSubmission =
  EvmMainnetLocalAdmissionSubmission & {
    readonly sourceDomain: typeof SCCP_DOMAIN_ETH;
  };
export type BscMainnetLocalAdmissionSubmissionInput =
  EvmMainnetLocalAdmissionSubmissionInput;
export type BscMainnetLocalAdmissionSubmission =
  EvmMainnetLocalAdmissionSubmission & {
    readonly sourceDomain: typeof SCCP_DOMAIN_BSC;
  };
export type BscTestnetLocalAdmissionSubmissionInput =
  EvmMainnetLocalAdmissionSubmissionInput;
export type BscTestnetLocalAdmissionSubmission =
  EvmMainnetLocalAdmissionSubmission & {
    readonly sourceDomain: typeof SCCP_DOMAIN_BSC;
  };

export function buildEthereumMainnetSccpLocalAdmissionSubmission(
  input: EthereumMainnetLocalAdmissionSubmissionInput,
): EthereumMainnetLocalAdmissionSubmission;
export function buildBscMainnetSccpLocalAdmissionSubmission(
  input: BscMainnetLocalAdmissionSubmissionInput,
): BscMainnetLocalAdmissionSubmission;
export function buildBscTestnetSccpLocalAdmissionSubmission(
  input: BscTestnetLocalAdmissionSubmissionInput,
): BscTestnetLocalAdmissionSubmission;

export interface TronSccpProofRequestInput {
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  sourceProofBytes?: BinaryLike;
  source_proof_bytes?: BinaryLike;
  backend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  destinationBinding?: TronSccpDestinationBindingInput;
  destination_binding?: TronSccpDestinationBindingInput;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
}

export interface TronSccpProofRequest {
  readonly version: 1;
  readonly backend: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly publicInputs: Readonly<
    Required<{
      version: number;
      messageId: string;
      payloadHash: string;
      targetDomain: number;
      commitmentRoot: string;
      finalityHeight: string;
      finalityBlockHash: string;
    }>
  >;
  readonly publicInputsBytes: Uint8Array;
  readonly publicSignalWords: readonly string[];
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBinding?: Readonly<TronSccpDestinationBinding>;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
}

export interface TronSccpProofResult {
  readonly version: 1;
  readonly backend: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
  readonly publicInputs: TronSccpProofRequest["publicInputs"];
  readonly publicSignalWords: readonly string[];
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBinding: Readonly<TronSccpDestinationBinding>;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
  readonly envelopeHash: string;
}

export interface TronSccpSubmissionInput {
  proofResult?: TronSccpProofResult;
  proof_result?: TronSccpProofResult;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  /** Accepted only with proofResult; raw contract-call submissions cannot bind bundle bytes. */
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  /** Accepted only with proofResult; raw contract-call submissions cannot bind source proof bytes. */
  sourceProofBytes?: BinaryLike;
  source_proof_bytes?: BinaryLike;
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
}

export interface TronSccpSubmission {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly verifierBackend: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  readonly platformPayload: "tron_contract_call";
  readonly envelopeEncoding: typeof SCCP_TRON_CONTRACT_CALL_ABI_TUPLE_V1;
  readonly submissionKind: "contract_call";
  readonly verifierEntrypoint: "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)";
  readonly contractMethod: typeof SCCP_SUBMIT_MESSAGE_PROOF_ABI_V1;
  readonly functionSelector: string;
  readonly sourceDomain: number;
  readonly targetDomain: typeof SCCP_DOMAIN_TRON;
  readonly publicInputs: TronSccpProofRequest["publicInputs"];
  readonly publicInputWords: readonly string[];
  readonly publicSignalWords: readonly string[];
  readonly proofBytes: Uint8Array;
  readonly publicInputWordsBytes: Uint8Array;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly arguments: ReadonlyArray<
    Readonly<{
      key: "proof_bytes" | "public_inputs" | "statement_hash";
      encoding: "raw_bytes" | "abi_bytes32x6" | "abi_bytes32";
      bytes: string;
    }>
  >;
  readonly callData: Uint8Array;
  readonly callDataHex: string;
  readonly envelopeBytes: Uint8Array;
  readonly envelopeHex: string;
}

export interface SccpBridgeProofSubmitPayloadBaseInput {
  authority: string;
  privateKey?: unknown;
  private_key?: unknown;
  publicKeyHex?: string;
  public_key_hex?: string;
  signatureB64?: string;
  signature_b64?: string;
  messageBundle?: Record<string, unknown>;
  message_bundle?: Record<string, unknown>;
  creationTimeMs?: NumericLike;
  creation_time_ms?: NumericLike;
}

export interface EvmSccpBridgeProofSubmitPayloadInput
  extends SccpBridgeProofSubmitPayloadBaseInput {
  submission?: EvmSccpSubmission;
  evmSccpSubmission?: EvmSccpSubmission;
  evm_sccp_submission?: EvmSccpSubmission;
  sccpSubmission?: EvmSccpSubmission;
  sccp_submission?: EvmSccpSubmission;
  destinationBinding?: EvmSccpDestinationBindingInput;
  destination_binding?: EvmSccpDestinationBindingInput;
}

export interface TronSccpBridgeProofSubmitPayloadInput
  extends SccpBridgeProofSubmitPayloadBaseInput {
  submission?: TronSccpSubmission;
  tronSccpSubmission?: TronSccpSubmission;
  tron_sccp_submission?: TronSccpSubmission;
  sccpSubmission?: TronSccpSubmission;
  sccp_submission?: TronSccpSubmission;
  destinationBinding?: TronSccpDestinationBindingInput;
  destination_binding?: TronSccpDestinationBindingInput;
}

export interface SubstrateSccpProofRequestInput {
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  sourceProofBytes?: BinaryLike;
  source_proof_bytes?: BinaryLike;
  backend?: typeof SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
}

export interface SubstrateSccpProofRequest {
  readonly version: 1;
  readonly backend: typeof SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly publicInputs: Readonly<
    Required<{
      version: number;
      messageId: string;
      payloadHash: string;
      targetDomain: number;
      commitmentRoot: string;
      finalityHeight: string;
      finalityBlockHash: string;
    }>
  >;
  readonly publicInputsBytes: Uint8Array;
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
}

export interface SubstrateSccpProofResult {
  readonly version: 1;
  readonly backend: typeof SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1;
  readonly proofBytes: Uint8Array;
  readonly proofBase64: string;
  readonly publicInputs: SubstrateSccpProofRequest["publicInputs"];
  readonly bundleBytes: Uint8Array;
  readonly sourceProofBytes: Uint8Array;
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
  readonly envelopeHash: string;
}

export interface SubstrateSccpSubmissionInput {
  proofResult?: SubstrateSccpProofResult;
  proof_result?: SubstrateSccpProofResult;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  publicInputsBytes?: BinaryLike;
  public_inputs_bytes?: BinaryLike;
  bundleBytes?: BinaryLike;
  bundle_bytes?: BinaryLike;
  /** Accepted only with `proofResult`; raw runtime-call payloads do not carry these request-bound bytes. */
  sourceProofBytes?: BinaryLike;
  /** Accepted only with `proof_result`; raw runtime-call payloads do not carry these request-bound bytes. */
  source_proof_bytes?: BinaryLike;
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
}

export interface SubstrateSccpSubmission {
  readonly version: 1;
  readonly proofFamily: typeof SCCP_STARK_FRI_PROOF_FAMILY_V1;
  readonly verifierBackend: typeof SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1;
  readonly platformPayload: "substrate_runtime_call";
  readonly envelopeEncoding: typeof SCCP_SUBSTRATE_RUNTIME_CALL_SCALE_V1;
  readonly submissionKind: "runtime_call";
  readonly verifierEntrypoint: typeof SCCP_SUBSTRATE_SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1;
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly publicInputs: SubstrateSccpProofRequest["publicInputs"];
  readonly proofContext: Readonly<SolanaSccpProofContext>;
  readonly statementHash: string;
  readonly destinationBindingHash: string;
  readonly requestHash: string;
  readonly proofBytes: Uint8Array;
  readonly publicInputsBytes: Uint8Array;
  readonly bundleBytes: Uint8Array;
  readonly arguments: ReadonlyArray<
    Readonly<{
      key: "proof_bytes" | "public_inputs" | "bundle_bytes";
      encoding: "raw_bytes";
      bytes: string;
    }>
  >;
  readonly runtimeCall: Uint8Array;
  readonly runtimeCallHex: string;
  readonly envelopeBytes: Uint8Array;
  readonly envelopeHex: string;
}

export type SccpWitnessProviderFn<Input> = (
  input: Input,
  options?: Record<string, unknown>,
) => Input | Promise<Input>;

export type SccpWitnessProviderResolverOption<Input> =
  | {
      resolveWitness: SccpWitnessProviderFn<Input>;
      resolve_witness?: never;
    }
  | {
      resolveWitness?: never;
      resolve_witness: SccpWitnessProviderFn<Input>;
    };

export type SccpProverWitnessProviderOption<Provider, Input> =
  | {
      witnessProvider?: Provider | SccpWitnessProviderFn<Input>;
      witness_provider?: never;
    }
  | {
      witnessProvider?: never;
      witness_provider?: Provider | SccpWitnessProviderFn<Input>;
    };

export type SccpProverProveOption<ProveFn> =
  | {
      prove?: ProveFn;
      proveFn?: never;
      prove_fn?: never;
    }
  | {
      prove?: never;
      proveFn?: ProveFn;
      prove_fn?: never;
    }
  | {
      prove?: never;
      proveFn?: never;
      prove_fn?: ProveFn;
    };

export type TonSccpWitnessProvider =
  SccpWitnessProviderResolverOption<TonSccpProofRequestInput>;

export interface TonSccpProveResult {
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  proofBase64?: string;
  proof_base64?: string;
  backend?: typeof SCCP_TON_CONTRACT_PROOF_BACKEND_V1;
  requestHash?: string;
  request_hash?: string;
  sourceAdapterDeploymentBindingHash?: string;
  source_adapter_deployment_binding_hash?: string;
  envelopeHash?: string;
  envelope_hash?: string;
}

export type TonSccpProveFn = (
  request: TonSccpProofRequest,
  options?: Record<string, unknown>,
) => TonSccpProveResult | Promise<TonSccpProveResult>;

export type TonSccpProverOptions = SccpProverWitnessProviderOption<
  TonSccpWitnessProvider,
  TonSccpProofRequestInput
> &
  SccpProverProveOption<TonSccpProveFn>;

export class TonSccpProver {
  constructor(options?: TonSccpProverOptions);
  buildRequest(
    input: TonSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<TonSccpProofRequest>;
  prove(
    input: TonSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<TonSccpProofResult>;
}

export type EvmSccpWitnessProvider =
  SccpWitnessProviderResolverOption<EvmSccpProofRequestInput>;

export interface EvmSccpProveResult {
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  proofBase64?: string;
  proof_base64?: string;
  backend?: typeof SCCP_EVM_GROTH16_BN254_PROOF_BACKEND_V1;
  requestHash?: string;
  request_hash?: string;
  envelopeHash?: string;
  envelope_hash?: string;
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  proofArtifactHash?: string;
  proof_artifact_hash?: string;
  proverArtifactHash?: string;
  prover_artifact_hash?: string;
  circuitArtifactHash?: string;
  circuit_artifact_hash?: string;
  provingKeyHash?: string;
  proving_key_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
}

export type EvmSccpProveFn = (
  request: EvmSccpProofRequest,
  options?: Record<string, unknown>,
) => EvmSccpProveResult | Promise<EvmSccpProveResult>;

export type EvmSccpProverOptions = SccpProverWitnessProviderOption<
  EvmSccpWitnessProvider,
  EvmSccpProofRequestInput
> &
  SccpProverProveOption<EvmSccpProveFn>;

export class EvmSccpProver {
  constructor(options?: EvmSccpProverOptions);
  buildRequest(
    input: EvmSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<EvmSccpProofRequest>;
  prove(
    input: EvmSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<EvmSccpProofResult>;
}

export type BscMainnetSccpProofRequestInput = EvmSccpProofRequestInput;
export type BscMainnetSccpProofRequest = EvmSccpProofRequest;
export type BscMainnetSccpProofResult = EvmSccpProofResult;
export type BscMainnetSccpSubmissionInput = EvmSccpSubmissionInput &
  ({ proofResult: EvmSccpProofResult } | { proof_result: EvmSccpProofResult });
export type BscMainnetSccpSubmission = EvmSccpSubmission;
export type BscMainnetSccpProverOptions = EvmSccpProverOptions;
export type BscTestnetSccpProofRequestInput = EvmSccpProofRequestInput;
export type BscTestnetSccpProofRequest = EvmSccpProofRequest;
export type BscTestnetSccpProofResult = EvmSccpProofResult;
export type BscTestnetSccpSubmissionInput = EvmSccpSubmissionInput &
  ({ proofResult: EvmSccpProofResult } | { proof_result: EvmSccpProofResult });
export type BscTestnetSccpSubmission = EvmSccpSubmission;
export type BscTestnetSccpProverOptions = EvmSccpProverOptions;

export class BscMainnetSccpProver {
  constructor(options?: BscMainnetSccpProverOptions);
  buildRequest(
    input: BscMainnetSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<BscMainnetSccpProofRequest>;
  prove(
    input: BscMainnetSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<BscMainnetSccpProofResult>;
}

export class BscTestnetSccpProver {
  constructor(options?: BscTestnetSccpProverOptions);
  buildRequest(
    input: BscTestnetSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<BscTestnetSccpProofRequest>;
  prove(
    input: BscTestnetSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<BscTestnetSccpProofResult>;
}

export type EthereumMainnetExecutionProvider =
  | {
      request(args: {
        method: string;
        params?: readonly unknown[];
      }): unknown | Promise<unknown>;
    }
  | {
      send(
        method: string,
        params?: readonly unknown[],
      ): unknown | Promise<unknown>;
    }
  | ((args: {
      method: string;
      params?: readonly unknown[];
    }) => unknown | Promise<unknown>);

export type EthereumMainnetBeaconRestFetchResponse = {
  readonly ok?: boolean;
  readonly status?: number;
  readonly statusText?: string;
  json(): unknown | Promise<unknown>;
};

export type EthereumMainnetBeaconRestFetch = (
  input: string,
  init?: { method?: string; headers?: unknown },
) =>
  | EthereumMainnetBeaconRestFetchResponse
  | Promise<EthereumMainnetBeaconRestFetchResponse>;

export interface EthereumMainnetBeaconRestConsensusProviderOptions {
  endpoint?: string | URL;
  baseUrl?: string | URL;
  baseURL?: string | URL;
  base_url?: string | URL;
  beaconRestUrl?: string | URL;
  beacon_rest_url?: string | URL;
  beaconRestEndpoint?: string | URL;
  beacon_rest_endpoint?: string | URL;
  fetch?: EthereumMainnetBeaconRestFetch;
  fetchFn?: EthereumMainnetBeaconRestFetch;
  fetch_fn?: EthereumMainnetBeaconRestFetch;
  headers?: unknown;
  syncCommitteeRoot?: string;
  sync_committee_root?: string;
  syncCommitteePayload?: EthSyncCommitteePayloadInput | BinaryLike;
  sync_committee_payload?: EthSyncCommitteePayloadInput | BinaryLike;
  verifyFinalityCheckpoint?: boolean;
  verify_finality_checkpoint?: boolean;
}

export interface EthereumMainnetBeaconFinalityEvidenceInput {
  executionBlockNumber?: string | number | bigint;
  execution_block_number?: string | number | bigint;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  executionBlockHash?: string;
  execution_block_hash?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  executionReceiptsRoot?: string;
  execution_receipts_root?: string;
  receiptsRoot?: string;
  receipts_root?: string;
  finalizedHeaderRoot?: string;
  finalized_header_root?: string;
  beaconFinalizedRoot?: string;
  beacon_finalized_root?: string;
  syncCommitteeRoot?: string;
  sync_committee_root?: string;
  beaconSlot?: string | number | bigint;
  beacon_slot?: string | number | bigint;
  finalizedSlot?: string | number | bigint;
  finalized_slot?: string | number | bigint;
  slot?: string | number | bigint;
  finalityBranch?: readonly string[];
  finality_branch?: readonly string[];
  syncCommitteeBits?: string;
  sync_committee_bits?: string;
  syncCommitteeSignature?: string;
  sync_committee_signature?: string;
  syncSignatureSlot?: string | number | bigint;
  sync_signature_slot?: string | number | bigint;
  signatureSlot?: string | number | bigint;
  signature_slot?: string | number | bigint;
  syncCommitteeParticipation?: string | number | bigint;
  sync_committee_participation?: string | number | bigint;
  [key: string]: unknown;
}

export interface EthereumMainnetBeaconFinalityEvidence
  extends EthereumMainnetBeaconFinalityEvidenceInput {
  readonly executionBlockNumber: string;
  readonly executionBlockHash: string;
  readonly executionReceiptsRoot: string;
  readonly finalizedHeaderRoot?: string;
  readonly syncCommitteeRoot?: string;
  readonly beaconSlot?: string;
  readonly finalityBranch?: readonly string[];
  readonly syncCommitteeBits?: string;
  readonly syncCommitteeSignature?: string;
  readonly syncSignatureSlot?: string;
  readonly syncCommitteeParticipation?: string;
}

export interface EthereumMainnetConsensusProviderInput {
  readonly receipt?: Record<string, unknown>;
  readonly block?: Record<string, unknown>;
  readonly transactionHash?: string;
  readonly beaconBlockId?: string | number | bigint;
  readonly beacon_block_id?: string | number | bigint;
  readonly targetBeaconBlockId?: string | number | bigint;
  readonly target_beacon_block_id?: string | number | bigint;
  readonly beaconBlockRoot?: string;
  readonly beacon_block_root?: string;
  readonly targetBeaconBlockRoot?: string;
  readonly target_beacon_block_root?: string;
  readonly beaconSlot?: string | number | bigint;
  readonly beacon_slot?: string | number | bigint;
  readonly finalizedSlot?: string | number | bigint;
  readonly finalized_slot?: string | number | bigint;
  readonly slot?: string | number | bigint;
}

export type EthereumMainnetConsensusProvider = {
  collectFinalityEvidence(
    input: EthereumMainnetConsensusProviderInput,
    options?: Record<string, unknown>,
  ):
    | EthereumMainnetBeaconFinalityEvidenceInput
    | Promise<EthereumMainnetBeaconFinalityEvidenceInput>;
};

export class EthereumMainnetBeaconRestConsensusProvider
  implements EthereumMainnetConsensusProvider
{
  constructor(
    options: EthereumMainnetBeaconRestConsensusProviderOptions | string | URL,
  );
  collectFinalityEvidence(
    input: EthereumMainnetConsensusProviderInput,
    options?: {
      fetch?: EthereumMainnetBeaconRestFetch;
      fetchFn?: EthereumMainnetBeaconRestFetch;
      fetch_fn?: EthereumMainnetBeaconRestFetch;
      headers?: unknown;
      beaconBlockId?: string | number | bigint;
      beacon_block_id?: string | number | bigint;
      targetBeaconBlockId?: string | number | bigint;
      target_beacon_block_id?: string | number | bigint;
      beaconBlockRoot?: string;
      beacon_block_root?: string;
      targetBeaconBlockRoot?: string;
      target_beacon_block_root?: string;
      beaconSlot?: string | number | bigint;
      beacon_slot?: string | number | bigint;
      finalizedSlot?: string | number | bigint;
      finalized_slot?: string | number | bigint;
      slot?: string | number | bigint;
      syncCommitteeRoot?: string;
      sync_committee_root?: string;
      syncCommitteePayload?: EthSyncCommitteePayloadInput | BinaryLike;
      sync_committee_payload?: EthSyncCommitteePayloadInput | BinaryLike;
      verifyFinalityCheckpoint?: boolean;
      verify_finality_checkpoint?: boolean;
    } & Record<string, unknown>,
  ): Promise<EthereumMainnetBeaconFinalityEvidence>;
}

export interface EthereumMainnetInboundEvidenceInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  transactionHash?: string;
  transaction_hash?: string;
  receipt?: Record<string, unknown>;
  blockReceipts?: readonly Record<string, unknown>[];
  block_receipts?: readonly Record<string, unknown>[];
  receiptBlockReceipts?: readonly Record<string, unknown>[];
  receipt_block_receipts?: readonly Record<string, unknown>[];
  block?: Record<string, unknown>;
  blockHash?: string;
  block_hash?: string;
  beaconFinality?: EthereumMainnetBeaconFinalityEvidenceInput;
  beacon_finality?: EthereumMainnetBeaconFinalityEvidenceInput;
  finalityEvidence?: EthereumMainnetBeaconFinalityEvidenceInput;
  finality_evidence?: EthereumMainnetBeaconFinalityEvidenceInput;
  receiptProof?: EvmSccpReceiptProofInput;
  receipt_proof?: EvmSccpReceiptProofInput;
  receiptProofHash?: string;
  receipt_proof_hash?: string;
  sourceEventDigest?: string;
  source_event_digest?: string;
  sourceBridgeEmitterAddress?: string;
  source_bridge_emitter_address?: string;
  expectedSourceBridgeEmitterAddress?: string;
  expected_source_bridge_emitter_address?: string;
  inclusionBranch?: readonly BinaryLike[];
  inclusion_branch?: readonly BinaryLike[];
  sourceVerifierMaterial?: SccpSourceVerifierMaterialInput;
  source_verifier_material?: SccpSourceVerifierMaterialInput;
  [key: string]: unknown;
}

export interface EthereumMainnetInboundEvidence
  extends EthereumMainnetInboundEvidenceInput {
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly beaconFinality?: EthereumMainnetBeaconFinalityEvidence;
  readonly receiptProofHash?: string;
  readonly sourceEventDigest?: string;
  readonly sourceBridgeEmitterAddress?: string;
}

export type EthereumMainnetInboundProveFn = (
  evidence: EthereumMainnetInboundEvidence,
  options?: Record<string, unknown>,
) => BinaryLike | Promise<BinaryLike>;

export type EthereumMainnetSubmitInboundFn = (
  proofBytes: Uint8Array,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type EthereumMainnetSubmitOutboundFn = (
  submission: EvmSccpSubmission,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type EthereumMainnetSccpOptions = EvmSccpProverOptions & {
  executionProvider?: EthereumMainnetExecutionProvider;
  execution_provider?: EthereumMainnetExecutionProvider;
  consensusProvider?: EthereumMainnetConsensusProvider;
  consensus_provider?: EthereumMainnetConsensusProvider;
  proveInbound?: EthereumMainnetInboundProveFn;
  proveInboundToSora?: EthereumMainnetInboundProveFn;
  prove_inbound?: EthereumMainnetInboundProveFn;
  submitInboundToIroha?: EthereumMainnetSubmitInboundFn;
  submit_inbound_to_iroha?: EthereumMainnetSubmitInboundFn;
  submitToIroha?: EthereumMainnetSubmitInboundFn;
  submitOutboundToEthereum?: EthereumMainnetSubmitOutboundFn;
  submit_outbound_to_ethereum?: EthereumMainnetSubmitOutboundFn;
  submitToEthereum?: EthereumMainnetSubmitOutboundFn;
  nativeProverSelfTest?: EthereumMainnetNativeProverSelfTestFn;
  native_prover_self_test?: EthereumMainnetNativeProverSelfTestFn;
  selfTestNativeProver?: EthereumMainnetNativeProverSelfTestFn;
  self_test_native_prover?: EthereumMainnetNativeProverSelfTestFn;
  destinationBinding?: EvmSccpDestinationBindingInput;
  destination_binding?: EvmSccpDestinationBindingInput;
  nativeProverBundle?: EthereumMainnetNativeEvmProverBundleInput;
  native_prover_bundle?: EthereumMainnetNativeEvmProverBundleInput;
  proverBundle?: EthereumMainnetNativeEvmProverBundleInput;
  prover_bundle?: EthereumMainnetNativeEvmProverBundleInput;
  sourceVerifierMaterial?: SccpSourceVerifierMaterialInput;
  source_verifier_material?: SccpSourceVerifierMaterialInput;
  sourceBridgeEmitterAddress?: string;
  source_bridge_emitter_address?: string;
  expectedSourceBridgeEmitterAddress?: string;
  expected_source_bridge_emitter_address?: string;
  bridgeAddress?: string;
  bridge_address?: string;
  outboundProver?: EvmSccpProver;
  outbound_prover?: EvmSccpProver;
  prover?: EvmSccpProver;
  from?: string;
  account?: string;
};

export type EthereumMainnetSccpNativeProverBundleOptions = Omit<
  EthereumMainnetSccpOptions,
  | "nativeProverBundle"
  | "native_prover_bundle"
  | "proverBundle"
  | "prover_bundle"
  | "nativeProverArtifacts"
  | "native_prover_artifacts"
  | "verifiedNativeProverArtifacts"
  | "verified_native_prover_artifacts"
> &
  EthereumMainnetNativeEvmProverArtifactBundleInput;

export class EthereumMainnetSccp {
  static fromNativeProverBundle(
    options: EthereumMainnetSccpNativeProverBundleOptions,
  ): Promise<EthereumMainnetSccp>;
  constructor(options?: EthereumMainnetSccpOptions);
  validateExecutionProviderMainnet(options?: {
    executionProvider?: EthereumMainnetExecutionProvider;
    execution_provider?: EthereumMainnetExecutionProvider;
  }): Promise<unknown>;
  collectInboundEvidenceFromReceipt(
    input?: EthereumMainnetInboundEvidenceInput,
    options?: {
      executionProvider?: EthereumMainnetExecutionProvider;
      execution_provider?: EthereumMainnetExecutionProvider;
      consensusProvider?: EthereumMainnetConsensusProvider;
      consensus_provider?: EthereumMainnetConsensusProvider;
      sourceVerifierMaterial?: SccpSourceVerifierMaterialInput;
      source_verifier_material?: SccpSourceVerifierMaterialInput;
      sourceBridgeEmitterAddress?: string;
      source_bridge_emitter_address?: string;
      expectedSourceBridgeEmitterAddress?: string;
      expected_source_bridge_emitter_address?: string;
      bridgeAddress?: string;
      bridge_address?: string;
    } & Record<string, unknown>,
  ): Promise<EthereumMainnetInboundEvidence>;
  proveInboundToSora(
    input: EthereumMainnetInboundEvidenceInput,
    options?: {
      executionProvider?: EthereumMainnetExecutionProvider;
      execution_provider?: EthereumMainnetExecutionProvider;
      consensusProvider?: EthereumMainnetConsensusProvider;
      consensus_provider?: EthereumMainnetConsensusProvider;
      proveInbound?: EthereumMainnetInboundProveFn;
      proveInboundToSora?: EthereumMainnetInboundProveFn;
      prove_inbound?: EthereumMainnetInboundProveFn;
    } & Record<string, unknown>,
  ): Promise<Uint8Array>;
  submitInboundToIroha(
    input: BinaryLike,
    options?: {
      submitInboundToIroha?: EthereumMainnetSubmitInboundFn;
      submit_inbound_to_iroha?: EthereumMainnetSubmitInboundFn;
      submitToIroha?: EthereumMainnetSubmitInboundFn;
    } & Record<string, unknown>,
  ): Promise<unknown>;
  buildLocalAdmissionSubmission(
    input: EthereumMainnetLocalAdmissionSubmissionInput,
  ): EthereumMainnetLocalAdmissionSubmission;
  buildOutboundProofRequest(
    input: EvmSccpProofRequestInput,
  ): EvmSccpProofRequest;
  runNativeProverSelfTest(
    options?: {
      nativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
      native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
      verifiedNativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
      verified_native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
      nativeProverSelfTest?: EthereumMainnetNativeProverSelfTestFn;
      native_prover_self_test?: EthereumMainnetNativeProverSelfTestFn;
      selfTestNativeProver?: EthereumMainnetNativeProverSelfTestFn;
      self_test_native_prover?: EthereumMainnetNativeProverSelfTestFn;
    } & Record<string, unknown>,
  ): Promise<EthereumMainnetNativeEvmProverSelfTestSdkResult>;
  proveOutboundToEthereum(
    input: EvmSccpProofRequestInput,
    options?: {
      nativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
      native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
      verifiedNativeProverArtifacts?: EthereumMainnetNativeEvmProverArtifacts;
      verified_native_prover_artifacts?: EthereumMainnetNativeEvmProverArtifacts;
      nativeProverSelfTest?: EthereumMainnetNativeProverSelfTestFn;
      native_prover_self_test?: EthereumMainnetNativeProverSelfTestFn;
      selfTestNativeProver?: EthereumMainnetNativeProverSelfTestFn;
      self_test_native_prover?: EthereumMainnetNativeProverSelfTestFn;
    } & Record<string, unknown>,
  ): Promise<EvmSccpProofResult>;
  buildEthereumCalldata(
    input: EthereumMainnetSccpSubmissionInput,
  ): EvmSccpSubmission;
  submitOutboundToEthereum(
    input: EthereumMainnetSccpSubmissionInput & {
      to?: string;
      bridgeAddress?: string;
      bridge_address?: string;
      from?: string;
      destinationBinding?: EvmSccpDestinationBindingInput;
      destination_binding?: EvmSccpDestinationBindingInput;
    },
    options?: {
      executionProvider?: EthereumMainnetExecutionProvider;
      execution_provider?: EthereumMainnetExecutionProvider;
      submitOutboundToEthereum?: EthereumMainnetSubmitOutboundFn;
      submit_outbound_to_ethereum?: EthereumMainnetSubmitOutboundFn;
      submitToEthereum?: EthereumMainnetSubmitOutboundFn;
      destinationBinding?: EvmSccpDestinationBindingInput;
      destination_binding?: EvmSccpDestinationBindingInput;
      to?: string;
      bridgeAddress?: string;
      bridge_address?: string;
      from?: string;
    } & Record<string, unknown>,
  ): Promise<unknown>;
}

export type BscMainnetExecutionProvider = EthereumMainnetExecutionProvider;

export interface BscMainnetParliaFinalityEvidenceInput {
  executionBlockNumber?: string | number | bigint;
  execution_block_number?: string | number | bigint;
  finalityHeight?: string | number | bigint;
  finality_height?: string | number | bigint;
  executionBlockHash?: string;
  execution_block_hash?: string;
  finalityBlockHash?: string;
  finality_block_hash?: string;
  executionReceiptsRoot?: string;
  execution_receipts_root?: string;
  receiptsRoot?: string;
  receipts_root?: string;
  validatorEpoch?: string | number | bigint;
  validator_epoch?: string | number | bigint;
  commitSealHash?: string;
  commit_seal_hash?: string;
  [key: string]: unknown;
}

export interface BscMainnetParliaFinalityEvidence
  extends BscMainnetParliaFinalityEvidenceInput {
  readonly executionBlockNumber: string;
  readonly executionBlockHash: string;
  readonly executionReceiptsRoot: string;
}

export interface BscMainnetConsensusProviderInput {
  readonly receipt?: Record<string, unknown>;
  readonly block?: Record<string, unknown>;
  readonly transactionHash?: string;
}

export type BscMainnetConsensusProvider = {
  collectFinalityEvidence(
    input: BscMainnetConsensusProviderInput,
    options?: Record<string, unknown>,
  ):
    | BscMainnetParliaFinalityEvidenceInput
    | Promise<BscMainnetParliaFinalityEvidenceInput>;
};

export interface BscMainnetInboundEvidenceInput {
  sourceDomain?: SccpDomainIdInput;
  source_domain?: SccpDomainIdInput;
  targetDomain?: SccpDomainIdInput;
  target_domain?: SccpDomainIdInput;
  transactionHash?: string;
  transaction_hash?: string;
  receipt?: Record<string, unknown>;
  block?: Record<string, unknown>;
  blockHash?: string;
  block_hash?: string;
  parliaFinality?: BscMainnetParliaFinalityEvidenceInput;
  parlia_finality?: BscMainnetParliaFinalityEvidenceInput;
  finalityEvidence?: BscMainnetParliaFinalityEvidenceInput;
  finality_evidence?: BscMainnetParliaFinalityEvidenceInput;
  receiptProof?: BscSccpReceiptProofInput;
  receipt_proof?: BscSccpReceiptProofInput;
  receiptProofHash?: string;
  receipt_proof_hash?: string;
  [key: string]: unknown;
}

export interface BscMainnetInboundEvidence
  extends BscMainnetInboundEvidenceInput {
  readonly sourceDomain: number;
  readonly targetDomain: number;
  readonly parliaFinality?: BscMainnetParliaFinalityEvidence;
  readonly receiptProofHash?: string;
}

export type BscMainnetInboundProveFn = (
  evidence: BscMainnetInboundEvidence,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type BscMainnetSubmitInboundFn = (
  proofOrPayload: unknown,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type BscMainnetSubmitOutboundFn = (
  submission: EvmSccpSubmission,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type BscMainnetSccpOptions = BscMainnetSccpProverOptions & {
  executionProvider?: BscMainnetExecutionProvider;
  execution_provider?: BscMainnetExecutionProvider;
  consensusProvider?: BscMainnetConsensusProvider;
  consensus_provider?: BscMainnetConsensusProvider;
  proveInbound?: BscMainnetInboundProveFn;
  proveInboundToSora?: BscMainnetInboundProveFn;
  prove_inbound?: BscMainnetInboundProveFn;
  submitInboundToIroha?: BscMainnetSubmitInboundFn;
  submit_inbound_to_iroha?: BscMainnetSubmitInboundFn;
  submitToIroha?: BscMainnetSubmitInboundFn;
  submitOutboundToBsc?: BscMainnetSubmitOutboundFn;
  submit_outbound_to_bsc?: BscMainnetSubmitOutboundFn;
  submitToBsc?: BscMainnetSubmitOutboundFn;
  destinationBinding?: EvmSccpDestinationBindingInput;
  destination_binding?: EvmSccpDestinationBindingInput;
  outboundProver?: BscMainnetSccpProver;
  outbound_prover?: BscMainnetSccpProver;
  prover?: BscMainnetSccpProver;
  from?: string;
  account?: string;
};

export class BscMainnetSccp {
  constructor(options?: BscMainnetSccpOptions);
  validateExecutionProviderMainnet(options?: {
    executionProvider?: BscMainnetExecutionProvider;
    execution_provider?: BscMainnetExecutionProvider;
  }): Promise<unknown>;
  collectInboundEvidenceFromReceipt(
    input?: BscMainnetInboundEvidenceInput,
    options?: {
      executionProvider?: BscMainnetExecutionProvider;
      execution_provider?: BscMainnetExecutionProvider;
      consensusProvider?: BscMainnetConsensusProvider;
      consensus_provider?: BscMainnetConsensusProvider;
    } & Record<string, unknown>,
  ): Promise<BscMainnetInboundEvidence>;
  proveInboundToSora(
    input: BscMainnetInboundEvidenceInput,
    options?: {
      executionProvider?: BscMainnetExecutionProvider;
      execution_provider?: BscMainnetExecutionProvider;
      consensusProvider?: BscMainnetConsensusProvider;
      consensus_provider?: BscMainnetConsensusProvider;
      proveInbound?: BscMainnetInboundProveFn;
      proveInboundToSora?: BscMainnetInboundProveFn;
      prove_inbound?: BscMainnetInboundProveFn;
    } & Record<string, unknown>,
  ): Promise<unknown>;
  submitInboundToIroha(
    input: unknown,
    options?: {
      submitInboundToIroha?: BscMainnetSubmitInboundFn;
      submit_inbound_to_iroha?: BscMainnetSubmitInboundFn;
      submitToIroha?: BscMainnetSubmitInboundFn;
    } & Record<string, unknown>,
  ): Promise<unknown>;
  buildLocalAdmissionSubmission(
    input: BscMainnetLocalAdmissionSubmissionInput,
  ): BscMainnetLocalAdmissionSubmission;
  buildOutboundProofRequest(
    input: BscMainnetSccpProofRequestInput,
  ): BscMainnetSccpProofRequest;
  proveOutboundToBsc(
    input: BscMainnetSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<BscMainnetSccpProofResult>;
  buildBscCalldata(
    input: BscMainnetSccpSubmissionInput,
  ): BscMainnetSccpSubmission;
  submitOutboundToBsc(
    input: BscMainnetSccpSubmissionInput & {
      to?: string;
      bridgeAddress?: string;
      bridge_address?: string;
      from?: string;
      destinationBinding?: EvmSccpDestinationBindingInput;
      destination_binding?: EvmSccpDestinationBindingInput;
    },
    options?: {
      executionProvider?: BscMainnetExecutionProvider;
      execution_provider?: BscMainnetExecutionProvider;
      submitOutboundToBsc?: BscMainnetSubmitOutboundFn;
      submit_outbound_to_bsc?: BscMainnetSubmitOutboundFn;
      submitToBsc?: BscMainnetSubmitOutboundFn;
      destinationBinding?: EvmSccpDestinationBindingInput;
      destination_binding?: EvmSccpDestinationBindingInput;
      to?: string;
      bridgeAddress?: string;
      bridge_address?: string;
      from?: string;
    } & Record<string, unknown>,
  ): Promise<unknown>;
}

export type BscTestnetExecutionProvider = BscMainnetExecutionProvider;
export type BscTestnetParliaFinalityEvidenceInput =
  BscMainnetParliaFinalityEvidenceInput;
export type BscTestnetParliaFinalityEvidence = BscMainnetParliaFinalityEvidence;
export type BscTestnetConsensusProviderInput = BscMainnetConsensusProviderInput;
export type BscTestnetConsensusProvider = BscMainnetConsensusProvider;
export type BscTestnetInboundEvidenceInput = BscMainnetInboundEvidenceInput;
export type BscTestnetInboundEvidence = BscMainnetInboundEvidence;

export type BscTestnetInboundProveFn = (
  evidence: BscTestnetInboundEvidence,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type BscTestnetSubmitInboundFn = (
  proofOrPayload: unknown,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type BscTestnetSubmitOutboundFn = (
  submission: EvmSccpSubmission,
  options?: Record<string, unknown>,
) => unknown | Promise<unknown>;

export type BscTestnetSccpOptions = BscTestnetSccpProverOptions & {
  executionProvider?: BscTestnetExecutionProvider;
  execution_provider?: BscTestnetExecutionProvider;
  consensusProvider?: BscTestnetConsensusProvider;
  consensus_provider?: BscTestnetConsensusProvider;
  proveInbound?: BscTestnetInboundProveFn;
  proveInboundToSora?: BscTestnetInboundProveFn;
  prove_inbound?: BscTestnetInboundProveFn;
  submitInboundToIroha?: BscTestnetSubmitInboundFn;
  submit_inbound_to_iroha?: BscTestnetSubmitInboundFn;
  submitToIroha?: BscTestnetSubmitInboundFn;
  submitOutboundToBsc?: BscTestnetSubmitOutboundFn;
  submit_outbound_to_bsc?: BscTestnetSubmitOutboundFn;
  submitToBsc?: BscTestnetSubmitOutboundFn;
  nativeProverSelfTest?: BscTestnetNativeProverSelfTestFn;
  native_prover_self_test?: BscTestnetNativeProverSelfTestFn;
  selfTestNativeProver?: BscTestnetNativeProverSelfTestFn;
  self_test_native_prover?: BscTestnetNativeProverSelfTestFn;
  destinationBinding?: EvmSccpDestinationBindingInput;
  destination_binding?: EvmSccpDestinationBindingInput;
  nativeProverBundle?: BscTestnetNativeEvmProverBundleInput;
  native_prover_bundle?: BscTestnetNativeEvmProverBundleInput;
  proverBundle?: BscTestnetNativeEvmProverBundleInput;
  prover_bundle?: BscTestnetNativeEvmProverBundleInput;
  nativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
  native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
  verifiedNativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
  verified_native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
  outboundProver?: BscTestnetSccpProver;
  outbound_prover?: BscTestnetSccpProver;
  prover?: BscTestnetSccpProver;
  from?: string;
  account?: string;
};

export class BscTestnetSccp {
  static fromNativeProverBundle(
    options?: BscTestnetSccpOptions &
      BscTestnetNativeEvmProverArtifactBundleInput,
  ): Promise<BscTestnetSccp>;
  constructor(options?: BscTestnetSccpOptions);
  validateExecutionProviderTestnet(options?: {
    executionProvider?: BscTestnetExecutionProvider;
    execution_provider?: BscTestnetExecutionProvider;
  }): Promise<unknown>;
  collectInboundEvidenceFromReceipt(
    input?: BscTestnetInboundEvidenceInput,
    options?: {
      executionProvider?: BscTestnetExecutionProvider;
      execution_provider?: BscTestnetExecutionProvider;
      consensusProvider?: BscTestnetConsensusProvider;
      consensus_provider?: BscTestnetConsensusProvider;
    } & Record<string, unknown>,
  ): Promise<BscTestnetInboundEvidence>;
  proveInboundToSora(
    input: BscTestnetInboundEvidenceInput,
    options?: {
      executionProvider?: BscTestnetExecutionProvider;
      execution_provider?: BscTestnetExecutionProvider;
      consensusProvider?: BscTestnetConsensusProvider;
      consensus_provider?: BscTestnetConsensusProvider;
      proveInbound?: BscTestnetInboundProveFn;
      proveInboundToSora?: BscTestnetInboundProveFn;
      prove_inbound?: BscTestnetInboundProveFn;
    } & Record<string, unknown>,
  ): Promise<unknown>;
  submitInboundToIroha(
    input: unknown,
    options?: {
      submitInboundToIroha?: BscTestnetSubmitInboundFn;
      submit_inbound_to_iroha?: BscTestnetSubmitInboundFn;
      submitToIroha?: BscTestnetSubmitInboundFn;
    } & Record<string, unknown>,
  ): Promise<unknown>;
  buildLocalAdmissionSubmission(
    input: BscTestnetLocalAdmissionSubmissionInput,
  ): BscTestnetLocalAdmissionSubmission;
  buildOutboundProofRequest(
    input: BscTestnetSccpProofRequestInput,
  ): BscTestnetSccpProofRequest;
  runNativeProverSelfTest(options?: {
    nativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
    native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
    verifiedNativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
    verified_native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
    nativeProverSelfTest?: BscTestnetNativeProverSelfTestFn;
    native_prover_self_test?: BscTestnetNativeProverSelfTestFn;
    selfTestNativeProver?: BscTestnetNativeProverSelfTestFn;
    self_test_native_prover?: BscTestnetNativeProverSelfTestFn;
  } & Record<string, unknown>): Promise<BscTestnetNativeEvmProverSelfTestSdkResult>;
  proveOutboundToBsc(
    input: BscTestnetSccpProofRequestInput,
    options?: {
      nativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
      native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
      verifiedNativeProverArtifacts?: BscTestnetNativeEvmProverArtifacts;
      verified_native_prover_artifacts?: BscTestnetNativeEvmProverArtifacts;
      nativeProverSelfTest?: BscTestnetNativeProverSelfTestFn;
      native_prover_self_test?: BscTestnetNativeProverSelfTestFn;
      selfTestNativeProver?: BscTestnetNativeProverSelfTestFn;
      self_test_native_prover?: BscTestnetNativeProverSelfTestFn;
    } & Record<string, unknown>,
  ): Promise<BscTestnetSccpProofResult>;
  buildBscCalldata(
    input: BscTestnetSccpSubmissionInput,
  ): BscTestnetSccpSubmission;
  submitOutboundToBsc(
    input: BscTestnetSccpSubmissionInput & {
      to?: string;
      bridgeAddress?: string;
      bridge_address?: string;
      from?: string;
      destinationBinding?: EvmSccpDestinationBindingInput;
      destination_binding?: EvmSccpDestinationBindingInput;
    },
    options?: {
      executionProvider?: BscTestnetExecutionProvider;
      execution_provider?: BscTestnetExecutionProvider;
      submitOutboundToBsc?: BscTestnetSubmitOutboundFn;
      submit_outbound_to_bsc?: BscTestnetSubmitOutboundFn;
      submitToBsc?: BscTestnetSubmitOutboundFn;
      destinationBinding?: EvmSccpDestinationBindingInput;
      destination_binding?: EvmSccpDestinationBindingInput;
      to?: string;
      bridgeAddress?: string;
      bridge_address?: string;
      from?: string;
    } & Record<string, unknown>,
  ): Promise<unknown>;
}

export type TronSccpWitnessProvider =
  SccpWitnessProviderResolverOption<TronSccpProofRequestInput>;

export interface TronSccpProveResult {
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  proofBase64?: string;
  proof_base64?: string;
  backend?: typeof SCCP_TRON_GROTH16_BN254_PROOF_BACKEND_V1;
  requestHash?: string;
  request_hash?: string;
  envelopeHash?: string;
  envelope_hash?: string;
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
  publicSignalWords?: readonly string[];
  public_signal_words?: readonly string[];
}

export type TronSccpProveFn = (
  request: TronSccpProofRequest,
  options?: Record<string, unknown>,
) => TronSccpProveResult | Promise<TronSccpProveResult>;

export type TronSccpProverOptions = SccpProverWitnessProviderOption<
  TronSccpWitnessProvider,
  TronSccpProofRequestInput
> &
  SccpProverProveOption<TronSccpProveFn>;

export class TronSccpProver {
  constructor(options?: TronSccpProverOptions);
  buildRequest(
    input: TronSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<TronSccpProofRequest>;
  prove(
    input: TronSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<TronSccpProofResult>;
}

export type SubstrateSccpWitnessProvider =
  SccpWitnessProviderResolverOption<SubstrateSccpProofRequestInput>;

export interface SubstrateSccpProveResult {
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  proofBase64?: string;
  proof_base64?: string;
  backend?: typeof SCCP_SUBSTRATE_RUNTIME_PROOF_BACKEND_V1;
  requestHash?: string;
  request_hash?: string;
  envelopeHash?: string;
  envelope_hash?: string;
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
  statementHash?: string;
  statement_hash?: string;
  destinationBindingHash?: string;
  destination_binding_hash?: string;
}

export type SubstrateSccpProveFn = (
  request: SubstrateSccpProofRequest,
  options?: Record<string, unknown>,
) => SubstrateSccpProveResult | Promise<SubstrateSccpProveResult>;

export type SubstrateSccpProverOptions = SccpProverWitnessProviderOption<
  SubstrateSccpWitnessProvider,
  SubstrateSccpProofRequestInput
> &
  SccpProverProveOption<SubstrateSccpProveFn>;

export class SubstrateSccpProver {
  constructor(options?: SubstrateSccpProverOptions);
  buildRequest(
    input: SubstrateSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<SubstrateSccpProofRequest>;
  prove(
    input: SubstrateSccpProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<SubstrateSccpProofResult>;
}

export type SolanaSccpWitnessProvider =
  SccpWitnessProviderResolverOption<SolanaSccpWitnessInput>;

export interface SolanaSccpProveResult {
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  proofBase64?: string;
  proof_base64?: string;
  backend?: typeof SCCP_SOLANA_RECURSIVE_PROOF_BACKEND_V1;
  publicInputs?: SolanaSccpProofPublicInputsInput;
  public_inputs?: SolanaSccpProofPublicInputsInput;
  sourceStateVerifierId?: string;
  source_state_verifier_id?: string;
  sourceStateVerifierHash?: string;
  source_state_verifier_hash?: string;
  proofContext?: SolanaSccpProofContextInput;
  proof_context?: SolanaSccpProofContextInput;
  sourceAdapterDeploymentBinding?: SccpSourceAdapterDeploymentBindingInput;
  source_adapter_deployment_binding?: SccpSourceAdapterDeploymentBindingInput;
  witnessHash?: string;
  witness_hash?: string;
  proofContextHash?: string;
  proof_context_hash?: string;
  sourceAdapterDeploymentBindingHash?: string;
  source_adapter_deployment_binding_hash?: string;
  envelopeHash?: string;
  envelope_hash?: string;
}

export type SolanaSccpProveFn = (
  request: SolanaSccpProofRequest,
  options?: Record<string, unknown>,
) => SolanaSccpProveResult | Promise<SolanaSccpProveResult>;

export type SolanaSccpProverOptions = SccpProverWitnessProviderOption<
  SolanaSccpWitnessProvider,
  SolanaSccpWitnessInput
> &
  SccpProverProveOption<SolanaSccpProveFn>;

export class SolanaSccpProver {
  constructor(options?: SolanaSccpProverOptions);
  buildRequest(
    input: SolanaSccpWitnessInput,
    options?: Record<string, unknown>,
  ): Promise<SolanaSccpProofRequest>;
  prove(
    input: SolanaSccpWitnessInput,
    options?: Record<string, unknown>,
  ): Promise<SolanaSccpProofResult>;
}

export function isSupportedSccpDomain(domainId: number): boolean;
export function canonicalSccpMessageTransparentPublicInputsBytes(
  input: SccpMessageTransparentPublicInputsInput,
): Uint8Array;
export function sccpMessageTransparentPublicInputAbiWords(
  input: SccpMessageTransparentPublicInputsInput,
): Uint8Array[];
export function sccpSubmitMessageProofCallData(
  proofBytes: BinaryLike,
  publicInputs: SccpMessageTransparentPublicInputsInput,
  statementHash: string,
  sourceDomain?: number,
): Uint8Array;
export function sccpTonSubmissionQueryId(
  publicInputs: SccpMessageTransparentPublicInputsInput,
): bigint;
export function canonicalSccpTonSubmissionMetadataBytes(
  input: TonSccpMessageBodyInput & { manifest: TonSccpManifestInput },
): Uint8Array;
export function buildSccpTonMessageBodyBoc(
  input: TonSccpMessageBodyInput,
): Uint8Array;
export function tonConfigValidatorSetPayloadFromProofBoc(
  input: BinaryLike,
): Uint8Array | null;
export function tonConfigValidatorSetPayloadHashFromProofBoc(
  input: BinaryLike,
): string | null;
export function tonHashmapECellRefValueHash(
  input: BinaryLike,
  key: BinaryLike,
  keyBitLen: number,
): string | null;
export function tonHashmapEProofRootHash(input: BinaryLike): string;
export function tonShardAccountsLastTransaction(
  input: BinaryLike,
  key: BinaryLike,
  keyBitLen: number,
): { hash: string; lt: bigint } | null;
export function tonShardAccountsLastTransactionHash(
  input: BinaryLike,
  key: BinaryLike,
  keyBitLen: number,
): string | null;
export function tonShardStateProofRootHash(input: BinaryLike): string;
export function tonShardStateAccountsRootHash(input: BinaryLike): string;
export function tonBocRootHashes(input: BinaryLike): string[];
export function tonBocSingleRootHash(input: BinaryLike): string;
export function buildTonSccpProofRequest(
  input: TonSccpProofRequestInput,
): TonSccpProofRequest;
export function wrapTonSccpProofResult(
  proofBytes: BinaryLike,
  request: TonSccpProofRequest,
): TonSccpProofResult;
export function buildTonSccpSubmission(
  input: TonSccpMessageBodyInput,
): TonSccpSubmission;
export function buildEvmSccpProofRequest(
  input: EvmSccpProofRequestInput,
): EvmSccpProofRequest;
export function wrapEvmSccpProofResult(
  proofBytes: BinaryLike,
  request: EvmSccpProofRequest,
): EvmSccpProofResult;
export function buildEvmSccpSubmission(
  input: EvmSccpSubmissionInput,
): EvmSccpSubmission;
export function buildEvmSccpBridgeProofSubmitPayload(
  input: EvmSccpBridgeProofSubmitPayloadInput,
): ToriiBridgeProofSubmitPayload;
export function buildBscMainnetSccpDestinationProofRequest(
  input: BscMainnetSccpProofRequestInput,
): BscMainnetSccpProofRequest;
export function wrapBscMainnetSccpDestinationProofResult(
  proofBytes: BinaryLike,
  request: BscMainnetSccpProofRequest,
): BscMainnetSccpProofResult;
export function buildBscMainnetSccpDestinationSubmission(
  input: BscMainnetSccpSubmissionInput,
): BscMainnetSccpSubmission;
export function buildBscTestnetSccpDestinationProofRequest(
  input: BscTestnetSccpProofRequestInput,
): BscTestnetSccpProofRequest;
export function wrapBscTestnetSccpDestinationProofResult(
  proofBytes: BinaryLike,
  request: BscTestnetSccpProofRequest,
): BscTestnetSccpProofResult;
export function buildBscTestnetSccpDestinationSubmission(
  input: BscTestnetSccpSubmissionInput,
): BscTestnetSccpSubmission;
export function buildTronSccpProofRequest(
  input: TronSccpProofRequestInput,
): TronSccpProofRequest;
export function wrapTronSccpProofResult(
  proofBytes: BinaryLike,
  request: TronSccpProofRequest,
): TronSccpProofResult;
export function buildTronSccpSubmission(
  input: TronSccpSubmissionInput,
): TronSccpSubmission;
export function buildTronSccpBridgeProofSubmitPayload(
  input: TronSccpBridgeProofSubmitPayloadInput,
): ToriiBridgeProofSubmitPayload;
export function buildSubstrateSccpProofRequest(
  input: SubstrateSccpProofRequestInput,
): SubstrateSccpProofRequest;
export function wrapSubstrateSccpProofResult(
  proofBytes: BinaryLike,
  request: SubstrateSccpProofRequest,
): SubstrateSccpProofResult;
export function buildSubstrateSccpSubmission(
  input: SubstrateSccpSubmissionInput,
): SubstrateSccpSubmission;
export function canonicalEvmSccpReceiptProofBytes(
  input: EvmSccpReceiptProofInput,
): Uint8Array;
export function evmSccpReceiptProofHash(
  input: EvmSccpReceiptProofInput,
): string;
export function canonicalEvmReceiptRlp(
  receipt: Record<string, unknown>,
): Uint8Array;
export function evmReceiptTrieKey(
  transactionIndex: number | bigint | string,
): string;
export function buildEvmReceiptTrieProofFromReceipts(
  receipts: readonly Record<string, unknown>[],
  options: {
    transactionIndex?: number | bigint | string;
    transaction_index?: number | bigint | string;
  },
): {
  readonly receiptsRoot: string;
  readonly receiptRlp: string;
  readonly receiptTrieKey: string;
  readonly receiptTrieProofNodes: readonly Uint8Array[];
};
export function evmSccpSourceEventTopic(): string;
export function canonicalEvmReceiptRootMptValue(
  receiptRoot: string,
): Uint8Array;
export function canonicalEthSyncCommitteePayloadBytes(
  input: EthSyncCommitteePayloadInput,
): Uint8Array;
export function ethSyncCommitteeHash(
  input: EthSyncCommitteePayloadInput,
): string;
export function ethSyncCommitteeHashFromPayload(
  input:
    | EthSyncCommitteePayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function ethSyncCommitteePayloadHash(
  input:
    | EthSyncCommitteePayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function ethExecutionPayloadHeaderRootFromRlp(
  headerRlp: string | Uint8Array | ArrayBuffer | ArrayBufferView | number[],
): string;
export function ethBeaconBodyRootFromExecutionPayloadBranch(
  executionPayloadHeaderRoot: string,
  executionPayloadBranch: readonly (
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[]
  )[],
): string;
export function ethBeaconBlockHeaderRoot(
  input: EthBeaconBlockHeaderRootInput,
): string;
export const SCCP_ETH_MAINNET_SLOTS_PER_EPOCH: 32;
export const SCCP_ETH_MAINNET_EPOCHS_PER_SYNC_COMMITTEE_PERIOD: 256;
export const SCCP_ETH_MAINNET_SLOTS_PER_SYNC_COMMITTEE_PERIOD: 8192;
export function ethMainnetSyncCommitteePeriodForSlot(
  slot: string | number | bigint,
): bigint;
export function canonicalEthSyncCommitteeTransitionMessageBytes(
  input: EthSyncCommitteeTransitionMessageInput,
): Uint8Array;
export function ethSyncCommitteeTransitionMessageHash(
  input: EthSyncCommitteeTransitionMessageInput,
): string;
export function canonicalEthSyncCommitteeTransitionSignatureBytes(
  input: EthSyncCommitteeTransitionSignatureInput,
): Uint8Array;
export function ethSyncCommitteeTransitionSignatureHash(
  input: EthSyncCommitteeTransitionSignatureInput,
): string;
export function canonicalBscSccpReceiptProofBytes(
  input: BscSccpReceiptProofInput,
): Uint8Array;
export function bscSccpReceiptProofHash(
  input: BscSccpReceiptProofInput,
): string;
export function canonicalBscValidatorSetPayloadBytes(
  input: BscValidatorSetPayloadInput,
): Uint8Array;
export function bscValidatorSetPayloadHash(
  input:
    | BscValidatorSetPayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function bscValidatorSetHashFromPayload(
  input:
    | BscValidatorSetPayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function bscValidatorSetStorageValueHash(input: BinaryLike): string;
export function canonicalBscValidatorSetMetadataProofBytes(
  input: BscValidatorSetMetadataProofInput,
): Uint8Array;
export function bscValidatorSetMetadataProofHash(
  input: BscValidatorSetMetadataProofInput,
): string;
export function canonicalBscValidatorSetTransitionMessageBytes(
  input: BscValidatorSetTransitionMessageInput,
): Uint8Array;
export function bscValidatorSetTransitionMessageHash(
  input: BscValidatorSetTransitionMessageInput,
): string;
export function canonicalBscCommitMessageBytes(
  input: BscCommitMessageInput,
): Uint8Array;
export function bscCommitMessageHash(input: BscCommitMessageInput): string;
export function canonicalBscCommitSealBytes(
  input: BscCommitSealInput,
): Uint8Array;
export function bscCommitSealHash(input: BscCommitSealInput): string;
export function bscValidatorSetPayloadFromParliaExtra(
  extraData: BinaryLike,
): Uint8Array;
export function bscValidatorSetPayloadFromHeaderRlp(
  headerRlp: BinaryLike,
): Uint8Array;
export function canonicalSolanaSccpMessageProofBytes(
  input: SolanaSccpMessageProofInput,
): Uint8Array;
export function solanaSccpMessageProofHash(
  input: SolanaSccpMessageProofInput,
): string;
export function canonicalSolanaSccpTransactionStatusLeafBytes(
  input: SolanaSccpMessageProofInput,
): Uint8Array;
export function solanaSccpTransactionStatusLeafHash(
  input: SolanaSccpMessageProofInput,
): string;
export function solanaSccpTransactionStatusRootFromBranch(
  input: SolanaSccpMessageProofInput,
): string;
export function canonicalTonSccpShardProofBytes(
  input: TonSccpShardProofInput,
): Uint8Array;
export function tonSccpShardProofHash(input: TonSccpShardProofInput): string;
export function canonicalTonShardStateProofPublicInputsBytes(
  input: TonShardStateProofRequestInput,
): Uint8Array;
export function tonShardStateProofPublicInputsHash(
  input: TonShardStateProofRequestInput,
): string;
export function canonicalTonShardStateWitnessCommitmentBytes(
  input: TonShardStateProofRequestInput,
): Uint8Array;
export function canonicalTonShardStateVerificationContextBytes(
  input: TonShardStateProofRequestInput,
): Uint8Array;
export function tonShardStatePublicInputColumns(
  input: TonShardStateProofRequestInput,
): string[][];
export function tonShardStateOpenVerifySchemaDescriptor(
  input: TonShardStateProofRequestInput,
): Uint8Array;
export function buildTonShardStateProofRequest(
  input: TonShardStateProofRequestInput,
): TonShardStateProofRequest;
export function tonSccpShardStateVerificationProofHash(
  input: TonSccpSourceStateVerificationProof,
): string;
export function wrapTonSccpSourceStateVerificationProof(
  proofBytes: BinaryLike | number[],
  request: TonShardStateProofRequest | TonSccpFullLightClientAuditProofRequest,
): WrappedTonSccpSourceStateVerificationProof;
export function canonicalTonSccpFullLightClientAuditStatementBytes(
  input: TonSccpFullLightClientAuditProofRequestInput,
  role: TonSccpFullLightClientAuditRole,
): Uint8Array;
export function tonSccpFullLightClientAuditStatementHash(
  input: TonSccpFullLightClientAuditProofRequestInput,
  role: TonSccpFullLightClientAuditRole,
): string;
export function tonSccpFullLightClientAuditPublicInputColumns(
  input: TonSccpFullLightClientAuditProofRequestInput,
  role: TonSccpFullLightClientAuditRole,
): string[][];
export function tonSccpFullLightClientAuditOpenVerifySchemaDescriptor(
  input: TonSccpFullLightClientAuditProofRequestInput,
  role: TonSccpFullLightClientAuditRole,
): Uint8Array;
export function buildTonSccpFullLightClientAuditProofRequest(
  input: TonSccpFullLightClientAuditProofRequestInput,
  role: TonSccpFullLightClientAuditRole,
): TonSccpFullLightClientAuditProofRequest;
export function buildTonSccpMasterchainConfigProofRequest(
  input: TonSccpFullLightClientAuditProofRequestInput,
): TonSccpFullLightClientAuditProofRequest;
export function buildTonSccpValidatorSetTransitionProofRequest(
  input: TonSccpFullLightClientAuditProofRequestInput,
): TonSccpFullLightClientAuditProofRequest;
export function buildTonSccpShardAccountsDictionaryProofRequest(
  input: TonSccpFullLightClientAuditProofRequestInput,
): TonSccpFullLightClientAuditProofRequest;
export function buildTonSccpFullLightClientAuditProofRequests(
  input: TonSccpFullLightClientAuditProofRequestInput,
): Readonly<{
  readonly masterchainConfig: TonSccpFullLightClientAuditProofRequest;
  readonly validatorSetTransition: TonSccpFullLightClientAuditProofRequest;
  readonly shardAccountsDictionary: TonSccpFullLightClientAuditProofRequest;
}>;
export type TonSccpSourceStateProofRequest =
  | TonShardStateProofRequest
  | TonSccpFullLightClientAuditProofRequest;

export type TonSccpSourceStateProverResultObject =
  SccpSourceStateProverResultProofMetadata<
    | typeof SCCP_TON_SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1
    | typeof SCCP_TON_SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1
  > & {
    parameterSet?: "fastpq-lane-balanced";
    parameter_set?: "fastpq-lane-balanced";
    sourceDomain?: typeof SCCP_DOMAIN_TON;
    source_domain?: typeof SCCP_DOMAIN_TON;
    masterchainSeqno?: string | number | bigint;
    masterchain_seqno?: string | number | bigint;
    shardSeqno?: string | number | bigint;
    shard_seqno?: string | number | bigint;
    sourceStateVerifierId?: string;
    source_state_verifier_id?: string;
    sourceStateVerifierHash?: string;
    source_state_verifier_hash?: string;
    shardStateProofPublicInputsHash?: string;
    shard_state_proof_public_inputs_hash?: string;
    shardStateVerificationProofHash?: string;
    shard_state_verification_proof_hash?: string;
    role?: TonSccpFullLightClientAuditRole;
    audit_role?: TonSccpFullLightClientAuditRole;
    roleCode?: 1 | 2 | 3 | string | number | bigint;
    role_code?: 1 | 2 | 3 | string | number | bigint;
    verifierId?: string;
    verifier_id?: string;
    verifierHash?: string;
    verifier_hash?: string;
    sourceVerifierMaterialHash?: string;
    source_verifier_material_hash?: string;
    sourceAdapterDeploymentHash?: string;
    source_adapter_deployment_hash?: string;
    fullLightClientGateHash?: string;
    full_light_client_gate_hash?: string;
    auditStatementHash?: string;
    audit_statement_hash?: string;
    publicInputColumns?: ReadonlyArray<ReadonlyArray<string>>;
    public_input_columns?: ReadonlyArray<ReadonlyArray<string>>;
    fastpqPublicInputs?: SccpSourceStateFastpqPublicInputsResultMetadata;
    fastpq_public_inputs?: SccpSourceStateFastpqPublicInputsResultMetadata;
    fastpqTransitions?: ReadonlyArray<SccpSourceStateFastpqTransitionResultMetadata>;
    fastpq_transitions?: ReadonlyArray<SccpSourceStateFastpqTransitionResultMetadata>;
    statementBytes?: BinaryLike | number[];
    statement_bytes?: BinaryLike | number[];
    witnessCommitmentBytes?: BinaryLike | number[];
    witness_commitment_bytes?: BinaryLike | number[];
    verificationContextBytes?: BinaryLike | number[];
    verification_context_bytes?: BinaryLike | number[];
    schemaDescriptor?: BinaryLike | number[];
    schema_descriptor?: BinaryLike | number[];
  };

export type TonSccpSourceStateProveResult =
  | BinaryLike
  | number[]
  | TonSccpSourceStateProverResultObject;
export type TonSccpSourceStateProveFn = (
  request: TonSccpSourceStateProofRequest,
  options?: Record<string, unknown>,
) => TonSccpSourceStateProveResult | Promise<TonSccpSourceStateProveResult>;
export type TonSccpSourceStateProverOptions =
  SccpProverProveOption<TonSccpSourceStateProveFn>;
export class TonSccpSourceStateProver {
  constructor(options?: TonSccpSourceStateProverOptions);
  proveRequest(
    request: TonSccpSourceStateProofRequest,
    options?: Record<string, unknown>,
  ): Promise<WrappedTonSccpSourceStateVerificationProof>;
  proveShardState(
    input: TonShardStateProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<WrappedTonSccpSourceStateVerificationProof>;
  proveFullLightClientAudit(
    input: TonSccpFullLightClientAuditProofRequestInput,
    options?: Record<string, unknown>,
  ): Promise<
    Readonly<{
      readonly masterchainConfig: WrappedTonSccpSourceStateVerificationProof;
      readonly validatorSetTransition: WrappedTonSccpSourceStateVerificationProof;
      readonly shardAccountsDictionary: WrappedTonSccpSourceStateVerificationProof;
    }>
  >;
}
export function canonicalTonValidatorSetBytes(
  input: TonValidatorSetPayloadInput,
): Uint8Array;
export function canonicalTonValidatorSetPayloadBytes(
  input: TonValidatorSetPayloadInput,
): Uint8Array;
export function tonValidatorSetHash(input: TonValidatorSetPayloadInput): string;
export function tonValidatorSetHashFromPayload(
  input:
    | TonValidatorSetPayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function tonValidatorSetPayloadHash(
  input:
    | TonValidatorSetPayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function canonicalTonMasterchainConfigLeafBytes(
  input: TonMasterchainConfigLeafInput,
): Uint8Array;
export function tonMasterchainConfigLeafHash(
  input: TonMasterchainConfigLeafInput,
): string;
export function canonicalTonMasterchainConfigProofBytes(
  input: TonMasterchainConfigProofInput,
): Uint8Array;
export function tonMasterchainConfigProofHash(
  input: TonMasterchainConfigProofInput,
): string;
export function canonicalTonMasterchainBlockMessageBytes(
  input: TonMasterchainBlockMessageInput,
): Uint8Array;
export function tonMasterchainBlockMessageHash(
  input: TonMasterchainBlockMessageInput,
): string;
export function canonicalTonMasterchainValidatorSignaturesBytes(
  input: TonValidatorSignatureProofInput,
): Uint8Array;
export function tonMasterchainValidatorSignaturesHash(
  input: TonValidatorSignatureProofInput,
): string;
export function canonicalTonValidatorSetTransitionMessageBytes(
  input: TonValidatorSetTransitionMessageInput,
): Uint8Array;
export function tonValidatorSetTransitionMessageHash(
  input: TonValidatorSetTransitionMessageInput,
): string;
export function canonicalTonValidatorSetTransitionSignatureBytes(
  input: TonValidatorSetTransitionSignatureInput,
): Uint8Array;
export function tonValidatorSetTransitionSignatureHash(
  input: TonValidatorSetTransitionSignatureInput,
): string;
export function canonicalTronReceiptRootMptValue(
  receiptRoot: string,
): Uint8Array;
export function canonicalTronSccpReceiptProofBytes(
  input: TronSccpReceiptProofInput,
): Uint8Array;
export function tronSccpReceiptProofHash(
  input: TronSccpReceiptProofInput,
): string;
export function canonicalTronSccpReceiptStateProofBytes(
  input: TronSccpReceiptStateProofInput,
): Uint8Array;
export function tronSccpReceiptStateProofHash(
  input: TronSccpReceiptStateProofInput,
): string;
export function tronSccpSourceMessageCallData(
  sourceDomain: number,
  targetDomain: number,
  sourceEventDigest: string,
): Uint8Array;
export interface TronTriggerSmartContractRawDataParseOptions {
  readonly expectedOwnerAddress?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  readonly expectedContractAddress?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
  readonly expectedCallData?:
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[];
}
export interface TronTriggerSmartContractRawDataView {
  readonly rawDataHash: string;
  readonly ownerAddress: string;
  readonly ownerAddress20: string;
  readonly contractAddress: string;
  readonly contractAddress20: string;
  readonly callData: string;
  readonly refBlockNum: string | null;
  readonly timestampMs: string;
  readonly expirationMs: string;
  readonly feeLimit: string;
}
export function parseTronTriggerSmartContractRawData(
  rawData: string | Uint8Array | ArrayBuffer | ArrayBufferView | number[],
  options?: TronTriggerSmartContractRawDataParseOptions,
): TronTriggerSmartContractRawDataView;
export function canonicalTronSccpTransactionSourceProofBytes(
  input: TronSccpTransactionSourceProofInput,
): Uint8Array;
export function tronSccpTransactionSourceProofHash(
  input: TronSccpTransactionSourceProofInput,
): string;
export function canonicalTronRawBlockHeaderBytes(
  input: TronRawBlockHeaderInput,
): Uint8Array;
export function tronRawBlockHeaderHash(
  rawData: string | Uint8Array | ArrayBuffer | ArrayBufferView | number[],
): string;
export function tronBlockIdFromRawDataHash(
  number: string | number | bigint,
  rawDataHash: string,
): string;
export function canonicalTronSolidBlockHeaderProofBytes(
  input: TronSolidBlockHeaderProofInput,
): Uint8Array;
export function tronSolidBlockHeaderProofHash(
  input: TronSolidBlockHeaderProofInput,
): string;
export function canonicalTronWitnessSchedulePayloadBytes(
  input: TronWitnessSchedulePayloadInput,
): Uint8Array;
export function tronWitnessSchedulePayloadHash(
  input:
    | TronWitnessSchedulePayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function tronWitnessScheduleHashFromPayload(
  input:
    | TronWitnessSchedulePayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function canonicalTronSolidBlockMessageBytes(
  input: TronSolidBlockMessageInput,
): Uint8Array;
export function tronSolidBlockMessageHash(
  input: TronSolidBlockMessageInput,
): string;
export function canonicalTronWitnessSealBytes(
  input: TronWitnessSealInput,
): Uint8Array;
export function tronWitnessSealHash(input: TronWitnessSealInput): string;
export function canonicalTronWitnessScheduleTransitionMessageBytes(
  input: TronWitnessScheduleTransitionMessageInput,
): Uint8Array;
export function tronWitnessScheduleTransitionMessageHash(
  input: TronWitnessScheduleTransitionMessageInput,
): string;
export function canonicalTronWitnessScheduleTransitionSealBytes(
  input: TronWitnessScheduleTransitionSealInput,
): Uint8Array;
export function tronWitnessScheduleTransitionSealHash(
  input: TronWitnessScheduleTransitionSealInput,
): string;
export function canonicalSubstrateSccpStorageProofBytes(
  input: SubstrateSccpStorageProofInput,
): Uint8Array;
export function substrateSccpStorageProofHash(
  input: SubstrateSccpStorageProofInput,
): string;
export function canonicalSubstrateSccpRuntimeStorageVerificationStatementBytes(
  input: SubstrateSccpRuntimeStorageProofRequestInput,
): Uint8Array;
export function substrateSccpRuntimeStorageProofPublicInputsHash(
  input: SubstrateSccpRuntimeStorageProofRequestInput,
): string;
export function canonicalSubstrateSccpRuntimeStorageVerificationContextBytes(
  input: SubstrateSccpRuntimeStorageProofRequestInput,
): Uint8Array;
export function substrateSccpRuntimeStoragePublicInputColumns(
  input: SubstrateSccpRuntimeStorageProofRequestInput,
): string[][];
export function substrateSccpRuntimeStorageOpenVerifySchemaDescriptor(
  input:
    | SubstrateSccpRuntimeStorageProofRequestInput
    | number
    | string
    | bigint,
): Uint8Array;
export function buildSubstrateSccpRuntimeStorageProofRequest(
  input: SubstrateSccpRuntimeStorageProofRequestInput,
): SubstrateSccpRuntimeStorageProofRequest;
export function canonicalSubstrateAuthoritySetPayloadBytes(
  input: SubstrateAuthoritySetPayloadInput,
): Uint8Array;
export function substrateAuthoritySetPayloadHash(
  input:
    | SubstrateAuthoritySetPayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function substrateAuthoritySetHashFromPayload(
  input:
    | SubstrateAuthoritySetPayloadInput
    | string
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | number[],
): string;
export function canonicalSubstrateAuthoritySetTransitionMessageBytes(
  input: SubstrateAuthoritySetTransitionMessageInput,
): Uint8Array;
export function substrateAuthoritySetTransitionMessageHash(
  input: SubstrateAuthoritySetTransitionMessageInput,
): string;
export function canonicalSubstrateAuthoritySetTransitionJustificationBytes(
  input: SubstrateAuthoritySetTransitionJustificationInput,
): Uint8Array;
export function substrateAuthoritySetTransitionJustificationHash(
  input: SubstrateAuthoritySetTransitionJustificationInput,
): string;
export function solanaSccpMainnetEpochForSlot(
  slot: string | number | bigint,
): bigint;
export function canonicalSolanaSccpEpochStakeRootBytes(
  input: SolanaSccpEpochStakeRootInput,
): Uint8Array;
export function solanaSccpEpochStakeRoot(
  input: SolanaSccpEpochStakeRootInput,
): string;
export function canonicalSolanaSccpStakeActivationBytes(
  input: SolanaSccpStakeActivationInput,
): Uint8Array;
export function solanaSccpStakeActivationHash(
  input: SolanaSccpStakeActivationInput,
): string;
export function canonicalSolanaSccpAccountOpeningBytes(
  input: SolanaSccpAccountOpeningInput,
): Uint8Array;
export function solanaSccpAccountOpeningHash(
  input: SolanaSccpAccountOpeningInput,
): string;
export function solanaSccpAccountRawDataHash(rawData: BinaryLike): string;
export function canonicalSolanaSccpAccountInclusionLeafBytes(
  input: SolanaSccpAccountInclusionLeafInput,
): Uint8Array;
export function solanaSccpAccountInclusionLeafHash(
  input: SolanaSccpAccountInclusionLeafInput,
): string;
export function canonicalSolanaSccpAccountInclusionNodeBytes(
  left: BinaryLike,
  right: BinaryLike,
): Uint8Array;
export function solanaSccpAccountInclusionNodeHash(
  left: BinaryLike,
  right: BinaryLike,
): string;
export function solanaSccpAccountInclusionRootFromBranch(
  leaf: BinaryLike,
  siblings?: readonly BinaryLike[],
): string;
export function solanaSccpAccountInclusionRootAndBranches(
  leaves: readonly BinaryLike[],
): SolanaSccpAccountInclusionRootAndBranches;
export function canonicalSolanaSccpVoteAccountDataBytes(
  input: SolanaSccpVoteAccountDataInput,
): Uint8Array;
export function solanaSccpVoteAccountDataHash(
  input: SolanaSccpVoteAccountDataInput,
): string;
export function solanaSccpVoteAccountDataFromRawVoteState(
  rawData: BinaryLike,
  epoch: string | number | bigint,
  voteAccountAddress: BinaryLike,
): SolanaSccpParsedVoteStateAccountData;
export function solanaSccpVoteAccountDataHashFromRawVoteState(
  rawData: BinaryLike,
  epoch: string | number | bigint,
  voteAccountAddress: BinaryLike,
): string;
export function solanaSccpVoteAccountDataFromRawVoteStateV1OrV3(
  rawData: BinaryLike,
  epoch: string | number | bigint,
  voteAccountAddress: BinaryLike,
): SolanaSccpParsedVoteStateV1OrV3AccountData;
export function solanaSccpVoteAccountDataHashFromRawVoteStateV1OrV3(
  rawData: BinaryLike,
  epoch: string | number | bigint,
  voteAccountAddress: BinaryLike,
): string;
export function canonicalSolanaSccpStakeAccountDataBytes(
  input: SolanaSccpStakeAccountDataInput,
): Uint8Array;
export function solanaSccpStakeAccountDataHash(
  input: SolanaSccpStakeAccountDataInput,
): string;
export function solanaSccpStakeAccountDataFromRawStakeStateV2(
  rawData: BinaryLike,
): SolanaSccpParsedStakeStateV2StakeAccountData;
export function solanaSccpStakeAccountDataHashFromRawStakeStateV2(
  rawData: BinaryLike,
): string;
export function canonicalSolanaSccpStakeAccountStateBytes(
  input: SolanaSccpStakeAccountStateInput,
): Uint8Array;
export function solanaSccpStakeAccountStateHash(
  input: SolanaSccpStakeAccountStateInput,
): string;
export function canonicalSolanaSccpStakeHistorySysvarDataBytes(
  input: SolanaSccpStakeHistorySysvarDataInput,
): Uint8Array;
export function solanaSccpStakeHistorySysvarDataHash(
  input: SolanaSccpStakeHistorySysvarDataInput,
): string;
export function solanaSccpStakeHistorySysvarDataHashFromRawData(
  rawData: BinaryLike,
): string;
export function canonicalSolanaSccpStakeHistoryBytes(
  input: SolanaSccpStakeHistoryInput,
): Uint8Array;
export function solanaSccpStakeHistoryHash(
  input: SolanaSccpStakeHistoryInput,
): string;
export function canonicalSolanaSccpTowerLockoutBytes(
  input: SolanaSccpTowerLockoutInput,
): Uint8Array;
export function solanaSccpTowerLockoutHash(
  input: SolanaSccpTowerLockoutInput,
): string;
export function canonicalSolanaSccpTowerReplayBytes(
  input: SolanaSccpTowerReplayInput,
): Uint8Array;
export function solanaSccpTowerReplayHash(
  input: SolanaSccpTowerReplayInput,
): string;
export function canonicalSolanaSccpBankForkBytes(
  input: SolanaSccpBankForkInput,
): Uint8Array;
export function solanaSccpBankForkHash(input: SolanaSccpBankForkInput): string;
export function canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(
  input: SolanaSccpAccountsLtHashProofPublicInputsInput,
): Uint8Array;
export function solanaSccpAccountsLtHashProofPublicInputsHash(
  input: SolanaSccpAccountsLtHashProofPublicInputsInput,
): string;
export function solanaSccpAgaveBankHash(
  input: SolanaSccpAgaveBankHashInput,
): string;
export function normalizeSolanaSccpWitness(
  input: SolanaSccpWitnessInput,
): SolanaSccpWitness;
export function canonicalSolanaSccpWitnessBytes(
  input: SolanaSccpWitnessInput,
): Uint8Array;
export function normalizeSolanaSccpProofContext(
  input: SolanaSccpProofContextInput,
): SolanaSccpProofContext;
export function canonicalSolanaSccpProofContextBytes(
  input: SolanaSccpProofContextInput,
): Uint8Array;
export function solanaSccpProofContextHash(
  input: SolanaSccpProofContextInput,
): string;
export function normalizeSccpSourceAdapterDeploymentBinding(
  input?: SccpSourceAdapterDeploymentBindingInput,
): SccpSourceAdapterDeploymentBinding;
export function canonicalSccpSourceAdapterDeploymentBindingBytes(
  input: SccpSourceAdapterDeploymentBindingInput,
): Uint8Array;
export function sccpSourceAdapterDeploymentBindingHash(
  input: SccpSourceAdapterDeploymentBindingInput,
): string;
export function sccpDestinationBindingKey(
  input: SccpDestinationBindingDomainInput,
): string;
export function sccpDestinationBindingHash(
  input: SccpDestinationBindingDomainInput,
): string;
export function canonicalSolanaSccpRouteCanaryEvidenceBytes(
  input: SolanaSccpRouteCanaryEvidenceInput,
): Uint8Array;
export function solanaSccpRouteCanaryEvidenceHash(
  input: SolanaSccpRouteCanaryEvidenceInput,
): string;
export function canonicalTonSccpRouteCanaryEvidenceBytes(
  input: TonSccpRouteCanaryEvidenceInput,
): Uint8Array;
export function tonSccpRouteCanaryEvidenceHash(
  input: TonSccpRouteCanaryEvidenceInput,
): string;
export function canonicalTronSccpRouteCanaryEvidenceBytes(
  input: TronSccpRouteCanaryEvidenceInput,
): Uint8Array;
export function tronSccpRouteCanaryEvidenceHash(
  input: TronSccpRouteCanaryEvidenceInput,
): string;
export function evmSccpDestinationBinding(
  input: EvmSccpDestinationBindingInput,
): EvmSccpDestinationBinding;
export function evmSccpDestinationBindingHash(
  input: EvmSccpDestinationBindingInput,
): string;
export function ethereumMainnetSccpDestinationBinding(
  input: EvmSccpDestinationBindingInput,
): EvmSccpDestinationBinding;
export function ethereumMainnetSccpDestinationBindingHash(
  input: EvmSccpDestinationBindingInput,
): string;
export function bscMainnetSccpDestinationBinding(
  input: EvmSccpDestinationBindingInput,
): EvmSccpDestinationBinding;
export function bscMainnetSccpDestinationBindingHash(
  input: EvmSccpDestinationBindingInput,
): string;
export function bscTestnetSccpDestinationBinding(
  input: EvmSccpDestinationBindingInput,
): EvmSccpDestinationBinding;
export function bscTestnetSccpDestinationBindingHash(
  input: EvmSccpDestinationBindingInput,
): string;
export function tronSccpDestinationBinding(
  input: TronSccpDestinationBindingInput,
): TronSccpDestinationBinding;
export function tronSccpDestinationBindingHash(
  input: TronSccpDestinationBindingInput,
): string;

export interface TairaXorRouteHashInput {
  routeId?: string;
  route_id?: string;
  routeIdHash?: string;
  route_id_hash?: string;
  assetKey?: string;
  asset_key?: string;
  assetKeyHash?: string;
  asset_key_hash?: string;
}

export interface TairaXorTransferPayloadHashInput
  extends TairaXorTransferPayloadInput {}

export interface TairaXorBurnSourceEventDigestInput
  extends TairaXorRouteHashInput {
  bridgeAddress?: string | BinaryLike | number[];
  bridge_address?: string | BinaryLike | number[];
  burnerAddress?: string | BinaryLike | number[];
  burner_address?: string | BinaryLike | number[];
  burner?: string | BinaryLike | number[];
  tairaRecipientHash?: string;
  taira_recipient_hash?: string;
  tairaRecipientBytes?: BinaryLike | number[];
  taira_recipient_bytes?: BinaryLike | number[];
  /** Canonical TAIRA I105 account id when provided as text; use tairaRecipientBytes for raw bytes. */
  tairaRecipient?: string | BinaryLike | number[];
  /** Canonical TAIRA I105 account id when provided as text; use taira_recipient_bytes for raw bytes. */
  taira_recipient?: string | BinaryLike | number[];
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
  nonce?: string | number | bigint;
  burnNonce?: string | number | bigint;
  burn_nonce?: string | number | bigint;
}

export interface TairaXorFinalizeFromTairaCallDataInput
  extends Partial<TairaXorTransferPayloadInput> {
  proofBytes?: BinaryLike | number[];
  proof_bytes?: BinaryLike | number[];
  publicInputs?: SccpMessageTransparentPublicInputsInput;
  public_inputs?: SccpMessageTransparentPublicInputsInput;
  statementHash?: string;
  statement_hash?: string;
  /** Canonical SORA->TRON TAIRA XOR transfer payload bytes; optional cleartext fields must match. */
  canonicalPayloadBytes?: BinaryLike | number[];
  /** Canonical SORA->TRON TAIRA XOR transfer payload bytes; optional cleartext fields must match. */
  canonical_payload_bytes?: BinaryLike | number[];
  /** Canonical SORA->TRON TAIRA XOR transfer payload hex; optional cleartext fields must match. */
  canonicalPayloadHex?: string;
  /** Canonical SORA->TRON TAIRA XOR transfer payload hex; optional cleartext fields must match. */
  canonical_payload_hex?: string;
  /** Canonical SORA->TRON TAIRA XOR transfer payload bytes; optional cleartext fields must match. */
  payloadBytes?: BinaryLike | number[];
  /** Canonical SORA->TRON TAIRA XOR transfer payload bytes; optional cleartext fields must match. */
  payload_bytes?: BinaryLike | number[];
}

export interface TairaXorBurnToTairaCallDataInput
  extends TairaXorRouteHashInput {
  tairaRecipientBytes?: BinaryLike | number[];
  taira_recipient_bytes?: BinaryLike | number[];
  /** Canonical TAIRA I105 account id when provided as text; use tairaRecipientBytes for raw bytes. */
  tairaRecipient?: string | BinaryLike | number[];
  /** Canonical TAIRA I105 account id when provided as text; use taira_recipient_bytes for raw bytes. */
  taira_recipient?: string | BinaryLike | number[];
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; aliases such as name@taira are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
}

export interface TairaXorBurnToTairaAccountCallDataInput
  extends TairaXorRouteHashInput {
  /** Canonical TAIRA I105 account id; raw bytes, hex strings, and aliases are rejected. */
  tairaRecipient?: string;
  /** Canonical TAIRA I105 account id; raw bytes, hex strings, and aliases are rejected. */
  taira_recipient?: string;
  /** Canonical TAIRA I105 account id; raw bytes, hex strings, and aliases are rejected. */
  tairaAccountId?: string;
  /** Canonical TAIRA I105 account id; raw bytes, hex strings, and aliases are rejected. */
  taira_account_id?: string;
  amount: string | number | bigint;
}

export interface TairaXorBscBurnToTairaCallDataInput
  extends TairaXorBurnToTairaCallDataInput {}

export interface TairaXorBscBurnToTairaAccountCallDataInput
  extends TairaXorBurnToTairaAccountCallDataInput {}

export function tairaXorRouteIdHash(routeId?: string): string;
export function tairaXorBscRouteIdHash(routeId?: string): string;
export function tairaXorAssetKeyHash(assetKey?: string): string;
export function buildTairaXorTransferPayload(
  input: TairaXorTransferPayloadInput,
): Readonly<SccpTransferPayload>;
export function buildTairaXorBscTransferPayload(
  input: TairaXorBscTransferPayloadInput,
): Readonly<SccpTransferPayload>;
export function buildTairaXorTronToTairaTransferPayload(
  input: TairaXorTronToTairaTransferPayloadInput,
): Readonly<SccpTransferPayload>;
export function buildTairaXorBscToTairaTransferPayload(
  input: TairaXorBscToTairaTransferPayloadInput,
): Readonly<SccpTransferPayload>;
export function buildTairaXorSccpRecordDescriptor(
  input: TairaXorSccpRecordDescriptorInput,
): Readonly<TairaXorSccpRecordDescriptor>;
export function buildTairaXorBscSccpRecordDescriptor(
  input: TairaXorBscSccpRecordDescriptorInput,
): Readonly<TairaXorBscSccpRecordDescriptor>;
export function buildRecordSccpMessageInstructionBytes(
  payloadBytes: BinaryLike | number[],
): Uint8Array;
export function buildTairaXorSccpBurnRecordContractPayload(
  input: TairaXorSccpBurnRecordInput,
): Readonly<TairaXorSccpBurnRecordContractPayload>;
export function buildTairaXorBscSccpBurnRecordContractPayload(
  input: TairaXorBscSccpBurnRecordInput,
): Readonly<TairaXorBscSccpBurnRecordContractPayload>;
export function buildTairaXorSccpBurnRecordZkIvmRequest(
  input: TairaXorSccpBurnRecordZkIvmRequestInput,
): Readonly<TairaXorSccpBurnRecordZkIvmRequest>;
export function buildTairaXorBscSccpBurnRecordZkIvmRequest(
  input: TairaXorBscSccpBurnRecordZkIvmRequestInput,
): Readonly<TairaXorBscSccpBurnRecordZkIvmRequest>;
export function tairaXorCanonicalTransferPayloadBytes(
  input: TairaXorTransferPayloadInput,
): Uint8Array;
export function tairaXorTransferMessageId(
  input: TairaXorTransferPayloadInput,
  options?: { prefix?: boolean },
): string;
export function tairaXorBscCanonicalTransferPayloadBytes(
  input: TairaXorBscTransferPayloadInput,
): Uint8Array;
export function tairaXorBscTransferMessageId(
  input: TairaXorBscTransferPayloadInput,
  options?: { prefix?: boolean },
): string;
export function tairaXorBscToTairaCanonicalTransferPayloadBytes(
  input: TairaXorBscToTairaTransferPayloadInput,
): Uint8Array;
export function tairaXorBscToTairaTransferMessageId(
  input: TairaXorBscToTairaTransferPayloadInput,
  options?: { prefix?: boolean },
): string;
export function tairaXorTronToTairaCanonicalTransferPayloadBytes(
  input: TairaXorTronToTairaTransferPayloadInput,
): Uint8Array;
export function tairaXorTronToTairaTransferMessageId(
  input: TairaXorTronToTairaTransferPayloadInput,
  options?: { prefix?: boolean },
): string;
export function tairaXorTransferPayloadHash(
  input: TairaXorTransferPayloadHashInput,
  options?: { prefix?: boolean },
): string;
export function tairaXorBscToTairaTransferPayloadHash(
  input: TairaXorBscToTairaTransferPayloadInput,
  options?: { prefix?: boolean },
): string;
export function tairaXorBurnSourceEventDigest(
  input: TairaXorBurnSourceEventDigestInput,
): string;
export function tairaXorBscBurnSourceEventDigest(
  input: TairaXorBurnSourceEventDigestInput,
): string;
export function tairaXorFinalizeFromTairaCallData(
  input: TairaXorFinalizeFromTairaCallDataInput,
): string;
export function tairaXorBurnToTairaCallData(
  input: TairaXorBurnToTairaCallDataInput,
): string;
export function tairaXorBurnToTairaAccountCallData(
  input: TairaXorBurnToTairaAccountCallDataInput,
): string;
export function tairaXorBscBurnToTairaCallData(
  input: TairaXorBscBurnToTairaCallDataInput,
): string;
export function tairaXorBscBurnToTairaAccountCallData(
  input: TairaXorBscBurnToTairaAccountCallDataInput,
): string;
export function isTairaXorTronBurnStartedEventName(value: unknown): boolean;
export function bindTairaXorTronBurnStartedEvent(
  input: TairaXorTronBurnStartedEventInput,
): Readonly<TairaXorTronBoundBurnStartedEvent>;
export function bindTairaXorTronToTairaSourceProofPackage(
  input: TairaXorTronToTairaSourceProofPackageInput,
): Readonly<TairaXorTronToTairaBoundSourceProofPackage>;
export function bindTairaXorBscToTairaSourceProofPackage(
  input: TairaXorBscToTairaSourceProofPackageInput,
): Readonly<TairaXorBscToTairaBoundSourceProofPackage>;
export function normalizeSccpSourceVerifierMaterial(
  input: SccpSourceVerifierMaterialInput,
): SccpSourceVerifierMaterial;
export function canonicalSccpSourceVerifierMaterialBytes(
  input: SccpSourceVerifierMaterialInput,
): Uint8Array;
export function sccpSourceVerifierMaterialHash(
  input: SccpSourceVerifierMaterialInput,
): string;
export function normalizeSccpSourceAdapterEngineDeployment(
  input: SccpSourceAdapterEngineDeploymentInput,
): SccpSourceAdapterEngineDeployment;
export function canonicalSccpSourceAdapterEngineDeploymentBytes(
  input: SccpSourceAdapterEngineDeploymentInput,
): Uint8Array;
export function sccpSourceAdapterEngineDeploymentHash(
  input: SccpSourceAdapterEngineDeploymentInput,
): string;
export function sccpSolanaFullLightClientGateHash(
  input: SccpSourceAdapterEngineDeploymentInput,
): string;
export function sccpTonFullLightClientGateHash(
  input: SccpSourceAdapterEngineDeploymentInput,
): string;
export function sccpSourceAdapterVerifierVkHash(
  input:
    | SccpDomainIdInput
    | {
        sourceDomain?: SccpDomainIdInput;
        source_domain?: SccpDomainIdInput;
        targetDomain?: SccpDomainIdInput;
        target_domain?: SccpDomainIdInput;
      },
): string;
export function sccpGroth16Bn254PublicSignalWords(
  input: SccpGroth16Bn254PublicSignalsInput,
): string[];
export function buildSolanaSccpProofRequest(
  input: SolanaSccpWitnessInput,
): SolanaSccpProofRequest;
export function wrapSolanaSccpProofResult(
  proofBytes: BinaryLike,
  request: SolanaSccpProofRequest,
): SolanaSccpProofResult;
export function buildSolanaSccpSubmission(
  input: SolanaSccpSubmissionInput,
): SolanaSccpSubmission;
export function canonicalSccpTransferPayloadBytes(
  payload: SccpTransferPayload,
): Uint8Array;
export function canonicalSccpBurnPayloadBytes(
  payload: SccpBurnPayload,
): Uint8Array;
export function canonicalSccpTokenAddPayloadBytes(
  payload: SccpTokenAddPayload,
): Uint8Array;
export function canonicalSccpTokenControlPayloadBytes(
  payload: SccpTokenControlPayload,
): Uint8Array;
export function canonicalSccpTokenMessagePayloadBytes(
  payload: SccpTokenMessagePayload,
): Uint8Array;
export function canonicalSccpGovernancePayloadBytes(
  payload: SccpGovernancePayload,
): Uint8Array;
export function canonicalSccpCommitmentBytes(
  commitment: SccpHubCommitment,
): Uint8Array;
export function canonicalSccpPayloadEnvelopeBytes(
  payload: Record<string, unknown>,
): Uint8Array;
export function canonicalSccpMerkleProofBytes(
  proof: Record<string, unknown>,
): Uint8Array;
export function canonicalSccpMessageProofBundleBytes(
  bundle: Record<string, unknown>,
): Uint8Array;
export function sccpBurnMessageId(
  payload: SccpBurnPayload,
  options?: { prefix?: boolean },
): string;
export function sccpTransferMessageId(
  payload: SccpTransferPayload,
  options?: { prefix?: boolean },
): string;
export function sccpTokenAddMessageId(
  payload: SccpTokenAddPayload,
  options?: { prefix?: boolean },
): string;
export function sccpTokenPauseMessageId(
  payload: SccpTokenControlPayload,
  options?: { prefix?: boolean },
): string;
export function sccpTokenResumeMessageId(
  payload: SccpTokenControlPayload,
  options?: { prefix?: boolean },
): string;
export function sccpTokenMessageId(
  payload: SccpTokenMessagePayload,
  options?: { prefix?: boolean },
): string;
export function sccpGovernanceMessageId(
  payload: SccpGovernancePayload,
  options?: { prefix?: boolean },
): string;
export function sccpTokenMessageTargetDomain(
  payload: SccpTokenMessagePayload,
): number;
export function sccpParliamentCertificateHash(
  certificate: Uint8Array | ArrayBufferView | ArrayBuffer | string,
  options?: { prefix?: boolean },
): string;
export function sccpPayloadHash(
  payload: Uint8Array | ArrayBufferView | ArrayBuffer | string,
  options?: { prefix?: boolean },
): string;
export function sccpCommitmentLeafHash(
  commitment: SccpHubCommitment,
  options?: { prefix?: boolean },
): string;
export function sccpMerkleRootFromCommitment(
  commitment: SccpHubCommitment,
  proof: SccpMerkleProof,
  options?: { prefix?: boolean },
): string;
export function validateSccpBurnBundleSurface(
  bundle: SccpBurnBundle,
): SccpBundleSurfaceValidation;
export function validateSccpTokenMessageBundleSurface(
  bundle: SccpTokenMessageBundle,
): SccpBundleSurfaceValidation;
export function validateSccpGovernanceBundleSurface(
  bundle: SccpGovernanceBundle,
): SccpGovernanceBundleSurfaceValidation;

export interface DefiOracleAttestationQuery {
  baseUrl?: string;
  toriiUrl?: string;
  domain: number | string;
  subjectId: number | string;
  status?: number | string;
}

export function queryOracleFeeds(
  baseUrl: string,
  options?: Record<string, string | number | boolean | undefined>,
): Promise<JsonValue>;
export function queryOracleFeedHistory(
  baseUrl: string,
  feedId: string,
  options?: Record<string, string | number | boolean | undefined>,
): Promise<JsonValue>;
export function getLatestDefiOracleAttestation(
  query: DefiOracleAttestationQuery,
): Promise<JsonValue>;

export interface BuildPacs008Options {
  messageId: string;
  creationDateTime: string | Date;
  instructionId: string;
  endToEndId?: string;
  transactionId?: string;
  settlementDate?: string | Date;
  amount: IsoBridgeAmount;
  instigatingAgent: IsoBridgeAgent;
  instructedAgent: IsoBridgeAgent;
  debtorAgent?: IsoBridgeAgent;
  creditorAgent?: IsoBridgeAgent;
  debtor?: IsoBridgeParty;
  creditor?: IsoBridgeParty;
  debtorAccount?: IsoBridgeAccount;
  creditorAccount?: IsoBridgeAccount;
  purposeCode?: string;
  remittanceInformation?: string | string[];
  supplementaryData?: Record<string, unknown>;
}

export interface BuildPacs009Options {
  messageId?: string;
  businessMessageId?: string;
  messageDefinitionId?: string;
  creationDateTime: string | Date;
  instructionId: string;
  transactionId?: string;
  settlementDate?: string | Date;
  amount: IsoBridgeAmount;
  instigatingAgent: IsoBridgeAgent;
  instructedAgent: IsoBridgeAgent;
  debtorAgent?: IsoBridgeAgent;
  creditorAgent?: IsoBridgeAgent;
  debtor?: IsoBridgeParty;
  creditor?: IsoBridgeParty;
  debtorAccount?: IsoBridgeAccount;
  creditorAccount?: IsoBridgeAccount;
  purposeCode?: string;
  remittanceInformation?: string | string[];
  supplementaryData?: Record<string, unknown>;
}

export function buildPacs008Message(options: BuildPacs008Options): string;
export function buildPacs009Message(options: BuildPacs009Options): string;
export interface SampleIsoMessageOptions {
  messageSuffix?: string;
  creationDateTime?: string | Date;
  settlementDate?: string | Date;
}
export function buildSamplePacs008Message(
  options?: SampleIsoMessageOptions,
): string;
export function buildSamplePacs009Message(
  options?: SampleIsoMessageOptions,
): string;
export interface CamtReportBalance {
  typeCode?: string;
  amount: IsoBridgeAmount;
  creditDebitIndicator: "CRDT" | "DBIT";
  asOfDateTime?: string | Date;
}
export interface CamtReportEntry {
  amount: IsoBridgeAmount;
  creditDebitIndicator: "CRDT" | "DBIT";
  status?: string;
  bookingDate?: string | Date;
  valueDate?: string | Date;
  reference?: string;
}
export interface CamtReportSummary {
  entryCount?: number;
  sum?: string | number;
  netAmount?: string | number;
  netCreditDebitIndicator?: "CRDT" | "DBIT";
}
export interface BuildCamt052Options {
  messageId: string;
  creationDateTime: string | Date;
  reportId: string;
  pagination?: {
    pageNumber: number;
    lastPage?: boolean;
  };
  sequenceNumber?: number;
  fromDateTime?: string | Date;
  toDateTime?: string | Date;
  account: IsoBridgeAccount;
  accountCurrency?: string;
  balances?: CamtReportBalance[];
  entries?: CamtReportEntry[];
  summary?: CamtReportSummary;
}
export interface BuildCamt056Options {
  assignmentId: string;
  creationDateTime: string | Date;
  cancellationId: string;
  assignerAgent: IsoBridgeAgent;
  assigneeAgent: IsoBridgeAgent;
  debtorAgent: IsoBridgeAgent;
  creditorAgent: IsoBridgeAgent;
  debtor?: IsoBridgeParty;
  debtorAccount?: IsoBridgeAccount;
  creditor?: IsoBridgeParty;
  creditorAccount?: IsoBridgeAccount;
  originalMessageId: string;
  originalMessageNameId: string;
  originalInstructionId?: string;
  originalEndToEndId?: string;
  originalTransactionId?: string;
  originalUetr?: string;
  serviceLevelCode?: string;
  interbankSettlementAmount: IsoBridgeAmount;
  interbankSettlementDate: string | Date;
  caseId?: string;
  caseCreatorName?: string;
}
export type SampleCamtMessageOptions = SampleIsoMessageOptions;
export function buildCamt052Message(options: BuildCamt052Options): string;
export function buildSampleCamt052Message(
  options?: SampleCamtMessageOptions,
): string;
export function buildCamt056Message(options: BuildCamt056Options): string;
export function buildSampleCamt056Message(
  options?: SampleCamtMessageOptions,
): string;

/**
 * Numeric values accepted by builder helpers. Prefer decimal strings for exact
 * quantities; numbers are accepted for convenience and will be serialised
 * through the canonical Norito string representation.
 */
export type NumericLike = string | number | bigint;

/**
 * Metadata payload accepted by transaction helpers. Objects are stringified
 * with deterministic key ordering; strings are passed through unchanged.
 */
export type MetadataLike = object | string | null;

/**
 * Inputs accepted where 32-byte hashes are required. Strings may be canonical
 * `hash:…#…` literals or raw hex; binary inputs are converted automatically.
 */
export type HashLike = string | Buffer | ArrayBuffer | ArrayBufferView;

export type BinaryLike =
  | Buffer
  | ArrayBuffer
  | ArrayBufferView
  | ReadonlyArray<number>
  | string;

export type PrivacyNativeArchiveLike = Buffer | ArrayBuffer | ArrayBufferView;

export type VerifyingKeyIdLike = string | { backend: string; name: string };

export type PrivacyBackendTag =
  | "Halo2IpaPasta"
  | "Halo2Bn254"
  | "Groth16"
  | "Stark"
  | "Unsupported"
  | "Halo2IpaOrchard"
  | "Groth16Bls12377"
  | "FcmpPlusPlusCurveTree"
  | "LatticePcsSis"
  | "MidenStark"
  | "AztecPlonkishPrivateKernel"
  | "PqMaspStarkFri"
  | "AnonymousPgc"
  | "VeRange"
  | "ZkAt"
  | "RecursiveAnonymousAdmission"
  | "VegaExistingCredentialZk"
  | "SilentThresholdAnoncred"
  | "ZkX509"
  | "SisWithHints"
  | "halo2-ipa-pasta"
  | "halo2/ipa"
  | "halo2/pasta/ipa"
  | "halo2-bn254"
  | "groth16"
  | "stark"
  | "stark/fri"
  | "stark/fri/sha256-goldilocks"
  | "unsupported"
  | "halo2-ipa-orchard"
  | "groth16-bls12-377"
  | "fcmp-plus-plus-curve-tree"
  | "lattice-pcs-sis"
  | "miden-stark"
  | "aztec-plonkish-private-kernel"
  | "pq-masp-stark-fri"
  | "anonymous-pgc"
  | "verange"
  | "zkat"
  | "recursive-anonymous-admission"
  | "vega-existing-credential-zk"
  | "silent-threshold-anoncred"
  | "zk-x509"
  | "sis-with-hints"
  | string;

export interface ConfidentialEncryptedPayloadInput {
  version?: number;
  ephemeralPublicKey: BinaryLike;
  nonce: BinaryLike;
  ciphertext: BinaryLike;
}

export interface ProofAttachmentInput {
  backend: string;
  proof: BinaryLike;
  verifyingKeyRef: VerifyingKeyIdLike;
  verifyingKeyCommitment?: BinaryLike | null;
  lanePrivacy?: {
    commitmentId: number;
    merkle?: {
      leaf: BinaryLike;
      leafIndex?: number;
      auditPath?: (BinaryLike | null)[];
    };
  } | null;
}

export interface PrivacyProofEnvelopeInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  publicInputs?: BinaryLike;
  public_inputs?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface ZkAtPolicyCommitmentInput {
  version?: NumericLike;
  policyCommitment?: BinaryLike;
  policy_commitment?: BinaryLike;
  commitment?: BinaryLike;
  policy?: unknown;
  policyBytes?: BinaryLike;
  policy_bytes?: BinaryLike;
  policyJson?: unknown;
  policy_json?: unknown;
  policyEpoch?: NumericLike;
  policy_epoch?: NumericLike;
  domainSeparator?: string;
  domain_separator?: string;
  policySchema?: string;
  policy_schema?: string;
  maxPolicyBytes?: NumericLike;
  max_policy_bytes?: NumericLike;
}

export interface ZkAtPolicyCommitment {
  version: 1;
  policy_commitment: number[];
  policy_epoch: number;
  domain_separator: string;
  policy_schema: string;
  commitment_kind: "external" | "dev-sha256-policy-digest";
  policy_digest: number[] | null;
}

export interface ZkAtAuthenticatorEnvelopeInput
  extends ZkAtPolicyCommitmentInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  txDigest?: BinaryLike;
  tx_digest?: BinaryLike;
  payloadDigest?: BinaryLike;
  payload_digest?: BinaryLike;
  payload?: BinaryLike;
  payloadBytes?: BinaryLike;
  payload_bytes?: BinaryLike;
  payloadJson?: unknown;
  payload_json?: unknown;
  accountId?: string;
  account_id?: string;
  actionClass?: string;
  action_class?: string;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
  maxPayloadBytes?: NumericLike;
  max_payload_bytes?: NumericLike;
}

export interface ZkAtPublicInputs {
  version: 1;
  policy_commitment: string;
  tx_digest: string;
  account_id: string;
  action_class: string;
  domain_separator: string;
  policy_epoch: number;
}

export interface ZkAtDevProofFixture {
  kind: "zkat-dev-fixture-v1";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  public_inputs: ZkAtPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface ZkAtAuthenticatorLocalVerificationInput
  extends ZkAtAuthenticatorEnvelopeInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface ZkAtAuthenticatorLocalVerificationResult {
  ok: true;
  production: false;
  kind: "zkat-dev-fixture-v1";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: ZkAtPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  account_id: string;
  action_class: string;
  policy_epoch: number;
}

export interface ZkAmsAdmissionBatchInput {
  version?: NumericLike;
  issuerRoot?: BinaryLike;
  issuer_root?: BinaryLike;
  admissionBatchRoot?: BinaryLike;
  admission_batch_root?: BinaryLike;
  batchRoot?: BinaryLike;
  batch_root?: BinaryLike;
  admissionNullifiers?: ReadonlyArray<BinaryLike>;
  admission_nullifiers?: ReadonlyArray<BinaryLike>;
  nullifiers?: ReadonlyArray<BinaryLike>;
  anonymousAccountCommitments?: ReadonlyArray<BinaryLike>;
  anonymous_account_commitments?: ReadonlyArray<BinaryLike>;
  accountCommitments?: ReadonlyArray<BinaryLike>;
  account_commitments?: ReadonlyArray<BinaryLike>;
  recursiveProofDigest?: BinaryLike;
  recursive_proof_digest?: BinaryLike;
  recursiveProof?: BinaryLike;
  recursiveProofBytes?: BinaryLike;
  recursive_proof?: BinaryLike;
  recursive_proof_bytes?: BinaryLike;
  domainSeparator?: string;
  domain_separator?: string;
  maxBatchSize?: NumericLike;
  max_batch_size?: NumericLike;
  maxRecursiveProofBytes?: NumericLike;
  max_recursive_proof_bytes?: NumericLike;
}

export interface ZkAmsAdmissionBatch {
  version: 1;
  issuer_root: number[];
  admission_batch_root: number[];
  admission_nullifiers: number[][];
  anonymous_account_commitments: number[][];
  recursive_proof_digest: number[];
  domain_separator: string;
  batch_size: number;
  root_kind: "dev-sha256-admission-batch-root";
}

export interface ZkAmsAdmissionProofEnvelopeInput
  extends ZkAmsAdmissionBatchInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface ZkAmsPublicInputs {
  version: 1;
  issuer_root: string;
  admission_batch_root: string;
  admission_nullifiers: string[];
  anonymous_account_commitments: string[];
  recursive_proof_digest: string;
  domain_separator: string;
}

export interface ZkAmsAdmissionDevProofFixture {
  kind: "zk-ams-dev-fixture-v0";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  batch: Omit<ZkAmsAdmissionBatch, "root_kind">;
  public_inputs: ZkAmsPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface ZkAmsAdmissionLocalVerificationInput
  extends ZkAmsAdmissionBatchInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface ZkAmsAdmissionLocalVerificationResult {
  ok: true;
  production: false;
  kind: "zk-ams-dev-fixture-v0";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: ZkAmsPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  admission_batch_root: string;
  batch_size: number;
}

export interface VegaCredentialPredicateCommitmentInput {
  version?: NumericLike;
  predicateCommitment?: BinaryLike;
  predicate_commitment?: BinaryLike;
  commitment?: BinaryLike;
  predicate?: unknown;
  predicateBytes?: BinaryLike;
  predicate_bytes?: BinaryLike;
  predicateJson?: unknown;
  predicate_json?: unknown;
  credentialSchema?: string;
  credential_schema?: string;
  domainSeparator?: string;
  domain_separator?: string;
  maxPredicateBytes?: NumericLike;
  max_predicate_bytes?: NumericLike;
}

export interface VegaCredentialPredicateCommitment {
  version: 1;
  predicate_commitment: number[];
  credential_schema: string;
  domain_separator: string;
  commitment_kind: "external" | "dev-sha256-predicate-digest";
  predicate_digest: number[] | null;
}

export interface VegaCredentialProofEnvelopeInput
  extends VegaCredentialPredicateCommitmentInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  issuerCommitment?: BinaryLike;
  issuer_commitment?: BinaryLike;
  issuer?: unknown;
  issuerBytes?: BinaryLike;
  issuer_bytes?: BinaryLike;
  issuerJson?: unknown;
  issuer_json?: unknown;
  subjectBinding?: BinaryLike;
  subject_binding?: BinaryLike;
  identityCommitment?: BinaryLike;
  identity_commitment?: BinaryLike;
  accountCommitment?: BinaryLike;
  account_commitment?: BinaryLike;
  accountId?: string;
  account_id?: string;
  subjectAccountId?: string;
  subject_account_id?: string;
  expirationEpoch?: NumericLike;
  expiration_epoch?: NumericLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxIssuerBytes?: NumericLike;
  max_issuer_bytes?: NumericLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface VegaCredentialPublicInputs {
  version: 1;
  issuer_commitment: string;
  credential_schema: string;
  predicate_commitment: string;
  subject_binding: string;
  expiration_epoch: number;
  domain_separator: string;
}

export interface VegaCredentialDevProofFixture {
  kind: "vega-dev-fixture-v0";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  public_inputs: VegaCredentialPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface VegaCredentialLocalVerificationInput
  extends VegaCredentialProofEnvelopeInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface VegaCredentialLocalVerificationResult {
  ok: true;
  production: false;
  kind: "vega-dev-fixture-v0";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: VegaCredentialPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  credential_schema: string;
  expiration_epoch: number;
}

export interface SilentThresholdCredentialCommitmentsInput {
  version?: NumericLike;
  issuerSetCommitment?: BinaryLike;
  issuer_set_commitment?: BinaryLike;
  issuerSet?: unknown;
  issuer_set?: unknown;
  issuerSetBytes?: BinaryLike;
  issuer_set_bytes?: BinaryLike;
  issuerSetJson?: unknown;
  issuer_set_json?: unknown;
  thresholdPolicyHash?: BinaryLike;
  threshold_policy_hash?: BinaryLike;
  thresholdPolicy?: unknown;
  threshold_policy?: unknown;
  thresholdPolicyBytes?: BinaryLike;
  threshold_policy_bytes?: BinaryLike;
  thresholdPolicyJson?: unknown;
  threshold_policy_json?: unknown;
  credentialShowingCommitment?: BinaryLike;
  credential_showing_commitment?: BinaryLike;
  showingCommitment?: BinaryLike;
  showing_commitment?: BinaryLike;
  credentialShowing?: unknown;
  credential_showing?: unknown;
  credentialShowingBytes?: BinaryLike;
  credential_showing_bytes?: BinaryLike;
  credentialShowingJson?: unknown;
  credential_showing_json?: unknown;
  showing?: unknown;
  showingBytes?: BinaryLike;
  showing_bytes?: BinaryLike;
  showingJson?: unknown;
  showing_json?: unknown;
  showingNullifier?: BinaryLike;
  showing_nullifier?: BinaryLike;
  credentialShowingNullifier?: BinaryLike;
  credential_showing_nullifier?: BinaryLike;
  nullifier?: BinaryLike;
  verifierPolicyHash?: BinaryLike;
  verifier_policy_hash?: BinaryLike;
  verifierPolicy?: unknown;
  verifier_policy?: unknown;
  verifierPolicyBytes?: BinaryLike;
  verifier_policy_bytes?: BinaryLike;
  verifierPolicyJson?: unknown;
  verifier_policy_json?: unknown;
  domainSeparator?: string;
  domain_separator?: string;
  maxIssuerSetBytes?: NumericLike;
  max_issuer_set_bytes?: NumericLike;
  maxPolicyBytes?: NumericLike;
  max_policy_bytes?: NumericLike;
  maxShowingBytes?: NumericLike;
  max_showing_bytes?: NumericLike;
}

export interface SilentThresholdCredentialCommitments {
  version: 1;
  issuer_set_commitment: number[];
  threshold_policy_hash: number[];
  credential_showing_commitment: number[];
  showing_nullifier: number[];
  verifier_policy_hash: number[];
  domain_separator: string;
  commitment_kinds: Record<string, "external" | string>;
  source_digests: Record<string, number[] | null>;
}

export interface SilentThresholdCredentialEnvelopeInput
  extends SilentThresholdCredentialCommitmentsInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface SilentThresholdCredentialPublicInputs {
  version: 1;
  issuer_set_commitment: string;
  threshold_policy_hash: string;
  credential_showing_commitment: string;
  showing_nullifier: string;
  verifier_policy_hash: string;
  domain_separator: string;
}

export interface SilentThresholdCredentialDevProofFixture {
  kind: "silent-threshold-dev-fixture-v0";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  commitments: Omit<
    SilentThresholdCredentialCommitments,
    "commitment_kinds" | "source_digests" | "version"
  >;
  public_inputs: SilentThresholdCredentialPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface SilentThresholdCredentialLocalVerificationInput
  extends SilentThresholdCredentialCommitmentsInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface SilentThresholdCredentialLocalVerificationResult {
  ok: true;
  production: false;
  kind: "silent-threshold-dev-fixture-v0";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: SilentThresholdCredentialPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  showing_nullifier: string;
}

export interface ZkX509IdentityCommitmentsInput {
  version?: NumericLike;
  caRootCommitment?: BinaryLike;
  ca_root_commitment?: BinaryLike;
  caRoot?: unknown;
  ca_root?: unknown;
  caRootBytes?: BinaryLike;
  ca_root_bytes?: BinaryLike;
  caRootJson?: unknown;
  ca_root_json?: unknown;
  trustRoot?: unknown;
  trust_root?: unknown;
  trustRootJson?: unknown;
  trust_root_json?: unknown;
  certificatePolicyHash?: BinaryLike;
  certificate_policy_hash?: BinaryLike;
  certificatePolicy?: unknown;
  certificate_policy?: unknown;
  certificatePolicyBytes?: BinaryLike;
  certificate_policy_bytes?: BinaryLike;
  certificatePolicyJson?: unknown;
  certificate_policy_json?: unknown;
  revocationRoot?: BinaryLike;
  revocation_root?: BinaryLike;
  revocationData?: unknown;
  revocation_data?: unknown;
  revocationBytes?: BinaryLike;
  revocation_bytes?: BinaryLike;
  revocationJson?: unknown;
  revocation_json?: unknown;
  revocationSet?: unknown;
  revocation_set?: unknown;
  revocationSetJson?: unknown;
  revocation_set_json?: unknown;
  revocationList?: unknown;
  revocation_list?: unknown;
  revocationListJson?: unknown;
  revocation_list_json?: unknown;
  subjectCommitment?: BinaryLike;
  subject_commitment?: BinaryLike;
  subject?: unknown;
  subjectBytes?: BinaryLike;
  subject_bytes?: BinaryLike;
  subjectJson?: unknown;
  subject_json?: unknown;
  certificateSubject?: unknown;
  certificate_subject?: unknown;
  certificateSubjectJson?: unknown;
  certificate_subject_json?: unknown;
  addressBinding?: BinaryLike;
  address_binding?: BinaryLike;
  walletBinding?: BinaryLike;
  wallet_binding?: BinaryLike;
  accountId?: string;
  account_id?: string;
  walletAccountId?: string;
  wallet_account_id?: string;
  walletAddress?: string;
  wallet_address?: string;
  domainSeparator?: string;
  domain_separator?: string;
  maxCaRootBytes?: NumericLike;
  max_ca_root_bytes?: NumericLike;
  maxPolicyBytes?: NumericLike;
  max_policy_bytes?: NumericLike;
  maxRevocationBytes?: NumericLike;
  max_revocation_bytes?: NumericLike;
  maxSubjectBytes?: NumericLike;
  max_subject_bytes?: NumericLike;
}

export interface ZkX509IdentityCommitments {
  version: 1;
  ca_root_commitment: number[];
  certificate_policy_hash: number[];
  revocation_root: number[];
  subject_commitment: number[];
  address_binding: number[];
  domain_separator: string;
  commitment_kinds: Record<string, "external" | string>;
  source_digests: Record<string, number[] | null>;
}

export interface ZkX509IdentityEnvelopeInput
  extends ZkX509IdentityCommitmentsInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface ZkX509IdentityPublicInputs {
  version: 1;
  ca_root_commitment: string;
  certificate_policy_hash: string;
  revocation_root: string;
  subject_commitment: string;
  address_binding: string;
  domain_separator: string;
}

export interface ZkX509IdentityDevProofFixture {
  kind: "zk-x509-dev-fixture-v0";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  commitments: Omit<
    ZkX509IdentityCommitments,
    "commitment_kinds" | "source_digests" | "version"
  >;
  public_inputs: ZkX509IdentityPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface ZkX509IdentityLocalVerificationInput
  extends ZkX509IdentityCommitmentsInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface ZkX509IdentityLocalVerificationResult {
  ok: true;
  production: false;
  kind: "zk-x509-dev-fixture-v0";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: ZkX509IdentityPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  address_binding: string;
}

export interface JindoLatticePublicInputsInput {
  version?: NumericLike;
  commitment?: BinaryLike;
  polynomialCommitment?: BinaryLike;
  polynomial_commitment?: BinaryLike;
  polynomial?: unknown;
  polynomialBytes?: BinaryLike;
  polynomial_bytes?: BinaryLike;
  polynomialJson?: unknown;
  polynomial_json?: unknown;
  commitmentMaterial?: unknown;
  commitment_material?: unknown;
  commitmentMaterialJson?: unknown;
  commitment_material_json?: unknown;
  openingClaimCommitment?: BinaryLike;
  opening_claim_commitment?: BinaryLike;
  openingClaimHash?: BinaryLike;
  opening_claim_hash?: BinaryLike;
  openingClaimDigest?: BinaryLike;
  opening_claim_digest?: BinaryLike;
  opening_claim?: BinaryLike;
  openingClaim?: BinaryLike;
  claim?: unknown;
  claimBytes?: BinaryLike;
  claim_bytes?: BinaryLike;
  claimJson?: unknown;
  claim_json?: unknown;
  openingClaimBytes?: BinaryLike;
  opening_claim_bytes?: BinaryLike;
  openingClaimJson?: unknown;
  opening_claim_json?: unknown;
  evaluationClaim?: unknown;
  evaluation_claim?: unknown;
  evaluationClaimJson?: unknown;
  evaluation_claim_json?: unknown;
  querySetHash?: BinaryLike;
  query_set_hash?: BinaryLike;
  querySetRoot?: BinaryLike;
  query_set_root?: BinaryLike;
  querySet?: unknown;
  query_set?: unknown;
  querySetBytes?: BinaryLike;
  query_set_bytes?: BinaryLike;
  querySetJson?: unknown;
  query_set_json?: unknown;
  queries?: unknown;
  queriesJson?: unknown;
  queries_json?: unknown;
  parameterHash?: BinaryLike;
  parameter_hash?: BinaryLike;
  paramsHash?: BinaryLike;
  params_hash?: BinaryLike;
  parameters?: unknown;
  parametersBytes?: BinaryLike;
  parameters_bytes?: BinaryLike;
  parametersJson?: unknown;
  parameters_json?: unknown;
  parameterSet?: unknown;
  parameter_set?: unknown;
  parameterSetJson?: unknown;
  parameter_set_json?: unknown;
  params?: unknown;
  paramsBytes?: BinaryLike;
  params_bytes?: BinaryLike;
  paramsJson?: unknown;
  params_json?: unknown;
  domainSeparator?: string;
  domain_separator?: string;
  maxPolynomialBytes?: NumericLike;
  max_polynomial_bytes?: NumericLike;
  maxOpeningClaimBytes?: NumericLike;
  max_opening_claim_bytes?: NumericLike;
  maxQuerySetBytes?: NumericLike;
  max_query_set_bytes?: NumericLike;
  maxParameterBytes?: NumericLike;
  max_parameter_bytes?: NumericLike;
}

export interface JindoLatticePublicInputsDescriptor {
  version: 1;
  commitment: number[];
  opening_claim: number[];
  query_set: number[];
  parameter_hash: number[];
  domain_separator: string;
  commitment_kinds: Record<string, "external" | string>;
  source_digests: Record<string, number[] | null>;
}

export interface JindoLatticeProofEnvelopeInput
  extends JindoLatticePublicInputsInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface JindoLatticePublicInputs {
  version: 1;
  commitment: string;
  opening_claim: string;
  query_set: string;
  parameter_hash: string;
  domain_separator: string;
}

export interface JindoLatticeDevProofFixture {
  kind: "jindo-lattice-dev-fixture-v0";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  public_inputs: JindoLatticePublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface JindoLatticeLocalVerificationInput
  extends JindoLatticePublicInputsInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface JindoLatticeLocalVerificationResult {
  ok: true;
  production: false;
  kind: "jindo-lattice-dev-fixture-v0";
  backend: "unsupported";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: JindoLatticePublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  parameter_hash: string;
}

export interface SisHintsCredentialCommitmentsInput {
  version?: NumericLike;
  issuerCommitment?: BinaryLike;
  issuer_commitment?: BinaryLike;
  issuerParameterCommitment?: BinaryLike;
  issuer_parameter_commitment?: BinaryLike;
  issuer?: unknown;
  issuerBytes?: BinaryLike;
  issuer_bytes?: BinaryLike;
  issuerJson?: unknown;
  issuer_json?: unknown;
  issuerParameters?: unknown;
  issuer_parameters?: unknown;
  issuerParametersJson?: unknown;
  issuer_parameters_json?: unknown;
  credentialCommitment?: BinaryLike;
  credential_commitment?: BinaryLike;
  credentialShowingCommitment?: BinaryLike;
  credential_showing_commitment?: BinaryLike;
  credential?: unknown;
  credentialBytes?: BinaryLike;
  credential_bytes?: BinaryLike;
  credentialJson?: unknown;
  credential_json?: unknown;
  credentialShowing?: unknown;
  credential_showing?: unknown;
  credentialShowingJson?: unknown;
  credential_showing_json?: unknown;
  showing?: unknown;
  showingJson?: unknown;
  showing_json?: unknown;
  showingPolicyHash?: BinaryLike;
  showing_policy_hash?: BinaryLike;
  policyHash?: BinaryLike;
  policy_hash?: BinaryLike;
  verifierPolicyHash?: BinaryLike;
  verifier_policy_hash?: BinaryLike;
  showingPolicy?: unknown;
  showing_policy?: unknown;
  showingPolicyBytes?: BinaryLike;
  showing_policy_bytes?: BinaryLike;
  showingPolicyJson?: unknown;
  showing_policy_json?: unknown;
  policy?: unknown;
  policyBytes?: BinaryLike;
  policy_bytes?: BinaryLike;
  policyJson?: unknown;
  policy_json?: unknown;
  verifierPolicy?: unknown;
  verifier_policy?: unknown;
  verifierPolicyJson?: unknown;
  verifier_policy_json?: unknown;
  parameterHash?: BinaryLike;
  parameter_hash?: BinaryLike;
  paramsHash?: BinaryLike;
  params_hash?: BinaryLike;
  parameters?: unknown;
  parametersBytes?: BinaryLike;
  parameters_bytes?: BinaryLike;
  parametersJson?: unknown;
  parameters_json?: unknown;
  parameterSet?: unknown;
  parameter_set?: unknown;
  parameterSetJson?: unknown;
  parameter_set_json?: unknown;
  sisParameters?: unknown;
  sis_parameters?: unknown;
  sisParametersJson?: unknown;
  sis_parameters_json?: unknown;
  params?: unknown;
  paramsJson?: unknown;
  params_json?: unknown;
  domainSeparator?: string;
  domain_separator?: string;
  maxIssuerBytes?: NumericLike;
  max_issuer_bytes?: NumericLike;
  maxCredentialBytes?: NumericLike;
  max_credential_bytes?: NumericLike;
  maxPolicyBytes?: NumericLike;
  max_policy_bytes?: NumericLike;
  maxParameterBytes?: NumericLike;
  max_parameter_bytes?: NumericLike;
}

export interface SisHintsCredentialCommitments {
  version: 1;
  issuer_commitment: number[];
  credential_commitment: number[];
  showing_policy_hash: number[];
  parameter_hash: number[];
  domain_separator: string;
  commitment_kinds: Record<string, "external" | string>;
  source_digests: Record<string, number[] | null>;
}

export interface SisHintsCredentialEnvelopeInput
  extends SisHintsCredentialCommitmentsInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface SisHintsCredentialPublicInputs {
  version: 1;
  issuer_commitment: string;
  credential_commitment: string;
  showing_policy_hash: string;
  parameter_hash: string;
  domain_separator: string;
}

export interface SisHintsCredentialDevProofFixture {
  kind: "sis-hints-dev-fixture-v0";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  commitments: Omit<
    SisHintsCredentialCommitments,
    "commitment_kinds" | "source_digests" | "version"
  >;
  public_inputs: SisHintsCredentialPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface SisHintsCredentialLocalVerificationInput
  extends SisHintsCredentialCommitmentsInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface SisHintsCredentialLocalVerificationResult {
  ok: true;
  production: false;
  kind: "sis-hints-dev-fixture-v0";
  backend: "unsupported";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: SisHintsCredentialPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  parameter_hash: string;
}

export interface AnonymousPgcReceiverInput {
  accountCommitment?: BinaryLike;
  account_commitment?: BinaryLike;
  receiverCommitment?: BinaryLike;
  receiver_commitment?: BinaryLike;
  ciphertextCommitment?: BinaryLike;
  ciphertext_commitment?: BinaryLike;
  receiverCiphertextCommitment?: BinaryLike;
  receiver_ciphertext_commitment?: BinaryLike;
  ciphertext?: BinaryLike;
  receiverCiphertext?: BinaryLike;
  receiver_ciphertext?: BinaryLike;
  encryptedNote?: BinaryLike;
  encrypted_note?: BinaryLike;
}

export interface AnonymousPgcReceiverSetInput {
  version?: NumericLike;
  threshold?: NumericLike;
  k?: NumericLike;
  receivers: ReadonlyArray<AnonymousPgcReceiverInput>;
}

export interface AnonymousPgcReceiverSet {
  version: 1;
  threshold: number;
  receiver_count: number;
  receivers: Array<{
    account_commitment: number[];
    ciphertext_commitment: number[];
    ciphertext_digest?: number[];
  }>;
  receiver_set_commitment: number[];
}

export interface AnonymousPgcDevProofFixtureInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  receiverSet?: AnonymousPgcReceiverSetInput | AnonymousPgcReceiverSet;
  receiver_set?: AnonymousPgcReceiverSetInput | AnonymousPgcReceiverSet;
  version?: NumericLike;
  threshold?: NumericLike;
  k?: NumericLike;
  receivers?: ReadonlyArray<AnonymousPgcReceiverInput>;
  anonymitySetRoot?: BinaryLike;
  anonymity_set_root?: BinaryLike;
  txDigest?: BinaryLike;
  tx_digest?: BinaryLike;
  payloadDigest?: BinaryLike;
  payload_digest?: BinaryLike;
  payload?: BinaryLike;
  payloadBytes?: BinaryLike;
  payload_bytes?: BinaryLike;
  payloadJson?: unknown;
  payload_json?: unknown;
  balanceCommitments?: ReadonlyArray<BinaryLike | { commitment?: BinaryLike }>;
  balance_commitments?: ReadonlyArray<BinaryLike | { commitment?: BinaryLike }>;
  linkTag?: BinaryLike;
  link_tag?: BinaryLike;
  rangeCommitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  range_commitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  chainId?: string;
  chain_id?: string;
  domainSeparator?: string;
  domain_separator?: string;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
  maxPayloadBytes?: NumericLike;
  max_payload_bytes?: NumericLike;
}

export interface AnonymousPgcPublicInputs {
  version: 1;
  anonymity_set_root: string;
  tx_digest: string;
  balance_commitments: string[];
  receiver_set_commitment: string;
  receiver_ciphertext_commitments: string[];
  receiver_threshold: number;
  receiver_count: number;
  link_tag: string;
  range_commitments: string[];
  chain_id: string;
  domain_separator: string;
}

export interface AnonymousPgcDevProofFixture {
  kind: "anonymous-pgc-dev-fixture-v1";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  receiver_set: AnonymousPgcReceiverSet;
  public_inputs: AnonymousPgcPublicInputs;
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface AnonymousPgcDevProofLocalVerificationInput
  extends AnonymousPgcDevProofFixtureInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface AnonymousPgcDevProofLocalVerificationResult {
  ok: true;
  production: false;
  kind: "anonymous-pgc-dev-fixture-v1";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: AnonymousPgcPublicInputs;
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
  receiver_count: number;
  receiver_threshold: number;
}

export type VeRangeCommitmentScheme =
  | "pedersen-v1"
  | "pedersen-bls12-381"
  | "pedersen-decaf377"
  | "verange-pedersen-v1"
  | string;

export interface RangeCommitmentInput {
  version?: NumericLike;
  commitment?: BinaryLike;
  rangeCommitment?: BinaryLike;
  range_commitment?: BinaryLike;
  valueCommitment?: BinaryLike;
  value_commitment?: BinaryLike;
  bitLength?: NumericLike;
  bit_length?: NumericLike;
  aggregationCount?: NumericLike;
  aggregation_count?: NumericLike;
  commitmentScheme?: VeRangeCommitmentScheme;
  commitment_scheme?: VeRangeCommitmentScheme;
  domainSeparator?: string;
  domain_separator?: string;
  payloadDigest?: BinaryLike;
  payload_digest?: BinaryLike;
  txDigest?: BinaryLike;
  tx_digest?: BinaryLike;
  payload?: BinaryLike;
  payloadBytes?: BinaryLike;
  payload_bytes?: BinaryLike;
  payloadJson?: unknown;
  payload_json?: unknown;
  maxPayloadBytes?: NumericLike;
  max_payload_bytes?: NumericLike;
}

export interface RangeCommitmentDescriptor {
  version: 1;
  commitment: number[];
  bit_length: number;
  aggregation_count: number;
  commitment_scheme: string;
  domain_separator: string;
  payload_digest: number[];
}

export interface VeRangeProofEnvelopeInput extends RangeCommitmentInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  commitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  rangeCommitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  range_commitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  proofBytes?: BinaryLike;
  proof_bytes?: BinaryLike;
  proof?: BinaryLike;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface VeRangeDevProofFixtureInput extends RangeCommitmentInput {
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  circuitId?: string;
  circuit_id?: string;
  vkHash?: BinaryLike;
  vk_hash?: BinaryLike;
  verifierKeyHash?: BinaryLike;
  verifyingKeyHash?: BinaryLike;
  commitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  rangeCommitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  range_commitments?: ReadonlyArray<BinaryLike | RangeCommitmentInput>;
  aux?: BinaryLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  maxPublicInputBytes?: NumericLike;
  max_public_input_bytes?: NumericLike;
}

export interface VeRangeDevProofFixture {
  kind: "verange-dev-fixture-v1";
  production: false;
  proof_bytes: number[];
  proofBytes: Buffer;
  public_inputs: {
    version: 1;
    commitments: string[];
    range_parameters: {
      bit_length: number;
      commitment_scheme: string;
    };
    aggregation_count: number;
    domain_separator: string;
    payload_digest: string;
  };
  publicInputBytes: Buffer;
  envelope: Buffer;
}

export interface VeRangeLocalVerificationInput extends RangeCommitmentInput {
  envelope?: BinaryLike;
  proofEnvelope?: BinaryLike;
  proof_envelope?: BinaryLike;
  bytes?: BinaryLike;
}

export interface VeRangeLocalVerificationResult {
  ok: true;
  production: false;
  kind: "verange-dev-fixture-v1";
  backend: "stark/fri/sha256-goldilocks";
  circuit_id: string;
  verifier_key_hash: string;
  public_inputs: VeRangeDevProofFixture["public_inputs"];
  public_input_bytes: number;
  proof_bytes: number;
  aux_bytes: number;
}

export interface PrivacyVerifierKeyBoxInput {
  backend?: string;
  backendId?: string;
  bytes?: BinaryLike;
  keyBytes?: BinaryLike;
  verifyingKeyBytes?: BinaryLike;
  vkBytes?: BinaryLike;
}

export interface PrivacyVerifierKeyRecordInput {
  version: NumericLike;
  circuitId?: string;
  circuit_id?: string;
  ownerManifestId?: string | null;
  owner_manifest_id?: string | null;
  owner?: string | null;
  namespace?: string;
  backend?: PrivacyBackendTag;
  backendTag?: PrivacyBackendTag;
  backend_tag?: PrivacyBackendTag;
  curve?: string;
  publicInputsSchemaHash?: BinaryLike;
  public_inputs_schema_hash?: BinaryLike;
  schemaHash?: BinaryLike;
  schema_hash?: BinaryLike;
  commitment?: BinaryLike;
  verifyingKeyCommitment?: BinaryLike;
  vkCommitment?: BinaryLike;
  vk_commitment?: BinaryLike;
  vkLen?: NumericLike;
  vk_len?: NumericLike;
  verifyingKeyLength?: NumericLike;
  maxProofBytes?: NumericLike;
  max_proof_bytes?: NumericLike;
  gasScheduleId?: string;
  gas_schedule_id?: string;
  metadataUriCid?: string | null;
  metadata_uri_cid?: string | null;
  vkBytesCid?: string | null;
  vk_bytes_cid?: string | null;
  activationHeight?: NumericLike | null;
  activation_height?: NumericLike | null;
  withdrawHeight?: NumericLike | null;
  withdraw_height?: NumericLike | null;
  key?: PrivacyVerifierKeyBoxInput | BinaryLike | null;
  verifyingKey?: PrivacyVerifierKeyBoxInput | BinaryLike | null;
  verifying_key?: PrivacyVerifierKeyBoxInput | BinaryLike | null;
  verifyingKeyBytes?: BinaryLike | null;
  verifying_key_bytes?: BinaryLike | null;
  vkBytes?: BinaryLike | null;
  vk_bytes?: BinaryLike | null;
  status?: "Proposed" | "Active" | "Withdrawn" | string;
}

export interface RegisterPrivacyVerifierKeyInstructionInput
  extends PrivacyVerifierKeyRecordInput {
  id?: VerifyingKeyIdLike;
  verifierKey?: VerifyingKeyIdLike;
  verifierKeyId?: VerifyingKeyIdLike;
  verifyingKeyId?: VerifyingKeyIdLike;
  keyId?: VerifyingKeyIdLike;
  vkRef?: VerifyingKeyIdLike;
  verifyingKeyRef?: VerifyingKeyIdLike;
  record?: PrivacyVerifierKeyRecordInput;
  verifierRecord?: PrivacyVerifierKeyRecordInput;
  verifyingKeyRecord?: PrivacyVerifierKeyRecordInput;
}

export interface RetirePrivacyVerifierKeyInstructionInput
  extends RegisterPrivacyVerifierKeyInstructionInput {}

/**
 * Canonicalise an account identifier to i105.
 *
 * Accepts only encoded i105 account ids.
 * Domain-suffixed literals (`<id>@domain`) and canonical-hex account literals are rejected.
 */
export function normalizeAccountId(value: string, name?: string): string;

/**
 * Canonicalise a public asset identifier to bare Base58 form.
 * Asset aliases (`name#dataspace` / `name#domain.dataspace`) must be resolved first.
 */
export function normalizeAssetId(value: string, name?: string): string;

/**
 * Canonicalise an internal asset-holding identifier in
 * `<base58-asset-definition-id>#<i105-account-id>` form.
 */
export function normalizeAssetHoldingId(value: string, name?: string): string;

/**
 * Canonicalise an RWA identifier in `<64-hex-hash>$<domain>` form.
 */
export function normalizeRwaId(value: string, name?: string): string;

export interface ConfidentialGasSchedule {
  proofBase: number;
  perPublicInput: number;
  perProofByte: number;
  perNullifier: number;
  perCommitment: number;
}

export function extractConfidentialGasConfig(
  input?: { config?: unknown } | unknown,
): ConfidentialGasSchedule | null;

export interface BlockListOptions {
  page?: NumericLike;
  page_number?: NumericLike;
  perPage?: NumericLike;
  per_page?: NumericLike;
  limit?: NumericLike;
  signal?: AbortSignal;
}

export interface EventStreamOptions {
  filter?: string | Record<string, unknown>;
  lastEventId?: string;
  signal?: AbortSignal;
}

export interface ContractEventStreamOptions {
  authority?: string;
  contractAddress?: string;
  contractAlias?: string;
  module?: string;
  eventKind?: string;
  participant?: string;
  assetId?: string;
  provenance?: string;
  sinceTimestampMs?: NumericLike;
  untilTimestampMs?: NumericLike;
  resultOk?: boolean;
  lastEventId?: string;
  signal?: AbortSignal;
}

export interface CanonicalRequestAuth {
  accountId: string;
  privateKey:
    | Buffer
    | Uint8Array
    | ArrayBuffer
    | ArrayBufferView
    | string
    | number[];
}

export interface PermissionedIterableOptions {
  requirePermissions?: boolean;
  canonicalAuth?: CanonicalRequestAuth;
}

export interface IterableListOptions extends PermissionedIterableOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  filter?: string | Record<string, unknown>;
  sort?: string | ReadonlyArray<{ key: string; order?: "asc" | "desc" }>;
  signal?: AbortSignal;
}

export interface AccountAssetListOptions extends IterableListOptions {
  assetId?: string;
}

export interface AccountTransactionListOptions extends IterableListOptions {
  assetId?: string;
}

export interface ContractActivityListOptions extends IterableListOptions {
  authority?: string;
  contractAddress?: string;
  contractAlias?: string;
  contractEntrypoint?: string;
  sinceTimestampMs?: NumericLike;
  untilTimestampMs?: NumericLike;
  resultOk?: boolean;
}

export interface ContractEventListOptions extends IterableListOptions {
  authority?: string;
  contractAddress?: string;
  contractAlias?: string;
  module?: string;
  eventKind?: string;
  participant?: string;
  assetId?: string;
  provenance?: string;
  sinceTimestampMs?: NumericLike;
  untilTimestampMs?: NumericLike;
  resultOk?: boolean;
}

export interface AssetHolderListOptions extends IterableListOptions {
  assetId?: string;
}

export interface IterableQueryOptions extends IterableListOptions {
  fetchSize?: NumericLike;
  queryName?: string;
  select?: ReadonlyArray<Record<string, unknown>>;
}

export interface TransactionQueryOptions extends IterableQueryOptions {
  assetId?: string;
  authority?: string;
  resultOk?: boolean;
  sinceTimestampMs?: NumericLike;
  untilTimestampMs?: NumericLike;
}

export interface PaginationIteratorOptions extends IterableListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface AccountAssetIteratorOptions extends PaginationIteratorOptions {
  assetId?: string;
}

export interface AccountTransactionIteratorOptions
  extends PaginationIteratorOptions {
  assetId?: string;
}

export interface TransactionIteratorOptions extends PaginationIteratorOptions {
  assetId?: string;
  authority?: string;
  resultOk?: boolean;
  sinceTimestampMs?: NumericLike;
  untilTimestampMs?: NumericLike;
}

export interface AssetHolderIteratorOptions extends PaginationIteratorOptions {
  assetId?: string;
}

export interface ExplorerNftListOptions {
  page?: NumericLike;
  perPage?: NumericLike;
  limit?: NumericLike;
  offset?: NumericLike;
  ownedBy?: string;
  domainId?: string;
  signal?: AbortSignal;
}

export interface ExplorerNftIteratorOptions extends ExplorerNftListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface ExplorerRwaListOptions {
  page?: NumericLike;
  perPage?: NumericLike;
  limit?: NumericLike;
  offset?: NumericLike;
  ownedBy?: string;
  domainId?: string;
  signal?: AbortSignal;
}

export interface ExplorerRwaIteratorOptions extends ExplorerRwaListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface ConnectAppListOptions {
  limit?: NumericLike;
  cursor?: string;
  signal?: AbortSignal;
}

export interface ConnectAppIteratorOptions extends ConnectAppListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface RepoLegDto {
  assetDefinitionId: string;
  quantity: string;
  metadata: unknown;
}

export interface RepoGovernanceDto {
  haircutBps: number;
  marginFrequencySecs: number;
}

export interface ToriiRepoAgreement {
  id: string;
  initiator: string;
  counterparty: string;
  custodian: string | null;
  cashLeg: RepoLegDto;
  collateralLeg: RepoLegDto;
  rateBps: number;
  maturityTimestampMs: number;
  initiatedTimestampMs: number;
  lastMarginCheckTimestampMs: number;
  governance: RepoGovernanceDto;
}

export interface RepoAgreementListResponse {
  items: ReadonlyArray<ToriiRepoAgreement>;
  total: number;
}

export interface TriggerListOptions {
  namespace?: string;
  authority?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface TriggerIteratorOptions extends TriggerListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface TriggerQueryIteratorOptions extends IterableQueryOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export type SubscriptionStatus =
  | "active"
  | "paused"
  | "past_due"
  | "canceled"
  | "suspended";

export interface SubscriptionPlanListOptions {
  provider?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface SubscriptionPlanIteratorOptions
  extends SubscriptionPlanListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface SubscriptionListOptions {
  ownedBy?: string;
  provider?: string;
  status?: SubscriptionStatus | string;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface SubscriptionIteratorOptions extends SubscriptionListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface ToriiIterableListResponse<T = unknown> {
  items: ReadonlyArray<T>;
  total: number;
}

export interface AliasResolutionDto {
  alias: string;
  account_id: string;
  index?: number;
  source?: string;
}

export interface CanonicalRequestOptions {
  signal?: AbortSignal;
  canonicalAuth?: CanonicalRequestAuth;
}

export interface AliasLookupByAccountItem {
  alias: string;
  dataspace: string;
  domain: string | null;
  is_primary: boolean;
}

export interface AliasLookupByAccountResponse {
  account_id: string;
  total: number;
  items: ReadonlyArray<AliasLookupByAccountItem>;
}

export interface AliasLookupByAccountOptions extends CanonicalRequestOptions {
  dataspace?: string;
  domain?: string;
}

export interface AliasVoprfEvaluateResponse {
  evaluated_element_hex: string;
  backend: string;
}

export interface RbcSampleRequestOptions {
  blockHash: string;
  height: number | string | bigint;
  view: number | string | bigint;
  count?: number | string | bigint;
  seed?: number | string | bigint;
  apiToken?: string;
  signal?: AbortSignal;
}

export interface RbcSampleRequestOverrides {
  count?: number | string | bigint;
  seed?: number | string | bigint;
  apiToken?: string;
}

export interface SumeragiRbcSnapshot {
  sessionsActive: number;
  sessionsPrunedTotal: number;
  readyBroadcastsTotal: number;
  deliverBroadcastsTotal: number;
  payloadBytesDeliveredTotal: number;
}

export interface SumeragiRbcSession {
  blockHash: string | null;
  height: number;
  view: number;
  totalChunks: number;
  receivedChunks: number;
  readyCount: number;
  delivered: boolean;
  invalid: boolean;
  payloadHash: string | null;
  recovered: boolean;
}

export interface SumeragiRbcSessionsSnapshot {
  sessionsActive: number;
  items: ReadonlyArray<SumeragiRbcSession>;
}

export interface SumeragiRbcDeliveryStatus {
  height: number;
  view: number;
  delivered: boolean;
  present: boolean;
  blockHash: string | null;
  readyCount: number;
  receivedChunks: number;
  totalChunks: number;
}

export interface SumeragiTelemetryAvailabilityCollector {
  collector_idx: number;
  peer_id: string;
  votes_ingested: number;
}

export interface SumeragiTelemetryAvailabilitySnapshot {
  total_votes_ingested: number;
  collectors: ReadonlyArray<SumeragiTelemetryAvailabilityCollector>;
}

export interface SumeragiTelemetryQcLatencyEntry {
  kind: string;
  last_ms: number;
}

export interface SumeragiTelemetryRbcBacklogSnapshot {
  pending_sessions: number;
  total_missing_chunks: number;
  max_missing_chunks: number;
}

export interface SumeragiTelemetryVrfLateReveal {
  signer: string;
  noted_at_height: number;
}

export interface SumeragiTelemetryVrfSummary {
  found: boolean;
  epoch: number;
  finalized: boolean;
  seed_hex: string | null;
  epoch_length: number;
  commit_deadline_offset: number;
  reveal_deadline_offset: number;
  roster_len: number;
  updated_at_height: number;
  participants_total: number;
  commitments_total: number;
  reveals_total: number;
  late_reveals_total: number;
  committed_no_reveal: ReadonlyArray<number>;
  no_participation: ReadonlyArray<number>;
  late_reveals: ReadonlyArray<SumeragiTelemetryVrfLateReveal>;
}

export interface SumeragiTelemetrySnapshot {
  availability: SumeragiTelemetryAvailabilitySnapshot;
  qc_latency_ms: ReadonlyArray<SumeragiTelemetryQcLatencyEntry>;
  rbc_backlog: SumeragiTelemetryRbcBacklogSnapshot;
  vrf: SumeragiTelemetryVrfSummary;
}

export interface CaptureSumeragiTelemetryOptions {
  signal?: AbortSignal;
  timestamp?: number;
}

export interface AppendSumeragiTelemetryOptions
  extends CaptureSumeragiTelemetryOptions {
  fs?: {
    mkdir?: typeof import("node:fs/promises").mkdir;
    appendFile?: typeof import("node:fs/promises").appendFile;
  };
}

export interface SumeragiTelemetryReplaySnapshot {
  capturedAtUnixMs: number;
  capturedAtIso: string;
  telemetry: SumeragiTelemetrySnapshot;
}

export interface RbcMerkleProof {
  leafIndex: number;
  depth: number | null;
  auditPath: ReadonlyArray<string | null>;
}

export interface RbcChunkProof {
  index: number;
  chunkHex: string;
  digestHex: string;
  proof: RbcMerkleProof;
}

export interface RbcSampleResponse {
  blockHash: string;
  height: number;
  view: number;
  totalChunks: number;
  chunkRoot: string;
  payloadHash: string | null;
  samples: ReadonlyArray<RbcChunkProof>;
}

export interface ToriiAccountListItem {
  id: string;
}

export type ToriiDomainListItem = ToriiAccountListItem;
export interface ToriiAssetDefinitionAliasBinding {
  alias: string;
  status:
    | "permanent"
    | "leased_active"
    | "leased_grace"
    | "expired_pending_cleanup";
  lease_expiry_ms?: number | null;
  grace_until_ms?: number | null;
  bound_at_ms: number;
}
export interface ToriiAssetDefinitionListItem {
  id: string;
  name?: string;
  alias?: string | null;
  alias_binding?: ToriiAssetDefinitionAliasBinding | null;
  description?: string | null;
  mintable?: unknown;
  spec?: unknown;
  logo?: string | null;
  metadata?: unknown;
  owned_by?: string;
  total_quantity?: string;
  balance_scope_policy?: unknown;
  confidential_policy?: unknown;
}
export interface ToriiNftListItem {
  id: string;
}
export interface ToriiRwaListItem {
  id: string;
}
export interface ToriiAccountAssetItem {
  asset: string;
  asset_id: string;
  quantity: string;
}
export interface ToriiAssetHolderItem {
  account_id: string;
  quantity: string;
}
export interface ToriiAccountTransactionItem {
  authority?: string;
  timestamp_ms?: number;
  entrypoint_hash: string;
  result_ok: boolean;
  asset_id?: string | string[];
}

export interface ToriiContractActivityItem {
  authority?: string;
  timestamp_ms?: number;
  entrypoint_hash: string;
  result_ok: boolean;
  contract_address: string;
  contract_alias?: string;
  contract_entrypoint?: string;
  contract_payload?: JsonValue;
  gas_asset_id?: string;
  fee_sponsor?: string;
  gas_limit?: number;
}

export interface ToriiContractEventItem {
  event_id: string;
  schema_version: number;
  provenance: string;
  authority?: string;
  timestamp_ms?: number;
  tx_hash_hex: string;
  block_height: number;
  block_hash_hex: string;
  result_ok: boolean;
  contract_address: string;
  contract_alias?: string;
  module: string;
  event_kind: string;
  participants?: ReadonlyArray<string>;
  asset_ids?: ReadonlyArray<string>;
  numeric_fields?: JsonValue;
  payload?: JsonValue;
  gas_asset_id?: string;
  fee_sponsor?: string;
  gas_limit?: number;
}

export interface ToriiProverReport {
  id: string;
  ok: boolean;
  error: string | null;
  content_type: string;
  size: number;
  created_ms: number;
  processed_ms: number;
  latency_ms: number;
  zk1_tags: ReadonlyArray<string> | null;
}

export interface ToriiProverReportIdList {
  kind: "ids";
  ids: ReadonlyArray<string>;
}

export interface ToriiProverReportMessageSummary {
  id: string;
  error: string | null;
}

export interface ToriiProverReportMessagesList {
  kind: "messages";
  messages: ReadonlyArray<ToriiProverReportMessageSummary>;
}

export interface ToriiProverReportFilters {
  okOnly?: boolean;
  failedOnly?: boolean;
  errorsOnly?: boolean;
  idsOnly?: boolean;
  messagesOnly?: boolean;
  latest?: boolean;
  contentType?: string;
  hasTag?: string;
  id?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  sinceMs?: NumericLike;
  beforeMs?: NumericLike;
  order?: "asc" | "desc";
}

export interface ToriiProverReportCollection {
  kind: "reports";
  reports: ReadonlyArray<ToriiProverReport>;
}

export type ToriiProverReportListResult =
  | ToriiProverReportCollection
  | ToriiProverReportIdList
  | ToriiProverReportMessagesList;

export interface ToriiAttachmentMetadata {
  id: string;
  contentType: string;
  size: number;
  createdMs: number;
  tenant: string | null;
}

export type ToriiVerifyingKeyStatus = "Proposed" | "Active" | "Withdrawn";

export interface ToriiVerifyingKeyInline {
  backend: string;
  bytes_b64: string;
}

export interface ToriiVerifyingKeyRecord {
  version: number;
  circuit_id: string;
  backend: string;
  curve: string | null;
  public_inputs_schema_hash: string;
  commitment_hex: string;
  vk_len: number;
  max_proof_bytes: number | null;
  gas_schedule_id: string | null;
  metadata_uri_cid: string | null;
  vk_bytes_cid: string | null;
  activation_height: number | null;
  withdraw_height: number | null;
  status: ToriiVerifyingKeyStatus;
  inline_key: ToriiVerifyingKeyInline | null;
}

export interface ToriiVerifyingKeyId {
  backend: string;
  name: string;
}

export interface ToriiVerifyingKeyDetail {
  id: ToriiVerifyingKeyId;
  record: ToriiVerifyingKeyRecord;
}

export interface ToriiVerifyingKeyListItem {
  id: ToriiVerifyingKeyId;
  record: ToriiVerifyingKeyRecord | null;
}

export interface ToriiVerifyingKeyListOptions {
  backend?: string;
  status?: ToriiVerifyingKeyStatus | string;
  nameContains?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  order?: "asc" | "desc";
  idsOnly?: boolean;
  signal?: AbortSignal | null;
}

export interface ToriiVerifyingKeyRegisterPayload {
  authority: string;
  private_key: string;
  backend: string;
  name: string;
  version: NumericLike;
  circuit_id: string;
  public_inputs_schema_hex?: string;
  public_inputs_schema_hash_hex?: string;
  public_inputs_schema_hash?: string;
  publicInputsSchemaHashHex?: string;
  publicInputsSchemaHash?: string;
  gas_schedule_id: string;
  curve?: string;
  max_proof_bytes?: NumericLike;
  metadata_uri_cid?: string;
  vk_bytes_cid?: string;
  activation_height?: NumericLike;
  withdraw_height?: NumericLike;
  commitment_hex?: string;
  vk_bytes?: Buffer | ArrayBuffer | ArrayBufferView | string;
  vk_len?: NumericLike;
  status?: ToriiVerifyingKeyStatus | string;
}

export interface ToriiVerifyingKeyUpdatePayload {
  authority: string;
  private_key: string;
  backend: string;
  name: string;
  version: NumericLike;
  circuit_id: string;
  public_inputs_schema_hex?: string;
  public_inputs_schema_hash_hex?: string;
  public_inputs_schema_hash?: string;
  publicInputsSchemaHashHex?: string;
  publicInputsSchemaHash?: string;
  gas_schedule_id?: string;
  curve?: string;
  max_proof_bytes?: NumericLike;
  metadata_uri_cid?: string;
  vk_bytes_cid?: string;
  activation_height?: NumericLike;
  withdraw_height?: NumericLike;
  commitment_hex?: string;
  vk_bytes?: Buffer | ArrayBuffer | ArrayBufferView | string;
  vk_len?: NumericLike;
  status?: ToriiVerifyingKeyStatus | string;
}

export interface ToriiPeerRecord {
  address: string;
  public_key_hex: string;
}

export interface ToriiTelemetryPeerInfo {
  url: string;
  connected: boolean;
  telemetryUnsupported: boolean;
  config?: ToriiTelemetryPeerConfig;
  location?: ToriiTelemetryPeerLocation;
  connectedPeers?: ReadonlyArray<string>;
}

export interface ToriiTelemetryPeerConfig {
  publicKey: string;
  queueCapacity?: number;
  networkBlockGossipSize?: number;
  networkBlockGossipPeriodMs?: number;
  networkTxGossipSize?: number;
  networkTxGossipPeriodMs?: number;
}

export interface ToriiTelemetryPeerLocation {
  lat: number;
  lon: number;
  country: string;
  city: string;
}

export interface ToriiExplorerMetricsSnapshot {
  peers: number;
  domains: number;
  accounts: number;
  assets: number;
  transactionsAccepted: number;
  transactionsRejected: number;
  blockHeight: number;
  blockCreatedAt: string | null;
  finalizedBlockHeight: number;
  averageCommitTimeMs: number | null;
  averageBlockTimeMs: number | null;
}

export interface ToriiExplorerPaginationMeta {
  page: number;
  perPage: number;
  totalPages: number;
  totalItems: number;
}

export interface ToriiExplorerNft {
  id: string;
  ownedBy: string;
  metadata: Record<string, unknown>;
}

export interface ToriiExplorerNftsPage {
  pagination: ToriiExplorerPaginationMeta;
  items: ReadonlyArray<ToriiExplorerNft>;
}

export interface ToriiExplorerRwa {
  id: string;
  ownedBy: string;
  quantity: string;
  heldQuantity: string;
  primaryReference: string;
  status: string | null;
  isFrozen: boolean;
  metadata: Record<string, JsonValue>;
  raw: Record<string, JsonValue>;
}

export interface ToriiExplorerRwasPage {
  pagination: ToriiExplorerPaginationMeta;
  items: ReadonlyArray<ToriiExplorerRwa>;
}

export interface ToriiExplorerBlock {
  hash: string;
  height: number;
  createdAt: string;
  prevBlockHash: string | null;
  transactionsHash: string | null;
  transactionsRejected: number;
  transactionsTotal: number;
}

export interface ToriiExplorerBlocksPage {
  pagination: ToriiExplorerPaginationMeta;
  items: ReadonlyArray<ToriiExplorerBlock>;
}

export interface ToriiExplorerAccountQrSnapshot {
  canonicalId: string;
  literal: string;
  networkPrefix: number;
  errorCorrection: string;
  modules: number;
  qrVersion: number;
  svg: string;
}

export interface ToriiVpnProfile {
  available: boolean;
  relayEndpoint: string;
  supportedExitClasses: ReadonlyArray<string>;
  defaultExitClass: string;
  leaseSecs: number;
  dnsPushIntervalSecs: number;
  meterFamily: string;
  routePushes: ReadonlyArray<string>;
  excludedRoutes: ReadonlyArray<string>;
  dnsServers: ReadonlyArray<string>;
  tunnelAddresses: ReadonlyArray<string>;
  mtuBytes: number;
  displayBillingLabel: string;
  feeAssetId: string;
  escrowAccountId: string;
  operatorAccountId: string;
  leaseFeeNanos: number;
  settlementGraceSecs: number;
  flowLabelBits: number;
  paddingBudgetMs: number;
  relayTlsSpkiSha256Hex: string | null;
}

export interface ToriiVpnTxInstruction {
  wireId: string;
  payloadHex: string;
}

export interface ToriiVpnQuote {
  quoteId: string;
  leaseIdHex: string;
  sessionIdHex: string;
  paymentReference: string;
  accountId: string;
  exitClass: string;
  relayEndpoint: string;
  leaseSecs: number;
  quoteExpiresAtMs: number;
  feeAssetId: string;
  escrowAccountId: string;
  operatorAccountId: string;
  leaseFeeNanos: number;
  routePushes: ReadonlyArray<string>;
  excludedRoutes: ReadonlyArray<string>;
  dnsServers: ReadonlyArray<string>;
  tunnelAddresses: ReadonlyArray<string>;
  mtuBytes: number;
  meterFamily: string;
  flowLabelBits: number;
  paddingBudgetMs: number;
  relayTlsSpkiSha256Hex: string | null;
  meteringPublicKeyHex: string;
  openLeaseInstruction: ToriiVpnTxInstruction | null;
  txInstructions: ReadonlyArray<ToriiVpnTxInstruction>;
}

export interface ToriiVpnSession {
  sessionId: string;
  accountId: string;
  exitClass: string;
  relayEndpoint: string;
  leaseSecs: number;
  expiresAtMs: number;
  connectedAtMs: number;
  meterFamily: string;
  quoteId: string;
  paymentReference: string;
  paymentTxHash: string;
  feeAssetId: string;
  escrowAccountId: string;
  operatorAccountId: string;
  leaseFeeNanos: number;
  flowLabelBits: number;
  paddingBudgetMs: number;
  relayTlsSpkiSha256Hex: string | null;
  routePushes: ReadonlyArray<string>;
  excludedRoutes: ReadonlyArray<string>;
  dnsServers: ReadonlyArray<string>;
  tunnelAddresses: ReadonlyArray<string>;
  mtuBytes: number;
  helperTicketHex: string;
  bytesIn: number;
  bytesOut: number;
  status: string;
}

export interface ToriiVpnReceipt {
  sessionId: string;
  accountId: string;
  exitClass: string;
  relayEndpoint: string;
  meterFamily: string;
  connectedAtMs: number;
  disconnectedAtMs: number;
  durationMs: number;
  bytesIn: number;
  bytesOut: number;
  status: string;
  receiptSource: string;
  quoteId: string;
  paymentTxHash: string;
  feeAssetId: string;
  escrowAccountId: string;
  operatorAccountId: string;
  leaseFeeNanos: number;
  earnedFeeNanos: number;
  refundedFeeNanos: number;
  leaseIdHex: string;
  settleLeaseInstruction: ToriiVpnTxInstruction | null;
  txInstructions: ReadonlyArray<ToriiVpnTxInstruction>;
}

export type SnsNameStatus =
  | { status: "Active" }
  | { status: "GracePeriod" }
  | { status: "Redemption" }
  | { status: "Frozen"; reason: string; untilMs: number }
  | { status: "Tombstoned"; reason: string };

export interface SnsNameSelector {
  version: number;
  suffixId: number;
  label: string;
}

export interface SnsNameController {
  controller_type: "Account" | "Multisig" | "ResolverTemplate" | "ExternalLink";
  account_address?: string;
  resolver_template_id?: string;
  payload?: Record<string, unknown>;
}

export interface SnsTokenValue {
  assetId: string;
  amount: string;
}

export interface SnsPricingTier {
  tierId: number;
  labelRegex: string;
  basePrice: SnsTokenValue;
  auctionKind: "VickreyCommitReveal" | "DutchReopen";
  dutchFloor: SnsTokenValue | null;
  minDurationYears: number;
  maxDurationYears: number;
}

export interface SnsReservedLabel {
  normalizedLabel: string;
  assignedTo: string | null;
  releaseAtMs: number | null;
  note: string;
}

export interface SnsFeeSplit {
  treasuryBps: number;
  stewardBps: number;
  referralMaxBps: number;
  escrowBps: number;
}

export interface SnsSuffixPolicy {
  suffixId: number;
  suffix: string;
  steward: string;
  status: "Active" | "Paused" | "Revoked";
  minTermYears: number;
  maxTermYears: number;
  gracePeriodDays: number;
  redemptionPeriodDays: number;
  referralCapBps: number;
  reservedLabels: ReadonlyArray<SnsReservedLabel>;
  paymentAssetId: string;
  pricing: ReadonlyArray<SnsPricingTier>;
  feeSplit: SnsFeeSplit;
  fundSplitterAccount: string;
  policyVersion: number;
  metadata: Record<string, unknown>;
}

export interface SnsGovernanceHook {
  proposal_id: string;
  council_vote_hash: string;
  dao_vote_hash: string;
  steward_ack: string;
  guardian_clearance?: string | null;
}

export interface SnsPaymentProof {
  asset_id: string;
  gross_amount: number;
  net_amount?: number;
  settlement_tx: unknown;
  payer: string;
  signature: unknown;
}

export interface SnsRegisterNameRequest {
  selector: {
    version?: number;
    suffix_id: number;
    label: string;
  };
  owner: string;
  controllers?: ReadonlyArray<SnsNameController>;
  term_years?: number;
  pricing_class_hint?: number;
  payment: SnsPaymentProof;
  governance?: SnsGovernanceHook | null;
  metadata?: Record<string, unknown>;
}

export interface SnsRenewNameRequest {
  term_years: number;
  payment: SnsPaymentProof;
}

export interface SnsTransferNameRequest {
  new_owner: string;
  governance: SnsGovernanceHook;
}

export interface SnsFreezeNameRequest {
  reason: string;
  until_ms: number;
  guardian_ticket: unknown;
}

export interface SnsNameRecord {
  selector: SnsNameSelector;
  nameHash: string;
  owner: string;
  controllers: ReadonlyArray<SnsNameController>;
  status: SnsNameStatus;
  pricingClass: number;
  registeredAtMs: number;
  expiresAtMs: number;
  graceExpiresAtMs: number;
  redemptionExpiresAtMs: number;
  metadata: Record<string, unknown>;
  auction: SnsAuction | null;
}

export interface SnsAuction {
  kind: "VickreyCommitReveal" | "DutchReopen";
  openedAtMs: number;
  closesAtMs: number;
  floorPrice: SnsTokenValue;
  highestCommitment: string | null;
  settlementTx: unknown;
}

export interface SnsRegisterNameResponse {
  nameRecord: SnsNameRecord;
}

export interface ToriiPipelineTransactionEvent {
  category: "Pipeline";
  event: "Transaction";
  hash: string;
  lane_id: number;
  dataspace_id: number;
  block_height: number | null;
  status: string;
}

export interface ToriiPipelineBlockEvent {
  category: "Pipeline";
  event: "Block";
  status: string;
}

export interface ToriiPipelineWarningEvent {
  category: "Pipeline";
  event: "Warning";
  kind: string;
  details: string;
  height: number;
}

export interface ToriiPipelineMergeLedgerEvent {
  category: "Pipeline";
  event: "MergeLedger";
  epoch_id: number;
  global_state_root: string;
}

export interface ToriiPipelineWitnessEvent {
  category: "Pipeline";
  event: "Witness";
  block_hash: string;
  height: number;
  view: number;
  epoch: number;
  read_count: number;
  write_count: number;
}

export type ToriiPipelineEvent =
  | ToriiPipelineTransactionEvent
  | ToriiPipelineBlockEvent
  | ToriiPipelineWarningEvent
  | ToriiPipelineMergeLedgerEvent
  | ToriiPipelineWitnessEvent;

export interface ToriiPipelineTransactionStatusStatus {
  kind: string;
  content: unknown;
  [key: string]: unknown;
}

export interface ToriiPipelineTransactionStatusContent {
  hash: string;
  status: ToriiPipelineTransactionStatusStatus;
  [key: string]: unknown;
}

export interface ToriiPipelineTransactionStatus {
  kind: string;
  content: ToriiPipelineTransactionStatusContent;
  [key: string]: unknown;
}

export interface ToriiProofEventBase {
  category: "Data";
  backend: string;
  proof_hash: string;
  call_hash: string | null;
  envelope_hash: string | null;
  vk_ref: string | null;
  vk_commitment: string | null;
}

export interface ToriiProofVerifiedEvent extends ToriiProofEventBase {
  event: "ProofVerified";
}

export interface ToriiProofRejectedEvent extends ToriiProofEventBase {
  event: "ProofRejected";
}

export type ToriiDataEvent =
  | ToriiProofVerifiedEvent
  | ToriiProofRejectedEvent
  | {
      category: "Data";
      summary: string;
    };

export interface ToriiOtherEvent {
  category: "Other";
  summary: string;
}

export type ToriiEventPayload =
  | ToriiPipelineEvent
  | ToriiDataEvent
  | ToriiOtherEvent;

export interface ToriiSseEvent<T = ToriiEventPayload> {
  event: string | null;
  data: T | string;
  id: string | null;
  retry?: number | null;
  raw: string | null;
}

export interface AccountPermissionsListOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface ToriiAccountPermissionItem {
  name: string;
  payload: unknown;
}

export class ConnectRetryPolicy {
  static readonly DEFAULT_BASE_DELAY_MS: number;
  static readonly DEFAULT_MAX_DELAY_MS: number;
  constructor(baseDelayMs?: number, maxDelayMs?: number);
  capMillis(attempt: number): number;
  delayMillis(
    attempt: number,
    seed: Uint8Array | ArrayBuffer | ArrayBufferView | ArrayLike<number>,
  ): number;
}

export type ConnectErrorCategory =
  | "transport"
  | "codec"
  | "authorization"
  | "timeout"
  | "queueOverflow"
  | "internal";

export interface ConnectErrorTelemetryOptions {
  fatal?: boolean | null;
  httpStatus?: number | null;
  underlying?: string | null;
}

export interface ConnectErrorFromOptions {
  fatal?: boolean | null;
  httpStatus?: number | null;
}

export interface ConnectErrorConvertible {
  toConnectError(): ConnectError;
}

export class ConnectError extends Error implements ConnectErrorConvertible {
  constructor(options?: {
    category?: ConnectErrorCategory;
    code?: string;
    message?: string;
    fatal?: boolean | null;
    httpStatus?: number | null;
    underlying?: string | null;
    cause?: unknown;
  });

  readonly category: ConnectErrorCategory;
  readonly code: string;
  readonly fatal: boolean;
  readonly httpStatus?: number;
  readonly underlying?: string;

  telemetryAttributes(
    options?: ConnectErrorTelemetryOptions,
  ): Record<string, string>;
  toConnectError(): ConnectError;
}

export type ConnectQueueErrorKind = "overflow" | "expired";

export class ConnectQueueError
  extends Error
  implements ConnectErrorConvertible
{
  constructor(
    kind: ConnectQueueErrorKind,
    options?: { limit?: number; ttlMs?: number },
  );

  readonly kind: ConnectQueueErrorKind;
  readonly limit?: number;
  readonly ttlMs?: number;

  static overflow(limit?: number): ConnectQueueError;
  static expired(ttlMs?: number): ConnectQueueError;
  toConnectError(): ConnectError;
}

export type ConnectDirection = "app_to_wallet" | "wallet_to_app";

export class ConnectJournalError extends Error {
  constructor(message?: string, options?: { cause?: unknown });
}

export interface ConnectJournalRecordInit {
  direction: ConnectDirection | string;
  sequence: number | bigint | string;
  ciphertext: ArrayLike<number> | ArrayBufferLike;
  payloadHash?: ArrayLike<number> | ArrayBufferLike;
  receivedAtMs?: number;
  expiresAtMs?: number;
}

export class ConnectJournalRecord {
  constructor(init: ConnectJournalRecordInit);
  static fromCiphertext(options: {
    direction: ConnectDirection | string;
    sequence: number | bigint | string;
    ciphertext: ArrayLike<number> | ArrayBufferLike;
    receivedAtMs?: number;
    retentionMs?: number;
  }): ConnectJournalRecord;
  static decode(
    data: ArrayLike<number> | ArrayBufferLike,
    offset?: number,
  ): { record: ConnectJournalRecord; bytesConsumed: number };
  readonly direction: ConnectDirection;
  readonly sequence: bigint;
  readonly ciphertext: Uint8Array;
  readonly payloadHash: Uint8Array;
  readonly receivedAtMs: number;
  readonly expiresAtMs: number;
  readonly payloadLength: number;
  readonly encodedLength: number;
  encode(): Uint8Array;
}

export interface ConnectQueueJournalOptions {
  maxRecordsPerQueue?: number;
  maxBytesPerQueue?: number;
  retentionMs?: number;
  indexedDbName?: string;
  indexedDbVersion?: number;
  storage?: "indexeddb" | "memory";
  indexedDbFactory?: IDBFactory;
}

export interface ConnectQueueJournalAppendOptions {
  ttlMs?: number;
  retentionMs?: number;
  receivedAtMs?: number;
}

export interface ConnectQueueJournalReadOptions {
  nowMs?: number;
}

export class ConnectQueueJournal {
  constructor(
    sessionId: string | ArrayBufferLike | ArrayLike<number>,
    options?: ConnectQueueJournalOptions,
  );
  readonly sessionKey: string;
  append(
    direction: ConnectDirection | string,
    sequence: number | bigint | string,
    ciphertext: ArrayLike<number> | ArrayBufferLike,
    options?: ConnectQueueJournalAppendOptions,
  ): Promise<void>;
  records(
    direction: ConnectDirection | string,
    options?: ConnectQueueJournalReadOptions,
  ): Promise<ConnectJournalRecord[]>;
  popOldest(
    direction: ConnectDirection | string,
    count?: number,
    options?: ConnectQueueJournalReadOptions,
  ): Promise<ConnectJournalRecord[]>;
}

export function connectErrorFrom(
  error: unknown,
  options?: ConnectErrorFromOptions,
): ConnectError;

export interface ToriiClientRetryOptions {
  timeoutMs?: number | null;
  maxRetries?: number | null;
  backoffInitialMs?: number | null;
  backoffMultiplier?: number | null;
  maxBackoffMs?: number | null;
  retryStatuses?: ReadonlyArray<number>;
  retryMethods?: ReadonlyArray<string>;
  defaultHeaders?: Record<string, string>;
  authToken?: string | null;
  apiToken?: string | null;
  retryProfiles?: Record<string, ToriiRetryProfileOptions>;
}

export interface ToriiRetryTelemetryEvent {
  phase: "response" | "network" | "timeout";
  attempt: number;
  nextAttempt: number;
  maxRetries: number;
  method: string;
  url: string;
  status?: number;
  errorName?: string | null;
  errorMessage?: string | null;
  timedOut?: boolean;
  backoffMs?: number;
  timestampMs: number;
  profile?: string;
  durationMs: number;
}

export interface InsecureTransportTelemetryEvent {
  client: string;
  method: string;
  url: string;
  baseUrl: string;
  host: string;
  protocol: string;
  pathIsAbsolute: boolean;
  originMatches: boolean;
  allowInsecure: boolean;
  hasCredentials: boolean;
  hasSensitiveBody?: boolean;
  hasCanonicalAuth?: boolean;
  timestampMs: number;
}

export interface ToriiRetryProfileOptions {
  maxRetries?: number | null;
  backoffInitialMs?: number | null;
  backoffMultiplier?: number | null;
  maxBackoffMs?: number | null;
  retryStatuses?: ReadonlyArray<number>;
  retryMethods?: ReadonlyArray<string>;
}

export interface ToriiResolvedRetryProfile {
  maxRetries: number;
  backoffInitialMs: number;
  backoffMultiplier: number;
  maxBackoffMs: number;
  retryStatuses: Set<number>;
  retryMethods: Set<string>;
}

export interface SorafsAliasPolicyOptions {
  positiveTtlSecs?: number;
  refreshWindowSecs?: number;
  hardExpirySecs?: number;
  negativeTtlSecs?: number;
  revocationTtlSecs?: number;
  rotationMaxAgeSecs?: number;
}

export interface SorafsAliasEvaluation {
  state: string | null;
  statusLabel: string | null;
  rotationDue: boolean;
  ageSeconds: number | null;
  generatedAtUnix: number | null;
  expiresAtUnix: number | null;
  expiresInSeconds: number | null;
  servable: boolean;
}

export interface SorafsAliasWarning {
  alias: string | null;
  evaluation: SorafsAliasEvaluation;
}

export interface SorafsReplicationAssignment {
  providerIdHex: string;
  sliceGiB: number;
  lane: string | null;
}

export interface SorafsReplicationSla {
  ingestDeadlineSecs: number;
  minAvailabilityPercentMilli: number;
  minPorSuccessPercentMilli: number;
}

export interface SorafsReplicationMetadataEntry {
  key: string;
  value: string;
}

export interface SorafsReplicationOrder {
  schemaVersion: number;
  orderIdHex: string;
  manifestCidUtf8: string | null;
  manifestCidBase64: string;
  manifestDigestHex: string;
  chunkingProfile: string;
  targetReplicas: number;
  assignments: ReadonlyArray<SorafsReplicationAssignment>;
  issuedAtUnix: number;
  deadlineAtUnix: number;
  sla: SorafsReplicationSla;
  metadata: ReadonlyArray<SorafsReplicationMetadataEntry>;
}

export function decodeReplicationOrder(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
): SorafsReplicationOrder;

export function captureSumeragiTelemetrySnapshot(
  client: ToriiClient,
  options?: CaptureSumeragiTelemetryOptions,
): Promise<SumeragiTelemetryReplaySnapshot>;

export function appendSumeragiTelemetrySnapshot(
  client: ToriiClient,
  outputPath: string,
  options?: AppendSumeragiTelemetryOptions,
): Promise<SumeragiTelemetryReplaySnapshot>;

export interface SorafsGatewayProviderSpec {
  name: string;
  providerIdHex: string;
  baseUrl: string;
  streamTokenB64: string;
  privacyEventsUrl?: string;
}

export interface SorafsLocalProxyNoritoBridgeOptions {
  spoolDir: string;
  extension?: string;
}

export interface SorafsLocalProxyCarBridgeOptions {
  cacheDir: string;
  extension?: string;
  allowZst?: boolean;
}

export interface SorafsLocalProxyKaigiBridgeOptions {
  spoolDir: string;
  extension?: string;
  roomPolicy?: "public" | "authenticated";
}

export interface SorafsLocalProxyOptions {
  bindAddr?: string;
  telemetryLabel?: string;
  guardCacheKeyHex?: string;
  emitBrowserManifest?: boolean;
  proxyMode?: "bridge" | "metadata-only";
  prewarmCircuits?: boolean;
  maxStreamsPerCircuit?: number;
  circuitTtlHintSecs?: number;
  noritoBridge?: SorafsLocalProxyNoritoBridgeOptions;
  carBridge?: SorafsLocalProxyCarBridgeOptions;
  kaigiBridge?: SorafsLocalProxyKaigiBridgeOptions;
}

export interface SorafsTaikaiCacheQosOptions {
  priorityRateBps: number | bigint;
  standardRateBps: number | bigint;
  bulkRateBps: number | bigint;
  burstMultiplier: number | bigint;
}

export interface SorafsTaikaiReliabilityOptions {
  failuresToTrip?: number | bigint;
  openSecs?: number | bigint;
}

export interface SorafsTaikaiCacheOptions {
  hotCapacityBytes: number | bigint;
  hotRetentionSecs: number | bigint;
  warmCapacityBytes: number | bigint;
  warmRetentionSecs: number | bigint;
  coldCapacityBytes: number | bigint;
  coldRetentionSecs: number | bigint;
  qos: SorafsTaikaiCacheQosOptions;
  reliability?: SorafsTaikaiReliabilityOptions;
}

export interface SorafsGatewayFetchOptions {
  manifestEnvelopeB64?: string;
  manifestCidHex?: string;
  clientId?: string;
  telemetryRegion?: string;
  rolloutPhase?:
    | "canary"
    | "ramp"
    | "default"
    | "stage-a"
    | "stage-b"
    | "stage-c";
  maxPeers?: number;
  retryBudget?: number;
  transportPolicy?: "soranet-first" | "soranet-strict" | "direct-only";
  anonymityPolicy?: "anon-guard-pq" | "anon-majority-pq" | "anon-strict-pq";
  writeMode?: "read-only" | "upload-pq-only" | string;
  policyOverride?: SorafsGatewayPolicyOverride;
  localProxy?: SorafsLocalProxyOptions;
  taikaiCache?: SorafsTaikaiCacheOptions;
  scoreboardOutPath?: string;
  scoreboardNowUnixSecs?: number | bigint;
  scoreboardTelemetryLabel?: string;
  scoreboardAllowImplicitMetadata?: boolean;
}

export interface SorafsGatewayPolicyOverride {
  transportPolicy?: "soranet-first" | "soranet-strict" | "direct-only";
  anonymityPolicy?: "anon-guard-pq" | "anon-majority-pq" | "anon-strict-pq";
}

export interface SorafsGatewayCarArchive {
  size: number | bigint;
  payloadDigestHex: string;
  archiveDigestHex: string;
  cidHex: string;
  rootCidsHex: ReadonlyArray<string>;
  verified: boolean;
  porLeafCount: number | bigint;
}

export interface SorafsGatewayCouncilSignature {
  signerHex: string;
  signatureHex: string;
}

export interface SorafsGatewayManifestGovernance {
  councilSignatures: ReadonlyArray<SorafsGatewayCouncilSignature>;
}

export interface SorafsGatewayCarVerification {
  manifestDigestHex: string;
  manifestPayloadDigestHex: string;
  manifestCarDigestHex: string;
  manifestContentLength: number | bigint;
  manifestChunkCount: number | bigint;
  manifestChunkProfileHandle: string;
  manifestGovernance: SorafsGatewayManifestGovernance;
  carArchive: SorafsGatewayCarArchive;
}

export interface SorafsGatewayAnonymityReport {
  policy: string;
  status: string;
  reason: string;
  soranetSelected: number;
  pqSelected: number;
  classicalSelected: number;
  classicalRatio: number;
  pqRatio: number;
  candidateRatio: number;
  deficitRatio: number;
  supplyDelta: number;
  brownout: boolean;
  brownoutEffective: boolean;
  usesClassical: boolean;
}

export interface SorafsGatewayProviderReport {
  provider: string;
  successes: number;
  failures: number;
  disabled: boolean;
}

export interface SorafsGatewayChunkReceipt {
  chunkIndex: number;
  provider: string;
  attempts: number;
  latencyMs: number;
  bytes: number;
}

export type SorafsGatewayProviderMix =
  | "mixed"
  | "direct-only"
  | "gateway-only"
  | "none";

export interface SorafsGatewayScoreboardMetadata {
  providerCount: number;
  gatewayProviderCount: number;
  providerMix: SorafsGatewayProviderMix;
  transportPolicy: string;
  transportPolicyOverride: boolean;
  transportPolicyOverrideLabel: string | null;
  anonymityPolicy: string;
  anonymityPolicyOverride: boolean;
  anonymityPolicyOverrideLabel: string | null;
  writeMode: string;
  writeModeEnforcesPq: boolean;
  maxParallel: number | null;
  maxPeers: number | null;
  retryBudget: number | null;
  providerFailureThreshold: number;
  assumeNowUnix: number;
  telemetrySourceLabel: string | null;
  telemetryRegion: string | null;
  gatewayManifestProvided: boolean;
  gatewayManifestId: string | null;
  gatewayManifestCid: string | null;
  allowImplicitMetadata: boolean;
}

export interface SorafsGatewayScoreboardEntry {
  provider_id: string;
  alias: string | null;
  raw_score: number;
  normalized_weight: number;
  eligibility: string | null;
}

export interface SorafsTaikaiCacheTierCounts {
  hot: number;
  warm: number;
  cold: number;
}

export interface SorafsTaikaiCacheEvictionCounts {
  expired: number;
  capacity: number;
}

export interface SorafsTaikaiCacheEvictions {
  hot: SorafsTaikaiCacheEvictionCounts;
  warm: SorafsTaikaiCacheEvictionCounts;
  cold: SorafsTaikaiCacheEvictionCounts;
}

export interface SorafsTaikaiCachePromotions {
  warmToHot: number;
  coldToWarm: number;
  coldToHot: number;
}

export interface SorafsTaikaiQosCounts {
  priority: number;
  standard: number;
  bulk: number;
}

export interface SorafsTaikaiCacheSummary {
  hits: SorafsTaikaiCacheTierCounts;
  misses: number;
  inserts: SorafsTaikaiCacheTierCounts;
  evictions: SorafsTaikaiCacheEvictions;
  promotions: SorafsTaikaiCachePromotions;
  qosDenials: SorafsTaikaiQosCounts;
}

export interface SorafsTaikaiCacheQueue {
  pendingSegments: number;
  pendingBytes: number;
  pendingBatches: number;
  inFlightBatches: number;
  hedgedBatches: number;
  shaperDenials: SorafsTaikaiQosCounts;
  droppedSegments: number;
  failovers: number;
  openCircuits: number;
}

export interface SorafsGatewayFetchResult {
  manifestIdHex: string;
  chunkerHandle: string;
  chunkCount: number;
  assembledBytes: number | bigint;
  payload: Buffer;
  telemetryRegion: string | null;
  anonymity: SorafsGatewayAnonymityReport;
  providerReports: ReadonlyArray<SorafsGatewayProviderReport>;
  chunkReceipts: ReadonlyArray<SorafsGatewayChunkReceipt>;
  localProxyManifest: Record<string, unknown> | null;
  carVerification: SorafsGatewayCarVerification | null;
  metadata: SorafsGatewayScoreboardMetadata;
  scoreboard: ReadonlyArray<SorafsGatewayScoreboardEntry> | null | undefined;
  taikaiCacheSummary?: SorafsTaikaiCacheSummary | null;
  taikaiCacheQueue?: SorafsTaikaiCacheQueue | null;
}

export type SorafsGatewayFetchErrorCode =
  | "no_providers"
  | "no_healthy_providers"
  | "no_compatible_providers"
  | "exhausted_retries"
  | "observer_failed"
  | "internal_invariant"
  | "unknown";

export interface SorafsGatewayFetchAttemptFailure {
  kind: "provider" | "invalid_chunk";
  message?: string;
  reason?: Record<string, unknown>;
  policyBlock?: {
    observedStatus: number;
    canonicalStatus: number;
    code?: string;
    cacheVersion?: string;
    denylistVersion?: string;
    proofTokenPresent?: boolean;
    message?: string | null;
  };
}

export interface SorafsGatewayFetchAttemptError {
  providerId: string;
  failure: SorafsGatewayFetchAttemptFailure;
}

export interface SorafsGatewayCapabilityIssue {
  providerId: string;
  reason: string;
  chunkLength?: number;
  maxSpan?: number;
  offset?: number;
  length?: number;
  requiredAlignment?: number;
  burstLimit?: number;
}

export class SorafsGatewayFetchError extends Error {
  readonly kind: string;
  readonly code: SorafsGatewayFetchErrorCode;
  readonly retryable: boolean;
  readonly chunkIndex: number | null;
  readonly attempts: number | null;
  readonly lastError: SorafsGatewayFetchAttemptError | null;
  readonly providers: ReadonlyArray<SorafsGatewayCapabilityIssue> | null;
  readonly observerError: string | null;
  readonly details: Record<string, unknown> | null;
  readonly original: Error | null;
  readonly payload: Record<string, unknown>;
  constructor(payload?: Record<string, unknown>, original?: Error | null);
}

export function sorafsGatewayFetch(
  manifestIdHex: string,
  chunkerHandle: string,
  planJson: string,
  providers: ReadonlyArray<SorafsGatewayProviderSpec>,
  options?: SorafsGatewayFetchOptions,
): SorafsGatewayFetchResult;

export const Torii: typeof import("./src/toriiClient.js");
export const Norito: typeof import("./src/norito.js");
export const Crypto: typeof import("./src/crypto.js");
export const OfflineQrStream: typeof import("./src/offlineQrStream.js");

export interface SoranetPuzzleParamsSnapshot {
  memoryKib: number;
  timeCost: number;
  lanes: number;
}

export interface SoranetTokenConfigSnapshot {
  enabled: boolean;
  suite: string | null;
  relayIdHex: string | null;
  issuerFingerprintHex: string | null;
  maxTtlSecs: number | null;
  minTtlSecs: number | null;
  defaultTtlSecs: number | null;
  clockSkewSecs: number | null;
  revocationIdsHex: ReadonlyArray<string>;
}

export interface SoranetPuzzleConfigSnapshot {
  required: boolean;
  difficulty: number;
  maxFutureSkewSecs: number;
  minTicketTtlSecs: number;
  ticketTtlSecs: number;
  puzzle: SoranetPuzzleParamsSnapshot | null;
  token: SoranetTokenConfigSnapshot;
}

export interface SoranetPuzzleTicketResponse {
  ticketB64: string;
  signedTicketB64: string | null;
  signedTicketFingerprintHex: string | null;
  difficulty: number;
  ttlSecs: number;
  expiresAt: number;
}

export interface SoranetAdmissionTokenResponse {
  tokenB64: string;
  tokenIdHex: string;
  issuedAt: number;
  expiresAt: number;
  ttlSecs: number;
  flags: number;
  issuerFingerprintHex: string;
  relayIdHex: string;
}

export interface SoranetPuzzleMintOptions {
  ttlSecs?: number | bigint | null;
  transcriptHashHex?: string | null;
  signed?: boolean | null;
  timeoutMs?: number | null;
  headers?: Record<string, string | null | undefined>;
  signal?: AbortSignal;
}

export interface SoranetTokenMintOptions extends SoranetPuzzleMintOptions {
  flags?: number;
  issuedAtUnix?: number | bigint | null;
}

export interface SoranetPuzzleClientOptions {
  fetchImpl?: typeof fetch;
  defaultHeaders?: Record<string, string>;
  timeoutMs?: number | null;
}

export class SoranetPuzzleError extends Error {
  readonly status: number;
  readonly body: string;
  constructor(status: number, body: string);
}

export class SoranetPuzzleClient {
  constructor(baseUrl: string, options?: SoranetPuzzleClientOptions);
  readonly baseUrl: string;
  getPuzzleConfig(
    options?: SoranetPuzzleMintOptions,
  ): Promise<SoranetPuzzleConfigSnapshot>;
  mintPuzzleTicket(
    options?: SoranetPuzzleMintOptions,
  ): Promise<SoranetPuzzleTicketResponse>;
  getTokenConfig(
    options?: SoranetPuzzleMintOptions,
  ): Promise<SoranetTokenConfigSnapshot>;
  mintAdmissionToken(
    transcriptHashHex: string,
    options?: SoranetTokenMintOptions,
  ): Promise<SoranetAdmissionTokenResponse>;
}

export interface ToriiClientConfigSource extends ToriiClientRetryOptions {
  retryTelemetryHook?: (event: ToriiRetryTelemetryEvent) => void;
  insecureTransportTelemetryHook?: (
    event: InsecureTransportTelemetryEvent,
  ) => void;
  torii?: {
    apiTokens?: ReadonlyArray<string>;
  };
  toriiClient?: ToriiClientRetryOptions & Record<string, unknown>;
}

export interface ResolvedToriiClientConfig {
  timeoutMs: number;
  maxRetries: number;
  backoffInitialMs: number;
  backoffMultiplier: number;
  maxBackoffMs: number;
  retryStatuses: Set<number>;
  retryMethods: Set<string>;
  defaultHeaders: Record<string, string>;
  authToken: string | null;
  apiToken: string | null;
  retryProfiles: Record<string, ToriiResolvedRetryProfile>;
  retryTelemetryHook: ((event: ToriiRetryTelemetryEvent) => void) | null;
  insecureTransportTelemetryHook:
    | ((event: InsecureTransportTelemetryEvent) => void)
    | null;
}

export type ToriiHealthStatus = { status: string } & Record<string, unknown>;

export interface ToriiClientOptions extends ToriiClientRetryOptions {
  fetchImpl?: typeof fetch;
  config?: ToriiClientConfigSource;
  allowInsecure?: boolean;
  sorafsAliasPolicy?: SorafsAliasPolicyOptions;
  onSorafsAliasWarning?: (warning: SorafsAliasWarning) => void;
  sorafsGatewayFetch?: typeof sorafsGatewayFetch;
  generateDaProofSummary?: typeof generateDaProofSummary;
  retryTelemetryHook?: (event: ToriiRetryTelemetryEvent) => void;
  insecureTransportTelemetryHook?: (
    event: InsecureTransportTelemetryEvent,
  ) => void;
}

export interface TransactionStatusPollOptions {
  signal?: AbortSignal;
  scope?: "local" | "auto" | "global";
  intervalMs?: number;
  timeoutMs?: number | null;
  maxAttempts?: number | null;
  successStatuses?: Iterable<string>;
  failureStatuses?: Iterable<string>;
  onStatus?: (
    status: string | null,
    payload: ToriiPipelineTransactionStatus | null,
    attempt: number,
  ) => void | Promise<void>;
}

export interface IsoBridgeSignerSnapshot {
  accountId: string;
  privateKey?: string | null;
}

export interface IsoBridgeAliasEntry {
  iban: string;
  accountId: string;
}

export interface IsoBridgeCurrencyBinding {
  currency: string;
  assetDefinition: string;
}

export interface IsoBridgeConfigSnapshot {
  enabled: boolean;
  dedupeTtlSecs: number;
  defaultProfile: string | null;
  profiles: ReadonlyArray<Record<string, unknown>>;
  storeDir: string | null;
  embeddedSignaturePolicy: string | null;
  signer: IsoBridgeSignerSnapshot | null;
  accountAliases: ReadonlyArray<IsoBridgeAliasEntry>;
  currencyAssets: ReadonlyArray<IsoBridgeCurrencyBinding>;
}

export interface RbcSamplingConfigSnapshot {
  enabled: boolean;
  maxSamplesPerRequest: number;
  maxBytesPerRequest: number;
  dailyByteBudget: number;
  ratePerMinute: number | null;
}

export interface ConnectConfigSnapshot {
  enabled: boolean;
  wsMaxSessions: number;
  wsPerIpMaxSessions: number;
  wsRatePerIpPerMin: number;
  sessionTtlMs: number;
  frameMaxBytes: number;
  sessionBufferMaxBytes: number;
  pingIntervalMs: number;
  pingMissTolerance: number;
  pingMinIntervalMs: number;
  dedupeTtlMs: number;
  dedupeCap: number;
  relayEnabled: boolean;
  relayStrategy: string;
  p2pTtlHops: number;
}

export interface ToriiFeatureConfigSnapshot {
  isoBridge: IsoBridgeConfigSnapshot | null;
  rbcSampling: RbcSamplingConfigSnapshot | null;
  connect: ConnectConfigSnapshot | null;
}

export interface ConnectStatusPolicySnapshot {
  wsMaxSessions: number;
  wsPerIpMaxSessions: number;
  wsRatePerIpPerMin: number;
  sessionTtlMs: number;
  frameMaxBytes: number;
  sessionBufferMaxBytes: number;
  relayEnabled: boolean;
  relayStrategy: string;
  relayEffectiveStrategy: string;
  relayP2pAttached: boolean;
  p2pTtlHops: number;
  heartbeatIntervalMs: number;
  heartbeatMissTolerance: number;
  heartbeatMinIntervalMs: number;
}

export interface ConnectStatusSnapshot {
  enabled: boolean;
  sessionsTotal: number;
  sessionsActive: number;
  perIpSessions: ReadonlyArray<{ ip: string; sessions: number }>;
  bufferedSessions: number;
  totalBufferBytes: number;
  dedupeSize: number;
  policy: ConnectStatusPolicySnapshot | null;
  framesInTotal: number;
  framesOutTotal: number;
  ciphertextTotal: number;
  dedupeDropsTotal: number;
  bufferDropsTotal: number;
  plaintextControlDropsTotal: number;
  monotonicDropsTotal: number;
  sequenceViolationClosesTotal: number;
  roleDirectionMismatchTotal: number;
  pingMissTotal: number;
  p2pRebroadcastsTotal: number;
  p2pRebroadcastSkippedTotal: number;
  p2pAuthFailuresTotal: number;
  p2pTtlDropsTotal: number;
  p2pUnknownSessionDropsTotal: number;
  p2pSessionClaimsInTotal: number;
  p2pSessionClaimsInstalledTotal: number;
  p2pSessionClaimConflictsTotal: number;
  p2pRoleConsumedTotal: number;
  p2pSessionTerminatedTotal: number;
}

export interface ConnectSessionResponse {
  sid: string;
  wallet_uri: string;
  app_uri: string;
  token_app: string;
  token_wallet: string;
  token_management: string;
  token_relay: string;
  extra: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export interface ConnectSidResult {
  sidBytes: Buffer;
  sidBase64Url: string;
  nonce: Buffer;
}

export interface ConnectKeyPair {
  publicKey: Buffer;
  privateKey: Buffer;
}

export interface ConnectSessionPreviewOptions {
  chainId: string;
  node?: string | null;
  nonce?: BinaryLike | null;
  appKeyPair?: {
    publicKey: BinaryLike;
    privateKey: BinaryLike;
  };
}

export interface ConnectSessionPreview {
  chainId: string;
  node: string | null;
  sidBytes: Buffer;
  sidBase64Url: string;
  nonce: Buffer;
  appKeyPair: ConnectKeyPair;
  walletUri: string;
  appUri: string;
}

export function generateConnectSid(options: {
  chainId: string;
  appPublicKey: BinaryLike;
  nonce?: BinaryLike | null;
}): ConnectSidResult;

export function createConnectSessionPreview(
  options: ConnectSessionPreviewOptions,
): ConnectSessionPreview;

export type ConnectQueueState =
  | "healthy"
  | "throttled"
  | "quarantined"
  | "disabled";

export interface ConnectQueueDirectionStats {
  depth: number;
  bytes: number;
  oldest_sequence: number | null;
  newest_sequence: number | null;
  oldest_timestamp_ms: number | null;
  newest_timestamp_ms: number | null;
}

export interface ConnectQueueSnapshot {
  schema_version: number;
  session_id_base64: string;
  state: ConnectQueueState;
  reason: string | null;
  warning_watermark: number;
  drop_watermark: number;
  last_updated_ms: number;
  app_to_wallet: ConnectQueueDirectionStats;
  wallet_to_app: ConnectQueueDirectionStats;
}

export interface ConnectQueueMetricsSample {
  timestamp_ms?: number;
  state?: ConnectQueueState;
  app_to_wallet_depth?: number;
  wallet_to_app_depth?: number;
  reason?: string | null;
}

export interface ConnectQueueEvidenceFiles {
  app_queue_filename?: string;
  wallet_queue_filename?: string;
  metrics_filename?: string;
}

export interface ConnectQueueEvidenceManifest {
  schema_version: number;
  session_id_base64: string;
  created_at_ms: number;
  snapshot: ConnectQueueSnapshot;
  files: ConnectQueueEvidenceFiles;
}

export interface ConnectQueueEvidenceExportResult {
  manifest: ConnectQueueEvidenceManifest;
  targetDir: string;
}

export interface ConnectQueueRootOptions {
  rootDir?: string;
  connectConfig?:
    | {
        connect?: {
          queue?: {
            root?: string;
            queue_root?: string;
          };
          queue_root?: string;
          queueRoot?: string;
        };
        connect_queue_root?: string;
        connectQueueRoot?: string;
      }
    | string;
  allowEnvOverride?: boolean;
}

export function defaultConnectQueueRoot(
  options?: ConnectQueueRootOptions,
): string;

export function deriveConnectSessionDirectory(
  options: { sid: BinaryLike | string } & ConnectQueueRootOptions,
): string;

export function readConnectQueueSnapshot(
  options: {
    sid?: BinaryLike | string;
    snapshotPath?: string;
    warningWatermark?: number;
    dropWatermark?: number;
  } & ConnectQueueRootOptions,
): Promise<{ snapshot: ConnectQueueSnapshot; statePath: string }>;

export function writeConnectQueueSnapshot(
  snapshot: ConnectQueueSnapshot,
  options?: ConnectQueueRootOptions & { sid?: BinaryLike | string },
): Promise<{ snapshot: ConnectQueueSnapshot; statePath: string }>;

export function updateConnectQueueSnapshot(
  sid: BinaryLike | string,
  updater:
    | Partial<ConnectQueueSnapshot>
    | ((snapshot: ConnectQueueSnapshot) => ConnectQueueSnapshot | void),
  options?: ConnectQueueRootOptions & {
    warningWatermark?: number;
    dropWatermark?: number;
  },
): Promise<ConnectQueueSnapshot>;

export function appendConnectQueueMetric(
  sid: BinaryLike | string,
  sample: ConnectQueueMetricsSample,
  options?: ConnectQueueRootOptions,
): Promise<string>;

export function exportConnectQueueEvidence(
  sid: BinaryLike | string,
  targetDir: string,
  options?: ConnectQueueRootOptions,
): Promise<ConnectQueueEvidenceExportResult>;

export interface BootstrapConnectPreviewOptions
  extends ConnectSessionPreviewOptions {
  register?: boolean;
  sessionOptions?: {
    node?: string | null;
  } | null;
}

export interface BootstrapConnectPreviewResult {
  preview: ConnectSessionPreview;
  session: ConnectSessionResponse | null;
  tokens: {
    wallet: string;
    app: string;
    management: string;
    relay: string;
  } | null;
}

export function bootstrapConnectPreviewSession(
  toriiClient: Pick<ToriiClient, "createConnectSession">,
  options: BootstrapConnectPreviewOptions,
): Promise<BootstrapConnectPreviewResult>;

export interface ConnectAppRecord {
  appId: string;
  displayName: string | null;
  description: string | null;
  iconUrl: string | null;
  namespaces: ReadonlyArray<string>;
  metadata: Record<string, unknown>;
  policy: Record<string, unknown>;
  extra: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export interface ConnectAppRegistryPage {
  items: ReadonlyArray<ConnectAppRecord>;
  total: number | null;
  nextCursor: string | null;
  extra: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export interface ConnectAppPolicyControls {
  relayEnabled: boolean | null;
  wsMaxSessions: number | null;
  wsPerIpMaxSessions: number | null;
  wsRatePerIpPerMin: number | null;
  sessionTtlMs: number | null;
  frameMaxBytes: number | null;
  sessionBufferMaxBytes: number | null;
  pingIntervalMs: number | null;
  pingMissTolerance: number | null;
  pingMinIntervalMs: number | null;
  extra: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export interface ConnectAppUpsertInput {
  appId: string;
  displayName?: string | null;
  description?: string | null;
  iconUrl?: string | null;
  namespaces?: ReadonlyArray<string>;
  metadata?: Record<string, unknown>;
  policy?: Record<string, unknown>;
  extra?: Record<string, unknown>;
}

export interface ConnectAppPolicyUpdate {
  relayEnabled?: boolean | null;
  wsMaxSessions?: number | null;
  wsPerIpMaxSessions?: number | null;
  wsRatePerIpPerMin?: number | null;
  sessionTtlMs?: number | null;
  frameMaxBytes?: number | null;
  sessionBufferMaxBytes?: number | null;
  pingIntervalMs?: number | null;
  pingMissTolerance?: number | null;
  pingMinIntervalMs?: number | null;
  extra?: Record<string, unknown>;
}

export interface ConnectAdmissionManifestEntry {
  appId: string;
  namespaces: ReadonlyArray<string>;
  metadata: Record<string, unknown>;
  policy: Record<string, unknown>;
  extra: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export interface ConnectAdmissionManifest {
  version: number | null;
  entries: ReadonlyArray<ConnectAdmissionManifestEntry>;
  manifestHash: string | null;
  updatedAt: string | null;
  extra: Record<string, unknown>;
  raw?: Record<string, unknown>;
}

export type ConnectAdmissionManifestInput =
  | ConnectAdmissionManifest
  | {
      manifest?: unknown;
      entries?: ReadonlyArray<Record<string, unknown>>;
      apps?: ReadonlyArray<Record<string, unknown>>;
      version?: number | string | null;
      manifestHash?: string | null;
      manifest_hash?: string | null;
      updatedAt?: string | null;
      updated_at?: string | null;
      [key: string]: unknown;
    };

export type ConnectWebSocketProtocols = string | ReadonlyArray<string>;

export type ConnectWebSocketConstructor<T = unknown> = new (
  url: string,
  protocols?: ConnectWebSocketProtocols,
  options?: unknown,
) => T;

export interface ConnectWebSocketParams {
  sid: string;
  role: "app" | "wallet";
  token: string;
  endpointPath?: string;
  allowInsecure?: boolean;
}

export interface ConnectWebSocketDialOptions<T = unknown>
  extends ConnectWebSocketParams {
  baseUrl: string;
  protocols?: ConnectWebSocketProtocols;
  websocketOptions?: unknown;
  WebSocketImpl?: ConnectWebSocketConstructor<T>;
  insecureTransportTelemetryHook?: (
    event: InsecureTransportTelemetryEvent,
  ) => void;
}

export interface ClientConnectWebSocketOptions<T = unknown>
  extends ConnectWebSocketParams {
  protocols?: ConnectWebSocketProtocols;
  websocketOptions?: unknown;
  WebSocketImpl?: ConnectWebSocketConstructor<T>;
  insecureTransportTelemetryHook?: (
    event: InsecureTransportTelemetryEvent,
  ) => void;
}

export interface ToriiSumeragiMembershipSnapshot {
  height: number;
  view: number;
  epoch: number;
  view_hash?: string | null;
}

/**
 * Aggregated TEU commitment for a Nexus lane recorded in the latest block.
 */
export interface ToriiLaneCommitmentSnapshot {
  block_height: number;
  lane_id: number;
  tx_count: number;
  total_chunks: number;
  rbc_bytes_total: number;
  teu_total: number;
  block_hash: string;
}

/**
 * Aggregated TEU commitment for a Nexus dataspace recorded in the latest block.
 */
export interface ToriiDataspaceCommitmentSnapshot {
  block_height: number;
  lane_id: number;
  dataspace_id: number;
  tx_count: number;
  total_chunks: number;
  rbc_bytes_total: number;
  teu_total: number;
  block_hash: string;
}

export interface ToriiLaneSettlementReceipt {
  source_id: string;
  local_amount_micro: number;
  xor_due_micro: number;
  xor_after_haircut_micro: number;
  xor_variance_micro: number;
  timestamp_ms: number;
}

export interface ToriiLaneSettlementCommitment {
  block_height: number;
  lane_id: number;
  dataspace_id: number;
  tx_count: number;
  total_local_micro: number;
  total_xor_due_micro: number;
  total_xor_after_haircut_micro: number;
  total_xor_variance_micro: number;
  swap_metadata: Record<string, unknown> | null;
  receipts: ReadonlyArray<ToriiLaneSettlementReceipt>;
}

export interface ToriiLaneFastpqProofMaterial {
  proof_digest: string;
  verified_at_height: number | null;
}

export interface ToriiLaneRelayEnvelope {
  lane_id: number;
  dataspace_id: number;
  block_height: number;
  block_header: Record<string, unknown>;
  qc: Record<string, unknown> | null;
  da_commitment_hash: string | null;
  settlement_commitment: ToriiLaneSettlementCommitment;
  settlement_hash: string;
  rbc_bytes_total: number;
  manifest_root: string | null;
  fastpq_proof: ToriiLaneFastpqProofMaterial | null;
}

export interface ToriiLaneRuntimeUpgradeHookSnapshot {
  allow: boolean;
  require_metadata: boolean;
  metadata_key?: string | null;
  allowed_ids: string[];
}

export interface ToriiLaneMerkleCommitmentSnapshot {
  root: string;
  max_depth: number;
}

export interface ToriiLaneSnarkCommitmentSnapshot {
  circuit_id: number;
  verifying_key_digest: string;
  statement_hash: string;
  proof_hash: string;
}

export interface ToriiLanePrivacyCommitmentSnapshot {
  id: number;
  scheme: "merkle" | "snark";
  merkle?: ToriiLaneMerkleCommitmentSnapshot | null;
  snark?: ToriiLaneSnarkCommitmentSnapshot | null;
}

export interface ToriiLaneGovernanceSnapshot {
  lane_id: number;
  alias: string;
  dataspace_id: number;
  visibility: string;
  storage_profile: string;
  governance?: string | null;
  manifest_required: boolean;
  manifest_ready: boolean;
  manifest_path?: string | null;
  validator_ids: string[];
  quorum?: number | null;
  protected_namespaces: string[];
  runtime_upgrade?: ToriiLaneRuntimeUpgradeHookSnapshot | null;
  privacy_commitments: ToriiLanePrivacyCommitmentSnapshot[];
}

export interface ToriiGovernanceProposalSnapshot {
  proposed: number;
  approved: number;
  rejected: number;
  enacted: number;
}

export interface ToriiGovernanceProtectedNamespaceSnapshot {
  total_checks: number;
  allowed: number;
  rejected: number;
}

export interface ToriiGovernanceManifestAdmissionSnapshot {
  total_checks: number;
  allowed: number;
  missing_manifest: number;
  non_validator_authority: number;
  quorum_rejected: number;
  protected_namespace_rejected: number;
  runtime_hook_rejected: number;
}

export interface ToriiGovernanceManifestQuorumSnapshot {
  total_checks: number;
  satisfied: number;
  rejected: number;
}

export interface ToriiGovernanceManifestActivationSnapshot {
  contract_address: string;
  code_hash_hex: string;
  abi_hash_hex?: string | null;
  height: number;
  activated_at_ms: number;
}

export interface ToriiGovernanceStatusSnapshot {
  proposals: ToriiGovernanceProposalSnapshot;
  protected_namespace: ToriiGovernanceProtectedNamespaceSnapshot;
  manifest_admission: ToriiGovernanceManifestAdmissionSnapshot;
  manifest_quorum: ToriiGovernanceManifestQuorumSnapshot;
  recent_manifest_activations: ReadonlyArray<ToriiGovernanceManifestActivationSnapshot>;
  sealed_lanes_total: number;
  sealed_lane_aliases: ReadonlyArray<string>;
}

export type ToriiGovernanceProposalStatus =
  | "Proposed"
  | "Approved"
  | "Rejected"
  | "Enacted";

export interface ToriiGovernanceDeployContractProposal {
  contract_address: string;
  code_hash_hex: string;
  abi_hash_hex: string;
  abi_version: string;
}

export interface ToriiGovernanceContractResponse {
  found: boolean;
  contract_address: string;
  dataspace: string | null;
  code_hash_hex: string | null;
}

export interface ToriiGovernanceProposalKind {
  variant: string;
  deploy_contract: ToriiGovernanceDeployContractProposal | null;
  raw: Record<string, unknown>;
}

export interface ToriiGovernanceProposalRecord {
  proposer: string;
  created_height: number;
  status: ToriiGovernanceProposalStatus;
  kind: ToriiGovernanceProposalKind;
}

export interface ToriiGovernanceProposalResult {
  found: boolean;
  proposal: ToriiGovernanceProposalRecord | null;
}

export interface ToriiGovernanceReferendumResult {
  found: boolean;
  referendum: Record<string, unknown> | null;
}

export interface ToriiGovernanceTally {
  referendum_id: string;
  approve: number;
  reject: number;
  abstain: number;
}

export interface ToriiGovernanceTallyResult {
  found: boolean;
  referendum_id: string;
  tally: ToriiGovernanceTally | null;
}

export interface ToriiGovernanceLockRecord {
  owner: string;
  amount: number;
  expiry_height: number;
  direction: number;
  duration_blocks: number;
}

export interface ToriiGovernanceLocksResult {
  found: boolean;
  referendum_id: string;
  locks: Record<string, ToriiGovernanceLockRecord>;
}

export interface ToriiGovernanceUnlockStats {
  height_current: number;
  expired_locks_now: number;
  referenda_with_expired: number;
  last_sweep_height: number;
}

export type ToriiGovernanceCouncilVariant = "Normal" | "Small" | string;

export interface ToriiGovernanceCouncilMember {
  account_id: string;
}

export interface ToriiGovernanceCouncilCurrentResponse {
  epoch: number;
  members: ReadonlyArray<ToriiGovernanceCouncilMember>;
  alternates: ReadonlyArray<ToriiGovernanceCouncilMember>;
  candidate_count: number;
  verified: number;
  derived_by: string;
}

export interface ToriiGovernanceCouncilCandidateInput {
  accountId: string;
  variant: ToriiGovernanceCouncilVariant;
  pk?: BinaryLike | string;
  pkB64?: string;
  publicKey?: BinaryLike | string;
  publicKeyB64?: string;
  proof?: BinaryLike | string;
  proofB64?: string;
  signature?: BinaryLike | string;
  signatureB64?: string;
}

export interface ToriiGovernanceCouncilDeriveRequest {
  committeeSize?: number | string | bigint;
  alternateSize?: number | string | bigint;
  epoch?: number | string | bigint | null;
  candidates: ReadonlyArray<ToriiGovernanceCouncilCandidateInput>;
}

export interface ToriiGovernanceCouncilDeriveResponse {
  epoch: number;
  members: ReadonlyArray<ToriiGovernanceCouncilMember>;
  alternates: ReadonlyArray<ToriiGovernanceCouncilMember>;
  total_candidates: number;
  verified: number;
  derived_by: string;
}

export interface ToriiGovernanceCouncilPersistRequest
  extends ToriiGovernanceCouncilDeriveRequest {
  authority?: string | null;
  privateKey?: string | null;
}

export type ToriiGovernanceCouncilPersistResponse =
  ToriiGovernanceCouncilDeriveResponse;

export interface ToriiGovernanceCouncilReplaceRequest {
  missing: string;
  epoch?: number | string | bigint | null;
  authority?: string | null;
  privateKey?: string | null;
}

export interface ToriiGovernanceCouncilReplaceResponse {
  epoch: number;
  members: ReadonlyArray<ToriiGovernanceCouncilMember>;
  alternates: ReadonlyArray<ToriiGovernanceCouncilMember>;
  replaced: boolean;
}

export interface ToriiGovernanceCouncilAuditOptions {
  epoch?: number | string | bigint | null;
  signal?: AbortSignal;
}

export interface ToriiGovernanceCouncilAuditResponse {
  epoch: number;
  seed_hex: string;
  beacon_hex: string;
  members_count: number;
  candidate_count: number;
  alternates_count: number;
  verified: number;
  derived_by: string;
  chain_id: string;
}

export interface ToriiProtectedNamespacesApplyResponse {
  ok: boolean;
  applied: number;
}

export interface ToriiProtectedNamespacesGetResponse {
  found: boolean;
  namespaces: string[];
}

export interface ToriiGovernanceFinalizeRequest {
  referendumId: string;
  proposalId: string;
}

export interface ToriiGovernanceWindow {
  lower: number;
  upper: number;
}

export interface ToriiGovernanceEnactRequest {
  proposalId: string;
  preimageHash?: string | null;
  window?: ToriiGovernanceWindow | null;
}

export interface ToriiGovernanceDraftInstruction {
  wire_id: string;
  payload_hex?: string | null;
}

export interface ToriiGovernanceDraftResponse {
  ok: boolean;
  proposal_id: string | null;
  tx_instructions: ReadonlyArray<ToriiGovernanceDraftInstruction>;
  accepted?: boolean;
  reason?: string | null;
}

export interface MinistryAgendaProposalDraftRequest {
  proposal: Record<string, unknown>;
  authority: string;
}

export interface MinistryAgendaProposalDraftResponse {
  ok: boolean;
  agenda_proposal_id: string;
  authority: string;
  tx_instructions: ReadonlyArray<ToriiGovernanceDraftInstruction>;
  signable_transaction_b64: string;
}

export interface MinistryAgendaProposalRecord {
  proposal: Record<string, unknown>;
  authority: string;
  submitted_tx_hash_hex: string;
  submitted_height: number;
}

export interface MinistryAgendaProposalGetResponse {
  found: boolean;
  record: MinistryAgendaProposalRecord | null;
}

export type ToriiGovernanceBallotDirection = "Aye" | "Nay" | "Abstain";

export interface ToriiGovernanceDeployContractProposalRequest {
  contractAddress?: string;
  contractAlias?: string;
  codeHash: string | BinaryLike;
  abiHash: string | BinaryLike;
  abiVersion?: string;
  window?: ToriiGovernanceWindow | null;
  mode?: "Zk" | "Plain" | "zk" | "plain";
  limits?: JsonValue;
}

export interface ToriiGovernancePlainBallotRequest {
  authority: string;
  chainId: string;
  referendumId: string;
  owner: string;
  amount: NumericLike;
  durationBlocks: number | string | bigint;
  direction: ToriiGovernanceBallotDirection | string;
}

export interface ToriiGovernanceZkBallotRequest {
  authority: string;
  chainId: string;
  electionId: string;
  proof?: BinaryLike | string;
  proofB64?: BinaryLike | string;
  public?: JsonValue;
}

export interface ToriiGovernanceZkBallotV1Request {
  authority: string;
  chainId: string;
  electionId: string;
  backend: string;
  envelope?: BinaryLike | string;
  envelopeB64?: BinaryLike | string;
  root_hint?: string | BinaryLike | null;
  owner?: string | null;
  amount?: NumericLike | null;
  durationBlocks?: number | string | bigint | null;
  direction?: ToriiGovernanceBallotDirection | string | null;
  nullifier?: string | BinaryLike | null;
}

export interface ToriiGovernanceZkBallotProofRequest {
  authority: string;
  chainId: string;
  electionId: string;
  ballot: JsonValue;
}

export interface ToriiGovernanceBallotResponse
  extends ToriiGovernanceDraftResponse {
  accepted: boolean;
  reason: string | null;
}

export interface ToriiTriggerUpsertRequest {
  id: string;
  action: JsonValue | string;
  metadata?: JsonValue | null;
}

export interface ToriiTriggerMutationResponse {
  ok: boolean;
  trigger_id: string | null;
  tx_instructions: ReadonlyArray<ToriiGovernanceDraftInstruction>;
  accepted?: boolean;
  message?: string;
}

export interface ToriiTriggerRecord {
  id: string;
  action: JsonValue;
  metadata: JsonValue;
  raw: JsonValue;
}

export interface ToriiTriggerListPage {
  items: ReadonlyArray<ToriiTriggerRecord>;
  total: number;
}

export type SubscriptionPlan = Record<string, unknown>;
export type SubscriptionState = Record<string, unknown>;
export type SubscriptionInvoice = Record<string, unknown>;

export interface SubscriptionPlanCreateRequest {
  authority: string;
  planId: string;
  plan: SubscriptionPlan;
  privateKey?:
    | ArrayBufferView
    | ArrayBuffer
    | Buffer
    | ReadonlyArray<number>
    | string;
  privateKeyHex?: string;
  privateKeyMultihash?: string;
  privateKeyAlgorithm?: string;
}

export interface SubscriptionPlanCreateResponse {
  ok: boolean;
  plan_id: string;
  tx_hash_hex: string;
}

export interface SubscriptionPlanListItem {
  plan_id: string;
  plan: SubscriptionPlan;
}

export interface SubscriptionPlanListResponse {
  items: ReadonlyArray<SubscriptionPlanListItem>;
  total: number;
}

export interface SubscriptionCreateRequest {
  authority: string;
  subscriptionId: string;
  planId: string;
  billingTriggerId?: string;
  usageTriggerId?: string | null;
  firstChargeMs?: NumericLike;
  grantUsageToProvider?: boolean;
  privateKey?:
    | ArrayBufferView
    | ArrayBuffer
    | Buffer
    | ReadonlyArray<number>
    | string;
  privateKeyHex?: string;
  privateKeyMultihash?: string;
  privateKeyAlgorithm?: string;
}

export interface SubscriptionCreateResponse {
  ok: boolean;
  subscription_id: string;
  billing_trigger_id: string;
  usage_trigger_id?: string;
  first_charge_ms: number;
  tx_hash_hex: string;
}

export interface SubscriptionListItem {
  subscription_id: string;
  subscription: SubscriptionState;
  invoice?: SubscriptionInvoice | null;
  plan?: SubscriptionPlan | null;
}

export interface SubscriptionListResponse {
  items: ReadonlyArray<SubscriptionListItem>;
  total: number;
}

export interface SubscriptionGetResponse {
  subscription_id: string;
  subscription: SubscriptionState;
  invoice?: SubscriptionInvoice | null;
  plan?: SubscriptionPlan | null;
}

export interface SubscriptionActionRequest {
  authority: string;
  chargeAtMs?: NumericLike;
  cancelMode?: "immediate" | "period_end";
  privateKey?:
    | ArrayBufferView
    | ArrayBuffer
    | Buffer
    | ReadonlyArray<number>
    | string;
  privateKeyHex?: string;
  privateKeyMultihash?: string;
  privateKeyAlgorithm?: string;
}

export interface SubscriptionUsageRequest {
  authority: string;
  unitKey: string;
  delta: NumericLike;
  usageTriggerId?: string | null;
  privateKey?:
    | ArrayBufferView
    | ArrayBuffer
    | Buffer
    | ReadonlyArray<number>
    | string;
  privateKeyHex?: string;
  privateKeyMultihash?: string;
  privateKeyAlgorithm?: string;
}

export interface SubscriptionActionResponse {
  ok: boolean;
  subscription_id: string;
  tx_hash_hex: string;
}

export interface ToriiOfflineReadinessResponse {
  offline_note: boolean;
  offline_one_use_keys: boolean;
  offline_recursive_note_proof: boolean;
  offline_fountain_qr: boolean;
  offline_sync_optional: boolean;
  offline_telemetry: boolean;
  [key: string]: unknown;
}

export interface ToriiStatusPayload {
  observed_at_ms: number;
  peers: number;
  queue_size: number;
  queue_queued: number;
  queue_inflight: number;
  last_block_committed_at_ms: number;
  last_non_empty_block_committed_at_ms: number;
  time_since_last_block_ms: number;
  time_since_last_non_empty_block_ms: number;
  commit_time_ms: number;
  da_reschedule_total: number;
  txs_approved: number;
  txs_rejected: number;
  view_changes: number;
  governance: ToriiGovernanceStatusSnapshot | null;
  lane_commitments: ToriiLaneCommitmentSnapshot[];
  dataspace_commitments: ToriiDataspaceCommitmentSnapshot[];
  lane_governance: ToriiLaneGovernanceSnapshot[];
  dataspace_catalog: ToriiDataspaceCatalogEntry[];
  lane_governance_sealed_total: number;
  lane_governance_sealed_aliases: ReadonlyArray<string>;
  raw: Record<string, unknown>;
}

export interface ToriiStatusMetrics {
  commit_latency_ms: number;
  queue_size: number;
  queue_queued: number;
  queue_inflight: number;
  queue_delta: number;
  time_since_last_block_ms: number;
  time_since_last_non_empty_block_ms: number;
  da_reschedule_delta: number;
  tx_approved_delta: number;
  tx_rejected_delta: number;
  view_change_delta: number;
  has_activity: boolean;
}

export interface ToriiDataspaceCatalogEntry {
  lane_id: number;
  lane_alias: string;
  dataspace_id: number;
  alias: string;
  visibility: string;
  storage_profile: string;
  manifest_required: boolean;
  manifest_ready: boolean;
  sealed: boolean;
  manifest_path: string | null;
  protected_namespaces: string[];
}

export interface ToriiStatusSnapshot {
  timestamp: number;
  status: ToriiStatusPayload;
  metrics: ToriiStatusMetrics;
}

export interface ToriiPipelinePreflight {
  schema_version: number;
  chain_height: number;
  sumeragi: {
    block_time_ms: number;
    commit_time_ms: number;
    stall_threshold_ms: number;
  };
  admission: {
    max_signatures: number;
    max_instructions: number;
    max_tx_bytes: number;
    max_decompressed_bytes: number;
    max_metadata_depth: number;
  };
  block: {
    max_transactions: number;
  };
  pipeline: {
    signature_batch_max: number;
    signature_batch_max_ed25519: number;
    signature_batch_max_secp256k1: number;
    signature_batch_max_pqc: number;
    signature_batch_max_bls: number;
    overlay_max_instructions: number;
    ivm_max_decoded_instructions: number;
  };
  queue: {
    size: number;
    queued: number;
    inflight: number;
  };
  fees: {
    fee_asset_id: string;
    fee_sink_account_id: string;
    base_fee: unknown;
    per_byte_fee: unknown;
    per_instruction_fee: unknown;
    per_gas_unit_fee: unknown;
    sponsorship_enabled: boolean;
    sponsor_max_fee: unknown;
    sponsor_verified_balance_safety_floor: unknown;
    canonical_sponsor_account_id: string | null;
    fee_receipts_activation_height: number;
    external_settlement_enabled: boolean;
    burn_from_unix_timestamp_ms: number;
    settlement_mode: string;
    successful_claim_fee_exempt_authorities: string[];
  };
  raw: Readonly<Record<string, unknown>>;
  isStatusStalled(
    status: ToriiStatusPayload | Record<string, unknown>,
  ): boolean;
}

export function statusLivenessElapsedMs(
  status: ToriiStatusPayload | Record<string, unknown>,
): number;

export function isStatusQueueStalled(
  status: ToriiStatusPayload | Record<string, unknown>,
  stallThresholdMs: number | string | bigint,
): boolean;

export interface ToriiNetworkTimeNow {
  timestampMs: number;
  offsetMs: number;
  confidenceMs: number;
}

export interface ToriiNetworkTimePeerSample {
  peer: string;
  lastOffsetMs: number;
  lastRttMs: number;
  count: number;
}

export interface ToriiNetworkTimeRttBucket {
  le: number;
  count: number;
}

export interface ToriiNetworkTimeRttHistogram {
  buckets: ReadonlyArray<ToriiNetworkTimeRttBucket>;
  sumMs: number;
  count: number;
}

export interface ToriiNetworkTimeStatus {
  peers: number;
  samples: ReadonlyArray<ToriiNetworkTimePeerSample>;
  rtt: ToriiNetworkTimeRttHistogram;
  note: string | null;
}

export interface ToriiNodeCapabilities {
  abiVersion: number;
  dataModelVersion: number;
  crypto: {
    sm: ToriiNodeSmCapabilities;
    curves: ToriiNodeCurveCapabilities;
  };
}

export interface ToriiNodeSmCapabilities {
  enabled: boolean;
  defaultHash: string | null;
  allowedSigning: ReadonlyArray<string>;
  sm2DistIdDefault: string | null;
  opensslPreview: boolean;
  acceleration: ToriiNodeSmAcceleration;
}

export interface ToriiNodeSmAcceleration {
  scalar: boolean;
  neonSm3: boolean;
  neonSm4: boolean;
  policy: string;
}

export interface ToriiNodeCurveCapabilities {
  registryVersion: number;
  allowedCurveIds: ReadonlyArray<number>;
  allowedCurveBitmap: ReadonlyArray<number>;
}

export interface ToriiSccpCodecCapability {
  id: number;
  key: string;
  description: string;
}

export interface ToriiSccpCounterpartyCapability {
  domain: number;
  chain: string;
  verifierBackendKey: string;
  messageBackend: string;
  registryBackend: string;
  counterpartyAccountCodec: number;
  counterpartyAccountCodecKey: string;
  destinationRollout: ToriiSccpDestinationRollout | null;
  productionReady: boolean;
  disabledReason: string | null;
}

export interface ToriiSccpCapabilities {
  localDomain: number;
  localChain: string;
  proofFamily: string;
  burnBundlePath: string;
  messageBundlePath: string;
  messageProofPath: string;
  messageJobPath: string;
  recentMessagesPath: string;
  proofManifestPath: string;
  burnRegistryBackend: string;
  proofSubmitPath: string | null;
  messageSubmitPath: string | null;
  messagePayloadKinds: ReadonlyArray<string>;
  codecs: ReadonlyArray<ToriiSccpCodecCapability>;
  counterparties: ReadonlyArray<ToriiSccpCounterpartyCapability>;
}

export type ToriiSccpProofFinalityModel =
  | "EthereumBeaconExecution"
  | "BscValidatorSet"
  | "SolanaFinalizedSlot"
  | "TonMasterchain"
  | "TronDpos"
  | "SubstrateGrandpa";

export type ToriiSccpProofVerifierTarget =
  | "EvmContract"
  | "SolanaProgram"
  | "TonContract"
  | "TronContract"
  | "SubstrateRuntime";

export type ToriiSccpProofSecurityModel = "RecursiveZk";

export type ToriiSccpAnchorGovernance = "CryptographicProof";

export interface ToriiSccpDestinationBinding {
  version: number;
  key: string;
  bindingHash: string;
}

export type ToriiSccpDestinationVerifierPlan =
  | "Unknown"
  | "EvmGroth16Bn254Adapter"
  | "SolanaProgramNativeRecursive"
  | "TonContractNativeRecursive"
  | "TronContractNativeRecursive"
  | "TronContractGroth16Bn254"
  | "SubstrateRuntimeNativeRecursive";

export interface ToriiSccpDestinationRollout {
  version: number;
  verifierPlan: ToriiSccpDestinationVerifierPlan;
  immutableVerifierReady: boolean;
  anchorsReady: boolean;
  verifierIdentity: string | null;
  verifierCodeHash: string | null;
  verifierKeyHash: string | null;
  destinationNetworkId: string | null;
  destinationBridgeAddress: string | null;
  destinationBindingKey: string | null;
  destinationBindingHash: string | null;
  anchorId: string | null;
  blockers: ReadonlyArray<string>;
}

export interface ToriiSccpTairaXorBurnRecordMaterial {
  settlementAssetDefinitionId: string;
  contractArtifactB64: string;
  vkRef: Readonly<{
    backend: string;
    name: string;
  }>;
  gasLimit?: number;
}

export interface ToriiSccpProofManifest {
  version: number;
  localDomain: number;
  localChain: string;
  counterpartyDomain: number;
  chain: string;
  proofFamily: string;
  securityModel: ToriiSccpProofSecurityModel;
  anchorGovernance: ToriiSccpAnchorGovernance;
  destinationBinding: ToriiSccpDestinationBinding;
  verifierBackendKey: string;
  messageBackend: string;
  registryBackend: string;
  counterpartyAccountCodec: number;
  counterpartyAccountCodecKey: string;
  finalityModel: ToriiSccpProofFinalityModel;
  verifierTarget: ToriiSccpProofVerifierTarget;
  manifestSeed: string;
  requiredPublicInputs: ReadonlyArray<string>;
  messagePayloadKinds: ReadonlyArray<string>;
  destinationRollout: ToriiSccpDestinationRollout | null;
  productionReady: boolean;
  disabledReason: string | null;
  submissionTemplate: ToriiSccpCounterpartySubmissionTemplate;
  tairaXorBurnRecord: ToriiSccpTairaXorBurnRecordMaterial | null;
}

export interface ToriiSccpProofManifestSet {
  localDomain: number;
  localChain: string;
  proofFamily: string;
  manifests: ReadonlyArray<ToriiSccpProofManifest>;
  routes: ReadonlyArray<Record<string, unknown>>;
}

export type ToriiSccpHubMessageKind =
  | "Burn"
  | "TokenAdd"
  | "TokenPause"
  | "TokenResume"
  | "AssetRegister"
  | "RouteActivate"
  | "Transfer";

export type ToriiSccpMessagePayloadKind =
  | "AssetRegister"
  | "RouteActivate"
  | "Transfer"
  | "TokenAdd"
  | "TokenPause"
  | "TokenResume";

export interface ToriiSccpHubCommitment {
  version: number;
  kind: ToriiSccpHubMessageKind;
  targetDomain: number;
  messageId: string;
  payloadHash: string;
}

export interface ToriiSccpMerkleStep {
  siblingHash: string;
  siblingIsLeft: boolean;
}

export interface ToriiSccpMerkleProof {
  steps: ReadonlyArray<ToriiSccpMerkleStep>;
}

export interface ToriiSccpPayloadEnvelope {
  kind: ToriiSccpMessagePayloadKind;
  value: Readonly<Record<string, unknown>>;
}

export interface ToriiSccpMessageProofBundle {
  version: number;
  commitmentRoot: string;
  commitment: ToriiSccpHubCommitment;
  merkleProof: ToriiSccpMerkleProof;
  payload: ToriiSccpPayloadEnvelope;
  finalityProof: string;
}

export interface ToriiSccpMessageTransparentPublicInputs {
  version: number;
  messageId: string;
  payloadHash: string;
  targetDomain: number;
  commitmentRoot: string;
  finalityHeight: number;
  finalityBlockHash: string;
}

export interface ToriiSccpEvmWordPublicInputs {
  messageId: string;
  payloadHash: string;
  targetDomainWord: string;
  commitmentRoot: string;
  finalityHeightWord: string;
  finalityBlockHash: string;
}

export type ToriiSccpPlatformSubmissionPayload =
  | {
      kind: "evm_contract_call" | "tron_contract_call";
      value: {
        proofBytes: string;
        publicInputs: ToriiSccpEvmWordPublicInputs;
        statementHash: string;
      };
    }
  | {
      kind: "solana_program_instruction";
      value: {
        proofBytes: string;
        publicInputsBytes: string;
        bundleBytes: string;
        destinationBinding: ToriiSccpDestinationBinding;
        destinationBindingHash: string;
        statementHash: string;
        proofContextHash: string;
      };
    }
  | {
      kind: "substrate_runtime_call";
      value: {
        proofBytes: string;
        publicInputsBytes: string;
        bundleBytes: string;
      };
    }
  | {
      kind: "ton_internal_message";
      value: {
        messageBodyBoc: string;
        queryId: number;
        destinationBinding: ToriiSccpDestinationBinding;
        destinationBindingHash: string;
        proofBytes: string;
        publicInputsBytes: string;
        bundleBytes: string;
        statementHash: string;
      };
    };

export interface ToriiSccpSubmissionArgumentValue {
  key: string;
  encoding: string;
  bytes: string;
}

export interface ToriiSccpCounterpartySubmissionPackage {
  version: number;
  proofFamily: string;
  verifierBackendKey: string;
  envelopeEncoding: string;
  submissionKind: string;
  verifierEntrypoint: string;
  platformPayload: ToriiSccpPlatformSubmissionPayload;
  arguments: ReadonlyArray<ToriiSccpSubmissionArgumentValue>;
  envelopeBytes: string;
}

export interface ToriiSccpMessageTransparentProofArtifact {
  version: number;
  localDomain: number;
  counterpartyDomain: number;
  proofFamily: string;
  securityModel: ToriiSccpProofSecurityModel;
  anchorGovernance: ToriiSccpAnchorGovernance;
  destinationBinding: ToriiSccpDestinationBinding;
  verifierBackendKey: string;
  messageBackend: string;
  registryBackend: string;
  manifestSeed: string;
  finalityModel: ToriiSccpProofFinalityModel;
  verifierTarget: ToriiSccpProofVerifierTarget;
  publicInputs: ToriiSccpMessageTransparentPublicInputs;
  proofBytes: string;
  submissionPackage: ToriiSccpCounterpartySubmissionPackage;
  bundle: ToriiSccpMessageProofBundle;
}

export interface ToriiSccpSubmissionArgument {
  key: string;
  description: string;
}

export interface ToriiSccpCounterpartySubmissionTemplate {
  version: number;
  encoding: string;
  submissionKind: string;
  verifierEntrypoint: string;
  requiredArguments: ReadonlyArray<ToriiSccpSubmissionArgument>;
}

export type ToriiSccpChainFamily =
  | "Evm"
  | "Solana"
  | "Ton"
  | "Tron"
  | "Substrate";

export type ToriiSccpNormalizedCodecValue =
  | { kind: "TextUtf8"; value: string }
  | { kind: "EvmHex"; bytes: string }
  | { kind: "SolanaBase58"; bytes: string }
  | { kind: "TonRaw"; workchain: number; account: string }
  | {
      kind: "TronBase58Check";
      /** Lowercase hex for the decoded 21-byte TRON payload: 0x41 prefix plus account bytes. */
      payload: string;
    };

export type ToriiSccpPayloadProjection =
  | {
      kind: "AssetRegister";
      value: {
        version: number;
        target_domain: number;
        home_domain: number;
        nonce: number;
        asset_id: ToriiSccpNormalizedCodecValue;
        decimals: number;
      };
    }
  | {
      kind: "RouteActivate";
      value: {
        version: number;
        source_domain: number;
        target_domain: number;
        nonce: number;
        asset_id: ToriiSccpNormalizedCodecValue;
        route_id: ToriiSccpNormalizedCodecValue;
      };
    }
  | {
      kind: "Transfer";
      value: {
        version: number;
        source_domain: number;
        dest_domain: number;
        nonce: number;
        asset_home_domain: number;
        asset_id: ToriiSccpNormalizedCodecValue;
        amount: number;
        sender: ToriiSccpNormalizedCodecValue;
        recipient: ToriiSccpNormalizedCodecValue;
        route_id: ToriiSccpNormalizedCodecValue;
      };
    };

export interface ToriiSccpCounterpartyProofJob {
  version: number;
  chainFamily: ToriiSccpChainFamily;
  chain: string;
  localDomain: number;
  counterpartyDomain: number;
  proofFamily: string;
  securityModel: ToriiSccpProofSecurityModel;
  anchorGovernance: ToriiSccpAnchorGovernance;
  destinationBinding: ToriiSccpDestinationBinding;
  verifierBackendKey: string;
  messageBackend: string;
  registryBackend: string;
  manifestSeed: string;
  finalityModel: ToriiSccpProofFinalityModel;
  verifierTarget: ToriiSccpProofVerifierTarget;
  publicInputs: ToriiSccpMessageTransparentPublicInputs;
  payloadKind: string;
  payloadProjection: ToriiSccpPayloadProjection;
  submissionTemplate: ToriiSccpCounterpartySubmissionTemplate;
  submissionPackage: ToriiSccpCounterpartySubmissionPackage;
  bundle: ToriiSccpMessageProofBundle;
}

export interface ToriiBridgeProofSubmitPayload {
  authority: string;
  privateKey?: unknown;
  private_key?: unknown;
  publicKeyHex?: string;
  public_key_hex?: string;
  signatureB64?: string;
  signature_b64?: string;
  burnBundle?: Record<string, unknown>;
  burn_bundle?: Record<string, unknown>;
  messageBundle?: Record<string, unknown>;
  message_bundle?: Record<string, unknown>;
  networkIdHex?: BinaryLike;
  network_id_hex?: BinaryLike;
  verifierAddressHex?: string;
  verifier_address_hex?: string;
  bridgeAddressHex?: string;
  bridge_address_hex?: string;
  verifierCodeHashHex?: BinaryLike;
  verifier_code_hash_hex?: BinaryLike;
  verifierKeyHashHex?: BinaryLike;
  verifier_key_hash_hex?: BinaryLike;
  expectedDestinationBindingHashHex?: BinaryLike;
  expected_destination_binding_hash_hex?: BinaryLike;
  /** Checksummed TRON Base58Check verifier contract address. */
  tronVerifierAddress?: string;
  /** Checksummed TRON Base58Check verifier contract address. */
  tron_verifier_address?: string;
  proofBytesHex?: BinaryLike;
  proof_bytes_hex?: BinaryLike;
  creationTimeMs?: NumericLike;
  creation_time_ms?: NumericLike;
}

export interface ToriiBridgeMessageSubmitPayload {
  authority: string;
  privateKey?: unknown;
  private_key?: unknown;
  publicKeyHex?: string;
  public_key_hex?: string;
  signatureB64?: string;
  signature_b64?: string;
  messageBundle?: Record<string, unknown>;
  message_bundle?: Record<string, unknown>;
  networkIdHex?: BinaryLike;
  network_id_hex?: BinaryLike;
  verifierAddressHex?: string;
  verifier_address_hex?: string;
  bridgeAddressHex?: string;
  bridge_address_hex?: string;
  verifierCodeHashHex?: BinaryLike;
  verifier_code_hash_hex?: BinaryLike;
  verifierKeyHashHex?: BinaryLike;
  verifier_key_hash_hex?: BinaryLike;
  expectedDestinationBindingHashHex?: BinaryLike;
  expected_destination_binding_hash_hex?: BinaryLike;
  /** Checksummed TRON Base58Check verifier contract address. */
  tronVerifierAddress?: string;
  /** Checksummed TRON Base58Check verifier contract address. */
  tron_verifier_address?: string;
  proofBytesHex?: BinaryLike;
  proof_bytes_hex?: BinaryLike;
  receiptLane?: NumericLike;
  receipt_lane?: NumericLike;
  settlement?: Record<string, unknown>;
  creationTimeMs?: NumericLike;
  creation_time_ms?: NumericLike;
}

export interface ToriiSccpEvmDestinationQueryOptions {
  signal?: AbortSignal;
  networkIdHex?: BinaryLike;
  verifierAddressHex?: string;
  bridgeAddressHex?: string;
  verifierCodeHashHex?: BinaryLike;
  verifierKeyHashHex?: BinaryLike;
  expectedDestinationBindingHashHex?: BinaryLike;
  /** Checksummed TRON Base58Check verifier contract address. */
  tronVerifierAddress?: string;
  proofBytesHex?: BinaryLike;
}

export interface ToriiLoggerConfig {
  level: string;
  filter: string | null;
}

export interface ToriiNetworkConfig {
  blockGossipSize: number;
  blockGossipPeriodMs: number;
  transactionGossipSize: number;
  transactionGossipPeriodMs: number;
}

export interface ToriiQueueConfig {
  capacity: number;
}

export interface ToriiConfigurationSnapshot {
  publicKeyHex: string;
  logger: ToriiLoggerConfig;
  network: ToriiNetworkConfig;
  queue: ToriiQueueConfig | null;
  confidentialGas: ConfidentialGasSchedule | null;
  transport: ToriiConfigurationTransport | null;
}

export interface ToriiRuntimeAbiActiveResponse {
  abiVersion: number;
}

export interface ToriiRuntimeAbiHashResponse {
  policy: string;
  abiHashHex: string;
}

export interface ToriiRuntimeMetrics {
  abiVersion: number;
  upgradeEventsTotal: ToriiRuntimeMetricsCounters;
}

export interface ToriiRuntimeMetricsCounters {
  proposed: number;
  activated: number;
  canceled: number;
}

export interface ToriiConfigurationTransport {
  noritoRpc: ToriiConfigurationTransportNoritoRpc | null;
  streaming: ToriiConfigurationStreaming | null;
}

export interface ToriiConfigurationTransportNoritoRpc {
  enabled: boolean;
  stage: string;
  requireMtls: boolean;
  canaryAllowlistSize: number;
}

export interface ToriiConfigurationStreaming {
  soranet: ToriiConfigurationStreamingSoranet | null;
}

export interface ToriiConfigurationStreamingSoranet {
  enabled: boolean;
  streamTag: string;
  exitMultiaddr: string;
  paddingBudgetMs: number | null;
  accessKind: string;
  garCategory: string;
  channelSalt: string;
  provisionSpoolDir: string;
  provisionWindowSegments: number;
  provisionQueueCapacity: number;
}

export interface ToriiRuntimeUpgradeManifestInput {
  name: string;
  description: string;
  abiVersion: number | string | bigint;
  abiHash: string | BinaryLike;
  startHeight: number | string | bigint;
  endHeight: number | string | bigint;
  addedSyscalls?: ReadonlyArray<number | string | bigint>;
  addedPointerTypes?: ReadonlyArray<number | string | bigint>;
}

export interface ToriiRuntimeUpgradeInstruction {
  wire_id: string;
  payload_hex?: string | null;
}

export interface ToriiRuntimeUpgradeTxResponse {
  ok: boolean;
  tx_instructions: ReadonlyArray<ToriiRuntimeUpgradeInstruction>;
}

export interface ToriiRuntimeUpgradeManifest {
  name: string;
  description: string;
  abiVersion: number;
  abiHashHex: string;
  addedSyscalls: ReadonlyArray<number>;
  addedPointerTypes: ReadonlyArray<number>;
  startHeight: number;
  endHeight: number;
}

export type ToriiRuntimeUpgradeStatus =
  | { kind: "Proposed" }
  | { kind: "Canceled" }
  | { kind: "ActivatedAt"; activatedHeight: number };

export interface ToriiRuntimeUpgradeRecord {
  manifest: ToriiRuntimeUpgradeManifest;
  status: ToriiRuntimeUpgradeStatus;
  proposer: string;
  createdHeight: number;
}

export interface ToriiRuntimeUpgradeListItem {
  idHex: string;
  record: ToriiRuntimeUpgradeRecord;
}

export interface ToriiPipelineStatusEntry {
  kind: string | null;
  content: unknown;
  raw: unknown;
}

export interface ToriiPipelineStatus {
  kind: string;
  hashHex: string | null;
  authority: string | null;
  status: ToriiPipelineStatusEntry | null;
  content: Readonly<Record<string, unknown>> | null;
  raw: Readonly<Record<string, unknown>>;
}

export interface ToriiPipelineDagSnapshot {
  fingerprintHex: string;
  keyCount: number;
}

export interface ToriiPipelineTxSnapshot {
  hashHex: string;
  reads: ReadonlyArray<string>;
  writes: ReadonlyArray<string>;
}

export interface ToriiPipelineRecoverySidecar {
  format: string;
  height: number;
  dag: ToriiPipelineDagSnapshot;
  txs: ReadonlyArray<ToriiPipelineTxSnapshot>;
}

export interface ToriiPipelineRecoveryFastpqProof {
  entryHash: string;
  batchIndex: number;
  parameter: string;
  transitionCount: number;
  traceCommitment: string;
  proofDigest: string;
  batchBase64: string | null;
  proofBase64: string | null;
  batchCompact: boolean | null;
  batchReconstructedFromBlock: boolean | null;
  batchReconstructionError: string | null;
  raw: Readonly<Record<string, unknown>>;
}

export interface ToriiPipelineRecoveryFastpqProofs {
  height: number;
  blockHashHex: string;
  proofs: ReadonlyArray<ToriiPipelineRecoveryFastpqProof>;
}

export interface ToriiSumeragiStatus {
  mode_tag?: string;
  staged_mode_tag?: string | null;
  staged_mode_activation_height?: number | null;
  mode_activation_lag_blocks?: number | null;
  consensus_caps?: ToriiConsensusCaps | null;
  commit_qc?: ToriiSumeragiCommitQcSummary | null;
  commit_quorum?: ToriiSumeragiCommitQuorumSummary | null;
  membership?: ToriiSumeragiMembershipSnapshot;
  lane_commitments?: ToriiLaneCommitmentSnapshot[];
  dataspace_commitments?: ToriiDataspaceCommitmentSnapshot[];
  lane_settlement_commitments?: ToriiLaneSettlementCommitment[];
  lane_relay_envelopes?: ToriiLaneRelayEnvelope[];
  lane_governance?: ToriiLaneGovernanceSnapshot[];
  lane_governance_sealed_total?: number;
  lane_governance_sealed_aliases?: ReadonlyArray<string>;
  da_reschedule_total?: number;
  [key: string]: unknown;
}

export interface ToriiConsensusCaps {
  collectors_k: number;
  redundant_send_r: number;
  da_enabled: boolean;
  rbc_chunk_max_bytes: number;
  rbc_session_ttl_ms: number;
  rbc_store_max_sessions: number;
  rbc_store_soft_sessions: number;
  rbc_store_max_bytes: number;
  rbc_store_soft_bytes: number;
}

export interface ToriiSumeragiCommitQcSummary {
  height: number;
  view: number;
  epoch: number;
  block_hash: string | null;
  validator_set_hash: string | null;
  validator_set_len: number;
  signatures_total: number;
}

export interface ToriiSumeragiCommitQcRecord {
  subject_block_hash: string;
  commit_qc: ToriiSumeragiCommitQc | null;
}

export interface ToriiSumeragiCommitQc {
  phase: string;
  parent_state_root: string;
  post_state_root: string;
  height: number;
  view: number;
  epoch: number;
  mode_tag: string;
  validator_set_hash: string;
  validator_set_hash_version: number;
  validator_set: ReadonlyArray<string>;
  signers_bitmap: string;
  bls_aggregate_signature: string;
}

export interface ToriiSumeragiCommitQuorumSummary {
  height: number;
  view: number;
  block_hash: string | null;
  signatures_present: number;
  signatures_counted: number;
  signatures_set_b: number;
  signatures_required: number;
  last_updated_ms: number;
}

export interface ToriiSumeragiPacemakerResponse {
  backoff_ms: number;
  rtt_floor_ms: number;
  jitter_ms: number;
  backoff_multiplier: number;
  rtt_floor_multiplier: number;
  max_backoff_ms: number;
  jitter_frac_permille: number;
  round_elapsed_ms: number;
  view_timeout_target_ms: number;
  view_timeout_remaining_ms: number;
}

export interface ToriiSumeragiQcEntry {
  height: number;
  view: number;
  subject_block_hash?: string | null;
}

export interface ToriiSumeragiQcSnapshot {
  highest_qc: ToriiSumeragiQcEntry;
  locked_qc: ToriiSumeragiQcEntry;
}

export interface ToriiSumeragiPhasesEmaSnapshot {
  propose_ms: number;
  collect_da_ms: number;
  collect_prevote_ms: number;
  collect_precommit_ms: number;
  collect_aggregator_ms: number;
  commit_ms: number;
  pipeline_total_ms: number;
}

export interface ToriiSumeragiPhasesSnapshot {
  propose_ms: number;
  collect_da_ms: number;
  collect_prevote_ms: number;
  collect_precommit_ms: number;
  collect_aggregator_ms: number;
  commit_ms: number;
  pipeline_total_ms: number;
  collect_aggregator_gossip_total: number;
  block_created_dropped_by_lock_total: number;
  block_created_hint_mismatch_total: number;
  block_created_proposal_mismatch_total: number;
  ema_ms: ToriiSumeragiPhasesEmaSnapshot;
}

export interface ToriiSumeragiPrfContext {
  height: number;
  view: number;
  epoch_seed?: string | null;
}

export interface ToriiSumeragiLeaderSnapshot {
  leader_index: number;
  prf: ToriiSumeragiPrfContext;
}

export interface ToriiSumeragiCollectorEntry {
  index: number;
  peer_id: string;
}

export interface ToriiSumeragiCollectorsPlan {
  consensus_mode: string;
  mode: string;
  topology_len: number;
  min_votes_for_commit: number;
  proxy_tail_index: number;
  height: number;
  view: number;
  collectors_k: number;
  redundant_send_r: number;
  epoch_seed?: string | null;
  collectors: ReadonlyArray<ToriiSumeragiCollectorEntry>;
  prf: ToriiSumeragiPrfContext;
}

export interface ToriiSumeragiParamsSnapshot {
  block_time_ms: number;
  commit_time_ms: number;
  max_clock_drift_ms: number;
  collectors_k: number;
  redundant_send_r: number;
  da_enabled: boolean;
  next_mode?: string | null;
  mode_activation_height?: number | null;
  chain_height: number;
}

export type SumeragiEvidenceKind =
  | "DoublePrepare"
  | "DoubleCommit"
  | "InvalidQc"
  | "InvalidProposal"
  | "Censorship";

export interface SumeragiEvidenceListOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  kind?: SumeragiEvidenceKind;
  signal?: AbortSignal;
}

export interface SumeragiEvidenceRecordBase {
  kind: string;
  recorded_height: number;
  recorded_view: number;
  recorded_ms: number;
}

export interface SumeragiDoubleVoteEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  kind: "DoublePrepare" | "DoubleCommit";
  phase: "Prepare" | "Commit" | "NewView";
  height: number;
  view: number;
  epoch: number;
  signer: string;
  block_hash_1: string;
  block_hash_2: string;
}

export interface SumeragiInvalidQcEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  kind: "InvalidQc";
  height: number;
  view: number;
  epoch: number;
  subject_block_hash: string;
  phase: string;
  reason: string;
}

export interface SumeragiInvalidProposalEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  kind: "InvalidProposal";
  height: number;
  view: number;
  epoch: number;
  subject_block_hash: string;
  payload_hash: string;
  reason: string;
}

export interface SumeragiCensorshipEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  kind: "Censorship";
  tx_hash: string;
  receipt_count: number;
  min_height: number;
  max_height: number;
  signers: ReadonlyArray<string>;
}

export interface SumeragiUnknownEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  detail?: string;
}

export type SumeragiEvidenceRecord =
  | SumeragiDoubleVoteEvidenceRecord
  | SumeragiInvalidQcEvidenceRecord
  | SumeragiInvalidProposalEvidenceRecord
  | SumeragiCensorshipEvidenceRecord
  | SumeragiUnknownEvidenceRecord;

export interface SumeragiEvidenceListResponse {
  total: number;
  items: ReadonlyArray<SumeragiEvidenceRecord>;
}

export interface SumeragiEvidenceCountResponse {
  count: number;
}

export interface SumeragiEvidenceSubmitRequest {
  evidence_hex: string;
  apiToken?: string;
}

export interface SumeragiEvidenceSubmitResponse {
  status: string;
  kind: string;
}

export type KaigiRelayHealthStatus = "healthy" | "degraded" | "unavailable";

export interface KaigiRelaySummary {
  relay_id: string;
  domain: string;
  bandwidth_class: number;
  hpke_fingerprint_hex: string;
  status?: KaigiRelayHealthStatus | null;
  reported_at_ms?: number | null;
}

export interface KaigiRelaySummaryList {
  total: number;
  items: ReadonlyArray<KaigiRelaySummary>;
}

export interface KaigiRelayDomainMetrics {
  domain: string;
  registrations_total: number;
  manifest_updates_total: number;
  failovers_total: number;
  health_reports_total: number;
}

export interface KaigiRelayDetail {
  relay: KaigiRelaySummary;
  hpke_public_key_b64: string;
  reported_call?: { domain_id: string; call_name: string } | null;
  reported_by?: string | null;
  notes?: string | null;
  metrics?: KaigiRelayDomainMetrics | null;
}

export interface KaigiRelayHealthSnapshot {
  healthy_total: number;
  degraded_total: number;
  unavailable_total: number;
  reports_total: number;
  registrations_total: number;
  failovers_total: number;
  domains: ReadonlyArray<KaigiRelayDomainMetrics>;
}

export interface KaigiRelayEventCallRef {
  domain: string;
  name: string;
}

export interface KaigiRelayRegistrationEvent {
  kind: "registration";
  domain: string;
  relay_id: string;
  bandwidth_class: number;
  hpke_fingerprint_hex: string;
}

export interface KaigiRelayHealthEvent {
  kind: "health";
  domain: string;
  relay_id: string;
  status: KaigiRelayHealthStatus;
  reported_at_ms: number;
  call: KaigiRelayEventCallRef;
}

export type KaigiRelayEventPayload =
  | KaigiRelayRegistrationEvent
  | KaigiRelayHealthEvent;

export interface KaigiRelayEventsOptions {
  domain?: string;
  relay?: string;
  kind?: string | ReadonlyArray<string>;
  lastEventId?: string;
  signal?: AbortSignal;
}

export interface KaigiCallEventRef {
  call_id: string;
  domain: string;
  call_name: string;
}

export interface KaigiCallView {
  call_id: string;
  domain: string;
  call_name: string;
  host_account_id?: string | null;
  billing_account_id?: string | null;
  title?: string | null;
  description?: string | null;
  max_participants?: number | null;
  gas_rate_per_minute: number;
  metadata: Record<string, unknown>;
  scheduled_start_ms?: number | null;
  privacy_mode: string;
  room_policy: string;
  relay_manifest?: Record<string, unknown> | null;
  roster_root_hex: string;
  participant_count: number;
  commitment_count: number;
  nullifier_count: number;
  usage_commitment_count: number;
  status: string;
  created_at_ms: number;
  ended_at_ms?: number | null;
  total_duration_ms: number;
  total_billed_gas: number;
  segments_recorded: number;
}

export interface KaigiCallSignal {
  entrypoint_hash: string;
  authority?: string | null;
  timestamp_ms?: number | null;
  call_id: string;
  signal_kind: string;
  host_account_id?: string | null;
  participant_account_id?: string | null;
  created_at_ms: number;
  metadata: Record<string, unknown>;
}

export interface KaigiCallSignalsList {
  total: number;
  items: ReadonlyArray<KaigiCallSignal>;
}

export interface KaigiCallSignalsOptions {
  afterTimestampMs?: NumericLike;
  after_timestamp_ms?: NumericLike;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface KaigiCallRosterUpdatedEvent {
  kind: "roster_updated";
  call: KaigiCallEventRef;
  privacy_mode: string;
  participant_count: number;
  commitment_count: number;
  nullifier_count: number;
  roster_root_hex?: string | null;
}

export interface KaigiCallEndedEvent {
  kind: "ended";
  call: KaigiCallEventRef;
  status: string;
  ended_at_ms: number;
}

export type KaigiCallEventPayload =
  | KaigiCallRosterUpdatedEvent
  | KaigiCallEndedEvent;

export interface KaigiCallEventsOptions {
  kind?: string | ReadonlyArray<string>;
  lastEventId?: string;
  signal?: AbortSignal;
}

type ExclusiveSingleOrMany<
  SingleKey extends PropertyKey,
  SingleValue,
  ManyKey extends PropertyKey,
  ManyValue,
> =
  | ({ [K in SingleKey]: SingleValue } & { [K in ManyKey]?: never })
  | ({ [K in SingleKey]?: never } & { [K in ManyKey]: ManyValue });

type ExclusiveSingleOrManyOptional<
  SingleKey extends PropertyKey,
  SingleValue,
  ManyKey extends PropertyKey,
  ManyValue,
> =
  | ExclusiveSingleOrMany<SingleKey, SingleValue, ManyKey, ManyValue>
  | ({ [K in SingleKey]?: never } & { [K in ManyKey]?: never });

type DomainMintSpec = {
  assetId: string;
  quantity: NumericLike;
};

type AssetDefinitionMintSpec = {
  accountId?: string;
  assetHoldingId?: string;
  quantity: NumericLike;
};

type MintTransferSpec = {
  sourceAssetHoldingId?: string;
  quantity: NumericLike;
  destinationAccountId: string;
};

type AccountTransferSpec = {
  sourceAssetHoldingId: string;
  quantity: NumericLike;
  destinationAccountId: string;
};

export interface ConfidentialKeyset {
  skSpend: Buffer;
  nk: Buffer;
  ivk: Buffer;
  ovk: Buffer;
  fvk: Buffer;
  skSpendHex: string;
  nkHex: string;
  ivkHex: string;
  ovkHex: string;
  fvkHex: string;
  asHex(): Record<string, string>;
}

export interface ConfidentialReceiveAddressV2 {
  ownerTag: Buffer;
  ownerTagHex: string;
  diversifier: Buffer;
  diversifierHex: string;
}

export interface KaigiRosterJoinProof {
  commitment: Buffer;
  nullifier: Buffer;
  rosterRoot: Buffer;
  proof: Buffer;
  commitmentHex: string;
  nullifierHex: string;
  rosterRootHex: string;
  proofBase64: string;
}

export interface KaigiRosterJoinProofOptions {
  seed: ArrayBufferView | ArrayBuffer | Buffer;
  rosterRootHex?: string | null;
  roster_root_hex?: string | null;
}

export interface ZkAceTransferAuthorizationV1Options {
  fromAccountId: string;
  toAccountId: string;
  assetDefinitionId: string;
  amount: NumericLike;
  chainId: string;
  identityRoot: BinaryLike;
  identityBlinding: BinaryLike;
  replaySecret: BinaryLike;
  policyHash: BinaryLike;
  verifierKeyId?: string | null;
  verifyingKeyCommitment?: BinaryLike | null;
}

export interface ZkAceTransferAuthorizationV1 {
  publicInputs: object;
  public_inputs: object;
  proof: ProofAttachmentInput;
  identityCommitment: string;
  identity_commitment: string;
  txDigest: string;
  tx_digest: string;
  replayNullifier: string;
  replay_nullifier: string;
  policyHash: string;
  policy_hash: string;
  verifierKeyId: string;
  verifier_key_id: string;
  authorizationProofBytes: number;
  authorization_proof_bytes: number;
  authorizationPublicInputBytes: number;
  authorization_public_input_bytes: number;
  replayNullifierBytes: number;
  replay_nullifier_bytes: number;
}

export interface RegisterDomainInput {
  chainId: string;
  authority: string;
  domainId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

/**
 * Parameters for {@link buildTransaction}. The `instructions` array must be
 * non-empty and each entry should be either a builder result or a JSON string
 * produced by `noritoEncodeInstruction`.
 */
export interface TransactionAssemblyInput {
  chainId: string;
  authority: string;
  instructions: Array<object | string>;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface IvmProvedTransactionAssemblyInput {
  chainId: string;
  authority: string;
  proved: object | string;
  attachment: object | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface MintAssetInput {
  chainId: string;
  authority: string;
  assetHoldingId: string;
  quantity: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface BurnAssetInput {
  chainId: string;
  authority: string;
  assetHoldingId: string;
  quantity: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface MintTriggerInput {
  chainId: string;
  authority: string;
  triggerId: string;
  repetitions: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface BurnTriggerInput {
  chainId: string;
  authority: string;
  triggerId: string;
  repetitions: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface TransferAssetInput {
  chainId: string;
  authority: string;
  sourceAssetHoldingId: string;
  quantity: NumericLike;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface TransferDomainInput {
  chainId: string;
  authority: string;
  sourceAccountId: string;
  domainId: string;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface TransferAssetDefinitionInput {
  chainId: string;
  authority: string;
  sourceAccountId: string;
  assetDefinitionId: string;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface TransferNftInput {
  chainId: string;
  authority: string;
  sourceAccountId: string;
  nftId: string;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RwaParentRefInput {
  rwa?: string;
  rwaId?: string;
  quantity: NumericLike;
}

export interface RwaControlPolicyInput {
  controllerAccounts?: ReadonlyArray<string> | null;
  controller_accounts?: ReadonlyArray<string> | null;
  controllerRoles?: ReadonlyArray<string> | null;
  controller_roles?: ReadonlyArray<string> | null;
  freezeEnabled?: boolean | null;
  freeze_enabled?: boolean | null;
  holdEnabled?: boolean | null;
  hold_enabled?: boolean | null;
  forceTransferEnabled?: boolean | null;
  force_transfer_enabled?: boolean | null;
  redeemEnabled?: boolean | null;
  redeem_enabled?: boolean | null;
}

export interface RegisterRwaPayloadInput {
  domain: string;
  quantity: NumericLike;
  spec?: Record<string, unknown> | null;
  primaryReference?: string;
  primary_reference?: string;
  status?: string | null;
  metadata?: Record<string, JsonValue> | null;
  parents?: ReadonlyArray<RwaParentRefInput> | null;
  controls?: RwaControlPolicyInput | null;
}

export interface MergeRwasPayloadInput {
  parents: ReadonlyArray<RwaParentRefInput>;
  primaryReference?: string;
  primary_reference?: string;
  status?: string | null;
  metadata?: Record<string, JsonValue> | null;
}

export interface RegisterRwaInput {
  chainId: string;
  authority: string;
  rwa?: RegisterRwaPayloadInput | string;
  rwaJson?: RegisterRwaPayloadInput | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface TransferRwaInput {
  chainId: string;
  authority: string;
  sourceAccountId: string;
  rwaId: string;
  quantity: NumericLike;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface MergeRwasInput {
  chainId: string;
  authority: string;
  merge?: MergeRwasPayloadInput | string;
  mergeJson?: MergeRwasPayloadInput | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RedeemRwaInput {
  chainId: string;
  authority: string;
  rwaId: string;
  quantity: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface FreezeRwaInput {
  chainId: string;
  authority: string;
  rwaId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface UnfreezeRwaInput extends FreezeRwaInput {}

export interface HoldRwaInput {
  chainId: string;
  authority: string;
  rwaId: string;
  quantity: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface ReleaseRwaInput extends HoldRwaInput {}

export interface ForceTransferRwaInput {
  chainId: string;
  authority: string;
  rwaId: string;
  quantity: NumericLike;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface SetRwaControlsInput {
  chainId: string;
  authority: string;
  rwaId: string;
  controls?: RwaControlPolicyInput | string;
  controlsJson?: RwaControlPolicyInput | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface SetRwaKeyValueInput {
  chainId: string;
  authority: string;
  rwaId: string;
  key: string;
  value: JsonValue;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RemoveRwaKeyValueInput {
  chainId: string;
  authority: string;
  rwaId: string;
  key: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

/**
 * Parameters for {@link buildMintAndTransferTransaction}. Provide either
 * `transfer` or `transfers`; when `sourceAssetHoldingId` is omitted on a transfer the
 * helper reuses `mint.assetHoldingId` and enforces that at least one transfer is
 * present.
 */
interface MintAndTransferInputBase {
  chainId: string;
  authority: string;
  mint: {
    assetHoldingId: string;
    quantity: NumericLike;
  };
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

type MintAndTransferVariants = ExclusiveSingleOrMany<
  "transfer",
  MintTransferSpec,
  "transfers",
  ReadonlyArray<MintTransferSpec>
>;

export type MintAndTransferInput = MintAndTransferInputBase &
  MintAndTransferVariants;

/**
 * Parameters for {@link buildRegisterDomainAndMintTransaction}. Supply either
 * a single `mint` descriptor or an array of `mints`. When neither is provided
 * the helper will register the domain without minting.
 */
interface RegisterDomainAndMintInputBase {
  chainId: string;
  authority: string;
  domain: {
    domainId: string;
    logo?: string | null;
    metadata?: object | null;
  };
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

type RegisterDomainMintOptions = ExclusiveSingleOrManyOptional<
  "mint",
  DomainMintSpec,
  "mints",
  ReadonlyArray<DomainMintSpec>
>;

export type RegisterDomainAndMintInput = RegisterDomainAndMintInputBase &
  RegisterDomainMintOptions;

/**
 * Parameters for {@link buildRegisterAccountAndTransferTransaction}. Provide
 * either `transfer` or `transfers`; each transfer must declare a source asset
 * so the helper can enforce explicit provenance.
 */
interface RegisterAccountAndTransferInputBase {
  chainId: string;
  authority: string;
  account: {
    accountId: string;
    metadata?: object;
  };
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

type RegisterAccountTransferOptions = ExclusiveSingleOrManyOptional<
  "transfer",
  AccountTransferSpec,
  "transfers",
  ReadonlyArray<AccountTransferSpec>
>;

export type RegisterAccountAndTransferInput =
  RegisterAccountAndTransferInputBase & RegisterAccountTransferOptions;

/**
 * Parameters for {@link buildRegisterAssetDefinitionAndMintTransaction}. Supply
 * either `mint` or `mints`. When `assetHoldingId` is omitted the helper derives it as
 * the canonical asset-holding id for `assetDefinitionId + accountId`, and
 * enforces that any provided `assetHoldingId` matches the derived value.
 */
interface RegisterAssetDefinitionAndMintInputBase {
  chainId: string;
  authority: string;
  assetDefinition: {
    assetDefinitionId: string;
    metadata?: object;
    mintable?: string;
    logo?: string | null;
    spec?: object;
    confidentialPolicy?: object;
  };
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

type RegisterAssetDefinitionMintOptions = ExclusiveSingleOrManyOptional<
  "mint",
  AssetDefinitionMintSpec,
  "mints",
  ReadonlyArray<AssetDefinitionMintSpec>
>;

export type RegisterAssetDefinitionAndMintInput =
  RegisterAssetDefinitionAndMintInputBase & RegisterAssetDefinitionMintOptions;

/**
 * Extends {@link RegisterAssetDefinitionAndMintInput} with optional transfer
 * descriptors. Provide either `transfer` or `transfers`; when a transfer omits
 * `sourceAssetHoldingId` the helper reuses the first minted asset destination.
 */
type RegisterAssetDefinitionMintRequired = ExclusiveSingleOrMany<
  "mint",
  AssetDefinitionMintSpec,
  "mints",
  ReadonlyArray<AssetDefinitionMintSpec>
>;

type RegisterAssetDefinitionTransferOptions = ExclusiveSingleOrManyOptional<
  "transfer",
  MintTransferSpec,
  "transfers",
  ReadonlyArray<MintTransferSpec>
>;

export type RegisterAssetDefinitionMintAndTransferInput =
  RegisterAssetDefinitionAndMintInputBase &
    RegisterAssetDefinitionMintRequired &
    RegisterAssetDefinitionTransferOptions;

export type KaigiIdLike =
  | string
  | {
      domain_id?: string;
      domainId?: string;
      call_name?: string;
      callName?: string;
    };

export type KaigiPrivacyModeValue = {
  mode: "Transparent" | "ZkRosterV1";
  state?: unknown;
};

export type KaigiPrivacyModeInput =
  | string
  | KaigiPrivacyModeValue
  | null
  | undefined;

export interface KaigiRelayHopInput {
  relayId: string;
  hpkePublicKey: ArrayBufferView | ArrayBuffer | Buffer | string;
  weight?: NumericLike;
}

export interface KaigiRelayManifestInput {
  expiryMs: NumericLike;
  hops?: ReadonlyArray<KaigiRelayHopInput>;
}

export interface KaigiParticipantCommitmentInput {
  commitment: ArrayBufferView | ArrayBuffer | Buffer | string;
  aliasTag?: string | null;
}

export interface KaigiParticipantNullifierInput {
  digest: ArrayBufferView | ArrayBuffer | Buffer | string;
  issuedAtMs: NumericLike;
}

export type KaigiRoomPolicyInput =
  | "public"
  | "read-only"
  | "read_only"
  | "open"
  | "authenticated"
  | "auth"
  | "protected";

export interface CreateKaigiInput {
  id: KaigiIdLike;
  host: string;
  title?: string | null;
  description?: string | null;
  maxParticipants?: NumericLike | null;
  gasRatePerMinute?: NumericLike;
  metadata?: object | null;
  scheduledStartMs?: NumericLike | null;
  billingAccount?: string | null;
  privacyMode?: KaigiPrivacyModeInput;
  roomPolicy?: KaigiRoomPolicyInput;
  relayManifest?: KaigiRelayManifestInput | null;
  commitment?: KaigiParticipantCommitmentInput | null;
  nullifier?: KaigiParticipantNullifierInput | null;
  rosterRoot?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
  proof?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
}

export interface JoinKaigiInput {
  callId: KaigiIdLike;
  participant: string;
  commitment?: KaigiParticipantCommitmentInput | null;
  nullifier?: KaigiParticipantNullifierInput | null;
  rosterRoot?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
  proof?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
}

export interface LeaveKaigiInput extends JoinKaigiInput {}

export interface EndKaigiInput {
  callId: KaigiIdLike;
  endedAtMs?: NumericLike | null;
  commitment?: KaigiParticipantCommitmentInput | null;
  nullifier?: KaigiParticipantNullifierInput | null;
  rosterRoot?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
  proof?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
}

export interface RecordKaigiUsageInput {
  callId: KaigiIdLike;
  durationMs: NumericLike;
  billedGas?: NumericLike;
  usageCommitment?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
  proof?: ArrayBufferView | ArrayBuffer | Buffer | string | null;
}

export interface SetKaigiRelayManifestInput {
  callId: KaigiIdLike;
  relayManifest?: KaigiRelayManifestInput | null;
}

export interface RegisterKaigiRelayInput {
  relayId: string;
  hpkePublicKey: ArrayBufferView | ArrayBuffer | Buffer | string;
  bandwidthClass?: NumericLike;
}

export type GovernanceVotingMode = "Zk" | "Plain";

export interface GovernanceWindowInput {
  lower: NumericLike;
  upper: NumericLike;
}

export interface ProposeDeployContractInstructionInput {
  contractAddress?: string;
  contractAlias?: string;
  codeHash: HashLike;
  abiHash: HashLike;
  abiVersion?: string;
  window?: GovernanceWindowInput | null;
  votingMode?: GovernanceVotingMode | string | null;
}

export interface CastZkBallotInstructionInput {
  electionId: string;
  proof: ArrayBufferView | ArrayBuffer | Buffer | string;
  publicInputs: Record<string, unknown> | string;
}

export interface CastPlainBallotInstructionInput {
  referendumId: string;
  owner: string;
  amount: NumericLike;
  durationBlocks: NumericLike;
  direction?: number | string;
}

export interface EnactReferendumInstructionInput {
  referendumId: HashLike;
  preimageHash: HashLike;
  window?: GovernanceWindowInput | null;
}

export interface FinalizeReferendumInstructionInput {
  referendumId: string;
  proposalId: HashLike;
}

export interface PersistCouncilForEpochInstructionInput {
  epoch: NumericLike;
  members: ReadonlyArray<string>;
  alternates?: ReadonlyArray<string>;
  verified?: NumericLike;
  candidatesCount: NumericLike;
  derivedBy?: "Vrf" | string;
}

export interface RegisterZkAssetInstructionInput {
  assetDefinitionId: string;
  mode?: "ZkNative" | "Hybrid" | string;
  allowShield?: boolean;
  allowUnshield?: boolean;
  transferVerifyingKey?: VerifyingKeyIdLike | null;
  unshieldVerifyingKey?: VerifyingKeyIdLike | null;
  shieldVerifyingKey?: VerifyingKeyIdLike | null;
}

export interface ScheduleConfidentialPolicyTransitionInstructionInput {
  assetDefinitionId: string;
  newMode: "TransparentOnly" | "ShieldedOnly" | "Convertible" | string;
  effectiveHeight: NumericLike;
  transitionId: HashLike;
  conversionWindow?: NumericLike | null;
}

export interface CancelConfidentialPolicyTransitionInstructionInput {
  assetDefinitionId: string;
  transitionId: HashLike;
}

export interface ShieldInstructionInput {
  assetDefinitionId: string;
  fromAccountId: string;
  amount: NumericLike;
  noteCommitment: BinaryLike;
  encryptedPayload: ConfidentialEncryptedPayloadInput;
}

export interface ZkTransferInstructionInput {
  assetDefinitionId: string;
  inputs: ReadonlyArray<BinaryLike>;
  outputs: ReadonlyArray<BinaryLike>;
  proof: ProofAttachmentInput;
  rootHint?: BinaryLike | null;
}

export interface AssetHiddenZkTransferInstructionInput {
  poolId: string;
  inputs: ReadonlyArray<BinaryLike>;
  outputs: ReadonlyArray<BinaryLike>;
  proof: ProofAttachmentInput;
  rootHint?: BinaryLike | null;
}

export interface RegisterAssetHiddenZkPoolInstructionInput {
  poolId: string;
  storageAssetDefinitionId: string;
  assetSetRoot: BinaryLike;
  transferVerifyingKey: VerifyingKeyIdLike;
}

export interface RegisterZkAceIdentityCommitmentInstructionInput {
  assetDefinitionId: string;
  identityCommitment: BinaryLike;
  policyHash: BinaryLike;
  allowedAccounts: string[];
  actionClass?: string;
  domainTag?: string;
  verifierKey: VerifyingKeyIdLike;
}

export interface RotateZkAceIdentityCommitmentInstructionInput {
  assetDefinitionId: string;
  oldIdentityCommitment: BinaryLike;
  newIdentityCommitment: BinaryLike;
  policyHash: BinaryLike;
  allowedAccounts: string[];
  actionClass?: string;
  domainTag?: string;
  verifierKey: VerifyingKeyIdLike;
}

export interface RevokeZkAceIdentityCommitmentInstructionInput {
  assetDefinitionId: string;
  identityCommitment: BinaryLike;
  reasonHash?: BinaryLike | null;
}

export interface ZkAcePublicInputsV1Input {
  version?: NumericLike;
  identityCommitment: BinaryLike;
  txDigest: BinaryLike;
  chainId: string;
  domainTag?: string;
  actionClass?: string;
  replayNullifier: BinaryLike;
  policyHash: BinaryLike;
  fromAccountId: string;
  toAccountId: string;
  assetDefinitionId: string;
  amount: NumericLike;
  verifierKeyId: VerifyingKeyIdLike;
}

export interface ZkAceAuthorizationProofV1Input {
  publicInputs: ZkAcePublicInputsV1Input;
  witness?: ZkAceWitnessV1Input;
  proof?: ProofAttachmentInput | BinaryLike;
  proofBytes?: BinaryLike;
  proofAttachment?: ProofAttachmentInput;
  verifyingKeyRef?: VerifyingKeyIdLike;
  verifyingKeyCommitment?: BinaryLike | null;
  envelopeHash?: BinaryLike | null;
}

export interface ZkAceAuthorizationProofV1 {
  public_inputs: object;
  proof: ProofAttachmentInput;
}

export interface ZkAceWitnessV1Input {
  identityRoot: BinaryLike;
  identityBlinding: BinaryLike;
  replaySecret: BinaryLike;
}

export interface ZkAceAuthorizedTransferInstructionInput {
  fromAccountId: string;
  toAccountId: string;
  assetDefinitionId: string;
  amount: NumericLike;
  identityCommitment: BinaryLike;
  txDigest: BinaryLike;
  chainId: string;
  domainTag?: string;
  actionClass?: string;
  replayNullifier: BinaryLike;
  policyHash: BinaryLike;
  proof?: ProofAttachmentInput;
  authorizationProof?: ZkAceAuthorizationProofV1Input;
}

export interface UnshieldInstructionInput {
  assetDefinitionId: string;
  destinationAccountId: string;
  publicAmount: NumericLike;
  inputs: ReadonlyArray<BinaryLike>;
  outputs?: ReadonlyArray<BinaryLike>;
  proof: ProofAttachmentInput;
  rootHint?: BinaryLike | null;
}

export interface CreateElectionInstructionInput {
  electionId: string;
  options: NumericLike;
  eligibleRoot: BinaryLike;
  startTs: NumericLike;
  endTs: NumericLike;
  ballotVerifyingKey: VerifyingKeyIdLike;
  tallyVerifyingKey: VerifyingKeyIdLike;
  domainTag?: string;
}

export interface SubmitBallotInstructionInput {
  electionId: string;
  ciphertext: BinaryLike;
  ballotProof: ProofAttachmentInput;
  nullifier: BinaryLike;
}

export interface FinalizeElectionInstructionInput {
  electionId: string;
  tally: ReadonlyArray<NumericLike>;
  tallyProof: ProofAttachmentInput;
}

export type IsoBridgeStatus = "Pending" | "Accepted" | "Rejected";
export type Pacs002StatusCode =
  | "ACTC"
  | "ACSP"
  | "ACSC"
  | "ACWC"
  | "PDNG"
  | "RJCT";

export interface IsoMessageSubmissionResponseBase {
  message_id: string;
  status: IsoBridgeStatus;
  pacs002_code: Pacs002StatusCode | null;
  transaction_hash: string | null;
  profile_id: string | null;
  message_type: string | null;
  business_service: string | null;
  business_message_id: string | null;
  uetr: string | null;
  payload_hash: string | null;
  reference_snapshot_id: string | null;
  embedded_signature_detected: boolean;
  status_history: ReadonlyArray<IsoStatusHistoryEntry>;
  hold_reason_code: string | null;
  change_reason_codes: ReadonlyArray<string>;
  rejection_reason_code: string | null;
  ledger_id: string | null;
  source_account_id: string | null;
  source_account_address: string | null;
  target_account_id: string | null;
  target_account_address: string | null;
  asset_definition_id: string | null;
  asset_id: string | null;
}

export interface IsoPacs008SubmissionResponse
  extends IsoMessageSubmissionResponseBase {}

export interface IsoPacs009SubmissionResponse
  extends IsoMessageSubmissionResponseBase {}

export interface IsoMessageStatusResponse
  extends IsoMessageSubmissionResponseBase {
  detail: string | null;
  updated_at_ms: number;
}

export interface IsoStatusHistoryEntry {
  status: IsoBridgeStatus;
  pacs002_code: Pacs002StatusCode;
  updated_at_ms: number | null;
  detail: string | null;
  reason_code: string | null;
}

export interface IsoMessagePollEvent {
  attempt: number;
  status: IsoMessageStatusResponse | null;
}

export interface IsoMessageWaitOptions {
  maxAttempts?: number;
  pollIntervalMs?: number;
  signal?: AbortSignal;
  retryProfile?: string;
  resolveOnAcceptedWithoutTransaction?: boolean;
  /**
   * Alias for {@link resolveOnAcceptedWithoutTransaction}.
   */
  resolveOnAccepted?: boolean;
  onPoll?: (event: IsoMessagePollEvent) => void | Promise<void>;
}

export interface SubmitIsoMessageOptions {
  kind?: "pacs.008" | "pacs.009";
  messageKind?: "pacs.008" | "pacs.009";
  contentType?: string;
  profile?: string;
  signal?: AbortSignal;
  retryProfile?: string;
  wait?: IsoMessageWaitOptions;
}

export interface ContractDynamicAccessHintInput {
  baseKey?: string;
  base_key?: string;
  keyType?: string;
  key_type?: string;
  boundKind?: string;
  bound_kind?: string;
  maxKeys?: NumericLike;
  max_keys?: NumericLike;
}

export interface ContractAccessSetHintsInput {
  readKeys?: ReadonlyArray<string>;
  writeKeys?: ReadonlyArray<string>;
  dynamicReads?: ReadonlyArray<ContractDynamicAccessHintInput>;
  dynamic_reads?: ReadonlyArray<ContractDynamicAccessHintInput>;
  dynamicWrites?: ReadonlyArray<ContractDynamicAccessHintInput>;
  dynamic_writes?: ReadonlyArray<ContractDynamicAccessHintInput>;
}

export interface ContractEntrypointInput {
  name: string;
  kind?: string;
  permission?: string | null;
}

export interface ContractKotobaTranslationInput {
  lang: string;
  text: string;
}

export type ContractKotobaEntryInput =
  | {
      msgId: string;
      translations: ReadonlyArray<ContractKotobaTranslationInput>;
    }
  | {
      msg_id: string;
      translations: ReadonlyArray<ContractKotobaTranslationInput>;
    };

export interface ContractManifestProvenanceInput {
  signer: string;
  signature: string;
}

export interface ContractManifestInput {
  codeHash?: HashLike | null;
  abiHash?: HashLike | null;
  compilerFingerprint?: string | null;
  featuresBitmap?: NumericLike | null;
  accessSetHints?: ContractAccessSetHintsInput | null;
  entrypoints?: ReadonlyArray<ContractEntrypointInput> | null;
  kotoba?: ReadonlyArray<ContractKotobaEntryInput> | null;
  provenance?: ContractManifestProvenanceInput | null;
}

/**
 * Manifest payload accepted by Torii HTTP contract endpoints. Hash fields must be provided
 * as 32-byte hex strings (optionally prefixed with `0x`); canonical `hash:` literals or
 * binary buffers are rejected at runtime.
 */
export interface ToriiContractManifestInput {
  codeHash?: string | null;
  abiHash?: string | null;
  compilerFingerprint?: string | null;
  featuresBitmap?: NumericLike | null;
  accessSetHints?: ContractAccessSetHintsInput | null;
  entrypoints?: ReadonlyArray<ContractEntrypointInput> | null;
  kotoba?: ReadonlyArray<ContractKotobaEntryInput> | null;
  provenance?: ContractManifestProvenanceInput | null;
}

export interface RegisterContractCodeRequest {
  authority: string;
  privateKey: string;
  manifest: ToriiContractManifestInput;
  codeBytes?: string | ArrayBufferView | ArrayBuffer | Buffer | null;
}

export interface DeployContractRequest {
  authority: string;
  privateKey: string;
  contractAlias: string;
  codeB64: string | ArrayBufferView | ArrayBuffer | Buffer;
  leaseExpiryMs?: number | null;
}

export interface DeployContractResponse {
  ok: boolean;
  contract_alias: string | null;
  contract_address: string | null;
  previous_contract_address: string | null;
  upgraded: boolean;
  dataspace: string | null;
  deploy_nonce: number | null;
  tx_hash_hex: string | null;
  pipeline_status?: ToriiPipelineTransactionStatus | null;
  code_hash_hex: string;
  abi_hash_hex: string;
}

export interface SetContractAliasRequest {
  authority: string;
  privateKey: string;
  contractAddress: string;
  contractAlias?: string | null;
  leaseExpiryMs?: number | null;
}

export interface SetContractAliasResponse {
  ok: boolean;
  contract_alias: string | null;
  contract_address: string;
  dataspace: string;
  tx_hash_hex: string | null;
  status: string;
}

export interface ContractCallRequest {
  authority: string;
  privateKey: string;
  contractAddress?: string;
  contractAlias?: string;
  entrypoint?: string | null;
  payload?: unknown;
  gasAssetId?: string | null;
  gasLimit: NumericLike;
}

export interface ContractCallResponse {
  ok: boolean;
  submitted: boolean;
  dataspace: string;
  contract_address?: string;
  code_hash_hex: string;
  abi_hash_hex: string;
  creation_time_ms: number;
  tx_hash_hex: string | null;
  pipeline_status?: ToriiPipelineTransactionStatus | null;
  entrypoint: string | null;
  transaction_scaffold_b64: string | null;
  signed_transaction_b64: string | null;
  signing_message_b64: string | null;
}

export interface ContractManifestRecord {
  manifest: {
    code_hash: string | null | undefined;
    abi_hash: string | null | undefined;
    compiler_fingerprint?: string | null | undefined;
    features_bitmap?: number | null | undefined;
    access_set_hints?:
      | {
          read_keys: ReadonlyArray<string>;
          write_keys: ReadonlyArray<string>;
        }
      | null
      | undefined;
    entrypoints?:
      | ReadonlyArray<{
          name: string;
          kind: { kind: string; value?: unknown };
          permission: string | null;
        }>
      | null
      | undefined;
    kotoba?: JsonValue | null | undefined;
    provenance?: JsonValue | null | undefined;
  };
  code_bytes: string | null;
}

export interface ContractCodeBytesRecord {
  code_b64: string;
}

export interface SorafsPinResponse {
  manifest_id_hex: string;
  payload_digest_hex: string;
  content_length: number;
}

export interface SorafsFetchResponse {
  manifest_id_hex: string;
  offset: number;
  length: number;
  data_b64: string;
}

export interface SorafsStorageStateResponse {
  bytes_used: number;
  bytes_capacity: number;
  pin_queue_depth: number;
  fetch_inflight: number;
  fetch_bytes_per_sec: number;
  por_inflight: number;
  por_samples_success_total: number;
  por_samples_failed_total: number;
  fetch_utilisation_bps: number;
  pin_queue_utilisation_bps: number;
  por_utilisation_bps: number;
}

export interface SorafsManifestResponse {
  manifest_id_hex: string;
  manifest_b64: string;
  manifest_digest_hex: string;
  payload_digest_hex: string;
  content_length: number;
  chunk_count: number;
  chunk_profile_handle: string;
  stored_at_unix_secs: number;
}

export interface SorafsUptimeObservationResponse {
  status: string;
  uptime_secs: number;
  observed_secs: number;
}

export interface SorafsPorSubmissionResponse {
  status: string;
}

export interface SorafsPorVerdictResponse {
  status: string;
}

export interface SorafsPorObservationResponse {
  status: string;
  success: boolean;
}

export interface DaManifestFetchResponse {
  storage_ticket_hex: string;
  client_blob_id_hex: string;
  blob_hash_hex: string;
  manifest_hash_hex: string;
  manifest_id_hex: string;
  chunk_root_hex: string;
  lane_id: number;
  epoch: number;
  manifest_len: number;
  manifest_b64: string;
  manifest_bytes: Buffer;
  manifest_json: unknown;
  chunk_plan: unknown;
  sampling_plan: {
    assignment_hash_hex: string;
    sample_window: number;
    samples: ReadonlyArray<{ index: number; role: string; group: number }>;
  } | null;
}

export interface DaProofSummaryOptions {
  sampleCount?: number;
  sampleSeed?: number | bigint;
  leafIndexes?: ReadonlyArray<number | bigint>;
}

export interface DaProofRecord {
  origin: string;
  leaf_index: number;
  chunk_index: number;
  segment_index: number;
  leaf_offset: number | bigint;
  leaf_length: number;
  segment_offset: number | bigint;
  segment_length: number;
  chunk_offset: number | bigint;
  chunk_length: number;
  payload_len: number | bigint;
  chunk_digest_hex: string;
  chunk_root_hex: string;
  segment_digest_hex: string;
  leaf_digest_hex: string;
  leaf_bytes_b64: string;
  segment_leaves_hex: ReadonlyArray<string>;
  chunk_segments_hex: ReadonlyArray<string>;
  chunk_roots_hex: ReadonlyArray<string>;
  verified: boolean;
}

export interface DaProofSummary {
  blob_hash_hex: string;
  chunk_root_hex: string;
  por_root_hex: string;
  leaf_count: number | bigint;
  segment_count: number | bigint;
  chunk_count: number | bigint;
  sample_count: number;
  sample_seed: number | bigint;
  proof_count: number;
  proofs: ReadonlyArray<DaProofRecord>;
}

export function generateDaProofSummary(
  manifestBytes: BinaryLike,
  payloadBytes: BinaryLike,
  options?: DaProofSummaryOptions,
): DaProofSummary;

export interface DaProofSummaryArtifactRecord {
  origin: string;
  leaf_index: number | string;
  chunk_index: number | string;
  segment_index: number | string;
  leaf_offset: number | string;
  leaf_length: number | string;
  segment_offset: number | string;
  segment_length: number | string;
  chunk_offset: number | string;
  chunk_length: number | string;
  payload_len: number | string;
  chunk_digest: string;
  chunk_root: string;
  segment_digest: string;
  leaf_digest: string;
  leaf_bytes_b64: string;
  segment_leaves: ReadonlyArray<string>;
  chunk_segments: ReadonlyArray<string>;
  chunk_roots: ReadonlyArray<string>;
  verified: boolean;
}

export interface DaProofSummaryArtifact {
  manifest_path: string | null;
  payload_path: string | null;
  blob_hash: string;
  chunk_root: string;
  por_root: string;
  leaf_count: number | string;
  segment_count: number | string;
  chunk_count: number | string;
  sample_count: number | string;
  sample_seed: number | string;
  proof_count: number | string;
  proofs: ReadonlyArray<DaProofSummaryArtifactRecord>;
}

export interface DaProofSummaryArtifactOptions {
  manifestPath?: string | null;
  payloadPath?: string | null;
}

export function buildDaProofSummaryArtifact(
  summary: DaProofSummary,
  options?: DaProofSummaryArtifactOptions,
): DaProofSummaryArtifact;

export interface EmitDaProofSummaryOptions {
  summary?: DaProofSummary;
  manifestBytes?: BinaryLike;
  payloadBytes?: BinaryLike;
  proofOptions?: DaProofSummaryOptions;
  manifestPath?: string | null;
  payloadPath?: string | null;
  outputPath?: string;
  pretty?: number | boolean;
}

export interface EmitDaProofSummaryResult {
  summary: DaProofSummary;
  artifact: DaProofSummaryArtifact;
  outputPath: string | null;
}

export function emitDaProofSummaryArtifact(
  options: EmitDaProofSummaryOptions,
): Promise<EmitDaProofSummaryResult>;

export function deriveDaChunkerHandle(manifestBytes: BinaryLike): string;

export interface DaGatewayFetchRequestBase {
  storageTicketHex?: string;
  manifestBundle?: DaManifestFetchResponse;
  chunkPlan?: unknown;
  planJson?: string;
  chunkerHandle?: string;
  fetchOptions?: SorafsGatewayFetchOptions;
  gatewayOptions?: SorafsGatewayFetchOptions;
  proofSummary?: boolean | DaProofSummaryOptions;
  signal?: AbortSignal;
}

export type DaGatewayFetchRequest =
  | (DaGatewayFetchRequestBase & {
      gatewayProviders: ReadonlyArray<SorafsGatewayProviderSpec>;
      providers?: never;
    })
  | (DaGatewayFetchRequestBase & {
      providers: ReadonlyArray<SorafsGatewayProviderSpec>;
      gatewayProviders?: never;
    });

export interface DaGatewayFetchSession {
  manifest: DaManifestFetchResponse;
  manifestIdHex: string;
  chunkerHandle: string;
  chunkPlan: unknown;
  chunkPlanJson: string;
  gatewayResult: SorafsGatewayFetchResult;
  proofSummary: DaProofSummary | null;
}

export interface DaManifestPersistedPaths {
  manifestPath: string;
  manifestJsonPath: string;
  chunkPlanPath: string;
  samplingPlanPath?: string | null;
  label: string;
}

export interface DaIngestMetadataEntry {
  key: string;
  value: ArrayBufferView | ArrayBuffer | Buffer | string;
  visibility?: "Public" | "GovernanceOnly";
  encryption?: {
    cipher?: "None" | "ChaCha20Poly1305";
    keyLabel?: string;
  };
}

export type DaIngestMetadataMapValue =
  | string
  | ArrayBuffer
  | ArrayBufferView
  | Buffer
  | {
      value: ArrayBufferView | ArrayBuffer | Buffer | string;
      visibility?: "Public" | "GovernanceOnly";
      encryption?: {
        cipher?: "None" | "ChaCha20Poly1305";
        keyLabel?: string;
      };
    };

export interface DaIngestRequestInput {
  payload: ArrayBufferView | ArrayBuffer | Buffer | string;
  laneId?: number;
  epoch?: number;
  sequence?: number;
  blobClass?:
    | "TaikaiSegment"
    | "NexusLaneSidecar"
    | "GovernanceArtifact"
    | { class: "Custom"; value: number };
  codec?: string;
  chunkSize?: number;
  erasureProfile?: {
    dataShards?: number;
    parityShards?: number;
    chunkAlignment?: number;
    fecScheme?:
      | "Rs12_10"
      | "RsWin14_10"
      | "Rs18_14"
      | { scheme: "Custom"; value: number };
  };
  retentionPolicy?: {
    hotRetentionSecs?: number;
    coldRetentionSecs?: number;
    requiredReplicas?: number;
    storageClass?: "Hot" | "Warm" | "Cold";
    governanceTag?: string;
  };
  metadata?: Record<string, DaIngestMetadataMapValue> | DaIngestMetadataEntry[];
  compression?: "Identity" | "Gzip" | "Deflate" | "Zstd";
  noritoManifest?: ArrayBufferView | ArrayBuffer | Buffer | string;
  clientBlobId?: ArrayBufferView | ArrayBuffer | Buffer | string;
  submitterPublicKey?: string;
  privateKey?: ArrayBufferView | ArrayBuffer | Buffer | string;
  privateKeyHex?: string;
  signatureHex?: string;
  artifactDir?: string;
  noSubmit?: boolean;
  dryRun?: boolean;
}

export interface DaRentQuote {
  base_rent_micro: string;
  protocol_reserve_micro: string;
  provider_reward_micro: string;
  pdp_bonus_micro: string;
  potr_bonus_micro: string;
  egress_credit_per_gib_micro: string;
}

export interface DaStripeLayout {
  total_stripes: number;
  shards_per_stripe: number;
  row_parity_stripes: number;
}

export interface DaIngestReceipt {
  client_blob_id_hex: string;
  client_blob_id_bytes: Buffer;
  lane_id: number;
  epoch: number;
  blob_hash_hex: string;
  blob_hash_bytes: Buffer;
  chunk_root_hex: string;
  chunk_root_bytes: Buffer;
  manifest_hash_hex: string;
  manifest_hash_bytes: Buffer;
  storage_ticket_hex: string;
  storage_ticket_bytes: Buffer;
  stripe_layout: DaStripeLayout;
  pdp_commitment_b64: string | null;
  pdp_commitment_bytes: Buffer | null;
  queued_at_unix: number;
  operator_signature_hex: string;
  rent_quote: DaRentQuote | null;
}

export interface DaIngestArtifacts {
  clientBlobIdHex: string;
  submitterPublicKey: string;
  signatureHex: string;
  payloadLength: number;
}

export interface DaIngestArtifactPaths {
  requestJsonPath: string | null;
  receiptJsonPath: string | null;
  responseHeadersPath: string | null;
}

export interface DaIngestSubmitResponse {
  status: string;
  duplicate: boolean;
  receipt: DaIngestReceipt | null;
  artifacts: DaIngestArtifacts;
  pdpCommitmentHeader: string | null;
  artifactPaths: DaIngestArtifactPaths | null;
}

export interface DaIngestBuildRequestResult {
  request: Record<string, unknown>;
  artifacts: DaIngestArtifacts;
}

export function buildDaIngestRequest(
  options?: DaIngestRequestInput,
): DaIngestBuildRequestResult;

export interface SorafsPorStatusOptions {
  manifestHex?: string | null;
  providerHex?: string | null;
  epoch?: NumericLike;
  status?: string;
  limit?: NumericLike;
  pageTokenHex?: string | null;
  signal?: AbortSignal;
}

export interface SorafsPorExportOptions {
  startEpoch?: NumericLike;
  endEpoch?: NumericLike;
  signal?: AbortSignal;
}

export type SorafsIsoWeekInput = string | { year: number; week: number };

export interface SorafsChunkerHandle {
  profile_id: number;
  namespace: string;
  name: string;
  semver: string;
  multihash_code: number;
}

export interface SorafsManifestAliasBinding {
  namespace: string;
  name: string;
  proof_b64: string;
}

export type SorafsManifestStatusState = "pending" | "approved" | "retired";

export interface SorafsManifestStatus {
  state: SorafsManifestStatusState;
  epoch: number | null;
}

export interface SorafsGovernanceReferenceTargets {
  alias: string | null;
  pin_digest_hex: string | null;
}

export interface SorafsGovernanceReference {
  cid: string | null;
  kind: string;
  effective_at: string | null;
  effective_at_unix: number | null;
  targets: SorafsGovernanceReferenceTargets;
  signers: ReadonlyArray<string>;
}

export interface SorafsLineageSuccessor {
  digest_hex: string;
  status: SorafsManifestStatus;
  approved_epoch: number | null;
  approved_at: string | null;
  status_timestamp_unix: number | null;
}

export interface SorafsManifestLineage {
  successor_of_hex: string | null;
  head_hex: string;
  depth_to_head: number;
  is_head: boolean;
  superseded_by: SorafsLineageSuccessor | null;
  immediate_successor: SorafsLineageSuccessor | null;
  anomalies: ReadonlyArray<string>;
}

export interface SorafsManifestRecord {
  digest_hex: string;
  chunker: SorafsChunkerHandle;
  chunk_digest_sha3_256_hex: string;
  pin_policy: Record<string, unknown>;
  submitted_by: string;
  submitted_epoch: number;
  status: SorafsManifestStatus;
  metadata: Record<string, unknown>;
  alias: SorafsManifestAliasBinding | null;
  successor_of_hex: string | null;
  status_timestamp_unix: number | null;
  governance_refs: ReadonlyArray<SorafsGovernanceReference>;
  council_envelope_digest_hex: string | null;
  lineage: SorafsManifestLineage | null;
}

export interface SorafsPinManifestResponse {
  attestation: Record<string, unknown> | null;
  manifest: SorafsManifestRecord;
  aliases: ReadonlyArray<SorafsAliasRecord>;
  replication_orders: ReadonlyArray<SorafsReplicationOrderRecord>;
}

export interface SorafsPinListResponse {
  attestation: Record<string, unknown> | null;
  total_count: number;
  returned_count: number;
  offset: number;
  limit: number;
  manifests: ReadonlyArray<SorafsManifestRecord>;
}

export interface SorafsPinListOptions {
  status?: SorafsManifestStatusState;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface SorafsPinRegisterPinPolicyInput {
  minReplicas?: NumericLike;
  min_replicas?: NumericLike;
  storageClass?:
    | "Hot"
    | "Warm"
    | "Cold"
    | string
    | {
        type?: string;
        name?: string;
        label?: string;
      };
  storage_class?:
    | "Hot"
    | "Warm"
    | "Cold"
    | string
    | {
        type?: string;
        name?: string;
        label?: string;
      };
  retentionEpoch?: NumericLike;
  retention_epoch?: NumericLike;
}

export interface SorafsPinRegisterChunkerInput {
  profileId: NumericLike;
  namespace: string;
  name: string;
  semver: string;
  multihashCode?: NumericLike;
}

export interface SorafsPinRegisterAliasInput {
  namespace: string;
  name: string;
  proof?: BinaryLike | string;
  proof_b64?: BinaryLike | string;
  proofB64?: BinaryLike | string;
  proof_base64?: BinaryLike | string;
  proofBase64?: BinaryLike | string;
}

export interface SorafsPinRegisterRequest {
  authority: string;
  privateKey: string;
  chunker: SorafsPinRegisterChunkerInput;
  pinPolicy?: SorafsPinRegisterPinPolicyInput;
  pin_policy?: SorafsPinRegisterPinPolicyInput;
  manifestDigestHex?: string;
  manifest_digest_hex?: string;
  chunkDigestSha3_256Hex?: string;
  chunk_digest_sha3_256_hex?: string;
  chunkDigest?: string;
  chunk_digest?: string;
  contentLength?: NumericLike;
  content_length?: NumericLike;
  submittedEpoch?: NumericLike;
  submitted_epoch?: NumericLike;
  alias?: SorafsPinRegisterAliasInput | null;
  aliasNamespace?: string;
  alias_namespace?: string;
  aliasName?: string;
  alias_name?: string;
  aliasProof?: BinaryLike | string;
  alias_proof?: BinaryLike | string;
  aliasProofB64?: BinaryLike | string;
  alias_proof_b64?: BinaryLike | string;
  aliasProofBase64?: BinaryLike | string;
  alias_proof_base64?: BinaryLike | string;
  successorOfHex?: string | null;
  successor_of_hex?: string | null;
  signal?: AbortSignal;
}

export interface SorafsPinRegisterAliasRecord {
  namespace: string;
  name: string;
  proof_base64: string;
}

export interface SorafsPinRegisterResponse {
  manifest_digest_hex: string;
  chunker_handle: string;
  submitted_epoch: number;
  content_length: number;
  pin_fee_nano: number;
  pin_fee_asset_id: string;
  pin_fee_treasury_account_id: string;
  alias: SorafsPinRegisterAliasRecord | null;
  successor_of_hex: string | null;
}

export interface SorafsAliasRecord {
  alias: string;
  namespace: string;
  name: string;
  manifest_digest_hex: string;
  bound_by: string;
  bound_epoch: number;
  expiry_epoch: number;
  proof_b64: string;
  cache_state: string | null;
  status_label: string | null;
  cache_rotation_due: boolean | null;
  cache_age_seconds: number | null;
  proof_generated_at_unix: number | null;
  proof_expires_at_unix: number | null;
  proof_expires_in_seconds: number | null;
  policy_positive_ttl_secs: number | null;
  policy_refresh_window_secs: number | null;
  policy_hard_expiry_secs: number | null;
  policy_rotation_max_age_secs: number | null;
  policy_successor_grace_secs: number | null;
  policy_governance_grace_secs: number | null;
  cache_decision: string | null;
  cache_reasons: ReadonlyArray<string> | null;
  cache_evaluation: Record<string, unknown> | null;
  lineage: Record<string, unknown> | null;
}

export interface SorafsAliasListResponse {
  attestation: Record<string, unknown> | null;
  total_count: number;
  returned_count: number;
  offset: number;
  limit: number;
  aliases: ReadonlyArray<SorafsAliasRecord>;
}

export interface SorafsAliasListOptions {
  namespace?: string;
  manifestDigestHex?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface SorafsReplicationReceipt {
  provider_hex: string;
  status: string;
  timestamp: number;
  por_sample_digest_hex: string | null;
}

export interface SorafsReplicationOrderRecord {
  order_id_hex: string;
  manifest_digest_hex: string;
  issued_by: string;
  issued_epoch: number;
  deadline_epoch: number;
  status: { state: string; epoch: number | null };
  canonical_order_b64: string;
  order: Record<string, unknown>;
  receipts: ReadonlyArray<SorafsReplicationReceipt>;
  providers: ReadonlyArray<string>;
}

export interface SorafsReplicationListResponse {
  attestation: Record<string, unknown> | null;
  total_count: number;
  returned_count: number;
  offset: number;
  limit: number;
  replication_orders: ReadonlyArray<SorafsReplicationOrderRecord>;
}

export interface SorafsReplicationListOptions {
  status?: "pending" | "completed" | "expired";
  manifestDigestHex?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

export interface UaidPortfolioTotals {
  accounts: number;
  positions: number;
}

export interface UaidPortfolioAsset {
  asset_id: string;
  asset_definition_id: string;
  quantity: string;
}

export interface UaidPortfolioAccount {
  account_id: string;
  label: string | null;
  assets: ReadonlyArray<UaidPortfolioAsset>;
}

export interface UaidPortfolioDataspace {
  dataspace_id: number;
  dataspace_alias: string | null;
  accounts: ReadonlyArray<UaidPortfolioAccount>;
}

export interface UaidPortfolioResponse {
  uaid: string;
  totals: UaidPortfolioTotals;
  dataspaces: ReadonlyArray<UaidPortfolioDataspace>;
}

export interface UaidPortfolioQueryOptions {
  assetId?: string;
  signal?: AbortSignal;
}

export interface UaidBindingsDataspace {
  dataspace_id: number;
  dataspace_alias: string | null;
  accounts: ReadonlyArray<string>;
}

export interface UaidBindingsResponse {
  uaid: string;
  dataspaces: ReadonlyArray<UaidBindingsDataspace>;
}

export type UaidManifestStatus = "Pending" | "Active" | "Expired" | "Revoked";

export interface UaidManifestLifecycleRevocation {
  epoch: number;
  reason: string | null;
}

export interface UaidManifestLifecycle {
  activated_epoch: number | null;
  expired_epoch: number | null;
  revocation: UaidManifestLifecycleRevocation | null;
}

export interface UaidManifestEntry {
  scope: Record<string, unknown>;
  effect: Record<string, unknown>;
  notes: string | null;
}

export interface UaidAssetPermissionManifest {
  version: string;
  uaid: string;
  dataspace: number;
  issued_ms: number;
  activation_epoch: number;
  expiry_epoch: number | null;
  entries: ReadonlyArray<UaidManifestEntry>;
}

export interface UaidManifestRecord {
  dataspace_id: number;
  dataspace_alias: string | null;
  manifest_hash: string;
  status: UaidManifestStatus;
  lifecycle: UaidManifestLifecycle;
  accounts: ReadonlyArray<string>;
  manifest: UaidAssetPermissionManifest;
}

export interface UaidManifestsResponse {
  uaid: string;
  manifests: ReadonlyArray<UaidManifestRecord>;
}

export interface PublishSpaceDirectoryManifestRequest {
  authority: string;
  manifest: UaidAssetPermissionManifest | Record<string, unknown>;
  privateKey?:
    | ArrayBufferView
    | ArrayBuffer
    | Buffer
    | ReadonlyArray<number>
    | string;
  privateKeyHex?: string;
  privateKeyMultihash?: string;
  privateKeyAlgorithm?: string;
  reason?: string | null;
}

export interface RevokeSpaceDirectoryManifestRequest {
  authority: string;
  uaid: string;
  dataspaceId: number;
  revokedEpoch: number;
  privateKey?:
    | ArrayBufferView
    | ArrayBuffer
    | Buffer
    | ReadonlyArray<number>
    | string;
  privateKeyHex?: string;
  privateKeyMultihash?: string;
  privateKeyAlgorithm?: string;
  reason?: string | null;
}

export interface UaidBindingsQueryOptions {
  signal?: AbortSignal;
}

export interface UaidManifestQueryOptions {
  dataspaceId?: number;
  signal?: AbortSignal;
}

export interface RegisterSmartContractCodeInstructionInput {
  manifest: ContractManifestInput;
}

export interface RegisterSmartContractBytesInstructionInput {
  codeHash: HashLike;
  code: ArrayBufferView | ArrayBuffer | Buffer | string;
}

export interface RemoveSmartContractBytesInstructionInput {
  codeHash: HashLike;
  reason?: string | null;
}

export interface CreateKaigiTransactionInput {
  chainId: string;
  authority: string;
  call: CreateKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface JoinKaigiTransactionInput {
  chainId: string;
  authority: string;
  join: JoinKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface LeaveKaigiTransactionInput {
  chainId: string;
  authority: string;
  leave: LeaveKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface EndKaigiTransactionInput {
  chainId: string;
  authority: string;
  end: EndKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface PrivateKaigiEntrypointResult {
  transactionEntrypoint: Buffer;
  hash: Buffer;
  actionHash: Buffer;
}

export interface PrivateKaigiFeeSpendResult {
  asset_definition_id: string;
  anchor_root: Buffer;
  nullifiers: ReadonlyArray<Buffer>;
  output_commitments: ReadonlyArray<Buffer>;
  encrypted_change_payloads: ReadonlyArray<Buffer>;
  proof: Buffer;
}

export interface PrivateKaigiFeeSpendInput {
  chainId: string;
  assetDefinitionId: string;
  actionHash: BinaryLike;
  anchorRootHex: string;
  feeAmount: NumericLike;
  verifyingKey: Record<string, unknown>;
}

export interface ConfidentialTransferProofInputV2 {
  amount: NumericLike;
  rhoHex?: string;
  rho?: BinaryLike;
  diversifierHex?: string;
  diversifier_hex?: string;
  diversifier?: BinaryLike;
  leafIndex?: number;
  leaf_index?: number;
}

export interface ConfidentialTransferProofOutputV2 {
  amount: NumericLike;
  rhoHex?: string;
  rho?: BinaryLike;
  ownerTagHex?: string;
  owner_tag_hex?: string;
  ownerTag?: BinaryLike;
}

export interface ConfidentialTransferProofResultV2 {
  nullifiers: ReadonlyArray<Buffer>;
  outputCommitments: ReadonlyArray<Buffer>;
  root: Buffer;
  proof: Buffer;
}

export interface ConfidentialUnshieldProofOutputV3 {
  amount: NumericLike;
  rhoHex?: string;
  rho?: BinaryLike;
}

export interface ConfidentialUnshieldProofResultV2 {
  nullifiers: ReadonlyArray<Buffer>;
  root: Buffer;
  proof: Buffer;
}

export interface ConfidentialUnshieldProofResultV3 {
  nullifiers: ReadonlyArray<Buffer>;
  outputCommitments: ReadonlyArray<Buffer>;
  root: Buffer;
  proof: Buffer;
}

export interface PrivateCreateKaigiTransactionInput {
  chainId: string;
  call: Record<string, unknown>;
  artifacts: Record<string, unknown>;
  feeSpend: Record<string, unknown>;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  nonce?: number | null;
}

export interface PrivateJoinKaigiTransactionInput {
  chainId: string;
  callId: string;
  artifacts: Record<string, unknown>;
  feeSpend: Record<string, unknown>;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  nonce?: number | null;
}

export interface PrivateEndKaigiTransactionInput {
  chainId: string;
  callId: string;
  endedAtMs?: number | null;
  artifacts: Record<string, unknown>;
  feeSpend: Record<string, unknown>;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  nonce?: number | null;
}

export interface RecordKaigiUsageTransactionInput {
  chainId: string;
  authority: string;
  usage: RecordKaigiUsageInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface SetKaigiRelayManifestTransactionInput {
  chainId: string;
  authority: string;
  manifest: SetKaigiRelayManifestInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RegisterKaigiRelayTransactionInput {
  chainId: string;
  authority: string;
  relay: RegisterKaigiRelayInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface ProposeDeployContractTransactionInput {
  chainId: string;
  authority: string;
  proposal: ProposeDeployContractInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface CastZkBallotTransactionInput {
  chainId: string;
  authority: string;
  ballot: CastZkBallotInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface CastPlainBallotTransactionInput {
  chainId: string;
  authority: string;
  ballot: CastPlainBallotInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface EnactReferendumTransactionInput {
  chainId: string;
  authority: string;
  enactment: EnactReferendumInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface FinalizeReferendumTransactionInput {
  chainId: string;
  authority: string;
  finalization: FinalizeReferendumInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface PersistCouncilForEpochTransactionInput {
  chainId: string;
  authority: string;
  record: PersistCouncilForEpochInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RegisterZkAssetTransactionInput {
  chainId: string;
  authority: string;
  registration: RegisterZkAssetInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface ScheduleConfidentialPolicyTransitionTransactionInput {
  chainId: string;
  authority: string;
  transition: ScheduleConfidentialPolicyTransitionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface CancelConfidentialPolicyTransitionTransactionInput {
  chainId: string;
  authority: string;
  cancellation: CancelConfidentialPolicyTransitionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface ShieldTransactionInput {
  chainId: string;
  authority: string;
  shield: ShieldInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface ZkTransferTransactionInput {
  chainId: string;
  authority: string;
  transfer: ZkTransferInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface UnshieldTransactionInput {
  chainId: string;
  authority: string;
  unshield: UnshieldInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface CreateElectionTransactionInput {
  chainId: string;
  authority: string;
  election: CreateElectionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface SubmitBallotTransactionInput {
  chainId: string;
  authority: string;
  ballot: SubmitBallotInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface FinalizeElectionTransactionInput {
  chainId: string;
  authority: string;
  finalization: FinalizeElectionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RegisterSmartContractCodeTransactionInput {
  chainId: string;
  authority: string;
  manifest: ContractManifestInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RegisterSmartContractBytesTransactionInput {
  chainId: string;
  authority: string;
  codeHash: HashLike;
  code: ArrayBufferView | ArrayBuffer | Buffer | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RemoveSmartContractBytesTransactionInput {
  chainId: string;
  authority: string;
  codeHash: HashLike;
  reason?: string | null;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface SubmitTransactionAndWaitOptions
  extends TransactionStatusPollOptions {
  hashHex: string;
}

export interface SubmitOfflineSettlementAndWaitOptions
  extends TransactionStatusPollOptions {
  signal?: AbortSignal;
}

export declare class ToriiHttpError extends Error {
  constructor(details: {
    status: number;
    statusText?: string | null;
    expected?: ReadonlyArray<number>;
    code?: string | null;
    rejectCode?: string | null;
    errorMessage?: string | null;
    bodyText?: string | null;
    bodyJson?: unknown;
    details?: Record<string, unknown> | null;
  });
  readonly status: number;
  readonly statusText: string | null;
  readonly expected: ReadonlyArray<number>;
  readonly code: string | null;
  readonly rejectCode: string | null;
  readonly errorMessage: string | null;
  readonly bodyText: string | null;
  readonly bodyJson: unknown;
  readonly details: Record<string, unknown> | null;
}

export declare class TransactionStatusError extends Error {
  constructor(
    hashHex: string,
    status: string | null,
    payload: ToriiPipelineTransactionStatus | null,
  );
  readonly hashHex: string;
  readonly status: string | null;
  readonly payload: ToriiPipelineTransactionStatus | null;
  readonly rejectionReason: string | null;
}

export declare class TransactionTimeoutError extends Error {
  constructor(
    message: string,
    hashHex: string,
    attempts: number,
    payload: ToriiPipelineTransactionStatus | null,
  );
  readonly hashHex: string;
  readonly attempts: number;
  readonly payload: ToriiPipelineTransactionStatus | null;
}

export declare class IsoMessageTimeoutError extends Error {
  constructor(
    messageId: string,
    attempts: number,
    lastStatus: IsoMessageStatusResponse | null,
  );
  readonly messageId: string;
  readonly attempts: number;
  readonly lastStatus: IsoMessageStatusResponse | null;
}

export declare class ToriiDataModelCompatibilityError extends Error {
  constructor(expected: number, actual?: number | null, cause?: unknown);
  readonly expected: number;
  readonly actual: number | null;
  readonly cause?: unknown;
}

export declare function extractPipelineStatusKind(
  payload: unknown,
): string | null;
export declare function extractPipelineRejectionReason(
  payload: unknown,
): string | null;
export declare function decodePdpCommitmentHeader(
  headers?:
    | Headers
    | Map<string, string>
    | Record<string, string | undefined | null>
    | null,
): Uint8Array | null;
export declare function buildConnectWebSocketUrl(
  baseUrl: string,
  options: ConnectWebSocketParams,
): string;

export declare function buildRbcSampleRequest(
  session: SumeragiRbcSession,
  overrides?: RbcSampleRequestOverrides,
): RbcSampleRequestOptions;
export declare function openConnectWebSocket<T = unknown>(
  options: ConnectWebSocketDialOptions<T>,
): T;

export interface InstructionBuilders {
  Mint: {
    Asset: {
      object: string;
      destination: string;
    };
  };
  Burn: {
    Asset: {
      object: string;
      destination: string;
    };
  };
}

export interface ToriiBrowserClientOptions {
  fetchImpl?: typeof fetch;
  defaultHeaders?: Record<string, string>;
  timeoutMs?: NumericLike;
  config?: {
    toriiClient?: {
      timeoutMs?: NumericLike;
      defaultHeaders?: Record<string, string>;
    };
  };
}

export declare class ToriiBrowserHttpError extends Error {
  readonly response: Response;
  readonly status: number;
  readonly bodyText: string;
}

export declare class ToriiBrowserClient {
  constructor(baseUrl: string | URL, options?: ToriiBrowserClientOptions);
  listExplorerAccounts(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerAccount(
    accountId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerDomains(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerDomain(
    domainId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerAssets(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerAsset(
    assetId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listAccountAssets(
    accountId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  queryAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: TransactionQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryTransactions<T = ToriiAccountTransactionItem>(
    options?: TransactionQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryVisibleTransactions<T = ToriiAccountTransactionItem>(
    options?: TransactionQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  listAssetHolders(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listAssetDefinitions(options?: Record<string, unknown>): Promise<unknown>;
  getAssetDefinition(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerAssetDefinitions(
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerAssetDefinitionEconometrics(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerAssetDefinitionSnapshot(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerNfts(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerNft(
    nftId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerRwas(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerRwa(
    rwaId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerBlocks(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerBlock(
    identifier: string | number | bigint,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerMetrics(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerHealth(options?: Record<string, unknown>): Promise<unknown>;
  listExplorerTransactions(options?: Record<string, unknown>): Promise<unknown>;
  listLatestExplorerTransactions(
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerTransaction(
    hash: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerInstructions(options?: Record<string, unknown>): Promise<unknown>;
  listLatestExplorerInstructions(
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerInstruction(
    transactionHash: string,
    index: number,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerInstructionContractView(
    transactionHash: string,
    index: number,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getSumeragiStatus(options?: Record<string, unknown>): Promise<unknown>;
  getSumeragiTelemetry(options?: Record<string, unknown>): Promise<unknown>;
  listKaigiRelays(options?: Record<string, unknown>): Promise<unknown>;
  getKaigiRelay(
    relayId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getKaigiRelaysHealth(options?: Record<string, unknown>): Promise<unknown>;
  deployContract(
    request: Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
}

export declare class ToriiClient {
  constructor(baseUrl: string, options?: ToriiClientOptions);
  listAccounts<T = ToriiAccountListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAccounts<T = ToriiAccountListItem>(
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccounts<T = ToriiAccountListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountsQuery<T = ToriiAccountListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listDomains<T = ToriiDomainListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryDomains<T = ToriiDomainListItem>(
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateDomains<T = ToriiDomainListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateDomainsQuery<T = ToriiDomainListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAssetDefinitions<T = ToriiAssetDefinitionListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAssetDefinitions<T = ToriiAssetDefinitionListItem>(
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAssetDefinitions<T = ToriiAssetDefinitionListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAssetDefinitionsQuery<T = ToriiAssetDefinitionListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listRepoAgreements(
    options?: IterableListOptions,
  ): Promise<RepoAgreementListResponse>;
  queryRepoAgreements(
    options?: IterableQueryOptions,
  ): Promise<RepoAgreementListResponse>;
  iterateRepoAgreements(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiRepoAgreement, void, unknown>;
  iterateRepoAgreementsQuery(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiRepoAgreement, void, unknown>;
  listNfts<T = ToriiNftListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryNfts<T = ToriiNftListItem>(
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateNfts<T = ToriiNftListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateNftsQuery<T = ToriiNftListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listRwas<T = ToriiRwaListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryRwas<T = ToriiRwaListItem>(
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateRwas<T = ToriiRwaListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateRwasQuery<T = ToriiRwaListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listExplorerRwas<T = ToriiExplorerRwa>(
    options?: ExplorerRwaListOptions,
  ): Promise<ToriiExplorerRwasPage>;
  getExplorerRwaDetail<T = ToriiExplorerRwa>(
    rwaId: string,
    options?: { signal?: AbortSignal },
  ): Promise<T | null>;
  iterateExplorerRwas<T = ToriiExplorerRwa>(
    options?: ExplorerRwaIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAccountRwas<T = ToriiExplorerRwa>(
    accountId: string,
    options?: ExplorerRwaListOptions,
  ): Promise<ToriiExplorerRwasPage>;
  iterateAccountRwas<T = ToriiExplorerRwa>(
    accountId: string,
    options?: ExplorerRwaIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listExplorerNfts<T = ToriiExplorerNft>(
    options?: ExplorerNftListOptions,
  ): Promise<ToriiExplorerNftsPage>;
  iterateExplorerNfts<T = ToriiExplorerNft>(
    options?: ExplorerNftIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAccountNfts<T = ToriiExplorerNft>(
    accountId: string,
    options?: ExplorerNftListOptions,
  ): Promise<ToriiExplorerNftsPage>;
  iterateAccountNfts<T = ToriiExplorerNft>(
    accountId: string,
    options?: ExplorerNftIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAccountAssets<T = ToriiAccountAssetItem>(
    accountId: string,
    options?: AccountAssetListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAccountAssets<T = ToriiAccountAssetItem>(
    accountId: string,
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccountAssets<T = ToriiAccountAssetItem>(
    accountId: string,
    options?: AccountAssetIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountAssetsQuery<T = ToriiAccountAssetItem>(
    accountId: string,
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: AccountTransactionListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  listContractActivity<T = ToriiContractActivityItem>(
    options?: ContractActivityListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  listContractEvents<T = ToriiContractEventItem>(
    options?: ContractEventListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: TransactionQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryTransactions<T = ToriiAccountTransactionItem>(
    options?: TransactionQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryVisibleTransactions<T = ToriiAccountTransactionItem>(
    options?: TransactionQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: AccountTransactionIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountTransactionsQuery<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateTransactionsQuery<T = ToriiAccountTransactionItem>(
    options?: TransactionIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateVisibleTransactionsQuery<T = ToriiAccountTransactionItem>(
    options?: TransactionIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAssetHolders<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options?: AssetHolderListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAssetHolders<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAssetHolders<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options?: AssetHolderIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAssetHoldersQuery<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAccountPermissions<T = ToriiAccountPermissionItem>(
    accountId: string,
    options?: AccountPermissionsListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccountPermissions<T = ToriiAccountPermissionItem>(
    accountId: string,
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  uploadAttachment(
    data: ArrayBufferView | ArrayBuffer | string,
    options: { contentType: string },
  ): Promise<ToriiAttachmentMetadata>;
  listAttachments(options?: {
    signal?: AbortSignal;
  }): Promise<ReadonlyArray<ToriiAttachmentMetadata>>;
  getAttachment(
    attachmentId: string,
    options?: { signal?: AbortSignal },
  ): Promise<{ data: Buffer; contentType: string | null }>;
  deleteAttachment(attachmentId: string): Promise<void>;
  listVerifyingKeys(options?: ToriiVerifyingKeyListOptions): Promise<unknown>;
  listVerifyingKeysTyped(
    options?: ToriiVerifyingKeyListOptions,
  ): Promise<ReadonlyArray<ToriiVerifyingKeyListItem>>;
  iterateVerifyingKeys(
    options?: ToriiVerifyingKeyListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<ToriiVerifyingKeyListItem, void, unknown>;
  getVerifyingKey(
    backend: string,
    name: string,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  getVerifyingKeyTyped(
    backend: string,
    name: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiVerifyingKeyDetail>;
  registerVerifyingKey(
    payload: ToriiVerifyingKeyRegisterPayload,
    options?: { signal?: AbortSignal },
  ): Promise<void>;
  updateVerifyingKey(
    payload: ToriiVerifyingKeyUpdatePayload,
    options?: { signal?: AbortSignal },
  ): Promise<void>;
  evaluateAliasVoprf(
    blindedElementHex: string,
  ): Promise<AliasVoprfEvaluateResponse>;
  resolveAlias(
    alias: string,
    options?: CanonicalRequestOptions,
  ): Promise<AliasResolutionDto | null>;
  resolveAliasByIndex(
    index: number | string | bigint,
    options?: CanonicalRequestOptions,
  ): Promise<AliasResolutionDto | null>;
  lookupAliasesByAccount(
    accountId: string,
    options?: AliasLookupByAccountOptions,
  ): Promise<AliasLookupByAccountResponse | null>;
  listRamLfeProgramPolicies(options?: {
    signal?: AbortSignal;
  }): Promise<{ total: number; items: Array<Record<string, unknown>> }>;
  executeRamLfeProgram(
    programId: string,
    options: { encryptedInput: string; signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  verifyRamLfeReceipt(options: {
    receipt: Record<string, unknown>;
    outputHex?: string;
    signal?: AbortSignal;
  }): Promise<Record<string, unknown>>;
  listSorafsPinManifests(
    options?: SorafsPinListOptions,
  ): Promise<SorafsPinListResponse>;
  iterateSorafsPinManifests(
    options?: SorafsPinListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<SorafsManifestRecord, void, unknown>;
  listSorafsAliases(
    options?: SorafsAliasListOptions,
  ): Promise<SorafsAliasListResponse>;
  iterateSorafsAliases(
    options?: SorafsAliasListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<SorafsAliasRecord, void, unknown>;
  listSorafsReplicationOrders(
    options?: SorafsReplicationListOptions,
  ): Promise<SorafsReplicationListResponse>;
  iterateSorafsReplicationOrders(
    options?: SorafsReplicationListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<SorafsReplicationOrderRecord, void, unknown>;
  getSorafsPinManifest(
    digestHex: string,
    options?: { headers?: Record<string, string>; signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getSorafsPinManifestTyped(
    digestHex: string,
    options?: { headers?: Record<string, string>; signal?: AbortSignal },
  ): Promise<SorafsPinManifestResponse>;
  registerSorafsPinManifest(
    input: SorafsPinRegisterRequest,
  ): Promise<Record<string, unknown>>;
  registerSorafsPinManifestTyped(
    input: SorafsPinRegisterRequest,
  ): Promise<SorafsPinRegisterResponse>;
  pinSorafsManifest(input: {
    manifest: ArrayBufferView | ArrayBuffer | Buffer | string;
    payload: ArrayBufferView | ArrayBuffer | Buffer | string;
    signal?: AbortSignal;
  }): Promise<SorafsPinResponse>;
  fetchSorafsPayloadRange(input: {
    manifestIdHex: string;
    offset: number | string | bigint;
    length: number | string | bigint;
    providerIdHex?: string | Buffer | ArrayBuffer | ArrayBufferView | null;
    signal?: AbortSignal;
  }): Promise<SorafsFetchResponse>;
  getSorafsStorageState(options?: {
    signal?: AbortSignal;
  }): Promise<SorafsStorageStateResponse>;
  getSorafsManifest(
    manifestIdHex: string,
    options?: { signal?: AbortSignal },
  ): Promise<SorafsManifestResponse>;
  getDaManifest(
    storageTicketHex: string,
    options?: { signal?: AbortSignal; blockHashHex?: string },
  ): Promise<DaManifestFetchResponse>;
  getDaManifestToDir(
    storageTicketHex: string,
    options?: {
      outputDir?: string;
      signal?: AbortSignal;
      label?: string;
      blockHashHex?: string;
    },
  ): Promise<{
    manifest: DaManifestFetchResponse;
    paths: DaManifestPersistedPaths;
    outputDir: string;
  }>;
  submitDaBlob(
    options: DaIngestRequestInput & { signal?: AbortSignal },
  ): Promise<DaIngestSubmitResponse>;
  fetchDaPayloadViaGateway(
    options: DaGatewayFetchRequest,
  ): Promise<DaGatewayFetchSession>;
  proveDaAvailabilityToDir(options: {
    storageTicketHex?: string;
    manifestBundle?: DaManifestFetchResponse;
    gatewayProviders: ReadonlyArray<SorafsGatewayProviderSpec>;
    fetchOptions?: SorafsGatewayFetchOptions;
    proofSummary?: boolean | DaProofSummaryOptions | Record<string, unknown>;
    outputDir?: string;
    chunkerHandle?: string;
    signal?: AbortSignal;
    scoreboardPath?: string;
  }): Promise<{
    manifest: DaManifestFetchResponse;
    manifestPaths: DaManifestPersistedPaths;
    payloadPath: string;
    scoreboardPath: string | null;
    proofSummaryPath: string;
    proofSummaryArtifact: DaProofSummaryArtifact;
    proofSummary: DaProofSummary;
    gatewayResult: SorafsGatewayFetchResult;
    outputDir: string;
  }>;
  submitSorafsUptimeObservation(input: {
    uptimeSecs: number;
    observedSecs: number;
    signal?: AbortSignal;
  }): Promise<SorafsUptimeObservationResponse>;
  recordSorafsPorChallenge(input: {
    challenge?: string | ArrayBuffer | ArrayBufferView | Buffer;
    challengeB64?: string;
    signal?: AbortSignal;
  }): Promise<SorafsPorSubmissionResponse>;
  recordSorafsPorProof(input: {
    proof?: string | ArrayBuffer | ArrayBufferView | Buffer;
    proofB64?: string;
    signal?: AbortSignal;
  }): Promise<SorafsPorSubmissionResponse>;
  recordSorafsPorVerdict(input: {
    verdict?: string | ArrayBuffer | ArrayBufferView | Buffer;
    verdictB64?: string;
    signal?: AbortSignal;
  }): Promise<SorafsPorVerdictResponse>;
  submitSorafsPorObservation(input: {
    success: boolean;
    signal?: AbortSignal;
  }): Promise<SorafsPorObservationResponse>;
  getSorafsPorStatus(options?: SorafsPorStatusOptions): Promise<Buffer>;
  exportSorafsPorStatus(options?: SorafsPorExportOptions): Promise<Buffer>;
  getSorafsPorWeeklyReport(
    isoWeek: SorafsIsoWeekInput,
    options?: { signal?: AbortSignal },
  ): Promise<Buffer>;
  getUaidPortfolio(
    uaid: string,
    options?: UaidPortfolioQueryOptions,
  ): Promise<UaidPortfolioResponse>;
  getUaidBindings(
    uaid: string,
    options?: UaidBindingsQueryOptions,
  ): Promise<UaidBindingsResponse>;
  getUaidManifests(
    uaid: string,
    options?: UaidManifestQueryOptions,
  ): Promise<UaidManifestsResponse>;
  publishSpaceDirectoryManifest(
    request: PublishSpaceDirectoryManifestRequest,
    options?: { signal?: AbortSignal },
  ): Promise<unknown | null>;
  revokeSpaceDirectoryManifest(
    request: RevokeSpaceDirectoryManifestRequest,
    options?: { signal?: AbortSignal },
  ): Promise<unknown | null>;
  submitTransaction(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
  ): Promise<unknown>;
  getTransactionStatus(
    hashHex: string,
    options?: {
      allowShortHash?: boolean;
      signal?: AbortSignal;
      scope?: "local" | "auto" | "global";
      endpoints?: ReadonlyArray<string> | string;
    },
  ): Promise<ToriiPipelineTransactionStatus | null>;
  waitForTransactionStatus(
    hashHex: string,
    options?: TransactionStatusPollOptions,
  ): Promise<ToriiPipelineTransactionStatus>;
  submitTransactionAndWait(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options: SubmitTransactionAndWaitOptions,
  ): Promise<ToriiPipelineTransactionStatus>;
  getTransactionStatusTyped(
    hashHex: string,
    options?: {
      allowShortHash?: boolean;
      signal?: AbortSignal;
      scope?: "local" | "auto" | "global";
      endpoints?: ReadonlyArray<string> | string;
    },
  ): Promise<ToriiPipelineStatus | null>;
  waitForTransactionStatusTyped(
    hashHex: string,
    options?: TransactionStatusPollOptions,
  ): Promise<ToriiPipelineStatus | null>;
  submitTransactionAndWaitTyped(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options: SubmitTransactionAndWaitOptions,
  ): Promise<ToriiPipelineStatus | null>;
  getPipelineRecovery(
    height: number | string | bigint,
  ): Promise<Record<string, unknown> | null>;
  getPipelineRecoveryTyped(
    height: number | string | bigint,
  ): Promise<ToriiPipelineRecoverySidecar | null>;
  getPipelinePreflight(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiPipelinePreflight>;
  getPipelineRecoveryFastpqProofs(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getPipelineRecoveryFastpqProofsTyped(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiPipelineRecoveryFastpqProofs | null>;
  getHealth(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiHealthStatus | null>;
  getConfiguration(): Promise<unknown | null>;
  getConfigurationTyped(): Promise<ToriiConfigurationSnapshot | null>;
  getConfidentialGasSchedule(): Promise<ConfidentialGasSchedule | null>;
  getStatusSnapshot(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiStatusSnapshot>;
  deploySoracloudAppInfra(
    request: SoracloudAppInfraRequest | Record<string, unknown>,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  upgradeSoracloudAppInfra(
    request: SoracloudAppInfraRequest | Record<string, unknown>,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  getSoracloudAppInfraStatus(options?: {
    appName?: string;
    auditLimit?: NumericLike;
    signal?: AbortSignal;
  }): Promise<unknown>;
  getSoracloudNamedAppInfraStatus(
    appName: string,
    options?: { auditLimit?: NumericLike; signal?: AbortSignal },
  ): Promise<unknown>;
  getNetworkTimeNow(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiNetworkTimeNow>;
  getNetworkTimeStatus(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiNetworkTimeStatus>;
  getNodeCapabilities(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiNodeCapabilities>;
  getSccpCapabilities(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSccpCapabilities>;
  getSccpProofManifests(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSccpProofManifestSet>;
  submitBridgeProof(
    payload: ToriiBridgeProofSubmitPayload,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown>>;
  submitBridgeMessage(
    payload: ToriiBridgeMessageSubmitPayload,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown>>;
  getSccpMessageProofArtifact(
    messageIdHex: string | Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
    options?: ToriiSccpEvmDestinationQueryOptions,
  ): Promise<ToriiSccpMessageTransparentProofArtifact>;
  getSccpMessageProofJob(
    messageIdHex: string | Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
    options?: ToriiSccpEvmDestinationQueryOptions,
  ): Promise<ToriiSccpCounterpartyProofJob>;
  getRuntimeAbiActive(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiRuntimeAbiActiveResponse>;
  getRuntimeAbiHash(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiRuntimeAbiHashResponse>;
  getRuntimeMetrics(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiRuntimeMetrics>;
  listRuntimeUpgrades(options?: {
    signal?: AbortSignal;
  }): Promise<ReadonlyArray<ToriiRuntimeUpgradeListItem>>;
  proposeRuntimeUpgrade(
    manifest: ToriiRuntimeUpgradeManifestInput,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiRuntimeUpgradeTxResponse>;
  activateRuntimeUpgrade(
    idHex: string | BinaryLike,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiRuntimeUpgradeTxResponse>;
  cancelRuntimeUpgrade(
    idHex: string | BinaryLike,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiRuntimeUpgradeTxResponse>;
  listPeers(options?: {
    signal?: AbortSignal;
  }): Promise<Array<Record<string, unknown>>>;
  listPeersTyped(options?: {
    signal?: AbortSignal;
  }): Promise<Array<ToriiPeerRecord>>;
  listTelemetryPeersInfo(options?: {
    signal?: AbortSignal;
  }): Promise<ReadonlyArray<ToriiTelemetryPeerInfo>>;
  getExplorerMetrics(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiExplorerMetricsSnapshot | null>;
  getExplorerAccountQr(
    accountId: string,
    options?: {
      signal?: AbortSignal;
    },
  ): Promise<ToriiExplorerAccountQrSnapshot>;
  getVpnProfile(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiVpnProfile | null>;
  createVpnQuote(
    request: {
      exitClass?: string;
      meteringPublicKeyHex: string;
    },
    options: {
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ToriiVpnQuote>;
  createVpnSession(
    request: {
      exitClass?: string;
      quoteId: string;
      paymentTxHash: string;
      meteringPublicKeyHex: string;
    },
    options: {
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ToriiVpnSession>;
  getVpnSession(
    sessionId: string,
    options: {
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ToriiVpnSession | null>;
  deleteVpnSession(
    sessionId: string,
    options?: {
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ToriiVpnReceipt | null>;
  submitVpnReceipt(
    request: {
      relayReceiptHex: string;
      clientVoucherHex: string;
      leaseIdHex?: string;
    },
    options: {
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ToriiVpnReceipt>;
  listVpnReceipts(options: {
    signal?: AbortSignal;
    canonicalAuth: CanonicalRequestAuth;
  }): Promise<ReadonlyArray<ToriiVpnReceipt>>;
  getSnsPolicy(
    suffixId: number,
    options?: { signal?: AbortSignal },
  ): Promise<SnsSuffixPolicy>;
  getSnsRegistration(
    selector: string,
    options?: { signal?: AbortSignal },
  ): Promise<SnsNameRecord>;
  registerSnsName(
    request: SnsRegisterNameRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SnsRegisterNameResponse>;
  renewSnsRegistration(
    selector: string,
    request: SnsRenewNameRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SnsNameRecord>;
  transferSnsRegistration(
    selector: string,
    request: SnsTransferNameRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SnsNameRecord>;
  freezeSnsRegistration(
    selector: string,
    request: SnsFreezeNameRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SnsNameRecord>;
  unfreezeSnsRegistration(
    selector: string,
    request: SnsGovernanceHook,
    options?: { signal?: AbortSignal },
  ): Promise<SnsNameRecord>;
  getGovernanceProposal(
    proposalId: string,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getGovernanceProposalTyped(
    proposalId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceProposalResult>;
  getGovernanceReferendum(
    referendumId: string,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getGovernanceReferendumTyped(
    referendumId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceReferendumResult>;
  getGovernanceTally(
    referendumId: string,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getGovernanceTallyTyped(
    referendumId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceTallyResult>;
  getGovernanceLocks(
    referendumId: string,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getGovernanceLocksTyped(
    referendumId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceLocksResult>;
  getGovernanceUnlockStats(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, unknown> | null>;
  getGovernanceUnlockStatsTyped(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiGovernanceUnlockStats>;
  getGovernanceCouncilCurrent(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiGovernanceCouncilCurrentResponse>;
  governanceDeriveCouncilVrf(
    payload: ToriiGovernanceCouncilDeriveRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceCouncilDeriveResponse>;
  governancePersistCouncil(
    payload: ToriiGovernanceCouncilPersistRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceCouncilPersistResponse>;
  governanceReplaceCouncil(
    payload: ToriiGovernanceCouncilReplaceRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceCouncilReplaceResponse>;
  getGovernanceCouncilAudit(
    options?: ToriiGovernanceCouncilAuditOptions,
  ): Promise<ToriiGovernanceCouncilAuditResponse>;
  draftMinistryAgendaProposal(
    payload: MinistryAgendaProposalDraftRequest,
    options?: { signal?: AbortSignal },
  ): Promise<MinistryAgendaProposalDraftResponse>;
  getMinistryAgendaProposal(
    proposalId: string,
    options?: { signal?: AbortSignal },
  ): Promise<MinistryAgendaProposalGetResponse>;
  governanceFinalizeReferendum(
    payload: ToriiGovernanceFinalizeRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceDraftResponse | null>;
  governanceFinalizeReferendumTyped(
    payload: ToriiGovernanceFinalizeRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceDraftResponse>;
  governanceEnactProposal(
    payload: ToriiGovernanceEnactRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceDraftResponse | null>;
  governanceEnactProposalTyped(
    payload: ToriiGovernanceEnactRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceDraftResponse>;
  governanceProposeDeployContract(
    payload: ToriiGovernanceDeployContractProposalRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceDraftResponse>;
  governanceSubmitPlainBallot(
    payload: ToriiGovernancePlainBallotRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceBallotResponse>;
  governanceSubmitZkBallot(
    payload: ToriiGovernanceZkBallotRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceBallotResponse>;
  governanceSubmitZkBallotV1(
    payload: ToriiGovernanceZkBallotV1Request,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceBallotResponse>;
  governanceSubmitZkBallotProofV1(
    payload: ToriiGovernanceZkBallotProofRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceBallotResponse>;
  setProtectedNamespaces(
    namespaces: string | string[],
    options?: { signal?: AbortSignal },
  ): Promise<ToriiProtectedNamespacesApplyResponse>;
  getProtectedNamespaces(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiProtectedNamespacesGetResponse>;
  getSumeragiStatus(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, unknown>>;
  getSumeragiStatusTyped(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiStatus>;
  getSumeragiPacemaker(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiPacemakerResponse | null>;
  getSumeragiQc(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiQcSnapshot>;
  getSumeragiCommitQc(
    blockHashHex: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiSumeragiCommitQcRecord>;
  getSumeragiPhases(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiPhasesSnapshot>;
  getSumeragiBlsKeys(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, string | null>>;
  getSumeragiLeader(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiLeaderSnapshot>;
  getSumeragiCollectors(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiCollectorsPlan>;
  getSumeragiParams(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiParamsSnapshot>;
  getSumeragiTelemetry(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, unknown>>;
  getSumeragiTelemetryTyped(options?: {
    signal?: AbortSignal;
  }): Promise<SumeragiTelemetrySnapshot>;
  getSumeragiRbc(options?: {
    signal?: AbortSignal;
  }): Promise<SumeragiRbcSnapshot | null>;
  getSumeragiRbcSessions(options?: {
    signal?: AbortSignal;
  }): Promise<SumeragiRbcSessionsSnapshot | null>;
  findRbcSamplingCandidate(options?: {
    signal?: AbortSignal;
  }): Promise<SumeragiRbcSession | null>;
  getSumeragiRbcDelivered(
    height: number | string | bigint,
    view: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<SumeragiRbcDeliveryStatus | null>;
  sampleRbcChunks(
    options: RbcSampleRequestOptions,
  ): Promise<RbcSampleResponse | null>;
  listSumeragiEvidence(
    options?: SumeragiEvidenceListOptions,
  ): Promise<SumeragiEvidenceListResponse>;
  getSumeragiEvidenceCount(): Promise<SumeragiEvidenceCountResponse>;
  submitSumeragiEvidence(
    request: SumeragiEvidenceSubmitRequest,
  ): Promise<SumeragiEvidenceSubmitResponse>;
  getMetrics(options: { asText: true; signal?: AbortSignal }): Promise<string>;
  getMetrics(options?: {
    asText?: boolean;
    signal?: AbortSignal;
  }): Promise<unknown>;
  getBlock(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiExplorerBlock | null>;
  listBlocks(options?: BlockListOptions): Promise<ToriiExplorerBlocksPage>;
  streamEvents<T = ToriiEventPayload>(
    options?: EventStreamOptions,
  ): AsyncGenerator<ToriiSseEvent<T>, void, unknown>;
  streamContractEvents<T = ToriiContractEventItem>(
    options?: ContractEventStreamOptions,
  ): AsyncGenerator<ToriiSseEvent<T>, void, unknown>;
  streamSumeragiStatus<T = ToriiSumeragiStatus>(
    options?: Omit<EventStreamOptions, "filter">,
  ): AsyncGenerator<ToriiSseEvent<T>, void, unknown>;
  getKaigiCall(
    callId: string,
    options?: { signal?: AbortSignal },
  ): Promise<KaigiCallView | null>;
  listKaigiCallSignals(
    callId: string,
    options?: KaigiCallSignalsOptions,
  ): Promise<KaigiCallSignalsList>;
  streamKaigiCallEvents(
    callId: string,
    options?: KaigiCallEventsOptions,
  ): AsyncGenerator<ToriiSseEvent<KaigiCallEventPayload>, void, unknown>;
  listKaigiRelays(options?: {
    signal?: AbortSignal;
  }): Promise<KaigiRelaySummaryList>;
  getKaigiRelay(
    relayId: string,
    options?: { signal?: AbortSignal },
  ): Promise<KaigiRelayDetail | null>;
  getKaigiRelaysHealth(options?: {
    signal?: AbortSignal;
  }): Promise<KaigiRelayHealthSnapshot>;
  streamKaigiRelayEvents(
    options?: KaigiRelayEventsOptions,
  ): AsyncGenerator<ToriiSseEvent<KaigiRelayEventPayload>, void, unknown>;
  listProverReports(
    filters?: ToriiProverReportFilters,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiProverReportListResult>;
  iterateProverReports(
    filters?: ToriiProverReportFilters,
    options?: PaginationIteratorOptions & { signal?: AbortSignal },
  ): AsyncGenerator<
    ToriiProverReport | string | ToriiProverReportMessageSummary,
    void,
    unknown
  >;
  getProverReport(
    reportId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiProverReport>;
  deleteProverReport(
    reportId: string,
    options?: { signal?: AbortSignal },
  ): Promise<void>;
  countProverReports(
    filters?: ToriiProverReportFilters,
    options?: { signal?: AbortSignal },
  ): Promise<number>;
  submitIsoPacs008(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: {
      contentType?: string;
      profile?: string;
      signal?: AbortSignal;
      retryProfile?: string;
    },
  ): Promise<IsoPacs008SubmissionResponse | null>;
  submitIsoPacs009(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: {
      contentType?: string;
      profile?: string;
      signal?: AbortSignal;
      retryProfile?: string;
    },
  ): Promise<IsoPacs009SubmissionResponse | null>;
  submitIsoPacs008AndWait(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: {
      contentType?: string;
      profile?: string;
      signal?: AbortSignal;
      retryProfile?: string;
      wait?: IsoMessageWaitOptions;
    },
  ): Promise<IsoMessageStatusResponse>;
  submitIsoPacs009AndWait(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: {
      contentType?: string;
      profile?: string;
      signal?: AbortSignal;
      retryProfile?: string;
      wait?: IsoMessageWaitOptions;
    },
  ): Promise<IsoMessageStatusResponse>;
  submitIsoMessage(
    message: BuildPacs008Options | BuildPacs009Options,
    options?: SubmitIsoMessageOptions,
  ): Promise<
    IsoMessageSubmissionResponseBase | IsoMessageStatusResponse | null
  >;
  getIsoMessageStatus(
    messageId: string,
    options?: { signal?: AbortSignal; retryProfile?: string },
  ): Promise<IsoMessageStatusResponse | null>;
  waitForIsoMessageStatus(
    messageId: string,
    options?: IsoMessageWaitOptions,
  ): Promise<IsoMessageStatusResponse>;
  getConnectStatus(): Promise<ConnectStatusSnapshot | null>;
  createConnectSession(input: {
    sid: string;
    node?: string | null;
  }): Promise<ConnectSessionResponse>;
  deleteConnectSession(input: {
    sid: string;
    tokenManagement?: string;
    token_management?: string;
  }): Promise<boolean>;
  listConnectApps(
    options?: ConnectAppListOptions,
  ): Promise<ConnectAppRegistryPage>;
  iterateConnectApps(
    options?: ConnectAppIteratorOptions,
  ): AsyncGenerator<ConnectAppRecord, void, unknown>;
  getConnectApp(
    appId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAppRecord>;
  registerConnectApp(
    record: ConnectAppUpsertInput,
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAppRecord | null>;
  deleteConnectApp(appId: string): Promise<boolean>;
  getConnectAppPolicy(options?: {
    signal?: AbortSignal;
  }): Promise<ConnectAppPolicyControls>;
  updateConnectAppPolicy(
    updates: ConnectAppPolicyUpdate,
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAppPolicyControls>;
  getConnectAdmissionManifest(options?: {
    signal?: AbortSignal;
  }): Promise<ConnectAdmissionManifest>;
  setConnectAdmissionManifest(
    manifest: ConnectAdmissionManifestInput,
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAdmissionManifest>;
  buildConnectWebSocketUrl(options: ConnectWebSocketParams): string;
  openConnectWebSocket<T = unknown>(
    options: ClientConnectWebSocketOptions<T>,
  ): T;
  static buildConnectWebSocketUrl(
    baseUrl: string,
    options: ConnectWebSocketParams,
  ): string;
  static buildRbcSampleRequest(
    session: SumeragiRbcSession,
    overrides?: RbcSampleRequestOverrides,
  ): RbcSampleRequestOptions;
  registerContractCode(
    request: RegisterContractCodeRequest,
  ): Promise<unknown | null>;
  deployContract(
    request: DeployContractRequest,
  ): Promise<DeployContractResponse | null>;
  setContractAlias(
    request: SetContractAliasRequest,
  ): Promise<SetContractAliasResponse | null>;
  callContract(
    request: ContractCallRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ContractCallResponse>;
  proposeMultisig(
    request: MultisigProposeRequest,
    options?: { signal?: AbortSignal },
  ): Promise<MultisigContractCallResponse>;
  proposeMultisigContractCall(
    request: MultisigContractCallProposeRequest,
    options?: { signal?: AbortSignal },
  ): Promise<MultisigContractCallResponse>;
  approveMultisigContractCall(
    request: MultisigContractCallApproveRequest,
    options?: { signal?: AbortSignal },
  ): Promise<MultisigContractCallResponse>;
  getMultisigSpec(
    request: MultisigAccountSelector,
    options?: { signal?: AbortSignal },
  ): Promise<MultisigSpecResponse>;
  listMultisigProposals(
    request: MultisigAccountSelector,
    options?: { signal?: AbortSignal },
  ): Promise<MultisigProposalListResponse>;
  getMultisigProposal(
    request: MultisigAccountSelector & {
      proposalId?: string | null;
      instructionsHash?: string | null;
    },
    options?: { signal?: AbortSignal },
  ): Promise<MultisigProposalGetResponse>;
  getContractManifest(
    codeHashHex: string,
  ): Promise<ContractManifestRecord | null>;
  getContractCodeBytes(
    codeHashHex: string,
  ): Promise<ContractCodeBytesRecord | null>;
  getGovernanceContract(
    contractAddress: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceContractResponse>;
  listTriggers(options?: TriggerListOptions): Promise<ToriiTriggerListPage>;
  iterateTriggers(
    options?: TriggerIteratorOptions,
  ): AsyncGenerator<ToriiTriggerRecord, void, unknown>;
  getTrigger(
    triggerId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiTriggerRecord | null>;
  registerTrigger(
    trigger: ToriiTriggerUpsertRequest,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  registerTriggerTyped(
    trigger: ToriiTriggerUpsertRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiTriggerMutationResponse | null>;
  deleteTrigger(
    triggerId: string,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  deleteTriggerTyped(
    triggerId: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiTriggerMutationResponse | null>;
  queryTriggers(options?: IterableQueryOptions): Promise<ToriiTriggerListPage>;
  iterateTriggersQuery(
    options?: TriggerQueryIteratorOptions,
  ): AsyncGenerator<ToriiTriggerRecord, void, unknown>;
  listSubscriptionPlans(
    options?: SubscriptionPlanListOptions,
  ): Promise<SubscriptionPlanListResponse>;
  iterateSubscriptionPlans(
    options?: SubscriptionPlanIteratorOptions,
  ): AsyncGenerator<SubscriptionPlanListItem, void, unknown>;
  createSubscriptionPlan(
    request: SubscriptionPlanCreateRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionPlanCreateResponse>;
  listSubscriptions(
    options?: SubscriptionListOptions,
  ): Promise<SubscriptionListResponse>;
  iterateSubscriptions(
    options?: SubscriptionIteratorOptions,
  ): AsyncGenerator<SubscriptionListItem, void, unknown>;
  createSubscription(
    request: SubscriptionCreateRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionCreateResponse>;
  getSubscription(
    subscriptionId: string,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionGetResponse | null>;
  pauseSubscription(
    subscriptionId: string,
    request: SubscriptionActionRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionActionResponse>;
  resumeSubscription(
    subscriptionId: string,
    request: SubscriptionActionRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionActionResponse>;
  cancelSubscription(
    subscriptionId: string,
    request: SubscriptionActionRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionActionResponse>;
  keepSubscription(
    subscriptionId: string,
    request: SubscriptionActionRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionActionResponse>;
  chargeSubscriptionNow(
    subscriptionId: string,
    request: SubscriptionActionRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionActionResponse>;
  recordSubscriptionUsage(
    subscriptionId: string,
    request: SubscriptionUsageRequest,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionActionResponse>;
  getOfflineReadiness(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiOfflineReadinessResponse>;
}

export interface NoritoRpcClientOptions {
  fetchImpl?: typeof fetch;
  timeoutMs?: number;
  defaultHeaders?: Record<string, string>;
  allowInsecure?: boolean;
  authToken?: string | null;
  apiToken?: string | null;
  insecureTransportTelemetryHook?: (
    event: InsecureTransportTelemetryEvent,
  ) => void;
}

export interface NoritoRpcCallOptions {
  timeoutMs?: number;
  headers?: Record<string, string | null | undefined>;
  accept?: string | null;
  method?: string;
  params?: Record<string, string | number | boolean>;
  signal?: AbortSignal;
  allowAbsoluteUrl?: boolean;
  authToken?: string | null;
  apiToken?: string | null;
}

export declare class NoritoRpcClient {
  constructor(baseUrl: string, options?: NoritoRpcClientOptions);
  readonly baseUrl: string;
  call(
    path: string,
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options?: NoritoRpcCallOptions,
  ): Promise<Uint8Array>;
  close(): void;
}

export declare class NoritoRpcError extends Error {
  readonly status: number;
  readonly body: string;
}

export function supportedCryptoAlgorithms(): CryptoAlgorithm[];

export function normalizeCryptoAlgorithm(
  algorithm?: string | null,
): CryptoAlgorithm;

export function generateKeyPair(options?: {
  seed?: ArrayBufferView | ArrayBuffer | Buffer;
  algorithm?: string | null;
}): CryptoKeyPair;

export function loadKeyPair(
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { algorithm?: string | null },
): CryptoKeyPair;

export function publicKeyFromPrivate(
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { algorithm?: string | null },
): Buffer;

export function sign(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { algorithm?: string | null },
): Buffer;

export function verify(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  signature: ArrayBufferView | ArrayBuffer | Buffer,
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { algorithm?: string | null },
): boolean;

export function publicKeyMultihash(
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { algorithm?: string | null },
): string;

export function privateKeyMultihash(
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { algorithm?: string | null },
): string;

export function generateSm2KeyPair(options?: { distid?: string }): Sm2KeyPair;

export function deriveSm2KeyPairFromSeed(
  seed: ArrayBufferView | ArrayBuffer | Buffer | string,
  distid?: string,
): Sm2KeyPair;

export function loadSm2KeyPair(
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
  distid?: string,
): Sm2KeyPair;

export function sm2PublicKeyMultihash(
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
  distid?: string,
): string;

export function signSm2(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
  distid?: string,
): Buffer;

export function verifySm2(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  signature: ArrayBufferView | ArrayBuffer | Buffer,
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
  distid?: string,
): boolean;

export function buildKaigiRosterJoinProof(
  options: KaigiRosterJoinProofOptions,
): KaigiRosterJoinProof;

export function buildZkAceTransferAuthorizationV1(
  options: ZkAceTransferAuthorizationV1Options,
): ZkAceTransferAuthorizationV1;

export function signEd25519(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function verifyEd25519(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  signature: ArrayBufferView | ArrayBuffer | Buffer,
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
): boolean;

export function canonicalQueryString(
  query?: string | URLSearchParams | null,
): string;

export function canonicalRequestMessage(params: {
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: Buffer | ArrayBuffer | ArrayBufferView | string;
}): Buffer;

export function canonicalRequestSignatureMessage(params: {
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: Buffer | ArrayBuffer | ArrayBufferView | string;
  timestampMs: number;
  nonce: string;
}): Buffer;

export function buildCanonicalRequestHeaders(params: {
  accountId: string;
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: Buffer | ArrayBuffer | ArrayBufferView | string;
  privateKey: ArrayBufferView | ArrayBuffer | Buffer;
  timestampMs?: number;
  nonce?: string;
}): {
  "X-Iroha-Account": string;
  "X-Iroha-Signature": string;
  "X-Iroha-Timestamp-Ms": string;
  "X-Iroha-Nonce": string;
};

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

export function buildCanonicalJsonRequest(params: {
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
  privateKey?: ArrayBufferView | ArrayBuffer | Buffer;
  sign?: (
    input: CanonicalJsonRequestSignerInput,
  ) => CanonicalJsonRequestSignature | Promise<CanonicalJsonRequestSignature>;
  timestampMs?: number;
  nonce?: string;
}): Promise<{
  method: string;
  headers: Record<string, string>;
  body: string;
}>;

export function deriveConfidentialKeyset(
  spendKey: ArrayBufferView | ArrayBuffer | Buffer,
): ConfidentialKeyset;

export function deriveConfidentialKeysetFromHex(
  spendKeyHex: string,
): ConfidentialKeyset;

export function deriveConfidentialOwnerTagV2(
  spendKey: ArrayBufferView | ArrayBuffer | Buffer,
  options?: {
    diversifierHex?: string;
    diversifier?: ArrayBufferView | ArrayBuffer | Buffer;
  },
): Buffer;

export function deriveConfidentialDiversifierV2(
  seed: ArrayBufferView | ArrayBuffer | Buffer | string,
): {
  diversifier: Buffer;
  diversifierHex: string;
};

export function deriveConfidentialReceiveAddressV2(input: {
  spendKey: ArrayBufferView | ArrayBuffer | Buffer;
  diversifierSeed: ArrayBufferView | ArrayBuffer | Buffer | string;
}): ConfidentialReceiveAddressV2;

export function deriveConfidentialNoteV2(input: {
  assetDefinitionId: string;
  amount: NumericLike;
  rhoHex?: string;
  rho?: ArrayBufferView | ArrayBuffer | Buffer;
  ownerTagHex?: string;
  ownerTag?: ArrayBufferView | ArrayBuffer | Buffer;
}): { commitment: Buffer; commitmentHex: string };

export function deriveConfidentialNullifierV2(input: {
  chainId: string;
  assetDefinitionId: string;
  spendKey: ArrayBufferView | ArrayBuffer | Buffer;
  rhoHex?: string;
  rho?: ArrayBufferView | ArrayBuffer | Buffer;
}): { nullifier: Buffer; nullifierHex: string };

export const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1: "recursive_compact_v1";
export const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1: "recursive_spend_v1";
export const KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1: "checked_prefold_v1";
export const KAGEMUSHA_RECURSIVE_SPEND_REQUIRED_BRIDGE_ABI_VERSION: 6;
export const KAGEMUSHA_RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION: 7;
export const KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1: "kagemusha-recursive-compact-v1";
export const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1: "kagemusha-recursive-aggregation-v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1: "kagemusha-recursive-spend-lineage-v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1: "kagemusha-recursive-spend-lineage-onehop-v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1: "kagemusha-recursive-spend-lineage-append-v1";
export const KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS: 64;
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: 64;
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1: true;
export const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1: 1;
export const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES: 8388608;
export const KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES: 128;
export const KAGEMUSHA_NATIVE_ARCHIVE_MAX_BYTES: 67108864;
export const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN: "iroha:kagemusha:v1:recursive-spend-transition-profile";
export const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN: "iroha:kagemusha:v1:recursive-spend-transition-profile-digest";
export const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN: "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1: "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1: "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1: "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1";
export const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1: "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1";
export type KagemushaOfflineSpendMode =
  | "recursive_compact_v1"
  | "recursive_spend_v1"
  | "checked_prefold_v1";
export function preferredKagemushaOfflineSpendMode(
  recursiveSpendAvailable?: boolean,
  recursiveCompactAvailable?: boolean,
): KagemushaOfflineSpendMode;
export function preferredKagemushaOfflineSpendModeForCapabilities(
  recursiveCompactAvailable: boolean,
  recursiveSpendAvailable: boolean,
): KagemushaOfflineSpendMode;
export function canRedeemKagemushaRecursiveSpendWitnessless(
  proofCircuitId: string,
  hopCount: number,
): boolean;
export function isKagemushaRecursiveSpendLineageProofCircuitId(
  proofCircuitId?: string | null,
): boolean;
export function isKagemushaRecursiveSpendLineageAppendOutputCircuitId(
  outputProofCircuitId?: string | null,
): boolean;
export function requiresKagemushaRecursiveSpendLineageKeyArtifactsForInit(): boolean;
export function requiresKagemushaRecursiveSpendLineageKeyArtifactsForAppendOutput(
  outputProofCircuitId?: string | null,
): boolean;
export function requiresKagemushaRecursiveSpendLineageWitnessForRedeem(
  proofCircuitId: string,
  hopCount: number,
): boolean;
export function canAppendKagemushaRecursiveSpendWitnesslessLineage(
  previousHopCount: number,
): boolean;
export function normalizeKagemushaRecursiveSpendAppendOutputProofCircuitId(
  outputProofCircuitId?: string | null,
): string;
export function isSupportedKagemushaRecursiveSpendAppendOutputProofCircuitId(
  outputProofCircuitId?: string | null,
): boolean;
export function isSupportedKagemushaRecursiveSpendPreviousProofCircuitId(
  previousProofCircuitId?: string | null,
): boolean;
export function requiresKagemushaRecursiveSpendPreviousLineageVerifierRecordForAppend(
  previousProofCircuitId?: string | null,
): boolean;
export function isSupportedKagemushaRecursiveSpendAppendProofTransition(
  previousProofCircuitId: string | null | undefined,
  outputProofCircuitId: string | null | undefined,
): boolean;
export function preferredKagemushaRecursiveSpendAppendOutputProofCircuitId(
  previousHopCount: number,
): string;
export function canProveKagemushaRecursiveSpendAppendOutputProofCircuitId(
  outputProofCircuitId: string | null | undefined,
  previousHopCount: number,
): boolean;
export function canSelectKagemushaRecursiveSpendAppendOutputProofCircuitId(
  previousProofCircuitId: string | null | undefined,
  outputProofCircuitId: string | null | undefined,
  previousHopCount: number,
): boolean;
export function requiresKagemushaRecursiveSpendPreviousProofOpenEnvelopesForAppend(
  outputProofCircuitId: string | null | undefined,
  previousHopCount: number,
): boolean;
export function isKagemushaRecursiveSpendNativeAvailable(): boolean;
export function isKagemushaCompactPaymentTokenNativeAvailable(): boolean;
export function isKagemushaRecursiveAggregationProofBundleNativeAvailable(): boolean;
export function isKagemushaRecursiveCompactPaymentTokenNativeAvailable(): boolean;
export function kagemushaProveVerifiedCompactPaymentTokenWithRecords(
  recordBundleArchive: BinaryLike,
): Buffer;
export function kagemushaProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
  recordBundleArchive: BinaryLike,
  pallasOpenEnvelopesArchive: BinaryLike,
): Buffer;
export function kagemushaProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
  recordBundleArchive: BinaryLike,
  pallasOpenEnvelopesArchive: BinaryLike,
): Buffer;
export function kagemushaVerifyRecursiveCompactPaymentToken(
  compactTokenArchive: BinaryLike,
): boolean;
export function kagemushaRecursiveSpendInit(requestArchive: BinaryLike): Buffer;
export function kagemushaRecursiveSpendAppend(
  requestArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendTransitionProfileInit(
  requestArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendTransitionProfileAppend(
  requestArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendLineageAppendBoundary(
  profileArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendLineageWitnessFromInitResult(
  requestArchive: BinaryLike,
  bundleArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendLineageWitnessAppendResult(
  previousWitnessArchive: BinaryLike,
  requestArchive: BinaryLike,
  bundleArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendVerify(
  requestArchive: BinaryLike,
): Buffer;
export function kagemushaRecursiveSpendRedeem(
  requestArchive: BinaryLike,
): Buffer;
export const PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: number;
export function isPrivacyNativeAvailable(): boolean;
export function privacyCapabilitiesV1(): Buffer;
export function privacyBuildProofV1(
  requestArchive: PrivacyNativeArchiveLike,
): Buffer;
export function privacyVerifyProofV1(
  requestArchive: PrivacyNativeArchiveLike,
): Buffer;

export interface Sm2Fixture {
  distid: string;
  seedHex: string;
  messageHex: string;
  privateKeyHex: string;
  publicKeySec1Hex: string;
  publicKeyMultihash: string;
  publicKeyPrefixed: string;
  za: string;
  signature: string;
  r: string;
  s: string;
}

export function sm2FixtureFromSeed(
  distid: string,
  seed: ArrayBufferView | ArrayBuffer | Buffer | string,
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
): Sm2Fixture;

export function noritoEncodeInstruction(instruction: object | string): Buffer;
export function noritoEncodePrivacyProofEnvelope(envelope: object): Buffer;
export function noritoDecodePrivacyProofEnvelope(
  bytes: ArrayBufferView | ArrayBuffer | Buffer | string,
): object;
export interface MultisigProposeNoritoRequest {
  multisig_account_id?: string | null;
  multisigAccountId?: string | null;
  multisig_account_alias?: string | null;
  multisigAccountAlias?: string | null;
  signer_account_id?: string;
  signerAccountId?: string;
  private_key?: string | null;
  privateKey?: string | null;
  public_key_hex?: string | null;
  publicKeyHex?: string | null;
  signature_b64?: string | null;
  signatureB64?: string | null;
  creation_time_ms?: number | string | bigint | null;
  creationTimeMs?: number | string | bigint | null;
  fee_sponsor?: string | null;
  feeSponsor?: string | null;
  instructions: Array<object | string | ArrayBufferView | ArrayBuffer | Buffer>;
}
export function noritoEncodeMultisigProposeRequest(
  request: MultisigProposeNoritoRequest,
): Buffer;
export function noritoEncodeMultisigContractCallProposeRequest(
  request: MultisigContractCallProposeRequest,
): Buffer;
export function noritoEncodeMultisigContractCallApproveRequest(
  request: MultisigContractCallApproveRequest,
): Buffer;
export function noritoDecodeInstruction(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { parseJson?: boolean },
): JsonValue;

export interface LaneRelaySample {
  valid: Buffer;
  tampered: Buffer;
}

export function laneRelayEnvelopeSample(): LaneRelaySample;
export function verifyLaneRelayEnvelope(
  envelope: ArrayBufferView | ArrayBuffer | Buffer | string,
): void;
export function verifyLaneRelayEnvelopeJson(envelope: object | string): void;
export function verifyLaneRelayEnvelopes(
  envelopes: Array<object | string>,
): void;
export function decodeLaneRelayEnvelope(
  envelope: ArrayBufferView | ArrayBuffer | Buffer | string,
): JsonValue;
export function laneSettlementHash(settlement: object | string): string;

export interface AxtTouchManifest {
  read: ReadonlyArray<string>;
  write: ReadonlyArray<string>;
}

export interface AxtTouchFragment {
  dsid: number;
  manifest: AxtTouchManifest;
}

export interface AxtTouchSpec {
  dsid: number;
  read: ReadonlyArray<string>;
  write: ReadonlyArray<string>;
}

export interface AxtDescriptorShape {
  dsids: ReadonlyArray<number>;
  touches: ReadonlyArray<AxtTouchSpec>;
}

export interface AxtDescriptorBuild {
  descriptor: AxtDescriptorShape;
  descriptorBytes: Buffer;
  bindingHex: string;
  binding: Buffer;
  touchManifest: ReadonlyArray<AxtTouchFragment>;
  native: true;
}

export function buildTouchManifest(
  read: Iterable<string> | ArrayLike<string>,
  write: Iterable<string> | ArrayLike<string>,
): AxtTouchManifest;

export function buildAxtDescriptor(options: {
  dsids: Iterable<number> | ArrayLike<number>;
  touches?: Iterable<{
    dsid: number;
    read?: Iterable<string> | ArrayLike<string>;
    write?: Iterable<string> | ArrayLike<string>;
  }>;
  touchManifest?: Iterable<{
    dsid: number;
    manifest?: Partial<AxtTouchManifest>;
    read?: Iterable<string> | ArrayLike<string>;
    write?: Iterable<string> | ArrayLike<string>;
  }>;
}): AxtDescriptorBuild;

export function computeAxtBinding(
  descriptorBytes: Buffer | Uint8Array | ArrayBuffer,
): Buffer;

export interface AxtRejectContext {
  reason: string;
  dataspace: number | null;
  lane: number | null;
  snapshot_version: number | null;
  detail: string;
  next_min_handle_era: number | null;
  next_min_sub_nonce: number | null;
}

export interface AxtHandleRefreshHint {
  dataspace: number | null;
  targetLane: number | null;
  nextMinHandleEra: number | null;
  nextMinSubNonce: number | null;
  reason: string;
  snapshotVersion: number | null;
  detail: string;
}

export function normalizeAxtRejectContext(
  ctx: unknown,
  context?: string,
): AxtRejectContext;

export function buildHandleRefreshRequest(
  ctx: unknown,
  overrides?: Partial<AxtHandleRefreshHint>,
): AxtHandleRefreshHint;

export function hashSignedTransaction(
  signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
  options?: { encoding?: BufferEncoding | "buffer" },
): string | Buffer;

export function resignSignedTransaction(
  signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export const OfflineQrStreamFrameKind: Readonly<{
  header: number;
  data: number;
  parity: number;
}>;

export const OfflineQrStreamFrameEncoding: Readonly<{
  binary: "binary";
  base64: "base64";
}>;

export const OfflineQrPayloadKind: Readonly<{
  unspecified: number;
  offlineReceiveRequest: number;
  offlinePaymentToken: number;
  offlineReceiptAck: number;
}>;

export class OfflineQrStreamOptions {
  chunkSize: number;
  parityGroup: number;
  payloadKind: number;
  constructor(options?: {
    chunkSize?: number;
    parityGroup?: number;
    payloadKind?: number;
  });
}

export class OfflineQrStreamEnvelope {
  readonly flags: number;
  readonly encoding: number;
  readonly parityGroup: number;
  readonly chunkSize: number;
  readonly dataChunks: number;
  readonly parityChunks: number;
  readonly payloadKind: number;
  readonly payloadLength: number;
  readonly payloadHash: Buffer;
  constructor(input: {
    flags?: number;
    encoding?: number;
    parityGroup?: number;
    chunkSize: number;
    dataChunks: number;
    parityChunks: number;
    payloadKind: number;
    payloadLength: number;
    payloadHash: ArrayBufferView | ArrayBuffer | Buffer;
  });
  get streamId(): Buffer;
  encode(): Buffer;
  static decode(
    bytes: ArrayBufferView | ArrayBuffer | Buffer,
  ): OfflineQrStreamEnvelope;
}

export class OfflineQrStreamFrame {
  readonly kind: number;
  readonly streamId: Buffer;
  readonly index: number;
  readonly total: number;
  readonly payload: Buffer;
  constructor(input: {
    kind: number;
    streamId: ArrayBufferView | ArrayBuffer | Buffer;
    index: number;
    total: number;
    payload?: ArrayBufferView | ArrayBuffer | Buffer;
  });
  encode(): Buffer;
  static decode(
    bytes: ArrayBufferView | ArrayBuffer | Buffer,
  ): OfflineQrStreamFrame;
}

export class OfflineQrStreamEncoder {
  static encodeFrames(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options?: {
      chunkSize?: number;
      parityGroup?: number;
      payloadKind?: number;
    },
  ): OfflineQrStreamFrame[];
  static encodeFrameBytes(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options?: {
      chunkSize?: number;
      parityGroup?: number;
      payloadKind?: number;
    },
  ): Buffer[];
}

export class OfflineQrStreamDecoder {
  ingest(frameBytes: ArrayBufferView | ArrayBuffer | Buffer): {
    payload: Buffer | null;
    receivedChunks: number;
    totalChunks: number;
    recoveredChunks: number;
    progress: number;
    isComplete: boolean;
  };
}

export class OfflineQrStreamScanSession {
  constructor(options?: { frameEncoding?: "binary" | "base64" });
  ingest(
    frame: string | ArrayBufferView | ArrayBuffer | Buffer,
    encoding?: "binary" | "base64",
  ): {
    payload: Buffer | null;
    receivedChunks: number;
    totalChunks: number;
    recoveredChunks: number;
    progress: number;
    isComplete: boolean;
  };
}

export interface OfflineQrStreamColor {
  red: number;
  green: number;
  blue: number;
}

export class OfflineQrStreamTheme {
  readonly name: string;
  readonly backgroundStart: OfflineQrStreamColor;
  readonly backgroundEnd: OfflineQrStreamColor;
  readonly accent: OfflineQrStreamColor;
  readonly petal: OfflineQrStreamColor;
  readonly petalCount: number;
  readonly pulsePeriod: number;
  constructor(input: {
    name: string;
    backgroundStart: OfflineQrStreamColor;
    backgroundEnd: OfflineQrStreamColor;
    accent: OfflineQrStreamColor;
    petal: OfflineQrStreamColor;
    petalCount: number;
    pulsePeriod: number;
  });
  frameStyle(
    frameIndex: number,
    totalFrames: number,
  ): { petalPhase: number; accentStrength: number; gradientAngle: number };
}

export class OfflineQrStreamPlaybackSkin {
  readonly name: string;
  readonly theme: OfflineQrStreamTheme;
  readonly frameRate: number;
  readonly petalDriftSpeed: number;
  readonly progressOverlayAlpha: number;
  readonly reducedMotion: boolean;
  readonly lowPower: boolean;
  constructor(input: {
    name: string;
    theme?: OfflineQrStreamTheme;
    frameRate?: number;
    petalDriftSpeed?: number;
    progressOverlayAlpha?: number;
    reducedMotion?: boolean;
    lowPower?: boolean;
  });
  frameStyle(
    frameIndex: number,
    totalFrames: number,
    progress?: number,
  ): {
    petalPhase: number;
    accentStrength: number;
    gradientAngle: number;
    driftOffset: number;
    progressAlpha: number;
  };
}

export const sakuraQrStreamTheme: OfflineQrStreamTheme;
export const sakuraQrStreamSkin: OfflineQrStreamPlaybackSkin;
export const sakuraQrStreamReducedMotionSkin: OfflineQrStreamPlaybackSkin;
export const sakuraQrStreamLowPowerSkin: OfflineQrStreamPlaybackSkin;

export function encodeQrFrameText(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  encoding?: "binary" | "base64",
): string;

export function decodeQrFrameText(
  value: string,
  encoding?: "binary" | "base64",
): Buffer;

export function scanQrStreamFrames(
  frames:
    | AsyncIterable<string | ArrayBufferView | ArrayBuffer | Buffer>
    | Iterable<string | ArrayBufferView | ArrayBuffer | Buffer>,
  options?: {
    session?: OfflineQrStreamScanSession;
    frameEncoding?: "binary" | "base64";
  },
): Promise<{
  payload: Buffer | null;
  receivedChunks: number;
  totalChunks: number;
  recoveredChunks: number;
  progress: number;
  isComplete: boolean;
} | null>;

export function buildRegisterDomainTransaction(
  input: RegisterDomainInput,
): SignedTransactionResult;

/**
 * Assemble and sign a transaction from pre-built instruction payloads. The
 * instruction array must be non-empty; each entry should be either a builder
 * result or a Norito JSON string.
 */
export function buildTransaction(
  input: TransactionAssemblyInput,
): SignedTransactionResult;

/**
 * Assemble and sign a transaction whose executable is `Executable::IvmProved`
 * and whose proof attachment list contains the provided attachment.
 */
export function buildIvmProvedTransaction(
  input: IvmProvedTransactionAssemblyInput,
): SignedTransactionResult;

/**
 * Build and sign a transaction containing a single `Mint::Asset` instruction.
 * Validates the quantity and asset identifier before serialising to Norito.
 */
export function buildMintAssetTransaction(
  input: MintAssetInput,
): SignedTransactionResult;
/**
 * Build and sign a transaction containing a single `Burn::Asset` instruction.
 * Throws if the quantity is non-positive or the asset identifier is empty.
 */
export function buildBurnAssetTransaction(
  input: BurnAssetInput,
): SignedTransactionResult;
/**
 * Build and sign a transaction containing a single `Burn::TriggerRepetitions`
 * instruction. Throws when repetitions are not positive integers.
 */
export function buildBurnTriggerTransaction(
  input: BurnTriggerInput,
): SignedTransactionResult;
export function buildMintTriggerTransaction(
  input: MintTriggerInput,
): SignedTransactionResult;
export function buildTransferAssetTransaction(
  input: TransferAssetInput,
): SignedTransactionResult;
export function buildTransferAssetDefinitionTransaction(
  input: TransferAssetDefinitionInput,
): SignedTransactionResult;
export function buildTransferDomainTransaction(
  input: TransferDomainInput,
): SignedTransactionResult;
export function buildTransferNftTransaction(
  input: TransferNftInput,
): SignedTransactionResult;
export function buildRegisterRwaTransaction(
  input: RegisterRwaInput,
): SignedTransactionResult;
export function buildTransferRwaTransaction(
  input: TransferRwaInput,
): SignedTransactionResult;
export function buildMergeRwasTransaction(
  input: MergeRwasInput,
): SignedTransactionResult;
export function buildRedeemRwaTransaction(
  input: RedeemRwaInput,
): SignedTransactionResult;
export function buildFreezeRwaTransaction(
  input: FreezeRwaInput,
): SignedTransactionResult;
export function buildUnfreezeRwaTransaction(
  input: UnfreezeRwaInput,
): SignedTransactionResult;
export function buildHoldRwaTransaction(
  input: HoldRwaInput,
): SignedTransactionResult;
export function buildReleaseRwaTransaction(
  input: ReleaseRwaInput,
): SignedTransactionResult;
export function buildForceTransferRwaTransaction(
  input: ForceTransferRwaInput,
): SignedTransactionResult;
export function buildSetRwaControlsTransaction(
  input: SetRwaControlsInput,
): SignedTransactionResult;
export function buildSetRwaKeyValueTransaction(
  input: SetRwaKeyValueInput,
): SignedTransactionResult;
export function buildRemoveRwaKeyValueTransaction(
  input: RemoveRwaKeyValueInput,
): SignedTransactionResult;
/**
 * Compose a mint followed by one or more transfers. Provide either `transfer`
 * or `transfers`; transfers without an explicit `sourceAssetHoldingId` reuse the mint's
 * destination asset identifier.
 */
export function buildMintAndTransferTransaction(
  input: MintAndTransferInput,
): SignedTransactionResult;
/**
 * Register a domain and optionally perform follow-up mints in the same
 * transaction. Accepts either a single `mint` or an array of `mints`.
 */
export function buildRegisterDomainAndMintTransaction(
  input: RegisterDomainAndMintInput,
): SignedTransactionResult;
/**
 * Register an account and enqueue one or more asset transfers. Each transfer
 * must specify its source asset identifier; the helper enforces this at runtime.
 */
export function buildRegisterAccountAndTransferTransaction(
  input: RegisterAccountAndTransferInput,
): SignedTransactionResult;
/**
 * Register an asset definition and optionally mint initial supply. When both
 * `accountId` and `assetHoldingId` are provided the helper validates that they match
 * the canonical asset-holding id derived from `assetDefinitionId + accountId`.
 */
export function buildRegisterAssetDefinitionAndMintTransaction(
  input: RegisterAssetDefinitionAndMintInput,
): SignedTransactionResult;
/**
 * Register an asset definition, mint supply, and optionally fan-out transfers.
 * When a transfer omits `sourceAssetHoldingId` the helper reuses the first minted
 * destination identifier.
 */
export function buildRegisterAssetDefinitionMintAndTransferTransaction(
  input: RegisterAssetDefinitionMintAndTransferInput,
): SignedTransactionResult;

export interface TimeTriggerActionOptions {
  authority: string;
  instructions: ReadonlyArray<object | string>;
  startTimestampMs: number | bigint;
  periodMs?: number | bigint | null;
  repeats?: number | bigint | null;
  metadata?: Record<string, unknown> | string | null;
}

export interface CommitTriggerActionOptions {
  authority: string;
  instructions: ReadonlyArray<object | string>;
  repeats?: number | bigint | null;
  metadata?: Record<string, unknown> | string | null;
}

export function buildTimeTriggerAction(
  options: TimeTriggerActionOptions,
): string;
export function buildPrecommitTriggerAction(
  options: CommitTriggerActionOptions,
): string;

export function buildCreateKaigiTransaction(
  input: CreateKaigiTransactionInput,
): SignedTransactionResult;
export function buildPrivateKaigiFeeSpend(
  input: PrivateKaigiFeeSpendInput,
): PrivateKaigiFeeSpendResult;
export function buildConfidentialTransferProofV2(input: {
  chainId: string;
  assetDefinitionId: string;
  spendKey: BinaryLike;
  treeCommitments: ReadonlyArray<BinaryLike>;
  inputs: ReadonlyArray<ConfidentialTransferProofInputV2>;
  outputs: ReadonlyArray<ConfidentialTransferProofOutputV2>;
  rootHintHex: string;
  verifyingKey: Record<string, unknown>;
}): ConfidentialTransferProofResultV2;
export function buildConfidentialUnshieldProofV2(input: {
  chainId: string;
  assetDefinitionId: string;
  spendKey: BinaryLike;
  treeCommitments: ReadonlyArray<BinaryLike>;
  inputs: ReadonlyArray<ConfidentialTransferProofInputV2>;
  publicAmount: NumericLike;
  rootHintHex: string;
  verifyingKey: Record<string, unknown>;
}): ConfidentialUnshieldProofResultV2;
export function buildConfidentialUnshieldProofV3(input: {
  chainId: string;
  assetDefinitionId: string;
  spendKey: BinaryLike;
  treeCommitments: ReadonlyArray<BinaryLike>;
  inputs: ReadonlyArray<ConfidentialTransferProofInputV2>;
  outputs?: ReadonlyArray<ConfidentialUnshieldProofOutputV3>;
  publicAmount: NumericLike;
  rootHintHex: string;
  verifyingKey: Record<string, unknown>;
}): ConfidentialUnshieldProofResultV3;
export function buildPrivateCreateKaigiTransaction(
  input: PrivateCreateKaigiTransactionInput,
): PrivateKaigiEntrypointResult;
export function buildJoinKaigiTransaction(
  input: JoinKaigiTransactionInput,
): SignedTransactionResult;
export function buildPrivateJoinKaigiTransaction(
  input: PrivateJoinKaigiTransactionInput,
): PrivateKaigiEntrypointResult;
export function buildLeaveKaigiTransaction(
  input: LeaveKaigiTransactionInput,
): SignedTransactionResult;
export function buildEndKaigiTransaction(
  input: EndKaigiTransactionInput,
): SignedTransactionResult;
export function buildPrivateEndKaigiTransaction(
  input: PrivateEndKaigiTransactionInput,
): PrivateKaigiEntrypointResult;
export function buildRecordKaigiUsageTransaction(
  input: RecordKaigiUsageTransactionInput,
): SignedTransactionResult;
export function buildSetKaigiRelayManifestTransaction(
  input: SetKaigiRelayManifestTransactionInput,
): SignedTransactionResult;
export function buildRegisterKaigiRelayTransaction(
  input: RegisterKaigiRelayTransactionInput,
): SignedTransactionResult;
export function buildRegisterSmartContractCodeTransaction(
  input: RegisterSmartContractCodeTransactionInput,
): SignedTransactionResult;
export function buildRegisterSmartContractBytesTransaction(
  input: RegisterSmartContractBytesTransactionInput,
): SignedTransactionResult;
export function buildRemoveSmartContractBytesTransaction(
  input: RemoveSmartContractBytesTransactionInput,
): SignedTransactionResult;
export function buildProposeDeployContractTransaction(
  input: ProposeDeployContractTransactionInput,
): SignedTransactionResult;
export function buildCastZkBallotTransaction(
  input: CastZkBallotTransactionInput,
): SignedTransactionResult;
export function buildCastPlainBallotTransaction(
  input: CastPlainBallotTransactionInput,
): SignedTransactionResult;
export function buildEnactReferendumTransaction(
  input: EnactReferendumTransactionInput,
): SignedTransactionResult;
export function buildFinalizeReferendumTransaction(
  input: FinalizeReferendumTransactionInput,
): SignedTransactionResult;
export function buildPersistCouncilForEpochTransaction(
  input: PersistCouncilForEpochTransactionInput,
): SignedTransactionResult;
export function buildRegisterZkAssetTransaction(
  input: RegisterZkAssetTransactionInput,
): SignedTransactionResult;
export function buildScheduleConfidentialPolicyTransitionTransaction(
  input: ScheduleConfidentialPolicyTransitionTransactionInput,
): SignedTransactionResult;
export function buildCancelConfidentialPolicyTransitionTransaction(
  input: CancelConfidentialPolicyTransitionTransactionInput,
): SignedTransactionResult;
export function buildShieldTransaction(
  input: ShieldTransactionInput,
): SignedTransactionResult;
export function buildZkTransferTransaction(
  input: ZkTransferTransactionInput,
): SignedTransactionResult;
export function buildUnshieldTransaction(
  input: UnshieldTransactionInput,
): SignedTransactionResult;
export function buildCreateElectionTransaction(
  input: CreateElectionTransactionInput,
): SignedTransactionResult;
export function buildSubmitBallotTransaction(
  input: SubmitBallotTransactionInput,
): SignedTransactionResult;
export function buildFinalizeElectionTransaction(
  input: FinalizeElectionTransactionInput,
): SignedTransactionResult;

export function submitSignedTransaction(
  client: ToriiClient,
  signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
  options?: {
    waitForCommit?: boolean;
    pollIntervalMs?: number;
    timeoutMs?: number;
    privateKey?: ArrayBufferView | ArrayBuffer | Buffer;
  },
): Promise<{ hash: string; submission: unknown; status?: unknown }>;

export function submitTransactionEntrypoint(
  client: ToriiClient,
  transactionEntrypoint: ArrayBufferView | ArrayBuffer | Buffer,
  options: {
    hashHex: string;
    waitForCommit?: boolean;
    pollIntervalMs?: number;
    timeoutMs?: number;
  },
): Promise<{ hash: string; submission: unknown; status?: unknown }>;

/**
 * Build a `Mint::Asset` instruction payload with deterministic quantity
 * normalisation.
 */
export function buildMintAssetInstruction({
  assetId,
  quantity,
}: {
  assetId: string;
  quantity: NumericLike;
}): object;

/**
 * Build a `Burn::Asset` instruction payload mirroring runtime validation in
 * {@link buildBurnAssetTransaction}.
 */
export function buildBurnAssetInstruction({
  assetId,
  quantity,
}: {
  assetId: string;
  quantity: NumericLike;
}): object;

/**
 * Build a `Mint::TriggerRepetitions` instruction payload. Repetitions must be
 * a positive integer; fractional values throw at runtime.
 */
export function buildMintTriggerRepetitionsInstruction({
  triggerId,
  repetitions,
}: {
  triggerId: string;
  repetitions: NumericLike;
}): object;

/**
 * Build a `Burn::TriggerRepetitions` instruction payload mirroring runtime
 * validation in {@link buildBurnTriggerTransaction}. Repetitions must be a
 * positive integer.
 */
export function buildBurnTriggerRepetitionsInstruction({
  triggerId,
  repetitions,
}: {
  triggerId: string;
  repetitions: NumericLike;
}): object;

export function buildRegisterDomainInstruction({
  domainId,
  logo,
  metadata,
}: {
  domainId: string;
  logo?: string | null;
  metadata?: object | null;
}): object;

export function buildRegisterAccountInstruction({
  accountId,
  metadata,
}: {
  accountId: string;
  metadata?: object | null;
}): object;

export function buildRegisterAssetDefinitionInstruction(options: {
  assetDefinitionId?: string;
  asset_definition_id?: string;
  id?: string;
  name?: string;
  description?: string | null;
  alias?: string | null;
  logo?: string | null;
  scale?: NumericLike | null;
  mintable?: string;
  mintOnce?: boolean;
  metadata?: object | null;
  balanceScopePolicy?: string;
  balance_scope_policy?: string;
  confidentialPolicy?: object;
  confidential_policy?: object;
}): object;

export function buildGrantAccountPermissionInstruction(options: {
  accountId?: string;
  destinationAccountId?: string;
  destination?: string;
  permission?: {
    name: string;
    payload?: JsonValue;
  };
  name?: string;
  payload?: JsonValue;
}): object;

export function buildSetAssetDefinitionAliasInstruction(options: {
  assetDefinitionId?: string;
  asset_definition_id?: string;
  alias?: string | null;
  leaseExpiryMs?: NumericLike | null;
  lease_expiry_ms?: NumericLike | null;
}): object;

export function buildExecuteTriggerInstruction(
  trigger: string,
  args?: JsonValue,
): ExecuteTriggerInstructionPayload;
export function buildExecuteTriggerInstruction(options: {
  trigger: string;
  args?: JsonValue;
}): ExecuteTriggerInstructionPayload;
export function buildExecuteTriggerNorito(
  trigger: string,
  args?: JsonValue,
): Buffer;
export function buildExecuteTriggerNorito(options: {
  trigger: string;
  args?: JsonValue;
}): Buffer;

export function buildMultisigTriggerArgs(
  preset: "lifecycle",
  input: MultisigLifecycleTriggerArgsInput,
): {
  action: string;
  request_id: string;
  fi_id?: string;
  to_account_id?: string;
  amount_i64?: number;
  requested_by_actor_id?: JsonValue;
  created_at_ms?: number;
  expires_at_ms?: number;
};
export function buildMultisigTriggerArgs(
  preset: "lookup",
  input: MultisigLookupTriggerArgsInput,
): {
  request_id: string;
  requested_by_actor_id?: JsonValue;
};

export function isMultisigSignerAuthorized(
  spec: MultisigSpecLike,
  signerAccountId: string,
): boolean;

export function buildMultisigExecuteTriggerInstruction(
  options: MultisigExecuteTriggerOptions,
): ExecuteTriggerInstructionPayload;
export function buildMultisigExecuteTriggerNorito(
  options: MultisigExecuteTriggerOptions,
): Buffer;

/**
 * Build a multisig registration instruction payload.
 */
export function buildRegisterMultisigInstruction({
  accountId,
  spec,
}: {
  accountId: string;
  spec: MultisigSpecLike;
}): object;

/**
 * Build a multisig proposal payload while enforcing the policy TTL cap.
 */
export function buildProposeMultisigInstruction({
  accountId,
  instructions,
  spec,
  transactionTtlMs,
}: {
  accountId: string;
  instructions: ReadonlyArray<object>;
  spec: MultisigSpecLike;
  transactionTtlMs?: number | null;
}): object;

export function buildProposeMultisigExecuteTriggerInstruction(
  options: ProposeMultisigExecuteTriggerOptions,
): object;
export function buildProposeMultisigExecuteTriggerNorito(
  options: ProposeMultisigExecuteTriggerOptions,
): Buffer;

export function buildMultisigProposeRequest(
  options: MultisigProposeRequest,
): MultisigProposePayload;

export function buildMultisigContractCallProposeRequest(
  options: MultisigContractCallProposeRequest & {
    trigger: string;
    args?: JsonValue;
    argPreset?: MultisigTriggerArgsPreset;
    preset?: MultisigTriggerArgsPreset;
    argInput?:
      | MultisigLifecycleTriggerArgsInput
      | MultisigLookupTriggerArgsInput;
    presetInput?:
      | MultisigLifecycleTriggerArgsInput
      | MultisigLookupTriggerArgsInput;
    multisigSpec?: MultisigSpecLike;
    spec?: MultisigSpecLike;
    strictSignerCheck?: boolean;
  },
): MultisigContractCallProposePayload;

export function buildMultisigContractCallApproveRequest(
  options: MultisigContractCallApproveRequest,
): MultisigContractCallApprovePayload;

export function buildTransferAssetInstruction({
  sourceAssetHoldingId,
  quantity,
  destinationAccountId,
}: {
  sourceAssetHoldingId: string;
  quantity: NumericLike;
  destinationAccountId: string;
}): object;

export function buildTransferDomainInstruction({
  sourceAccountId,
  domainId,
  destinationAccountId,
}: {
  sourceAccountId: string;
  domainId: string;
  destinationAccountId: string;
}): object;

export function buildTransferAssetDefinitionInstruction({
  sourceAccountId,
  assetDefinitionId,
  destinationAccountId,
}: {
  sourceAccountId: string;
  assetDefinitionId: string;
  destinationAccountId: string;
}): object;

export function buildTransferNftInstruction({
  sourceAccountId,
  nftId,
  destinationAccountId,
}: {
  sourceAccountId: string;
  nftId: string;
  destinationAccountId: string;
}): object;

export function buildRegisterRwaInstruction(
  options:
    | {
        rwa?: RegisterRwaPayloadInput | string;
        rwaJson?: RegisterRwaPayloadInput | string;
      }
    | RegisterRwaPayloadInput,
): object;

export function buildTransferRwaInstruction({
  sourceAccountId,
  rwaId,
  quantity,
  destinationAccountId,
}: {
  sourceAccountId: string;
  rwaId: string;
  quantity: NumericLike;
  destinationAccountId: string;
}): object;

export function buildMergeRwasInstruction(
  options:
    | {
        merge?: MergeRwasPayloadInput | string;
        mergeJson?: MergeRwasPayloadInput | string;
      }
    | MergeRwasPayloadInput,
): object;

export function buildRedeemRwaInstruction({
  rwaId,
  quantity,
}: {
  rwaId: string;
  quantity: NumericLike;
}): object;

export function buildFreezeRwaInstruction({ rwaId }: { rwaId: string }): object;

export function buildUnfreezeRwaInstruction({
  rwaId,
}: {
  rwaId: string;
}): object;

export function buildHoldRwaInstruction({
  rwaId,
  quantity,
}: {
  rwaId: string;
  quantity: NumericLike;
}): object;

export function buildReleaseRwaInstruction({
  rwaId,
  quantity,
}: {
  rwaId: string;
  quantity: NumericLike;
}): object;

export function buildForceTransferRwaInstruction({
  rwaId,
  quantity,
  destinationAccountId,
}: {
  rwaId: string;
  quantity: NumericLike;
  destinationAccountId: string;
}): object;

export function buildSetRwaControlsInstruction(options: {
  rwaId: string;
  controls?: RwaControlPolicyInput | string;
  controlsJson?: RwaControlPolicyInput | string;
}): object;

export function buildSetRwaKeyValueInstruction({
  rwaId,
  key,
  value,
}: {
  rwaId: string;
  key: string;
  value: JsonValue;
}): object;

export function buildRemoveRwaKeyValueInstruction({
  rwaId,
  key,
}: {
  rwaId: string;
  key: string;
}): object;

export function buildCreateKaigiInstruction(call: CreateKaigiInput): object;

export function buildJoinKaigiInstruction(join: JoinKaigiInput): object;

export function buildLeaveKaigiInstruction(leave: LeaveKaigiInput): object;

export function buildEndKaigiInstruction(end: EndKaigiInput): object;

export function buildRecordKaigiUsageInstruction(
  usage: RecordKaigiUsageInput,
): object;

export function buildSetKaigiRelayManifestInstruction(
  manifest: SetKaigiRelayManifestInput,
): object;

export function buildRegisterKaigiRelayInstruction(
  relay: RegisterKaigiRelayInput,
): object;

export function buildProposeDeployContractInstruction(
  input: ProposeDeployContractInstructionInput,
): object;

export function buildCastZkBallotInstruction(
  input: CastZkBallotInstructionInput,
): object;

export function buildCastPlainBallotInstruction(
  input: CastPlainBallotInstructionInput,
): object;

export function buildEnactReferendumInstruction(
  input: EnactReferendumInstructionInput,
): object;

export function buildFinalizeReferendumInstruction(
  input: FinalizeReferendumInstructionInput,
): object;

export function buildPersistCouncilForEpochInstruction(
  input: PersistCouncilForEpochInstructionInput,
): object;

export function buildSubmitAgendaProposalInstruction(input: {
  proposal: Record<string, unknown>;
}): object;

export interface ClaimTwitterFollowRewardInstructionInput {
  bindingHash:
    | {
        pepper_id?: string;
        pepperId?: string;
        pepper?: string;
        digest?: string | ArrayBufferView | ArrayBuffer | Buffer;
        hash?: string | ArrayBufferView | ArrayBuffer | Buffer;
        value?: string | ArrayBufferView | ArrayBuffer | Buffer;
      }
    | Record<string, unknown>;
}

export interface SendToTwitterInstructionInput {
  bindingHash:
    | {
        pepper_id?: string;
        pepperId?: string;
        pepper?: string;
        digest?: string | ArrayBufferView | ArrayBuffer | Buffer;
        hash?: string | ArrayBufferView | ArrayBuffer | Buffer;
        value?: string | ArrayBufferView | ArrayBuffer | Buffer;
      }
    | Record<string, unknown>;
  amount: string | number | bigint;
}

export interface CancelTwitterEscrowInstructionInput {
  bindingHash:
    | {
        pepper_id?: string;
        pepperId?: string;
        pepper?: string;
        digest?: string | ArrayBufferView | ArrayBuffer | Buffer;
        hash?: string | ArrayBufferView | ArrayBuffer | Buffer;
        value?: string | ArrayBufferView | ArrayBuffer | Buffer;
      }
    | Record<string, unknown>;
}

export function buildClaimTwitterFollowRewardInstruction(
  input: ClaimTwitterFollowRewardInstructionInput,
): object;

export function buildSendToTwitterInstruction(
  input: SendToTwitterInstructionInput,
): object;

export function buildCancelTwitterEscrowInstruction(
  input: CancelTwitterEscrowInstructionInput,
): object;

export function buildZkAtPolicyCommitment(
  input: ZkAtPolicyCommitmentInput,
): ZkAtPolicyCommitment;

export function buildZkAtAuthenticatorEnvelope(
  input: ZkAtAuthenticatorEnvelopeInput,
): Buffer;

export function buildZkAtDevProofFixture(
  input: Omit<
    ZkAtAuthenticatorEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): ZkAtDevProofFixture;

export function verifyZkAtAuthenticatorLocally(
  input: ZkAtAuthenticatorLocalVerificationInput | BinaryLike,
): ZkAtAuthenticatorLocalVerificationResult;

export function buildZkAmsAdmissionBatch(
  input: ZkAmsAdmissionBatchInput,
): ZkAmsAdmissionBatch;

export function buildZkAmsAdmissionProofEnvelope(
  input: ZkAmsAdmissionProofEnvelopeInput,
): Buffer;

export function buildZkAmsAdmissionDevProofFixture(
  input: Omit<
    ZkAmsAdmissionProofEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): ZkAmsAdmissionDevProofFixture;

export function verifyZkAmsAdmissionProofLocally(
  input: ZkAmsAdmissionLocalVerificationInput | BinaryLike,
): ZkAmsAdmissionLocalVerificationResult;

export function buildVegaCredentialPredicateCommitment(
  input: VegaCredentialPredicateCommitmentInput,
): VegaCredentialPredicateCommitment;

export function buildVegaCredentialProofEnvelope(
  input: VegaCredentialProofEnvelopeInput,
): Buffer;

export function buildVegaCredentialDevProofFixture(
  input: Omit<
    VegaCredentialProofEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): VegaCredentialDevProofFixture;

export function verifyVegaCredentialProofLocally(
  input: VegaCredentialLocalVerificationInput | BinaryLike,
): VegaCredentialLocalVerificationResult;

export function buildSilentThresholdCredentialCommitments(
  input: SilentThresholdCredentialCommitmentsInput,
): SilentThresholdCredentialCommitments;

export function buildSilentThresholdCredentialEnvelope(
  input: SilentThresholdCredentialEnvelopeInput,
): Buffer;

export function buildSilentThresholdCredentialDevProofFixture(
  input: Omit<
    SilentThresholdCredentialEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): SilentThresholdCredentialDevProofFixture;

export function verifySilentThresholdCredentialProofLocally(
  input: SilentThresholdCredentialLocalVerificationInput | BinaryLike,
): SilentThresholdCredentialLocalVerificationResult;

export function buildZkX509IdentityCommitments(
  input: ZkX509IdentityCommitmentsInput,
): ZkX509IdentityCommitments;

export function buildZkX509IdentityEnvelope(
  input: ZkX509IdentityEnvelopeInput,
): Buffer;

export function buildZkX509IdentityDevProofFixture(
  input: Omit<
    ZkX509IdentityEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): ZkX509IdentityDevProofFixture;

export function verifyZkX509IdentityProofLocally(
  input: ZkX509IdentityLocalVerificationInput | BinaryLike,
): ZkX509IdentityLocalVerificationResult;

export function buildJindoLatticePublicInputs(
  input: JindoLatticePublicInputsInput,
): JindoLatticePublicInputsDescriptor;

export function buildJindoLatticeProofEnvelope(
  input: JindoLatticeProofEnvelopeInput,
): Buffer;

export function buildJindoLatticeDevProofFixture(
  input: Omit<
    JindoLatticeProofEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): JindoLatticeDevProofFixture;

export function verifyJindoLatticeProofLocally(
  input: JindoLatticeLocalVerificationInput | BinaryLike,
): JindoLatticeLocalVerificationResult;

export function buildSisHintsCredentialCommitments(
  input: SisHintsCredentialCommitmentsInput,
): SisHintsCredentialCommitments;

export function buildSisHintsCredentialEnvelope(
  input: SisHintsCredentialEnvelopeInput,
): Buffer;

export function buildSisHintsCredentialDevProofFixture(
  input: Omit<
    SisHintsCredentialEnvelopeInput,
    "proof" | "proofBytes" | "proof_bytes"
  >,
): SisHintsCredentialDevProofFixture;

export function verifySisHintsCredentialProofLocally(
  input: SisHintsCredentialLocalVerificationInput | BinaryLike,
): SisHintsCredentialLocalVerificationResult;

export function buildAnonymousPgcReceiverSet(
  input: AnonymousPgcReceiverSetInput,
): AnonymousPgcReceiverSet;

export function buildAnonymousPgcDevProofFixture(
  input: AnonymousPgcDevProofFixtureInput,
): AnonymousPgcDevProofFixture;

export function verifyAnonymousPgcDevProofLocally(
  input: AnonymousPgcDevProofLocalVerificationInput | BinaryLike,
): AnonymousPgcDevProofLocalVerificationResult;

export function buildRangeCommitment(
  input: RangeCommitmentInput,
): RangeCommitmentDescriptor;

export function buildVeRangeDevProofFixture(
  input: VeRangeDevProofFixtureInput,
): VeRangeDevProofFixture;

export function buildVeRangeProofEnvelope(
  input: VeRangeProofEnvelopeInput,
): Buffer;

export function verifyVeRangeProofLocally(
  input: VeRangeLocalVerificationInput | BinaryLike,
): VeRangeLocalVerificationResult;

export function buildPrivacyProofEnvelope(
  input: PrivacyProofEnvelopeInput,
): Buffer;

export function buildRegisterPrivacyVerifierKeyInstruction(
  input: RegisterPrivacyVerifierKeyInstructionInput,
): object;

export function buildRetirePrivacyVerifierKeyInstruction(
  input: RetirePrivacyVerifierKeyInstructionInput,
): object;

export function buildRegisterZkAssetInstruction(
  input: RegisterZkAssetInstructionInput,
): object;

export function buildRegisterZkAceIdentityCommitmentInstruction(
  input: RegisterZkAceIdentityCommitmentInstructionInput,
): object;

export function buildRotateZkAceIdentityCommitmentInstruction(
  input: RotateZkAceIdentityCommitmentInstructionInput,
): object;

export function buildRevokeZkAceIdentityCommitmentInstruction(
  input: RevokeZkAceIdentityCommitmentInstructionInput,
): object;

export function buildZkAceAuthorizationProofV1(
  input: ZkAceAuthorizationProofV1Input,
): ZkAceAuthorizationProofV1;

export function buildZkAceAuthorizedTransferInstruction(
  input: ZkAceAuthorizedTransferInstructionInput,
): object;

export function buildScheduleConfidentialPolicyTransitionInstruction(
  input: ScheduleConfidentialPolicyTransitionInstructionInput,
): object;

export function buildCancelConfidentialPolicyTransitionInstruction(
  input: CancelConfidentialPolicyTransitionInstructionInput,
): object;

export function buildShieldInstruction(input: ShieldInstructionInput): object;

export function buildZkTransferInstruction(
  input: ZkTransferInstructionInput,
): object;

/**
 * Build the asset-hidden transfer payload shape.
 *
 * The Rust data model and Norito encoder accept this instruction. Validator
 * execution remains fail-closed until asset-pool verifier state exists.
 */
export function buildAssetHiddenZkTransferInstruction(
  input: AssetHiddenZkTransferInstructionInput,
): object;

export function buildRegisterAssetHiddenZkPoolInstruction(
  input: RegisterAssetHiddenZkPoolInstructionInput,
): object;

export function buildUnshieldInstruction(
  input: UnshieldInstructionInput,
): object;

export function buildCreateElectionInstruction(
  input: CreateElectionInstructionInput,
): object;

export function buildSubmitBallotInstruction(
  input: SubmitBallotInstructionInput,
): object;

export function buildFinalizeElectionInstruction(
  input: FinalizeElectionInstructionInput,
): object;

export function buildRegisterSmartContractCodeInstruction(
  input: RegisterSmartContractCodeInstructionInput,
): object;

export function buildRegisterSmartContractBytesInstruction(
  input: RegisterSmartContractBytesInstructionInput,
): object;

export function buildRemoveSmartContractBytesInstruction(
  input: RemoveSmartContractBytesInstructionInput,
): object;

export function encodeInstruction(instruction: object): Buffer;

export const DEFAULT_TORII_CLIENT_CONFIG: {
  timeoutMs: number;
  maxRetries: number;
  backoffInitialMs: number;
  backoffMultiplier: number;
  maxBackoffMs: number;
  retryStatuses: ReadonlyArray<number>;
  retryMethods: ReadonlyArray<string>;
  defaultHeaders: Readonly<Record<string, string>>;
  authToken: string | null;
  apiToken: string | null;
  retryTelemetryHook: ((event: ToriiRetryTelemetryEvent) => void) | null;
  insecureTransportTelemetryHook:
    | ((event: InsecureTransportTelemetryEvent) => void)
    | null;
};

export const DEFAULT_RETRY_PROFILE_PIPELINE: ToriiRetryProfileOptions;

export const DEFAULT_RETRY_PROFILE_STREAMING: ToriiRetryProfileOptions;

export function resolveToriiClientConfig(input?: {
  config?: ToriiClientConfigSource;
  env?: Record<string, string | undefined>;
  overrides?: ToriiClientConfigSource;
}): ResolvedToriiClientConfig;

export function extractToriiFeatureConfig(
  input?: {
    config?: Record<string, unknown>;
  } & Record<string, unknown>,
): ToriiFeatureConfigSnapshot;

export type SoracloudStorageClass = "hot" | "warm" | "cold";

export interface SoracloudHfDeployDraftInput {
  repoId: string;
  revision?: string;
  modelName: string;
  serviceName: string;
  apartmentName?: string;
  storageClass: SoracloudStorageClass;
  leaseTermMs: number | bigint | string;
  leaseAssetDefinitionId: string;
  baseFeeNanos: number | bigint | string;
}

export interface SoracloudManifestProvenance {
  signer: string;
  signature: string;
}

export interface SoracloudHfDeployDraft {
  payload: {
    repo_id: string;
    revision?: string;
    model_name: string;
    service_name: string;
    apartment_name?: string;
    storage_class: SoracloudStorageClass | { type: string; value?: unknown };
    lease_term_ms: number;
    lease_asset_definition_id: string;
    base_fee_nanos: string;
  };
  provenancePayloads: {
    deploy: Record<string, unknown>;
    generatedService: Record<string, unknown>;
    generatedApartment?: Record<string, unknown>;
  };
}

export interface SoracloudHfDeployRequest {
  payload: SoracloudHfDeployDraft["payload"];
  provenance: SoracloudManifestProvenance;
  generated_service_provenance: SoracloudManifestProvenance;
  generated_apartment_provenance?: SoracloudManifestProvenance;
}

export interface SoracloudAppInfraRouteInput {
  path: string;
  publicHost?: string;
  public_host?: string;
  internalUrl?: string;
  internal_url?: string;
}

export interface SoracloudAppInfraLeaseVolumeInput {
  name: string;
  mountPath: string;
  maxTotalBytes: NumericLike;
  temperature?: "hot" | "warm" | "cold";
}

export interface SoracloudAppInfraShardInput {
  count: NumericLike;
  shardIdEnv?: string;
  shardCountEnv?: string;
}

export interface SoracloudAppInfraServiceInput {
  name: string;
  version?: string;
  serviceVersion?: string;
  service_version?: string;
  serviceManifestHash?: string;
  service_manifest_hash?: string;
  containerManifestHash?: string;
  container_manifest_hash?: string;
  runtime?: "Inrou" | "Ivm";
  executionPlane?: "HttpService" | "DeterministicService" | "Ivm";
  execution_plane?: "HttpService" | "DeterministicService" | "Ivm";
  routes?: ReadonlyArray<SoracloudAppInfraRouteInput>;
  leaseVolumes?: ReadonlyArray<SoracloudAppInfraLeaseVolumeInput>;
  lease_volumes?: ReadonlyArray<SoracloudAppInfraLeaseVolumeInput>;
  shards?: SoracloudAppInfraShardInput;
}

export interface SoracloudAppInfraStaticSiteInput {
  publicUrl: string;
  contentCid?: string;
  content_cid?: string;
  manifestDigestHex?: string;
  manifest_digest_hex?: string;
  mountPath?: string;
  apiBasePath?: string;
  api_base_path?: string;
}

export interface SoracloudAppInfraDraftInput {
  appName: string;
  appVersion?: string;
  publicUrl: string;
  staticSite?: SoracloudAppInfraStaticSiteInput;
  static_site?: SoracloudAppInfraStaticSiteInput;
  services: ReadonlyArray<SoracloudAppInfraServiceInput>;
}

export interface SoracloudAppInfraDraft {
  payload: Record<string, unknown> & {
    schema_version: 1;
    app_name: string;
    app_version: string;
    public_url: string;
    static_site?: Record<string, unknown>;
    services: Array<Record<string, unknown>>;
  };
  provenancePayloads: {
    deploy: Record<string, unknown>;
    services: Array<Record<string, unknown>>;
  };
}

export interface SoracloudAppReportPhaseV1 {
  name:
    | "build"
    | "sync_manifests"
    | "doctor"
    | "publish"
    | "sign"
    | "submit"
    | "status"
    | "verify";
  ok: boolean;
  skipped: boolean;
  diagnostics: string[];
}

export interface SoracloudAppReportServiceV1 {
  service_name: string;
  execution_plane: string;
  runtime: string;
}

export interface SoracloudAppReportV1 {
  schema_version: "soracloud.app.report.v1";
  app_name: string;
  manifest_path: string;
  ok: boolean;
  phases: SoracloudAppReportPhaseV1[];
  app_infra_manifest_hash?: string;
  routes: Array<Record<string, unknown>>;
  services: SoracloudAppReportServiceV1[];
  static_site?: Record<string, unknown>;
  blockers: string[];
  next_action: string;
}

export function buildSoracloudHfDeployDraft(
  input: SoracloudHfDeployDraftInput,
): SoracloudHfDeployDraft;

export function buildSoracloudAppInfraDraft(
  input: SoracloudAppInfraDraftInput,
): SoracloudAppInfraDraft;

export interface SoracloudAppInfraRequest {
  deploy_services: unknown[];
  upgrade_services: unknown[];
  manifest: SoracloudAppInfraDraft["payload"];
  provenance: SoracloudManifestProvenance;
}

export function assembleSoracloudAppInfraRequest(
  draft: SoracloudAppInfraDraft,
  provenances: { deploy: SoracloudManifestProvenance },
  options?: { deployServices?: unknown[]; upgradeServices?: unknown[] },
): SoracloudAppInfraRequest;

export function deploySoracloudAppInfraInstruction(
  manifest: Record<string, unknown>,
  provenance: SoracloudManifestProvenance,
): { wire_id: string; payload: Record<string, unknown> };

export function upgradeSoracloudAppInfraInstruction(
  manifest: Record<string, unknown>,
  provenance: SoracloudManifestProvenance,
): { wire_id: string; payload: Record<string, unknown> };

export function assembleSoracloudHfDeployRequest(
  draft: SoracloudHfDeployDraft,
  provenances: {
    deploy: SoracloudManifestProvenance;
    generatedService: SoracloudManifestProvenance;
    generatedApartment?: SoracloudManifestProvenance;
  },
): SoracloudHfDeployRequest;

export interface SoracloudPrivateArtifactRefInput {
  schemaVersion?: number | bigint | string;
  schema_version?: number | bigint | string;
  sorafsManifestDigest?: string;
  sorafs_manifest_digest?: string;
  artifactHash?: string;
  artifact_hash?: string;
  ciphertextBytes?: number | bigint | string;
  ciphertext_bytes?: number | bigint | string;
  artifactRole?: "input" | "output";
  artifact_role?: "input" | "output";
}

export interface SoracloudPrivateQuantizedCpuModelInput {
  inputLen?: number | bigint | string;
  input_len?: number | bigint | string;
  outputLen?: number | bigint | string;
  output_len?: number | bigint | string;
  weightsI8?: number[];
  weights_i8?: number[];
  biasI32?: number[];
  bias_i32?: number[];
  outputShift?: number | bigint | string;
  output_shift?: number | bigint | string;
  outputMin?: number;
  output_min?: number;
  outputMax?: number;
  output_max?: number;
}

export interface SoracloudPrivateUploadedModelExecuteInput {
  serviceName: string;
  weightVersion: string;
  modelId?: string;
  modelName?: string;
  bundleRoot?: string;
  policyId: string;
  model: SoracloudPrivateQuantizedCpuModelInput;
  plaintextInputI32: number[];
  inputArtifact: SoracloudPrivateArtifactRefInput;
  outputArtifact: SoracloudPrivateArtifactRefInput;
  emittedSequence: number | bigint | string;
}

export interface SoracloudPrivateUploadedModelReceiptQueryInput {
  receiptId?: string;
  serviceName?: string;
  modelId?: string;
  weightVersion?: string;
  limit?: number | bigint | string;
  countMode?: "bounded" | "exact";
}

export interface SoracloudTxInstruction {
  wire_id: string;
  payload_hex: string;
}

export function buildSoracloudPrivateUploadedModelExecuteRequest(
  input: SoracloudPrivateUploadedModelExecuteInput,
): Record<string, unknown>;

export function buildSoracloudPrivateUploadedModelReceiptQuery(
  input?: SoracloudPrivateUploadedModelReceiptQueryInput,
): Record<string, string>;

export function privateUploadedModelReceiptInstruction(
  response: Record<string, unknown>,
): SoracloudTxInstruction;

export * from "./nexus-app";
