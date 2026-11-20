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
import { NetworkId, validateNoritoFrame, type NoritoWireLayout, decodeTransactionPayloadJson, encodeTransactionPayloadJson, decodeSignedTransaction, encodeSignedTransaction, signTransactionPayload, transactionPayloadSigningHash, noritoEncodeInstruction, noritoDecodeInstructionBoxArchive, buildTransaction, type TransactionAssemblyInput } from "@iroha/iroha-js";
import {
  browserTransactionCodec,
  buildBrowserExecutableBatchPayload,
  buildBrowserTransferPayload,
  validateBrowserTransferSignable,
  browserTransactionPayloadHashHex,
  browserSignedTransactionHashHex,
  finalizeBrowserSignedTransaction,
  type BrowserTransactionSignable,
  type BrowserExecutableBatchInput,
  type BrowserTransferInput,
  type ValidatedBrowserTransactionSignable,
} from "@iroha/iroha-js/transaction-codec";

const codec: NexusTransactionCodec = browserTransactionCodec;
new NexusAppClient({
  chainDiscriminant: 753,
  transactionCodec: browserTransactionCodec,
});
const networkId = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);

const input: BrowserTransferInput = {
  networkId,
  networkPrefix: 753,
  authority: "sora-test-authority",
  sourceAssetHoldingId: "asset#sora-test-authority",
  quantity: "1",
  destinationAccountId: "sora-test-destination",
  feePayment: { payer: "authority", chargeLimits: [] },
  metadata: { decimalAsString: "1.25", integer: 1 },
};

const stronglyTypedPayload: Uint8Array = buildBrowserTransferPayload(input);
const denseLayout: NoritoWireLayout = "dense";
const densePayload: Uint8Array = buildBrowserTransferPayload(input, { layout: denseLayout });
browserTransactionPayloadHashHex(densePayload, 753, { layout: denseLayout });
browserSignedTransactionHashHex(new Uint8Array(), 753, { layout: denseLayout, format: "bare" });
const frameLayout: NoritoWireLayout = validateNoritoFrame(new Uint8Array(), { expectedLayout: denseLayout }).layout;
void frameLayout;
// @ts-expect-error A dense bare archive must never masquerade as version-1 submission wire.
browserSignedTransactionHashHex(new Uint8Array(), 753, { layout: "dense" });
// @ts-expect-error Unknown layout names are not aliases.
buildBrowserTransferPayload(input, { layout: "compact" });
// @ts-expect-error Options are exact.
buildBrowserTransferPayload(input, { layout: "dense", flags: 0 });
// @ts-expect-error Dense signing requires explicit offline bare format.
finalizeBrowserSignedTransaction({} as BrowserTransactionSignable, new Uint8Array(64), new Uint8Array(32), { layout: "dense", format: "versioned" });
const retiredChain: BrowserTransferInput = {
  ...input,
  // @ts-expect-error chain is not an ordinary-transaction domain.
  chain: "compile-time-chain",
};
const retiredChainId: BrowserTransferInput = {
  ...input,
  // @ts-expect-error chainId is not an ordinary-transaction domain.
  chainId: "compile-time-chain",
};
const retiredSnakeChain: BrowserTransferInput = {
  ...input,
  // @ts-expect-error chain_id is not an ordinary-transaction domain.
  chain_id: "compile-time-chain",
};
const batchInput: BrowserExecutableBatchInput = {
  networkId,
  networkPrefix: 753,
  authority: "sora-test-authority",
  entries: [
    { kind: "instruction", instruction: { Log: { level: "INFO", message: "before" } } },
    {
      kind: "contractCall",
      contractAddress: "irohac1example",
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
const removedWaitScope: NexusWaitFinalizeOptions = {
  wait: true,
  // @ts-expect-error finality waits are global-only.
  scope: "global",
};
const invalidSuccessOverride: NexusWaitFinalizeOptions = {
  wait: true,
  // @ts-expect-error the success state is immutable and exact Applied.
  successStatuses: new Set(["Committed"]),
};
const invalidFailureOverride: NexusWaitFinalizeOptions = {
  wait: true,
  // @ts-expect-error Rejected and Expired are the fixed failures.
  failureStatuses: new Set(["Committed"]),
};
const invalidTerminalOverride: NexusWaitFinalizeOptions = {
  wait: true,
  // @ts-expect-error finality terminal policy is not caller-selectable.
  terminalStatuses: new Set(["Committed"]),
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
void retiredChain;
void retiredChainId;
void retiredSnakeChain;
void stronglyTypedBatchPayload;
void codecPayload;
void validated;
void finalizeOptions;
void removedWaitScope;
void invalidSuccessOverride;
void invalidFailureOverride;
void invalidTerminalOverride;
void submittedErrorContext;
void invalidNoWaitOptions;

// Native typed model APIs retain exact JSON text and explicitly select archive format.
const nativeOptions = { layout: "dense", format: "bare" } as const;
const nativeJson: string = decodeTransactionPayloadJson(densePayload, 753, nativeOptions);
encodeTransactionPayloadJson(nativeJson, 753, nativeOptions);
const nativeSignedJson: string = decodeSignedTransaction(densePayload, 753, { ...nativeOptions, parseJson: false });
encodeSignedTransaction(nativeSignedJson, 753, nativeOptions);
signTransactionPayload(networkId, densePayload, new Uint8Array(32), 753, nativeOptions);
transactionPayloadSigningHash(densePayload, nativeOptions);
noritoEncodeInstruction({}, 753, { layout: "dense" });
noritoDecodeInstructionBoxArchive(densePayload, 753, { layout: "dense" });
buildTransaction({} as TransactionAssemblyInput, nativeOptions);
// @ts-expect-error Dense has no versioned signed mapping.
encodeSignedTransaction(nativeSignedJson, 753, { layout: "dense" });
// @ts-expect-error Typed JSON encoding requires lossless JSON text.
encodeTransactionPayloadJson({}, 753, nativeOptions);
// @ts-expect-error Payloads have no versioned mapping.
transactionPayloadSigningHash(densePayload, { format: "versioned" });
// @ts-expect-error No layout inference/alias option.
decodeSignedTransaction(densePayload, 753, { auto: true });
