"use strict";

// Cross-implementation check of the Rust SCCP v1 vectors against the EVM
// destination (`specs/sccp.md` §3, §5.1.3, §5.1.5, §5.1.6, §5.1.8, §5.2.2,
// §11). It loads `fixtures/sccp/{eip712,evm_calldata,control,commitment_tree}_v1.json`,
// which `crates/iroha_sccp/tests/v1_vectors.rs` generates from `iroha_sccp::v1`
// (the same encoders `iroha_sccp_wallet` uses), and proves on the locked EDR
// runtime with `SccpTairaXor.sol` that:
//
// - every selector, event topic and EIP-712 typehash of the fixtures equals the
//   contract ABI and the independent ethers model, and the domain separator
//   equals the deployed contract's `domainSeparator()`;
// - the fixture digests, leaves, Merkle roots and signatures agree with the model
//   (the signatures come from the fixtures' fixed keys);
// - the Rust-built calldata of `finalizeFromTaira`, `applyControl`,
//   `rotateRosters`, their historical forms and the voids is canonical ABI and is
//   accepted by the contract, with the expected state changes and events.
//
// The fixtures bind leaves to the destination word `word(0x22…22)`, so each
// test deploys the contract with the scenario's roster g7 and relocates its
// runtime and storage to that address (`hardhat_setCode`/`hardhat_setStorageAt`);
// the contract derives its destination word from `address()` at run time.
//
// Run: `node --test contracts/evm/sccp/test/sccp_v1_fixtures.test.js` after
// `npm ci --ignore-scripts` in `scripts/contract_tooling/evm-runtime`.

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { after, describe, it } = require("node:test");
const m = require("./sccp_v1_model.js");
const h = require("./sccp_edr_harness.js");

const { ethers, PROFILES, REPO } = m;
const { Chain, Destination, expectRevert, loadArtifacts } = h;

const FIXTURES = path.join(REPO, "fixtures", "sccp");
const DESTINATION = "0x2222222222222222222222222222222222222222";
const T0_S = 1_800_000_000n;
const CAP = 1_000_000_000_000_000_000n;
const STORAGE_SLOTS = 8;

function load(name) {
  return JSON.parse(fs.readFileSync(path.join(FIXTURES, name), "utf8"));
}

const EIP712 = load("eip712_v1.json");
const CALLDATA = load("evm_calldata_v1.json");
const CONTROL = load("control_v1.json");
const TREE = load("commitment_tree_v1.json");
const SCENARIO = CALLDATA.scenario;
const NETWORK_ID = SCENARIO.taira_network_id;

function call(label) {
  const found = CALLDATA.calls.filter((entry) => entry.label === label);
  assert.equal(found.length, 1, `exactly one golden call ${label}`);
  return found[0];
}

/** Fixture attestation in the model's field names. */
function attestation(fixture) {
  return {
    height: BigInt(fixture.height),
    epoch: BigInt(fixture.epoch),
    timestampMs: BigInt(fixture.timestamp_ms),
    blockHash: fixture.block_hash,
    sccpRoot: fixture.sccp_root,
    messageCount: BigInt(fixture.message_count),
    historyRoot: fixture.history_root,
    historySize: BigInt(fixture.history_size),
    rosterDigest: fixture.roster_digest,
    nextRosterDigest: fixture.next_roster_digest,
  };
}

/** Fixture roster as the constructor's `RosterV1` argument. */
function rosterStruct(fixture) {
  return {
    generation: BigInt(fixture.generation),
    validFromMs: BigInt(fixture.valid_from_ms),
    validUntilMs: BigInt(fixture.valid_until_ms),
    threshold: fixture.threshold,
    members: fixture.packed_members,
  };
}

/** §2.1 target tag and identity word of a fixture profile key. */
function target(profileKey) {
  switch (profileKey) {
    case "ethereum-mainnet":
      return { tag: 0x41, identity: m.word(1n) };
    case "bsc-mainnet":
      return { tag: 0x42, identity: m.word(56n) };
    case "tron-mainnet":
      return { tag: 0x43, identity: m.word(0x2b6653dcn) };
    case "ton-mainnet":
      return { tag: 0x44, identity: ethers.toBeHex(ethers.toTwos(-239n, 256), 32) };
    default:
      throw new Error(`unknown profile ${profileKey}`);
  }
}

/** Offset of the first byte after one `codec ‖ u16 len ‖ bytes` field at `offset`. */
function fieldEnd(bytes, offset) {
  return offset + 3 + ((bytes[offset + 1] << 8) | bytes[offset + 2]);
}

/** Nonce, amount and recipient of a §3.2 Taira → EVM payload. */
function payloadFields(payload) {
  const bytes = ethers.getBytes(payload);
  const nonce = BigInt(ethers.hexlify(bytes.slice(10, 18)));
  // kind, version, domains, nonce, revision, deadline and asset home domain take 34 bytes.
  const assetEnd = fieldEnd(bytes, 34);
  const amount = BigInt(ethers.hexlify(bytes.slice(assetEnd, assetEnd + 16)));
  const recipientAt = fieldEnd(bytes, assetEnd + 16);
  assert.equal(bytes[recipientAt], 2, "EVM recipient codec");
  const recipient = ethers.getAddress(ethers.hexlify(bytes.slice(recipientAt + 3, recipientAt + 23)));
  return { nonce, amount, recipient };
}

/** Deploys the scenario destination (g7) and relocates it to `DESTINATION`. */
async function scenarioDestination() {
  const chain = await Chain.open(PROFILES.ethereum);
  const { destination: deployed } = await h.deploy(chain, {
    networkId: NETWORK_ID,
    revision: SCENARIO.destination.route_revision,
    cap: CAP,
    roster: rosterStruct(SCENARIO.roster_g7),
    at: T0_S + 100n,
  });
  const code = await chain.rpc("eth_getCode", [deployed.address, "latest"]);
  await chain.rpc("hardhat_setCode", [DESTINATION, code]);
  for (let slot = 0; slot < STORAGE_SLOTS; slot += 1) {
    const key = ethers.toQuantity(slot);
    const value = await chain.rpc("eth_getStorageAt", [deployed.address, key, "latest"]);
    await chain.rpc("hardhat_setStorageAt", [DESTINATION, key, value]);
  }
  const destination = new Destination(chain, DESTINATION);
  assert.deepEqual(
    [...(await destination.view("rosterState"))],
    [...(await deployed.view("rosterState"))],
    "the relocated destination keeps its roster state",
  );
  return { chain, destination };
}

const chains = [];

async function open() {
  const context = await scenarioDestination();
  chains.push(context.chain);
  return context;
}

after(async () => {
  for (const chain of chains) await chain.close();
});

describe("SCCP v1 fixtures: selectors, topics and typehashes", () => {
  it("fixture selectors equal the contract ABI and the canonical signatures", () => {
    const { iface } = loadArtifacts();
    assert.equal(CALLDATA.selectors.length, 22);
    for (const { function: name, signature, selector } of CALLDATA.selectors) {
      assert.equal(m.selector(signature), selector, signature);
      const fragment = iface.getFunction(name);
      assert(fragment, `the ABI has ${name}`);
      assert.equal(fragment.format("sighash"), signature);
      assert.equal(fragment.selector, selector);
    }
  });

  it("fixture event topics equal the contract ABI", () => {
    const { iface } = loadArtifacts();
    assert.equal(CALLDATA.events.length, 5);
    for (const { signature, topic0 } of CALLDATA.events) {
      assert.equal(m.topic(signature), topic0, signature);
      const event = iface.getEvent(signature.slice(0, signature.indexOf("(")));
      assert.equal(event.format("sighash"), signature);
      assert.equal(event.topicHash, topic0);
    }
    assert.equal(CONTROL.topic_control_applied, iface.getEvent("SccpControlApplied").topicHash);
    assert.equal(m.topic(CONTROL.event_signature), CONTROL.topic_control_applied);
  });

  it("EIP-712 typehashes, domain and digests equal the model", () => {
    assert.equal(EIP712.domain.type, m.DOMAIN_TYPE);
    assert.equal(EIP712.attestation.type, m.ATTESTATION_TYPE);
    assert.equal(EIP712.bridge_key_pop.type, m.KEY_POP_TYPE);
    assert.equal(m.topic(EIP712.domain.type), EIP712.domain.typehash);
    assert.equal(m.topic(EIP712.attestation.type), EIP712.attestation.typehash);
    assert.equal(m.topic(EIP712.bridge_key_pop.type), EIP712.bridge_key_pop.typehash);
    assert.equal(m.topic("SCCP"), EIP712.domain.name_hash);
    assert.equal(m.topic("1"), EIP712.domain.version_hash);
    assert.equal(m.domainSeparator(EIP712.taira_network_id), EIP712.domain.separator);
    assert.equal(
      m.attestationDigest(EIP712.taira_network_id, attestation(EIP712.attestation.fields)),
      EIP712.attestation.fields.digest,
    );
    assert.equal(
      m.attestationDigest(EIP712.taira_network_id, attestation(EIP712.rotation_attestation)),
      EIP712.rotation_attestation.digest,
    );
    for (const key of ["attestation_100", "attestation_150_rotation", "attestation_200"]) {
      assert.equal(m.attestationDigest(NETWORK_ID, attestation(SCENARIO[key])), SCENARIO[key].digest, key);
    }
  });

  it("the fixed-key signatures recover to their keys and the scenario rosters", () => {
    for (const key of EIP712.keys) {
      assert.equal(ethers.computeAddress(key.secret).toLowerCase(), key.address);
    }
    for (const entry of EIP712.signatures) {
      assert.equal(ethers.recoverAddress(entry.digest, entry.signature).toLowerCase(), entry.address);
    }
    const sets = [
      ["signatures_g7_100", "roster_g7", "attestation_100"],
      ["signatures_g7_150", "roster_g7", "attestation_150_rotation"],
      ["signatures_g8_200", "roster_g8", "attestation_200"],
    ];
    for (const [setKey, rosterKey, attestationKey] of sets) {
      const set = SCENARIO[setKey];
      const members = SCENARIO[rosterKey].members;
      const signatures = ethers.getBytes(set.signatures);
      let offset = 0;
      for (let bit = 0; bit < members.length; bit += 1) {
        if ((set.signer_bitmap >> bit) & 1) {
          const signature = ethers.hexlify(signatures.slice(offset, offset + 65));
          offset += 65;
          assert.equal(
            ethers.recoverAddress(SCENARIO[attestationKey].digest, signature).toLowerCase(),
            members[bit],
            `${setKey} bit ${bit}`,
          );
        }
      }
      assert.equal(offset, signatures.length);
      assert.equal(m.rosterDigest(NETWORK_ID, rosterStruct(SCENARIO[rosterKey])), SCENARIO[rosterKey].digest);
    }
  });

  it("control leaves, transfer leaves and Merkle roots equal the model", () => {
    for (const entry of [...CONTROL.spec_examples, ...CONTROL.controls]) {
      const { tag, identity } = target(entry.target);
      assert.equal(
        m.controlLeaf({
          networkId: CONTROL.taira_network_id,
          targetTag: tag,
          targetIdentity: identity,
          destinationWord: entry.destination_word,
          revision: entry.route_revision,
          nonce: BigInt(entry.control_nonce),
          paused: entry.paused,
        }),
        entry.leaf,
        `${entry.target} r${entry.route_revision} n${entry.control_nonce}`,
      );
    }
    const example = TREE.examples.transfer;
    assert.equal(m.transferLeaf(example.message_id, example.destination_word), example.leaf);
    const ethereum = PROFILES.ethereum;
    for (const payload of SCENARIO.payloads) {
      assert.equal(m.outboundMessageId(NETWORK_ID, ethereum, payload.payload), payload.message_id);
    }
    for (const tree of TREE.trees) {
      const leaves = TREE.leaves.slice(0, tree.count);
      assert.equal(m.merkleRoot(leaves), tree.root, `count ${tree.count}`);
      // Every path of the small trees; a sample of the 512-leaf tree (the Rust suite checks all).
      const stride = tree.count > 17 ? 37 : 1;
      tree.paths.forEach((pathFixture, index) => {
        if (index % stride !== 0 && index !== tree.count - 1) return;
        assert.deepEqual(m.merklePath(leaves, index), pathFixture, `count ${tree.count} index ${index}`);
        assert.equal(m.merkleRootFromPath(leaves[index], index, tree.count, pathFixture), tree.root);
      });
    }
  });

  it("the Rust calldata is the canonical ABI encoding of its arguments", () => {
    const { iface } = loadArtifacts();
    for (const golden of CALLDATA.calls) {
      const parsed = iface.parseTransaction({ data: golden.calldata });
      assert.equal(parsed.name, golden.function, golden.label);
      assert.equal(iface.encodeFunctionData(parsed.fragment, parsed.args), golden.calldata, golden.label);
    }
  });
});

describe("SCCP v1 fixtures: Rust calldata on EDR", () => {
  it("accepts finalizeFromTaira, then applyControl pauses minting", async () => {
    const { chain, destination } = await open();
    assert.equal(await destination.view("domainSeparator"), EIP712.domain.separator);
    assert.equal(await destination.view("tairaNetworkId"), NETWORK_ID);
    assert.equal(await destination.view("initialRosterDigest"), SCENARIO.roster_g7.digest);

    const finalize = await destination.raw(call("finalize_direct").calldata, { at: T0_S + 200n });
    const payload = SCENARIO.payloads.find((entry) => entry.nonce === 1);
    const fields = payloadFields(payload.payload);
    const [finalized] = destination.events(finalize, "SccpFinalized");
    assert.equal(finalized.parsed.args.messageId, payload.message_id);
    assert.equal(finalized.parsed.args.nonce, 1n);
    assert.equal(finalized.parsed.args.recipient, fields.recipient);
    assert.equal(finalized.parsed.args.tokenAmount, fields.amount);
    assert.equal(await destination.view("isConsumed", [1n]), true);
    assert.equal(await destination.view("balanceOf", [fields.recipient]), fields.amount);
    assert.equal(await destination.view("totalSupply"), fields.amount);
    assert.equal(await destination.view("opCount"), 1n);

    const apply = await destination.raw(call("apply_control_direct").calldata);
    const [applied] = destination.events(apply, "SccpControlApplied");
    assert.equal(applied.parsed.args.controlNonce, 1n);
    assert.equal(applied.parsed.args.paused, true);
    assert.equal(await destination.view("mintingPaused"), true);
    assert.equal(await destination.view("controlNonce"), 1n);
    assert.equal(await destination.view("opCount"), 2n);
    await expectRevert(destination.raw(call("apply_control_direct").calldata), "StaleControl");
    await expectRevert(destination.raw(call("finalize_direct").calldata), "MintingIsPaused");
    assert(chain.nowMs() > (T0_S + 200n) * 1000n);
  });

  it("accepts rotateRosters, then the historical finalize and control", async () => {
    const { destination } = await open();
    const rotate = await destination.raw(call("rotate_one").calldata, { at: T0_S + 200n });
    const [rotated] = destination.events(rotate, "SccpRosterRotated");
    assert.equal(rotated.parsed.args.generation, 8n);
    assert.equal(rotated.parsed.args.digest, SCENARIO.roster_g8.digest);
    const state = await destination.chain.call(DESTINATION, "0x85a00f5c");
    assert.equal(state, SCENARIO.roster_state_after_rotation, "rosterState() equals the Rust encoding");

    const finalize = await destination.raw(call("finalize_historical").calldata, { at: T0_S + 300n });
    const payload = SCENARIO.payloads.find((entry) => entry.nonce === 2);
    const [finalized] = destination.events(finalize, "SccpFinalized");
    assert.equal(finalized.parsed.args.messageId, payload.message_id);
    assert.equal(finalized.parsed.args.nonce, 2n);
    assert.equal(await destination.view("isConsumed", [2n]), true);

    await destination.raw(call("apply_control_historical").calldata);
    assert.equal(await destination.view("mintingPaused"), true);
    assert.equal(await destination.view("controlNonce"), 1n);
    assert.equal(await destination.view("opCount"), 2n, "rotations are not counted");
  });

  it("accepts voidExpired and voidExpiredHistorical after the deadline", async () => {
    const { destination } = await open();
    await destination.raw(call("rotate_one").calldata, { at: T0_S + 200n });
    const deadlineS = BigInt(SCENARIO.deadline_ms) / 1000n;
    await expectRevert(destination.raw(call("void_expired").calldata, { at: deadlineS }), "DeadlineNotReached");

    const voided = await destination.raw(call("void_expired").calldata, { at: deadlineS + 1n });
    const [direct] = destination.events(voided, "SccpVoided");
    assert.equal(direct.parsed.args.messageId, SCENARIO.payloads.find((entry) => entry.nonce === 1).message_id);
    assert.equal(direct.parsed.args.nonce, 1n);
    const historical = await destination.raw(call("void_expired_historical").calldata);
    const [late] = destination.events(historical, "SccpVoided");
    assert.equal(late.parsed.args.nonce, 2n);
    assert.equal(await destination.view("isConsumed", [1n]), true);
    assert.equal(await destination.view("isConsumed", [2n]), true);
    assert.equal(await destination.view("totalSupply"), 0n);
    // Both rosters are still accepted, so the destination is not frozen.
    await expectRevert(destination.raw(call("void_frozen").calldata), "NotFrozen");
  });
});
