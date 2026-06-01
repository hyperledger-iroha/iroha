const fs = require("fs");
const path = require("path");
const assert = require("assert");
const solc = require("solc");
const ganache = require("ganache");
const { ethers } = require("ethers");

function contractPath(fileName) {
  return path.join(__dirname, "..", fileName);
}

function repoPath(...segments) {
  return path.join(__dirname, "..", "..", "..", "..", ...segments);
}

function loadSource(fileName) {
  return {
    content: fs.readFileSync(contractPath(fileName), "utf8"),
  };
}

function loadRepoSource(...segments) {
  return {
    content: fs.readFileSync(repoPath(...segments), "utf8"),
  };
}

function compileContracts() {
  const input = {
    language: "Solidity",
    sources: {
      "ISccpMessageVerifier.sol": loadSource("ISccpMessageVerifier.sol"),
      "Ownable.sol": loadSource("Ownable.sol"),
      "SccpMessageBridge.sol": loadSource("SccpMessageBridge.sol"),
      "SccpGroth16Bn254MessageVerifier.sol": loadSource(
        "SccpGroth16Bn254MessageVerifier.sol"
      ),
      "SccpSecp256k1MessageVerifier.sol": loadSource(
        "SccpSecp256k1MessageVerifier.sol"
      ),
      "contracts/evm/sccp/ISccpMessageVerifier.sol": loadRepoSource(
        "contracts",
        "evm",
        "sccp",
        "ISccpMessageVerifier.sol"
      ),
      "contracts/evm/sccp/Ownable.sol": loadRepoSource(
        "contracts",
        "evm",
        "sccp",
        "Ownable.sol"
      ),
      "contracts/evm/sccp/SccpGroth16Bn254MessageVerifier.sol": loadRepoSource(
        "contracts",
        "evm",
        "sccp",
        "SccpGroth16Bn254MessageVerifier.sol"
      ),
      "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol":
        loadRepoSource(
          "contracts",
          "tron",
          "sccp",
          "SccpTronGroth16Bn254MessageVerifier.sol"
        ),
      "contracts/tron/sccp/SccpTronSourceBridge.sol": loadRepoSource(
        "contracts",
        "tron",
        "sccp",
        "SccpTronSourceBridge.sol"
      ),
      "contracts/tron/sccp/TairaXOR.sol": loadRepoSource(
        "contracts",
        "tron",
        "sccp",
        "TairaXOR.sol"
      ),
      "contracts/tron/sccp/TairaXorSccpBridge.sol": loadRepoSource(
        "contracts",
        "tron",
        "sccp",
        "TairaXorSccpBridge.sol"
      ),
    },
    settings: {
      optimizer: { enabled: true, runs: 200 },
      outputSelection: {
        "*": {
          "*": ["abi", "evm.bytecode.object"],
        },
      },
    },
  };

  const output = JSON.parse(solc.compile(JSON.stringify(input)));
  if (output.errors) {
    const fatal = output.errors.filter((entry) => entry.severity === "error");
    if (fatal.length > 0) {
      throw new Error(fatal.map((entry) => entry.formattedMessage).join("\n"));
    }
  }
  return output.contracts;
}

function artifact(contracts, fileName, contractName) {
  const contract = contracts[fileName][contractName];
  return {
    abi: contract.abi,
    bytecode: `0x${contract.evm.bytecode.object}`,
  };
}

async function deploy(signer, abi, bytecode, args = []) {
  const factory = new ethers.ContractFactory(abi, bytecode, signer);
  const contract = await factory.deploy(...args);
  await contract.waitForDeployment();
  return contract;
}

async function contractCodeHash(provider, address) {
  return ethers.keccak256(await provider.getCode(address));
}

function computeDestinationBindingHash(
  abi,
  {
    verifierBackendHash,
    proofFamilyHash,
    networkId,
    sourceDomain,
    targetDomain,
    verifierAddress,
    wrapperAddress,
    verifierCodeHash,
    verifierKeyHash,
  }
) {
  return ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "uint256",
        "uint256",
        "address",
        "address",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:evm-destination-binding:v1")
        ),
        verifierBackendHash,
        proofFamilyHash,
        networkId,
        sourceDomain,
        targetDomain,
        verifierAddress,
        wrapperAddress,
        verifierCodeHash,
        verifierKeyHash,
      ]
    )
  );
}

function computeTronSourceBridgeConfigHash(
  abi,
  { bridgeAddress, networkId, sourceDomain, targetDomain, owner }
) {
  return ethers.keccak256(
    abi.encode(
      ["bytes32", "address", "bytes32", "uint32", "uint32", "address"],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:tron-source-bridge-config:v1")
        ),
        bridgeAddress,
        networkId,
        sourceDomain,
        targetDomain,
        owner,
      ]
    )
  );
}

function computeTronDestinationBindingHash(
  abi,
  {
    verifierBackendHash,
    proofFamilyHash,
    networkId,
    sourceDomain,
    targetDomain,
    verifierAddress,
    verifierCodeHash,
    verifierKeyHash,
  }
) {
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
      ],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:tron-destination-binding:v1")
        ),
        verifierBackendHash,
        proofFamilyHash,
        networkId,
        sourceDomain,
        targetDomain,
        tronAddressWord(verifierAddress),
        verifierCodeHash,
        verifierKeyHash,
      ]
    )
  );
}

function tronAddressWord(address) {
  return ethers.zeroPadValue(`0x41${ethers.getAddress(address).slice(2)}`, 32);
}

function computeTairaXorTransferPayloadHash(
  abi,
  { routeIdHash, assetKeyHash, bridgeAddress, recipient, amount }
) {
  return ethers.keccak256(
    abi.encode(
      ["bytes32", "bytes32", "bytes32", "address", "address", "uint256"],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:taira-xor:transfer-payload:v1")
        ),
        routeIdHash,
        assetKeyHash,
        bridgeAddress,
        recipient,
        amount,
      ]
    )
  );
}

function computeTairaXorBurnSourceEventDigest(
  abi,
  { routeIdHash, assetKeyHash, bridgeAddress, burner, tairaRecipientHash, amount, nonce }
) {
  return ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "bytes32",
        "bytes32",
        "address",
        "address",
        "bytes32",
        "uint256",
        "uint256",
      ],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:taira-xor:burn-source-event:v1")
        ),
        routeIdHash,
        assetKeyHash,
        bridgeAddress,
        burner,
        tairaRecipientHash,
        amount,
        nonce,
      ]
    )
  );
}

const BN254_BASE_FIELD_MODULUS =
  21888242871839275222246405745257275088696311157297823662689037894645226208583n;
const BN254_SCALAR_FIELD_MODULUS =
  21888242871839275222246405745257275088548364400416034343698204186575808495617n;
const G1_SCALAR_MUL_PRECOMPILE =
  "0x0000000000000000000000000000000000000007";
const GROTH16_SIGNAL_LABELS = [
  "sccp:groth16-bn254:signal:message-id:v1",
  "sccp:groth16-bn254:signal:payload-hash:v1",
  "sccp:groth16-bn254:signal:target-domain:v1",
  "sccp:groth16-bn254:signal:commitment-root:v1",
  "sccp:groth16-bn254:signal:finality-height:v1",
  "sccp:groth16-bn254:signal:finality-block-hash:v1",
  "sccp:groth16-bn254:signal:source-domain:v1",
  "sccp:groth16-bn254:signal:statement-hash:v1",
  "sccp:groth16-bn254:signal:destination-binding-hash:v1",
].map((label) => ethers.keccak256(ethers.toUtf8Bytes(label)));

function abiWordU32(value) {
  return ethers.zeroPadValue(ethers.toBeHex(value), 32);
}

function groth16PublicSignalWords(
  abi,
  { publicInputs, sourceDomain, statementHash, destinationBindingHash }
) {
  const signalValues = [
    publicInputs[0],
    publicInputs[1],
    publicInputs[2],
    publicInputs[3],
    publicInputs[4],
    publicInputs[5],
    abiWordU32(sourceDomain),
    statementHash,
    destinationBindingHash,
  ];
  return signalValues.map(
    (value, index) =>
      BigInt(
        ethers.keccak256(
          abi.encode(["bytes32", "bytes32"], [GROTH16_SIGNAL_LABELS[index], value])
        )
      ) % BN254_SCALAR_FIELD_MODULUS
  );
}

async function g1ScalarMul(provider, abi, point, scalar) {
  const encoded = abi.encode(
    ["uint256", "uint256", "uint256"],
    [point[0], point[1], scalar]
  );
  const result = await provider.call({
    to: G1_SCALAR_MUL_PRECOMPILE,
    data: encoded,
  });
  const decoded = abi.decode(["uint256", "uint256"], result);
  return [decoded[0], decoded[1]];
}

function g1Negate(point) {
  if (point[0] === 0n && point[1] === 0n) {
    return [0n, 0n];
  }
  return [
    point[0],
    (BN254_BASE_FIELD_MODULUS -
      (point[1] % BN254_BASE_FIELD_MODULUS)) %
      BN254_BASE_FIELD_MODULUS,
  ];
}

async function buildAcceptingGroth16ProofBytes(
  provider,
  abi,
  {
    publicInputs,
    sourceDomain,
    statementHash,
    destinationBindingHash,
    g1,
    g2,
  }
) {
  const signals = groth16PublicSignalWords(abi, {
    publicInputs,
    sourceDomain,
    statementHash,
    destinationBindingHash,
  });
  const vkScalar = signals.reduce(
    (sum, signal) => (sum + signal) % BN254_SCALAR_FIELD_MODULUS,
    1n
  );
  const vkX = await g1ScalarMul(provider, abi, g1, vkScalar);
  assert(
    vkX[0] !== 0n || vkX[1] !== 0n,
    "test verifying-key accumulator must be non-zero"
  );
  // The test key uses generator points for alpha/beta/gamma/delta and every IC,
  // so c = -vkX makes e(-a,b) * e(alpha,beta) * e(vkX,gamma) * e(c,delta) = 1.
  const c = g1Negate(vkX);
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
    [1, publicInputs[0], sourceDomain, publicInputs[3], g1, g2, c]
  );
}

function callException(error) {
  return error && error.code === "CALL_EXCEPTION";
}

function errorMessage(error) {
  return [
    error && error.reason,
    error && error.shortMessage,
    error && error.message,
    error && error.info && error.info.error && error.info.error.message,
    error && error.data && error.data.message,
  ]
    .filter(Boolean)
    .join("\n");
}

function callExceptionWithReason(reason) {
  return (error) => callException(error) && errorMessage(error).includes(reason);
}

async function main() {
  const contracts = compileContracts();
  const provider = new ethers.BrowserProvider(
    ganache.provider({ logging: { quiet: true } })
  );
  const signer = await provider.getSigner();
  const outsider = await provider.getSigner(1);
  const abi = ethers.AbiCoder.defaultAbiCoder();
  const attestor = ethers.Wallet.createRandom();

  const verifierArtifact = artifact(
    contracts,
    "SccpSecp256k1MessageVerifier.sol",
    "SccpSecp256k1MessageVerifier"
  );
  const bridgeArtifact = artifact(
    contracts,
    "SccpMessageBridge.sol",
    "SccpMessageBridge"
  );
  const bridgeIface = new ethers.Interface(bridgeArtifact.abi);
  const bridgeAcceptedEvent = bridgeArtifact.abi.find(
    (entry) => entry.type === "event" && entry.name === "MessageProofAccepted"
  );
  assert.deepEqual(
    bridgeAcceptedEvent.inputs.map((input) => input.name),
    [
      "messageId",
      "sourceDomain",
      "commitmentRoot",
      "statementHash",
      "destinationBindingHash",
      "verifierBackendHash",
      "proofFamilyHash",
      "networkId",
    ]
  );
  const groth16VerifierArtifact = artifact(
    contracts,
    "SccpGroth16Bn254MessageVerifier.sol",
    "SccpGroth16Bn254MessageVerifier"
  );
  const tronGroth16VerifierArtifact = artifact(
    contracts,
    "contracts/tron/sccp/SccpTronGroth16Bn254MessageVerifier.sol",
    "SccpTronGroth16Bn254MessageVerifier"
  );
  const tronSourceBridgeArtifact = artifact(
    contracts,
    "contracts/tron/sccp/SccpTronSourceBridge.sol",
    "SccpTronSourceBridge"
  );
  const tairaXorArtifact = artifact(
    contracts,
    "contracts/tron/sccp/TairaXOR.sol",
    "TairaXOR"
  );
  const tairaXorBridgeArtifact = artifact(
    contracts,
    "contracts/tron/sccp/TairaXorSccpBridge.sol",
    "TairaXorSccpBridge"
  );
  const tronAcceptedEvent = tronGroth16VerifierArtifact.abi.find(
    (entry) => entry.type === "event" && entry.name === "MessageProofAccepted"
  );
  assert.deepEqual(
    tronAcceptedEvent.inputs.map((input) => input.name),
    [
      "messageId",
      "sourceDomain",
      "commitmentRoot",
      "statementHash",
      "destinationBindingHash",
      "verifierBackendHash",
      "proofFamilyHash",
      "networkId",
    ]
  );

  const verifier = await deploy(
    signer,
    verifierArtifact.abi,
    verifierArtifact.bytecode,
    [[attestor.address], 1]
  );
  const verifierAddress = await verifier.getAddress();
  const verifierCodeHash = await contractCodeHash(provider, verifierAddress);
  const bridgeConstructorArgs = ({
    bridgeVerifierAddress = verifierAddress,
    bridgeVerifierCodeHash = verifierCodeHash,
    bridgeVerifierKeyHash = ethers.ZeroHash,
    verifierBackendKey = "evm-secp256k1-keccak-v1",
    proofFamily = "stark-fri-v1",
    networkId = ethers.encodeBytes32String("evm-devnet"),
    sourceDomain = 0,
    targetDomain = 1,
  } = {}) => [
    bridgeVerifierAddress,
    bridgeVerifierCodeHash,
    bridgeVerifierKeyHash,
    verifierBackendKey,
    proofFamily,
    networkId,
    sourceDomain,
    targetDomain,
  ];

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        [
          verifierAddress,
          verifierCodeHash,
          ethers.ZeroHash,
          "evm-secp256k1-keccak-v1",
          "stark-fri-v1",
          ethers.encodeBytes32String("evm-devnet"),
          0,
          1,
        ]
      );
    },
    callExceptionWithReason("Unsupported verifier backend")
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ sourceDomain: 99, targetDomain: 1 })
      );
    },
    callExceptionWithReason("Source domain must be SORA")
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ sourceDomain: 0, targetDomain: 99 })
      );
    },
    callExceptionWithReason("Target domain must be ETH or BSC")
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ verifierBackendKey: "evm-debug-verifier-v1" })
      );
    },
    callExceptionWithReason("Unsupported verifier backend")
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ verifierBackendKey: "" })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({
          verifierBackendKey: "evm-groth16-bn254-v1",
          proofFamily: "debug-proof-family",
        })
      );
    },
    callExceptionWithReason("Proof family must be stark-fri-v1")
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ proofFamily: "" })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ networkId: ethers.ZeroHash })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ sourceDomain: 1, targetDomain: 0 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        bridgeConstructorArgs({ sourceDomain: 1, targetDomain: 1 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
          bridgeConstructorArgs({
            bridgeVerifierKeyHash: ethers.keccak256(
              ethers.toUtf8Bytes("reference-verifier-key")
            ),
          })
        );
      },
    callExceptionWithReason("Unsupported verifier backend")
  );

  const messageId = ethers.keccak256(ethers.toUtf8Bytes("message-1"));
  const statementHash = ethers.keccak256(ethers.toUtf8Bytes("statement-1"));
  const nativeProofBytes = ethers.toUtf8Bytes("native-fastpq-proof-bytes");
  const publicInputs = [
    messageId,
    ethers.keccak256(ethers.toUtf8Bytes("payload-hash")),
    ethers.zeroPadValue(ethers.toBeHex(1), 32),
    ethers.keccak256(ethers.toUtf8Bytes("commitment-root")),
    ethers.zeroPadValue(ethers.toBeHex(44), 32),
    ethers.keccak256(ethers.toUtf8Bytes("finality-block")),
  ];
  const publicInputsHash = ethers.keccak256(
    abi.encode(
      ["bytes32", "bytes32", "bytes32", "bytes32", "bytes32", "bytes32"],
      publicInputs
    )
  );
  const nativeProofHash = ethers.keccak256(nativeProofBytes);
  const verifierBackendHash = ethers.keccak256(
    ethers.toUtf8Bytes("evm-secp256k1-keccak-v1")
  );
  const proofFamilyHash = ethers.keccak256(ethers.toUtf8Bytes("stark-fri-v1"));
  const networkId = ethers.encodeBytes32String("evm-devnet");
  const referenceWrapperAddress = await signer.getAddress();
  const destinationBindingHash = computeDestinationBindingHash(abi, {
    verifierBackendHash,
    proofFamilyHash,
    networkId,
    sourceDomain: 0,
    targetDomain: 1,
    verifierAddress,
    wrapperAddress: referenceWrapperAddress,
    verifierCodeHash,
    verifierKeyHash: ethers.ZeroHash,
  });
  const attestationDigest = ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "bytes32",
        "uint256",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(ethers.toUtf8Bytes("iroha:sccp:evm-attestation:v1")),
        messageId,
        0,
        publicInputs[3],
        publicInputsHash,
        statementHash,
        nativeProofHash,
        destinationBindingHash,
      ]
    )
  );
  const attestationSignature = attestor.signingKey.sign(attestationDigest).serialized;
  const proofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "bytes"],
    [
      1,
      messageId,
      0,
      publicInputs[3],
      nativeProofHash,
      destinationBindingHash,
      attestationSignature,
    ]
  );

  const directReferenceResult = await verifier.verifySccpMessageProof.staticCall(
    proofBytes,
    publicInputs,
    statementHash,
    destinationBindingHash
  );
  assert.equal(directReferenceResult[0], messageId);
  assert.equal(directReferenceResult[1], 0n);
  assert.equal(directReferenceResult[2], publicInputs[3]);

  const attestationAbiHeadLength = 7 * 32;
  const noncanonicalOffsetProof = ethers.getBytes(proofBytes);
  noncanonicalOffsetProof.set(
    ethers.getBytes(
      ethers.zeroPadValue(ethers.toBeHex(attestationAbiHeadLength + 32), 32)
    ),
    6 * 32
  );
  const noncanonicalOffsetProofBytes = ethers.concat([
    noncanonicalOffsetProof.slice(0, attestationAbiHeadLength),
    ethers.ZeroHash,
    noncanonicalOffsetProof.slice(attestationAbiHeadLength),
  ]);
  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        noncanonicalOffsetProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("Invalid signatures offset")
  );

  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        ethers.concat([proofBytes, "0x00"]),
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const zeroNativeProofDigest = ethers.keccak256(
    abi.encode(
      [
        "bytes32",
        "bytes32",
        "uint256",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
        "bytes32",
      ],
      [
        ethers.keccak256(ethers.toUtf8Bytes("iroha:sccp:evm-attestation:v1")),
        messageId,
        0,
        publicInputs[3],
        publicInputsHash,
        statementHash,
        ethers.ZeroHash,
        destinationBindingHash,
      ]
    )
  );
  const zeroNativeProofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "bytes"],
    [
      1,
      messageId,
      0,
      publicInputs[3],
      ethers.ZeroHash,
      destinationBindingHash,
      attestor.signingKey.sign(zeroNativeProofDigest).serialized,
    ]
  );
  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        zeroNativeProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("Native proof hash is required")
  );

  const mismatchedReferenceInputs = publicInputs.slice();
  mismatchedReferenceInputs[0] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-reference-message")
  );
  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        proofBytes,
        mismatchedReferenceInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("Public input message id mismatch")
  );

  const zeroReferenceTargetInputs = publicInputs.slice();
  zeroReferenceTargetInputs[2] = ethers.ZeroHash;
  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        proofBytes,
        zeroReferenceTargetInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("Target domain is required")
  );

  const mismatchedReferenceCommitmentInputs = publicInputs.slice();
  mismatchedReferenceCommitmentInputs[3] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-reference-commitment")
  );
  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        proofBytes,
        mismatchedReferenceCommitmentInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("Public input commitment root mismatch")
  );

  const tamperedProofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "bytes"],
    [
      1,
      ethers.keccak256(ethers.toUtf8Bytes("message-2")),
      0,
      publicInputs[3],
      nativeProofHash,
      destinationBindingHash,
      attestationSignature,
    ]
  );

  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        tamperedProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        proofBytes,
        publicInputs,
        ethers.keccak256(ethers.toUtf8Bytes("wrong-statement")),
        destinationBindingHash
      ),
    callException
  );

  const zeroPayloadInputs = publicInputs.slice();
  zeroPayloadInputs[1] = ethers.ZeroHash;

  const zeroFinalityHeightInputs = publicInputs.slice();
  zeroFinalityHeightInputs[4] = ethers.ZeroHash;

  const unauthorized = ethers.Wallet.createRandom();
  const unauthorizedSignature = unauthorized.signingKey.sign(attestationDigest).serialized;
  const unauthorizedProofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "bytes"],
    [
      1,
      messageId,
      0,
      publicInputs[3],
      nativeProofHash,
      destinationBindingHash,
      unauthorizedSignature,
    ]
  );

  await assert.rejects(
    () =>
      verifier.verifySccpMessageProof.staticCall(
        unauthorizedProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const g1 = [1n, 2n];
  const zeroG1 = [0n, 0n];
  const zeroG2 = [0n, 0n, 0n, 0n];
  const g2 = [
    10857046999023057135944570762232829481370756359578518086990519993285655852781n,
    11559732032986387107991004021392285783925812861821192530917403151452391805634n,
    8495653923123431417604973247489272438418190587263600148770280649306958101930n,
    4082367875863433681332203403145435568316851327593401208105741076214120093531n,
  ];
  const vkIc = Array.from({ length: 10 }, () => g1).flat();

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        groth16VerifierArtifact.abi,
        groth16VerifierArtifact.bytecode,
        [zeroG1, g2, g2, g2, vkIc]
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        groth16VerifierArtifact.abi,
        groth16VerifierArtifact.bytecode,
        [[1n, 1n], g2, g2, g2, vkIc]
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        groth16VerifierArtifact.abi,
        groth16VerifierArtifact.bytecode,
        [g1, zeroG2, g2, g2, vkIc]
      );
    },
    callException
  );

  const invalidG2 = g2.slice();
  invalidG2[0] = invalidG2[0] + 1n;
  const nonSubgroupG2 = [
    0n,
    1n,
    0x0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8n,
    0x07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2n,
  ];
  await assert.rejects(
    async () => {
      await deploy(
        signer,
        groth16VerifierArtifact.abi,
        groth16VerifierArtifact.bytecode,
        [g1, invalidG2, g2, g2, vkIc]
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        groth16VerifierArtifact.abi,
        groth16VerifierArtifact.bytecode,
        [g1, g2, g2, g2, vkIc.slice(0, 9)]
      );
    },
    callException
  );

  const groth16Verifier = await deploy(
    signer,
    groth16VerifierArtifact.abi,
    groth16VerifierArtifact.bytecode,
    [g1, g2, g2, g2, vkIc]
  );
  assert.equal(await groth16Verifier.publicInputCount(), 9n);
  const groth16VerifierAddress = await groth16Verifier.getAddress();
  const groth16VerifierCodeHash = await contractCodeHash(
    provider,
    groth16VerifierAddress
  );
  const groth16VerifierKeyHash = await groth16Verifier.verifyingKeyHash();

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        [
          groth16VerifierAddress,
          ethers.keccak256(ethers.toUtf8Bytes("wrong-verifier-code")),
          groth16VerifierKeyHash,
          "evm-groth16-bn254-v1",
          "stark-fri-v1",
          networkId,
          0,
          1,
        ]
      );
    },
    callExceptionWithReason("Verifier code hash mismatch")
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        [
          groth16VerifierAddress,
          groth16VerifierCodeHash,
          ethers.ZeroHash,
          "evm-groth16-bn254-v1",
          "stark-fri-v1",
          networkId,
          0,
          1,
        ]
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        bridgeArtifact.abi,
        bridgeArtifact.bytecode,
        [
          groth16VerifierAddress,
          groth16VerifierCodeHash,
          ethers.keccak256(ethers.toUtf8Bytes("wrong-verifier-key")),
          "evm-groth16-bn254-v1",
          "stark-fri-v1",
          networkId,
          0,
          1,
        ]
      );
    },
    callException
  );

  const groth16Bridge = await deploy(
    signer,
    bridgeArtifact.abi,
    bridgeArtifact.bytecode,
    [
      groth16VerifierAddress,
      groth16VerifierCodeHash,
      groth16VerifierKeyHash,
      "evm-groth16-bn254-v1",
      "stark-fri-v1",
      networkId,
      0,
      1,
    ]
  );
  assert.equal(await groth16Bridge.verifierCodeHash(), groth16VerifierCodeHash);
  assert.equal(await groth16Bridge.verifierKeyHash(), groth16VerifierKeyHash);
  const invalidGroth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, g2, g1]
  );

  await assert.rejects(
    async () => {
      const zeroStatementTx = await groth16Bridge.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        publicInputs,
        ethers.ZeroHash
      );
      await zeroStatementTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const zeroPayloadTx = await groth16Bridge.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        zeroPayloadInputs,
        statementHash
      );
      await zeroPayloadTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const zeroFinalityHeightTx = await groth16Bridge.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        zeroFinalityHeightInputs,
        statementHash
      );
      await zeroFinalityHeightTx.wait();
    },
    callException
  );

  const wrongTargetBridgeInputs = publicInputs.slice();
  wrongTargetBridgeInputs[2] = ethers.zeroPadValue(ethers.toBeHex(2), 32);
  await assert.rejects(
    async () => {
      const wrongTargetTx = await groth16Bridge.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        wrongTargetBridgeInputs,
        statementHash
      );
      await wrongTargetTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const badGrothTx = await groth16Bridge.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        publicInputs,
        statementHash
      );
      await badGrothTx.wait();
    },
    callException
  );

  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        invalidGroth16ProofBytes,
        publicInputs,
        ethers.ZeroHash,
        destinationBindingHash
      ),
    callException
  );

  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        invalidGroth16ProofBytes,
        publicInputs,
        statementHash,
        ethers.ZeroHash
      ),
    callException
  );

  const zeroTargetGrothInputs = publicInputs.slice();
  zeroTargetGrothInputs[2] = ethers.ZeroHash;
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        invalidGroth16ProofBytes,
        zeroTargetGrothInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        invalidGroth16ProofBytes,
        zeroPayloadInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const malformedGroth16ProofBytes = "0x1234";
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        malformedGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const trailingGroth16ProofBytes = ethers.concat([
    invalidGroth16ProofBytes,
    "0x00",
  ]);
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        trailingGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("Unexpected Groth16 proof length")
  );

  const wrongVersionGroth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [2, messageId, 0, publicInputs[3], g1, g2, g1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        wrongVersionGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const overflowSourceDomainGroth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 4294967296n, publicInputs[3], g1, g2, g1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        overflowSourceDomainGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const zeroPointGroth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], zeroG1, g2, g1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        zeroPointGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const zeroG2Groth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, zeroG2, g1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        zeroG2Groth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("G2 point is zero")
  );

  const zeroCGroth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, g2, zeroG1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        zeroCGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callExceptionWithReason("G1 point is zero")
  );

  const invalidG2Groth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, invalidG2, g1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        invalidG2Groth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const nonSubgroupG2Groth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, nonSubgroupG2, g1]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        nonSubgroupG2Groth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const wrongCommitmentGroth16ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [
      1,
      messageId,
      0,
      ethers.keccak256(ethers.toUtf8Bytes("wrong-commitment-root")),
      g1,
      g2,
      g1,
    ]
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        wrongCommitmentGroth16ProofBytes,
        publicInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const mismatchedGrothInputs = publicInputs.slice();
  mismatchedGrothInputs[0] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-groth-message")
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        invalidGroth16ProofBytes,
        mismatchedGrothInputs,
        statementHash,
        destinationBindingHash
      ),
    callException
  );

  const groth16BridgeAddress = await groth16Bridge.getAddress();
  const groth16DestinationBindingHash = computeDestinationBindingHash(abi, {
    verifierBackendHash: ethers.keccak256(
      ethers.toUtf8Bytes("evm-groth16-bn254-v1")
    ),
    proofFamilyHash: ethers.keccak256(ethers.toUtf8Bytes("stark-fri-v1")),
    networkId,
    sourceDomain: 0,
    targetDomain: 1,
    verifierAddress: groth16VerifierAddress,
    wrapperAddress: groth16BridgeAddress,
    verifierCodeHash: groth16VerifierCodeHash,
    verifierKeyHash: groth16VerifierKeyHash,
  });
  const acceptingGroth16ProofBytes = await buildAcceptingGroth16ProofBytes(
    provider,
    abi,
    {
      publicInputs,
      sourceDomain: 0,
      statementHash,
      destinationBindingHash: groth16DestinationBindingHash,
      g1,
      g2,
    }
  );
  const acceptedGroth16Result =
    await groth16Verifier.verifySccpMessageProof.staticCall(
      acceptingGroth16ProofBytes,
      publicInputs,
      statementHash,
      groth16DestinationBindingHash
    );
  assert.equal(acceptedGroth16Result[0], messageId);
  assert.equal(acceptedGroth16Result[1], 0n);
  assert.equal(acceptedGroth16Result[2], publicInputs[3]);
  const mismatchedPayloadGroth16Inputs = publicInputs.slice();
  mismatchedPayloadGroth16Inputs[1] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-accepted-groth16-payload")
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        acceptingGroth16ProofBytes,
        mismatchedPayloadGroth16Inputs,
        statementHash,
        groth16DestinationBindingHash
    ),
    callExceptionWithReason("Groth16 proof verification failed")
  );
  const mismatchedFinalityHeightGroth16Inputs = publicInputs.slice();
  mismatchedFinalityHeightGroth16Inputs[4] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-accepted-groth16-finality-height")
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        acceptingGroth16ProofBytes,
        mismatchedFinalityHeightGroth16Inputs,
        statementHash,
        groth16DestinationBindingHash
      ),
    callExceptionWithReason("Groth16 proof verification failed")
  );
  const mismatchedFinalityGroth16Inputs = publicInputs.slice();
  mismatchedFinalityGroth16Inputs[5] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-accepted-groth16-finality")
  );
  await assert.rejects(
    () =>
      groth16Verifier.verifySccpMessageProof.staticCall(
        acceptingGroth16ProofBytes,
        mismatchedFinalityGroth16Inputs,
        statementHash,
        groth16DestinationBindingHash
      ),
    callExceptionWithReason("Groth16 proof verification failed")
  );
  const acceptedGroth16Tx = await groth16Bridge.submitSccpMessageProof(
    acceptingGroth16ProofBytes,
    publicInputs,
    statementHash
  );
  const acceptedGroth16Receipt = await acceptedGroth16Tx.wait();
  assert.equal(await groth16Bridge.usedMessageProofs(messageId), true);
  assert.equal(
    await groth16Bridge.destinationBindingHash(),
    groth16DestinationBindingHash
  );
  const acceptedGroth16Logs = acceptedGroth16Receipt.logs
    .filter(
      (log) => log.address.toLowerCase() === groth16BridgeAddress.toLowerCase()
    )
    .map((log) => {
      try {
        return bridgeIface.parseLog(log);
      } catch (_error) {
        return null;
      }
    })
    .filter((log) => log && log.name === "MessageProofAccepted");
  assert.equal(acceptedGroth16Logs.length, 1);
  assert.equal(acceptedGroth16Logs[0].args.messageId, messageId);
  assert.equal(acceptedGroth16Logs[0].args.sourceDomain, 0n);
  assert.equal(acceptedGroth16Logs[0].args.commitmentRoot, publicInputs[3]);
  assert.equal(acceptedGroth16Logs[0].args.statementHash, statementHash);
  assert.equal(
    acceptedGroth16Logs[0].args.destinationBindingHash,
    groth16DestinationBindingHash
  );
  assert.equal(
    acceptedGroth16Logs[0].args.verifierBackendHash,
    ethers.keccak256(ethers.toUtf8Bytes("evm-groth16-bn254-v1"))
  );
  assert.equal(
    acceptedGroth16Logs[0].args.proofFamilyHash,
    ethers.keccak256(ethers.toUtf8Bytes("stark-fri-v1"))
  );
  assert.equal(acceptedGroth16Logs[0].args.networkId, networkId);

  await assert.rejects(
    async () => {
      const replayGroth16Tx = await groth16Bridge.submitSccpMessageProof(
        acceptingGroth16ProofBytes,
        publicInputs,
        statementHash
      );
      await replayGroth16Tx.wait();
    },
    callException
  );

  const tronNetworkId = ethers.encodeBytes32String("tron-mainnet");
  const tronConstructorArgs = ({
    expectedVerifierKeyHash = groth16VerifierKeyHash,
    proofFamily = "stark-fri-v1",
    configuredNetworkId = tronNetworkId,
    configuredSourceDomain = 0,
    configuredTargetDomain = 5,
  } = {}) => [
    g1,
    g2,
    g2,
    g2,
    vkIc,
    expectedVerifierKeyHash,
    proofFamily,
    configuredNetworkId,
    configuredSourceDomain,
    configuredTargetDomain,
  ];

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ configuredTargetDomain: 4 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ configuredSourceDomain: 1 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ proofFamily: "debug-proof-family" })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ expectedVerifierKeyHash: ethers.ZeroHash })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({
          expectedVerifierKeyHash: ethers.keccak256(
            ethers.toUtf8Bytes("wrong-tron-key")
          ),
        })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ proofFamily: "" })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ configuredNetworkId: ethers.ZeroHash })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ configuredTargetDomain: 0 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({ configuredSourceDomain: 99 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronGroth16VerifierArtifact.abi,
        tronGroth16VerifierArtifact.bytecode,
        tronConstructorArgs({
          configuredSourceDomain: 5,
          configuredTargetDomain: 5,
        })
      );
    },
    callException
  );

  const tronSourceBridgeConstructorArgs = ({
    configuredNetworkId = tronNetworkId,
    configuredSourceDomain = 5,
    configuredTargetDomain = 0,
  } = {}) => [
    configuredNetworkId,
    configuredSourceDomain,
    configuredTargetDomain,
  ];
  const tronSourceDomain = 5;
  const tronTargetDomain = 0;

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronSourceBridgeArtifact.abi,
        tronSourceBridgeArtifact.bytecode,
        tronSourceBridgeConstructorArgs({ configuredNetworkId: ethers.ZeroHash })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronSourceBridgeArtifact.abi,
        tronSourceBridgeArtifact.bytecode,
        tronSourceBridgeConstructorArgs({ configuredTargetDomain: 1 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronSourceBridgeArtifact.abi,
        tronSourceBridgeArtifact.bytecode,
        tronSourceBridgeConstructorArgs({ configuredTargetDomain: 99 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronSourceBridgeArtifact.abi,
        tronSourceBridgeArtifact.bytecode,
        tronSourceBridgeConstructorArgs({ configuredSourceDomain: 4 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronSourceBridgeArtifact.abi,
        tronSourceBridgeArtifact.bytecode,
        tronSourceBridgeConstructorArgs({ configuredSourceDomain: 0 })
      );
    },
    callException
  );

  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tronSourceBridgeArtifact.abi,
        tronSourceBridgeArtifact.bytecode,
        tronSourceBridgeConstructorArgs({
          configuredSourceDomain: 5,
          configuredTargetDomain: 5,
        })
      );
    },
    callException
  );

  const tronSourceBridge = await deploy(
    signer,
    tronSourceBridgeArtifact.abi,
    tronSourceBridgeArtifact.bytecode,
    tronSourceBridgeConstructorArgs()
  );
  const sourceBridgeAddress = (await tronSourceBridge.getAddress()).toLowerCase();
  const expectedSourceBridgeConfigHash = computeTronSourceBridgeConfigHash(abi, {
    bridgeAddress: await tronSourceBridge.getAddress(),
    networkId: tronNetworkId,
    sourceDomain: 5,
    targetDomain: 0,
    owner: await signer.getAddress(),
  });
  assert.equal(await tronSourceBridge.owner(), await signer.getAddress());
  assert.equal(await tronSourceBridge.networkId(), tronNetworkId);
  assert.equal(await tronSourceBridge.sourceDomain(), 5n);
  assert.equal(await tronSourceBridge.targetDomain(), 0n);
  assert.equal(
    await tronSourceBridge.sourceBridgeConfigHash(),
    expectedSourceBridgeConfigHash
  );
  const sourceBridgeIface = new ethers.Interface(tronSourceBridgeArtifact.abi);
  const sourceBridgeDeploymentReceipt = await tronSourceBridge
    .deploymentTransaction()
    .wait();
  const configuredLogs = sourceBridgeDeploymentReceipt.logs
    .filter((log) => log.address.toLowerCase() === sourceBridgeAddress)
    .map((log) => {
      try {
        return sourceBridgeIface.parseLog(log);
      } catch (_error) {
        return null;
      }
    })
    .filter((log) => log && log.name === "SourceBridgeConfigured");
  assert.equal(configuredLogs.length, 1);
  assert.equal(configuredLogs[0].args.bridge.toLowerCase(), sourceBridgeAddress);
  assert.equal(configuredLogs[0].args.networkId, tronNetworkId);
  assert.equal(configuredLogs[0].args.sourceDomain, 5n);
  assert.equal(configuredLogs[0].args.targetDomain, 0n);
  assert.equal(configuredLogs[0].args.ownerAddress, await signer.getAddress());
  assert.equal(configuredLogs[0].args.configHash, expectedSourceBridgeConfigHash);

  assert.equal(
    await tronSourceBridge.emitSourceBridgeConfigHash.staticCall(),
    expectedSourceBridgeConfigHash
  );
  const configHashTx = await tronSourceBridge.emitSourceBridgeConfigHash();
  const configHashReceipt = await configHashTx.wait();
  const configHashLogs = configHashReceipt.logs
    .filter((log) => log.address.toLowerCase() === sourceBridgeAddress)
    .map((log) => sourceBridgeIface.parseLog(log))
    .filter((log) => log.name === "SourceBridgeConfigHash");
  assert.equal(configHashLogs.length, 1);
  assert.equal(configHashLogs[0].args.configHash, expectedSourceBridgeConfigHash);
  assert.equal(configHashLogs[0].args.ownerAddress, await signer.getAddress());
  await assert.rejects(
    async () => {
      const emitConfigHashTx = await tronSourceBridge
        .connect(outsider)
        .emitSourceBridgeConfigHash();
      await emitConfigHashTx.wait();
    },
    callException
  );

  const transferSourceBridge = await deploy(
    signer,
    tronSourceBridgeArtifact.abi,
    tronSourceBridgeArtifact.bytecode,
    tronSourceBridgeConstructorArgs()
  );
  const transferSourceBridgeAddress = await transferSourceBridge.getAddress();
  const preTransferConfigHash = computeTronSourceBridgeConfigHash(abi, {
    bridgeAddress: transferSourceBridgeAddress,
    networkId: tronNetworkId,
    sourceDomain: 5,
    targetDomain: 0,
    owner: await signer.getAddress(),
  });
  assert.equal(
    await transferSourceBridge.sourceBridgeConfigHash(),
    preTransferConfigHash
  );
  const transferOwnershipTx = await transferSourceBridge.transferOwnership(
    await outsider.getAddress()
  );
  const transferOwnershipReceipt = await transferOwnershipTx.wait();
  const postTransferConfigHash = computeTronSourceBridgeConfigHash(abi, {
    bridgeAddress: transferSourceBridgeAddress,
    networkId: tronNetworkId,
    sourceDomain: 5,
    targetDomain: 0,
    owner: await outsider.getAddress(),
  });
  const transferConfigHashLogs = transferOwnershipReceipt.logs
    .filter(
      (log) =>
        log.address.toLowerCase() === transferSourceBridgeAddress.toLowerCase()
    )
    .map((log) => sourceBridgeIface.parseLog(log))
    .filter((log) => log.name === "SourceBridgeConfigHash");
  assert.equal(transferConfigHashLogs.length, 1);
  assert.equal(transferConfigHashLogs[0].args.configHash, postTransferConfigHash);
  assert.equal(
    transferConfigHashLogs[0].args.ownerAddress,
    await outsider.getAddress()
  );
  assert.notEqual(postTransferConfigHash, preTransferConfigHash);
  assert.equal(await transferSourceBridge.owner(), await outsider.getAddress());
  assert.equal(
    await transferSourceBridge.sourceBridgeConfigHash(),
    postTransferConfigHash
  );
  await assert.rejects(
    async () => {
      const staleOwnerConfigHashTx =
        await transferSourceBridge.emitSourceBridgeConfigHash();
      await staleOwnerConfigHashTx.wait();
    },
    callException
  );
  assert.equal(
    await transferSourceBridge.connect(outsider).emitSourceBridgeConfigHash.staticCall(),
    postTransferConfigHash
  );
  const transferDigest = ethers.keccak256(
    ethers.toUtf8Bytes("tron-source-event-after-transfer")
  );
  await assert.rejects(
    async () => {
      const staleOwnerSubmitTx =
        await transferSourceBridge.submitSccpSourceEvent(
          tronSourceDomain,
          tronTargetDomain,
          transferDigest
        );
      await staleOwnerSubmitTx.wait();
    },
    callException
  );
  assert.equal(
    await transferSourceBridge
      .connect(outsider)
      .submitSccpSourceEvent.staticCall(
        tronSourceDomain,
        tronTargetDomain,
        transferDigest
      ),
    transferDigest
  );
  const transferSubmitTx = await transferSourceBridge
    .connect(outsider)
    .submitSccpSourceEvent(tronSourceDomain, tronTargetDomain, transferDigest);
  await transferSubmitTx.wait();
  assert.equal(
    await transferSourceBridge.submittedSourceEvents(transferDigest),
    true
  );

  const sourceEventDigest = ethers.keccak256(
    ethers.toUtf8Bytes("tron-source-event-digest")
  );
  assert.equal(
    await tronSourceBridge.submitSccpSourceEvent.staticCall(
      tronSourceDomain,
      tronTargetDomain,
      sourceEventDigest
    ),
    sourceEventDigest
  );

  await assert.rejects(
    async () => {
      const zeroSourceEventTx = await tronSourceBridge.submitSccpSourceEvent(
        tronSourceDomain,
        tronTargetDomain,
        ethers.ZeroHash
      );
      await zeroSourceEventTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const wrongSourceDomainTx = await tronSourceBridge.submitSccpSourceEvent(
        4,
        tronTargetDomain,
        sourceEventDigest
      );
      await wrongSourceDomainTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const wrongTargetDomainTx = await tronSourceBridge.submitSccpSourceEvent(
        tronSourceDomain,
        1,
        sourceEventDigest
      );
      await wrongTargetDomainTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const unauthorizedTx = await tronSourceBridge
        .connect(outsider)
        .submitSccpSourceEvent(
          tronSourceDomain,
          tronTargetDomain,
          sourceEventDigest
        );
      await unauthorizedTx.wait();
    },
    callException
  );

  const sourceEventTx = await tronSourceBridge.submitSccpSourceEvent(
    tronSourceDomain,
    tronTargetDomain,
    sourceEventDigest
  );
  const sourceEventReceipt = await sourceEventTx.wait();
  assert.equal(
    await tronSourceBridge.submittedSourceEvents(sourceEventDigest),
    true
  );
  const sourceEventTopic = ethers.id("SccpSourceEvent(bytes32)");
  const sourceEventLogs = sourceEventReceipt.logs.filter(
    (log) =>
      log.address.toLowerCase() === sourceBridgeAddress &&
      log.topics[0] === sourceEventTopic
  );
  assert.equal(sourceEventLogs.length, 1);
  assert.deepEqual(sourceEventLogs[0].topics, [
    sourceEventTopic,
    sourceEventDigest,
  ]);
  assert.equal(sourceEventLogs[0].data, "0x");

  await assert.rejects(
    async () => {
      const replaySourceEventTx = await tronSourceBridge.submitSccpSourceEvent(
        tronSourceDomain,
        tronTargetDomain,
        sourceEventDigest
      );
      await replaySourceEventTx.wait();
    },
    callException
  );

  const tronGroth16Verifier = await deploy(
    signer,
    tronGroth16VerifierArtifact.abi,
    tronGroth16VerifierArtifact.bytecode,
    tronConstructorArgs()
  );
  const tronGroth16Address = await tronGroth16Verifier.getAddress();
  const tronRuntimeCodeHash = await contractCodeHash(provider, tronGroth16Address);
  const expectedTronBackendHash = ethers.keccak256(
    ethers.toUtf8Bytes("tron-groth16-bn254-v1")
  );
  const expectedTronProofFamilyHash = ethers.keccak256(
    ethers.toUtf8Bytes("stark-fri-v1")
  );
  const tronGroth16Iface = new ethers.Interface(tronGroth16VerifierArtifact.abi);
  const tronGroth16DeploymentReceipt = await tronGroth16Verifier
    .deploymentTransaction()
    .wait();
  const verifierBoundLogs = tronGroth16DeploymentReceipt.logs
    .filter(
      (log) => log.address.toLowerCase() === tronGroth16Address.toLowerCase()
    )
    .map((log) => {
      try {
        return tronGroth16Iface.parseLog(log);
      } catch (_error) {
        return null;
      }
    })
    .filter((log) => log && log.name === "VerifierBound");
  assert.equal(verifierBoundLogs.length, 1);
  assert.equal(
    verifierBoundLogs[0].args.verifier.toLowerCase(),
    tronGroth16Address.toLowerCase()
  );
  assert.equal(verifierBoundLogs[0].args.verifierKeyHash, groth16VerifierKeyHash);
  assert.equal(
    verifierBoundLogs[0].args.verifierBackendHash,
    expectedTronBackendHash
  );
  assert.equal(
    verifierBoundLogs[0].args.proofFamilyHash,
    expectedTronProofFamilyHash
  );
  assert.equal(
    await tronGroth16Verifier.verifierKeyHash(),
    groth16VerifierKeyHash
  );
  assert.equal(await tronGroth16Verifier.verifierCodeHash(), tronRuntimeCodeHash);
  assert.equal(
    await tronGroth16Verifier.verifierBackendHash(),
    expectedTronBackendHash
  );
  assert.equal(
    await tronGroth16Verifier.proofFamilyHash(),
    expectedTronProofFamilyHash
  );
  assert.equal(await tronGroth16Verifier.networkId(), tronNetworkId);
  assert.equal(await tronGroth16Verifier.expectedSourceDomain(), 0n);
  assert.equal(await tronGroth16Verifier.expectedTargetDomain(), 5n);
  const expectedTronDestinationBindingHash =
    computeTronDestinationBindingHash(abi, {
      verifierBackendHash: expectedTronBackendHash,
      proofFamilyHash: expectedTronProofFamilyHash,
      networkId: tronNetworkId,
      sourceDomain: 0,
      targetDomain: 5,
      verifierAddress: tronGroth16Address,
      verifierCodeHash: tronRuntimeCodeHash,
      verifierKeyHash: groth16VerifierKeyHash,
    });
  assert.equal(
    await tronGroth16Verifier.destinationBindingHash(),
    expectedTronDestinationBindingHash
  );
  assert.equal(
    await tronGroth16Verifier.emitDestinationBindingConfigured.staticCall(),
    expectedTronDestinationBindingHash
  );
  const destinationBindingTx =
    await tronGroth16Verifier.emitDestinationBindingConfigured();
  const destinationBindingReceipt = await destinationBindingTx.wait();
  const destinationBindingLogs = destinationBindingReceipt.logs
    .filter(
      (log) => log.address.toLowerCase() === tronGroth16Address.toLowerCase()
    )
    .map((log) => {
      try {
        return tronGroth16Iface.parseLog(log);
      } catch (_error) {
        return null;
      }
    })
    .filter((log) => log && log.name === "DestinationBindingConfigured");
  assert.equal(destinationBindingLogs.length, 1);
  assert.equal(
    destinationBindingLogs[0].args.destinationBindingHash,
    expectedTronDestinationBindingHash
  );
  assert.equal(
    destinationBindingLogs[0].args.verifierCodeHash,
    tronRuntimeCodeHash
  );
  assert.equal(
    destinationBindingLogs[0].args.verifierKeyHash,
    groth16VerifierKeyHash
  );
  assert.equal(destinationBindingLogs[0].args.networkId, tronNetworkId);
  assert.equal(destinationBindingLogs[0].args.sourceDomain, 0n);
  assert.equal(destinationBindingLogs[0].args.targetDomain, 5n);

  const tronInputs = publicInputs.slice();
  tronInputs[2] = ethers.zeroPadValue(ethers.toBeHex(5), 32);

  await assert.rejects(
    async () => {
      const zeroStatementTx =
        await tronGroth16Verifier.submitSccpMessageProof(
          invalidGroth16ProofBytes,
          tronInputs,
          ethers.ZeroHash
        );
      await zeroStatementTx.wait();
    },
    callException
  );

  const zeroTronMessageInputs = tronInputs.slice();
  zeroTronMessageInputs[0] = ethers.ZeroHash;
  await assert.rejects(
    async () => {
      const zeroMessageTx = await tronGroth16Verifier.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        zeroTronMessageInputs,
        statementHash
      );
      await zeroMessageTx.wait();
    },
    callException
  );

  const zeroTronPayloadInputs = tronInputs.slice();
  zeroTronPayloadInputs[1] = ethers.ZeroHash;
  await assert.rejects(
    async () => {
      const zeroPayloadTx = await tronGroth16Verifier.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        zeroTronPayloadInputs,
        statementHash
      );
      await zeroPayloadTx.wait();
    },
    callException
  );

  const zeroTronCommitmentInputs = tronInputs.slice();
  zeroTronCommitmentInputs[3] = ethers.ZeroHash;
  const zeroTronCommitmentProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, ethers.ZeroHash, g1, g2, g1]
  );
  await assert.rejects(
    async () => {
      const zeroCommitmentTx =
        await tronGroth16Verifier.submitSccpMessageProof(
          zeroTronCommitmentProofBytes,
          zeroTronCommitmentInputs,
          statementHash
        );
      await zeroCommitmentTx.wait();
    },
    callException
  );

  const zeroTronFinalityInputs = tronInputs.slice();
  zeroTronFinalityInputs[4] = ethers.ZeroHash;
  await assert.rejects(
    async () => {
      const zeroFinalityTx = await tronGroth16Verifier.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        zeroTronFinalityInputs,
        statementHash
      );
      await zeroFinalityTx.wait();
    },
    callException
  );

  const zeroTronFinalityBlockInputs = tronInputs.slice();
  zeroTronFinalityBlockInputs[5] = ethers.ZeroHash;
  await assert.rejects(
    async () => {
      const zeroFinalityBlockTx =
        await tronGroth16Verifier.submitSccpMessageProof(
          invalidGroth16ProofBytes,
          zeroTronFinalityBlockInputs,
          statementHash
        );
      await zeroFinalityBlockTx.wait();
    },
    callException
  );

  const wrongTronTargetInputs = tronInputs.slice();
  wrongTronTargetInputs[2] = ethers.zeroPadValue(ethers.toBeHex(6), 32);
  await assert.rejects(
    async () => {
      const wrongTargetTx = await tronGroth16Verifier.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        wrongTronTargetInputs,
        statementHash
      );
      await wrongTargetTx.wait();
    },
    callException
  );

  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        "0x1234",
        tronInputs,
        statementHash
      ),
    callExceptionWithReason("Unexpected Groth16 proof length")
  );

  const trailingTronProofBytes = ethers.concat([
    invalidGroth16ProofBytes,
    "0x00",
  ]);
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        trailingTronProofBytes,
        tronInputs,
        statementHash
      ),
    callExceptionWithReason("Unexpected Groth16 proof length")
  );

  const wrongTronVersionProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [2, messageId, 0, publicInputs[3], g1, g2, g1]
  );
  await assert.rejects(
    async () => {
      const wrongVersionTx = await tronGroth16Verifier.submitSccpMessageProof(
        wrongTronVersionProofBytes,
        tronInputs,
        statementHash
      );
      await wrongVersionTx.wait();
    },
    callException
  );

  const wrongTronMessageProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [
      1,
      ethers.keccak256(ethers.toUtf8Bytes("wrong-tron-message")),
      0,
      publicInputs[3],
      g1,
      g2,
      g1,
    ]
  );
  await assert.rejects(
    async () => {
      const wrongMessageTx = await tronGroth16Verifier.submitSccpMessageProof(
        wrongTronMessageProofBytes,
        tronInputs,
        statementHash
      );
      await wrongMessageTx.wait();
    },
    callException
  );

  const wrongTronSourceProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 1, publicInputs[3], g1, g2, g1]
  );
  await assert.rejects(
    async () => {
      const wrongSourceTx = await tronGroth16Verifier.submitSccpMessageProof(
        wrongTronSourceProofBytes,
        tronInputs,
        statementHash
      );
      await wrongSourceTx.wait();
    },
    callException
  );

  const overflowTronSourceProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 4294967296n, publicInputs[3], g1, g2, g1]
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        overflowTronSourceProofBytes,
        tronInputs,
        statementHash
      ),
    callExceptionWithReason("Source domain overflow")
  );

  const wrongTronCommitmentProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [
      1,
      messageId,
      0,
      ethers.keccak256(ethers.toUtf8Bytes("wrong-tron-commitment-root")),
      g1,
      g2,
      g1,
    ]
  );
  await assert.rejects(
    async () => {
      const wrongCommitmentTx = await tronGroth16Verifier.submitSccpMessageProof(
        wrongTronCommitmentProofBytes,
        tronInputs,
        statementHash
      );
      await wrongCommitmentTx.wait();
    },
    callException
  );

  const zeroTronAProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], zeroG1, g2, g1]
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        zeroTronAProofBytes,
        tronInputs,
        statementHash
      ),
    callExceptionWithReason("G1 point is zero")
  );

  const zeroTronBProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, zeroG2, g1]
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        zeroTronBProofBytes,
        tronInputs,
        statementHash
      ),
    callExceptionWithReason("G2 point is zero")
  );

  const zeroTronCProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, g2, zeroG1]
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        zeroTronCProofBytes,
        tronInputs,
        statementHash
      ),
    callExceptionWithReason("G1 point is zero")
  );

  const invalidTronG2ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, invalidG2, g1]
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        invalidTronG2ProofBytes,
        tronInputs,
        statementHash
      ),
    callException
  );

  const nonSubgroupTronG2ProofBytes = abi.encode(
    [
      "uint256",
      "bytes32",
      "uint256",
      "bytes32",
      "uint256[2]",
      "uint256[4]",
      "uint256[2]",
    ],
    [1, messageId, 0, publicInputs[3], g1, nonSubgroupG2, g1]
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        nonSubgroupTronG2ProofBytes,
        tronInputs,
        statementHash
      ),
    callException
  );

  const acceptingTronGroth16ProofBytes = await buildAcceptingGroth16ProofBytes(
    provider,
    abi,
    {
      publicInputs: tronInputs,
      sourceDomain: 0,
      statementHash,
      destinationBindingHash: expectedTronDestinationBindingHash,
      g1,
      g2,
    }
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        acceptingTronGroth16ProofBytes,
        tronInputs,
        ethers.keccak256(ethers.toUtf8Bytes("wrong-tron-statement"))
      ),
    callException
  );
  const mismatchedTronPayloadInputs = tronInputs.slice();
  mismatchedTronPayloadInputs[1] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-accepted-tron-payload")
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        acceptingTronGroth16ProofBytes,
        mismatchedTronPayloadInputs,
        statementHash
    ),
    callExceptionWithReason("Groth16 proof verification failed")
  );
  const mismatchedTronFinalityHeightInputs = tronInputs.slice();
  mismatchedTronFinalityHeightInputs[4] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-accepted-tron-finality-height")
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        acceptingTronGroth16ProofBytes,
        mismatchedTronFinalityHeightInputs,
        statementHash
      ),
    callExceptionWithReason("Groth16 proof verification failed")
  );
  const mismatchedTronFinalityInputs = tronInputs.slice();
  mismatchedTronFinalityInputs[5] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-accepted-tron-finality")
  );
  await assert.rejects(
    () =>
      tronGroth16Verifier.submitSccpMessageProof.staticCall(
        acceptingTronGroth16ProofBytes,
        mismatchedTronFinalityInputs,
        statementHash
      ),
    callExceptionWithReason("Groth16 proof verification failed")
  );
  const otherTronNetworkVerifier = await deploy(
    signer,
    tronGroth16VerifierArtifact.abi,
    tronGroth16VerifierArtifact.bytecode,
    tronConstructorArgs({
      configuredNetworkId: ethers.encodeBytes32String("tron-alt-network"),
    })
  );
  assert.notEqual(
    await otherTronNetworkVerifier.destinationBindingHash(),
    expectedTronDestinationBindingHash
  );
  await assert.rejects(
    () =>
      otherTronNetworkVerifier.submitSccpMessageProof.staticCall(
        acceptingTronGroth16ProofBytes,
        tronInputs,
        statementHash
      ),
    callException
  );
  assert.equal(
    await tronGroth16Verifier.submitSccpMessageProof.staticCall(
      acceptingTronGroth16ProofBytes,
      tronInputs,
      statementHash
    ),
    messageId
  );
  const acceptedTronGroth16Tx =
    await tronGroth16Verifier.submitSccpMessageProof(
      acceptingTronGroth16ProofBytes,
      tronInputs,
      statementHash
    );
  const acceptedTronGroth16Receipt = await acceptedTronGroth16Tx.wait();
  assert.equal(await tronGroth16Verifier.usedMessageProofs(messageId), true);
  const acceptedTronGroth16Logs = acceptedTronGroth16Receipt.logs
    .filter(
      (log) => log.address.toLowerCase() === tronGroth16Address.toLowerCase()
    )
    .map((log) => {
      try {
        return tronGroth16Iface.parseLog(log);
      } catch (_error) {
        return null;
      }
    })
    .filter((log) => log && log.name === "MessageProofAccepted");
  assert.equal(acceptedTronGroth16Logs.length, 1);
  assert.equal(acceptedTronGroth16Logs[0].args.messageId, messageId);
  assert.equal(acceptedTronGroth16Logs[0].args.sourceDomain, 0n);
  assert.equal(acceptedTronGroth16Logs[0].args.commitmentRoot, tronInputs[3]);
  assert.equal(acceptedTronGroth16Logs[0].args.statementHash, statementHash);
  assert.equal(
    acceptedTronGroth16Logs[0].args.destinationBindingHash,
    expectedTronDestinationBindingHash
  );
  assert.equal(
    acceptedTronGroth16Logs[0].args.verifierBackendHash,
    ethers.keccak256(ethers.toUtf8Bytes("tron-groth16-bn254-v1"))
  );
  assert.equal(
    acceptedTronGroth16Logs[0].args.proofFamilyHash,
    ethers.keccak256(ethers.toUtf8Bytes("stark-fri-v1"))
  );
  assert.equal(acceptedTronGroth16Logs[0].args.networkId, tronNetworkId);

  await assert.rejects(
    async () => {
      const replayTronGroth16Tx =
        await tronGroth16Verifier.submitSccpMessageProof(
          acceptingTronGroth16ProofBytes,
          tronInputs,
          statementHash
        );
      await replayTronGroth16Tx.wait();
    },
    callException
  );

  const routeIdHash = ethers.keccak256(ethers.toUtf8Bytes("taira_tron_xor"));
  const assetKeyHash = ethers.keccak256(ethers.toUtf8Bytes("xor"));
  const tairaXor = await deploy(
    signer,
    tairaXorArtifact.abi,
    tairaXorArtifact.bytecode
  );
  const tairaXorAddress = await tairaXor.getAddress();
  assert.equal(await tairaXor.name(), "TAIRA XOR");
  assert.equal(await tairaXor.symbol(), "TairaXOR");
  assert.equal(await tairaXor.decimals(), 18n);
  assert.equal(await tairaXor.totalSupply(), 0n);

  await assert.rejects(
    async () => {
      const nonOwnerBridgeTx = await tairaXor
        .connect(outsider)
        .setBridge(await outsider.getAddress());
      await nonOwnerBridgeTx.wait();
    },
    callException
  );
  await assert.rejects(
    async () => {
      const zeroBridgeTx = await tairaXor.setBridge(ethers.ZeroAddress);
      await zeroBridgeTx.wait();
    },
    callException
  );
  await assert.rejects(
    async () => {
      const unauthorizedMintTx = await tairaXor.mint(await signer.getAddress(), 1);
      await unauthorizedMintTx.wait();
    },
    callException
  );
  await assert.rejects(
    async () => {
      const overdrawTx = await tairaXor.transfer(await outsider.getAddress(), 1);
      await overdrawTx.wait();
    },
    callException
  );

  const bridgeSourceBridge = await deploy(
    signer,
    tronSourceBridgeArtifact.abi,
    tronSourceBridgeArtifact.bytecode,
    tronSourceBridgeConstructorArgs()
  );
  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tairaXorBridgeArtifact.abi,
        tairaXorBridgeArtifact.bytecode,
        [
          ethers.ZeroAddress,
          tronGroth16Address,
          await bridgeSourceBridge.getAddress(),
          routeIdHash,
          assetKeyHash,
        ]
      );
    },
    callException
  );
  await assert.rejects(
    async () => {
      await deploy(
        signer,
        tairaXorBridgeArtifact.abi,
        tairaXorBridgeArtifact.bytecode,
        [
          tairaXorAddress,
          tronGroth16Address,
          await bridgeSourceBridge.getAddress(),
          ethers.ZeroHash,
          assetKeyHash,
        ]
      );
    },
    callException
  );

  const tairaXorBridge = await deploy(
    signer,
    tairaXorBridgeArtifact.abi,
    tairaXorBridgeArtifact.bytecode,
    [
      tairaXorAddress,
      tronGroth16Address,
      await bridgeSourceBridge.getAddress(),
      routeIdHash,
      assetKeyHash,
    ]
  );
  const tairaXorBridgeAddress = await tairaXorBridge.getAddress();
  assert.equal(await tairaXorBridge.routeIdHash(), routeIdHash);
  assert.equal(await tairaXorBridge.assetKeyHash(), assetKeyHash);
  assert.equal(await tairaXorBridge.networkId(), tronNetworkId);
  assert.equal(
    await tairaXorBridge.destinationBindingHash(),
    expectedTronDestinationBindingHash
  );
  const setBridgeTx = await tairaXor.setBridge(tairaXorBridgeAddress);
  await setBridgeTx.wait();
  const lockBridgeTx = await tairaXor.lockBridge();
  await lockBridgeTx.wait();
  await assert.rejects(
    async () => {
      const resetBridgeTx = await tairaXor.setBridge(await outsider.getAddress());
      await resetBridgeTx.wait();
    },
    callException
  );
  const transferSourceOwnershipTx =
    await bridgeSourceBridge.transferOwnership(tairaXorBridgeAddress);
  await transferSourceOwnershipTx.wait();
  assert.equal(await bridgeSourceBridge.owner(), tairaXorBridgeAddress);

  const recipient = await outsider.getAddress();
  const mintAmount = 12_345n;
  const bridgePayloadHash = computeTairaXorTransferPayloadHash(abi, {
    routeIdHash,
    assetKeyHash,
    bridgeAddress: tairaXorBridgeAddress,
    recipient,
    amount: mintAmount,
  });
  assert.equal(
    await tairaXorBridge.tairaXorTransferPayloadHash.staticCall(
      routeIdHash,
      assetKeyHash,
      recipient,
      mintAmount
    ),
    bridgePayloadHash
  );
  const bridgeMessageId = ethers.keccak256(
    ethers.toUtf8Bytes("taira-xor-bridge-message")
  );
  const bridgeInputs = [
    bridgeMessageId,
    bridgePayloadHash,
    ethers.zeroPadValue(ethers.toBeHex(5), 32),
    ethers.keccak256(ethers.toUtf8Bytes("taira-xor-commitment-root")),
    ethers.zeroPadValue(ethers.toBeHex(77), 32),
    ethers.keccak256(ethers.toUtf8Bytes("taira-xor-finality-block")),
  ];
  const bridgeStatementHash = ethers.keccak256(
    ethers.toUtf8Bytes("taira-xor-statement")
  );
  const bridgeProofBytes = await buildAcceptingGroth16ProofBytes(
    provider,
    abi,
    {
      publicInputs: bridgeInputs,
      sourceDomain: 0,
      statementHash: bridgeStatementHash,
      destinationBindingHash: expectedTronDestinationBindingHash,
      g1,
      g2,
    }
  );
  const standaloneConsumeTx = await tronGroth16Verifier.submitSccpMessageProof(
    bridgeProofBytes,
    bridgeInputs,
    bridgeStatementHash
  );
  await standaloneConsumeTx.wait();
  assert.equal(await tronGroth16Verifier.usedMessageProofs(bridgeMessageId), true);
  assert.equal(
    await tairaXorBridge.finalizeFromTaira.staticCall(
      bridgeProofBytes,
      bridgeInputs,
      bridgeStatementHash,
      routeIdHash,
      assetKeyHash,
      recipient,
      mintAmount
    ),
    bridgeMessageId
  );

  const wrongRouteHash = ethers.keccak256(ethers.toUtf8Bytes("wrong-route"));
  await assert.rejects(
    async () => {
      const wrongRouteTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        bridgeInputs,
        bridgeStatementHash,
        wrongRouteHash,
        assetKeyHash,
        recipient,
        mintAmount
      );
      await wrongRouteTx.wait();
    },
    callException
  );
  const wrongAssetHash = ethers.keccak256(ethers.toUtf8Bytes("wrong-asset"));
  await assert.rejects(
    async () => {
      const wrongAssetTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        bridgeInputs,
        bridgeStatementHash,
        routeIdHash,
        wrongAssetHash,
        recipient,
        mintAmount
      );
      await wrongAssetTx.wait();
    },
    callException
  );
  const wrongBridgePayloadInputs = bridgeInputs.slice();
  wrongBridgePayloadInputs[1] = ethers.keccak256(
    ethers.toUtf8Bytes("wrong-taira-xor-payload")
  );
  await assert.rejects(
    async () => {
      const wrongPayloadTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        wrongBridgePayloadInputs,
        bridgeStatementHash,
        routeIdHash,
        assetKeyHash,
        recipient,
        mintAmount
      );
      await wrongPayloadTx.wait();
    },
    callExceptionWithReason("Payload hash mismatch")
  );
  const wrongBridgeTargetInputs = bridgeInputs.slice();
  wrongBridgeTargetInputs[2] = ethers.zeroPadValue(ethers.toBeHex(4), 32);
  await assert.rejects(
    async () => {
      const wrongTargetTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        wrongBridgeTargetInputs,
        bridgeStatementHash,
        routeIdHash,
        assetKeyHash,
        recipient,
        mintAmount
      );
      await wrongTargetTx.wait();
    },
    callExceptionWithReason("Unexpected target domain")
  );
  await assert.rejects(
    async () => {
      const zeroRecipientTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        bridgeInputs,
        bridgeStatementHash,
        routeIdHash,
        assetKeyHash,
        ethers.ZeroAddress,
        mintAmount
      );
      await zeroRecipientTx.wait();
    },
    callException
  );
  await assert.rejects(
    async () => {
      const zeroAmountTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        bridgeInputs,
        bridgeStatementHash,
        routeIdHash,
        assetKeyHash,
        recipient,
        0
      );
      await zeroAmountTx.wait();
    },
    callException
  );

  const bridgeMintTx = await tairaXorBridge.finalizeFromTaira(
    bridgeProofBytes,
    bridgeInputs,
    bridgeStatementHash,
    routeIdHash,
    assetKeyHash,
    recipient,
    mintAmount
  );
  const bridgeMintReceipt = await bridgeMintTx.wait();
  assert.equal(await tairaXorBridge.usedMessageProofs(bridgeMessageId), true);
  assert.equal(await tairaXor.balanceOf(recipient), mintAmount);
  assert.equal(await tairaXor.totalSupply(), mintAmount);
  const tairaXorBridgeIface = new ethers.Interface(tairaXorBridgeArtifact.abi);
  const bridgeMintLogs = bridgeMintReceipt.logs
    .filter((log) => log.address.toLowerCase() === tairaXorBridgeAddress.toLowerCase())
    .map((log) => tairaXorBridgeIface.parseLog(log))
    .filter((log) => log.name === "TairaXorMintFinalized");
  assert.equal(bridgeMintLogs.length, 1);
  assert.equal(bridgeMintLogs[0].args.messageId, bridgeMessageId);
  assert.equal(bridgeMintLogs[0].args.recipient, recipient);
  assert.equal(bridgeMintLogs[0].args.amount, mintAmount);
  assert.equal(bridgeMintLogs[0].args.payloadHash, bridgePayloadHash);
  await assert.rejects(
    async () => {
      const bridgeReplayTx = await tairaXorBridge.finalizeFromTaira(
        bridgeProofBytes,
        bridgeInputs,
        bridgeStatementHash,
        routeIdHash,
        assetKeyHash,
        recipient,
        mintAmount
      );
      await bridgeReplayTx.wait();
    },
    callExceptionWithReason("Message proof already used")
  );

  const transferAmount = 100n;
  const transferTx = await tairaXor
    .connect(outsider)
    .transfer(await signer.getAddress(), transferAmount);
  await transferTx.wait();
  assert.equal(await tairaXor.balanceOf(await signer.getAddress()), transferAmount);
  const approveTx = await tairaXor
    .connect(outsider)
    .approve(await signer.getAddress(), 7n);
  await approveTx.wait();
  const transferFromTx = await tairaXor.transferFrom(recipient, await signer.getAddress(), 7n);
  await transferFromTx.wait();
  assert.equal(await tairaXor.allowance(recipient, await signer.getAddress()), 0n);
  assert.equal(await tairaXor.balanceOf(await signer.getAddress()), 107n);

  await assert.rejects(
    async () => {
      const badBurnRouteTx = await tairaXorBridge
        .connect(outsider)
        .burnToTaira(wrongRouteHash, assetKeyHash, ethers.toUtf8Bytes("testu1@taira"), 1);
      await badBurnRouteTx.wait();
    },
    callExceptionWithReason("Unexpected route")
  );
  await assert.rejects(
    async () => {
      const badBurnAssetTx = await tairaXorBridge
        .connect(outsider)
        .burnToTaira(routeIdHash, wrongAssetHash, ethers.toUtf8Bytes("testu1@taira"), 1);
      await badBurnAssetTx.wait();
    },
    callExceptionWithReason("Unexpected asset")
  );
  await assert.rejects(
    async () => {
      const zeroBurnAmountTx = await tairaXorBridge
        .connect(outsider)
        .burnToTaira(routeIdHash, assetKeyHash, ethers.toUtf8Bytes("testu1@taira"), 0);
      await zeroBurnAmountTx.wait();
    },
    callException
  );
  await assert.rejects(
    async () => {
      const emptyRecipientTx = await tairaXorBridge
        .connect(outsider)
        .burnToTaira(routeIdHash, assetKeyHash, "0x", 1);
      await emptyRecipientTx.wait();
    },
    callException
  );
  await assert.rejects(
    async () => {
      const outsiderOverburnTx = await tairaXorBridge
        .connect(outsider)
        .burnToTaira(routeIdHash, assetKeyHash, ethers.toUtf8Bytes("testu1@taira"), mintAmount);
      await outsiderOverburnTx.wait();
    },
    callException
  );

  const burnRecipient = ethers.toUtf8Bytes("testu2@taira");
  const burnRecipientHash = ethers.keccak256(burnRecipient);
  const burnAmount = 222n;
  const expectedBurnDigest = computeTairaXorBurnSourceEventDigest(abi, {
    routeIdHash,
    assetKeyHash,
    bridgeAddress: tairaXorBridgeAddress,
    burner: recipient,
    tairaRecipientHash: burnRecipientHash,
    amount: burnAmount,
    nonce: 0n,
  });
  assert.equal(
    await tairaXorBridge.tairaXorBurnSourceEventDigest.staticCall(
      routeIdHash,
      assetKeyHash,
      recipient,
      burnRecipientHash,
      burnAmount,
      0
    ),
    expectedBurnDigest
  );
  const burnTx = await tairaXorBridge
    .connect(outsider)
    .burnToTaira(routeIdHash, assetKeyHash, burnRecipient, burnAmount);
  const burnReceipt = await burnTx.wait();
  assert.equal(await tairaXorBridge.burnNonce(), 1n);
  assert.equal(await bridgeSourceBridge.submittedSourceEvents(expectedBurnDigest), true);
  assert.equal(await tairaXor.balanceOf(recipient), mintAmount - transferAmount - 7n - burnAmount);
  assert.equal(await tairaXor.totalSupply(), mintAmount - burnAmount);
  const bridgeBurnLogs = burnReceipt.logs
    .filter((log) => log.address.toLowerCase() === tairaXorBridgeAddress.toLowerCase())
    .map((log) => tairaXorBridgeIface.parseLog(log))
    .filter((log) => log.name === "TairaXorBurnStarted");
  assert.equal(bridgeBurnLogs.length, 1);
  assert.equal(bridgeBurnLogs[0].args.sourceEventDigest, expectedBurnDigest);
  assert.equal(bridgeBurnLogs[0].args.burner, recipient);
  assert.equal(bridgeBurnLogs[0].args.tairaRecipientHash, burnRecipientHash);
  assert.equal(bridgeBurnLogs[0].args.amount, burnAmount);
  assert.equal(bridgeBurnLogs[0].args.nonce, 0n);
  assert.equal(ethers.hexlify(bridgeBurnLogs[0].args.tairaRecipient), ethers.hexlify(burnRecipient));

  console.log("sccp_message_bridge_smoke: ok");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
