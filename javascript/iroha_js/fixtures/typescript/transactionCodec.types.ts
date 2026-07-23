import {
  NexusAppClient,
  type NexusAppErrorContext,
  type NexusBytes,
  type NexusFinalizeOptions,
  type NexusNoWaitFinalizeOptions,
  type NexusTransactionPayloadResult,
  type NexusTransactionCodec,
  type NexusWaitFinalizeOptions,
} from "@iroha/iroha-js/nexus-app";
import {
  browserTransactionCodec,
  buildBrowserExecutableBatchPayload,
  buildBrowserTransferPayload,
  validateBrowserTransferSignable,
  type BrowserTransactionSignable,
  type BrowserExecutableBatchInput,
  type BrowserTransferInput,
  type ValidatedBrowserTransactionSignable,
} from "@iroha/iroha-js/transaction-codec";

const codec: NexusTransactionCodec = browserTransactionCodec;
new NexusAppClient({ transactionCodec: browserTransactionCodec });

const input: BrowserTransferInput = {
  chainId: "compile-time-chain",
  authority: "sora-test-authority",
  sourceAssetHoldingId: "asset#sora-test-authority",
  quantity: "1",
  destinationAccountId: "sora-test-destination",
  feePayment: { payer: "authority", chargeLimits: [] },
  metadata: { decimalAsString: "1.25", integer: 1 },
};

const stronglyTypedPayload: Uint8Array = buildBrowserTransferPayload(input);
const batchInput: BrowserExecutableBatchInput = {
  chainId: "compile-time-chain",
  authority: "sora-test-authority",
  entries: [
    { kind: "instruction", instruction: { Log: { level: "INFO", message: "before" } } },
    {
      kind: "contractCall",
      contractAddress: "sorac1example",
      expectedCodeHash: new Uint8Array(32),
      entrypoint: "run",
      arguments: new Uint8Array([1, 2, 3]),
    },
  ],
  feePayment: { payer: "authority", chargeLimits: [], gasLimit: 1_000 },
};
const stronglyTypedBatchPayload: Uint8Array =
  buildBrowserExecutableBatchPayload(batchInput);
const codecPayload: NexusBytes | NexusTransactionPayloadResult =
  codec.buildTransferPayload(input as unknown as Record<string, unknown>);
declare const signable: BrowserTransactionSignable;
const validated: Readonly<ValidatedBrowserTransactionSignable> =
  validateBrowserTransferSignable(signable);
const waitOptions: NexusWaitFinalizeOptions = {
  wait: true,
  signal: new AbortController().signal,
};
const invalidSuccessOverride: NexusWaitFinalizeOptions = {
  wait: true,
  // @ts-expect-error the success state is immutable and exact Applied.
  successStatuses: new Set(["Committed"]),
};
const noWaitOptions: NexusNoWaitFinalizeOptions = { wait: false };
const finalizeOptions: readonly NexusFinalizeOptions[] = [
  waitOptions,
  noWaitOptions,
];
const submittedErrorContext: NexusAppErrorContext = {
  phase: "status_wait",
  submissionState: "submitted",
  signedTransactionHashHex: "0".repeat(64),
};
// @ts-expect-error no-wait submissions must reject status-only options.
const invalidNoWaitOptions: NexusFinalizeOptions = {
  wait: false,
  signal: new AbortController().signal,
};

void stronglyTypedPayload;
void stronglyTypedBatchPayload;
void codecPayload;
void validated;
void finalizeOptions;
void submittedErrorContext;
void invalidNoWaitOptions;
