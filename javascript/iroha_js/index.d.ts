/// <reference types="node" />

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface KeyPair {
  algorithm: "ed25519";
  publicKey: Buffer;
  privateKey: Buffer;
}

export interface Sm2KeyPair {
  algorithm: "sm2";
  distid: string;
  publicKey: Buffer;
  privateKey: Buffer;
}

export const SM2_PRIVATE_KEY_LENGTH: number;
export const SM2_PUBLIC_KEY_LENGTH: number;
export const SM2_SIGNATURE_LENGTH: number;
export const SM2_DEFAULT_DISTINGUISHED_ID: string;

export interface SignedTransactionResult {
  signedTransaction: Buffer;
  hash: Buffer;
}

export type AccountAddressFormat = "ih58" | "compressed";

export const AccountAddressErrorCode: {
  readonly UNSUPPORTED_ALGORITHM: "ERR_UNSUPPORTED_ALGORITHM";
  readonly KEY_PAYLOAD_TOO_LONG: "ERR_KEY_PAYLOAD_TOO_LONG";
  readonly INVALID_HEADER_VERSION: "ERR_INVALID_HEADER_VERSION";
  readonly INVALID_NORM_VERSION: "ERR_INVALID_NORM_VERSION";
  readonly INVALID_IH58_PREFIX: "ERR_INVALID_IH58_PREFIX";
  readonly CANONICAL_HASH_FAILURE: "ERR_CANONICAL_HASH_FAILURE";
  readonly INVALID_IH58_ENCODING: "ERR_INVALID_IH58_ENCODING";
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
  readonly INVALID_IH58_PREFIX_ENCODING: "ERR_INVALID_IH58_PREFIX_ENCODING";
  readonly MISSING_COMPRESSED_SENTINEL: "ERR_MISSING_COMPRESSED_SENTINEL";
  readonly COMPRESSED_TOO_SHORT: "ERR_COMPRESSED_TOO_SHORT";
  readonly INVALID_COMPRESSED_CHAR: "ERR_INVALID_COMPRESSED_CHAR";
  readonly INVALID_COMPRESSED_BASE: "ERR_INVALID_COMPRESSED_BASE";
  readonly INVALID_COMPRESSED_DIGIT: "ERR_INVALID_COMPRESSED_DIGIT";
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

export type AccountAddressDomainKind = "default" | "local12" | "global" | "unknown";

export interface AccountAddressDomainSelector {
  tag: number | null;
  digestHex: string | null;
  registryId: number | null;
  label: string | null;
}

export interface AccountAddressDomainSummary {
  kind: AccountAddressDomainKind;
  warning: string | null;
  selector: AccountAddressDomainSelector;
}

export interface AccountAddressDisplay {
  ih58: string;
  compressed: string;
  networkPrefix: number;
  compressedWarning: string;
  domainSummary: AccountAddressDomainSummary;
}

export interface CurveSupportOptions {
  allowMlDsa?: boolean;
  allowGost?: boolean;
  allowSm2?: boolean;
}

export function configureCurveSupport(options?: CurveSupportOptions): void;

export class AccountAddress {
  static fromAccount(
    options: {
      domain: string;
      publicKey: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView | number[] | string;
      algorithm?: string;
    },
  ): AccountAddress;
  static fromAccount(
    options: {
      registryId: number | string | bigint;
      publicKey: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView | number[] | string;
      algorithm?: string;
    },
  ): AccountAddress;
  static fromCanonicalBytes(
    bytes: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
  ): AccountAddress;
  static fromIH58(
    encoded: string,
    expectedPrefix?: number | string | bigint,
  ): AccountAddress;
  static fromCompressedSora(encoded: string): AccountAddress;
  static parseEncoded(
    input: string,
    expectedPrefix?: number | string | bigint,
    expectedDomain?: string,
  ): { address: AccountAddress; format: AccountAddressFormat; networkPrefix?: number };
  canonicalBytes(): Uint8Array;
  canonicalHex(): string;
  toIH58(networkPrefix: number | string | bigint): string;
  toCompressedSora(): string;
  toCompressedSoraFullWidth(): string;
  toString(): string;
  displayFormats(networkPrefix?: number | string | bigint): AccountAddressDisplay;
  domainSummary(): AccountAddressDomainSummary;
}

export function encodeCompressedAccountAddress(
  canonicalBytes: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView,
  options?: { fullWidth?: boolean },
): string;
export function decodeCompressedAccountAddress(encoded: string): Uint8Array;

export interface InspectAccountIdOptions {
  networkPrefix?: number | string | bigint;
  expectPrefix?: number | string | bigint;
}

export interface AccountIdInspection {
  detectedFormat: { kind: string; networkPrefix?: number };
  domain: { kind: string; warning: string | null };
  canonicalHex: string;
  ih58: { value: string; networkPrefix: number };
  compressed: string;
  compressedWarning: string;
  inputDomain: string | null;
  warnings: string[];
}

export function inspectAccountId(
  accountId: string,
  options?: InspectAccountIdOptions,
): AccountIdInspection;

export const DEFAULT_DOMAIN_NAME: string;

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
  previewProposalExpiry(options?: { requestedTtlMs?: number | bigint | null; nowMs?: number | bigint }): MultisigProposalTtlPreview;
  enforceProposalTtl(options?: { requestedTtlMs?: number | bigint | null; nowMs?: number | bigint }): MultisigProposalTtlPreview;
  toPayload(): { signatories: Record<string, number>; quorum: number; transaction_ttl_ms: number };
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

export interface SoradnsGatewayHosts {
  readonly normalizedName: string;
  readonly canonicalLabel: string;
  readonly canonicalHost: string;
  readonly canonicalWildcard: string;
  readonly prettyHost: string;
  readonly hostPatterns: ReadonlyArray<string>;
  matchesHost(host: string): boolean;
}

export function deriveSoradnsGatewayHosts(fqdn: string): SoradnsGatewayHosts;
export function hostPatternsCoverDerivedHosts(
  patterns: Iterable<string>,
  derived: SoradnsGatewayHosts,
): boolean;
export function canonicalGatewaySuffix(): string;
export function canonicalGatewayWildcard(): string;
export function prettyGatewaySuffix(): string;

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
export function buildSamplePacs008Message(options?: SampleIsoMessageOptions): string;
export function buildSamplePacs009Message(options?: SampleIsoMessageOptions): string;
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
export function buildSampleCamt052Message(options?: SampleCamtMessageOptions): string;
export function buildCamt056Message(options: BuildCamt056Options): string;
export function buildSampleCamt056Message(options?: SampleCamtMessageOptions): string;

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

export type VerifyingKeyIdLike = string | { backend: string; name: string };

export interface ConfidentialEncryptedPayloadInput {
  version?: number;
  ephemeralPublicKey: BinaryLike;
  nonce: BinaryLike;
  ciphertext: BinaryLike;
}

export interface ProofAttachmentInput {
  backend: string;
  proof: BinaryLike;
  verifyingKeyRef?: VerifyingKeyIdLike | null;
  verifyingKeyInline?: {
    backend: string;
    bytes: BinaryLike;
  } | null;
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

/**
 * Canonicalise an account identifier to IH58.
 *
 * Accepts IH58/sora encoded account IDs plus alias forms (`<alias>@domain`).
 * Canonical hex account literals are rejected.
 */
export function normalizeAccountId(value: string, name?: string): string;

/**
 * Canonicalise an asset identifier, including embedded account components.
 */
export function normalizeAssetId(value: string, name?: string): string;

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
  offsetHeight?: NumericLike;
  limit?: NumericLike;
}

export interface EventStreamOptions {
  filter?: string | Record<string, unknown>;
  lastEventId?: string;
  signal?: AbortSignal;
}

export type ToriiAddressFormat = "ih58" | "canonical" | "compressed";

export interface CanonicalRequestAuth {
  accountId: string;
  privateKey: Buffer | Uint8Array | ArrayBuffer | ArrayBufferView | string | number[];
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
  addressFormat?: ToriiAddressFormat;
  signal?: AbortSignal;
}

export interface AccountAssetListOptions extends IterableListOptions {
  assetId?: string;
}

export interface AccountTransactionListOptions extends IterableListOptions {
  assetId?: string;
}

export interface AssetHolderListOptions extends IterableListOptions {
  assetId?: string;
}

export interface OfflineAllowanceListOptions extends IterableListOptions {
  controllerId?: string;
  assetId?: string;
  certificateExpiresBeforeMs?: NumericLike;
  certificateExpiresAfterMs?: NumericLike;
  policyExpiresBeforeMs?: NumericLike;
  policyExpiresAfterMs?: NumericLike;
  refreshBeforeMs?: NumericLike;
  refreshAfterMs?: NumericLike;
  verdictIdHex?: string;
  attestationNonceHex?: string;
  requireVerdict?: boolean;
  onlyMissingVerdict?: boolean;
  includeExpired?: boolean;
}

export interface OfflineTransferListOptions extends IterableListOptions {
  controllerId?: string;
  receiverId?: string;
  depositAccountId?: string;
  assetId?: string;
  certificateIdHex?: string;
  certificateExpiresBeforeMs?: NumericLike;
  certificateExpiresAfterMs?: NumericLike;
  policyExpiresBeforeMs?: NumericLike;
  policyExpiresAfterMs?: NumericLike;
  refreshBeforeMs?: NumericLike;
  refreshAfterMs?: NumericLike;
  verdictIdHex?: string;
  attestationNonceHex?: string;
  requireVerdict?: boolean;
  onlyMissingVerdict?: boolean;
  platformPolicy?: string;
}

export interface IterableQueryOptions extends IterableListOptions {
  fetchSize?: NumericLike;
  queryName?: string;
  select?: ReadonlyArray<Record<string, unknown>>;
}

export interface PaginationIteratorOptions extends IterableListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
}

export interface AccountAssetIteratorOptions extends PaginationIteratorOptions {
  assetId?: string;
}

export interface AccountTransactionIteratorOptions extends PaginationIteratorOptions {
  assetId?: string;
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
  addressFormat?: ToriiAddressFormat;
  signal?: AbortSignal;
}

export interface ExplorerNftIteratorOptions extends ExplorerNftListOptions {
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

export interface SubscriptionPlanIteratorOptions extends SubscriptionPlanListOptions {
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

export interface AppendSumeragiTelemetryOptions extends CaptureSumeragiTelemetryOptions {
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
export type ToriiAssetDefinitionListItem = ToriiAccountListItem;
export interface ToriiNftListItem {
  id: string;
}
export interface ToriiAccountAssetItem {
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
  private_key: string;
  backend: string;
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

export interface ToriiExplorerNft {
  id: string;
  ownedBy: string;
  metadata: Record<string, unknown>;
}

export interface ToriiExplorerNftsPage {
  pagination: ToriiExplorerPaginationMeta;
  items: ReadonlyArray<ToriiExplorerNft>;
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
  addressFormat: "ih58" | "compressed";
  networkPrefix: number;
  errorCorrection: string;
  modules: number;
  qrVersion: number;
  svg: string;
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

export interface SnsCaseExportOptions {
  since?: string | null;
  status?: string | null;
  limit?: number | null;
  signal?: AbortSignal;
}

export type SnsGovernanceCaseDisputeType = "ownership" | "policy_violation" | "abuse" | "billing" | "other";
export type SnsGovernanceCasePriority = "urgent" | "high" | "standard" | "info";
export type SnsGovernanceCaseStatus =
  | "open"
  | "triage"
  | "hearing"
  | "decision"
  | "remediation"
  | "closed"
  | "suspended";
export type SnsGovernanceCaseReporterRole = "registrar" | "steward" | "guardian" | "public" | "support";
export type SnsGovernanceCaseRespondentRole = "registrant" | "controller" | "registrar" | "steward" | "other";
export type SnsGovernanceCaseEvidenceKind = "document" | "screenshot" | "log" | "governance" | "other";
export type SnsGovernanceCaseDecisionFinding = "upheld" | "rejected" | "partial" | "withdrawn";
export type SnsGovernanceCasePublicationState = "public" | "redacted" | "sealed";

export interface SnsGovernanceCaseSelector {
  suffixId: number;
  label: string;
  globalForm: string;
}

export interface SnsGovernanceCaseReporter {
  role: SnsGovernanceCaseReporterRole;
  contact: string;
  referenceTicket: string | null;
}

export interface SnsGovernanceCaseRespondent {
  role: SnsGovernanceCaseRespondentRole;
  accountId: string;
  contact: string | null;
}

export interface SnsGovernanceCaseAllegation {
  code: string;
  summary: string | null;
  policyReference: string | null;
}

export interface SnsGovernanceCaseEvidence {
  id: string;
  kind: SnsGovernanceCaseEvidenceKind;
  uri: string | null;
  hashHex: string;
  description: string | null;
  sealed: boolean;
}

export interface SnsGovernanceCaseSlaExtension {
  approvedBy: string;
  reason: string;
  newResolutionBy: string;
}

export interface SnsGovernanceCaseSla {
  acknowledgeBy: string;
  resolutionBy: string;
  extensions: ReadonlyArray<SnsGovernanceCaseSlaExtension>;
}

export interface SnsGovernanceCaseAction {
  timestamp: string;
  actor: string;
  action: string;
  notes: string | null;
}

export interface SnsGovernanceCaseDecision {
  finding: SnsGovernanceCaseDecisionFinding | null;
  remedies: ReadonlyArray<string>;
  effectiveAt: string | null;
  publicationState: SnsGovernanceCasePublicationState | null;
}

export interface SnsGovernanceCase {
  caseId: string;
  selector: SnsGovernanceCaseSelector;
  disputeType: SnsGovernanceCaseDisputeType;
  priority: SnsGovernanceCasePriority;
  reportedAt: string;
  acknowledgedAt: string | null;
  triageStartedAt: string | null;
  hearingScheduledAt: string | null;
  resolutionIssuedAt: string | null;
  status: SnsGovernanceCaseStatus;
  reporter: SnsGovernanceCaseReporter;
  respondents: ReadonlyArray<SnsGovernanceCaseRespondent>;
  allegations: ReadonlyArray<SnsGovernanceCaseAllegation>;
  evidence: ReadonlyArray<SnsGovernanceCaseEvidence>;
  sla: SnsGovernanceCaseSla;
  actions: ReadonlyArray<SnsGovernanceCaseAction>;
  decision: SnsGovernanceCaseDecision | null;
}

export interface SnsGovernanceCaseExportResult {
  cases: ReadonlyArray<SnsGovernanceCase>;
  nextSince: string | null;
  nextCursor: string | null;
  totalCount: number | null;
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
  content: string | null;
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

export type ToriiEventPayload = ToriiPipelineEvent | ToriiDataEvent | ToriiOtherEvent;

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

  telemetryAttributes(options?: ConnectErrorTelemetryOptions): Record<string, string>;
  toConnectError(): ConnectError;
}

export type ConnectQueueErrorKind = "overflow" | "expired";

export class ConnectQueueError extends Error implements ConnectErrorConvertible {
  constructor(kind: ConnectQueueErrorKind, options?: { limit?: number; ttlMs?: number });

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
  storage?: "auto" | "memory";
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
  readonly fallbackError?: unknown;
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

export type OfflineCounterPlatform = "apple_key" | "android_series";

export const OfflineCounterPlatform: {
  readonly APPLE_KEY: "apple_key";
  readonly ANDROID_SERIES: "android_series";
};

export class OfflineCounterJournalError extends Error {
  constructor(message?: string, options?: { code?: string; context?: string; cause?: unknown });
  readonly code?: string;
  readonly context?: string;
}

export interface OfflineCounterCheckpoint {
  certificateIdHex: string;
  controllerId: string;
  controllerDisplay: string | null;
  summaryHashHex: string;
  appleKeyCounters: Record<string, number>;
  androidSeriesCounters: Record<string, number>;
  recordedAtMs: number;
}

export interface OfflineCounterJournalOptions {
  storage?: "file" | "memory";
  storagePath?: string;
}

export interface OfflineCounterJournalUpsertOptions {
  recordedAtMs?: number;
  allowSummaryHashMismatch?: boolean;
}

export interface OfflineCounterJournalUpdateParams {
  certificateIdHex: string;
  controllerId: string;
  controllerDisplay?: string | null;
  platform: OfflineCounterPlatform | string;
  scope: string;
  counter: number | bigint;
  recordedAtMs?: number;
}

export interface OfflineCounterJournalAdvanceParams {
  certificateIdHex: string;
  controllerId: string;
  controllerDisplay?: string | null;
  platformProof: Record<string, unknown>;
  recordedAtMs?: number;
}

export class OfflineCounterJournal {
  constructor(options?: OfflineCounterJournalOptions);
  static computeSummaryHashHex(
    appleKeyCounters: Record<string, number>,
    androidSeriesCounters: Record<string, number>,
  ): string;
  static computeSummaryHash(
    appleKeyCounters: Record<string, number>,
    androidSeriesCounters: Record<string, number>,
  ): Uint8Array;
  readonly storage: "file" | "memory";
  readonly storagePath: string | null;
  upsert(
    summaries:
      | ReadonlyArray<ToriiOfflineSummaryItem>
      | ToriiOfflineSummaryListResponse
      | ReadonlyArray<Record<string, unknown>>
      | { items: ReadonlyArray<Record<string, unknown>> },
    options?: OfflineCounterJournalUpsertOptions,
  ): Promise<OfflineCounterCheckpoint[]>;
  updateCounter(params: OfflineCounterJournalUpdateParams): Promise<OfflineCounterCheckpoint>;
  advanceCounterFromProof(
    params: OfflineCounterJournalAdvanceParams,
  ): Promise<OfflineCounterCheckpoint>;
  checkpoint(certificateIdHex: string): Promise<OfflineCounterCheckpoint | null>;
  snapshot(): Promise<OfflineCounterCheckpoint[]>;
  clear(): Promise<void>;
  refresh(): Promise<void>;
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
  rolloutPhase?: "canary" | "ramp" | "default" | "stage-a" | "stage-b" | "stage-c";
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
  allowSingleSourceFallback?: boolean;
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

export type SorafsGatewayProviderMix = "mixed" | "direct-only" | "gateway-only" | "none";

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
  allowSingleSourceFallback: boolean;
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
  constructor(
    payload?: Record<string, unknown>,
    original?: Error | null,
  );
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
export const Offline: typeof import("./src/offlineEnvelope.js");
export const OfflineCounters: typeof import("./src/offlineCounterJournal.js");
export const OfflineQrStream: typeof import("./src/offlineQrStream.js");
export const OfflinePetalStream: typeof import("./src/offlinePetalStream.js");

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
  getPuzzleConfig(options?: SoranetPuzzleMintOptions): Promise<SoranetPuzzleConfigSnapshot>;
  mintPuzzleTicket(options?: SoranetPuzzleMintOptions): Promise<SoranetPuzzleTicketResponse>;
  getTokenConfig(options?: SoranetPuzzleMintOptions): Promise<SoranetTokenConfigSnapshot>;
  mintAdmissionToken(
    transcriptHashHex: string,
    options?: SoranetTokenMintOptions,
  ): Promise<SoranetAdmissionTokenResponse>;
}

export interface ToriiClientConfigSource extends ToriiClientRetryOptions {
  retryTelemetryHook?: (event: ToriiRetryTelemetryEvent) => void;
  insecureTransportTelemetryHook?: (event: InsecureTransportTelemetryEvent) => void;
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
  insecureTransportTelemetryHook: ((event: InsecureTransportTelemetryEvent) => void) | null;
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
  insecureTransportTelemetryHook?: (event: InsecureTransportTelemetryEvent) => void;
}

export interface TransactionStatusPollOptions {
  signal?: AbortSignal;
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
  pingMissTotal: number;
}

export interface ConnectSessionResponse {
  sid: string;
  wallet_uri: string;
  app_uri: string;
  token_app: string;
  token_wallet: string;
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

export type ConnectQueueState = "healthy" | "throttled" | "quarantined" | "disabled";

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

export function defaultConnectQueueRoot(options?: ConnectQueueRootOptions): string;

export function deriveConnectSessionDirectory(
  options: { sid: BinaryLike | string } & ConnectQueueRootOptions,
): string;

export function readConnectQueueSnapshot(options: {
  sid?: BinaryLike | string;
  snapshotPath?: string;
  warningWatermark?: number;
  dropWatermark?: number;
} & ConnectQueueRootOptions): Promise<{ snapshot: ConnectQueueSnapshot; statePath: string }>;

export function writeConnectQueueSnapshot(
  snapshot: ConnectQueueSnapshot,
  options?: ConnectQueueRootOptions & { sid?: BinaryLike | string },
): Promise<{ snapshot: ConnectQueueSnapshot; statePath: string }>;

export function updateConnectQueueSnapshot(
  sid: BinaryLike | string,
  updater:
    | Partial<ConnectQueueSnapshot>
    | ((snapshot: ConnectQueueSnapshot) => ConnectQueueSnapshot | void),
  options?: ConnectQueueRootOptions & { warningWatermark?: number; dropWatermark?: number },
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

export interface BootstrapConnectPreviewOptions extends ConnectSessionPreviewOptions {
  register?: boolean;
  sessionOptions?: {
    node?: string | null;
  } | null;
}

export interface BootstrapConnectPreviewResult {
  preview: ConnectSessionPreview;
  session: ConnectSessionResponse | null;
  tokens: { wallet: string; app: string } | null;
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

export interface ConnectWebSocketDialOptions<T = unknown> extends ConnectWebSocketParams {
  baseUrl: string;
  protocols?: ConnectWebSocketProtocols;
  websocketOptions?: unknown;
  WebSocketImpl?: ConnectWebSocketConstructor<T>;
  insecureTransportTelemetryHook?: (event: InsecureTransportTelemetryEvent) => void;
}

export interface ClientConnectWebSocketOptions<T = unknown> extends ConnectWebSocketParams {
  protocols?: ConnectWebSocketProtocols;
  websocketOptions?: unknown;
  WebSocketImpl?: ConnectWebSocketConstructor<T>;
  insecureTransportTelemetryHook?: (event: InsecureTransportTelemetryEvent) => void;
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
  namespace: string;
  contract_id: string;
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

export type ToriiGovernanceProposalStatus = "Proposed" | "Approved" | "Rejected" | "Enacted";

export interface ToriiGovernanceDeployContractProposal {
  namespace: string;
  contract_id: string;
  code_hash_hex: string;
  abi_hash_hex: string;
  abi_version: string;
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

export type ToriiGovernanceBallotDirection = "Aye" | "Nay" | "Abstain";

export interface ToriiGovernanceDeployContractProposalRequest {
  namespace: string;
  contractId: string;
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

export interface ToriiGovernanceBallotResponse extends ToriiGovernanceDraftResponse {
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

export interface ToriiOfflineAllowanceItem {
  certificate_id_hex: string;
  controller_id: string;
  controller_display: string;
  asset_id: string;
  registered_at_ms: number;
  expires_at_ms: number;
  policy_expires_at_ms: number;
  refresh_at_ms: number | null;
  verdict_id_hex: string | null;
  attestation_nonce_hex: string | null;
  remaining_amount: string;
  deadline_kind?: string | null;
  deadline_state?: string | null;
  deadline_ms?: number | null;
  deadline_ms_remaining?: number | null;
  record: Record<string, unknown>;
  integrity_metadata: ToriiOfflineIntegrityMetadata | null;
}

export interface ToriiOfflineAllowanceListResponse {
  items: ReadonlyArray<ToriiOfflineAllowanceItem>;
  total: number;
}

export interface ToriiOfflineWalletCertificateDraft {
  controller: string;
  allowance: {
    asset: string;
    amount: string;
    commitment: ReadonlyArray<number>;
  };
  spend_public_key: string;
  attestation_report: ReadonlyArray<number>;
  issued_at_ms: number;
  expires_at_ms: number;
  policy: {
    max_balance: string;
    max_tx_value: string;
    expires_at_ms: number;
  };
  metadata?: Record<string, unknown>;
  verdict_id?: string | null;
  attestation_nonce?: string | null;
  refresh_at_ms?: number | null;
}

export interface ToriiOfflineWalletCertificate {
  controller: string;
  operator: string;
  allowance: {
    asset: string;
    amount: string;
    commitment: ReadonlyArray<number>;
  };
  spend_public_key: string;
  attestation_report: ReadonlyArray<number>;
  issued_at_ms: number;
  expires_at_ms: number;
  policy: {
    max_balance: string;
    max_tx_value: string;
    expires_at_ms: number;
  };
  metadata?: Record<string, unknown>;
  verdict_id?: string | null;
  attestation_nonce?: string | null;
  refresh_at_ms?: number | null;
  operator_signature: string;
}

export interface ToriiOfflineCertificateIssueResponse {
  certificate_id_hex: string;
  certificate: ToriiOfflineWalletCertificate;
}

export interface ToriiOfflineAllowanceRegisterRequest {
  authority: string;
  certificate: ToriiOfflineWalletCertificate;
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

export interface ToriiOfflineAllowanceRegisterResponse {
  certificate_id_hex: string;
}

export interface ToriiOfflineTopUpRequest {
  authority: string;
  certificate: ToriiOfflineWalletCertificateDraft;
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

export interface ToriiOfflineTopUpResponse {
  certificate: ToriiOfflineCertificateIssueResponse;
  registration: ToriiOfflineAllowanceRegisterResponse;
}

export interface ToriiOfflineSettlementBuildClaimOverride {
  tx_id_hex?: string;
  txIdHex?: string;
  app_id?: string | null;
  appId?: string | null;
  build_number?: number | null;
  buildNumber?: number | null;
  issued_at_ms?: number | null;
  issuedAtMs?: number | null;
  expires_at_ms?: number | null;
  expiresAtMs?: number | null;
}

export interface ToriiOfflineSettlementSubmitRequest {
  authority: string;
  transfer: Record<string, unknown>;
  build_claim_overrides?: ReadonlyArray<ToriiOfflineSettlementBuildClaimOverride>;
  buildClaimOverrides?: ReadonlyArray<ToriiOfflineSettlementBuildClaimOverride>;
  repair_existing_build_claims?: boolean;
  repairExistingBuildClaims?: boolean;
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

export interface ToriiOfflineSettlementSubmitResponse {
  bundle_id_hex: string;
  transaction_hash_hex?: string | null;
}

export interface ToriiOfflineBuildClaimIssueRequest {
  certificate_id_hex?: string;
  certificateIdHex?: string;
  tx_id_hex?: string;
  txIdHex?: string;
  platform: string;
  app_id?: string | null;
  appId?: string | null;
  build_number?: number | null;
  buildNumber?: number | null;
  issued_at_ms?: number | null;
  issuedAtMs?: number | null;
  expires_at_ms?: number | null;
  expiresAtMs?: number | null;
}

export type ToriiOfflineBuildClaimPlatform = "Apple" | "Android";

export interface ToriiOfflineBuildClaim {
  claim_id: string;
  nonce: string;
  platform: ToriiOfflineBuildClaimPlatform;
  app_id: string;
  build_number: number;
  issued_at_ms: number;
  expires_at_ms: number;
  lineage_scope?: string;
  operator_signature: string;
}

export interface ToriiOfflineBuildClaimIssueResponse {
  claim_id_hex: string;
  build_claim: ToriiOfflineBuildClaim;
}

export interface ToriiOfflineIntegrityMetadata {
  policy: string;
  provisioned?: ToriiProvisionedIntegrityMetadata;
  play_integrity?: ToriiPlayIntegrityMetadata;
  hms_safety_detect?: ToriiHmsSafetyDetectMetadata;
}

export interface ToriiProvisionedIntegrityMetadata {
  inspector_public_key: string;
  manifest_schema: string;
  manifest_version: number | null;
  max_manifest_age_ms: number | null;
  manifest_digest_hex: string | null;
}

export interface ToriiPlayIntegrityMetadata {
  cloud_project_number: number;
  environment: string;
  package_names: ReadonlyArray<string>;
  signing_digests_sha256: ReadonlyArray<string>;
  allowed_app_verdicts: ReadonlyArray<string>;
  allowed_device_verdicts: ReadonlyArray<string>;
  max_token_age_ms: number | null;
}

export interface ToriiHmsSafetyDetectMetadata {
  app_id: string;
  package_names: ReadonlyArray<string>;
  signing_digests_sha256: ReadonlyArray<string>;
  required_evaluations: ReadonlyArray<string>;
  max_token_age_ms: number | null;
}

export interface ToriiOfflineTransferItem {
  bundle_id_hex: string;
  controller_id: string;
  controller_display: string;
  receiver_id: string;
  receiver_display: string;
  deposit_account_id: string;
  deposit_account_display: string;
  asset_id: string | null;
  receipt_count: number;
  total_amount: string;
  claimed_delta: string;
  status: string;
  recorded_at_ms: number;
  recorded_at_height: number;
  archived_at_height: number | null;
  certificate_id_hex: string | null;
  certificate_expires_at_ms: number | null;
  policy_expires_at_ms: number | null;
  refresh_at_ms: number | null;
  verdict_id_hex: string | null;
  attestation_nonce_hex: string | null;
  platform_policy: string | null;
  platform_token_snapshot: ToriiOfflinePlatformTokenSnapshot | null;
  verdict_snapshot: ToriiOfflineVerdictSnapshot | null;
  status_transitions: ReadonlyArray<ToriiOfflineStatusTransition>;
  transfer: Record<string, unknown>;
  integrity_metadata: ToriiOfflineIntegrityMetadata | null;
}

export interface ToriiOfflinePlatformTokenSnapshot {
  policy: string;
  attestation_jws_b64: string;
}

export interface ToriiOfflineVerdictSnapshot {
  certificate_id: string;
  verdict_id: string | null;
  attestation_nonce: string | null;
  refresh_at_ms: number | null;
  certificate_expires_at_ms: number;
  policy_expires_at_ms: number;
}

export interface ToriiOfflineStatusTransition {
  status: string;
  transitioned_at_ms: number;
  verdict_snapshot: ToriiOfflineVerdictSnapshot | null;
}

export interface ToriiOfflineTransferListResponse {
  items: ReadonlyArray<ToriiOfflineTransferItem>;
  total: number;
}

export interface ToriiOfflineRevocationItem {
  verdict_id_hex: string;
  issuer_id: string;
  issuer_display: string;
  revoked_at_ms: number;
  reason: string;
  note: string | null;
  metadata: Record<string, unknown> | null;
  record: Record<string, unknown>;
}

export interface ToriiOfflineRevocationListResponse {
  items: ReadonlyArray<ToriiOfflineRevocationItem>;
  total: number;
}

export interface ToriiOfflineRejectionStatsEntry {
  platform: string;
  reason: string;
  count: number;
}

export interface ToriiOfflineRejectionStatsResponse {
  items: ReadonlyArray<ToriiOfflineRejectionStatsEntry>;
  total: number;
}

export type ToriiOfflineCounterMap = Record<string, number>;

export interface ToriiOfflineCounterTotals {
  total_counters: number;
  total_weight: number;
  apple: number;
  android: number;
  policy: number;
}

export interface ToriiOfflineSummaryItem {
  certificate_id_hex: string;
  controller_id: string;
  controller_display: string;
  summary_hash_hex: string;
  apple_key_counters: ToriiOfflineCounterMap;
  android_series_counters: ToriiOfflineCounterMap;
  policy_key_counters: ToriiOfflineCounterMap;
  counter_totals: ToriiOfflineCounterTotals | null;
  metadata: Record<string, unknown> | null;
}

export interface ToriiOfflineSummaryListResponse {
  items: ReadonlyArray<ToriiOfflineSummaryItem>;
  total: number;
}

export interface ToriiStatusPayload {
  peers: number;
  queue_size: number;
  commit_time_ms: number;
  da_reschedule_total: number;
  txs_approved: number;
  txs_rejected: number;
  view_changes: number;
  governance: ToriiGovernanceStatusSnapshot | null;
  lane_commitments: ToriiLaneCommitmentSnapshot[];
  dataspace_commitments: ToriiDataspaceCommitmentSnapshot[];
  lane_governance: ToriiLaneGovernanceSnapshot[];
  lane_governance_sealed_total: number;
  lane_governance_sealed_aliases: ReadonlyArray<string>;
  raw: Record<string, unknown>;
}

export interface ToriiStatusMetrics {
  commit_latency_ms: number;
  queue_size: number;
  queue_delta: number;
  da_reschedule_delta: number;
  tx_approved_delta: number;
  tx_rejected_delta: number;
  view_change_delta: number;
  has_activity: boolean;
}

export interface ToriiStatusSnapshot {
  timestamp: number;
  status: ToriiStatusPayload;
  metrics: ToriiStatusMetrics;
}

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
  supportedAbiVersions: ReadonlyArray<number>;
  defaultCompileTarget: number;
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
  activeVersions: ReadonlyArray<number>;
  defaultCompileTarget: number;
}

export interface ToriiRuntimeAbiHashResponse {
  policy: string;
  abiHashHex: string;
}

export interface ToriiRuntimeMetrics {
  activeAbiVersionsCount: number;
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

export interface SumeragiDoubleVoteEvidenceRecord extends SumeragiEvidenceRecordBase {
  kind: "DoublePrepare" | "DoubleCommit";
  phase: "Prepare" | "Commit" | "NewView";
  height: number;
  view: number;
  epoch: number;
  signer: string;
  block_hash_1: string;
  block_hash_2: string;
}

export interface SumeragiInvalidQcEvidenceRecord extends SumeragiEvidenceRecordBase {
  kind: "InvalidQc";
  height: number;
  view: number;
  epoch: number;
  subject_block_hash: string;
  phase: string;
  reason: string;
}

export interface SumeragiInvalidProposalEvidenceRecord extends SumeragiEvidenceRecordBase {
  kind: "InvalidProposal";
  height: number;
  view: number;
  epoch: number;
  subject_block_hash: string;
  payload_hash: string;
  reason: string;
}

export interface SumeragiCensorshipEvidenceRecord extends SumeragiEvidenceRecordBase {
  kind: "Censorship";
  tx_hash: string;
  receipt_count: number;
  min_height: number;
  max_height: number;
  signers: ReadonlyArray<string>;
}

export interface SumeragiUnknownEvidenceRecord extends SumeragiEvidenceRecordBase {
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

export type KaigiRelayEventPayload = KaigiRelayRegistrationEvent | KaigiRelayHealthEvent;

export interface KaigiRelayEventsOptions {
  domain?: string;
  relay?: string;
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
  assetId?: string;
  quantity: NumericLike;
};

type MintTransferSpec = {
  sourceAssetId?: string;
  quantity: NumericLike;
  destinationAccountId: string;
};

type AccountTransferSpec = {
  sourceAssetId: string;
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

export interface MintAssetInput {
  chainId: string;
  authority: string;
  assetId: string;
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
  assetId: string;
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
  sourceAssetId: string;
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

/**
 * Parameters for {@link buildMintAndTransferTransaction}. Provide either
 * `transfer` or `transfers`; when `sourceAssetId` is omitted on a transfer the
 * helper reuses `mint.assetId` and enforces that at least one transfer is
 * present.
 */
interface MintAndTransferInputBase {
  chainId: string;
  authority: string;
  mint: {
    assetId: string;
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

export type MintAndTransferInput = MintAndTransferInputBase & MintAndTransferVariants;

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

export type RegisterDomainAndMintInput = RegisterDomainAndMintInputBase & RegisterDomainMintOptions;

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
 * either `mint` or `mints`. When `assetId` is omitted the helper derives it as
 * `<assetDefinitionId>#<accountId>` and enforces that any provided `assetId`
 * matches the derived value.
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
 * `sourceAssetId` the helper reuses the first minted asset destination.
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

export type KaigiPrivacyModeInput = string | KaigiPrivacyModeValue | null | undefined;

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
  namespace: string;
  contractId: string;
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
  derivedBy?: "Vrf" | "Fallback" | string;
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

export interface UnshieldInstructionInput {
  assetDefinitionId: string;
  destinationAccountId: string;
  publicAmount: NumericLike;
  inputs: ReadonlyArray<BinaryLike>;
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
export type Pacs002StatusCode = "ACTC" | "ACSP" | "ACSC" | "ACWC" | "PDNG" | "RJCT";

export interface IsoMessageSubmissionResponseBase {
  message_id: string;
  status: IsoBridgeStatus;
  pacs002_code: Pacs002StatusCode | null;
  transaction_hash: string | null;
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

export interface IsoPacs008SubmissionResponse extends IsoMessageSubmissionResponseBase {}

export interface IsoPacs009SubmissionResponse extends IsoMessageSubmissionResponseBase {}

export interface IsoMessageStatusResponse extends IsoMessageSubmissionResponseBase {
  detail: string | null;
  updated_at_ms: number;
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
  signal?: AbortSignal;
  retryProfile?: string;
  wait?: IsoMessageWaitOptions;
}

export interface ContractAccessSetHintsInput {
  readKeys?: ReadonlyArray<string>;
  writeKeys?: ReadonlyArray<string>;
}

export interface ContractEntrypointInput {
  name: string;
  kind?: string;
  permission?: string | null;
}

export interface ContractManifestInput {
  codeHash?: HashLike | null;
  abiHash?: HashLike | null;
  compilerFingerprint?: string | null;
  featuresBitmap?: NumericLike | null;
  accessSetHints?: ContractAccessSetHintsInput | null;
  entrypoints?: ReadonlyArray<ContractEntrypointInput> | null;
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
  codeB64: string | ArrayBufferView | ArrayBuffer | Buffer;
  manifest?: ToriiContractManifestInput | null;
}

export interface DeployContractResponse {
  ok: boolean;
  code_hash_hex: string;
  abi_hash_hex: string;
}

export interface DeployContractInstanceRequest extends DeployContractRequest {
  namespace: string;
  contractId: string;
}

export interface DeployContractInstanceResponse extends DeployContractResponse {
  namespace: string;
  contract_id: string;
}

export interface ActivateContractInstanceRequest {
  authority: string;
  privateKey: string;
  namespace: string;
  contractId: string;
  codeHash: string;
}

export interface ActivateContractInstanceResponse {
  ok: boolean;
}

export interface ContractCallRequest {
  authority: string;
  privateKey: string;
  namespace: string;
  contractId: string;
  entrypoint?: string | null;
  payload?: unknown;
  gasAssetId?: string | null;
  gasLimit: NumericLike;
}

export interface ContractCallResponse {
  ok: boolean;
  namespace: string;
  contract_id: string;
  code_hash_hex: string;
  abi_hash_hex: string;
  tx_hash_hex: string;
  entrypoint: string | null;
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
  };
  code_bytes: string | null;
}

export interface ContractCodeBytesRecord {
  code_b64: string;
}

export interface ContractInstanceRecord {
  contract_id: string;
  code_hash_hex: string;
}

export interface ContractInstanceListResponse {
  namespace: string;
  total: number;
  offset: number;
  limit: number;
  instances: ReadonlyArray<ContractInstanceRecord>;
}

export interface ContractInstanceListOptions {
  contains?: string | null;
  hashPrefix?: string | null;
  offset?: number | string | bigint;
  limit?: number | string | bigint;
  order?: "cid_asc" | "cid_desc" | "hash_asc" | "hash_desc" | null;
  signal?: AbortSignal;
}

export interface ContractInstanceIteratorOptions extends ContractInstanceListOptions {
  pageSize?: NumericLike;
  maxItems?: NumericLike;
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
  minReplicas: NumericLike;
  storageClass:
    | "Hot"
    | "Warm"
    | "Cold"
    | string
    | {
        type: string;
      };
  retentionEpoch?: NumericLike;
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
  proofB64?: BinaryLike | string;
}

export interface SorafsPinRegisterRequest {
  authority: string;
  privateKey: string;
  chunker: SorafsPinRegisterChunkerInput;
  pinPolicy: SorafsPinRegisterPinPolicyInput;
  manifestDigestHex?: string;
  manifest_digest_hex?: string;
  chunkDigestSha3_256Hex?: string;
  chunk_digest_sha3_256_hex?: string;
  chunkDigest?: string;
  chunk_digest?: string;
  submittedEpoch?: NumericLike;
  submitted_epoch?: NumericLike;
  alias?: SorafsPinRegisterAliasInput | null;
  aliasNamespace?: string;
  aliasName?: string;
  aliasProof?: BinaryLike | string;
  aliasProofB64?: BinaryLike | string;
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
  addressFormat?: ToriiAddressFormat;
  signal?: AbortSignal;
}

export interface UaidManifestQueryOptions {
  dataspaceId?: number;
  addressFormat?: ToriiAddressFormat;
  signal?: AbortSignal;
}

export interface RegisterSmartContractCodeInstructionInput {
  manifest: ContractManifestInput;
}

export interface RegisterSmartContractBytesInstructionInput {
  codeHash: HashLike;
  code: ArrayBufferView | ArrayBuffer | Buffer | string;
}

export interface DeactivateContractInstanceInstructionInput {
  namespace: string;
  contractId: string;
  reason?: string | null;
}

export interface ActivateContractInstanceInstructionInput {
  namespace: string;
  contractId: string;
  codeHash: HashLike;
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

export interface DeactivateContractInstanceTransactionInput {
  chainId: string;
  authority: string;
  namespace: string;
  contractId: string;
  reason?: string | null;
  metadata?: MetadataLike;
  creationTimeMs?: number | null;
  ttlMs?: number | null;
  nonce?: number | null;
  privateKey: Buffer | ArrayBuffer | ArrayBufferView;
}

export interface ActivateContractInstanceTransactionInput {
  chainId: string;
  authority: string;
  namespace: string;
  contractId: string;
  codeHash: HashLike;
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

export interface SubmitTransactionAndWaitOptions extends TransactionStatusPollOptions {
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
  });
  readonly status: number;
  readonly statusText: string | null;
  readonly expected: ReadonlyArray<number>;
  readonly code: string | null;
  readonly rejectCode: string | null;
  readonly errorMessage: string | null;
  readonly bodyText: string | null;
  readonly bodyJson: unknown;
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

export declare function extractPipelineStatusKind(payload: unknown): string | null;
export declare function extractPipelineRejectionReason(payload: unknown): string | null;
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
  queryAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: IterableQueryOptions,
  ): Promise<ToriiIterableListResponse<T>>;
  iterateAccountTransactions<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: AccountTransactionIteratorOptions,
  ): AsyncGenerator<T, void, unknown>;
  iterateAccountTransactionsQuery<T = ToriiAccountTransactionItem>(
    accountId: string,
    options?: PaginationIteratorOptions,
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
  listAttachments(
    options?: { signal?: AbortSignal },
  ): Promise<ReadonlyArray<ToriiAttachmentMetadata>>;
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
  evaluateAliasVoprf(blindedElementHex: string): Promise<AliasVoprfEvaluateResponse>;
  resolveAlias(alias: string): Promise<AliasResolutionDto | null>;
  resolveAliasByIndex(index: number | string | bigint): Promise<AliasResolutionDto | null>;
  listSorafsPinManifests(options?: SorafsPinListOptions): Promise<SorafsPinListResponse>;
  iterateSorafsPinManifests(
    options?: SorafsPinListOptions & PaginationIteratorOptions,
  ): AsyncGenerator<SorafsManifestRecord, void, unknown>;
  listSorafsAliases(options?: SorafsAliasListOptions): Promise<SorafsAliasListResponse>;
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
  getSorafsStorageState(
    options?: { signal?: AbortSignal },
  ): Promise<SorafsStorageStateResponse>;
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
    options?: { outputDir?: string; signal?: AbortSignal; label?: string; blockHashHex?: string },
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
  submitTransaction(payload: ArrayBufferView | ArrayBuffer | Buffer): Promise<unknown>;
  getTransactionStatus(
    hashHex: string,
    options?: { allowShortHash?: boolean; signal?: AbortSignal },
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
    options?: { allowShortHash?: boolean; signal?: AbortSignal },
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
  getHealth(options?: { signal?: AbortSignal }): Promise<ToriiHealthStatus | null>;
  getConfiguration(): Promise<unknown | null>;
  getConfigurationTyped(): Promise<ToriiConfigurationSnapshot | null>;
  getConfidentialGasSchedule(): Promise<ConfidentialGasSchedule | null>;
  getStatusSnapshot(options?: { signal?: AbortSignal }): Promise<ToriiStatusSnapshot>;
  getNetworkTimeNow(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiNetworkTimeNow>;
  getNetworkTimeStatus(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiNetworkTimeStatus>;
  getNodeCapabilities(options?: { signal?: AbortSignal }): Promise<ToriiNodeCapabilities>;
  getRuntimeAbiActive(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiRuntimeAbiActiveResponse>;
  getRuntimeAbiHash(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiRuntimeAbiHashResponse>;
  getRuntimeMetrics(options?: { signal?: AbortSignal }): Promise<ToriiRuntimeMetrics>;
  listRuntimeUpgrades(
    options?: { signal?: AbortSignal },
  ): Promise<ReadonlyArray<ToriiRuntimeUpgradeListItem>>;
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
  listPeers(options?: { signal?: AbortSignal }): Promise<Array<Record<string, unknown>>>;
  listPeersTyped(options?: { signal?: AbortSignal }): Promise<Array<ToriiPeerRecord>>;
  listTelemetryPeersInfo(options?: {
    signal?: AbortSignal;
  }): Promise<ReadonlyArray<ToriiTelemetryPeerInfo>>;
  getExplorerMetrics(options?: {
    signal?: AbortSignal;
  }): Promise<ToriiExplorerMetricsSnapshot | null>;
  getExplorerAccountQr(
    accountId: string,
    options?: {
      addressFormat?: ToriiAddressFormat;
      signal?: AbortSignal;
    },
  ): Promise<ToriiExplorerAccountQrSnapshot>;
  getSnsPolicy(suffixId: number, options?: { signal?: AbortSignal }): Promise<SnsSuffixPolicy>;
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
  createSnsGovernanceCase(
    payload: Record<string, unknown>,
    options?: { signal?: AbortSignal },
  ): Promise<SnsGovernanceCase>;
  exportSnsGovernanceCases(
    options?: SnsCaseExportOptions,
  ): Promise<SnsGovernanceCaseExportResult>;
  iterateSnsGovernanceCases(
    options?: SnsCaseExportOptions,
  ): AsyncGenerator<SnsGovernanceCase, void, unknown>;
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
  getGovernanceUnlockStats(
    options?: { signal?: AbortSignal },
  ): Promise<Record<string, unknown> | null>;
  getGovernanceUnlockStatsTyped(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceUnlockStats>;
  getGovernanceCouncilCurrent(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiGovernanceCouncilCurrentResponse>;
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
  getProtectedNamespaces(
    options?: { signal?: AbortSignal },
  ): Promise<ToriiProtectedNamespacesGetResponse>;
  getSumeragiStatus(options?: { signal?: AbortSignal }): Promise<Record<string, unknown>>;
  getSumeragiStatusTyped(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiStatus>;
  getSumeragiPacemaker(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiPacemakerResponse | null>;
  getSumeragiQc(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiQcSnapshot>;
  getSumeragiCommitQc(
    blockHashHex: string,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiSumeragiCommitQcRecord>;
  getSumeragiPhases(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiPhasesSnapshot>;
  getSumeragiBlsKeys(options?: { signal?: AbortSignal }): Promise<Record<string, string | null>>;
  getSumeragiLeader(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiLeaderSnapshot>;
  getSumeragiCollectors(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiCollectorsPlan>;
  getSumeragiParams(options?: { signal?: AbortSignal }): Promise<ToriiSumeragiParamsSnapshot>;
  getSumeragiTelemetry(options?: { signal?: AbortSignal }): Promise<Record<string, unknown>>;
  getSumeragiTelemetryTyped(options?: {
    signal?: AbortSignal;
  }): Promise<SumeragiTelemetrySnapshot>;
  getSumeragiRbc(options?: { signal?: AbortSignal }): Promise<SumeragiRbcSnapshot | null>;
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
  sampleRbcChunks(options: RbcSampleRequestOptions): Promise<RbcSampleResponse | null>;
  listSumeragiEvidence(options?: SumeragiEvidenceListOptions): Promise<SumeragiEvidenceListResponse>;
  getSumeragiEvidenceCount(): Promise<SumeragiEvidenceCountResponse>;
  submitSumeragiEvidence(
    request: SumeragiEvidenceSubmitRequest,
  ): Promise<SumeragiEvidenceSubmitResponse>;
  getMetrics(options: { asText: true; signal?: AbortSignal }): Promise<string>;
  getMetrics(options?: { asText?: boolean; signal?: AbortSignal }): Promise<unknown>;
  getBlock(
    height: number | string | bigint,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiExplorerBlock | null>;
  listBlocks(options?: BlockListOptions): Promise<ToriiExplorerBlocksPage>;
  streamEvents<T = ToriiEventPayload>(
    options?: EventStreamOptions,
  ): AsyncGenerator<ToriiSseEvent<T>, void, unknown>;
  streamSumeragiStatus<T = ToriiSumeragiStatus>(
    options?: Omit<EventStreamOptions, "filter">,
  ): AsyncGenerator<ToriiSseEvent<T>, void, unknown>;
  listKaigiRelays(
    options?: { signal?: AbortSignal },
  ): Promise<KaigiRelaySummaryList>;
  getKaigiRelay(
    relayId: string,
    options?: { signal?: AbortSignal },
  ): Promise<KaigiRelayDetail | null>;
  getKaigiRelaysHealth(
    options?: { signal?: AbortSignal },
  ): Promise<KaigiRelayHealthSnapshot>;
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
    options?: { contentType?: string; signal?: AbortSignal; retryProfile?: string },
  ): Promise<IsoPacs008SubmissionResponse | null>;
  submitIsoPacs009(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: { contentType?: string; signal?: AbortSignal; retryProfile?: string },
  ): Promise<IsoPacs009SubmissionResponse | null>;
  submitIsoPacs008AndWait(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: {
      contentType?: string;
      signal?: AbortSignal;
      retryProfile?: string;
      wait?: IsoMessageWaitOptions;
    },
  ): Promise<IsoMessageStatusResponse>;
  submitIsoPacs009AndWait(
    message: ArrayBufferView | ArrayBuffer | Buffer | string,
    options?: {
      contentType?: string;
      signal?: AbortSignal;
      retryProfile?: string;
      wait?: IsoMessageWaitOptions;
    },
  ): Promise<IsoMessageStatusResponse>;
  submitIsoMessage(
    message: BuildPacs008Options | BuildPacs009Options,
    options?: SubmitIsoMessageOptions,
  ): Promise<IsoMessageSubmissionResponseBase | IsoMessageStatusResponse | null>;
  getIsoMessageStatus(
    messageId: string,
    options?: { signal?: AbortSignal; retryProfile?: string },
  ): Promise<IsoMessageStatusResponse | null>;
  waitForIsoMessageStatus(
    messageId: string,
    options?: IsoMessageWaitOptions,
  ): Promise<IsoMessageStatusResponse>;
  getConnectStatus(): Promise<ConnectStatusSnapshot | null>;
  createConnectSession(input: { sid: string; node?: string | null }): Promise<ConnectSessionResponse>;
  deleteConnectSession(sid: string): Promise<boolean>;
  listConnectApps(options?: ConnectAppListOptions): Promise<ConnectAppRegistryPage>;
  iterateConnectApps(
    options?: ConnectAppIteratorOptions,
  ): AsyncGenerator<ConnectAppRecord, void, unknown>;
  getConnectApp(appId: string, options?: { signal?: AbortSignal }): Promise<ConnectAppRecord>;
  registerConnectApp(
    record: ConnectAppUpsertInput,
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAppRecord | null>;
  deleteConnectApp(appId: string): Promise<boolean>;
  getConnectAppPolicy(options?: { signal?: AbortSignal }): Promise<ConnectAppPolicyControls>;
  updateConnectAppPolicy(
    updates: ConnectAppPolicyUpdate,
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAppPolicyControls>;
  getConnectAdmissionManifest(
    options?: { signal?: AbortSignal },
  ): Promise<ConnectAdmissionManifest>;
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
  registerContractCode(request: RegisterContractCodeRequest): Promise<unknown | null>;
  deployContract(request: DeployContractRequest): Promise<DeployContractResponse | null>;
  deployContractInstance(
    request: DeployContractInstanceRequest,
  ): Promise<DeployContractInstanceResponse | null>;
  activateContractInstance(
    request: ActivateContractInstanceRequest,
  ): Promise<ActivateContractInstanceResponse | null>;
  callContract(
    request: ContractCallRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ContractCallResponse>;
  getContractManifest(codeHashHex: string): Promise<ContractManifestRecord | null>;
  getContractCodeBytes(codeHashHex: string): Promise<ContractCodeBytesRecord | null>;
  listContractInstances(
    namespace: string,
    options?: ContractInstanceListOptions,
  ): Promise<ContractInstanceListResponse>;
  iterateContractInstances(
    namespace: string,
    options?: ContractInstanceIteratorOptions,
  ): AsyncGenerator<ContractInstanceRecord, void, unknown>;
  listGovernanceInstances(
    namespace: string,
    options?: ContractInstanceListOptions,
  ): Promise<ContractInstanceListResponse>;
  iterateGovernanceInstances(
    namespace: string,
    options?: ContractInstanceIteratorOptions,
  ): AsyncGenerator<ContractInstanceRecord, void, unknown>;
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
  listSubscriptions(options?: SubscriptionListOptions): Promise<SubscriptionListResponse>;
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
  listOfflineAllowances(
    options?: OfflineAllowanceListOptions,
  ): Promise<ToriiOfflineAllowanceListResponse>;
  queryOfflineAllowances(
    options?: IterableQueryOptions,
  ): Promise<ToriiOfflineAllowanceListResponse>;
  iterateOfflineAllowances(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineAllowanceItem, void, unknown>;
  iterateOfflineAllowancesQuery(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineAllowanceItem, void, unknown>;
  listOfflineSummaries(
    options?: IterableListOptions,
  ): Promise<ToriiOfflineSummaryListResponse>;
  queryOfflineSummaries(
    options?: IterableQueryOptions,
  ): Promise<ToriiOfflineSummaryListResponse>;
  iterateOfflineSummaries(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineSummaryItem, void, unknown>;
  iterateOfflineSummariesQuery(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineSummaryItem, void, unknown>;
  listOfflineTransfers(
    options?: OfflineTransferListOptions,
  ): Promise<ToriiOfflineTransferListResponse>;
  queryOfflineTransfers(
    options?: IterableQueryOptions,
  ): Promise<ToriiOfflineTransferListResponse>;
  iterateOfflineTransfers(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineTransferItem, void, unknown>;
  iterateOfflineTransfersQuery(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineTransferItem, void, unknown>;
  listOfflineRevocations(
    options?: IterableListOptions,
  ): Promise<ToriiOfflineRevocationListResponse>;
  queryOfflineRevocations(
    options?: IterableQueryOptions,
  ): Promise<ToriiOfflineRevocationListResponse>;
  iterateOfflineRevocations(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineRevocationItem, void, unknown>;
  iterateOfflineRevocationsQuery(
    options?: PaginationIteratorOptions,
  ): AsyncGenerator<ToriiOfflineRevocationItem, void, unknown>;
  issueOfflineCertificate(
    certificate: ToriiOfflineWalletCertificateDraft,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineCertificateIssueResponse>;
  issueOfflineCertificateRenewal(
    certificateIdHex: string,
    certificate: ToriiOfflineWalletCertificateDraft,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineCertificateIssueResponse>;
  submitOfflineSettlement(
    request: ToriiOfflineSettlementSubmitRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineSettlementSubmitResponse>;
  submitOfflineSettlementAndWait(
    request: ToriiOfflineSettlementSubmitRequest,
    options?: SubmitOfflineSettlementAndWaitOptions,
  ): Promise<ToriiOfflineSettlementSubmitResponse>;
  issueOfflineBuildClaim(
    request: ToriiOfflineBuildClaimIssueRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineBuildClaimIssueResponse>;
  registerOfflineAllowance(
    request: ToriiOfflineAllowanceRegisterRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineAllowanceRegisterResponse>;
  renewOfflineAllowance(
    certificateIdHex: string,
    request: ToriiOfflineAllowanceRegisterRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineAllowanceRegisterResponse>;
  topUpOfflineAllowance(
    request: ToriiOfflineTopUpRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineTopUpResponse>;
  topUpOfflineAllowanceRenewal(
    certificateIdHex: string,
    request: ToriiOfflineTopUpRequest,
    options?: { signal?: AbortSignal },
  ): Promise<ToriiOfflineTopUpResponse>;
  getOfflineRejectionStats(options?: {
    telemetryProfile?: string;
    signal?: AbortSignal;
  }): Promise<ToriiOfflineRejectionStatsResponse | null>;
}

export interface NoritoRpcClientOptions {
  fetchImpl?: typeof fetch;
  timeoutMs?: number;
  defaultHeaders?: Record<string, string>;
  allowInsecure?: boolean;
  authToken?: string | null;
  apiToken?: string | null;
  insecureTransportTelemetryHook?: (event: InsecureTransportTelemetryEvent) => void;
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

export function generateKeyPair(options?: {
  seed?: ArrayBufferView | ArrayBuffer | Buffer;
}): KeyPair;

export function publicKeyFromPrivate(privateKey: ArrayBufferView | ArrayBuffer | Buffer): Buffer;

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

export function signEd25519(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  privateKey: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export function verifyEd25519(
  message: ArrayBufferView | ArrayBuffer | Buffer | string,
  signature: ArrayBufferView | ArrayBuffer | Buffer,
  publicKey: ArrayBufferView | ArrayBuffer | Buffer,
): boolean;

export function canonicalQueryString(query?: string | URLSearchParams | null): string;

export function canonicalRequestMessage(params: {
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: Buffer | ArrayBuffer | ArrayBufferView | string;
}): Buffer;

export function buildCanonicalRequestHeaders(params: {
  accountId: string;
  method: string;
  path: string;
  query?: string | URLSearchParams;
  body?: Buffer | ArrayBuffer | ArrayBufferView | string;
  privateKey: ArrayBufferView | ArrayBuffer | Buffer;
}): { "X-Iroha-Account": string; "X-Iroha-Signature": string };

export function deriveConfidentialKeyset(
  spendKey: ArrayBufferView | ArrayBuffer | Buffer,
): ConfidentialKeyset;

export function deriveConfidentialKeysetFromHex(spendKeyHex: string): ConfidentialKeyset;

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
  descriptorBytes: Buffer | null;
  bindingHex: string | null;
  binding: Buffer | null;
  touchManifest: ReadonlyArray<AxtTouchFragment>;
  native: boolean;
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

export function encodeSignedTransactionNorito(
  signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
): Buffer;

export interface OfflineEnvelope {
  version: number;
  signedTransaction: Buffer;
  hashHex: string;
  schemaName: string;
  keyAlias: string;
  issuedAtMs: number;
  metadata: Record<string, string>;
  publicKey: Buffer | null;
  exportedKeyBundle: Buffer | null;
}

export interface SerializedOfflineEnvelope {
  version: number;
  schema_name: string;
  key_alias: string;
  issued_at_ms: number;
  hash_hex: string;
  metadata: Record<string, string>;
  signed_transaction_b64: string;
  public_key_b64: string | null;
  exported_key_bundle_b64: string | null;
}

export function buildOfflineEnvelope(input: {
  signedTransaction: ArrayBufferView | ArrayBuffer | Buffer;
  hashHex?: string;
  schemaName?: string;
  keyAlias: string;
  issuedAtMs?: number;
  metadata?: Record<string, string>;
  publicKey?: ArrayBufferView | ArrayBuffer | Buffer | null;
  exportedKeyBundle?: ArrayBufferView | ArrayBuffer | Buffer | null;
}): OfflineEnvelope;

export function serializeOfflineEnvelope(envelope: OfflineEnvelope): SerializedOfflineEnvelope;

export function parseOfflineEnvelope(
  payload: SerializedOfflineEnvelope | string | Record<string, unknown>,
): OfflineEnvelope;

export function readOfflineEnvelopeFile(path: string): Promise<OfflineEnvelope>;

export function writeOfflineEnvelopeFile(
  path: string,
  envelope: OfflineEnvelope,
): Promise<void>;

export function replayOfflineEnvelope(
  toriiClient: ToriiClient,
  envelope: OfflineEnvelope,
  options?: {
    waitForStatus?: boolean;
    intervalMs?: number;
    timeoutMs?: number | null;
    maxAttempts?: number | null;
  },
): Promise<any>;

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
  offlineToOnlineTransfer: number;
  offlineSpendReceipt: number;
  offlineEnvelope: number;
}>;

export class OfflineQrStreamOptions {
  chunkSize: number;
  parityGroup: number;
  payloadKind: number;
  constructor(options?: { chunkSize?: number; parityGroup?: number; payloadKind?: number });
}

export class OfflineQrStreamEnvelope {
  readonly version: number;
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
  static decode(bytes: ArrayBufferView | ArrayBuffer | Buffer): OfflineQrStreamEnvelope;
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
  static decode(bytes: ArrayBufferView | ArrayBuffer | Buffer): OfflineQrStreamFrame;
}

export class OfflineQrStreamEncoder {
  static encodeFrames(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options?: { chunkSize?: number; parityGroup?: number; payloadKind?: number },
  ): OfflineQrStreamFrame[];
  static encodeFrameBytes(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options?: { chunkSize?: number; parityGroup?: number; payloadKind?: number },
  ): Buffer[];
}

export class OfflineQrStreamDecoder {
  ingest(
    frameBytes: ArrayBufferView | ArrayBuffer | Buffer,
  ): {
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
  frames: AsyncIterable<string | ArrayBufferView | ArrayBuffer | Buffer> | Iterable<string | ArrayBufferView | ArrayBuffer | Buffer>,
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

export const PETAL_STREAM_GRID_SIZES: ReadonlyArray<number>;

export class OfflinePetalStreamOptions {
  gridSize: number;
  border: number;
  anchorSize: number;
  constructor(options?: { gridSize?: number; border?: number; anchorSize?: number });
}

export class OfflinePetalStreamGrid {
  readonly gridSize: number;
  readonly cells: boolean[];
  constructor(input: { gridSize: number; cells: boolean[] });
  get(x: number, y: number): boolean | null;
}

export class OfflinePetalStreamSampleGrid {
  readonly gridSize: number;
  readonly samples: number[];
  constructor(input: { gridSize: number; samples: number[] });
}

export class OfflinePetalStreamEncoder {
  static encodeGrid(
    payload: ArrayBufferView | ArrayBuffer | Buffer,
    options?: { gridSize?: number; border?: number; anchorSize?: number },
  ): OfflinePetalStreamGrid;
  static encodeGrids(
    payloads: ReadonlyArray<ArrayBufferView | ArrayBuffer | Buffer>,
    options?: { gridSize?: number; border?: number; anchorSize?: number },
  ): { gridSize: number; grids: OfflinePetalStreamGrid[] };
}

export class OfflinePetalStreamDecoder {
  static decodeGrid(
    grid: OfflinePetalStreamGrid,
    options?: { gridSize?: number; border?: number; anchorSize?: number },
  ): Buffer;
  static decodeSamples(
    sampleGrid: OfflinePetalStreamSampleGrid,
    options?: { gridSize?: number; border?: number; anchorSize?: number },
  ): Buffer;
}

export function samplePetalStreamGridFromRgba(
  image: {
    data: ArrayBufferView | ArrayBuffer | Buffer;
    width: number;
    height: number;
  },
  gridSize: number,
): OfflinePetalStreamSampleGrid;

export function decodePetalStreamFrameAuto(
  image: {
    data: ArrayBufferView | ArrayBuffer | Buffer;
    width: number;
    height: number;
  },
  options?: { gridSize?: number; border?: number; anchorSize?: number },
): { gridSize: number; payload: Buffer };

export class OfflinePetalStreamScanSession {
  readonly qrSession: OfflineQrStreamScanSession;
  gridSize: number | null;
  constructor(options?: {
    qrSession?: OfflineQrStreamScanSession;
    petalOptions?: { gridSize?: number; border?: number; anchorSize?: number };
  });
  ingestSampleGrid(sampleGrid: OfflinePetalStreamSampleGrid): {
    payload: Buffer | null;
    receivedChunks: number;
    totalChunks: number;
    recoveredChunks: number;
    progress: number;
    isComplete: boolean;
  };
  ingestRgba(
    image: {
      data: ArrayBufferView | ArrayBuffer | Buffer;
      width: number;
      height: number;
    },
    gridSize?: number | null,
  ): {
    payload: Buffer | null;
    receivedChunks: number;
    totalChunks: number;
    recoveredChunks: number;
    progress: number;
    isComplete: boolean;
  };
  ingestRgbaAuto(image: {
    data: ArrayBufferView | ArrayBuffer | Buffer;
    width: number;
    height: number;
  }): {
    payload: Buffer | null;
    receivedChunks: number;
    totalChunks: number;
    recoveredChunks: number;
    progress: number;
    isComplete: boolean;
  };
}

export function buildRegisterDomainTransaction(
  input: RegisterDomainInput,
): SignedTransactionResult;

/**
 * Assemble and sign a transaction from pre-built instruction payloads. The
 * instruction array must be non-empty; each entry should be either a builder
 * result or a Norito JSON string.
 */
export function buildTransaction(input: TransactionAssemblyInput): SignedTransactionResult;

/**
 * Build and sign a transaction containing a single `Mint::Asset` instruction.
 * Validates the quantity and asset identifier before serialising to Norito.
 */
export function buildMintAssetTransaction(input: MintAssetInput): SignedTransactionResult;
/**
 * Build and sign a transaction containing a single `Burn::Asset` instruction.
 * Throws if the quantity is non-positive or the asset identifier is empty.
 */
export function buildBurnAssetTransaction(input: BurnAssetInput): SignedTransactionResult;
/**
 * Build and sign a transaction containing a single `Burn::TriggerRepetitions`
 * instruction. Throws when repetitions are not positive integers.
 */
export function buildBurnTriggerTransaction(
  input: BurnTriggerInput,
): SignedTransactionResult;
export function buildMintTriggerTransaction(input: MintTriggerInput): SignedTransactionResult;
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
/**
 * Compose a mint followed by one or more transfers. Provide either `transfer`
 * or `transfers`; transfers without an explicit `sourceAssetId` reuse the mint's
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
 * `accountId` and `assetId` are provided the helper validates that they match
 * the canonical `<assetDefinitionId>#<accountId>` format.
 */
export function buildRegisterAssetDefinitionAndMintTransaction(
  input: RegisterAssetDefinitionAndMintInput,
): SignedTransactionResult;
/**
 * Register an asset definition, mint supply, and optionally fan-out transfers.
 * When a transfer omits `sourceAssetId` the helper reuses the first minted
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

export function buildTimeTriggerAction(options: TimeTriggerActionOptions): string;
export function buildPrecommitTriggerAction(
  options: CommitTriggerActionOptions,
): string;

export function buildCreateKaigiTransaction(
  input: CreateKaigiTransactionInput,
): SignedTransactionResult;
export function buildJoinKaigiTransaction(
  input: JoinKaigiTransactionInput,
): SignedTransactionResult;
export function buildLeaveKaigiTransaction(
  input: LeaveKaigiTransactionInput,
): SignedTransactionResult;
export function buildEndKaigiTransaction(
  input: EndKaigiTransactionInput,
): SignedTransactionResult;
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
export function buildDeactivateContractInstanceTransaction(
  input: DeactivateContractInstanceTransactionInput,
): SignedTransactionResult;
export function buildActivateContractInstanceTransaction(
  input: ActivateContractInstanceTransactionInput,
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

export function buildTransferAssetInstruction({
  sourceAssetId,
  quantity,
  destinationAccountId,
}: {
  sourceAssetId: string;
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

export function buildRegisterZkAssetInstruction(
  input: RegisterZkAssetInstructionInput,
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

export function buildUnshieldInstruction(input: UnshieldInstructionInput): object;

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

export function buildDeactivateContractInstanceInstruction(
  input: DeactivateContractInstanceInstructionInput,
): object;

export function buildActivateContractInstanceInstruction(
  input: ActivateContractInstanceInstructionInput,
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
  insecureTransportTelemetryHook: ((event: InsecureTransportTelemetryEvent) => void) | null;
};

export const DEFAULT_RETRY_PROFILE_PIPELINE: ToriiRetryProfileOptions;

export const DEFAULT_RETRY_PROFILE_STREAMING: ToriiRetryProfileOptions;

export function resolveToriiClientConfig(input?: {
  config?: ToriiClientConfigSource;
  env?: Record<string, string | undefined>;
  overrides?: ToriiClientConfigSource;
}): ResolvedToriiClientConfig;

export function extractToriiFeatureConfig(input?: {
  config?: Record<string, unknown>;
} & Record<string, unknown>): ToriiFeatureConfigSnapshot;
