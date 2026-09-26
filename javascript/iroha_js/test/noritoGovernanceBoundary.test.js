import test from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

import {
  createNoritoGovernanceInstructionBoundary,
  parseStrictGovernanceInstructionJson,
} from "../src/noritoGovernanceBoundary.js";

const isPlainObject = (value) =>
  value !== null &&
  typeof value === "object" &&
  !Array.isArray(value);

const strictBoundary = () => createNoritoGovernanceInstructionBoundary({
  assertExactNonEmptyString: (value) => value,
  assertOnlyObjectKeys: (value, allowed, context) => {
    for (const key of Object.keys(value)) {
      if (!allowed.includes(key)) {
        throw new TypeError(`${context} has unexpected field ${key}`);
      }
    }
  },
  decodeExactStandardBase64: () => Buffer.of(1),
  decodeManifestProvenanceValue: (value) => value,
  encodeManifestProvenanceValue: (value) => value,
  isPlainObject,
});

test("governance boundary factory exposes every Norito call-path helper", () => {
  const boundary = createNoritoGovernanceInstructionBoundary({
    assertExactNonEmptyString: (value) => value,
    assertOnlyObjectKeys: () => {},
    decodeExactStandardBase64: () => Buffer.of(1),
    decodeManifestProvenanceValue: (value) => value,
    encodeManifestProvenanceValue: (value) => value,
    isPlainObject,
  });

  for (const name of [
    "assertCanonicalGovernanceSelectorV1",
    "isStrictGovernanceInstructionCandidate",
    "validateCastZkBallotPayload",
    "validateCastPlainBallotPayload",
    "validateUpdatePlainConvictionPayload",
    "validateGovernanceInstructionBoundary",
    "validateProposeDeployContractPayload",
  ]) {
    assert.equal(typeof boundary[name], "function", name);
  }
  assert.equal(Object.isFrozen(boundary), true);
});

test("choice-free conviction update rejects extra direction and duplicate JSON fields", () => {
  const boundary = strictBoundary();
  const update = {
    UpdatePlainConviction: {
      referendum_id: "ref-2",
      owner: "unused-after-structural-rejection",
      amount: "10",
      duration_blocks: 5,
      direction: 1,
    },
  };
  assert.equal(boundary.isStrictGovernanceInstructionCandidate(update), true);
  assert.throws(
    () => boundary.validateGovernanceInstructionBoundary(update),
    /UpdatePlainConviction has unexpected field direction/u,
  );
  assert.throws(
    () => parseStrictGovernanceInstructionJson(
      '{"UpdatePlainConviction":{"referendum_id":"one","referendum_id":"two"}}',
      "governance instruction",
    ),
    /duplicate object key "referendum_id"/u,
  );
});

test("public cast has one exact choice-bearing instruction shape", () => {
  const boundary = strictBoundary();
  const cast = {
    CastPlainBallot: {
      referendum_id: "ref-2",
      owner: "unused-after-structural-rejection",
      amount: "10",
      duration_blocks: 5,
      direction: 1,
    },
  };
  assert.equal(boundary.isStrictGovernanceInstructionCandidate(cast), true);
  assert.throws(
    () => boundary.validateGovernanceInstructionBoundary({
      CastPlainBallot: { ...cast.CastPlainBallot, update: true },
    }),
    /CastPlainBallot has unexpected field update/u,
  );
  assert.throws(
    () => boundary.validateGovernanceInstructionBoundary({
      CastPlainBallot: { ...cast.CastPlainBallot, direction: 3 },
    }),
    /CastPlainBallot.direction must be exactly 0, 1, or 2/u,
  );
});

test("strict governance JSON retains the numeric ABI version", () => {
  const parsed = parseStrictGovernanceInstructionJson(
    '{"ProposeDeployContract":{"abi_version":1}}',
    "governance instruction",
  );
  assert.equal(parsed.ProposeDeployContract.abi_version, 1);
});

test("strict governance JSON rejects duplicate keys and malformed scalars", () => {
  assert.throws(
    () =>
      parseStrictGovernanceInstructionJson(
        '{"CastZkBallot":{"election_id":"one","election_id":"two"}}',
        "governance instruction",
      ),
    /duplicate object key "election_id"/u,
  );
  assert.throws(
    () =>
      parseStrictGovernanceInstructionJson(
        '{"CastZkBallot":{"election_id":"\\ud800"}}',
        "governance instruction",
      ),
    /unpaired high surrogate/u,
  );
  assert.throws(
    () =>
      parseStrictGovernanceInstructionJson(
        `{"CastZkBallot":"${String.fromCharCode(0xd800)}`,
        "governance instruction",
      ),
    /unpaired high surrogate/u,
  );
  for (const token of ["-0", "1.0", "1e0", "01"]) {
    assert.throws(
      () =>
        parseStrictGovernanceInstructionJson(
          `{"ProposeDeployContract":{"abi_version":${token}}}`,
          "governance instruction",
        ),
      /invalid JSON|canonical integers|Unexpected|Expected/u,
    );
  }
});
