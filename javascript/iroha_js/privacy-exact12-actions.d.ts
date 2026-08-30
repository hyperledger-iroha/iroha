import type {
  PrivacyOperationSchemaV1,
  PrivacyProtocolIdV1,
} from "./privacy-capabilities.js";

/** Closed public action spelling for the thirteen Exact12 operations. */
export type PrivacyExact12ActionOperationV1 = PrivacyOperationSchemaV1;

/** Closed ledger-effect class committed by a public Exact12 operation. */
export type PrivacyLedgerEffectKindV1 =
  | "verification_only"
  | "zk_ace_transparent_transfer"
  | "anonymous_pgc_account_state_transition"
  | "zk_ams_batch_admission"
  | "zk_ams_provision_account"
  | "zk_x509_certificate_nullifier"
  | "orchard_note_state_transition"
  | "fcmp_membership_payment"
  | "ivm_private_note_state_transition"
  | "pq_masp_note_state_transition";

export type PrivacyActionLocalStateV1 = "submitted" | "terminal";
export type PrivacyActionTerminalChainStateV1 =
  | "Committed"
  | "Applied"
  | "Rejected"
  | "Expired";

export const PRIVACY_EXACT12_SIGNED_TRANSACTION_MAX_BYTES_V1: 10485760;
export const PRIVACY_EXACT12_ACTION_OPERATIONS_V1:
  readonly PrivacyExact12ActionOperationV1[];
export const PRIVACY_LEDGER_EFFECT_KINDS_V1:
  readonly PrivacyLedgerEffectKindV1[];

/** Fail-closed validation error for an Exact12 public action model. */
export class PrivacyExact12ActionModelErrorV1 extends TypeError {
  readonly path: string;
}

export function privacyExact12ProtocolIdV1(
  operation: PrivacyExact12ActionOperationV1,
): PrivacyProtocolIdV1;
export function privacyExact12LedgerEffectKindV1(
  operation: PrivacyExact12ActionOperationV1,
): PrivacyLedgerEffectKindV1;

export const PrivacyExact12ActionContractV1: Readonly<{
  protocolId: typeof privacyExact12ProtocolIdV1;
  ledgerEffectKind: typeof privacyExact12LedgerEffectKindV1;
}>;

export interface PrivacyExact12ActionRequestV1Options {
  readonly operation: PrivacyExact12ActionOperationV1;
  readonly signedTransactionVersioned: Uint8Array;
  /**
   * Optional pre-submit check against the fresh finalized capability snapshot.
   * This is not a signed consensus condition and does not pin the execution-time manifest.
   */
  readonly expectedManifestDigest?: Uint8Array | null;
}

/** One closed operation and a bounded snapshot of its signed transaction wire. */
export class PrivacyExact12ActionRequestV1 {
  constructor(options: PrivacyExact12ActionRequestV1Options);
  constructor(
    operation: PrivacyExact12ActionOperationV1,
    signedTransactionVersioned: Uint8Array,
    expectedManifestDigest?: Uint8Array | null,
  );
  readonly operation: PrivacyExact12ActionOperationV1;
  readonly signedTransactionVersioned: Uint8Array;
  /** Pre-submit observation only; execution evidence comes from the finalized receipt. */
  readonly expectedManifestDigest: Uint8Array | null;
}

export interface PrivacyActionOperationViewBaseV1 {
  readonly protocolId: PrivacyProtocolIdV1;
  readonly operationSchema: PrivacyExact12ActionOperationV1;
  readonly transactionHash: Uint8Array;
  readonly transactionIntentDigest: Uint8Array;
  readonly statementDigest: Uint8Array;
  readonly proofEnvelopeHash: Uint8Array;
  readonly ledgerEffectKind: PrivacyLedgerEffectKindV1;
  /** Fresh finalized capability snapshot used for pre-submit admission. */
  readonly capabilityManifestDigest: Uint8Array;
  /** Height of the fresh finalized capability snapshot used before submission. */
  readonly capabilityCommittedHeight: bigint | number;
}

export type PrivacyActionNoExecutionEvidenceV1 = Readonly<{
  executionCapabilityManifestDigest?: null;
  executionCapabilityCommittedHeight?: null;
  executionReceiptFinalizedHeight?: null;
  executionReceiptFinalizedBlockHash?: null;
}>;

export type PrivacyActionAppliedExecutionEvidenceV1 = Readonly<{
  /** Capability manifest actually admitted by native execution. */
  executionCapabilityManifestDigest: Uint8Array;
  executionCapabilityCommittedHeight: bigint | number;
  /** Finalized block binding of the consensus execution receipt. */
  executionReceiptFinalizedHeight: bigint | number;
  executionReceiptFinalizedBlockHash: Uint8Array;
}>;

export type PrivacyActionOperationViewV1Options =
  | (PrivacyActionOperationViewBaseV1 & PrivacyActionNoExecutionEvidenceV1 & Readonly<{
      localState: "submitted";
      terminalChainState?: null;
      committedHeight?: null;
      rejectionReason?: null;
    }>)
  | (PrivacyActionOperationViewBaseV1 & PrivacyActionNoExecutionEvidenceV1 & Readonly<{
      localState: "terminal";
      terminalChainState: "Committed";
      committedHeight: bigint | number;
      rejectionReason?: null;
    }>)
  | (PrivacyActionOperationViewBaseV1 & PrivacyActionAppliedExecutionEvidenceV1 & Readonly<{
      localState: "terminal";
      terminalChainState: "Applied";
      committedHeight: bigint | number;
      rejectionReason?: null;
    }>)
  | (PrivacyActionOperationViewBaseV1 & PrivacyActionNoExecutionEvidenceV1 & Readonly<{
      localState: "terminal";
      terminalChainState: "Rejected";
      committedHeight: bigint | number;
      rejectionReason: string;
    }>)
  | (PrivacyActionOperationViewBaseV1 & PrivacyActionNoExecutionEvidenceV1 & Readonly<{
      localState: "terminal";
      terminalChainState: "Expired";
      committedHeight?: null;
      rejectionReason?: null;
    }>);

/** Validated immutable state; successful terminal views require a finalized native receipt. */
export class PrivacyActionOperationViewV1 {
  constructor(options: PrivacyActionOperationViewV1Options);
  readonly protocolId: PrivacyProtocolIdV1;
  readonly operationSchema: PrivacyExact12ActionOperationV1;
  readonly transactionHash: Uint8Array;
  readonly transactionIntentDigest: Uint8Array;
  readonly statementDigest: Uint8Array;
  readonly proofEnvelopeHash: Uint8Array;
  readonly localState: PrivacyActionLocalStateV1;
  readonly terminalChainState: PrivacyActionTerminalChainStateV1 | null;
  readonly committedHeight: bigint | null;
  readonly rejectionReason: string | null;
  readonly ledgerEffectKind: PrivacyLedgerEffectKindV1;
  /** Fresh finalized capability snapshot used for pre-submit admission. */
  readonly capabilityManifestDigest: Uint8Array;
  readonly capabilityCommittedHeight: bigint;
  /** Native execution capability evidence; present only for Applied. */
  readonly executionCapabilityManifestDigest: Uint8Array | null;
  readonly executionCapabilityCommittedHeight: bigint | null;
  /** Finalized consensus receipt binding; present only for Applied. */
  readonly executionReceiptFinalizedHeight: bigint | null;
  readonly executionReceiptFinalizedBlockHash: Uint8Array | null;
}
