import type {
  ToriiClient,
  RequiredCanonicalRequestOptions,
  ToriiElectionTally,
  ToriiGovernanceLockRecord,
  ToriiGovernanceReferendumRecord,
  ToriiGovernanceTally,
} from "../../../index.js";

declare const client: ToriiClient;
declare const auth: RequiredCanonicalRequestOptions;
declare const referendum: ToriiGovernanceReferendumRecord;
declare const lock: ToriiGovernanceLockRecord;
declare const tally: ToriiGovernanceTally;
declare const electionTally: ToriiElectionTally;

const exactVotes: number | bigint = tally.approve;
const exactElectionWeight: number | bigint = electionTally.tally[0];
const exactHeight: number | bigint = tally.evaluated_block_height;
const evaluatedHash: string = tally.evaluated_block_hash;
const exactDuration: number | bigint = lock.duration_blocks;
const custodyAsset: string = lock.custody.asset_definition_id;
if (referendum.mode === "Plain") {
  const scale: number = referendum.plain_context.content.asset_scale;
  const minimum: number | bigint = referendum.plain_context.content.minimum_turnout;
  if (referendum.status === "Closed") {
    const immutableApproved: boolean = referendum.plain_result.content.approved;
    const exactFinal: number | bigint = referendum.plain_result.content.approve;
    void [immutableApproved, exactFinal];
  } else {
    const pending: "Pending" = referendum.plain_result.kind;
    const noDecision: null = referendum.plain_result.content;
    void [pending, noDecision];
  }
  void [scale, minimum];
} else {
  const notApplicable: "NotApplicable" = referendum.plain_context.kind;
  void notApplicable;
}

async function read() {
  const raw = await client.getGovernanceReferendum("ref-1", auth);
  if (raw?.found) {
    const context = raw.referendum.plain_context;
    void context;
  }
  const rawLocks = await client.getGovernanceLocks("ref-1", auth);
  if (rawLocks?.found) {
    const inner: Record<string, ToriiGovernanceLockRecord> = rawLocks.locks.locks;
    void inner;
  }
  const rawTally = await client.getGovernanceTally("ref-1", auth);
  const count: number | bigint | undefined = rawTally?.approve;
  const election = await client.getElectionTally("election-1", auth);
  const weight: number | bigint | undefined = election?.tally[0];
  void [count, weight];
}

// @ts-expect-error authoritative tally counts are not safely narrowed to Number
const rounded: number = tally.approve;
// @ts-expect-error standalone-election weights may exceed Number.MAX_SAFE_INTEGER
const roundedElectionWeight: number = electionTally.tally[0];
// @ts-expect-error authoritative lock custody is required and cannot be null
const nullCustody: ToriiGovernanceLockRecord = { ...lock, custody: null };
// @ts-expect-error the frozen context is required even before the first ballot
const missingContext: ToriiGovernanceReferendumRecord = {
  h_start: 1, h_end: 2, status: "Open", mode: "Plain",
  plain_result: { kind: "Pending", content: null },
};
void [exactVotes, exactElectionWeight, exactHeight, evaluatedHash, exactDuration, custodyAsset, read,
  rounded, roundedElectionWeight, nullCustody, missingContext];
