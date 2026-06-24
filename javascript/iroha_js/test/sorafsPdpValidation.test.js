import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  SORAFS_PDP_PAYLOAD_KINDS,
  validatePdpBundle,
  validatePdpChallengeProof,
  validatePdpCommitmentChallenge,
  validatePdpPayload,
} from "../src/sorafs.js";

const COMMITMENT_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/pdp/commitment_v1.to",
  import.meta.url,
);
const CHALLENGE_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/pdp/challenge_v1.to",
  import.meta.url,
);
const PROOF_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/pdp/proof_v1.to",
  import.meta.url,
);
const MISSING_SIGNATURE_PROOF_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/pdp/negative/missing_signature_proof_v1.to",
  import.meta.url,
);

function pdpFixtures() {
  return {
    commitment: readFileSync(COMMITMENT_FIXTURE),
    challenge: readFileSync(CHALLENGE_FIXTURE),
    proof: readFileSync(PROOF_FIXTURE),
  };
}

test("validatePdpPayload accepts canonical commitment fixture", () => {
  const { commitment } = pdpFixtures();
  const outcome = validatePdpPayload(
    SORAFS_PDP_PAYLOAD_KINDS.COMMITMENT,
    commitment,
    {
      label: "fixtures/sorafs_manifest/pdp/commitment_v1.to",
      generatedAtUnix: 1_700_001_001,
    },
  );

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.equal(outcome.inputs[0]?.kind, "pdp_commitment");
  assert.equal(
    outcome.inputs[0]?.path,
    "fixtures/sorafs_manifest/pdp/commitment_v1.to",
  );
  assert.equal(outcome.generated_at, 1_700_001_001);
});

test("validatePdpCommitmentChallenge accepts bound fixtures", () => {
  const { commitment, challenge } = pdpFixtures();
  const outcome = validatePdpCommitmentChallenge(commitment, challenge, {
    commitmentLabel: "commitment.to",
    challengeLabel: "challenge.to",
    generated_at: 1_700_001_002,
  });

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.deepEqual(
    outcome.inputs.map((input) => input.kind),
    ["pdp_commitment", "pdp_challenge"],
  );
  assert.equal(outcome.generated_at, 1_700_001_002);
});

test("validatePdpChallengeProof accepts bound fixtures", () => {
  const { challenge, proof } = pdpFixtures();
  const outcome = validatePdpChallengeProof(challenge, proof, {
    challenge_label: "challenge.to",
    proof_label: "proof.to",
    generatedAtUnix: 1_700_001_003,
  });

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.deepEqual(
    outcome.inputs.map((input) => input.kind),
    ["pdp_challenge", "pdp_proof"],
  );
});

test("validatePdpBundle accepts the canonical commitment challenge proof set", () => {
  const { commitment, challenge, proof } = pdpFixtures();
  const outcome = validatePdpBundle(commitment, challenge, proof, {
    commitmentLabel: "commitment.to",
    challengeLabel: "challenge.to",
    proofLabel: "proof.to",
    generatedAtUnix: 1_700_001_004,
  });

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.deepEqual(
    outcome.inputs.map((input) => input.kind),
    ["pdp_commitment", "pdp_challenge", "pdp_proof"],
  );
  assert.equal(outcome.generated_at, 1_700_001_004);
});

test("validatePdpPayload reports malformed payloads as reference outcomes", () => {
  const outcome = validatePdpPayload("proof", Buffer.alloc(8), {
    generatedAtUnix: 1_700_001_005,
  });

  assert.equal(outcome.status, "Error");
  assert.equal(outcome.category, "norito");
  assert.equal(outcome.code, "SFS-NORITO-001");
  assert.equal(outcome.inputs[0]?.kind, "pdp_proof");
});

test("validatePdpChallengeProof returns signature outcomes for invalid proof fixtures", () => {
  const { challenge } = pdpFixtures();
  const outcome = validatePdpChallengeProof(
    challenge,
    readFileSync(MISSING_SIGNATURE_PROOF_FIXTURE),
    { generatedAtUnix: 1_700_001_006 },
  );

  assert.equal(outcome.status, "Error");
  assert.equal(outcome.category, "signature");
  assert.equal(outcome.code, "SFS-SIG-008");
});

test("validatePdpPayload rejects unknown kinds before native validation", () => {
  assert.throws(
    () => validatePdpPayload("bad-kind", Buffer.alloc(8)),
    /unsupported SoraFS PDP payload kind/i,
  );
});
