import assert from "node:assert/strict";
import test from "node:test";

import { parseElectionTallyResponseV1 } from "../src/electionTallyV1.js";
import { parseStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";

const MAX_U64 = (1n << 64n) - 1n;
const MAX_U128 = (1n << 128n) - 1n;
const HASH = "12".repeat(32);
const parse = (raw) => parseElectionTallyResponseV1(
  parseStrictLosslessIntegerJson(raw, "election tally fixture"),
);
const response = (tally = [0, 0]) => ({
  evaluated_block_height: 7,
  evaluated_block_hash: HASH,
  finalized: true,
  tally,
});

test("election tally preserves unquoted u64/u128 weights and block height exactly", () => {
  const tally = parse(`{"evaluated_block_height":${MAX_U64},"evaluated_block_hash":"${HASH}","finalized":true,"tally":[9007199254740993,18446744073709551616]}`);
  assert.equal(tally.evaluated_block_height, MAX_U64);
  assert.deepEqual(tally.tally, [9007199254740993n, 18446744073709551616n]);
  assert.equal(tally.finalized, true);
  assert.equal(tally.evaluated_block_hash, HASH);
});

test("election tally enforces V1 shape and checked u128 aggregate", () => {
  assert.deepEqual(parseElectionTallyResponseV1(response([MAX_U128, 0])).tally, [MAX_U128, 0]);
  assert.equal(parseElectionTallyResponseV1(response(Array(64).fill(0))).tally.length, 64);
  for (const tally of [[], [0], Array(65).fill(0)]) {
    assert.throws(() => parseElectionTallyResponseV1(response(tally)), /2–64/u);
  }
  for (const tally of [[MAX_U128, 1], [MAX_U128 + 1n, 0], [-1, 0], ["1", 0],
    [Number.MAX_SAFE_INTEGER + 1, 0], [0.5, 0], Array(2)]) {
    assert.throws(() => parseElectionTallyResponseV1(response(tally)), /election tally/u);
  }
});

test("election tally rejects malformed coordinates and extra or missing fields", () => {
  const valid = response();
  for (const bad of [
    { ...valid, evaluated_block_height: MAX_U64 + 1n },
    { ...valid, evaluated_block_height: 0 },
    { ...valid, evaluated_block_hash: "0".repeat(64) },
    { ...valid, evaluated_block_hash: "ab".repeat(32).toUpperCase() },
    { ...valid, finalized: 1 },
    { ...valid, unexpected: true },
    { evaluated_block_hash: HASH, finalized: true, tally: [0, 0] },
  ]) {
    assert.throws(() => parseElectionTallyResponseV1(bad), /election tally/u);
  }
  assert.equal(parseElectionTallyResponseV1({
    ...valid, evaluated_block_height: 0, evaluated_block_hash: "0".repeat(64),
  }).evaluated_block_height, 0);
});

test("election tally rejects duplicate keys and noncanonical number tokens before projection", () => {
  for (const raw of [
    `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[0,0],"tally":[1,0]}`,
    `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[1e0,0]}`,
    `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[01,0]}`,
  ]) {
    assert.throws(() => parse(raw), /duplicate|canonical integers|invalid/u);
  }
});
