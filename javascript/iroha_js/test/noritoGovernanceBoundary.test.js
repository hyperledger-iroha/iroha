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
    "validateGovernanceInstructionBoundary",
    "validateProposeDeployContractPayload",
  ]) {
    assert.equal(typeof boundary[name], "function", name);
  }
  assert.equal(Object.isFrozen(boundary), true);
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
