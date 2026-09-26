import assert from "node:assert/strict";
import { test } from "node:test";

import { parseSumeragiStatusJson } from "../src/sumeragiTyped.js";
import { browserSumeragiStatusFixture } from "./sumeragiBrowserFixtures.js";

function parseFixture(payload) {
  return parseSumeragiStatusJson(JSON.stringify(payload));
}

function commitmentOf(payload) {
  return payload.last_commit_qc.certificate.execution_commitment;
}

function populatedFixture() {
  const payload = browserSumeragiStatusFixture();
  const commitment = commitmentOf(payload);
  commitment.transaction_input_commitment = {
    root: payload.node_fingerprint,
    leaf_count: 2,
  };
  commitment.transaction_output_commitment = {
    root: payload.build_fingerprint,
    leaf_count: 3,
  };
  return payload;
}

test("V4 status preserves both populated transaction trees and output-only trees", () => {
  const payload = populatedFixture();
  const parsed = parseFixture(payload);
  assert.deepEqual(
    commitmentOf(parsed).transaction_input_commitment,
    commitmentOf(payload).transaction_input_commitment,
  );
  assert.deepEqual(
    commitmentOf(parsed).transaction_output_commitment,
    commitmentOf(payload).transaction_output_commitment,
  );

  const outputOnly = browserSumeragiStatusFixture();
  commitmentOf(outputOnly).transaction_output_commitment =
    commitmentOf(payload).transaction_output_commitment;
  const parsedOutputOnly = parseFixture(outputOnly);
  assert.equal(commitmentOf(parsedOutputOnly).transaction_input_commitment, null);
  assert.deepEqual(
    commitmentOf(parsedOutputOnly).transaction_output_commitment,
    commitmentOf(payload).transaction_output_commitment,
  );
});

test("V4 status rejects missing, malformed, and unpaired transaction trees", () => {
  const mutations = [
    (commitment) => { delete commitment.transaction_input_commitment; },
    (commitment) => { delete commitment.transaction_output_commitment; },
    (commitment) => { commitment.transaction_input_commitment = { leaf_count: 2 }; },
    (commitment) => { commitment.transaction_output_commitment = { root: commitment.transaction_output_commitment.root }; },
    (commitment) => { commitment.transaction_input_commitment.root = "invalid"; },
    (commitment) => { commitment.transaction_input_commitment.leaf_count = 0; },
    (commitment) => { commitment.transaction_input_commitment.leaf_count = -1; },
    (commitment) => { commitment.transaction_input_commitment.leaf_count = "2"; },
    (commitment) => { commitment.transaction_input_commitment.extra = true; },
    (commitment) => { commitment.transaction_output_commitment = null; },
    (commitment) => { commitment.transaction_output_commitment.leaf_count = 1; },
  ];
  for (const mutate of mutations) {
    const payload = populatedFixture();
    mutate(commitmentOf(payload));
    assert.throws(() => parseFixture(payload));
  }
});

test("V4 status preserves full u64 transaction leaf counts and rejects overflow", () => {
  const encoded = JSON.stringify(populatedFixture());
  const maximum = "18446744073709551615";
  const withMaximum = encoded
    .replaceAll('"leaf_count":2', `"leaf_count":${maximum}`)
    .replaceAll('"leaf_count":3', `"leaf_count":${maximum}`);
  const parsed = parseSumeragiStatusJson(withMaximum);
  assert.equal(commitmentOf(parsed).transaction_input_commitment.leaf_count, BigInt(maximum));
  assert.equal(commitmentOf(parsed).transaction_output_commitment.leaf_count, BigInt(maximum));
  assert.throws(() => parseSumeragiStatusJson(
    withMaximum.replaceAll(maximum, "18446744073709551616"),
  ));
});
