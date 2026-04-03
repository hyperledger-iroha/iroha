#!/usr/bin/env node
/**
 * Governance helper walkthrough.
 *
 * Builds sample transactions for:
 *   1. Proposing a contract deployment
 *   2. Casting a plain ballot
 *   3. Enacting a referendum (after approval)
 *   4. Finalizing the referendum
 *   5. Persisting a council snapshot
 *
 * By default the script only prints the deterministic hashes. Set
 *   GOV_SUBMIT=1 TORII_URL=http://localhost:8080 AUTHORITY=sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB PRIVATE_KEY_HEX=...
 * to submit them to a Torii node (requires the account to hold the relevant permissions).
 */
import { Buffer } from "node:buffer";
import { ToriiClient } from "../src/index.js";
import {
  buildProposeDeployContractTransaction,
  buildCastPlainBallotTransaction,
  buildEnactReferendumTransaction,
  buildFinalizeReferendumTransaction,
  buildPersistCouncilForEpochTransaction,
  hashSignedTransaction,
} from "../src/index.js";

const TORII_URL = process.env.TORII_URL ?? "http://localhost:8080";
const SHOULD_SUBMIT = process.env.GOV_SUBMIT === "1";
const SHOULD_FETCH = process.env.GOV_FETCH === "1";
const CHAIN_ID = process.env.CHAIN_ID ?? "00000000-0000-0000-0000-000000000000";
const AUTHORITY =
  process.env.AUTHORITY ??
  "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
const PRIVATE_KEY =
  process.env.PRIVATE_KEY_HEX != null
    ? Buffer.from(process.env.PRIVATE_KEY_HEX, "hex")
    : Buffer.alloc(32, 0x11);

const SAMPLE_CONTRACT_ADDRESS =
  process.env.GOV_CONTRACT_ADDRESS ??
  "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7";
const SAMPLE_REFERENDUM_ID = "demo-referendum";
const SAMPLE_REFERENDUM_HASH = Buffer.alloc(32, 0xaa);
const SAMPLE_PROPOSAL_HASH = Buffer.alloc(32, 0xbb);
const GOV_PROPOSAL_ID = process.env.GOV_PROPOSAL_ID;
const GOV_REFERENDUM_ID = process.env.GOV_REFERENDUM_ID;
const GOV_LOCKS_ID = process.env.GOV_LOCKS_ID ?? GOV_REFERENDUM_ID;

function logTransaction(label, tx) {
  const hashHex = tx.hash.toString("hex");
  console.log(`\n[${label}]`);
  console.log("  hash:", hashHex);
  try {
    const recomputed = hashSignedTransaction(tx.signedTransaction);
    console.log("  matches recomputed hash:", recomputed === hashHex);
  } catch (error) {
    console.log(
      "  matches recomputed hash: skipped (native hash helper unavailable:",
      error?.message ?? error,
      ")",
    );
  }
  console.log("  signedTransaction bytes:", tx.signedTransaction.length);
}

async function maybeSubmit(client, label, tx) {
  if (!SHOULD_SUBMIT) {
    return;
  }
  try {
    const submission = await client.submitTransaction(tx.signedTransaction);
    console.log(`  submitted via Torii (${label}):`, submission ?? "<empty>");
  } catch (error) {
    console.warn(`  submission failed for ${label}:`, error.message ?? error);
  }
}

async function main() {
  const transactions = [
    {
      label: "ProposeDeployContract",
      build: () =>
        buildProposeDeployContractTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          proposal: {
            contractAddress: SAMPLE_CONTRACT_ADDRESS,
            codeHash: Buffer.alloc(32, 0xcd),
            abiHash: Buffer.alloc(32, 0xef),
            abiVersion: "1",
            window: { lower: 100, upper: 200 },
            votingMode: "Plain",
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "CastPlainBallot",
      build: () =>
        buildCastPlainBallotTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          ballot: {
            referendumId: SAMPLE_REFERENDUM_ID,
            owner: AUTHORITY,
            amount: "2500",
            durationBlocks: 7200,
            direction: "aye",
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "EnactReferendum",
      build: () =>
        buildEnactReferendumTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          enactment: {
            referendumId: SAMPLE_REFERENDUM_HASH,
            preimageHash: Buffer.alloc(32, 0xee),
            window: { lower: 300, upper: 360 },
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "FinalizeReferendum",
      build: () =>
        buildFinalizeReferendumTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          finalization: {
            referendumId: SAMPLE_REFERENDUM_ID,
            proposalId: SAMPLE_PROPOSAL_HASH,
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "PersistCouncilForEpoch",
      build: () =>
        buildPersistCouncilForEpochTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          record: {
            epoch: 42,
            members: [AUTHORITY],
            candidatesCount: 10,
            derivedBy: "Vrf",
          },
          privateKey: PRIVATE_KEY,
        }),
    },
  ];

  const needsClient = SHOULD_SUBMIT || SHOULD_FETCH;
  const client = needsClient ? new ToriiClient(TORII_URL) : null;
  console.log(
    `Building governance transactions (submit=${SHOULD_SUBMIT ? "yes" : "no"}, fetch=${
      SHOULD_FETCH ? "yes" : "no"
    })`,
  );

  for (const entry of transactions) {
    const tx = entry.build();
    logTransaction(entry.label, tx);
    if (client) {
      // eslint-disable-next-line no-await-in-loop
      await maybeSubmit(client, entry.label, tx);
    }
  }

  if (client && SHOULD_FETCH) {
    await inspectGovernance(client);
  }

  if (!SHOULD_SUBMIT) {
    console.log(
      "\nSet GOV_SUBMIT=1 (plus TORII_URL, AUTHORITY, PRIVATE_KEY_HEX) to push these transactions to a node.",
    );
  }
}

async function inspectGovernance(client) {
  console.log("\nInspecting governance state via Torii...");
  await fetchProposal(client);
  await fetchReferendum(client);
  await fetchLocks(client);
  await fetchUnlockStats(client);
}

async function fetchProposal(client) {
  if (!GOV_PROPOSAL_ID) {
    console.log("  GOV_PROPOSAL_ID not set; skipping proposal lookup.");
    return;
  }
  await logJsonResult(
    `proposal:${GOV_PROPOSAL_ID}`,
    () => client.getGovernanceProposalTyped(GOV_PROPOSAL_ID),
  );
}

async function fetchReferendum(client) {
  if (!GOV_REFERENDUM_ID) {
    console.log("  GOV_REFERENDUM_ID not set; skipping referendum lookup.");
    return;
  }
  await logJsonResult(
    `referendum:${GOV_REFERENDUM_ID}`,
    () => client.getGovernanceReferendumTyped(GOV_REFERENDUM_ID),
  );
  await logJsonResult(
    `tally:${GOV_REFERENDUM_ID}`,
    () => client.getGovernanceTallyTyped(GOV_REFERENDUM_ID),
  );
}

async function fetchLocks(client) {
  if (!GOV_LOCKS_ID) {
    console.log("  GOV_LOCKS_ID not set; skipping lock lookup.");
    return;
  }
  await logJsonResult(
    `locks:${GOV_LOCKS_ID}`,
    () => client.getGovernanceLocksTyped(GOV_LOCKS_ID),
  );
}

async function fetchUnlockStats(client) {
  await logJsonResult("unlock_stats", () => client.getGovernanceUnlockStatsTyped());
}

async function logJsonResult(label, fetcher) {
  try {
    const payload = await fetcher();
    console.log(`  ${label}:`);
    console.log(indent(JSON.stringify(payload, null, 2), 4));
  } catch (error) {
    console.warn(`  ${label} failed:`, error?.message ?? error);
  }
}

function indent(text, spaces) {
  const pad = " ".repeat(spaces);
  return text
    .split("\n")
    .map((line) => pad + line)
    .join("\n");
}

main().catch((error) => {
  console.error("governance recipe failed:", error);
  process.exitCode = 1;
});
