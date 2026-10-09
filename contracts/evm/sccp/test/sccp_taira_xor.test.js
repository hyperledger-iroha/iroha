"use strict";

// EDR suite for `contracts/evm/sccp/SccpTairaXor.sol` (`specs/sccp.md` §5.1,
// §5.2, §9.2, §9.4 and the §11 Contracts bullets). Every rule runs under the
// Ethereum (1), BSC (56) and TRON (0x2b6653dc) chain ids with the locked
// Ethereum-compiler build; the TRON-compiler build is qualified on java-tron
// (TRE) separately. Attestations are signed from fixed keys with ethers, and
// every digest, leaf, selector, topic and typehash comes from the independent
// model in `sccp_v1_model.js`.
//
// Run: `node --test contracts/evm/sccp/test/sccp_taira_xor.test.js` after
// `npm ci --ignore-scripts` in `scripts/contract_tooling/evm-runtime`. The
// harness builds and verifies the contract through the artifact corridor.
// Set `SCCP_GAS_REPORT=<file>` to also write the measured gas as JSON.

const assert = require("node:assert/strict");
const { after, describe, it } = require("node:test");
const m = require("./sccp_v1_model.js");
const h = require("./sccp_edr_harness.js");

const { ethers, PROFILES, ZERO32, ZERO_ADDRESS, N, HALF_N } = m;
const { Chain, expectRevert } = h;
const abi = ethers.AbiCoder.defaultAbiCoder();

const NETWORK_ID = `0x${"5a".repeat(32)}`;
const OTHER_NETWORK_ID = `0x${"a5".repeat(32)}`;
const REVISION = 3;
const UNIT = 1_000_000_000n; // one XOR in Taira units (9 decimals)
const CAP = 1_000_000n * UNIT;
const DAY_MS = 86_400_000n;
const HOUR_MS = 3_600_000n;
const DAY = 86_400n;
const HOUR = 3_600n;
const U64_MAX = (1n << 64n) - 1n;
const TAIRA_SENDER = ethers.concat(["0x0102", ethers.keccak256("0x01"), "0x0304"]);
const TAIRA_RECIPIENT = ethers.concat(["0x0102", ethers.keccak256("0x02")]);

// ---------------------------------------------------------------------------
// Scenario helpers
// ---------------------------------------------------------------------------

function rosterFor(profileLabel, generation, n, validFromMs, validUntilMs, zeroSlots = 0) {
  return m.makeRoster(`${profileLabel}/g${generation}/n${n}/z${zeroSlots}`, {
    n,
    zeroSlots,
    generation,
    validFromMs,
    validUntilMs,
  });
}

/** Deploys a fresh destination on a fresh chain; the initial roster is valid for 14 days. */
async function setup(profile, options = {}) {
  const chain = await Chain.open(profile);
  const t0 = chain.time + 100n;
  const n = options.n ?? 4;
  const generation = options.generation ?? 7n;
  const initial = rosterFor(
    "sccp",
    generation,
    n,
    options.validFromMs ?? t0 * 1000n - 60_000n,
    options.validUntilMs ?? t0 * 1000n + (options.validityMs ?? 14n * DAY_MS),
    options.zeroSlots ?? 0,
  );
  const networkId = options.networkId ?? NETWORK_ID;
  const revision = options.revision ?? REVISION;
  const cap = options.cap ?? CAP;
  const { destination, receipt } = await h.deploy(chain, { networkId, revision, cap, roster: initial, at: t0 });
  return {
    chain,
    profile,
    dest: destination,
    deployReceipt: receipt,
    networkId,
    revision,
    cap,
    taira: new m.TairaModel(networkId),
    current: initial,
    previous: null,
    initial,
    recipient: ethers.getAddress(chain.accounts[5]),
  };
}

/** Wraps a test body with a fresh destination and closes its chain afterwards. */
function withDestination(profile, options, body) {
  return async () => {
    const ctx = await setup(profile, options);
    try {
      await body(ctx);
    } finally {
      await ctx.chain.close();
    }
  };
}

/** One Taira -> destination transfer message addressed to `ctx.dest`. */
function transfer(ctx, { nonce, amount = UNIT, recipient = ctx.recipient, deadlineMs, overrides = {} }) {
  const fields = {
    ...m.outboundFields(ctx.profile, {
      nonce,
      revision: ctx.revision,
      deadlineMs: deadlineMs ?? ctx.chain.nowMs() + 7n * DAY_MS,
      amount,
      sender: TAIRA_SENDER,
      recipient,
    }),
    ...overrides,
  };
  const payload = m.encodePayload(fields);
  const id = m.outboundMessageId(ctx.networkId, ctx.profile, payload);
  return {
    kind: "transfer",
    payload,
    id,
    nonce: BigInt(nonce),
    amount: BigInt(amount),
    recipient,
    leaf: m.transferLeaf(id, m.addressWord(ctx.dest.address)),
  };
}

/** One Parliament control leaf; every binding defaults to this destination. */
function control(ctx, { nonce, paused, networkId = ctx.networkId, profile = ctx.profile, destination, revision }) {
  return {
    kind: "control",
    nonce: BigInt(nonce),
    paused,
    leaf: m.controlLeaf({
      networkId,
      targetTag: profile.tag,
      targetIdentity: m.identityWord(profile),
      destinationWord: m.addressWord(destination ?? ctx.dest.address),
      revision: revision ?? ctx.revision,
      nonce,
      paused,
    }),
  };
}

/** A leaf of some other route or destination. */
function foreign(label) {
  return { kind: "foreign", leaf: ethers.keccak256(ethers.toUtf8Bytes(label)) };
}

/**
 * Commits `items` as one Taira block (or reuses `block`) and signs its
 * attestation. `mutate` edits statement fields before signing; `tamper` edits
 * individual signatures.
 */
function attest(ctx, items, options = {}) {
  const roster = options.roster ?? ctx.current;
  const block =
    options.block ?? ctx.taira.commit(items.map((item) => item.leaf), options.timestampMs ?? ctx.chain.nowMs());
  const digest = options.rosterDigest ?? m.rosterDigest(ctx.networkId, roster.struct);
  let statement = ctx.taira.statement(block, digest);
  if (options.mutate) statement = { ...statement, ...options.mutate(statement) };
  const signatures = m.signatureSet(
    roster,
    m.attestationDigest(ctx.networkId, statement),
    options.signers ?? m.signerIndices(roster),
    options.tamper,
  );
  return { block, attestation: statement, roster: m.rosterArg(options.suppliedRoster ?? roster), signatures, items };
}

function leafIndex(a, item) {
  const index = a.block.leaves.indexOf(item.leaf);
  assert(index >= 0, "item is not a leaf of the attested block");
  return index;
}

function messageProof(a, item, overrides = {}) {
  const index = leafIndex(a, item);
  return { payload: item.payload, leafIndex: index, path: m.merklePath(a.block.leaves, index), ...overrides };
}

function controlProof(a, item, overrides = {}) {
  const index = leafIndex(a, item);
  return {
    controlNonce: item.nonce,
    paused: item.paused,
    leafIndex: index,
    path: m.merklePath(a.block.leaves, index),
    ...overrides,
  };
}

function finalize(ctx, a, item, options = {}, proof = messageProof(a, item)) {
  return ctx.dest.tx("finalizeFromTaira", [a.attestation, a.roster, a.signatures, proof], options);
}

function finalizeHistorical(ctx, a, history, item, options = {}, proof = null) {
  return ctx.dest.tx(
    "finalizeFromTairaHistorical",
    [a.attestation, a.roster, a.signatures, history, proof ?? messageProofOf(history, item)],
    options,
  );
}

function messageProofOf(historyBlock, item) {
  const index = historyBlock.leaves.indexOf(item.leaf);
  return { payload: item.payload, leafIndex: index, path: m.merklePath(historyBlock.leaves, index) };
}

function applyControl(ctx, a, item, options = {}, proof = controlProof(a, item)) {
  return ctx.dest.tx("applyControl", [a.attestation, a.roster, a.signatures, proof], options);
}

function voidExpired(ctx, nonce, a, item, options = {}) {
  return ctx.dest.tx("voidExpired", [nonce, a.attestation, a.roster, a.signatures, messageProof(a, item)], options);
}

/** Attests and finalizes one fresh transfer of `amount` to `recipient`. */
async function mintTo(ctx, { nonce, amount, recipient = ctx.recipient, at }) {
  const message = transfer(ctx, { nonce, amount, recipient });
  const a = attest(ctx, [message]);
  const receipt = await finalize(ctx, a, message, { at });
  return { message, receipt };
}

/** Next-generation roster whose validity starts at `validFromMs`. */
function successor(ctx, from, { validFromMs, validityMs = 14n * DAY_MS, n = from.n, generation } = {}) {
  const start = validFromMs ?? ctx.chain.nowMs() - 10_000n;
  const nextGeneration = generation ?? from.struct.generation + 1n;
  return rosterFor("sccp", nextGeneration, n, start, start + validityMs);
}

/**
 * `RotationV1` from `from` to `next`, signed by `from` over an empty rotation
 * block whose timestamp is `next.validFromMs` unless overridden.
 */
function rotation(ctx, from, next, options = {}) {
  const block = ctx.taira.commit([], options.timestampMs ?? next.struct.validFromMs);
  let statement = ctx.taira.statement(
    block,
    m.rosterDigest(ctx.networkId, from.struct),
    options.nextDigest ?? m.rosterDigest(ctx.networkId, next.struct),
  );
  if (options.mutate) statement = { ...statement, ...options.mutate(statement) };
  const signatures = m.signatureSet(
    from,
    m.attestationDigest(ctx.networkId, statement),
    options.signers ?? m.signerIndices(from),
  );
  return {
    attestation: statement,
    current: m.rosterArg(from),
    signatures,
    next: m.rosterArg(options.suppliedNext ?? next),
  };
}

async function rotate(ctx, rotations, options = {}) {
  return ctx.dest.tx("rotateRosters", [rotations], options);
}

async function rosterState(ctx) {
  const [digest, generation, validUntilMs, prevDigest, prevValidUntilMs] = await ctx.dest.view("rosterState");
  return { digest, generation, validUntilMs, prevDigest, prevValidUntilMs };
}

function assertTopics(log, expected) {
  assert.deepEqual(
    log.topics.map((value) => value.toLowerCase()),
    expected.map((value) => value.toLowerCase()),
  );
}

const TOPIC = {
  transfer: m.topic("Transfer(address,address,uint256)"),
  approval: m.topic("Approval(address,address,uint256)"),
  toTaira: m.topic("SccpTransferToTaira(bytes32,address,uint64,bytes)"),
  finalized: m.topic("SccpFinalized(bytes32,uint64,address,uint256)"),
  voided: m.topic("SccpVoided(bytes32,uint64)"),
  rotated: m.topic("SccpRosterRotated(uint64,bytes32,uint64)"),
  control: m.topic("SccpControlApplied(uint64,bool)"),
};

/** Checks the `SccpFinalized` and ERC-20 mint logs of one finalization receipt. */
function assertFinalizedLogs(ctx, receipt, message) {
  const logs = receipt.logs.filter((log) => ethers.getAddress(log.address) === ctx.dest.address);
  assert.equal(logs.length, 2);
  assertTopics(logs[0], [TOPIC.transfer, m.addressWord(ZERO_ADDRESS), m.addressWord(message.recipient)]);
  assert.equal(logs[0].data, m.word(message.amount));
  assertTopics(logs[1], [TOPIC.finalized, message.id, m.word(message.nonce), m.addressWord(message.recipient)]);
  assert.equal(logs[1].data, m.word(message.amount));
}

/** Canonical `transferToTaira` calldata split into editable parts. */
function transferCalldata(recipient, amount, nonce) {
  return h.loadArtifacts().iface.encodeFunctionData("transferToTaira", [recipient, amount, nonce]);
}

function padRight(hex) {
  const bytes = ethers.getBytes(hex);
  const padded = new Uint8Array(Math.ceil(bytes.length / 32) * 32);
  padded.set(bytes);
  return ethers.hexlify(padded);
}

// ---------------------------------------------------------------------------
// Constants, selectors, topics and typehashes (§3.6, §3.9, §5.2.2)
// ---------------------------------------------------------------------------

const A = "(uint64,uint64,uint64,bytes32,bytes32,uint32,bytes32,uint64,bytes32,bytes32)";
const R = "(uint64,uint64,uint64,uint8,bytes)";
const S = "(uint32,bytes)";
const MP = "(bytes,uint32,bytes32[])";
const HP = "(uint64,bytes32,uint32,uint64,bytes32[])";
const CP = "(uint64,bool,uint32,bytes32[])";

const SPEC_FUNCTIONS = [
  [`finalizeFromTaira(${A},${R},${S},${MP})`, "0x8056d161"],
  [`finalizeFromTairaHistorical(${A},${R},${S},${HP},${MP})`, "0x96925736"],
  [`rotateRosters((${A},${R},${S},${R})[])`, "0x909ea456"],
  [`applyControl(${A},${R},${S},${CP})`, "0x0ce970d6"],
  [`applyControlHistorical(${A},${R},${S},${HP},${CP})`, "0x935a913b"],
  ["transferToTaira(bytes,uint256,uint64)", "0xebfc6ca8"],
  [`voidExpired(uint64,${A},${R},${S},${MP})`, "0xc3de98ad"],
  [`voidExpiredHistorical(uint64,${A},${R},${S},${HP},${MP})`, "0xbe335b84"],
  ["voidFrozen(uint64,uint64)", "0x5b094c00"],
  ["rosterState()", "0x85a00f5c"],
  ["isConsumed(uint64)", "0x95b67034"],
  ["transferNonces(address)", "0xf6f0e8a6"],
  ["tairaNetworkId()", "0x6b91d731"],
  ["routeRevision()", "0x1818891a"],
  ["maxWrappedSupply()", "0xd8bc4dcd"],
  ["mintingPaused()", "0xe1a283d6"],
  ["controlNonce()", "0x4faac8ca"],
  ["domainSeparator()", "0xf698da25"],
  ["initialRosterDigest()", "0x97525bf3"],
  ["initialRosterGeneration()", "0x3ddaa7c6"],
  ["opCount()", "0xcb58065f"],
  ["maxRosterValidityMs()", "0xdeddb507"],
];

const ERC20_FUNCTIONS = [
  ["name()", "0x06fdde03"],
  ["symbol()", "0x95d89b41"],
  ["decimals()", "0x313ce567"],
  ["totalSupply()", "0x18160ddd"],
  ["balanceOf(address)", "0x70a08231"],
  ["transfer(address,uint256)", "0xa9059cbb"],
  ["transferFrom(address,address,uint256)", "0x23b872dd"],
  ["approve(address,uint256)", "0x095ea7b3"],
  ["allowance(address,address)", "0xdd62ed3e"],
];

const SPEC_ERRORS = [
  "WrongChain()",
  "MintingIsPaused()",
  "RosterNotAccepted()",
  "BadRoster()",
  "BadRosterValidity()",
  "BadSignatures()",
  "TooFewSignatures()",
  "BadProof()",
  "BadPayload()",
  "AlreadyConsumed(uint64)",
  "SupplyCapExceeded()",
  "BadRotation()",
  "BadRecipient()",
  "BadAmount()",
  "BadNonce(uint64)",
  "DirectCallerRequired()",
  "NonCanonicalCalldata()",
  "DeadlinePassed()",
  "DeadlineNotReached()",
  "NotFrozen()",
  "StaleControl()",
];

const ERC6093_ERRORS = [
  "ERC20InsufficientBalance(address,uint256,uint256)",
  "ERC20InsufficientAllowance(address,uint256,uint256)",
  "ERC20InvalidSender(address)",
  "ERC20InvalidReceiver(address)",
  "ERC20InvalidSpender(address)",
];

describe("SCCP v1 constants and ABI (§3.6, §3.9, §5.2.2)", () => {
  it("computes every typehash and domain constant from its canonical string", () => {
    const k = (text) => ethers.keccak256(ethers.toUtf8Bytes(text));
    assert.equal(k(m.DOMAIN_TYPE), "0x599a80fcaa47b95e2323ab4d34d34e0cc9feda4b843edafcc30c7bdf60ea15bf");
    assert.equal(k("SCCP"), "0xd7bacbdfe367013f66397ff7242325c31126ab14c95620817c994f22f1d123ba");
    assert.equal(k("1"), "0xc89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc6");
    assert.equal(k(m.ATTESTATION_TYPE), "0x6ea54d1f320a2e5892d6a362adc4b569967facc991d3a327cbc84b746f520c66");
    assert.equal(k(m.KEY_POP_TYPE), "0x87f74eabbf9693141811f707b2d51c6a18546376727b384e155d2539ed0a1a3a");
    assert.equal(N - 1n - (HALF_N << 1n), 0n, "HALF_N = floor(N / 2)");
  });

  it("computes every event topic of §3.9", () => {
    assert.equal(TOPIC.toTaira, "0x79ac1cc63262b80bfbf96e55b2d49d96bd82f018ce0192f7002168724c7d264b");
    assert.equal(TOPIC.finalized, "0x8ab0eb669cb9abcdf0e37e9ce8443162d317d10c2b059296e40aebbd3fa23748");
    assert.equal(TOPIC.voided, "0xfc0fe8523e61c1e6bcbb957a8b692dd0b08e1774c4c4bd897fbc68c047c82fcf");
    assert.equal(TOPIC.rotated, "0x8619e96c15d0af080499408d3d5fb9af0101014daa9acaad772a5aa03ebf315e");
    assert.equal(TOPIC.control, "0x38b98e72aaf30cffd4309ad36378e1587525135a709fe14074156ebb44254d65");
    const { iface } = h.loadArtifacts();
    const events = iface.fragments.filter((fragment) => fragment.type === "event");
    assert.deepEqual(
      events.map((event) => event.topicHash).sort(),
      [...Object.values(TOPIC)].sort(),
      "the ABI declares exactly the SCCP events plus ERC-20 Transfer and Approval",
    );
  });

  it("computes every §5.2.2 selector and matches the compiled ABI exactly", () => {
    const { iface, tron } = h.loadArtifacts();
    for (const [signature, expected] of [...SPEC_FUNCTIONS, ...ERC20_FUNCTIONS]) {
      assert.equal(m.selector(signature), expected, signature);
      const fragment = iface.getFunction(expected);
      assert(fragment, `ABI lacks ${signature}`);
      assert.equal(fragment.format("sighash"), signature);
    }
    const functions = iface.fragments.filter((fragment) => fragment.type === "function");
    assert.deepEqual(
      functions.map((fragment) => fragment.selector).sort(),
      [...SPEC_FUNCTIONS, ...ERC20_FUNCTIONS].map(([, value]) => value).sort(),
      "no function beyond §5.2.2 and ERC-20 exists (no owner, guardian, setter or roster-signed breaker)",
    );
    const selectors = functions.map((fragment) => fragment.selector);
    assert.equal(new Set(selectors).size, selectors.length, "selectors are collision-free");
    assert.deepEqual(tron.abi, h.loadArtifacts().abi, "the TRON build exposes the identical ABI");
  });

  it("declares exactly the §5.2.2 custom errors plus ERC-6093 token errors", () => {
    const { iface } = h.loadArtifacts();
    const errors = iface.fragments.filter((fragment) => fragment.type === "error").map((f) => f.format("sighash"));
    assert.deepEqual(errors.sort(), [...SPEC_ERRORS, ...ERC6093_ERRORS].sort());
    for (const signature of SPEC_ERRORS) {
      assert.equal(iface.getError(signature.split("(")[0]).selector, m.selector(signature));
    }
  });

  it("reproduces the §3.4 control-leaf examples and the promote-odd tree rule", () => {
    const destinationWord = `0x${"00".repeat(12)}${"22".repeat(20)}`;
    const base = {
      networkId: `0x${"11".repeat(32)}`,
      targetTag: 0x41,
      targetIdentity: m.identityWord(PROFILES.ethereum),
      destinationWord,
      revision: 1,
    };
    assert.equal(
      m.controlLeaf({ ...base, nonce: 1, paused: true }),
      "0x93d641053e51b4d28f40930e098212e8b6ab340ceaaf8ad38a458203ce98c662",
    );
    assert.equal(
      m.controlLeaf({ ...base, nonce: 2, paused: false }),
      "0x85fcfc8718c845c212df8ae486161d03bde8116eaa7dd74bbb2312667bce5cdf",
    );
    for (let count = 1; count <= 17; count += 1) {
      const leaves = Array.from({ length: count }, (_, i) => ethers.keccak256(m.be(i, 4)));
      const root = m.merkleRoot(leaves);
      for (let index = 0; index < count; index += 1) {
        const path = m.merklePath(leaves, index);
        assert(path.length <= 5);
        assert.equal(m.merkleRootFromPath(leaves[index], index, count, path), root);
      }
    }
    const history = Array.from({ length: 7 }, (_, i) => m.historyLeaf(i + 1, ethers.keccak256(m.be(i, 4)), 1));
    const peaks = [m.merkleRoot(history.slice(0, 4)), m.merkleRoot(history.slice(4, 6)), history[6]];
    assert.equal(m.merkleRoot(history), m.node(peaks[0], m.node(peaks[1], peaks[2])), "right-bagged MMR");
  });

  it("locks the runtime template and named immutable references", () => {
    const { evm, tron, lock } = h.loadArtifacts();
    const locked = lock.targets.evm.contracts[h.CONTRACT];
    assert.equal(locked.runtime_template_hex, evm.runtime_bytecode.hex);
    assert.deepEqual(locked.immutable_references, evm.runtime_immutable_references);
    const names = (refs) => [...new Set(refs.map((ref) => ref.name))].sort();
    const expected = [
      "DOMAIN_SEPARATOR",
      "INITIAL_GENERATION",
      "INITIAL_ROSTER_DIGEST",
      "MAX_WRAPPED_SUPPLY",
      "NETWORK_TAG",
      "REQUIRE_DIRECT_CALLER",
      "ROUTE_REVISION",
      "TAIRA_NETWORK_ID",
    ];
    assert.deepEqual(names(evm.runtime_immutable_references), expected);
    assert.deepEqual(names(tron.runtime_immutable_references), expected);
    assert.notEqual(tron.runtime_bytecode.hex, evm.runtime_bytecode.hex, "TRON bytecode comes from the TRON compiler");
    assert.equal(lock.targets.tron.contracts[h.CONTRACT].runtime_template_hex, tron.runtime_bytecode.hex);
  });

  it("reproduces both locked builds through the native compiler adapter (§5.5)", () => {
    const { evm, tron } = h.loadArtifacts();
    const outputs = h.recompileWithNativeCompilers();
    for (const [target, artifact] of [["evm", evm], ["tron", tron]]) {
      assert.equal(`0x${outputs[target].evm.bytecode.object}`, artifact.creation_bytecode.hex, `${target} creation`);
      assert.equal(`0x${outputs[target].evm.deployedBytecode.object}`, artifact.runtime_bytecode.hex, `${target} runtime`);
    }
  });
});

// ---------------------------------------------------------------------------
// Per-profile rules
// ---------------------------------------------------------------------------

for (const profile of Object.values(PROFILES)) {
  describe(`SccpTairaXor on ${profile.name} (chain id ${profile.chainId})`, () => {
    it(
      "pins the initial roster immutables with zero counters (§5.2.1)",
      withDestination(profile, {}, async (ctx) => {
        const { dest, initial } = ctx;
        const digest = m.rosterDigest(NETWORK_ID, initial.struct);
        h.recordGas(profile, "deployment (n=4)", ctx.deployReceipt);
        assert.equal(await dest.view("tairaNetworkId"), NETWORK_ID);
        assert.equal(await dest.view("routeRevision"), BigInt(REVISION));
        assert.equal(await dest.view("maxWrappedSupply"), CAP);
        assert.equal(await dest.view("domainSeparator"), m.domainSeparator(NETWORK_ID));
        assert.equal(await dest.view("initialRosterDigest"), digest);
        assert.equal(await dest.view("initialRosterGeneration"), initial.struct.generation);
        assert.equal(await dest.view("maxRosterValidityMs"), m.MAX_ROSTER_VALIDITY_MS);
        assert.equal(await dest.view("opCount"), 0n);
        assert.equal(await dest.view("controlNonce"), 0n);
        assert.equal(await dest.view("mintingPaused"), false);
        assert.deepEqual(await rosterState(ctx), {
          digest,
          generation: initial.struct.generation,
          validUntilMs: initial.struct.validUntilMs,
          prevDigest: ZERO32,
          prevValidUntilMs: 0n,
        });
        assert.equal(await dest.view("name"), "Taira XOR");
        assert.equal(await dest.view("symbol"), "tXOR");
        assert.equal(await dest.view("decimals"), 9n);
        assert.equal(await dest.view("totalSupply"), 0n);
        assert.equal(await dest.view("isConsumed", [0]), false);
        assert.equal(await dest.view("transferNonces", [ctx.recipient]), 0n);
        const code = await ctx.chain.rpc("eth_getCode", [dest.address, "latest"]);
        assert.equal(
          code,
          h.expectedRuntime({
            INITIAL_ROSTER_DIGEST: digest,
            INITIAL_GENERATION: m.word(initial.struct.generation),
            TAIRA_NETWORK_ID: NETWORK_ID,
            NETWORK_TAG: m.word(profile.tag),
            ROUTE_REVISION: m.word(REVISION),
            MAX_WRAPPED_SUPPLY: m.word(CAP),
            DOMAIN_SEPARATOR: m.domainSeparator(NETWORK_ID),
            REQUIRE_DIRECT_CALLER: m.word(profile.directCaller ? 1 : 0),
          }),
          "deployed runtime equals the locked template with the §5.2.3 immutables filled",
        );
        const slots = [];
        for (let slot = 0; slot < 4; slot += 1) {
          slots.push(await ctx.chain.rpc("eth_getStorageAt", [dest.address, ethers.toQuantity(slot), "latest"]));
        }
        assert.equal(slots[0], digest, "slot A: roster digest");
        assert.equal(
          slots[1],
          ethers.zeroPadValue(
            ethers.concat([m.be(0, 1), m.be(0, 8), m.be(initial.struct.validUntilMs, 8), m.be(initial.struct.generation, 8)]),
            32,
          ),
          "slot B: generation, validUntilMs, controlNonce, mintingPaused",
        );
        assert.equal(slots[2], ZERO32, "slot C: previous digest");
        assert.equal(slots[3], ZERO32, "slot D: previous validity and opCount");
      }),
    );

    it("validates chain, identity, revision, cap and the initial roster in the constructor (§5.2.1, §3.7)", async () => {
      const chain = await Chain.open(profile);
      try {
        const now = () => (chain.time + 1n) * 1000n;
        const valid = () => rosterFor("ctor", 1n, 4, now() - 60_000n, now() + 14n * DAY_MS);
        const args = (overrides = {}) => ({ networkId: NETWORK_ID, revision: 1, cap: CAP, roster: valid(), ...overrides });
        const reject = (overrides, error) => expectRevert(h.deploy(chain, args(overrides)), error);
        for (const other of Object.values(PROFILES).filter((p) => p !== profile)) {
          await reject({ tag: other.tag }, "WrongChain");
        }
        await reject({ tag: 0x40 }, "WrongChain");
        await reject({ tag: 0x44 }, "WrongChain");
        await reject({ networkId: ZERO32 }, "WrongChain");
        await reject({ revision: 0 }, "BadPayload");
        await reject({ cap: 0n }, "BadAmount");
        await reject({ cap: 1n << 128n }, "BadAmount");
        await h.deploy(chain, args({ cap: (1n << 128n) - 1n }));

        const withStruct = (edit) => {
          const roster = valid();
          return { ...roster, struct: { ...roster.struct, ...edit(roster.struct) } };
        };
        await reject({ roster: rosterFor("ctor-small", 1n, 3, now() - 1000n, now() + DAY_MS) }, "BadRoster");
        await reject({ roster: rosterFor("ctor-large", 1n, 32, now() - 1000n, now() + DAY_MS) }, "BadRoster");
        await reject({ roster: withStruct((s) => ({ members: ethers.concat([s.members, "0x01"]) })) }, "BadRoster");
        await reject({ roster: withStruct(() => ({ threshold: 4 })) }, "BadRoster");
        await reject({ roster: withStruct(() => ({ threshold: 2 })) }, "BadRoster");
        await reject({ roster: withStruct(() => ({ generation: 0n })) }, "BadRoster");
        const members = (s) => ethers.getBytes(s.members);
        const swap = (bytes, i, j) => {
          const copy = new Uint8Array(bytes);
          copy.set(bytes.slice(j * 20, j * 20 + 20), i * 20);
          copy.set(bytes.slice(i * 20, i * 20 + 20), j * 20);
          return ethers.hexlify(copy);
        };
        await reject({ roster: withStruct((s) => ({ members: swap(members(s), 1, 2) })) }, "BadRoster");
        await reject(
          {
            roster: withStruct((s) => {
              const copy = members(s);
              copy.set(copy.slice(0, 20), 20);
              return { members: ethers.hexlify(copy) };
            }),
          },
          "BadRoster",
        );
        await reject(
          {
            roster: withStruct((s) => {
              const copy = members(s);
              copy.fill(0, 40, 60);
              return { members: ethers.hexlify(copy) };
            }),
          },
          "BadRoster",
        );
        const zeroFirst = rosterFor("ctor-zero", 1n, 7, now() - 1000n, now() + DAY_MS, 2);
        const { destination } = await h.deploy(chain, args({ roster: zeroFirst }));
        assert.equal(await destination.view("initialRosterDigest"), m.rosterDigest(NETWORK_ID, zeroFirst.struct));

        const window = (fromMs, untilMs) => rosterFor("ctor-window", 1n, 4, fromMs, untilMs);
        await reject({ roster: window(now(), now()) }, "BadRosterValidity");
        await reject({ roster: window(now() + DAY_MS, now() + 1000n) }, "BadRosterValidity");
        await reject({ roster: window(now() + m.MAX_CLOCK_SKEW_MS + 1n, now() + DAY_MS) }, "BadRosterValidity");
        await h.deploy(chain, args({ roster: window(now() + m.MAX_CLOCK_SKEW_MS, now() + DAY_MS) }));
        await reject(
          { roster: window(now() - DAY_MS, now() - DAY_MS + m.MAX_ROSTER_VALIDITY_MS + 1n) },
          "BadRosterValidity",
        );
        await reject({ roster: window(now() - DAY_MS, now()) }, "BadRosterValidity");
        await reject(
          { roster: window(now() + HOUR_MS, now() + m.MAX_ROSTER_VALIDITY_MS + 1n) },
          "BadRosterValidity",
        );
        await h.deploy(chain, args({ roster: window(now(), now() + m.MAX_ROSTER_VALIDITY_MS) }));
        await h.deploy(chain, args({ roster: window(now() - 2n * DAY_MS, now() + 1n) }));
      } finally {
        await chain.close();
      }
    });

    it(
      "finalizes a transfer once and emits the §5.2.2 events (§5.1.3)",
      withDestination(profile, {}, async (ctx) => {
        const messages = [0, 1, 2].map((nonce) => transfer(ctx, { nonce, amount: (BigInt(nonce) + 1n) * UNIT }));
        const other = control(ctx, { nonce: 1, paused: true, revision: REVISION + 1 });
        const a = attest(ctx, [messages[0], foreign("other route"), messages[1], other, messages[2]]);
        const message = messages[1];
        const simulated = await ctx.dest.simulate("finalizeFromTaira", [
          a.attestation,
          a.roster,
          a.signatures,
          messageProof(a, message),
        ]);
        assert.equal(simulated, message.id, "finalizeFromTaira returns the message id");
        const receipt = await finalize(ctx, a, message);
        h.recordGas(profile, "finalizeFromTaira n=4 t=3, first mint (new balance, new bitmap word, zero supply)", receipt);
        assertFinalizedLogs(ctx, receipt, message);
        assert.equal(await ctx.dest.view("balanceOf", [ctx.recipient]), message.amount);
        assert.equal(await ctx.dest.view("totalSupply"), message.amount);
        assert.equal(await ctx.dest.view("isConsumed", [message.nonce]), true);
        assert.equal(await ctx.dest.view("isConsumed", [0]), false);
        assert.equal(await ctx.dest.view("opCount"), 1n);
        await expectRevert(finalize(ctx, a, message), "AlreadyConsumed", [message.nonce]);
        const second = await finalize(ctx, a, messages[2]);
        h.recordGas(profile, "finalizeFromTaira n=4 t=3 (existing balance, same bitmap word)", second);
        const third = transfer(ctx, { nonce: 3, recipient: ethers.getAddress(ctx.chain.accounts[6]) });
        const single = attest(ctx, [third]);
        assert.equal(single.attestation.sccpRoot, third.leaf, "a single-leaf root is the leaf");
        h.recordGas(profile, "finalizeFromTaira n=4 t=3 (new balance, same bitmap word)", await finalize(ctx, single, third));
        assert.equal(await ctx.dest.view("opCount"), 3n);
        assert.equal(await ctx.dest.view("totalSupply"), 2n * UNIT + 3n * UNIT + UNIT);
      }),
    );

    it(
      "enforces every §3.2 payload rule for this destination",
      withDestination(profile, {}, async (ctx) => {
        const evm = profile.accountCodec === 2;
        const wrongDomain = Object.values(PROFILES).find((p) => p.domain !== profile.domain).domain;
        const otherRoute = Object.values(PROFILES).find((p) => p.routeId !== profile.routeId).routeId;
        const cases = [
          [{ kind: 1 }, "BadPayload"],
          [{ version: 2 }, "BadPayload"],
          [{ sourceDomain: profile.domain }, "BadPayload"],
          [{ sourceDomain: 4 }, "BadPayload"],
          [{ destDomain: wrongDomain }, "BadPayload"],
          [{ destDomain: 0 }, "BadPayload"],
          [{ routeRevision: REVISION + 1 }, "BadPayload"],
          [{ deadlineMs: 0 }, "BadPayload"],
          [{ assetHomeDomain: 1 }, "BadPayload"],
          [{ assetIdCodec: 2 }, "BadPayload"],
          [{ assetId: m.ascii("XOR") }, "BadPayload"],
          [{ assetId: m.ascii("xorx") }, "BadPayload"],
          [{ amount: 0 }, "BadPayload"],
          [{ senderCodec: 2 }, "BadPayload"],
          [{ sender: "0x" }, "BadPayload"],
          [{ sender: `0x${"ab".repeat(1025)}` }, "BadPayload"],
          [{ senderLen: ethers.dataLength(TAIRA_SENDER) + 1 }, "BadPayload"],
          [{ recipientCodec: evm ? 5 : 2 }, "BadPayload"],
          [{ recipient: evm ? ethers.concat(["0x41", ctx.recipient]) : ctx.recipient }, "BadPayload"],
          [{ recipientLen: evm ? 21 : 20 }, "BadPayload"],
          [{ routeIdCodec: 2 }, "BadPayload"],
          [{ routeId: m.ascii(otherRoute) }, "BadPayload"],
          [{ routeIdLen: profile.routeId.length + 1 }, "BadPayload"],
          [{ trailing: "0x00" }, "BadPayload"],
          [{ trailing: `0x${"00".repeat(4096)}` }, "BadPayload"],
          [{ recipient: m.accountBytes(profile, ZERO_ADDRESS) }, "BadRecipient"],
          [{ recipient: m.accountBytes(profile, ctx.dest.address) }, "BadRecipient"],
        ];
        if (!evm) cases.push([{ recipient: ethers.concat(["0x42", ctx.recipient]) }, "BadRecipient"]);
        const messages = cases.map(([overrides], index) => transfer(ctx, { nonce: 100 + index, overrides }));
        const truncatedBase = transfer(ctx, { nonce: 200 });
        const truncatedPayload = ethers.dataSlice(truncatedBase.payload, 0, ethers.dataLength(truncatedBase.payload) - 1);
        const truncatedId = m.outboundMessageId(NETWORK_ID, profile, truncatedPayload);
        const truncated = {
          payload: truncatedPayload,
          nonce: 200n,
          leaf: m.transferLeaf(truncatedId, m.addressWord(ctx.dest.address)),
        };
        const maxSender = transfer(ctx, { nonce: 300, overrides: { sender: `0x${"cd".repeat(1024)}` } });
        const a = attest(ctx, [...messages, truncated, maxSender]);
        for (let i = 0; i < cases.length; i += 1) {
          await expectRevert(finalize(ctx, a, messages[i]), cases[i][1]);
        }
        await expectRevert(finalize(ctx, a, truncated), "BadPayload");
        await finalize(ctx, a, maxSender);
        assert.equal(await ctx.dest.view("opCount"), 1n);
      }),
    );

    it(
      "binds the Merkle position, count and attestation invariants (§3.4, §3.6.1)",
      withDestination(profile, {}, async (ctx) => {
        const messages = [0, 1, 2, 3, 4].map((nonce) => transfer(ctx, { nonce }));
        const pause = control(ctx, { nonce: 1, paused: true });
        const a = attest(ctx, [...messages, pause]);
        const target = messages[4];
        const proof = messageProof(a, target);
        await expectRevert(finalize(ctx, a, target, {}, { ...proof, leafIndex: 3 }), "BadProof");
        await expectRevert(finalize(ctx, a, target, {}, { ...proof, leafIndex: 6 }), "BadProof");
        await expectRevert(finalize(ctx, a, target, {}, { ...proof, path: [...proof.path, ZERO32] }), "BadProof");
        await expectRevert(finalize(ctx, a, target, {}, { ...proof, path: proof.path.slice(1) }), "BadProof");
        await expectRevert(finalize(ctx, a, target, {}, { ...proof, path: [ZERO32, ...proof.path.slice(1)] }), "BadProof");
        const pauseIndex = leafIndex(a, pause);
        await expectRevert(
          finalize(ctx, a, target, {}, { ...proof, leafIndex: pauseIndex, path: m.merklePath(a.block.leaves, pauseIndex) }),
          "BadProof",
        );
        const recount = attest(ctx, [], { block: a.block, mutate: (s) => ({ messageCount: s.messageCount + 1 }) });
        await expectRevert(finalize(ctx, recount, target, {}, proof), "BadProof");
        const empty = attest(ctx, []);
        await expectRevert(
          finalize(ctx, empty, target, {}, { payload: target.payload, leafIndex: 0, path: [] }),
          "BadProof",
        );
        const invariant = (mutate) => attest(ctx, [], { block: a.block, mutate });
        await expectRevert(finalize(ctx, invariant(() => ({ sccpRoot: ZERO32 })), target, {}, proof), "BadProof");
        await expectRevert(finalize(ctx, invariant(() => ({ messageCount: 0 })), target, {}, proof), "BadProof");
        await expectRevert(finalize(ctx, invariant(() => ({ historyRoot: ZERO32 })), target, {}, proof), "BadProof");
        await expectRevert(finalize(ctx, invariant(() => ({ historySize: 0n })), target, {}, proof), "BadProof");
        await expectRevert(finalize(ctx, invariant(() => ({ messageCount: 513 })), target, {}, proof), "BadProof");
        const receipt = await finalize(ctx, a, target, {}, proof);
        assertFinalizedLogs(ctx, receipt, target);
      }),
    );

    it(
      "rejects malleable, duplicated, unordered and insufficient signatures (§3.8, §9.4)",
      withDestination(profile, { n: 7, zeroSlots: 2 }, async (ctx) => {
        const roster = ctx.current;
        assert.equal(roster.struct.threshold, 5);
        const message = transfer(ctx, { nonce: 9 });
        const a = attest(ctx, [message]);
        const digest = m.attestationDigest(NETWORK_ID, a.attestation);
        const keyed = m.signerIndices(roster, 5);
        const signed = (tamper, indices = keyed) => ({ ...a, signatures: m.signatureSet(roster, digest, indices, tamper) });
        const first = keyed[0];
        const at = (fn) => (index, signature) => (index === first ? fn(signature) : undefined);
        const highS = at((sig) => ({ r: sig.r, s: N - BigInt(sig.s), v: sig.v === 27 ? 28 : 27 }));
        const highSet = signed(highS);
        const highSig = ethers.dataSlice(highSet.signatures.signatures, 0, 65);
        const precompile = await ctx.chain.call(
          "0x0000000000000000000000000000000000000001",
          ethers.concat([digest, m.word(Number(ethers.dataSlice(highSig, 64, 65))), ethers.dataSlice(highSig, 0, 64)]),
        );
        assert.equal(
          precompile,
          m.addressWord(ethers.computeAddress(roster.signers[first].publicKey)).toLowerCase(),
          "the raw ecrecover precompile accepts the high-S twin, so the contract must reject it",
        );
        await expectRevert(finalize(ctx, highSet, message), "BadSignatures");
        for (const v of [0, 1, 26, 29, 30]) {
          await expectRevert(finalize(ctx, signed(at((sig) => ({ ...sig, v }))), message), "BadSignatures");
        }
        await expectRevert(finalize(ctx, signed(at((sig) => ({ ...sig, r: 0n }))), message), "BadSignatures");
        await expectRevert(finalize(ctx, signed(at((sig) => ({ ...sig, s: 0n }))), message), "BadSignatures");
        await expectRevert(finalize(ctx, signed(at((sig) => ({ ...sig, r: N }))), message), "BadSignatures");
        await expectRevert(finalize(ctx, signed(at((sig) => ({ ...sig, s: HALF_N + 1n }))), message), "BadSignatures");
        const duplicate = (index, signature) =>
          index === keyed[1] ? m.signDigest(roster.signers[first], digest) : signature;
        await expectRevert(finalize(ctx, signed(duplicate), message), "BadSignatures");
        const outsider = m.testKey("outsider", 0);
        await expectRevert(
          finalize(ctx, signed(at(() => m.signDigest(outsider, digest))), message),
          "BadSignatures",
        );
        const otherDigest = m.attestationDigest(OTHER_NETWORK_ID, a.attestation);
        await expectRevert(
          finalize(ctx, signed(at(() => m.signDigest(roster.signers[first], otherDigest))), message),
          "BadSignatures",
        );
        await expectRevert(finalize(ctx, signed(null, [0, ...keyed.slice(1)]), message), "BadSignatures");
        await expectRevert(finalize(ctx, signed(null, keyed.slice(0, 4)), message), "TooFewSignatures");
        const extraBit = { ...a, signatures: { ...a.signatures, signerBitmap: a.signatures.signerBitmap | (1n << 7n) } };
        await expectRevert(finalize(ctx, extraBit, message), "BadSignatures");
        const extraByte = { ...a, signatures: { ...a.signatures, signatures: ethers.concat([a.signatures.signatures, "0x00"]) } };
        await expectRevert(finalize(ctx, extraByte, message), "BadSignatures");
        const members = ethers.getBytes(roster.struct.members);
        const unordered = new Uint8Array(members);
        unordered.set(members.slice(3 * 20, 4 * 20), 2 * 20);
        unordered.set(members.slice(2 * 20, 3 * 20), 3 * 20);
        const unorderedRoster = { ...a, roster: { ...a.roster, members: ethers.hexlify(unordered) } };
        await expectRevert(finalize(ctx, unorderedRoster, message), "BadRoster");
        const otherRoster = { ...a, roster: { ...a.roster, validUntilMs: a.roster.validUntilMs + 1n } };
        await expectRevert(finalize(ctx, otherRoster, message), "BadRoster");
        const unknown = attest(ctx, [], { block: a.block, rosterDigest: ethers.keccak256("0x1234") });
        await expectRevert(finalize(ctx, unknown, message), "RosterNotAccepted");
        const zeroDigest = attest(ctx, [], { block: a.block, rosterDigest: ZERO32 });
        await expectRevert(finalize(ctx, zeroDigest, message), "RosterNotAccepted");
        await finalize(ctx, signed(null, m.signerIndices(roster, 5)), message);
        assert.equal(await ctx.dest.view("opCount"), 1n);
      }),
    );

    it(
      "accepts every keyed signer and measures n=31 finalization and controls",
      withDestination(profile, { n: 31 }, async (ctx) => {
        assert.equal(ctx.current.struct.threshold, 21);
        const warm = transfer(ctx, { nonce: 1 });
        await finalize(ctx, attest(ctx, [warm]), warm);
        const message = transfer(ctx, { nonce: 2, recipient: ethers.getAddress(ctx.chain.accounts[6]) });
        const a = attest(ctx, [message]);
        h.recordGas(profile, "finalizeFromTaira n=31 t=21 (new balance, same bitmap word)", await finalize(ctx, a, message));
        const all = transfer(ctx, { nonce: 3, recipient: ethers.getAddress(ctx.chain.accounts[7]) });
        const everyone = attest(ctx, [all], { signers: [...Array(31).keys()] });
        h.recordGas(profile, "finalizeFromTaira n=31, all 31 signatures", await finalize(ctx, everyone, all));
        const pause = control(ctx, { nonce: 1, paused: true });
        const c = attest(ctx, [pause]);
        h.recordGas(profile, "applyControl n=31 t=21", await applyControl(ctx, c, pause));
        const next = successor(ctx, ctx.current, { n: 31 });
        h.recordGas(
          profile,
          "rotateRosters 1 rotation n=31 t=21, first rotation (previous slots zero)",
          await rotate(ctx, [rotation(ctx, ctx.current, next)]),
        );
        const following = successor(ctx, next, { n: 31 });
        h.recordGas(
          profile,
          "rotateRosters 1 rotation n=31 t=21 (steady state)",
          await rotate(ctx, [rotation(ctx, next, following)]),
        );
      }),
    );

    it(
      "mints while now_ms <= deadline_ms and voids only after it (§3.2, §5.1.8)",
      withDestination(profile, {}, async (ctx) => {
        const T = ctx.chain.time + 50n;
        const voidable = transfer(ctx, { nonce: 4, deadlineMs: T * 1000n });
        const exact = transfer(ctx, { nonce: 1, deadlineMs: (T + 1n) * 1000n });
        const late = transfer(ctx, { nonce: 2, deadlineMs: (T + 2n) * 1000n - 1n });
        const within = transfer(ctx, { nonce: 3, deadlineMs: (T + 3n) * 1000n + 999n });
        const a = attest(ctx, [exact, late, within, voidable]);
        await expectRevert(voidExpired(ctx, 4, a, voidable, { at: T }), "DeadlineNotReached");
        await finalize(ctx, a, exact, { at: T + 1n });
        await expectRevert(finalize(ctx, a, late, { at: T + 2n }), "DeadlinePassed");
        await finalize(ctx, a, within, { at: T + 3n });
        await expectRevert(voidExpired(ctx, 3, a, within, { at: T + 4n }), "AlreadyConsumed", [3n]);
        await voidExpired(ctx, 2, a, late, { at: T + 5n });
        const receipt = await voidExpired(ctx, 4, a, voidable, { at: T + 7n });
        const [voided] = ctx.dest.events(receipt, "SccpVoided");
        assertTopics(voided.log, [TOPIC.voided, voidable.id, m.word(4)]);
        assert.equal(voided.log.data, "0x");
      }),
    );

    it(
      "voids expired messages with the mint nonce bit and never mints (§5.1.8)",
      withDestination(profile, { cap: 2n * UNIT }, async (ctx) => {
        const deadlineMs = ctx.chain.nowMs() + HOUR_MS;
        const expiring = transfer(ctx, { nonce: 42, deadlineMs });
        const minted = transfer(ctx, { nonce: 43, deadlineMs, amount: 2n * UNIT });
        const capped = transfer(ctx, { nonce: 44, deadlineMs, amount: 5n * UNIT });
        const a = attest(ctx, [expiring, minted, capped]);
        await finalize(ctx, a, minted);
        await expectRevert(voidExpired(ctx, 42, a, expiring), "DeadlineNotReached");
        const after = ctx.chain.time + HOUR + 1n;
        await expectRevert(voidExpired(ctx, 41, a, expiring, { at: after }), "BadNonce", [42n]);
        const expiredData = ctx.dest.iface.encodeFunctionData("voidExpired", [
          42,
          a.attestation,
          a.roster,
          a.signatures,
          messageProof(a, expiring),
        ]);
        for (const tail of ["0x00", m.word(0)]) {
          await expectRevert(ctx.dest.raw(ethers.concat([expiredData, tail])), "NonCanonicalCalldata");
        }
        const receipt = await voidExpired(ctx, 42, a, expiring);
        h.recordGas(profile, "voidExpired n=4 t=3", receipt);
        const logs = receipt.logs.filter((log) => ethers.getAddress(log.address) === ctx.dest.address);
        assert.equal(logs.length, 1, "a void emits only SccpVoided (no mint)");
        assertTopics(logs[0], [TOPIC.voided, expiring.id, m.word(42)]);
        assert.equal(await ctx.dest.view("isConsumed", [42]), true);
        assert.equal(await ctx.dest.view("totalSupply"), 2n * UNIT);
        assert.equal(await ctx.dest.view("opCount"), 2n);
        await expectRevert(voidExpired(ctx, 42, a, expiring), "AlreadyConsumed", [42n]);
        await expectRevert(voidExpired(ctx, 43, a, minted), "AlreadyConsumed", [43n]);
        await expectRevert(finalize(ctx, a, expiring), "DeadlinePassed");
        await voidExpired(ctx, 44, a, capped);
        assert.equal(await ctx.dest.view("opCount"), 3n, "voids skip the supply cap");
        const stale = transfer(ctx, { nonce: 45, deadlineMs: ctx.chain.nowMs() - 1n });
        const olderBlock = attest(ctx, [stale]);
        const newer = attest(ctx, []);
        const olderHistory = ctx.taira.historyProof(olderBlock.block, newer.attestation.historySize);
        const receiptHistorical = await ctx.dest.tx("voidExpiredHistorical", [
          45,
          newer.attestation,
          newer.roster,
          newer.signatures,
          olderHistory,
          messageProofOf(olderBlock.block, stale),
        ]);
        h.recordGas(profile, "voidExpiredHistorical n=4 t=3", receiptHistorical);
        assert.equal(await ctx.dest.view("isConsumed", [45]), true);
        assert.equal(await ctx.dest.view("opCount"), 4n);
      }),
    );

    it(
      "enforces the immutable supply cap and bitmap word boundaries (§5.1.4, §5.2.3)",
      withDestination(profile, { cap: 3000n }, async (ctx) => {
        const nonces = [255n, 256n, 0n];
        const messages = nonces.map((nonce) => transfer(ctx, { nonce, amount: 1000n }));
        const over = transfer(ctx, { nonce: 511n, amount: 1n });
        const top = transfer(ctx, { nonce: U64_MAX, amount: 1n });
        const a = attest(ctx, [...messages, over, top]);
        for (const message of messages) await finalize(ctx, a, message);
        assert.equal(await ctx.dest.view("totalSupply"), 3000n);
        await expectRevert(finalize(ctx, a, over), "SupplyCapExceeded");
        assert.equal(await ctx.dest.view("isConsumed", [511]), false, "a reverted mint consumes nothing");
        await ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, 1n, 0n], { from: 5 });
        await finalize(ctx, a, over);
        await expectRevert(finalize(ctx, a, top), "SupplyCapExceeded");
        await ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, 1n, 1n], { from: 5 });
        await finalize(ctx, a, top);
        for (const [nonce, consumed] of [
          [0n, true],
          [1n, false],
          [254n, false],
          [255n, true],
          [256n, true],
          [257n, false],
          [510n, false],
          [511n, true],
          [512n, false],
          [U64_MAX - 1n, false],
          [U64_MAX, true],
        ]) {
          assert.equal(await ctx.dest.view("isConsumed", [nonce]), consumed, `nonce ${nonce}`);
        }
        const word0 = ethers.solidityPackedKeccak256(["uint256", "uint256"], [0n, 4n]);
        const bits = BigInt(await ctx.chain.rpc("eth_getStorageAt", [ctx.dest.address, word0, "latest"]));
        assert.equal(bits, (1n << 255n) | 1n, "word 0 holds bits 0 and 255");
        assert.equal(await ctx.dest.view("totalSupply"), 3000n);
      }),
    );

    it(
      "refuses expired rosters and honours the previous-roster grace (§5.1.2, §5.1.5)",
      withDestination(profile, {}, async (ctx) => {
        const g1 = ctx.current;
        const rotateAt = ctx.chain.time + DAY;
        const g2 = successor(ctx, g1, { validFromMs: rotateAt * 1000n - 5000n });
        const pending = transfer(ctx, { nonce: 1 });
        const late = transfer(ctx, { nonce: 2 });
        const byG1 = attest(ctx, [pending, late], { roster: g1 });
        await rotate(ctx, [rotation(ctx, g1, g2)], { at: rotateAt });
        const graceUntil = rotateAt * 1000n + m.PREVIOUS_ROSTER_GRACE_MS;
        const state = await rosterState(ctx);
        assert.equal(state.prevDigest, m.rosterDigest(NETWORK_ID, g1.struct));
        assert.equal(state.prevValidUntilMs, graceUntil, "prevValidUntilMs = min(validUntilMs, now + 24h)");
        assert.equal(state.digest, m.rosterDigest(NETWORK_ID, g2.struct));
        await finalize(ctx, byG1, pending, { at: graceUntil / 1000n });
        await expectRevert(finalize(ctx, byG1, late, { at: graceUntil / 1000n + 1n }), "RosterNotAccepted");
        ctx.current = g2;
        const byG2 = attest(ctx, [late]);
        await finalize(ctx, byG2, late);
      }),
    );

    it(
      "caps the previous-roster grace at the old roster's own validity (§5.1.5 step 4)",
      withDestination(profile, { validityMs: 2n * DAY_MS }, async (ctx) => {
        const g1 = ctx.current;
        const rotateAt = ctx.chain.time + (3n * DAY) / 2n;
        const g2 = successor(ctx, g1, { validFromMs: rotateAt * 1000n - 1000n });
        await rotate(ctx, [rotation(ctx, g1, g2)], { at: rotateAt });
        const state = await rosterState(ctx);
        assert.equal(state.prevValidUntilMs, g1.struct.validUntilMs, "the grace never extends the old validity");
        assert(state.prevValidUntilMs < rotateAt * 1000n + m.PREVIOUS_ROSTER_GRACE_MS);
        const message = transfer(ctx, { nonce: 1 });
        const byG1 = attest(ctx, [message], { roster: g1 });
        await expectRevert(finalize(ctx, byG1, message, { at: g1.struct.validUntilMs / 1000n + 1n }), "RosterNotAccepted");
      }),
    );

    it(
      "freezes minting, voidExpired and controls once the current roster expires (§5.1.5)",
      withDestination(profile, {}, async (ctx) => {
        const expireAt = ctx.initial.struct.validUntilMs / 1000n;
        const deadlineMs = (expireAt + DAY) * 1000n;
        const message = transfer(ctx, { nonce: 7, deadlineMs });
        const again = transfer(ctx, { nonce: 9, deadlineMs });
        const pause = control(ctx, { nonce: 1, paused: true });
        const a = attest(ctx, [message, again, pause]);
        await mintTo(ctx, { nonce: 8, amount: UNIT });
        await finalize(ctx, a, message, { at: expireAt });
        await expectRevert(finalize(ctx, a, again, { at: expireAt + 1n }), "RosterNotAccepted");
        await expectRevert(applyControl(ctx, a, pause), "RosterNotAccepted");
        await expectRevert(voidExpired(ctx, 9, a, again, { at: expireAt + DAY + 1n }), "RosterNotAccepted");
        const g2 = successor(ctx, ctx.current, { validFromMs: ctx.chain.nowMs() });
        await expectRevert(rotate(ctx, [rotation(ctx, ctx.current, g2)]), "RosterNotAccepted");
        await ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, UNIT, 0n], { from: 5 });
        assert.equal(await ctx.dest.view("opCount"), 3n, "burns stay open after expiry");
        assert.equal(await ctx.dest.view("mintingPaused"), false);
      }),
    );

    it(
      "rotates sequential generations in one batch and enforces every §5.1.5 bound",
      withDestination(profile, {}, async (ctx) => {
        const g1 = ctx.current;
        const at = ctx.chain.time + HOUR;
        const nowMs = at * 1000n;
        const chainOf = [g1];
        const firstNegativeAt = at - 100n;
        for (let i = 0; i < 3; i += 1) {
          chainOf.push(successor(ctx, chainOf[i], { validFromMs: nowMs - 60_000n + BigInt(i) }));
        }
        const batch = [0, 1, 2].map((i) => rotation(ctx, chainOf[i], chainOf[i + 1]));

        const single = (next, options) => [rotation(ctx, g1, next, options)];
        const g2 = successor(ctx, g1, { validFromMs: nowMs - 60_000n });
        await expectRevert(rotate(ctx, [], { at: firstNegativeAt }), "BadRotation");
        await expectRevert(rotate(ctx, Array(17).fill(batch[0])), "BadRotation");
        await expectRevert(rotate(ctx, [rotation(ctx, chainOf[1], chainOf[2])]), "RosterNotAccepted");
        await expectRevert(rotate(ctx, single(g2, { nextDigest: ZERO32 })), "BadRotation");
        await expectRevert(
          rotate(ctx, single(g2, { nextDigest: m.rosterDigest(NETWORK_ID, chainOf[2].struct) })),
          "BadRotation",
        );
        await expectRevert(
          rotate(ctx, single(successor(ctx, g1, { generation: g1.struct.generation + 2n, validFromMs: nowMs - 1n }))),
          "BadRotation",
        );
        await expectRevert(
          rotate(ctx, single(successor(ctx, g1, { generation: g1.struct.generation, validFromMs: nowMs - 1n }))),
          "BadRotation",
        );
        await expectRevert(rotate(ctx, single(g2, { timestampMs: g2.struct.validFromMs + 1n })), "BadRotation");
        const badThreshold = { ...g2, struct: { ...g2.struct, threshold: 4 } };
        await expectRevert(
          rotate(ctx, single(g2, { suppliedNext: badThreshold, nextDigest: m.rosterDigest(NETWORK_ID, badThreshold.struct) })),
          "BadRoster",
        );
        await expectRevert(rotate(ctx, single(g2, { signers: m.signerIndices(g1, 2) })), "TooFewSignatures");
        await expectRevert(rotate(ctx, [batch[0], batch[2]]), "RosterNotAccepted");
        await expectRevert(rotate(ctx, [batch[0], rotation(ctx, chainOf[1], chainOf[1])]), "BadRotation");

        const bound = (validFromMs, validUntilMs) =>
          single(rosterFor("bound", g1.struct.generation + 1n, 4, validFromMs, validUntilMs));
        const nextAt = () => ctx.chain.time + 1n;
        let now = nextAt() * 1000n;
        await expectRevert(rotate(ctx, bound(now + m.MAX_CLOCK_SKEW_MS + 1n, now + DAY_MS)), "BadRosterValidity");
        now = nextAt() * 1000n;
        await expectRevert(
          rotate(ctx, bound(now - 1000n, now - 1000n + m.MAX_ROSTER_VALIDITY_MS + 1n)),
          "BadRosterValidity",
        );
        now = nextAt() * 1000n;
        await expectRevert(rotate(ctx, bound(now - 2n * HOUR_MS, now)), "BadRosterValidity");
        now = nextAt() * 1000n;
        await expectRevert(
          rotate(ctx, bound(now + HOUR_MS / 2n, now + m.MAX_ROSTER_VALIDITY_MS + 1n)),
          "BadRosterValidity",
        );
        now = nextAt() * 1000n;
        await expectRevert(rotate(ctx, bound(now, now)), "BadRosterValidity");
        assert.deepEqual((await rosterState(ctx)).digest, m.rosterDigest(NETWORK_ID, g1.struct), "reverts change nothing");

        const pause = control(ctx, { nonce: 1, paused: true });
        await applyControl(ctx, attest(ctx, [pause]), pause);
        const receipt = await rotate(ctx, batch, { at });
        h.recordGas(profile, "rotateRosters 3 rotations n=4 t=3", receipt);
        const rotated = ctx.dest.events(receipt, "SccpRosterRotated");
        assert.equal(rotated.length, 3);
        rotated.forEach((event, i) => {
          const next = chainOf[i + 1];
          assertTopics(event.log, [TOPIC.rotated, m.word(next.struct.generation)]);
          assert.equal(
            event.log.data,
            abi.encode(["bytes32", "uint64"], [m.rosterDigest(NETWORK_ID, next.struct), next.struct.validUntilMs]),
          );
        });
        const state = await rosterState(ctx);
        assert.equal(state.digest, m.rosterDigest(NETWORK_ID, chainOf[3].struct));
        assert.equal(state.generation, g1.struct.generation + 3n);
        assert.equal(state.validUntilMs, chainOf[3].struct.validUntilMs);
        assert.equal(state.prevDigest, m.rosterDigest(NETWORK_ID, chainOf[2].struct));
        assert.equal(state.prevValidUntilMs, nowMs + m.PREVIOUS_ROSTER_GRACE_MS);
        assert.equal(await ctx.dest.view("opCount"), 1n, "rotations do not count, the applied control does");
        assert.equal(await ctx.dest.view("initialRosterDigest"), m.rosterDigest(NETWORK_ID, g1.struct));

        ctx.current = chainOf[3];
        const edge = ctx.chain.time + 1n;
        const edgeNext = rosterFor(
          "edge",
          ctx.current.struct.generation + 1n,
          4,
          edge * 1000n + m.MAX_CLOCK_SKEW_MS,
          edge * 1000n + m.MAX_ROSTER_VALIDITY_MS,
        );
        h.recordGas(
          profile,
          "rotateRosters 1 rotation n=4 t=3 (steady state)",
          await rotate(ctx, [rotation(ctx, ctx.current, edgeNext)], { at: edge }),
        );
        ctx.current = edgeNext;
        const longBatch = [];
        let from = edgeNext;
        const batchAt = ctx.chain.time + 1n;
        for (let i = 0; i < 16; i += 1) {
          const next = successor(ctx, from, { validFromMs: batchAt * 1000n - 1000n });
          longBatch.push(rotation(ctx, from, next));
          from = next;
        }
        h.recordGas(profile, "rotateRosters 16 rotations n=4 t=3", await rotate(ctx, longBatch, { at: batchAt }));
        assert.equal((await rosterState(ctx)).generation, edgeNext.struct.generation + 16n);
      }),
    );

    it(
      "applies Parliament controls with strictly increasing nonces (§5.1.6)",
      withDestination(profile, {}, async (ctx) => {
        const minted = await mintTo(ctx, { nonce: 1, amount: 10n * UNIT });
        assert(minted.receipt);
        const waiting = transfer(ctx, { nonce: 2 });
        const expiring = transfer(ctx, { nonce: 3, deadlineMs: ctx.chain.nowMs() + 60_000n });
        const pause = control(ctx, { nonce: 1, paused: true });
        const a = attest(ctx, [waiting, pause, expiring]);
        const receipt = await applyControl(ctx, a, pause);
        h.recordGas(profile, "applyControl n=4 t=3", receipt);
        const [applied] = ctx.dest.events(receipt, "SccpControlApplied");
        assertTopics(applied.log, [TOPIC.control, m.word(1)]);
        assert.equal(applied.log.data, m.word(1));
        assert.equal(await ctx.dest.view("mintingPaused"), true);
        assert.equal(await ctx.dest.view("controlNonce"), 1n);
        assert.equal(await ctx.dest.view("opCount"), 2n);
        await expectRevert(finalize(ctx, a, waiting), "MintingIsPaused");
        const history = ctx.taira.historyProof(a.block, ctx.taira.history.length);
        const later = attest(ctx, []);
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", [
            later.attestation,
            later.roster,
            later.signatures,
            history,
            messageProofOf(a.block, waiting),
          ]),
          "MintingIsPaused",
        );
        await ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, UNIT, 0n], { from: 5 });
        await voidExpired(ctx, 3, a, expiring, { at: ctx.chain.time + 120n });
        const g2 = successor(ctx, ctx.current);
        await rotate(ctx, [rotation(ctx, ctx.current, g2)]);
        assert.equal(await ctx.dest.view("opCount"), 4n, "burn and void count while paused; rotation does not");

        await expectRevert(applyControl(ctx, a, pause), "StaleControl");
        const equal = control(ctx, { nonce: 1, paused: false });
        const resumeGap = control(ctx, { nonce: 3, paused: false });
        const skipped = control(ctx, { nonce: 2, paused: true });
        const b = attest(ctx, [equal, skipped, resumeGap], { roster: ctx.initial });
        await expectRevert(applyControl(ctx, b, equal), "StaleControl");
        await applyControl(ctx, b, resumeGap);
        assert.equal(await ctx.dest.view("mintingPaused"), false);
        assert.equal(await ctx.dest.view("controlNonce"), 3n, "gaps are allowed");
        await expectRevert(applyControl(ctx, b, skipped), "StaleControl");
        await expectRevert(applyControl(ctx, b, resumeGap), "StaleControl");
        ctx.current = g2;
        await finalize(ctx, attest(ctx, [waiting]), waiting);
        assert.equal(await ctx.dest.view("opCount"), 6n);
      }),
    );

    it(
      "binds control leaves to the Taira network, target, destination and revision (§3.4, §9.2)",
      withDestination(profile, {}, async (ctx) => {
        const otherProfile = Object.values(PROFILES).find((p) => p !== profile);
        const stranger = ethers.getAddress(ctx.chain.accounts[9]);
        const cases = [
          control(ctx, { nonce: 10, paused: true, networkId: OTHER_NETWORK_ID }),
          control(ctx, { nonce: 10, paused: true, profile: otherProfile }),
          control(ctx, { nonce: 10, paused: true, destination: stranger }),
          control(ctx, { nonce: 10, paused: true, revision: REVISION + 1 }),
        ];
        const message = transfer(ctx, { nonce: 5 });
        const genuine = control(ctx, { nonce: 10, paused: true });
        const a = attest(ctx, [...cases, message, genuine]);
        for (const item of cases) {
          await expectRevert(applyControl(ctx, a, item, {}, controlProof(a, item, { controlNonce: 10n, paused: true })), "BadProof");
        }
        const messageIndex = leafIndex(a, message);
        await expectRevert(
          applyControl(ctx, a, genuine, {}, {
            controlNonce: 10n,
            paused: true,
            leafIndex: messageIndex,
            path: m.merklePath(a.block.leaves, messageIndex),
          }),
          "BadProof",
        );
        await expectRevert(applyControl(ctx, a, genuine, {}, controlProof(a, genuine, { paused: false })), "BadProof");
        await expectRevert(applyControl(ctx, a, genuine, {}, controlProof(a, genuine, { controlNonce: 11n })), "BadProof");
        const empty = attest(ctx, []);
        await expectRevert(
          applyControl(ctx, empty, genuine, {}, { controlNonce: 10n, paused: true, leafIndex: 0, path: [] }),
          "BadProof",
        );
        await applyControl(ctx, a, genuine);
        assert.equal(await ctx.dest.view("controlNonce"), 10n);
        assert.equal(await ctx.dest.view("mintingPaused"), true);
      }),
    );

    it(
      "proves transfers and controls of older blocks through the history root (§3.5, §5.1.3, §5.1.6)",
      withDestination(profile, {}, async (ctx) => {
        const blocks = [];
        const messages = [];
        for (let i = 0; i < 5; i += 1) {
          const message = transfer(ctx, { nonce: 20 + i, recipient: ethers.getAddress(ctx.chain.accounts[10 + i]) });
          messages.push(message);
          blocks.push(attest(ctx, [foreign(`noise ${i}`), message]).block);
          if (i === 1) attest(ctx, []);
        }
        const pause = control(ctx, { nonce: 4, paused: true });
        const controlBlock = attest(ctx, [pause]).block;
        const heartbeat = attest(ctx, []);
        const size = heartbeat.attestation.historySize;
        assert.equal(size, 6n);
        const target = blocks[1];
        const history = ctx.taira.historyProof(target, size);
        const args = (hist, proof = messageProofOf(target, messages[1])) => [
          heartbeat.attestation,
          heartbeat.roster,
          heartbeat.signatures,
          hist,
          proof,
        ];
        await expectRevert(ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, leafIndex: 2n })), "BadProof");
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, messageCount: 3 })),
          "BadProof",
        );
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, messageCount: 0, sccpRoot: ZERO32 })),
          "BadProof",
        );
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, sccpRoot: blocks[2].sccpRoot })),
          "BadProof",
        );
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, height: history.height + 1n })),
          "BadProof",
        );
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, path: Array(33).fill(ZERO32) })),
          "BadProof",
        );
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", args({ ...history, path: [...history.path, ZERO32] })),
          "BadProof",
        );
        const noHistory = attest(ctx, [], {
          block: heartbeat.block,
          mutate: () => ({ historySize: 0n, historyRoot: ZERO32 }),
        });
        await expectRevert(
          ctx.dest.tx("finalizeFromTairaHistorical", [
            noHistory.attestation,
            noHistory.roster,
            noHistory.signatures,
            history,
            messageProofOf(target, messages[1]),
          ]),
          "BadProof",
        );
        const direct = attest(ctx, [], { block: blocks[3] });
        await finalize(ctx, direct, messages[3], {}, messageProofOf(blocks[3], messages[3]));
        const receipt = await ctx.dest.tx("finalizeFromTairaHistorical", args(history));
        h.recordGas(profile, "finalizeFromTairaHistorical n=4 t=3, history size 6 (new balance, same bitmap word)", receipt);
        assertFinalizedLogs(ctx, receipt, messages[1]);
        const controlHistory = ctx.taira.historyProof(controlBlock, size);
        const controlArgs = [
          heartbeat.attestation,
          heartbeat.roster,
          heartbeat.signatures,
          controlHistory,
          { controlNonce: 4n, paused: true, leafIndex: 0, path: [] },
        ];
        await expectRevert(
          ctx.dest.tx("applyControlHistorical", [...controlArgs.slice(0, 3), history, controlArgs[4]]),
          "BadProof",
        );
        const applied = await ctx.dest.tx("applyControlHistorical", controlArgs);
        h.recordGas(profile, "applyControlHistorical n=4 t=3 (history size 6)", applied);
        assert.equal(await ctx.dest.view("controlNonce"), 4n);
        assert.equal(await ctx.dest.view("mintingPaused"), true);
        await expectRevert(ctx.dest.tx("applyControlHistorical", controlArgs), "StaleControl");
        assert.equal(await ctx.dest.view("opCount"), 3n);
      }),
    );

    it(
      "bounds the attested history at 2^32 blocks (§3.5, §5.1.3)",
      withDestination(profile, {}, async (ctx) => {
        const messages = [transfer(ctx, { nonce: 70 }), transfer(ctx, { nonce: 71 })];
        const blocks = messages.map((message) => attest(ctx, [message]).block);
        const heartbeat = attest(ctx, []);
        const leafOf = (block) => m.historyLeaf(block.height, block.sccpRoot, block.messageCount);
        const sibling = ethers.keccak256("0x5a");
        const limit = 1n << 32n;
        // At 2^32 leaves the last leaf has 32 siblings; at 2^32 + 1 the last leaf is promoted
        // 32 times and needs one sibling, so only the size bound refuses that statement.
        const full = { size: limit, index: limit - 1n, path: Array(32).fill(sibling), block: blocks[0] };
        const over = { size: limit + 1n, index: limit, path: [sibling], block: blocks[1] };
        const historical = (shape, message) => {
          const root = m.merkleRootFromPath(leafOf(shape.block), shape.index, shape.size, shape.path);
          const a = attest(ctx, [], {
            block: heartbeat.block,
            mutate: () => ({ historySize: shape.size, historyRoot: root }),
          });
          const history = {
            height: shape.block.height,
            sccpRoot: shape.block.sccpRoot,
            messageCount: shape.block.messageCount,
            leafIndex: shape.index,
            path: shape.path,
          };
          return ctx.dest.tx("finalizeFromTairaHistorical", [
            a.attestation,
            a.roster,
            a.signatures,
            history,
            messageProofOf(shape.block, message),
          ]);
        };
        await expectRevert(historical(over, messages[1]), "BadProof");
        assert.equal(await ctx.dest.view("isConsumed", [71]), false);
        await historical(full, messages[0]);
        assert.equal(await ctx.dest.view("isConsumed", [70]), true);
      }),
    );

    it(
      "burns with per-sender nonces and emits the canonical inbound payload (§5.1.7)",
      withDestination(profile, {}, async (ctx) => {
        await mintTo(ctx, { nonce: 1, amount: 100n * UNIT });
        const sender = ctx.recipient;
        const amount = 7n * UNIT;
        const expected = m.inboundPayload(profile, {
          nonce: 0,
          revision: REVISION,
          amount,
          sender,
          tairaRecipient: TAIRA_RECIPIENT,
        });
        const expectedId = m.inboundMessageId(NETWORK_ID, profile, expected);
        assert.equal(await ctx.dest.simulate("transferToTaira", [TAIRA_RECIPIENT, amount, 0n], 5), expectedId);
        const receipt = await ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, amount, 0n], { from: 5 });
        h.recordGas(profile, "transferToTaira (34-byte Taira recipient)", receipt);
        const logs = receipt.logs.filter((log) => ethers.getAddress(log.address) === ctx.dest.address);
        assert.equal(logs.length, 2);
        assertTopics(logs[0], [TOPIC.transfer, m.addressWord(sender), m.addressWord(ZERO_ADDRESS)]);
        assert.equal(logs[0].data, m.word(amount));
        assertTopics(logs[1], [TOPIC.toTaira, expectedId, m.addressWord(sender)]);
        assert.equal(logs[1].data, abi.encode(["uint64", "bytes"], [0n, expected]));
        assert.equal(await ctx.dest.view("transferNonces", [sender]), 1n);
        assert.equal(await ctx.dest.view("balanceOf", [sender]), 93n * UNIT);
        assert.equal(await ctx.dest.view("totalSupply"), 93n * UNIT);
        assert.equal(await ctx.dest.view("opCount"), 2n);

        await expectRevert(ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, UNIT, 0n], { from: 5 }), "BadNonce", [1n]);
        await expectRevert(ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, UNIT, 5n], { from: 5 }), "BadNonce", [1n]);
        await expectRevert(ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, 0n, 1n], { from: 5 }), "BadAmount");
        await expectRevert(ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, 1n << 128n, 1n], { from: 5 }), "BadAmount");
        await expectRevert(
          ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, 94n * UNIT, 1n], { from: 5 }),
          "ERC20InsufficientBalance",
          [sender, 93n * UNIT, 94n * UNIT],
        );
        await expectRevert(ctx.dest.tx("transferToTaira", ["0x", UNIT, 1n], { from: 5 }), "NonCanonicalCalldata");
        await expectRevert(
          ctx.dest.tx("transferToTaira", [`0x${"ee".repeat(1025)}`, UNIT, 1n], { from: 5 }),
          "NonCanonicalCalldata",
        );
        let nonce = 1n;
        for (const recipient of ["0x07", `0x${"ee".repeat(32)}`, `0x${"ee".repeat(1024)}`]) {
          const payload = m.inboundPayload(profile, { nonce, revision: REVISION, amount: 1n, sender, tairaRecipient: recipient });
          const burn = await ctx.dest.tx("transferToTaira", [recipient, 1n, nonce], { from: 5 });
          const [event] = ctx.dest.events(burn, "SccpTransferToTaira");
          assert.equal(event.log.data, abi.encode(["uint64", "bytes"], [nonce, payload]));
          assert.equal(event.log.topics[1], m.inboundMessageId(NETWORK_ID, profile, payload));
          nonce += 1n;
        }
        assert.equal(await ctx.dest.view("transferNonces", [sender]), nonce);
        const other = ethers.getAddress(ctx.chain.accounts[6]);
        assert.equal(await ctx.dest.view("transferNonces", [other]), 0n, "nonces are per sender");
      }),
    );

    it(
      "accepts only canonical transferToTaira calldata (§5.1.7)",
      withDestination(profile, {}, async (ctx) => {
        await mintTo(ctx, { nonce: 1, amount: 100n * UNIT });
        const recipient = "0x0102030405";
        const canonical = transferCalldata(recipient, UNIT, 0n);
        assert.equal(ethers.dataLength(canonical), 4 + 0x80 + 32);
        const sel = ethers.dataSlice(canonical, 0, 4);
        const head = (offset) => ethers.concat([sel, m.word(offset), m.word(UNIT), m.word(0)]);
        const body = ethers.concat([m.word(5), padRight(recipient)]);
        const send = (data) => ctx.dest.raw(data, { from: 5 });
        await expectRevert(send(ethers.concat([head(0x80), m.word(0), body])), "NonCanonicalCalldata");
        await expectRevert(send(ethers.concat([canonical, "0x00"])), "NonCanonicalCalldata");
        await expectRevert(send(ethers.concat([canonical, m.word(0)])), "NonCanonicalCalldata");
        const dirtyPad = ethers.getBytes(canonical);
        dirtyPad[dirtyPad.length - 1] = 1;
        await expectRevert(send(ethers.hexlify(dirtyPad)), "NonCanonicalCalldata");
        await expectRevert(send(ethers.concat([head(0x60), m.word(5), recipient])), "NonCanonicalCalldata");
        const lying = ethers.concat([head(0x60), m.word(4), padRight(recipient)]);
        await expectRevert(send(lying), "NonCanonicalCalldata");
        const dirtyNonce = ethers.concat([sel, m.word(0x60), m.word(UNIT), m.word(1n << 64n), body]);
        await expectRevert(send(dirtyNonce), null);
        await send(canonical);
        assert.equal(await ctx.dest.view("transferNonces", [ctx.recipient]), 1n);
        const full = transferCalldata(`0x${"11".repeat(32)}`, UNIT, 1n);
        assert.equal(ethers.dataLength(full), 4 + 0x80 + 32);
        await send(full);
      }),
    );

    it(
      `${profile.directCaller ? "requires" : "does not require"} a direct caller for burns and voids (§5.1.7, §5.2.5)`,
      withDestination(profile, {}, async (ctx) => {
        const forwarder = await h.deployForwarder(ctx.chain, ctx.dest.address);
        await mintTo(ctx, { nonce: 1, amount: 10n * UNIT, recipient: forwarder });
        const burn = transferCalldata(TAIRA_RECIPIENT, UNIT, 0n);
        const deadlineMs = ctx.chain.nowMs() + 30_000n;
        const expiring = transfer(ctx, { nonce: 2, deadlineMs });
        const a = attest(ctx, [expiring]);
        const voidData = ctx.dest.iface.encodeFunctionData("voidExpired", [
          2,
          a.attestation,
          a.roster,
          a.signatures,
          messageProof(a, expiring),
        ]);
        const frozenData = ctx.dest.iface.encodeFunctionData("voidFrozen", [1000, 1]);
        const viaForwarder = (data, at) => ctx.chain.send({ to: forwarder, data, at });
        if (profile.directCaller) {
          await expectRevert(viaForwarder(burn), "DirectCallerRequired");
          await expectRevert(viaForwarder(voidData, ctx.chain.time + 60n), "DirectCallerRequired");
          await expectRevert(
            viaForwarder(frozenData, ctx.initial.struct.validUntilMs / 1000n + 1n),
            "DirectCallerRequired",
          );
          assert.equal(await ctx.dest.view("opCount"), 1n);
        } else {
          const receipt = await viaForwarder(burn);
          const [event] = ctx.dest.events(receipt, "SccpTransferToTaira");
          assert.equal(event.log.topics[2], m.addressWord(forwarder).toLowerCase(), "the forwarder is the sender");
          await viaForwarder(voidData, ctx.chain.time + 60n);
          await viaForwarder(frozenData, ctx.initial.struct.validUntilMs / 1000n + 1n);
          assert.equal(await ctx.dest.view("opCount"), 4n);
        }
      }),
    );

    it(
      "voids frozen ranges only after both rosters expire (§5.1.8)",
      withDestination(profile, {}, async (ctx) => {
        const g1 = ctx.current;
        await expectRevert(ctx.dest.tx("voidFrozen", [0, 1]), "NotFrozen");
        const rotateAt = ctx.chain.time + DAY;
        const g2 = rosterFor("frozen", g1.struct.generation + 1n, 4, rotateAt * 1000n - 1000n, rotateAt * 1000n + HOUR_MS);
        await rotate(ctx, [rotation(ctx, g1, g2)], { at: rotateAt });
        ctx.current = g2;
        await mintTo(ctx, { nonce: 300, amount: UNIT });
        const graceUntil = rotateAt * 1000n + m.PREVIOUS_ROSTER_GRACE_MS;
        await expectRevert(ctx.dest.tx("voidFrozen", [0, 1], { at: rotateAt + 2n * HOUR }), "NotFrozen");
        const early = transfer(ctx, { nonce: 301 });
        const byG1 = attest(ctx, [early], { roster: g1, timestampMs: rotateAt * 1000n });
        await finalize(ctx, byG1, early);
        await expectRevert(ctx.dest.tx("voidFrozen", [0, 1], { at: graceUntil / 1000n }), "NotFrozen");
        const frozen = graceUntil / 1000n + 1n;
        await expectRevert(ctx.dest.tx("voidFrozen", [0, 0], { at: frozen }), "BadAmount");
        await expectRevert(ctx.dest.tx("voidFrozen", [0, 257]), "BadAmount");
        await expectRevert(ctx.dest.tx("voidFrozen", [U64_MAX, 2]), "BadAmount");
        const frozenData = ctx.dest.iface.encodeFunctionData("voidFrozen", [0, 1]);
        for (const tail of ["0x00", m.word(0)]) {
          await expectRevert(ctx.dest.raw(ethers.concat([frozenData, tail])), "NonCanonicalCalldata");
        }
        await expectRevert(ctx.dest.raw(ethers.dataSlice(frozenData, 0, 67)), null);
        await expectRevert(ctx.dest.tx("voidFrozen", [290, 20]), "AlreadyConsumed", [300n]);
        assert.equal(await ctx.dest.view("isConsumed", [290]), false, "a reverted range consumes nothing");
        const opBefore = await ctx.dest.view("opCount");
        const receipt = await ctx.dest.tx("voidFrozen", [250, 10]);
        const voided = ctx.dest.events(receipt, "SccpVoided");
        assert.deepEqual(
          voided.map((event) => event.log.topics),
          Array.from({ length: 10 }, (_, i) => [TOPIC.voided, ZERO32, m.word(250 + i)]),
        );
        for (const [nonce, consumed] of [[249, false], [250, true], [255, true], [256, true], [259, true], [260, false]]) {
          assert.equal(await ctx.dest.view("isConsumed", [nonce]), consumed, `nonce ${nonce}`);
        }
        assert.equal(await ctx.dest.view("opCount"), opBefore + 1n, "one void operation per call");
        h.recordGas(profile, "voidFrozen 1 nonce", await ctx.dest.tx("voidFrozen", [5, 1]));
        h.recordGas(profile, "voidFrozen 256 nonces (one bitmap word)", await ctx.dest.tx("voidFrozen", [512, 256]));
        h.recordGas(profile, "voidFrozen 256 nonces (two bitmap words)", await ctx.dest.tx("voidFrozen", [1000, 256]));
        await ctx.dest.tx("voidFrozen", [U64_MAX - 255n, 256]);
        assert.equal(await ctx.dest.view("isConsumed", [U64_MAX]), true);
        await expectRevert(ctx.dest.tx("voidFrozen", [300, 1]), "AlreadyConsumed", [300n]);
        const message = transfer(ctx, { nonce: 5000 });
        await expectRevert(finalize(ctx, attest(ctx, [message]), message), "RosterNotAccepted");
        await ctx.dest.tx("transferToTaira", [TAIRA_RECIPIENT, UNIT, 0n], { from: 5 });
      }),
    );

    it(
      "implements the ERC-20 surface with ERC-6093 errors",
      withDestination(profile, {}, async (ctx) => {
        await mintTo(ctx, { nonce: 1, amount: 10n * UNIT });
        const [owner, spender, other] = [5, 6, 7].map((i) => ethers.getAddress(ctx.chain.accounts[i]));
        const sent = await ctx.dest.tx("transfer", [other, UNIT], { from: 5 });
        assertTopics(sent.logs[0], [TOPIC.transfer, m.addressWord(owner), m.addressWord(other)]);
        await expectRevert(ctx.dest.tx("transfer", [ZERO_ADDRESS, 1n], { from: 5 }), "ERC20InvalidReceiver", [ZERO_ADDRESS]);
        await expectRevert(
          ctx.dest.tx("transfer", [ctx.dest.address, 1n], { from: 5 }),
          "ERC20InvalidReceiver",
          [ctx.dest.address],
        );
        await expectRevert(
          ctx.dest.tx("transfer", [other, 10n * UNIT], { from: 5 }),
          "ERC20InsufficientBalance",
          [owner, 9n * UNIT, 10n * UNIT],
        );
        await expectRevert(ctx.dest.tx("approve", [ZERO_ADDRESS, 1n], { from: 5 }), "ERC20InvalidSpender", [ZERO_ADDRESS]);
        const approved = await ctx.dest.tx("approve", [spender, 2n * UNIT], { from: 5 });
        assertTopics(approved.logs[0], [TOPIC.approval, m.addressWord(owner), m.addressWord(spender)]);
        await expectRevert(
          ctx.dest.tx("transferFrom", [owner, other, 3n * UNIT], { from: 6 }),
          "ERC20InsufficientAllowance",
          [spender, 2n * UNIT, 3n * UNIT],
        );
        await ctx.dest.tx("transferFrom", [owner, other, UNIT], { from: 6 });
        assert.equal(await ctx.dest.view("allowance", [owner, spender]), UNIT);
        await ctx.dest.tx("approve", [spender, ethers.MaxUint256], { from: 5 });
        await ctx.dest.tx("transferFrom", [owner, other, UNIT], { from: 6 });
        assert.equal(await ctx.dest.view("allowance", [owner, spender]), ethers.MaxUint256);
        await expectRevert(
          ctx.dest.tx("transferFrom", [ZERO_ADDRESS, other, 0n], { from: 6 }),
          "ERC20InvalidSender",
          [ZERO_ADDRESS],
        );
        assert.equal(await ctx.dest.view("balanceOf", [owner]), 7n * UNIT);
        assert.equal(await ctx.dest.view("balanceOf", [other]), 3n * UNIT);
        assert.equal(await ctx.dest.view("totalSupply"), 10n * UNIT);
        assert.equal(await ctx.dest.view("opCount"), 1n, "ERC-20 moves are not SCCP operations");
      }),
    );

    it(
      "refuses every entry point on a foreign chain id (§5.1.3 step 1)",
      withDestination(profile, {}, async (ctx) => {
        const foreignProfile = Object.values(PROFILES).find((p) => p !== profile);
        const foreignChain = await Chain.open(foreignProfile);
        try {
          const code = await ctx.chain.rpc("eth_getCode", [ctx.dest.address, "latest"]);
          await foreignChain.rpc("hardhat_setCode", [ctx.dest.address, code]);
          for (let slot = 0; slot < 4; slot += 1) {
            const value = await ctx.chain.rpc("eth_getStorageAt", [ctx.dest.address, ethers.toQuantity(slot), "latest"]);
            await foreignChain.rpc("hardhat_setStorageAt", [ctx.dest.address, ethers.toQuantity(slot), value]);
          }
          const copy = new h.Destination(foreignChain, ctx.dest.address);
          assert.equal((await copy.view("rosterState"))[0], m.rosterDigest(NETWORK_ID, ctx.initial.struct));
          const message = transfer(ctx, { nonce: 1 });
          const pause = control(ctx, { nonce: 1, paused: true });
          const a = attest(ctx, [message, pause]);
          const next = successor(ctx, ctx.current);
          const calls = [
            ["finalizeFromTaira", [a.attestation, a.roster, a.signatures, messageProof(a, message)]],
            ["applyControl", [a.attestation, a.roster, a.signatures, controlProof(a, pause)]],
            ["rotateRosters", [[rotation(ctx, ctx.current, next)]]],
            ["transferToTaira", [TAIRA_RECIPIENT, UNIT, 0n]],
            ["voidExpired", [1, a.attestation, a.roster, a.signatures, messageProof(a, message)]],
            ["voidFrozen", [0, 1]],
          ];
          for (const [name, args] of calls) {
            await expectRevert(copy.tx(name, args), "WrongChain");
          }
          await finalize(ctx, a, message);
        } finally {
          await foreignChain.close();
        }
      }),
    );
  });
}

after(() => {
  const report = h.gasReport();
  if (h.GAS.size) console.log(`\nMeasured gas (EDR, Osaka rules):\n${report}`);
  h.writeGasReport();
});
