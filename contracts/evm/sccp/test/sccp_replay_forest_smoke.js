const assert = require("assert");
const fs = require("fs");
const path = require("path");

const REPO = path.join(__dirname, "..", "..", "..", "..");
const solc = require(path.join(
  REPO,
  "scripts",
  "contract_tooling",
  "authenticated-solc",
));
const { createHardhatProvider } = require(path.join(
  REPO,
  "scripts",
  "contract_tooling",
  "evm-runtime",
  "hardhat-provider.js",
));
const { ethers } = require("ethers");

const SOLC_BUILD = "0.7.4+commit.3f05b770.Emscripten.clang";
const MAGIC = ethers.toUtf8Bytes("SCCP-REPLAY-SMT-V1");
const DEPTH = 248;
const WITNESS_TYPE =
  "tuple(bytes32 expectedShardRoot,bytes32 priorRecordDigest,bytes32 siblingBitmap,bytes32[] siblings)";

function sha256(parts) {
  return ethers.sha256(ethers.concat(parts));
}

function parent(level, left, right) {
  return sha256([
    MAGIC,
    "0x12",
    ethers.toBeHex(level, 2),
    left,
    right,
  ]);
}

function emptyHashes() {
  const hashes = [sha256([MAGIC, "0x10"])];
  for (let level = 0; level < DEPTH; level++) {
    hashes.push(parent(level, hashes[level], hashes[level]));
  }
  return hashes;
}

function fold(key, leaf, explicit) {
  const empty = emptyHashes();
  const keyBits = BigInt(key);
  let root = leaf;
  for (let level = 0; level < DEPTH; level++) {
    const sibling = explicit.get(level) || empty[level];
    root = ((keyBits >> BigInt(level)) & 1n) === 1n
      ? parent(level, sibling, root)
      : parent(level, root, sibling);
  }
  return root;
}

function encodedWitness(abi, expectedShardRoot, priorRecordDigest, bitmap, siblings) {
  return abi.encode(
    [WITNESS_TYPE],
    [[expectedShardRoot, priorRecordDigest, ethers.toBeHex(bitmap, 32), siblings]],
  );
}

function source(file) {
  return { content: fs.readFileSync(path.join(REPO, file), "utf8") };
}

function compile() {
  assert.equal(solc.version(), SOLC_BUILD, "replay smoke used an unauthenticated compiler");
  const compilerLock = JSON.parse(
    fs.readFileSync(path.join(REPO, "scripts", "contract_tooling", "compiler-lock.json")),
  );
  const files = [...new Set([
    ...compilerLock.sources.evm,
    ...compilerLock.sources.tron,
    "contracts/evm/sccp/SccpSha256ReplayForest.sol",
  ])];
  const sources = Object.fromEntries(files.map((file) => [file, source(file)]));
  const output = JSON.parse(solc.compile(JSON.stringify({
    language: "Solidity",
    sources,
    settings: compilerLock.settings,
  })));
  const rejected = (output.errors || []).filter(
    (entry) => entry.severity === "error" || entry.severity === "warning",
  );
  assert.deepEqual(rejected, [], rejected.map((entry) => entry.formattedMessage).join("\n"));
  const sizes = [];
  for (const [file, name] of [
    ["contracts/bsc/sccp/TairaXorBscSccpBridge.sol", "TairaXorBscSccpBridge"],
    ["contracts/ethereum/sccp/TairaXorEthereumSccpBridge.sol", "TairaXorEthereumSccpBridge"],
    ["contracts/tron/sccp/TairaXorSccpBridge.sol", "TairaXorSccpBridge"],
  ]) {
    const contract = output.contracts[file][name];
    const creation = contract.evm.bytecode.object.length / 2;
    const runtime = contract.evm.deployedBytecode.object.length / 2;
    assert(runtime <= 24_576, `${name} exceeds EIP-170`);
    assert(creation <= 49_152, `${name} exceeds initcode limit`);
    sizes.push(`${name}=${creation}/${runtime}`);
    const names = new Set(contract.abi.filter((entry) => entry.type === "function").map((entry) => entry.name));
    assert(!names.has("usedSourceMessages") && !names.has("usedDestinationMessages"));
    assert(names.has("replayVerifier") && names.has("replayVerifierCodeHash"));
    assert(names.has("replayForestState"));
  }
  const verifier = output.contracts["contracts/evm/sccp/SccpSha256ReplayForest.sol"]
    .SccpSha256ReplayForest;
  const verifierCreation = verifier.evm.bytecode.object.length / 2;
  const verifierRuntime = verifier.evm.deployedBytecode.object.length / 2;
  assert(verifierRuntime <= 24_576, "SccpSha256ReplayForest exceeds EIP-170");
  assert(verifierCreation <= 49_152, "SccpSha256ReplayForest exceeds initcode limit");
  sizes.unshift(`SccpSha256ReplayForest=${verifierCreation}/${verifierRuntime}`);
  console.log(`SCCP creation/runtime bytes: ${sizes.join(", ")}`);
  return verifier;
}

async function rejectsWith(promise, marker) {
  let rejection;
  try {
    await promise;
  } catch (error) {
    rejection = error;
  }
  assert(rejection, `expected rejection ${marker}`);
  const seen = new Set();
  const queue = [rejection];
  const fragments = [];
  while (queue.length) {
    const value = queue.shift();
    if (!value || typeof value !== "object" || seen.has(value)) continue;
    seen.add(value);
    for (const key of ["message", "shortMessage", "reason"]) {
      if (typeof value[key] === "string") fragments.push(value[key]);
    }
    for (const key of ["error", "info", "cause"]) queue.push(value[key]);
  }
  assert(
    fragments.some((fragment) => fragment.includes(marker)),
    `rejection did not contain ${marker}: ${fragments.join(" | ")}`,
  );
}

async function main() {
  const artifact = compile();
  const providerHandle = createHardhatProvider({ chainId: 56, blockGasLimit: 20_000_000 });
  try {
    const provider = new ethers.BrowserProvider(providerHandle);
    const signer = await provider.getSigner();
    const factory = new ethers.ContractFactory(
      artifact.abi,
      `0x${artifact.evm.bytecode.object}`,
      signer,
    );
    const verifier = await factory.deploy({ gasLimit: 8_000_000 });
    await verifier.waitForDeployment();
    const abi = ethers.AbiCoder.defaultAbiCoder();
    const fixture = JSON.parse(
      fs.readFileSync(path.join(REPO, "fixtures", "sccp", "replay_forest_v1.json")),
    );
    const expected = fixture.expected;
    const routeHash = `0x${fixture.domain.route_configuration_hash_hex}`;
    const replayId = `0x${fixture.record.replay_id_hex}`;
    const payloadSha256 = `0x${fixture.record.payload_sha256_hex}`;
    const auxiliary = `0x${fixture.record.auxiliary_identity_sha256_hex}`;
    const principal = `0x${fixture.record.principal_bytes_hex}`;
    const domainHash = await verifier.domainHash(
      fixture.domain.source_network_tag,
      fixture.domain.target_network_tag,
      fixture.domain.operation_tag,
      fixture.domain.route_revision,
      routeHash,
      0,
      "0x",
    );
    assert.equal(domainHash.slice(2), expected.domain_hash_hex);
    const key = await verifier.replayKey(domainHash, replayId);
    assert.equal(key.slice(2), expected.replay_key_hex);
    assert.equal(Number(BigInt(key) >> 248n), expected.shard);
    const recordDigest = await verifier.addressRecordDigest(
      1,
      replayId,
      payloadSha256,
      9,
      2,
      principal,
      auxiliary,
    );
    assert.equal(recordDigest.slice(2), expected.record_digest_hex);
    const emptyRoot = await verifier.emptyShardRoot();
    assert.equal(emptyRoot.slice(2), expected.empty_shard_root_hex);
    assert.equal(emptyHashes()[0].slice(2), expected.empty_leaf_hash_hex);
    const emptyWitness = encodedWitness(abi, emptyRoot, ethers.ZeroHash, 0n, []);
    const record = [1, replayId, payloadSha256, 9, 2, principal, auxiliary];
    const transition = await verifier.prepareAddressOccupation(domainHash, record, emptyWitness);
    assert.equal(transition[0], BigInt(expected.shard));
    assert.equal(transition[1], key);
    assert.equal(transition[2], recordDigest);
    assert.equal(transition[3], emptyRoot);
    assert.equal(transition[4].slice(2), expected.occupied_shard_root_hex);

  const membership = encodedWitness(abi, transition[4], recordDigest, 0n, []);
  assert.equal(await verifier.verifyMembership(key, recordDigest, transition[4], membership), true);
  assert.equal(await verifier.verifyNonMembership(key, emptyRoot, emptyWitness), true);
  await rejectsWith(
    verifier.prepareAddressOccupation(domainHash, record, membership),
    "SR09",
  );
  await rejectsWith(
    verifier.prepareAddressOccupation(
      domainHash,
      record,
      encodedWitness(abi, `0x${"88".repeat(32)}`, ethers.ZeroHash, 0n, []),
    ),
    "SR10",
  );
  await rejectsWith(
    verifier.prepareAddressOccupation(
      domainHash,
      record,
      encodedWitness(abi, emptyRoot, ethers.ZeroHash, 1n << 248n, [`0x${"77".repeat(32)}`]),
    ),
    "SR19",
  );
  await rejectsWith(
    verifier.prepareAddressOccupation(
      domainHash,
      record,
      encodedWitness(abi, emptyRoot, ethers.ZeroHash, 1n, []),
    ),
    "SR17",
  );
  await rejectsWith(
    verifier.prepareAddressOccupation(
      domainHash,
      record,
      encodedWitness(abi, emptyRoot, ethers.ZeroHash, 1n, [emptyHashes()[0]]),
    ),
    "SR16",
  );
  await rejectsWith(
    verifier.prepareAddressOccupation(domainHash, record, ethers.concat([emptyWitness, "0x00"])),
    "SR20",
  );

  const sibling0 = `0x${"66".repeat(32)}`;
  const sibling1 = `0x${"77".repeat(32)}`;
  const orderedRoot = fold(key, emptyHashes()[0], new Map([[0, sibling0], [1, sibling1]]));
  const ordered = encodedWitness(abi, orderedRoot, ethers.ZeroHash, 3n, [sibling0, sibling1]);
  await verifier.prepareAddressOccupation(domainHash, record, ordered);
  const swapped = encodedWitness(abi, orderedRoot, ethers.ZeroHash, 3n, [sibling1, sibling0]);
  await rejectsWith(verifier.prepareAddressOccupation(domainHash, record, swapped), "SR10");

  const zeroEvmActor = `0x${"00".repeat(20)}`;
  await verifier.domainHash(0x41, 0x40, 0x10, 7, routeHash, 1, zeroEvmActor);
  const negativeWorkchainTon = `0xffffffff${"00".repeat(32)}`;
  const tonDomain = await verifier.domainHash(
    0x40,
    0x44,
    0x30,
    7,
    routeHash,
    3,
    negativeWorkchainTon,
  );
  const expectedTonDomain = sha256([
    MAGIC,
    "0x00",
    ethers.toBeHex(0x40, 4),
    ethers.toBeHex(0x44, 4),
    "0x30",
    ethers.toBeHex(7, 4),
    routeHash,
    "0x03",
    ethers.toBeHex(36, 2),
    negativeWorkchainTon,
  ]);
  assert.equal(tonDomain, expectedTonDomain);
  await rejectsWith(
    verifier.domainHash(0x40, 0x44, 0x30, 7, routeHash, 3, `0xffffffff${"00".repeat(31)}`),
    "SR03",
  );
  for (const retiredTag of fixture.rejected_network_tags) {
    await rejectsWith(
      verifier.domainHash(retiredTag, 0x41, 0x01, 7, routeHash, 0, "0x"),
      "SR03",
    );
  }
  await verifier.addressRecordDigest(0x10, replayId, payloadSha256, 9, 2, ethers.ZeroAddress, auxiliary);
  assert.equal(
    await verifier.replayKey(ethers.ZeroHash, ethers.ZeroHash),
    sha256([MAGIC, "0x01", ethers.ZeroHash, ethers.ZeroHash]),
  );
  await rejectsWith(
    verifier.addressRecordDigest(0x12, replayId, payloadSha256, 9, 2, principal, auxiliary),
    "SR04",
  );

    console.log("SCCP SHA-256 replay forest golden and adversarial smoke passed.");
  } finally {
    await providerHandle.disconnect();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
