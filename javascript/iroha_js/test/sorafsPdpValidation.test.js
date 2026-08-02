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
const BUNDLE_OUTCOME_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/pdp/bundle_validation_outcome_v1.json",
  import.meta.url,
);

function pdpNegativeFixture(name, suffix = "v1.to") {
  return new URL(
    `../../../fixtures/sorafs_manifest/pdp/negative/${name}_${suffix}`,
    import.meta.url,
  );
}

function pdpNegativeOutcome(name) {
  return new URL(
    `../../../fixtures/sorafs_manifest/pdp/negative/${name}_validation_outcome_v1.json`,
    import.meta.url,
  );
}

function canonicalOutcomeJson(outcome) {
  return `${JSON.stringify(outcome, null, 2)}\n`;
}

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
  assert.equal(outcome.code, "SFS-PDP-DIAG-000");
  assert.equal(
    outcome.context.find((field) => field.key === "production_acceptance")
      ?.value,
    "false",
  );
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
    generatedAtUnix: 1_700_001_002,
  });

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-PDP-DIAG-000");
  assert.deepEqual(
    outcome.inputs.map((input) => input.kind),
    ["pdp_commitment", "pdp_challenge"],
  );
  assert.equal(outcome.generated_at, 1_700_001_002);
});

test("validatePdpChallengeProof accepts bound fixtures", () => {
  const { challenge, proof } = pdpFixtures();
  const outcome = validatePdpChallengeProof(challenge, proof, {
    challengeLabel: "challenge.to",
    proofLabel: "proof.to",
    generatedAtUnix: 1_700_001_003,
  });

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-PDP-DIAG-000");
  assert.deepEqual(
    outcome.inputs.map((input) => input.kind),
    ["pdp_challenge", "pdp_proof"],
  );
});

test("PDP validators reject noncanonical option aliases before native dispatch", () => {
  const bytes = Buffer.alloc(1);
  const cases = [
    () => validatePdpPayload("commitment", bytes, { generated_at: 1 }),
    () =>
      validatePdpCommitmentChallenge(bytes, bytes, {
        commitment_label: "commitment.to",
      }),
    () =>
      validatePdpChallengeProof(bytes, bytes, {
        challenge_label: "challenge.to",
      }),
    () => validatePdpBundle(bytes, bytes, bytes, { proof_label: "proof.to" }),
  ];
  for (const invoke of cases) {
    assert.throws(invoke, /unsupported fields/i);
  }
});

test("validatePdpBundle accepts the canonical commitment challenge proof set", () => {
  const { commitment, challenge, proof } = pdpFixtures();
  const outcome = validatePdpBundle(commitment, challenge, proof, {
    commitmentLabel: "commitment_v1.to",
    challengeLabel: "challenge_v1.to",
    proofLabel: "proof_v1.to",
    generatedAtUnix: 123,
  });

  assert.equal(
    canonicalOutcomeJson(outcome),
    readFileSync(BUNDLE_OUTCOME_FIXTURE, "utf8"),
  );
});

test("PDP wrappers match every committed negative ValidationOutcomeV1", () => {
  const { commitment, challenge } = pdpFixtures();
  const cases = [
    {
      name: "duplicate_hot_leaf_challenge",
      validate: (payload) =>
        validatePdpPayload("challenge", payload, {
          label: "duplicate_hot_leaf_challenge_v1.to",
          generatedAtUnix: 123,
        }),
    },
    {
      name: "missing_signature_proof",
      validate: (payload) =>
        validatePdpPayload("proof", payload, {
          label: "missing_signature_proof_v1.to",
          generatedAtUnix: 123,
        }),
    },
    ...["late_proof", "wrong_manifest_proof", "wrong_provider_proof"].map(
      (name) => ({
        name,
        validate: (payload) =>
          validatePdpChallengeProof(challenge, payload, {
            challengeLabel: "challenge_v1.to",
            proofLabel: `${name}_v1.to`,
            generatedAtUnix: 123,
          }),
      }),
    ),
    ...[
      "missing_hot_leaf_path_proof",
      "missing_segment_path_proof",
      "wrong_path_proof",
    ].map((name) => ({
      name,
      validate: (payload) =>
        validatePdpBundle(commitment, challenge, payload, {
          commitmentLabel: "commitment_v1.to",
          challengeLabel: "challenge_v1.to",
          proofLabel: `${name}_v1.to`,
          generatedAtUnix: 123,
        }),
    })),
  ];

  for (const { name, validate } of cases) {
    const outcome = validate(readFileSync(pdpNegativeFixture(name)));
    assert.equal(
      canonicalOutcomeJson(outcome),
      readFileSync(pdpNegativeOutcome(name), "utf8"),
      name,
    );
  }
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

test("validatePdpPayload rejects unknown and retired kind aliases", () => {
  for (const kind of [
    "bad-kind",
    "pdp-proof",
    "pdp_proof",
    "PROOF",
    "Proof",
    " proof ",
  ]) {
    assert.throws(
      () => validatePdpPayload(kind, Buffer.alloc(8)),
      /unsupported SoraFS PDP payload kind/i,
      kind,
    );
  }
});
