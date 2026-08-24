#!/usr/bin/env node
/**
 * Governance helper walkthrough.
 *
 * Builds sample transactions for:
 *   1. Proposing a contract deployment
 *   2. Casting a plain ballot
 *   3. Persisting a council snapshot
 *
 * Every transaction is quoted before signing. By default the script only
 * prints the resulting hashes. Set
 *   GOV_SUBMIT=1 TORII_URL=http://localhost:8080 AUTHORITY=sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV PRIVATE_KEY_HEX=...
 * to submit them to a Torii node (requires the account to hold the relevant permissions).
 */
import { Buffer } from "node:buffer";
import { NetworkId, ToriiClient } from "../src/index.js";
import {
  buildProposeDeployContractInstruction,
  buildCastPlainBallotInstruction,
  buildPersistCouncilForEpochInstruction,
  hashSignedTransaction,
  quoteAndSignTransaction,
} from "../src/index.js";

const TORII_URL = process.env.TORII_URL ?? "http://localhost:8080";
const SHOULD_SUBMIT = process.env.GOV_SUBMIT === "1";
const SHOULD_FETCH = process.env.GOV_FETCH === "1";
const NETWORK_ID = NetworkId.parse(
  process.env.NETWORK_ID ??
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const AUTHORITY =
  process.env.AUTHORITY ??
  "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
const PRIVATE_KEY =
  process.env.PRIVATE_KEY_HEX != null
    ? Buffer.from(process.env.PRIVATE_KEY_HEX, "hex")
    : Buffer.from(
        "CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
        "hex",
      );
const REQUESTED_FEE_PAYMENT = process.env.FEE_SPONSOR_PROGRAM
  ? {
      payer: "sponsor",
      programId: process.env.FEE_SPONSOR_PROGRAM,
      programRevision: Number(process.env.FEE_SPONSOR_PROGRAM_REVISION),
      chargeLimits: [],
    }
  : { payer: "authority", chargeLimits: [] };

const SAMPLE_CONTRACT_ADDRESS =
  process.env.GOV_CONTRACT_ADDRESS ??
  "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
const SAMPLE_REFERENDUM_ID = "demo-referendum";
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
      buildInstruction: () =>
        buildProposeDeployContractInstruction({
          contractAddress: SAMPLE_CONTRACT_ADDRESS,
          codeHash: Buffer.alloc(32, 0xcd),
          abiHash: Buffer.alloc(32, 0xef),
          abiVersion: 1,
        }),
    },
    {
      label: "CastPlainBallot",
      buildInstruction: () =>
        buildCastPlainBallotInstruction({
          referendumId: SAMPLE_REFERENDUM_ID,
          owner: AUTHORITY,
          amount: "2500",
          durationBlocks: 7200,
          direction: "aye",
        }),
    },
    {
      label: "PersistCouncilForEpoch",
      buildInstruction: () =>
        buildPersistCouncilForEpochInstruction({
          epoch: 42,
          members: [AUTHORITY],
        }),
    },
  ];

  if (REQUESTED_FEE_PAYMENT.payer === "sponsor" &&
      (!Number.isSafeInteger(REQUESTED_FEE_PAYMENT.programRevision) ||
       REQUESTED_FEE_PAYMENT.programRevision <= 0)) {
    throw new Error("FEE_SPONSOR_PROGRAM requires a positive FEE_SPONSOR_PROGRAM_REVISION");
  }
  const client = new ToriiClient(TORII_URL);
  const canonicalAuth = { accountId: AUTHORITY, privateKey: PRIVATE_KEY };
  console.log(
    `Building governance transactions (submit=${SHOULD_SUBMIT ? "yes" : "no"}, fetch=${
      SHOULD_FETCH ? "yes" : "no"
    })`,
  );

  for (const entry of transactions) {
    // eslint-disable-next-line no-await-in-loop
    const tx = await quoteAndSignTransaction(
      client,
      {
        networkId: NETWORK_ID,
        authority: AUTHORITY,
        instructions: [entry.buildInstruction()],
        feePayment: REQUESTED_FEE_PAYMENT,
        privateKey: PRIVATE_KEY,
      },
      { canonicalAuth },
    );
    logTransaction(entry.label, tx);
    // eslint-disable-next-line no-await-in-loop
    await maybeSubmit(client, entry.label, tx);
  }

  if (SHOULD_FETCH) {
    await inspectGovernance(client, canonicalAuth);
  }

  if (!SHOULD_SUBMIT) {
    console.log(
      "\nSet GOV_SUBMIT=1 (plus TORII_URL, AUTHORITY, PRIVATE_KEY_HEX) to push these transactions to a node.",
    );
  }
}

async function inspectGovernance(client, canonicalAuth) {
  console.log("\nInspecting governance state via Torii...");
  await fetchProposal(client, canonicalAuth);
  await fetchReferendum(client, canonicalAuth);
  await fetchLocks(client, canonicalAuth);
  await fetchUnlockStats(client, canonicalAuth);
}

async function fetchProposal(client, canonicalAuth) {
  if (!GOV_PROPOSAL_ID) {
    console.log("  GOV_PROPOSAL_ID not set; skipping proposal lookup.");
    return;
  }
  await logJsonResult(
    `proposal:${GOV_PROPOSAL_ID}`,
    () => client.getGovernanceProposalTyped(GOV_PROPOSAL_ID, { canonicalAuth }),
  );
}

async function fetchReferendum(client, canonicalAuth) {
  if (!GOV_REFERENDUM_ID) {
    console.log("  GOV_REFERENDUM_ID not set; skipping referendum lookup.");
    return;
  }
  await logJsonResult(
    `referendum:${GOV_REFERENDUM_ID}`,
    () => client.getGovernanceReferendumTyped(GOV_REFERENDUM_ID, { canonicalAuth }),
  );
  await logJsonResult(
    `tally:${GOV_REFERENDUM_ID}`,
    () => client.getGovernanceTallyTyped(GOV_REFERENDUM_ID, { canonicalAuth }),
  );
}

async function fetchLocks(client, canonicalAuth) {
  if (!GOV_LOCKS_ID) {
    console.log("  GOV_LOCKS_ID not set; skipping lock lookup.");
    return;
  }
  await logJsonResult(
    `locks:${GOV_LOCKS_ID}`,
    () => client.getGovernanceLocksTyped(GOV_LOCKS_ID, { canonicalAuth }),
  );
}

async function fetchUnlockStats(client, canonicalAuth) {
  await logJsonResult(
    "unlock_stats",
    () => client.getGovernanceUnlockStatsTyped({ canonicalAuth }),
  );
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
