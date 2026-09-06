import type {
  IvmProvedContractCallOptions,
  ToriiAppliedTransactionStatus,
  ToriiBrowserRequestOptions,
  ToriiBrowserTransactionStatusOptions,
  ToriiBrowserTransactionStatusPollOptions,
  ToriiClientOptions,
  TransactionStatusPollOptions,
  TransactionStatusReadOptions,
} from "../../../index.js";

// @ts-expect-error the pre-release generic finality alias is not exported.
type RemovedGenericFinalityAlias = import("../../../index.js").ToriiPipelineStatus;

const exactAppliedFinality: ToriiAppliedTransactionStatus = {
  hash: "ab".repeat(32),
  status: { kind: "Applied", block_height: 1 },
  scope: "global",
  resolved_from: "state",
};

const rawNodeStatus: TransactionStatusReadOptions = {
  scope: "local",
};
const rawBrowserStatus: ToriiBrowserTransactionStatusOptions = {
  scope: "global",
};
const genericBrowserHttpRequest: ToriiBrowserRequestOptions = {
  // @ts-expect-error request success status policy is fixed by each route.
  successStatuses: [200, 202],
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
const removedNodeShortHashOption: TransactionStatusReadOptions = {
  // @ts-expect-error first-release status reads accept only exact full hashes.
  allowShortHash: false,
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
const removedNodeSuccessSelector: TransactionStatusPollOptions = {
  // @ts-expect-error finality success is fixed to state-resolved Applied.
  successStatuses: ["Committed"],
};
const removedNodeFailureSelector: TransactionStatusPollOptions = {
  // @ts-expect-error finality failures are fixed to Rejected or Expired.
  failureStatuses: ["Committed"],
};
const removedNodeTerminalSelector: TransactionStatusPollOptions = {
  // @ts-expect-error terminal status policy is not caller-selectable.
  terminalStatuses: ["Committed"],
};
const removedBrowserPollScope: ToriiBrowserTransactionStatusPollOptions = {
  // @ts-expect-error finality waits are global-only.
  scope: "global",
};
const removedBrowserReadHttpSelector: ToriiBrowserTransactionStatusOptions = {
  // @ts-expect-error status reads accept only the fixed HTTP 200/404 contract.
  successStatuses: [200, 202],
};
const removedBrowserSuccessSelector: ToriiBrowserTransactionStatusPollOptions = {
  // @ts-expect-error finality waits do not inherit generic HTTP success selectors.
  successStatuses: [200, 202],
};
const removedBrowserFailureSelector: ToriiBrowserTransactionStatusPollOptions = {
  // @ts-expect-error finality failure policy is fixed.
  failureStatuses: ["Committed"],
};
const removedBrowserTerminalSelector: ToriiBrowserTransactionStatusPollOptions = {
  // @ts-expect-error finality terminal policy is fixed.
  terminalStatuses: ["Committed"],
};
const removedProvedCallScope: IvmProvedContractCallOptions = {
  // @ts-expect-error finality waits are global-only.
  transactionStatusScope: "global",
};

void rawNodeStatus;
void exactAppliedFinality;
void rawBrowserStatus;
void genericBrowserHttpRequest;
void nodePoll;
void browserPoll;
void provedCall;
void removedNodeAutoScope;
void removedNodeShortHashOption;
void removedBrowserAutoScope;
void removedNodeFallback;
void removedClientFallback;
void removedClientPollScope;
void removedNestedClientFallback;
void removedNestedClientPollScope;
void removedNodePollScope;
void removedNodeSuccessSelector;
void removedNodeFailureSelector;
void removedNodeTerminalSelector;
void removedBrowserPollScope;
void removedBrowserReadHttpSelector;
void removedBrowserSuccessSelector;
void removedBrowserFailureSelector;
void removedBrowserTerminalSelector;
void removedProvedCallScope;
void (null as unknown as RemovedGenericFinalityAlias);
