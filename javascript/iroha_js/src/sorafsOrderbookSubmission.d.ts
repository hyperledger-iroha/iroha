export interface SorafsOrderbookTransactionSubmitOptions {
  expectedReceiptSigner: string;
  signal?: AbortSignal;
}

export type SorafsOrderbookSignedTransaction = ArrayBuffer | ArrayBufferView;

export interface SorafsOrderbookSubmissionIdentity {
  readonly entrypointHash: string;
  readonly signedTransactionHash: string;
}

export interface SorafsOrderbookSubmissionReceipt {
  readonly payload: {
    readonly entrypoint_hash: string;
    readonly signed_transaction_hash: string;
    readonly submitted_at_ms: number | bigint;
    readonly submitted_at_height: number | bigint;
    readonly signer: string;
  };
  readonly signature: string;
}

export declare class SorafsOrderbookSubmissionAmbiguousError extends Error {
  constructor(
    route: string,
    expectedIdentity: SorafsOrderbookSubmissionIdentity,
    cause?: unknown,
  );
  readonly route: string;
  readonly expectedIdentity: Readonly<SorafsOrderbookSubmissionIdentity>;
}
