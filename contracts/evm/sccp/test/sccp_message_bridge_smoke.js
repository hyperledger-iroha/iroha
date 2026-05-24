const fs = require("fs");
const path = require("path");
const assert = require("assert");
const solc = require("solc");
const ganache = require("ganache");
const { ethers } = require("ethers");

function contractPath(fileName) {
  return path.join(__dirname, "..", fileName);
}

function loadSource(fileName) {
  return {
    content: fs.readFileSync(contractPath(fileName), "utf8"),
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

function callException(error) {
  return error && error.code === "CALL_EXCEPTION";
}

async function main() {
  const contracts = compileContracts();
  const provider = new ethers.BrowserProvider(ganache.provider());
  const signer = await provider.getSigner();
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
  const groth16VerifierArtifact = artifact(
    contracts,
    "SccpGroth16Bn254MessageVerifier.sol",
    "SccpGroth16Bn254MessageVerifier"
  );

  const verifier = await deploy(
    signer,
    verifierArtifact.abi,
    verifierArtifact.bytecode,
    [[attestor.address], 1]
  );

  const bridge = await deploy(
    signer,
    bridgeArtifact.abi,
    bridgeArtifact.bytecode,
    [
      await verifier.getAddress(),
      "evm-secp256k1-keccak-v1",
      "stark-fri-v1",
      ethers.encodeBytes32String("evm-devnet"),
      0,
      1,
    ]
  );
  const wrongLaneBridge = await deploy(
    signer,
    bridgeArtifact.abi,
    bridgeArtifact.bytecode,
    [
      await verifier.getAddress(),
      "evm-secp256k1-keccak-v1",
      "stark-fri-v1",
      ethers.encodeBytes32String("evm-devnet"),
      0,
      2,
    ]
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
  const destinationBindingHash = ethers.keccak256(
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
      ],
      [
        ethers.keccak256(
          ethers.toUtf8Bytes("iroha:sccp:evm-destination-binding:v1")
        ),
        verifierBackendHash,
        proofFamilyHash,
        networkId,
        0,
        1,
        await verifier.getAddress(),
        await bridge.getAddress(),
      ]
    )
  );
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

  const tx = await bridge.submitSccpMessageProof(
    proofBytes,
    publicInputs,
    statementHash
  );
  await tx.wait();

  assert.equal(await bridge.usedMessageProofs(messageId), true);

  await assert.rejects(
    async () => {
      const replayTx = await bridge.submitSccpMessageProof(
        proofBytes,
        publicInputs,
        statementHash
      );
      await replayTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const wrongLaneTx = await wrongLaneBridge.submitSccpMessageProof(
        proofBytes,
        publicInputs,
        statementHash
      );
      await wrongLaneTx.wait();
    },
    callException
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
    async () => {
      const badTx = await bridge.submitSccpMessageProof(
        tamperedProofBytes,
        publicInputs,
        statementHash
      );
      await badTx.wait();
    },
    callException
  );

  await assert.rejects(
    async () => {
      const badStatementTx = await bridge.submitSccpMessageProof(
        proofBytes,
        publicInputs,
        ethers.keccak256(ethers.toUtf8Bytes("wrong-statement"))
      );
      await badStatementTx.wait();
    },
    callException
  );

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
    async () => {
      const badSignerTx = await bridge.submitSccpMessageProof(
        unauthorizedProofBytes,
        publicInputs,
        statementHash
      );
      await badSignerTx.wait();
    },
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

  const groth16Bridge = await deploy(
    signer,
    bridgeArtifact.abi,
    bridgeArtifact.bytecode,
    [
      await groth16Verifier.getAddress(),
      "evm-groth16-bn254-v1",
      "stark-fri-v1",
      networkId,
      0,
      1,
    ]
  );
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
      const badGrothTx = await groth16Bridge.submitSccpMessageProof(
        invalidGroth16ProofBytes,
        publicInputs,
        statementHash
      );
      await badGrothTx.wait();
    },
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

  console.log("sccp_message_bridge_smoke: ok");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
