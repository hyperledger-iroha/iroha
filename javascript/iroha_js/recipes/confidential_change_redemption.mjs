#!/usr/bin/env node
// Disposable local proofs only; no ledger notes, authenticated roots, or transactions.
// In a real wallet, persist each private opening securely before proving and obtain
// its new leaf index/root from authenticated protocol state before spending it.
import assert from "node:assert/strict";
import { randomFillSync } from "node:crypto";
import {
  ConfidentialProver,
  NetworkId,
  computeConfidentialRoot,
  confidentialChangeToInput,
  defaultConfidentialDiversifier,
  deriveConfidentialNoteV2,
  deriveConfidentialOwnerTagV2,
  deriveConfidentialReceiveAddressV2,
} from "@iroha/iroha-js";

const spendKey = Buffer.alloc(32);
const diversifierSeed = Buffer.alloc(32);
const inputRho = Buffer.alloc(32);
const changeRho = Buffer.alloc(32);
const networkBytes = Buffer.alloc(32);
const assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
let prover;
let timer;
try {
  for (const bytes of [spendKey, diversifierSeed, inputRho, changeRho, networkBytes]) {
    randomFillSync(bytes);
  }
  networkBytes[31] |= 1;
  const networkId = NetworkId.fromBytes(networkBytes);
  const address = deriveConfidentialReceiveAddressV2({ spendKey, diversifierSeed });
  const defaultDiversifier = defaultConfidentialDiversifier();
  assert.notEqual(address.diversifierHex, defaultDiversifier.toString("hex"));
  const changeOwner = deriveConfidentialOwnerTagV2(spendKey, {
    diversifierHex: defaultDiversifier.toString("hex"),
  });
  const originalInput = {
    amount: 7n, rhoHex: inputRho.toString("hex"),
    diversifierHex: address.diversifierHex, leafIndex: 0,
  };
  // Retain the opening before submitting a proving job. Memory suffices only
  // for this disposable example; a real wallet needs secure durable storage.
  const retainedChange = Object.freeze({ amount: 2n, rhoHex: changeRho.toString("hex") });
  const inputCommitment = deriveConfidentialNoteV2({
    assetDefinitionId, amount: originalInput.amount, rhoHex: originalInput.rhoHex,
    ownerTagHex: address.ownerTagHex,
  }).commitment;
  const expectedChangeCommitment = deriveConfidentialNoteV2({
    assetDefinitionId, ...retainedChange, ownerTagHex: changeOwner.toString("hex"),
  }).commitment;
  const originalRoot = await computeConfidentialRoot({ commitments: [inputCommitment] });
  prover = new ConfidentialProver({ networkId, assetDefinitionId, spendKey });
  spendKey.fill(0);
  diversifierSeed.fill(0);
  inputRho.fill(0);
  changeRho.fill(0);

  let firstTicks = 0;
  timer = setInterval(() => { firstTicks += 1; }, 10);
  const first = await prover.proveRedemption({
    treeCommitments: [inputCommitment], rootHex: originalRoot.toString("hex"),
    inputs: [originalInput], publicAmount: 5n, change: retainedChange,
  });
  clearInterval(timer);
  assert.equal(first.relation, "confidential-redemption-with-change");
  assert.equal(first.nullifiers.length, 1);
  assert.deepEqual(first.outputCommitments, [expectedChangeCommitment]);
  assert.deepEqual(first.root, originalRoot);
  assert.ok(first.proof.length > 0);
  assert.ok(firstTicks > 0, "first proof must yield to the JavaScript event loop");

  // This example constructs its own local history. Production callers must
  // authenticate both the index and root independently before this conversion.
  const changeLeafIndex = 1;
  const history = [inputCommitment, first.outputCommitments[0]];
  const root = await computeConfidentialRoot({ commitments: history });
  const changeInput = confidentialChangeToInput(retainedChange, changeLeafIndex);
  assert.equal(changeInput.diversifierHex, defaultDiversifier.toString("hex"));
  assert.notEqual(changeInput.diversifierHex, originalInput.diversifierHex);
  assert.equal(retainedChange.amount, 2n);
  const request = {
    treeCommitments: history, rootHex: root.toString("hex"),
    inputs: [changeInput], publicAmount: 2n,
  };
  let secondTicks = 0;
  timer = setInterval(() => { secondTicks += 1; }, 10);
  const pending = prover.proveRedemption(request);
  prover.dispose(); // The accepted native job owns its key and note independently.
  await assert.rejects(prover.proveRedemption(request), (error) => error.code === "DISPOSED");
  const second = await pending;
  clearInterval(timer);
  assert.equal(second.relation, "confidential-redemption");
  assert.equal(second.nullifiers.length, 1);
  assert.equal(second.outputCommitments.length, 0);
  assert.deepEqual(second.root, root);
  assert.ok(second.proof.length > 0);
  assert.ok(secondTicks > 0, "second proof must yield to the JavaScript event loop");
  console.log(JSON.stringify({
    relations: [first.relation, second.relation],
    proofBytes: [first.proof.length, second.proof.length],
    eventLoopTicks: [firstTicks, secondTicks],
    changeCommitmentMatches: true,
    defaultDiversifierUsed: true,
    acceptedJobSurvivedDisposal: true,
  }));
} finally {
  clearInterval(timer);
  prover?.dispose();
  spendKey.fill(0);
  diversifierSeed.fill(0);
  inputRho.fill(0);
  changeRho.fill(0);
}
// Private JavaScript strings remain runtime-managed; these helpers do not promise
// erasure of caller-retained openings or establish ledger spending authorization.
