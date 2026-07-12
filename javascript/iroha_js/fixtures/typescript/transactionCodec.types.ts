import {
  NexusAppClient,
  type NexusBytes,
  type NexusTransactionPayloadResult,
  type NexusTransactionCodec,
} from "@iroha/iroha-js/nexus-app";
import {
  browserTransactionCodec,
  buildBrowserTransferPayload,
  validateBrowserTransferSignable,
  type BrowserTransactionSignable,
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
  metadata: { decimalAsString: "1.25", integer: 1 },
};

const stronglyTypedPayload: Buffer = buildBrowserTransferPayload(input);
const codecPayload: NexusBytes | NexusTransactionPayloadResult =
  codec.buildTransferPayload(input as unknown as Record<string, unknown>);
declare const signable: BrowserTransactionSignable;
const validated: Readonly<ValidatedBrowserTransactionSignable> =
  validateBrowserTransferSignable(signable);

void stronglyTypedPayload;
void codecPayload;
void validated;
