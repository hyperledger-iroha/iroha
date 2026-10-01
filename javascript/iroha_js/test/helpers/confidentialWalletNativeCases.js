// Actual public SDK consumers, reused unchanged by source and installed-package tests.
// Fixed disposable material is never production wallet state; roots are local fixtures.
import assert from "node:assert/strict";

const ASSET = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const CAPACITY = 65_536;
const nonce = (index) => {
  const bytes = Buffer.alloc(32);
  bytes.writeUInt32LE(index + 1);
  return bytes;
};

async function withWallet(sdk, operation) {
  const key = Buffer.alloc(32, 94);
  const seed = Buffer.alloc(32, 97);
  let prover;
  try {
    const address = sdk.deriveConfidentialReceiveAddressV2({ spendKey: key, diversifierSeed: seed });
    prover = new sdk.ConfidentialProver({
      networkId: sdk.NetworkId.fromBytes(Buffer.alloc(32, 93)),
      assetDefinitionId: ASSET, spendKey: key,
    });
    const noteAt = (index) => {
      const rho = nonce(index);
      try {
        return { amount: 7n, rhoHex: rho.toString("hex"), diversifierHex: address.diversifierHex, leafIndex: index };
      } finally { rho.fill(0); }
    };
    const commitmentFor = (note) => sdk.deriveConfidentialNoteV2({
      assetDefinitionId: ASSET, amount: note.amount, rhoHex: note.rhoHex, ownerTagHex: address.ownerTagHex,
    }).commitment;
    return await operation(prover, noteAt, commitmentFor);
  } finally {
    prover?.dispose();
    key.fill(0);
    seed.fill(0);
  }
}

function assertFullProof(proof, root) {
  assert.equal(proof.relation, "confidential-redemption");
  assert.ok(proof.proof.length > 0); // Native Core proves and verifies before returning.
  assert.deepEqual(proof.root, root);
  assert.equal(proof.nullifiers.length, 1);
  assert.equal(proof.nullifiers[0].length, 32);
  assert.ok(proof.nullifiers[0].some((byte) => byte !== 0));
  assert.deepEqual(proof.outputCommitments, []);
}

/** No injected runtime, fake result, omitted input, skip or network submission. */
export function confidentialWalletNativeCases(sdk) {
  return [
    ["one actual input at index 65535 of a genuine full tree proves and locally verifies", async () =>
      withWallet(sdk, async (prover, noteAt, commitmentFor) => {
        const commitments = Array.from({ length: CAPACITY }, (_, index) => commitmentFor(noteAt(index)));
        const note = noteAt(CAPACITY - 1);
        assert.equal(commitments.length, CAPACITY);
        assert.equal(new Set(commitments.map((word) => word.toString("hex"))).size, CAPACITY);
        const root = await sdk.computeConfidentialRoot({ commitments });
        const proof = await prover.proveRedemption({
          treeCommitments: commitments, rootHex: root.toString("hex"), inputs: [note], publicAmount: 7n,
        });
        assertFullProof(proof, root);
        return { treeLeaves: CAPACITY, leafIndex: note.leafIndex, actualInputs: 1, proofBytes: proof.proof.length };
      })],
    ["genuine wallet rejects wrong root, duplicate inputs and invalid conservation then recovers", async () =>
      withWallet(sdk, async (prover, noteAt, commitmentFor) => {
        const note = noteAt(0);
        const commitments = [commitmentFor(note)];
        const root = await sdk.computeConfidentialRoot({ commitments });
        const wrongRoot = await sdk.computeConfidentialRoot({ commitments: [commitmentFor(noteAt(1))] });
        assert.notDeepEqual(root, wrongRoot);
        const request = { treeCommitments: commitments, rootHex: root.toString("hex"), inputs: [note], publicAmount: 7n };
        await assert.rejects(prover.proveRedemption({ ...request, rootHex: wrongRoot.toString("hex") }),
          (error) => error instanceof sdk.ConfidentialProverError && error.code === "PROVING_FAILED");
        // The public SDK rejects duplicate indices and conservation before native dispatch.
        // This checks the actual installed API with real openings, not an injected backend.
        for (const patch of [{ inputs: [note, note], publicAmount: 14n },
          { publicAmount: 8n }, { publicAmount: 4n, change: { amount: 2n, rhoHex: noteAt(2).rhoHex } }]) {
          await assert.rejects(prover.proveRedemption({ ...request, ...patch }),
            (error) => error instanceof sdk.ConfidentialProverError && error.code === "INVALID_INPUT");
        }
        const proof = await prover.proveRedemption(request);
        assertFullProof(proof, root);
        prover.dispose();
        await assert.rejects(prover.proveRedemption(request),
          (error) => error instanceof sdk.ConfidentialProverError && error.code === "DISPOSED");
        return { wrongRootRejected: true, duplicateRejected: true, conservationRejected: true,
          recoveredProofBytes: proof.proof.length, disposedRejected: true };
      })],
  ];
}
// JavaScript private strings remain runtime-managed; these tests make no erasure claim for them.
