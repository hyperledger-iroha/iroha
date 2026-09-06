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
  sccpReplayVerifyAgainstCurrentRootV1,
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
const FROZEN_HYBRID_FIXTURE = JSON.parse(
  fs.readFileSync(new URL("../../../fixtures/sccp/replay_forest_v1.json", import.meta.url), "utf8"),
);

const DOMAIN = {
  sourceProfile: "sora-taira",
  targetProfile: "ethereum-mainnet",
  boundary: SCCP_REPLAY_BOUNDARIES_V1.evm_destination_mint,
  routeRevision: 7,
  routeConfigurationHash: repeat("44", 32),
  actor: { kind: "evm", address: repeat("33", 20) },
};

const RECORD = {
  operation: SCCP_REPLAY_BOUNDARIES_V1.evm_destination_mint,
  replayId: repeat("11", 32),
  payloadSha256: repeat("22", 32),
  amount: "9",
  principal: { kind: "evm", address: repeat("33", 20) },
  auxiliaryIdentitySha256: repeat("55", 32),
};

test("final-V1 replay forest hashes match one self-contained valid vector", () => {
  const domainHash = sccpReplayDomainHashV1(DOMAIN);
  assert.equal(domainHash, "0xebc495541ef2265beebe7ee9e4e8764595c2a55ed67dc6d0a8ff69ccd3ff3228");
  const key = sccpReplayKeyV1(domainHash, RECORD.replayId);
  assert.equal(key, "0x035bcebe9423edd4f1b945bae54905e0f0860bcc54718d372b1a58797ce614d4");
  const recordDigest = sccpReplayRecordDigestV1(RECORD);
  assert.equal(recordDigest, "0xbb0a7e99f5d2d136375e46ba231903611366ea85ec0e10130488a085fa05bf4f");

  const empty = sccpReplayEmptyHashesV1();
  assert.equal(empty.length, SCCP_REPLAY_SMT_DEPTH_V1 + 1);
  assert.equal(empty[0], "0x6841d062186b649a505eb694ebce936fe978c5530596882a70c6e04303c88d43");
  assert.equal(empty.at(-1), "0xcefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f");

  const emptyWitness = {
    expectedShardRoot: empty.at(-1),
    priorRecordDigest: ZERO,
    siblingBitmap: ZERO,
    siblings: [],
  };
  const nonMembership = sccpReplayRootFromWitnessV1(key, ZERO, emptyWitness);
  assert.equal(nonMembership.root, empty.at(-1));
  assert.equal(nonMembership.matchesExpectedRoot, true);
  assert.equal(nonMembership.shard, 3);

  const occupied = sccpReplayRootFromWitnessV1(key, recordDigest, {
    ...emptyWitness,
    expectedShardRoot: "0xec10fe878a6429557c7af279b8cb6fa5cc51165f4e6a54fb27ed6ad8525caf91",
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
    () => sccpReplayRootFromWitnessV1(key, ZERO, { ...base, siblingBitmap: `0x01${"00".repeat(31)}` }),
    /reserved high bits/u,
  );
  assert.throws(
    () => sccpReplayRootFromWitnessV1(key, ZERO, { ...base, siblingBitmap: `0x${"00".repeat(31)}01` }),
    /count does not match/u,
  );
  assert.throws(
    () =>
      sccpReplayRootFromWitnessV1(key, ZERO, {
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
    () => sccpReplayDomainHashV1({ ...DOMAIN, actor: { kind: "route" } }),
    /invalid boundary, direction, or actor/u,
  );
  assert.throws(
    () => sccpReplayRecordDigestV1({ ...RECORD, amount: (1n << 128n).toString() }),
    /exceeds u128/u,
  );
});

test("replay operations bind principal kinds and canonical AccountId bytes", () => {
  const account =
    "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
  const canonicalBytes = encodeAccountIdNoritoValue(account);
  assert.doesNotThrow(() => sccpReplayRecordDigestV1({
    ...RECORD,
    operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
    principal: { kind: "sora_account", canonicalBytes },
  }));
  assert.throws(
    () => sccpReplayRecordDigestV1({
      ...RECORD,
      principal: { kind: "sora_account", canonicalBytes },
    }),
    /principal kind are inconsistent/u,
  );
  assert.throws(
    () => sccpReplayRecordDigestV1({
      ...RECORD,
      operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
      principal: {
        kind: "sora_account",
        canonicalBytes: Uint8Array.from([...canonicalBytes, 0]),
      },
    }),
    /canonical AccountId/u,
  );
  const overlongLength = Uint8Array.from([
    ...canonicalBytes.slice(0, 4),
    canonicalBytes[4] | 0x80,
    0,
    ...canonicalBytes.slice(5),
  ]);
  assert.throws(
    () => sccpReplayRecordDigestV1({
      ...RECORD,
      operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
      principal: { kind: "sora_account", canonicalBytes: overlongLength },
    }),
    /canonical AccountId/u,
  );
});

test("the frozen hybrid replay fixture is rejected at its operation/principal boundary", () => {
  assert.equal(
    FROZEN_HYBRID_FIXTURE.domain.operation_tag,
    SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
  );
  assert.equal(FROZEN_HYBRID_FIXTURE.record.principal_kind, "evm");
  assert.throws(
    () => sccpReplayRecordDigestV1({
      operation: FROZEN_HYBRID_FIXTURE.domain.operation_tag,
      replayId: `0x${FROZEN_HYBRID_FIXTURE.record.replay_id_hex}`,
      payloadSha256: `0x${FROZEN_HYBRID_FIXTURE.record.payload_sha256_hex}`,
      amount: FROZEN_HYBRID_FIXTURE.record.amount_scale9,
      principal: {
        kind: "evm",
        address: `0x${FROZEN_HYBRID_FIXTURE.record.principal_bytes_hex}`,
      },
      auxiliaryIdentitySha256:
        `0x${FROZEN_HYBRID_FIXTURE.record.auxiliary_identity_sha256_hex}`,
    }),
    /principal kind are inconsistent/u,
  );
});

test("replay root verification accepts zero keys and only rejects level defaults", () => {
  const empty = sccpReplayEmptyHashesV1();
  const emptyWitness = {
    expectedShardRoot: empty.at(-1),
    priorRecordDigest: ZERO,
    siblingBitmap: ZERO,
    siblings: [],
  };
  const zeroKey = ZERO;
  const zeroExpectedWitness = { ...emptyWitness, expectedShardRoot: ZERO };
  assert.equal(
    sccpReplayRootFromWitnessV1(zeroKey, ZERO, zeroExpectedWitness).matchesExpectedRoot,
    false,
  );
  assert.throws(
    () => sccpReplayVerifyAgainstCurrentRootV1(zeroKey, ZERO, zeroExpectedWitness, ZERO),
    /does not match the current shard root/u,
  );
  assert.equal(
    sccpReplayVerifyAgainstCurrentRootV1(zeroKey, ZERO, emptyWitness, empty.at(-1))
      .matchesExpectedRoot,
    true,
  );

  const zeroSiblingWitness = {
    ...emptyWitness,
    siblingBitmap: `0x${"00".repeat(31)}01`,
    siblings: [ZERO],
  };
  const reconstructed = sccpReplayRootFromWitnessV1(zeroKey, ZERO, zeroSiblingWitness);
  const boundWitness = { ...zeroSiblingWitness, expectedShardRoot: reconstructed.root };
  assert.equal(
    sccpReplayVerifyAgainstCurrentRootV1(
      zeroKey,
      ZERO,
      boundWitness,
      reconstructed.root,
    ).matchesExpectedRoot,
    true,
  );
  assert.throws(
    () => sccpReplayVerifyAgainstCurrentRootV1(
      zeroKey,
      ZERO,
      boundWitness,
      repeat("77", 32),
    ),
    /current shard root/u,
  );
});

test("final-V1 TON boundaries have exact names, tags, and directions", () => {
  const B = SCCP_REPLAY_BOUNDARIES_V1;
  assert.deepEqual(
    [B.ton_wallet_burn_authorization, B.ton_wallet_burn_lock, B.ton_wallet_burn_refund],
    [0x35, 0x36, 0x37],
  );
  const tonActor = { kind: "ton", workchain: 0, account: repeat("66", 32) };
  for (const boundary of [
    B.ton_bridge_inbound_mint,
    B.ton_master_mint,
    B.ton_wallet_mint_credit,
  ]) {
    assert.doesNotThrow(() => sccpReplayDomainHashV1({
      ...DOMAIN,
      targetProfile: "ton-mainnet",
      boundary,
      actor: tonActor,
    }));
  }
  for (const boundary of [
    B.ton_bridge_outbound_burn,
    B.ton_master_burn,
    B.ton_wallet_burn_authorization,
    B.ton_wallet_burn_lock,
    B.ton_wallet_burn_refund,
  ]) {
    const outbound = {
      ...DOMAIN,
      sourceProfile: "ton-mainnet",
      targetProfile: "sora-taira",
      boundary,
      actor: tonActor,
    };
    assert.doesNotThrow(() => sccpReplayDomainHashV1(outbound));
    assert.throws(
      () => sccpReplayDomainHashV1({
        ...outbound,
        sourceProfile: "sora-taira",
        targetProfile: "ton-mainnet",
      }),
      /invalid boundary, direction, or actor/u,
    );
  }
});

test("SORA replay principals require exact compact AccountId bytes", () => {
  const valid = {
    ...RECORD,
    operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
    principal: { kind: "sora_account", canonicalBytes: SORA_ACCOUNT },
  };
  assert.match(sccpReplayRecordDigestV1(valid), /^0x[0-9a-f]{64}$/u);
  assert.match(
    sccpReplayRecordDigestV1({
      ...RECORD,
      operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
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
      operation: SCCP_REPLAY_BOUNDARIES_V1.sora_outbound_lock,
          principal: { kind: "sora_account", canonicalBytes },
        }),
      /canonical AccountId/u,
    );
  }
});
