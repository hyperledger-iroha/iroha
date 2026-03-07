import { test as baseTest } from "node:test";
import assert from "node:assert/strict";
import {
  buildRegisterDomainTransaction,
  buildTransaction,
  buildMintAssetTransaction,
  buildMintAndTransferTransaction,
  buildRegisterDomainAndMintTransaction,
  buildRegisterAssetDefinitionMintAndTransferTransaction,
  buildTransferAssetTransaction,
  buildCreateKaigiTransaction,
  buildJoinKaigiTransaction,
  buildRegisterKaigiRelayTransaction,
  buildRegisterSmartContractCodeTransaction,
  buildRegisterSmartContractBytesTransaction,
  buildDeactivateContractInstanceTransaction,
  buildActivateContractInstanceTransaction,
  buildRemoveSmartContractBytesTransaction,
  buildProposeDeployContractTransaction,
  buildCastZkBallotTransaction,
  buildCastPlainBallotTransaction,
  buildEnactReferendumTransaction,
  buildFinalizeReferendumTransaction,
  buildPersistCouncilForEpochTransaction,
  buildRegisterZkAssetTransaction,
  buildScheduleConfidentialPolicyTransitionTransaction,
  buildCancelConfidentialPolicyTransitionTransaction,
  buildShieldTransaction,
  buildZkTransferTransaction,
  buildUnshieldTransaction,
  buildCreateElectionTransaction,
  buildSubmitBallotTransaction,
  buildFinalizeElectionTransaction,
  hashSignedTransaction,
} from "../src/transaction.js";
import {
  buildMintAssetInstruction,
  buildRegisterDomainInstruction,
} from "../src/instructionBuilders.js";
import { AccountAddress } from "../src/address.js";
import { makeNativeTest } from "./helpers/native.js";

const AUTHORITY_ID_RAW =
  "6cmzPVPX8e5qQsHdB57DhqFT9wp2MiMoXsvt9LYUtypj1nx96bF5s8W";
const AUTHORITY_ID = ih58FromEd25519AccountId(AUTHORITY_ID_RAW);
const AUTHORITY_ID_INPUT = ih58FromEd25519AccountId(AUTHORITY_ID_RAW);
const PRIVATE_KEY = Buffer.alloc(32, 0x11);
const RELAY_ACCOUNT_ID_RAW =
  "6cmzPVPX4Vnjpp7MFrUdgoZ9scoVXwFPcp4U6r6yELFetMDx2taw8et";
const RELAY_ACCOUNT_ID = ih58FromEd25519AccountId(RELAY_ACCOUNT_ID_RAW);
const RELAY_ACCOUNT_ID_INPUT = ih58FromEd25519AccountId(RELAY_ACCOUNT_ID_RAW);
const ASSET_ID = `rose##${AUTHORITY_ID}`;
const ASSET_ID_INPUT = `rose##${AUTHORITY_ID_INPUT}`;
const CANONICAL_ASSET_ID_INPUT =
  "norito:4e52543000000eaf5ef05db6ed320eaf5ef05db6ed3200c4000000000000006165e1e191d7b79c00810000000000000017000000000000000f00000000000000070000000000000064656661756c745a00000000000000000000004e00000000000000460000000000000065643031323045444636443742353243373033324430334145433639364632303638424435333130313532384633433742363038314246463035413136363244374643323435330000000000000017000000000000000f00000000000000070000000000000064656661756c740c000000000000000400000000000000726f7365";
const test = makeNativeTest(baseTest);

function ih58FromEd25519AccountId(raw) {
  const trimmed = raw.trim();
  const atIndex = trimmed.lastIndexOf("@");
  if (atIndex === -1) {
    const { address } = AccountAddress.parseEncoded(trimmed);
    return address.toIH58();
  }
  const signatory = trimmed.slice(0, atIndex).trim().toUpperCase();
  const domain = trimmed.slice(atIndex + 1).trim();
  if (!signatory.startsWith("ED0120")) {
    throw new Error("expected ed25519 multihash signatory");
  }
  const publicKeyHex = signatory.slice(6);
  const publicKey = Buffer.from(publicKeyHex, "hex");
  return AccountAddress.fromAccount({ domain, publicKey }).toIH58();
}

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return crc & 0xffff;
}

function normalizedHashHex(bytes) {
  const buffer = Buffer.from(bytes);
  if (buffer.length !== 32) {
    throw new TypeError("hash literal test helper requires 32 bytes");
  }
  buffer[buffer.length - 1] |= 1;
  const body = buffer.toString("hex").toUpperCase();
  const checksum = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
  return `hash:${body}#${checksum}`;
}

function toByteArray(bytes) {
  return Array.from(Buffer.from(bytes));
}

function buildSampleRegisterDomain(additionalOptions = {}) {
  return buildRegisterDomainTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    domainId: "garden_of_live_flowers",
    metadata: { key: "value" },
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 5_000,
    nonce: 42,
    privateKey: PRIVATE_KEY,
    ...additionalOptions,
  });
}

test("buildRegisterDomainTransaction returns canonical hash", () => {
  const built = buildSampleRegisterDomain();
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  assert.ok(Buffer.isBuffer(built.hash));
  assert.equal(built.hash.length, 32);

  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildRegisterDomainTransaction accepts metadata JSON strings", () => {
  const built = buildSampleRegisterDomain({
    metadata: JSON.stringify({ foo: "bar" }),
  });
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransaction normalizes instruction objects", () => {
  const instruction = buildMintAssetInstruction({ assetId: ASSET_ID_INPUT, quantity: "2" });
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x01, 0x02]),
    hash: Buffer.alloc(32, 0xaa),
  };

  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const built = buildTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        instructions: [instruction],
        metadata: { tag: "value" },
        creationTimeMs: 10,
        ttlMs: 20,
        nonce: 5,
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(built.signedTransaction, Buffer.from(fakeResult.signed_transaction));
      assert.deepEqual(built.hash, Buffer.from(fakeResult.hash));
    },
  );

  assert.equal(captures.length, 1);
  const call = captures[0];
  assert.equal(call.chainId, "test-chain");
  assert.equal(call.authority, AUTHORITY_ID);
  assert.deepEqual(call.instructions, [JSON.stringify(instruction)]);
  assert.equal(call.metadataPayload, JSON.stringify({ tag: "value" }));
  assert.equal(call.creationTimeMs, 10);
  assert.equal(call.ttlMs, 20);
  assert.equal(call.nonce, 5);
});

test("buildTransaction rejects empty instruction arrays", () => {
  assert.throws(
    () =>
      buildTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        instructions: [],
        privateKey: PRIVATE_KEY,
      }),
    /non-empty array/i,
  );
});

test("buildMintAssetTransaction returns canonical hash", () => {
  const built = buildMintAssetTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    assetId: CANONICAL_ASSET_ID_INPUT,
    quantity: "10",
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildTransferAssetTransaction returns canonical hash", () => {
  const built = buildTransferAssetTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    sourceAssetId: CANONICAL_ASSET_ID_INPUT,
    quantity: "3",
    destinationAccountId: AUTHORITY_ID_INPUT,
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildMintAndTransferTransaction composes instructions in order", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10, 0x20]),
    hash: Buffer.alloc(32, 0xbb),
  };
  withNativeBinding(
    {
      buildTransaction: (_, __, instructions) => {
        captures.push(instructions.map((payload) => JSON.parse(payload)));
        return fakeResult;
      },
    },
    () => {
      const result = buildMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        mint: { assetId: ASSET_ID_INPUT, quantity: "6" },
        transfer: {
          quantity: "2",
          destinationAccountId: AUTHORITY_ID_INPUT,
        },
        privateKey: PRIVATE_KEY,
      });
      assert.deepEqual(result.hash, Buffer.from(fakeResult.hash));
    },
  );
  assert.equal(captures.length, 1);
  const [mintInstruction, transferInstruction] = captures[0];
  assert.deepEqual(mintInstruction, {
    Mint: { Asset: { destination: ASSET_ID, object: "6" } },
  });
  assert.deepEqual(transferInstruction, {
    Transfer: {
      Asset: {
        source: ASSET_ID,
        object: "2",
        destination: AUTHORITY_ID,
      },
    },
  });
});

test("buildRegisterDomainAndMintTransaction supports mint arrays", () => {
  const captures = [];
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x30]),
          hash: Buffer.alloc(32, 0xcc),
        };
      },
    },
    () =>
      buildRegisterDomainAndMintTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        domain: { domainId: "wonderland" },
        mints: [
          { assetId: ASSET_ID_INPUT, quantity: "4" },
          { assetId: `lily##${AUTHORITY_ID_INPUT}`, quantity: "1" },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 3);
  assert.deepEqual(
    instructions[0],
    buildRegisterDomainInstruction({ domainId: "wonderland" }),
  );
  assert.deepEqual(instructions[1], {
    Mint: { Asset: { destination: ASSET_ID, object: "4" } },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: `lily##${AUTHORITY_ID}`,
        object: "1",
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction supports transfer arrays", () => {
  const captures = [];
  const secondAccountIdRaw =
    "6cmzPVPX7iXwUZwgBeaKv96unyGNU1Z5xSmzKApk6TUXv7bTs4t4wZm";
  const secondAccountId = ih58FromEd25519AccountId(secondAccountIdRaw);
  const secondAccountIdInput = ih58FromEd25519AccountId(secondAccountIdRaw);
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((j) => JSON.parse(j)) });
        return {
          signed_transaction: Buffer.from([0x31]),
          hash: Buffer.alloc(32, 0xdd),
        };
      },
    },
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: "rose#wonderland" },
        mints: [
          { accountId: AUTHORITY_ID_INPUT, quantity: "7" },
          { assetId: `rose#wonderland##${secondAccountIdInput}`, quantity: "2" },
        ],
        transfers: [
          { quantity: "5", destinationAccountId: AUTHORITY_ID_INPUT },
          {
            sourceAssetId: `rose#wonderland##${secondAccountIdInput}`,
            quantity: "1",
            destinationAccountId: secondAccountIdInput,
          },
        ],
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 5);
  assert.deepEqual(instructions[1], {
    Mint: {
      Asset: {
        destination: `rose#wonderland##${AUTHORITY_ID}`,
        object: "7",
      },
    },
  });
  assert.deepEqual(instructions[2], {
    Mint: {
      Asset: {
        destination: `rose#wonderland##${secondAccountId}`,
        object: "2",
      },
    },
  });
  assert.deepEqual(instructions[3], {
    Transfer: {
      Asset: {
        source: `rose#wonderland##${AUTHORITY_ID}`,
        object: "5",
        destination: AUTHORITY_ID,
      },
    },
  });
  assert.deepEqual(instructions[4], {
    Transfer: {
      Asset: {
        source: `rose#wonderland##${secondAccountId}`,
        object: "1",
        destination: secondAccountId,
      },
    },
  });
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction rejects mismatched assetId/accountId", () => {
  assert.throws(
    () =>
      buildRegisterAssetDefinitionMintAndTransferTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        assetDefinition: { assetDefinitionId: "rose#wonderland" },
        mints: [
          {
            accountId: AUTHORITY_ID_INPUT,
            assetId: "rose#wonderland##someone_else",
            quantity: "1",
          },
        ],
        transfers: [{ quantity: "1", destinationAccountId: AUTHORITY_ID_INPUT }],
        privateKey: PRIVATE_KEY,
      }),
    /must match/,
  );
});

test("buildMintAndTransferTransaction returns canonical hash", () => {
  const built = buildMintAndTransferTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    mint: { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "8" },
    transfer: {
      sourceAssetId: CANONICAL_ASSET_ID_INPUT,
      quantity: "3",
      destinationAccountId: AUTHORITY_ID_INPUT,
    },
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildRegisterAssetDefinitionMintAndTransferTransaction returns canonical hash", () => {
  const built = buildRegisterAssetDefinitionMintAndTransferTransaction({
    chainId: "test-chain",
    authority: AUTHORITY_ID_INPUT,
    assetDefinition: { assetDefinitionId: "rose#wonderland" },
    mint: { assetId: CANONICAL_ASSET_ID_INPUT, quantity: "4" },
    transfer: {
      sourceAssetId: CANONICAL_ASSET_ID_INPUT,
      destinationAccountId: AUTHORITY_ID_INPUT,
      quantity: "1",
    },
    privateKey: PRIVATE_KEY,
  });
  assert.ok(Buffer.isBuffer(built.signedTransaction));
  const recomputed = hashSignedTransaction(built.signedTransaction, {
    encoding: "buffer",
  });
  assert.deepEqual(recomputed, built.hash);
});

test("buildCreateKaigiTransaction composes Kaigi create instruction", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x40]),
    hash: Buffer.alloc(32, 0x55),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((payload) => JSON.parse(payload)) });
        return fakeResult;
      },
    },
    () =>
      buildCreateKaigiTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        call: {
          id: { domainId: "wonderland", callName: "weekly-sync" },
          host: AUTHORITY_ID_INPUT,
          gasRatePerMinute: 120,
          relayManifest: {
            expiryMs: 1700111000000,
            hops: [
              {
                relayId: RELAY_ACCOUNT_ID_INPUT,
                hpkePublicKey: Buffer.alloc(32, 0x01),
                weight: 2,
              },
            ],
          },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  assert.equal(instructions.length, 1);
  const created = instructions[0];
  assert.deepEqual(created.Kaigi.CreateKaigi.call.id, {
    domain_id: "wonderland",
    call_name: "weekly-sync",
  });
  assert.equal(created.Kaigi.CreateKaigi.call.gas_rate_per_minute, 120);
  assert.deepEqual(created.Kaigi.CreateKaigi.call.relay_manifest.hops[0], {
    relay_id: RELAY_ACCOUNT_ID,
    hpke_public_key: "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
    weight: 2,
  });
});

test("buildJoinKaigiTransaction normalizes binary fields", () => {
  const commitment = Buffer.alloc(32, 0x77);
  const nullifier = Buffer.alloc(32, 0x22);
  const proof = Buffer.from([0xaa, 0xbb, 0xcc, 0xdd]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x41]),
    hash: Buffer.alloc(32, 0x66),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((payload) => JSON.parse(payload)) });
        return fakeResult;
      },
    },
    () =>
      buildJoinKaigiTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        join: {
          callId: "wonderland:weekly-sync",
          participant: AUTHORITY_ID_INPUT,
          commitment: { commitment, aliasTag: "alice" },
          nullifier: { digest: nullifier, issuedAtMs: 42 },
          proof,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  const joinInstruction = instructions[0].Kaigi.JoinKaigi;
  assert.equal(joinInstruction.participant, AUTHORITY_ID);
  assert.equal(joinInstruction.commitment.commitment, normalizedHashHex(commitment));
  assert.equal(joinInstruction.nullifier.digest, normalizedHashHex(nullifier));
  assert.equal(joinInstruction.nullifier.issued_at_ms, 42);
  assert.equal(joinInstruction.proof, proof.toString("base64"));
});

test("buildRegisterKaigiRelayTransaction encodes hpke key", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x42]),
    hash: Buffer.alloc(32, 0x77),
  };
  const relayId = RELAY_ACCOUNT_ID;
  const relayIdInput = RELAY_ACCOUNT_ID_INPUT;
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((payload) => JSON.parse(payload)) });
        return fakeResult;
      },
    },
    () =>
      buildRegisterKaigiRelayTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        relay: {
          relayId: relayIdInput,
          hpkePublicKey: Buffer.alloc(32, 0xaa),
          bandwidthClass: 6,
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const [{ instructions }] = captures;
  const relayInstruction = instructions[0].Kaigi.RegisterKaigiRelay;
  assert.equal(relayInstruction.relay.relay_id, relayId);
  assert.equal(
    relayInstruction.relay.hpke_public_key,
    "qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqo=",
  );
  assert.equal(relayInstruction.relay.bandwidth_class, 6);
});

test("buildProposeDeployContractTransaction wraps proposal", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x10]),
    hash: Buffer.alloc(32, 0x10),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push({ authority, instructions: instructions.map((payload) => JSON.parse(payload)) });
        return fakeResult;
      },
    },
    () =>
      buildProposeDeployContractTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        proposal: {
          namespace: "apps",
          contractId: "ledger",
          codeHash: "aa".repeat(32),
          abiHash: "bb".repeat(32),
          window: { lower: 1, upper: 2 },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures.length, 1);
  const propose = captures[0].instructions[0].ProposeDeployContract;
  assert.equal(propose.namespace, "apps");
});

test("buildCastZkBallotTransaction encodes ballot", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x11]),
    hash: Buffer.alloc(32, 0x11),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildCastZkBallotTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        ballot: {
          electionId: "ref-1",
          proof: Buffer.alloc(32, 0x01),
          publicInputs: { tally: "aye" },
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].CastZkBallot.election_id, "ref-1");
});

test("buildCastPlainBallotTransaction normalizes amount", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x12]),
    hash: Buffer.alloc(32, 0x12),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildCastPlainBallotTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        ballot: {
          referendumId: "ref-2",
          owner: AUTHORITY_ID_INPUT,
          amount: 10,
          durationBlocks: 5,
          direction: "aye",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].CastPlainBallot.direction, 0);
});

test("buildEnactReferendumTransaction wraps enactment", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x13]),
    hash: Buffer.alloc(32, 0x13),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildEnactReferendumTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        enactment: {
          referendumId: Buffer.alloc(32, 0x33),
          preimageHash: Buffer.alloc(32, 0x44),
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.ok(captures[0].EnactReferendum);
});

test("buildFinalizeReferendumTransaction normalizes proposal id", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x14]),
    hash: Buffer.alloc(32, 0x14),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildFinalizeReferendumTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        finalization: {
          referendumId: "ref-3",
          proposalId: Buffer.alloc(32, 0x55),
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.deepEqual(
    captures[0].FinalizeReferendum.proposal_id,
    toByteArray(Buffer.alloc(32, 0x55)),
  );
});

test("buildPersistCouncilForEpochTransaction wraps council", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x15]),
    hash: Buffer.alloc(32, 0x15),
  };
  withNativeBinding(
    {
      buildTransaction: (_chain, authority, instructions) => {
        captures.push(JSON.parse(instructions[0]));
        return fakeResult;
      },
    },
    () =>
      buildPersistCouncilForEpochTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        record: {
          epoch: 1,
          members: [AUTHORITY_ID_INPUT],
          candidatesCount: 5,
          derivedBy: "fallback",
        },
        privateKey: PRIVATE_KEY,
      }),
  );
  assert.equal(captures[0].PersistCouncilForEpoch.members.length, 1);
});

test("buildRegisterSmartContractCodeTransaction wraps manifest instruction", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x01]),
    hash: Buffer.alloc(32, 0xbb),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const result = buildRegisterSmartContractCodeTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        manifest: {
          codeHash: Buffer.alloc(32, 0xaa),
          compilerFingerprint: "rustc",
        },
        privateKey: PRIVATE_KEY,
      });
      assert.ok(Buffer.isBuffer(result.hash));
    },
  );
  assert.equal(captures.length, 1);
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(parsed.RegisterSmartContractCode.manifest.compiler_fingerprint, "rustc");
});

test("buildRegisterSmartContractBytesTransaction encodes code payload", () => {
  const codeBytes = Buffer.from([0xde, 0xad]);
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x02]),
    hash: Buffer.alloc(32, 0xcc),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      const result = buildRegisterSmartContractBytesTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        codeHash: Buffer.alloc(32, 0xdd),
        code: codeBytes,
        privateKey: PRIVATE_KEY,
      });
      assert.ok(Buffer.isBuffer(result.signedTransaction));
    },
  );
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(parsed.RegisterSmartContractBytes.code, codeBytes.toString("base64"));
});

test("buildDeactivateContractInstanceTransaction wraps instruction with reason", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x02]),
    hash: Buffer.alloc(32, 0xdd),
  };
  withNativeBinding(
    {
      buildTransaction: (...args) => {
        captures.push(args[2]);
        return fakeResult;
      },
    },
    () => {
      buildDeactivateContractInstanceTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        namespace: "apps",
        contractId: "ledger",
        reason: "audit",
        privateKey: PRIVATE_KEY,
      });
    },
  );
  const parsed = JSON.parse(captures[0][0]);
  assert.equal(parsed.DeactivateContractInstance.reason, "audit");
});

test("buildActivateContractInstanceTransaction normalizes identifiers", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x03]),
    hash: Buffer.alloc(32, 0xdd),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions,
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    () => {
      buildActivateContractInstanceTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        namespace: "apps",
        contractId: "governance",
        codeHash: Buffer.alloc(32, 0xee),
        privateKey: PRIVATE_KEY,
      });
    },
  );
  const parsed = JSON.parse(captures[0].instructions[0]);
  assert.equal(parsed.ActivateContractInstance.contract_id, "governance");
});

test("buildRemoveSmartContractBytesTransaction wraps removal payload", () => {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0x04]),
    hash: Buffer.alloc(32, 0xee),
  };
  withNativeBinding(
    {
      buildTransaction: (...args) => {
        captures.push(args[2]);
        return fakeResult;
      },
    },
    () => {
      buildRemoveSmartContractBytesTransaction({
        chainId: "test-chain",
        authority: AUTHORITY_ID_INPUT,
        codeHash: Buffer.alloc(32, 0xaa),
        reason: "cleanup",
        privateKey: PRIVATE_KEY,
      });
    },
  );
  const parsed = JSON.parse(captures[0][0]);
  assert.equal(parsed.RemoveSmartContractBytes.reason, "cleanup");
});

test("confidential transaction builders wrap expected instruction payloads", () => {
  const encryptedPayload = {
    version: 1,
    ephemeralPublicKey: Buffer.alloc(32, 0x01),
    nonce: Buffer.alloc(24, 0x02),
    ciphertext: Buffer.from("note"),
  };
  const proof = {
    backend: "halo2/ipa",
    proof: Buffer.from("proof"),
    verifyingKeyRef: "halo2/ipa:vk_transfer",
  };
  const register = captureInstructionObject(() =>
    buildRegisterZkAssetTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      registration: {
        assetDefinitionId: "rose#wonderland",
        mode: "Hybrid",
        transferVerifyingKey: "halo2/ipa:vk_transfer",
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(register.zk?.RegisterZkAsset);

  const policy = captureInstructionObject(() =>
    buildScheduleConfidentialPolicyTransitionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      transition: {
        assetDefinitionId: "rose#wonderland",
        newMode: "TransparentOnly",
        effectiveHeight: 5,
        transitionId: Buffer.alloc(32, 0xaa),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(policy.zk?.ScheduleConfidentialPolicyTransition);

  const cancel = captureInstructionObject(() =>
    buildCancelConfidentialPolicyTransitionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      cancellation: {
        assetDefinitionId: "rose#wonderland",
        transitionId: Buffer.alloc(32, 0xbb),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(cancel.zk?.CancelConfidentialPolicyTransition);

  const shield = captureInstructionObject(() =>
    buildShieldTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      shield: {
        assetDefinitionId: "rose#wonderland",
        fromAccountId: AUTHORITY_ID_INPUT,
        amount: "10",
        noteCommitment: Buffer.alloc(32, 0x03),
        encryptedPayload,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(shield.zk?.Shield);

  const transfer = captureInstructionObject(() =>
    buildZkTransferTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      transfer: {
        assetDefinitionId: "rose#wonderland",
        inputs: [Buffer.alloc(32, 0x10)],
        outputs: [Buffer.alloc(32, 0x20)],
        proof,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(transfer.zk?.ZkTransfer);

  const unshield = captureInstructionObject(() =>
    buildUnshieldTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      unshield: {
        assetDefinitionId: "rose#wonderland",
        destinationAccountId: AUTHORITY_ID_INPUT,
        publicAmount: 3,
        inputs: [Buffer.alloc(32, 0x30)],
        proof,
        rootHint: Buffer.alloc(32, 0x40),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(unshield.zk?.Unshield);

  const election = captureInstructionObject(() =>
    buildCreateElectionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      election: {
        electionId: "election-1",
        options: 2,
        eligibleRoot: Buffer.alloc(32, 0x05),
        startTs: 1,
        endTs: 2,
        ballotVerifyingKey: "halo2/ipa:vk_ballot",
        tallyVerifyingKey: { backend: "halo2/ipa", name: "vk_tally" },
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(election.zk?.CreateElection);

  const ballot = captureInstructionObject(() =>
    buildSubmitBallotTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      ballot: {
        electionId: "election-1",
        ciphertext: Buffer.from("encrypted"),
        ballotProof: proof,
        nullifier: Buffer.alloc(32, 0x06),
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(ballot.zk?.SubmitBallot);

  const finalize = captureInstructionObject(() =>
    buildFinalizeElectionTransaction({
      chainId: "test-chain",
      authority: AUTHORITY_ID_INPUT,
      finalization: {
        electionId: "election-1",
        tally: [1n],
        tallyProof: proof,
      },
      privateKey: PRIVATE_KEY,
    }),
  );
  assert.ok(finalize.zk?.FinalizeElection);
});

function captureInstructionObject(buildFn) {
  const captures = [];
  const fakeResult = {
    signed_transaction: Buffer.from([0xff]),
    hash: Buffer.alloc(32, 0xff),
  };
  withNativeBinding(
    {
      buildTransaction: (
        chainId,
        authority,
        instructions,
        metadataPayload,
        creationTimeMs,
        ttlMs,
        nonce,
        secret,
      ) => {
        captures.push({
          chainId,
          authority,
          instructions: instructions.map((payload) => JSON.parse(payload)),
          metadataPayload,
          creationTimeMs,
          ttlMs,
          nonce,
          secret,
        });
        return fakeResult;
      },
    },
    buildFn,
  );
  return captures[0].instructions[0];
}

function withNativeBinding(binding, fn) {
  const previous = globalThis.__IROHA_NATIVE_BINDING__;
  globalThis.__IROHA_NATIVE_BINDING__ = binding;
  try {
    return fn();
  } finally {
    globalThis.__IROHA_NATIVE_BINDING__ = previous;
  }
}
