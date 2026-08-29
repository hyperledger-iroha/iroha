import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import {
  SCCP_REPLAY_BOUNDARIES_V1,
  SCCP_REPLAY_SMT_DEPTH_V1,
  sccpReplayDomainHashV1,
  sccpReplayEmptyHashesV1,
  sccpReplayKeyV1,
  sccpReplayRecordDigestV1,
  sccpReplayRootFromWitnessV1,
} from "../src/sccp.js";

const repeat = (byte, length) => `0x${byte.repeat(length)}`;
const ZERO = repeat("00", 32);
const GOLDEN = JSON.parse(
  fs.readFileSync(new URL("../../../fixtures/sccp/replay_forest_v1.json", import.meta.url), "utf8"),
);

const DOMAIN = {
  sourceProfile: "sora-taira",
  targetProfile: "ethereum-mainnet",
  boundary: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
  routeRevision: 7,
  routeConfigurationHash: repeat("44", 32),
  actor: { kind: "route" },
};

const RECORD = {
  operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
  replayId: repeat("11", 32),
  payloadSha256: repeat("22", 32),
  amount: "9",
  principal: { kind: "evm", address: repeat("33", 20) },
  auxiliaryIdentitySha256: repeat("55", 32),
};

test("final-V1 replay forest hashes match the cross-language golden", () => {
  const domainHash = sccpReplayDomainHashV1(DOMAIN);
  assert.equal(domainHash, `0x${GOLDEN.expected.domain_hash_hex}`);
  const key = sccpReplayKeyV1(domainHash, RECORD.replayId);
  assert.equal(key, `0x${GOLDEN.expected.replay_key_hex}`);
  const recordDigest = sccpReplayRecordDigestV1(RECORD);
  assert.equal(recordDigest, `0x${GOLDEN.expected.record_digest_hex}`);

  const empty = sccpReplayEmptyHashesV1();
  assert.equal(empty.length, SCCP_REPLAY_SMT_DEPTH_V1 + 1);
  assert.equal(empty[0], `0x${GOLDEN.expected.empty_leaf_hash_hex}`);
  assert.equal(empty.at(-1), `0x${GOLDEN.expected.empty_shard_root_hex}`);

  const emptyWitness = {
    expectedShardRoot: empty.at(-1),
    priorRecordDigest: ZERO,
    siblingBitmap: ZERO,
    siblings: [],
  };
  const nonMembership = sccpReplayRootFromWitnessV1(key, null, emptyWitness);
  assert.equal(nonMembership.root, empty.at(-1));
  assert.equal(nonMembership.matchesExpectedRoot, true);
  assert.equal(nonMembership.shard, GOLDEN.expected.shard);

  const occupied = sccpReplayRootFromWitnessV1(key, recordDigest, {
    ...emptyWitness,
    expectedShardRoot: `0x${GOLDEN.expected.occupied_shard_root_hex}`,
    priorRecordDigest: recordDigest,
  });
  assert.equal(occupied.root, occupied.expectedRoot);
  assert.equal(occupied.matchesExpectedRoot, true);
});

test("replay witnesses reject reserved bits, explicit defaults, and count drift", () => {
  const key = sccpReplayKeyV1(sccpReplayDomainHashV1(DOMAIN), RECORD.replayId);
  const empty = sccpReplayEmptyHashesV1();
  const base = {
    expectedShardRoot: empty.at(-1),
    priorRecordDigest: ZERO,
    siblingBitmap: ZERO,
    siblings: [],
  };
  assert.throws(
    () => sccpReplayRootFromWitnessV1(key, null, { ...base, siblingBitmap: `0x01${"00".repeat(31)}` }),
    /reserved high bits/u,
  );
  assert.throws(
    () => sccpReplayRootFromWitnessV1(key, null, { ...base, siblingBitmap: `0x${"00".repeat(31)}01` }),
    /count does not match/u,
  );
  assert.throws(
    () =>
      sccpReplayRootFromWitnessV1(key, null, {
        ...base,
        siblingBitmap: `0x${"00".repeat(31)}01`,
        siblings: [empty[0]],
      }),
    /explicitly encodes a default/u,
  );
});

test("replay domains reject testnets, wrong actors, and amount overflow", () => {
  assert.throws(
    () => sccpReplayDomainHashV1({ ...DOMAIN, targetProfile: "ethereum-sepolia" }),
    /final-V1 production network/u,
  );
  assert.throws(
    () => sccpReplayDomainHashV1({ ...DOMAIN, actor: { kind: "evm", address: repeat("33", 20) } }),
    /invalid boundary, direction, or actor/u,
  );
  assert.throws(
    () => sccpReplayRecordDigestV1({ ...RECORD, amount: (1n << 128n).toString() }),
    /exceeds u128/u,
  );
});
