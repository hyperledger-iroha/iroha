import assert from "node:assert/strict";
import test from "node:test";

const SOURCE_MODULE = new URL("../src/instructionBuilders.js", import.meta.url);
const DIST_MODULE = new URL("../dist/instructionBuilders.js", import.meta.url);
const BROWSER_MODULE = new URL("../dist/browser.js", import.meta.url);

function attachmentInput() {
  return {
    backend: "lane/privacy",
    proof: new Uint8Array([1, 2, 3]),
    verifyingKeyRef: {
      backend: "lane/privacy",
      name: "lane/privacy::membership_v1",
    },
    lanePrivacy: {
      commitmentId: 0,
      merkle: {
        leaf: new Uint8Array(32).fill(1),
        leafIndex: 0,
        auditPath: [new Uint8Array(32).fill(2)],
      },
    },
  };
}

for (const [surface, moduleUrl] of [
  ["source", SOURCE_MODULE],
  ["distribution", DIST_MODULE],
  ["browser", BROWSER_MODULE],
]) {
  test(`${surface} ProofAttachment builder enforces canonical lane semantics`, async () => {
    const module = await import(moduleUrl);
    assert.equal(typeof module.buildFinalizeElectionInstruction, "function");
    const input = attachmentInput();
    const originalSiblingTail = input.lanePrivacy.merkle.auditPath[0][31];
    const instruction = module.buildFinalizeElectionInstruction({
      electionId: "privacy-parity",
      tally: [1],
      proof: input,
    });
    const proof = instruction.zk.FinalizeElection.tally_proof;
    assert.equal(proof.vk_ref.name, "lane/privacy::membership_v1");
    assert.equal(
      proof.lane_privacy.witness.payload.proof.audit_path[0][31] & 1,
      1,
    );
    assert.equal(
      input.lanePrivacy.merkle.auditPath[0][31],
      originalSiblingTail,
      "builder must not mutate caller-owned bytes",
    );

    assert.throws(
      () =>
        module.buildFinalizeElectionInstruction({
          electionId: "privacy-parity",
          tally: [1],
          proof: {
            ...attachmentInput(),
            verifyingKeyRef: "lane/privacy:legacy",
          },
        }),
      /must be a plain object/,
    );
    assert.throws(
      () =>
        module.buildFinalizeElectionInstruction({
          electionId: "privacy-parity",
          tally: [1],
          proof: {
            ...attachmentInput(),
            lanePrivacy: {
              commitmentId: 0,
              merkle: {
                ...attachmentInput().lanePrivacy.merkle,
                leafIndex: 2,
              },
            },
          },
        }),
      /impossible for the Merkle path depth/,
    );
  });
}
