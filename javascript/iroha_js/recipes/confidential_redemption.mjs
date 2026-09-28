#!/usr/bin/env node
// Local proving only: this recipe creates no ledger note and submits no transaction.
import { randomBytes } from "node:crypto";
import {
  ConfidentialProver, NetworkId, computeConfidentialRoot,
  deriveConfidentialReceiveAddressV2, deriveConfidentialNoteV2,
} from "../src/index.js";

const spendKey = randomBytes(32);
const diversifierSeed = randomBytes(32);
const rho = randomBytes(32);
// A disposable local context, not the NetworkId of a deployed ledger.
const networkBytes = randomBytes(32);
networkBytes[31] |= 1; // Preserve the canonical Iroha hash marker.
const networkId = NetworkId.fromBytes(networkBytes);
const assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
let prover;
let timer;
try {
  const address = deriveConfidentialReceiveAddressV2({ spendKey, diversifierSeed });
  const rhoHex = rho.toString("hex");
  const { commitment } = deriveConfidentialNoteV2({
    assetDefinitionId, amount: 7n, rhoHex, ownerTagHex: address.ownerTagHex,
  });
  const root = await computeConfidentialRoot({ commitments: [commitment] });
  prover = new ConfidentialProver({ networkId, assetDefinitionId, spendKey });
  spendKey.fill(0);
  diversifierSeed.fill(0);
  rho.fill(0);
  let eventLoopTicks = 0;
  timer = setInterval(() => { eventLoopTicks += 1; }, 10);
  const pending = prover.proveRedemption({
    treeCommitments: [commitment], rootHex: root.toString("hex"), publicAmount: 7n,
    inputs: [{ amount: 7n, rhoHex, diversifierHex: address.diversifierHex, leafIndex: 0 }],
  });
  // The queued native job owns its key and witnesses independently.
  prover.dispose();
  const proof = await pending;
  console.log(JSON.stringify({
    relation: proof.relation, proofBytes: proof.proof.length,
    nullifiers: proof.nullifiers.length, outputs: proof.outputCommitments.length,
    rootMatches: proof.root.equals(root), eventLoopTicks,
  }));
} finally {
  clearInterval(timer);
  prover?.dispose();
  spendKey.fill(0);
  diversifierSeed.fill(0);
  rho.fill(0);
}
// JavaScript strings remain runtime-managed. A computed history root must be
// authenticated separately before using a real ledger's notes and admission path.
