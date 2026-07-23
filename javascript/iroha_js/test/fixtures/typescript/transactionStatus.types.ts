import type {
  IvmProvedContractCallOptions,
  ToriiBrowserTransactionStatusOptions,
  ToriiBrowserTransactionStatusPollOptions,
  ToriiClientOptions,
  TransactionStatusPollOptions,
  TransactionStatusReadOptions,
} from "../../../index.js";

const rawNodeStatus: TransactionStatusReadOptions = {
  allowShortHash: false,
  scope: "local",
};
const rawBrowserStatus: ToriiBrowserTransactionStatusOptions = {
  scope: "global",
};
const nodePoll: TransactionStatusPollOptions = {
  intervalMs: 10,
};
const browserPoll: ToriiBrowserTransactionStatusPollOptions = {
  intervalMs: 10,
};
const provedCall: IvmProvedContractCallOptions = {
  waitForCommit: true,
  transactionIntervalMs: 10,
};

const removedNodeAutoScope: TransactionStatusReadOptions = {
  // @ts-expect-error raw reads expose only explicit local or global scope.
  scope: "auto",
};
const removedBrowserAutoScope: ToriiBrowserTransactionStatusOptions = {
  // @ts-expect-error raw reads expose only explicit local or global scope.
  scope: "auto",
};
const removedNodeFallback: TransactionStatusReadOptions = {
  // @ts-expect-error cross-endpoint status fallback is not supported.
  endpoints: ["https://fallback.example"],
};
const removedClientFallback: ToriiClientOptions = {
  // @ts-expect-error cross-endpoint status fallback is not configurable.
  statusEndpoints: ["https://fallback.example"],
};
const removedClientPollScope: ToriiClientOptions = {
  // @ts-expect-error finality polling scope is not configurable.
  transactionStatusScope: "local",
};
const removedNestedClientFallback: ToriiClientOptions = {
  config: {
    toriiClient: {
      // @ts-expect-error nested cross-endpoint status fallback is not configurable.
      statusEndpoints: ["https://fallback.example"],
    },
  },
};
const removedNestedClientPollScope: ToriiClientOptions = {
  config: {
    toriiClient: {
      // @ts-expect-error nested finality polling scope is not configurable.
      transactionStatusScope: "local",
    },
  },
};
const removedNodePollScope: TransactionStatusPollOptions = {
  // @ts-expect-error finality waits are global-only.
  scope: "global",
};
const removedBrowserPollScope: ToriiBrowserTransactionStatusPollOptions = {
  // @ts-expect-error finality waits are global-only.
  scope: "global",
};
const removedProvedCallScope: IvmProvedContractCallOptions = {
  // @ts-expect-error finality waits are global-only.
  transactionStatusScope: "global",
};

void rawNodeStatus;
void rawBrowserStatus;
void nodePoll;
void browserPoll;
void provedCall;
void removedNodeAutoScope;
void removedBrowserAutoScope;
void removedNodeFallback;
void removedClientFallback;
void removedClientPollScope;
void removedNestedClientFallback;
void removedNestedClientPollScope;
void removedNodePollScope;
void removedBrowserPollScope;
void removedProvedCallScope;
