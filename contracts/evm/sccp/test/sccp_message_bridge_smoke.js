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
  const attestationDigest = ethers.keccak256(
    abi.encode(
      ["bytes32", "bytes32", "uint256", "bytes32", "bytes32", "bytes32", "bytes32"],
      [
        ethers.keccak256(ethers.toUtf8Bytes("iroha:sccp:evm-attestation:v1")),
        messageId,
        0,
        publicInputs[3],
        publicInputsHash,
        statementHash,
        nativeProofHash,
      ]
    )
  );
  const attestationSignature = attestor.signingKey.sign(attestationDigest).serialized;
  const proofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes"],
    [
      1,
      messageId,
      0,
      publicInputs[3],
      nativeProofHash,
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
    (error) => error && error.code === "CALL_EXCEPTION"
  );

  const tamperedProofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes"],
    [
      1,
      ethers.keccak256(ethers.toUtf8Bytes("message-2")),
      0,
      publicInputs[3],
      nativeProofHash,
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
    (error) => error && error.code === "CALL_EXCEPTION"
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
    (error) => error && error.code === "CALL_EXCEPTION"
  );

  const unauthorized = ethers.Wallet.createRandom();
  const unauthorizedSignature = unauthorized.signingKey.sign(attestationDigest).serialized;
  const unauthorizedProofBytes = abi.encode(
    ["uint256", "bytes32", "uint256", "bytes32", "bytes32", "bytes"],
    [
      1,
      messageId,
      0,
      publicInputs[3],
      nativeProofHash,
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
    (error) => error && error.code === "CALL_EXCEPTION"
  );

  console.log("sccp_message_bridge_smoke: ok");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
