/** Node confidential-payment sessions owned by the installed native runtime. */
export interface PrivateSettlementHostLimits {
  readonly maximumSessions: number;
  /** Cumulative original descriptor, configuration, pins, genesis and request bytes. */
  readonly maximumOriginalBytes: number;
  readonly maximumArtifactBytes: number;
  readonly maximumPreparedBytes: number;
}

/** Local observation controls. Omitted values come from the native defaults owner. */
export interface PrivateSettlementRunOptions {
  /** Zero performs no new proof reads. */
  readonly maxFinalityProofReads?: number;
  readonly observationTimeoutMs?: number;
  readonly pollIntervalMs?: number;
  readonly retransmitIdenticalBytes?: boolean;
  readonly retryRejectedStep?: boolean;
}

export type PrivateSettlementCommand = "prepare" | "prepared" | "run" | "recover";
export type PrivateSettlementOutcomeKind =
  | "opened" | "staged" | "prepared" | "not_prepared" | "pending"
  | "settled" | "aborted" | "reconciliation_pending_settled"
  | "reconciliation_pending_aborted" | "failed";
export type PrivateSettlementStatus =
  | "reserved" | "pending" | "ready" | "idle" | "closing" | "closed" | "unavailable";

/** Redacted native journal projection; observation data carries no receipt authority. */
export interface PrivateSettlementProgress {
  readonly workflowId: string;
  readonly status: string;
  readonly sequence: string;
  readonly cancelRequested: boolean;
  readonly currentStep: number | null;
  readonly failureCode: string | null;
  readonly confirmedSteps: readonly number[];
  readonly unresolvedSteps: readonly number[];
  readonly failedSteps: readonly number[];
}

/** Native-issued result; exported data alone is not an independent finality proof. */
export class PrivateSettlementOutcome {
  private constructor();
  private readonly nativeCapability;
  readonly kind: PrivateSettlementOutcomeKind;
  readonly failureCode: string | null;
  readonly progress: PrivateSettlementProgress | null;
  /** Bounded canonical Norito data; call before host shutdown. No import creates verified authority. */
  data(): Promise<Uint8Array>;
}

export class PrivateSettlementTicket {
  private constructor();
  private readonly nativeCapability;
  readonly status: PrivateSettlementStatus;
  wait(): Promise<PrivateSettlementStatus>;
  result(): Promise<PrivateSettlementOutcome>;
}

export class PrivateSettlementOperation {
  private constructor();
  private readonly nativeCapability;
  ready(): Promise<PrivateSettlementOutcome>;
  start(command: PrivateSettlementCommand, options?: PrivateSettlementRunOptions): Promise<PrivateSettlementTicket>;
  /** Explicit native provisioning after ready(); recovery never provisions inputs. */
  stageFiles(descriptor: Uint8Array): PrivateSettlementTicket;
  prepare(options?: PrivateSettlementRunOptions): Promise<PrivateSettlementOutcome>;
  prepared(options?: PrivateSettlementRunOptions): Promise<PrivateSettlementOutcome>;
  run(options?: PrivateSettlementRunOptions): Promise<PrivateSettlementOutcome>;
  recover(options?: PrivateSettlementRunOptions): Promise<PrivateSettlementOutcome>;
  /** Drain accepted native work and cleanup; this does not cancel a payment. */
  close(): Promise<void>;
}

export class PrivateSettlementHost {
  private constructor();
  private readonly nativeCapability;
  open(descriptor: Uint8Array): PrivateSettlementOperation;
  shutdown(): Promise<void>;
}

export function createPrivateSettlementHost(
  limits: PrivateSettlementHostLimits,
): Promise<PrivateSettlementHost>;
