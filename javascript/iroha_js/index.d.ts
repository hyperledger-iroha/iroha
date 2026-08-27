import type { Buffer } from "buffer";
import type { BrowserFeePayment } from "./transaction-codec.js";
import { OperatorSigningContext } from "./operator-request.js";
import type { RepoAgreementLifecycleFields } from "./repo-agreement.js";
import type { ToriiBlockMerkleCommitment, ToriiBlockMerkleProof, ToriiBlockProofs, ToriiBlockProofTrustedAnchor, ToriiBlockProofVerification } from "./src/blockProofTypes.js";
import type { BufferEncoding } from "./src/nodeBufferTypes.js";
import type { ToriiBrowserExplorerAccountsOptions, ToriiBrowserExplorerAssetDefinition, ToriiBrowserExplorerAssetDefinitionsOptions, ToriiBrowserExplorerAssetsOptions, ToriiBrowserExplorerCursorPage, ToriiBrowserExplorerDomainsOptions, ToriiBrowserExplorerOwnedDomainOptions } from "./src/toriiBrowserExplorerTypes.js";
import type { SubscriptionActionResponse, SubscriptionAuthorityActionRequest, SubscriptionCancelActionRequest, SubscriptionChargeActionRequest, SubscriptionCreateRequest, SubscriptionCreateResponse, SubscriptionGetResponse, SubscriptionListItem, SubscriptionListResponse, SubscriptionPlanCreateRequest, SubscriptionPlanCreateResponse, SubscriptionPlanListItem, SubscriptionPlanListResponse, SubscriptionUsageDraft, SubscriptionUsageRequest } from "./src/subscriptionTypes.js";
import type { SorafsOrderbookSignedTransaction, SorafsOrderbookSubmissionReceipt, SorafsOrderbookTransactionSubmitOptions } from "./src/sorafsOrderbookSubmission.js";
import { NetworkId } from "./src/networkId.js";
export { NetworkId, OperatorSigningContext };
export * from "./kotodama-compiler.js";
export * from "./transaction-codec.js";
export * from "./smart-contract-deployment.js";
export * from "./bootle-lantern-issuance.js";
export * from "./src/blockProofTypes.js";
export * from "./src/toriiBrowserExplorerTypes.js";
export type * from "./src/subscriptionTypes.js";
export * from "./src/sorafsOrderbookSubmission.js";

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export const KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION: 22;
export const KAGEMUSHA_MANIFEST_VERSION: 4;
export const KAGEMUSHA_MAX_HOPS: 8;
export const KAGEMUSHA_CASH_HANDOFF_CAPABILITY: "cash_handoff_v1";
export const KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES: 524288;
export const KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES: 50331648;

export interface KagemushaNoritoRequestV4 {
  readonly version: 4;
  readonly operationId: string;
  readonly norito: Uint8Array;
}

export interface OfflineStatus {
  readonly cash_handoff_capability: "cash_handoff_v1";
  readonly required_bridge_abi_version: 22;
  readonly max_hops: 8;
  readonly ready: true;
}

export type KagemushaOperationKind = Readonly<{
  kind: "top_up" | "redeem";
  value: null;
}>;

export interface KagemushaOperationReference {
  readonly operation_id: string;
  readonly kind: KagemushaOperationKind;
  readonly state: Readonly<{ state: "pending"; value: null }>;
  readonly transaction_hash: string;
  readonly status_uri: string;
  readonly submitted_at_ms: number;
}

export type KagemushaOperationStatus =
  | Readonly<{
      state: "pending";
      value: Readonly<{
        operation_id: string;
        kind: KagemushaOperationKind;
        transaction_hash: string;
        submitted_at_ms: number;
      }>;
    }>
  | Readonly<{
      state: "applied";
      value: Readonly<{
        operation_id: string;
        result: Readonly<{
          kind: "top_up" | "redeem";
          result: Readonly<Record<string, JsonValue>>;
        }>;
      }>;
    }>
  | Readonly<{
      state: "rejected";
      value: Readonly<{
        operation_id: string;
        kind: KagemushaOperationKind;
        transaction_hash: string;
        error: Readonly<{
          code: string;
          message: string;
          details?: Readonly<Record<string, JsonValue>>;
        }>;
      }>;
    }>;

export function normalizeKagemushaOperationId(value: string, context?: string): string;
export function normalizeKagemushaTopUpRequestV4(
  value: KagemushaNoritoRequestV4,
  context?: string,
): KagemushaNoritoRequestV4;
export function normalizeKagemushaRedeemRequestV4(
  value: KagemushaNoritoRequestV4,
  context?: string,
): KagemushaNoritoRequestV4;
export function normalizeOfflineStatus(
  payload: Record<string, unknown>,
): OfflineStatus;
export function normalizeKagemushaOperationReference(
  payload: Record<string, unknown>,
  expected: {
    expectedOperationId: string;
    expectedKind: "top_up" | "redeem";
    location: string | null;
  },
): KagemushaOperationReference;
export function normalizeKagemushaOperationStatus(
  payload: Record<string, unknown>,
  expectedOperationId: string,
): KagemushaOperationStatus;

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

export type {
  PrivacyCapabilityRowV1,
  PrivacyCapabilitySnapshotV1,
  PrivacyCompiledProfileBindingsV1,
  PrivacyCompiledProfileResultV1,
  PrivacyConsensusLimitsV1,
  PrivacyConsensusPolicyTighteningV1,
  PrivacyConsensusPolicyV1,
  PrivacyEngineIdV1,
  PrivacyEngineTagV1,
  PrivacyFixed32BytesV1,
  PrivacyProofSystemIdV1,
  PrivacyProofSystemTagV1,
  PrivacyProtocolActivationRecordV1,
  PrivacyProtocolIdV1,
  PrivacyProtocolLifecycleV1,
  PrivacyProtocolLimitsV1,
  PrivacyProtocolTagV1,
  PrivacyTaggedUnitV1,
  PrivacyU64V1,
} from "./privacy-capabilities.js";
export interface CryptoKeyPair {
  algorithm: CryptoAlgorithm;
  publicKey: Buffer;
  privateKey: Buffer;
  distid?: string | null;
}

export type RecoveryPhraseWordCount = 12 | 24;

export interface RecoveryPhrase {
  readonly phrase: string;
  readonly words: readonly string[];
  readonly wordCount: RecoveryPhraseWordCount;
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
export const PRIVACY_REQUIRED_BRIDGE_ABI_VERSION: 22;

export interface SignedTransactionResult {
  /** Exact canonical VersionedSignedTransaction V1 bytes. */
  signedTransaction: Buffer;
  hash: Buffer;
}

/** Exact canonical VersionedSignedTransaction V1 submission bytes. */
export type VersionedSignedTransactionV1 =
  | Buffer
  | ArrayBuffer
  | ArrayBufferView;

export const AccountAddressErrorCode: {
  readonly UNSUPPORTED_ALGORITHM: "ERR_UNSUPPORTED_ALGORITHM";
  readonly KEY_PAYLOAD_TOO_LONG: "ERR_KEY_PAYLOAD_TOO_LONG";
  readonly INVALID_HEADER_VERSION: "ERR_INVALID_HEADER_VERSION";
  readonly INVALID_NORM_VERSION: "ERR_INVALID_NORM_VERSION";
  readonly INVALID_I105_DISCRIMINANT: "ERR_INVALID_I105_DISCRIMINANT";
  readonly INVALID_LENGTH: "ERR_INVALID_LENGTH";
  readonly CHECKSUM_MISMATCH: "ERR_CHECKSUM_MISMATCH";
  readonly UNEXPECTED_NETWORK_PREFIX: "ERR_UNEXPECTED_NETWORK_PREFIX";
  readonly UNKNOWN_ADDRESS_CLASS: "ERR_UNKNOWN_ADDRESS_CLASS";
  readonly UNEXPECTED_EXTENSION_FLAG: "ERR_UNEXPECTED_EXTENSION_FLAG";
  readonly UNKNOWN_CONTROLLER_TAG: "ERR_UNKNOWN_CONTROLLER_TAG";
  readonly INVALID_PUBLIC_KEY: "ERR_INVALID_PUBLIC_KEY";
  readonly UNKNOWN_CURVE: "ERR_UNKNOWN_CURVE";
  readonly UNEXPECTED_TRAILING_BYTES: "ERR_UNEXPECTED_TRAILING_BYTES";
  readonly I105_TOO_SHORT: "ERR_I105_TOO_SHORT";
  readonly INVALID_I105_CHAR: "ERR_INVALID_I105_CHAR";
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
  readonly INVALID_ASSET_DEFINITION_ID: "ERR_INVALID_ASSET_DEFINITION_ID";
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
  canonicalHex: string;
  i105: { value: string; chainDiscriminant: number };
  i105Warning: string;
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
  multisig_account_id?: string;
  multisig_account_alias?: string;
}

export type MultisigProposalStatus =
  | "COLLECTING_SIGNATURES"
  | "FINALIZED"
  | "CANCELED"
  | "EXPIRED";

export interface MultisigProposalsQueryRequest extends MultisigAccountSelector {
  status?: ReadonlyArray<MultisigProposalStatus>;
  cursor?: string | null;
  limit?: number | string | bigint | null;
}

export interface MultisigProposalsResolveRequest extends MultisigAccountSelector {
  proposalId?: string | null;
  instructionsHash?: string | null;
  proposal_id?: string | null;
  instructions_hash?: string | null;
}

export type MultisigProposeInstructionInput =
  | object
  | string
  | BinaryLike
  | number[];

export interface MultisigProposeRequest extends MultisigAccountSelector {
  signerAccountId: string;
  instructions: MultisigProposeInstructionInput[];
  feePayment: NoritoFeePaymentIntent;
  publicKeyHex?: string | null;
  signatureB64?: string | null;
  creationTimeMs?: number | string | bigint | null;
  validationFeePolicyVersion?: number | string | bigint | null;
  validationFeePolicyHash?: string | null;
  validationFeeInstructionIndex?: number | string | bigint | null;
  validationFeeTransferEntryIndex?: number | string | bigint | null;
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id?: string;
  fee_payment?: NoritoFeePaymentIntent;
  public_key_hex?: string | null;
  signature_b64?: string | null;
  creation_time_ms?: number | string | bigint | null;
}

export interface MultisigProposePayload {
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id: string;
  instructions: string[];
  fee_payment: NoritoFeePaymentIntent;
  public_key_hex?: string;
  signature_b64?: string;
  creation_time_ms?: number;
  validation_fee_policy_version?: string;
  validation_fee_policy_hash?: string;
  validation_fee_instruction_index?: string;
  validation_fee_transfer_entry_index?: string;
}

export interface MultisigContractCallProposeRequest
  extends MultisigAccountSelector {
  signerAccountId: string;
  contractAddress?: string;
  contractAlias?: string;
  entrypoint: string;
  payload?: JsonValue;
  feePayment: NoritoFeePaymentIntent;
  publicKeyHex?: string | null;
  signatureB64?: string | null;
  creationTimeMs?: number | string | bigint | null;
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id?: string;
  contract_address?: string;
  contract_alias?: string;
  fee_payment?: NoritoFeePaymentIntent;
  public_key_hex?: string | null;
  signature_b64?: string | null;
  creation_time_ms?: number | string | bigint | null;
}

export interface MultisigContractCallProposePayload {
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id: string;
  contract_address?: string;
  contract_alias?: string;
  entrypoint: string;
  payload: JsonValue;
  fee_payment: NoritoFeePaymentIntent;
  public_key_hex?: string;
  signature_b64?: string;
  creation_time_ms?: number;
}

export interface MultisigContractCallApproveRequest
  extends MultisigAccountSelector {
  signerAccountId: string;
  proposalId?: string | null;
  instructionsHash?: string | null;
  feePayment: NoritoFeePaymentIntent;
  publicKeyHex?: string | null;
  signatureB64?: string | null;
  creationTimeMs?: number | string | bigint | null;
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id?: string;
  proposal_id?: string | null;
  instructions_hash?: string | null;
  fee_payment?: NoritoFeePaymentIntent;
  public_key_hex?: string | null;
  signature_b64?: string | null;
  creation_time_ms?: number | string | bigint | null;
}

export interface MultisigContractCallApprovePayload {
  multisig_account_id?: string;
  multisig_account_alias?: string;
  signer_account_id: string;
  proposal_id?: string;
  instructions_hash?: string;
  fee_payment: NoritoFeePaymentIntent;
  public_key_hex?: string;
  signature_b64?: string;
  creation_time_ms?: number;
}

export interface MultisigContractCallResponse {
  ok: boolean;
  resolved_multisig_account_id: string;
  submitted: boolean;
  proposal_id: string | null;
  instructions_hash: string | null;
  tx_hash_hex: string | null;
  executed_tx_hash_hex: string | null;
  creation_time_ms: number | null;
  transaction_payload_b64: string | null;
  signing_message_b64: string | null;
}

export interface MultisigSpecResponse {
  resolved_multisig_account_id: string;
  spec: JsonValue;
}

export interface MultisigProposalEntry {
  proposal_id: string;
  instructions_hash: string;
  operation_type: string;
  intent: JsonValue | null;
  proposal: JsonValue;
  status: MultisigProposalStatus;
  terminal_at_ms: number | null;
}

export interface MultisigProposalsQueryResponse {
  resolved_multisig_account_id: string;
  proposals: ReadonlyArray<MultisigProposalEntry>;
  next_cursor: string | null;
}

export interface MultisigProposalResolveResponse extends MultisigProposalEntry {
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

export const SCCP_DOMAIN_SORA: 0;
export const SCCP_DOMAIN_ETH: 1;
export const SCCP_DOMAIN_BSC: 2;
export const SCCP_DOMAIN_SOLANA: 3;
export const SCCP_DOMAIN_TRON: 5;
export type SccpDomain = 0 | 1 | 2 | 3 | 5;
export const SCCP_CODEC_CANONICAL_TEXT: 1;
export const SCCP_CODEC_EVM_ADDRESS20: 2;
export const SCCP_CODEC_TRON_ADDRESS21: 5;
export const SCCP_CODEC_SOLANA_PUBKEY32: 6;
export type SccpCodecTag = 1 | 2 | 5 | 6;
export const SCCP_CODEC_KEYS: Readonly<Record<SccpCodecTag, string>>;
export type SccpPayloadKind = "transfer";
export const SCCP_PAYLOAD_KINDS: readonly SccpPayloadKind[];
export const SCCP_SOLANA_TESTNET_GENESIS_HASH: "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY";
export type SccpNetworkProfile = "sora-taira" | "ethereum-mainnet" | "ethereum-sepolia" | "bsc-mainnet" | "bsc-testnet" | "tron-mainnet" | "tron-nile" | "tron-shasta" | "solana-testnet";
export interface SccpNetworkDescriptor { readonly profile: SccpNetworkProfile; readonly tag: number; readonly domain: SccpDomain; readonly sora: boolean; readonly genesisHash?: string; }
export const SCCP_NETWORK_PROFILES: Readonly<Record<SccpNetworkProfile, SccpNetworkDescriptor>>;
export function normalizeSccpCodecValue(codec: SccpCodecTag, value: string | BinaryLike): Uint8Array;
export function sccpSourceEventDigest(laneHash: string | BinaryLike, messageId: string | BinaryLike, payloadHash: string | BinaryLike): string;
export interface SccpRegistryLimits {
  readonly max_governed_lanes: 16;
  readonly max_live_governed_routes: 64;
  readonly max_live_routes_per_lane: 8;
  readonly max_retained_routes_per_lane: 64;
  readonly max_retained_native_trust_anchors_per_lane: 4096;
}
export interface SccpResourceLimits {
  readonly max_outbound_messages_per_block: 512;
  readonly max_outbound_message_payload_bytes: 4096;
  readonly max_pending_outbound_messages: number;
  readonly max_pending_outbound_payload_bytes: number;
  readonly max_proofs_per_transaction: number;
  readonly max_proofs_per_block: number;
  readonly max_proof_bytes_per_proof: number;
  readonly max_proof_bytes_per_transaction: number;
  readonly max_proof_bytes_per_block: number;
  readonly max_native_headers_per_transaction: number;
  readonly max_native_headers_per_block: number;
  readonly max_ethereum_light_client_updates_per_transaction: number;
  readonly max_ethereum_light_client_updates_per_block: number;
  readonly max_native_header_bytes_per_transaction: number;
  readonly max_native_header_bytes_per_block: number;
  readonly max_secp256k1_recoveries_per_transaction: number;
  readonly max_secp256k1_recoveries_per_block: number;
  readonly max_bls_aggregate_checks_per_transaction: number;
  readonly max_bls_aggregate_checks_per_block: number;
  readonly max_bls_signer_contributions_per_transaction: number;
  readonly max_bls_signer_contributions_per_block: number;
  readonly max_bn254_pairing_checks_per_transaction: number;
  readonly max_bn254_pairing_checks_per_block: number;
}
export interface SccpCapabilities { readonly version: 1; readonly registry_revision: string; readonly registry_path: "/v1/sccp/registry"; readonly message_bundle_path: "/v1/sccp/proofs/message/{message_id}"; readonly proof_request_path: "/v1/sccp/proof-requests/{message_id}"; readonly recent_messages_path: "/v1/sccp/messages/recent"; readonly sora_outbound_material_path: "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material"; readonly registry_limits: SccpRegistryLimits; readonly resource_limits: SccpResourceLimits; readonly proof_submit_path: "/v1/bridge/proofs/submit" | null; readonly native_message_submit_path: "/v1/bridge/messages" | null; }
export type SccpNetworkWireName = "sora_taira" | "ethereum_mainnet" | "ethereum_sepolia" | "bsc_mainnet" | "bsc_testnet" | "tron_mainnet" | "tron_nile" | "tron_shasta" | "solana_testnet";
export interface SccpNetworkV1 { readonly network: SccpNetworkWireName; readonly profile: null; }
export interface SccpLaneIdV1 { readonly source: SccpNetworkV1; readonly target: SccpNetworkV1; }
export interface SccpTransferPayloadV1 {
  readonly version: 1;
  readonly source_domain: SccpDomain;
  readonly dest_domain: SccpDomain;
  readonly nonce: string;
  readonly route_revision: number;
  readonly asset_home_domain: SccpDomain;
  readonly asset_id_codec: SccpCodecTag;
  readonly asset_id: string;
  readonly amount: string;
  readonly sender_codec: SccpCodecTag;
  readonly sender: string;
  readonly recipient_codec: SccpCodecTag;
  readonly recipient: string;
  readonly route_id_codec: SccpCodecTag;
  readonly route_id: string;
}
export interface SccpPayloadV1 { readonly Transfer: SccpTransferPayloadV1; }
export interface SccpOutboundMessageContextV1 {
  readonly lane: SccpLaneIdV1;
  readonly destination_binding_hash: string;
  readonly route_configuration_hash: string;
}
export interface SccpHubCommitmentV1 {
  readonly version: 1;
  readonly kind: "Transfer";
  readonly context: SccpOutboundMessageContextV1;
  readonly message_id: string;
  readonly payload_hash: string;
}
export interface SccpMerkleStepV1 {
  readonly sibling_hash: string;
  readonly sibling_is_left: boolean;
}
export interface SccpMerkleProofV1 { readonly steps: readonly SccpMerkleStepV1[]; }
export interface TairaSccpMessageBundleV1 {
  readonly version: 1;
  readonly commitment_root: string;
  readonly commitment: SccpHubCommitmentV1;
  readonly merkle_proof: SccpMerkleProofV1;
  readonly payload: SccpPayloadV1;
  readonly finality_proof: string;
}
export function canonicalSccpTransferPayloadBytes(payload: SccpTransferPayloadV1): Uint8Array;
export function canonicalSccpPayloadBytes(payload: SccpPayloadV1): Uint8Array;
export function sccpLaneIdHash(lane: SccpLaneIdV1): string;
export function sccpMessageId(lane: SccpLaneIdV1, payload: SccpPayloadV1): string;
export function sccpPayloadHash(payloadBytes: BinaryLike): string;
export function sccpHubCommitmentFromPayload(context: SccpOutboundMessageContextV1, payload: SccpPayloadV1): Readonly<SccpHubCommitmentV1>;
export function canonicalSccpHubCommitmentBytes(commitment: SccpHubCommitmentV1): Uint8Array;
export function sccpCommitmentLeafHash(commitment: SccpHubCommitmentV1): string;
export function canonicalSccpMerkleProofBytes(proof: SccpMerkleProofV1): Uint8Array;
export function sccpMerkleRootFromCommitment(commitment: SccpHubCommitmentV1, proof: SccpMerkleProofV1): string;
export function canonicalTairaSccpMessageBundleBytes(bundle: TairaSccpMessageBundleV1): Uint8Array;
export function canonicalSccpMessagePublicInputsBytes(input: SccpMessagePublicInputsV1): Uint8Array;
export interface SccpBn254G1PointV1 { readonly x: string; readonly y: string; }
export interface SccpBn254G2PointV1 { readonly x_c0: string; readonly x_c1: string; readonly y_c0: string; readonly y_c1: string; }
export interface SccpGroth16Bn254IcV1 {
  readonly constant: SccpBn254G1PointV1;
  readonly signal_0: SccpBn254G1PointV1;
  readonly signal_1: SccpBn254G1PointV1;
  readonly signal_2: SccpBn254G1PointV1;
  readonly signal_3: SccpBn254G1PointV1;
  readonly signal_4: SccpBn254G1PointV1;
  readonly signal_5: SccpBn254G1PointV1;
  readonly signal_6: SccpBn254G1PointV1;
  readonly signal_7: SccpBn254G1PointV1;
  readonly signal_8: SccpBn254G1PointV1;
  readonly signal_9: SccpBn254G1PointV1;
  readonly signal_10: SccpBn254G1PointV1;
}
export interface SccpGroth16Bn254VerifyingKeyV1 {
  readonly version: 1;
  readonly alpha1: SccpBn254G1PointV1;
  readonly beta2: SccpBn254G2PointV1;
  readonly gamma2: SccpBn254G2PointV1;
  readonly delta2: SccpBn254G2PointV1;
  readonly ic: SccpGroth16Bn254IcV1;
}
export interface SccpGroth16Bn254SemanticCircuitV1 {
  readonly version: 1;
  readonly circuit_commitment: string;
  readonly witness_generator_commitment: string;
  readonly public_signal_schema_hash: string;
}
export interface SccpSemanticProofProfileV1 {
  readonly profile: "sora_taira_finality_inclusion_groth16_bn254";
  readonly commitments: SccpGroth16Bn254SemanticCircuitV1;
}
export interface SccpSoraFinalityAnchorV1 {
  readonly version: 1;
  readonly source_network: SccpNetworkV1;
  readonly protocol_version: 4;
  readonly chain_id_hash: string;
  readonly checkpoint_height: number;
  readonly checkpoint_block_hash: string;
  readonly checkpoint_context_id: string;
  readonly checkpoint_finality_artifact_hash: string;
}
export interface SccpOutboundProofPolicyV1 {
  readonly version: 1;
  readonly semantic_profile: SccpSemanticProofProfileV1;
  readonly sora_finality_anchor: SccpSoraFinalityAnchorV1;
}
export type SccpSoraOutboundExecutionSemanticsV1 = "ivm_proved_record_sccp_message_v1";
export interface SccpPortableVerifyingKeyRefV1 {
  readonly backend: string;
  readonly name: string;
  readonly version: number;
  readonly commitment: string;
}
export interface SccpSoraOutboundExecutionPolicyV1 {
  readonly version: 1;
  readonly semantics: SccpSoraOutboundExecutionSemanticsV1;
  readonly contract_artifact_sha256: string;
  readonly vk_ref: SccpPortableVerifyingKeyRefV1;
  readonly gas_limit: number;
}
export interface SccpSoraOutboundMaterialV1 {
  readonly version: 1;
  readonly registry_revision: string;
  readonly route_key: SccpRouteKeyV1;
  readonly route_configuration_hash: string;
  readonly destination_binding_hash: string;
  readonly settlement_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
  readonly policy: SccpSoraOutboundExecutionPolicyV1;
  readonly contract_artifact_b64: string;
  readonly contract_code_hash: string;
  readonly verifying_key_version: number;
}
export interface SccpSoraOutboundMaterialExpectations {
  readonly sourceProfile?: Exclude<SccpNetworkProfile, "sora-taira">;
  readonly routeId?: string;
  readonly assetKey?: string;
  readonly revision?: number;
  readonly registryRevision?: string;
}
export interface SccpDestinationDeploymentFieldsV1 {
  readonly token_address: string;
  readonly token_code_hash: string;
  readonly verifier_address: string;
  readonly verifier_code_hash: string;
  readonly verifying_key: SccpGroth16Bn254VerifyingKeyV1;
  readonly verifier_key_hash: string;
  readonly outbound_proof_policy: SccpOutboundProofPolicyV1;
  readonly route_address: string;
  readonly route_code_hash: string;
  readonly taira_to_token_multiplier: 1000000000;
}
export interface SccpEvmDestinationDeploymentV1 extends SccpDestinationDeploymentFieldsV1 {}
export interface SccpTronDestinationDeploymentV1 extends SccpDestinationDeploymentFieldsV1 {}
export interface SccpSolanaDestinationDeploymentV1 {
  readonly token_mint_address: string;
  readonly route_program_id: string;
  readonly route_program_data_address: string;
  readonly route_program_data_slot: number;
  readonly route_state_account: string;
  readonly route_program_code_hash: string;
  readonly native_verifier_program_id: string;
  readonly native_verifier_program_data_address: string;
  readonly native_verifier_program_data_slot: number;
  readonly native_verifier_material_account: string;
  readonly native_verifier_program_code_hash: string;
  readonly native_verifier_config_hash: string;
  readonly verifying_key: SccpGroth16Bn254VerifyingKeyV1;
  readonly verifier_key_hash: string;
  readonly outbound_proof_policy: SccpOutboundProofPolicyV1;
  readonly taira_to_token_multiplier: 1;
}
export type SccpDestinationDeploymentV1 =
  | Readonly<{ family: "evm"; deployment: SccpEvmDestinationDeploymentV1 }>
  | Readonly<{ family: "tron"; deployment: SccpTronDestinationDeploymentV1 }>
  | Readonly<{ family: "solana"; deployment: SccpSolanaDestinationDeploymentV1 }>;
export interface SccpSourceEmitterIdentityV1 {
  readonly address: string;
  readonly runtime_code_hash: string;
  readonly route_config_hash: string;
}
export interface SccpSolanaSourceEmitterIdentityV1 {
  readonly program_id: string;
  readonly program_data_address: string;
  readonly program_data_slot: number;
  readonly state_account: string;
  readonly program_code_hash: string;
  readonly route_config_hash: string;
}
export type SccpSourceEmitterV1 =
  | Readonly<{ emitter: "evm"; identity: SccpSourceEmitterIdentityV1 }>
  | Readonly<{ emitter: "tron"; identity: SccpSourceEmitterIdentityV1 }>
  | Readonly<{ emitter: "solana"; identity: SccpSolanaSourceEmitterIdentityV1 }>;
export interface SccpSourceIdentityV1 { readonly lane: SccpLaneIdV1; readonly emitter: SccpSourceEmitterV1; }
export interface SccpSolanaDestinationHashesV1 {
  readonly destination_binding_hash: string;
  readonly deployment_config_hash: string;
  readonly route_configuration_hash: string;
}
export interface SccpSolanaSourceIdentityHashesV1 {
  readonly source_emitter_identity_hash: string;
  readonly source_identity_hash: string;
}
export function deriveSccpSolanaDestinationHashesV1(deployment: SccpSolanaDestinationDeploymentV1, sourceProgramId: string, routeRevision?: number): Readonly<SccpSolanaDestinationHashesV1>;
/** Derive the one-way native-verifier material config before its config-addressed PDA is created. */
export function deriveSccpSolanaNativeVerifierConfigHashV1(deployment: SccpSolanaDestinationDeploymentV1, sourceProgramId: string, routeRevision?: number): string;
export function deriveSccpSolanaSourceIdentityHashesV1(identity: SccpSourceIdentityV1): Readonly<SccpSolanaSourceIdentityHashesV1>;
export type SccpRouteActivationKindV1 = "staged" | "bidirectional" | "inbound_only" | "paused" | "retired";
export interface SccpRouteActivationV1 { readonly activation: SccpRouteActivationKindV1; readonly direction: null; }
export interface SccpInboundFinalityCutoffV1 {
  readonly trust_anchor_hash: string;
  readonly max_anchor_interval_height: number;
}
export interface SccpSoraSettlementV1 { readonly asset_definition_id: string; readonly custody_owner: string; readonly payload_amount_scale: 9; }
export interface SccpGovernedRouteV1 {
  readonly lane_id: SccpLaneIdV1;
  readonly route_id: string;
  readonly asset_key: string;
  readonly revision: number;
  readonly activation: SccpRouteActivationV1;
  readonly inbound_finality_cutoff: SccpInboundFinalityCutoffV1 | null;
  readonly source_identity: SccpSourceIdentityV1;
  readonly destination: SccpDestinationDeploymentV1;
  readonly sora_outbound_execution_policy: SccpSoraOutboundExecutionPolicyV1;
  readonly settlement: SccpSoraSettlementV1;
}
export type SccpNativeProofBackendV1 =
  | "ethereum_beacon_v1"
  | "bsc_parlia_v1"
  | "tron_dpos_v1"
  | "solana_agave_v1";
export interface SccpNativeTrustAnchorV1 {
  readonly backend: Readonly<{ backend: SccpNativeProofBackendV1; protocol: null }>;
  readonly anchor_hash: string;
  readonly checkpoint_height: number;
}
export interface SccpGovernedLaneV1 {
  readonly lane_id: SccpLaneIdV1;
  readonly native_trust_anchors: readonly SccpNativeTrustAnchorV1[];
  readonly current_native_trust_anchor_hash: string | null;
  readonly routes: readonly SccpGovernedRouteV1[];
}
export interface SccpRegistry { readonly version: 1; readonly lanes: readonly SccpGovernedLaneV1[]; }
export interface SccpCanonicalTextValueV1 { readonly CanonicalText: Readonly<{ value: string }>; }
export interface SccpEvmAddressValueV1 { readonly EvmAddress20: Readonly<{ bytes: string }>; }
export interface SccpTronAddressValueV1 { readonly TronAddress21: Readonly<{ bytes: string }>; }
export interface SccpSolanaPubkeyValueV1 { readonly SolanaPubkey32: Readonly<{ bytes: string }>; }
export interface SccpTransferProjectionV1 {
  readonly version: 1;
  readonly source_domain: 0;
  readonly dest_domain: 1 | 2 | 3 | 5;
  readonly nonce: string;
  readonly route_revision: number;
  readonly asset_home_domain: 0;
  readonly asset_id: SccpCanonicalTextValueV1;
  readonly amount: string;
  readonly sender: SccpCanonicalTextValueV1;
  readonly recipient: SccpEvmAddressValueV1 | SccpTronAddressValueV1 | SccpSolanaPubkeyValueV1;
  readonly route_id: SccpCanonicalTextValueV1;
}
export interface SccpPayloadProjectionV1 { readonly Transfer: SccpTransferProjectionV1; }
export interface SccpRecentMessage { readonly height: number; readonly commitment_index: number; readonly message_id_hex: string; readonly kind: "transfer"; readonly source_profile: "sora-taira"; readonly target_profile: Exclude<SccpNetworkProfile, "sora-taira">; readonly destination_binding_hash: string; readonly route_configuration_hash: string; readonly target_domain: 1 | 2 | 3 | 5; readonly asset_id: string | null; readonly route_id: string | null; readonly recipient: string | null; readonly amount: string; readonly payload_projection: SccpPayloadProjectionV1; readonly links: Readonly<{ bundle_path: string; proof_request_path: string }>; }
export interface SccpRecentCursor { readonly from: number; readonly after_index: number; }
export interface SccpRecentMessages { readonly items: readonly SccpRecentMessage[]; readonly next: SccpRecentCursor | null; }
export interface SccpMessageBundle { readonly version: 1; readonly commitment_root: string; readonly commitment: Readonly<Record<string, unknown>>; readonly merkle_proof: Readonly<Record<string, unknown>>; readonly payload: Readonly<{ Transfer: Readonly<Record<string, unknown>> }>; readonly finality_proof: string; }
export interface SccpMessagePublicInputsV1 { readonly version: 1; readonly message_id: string; readonly payload_hash: string; readonly target_domain: 1 | 2 | 3 | 5; readonly commitment_root: string; readonly finality_height: string; readonly finality_block_hash: string; }
export type SccpDestinationProofBackendV1 =
  | Readonly<{ backend: "evm_groth16_bn254_v1"; family: null }>
  | Readonly<{ backend: "tron_groth16_bn254_v1"; family: null }>
  | Readonly<{ backend: "solana_groth16_bn254_v1"; family: null }>;
export interface SccpProofRequest {
  readonly version: 1;
  readonly backend: SccpDestinationProofBackendV1;
  readonly source_network: SccpNetworkV1;
  readonly target_network: SccpNetworkV1;
  readonly public_inputs: SccpMessagePublicInputsV1;
  readonly verifying_key: SccpGroth16Bn254VerifyingKeyV1;
  readonly verifier_key_hash: string;
  readonly semantic_proof_profile: SccpSemanticProofProfileV1;
  readonly semantic_proof_profile_hash: string;
  readonly sora_finality_anchor: SccpSoraFinalityAnchorV1;
  readonly sora_finality_anchor_hash: string;
  readonly bundle_bytes: string;
  readonly statement_hash: string;
  readonly destination_binding_hash: string;
  readonly route_configuration_hash: string;
  readonly request_hash: string;
}
export type SccpDetachedSigningState =
  | Readonly<{ signature_b64?: never; transaction_payload_b64?: never; creation_time_ms?: number }>
  | Readonly<{ signature_b64: string; transaction_payload_b64: string; creation_time_ms: number }>;
export type SccpBridgeProofSubmitPayload = Readonly<{
  authority: string;
  fee_payment: NoritoFeePaymentIntent;
  destination_proof_b64: string;
}> & SccpDetachedSigningState;
export type SccpBridgeMessageSubmitPayload = Readonly<{
  authority: string;
  fee_payment: NoritoFeePaymentIntent;
  native_proof_b64: string;
}> & SccpDetachedSigningState;
export interface SccpBridgeSubmitResponse { readonly submitted: boolean; readonly payload_kind: SccpPayloadKind; readonly message_id_hex: string; readonly backend: string; readonly counterparty_domain: number; readonly counterparty_chain: SccpNetworkProfile; readonly route_configuration_hash_hex: string; readonly range_start_height: number; readonly range_end_height: number; readonly creation_time_ms: number; readonly tx_hash_hex: string | null; readonly transaction_payload_b64: string | null; readonly signing_message_b64: string | null; }
export interface SccpBridgeResponseExpectations { readonly submitted?: boolean; readonly creation_time_ms?: number; }
export function normalizeSccpCapabilities(value: unknown): SccpCapabilities;
export function normalizeSccpSoraOutboundMaterial(value: unknown, expectations?: SccpSoraOutboundMaterialExpectations): SccpSoraOutboundMaterialV1;
export function normalizeSccpRegistry(value: unknown): SccpRegistry;
export function normalizeSccpRouteGovernanceAction(value: SccpRouteGovernanceActionInput): Readonly<SccpRouteGovernanceActionInput>;
export function normalizeSccpRecentMessages(value: unknown): SccpRecentMessages;
export function normalizeSccpMessageBundle(value: unknown): SccpMessageBundle;
export function normalizeSccpProofRequest(value: unknown): SccpProofRequest;
export function normalizeBridgeProofSubmitPayload(value: SccpBridgeProofSubmitPayload): Readonly<SccpBridgeProofSubmitPayload>;
export function normalizeBridgeMessageSubmitPayload(value: SccpBridgeMessageSubmitPayload): Readonly<SccpBridgeMessageSubmitPayload>;
export function normalizeSccpBridgeSubmitResponse(value: unknown, expectations?: SccpBridgeResponseExpectations): SccpBridgeSubmitResponse;
export function parseSccpBridgeSubmitResponseJson(text: string, expectations?: SccpBridgeResponseExpectations): SccpBridgeSubmitResponse;
export function parseSccpJsonObject(text: string, label?: string): Readonly<Record<string, unknown>>;

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
 * Numeric values accepted by non-quantity helpers. Quantity-bearing APIs use
 * {@link QuantityInput} so JavaScript `number` can never lose precision.
 */
export type NumericLike = string | number | bigint;

/** Lossless canonical input accepted by asset and RWA quantity builders. */
export type QuantityInput = KotodamaQuantity | string | bigint;

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

export type VerifyingKeyIdLike = string | { backend: string; name: string };

/** Exact JSON labels for the two generic OpenVerify engines in Norito order. */
export type OpenVerifyBackendTag = "halo2-ipa-pasta" | "stark";

export interface OpenVerifyEnvelope {
  backend: OpenVerifyBackendTag;
  circuit_id: string;
  vk_hash: BinaryLike;
  public_inputs: BinaryLike;
  proof_bytes: BinaryLike;
  aux?: BinaryLike;
}

export interface ProofAttachmentInput {
  backend: string;
  proof: BinaryLike;
  verifyingKeyRef: { backend: string; name: string };
  verifyingKeyCommitment?: BinaryLike | null;
  envelopeHash?: BinaryLike | null;
  lanePrivacy?: {
    commitmentId: number;
    merkle: {
      leaf: BinaryLike;
      leafIndex: number;
      auditPath: BinaryLike[];
    };
  } | null;
}

/**
 * Canonicalise an account identifier to i105.
 *
 * Accepts only encoded i105 account ids.
 * Domain-suffixed literals (`<id>@domain`) and canonical-hex account literals are rejected.
 */
export function normalizeAccountId(value: string, name?: string): string;
export function ensureCanonicalAccountId(value: string, name?: string): string;
export function normalizeI105AccountId(value: string, name?: string): string;
export type IdentifierNormalization =
  | "exact"
  | "lowercase_trimmed"
  | "phone_e164"
  | "email_address"
  | "account_number";
export function normalizeIdentifierInput(
  value: string,
  normalization: IdentifierNormalization,
  name?: string,
): string;
export function tryNormalizeI105AccountId(
  value: unknown,
  name?: string,
): string | null;
export function normalizeToriiAccountReference(
  value: unknown,
  name?: string,
): string;
export function normalizeAccountAliasFqn(value: string, name?: string): string;
export function tryNormalizeAccountAliasFqn(
  value: unknown,
  name?: string,
): string | null;

/**
 * Canonicalise a public asset identifier to bare Base58 form.
 * Asset aliases (`name#dataspace` / `name#domain.dataspace`) must be resolved first.
 */
export function normalizeAssetId(value: string, name?: string): string;
export function normalizeAssetDefinitionId(
  value: string,
  name?: string,
): string;
export function tryNormalizeAssetDefinitionId(
  value: unknown,
  name?: string,
): string | null;
export function normalizeAssetAliasFqn(value: string, name?: string): string;
export function tryNormalizeAssetAliasFqn(
  value: unknown,
  name?: string,
): string | null;

/**
 * Canonicalise an internal asset-holding identifier in
 * `<base58-asset-definition-id>#<i105-account-id>` form.
 */
export function normalizeAssetHoldingId(value: string, name?: string): string;
export function composeAssetHoldingId(
  assetId: string,
  accountId: string,
  dataspaceId?: string | number | null,
  name?: string,
): string;
export function extractAssetDefinitionId(value: string, name?: string): string;
export function tryExtractAssetDefinitionId(
  value: unknown,
  name?: string,
): string | null;
export function assetReferencesMatch(left: unknown, right: unknown): boolean;

/**
 * Canonicalise an RWA identifier in `<64-hex-hash>$<domain>` form.
 */
export function normalizeRwaId(value: string, name?: string): string;

export function blake2b256(
  data: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
  options?: {
    personalization?: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView;
    includeZeroKeyBlock?: boolean;
  },
): Uint8Array;

export const IVM_PROGRAM_HEADER_LENGTH: 49;
export const IVM_ARTIFACT_MAX_BYTES: 4194304;

/** Compute ledger/Core body identity and full-artifact SHA-256 identity. */
export function computeIvmArtifactHashes(
  artifact: Uint8Array | ArrayBuffer | ArrayBufferView,
): {
  codeHashHex: string;
  artifactSha256Hex: string;
};

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
  signal?: AbortSignal;
}

export interface CanonicalRequestAuth {
  /** Exact canonical I105 account or canonical ASCII account alias. */
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

export type ToriiCountMode = "bounded" | "exact";

export interface ToriiBrowserCountedListOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  countMode?: ToriiCountMode;
  count_mode?: ToriiCountMode;
  signal?: AbortSignal;
}

export type ToriiBrowserAccountPermissionsListOptions =
  ToriiBrowserCountedListOptions;

export interface ToriiBrowserAccountHistoryListOptions
  extends ToriiBrowserCountedListOptions {
  assetId?: string;
  asset_id?: string;
}

export interface ToriiBrowserContractActivityListOptions
  extends ToriiBrowserCountedListOptions {
  authority?: string;
  contractAddress?: string;
  contract_address?: string;
  contractAlias?: string;
  contract_alias?: string;
  contractEntrypoint?: string;
  contract_entrypoint?: string;
  sinceTimestampMs?: NumericLike;
  since_timestamp_ms?: NumericLike;
  untilTimestampMs?: NumericLike;
  until_timestamp_ms?: NumericLike;
  resultOk?: boolean;
  result_ok?: boolean;
}

export interface ToriiBrowserContractEventListOptions
  extends ToriiBrowserCountedListOptions {
  authority?: string;
  contractAddress?: string;
  contract_address?: string;
  contractAlias?: string;
  contract_alias?: string;
  module?: string;
  eventKind?: string;
  event_kind?: string;
  participant?: string;
  assetId?: string;
  asset_id?: string;
  provenance?: "emitted" | "derived";
  sinceTimestampMs?: NumericLike;
  since_timestamp_ms?: NumericLike;
  untilTimestampMs?: NumericLike;
  until_timestamp_ms?: NumericLike;
  resultOk?: boolean;
  result_ok?: boolean;
}

export interface ToriiBrowserContractEventStreamOptions
  extends Omit<ToriiBrowserContractEventListOptions, keyof ToriiBrowserCountedListOptions> {
  signal?: AbortSignal;
}

export interface IterableListOptions extends PermissionedIterableOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  filter?: string | Record<string, unknown>;
  sort?: string | ReadonlyArray<{ key: string; order?: "asc" | "desc" }>;
  countMode?: ToriiCountMode;
  count_mode?: ToriiCountMode;
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
  fetch_size?: NumericLike;
  queryName?: string;
  query_name?: string;
  select?: ReadonlyArray<string | Record<string, unknown>>;
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
  limit?: NumericLike;
  cursor?: string;
  ownedBy?: string;
  domainId?: string;
  signal?: AbortSignal;
}

export interface ExplorerNftIteratorOptions extends ExplorerNftListOptions {
  maxItems?: NumericLike;
}

export interface ExplorerRwaListOptions {
  limit?: NumericLike;
  cursor?: string;
  ownedBy?: string;
  domainId?: string;
  signal?: AbortSignal;
}

export interface ExplorerRwaIteratorOptions extends ExplorerRwaListOptions {
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

export interface ToriiRepoAgreement extends RepoAgreementLifecycleFields {
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

/** Exact wire shape returned by Torii app-list routes. */
export interface ToriiBrowserCountedListResponse<T = unknown> {
  items: ReadonlyArray<T>;
  /** Present only when `count_mode` is `exact`. */
  total?: number;
  has_more: boolean;
  count_mode: ToriiCountMode;
}

export type ToriiBrowserAccountPermissionsListResponse<
  T = ToriiAccountPermissionItem,
> = ToriiBrowserCountedListResponse<T>;

export interface ToriiBrowserAccountHistoryListResponse<
  T = ToriiAccountHistoryItem,
> extends ToriiBrowserCountedListResponse<T> {
  indexed_height: number;
  indexed_block_hash: string | null;
  query_source: "account_history_index";
}

export type ToriiBrowserContractActivityListResponse<
  T = ToriiContractActivityItem,
> = ToriiBrowserCountedListResponse<T>;

export type ToriiBrowserContractEventListResponse<
  T = ToriiContractEventItem,
> = ToriiBrowserCountedListResponse<T>;

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

export interface RequiredCanonicalRequestOptions {
  signal?: AbortSignal;
  canonicalAuth: CanonicalRequestAuth;
}

export interface AbortSignalOptions {
  signal?: AbortSignal;
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

export interface RetailRecipientLookupRequest {
  accountId?: string;
  account_id?: string;
  aliasFqn?: string;
  alias_fqn?: string;
}

export interface RetailRecipientLookupResponse {
  resolved: boolean;
  account_id: string;
  alias_fqn: string;
  fi_id: "hbl.sbp" | "ubl.sbp";
  full_name?: string;
}

export interface RetailRecipientRouteResponse {
  account_id: string;
  alias_fqn: string;
  fi_id: "hbl.sbp" | "ubl.sbp";
}

export interface FeeSponsorProgramId {
  sponsor: string;
  name: string;
}

export type FeeSponsorProgramLifecycleState =
  | "staged"
  | "paused"
  | "active"
  | "closing"
  | "closed";

export interface FeeSponsorProgram {
  id: FeeSponsorProgramId;
  payout_account: string;
  lifecycle: { state: FeeSponsorProgramLifecycleState; value: null };
  active_revision?: number | null;
  staged_revision?: number | null;
  scheduled_activation?: {
    revision: number;
    activate_at_height: number;
  } | null;
}

export interface NoritoFeeChargeKind {
  kind: "nexus" | "pipeline_gas";
  value: null;
}

export interface NoritoFeeChargeLimit {
  kind: NoritoFeeChargeKind;
  asset_definition_id: string;
  max_amount: string;
}

export type NoritoFeePaymentIntent =
  | {
      payer: "authority";
      value: {
        charge_limits: ReadonlyArray<NoritoFeeChargeLimit>;
        gas_limit?: number | null;
      };
    }
  | {
      payer: "sponsor";
      value: {
        program_id: FeeSponsorProgramId;
        program_revision: number;
        charge_limits: ReadonlyArray<NoritoFeeChargeLimit>;
        gas_limit?: number | null;
      };
    };

export type FeeDebitSource =
  | { kind: "account"; value: string }
  | { kind: "sponsor_program"; value: FeeSponsorProgramId };

export interface FeeQuoteResponse {
  intent: NoritoFeePaymentIntent;
  observation: {
    ledger_time_ms: number;
    next_block_height: number;
    route_dataspace_id?: number | null;
  };
  components: ReadonlyArray<{
    kind: NoritoFeeChargeKind;
    asset_definition_id: string;
    max_amount: string;
  }>;
  capacities: ReadonlyArray<{
    asset_definition_id: string;
    vault_balance: string;
    reserve_floor: string;
    block_remaining: string;
    program_epoch_remaining: string;
    beneficiary_epoch_remaining: string;
  }>;
  decision: {
    status: "accepted";
    value: {
      debit_source: FeeDebitSource;
      program_revision?: number | null;
    };
  };
}

export type FeeRejectionCode =
  | "invalid_fee_intent"
  | "program_not_found"
  | "revision_not_found"
  | "revision_not_active"
  | "program_not_active"
  | "beneficiary_not_eligible"
  | "operation_not_allowed"
  | "operation_denied"
  | "invalid_gas_limit"
  | "fee_asset_not_covered"
  | "signed_limit_exceeded"
  | "program_transaction_limit_exceeded"
  | "program_block_budget_exhausted"
  | "program_epoch_budget_exhausted"
  | "beneficiary_epoch_budget_exhausted"
  | "vault_insufficient"
  | "authority_payer_insufficient"
  | "relay_capacity_unavailable"
  | "invalid_program_configuration";

export type IdentifierBfvInteger = number | bigint;

export interface IdentifierBfvParameters {
  polynomial_degree: number;
  plaintext_modulus: IdentifierBfvInteger;
  ciphertext_modulus: IdentifierBfvInteger;
  decomposition_base_log: number;
}

export interface IdentifierBfvPublicKey {
  b: ReadonlyArray<IdentifierBfvInteger>;
  a: ReadonlyArray<IdentifierBfvInteger>;
}

export interface IdentifierBfvPublicParameters {
  parameters: IdentifierBfvParameters;
  public_key: IdentifierBfvPublicKey;
  max_input_bytes: number;
  norito_length_encoding?: string;
}

export interface RamLfeProgramProfile {
  profile_version: number;
  register_count: number;
  memory_lane_count: number;
  ciphertext_mul_per_step: number;
  encrypted_input_mode: "encrypted_envelope_v1";
  min_ciphertext_modulus: IdentifierBfvInteger;
}

export interface RamLfeProofVerifierMetadata {
  proof_backend: string;
  circuit_id: string;
  public_inputs_schema_hash: string;
  verifying_key_bytes_b64: string;
}

export interface RamLfeProgramPolicySummary {
  program_id: string;
  owner: string;
  active: boolean;
  resolver_public_key: string;
  output_opening_public_key: string;
  backend: string;
  verification_mode: string;
  input_encryption?: string;
  input_encryption_public_parameters?: string;
  input_encryption_public_parameters_decoded?: IdentifierBfvPublicParameters;
  ram_fhe_profile?: RamLfeProgramProfile;
  proof_verifier?: RamLfeProofVerifierMetadata;
  note?: string;
}

export interface RamLfeProgramPolicyListResponse {
  total: number;
  items: ReadonlyArray<RamLfeProgramPolicySummary>;
}

export interface IdentifierPolicySummary {
  policy_id: string;
  program_id: string;
  owner: string;
  active: boolean;
  normalization: string;
  resolver_public_key: string;
  output_opening_public_key: string;
  backend: string;
  input_encryption?: string;
  input_encryption_public_parameters?: string;
  input_encryption_public_parameters_decoded?: IdentifierBfvPublicParameters;
  ram_fhe_profile?: RamLfeProgramProfile;
  proof_verifier?: RamLfeProofVerifierMetadata;
  note?: string;
}

export interface IdentifierPolicyListResponse {
  total: number;
  items: ReadonlyArray<IdentifierPolicySummary>;
}

export type IdentifierPolicyClientSummary = Omit<
  IdentifierPolicySummary,
  "program_id" | "output_opening_public_key"
> &
  Partial<
    Pick<IdentifierPolicySummary, "program_id" | "output_opening_public_key">
  >;

export interface RamLfeOutputOpeningPayload {
  program_id: string;
  input_ciphertext_hash: string;
  output_ciphertext_hash: string;
  parameter_digest: string;
  evaluation_key_digest: string;
  opened_output_hash: string;
  opened_at_ms: number;
  expires_at_ms: number | null;
}

export interface RamLfeOutputOpening {
  payload: RamLfeOutputOpeningPayload;
  signature: string;
}

export interface RamLfeExecutionReceiptPayload {
  program_id: string;
  program_digest: string;
  backend: string;
  verification_mode: string;
  input_ciphertext_hash: string;
  output_ciphertext_hash: string;
  parameter_digest: string;
  evaluation_key_digest: string;
  output_hash: string;
  associated_data_hash: string;
  executed_at_ms: number;
  expires_at_ms: number | null;
}

export type RamLfeReceiptAttestation =
  | { kind: "signed"; signature: string }
  | { kind: "proof"; proof_backend: string; proof_b64: string };

export interface RamLfeExecutionReceipt {
  payload: RamLfeExecutionReceiptPayload;
  attestation: RamLfeReceiptAttestation;
}

export interface RamLfeExecuteOptions {
  encryptedInput: string;
  signal?: AbortSignal;
  canonicalAuth: CanonicalRequestAuth;
}

export interface RamLfeExecuteResponse {
  program_id: string;
  opaque_hash: string;
  receipt_hash: string;
  output_ciphertext: string;
  output_hash: string;
  associated_data_hash: string;
  executed_at_ms: number;
  expires_at_ms: number | null;
  backend: string;
  verification_mode: string;
  receipt: RamLfeExecutionReceipt;
  output_opening: RamLfeOutputOpening;
}

export interface IdentifierResolutionRequestOptions {
  policyId: string;
  encryptedInput: string;
  outputOpening: RamLfeOutputOpening;
  signal?: AbortSignal;
  canonicalAuth: CanonicalRequestAuth;
}

export interface IdentifierResolutionReceiptPayload {
  policy_id: string;
  execution: RamLfeExecutionReceiptPayload;
  opening: RamLfeOutputOpening;
  opaque_id: string;
  receipt_hash: string;
  uaid: string;
  account_id: string;
}

export interface IdentifierResolutionReceipt {
  payload: IdentifierResolutionReceiptPayload;
  attestation: RamLfeReceiptAttestation;
}

export interface IdentifierClaimLookupResponse {
  policy_id: string;
  opaque_id: string;
  receipt_hash: string;
  uaid: string;
  account_id: string;
  verified_at_ms: number;
  expires_at_ms: number | null;
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

export interface ToriiAccountHistoryItem {
  id: string;
  source: string;
  type: string;
  timestamp_ms?: number;
  status: string;
  result_ok?: boolean;
  direction: string;
  account_id: string;
  counterparty_account_id?: string;
  asset_id?: string;
  asset_definition_id?: string;
  amount?: string;
  tx_hash?: string;
  operation_id?: string;
  expires_at_ms?: number;
  finalized_at_ms?: number;
  requesting_fi_id?: string;
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
  fee_payment?: NoritoFeePaymentIntent;
}

export interface ToriiContractEventItem {
  event_id: string;
  schema_version: number;
  provenance: "emitted" | "derived";
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
  fee_payment?: NoritoFeePaymentIntent;
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

/** Exact verifier-registry labels admitted by the native Rust dispatcher. */
export type ToriiVerifierBackendLabelV1 =
  | "halo2/ipa"
  | "halo2/pasta/kaigi-roster-v1"
  | "halo2/pasta/kaigi-usage-v1"
  | "halo2/pasta/ivm-execution-v1"
  | "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3"
  | "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3"
  | "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3"
  | "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4"
  | "stark/fri"
  | "stark/fri/sha256-goldilocks"
  | "stark/fri/poseidon2-goldilocks"
  | "stark/fri/sha256_goldilocks.v1";

export interface ToriiVerifyingKeyInline {
  backend: ToriiVerifierBackendLabelV1;
  bytes_b64: string;
}

export interface ToriiVerifyingKeyRecord {
  version: number;
  circuit_id: string;
  backend: ToriiVerifierBackendLabelV1;
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
  backend: ToriiVerifierBackendLabelV1;
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
  backend?: ToriiVerifierBackendLabelV1;
  status?: ToriiVerifyingKeyStatus | string;
  nameContains?: string;
  limit?: NumericLike;
  offset?: NumericLike;
  order?: "asc" | "desc";
  idsOnly?: boolean;
  signal?: AbortSignal | null;
}

export interface ToriiVerifyingKeyTransactionDraft {
  readonly submitted: false;
  readonly transaction_payload_b64: string;
  readonly signing_message_b64: string;
}

export interface AppApiTransactionDraft {
  readonly submitted: false;
  readonly transaction_payload_b64: string;
  readonly signing_message_b64: string;
}

export interface ToriiVerifyingKeyRegisterPayload {
  authority: string;
  backend: ToriiVerifierBackendLabelV1;
  name: string;
  version: NumericLike;
  circuit_id: string;
  public_inputs_schema_hash_hex: string;
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
  backend: ToriiVerifierBackendLabelV1;
  name: string;
  version: NumericLike;
  circuit_id: string;
  public_inputs_schema_hash_hex: string;
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

/** Seek-pagination metadata for canonical Explorer world collections. */
export interface ToriiExplorerCursorMeta {
  limit: number;
  nextCursor: string | null;
  hasMore: boolean;
}

export interface ToriiExplorerNft {
  id: string;
  ownedBy: string;
  metadata: Record<string, unknown>;
}

export interface ToriiExplorerNftsPage {
  pagination: ToriiExplorerCursorMeta;
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
  pagination: ToriiExplorerCursorMeta;
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
  operatorAccountId: string;
  leaseFee: string;
  settlementGraceSecs: number;
  flowLabelBits: number;
  paddingBudgetMs: number;
  relayIdHex: string;
  relayMldsa65PublicKeyHex: string;
  descriptorCommitHex: string;
  tlsServerName: string;
  relayTlsSpkiSha256Hex: string;
  relayCertificateSha256Hex: string;
  directorySnapshotDigestHex: string;
}

export interface ToriiVpnTxInstruction {
  wireId: string;
  payloadHex: string;
}

export interface ToriiVpnQuote {
  quoteId: string;
  leaseIdHex: string;
  /** Canonical 16-byte session ID encoded as 32 lowercase hex characters. */
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
  leaseFee: string;
  routePushes: ReadonlyArray<string>;
  excludedRoutes: ReadonlyArray<string>;
  dnsServers: ReadonlyArray<string>;
  tunnelAddresses: ReadonlyArray<string>;
  mtuBytes: number;
  meterFamily: string;
  flowLabelBits: number;
  paddingBudgetMs: number;
  relayIdHex: string;
  relayMldsa65PublicKeyHex: string;
  descriptorCommitHex: string;
  tlsServerName: string;
  relayTlsSpkiSha256Hex: string;
  relayCertificateSha256Hex: string;
  directorySnapshotDigestHex: string;
  meteringPublicKeyHex: string;
  openLeaseInstruction: ToriiVpnTxInstruction;
}

export interface ToriiVpnSession {
  /** Canonical 16-byte session ID encoded as 32 lowercase hex characters. */
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
  leaseFee: string;
  flowLabelBits: number;
  paddingBudgetMs: number;
  relayIdHex: string;
  relayMldsa65PublicKeyHex: string;
  descriptorCommitHex: string;
  tlsServerName: string;
  relayTlsSpkiSha256Hex: string;
  relayCertificateSha256Hex: string;
  directorySnapshotDigestHex: string;
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
  /** Canonical 16-byte session ID encoded as 32 lowercase hex characters. */
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
  status:
    | "disconnected"
    | "expired"
    | "replaced"
    | "settlement_pending"
    | "settled";
  receiptSource: string;
  quoteId: string;
  paymentTxHash: string;
  feeAssetId: string;
  escrowAccountId: string;
  operatorAccountId: string;
  leaseFee: string;
  earnedFee: string;
  refundedFee: string;
  leaseIdHex: string;
  settleLeaseInstruction: ToriiVpnTxInstruction | null;
}

export interface ToriiVpnReceiptListResponse {
  items: ReadonlyArray<ToriiVpnReceipt>;
  total: number;
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
  kind: "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired";
  block_height?: number;
}

export interface ToriiPipelineTransactionStatus {
  hash: string;
  status: ToriiPipelineTransactionStatusStatus;
  scope: "local" | "global";
  resolved_from: "cache" | "queue" | "state";
}

/** Exact finality returned only after global state has applied the transaction. */
export interface ToriiAppliedTransactionStatus
  extends ToriiPipelineTransactionStatus {
  status: {
    kind: "Applied";
    block_height: number;
  };
  scope: "global";
  resolved_from: "state";
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

export interface ToriiContractEventStreamErrorPayload {
  code: string;
  message: string;
  dropped_messages: number | null;
  replay_available: boolean;
}

export interface ToriiWebSocketEvent<T = unknown> {
  event: string | null;
  data: T | string;
  raw: string;
}

export interface AccountPermissionsListOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  signal?: AbortSignal;
}

/** An effective permission, including grants inherited from assigned roles. */
export interface ToriiAccountPermissionItem {
  name: string;
  payload: JsonValue;
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
  manifestCidHex: string;
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

export declare const SORAFS_ORDERBOOK_PAYLOAD_KINDS: Readonly<{
  ORDER_REQUEST: "order-request";
  ORDER_CANCEL: "order-cancel";
  TRADE_EVENT: "trade-event";
  SETTLEMENT_CHANNEL: "settlement-channel";
  SETTLEMENT_RECEIPT: "settlement-receipt";
}>;

/** Canonical maximum byte length for a V1 orderbook owner account. */
export declare const ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1: 256;

export type SorafsOrderbookPayloadKind =
  | "order-request"
  | "order-cancel"
  | "trade-event"
  | "settlement-channel"
  | "settlement-receipt";

export type SorafsOrderbookSignablePayloadKind =
  | "order-request"
  | "order-cancel"
  | "settlement-receipt";

export type SorafsOrderbookCancelReason =
  | "owner-requested"
  | "expired"
  | "governance"
  | "replaced";

export type SorafsOrderbookIntegerInput = number | bigint | string;
/** Canonical, non-negative XOR quantity text with at most nine fractional digits. */
export type SorafsOrderbookXorQuantityInput = string;
export type SorafsOrderbookBytesInput = ArrayBufferView | ArrayBuffer | Buffer;

export interface SorafsSignedOrderbookOrderRequestFields {
  orderId?: SorafsOrderbookBytesInput;
  side: SorafsOrderbookSide;
  tier: SorafsOrderbookTier;
  pricePerGib: SorafsOrderbookXorQuantityInput;
  quantityGib: SorafsOrderbookIntegerInput;
  remainingGib?: SorafsOrderbookIntegerInput;
  ownerAccount: SorafsOrderbookBytesInput;
  /** Exact non-zero 32-byte provider identity for asks; omit or pass empty bytes for bids. */
  providerId?: SorafsOrderbookBytesInput;
  expiryUnix: SorafsOrderbookIntegerInput;
  nonce: SorafsOrderbookIntegerInput;
  makerFeeBps: SorafsOrderbookIntegerInput;
  takerFeeBps: SorafsOrderbookIntegerInput;
}

export interface SorafsSignedOrderbookOrderCancelFields {
  orderId: SorafsOrderbookBytesInput;
  ownerAccount: SorafsOrderbookBytesInput;
  reason: SorafsOrderbookCancelReason;
  nonce: SorafsOrderbookIntegerInput;
}

export interface SorafsSignedOrderbookSettlementReceiptFields {
  receiptId: SorafsOrderbookBytesInput;
  channelId: SorafsOrderbookBytesInput;
  tradeId: SorafsOrderbookBytesInput;
  rangeStart: SorafsOrderbookIntegerInput;
  rangeEnd: SorafsOrderbookIntegerInput;
  chunkHash: SorafsOrderbookBytesInput;
  bytesDelivered: SorafsOrderbookIntegerInput;
  xorDebited: SorafsOrderbookXorQuantityInput;
  providerCredit: SorafsOrderbookXorQuantityInput;
  feeAmount: SorafsOrderbookXorQuantityInput;
  issuedAtUnix: SorafsOrderbookIntegerInput;
}

export declare const SORAFS_PDP_PAYLOAD_KINDS: Readonly<{
  COMMITMENT: "commitment";
  CHALLENGE: "challenge";
  PROOF: "proof";
}>;

export declare const SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS: Readonly<{
  PROVIDER_ADVERT: "provider-advert";
  PROVIDER_ADMISSION_ENVELOPE: "provider-admission-envelope";
  REPLICATION_ORDER: "replication-order";
  POR_CHALLENGE: "por-challenge";
  POR_PROOF: "por-proof";
  POTR_RECEIPT: "potr-receipt";
  REPAIR_EVIDENCE: "repair-evidence";
  REPAIR_REPORT: "repair-report";
  REPAIR_TASK_RECORD: "repair-task-record";
  REPAIR_SLASH_PROPOSAL: "repair-slash-proposal";
  REPAIR_TASK_EVENT: "repair-task-event";
  ORDERBOOK_ORDER_REQUEST: "orderbook-order-request";
  ORDERBOOK_ORDER_CANCEL: "orderbook-order-cancel";
  ORDERBOOK_TRADE_EVENT: "orderbook-trade-event";
  ORDERBOOK_SETTLEMENT_CHANNEL: "orderbook-settlement-channel";
  ORDERBOOK_SETTLEMENT_RECEIPT: "orderbook-settlement-receipt";
  PDP_COMMITMENT: "pdp-commitment";
  PDP_CHALLENGE: "pdp-challenge";
  PDP_PROOF: "pdp-proof";
}>;

export declare const SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1: 64;
export declare const SORAFS_GOVERNANCE_DAG_MAX_BLOCKS_V1: 64;
export declare const SORAFS_GOVERNANCE_DAG_CID_BYTES_V1: 32;
export declare const SORAFS_REFERENCE_MAX_INPUT_BYTES_V1: 67108864;
export declare const SORAFS_REFERENCE_MAX_LABEL_BYTES_V1: 1024;

export type SorafsPdpPayloadKind =
  | "commitment"
  | "challenge"
  | "proof";

export type SorafsFixtureBundlePayloadKind =
  | "provider-advert"
  | "provider-admission-envelope"
  | "replication-order"
  | "por-challenge"
  | "por-proof"
  | "potr-receipt"
  | "repair-evidence"
  | "repair-report"
  | "repair-task-record"
  | "repair-slash-proposal"
  | "repair-task-event"
  | "orderbook-order-request"
  | "orderbook-order-cancel"
  | "orderbook-trade-event"
  | "orderbook-settlement-channel"
  | "orderbook-settlement-receipt"
  | "pdp-commitment"
  | "pdp-challenge"
  | "pdp-proof";

export interface SorafsValidationContextField {
  key: string;
  value: string;
}

export interface SorafsValidationInput {
  kind: string;
  path: string;
}

export type SorafsValidationCategory =
  | "validation"
  | "policy"
  | "signature"
  | "norito"
  | "internal";

export interface SorafsValidationOutcomeFields {
  code: string;
  message: string;
  docs_url: "https://docs.iroha.tech/";
  telemetry_tags: ReadonlyArray<string>;
  context: ReadonlyArray<SorafsValidationContextField>;
  inputs: ReadonlyArray<SorafsValidationInput>;
  version: 1;
  generated_at: number;
}

export type SorafsValidationOutcome =
  | (SorafsValidationOutcomeFields & {
      status: "Ok";
      category: "validation";
      action: null;
    })
  | (SorafsValidationOutcomeFields & {
      status: "Error";
      category: SorafsValidationCategory;
      action: string;
    });

export interface SorafsOrderbookValidationOptions {
  label?: string;
  generatedAtUnix?: number | bigint;
}

export interface SorafsAppealFinanceValidationOptions {
  label?: string;
  generatedAtUnix?: number | bigint;
}

export interface SorafsPdpPayloadValidationOptions {
  label?: string;
  generatedAtUnix?: number | bigint;
}

export interface SorafsPdpPairValidationOptions {
  commitmentLabel?: string;
  challengeLabel?: string;
  proofLabel?: string;
  generatedAtUnix?: number | bigint;
}

export type SorafsReferenceBytesInput =
  | ArrayBufferView
  | ArrayBuffer
  | Buffer;

export interface SorafsGovernanceDagBlockInput {
  bytes: SorafsReferenceBytesInput;
  label?: string;
}

export interface SorafsFixtureBundlePayloadInput {
  kind: SorafsFixtureBundlePayloadKind;
  bytes: SorafsReferenceBytesInput;
  label?: string;
}

export interface SorafsFixtureBundleValidationOptions {
  nowUnix?: number | bigint;
  generatedAtUnix?: number | bigint;
}

export interface SorafsGovernanceLogNodeValidationOptions {
  label?: string;
  expectedNodeCid: SorafsReferenceBytesInput;
  generatedAtUnix?: number | bigint;
}

export interface SorafsGovernanceDagBlockValidationOptions {
  label?: string;
  expectedBlockCid?: SorafsReferenceBytesInput;
  generatedAtUnix?: number | bigint;
}

export interface SorafsGovernanceDagHeadValidationOptions {
  headLabel?: string;
  generatedAtUnix?: number | bigint;
}

export function decodeReplicationOrder(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
): SorafsReplicationOrder;

export function validateOrderbookPayload(
  kind: SorafsOrderbookPayloadKind,
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: SorafsOrderbookValidationOptions,
): SorafsValidationOutcome;

export function validateAppealFinanceCancelAssetLock(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: SorafsAppealFinanceValidationOptions,
): SorafsValidationOutcome;

export function signOrderbookPayload(
  kind: SorafsOrderbookSignablePayloadKind,
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function deriveOrderbookOrderId(
  ownerAccount: SorafsOrderbookBytesInput,
  nonce: SorafsOrderbookIntegerInput,
): Buffer;

export function buildSignedOrderbookOrderRequest(
  fields: SorafsSignedOrderbookOrderRequestFields,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function buildSignedOrderbookOrderCancel(
  fields: SorafsSignedOrderbookOrderCancelFields,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function buildSignedOrderbookSettlementReceipt(
  fields: SorafsSignedOrderbookSettlementReceiptFields,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function validatePdpPayload(
  kind: SorafsPdpPayloadKind,
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: SorafsPdpPayloadValidationOptions,
): SorafsValidationOutcome;

export function validatePdpCommitmentChallenge(
  commitmentBytes: ArrayBufferView | ArrayBuffer | Buffer,
  challengeBytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: SorafsPdpPairValidationOptions,
): SorafsValidationOutcome;

export function validatePdpChallengeProof(
  challengeBytes: ArrayBufferView | ArrayBuffer | Buffer,
  proofBytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: SorafsPdpPairValidationOptions,
): SorafsValidationOutcome;

export function validatePdpBundle(
  commitmentBytes: ArrayBufferView | ArrayBuffer | Buffer,
  challengeBytes: ArrayBufferView | ArrayBuffer | Buffer,
  proofBytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: SorafsPdpPairValidationOptions,
): SorafsValidationOutcome;

export function validateFixtureBundle(
  payloads: ReadonlyArray<SorafsFixtureBundlePayloadInput>,
  options?: SorafsFixtureBundleValidationOptions,
): SorafsValidationOutcome;

export function validateGovernanceLogNode(
  bytes: SorafsReferenceBytesInput,
  options: SorafsGovernanceLogNodeValidationOptions,
): SorafsValidationOutcome;

export function validateGovernanceDagBlock(
  bytes: SorafsReferenceBytesInput,
  options?: SorafsGovernanceDagBlockValidationOptions,
): SorafsValidationOutcome;

export function validateGovernanceDagHeadChain(
  headBytes: SorafsReferenceBytesInput,
  blocks: ReadonlyArray<SorafsGovernanceDagBlockInput>,
  options?: SorafsGovernanceDagHeadValidationOptions,
): SorafsValidationOutcome;

export interface SorafsGatewayProviderSpec {
  name: string;
  providerIdHex: string;
  /** Canonical lowercase Ed25519 public key used to verify the provider stream token. */
  gatewayPublicKeyHex: string;
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
  /** Expected cache version advertised by successful gateway responses. */
  cacheVersion?: string;
  clientId?: string;
  telemetryRegion?: string;
  rolloutPhase?: "canary" | "ramp" | "default";
  maxPeers?: number;
  retryBudget?: number;
  transportPolicy?: "soranet-first" | "soranet-strict" | "direct-only";
  anonymityPolicy?: "anon-guard-pq" | "anon-majority-pq" | "anon-strict-pq";
  writeMode?: "read-only" | "upload-pq-only";
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
  | "invalid_plan"
  | "no_providers"
  | "no_healthy_providers"
  | "no_compatible_providers"
  | "no_policy_eligible_providers"
  | "exhausted_retries"
  | "observer_failed"
  | "internal_invariant"
  | "unknown";

export interface SorafsGatewayFetchAttemptFailure {
  kind: "provider" | "invalid_chunk";
  message?: string;
  reason?: Record<string, unknown>;
  policyBlock?: {
    observedStatus: 451;
    code: "gateway_compliance_denied";
    source: "baseline" | "legal_safety_hold";
    catalogDigestHex: string;
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

export class ToriiDataModelMismatchError extends Error {
  readonly expected: unknown;
  readonly actual: unknown | null;
  readonly cause?: unknown;
  constructor(expected: unknown, actual: unknown, cause?: unknown);
}

export interface IdentifierRequestForPolicyOptions {
  input?: unknown;
  encryptedInput?: string;
  encrypt?: boolean;
  seed?: BinaryLike;
  seedHex?: string;
  outputOpening: RamLfeOutputOpening;
}

export interface IdentifierRequestForPolicy {
  policyId: string;
  encryptedInput: string;
  outputOpening: RamLfeOutputOpening;
}

export function encodeIdentifierResolutionReceiptPayload(payload: unknown): Buffer;
export function encodeIdentifierResolutionReceiptAttestation(
  attestation: unknown,
): Buffer;
export function getIdentifierBfvPublicParameters(
  policySummary: IdentifierPolicyClientSummary,
): Readonly<IdentifierBfvPublicParameters> | null;
export function encryptIdentifierInputForPolicy(
  policySummary: IdentifierPolicyClientSummary,
  input: unknown,
  options?: { seed?: BinaryLike; seedHex?: string },
): string;
export function hashIdentifierEncryptedInput(encryptedInput: string): string;
export function buildIdentifierRequestForPolicy(
  policySummary: IdentifierPolicyClientSummary,
  options: IdentifierRequestForPolicyOptions,
): IdentifierRequestForPolicy;
export function verifyIdentifierResolutionReceipt(
  receipt: IdentifierResolutionReceipt,
  policySummary: IdentifierPolicyClientSummary,
): boolean;

type IrohaJsPublicApi = typeof import("./index.js");
type IrohaJsRuntimeNamespace<Keys extends keyof IrohaJsPublicApi> = Readonly<
  Pick<IrohaJsPublicApi, Keys>
>;

type ToriiRuntimeNamespaceExport =
    "IsoMessageTimeoutError"
  | "LocalSigningContext"
  | "OperatorSigningContext"
  | "ToriiClient"
  | "TransactionBatchAdmissionAmbiguousError"
  | "SorafsOrderbookSubmissionAmbiguousError"
  | "ToriiDataModelMismatchError"
  | "ToriiHttpError"
  | "TransactionStatusError"
  | "TransactionTimeoutError"
  | "buildConnectWebSocketUrl"
  | "buildIdentifierRequestForPolicy"
  | "buildSorafsOrderbookEventsWebSocketUrl"
  | "decodePdpCommitmentHeader"
  | "encodeIdentifierResolutionReceiptAttestation"
  | "encodeIdentifierResolutionReceiptPayload"
  | "encryptIdentifierInputForPolicy"
  | "hashIdentifierEncryptedInput"
  | "extractPipelineStatusKind"
  | "getIdentifierBfvPublicParameters"
  | "isStatusQueueStalled"
  | "openConnectWebSocket"
  | "openSorafsOrderbookEventsWebSocket"
  | "statusLivenessElapsedMs"
  | "verifyIdentifierResolutionReceipt";

type NoritoRuntimeNamespaceExport =
    "decodeCancelAssetLockV1"
  | "encodeAccountIdNoritoValue"
  | "encodeAssetDefinitionIdNoritoValue"
  | "encodeCancelAssetLockV1"
  | "encodeQuantityNoritoValue"
  | "inspectSubscriptionTriggerAction"
  | "noritoDecodeBlockProofs"
  | "noritoDecodeInstruction"
  | "noritoDecodeInstructionBoxArchive"
  | "noritoDecodeOpenVerifyEnvelope"
  | "noritoDecodePrivacyExact12FixtureBundleBase64V1"
  | "noritoDecodePrivacyExact12FixtureBundleV1"
  | "noritoEncodeInstruction"
  | "noritoEncodeInstructionBoxArchive"
  | "noritoEncodeContractManifestSignaturePayload"
  | "noritoEncodeMultisigContractCallApproveRequest"
  | "noritoEncodeMultisigContractCallProposeRequest"
  | "noritoEncodeMultisigProposeRequest"
  | "noritoEncodeSorafsBillingAcknowledgementProofV1"
  | "noritoEncodeOpenVerifyEnvelope"
  | "noritoEncodePrivacyExact12FixtureBundleV1"
  | "noritoEncodeTransactionPayloadBatch"
  | "PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1"
  | "PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_NAME_V1"
  | "PRIVACY_EXACT12_PROTOCOL_IDS_V1"
  | "SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1"
  | "SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1"
  | "validateNoritoFrame"
  | "validateSorafsReplicationOrderPayloadV1"
  | "verifyBlockMerkleProof"
  | "verifyBlockProofs";

type CryptoRuntimeNamespaceExport =
    "CRYPTO_ALGORITHMS"
  | "PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES"
  | "PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1"
  | "PRIVACY_REQUIRED_BRIDGE_ABI_VERSION"
  | "SM2_DEFAULT_DISTINGUISHED_ID"
  | "SM2_PRIVATE_KEY_LENGTH"
  | "SM2_PUBLIC_KEY_LENGTH"
  | "SM2_SIGNATURE_LENGTH"
  | "SUPPORTED_CRYPTO_ALGORITHMS"
  | "buildKaigiRosterJoinProof"
  | "deriveConfidentialDiversifierV2"
  | "deriveConfidentialKeyset"
  | "deriveConfidentialKeysetFromHex"
  | "deriveConfidentialNoteV2"
  | "deriveConfidentialNullifierV2"
  | "deriveConfidentialOwnerTagV2"
  | "deriveConfidentialReceiveAddressV2"
  | "deriveEd25519SeedFromRecoveryPhrase"
  | "deriveSm2KeyPairFromSeed"
  | "ed25519SeedToRecoveryPhrase"
  | "entropyToRecoveryPhrase"
  | "generateKeyPair"
  | "generateRecoveryPhrase"
  | "generateSm2KeyPair"
  | "isPrivacyNativeAvailable"
  | "loadKeyPair"
  | "loadSm2KeyPair"
  | "normalizeCryptoAlgorithm"
  | "normalizeRecoveryPhrase"
  | "privacyCompiledProfileCatalogV1"
  | "privateKeyMultihash"
  | "publicKeyFromPrivate"
  | "publicKeyMultihash"
  | "recoveryPhraseToEntropy"
  | "sign"
  | "signEd25519"
  | "signSm2"
  | "sm2FixtureFromSeed"
  | "sm2PublicKeyMultihash"
  | "supportedCryptoAlgorithms"
  | "validateRecoveryPhrase"
  | "verify"
  | "verifyEd25519"
  | "verifySm2";

export const Torii: IrohaJsRuntimeNamespace<ToriiRuntimeNamespaceExport>;
export const Norito: IrohaJsRuntimeNamespace<NoritoRuntimeNamespaceExport>;
export const Crypto: IrohaJsRuntimeNamespace<CryptoRuntimeNamespaceExport>;
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
  ticketB64: string | null;
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
  signed?: boolean | null;
  timeoutMs?: number | null;
  headers?: Record<string, string | null | undefined>;
  signal?: AbortSignal;
}

export interface SoranetTokenMintOptions extends SoranetPuzzleMintOptions {
  flags?: number;
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
    transcriptHashHex: string,
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
  toriiClient?: ToriiClientRetryOptions;
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
/** Immutable NetworkId context required by APIs that return local-signing drafts. */
export class LocalSigningContext {
  constructor(networkId: NetworkId, chainDiscriminant?: number);
  readonly networkId: NetworkId; readonly chainDiscriminant: number;
}

export interface ToriiClientOptions extends ToriiClientRetryOptions {
  chain?: never;
  chainId?: never;
  chain_id?: never;
  networkId?: never;
  fetchImpl?: typeof fetch;
  config?: ToriiClientConfigSource;
  localSigningContext?: LocalSigningContext;
  operatorSigningContext?: OperatorSigningContext;
  canonicalRequestAuth?: CanonicalRequestAuth;
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

/** Polling controls for global, state-authoritative transaction finality. */
export interface TransactionStatusPollOptions {
  signal?: AbortSignal;
  intervalMs?: number;
  timeoutMs?: number | null;
  maxAttempts?: number | null;
  onStatus?: (
    status: string | null,
    payload: ToriiPipelineTransactionStatus | null,
    attempt: number,
  ) => void | Promise<void>;
}

/** Options for a single diagnostic status read; scope defaults to `global`. */
export interface TransactionStatusReadOptions {
  signal?: AbortSignal;
  scope?: "local" | "global";
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
  maxAmount: string;
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
  network_id: NetworkId;
  app_pk: string;
  nonce: string;
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
  networkId: NetworkId;
  node?: string | null;
  nonce?: BinaryLike | null;
  appKeyPair?: {
    publicKey: BinaryLike;
    privateKey: BinaryLike;
  };
}

export interface ConnectSessionPreview {
  networkId: NetworkId;
  node: string | null;
  sidBytes: Buffer;
  sidBase64Url: string;
  nonce: Buffer;
  appKeyPair: ConnectKeyPair;
  walletUri: string;
  appUri: string;
}

export function generateConnectSid(options: {
  networkId: NetworkId;
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
 * Exact protocol `u64` decoded from JSON.
 *
 * Values through `Number.MAX_SAFE_INTEGER` remain numbers; larger values are
 * returned as bigint so typed Sumeragi reads never round wire integers.
 */
export type ToriiU64 = number | bigint;

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
  local_amount: string;
  xor_due: string;
  xor_after_haircut: string;
  xor_variance: string;
  timestamp_ms: ToriiU64;
}

export type ToriiLaneLiquidityProfile = Readonly<{
  profile: "Tier1" | "Tier2" | "Tier3";
  state: null;
}>;

export type ToriiLaneVolatilityClass = Readonly<{
  bucket: "Stable" | "Elevated" | "Dislocated";
  state: null;
}>;

export interface ToriiLaneSwapMetadata {
  epsilon_bps: number;
  twap_window_seconds: number;
  liquidity_profile: ToriiLaneLiquidityProfile;
  twap_local_per_xor: string;
  volatility_class: ToriiLaneVolatilityClass;
}

export interface ToriiNexusFeeScheduleInputs {
  tx_bytes_len: ToriiU64;
  instruction_count: ToriiU64;
  gas_used: ToriiU64;
  base_fee: string;
  per_byte_fee: string;
  per_instruction_fee: string;
  per_gas_unit_fee: string;
}

export interface ToriiNexusFeeReceipt {
  version: 1;
  source_id: string;
  dataspace_id: ToriiU64;
  lane_id: number;
  block_height: ToriiU64;
  payer_account_id: string;
  fee_asset_id: string;
  fee_amount: string;
  schedule: Readonly<ToriiNexusFeeScheduleInputs>;
}

export type ToriiNativeAmxPhase = Readonly<{
  phase: "prepare" | "commit";
  detail: null;
}>;

declare const toriiNativeAmxSourceIdBrand: unique symbol;
declare const toriiNativeAmxTransactionEntrypointHashBrand: unique symbol;

/** Exact uppercase raw 32-byte source identity. */
export type ToriiNativeAmxSourceId = string & {
  readonly [toriiNativeAmxSourceIdBrand]: "ToriiNativeAmxSourceId";
};

/** Canonical Iroha transaction-entrypoint hash, distinct from a source ID. */
export type ToriiNativeAmxTransactionEntrypointHash = string & {
  readonly [toriiNativeAmxTransactionEntrypointHashBrand]:
    "ToriiNativeAmxTransactionEntrypointHash";
};

export interface ToriiNativeAmxAttestationBody {
  round: ToriiSumeragiV2ConsensusRound;
  epoch: ToriiU64;
  network_id: string;
  source_id: ToriiNativeAmxSourceId;
  tx_entrypoint_hash: ToriiNativeAmxTransactionEntrypointHash;
  plan_digest: string;
  phase: ToriiNativeAmxPhase;
  coordinator_lane_id: number;
  coordinator_dataspace_id: ToriiU64;
  coordinator_lane_incarnation: string;
  participant_lane_id: number;
  participant_dataspace_id: ToriiU64;
  participant_lane_incarnation: string;
  participant_previous_block_height: ToriiU64;
  participant_previous_block_descriptor_hash: string | null;
  participant_lane_block_height: ToriiU64;
  participant_lane_block_view: ToriiU64;
  participant_proposal_hash: string;
  participant_settlement_commitment: string;
  participant_validator_set_hash: string;
  participant_validator_count: number;
  participant_min_quorum: number;
  authority_context_height: ToriiU64;
  planned_coordinator_block_height: ToriiU64;
  coordinator_lane_block_view: ToriiU64;
  coordinator_proposal_hash: string;
}

export interface ToriiNativeAmxAttestationQc {
  body: Readonly<ToriiNativeAmxAttestationBody>;
  validator_set_hash_version: 1;
  validator_set_hash: string;
  validator_set: ReadonlyArray<string>;
  validator_set_pops: ReadonlyArray<ReadonlyArray<number>>;
  signers_bitmap: ReadonlyArray<number>;
  bls_aggregate_signature: ReadonlyArray<number>;
}

export interface ToriiNativeAmxParticipantLaneBlockDescriptor {
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  proposal_height: ToriiU64;
  previous_lane_block_height: ToriiU64;
  previous_lane_block_descriptor_hash?: string;
  lane_block_height: ToriiU64;
  lane_block_view: ToriiU64;
  subject_hash: string;
  payload_ownership_hash: string;
  rbc_instance_hash: string;
  accepted_candidate_indices: ReadonlyArray<ToriiU64>;
  accepted_transaction_hashes: ReadonlyArray<string>;
  validator_set_hash_version: 1;
  validator_set_hash: string;
  validator_set: ReadonlyArray<string>;
  validator_count: number;
  min_quorum: number;
  qc_mode_tag: string;
  descriptor_hash: string;
}

export interface ToriiNativeAmxParticipantLaneBlockProposal {
  descriptor: Readonly<ToriiNativeAmxParticipantLaneBlockDescriptor>;
  proposal_hash: string;
  /** Required control-only marker; Native AMX payload recovery hints are forbidden. */
  payload_block_hint: null;
}

export interface ToriiNativeAmxLeg {
  lane_id: number;
  dataspace_id: ToriiU64;
  participant_proposal: Readonly<ToriiNativeAmxParticipantLaneBlockProposal>;
  participant_settlement: Readonly<ToriiLaneSettlementCommitment>;
  participant_settlement_hash: string;
  prepare_qc: Readonly<ToriiNativeAmxAttestationQc>;
  commit_qc: Readonly<ToriiNativeAmxAttestationQc>;
  /** Full block validation must establish the mixed-role coordinator anchor. */
  readonly requires_mixed_role_anchor_validation: boolean;
}

export interface ToriiNativeAmxReceipt {
  version: 2;
  source_id: ToriiNativeAmxSourceId;
  network_id: string;
  plan_digest: string;
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  authority_context_height: ToriiU64;
  lane_block_height: ToriiU64;
  lane_block_view: ToriiU64;
  coordinator_proposal_hash: string;
  legs: ReadonlyArray<Readonly<ToriiNativeAmxLeg>>;
}

export interface ToriiLaneSettlementCommitment {
  block_height: ToriiU64;
  lane_id: number;
  lane_incarnation: string;
  dataspace_id: ToriiU64;
  tx_count: ToriiU64;
  total_local_amount: string;
  total_xor_due: string;
  total_xor_after_haircut: string;
  total_xor_variance: string;
  swap_metadata: ToriiLaneSwapMetadata | null;
  receipts: ReadonlyArray<ToriiLaneSettlementReceipt>;
  nexus_fee_receipts: ReadonlyArray<Readonly<ToriiNexusFeeReceipt>>;
  native_amx_receipts: ReadonlyArray<Readonly<ToriiNativeAmxReceipt>>;
}

export interface ToriiLaneFastpqProofMaterial {
  proof_digest: string;
  verified_at_height: ToriiU64 | null;
}

export interface ToriiLaneRelayEnvelope {
  lane_id: number;
  lane_incarnation: string;
  dataspace_id: ToriiU64;
  block_height: ToriiU64;
  block_header: Record<string, unknown>;
  qc: Record<string, unknown> | null;
  da_commitment_hash: string | null;
  lane_block_descriptor_hash: string | null;
  settlement_commitment: ToriiLaneSettlementCommitment;
  settlement_hash: string;
  rbc_bytes_total: ToriiU64;
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

export interface ToriiLanePrivacyCommitmentSnapshot {
  id: number;
  scheme: "merkle";
  merkle: ToriiLaneMerkleCommitmentSnapshot;
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
  rejected: number;
  enacted: number;
  superseded: number;
  execution_failed: number;
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
  | "Rejected"
  | "Enacted"
  | "Superseded"
  | "ExecutionFailed";

export interface ToriiGovernanceDeployContractProposal {
  contract_address: string;
  code_hash: string;
  abi_hash: string;
  abi_version: 1;
  manifest_provenance: ToriiGovernanceManifestProvenance | null;
}

export interface ToriiGovernanceManifestProvenance {
  signer: string;
  signature: string;
}

export interface ToriiGovernanceRuntimeUpgradeSbomDigest {
  algorithm: string;
  digest: string;
}

export interface ToriiGovernanceRuntimeUpgradeManifest {
  name: string;
  description: string;
  abi_version: 1;
  abi_hash: ReadonlyArray<number>;
  added_syscalls: readonly [];
  added_pointer_types: readonly [];
  start_height: number;
  end_height: number;
  sbom_digests: ReadonlyArray<ToriiGovernanceRuntimeUpgradeSbomDigest>;
  slsa_attestation: string;
  provenance: ReadonlyArray<ToriiGovernanceManifestProvenance>;
}

export interface ToriiGovernanceRuntimeUpgradeProposal {
  manifest: ToriiGovernanceRuntimeUpgradeManifest;
}

export interface ToriiGovernanceSccpRouteProposal {
  anchor: {
    network_id: string;
    action: Readonly<SccpRouteGovernanceActionInput>;
  };
}

export type ToriiGovernanceValidationFeeChargingMode = Readonly<
  | { charging_mode: "DISABLED"; value: null }
  | { charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION"; value: null }
>;

export interface ToriiGovernanceValidationFeePayoutRecipient {
  account_id: string;
  share: "0.25";
}

export interface ToriiGovernanceValidationFeePayoutBinding {
  contract_address: string;
  code_hash: ReadonlyArray<number>;
  entrypoint: "autonomous_validation_fee_tick";
  treasury_account_id: string;
  ds_asset_id: string;
  xor_asset_id: string;
  pool_vault_account_id: string;
  batch_ds: "10";
  min_xor_out: "4";
  max_xor_out: "100";
  recipients: ReadonlyArray<ToriiGovernanceValidationFeePayoutRecipient>;
}

export interface ToriiGovernanceValidationFeePolicyV1 {
  schema_version: 1;
  network_id: string;
  policy_version: string;
  previous_policy_hash: ReadonlyArray<number> | null;
  ds_asset_id: string;
  ds_scale: 2;
  fee: string;
  treasury_account_id: string;
  charging_mode: ToriiGovernanceValidationFeeChargingMode;
  effective_from_height: string;
  expires_after_height: string | null;
  exemption_classes: ReadonlyArray<"TREASURY_PAYOUT">;
  treasury_payout_binding: ToriiGovernanceValidationFeePayoutBinding | null;
}

export interface ToriiGovernanceValidationFeePolicyProposal {
  proposal_operator: string;
  policy: ToriiGovernanceValidationFeePolicyV1;
  payout_lifecycle_proposal_id: ReadonlyArray<number> | null;
}

export interface ToriiGovernanceValidationFeePayoutLifecycleProposal {
  proposal_operator: string;
  payout_binding: ToriiGovernanceValidationFeePayoutBinding;
}

export type ToriiGovernanceMusubiPackageScope = Readonly<
  | { kind: "DataspaceRoot"; value: null }
  | { kind: "Domain"; value: string }
>;

export interface ToriiGovernanceMusubiPackageId {
  home_dataspace: number;
  scope: ToriiGovernanceMusubiPackageScope;
  name: string;
}

export type ToriiGovernanceMusubiPrereleaseIdentifier = Readonly<
  | { kind: "Numeric"; value: number }
  | { kind: "AlphaNumeric"; value: string }
>;

export interface ToriiGovernanceMusubiVersion {
  major: number;
  minor: number;
  patch: number;
  prerelease: ReadonlyArray<ToriiGovernanceMusubiPrereleaseIdentifier>;
}

export interface ToriiGovernanceMusubiReleaseId {
  package: ToriiGovernanceMusubiPackageId;
  version: ToriiGovernanceMusubiVersion;
}

export interface ToriiGovernanceMusubiAliasPricingPolicy {
  revision: number;
  length_1_xor: number;
  length_2_xor: number;
  length_3_xor: number;
  length_4_xor: number;
  length_5_to_32_xor: number;
}

export interface ToriiGovernanceMusubiRegistryPolicy {
  version: 1;
  revision: number;
  mode: Readonly<
    | { kind: "Closed"; value: null }
    | { kind: "Allowlisted"; value: null }
    | { kind: "Open"; value: null }
  >;
  allowlisted_dataspaces: ReadonlyArray<number>;
  alias_pricing: ToriiGovernanceMusubiAliasPricingPolicy;
}

export type ToriiGovernanceMusubiAction = Readonly<
  | {
      kind: "RecoverPackageOwners";
      value: {
        package: ToriiGovernanceMusubiPackageId;
        owners: ReadonlyArray<string>;
        expected_revision: number;
      };
    }
  | {
      kind: "RetargetAlias";
      value: {
        alias: string;
        target: ToriiGovernanceMusubiPackageId;
        expected_revision: number;
      };
    }
  | {
      kind: "TakedownArtifact";
      value: {
        release: ToriiGovernanceMusubiReleaseId;
        reason: string;
        expected_artifact_governance_revision: number;
      };
    }
  | {
      kind: "SetRegistryPolicy";
      value: {
        policy: ToriiGovernanceMusubiRegistryPolicy;
        expected_revision: number;
      };
    }
>;

export type ToriiGovernanceSorafsProviderAction = Readonly<
  | {
      action: "establish";
      value: { provider_id: ReadonlyArray<number>; owner: string };
    }
  | {
      action: "rebind";
      value: {
        provider_id: ReadonlyArray<number>;
        expected_owner: string;
        next_owner: string;
      };
    }
  | {
      action: "remove";
      value: { provider_id: ReadonlyArray<number>; expected_owner: string };
    }
>;

export interface ToriiGovernanceSorafsProviderProposal {
  action: ToriiGovernanceSorafsProviderAction;
}

export interface ToriiGovernanceContractResponse {
  found: boolean;
  contract_address: string;
  dataspace: string | null;
  code_hash_hex: string | null;
}

export type ToriiGovernanceProposalKind =
  | Readonly<{
      variant: "DeployContract";
      deploy_contract: ToriiGovernanceDeployContractProposal;
    }>
  | Readonly<{
      variant: "RuntimeUpgrade";
      runtime_upgrade: ToriiGovernanceRuntimeUpgradeProposal;
    }>
  | Readonly<{
      variant: "SccpRouteGovernance";
      sccp_route_governance: ToriiGovernanceSccpRouteProposal;
    }>
  | Readonly<{
      variant: "ValidationFeePolicy";
      validation_fee_policy: ToriiGovernanceValidationFeePolicyProposal;
    }>
  | Readonly<{
      variant: "ValidationFeePayoutLifecycle";
      validation_fee_payout_lifecycle: ToriiGovernanceValidationFeePayoutLifecycleProposal;
    }>
  | Readonly<{
      variant: "MusubiRegistryGovernance";
      musubi_registry_governance: ToriiGovernanceMusubiAction;
    }>
  | Readonly<{
      variant: "SorafsProviderGovernance";
      sorafs_provider_governance: ToriiGovernanceSorafsProviderProposal;
    }>;

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

export interface ToriiGovernanceLockCustody {
  escrowed: boolean;
  asset_definition_id: string;
  bond_escrow_account: string;
  slash_receiver_account: string;
}

export interface ToriiGovernanceLockRecord {
  owner: string;
  amount: string;
  slashed: string;
  expiry_height: number;
  direction: number;
  duration_blocks: number;
  custody: ToriiGovernanceLockCustody | null;
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

export interface ToriiGovernanceCouncilMember {
  account_id: string;
}

export interface ToriiGovernanceCouncilCurrentResponse {
  epoch: number;
  members: ReadonlyArray<ToriiGovernanceCouncilMember>;
  alternates: ReadonlyArray<ToriiGovernanceCouncilMember>;
  candidate_count: number;
  derived_by: "Sortition" | "Manual";
}

export interface ToriiProtectedNamespacesApplyResponse {
  ok: boolean;
  applied: number;
}

export interface ToriiProtectedNamespacesGetResponse {
  found: boolean;
  namespaces: string[];
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

export interface ToriiGovernanceProposalInstructionDraftV1 {
  wire_id: string;
  payload_hex: string;
}

export interface ToriiGovernanceProposalDraftResponseV1 {
  proposal_id: string;
  tx_instructions: readonly [ToriiGovernanceProposalInstructionDraftV1];
}

export const PARLIAMENT_API_VERSION_V1: 1;
export const PARLIAMENT_ATTEMPT_DRAFT_PATH_V1: "/v1/gov/parliament/attempts/draft";
export const PARLIAMENT_ATTEMPT_READ_PATH_V1: "/v1/gov/parliament/attempts/{governance_attempt_id}";
export const PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_READ_PATH_V1: "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-context";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_PATH_V1: "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1: 1;
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_SCHEMA_NAME_V1: "iroha.torii.v1.parliament.timed_ovn_casting_proof.request";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_SCHEMA_HASH_HEX_V1: "adccf322a5fcf43040e20bea238f55f3";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_NAME_V1: "iroha.torii.v1.parliament.timed_ovn_casting_proof.response";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_HASH_HEX_V1: "46d29299272433b1299646bee722bd11";
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_FLAGS_V1: 2;
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_PAYLOAD_ALIGNMENT_V1: 8;
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_PADDING_BYTES_V1: 0;
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_REQUEST_BYTES_V1: 52;
export const PARLIAMENT_TLE_RELEASE_CONTEXT_READ_PATH_V1: "/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context";
export const PARLIAMENT_TLE_PARTIAL_RELEASE_PATH_V1: "/v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release";
export const PARLIAMENT_TRANSITION_DRAFT_PATH_V1: "/v1/gov/parliament/transitions/draft";
export const PARLIAMENT_ATTEMPT_CREATE_WIRE_ID_V1: "iroha.governance.parliament.attempt.create.v1";
export const PARLIAMENT_TRANSITION_SUBMIT_WIRE_ID_V1: "iroha.governance.parliament.transition.submit.v1";
export const PARLIAMENT_ATTEMPT_STATE_MAX_BYTES_V1: 16777216;
export const PARLIAMENT_GOVERNANCE_ATTEMPT_SEQUENCE_MAX_V1: 16;
export const PARLIAMENT_TIMED_OVN_REGISTRATION_RECORD_BYTES_V1: 3624;
export const PARLIAMENT_TIMED_OVN_BALLOT_RECORD_BYTES_V1: 2858;
export const PARLIAMENT_TIMED_OVN_BALLOT_CHUNK_MAX_RECORDS_V1: 32;
export const PARLIAMENT_TIMED_OVN_CORPUS_ENTRIES_V1: 1000;
export const PARLIAMENT_TLE_MAX_COMMITTEE_SIZE_V1: 31;
export const PARLIAMENT_TIMED_OVN_CASTING_CONTEXT_ARCHIVE_MAX_BYTES_V1: 4194304;
export const PARLIAMENT_TIMED_OVN_CASTING_PROOF_RESPONSE_MAX_BYTES_V1: 8388608;

export type ParliamentProposalTagV1 =
  | "DeployContract"
  | "RuntimeUpgrade"
  | "SccpRouteGovernance"
  | "ValidationFeePolicy"
  | "ValidationFeePayoutLifecycle"
  | "MusubiRegistryGovernance"
  | "SorafsProviderGovernance";

export type ParliamentPublicTransitionTagV1 =
  | "EscalateRisk"
  | "CompleteQualification"
  | "RegisterSortitionRequest"
  | "ConsumeSortitionPulseBatch"
  | "BeginInvitationAcceptance"
  | "FailBodyElectionNoRoster"
  | "SealBodyRoster"
  | "AdvanceBodyPhase"
  | "RecordAttemptAbsence"
  | "EndorsePublicFinding"
  | "RegisterBallotAttempt"
  | "CloseBallotRegistration"
  | "FreezeBallotSurvivors"
  | "FreezeTimedOvnCorpus"
  | "BeginBallotOpeningBatch"
  | "FailBallotNoResult"
  | "FinalizeOpenedBallot"
  | "RecordInvitationResponse"
  | "RegisterBallotParticipant"
  | "RecordBallotDropout"
  | "FailPublicFindingNoResult";

export interface ParliamentTransitionLayoutV1 {
  readonly noritoIndex: number;
  readonly jsonTag: ParliamentPublicTransitionTagV1;
  readonly jsonPayloadRequired: boolean;
  readonly eventKindIndex: number;
}

export interface ParliamentAutomaticExecutionOutcomeLayoutV1 {
  readonly noritoIndex: number;
  readonly jsonTag: "Enacted" | "Superseded" | "ExecutionFailed";
  readonly jsonPayloadRequired: boolean;
  readonly eventKind: "MarkEnacted" | "MarkSuperseded" | "MarkExecutionFailed";
  readonly eventKindIndex: 17 | 18 | 19;
}

export type ParliamentNoResultKindTagV1 =
  | "PublicFindingQuorumUnreachable"
  | "PublicFindingDeadlineExpired"
  | "BallotRegistrationDeadlineExpired"
  | "BallotSurvivorDeadlineExpired"
  | "BallotCommitmentDeadlineExpired"
  | "BallotReleasePulseUnavailable"
  | "BallotOpeningDeadlineExpired"
  | "SortitionRetriesExhausted";

export interface ParliamentNoResultKindLayoutV1 {
  readonly noritoIndex: number;
  readonly jsonTag: ParliamentNoResultKindTagV1;
}

export type ParliamentBodyNameV1 =
  | "rules-committee"
  | "agenda-council"
  | "interest-panel"
  | "review-panel"
  | "coordination-council"
  | "mpc-committee"
  | "fma-committee"
  | "oversight-committee"
  | "policy-jury"
  | "confirmation-jury";

export const PARLIAMENT_PUBLIC_TRANSITIONS_V1: ReadonlyArray<ParliamentTransitionLayoutV1>;
/** Read/audit-only inventory; these tags are never accepted by the public builder. */
export const PARLIAMENT_AUTOMATIC_EXECUTION_OUTCOMES_V1: ReadonlyArray<ParliamentAutomaticExecutionOutcomeLayoutV1>;
export const PARLIAMENT_NO_RESULT_KINDS_V1: ReadonlyArray<ParliamentNoResultKindLayoutV1>;
/** Canonical presentation order for first-release Parliament bodies. */
export const PARLIAMENT_CANONICAL_BODY_ORDER_V1: ReadonlyArray<ParliamentBodyNameV1>;
export const PARLIAMENT_BODY_STATE_FIELDS_V1: ReadonlyArray<
  | "body"
  | "body_instance_id"
  | "status"
  | "public_finding_opened_at_height"
  | "public_finding_phase_blocks"
  | "public_finding_deadline_height"
  | "no_result_kind"
  | "no_result_height"
  | "timed_ovn_progress"
>;
export const PARLIAMENT_CERTIFICATE_BODY_BINDING_FIELDS_V1: ReadonlyArray<string>;
export const PARLIAMENT_PUBLIC_FINDING_CERTIFICATE_FIELDS_V1: ReadonlyArray<
  "endorsement_root" | "endorsing_assignments" | "endorsements" | "quorum"
>;

export interface ParliamentMusubiPackageIdV1 {
  home_dataspace: number;
  scope: ToriiGovernanceMusubiPackageScope;
  /** Exact Norito JSON tuple encoding of `MusubiPackageNameV1`. */
  name: readonly [string];
}

export interface ParliamentMusubiReleaseIdV1 {
  package: ParliamentMusubiPackageIdV1;
  version: ToriiGovernanceMusubiVersion;
}

export type ParliamentMusubiActionV1 = Readonly<
  | {
      kind: "RecoverPackageOwners";
      value: {
        package: ParliamentMusubiPackageIdV1;
        owners: ReadonlyArray<string>;
        expected_revision: number;
      };
    }
  | {
      kind: "RetargetAlias";
      value: {
        /** Exact Norito JSON tuple encoding of `MusubiAliasNameV1`. */
        alias: readonly [string];
        target: ParliamentMusubiPackageIdV1;
        expected_revision: number;
      };
    }
  | {
      kind: "TakedownArtifact";
      value: {
        release: ParliamentMusubiReleaseIdV1;
        /** Exact Norito JSON tuple encoding of `MusubiReasonV1`. */
        reason: readonly [string];
        expected_artifact_governance_revision: number;
      };
    }
  | {
      kind: "SetRegistryPolicy";
      value: {
        policy: ToriiGovernanceMusubiRegistryPolicy;
        expected_revision: number;
      };
    }
>;

export type ParliamentSorafsProviderActionV1 = Readonly<
  | {
      action: "establish";
      value: {
        /** Exact Norito JSON tuple encoding of `ProviderId`. */
        provider_id: readonly [ReadonlyArray<number>];
        owner: string;
      };
    }
  | {
      action: "rebind";
      value: {
        provider_id: readonly [ReadonlyArray<number>];
        expected_owner: string;
        next_owner: string;
      };
    }
  | {
      action: "remove";
      value: {
        provider_id: readonly [ReadonlyArray<number>];
        expected_owner: string;
      };
    }
>;

export type ParliamentProposalV1 =
  | Readonly<{
      kind: "DeployContract";
      payload: ToriiGovernanceDeployContractProposal;
    }>
  | Readonly<{
      kind: "RuntimeUpgrade";
      payload: ToriiGovernanceRuntimeUpgradeProposal;
    }>
  | Readonly<{
      kind: "SccpRouteGovernance";
      payload: ToriiGovernanceSccpRouteProposal;
    }>
  | Readonly<{
      kind: "ValidationFeePolicy";
      payload: ToriiGovernanceValidationFeePolicyProposal;
    }>
  | Readonly<{
      kind: "ValidationFeePayoutLifecycle";
      payload: ToriiGovernanceValidationFeePayoutLifecycleProposal;
    }>
  | Readonly<{
      kind: "MusubiRegistryGovernance";
      payload: ParliamentMusubiActionV1;
    }>
  | Readonly<{
      kind: "SorafsProviderGovernance";
      payload: { action: ParliamentSorafsProviderActionV1 };
    }>;

export type ParliamentLifecycleTransitionV1 =
  | { transition: "CompleteQualification" }
  | {
      transition: Exclude<ParliamentPublicTransitionTagV1, "CompleteQualification">;
      payload: Record<string, unknown>;
    };

export interface ParliamentAttemptDraftRequestV1 {
  version: 1;
  proposal: ParliamentProposalV1;
  attempt_sequence: number;
}

export interface ParliamentTransitionDraftRequestV1 {
  version: 1;
  governance_attempt_id: string;
  transition: ParliamentLifecycleTransitionV1;
}

export interface ParliamentInstructionDraftV1 {
  wire_id: string;
  payload_hex: string;
}

export interface ParliamentAttemptDraftResponseV1 {
  version: 1;
  proposal_content_id: string;
  governance_attempt_id: string;
  tx_instructions: readonly [ParliamentInstructionDraftV1];
}

export interface ParliamentTransitionDraftResponseV1 {
  version: 1;
  governance_attempt_id: string;
  transition_kind: { kind: ParliamentPublicTransitionTagV1 };
  transition_digest: ReadonlyArray<number>;
  tx_instructions: readonly [ParliamentInstructionDraftV1];
}

export interface ParliamentAttemptReadResponseV1 extends Record<string, unknown> {
  version: 1;
  current_height: number | bigint;
  attempt: Record<string, unknown> & { id: string };
  policy_version: number | bigint;
  required_bodies: ReadonlyArray<{
    body: ParliamentBodyNameV1;
    decision_mode: { mode: "PublicFinding" | "HiddenBindingBallot" };
  }>;
  body_states: ReadonlyArray<ParliamentBodyStateProjectionV1>;
  certificate: (Record<string, unknown> & {
    body_bindings: ReadonlyArray<Record<string, unknown> & { body: ParliamentBodyNameV1 }>;
  }) | null;
  terminal_height: number | bigint | null;
  execution_failure_root: ReadonlyArray<number> | null;
  superseding_head: Record<string, unknown> | null;
  state_payload_hex: string;
}

export interface ParliamentBodyStateProjectionV1 {
  body: string;
  body_instance_id: string | null;
  status: Record<string, unknown> | null;
  public_finding_opened_at_height: number | bigint | null;
  public_finding_phase_blocks: number | bigint | null;
  public_finding_deadline_height: number | bigint | null;
  no_result_kind: { reason: ParliamentNoResultKindTagV1 } | null;
  no_result_height: number | bigint | null;
  timed_ovn_progress: ParliamentTimedOvnProgressProjectionV1 | null;
}

export type ParliamentBallotAttemptStatusTagV1 =
  | "Registration"
  | "SurvivorFreeze"
  | "TimedCommitment"
  | "AwaitingRelease"
  | "Opening"
  | "Finalized"
  | "NoResult"
  | "Superseded";

/** Aggregate-only next-offset projection; contains no ballot or participant evidence. */
export interface ParliamentTimedOvnProgressProjectionV1 {
  ballot_attempt_id: string;
  status: { status: ParliamentBallotAttemptStatusTagV1 };
  frozen_survivor_count: number | null;
  accepted_ballot_prefix_count: number | null;
}

export interface ParliamentTleAdaptiveDealerCommitmentV1 {
  dealer_index: number;
  coefficient_commitments: ReadonlyArray<ReadonlyArray<number>>;
  constant_pok_commitment: ReadonlyArray<number>;
  constant_pok_response: ReadonlyArray<number>;
}

export interface ParliamentTleAdaptivePublicShareV1 {
  index: number;
  participant_hash: ReadonlyArray<number>;
  public_key_share: ReadonlyArray<number>;
}

/** Complete public transcript required for independent adaptive-partial verification. */
export interface ParliamentTleKeySessionPublicStateV1 {
  version: 1;
  key_session_id: string;
  network_id: ReadonlyArray<number>;
  roster_hash: ReadonlyArray<number>;
  committee_size: number;
  threshold: number;
  generator_h: ReadonlyArray<number>;
  generator_v: ReadonlyArray<number>;
  qualified_dealers: ReadonlyArray<number>;
  qualified_dealer_commitments: ReadonlyArray<ParliamentTleAdaptiveDealerCommitmentV1>;
  dkg_event_hash: ReadonlyArray<number>;
  group_public_key: ReadonlyArray<number>;
  public_shares: ReadonlyArray<ParliamentTleAdaptivePublicShareV1>;
  transcript_hash: ReadonlyArray<number>;
}

export interface ParliamentTimedOvnReleaseIdentityProjectionV1 {
  tle_key_session_id: string;
  governance_attempt_id: string;
  body_instance_id: string;
  ballot_attempt_id: string;
  survivor_corpus_root: ReadonlyArray<number>;
  no_recovery_root: ReadonlyArray<number>;
  target_finalized_height: number | bigint;
  parameter_hash: ReadonlyArray<number>;
}

export type ParliamentTimedOvnCastingPhaseV1 =
  | "Registered"
  | "RegistrationClosed"
  | "SurvivorsFrozen";

export interface ParliamentTimedOvnSessionProjectionV1 {
  network_id: ReadonlyArray<number>;
  proposal_content_id: string;
  governance_attempt_id: string;
  body_instance_id: string;
  ballot_attempt_id: string;
  parameter_hash: ReadonlyArray<number>;
  tle_key_session_id: string;
  tle_key_transcript_hash: ReadonlyArray<number>;
  tle_master_public_key: ReadonlyArray<number>;
}

export interface ParliamentTimedOvnCastingContextResponseV1 {
  version: 1;
  current_height: number | bigint;
  phase: ParliamentTimedOvnCastingPhaseV1;
  session: ParliamentTimedOvnSessionProjectionV1;
  registration_opened_at_finalized_height: number | bigint;
  target_finalized_height: number | bigint;
  tle_key_session: ParliamentTleKeySessionPublicStateV1;
  registration_records_hex: ReadonlyArray<string>;
  survivor_participant_hashes: ReadonlyArray<ReadonlyArray<number>> | null;
  release_identity: ParliamentTimedOvnReleaseIdentityProjectionV1 | null;
  archive_norito_base64: string;
}

export interface ParliamentTleReleaseContextResponseV1 {
  version: 1;
  current_height: number | bigint;
  ballot_attempt_id: string;
  governance_attempt_id: string;
  body_instance_id: string;
  status: { status: "Opening" };
  release_height: number | bigint;
  opening_deadline_height: number | bigint;
  tle_key_session: ParliamentTleKeySessionPublicStateV1;
  release_identity: ParliamentTimedOvnReleaseIdentityProjectionV1;
  identity_digest: ReadonlyArray<number>;
  identity_payload_hex: string;
}

export interface ParliamentTlePartialReleaseShareV1 {
  key_session_id: string;
  identity_digest: ReadonlyArray<number>;
  participant_index: number;
  sigma: ReadonlyArray<number>;
  proof_x: ReadonlyArray<number>;
  proof_y: ReadonlyArray<number>;
  z_s: ReadonlyArray<number>;
  z_r: ReadonlyArray<number>;
  z_u: ReadonlyArray<number>;
}

export interface ParliamentAttemptDraftOptionsV1 extends RequiredCanonicalRequestOptions {
  expectedProposalContentId: string;
  expectedGovernanceAttemptId: string;
}

export interface ParliamentTransitionDraftOptionsV1 extends RequiredCanonicalRequestOptions {
  expectedTransitionDigest: BinaryLike;
}

export interface ParliamentTlePartialReleaseOptionsV1 extends RequiredCanonicalRequestOptions {
  expectedKeySessionId: string;
  expectedIdentityDigest: BinaryLike;
  committeeSize: number;
}

export function parliamentAttemptReadPathV1(governanceAttemptId: string): string;
export function parliamentTimedOvnCastingContextReadPathV1(ballotAttemptId: string): string;
export function parliamentTimedOvnCastingProofPathV1(ballotAttemptId: string): string;
export function encodeParliamentTimedOvnCastingProofRequestV1(
  trustedCheckpointHeight: number | bigint,
): Buffer;
export function validateParliamentTimedOvnCastingProofResponseFrameV1(
  value: Buffer | ArrayBuffer | ArrayBufferView,
): Buffer;
export function parliamentTleReleaseContextReadPathV1(ballotAttemptId: string): string;
export function parliamentTlePartialReleasePathV1(ballotAttemptId: string): string;
export function buildParliamentAttemptDraftRequestV1(
  proposal: ParliamentProposalV1,
  attemptSequence: number,
): ParliamentAttemptDraftRequestV1;
export function buildParliamentTransitionDraftRequestV1(
  governanceAttemptId: string,
  transition: ParliamentLifecycleTransitionV1,
): ParliamentTransitionDraftRequestV1;
export function normalizeParliamentAttemptDraftResponseV1(
  value: unknown,
  bindings: {
    expectedProposalContentId: string;
    expectedGovernanceAttemptId: string;
  },
): ParliamentAttemptDraftResponseV1;
export function normalizeParliamentTransitionDraftResponseV1(
  value: unknown,
  bindings: {
    expectedGovernanceAttemptId: string;
    expectedTransitionKind: ParliamentPublicTransitionTagV1;
    expectedTransitionDigest: BinaryLike;
  },
): ParliamentTransitionDraftResponseV1;
export function normalizeParliamentAttemptReadResponseV1(
  value: unknown,
  expectedGovernanceAttemptId: string,
): ParliamentAttemptReadResponseV1;
export function normalizeParliamentTimedOvnCastingContextResponseV1(
  value: unknown,
  expectedBallotAttemptId: string,
): ParliamentTimedOvnCastingContextResponseV1;
export function normalizeParliamentTleReleaseContextResponseV1(
  value: unknown,
  expectedBallotAttemptId: string,
): ParliamentTleReleaseContextResponseV1;
export function normalizeParliamentTlePartialReleaseShareV1(
  value: unknown,
  bindings: {
    expectedKeySessionId: string;
    expectedIdentityDigest: BinaryLike;
    committeeSize: number;
  },
): ParliamentTlePartialReleaseShareV1;

export interface MinistryAgendaProposalDraftRequest {
  proposal: MinistryAgendaProposalV1;
  authority: string;
}

export type MinistryAgendaProposalAction =
  | "add-to-denylist"
  | "remove-from-denylist"
  | "amend-policy";

export type MinistryAgendaProposalTag =
  | "csam"
  | "malware"
  | "fraud"
  | "harassment"
  | "impersonation"
  | "policy-escalation"
  | "terrorism"
  | "spam";

export type MinistryAgendaEvidenceKind =
  | "url"
  | "torii-case"
  | "sorafs-cid"
  | "attachment";

export interface MinistryAgendaProposalSummaryV1 {
  title: string;
  motivation: string;
  expected_impact: string;
}

export interface MinistryAgendaProposalTargetV1 {
  label: string;
  hash_family: string;
  hash_hex: string;
  reason: string;
}

export interface MinistryAgendaEvidenceAttachmentV1 {
  kind: MinistryAgendaEvidenceKind;
  uri: string;
  digest_blake3_hex?: string | null;
  description?: string | null;
}

export interface MinistryAgendaProposalSubmitterV1 {
  name: string;
  contact: string;
  organization?: string | null;
  pgp_fingerprint?: string | null;
}

export interface MinistryAgendaProposalV1 {
  version: 1;
  proposal_id: string;
  submitted_at_unix_ms: number | string | bigint;
  language: string;
  action: MinistryAgendaProposalAction;
  summary: MinistryAgendaProposalSummaryV1;
  tags?: ReadonlyArray<MinistryAgendaProposalTag>;
  targets: ReadonlyArray<MinistryAgendaProposalTargetV1>;
  evidence: ReadonlyArray<MinistryAgendaEvidenceAttachmentV1>;
  submitter: MinistryAgendaProposalSubmitterV1;
  duplicates?: ReadonlyArray<string>;
}

export interface MinistryAgendaProposalDraftResponse {
  ok: boolean;
  agenda_proposal_id: string;
  authority: string;
  tx_instructions: ReadonlyArray<ToriiGovernanceDraftInstruction>;
  signable_transaction_b64: string;
}

export interface MinistryAgendaProposalRecord {
  proposal: MinistryAgendaProposalV1;
  authority: string;
  submitted_tx_hash_hex: string;
  submitted_height: number;
}

export interface MinistryAgendaProposalGetResponse {
  found: boolean;
  record: MinistryAgendaProposalRecord | null;
}

export type ToriiGovernanceBallotDirection = "Aye" | "Nay" | "Abstain";

export interface ToriiGovernanceManifestProvenanceInput {
  signer: string;
  signature: string;
}

export interface ToriiGovernanceDeployContractProposalRequest {
  contractAddress?: string;
  contractAlias?: string;
  codeHash: string | BinaryLike;
  abiHash: string | BinaryLike;
  abiVersion?: 1;
  manifestProvenance?: ToriiGovernanceManifestProvenanceInput | null;
}

export interface ToriiGovernancePlainBallotRequest {
  authority: string;
  networkId: NetworkId;
  referendumId: string;
  owner: string;
  amount: QuantityInput;
  durationBlocks: number | string | bigint;
  direction: ToriiGovernanceBallotDirection;
}

export interface ToriiGovernanceZkBallotV1Request {
  authority: string;
  networkId: NetworkId;
  electionId: string;
  backend: string;
  envelope: BinaryLike | string;
  rootHint?: string | BinaryLike | null;
  owner?: string | null;
  amount?: QuantityInput | null;
  durationBlocks?: number | string | bigint | null;
  direction?: ToriiGovernanceBallotDirection | null;
  nullifier?: string | BinaryLike | null;
}

export interface ToriiGovernanceBallotProof {
  backend: string;
  envelopeBytes: BinaryLike | string;
  rootHint?: string | null;
  owner?: string | null;
  nullifier?: string | null;
  amount?: QuantityInput | null;
  durationBlocks?: number | string | bigint | null;
  direction?: ToriiGovernanceBallotDirection | null;
}

export interface ToriiGovernanceZkBallotProofRequest {
  authority: string;
  networkId: NetworkId;
  electionId: string;
  ballot: ToriiGovernanceBallotProof;
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
    signature_batch_max_ed25519: number;
    signature_batch_max_secp256k1: number;
    signature_batch_max_pqc: number;
    signature_batch_max_bls: number;
    overlay_max_instructions: number;
    ivm_max_cycles_upper_bound: number;
    ivm_admission_cycle_limit: number;
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
    sponsor_vault_custody_account_id: string;
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

export type ToriiSccpCapabilities = SccpCapabilities;
export type ToriiSccpRegistry = SccpRegistry;
export type ToriiSccpSoraOutboundMaterial = SccpSoraOutboundMaterialV1;
export type ToriiSccpRecentMessages = SccpRecentMessages;
export type ToriiBridgeProofSubmitPayload = SccpBridgeProofSubmitPayload;
export type ToriiBridgeMessageSubmitPayload = SccpBridgeMessageSubmitPayload;
export type ToriiSccpBridgeSubmitResponse = SccpBridgeSubmitResponse;

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

export type ToriiSumeragiV2HeightContextId = readonly [string];

export type ToriiSumeragiV2ConsensusMode = Readonly<{
  mode: "permissioned" | "npos";
  details: null;
}>;

export type ToriiSumeragiV2GlobalPhase = Readonly<{
  phase: "prepare" | "commit";
  details: null;
}>;

export type ToriiSumeragiV2StatusPhase = Readonly<{
  phase:
    | "awaiting_proposal"
    | "reconstructing_payload"
    | "validating_payload"
    | "prepare"
    | "commit"
    | "pending_apply";
  details: null;
}>;

export type ToriiSumeragiV2BodyState = Readonly<{
  state:
    | "missing"
    | "reconstructing"
    | "stored"
    | "validated"
    | "pending_apply"
    | "applied";
  details: null;
}>;

export interface ToriiSumeragiV2ConsensusRound {
  context_id: ToriiSumeragiV2HeightContextId;
  height: ToriiU64;
  view: ToriiU64;
}

export interface ToriiSumeragiV2BlockSubject {
  parent_block_hash: string | null;
  block_hash: string;
  payload_hash: string;
}

export interface ToriiSumeragiV2LaneFinalityManifestCommitment { root: string; leaf_count: number; }

export interface ToriiSumeragiV2MergeCarrierCommitment { version: 1; entry_hash: string; }

export interface ToriiSumeragiV2ExecutionCommitment {
  parent_state_root: string;
  post_state_root: string;
  ordinary_writes_root: string;
  topup_anchor_root: string | null;
  topup_anchor_count: number;
  native_amx_application_manifest_version: number;
  native_amx_application_manifest_root: string;
  native_amx_application_manifest_count: number;
  lane_finality_manifest: ToriiSumeragiV2LaneFinalityManifestCommitment | null;
  merge_carrier: ToriiSumeragiV2MergeCarrierCommitment | null;
  executed_block_wire_len: ToriiU64;
  executed_block_wire_hash: string;
}

export interface ToriiSumeragiV2QuorumCertificateRef {
  round: ToriiSumeragiV2ConsensusRound;
  proposal_round: ToriiSumeragiV2ConsensusRound;
  phase: ToriiSumeragiV2GlobalPhase;
  subject: ToriiSumeragiV2BlockSubject;
  execution_commitment: ToriiSumeragiV2ExecutionCommitment;
}

export interface ToriiSumeragiV2TimeoutCertificateRef {
  round: ToriiSumeragiV2ConsensusRound;
  highest_prepare_qc: ToriiSumeragiV2QuorumCertificateRef | null;
  certificate_hash: string;
}

export interface ToriiSumeragiV2HeightContextStatus {
  epoch: ToriiU64;
  epoch_end_height: ToriiU64;
  mode: ToriiSumeragiV2ConsensusMode;
  epoch_seed: ReadonlyArray<number>;
  validator_count: number;
  quorum: Readonly<{
    min_signers: number;
    total_power: ToriiU64;
  }>;
}

export interface ToriiSumeragiV2CommitQcStatus {
  certificate: ToriiSumeragiV2QuorumCertificateRef;
  validator_count: number;
  signer_count: number;
  min_signers: number;
  signed_power: ToriiU64;
  total_power: ToriiU64;
}

export interface ToriiSumeragiV2VoteQuorumStatus {
  round: ToriiSumeragiV2ConsensusRound;
  proposal_round: ToriiSumeragiV2ConsensusRound;
  subject: ToriiSumeragiV2BlockSubject;
  execution_commitment: ToriiSumeragiV2ExecutionCommitment;
  signer_count: number;
  signed_power: ToriiU64;
  min_signers: number;
  total_power: ToriiU64;
}

export interface ToriiSumeragiV2TimeoutQuorumStatus {
  round: ToriiSumeragiV2ConsensusRound;
  signer_count: number;
  signed_power: ToriiU64;
  min_signers: number;
  total_power: ToriiU64;
  certificate_formed: boolean;
}

export type ToriiSumeragiV2OutboundIntentKind = Readonly<{
  kind:
    | "proposal"
    | "prepare_vote"
    | "commit_vote"
    | "timeout_vote"
    | "prepare_qc"
    | "commit_qc"
    | "timeout_certificate";
  details: null;
}>;

export type ToriiSumeragiV2OutboundIntentStage = Readonly<{
  stage: "pending_persistence" | "pending_signature" | "queued" | "sent";
  details: null;
}>;

export interface ToriiSumeragiV2OutboundIntentStatus {
  kind: ToriiSumeragiV2OutboundIntentKind;
  round: ToriiSumeragiV2ConsensusRound;
  proposal_round: ToriiSumeragiV2ConsensusRound | null;
  subject: ToriiSumeragiV2BlockSubject | null;
  execution_commitment: ToriiSumeragiV2ExecutionCommitment | null;
  stage: ToriiSumeragiV2OutboundIntentStage;
}

export type ToriiSumeragiV2LocalWorkStage = Readonly<{
  stage: "idle" | "queued" | "running" | "complete";
  details: null;
}>;

export interface ToriiSumeragiV2WorkStatus {
  candidate: ToriiSumeragiV2LocalWorkStage;
  body_recovery: ToriiSumeragiV2LocalWorkStage;
  body_store: ToriiSumeragiV2LocalWorkStage;
  validation: ToriiSumeragiV2LocalWorkStage;
  application: ToriiSumeragiV2LocalWorkStage;
  successor_height: ToriiSumeragiV2LocalWorkStage;
}

export type ToriiSumeragiV2QueueKind = Readonly<{
  queue:
    | "ingress"
    | "deferred_normal"
    | "deferred_progress"
    | "deferred_completion"
    | "runtime_normal"
    | "runtime_progress"
    | "runtime_completion"
    | "effect_completion"
    | "network_ingress"
    | "effect_dispatch";
  details: null;
}>;

export interface ToriiSumeragiV2QueueLivenessStatus {
  queue: ToriiSumeragiV2QueueKind;
  depth: number;
  capacity: number;
  oldest_age_ms: ToriiU64 | null;
  /** Accumulated eligible dispatches skipped by this queue's oldest item. */
  service_debt: ToriiU64;
}

export type ToriiSumeragiV2ProgressTransition = Readonly<{
  transition:
    | "proposal_admitted"
    | "body_available"
    | "body_stored"
    | "body_validated"
    | "prepare_vote_admitted"
    | "commit_vote_admitted"
    | "timeout_vote_admitted"
    | "prepare_quorum"
    | "lock_installed"
    | "commit_quorum"
    | "timeout_certificate_installed"
    | "decision_persisted"
    | "applied"
    | "successor_height_activated"
    | "recovery_replayed";
  details: null;
}>;

export interface ToriiSumeragiV2ProgressTransitionStatus {
  generation: ToriiU64;
  round: ToriiSumeragiV2ConsensusRound;
  transition: ToriiSumeragiV2ProgressTransition;
  age_ms: ToriiU64;
}

export type ToriiSumeragiV2LivenessBlocker = Readonly<{
  blocker:
    | "missing_proposal"
    | "body_unavailable"
    | "prepare_quorum_missing"
    | "commit_quorum_missing"
    | "timeout_certificate_missing"
    | "scheduler_starvation"
    | "application_pending"
    | "successor_activation_pending"
    | "local_control_pending";
  details: null;
}>;

export type ToriiSumeragiV2IgnoreReason = Readonly<{
  reason:
    | "wrong_height"
    | "wrong_view"
    | "stale_generation"
    | "busy"
    | "duplicate"
    | "no_matching_work"
    | "observer"
    | "view_closed"
    | "already_decided"
    | "recovery_pending"
    | "irrelevant_view"
    | "unsafe_proposal";
  details: null;
}>;

export interface ToriiSumeragiV2IgnoreCount {
  reason: ToriiSumeragiV2IgnoreReason;
  count: ToriiU64;
}

export interface ToriiSumeragiV2LivenessStatus {
  generation: ToriiU64;
  prepare_quorums: ReadonlyArray<ToriiSumeragiV2VoteQuorumStatus>;
  commit_quorums: ReadonlyArray<ToriiSumeragiV2VoteQuorumStatus>;
  timeout_quorums: ReadonlyArray<ToriiSumeragiV2TimeoutQuorumStatus>;
  outbound_intents: ReadonlyArray<ToriiSumeragiV2OutboundIntentStatus>;
  work: ToriiSumeragiV2WorkStatus;
  queues: ReadonlyArray<ToriiSumeragiV2QueueLivenessStatus>;
  last_progress: ToriiSumeragiV2ProgressTransitionStatus | null;
  no_progress_age_ms: ToriiU64;
  blocker: ToriiSumeragiV2LivenessBlocker | null;
  ignore_counts: ReadonlyArray<ToriiSumeragiV2IgnoreCount>;
}

export interface ToriiSumeragiV2AdapterQueueStatus {
  ingress_keys: number;
  ingress_capacity: number;
  deferred_completion: number;
  deferred_progress: number;
  deferred_progress_capacity: number;
  deferred_normal: number;
  deferred_normal_capacity: number;
}

export interface ToriiSumeragiV2TxQueueStatus {
  tracked_transactions: number;
  queued_transactions: number;
  capacity: number;
  retained_bytes: number;
  max_retained_bytes: number;
  oldest_queued_age_ms: number;
  saturated_by_count: boolean;
  saturated_by_bytes: boolean;
  saturated_by_age: boolean;
}

export interface ToriiSumeragiV2OperatorStatus {
  view_change_install_total: ToriiU64;
  busy_deferral_total: ToriiU64;
  adapter_queues: ToriiSumeragiV2AdapterQueueStatus;
  tx_queue: ToriiSumeragiV2TxQueueStatus;
}

export interface ToriiSumeragiLanePayloadOwnership {
  proposal_height: ToriiU64;
  proposal_view: ToriiU64;
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  lane_block_height: ToriiU64;
  lane_block_view: ToriiU64;
  subject_hash: string;
  qc_mode_tag: string;
  accepted_candidate_indices: ReadonlyArray<ToriiU64>;
  accepted_transaction_hashes: ReadonlyArray<string>;
  previous_lane_block_height: ToriiU64;
  previous_lane_block_descriptor_hash: string | null;
  lane_block_descriptor_hash: string;
  lane_block_descriptor_validator_set: ReadonlyArray<string>;
  lane_block_descriptor_validator_count: number;
  lane_block_descriptor_min_quorum: number;
  payload_ownership_hash: string;
  rbc_instance_hash: string;
}

export type ToriiSumeragiCommittedLaneExecutionStatus =
  | "awaiting_executable_payload"
  | "payload_available_awaiting_executor"
  | "payload_recovered_awaiting_state_application"
  | "payload_preflighted_awaiting_state_application"
  | "payload_preflight_rejected_awaiting_state_application"
  | "application_receipt_conflicts_with_preflight"
  | "awaiting_predecessor_application"
  | "state_applied_by_canonical_block"
  | "state_applied_by_direct_execution";

export interface ToriiSumeragiCommittedLaneBlock {
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  lane_block_height: ToriiU64;
  lane_block_view: ToriiU64;
  descriptor_hash: string;
  proposal_hash: string;
  execution_status: ToriiSumeragiCommittedLaneExecutionStatus;
  executable_payload_available: boolean;
  subject_hash: string;
  payload_ownership_hash: string;
  rbc_instance_hash: string;
  qc_mode_tag: string;
  validator_count: number;
  min_quorum: number;
  prepare_qc_signer_count: number;
  commit_qc_signer_count: number;
}

export interface ToriiSumeragiLaneBlockSessionStatus {
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  lane_block_height: ToriiU64;
  lane_block_view: ToriiU64;
  proposal_hash: string;
  has_proposal: boolean;
  prepare_vote_count: number;
  commit_vote_count: number;
  has_prepare_qc: boolean;
  has_commit_qc: boolean;
  pending_commit_vote_request: boolean;
  pending_committed_session_drain: boolean;
  committed_session_drained: boolean;
  validator_count: number;
  min_quorum: number;
}

export interface ToriiSumeragiSafetyHaltStatus {
  active: boolean;
  reason: string | null;
  height: ToriiU64;
  epoch: ToriiU64;
  first_block_hash: string | null;
  conflicting_block_hash: string | null;
  first_parent_state_root: string | null;
  first_post_state_root: string | null;
  conflicting_parent_state_root: string | null;
  conflicting_post_state_root: string | null;
}

export interface ToriiSumeragiStatus {
  protocol_version: 4;
  node_fingerprint: string;
  build_fingerprint: string;
  config_fingerprint: string;
  restart_required: boolean;
  height_context_id: ToriiSumeragiV2HeightContextId;
  height: ToriiU64;
  view: ToriiU64;
  phase: ToriiSumeragiV2StatusPhase;
  leader: number;
  locked_prepare_qc: ToriiSumeragiV2QuorumCertificateRef | null;
  highest_prepare_qc: ToriiSumeragiV2QuorumCertificateRef | null;
  last_timeout_certificate: ToriiSumeragiV2TimeoutCertificateRef | null;
  body_state: ToriiSumeragiV2BodyState;
  pending_persistence_id: ToriiU64 | null;
  last_committed_height: ToriiU64;
  last_committed_subject: ToriiSumeragiV2BlockSubject | null;
  height_context: ToriiSumeragiV2HeightContextStatus;
  last_commit_qc: ToriiSumeragiV2CommitQcStatus | null;
  liveness: ToriiSumeragiV2LivenessStatus;
}

export interface ToriiSumeragiPipelineExecutionStatus {
  tx_vertices_total: ToriiU64;
  tx_edges_total: ToriiU64;
  overlay_count_total: ToriiU64;
  overlay_instr_total: ToriiU64;
  overlay_bytes_total: ToriiU64;
  rbc_chunks_total: ToriiU64;
  rbc_bytes_total: ToriiU64;
  detached_prepared_total: ToriiU64;
  detached_merged_total: ToriiU64;
  detached_fallback_total: ToriiU64;
  detached_fallback_fee_postprocessing_total: ToriiU64;
  detached_fallback_user_executor_total: ToriiU64;
  detached_fallback_durable_state_total: ToriiU64;
  detached_fallback_unsupported_instruction_total: ToriiU64;
  detached_fallback_rejected_eval_total: ToriiU64;
  detached_fallback_overlay_error_total: ToriiU64;
  quarantine_executed_total: ToriiU64;
}

export interface ToriiSumeragiNposDiagnostics {
  epoch_length_blocks: ToriiU64;
  vrf_commit_deadline_offset: ToriiU64;
  vrf_reveal_deadline_offset: ToriiU64;
  epoch_seed: string;
  prf_height: ToriiU64;
  prf_view: ToriiU64;
}

export interface ToriiSumeragiDiagnosticLaneCommitment {
  block_height: ToriiU64;
  lane_id: number;
  tx_count: ToriiU64;
  total_chunks: ToriiU64;
  rbc_bytes_total: ToriiU64;
  teu_total: ToriiU64;
  block_hash: string;
}

export interface ToriiSumeragiDiagnosticDataspaceCommitment
  extends ToriiSumeragiDiagnosticLaneCommitment {
  dataspace_id: ToriiU64;
}

export interface ToriiSumeragiDiagnosticRuntimeUpgrade {
  allow: boolean;
  require_metadata: boolean;
  metadata_key: string | null;
  allowed_ids: ReadonlyArray<string>;
}

export interface ToriiSumeragiDiagnosticLaneGovernance {
  lane_id: number;
  alias: string;
  governance: string | null;
  manifest_required: boolean;
  manifest_ready: boolean;
  manifest_path: string | null;
  validator_ids: ReadonlyArray<string>;
  quorum: number | null;
  protected_namespaces: ReadonlyArray<string>;
  runtime_upgrade: ToriiSumeragiDiagnosticRuntimeUpgrade | null;
}

export type ToriiSumeragiNativeAmxParticipantApplicationState =
  | "certified_pending_carrier"
  | "committed_evidence_pending"
  | "durably_applied"
  | "conflict";

export interface ToriiSumeragiNativeAmxParticipantApplication {
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  participant_height: ToriiU64;
  participant_view: ToriiU64;
  predecessor_height: ToriiU64;
  predecessor_descriptor_hash: string | null;
  descriptor_hash: string;
  proposal_hash: string;
  settlement_hash: string;
  source_count: number;
  application_block_height: ToriiU64 | null;
  application_block_hash: string | null;
  state: ToriiSumeragiNativeAmxParticipantApplicationState;
}

export type ToriiSumeragiAutonomousLaneExecutionStage =
  | "reservations_durable"
  | "executable_payload_durable"
  | "payload_availability_certified"
  | "lane_certified"
  | "certified_bundle_durable"
  | "merge_candidate_durable"
  | "global_carrier_committed"
  | "kura_wsv_application_receipt_durable"
  | "queue_finalized"
  | "conflict";

export type ToriiSumeragiAutonomousLaneExecutionStuckReason =
  | "awaiting_executable_payload"
  | "awaiting_payload_availability"
  | "awaiting_lane_certification"
  | "certified_bundle_unavailable"
  | "awaiting_merge_selection"
  | "awaiting_global_carrier"
  | "awaiting_application_receipt"
  | "queue_finalization_unverifiable"
  | "evidence_conflict";

export interface ToriiSumeragiAutonomousLaneExecution {
  lane_id: number;
  dataspace_id: ToriiU64;
  lane_incarnation: string;
  lane_block_height: ToriiU64;
  lane_block_view: ToriiU64;
  proposal_height: ToriiU64;
  proposal_view: ToriiU64 | null;
  reservation_owner_hash: string;
  proposal_identity_hash: string;
  reservation_group_hash: string;
  proposal_hash: string | null;
  descriptor_hash: string | null;
  executable_payload_hash: string | null;
  source_bundle_hash: string | null;
  merge_entry_hash: string | null;
  application_block_height: ToriiU64 | null;
  application_block_hash: string | null;
  reservation_count: number;
  transaction_count: number;
  highest_durable_stage: ToriiSumeragiAutonomousLaneExecutionStage;
  stuck_reason: ToriiSumeragiAutonomousLaneExecutionStuckReason | null;
}

export interface ToriiSumeragiDiagnostics {
  pipeline_execution: ToriiSumeragiPipelineExecutionStatus;
  tx_queue_depth: ToriiU64;
  tx_queue_capacity: ToriiU64;
  tx_queue_retained_bytes: ToriiU64;
  tx_queue_max_retained_bytes: ToriiU64;
  tx_queue_saturated: boolean;
  tx_queue_saturated_by_count: boolean;
  tx_queue_saturated_by_bytes: boolean;
  tx_queue_saturated_by_age: boolean;
  tx_queue_oldest_queued_age_ms: ToriiU64;
  npos: ToriiSumeragiNposDiagnostics | null;
  lane_commitments: ReadonlyArray<ToriiSumeragiDiagnosticLaneCommitment>;
  dataspace_commitments: ReadonlyArray<ToriiSumeragiDiagnosticDataspaceCommitment>;
  lane_settlement_commitments: ReadonlyArray<ToriiLaneSettlementCommitment>;
  lane_relay_envelopes: ReadonlyArray<ToriiLaneRelayEnvelope>;
  lane_payload_ownerships: ReadonlyArray<ToriiSumeragiLanePayloadOwnership>;
  committed_lane_blocks: ReadonlyArray<ToriiSumeragiCommittedLaneBlock>;
  lane_block_sessions: ReadonlyArray<ToriiSumeragiLaneBlockSessionStatus>;
  lane_governance_sealed_total: number;
  lane_governance_sealed_aliases: ReadonlyArray<string>;
  lane_governance: ReadonlyArray<ToriiSumeragiDiagnosticLaneGovernance>;
  native_amx_participant_applications: ReadonlyArray<
    ToriiSumeragiNativeAmxParticipantApplication
  >;
  autonomous_lane_executions: ReadonlyArray<
    ToriiSumeragiAutonomousLaneExecution
  >;
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

export interface ToriiSumeragiV2QcResponse {
  highest_prepare_qc: ToriiSumeragiV2QuorumCertificateRef | null;
  locked_prepare_qc: ToriiSumeragiV2QuorumCertificateRef | null;
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
  | "Censorship"
  | "SumeragiV2Equivocation";

export interface SumeragiEvidenceListOptions {
  limit?: NumericLike;
  offset?: NumericLike;
  kind?: SumeragiEvidenceKind;
  signal?: AbortSignal;
}

export interface SumeragiEvidenceRecordBase {
  kind: SumeragiEvidenceKind;
  recorded_height: number;
  recorded_view: number;
  recorded_ms: number;
  consensus_admitted_height: number | null;
}

export interface SumeragiDoubleVoteEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  kind: "DoublePrepare" | "DoubleCommit";
  phase: "Prepare" | "Commit" | "NewView";
  height: number;
  view: number;
  epoch: number;
  signer: number;
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
  phase: "Prepare" | "Commit" | "NewView";
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
  signers: ReadonlyArray<string>;
  submitted_at_height_min?: number;
  submitted_at_height_max?: number;
}

export interface SumeragiV2EquivocationEvidenceRecord
  extends SumeragiEvidenceRecordBase {
  kind: "SumeragiV2Equivocation";
  class: "proposal" | "phase_vote" | "timeout_vote";
  height: number;
  view: number;
  epoch: number;
  signer: number;
  context_id: string;
  artifact_hash_1: string;
  artifact_hash_2: string;
}

export type SumeragiEvidenceRecord =
  | SumeragiDoubleVoteEvidenceRecord
  | SumeragiInvalidQcEvidenceRecord
  | SumeragiInvalidProposalEvidenceRecord
  | SumeragiCensorshipEvidenceRecord
  | SumeragiV2EquivocationEvidenceRecord;

export interface SumeragiEvidenceListResponse {
  total: number;
  items: ReadonlyArray<SumeragiEvidenceRecord>;
}

export interface SumeragiEvidenceCountResponse {
  count: number;
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
  /** Per-call signer; falls back to `ToriiClient`'s `canonicalRequestAuth`. */
  canonicalAuth?: CanonicalRequestAuth;
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
  quantity: QuantityInput;
};

type AssetDefinitionMintSpec = {
  accountId?: string;
  assetHoldingId?: string;
  quantity: QuantityInput;
};

type MintTransferSpec = {
  sourceAssetHoldingId?: string;
  quantity: QuantityInput;
  destinationAccountId: string;
};

type AccountTransferSpec = {
  sourceAssetHoldingId: string;
  quantity: QuantityInput;
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

export interface RegisterDomainInput {
  networkId: NetworkId;
  authority: string;
  domainId: string;
  /** Required signature-bound fee payer, maxima, and gas bound. */
  feePayment: BrowserFeePayment;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

/**
 * Parameters for {@link buildTransaction}. The `instructions` array must be
 * non-empty and each entry should be either a builder result or a JSON string
 * produced by `noritoEncodeInstruction`.
 */
export interface TransactionAssemblyInput {
  networkId: NetworkId;
  authority: string;
  instructions: Array<object | string>;
  /** Required signature-bound fee payer, maxima, and gas bound. */
  feePayment: BrowserFeePayment;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export type ExecutableBatchEntry =
  | {
      kind: "instruction";
      instruction: object | string;
    }
  | {
      kind: "contractCall";
      contractAddress: string;
      /** Exact marked 32-byte Iroha code hash. */
      expectedCodeHash: Buffer | ArrayBuffer | ArrayBufferView | string;
      entrypoint: string;
      /** Canonical schema-bound argument-record bytes; maximum 1 MiB. */
      arguments?: Buffer | ArrayBuffer | ArrayBufferView | null;
    };

export interface ExecutableBatchTransactionAssemblyInput {
  networkId: NetworkId;
  authority: string;
  entries: ExecutableBatchEntry[];
  /** Must include `gasLimit` when any entry is a contract call. */
  feePayment: BrowserFeePayment;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export type ExecutableBatchTransactionPayloadDraftInput = Omit<
  ExecutableBatchTransactionAssemblyInput,
  "privateKey" | "privateKeyAlgorithm"
>;

/** Exact unsigned payload whose non-fee fields are fixed before quoting. */
export type TransactionPayloadDraftInput = Omit<
  TransactionAssemblyInput,
  "privateKey" | "privateKeyAlgorithm"
>;

/** Native and JSON projections of one exact unsigned quote draft. */
export interface TransactionPayloadDraftResult {
  payload: Record<string, unknown>;
  payloadJson: string;
  payloadBytes: Buffer;
  payloadHash: Buffer;
}
/** Input for applying a returned quote to the exact draft and signing it. */
export interface QuotedTransactionPayloadSigningInput {
  /** Application-pinned exact NetworkId expected in the quoted payload. */
  networkId: NetworkId;
  payload: Record<string, unknown> | TransactionPayloadDraftResult;
  quotedFeePayment: BrowserFeePayment | Record<string, unknown> | string;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

/** Required signature-bound fee intent shared by all transaction builders. */
export interface FeePaymentRequired {
  feePayment: BrowserFeePayment;
}

export type SccpRouteGovernanceActionKind =
  | "Register"
  | "SetActivation"
  | "SwitchRevision"
  | "InitializeTrustAnchor"
  | "AdvanceTrustAnchor"
  | "Remove";

export interface SccpRouteKeyV1 {
  readonly lane_id: SccpLaneIdV1;
  readonly route_id: string;
  readonly asset_key: string;
  readonly revision: number;
}

export interface SccpRegisterRouteV1 {
  readonly route: SccpGovernedRouteV1;
  readonly native_trust_anchor: SccpNativeTrustAnchorV1 | null;
}

export interface SccpSetRouteActivationV1 {
  readonly key: SccpRouteKeyV1;
  readonly expected_current: SccpRouteActivationV1;
  readonly next: SccpRouteActivationV1;
  readonly inbound_finality_cutoff: SccpInboundFinalityCutoffV1 | null;
}

export interface SccpSwitchRouteRevisionV1 {
  readonly previous_key: SccpRouteKeyV1;
  readonly expected_previous: SccpRouteActivationV1;
  readonly previous_next: SccpRouteActivationV1;
  readonly previous_inbound_finality_cutoff: SccpInboundFinalityCutoffV1 | null;
  readonly successor_key: SccpRouteKeyV1;
  readonly successor_next: SccpRouteActivationV1;
}

export interface SccpInitializeLaneTrustAnchorV1 {
  readonly lane_id: SccpLaneIdV1;
  readonly expected_current: null;
  readonly initial: SccpNativeTrustAnchorV1;
}

export interface SccpAdvanceLaneTrustAnchorV1 {
  readonly lane_id: SccpLaneIdV1;
  readonly expected_current: SccpNativeTrustAnchorV1;
  readonly next: SccpNativeTrustAnchorV1;
}

export type SccpRouteGovernanceActionInput =
  | Readonly<{ action: "Register"; route: SccpRegisterRouteV1 }>
  | Readonly<{ action: "SetActivation"; route: SccpSetRouteActivationV1 }>
  | Readonly<{ action: "SwitchRevision"; route: SccpSwitchRouteRevisionV1 }>
  | Readonly<{
      action: "InitializeTrustAnchor";
      route: SccpInitializeLaneTrustAnchorV1;
    }>
  | Readonly<{ action: "AdvanceTrustAnchor"; route: SccpAdvanceLaneTrustAnchorV1 }>
  | Readonly<{ action: "Remove"; route: SccpRouteKeyV1 }>;

export interface ApplySccpRouteGovernanceTransactionInput
  extends Omit<TransactionAssemblyInput, "instructions"> {
  action: SccpRouteGovernanceActionInput;
}

export interface IvmProvedTransactionAssemblyInput {
  networkId: NetworkId;
  authority: string;
  proved: object | string;
  attachment: object | string;
  /** Required signature-bound fee payer, maxima, and gas bound. */
  feePayment: BrowserFeePayment;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

/** Exact unsigned proved-IVM payload plus its envelope-only proof attachment. */
export type IvmProvedTransactionPayloadDraftInput = Omit<
  IvmProvedTransactionAssemblyInput,
  "privateKey" | "privateKeyAlgorithm"
>;

export interface IvmProvedTransactionPayloadDraftResult
  extends TransactionPayloadDraftResult {
  attachment: Record<string, unknown>;
  attachmentJson: string;
}
export interface QuotedIvmProvedTransactionPayloadSigningInput {
  /** Application-pinned exact NetworkId expected in the quoted payload. */
  networkId: NetworkId;
  payload: Record<string, unknown> | IvmProvedTransactionPayloadDraftResult;
  attachment?: object | string;
  quotedFeePayment: BrowserFeePayment | Record<string, unknown> | string;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RegisterMultisigTransactionInput extends FeePaymentRequired {
  networkId: NetworkId;
  authority: string;
  accountId: string;
  spec: MultisigSpecLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RequiredIvmOverlayTransfer {
  sourceAssetHoldingId?: string;
  source_asset_holding_id?: string;
  sourceAssetId?: string;
  source_asset_id?: string;
  quantity: QuantityInput;
  destinationAccountId?: string;
  destination_account_id?: string;
}

export interface IvmProvedContractCallInputBase {
  authority: string;
  entrypoint?: string | null;
  payload?: JsonValue;
  metadata?: MetadataLike;
  nonce?: number | null;
}

type IvmRequiredAliasPair<
  Camel extends string,
  Snake extends string,
  Value,
> =
  | ({ [Key in Camel]: Value } & { [Key in Snake]?: never })
  | ({ [Key in Camel]?: never } & { [Key in Snake]: Value });

type IvmOptionalAliasPair<
  Camel extends string,
  Snake extends string,
  Value,
> =
  | ({ [Key in Camel]?: Value } & { [Key in Snake]?: never })
  | ({ [Key in Camel]?: never } & { [Key in Snake]: Value });

type IvmContractTarget =
  | (IvmRequiredAliasPair<"contractAddress", "contract_address", string> & {
      contractAlias?: never;
      contract_alias?: never;
    })
  | (IvmRequiredAliasPair<"contractAlias", "contract_alias", string> & {
      contractAddress?: never;
      contract_address?: never;
    });

type IvmProvedContractCallCore = IvmProvedContractCallInputBase &
  {
    networkId: NetworkId;
    chain?: never;
    chainId?: never;
    chain_id?: never;
  } &
  IvmRequiredAliasPair<
    "privateKey",
    "private_key",
    Buffer | ArrayBuffer | ArrayBufferView
  > &
  IvmOptionalAliasPair<
    "privateKeyAlgorithm",
    "private_key_algorithm",
    string | null
  > &
  IvmRequiredAliasPair<"vkRef", "vk_ref", IvmVerifyingKeyRef> &
  IvmContractTarget &
  IvmRequiredAliasPair<"feePayment", "fee_payment", BrowserFeePayment> &
  IvmOptionalAliasPair<
    "requiredOverlayTransfer",
    "required_overlay_transfer",
    RequiredIvmOverlayTransfer | null
  > &
  IvmOptionalAliasPair<"creationTimeMs", "creation_time_ms", number | null> &
  IvmOptionalAliasPair<"ttlMs", "ttl_ms", number | null> &
  IvmRequiredAliasPair<
    "expectedCodeHashHex",
    "expected_code_hash_hex",
    string
  > &
  IvmRequiredAliasPair<
    "expectedArtifactSha256Hex",
    "expected_artifact_sha256_hex",
    string
  >;

/**
 * A proved deployed-contract call must carry an independently trusted code
 * hash and a SHA-256 digest of the complete artifact. The helper verifies
 * Torii's simulation, the ledger/Core body hash, and every header/body byte
 * against those values before deriving, proving, signing, or submitting.
 */
export type IvmProvedContractCallInput = IvmProvedContractCallCore;

export interface IvmProvedContractCallOptions {
  signal?: AbortSignal;
  proofIntervalMs?: number;
  proofTimeoutMs?: number | null;
  waitForCommit?: boolean;
  transactionIntervalMs?: number;
  transactionTimeoutMs?: number | null;
}

export interface IvmProvedContractCallResult {
  hash: string;
  signedTransaction: Buffer;
  submission: unknown;
  status: ToriiAppliedTransactionStatus | null;
  simulation: ContractCallSimulateResponse;
  metadata: { [key: string]: JsonValue };
  proved: IvmProvedPayload;
  attachment: { [key: string]: JsonValue };
  proofJobId: string;
  feeQuoteDraft: IvmProvedTransactionPayloadDraftResult;
  feeQuote: FeeQuoteResponse;
  requiredOverlayTransfer: JsonValue | null;
}

export interface MintAssetInput {
  networkId: NetworkId;
  authority: string;
  assetHoldingId: string;
  quantity: QuantityInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface BurnAssetInput {
  networkId: NetworkId;
  authority: string;
  assetHoldingId: string;
  quantity: QuantityInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface MintTriggerInput {
  networkId: NetworkId;
  authority: string;
  triggerId: string;
  repetitions: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface BurnTriggerInput {
  networkId: NetworkId;
  authority: string;
  triggerId: string;
  repetitions: NumericLike;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface TransferAssetInput {
  networkId: NetworkId;
  authority: string;
  sourceAssetHoldingId: string;
  quantity: QuantityInput;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface TransferDomainInput {
  networkId: NetworkId;
  authority: string;
  sourceAccountId: string;
  domainId: string;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface TransferAssetDefinitionInput {
  networkId: NetworkId;
  authority: string;
  sourceAccountId: string;
  assetDefinitionId: string;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface TransferNftInput {
  networkId: NetworkId;
  authority: string;
  sourceAccountId: string;
  nftId: string;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RwaParentRefInput {
  rwa?: string;
  rwaId?: string;
  quantity: QuantityInput;
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
  quantity: QuantityInput;
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
  networkId: NetworkId;
  authority: string;
  rwa?: RegisterRwaPayloadInput | string;
  rwaJson?: RegisterRwaPayloadInput | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface TransferRwaInput {
  networkId: NetworkId;
  authority: string;
  sourceAccountId: string;
  rwaId: string;
  quantity: QuantityInput;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface MergeRwasInput {
  networkId: NetworkId;
  authority: string;
  merge?: MergeRwasPayloadInput | string;
  mergeJson?: MergeRwasPayloadInput | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RedeemRwaInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  quantity: QuantityInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface FreezeRwaInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface UnfreezeRwaInput extends FreezeRwaInput {}

export interface HoldRwaInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  quantity: QuantityInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface ReleaseRwaInput extends HoldRwaInput {}

export interface ForceTransferRwaInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  quantity: QuantityInput;
  destinationAccountId: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface SetRwaControlsInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  controls?: RwaControlPolicyInput | string;
  controlsJson?: RwaControlPolicyInput | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface SetRwaKeyValueInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  key: string;
  value: JsonValue;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RemoveRwaKeyValueInput {
  networkId: NetworkId;
  authority: string;
  rwaId: string;
  key: string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

/**
 * Parameters for {@link buildMintAndTransferTransaction}. Provide either
 * `transfer` or `transfers`; when `sourceAssetHoldingId` is omitted on a transfer the
 * helper reuses `mint.assetHoldingId` and enforces that at least one transfer is
 * present.
 */
interface MintAndTransferInputBase {
  networkId: NetworkId;
  authority: string;
  mint: {
    assetHoldingId: string;
    quantity: QuantityInput;
  };
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
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
  networkId: NetworkId;
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
  privateKeyAlgorithm?: string | null;
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
  networkId: NetworkId;
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
  privateKeyAlgorithm?: string | null;
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
  networkId: NetworkId;
  authority: string;
  assetDefinition: {
    assetDefinitionId: string;
    /** Immutable ownership intent; null means intentionally unowned global. */
    owningDomain: string | null;
    metadata?: object;
    mintable?: string;
    logo?: string | null;
    spec?: object;
    balanceScopePolicy: string;
  };
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
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
  state?: null;
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
  hops: ReadonlyArray<KaigiRelayHopInput>;
}

export interface KaigiParticipantCommitmentInput {
  commitment: ArrayBufferView | ArrayBuffer | Buffer | string;
  /** Clear aliases are off-chain only; native ledger instructions require null/omission. */
  aliasTag?: null;
}

export interface KaigiParticipantNullifierInput {
  digest: ArrayBufferView | ArrayBuffer | Buffer | string;
  /** Clear issuance time is off-chain only; native ledger instructions require zero. */
  issuedAtMs: 0;
}

export type KaigiRoomPolicyValue = {
  policy: "Public" | "Authenticated";
  state?: null;
};

export type KaigiRoomPolicyInput =
  | "public"
  | "read-only"
  | "read_only"
  | "open"
  | "authenticated"
  | "auth"
  | "protected"
  | KaigiRoomPolicyValue;

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

export interface LeaveKaigiInput {
  callId: KaigiIdLike;
  participant: string;
  /** Privacy-mode departure is off-chain only in V1. */
  commitment?: null;
  /** Privacy-mode departure is off-chain only in V1. */
  nullifier?: null;
  /** Privacy-mode departure is off-chain only in V1. */
  rosterRoot?: null;
  /** Privacy-mode departure is off-chain only in V1. */
  proof?: null;
}

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
  bandwidthClass: NumericLike;
}

export type KaigiRelayHealthStatusInput =
  | "Healthy"
  | "Degraded"
  | "Unavailable";

export interface ReportKaigiRelayHealthInput {
  callId: KaigiIdLike;
  relayId: string;
  status: KaigiRelayHealthStatusInput;
  reportedAtMs: NumericLike;
  notes?: string | null;
}

export interface ProposeDeployContractInstructionInput {
  contractAddress: string;
  codeHash: HashLike;
  abiHash: HashLike;
  abiVersion?: 1;
  manifestProvenance?: ToriiGovernanceManifestProvenanceInput | null;
}

export interface ProposeSccpRouteGovernanceInstructionInput {
  networkId: NetworkId;
  action: SccpRouteGovernanceActionInput;
}

export interface CastZkBallotInstructionInput {
  electionId: string;
  proof: ArrayBufferView | ArrayBuffer | Buffer | string;
  publicInputs?: GovernanceZkBallotPublicInputs;
}

export interface GovernanceZkBallotPublicInputs {
  root_hint?: string | null;
  owner?: string | null;
  amount?: QuantityInput | null;
  duration_blocks?: number | string | bigint | null;
  direction?: ToriiGovernanceBallotDirection | null;
  nullifier?: string | null;
}

export interface CastPlainBallotInstructionInput {
  referendumId: string;
  owner: string;
  amount: QuantityInput;
  durationBlocks: NumericLike;
  direction?: number | string;
}

export interface PersistCouncilForEpochInstructionInput {
  epoch: NumericLike;
  members: ReadonlyArray<string>;
  alternates?: ReadonlyArray<string>;
}

export interface RegisterZkAssetInstructionInput {
  assetDefinitionId: string;
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

type ContractRequiredAliasPair<
  CamelCase extends string,
  SnakeCase extends string,
  Value,
> =
  | ({ [Key in CamelCase]: Value } & { [Key in SnakeCase]?: Value })
  | ({ [Key in CamelCase]?: Value } & { [Key in SnakeCase]: Value });

// BEGIN GENERATED: kotodama-v1-dynamic-access-policy
export const KOTODAMA_V1_STATE_MAP_KEY_TYPES: readonly [
  "int",
  "decimal",
  "quantity",
  "bool",
  "string",
  "bytes",
  "DataSpaceId",
  "AccountId",
  "AssetDefinitionId",
  "AssetId",
  "NftId",
  "DomainId",
  "Name",
];
export const KOTODAMA_V1_DYNAMIC_ACCESS_BOUND_KINDS: readonly [
  "range",
  "take",
];
export const KOTODAMA_V1_DYNAMIC_ACCESS_MAX_KEYS: 64;

export type ContractStateMapKeyTypeName =
  (typeof KOTODAMA_V1_STATE_MAP_KEY_TYPES)[number];
export type ContractDynamicAccessBoundKind =
  (typeof KOTODAMA_V1_DYNAMIC_ACCESS_BOUND_KINDS)[number];
// END GENERATED: kotodama-v1-dynamic-access-policy

export type ContractDynamicAccessHintInput =
  & ContractRequiredAliasPair<"baseKey", "base_key", string>
  & ContractRequiredAliasPair<
    "keyType",
    "key_type",
    ContractStateMapKeyTypeName
  >
  & ContractRequiredAliasPair<
    "boundKind",
    "bound_kind",
    ContractDynamicAccessBoundKind
  >
  & ContractRequiredAliasPair<"maxKeys", "max_keys", NumericLike>;

export interface ContractAccessSetHintsInput {
  readKeys?: ReadonlyArray<string>;
  writeKeys?: ReadonlyArray<string>;
  dynamicReads?: ReadonlyArray<ContractDynamicAccessHintInput>;
  dynamic_reads?: ReadonlyArray<ContractDynamicAccessHintInput>;
  dynamicWrites?: ReadonlyArray<ContractDynamicAccessHintInput>;
  dynamic_writes?: ReadonlyArray<ContractDynamicAccessHintInput>;
}

export type ContractEntrypointKind =
  | "Kotoage"
  | "View"
  | "Hajimari"
  | "Kaizen";

export interface ContractEntrypointKindRecord {
  kind: ContractEntrypointKind;
  value: null;
}

export type ContractEntrypointValueKindName =
  | "Int"
  | "Decimal"
  | "Quantity"
  | "Bool"
  | "String"
  | "Json"
  | "Name"
  | "AccountId"
  | "AssetDefinitionId"
  | "AssetId"
  | "DomainId"
  | "NftId"
  | "DataSpaceId"
  | "Blob";

export interface ContractEntrypointValueKindRecord {
  kind: ContractEntrypointValueKindName;
  value: null;
}

export interface ContractEntrypointStructTypeNode {
  name: string;
  fields: ReadonlyArray<string>;
}

export interface ContractEntrypointListTypeNode {
  capacity: NumericLike;
}

export type ContractEntrypointValueTypeNode =
  | { kind: "Struct"; value: ContractEntrypointStructTypeNode }
  | { kind: "Tuple"; value: NumericLike }
  | { kind: "Option"; value: null }
  | { kind: "Result"; value: null }
  | { kind: "List"; value: ContractEntrypointListTypeNode }
  | { kind: "Leaf"; value: ContractEntrypointValueKindRecord };

export interface ContractEntrypointValueType {
  nodes: ReadonlyArray<ContractEntrypointValueTypeNode>;
}

export interface ContractEntrypointArgumentField {
  name: string;
  ty: ContractEntrypointValueType;
}

export interface ContractEntrypointArgumentSchema {
  fields: ReadonlyArray<ContractEntrypointArgumentField>;
}

export type ContractEntrypointParamInput = {
  name: string;
} & ContractRequiredAliasPair<"typeName", "type_name", string>;

export type ContractTriggerRepeats =
  | { Indefinitely: null }
  | { Exactly: NumericLike };

export interface ContractTriggerCallbackInput {
  namespace?: string | null;
  entrypoint: string;
}

export interface ContractTriggerDescriptorInput {
  id: string;
  repeats: ContractTriggerRepeats;
  /** Canonical standard-base64 NRT0 frame for `EventFilterBox`. */
  filter: string;
  authority?: string | null;
  metadata?: Readonly<Record<string, JsonValue>>;
  callback: ContractTriggerCallbackInput;
}

export interface ContractEntrypointInput {
  name: string;
  kind: ContractEntrypointKind | ContractEntrypointKindRecord;
  params?: ReadonlyArray<ContractEntrypointParamInput>;
  argumentSchema?: ContractEntrypointArgumentSchema | null;
  returnType?: string | null;
  returnSchema?: ContractEntrypointValueType | null;
  permission?: string | null;
  readKeys?: ReadonlyArray<string>;
  writeKeys?: ReadonlyArray<string>;
  accessHintsComplete?: boolean | null;
  accessHintsSkipped?: ReadonlyArray<string>;
  triggers?: ReadonlyArray<ContractTriggerDescriptorInput>;
}

export type ContractStateDescriptorInput = {
  name: string;
} & ContractRequiredAliasPair<"typeName", "type_name", string>;

export interface ContractErrorCodeDescriptorInput {
  namespace: string;
  name: string;
  code: NumericLike;
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
  seiyakuName?: string | null;
  codeHash?: HashLike | null;
  abiHash?: HashLike | null;
  compilerFingerprint?: string | null;
  featuresBitmap?: NumericLike | null;
  accessSetHints?: ContractAccessSetHintsInput | null;
  entrypoints?: ReadonlyArray<ContractEntrypointInput> | null;
  states?: ReadonlyArray<ContractStateDescriptorInput> | null;
  errorCodes?: ReadonlyArray<ContractErrorCodeDescriptorInput> | null;
  kotoba?: ReadonlyArray<ContractKotobaEntryInput> | null;
  provenance?: ContractManifestProvenanceInput | null;
}

/**
 * Manifest payload accepted by Torii HTTP contract endpoints. Hash fields must be provided
 * as 32-byte hex strings (optionally prefixed with `0x`); canonical `hash:` literals or
 * binary buffers are rejected at runtime.
 */
export interface ToriiContractManifestInput {
  seiyakuName?: string | null;
  codeHash?: string | null;
  abiHash?: string | null;
  compilerFingerprint?: string | null;
  featuresBitmap?: NumericLike | null;
  accessSetHints?: ContractAccessSetHintsInput | null;
  entrypoints?: ReadonlyArray<ContractEntrypointInput> | null;
  states?: ReadonlyArray<ContractStateDescriptorInput> | null;
  errorCodes?: ReadonlyArray<ContractErrorCodeDescriptorInput> | null;
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

export interface DeployContractReceiptContract {
  name: string;
  contract_alias: string;
  contract_address: string;
  previous_contract_address: string | null;
  kaizen: boolean;
  dataspace: string;
  deploy_nonce: number;
  code_hash_hex: string;
  abi_hash_hex: string;
  tx_hash_hex: string | null;
  pipeline_status?: ToriiPipelineTransactionStatus | null;
  status: string;
}

export interface DeployContractHajimariCallReceipt {
  id: string;
  contract_alias: string;
  entrypoint: string | null;
  tx_hash_hex: string | null;
  pipeline_status?: ToriiPipelineTransactionStatus | null;
  status: string;
}

export interface DeployContractAssertionReceipt {
  id: string;
  contract_alias: string;
  entrypoint: string | null;
  status: string;
  actual_result?: unknown;
  expected_result?: unknown;
  error?: string | null;
}

export interface ContractOperationReceipt {
  operation_kind: string;
  status: string;
  transport: string;
  dataspace: string;
  contract_alias: string | null;
  contract_address: string | null;
  code_hash_hex: string | null;
  abi_hash_hex: string | null;
  tx_hash_hex: string | null;
  entrypoint: string | null;
  entrypoint_hash_hex: string | null;
  gas_limit: number | null;
  gas_used: number | null;
  fee_payment: NoritoFeePaymentIntent | null;
  payload_digest_hex: string;
}

export interface DeployContractResponse {
  ok: boolean;
  bundle_name: string;
  bundle_digest: string;
  chain_fingerprint: string;
  dry_run: boolean;
  completed_stages: string[];
  failure_point: string | null;
  contracts: DeployContractReceiptContract[];
  hajimari_calls: DeployContractHajimariCallReceipt[];
  assertions: DeployContractAssertionReceipt[];
  operation_receipt?: ContractOperationReceipt | null;
}

export interface SetContractAliasRequest {
  authority: string;
  contractAddress: string;
  contractAlias?: string | null;
  leaseExpiryMs?: number | null;
}

export interface SetContractAliasResponse extends AppApiTransactionDraft {
  contract_alias: string | null;
  contract_address: string;
  dataspace: string;
}

export interface ContractCallRequest {
  authority: string;
  contractAddress?: string;
  contractAlias?: string;
  entrypoint: string;
  payload?: unknown;
  creationTimeMs?: NumericLike | null;
  creation_time_ms?: NumericLike | null;
  transactionTtlMs?: NumericLike | null;
  transaction_ttl_ms?: NumericLike | null;
  feePayment: NoritoFeePaymentIntent;
  fee_payment?: NoritoFeePaymentIntent;
}

export interface ContractCallResponse {
  ok: boolean;
  submitted: boolean;
  dataspace: string;
  contract_address?: string;
  code_hash_hex: string;
  abi_hash_hex: string;
  creation_time_ms: number;
  transaction_ttl_ms: number | null;
  tx_hash_hex: string | null;
  entrypoint_hash_hex: string | null;
  pipeline_status?: ToriiPipelineTransactionStatus | null;
  entrypoint: string | null;
  transaction_payload_b64: string | null;
  signing_message_b64: string | null;
  operation_receipt: ContractOperationReceipt;
}

export interface ContractCallSimulateRequest {
  authority: string;
  contractAddress?: string;
  contract_address?: string;
  contractAlias?: string;
  contract_alias?: string;
  entrypoint?: string | null;
  payload?: JsonValue;
  gasLimit: NumericLike;
  gas_limit?: NumericLike;
}

export interface ContractCallSimulateResponse {
  ok: boolean;
  dataspace: string;
  contract_address: string | null;
  code_hash_hex: string;
  abi_hash_hex: string;
  entrypoint: string;
  normalized_payload: JsonValue | null;
  gas_limit: number;
  gas_used: number;
  queued_instructions: JsonValue[];
  result: JsonValue | null;
  error: string | null;
  vm_diagnostic: JsonValue | null;
}

export interface IvmVerifyingKeyRef {
  backend: string;
  name: string;
}

export interface IvmProvedPayload {
  bytecode: string;
  overlay: JsonValue[];
  events_commitment: string;
  gas_policy_commitment: string;
}

export interface IvmCompactProofAttachment {
  backend: string;
  proof: {
    backend: string;
    bytes_b64: string;
  };
  vk_ref: IvmVerifyingKeyRef;
  vk_commitment?: JsonValue;
  envelope_hash?: JsonValue;
  lane_privacy?: JsonValue;
}

export interface ZkIvmExecutionRequest {
  vkRef?: IvmVerifyingKeyRef;
  vk_ref?: IvmVerifyingKeyRef;
  authority: string;
  metadata?: { [key: string]: JsonValue };
  bytecode: string | ArrayBufferView | ArrayBuffer | Buffer;
  proved?: IvmProvedPayload | null;
}

export interface ZkIvmDeriveResponse {
  proved: IvmProvedPayload;
}

export interface ZkIvmProveJobCreatedResponse {
  job_id: string;
}

export interface ZkIvmProveJobResponse {
  job_id: string;
  status: "pending" | "running" | "done" | "error";
  error: string | null;
  proved: IvmProvedPayload | null;
  attachment: IvmCompactProofAttachment | null;
}

export interface ZkIvmProveWaitOptions {
  signal?: AbortSignal;
  intervalMs?: number;
  timeoutMs?: number | null;
  canonicalAuth: CanonicalRequestAuth;
}

export interface ContractManifestRecord {
  manifest: {
    seiyaku_name: string | null;
    /** Lowercase 32-byte hex normalized from Rust's canonical Hash literal. */
    code_hash: string | null;
    /** Lowercase 32-byte hex normalized from Rust's canonical Hash literal. */
    abi_hash: string | null;
    compiler_fingerprint: string | null;
    features_bitmap: number | null;
    access_set_hints:
      | {
          read_keys: ReadonlyArray<string>;
          write_keys: ReadonlyArray<string>;
          dynamic_reads: ReadonlyArray<{
            base_key: string;
            key_type: ContractStateMapKeyTypeName;
            bound_kind: ContractDynamicAccessBoundKind;
            max_keys: number;
          }>;
          dynamic_writes: ReadonlyArray<{
            base_key: string;
            key_type: ContractStateMapKeyTypeName;
            bound_kind: ContractDynamicAccessBoundKind;
            max_keys: number;
          }>;
        }
      | null;
    entrypoints: ReadonlyArray<ContractEntrypointRecord> | null;
    states: ReadonlyArray<ContractStateDescriptorRecord> | null;
    error_codes: ReadonlyArray<ContractErrorCodeDescriptorRecord> | null;
    kotoba: ReadonlyArray<ContractKotobaEntryRecord> | null;
    provenance: ContractManifestProvenanceInput | null;
  };
  code_hash: string | null;
  abi_hash: string | null;
}

export interface ContractEntrypointParamRecord {
  name: string;
  type_name: string;
}

export interface ContractTriggerDescriptorRecord {
  id: string;
  repeats: { Indefinitely: null } | { Exactly: number };
  filter: string;
  authority: string | null;
  metadata: Readonly<Record<string, JsonValue>>;
  callback: { namespace: string | null; entrypoint: string };
}

export interface ContractEntrypointRecord {
  name: string;
  kind: ContractEntrypointKindRecord;
  params: ReadonlyArray<ContractEntrypointParamRecord>;
  argument_schema: ContractEntrypointArgumentSchema | null;
  return_type: string | null;
  return_schema: ContractEntrypointValueType | null;
  permission: string | null;
  read_keys: ReadonlyArray<string>;
  write_keys: ReadonlyArray<string>;
  access_hints_complete: boolean | null;
  access_hints_skipped: ReadonlyArray<string>;
  triggers: ReadonlyArray<ContractTriggerDescriptorRecord>;
}

export interface ContractStateDescriptorRecord {
  name: string;
  type_name: string;
}

export interface ContractErrorCodeDescriptorRecord {
  namespace: string;
  name: string;
  code: number;
}

export interface ContractKotobaEntryRecord {
  msg_id: string;
  translations: ReadonlyArray<ContractKotobaTranslationInput>;
}

export interface ContractCodeBytesRecord {
  code_b64: string;
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

export interface SorafsPorSubmissionResponse {
  status: string;
}

export interface SorafsPorVerdictResponse {
  status: string;
}

export interface SorafsChunkFetchSpecV1 {
  chunk_index: number;
  offset: number;
  length: number;
  digest_blake3: string;
  taikai_segment_hint?: {
    event: string;
    stream: string;
    rendition: string;
    sequence: number;
    payload_len?: number;
    payload_blake3_hex?: string;
  };
}

export interface SorafsChunkFetchPlanV1 {
  schema: "sorafs.chunk_fetch_plan.v1";
  payload_digest_blake3_hex: string;
  chunk_fetch_specs: ReadonlyArray<SorafsChunkFetchSpecV1>;
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
  chunk_plan: SorafsChunkFetchPlanV1;
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
  chunk_count: number | bigint;
  chunk_merkle_path_hex: ReadonlyArray<string>;
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
  chunk_count: number | string;
  chunk_merkle_path: ReadonlyArray<string>;
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
  chunkPlan?: SorafsChunkFetchPlanV1;
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
  chunkPlan: SorafsChunkFetchPlanV1;
  chunkPlanJson: string;
  gatewayResult: SorafsGatewayFetchResult;
  proofSummary: DaProofSummary | null;
}

export interface DaManifestPersistedPaths {
  manifestPath: string;
  manifestJsonPath: string;
  chunkPlanPath: string;
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
  /** Exact genesis-derived network identity signed into the request. */
  networkId: NetworkId;
  /** Canonical I105 account whose consensus DA quota is charged. */
  owner: string;
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
    rowParityStripes?: number;
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
  signerPublicKey?: string;
  privateKey?: ArrayBufferView | ArrayBuffer | Buffer | string;
  privateKeyHex?: string;
  signatureHex?: string;
  artifactDir?: string;
  noSubmit?: boolean;
  dryRun?: boolean;
}

export interface DaRentQuote {
  base_rent: string;
  protocol_reserve: string;
  provider_reward: string;
  pdp_bonus: string;
  potr_bonus: string;
  egress_credit_per_gib: string;
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
  rent_quote: DaRentQuote;
}

export interface DaIngestArtifacts {
  clientBlobIdHex: string;
  payloadHashHex: string;
  signerPublicKey: string;
  signatureHex: string;
  signingDigestHex: string;
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

export function computeDaIngestSigningDigest(
  request: Record<string, unknown>,
): Buffer;

export interface SorafsPorStatusOptions {
  manifestHex?: string | null;
  providerHex?: string | null;
  epoch?: NumericLike;
  status?: string;
  limit?: NumericLike;
  maxBytes?: NumericLike;
  cursor?: string | null;
  signal?: AbortSignal;
}

export interface SorafsPorExportOptions {
  startEpoch?: NumericLike;
  endEpoch?: NumericLike;
  limit?: NumericLike;
  maxBytes?: NumericLike;
  cursor?: string | null;
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

export interface SorafsPinFinalizedCursorV1 {
  height: number;
  block_hash: Uint8Array;
}

export interface SorafsPinResourceUsage {
  manifest_count: number;
  content_bytes: number;
}

export type SorafsPinNativeStatus =
  | { status: "Pending"; value: null }
  | { status: "Approved"; value: number }
  | { status: "Retired"; value: number };

export interface SorafsPinManifestSummaryV1 {
  digest: Uint8Array;
  submitted_by: string;
  submitted_epoch: number;
  content_length: number;
  retention_epoch: number;
  status: SorafsPinNativeStatus;
  successor_of: Uint8Array | null;
}

export interface SorafsPinListResponse {
  finalized_cursor: SorafsPinFinalizedCursorV1;
  charged_usage: SorafsPinResourceUsage;
  manifests: ReadonlyArray<SorafsPinManifestSummaryV1>;
  has_more: boolean;
  next_after_digest: Uint8Array | null;
}

export interface SorafsPinListOptions {
  status?: SorafsManifestStatusState;
  limit?: NumericLike;
  maxBytes?: NumericLike;
  afterDigestHex?: string;
  expectedFinalizedHeight?: NumericLike;
  expectedFinalizedBlockHashHex?: string;
  signal?: AbortSignal;
}

export interface SorafsPinIteratorOptions extends SorafsPinListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface RegisterPinManifestAliasInput {
  namespace: string;
  name: string;
  proof: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface RegisterPinManifestInstructionInput {
  manifestPayload: Buffer | ArrayBuffer | ArrayBufferView;
  alias?: RegisterPinManifestAliasInput | null;
  successorOf?: string | Buffer | ArrayBuffer | ArrayBufferView | null;
}

export type RegisterPinManifestTransactionInput = Omit<
  TransactionAssemblyInput,
  "instructions"
> &
  RegisterPinManifestInstructionInput;

export interface SorafsPinRegisterResponse {
  status: "submitted";
  tx_hash_hex: string;
  manifest_digest_hex: string;
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
  canonicalAuth: CanonicalRequestAuth;
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
  canonicalAuth: CanonicalRequestAuth;
}

export type SorafsOrderbookSide = "bid" | "ask";
export type SorafsOrderbookTier = "hot" | "warm" | "archive";
export type SorafsOrderbookEventKind =
  | "policy_activated"
  | "order_admitted"
  | "order_cancelled"
  | "trade_matched"
  | "order_expired"
  | "channel_expired"
  | "receipt_recorded";

export interface SorafsOrderbookFinalizedAnchorOptions {
  expectedFinalizedHeight?: NumericLike;
  expectedFinalizedBlockHashHex?: string;
}

export interface SorafsOrderbookReadOptions
  extends SorafsOrderbookFinalizedAnchorOptions {
  limit?: NumericLike;
  afterIdHex?: string;
  headers?: Record<string, string>;
  signal?: AbortSignal;
}

export interface SorafsOrderbookEventCursorOptions {
  afterSequence?: NumericLike;
  afterBlockHeight?: NumericLike;
  afterBlockHashHex?: string;
  afterEventIndex?: NumericLike;
}

export interface SorafsOrderbookEventsOptions
  extends SorafsOrderbookFinalizedAnchorOptions,
    SorafsOrderbookEventCursorOptions {
  limit?: NumericLike;
  ifNoneMatch?: string;
  headers?: Record<string, string>;
  signal?: AbortSignal;
}

export interface SorafsOrderbookEventStreamOptions
  extends SorafsOrderbookFinalizedAnchorOptions,
    SorafsOrderbookEventCursorOptions {
  limit?: NumericLike;
  signal?: AbortSignal;
}

export interface SorafsOrderbookEventsWebSocketParams
  extends SorafsOrderbookFinalizedAnchorOptions,
    SorafsOrderbookEventCursorOptions {
  limit?: NumericLike;
  endpointPath?: string;
}

export interface SorafsOrderbookEventsWebSocketDialOptions<T = unknown>
  extends SorafsOrderbookEventsWebSocketParams {
  baseUrl: string;
  protocols?: ConnectWebSocketProtocols;
  websocketOptions?: unknown;
  WebSocketImpl?: ConnectWebSocketConstructor<T>;
}

export interface ClientSorafsOrderbookEventsWebSocketOptions<T = unknown>
  extends SorafsOrderbookEventsWebSocketParams {
  protocols?: ConnectWebSocketProtocols;
  websocketOptions?: unknown;
  WebSocketImpl?: ConnectWebSocketConstructor<T>;
}

export interface SorafsOrderbookEventsWebSocketStreamOptions<T = unknown>
  extends ClientSorafsOrderbookEventsWebSocketOptions<T> {
  signal?: AbortSignal;
  closeOnReturn?: boolean;
}

export interface SorafsOrderbookFinalizedCursor {
  height: number;
  block_hash: string;
}

export interface SorafsOrderbookLedgerStatus {
  open_orders: number;
  partially_filled_orders: number;
  filled_orders: number;
  cancelled_orders: number;
  expired_orders: number;
  trades: number;
  settlement_receipts: number;
  settlement_channels: number;
  open_settlement_channels: number;
  book_revision: number;
  next_admission_sequence: number;
  next_trade_sequence: number;
  updated_at_unix: number;
}

export type SorafsOrderbookNativeOrderStatus =
  | "open"
  | "partially_filled"
  | "filled"
  | "cancelled"
  | "expired"
  | Readonly<Record<string, unknown>>;

export interface SorafsOrderbookOrderRecord
  extends Readonly<Record<string, unknown>> {
  order_id: string;
  owner: unknown;
  canonical_order: string;
  admitted_policy_digest: string;
  admitted_at_unix: number;
  admission_sequence: number;
  remaining_gib: number;
  status: SorafsOrderbookNativeOrderStatus;
  updated_at_unix: number;
  canonical_cancel: string | null;
  cancelled_at_unix: number | null;
  cancelled_policy_digest: string | null;
}

export interface SorafsOrderbookTradeRecord
  extends Readonly<Record<string, unknown>> {
  trade_id: string;
  maker_order_id: string;
  taker_order_id: string;
  trade_sequence: number;
  canonical_trade: string;
  channel_id: string;
  book_revision: number;
  recorded_at_unix: number;
}

export type SorafsOrderbookNativeChannelStatus =
  | "open"
  | "closed"
  | "expired"
  | Readonly<Record<string, unknown>>;

export interface SorafsOrderbookSettlementChannelRecord
  extends Readonly<Record<string, unknown>> {
  channel_id: string;
  trade_id: string;
  buyer: unknown;
  provider: unknown;
  provider_id: string;
  settlement_authority: unknown;
  total_bytes: number;
  remaining_bytes: number;
  initial_xor_locked: string;
  remaining_xor_locked: string;
  status: SorafsOrderbookNativeChannelStatus;
  opened_at_unix: number;
  expires_at_unix: number;
  updated_at_unix: number;
}

export interface SorafsOrderbookSettlementReceiptRecord
  extends Readonly<Record<string, unknown>> {
  receipt_id: string;
  channel_id: string;
  trade_id: string;
  canonical_receipt: string;
  admitted_policy_digest: string;
  admitted_at_unix: number;
  recorded_by: unknown;
}

export interface SorafsOrderbookOrderPage {
  finalized_cursor: SorafsOrderbookFinalizedCursor;
  orders: ReadonlyArray<SorafsOrderbookOrderRecord>;
  has_more: boolean;
  next_after_order_id: string | null;
}

export interface SorafsOrderbookTradePage {
  finalized_cursor: SorafsOrderbookFinalizedCursor;
  trades: ReadonlyArray<SorafsOrderbookTradeRecord>;
  has_more: boolean;
  next_after_trade_id: string | null;
}

export interface SorafsOrderbookSettlementChannelPage {
  finalized_cursor: SorafsOrderbookFinalizedCursor;
  channels: ReadonlyArray<SorafsOrderbookSettlementChannelRecord>;
  has_more: boolean;
  next_after_channel_id: string | null;
}

export interface SorafsOrderbookSettlementReceiptPage {
  finalized_cursor: SorafsOrderbookFinalizedCursor;
  receipts: ReadonlyArray<SorafsOrderbookSettlementReceiptRecord>;
  has_more: boolean;
  next_after_receipt_id: string | null;
}

export interface SorafsOrderbookBookResponse {
  source: "finalized_chain";
  status: SorafsOrderbookLedgerStatus;
  orders: SorafsOrderbookOrderPage;
}

export interface SorafsOrderbookTradesResponse {
  source: "finalized_chain";
  trades: SorafsOrderbookTradePage;
}

export interface SorafsOrderbookChannelsResponse {
  source: "finalized_chain";
  channels: SorafsOrderbookSettlementChannelPage;
}

export interface SorafsOrderbookReceiptsResponse {
  source: "finalized_chain";
  receipts: SorafsOrderbookSettlementReceiptPage;
}

export interface SorafsOrderbookLedgerEvent
  extends Readonly<Record<string, unknown>> {
  kind:
    | SorafsOrderbookEventKind
    | Readonly<{ kind: SorafsOrderbookEventKind; detail?: unknown }>;
  order_id: string | null;
  trade_id: string | null;
  channel_id: string | null;
  receipt_id: string | null;
  provider_id: string | null;
  book_revision: number;
  authority: unknown;
  occurred_at_unix_ms: number;
}

export interface SorafsOrderbookFinalizedEventCursor {
  sequence: number;
  block_height: number;
  block_hash: string;
  event_index: number;
}

export interface SorafsOrderbookFinalizedEvent
  extends SorafsOrderbookFinalizedEventCursor {
  event: SorafsOrderbookLedgerEvent;
}

export interface SorafsOrderbookFinalizedEventPage {
  finalized_cursor: SorafsOrderbookFinalizedCursor;
  events: ReadonlyArray<SorafsOrderbookFinalizedEvent>;
  has_more: boolean;
  next_after: SorafsOrderbookFinalizedEventCursor | null;
}

export interface SorafsOrderbookEventsResponse {
  source: "finalized_chain";
  events: SorafsOrderbookFinalizedEventPage;
}

export interface SorafsReputationWitnessHeaders
  extends Record<string, string | undefined> {
  /**
   * Exact canonical Norito witness. Reputation requests carrying a static
   * witness are single-attempt and are never transparently retried.
   */
  "X-Iroha-Witness": string;
  "X-Iroha-Account"?: string;
}

export type SorafsReputationAuthenticationOptions =
  | {
      canonicalAuth: CanonicalRequestAuth;
      headers?: Record<string, string>;
    }
  | {
      canonicalAuth?: never;
      headers: SorafsReputationWitnessHeaders;
    };

export type SorafsReputationCacheOptions =
  SorafsReputationAuthenticationOptions & {
  ifNoneMatch?: string;
  signal?: AbortSignal;
};

export type SorafsReputationEventsOptions =
  SorafsReputationCacheOptions & {
  since?: NumericLike;
  limit?: NumericLike;
};

export type SorafsReputationEventStreamOptions =
  SorafsReputationAuthenticationOptions & {
  since?: NumericLike;
  limit?: NumericLike;
  signal?: AbortSignal;
};

export interface SorafsHedgingBillingAuthOptions {
  canonicalAuth: CanonicalRequestAuth;
  signal?: AbortSignal;
}

export interface SorafsBillingStatementListOptions
  extends SorafsHedgingBillingAuthOptions {
  expectedCheckpointFingerprintHex: string;
  afterStatementIdHex?: string;
  limit: number;
}

export interface SorafsHedgingProjectionOptions
  extends SorafsHedgingBillingAuthOptions {
  expectedCheckpointFingerprintHex: string;
  afterHex?: string;
  limit: number;
}

export type SorafsReputationU64 = number | bigint;

export interface SorafsReputationWeights {
  version: 1;
  por_success_bps: number;
  pdp_success_bps: number;
  potr_success_bps: number;
  latency_bps: number;
  dispute_bps: number;
  token_violation_bps: number;
  repair_breach_bps: number;
}

export interface SorafsReputationProviderMetrics {
  version: 1;
  por_success_bps: number;
  pdp_success_bps: number;
  potr_success_bps: number;
  latency_health_bps: number;
  dispute_rate_bps: number;
  token_violation_rate_bps: number;
  repair_breach_rate_bps: number;
}

export type SorafsReputationDegradationFlagName =
  | "reserve_warning"
  | "reserve_grace"
  | "reserve_delinquent"
  | "reserve_default"
  | "proof_success_below90"
  | "proof_success_below80"
  | "active_dispute"
  | "slashing_event"
  | "low_score";

export interface SorafsReputationDegradationFlag {
  flag: SorafsReputationDegradationFlagName;
  value: null;
}

export interface SorafsReputationProvider {
  provider_id: string;
  score_bps: number;
  degradation_flags: ReadonlyArray<SorafsReputationDegradationFlag>;
  raw_metrics: SorafsReputationProviderMetrics;
  raw_metrics_hash_hex: string;
}

export interface SorafsReputationSnapshotSummary {
  snapshot_id_hex: string;
  generated_at_unix: SorafsReputationU64;
  previous_snapshot_id_hex: string | null;
  merkle_root_hex: string;
  provider_count: number;
  returned_provider_count: number;
  limit: number;
  truncated_providers: boolean;
  alpha_bps: 8500;
  current_score_weight_bps: 7000;
  weights: SorafsReputationWeights;
  providers: ReadonlyArray<SorafsReputationProvider>;
}

export interface SorafsReputationProviderProof {
  provider_id: string;
  leaf_index: number;
  leaf_count: number;
  siblings_hex: ReadonlyArray<string>;
}

export interface SorafsReputationProviderResponse {
  snapshot_id_hex: string;
  generated_at_unix: SorafsReputationU64;
  merkle_root_hex: string;
  provider: SorafsReputationProvider;
  proof: SorafsReputationProviderProof;
}

export interface SorafsReputationWeightsResponse {
  snapshot_id_hex: string;
  generated_at_unix: SorafsReputationU64;
  alpha_bps: 8500;
  current_score_weight_bps: 7000;
  weights: SorafsReputationWeights;
}

export interface SorafsReputationSnapshotEvent {
  version: 1;
  sequence: SorafsReputationU64;
  snapshot_id_hex: string;
  generated_at_unix: SorafsReputationU64;
  merkle_root_hex: string;
  provider_count: number;
  previous_snapshot_id_hex: string | null;
}

export interface SorafsReputationEventsResponse {
  since: SorafsReputationU64 | null;
  limit: number;
  count: number;
  next_since: SorafsReputationU64 | null;
  events: ReadonlyArray<SorafsReputationSnapshotEvent>;
}

export interface SorafsReputationSnapshotSseEvent {
  event: "reputation_snapshot";
  data: SorafsReputationSnapshotEvent;
  id: string;
  retry: null;
  raw: string;
}

export interface SorafsReputationLaggedSseEvent {
  event: "lagged";
  data: SorafsReputationU64;
  id: null;
  retry: null;
  raw: string;
}

export type SorafsReputationSseEvent =
  | SorafsReputationSnapshotSseEvent
  | SorafsReputationLaggedSseEvent;

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

export type UaidManifestRole = "Initiator" | "Participant";

export interface UaidManifestScope {
  dataspace?: number;
  program?: string;
  method?: string;
  asset?: string;
  role?: UaidManifestRole;
}

export type UaidManifestAllowanceWindow = "PerSlot" | "PerMinute" | "PerDay";

export interface UaidManifestAllowEffect {
  Allow: {
    window: UaidManifestAllowanceWindow;
    max_amount?: string;
  };
}

export interface UaidManifestDenyEffect {
  Deny: {
    reason?: string;
  };
}

export type UaidManifestEffect =
  | UaidManifestAllowEffect
  | UaidManifestDenyEffect;

export interface UaidManifestEntry {
  scope: UaidManifestScope;
  effect: UaidManifestEffect;
  notes?: string;
}

export interface UaidAssetPermissionManifest {
  version: 1;
  uaid: string;
  dataspace: number;
  issued_ms: number;
  activation_epoch: number;
  expiry_epoch?: number;
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
  total: number;
  has_more: boolean;
  count_mode: ToriiCountMode;
  manifests: ReadonlyArray<UaidManifestRecord>;
}

export interface PublishSpaceDirectoryManifestRequest {
  authority: string;
  manifest: UaidAssetPermissionManifest;
  reason?: string;
}

export interface RevokeSpaceDirectoryManifestRequest {
  authority: string;
  uaid: string;
  dataspaceId: number;
  revokedEpoch: number;
  reason?: string;
}

export interface UaidBindingsQueryOptions {
  signal?: AbortSignal;
}

export interface UaidManifestQueryOptions {
  dataspaceId?: number;
  status?: "active" | "inactive" | "all";
  limit?: number;
  offset?: number;
  countMode?: ToriiCountMode;
  signal?: AbortSignal;
}

export interface RegisterSmartContractCodeInstructionInput {
  manifest: ContractManifestInput;
}

export interface RegisterSmartContractBytesInstructionInput {
  codeHash: HashLike;
  code: ArrayBufferView | ArrayBuffer | Buffer | string;
}

export type SmartContractUnsigned64 = number | bigint | string;

export interface UploadSmartContractCodeChunkInstructionInput {
  codeHash: HashLike;
  totalSize: SmartContractUnsigned64;
  chunkIndex: number;
  chunkCount: number;
  chunk: ArrayBufferView | ArrayBuffer | Buffer | string;
}

export interface FinalizeSmartContractCodeUploadInstructionInput {
  codeHash: HashLike;
  totalSize: SmartContractUnsigned64;
  chunkCount: number;
}

export interface CancelSmartContractCodeUploadInstructionInput {
  codeHash: HashLike;
}

export interface CommitContractDeploymentInstructionInput {
  expectedDeployNonce: SmartContractUnsigned64;
  contractAddress: string;
  codeHash: HashLike;
  contractAlias: string;
  leaseExpiryMs?: SmartContractUnsigned64 | null;
  expectedPreviousContractAddress?: string | null;
}

export interface RemoveSmartContractBytesInstructionInput {
  codeHash: HashLike;
  reason?: string | null;
}

export interface CreateKaigiTransactionInput {
  networkId: NetworkId;
  authority: string;
  call: CreateKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface JoinKaigiTransactionInput {
  networkId: NetworkId;
  authority: string;
  join: JoinKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface LeaveKaigiTransactionInput {
  networkId: NetworkId;
  authority: string;
  leave: LeaveKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface EndKaigiTransactionInput {
  networkId: NetworkId;
  authority: string;
  end: EndKaigiInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface ConfidentialTransferProofInputV2 {
  amount: NumericLike;
  rhoHex: string;
  diversifierHex: string;
  leafIndex: number;
}

export interface ConfidentialTransferProofOutputV2 {
  amount: NumericLike;
  rhoHex: string;
  ownerTagHex: string;
}

export interface ConfidentialTransferProofResultV2 {
  nullifiers: ReadonlyArray<Buffer>;
  outputCommitments: ReadonlyArray<Buffer>;
  root: Buffer;
  proof: Buffer;
}

export interface ConfidentialUnshieldProofOutputV3 {
  amount: NumericLike;
  rhoHex: string;
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

export interface RecordKaigiUsageTransactionInput {
  networkId: NetworkId;
  authority: string;
  usage: RecordKaigiUsageInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface SetKaigiRelayManifestTransactionInput {
  networkId: NetworkId;
  authority: string;
  manifest: SetKaigiRelayManifestInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RegisterKaigiRelayTransactionInput {
  networkId: NetworkId;
  authority: string;
  relay: RegisterKaigiRelayInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface ReportKaigiRelayHealthTransactionInput {
  networkId: NetworkId;
  authority: string;
  report: ReportKaigiRelayHealthInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface ProposeDeployContractTransactionInput {
  networkId: NetworkId;
  authority: string;
  proposal: ProposeDeployContractInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface ProposeSccpRouteGovernanceTransactionInput
  extends Omit<TransactionAssemblyInput, "instructions"> {
  proposal?: ProposeSccpRouteGovernanceInstructionInput;
  action?: SccpRouteGovernanceActionInput;
}

export interface CastZkBallotTransactionInput {
  networkId: NetworkId;
  authority: string;
  ballot: CastZkBallotInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface CastPlainBallotTransactionInput {
  networkId: NetworkId;
  authority: string;
  ballot: CastPlainBallotInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface PersistCouncilForEpochTransactionInput {
  networkId: NetworkId;
  authority: string;
  record: PersistCouncilForEpochInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RegisterZkAssetTransactionInput {
  networkId: NetworkId;
  authority: string;
  registration: RegisterZkAssetInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface ScheduleConfidentialPolicyTransitionTransactionInput {
  networkId: NetworkId;
  authority: string;
  transition: ScheduleConfidentialPolicyTransitionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface CancelConfidentialPolicyTransitionTransactionInput {
  networkId: NetworkId;
  authority: string;
  cancellation: CancelConfidentialPolicyTransitionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface CreateElectionTransactionInput {
  networkId: NetworkId;
  authority: string;
  election: CreateElectionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface SubmitBallotTransactionInput {
  networkId: NetworkId;
  authority: string;
  ballot: SubmitBallotInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface FinalizeElectionTransactionInput {
  networkId: NetworkId;
  authority: string;
  finalization: FinalizeElectionInstructionInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RegisterSmartContractCodeTransactionInput {
  networkId: NetworkId;
  authority: string;
  manifest: ContractManifestInput;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RegisterSmartContractBytesTransactionInput {
  networkId: NetworkId;
  authority: string;
  codeHash: HashLike;
  code: ArrayBufferView | ArrayBuffer | Buffer | string;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
}

export interface RemoveSmartContractBytesTransactionInput {
  networkId: NetworkId;
  authority: string;
  codeHash: HashLike;
  reason?: string | null;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
  privateKeyAlgorithm?: string | null;
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

export declare class TransactionBatchAdmissionAmbiguousError extends Error {
  constructor(
    message: string,
    expectedCount: number,
    acceptedCount?: number | null,
    cause?: unknown,
  );
  readonly expectedCount: number;
  readonly acceptedCount: number | null;
  readonly ambiguous: true;
  readonly retryable: false;
  readonly cause?: unknown;
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
export declare function buildSorafsOrderbookEventsWebSocketUrl(
  baseUrl: string,
  options?: SorafsOrderbookEventsWebSocketParams,
): string;

export declare function openConnectWebSocket<T = unknown>(
  options: ConnectWebSocketDialOptions<T>,
): T;
export declare function openSorafsOrderbookEventsWebSocket<T = unknown>(
  options: SorafsOrderbookEventsWebSocketDialOptions<T>,
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
  /** Exact genesis-derived network identity required by canonical-auth methods. */
  networkId?: NetworkId;
  /** Immutable exact-network signer required by operator-only browser reads. */
  operatorSigningContext?: OperatorSigningContext;
  defaultHeaders?: Record<string, string>;
  timeoutMs?: NumericLike;
  config?: {
    toriiClient?: {
      timeoutMs?: NumericLike;
      defaultHeaders?: Record<string, string>;
    };
  };
}

export interface ToriiBrowserRequestOptions {
  signal?: AbortSignal;
  headers?: Record<string, string>;
  successStatuses?: ReadonlyArray<number>;
}

export interface ToriiLedgerHeadersOptions {
  from?: number | string | bigint;
  limit?: number | string | bigint;
  signal?: AbortSignal;
}

export interface ToriiBrowserTransactionStatusOptions {
  signal?: AbortSignal;
  headers?: Record<string, string>;
  scope?: "local" | "global";
}

export interface ToriiBrowserTransactionStatusPollOptions {
  signal?: AbortSignal;
  headers?: Record<string, string>;
  intervalMs?: number;
  timeoutMs?: number;
  maxAttempts?: number;
}

export interface ToriiBrowserSubmitTransactionAndWaitOptions
  extends ToriiBrowserTransactionStatusPollOptions {
  hashHex?: string;
}

export interface ToriiBrowserNodeCapabilities {
  abi_version: number;
  data_model_version: number;
  signed_transaction_schema_hash_hex: string;
  crypto?: Record<string, unknown>;
  query?: Record<string, unknown>;
}

export interface ToriiBrowserContractDeploymentStateRequest {
  authority: string;
  contract_alias: string;
}

export interface ToriiBrowserContractDeploymentStateResponse {
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

export interface CanonicalJsonRequestSignerInput {
  message: Buffer;
  networkId: NetworkId;
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

export interface ToriiBrowserContractDeploymentStateOptions
  extends ToriiBrowserRequestOptions {
  authAccountId?: string;
  sign?: (
    input: CanonicalJsonRequestSignerInput,
  ) => CanonicalJsonRequestSignature | Promise<CanonicalJsonRequestSignature>;
  timestampMs?: number;
  nonce?: string;
}

export interface ToriiBrowserCanonicalRequestOptions
  extends ToriiBrowserRequestOptions {
  authAccountId: string;
  sign: (
    input: CanonicalJsonRequestSignerInput,
  ) => CanonicalJsonRequestSignature | Promise<CanonicalJsonRequestSignature>;
  timestampMs?: number;
  nonce?: string;
}

export declare class ToriiBrowserHttpError extends Error {
  readonly response: Response;
  readonly status: number;
  readonly bodyText: string;
}

/** Terminal non-replayable loss reported by, or inferred for, a live Torii stream. */
export declare class ToriiBrowserStreamGapError extends Error {
  readonly code: string;
  readonly droppedMessages: number | null;
  readonly replayAvailable: boolean;
  readonly payload: ToriiContractEventStreamErrorPayload | null;
  constructor(
    message: string,
    options?: {
      code?: string;
      droppedMessages?: number | null;
      replayAvailable?: boolean;
      payload?: ToriiContractEventStreamErrorPayload | null;
    },
  );
}

export declare class ToriiBrowserClient {
  readonly baseUrl: string;
  constructor(baseUrl: string | URL, options?: ToriiBrowserClientOptions);
  submitTransaction(
    signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
    options?: ToriiBrowserRequestOptions,
  ): Promise<unknown | null>;
  getTransactionStatus(
    hashHex: string,
    options?: ToriiBrowserTransactionStatusOptions,
  ): Promise<ToriiPipelineTransactionStatus | null>;
  waitForTransactionStatus(
    hashHex: string,
    options?: ToriiBrowserTransactionStatusPollOptions,
  ): Promise<ToriiAppliedTransactionStatus>;
  submitTransactionAndWait(
    signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
    options: ToriiBrowserSubmitTransactionAndWaitOptions,
  ): Promise<ToriiAppliedTransactionStatus>;
  getNodeCapabilities(
    options: ToriiBrowserCanonicalRequestOptions,
  ): Promise<ToriiBrowserNodeCapabilities>;
  getContractDeploymentState(
    request: ToriiBrowserContractDeploymentStateRequest,
    options?: ToriiBrowserContractDeploymentStateOptions,
  ): Promise<ToriiBrowserContractDeploymentStateResponse>;
  resolveContractAlias(
    contractAlias: string,
    options: ToriiBrowserCanonicalRequestOptions,
  ): Promise<unknown>;
  getAccount(
    accountId: string,
    options?: ToriiBrowserRequestOptions,
  ): Promise<unknown>;
  getOfflineCapability(
    options?: { signal?: AbortSignal },
  ): Promise<OfflineStatus>;
  submitKagemushaTopUpV4(
    request: KagemushaNoritoRequestV4,
    options?: { signal?: AbortSignal },
  ): Promise<KagemushaOperationReference>;
  submitKagemushaRedeemV4(
    request: KagemushaNoritoRequestV4,
    options?: { signal?: AbortSignal },
  ): Promise<KagemushaOperationReference>;
  getKagemushaOperationStatus(
    operationId: string,
    options?: { signal?: AbortSignal },
  ): Promise<KagemushaOperationStatus>;
  listExplorerAccounts<T = unknown>(
    options?: ToriiBrowserExplorerAccountsOptions,
  ): Promise<ToriiBrowserExplorerCursorPage<T>>;
  getExplorerAccount(
    accountId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerDomains<T = unknown>(
    options?: ToriiBrowserExplorerDomainsOptions,
  ): Promise<ToriiBrowserExplorerCursorPage<T>>;
  getExplorerDomain(
    domainId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerAssets<T = unknown>(
    options?: ToriiBrowserExplorerAssetsOptions,
  ): Promise<ToriiBrowserExplorerCursorPage<T>>;
  getExplorerAsset(
    assetId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listAccountAssets(
    accountId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  /** List effective direct and role-inherited permissions for an account. */
  listAccountPermissions<T = ToriiAccountPermissionItem>(
    accountId: string,
    options?: ToriiBrowserAccountPermissionsListOptions,
  ): Promise<ToriiBrowserAccountPermissionsListResponse<T>>;
  listAccountHistory<T = ToriiAccountHistoryItem>(
    accountId: string,
    options?: ToriiBrowserAccountHistoryListOptions,
  ): Promise<ToriiBrowserAccountHistoryListResponse<T>>;
  queryAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options: TransactionQueryOptions & ToriiBrowserCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryTransactions<T = ToriiAccountTransactionItem>(
    options: TransactionQueryOptions & ToriiBrowserCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryVisibleTransactions<T = ToriiAccountTransactionItem>(
    options: TransactionQueryOptions & ToriiBrowserCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  listContractActivity<T = ToriiContractActivityItem>(
    options?: ToriiBrowserContractActivityListOptions,
  ): Promise<ToriiBrowserContractActivityListResponse<T>>;
  listContractEvents<T = ToriiContractEventItem>(
    options?: ToriiBrowserContractEventListOptions,
  ): Promise<ToriiBrowserContractEventListResponse<T>>;
  streamContractEvents<T = ToriiContractEventItem>(
    options?: ToriiBrowserContractEventStreamOptions,
  ): AsyncGenerator<ToriiSseEvent<T>, void, unknown>;
  listAssetHolders(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listAssetDefinitions(options?: Record<string, unknown>): Promise<unknown>;
  getAssetDefinition(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  resolveAlias(
    aliasOrRequest: string | Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  resolveAssetAlias(
    aliasOrRequest: string | Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerAssetDefinitions<T = ToriiBrowserExplorerAssetDefinition>(
    options?: ToriiBrowserExplorerAssetDefinitionsOptions,
  ): Promise<ToriiBrowserExplorerCursorPage<T>>;
  getExplorerAssetDefinitionEconometrics(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getExplorerAssetDefinitionSnapshot(
    assetDefinitionId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerNfts<T = unknown>(
    options?: ToriiBrowserExplorerOwnedDomainOptions,
  ): Promise<ToriiBrowserExplorerCursorPage<T>>;
  getExplorerNft(
    nftId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerRwas<T = unknown>(
    options?: ToriiBrowserExplorerOwnedDomainOptions,
  ): Promise<ToriiBrowserExplorerCursorPage<T>>;
  getExplorerRwa(
    rwaId: string,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listExplorerBlocks(options?: Record<string, unknown>): Promise<unknown>;
  getExplorerBlock(
    identifier: string | number | bigint,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  listLedgerHeaders(options?: ToriiLedgerHeadersOptions): Promise<unknown>;
  getLedgerStateRoot(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  getLedgerStateProof(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  /** Exact canonical result-bearing SignedBlockWire at a finalized height. */
  getLedgerExecutedBlockWire(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<Buffer>;
  getLedgerBlockProof(
    height: number | string | bigint,
    entryHash: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiBlockProofs>;
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
  getMultisigSpec(
    selector: MultisigAccountSelector,
    options: ToriiBrowserCanonicalRequestOptions,
  ): Promise<MultisigSpecResponse>;
  queryMultisigProposals(
    selector: MultisigProposalsQueryRequest,
    options: ToriiBrowserCanonicalRequestOptions,
  ): Promise<MultisigProposalsQueryResponse>;
  resolveMultisigProposal(
    request: MultisigProposalsResolveRequest,
    options: ToriiBrowserCanonicalRequestOptions,
  ): Promise<MultisigProposalResolveResponse>;
  submitMultisigPropose(
    request: Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  submitMultisigContractCallPropose(
    request: Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  submitMultisigContractCallApprove(
    request: Record<string, unknown>,
    options?: Record<string, unknown>,
  ): Promise<unknown>;
  getSumeragiStatus(options?: Record<string, unknown>): Promise<Record<string, unknown>>;
  getSumeragiStatusTyped(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiStatus>;
  getSumeragiDiagnostics(options?: Record<string, unknown>): Promise<Record<string, unknown>>;
  getSumeragiDiagnosticsTyped(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiDiagnostics>;
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

export interface ValidationFeeCheckpointV1 {
  readonly height: number | string | bigint;
  readonly contextId: string;
}

export interface ValidationFeeLedgerBindingV1 {
  readonly schema: "cbsi.mobile-validation-fee-ledger-binding.v1";
  readonly networkId: NetworkId;
  readonly policyChainGenesisHash: string;
  readonly checkpoint: ValidationFeeCheckpointV1;
}

export interface NormalizedValidationFeeCheckpointV1 {
  readonly height: bigint;
  readonly contextId: string;
}

export interface NormalizedValidationFeeLedgerBindingV1 {
  readonly schema: "cbsi.mobile-validation-fee-ledger-binding.v1";
  readonly networkId: NetworkId;
  readonly policyChainGenesisHash: string;
  readonly checkpoint: NormalizedValidationFeeCheckpointV1;
}

export interface ValidationFeeVerifiedPlainElectorateSnapshotV1 {
  readonly rosterRoot: string;
  readonly memberCount: string;
  readonly capturedAtHeight: string;
  readonly approvalGateHeight: string;
}

export interface ValidationFeeVerifiedEnactmentWindowV1 {
  readonly opens_at_height: string;
  readonly closes_at_height: string;
  readonly enacted_at_height: string;
}

export interface ValidationFeeVerifiedFinalizationV1 {
  readonly proposal_id: string;
  readonly referendum_id: string;
  readonly finalized_at_height: string;
  readonly mode: "PLAIN";
  readonly approve: string;
  readonly reject: string;
  readonly abstain: string;
  readonly min_turnout: string;
  readonly approval_threshold_numerator: string;
  readonly approval_threshold_denominator: string;
  readonly approved: true;
}

export interface ValidationFeeVerifiedParliamentProposalV1 {
  readonly proposal_kind:
    | "ValidationFeePolicyV1"
    | "ValidationFeePayoutLifecycleV1";
  readonly proposal_id: string;
  readonly payload_hash: string;
  readonly parliament_roster_root: string;
  readonly plainElectorateRules: Readonly<ValidationFeePlainElectorateRulesV1>;
  readonly plainElectorateSnapshot: Readonly<ValidationFeeVerifiedPlainElectorateSnapshotV1>;
  readonly enactment_window: Readonly<ValidationFeeVerifiedEnactmentWindowV1>;
  readonly finalization: Readonly<ValidationFeeVerifiedFinalizationV1>;
}

export interface ValidationFeeVerifiedParliamentV1 {
  readonly validationFeePolicy: Readonly<ValidationFeeVerifiedParliamentProposalV1>;
  readonly payoutLifecycle: Readonly<ValidationFeeVerifiedParliamentProposalV1>;
  readonly payoutLifecycleSealHash: string;
}

export interface ValidationFeeVerifiedPayoutRecipientV1 {
  readonly account_id: string;
  readonly share_basis_points: 2500;
}

export interface ValidationFeeVerifiedPayoutV1 {
  readonly contractAddress: string;
  readonly codeHash: string;
  readonly entrypoint: "autonomous_validation_fee_tick";
  readonly dsAssetDefinitionId: string;
  readonly xorAssetDefinitionId: string;
  readonly treasuryAccountId: string;
  readonly vaultAccountId: string;
  readonly batchDsMinorUnits: "1000";
  readonly dsScale: 2;
  readonly xorOutputMin: "4";
  readonly xorOutputMax: "100";
  readonly recipients: ReadonlyArray<
    Readonly<ValidationFeeVerifiedPayoutRecipientV1>
  >;
}

export interface ValidationFeeVerifiedCurrentPolicyV1 {
  readonly activePolicyVersion: string;
  readonly activePolicyHash: string;
  readonly feeAssetDefinitionId: string;
  readonly feeScale: 2;
  readonly feeMinorUnits: "10";
  readonly chargingMode: "PER_QUALIFYING_TRANSFER_INSTRUCTION";
  readonly effectiveFromHeight: string;
  readonly expiresAfterHeight: string | null;
  readonly parliament: Readonly<ValidationFeeVerifiedParliamentV1>;
  readonly payout: Readonly<ValidationFeeVerifiedPayoutV1>;
}

export interface ValidationFeeVerifiedPolicyProjectionV1 {
  readonly schema: "iroha.validation_fee.verified_policy_projection.v1";
  readonly version: 1;
  readonly network_id: string;
  readonly policy_chain_genesis_hash: string;
  readonly registry_hash: string;
  readonly head_policy_version: bigint;
  readonly head_policy_hash: string;
  readonly current_policy: Readonly<ValidationFeeVerifiedCurrentPolicyV1> | null;
  readonly trusted_checkpoint_height: bigint;
  readonly trusted_checkpoint_context_id: string;
  readonly evaluated_block_height: bigint;
  readonly evaluated_context_id: string;
  readonly evaluated_block_hash: string;
  readonly observed_ledger_tip_height: bigint;
  readonly more_available: boolean;
}

export interface ValidationFeeCurrentPolicyProofPageV1 {
  readonly proofNorito: Buffer;
  readonly projection: ValidationFeeVerifiedPolicyProjectionV1;
  readonly promotedCheckpoint: NormalizedValidationFeeCheckpointV1;
}

export interface ValidationFeePolicyProofCatchUpV1
  extends ValidationFeeCurrentPolicyProofPageV1 {
  readonly binding: NormalizedValidationFeeLedgerBindingV1;
  readonly pagesVerified: number;
}

export declare class ToriiClient {
  constructor(baseUrl: string, options?: ToriiClientOptions);
  getOfflineCapability(
    options?: { signal?: AbortSignal },
  ): Promise<OfflineStatus>;
  submitKagemushaTopUpV4(
    request: KagemushaNoritoRequestV4,
    options?: { signal?: AbortSignal },
  ): Promise<KagemushaOperationReference>;
  submitKagemushaRedeemV4(
    request: KagemushaNoritoRequestV4,
    options?: { signal?: AbortSignal },
  ): Promise<KagemushaOperationReference>;
  getKagemushaOperationStatus(
    operationId: string,
    options?: { signal?: AbortSignal },
  ): Promise<KagemushaOperationStatus>;
  listAccounts<T = ToriiAccountListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAccounts<T = ToriiAccountListItem>(
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccounts<T = ToriiAccountListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountsQuery<T = ToriiAccountListItem>(
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  listDomains<T = ToriiDomainListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryDomains<T = ToriiDomainListItem>(
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateDomains<T = ToriiDomainListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateDomainsQuery<T = ToriiDomainListItem>(
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAssetDefinitions<T = ToriiAssetDefinitionListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAssetDefinitions<T = ToriiAssetDefinitionListItem>(
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAssetDefinitions<T = ToriiAssetDefinitionListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAssetDefinitionsQuery<T = ToriiAssetDefinitionListItem>(
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  listRepoAgreements(
    options?: IterableListOptions,
  ): Promise<RepoAgreementListResponse>;
  queryRepoAgreements(
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<RepoAgreementListResponse>;
  iterateRepoAgreements(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiRepoAgreement, void, unknown>;
  iterateRepoAgreementsQuery(
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<ToriiRepoAgreement, void, unknown>;
  listNfts<T = ToriiNftListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryNfts<T = ToriiNftListItem>(
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateNfts<T = ToriiNftListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateNftsQuery<T = ToriiNftListItem>(
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  listRwas<T = ToriiRwaListItem>(
    options?: IterableListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryRwas<T = ToriiRwaListItem>(
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateRwas<T = ToriiRwaListItem>(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateRwasQuery<T = ToriiRwaListItem>(
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
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
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccountAssets<T = ToriiAccountAssetItem>(
    accountId: string,
    options?: AccountAssetIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountAssetsQuery<T = ToriiAccountAssetItem>(
    accountId: string,
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
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
    options: TransactionQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryTransactions<T = ToriiAccountTransactionItem>(
    options: TransactionQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryVisibleTransactions<T = ToriiAccountTransactionItem>(
    options: TransactionQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: AccountTransactionIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountTransactionsQuery<T = ToriiAccountTransactionItem>(
    accountId: string,
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateTransactionsQuery<T = ToriiAccountTransactionItem>(
    options: TransactionIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateVisibleTransactionsQuery<T = ToriiAccountTransactionItem>(
    options: TransactionIteratorOptions & RequiredCanonicalRequestOptions,
  ): AsyncGenerator<T, void, unknown>;
  listAssetHolders<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options?: AssetHolderListOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  queryAssetHolders<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options: IterableQueryOptions & RequiredCanonicalRequestOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAssetHolders<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options?: AssetHolderIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAssetHoldersQuery<T = ToriiAssetHolderItem>(
    assetDefinitionId: string,
    options: PaginationIteratorOptions & RequiredCanonicalRequestOptions,
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
    options: { contentType: string; signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ToriiAttachmentMetadata>;
  listAttachments(options: {
    signal?: AbortSignal;
    canonicalAuth: CanonicalRequestAuth;
  }): Promise<ReadonlyArray<ToriiAttachmentMetadata>>;
  getAttachment(
    attachmentId: string,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<{ data: Buffer; contentType: string | null }>;
  deleteAttachment(
    attachmentId: string,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<void>;
  listVerifyingKeys(options?: ToriiVerifyingKeyListOptions): Promise<unknown>;
  listVerifyingKeysTyped(
    options?: ToriiVerifyingKeyListOptions,
  ): Promise<ReadonlyArray<ToriiVerifyingKeyListItem>>;
  iterateVerifyingKeys(
    options?: ToriiVerifyingKeyListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<ToriiVerifyingKeyListItem, void, unknown>;
  getVerifyingKey(
    backend: ToriiVerifierBackendLabelV1,
    name: string,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  getVerifyingKeyTyped(
    backend: ToriiVerifierBackendLabelV1,
    name: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiVerifyingKeyDetail>;
  registerVerifyingKey(
    payload: ToriiVerifyingKeyRegisterPayload,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiVerifyingKeyTransactionDraft>;
  updateVerifyingKey(
    payload: ToriiVerifyingKeyUpdatePayload,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiVerifyingKeyTransactionDraft>;
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
  lookupRetailRecipient(
    request: RetailRecipientLookupRequest,
    options?: CanonicalRequestOptions,
  ): Promise<RetailRecipientLookupResponse>;
  routeRetailRecipient(
    accountId: string,
    options?: CanonicalRequestOptions,
  ): Promise<RetailRecipientRouteResponse>;
  findFeeSponsorProgramById(
    programId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<FeeSponsorProgram | null>;
  quoteFees(
    payload: Record<string, unknown> | TransactionPayloadDraftResult,
    options: RequiredCanonicalRequestOptions,
  ): Promise<FeeQuoteResponse>;
  getValidationFeeCurrentPolicyProofPage(
    binding: ValidationFeeLedgerBindingV1,
    checkpoint: ValidationFeeCheckpointV1 | null,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ValidationFeeCurrentPolicyProofPageV1>;
  catchUpValidationFeeCurrentPolicyProof(
    binding: ValidationFeeLedgerBindingV1,
    options: {
      checkpoint?: ValidationFeeCheckpointV1;
      maxPages?: number;
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ValidationFeePolicyProofCatchUpV1>;
  listIdentifierPolicies(options?: {
    signal?: AbortSignal;
  }): Promise<IdentifierPolicyListResponse>;
  resolveIdentifier(
    options: IdentifierResolutionRequestOptions,
  ): Promise<IdentifierResolutionReceipt | null>;
  listRamLfeProgramPolicies(options?: {
    signal?: AbortSignal;
  }): Promise<RamLfeProgramPolicyListResponse>;
  executeRamLfeProgram(
    programId: string,
    options: RamLfeExecuteOptions,
  ): Promise<RamLfeExecuteResponse | null>;
  getIdentifierClaimByReceiptHash(
    receiptHash: string,
    options?: { signal?: AbortSignal },
  ): Promise<IdentifierClaimLookupResponse | null>;
  issueIdentifierClaimReceipt(
    accountId: string,
    options: IdentifierResolutionRequestOptions,
  ): Promise<IdentifierResolutionReceipt | null>;
  verifyRamLfeReceipt(options: {
    receipt: Record<string, unknown>;
    outputHex?: string;
    signal?: AbortSignal;
    canonicalAuth: CanonicalRequestAuth;
  }): Promise<Record<string, unknown>>;
  listSorafsPinManifests(
    options?: SorafsPinListOptions,
  ): Promise<SorafsPinListResponse>;
  iterateSorafsPinManifests(
    options?: SorafsPinIteratorOptions,
  ): AsyncGenerator<SorafsPinManifestSummaryV1, void, unknown>;
  listSorafsAliases(
    options: SorafsAliasListOptions,
  ): Promise<SorafsAliasListResponse>;
  iterateSorafsAliases(
    options: SorafsAliasListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<SorafsAliasRecord, void, unknown>;
  listSorafsReplicationOrders(
    options: SorafsReplicationListOptions,
  ): Promise<SorafsReplicationListResponse>;
  iterateSorafsReplicationOrders(
    options: SorafsReplicationListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<SorafsReplicationOrderRecord, void, unknown>;
  submitSorafsOrderbookOrder(
    signedTransaction: SorafsOrderbookSignedTransaction,
    options: SorafsOrderbookTransactionSubmitOptions,
  ): Promise<SorafsOrderbookSubmissionReceipt>;
  submitSorafsOrderbookCancel(
    signedTransaction: SorafsOrderbookSignedTransaction,
    options: SorafsOrderbookTransactionSubmitOptions,
  ): Promise<SorafsOrderbookSubmissionReceipt>;
  submitSorafsOrderbookReceipt(
    signedTransaction: SorafsOrderbookSignedTransaction,
    options: SorafsOrderbookTransactionSubmitOptions,
  ): Promise<SorafsOrderbookSubmissionReceipt>;
  getSorafsOrderbook(
    options?: SorafsOrderbookReadOptions,
  ): Promise<SorafsOrderbookBookResponse>;
  listSorafsOrderbookTrades(
    options?: SorafsOrderbookReadOptions,
  ): Promise<SorafsOrderbookTradesResponse>;
  listSorafsOrderbookChannels(
    options?: SorafsOrderbookReadOptions,
  ): Promise<SorafsOrderbookChannelsResponse>;
  listSorafsOrderbookReceipts(
    options?: SorafsOrderbookReadOptions,
  ): Promise<SorafsOrderbookReceiptsResponse>;
  listSorafsOrderbookEvents(
    options?: SorafsOrderbookEventsOptions,
  ): Promise<SorafsOrderbookEventsResponse | null>;
  streamSorafsOrderbookEvents(
    options?: SorafsOrderbookEventStreamOptions,
  ): AsyncGenerator<ToriiSseEvent<SorafsOrderbookFinalizedEvent>, void, unknown>;
  buildSorafsOrderbookEventsWebSocketUrl(
    options?: SorafsOrderbookEventsWebSocketParams,
  ): string;
  openSorafsOrderbookEventsWebSocket<T = unknown>(
    options?: ClientSorafsOrderbookEventsWebSocketOptions<T>,
  ): T;
  streamSorafsOrderbookEventsWebSocket<T = unknown>(
    options?: SorafsOrderbookEventsWebSocketStreamOptions<T>,
  ): AsyncGenerator<ToriiWebSocketEvent<SorafsOrderbookFinalizedEvent>, void, unknown>;
  getSorafsReputationLatest(
    options: SorafsReputationCacheOptions,
  ): Promise<SorafsReputationSnapshotSummary | null>;
  getSorafsReputationProvider(
    providerId: string,
    options: SorafsReputationCacheOptions,
  ): Promise<SorafsReputationProviderResponse | null>;
  getSorafsReputationSnapshot(
    snapshotIdHex: string,
    options: SorafsReputationCacheOptions,
  ): Promise<SorafsReputationSnapshotSummary | null>;
  getSorafsReputationWeights(
    options: SorafsReputationCacheOptions,
  ): Promise<SorafsReputationWeightsResponse | null>;
  listSorafsReputationEvents(
    options: SorafsReputationEventsOptions,
  ): Promise<SorafsReputationEventsResponse | null>;
  streamSorafsReputationEvents(
    options: SorafsReputationEventStreamOptions,
  ): AsyncGenerator<SorafsReputationSseEvent, void, unknown>;
  getSorafsBillingStatus(
    options: SorafsHedgingBillingAuthOptions,
  ): Promise<Record<string, unknown>>;
  listSorafsBillingStatements(
    options: SorafsBillingStatementListOptions,
  ): Promise<Record<string, unknown>>;
  getSorafsBillingStatement(
    statementIdHex: string,
    expectedCheckpointFingerprintHex: string,
    options: SorafsHedgingBillingAuthOptions,
  ): Promise<Buffer>;
  acknowledgeSorafsBillingStatement(
    statementIdHex: string,
    expectedCheckpointFingerprintHex: string,
    proof: Readonly<SorafsBillingAcknowledgementProofV1>,
    options: SorafsHedgingBillingAuthOptions,
  ): Promise<Record<string, unknown>>;
  getSorafsBillingReconciliation(
    options: SorafsHedgingBillingAuthOptions,
  ): Promise<Record<string, unknown>>;
  getSorafsHedgingExposure(
    options: SorafsHedgingProjectionOptions,
  ): Promise<Record<string, unknown>>;
  getSorafsHedgingIntents(
    options: SorafsHedgingProjectionOptions,
  ): Promise<Record<string, unknown>>;
  getSorafsPinManifest(
    digestHex: string,
    options?: { headers?: Record<string, string>; signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getSorafsPinManifestTyped(
    digestHex: string,
    options?: { headers?: Record<string, string>; signal?: AbortSignal },
  ): Promise<SorafsPinManifestResponse>;
  registerSorafsPinManifest(
    signedTransaction: VersionedSignedTransactionV1,
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown>>;
  registerSorafsPinManifestTyped(
    signedTransaction: VersionedSignedTransactionV1,
    options?: { signal?: AbortSignal },
  ): Promise<SorafsPinRegisterResponse>;
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
    options?: { signal?: AbortSignal },
  ): Promise<DaManifestFetchResponse>;
  getDaManifestToDir(
    storageTicketHex: string,
    options?: {
      outputDir?: string;
      signal?: AbortSignal;
      label?: string;
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
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<AppApiTransactionDraft>;
  revokeSpaceDirectoryManifest(
    request: RevokeSpaceDirectoryManifestRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<AppApiTransactionDraft>;
  submitTransaction(
    payload: VersionedSignedTransactionV1,
    options?: { signal?: AbortSignal },
  ): Promise<unknown>;
  submitTransactionBatch(
    payloads: ReadonlyArray<VersionedSignedTransactionV1>,
    options?: { signal?: AbortSignal },
  ): Promise<{ acceptedCount: number; route?: unknown }>;
  getTransactionStatus(
    hashHex: string,
    options?: TransactionStatusReadOptions,
  ): Promise<ToriiPipelineTransactionStatus | null>;
  waitForTransactionStatus(
    hashHex: string,
    options?: TransactionStatusPollOptions,
  ): Promise<ToriiAppliedTransactionStatus>;
  submitTransactionAndWait(
    payload: VersionedSignedTransactionV1,
    options: SubmitTransactionAndWaitOptions,
  ): Promise<ToriiAppliedTransactionStatus>;
  getTransactionStatusTyped(
    hashHex: string,
    options?: TransactionStatusReadOptions,
  ): Promise<ToriiPipelineTransactionStatus | null>;
  waitForTransactionStatusTyped(
    hashHex: string,
    options?: TransactionStatusPollOptions,
  ): Promise<ToriiAppliedTransactionStatus>;
  submitTransactionAndWaitTyped(
    payload: VersionedSignedTransactionV1,
    options: SubmitTransactionAndWaitOptions,
  ): Promise<ToriiAppliedTransactionStatus>;
  getPipelineRecovery(
    height: number | string | bigint,
    options?: AbortSignalOptions,
  ): Promise<Record<string, unknown> | null>;
  getPipelineRecoveryTyped(
    height: number | string | bigint,
    options?: AbortSignalOptions,
  ): Promise<ToriiPipelineRecoverySidecar | null>;
  getPipelinePreflight(options?: AbortSignalOptions): Promise<ToriiPipelinePreflight>;
  getPipelineRecoveryFastpqProofs(
    height: number | string | bigint,
    options?: AbortSignalOptions,
  ): Promise<Record<string, unknown> | null>;
  getPipelineRecoveryFastpqProofsTyped(
    height: number | string | bigint,
    options?: AbortSignalOptions,
  ): Promise<ToriiPipelineRecoveryFastpqProofs | null>;
  getHealth(options?: AbortSignalOptions): Promise<ToriiHealthStatus | null>;
  getConfiguration(): Promise<unknown | null>;
  getConfigurationTyped(): Promise<ToriiConfigurationSnapshot | null>;
  getConfidentialGasSchedule(): Promise<ConfidentialGasSchedule | null>;
  getStatusSnapshot(options?: AbortSignalOptions): Promise<ToriiStatusSnapshot>;
  deploySoracloudAppInfra(request: SoracloudAppInfraRequest, options: RequiredCanonicalRequestOptions): Promise<SoracloudMutationDraftResponseV1>;
  upgradeSoracloudAppInfra(request: SoracloudAppInfraRequest, options: RequiredCanonicalRequestOptions): Promise<SoracloudMutationDraftResponseV1>;
  getSoracloudAppInfraStatus(options: RequiredCanonicalRequestOptions & {
    appName?: string;
    auditLimit?: NumericLike;
  }): Promise<SoracloudAppInfraStatusResponseV1>;
  getSoracloudNamedAppInfraStatus(
    appName: string,
    options: RequiredCanonicalRequestOptions & { auditLimit?: NumericLike },
  ): Promise<SoracloudAppInfraStatusResponseV1>;
  getNetworkTimeNow(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiNetworkTimeNow>;
  getNetworkTimeStatus(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiNetworkTimeStatus>;
  getNodeCapabilities(options: RequiredCanonicalRequestOptions): Promise<ToriiNodeCapabilities>;
  getSccpCapabilities(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSccpCapabilities>;
  getSccpRegistry(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSccpRegistry>;
  getSccpSoraOutboundMaterial(
    route: {
      sourceProfile: Exclude<SccpNetworkProfile, "sora-taira">;
      routeId: string;
      assetKey: string;
      revision: number;
    },
    options?: { signal?: AbortSignal },
  ): Promise<ToriiSccpSoraOutboundMaterial>;
  getSccpMessageBundle(
    messageId: string,
    options?: { format?: "json"; signal?: AbortSignal },
  ): Promise<SccpMessageBundle>;
  /**
   * Returns an opaque frame preflighted against canonical uncompressed Norito
   * and the `TairaSccpMessageProofV1` schema. It does not decode the embedded
   * message id for independent path-to-payload binding.
   */
  getSccpMessageBundle(
    messageId: string,
    options: { format: "norito"; signal?: AbortSignal },
  ): Promise<Uint8Array>;
  getSccpProofRequest(
    messageId: string,
    options?: { format?: "json"; signal?: AbortSignal },
  ): Promise<SccpProofRequest>;
  /**
   * Returns an opaque frame preflighted against canonical uncompressed Norito
   * and the `SccpGroth16Bn254ProofRequestV1` schema. It does not decode the
   * embedded message id for independent path-to-payload binding.
   */
  getSccpProofRequest(
    messageId: string,
    options: { format: "norito"; signal?: AbortSignal },
  ): Promise<Uint8Array>;
  getSccpRecentMessages(options?: {
    from?: number;
    after_index?: number;
    limit?: number;
    signal?: AbortSignal;
  }): Promise<ToriiSccpRecentMessages>;
  submitBridgeProof(
    payload: ToriiBridgeProofSubmitPayload,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiSccpBridgeSubmitResponse>;
  submitBridgeMessage(
    payload: ToriiBridgeMessageSubmitPayload,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiSccpBridgeSubmitResponse>;
  getRuntimeAbiActive(options: RequiredCanonicalRequestOptions): Promise<ToriiRuntimeAbiActiveResponse>;
  getRuntimeAbiHash(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiRuntimeAbiHashResponse>;
  getRuntimeMetrics(options: RequiredCanonicalRequestOptions): Promise<ToriiRuntimeMetrics>;
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
    /** Canonical 16-byte session ID encoded as hexadecimal text. */
    sessionId: string,
    options: {
      signal?: AbortSignal;
      canonicalAuth: CanonicalRequestAuth;
    },
  ): Promise<ToriiVpnSession | null>;
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
  }): Promise<ToriiVpnReceiptListResponse>;
  getSnsPolicy(
    suffixId: number,
    options?: { signal?: AbortSignal },
  ): Promise<SnsSuffixPolicy>;
  getSnsRegistration(
    selector: string,
    options?: { signal?: AbortSignal },
  ): Promise<SnsNameRecord>;
  getGovernanceProposal(
    proposalId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<Record<string, unknown> | null>;
  getGovernanceProposalTyped(
    proposalId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ToriiGovernanceProposalResult>;
  getGovernanceReferendum(
    referendumId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<Record<string, unknown> | null>;
  getGovernanceReferendumTyped(
    referendumId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ToriiGovernanceReferendumResult>;
  getGovernanceTally(
    referendumId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<Record<string, unknown> | null>;
  getGovernanceTallyTyped(
    referendumId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ToriiGovernanceTallyResult>;
  getGovernanceLocks(
    referendumId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<Record<string, unknown> | null>;
  getGovernanceLocksTyped(
    referendumId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ToriiGovernanceLocksResult>;
  getGovernanceUnlockStats(options: RequiredCanonicalRequestOptions): Promise<Record<string, unknown> | null>;
  getGovernanceUnlockStatsTyped(options: RequiredCanonicalRequestOptions): Promise<ToriiGovernanceUnlockStats>;
  getGovernanceCouncilCurrent(options: RequiredCanonicalRequestOptions): Promise<ToriiGovernanceCouncilCurrentResponse>;
  draftParliamentAttemptV1(
    proposal: ParliamentProposalV1,
    attemptSequence: number,
    options: ParliamentAttemptDraftOptionsV1,
  ): Promise<ParliamentAttemptDraftResponseV1>;
  getParliamentAttemptV1(
    governanceAttemptId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ParliamentAttemptReadResponseV1>;
  getParliamentTimedOvnCastingContextV1(
    ballotAttemptId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ParliamentTimedOvnCastingContextResponseV1>;
  getParliamentTimedOvnCastingProofPageV1(
    ballotAttemptId: string,
    trustedCheckpointHeight: number | bigint,
    options: RequiredCanonicalRequestOptions,
  ): Promise<Buffer>;
  getParliamentTleReleaseContextV1(
    ballotAttemptId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ParliamentTleReleaseContextResponseV1>;
  requestParliamentTlePartialReleaseV1(
    ballotAttemptId: string,
    options: ParliamentTlePartialReleaseOptionsV1,
  ): Promise<ParliamentTlePartialReleaseShareV1>;
  draftParliamentTransitionV1(
    governanceAttemptId: string,
    transition: ParliamentLifecycleTransitionV1,
    options: ParliamentTransitionDraftOptionsV1,
  ): Promise<ParliamentTransitionDraftResponseV1>;
  draftMinistryAgendaProposal(
    payload: MinistryAgendaProposalDraftRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<MinistryAgendaProposalDraftResponse>;
  getMinistryAgendaProposal(
    proposalId: string,
    options: RequiredCanonicalRequestOptions,
  ): Promise<MinistryAgendaProposalGetResponse>;
  governanceProposeDeployContract(
    payload: ToriiGovernanceDeployContractProposalRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<ToriiGovernanceProposalDraftResponseV1>;
  governanceSubmitPlainBallot(
    payload: ToriiGovernancePlainBallotRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ToriiGovernanceBallotResponse>;
  governanceSubmitZkBallotV1(
    payload: ToriiGovernanceZkBallotV1Request,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ToriiGovernanceBallotResponse>;
  governanceSubmitZkBallotProofV1(
    payload: ToriiGovernanceZkBallotProofRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ToriiGovernanceBallotResponse>;
  setProtectedNamespaces(
    namespaces: string | string[],
    options?: { signal?: AbortSignal },
  ): Promise<ToriiProtectedNamespacesApplyResponse>;
  getProtectedNamespaces(options: RequiredCanonicalRequestOptions): Promise<ToriiProtectedNamespacesGetResponse>;
  getSumeragiStatus(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, unknown>>;
  getSumeragiStatusTyped(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiStatus>;
  getSumeragiDiagnostics(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, unknown>>;
  getSumeragiDiagnosticsTyped(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiDiagnostics>;
  getSumeragiPacemaker(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiPacemakerResponse | null>;
  getSumeragiQc(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiV2QcResponse>;
  getSumeragiBlsKeys(options?: {
    signal?: AbortSignal;
  }): Promise<Record<string, string | null>>;
  getSumeragiLeader(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiLeaderSnapshot>;
  getSumeragiParams(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiSumeragiParamsSnapshot>;
  listSumeragiEvidence(
    options?: SumeragiEvidenceListOptions,
  ): Promise<SumeragiEvidenceListResponse>;
  getSumeragiEvidenceCount(): Promise<SumeragiEvidenceCountResponse>;
  getMetrics(options: { asText: true; signal?: AbortSignal }): Promise<string>;
  getMetrics(options?: {
    asText?: boolean;
    signal?: AbortSignal;
  }): Promise<unknown>;
  /** Exact canonical result-bearing SignedBlockWire at a finalized height. */
  getLedgerExecutedBlockWire(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<Buffer>;
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
    networkId: NetworkId;
    appPublicKey: Uint8Array;
    nonce: Uint8Array;
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
  registerContractCode(
    request: RegisterContractCodeRequest,
  ): Promise<unknown | null>;
  deployContract(
    request: DeployContractRequest,
  ): Promise<DeployContractResponse | null>;
  setContractAlias(
    request: SetContractAliasRequest,
  ): Promise<SetContractAliasResponse>;
  prepareContractCall(
    request: ContractCallRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ContractCallResponse>;
  simulateContractCall(
    request: ContractCallSimulateRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ContractCallSimulateResponse>;
  deriveIvmProved(
    request: ZkIvmExecutionRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ZkIvmDeriveResponse>;
  startIvmProve(
    request: ZkIvmExecutionRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ZkIvmProveJobCreatedResponse>;
  getIvmProveJob(
    jobId: string,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ZkIvmProveJobResponse>;
  cancelIvmProveJob(
    jobId: string,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ZkIvmProveJobCreatedResponse>;
  waitForIvmProveJob(
    jobId: string,
    options: ZkIvmProveWaitOptions,
  ): Promise<ZkIvmProveJobResponse>;
  proveIvmAndWait(
    request: ZkIvmExecutionRequest,
    options: ZkIvmProveWaitOptions,
  ): Promise<ZkIvmProveJobResponse>;
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
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<MultisigSpecResponse>;
  queryMultisigProposals(
    request: MultisigProposalsQueryRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<MultisigProposalsQueryResponse>;
  resolveMultisigProposal(
    request: MultisigProposalsResolveRequest,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<MultisigProposalResolveResponse>;
  getContractManifest(
    codeHashHex: string,
  ): Promise<ContractManifestRecord | null>;
  getContractCodeBytes(
    codeHashHex: string,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
  ): Promise<ContractCodeBytesRecord | null>;
  getGovernanceContract(
    contractAddress: string,
    options: { signal?: AbortSignal; canonicalAuth: CanonicalRequestAuth },
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
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionPlanCreateResponse>;
  listSubscriptions(
    options?: SubscriptionListOptions,
  ): Promise<SubscriptionListResponse>;
  iterateSubscriptions(
    options?: SubscriptionIteratorOptions,
  ): AsyncGenerator<SubscriptionListItem, void, unknown>;
  createSubscription(
    request: SubscriptionCreateRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionCreateResponse>;
  getSubscription(
    subscriptionId: string,
    options?: { signal?: AbortSignal },
  ): Promise<SubscriptionGetResponse | null>;
  pauseSubscription(
    subscriptionId: string,
    request: SubscriptionAuthorityActionRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionActionResponse>;
  resumeSubscription(
    subscriptionId: string,
    request: SubscriptionChargeActionRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionActionResponse>;
  cancelSubscription(
    subscriptionId: string,
    request: SubscriptionCancelActionRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionActionResponse>;
  keepSubscription(
    subscriptionId: string,
    request: SubscriptionAuthorityActionRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionActionResponse>;
  chargeSubscriptionNow(
    subscriptionId: string,
    request: SubscriptionChargeActionRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionActionResponse>;
  recordSubscriptionUsage(
    subscriptionId: string,
    request: SubscriptionUsageRequest,
    options: RequiredCanonicalRequestOptions,
  ): Promise<SubscriptionUsageDraft>;
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
): never;

export function signEd25519(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function verifyEd25519(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  signature: ArrayBufferView | ArrayBuffer | Buffer,
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
): boolean;

export function normalizeRecoveryPhrase(phrase: string): RecoveryPhrase;

export function validateRecoveryPhrase(phrase: string): boolean;

export function generateRecoveryPhrase(
  wordCount?: RecoveryPhraseWordCount,
): RecoveryPhrase;

export function entropyToRecoveryPhrase(
  entropy: ArrayBufferView | ArrayBuffer | Buffer,
): RecoveryPhrase;

export function recoveryPhraseToEntropy(phrase: string): Buffer;

export function deriveEd25519SeedFromRecoveryPhrase(phrase: string): Buffer;

export function ed25519SeedToRecoveryPhrase(
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): RecoveryPhrase;

export * from "./canonical-request.js";

export function deriveConfidentialKeyset(
  spendKey: ArrayBufferView | ArrayBuffer | Buffer,
): ConfidentialKeyset;

export function deriveConfidentialKeysetFromHex(
  spendKeyHex: string,
): ConfidentialKeyset;

export function deriveConfidentialOwnerTagV2(
  spendKey: ArrayBufferView | ArrayBuffer | Buffer,
  options: {
    diversifierHex: string;
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
  networkId: NetworkId;
  assetDefinitionId: string;
  spendKey: ArrayBufferView | ArrayBuffer | Buffer;
  rhoHex?: string;
  rho?: ArrayBufferView | ArrayBuffer | Buffer;
}): { nullifier: Buffer; nullifierHex: string };

export const PRIVACY_COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES: number;
export const PRIVACY_COMPILED_PROFILE_CATALOG_VALIDATION_STATUS_V1: Readonly<{
  VALID: 0;
  NULL_POINTER: 1;
  EMPTY: 2;
  ARCHIVE_TOO_LARGE: 3;
  DECODE_RESOURCE_LIMIT: 4;
  SCHEMA_MISMATCH: 5;
  NON_CANONICAL: 6;
  MALFORMED_ARCHIVE: 7;
  INVALID_CATALOG: 8;
}>;
export function isPrivacyNativeAvailable(): boolean;
/**
 * Return this native binary's local compiled-profile catalog. This is build
 * metadata only; network readiness requires `getPrivacyCapabilitiesV1` and a
 * fresh committed Torii response.
 */
export function privacyCompiledProfileCatalogV1(): Buffer;

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

/** Exact compact-length AccountId value encoding for typed policy codecs. */
export function encodeAccountIdNoritoValue(
  value: string,
  context?: string,
): Uint8Array;
/** Exact compact-length AssetDefinitionId value encoding for typed policy codecs. */
export function encodeAssetDefinitionIdNoritoValue(
  value: string,
  context?: string,
): Uint8Array;
/** Exact compact-length Quantity value encoding for typed policy codecs. */
export function encodeQuantityNoritoValue(
  value: QuantityInput,
  context?: string,
): Uint8Array;
/** An ordinary owned byte array; Node `Buffer` compatibility aliases are excluded. */
export interface CancelAssetLockV1Archive extends Uint8Array<ArrayBuffer> {
  readonly write?: never;
}

/** Encode the exact schema-bound bare `CancelAssetLock` V1 archive. */
export function encodeCancelAssetLockV1(
  value: Readonly<CancelAssetLockV1>,
): CancelAssetLockV1Archive;
/** Decode an exact schema-bound bare `CancelAssetLock` V1 archive. */
export function decodeCancelAssetLockV1(
  bytes: CancelAssetLockV1Archive,
): CancelAssetLockV1;
export function noritoEncodeInstruction(instruction: object | string): Buffer;
export function noritoDecodeBlockProofs(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
): ToriiBlockProofs;
export function verifyBlockMerkleProof(
  leaf: string | ArrayBufferView | ArrayBuffer | Buffer,
  proof: ToriiBlockMerkleProof,
  commitment: ToriiBlockMerkleCommitment,
): boolean;
/**
 * Perform pure local Merkle consistency checks against a caller-authenticated
 * anchor. This function does not authenticate the anchor or verify finality.
 */
export function verifyBlockProofs(
  proofs: ToriiBlockProofs,
  trustedAnchor: ToriiBlockProofTrustedAnchor,
): ToriiBlockProofVerification;
/** Encode a canonical compact `InstructionBox` archive for a transaction. */
export function noritoEncodeInstructionBoxArchive(
  instruction: object | string | ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;
/** Decode one exact canonical compact `InstructionBox` transaction archive. */
export function noritoDecodeInstructionBoxArchive(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
): unknown;
/** Encode the exact current Rust manifest-provenance signing frame. */
export function noritoEncodeContractManifestSignaturePayload(
  manifest: Record<string, unknown>,
): Buffer;
export function noritoEncodeTransactionPayloadBatch(
  payloads: ReadonlyArray<ArrayBufferView | ArrayBuffer | Buffer>,
): Buffer;
export const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1:
  "iroha.torii.v1.sorafs.billing.acknowledgement_proof";
export const SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1: 65536;
export interface SorafsBillingAcknowledgementProofV1 {
  requestNonceHex: string;
  authenticationProof: ArrayBufferView | ArrayBuffer | Buffer;
}
export function noritoEncodeSorafsBillingAcknowledgementProofV1(
  proof: Readonly<SorafsBillingAcknowledgementProofV1>,
): Buffer;
export function noritoEncodeOpenVerifyEnvelope(envelope: OpenVerifyEnvelope): Buffer;
export function noritoDecodeOpenVerifyEnvelope(
  bytes: ArrayBufferView | ArrayBuffer | Buffer | string,
): {
  backend: OpenVerifyBackendTag;
  circuit_id: string;
  vk_hash: number[];
  public_inputs: number[];
  proof_bytes: number[];
  aux: number[];
};
export const PRIVACY_EXACT12_FIXTURE_BUNDLE_SCHEMA_NAME_V1:
  "iroha.privacy.exact12-typed-fixture-bundle.v1";
export const PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1: 2097152;
export const PRIVACY_EXACT12_PROTOCOL_IDS_V1: readonly [
  "zk-ace-pq-authorization-v0",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v0",
  "iroha-zk-x509-stark-p256-v0",
  "iroha-jindo-polynomial-commitment-v0",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v0",
];
export type PrivacyExact12ProtocolIdV1 =
  (typeof PRIVACY_EXACT12_PROTOCOL_IDS_V1)[number];
export interface PrivacyExact12TypedFixtureRowV1 {
  protocolId: PrivacyExact12ProtocolIdV1;
  statementNorito: Uint8Array;
  envelopeNorito: Uint8Array;
  submitProofWireId: "iroha.privacy.submit_proof.v1";
  submitProofInstructionNorito: Uint8Array;
  transactionIntentProjectionNorito: Uint8Array;
  transactionIntentDigest: Uint8Array;
  unsignedTransactionPayloadNorito: Uint8Array;
  signedTransactionVersionedNorito: Uint8Array;
  signedTransactionHash: Uint8Array;
}
export interface PrivacyExact12TypedFixtureRowInputV1 {
  protocolId: PrivacyExact12ProtocolIdV1;
  statementNorito: BinaryLike;
  envelopeNorito: BinaryLike;
  submitProofWireId: "iroha.privacy.submit_proof.v1";
  submitProofInstructionNorito: BinaryLike;
  transactionIntentProjectionNorito: BinaryLike;
  transactionIntentDigest: BinaryLike;
  unsignedTransactionPayloadNorito: BinaryLike;
  signedTransactionVersionedNorito: BinaryLike;
  signedTransactionHash: BinaryLike;
}
export interface PrivacyExact12FixtureBundleV1 {
  version: 1;
  rows: PrivacyExact12TypedFixtureRowV1[];
}
export interface PrivacyExact12FixtureBundleInputV1 {
  version: 1;
  rows: ReadonlyArray<Readonly<PrivacyExact12TypedFixtureRowInputV1>>;
}
/** Decode an exact canonical-standard-base64 checked Exact12 archive. */
export function noritoDecodePrivacyExact12FixtureBundleBase64V1(
  value: string,
): PrivacyExact12FixtureBundleV1;
/** Decode one canonical native-independent Exact12 outer Norito archive. */
export function noritoDecodePrivacyExact12FixtureBundleV1(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
): PrivacyExact12FixtureBundleV1;
/** Re-encode a complete Exact12 bundle with canonical outer Norito layout. */
export function noritoEncodePrivacyExact12FixtureBundleV1(
  value: Readonly<PrivacyExact12FixtureBundleInputV1>,
): Uint8Array;
export interface NoritoFrameValidationOptions {
  context?: string;
  expectedSchemaHash?: ArrayBufferView | ArrayBuffer | Buffer;
  expectedTypeName?: string;
  expectedPaddingLength?: number;
  requireNonEmptyPayload?: boolean;
}
export interface ValidatedNoritoFrame {
  payload: Buffer;
  schemaHash: Buffer;
  flags: number;
}
/** Validate one canonical, uncompressed Norito v1 frame without decoding its payload. */
export function validateNoritoFrame(
  bytes: ArrayBufferView | ArrayBuffer | Buffer,
  options?: NoritoFrameValidationOptions,
): ValidatedNoritoFrame;
export interface MultisigProposeNoritoRequest {
  multisig_account_id?: string | null;
  multisigAccountId?: string | null;
  multisig_account_alias?: string | null;
  multisigAccountAlias?: string | null;
  signer_account_id?: string;
  signerAccountId?: string;
  public_key_hex?: string | null;
  publicKeyHex?: string | null;
  signature_b64?: string | null;
  signatureB64?: string | null;
  creation_time_ms?: number | string | bigint | null;
  creationTimeMs?: number | string | bigint | null;
  fee_payment?: NoritoFeePaymentIntent;
  feePayment?: NoritoFeePaymentIntent;
  memo?: string | null;
  validation_fee_policy_version?: string | null;
  validation_fee_policy_hash?: string | null;
  validation_fee_instruction_index?: string | null;
  validation_fee_transfer_entry_index?: string | null;
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
export interface SubscriptionTriggerActionSummary {
  version: 1;
  kind: "billing" | "usage";
  authority: string;
  max_cycles: string;
  charge_at_ms?: number;
  subscription_id?: string;
  trigger_id?: string;
}
export function inspectSubscriptionTriggerAction(
  encodedAction: string,
): SubscriptionTriggerActionSummary;

/**
 * Exact first-release PLAIN eligibility rule bound into validation-fee proposals.
 */
export interface ValidationFeePlainEligibilityRuleV1 {
  readonly rule: "proposal_operator_at_or_before_gate_others_after_gate";
  readonly value: null;
}

/**
 * Exact first-release PLAIN electorate contract bound into a proposal fingerprint.
 */
export interface ValidationFeePlainElectorateRulesV1 {
  readonly voting_asset_id: string;
  readonly bond_escrow_account: string;
  readonly slash_receiver_account: string;
  readonly ballot_amount: string;
  readonly ballot_duration_blocks: string;
  readonly citizenship_amount: string;
  readonly max_members: string;
  readonly conviction_step_blocks: string;
  readonly max_conviction: string;
  readonly min_turnout: string;
  readonly approval_threshold_numerator: string;
  readonly approval_threshold_denominator: string;
  readonly eligibility_rule: Readonly<ValidationFeePlainEligibilityRuleV1>;
}

/**
 * Compute the exact native Parliament fingerprint for a validation-fee policy.
 *
 * The policy must use the native snake-case `ValidationFeePolicyV1` JSON
 * contract. The electorate rules must use the exact first-release PLAIN
 * contract. Missing, unknown, and legacy fields are rejected natively.
 */
export function computeValidationFeePolicyProposalFingerprintV1(
  policy: Readonly<Record<string, JsonValue>>,
  payoutLifecycleProposalId: string | null,
  plainElectorateRules: Readonly<ValidationFeePlainElectorateRulesV1>,
): string;

/**
 * Compute the exact native Parliament fingerprint for a validation-fee payout lifecycle.
 *
 * Both arguments must use their exact native snake-case JSON contracts.
 * Missing, unknown, legacy, and non-canonical fields are rejected natively.
 */
export function computeValidationFeePayoutLifecycleProposalFingerprintV1(
  payoutBinding: Readonly<Record<string, JsonValue>>,
  plainElectorateRules: Readonly<ValidationFeePlainElectorateRulesV1>,
): string;

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
  active_handle_era: number | null;
  next_handle_counter: number | null;
}

export interface AxtHandleRefreshHint {
  dataspace: number | null;
  targetLane: number | null;
  activeHandleEra: number | null;
  nextHandleCounter: number | null;
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
  signedTransaction: VersionedSignedTransactionV1,
  options?: { encoding?: BufferEncoding | "buffer" },
): string | Buffer;

export function hashSignedTransactionPayload(
  signedTransaction: VersionedSignedTransactionV1,
  options?: { encoding?: BufferEncoding | "buffer" },
): string | Buffer;

export function decodeSignedTransaction(
  signedTransaction: VersionedSignedTransactionV1,
): Record<string, unknown>;

export function encodeContractArgumentRecord(
  argumentSchema: Record<string, unknown>,
  payload: Record<string, unknown>,
): Buffer;

export function hashInstructionBatch(
  instructions: Array<object | string>,
  options?: { encoding?: BufferEncoding | "buffer" },
): string | Buffer;

export function resignSignedTransaction(
  networkId: NetworkId,
  signedTransaction: VersionedSignedTransactionV1,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

/** Convert the ergonomic fee intent into the native signer's exact Norito JSON. */
export function feePaymentIntentToNoritoJson(
  feePayment: BrowserFeePayment,
): string;

export function buildTransaction(
  input: TransactionAssemblyInput,
): SignedTransactionResult;

export function buildExecutableBatchTransaction(
  input: ExecutableBatchTransactionAssemblyInput,
): SignedTransactionResult;

export function buildTransactionPayload(
  input: TransactionPayloadDraftInput,
): TransactionPayloadDraftResult;

export function buildExecutableBatchTransactionPayload(
  input: ExecutableBatchTransactionPayloadDraftInput,
): TransactionPayloadDraftResult;

export function signQuotedTransactionPayload(
  input: QuotedTransactionPayloadSigningInput,
): SignedTransactionResult;

export function quoteAndSignTransaction(
  client: ToriiClient,
  input: TransactionPayloadDraftInput & {
    privateKey: Buffer | ArrayBuffer | ArrayBufferView;
    privateKeyAlgorithm?: string | null;
  },
  options?: {
    canonicalAuth?: CanonicalRequestAuth;
    signal?: AbortSignal;
  },
): Promise<
  SignedTransactionResult & {
    draft: TransactionPayloadDraftResult;
    quote: FeeQuoteResponse;
  }
>;

export function buildRegisterPinManifestInstruction(
  input: RegisterPinManifestInstructionInput,
): {
  RegisterPinManifest: {
    manifest_payload: string;
    alias: {
      namespace: string;
      name: string;
      proof: string;
    } | null;
    successor_of: ReadonlyArray<number> | null;
  };
};

export function buildRegisterPinManifestTransaction(
  client: ToriiClient,
  input: RegisterPinManifestTransactionInput,
  options?: {
    canonicalAuth?: CanonicalRequestAuth;
    signal?: AbortSignal;
  },
): Promise<
  SignedTransactionResult & {
    draft: TransactionPayloadDraftResult;
    quote: FeeQuoteResponse;
  }
>;

export function buildRegisterDomainTransaction(
  input: RegisterDomainInput,
): SignedTransactionResult;

export function buildApplySccpRouteGovernanceInstruction(
  action: SccpRouteGovernanceActionInput,
): {
  ApplySccpRouteGovernance: {
    action: Readonly<SccpRouteGovernanceActionInput>;
  };
};

export function buildApplySccpRouteGovernanceTransaction(
  input: ApplySccpRouteGovernanceTransactionInput & FeePaymentRequired,
): SignedTransactionResult;

/**
 * Assemble and sign a transaction whose executable is `Executable::IvmProved`
 * and whose proof attachment list contains the provided attachment.
 */
export function buildIvmProvedTransaction(
  input: IvmProvedTransactionAssemblyInput,
): SignedTransactionResult;
export function buildIvmProvedTransactionPayload(
  input: IvmProvedTransactionPayloadDraftInput,
): IvmProvedTransactionPayloadDraftResult;
export function signQuotedIvmProvedTransactionPayload(
  input: QuotedIvmProvedTransactionPayloadSigningInput,
): SignedTransactionResult;

export const VALIDATION_FEE_CURRENT_POLICY_PROOF_PATH: "/v1/validation-fee/policy/current/proof";
export const VALIDATION_FEE_LEDGER_BINDING_SCHEMA: "cbsi.mobile-validation-fee-ledger-binding.v1";
export const VALIDATION_FEE_POLICY_PROOF_MAX_RESPONSE_BYTES: 4194304;
export const VALIDATION_FEE_REQUIRED_BRIDGE_ABI_VERSION: 22;
export const VALIDATION_FEE_VERIFIED_POLICY_PROJECTION_SCHEMA: "iroha.validation_fee.verified_policy_projection.v1";

export function normalizeValidationFeeCheckpointV1(
  checkpoint: ValidationFeeCheckpointV1,
): NormalizedValidationFeeCheckpointV1;
export function normalizeValidationFeeLedgerBindingV1(
  binding: ValidationFeeLedgerBindingV1,
): NormalizedValidationFeeLedgerBindingV1;
export function encodeValidationFeeCurrentPolicyProofRequestV1(
  checkpoint: ValidationFeeCheckpointV1,
): Buffer;
export function verifyValidationFeeCurrentPolicyProofV1(
  proofNorito: Buffer | ArrayBuffer | ArrayBufferView,
  binding: ValidationFeeLedgerBindingV1,
  checkpoint: ValidationFeeCheckpointV1,
): ValidationFeeVerifiedPolicyProjectionV1;

/** Generic proof-bound submission helper. */
export function submitIvmProvedContractCall(
  client: ToriiClient,
  input: IvmProvedContractCallInput,
  options?: IvmProvedContractCallOptions,
): Promise<IvmProvedContractCallResult>;

export function buildMintAssetTransaction(
  input: MintAssetInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Build and sign a transaction containing a single `Burn::Asset` instruction.
 * Throws if the quantity is non-positive or the asset identifier is empty.
 */
export function buildBurnAssetTransaction(
  input: BurnAssetInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Build and sign a transaction containing a single `Burn::TriggerRepetitions`
 * instruction. Throws when repetitions are not positive integers.
 */
export function buildBurnTriggerTransaction(
  input: BurnTriggerInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildMintTriggerTransaction(
  input: MintTriggerInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildTransferAssetTransaction(
  input: TransferAssetInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRegisterMultisigTransaction(
  input: RegisterMultisigTransactionInput,
): SignedTransactionResult;
export function buildTransferAssetDefinitionTransaction(
  input: TransferAssetDefinitionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildTransferDomainTransaction(
  input: TransferDomainInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildTransferNftTransaction(
  input: TransferNftInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRegisterRwaTransaction(
  input: RegisterRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildTransferRwaTransaction(
  input: TransferRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildMergeRwasTransaction(
  input: MergeRwasInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRedeemRwaTransaction(
  input: RedeemRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildFreezeRwaTransaction(
  input: FreezeRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildUnfreezeRwaTransaction(
  input: UnfreezeRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildHoldRwaTransaction(
  input: HoldRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildReleaseRwaTransaction(
  input: ReleaseRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildForceTransferRwaTransaction(
  input: ForceTransferRwaInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildSetRwaControlsTransaction(
  input: SetRwaControlsInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildSetRwaKeyValueTransaction(
  input: SetRwaKeyValueInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRemoveRwaKeyValueTransaction(
  input: RemoveRwaKeyValueInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Compose a mint followed by one or more transfers. Provide either `transfer`
 * or `transfers`; transfers without an explicit `sourceAssetHoldingId` reuse the mint's
 * destination asset identifier.
 */
export function buildMintAndTransferTransaction(
  input: MintAndTransferInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Register a domain and optionally perform follow-up mints in the same
 * transaction. Accepts either a single `mint` or an array of `mints`.
 */
export function buildRegisterDomainAndMintTransaction(
  input: RegisterDomainAndMintInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Register an account and enqueue one or more asset transfers. Each transfer
 * must specify its source asset identifier; the helper enforces this at runtime.
 */
export function buildRegisterAccountAndTransferTransaction(
  input: RegisterAccountAndTransferInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Register an asset definition and optionally mint initial supply. When both
 * `accountId` and `assetHoldingId` are provided the helper validates that they match
 * the canonical asset-holding id derived from `assetDefinitionId + accountId`.
 */
export function buildRegisterAssetDefinitionAndMintTransaction(
  input: RegisterAssetDefinitionAndMintInput & FeePaymentRequired,
): SignedTransactionResult;
/**
 * Register an asset definition, mint supply, and optionally fan-out transfers.
 * When a transfer omits `sourceAssetHoldingId` the helper reuses the first minted
 * destination identifier.
 */
export function buildRegisterAssetDefinitionMintAndTransferTransaction(
  input: RegisterAssetDefinitionMintAndTransferInput & FeePaymentRequired,
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
  input: CreateKaigiTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildConfidentialTransferProofV2(input: {
  networkId: NetworkId;
  assetDefinitionId: string;
  spendKey: BinaryLike;
  treeCommitments: ReadonlyArray<BinaryLike>;
  inputs: ReadonlyArray<ConfidentialTransferProofInputV2>;
  outputs: ReadonlyArray<ConfidentialTransferProofOutputV2>;
  rootHintHex: string;
  verifyingKey: ToriiVerifyingKeyDetail;
}): ConfidentialTransferProofResultV2;
export function buildConfidentialUnshieldProofV2(input: {
  networkId: NetworkId;
  assetDefinitionId: string;
  spendKey: BinaryLike;
  treeCommitments: ReadonlyArray<BinaryLike>;
  inputs: ReadonlyArray<ConfidentialTransferProofInputV2>;
  publicAmount: NumericLike;
  rootHintHex: string;
  verifyingKey: ToriiVerifyingKeyDetail;
}): ConfidentialUnshieldProofResultV2;
export function buildConfidentialUnshieldProofV3(input: {
  networkId: NetworkId;
  assetDefinitionId: string;
  spendKey: BinaryLike;
  treeCommitments: ReadonlyArray<BinaryLike>;
  inputs: ReadonlyArray<ConfidentialTransferProofInputV2>;
  outputs?: ReadonlyArray<ConfidentialUnshieldProofOutputV3>;
  publicAmount: NumericLike;
  rootHintHex: string;
  verifyingKey: ToriiVerifyingKeyDetail;
}): ConfidentialUnshieldProofResultV3;
export function buildJoinKaigiTransaction(
  input: JoinKaigiTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildLeaveKaigiTransaction(
  input: LeaveKaigiTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildEndKaigiTransaction(
  input: EndKaigiTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRecordKaigiUsageTransaction(
  input: RecordKaigiUsageTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildSetKaigiRelayManifestTransaction(
  input: SetKaigiRelayManifestTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRegisterKaigiRelayTransaction(
  input: RegisterKaigiRelayTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildReportKaigiRelayHealthTransaction(
  input: ReportKaigiRelayHealthTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRegisterSmartContractCodeTransaction(
  input: RegisterSmartContractCodeTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRegisterSmartContractBytesTransaction(
  input: RegisterSmartContractBytesTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRemoveSmartContractBytesTransaction(
  input: RemoveSmartContractBytesTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildProposeDeployContractTransaction(
  input: ProposeDeployContractTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildProposeSccpRouteGovernanceTransaction(
  input: ProposeSccpRouteGovernanceTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildCastZkBallotTransaction(
  input: CastZkBallotTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildCastPlainBallotTransaction(
  input: CastPlainBallotTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildPersistCouncilForEpochTransaction(
  input: PersistCouncilForEpochTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildRegisterZkAssetTransaction(
  input: RegisterZkAssetTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildScheduleConfidentialPolicyTransitionTransaction(
  input: ScheduleConfidentialPolicyTransitionTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildCancelConfidentialPolicyTransitionTransaction(
  input: CancelConfidentialPolicyTransitionTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildCreateElectionTransaction(
  input: CreateElectionTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildSubmitBallotTransaction(
  input: SubmitBallotTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function buildFinalizeElectionTransaction(
  input: FinalizeElectionTransactionInput & FeePaymentRequired,
): SignedTransactionResult;
export function submitSignedTransaction(
  client: ToriiClient,
  signedTransaction: VersionedSignedTransactionV1,
  options?: {
    waitForCommit?: boolean;
    pollIntervalMs?: number;
    timeoutMs?: number;
  } & (
    | { privateKey?: undefined; networkId?: undefined }
    | {
        networkId: NetworkId;
        privateKey: ArrayBufferView | ArrayBuffer | Buffer;
      }
  ),
): Promise<{
  hash: string;
  submission: unknown;
  status?: ToriiAppliedTransactionStatus;
}>;

export const SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1: 1048576;

export interface IssueReplicationOrderInstruction {
  IssueReplicationOrder: {
    order_id: string;
    order_payload: string;
    issued_epoch: number;
    deadline_epoch: number;
    musubi_archive: string | null;
  };
}

export interface CompleteReplicationOrderInstruction {
  CompleteReplicationOrder: {
    order_id: string;
    provider_id: string;
    completion_epoch: number;
    expected_authority: ProviderIngestCompletionAuthorityV1;
    expected_assignment_revision: number;
    finalized_anchor: ProviderIngestFinalizedAnchorV1;
  };
}

export interface ProviderIngestCompletionSignerPolicyV1 {
  policy_id: string;
  revision: number;
  predecessor_digest: string | null;
  policy_digest: string;
}

export interface ProviderIngestCompletionAuthorityV1 {
  provider_owner: string;
  signer_policy: ProviderIngestCompletionSignerPolicyV1;
}

export interface ProviderIngestFinalizedAnchorV1 {
  height: number;
  block_hash: string;
}

export interface ExpireReplicationOrderInstruction {
  ExpireReplicationOrder: {
    order_id: string;
    expiration_epoch: number;
  };
}

export interface SorafsReplicationOrderPayloadSummaryV1 {
  orderId: string;
  targetReplicas: number;
  providerIds: string[];
  issuedAt: string;
  deadlineAt: string;
}

/**
 * Validate a canonical, unpadded Norito `ReplicationOrderV1` archive.
 */
export function validateSorafsReplicationOrderPayloadV1(
  payload: BinaryLike,
  expectedOrderId?: string | null,
): SorafsReplicationOrderPayloadSummaryV1;

/**
 * Build a native `IssueReplicationOrder` instruction. IDs are exact non-zero
 * lowercase 64-hex strings and `orderPayload` is canonical standard base64.
 */
export function buildIssueReplicationOrderInstruction(options: {
  orderId: string;
  orderPayload: string;
  issuedEpoch: NumericLike;
  deadlineEpoch: NumericLike;
  musubiArchiveId?: string | null;
}): IssueReplicationOrderInstruction;

/**
 * Build the provider-specific six-field completion instruction. The authority,
 * assignment revision, and finalized anchor are mandatory commit-time
 * compare-and-set inputs.
 */
export function buildCompleteReplicationOrderInstruction(options: {
  orderId: string;
  providerId: string;
  completionEpoch: NumericLike;
  expectedAuthority: {
    providerOwner: string;
    signerPolicy: {
      policyId: string;
      revision: NumericLike;
      predecessorDigest: string | null;
      policyDigest: string;
    };
  };
  expectedAssignmentRevision: NumericLike;
  finalizedAnchor: {
    height: NumericLike;
    blockHash: string;
  };
}): CompleteReplicationOrderInstruction;

export function buildExpireReplicationOrderInstruction(options: {
  orderId: string;
  expirationEpoch: NumericLike;
}): ExpireReplicationOrderInstruction;

export interface CancelAssetLockInstruction {
  CancelAssetLock: {
    escrow_id: string;
    expected_remaining_amount: string;
  };
}

/** Exact two-field value carried by a bare `CancelAssetLock` V1 archive. */
export interface CancelAssetLockV1 {
  readonly escrow_id: string;
  readonly expected_remaining_amount: string;
}

/** Maximum UTF-8 bytes accepted for a CancelAssetLock lock-id preimage. */
export declare const CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1: 4096;

/**
 * Build a native compare-and-cancel asset-lock instruction. `lockId` is
 * exact nonempty text without surrounding whitespace/BOM and at most
 * {@link CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1} UTF-8 bytes. It is
 * deterministically hashed to the ledger `EscrowId`; the precondition must be
 * a positive canonical quantity.
 */
export function buildCancelAssetLockInstruction(options: {
  lockId: string;
  expectedRemainingAmount: QuantityInput;
}): CancelAssetLockInstruction;

export type AssetTransferAvailability = "Enabled" | "Disabled";

/** Maximum UTF-8 bytes accepted for an asset-transfer availability reason. */
export declare const ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1: 512;

export interface SetAssetTransferAvailabilityInstruction {
  SetAssetTransferAvailability: {
    account_id: string;
    asset_definition_id: string;
    expected_revision: string;
    incoming: AssetTransferAvailability;
    outgoing: AssetTransferAvailability;
    reason: string | null;
  };
}

/**
 * Atomically update both transfer directions for one account and asset
 * definition. `expectedRevision` is a compare-and-set precondition, and
 * `reason` is bounded by
 * {@link ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1}.
 */
export function buildSetAssetTransferAvailabilityInstruction(options: {
  accountId: string;
  assetDefinitionId: string;
  expectedRevision: NumericLike;
  incoming: AssetTransferAvailability;
  outgoing: AssetTransferAvailability;
  reason?: string | null;
}): SetAssetTransferAvailabilityInstruction;

/**
 * Build a `Mint::Asset` instruction payload with deterministic quantity
 * normalisation.
 */
export function buildMintAssetInstruction({
  assetId,
  quantity,
}: {
  assetId: string;
  quantity: QuantityInput;
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
  quantity: QuantityInput;
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
  balanceScopePolicy: string;
  balance_scope_policy?: string;
  /** Immutable ownership intent; null means intentionally unowned global. */
  owningDomain: string | null;
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

export function buildSetAccountKeyValueInstruction(options: {
  accountId: string;
  key: string;
  value: JsonValue;
}): {
  SetKeyValue: {
    Account: {
      object: string;
      key: string;
      value: JsonValue;
    };
  };
};

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
  quantity: QuantityInput;
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
  quantity: QuantityInput;
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
  quantity: QuantityInput;
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
  quantity: QuantityInput;
}): object;

export function buildReleaseRwaInstruction({
  rwaId,
  quantity,
}: {
  rwaId: string;
  quantity: QuantityInput;
}): object;

export function buildForceTransferRwaInstruction({
  rwaId,
  quantity,
  destinationAccountId,
}: {
  rwaId: string;
  quantity: QuantityInput;
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

export function buildReportKaigiRelayHealthInstruction(
  report: ReportKaigiRelayHealthInput,
): object;

export function buildProposeDeployContractInstruction(
  input: ProposeDeployContractInstructionInput,
): object;
export function buildProposeSccpRouteGovernanceInstruction(
  input: ProposeSccpRouteGovernanceInstructionInput,
): object;

export function buildCastZkBallotInstruction(
  input: CastZkBallotInstructionInput,
): object;

export function buildCastPlainBallotInstruction(
  input: CastPlainBallotInstructionInput,
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
  amount: QuantityInput;
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

export function buildRegisterZkAssetInstruction(
  input: RegisterZkAssetInstructionInput,
): object;

export function buildScheduleConfidentialPolicyTransitionInstruction(
  input: ScheduleConfidentialPolicyTransitionInstructionInput,
): object;

export function buildCancelConfidentialPolicyTransitionInstruction(
  input: CancelConfidentialPolicyTransitionInstructionInput,
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

export function buildUploadSmartContractCodeChunkInstruction(
  input: UploadSmartContractCodeChunkInstructionInput,
): object;

export function buildFinalizeSmartContractCodeUploadInstruction(
  input: FinalizeSmartContractCodeUploadInstructionInput,
): object;

export function buildCancelSmartContractCodeUploadInstruction(
  input: CancelSmartContractCodeUploadInstructionInput,
): object;

export function buildCommitContractDeploymentInstruction(
  input: CommitContractDeploymentInstructionInput,
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

export interface SoracloudHfSharedLeaseJoinDraftInput {
  repoId: string;
  /** Full 40-character lowercase Hugging Face commit OID. */
  revision: string;
  serviceName: string;
  apartmentName: string | null;
  storageClass: SoracloudStorageClass;
  leaseTermMs: number | bigint | string;
  leaseAssetDefinitionId: string;
  baseFee: QuantityInput;
}

export interface SoracloudManifestProvenance {
  signer: string;
  signature: string;
}

export interface SoracloudHfSharedLeaseJoinPayload {
  repo_id: string;
  revision: string;
  service_name: string;
  apartment_name: string | null;
  storage_class: SoracloudStorageClass;
  lease_term_ms: number;
  lease_asset_definition_id: string;
  base_fee: string;
}

export interface SoracloudSigningPayload<
  TPayload,
  TSchema extends string,
  TLabel extends string,
> {
  schema: TSchema;
  label: TLabel;
  payload: TPayload;
}

export interface SoracloudHfSharedLeaseJoinDraft {
  payload: SoracloudHfSharedLeaseJoinPayload;
  provenancePayloads: {
    join: SoracloudSigningPayload<
      SoracloudHfSharedLeaseJoinPayload,
      "soracloud.hf.shared_lease_join.provenance.v1",
      "hf_shared_lease_join"
    >;
  };
}

export interface SoracloudHfSharedLeaseJoinRequest {
  payload: SoracloudHfSharedLeaseJoinDraft["payload"];
  provenance: SoracloudManifestProvenance;
}

export interface SoracloudAppInfraRouteInput {
  path: string;
  publicHost: string | null;
  internalUrl: string | null;
}

export interface SoracloudAppInfraLeaseVolumeInput {
  name: string;
  mountPath: string;
  maxTotalBytes: NumericLike;
  temperature: "hot" | "warm" | "cold";
}

export interface SoracloudAppInfraShardInput {
  count: NumericLike;
  shardIdEnv: string;
  shardCountEnv: string;
}

export interface SoracloudAppInfraServiceInput {
  name: string;
  serviceVersion: string;
  serviceManifestHash: string;
  containerManifestHash: string;
  runtime: "Inrou" | "Ivm";
  executionPlane: "HttpService" | "DeterministicService";
  routes: ReadonlyArray<SoracloudAppInfraRouteInput>;
  leaseVolumes: ReadonlyArray<SoracloudAppInfraLeaseVolumeInput>;
  shards: SoracloudAppInfraShardInput | null;
}

export interface SoracloudAppInfraStaticSiteInput {
  publicUrl: string;
  contentCid: string | null;
  manifestDigestHex: string | null;
  mountPath: string;
  apiBasePath: string | null;
}

export interface SoracloudAppInfraDraftInput {
  appName: string;
  appVersion: string;
  publicUrl: string;
  staticSite: SoracloudAppInfraStaticSiteInput | null;
  services: ReadonlyArray<SoracloudAppInfraServiceInput>;
}

export interface SoracloudAppInfraRouteV1 {
  schema_version: 1;
  public_host: string | null;
  path_prefix: string;
  internal_url: string | null;
}

export interface SoracloudAppInfraStaticSiteV1 {
  schema_version: 1;
  public_url: string;
  content_cid: string | null;
  manifest_digest_hex: string | null;
  mount_path: string;
  api_base_path: string | null;
}

export interface SoracloudAppInfraServiceV1 {
  schema_version: 1;
  service_name: string;
  service_version: string;
  service_manifest_hash: string;
  container_manifest_hash: string;
  execution_plane: {
    execution_plane: "HttpService" | "DeterministicService";
    value: null;
  };
  runtime: { runtime: "Inrou" | "Ivm"; value: null };
  routes: SoracloudAppInfraRouteV1[];
  lease_volumes: string[];
  shard: string | null;
}

export interface SoracloudAppInfraPayload {
  schema_version: 1;
  app_name: string;
  app_version: string;
  public_url: string;
  static_site: SoracloudAppInfraStaticSiteV1 | null;
  services: SoracloudAppInfraServiceV1[];
}

export interface SoracloudAppInfraDraft {
  payload: SoracloudAppInfraPayload;
  provenancePayloads: {
    deploy: SoracloudSigningPayload<
      SoracloudAppInfraPayload,
      "soracloud.app.infra.provenance.v1",
      "app_infra_deploy"
    >;
    services: Array<
      SoracloudSigningPayload<
        SoracloudAppInfraServiceV1,
        "soracloud.app.infra.provenance.v1",
        "app_infra_service"
      >
    >;
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

export function buildSoracloudHfSharedLeaseJoinDraft(
  input: SoracloudHfSharedLeaseJoinDraftInput,
): SoracloudHfSharedLeaseJoinDraft;

export function buildSoracloudAppInfraDraft(
  input: SoracloudAppInfraDraftInput,
): SoracloudAppInfraDraft;

export interface SoracloudAppInfraRequest {
  deploy_services: unknown[];
  upgrade_services: unknown[];
  manifest: SoracloudAppInfraDraft["payload"];
  provenance: SoracloudManifestProvenance;
}

export interface SoracloudMutationDraftResponseV1 {
  ok: true;
  authority: string;
  signed_by: string;
  tx_instructions: [SoracloudTxInstruction, ...SoracloudTxInstruction[]];
}

export interface SoracloudAppInfraStateV1 {
  schema_version: 1;
  app_name: string;
  current_app_version: string;
  current_manifest_hash: string;
  revision_count: number;
  deployed_sequence: number;
  updated_sequence: number;
  manifest: SoracloudAppInfraPayload;
}

export interface SoracloudAppInfraAuditEventV1 {
  schema_version: 1;
  sequence: number;
  action: { action: "Deploy" | "Upgrade"; value: null };
  app_name: string;
  from_version: string | null;
  to_version: string;
  app_manifest_hash: string;
  service_count: number;
  signer: string;
}

export interface SoracloudAppInfraStatusResponseV1 {
  schema_version: 1;
  app_count: number;
  audit_event_count: number;
  apps: SoracloudAppInfraStateV1[];
  recent_audit_events: SoracloudAppInfraAuditEventV1[];
}

export function assembleSoracloudAppInfraRequest(
  draft: SoracloudAppInfraDraft,
  provenances: { deploy: SoracloudManifestProvenance },
  options: { deployServices: unknown[]; upgradeServices: unknown[] },
): SoracloudAppInfraRequest;

export function deploySoracloudAppInfraInstruction(
  manifest: Record<string, unknown>,
  provenance: SoracloudManifestProvenance,
): { wire_id: string; payload: Record<string, unknown> };

export function upgradeSoracloudAppInfraInstruction(
  manifest: Record<string, unknown>,
  provenance: SoracloudManifestProvenance,
): { wire_id: string; payload: Record<string, unknown> };

export function assembleSoracloudHfSharedLeaseJoinRequest(
  draft: SoracloudHfSharedLeaseJoinDraft,
  provenances: { join: SoracloudManifestProvenance },
): SoracloudHfSharedLeaseJoinRequest;

export interface SoracloudTxInstruction {
  wire_id: string;
  /** Non-empty lowercase hexadecimal with an even number of digits. */
  payload_hex: string;
}

export interface SoracloudUploadedModelBundleV1 {
  schema_version: 1;
  service_name: string;
  model_id: string;
  weight_version: string;
  family: string;
  modalities: string[];
  plaintext_root: string;
  package_format: {
    package_format: "NormalizedHuggingFaceSafetensorsV1";
    value: null;
  };
  bundle_root: string;
  sorafs_manifest_digest: number[];
  chunk_count: number;
  plaintext_bytes: number;
  ciphertext_bytes: number;
  chunk_manifest_root: string;
  pricing_policy: { storage_price: unknown };
}

export interface SoracloudModelArtifactStatusEntryV1 {
  service_name: string;
  model_name: string;
  artifact_id: string;
  training_job_id: string;
  weight_version: string | null;
  weight_artifact_hash: string;
  dataset_ref: string;
  training_config_hash: string;
  reproducibility_hash: string;
  provenance_attestation_hash: string;
  registered_sequence: number;
  consumed_by_version: string | null;
  chunk_manifest_root: string | null;
}

export interface SoracloudUploadedModelStatusV1 {
  schema_version: 1;
  bundle: SoracloudUploadedModelBundleV1;
  artifact: SoracloudModelArtifactStatusEntryV1 | null;
}

export class NumericV1Error extends Error {
  readonly code: string;
}

export class KotodamaInt {
  constructor(value: bigint | string);
  readonly value: bigint;
  toString(): string;
}

export class KotodamaDecimal {
  constructor(value: string);
  constructor(mantissa: bigint | string, scale: number);
  readonly mantissa: bigint;
  readonly scale: number;
  toString(): string;
}

export class KotodamaQuantity {
  constructor(value: string);
  constructor(mantissa: bigint | string, scale: number);
  readonly mantissa: bigint;
  readonly scale: number;
  toString(): string;
}

export const NumericV1: {
  readonly INT_MIN: bigint;
  readonly INT_MAX: bigint;
  readonly MAX_MANTISSA_BYTES: 64;
  readonly MAX_SCALE: 28;
  // BEGIN GENERATED: kotodama-v1-numeric-policy
  readonly schemas: {
    readonly int: {
      readonly name: "iroha.numeric.IntValueV1";
      readonly hash: string;
      readonly pointerType: 0x0011;
      readonly scaled: false;
    };
    readonly decimal: {
      readonly name: "iroha.numeric.DecimalValueV1";
      readonly hash: string;
      readonly pointerType: 0x0012;
      readonly scaled: true;
    };
    readonly quantity: {
      readonly name: "iroha.numeric.QuantityValueV1";
      readonly hash: string;
      readonly pointerType: 0x0010;
      readonly scaled: true;
    };
  };
  // END GENERATED: kotodama-v1-numeric-policy
  encodeIntFrame(value: KotodamaInt | bigint | string): Uint8Array;
  encodeDecimalFrame(value: KotodamaDecimal | string): Uint8Array;
  encodeQuantityFrame(value: KotodamaQuantity | bigint | string): Uint8Array;
  decodeIntFrame(value: ArrayBuffer | ArrayBufferView): KotodamaInt;
  decodeDecimalFrame(value: ArrayBuffer | ArrayBufferView): KotodamaDecimal;
  decodeQuantityFrame(value: ArrayBuffer | ArrayBufferView): KotodamaQuantity;
  encodeIntEnvelope(value: KotodamaInt | bigint | string): Uint8Array;
  encodeDecimalEnvelope(value: KotodamaDecimal | string): Uint8Array;
  encodeQuantityEnvelope(value: KotodamaQuantity | bigint | string): Uint8Array;
  decodeIntEnvelope(value: ArrayBuffer | ArrayBufferView): KotodamaInt;
  decodeDecimalEnvelope(value: ArrayBuffer | ArrayBufferView): KotodamaDecimal;
  decodeQuantityEnvelope(value: ArrayBuffer | ArrayBufferView): KotodamaQuantity;
  encodeIntJson(value: KotodamaInt | bigint | string): string;
  encodeDecimalJson(value: KotodamaDecimal | string): string;
  encodeQuantityJson(value: KotodamaQuantity | bigint | string): string;
  decodeIntJson(value: string): KotodamaInt;
  decodeDecimalJson(value: string): KotodamaDecimal;
  decodeQuantityJson(value: string): KotodamaQuantity;
};

export * from "./nexus-app.js";
export * from "./transaction-codec.js";
export * from "./smart-contract-deployment.js";
