import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import { encodeAccountIdNoritoValue } from "../src/norito.js";
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
const SORA_PUBLIC_KEY = Buffer.from(
  "68F4B6017D0F876A55C80A82B8388A54AAD264D367269E2DE8BE079C935B5F96",
  "hex",
);
const SORA_ACCOUNT = encodeAccountIdNoritoValue(
  AccountAddress.fromAccount({
    publicKey: SORA_PUBLIC_KEY,
  }).toI105(),
);
const SORA_MULTISIG_ACCOUNT = encodeAccountIdNoritoValue(
  new AccountAddress(
    { version: 0, classId: 1, normVersion: 1, extFlag: false },
    {
      tag: 1,
      version: 1,
      threshold: 2,
      members: [
        { curve: 1, weight: 1, publicKey: SORA_PUBLIC_KEY },
        {
          curve: 1,
          weight: 1,
          publicKey: Buffer.from(
            "7EA0E3BD52E207C9D3B0EBA65C0704E66FCA2D8E165A175218B174FC4160E413",
            "hex",
          ),
        },
      ],
    },
  ).toI105(),
);
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

test("SORA replay principals require exact compact AccountId bytes", () => {
  const valid = {
    ...RECORD,
    principal: { kind: "sora_account", canonicalBytes: SORA_ACCOUNT },
  };
  assert.match(sccpReplayRecordDigestV1(valid), /^0x[0-9a-f]{64}$/u);
  assert.match(
    sccpReplayRecordDigestV1({
      ...RECORD,
      principal: { kind: "sora_account", canonicalBytes: SORA_MULTISIG_ACCOUNT },
    }),
    /^0x[0-9a-f]{64}$/u,
  );

  const malformed = [
    Uint8Array.of(0),
    SORA_ACCOUNT.slice(0, -1),
    Uint8Array.from([...SORA_ACCOUNT, 0]),
    Uint8Array.from([
      ...SORA_ACCOUNT.slice(0, 4),
      SORA_ACCOUNT[4] | 0x80,
      0,
      ...SORA_ACCOUNT.slice(5),
    ]),
  ];
  for (const canonicalBytes of malformed) {
    assert.throws(
      () =>
        sccpReplayRecordDigestV1({
          ...RECORD,
          principal: { kind: "sora_account", canonicalBytes },
        }),
      /exact canonical SORA AccountId/u,
    );
  }
});
