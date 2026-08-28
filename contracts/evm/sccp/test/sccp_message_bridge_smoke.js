const fs = require("fs");
const path = require("path");
const assert = require("assert");
const crypto = require("crypto");
const REPO = path.join(__dirname, "..", "..", "..", "..");
const solc = process.env.SCCP_SOLJSON_PATH
  ? require(path.join(REPO, "scripts", "contract_tooling", "authenticated-solc"))
  : require("solc");
const { createHardhatProvider } = require(path.join(
  REPO,
  "scripts",
  "contract_tooling",
  "evm-runtime",
  "hardhat-provider.js",
));
const { ethers } = require("ethers");

const BSC_TESTNET_PROFILE = 5;
const ETHEREUM_SEPOLIA_PROFILE = 3;
const TRON_NILE_PROFILE = 11;
const ROUTE_REVISION = 7;
const DOMAIN_SORA = 0;
const DOMAIN_ETHEREUM = 1;
const DOMAIN_BSC = 2;
const DOMAIN_TRON = 5;
const CODEC_TEXT = 1;
const CODEC_EVM20 = 2;
const CODEC_TRON21 = 5;
const SCALE = 1_000_000_000n;
const MAX_U128 = (1n << 128n) - 1n;
const MAX_OUTSTANDING_LIABILITY = 1_000_000_000_000n;
const MAX_WRAPPED_SUPPLY = MAX_OUTSTANDING_LIABILITY * SCALE;
const MAX_RUNTIME_BYTES = 24_576;
const MAX_INITCODE_BYTES = 49_152;
// Stay below the EIP-7825 per-transaction gas cap enforced by the locked
// Hardhat runtime while retaining ample headroom for the largest constructor.
const MAX_DEPLOYMENT_GAS = 16_000_000n;
const I105_ALPHABET = Array.from(
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz" +
    "ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ",
);
const ED25519_FIELD = (1n << 255n) - 19n;
const ED25519_TORSION_ENCODINGS = [
  "0100000000000000000000000000000000000000000000000000000000000000",
  "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a",
  "0000000000000000000000000000000000000000000000000000000000000080",
  "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05",
  "ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f",
  "26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc85",
  "0000000000000000000000000000000000000000000000000000000000000000",
  "c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac03fa",
];
const ED25519_MIXED_TORSION_ENCODINGS = JSON.parse(
  fs.readFileSync(
    path.join(REPO, "fixtures", "crypto", "ed25519_public_key_admission_v1.json"),
    "utf8",
  ),
).vectors
  .filter((vector) => vector.name.startsWith("mixed-torsion-"))
  .map((vector) => vector.key_hex);
assert.equal(ED25519_MIXED_TORSION_ENCODINGS.length, 2);
const CANONICAL_SORA_I105 =
  "sorauﾛ1PYﾛ9ｵﾆﾘﾐ3Yf8wﾜｿﾋﾉajｼｱ6eﾑbHｱﾜｶBｳdUｺcヰｲnﾌNP21YC";
// I105 checksums cover canonical AccountAddress bytes, while the named
// sentinel carries the chain discriminant. Exercise Taira's exact `test` form.
const CANONICAL_I105 = `test${CANONICAL_SORA_I105.slice(4)}`;
const CANONICAL_I105_BYTES = ethers.toUtf8Bytes(CANONICAL_I105);
const CHECKSUM_MUTATED_I105 = `${CANONICAL_I105.slice(0, -1)}D`;
const NUMERIC_SENTINEL_ALIAS_I105 = `n369${CANONICAL_I105.slice(4)}`;
// Produced by Rust `iroha tools address convert --network-prefix 369` from the
// canonical compressed SEC1 generator key. This is an AccountId/I105 oracle,
// not a value independently invented by the Solidity smoke.
const RUST_TAIRA_SECP256K1_I105 =
  "test2QHHｴﾒﾔBgfﾐdｹヱa6ﾊyqVﾐｻrpruﾖZﾗｾWkｳzqGGﾕdｳｳﾏｻiM2HYQ6";
const SECP256K1_FIELD =
  0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2fn;
// Shared Rust/Solidity fixture commitments for the exact V1 semantic profile
// and Taira finality anchor encodings.
const SEMANTIC_PROOF_PROFILE_HASH =
  "0xce5a1e17aca3cafe47a403fd66479f0a36339eb56092dafa67c8d97bdeeb60ef";
const SORA_FINALITY_ANCHOR_HASH =
  "0x7dda271d98d9e4333093da84236157e39ce67f6f68680fedbdc17fbe8b7b6a4a";
const ALTERNATE_SEMANTIC_PROOF_PROFILE_HASH = ethers.keccak256(
  ethers.toUtf8Bytes("sccp:test:semantic-proof-profile:alternate:v1"),
);
const ALTERNATE_SORA_FINALITY_ANCHOR_HASH = ethers.keccak256(
  ethers.toUtf8Bytes("sccp:test:sora-finality-anchor:alternate:v1"),
);
const PROFILE_TAG = {
  "ethereum-mainnet": 2,
  "ethereum-sepolia": 3,
  "bsc-mainnet": 4,
  "bsc-testnet": 5,
  "tron-mainnet": 10,
  "tron-nile": 11,
  "tron-shasta": 12,
};
const SCALAR_FIELD =
  21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const BASE_FIELD =
  21888242871839275222246405745257275088696311157297823662689037894645226208583n;
// Deterministically generated from x=(1,2) on the BN254 twist. The point is
// on-curve but r * P is non-infinity, so EIP-197 must reject it as outside G2.
const NON_SUBGROUP_G2 = [
  1n,
  2n,
  2318417032921752773706234968143028537016473046724237753379416958334661833740n,
  12286822439340662745461952989251194370289180628671678316022104244014550321766n,
];
const SIGNAL_LABELS = [
  "message-id",
  "payload-hash",
  "target-domain",
  "commitment-root",
  "finality-height",
  "finality-block-hash",
  "source-domain",
  "statement-hash",
  "destination-binding-hash",
  "route-configuration-hash",
  "sora-finality-anchor-hash",
].map((name) =>
  ethers.keccak256(ethers.toUtf8Bytes(`sccp:groth16-bn254:signal:${name}:v1`)),
);
const GROTH16_PROOF_ABI_TYPES = [
  "uint256",
  "bytes32",
  "uint256",
  "bytes32",
  "uint256[2]",
  "uint256[4]",
  "uint256[2]",
];
const PINNED_EVM_SOLC_BUILD = "0.7.4+commit.3f05b770.Emscripten.clang";
const PINNED_EVM_SOLJSON_SHA256 =
  "2b55ed5fec4d9625b6c7b3ab1abd2b7fb7dd2a9c68543bf0323db2c7e2d55af2";
const PINNED_TRON_SOLC_BUILD = "0.7.4+commit.3f05b770.Emscripten.clang";
const PINNED_TRON_SOLJSON_SHA256 =
  "2b55ed5fec4d9625b6c7b3ab1abd2b7fb7dd2a9c68543bf0323db2c7e2d55af2";
const MAX_MANIFEST_BYTES = 128 * 1024 * 1024;

function source(file) {
  return { content: fs.readFileSync(path.join(REPO, file), "utf8") };
}

function sha256Hex(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function readStableRegularFile(filePath, maximumBytes, readOnly) {
  assert(path.isAbsolute(filePath), "authenticated input path must be absolute");
  const noFollow = fs.constants.O_NOFOLLOW || 0;
  const descriptor = fs.openSync(filePath, fs.constants.O_RDONLY | noFollow);
  try {
    const before = fs.fstatSync(descriptor, { bigint: true });
    assert((before.mode & 0o170000n) === 0o100000n, "authenticated input is not regular");
    assert(before.size > 0n && before.size <= BigInt(maximumBytes), "authenticated input is unbounded");
    if (readOnly) {
      assert((before.mode & 0o222n) === 0n, "authenticated runtime input must be read-only");
    }
    const payload = fs.readFileSync(descriptor);
    const after = fs.fstatSync(descriptor, { bigint: true });
    for (const field of ["dev", "ino", "mode", "size", "mtimeNs", "ctimeNs"]) {
      assert.equal(after[field], before[field], "authenticated input changed during its read");
    }
    assert.equal(BigInt(payload.length), before.size, "authenticated input read was incomplete");
    return payload;
  } finally {
    fs.closeSync(descriptor);
  }
}

function checkedJson(payload, label) {
  assert(payload[payload.length - 1] === 0x0a, `${label} must end in one newline`);
  assert(payload[payload.length - 2] !== 0x0a, `${label} has noncanonical trailing data`);
  const text = payload.subarray(0, payload.length - 1).toString("utf8");
  assert(Buffer.from(text, "utf8").equals(payload.subarray(0, -1)), `${label} is not UTF-8`);
  const parsed = JSON.parse(text);
  assert(parsed && typeof parsed === "object" && !Array.isArray(parsed), `${label} is not an object`);
  return { parsed, canonicalBytes: payload.subarray(0, payload.length - 1) };
}

function checkedBytecodeRecord(record, label) {
  assert(record && typeof record === "object" && !Array.isArray(record), `${label} record is missing`);
  assert(/^0x(?:[0-9a-f][0-9a-f])*$/.test(record.hex), `${label} hex is noncanonical`);
  const bytes = Buffer.from(record.hex.slice(2), "hex");
  assert.equal(record.byte_length, bytes.length, `${label} byte length drift`);
  assert.equal(record.sha256_hex, sha256Hex(bytes), `${label} SHA-256 drift`);
  assert.equal(record.keccak256_hex, ethers.keccak256(bytes).slice(2), `${label} Keccak-256 drift`);
  return bytes;
}

function checkedImmutableReferences(references, runtimeLength, label) {
  assert(Array.isArray(references), `${label} immutable references are missing`);
  const occupied = [];
  let previous = null;
  for (const entry of references) {
    assert.deepEqual(
      Object.keys(entry).sort(),
      ["ast_id", "length", "start"],
      `${label} immutable reference fields drifted`,
    );
    assert(/^(?:0|[1-9][0-9]*)$/.test(entry.ast_id), `${label} immutable AST id is invalid`);
    assert(Number.isSafeInteger(entry.start) && entry.start >= 0, `${label} immutable start is invalid`);
    assert(Number.isSafeInteger(entry.length) && entry.length > 0, `${label} immutable length is invalid`);
    assert(entry.start + entry.length <= runtimeLength, `${label} immutable reference is out of bounds`);
    const ordering = [entry.start, entry.length, BigInt(entry.ast_id)];
    if (previous) {
      assert(
        ordering[0] > previous[0] ||
          (ordering[0] === previous[0] &&
            (ordering[1] > previous[1] ||
              (ordering[1] === previous[1] && ordering[2] >= previous[2]))),
        `${label} immutable references are not canonical`,
      );
    }
    previous = ordering;
    occupied.push([entry.start, entry.start + entry.length]);
  }
  for (let index = 1; index < occupied.length; index++) {
    assert(occupied[index - 1][1] <= occupied[index][0], `${label} immutable references overlap`);
  }
  return references;
}

function manifestArtifact(record, target, limits) {
  const label = record.fully_qualified_name;
  assert.equal(
    label,
    `${record.source_path}:${record.contract_name}`,
    `${target} artifact identity drift`,
  );
  assert(Array.isArray(record.abi), `${label} ABI is missing`);
  const creation = checkedBytecodeRecord(record.creation_bytecode, `${label} creation`);
  const runtime = checkedBytecodeRecord(record.runtime_bytecode, `${label} runtime`);
  assert.equal(
    creation.length,
    limits.creation_bytecode_bytes,
    `${label} reviewed creation-byte size drift`,
  );
  assert.equal(
    runtime.length,
    limits.runtime_bytecode_bytes,
    `${label} reviewed runtime-byte size drift`,
  );
  assert(creation.length <= MAX_INITCODE_BYTES, `${label} creation code exceeds its ceiling`);
  assert(runtime.length <= MAX_RUNTIME_BYTES, `${label} runtime exceeds its ceiling`);
  return {
    abi: record.abi,
    bytecode: record.creation_bytecode.hex,
    runtimeBytecode: record.runtime_bytecode.hex,
    runtimeImmutableReferences: checkedImmutableReferences(
      record.runtime_immutable_references,
      runtime.length,
      label,
    ),
    fullyQualifiedName: label,
    target,
    authenticatedProductionArtifact: true,
  };
}

function loadAuthenticatedProductionArtifacts(manifestPayload, lockPayload) {
  const manifestJson = checkedJson(manifestPayload, "artifact manifest");
  const lockJson = checkedJson(lockPayload, "artifact lock");
  const manifest = manifestJson.parsed;
  const lock = lockJson.parsed;
  assert.equal(manifest.schema, "iroha.sccp.contract-artifacts.v1", "artifact manifest schema drift");
  assert.equal(lock.schema, "iroha.sccp.contract-artifact-lock.v1", "artifact lock schema drift");
  assert.equal(
    sha256Hex(manifestJson.canonicalBytes),
    lock.corridor_manifest_sha256_hex,
    "artifact manifest digest drift",
  );
  assert.equal(
    manifest.compiler_lock_sha256_hex,
    lock.compiler_lock_sha256_hex,
    "compiler lock binding drift",
  );
  assert.deepEqual(Object.keys(manifest.targets).sort(), ["evm", "tron"]);
  const expectedCompilers = {
    evm: {
      build: PINNED_EVM_SOLC_BUILD,
      digest: PINNED_EVM_SOLJSON_SHA256,
    },
    tron: {
      build: PINNED_TRON_SOLC_BUILD,
      digest: PINNED_TRON_SOLJSON_SHA256,
    },
  };
  const nested = {};
  const runtimeSizes = [];
  for (const target of ["evm", "tron"]) {
    const targetManifest = manifest.targets[target];
    assert.equal(targetManifest.target, target, `${target} target role drift`);
    assert.equal(
      targetManifest.compiler.reported_version,
      expectedCompilers[target].build,
      `${target} compiler version drift`,
    );
    assert.equal(
      targetManifest.compiler.soljson_sha256_hex,
      expectedCompilers[target].digest,
      `${target} compiler digest drift`,
    );
    const limits = lock.targets[target].contract_sizes;
    const byFile = {};
    const seen = new Set();
    for (const record of targetManifest.contracts) {
      assert(!seen.has(record.fully_qualified_name), `${target} artifact is duplicated`);
      seen.add(record.fully_qualified_name);
      const reviewedSize = limits[record.fully_qualified_name];
      assert(reviewedSize, `${record.fully_qualified_name} is absent from the review lock`);
      const artifactLimits = {
        creation_bytecode_bytes: reviewedSize.creation_bytecode_bytes,
        runtime_bytecode_bytes: reviewedSize.runtime_bytecode_bytes,
      };
      const normalized = manifestArtifact(record, target, artifactLimits);
      byFile[record.source_path] ||= {};
      byFile[record.source_path][record.contract_name] = normalized;
      runtimeSizes.push(`${target}:${record.fully_qualified_name}=${record.runtime_bytecode.byte_length}`);
    }
    assert.equal(seen.size, Object.keys(limits).length, `${target} review lock artifact set drift`);
    nested[target] = byFile;
  }
  return { manifest, targets: nested, runtimeSizes };
}

function compilerArtifact(value, fullyQualifiedName, target = "evm-test-harness") {
  assert(value && Array.isArray(value.abi), `${fullyQualifiedName} mock ABI is missing`);
  const creationHex = `0x${value.evm.bytecode.object}`;
  const runtimeHex = `0x${value.evm.deployedBytecode.object}`;
  assert(/^0x(?:[0-9a-f][0-9a-f])*$/.test(creationHex), `${fullyQualifiedName} mock creation code is invalid`);
  assert(/^0x(?:[0-9a-f][0-9a-f])*$/.test(runtimeHex), `${fullyQualifiedName} mock runtime is invalid`);
  const references = [];
  for (const [astId, locations] of Object.entries(value.evm.deployedBytecode.immutableReferences || {})) {
    for (const location of locations) references.push({ ast_id: astId, ...location });
  }
  references.sort((left, right) =>
    left.start - right.start || left.length - right.length || Number(BigInt(left.ast_id) - BigInt(right.ast_id)),
  );
  checkedImmutableReferences(references, ethers.getBytes(runtimeHex).length, fullyQualifiedName);
  return {
    abi: value.abi,
    bytecode: creationHex,
    runtimeBytecode: runtimeHex,
    runtimeImmutableReferences: references,
    fullyQualifiedName,
    target,
    authenticatedProductionArtifact: false,
  };
}

function compile() {
  const expectedCompilerBuild = process.env.SCCP_EXPECTED_SOLC_BUILD || PINNED_EVM_SOLC_BUILD;
  assert.strictEqual(expectedCompilerBuild, PINNED_EVM_SOLC_BUILD, "SCCP V1 requires the locked EVM compiler build");
  assert.strictEqual(solc.version(), PINNED_EVM_SOLC_BUILD, "unexpected Solidity compiler build");

  const manifestPath = process.env.SCCP_CONTRACT_ARTIFACT_MANIFEST;
  const artifactLockPath = process.env.SCCP_CONTRACT_ARTIFACT_LOCK;
  assert(manifestPath && artifactLockPath, "authenticated artifact manifest and lock are required");
  const manifestPayload = readStableRegularFile(manifestPath, MAX_MANIFEST_BYTES, true);
  const lockPayload = readStableRegularFile(artifactLockPath, MAX_MANIFEST_BYTES, true);
  const production = loadAuthenticatedProductionArtifacts(manifestPayload, lockPayload);

  const mutatedManifest = Buffer.from(manifestPayload);
  const mutationIndex = mutatedManifest.indexOf(Buffer.from('"abi":'));
  assert(mutationIndex >= 0, "artifact manifest contains no ABI to authenticate");
  mutatedManifest[mutationIndex + 1] ^= 1;
  assert.throws(
    () => loadAuthenticatedProductionArtifacts(mutatedManifest, lockPayload),
    /artifact manifest digest drift|JSON/,
    "artifact mutation must fail before any provider or deployment is created",
  );

  const files = [
    "contracts/evm/sccp/ISccpMessageVerifier.sol",
    "contracts/evm/sccp/SccpExactTransferCodec.sol",
    "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol",
    "contracts/evm/sccp/TairaXorEvmToken.sol",
    "contracts/evm/sccp/TairaXorExactEvmSccpBridge.sol",
    "contracts/bsc/sccp/TairaXOR.sol",
    "contracts/bsc/sccp/TairaXorBscSccpBridge.sol",
    "contracts/ethereum/sccp/TairaXOR.sol",
    "contracts/ethereum/sccp/TairaXorEthereumSccpBridge.sol",
    "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    "contracts/tron/sccp/TairaXOR.sol",
    "contracts/tron/sccp/TairaXorSccpBridge.sol",
  ];
  const mocks = `
// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;
pragma experimental ABIEncoderV2;
import "contracts/evm/sccp/SccpExactTransferCodec.sol";
import "contracts/evm/sccp/TairaXorExactEvmSccpBridge.sol";
import {TairaXOR as BscTairaXOR} from "contracts/bsc/sccp/TairaXOR.sol";
interface IReentryRoute { function transferToTaira(bytes calldata,uint256) external returns(bytes32); }
contract InjectedBscRouteHarness is TairaXorExactEvmSccpBridge {
  constructor(
    address tokenAddress,
    VerifierPolicyV1 memory policy,
    uint32 revision,
    uint256 maxWrappedSupply
  ) TairaXorExactEvmSccpBridge(tokenAddress, policy, 2, 5, revision, maxWrappedSupply)
  {}
}
contract CodecHarness {
  function isTairaRecipient(bytes calldata input) external pure returns(bool) {
    bytes memory value = input;
    return SccpExactTransferCodec.isCanonicalTairaRecipient(value);
  }
  function isTairaAccount(bytes calldata input) external pure returns(bool) {
    bytes memory value = input;
    return SccpExactTransferCodec.isCanonicalTairaAccountRange(value, 0, value.length);
  }
  function sourceVector(uint8 profile, bytes calldata canonicalPayload)
    external pure returns(bytes memory,bytes32,bytes32,bytes32,bytes32)
  {
    bytes memory sourceNetwork;
    if (profile == 2 || profile == 3) sourceNetwork = SccpExactTransferCodec.ethereumNetwork(profile);
    else if (profile == 4 || profile == 5) sourceNetwork = SccpExactTransferCodec.bscNetwork(profile);
    else sourceNetwork = SccpExactTransferCodec.tronNetwork(profile);
    bytes memory exactLane = SccpExactTransferCodec.lane(
      sourceNetwork, SccpExactTransferCodec.tairaNetwork()
    );
    bytes memory payload = canonicalPayload;
    bytes32 laneHash = SccpExactTransferCodec.laneHash(exactLane);
    bytes32 messageId = SccpExactTransferCodec.messageId(exactLane, payload);
    bytes32 payloadHash = SccpExactTransferCodec.payloadHash(payload);
    return (
      exactLane,
      laneHash,
      messageId,
      payloadHash,
      SccpExactTransferCodec.sourceEventDigest(laneHash, messageId, payloadHash)
    );
  }
  function evmHashVector(uint8 profile, bytes calldata canonicalPayload)
    external view returns(bytes32,bytes32)
  {
    bytes memory sourceNetwork;
    if (profile == 2 || profile == 3) sourceNetwork = SccpExactTransferCodec.ethereumNetwork(profile);
    else if (profile == 4 || profile == 5) sourceNetwork = SccpExactTransferCodec.bscNetwork(profile);
    else sourceNetwork = SccpExactTransferCodec.tronNetwork(profile);
    bytes memory exactLane = SccpExactTransferCodec.lane(
      sourceNetwork, SccpExactTransferCodec.tairaNetwork()
    );
    bytes memory payload = canonicalPayload;
    return (
      SccpExactTransferCodec.laneHashEvm(exactLane),
      SccpExactTransferCodec.payloadHashEvm(payload)
    );
  }
  function rawBlakeParity(bytes calldata input)
    external view returns(bytes32,bytes32)
  {
    bytes memory value = input;
    return (
      SccpExactTransferCodec.blake2b256(value),
      SccpExactTransferCodec.blake2b256Evm(value)
    );
  }
}
contract TokenRouteHarness {
  BscTairaXOR public immutable token;
  constructor() { token = new BscTairaXOR(address(this)); }
  function mint(address to, uint256 value) external returns(bool) {
    return token.mint(to, value);
  }
  function burn(address from, uint256 value) external returns(bool) {
    return token.burnFrom(from, value);
  }
}
contract FalseToken {
  address public immutable bridge;
  uint8 public constant decimals = 18;
  uint256 public totalSupply;
  mapping(address => uint256) public balanceOf;
  constructor(address routeBridge) { bridge = routeBridge; }
  function seed(address holder,uint256 value) external {
    require(totalSupply == 0, "already seeded");
    totalSupply = value;
    balanceOf[holder] = value;
  }
  function mint(address,uint256) external pure returns(bool) { return false; }
  function burnFrom(address,uint256) external pure returns(bool) { return false; }
}
contract WrongDecimalsToken {
  address public immutable bridge;
  uint8 public constant decimals = 17;
  uint256 public totalSupply;
  mapping(address => uint256) public balanceOf;
  constructor(address routeBridge) { bridge = routeBridge; }
  function mint(address,uint256) external pure returns(bool) { return false; }
  function burnFrom(address,uint256) external pure returns(bool) { return false; }
}
contract NonzeroSupplyToken {
  address public immutable bridge;
  uint8 public constant decimals = 18;
  uint256 public totalSupply = 1;
  mapping(address => uint256) public balanceOf;
  constructor(address routeBridge) { bridge = routeBridge; }
  function mint(address,uint256) external pure returns(bool) { return false; }
  function burnFrom(address,uint256) external pure returns(bool) { return false; }
}
contract TrueNoopToken {
  address public immutable bridge;
  uint8 public constant decimals = 18;
  uint256 public totalSupply;
  mapping(address => uint256) public balanceOf;
  constructor(address routeBridge) { bridge = routeBridge; }
  function seed(address holder,uint256 value) external {
    require(totalSupply == 0, "already seeded");
    totalSupply = value;
    balanceOf[holder] = value;
  }
  function mint(address,uint256) external pure returns(bool) { return true; }
  function burnFrom(address,uint256) external pure returns(bool) { return true; }
}
contract WrongDeltaToken {
  address public immutable bridge;
  uint8 public constant decimals = 18;
  uint256 public totalSupply;
  mapping(address => uint256) public balanceOf;
  constructor(address routeBridge) { bridge = routeBridge; }
  function seed(address holder,uint256 value) external {
    require(totalSupply == 0, "already seeded");
    totalSupply = value;
    balanceOf[holder] = value;
  }
  function mint(address to,uint256 value) external returns(bool) {
    require(msg.sender == bridge, "not bridge");
    totalSupply += value + 1;
    balanceOf[to] += value + 1;
    return true;
  }
  function burnFrom(address from,uint256 value) external returns(bool) {
    require(msg.sender == bridge && value > 1, "bad burn");
    totalSupply -= value - 1;
    balanceOf[from] -= value - 1;
    return true;
  }
}
contract ReentrantToken {
  address public immutable bridge;
  uint8 public constant decimals = 18;
  uint256 public totalSupply;
  mapping(address => uint256) public balanceOf;
  bytes private recipient;
  bool private entered;
  constructor(address routeBridge, bytes memory tairaRecipient) {
    bridge = routeBridge;
    recipient = tairaRecipient;
  }
  function seed(address holder,uint256 value) external {
    require(totalSupply == 0, "already seeded");
    totalSupply = value;
    balanceOf[holder] = value;
  }
  function mint(address to,uint256 value) external returns(bool) {
    require(msg.sender == bridge, "bad mint setup");
    totalSupply += value;
    balanceOf[to] += value;
    return true;
  }
  function burnFrom(address from,uint256 value) external returns(bool) {
    require(msg.sender == bridge && !entered, "bad reentry setup");
    entered = true;
    (bool success,) = bridge.call(
      abi.encodeWithSelector(IReentryRoute.transferToTaira.selector, recipient, uint256(1e9))
    );
    require(!success, "reentry was accepted");
    entered = false;
    totalSupply -= value;
    balanceOf[from] -= value;
    return true;
  }
}
contract CodeAliasedVerifier {
  bytes32 private configuredNetworkId;
  bool private anchorAliasesCode;
  constructor(bytes32 networkIdValue, bool aliasAnchor) {
    configuredNetworkId = networkIdValue;
    anchorAliasesCode = aliasAnchor;
  }
  function networkId() external view returns(bytes32) { return configuredNetworkId; }
  function expectedSourceDomain() external pure returns(uint32) { return 0; }
  function expectedTargetDomain() external pure returns(uint32) { return 5; }
  function verifyingKeyHash() external pure returns(bytes32) {
    return keccak256("sccp:test:code-aliased-verifier:key:v1");
  }
  function _codeHash() private view returns(bytes32 value) {
    assembly { value := extcodehash(address()) }
  }
  function semanticProofProfileHash() external view returns(bytes32) {
    return anchorAliasesCode
      ? keccak256("sccp:test:code-aliased-verifier:semantic:v1")
      : _codeHash();
  }
  function soraFinalityAnchorHash() external view returns(bytes32) {
    return anchorAliasesCode
      ? _codeHash()
      : keccak256("sccp:test:code-aliased-verifier:anchor:v1");
  }
  function verifySccpMessageProof(bytes calldata,bytes32[6] calldata,bytes32,bytes32,bytes32)
    external pure returns(bytes32,uint32,bytes32)
  {
    return (bytes32(uint256(1)), 0, bytes32(uint256(2)));
  }
}`;
  const sources = Object.fromEntries(files.map((file) => [file, source(file)]));
  sources["Mocks.sol"] = { content: mocks };
  const input = JSON.stringify({
    language: "Solidity",
    sources,
    settings: production.manifest.targets.evm.settings,
  });
  const output = JSON.parse(solc.compile(input));
  if (output.errors) {
    const rejected = output.errors.filter(
      (entry) => entry.severity === "error" || entry.severity === "warning",
    );
    if (rejected.length) {
      throw new Error(rejected.map((entry) => entry.formattedMessage).join("\n"));
    }
  }
  const mocksByFile = { "Mocks.sol": {} };
  for (const [name, value] of Object.entries(output.contracts["Mocks.sol"])) {
    const normalized = compilerArtifact(value, `Mocks.sol:${name}`);
    assert(
      ethers.getBytes(normalized.runtimeBytecode).length <= MAX_RUNTIME_BYTES,
      `${name} mock runtime exceeds the ${MAX_RUNTIME_BYTES}-byte ceiling`,
    );
    assert(
      ethers.getBytes(normalized.bytecode).length <= MAX_INITCODE_BYTES,
      `${name} mock creation code exceeds the ${MAX_INITCODE_BYTES}-byte ceiling`,
    );
    mocksByFile["Mocks.sol"][name] = normalized;
  }
  const tronCompatibilityByFile = {};
  for (const file of [
    "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    "contracts/tron/sccp/TairaXOR.sol",
    "contracts/tron/sccp/TairaXorSccpBridge.sol",
  ]) {
    tronCompatibilityByFile[file] = {};
    for (const [name, value] of Object.entries(output.contracts[file])) {
      const normalized = compilerArtifact(
        value,
        `${file}:${name}`,
        "tron-evm-compatibility-test",
      );
      assert(
        ethers.getBytes(normalized.bytecode).length <= MAX_INITCODE_BYTES,
        `${file}:${name} compatibility creation code exceeds its ceiling`,
      );
      assert(
        ethers.getBytes(normalized.runtimeBytecode).length <= MAX_RUNTIME_BYTES,
        `${file}:${name} compatibility runtime exceeds its ceiling`,
      );
      tronCompatibilityByFile[file][name] = normalized;
    }
  }
  const exactTvmRoute = production.targets.tron[
    "contracts/tron/sccp/TairaXorSccpBridge.sol"
  ].TairaXorSccpBridge;
  const compatibilityRoute = tronCompatibilityByFile[
    "contracts/tron/sccp/TairaXorSccpBridge.sol"
  ].TairaXorSccpBridge;
  assert.equal(
    compatibilityRoute.bytecode,
    exactTvmRoute.bytecode,
    "the shared pinned 0.7.4 compiler must reproduce the exact TRON route bytecode",
  );
  return {
    evmContracts: production.targets.evm,
    tvmContracts: production.targets.tron,
    mockContracts: mocksByFile,
    runtimeSizes: production.runtimeSizes,
  };
}

function i105PolymodStep(current, value) {
  const top = current >> 25n;
  let next = ((current & 0x01ffffffn) << 5n) ^ BigInt(value);
  if ((top & 1n) !== 0n) next ^= 0x3b6a57b2n;
  if ((top & 2n) !== 0n) next ^= 0x26508e6dn;
  if ((top & 4n) !== 0n) next ^= 0x1ea119fan;
  if ((top & 8n) !== 0n) next ^= 0x3d4233ddn;
  if ((top & 16n) !== 0n) next ^= 0x2a1462b3n;
  return next;
}

function i105Checksum(canonical) {
  let polymod = 1n;
  for (const value of [3, 3, 3, 0, 19, 14, 24]) {
    polymod = i105PolymodStep(polymod, value);
  }
  let accumulator = 0;
  let bits = 0;
  for (const byte of canonical) {
    accumulator = (accumulator << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      polymod = i105PolymodStep(polymod, (accumulator >> bits) & 31);
    }
    accumulator &= bits === 0 ? 0 : (1 << bits) - 1;
  }
  if (bits !== 0) {
    polymod = i105PolymodStep(polymod, (accumulator << (5 - bits)) & 31);
  }
  for (let i = 0; i < 6; i++) polymod = i105PolymodStep(polymod, 0);
  polymod ^= 0x2bc830a3n;
  return Array.from(
    { length: 6 },
    (_, i) => Number((polymod >> BigInt(5 * (5 - i))) & 31n),
  );
}

function base105Digits(canonical) {
  let leadingZeros = 0;
  while (leadingZeros < canonical.length && canonical[leadingZeros] === 0) {
    leadingZeros += 1;
  }
  let number = 0n;
  for (const byte of canonical.subarray(leadingZeros)) {
    number = (number << 8n) | BigInt(byte);
  }
  const digits = [];
  while (number !== 0n) {
    digits.unshift(Number(number % 105n));
    number /= 105n;
  }
  if (digits.length === 0 && leadingZeros === 0) digits.push(0);
  return [...Array(leadingZeros).fill(0), ...digits];
}

function encodeI105(canonical, sentinel = "test") {
  const bytes = Buffer.from(canonical);
  return `${sentinel}${[...base105Digits(bytes), ...i105Checksum(bytes)]
    .map((digit) => I105_ALPHABET[digit])
    .join("")}`;
}

function decodeI105Canonical(literal, sentinelLength = 4) {
  const symbols = Array.from(literal.slice(sentinelLength));
  const digits = symbols.map((symbol) => {
    const digit = I105_ALPHABET.indexOf(symbol);
    assert.notEqual(digit, -1, `unknown I105 symbol ${symbol}`);
    return digit;
  });
  const payloadDigits = digits.slice(0, -6);
  let leadingZeros = 0;
  while (payloadDigits[leadingZeros] === 0) leadingZeros += 1;
  let number = 0n;
  for (const digit of payloadDigits.slice(leadingZeros)) {
    number = number * 105n + BigInt(digit);
  }
  let hex = number.toString(16);
  if (hex.length % 2 !== 0) hex = `0${hex}`;
  const body = number === 0n ? Buffer.alloc(0) : Buffer.from(hex, "hex");
  return Buffer.concat([Buffer.alloc(leadingZeros), body]);
}

function le32(value) {
  const out = Buffer.alloc(32);
  let current = BigInt(value);
  for (let i = 0; i < out.length; i++) {
    out[i] = Number(current & 0xffn);
    current >>= 8n;
  }
  assert.equal(current, 0n);
  return out;
}

function exactSingleKeyCanonical(key) {
  const bytes = Buffer.from(key);
  assert.equal(bytes.length, 32);
  return Buffer.concat([Buffer.from([0x02, 0x00, 0x01, 0x20]), bytes]);
}

function universalTairaSenderCanonicals() {
  const addressVectors = JSON.parse(
    fs.readFileSync(
      path.join(REPO, "fixtures/account/address_vectors.json"),
      "utf8",
    ),
  );
  const rustMultisig = addressVectors.cases.positive.find(
    ({ case_id: caseId }) => caseId === "addr-multisig-wonderland-threshold2",
  );
  assert(rustMultisig, "Rust two-member multisig address vector is missing");
  const multisig = Buffer.from(
    rustMultisig.encodings.canonical_hex.slice(2),
    "hex",
  );
  assert.equal(multisig.length, 81);
  assert.equal(multisig.subarray(0, 7).toString("hex"), "0a010100020002");
  assert.equal(
    encodeI105(multisig, "sora"),
    rustMultisig.encodings.i105.string,
    "smoke I105 encoding must reproduce Rust's checked-in multisig vector",
  );

  const secp256k1 = Buffer.concat([
    Buffer.from([0x02, 0x00, 0x04, 0x21]),
    Buffer.from(
      "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
      "hex",
    ),
  ]);
  assert.equal(
    encodeI105(secp256k1),
    RUST_TAIRA_SECP256K1_I105,
    "smoke I105 encoding must reproduce Rust's secp256k1 AccountId",
  );
  return [multisig, secp256k1];
}

function universalTairaSenders() {
  return universalTairaSenderCanonicals().map((value) =>
    ethers.toUtf8Bytes(encodeI105(value)),
  );
}

function invalidI105Values() {
  const canonical = decodeI105Canonical(CANONICAL_I105);
  assert.equal(
    canonical.subarray(0, 4).toString("hex"),
    "02000120",
    "positive fixture must use the exact Taira single-Ed25519 layout",
  );
  assert.equal(encodeI105(canonical), CANONICAL_I105);
  const oversized = Buffer.concat([
    Buffer.from(CANONICAL_I105_BYTES),
    Buffer.alloc(257 - CANONICAL_I105_BYTES.length, 0x61),
  ]);
  assert.equal(oversized.length, 257);
  const mutateCanonical = (offset, value) => {
    const copy = Buffer.from(canonical);
    copy[offset] = value;
    return ethers.toUtf8Bytes(encodeI105(copy));
  };
  const payloadSymbols = Array.from(CANONICAL_I105);
  payloadSymbols[12] = payloadSymbols[12] === "1" ? "2" : "1";
  const malformedPayloadDigit = payloadSymbols.join("");
  const hostileKeys = [
    Buffer.alloc(32),
    le32(ED25519_FIELD),
    Buffer.from(`ee${"ff".repeat(30)}7f`, "hex"),
    Buffer.from(`f0${"ff".repeat(30)}7f`, "hex"),
    Buffer.alloc(32, 0x02),
    ...ED25519_TORSION_ENCODINGS.map((value) => Buffer.from(value, "hex")),
    ...ED25519_MIXED_TORSION_ENCODINGS.map((value) => Buffer.from(value, "hex")),
    Buffer.from(`01${"00".repeat(30)}80`, "hex"),
  ];
  const values = [
    ethers.toUtf8Bytes(CANONICAL_SORA_I105),
    ethers.toUtf8Bytes(`dev${CANONICAL_I105.slice(4)}`),
    ethers.toUtf8Bytes(`n42${CANONICAL_I105.slice(4)}`),
    ethers.toUtf8Bytes(CHECKSUM_MUTATED_I105),
    ethers.toUtf8Bytes(NUMERIC_SENTINEL_ALIAS_I105),
    ethers.toUtf8Bytes(malformedPayloadDigit),
    ethers.toUtf8Bytes(CANONICAL_I105.slice(0, -1)),
    ethers.toUtf8Bytes(`${CANONICAL_I105}1`),
    Buffer.from(CANONICAL_I105_BYTES).subarray(0, CANONICAL_I105_BYTES.length - 1),
    ethers.toUtf8Bytes("alice@taira"),
    ethers.toUtf8Bytes("alice"),
    ethers.toUtf8Bytes("sora雪"),
    ethers.toUtf8Bytes(` ${CANONICAL_I105}`),
    mutateCanonical(0, 0x00),
    mutateCanonical(0, 0x03),
    mutateCanonical(1, 0x01),
    mutateCanonical(2, 0x04),
    mutateCanonical(3, 0x1f),
    ...hostileKeys.map((key) =>
      ethers.toUtf8Bytes(encodeI105(exactSingleKeyCanonical(key))),
    ),
    oversized,
  ];
  return [...new Map(values.map((value) => [ethers.hexlify(value), value])).values()];
}

function invalidTairaSenderValues() {
  const [multisig, secp256k1] = universalTairaSenderCanonicals();
  const mutate = (value, mutation) => {
    const copy = Buffer.from(value);
    mutation(copy);
    return copy;
  };
  const firstMember = multisig.subarray(7, 44);
  const secondMember = multisig.subarray(44, 81);
  assert.equal(firstMember.length, 37);
  assert.equal(secondMember.length, 37);

  const secpFieldBytes = Buffer.from(
    SECP256K1_FIELD.toString(16).padStart(64, "0"),
    "hex",
  );
  const nonminimalExtendedSecp = Buffer.concat([
    Buffer.from([0x02, 0x02, 0x04, 0x00, 0x21]),
    secp256k1.subarray(4),
  ]);
  const structuralCanonicals = [
    mutate(multisig, (value) => {
      value[2] = 0;
    }), // unsupported multisig version
    mutate(multisig, (value) => {
      value[3] = 0;
      value[4] = 0;
    }), // zero threshold
    mutate(multisig, (value) => {
      value[5] = 0;
      value[6] = 0;
    }), // empty policy
    mutate(multisig, (value) => {
      value[3] = 0;
      value[4] = 4;
    }), // threshold exceeds total weight three
    mutate(multisig, (value) => {
      value[8] = 0;
      value[9] = 0;
    }), // zero member weight
    mutate(multisig, (value) => {
      value.copy(value, 49, 12, 44);
    }), // duplicate member key, even with a different weight
    Buffer.concat([multisig.subarray(0, 7), secondMember, firstMember]),
    mutate(multisig, (value) => {
      value.fill(0, 49, 81);
    }), // weak second Ed25519 member
    mutate(multisig, (value) => {
      value[7] = 3;
    }), // controller curve without an exact contract-side validator
    mutate(secp256k1, (value) => {
      value[4] = 4;
    }), // uncompressed SEC1 prefix in a compressed-length payload
    mutate(secp256k1, (value) => {
      value.fill(0, 5, 37);
    }), // x=0 has no secp256k1 square root
    mutate(secp256k1, (value) => {
      secpFieldBytes.copy(value, 5);
    }), // x equal to the base field modulus
    nonminimalExtendedSecp,
  ];
  const values = [
    ...invalidI105Values(),
    ...structuralCanonicals.map((value) =>
      ethers.toUtf8Bytes(encodeI105(value)),
    ),
  ];
  return [...new Map(values.map((value) => [ethers.hexlify(value), value])).values()];
}

function artifact(contracts, file, name) {
  const value = contracts[file][name];
  assert(value, `missing compiled artifact ${file}:${name}`);
  return value;
}

async function assertRuntimeAtAddress(provider, address, value) {
  const actual = ethers.getBytes(await provider.getCode(address));
  const expected = ethers.getBytes(value.runtimeBytecode);
  assert(expected.length > 0, `${value.fullyQualifiedName} has empty runtime bytecode`);
  assert.equal(
    actual.length,
    expected.length,
    `${value.fullyQualifiedName} deployed runtime length drift`,
  );
  assert(actual.length <= MAX_RUNTIME_BYTES, `${value.fullyQualifiedName} deployed runtime exceeds its ceiling`);
  const immutable = new Uint8Array(expected.length);
  for (const reference of value.runtimeImmutableReferences) {
    immutable.fill(1, reference.start, reference.start + reference.length);
  }
  for (let offset = 0; offset < expected.length; offset++) {
    if (!immutable[offset]) {
      assert.equal(
        actual[offset],
        expected[offset],
        `${value.fullyQualifiedName} deployed runtime drift outside immutable slot at byte ${offset}`,
      );
    }
  }
}

async function assertDeployedRuntimeMatchesArtifact(contract, value) {
  await assertRuntimeAtAddress(
    contract.runner.provider,
    await contract.getAddress(),
    value,
  );
}

async function deploy(signer, value, args = []) {
  const factory = new ethers.ContractFactory(value.abi, value.bytecode, signer);
  const deployment = await factory.getDeployTransaction(...args);
  const creationBytes = ethers.getBytes(value.bytecode);
  const initcodeBytes = ethers.getBytes(deployment.data);
  assert(creationBytes.length <= MAX_INITCODE_BYTES, "artifact creation bytecode exceeds its ceiling");
  assert(
    initcodeBytes.length <= MAX_INITCODE_BYTES,
    `deployment initcode exceeds the ${MAX_INITCODE_BYTES}-byte ceiling`,
  );
  assert(
    Buffer.from(initcodeBytes.subarray(0, creationBytes.length)).equals(Buffer.from(creationBytes)),
    "deployment did not use the authenticated creation bytecode prefix",
  );
  const estimatedGas = await signer.estimateGas(deployment);
  assert(
    estimatedGas <= MAX_DEPLOYMENT_GAS,
    `deployment estimate exceeds the ${MAX_DEPLOYMENT_GAS} gas ceiling`,
  );
  const contract = await factory.deploy(...args, {
    gasLimit: MAX_DEPLOYMENT_GAS,
  });
  const receipt = await contract.deploymentTransaction().wait();
  assert.equal(receipt.status, 1, "contract deployment failed");
  assert(
    receipt.gasUsed <= MAX_DEPLOYMENT_GAS,
    `deployment exceeds the ${MAX_DEPLOYMENT_GAS} gas ceiling`,
  );
  await assertDeployedRuntimeMatchesArtifact(contract, value);
  return contract;
}

async function assertConstructorRevertsWith(signer, value, args, reason) {
  const factory = new ethers.ContractFactory(value.abi, value.bytecode, signer);
  const transaction = await factory.getDeployTransaction(...args);
  const creationBytes = ethers.getBytes(value.bytecode);
  const initcodeBytes = ethers.getBytes(transaction.data);
  assert(
    initcodeBytes.length <= MAX_INITCODE_BYTES,
    `rejected deployment initcode exceeds the ${MAX_INITCODE_BYTES}-byte ceiling`,
  );
  assert(
    Buffer.from(initcodeBytes.subarray(0, creationBytes.length)).equals(Buffer.from(creationBytes)),
    "rejected deployment did not use the authenticated creation bytecode prefix",
  );
  await assert.rejects(
    signer.call({ ...transaction, gasLimit: MAX_DEPLOYMENT_GAS }),
    rejectedWith(reason),
  );
}

async function nextCreateAddress(signer, offset = 0) {
  const nonce = Number(
    BigInt(
      await signer.provider.send("eth_getTransactionCount", [
        await signer.getAddress(),
        "pending",
      ]),
    ),
  );
  return ethers.getCreateAddress({
    from: await signer.getAddress(),
    nonce: nonce + offset,
  });
}

async function deployPreboundRoute(
  signer,
  tokenArtifact,
  routeArtifact,
  routeArgs,
) {
  const routeAddress = await nextCreateAddress(signer, 1);
  const token = await deploy(signer, tokenArtifact, [routeAddress]);
  assert.equal(await token.decimals(), 18n);
  assert.equal(await token.totalSupply(), 0n);
  assert.equal(
    await nextCreateAddress(signer),
    routeAddress,
    "token deployment must leave the precomputed route address next",
  );
  const route = await deploy(signer, routeArtifact, [
    await token.getAddress(),
    ...routeArgs,
  ]);
  assert.equal(
    await route.getAddress(),
    routeAddress,
    "route deployment address drifted from the token's immutable binding",
  );
  assert.equal(await token.bridge(), routeAddress);
  assert.equal(await route.token(), await token.getAddress());
  return { route, token };
}

function le(value, bytes) {
  let current = BigInt(value);
  const out = Buffer.alloc(bytes);
  for (let i = 0; i < bytes; i++) {
    out[i] = Number(current & 0xffn);
    current >>= 8n;
  }
  if (current !== 0n) throw new RangeError(`${value} exceeds ${bytes} bytes`);
  return out;
}

function vec(value) {
  const bytes = Buffer.from(value);
  return Buffer.concat([le(bytes.length, 4), bytes]);
}

function transferPayload({
  sourceDomain,
  destinationDomain,
  nonce,
  routeRevision = ROUTE_REVISION,
  amount,
  senderCodec,
  sender,
  recipientCodec,
  recipient,
  route = "taira_bsc_xor",
}) {
  return Buffer.concat([
    Buffer.from([2, 1]),
    le(sourceDomain, 4),
    le(destinationDomain, 4),
    le(nonce, 8),
    le(routeRevision, 4),
    le(DOMAIN_SORA, 4),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from("xor")),
    le(amount, 16),
    Buffer.from([senderCodec]),
    vec(sender),
    Buffer.from([recipientCodec]),
    vec(recipient),
    Buffer.from([CODEC_TEXT]),
    vec(Buffer.from(route)),
  ]);
}

function network(profile) {
  if (profile === "sora-taira") {
    return Buffer.from("010100000000fc56984b2be7431d840e21514d1883f0", "hex");
  }
  if (profile === "sora-nexus") {
    return Buffer.from("01000000000000000000000000000000000000000753", "hex");
  }
  if (profile === "bsc-mainnet")
    return Buffer.concat([Buffer.from([1, 4]), le(2, 4), le(56, 8)]);
  if (profile === "bsc-testnet")
    return Buffer.concat([Buffer.from([1, 5]), le(2, 4), le(97, 8)]);
  if (profile === "ethereum-mainnet")
    return Buffer.concat([Buffer.from([1, 2]), le(1, 4), le(1, 8)]);
  if (profile === "ethereum-sepolia") {
    return Buffer.concat([Buffer.from([1, 3]), le(1, 4), le(11_155_111, 8)]);
  }
  if (profile === "tron-nile") {
    return Buffer.concat([
      Buffer.from([1, TRON_NILE_PROFILE]),
      le(DOMAIN_TRON, 4),
      le(0xcd8690dc, 4),
    ]);
  }
  throw new Error(`unsupported profile ${profile}`);
}

function lane(sourceProfile, targetProfile) {
  return Buffer.concat([
    Buffer.from([1]),
    vec(network(sourceProfile)),
    vec(network(targetProfile)),
  ]);
}

function messageId(sourceProfile, targetProfile, payload) {
  return ethers.keccak256(
    ethers.concat([
      ethers.toUtf8Bytes("sccp:lane-message-id:v1"),
      "0x01",
      vec(lane(sourceProfile, targetProfile)),
      vec(payload),
    ]),
  );
}

function word(value) {
  return ethers.zeroPadValue(ethers.toBeHex(value), 32);
}

function verifierPolicy(
  verifierAddress,
  verifierCodeHash,
  verifierKeyHash,
  semanticProofProfileHash = SEMANTIC_PROOF_PROFILE_HASH,
  soraFinalityAnchorHash = SORA_FINALITY_ANCHOR_HASH,
) {
  return [
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
    semanticProofProfileHash,
    soraFinalityAnchorHash,
  ];
}

function exactEvmRouteConfigHash({
  abi,
  domain,
  profile,
  chainId,
  sourceLaneHash,
  destinationLaneHash,
  tokenAddress,
  tokenCodeHash,
  verifierAddress,
  verifierCodeHash,
  verifierKeyHash,
  semanticProofProfileHash,
  soraFinalityAnchorHash,
  route,
  routeRevision,
  maxWrappedSupply,
}) {
  const deploymentConfigHash = ethers.keccak256(
    abi.encode(
      [
        "address",
        "bytes32",
        "address",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        tokenAddress,
        tokenCodeHash,
        verifierAddress,
        verifierCodeHash,
        verifierKeyHash,
        semanticProofProfileHash,
        soraFinalityAnchorHash,
      ],
    ),
  );
  const assetRouteConfigHash = ethers.keccak256(
    abi.encode(
      ["bytes32", "bytes32", "uint32", "uint256", "uint256"],
      [
        ethers.keccak256(ethers.toUtf8Bytes("xor")),
        ethers.keccak256(ethers.toUtf8Bytes(route)),
        routeRevision,
        SCALE,
        maxWrappedSupply,
      ],
    ),
  );
  return ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "uint32",
        "uint8",
        "uint256",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(ethers.toUtf8Bytes("sccp:concrete-route-config:v1")),
        domain,
        profile,
        chainId,
        sourceLaneHash,
        destinationLaneHash,
        deploymentConfigHash,
        assetRouteConfigHash,
      ],
    ),
  );
}

function exactTronRouteConfigHash({
  abi,
  profile,
  networkId,
  sourceLaneHash,
  destinationLaneHash,
  tokenAddress,
  tokenCodeHash,
  verifierAddress,
  verifierCodeHash,
  verifierKeyHash,
  semanticProofProfileHash,
  soraFinalityAnchorHash,
  destinationBindingHash,
  routeRevision,
  maxWrappedSupply,
}) {
  const deploymentConfigHash = ethers.keccak256(
    abi.encode(
      [
        "address",
        "bytes32",
        "address",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        tokenAddress,
        tokenCodeHash,
        verifierAddress,
        verifierCodeHash,
        verifierKeyHash,
        semanticProofProfileHash,
        soraFinalityAnchorHash,
        destinationBindingHash,
      ],
    ),
  );
  const assetRouteConfigHash = ethers.keccak256(
    abi.encode(
      ["bytes32", "bytes32", "uint32", "uint256", "uint256"],
      [
        ethers.keccak256(ethers.toUtf8Bytes("xor")),
        ethers.keccak256(ethers.toUtf8Bytes("taira_tron_xor")),
        routeRevision,
        SCALE,
        maxWrappedSupply,
      ],
    ),
  );
  return ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "uint32",
        "uint8",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(ethers.toUtf8Bytes("sccp:concrete-route-config:v1")),
        DOMAIN_TRON,
        profile,
        networkId,
        sourceLaneHash,
        destinationLaneHash,
        deploymentConfigHash,
        assetRouteConfigHash,
      ],
    ),
  );
}

function exactTronDestinationBindingHash({
  abi,
  networkId,
  verifierAddress,
  bridgeAddress,
  verifierCodeHash,
  verifierKeyHash,
  semanticProofProfileHash,
  soraFinalityAnchorHash,
}) {
  const tronAddressWord = (address) =>
    ethers.zeroPadValue(ethers.concat(["0x41", address]), 32);
  return ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "bytes32",
        "bytes32",
        "uint256",
        "uint256",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:tron-destination-binding:v1"),
        ),
        ethers.keccak256(ethers.toUtf8Bytes("tron-groth16-bn254-v1")),
        networkId,
        DOMAIN_SORA,
        DOMAIN_TRON,
        tronAddressWord(verifierAddress),
        tronAddressWord(bridgeAddress),
        verifierCodeHash,
        verifierKeyHash,
        semanticProofProfileHash,
        soraFinalityAnchorHash,
      ],
    ),
  );
}

function exactEvmDestinationBindingHash({
  abi,
  chainId,
  targetDomain,
  verifierAddress,
  bridgeAddress,
  verifierCodeHash,
  verifierKeyHash,
  semanticProofProfileHash,
  soraFinalityAnchorHash,
}) {
  return ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "bytes32",
        "bytes32",
        "uint256",
        "uint256",
        "address",
        "address",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:evm-destination-binding:v1"),
        ),
        ethers.keccak256(ethers.toUtf8Bytes("evm-groth16-bn254-v1")),
        word(chainId),
        DOMAIN_SORA,
        targetDomain,
        verifierAddress,
        bridgeAddress,
        verifierCodeHash,
        verifierKeyHash,
        semanticProofProfileHash,
        soraFinalityAnchorHash,
      ],
    ),
  );
}

function signalWords(
  abi,
  publicInputs,
  statementHash,
  binding,
  routeConfigurationHash,
  soraFinalityAnchorHash,
) {
  const values = [
    ...publicInputs.slice(0, 6),
    word(DOMAIN_SORA),
    statementHash,
    binding,
    routeConfigurationHash,
    soraFinalityAnchorHash,
  ];
  return values.map(
    (value, index) =>
      BigInt(
        ethers.keccak256(
          abi.encode(["bytes32", "bytes32"], [SIGNAL_LABELS[index], value]),
        ),
      ) % SCALAR_FIELD,
  );
}

async function scalarMul(provider, abi, point, scalar) {
  const result = await provider.call({
    to: "0x0000000000000000000000000000000000000007",
    data: abi.encode(
      ["uint256", "uint256", "uint256"],
      [point[0], point[1], scalar],
    ),
  });
  return Array.from(abi.decode(["uint256", "uint256"], result));
}

async function acceptingProof(
  provider,
  abi,
  publicInputs,
  statementHash,
  binding,
  routeConfigurationHash,
  soraFinalityAnchorHash,
  g1,
  g2,
) {
  const scalar = signalWords(
    abi,
    publicInputs,
    statementHash,
    binding,
    routeConfigurationHash,
    soraFinalityAnchorHash,
  ).reduce((sum, value) => (sum + value) % SCALAR_FIELD, 1n);
  const vkX = await scalarMul(provider, abi, g1, scalar);
  const c = [vkX[0], (BASE_FIELD - (vkX[1] % BASE_FIELD)) % BASE_FIELD];
  return abi.encode(
    GROTH16_PROOF_ABI_TYPES,
    [1, publicInputs[0], DOMAIN_SORA, publicInputs[3], g1, g2, c],
  );
}

async function assertDynamicVerifierRoleCollisionsRejected({
  verifier,
  provider,
  abi,
  publicInputs,
  statement,
  destination,
  route,
  g1,
  g2,
}) {
  const roles = {
    statement,
    destination,
    route,
    key: await verifier.verifyingKeyHash(),
    semantic: await verifier.semanticProofProfileHash(),
    anchor: await verifier.soraFinalityAnchorHash(),
  };
  const labels = [
    "statement",
    "destination",
    "route",
    "key",
    "semantic",
    "anchor",
  ];
  assert.equal(new Set(Object.values(roles)).size, labels.length);
  for (let dynamicIndex = 0; dynamicIndex < 3; dynamicIndex++) {
    for (
      let collidingIndex = dynamicIndex + 1;
      collidingIndex < labels.length;
      collidingIndex++
    ) {
      const colliding = { ...roles };
      colliding[labels[dynamicIndex]] = colliding[labels[collidingIndex]];
      const matchingProof = await acceptingProof(
        provider,
        abi,
        publicInputs,
        colliding.statement,
        colliding.destination,
        colliding.route,
        colliding.anchor,
        g1,
        g2,
      );
      await assert.rejects(
        verifier.verifySccpMessageProof(
          matchingProof,
          publicInputs,
          colliding.statement,
          colliding.destination,
          colliding.route,
        ),
        rejectedWith("Protocol hash roles must differ"),
        `${labels[dynamicIndex]} must not alias ${labels[collidingIndex]}`,
      );
    }
  }
}

function mutateGroth16Proof(abi, proofBytes, overrides) {
  const decoded = abi.decode(GROTH16_PROOF_ABI_TYPES, proofBytes);
  const field = (name, index) =>
    Object.prototype.hasOwnProperty.call(overrides, name)
      ? overrides[name]
      : decoded[index];
  return abi.encode(GROTH16_PROOF_ABI_TYPES, [
    field("version", 0),
    field("messageId", 1),
    field("sourceDomain", 2),
    field("commitmentRoot", 3),
    field("a", 4),
    field("b", 5),
    field("c", 6),
  ]);
}

function rejectedWith(reason) {
  return (error) => {
    const candidates = [
      error,
      error?.error,
      error?.info,
      error?.info?.error,
      error?.cause,
      error?.cause?.error,
    ].filter((value) => value && typeof value === "object");
    const text = candidates
      .flatMap((value) => [value.reason, value.shortMessage, value.message])
      .filter(Boolean)
      .join("\n");
    const codes = candidates.map((value) => value.code).filter(Boolean);
    const revertData = candidates
      .map((value) => value.data)
      .find((value) => typeof value === "string" && /^0x[0-9a-fA-F]*$/.test(value));
    const isRevert =
      codes.includes("CALL_EXCEPTION") ||
      (codes.some((code) => code === 3 || code === "3") &&
        (/revert|VM Exception/i.test(text) || revertData !== undefined));
    if (!isRevert) return false;
    if (!reason) return true;
    if (text.includes(reason)) return true;
    if (revertData?.startsWith("0x08c379a0")) {
      try {
        const [decoded] = ethers.AbiCoder.defaultAbiCoder().decode(
          ["string"],
          `0x${revertData.slice(10)}`,
        );
        return decoded === reason;
      } catch (_error) {
        return false;
      }
    }
    return false;
  };
}

async function main() {
  const {
    evmContracts: contracts,
    tvmContracts,
    mockContracts,
    runtimeSizes,
  } = compile();
  const bscEip1193Provider = createHardhatProvider({
    chainId: 97,
    blockGasLimit: Number(MAX_DEPLOYMENT_GAS + 5_000_000n),
  });
  const provider = new ethers.BrowserProvider(bscEip1193Provider);
  assert.equal((await provider.getNetwork()).chainId, 97n);
  const signer = await provider.getSigner(0);
  const outsider = await provider.getSigner(1);
  const abi = ethers.AbiCoder.defaultAbiCoder();

  const verifierArtifact = artifact(
    contracts,
    "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol",
    "SccpGroth16Bn254MessageVerifier",
  );
  const tokenArtifact = artifact(
    contracts,
    "contracts/bsc/sccp/TairaXOR.sol",
    "TairaXOR",
  );
  const bridgeArtifact = artifact(
    contracts,
    "contracts/bsc/sccp/TairaXorBscSccpBridge.sol",
    "TairaXorBscSccpBridge",
  );
  const ethereumTokenArtifact = artifact(
    contracts,
    "contracts/ethereum/sccp/TairaXOR.sol",
    "TairaXOR",
  );
  const ethereumBridgeArtifact = artifact(
    contracts,
    "contracts/ethereum/sccp/TairaXorEthereumSccpBridge.sol",
    "TairaXorEthereumSccpBridge",
  );
  const tronVerifierArtifact = artifact(
    tvmContracts,
    "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    "SccpTronGroth16Bn254MessageVerifier",
  );
  const tronTokenArtifact = artifact(
    tvmContracts,
    "contracts/tron/sccp/TairaXOR.sol",
    "TairaXOR",
  );
  const tronBridgeArtifact = artifact(
    tvmContracts,
    "contracts/tron/sccp/TairaXorSccpBridge.sol",
    "TairaXorSccpBridge",
  );
  const falseTokenArtifact = artifact(mockContracts, "Mocks.sol", "FalseToken");
  const wrongDecimalsTokenArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "WrongDecimalsToken",
  );
  const nonzeroSupplyTokenArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "NonzeroSupplyToken",
  );
  const trueNoopTokenArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "TrueNoopToken",
  );
  const wrongDeltaTokenArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "WrongDeltaToken",
  );
  const reentrantTokenArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "ReentrantToken",
  );
  const codecHarnessArtifact = artifact(mockContracts, "Mocks.sol", "CodecHarness");
  const tokenRouteHarnessArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "TokenRouteHarness",
  );
  const codeAliasedVerifierArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "CodeAliasedVerifier",
  );
  const injectedBscRouteArtifact = artifact(
    mockContracts,
    "Mocks.sol",
    "InjectedBscRouteHarness",
  );

  for (const [label, deploymentArtifact] of [
    ["EVM BN254 verifier", verifierArtifact],
    ["TRON BN254 verifier", tronVerifierArtifact],
    ["BSC token", tokenArtifact],
    ["Ethereum token", ethereumTokenArtifact],
    ["TRON token", tronTokenArtifact],
    ["BSC", bridgeArtifact],
    ["Ethereum", ethereumBridgeArtifact],
    ["TRON", tronBridgeArtifact],
  ]) {
    assert(
      ethers.getBytes(deploymentArtifact.runtimeBytecode).length <=
        MAX_RUNTIME_BYTES,
      `${label} runtime exceeds the ${MAX_RUNTIME_BYTES}-byte deployment ceiling`,
    );
  }

  for (const exactTokenArtifact of [
    tokenArtifact,
    ethereumTokenArtifact,
    tronTokenArtifact,
  ]) {
    for (const forbiddenEntrypoint of [
      "owner",
      "transferOwnership",
      "setBridge",
      "lockBridge",
      "bridgeLocked",
    ]) {
      assert(
        !exactTokenArtifact.abi.some(
          (entry) => entry.name === forbiddenEntrypoint,
        ),
        `exact token unexpectedly exposes ${forbiddenEntrypoint}`,
      );
    }
    await assertConstructorRevertsWith(
      signer,
      exactTokenArtifact,
      [ethers.ZeroAddress],
      "Bridge address is required",
    );
    const preboundToken = await deploy(signer, exactTokenArtifact, [
      await outsider.getAddress(),
    ]);
    assert.equal(await preboundToken.bridge(), await outsider.getAddress());
    assert.equal(await preboundToken.decimals(), 18n);
    assert.equal(await preboundToken.totalSupply(), 0n);
  }

  for (const [label, exactRouteArtifact] of [
    ["BSC", bridgeArtifact],
    ["Ethereum", ethereumBridgeArtifact],
    ["TRON", tronBridgeArtifact],
  ]) {
    const constructor = exactRouteArtifact.abi.find(
      (entry) => entry.type === "constructor",
    );
    assert(constructor, `${label} route constructor ABI is missing`);
    assert.equal(
      constructor.inputs.length,
      5,
      `${label} route constructor must accept token, verifier policy, profile, revision, and cap`,
    );
    assert.equal(constructor.inputs[0].name, "tokenAddress");
    assert.equal(constructor.inputs[0].type, "address");
    assert.equal(constructor.inputs[1].name, "configuredVerifierPolicy");
    assert.equal(constructor.inputs[1].type, "tuple");
    assert.deepEqual(
      constructor.inputs[1].components.map(({ name, type }) => [name, type]),
      [
        ["verifierAddress", "address"],
        ["verifierCodeHash", "bytes32"],
        ["verifierKeyHash", "bytes32"],
        ["semanticProofProfileHash", "bytes32"],
        ["soraFinalityAnchorHash", "bytes32"],
      ],
      `${label} route verifier policy ABI drifted`,
    );
    assert.equal(constructor.inputs[4].name, "configuredMaxWrappedSupply");
    assert.equal(constructor.inputs[4].type, "uint256");
    const maxWrappedSupplyGetter = exactRouteArtifact.abi.find(
      (entry) => entry.name === "maxWrappedSupply",
    );
    assert(maxWrappedSupplyGetter, `${label} immutable wrapped-supply cap getter is missing`);
    assert.equal(maxWrappedSupplyGetter.outputs[0].type, "uint256");
  }

  assert(!bridgeArtifact.abi.some((entry) => entry.name === "burnToTaira"));
  assert(!bridgeArtifact.abi.some((entry) => entry.name === "transferNonce"));
  assert(!ethereumBridgeArtifact.abi.some((entry) => entry.name === "transferNonce"));
  for (const exactRouteArtifact of [bridgeArtifact, ethereumBridgeArtifact]) {
    const nonceGetter = exactRouteArtifact.abi.find(
      (entry) => entry.name === "transferNonces",
    );
    assert(nonceGetter, "per-sender EVM nonce getter is missing");
    assert.deepEqual(
      nonceGetter.inputs.map(({ type }) => type),
      ["address"],
    );
  }
  assert(
    !bridgeArtifact.abi.some((entry) => entry.name === "submitSccpSourceEvent"),
  );
  assert(
    !ethereumBridgeArtifact.abi.some((entry) => entry.name === "burnToTaira"),
  );
  assert(
    !ethereumBridgeArtifact.abi.some(
      (entry) => entry.name === "submitSccpSourceEvent",
    ),
  );
  assert(!tronBridgeArtifact.abi.some((entry) => entry.name === "burnToTaira"));
  assert(
    !tronBridgeArtifact.abi.some(
      (entry) => entry.name === "submitSccpSourceEvent",
    ),
  );
  assert(
    !tronVerifierArtifact.abi.some(
      (entry) => entry.name === "submitSccpMessageProof",
    ),
  );
  assert(
    !tronVerifierArtifact.abi.some(
      (entry) => entry.name === "usedMessageProofs",
    ),
  );
  assert(
    !tronVerifierArtifact.abi.some(
      (entry) => entry.name === "destinationBindingHash",
    ),
  );
  assert(
    !Object.values(contracts).some((file) => file.SccpSecp256k1MessageVerifier),
  );
  for (const retiredSource of [
    "contracts/evm/sccp/SccpEvmSourceBridge.sol",
    "contracts/bsc/sccp/SccpBscSourceBridge.sol",
    "contracts/tron/sccp/SccpTronSourceBridge.sol",
    "contracts/ethereum/sccp/SccpEthereumSourceBridge.sol",
  ]) {
    assert(
      !fs.existsSync(path.join(REPO, retiredSource)),
      `${retiredSource} must stay deleted`,
    );
  }
  const sourceEvent = bridgeArtifact.abi.find(
    (entry) => entry.type === "event" && entry.name === "SccpTransfer",
  );
  assert.deepEqual(
    sourceEvent.inputs.map((input) => input.indexed),
    [true, true, true, false, false, false],
  );

  const g1 = [1n, 2n];
  const g2 = [
    10857046999023057135944570762232829481370756359578518086990519993285655852781n,
    11559732032986387107991004021392285783925812861821192530917403151452391805634n,
    8495653923123431417604973247489272438418190587263600148770280649306958101930n,
    4082367875863433681332203403145435568316851327593401208105741076214120093531n,
  ];
  const configuredIc = Array(12).fill(g1).flat();
  const constructorAdversaries = [
    {
      label: "truncated IC",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: Array(11).fill(g1).flat(),
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "Unexpected verifying key input count",
    },
    {
      label: "trailing IC",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: Array(13).fill(g1).flat(),
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "Unexpected verifying key input count",
    },
    {
      label: "zero G1 alpha",
      alpha: [0n, 0n],
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G1 point is zero",
    },
    {
      label: "out-of-field G1 alpha",
      alpha: [BASE_FIELD, 2n],
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G1 point out of range",
    },
    {
      label: "off-curve G1 alpha",
      alpha: [1n, 3n],
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G1 scalar multiplication failed",
    },
    {
      label: "zero G2 beta",
      alpha: g1,
      beta: [0n, 0n, 0n, 0n],
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G2 point is zero",
    },
    {
      label: "out-of-field G2 gamma",
      alpha: g1,
      beta: g2,
      gamma: [BASE_FIELD, g2[1], g2[2], g2[3]],
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G2 point out of range",
    },
    {
      label: "off-curve G2 delta",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: [1n, 2n, 3n, 4n],
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "Pairing precompile failed",
    },
    {
      label: "non-subgroup G2 beta",
      alpha: g1,
      beta: NON_SUBGROUP_G2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "Pairing precompile failed",
    },
    {
      label: "zero first IC point",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: [0n, 0n, ...configuredIc.slice(2)],
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G1 point is zero",
    },
    {
      label: "off-curve final IC point",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: [...configuredIc.slice(0, -2), 1n, 3n],
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "G1 scalar multiplication failed",
    },
    {
      label: "zero semantic profile",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: ethers.ZeroHash,
      anchor: SORA_FINALITY_ANCHOR_HASH,
      reason: "Semantic proof profile hash is required",
    },
    {
      label: "zero finality anchor",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: ethers.ZeroHash,
      reason: "SORA finality anchor hash is required",
    },
    {
      label: "aliased semantic and anchor",
      alpha: g1,
      beta: g2,
      gamma: g2,
      delta: g2,
      ic: configuredIc,
      semantic: SEMANTIC_PROOF_PROFILE_HASH,
      anchor: SEMANTIC_PROOF_PROFILE_HASH,
      reason: "Semantic profile and finality anchor must differ",
    },
  ];
  for (const adversary of constructorAdversaries) {
    await assertConstructorRevertsWith(
      signer,
      verifierArtifact,
      [
        adversary.alpha,
        adversary.beta,
        adversary.gamma,
        adversary.delta,
        adversary.ic,
        adversary.semantic,
        adversary.anchor,
      ],
      adversary.reason,
    ).catch((error) => {
      error.message = `${adversary.label}: ${error.message}`;
      throw error;
    });
  }
  const codecHarness = await deploy(signer, codecHarnessArtifact);
  const invalidTairaRecipients = invalidI105Values();
  const invalidTairaSenders = invalidTairaSenderValues();
  const validTairaSenders = universalTairaSenders();
  assert.equal(
    await codecHarness.isTairaRecipient(CANONICAL_I105_BYTES),
    true,
  );
  assert.equal(await codecHarness.isTairaAccount(CANONICAL_I105_BYTES), true);
  for (const sender of validTairaSenders) {
    assert.equal(await codecHarness.isTairaAccount(sender), true);
    assert.equal(
      await codecHarness.isTairaRecipient(sender),
      false,
      "multisig and non-Ed25519 accounts must not become irreversible burn recipients",
    );
  }
  for (const invalidRecipient of invalidTairaRecipients) {
    assert.equal(
      await codecHarness.isTairaRecipient(invalidRecipient),
      false,
      `hostile Taira recipient was accepted: ${ethers.hexlify(invalidRecipient)}`,
    );
  }
  for (const invalidSender of invalidTairaSenders) {
    assert.equal(
      await codecHarness.isTairaAccount(invalidSender),
      false,
      `hostile Taira sender was accepted: ${ethers.hexlify(invalidSender)}`,
    );
  }
  const tokenRouteHarness = await deploy(signer, tokenRouteHarnessArtifact);
  const arithmeticToken = new ethers.Contract(
    await tokenRouteHarness.token(),
    tokenArtifact.abi,
    signer,
  );
  const arithmeticAccount = await signer.getAddress();
  await (await tokenRouteHarness.mint(arithmeticAccount, ethers.MaxUint256)).wait();
  assert.equal(await arithmeticToken.totalSupply(), ethers.MaxUint256);
  assert.equal(
    await arithmeticToken.balanceOf(arithmeticAccount),
    ethers.MaxUint256,
  );
  await assert.rejects(
    tokenRouteHarness.mint(arithmeticAccount, 1n),
    rejectedWith(),
  );
  assert.equal(await arithmeticToken.totalSupply(), ethers.MaxUint256);
  assert.equal(
    await arithmeticToken.balanceOf(arithmeticAccount),
    ethers.MaxUint256,
  );
  await assert.rejects(
    arithmeticToken.mint(arithmeticAccount, 1n),
    rejectedWith("Caller is not the bridge"),
  );
  await (await tokenRouteHarness.burn(arithmeticAccount, ethers.MaxUint256)).wait();
  assert.equal(await arithmeticToken.totalSupply(), 0n);
  assert.equal(await arithmeticToken.balanceOf(arithmeticAccount), 0n);
  await assert.rejects(
    tokenRouteHarness.burn(arithmeticAccount, 1n),
    rejectedWith("Uint256 underflow"),
  );
  assert.equal(await arithmeticToken.totalSupply(), 0n);
  assert.equal(await arithmeticToken.balanceOf(arithmeticAccount), 0n);
  const nativeVectors = JSON.parse(
    fs.readFileSync(
      path.join(REPO, "fixtures/sccp/native_transfer_event_v1.json"),
      "utf8",
    ),
  );
  assert.equal(nativeVectors.version, 1);
  assert.equal(nativeVectors.vectors.length, 7);
  for (const vector of nativeVectors.vectors) {
    const result = await codecHarness.sourceVector(
      PROFILE_TAG[vector.source_profile],
      `0x${vector.canonical_payload_hex}`,
    );
    assert.equal(result[0], `0x${vector.canonical_lane_hex}`);
    assert.equal(result[1], `0x${vector.lane_hash_hex}`);
    assert.equal(result[2], `0x${vector.message_id_hex}`);
    assert.equal(result[3], `0x${vector.payload_hash_hex}`);
    assert.equal(result[4], `0x${vector.source_event_digest_hex}`);
    const evmHashes = await codecHarness.evmHashVector(
      PROFILE_TAG[vector.source_profile],
      `0x${vector.canonical_payload_hex}`,
    );
    assert.equal(evmHashes[0], result[1]);
    assert.equal(evmHashes[1], result[3]);
  }
  for (const length of [0, 1, 63, 127, 128, 129, 255, 256, 257, 511]) {
    const input = Buffer.from(
      Array.from({ length }, (_, index) => (index * 197 + length) & 0xff),
    );
    const hashes = await codecHarness.rawBlakeParity(input);
    assert.equal(
      hashes[1],
      hashes[0],
      `EIP-152 BLAKE2b parity failed for ${length} bytes`,
    );
  }
  const verifier = await deploy(signer, verifierArtifact, [
    g1,
    g2,
    g2,
    g2,
    configuredIc,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
  ]);
  const verifierAddress = await verifier.getAddress();
  const verifierCodeHash = ethers.keccak256(
    await provider.getCode(verifierAddress),
  );
  const verifierKeyHash = await verifier.verifyingKeyHash();
  await assert.rejects(
    deploy(signer, verifierArtifact, [
      g1,
      g2,
      g2,
      g2,
      configuredIc,
      verifierKeyHash,
      SORA_FINALITY_ANCHOR_HASH,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, verifierArtifact, [
      g1,
      g2,
      g2,
      g2,
      configuredIc,
      SEMANTIC_PROOF_PROFILE_HASH,
      verifierKeyHash,
    ]),
    rejectedWith(),
  );
  assert.equal(
    await verifier.semanticProofProfileHash(),
    SEMANTIC_PROOF_PROFILE_HASH,
  );
  assert.equal(
    await verifier.soraFinalityAnchorHash(),
    SORA_FINALITY_ANCHOR_HASH,
  );

  const tronNetworkId = word(0xcd8690dc);
  await assertConstructorRevertsWith(
    signer,
    tronVerifierArtifact,
    [
      g1,
      g2,
      g2,
      g2,
      configuredIc,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      verifierKeyHash,
      tronNetworkId,
      DOMAIN_SORA,
      DOMAIN_TRON,
    ],
    undefined,
  );
  await assertConstructorRevertsWith(
    signer,
    tronBridgeArtifact,
    [
      await outsider.getAddress(),
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ],
    undefined,
  );

  {
  const tronEip1193Provider = createHardhatProvider({
    chainId: 0xcd8690dc,
    blockGasLimit: Number(MAX_DEPLOYMENT_GAS + 5_000_000n),
  });
  const provider = new ethers.BrowserProvider(tronEip1193Provider);
  assert.equal((await provider.getNetwork()).chainId, 0xcd8690dcn);
  const signer = await provider.getSigner(0);
  const tronOutsider = await provider.getSigner(1);
  const tronVerifier = await deploy(signer, tronVerifierArtifact, [
    g1,
    g2,
    g2,
    g2,
    Array(12).fill(g1).flat(),
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    verifierKeyHash,
    tronNetworkId,
    DOMAIN_SORA,
    DOMAIN_TRON,
  ]);
  const tronVerifierAddress = await tronVerifier.getAddress();
  const tronVerifierCodeHash = ethers.keccak256(
    await provider.getCode(tronVerifierAddress),
  );
  assert.equal(await tronVerifier.verifierCodeHash(), tronVerifierCodeHash);
  assert.equal(await tronVerifier.verifyingKeyHash(), verifierKeyHash);
  assert.equal(
    await tronVerifier.semanticProofProfileHash(),
    SEMANTIC_PROOF_PROFILE_HASH,
  );
  assert.equal(
    await tronVerifier.soraFinalityAnchorHash(),
    SORA_FINALITY_ANCHOR_HASH,
  );

  const tronWrongDecimalsRouteAddress = await nextCreateAddress(signer, 1);
  const tronWrongDecimalsToken = await deploy(signer, wrongDecimalsTokenArtifact, [
    tronWrongDecimalsRouteAddress,
  ]);
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      await tronWrongDecimalsToken.getAddress(),
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith("Unexpected token decimals"),
  );
  const tronNonzeroSupplyRouteAddress = await nextCreateAddress(signer, 1);
  const tronNonzeroSupplyToken = await deploy(signer, nonzeroSupplyTokenArtifact, [
    tronNonzeroSupplyRouteAddress,
  ]);
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      await tronNonzeroSupplyToken.getAddress(),
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith("Token supply must start at zero"),
  );

  const tronRoute = await deployPreboundRoute(
    signer,
    tronTokenArtifact,
    tronBridgeArtifact,
    [
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ],
  );
  const tronBridge = tronRoute.route;
  const tronBridgeAddress = await tronBridge.getAddress();
  const tronToken = tronRoute.token;
  const tronTokenAddress = await tronToken.getAddress();
  const tronTokenCodeHash = ethers.keccak256(
    await provider.getCode(tronTokenAddress),
  );
  assert.equal(await tronToken.bridge(), tronBridgeAddress);
  assert.equal(await tronBridge.networkId(), tronNetworkId);
  assert.equal(await tronBridge.routeRevision(), BigInt(ROUTE_REVISION));
  assert.equal(await tronBridge.maxWrappedSupply(), MAX_WRAPPED_SUPPLY);
  const secondTronRoute = await deployPreboundRoute(
    signer,
    tronTokenArtifact,
    tronBridgeArtifact,
    [
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ],
  );
  const secondTronBridge = secondTronRoute.route;
  const secondTronBridgeAddress = await secondTronBridge.getAddress();
  const secondTronToken = secondTronRoute.token;
  assert.equal(await secondTronToken.bridge(), secondTronBridgeAddress);
  assert.equal(await secondTronBridge.maxWrappedSupply(), MAX_WRAPPED_SUPPLY);
  const rejectedTronRouteAddress = await nextCreateAddress(signer, 1);
  const rejectedTronToken = await deploy(signer, tronTokenArtifact, [
    rejectedTronRouteAddress,
  ]);
  const rejectedTronTokenAddress = await rejectedTronToken.getAddress();
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      rejectedTronTokenAddress,
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      0,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      rejectedTronTokenAddress,
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      0,
    ]),
    rejectedWith("Invalid wrapped supply cap"),
  );
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      rejectedTronTokenAddress,
      verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_U128 + 1n,
    ]),
    rejectedWith("Invalid wrapped supply cap"),
  );
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      rejectedTronTokenAddress,
      verifierPolicy(
        tronVerifierAddress,
        tronVerifierCodeHash,
        verifierKeyHash,
        ALTERNATE_SEMANTIC_PROOF_PROFILE_HASH,
      ),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      rejectedTronTokenAddress,
      verifierPolicy(
        tronVerifierAddress,
        tronVerifierCodeHash,
        verifierKeyHash,
        SEMANTIC_PROOF_PROFILE_HASH,
        ALTERNATE_SORA_FINALITY_ANCHOR_HASH,
      ),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  const tronCodeAliasedVerifier = await deploy(
    signer,
    codeAliasedVerifierArtifact,
    [tronNetworkId, true],
  );
  const tronCodeAliasedVerifierAddress =
    await tronCodeAliasedVerifier.getAddress();
  const tronCodeAliasedVerifierCodeHash = ethers.keccak256(
    await provider.getCode(tronCodeAliasedVerifierAddress),
  );
  assert.equal(
    await tronCodeAliasedVerifier.soraFinalityAnchorHash(),
    tronCodeAliasedVerifierCodeHash,
  );
  const aliasedTronRouteAddress = await nextCreateAddress(signer, 1);
  const aliasedTronToken = await deploy(signer, tronTokenArtifact, [
    aliasedTronRouteAddress,
  ]);
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      await aliasedTronToken.getAddress(),
      verifierPolicy(
        tronCodeAliasedVerifierAddress,
        tronCodeAliasedVerifierCodeHash,
        await tronCodeAliasedVerifier.verifyingKeyHash(),
        await tronCodeAliasedVerifier.semanticProofProfileHash(),
        await tronCodeAliasedVerifier.soraFinalityAnchorHash(),
      ),
      TRON_NILE_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  const tronDestinationBinding = await tronBridge.destinationBindingHash();
  const secondTronDestinationBinding =
    await secondTronBridge.destinationBindingHash();
  assert.notEqual(tronDestinationBinding, secondTronDestinationBinding);
  assert.equal(
    tronDestinationBinding,
    exactTronDestinationBindingHash({
      abi,
      networkId: tronNetworkId,
      verifierAddress: tronVerifierAddress,
      bridgeAddress: tronBridgeAddress,
      verifierCodeHash: tronVerifierCodeHash,
      verifierKeyHash,
      semanticProofProfileHash: SEMANTIC_PROOF_PROFILE_HASH,
      soraFinalityAnchorHash: SORA_FINALITY_ANCHOR_HASH,
    }),
  );
  assert.equal(
    secondTronDestinationBinding,
    exactTronDestinationBindingHash({
      abi,
      networkId: tronNetworkId,
      verifierAddress: tronVerifierAddress,
      bridgeAddress: secondTronBridgeAddress,
      verifierCodeHash: tronVerifierCodeHash,
      verifierKeyHash,
      semanticProofProfileHash: SEMANTIC_PROOF_PROFILE_HASH,
      soraFinalityAnchorHash: SORA_FINALITY_ANCHOR_HASH,
    }),
  );
  assert.equal(
    await tronBridge.routeConfigHash(),
    exactTronRouteConfigHash({
      abi,
      profile: TRON_NILE_PROFILE,
      networkId: tronNetworkId,
      sourceLaneHash: await tronBridge.sourceLaneHash(),
      destinationLaneHash: await tronBridge.destinationLaneHash(),
      tokenAddress: tronTokenAddress,
      tokenCodeHash: tronTokenCodeHash,
      verifierAddress: tronVerifierAddress,
      verifierCodeHash: tronVerifierCodeHash,
      verifierKeyHash,
      semanticProofProfileHash: SEMANTIC_PROOF_PROFILE_HASH,
      soraFinalityAnchorHash: SORA_FINALITY_ANCHOR_HASH,
      destinationBindingHash: tronDestinationBinding,
      routeRevision: ROUTE_REVISION,
      maxWrappedSupply: MAX_WRAPPED_SUPPLY,
    }),
  );
  const tronRecipient = Buffer.concat([
    Buffer.from([0x41]),
    Buffer.from((await signer.getAddress()).slice(2), "hex"),
  ]);
  const tronInboundPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_TRON,
    nonce: 23,
    amount: 3,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_TRON21,
    recipient: tronRecipient,
    route: "taira_tron_xor",
  });
  const tronPayloadHex = ethers.hexlify(tronInboundPayload);
  const tronMessageId =
    await tronBridge.sccpDestinationMessageId(tronPayloadHex);
  assert.equal(
    tronMessageId,
    messageId("sora-taira", "tron-nile", tronInboundPayload),
  );
  const tronPublicInputs = [
    tronMessageId,
    await tronBridge.sccpPayloadHash(tronPayloadHex),
    word(DOMAIN_TRON),
    ethers.keccak256(ethers.toUtf8Bytes("tron-commitment-root")),
    word(300),
    ethers.keccak256(ethers.toUtf8Bytes("tron-finality-block")),
  ];
  const tronStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("exact-taira-tron-statement"),
  );
  const tronProof = await acceptingProof(
    provider,
    abi,
    tronPublicInputs,
    tronStatementHash,
    tronDestinationBinding,
    await tronBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assertDynamicVerifierRoleCollisionsRejected({
    verifier: tronVerifier,
    provider,
    abi,
    publicInputs: tronPublicInputs,
    statement: tronStatementHash,
    destination: tronDestinationBinding,
    route: await tronBridge.routeConfigHash(),
    g1,
    g2,
  });
  const wrongTronRevisionPayload = Buffer.from(tronInboundPayload);
  wrongTronRevisionPayload.writeUInt32LE(ROUTE_REVISION + 1, 18);
  assert.notEqual(
    messageId("sora-taira", "tron-nile", wrongTronRevisionPayload),
    tronMessageId,
  );
  await assert.rejects(
    tronBridge.finalizeFromTaira(
      tronProof,
      tronPublicInputs,
      tronStatementHash,
      wrongTronRevisionPayload,
    ),
    rejectedWith("Wrong route revision"),
  );
  for (const invalidSender of invalidTairaSenders) {
    const invalidSenderPayload = transferPayload({
      sourceDomain: DOMAIN_SORA,
      destinationDomain: DOMAIN_TRON,
      nonce: 23,
      amount: 3,
      senderCodec: CODEC_TEXT,
      sender: invalidSender,
      recipientCodec: CODEC_TRON21,
      recipient: tronRecipient,
      route: "taira_tron_xor",
    });
    await assert.rejects(
      tronBridge.finalizeFromTaira(
        tronProof,
        tronPublicInputs,
        tronStatementHash,
        invalidSenderPayload,
      ),
      rejectedWith("Noncanonical sender"),
    );
  }
  await assert.rejects(
    secondTronBridge.finalizeFromTaira(
      tronProof,
      tronPublicInputs,
      tronStatementHash,
      tronPayloadHex,
    ),
    rejectedWith("Groth16 proof verification failed"),
  );
  await (
    await tronBridge.finalizeFromTaira(
      tronProof,
      tronPublicInputs,
      tronStatementHash,
      tronPayloadHex,
    )
  ).wait();
  assert.equal(
    await tronToken.balanceOf(await signer.getAddress()),
    3n * SCALE,
  );
  await assert.rejects(
    tronBridge.finalizeFromTaira(
      tronProof,
      tronPublicInputs,
      tronStatementHash,
      tronPayloadHex,
    ),
    rejectedWith("Destination message already used"),
  );
  const tronCapPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_TRON,
    nonce: 990,
    amount: MAX_OUTSTANDING_LIABILITY,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_TRON21,
    recipient: tronRecipient,
    route: "taira_tron_xor",
  });
  const tronCapPayloadHex = ethers.hexlify(tronCapPayload);
  const tronCapMessageId =
    await tronBridge.sccpDestinationMessageId(tronCapPayloadHex);
  const tronCapPublicInputs = [
    tronCapMessageId,
    await tronBridge.sccpPayloadHash(tronCapPayloadHex),
    word(DOMAIN_TRON),
    ethers.keccak256(ethers.toUtf8Bytes("tron-cap-commitment-root")),
    word(990),
    ethers.keccak256(ethers.toUtf8Bytes("tron-cap-finality-block")),
  ];
  const tronCapStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("tron-cap-statement"),
  );
  const tronCapProof = await acceptingProof(
    provider,
    abi,
    tronCapPublicInputs,
    tronCapStatementHash,
    tronDestinationBinding,
    await tronBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    tronBridge.finalizeFromTaira(
      tronCapProof,
      tronCapPublicInputs,
      tronCapStatementHash,
      tronCapPayloadHex,
    ),
    rejectedWith(),
  );
  assert.equal(await tronBridge.usedDestinationMessages(tronCapMessageId), false);
  assert.equal(await tronToken.totalSupply(), 3n * SCALE);
  for (let index = 0; index < validTairaSenders.length; index++) {
    const recipientAddress = ethers.getAddress(
      `0x${(0xb1 + index).toString(16).padStart(2, "0").repeat(20)}`,
    );
    const universalPayload = transferPayload({
      sourceDomain: DOMAIN_SORA,
      destinationDomain: DOMAIN_TRON,
      nonce: 200 + index,
      amount: 3,
      senderCodec: CODEC_TEXT,
      sender: validTairaSenders[index],
      recipientCodec: CODEC_TRON21,
      recipient: Buffer.concat([
        Buffer.from([0x41]),
        Buffer.from(recipientAddress.slice(2), "hex"),
      ]),
      route: "taira_tron_xor",
    });
    const universalPayloadHex = ethers.hexlify(universalPayload);
    const universalInputs = [
      await tronBridge.sccpDestinationMessageId(universalPayloadHex),
      await tronBridge.sccpPayloadHash(universalPayloadHex),
      word(DOMAIN_TRON),
      ethers.keccak256(ethers.toUtf8Bytes(`tron-universal-account-root-${index}`)),
      word(301 + index),
      ethers.keccak256(ethers.toUtf8Bytes(`tron-universal-account-finality-${index}`)),
    ];
    const universalStatement = ethers.keccak256(
      ethers.toUtf8Bytes(`tron-universal-account-statement-${index}`),
    );
    const universalProof = await acceptingProof(
      provider,
      abi,
      universalInputs,
      universalStatement,
      tronDestinationBinding,
      await tronBridge.routeConfigHash(),
      SORA_FINALITY_ANCHOR_HASH,
      g1,
      g2,
    );
    await (
      await tronBridge.finalizeFromTaira(
        universalProof,
        universalInputs,
        universalStatement,
        universalPayloadHex,
      )
    ).wait();
    assert.equal(await tronToken.balanceOf(recipientAddress), 3n * SCALE);
  }
  const tronOutsiderAddress = await tronOutsider.getAddress();
  await (await tronToken.approve(tronOutsiderAddress, SCALE)).wait();
  await (
    await tronToken
      .connect(tronOutsider)
      .transferFrom(await signer.getAddress(), tronOutsiderAddress, SCALE)
  ).wait();
  assert.equal(
    await tronToken.allowance(await signer.getAddress(), tronOutsiderAddress),
    0n,
  );
  await assert.rejects(
    tronToken.approve(tronOutsiderAddress, 2n * SCALE),
    rejectedWith("Clear allowance first"),
  );
  await (await tronToken.approve(tronOutsiderAddress, 0n)).wait();
  await (await tronToken.approve(tronOutsiderAddress, 2n * SCALE)).wait();
  assert.equal(
    await tronToken.allowance(await signer.getAddress(), tronOutsiderAddress),
    2n * SCALE,
  );
  await (await tronToken.approve(tronOutsiderAddress, 0n)).wait();

  const tronBurnAccount = await signer.getAddress();
  assert.equal(await tronBridge.transferNonces(tronOutsiderAddress), 0n);
  assert.equal(await tronBridge.transferNonces(tronBurnAccount), 0n);
  const outsiderFirstMessageId = await tronBridge
    .connect(tronOutsider)
    .transferToTaira.staticCall(CANONICAL_I105_BYTES, SCALE, 0n);
  const signerFirstMessageId = await tronBridge.transferToTaira.staticCall(
    CANONICAL_I105_BYTES,
    SCALE,
    0n,
  );
  assert.notEqual(outsiderFirstMessageId, signerFirstMessageId);
  await (
    await tronBridge
      .connect(tronOutsider)
      .transferToTaira(CANONICAL_I105_BYTES, SCALE, 0n)
  ).wait();
  assert.equal(await tronBridge.transferNonces(tronOutsiderAddress), 1n);
  assert.equal(await tronBridge.transferNonces(tronBurnAccount), 0n);
  await (
    await tronBridge.transferToTaira(CANONICAL_I105_BYTES, SCALE, 0n)
  ).wait();
  assert.equal(await tronBridge.transferNonces(tronBurnAccount), 1n);

  const tronNonceBeforeInvalidBurns = await tronBridge.transferNonces(tronBurnAccount);
  await assert.rejects(
    tronBridge.transferToTaira(CANONICAL_I105_BYTES, 1n, tronNonceBeforeInvalidBurns),
    rejectedWith("Amount is not aligned to Taira scale"),
  );
  const tronBalanceBeforeInvalidBurns = await tronToken.balanceOf(tronBurnAccount);
  await assert.rejects(
    tronBridge.transferToTaira(
      CANONICAL_I105_BYTES,
      SCALE,
      tronNonceBeforeInvalidBurns + 1n,
    ),
    rejectedWith("Transfer nonce mismatch"),
  );
  for (const invalidRecipient of invalidTairaRecipients) {
    await assert.rejects(
      tronBridge.transferToTaira(invalidRecipient, SCALE, tronNonceBeforeInvalidBurns),
      rejectedWith("Noncanonical Taira recipient"),
    );
  }
  assert.equal(await tronToken.balanceOf(tronBurnAccount), tronBalanceBeforeInvalidBurns);
  assert.equal(
    await tronBridge.transferNonces(tronBurnAccount),
    tronNonceBeforeInvalidBurns,
  );
  const tronSourceReceipt = await (
    await tronBridge.transferToTaira(
      CANONICAL_I105_BYTES,
      SCALE,
      tronNonceBeforeInvalidBurns,
    )
  ).wait();
  const tronSourceEvents = tronSourceReceipt.logs
    .filter(
      (log) => log.address.toLowerCase() === tronBridgeAddress.toLowerCase(),
    )
    .map((log) => {
      try {
        return tronBridge.interface.parseLog(log);
      } catch (_) {
        return null;
      }
    })
    .filter((log) => log && log.name === "SccpTransfer");
  assert.equal(tronSourceEvents.length, 1);
  const tronSourceEvent = tronSourceEvents[0].args;
  const tronSourcePayload = ethers.getBytes(tronSourceEvent.canonicalPayload);
  assert.equal(Buffer.from(tronSourcePayload).readUInt32LE(18), ROUTE_REVISION);
  assert.equal(
    tronSourceEvent.messageId,
    messageId("tron-nile", "sora-taira", tronSourcePayload),
  );
  assert.equal(tronSourceEvent.laneHash, await tronBridge.sourceLaneHash());
  assert.equal(
    tronSourceEvent.routeConfigHash,
    await tronBridge.routeConfigHash(),
  );
  assert.equal(
    tronSourceEvent.sourceEventDigest,
    await tronBridge.sourceEventDigest(
      tronSourceEvent.messageId,
      tronSourceEvent.payloadHash,
    ),
  );
  assert.equal(
    await tronToken.balanceOf(await signer.getAddress()),
    0n,
  );
  assert.equal(await tronBridge.transferNonces(tronBurnAccount), 2n);
  assert.equal(await tronBridge.transferNonces(tronOutsiderAddress), 1n);

  const predictedTronNoopBridgeAddress = await nextCreateAddress(signer, 1);
  const tronNoopToken = await deploy(signer, trueNoopTokenArtifact, [
    predictedTronNoopBridgeAddress,
  ]);
  const tronNoopBridge = await deploy(signer, tronBridgeArtifact, [
    await tronNoopToken.getAddress(),
    verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
    TRON_NILE_PROFILE,
    ROUTE_REVISION,
    MAX_WRAPPED_SUPPLY,
  ]);
  assert.equal(
    await tronNoopBridge.getAddress(),
    predictedTronNoopBridgeAddress,
  );
  await (await tronNoopToken.seed(tronBurnAccount, 2n * SCALE)).wait();
  await assert.rejects(
    tronNoopBridge.transferToTaira(CANONICAL_I105_BYTES, SCALE, 0n),
    rejectedWith(),
  );
  assert.equal(await tronNoopBridge.transferNonces(tronBurnAccount), 0n);
  assert.equal(await tronNoopToken.totalSupply(), 2n * SCALE);
  assert.equal(await tronNoopToken.balanceOf(tronBurnAccount), 2n * SCALE);

  const tronNoopMintPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_TRON,
    nonce: 400,
    amount: 1,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_TRON21,
    recipient: tronRecipient,
    route: "taira_tron_xor",
  });
  const tronNoopMintPayloadHex = ethers.hexlify(tronNoopMintPayload);
  const tronNoopMintMessageId =
    await tronNoopBridge.sccpDestinationMessageId(tronNoopMintPayloadHex);
  const tronNoopMintPublicInputs = [
    tronNoopMintMessageId,
    await tronNoopBridge.sccpPayloadHash(tronNoopMintPayloadHex),
    word(DOMAIN_TRON),
    ethers.keccak256(ethers.toUtf8Bytes("tron-noop-mint-commitment-root")),
    word(400),
    ethers.keccak256(ethers.toUtf8Bytes("tron-noop-mint-finality-block")),
  ];
  const tronNoopMintStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("tron-noop-mint-statement"),
  );
  const tronNoopMintProof = await acceptingProof(
    provider,
    abi,
    tronNoopMintPublicInputs,
    tronNoopMintStatementHash,
    await tronNoopBridge.destinationBindingHash(),
    await tronNoopBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    tronNoopBridge.finalizeFromTaira(
      tronNoopMintProof,
      tronNoopMintPublicInputs,
      tronNoopMintStatementHash,
      tronNoopMintPayloadHex,
    ),
    rejectedWith(),
  );
  assert.equal(
    await tronNoopBridge.usedDestinationMessages(tronNoopMintMessageId),
    false,
  );
  assert.equal(await tronNoopToken.totalSupply(), 2n * SCALE);
  assert.equal(await tronNoopToken.balanceOf(tronBurnAccount), 2n * SCALE);

  const predictedTronWrongDeltaBridgeAddress = await nextCreateAddress(signer, 1);
  const tronWrongDeltaToken = await deploy(signer, wrongDeltaTokenArtifact, [
    predictedTronWrongDeltaBridgeAddress,
  ]);
  const tronWrongDeltaBridge = await deploy(signer, tronBridgeArtifact, [
    await tronWrongDeltaToken.getAddress(),
    verifierPolicy(tronVerifierAddress, tronVerifierCodeHash, verifierKeyHash),
    TRON_NILE_PROFILE,
    ROUTE_REVISION,
    MAX_WRAPPED_SUPPLY,
  ]);
  assert.equal(
    await tronWrongDeltaBridge.getAddress(),
    predictedTronWrongDeltaBridgeAddress,
  );
  await (await tronWrongDeltaToken.seed(tronBurnAccount, 2n * SCALE)).wait();
  await assert.rejects(
    tronWrongDeltaBridge.transferToTaira(CANONICAL_I105_BYTES, SCALE, 0n),
    rejectedWith(),
  );
  assert.equal(await tronWrongDeltaBridge.transferNonces(tronBurnAccount), 0n);
  assert.equal(await tronWrongDeltaToken.totalSupply(), 2n * SCALE);
  assert.equal(await tronWrongDeltaToken.balanceOf(tronBurnAccount), 2n * SCALE);

  const tronWrongDeltaMintPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_TRON,
    nonce: 401,
    amount: 1,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_TRON21,
    recipient: tronRecipient,
    route: "taira_tron_xor",
  });
  const tronWrongDeltaMintPayloadHex = ethers.hexlify(
    tronWrongDeltaMintPayload,
  );
  const tronWrongDeltaMintMessageId =
    await tronWrongDeltaBridge.sccpDestinationMessageId(
      tronWrongDeltaMintPayloadHex,
    );
  const tronWrongDeltaMintPublicInputs = [
    tronWrongDeltaMintMessageId,
    await tronWrongDeltaBridge.sccpPayloadHash(tronWrongDeltaMintPayloadHex),
    word(DOMAIN_TRON),
    ethers.keccak256(
      ethers.toUtf8Bytes("tron-wrong-delta-mint-commitment-root"),
    ),
    word(401),
    ethers.keccak256(
      ethers.toUtf8Bytes("tron-wrong-delta-mint-finality-block"),
    ),
  ];
  const tronWrongDeltaMintStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("tron-wrong-delta-mint-statement"),
  );
  const tronWrongDeltaMintProof = await acceptingProof(
    provider,
    abi,
    tronWrongDeltaMintPublicInputs,
    tronWrongDeltaMintStatementHash,
    await tronWrongDeltaBridge.destinationBindingHash(),
    await tronWrongDeltaBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    tronWrongDeltaBridge.finalizeFromTaira(
      tronWrongDeltaMintProof,
      tronWrongDeltaMintPublicInputs,
      tronWrongDeltaMintStatementHash,
      tronWrongDeltaMintPayloadHex,
    ),
    rejectedWith(),
  );
  assert.equal(
    await tronWrongDeltaBridge.usedDestinationMessages(
      tronWrongDeltaMintMessageId,
    ),
    false,
  );
  assert.equal(await tronWrongDeltaToken.totalSupply(), 2n * SCALE);
  assert.equal(await tronWrongDeltaToken.balanceOf(tronBurnAccount), 2n * SCALE);
  await tronEip1193Provider.disconnect();
  }

  const bscRoute = await deployPreboundRoute(
    signer,
    tokenArtifact,
    bridgeArtifact,
    [
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ],
  );
  const bridge = bscRoute.route;
  const bridgeAddress = await bridge.getAddress();
  const token = bscRoute.token;
  const tokenAddress = await token.getAddress();
  assert.equal(await token.bridge(), bridgeAddress);
  assert.equal(await bridge.routeRevision(), BigInt(ROUTE_REVISION));
  assert.equal(await bridge.maxWrappedSupply(), MAX_WRAPPED_SUPPLY);
  assert.equal(
    await bridge.semanticProofProfileHash(),
    SEMANTIC_PROOF_PROFILE_HASH,
  );
  assert.equal(
    await bridge.soraFinalityAnchorHash(),
    SORA_FINALITY_ANCHOR_HASH,
  );
  const tokenCodeHash = ethers.keccak256(await provider.getCode(tokenAddress));
  assert.equal(await bridge.tokenCodeHash(), tokenCodeHash);
  assert.equal(
    await bridge.routeConfigHash(),
    exactEvmRouteConfigHash({
      abi,
      domain: DOMAIN_BSC,
      profile: BSC_TESTNET_PROFILE,
      chainId: 97,
      sourceLaneHash: await bridge.sourceLaneHash(),
      destinationLaneHash: await bridge.destinationLaneHash(),
      tokenAddress,
      tokenCodeHash,
      verifierAddress,
      verifierCodeHash,
      verifierKeyHash,
      semanticProofProfileHash: SEMANTIC_PROOF_PROFILE_HASH,
      soraFinalityAnchorHash: SORA_FINALITY_ANCHOR_HASH,
      route: "taira_bsc_xor",
      routeRevision: ROUTE_REVISION,
      maxWrappedSupply: MAX_WRAPPED_SUPPLY,
    }),
  );
  const wrongBoundToken = await deploy(signer, falseTokenArtifact, [
    await outsider.getAddress(),
  ]);
  await assert.rejects(
    deploy(signer, injectedBscRouteArtifact, [
      await wrongBoundToken.getAddress(),
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  const wrongDecimalsRouteAddress = await nextCreateAddress(signer, 1);
  const wrongDecimalsToken = await deploy(signer, wrongDecimalsTokenArtifact, [
    wrongDecimalsRouteAddress,
  ]);
  await assert.rejects(
    deploy(signer, injectedBscRouteArtifact, [
      await wrongDecimalsToken.getAddress(),
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith("Unexpected token decimals"),
  );
  const nonzeroSupplyRouteAddress = await nextCreateAddress(signer, 1);
  const nonzeroSupplyToken = await deploy(signer, nonzeroSupplyTokenArtifact, [
    nonzeroSupplyRouteAddress,
  ]);
  await assert.rejects(
    deploy(signer, injectedBscRouteArtifact, [
      await nonzeroSupplyToken.getAddress(),
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith("Token supply must start at zero"),
  );
  const rejectedBscRouteAddress = await nextCreateAddress(signer, 1);
  const rejectedBscToken = await deploy(signer, tokenArtifact, [
    rejectedBscRouteAddress,
  ]);
  const rejectedBscTokenAddress = await rejectedBscToken.getAddress();
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      BSC_TESTNET_PROFILE,
      0,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      0,
    ]),
    rejectedWith("Invalid wrapped supply cap"),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_U128 + 1n,
    ]),
    rejectedWith("Invalid wrapped supply cap"),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(verifierAddress, ethers.ZeroHash, verifierKeyHash),
      5,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(verifierAddress, verifierCodeHash, ethers.ZeroHash),
      5,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(
        verifierAddress,
        verifierCodeHash,
        verifierKeyHash,
        ALTERNATE_SEMANTIC_PROOF_PROFILE_HASH,
      ),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      rejectedBscTokenAddress,
      verifierPolicy(
        verifierAddress,
        verifierCodeHash,
        verifierKeyHash,
        SEMANTIC_PROOF_PROFILE_HASH,
        ALTERNATE_SORA_FINALITY_ANCHOR_HASH,
      ),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  const evmCodeAliasedVerifier = await deploy(
    signer,
    codeAliasedVerifierArtifact,
    [ethers.ZeroHash, false],
  );
  const evmCodeAliasedVerifierAddress =
    await evmCodeAliasedVerifier.getAddress();
  const evmCodeAliasedVerifierCodeHash = ethers.keccak256(
    await provider.getCode(evmCodeAliasedVerifierAddress),
  );
  assert.equal(
    await evmCodeAliasedVerifier.semanticProofProfileHash(),
    evmCodeAliasedVerifierCodeHash,
  );
  const aliasedBscRouteAddress = await nextCreateAddress(signer, 1);
  const aliasedBscToken = await deploy(signer, tokenArtifact, [
    aliasedBscRouteAddress,
  ]);
  const aliasedBscTokenAddress = await aliasedBscToken.getAddress();
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      aliasedBscTokenAddress,
      verifierPolicy(
        evmCodeAliasedVerifierAddress,
        evmCodeAliasedVerifierCodeHash,
        await evmCodeAliasedVerifier.verifyingKeyHash(),
        await evmCodeAliasedVerifier.semanticProofProfileHash(),
        await evmCodeAliasedVerifier.soraFinalityAnchorHash(),
      ),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      aliasedBscTokenAddress,
      verifierPolicy(
        verifierAddress,
        verifierCodeHash,
        verifierKeyHash,
        ethers.ZeroHash,
      ),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      aliasedBscTokenAddress,
      verifierPolicy(
        verifierAddress,
        verifierCodeHash,
        verifierKeyHash,
        SORA_FINALITY_ANCHOR_HASH,
        SORA_FINALITY_ANCHOR_HASH,
      ),
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      aliasedBscTokenAddress,
      verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
      4,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );

  const signerAddress = await signer.getAddress();
  const recipient20 = Buffer.from(signerAddress.slice(2), "hex");
  const inboundPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_BSC,
    nonce: 9,
    amount: 5,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: recipient20,
  });
  const payloadHex = ethers.hexlify(inboundPayload);
  const payloadHash = await bridge.sccpPayloadHash(payloadHex);
  const exactMessageId = await bridge.sccpDestinationMessageId(payloadHex);
  assert.equal(
    exactMessageId,
    messageId("sora-taira", "bsc-testnet", inboundPayload),
  );
  const destinationBinding = await bridge.destinationBindingHash();
  assert.equal(
    destinationBinding,
    exactEvmDestinationBindingHash({
      abi,
      chainId: 97,
      targetDomain: DOMAIN_BSC,
      verifierAddress,
      bridgeAddress,
      verifierCodeHash,
      verifierKeyHash,
      semanticProofProfileHash: SEMANTIC_PROOF_PROFILE_HASH,
      soraFinalityAnchorHash: SORA_FINALITY_ANCHOR_HASH,
    }),
  );
  const publicInputs = [
    exactMessageId,
    payloadHash,
    word(DOMAIN_BSC),
    ethers.keccak256(ethers.toUtf8Bytes("commitment-root")),
    word(100),
    ethers.keccak256(ethers.toUtf8Bytes("finality-block")),
  ];
  const statementHash = ethers.keccak256(
    ethers.toUtf8Bytes("exact-taira-bsc-statement"),
  );
  const proof = await acceptingProof(
    provider,
    abi,
    publicInputs,
    statementHash,
    destinationBinding,
    await bridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  const routeConfigurationHash = await bridge.routeConfigHash();
  const changedWord = (label) => ethers.keccak256(ethers.toUtf8Bytes(label));
  const withInput = (index, value) => {
    const changed = [...publicInputs];
    changed[index] = value;
    return changed;
  };
  const verifierTupleAdversaries = [
    {
      label: "unsupported embedded proof version",
      proof: mutateGroth16Proof(abi, proof, { version: 2n }),
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Unsupported Groth16 proof version",
    },
    {
      label: "embedded message id mismatch",
      proof: mutateGroth16Proof(abi, proof, { messageId: changedWord("wrong-embedded-message") }),
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Public input message id mismatch",
    },
    {
      label: "embedded commitment root mismatch",
      proof: mutateGroth16Proof(abi, proof, {
        commitmentRoot: changedWord("wrong-embedded-root"),
      }),
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Public input commitment root mismatch",
    },
    {
      label: "embedded source domain substitution",
      proof: mutateGroth16Proof(abi, proof, { sourceDomain: DOMAIN_ETHEREUM }),
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "embedded source domain overflow",
      proof: mutateGroth16Proof(abi, proof, { sourceDomain: 1n << 32n }),
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Source domain overflow",
    },
    {
      label: "embedded source equals target",
      proof: mutateGroth16Proof(abi, proof, { sourceDomain: DOMAIN_BSC }),
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Source and target domains must differ",
    },
    {
      label: "public message id mismatch",
      proof,
      inputs: withInput(0, changedWord("wrong-public-message")),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Public input message id mismatch",
    },
    {
      label: "public payload signal mismatch",
      proof,
      inputs: withInput(1, changedWord("wrong-public-payload")),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "public target signal mismatch",
      proof,
      inputs: withInput(2, word(DOMAIN_ETHEREUM)),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "public target signal overflow",
      proof,
      inputs: withInput(2, word(1n << 32n)),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Target domain overflow",
    },
    {
      label: "public commitment root mismatch",
      proof,
      inputs: withInput(3, changedWord("wrong-public-root")),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Public input commitment root mismatch",
    },
    {
      label: "public finality height mismatch",
      proof,
      inputs: withInput(4, word(101)),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "public finality block mismatch",
      proof,
      inputs: withInput(5, changedWord("wrong-finality-block")),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "statement signal mismatch",
      proof,
      inputs: publicInputs,
      statement: changedWord("wrong-statement"),
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "destination signal mismatch",
      proof,
      inputs: publicInputs,
      statement: statementHash,
      destination: changedWord("wrong-destination"),
      route: routeConfigurationHash,
      reason: "Groth16 proof verification failed",
    },
    {
      label: "route signal mismatch",
      proof,
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: changedWord("wrong-route"),
      reason: "Groth16 proof verification failed",
    },
    {
      label: "zero embedded and public message id",
      proof: mutateGroth16Proof(abi, proof, { messageId: ethers.ZeroHash }),
      inputs: withInput(0, ethers.ZeroHash),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Message id is required",
    },
    {
      label: "zero payload hash",
      proof,
      inputs: withInput(1, ethers.ZeroHash),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Payload hash is required",
    },
    {
      label: "zero target domain",
      proof,
      inputs: withInput(2, ethers.ZeroHash),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Target domain is required",
    },
    {
      label: "zero embedded and public commitment root",
      proof: mutateGroth16Proof(abi, proof, { commitmentRoot: ethers.ZeroHash }),
      inputs: withInput(3, ethers.ZeroHash),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Commitment root is required",
    },
    {
      label: "zero finality height",
      proof,
      inputs: withInput(4, ethers.ZeroHash),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Finality height is required",
    },
    {
      label: "zero finality block hash",
      proof,
      inputs: withInput(5, ethers.ZeroHash),
      statement: statementHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Finality block hash is required",
    },
    {
      label: "zero statement hash",
      proof,
      inputs: publicInputs,
      statement: ethers.ZeroHash,
      destination: destinationBinding,
      route: routeConfigurationHash,
      reason: "Statement hash is required",
    },
    {
      label: "zero destination binding",
      proof,
      inputs: publicInputs,
      statement: statementHash,
      destination: ethers.ZeroHash,
      route: routeConfigurationHash,
      reason: "Destination binding hash is required",
    },
    {
      label: "zero route configuration",
      proof,
      inputs: publicInputs,
      statement: statementHash,
      destination: destinationBinding,
      route: ethers.ZeroHash,
      reason: "Route configuration hash is required",
    },
  ];
  for (const adversary of verifierTupleAdversaries) {
    await assert.rejects(
      verifier.verifySccpMessageProof(
        adversary.proof,
        adversary.inputs,
        adversary.statement,
        adversary.destination,
        adversary.route,
      ),
      rejectedWith(adversary.reason),
      adversary.label,
    );
  }

  const proofPointAdversaries = [
    ["zero A", { a: [0n, 0n] }, "G1 point is zero"],
    ["out-of-field A", { a: [BASE_FIELD, 2n] }, "G1 point out of range"],
    ["off-curve A", { a: [1n, 3n] }, "G1 scalar multiplication failed"],
    ["zero B", { b: [0n, 0n, 0n, 0n] }, "G2 point is zero"],
    [
      "out-of-field B",
      { b: [BASE_FIELD, g2[1], g2[2], g2[3]] },
      "G2 point out of range",
    ],
    ["off-curve B", { b: [1n, 2n, 3n, 4n] }, "Pairing precompile failed"],
    ["non-subgroup B", { b: NON_SUBGROUP_G2 }, "Pairing precompile failed"],
    ["zero C", { c: [0n, 0n] }, "G1 point is zero"],
    ["out-of-field C", { c: [BASE_FIELD, 2n] }, "G1 point out of range"],
    ["off-curve C", { c: [1n, 3n] }, "G1 scalar multiplication failed"],
  ];
  for (const [label, overrides, reason] of proofPointAdversaries) {
    await assert.rejects(
      verifier.verifySccpMessageProof(
        mutateGroth16Proof(abi, proof, overrides),
        publicInputs,
        statementHash,
        destinationBinding,
        routeConfigurationHash,
      ),
      rejectedWith(reason),
      label,
    );
  }

  const proofBytes = ethers.getBytes(proof);
  assert.equal(proofBytes.length, 12 * 32, "canonical Groth16 proof tuple length drift");
  const proofFramingAdversaries = [
    ...[0, 1, 32, proofBytes.length - 1].map((length) => [
      `truncated proof length ${length}`,
      ethers.hexlify(proofBytes.slice(0, length)),
    ]),
    ["one trailing proof byte", ethers.hexlify(Buffer.concat([proofBytes, Buffer.from([0])]))],
    ["one trailing proof word", ethers.hexlify(Buffer.concat([proofBytes, Buffer.alloc(32)]))],
  ];
  for (const [label, malformedProof] of proofFramingAdversaries) {
    await assert.rejects(
      verifier.verifySccpMessageProof(
        malformedProof,
        publicInputs,
        statementHash,
        destinationBinding,
        routeConfigurationHash,
      ),
      rejectedWith("Unexpected Groth16 proof length"),
      label,
    );
  }
  await assertDynamicVerifierRoleCollisionsRejected({
    verifier,
    provider,
    abi,
    publicInputs,
    statement: statementHash,
    destination: destinationBinding,
    route: routeConfigurationHash,
    g1,
    g2,
  });
  const proofForSubstitutedFinalityAnchor = await acceptingProof(
    provider,
    abi,
    publicInputs,
    statementHash,
    destinationBinding,
    await bridge.routeConfigHash(),
    ALTERNATE_SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    verifier.verifySccpMessageProof(
      proofForSubstitutedFinalityAnchor,
      publicInputs,
      statementHash,
      destinationBinding,
      await bridge.routeConfigHash(),
    ),
    rejectedWith("Groth16 proof verification failed"),
  );
  const wrongRouteConfigurationHash = ethers.keccak256(
    ethers.toUtf8Bytes("adversarial-wrong-route-configuration"),
  );
  await assert.rejects(
    verifier.verifySccpMessageProof(
      proof,
      publicInputs,
      statementHash,
      destinationBinding,
      wrongRouteConfigurationHash,
    ),
    rejectedWith("Groth16 proof verification failed"),
  );
  await assert.rejects(
    verifier.verifySccpMessageProof(
      proof,
      publicInputs,
      statementHash,
      destinationBinding,
      ethers.ZeroHash,
    ),
    rejectedWith("Route configuration hash is required"),
  );
  const wrongRevisionPayload = Buffer.from(inboundPayload);
  wrongRevisionPayload.writeUInt32LE(ROUTE_REVISION + 1, 18);
  assert.notEqual(
    messageId("sora-taira", "bsc-testnet", wrongRevisionPayload),
    exactMessageId,
  );
  await assert.rejects(
    bridge.finalizeFromTaira(
      proof,
      publicInputs,
      statementHash,
      wrongRevisionPayload,
    ),
    rejectedWith("Wrong route revision"),
  );

  await assert.rejects(
    bridge.finalizeFromTaira(
      proof,
      [
        messageId("sora-nexus", "bsc-testnet", inboundPayload),
        ...publicInputs.slice(1),
      ],
      statementHash,
      payloadHex,
    ),
    rejectedWith("Message id mismatch"),
  );
  await assert.rejects(
    bridge.finalizeFromTaira(
      proof,
      [
        messageId("sora-taira", "bsc-mainnet", inboundPayload),
        ...publicInputs.slice(1),
      ],
      statementHash,
      payloadHex,
    ),
    rejectedWith("Message id mismatch"),
  );
  const oldPayloadOnlyId = ethers.keccak256(
    ethers.concat([ethers.toUtf8Bytes("sccp:transfer:v1"), payloadHex]),
  );
  await assert.rejects(
    bridge.finalizeFromTaira(
      proof,
      [oldPayloadOnlyId, ...publicInputs.slice(1)],
      statementHash,
      payloadHex,
    ),
    rejectedWith("Message id mismatch"),
  );
  await assert.rejects(
    bridge.finalizeFromTaira(
      proof,
      publicInputs,
      statementHash,
      ethers.concat([payloadHex, "0x00"]),
    ),
    rejectedWith("Trailing payload bytes"),
  );
  const wrongCodecPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_BSC,
    nonce: 9,
    amount: 5,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_TEXT,
    recipient: recipient20,
  });
  await assert.rejects(
    bridge.finalizeFromTaira(
      proof,
      publicInputs,
      statementHash,
      wrongCodecPayload,
    ),
    rejectedWith("Wrong recipient codec"),
  );
  await assert.rejects(
    bridge.finalizeFromTaira("0x00", publicInputs, statementHash, payloadHex),
    rejectedWith("Unexpected Groth16 proof length"),
  );

  for (const invalidSender of invalidTairaSenders) {
    const invalidSenderPayload = transferPayload({
      sourceDomain: DOMAIN_SORA,
      destinationDomain: DOMAIN_BSC,
      nonce: 9,
      amount: 5,
      senderCodec: CODEC_TEXT,
      sender: invalidSender,
      recipientCodec: CODEC_EVM20,
      recipient: recipient20,
    });
    await assert.rejects(
      bridge.finalizeFromTaira(
        proof,
        publicInputs,
        statementHash,
        invalidSenderPayload,
      ),
      rejectedWith("Noncanonical sender"),
    );
  }

  await (
    await bridge.finalizeFromTaira(
      proof,
      publicInputs,
      statementHash,
      payloadHex,
    )
  ).wait();
  assert.equal(await token.balanceOf(signerAddress), 5n * SCALE);
  assert.equal(await bridge.usedDestinationMessages(exactMessageId), true);
  await assert.rejects(
    bridge.finalizeFromTaira(proof, publicInputs, statementHash, payloadHex),
    rejectedWith("Destination message already used"),
  );
  const bscCapPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_BSC,
    nonce: 991,
    amount: MAX_OUTSTANDING_LIABILITY,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: recipient20,
  });
  const bscCapPayloadHex = ethers.hexlify(bscCapPayload);
  const bscCapMessageId = await bridge.sccpDestinationMessageId(bscCapPayloadHex);
  const bscCapPublicInputs = [
    bscCapMessageId,
    await bridge.sccpPayloadHash(bscCapPayloadHex),
    word(DOMAIN_BSC),
    ethers.keccak256(ethers.toUtf8Bytes("bsc-cap-commitment-root")),
    word(991),
    ethers.keccak256(ethers.toUtf8Bytes("bsc-cap-finality-block")),
  ];
  const bscCapStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("bsc-cap-statement"),
  );
  const bscCapProof = await acceptingProof(
    provider,
    abi,
    bscCapPublicInputs,
    bscCapStatementHash,
    destinationBinding,
    await bridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    bridge.finalizeFromTaira(
      bscCapProof,
      bscCapPublicInputs,
      bscCapStatementHash,
      bscCapPayloadHex,
    ),
    rejectedWith("Wrapped supply cap exceeded"),
  );
  assert.equal(await bridge.usedDestinationMessages(bscCapMessageId), false);
  assert.equal(await token.totalSupply(), 5n * SCALE);
  for (let index = 0; index < validTairaSenders.length; index++) {
    const recipientAddress = ethers.getAddress(
      `0x${(0xa1 + index).toString(16).padStart(2, "0").repeat(20)}`,
    );
    const universalPayload = transferPayload({
      sourceDomain: DOMAIN_SORA,
      destinationDomain: DOMAIN_BSC,
      nonce: 100 + index,
      amount: 5,
      senderCodec: CODEC_TEXT,
      sender: validTairaSenders[index],
      recipientCodec: CODEC_EVM20,
      recipient: Buffer.from(recipientAddress.slice(2), "hex"),
    });
    const universalPayloadHex = ethers.hexlify(universalPayload);
    const universalInputs = [
      await bridge.sccpDestinationMessageId(universalPayloadHex),
      await bridge.sccpPayloadHash(universalPayloadHex),
      word(DOMAIN_BSC),
      ethers.keccak256(ethers.toUtf8Bytes(`universal-account-root-${index}`)),
      word(101 + index),
      ethers.keccak256(ethers.toUtf8Bytes(`universal-account-finality-${index}`)),
    ];
    const universalStatement = ethers.keccak256(
      ethers.toUtf8Bytes(`universal-account-statement-${index}`),
    );
    const universalProof = await acceptingProof(
      provider,
      abi,
      universalInputs,
      universalStatement,
      destinationBinding,
      await bridge.routeConfigHash(),
      SORA_FINALITY_ANCHOR_HASH,
      g1,
      g2,
    );
    await (
      await bridge.finalizeFromTaira(
        universalProof,
        universalInputs,
        universalStatement,
        universalPayloadHex,
      )
    ).wait();
    assert.equal(await token.balanceOf(recipientAddress), 5n * SCALE);
  }

  const outsiderAddress = await outsider.getAddress();
  await assert.rejects(
    token.mint(signerAddress, SCALE),
    rejectedWith("Caller is not the bridge"),
  );
  await assert.rejects(
    token.burnFrom(signerAddress, SCALE),
    rejectedWith("Caller is not the bridge"),
  );
  await assert.rejects(
    token.approve(ethers.ZeroAddress, SCALE),
    rejectedWith("Spender address is required"),
  );
  const signerBalanceBeforeAllowanceAttacks = await token.balanceOf(signerAddress);
  const outsiderBalanceBeforeAllowanceAttacks = await token.balanceOf(outsiderAddress);
  await (await token.approve(outsiderAddress, SCALE)).wait();
  await (
    await token
      .connect(outsider)
      .transferFrom(signerAddress, outsiderAddress, SCALE)
  ).wait();
  assert.equal(await token.allowance(signerAddress, outsiderAddress), 0n);
  await assert.rejects(
    token.approve(outsiderAddress, 2n * SCALE),
    rejectedWith("Clear allowance first"),
  );
  await (await token.approve(outsiderAddress, 0n)).wait();
  await (await token.connect(outsider).transfer(signerAddress, SCALE)).wait();
  await (await token.approve(outsiderAddress, 2n * SCALE)).wait();
  await assert.rejects(
    token.connect(outsider).transferFrom(signerAddress, outsiderAddress, 3n * SCALE),
    rejectedWith("Allowance exceeded"),
  );
  assert.equal(await token.allowance(signerAddress, outsiderAddress), 2n * SCALE);
  await (await token.approve(outsiderAddress, 0n)).wait();
  await (await token.approve(outsiderAddress, ethers.MaxUint256)).wait();
  await assert.rejects(
    token.connect(outsider).transferFrom(signerAddress, outsiderAddress, 6n * SCALE),
    rejectedWith("Uint256 underflow"),
  );
  assert.equal(
    await token.allowance(signerAddress, outsiderAddress),
    ethers.MaxUint256,
    "allowance decrement must roll back when the balance transfer fails",
  );
  assert.equal(
    await token.balanceOf(signerAddress),
    signerBalanceBeforeAllowanceAttacks,
  );
  assert.equal(
    await token.balanceOf(outsiderAddress),
    outsiderBalanceBeforeAllowanceAttacks,
  );
  await (await token.approve(outsiderAddress, 0n)).wait();

  await (await token.transfer(outsiderAddress, SCALE)).wait();
  assert.equal(await bridge.transferNonces(signerAddress), 0n);
  assert.equal(await bridge.transferNonces(outsiderAddress), 0n);
  const outsiderFirstMessageId = await bridge
    .connect(outsider)
    .transferToTaira.staticCall(CANONICAL_I105_BYTES, SCALE);
  const signerFirstMessageId = await bridge.transferToTaira.staticCall(
    CANONICAL_I105_BYTES,
    SCALE,
  );
  assert.notEqual(outsiderFirstMessageId, signerFirstMessageId);
  await (
    await bridge
      .connect(outsider)
      .transferToTaira(CANONICAL_I105_BYTES, SCALE)
  ).wait();
  assert.equal(await bridge.transferNonces(outsiderAddress), 1n);
  assert.equal(await bridge.transferNonces(signerAddress), 0n);
  assert.equal(
    await token.balanceOf(outsiderAddress),
    outsiderBalanceBeforeAllowanceAttacks,
  );
  await assert.rejects(
    token.transfer(ethers.ZeroAddress, SCALE),
    rejectedWith("Recipient address is required"),
  );
  assert.equal(
    await token.balanceOf(signerAddress),
    signerBalanceBeforeAllowanceAttacks - SCALE,
  );

  const invalidRecipients = [
    "0x",
    ethers.hexlify(Buffer.from(" bad")),
    ...invalidTairaRecipients,
  ];
  const balanceBeforeInvalidBurns = await token.balanceOf(signerAddress);
  const nonceBeforeInvalidBurns = await bridge.transferNonces(signerAddress);
  for (const recipient of invalidRecipients) {
    await assert.rejects(
      bridge.transferToTaira(recipient, SCALE),
      rejectedWith(),
    );
  }
  for (const amount of [0n, SCALE + 1n, ((1n << 128n) + 1n) * SCALE]) {
    await assert.rejects(
      bridge.transferToTaira(CANONICAL_I105_BYTES, amount),
      rejectedWith(),
    );
  }
  assert.equal(await token.balanceOf(signerAddress), balanceBeforeInvalidBurns);
  assert.equal(await bridge.transferNonces(signerAddress), nonceBeforeInvalidBurns);
  const sourceTx = await bridge.transferToTaira(
    CANONICAL_I105_BYTES,
    SCALE,
  );
  const sourceReceipt = await sourceTx.wait();
  const parsedSource = sourceReceipt.logs
    .filter((log) => log.address.toLowerCase() === bridgeAddress.toLowerCase())
    .map((log) => {
      try {
        return bridge.interface.parseLog(log);
      } catch (_) {
        return null;
      }
    })
    .filter((log) => log && log.name === "SccpTransfer");
  assert.equal(parsedSource.length, 1);
  assert.equal(parsedSource[0].args.laneHash, await bridge.sourceLaneHash());
  assert.equal(
    parsedSource[0].args.routeConfigHash,
    await bridge.routeConfigHash(),
  );
  assert.equal(await bridge.transferNonces(signerAddress), 1n);
  assert.equal(
    await token.balanceOf(signerAddress),
    balanceBeforeInvalidBurns - SCALE,
  );
  assert.equal(await bridge.transferNonces(outsiderAddress), 1n);
  const sourcePayload = ethers.getBytes(parsedSource[0].args.canonicalPayload);
  assert.equal(Buffer.from(sourcePayload).readUInt32LE(18), ROUTE_REVISION);
  assert.equal(
    parsedSource[0].args.messageId,
    messageId("bsc-testnet", "sora-taira", sourcePayload),
  );
  assert.equal(
    parsedSource[0].args.sourceEventDigest,
    await bridge.sourceEventDigest(
      parsedSource[0].args.messageId,
      parsedSource[0].args.payloadHash,
    ),
  );

  const retiredSelector = ethers
    .id("submitSccpSourceEvent(bytes32,bytes32)")
    .slice(0, 10);
  await assert.rejects(
    signer.sendTransaction({
      to: bridgeAddress,
      data: ethers.concat([retiredSelector, ethers.ZeroHash, ethers.ZeroHash]),
    }),
    rejectedWith(),
  );

  const predictedFalseBridgeAddress = await nextCreateAddress(signer, 1);
  const falseToken = await deploy(signer, falseTokenArtifact, [
    predictedFalseBridgeAddress,
  ]);
  const falseBridge = await deploy(signer, injectedBscRouteArtifact, [
    await falseToken.getAddress(),
    verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
    ROUTE_REVISION,
    MAX_WRAPPED_SUPPLY,
  ]);
  assert.equal(await falseBridge.getAddress(), predictedFalseBridgeAddress);
  await (await falseToken.seed(signerAddress, SCALE)).wait();
  await assert.rejects(
    falseBridge.transferToTaira(CANONICAL_I105_BYTES, SCALE),
    rejectedWith("Token burn failed"),
  );
  assert.equal(await falseBridge.transferNonces(signerAddress), 0n);

  const falseMintPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_BSC,
    nonce: 10,
    amount: 1,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: recipient20,
  });
  const falseMintPayloadHex = ethers.hexlify(falseMintPayload);
  const falseMintMessageId =
    await falseBridge.sccpDestinationMessageId(falseMintPayloadHex);
  const falseMintPublicInputs = [
    falseMintMessageId,
    await falseBridge.sccpPayloadHash(falseMintPayloadHex),
    word(DOMAIN_BSC),
    ethers.keccak256(ethers.toUtf8Bytes("false-mint-commitment-root")),
    word(101),
    ethers.keccak256(ethers.toUtf8Bytes("false-mint-finality-block")),
  ];
  const falseMintStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("false-mint-statement"),
  );
  const falseMintProof = await acceptingProof(
    provider,
    abi,
    falseMintPublicInputs,
    falseMintStatementHash,
    await falseBridge.destinationBindingHash(),
    await falseBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  assert.equal(
    await falseBridge.usedDestinationMessages(falseMintMessageId),
    false,
  );
  await assert.rejects(
    falseBridge.finalizeFromTaira(
      falseMintProof,
      falseMintPublicInputs,
      falseMintStatementHash,
      falseMintPayloadHex,
    ),
    rejectedWith("Token mint failed"),
  );
  assert.equal(
    await falseBridge.usedDestinationMessages(falseMintMessageId),
    false,
    "destination-message consumption must roll back when minting fails",
  );

  const predictedNoopBridgeAddress = await nextCreateAddress(signer, 1);
  const trueNoopToken = await deploy(signer, trueNoopTokenArtifact, [
    predictedNoopBridgeAddress,
  ]);
  const trueNoopBridge = await deploy(signer, injectedBscRouteArtifact, [
    await trueNoopToken.getAddress(),
    verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
    ROUTE_REVISION,
    MAX_WRAPPED_SUPPLY,
  ]);
  assert.equal(await trueNoopBridge.getAddress(), predictedNoopBridgeAddress);
  await (await trueNoopToken.seed(signerAddress, 2n * SCALE)).wait();
  await assert.rejects(
    trueNoopBridge.transferToTaira(CANONICAL_I105_BYTES, SCALE),
    rejectedWith("Token delta mismatch"),
  );
  assert.equal(await trueNoopBridge.transferNonces(signerAddress), 0n);
  assert.equal(await trueNoopToken.totalSupply(), 2n * SCALE);
  assert.equal(await trueNoopToken.balanceOf(signerAddress), 2n * SCALE);

  const noopMintPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_BSC,
    nonce: 11,
    amount: 1,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: recipient20,
  });
  const noopMintPayloadHex = ethers.hexlify(noopMintPayload);
  const noopMintMessageId =
    await trueNoopBridge.sccpDestinationMessageId(noopMintPayloadHex);
  const noopMintPublicInputs = [
    noopMintMessageId,
    await trueNoopBridge.sccpPayloadHash(noopMintPayloadHex),
    word(DOMAIN_BSC),
    ethers.keccak256(ethers.toUtf8Bytes("noop-mint-commitment-root")),
    word(102),
    ethers.keccak256(ethers.toUtf8Bytes("noop-mint-finality-block")),
  ];
  const noopMintStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("noop-mint-statement"),
  );
  const noopMintProof = await acceptingProof(
    provider,
    abi,
    noopMintPublicInputs,
    noopMintStatementHash,
    await trueNoopBridge.destinationBindingHash(),
    await trueNoopBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    trueNoopBridge.finalizeFromTaira(
      noopMintProof,
      noopMintPublicInputs,
      noopMintStatementHash,
      noopMintPayloadHex,
    ),
    rejectedWith("Token delta mismatch"),
  );
  assert.equal(await trueNoopBridge.usedDestinationMessages(noopMintMessageId), false);
  assert.equal(await trueNoopToken.totalSupply(), 2n * SCALE);
  assert.equal(await trueNoopToken.balanceOf(signerAddress), 2n * SCALE);

  const predictedWrongDeltaBridgeAddress = await nextCreateAddress(signer, 1);
  const wrongDeltaToken = await deploy(signer, wrongDeltaTokenArtifact, [
    predictedWrongDeltaBridgeAddress,
  ]);
  const wrongDeltaBridge = await deploy(signer, injectedBscRouteArtifact, [
    await wrongDeltaToken.getAddress(),
    verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
    ROUTE_REVISION,
    MAX_WRAPPED_SUPPLY,
  ]);
  assert.equal(
    await wrongDeltaBridge.getAddress(),
    predictedWrongDeltaBridgeAddress,
  );
  await (await wrongDeltaToken.seed(outsiderAddress, 2n * SCALE)).wait();
  await assert.rejects(
    wrongDeltaBridge
      .connect(outsider)
      .transferToTaira(CANONICAL_I105_BYTES, SCALE),
    rejectedWith("Token delta mismatch"),
  );
  assert.equal(await wrongDeltaBridge.transferNonces(outsiderAddress), 0n);
  assert.equal(await wrongDeltaToken.totalSupply(), 2n * SCALE);
  assert.equal(await wrongDeltaToken.balanceOf(outsiderAddress), 2n * SCALE);

  const wrongDeltaMintPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_BSC,
    nonce: 12,
    amount: 1,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: recipient20,
  });
  const wrongDeltaMintPayloadHex = ethers.hexlify(wrongDeltaMintPayload);
  const wrongDeltaMintMessageId =
    await wrongDeltaBridge.sccpDestinationMessageId(wrongDeltaMintPayloadHex);
  const wrongDeltaMintPublicInputs = [
    wrongDeltaMintMessageId,
    await wrongDeltaBridge.sccpPayloadHash(wrongDeltaMintPayloadHex),
    word(DOMAIN_BSC),
    ethers.keccak256(ethers.toUtf8Bytes("wrong-delta-mint-commitment-root")),
    word(103),
    ethers.keccak256(ethers.toUtf8Bytes("wrong-delta-mint-finality-block")),
  ];
  const wrongDeltaMintStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-delta-mint-statement"),
  );
  const wrongDeltaMintProof = await acceptingProof(
    provider,
    abi,
    wrongDeltaMintPublicInputs,
    wrongDeltaMintStatementHash,
    await wrongDeltaBridge.destinationBindingHash(),
    await wrongDeltaBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );
  await assert.rejects(
    wrongDeltaBridge.finalizeFromTaira(
      wrongDeltaMintProof,
      wrongDeltaMintPublicInputs,
      wrongDeltaMintStatementHash,
      wrongDeltaMintPayloadHex,
    ),
    rejectedWith("Token delta mismatch"),
  );
  assert.equal(
    await wrongDeltaBridge.usedDestinationMessages(wrongDeltaMintMessageId),
    false,
  );
  assert.equal(await wrongDeltaToken.totalSupply(), 2n * SCALE);
  assert.equal(await wrongDeltaToken.balanceOf(signerAddress), 0n);

  const predictedReentrantBridgeAddress = await nextCreateAddress(signer, 1);
  const reentrantToken = await deploy(signer, reentrantTokenArtifact, [
    predictedReentrantBridgeAddress,
    CANONICAL_I105_BYTES,
  ]);
  assert.equal(
    await reentrantToken.bridge(),
    await nextCreateAddress(signer),
    "reentrant token must bind the immediately following CREATE address",
  );
  const reentrantBridge = await deploy(signer, injectedBscRouteArtifact, [
    await reentrantToken.getAddress(),
    verifierPolicy(verifierAddress, verifierCodeHash, verifierKeyHash),
    ROUTE_REVISION,
    MAX_WRAPPED_SUPPLY,
  ]);
  assert.equal(
    await reentrantBridge.getAddress(),
    predictedReentrantBridgeAddress,
  );
  await (await reentrantToken.seed(outsiderAddress, SCALE)).wait();
  const reentrantReceipt = await (
    await reentrantBridge
      .connect(outsider)
      .transferToTaira(CANONICAL_I105_BYTES, SCALE)
  ).wait();
  const reentrantBridgeAddress = await reentrantBridge.getAddress();
  const reentrantEvents = reentrantReceipt.logs.filter(
    (log) => log.address.toLowerCase() === reentrantBridgeAddress.toLowerCase(),
  );
  assert.equal(reentrantEvents.length, 1);
  assert.equal(await reentrantBridge.transferNonces(outsiderAddress), 1n);
  assert.equal(await reentrantToken.totalSupply(), 0n);
  assert.equal(await reentrantToken.balanceOf(outsiderAddress), 0n);

  const bscSourceLaneHash = await bridge.sourceLaneHash();
  const bscDestinationLaneHash = await bridge.destinationLaneHash();
  const bscRouteConfigHash = await bridge.routeConfigHash();
  await bscEip1193Provider.disconnect();

  const ethereumEip1193Provider = createHardhatProvider({
    chainId: 11_155_111,
    blockGasLimit: Number(MAX_DEPLOYMENT_GAS + 5_000_000n),
  });
  const ethereumProvider = new ethers.BrowserProvider(ethereumEip1193Provider);
  assert.equal((await ethereumProvider.getNetwork()).chainId, 11_155_111n);
  const ethereumSigner = await ethereumProvider.getSigner(0);
  const ethereumVerifier = await deploy(ethereumSigner, verifierArtifact, [
    g1,
    g2,
    g2,
    g2,
    Array(12).fill(g1).flat(),
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
  ]);
  const ethereumVerifierAddress = await ethereumVerifier.getAddress();
  const ethereumVerifierCodeHash = ethers.keccak256(
    await ethereumProvider.getCode(ethereumVerifierAddress),
  );
  const ethereumVerifierKeyHash = await ethereumVerifier.verifyingKeyHash();
  assert.equal(
    await ethereumVerifier.semanticProofProfileHash(),
    SEMANTIC_PROOF_PROFILE_HASH,
  );
  assert.equal(
    await ethereumVerifier.soraFinalityAnchorHash(),
    SORA_FINALITY_ANCHOR_HASH,
  );
  const ethereumRoute = await deployPreboundRoute(
    ethereumSigner,
    ethereumTokenArtifact,
    ethereumBridgeArtifact,
    [
      verifierPolicy(
        ethereumVerifierAddress,
        ethereumVerifierCodeHash,
        ethereumVerifierKeyHash,
      ),
      ETHEREUM_SEPOLIA_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ],
  );
  const ethereumBridge = ethereumRoute.route;
  const ethereumBridgeAddress = await ethereumBridge.getAddress();
  const ethereumToken = ethereumRoute.token;
  const ethereumTokenAddress = await ethereumToken.getAddress();
  assert.equal(await ethereumToken.bridge(), ethereumBridgeAddress);
  assert.equal(await ethereumBridge.maxWrappedSupply(), MAX_WRAPPED_SUPPLY);
  assert.equal(
    await ethereumBridge.semanticProofProfileHash(),
    SEMANTIC_PROOF_PROFILE_HASH,
  );
  assert.equal(
    await ethereumBridge.soraFinalityAnchorHash(),
    SORA_FINALITY_ANCHOR_HASH,
  );
  const ethereumTokenCodeHash = ethers.keccak256(
    await ethereumProvider.getCode(ethereumTokenAddress),
  );
  assert.equal(await ethereumBridge.tokenCodeHash(), ethereumTokenCodeHash);
  assert.equal(
    await ethereumBridge.routeConfigHash(),
    exactEvmRouteConfigHash({
      abi,
      domain: DOMAIN_ETHEREUM,
      profile: ETHEREUM_SEPOLIA_PROFILE,
      chainId: 11_155_111,
      sourceLaneHash: await ethereumBridge.sourceLaneHash(),
      destinationLaneHash: await ethereumBridge.destinationLaneHash(),
      tokenAddress: ethereumTokenAddress,
      tokenCodeHash: ethereumTokenCodeHash,
      verifierAddress: ethereumVerifierAddress,
      verifierCodeHash: ethereumVerifierCodeHash,
      verifierKeyHash: ethereumVerifierKeyHash,
      semanticProofProfileHash: SEMANTIC_PROOF_PROFILE_HASH,
      soraFinalityAnchorHash: SORA_FINALITY_ANCHOR_HASH,
      route: "taira_eth_xor",
      routeRevision: ROUTE_REVISION,
      maxWrappedSupply: MAX_WRAPPED_SUPPLY,
    }),
  );
  assert.equal(await ethereumBridge.ethereumProfile(), 3n);
  assert.equal(await ethereumBridge.networkProfile(), 3n);
  assert.equal(await ethereumBridge.externalDomain(), 1n);
  assert.equal(await ethereumBridge.externalChainId(), 11_155_111n);
  assert.equal(await ethereumBridge.routeRevision(), BigInt(ROUTE_REVISION));
  assert.notEqual(await ethereumBridge.sourceLaneHash(), bscSourceLaneHash);
  assert.notEqual(
    await ethereumBridge.destinationLaneHash(),
    bscDestinationLaneHash,
  );
  assert.notEqual(await ethereumBridge.routeConfigHash(), bscRouteConfigHash);

  const rejectedEthereumRouteAddress = await nextCreateAddress(
    ethereumSigner,
    1,
  );
  const rejectedEthereumToken = await deploy(
    ethereumSigner,
    ethereumTokenArtifact,
    [rejectedEthereumRouteAddress],
  );
  const rejectedEthereumTokenAddress = await rejectedEthereumToken.getAddress();
  await assert.rejects(
    deploy(ethereumSigner, ethereumBridgeArtifact, [
      rejectedEthereumTokenAddress,
      verifierPolicy(
        ethereumVerifierAddress,
        ethereumVerifierCodeHash,
        ethereumVerifierKeyHash,
      ),
      2,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(ethereumSigner, bridgeArtifact, [
      rejectedEthereumTokenAddress,
      verifierPolicy(
        ethereumVerifierAddress,
        ethereumVerifierCodeHash,
        ethereumVerifierKeyHash,
      ),
      ETHEREUM_SEPOLIA_PROFILE,
      ROUTE_REVISION,
      MAX_WRAPPED_SUPPLY,
    ]),
    rejectedWith(),
  );

  const ethereumSignerAddress = await ethereumSigner.getAddress();
  const ethereumRecipient20 = Buffer.from(
    ethereumSignerAddress.slice(2),
    "hex",
  );
  const ethereumInboundPayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_ETHEREUM,
    nonce: 17,
    amount: 7,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: ethereumRecipient20,
    route: "taira_eth_xor",
  });
  const ethereumPayloadHex = ethers.hexlify(ethereumInboundPayload);
  const ethereumPayloadHash =
    await ethereumBridge.sccpPayloadHash(ethereumPayloadHex);
  const ethereumMessageId =
    await ethereumBridge.sccpDestinationMessageId(ethereumPayloadHex);
  assert.equal(
    ethereumMessageId,
    messageId("sora-taira", "ethereum-sepolia", ethereumInboundPayload),
  );
  const ethereumPublicInputs = [
    ethereumMessageId,
    ethereumPayloadHash,
    word(DOMAIN_ETHEREUM),
    ethers.keccak256(ethers.toUtf8Bytes("ethereum-commitment-root")),
    word(200),
    ethers.keccak256(ethers.toUtf8Bytes("ethereum-finality-block")),
  ];
  const ethereumStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("exact-taira-ethereum-statement"),
  );
  const ethereumProof = await acceptingProof(
    ethereumProvider,
    abi,
    ethereumPublicInputs,
    ethereumStatementHash,
    await ethereumBridge.destinationBindingHash(),
    await ethereumBridge.routeConfigHash(),
    SORA_FINALITY_ANCHOR_HASH,
    g1,
    g2,
  );

  await assert.rejects(
    ethereumBridge.finalizeFromTaira(
      proof,
      publicInputs,
      statementHash,
      payloadHex,
    ),
    rejectedWith("Unexpected target domain"),
  );
  await assert.rejects(
    ethereumBridge.finalizeFromTaira(
      ethereumProof,
      [
        messageId("sora-taira", "ethereum-mainnet", ethereumInboundPayload),
        ...ethereumPublicInputs.slice(1),
      ],
      ethereumStatementHash,
      ethereumPayloadHex,
    ),
    rejectedWith("Message id mismatch"),
  );
  const wrongEthereumRoutePayload = transferPayload({
    sourceDomain: DOMAIN_SORA,
    destinationDomain: DOMAIN_ETHEREUM,
    nonce: 17,
    amount: 7,
    senderCodec: CODEC_TEXT,
    sender: Buffer.from(CANONICAL_I105_BYTES),
    recipientCodec: CODEC_EVM20,
    recipient: ethereumRecipient20,
    route: "taira_bsc_xor",
  });
  await assert.rejects(
    ethereumBridge.finalizeFromTaira(
      ethereumProof,
      ethereumPublicInputs,
      ethereumStatementHash,
      wrongEthereumRoutePayload,
    ),
    rejectedWith("Wrong route"),
  );
  await assert.rejects(
    ethereumBridge.finalizeFromTaira(
      "0x00",
      ethereumPublicInputs,
      ethereumStatementHash,
      ethereumPayloadHex,
    ),
    rejectedWith("Unexpected Groth16 proof length"),
  );

  await (
    await ethereumBridge.finalizeFromTaira(
      ethereumProof,
      ethereumPublicInputs,
      ethereumStatementHash,
      ethereumPayloadHex,
    )
  ).wait();
  assert.equal(
    await ethereumToken.balanceOf(ethereumSignerAddress),
    7n * SCALE,
  );
  await assert.rejects(
    ethereumBridge.finalizeFromTaira(
      ethereumProof,
      ethereumPublicInputs,
      ethereumStatementHash,
      ethereumPayloadHex,
    ),
    rejectedWith("Destination message already used"),
  );

  const ethereumSourceReceipt = await (
    await ethereumBridge.transferToTaira(CANONICAL_I105_BYTES, SCALE)
  ).wait();
  const ethereumSourceEvents = ethereumSourceReceipt.logs
    .filter(
      (log) =>
        log.address.toLowerCase() === ethereumBridgeAddress.toLowerCase(),
    )
    .map((log) => {
      try {
        return ethereumBridge.interface.parseLog(log);
      } catch (_) {
        return null;
      }
    })
    .filter((log) => log && log.name === "SccpTransfer");
  assert.equal(ethereumSourceEvents.length, 1);
  const ethereumSourceEvent = ethereumSourceEvents[0].args;
  const ethereumSourcePayload = ethers.getBytes(
    ethereumSourceEvent.canonicalPayload,
  );
  assert.equal(
    Buffer.from(ethereumSourcePayload).readUInt32LE(18),
    ROUTE_REVISION,
  );
  assert.equal(
    ethereumSourceEvent.messageId,
    messageId("ethereum-sepolia", "sora-taira", ethereumSourcePayload),
  );
  assert.equal(
    ethereumSourceEvent.laneHash,
    await ethereumBridge.sourceLaneHash(),
  );
  assert.equal(
    ethereumSourceEvent.routeConfigHash,
    await ethereumBridge.routeConfigHash(),
  );
  assert.equal(
    ethereumSourceEvent.sourceEventDigest,
    await ethereumBridge.sourceEventDigest(
      ethereumSourceEvent.messageId,
      ethereumSourceEvent.payloadHash,
    ),
  );
  assert.equal(
    await ethereumToken.balanceOf(ethereumSignerAddress),
    6n * SCALE,
  );
  assert.equal(
    await ethereumBridge.transferNonces(await ethereumSigner.getAddress()),
    1n,
  );

  await assert.rejects(
    ethereumSigner.sendTransaction({
      to: ethereumBridgeAddress,
      data: ethers.concat([retiredSelector, ethers.ZeroHash, ethers.ZeroHash]),
    }),
    rejectedWith(),
  );

  await ethereumEip1193Provider.disconnect();
  console.log(`sccp_runtime_bytes: ${runtimeSizes.join(", ")}`);
  console.log("sccp_message_bridge_smoke: ok");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
