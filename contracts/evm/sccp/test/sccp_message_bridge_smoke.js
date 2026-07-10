const fs = require("fs");
const path = require("path");
const assert = require("assert");
const solc = require("solc");
const ganache = require("ganache");
const { ethers } = require("ethers");

const REPO = path.join(__dirname, "..", "..", "..", "..");
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
const SEMANTIC_PROOF_PROFILE_HASH = ethers.keccak256(
  ethers.toUtf8Bytes("sccp:test:semantic-proof-profile:v1"),
);
const SORA_FINALITY_ANCHOR_HASH = ethers.keccak256(
  ethers.toUtf8Bytes("sccp:test:sora-finality-anchor:v1"),
);
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

function source(file) {
  return { content: fs.readFileSync(path.join(REPO, file), "utf8") };
}

function compile() {
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
pragma solidity ^0.7.4;
import "contracts/evm/sccp/SccpExactTransferCodec.sol";
interface IReentryRoute { function transferToTaira(bytes calldata,uint256) external returns(bytes32); }
contract CodecHarness {
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
contract FalseToken {
  address public immutable bridge;
  constructor(address routeBridge) { bridge = routeBridge; }
  function mint(address,uint256) external pure returns(bool) { return false; }
  function burnFrom(address,uint256) external pure returns(bool) { return false; }
}
contract ReentrantToken {
  address public immutable bridge;
  bool private entered;
  constructor(address routeBridge) { bridge = routeBridge; }
  function mint(address,uint256) external pure returns(bool) { return true; }
  function burnFrom(address,uint256) external returns(bool) {
    require(msg.sender == bridge && !entered, "bad reentry setup");
    entered = true;
    (bool success,) = bridge.call(abi.encodeWithSignature("transferToTaira(bytes,uint256)", bytes("mallory@taira"), uint256(1e9)));
    require(!success, "reentry was accepted");
    entered = false;
    return true;
  }
}`;
  const sources = Object.fromEntries(files.map((file) => [file, source(file)]));
  sources["Mocks.sol"] = { content: mocks };
  const output = JSON.parse(
    solc.compile(
      JSON.stringify({
        language: "Solidity",
        sources,
        settings: {
          optimizer: { enabled: true, runs: 200 },
          outputSelection: {
            "*": {
              "*": [
                "abi",
                "evm.bytecode.object",
                "evm.deployedBytecode.object",
              ],
            },
          },
        },
      }),
    ),
  );
  if (output.errors) {
    const errors = output.errors.filter((entry) => entry.severity === "error");
    if (errors.length)
      throw new Error(errors.map((entry) => entry.formattedMessage).join("\n"));
  }
  return output.contracts;
}

function artifact(contracts, file, name) {
  const value = contracts[file][name];
  return {
    abi: value.abi,
    bytecode: `0x${value.evm.bytecode.object}`,
    runtimeBytecode: `0x${value.evm.deployedBytecode.object}`,
  };
}

async function deploy(signer, value, args = []) {
  const contract = await new ethers.ContractFactory(
    value.abi,
    value.bytecode,
    signer,
  ).deploy(...args, {
    gasLimit: 90_000_000,
  });
  await contract.waitForDeployment();
  return contract;
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

async function deployTokenBoundToNextRoute(signer, tokenArtifact) {
  const routeAddress = await nextCreateAddress(signer, 1);
  const token = await deploy(signer, tokenArtifact, [routeAddress]);
  return { token, routeAddress };
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
    return Buffer.from("010100000000809574f5fee75e69bfcf52451e42d50f", "hex");
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
      ["bytes32", "bytes32", "uint32", "uint256"],
      [
        ethers.keccak256(ethers.toUtf8Bytes("xor")),
        ethers.keccak256(ethers.toUtf8Bytes(route)),
        routeRevision,
        SCALE,
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
      ["bytes32", "bytes32", "uint32", "uint256"],
      [
        ethers.keccak256(ethers.toUtf8Bytes("xor")),
        ethers.keccak256(ethers.toUtf8Bytes("taira_tron_xor")),
        routeRevision,
        SCALE,
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
        ethers.keccak256(ethers.toUtf8Bytes("stark-fri-v1")),
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
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, publicInputs[0], DOMAIN_SORA, publicInputs[3], g1, g2, c],
  );
}

function rejectedWith(reason) {
  return (error) => {
    const text = [
      error.reason,
      error.shortMessage,
      error.message,
      error.info?.error?.message,
    ]
      .filter(Boolean)
      .join("\n");
    return (
      error.code === "CALL_EXCEPTION" && (!reason || text.includes(reason))
    );
  };
}

async function main() {
  const contracts = compile();
  const ganacheProvider = ganache.provider({
    logging: { quiet: true },
    chain: { chainId: 97, allowUnlimitedContractSize: true },
    miner: { blockGasLimit: 100_000_000 },
    wallet: { totalAccounts: 4, defaultBalance: 10_000 },
  });
  const provider = new ethers.BrowserProvider(ganacheProvider);
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
    contracts,
    "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    "SccpTronGroth16Bn254MessageVerifier",
  );
  const tronTokenArtifact = artifact(
    contracts,
    "contracts/tron/sccp/TairaXOR.sol",
    "TairaXOR",
  );
  const tronBridgeArtifact = artifact(
    contracts,
    "contracts/tron/sccp/TairaXorSccpBridge.sol",
    "TairaXorSccpBridge",
  );
  const falseTokenArtifact = artifact(contracts, "Mocks.sol", "FalseToken");
  const reentrantTokenArtifact = artifact(
    contracts,
    "Mocks.sol",
    "ReentrantToken",
  );
  const codecHarnessArtifact = artifact(contracts, "Mocks.sol", "CodecHarness");

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
      ethers.getBytes(deploymentArtifact.runtimeBytecode).length <= 24_576,
      `${label} runtime exceeds the 24,576-byte deployment ceiling`,
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
  }

  assert(!bridgeArtifact.abi.some((entry) => entry.name === "burnToTaira"));
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
  const codecHarness = await deploy(signer, codecHarnessArtifact);
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
    Array(12).fill(g1).flat(),
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
  ]);
  const verifierAddress = await verifier.getAddress();
  const verifierCodeHash = ethers.keccak256(
    await provider.getCode(verifierAddress),
  );
  const verifierKeyHash = await verifier.verifyingKeyHash();
  assert.equal(
    await verifier.semanticProofProfileHash(),
    SEMANTIC_PROOF_PROFILE_HASH,
  );
  assert.equal(
    await verifier.soraFinalityAnchorHash(),
    SORA_FINALITY_ANCHOR_HASH,
  );

  const tronNetworkId = word(0xcd8690dc);
  const tronVerifier = await deploy(signer, tronVerifierArtifact, [
    g1,
    g2,
    g2,
    g2,
    Array(12).fill(g1).flat(),
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    verifierKeyHash,
    "stark-fri-v1",
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

  const predictedTronBridgeAddress = await nextCreateAddress(signer, 1);
  const tronToken = await deploy(signer, tronTokenArtifact, [
    predictedTronBridgeAddress,
  ]);
  const tronTokenAddress = await tronToken.getAddress();
  const tronTokenCodeHash = ethers.keccak256(
    await provider.getCode(tronTokenAddress),
  );
  const tronBridge = await deploy(signer, tronBridgeArtifact, [
    tronTokenAddress,
    tronVerifierAddress,
    tronVerifierCodeHash,
    verifierKeyHash,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    TRON_NILE_PROFILE,
    ROUTE_REVISION,
  ]);
  assert.equal(await tronBridge.getAddress(), predictedTronBridgeAddress);
  assert.equal(await tronToken.bridge(), predictedTronBridgeAddress);
  assert.equal(await tronBridge.routeRevision(), BigInt(ROUTE_REVISION));
  const predictedSecondTronBridgeAddress = await nextCreateAddress(signer, 1);
  const secondTronToken = await deploy(signer, tronTokenArtifact, [
    predictedSecondTronBridgeAddress,
  ]);
  const secondTronBridge = await deploy(signer, tronBridgeArtifact, [
    await secondTronToken.getAddress(),
    tronVerifierAddress,
    tronVerifierCodeHash,
    verifierKeyHash,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    TRON_NILE_PROFILE,
    ROUTE_REVISION,
  ]);
  const tronBridgeAddress = await tronBridge.getAddress();
  const secondTronBridgeAddress = await secondTronBridge.getAddress();
  assert.equal(secondTronBridgeAddress, predictedSecondTronBridgeAddress);
  assert.equal(await secondTronToken.bridge(), secondTronBridgeAddress);
  const zeroRevisionTronRoute = await deployTokenBoundToNextRoute(
    signer,
    tronTokenArtifact,
  );
  await assert.rejects(
    deploy(signer, tronBridgeArtifact, [
      await zeroRevisionTronRoute.token.getAddress(),
      tronVerifierAddress,
      tronVerifierCodeHash,
      verifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      TRON_NILE_PROFILE,
      0,
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
    sender: Buffer.from("alice@taira"),
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
  await assert.rejects(
    tronBridge.transferToTaira(ethers.toUtf8Bytes("bob@taira"), 1n),
    rejectedWith("Amount is not aligned to Taira scale"),
  );
  const tronSourceReceipt = await (
    await tronBridge.transferToTaira(ethers.toUtf8Bytes("bob@taira"), SCALE)
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
    2n * SCALE,
  );
  assert.equal(await tronBridge.transferNonce(), 1n);

  const predictedBridgeAddress = await nextCreateAddress(signer, 1);
  const token = await deploy(signer, tokenArtifact, [predictedBridgeAddress]);
  const tokenAddress = await token.getAddress();
  const bridge = await deploy(signer, bridgeArtifact, [
    tokenAddress,
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    BSC_TESTNET_PROFILE,
    ROUTE_REVISION,
  ]);
  const bridgeAddress = await bridge.getAddress();
  assert.equal(bridgeAddress, predictedBridgeAddress);
  assert.equal(await token.bridge(), bridgeAddress);
  assert.equal(await bridge.routeRevision(), BigInt(ROUTE_REVISION));
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
    }),
  );
  const wrongBoundToken = await deploy(signer, tokenArtifact, [
    await outsider.getAddress(),
  ]);
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      await wrongBoundToken.getAddress(),
      verifierAddress,
      verifierCodeHash,
      verifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      BSC_TESTNET_PROFILE,
      ROUTE_REVISION,
    ]),
    rejectedWith(),
  );
  const zeroRevisionRoute = await deployTokenBoundToNextRoute(
    signer,
    tokenArtifact,
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      await zeroRevisionRoute.token.getAddress(),
      verifierAddress,
      verifierCodeHash,
      verifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      BSC_TESTNET_PROFILE,
      0,
    ]),
    rejectedWith(),
  );
  const zeroCodeRoute = await deployTokenBoundToNextRoute(signer, tokenArtifact);
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      await zeroCodeRoute.token.getAddress(),
      verifierAddress,
      ethers.ZeroHash,
      verifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      5,
      ROUTE_REVISION,
    ]),
    rejectedWith(),
  );
  const zeroKeyRoute = await deployTokenBoundToNextRoute(signer, tokenArtifact);
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      await zeroKeyRoute.token.getAddress(),
      verifierAddress,
      verifierCodeHash,
      ethers.ZeroHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      5,
      ROUTE_REVISION,
    ]),
    rejectedWith(),
  );
  const wrongProfileRoute = await deployTokenBoundToNextRoute(
    signer,
    tokenArtifact,
  );
  await assert.rejects(
    deploy(signer, bridgeArtifact, [
      await wrongProfileRoute.token.getAddress(),
      verifierAddress,
      verifierCodeHash,
      verifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      4,
      ROUTE_REVISION,
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
    sender: Buffer.from("alice@taira"),
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
    sender: Buffer.from("alice@taira"),
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

  const invalidRecipients = [
    "0x",
    ethers.hexlify(Buffer.from(" bad")),
    `0x${"61".repeat(257)}`,
  ];
  for (const recipient of invalidRecipients) {
    await assert.rejects(
      bridge.transferToTaira(recipient, SCALE),
      rejectedWith(),
    );
  }
  for (const amount of [0n, SCALE + 1n, ((1n << 128n) + 1n) * SCALE]) {
    await assert.rejects(
      bridge.transferToTaira(ethers.toUtf8Bytes("bob@taira"), amount),
      rejectedWith(),
    );
  }
  const sourceTx = await bridge.transferToTaira(
    ethers.toUtf8Bytes("bob@taira"),
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
  assert.equal(await bridge.transferNonce(), 1n);
  assert.equal(await token.balanceOf(signerAddress), 4n * SCALE);
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
  const falseBridge = await deploy(signer, bridgeArtifact, [
    await falseToken.getAddress(),
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    BSC_TESTNET_PROFILE,
    ROUTE_REVISION,
  ]);
  assert.equal(await falseBridge.getAddress(), predictedFalseBridgeAddress);
  await assert.rejects(
    falseBridge.transferToTaira(ethers.toUtf8Bytes("bob@taira"), SCALE),
    rejectedWith("Token burn failed"),
  );
  assert.equal(await falseBridge.transferNonce(), 0n);

  const predictedReentrantBridgeAddress = await nextCreateAddress(signer, 1);
  const reentrantToken = await deploy(signer, reentrantTokenArtifact, [
    predictedReentrantBridgeAddress,
  ]);
  assert.equal(
    await reentrantToken.bridge(),
    await nextCreateAddress(signer),
    "reentrant token must bind the immediately following CREATE address",
  );
  const reentrantBridge = await deploy(signer, bridgeArtifact, [
    await reentrantToken.getAddress(),
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    BSC_TESTNET_PROFILE,
    ROUTE_REVISION,
  ]);
  assert.equal(
    await reentrantBridge.getAddress(),
    predictedReentrantBridgeAddress,
  );
  const reentrantReceipt = await (
    await reentrantBridge
      .connect(outsider)
      .transferToTaira(ethers.toUtf8Bytes("bob@taira"), SCALE)
  ).wait();
  const reentrantBridgeAddress = await reentrantBridge.getAddress();
  const reentrantEvents = reentrantReceipt.logs.filter(
    (log) => log.address.toLowerCase() === reentrantBridgeAddress.toLowerCase(),
  );
  assert.equal(reentrantEvents.length, 1);
  assert.equal(await reentrantBridge.transferNonce(), 1n);

  const bscSourceLaneHash = await bridge.sourceLaneHash();
  const bscDestinationLaneHash = await bridge.destinationLaneHash();
  const bscRouteConfigHash = await bridge.routeConfigHash();
  await ganacheProvider.disconnect();

  const ethereumGanacheProvider = ganache.provider({
    logging: { quiet: true },
    chain: { chainId: 11_155_111, allowUnlimitedContractSize: true },
    miner: { blockGasLimit: 100_000_000 },
    wallet: { totalAccounts: 4, defaultBalance: 10_000 },
  });
  const ethereumProvider = new ethers.BrowserProvider(ethereumGanacheProvider);
  const ethereumSigner = await ethereumProvider.getSigner(0);
  const ethereumOutsider = await ethereumProvider.getSigner(1);
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
  const predictedEthereumBridgeAddress = await nextCreateAddress(
    ethereumSigner,
    1,
  );
  const ethereumToken = await deploy(ethereumSigner, ethereumTokenArtifact, [
    predictedEthereumBridgeAddress,
  ]);
  const ethereumTokenAddress = await ethereumToken.getAddress();
  const ethereumBridge = await deploy(ethereumSigner, ethereumBridgeArtifact, [
    ethereumTokenAddress,
    ethereumVerifierAddress,
    ethereumVerifierCodeHash,
    ethereumVerifierKeyHash,
    SEMANTIC_PROOF_PROFILE_HASH,
    SORA_FINALITY_ANCHOR_HASH,
    ETHEREUM_SEPOLIA_PROFILE,
    ROUTE_REVISION,
  ]);
  const ethereumBridgeAddress = await ethereumBridge.getAddress();
  assert.equal(ethereumBridgeAddress, predictedEthereumBridgeAddress);
  assert.equal(await ethereumToken.bridge(), ethereumBridgeAddress);
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

  await assert.rejects(
    deploy(ethereumSigner, ethereumBridgeArtifact, [
      ethereumVerifierAddress,
      ethereumVerifierAddress,
      ethereumVerifierCodeHash,
      ethereumVerifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      ETHEREUM_SEPOLIA_PROFILE,
      ROUTE_REVISION,
    ]),
    rejectedWith(),
  );
  await assert.rejects(
    deploy(ethereumSigner, ethereumBridgeArtifact, [
      await ethereumOutsider.getAddress(),
      ethereumVerifierAddress,
      ethereumVerifierCodeHash,
      ethereumVerifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      ETHEREUM_SEPOLIA_PROFILE,
      ROUTE_REVISION,
    ]),
    rejectedWith(),
  );
  const wrongEthereumProfileRoute = await deployTokenBoundToNextRoute(
    ethereumSigner,
    ethereumTokenArtifact,
  );
  await assert.rejects(
    deploy(ethereumSigner, ethereumBridgeArtifact, [
      await wrongEthereumProfileRoute.token.getAddress(),
      ethereumVerifierAddress,
      ethereumVerifierCodeHash,
      ethereumVerifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      2,
      ROUTE_REVISION,
    ]),
    rejectedWith(),
  );
  const crossFamilyRoute = await deployTokenBoundToNextRoute(
    ethereumSigner,
    tokenArtifact,
  );
  await assert.rejects(
    deploy(ethereumSigner, bridgeArtifact, [
      await crossFamilyRoute.token.getAddress(),
      ethereumVerifierAddress,
      ethereumVerifierCodeHash,
      ethereumVerifierKeyHash,
      SEMANTIC_PROOF_PROFILE_HASH,
      SORA_FINALITY_ANCHOR_HASH,
      ETHEREUM_SEPOLIA_PROFILE,
      ROUTE_REVISION,
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
    sender: Buffer.from("alice@taira"),
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
    sender: Buffer.from("alice@taira"),
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
    await ethereumBridge.transferToTaira(ethers.toUtf8Bytes("bob@taira"), SCALE)
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
  assert.equal(await ethereumBridge.transferNonce(), 1n);

  await assert.rejects(
    ethereumSigner.sendTransaction({
      to: ethereumBridgeAddress,
      data: ethers.concat([retiredSelector, ethers.ZeroHash, ethers.ZeroHash]),
    }),
    rejectedWith(),
  );

  await ethereumGanacheProvider.disconnect();
  console.log("sccp_message_bridge_smoke: ok");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
